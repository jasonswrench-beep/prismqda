//! The data editor's later tools through the command layer: sorting, tables by group, standing a table on end and laying it across (from the
//! cases and from a file), saving a table, reading a file's columns, merging variables from a file, the groups an analysis can be split into,
//! a weighted analysis, and the three new transformations.

use std::fs;

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn write(dir: &TempDir, name: &str, text: &str) -> String {
    let path = dir.path().join(name);
    fs::write(&path, text).unwrap();
    path.to_string_lossy().to_string()
}

/// A project of six respondents, each a case: Site, Score, a date they were seen, and three waves.
fn world(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let csv = "id,Site,Score,Seen,W1,W2,W3,answer\n\
               r1,North,10,2024-03-14,1,2,3,Felt fine.\n\
               r2,North,12,2024-12-30,4,5,6,Felt well.\n\
               r3,North,14,2021-01-03,7,,9,Felt good.\n\
               r4,South,20,2020-12-31,10,11,12,Felt calm.\n\
               r5,South,22,2022-01-01,13,14,15,Felt happy.\n\
               r6,South,24,2023-06-30,16,17,18,Felt sure.\n";
    let file = write(dir, "people.csv", csv);
    let mapping = json!({
        "id_column": "id",
        "text_columns": ["answer"],
        "attribute_columns": [
            { "name": "Site", "level": "nominal" }, { "name": "Score", "level": "ratio" }, { "name": "Seen", "level": "date" },
            { "name": "W1", "level": "ratio" }, { "name": "W2", "level": "ratio" }, { "name": "W3", "level": "ratio" }
        ]
    });
    ok(&mut a, "import.run", json!({ "path": file, "mapping": mapping }));
    a
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

fn column(a: &mut Api, name: &str) -> Vec<Value> {
    let d = ok(a, "data.get", json!({}));
    let i = d["variables"].as_array().unwrap().iter().position(|v| v["name"] == name).unwrap_or_else(|| panic!("no variable {name}"));
    d["values"].as_array().unwrap().iter().map(|row| row[i].clone()).collect()
}

#[test]
fn the_new_transformations_are_made_and_previewed_through_the_command_layer() {
    let dir = TempDir::new().unwrap();
    let mut a = world(&dir);
    let preview = ok(&mut a, "variable.preview", json!({ "target": "Seen month", "kind": "date_part", "source": "Seen", "part": "month" }));
    assert_eq!((preview["level"].as_str(), preview["cases_with_value"].as_i64()), (Some("ordinal"), Some(6)));
    ok(&mut a, "variable.transform", json!({ "target": "Seen month", "kind": "date_part", "source": "Seen", "part": "month" }));
    assert_eq!(column(&mut a, "Seen month").iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>(), ["3", "12", "1", "12", "1", "6"]);
    ok(&mut a, "variable.transform", json!({ "target": "Pick", "kind": "sample", "size": { "by": "count", "value": 2 }, "seed": 4 }));
    assert_eq!(column(&mut a, "Pick").iter().filter(|v| v == &&json!("1")).count(), 2);
    ok(&mut a, "variable.transform", json!({ "target": "Site mean", "kind": "aggregate", "source": "Score", "by": ["Site"], "function": "mean" }));
    assert_eq!(column(&mut a, "Site mean").iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>(), ["12", "12", "12", "22", "22", "22"]);
    let e = err(&mut a, "variable.transform", json!({ "target": "Bad", "kind": "date_part", "source": "Score", "part": "year" }));
    assert!(e.message.contains("not of dates"), "{}", e.message);
    let e = err(&mut a, "variable.transform", json!({ "target": "Bad", "kind": "sample", "size": { "by": "count", "value": 99 } }));
    assert!(e.message.contains("cannot be drawn from 6"), "{}", e.message);
}

#[test]
fn the_cases_are_sorted_by_the_keys_without_changing_the_project() {
    let dir = TempDir::new().unwrap();
    let mut a = world(&dir);
    let sorted = ok(&mut a, "data.sort", json!({ "keys": [{ "variable": "Score", "descending": true }] }));
    let ids: Vec<i64> = sorted["case_ids"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    let data = ok(&mut a, "data.get", json!({}));
    let name_of = |id: i64| data["cases"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap()["name"].as_str().unwrap().to_string();
    assert_eq!(ids.iter().map(|i| name_of(*i)).collect::<Vec<_>>(), ["r6", "r5", "r4", "r3", "r2", "r1"]);
    assert_eq!(err(&mut a, "data.sort", json!({ "keys": [{ "variable": "Nope" }] })).code, "invalid");
}

#[test]
fn a_table_by_group_is_shown_and_saved_as_csv_and_as_a_workbook() {
    let dir = TempDir::new().unwrap();
    let mut a = world(&dir);
    let spec = json!({ "op": "groups", "by": ["Site"], "summaries": [{ "source": "Score", "function": "mean" }, { "source": "Score", "function": "max" }] });
    let table = ok(&mut a, "data.table", json!({ "spec": spec }));
    assert_eq!(names(&table["columns"]), ["Site", "Cases", "mean of Score", "max of Score"]);
    assert_eq!(table["rows"], json!([["North", "3", "12", "14"], ["South", "3", "22", "24"]]));
    assert_eq!((table["total_rows"].as_i64(), table["left_out"].as_i64()), (Some(2), Some(0)));

    let csv_path = dir.path().join("groups.csv");
    let saved = ok(&mut a, "data.table_save", json!({ "spec": spec, "path": csv_path, "format": "csv" }));
    assert_eq!((saved["rows"].as_i64(), saved["columns"].as_i64()), (Some(2), Some(4)));
    let text = fs::read_to_string(&csv_path).unwrap();
    assert!(text.contains("Site,Cases,mean of Score,max of Score") && text.contains("South,3,22,24"), "{text}");
    // A file already there is never replaced unless asked.
    assert_eq!(err(&mut a, "data.table_save", json!({ "spec": spec, "path": csv_path, "format": "csv" })).code, "already_exists");
    ok(&mut a, "data.table_save", json!({ "spec": spec, "path": csv_path, "format": "csv", "overwrite": true }));
    // The workbook reads back as a table.
    let xlsx = dir.path().join("groups.xlsx");
    ok(&mut a, "data.table_save", json!({ "spec": spec, "path": xlsx, "format": "xlsx" }));
    let back = ok(&mut a, "data.inspect_file", json!({ "path": xlsx }));
    assert_eq!(back["rows"], 2);
    assert_eq!(back["columns"].as_array().unwrap().len(), 4);
    assert_eq!(err(&mut a, "data.table_save", json!({ "spec": spec, "path": dir.path().join("x.docx"), "format": "docx" })).code, "invalid");
}

#[test]
fn the_cases_are_stood_on_end_and_a_long_file_is_laid_across() {
    let dir = TempDir::new().unwrap();
    let mut a = world(&dir);
    let spec = json!({ "op": "stand_on_end", "from": { "kind": "cases" }, "keep": ["Case", "Site"], "measures": ["W1", "W2", "W3"], "index_name": "Wave", "value_name": "Score" });
    let long = ok(&mut a, "data.table", json!({ "spec": spec }));
    assert_eq!(names(&long["columns"]), ["Case", "Site", "Wave", "Score"]);
    assert_eq!(long["total_rows"], 18, "six cases, three waves");
    assert_eq!(long["rows"][0], json!(["r1", "North", "W1", "1"]));
    assert_eq!(long["rows"][7], json!(["r3", "North", "W2", null]), "r3 has no second wave");

    // The long table saved as CSV is a long file; laid across, it is the wide table again.
    let long_path = dir.path().join("long.csv");
    ok(&mut a, "data.table_save", json!({ "spec": spec, "path": long_path, "format": "csv" }));
    let across = ok(&mut a, "data.table", json!({ "spec": { "op": "lay_across", "from": { "kind": "file", "path": long_path }, "id": ["Case", "Site"], "index": "Wave", "values": ["Score"] } }));
    assert_eq!(names(&across["columns"]), ["Case", "Site", "W1", "W2", "W3"]);
    assert_eq!(across["total_rows"], 6);
    assert_eq!(across["rows"][2], json!(["r3", "North", "7", null, "9"]));

    // What cannot be done says why.
    let e = err(&mut a, "data.table", json!({ "spec": { "op": "stand_on_end", "from": { "kind": "cases" }, "keep": [], "measures": ["W1"], "index_name": "I", "value_name": "V" } }));
    assert!(e.message.contains("at least two columns"), "{}", e.message);
    let e = err(&mut a, "data.table", json!({ "spec": { "op": "lay_across", "from": { "kind": "file", "path": "long.csv" }, "id": ["Case"], "index": "Wave", "values": ["Score"] } }));
    assert!(e.message.contains("full path"), "{}", e.message);
    let doc = write(&dir, "notes.txt", "Some words.");
    let e = err(&mut a, "data.inspect_file", json!({ "path": doc }));
    assert!(e.message.contains("not a table") || e.message.contains("document"), "{}", e.message);
}

#[test]
fn variables_are_merged_in_from_a_file_by_a_key() {
    let dir = TempDir::new().unwrap();
    let mut a = world(&dir);
    let file = write(&dir, "extra.csv", "who,age,mood,note\nr1,31,up,a\nR2,40,down,b\nr4,52,,c\nr9,99,up,d\n");
    let read = ok(&mut a, "data.inspect_file", json!({ "path": file }));
    assert_eq!(read["rows"], 4);
    let level = |name: &str| read["columns"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap()["level"].as_str().unwrap().to_string();
    // The level is the import wizard's guess from the values: a few whole numbers read as ordinal, words as nominal.
    assert!(["ordinal", "interval", "ratio"].contains(&level("age").as_str()), "{}", level("age"));
    assert_eq!(level("mood"), "nominal");
    let done = ok(&mut a, "data.merge", json!({ "path": file, "key_column": "who", "columns": [{ "name": "age" }, { "name": "mood", "as": "Mood now", "level": "nominal" }] }));
    assert_eq!((done["cases_matched"].as_i64(), done["cases_unmatched"].as_i64(), done["rows_unmatched"].as_i64()), (Some(3), Some(3), Some(1)), "r1, R2 and r4 match; r9 matches none");
    assert_eq!(done["unmatched_keys"], json!(["r9"]));
    assert_eq!(done["variables_made"], json!(["age", "Mood now"]));
    let age = column(&mut a, "age");
    assert_eq!((age[0].as_str(), age[1].as_str(), age[2].as_str(), age[3].as_str()), (Some("31"), Some("40"), None, Some("52")));
    assert_eq!(column(&mut a, "Mood now")[1], "down");
    // The key itself cannot be brought in, and a missing column or key is named.
    assert_eq!(err(&mut a, "data.merge", json!({ "path": file, "key_column": "who", "columns": [{ "name": "who" }] })).code, "invalid");
    assert!(err(&mut a, "data.merge", json!({ "path": file, "key_column": "nobody", "columns": [{ "name": "age" }] })).message.contains("no column named 'nobody'"));
    // A second merge of the same names is refused, and fills in when asked.
    let again = err(&mut a, "data.merge", json!({ "path": file, "key_column": "who", "columns": [{ "name": "age" }] }));
    assert!(again.message.contains("already a variable named 'age'"), "{}", again.message);
    let filled = ok(&mut a, "data.merge", json!({ "path": file, "key_column": "who", "columns": [{ "name": "age" }], "overwrite": true }));
    assert_eq!(filled["variables_filled"], json!(["age"]));
}

#[test]
fn an_analysis_can_be_weighted_and_the_groups_it_is_split_into_have_rules() {
    let dir = TempDir::new().unwrap();
    let mut a = world(&dir);
    ok(&mut a, "variable.define", json!({ "name": "Times", "level": "ratio" }));
    let data = ok(&mut a, "data.get", json!({}));
    let changes: Vec<Value> = data["cases"].as_array().unwrap().iter().zip(["2", "1", "1", "3", "1", "0"]).map(|(c, w)| json!({ "case_id": c["id"], "value": w })).collect();
    ok(&mut a, "variable.set_values", json!({ "variable": "Times", "changes": changes }));
    let plain = ok(&mut a, "stats.run", json!({ "procedure": "describe", "variables": ["attr:Score"] }));
    let weighted = ok(&mut a, "stats.run", json!({ "procedure": "describe", "variables": ["attr:Score"], "options": { "weight": "Times" } }));
    assert!(plain["weight"].is_null());
    assert_eq!((weighted["weight"]["counted"].as_i64(), weighted["weight"]["cases"].as_i64(), weighted["weight"]["left_out"].as_i64()), (Some(8), Some(6), Some(1)));
    assert_eq!(weighted["n_used"], 8);
    // The mean of 10 10 12 14 20 20 20 22 is 16.
    assert!((weighted["details"]["descriptives"]["mean"].as_f64().unwrap() - 16.0).abs() < 1e-12);
    let e = err(&mut a, "stats.run", json!({ "procedure": "describe", "variables": ["attr:Score"], "options": { "weight": "Site" } }));
    assert!(e.message.contains("not a variable of numbers"), "{}", e.message);

    let plan = ok(&mut a, "stats.split_groups", json!({ "variable": "Site" }));
    assert_eq!(plan["groups"].as_array().unwrap().len(), 2);
    assert_eq!(plan["groups"][0]["rule"], "`Site` == \"North\"");
    let rule = ok(&mut a, "variable.rule", json!({ "formula": plan["groups"][1]["rule"] }));
    assert_eq!(rule["matched"], 3, "the rule picks the group");
    assert!(err(&mut a, "stats.split_groups", json!({ "variable": "Seen" })).message.contains("holds dates"));
}
