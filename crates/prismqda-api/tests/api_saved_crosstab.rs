//! Saved and frozen queries, date clauses, crosstabs, and the month rows of the code matrix, through the command layer.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

/// A project with four survey respondents (two groups, two sites) and one code used on two of them.
fn setup(dir: &TempDir) -> (Api, i64, i64) {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let path = dir.path().join("survey.csv");
    std::fs::write(&path, "id,group,site,answer\nP1,A,North,I felt welcome here.\nP2,A,South,The cost was high.\nP3,B,North,Staff were warm.\nP4,B,South,It is far away.\n").unwrap();
    let mapping = json!({ "id_column": "id", "text_columns": ["answer"], "attribute_columns": [{ "name": "group", "level": "nominal" }, { "name": "site", "level": "nominal" }] });
    let out = ok(&mut a, "import.run", json!({ "path": path, "mapping": mapping }));
    let source = out["outcome"]["source_ids"][0].as_i64().unwrap();
    let code = ok(&mut a, "code.add", json!({ "name": "Welcome" }))["id"].as_i64().unwrap();
    let text = ok(&mut a, "source.get", json!({ "id": source }))["text"].as_str().unwrap().to_string();
    for phrase in ["felt welcome", "were warm"] {
        let start = text.chars().take(text.find(phrase).unwrap()).count() as i64;
        ok(&mut a, "coding.apply", json!({ "source_id": source, "start": start, "end": start + phrase.chars().count() as i64, "code_id": code }));
    }
    (a, source, code)
}

fn rule(code: i64) -> Value {
    json!({ "type": "group", "match": "any", "items": [{ "type": "code", "code_ids": [code] }] })
}

#[test]
fn a_crosstab_of_two_case_variables_counts_cases_and_opens_to_them() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, _) = setup(&dir);
    let vars = ok(&mut a, "case.variables", json!({}));
    let names: Vec<&str> = vars.as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"group") && names.contains(&"site"), "{vars}");
    let group = vars.as_array().unwrap().iter().find(|v| v["name"] == "group").unwrap();
    assert!(group["dates"].is_i64() && group["numbers"].is_i64(), "a variable says how many of its values read as dates and numbers");

    let spec = json!({ "row_variable": "group", "column_variable": "site" });
    let t = ok(&mut a, "explore.crosstab", spec.clone());
    assert_eq!(t["cells"], json!([[1, 1], [1, 1]]));
    assert_eq!(t["total"], 4);
    assert_eq!(t["rows"]["values"][0]["label"], "A");
    let cases = ok(&mut a, "explore.crosstab_cases", json!({ "spec": spec, "row_key": "A", "column_key": "North" }));
    assert_eq!(cases.as_array().unwrap().len(), 1);
    assert_eq!(err(&mut a, "explore.crosstab", json!({ "row_variable": "nope", "column_variable": "site" })).code, "invalid");
    assert_eq!(err(&mut a, "explore.crosstab", json!({ "row_variable": "group" })).code, "bad_args");
}

#[test]
fn date_clauses_and_the_month_rows_work_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let (mut a, _, code) = setup(&dir);
    let all_time = json!({ "rule": { "type": "group", "match": "all", "items": [
        { "type": "code", "code_ids": [code] }, { "type": "coded_between", "from": "2000-01-01", "to": "2999-12-31" } ] }, "result_unit": "assignments" });
    assert_eq!(ok(&mut a, "explore.query", all_time)["total"], 2);
    let long_ago = json!({ "rule": { "type": "group", "match": "any", "items": [{ "type": "coded_between", "from": "2000-01-01", "to": "2000-12-31" }] } });
    assert_eq!(ok(&mut a, "explore.query", long_ago)["total"], 0);
    let bad = json!({ "rule": { "type": "group", "match": "any", "items": [{ "type": "coded_between", "from": "1/2/2024" }] } });
    assert_eq!(err(&mut a, "explore.query", bad).code, "invalid");

    let m = ok(&mut a, "explore.matrix", json!({ "rows": { "kind": "month_coded" }, "code_ids": [code] }));
    let rows = m["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "both codings were made this month");
    assert_eq!(rows[0]["cells"][0]["assignments"], 2);
    assert!(rows[0]["key"].as_str().unwrap().starts_with("month:20"));
}

#[test]
fn saved_queries_are_kept_frozen_compared_and_taken_back_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, code) = setup(&dir);
    let spec = json!({ "rule": rule(code), "result_unit": "assignments" });
    let saved = ok(&mut a, "query.save", json!({ "name": "Welcome talk", "note": "first pass", "draft": { "ui": 1 }, "spec": spec }));
    let id = saved["id"].as_i64().unwrap();
    assert_eq!((saved["name"].as_str(), saved["draft"]["ui"].as_i64()), (Some("Welcome talk"), Some(1)));
    assert_eq!(err(&mut a, "query.save", json!({ "name": "welcome TALK", "spec": spec })).code, "invalid");
    assert_eq!(err(&mut a, "query.save", json!({ "name": "Nothing", "spec": { "rule": 5 } })).code, "invalid");
    assert_eq!(ok(&mut a, "query.list", json!({})).as_array().unwrap().len(), 1);

    let run = ok(&mut a, "query.run_saved", json!({ "id": id }));
    assert_eq!(run["result"]["total"], 2);
    assert_eq!(run["query"]["name"], "Welcome talk");

    let frozen = ok(&mut a, "query.freeze", json!({ "id": id }));
    assert_eq!((frozen["codings"].as_i64(), frozen["results"].as_i64()), (Some(2), Some(2)));
    assert_eq!(ok(&mut a, "query.list", json!({}))[0]["frozen_count"], 2);
    assert_eq!(err(&mut a, "query.update", json!({ "id": id, "draft": {}, "spec": spec })).code, "invalid", "a frozen query's question is fixed");
    assert_eq!(err(&mut a, "query.update", json!({ "id": id, "draft": {} })).code, "bad_args", "the draft and the spec go together");

    // One more coding of the code: the frozen answer stays, the comparison sees it.
    let text = ok(&mut a, "source.get", json!({ "id": source }))["text"].as_str().unwrap().to_string();
    let start = text.chars().take(text.find("far away").unwrap()).count() as i64;
    ok(&mut a, "coding.apply", json!({ "source_id": source, "start": start, "end": start + 8, "code_id": code }));
    assert_eq!(ok(&mut a, "query.run_saved", json!({ "id": id }))["result"]["total"], 2, "frozen: still the two it found");
    let cmp = ok(&mut a, "query.compare", json!({ "id": id }));
    assert_eq!((cmp["frozen"].as_i64(), cmp["now"].as_i64(), cmp["added"].as_i64(), cmp["no_longer_found"].as_i64()), (Some(2), Some(3), Some(1), Some(0)));
    ok(&mut a, "query.unfreeze", json!({ "id": id }));
    assert_eq!(ok(&mut a, "query.run_saved", json!({ "id": id }))["result"]["total"], 3, "no longer frozen: it follows the coding");
    assert_eq!(err(&mut a, "query.compare", json!({ "id": id })).code, "invalid");

    ok(&mut a, "query.update", json!({ "id": id, "name": "Welcome" }));
    ok(&mut a, "query.delete", json!({ "id": id }));
    assert_eq!(err(&mut a, "query.get", json!({ "id": id })).code, "not_found");
    let undone = ok(&mut a, "undo", json!({}));
    assert_eq!(undone["action"], "query.delete");
    assert_eq!(ok(&mut a, "query.get", json!({ "id": id }))["name"], "Welcome");
}
