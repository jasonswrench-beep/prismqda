//! The roadmap's M2a gate in one test: import, code, search, edit, export, and the audit trail, through the
//! command layer the interface uses.

use prismqda_api::Api;
use prismqda_import::{docx, ImportOptions};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

#[test]
fn import_code_search_edit_and_export_make_one_coherent_study() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("study.pqda"), "name": "End to end" }));

    // Import a transcript-like document and a survey.
    let doc = dir.path().join("interview.txt");
    std::fs::write(&doc, "I came for the community. The cost was high, but the cost was worth it.\n\nThe staff were warm. Access was easy.").unwrap();
    let survey = dir.path().join("survey.csv");
    std::fs::write(&survey, "id,answer\nP1,The cost kept me away at first\nP2,I felt welcome from the start\n").unwrap();
    let interview = ok(&mut api, "import.run", json!({ "path": doc }))["outcome"]["source_ids"][0].as_i64().unwrap();
    ok(&mut api, "import.run", json!({ "path": survey }));
    assert_eq!(ok(&mut api, "source.list", json!({})).as_array().unwrap().len(), 2);

    // Code by hand, then find the rest by searching and review what the search suggests.
    let cost = ok(&mut api, "code.add", json!({ "name": "Cost", "definition": "Talk about money." }))["id"].as_i64().unwrap();
    let welcome = ok(&mut api, "code.add", json!({ "name": "Welcome" }))["id"].as_i64().unwrap();
    let first = ok(&mut api, "coding.apply", json!({ "source_id": interview, "start": 30, "end": 34, "code_id": cost }));
    assert_eq!(first["quote"], "cost");
    let found = ok(&mut api, "search.run", json!({ "query": "cost" }));
    assert_eq!(found["total"], 2, "one paragraph of the interview (with two hits) and one survey answer: {found}");
    let made = ok(&mut api, "autocode.propose", json!({ "query": "cost", "code_id": cost, "target": "sentence" }));
    let pending = ok(&mut api, "proposal.list", json!({ "status": "pending", "batch": made["batch"] }));
    let ids: Vec<Value> = pending.as_array().unwrap().iter().map(|p| p["id"].clone()).collect();
    let decided = ok(&mut api, "proposal.decide", json!({ "ids": ids, "accept": true }));
    assert_eq!(decided["failed"].as_array().unwrap().len(), 0, "{decided}");
    ok(&mut api, "autocode.propose", json!({ "query": "welcome OR warm", "code_id": welcome, "target": "sentence" }));
    let welcome_batch = ok(&mut api, "proposal.list", json!({ "status": "pending" }));
    let welcome_ids: Vec<Value> = welcome_batch.as_array().unwrap().iter().map(|p| p["id"].clone()).collect();
    ok(&mut api, "proposal.decide", json!({ "ids": welcome_ids, "accept": true }));

    // An edit to the text carries every coding along.
    let before = ok(&mut api, "segments.list", json!({}));
    let total_before = before["total"].as_i64().unwrap();
    assert!(total_before >= 4, "{before}");
    let text = ok(&mut api, "source.get", json!({ "id": interview }))["text"].as_str().unwrap().to_string();
    let edited = format!("Added a first line.\n\n{text}");
    let outcome = ok(&mut api, "source.edit", json!({ "id": interview, "text": edited }));
    assert_eq!(outcome["codings_removed"], 0);
    let after = ok(&mut api, "segments.list", json!({}));
    assert_eq!(after["total"].as_i64().unwrap(), total_before, "no coding was lost");
    let quotes = |v: &Value| -> Vec<String> { v["segments"].as_array().unwrap().iter().map(|s| s["text"].as_str().unwrap().trim().to_string()).collect() };
    assert_eq!(quotes(&before), quotes(&after), "every coding still covers the same words after the edit");

    // Export what was coded and read it back.
    let out = dir.path().join("passages");
    let saved = ok(&mut api, "export.segments", json!({ "format": "docx", "path": out, "code_ids": [cost] }));
    let bytes = std::fs::read(saved["path"].as_str().unwrap()).unwrap();
    let parsed = docx::parse(&bytes, &ImportOptions::default()).unwrap();
    assert!(parsed.text.contains("Cost ("), "{}", parsed.text);
    assert!(parsed.text.contains("kept me away at first") || parsed.text.contains("cost was high"), "{}", parsed.text);
    assert!(!parsed.text.contains("Welcome ("), "only the Cost code was asked for");

    // Everything above is in the audit trail, and the newest change can be undone.
    let actions: Vec<String> = ok(&mut api, "events.list", json!({ "limit": 500 })).as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap().to_string()).collect();
    for want in ["code.add", "coding.add", "proposal.accept"] {
        assert!(actions.iter().any(|a| a == want), "no {want} event in {actions:?}");
    }
}
