//! Bringing an older project file forward: it is copied aside first, migrated in one step, and
//! everything made before keeps working. Newer files still open read-only.

use std::fs;
use std::path::Path;

use prismqda_engine::{schema, CodeEdit, Error, NewProject, OpenOptions, Project};
use rusqlite::Connection;
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn user_version(c: &Connection) -> i32 {
    c.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap()
}

fn coding_columns(c: &Connection) -> Vec<String> {
    let mut stmt = c.prepare("SELECT name FROM pragma_table_info('coding') ORDER BY cid").unwrap();
    stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
}

/// A project file exactly as version 1 made it: the version 1 schema only, none of the migrations,
/// with one source, one code, one coder, and one coding.
fn make_version_1_file(path: &Path) {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.execute_batch(schema::INITIAL_SCHEMA).unwrap();
    c.pragma_update(None, "application_id", schema::APPLICATION_ID).unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    c.execute_batch(
        "INSERT INTO project (id, project_uid, name, created_at) VALUES (1, 'abc', 'Old study', '2026-01-01T00:00:00.000Z');
         INSERT INTO source (id, name, source_type, text, char_count, created_at)
             VALUES (1, 'Interview', 'document', 'I felt welcome from the first day.', 34, '2026-01-01T00:00:00.000Z');
         INSERT INTO code (id, name, created_at) VALUES (1, 'Welcome', '2026-01-01T00:00:00.000Z');
         INSERT INTO coder (id, name, role, local_id, created_at) VALUES (1, 'Ada', 'human', 'ada', '2026-01-01T00:00:00.000Z');
         INSERT INTO coding (id, source_id, start_offset, end_offset, code_id, coder_id, rationale, created_at)
             VALUES (1, 1, 2, 14, 1, 1, 'explicit feeling', '2026-01-02T03:04:05.678Z');
         INSERT INTO event (ts, actor, session, action, target_type, target_id, after_json)
             VALUES ('2026-01-02T03:04:05.678Z', 'Ada', 's1', 'coding.add', 'coding', 1, '{\"code_id\":1}');",
    )
    .unwrap();
}

#[test]
fn the_helper_really_builds_a_version_1_file_without_a_rating_column() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.pqda");
    make_version_1_file(&path);
    let c = Connection::open(&path).unwrap();
    assert_eq!(user_version(&c), 1);
    assert!(!coding_columns(&c).contains(&"rating".to_string()));
}

#[test]
fn schema_version_sixteen_adds_fifteen_migrations_ratings_code_properties_saved_queries_and_maps_ai_recordings_images_adjudication_the_notebook_mixed_methods_pdf_pages_coded_stretches_then_the_list_of_names_then_where_a_theme_stands_then_decisions_on_areas_then_ids_that_are_never_given_again() {
    assert_eq!(schema::MIGRATIONS.len(), 15);
    assert_eq!(schema::SCHEMA_VERSION, 16);
    assert!(schema::MIGRATIONS[14].contains("CREATE TABLE id_floor"));
    assert!(schema::MIGRATIONS[13].contains("CREATE TABLE region_decision"));
    assert!(schema::MIGRATIONS[12].contains("ALTER TABLE code ADD COLUMN theme_stage"));
    assert!(schema::MIGRATIONS[0].contains("ALTER TABLE coding ADD COLUMN rating"));
    assert!(schema::MIGRATIONS[1].contains("ALTER TABLE code ADD COLUMN exclusive_group"));
    assert!(schema::MIGRATIONS[1].contains("ALTER TABLE code_anchor ADD COLUMN kind"));
    for table in ["saved_query", "concept_map", "map_node", "map_link"] {
        assert!(schema::MIGRATIONS[2].contains(&format!("CREATE TABLE {table}")), "{table}");
    }
    assert!(schema::MIGRATIONS[3].contains("ALTER TABLE code ADD COLUMN ai_suggestible"));
    assert!(schema::MIGRATIONS[3].contains("ALTER TABLE project ADD COLUMN ai_mode"));
    assert!(schema::MIGRATIONS[4].contains("CREATE TABLE media"));
    assert!(schema::MIGRATIONS[4].contains("ALTER TABLE unit ADD COLUMN time_start_ms"));
    assert!(schema::MIGRATIONS[6].contains("CREATE TABLE adjudication ") && schema::MIGRATIONS[6].contains("CREATE TABLE adjudication_code"));
    assert!(schema::MIGRATIONS[7].contains("CREATE TABLE stat_page ") && schema::MIGRATIONS[7].contains("CREATE TABLE stat_page_run"));
    for table in ["mixed_design", "triangulation", "integration_entry"] {
        assert!(schema::MIGRATIONS[8].contains(&format!("CREATE TABLE {table}")), "{table}");
    }
    for table in ["pdf_file", "pdf_page"] {
        assert!(schema::MIGRATIONS[9].contains(&format!("CREATE TABLE {table}")), "{table}");
    }
    assert!(schema::MIGRATIONS[9].contains("ALTER TABLE region ADD COLUMN page"));
    assert!(schema::MIGRATIONS[10].contains("CREATE TABLE region_v12") && schema::MIGRATIONS[10].contains("'span'"));
    assert!(schema::MIGRATIONS[10].contains("ALTER TABLE media ADD COLUMN stands_alone"));
    assert!(schema::MIGRATIONS[11].contains("CREATE TABLE deid_rule"));
    assert!(schema::MIGRATIONS[11].contains("ALTER TABLE project ADD COLUMN deid_exports"));
}

