//! Ratings, cases made by hand, and the history of a coding, driven the way the interface drives them.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn api() -> Api {
    Api::new("Ada Lovelace").without_backup_on_open()
}

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

struct Study {
    api: Api,
    source: i64,
    code: i64,
    coding: i64,
}

fn study(dir: &TempDir) -> Study {
    let mut a = api();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Study" }));
    let path = dir.path().join("int.txt");
    std::fs::write(&path, "I felt welcome from the first day.").unwrap();
    let source = ok(&mut a, "import.run", json!({ "path": path }))["outcome"]["source_ids"][0].as_i64().unwrap();
    // Ratings live on rated codes, so this one is made rated.
    let code = ok(&mut a, "code.add", json!({ "name": "Welcome", "rated": true }))["id"].as_i64().unwrap();
    let coding = ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 2, "end": 14, "code_id": code }))["id"].as_i64().unwrap();
    Study { api: a, source, code, coding }
}

fn listed(s: &mut Study) -> Value {
    ok(&mut s.api, "coding.list", json!({ "source_id": s.source }))[0].clone()
}

// ---- ratings -----------------------------------------------------------------------------------

#[test]
fn coding_list_rows_carry_a_rating_that_is_null_until_one_is_given() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let row = listed(&mut s);
    assert!(row.as_object().unwrap().contains_key("rating"), "{row}");
    assert_eq!(row["rating"], Value::Null);

    assert_eq!(ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": 4 })), json!({}));
    assert_eq!(listed(&mut s)["rating"], 4);

    ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": null }));
    assert_eq!(listed(&mut s)["rating"], Value::Null);
}

#[test]
fn the_row_keeps_every_field_it_had_and_gains_the_rating() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let row = listed(&mut s);
    let mut keys: Vec<&str> = row.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        ["ai_run_id", "code_id", "coder_id", "created_at", "end", "id", "layer", "origin", "rating", "rationale", "source_id", "start", "unit_id"]
    );
}

#[test]
fn only_whole_numbers_from_one_to_five_or_null_are_accepted() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": 3 }));
    for bad in [0, 6, -2, 99] {
        let e = err(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": bad }));
        assert_eq!(e.code, "invalid", "{bad}");
        assert!(e.message.contains("1 to 5"), "{}", e.message);
    }
    // Wrong shapes are the interface's mistake, and must not quietly clear or change a rating.
    for bad in [json!(4.5), json!("4"), json!(true), json!([4]), json!({})] {
        assert_eq!(err(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": bad })).code, "bad_args", "{bad}");
    }
    assert_eq!(err(&mut s.api, "coding.rating", json!({ "id": s.coding })).code, "bad_args", "leaving the rating out is not the same as null");
    assert_eq!(err(&mut s.api, "coding.rating", json!({ "rating": 3 })).code, "bad_args");
    assert_eq!(listed(&mut s)["rating"], 3, "none of the failures changed it");
}

#[test]
fn rating_a_coding_that_is_not_there_is_not_found() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    assert_eq!(err(&mut s.api, "coding.rating", json!({ "id": 999, "rating": 3 })).code, "not_found");
}

#[test]
fn a_rating_can_be_undone_through_the_api() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": 5 }));
    ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": 2 }));
    assert_eq!(ok(&mut s.api, "undo.next", Value::Null)["next"], "Restored the coding's earlier rating");
    let undone = ok(&mut s.api, "undo", Value::Null);
    assert_eq!(undone["action"], "coding.rating");
    assert_eq!(undone["what"], "Restored the coding's earlier rating");
    assert_eq!(listed(&mut s)["rating"], 5);
    ok(&mut s.api, "undo", Value::Null);
    assert_eq!(listed(&mut s)["rating"], Value::Null);
}

#[test]
fn the_segments_listing_carries_the_rating_too() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": 4 }));
    let out = ok(&mut s.api, "segments.list", json!({}));
    assert_eq!(out["segments"][0]["rating"], 4, "{out}");
    ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": null }));
    let out = ok(&mut s.api, "segments.list", json!({}));
    assert_eq!(out["segments"][0]["rating"], Value::Null);
}

// ---- cases -------------------------------------------------------------------------------------

#[test]
fn a_case_is_added_assigned_listed_and_unassigned() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let case = ok(&mut s.api, "case.add", json!({ "name": "Urban Residents", "notes": "Lives in a city" }));
    let id = case["id"].as_i64().unwrap();
    assert_eq!(case, json!({ "id": id, "name": "Urban Residents", "notes": "Lives in a city", "sources": 0, "units": 0, "source_ids": [] }));

    let after = ok(&mut s.api, "case.assign", json!({ "case_id": id, "source_id": s.source }));
    assert_eq!(after["source_ids"], json!([s.source]));
    assert_eq!(after["sources"], 1);
    assert_eq!(ok(&mut s.api, "case.list", Value::Null), json!([after]));

    // Assigning twice changes nothing.
    ok(&mut s.api, "case.assign", json!({ "case_id": id, "source_id": s.source }));
    assert_eq!(ok(&mut s.api, "case.list", Value::Null)[0]["sources"], 1);

    let gone = ok(&mut s.api, "case.unassign", json!({ "case_id": id, "source_id": s.source }));
    assert_eq!(gone["source_ids"], json!([]));
    assert_eq!(gone["sources"], 0);
    assert_eq!(ok(&mut s.api, "case.list", Value::Null)[0]["source_ids"], json!([]));
}

#[test]
fn notes_may_be_left_out() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    assert_eq!(ok(&mut s.api, "case.add", json!({ "name": "Rural" }))["notes"], "");
}

