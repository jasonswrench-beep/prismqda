//! The case table for PrismStatistics through the command layer (`export.pstat`): the file appears under the name asked for with its ending,
//! nothing is replaced unasked, the list of names to replace is applied, and a project with no cases is told so in words.

use std::io::Read;

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn entry(path: &str, name: &str) -> Value {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut text = String::new();
    zip.by_name(name).unwrap().read_to_string(&mut text).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Two respondents as cases (a spreadsheet of answers: the name, a score, a comment to code).
fn project(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Staff survey" }));
    let csv = dir.path().join("answers.csv");
    std::fs::write(&csv, "Respondent,Score,Comment\nSarah Jones,4,The market was friendly\nTomas Reyes,2,Too far to travel\n").unwrap();
    let mapping = json!({
        "id_column": "Respondent",
        "text_columns": ["Comment"],
        "attribute_columns": [{ "name": "Score", "level": "ratio" }]
    });
    ok(&mut a, "import.run", json!({ "path": csv, "mapping": mapping }));
    a
}

#[test]
fn the_case_table_is_written_with_its_ending_and_says_what_it_holds() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    assert_eq!(ok(&mut a, "case.list", json!({})).as_array().unwrap().len(), 2);
    let target = dir.path().join("table").display().to_string();
    let out = ok(&mut a, "export.pstat", json!({ "path": target, "app_version": "0.1.0-beta.2" }));
    let path = out["path"].as_str().unwrap().to_string();
    assert!(path.ends_with("table.pstat"), "{path}");
    assert!(out["bytes"].as_i64().unwrap() > 300);
    assert_eq!(out["report"]["cases"], 2);
    assert!(out["report"]["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("No excerpt")), "{out}");

    let project = entry(&path, "project.json");
    assert_eq!(project["app_version"], "PrismQDA 0.1.0-beta.2");
    assert_eq!(project["profile"], "PrismQDA case table 1");
    assert_eq!(project["dataset"]["name"], "Staff survey");
    assert_eq!(entry(&path, "case-table.json")["project"]["name"], "Staff survey");

    // The name asked for is kept as it is when it already has the ending, and a file that is there is not replaced.
    assert_eq!(err(&mut a, "export.pstat", json!({ "path": target })).code, "already_exists");
    let again = ok(&mut a, "export.pstat", json!({ "path": target, "overwrite": true, "leave_out_names": true }));
    assert!(again["report"]["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("names were left out")));
}

#[test]
fn a_release_that_is_not_a_plain_version_is_not_written_into_the_file() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let out = ok(&mut a, "export.pstat", json!({ "path": dir.path().join("t1"), "app_version": "9 <script>" }));
    let version = entry(out["path"].as_str().unwrap(), "project.json")["app_version"].as_str().unwrap().to_string();
    assert!(version.starts_with("PrismQDA ") && !version.contains("script"), "{version}");
}

#[test]
fn the_list_of_names_to_replace_is_applied_to_the_case_names() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    ok(&mut a, "case.add", json!({ "name": "Sarah Jones (second)" }));
    ok(&mut a, "deid.add", json!({ "terms": ["Sarah Jones"], "replacement": "Participant 1" }));
    ok(&mut a, "deid.set_exports", json!({ "on": true }));
    let out = ok(&mut a, "export.pstat", json!({ "path": dir.path().join("t2") }));
    let project = entry(out["path"].as_str().unwrap(), "project.json");
    let text = project.to_string();
    assert!(!text.contains("Sarah Jones"), "the name must not reach the file: {text}");
    assert!(text.contains("Participant 1"));
    assert!(out["report"]["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("list of names to replace")));
}

#[test]
fn a_project_with_no_cases_is_told_so() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Empty" }));
    let e = err(&mut a, "export.pstat", json!({ "path": dir.path().join("t3") }));
    assert!(e.message.contains("no cases"), "{}", e.message);
    assert!(!dir.path().join("t3.pstat").exists(), "nothing was written");
}

#[test]
fn nothing_is_written_when_no_project_is_open() {
    let mut a = Api::new("Ada").without_backup_on_open();
    let e = err(&mut a, "export.pstat", json!({ "path": "x" }));
    assert!(!e.message.is_empty());
}