#[test]
fn the_published_migration_file_is_what_the_engine_runs() {
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v2.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[0]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v3.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[1]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v4.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[2]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v5.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[3]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v6.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[4]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v7.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[5]);
    for table in ["image", "region"] {
        assert!(schema::MIGRATIONS[5].contains(&format!("CREATE TABLE {table}")), "{table}");
    }
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v9.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[7]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v10.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[8]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v11.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[9]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v12.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[10]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v13.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[11]);
    let on_disk = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v14.sql")).unwrap();
    assert_eq!(on_disk, schema::MIGRATIONS[12]);
    // Version 1's file is untouched: it still describes version 1.
    let v1 = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/v1.sql")).unwrap();
    assert_eq!(v1, schema::INITIAL_SCHEMA);
    assert!(!v1.contains("rating"));
}

#[test]
fn the_published_schema_text_is_version_1_followed_by_each_migration() {
    let text = schema::published_schema();
    assert!(text.starts_with(schema::INITIAL_SCHEMA));
    assert!(text.contains("CREATE TABLE coding"));
    assert!(text.contains("ALTER TABLE coding ADD COLUMN rating"));
    assert!(text.find("CREATE TABLE coding").unwrap() < text.find("ADD COLUMN rating").unwrap());
}

#[test]
fn a_version_1_file_opens_migrates_and_keeps_an_untouched_copy() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.pqda");
    make_version_1_file(&path);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(!p.is_read_only());
    assert_eq!(p.info().unwrap().schema_version, schema::SCHEMA_VERSION);
    assert_eq!(p.notices().len(), 1);
    assert!(p.notices()[0].contains(&format!("upgraded from format 1 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());

    // The original is kept beside the project, still at version 1 and still without the column.
    let copy = dir.path().join("old.before-format-1.pqda");
    assert!(copy.is_file(), "the original must be kept as a copy");
    assert!(p.notices()[0].contains("before-format-1"));
    let kept = Connection::open(&copy).unwrap();
    assert_eq!(user_version(&kept), 1);
    assert!(!coding_columns(&kept).contains(&"rating".to_string()));
    let kept_codings: i64 = kept.query_row("SELECT COUNT(*) FROM coding", [], |r| r.get(0)).unwrap();
    assert_eq!(kept_codings, 1);

    // The migrated file has the column, a version of 2, and the migration in its audit trail.
    let c = Connection::open(&path).unwrap();
    assert_eq!(user_version(&c), schema::SCHEMA_VERSION);
    assert!(coding_columns(&c).contains(&"rating".to_string()));
    let events = p.events(10).unwrap();
    let migrate = events.iter().find(|e| e.action == "project.migrate").expect("a migrate event");
    assert_eq!(migrate.before.as_ref().unwrap()["schema_version"], 1);
    assert_eq!(migrate.after.as_ref().unwrap()["schema_version"], schema::SCHEMA_VERSION);

    // A coding made before the migration still works, and is simply not rated.
    let codings = p.list_codings(1).unwrap();
    assert_eq!(codings.len(), 1);
    assert_eq!(codings[0].rating, None);
    assert_eq!(codings[0].rationale, "explicit feeling");
    assert_eq!(codings[0].created_at, "2026-01-02T03:04:05.678Z");
    // Ratings live on rated codes, so the code is made one first.
    assert!(p.set_coding_rating(1, Some(4)).is_err());
    p.update_code(1, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
    p.set_coding_rating(1, Some(4)).unwrap();
    assert_eq!(p.list_codings(1).unwrap()[0].rating, Some(4));
    assert_eq!(p.coded_segments(&Default::default()).unwrap()[0].rating, Some(4));
}

#[test]
fn a_migrated_file_has_the_same_coding_table_as_a_new_one() {
    let dir = TempDir::new().unwrap();
    let old = dir.path().join("old.pqda");
    make_version_1_file(&old);
    drop(Project::open(&old, &opts()).unwrap());
    let new = dir.path().join("new.pqda");
    drop(Project::create(&new, &NewProject::new("New"), &opts()).unwrap());

    assert_eq!(coding_columns(&Connection::open(&old).unwrap()), coding_columns(&Connection::open(&new).unwrap()));
}

#[test]
fn a_file_is_migrated_once() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.pqda");
    make_version_1_file(&path);
    drop(Project::open(&path, &opts()).unwrap());

    let again = Project::open(&path, &opts()).unwrap();
    assert!(again.notices().is_empty(), "nothing to migrate the second time: {:?}", again.notices());
    let migrations = again.events(100).unwrap().iter().filter(|e| e.action == "project.migrate").count();
    assert_eq!(migrations, 1);
}

#[test]
fn a_new_project_records_the_current_version_and_has_the_column() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("new.pqda");
    let p = Project::create(&path, &NewProject::new("New"), &opts()).unwrap();
    assert_eq!(p.info().unwrap().schema_version, schema::SCHEMA_VERSION);
    assert_eq!(p.events(1).unwrap()[0].after.as_ref().unwrap()["schema_version"], schema::SCHEMA_VERSION);
    assert!(coding_columns(&Connection::open(&path).unwrap()).contains(&"rating".to_string()));
}

#[test]
fn the_database_itself_refuses_a_rating_outside_one_to_five() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.pqda");
    make_version_1_file(&path);
    drop(Project::open(&path, &opts()).unwrap());

    let c = Connection::open(&path).unwrap();
    for bad in [0, 6, -1, 100] {
        assert!(c.execute("UPDATE coding SET rating = ?1", [bad]).is_err(), "{bad} must be refused");
    }
    for good in 1..=5 {
        c.execute("UPDATE coding SET rating = ?1", [good]).unwrap();
    }
    c.execute("UPDATE coding SET rating = NULL", []).unwrap();
}

