//! Encrypted projects: a file nobody can read without the passphrase, turning encryption on and off, changing the passphrase, and what
//! happens to backups, merges, and older files.

use std::fs;
use std::path::Path;

use prismqda_engine::{file_looks_encrypted, Error, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SourceType};
use rusqlite::Connection;
use tempfile::TempDir;

const PASS: &str = "correct horse battery staple";

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn with(pass: &str) -> OpenOptions {
    opts().with_passphrase(pass)
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

/// Every file in the folder, as text, to look for a name in.
fn holds(dir: &Path, needle: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if fs::read(&p).unwrap().windows(needle.len()).any(|w| w == needle) {
                found.push(p.display().to_string());
            }
        }
    }
    found
}

fn fill(p: &mut Project) -> (i64, i64) {
    let source = p.add_source(&NewSource::new("Interview with Sarah Jones", SourceType::Document, "Sarah Jones said the bus was late.")).unwrap();
    let code = p.add_code(&NewCode::new("Lateness")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_coding(&NewCoding::new(source, 0, 11, code, coder)).unwrap();
    (source, code)
}

#[test]
fn a_new_encrypted_project_cannot_be_read_without_its_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("Secret"), &with(PASS)).unwrap();
    let (source, _) = fill(&mut p);
    assert!(p.is_encrypted() && p.info().unwrap().encrypted);
    drop(p);

    assert!(file_looks_encrypted(&path).unwrap());
    assert_eq!(holds(dir.path(), b"Sarah"), Vec::<String>::new(), "no file holds the name: not the project, and not the write-ahead file");
    assert_eq!(holds(dir.path(), b"SQLite format 3"), Vec::<String>::new(), "it does not even look like a SQLite file");

    assert!(matches!(Project::open(&path, &opts()), Err(Error::NeedsPassphrase(_))), "no passphrase");
    assert!(matches!(Project::open(&path, &with("not the right one")), Err(Error::WrongPassphrase)), "a wrong passphrase");

    let p = Project::open(&path, &with(PASS)).unwrap();
    assert!(p.info().unwrap().encrypted);
    assert_eq!(p.source_text(source).unwrap(), "Sarah Jones said the bus was late.");
    assert_eq!(p.list_codings(source).unwrap().len(), 1);
}

#[test]
fn a_short_passphrase_or_one_that_would_be_misread_is_refused_and_leaves_no_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    for bad in ["short", "x'00112233445566778899'", "        "] {
        let e = Project::create(&path, &NewProject::new("P"), &with(bad));
        assert!(e.is_err(), "{bad}");
        assert!(!path.exists(), "nothing is left behind for {bad}");
    }
}

#[test]
fn a_passphrase_with_quotes_and_accents_opens_however_the_accent_was_typed() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let hard = "it's \"quoted\" caf\u{e9} \u{1F600}";
    drop(Project::create(&path, &NewProject::new("P"), &with(hard)).unwrap());
    assert!(Project::open(&path, &with(hard)).is_ok());
    assert!(Project::open(&path, &with("it's \"quoted\" cafe\u{301} \u{1F600}")).is_ok(), "the accent typed as a letter and a mark");
    assert!(Project::open(&path, &with("it's \"quoted\" cafe \u{1F600}")).is_err());
}

