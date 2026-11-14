//! Engine tests through the public API, plus raw SQLite access where a test needs to look inside
//! the file the way R or Python would.

use std::fs;

use prismqda_engine::{
    schema, sha256_hex, CoderRole, CodingMode, Error, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions,
    Origin, Project, SourceType, UnitKind,
};
use rusqlite::{Connection, OpenFlags};
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn new_project(dir: &TempDir) -> Project {
    Project::create(dir.path().join("study.pqda"), &NewProject::new("Study"), &opts()).unwrap()
}

/// A project with one source ("Hello, world!"), one code, and one human coder.
fn seeded(dir: &TempDir) -> (Project, i64, i64, i64) {
    let mut p = new_project(dir);
    let source = p.add_source(&NewSource::new("Doc", SourceType::Document, "Hello, world!")).unwrap();
    let code = p.add_code(&NewCode::new("Greeting")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    (p, source, code, coder)
}

fn raw(path: &std::path::Path) -> Connection {
    Connection::open(path).unwrap()
}

// ---- the project file ------------------------------------------------------------------------

#[test]
fn a_new_project_is_a_marked_wal_database_with_its_first_event() {
    let dir = TempDir::new().unwrap();
    let p = new_project(&dir);
    let c = raw(p.path());
    let app_id: i32 = c.pragma_query_value(None, "application_id", |r| r.get(0)).unwrap();
    let version: i32 = c.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    let mode: String = c.pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
    assert_eq!(app_id, schema::APPLICATION_ID);
    assert_eq!(version, schema::SCHEMA_VERSION);
    assert_eq!(mode.to_lowercase(), "wal");

    let events = p.events(10).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "project.create");
    assert_eq!(events[0].actor, "tester");
}

#[test]
fn create_refuses_to_overwrite_and_leaves_the_existing_file_alone() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("study.pqda");
    fs::write(&path, b"precious").unwrap();
    let err = Project::create(&path, &NewProject::new("Study"), &opts()).err().unwrap();
    assert!(matches!(err, Error::AlreadyExists(_)));
    assert_eq!(fs::read(&path).unwrap(), b"precious");
}

#[test]
fn create_needs_a_name() {
    let dir = TempDir::new().unwrap();
    let err = Project::create(dir.path().join("x.pqda"), &NewProject::new("   "), &opts()).err().unwrap();
    assert!(matches!(err, Error::Invalid(_)));
    assert!(!dir.path().join("x.pqda").exists());
}

#[test]
fn reopening_keeps_everything() {
    let dir = TempDir::new().unwrap();
    let path = {
        let (mut p, source, code, coder) = seeded(&dir);
        p.apply_coding(&NewCoding::new(source, 0, 5, code, coder)).unwrap();
        p.path().to_path_buf()
    };
    let p = Project::open(&path, &opts()).unwrap();
    let info = p.info().unwrap();
    assert_eq!((info.sources, info.codes, info.coders, info.codings), (1, 1, 1, 1));
    assert_eq!(info.name, "Study");
    assert!(!info.read_only);
    assert_eq!(info.schema_version, schema::SCHEMA_VERSION);
}

#[test]
fn project_uid_is_unique_per_project_and_stable_across_opens() {
    let dir = TempDir::new().unwrap();
    let a = Project::create(dir.path().join("a.pqda"), &NewProject::new("A"), &opts()).unwrap();
    let b = Project::create(dir.path().join("b.pqda"), &NewProject::new("B"), &opts()).unwrap();
    let uid_a = a.info().unwrap().project_uid;
    assert_eq!(uid_a.len(), 32);
    assert_ne!(uid_a, b.info().unwrap().project_uid);
    let path = a.path().to_path_buf();
    drop(a);
    assert_eq!(Project::open(&path, &opts()).unwrap().info().unwrap().project_uid, uid_a);
}