#[test]
fn a_newer_file_still_opens_read_only_and_is_left_alone() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("future.pqda");
    drop(Project::create(&path, &NewProject::new("Future"), &opts()).unwrap());
    Connection::open(&path).unwrap().pragma_update(None, "user_version", schema::SCHEMA_VERSION + 1).unwrap();
    let before = fs::read(&path).unwrap();

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.is_read_only());
    assert!(p.notices()[0].contains("newer version"));
    assert!(matches!(p.set_coding_rating(1, Some(3)).err().unwrap(), Error::ReadOnly(_)));
    assert!(matches!(p.add_case("Urban Residents", "").err().unwrap(), Error::ReadOnly(_)));
    drop(p);
    assert_eq!(fs::read(&path).unwrap(), before, "a read-only open must not change the file");
    for version in [schema::SCHEMA_VERSION, schema::SCHEMA_VERSION + 1] {
        assert!(!dir.path().join(format!("future.before-format-{version}.pqda")).exists(), "nothing is copied or migrated");
    }
}

/// A project file exactly as version 2 made it: version 1 and the rating step, none of the later steps. It has two
/// codes, one coding with a rating, one without, and an anchor example.
fn make_version_2_file(path: &Path) {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.execute_batch(schema::INITIAL_SCHEMA).unwrap();
    c.execute_batch(schema::MIGRATIONS[0]).unwrap();
    c.pragma_update(None, "application_id", schema::APPLICATION_ID).unwrap();
    c.pragma_update(None, "user_version", 2).unwrap();
    c.execute_batch(
        "INSERT INTO project (id, project_uid, name, created_at) VALUES (1, 'abc', 'Older study', '2026-01-01T00:00:00.000Z');
         INSERT INTO source (id, name, source_type, text, char_count, created_at)
             VALUES (1, 'Interview', 'document', 'I felt welcome from the first day.', 34, '2026-01-01T00:00:00.000Z');
         INSERT INTO code (id, name, created_at) VALUES (1, 'Welcome', '2026-01-01T00:00:00.000Z');
         INSERT INTO code (id, name, created_at) VALUES (2, 'Cost', '2026-01-01T00:00:00.000Z');
         INSERT INTO coder (id, name, role, local_id, created_at) VALUES (1, 'Ada', 'human', 'ada', '2026-01-01T00:00:00.000Z');
         INSERT INTO coding (id, source_id, start_offset, end_offset, code_id, coder_id, rating, created_at)
             VALUES (1, 1, 2, 14, 1, 1, 4, '2026-01-02T03:04:05.678Z');
         INSERT INTO coding (id, source_id, start_offset, end_offset, code_id, coder_id, created_at)
             VALUES (2, 1, 20, 34, 2, 1, '2026-01-02T03:04:06.678Z');
         INSERT INTO code_anchor (code_id, coding_id) VALUES (1, 1);",
    )
    .unwrap();
}

