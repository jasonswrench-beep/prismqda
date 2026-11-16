//! Codes and cases keep their numbers for good (project format 16): removing the newest one, which is what Undo of "add" does, never
//! lets its number go to the next one added. A file that names codes and cases by number (the case table sent to PrismStatistics)
//! relies on that.

use prismqda_engine::{NewCode, NewProject, OpenOptions, Project};
use rusqlite::Connection;
use tempfile::TempDir;

fn make(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

#[test]
fn a_code_whose_adding_was_undone_never_gives_its_number_to_the_next_code() {
    let dir = TempDir::new().unwrap();
    let mut p = make(&dir);
    let first = p.add_code(&NewCode::new("Welcome")).unwrap();
    let second = p.add_code(&NewCode::new("Cost")).unwrap();
    assert_eq!(second, first + 1);
    p.undo().unwrap().expect("the adding of Cost is undone");
    assert!(p.list_codes().unwrap().iter().all(|c| c.id != second), "the code is gone");
    let third = p.add_code(&NewCode::new("Safety")).unwrap();
    assert!(third > second, "the next code is {third}, not the number {second} that Cost had");
    assert_eq!(p.get_code(third).unwrap().name, "Safety");
}

#[test]
fn a_case_whose_adding_was_undone_never_gives_its_number_to_the_next_case() {
    let dir = TempDir::new().unwrap();
    let mut p = make(&dir);
    let first = p.add_case("Maria", "").unwrap();
    let second = p.add_case("Tomas", "").unwrap();
    assert_eq!(second.id, first.id + 1);
    p.undo().unwrap().expect("the adding of Tomas is undone");
    assert!(p.list_cases().unwrap().iter().all(|c| c.id != second.id));
    let third = p.add_case("Priya", "").unwrap();
    assert!(third.id > second.id, "the next case is {}, not the number {} that Tomas had", third.id, second.id);
}

#[test]
fn the_numbers_keep_rising_through_several_adds_and_undos_and_after_the_file_is_closed_and_opened_again() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut seen = Vec::new();
    {
        let mut p = make(&dir);
        for name in ["A", "B", "C"] {
            seen.push(p.add_code(&NewCode::new(name)).unwrap());
            p.undo().unwrap();
        }
        seen.push(p.add_code(&NewCode::new("D")).unwrap());
    }
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::open(&path, &o).unwrap();
    seen.push(p.add_code(&NewCode::new("E")).unwrap());
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), seen.len(), "no number was given twice: {seen:?}");
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "each is above the one before: {seen:?}");
}

#[test]
fn a_new_project_records_the_highest_numbers_given() {
    let dir = TempDir::new().unwrap();
    let mut p = make(&dir);
    let code = p.add_code(&NewCode::new("Welcome")).unwrap();
    let case = p.add_case("Maria", "").unwrap();
    let c = Connection::open(p.path()).unwrap();
    let last = |name: &str| -> i64 { c.query_row("SELECT last_id FROM id_floor WHERE table_name = ?1", [name], |r| r.get(0)).unwrap() };
    assert_eq!(last("code"), code);
    assert_eq!(last("research_case"), case.id);
}