#[test]
fn open_rejects_files_that_are_not_projects() {
    let dir = TempDir::new().unwrap();

    let garbage = dir.path().join("garbage.pqda");
    fs::write(&garbage, b"this is not a database at all, just some text that is long enough").unwrap();
    assert!(matches!(Project::open(&garbage, &opts()).err().unwrap(), Error::NotAProject(_)));

    let empty = dir.path().join("empty.pqda");
    fs::write(&empty, b"").unwrap();
    assert!(matches!(Project::open(&empty, &opts()).err().unwrap(), Error::NotAProject(_)));

    let other = dir.path().join("other.sqlite");
    {
        let c = raw(&other);
        c.execute_batch("CREATE TABLE t (a); INSERT INTO t VALUES (1);").unwrap();
    }
    assert!(matches!(Project::open(&other, &opts()).err().unwrap(), Error::NotAProject(_)));

    let missing = dir.path().join("missing.pqda");
    assert!(matches!(Project::open(&missing, &opts()).err().unwrap(), Error::Io(_)));
}

#[test]
fn a_newer_file_opens_read_only_with_a_notice_and_refuses_every_change() {
    let dir = TempDir::new().unwrap();
    let path = {
        let p = new_project(&dir);
        p.path().to_path_buf()
    };
    {
        let c = raw(&path);
        c.pragma_update(None, "user_version", schema::SCHEMA_VERSION + 1).unwrap();
    }
    let before = fs::read(&path).unwrap();

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.is_read_only());
    assert_eq!(p.notices().len(), 1);
    assert!(p.notices()[0].contains("read-only"));
    assert_eq!(p.info().unwrap().name, "Study");

    assert!(matches!(p.add_coder(&NewCoder::human("Ada")).err().unwrap(), Error::ReadOnly(_)));
    assert!(matches!(
        p.add_source(&NewSource::new("Doc", SourceType::Document, "text")).err().unwrap(),
        Error::ReadOnly(_)
    ));
    assert!(matches!(p.annotate("note").err().unwrap(), Error::ReadOnly(_)));
    assert!(matches!(p.backup_now().err().unwrap(), Error::ReadOnly(_)));

    drop(p);
    assert_eq!(fs::read(&path).unwrap(), before, "a read-only open must not change the file");
}

// ---- backups -----------------------------------------------------------------------------------

#[test]
fn backups_are_valid_projects_and_only_the_newest_are_kept() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    p.apply_coding(&NewCoding::new(source, 0, 5, code, coder)).unwrap();

    let mut made = Vec::new();
    for _ in 0..4 {
        made.push(p.backup_now().unwrap());
    }
    let backups_dir = dir.path().join("study.backups");
    let mut names: Vec<_> =
        fs::read_dir(&backups_dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(names.len(), 4, "keeps up to the default ten");

    // Keep only two: the two newest survive.
    let mut o = opts();
    o.keep_backups = 2;
    let path = p.path().to_path_buf();
    drop(p);
    let p = Project::open(&path, &o).unwrap();
    let newest = p.backup_now().unwrap();
    let mut names: Vec<_> =
        fs::read_dir(&backups_dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&newest.file_name().unwrap().to_string_lossy().to_string()));
    assert!(!made[0].exists() && !made[1].exists(), "the oldest were removed");

    // A backup opens as a project with the coding in it.
    let restored = Project::open(&newest, &opts()).unwrap().info().unwrap();
    assert_eq!(restored.codings, 1);
    assert_eq!(restored.name, "Study");
}

#[test]
fn opening_makes_a_backup_by_default() {
    let dir = TempDir::new().unwrap();
    let path = {
        let p = new_project(&dir);
        p.path().to_path_buf()
    };
    let _p = Project::open(&path, &OpenOptions::new("tester")).unwrap();
    let count = fs::read_dir(dir.path().join("study.backups")).unwrap().count();
    assert_eq!(count, 1);
}

// ---- the event log -----------------------------------------------------------------------------