#[test]
fn a_version_2_file_keeps_everything_and_a_code_that_was_rated_stays_rated() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("older.pqda");
    make_version_2_file(&path);

    let p = Project::open(&path, &opts()).unwrap();
    assert_eq!(p.info().unwrap().schema_version, schema::SCHEMA_VERSION);
    assert!(p.notices()[0].contains(&format!("upgraded from format 2 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("older.before-format-2.pqda").is_file(), "the original is kept");

    let codes = p.list_codes().unwrap();
    let welcome = codes.iter().find(|c| c.name == "Welcome").unwrap();
    let cost = codes.iter().find(|c| c.name == "Cost").unwrap();
    assert!(welcome.rated, "its coding had a rating, so the code is rated");
    assert!(!cost.rated, "no rating was ever given for it");
    assert_eq!((welcome.exclusive_group.as_str(), welcome.rating_label.as_str()), ("", ""));
    assert_eq!(p.list_codings(1).unwrap()[0].rating, Some(4), "the rating itself is untouched");
    assert_eq!(p.list_anchors(1).unwrap(), vec![1], "an anchor is an example");
    assert!(p.list_counterexamples(1).unwrap().is_empty());
}

#[test]
fn a_migrated_file_has_the_same_code_table_as_a_new_one() {
    let dir = TempDir::new().unwrap();
    let old = dir.path().join("older.pqda");
    make_version_2_file(&old);
    drop(Project::open(&old, &opts()).unwrap());
    let new = dir.path().join("new.pqda");
    drop(Project::create(&new, &NewProject::new("New"), &opts()).unwrap());
    let columns = |p: &Path, table: &str| -> Vec<String> {
        let c = Connection::open(p).unwrap();
        let mut stmt = c.prepare(&format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid")).unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    };
    assert_eq!(columns(&old, "code"), columns(&new, "code"));
    assert_eq!(columns(&old, "code_anchor"), columns(&new, "code_anchor"));
}

#[test]
fn a_version_3_file_gains_the_saved_query_and_concept_map_tables_and_keeps_an_untouched_copy() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("three.pqda");
    let p = Project::create(&path, &NewProject::new("Three"), &opts()).unwrap();
    drop(p);
    // Make it a version 3 file: no saved queries or maps yet, and none of version 5's AI settings.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE IF EXISTS id_floor; DROP TABLE IF EXISTS region_decision; DROP TABLE IF EXISTS deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.execute_batch("DROP TABLE IF EXISTS pdf_page; DROP TABLE IF EXISTS pdf_file; DROP TABLE IF EXISTS integration_entry; DROP TABLE IF EXISTS triangulation; DROP TABLE IF EXISTS mixed_design; DROP TABLE IF EXISTS stat_page_run; DROP TABLE IF EXISTS stat_page; DROP TABLE IF EXISTS adjudication_code; DROP TABLE IF EXISTS adjudication; DROP TABLE map_link; DROP TABLE map_node; DROP TABLE concept_map; DROP TABLE saved_query;").unwrap();
    c.execute_batch("ALTER TABLE code DROP COLUMN ai_suggestible; ALTER TABLE project DROP COLUMN ai_mode;").unwrap();
    c.execute_batch("DROP TABLE IF EXISTS pdf_page; DROP TABLE IF EXISTS pdf_file; DROP TABLE IF EXISTS integration_entry; DROP TABLE IF EXISTS triangulation; DROP TABLE IF EXISTS mixed_design; DROP TABLE IF EXISTS stat_page_run; DROP TABLE IF EXISTS stat_page; DROP TABLE IF EXISTS adjudication_code; DROP TABLE IF EXISTS adjudication; DROP TABLE region; DROP TABLE image; DROP TABLE media; ALTER TABLE unit DROP COLUMN time_start_ms; ALTER TABLE unit DROP COLUMN time_end_ms;").unwrap();
    c.pragma_update(None, "user_version", 3).unwrap();
    drop(c);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(!p.is_read_only());
    assert_eq!(p.info().unwrap().schema_version, schema::SCHEMA_VERSION);
    assert!(p.notices()[0].contains(&format!("upgraded from format 3 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    let copy = dir.path().join("three.before-format-3.pqda");
    assert!(copy.is_file(), "the original is kept");
    let kept = Connection::open(&copy).unwrap();
    assert_eq!(user_version(&kept), 3);
    let tables: i64 = kept.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'saved_query'", [], |r| r.get(0)).unwrap();
    assert_eq!(tables, 0, "the kept copy is still the old format");
    // The new tables work.
    let id = p.save_query("Anything", "", &serde_json::json!({}), &serde_json::json!({ "rule": { "type": "group", "match": "any", "items": [{ "type": "rating", "min": 1 }] } })).unwrap();
    assert_eq!(p.get_saved_query(id).unwrap().name, "Anything");
}

#[test]
fn a_version_4_file_gains_the_ai_settings_with_nothing_changed_for_the_researcher() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("four.pqda");
    let mut p = Project::create(&path, &NewProject::new("Four"), &opts()).unwrap();
    let code = p.add_code(&prismqda_engine::NewCode::new("Welcome")).unwrap();
    drop(p);
    // Make it a version 4 file: no AI columns.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE IF EXISTS id_floor; DROP TABLE IF EXISTS region_decision; DROP TABLE IF EXISTS deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.execute_batch("ALTER TABLE code DROP COLUMN ai_suggestible; ALTER TABLE project DROP COLUMN ai_mode;").unwrap();
    c.execute_batch("DROP TABLE IF EXISTS pdf_page; DROP TABLE IF EXISTS pdf_file; DROP TABLE IF EXISTS integration_entry; DROP TABLE IF EXISTS triangulation; DROP TABLE IF EXISTS mixed_design; DROP TABLE IF EXISTS stat_page_run; DROP TABLE IF EXISTS stat_page; DROP TABLE IF EXISTS adjudication_code; DROP TABLE IF EXISTS adjudication; DROP TABLE region; DROP TABLE image; DROP TABLE media; ALTER TABLE unit DROP COLUMN time_start_ms; ALTER TABLE unit DROP COLUMN time_end_ms;").unwrap();
    c.pragma_update(None, "user_version", 4).unwrap();
    drop(c);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 4 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("four.before-format-4.pqda").is_file(), "the original is kept");
    // Prompt packets are available, and every code may be suggested, exactly as before nothing existed to say otherwise.
    assert_eq!(p.info().unwrap().ai_mode, "packet");
    assert!(p.get_code(code).unwrap().ai_suggestible);
    p.set_ai_mode("off").unwrap();
    assert_eq!(p.info().unwrap().ai_mode, "off");
    // The database itself refuses a setting that is neither.
    let c = Connection::open(&path).unwrap();
    assert!(c.execute("UPDATE project SET ai_mode = 'cloud'", []).is_err());
}

#[test]
fn a_version_5_file_gains_recordings_and_unit_times_and_keeps_what_it_had() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("five.pqda");
    let mut p = Project::create(&path, &NewProject::new("Five"), &opts()).unwrap();
    let src = p.add_source(&prismqda_engine::NewSource::new("Talk", prismqda_engine::SourceType::Document, "Some words here.")).unwrap();
    drop(p);
    // Make it a version 5 file: no recordings, and no times on units.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE IF EXISTS id_floor; DROP TABLE IF EXISTS region_decision; DROP TABLE IF EXISTS deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.execute_batch("DROP TABLE IF EXISTS pdf_page; DROP TABLE IF EXISTS pdf_file; DROP TABLE IF EXISTS integration_entry; DROP TABLE IF EXISTS triangulation; DROP TABLE IF EXISTS mixed_design; DROP TABLE IF EXISTS stat_page_run; DROP TABLE IF EXISTS stat_page; DROP TABLE IF EXISTS adjudication_code; DROP TABLE IF EXISTS adjudication; DROP TABLE region; DROP TABLE image; DROP TABLE media; ALTER TABLE unit DROP COLUMN time_start_ms; ALTER TABLE unit DROP COLUMN time_end_ms;").unwrap();
    c.pragma_update(None, "user_version", 5).unwrap();
    drop(c);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 5 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("five.before-format-5.pqda").is_file(), "the original is kept");
    assert_eq!(p.source_detail(src).unwrap().text, "Some words here.", "the document is as it was");
    assert!(p.list_units(src).unwrap().iter().all(|u| u.time_start_ms.is_none()), "units have no times until a file with times gives them");
    let recording = dir.path().join("talk.wav");
    fs::write(&recording, vec![0u8; 64]).unwrap();
    assert!(p.link_media(src, recording.to_str().unwrap()).is_ok(), "and a recording can be linked");
}

#[test]
fn a_version_6_file_gains_pictures_and_coded_areas_and_keeps_what_it_had() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("six.pqda");
    let mut p = Project::create(&path, &NewProject::new("Six"), &opts()).unwrap();
    let src = p.add_source(&prismqda_engine::NewSource::new("Talk", prismqda_engine::SourceType::Document, "Some words here.")).unwrap();
    drop(p);
    // Make it a version 6 file: no pictures and no coded areas.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE IF EXISTS id_floor; DROP TABLE IF EXISTS region_decision; DROP TABLE IF EXISTS deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.execute_batch("ALTER TABLE media DROP COLUMN stands_alone; DROP TABLE IF EXISTS pdf_page; DROP TABLE IF EXISTS pdf_file; DROP TABLE IF EXISTS integration_entry; DROP TABLE IF EXISTS triangulation; DROP TABLE IF EXISTS mixed_design; DROP TABLE IF EXISTS stat_page_run; DROP TABLE IF EXISTS stat_page; DROP TABLE IF EXISTS adjudication_code; DROP TABLE IF EXISTS adjudication; DROP TABLE region; DROP TABLE image;").unwrap();
    c.pragma_update(None, "user_version", 6).unwrap();
    drop(c);

    let p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 6 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("six.before-format-6.pqda").is_file(), "the original is kept");
    assert_eq!(p.source_detail(src).unwrap().text, "Some words here.", "the document is as it was");
    assert!(p.list_images().unwrap().is_empty() && p.all_regions().unwrap().is_empty());
}

