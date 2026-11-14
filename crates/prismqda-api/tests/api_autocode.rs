//! Auto Code through the command layer: previews, the three ways of finding passages (words, speakers, headings),
//! the review queue (paging, deciding by batch, adjusting), and taking a decision back with one undo.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn create(dir: &TempDir, mode: Option<&str>) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    let mut args = json!({ "path": dir.path().join("p.pqda"), "name": "S" });
    if let Some(m) = mode {
        args["coding_mode"] = json!(m);
    }
    ok(&mut a, "project.create", args);
    a
}

fn import(api: &mut Api, dir: &TempDir, file: &str, text: &str, extra: Value) -> i64 {
    let path = dir.path().join(file);
    std::fs::write(&path, text).unwrap();
    let mut args = json!({ "path": path });
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        args[k] = v;
    }
    ok(api, "import.run", args)["outcome"]["source_ids"][0].as_i64().unwrap()
}

const DOC: &str = "The cost was high. We stayed anyway.\n\nAccess was easy. No problem at all.\n\nThe cost of access is a worry. It keeps coming up.";

fn doc_project(dir: &TempDir) -> (Api, i64, i64) {
    let mut a = create(dir, None);
    let source = import(&mut a, dir, "t.txt", DOC, json!({}));
    let code = ok(&mut a, "code.add", json!({ "name": "Cost" }))["id"].as_i64().unwrap();
    (a, source, code)
}

#[test]
fn a_preview_counts_and_shows_examples_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, code) = doc_project(&dir);
    let p = ok(&mut a, "autocode.preview", json!({ "query": "cost", "code_id": code, "target": "sentence" }));
    assert_eq!((p["method"].as_str(), p["suggestions"].as_i64(), p["found"].as_i64(), p["documents"].as_i64()), (Some("words"), Some(2), Some(2), Some(1)));
    assert_eq!(p["label"], "Words: cost");
    let first = &p["examples"][0];
    assert_eq!(first["text"], "The cost was high.");
    assert_eq!(first["code_name"], "Cost");
    assert_eq!(first["before"], "");
    assert!(first["after"].as_str().unwrap().starts_with(" We stayed anyway."));
    assert_eq!(p["codes"], json!([{ "code_id": code, "name": "Cost", "new": false, "suggestions": 2 }]));
    assert_eq!(ok(&mut a, "proposal.batches", json!({})), json!([]), "a preview leaves nothing in the queue");
    assert_eq!(ok(&mut a, "proposal.list", json!({})), json!([]));
}

#[test]
fn the_number_of_words_around_a_match_widens_it_inside_its_paragraph() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, code) = doc_project(&dir);
    let p = ok(&mut a, "autocode.preview", json!({ "query": "access", "code_id": code, "target": "words", "around": 2 }));
    // "access" is in two paragraphs; two words each side, never past the start or end of its paragraph.
    let texts: Vec<&str> = p["examples"].as_array().unwrap().iter().map(|e| e["text"].as_str().unwrap()).collect();
    assert_eq!(texts, ["Access was easy", "cost of access is a"], "{p}");
    assert_eq!(err(&mut a, "autocode.preview", json!({ "query": "access", "code_id": code, "target": "words", "around": 0 })).code, "bad_args");
    assert_eq!(err(&mut a, "autocode.preview", json!({ "query": "access", "code_id": code, "target": "words", "around": 99 })).code, "bad_args");
}

#[test]
fn running_the_same_search_again_finds_nothing_new() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = doc_project(&dir);
    let args = json!({ "query": "cost", "code_id": code, "target": "sentence" });
    let made = ok(&mut a, "autocode.propose", args.clone());
    assert_eq!((made["created"].as_i64(), made["already_waiting"].as_i64(), made["already_coded"].as_i64()), (Some(2), Some(0), Some(0)));

    // The same suggestions already wait.
    let again = ok(&mut a, "autocode.preview", args.clone());
    assert_eq!((again["suggestions"].as_i64(), again["already_waiting"].as_i64()), (Some(0), Some(2)));
    assert!(err(&mut a, "autocode.propose", args.clone()).message.contains("already waiting"));

    // After they are accepted, they are already coded.
    ok(&mut a, "proposal.decide", json!({ "batch": made["batch"], "accept": true }));
    assert_eq!(ok(&mut a, "coding.list", json!({ "source_id": source })).as_array().unwrap().len(), 2);
    let after = ok(&mut a, "autocode.preview", args.clone());
    assert_eq!((after["suggestions"].as_i64(), after["already_coded"].as_i64()), (Some(0), Some(2)));
    assert!(err(&mut a, "autocode.propose", args).message.contains("already coded"));
    // A search that finds nothing is its own message.
    assert!(err(&mut a, "autocode.propose", json!({ "query": "zebra", "code_id": code })).message.contains("found nothing"));
}

