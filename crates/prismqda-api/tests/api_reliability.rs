//! Samples and agreement runs through the command layer, the way the interface will call them.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

struct Setup {
    api: Api,
    source: i64,
    units: Vec<Value>,
    codes: [i64; 2],
    ada: i64,
    ben: i64,
    model_a: i64,
}

/// Ten paragraphs, two codes, and three coders. Ada and Ben agree on nine; Model A agrees with Ada on eight.
fn setup(dir: &TempDir) -> Setup {
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let text: String = (0..10).map(|i| format!("Paragraph number {i} has some words.")).collect::<Vec<_>>().join("\n\n");
    let path = dir.path().join("t.txt");
    std::fs::write(&path, text).unwrap();
    let out = ok(&mut api, "import.run", json!({ "path": path }));
    let source = out["outcome"]["source_ids"][0].as_i64().unwrap();
    let units = ok(&mut api, "unit.list", json!({ "source_id": source })).as_array().unwrap().clone();
    assert_eq!(units.len(), 10);
    let a = ok(&mut api, "code.add", json!({ "name": "Yes" }))["id"].as_i64().unwrap();
    let b = ok(&mut api, "code.add", json!({ "name": "No" }))["id"].as_i64().unwrap();
    let ada = ok(&mut api, "project.info", json!({}))["me"].as_i64().unwrap();
    let ben = ok(&mut api, "coder.add", json!({ "name": "Ben" }))["id"].as_i64().unwrap();
    let model_a = ok(&mut api, "coder.add", json!({ "name": "Model A", "role": "model", "provider": "Provider A", "model_name": "Model A", "model_version": "5.5" }))["id"].as_i64().unwrap();
    let mut s = Setup { api, source, units, codes: [a, b], ada, ben, model_a };
    let (ada, ben, model_a) = (s.ada, s.ben, s.model_a);
    for i in 0..10 {
        let first = i % 2;
        apply(&mut s, i, first, ada);
        apply(&mut s, i, if i == 3 { 1 - first } else { first }, ben);
        apply(&mut s, i, if i == 3 || i == 7 { 1 - first } else { first }, model_a);
    }
    s
}

fn apply(s: &mut Setup, unit: usize, code: usize, coder: i64) {
    let u = &s.units[unit];
    let args = json!({ "source_id": s.source, "start": u["start"], "end": u["end"], "code_id": s.codes[code], "coder_id": coder, "unit_id": u["id"] });
    ok(&mut s.api, "coding.apply", args);
}

#[test]
fn a_person_can_add_coders_and_models_are_labeled() {
    let dir = TempDir::new().unwrap();
    let mut s = setup(&dir);
    let coders = ok(&mut s.api, "coder.list", json!({}));
    let roles: Vec<(&str, &str)> = coders.as_array().unwrap().iter().map(|c| (c["name"].as_str().unwrap(), c["role"].as_str().unwrap())).collect();
    assert_eq!(roles, [("Ada", "human"), ("Ben", "human"), ("Model A", "model")]);
    assert_eq!(err(&mut s.api, "coder.add", json!({ "name": "Bot", "role": "model" })).code, "invalid", "a model coder needs a model name");
    assert_eq!(err(&mut s.api, "coder.add", json!({ "name": "X", "role": "robot" })).code, "bad_args");
}

