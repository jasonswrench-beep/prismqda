//! The Statistics commands through the command layer: the variables of cases, a test run on them, and the refusals.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

/// Eight respondents in two groups with a score and an age, and a code on three of them.
fn setup(dir: &TempDir) -> (Api, i64) {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let path = dir.path().join("survey.csv");
    let mut csv = String::from("id,group,score,age,answer\n");
    let rows = [("P1", "A", 10, 21), ("P2", "A", 12, 25), ("P3", "A", 11, 33), ("P4", "A", 13, 41), ("P5", "B", 15, 29), ("P6", "B", 17, 38), ("P7", "B", 16, 44), ("P8", "B", 18, 52)];
    for (id, group, score, age) in rows {
        csv.push_str(&format!("{id},{group},{score},{age},I felt welcome here number {id}.\n"));
    }
    std::fs::write(&path, csv).unwrap();
    let mapping = json!({ "id_column": "id", "text_columns": ["answer"], "attribute_columns": [
        { "name": "group", "level": "nominal" }, { "name": "score", "level": "ratio" }, { "name": "age", "level": "ratio" }] });
    let out = ok(&mut a, "import.run", json!({ "path": path, "mapping": mapping }));
    let source = out["outcome"]["source_ids"][0].as_i64().unwrap();
    let code = ok(&mut a, "code.add", json!({ "name": "Welcome" }))["id"].as_i64().unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 2, "end": 8, "code_id": code }));
    (a, code)
}

#[test]
fn the_variables_list_the_case_variables_and_the_codes() {
    let dir = TempDir::new().unwrap();
    let (mut a, code) = setup(&dir);
    let v = ok(&mut a, "stats.variables", json!({}));
    assert_eq!(v["cases"], 8);
    let keys: Vec<&str> = v["variables"].as_array().unwrap().iter().map(|x| x["key"].as_str().unwrap()).collect();
    for want in ["attr:group", "attr:score", "attr:age"] {
        assert!(keys.contains(&want), "{want} is offered: {keys:?}");
    }
    assert!(keys.contains(&format!("code:{code}:present").as_str()) && keys.contains(&format!("code:{code}:count").as_str()));
    let score = v["variables"].as_array().unwrap().iter().find(|x| x["key"] == "attr:score").unwrap();
    assert_eq!((score["measure"].as_str(), score["valid"].as_i64()), (Some("scale"), Some(8)));
}

#[test]
fn a_test_runs_on_the_variables_and_says_what_it_used() {
    let dir = TempDir::new().unwrap();
    let (mut a, _) = setup(&dir);
    let r = ok(&mut a, "stats.run", json!({ "procedure": "independent_t", "variables": ["attr:score", "attr:group"] }));
    assert_eq!(r["n_used"], 8);
    assert_eq!(r["groups"][0]["label"], "A");
    assert_eq!(r["groups"][1]["n"], 4);
    // A = 10, 12, 11, 13 and B = 15, 17, 16, 18: the means differ by 5.
    assert!((r["details"]["t_test"]["estimate"].as_f64().unwrap() + 5.0).abs() < 1e-9);
    assert!(r["details"]["t_test"]["p"].as_f64().unwrap() < 0.01);
    assert!(r["assumptions"].as_array().unwrap().iter().any(|x| x["id"] == "sample_size"));

    let c = ok(&mut a, "stats.run", json!({ "procedure": "pearson", "variables": ["attr:score", "attr:age"], "options": { "level": 0.9 } }));
    assert_eq!(c["details"]["correlation"]["level"], 0.9);
}

#[test]
fn the_refusals_come_back_as_plain_errors_with_a_code() {
    let dir = TempDir::new().unwrap();
    let (mut a, _) = setup(&dir);
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "independent_t", "variables": ["attr:score"] })).code, "invalid");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "no_such_test", "variables": [] })).code, "bad_args");
    assert_eq!(err(&mut a, "stats.run", json!({ "variables": ["attr:score"] })).code, "bad_args");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "describe", "variables": ["attr:missing"] })).code, "invalid");
}