#[test]
fn a_new_code_is_made_when_the_run_is_sent_and_a_code_that_is_gone_is_refused() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, code) = doc_project(&dir);
    let p = ok(&mut a, "autocode.preview", json!({ "query": "access", "new_code": "Access" }));
    assert_eq!(p["codes"], json!([{ "code_id": null, "name": "Access", "new": true, "suggestions": 2 }]));
    assert_eq!(ok(&mut a, "code.list", json!({})).as_array().unwrap().len(), 1, "previewing makes no code");

    let made = ok(&mut a, "autocode.propose", json!({ "query": "access", "new_code": "Access" }));
    let created = made["codes_created"].as_array().unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0]["name"], "Access");
    let codes = ok(&mut a, "code.list", json!({}));
    let access = codes.as_array().unwrap().iter().find(|c| c["name"] == "Access").unwrap();
    assert!(access["color"].as_str().unwrap().starts_with('#') && access["color"] != "#808080", "the new code gets one of the interface's colors");

    // Naming a code that already exists uses it, in any case.
    let again = ok(&mut a, "autocode.preview", json!({ "query": "cost", "new_code": "cost" }));
    assert_eq!(again["codes"][0]["code_id"], code);
    assert_eq!(again["codes"][0]["new"], false);

    ok(&mut a, "code.retire", json!({ "id": code }));
    assert_eq!(err(&mut a, "autocode.preview", json!({ "query": "cost", "code_id": code })).code, "invalid");
    assert_eq!(err(&mut a, "autocode.preview", json!({ "query": "cost" })).code, "bad_args", "a code is needed");
    assert_eq!(err(&mut a, "autocode.preview", json!({ "method": "tea", "code_id": code })).code, "bad_args");
}

// ---- speakers ----

const TRANSCRIPT: &str = "Interviewer: How do you get to work?\nPriya: I take the train most days.\nINTERVIEWER: And the cost?\npriya: It costs a lot.\nSam: I walk.\n";

fn transcript_project(dir: &TempDir) -> (Api, i64) {
    let mut a = create(dir, None);
    let source = import(&mut a, dir, "talk.txt", TRANSCRIPT, json!({ "source_type": "transcript", "interviewers": ["Interviewer"] }));
    (a, source)
}

#[test]
fn speakers_are_listed_once_each_and_their_turns_become_suggestions() {
    let dir = TempDir::new().unwrap();
    let (mut a, source) = transcript_project(&dir);
    let listed = ok(&mut a, "autocode.speakers", json!({}));
    let rows: Vec<(String, i64, i64)> = listed.as_array().unwrap().iter().map(|g| (g["key"].as_str().unwrap().to_string(), g["turns"].as_i64().unwrap(), g["left_out"].as_i64().unwrap())).collect();
    assert_eq!(rows, [("interviewer".into(), 0, 2), ("priya".into(), 2, 0), ("sam".into(), 1, 0)]);
    assert_eq!(listed[1]["variants"], json!(["Priya", "priya"]));

    let preview = ok(&mut a, "autocode.preview", json!({ "method": "speakers", "speakers": [{ "key": "priya", "new_code": "Priya" }, { "key": "sam", "new_code": "Sam" }] }));
    assert_eq!((preview["suggestions"].as_i64(), preview["documents"].as_i64(), preview["left_out"].as_i64()), (Some(3), Some(1), Some(0)));
    assert_eq!(preview["label"], "Speakers: Priya, Sam");
    assert_eq!(preview["examples"][0]["text"], "I take the train most days.");

    let made = ok(&mut a, "autocode.propose", json!({ "method": "speakers", "speakers": [{ "key": "Priya:", "new_code": "Priya" }] }));
    assert_eq!(made["created"], 2);
    assert_eq!(made["codes_created"].as_array().unwrap().len(), 1);
    let queue = ok(&mut a, "proposal.list", json!({ "batch": made["batch"] }));
    assert_eq!(queue[0]["text"], "I take the train most days.");
    assert!(queue[0]["unit_id"].is_i64(), "a turn is tied to its unit");
    assert!(queue[0]["rationale"].as_str().unwrap().starts_with("autocoded from speaker: "));
    let batches = ok(&mut a, "proposal.batches", json!({}));
    assert_eq!((batches[0]["method"].as_str(), batches[0]["pending"].as_i64()), (Some("speakers"), Some(2)));

    // Priya's code exists now, so a second run names it instead of failing, and finds nothing new while the first waits.
    let second = ok(&mut a, "autocode.preview", json!({ "method": "speakers", "speakers": [{ "key": "priya", "new_code": "priya" }] }));
    assert_eq!((second["suggestions"].as_i64(), second["already_waiting"].as_i64()), (Some(0), Some(2)));

    // The interviewer is left out of coding, so choosing the interviewer finds no turns and says why.
    let none = ok(&mut a, "autocode.preview", json!({ "method": "speakers", "speakers": [{ "key": "interviewer", "new_code": "Question" }] }));
    assert_eq!((none["suggestions"].as_i64(), none["left_out"].as_i64()), (Some(0), Some(2)));

    ok(&mut a, "proposal.decide", json!({ "batch": made["batch"], "accept": true }));
    let codings = ok(&mut a, "coding.list", json!({ "source_id": source }));
    assert_eq!(codings.as_array().unwrap().len(), 2);
    assert_eq!(err(&mut a, "autocode.propose", json!({ "method": "speakers", "speakers": [] })).code, "bad_args");
}

