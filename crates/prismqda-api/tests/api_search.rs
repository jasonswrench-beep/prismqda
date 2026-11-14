//! Search, autocoding with review, and text editing through the command layer.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

/// A project with one source of three paragraphs and one code.
fn setup(dir: &TempDir) -> (Api, i64, i64) {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let path = dir.path().join("t.txt");
    std::fs::write(&path, "The cost was high. We stayed anyway.\n\nAccess was easy. No problem at all.\n\nThe cost of access is a worry. It keeps coming up.").unwrap();
    let out = ok(&mut a, "import.run", json!({ "path": path }));
    let source = out["outcome"]["source_ids"][0].as_i64().unwrap();
    let code = ok(&mut a, "code.add", json!({ "name": "Cost" }))["id"].as_i64().unwrap();
    (a, source, code)
}

#[test]
fn searching_returns_hits_with_offsets_and_honors_filters() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = setup(&dir);
    let r = ok(&mut a, "search.run", json!({ "query": "cost" }));
    assert_eq!(r["total"], 2);
    assert_eq!(r["hits"][0]["source_id"], source);
    assert_eq!(r["hits"][0]["spans"], json!([[4, 8]]));
    assert!(r["hits"][0]["snippet"].as_str().unwrap().contains("cost was high"));

    assert_eq!(ok(&mut a, "search.run", json!({ "query": "cost AND access" }))["total"], 1);
    assert_eq!(ok(&mut a, "search.run", json!({ "query": "cost", "limit": 1 }))["truncated"], true);
    assert_eq!(ok(&mut a, "search.run", json!({ "query": "cost", "source_ids": [999] }))["total"], 0);
    assert_eq!(ok(&mut a, "search.run", json!({ "query": "cost", "with_code": code }))["total"], 0);
    assert_eq!(err(&mut a, "search.run", json!({ "query": "(cost" })).code, "invalid");
    assert_eq!(err(&mut a, "search.run", json!({})).code, "bad_args");
}

#[test]
fn autocode_suggests_then_a_person_decides() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = setup(&dir);
    let made = ok(&mut a, "autocode.propose", json!({ "query": "cost", "code_id": code, "target": "sentence" }));
    assert_eq!(made["created"], 2);
    assert_eq!(ok(&mut a, "coding.list", json!({ "source_id": source })).as_array().unwrap().len(), 0, "nothing is coded until a decision");

    let pending = ok(&mut a, "proposal.list", json!({ "status": "pending", "batch": made["batch"] }));
    let rows = pending.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    // The suggestion covers the whole sentence, not just the word.
    assert_eq!((rows[0]["start"].as_i64(), rows[0]["end"].as_i64()), (Some(0), Some(18)));
    assert!(rows[0]["rationale"].as_str().unwrap().contains("autocoded from search: cost"));

    let first = rows[0]["id"].as_i64().unwrap();
    let second = rows[1]["id"].as_i64().unwrap();
    let done = ok(&mut a, "proposal.decide", json!({ "ids": [first], "accept": true }));
    assert_eq!((done["accepted"].as_i64(), done["failed"].as_array().unwrap().len()), (Some(1), 0));
    ok(&mut a, "proposal.decide", json!({ "ids": [second], "accept": false }));
    let codings = ok(&mut a, "coding.list", json!({ "source_id": source }));
    assert_eq!(codings.as_array().unwrap().len(), 1);
    assert_eq!(codings[0]["origin"], "human");
    assert_eq!(ok(&mut a, "proposal.list", json!({ "status": "pending" })).as_array().unwrap().len(), 0);
}

#[test]
fn autocode_targets_cover_the_hit_the_paragraph_or_the_unit() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, code) = setup(&dir);
    let spans = |a: &mut Api, target: &str| -> Vec<(i64, i64)> {
        let made = ok(a, "autocode.propose", json!({ "query": "cost", "code_id": code, "target": target }));
        ok(a, "proposal.list", json!({ "batch": made["batch"] }))
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["start"].as_i64().unwrap(), r["end"].as_i64().unwrap()))
            .collect()
    };
    assert_eq!(spans(&mut a, "hit"), [(4, 8), (79, 83)]);
    assert_eq!(spans(&mut a, "paragraph"), [(0, 36), (75, 125)]);
    assert_eq!(spans(&mut a, "unit").len(), 2, "a source with no units searches by paragraph");
    assert_eq!(err(&mut a, "autocode.propose", json!({ "query": "cost", "code_id": code, "target": "word" })).code, "bad_args");
    assert_eq!(err(&mut a, "autocode.propose", json!({ "query": "zebra", "code_id": code })).code, "invalid");
}