#[test]
fn a_version_10_file_gains_pdf_pages_and_every_coded_area_it_had_is_on_page_one() {
    use prismqda_engine::{NewCode, NewCoder, NewRegion, Shape};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ten.pqda");
    let mut p = Project::create(&path, &NewProject::new("Ten"), &opts()).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Street")).unwrap();
    // A picture with a coded area, the way a version 10 project had one.
    let image = prismqda_engine::read_image("street.png", {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend_from_slice(&640u32.to_be_bytes());
        b.extend_from_slice(&480u32.to_be_bytes());
        b.extend_from_slice(&[8, 2, 0, 0, 0]);
        b
    })
    .unwrap();
    let batch = prismqda_engine::ImportBatch {
        sources: vec![prismqda_engine::ImportedSource {
            name: "Street".to_string(),
            source_type: prismqda_engine::SourceType::Other,
            folder: String::new(),
            text: "A picture of a street.".to_string(),
            runs: vec![],
            units: vec![prismqda_engine::ImportedUnit::new(0, 22, prismqda_engine::UnitKind::Paragraph)],
            default_unit_kind: prismqda_engine::UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        images: vec![prismqda_engine::ImageAttachment { source: 0, image }],
        ..Default::default()
    };
    let source = p.apply_import(&batch).unwrap().source_ids[0];
    p.apply_region(&NewRegion::new(source, Shape::Rectangle { x: 0.1, y: 0.1, w: 0.3, h: 0.3 }, code, coder)).unwrap();
    drop(p);
    // Make it a version 10 file: no PDFs, and no page on a coded area.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE IF EXISTS id_floor; DROP TABLE IF EXISTS region_decision; DROP TABLE IF EXISTS deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.execute_batch("ALTER TABLE media DROP COLUMN stands_alone; DROP INDEX region_page; ALTER TABLE region DROP COLUMN page; DROP TABLE pdf_page; DROP TABLE pdf_file;").unwrap();
    c.pragma_update(None, "user_version", 10).unwrap();
    drop(c);

    let p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 10 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("ten.before-format-10.pqda").is_file(), "the original is kept");
    let areas = p.list_regions(source).unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0].page, 1, "an area of a picture is on its one page");
    assert!(p.get_pdf(source).unwrap().is_none());
}

