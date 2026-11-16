//! The project file: create, open, read-only handling for newer files, migration, and backups
//! (specification 4.1).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::json;

use crate::crypto::{self, Secret};
use crate::model::CodingMode;
use crate::{event, schema, Error, Result};

/// The file extension for a project. Provisional: the final extension is still an open question
/// (specification section 30), and this constant is the only place that has to change.
pub const PROJECT_EXTENSION: &str = "pqda";

/// How many dated backups are kept by default (specification 4.1).
pub const DEFAULT_KEEP_BACKUPS: usize = 10;

/// Who is working, and what to do when a project opens.
#[derive(Clone)]
pub struct OpenOptions {
    /// The name recorded as the actor on every event this session logs.
    pub actor: String,
    /// Make a dated backup copy as the project opens (specification 4.1).
    pub backup_on_open: bool,
    /// How many dated backups to keep.
    pub keep_backups: usize,
    /// A folder to keep backups in, instead of beside the project. Each project gets its own folder inside it.
    pub backup_dir: Option<PathBuf>,
    /// The passphrase of an encrypted project: needed to open one, and, when a project is created, what encrypts it. Ignored when
    /// opening a project that is not encrypted.
    pub passphrase: Option<String>,
}

impl OpenOptions {
    pub fn new(actor: impl Into<String>) -> Self {
        Self { actor: actor.into(), backup_on_open: true, keep_backups: DEFAULT_KEEP_BACKUPS, backup_dir: None, passphrase: None }
    }

    /// The same options with a passphrase.
    pub fn with_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }
}

impl std::fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The passphrase is never printed.
        f.debug_struct("OpenOptions")
            .field("actor", &self.actor)
            .field("backup_on_open", &self.backup_on_open)
            .field("keep_backups", &self.keep_backups)
            .field("backup_dir", &self.backup_dir)
            .field("passphrase", &self.passphrase.as_ref().map(|_| "..."))
            .finish()
    }
}

/// Settings for a new project.
#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub method_template: String,
    pub research_questions: String,
    pub coding_mode: CodingMode,
    /// The bundled sample this project is a copy of, if it is one. It is written into the project's
    /// creation event, so it needs no extra table and no new format version, and it can never change.
    pub sample_id: Option<String>,
}

impl NewProject {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            method_template: "free".to_string(),
            research_questions: String::new(),
            coding_mode: CodingMode::FreeSpan,
            sample_id: None,
        }
    }
}

/// A summary of an open project.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub name: String,
    pub project_uid: String,
    pub method_template: String,
    /// The research question(s) given when the project was made; empty when none was.
    pub research_questions: String,
    pub coding_mode: String,
    pub schema_version: i32,
    pub read_only: bool,
    pub sources: i64,
    pub units: i64,
    pub codes: i64,
    pub coders: i64,
    pub codings: i64,
    pub events: i64,
    /// The bundled sample this project was copied from, or `None` for every other project. A copy is an
    /// ordinary project that only remembers where it came from.
    pub sample_id: Option<String>,
    /// `packet` (a person may build a prompt packet to take to a chatbot) or `off` (no packet can be built).
    pub ai_mode: String,
    /// True when every page of the project file is encrypted (decision 157).
    pub encrypted: bool,
    /// Who this project is a packet for (decision 161), or `None` for an ordinary project.
    pub packet: Option<crate::CoderPacketInfo>,
    /// True when the offline lock is on (decision 165): no prompt packet can be built for an AI service.
    pub offline_lock: bool,
}

/// An open project file. Every change is one transaction that also appends an event.
pub struct Project {
    pub(crate) conn: Connection,
    pub(crate) path: PathBuf,
    pub(crate) read_only: bool,
    pub(crate) notices: Vec<String>,
    pub(crate) session: String,
    pub(crate) actor: String,
    pub(crate) keep_backups: usize,
    pub(crate) backup_dir: Option<PathBuf>,
    /// The passphrase of an encrypted project, kept in memory while it is open so backups and merges can be encrypted with it.
    pub(crate) key: Option<Secret>,
}

