//! The offline lock and the ethics summary (decision 165), through the command layer: the lock stops a prompt packet and is undone by Undo,
//! the facts say what the project says about itself, and the report section states them without repeating a name that is to be replaced.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn study(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("study.pqda"), "name": "Study" }));
    let one = dir.path().join("interview-with-sarah.txt");
    std::fs::write(&one, "Sarah Jones said the market was friendly. She would go back.").unwrap();
    ok(&mut a, "import.run", json!({ "path": one }));
    ok(&mut a, "code.add", json!({ "name": "Kindness" }));
    a
}

#[test]
fn the_offline_lock_stops_a_prompt_packet_and_undo_takes_the_lock_back() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    assert_eq!(ok(&mut a, "project.info", json!({}))["offline_lock"], false, "off by default");
    let info = ok(&mut a, "privacy.set_offline_lock", json!({ "on": true }));
    assert_eq!(info["offline_lock"], true);
    let refused = err(&mut a, "ai.packet", json!({}));
    assert_eq!(refused.code, "invalid");
    assert!(refused.message.contains("offline lock is on"), "{}", refused.message);
    assert!(refused.message.contains("no prompt packet"), "{}", refused.message);
    assert_eq!(ok(&mut a, "ai.runs", json!({})).as_array().unwrap().len(), 0, "a refused packet leaves no run behind");
    // the lock is a switch of the project: it is there after the project is closed and opened again
    ok(&mut a, "project.close", json!({}));
    ok(&mut a, "project.open", json!({ "path": dir.path().join("study.pqda") }));
    assert_eq!(ok(&mut a, "project.info", json!({}))["offline_lock"], true);
    // off again, and a packet is built
    ok(&mut a, "privacy.set_offline_lock", json!({ "on": false }));
    assert!(ok(&mut a, "ai.packet", json!({}))["passages"].as_i64().unwrap() >= 1);
    // turning it on twice, or off when it is off, changes nothing and adds no entry to the log
    let before = ok(&mut a, "events.list", json!({ "limit": 500 })).as_array().unwrap().len();
    ok(&mut a, "privacy.set_offline_lock", json!({ "on": false }));
    assert_eq!(ok(&mut a, "events.list", json!({ "limit": 500 })).as_array().unwrap().len(), before);
    // Undo
    ok(&mut a, "privacy.set_offline_lock", json!({ "on": true }));
    ok(&mut a, "undo", json!({}));
    assert_eq!(ok(&mut a, "project.info", json!({}))["offline_lock"], false, "Undo took the lock back");
}

#[test]
fn the_facts_say_what_the_project_says_about_itself() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let sources = ok(&mut a, "source.list", json!({}));
    let id = sources[0]["id"].as_i64().unwrap();
    ok(&mut a, "source.set_do_not_share", json!({ "id": id, "do_not_share": true }));
    ok(&mut a, "privacy.set_offline_lock", json!({ "on": true }));
    let f = ok(&mut a, "privacy.facts", json!({}));
    assert_eq!(f["project_name"], "Study");
    assert_eq!((f["documents"].as_i64(), f["codes"].as_i64()), (Some(1), Some(1)));
    assert_eq!(f["offline_lock"], true);
    assert_eq!(f["encrypted"], false);
    assert_eq!(f["not_shared"], json!([{ "kind": "document", "name": "interview-with-sarah" }]));
    assert_eq!(f["ai_runs"], json!([]));
    assert!(f["events"].as_i64().unwrap() >= 4);
    assert!(f["project_path"].as_str().unwrap().ends_with("study.pqda"));
    assert!(f["coders"].is_array());
}

#[test]
fn the_ethics_section_of_the_report_states_the_facts_and_does_not_repeat_a_name_that_is_to_be_replaced() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let sources = ok(&mut a, "source.list", json!({}));
    ok(&mut a, "source.set_do_not_share", json!({ "id": sources[0]["id"], "do_not_share": true }));
    ok(&mut a, "privacy.set_offline_lock", json!({ "on": true }));
    ok(&mut a, "deid.add", json!({ "terms": ["Sarah"], "replacement": "[name]" }));
    let out = ok(&mut a, "export.report", json!({ "path": dir.path().join("ethics"), "sections": ["ethics"] }));
    let docx = std::fs::read(out["path"].as_str().unwrap()).unwrap();
    let text = docx_text(&docx);
    assert!(text.contains("Ethics and data handling"), "{text}");
    assert!(text.contains("Encrypted at rest") && text.contains("No. The file is not encrypted"), "{text}");
    assert!(text.contains("Offline lock") && text.contains("On. While it is on"), "{text}");
    assert!(text.contains("No prompt packet was ever built"), "{text}");
    assert!(text.contains("A list of 1 name or term to replace is kept"), "{text}");
    assert!(text.contains("not applied to what is exported") || text.contains("not applied to"), "{text}");
    assert!(text.contains("do not share"), "{text}");
    assert!(text.contains("interview-with-sarah"), "the document is named while the list is not applied to exports: {text}");
    // once exports apply the list, the document's name in the summary is replaced too
    ok(&mut a, "deid.set_exports", json!({ "on": true }));
    let again = ok(&mut a, "export.report", json!({ "path": dir.path().join("ethics2"), "sections": ["ethics"] }));
    let text2 = docx_text(&std::fs::read(again["path"].as_str().unwrap()).unwrap());
    assert!(text2.contains("interview-with-[name]"), "{text2}");
    assert!(!text2.to_lowercase().contains("sarah"), "the word to replace is not in the summary, even in a document's name: {text2}");
}

/// The words of a .docx, joined, enough to look for sentences.
fn docx_text(bytes: &[u8]) -> String {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    // drop the tags, keep the text
    let mut out = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