#[test]
fn a_version_11_file_gains_coded_stretches_and_every_coded_area_it_had_is_kept_as_it_was() {
    use prismqda_engine::{NewCode, NewCoder, NewRegion, Shape};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("eleven.pqda");
    let mut p = Project::create(&path, &NewProject::new("Eleven"), &opts()).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Street")).unwrap();
    let image = prismqda_engine::read_image("street.png", {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend_from_slice(&640u32.to_be_bytes());
        b.extend_from_slice(&480u32.to_be_bytes());
        b.extend_from_slice(&[8, 2, 0, 0, 0]);
        b
    })
    .unwrap();
    let batch = prismqda_engine::ImportBatch {
        sources: vec![prismqda_engine::ImportedSource {
            name: "Street".to_string(),
            source_type: prismqda_engine::SourceType::Other,
            folder: String::new(),
            text: "A picture of a street.".to_string(),
            runs: vec![],
            units: vec![prismqda_engine::ImportedUnit::new(0, 22, prismqda_engine::UnitKind::Paragraph)],
            default_unit_kind: prismqda_engine::UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        images: vec![prismqda_engine::ImageAttachment { source: 0, image }],
        ..Default::default()
    };
    let source = p.apply_import(&batch).unwrap().source_ids[0];
    let area = p.apply_region(&NewRegion::new(source, Shape::Rectangle { x: 0.1, y: 0.2, w: 0.3, h: 0.4 }, code, coder)).unwrap();
    let before = p.get_region(area).unwrap();
    let recording = dir.path().join("talk.wav");
    fs::write(&recording, vec![0u8; 64]).unwrap();
    let talk = p.add_source(&prismqda_engine::NewSource::new("Talk", prismqda_engine::SourceType::Other, "A talk.")).unwrap();
    p.link_media(talk, recording.to_str().unwrap()).unwrap();
    drop(p);
    // Make it a version 11 file: the old region table (three shapes, boxes inside 0 to 1) and no mark on a recording.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE IF EXISTS id_floor; DROP TABLE IF EXISTS region_decision; DROP TABLE IF EXISTS deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.execute_batch(
        "ALTER TABLE media DROP COLUMN stands_alone;
         DROP INDEX region_source; DROP INDEX region_code; DROP INDEX region_coder; DROP INDEX region_page;
         ALTER TABLE region RENAME TO region_new_format;
         CREATE TABLE region (
             id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL REFERENCES source (id),
             shape TEXT NOT NULL CHECK (shape IN ('rectangle', 'ellipse', 'polygon')), geometry_json TEXT NOT NULL CHECK (json_valid(geometry_json)),
             x0 REAL NOT NULL, y0 REAL NOT NULL, x1 REAL NOT NULL, y1 REAL NOT NULL,
             code_id INTEGER NOT NULL REFERENCES code (id), coder_id INTEGER NOT NULL REFERENCES coder (id),
             origin TEXT NOT NULL DEFAULT 'human', layer TEXT NOT NULL DEFAULT 'coder', rationale TEXT NOT NULL DEFAULT '',
             rating INTEGER, created_at TEXT NOT NULL, page INTEGER NOT NULL DEFAULT 1 CHECK (page >= 1),
             CHECK (x0 >= 0 AND y0 >= 0 AND x1 <= 1 AND y1 <= 1 AND x1 > x0 AND y1 > y0)
         );
         INSERT INTO region SELECT id, source_id, shape, geometry_json, x0, y0, x1, y1, code_id, coder_id, origin, layer, rationale, rating, created_at, page FROM region_new_format;
         DROP TABLE region_new_format;
         CREATE INDEX region_source ON region (source_id);
         CREATE INDEX region_page ON region (source_id, page);",
    )
    .unwrap();
    assert!(c.execute("INSERT INTO region (source_id, shape, geometry_json, x0, y0, x1, y1, code_id, coder_id, created_at) VALUES (1, 'span', '{}', 1000, 0, 2000, 1, 1, 1, 'now')", []).is_err(), "the old table refuses a stretch");
    c.pragma_update(None, "user_version", 11).unwrap();
    drop(c);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 11 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("eleven.before-format-11.pqda").is_file(), "the original is kept");
    assert_eq!(p.get_region(area).unwrap(), before, "a coded area is exactly as it was");
    assert!(!p.get_media(talk).unwrap().unwrap().stands_alone, "a recording linked before stands for nothing");
    let id = p.apply_region(&NewRegion::new(talk, Shape::Span { start_ms: 1000, end_ms: 2500 }, code, coder)).unwrap();
    assert_eq!(p.get_region(id).unwrap().shape, Shape::Span { start_ms: 1000, end_ms: 2500 });
    let c = Connection::open(&path).unwrap();
    let indexes: Vec<String> = c.prepare("SELECT name FROM pragma_index_list('region') WHERE name LIKE 'region_%' ORDER BY name").unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(indexes, ["region_code", "region_coder", "region_page", "region_source"], "the indexes are all there again");
}

