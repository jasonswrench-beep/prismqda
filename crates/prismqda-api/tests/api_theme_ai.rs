//! Candidate themes suggested by an AI through a prompt packet (decision 168): the packet holds the codebook and no passage, the checker drops
//! what the model made up, nothing is made until a suggestion is accepted, and the run keeps the record.

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
    let f = dir.path().join("one.txt");
    std::fs::write(&f, "Secret words that must not be in a theme packet.").unwrap();
    let source = ok(&mut a, "import.run", json!({ "path": f }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let mut ids = Vec::new();
    for (name, def) in [("Welcome", "Being greeted."), ("Greeting", ""), ("Cost", "Talk about money."), ("Fares", "The price of a ticket."), ("Weather", "Rain and sun.")] {
        let id = ok(&mut a, "code.add", json!({ "name": name, "definition": def }))["id"].as_i64().unwrap();
        ids.push(id);
    }
    ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 0, "end": 12, "code_id": ids[0] }));
    ok(&mut a, "code.add", json!({ "name": "Existing theme", "theme": true }));
    a
}

fn id_of(a: &mut Api, name: &str) -> i64 {
    ok(a, "code.list", json!({})).as_array().unwrap().iter().find(|c| c["name"] == name).unwrap()["id"].as_i64().unwrap()
}

const GOOD: &str = "Here you go:\n```json\n[\
  {\"theme\": \"Being welcomed\", \"definition\": \"People are greeted.\", \"codes\": [\"Welcome\", \"greeting\", \"Made up code\"]},\
  {\"theme\": \"The price of travel\", \"definition\": \"Money.\", \"codes\": [\"Cost\", \"Fares\", \"Welcome\"]},\
  {\"theme\": \"Lonely\", \"codes\": [\"Weather\"]},\
  {\"definition\": \"no name\", \"codes\": [\"Cost\", \"Fares\"]}\
]\n```";

#[test]
fn the_packet_holds_the_codebook_and_no_passage_and_asks_for_themes() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let p = ok(&mut a, "theme.ai_packet", json!({}));
    let prompt = p["prompt"].as_str().unwrap();
    assert!(prompt.contains("Group the codes into candidate themes"), "{prompt}");
    assert!(prompt.contains("- Welcome: Being greeted. (applied 1 time)"), "{prompt}");
    assert!(prompt.contains("- Cost: Talk about money. (applied 0 times)"), "{prompt}");
    assert!(prompt.contains("THEMES THE RESEARCHER ALREADY HAS") && prompt.contains("- Existing theme"), "{prompt}");
    assert!(!prompt.contains("Secret words"), "no passage is in the packet");
    assert_eq!(p["codes"].as_array().unwrap().len(), 5, "the themes are not codes to group");
    let runs = ok(&mut a, "ai.runs", json!({}));
    assert_eq!(runs[0]["task"], "suggest_themes");
    assert_eq!(runs[0]["passages"], 0);
}

#[test]
fn what_cannot_be_sent_is_refused_or_left_out() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let cost = id_of(&mut a, "Cost");
    ok(&mut a, "code.update", json!({ "id": cost, "do_not_share": true }));
    let p = ok(&mut a, "theme.ai_packet", json!({}));
    assert_eq!(p["left_out"], json!(["Cost"]));
    assert!(!p["prompt"].as_str().unwrap().contains("Cost"), "a code marked do not share is not in it");
    // too few codes
    let dir2 = TempDir::new().unwrap();
    let mut few = Api::new("Ada").without_backup_on_open();
    ok(&mut few, "project.create", json!({ "path": dir2.path().join("p.pqda"), "name": "Few" }));
    ok(&mut few, "code.add", json!({ "name": "A" }));
    ok(&mut few, "code.add", json!({ "name": "B" }));
    assert!(err(&mut few, "theme.ai_packet", json!({})).message.contains("at least 3"));
    // the offline lock and the AI switch
    ok(&mut a, "privacy.set_offline_lock", json!({ "on": true }));
    assert!(err(&mut a, "theme.ai_packet", json!({})).message.contains("offline lock is on"));
    ok(&mut a, "privacy.set_offline_lock", json!({ "on": false }));
    ok(&mut a, "ai.set_mode", json!({ "mode": "off" }));
    assert!(err(&mut a, "theme.ai_packet", json!({})).message.contains("AI is off"));
}