#[test]
fn the_event_log_cannot_be_edited_or_deleted_even_with_raw_access() {
    let dir = TempDir::new().unwrap();
    let (mut p, ..) = seeded(&dir);
    p.annotate("Chose to split the code after the pilot").unwrap();

    let c = raw(p.path());
    let update = c.execute("UPDATE event SET actor = 'someone else'", []).unwrap_err();
    assert!(update.to_string().contains("append-only"), "{update}");
    let delete = c.execute("DELETE FROM event", []).unwrap_err();
    assert!(delete.to_string().contains("append-only"), "{delete}");
    // Nothing changed.
    let n: i64 = c.query_row("SELECT COUNT(*) FROM event WHERE actor = 'tester'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, p.events(1000).unwrap().len() as i64);
}

#[test]
fn every_change_logs_one_event_with_the_actor_and_session() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    p.apply_coding(&NewCoding::new(source, 7, 12, code, coder)).unwrap();
    let events = p.events(100).unwrap();
    let actions: Vec<&str> = events.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(actions, ["project.create", "source.import", "code.add", "coder.add", "coding.add"]);
    let session = &events[0].session;
    assert_eq!(session.len(), 16);
    assert!(events.iter().all(|e| &e.session == session && e.actor == "tester"));
    let coding = events.last().unwrap();
    assert_eq!(coding.target_type.as_deref(), Some("coding"));
    let after = coding.after.as_ref().unwrap();
    assert_eq!((after["start"].as_i64(), after["end"].as_i64()), (Some(7), Some(12)));
}

#[test]
fn a_new_session_gets_a_new_session_id() {
    let dir = TempDir::new().unwrap();
    let path = {
        let mut p = new_project(&dir);
        p.annotate("first").unwrap();
        p.path().to_path_buf()
    };
    let mut p = Project::open(&path, &opts()).unwrap();
    p.annotate("second").unwrap();
    let events = p.events(10).unwrap();
    let first = events.iter().find(|e| e.action == "project.create").unwrap();
    let second = events.iter().rfind(|e| e.action == "annotation").unwrap();
    assert_ne!(first.session, second.session);
}

#[test]
fn a_refused_change_leaves_no_event_and_no_row() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    let events_before = p.events(1000).unwrap().len();
    assert!(p.apply_coding(&NewCoding::new(source, 0, 500, code, coder)).is_err());
    assert_eq!(p.events(1000).unwrap().len(), events_before);
    assert_eq!(p.info().unwrap().codings, 0);
}

#[test]
fn removing_a_coding_is_logged_with_its_full_contents() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    let mut c = NewCoding::new(source, 0, 5, code, coder);
    c.rationale = "greeting word".to_string();
    let id = p.apply_coding(&c).unwrap();
    p.remove_coding(id).unwrap();
    assert_eq!(p.info().unwrap().codings, 0);
    let removed = p.events(100).unwrap().into_iter().rfind(|e| e.action == "coding.remove").unwrap();
    let before = removed.before.unwrap();
    assert_eq!(before["rationale"], "greeting word");
    assert_eq!((before["start"].as_i64(), before["end"].as_i64()), (Some(0), Some(5)));
    assert!(matches!(p.remove_coding(id).err().unwrap(), Error::NotFound { .. }));
}

#[test]
fn annotations_need_text() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    assert!(matches!(p.annotate("   ").err().unwrap(), Error::Invalid(_)));
}

// ---- sources and offsets ----------------------------------------------------------------------

#[test]
fn source_text_is_stored_in_nfc_and_offsets_count_scalar_values() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    // "e" followed by a combining acute accent, then "x": two scalar values once composed.
    let id = p.add_source(&NewSource::new("Accents", SourceType::Document, "e\u{0301}x")).unwrap();
    assert_eq!(p.source_text(id).unwrap(), "\u{00e9}x");
    assert_eq!(p.text_of_span(id, 0, 1).unwrap(), "\u{00e9}");
    assert!(p.text_of_span(id, 0, 3).is_err());

    // Characters outside the basic plane are one scalar value each, not two.
    let emoji = p.add_source(&NewSource::new("Emoji", SourceType::Document, "a\u{1F600}b")).unwrap();
    assert_eq!(p.text_of_span(emoji, 1, 2).unwrap(), "\u{1F600}");
    assert_eq!(p.text_of_span(emoji, 0, 3).unwrap(), "a\u{1F600}b");
    assert!(p.text_of_span(emoji, 0, 4).is_err());
}