#[test]
fn a_regression_and_a_bootstrap_run_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let (mut a, _) = setup(&dir);
    let r = ok(
        &mut a,
        "stats.run",
        json!({ "procedure": "linear_regression", "variables": ["attr:score", "attr:age", "attr:group"], "options": { "bootstrap": { "resamples": 200, "seed": 5 } } }),
    );
    assert_eq!(r["n_used"], 8);
    assert_eq!(r["details"]["models"][0]["n"], 8);
    assert_eq!(r["details"]["terms"].as_array().unwrap().len(), 2, "age, and group B against group A");
    assert_eq!(r["details"]["terms"][1]["dummy"], true);
    assert_eq!(r["details"]["bootstrap"]["seed"], 5);
    assert_eq!(r["details"]["bootstrap"]["items"].as_array().unwrap().len(), 4, "the intercept, two terms, and R squared");
    let again = ok(
        &mut a,
        "stats.run",
        json!({ "procedure": "linear_regression", "variables": ["attr:score", "attr:age", "attr:group"], "options": { "bootstrap": { "resamples": 200, "seed": 5 } } }),
    );
    assert_eq!(r["details"]["bootstrap"], again["details"]["bootstrap"], "the same seed gives the same intervals");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "describe", "variables": ["attr:age"], "options": { "bootstrap": { "resamples": 10 } } })).code, "invalid");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "linear_regression", "variables": ["attr:score"] })).code, "invalid");
}

#[test]
fn the_table_for_a_script_has_every_case_and_the_variables_asked_for_in_order() {
    let dir = TempDir::new().unwrap();
    let (mut a, code) = setup(&dir);
    let present = format!("code:{code}:present");
    let t = ok(&mut a, "stats.table", json!({ "keys": ["attr:score", "attr:group", present] }));
    assert_eq!(t["cases"].as_array().unwrap().len(), 8);
    let columns = t["columns"].as_array().unwrap();
    assert_eq!(columns.iter().map(|c| c["key"].as_str().unwrap()).collect::<Vec<_>>(), ["attr:score", "attr:group", present.as_str()]);
    assert_eq!(columns[0]["numeric"], true);
    assert_eq!(columns[1]["numeric"], false);
    assert_eq!(columns[0]["values"][0], "10");
    assert_eq!(columns[1]["values"][4], "B");
    assert_eq!(columns[2]["name"], "Welcome", "a code variable is named for its code");
    assert_eq!(columns[2]["values"].as_array().unwrap().len(), 8);
    assert_eq!(err(&mut a, "stats.table", json!({ "keys": ["attr:missing"] })).code, "invalid");
    assert_eq!(err(&mut a, "stats.table", json!({})).code, "bad_args");
}

#[test]
fn a_logistic_regression_runs_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let (mut a, _) = setup(&dir);
    let r = ok(&mut a, "stats.run", json!({ "procedure": "logistic_regression", "variables": ["attr:group", "attr:age"], "options": { "event": "B", "bootstrap": { "resamples": 200, "seed": 3 } } }));
    assert_eq!(r["n_used"], 8);
    assert_eq!(r["details"]["event"]["value"], "B");
    assert_eq!(r["details"]["other"]["value"], "A");
    assert_eq!(r["details"]["models"][0]["events"], 4);
    assert!(r["details"]["models"][0]["terms"][1]["odds_ratio"].as_f64().unwrap() > 1.0, "the older, the likelier group B");
    assert_eq!(r["details"]["bootstrap"]["items"].as_array().unwrap().len(), 3, "the intercept, age, and the area under the curve");
    let default = ok(&mut a, "stats.run", json!({ "procedure": "logistic_regression", "variables": ["attr:group", "attr:age"] }));
    assert_eq!(default["details"]["event"]["value"], "B", "the later of the two values is modelled by default");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "logistic_regression", "variables": ["attr:group"] })).code, "invalid");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "logistic_regression", "variables": ["attr:score", "attr:age"] })).code, "invalid", "a number with many values is not a yes-or-no");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "logistic_regression", "variables": ["attr:group", "attr:age"], "options": { "event": "C" } })).code, "invalid");
}

