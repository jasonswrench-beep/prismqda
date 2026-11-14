//! The Explore commands through the command layer, on the Urban Mobility sample (ten codings by one coder, two cases).

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn sample(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada Lovelace").without_backup_on_open();
    ok(&mut a, "sample.create", json!({ "id": "urban-mobility", "path": dir.path().join("Urban.pqda") }));
    a
}

fn code(api: &mut Api, name: &str) -> i64 {
    ok(api, "code.list", Value::Null).as_array().unwrap().iter().find(|c| c["name"] == name).unwrap()["id"].as_i64().unwrap()
}

#[test]
fn the_summary_counts_a_parent_with_its_sub_codes() {
    let dir = TempDir::new().unwrap();
    let mut a = sample(&dir);
    let rows = ok(&mut a, "explore.summary", json!({}));
    let by = |name: &str| rows.as_array().unwrap().iter().find(|r| r["name"] == name).unwrap().clone();
    let reliability = by("Reliability");
    assert_eq!((reliability["assignments"].clone(), reliability["documents"].clone(), reliability["cases"].clone(), reliability["coders"].clone()), (json!(2), json!(2), json!(1), json!(1)));
    let barriers = by("Barriers & Challenges");
    assert_eq!((barriers["own_assignments"].clone(), barriers["assignments"].clone(), barriers["documents"].clone(), barriers["cases"].clone()), (json!(0), json!(6), json!(4), json!(2)), "Barriers & Challenges: Cost 2, Reliability 2, Accessibility 1, Safety 1; four documents; two cases");
}

#[test]
fn a_matrix_and_the_evidence_behind_its_cell_agree() {
    let dir = TempDir::new().unwrap();
    let mut a = sample(&dir);
    let (reliability, cost) = (code(&mut a, "Reliability"), code(&mut a, "Cost"));
    let spec = json!({ "rows": { "kind": "cases" }, "code_ids": [reliability, cost], "include_descendants": true });
    let m = ok(&mut a, "explore.matrix", spec.clone());
    let rows = m["rows"].as_array().unwrap();
    assert_eq!(rows.iter().map(|r| r["label"].as_str().unwrap()).collect::<Vec<_>>(), ["Commuters", "Urban Residents"]);
    let residents = rows.iter().find(|r| r["label"] == "Urban Residents").unwrap();
    assert_eq!(residents["cells"][0]["assignments"], 2);
    assert_eq!(residents["total"]["assignments"], 3);
    assert_eq!(m["total"]["excerpts"], 4);
    assert_eq!(m["universe"]["assignments"], 10);

    let ev = ok(&mut a, "explore.matrix_evidence", json!({ "spec": spec, "row_key": residents["key"], "code_id": reliability }));
    let ids = ev["coding_ids"].as_array().unwrap();
    assert_eq!(ids.len() as i64, residents["cells"][0]["assignments"].as_i64().unwrap(), "the evidence is what the cell counted");
    let shown = ok(&mut a, "segments.list", json!({ "coding_ids": ids, "limit": 10 }));
    assert_eq!(shown["total"], 2);
    assert!(shown["segments"].as_array().unwrap().iter().all(|s| s["code_name"] == "Reliability"));
}

#[test]
fn an_unknown_case_variable_and_an_empty_query_are_refused_with_their_reason() {
    let dir = TempDir::new().unwrap();
    let mut a = sample(&dir);
    let cost = code(&mut a, "Cost");
    let e = err(&mut a, "explore.matrix", json!({ "rows": { "kind": "case_variable", "name": "Nope" }, "code_ids": [cost], "include_descendants": true }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("no case variable is named 'Nope'"), "{e:?}");
    let e = err(&mut a, "explore.query", json!({ "rule": { "type": "group", "match": "any", "items": [] } }));
    assert!(e.message.contains("no conditions yet"), "{e:?}");
    let e = err(&mut a, "explore.matrix", json!({ "rows": { "kind": "cases" }, "code_ids": [999999], "include_descendants": true }));
    assert_eq!(e.code, "not_found");
    assert_eq!(ok(&mut a, "case.variables", Value::Null), json!([]), "the sample's cases have no variables");
}

#[test]
fn a_query_returns_excerpts_with_the_codings_that_matched_and_relations_agree_with_the_matrix() {
    let dir = TempDir::new().unwrap();
    let mut a = sample(&dir);
    let (reliability, cost) = (code(&mut a, "Reliability"), code(&mut a, "Cost"));
    let q = ok(&mut a, "explore.query", json!({ "rule": { "type": "group", "match": "any", "items": [{ "type": "code", "code_ids": [reliability, cost] }] } }));
    assert_eq!((q["total"].clone(), q["assignments"].clone(), q["truncated"].clone()), (json!(4), json!(4), json!(false)));
    assert!(q["items"].as_array().unwrap().iter().all(|i| i["codings"].as_array().unwrap().len() == 1 && !i["text"].as_str().unwrap().is_empty()));

    let r = ok(&mut a, "explore.relations", json!({ "code_ids": [reliability, cost], "mode": "document", "include_descendants": true, "same_coder": true }));
    assert_eq!(r["cells"], json!([[2, 1], [1, 2]]), "both codes are in one document (Alex's interview)");
    let ev = ok(&mut a, "explore.relations_evidence", json!({ "spec": { "code_ids": [reliability, cost], "mode": "document", "include_descendants": true, "same_coder": true }, "a": reliability, "b": cost }));
    assert_eq!(ev["coding_ids"].as_array().unwrap().len(), 2);

    let profile = ok(&mut a, "explore.profile", json!({ "code_id": reliability }));
    assert_eq!(profile["summary"]["assignments"], 2);
    assert_eq!(profile["by_document"].as_array().unwrap().len(), 2);
}