#[test]
fn turning_encryption_on_keeps_everything_replaces_the_plain_file_and_says_where_plain_backups_are() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("Plain at first"), &opts()).unwrap();
    let (source, code) = fill(&mut p);
    let plain_backup = p.backup_now().unwrap();
    assert!(holds(dir.path(), b"Sarah").len() >= 2, "the project and its backup are readable now");
    let events_before = p.events(10_000).unwrap().len();

    assert!(p.encrypt("short").is_err(), "too short");
    assert!(!p.is_encrypted());
    let out = p.encrypt(PASS).unwrap();
    assert!(out.encrypted && out.old_file_erased);
    assert_eq!(out.stale_backups, 1, "the backup made before is plain");
    assert!(p.is_encrypted());
    assert!(p.encrypt(PASS).is_err(), "already encrypted");

    // The project works as it did, in the new file.
    assert_eq!(p.source_text(source).unwrap(), "Sarah Jones said the bus was late.");
    assert_eq!(p.list_codings(source).unwrap().len(), 1);
    p.add_code(&NewCode::new("Another")).unwrap();
    let events = p.events(10_000).unwrap();
    assert_eq!(events.len(), events_before + 2, "the log has the encryption and the new code");
    let enc = events.iter().find(|e| e.action == "project.encrypt").unwrap();
    assert!(enc.after.as_ref().unwrap()["cipher"].as_str().unwrap().contains("AES-256"));
    assert!(!serde_json::to_string(&events).unwrap().contains(PASS), "the passphrase is never logged");

    // No half-made file is left, and the readable copy of the project is gone.
    let left = names(dir.path());
    assert!(!left.iter().any(|n| n.contains("converting") || n.contains("replaced")), "{left:?}");
    let listed = holds(dir.path(), b"Sarah");
    assert_eq!(listed.len(), 1, "only the plain backup still holds the name: {listed:?}");
    assert!(listed[0].contains(".backups"));

    // The backups the passphrase does not protect are named, and can be deleted (and are overwritten first).
    let stale = p.stale_backups().unwrap();
    assert_eq!((stale.len(), stale[0].why.as_str()), (1, "plain"));
    assert_eq!(p.delete_stale_backups().unwrap(), 1);
    assert!(!plain_backup.exists());
    assert!(p.stale_backups().unwrap().is_empty());
    assert!(holds(dir.path(), b"Sarah").is_empty());
    drop(p);

    let again = Project::open(&path, &with(PASS)).unwrap();
    let mut code_names: Vec<String> = again.list_codes().unwrap().iter().map(|c| c.name.clone()).collect();
    code_names.sort();
    assert_eq!(code_names, [String::from("Another"), String::from("Lateness")]);
    let _ = code;
}

#[test]
fn a_backup_of_an_encrypted_project_is_encrypted_with_the_same_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("P"), &with(PASS)).unwrap();
    let (source, _) = fill(&mut p);
    let backup = p.backup_now().unwrap();
    assert!(file_looks_encrypted(&backup).unwrap());
    assert!(holds(dir.path(), b"Sarah").is_empty(), "the backup holds no name either");
    assert!(matches!(Project::open(&backup, &opts()), Err(Error::NeedsPassphrase(_))));
    let b = Project::open(&backup, &with(PASS)).unwrap();
    assert_eq!(b.source_text(source).unwrap(), "Sarah Jones said the bus was late.");
    assert!(p.stale_backups().unwrap().is_empty(), "a backup under the same passphrase is not stale");
}

#[test]
fn opening_an_encrypted_project_makes_an_encrypted_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("P"), &with(PASS)).unwrap();
    fill(&mut p);
    drop(p);
    let mut o = with(PASS);
    o.backup_on_open = true;
    let _p = Project::open(&path, &o).unwrap();
    assert!(holds(dir.path(), b"Sarah").is_empty());
    let backups: Vec<_> = fs::read_dir(dir.path().join("p.backups")).unwrap().collect();
    assert_eq!(backups.len(), 1);
}

#[test]
fn turning_encryption_off_needs_the_passphrase_again_and_gives_back_an_ordinary_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("P"), &with(PASS)).unwrap();
    let (source, _) = fill(&mut p);
    assert!(matches!(p.decrypt("wrong wrong wrong"), Err(Error::WrongPassphrase)));
    assert!(p.is_encrypted());
    let out = p.decrypt(PASS).unwrap();
    assert!(!out.encrypted && out.old_file_erased);
    assert!(!p.is_encrypted() && !p.info().unwrap().encrypted);
    assert_eq!(p.source_text(source).unwrap(), "Sarah Jones said the bus was late.");
    assert!(p.decrypt(PASS).is_err(), "it is not encrypted any more");
    drop(p);
    assert!(!file_looks_encrypted(&path).unwrap());
    let c = Connection::open(&path).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM source", [], |r| r.get::<_, i64>(0)).unwrap(), 1, "any SQLite program reads it");
    assert!(Project::open(&path, &opts()).is_ok());
}

