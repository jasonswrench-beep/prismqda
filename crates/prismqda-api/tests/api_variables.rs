//! The data editor's commands: the variables of cases, their values, and transformations, with the refusals as plain errors.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn setup(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let path = dir.path().join("survey.csv");
    let mut csv = String::from("id,group,score,age,answer\n");
    for (i, (g, s, ag)) in [("A", 10, 21), ("A", 12, 25), ("A", 11, 33), ("B", 15, 29), ("B", 17, 38), ("B", 16, 44)].iter().enumerate() {
        csv.push_str(&format!("P{i},{g},{s},{ag},I felt welcome here.\n"));
    }
    std::fs::write(&path, csv).unwrap();
    let mapping = json!({ "id_column": "id", "text_columns": ["answer"], "attribute_columns": [
        { "name": "group", "level": "nominal" }, { "name": "score", "level": "ratio" }, { "name": "age", "level": "ratio" }] });
    ok(&mut a, "import.run", json!({ "path": path, "mapping": mapping }));
    a
}

#[test]
fn the_data_view_lists_cases_variables_and_values() {
    let dir = TempDir::new().unwrap();
    let mut a = setup(&dir);
    let d = ok(&mut a, "data.get", json!({}));
    assert_eq!(d["cases"].as_array().unwrap().len(), 6);
    let names: Vec<&str> = d["variables"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"group") && names.contains(&"score") && names.contains(&"age"), "{names:?}");
    assert_eq!(d["values"].as_array().unwrap().len(), 6);
}

#[test]
fn a_variable_is_defined_filled_transformed_and_undone_through_the_commands() {
    let dir = TempDir::new().unwrap();
    let mut a = setup(&dir);
    let def = ok(&mut a, "variable.define", json!({ "name": "Trained", "level": "nominal", "value_labels": { "1": "Yes" }, "missing_codes": ["99"] }));
    assert_eq!(def["name"], "Trained");
    let cases = ok(&mut a, "data.get", json!({}))["cases"].as_array().unwrap().clone();
    let id0 = cases[0]["id"].as_i64().unwrap();
    assert_eq!(ok(&mut a, "variable.set_values", json!({ "variable": "Trained", "changes": [{ "case_id": id0, "value": "1" }] }))["changed"], 1);

    let preview = ok(&mut a, "variable.preview", json!({ "target": "Score z", "kind": "standardize", "source": "score" }));
    assert_eq!(preview["cases_with_value"], 6);
    assert!(ok(&mut a, "data.get", json!({}))["variables"].as_array().unwrap().iter().all(|v| v["name"] != "Score z"), "a preview saves nothing");
    let made = ok(&mut a, "variable.transform", json!({ "target": "Score z", "kind": "standardize", "source": "score" }));
    assert_eq!((made["created"].as_bool(), made["level"].as_str()), (Some(true), Some("interval")));
    let cut = ok(&mut a, "variable.transform", json!({ "target": "Age band", "kind": "cut", "source": "age", "cuts": [20, 30, 50] }));
    assert_eq!(cut["level"], "ordinal");
    assert_eq!(cut["value_labels"]["1"], "[20, 30]", "a cut says what its ranges are: {cut}");
    let recode = ok(&mut a, "variable.transform", json!({ "target": "Team", "kind": "recode", "source": "group", "rules": [{ "from": ["A"], "to": "Alpha" }], "otherwise": "keep" }));
    assert_eq!(recode["cases_with_value"], 6);
    let formula = ok(&mut a, "variable.transform", json!({ "target": "Total", "kind": "compute", "formula": "score + age" }));
    assert_eq!(formula["preview"][0]["value"], "31");

    let rule = ok(&mut a, "variable.rule", json!({ "formula": "group == \"B\" & age > 30" }));
    assert_eq!((rule["matched"].as_i64(), rule["total"].as_i64()), (Some(2), Some(6)));

    ok(&mut a, "variable.recompute", json!({ "name": "Total" }));
    assert_eq!(err(&mut a, "variable.recompute", json!({ "name": "score" })).code, "invalid");

    // The variable made and the edit are taken back one at a time.
    ok(&mut a, "undo", json!({}));
    ok(&mut a, "undo", json!({}));
    ok(&mut a, "variable.delete", json!({ "name": "Trained" }));
    ok(&mut a, "undo", json!({}));
    let names: Vec<String> = ok(&mut a, "data.get", json!({}))["variables"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"Trained".to_string()), "the deleted variable is back: {names:?}");
}

#[test]
fn the_refusals_come_back_as_plain_errors_with_a_code() {
    let dir = TempDir::new().unwrap();
    let mut a = setup(&dir);
    assert_eq!(err(&mut a, "variable.define", json!({ "name": "score", "level": "ratio" })).code, "invalid", "a name already in use");
    assert_eq!(err(&mut a, "variable.define", json!({ "level": "ratio" })).code, "bad_args", "no name");
    assert_eq!(err(&mut a, "variable.transform", json!({ "target": "X", "kind": "compute", "formula": "nothing + 1" })).code, "invalid");
    assert_eq!(err(&mut a, "variable.transform", json!({ "target": "X", "kind": "magic" })).code, "bad_args");
    assert_eq!(err(&mut a, "variable.set_values", json!({ "variable": "score", "changes": [{ "case_id": 99999, "value": "1" }] })).code, "not_found");
    assert_eq!(err(&mut a, "variable.rule", json!({ "formula": "score * 2" })).code, "invalid");
    let used = ok(&mut a, "stats.run", json!({ "procedure": "describe", "variables": ["attr:score"], "options": { "filter": "group == \"A\"" } }));
    assert_eq!(used["n_used"], 3, "an analysis can be limited by a rule");
}
