//! Links between two coded passages with a named relation (decision 162), through the command layer: made, listed with both passages,
//! renamed, deleted, undone, refused in words, hidden while a passage is gone, and carried by a merge (and so by a packet).

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

struct Study {
    api: Api,
    first: i64,
    second: i64,
    third: i64,
}

/// Three coded passages: two in one document, one in another.
fn study(dir: &TempDir) -> Study {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("study.pqda"), "name": "Study" }));
    let one = dir.path().join("one.txt");
    let two = dir.path().join("two.txt");
    std::fs::write(&one, "The market was friendly. Nobody was rude to me.").unwrap();
    std::fs::write(&two, "The bus was late again. Nobody said sorry.").unwrap();
    let s1 = ok(&mut a, "import.run", json!({ "path": one }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let s2 = ok(&mut a, "import.run", json!({ "path": two }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let kind = ok(&mut a, "code.add", json!({ "name": "Kindness" }))["id"].as_i64().unwrap();
    let rude = ok(&mut a, "code.add", json!({ "name": "Rudeness" }))["id"].as_i64().unwrap();
    let first = ok(&mut a, "coding.apply", json!({ "source_id": s1, "start": 0, "end": 24, "code_id": kind }))["id"].as_i64().unwrap();
    let second = ok(&mut a, "coding.apply", json!({ "source_id": s1, "start": 25, "end": 47, "code_id": kind }))["id"].as_i64().unwrap();
    let third = ok(&mut a, "coding.apply", json!({ "source_id": s2, "start": 24, "end": 42, "code_id": rude }))["id"].as_i64().unwrap();
    Study { api: a, first, second, third }
}

#[test]
fn a_link_has_a_named_relation_and_lists_with_both_passages() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let id = ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.third, "relation": "  contradicts " }))["id"].as_i64().unwrap();
    let all = ok(&mut s.api, "link.list", json!({}));
    assert_eq!(all.as_array().unwrap().len(), 1, "{all}");
    let l = &all[0];
    assert_eq!(l["id"], id);
    assert_eq!(l["relation"], "contradicts", "spaces are folded");
    assert_eq!(l["from"]["text"], "The market was friendly.");
    assert_eq!(l["to"]["text"], "Nobody said sorry.");
    assert_eq!((l["from"]["code_name"].as_str(), l["to"]["code_name"].as_str()), (Some("Kindness"), Some("Rudeness")));
    assert_eq!(l["to"]["source_name"], "two");

    // a passage sees the links at either end; one that is in none sees none
    assert_eq!(ok(&mut s.api, "link.list", json!({ "coding_id": s.third })).as_array().unwrap().len(), 1);
    assert_eq!(ok(&mut s.api, "link.list", json!({ "coding_id": s.first })).as_array().unwrap().len(), 1);
    assert_eq!(ok(&mut s.api, "link.list", json!({ "coding_id": s.second })).as_array().unwrap().len(), 0);

    // the same pair may be linked with another relation, or the other way round; the common relations are offered after the used ones
    ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.third, "relation": "contrasts with" }));
    ok(&mut s.api, "link.add", json!({ "from_coding": s.third, "to_coding": s.first, "relation": "Contradicts" }));
    let rel = ok(&mut s.api, "link.relations", json!({}));
    assert_eq!(rel["used"][0]["name"].as_str().map(str::to_lowercase), Some("contradicts".to_string()), "{rel}");
    assert_eq!(rel["used"][0]["uses"], 2, "case does not split a relation: {rel}");
    assert!(rel["common"].as_array().unwrap().iter().any(|c| c == "explains"));

    // the relation can be changed, and a link deleted
    ok(&mut s.api, "link.update", json!({ "id": id, "relation": "is undone by" }));
    assert_eq!(ok(&mut s.api, "link.list", json!({ "coding_id": s.first }))[0]["relation"], "is undone by");
    ok(&mut s.api, "link.delete", json!({ "id": id }));
    assert_eq!(ok(&mut s.api, "link.list", json!({})).as_array().unwrap().len(), 2);
}

#[test]
fn what_cannot_be_linked_is_refused_in_words() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    assert!(err(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.first, "relation": "explains" })).message.contains("itself"));
    assert!(err(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "   " })).message.contains("related"));
    assert!(err(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "x".repeat(81) })).message.contains("80"));
    assert_eq!(err(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": 9999, "relation": "explains" })).code, "not_found");
    assert_eq!(err(&mut s.api, "link.add", json!({ "from_coding": -9999, "to_coding": s.first, "relation": "explains" })).code, "not_found", "an area that is not there");
    ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "explains" }));
    let twin = err(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "EXPLAINS" }));
    assert!(twin.message.contains("already linked"), "{}", twin.message);
    assert_eq!(err(&mut s.api, "link.update", json!({ "id": 9999, "relation": "x" })).code, "not_found");
}