#[test]
fn a_version_12_file_gains_the_list_of_names_and_the_switch_and_keeps_what_it_had() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("twelve.pqda");
    let mut p = Project::create(&path, &NewProject::new("Twelve"), &opts()).unwrap();
    let src = p.add_source(&prismqda_engine::NewSource::new("Talk", prismqda_engine::SourceType::Document, "Ann spoke to Bo.")).unwrap();
    drop(p);
    // Make it a version 12 file: no list of names and no switch for exports.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE id_floor; DROP TABLE region_decision; DROP TABLE deid_rule; ALTER TABLE project DROP COLUMN deid_exports; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.pragma_update(None, "user_version", 12).unwrap();
    drop(c);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 12 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("twelve.before-format-12.pqda").is_file(), "the original is kept");
    assert_eq!(p.source_text(src).unwrap(), "Ann spoke to Bo.");
    let state = p.deid_state().unwrap();
    assert!(state.rules.is_empty() && !state.exports && state.last_applied.is_none());
    p.add_deid_rules(&prismqda_engine::NewDeidRules { terms: vec!["Ann".into()], replacement: "P1".into(), kind: "pseudonym".into(), match_case: false, whole_word: true, note: String::new() }).unwrap();
    p.set_deid_exports(true).unwrap();
    assert!(p.export_scrubber().unwrap().is_some());
}