#[test]
fn case_commands_report_plain_problems() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let id = ok(&mut s.api, "case.add", json!({ "name": "Urban Residents" }))["id"].as_i64().unwrap();
    assert_eq!(err(&mut s.api, "case.add", json!({ "name": "urban residents" })).code, "invalid");
    assert_eq!(err(&mut s.api, "case.add", json!({ "name": "  " })).code, "invalid");
    assert_eq!(err(&mut s.api, "case.add", json!({})).code, "bad_args");
    assert_eq!(err(&mut s.api, "case.assign", json!({ "case_id": 99, "source_id": s.source })).code, "not_found");
    assert_eq!(err(&mut s.api, "case.assign", json!({ "case_id": id, "source_id": 99 })).code, "not_found");
    assert_eq!(err(&mut s.api, "case.assign", json!({ "case_id": id })).code, "bad_args");
    assert_eq!(err(&mut s.api, "case.unassign", json!({ "case_id": 99, "source_id": s.source })).code, "not_found");
    assert_eq!(err(&mut s.api, "case.unassign", json!({ "source_id": s.source })).code, "bad_args");
}

#[test]
fn case_changes_can_be_undone_through_the_api() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let id = ok(&mut s.api, "case.add", json!({ "name": "Urban Residents" }))["id"].as_i64().unwrap();
    ok(&mut s.api, "case.assign", json!({ "case_id": id, "source_id": s.source }));
    assert_eq!(ok(&mut s.api, "undo", Value::Null)["what"], "Took the source out of the case");
    assert_eq!(ok(&mut s.api, "case.list", Value::Null)[0]["source_ids"], json!([]));
    assert_eq!(ok(&mut s.api, "undo", Value::Null)["what"], "Removed the case you added");
    assert_eq!(ok(&mut s.api, "case.list", Value::Null), json!([]));
}

// ---- the history of a coding --------------------------------------------------------------------

#[test]
fn coding_history_lists_the_events_oldest_first() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "coding.rationale", json!({ "id": s.coding, "rationale": "explicit feeling" }));
    ok(&mut s.api, "coding.rating", json!({ "id": s.coding, "rating": 4 }));
    let other = ok(&mut s.api, "code.add", json!({ "name": "Belonging", "rated": true }))["id"].as_i64().unwrap();
    ok(&mut s.api, "coding.change_code", json!({ "id": s.coding, "code_id": other }));

    let history = ok(&mut s.api, "coding.history", json!({ "id": s.coding }));
    let actions: Vec<&str> = history.as_array().unwrap().iter().map(|h| h["action"].as_str().unwrap()).collect();
    assert_eq!(actions, ["coding.add", "coding.rationale", "coding.rating", "coding.change_code"]);

    for entry in history.as_array().unwrap() {
        let mut keys: Vec<&str> = entry.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["action", "actor", "after", "before", "ts"]);
        assert_eq!(entry["actor"], "Ada Lovelace");
        assert!(entry["ts"].as_str().unwrap().ends_with('Z'));
    }
    assert_eq!(history[0]["before"], Value::Null);
    assert_eq!(history[0]["after"]["code_id"], s.code);
    assert_eq!(history[2]["before"], json!({ "rating": null }));
    assert_eq!(history[2]["after"], json!({ "rating": 4 }));
    assert_eq!(history[3]["before"], json!({ "code_id": s.code }));
    assert_eq!(history[3]["after"], json!({ "code_id": other }));
}

#[test]
fn coding_history_of_something_that_is_not_there_is_empty_and_needs_an_id() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    assert_eq!(ok(&mut s.api, "coding.history", json!({ "id": 4242 })), json!([]));
    assert_eq!(err(&mut s.api, "coding.history", json!({})).code, "bad_args");
}

#[test]
fn removed_and_new_codings_do_not_share_a_history() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "coding.remove", json!({ "id": s.coding }));
    let again = ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 2, "end": 14, "code_id": s.code }))["id"].as_i64().unwrap();
    assert_eq!(again, s.coding, "the number is reused");
    let history = ok(&mut s.api, "coding.history", json!({ "id": again }));
    assert_eq!(history.as_array().unwrap().len(), 1);
}

// ---- newer files and older files ------------------------------------------------------------------

#[test]
fn a_newer_project_refuses_ratings_and_case_changes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    {
        let mut s = study(&dir);
        ok(&mut s.api, "case.add", json!({ "name": "Urban Residents" }));
        ok(&mut s.api, "project.close", Value::Null);
    }
    rusqlite::Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();
    let mut a = api();
    assert_eq!(ok(&mut a, "project.open", json!({ "path": path }))["read_only"], true);
    assert_eq!(err(&mut a, "coding.rating", json!({ "id": 1, "rating": 3 })).code, "read_only");
    assert_eq!(err(&mut a, "case.add", json!({ "name": "Other" })).code, "read_only");
    assert_eq!(err(&mut a, "case.assign", json!({ "case_id": 1, "source_id": 1 })).code, "read_only");
    assert_eq!(err(&mut a, "case.unassign", json!({ "case_id": 1, "source_id": 1 })).code, "read_only");
    assert_eq!(ok(&mut a, "case.list", Value::Null).as_array().unwrap().len(), 1, "reading still works");
    assert_eq!(ok(&mut a, "coding.history", json!({ "id": 1 })).as_array().unwrap().len(), 1, "so does the history");
}

#[test]
fn project_info_reports_the_new_format_version() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    assert_eq!(ok(&mut s.api, "project.info", Value::Null)["schema_version"], prismqda_engine::schema::SCHEMA_VERSION);
}