#[test]
fn the_checker_drops_what_the_model_made_up_and_nothing_is_made_until_a_suggestion_is_accepted() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let run = ok(&mut a, "theme.ai_packet", json!({}))["run_id"].as_i64().unwrap();
    let before = ok(&mut a, "code.list", json!({})).as_array().unwrap().len();
    assert_eq!(err(&mut a, "theme.ai_answer", json!({ "run_id": run, "model": " ", "answer": GOOD })).code, "invalid");
    assert!(err(&mut a, "theme.ai_answer", json!({ "run_id": run, "model": "Chatbot 1", "answer": "no list" })).message.contains("no list"));
    let out = ok(&mut a, "theme.ai_answer", json!({ "run_id": run, "model": "Chatbot 1", "answer": GOOD }));
    assert_eq!(out["read"], 4);
    let s = out["suggestions"].as_array().unwrap();
    assert_eq!(s.len(), 2, "{out}");
    assert_eq!(s[0]["name"], "Being welcomed");
    assert_eq!(s[0]["code_names"], json!(["Welcome", "Greeting"]), "a name is matched without regard to case, and the made-up code is dropped");
    assert!(s[0]["problems"][0].as_str().unwrap().contains("Made up code"), "{out}");
    assert_eq!(s[1]["name"], "The price of travel");
    assert_eq!(s[1]["code_names"], json!(["Cost", "Fares"]), "Welcome stays in the first theme");
    assert!(s[1]["problems"][0].as_str().unwrap().contains("already placed in \"Being welcomed\""), "{out}");
    let why: Vec<&str> = out["rejected"].as_array().unwrap().iter().map(|r| r["why"].as_str().unwrap()).collect();
    assert_eq!(why.len(), 2, "{out}");
    assert!(why.iter().any(|w| w.contains("fewer than two")) && why.iter().any(|w| w.contains("no theme name")), "{why:?}");
    assert_eq!(ok(&mut a, "code.list", json!({})).as_array().unwrap().len(), before, "nothing was made");
    assert!(err(&mut a, "theme.ai_answer", json!({ "run_id": run, "model": "Chatbot 1", "answer": GOOD })).message.contains("already has an answer"));
    let log = ok(&mut a, "ai.runs", json!({}));
    assert_eq!((log[0]["answered"].clone(), log[0]["pending"].clone(), log[0]["model"].clone()), (json!(true), json!(2), json!("Chatbot 1")));
}

#[test]
fn accepting_makes_a_candidate_theme_with_its_codes_and_a_memo_and_a_decision_is_made_once() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let run = ok(&mut a, "theme.ai_packet", json!({}))["run_id"].as_i64().unwrap();
    ok(&mut a, "theme.ai_answer", json!({ "run_id": run, "model": "Chatbot 1", "answer": GOOD }));
    let r = ok(&mut a, "theme.ai_decide", json!({ "run_id": run, "index": 1, "accept": true }));
    assert_eq!(r["suggestions"][0]["status"], "accepted");
    let o = ok(&mut a, "theme.overview", json!({}));
    let t = o["themes"].as_array().unwrap().iter().find(|t| t["name"] == "Being welcomed").expect("the theme was made");
    assert_eq!(t["stage"], "candidate");
    assert_eq!(t["definition"], "People are greeted.");
    let mut names: Vec<&str> = t["codes"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Greeting", "Welcome"]);
    assert_eq!(t["memos"], 1, "a memo says where it came from");
    let memos = ok(&mut a, "memo.list", json!({ "scope": "code", "target_id": t["id"] }));
    assert!(memos[0]["body"].as_str().unwrap().contains("Chatbot 1") && memos[0]["body"].as_str().unwrap().contains("AI run"), "{memos}");
    // the first cannot be decided again; a name that is already a code is refused for the second
    assert!(err(&mut a, "theme.ai_decide", json!({ "run_id": run, "index": 1, "accept": false })).message.contains("already been decided"));
    let clash = err(&mut a, "theme.ai_decide", json!({ "run_id": run, "index": 2, "accept": true, "name": "Being welcomed" }));
    assert_eq!(clash.code, "invalid");
    ok(&mut a, "theme.ai_decide", json!({ "run_id": run, "index": 2, "accept": false }));
    let after = ok(&mut a, "theme.ai_run", json!({ "run_id": run }));
    let status: Vec<&str> = after["suggestions"].as_array().unwrap().iter().map(|s| s["status"].as_str().unwrap()).collect();
    assert_eq!(status, ["accepted", "rejected"]);
    // the AI log counts the decisions, and the paragraph for a methods section says what was done
    let log = ok(&mut a, "ai.runs", json!({}));
    assert_eq!((log[0]["accepted"].clone(), log[0]["rejected"].clone(), log[0]["pending"].clone()), (json!(1), json!(1), json!(0)));
    let para = ok(&mut a, "ai.disclosure", json!({}))["text"].as_str().unwrap().to_string();
    assert!(para.contains("asked for candidate themes") && para.contains("no passage of any document") && para.contains("1 accepted, 1 rejected"), "{para}");
    // Undo still reaches what the researcher did, a decision is not in the way
    ok(&mut a, "undo", json!({}));
}

