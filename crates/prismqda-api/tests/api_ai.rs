//! AI through prompt packets, by way of the command layer: building a packet, pasting an answer back, the log of runs,
//! the settings that keep documents and codes out of a packet, and the paragraph for a methods section.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

const DOC: &str = "Fares are too high for me.\n\nThe stop is far from my house.\n\nI love the new buses.";

fn setup(dir: &TempDir) -> (Api, i64, i64, i64) {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let path = dir.path().join("riders.txt");
    std::fs::write(&path, DOC).unwrap();
    let source = ok(&mut a, "import.run", json!({ "path": path }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let cost = ok(&mut a, "code.add", json!({ "name": "Cost", "definition": "Talk about money." }))["id"].as_i64().unwrap();
    let access = ok(&mut a, "code.add", json!({ "name": "Access" }))["id"].as_i64().unwrap();
    (a, source, cost, access)
}

#[test]
fn a_packet_is_built_for_the_person_using_the_project_and_nothing_is_coded() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, _, _) = setup(&dir);
    let packet = ok(&mut a, "ai.packet", json!({}));
    assert_eq!(packet["passages"], 3);
    assert_eq!(packet["codes"].as_array().unwrap().len(), 2);
    assert!(packet["prompt"].as_str().unwrap().contains("[P1] Fares are too high for me."));
    assert_eq!(packet["more_after"], 0);
    assert_eq!(ok(&mut a, "coding.list", json!({ "source_id": source })), json!([]), "building a packet codes nothing");
    assert_eq!(ok(&mut a, "proposal.list", json!({})), json!([]));
    let runs = ok(&mut a, "ai.runs", json!({}));
    assert_eq!(runs.as_array().unwrap().len(), 1);
    assert_eq!(runs[0]["answered"], false);
    assert_eq!(runs[0]["route"], "packet");
}

#[test]
fn a_pasted_answer_goes_through_the_checker_to_the_review_queue_and_the_run_keeps_the_record() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, cost, _) = setup(&dir);
    let run_id = ok(&mut a, "ai.packet", json!({}))["run_id"].as_i64().unwrap();
    let answer = r#"[{"passage": "P1", "code": "Cost", "quote": "too high", "reason": "fares"}, {"passage": "P2", "code": "Cost", "quote": "not in the passage"}]"#;
    let outcome = ok(&mut a, "ai.answer", json!({ "run_id": run_id, "model": "Test Bot", "answer": answer }));
    assert_eq!((outcome["read"].as_i64(), outcome["suggested"].as_i64()), (Some(2), Some(1)));
    assert_eq!(outcome["rejected"][0]["number"], 2);

    let queue = ok(&mut a, "proposal.list", json!({ "status": "pending" }));
    assert_eq!(queue.as_array().unwrap().len(), 1);
    assert_eq!((queue[0]["origin"].as_str(), queue[0]["text"].as_str(), queue[0]["ai_run_id"].as_i64()), (Some("ai"), Some("too high"), Some(run_id)));
    let batches = ok(&mut a, "proposal.batches", json!({}));
    assert_eq!(batches[0]["origin"], "ai");
    assert_eq!(batches[0]["method"], "ai");

    let id = queue[0]["id"].as_i64().unwrap();
    ok(&mut a, "proposal.decide", json!({ "ids": [id], "accept": true }));
    let codings = ok(&mut a, "coding.list", json!({ "source_id": source }));
    assert_eq!((codings[0]["origin"].as_str(), codings[0]["code_id"].as_i64(), codings[0]["ai_run_id"].as_i64()), (Some("ai_suggestion_accepted"), Some(cost), Some(run_id)));

    let run = ok(&mut a, "ai.run", json!({ "id": run_id }));
    assert_eq!((run["model"].as_str(), run["accepted"].as_i64(), run["checker_rejected"].as_i64()), (Some("Test Bot"), Some(1), Some(1)));
    assert_eq!(run["raw_output"], answer);
    assert!(run["prompt"].as_str().unwrap().contains("CODEBOOK"));
    assert_eq!(run["checker_rejections"][0]["number"], 2);

    // The same answer cannot be pasted twice.
    assert_eq!(err(&mut a, "ai.answer", json!({ "run_id": run_id, "model": "Test Bot", "answer": answer })).code, "invalid");
    assert_eq!(err(&mut a, "ai.run", json!({ "id": 999 })).code, "not_found");
    assert_eq!(err(&mut a, "ai.answer", json!({ "run_id": run_id })).code, "bad_args");
}