impl Project {
    /// Creates a new project file. Fails if the file already exists; removes any half-made file on failure.
    pub fn create(path: impl AsRef<Path>, new: &NewProject, opts: &OpenOptions) -> Result<Project> {
        let path = path.as_ref();
        if path.exists() {
            return Err(Error::AlreadyExists(path.to_path_buf()));
        }
        if new.name.trim().is_empty() {
            return Err(Error::Invalid("a project needs a name".to_string()));
        }
        match Self::create_inner(path, new, opts) {
            Ok(project) => Ok(project),
            Err(e) => {
                remove_project_files(path);
                Err(e)
            }
        }
    }

    fn create_inner(path: &Path, new: &NewProject, opts: &OpenOptions) -> Result<Project> {
        let key = match &opts.passphrase {
            Some(p) => Some(crypto::clean_new_passphrase(p)?),
            None => None,
        };
        let mut conn = Connection::open(path)?;
        if let Some(k) = &key {
            crypto::apply_key(&conn, k)?;
        }
        conn.pragma_update(None, "foreign_keys", true)?;
        enable_wal(&conn)?;
        schema::install(&mut conn)?;
        conn.execute(
            "INSERT INTO project (id, project_uid, name, method_template, research_questions, coding_mode, created_at)
             VALUES (1, lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![new.name.trim(), new.method_template, new.research_questions, new.coding_mode.as_str()],
        )?;
        let project = Project {
            session: new_session(&conn)?,
            conn,
            path: path.to_path_buf(),
            read_only: false,
            notices: Vec::new(),
            actor: opts.actor.clone(),
            keep_backups: opts.keep_backups,
            backup_dir: opts.backup_dir.clone(),
            key,
        };
        let mut created = json!({
            "name": new.name.trim(),
            "method_template": new.method_template,
            "coding_mode": new.coding_mode.as_str(),
            "schema_version": schema::SCHEMA_VERSION,
        });
        if project.key.is_some() {
            created["encrypted"] = json!(true);
        }
        if let Some(sample) = &new.sample_id {
            created["sample_id"] = json!(sample);
        }
        event::insert(&project.conn, &project.actor, &project.session, "project.create", Some(("project", 1)), None, Some(&created))?;
        Ok(project)
    }

    /// Opens an existing project.
    ///
    /// * A file from a newer version of the app opens read-only, with a notice.
    /// * A file from an older version is copied aside, then migrated forward.
    /// * A dated backup is made unless `opts.backup_on_open` is false.
    pub fn open(path: impl AsRef<Path>, opts: &OpenOptions) -> Result<Project> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} does not exist", path.display()),
            )));
        }
        // An encrypted file does not begin like a SQLite file: it needs its passphrase before anything in it can be read.
        let key = if crypto::file_looks_encrypted(path)? {
            match &opts.passphrase {
                Some(p) => Some(crypto::clean_passphrase(p)?),
                None => return Err(Error::NeedsPassphrase(path.to_path_buf())),
            }
        } else {
            None
        };
        let (application_id, version) = read_header(path, key.as_ref())?;
        if application_id != schema::APPLICATION_ID || version < 1 {
            return Err(Error::NotAProject(path.to_path_buf()));
        }

        let mut notices = Vec::new();
        let mut migrated_from = None;
        let (conn, read_only) = if version > schema::SCHEMA_VERSION {
            notices.push(format!(
                "This project was made by a newer version of PrismQDA (project format {version}; this version understands up to {}). It is open read-only so nothing is lost.",
                schema::SCHEMA_VERSION
            ));
            let conn = open_conn(path, OpenFlags::SQLITE_OPEN_READ_ONLY, key.as_ref())?;
            (conn, true)
        } else {
            let mut conn = open_conn(path, OpenFlags::default(), key.as_ref())?;
            conn.pragma_update(None, "foreign_keys", true)?;
            enable_wal(&conn)?;
            if version < schema::SCHEMA_VERSION {
                let copy = sibling(path, &format!("before-format-{version}"));
                backup_to(&conn, &copy, key.as_ref())?;
                schema::migrate(&mut conn, version, schema::MIGRATIONS)?;
                notices.push(format!(
                    "This project was upgraded from format {version} to {}. The original was kept as {}.",
                    schema::SCHEMA_VERSION,
                    copy.display()
                ));
                migrated_from = Some((version, copy));
            }
            (conn, false)
        };

        let mut project = Project {
            session: new_session(&conn)?,
            conn,
            path: path.to_path_buf(),
            read_only,
            notices,
            actor: opts.actor.clone(),
            keep_backups: opts.keep_backups,
            backup_dir: opts.backup_dir.clone(),
            key,
        };
        if let Some((from, copy)) = migrated_from {
            event::insert(
                &project.conn,
                &project.actor,
                &project.session,
                "project.migrate",
                Some(("project", 1)),
                Some(&json!({ "schema_version": from })),
                Some(&json!({ "schema_version": schema::SCHEMA_VERSION, "original_kept_as": copy.display().to_string() })),
            )?;
        }
        if !read_only && opts.backup_on_open {
            // A backup that cannot be made must not keep someone out of their own project: it is told, and the project opens.
            match project.backup_now_noted() {
                Ok((_, Some(note))) => project.notices.push(note),
                Ok((_, None)) => {}
                Err(e) => project.notices.push(format!("No backup was made when the project opened: {e}. The project itself opened normally.")),
            }
        }
        Ok(project)
    }

    /// Where the project file is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many dated backups are kept from now on (at least one). Takes effect at the next backup.
    pub fn set_keep_backups(&mut self, keep: usize) {
        self.keep_backups = keep.max(1);
    }

    /// True when the file came from a newer version, so every change is refused.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Messages the person should see after opening, such as the read-only notice.
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    /// Where the backups of this project go: `<name>.backups` beside the project, or, when a backup folder was
    /// chosen, a folder for this project inside it (named for the project and the start of its identity, so two
    /// projects with the same file name never share backups).
    pub fn backup_folder(&self) -> Result<PathBuf> {
        let stem = file_stem(&self.path);
        Ok(match &self.backup_dir {
            None => self.path.with_file_name(format!("{stem}.backups")),
            Some(root) => {
                let uid: String = self.conn.query_row("SELECT project_uid FROM project WHERE id = 1", [], |r| r.get(0))?;
                root.join(format!("{stem} ({}).backups", &uid[..uid.len().min(8)]))
            }
        })
    }

    /// The name recorded on every change from now on (the name the person goes by in the history of a project).
    pub fn set_actor(&mut self, name: &str) {
        self.actor = name.to_string();
    }

    /// The folder backups go to from now on: `None` for beside the project. Takes effect at the next backup.
    pub fn set_backup_dir(&mut self, dir: Option<PathBuf>) {
        self.backup_dir = dir;
    }

    /// Writes a consistent copy of the whole project file to `dest` (the way a backup is made), without touching the project.
    pub(crate) fn copy_database(&self, dest: &Path) -> Result<()> {
        backup_to(&self.conn, dest, self.key.as_ref())
    }

    /// Makes a dated backup now and removes the oldest beyond the keep count. Backups sit in the project's
    /// backup folder and are ordinary project files.
    pub fn backup_now(&self) -> Result<PathBuf> {
        Ok(self.backup_now_noted()?.0)
    }

    /// Like [`backup_now`](Project::backup_now), and says so when the chosen backup folder could not be used and the
    /// backup was made beside the project instead (the second part is the sentence to show).
    pub fn backup_now_noted(&self) -> Result<(PathBuf, Option<String>)> {
        self.ensure_writable()?;
        let stem = file_stem(&self.path);
        let ext = file_ext(&self.path);
        let ts: String = self.conn.query_row("SELECT strftime('%Y%m%dT%H%M%S', 'now')", [], |r| r.get(0))?;
        let make = |dir: &Path| -> Result<PathBuf> {
            fs::create_dir_all(dir)?;
            let mut n = 1;
            let dest = loop {
                let candidate = dir.join(format!("{stem}-{ts}-{n:02}.{ext}"));
                if !candidate.exists() {
                    break candidate;
                }
                n += 1;
            };
            backup_to(&self.conn, &dest, self.key.as_ref())?;
            prune_backups(dir, &stem, &ext, self.keep_backups)?;
            Ok(dest)
        };
        let dir = self.backup_folder()?;
        match make(&dir) {
            Ok(dest) => Ok((dest, None)),
            Err(e) if self.backup_dir.is_some() => {
                // The folder the person chose is gone or refuses writing (a drive that is not plugged in, say): the project
                // is still protected, beside itself, and the person is told why that happened.
                let beside = self.path.with_file_name(format!("{stem}.backups"));
                let dest = make(&beside)?;
                let note = format!(
                    "The backup folder {} could not be used ({e}), so this backup was made beside the project instead.",
                    self.backup_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default()
                );
                Ok((dest, Some(note)))
            }
            Err(e) => Err(e),
        }
    }

    /// Counts and settings for display.
    pub fn info(&self) -> Result<ProjectInfo> {
        let (name, project_uid, method_template, research_questions, coding_mode, ai_mode) = self.conn.query_row(
            "SELECT name, project_uid, method_template, research_questions, coding_mode, ai_mode FROM project WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )?;
        let schema_version: i32 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        // The creation event is the first event and the log is append-only, so this can never change.
        let sample_id: Option<String> = self
            .conn
            .query_row("SELECT json_extract(after_json, '$.sample_id') FROM event WHERE action = 'project.create' ORDER BY id LIMIT 1", [], |r| r.get(0))
            .optional()?
            .flatten();
        Ok(ProjectInfo {
            path: self.path.clone(),
            name,
            project_uid,
            method_template,
            research_questions,
            coding_mode,
            schema_version,
            read_only: self.read_only,
            sources: self.count("source")?,
            units: self.count("unit")?,
            codes: self.count("code")?,
            coders: self.count("coder")?,
            // Coded areas of pictures are codings too.
            codings: self.count("coding")? + self.count("region")?,
            events: self.count("event")?,
            sample_id,
            ai_mode,
            encrypted: self.key.is_some(),
            packet: self.coder_packet_info()?,
            offline_lock: self.offline_lock()?,
        })
    }

    /// Sets whether this project can have AI prompt packets built (`packet`) or not (`off`).
    pub fn set_ai_mode(&mut self, mode: &str) -> Result<()> {
        self.ensure_writable()?;
        if !matches!(mode, "packet" | "off") {
            return Err(Error::Invalid(format!("the AI setting is packet or off, not '{mode}'")));
        }
        // An instructor's assignment (decision 200) can close AI prompt packets in the packet it hands out; they stay closed there.
        if mode == "packet" && self.coder_packet_info()?.and_then(|p| p.assignment).is_some_and(|a| !a.ai_allowed) {
            return Err(Error::Invalid("your instructor has turned AI prompt packets off for this assignment, so they cannot be turned on here".to_string()));
        }
        let before: String = self.conn.query_row("SELECT ai_mode FROM project WHERE id = 1", [], |r| r.get(0))?;
        if before == mode {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE project SET ai_mode = ?1 WHERE id = 1", [mode])?;
        event::insert(&tx, &self.actor, &self.session, "project.ai_mode", None, Some(&serde_json::json!({ "ai_mode": before })), Some(&serde_json::json!({ "ai_mode": mode })))?;
        tx.commit()?;
        Ok(())
    }

    /// Sets whether a document may be put in a packet for an AI (false) or never may (true).
    pub fn set_source_do_not_share(&mut self, source_id: i64, do_not_share: bool) -> Result<()> {
        self.ensure_writable()?;
        let before: i64 = self
            .conn
            .query_row("SELECT do_not_share FROM source WHERE id = ?1", [source_id], |r| r.get(0))
            .optional()?
            .ok_or(Error::NotFound { what: "source", id: source_id })?;
        if (before != 0) == do_not_share {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE source SET do_not_share = ?1 WHERE id = ?2", rusqlite::params![do_not_share as i64, source_id])?;
        event::insert(&tx, &self.actor, &self.session, "source.share", Some(("source", source_id)), Some(&serde_json::json!({ "do_not_share": before != 0 })), Some(&serde_json::json!({ "do_not_share": do_not_share })))?;
        tx.commit()?;
        Ok(())
    }

    fn count(&self, table: &'static str) -> Result<i64> {
        // `table` is always one of our own table names, never user input.
        Ok(self.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
    }

    pub(crate) fn ensure_writable(&self) -> Result<()> {
        if self.read_only {
            Err(Error::ReadOnly("it was made by a newer version of PrismQDA".to_string()))
        } else {
            Ok(())
        }
    }
}

pub(crate) fn enable_wal(conn: &Connection) -> Result<()> {
    // Write-ahead logging makes each commit all-or-nothing, so a crash never leaves a half-written project.
    let mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::Invalid(format!("this location cannot keep a project safely (journal mode {mode})")));
    }
    Ok(())
}

fn new_session(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT lower(hex(randomblob(8)))", [], |r| r.get(0))?)
}

/// Opens the file as it is (the flags say how), giving the connection its key when the project is encrypted. A wrong key is found out
/// here, before anything is read for real.
pub(crate) fn open_conn(path: &Path, flags: OpenFlags, key: Option<&Secret>) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, flags)?;
    if let Some(k) = key {
        crypto::apply_key(&conn, k)?;
        crypto::check_open(&conn)?;
    }
    Ok(conn)
}