#[test]
fn links_can_be_undone_and_are_hidden_while_a_passage_is_gone() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let id = ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "explains" }))["id"].as_i64().unwrap();
    ok(&mut s.api, "undo", json!({}));
    assert_eq!(ok(&mut s.api, "link.list", json!({})).as_array().unwrap().len(), 0, "undo removes the link it added");
    // add again (a new id), rename, delete, then take each back in turn
    let id2 = ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "explains" }))["id"].as_i64().unwrap();
    assert_ne!(id, 0);
    ok(&mut s.api, "link.update", json!({ "id": id2, "relation": "leads to" }));
    ok(&mut s.api, "link.delete", json!({ "id": id2 }));
    assert_eq!(ok(&mut s.api, "link.list", json!({})).as_array().unwrap().len(), 0);
    ok(&mut s.api, "undo", json!({}));
    let back = ok(&mut s.api, "link.list", json!({}));
    assert_eq!((back[0]["id"].as_i64(), back[0]["relation"].as_str()), (Some(id2), Some("leads to")), "the deleted link is back with its id and relation");
    ok(&mut s.api, "undo", json!({}));
    assert_eq!(ok(&mut s.api, "link.list", json!({}))[0]["relation"], "explains", "the rename is taken back");

    // removing a passage hides its links; Undo brings both back
    ok(&mut s.api, "coding.remove", json!({ "id": s.second }));
    assert_eq!(ok(&mut s.api, "link.list", json!({})).as_array().unwrap().len(), 0, "a link to a removed passage is not listed");
    ok(&mut s.api, "undo", json!({}));
    assert_eq!(ok(&mut s.api, "link.list", json!({})).as_array().unwrap().len(), 1, "and is back when the passage is");
}

#[test]
fn a_merge_and_so_a_packet_carries_the_links_whose_two_passages_are_in_both() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.third, "relation": "contradicts" }));
    // a packet with the codings carries the link
    let made = ok(&mut s.api, "packet.make", json!({ "path": dir.path().join("review"), "coder_name": "Dee", "include_codings": true }));
    assert_eq!(made["codings"], 3, "{made}");
    let mut b = Api::new("Zed").without_backup_on_open();
    ok(&mut b, "project.open", json!({ "path": dir.path().join("review.pqda") }));
    let carried = ok(&mut b, "link.list", json!({}));
    assert_eq!(carried.as_array().unwrap().len(), 1, "{carried}");
    assert_eq!(carried[0]["relation"], "contradicts");
    assert_eq!(carried[0]["to"]["text"], "Nobody said sorry.");
    // Dee adds a link of her own and sends it back: the first is already here, the second comes in
    let segs = ok(&mut b, "segments.list", json!({ "filter": {}, "limit": 50 }));
    let ids: Vec<i64> = segs["segments"].as_array().unwrap().iter().filter(|g| g["text"] == "Nobody was rude to me." || g["text"] == "The market was friendly.").map(|g| g["coding_id"].as_i64().unwrap()).collect();
    assert_eq!(ids.len(), 2, "{segs}");
    ok(&mut b, "link.add", json!({ "from_coding": ids[0], "to_coding": ids[1], "relation": "supports" }));
    drop(b);
    let merged = ok(&mut s.api, "merge.run", json!({ "path": dir.path().join("review.pqda") }));
    assert_eq!((merged["links_added"].as_i64(), merged["links_already_here"].as_i64()), (Some(1), Some(1)), "{merged}");
    let all = ok(&mut s.api, "link.list", json!({}));
    let rel: Vec<&str> = all.as_array().unwrap().iter().map(|l| l["relation"].as_str().unwrap()).collect();
    assert_eq!(rel, ["contradicts", "supports"], "{all}");
}

#[test]
fn a_link_goes_with_its_passage_so_a_new_passage_that_gets_the_same_number_does_not_inherit_it() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    // `third` has the largest number, which SQLite gives to the next coding once it is removed
    let id = ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.third, "relation": "contradicts" }))["id"].as_i64().unwrap();
    ok(&mut s.api, "coding.remove", json!({ "id": s.third }));
    let code = ok(&mut s.api, "code.list", json!({}))[0]["id"].as_i64().unwrap();
    let source = ok(&mut s.api, "source.list", json!({}))[1]["id"].as_i64().unwrap();
    let fresh = ok(&mut s.api, "coding.apply", json!({ "source_id": source, "start": 0, "end": 23, "code_id": code }))["id"].as_i64().unwrap();
    assert_eq!(fresh, s.third, "the number was used again, which is what the check is about");
    assert_eq!(ok(&mut s.api, "link.list", json!({})).as_array().unwrap().len(), 0, "the new passage has no link it did not make");
    assert_eq!(ok(&mut s.api, "link.list", json!({ "coding_id": fresh })).as_array().unwrap().len(), 0);
    // taking both back brings the old passage and its link back together
    ok(&mut s.api, "undo", json!({}));
    ok(&mut s.api, "undo", json!({}));
    let back = ok(&mut s.api, "link.list", json!({}));
    assert_eq!((back[0]["id"].as_i64(), back[0]["relation"].as_str()), (Some(id), Some("contradicts")), "{back}");
}

#[test]
fn a_link_cannot_be_renamed_into_a_twin_of_another() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "explains" }));
    let other = ok(&mut s.api, "link.add", json!({ "from_coding": s.first, "to_coding": s.second, "relation": "supports" }))["id"].as_i64().unwrap();
    let e = err(&mut s.api, "link.update", json!({ "id": other, "relation": "Explains" }));
    assert!(e.message.contains("already linked"), "{}", e.message);
    // renaming to what it already is changes nothing and writes nothing
    let before = ok(&mut s.api, "events.list", json!({ "limit": 500 })).as_array().unwrap().len();
    ok(&mut s.api, "link.update", json!({ "id": other, "relation": "supports" }));
    assert_eq!(ok(&mut s.api, "events.list", json!({ "limit": 500 })).as_array().unwrap().len(), before);
}
