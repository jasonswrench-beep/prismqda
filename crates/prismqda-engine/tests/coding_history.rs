//! The history of one coding: the audit events that name it, oldest first. Counting what was added
//! or removed, and finding the newest change, is left to whoever shows it.

use prismqda_engine::{CodeEdit, NewCode, NewCoder, NewCoding, NewProject, NewProposal, NewSource, OpenOptions, Project, ProposalOrigin, SourceType};
use serde_json::json;
use tempfile::TempDir;

struct W {
    p: Project,
    source: i64,
    coder: i64,
    code: i64,
}

fn world(dir: &TempDir) -> W {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let source = p.add_source(&NewSource::new("S", SourceType::Document, "abcdefghijklmnopqrstuvwxyz")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("C")).unwrap();
    // Ratings only live on rated codes.
    p.update_code(code, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
    W { p, source, coder, code }
}

fn code_one(w: &mut W, start: i64, end: i64) -> i64 {
    w.p.apply_coding(&NewCoding::new(w.source, start, end, w.code, w.coder)).unwrap()
}

fn actions(w: &W, coding: i64) -> Vec<String> {
    w.p.coding_history(coding).unwrap().into_iter().map(|h| h.action).collect()
}

#[test]
fn a_new_coding_has_the_event_that_created_it() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 2, 8);
    let history = w.p.coding_history(id).unwrap();
    assert_eq!(history.len(), 1);
    let first = &history[0];
    assert_eq!(first.action, "coding.add", "the creation event is named coding.add (the API command is coding.apply)");
    assert_eq!(first.actor, "tester");
    assert!(first.ts.len() == 24 && first.ts.ends_with('Z') && first.ts.contains('T'), "a UTC timestamp with milliseconds: {}", first.ts);
    assert_eq!(first.before, None);
    let after = first.after.as_ref().unwrap();
    assert_eq!((after["start"].as_i64(), after["end"].as_i64(), after["code_id"].as_i64()), (Some(2), Some(8), Some(w.code)));
}

#[test]
fn a_history_entry_serializes_to_exactly_ts_actor_action_before_after() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 3);
    w.p.set_coding_rating(id, Some(4)).unwrap();
    let v = serde_json::to_value(w.p.coding_history(id).unwrap()).unwrap();
    for entry in v.as_array().unwrap() {
        let mut keys: Vec<&str> = entry.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["action", "actor", "after", "before", "ts"]);
    }
    assert_eq!(v[0]["before"], serde_json::Value::Null);
    assert_eq!(v[1]["before"], json!({ "rating": null }));
    assert_eq!(v[1]["after"], json!({ "rating": 4 }));
}

#[test]
fn every_change_to_the_coding_is_listed_oldest_first() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let other_code = w.p.add_code(&NewCode::new("D")).unwrap();
    w.p.update_code(other_code, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rationale(id, "because").unwrap();
    w.p.set_coding_rating(id, Some(5)).unwrap();
    w.p.change_coding_code(id, other_code).unwrap();
    w.p.set_anchor(id, true).unwrap();
    assert_eq!(actions(&w, id), ["coding.add", "coding.rationale", "coding.rating", "coding.change_code", "anchor.add"]);

    let history = w.p.coding_history(id).unwrap();
    let mut sorted = history.iter().map(|h| h.ts.clone()).collect::<Vec<_>>();
    sorted.sort();
    assert_eq!(sorted, history.iter().map(|h| h.ts.clone()).collect::<Vec<_>>(), "timestamps never go backwards");
    let change = &history[3];
    assert_eq!(change.before, Some(json!({ "code_id": w.code })));
    assert_eq!(change.after, Some(json!({ "code_id": other_code })));
}

#[test]
fn other_codings_and_other_events_are_left_out() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let one = code_one(&mut w, 0, 5);
    let two = code_one(&mut w, 6, 9);
    w.p.set_coding_rating(two, Some(2)).unwrap();
    w.p.annotate("a note").unwrap();
    w.p.add_code(&NewCode::new("E")).unwrap();
    assert_eq!(actions(&w, one), ["coding.add"]);
    assert_eq!(actions(&w, two), ["coding.add", "coding.rating"]);
}

#[test]
fn a_removal_is_in_the_history_with_the_whole_coding() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, Some(3)).unwrap();
    w.p.remove_coding(id).unwrap();
    assert_eq!(actions(&w, id), ["coding.add", "coding.rating", "coding.remove"]);
    let removed = w.p.coding_history(id).unwrap().pop().unwrap();
    assert_eq!(removed.after, None);
    assert_eq!(removed.before.as_ref().unwrap()["rating"], 3);
    assert_eq!(removed.before.as_ref().unwrap()["code_id"], w.code);

    // Putting it back with undo keeps the same history, because it is the same coding.
    w.p.undo().unwrap().unwrap();
    assert_eq!(actions(&w, id), ["coding.add", "coding.rating", "coding.remove"]);
}

#[test]
fn a_new_coding_that_reuses_a_deleted_codings_number_starts_a_fresh_history() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let first = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(first, Some(1)).unwrap();
    w.p.remove_coding(first).unwrap();
    // The database hands out the freed number again.
    let second = code_one(&mut w, 10, 15);
    assert_eq!(second, first, "this test needs the number to be reused");
    assert_eq!(actions(&w, second), ["coding.add"], "none of the first coding's events belong to the second");
    w.p.set_coding_rating(second, Some(5)).unwrap();
    assert_eq!(actions(&w, second), ["coding.add", "coding.rating"]);
}

#[test]
fn a_coding_accepted_from_a_suggestion_has_its_creation_event_too() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let batch = w
        .p
        .create_proposals(
            ProposalOrigin::Autocode,
            "search welcome",
            &[NewProposal { source_id: w.source, start: 0, end: 5, code_id: w.code, coder_id: w.coder, unit_id: None, rationale: String::new() }],
        )
        .unwrap();
    assert_eq!(batch.created, 1);
    let proposal = w.p.list_proposals(Some("pending"), None, 10).unwrap().remove(0);
    w.p.decide_proposals(&[proposal.id], true).unwrap();
    let coding = w.p.list_codings(w.source).unwrap().remove(0);
    assert_eq!(actions(&w, coding.id), ["coding.add"]);
}

#[test]
fn a_coding_nobody_made_has_no_history() {
    let dir = TempDir::new().unwrap();
    let w = world(&dir);
    assert!(w.p.coding_history(12345).unwrap().is_empty());
}

#[test]
fn codings_added_in_one_batch_each_have_their_own_history() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let ids = w
        .p
        .apply_codings(&[NewCoding::new(w.source, 0, 3, w.code, w.coder), NewCoding::new(w.source, 3, 6, w.code, w.coder)])
        .unwrap();
    assert_eq!(actions(&w, ids[0]), ["coding.add"]);
    assert_eq!(actions(&w, ids[1]), ["coding.add"]);
}
