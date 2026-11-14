//! Encrypted projects (specification 27, decisions 137 and 157).
//!
//! An encrypted project is the project's own SQLite file with every page encrypted: no readable copy sits inside a wrapper, none is
//! written to a temporary file while the project is open, and the write-ahead file beside it is encrypted too. The pages are
//! encrypted by SQLite3 Multiple Ciphers (the SQLite that is built into the program) in its SQLCipher 4 compatible scheme:
//! AES-256-CBC, an HMAC-SHA512 on every page, and a key made from the passphrase with PBKDF2-HMAC-SHA512 at 256,000 rounds. Any
//! program that opens SQLCipher 4 files opens one with the same passphrase (the passphrase is first put in Unicode NFC, which changes
//! nothing for a plain English one).
//!
//! **A lost passphrase cannot be recovered by anyone.** The passphrase is never stored in the project or anywhere else; while a
//! project is open it is kept in memory so backups and merges can be encrypted with it, and it is overwritten when the project closes.
//!
//! Turning encryption on, off, or changing the passphrase writes a new file beside the old one, checks it, swaps the two, and then
//! overwrites the old file with zeros before deleting it (best effort: a solid-state drive, a cloud folder, or a backup made earlier
//! may still hold the old bytes, so the project says where earlier backups are and offers to delete them).

use std::fs::{self, File, OpenOptions as FileOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::json;
use unicode_normalization::UnicodeNormalization;

use crate::event;
use crate::project::{enable_wal, file_ext, file_stem, sibling, Project, PROJECT_EXTENSION};
use crate::{Error, Result};

/// What the encryption is, in words, for the window, the log, and the documents.
pub const CIPHER_DESCRIPTION: &str = "AES-256-CBC with an HMAC-SHA512 on each page; the key is made from the passphrase with PBKDF2-HMAC-SHA512 at 256,000 rounds (SQLCipher 4 compatible)";

/// The shortest passphrase a person can choose when turning encryption on. A longer one is much safer.
pub const MIN_PASSPHRASE_CHARS: usize = 8;
const MAX_PASSPHRASE_CHARS: usize = 512;

const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// A passphrase kept in memory, overwritten when it is let go.
#[derive(Clone)]
pub(crate) struct Secret(Vec<u8>);

impl Secret {
    pub(crate) fn new(text: String) -> Secret {
        Secret(text.into_bytes())
    }

    pub(crate) fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            *b = 0;
        }
        std::hint::black_box(&self.0);
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

/// The checks every passphrase passes, new or typed to open: put in Unicode NFC, not empty, no NUL, and not read as a raw key.
pub(crate) fn clean_passphrase(typed: &str) -> Result<Secret> {
    let p: String = typed.nfc().collect();
    if p.trim().is_empty() {
        return Err(Error::Invalid("the passphrase is empty (a passphrase of only spaces counts as empty)".to_string()));
    }
    if p.contains('\0') {
        return Err(Error::Invalid("a passphrase cannot contain a NUL character".to_string()));
    }
    if p.chars().count() > MAX_PASSPHRASE_CHARS {
        return Err(Error::Invalid(format!("a passphrase can be at most {MAX_PASSPHRASE_CHARS} characters")));
    }
    // The encryption reads x'...' as a raw key given in hexadecimal, not as a passphrase.
    if p.starts_with("x'") || p.starts_with("X'") {
        return Err(Error::Invalid("a passphrase cannot begin with x' (it would be read as a raw key); begin it another way".to_string()));
    }
    Ok(Secret::new(p))
}

/// The passphrase chosen for a new encryption: the checks above, and long enough.
pub(crate) fn clean_new_passphrase(typed: &str) -> Result<Secret> {
    let s = clean_passphrase(typed)?;
    if s.as_str().chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(Error::Invalid(format!(
            "the passphrase is too short: it needs at least {MIN_PASSPHRASE_CHARS} characters, and twelve or more (or four unrelated words) is much safer"
        )));
    }
    Ok(s)
}

/// Gives a connection its key. Nothing is read yet: a wrong key is found out by [`check_open`].
pub(crate) fn apply_key(conn: &Connection, key: &Secret) -> Result<()> {
    conn.pragma_update(None, "cipher", "sqlcipher")?;
    conn.pragma_update(None, "legacy", 4)?;
    conn.pragma_update(None, "key", key.as_str())?;
    // Sorting and indexing may spill to a temporary file, and a temporary file is not encrypted: keep them in memory.
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    Ok(())
}

