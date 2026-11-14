//! Backups in a folder the person chose: where they go, that two projects never share a folder, and that a folder
//! that cannot be used never keeps someone out of their project.

use std::fs;
use std::path::Path;

use prismqda_engine::{check_backup_dir, NewProject, OpenOptions, Project};
use tempfile::TempDir;

fn quiet() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn backups_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    names.sort();
    names
}

#[test]
fn backups_go_beside_the_project_unless_a_folder_is_chosen() {
    let dir = TempDir::new().unwrap();
    let p = Project::create(dir.path().join("Study.pqda"), &NewProject::new("Study"), &quiet()).unwrap();
    let made = p.backup_now().unwrap();
    assert_eq!(made.parent().unwrap(), dir.path().join("Study.backups"));
    assert_eq!(p.backup_folder().unwrap(), dir.path().join("Study.backups"));
}

#[test]
fn a_chosen_folder_gets_one_folder_per_project_named_for_it_and_its_identity() {
    let dir = TempDir::new().unwrap();
    let chosen = dir.path().join("my backups");
    fs::create_dir(&chosen).unwrap();
    let mut o = quiet();
    o.backup_dir = Some(chosen.clone());
    let a = Project::create(dir.path().join("Study.pqda"), &NewProject::new("A"), &o).unwrap();
    fs::create_dir(dir.path().join("other")).unwrap();
    let b = Project::create(dir.path().join("other").join("Study.pqda"), &NewProject::new("B"), &o).unwrap();

    let (made_a, note_a) = a.backup_now_noted().unwrap();
    let (made_b, note_b) = b.backup_now_noted().unwrap();
    assert!(note_a.is_none() && note_b.is_none());
    assert_ne!(made_a.parent(), made_b.parent(), "two projects with the same file name do not share a backup folder");
    for (made, project) in [(&made_a, &a), (&made_b, &b)] {
        assert_eq!(made.parent().unwrap(), project.backup_folder().unwrap());
        assert!(made.parent().unwrap().starts_with(&chosen));
        let folder = made.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
        assert!(folder.starts_with("Study (") && folder.ends_with(").backups"), "{folder}");
    }
    // The copy is an ordinary project: it opens.
    drop(Project::open(&made_a, &quiet()).unwrap());
}

#[test]
fn the_keep_count_applies_inside_each_projects_own_folder() {
    let dir = TempDir::new().unwrap();
    let chosen = dir.path().join("backups");
    fs::create_dir(&chosen).unwrap();
    let mut o = quiet();
    o.backup_dir = Some(chosen);
    o.keep_backups = 2;
    let a = Project::create(dir.path().join("Study.pqda"), &NewProject::new("A"), &o).unwrap();
    for _ in 0..4 {
        a.backup_now().unwrap();
    }
    assert_eq!(backups_in(&a.backup_folder().unwrap()).len(), 2, "the oldest are removed");
}

#[test]
fn the_folder_can_be_changed_while_the_project_is_open() {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("Study.pqda"), &NewProject::new("A"), &quiet()).unwrap();
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    p.set_backup_dir(Some(elsewhere.clone()));
    assert!(p.backup_now().unwrap().starts_with(&elsewhere));
    p.set_backup_dir(None);
    assert_eq!(p.backup_now().unwrap().parent().unwrap(), dir.path().join("Study.backups"));
}

/// A folder that cannot be made: its parent is a file (as a drive that is not plugged in cannot be made either).
fn unusable(dir: &TempDir) -> std::path::PathBuf {
    let file = dir.path().join("not-a-folder.txt");
    fs::write(&file, "x").unwrap();
    file.join("backups")
}

#[test]
fn a_chosen_folder_that_cannot_be_used_falls_back_beside_the_project_and_says_so() {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("Study.pqda"), &NewProject::new("A"), &quiet()).unwrap();
    p.set_backup_dir(Some(unusable(&dir)));
    let (made, note) = p.backup_now_noted().unwrap();
    assert_eq!(made.parent().unwrap(), dir.path().join("Study.backups"), "the project is still protected, beside itself");
    let note = note.expect("and the person is told");
    assert!(note.starts_with("The backup folder ") && note.contains("could not be used") && note.ends_with("beside the project instead."), "{note}");
}

#[test]
fn opening_a_project_with_an_unusable_backup_folder_still_opens_it_with_a_notice() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Study.pqda");
    drop(Project::create(&path, &NewProject::new("A"), &quiet()).unwrap());
    let mut o = OpenOptions::new("tester");
    o.backup_dir = Some(unusable(&dir));
    let p = Project::open(&path, &o).unwrap();
    assert!(p.notices().iter().any(|n| n.contains("could not be used")), "{:?}", p.notices());
    assert!(dir.path().join("Study.backups").is_dir(), "the backup was made beside the project");
}

#[test]
fn a_folder_is_checked_before_it_is_accepted() {
    let dir = TempDir::new().unwrap();
    assert!(check_backup_dir(dir.path()).is_ok());
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none(), "the check leaves nothing behind");
    let missing = check_backup_dir(&dir.path().join("nope")).unwrap_err().to_string();
    assert!(missing.ends_with("was not found."), "{missing}");
    let file = dir.path().join("a.txt");
    fs::write(&file, "x").unwrap();
    assert!(check_backup_dir(&file).unwrap_err().to_string().ends_with("is a file, not a folder."));
}