/// Reads the application id and format version without changing the file.
fn read_header(path: &Path, key: Option<&Secret>) -> Result<(i32, i32)> {
    let not_a_project = |_: rusqlite::Error| Error::NotAProject(path.to_path_buf());
    let conn = open_conn(path, OpenFlags::SQLITE_OPEN_READ_ONLY, key).map_err(|e| match e {
        Error::WrongPassphrase => Error::WrongPassphrase,
        _ => Error::NotAProject(path.to_path_buf()),
    })?;
    let application_id = conn.pragma_query_value(None, "application_id", |r| r.get(0)).map_err(not_a_project)?;
    let version = conn.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(not_a_project)?;
    Ok((application_id, version))
}

/// Checks that a folder can hold backups: it exists, it is a folder, and a file can really be written in it (a
/// probe file is made and removed). The message says which of these failed, so it can be shown as it is.
pub fn check_backup_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Err(Error::Invalid(format!("{} was not found.", dir.display())));
    }
    if !dir.is_dir() {
        return Err(Error::Invalid(format!("{} is a file, not a folder.", dir.display())));
    }
    let probe = dir.join(format!(".prismqda-write-check-{}", std::process::id()));
    match fs::write(&probe, b"x") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(Error::Invalid(format!("Nothing can be written in {} ({e}).", dir.display()))),
    }
}

