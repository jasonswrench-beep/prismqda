//! A whole project in the REFI-QDA format through the command layer (decision 159): out of one project and into a new one, the refusals,
//! and the rule that the list of names to replace stops a whole-project export.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn project_with_coding(dir: &TempDir) -> (Api, String) {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Study" }));
    let text = dir.path().join("note.txt");
    std::fs::write(&text, "Sarah Jones said the market was friendly. She would go back.").unwrap();
    let out = ok(&mut a, "import.run", json!({ "path": text }));
    let source = out["outcome"]["source_ids"][0].as_i64().unwrap();
    let code = ok(&mut a, "code.add", json!({ "name": "Friendly" }))["id"].as_i64().unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 25, "end": 40, "code_id": code }));
    let target = dir.path().join("study.qdpx").display().to_string();
    (a, target)
}

#[test]
fn a_project_goes_out_as_qdpx_and_comes_in_as_a_new_project_with_a_report() {
    let dir = TempDir::new().unwrap();
    let (mut a, target) = project_with_coding(&dir);
    let out = ok(&mut a, "export.qdpx", json!({ "path": target }));
    assert!(out["path"].as_str().unwrap().ends_with("study.qdpx"));
    assert!(out["bytes"].as_i64().unwrap() > 200);
    assert!(out["report"]["carried"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("1 coding")), "{out}");
    assert!(!out["report"]["left_out"].as_array().unwrap().is_empty(), "the file says what it does not hold");

    // the ending is added, and a file that is there is not replaced
    let second = ok(&mut a, "export.qdpx", json!({ "path": dir.path().join("other") }));
    assert!(second["path"].as_str().unwrap().ends_with("other.qdpx"));
    assert_eq!(err(&mut a, "export.qdpx", json!({ "path": target })).code, "already_exists");

    // into a new project; the project that is open stays the open one
    let new_path = dir.path().join("copy").display().to_string();
    let back = ok(&mut a, "import.qdpx", json!({ "path": target, "project_path": new_path }));
    assert!(back["project_path"].as_str().unwrap().ends_with("copy.pqda"));
    assert!(back["report"]["carried"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("1 coding")), "{back}");
    assert_eq!(ok(&mut a, "project.info", json!({}))["name"], "Study", "the open project did not change");

    let mut b = Api::new("Ada").without_backup_on_open();
    ok(&mut b, "project.open", json!({ "path": back["project_path"] }));
    assert_eq!(ok(&mut b, "project.info", json!({}))["name"], "study", "the file's name names the project when none is given");
    let codes = ok(&mut b, "code.list", json!({}));
    assert!(codes.to_string().contains("Friendly"));
    let sources = ok(&mut b, "source.list", json!({}));
    let id = sources.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let codings = ok(&mut b, "coding.list", json!({ "source_id": id }));
    let c = &codings.as_array().unwrap()[0];
    assert_eq!((c["start"].as_i64(), c["end"].as_i64()), (Some(25), Some(40)));

    // a name can be given
    let named = ok(&mut a, "import.qdpx", json!({ "path": target, "project_path": dir.path().join("named"), "name": "Chosen name" }));
    let mut c2 = Api::new("Ada").without_backup_on_open();
    ok(&mut c2, "project.open", json!({ "path": named["project_path"] }));
    assert_eq!(ok(&mut c2, "project.info", json!({}))["name"], "Chosen name");
}

#[test]
fn a_file_that_is_not_qdpx_is_refused_and_leaves_no_project_behind() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada").without_backup_on_open();
    let bad = dir.path().join("notes.qdpx");
    std::fs::write(&bad, b"this is plain text").unwrap();
    let e = err(&mut a, "import.qdpx", json!({ "path": bad, "project_path": dir.path().join("x") }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("not a QDPX file"), "{}", e.message);
    assert!(!dir.path().join("x.pqda").exists());
    // a project that is already there is not replaced
    std::fs::write(dir.path().join("there.pqda"), b"mine").unwrap();
    assert_eq!(err(&mut a, "import.qdpx", json!({ "path": bad, "project_path": dir.path().join("there.pqda") })).code, "already_exists");
    assert_eq!(std::fs::read(dir.path().join("there.pqda")).unwrap(), b"mine");
    // a file that is not there
    assert_eq!(err(&mut a, "import.qdpx", json!({ "path": dir.path().join("missing.qdpx"), "project_path": dir.path().join("y") })).code, "io");
}

#[test]
fn while_names_are_replaced_in_exports_the_whole_project_is_not_exported() {
    let dir = TempDir::new().unwrap();
    let (mut a, target) = project_with_coding(&dir);
    ok(&mut a, "deid.add", json!({ "terms": ["Sarah Jones"], "replacement": "Participant 1" }));
    ok(&mut a, "deid.set_exports", json!({ "on": true }));
    let e = err(&mut a, "export.qdpx", json!({ "path": target }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("list of names"), "{}", e.message);
    assert!(!std::path::Path::new(&target).exists(), "nothing was written");
    // with the switch off it goes out
    ok(&mut a, "deid.set_exports", json!({ "on": false }));
    ok(&mut a, "export.qdpx", json!({ "path": target }));
}