#[test]
fn a_document_or_a_code_marked_private_never_goes_in_a_packet() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, cost, access) = setup(&dir);
    ok(&mut a, "source.set_do_not_share", json!({ "id": source, "do_not_share": true }));
    assert!(ok(&mut a, "source.list", json!({}))[0]["do_not_share"].as_bool().unwrap());
    let e = err(&mut a, "ai.packet", json!({}));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("no passage to send"), "{}", e.message);
    ok(&mut a, "source.set_do_not_share", json!({ "id": source, "do_not_share": false }));

    ok(&mut a, "code.update", json!({ "id": access, "ai_suggestible": false }));
    let codes = ok(&mut a, "code.list", json!({}));
    let found = codes.as_array().unwrap().iter().find(|c| c["id"] == json!(access)).unwrap();
    assert_eq!(found["ai_suggestible"], false);
    let packet = ok(&mut a, "ai.packet", json!({}));
    assert_eq!(packet["codes"], json!([{ "id": cost, "name": "Cost" }]));
    assert_eq!(packet["left_out"]["codes"], json!(["Access"]));
    assert!(!packet["prompt"].as_str().unwrap().contains("Access"));

    // Asking for a code by number does not get round the setting.
    ok(&mut a, "code.update", json!({ "id": cost, "ai_suggestible": false }));
    let e = err(&mut a, "ai.packet", json!({ "code_ids": [cost] }));
    assert!(e.message.contains("no code that can be sent"), "{}", e.message);
}

#[test]
fn ai_can_be_switched_off_for_a_project_and_the_log_and_paragraph_follow_the_runs() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, _, _) = setup(&dir);
    assert_eq!(ok(&mut a, "project.info", json!({}))["ai_mode"], "packet");
    assert_eq!(ok(&mut a, "ai.disclosure", json!({}))["text"], "No AI was used in this project.");

    let run_id = ok(&mut a, "ai.packet", json!({}))["run_id"].as_i64().unwrap();
    ok(&mut a, "ai.answer", json!({ "run_id": run_id, "model": "Test Bot", "answer": "[]" }));
    let text = ok(&mut a, "ai.disclosure", json!({}))["text"].as_str().unwrap().to_string();
    assert!(text.contains("1 packet was built") && text.contains("(by Test Bot)"), "{text}");

    assert_eq!(ok(&mut a, "ai.set_mode", json!({ "mode": "off" }))["ai_mode"], "off");
    assert!(err(&mut a, "ai.packet", json!({})).message.contains("AI is off"));
    assert_eq!(err(&mut a, "ai.set_mode", json!({ "mode": "cloud" })).code, "invalid");
    assert_eq!(ok(&mut a, "ai.runs", json!({})).as_array().unwrap().len(), 1, "switching off keeps the record");
}

#[test]
fn a_project_that_codes_whole_units_gets_whole_unit_suggestions_that_can_be_accepted() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S", "coding_mode": "unit" }));
    let path = dir.path().join("talk.txt");
    std::fs::write(&path, "Priya: Fares are too high for me.\nSam: I walk to work most days.\nPriya: The bus is late.\nSam: I like the exercise.\n").unwrap();
    ok(&mut a, "import.run", json!({ "path": path, "source_type": "transcript" }));
    ok(&mut a, "code.add", json!({ "name": "Cost" }));

    let packet = ok(&mut a, "ai.packet", json!({}));
    assert_eq!(packet["passages"], 4, "each turn is a passage: {packet}");
    let answer = r#"[{"passage": "P1", "code": "Cost", "quote": "too high", "reason": "fares"}]"#;
    let outcome = ok(&mut a, "ai.answer", json!({ "run_id": packet["run_id"], "model": "Test Bot", "answer": answer }));
    assert_eq!(outcome["suggested"], 1, "{outcome}");
    let queue = ok(&mut a, "proposal.list", json!({ "status": "pending" }));
    let id = queue[0]["id"].as_i64().unwrap();
    assert!(queue[0]["text"].as_str().unwrap().contains("Fares are too high for me."), "the whole unit, not just the quoted words: {}", queue[0]["text"]);
    let decided = ok(&mut a, "proposal.decide", json!({ "ids": [id], "accept": true }));
    assert_eq!((decided["accepted"].as_i64(), decided["failed"].as_array().map(Vec::len)), (Some(1), Some(0)), "{decided}");
}