/// Copies a live database with SQLite's online backup, so the copy is consistent even mid-use. With a key, the copy is encrypted with it.
///
/// The copy is then opened again, with the same key, and checked with SQLite's `quick_check`. A backup that is only found to be damaged on the
/// day it is needed is no backup, and the online backup of the library underneath has had a bug that wrote a damaged copy without saying so
/// (fixed in SQLite3 Multiple Ciphers 2.7.0). A copy that does not pass is removed and reported, and the project is not touched.
pub(crate) fn backup_to(source: &Connection, dest: &Path, key: Option<&Secret>) -> Result<()> {
    {
        let mut target = Connection::open(dest)?;
        if let Some(k) = key {
            crypto::apply_key(&target, k)?;
        }
        let backup = rusqlite::backup::Backup::new(source, &mut target)?;
        backup.run_to_completion(256, Duration::ZERO, None)?;
    }
    if let Err(e) = verify_backup(dest, key) {
        remove_project_files(dest);
        return Err(e);
    }
    Ok(())
}

/// Opens a copy that was just written, with the key it was written with, and runs `PRAGMA quick_check` on it. Anything but "ok" is an error.
pub(crate) fn verify_backup(dest: &Path, key: Option<&Secret>) -> Result<()> {
    let trouble = |why: String| Error::Invalid(format!("The backup {} was written but did not pass its check ({why}), so it was removed. The project itself is not changed. Try again, and if it keeps happening, tell the maintainer.", dest.display()));
    let check = Connection::open(dest).map_err(|e| trouble(e.to_string()))?;
    if let Some(k) = key {
        crypto::apply_key(&check, k).map_err(|e| trouble(e.to_string()))?;
    }
    let first: String = check.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|e| trouble(e.to_string()))?;
    if first != "ok" {
        return Err(trouble(first));
    }
    Ok(())
}