#[test]
fn overlapping_suggestions_for_one_code_are_merged() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, code) = setup(&dir);
    // "cost" and "access" are both in the last paragraph, so a paragraph-wide suggestion covers both once.
    let made = ok(&mut a, "autocode.propose", json!({ "query": "cost OR access", "code_id": code, "target": "paragraph" }));
    assert_eq!(made["created"], 3, "three paragraphs, each once");
}

#[test]
fn a_method_can_be_chosen_and_worked_through() {
    let dir = TempDir::new().unwrap();
    let (mut a, ..) = setup(&dir);
    let list = ok(&mut a, "template.list", Value::Null);
    assert_eq!(list.as_array().unwrap().len(), 6);
    assert_eq!(list[1]["id"], "reflexive_ta");

    let state = ok(&mut a, "template.set", json!({ "id": "reflexive_ta" }));
    assert_eq!(state["current_phase"], "familiarization");
    let state = ok(&mut a, "template.tick", json!({ "phase": "familiarization", "item": 0 }));
    assert_eq!(state["phases"][0]["items"][0]["done"], true);
    let state = ok(&mut a, "template.move", json!({ "phase": "initial_coding" }));
    assert_eq!(state["current_phase"], "initial_coding");
    assert_eq!(ok(&mut a, "template.state", Value::Null)["template"]["name"], "Reflexive thematic analysis");
    assert_eq!(ok(&mut a, "template.check", Value::Null).as_array().unwrap().len(), 0);
    assert_eq!(err(&mut a, "template.set", json!({ "id": "nonsense" })).code, "invalid");
    assert_eq!(err(&mut a, "template.tick", json!({ "phase": "familiarization", "item": 99 })).code, "invalid");

    // A method that codes units cannot be chosen once free-span codings exist.
    let code = ok(&mut a, "code.add", json!({ "name": "C" }))["id"].as_i64().unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": 1, "start": 0, "end": 3, "code_id": code }));
    assert_eq!(err(&mut a, "template.set", json!({ "id": "content_analysis" })).code, "invalid");
}

#[test]
fn undo_reverses_the_newest_change_and_says_what_it_did() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = setup(&dir);
    // The setup imported a source and then added a code; the code is the newest change.
    assert_eq!(ok(&mut a, "undo.next", Value::Null)["next"], "Removed the code you added");
    ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 4, "end": 8, "code_id": code }));
    assert_eq!(ok(&mut a, "undo.next", Value::Null)["next"], "Removed the coding you added");
    let out = ok(&mut a, "undo", Value::Null);
    assert_eq!(out["action"], "coding.add");
    assert_eq!(ok(&mut a, "coding.list", json!({ "source_id": source })).as_array().unwrap().len(), 0);
    ok(&mut a, "undo", Value::Null); // the code the setup added
    let blocked = err(&mut a, "undo", Value::Null);
    assert_eq!(blocked.code, "invalid");
    assert!(blocked.message.contains("too large to undo"), "{}", blocked.message);
}

#[test]
fn editing_a_source_moves_its_codings_and_reports_what_happened() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = setup(&dir);
    let coded = ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 4, "end": 8, "code_id": code }));
    assert_eq!(coded["quote"], "cost");
    let text = ok(&mut a, "source.get", json!({ "id": source }))["text"].as_str().unwrap().to_string();
    let edited = text.replacen("The cost was high.", "Honestly, the cost was high.", 1);
    let out = ok(&mut a, "source.edit", json!({ "id": source, "text": edited }));
    assert_eq!((out["text_revision"].as_i64(), out["codings_removed"].as_i64()), (Some(2), Some(0)));
    let codings = ok(&mut a, "coding.list", json!({ "source_id": source }));
    let (s, e) = (codings[0]["start"].as_i64().unwrap(), codings[0]["end"].as_i64().unwrap());
    let now = ok(&mut a, "source.get", json!({ "id": source }))["text"].as_str().unwrap().to_string();
    let quote: String = now.chars().skip(s as usize).take((e - s) as usize).collect();
    assert_eq!(quote, "cost");

    // Removing the coded word removes the coding, and says so.
    let gone = now.replacen("the cost", "the price", 1);
    let out = ok(&mut a, "source.edit", json!({ "id": source, "text": gone }));
    assert_eq!(out["codings_removed"], 1);
    assert_eq!(err(&mut a, "source.edit", json!({ "id": source, "text": "" })).code, "invalid");
}