#[test]
fn a_version_13_file_gains_where_a_theme_stands_and_every_theme_begins_as_a_candidate() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("thirteen.pqda");
    let mut p = Project::create(&path, &NewProject::new("Thirteen"), &opts()).unwrap();
    let mut theme = prismqda_engine::NewCode::new("Belonging");
    theme.theme = true;
    let theme_id = p.add_code(&theme).unwrap();
    drop(p);
    // Make it a version 13 file: no stage for a theme.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE id_floor; DROP TABLE region_decision; ALTER TABLE code DROP COLUMN theme_stage;").unwrap();
    c.pragma_update(None, "user_version", 13).unwrap();
    drop(c);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 13 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    assert!(dir.path().join("thirteen.before-format-13.pqda").is_file(), "the original is kept");
    let code = p.get_code(theme_id).unwrap();
    assert_eq!((code.kind.as_str(), code.theme_stage.as_str()), ("theme", "candidate"));
    p.update_code(theme_id, &prismqda_engine::CodeEdit { theme_stage: Some("reviewed".into()), ..Default::default() }).unwrap();
    assert_eq!(p.get_code(theme_id).unwrap().theme_stage, "reviewed");
    assert!(p.update_code(theme_id, &prismqda_engine::CodeEdit { theme_stage: Some("finished".into()), ..Default::default() }).is_err());
}

#[test]
fn a_version_15_file_gains_the_id_floor_and_starts_from_the_highest_ids_it_holds() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fifteen.pqda");
    let mut p = Project::create(&path, &NewProject::new("Fifteen"), &opts()).unwrap();
    let a = p.add_code(&prismqda_engine::NewCode::new("Welcome")).unwrap();
    let b = p.add_code(&prismqda_engine::NewCode::new("Cost")).unwrap();
    let case = p.add_case("Maria", "").unwrap();
    drop(p);
    // Make it a version 15 file: no table of the highest ids given.
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE id_floor;").unwrap();
    c.pragma_update(None, "user_version", 15).unwrap();
    drop(c);

    let mut p = Project::open(&path, &opts()).unwrap();
    assert!(p.notices()[0].contains(&format!("upgraded from format 15 to {}", schema::SCHEMA_VERSION)), "{:?}", p.notices());
    let floor: Vec<(String, i64)> = {
        let c = Connection::open(&path).unwrap();
        let mut s = c.prepare("SELECT table_name, last_id FROM id_floor ORDER BY table_name").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
    };
    assert_eq!(floor, vec![("code".to_string(), b), ("research_case".to_string(), case.id)]);
    assert!(b > a);
    let next = p.add_code(&prismqda_engine::NewCode::new("Safety")).unwrap();
    assert_eq!(next, b + 1, "a code added after the upgrade takes the number above the highest held");
}
