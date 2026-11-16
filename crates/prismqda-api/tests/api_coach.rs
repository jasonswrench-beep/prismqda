//! Coach mode through the command layer (decision 199): questions about the coding of the person using the project, and the prompt to take to a chatbot.

use prismqda_api::Api;
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

const TEXT: &str = "The fares are too high and the bus is late. I love the bus but the fares are rising.";

fn code_words(api: &mut Api, needle: &str, which: usize, code: &str) {
    let source = ok(api, "source.list", json!({}))[0]["id"].as_i64().unwrap();
    let id = ok(api, "code.list", json!({})).as_array().unwrap().iter().find(|c| c["name"] == code).unwrap()["id"].as_i64().unwrap();
    let byte = TEXT.match_indices(needle).nth(which).unwrap().0;
    let start = TEXT[..byte].chars().count();
    ok(api, "coding.apply", json!({ "source_id": source, "start": start, "end": start + needle.chars().count(), "code_id": id }));
}

#[test]
fn the_coach_looks_at_the_coding_of_the_person_using_the_project_and_makes_a_prompt() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Student").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("Coach.pqda"), "name": "Coach" }));
    let file = dir.path().join("Riders.txt");
    std::fs::write(&file, TEXT).unwrap();
    ok(&mut api, "import.run", json!({ "path": file }));
    ok(&mut api, "code.add", json!({ "name": "Cost" }));
    code_words(&mut api, "fares", 0, "Cost");

    let report = ok(&mut api, "coach.check", json!({}));
    assert_eq!(report["coder"]["name"], "Student");
    assert_eq!(report["codings"], 1);
    let rules: Vec<(String, i64)> = report["rules"].as_array().unwrap().iter().map(|r| (r["rule"].as_str().unwrap().to_string(), r["findings"].as_i64().unwrap())).collect();
    assert_eq!(rules, [("no_definition".to_string(), 1), ("used_once".to_string(), 0), ("overlap".to_string(), 0), ("inconsistent".to_string(), 1)]);
    let findings = report["findings"].as_array().unwrap();
    assert_eq!(findings[0]["rule"], "no_definition");
    assert_eq!(findings[1]["rule"], "inconsistent");
    assert_eq!(findings[1]["phrase"], "fares");
    assert_eq!(findings[1]["count"], 1);

    let prompt = ok(&mut api, "coach.prompt", json!({}));
    assert!(prompt["text"].as_str().unwrap().contains("\"Cost\" has no definition"), "{prompt}");
    assert_eq!(prompt["findings"], 2);

    // A coder who is not there, and a switched-off assistant.
    assert_eq!(api.call("coach.check", json!({ "coder_id": 987654 })).unwrap_err().code, "not_found");
    ok(&mut api, "ai.set_mode", json!({ "mode": "off" }));
    assert_eq!(api.call("coach.prompt", json!({})).unwrap_err().code, "invalid");
    assert_eq!(ok(&mut api, "coach.check", json!({}))["codings"], 1, "the checks need no assistant");
}

#[test]
fn an_assignments_settings_travel_in_the_packet_and_the_answer_key_is_a_file_of_its_own() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Teacher").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("Exercise.pqda"), "name": "Exercise" }));
    let file = dir.path().join("Riders.txt");
    std::fs::write(&file, TEXT).unwrap();
    ok(&mut api, "import.run", json!({ "path": file }));
    ok(&mut api, "code.add", json!({ "name": "Cost" }));
    code_words(&mut api, "fares", 0, "Cost");

    // Settings that make no sense are refused before any packet is made.
    let refused = api.call("class.packets", json!({ "folder": dir.path().join("nope"), "students": ["Eve"], "assignment": { "closed": ["code"] } })).unwrap_err();
    assert_eq!(refused.code, "invalid", "{refused}");
    let folder = dir.path().join("packets");
    let made = ok(&mut api, "class.packets", json!({ "folder": folder, "students": ["Dina"], "assignment": { "title": "Week 3", "ai_allowed": false, "coach_locked": true, "closed": ["explore"] } }));
    assert_eq!(made["made"], 1);

    // The student opens the packet: the settings are there, AI stays closed, and the coach works for the person the packet is for.
    let mut theirs = Api::new("Student").without_backup_on_open();
    ok(&mut theirs, "project.open", json!({ "path": folder.join("Exercise - packet for Dina.pqda") }));
    let info = ok(&mut theirs, "project.info", json!({}));
    assert_eq!(info["packet"]["assignment"], json!({ "title": "Week 3", "ai_allowed": false, "coach_locked": true, "closed": ["explore"] }));
    assert_eq!(info["ai_mode"], "off");
    assert_eq!(theirs.call("ai.set_mode", json!({ "mode": "packet" })).unwrap_err().code, "invalid");
    assert_eq!(ok(&mut theirs, "coach.check", json!({}))["coder"]["name"], "Dina");
    ok(&mut theirs, "project.close", json!({}));

    // The answer key: a file of its own, made once, holding the instructor's coding.
    let key = ok(&mut api, "export.class_key", json!({ "path": dir.path().join("keys").join("Exercise key") }));
    assert_eq!(key["coder"], "Teacher");
    assert_eq!(key["codings"], 1);
    assert!(dir.path().join("keys").join("Exercise key.pqda").is_file(), "the extension is added");
    assert_eq!(api.call("export.class_key", json!({ "path": dir.path().join("keys").join("Exercise key.pqda") })).unwrap_err().code, "already_exists");
    assert_eq!(api.call("export.class_key", json!({ "path": "" })).unwrap_err().code, "invalid");
}