// ---- headings ----

const REPORT: &str = "# Background\n\nPeople ride the bus.\n\n# Findings\n\nThe cost is high.\n\n## Access\n\nStops are far apart.\n\n# Next steps\n\nAsk again.\n";

#[test]
fn sections_under_headings_become_suggestions() {
    let dir = TempDir::new().unwrap();
    let mut a = create(&dir, None);
    let source = import(&mut a, &dir, "report.md", REPORT, json!({}));
    let levels = ok(&mut a, "autocode.headings", json!({}));
    assert_eq!(levels["levels"].as_array().unwrap().iter().map(|l| (l["level"].as_i64().unwrap(), l["count"].as_i64().unwrap())).collect::<Vec<_>>(), [(1, 3), (2, 1)]);

    let p = ok(&mut a, "autocode.preview", json!({ "method": "headings", "levels": [1], "heading_query": "findings", "new_code": "Findings section" }));
    assert_eq!((p["suggestions"].as_i64(), p["label"].as_str()), (Some(1), Some("Headings: level 1 containing findings")));
    let text = p["examples"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("Findings") && text.ends_with("Stops are far apart."), "the section runs through the level-2 heading inside it: {text:?}");

    let made = ok(&mut a, "autocode.propose", json!({ "method": "headings", "levels": [1], "include_heading": false, "new_code": "Body text" }));
    assert_eq!(made["created"], 3);
    let queue = ok(&mut a, "proposal.list", json!({ "batch": made["batch"] }));
    assert_eq!(queue[0]["text"], "People ride the bus.");
    assert!(queue[0]["rationale"].as_str().unwrap().contains("heading: Background"));
    let _ = source;

    assert_eq!(err(&mut a, "autocode.preview", json!({ "method": "headings", "levels": [], "new_code": "X" })).code, "bad_args");
    assert_eq!(err(&mut a, "autocode.preview", json!({ "method": "headings", "levels": [9], "new_code": "X" })).code, "bad_args");
    let none = ok(&mut a, "autocode.preview", json!({ "method": "headings", "levels": [3], "new_code": "X" }));
    assert_eq!(none["suggestions"], 0);
}

#[test]
fn a_project_that_codes_whole_units_only_takes_matches_and_units() {
    let dir = TempDir::new().unwrap();
    let mut a = create(&dir, Some("unit"));
    import(&mut a, &dir, "t.txt", DOC, json!({}));
    let code = ok(&mut a, "code.add", json!({ "name": "Cost" }))["id"].as_i64().unwrap();
    assert_eq!(err(&mut a, "autocode.preview", json!({ "query": "cost", "code_id": code, "target": "paragraph" })).code, "invalid");
    assert_eq!(err(&mut a, "autocode.preview", json!({ "query": "cost", "code_id": code, "target": "words", "around": 3 })).code, "invalid");
    assert_eq!(err(&mut a, "autocode.preview", json!({ "method": "headings", "levels": [1], "code_id": code })).code, "invalid");
    let unit = ok(&mut a, "autocode.preview", json!({ "query": "cost", "code_id": code, "target": "unit" }));
    assert!(unit["suggestions"].as_i64().unwrap() >= 1);
}