#[test]
fn a_curve_fit_runs_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let (mut a, _) = setup(&dir);
    let r = ok(&mut a, "stats.run", json!({ "procedure": "curve_fit", "variables": ["attr:score", "attr:age"] }));
    assert_eq!(r["n_used"], 8);
    assert_eq!(r["details"]["fits"].as_array().unwrap().len(), 8);
    assert_eq!(r["charts"][0]["kind"], "curves");
    assert_eq!(r["charts"][0]["curves"].as_array().unwrap().len(), 8);
    let two = ok(&mut a, "stats.run", json!({ "procedure": "curve_fit", "variables": ["attr:score", "attr:age"], "options": { "curves": ["power", "linear"] } }));
    assert_eq!(two["details"]["fits"][0]["kind"], "power");
    assert_eq!(two["details"]["fits"].as_array().unwrap().len(), 2);
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "curve_fit", "variables": ["attr:score"] })).code, "invalid");
    assert_eq!(err(&mut a, "stats.run", json!({ "procedure": "curve_fit", "variables": ["attr:score", "attr:age"], "options": { "curves": ["wiggly"] } })).code, "bad_args");
}

#[test]
fn the_whole_analysis_can_be_bootstrapped_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let (mut a, _) = setup(&dir);
    let r = ok(&mut a, "stats.run", json!({ "procedure": "one_way_anova", "variables": ["attr:score", "attr:group"], "options": { "bootstrap": { "resamples": 200, "seed": 2, "analysis": true } } }));
    let items = r["details"]["bootstrap_analysis"]["items"].as_array().unwrap();
    assert!(items.iter().any(|i| i["path"] == "anova/test/p" && i["below_05"].is_number()));
    assert!(r["details"].get("bootstrap").is_none());
    let again = ok(&mut a, "stats.run", json!({ "procedure": "one_way_anova", "variables": ["attr:score", "attr:group"], "options": { "bootstrap": { "resamples": 200, "seed": 2, "analysis": true } } }));
    assert_eq!(r["details"]["bootstrap_analysis"], again["details"]["bootstrap_analysis"], "the same seed gives the same intervals");
    // Eight cases cannot be grouped into five groups by a variable with two values.
    let e = err(&mut a, "stats.run", json!({ "procedure": "pearson", "variables": ["attr:score", "attr:age"], "options": { "bootstrap": { "resamples": 200, "cluster": "attr:group" } } }));
    assert_eq!(e.code, "invalid");
}

#[test]
fn the_notebook_is_kept_in_the_project_noted_and_run_again_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let (mut a, _) = setup(&dir);
    assert_eq!(ok(&mut a, "stats.notebook", json!({})).as_array().unwrap().len(), 0);
    let page = ok(&mut a, "stats.notebook.run", json!({ "request": { "procedure": "describe", "variables": ["attr:score"] }, "section": "Scores" }));
    assert_eq!((page["kind"].as_str(), page["section"].as_str()), (Some("analysis"), Some("Scores")));
    assert_eq!(page["result"]["procedure"], "describe");
    let id = page["id"].as_i64().unwrap();
    let note = ok(&mut a, "stats.notebook.note", json!({ "title": "Plan", "body": "Look at age next." }));
    assert_eq!(note["kind"], "note");
    let updated = ok(&mut a, "stats.notebook.update", json!({ "id": id, "note": "Scores are spread out." }));
    assert_eq!(updated["note"], "Scores are spread out.");
    let moved = ok(&mut a, "stats.notebook.move", json!({ "id": note["id"], "up": true }));
    assert_eq!(moved[0]["kind"], "note", "the page of notes moved above the analysis");
    // Nothing changed, so running again finds the same and there is no earlier run to compare with.
    let again = ok(&mut a, "stats.notebook.rerun", json!({ "id": id }));
    assert_eq!(again["changed"], false);
    assert!(ok(&mut a, "stats.notebook.previous", json!({ "id": id })).is_null());
    let all = ok(&mut a, "stats.notebook.rerun_all", json!({}));
    assert_eq!(all.as_array().unwrap().len(), 1, "only the analysis is run");
    assert_eq!(err(&mut a, "stats.notebook.rerun", json!({ "id": note["id"] })).code, "invalid");
    ok(&mut a, "stats.notebook.remove", json!({ "id": id }));
    assert_eq!(ok(&mut a, "stats.notebook", json!({})).as_array().unwrap().len(), 1);
    assert_eq!(err(&mut a, "stats.notebook.run", json!({ "request": { "procedure": "describe", "variables": ["attr:missing"] } })).code, "invalid");
    assert_eq!(err(&mut a, "stats.notebook.update", json!({ "id": 9999, "note": "x" })).code, "not_found");
}