#[test]
fn changing_the_passphrase_leaves_earlier_backups_under_the_old_one_and_says_so() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("P"), &with(PASS)).unwrap();
    fill(&mut p);
    let old_backup = p.backup_now().unwrap();
    assert!(matches!(p.change_passphrase("wrong wrong wrong", "a brand new passphrase"), Err(Error::WrongPassphrase)));
    assert!(p.change_passphrase(PASS, "short").is_err());
    assert!(p.change_passphrase(PASS, PASS).is_err(), "the same one");
    let out = p.change_passphrase(PASS, "a brand new passphrase").unwrap();
    assert!(out.encrypted && out.old_file_erased);
    assert_eq!(out.stale_backups, 1, "the backup is under the old passphrase");
    assert_eq!(p.stale_backups().unwrap()[0].why, "other_passphrase");
    // New backups use the new passphrase.
    let new_backup = p.backup_now().unwrap();
    assert!(Project::open(&new_backup, &with("a brand new passphrase")).is_ok());
    assert!(Project::open(&new_backup, &with(PASS)).is_err());
    drop(p);

    assert!(matches!(Project::open(&path, &with(PASS)), Err(Error::WrongPassphrase)), "the old passphrase no longer opens it");
    assert!(Project::open(&path, &with("a brand new passphrase")).is_ok());
    assert!(Project::open(&old_backup, &with(PASS)).is_ok(), "the earlier backup still opens with the old one");
}

#[test]
fn a_change_that_is_not_allowed_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("P"), &opts()).unwrap();
    fill(&mut p);
    assert!(p.decrypt(PASS).is_err(), "not encrypted");
    assert!(p.change_passphrase(PASS, "another passphrase").is_err(), "not encrypted");
    assert!(p.stale_backups().unwrap().is_empty());
    assert_eq!(names(dir.path()), ["p.pqda", "p.pqda-shm", "p.pqda-wal"].iter().filter(|n| dir.path().join(n).exists()).map(|n| n.to_string()).collect::<Vec<_>>());
}

#[test]
fn undo_walks_past_turning_encryption_on() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("P"), &opts()).unwrap();
    let (source, _) = fill(&mut p);
    p.encrypt(PASS).unwrap();
    let undone = p.undo().unwrap().expect("the coding is still undoable");
    assert_eq!(undone.action, "coding.add");
    assert_eq!(p.list_codings(source).unwrap().len(), 0);
}

#[test]
fn an_older_file_in_an_encrypted_project_is_brought_forward_and_its_kept_copy_is_encrypted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.pqda");
    let mut p = Project::create(&path, &NewProject::new("Old"), &with(PASS)).unwrap();
    fill(&mut p);
    drop(p);
    // Make it a version 12 file, inside the encrypted file.
    let c = Connection::open(&path).unwrap();
    c.pragma_update(None, "cipher", "sqlcipher").unwrap();
    c.pragma_update(None, "legacy", 4).unwrap();
    c.pragma_update(None, "key", PASS).unwrap();
    c.execute_batch("DROP TABLE id_floor; DROP TABLE region_decision; DROP TABLE deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.pragma_update(None, "user_version", 12).unwrap();
    drop(c);

    let p = Project::open(&path, &with(PASS)).unwrap();
    assert!(p.notices()[0].contains("upgraded from format 12"), "{:?}", p.notices());
    assert!(p.is_encrypted());
    let kept = dir.path().join("old.before-format-12.pqda");
    assert!(kept.is_file());
    assert!(file_looks_encrypted(&kept).unwrap(), "the copy kept before the upgrade is encrypted too");
    assert!(Project::open(&kept, &with(PASS)).is_ok());
    assert!(holds(dir.path(), b"Sarah").is_empty());
}