#[test]
fn two_people_are_compared_and_the_run_is_kept() {
    let dir = TempDir::new().unwrap();
    let mut s = setup(&dir);
    let r = ok(&mut s.api, "reliability.run", json!({ "coder_ids": [s.ada, s.ben], "bootstrap_replications": 100 }));
    assert_eq!(r["reporting_term"], "intercoder_reliability");
    assert_eq!(r["units_in_sample"], 10);
    assert!((r["overall"]["percent_agreement"].as_f64().unwrap() - 0.9).abs() < 1e-12);
    assert_eq!(r["disagreements"], 1);
    assert_eq!(r["confusion_codes"], json!(["Yes", "No"]));
    assert!(r["report_text"].as_str().unwrap().contains("Ben (human)"));

    let id = r["run_id"].as_i64().unwrap();
    let list = ok(&mut s.api, "reliability.list", json!({}));
    assert_eq!(list[0]["id"], id);
    assert_eq!(list[0]["units"], 10);
    assert_eq!(list[0]["archived"], false);
    let got = ok(&mut s.api, "reliability.get", json!({ "id": id }));
    assert_eq!(got["text"], r["report_text"]);
    assert_eq!(err(&mut s.api, "reliability.get", json!({ "id": 999 })).code, "not_found");
}

#[test]
fn a_model_makes_it_concordance_and_says_which_model() {
    let dir = TempDir::new().unwrap();
    let mut s = setup(&dir);
    let r = ok(&mut s.api, "reliability.run", json!({ "coder_ids": [s.ada, s.model_a], "save": false }));
    assert_eq!(r["reporting_term"], "concordance");
    assert!(r["report_text"].as_str().unwrap().contains("Model A (model, Model A version 5.5)"));
    assert_eq!(r["run_id"], Value::Null);
    assert_eq!(ok(&mut s.api, "reliability.list", json!({})).as_array().unwrap().len(), 0);
}

#[test]
fn a_run_locks_its_sources_text_until_archived() {
    let dir = TempDir::new().unwrap();
    let mut s = setup(&dir);
    let id = ok(&mut s.api, "reliability.run", json!({ "coder_ids": [s.ada, s.ben] }))["run_id"].as_i64().unwrap();
    let e = err(&mut s.api, "source.edit", json!({ "id": s.source, "text": "Something else entirely." }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("archive the run first"), "{}", e.message);

    ok(&mut s.api, "reliability.archive", json!({ "id": id }));
    ok(&mut s.api, "source.edit", json!({ "id": s.source, "text": "Something else entirely." }));
    assert_eq!(err(&mut s.api, "reliability.archive", json!({ "id": id })).code, "invalid");
}

#[test]
fn samples_are_drawn_from_the_interface_with_a_saved_seed() {
    let dir = TempDir::new().unwrap();
    let mut s = setup(&dir);
    let a = ok(&mut s.api, "sample.draw", json!({ "size": 4, "seed": 11 }));
    let b = ok(&mut s.api, "sample.draw", json!({ "size": 4, "seed": 11 }));
    assert_eq!(a, b);
    assert_eq!(a["unit_ids"].as_array().unwrap().len(), 4);
    assert_eq!(a["pool"], 10);
    let by_source = ok(&mut s.api, "sample.draw", json!({ "size": 5, "seed": 2, "stratify": { "by": "source" } }));
    assert_eq!(by_source["unit_ids"].as_array().unwrap().len(), 5);
    assert_eq!(err(&mut s.api, "sample.draw", json!({ "size": 11 })).code, "invalid");

    // The sample's units are what the run measures.
    let r = ok(&mut s.api, "reliability.run", json!({ "coder_ids": [s.ada, s.ben], "unit_ids": a["unit_ids"], "seed": a["seed"] }));
    assert_eq!(r["units_in_sample"], 4);
}

#[test]
fn per_code_view_and_bad_requests_come_back_as_plain_errors() {
    let dir = TempDir::new().unwrap();
    let mut s = setup(&dir);
    let r = ok(&mut s.api, "reliability.run", json!({ "coder_ids": [s.ada, s.ben], "mode": "per_code", "save": false }));
    assert_eq!(r["overall"], Value::Null);
    assert_eq!(r["by_code"].as_array().unwrap().len(), 2);
    assert_eq!(err(&mut s.api, "reliability.run", json!({ "coder_ids": [s.ada] })).code, "invalid");
    assert_eq!(err(&mut s.api, "reliability.run", json!({})).code, "bad_args");
}