fn prune_backups(dir: &Path, stem: &str, ext: &str, keep: usize) -> Result<()> {
    let prefix = format!("{stem}-");
    let suffix = format!(".{ext}");
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(&prefix) && n.ends_with(&suffix))
                .unwrap_or(false)
        })
        .collect();
    // The names carry a sortable timestamp and a two-digit counter, so name order is time order.
    files.sort();
    let excess = files.len().saturating_sub(keep);
    for old in files.into_iter().take(excess) {
        remove_project_files(&old);
    }
    Ok(())
}

pub(crate) fn remove_project_files(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let _ = fs::remove_file(PathBuf::from(name));
    }
}

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("project").to_string()
}

pub(crate) fn file_ext(path: &Path) -> String {
    path.extension().and_then(|s| s.to_str()).unwrap_or(PROJECT_EXTENSION).to_string()
}

/// `<dir>/<stem>.<tag>.<ext>` beside the project.
pub(crate) fn sibling(path: &Path, tag: &str) -> PathBuf {
    path.with_file_name(format!("{}.{tag}.{}", file_stem(path), file_ext(path)))
}

#[cfg(test)]
mod backup_check_tests {
    use super::*;

    /// A small database to back up, with a key or without.
    fn source(dir: &Path, key: Option<&Secret>) -> Connection {
        let conn = Connection::open(dir.join("source.db")).unwrap();
        if let Some(k) = key {
            crypto::apply_key(&conn, k).unwrap();
        }
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, words TEXT); INSERT INTO t (words) VALUES ('first'), ('second'), ('third');").unwrap();
        conn
    }

    #[test]
    fn a_backup_is_opened_again_and_checked_before_it_counts() {
        let dir = tempfile::tempdir().unwrap();
        for key in [None, Some(Secret::new("a passphrase long enough".to_string()))] {
            let conn = source(dir.path(), key.as_ref());
            let dest = dir.path().join(format!("copy-{}.db", key.is_some()));
            backup_to(&conn, &dest, key.as_ref()).unwrap();
            let back = Connection::open(&dest).unwrap();
            if let Some(k) = key.as_ref() {
                crypto::apply_key(&back, k).unwrap();
            }
            let n: i64 = back.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0)).unwrap();
            assert_eq!(n, 3, "the backup holds the rows");
            drop(back);
            drop(conn);
            std::fs::remove_file(dir.path().join("source.db")).unwrap();
        }
    }

    #[test]
    fn a_copy_that_does_not_pass_the_check_is_refused_in_words() {
        let dir = tempfile::tempdir().unwrap();
        let conn = source(dir.path(), None);
        let dest = dir.path().join("copy.db");
        backup_to(&conn, &dest, None).unwrap();
        verify_backup(&dest, None).unwrap();
        // Damage the first page: the file no longer says it is a database.
        let mut bytes = std::fs::read(&dest).unwrap();
        bytes[..16].fill(0);
        std::fs::write(&dest, &bytes).unwrap();
        let why = verify_backup(&dest, None).unwrap_err().to_string();
        assert!(why.contains("did not pass its check") && why.contains("project itself is not changed"), "{why}");
        // A copy of an encrypted project read with another key does not pass either.
        let right = Secret::new("a passphrase long enough".to_string());
        let wrong = Secret::new("another passphrase, also long".to_string());
        drop(conn);
        std::fs::remove_file(dir.path().join("source.db")).unwrap();
        let enc = source(dir.path(), Some(&right));
        let sealed = dir.path().join("sealed.db");
        backup_to(&enc, &sealed, Some(&right)).unwrap();
        assert!(verify_backup(&sealed, Some(&wrong)).is_err());
    }
}