#[test]
fn right_to_left_and_cjk_text_round_trip() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    let text = "\u{05e9}\u{05dc}\u{05d5}\u{05dd} \u{4f60}\u{597d}";
    let id = p.add_source(&NewSource::new("Mixed", SourceType::Document, text)).unwrap();
    assert_eq!(p.source_text(id).unwrap(), text);
    assert_eq!(p.text_of_span(id, 5, 7).unwrap(), "\u{4f60}\u{597d}");
}

#[test]
fn bad_sources_are_refused() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    assert!(p.add_source(&NewSource::new("", SourceType::Document, "text")).is_err());
    assert!(p.add_source(&NewSource::new("Empty", SourceType::Document, "")).is_err());
    assert!(p.add_source(&NewSource::new("Nul", SourceType::Document, "a\0b")).is_err());
    assert_eq!(p.info().unwrap().sources, 0);
}

#[test]
fn the_default_unit_follows_the_source_type() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    let cases = [
        (SourceType::Document, "sentence"),
        (SourceType::Transcript, "speaker_turn"),
        (SourceType::Survey, "survey_response"),
        (SourceType::Social, "comment"),
    ];
    for (kind, expected) in cases {
        let id = p.add_source(&NewSource::new("s", kind, "text")).unwrap();
        let got: String = raw(p.path())
            .query_row("SELECT default_unit_kind FROM source WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(got, expected);
    }
    let mut custom = NewSource::new("c", SourceType::Document, "text");
    custom.default_unit_kind = Some(UnitKind::Paragraph);
    let id = p.add_source(&custom).unwrap();
    let got: String =
        raw(p.path()).query_row("SELECT default_unit_kind FROM source WHERE id = ?1", [id], |r| r.get(0)).unwrap();
    assert_eq!(got, "paragraph");
}

#[test]
fn duplicate_imports_are_found_by_file_hash() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    let hash = sha256_hex(b"the file bytes");
    assert_eq!(hash.len(), 64);
    assert_eq!(p.find_source_by_hash(&hash).unwrap(), None);
    let mut s = NewSource::new("Interview 1", SourceType::Transcript, "text");
    s.original_file_hash = Some(hash.clone());
    let id = p.add_source(&s).unwrap();
    assert_eq!(p.find_source_by_hash(&hash).unwrap(), Some(id));
    assert_eq!(p.find_source_by_hash(&sha256_hex(b"other")).unwrap(), None);
}

#[test]
fn sha256_matches_a_known_value() {
    // SHA-256 of the empty input.
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

// ---- codes and coders -------------------------------------------------------------------------

#[test]
fn code_names_are_unique_among_active_siblings_ignoring_case() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    let parent = p.add_code(&NewCode::new("Barriers")).unwrap();
    let mut child = NewCode::new("Cost");
    child.parent_id = Some(parent);
    p.add_code(&child).unwrap();
    child.name = "cost".to_string();
    assert!(matches!(p.add_code(&child).err().unwrap(), Error::Invalid(_)));
    // The same name elsewhere in the tree is fine.
    p.add_code(&NewCode::new("Cost")).unwrap();
    // A missing parent is reported.
    let mut orphan = NewCode::new("Lost");
    orphan.parent_id = Some(999);
    assert!(matches!(p.add_code(&orphan).err().unwrap(), Error::NotFound { .. }));
}

#[test]
fn a_model_coder_must_name_its_model() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    let mut m = NewCoder::human("Assistant");
    m.role = CoderRole::Model;
    assert!(matches!(p.add_coder(&m).err().unwrap(), Error::Invalid(_)));
    m.provider = Some("local".to_string());
    m.model_name = Some("example-model".to_string());
    m.model_version = Some("1.0".to_string());
    p.add_coder(&m).unwrap();
}

