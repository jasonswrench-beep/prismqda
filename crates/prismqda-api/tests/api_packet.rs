//! Coder packets through the command layer (decision 161): a small project made for another coder, who codes blind, and the way their
//! coding comes back through Team > Merge to the same coder.

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
    kind: i64,
}

/// A study with two documents, a theme with a code under it, Ada's coding of both documents, and a memo.
fn study(dir: &TempDir) -> Study {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("study.pqda"), "name": "Study" }));
    let one = dir.path().join("one.txt");
    let two = dir.path().join("two.txt");
    std::fs::write(&one, "Sarah Jones said the market was friendly. She would go back.").unwrap();
    std::fs::write(&two, "The bus was late again. Nobody said sorry.").unwrap();
    let first = ok(&mut a, "import.run", json!({ "path": one }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let second = ok(&mut a, "import.run", json!({ "path": two }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let theme = ok(&mut a, "code.add", json!({ "name": "Belonging", "theme": true }))["id"].as_i64().unwrap();
    let kind = ok(&mut a, "code.add", json!({ "name": "Kindness", "parent_id": theme, "definition": "Being kind." }))["id"].as_i64().unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": first, "start": 25, "end": 40, "code_id": kind }));
    ok(&mut a, "coding.apply", json!({ "source_id": second, "start": 0, "end": 22, "code_id": kind }));
    ok(&mut a, "memo.add", json!({ "kind": "analytic", "scope": "project", "title": "Private thought", "body": "Do not show this to the coder." }));
    Study { api: a, first, second, kind }
}

fn open_as(dir: &TempDir, file: &str, actor: &str) -> Api {
    let mut b = Api::new(actor).without_backup_on_open();
    ok(&mut b, "project.open", json!({ "path": dir.path().join(file) }));
    b
}

#[test]
fn a_packet_holds_the_codebook_and_the_chosen_documents_and_nothing_else() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let r = ok(&mut s.api, "packet.make", json!({ "path": dir.path().join("for-cy"), "coder_name": "Cy", "source_ids": [s.first], "instructions": "Code every kind act." }));
    assert!(r["path"].as_str().unwrap().ends_with("for-cy.pqda"), "{r}");
    assert_eq!(r["documents"], json!(["one"]), "{r}");
    assert_eq!(r["codes"], 2);
    assert_eq!(r["codings"], 0);
    assert_eq!(r["coder_known"], false);
    let left = r["left_out"].to_string();
    assert!(left.contains("1 document") && left.contains("blind") && left.contains("recordings"), "{left}");

    // a different person opens it: they are the coder it was made for
    let mut b = open_as(&dir, "for-cy.pqda", "Zed");
    let info = ok(&mut b, "project.info", json!({}));
    assert_eq!(info["packet"]["coder_name"], "Cy", "{info}");
    assert_eq!(info["packet"]["from_project"], "Study");
    assert!(info["name"].as_str().unwrap().contains("packet for Cy"));
    let coders = ok(&mut b, "coder.list", json!({}));
    let names: Vec<&str> = coders.as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Cy"], "{coders}");
    assert_eq!(coders[0]["id"], info["me"], "the person using a packet is its coder");
    let sources = ok(&mut b, "source.list", json!({}));
    assert_eq!(sources.as_array().unwrap().len(), 1);
    let codes = ok(&mut b, "code.list", json!({})).to_string();
    assert!(codes.contains("Belonging") && codes.contains("Kindness") && codes.contains("Being kind."), "{codes}");
    let sid = sources[0]["id"].as_i64().unwrap();
    assert_eq!(ok(&mut b, "coding.list", json!({ "source_id": sid })).as_array().unwrap().len(), 0, "blind: no codings");
    let memos = ok(&mut b, "memo.list", json!({})).to_string();
    assert!(memos.contains("Instructions for Cy") && memos.contains("Code every kind act."), "{memos}");
    assert!(!memos.contains("Private thought") && !memos.contains("Do not show"), "the study's memos stay behind: {memos}");
    // the other document's words are nowhere in the file
    drop(b);
    let bytes = std::fs::read(dir.path().join("for-cy.pqda")).unwrap();
    let hay = String::from_utf8_lossy(&bytes);
    assert!(!hay.contains("The bus was late again"), "an unchosen document must not be in the packet file");
    assert!(!hay.contains("Do not show this"), "nor the study's memos");
    assert!(hay.contains("Sarah Jones said"), "the chosen one is");
}

#[test]
fn what_the_coder_coded_comes_back_through_merge_to_the_same_coder_and_a_second_packet_finds_them() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "packet.make", json!({ "path": dir.path().join("for-cy"), "coder_name": "Cy" }));
    let mut b = open_as(&dir, "for-cy.pqda", "Zed");
    let sources = ok(&mut b, "source.list", json!({}));
    assert_eq!(sources.as_array().unwrap().len(), 2, "no documents named, so all of them");
    let codes = ok(&mut b, "code.list", json!({}));
    let kind = codes.as_array().unwrap().iter().find(|c| c["name"] == "Kindness").unwrap()["id"].as_i64().unwrap();
    let doc = sources.as_array().unwrap().iter().find(|d| d["name"] == "one").unwrap()["id"].as_i64().unwrap();
    ok(&mut b, "coding.apply", json!({ "source_id": doc, "start": 0, "end": 5, "code_id": kind, "rationale": "a warm greeting" }));
    drop(b);

    let before = ok(&mut s.api, "coder.list", json!({})).as_array().unwrap().len();
    let preview = ok(&mut s.api, "merge.preview", json!({ "path": dir.path().join("for-cy.pqda") }));
    assert_eq!(preview["codings_added"], 1, "{preview}");
    assert_eq!(preview["documents_added"], json!([]), "the documents are matched, not added again: {preview}");
    assert_eq!(preview["codes_added"], json!([]), "{preview}");
    let merged = ok(&mut s.api, "merge.run", json!({ "path": dir.path().join("for-cy.pqda") }));
    assert_eq!(merged["codings_added"], 1, "{merged}");
    assert_eq!(merged["coders_added"], json!(["Cy"]));
    let coders = ok(&mut s.api, "coder.list", json!({}));
    assert_eq!(coders.as_array().unwrap().len(), before + 1);
    let cy = coders.as_array().unwrap().iter().find(|c| c["name"] == "Cy").unwrap()["id"].as_i64().unwrap();
    let codings = ok(&mut s.api, "coding.list", json!({ "source_id": s.first }));
    let mine = codings.as_array().unwrap().iter().find(|c| c["coder_id"].as_i64() == Some(cy)).expect("Cy's coding came back");
    assert_eq!((mine["start"].as_i64(), mine["end"].as_i64(), mine["code_id"].as_i64()), (Some(0), Some(5), Some(s.kind)));
    assert_eq!(mine["rationale"], "a warm greeting");

    // a second packet for the same person (any case) is for the coder the study now has
    let again = ok(&mut s.api, "packet.make", json!({ "path": dir.path().join("for-cy-2"), "coder_name": "  cy " }));
    assert_eq!(again["coder_known"], true, "{again}");
    let mut c = open_as(&dir, "for-cy-2.pqda", "Zed");
    let info = ok(&mut c, "project.info", json!({}));
    let a_id = ok(&mut s.api, "coder.list", json!({})).as_array().unwrap().iter().find(|c| c["name"] == "Cy").unwrap()["local_id"].clone();
    let packet_coder = ok(&mut c, "coder.list", json!({}))[0].clone();
    assert_eq!(packet_coder["local_id"], a_id, "the same local id, so the work lands on the same coder");
    assert_eq!(info["packet"]["coder_local_id"], a_id);
}