// ---- the queue ----

#[test]
fn the_queue_pages_decides_by_batch_adjusts_and_takes_a_decision_back_with_one_undo() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = doc_project(&dir);
    let made = ok(&mut a, "autocode.propose", json!({ "query": "cost OR access", "code_id": code, "target": "sentence" }));
    let batch = made["batch"].clone();
    let total = made["created"].as_i64().unwrap();
    assert!(total >= 3);

    let first_page = ok(&mut a, "proposal.list", json!({ "batch": batch, "limit": 2 }));
    let second_page = ok(&mut a, "proposal.list", json!({ "batch": batch, "limit": 2, "offset": 2 }));
    assert_eq!(first_page.as_array().unwrap().len(), 2);
    assert_ne!(first_page[0]["id"], second_page[0]["id"]);
    assert_eq!(first_page[0]["source_name"], "t");
    assert!(first_page[0]["text"].as_str().unwrap().ends_with('.'));

    // Correct one: move its end back one word and give it another code.
    let other = ok(&mut a, "code.add", json!({ "name": "Other" }))["id"].as_i64().unwrap();
    let id = first_page[0]["id"].as_i64().unwrap();
    let (start, end) = (first_page[0]["start"].as_i64().unwrap(), first_page[0]["end"].as_i64().unwrap());
    let adjusted = ok(&mut a, "proposal.adjust", json!({ "id": id, "start": start, "end": end - 1, "code_id": other }));
    assert_eq!((adjusted["code_id"].as_i64(), adjusted["text"].as_str()), (Some(other), Some("The cost was high")));
    assert_eq!(err(&mut a, "proposal.adjust", json!({ "id": id, "start": 5, "end": 5, "code_id": other })).code, "invalid");

    // Accept the whole batch in one step: every coding is there, with the adjusted one in its corrected form.
    let decided = ok(&mut a, "proposal.decide", json!({ "batch": batch, "accept": true }));
    assert_eq!((decided["accepted"].as_i64(), decided["failed"].as_array().map(Vec::len)), (Some(total), Some(0)));
    let codings = ok(&mut a, "coding.list", json!({ "source_id": source }));
    assert_eq!(codings.as_array().unwrap().len() as i64, total);
    assert!(codings.as_array().unwrap().iter().any(|c| c["code_id"] == other && c["end"].as_i64() == Some(end - 1)));
    let batches = ok(&mut a, "proposal.batches", json!({}));
    assert_eq!((batches[0]["pending"].as_i64(), batches[0]["accepted"].as_i64()), (Some(0), Some(total)));

    // One undo takes the whole decision back, and says how many.
    assert_eq!(ok(&mut a, "undo.next", json!({}))["next"], format!("Took back {total} accepted suggestions"));
    let undone = ok(&mut a, "undo", json!({}));
    assert_eq!(undone["what"], format!("Took back {total} accepted suggestions"));
    assert!(ok(&mut a, "coding.list", json!({ "source_id": source })).as_array().unwrap().is_empty());
    assert_eq!(ok(&mut a, "proposal.batches", json!({}))[0]["pending"].as_i64(), Some(total));

    // Rejecting by batch and undoing it works the same way.
    ok(&mut a, "proposal.decide", json!({ "batch": batch, "accept": false }));
    assert_eq!(ok(&mut a, "proposal.list", json!({ "status": "pending" })).as_array().unwrap().len(), 0);
    ok(&mut a, "undo", json!({}));
    assert_eq!(ok(&mut a, "proposal.list", json!({ "status": "pending" })).as_array().unwrap().len() as i64, total);
    assert_eq!(err(&mut a, "proposal.decide", json!({ "accept": true, "ids": [999999] })).code, "not_found");
}

#[test]
fn text_edits_reject_waiting_suggestions_and_the_queue_shows_that() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = doc_project(&dir);
    ok(&mut a, "autocode.propose", json!({ "query": "cost", "code_id": code, "target": "sentence" }));
    ok(&mut a, "source.edit", json!({ "id": source, "text": format!("Changed. {DOC}") }));
    let rejected = ok(&mut a, "proposal.list", json!({ "status": "rejected" }));
    assert_eq!(rejected.as_array().unwrap().len(), 2);
    assert_eq!(rejected[0]["decided_by"], "text edit");
}