#[test]
fn coder_ids_are_unique() {
    let dir = TempDir::new().unwrap();
    let mut p = new_project(&dir);
    let mut a = NewCoder::human("Ada");
    a.local_id = Some("ada-1".to_string());
    p.add_coder(&a).unwrap();
    assert!(matches!(p.add_coder(&a).err().unwrap(), Error::Invalid(_)));
    // Generated ids never collide.
    let x = p.add_coder(&NewCoder::human("Bo")).unwrap();
    let y = p.add_coder(&NewCoder::human("Bo")).unwrap();
    assert_ne!(x, y);
}

// ---- codings ----------------------------------------------------------------------------------

#[test]
fn codings_are_checked_against_the_text_the_code_and_the_coder() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir); // "Hello, world!" is 13 characters
    assert!(p.apply_coding(&NewCoding::new(source, 0, 13, code, coder)).is_ok());
    assert!(p.apply_coding(&NewCoding::new(source, 0, 14, code, coder)).is_err(), "past the end");
    assert!(p.apply_coding(&NewCoding::new(source, 5, 5, code, coder)).is_err(), "empty span");
    assert!(p.apply_coding(&NewCoding::new(source, 6, 2, code, coder)).is_err(), "backwards span");
    assert!(p.apply_coding(&NewCoding::new(source, -1, 2, code, coder)).is_err(), "negative start");
    assert!(matches!(p.apply_coding(&NewCoding::new(99, 0, 2, code, coder)).err().unwrap(), Error::NotFound { .. }));
    assert!(matches!(p.apply_coding(&NewCoding::new(source, 0, 2, 99, coder)).err().unwrap(), Error::NotFound { .. }));
    assert!(matches!(p.apply_coding(&NewCoding::new(source, 0, 2, code, 99)).err().unwrap(), Error::NotFound { .. }));
    assert_eq!(p.info().unwrap().codings, 1);
}

#[test]
fn a_retired_code_cannot_be_applied() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    raw(p.path()).execute("UPDATE code SET status = 'retired' WHERE id = ?1", [code]).unwrap();
    let err = p.apply_coding(&NewCoding::new(source, 0, 2, code, coder)).err().unwrap();
    assert!(err.to_string().contains("retired"), "{err}");
}

#[test]
fn ai_origin_is_kept_on_the_coding() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    let mut c = NewCoding::new(source, 0, 5, code, coder);
    c.origin = Origin::AiSuggestionAccepted;
    let id = p.apply_coding(&c).unwrap();
    let origin: String =
        raw(p.path()).query_row("SELECT origin FROM coding WHERE id = ?1", [id], |r| r.get(0)).unwrap();
    assert_eq!(origin, "ai_suggestion_accepted");
}

#[test]
fn unit_mode_requires_a_unit_and_keeps_the_span_inside_it() {
    let dir = TempDir::new().unwrap();
    let mut np = NewProject::new("Content analysis");
    np.coding_mode = CodingMode::Unit;
    np.method_template = "content_analysis".to_string();
    let mut p = Project::create(dir.path().join("ca.pqda"), &np, &opts()).unwrap();
    let source = p.add_source(&NewSource::new("Doc", SourceType::Document, "First one. Second one.")).unwrap();
    let other = p.add_source(&NewSource::new("Other", SourceType::Document, "Elsewhere.")).unwrap();
    let code = p.add_code(&NewCode::new("Topic")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let first = p.add_unit(source, UnitKind::Sentence, 0, 10, None).unwrap();
    let second = p.add_unit(source, UnitKind::Sentence, 11, 22, None).unwrap();

    // No unit: refused.
    assert!(p.apply_coding(&NewCoding::new(source, 0, 10, code, coder)).is_err());

    // With the unit: accepted, and the span can be the whole unit or part of it.
    let mut whole = NewCoding::new(source, 0, 10, code, coder);
    whole.unit_id = Some(first);
    p.apply_coding(&whole).unwrap();
    let mut part = NewCoding::new(source, 11, 17, code, coder);
    part.unit_id = Some(second);
    p.apply_coding(&part).unwrap();

    // Span leaking out of the unit, or a unit from another source: refused.
    let mut leak = NewCoding::new(source, 5, 15, code, coder);
    leak.unit_id = Some(first);
    assert!(p.apply_coding(&leak).is_err());
    let mut wrong = NewCoding::new(other, 0, 5, code, coder);
    wrong.unit_id = Some(first);
    assert!(p.apply_coding(&wrong).is_err());
    assert_eq!(p.info().unwrap().codings, 2);
}

#[test]
fn an_excluded_unit_cannot_be_coded() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    let unit = p.add_unit(source, UnitKind::Sentence, 0, 6, Some("Interviewer")).unwrap();
    raw(p.path()).execute("UPDATE unit SET excluded = 1 WHERE id = ?1", [unit]).unwrap();
    let mut c = NewCoding::new(source, 0, 5, code, coder);
    c.unit_id = Some(unit);
    let err = p.apply_coding(&c).err().unwrap();
    assert!(err.to_string().contains("excluded"), "{err}");
}

