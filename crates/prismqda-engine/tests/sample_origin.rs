//! A project can remember which bundled sample it was copied from. The note lives in the project's creation
//! event, so it needs no new table and no new format version, and it cannot be changed afterwards.

use prismqda_engine::{schema, NewProject, OpenOptions, Project};
use rusqlite::Connection;
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn from_sample(dir: &TempDir, file: &str, sample: &str) -> Project {
    let mut new = NewProject::new("A copy");
    new.sample_id = Some(sample.to_string());
    Project::create(dir.path().join(file), &new, &opts()).unwrap()
}

#[test]
fn an_ordinary_project_has_no_sample_and_its_creation_event_is_as_it_always_was() {
    let dir = TempDir::new().unwrap();
    let p = Project::create(dir.path().join("p.pqda"), &NewProject::new("Mine"), &opts()).unwrap();
    assert_eq!(NewProject::new("x").sample_id, None);
    assert_eq!(p.info().unwrap().sample_id, None);

    let created = &p.events(10).unwrap()[0];
    assert_eq!(created.action, "project.create");
    let after = created.after.as_ref().unwrap().as_object().unwrap();
    let mut keys: Vec<&str> = after.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["coding_mode", "method_template", "name", "schema_version"], "no sample key unless there is a sample");
}

#[test]
fn a_copy_of_a_sample_reports_it_now_and_after_every_reopen() {
    let dir = TempDir::new().unwrap();
    let path = {
        let p = from_sample(&dir, "copy.pqda", "urban-mobility");
        assert_eq!(p.info().unwrap().sample_id.as_deref(), Some("urban-mobility"));
        p.path().to_path_buf()
    };
    let mut again = Project::open(&path, &opts()).unwrap();
    assert_eq!(again.info().unwrap().sample_id.as_deref(), Some("urban-mobility"));

    // Working in it changes nothing about where it came from, however many events follow.
    again.annotate("one").unwrap();
    again.annotate("two").unwrap();
    assert_eq!(again.info().unwrap().sample_id.as_deref(), Some("urban-mobility"));
    let created = &again.events(10).unwrap()[0];
    assert_eq!(created.after.as_ref().unwrap()["sample_id"], "urban-mobility");
}

#[test]
fn two_copies_of_one_sample_are_two_projects() {
    let dir = TempDir::new().unwrap();
    let a = from_sample(&dir, "a.pqda", "urban-mobility");
    let b = from_sample(&dir, "b.pqda", "urban-mobility");
    let (ia, ib) = (a.info().unwrap(), b.info().unwrap());
    assert_eq!(ia.sample_id, ib.sample_id);
    assert_ne!(ia.project_uid, ib.project_uid);
}

#[test]
fn the_note_costs_the_file_nothing_in_structure() {
    let dir = TempDir::new().unwrap();
    drop(Project::create(dir.path().join("plain.pqda"), &NewProject::new("Plain"), &opts()).unwrap());
    drop(from_sample(&dir, "copy.pqda", "urban-mobility"));

    let shape = |file: &str| -> (Vec<String>, i32) {
        let c = Connection::open(dir.path().join(file)).unwrap();
        let mut stmt = c.prepare("SELECT type || ' ' || name FROM sqlite_master ORDER BY 1").unwrap();
        let names = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect();
        (names, c.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap())
    };
    assert_eq!(shape("copy.pqda"), shape("plain.pqda"));
    assert_eq!(shape("copy.pqda").1, schema::SCHEMA_VERSION);
}

#[test]
fn a_file_from_a_newer_version_still_says_where_it_came_from() {
    let dir = TempDir::new().unwrap();
    let path = from_sample(&dir, "copy.pqda", "urban-mobility").path().to_path_buf();
    {
        let c = Connection::open(&path).unwrap();
        c.pragma_update(None, "user_version", schema::SCHEMA_VERSION + 1).unwrap();
    }
    let p = Project::open(&path, &opts()).unwrap();
    assert!(p.is_read_only());
    assert_eq!(p.info().unwrap().sample_id.as_deref(), Some("urban-mobility"));
}