/// Changes a database file from one key to another in place: from none to a key (it becomes encrypted), from a key to none (it becomes
/// a plain file), or from one key to another. The file is a copy made for the purpose, never the project itself, so a failure here
/// leaves the project as it was.
pub(crate) fn rekey_file(path: &Path, old: Option<&Secret>, new: Option<&Secret>) -> Result<()> {
    let conn = crate::project::open_conn(path, rusqlite::OpenFlags::default(), old)?;
    if old.is_none() && new.is_some() {
        // A plain file is given the scheme before it is given a key.
        conn.pragma_update(None, "cipher", "sqlcipher")?;
        conn.pragma_update(None, "legacy", 4)?;
    }
    // Pages are rewritten in one transaction, which is simplest without the write-ahead file.
    let _: String = conn.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
    match new {
        Some(k) => conn.pragma_update(None, "rekey", k.as_str())?,
        None => conn.pragma_update(None, "rekey", "")?,
    }
    // The file is left whole, with no write-ahead file beside it, so that renaming it moves everything it holds. The project puts it
    // back in write-ahead mode when it is opened.
    Ok(())
}

/// Reads one page, which only works with the right key.
pub(crate) fn check_open(conn: &Connection) -> Result<()> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).map(|_| ()).map_err(|_| Error::WrongPassphrase)
}

/// True when the file does not begin like a plain SQLite file and is long enough to be a page, so it is encrypted (or is not a
/// project at all, which the passphrase then cannot open).
pub fn file_looks_encrypted(path: &Path) -> Result<bool> {
    let mut f = File::open(path)?;
    let mut head = [0u8; 16];
    let n = f.read(&mut head)?;
    if n == 16 && &head == SQLITE_HEADER {
        return Ok(false);
    }
    Ok(f.metadata()?.len() >= 512)
}

/// Overwrites a file with zeros and deletes it. Returns whether the overwrite went through; the file is deleted either way.
pub(crate) fn secure_erase(path: &Path) -> bool {
    let overwritten = (|| -> std::io::Result<()> {
        let len = fs::metadata(path)?.len();
        let mut f = FileOptions::new().write(true).open(path)?;
        f.seek(SeekFrom::Start(0))?;
        let zeros = vec![0u8; 1 << 20];
        let mut left = len;
        while left > 0 {
            let n = left.min(zeros.len() as u64) as usize;
            f.write_all(&zeros[..n])?;
            left -= n as u64;
        }
        f.sync_all()
    })()
    .is_ok();
    let _ = fs::remove_file(path);
    overwritten
}

fn erase_with_companions(path: &Path) -> bool {
    let mut all = secure_erase(path);
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let p = PathBuf::from(name);
        if p.exists() {
            all &= secure_erase(&p);
        }
    }
    all
}

/// What turning encryption on or off, or changing the passphrase, did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EncryptionOutcome {
    /// True when the project is encrypted now.
    pub encrypted: bool,
    /// Whether the old file was overwritten with zeros before it was deleted.
    pub old_file_erased: bool,
    /// Backups made before this change that do not open with the project's passphrase now (plain ones after turning encryption on,
    /// ones under the old passphrase after a change). They still hold what the project held.
    pub stale_backups: usize,
    pub backup_folder: String,
    /// What the encryption is.
    pub cipher: String,
}

/// A backup that the project's passphrase does not protect.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StaleBackup {
    pub path: String,
    /// `plain` (anyone can read it) or `other_passphrase` (it opens with an earlier passphrase).
    pub why: String,
    pub bytes: u64,
}