#[test]
fn a_theme_marked_do_not_share_is_not_named_in_the_packet_and_an_answer_to_a_theme_run_is_not_a_coding_answer() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let theme = id_of(&mut a, "Existing theme");
    ok(&mut a, "code.update", json!({ "id": theme, "do_not_share": true }));
    let p = ok(&mut a, "theme.ai_packet", json!({}));
    assert!(!p["prompt"].as_str().unwrap().contains("Existing theme"), "a flagged theme is not named");
    assert_eq!(p["left_out"], json!(["Existing theme"]));
    let run = p["run_id"].as_i64().unwrap();
    let e = err(&mut a, "ai.answer", json!({ "run_id": run, "model": "Chatbot 1", "answer": "[]" }));
    assert!(e.message.contains("asked for themes"), "{}", e.message);
}

#[test]
fn a_suggestion_with_a_code_and_its_sub_code_does_not_pull_the_sub_code_out_of_its_parent() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let welcome = id_of(&mut a, "Welcome");
    let child = ok(&mut a, "code.add", json!({ "name": "Warm hello", "parent_id": welcome }))["id"].as_i64().unwrap();
    let run = ok(&mut a, "theme.ai_packet", json!({}))["run_id"].as_i64().unwrap();
    let answer = r#"[{"theme": "Being welcomed", "codes": ["Welcome", "Warm hello", "Greeting"]}]"#;
    ok(&mut a, "theme.ai_answer", json!({ "run_id": run, "model": "Chatbot 1", "answer": answer }));
    ok(&mut a, "theme.ai_decide", json!({ "run_id": run, "index": 1, "accept": true }));
    let codes = ok(&mut a, "code.list", json!({}));
    let by = |n: &str| codes.as_array().unwrap().iter().find(|c| c["name"] == n).unwrap().clone();
    let theme = by("Being welcomed")["id"].clone();
    assert_eq!(by("Welcome")["parent_id"], theme);
    assert_eq!(by("Greeting")["parent_id"], theme);
    assert_eq!(by("Warm hello")["id"], child);
    assert_eq!(by("Warm hello")["parent_id"], by("Welcome")["id"], "the sub-code stays under its parent, which moved with the theme");
}

#[test]
fn undoing_the_theme_an_accepted_suggestion_made_lets_the_suggestion_wait_again() {
    let dir = TempDir::new().unwrap();
    let mut a = study(&dir);
    let run = ok(&mut a, "theme.ai_packet", json!({}))["run_id"].as_i64().unwrap();
    ok(&mut a, "theme.ai_answer", json!({ "run_id": run, "model": "Chatbot 1", "answer": GOOD }));
    ok(&mut a, "theme.ai_decide", json!({ "run_id": run, "index": 1, "accept": true }));
    // the memo, the two moves, and the theme are taken back one at a time
    for _ in 0..4 {
        ok(&mut a, "undo", json!({}));
    }
    let after = ok(&mut a, "theme.ai_run", json!({ "run_id": run }));
    assert_eq!(after["suggestions"][0]["status"], "pending", "{after}");
    ok(&mut a, "theme.ai_decide", json!({ "run_id": run, "index": 1, "accept": true }));
}
