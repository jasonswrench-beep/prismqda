//! The name a person goes by in the history of a project (`history.name`): recorded on their changes from then on, and never taken from the
//! Windows account.

use prismqda_api::Api;
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn actors_of_code_changes(api: &mut Api) -> Vec<String> {
    let events = ok(api, "events.list", json!({ "limit": 1000 }));
    events.as_array().expect("a list of events").iter().filter(|e| e["action"] == "code.add").map(|e| e["actor"].as_str().unwrap().to_string()).collect()
}

#[test]
fn the_chosen_name_is_recorded_from_then_on_and_an_empty_one_goes_back_to_the_programs_own() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Researcher 3F2A").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Names" }));
    ok(&mut api, "code.add", json!({ "name": "One" }));
    assert_eq!(ok(&mut api, "history.name", json!({ "name": "  Ada Lovelace " }))["name"], "Ada Lovelace", "the name is trimmed");
    ok(&mut api, "code.add", json!({ "name": "Two" }));
    assert_eq!(ok(&mut api, "history.name", json!({ "name": "" }))["name"], "Researcher 3F2A", "no name chosen: the program's own");
    ok(&mut api, "code.add", json!({ "name": "Three" }));
    assert_eq!(actors_of_code_changes(&mut api), ["Researcher 3F2A", "Ada Lovelace", "Researcher 3F2A"]);

    // A project opened afterwards is worked in under the name too, and nothing asks the computer who is signed in.
    ok(&mut api, "history.name", json!({ "name": "Ada Lovelace" }));
    ok(&mut api, "project.close", json!({}));
    ok(&mut api, "project.open", json!({ "path": dir.path().join("p.pqda"), "backup": false }));
    ok(&mut api, "code.add", json!({ "name": "Four" }));
    assert_eq!(actors_of_code_changes(&mut api).last().map(String::as_str), Some("Ada Lovelace"));
    assert!(!actors_of_code_changes(&mut api).iter().any(|a| a.eq_ignore_ascii_case(&std::env::var("USERNAME").unwrap_or_else(|_| "no such account".to_string()))), "the account name is nowhere in the history");
}

#[test]
fn a_name_that_is_too_long_or_runs_over_two_lines_is_refused() {
    let mut api = Api::new("Researcher 3F2A").without_backup_on_open();
    for bad in ["x".repeat(61), "two\nlines".to_string(), "tab\there".to_string()] {
        assert_eq!(api.call("history.name", json!({ "name": bad })).unwrap_err().code, "invalid");
    }
    assert!(api.call("history.name", json!({ "name": "x".repeat(60) })).is_ok());
}