impl Project {
    /// True when this project's file is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.key.is_some()
    }

    /// Turns encryption on. The passphrase is asked for twice by the caller and given here once; it must be at least eight characters.
    pub fn encrypt(&mut self, passphrase: &str) -> Result<EncryptionOutcome> {
        self.ensure_writable()?;
        if self.key.is_some() {
            return Err(Error::Invalid("this project is already encrypted; to use another passphrase, change it".to_string()));
        }
        let key = clean_new_passphrase(passphrase)?;
        self.convert(Some(key), "project.encrypt")
    }

    /// Turns encryption off. The passphrase has to be typed again.
    pub fn decrypt(&mut self, passphrase: &str) -> Result<EncryptionOutcome> {
        self.ensure_writable()?;
        self.confirm_passphrase(passphrase)?;
        self.convert(None, "project.decrypt")
    }

    /// Changes the passphrase. The old one has to be typed again. Earlier backups stay under the old passphrase.
    pub fn change_passphrase(&mut self, old: &str, new: &str) -> Result<EncryptionOutcome> {
        self.ensure_writable()?;
        self.confirm_passphrase(old)?;
        let key = clean_new_passphrase(new)?;
        if key.as_str() == self.key.as_ref().map(Secret::as_str).unwrap_or("") {
            return Err(Error::Invalid("the new passphrase is the same as the old one".to_string()));
        }
        self.convert(Some(key), "project.passphrase")
    }

    fn confirm_passphrase(&self, typed: &str) -> Result<()> {
        let Some(current) = &self.key else {
            return Err(Error::Invalid("this project is not encrypted".to_string()));
        };
        let typed = clean_passphrase(typed)?;
        if typed.as_str() != current.as_str() {
            return Err(Error::WrongPassphrase);
        }
        Ok(())
    }

    /// Writes the project as a new file (encrypted with `new_key`, or plain), checks it, swaps it for the old file, and erases the old one.
    fn convert(&mut self, new_key: Option<Secret>, action: &str) -> Result<EncryptionOutcome> {
        let path = self.path.clone();
        let temp = sibling(&path, "converting");
        let replaced = sibling(&path, "replaced");
        for stale in [&temp, &replaced] {
            erase_with_companions(stale);
        }

        // The new file: a copy of the project made with SQLite's online copy (so it is consistent however the project is in use; the copy
        // has the same key as the project, because SQLite copies only between databases in the same state), which is then changed to
        // the new key. The project itself is not touched.
        let made = crate::project::backup_to(&self.conn, &temp, self.key.as_ref()).and_then(|()| rekey_file(&temp, self.key.as_ref(), new_key.as_ref()));
        if let Err(e) = made {
            erase_with_companions(&temp);
            return Err(e);
        }
        let check = (|| -> Result<()> {
            let plain = !file_looks_encrypted(&temp)?;
            if plain != new_key.is_none() {
                return Err(Error::Invalid("the new file is not in the form it should be in, so nothing was changed".to_string()));
            }
            let conn = crate::project::open_conn(&temp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY, new_key.as_ref())?;
            let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            if ok != "ok" {
                return Err(Error::Invalid(format!("the new file did not pass the check ({ok}), so nothing was changed")));
            }
            for table in ["event", "source", "coding", "code", "memo"] {
                let there: i64 = self.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
                let here: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
                if there != here {
                    return Err(Error::Invalid(format!("the new file has {here} rows of {table} where the project has {there}, so nothing was changed")));
                }
            }
            let (a, b): (i64, i64) = (
                self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?,
                conn.pragma_query_value(None, "user_version", |r| r.get(0))?,
            );
            if a != b {
                return Err(Error::Invalid("the new file has another format version, so nothing was changed".to_string()));
            }
            Ok(())
        })();
        if let Err(e) = check {
            erase_with_companions(&temp);
            return Err(e);
        }

        // Swap: let go of the old file, put the new one in its place, and open it. If anything fails, the old file goes back.
        let placeholder = Connection::open_in_memory()?;
        let old = std::mem::replace(&mut self.conn, placeholder);
        drop(old);
        let old_key = self.key.clone();
        let swapped = fs::rename(&path, &replaced).and_then(|()| fs::rename(&temp, &path));
        let reopened = match swapped {
            Ok(()) => crate::project::open_conn(&path, rusqlite::OpenFlags::default(), new_key.as_ref()).and_then(|c| {
                c.pragma_update(None, "foreign_keys", true)?;
                enable_wal(&c)?;
                Ok(c)
            }),
            Err(e) => Err(Error::Io(e)),
        };
        let conn = match reopened {
            Ok(c) => c,
            Err(e) => {
                // Put the old file back.
                if replaced.exists() {
                    let _ = fs::remove_file(&path);
                    let _ = fs::rename(&replaced, &path);
                }
                erase_with_companions(&temp);
                if let Ok(c) = crate::project::open_conn(&path, rusqlite::OpenFlags::default(), old_key.as_ref()) {
                    let _ = c.pragma_update(None, "foreign_keys", true);
                    self.conn = c;
                }
                return Err(e);
            }
        };
        self.conn = conn;
        self.key = new_key;

        let after = json!({ "cipher": if self.key.is_some() { CIPHER_DESCRIPTION } else { "none" } });
        event::insert(&self.conn, &self.actor, &self.session, action, Some(("project", 1)), None, Some(&after))?;

        let old_file_erased = erase_with_companions(&replaced);
        let stale_backups = self.stale_backups()?.len();
        Ok(EncryptionOutcome {
            encrypted: self.key.is_some(),
            old_file_erased,
            stale_backups,
            backup_folder: self.backup_folder()?.display().to_string(),
            cipher: if self.key.is_some() { CIPHER_DESCRIPTION.to_string() } else { String::new() },
        })
    }

    /// The backups (and the copies kept when the project was brought to a newer format) that the project's passphrase does not
    /// protect. A project that is not encrypted has none: there is nothing to protect.
    pub fn stale_backups(&self) -> Result<Vec<StaleBackup>> {
        let Some(key) = &self.key else { return Ok(Vec::new()) };
        let ext = file_ext(&self.path);
        let mut found = Vec::new();
        let mut candidates: Vec<PathBuf> = Vec::new();
        let folder = self.backup_folder()?;
        if let Ok(rd) = fs::read_dir(&folder) {
            candidates.extend(rd.filter_map(|e| e.ok().map(|e| e.path())));
        }
        let stem = file_stem(&self.path);
        if let Some(dir) = self.path.parent() {
            if let Ok(rd) = fs::read_dir(dir) {
                candidates.extend(rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| {
                    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&format!("{stem}.before-format-")) && n.ends_with(&format!(".{ext}")))
                }));
            }
        }
        for p in candidates {
            if !p.is_file() || !p.extension().is_some_and(|x| x.eq_ignore_ascii_case(PROJECT_EXTENSION)) {
                continue;
            }
            let bytes = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let why = if !file_looks_encrypted(&p).unwrap_or(false) {
                // A plain SQLite file; one that is not even that (a stray file) is left alone.
                let plain = File::open(&p).ok().and_then(|mut f| {
                    let mut h = [0u8; 16];
                    f.read_exact(&mut h).ok().map(|()| &h == SQLITE_HEADER)
                });
                if plain == Some(true) {
                    Some("plain")
                } else {
                    None
                }
            } else if crate::project::open_conn(&p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY, Some(key)).is_err() {
                Some("other_passphrase")
            } else {
                None
            };
            if let Some(why) = why {
                found.push(StaleBackup { path: p.display().to_string(), why: why.to_string(), bytes });
            }
        }
        found.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(found)
    }

    /// Overwrites with zeros and deletes the backups [`stale_backups`](Project::stale_backups) lists. Returns how many were removed.
    pub fn delete_stale_backups(&mut self) -> Result<usize> {
        self.ensure_writable()?;
        let stale = self.stale_backups()?;
        let mut n = 0;
        for s in &stale {
            erase_with_companions(Path::new(&s.path));
            n += 1;
        }
        if n > 0 {
            self.conn.execute(
                "INSERT INTO event (ts, actor, session, action, target_type, target_id, before_json, after_json)
                 VALUES (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?1, ?2, 'project.backups_deleted', 'project', 1, NULL, ?3)",
                params![self.actor, self.session, json!({ "backups": n }).to_string()],
            )?;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_passphrase_is_put_in_nfc_and_refused_when_it_would_be_misread() {
        assert_eq!(clean_passphrase("cafe\u{301} au lait").unwrap().as_str(), "caf\u{e9} au lait");
        assert!(clean_passphrase("").is_err());
        assert!(clean_passphrase("a\0b").is_err());
        assert!(clean_passphrase("x'0011'").is_err());
        assert!(clean_passphrase("X'ab'").is_err());
        assert!(clean_passphrase("it's fine").is_ok(), "an apostrophe inside is fine");
    }

    #[test]
    fn a_new_passphrase_needs_eight_characters() {
        assert!(clean_new_passphrase("short").is_err());
        assert!(clean_new_passphrase("eight ch").is_ok());
        assert!(clean_new_passphrase("\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}").is_ok(), "characters, not bytes, are counted");
    }

    #[test]
    fn a_secret_does_not_show_itself() {
        assert_eq!(format!("{:?}", Secret::new("hunter22".into())), "Secret(..)");
    }
}