#[test]
fn a_packet_can_carry_the_codings_so_far_for_a_coder_to_review() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let r = ok(&mut s.api, "packet.make", json!({ "path": dir.path().join("review"), "coder_name": "Dee", "source_ids": [s.second], "include_codings": true }));
    assert_eq!(r["codings"], 1, "{r}");
    assert!(!r["left_out"].to_string().contains("blind"), "{r}");
    let mut b = open_as(&dir, "review.pqda", "Zed");
    let coders = ok(&mut b, "coder.list", json!({})).to_string();
    assert!(coders.contains("Ada") && coders.contains("Dee"), "the coder who made the codings comes along: {coders}");
}

#[test]
fn refusals_say_why_and_leave_nothing_behind() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    assert!(err(&mut s.api, "packet.make", json!({ "path": dir.path().join("x"), "coder_name": "  " })).message.contains("who the packet is for"));
    assert_eq!(err(&mut s.api, "packet.make", json!({ "path": dir.path().join("x"), "coder_name": "Cy", "source_ids": [999] })).code, "not_found");
    assert!(!dir.path().join("x.pqda").exists(), "nothing is left behind");
    assert_eq!(err(&mut s.api, "packet.make", json!({ "path": "", "coder_name": "Cy" })).code, "invalid");
    ok(&mut s.api, "packet.make", json!({ "path": dir.path().join("there"), "coder_name": "Cy" }));
    assert_eq!(err(&mut s.api, "packet.make", json!({ "path": dir.path().join("there"), "coder_name": "Cy" })).code, "already_exists");
    // names replaced in exports: the documents would go whole, so no
    ok(&mut s.api, "deid.add", json!({ "terms": ["Sarah"], "replacement": "[name]" }));
    ok(&mut s.api, "deid.set_exports", json!({ "on": true }));
    let e = err(&mut s.api, "packet.make", json!({ "path": dir.path().join("y"), "coder_name": "Cy" }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("names to replace"), "{}", e.message);
    assert!(!dir.path().join("y.pqda").exists());
}

#[test]
fn what_keeps_material_out_of_an_ai_packet_goes_with_the_packet() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "source.set_do_not_share", json!({ "id": s.first, "do_not_share": true }));
    let kind = s.kind;
    ok(&mut s.api, "code.update", json!({ "id": kind, "do_not_share": true }));
    ok(&mut s.api, "ai.set_mode", json!({ "mode": "off" }));
    ok(&mut s.api, "privacy.set_offline_lock", json!({ "on": true }));
    ok(&mut s.api, "packet.make", json!({ "path": dir.path().join("flags"), "coder_name": "Cy" }));
    let mut b = open_as(&dir, "flags.pqda", "Zed");
    let docs = ok(&mut b, "source.list", json!({}));
    assert!(docs.as_array().unwrap().iter().all(|d| d["do_not_share"] == true) || docs.as_array().unwrap().iter().any(|d| d["do_not_share"] == true), "{docs}");
    let flagged = docs.as_array().unwrap().iter().filter(|d| d["do_not_share"] == true).count();
    assert_eq!(flagged, 1, "the document marked do not share is still marked in the packet: {docs}");
    let codes = ok(&mut b, "code.list", json!({}));
    assert_eq!(codes.as_array().unwrap().iter().find(|c| c["name"] == "Kindness").unwrap()["do_not_share"], true, "and the code");
    let info = ok(&mut b, "project.info", json!({}));
    assert_eq!(info["ai_mode"], "off", "the sender's AI setting goes with it: {info}");
    assert_eq!(info["offline_lock"], true, "and the offline lock");
}