#[test]
fn a_merge_reads_an_encrypted_copy_with_its_passphrase_or_the_projects_own() {
    let dir = TempDir::new().unwrap();
    let ours = dir.path().join("ours.pqda");
    let mut p = Project::create(&ours, &NewProject::new("Shared"), &opts()).unwrap();
    fill(&mut p);
    // Their copy: the same project, worked in further, and encrypted with a passphrase of their own.
    let copy = p.backup_now().unwrap();
    let theirs = dir.path().join("theirs.pqda");
    fs::copy(&copy, &theirs).unwrap();
    let mut t = Project::open(&theirs, &opts()).unwrap();
    let other = t.add_source(&NewSource::new("Their interview", SourceType::Document, "Another voice.")).unwrap();
    t.encrypt("their own passphrase").unwrap();
    drop(t);
    let _ = other;

    assert!(matches!(p.merge_preview(&theirs), Err(Error::NeedsPassphrase(_))), "our project is plain, so there is no passphrase to try");
    assert!(matches!(p.merge_preview_with(&theirs, Some("not their passphrase")), Err(Error::WrongPassphrase)));
    let report = p.merge_preview_with(&theirs, Some("their own passphrase")).unwrap();
    assert_eq!(report.documents_added.len(), 1);
    assert!(!report.applied);
    let report = p.merge_copy_with(&theirs, Some("their own passphrase")).unwrap();
    assert!(report.applied);
    assert_eq!(p.list_sources().unwrap().len(), 2);
    let holding = holds(dir.path(), b"Another voice");
    assert!(!holding.is_empty() && holding.iter().all(|f| f.contains("ours.pqda")), "only our own (plain) project holds what came in: {holding:?}");

    // When both are encrypted with the same passphrase, nothing has to be typed for the copy.
    let mut q = Project::create(dir.path().join("q.pqda"), &NewProject::new("Q"), &with(PASS)).unwrap();
    let (s, _) = fill(&mut q);
    let qcopy = q.backup_now().unwrap();
    let mut q2 = Project::open(&qcopy, &with(PASS)).unwrap();
    q2.add_source(&NewSource::new("More", SourceType::Document, "More words.")).unwrap();
    drop(q2);
    let report = q.merge_preview(&qcopy).unwrap();
    assert_eq!(report.documents_added.len(), 1);
    assert_eq!(q.source_text(s).unwrap(), "Sarah Jones said the bus was late.");
    // The scratch copies made for the preview are encrypted, and are gone.
    let scratch: Vec<_> = fs::read_dir(std::env::temp_dir()).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with(&format!("prismqda-preview-{}", std::process::id()))).collect();
    assert!(scratch.is_empty(), "no scratch copy is left in the temporary folder");
}

#[test]
fn a_newer_encrypted_file_opens_read_only_with_its_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("new.pqda");
    drop(Project::create(&path, &NewProject::new("New"), &with(PASS)).unwrap());
    let c = Connection::open(&path).unwrap();
    c.pragma_update(None, "cipher", "sqlcipher").unwrap();
    c.pragma_update(None, "legacy", 4).unwrap();
    c.pragma_update(None, "key", PASS).unwrap();
    c.pragma_update(None, "user_version", prismqda_engine::schema::SCHEMA_VERSION + 1).unwrap();
    drop(c);
    assert!(matches!(Project::open(&path, &opts()), Err(Error::NeedsPassphrase(_))));
    let mut p = Project::open(&path, &with(PASS)).unwrap();
    assert!(p.is_read_only());
    assert!(p.change_passphrase(PASS, "another passphrase").is_err(), "a project that is read only cannot be changed");
}

#[test]
fn a_stray_file_is_not_a_project_and_a_plain_project_ignores_a_passphrase() {
    let dir = TempDir::new().unwrap();
    let stray = dir.path().join("notes.pqda");
    fs::write(&stray, b"just some words").unwrap();
    assert!(matches!(Project::open(&stray, &with(PASS)), Err(Error::NotAProject(_))));
    let big = dir.path().join("big.pqda");
    fs::write(&big, vec![7u8; 8192]).unwrap();
    assert!(matches!(Project::open(&big, &with(PASS)), Err(Error::WrongPassphrase)), "a file that could be encrypted says the passphrase is wrong");

    let plain = dir.path().join("plain.pqda");
    drop(Project::create(&plain, &NewProject::new("Plain"), &opts()).unwrap());
    let p = Project::open(&plain, &with(PASS)).unwrap();
    assert!(!p.is_encrypted(), "a passphrase given for a project that is not encrypted changes nothing");
}

#[test]
fn a_backup_of_an_encrypted_project_is_checked_and_opens_with_the_passphrase_and_holds_the_work() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut p = Project::create(&path, &NewProject::new("Sealed"), &with(PASS)).unwrap();
    let (source, _code) = fill(&mut p);
    let backup = p.backup_now().unwrap();
    assert!(backup.is_file(), "the backup is there after it passed its check");
    assert!(names(&dir.path().join("p.backups")).iter().all(|n| !n.ends_with("-wal") && !n.ends_with("-shm")), "the check left no half-made file beside it");
    assert!(fs::read(&backup).unwrap().windows(5).all(|w| w != b"Sarah"), "and what it holds cannot be read without the passphrase");
    assert!(Project::open(&backup, &opts()).is_err(), "it is not opened without the passphrase");
    let again = Project::open(&backup, &with(PASS)).unwrap();
    assert_eq!(again.source_text(source).unwrap(), "Sarah Jones said the bus was late.");
    assert_eq!(again.list_codings(source).unwrap().len(), 1);
}