#[test]
fn units_must_fit_their_source() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, ..) = seeded(&dir);
    assert!(p.add_unit(source, UnitKind::Sentence, 0, 14, None).is_err());
    assert!(p.add_unit(source, UnitKind::Sentence, 3, 3, None).is_err());
    assert!(matches!(p.add_unit(99, UnitKind::Sentence, 0, 2, None).err().unwrap(), Error::NotFound { .. }));
}

// ---- the published schema ---------------------------------------------------------------------

#[test]
fn the_published_schema_file_is_what_the_engine_installs() {
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v1.sql")).unwrap();
    assert_eq!(on_disk, schema::INITIAL_SCHEMA);
}

#[test]
fn foreign_keys_are_enforced() {
    let dir = TempDir::new().unwrap();
    let (p, _source, code, _coder) = seeded(&dir);
    // Foreign keys are a per-connection setting, so turn them on the way the engine does.
    let c = raw(p.path());
    c.pragma_update(None, "foreign_keys", true).unwrap();
    let err = c
        .execute(
            "INSERT INTO coding (source_id, start_offset, end_offset, code_id, coder_id, created_at)
             VALUES (999, 0, 1, ?1, 1, 'now')",
            [code],
        )
        .unwrap_err();
    assert!(err.to_string().to_lowercase().contains("foreign key"), "{err}");
}

#[test]
fn a_project_is_readable_without_the_engine() {
    // Specification 4.1: other tools can read the file. Open it read-only with plain SQLite.
    let dir = TempDir::new().unwrap();
    let (mut p, source, code, coder) = seeded(&dir);
    p.apply_coding(&NewCoding::new(source, 0, 5, code, coder)).unwrap();
    let c = Connection::open_with_flags(p.path(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let row: (String, String) = c
        .query_row(
            "SELECT code.name, substr(source.text, coding.start_offset + 1, coding.end_offset - coding.start_offset)
             FROM coding JOIN code ON code.id = coding.code_id JOIN source ON source.id = coding.source_id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(row, ("Greeting".to_string(), "Hello".to_string()));
}

#[test]
fn attributes_allow_at_most_three_missing_value_codes() {
    let dir = TempDir::new().unwrap();
    let p = new_project(&dir);
    let c = raw(p.path());
    let ok = c.execute(
        "INSERT INTO attribute (name, applies_to, level, missing_codes_json) VALUES ('Age', 'case', 'ratio', '[-1,-2,-3]')",
        [],
    );
    assert!(ok.is_ok());
    let too_many = c.execute(
        "INSERT INTO attribute (name, applies_to, level, missing_codes_json) VALUES ('Income', 'case', 'ratio', '[1,2,3,4]')",
        [],
    );
    assert!(too_many.is_err());
    let bad_level = c.execute(
        "INSERT INTO attribute (name, applies_to, level) VALUES ('Mood', 'case', 'vibes')",
        [],
    );
    assert!(bad_level.is_err());
}
