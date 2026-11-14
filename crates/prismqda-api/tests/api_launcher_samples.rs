//! The launcher's commands, driven the way the interface drives them: `file.stat` for recent files, and
//! `sample.list` and `sample.create` for the bundled sample projects.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

/// The sample as shipped, read here independently of the code that builds it.
const URBAN: &str = include_str!("../samples/urban-mobility.json");
const INT_001: &str = "INT-001 \u{2013} Alex";

fn api() -> Api {
    Api::new("Ada Lovelace").without_backup_on_open()
}

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn sample() -> Value {
    serde_json::from_str(URBAN).unwrap()
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    names.sort();
    names
}

fn make(api: &mut Api, path: &Path) -> Value {
    ok(api, "sample.create", json!({ "id": "urban-mobility", "path": path }))
}

/// Every code in the sample's tree, in file order, with the name of its parent.
fn sample_codes() -> Vec<(Value, Option<String>)> {
    fn walk(codes: &Value, parent: Option<&str>, out: &mut Vec<(Value, Option<String>)>) {
        for code in codes.as_array().into_iter().flatten() {
            out.push((code.clone(), parent.map(str::to_string)));
            walk(&code["children"], code["name"].as_str(), out);
        }
    }
    let mut out = Vec::new();
    walk(&sample()["codes"], None, &mut out);
    out
}

fn text_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_else(|| panic!("{key} in {v}"))
}

// ---- file.stat ----------------------------------------------------------------------------------------

#[test]
fn stat_reports_a_file_and_touches_nothing_beside_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("recent.pqda");
    fs::write(&path, b"this is not a project").unwrap();
    let file = File::options().write(true).open(&path).unwrap();
    file.set_modified(UNIX_EPOCH + Duration::new(1_700_000_000, 123_000_000)).unwrap();
    drop(file);
    let before = names_in(dir.path());

    let mut a = api(); // no project is open, and none is needed
    let stat = ok(&mut a, "file.stat", json!({ "path": path }));
    assert_eq!(stat, json!({ "exists": true, "is_file": true, "name": "recent.pqda", "modified": "2023-11-14T22:13:20.123Z", "size": 21, "encrypted": false }));

    // It was never opened as a project: it did not fail for being something else, and nothing appeared beside it.
    assert_eq!(names_in(dir.path()), before);
    assert_eq!(fs::read(&path).unwrap(), b"this is not a project");
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), UNIX_EPOCH + Duration::new(1_700_000_000, 123_000_000));
}

#[test]
fn stat_of_a_real_project_does_not_open_back_up_or_change_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("study.pqda");
    let mut maker = api();
    ok(&mut maker, "project.create", json!({ "path": path, "name": "Study" }));
    ok(&mut maker, "project.close", Value::Null);
    let (bytes, listing, modified) = (fs::read(&path).unwrap(), names_in(dir.path()), fs::metadata(&path).unwrap().modified().unwrap());

    // A normal session that makes backups on open, so a stray open would leave a `.backups` folder.
    let mut a = Api::new("Ada Lovelace");
    let stat = ok(&mut a, "file.stat", json!({ "path": path }));
    assert_eq!(stat["exists"], true);
    assert_eq!(stat["is_file"], true);
    assert_eq!(stat["name"], "study.pqda");
    assert_eq!(stat["size"], bytes.len());
    assert!(text_of(&stat, "modified").ends_with('Z') && stat["modified"].as_str().unwrap().len() == 24, "{stat}");

    assert_eq!(names_in(dir.path()), listing, "no backup folder, no -wal or -shm");
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert_eq!(err(&mut a, "project.info", Value::Null).code, "no_project", "stat did not open it");
}

#[test]
fn stat_of_a_missing_path_is_an_answer_not_an_error() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    let gone = ok(&mut a, "file.stat", json!({ "path": dir.path().join("gone.pqda") }));
    assert_eq!(gone, json!({ "exists": false, "is_file": false, "name": "gone.pqda", "modified": null, "size": 0, "encrypted": false }));
    let deeper = ok(&mut a, "file.stat", json!({ "path": dir.path().join("no").join("such").join("folder.pqda") }));
    assert_eq!((deeper["exists"].clone(), deeper["modified"].clone()), (json!(false), Value::Null));
}

#[test]
fn stat_of_a_folder_says_it_is_not_a_file() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("Projects")).unwrap();
    let mut a = api();
    let stat = ok(&mut a, "file.stat", json!({ "path": dir.path().join("Projects") }));
    assert_eq!((stat["exists"].clone(), stat["is_file"].clone(), stat["name"].clone(), stat["size"].clone()), (json!(true), json!(false), json!("Projects"), json!(0)));
    assert!(stat["modified"].is_string(), "{stat}");
}

#[test]
fn stat_needs_a_path() {
    let mut a = api();
    assert_eq!(err(&mut a, "file.stat", json!({})).code, "bad_args");
    assert_eq!(err(&mut a, "file.stat", json!({ "path": "" })).code, "invalid");
    assert_eq!(err(&mut a, "file.stat", json!({ "path": "   " })).code, "invalid");
}

#[test]
fn stat_leaves_the_open_project_alone() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Open one" }));
    let other = dir.path().join("other.pqda");
    fs::write(&other, b"x").unwrap();
    ok(&mut a, "file.stat", json!({ "path": other }));
    assert_eq!(ok(&mut a, "project.info", Value::Null)["name"], "Open one");
}

// ---- sample.list --------------------------------------------------------------------------------------

#[test]
fn the_list_describes_each_bundled_sample_and_needs_no_project() {
    let mut a = api();
    let list = ok(&mut a, "sample.list", Value::Null);
    let list = list.as_array().unwrap();
    assert!(!list.is_empty());
    let urban = list.iter().find(|s| s["id"] == "urban-mobility").expect("the urban mobility sample is listed");

    let keys: BTreeSet<&str> = urban.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, BTreeSet::from(["codes", "codings", "description", "documents", "id", "name"]));
    assert_eq!(urban["name"], "Urban Mobility Study");
    assert!(urban["description"].as_str().unwrap().len() > 40);

    // The counts are the file's own, counted here from the JSON.
    let json = sample();
    assert_eq!(urban["documents"], json["documents"].as_array().unwrap().len());
    assert_eq!(urban["codes"], sample_codes().len());
    assert_eq!(urban["codings"], json["codings"].as_array().unwrap().len());
    assert_eq!((urban["documents"].clone(), urban["codes"].clone(), urban["codings"].clone()), (json!(7), json!(15), json!(10)));

    // Every id appears once.
    let ids: BTreeSet<&str> = list.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), list.len());
}

// ---- sample.create ------------------------------------------------------------------------------------

#[test]
fn a_copy_has_the_counts_the_sample_file_has() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Urban Mobility Study.pqda");
    let mut a = api();
    let info = make(&mut a, &path);

    assert_eq!(info["name"], "Urban Mobility Study");
    assert_eq!(info["path"], path.to_str().unwrap());
    assert_eq!(info["sample_id"], "urban-mobility");
    assert_eq!(info["schema_version"], prismqda_engine::schema::SCHEMA_VERSION);
    assert_eq!(info["read_only"], false);
    assert_eq!(info["coding_mode"], "free_span");
    assert_eq!((info["sources"].clone(), info["codes"].clone(), info["codings"].clone(), info["coders"].clone()), (json!(7), json!(15), json!(10), json!(1)));
    assert!(info["units"].as_i64().unwrap() >= 7);
    assert!(info["me"].is_i64());

    // The project that is now open is the copy.
    assert_eq!(ok(&mut a, "project.info", Value::Null), info);

    // The same numbers from the lists, not just the counters.
    assert_eq!(ok(&mut a, "source.list", Value::Null).as_array().unwrap().len(), 7);
    assert_eq!(ok(&mut a, "code.list", Value::Null).as_array().unwrap().len(), 15);
    assert_eq!(ok(&mut a, "case.list", Value::Null).as_array().unwrap().len(), 2);
    let memos = ok(&mut a, "memo.list", json!({}));
    assert_eq!(memos.as_array().unwrap().len(), 1);
    assert_eq!(ok(&mut a, "segments.list", json!({}))["total"], 10);
}

#[test]
fn documents_arrive_with_their_folders_text_and_the_units_a_txt_import_gives() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    make(&mut a, &dir.path().join("copy.pqda"));
    let sources = ok(&mut a, "source.list", Value::Null);
    let json = sample();

    for doc in json["documents"].as_array().unwrap() {
        let row = sources.as_array().unwrap().iter().find(|s| s["name"] == doc["name"]).unwrap_or_else(|| panic!("no source named {}", doc["name"]));
        assert_eq!(row["folder"], doc["folder"]);
        assert_eq!(row["source_type"], "document");
        assert_eq!(row["default_unit_kind"], "sentence");
        assert!(row["units"].as_i64().unwrap() > 0, "{row}");
        let text = ok(&mut a, "source.get", json!({ "id": row["id"] }))["text"].clone();
        assert_eq!(text, doc["text"], "{}", doc["name"]);
    }

    // The same text imported as a .txt file into an ordinary project gives the same source and the same units.
    let mut plain = api();
    ok(&mut plain, "project.create", json!({ "path": dir.path().join("plain.pqda"), "name": "Plain" }));
    for doc in json["documents"].as_array().unwrap() {
        let file = dir.path().join("doc.txt");
        fs::write(&file, text_of(doc, "text")).unwrap();
        let imported = ok(&mut plain, "import.run", json!({ "path": file, "name": doc["name"], "folder": doc["folder"], "force": true }));
        let id = imported["outcome"]["source_ids"][0].clone();
        let theirs = ok(&mut plain, "unit.list", json!({ "source_id": id }));
        let mine_row = sources.as_array().unwrap().iter().find(|s| s["name"] == doc["name"]).unwrap();
        let mine = ok(&mut a, "unit.list", json!({ "source_id": mine_row["id"] }));
        let strip = |units: &Value| -> Vec<Value> { units.as_array().unwrap().iter().map(|u| json!([u["kind"], u["start"], u["end"], u["speaker"], u["excluded"]])).collect() };
        assert_eq!(strip(&mine), strip(&theirs), "{}", doc["name"]);
    }
}

#[test]
fn the_cases_hold_the_documents_the_sample_names() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    make(&mut a, &dir.path().join("copy.pqda"));
    let sources = ok(&mut a, "source.list", Value::Null);
    let name_of: BTreeMap<i64, String> = sources.as_array().unwrap().iter().map(|s| (s["id"].as_i64().unwrap(), text_of(s, "name").to_string())).collect();
    let cases = ok(&mut a, "case.list", Value::Null);

    for expected in sample()["cases"].as_array().unwrap() {
        let case = cases.as_array().unwrap().iter().find(|c| c["name"] == expected["name"]).unwrap_or_else(|| panic!("no case {}", expected["name"]));
        let held: BTreeSet<String> = case["source_ids"].as_array().unwrap().iter().map(|id| name_of[&id.as_i64().unwrap()].clone()).collect();
        let wanted: BTreeSet<String> = expected["documents"].as_array().unwrap().iter().map(|d| d.as_str().unwrap().to_string()).collect();
        assert_eq!(held, wanted, "{}", expected["name"]);
        assert_eq!(case["sources"], wanted.len());
    }
}

#[test]
fn the_codebook_has_the_sample_tree_colors_definitions_and_order() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    make(&mut a, &dir.path().join("copy.pqda"));
    let rows = ok(&mut a, "code.list", Value::Null);
    let rows = rows.as_array().unwrap();
    let name_of: BTreeMap<i64, &str> = rows.iter().map(|r| (r["id"].as_i64().unwrap(), r["name"].as_str().unwrap())).collect();

    for (code, parent) in sample_codes() {
        let row = rows.iter().find(|r| r["name"] == code["name"]).unwrap_or_else(|| panic!("no code {}", code["name"]));
        assert_eq!(row["color"], code["color"], "{}", code["name"]);
        assert_eq!(row["definition"], code["definition"].as_str().unwrap_or(""), "{}", code["name"]);
        assert_eq!(row["parent_id"].as_i64().map(|p| name_of[&p].to_string()), parent, "{}", code["name"]);
        assert_eq!(row["status"], "active");
    }

    // Siblings come back in the order the sample lists them, not alphabetically.
    let order_under = |parent: Option<&str>| -> Vec<&str> {
        rows.iter().filter(|r| r["parent_id"].as_i64().map(|p| name_of[&p]) == parent).map(|r| r["name"].as_str().unwrap()).collect()
    };
    assert_eq!(order_under(None), ["Mobility Experience", "Barriers & Challenges", "Perceptions & Attitudes", "Change & Future"]);
    assert_eq!(order_under(Some("Mobility Experience")), ["Daily Travel", "Mode Choice", "Convenience"]);
    assert_eq!(order_under(Some("Barriers & Challenges")), ["Cost", "Reliability", "Accessibility", "Safety"]);
}

#[test]
fn every_coding_covers_exactly_its_phrase_and_keeps_its_note() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    let info = make(&mut a, &dir.path().join("copy.pqda"));
    let segments = ok(&mut a, "segments.list", json!({}));
    let segments = segments["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 10);

    let mut used = BTreeSet::new();
    for coding in sample()["codings"].as_array().unwrap() {
        let found: Vec<&Value> = segments.iter().filter(|s| s["source_name"] == coding["document"] && s["code_name"] == coding["code"] && s["text"] == coding["phrase"]).collect();
        assert_eq!(found.len(), 1, "{}", coding["phrase"]);
        let seg = found[0];
        assert_eq!(seg["text"], coding["phrase"]);
        assert_eq!(seg["rationale"], coding["note"].as_str().unwrap_or(""));
        assert_eq!(seg["rating"], coding.get("rating").cloned().unwrap_or(Value::Null));
        assert_eq!(seg["coder_id"], info["me"], "the codings belong to the person using the project");
        assert_eq!(seg["origin"], "human");
        assert!(used.insert(seg["coding_id"].as_i64().unwrap()), "each coding is a different row");
    }
}

#[test]
fn the_reliability_coding_has_its_rating_and_its_memo() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    make(&mut a, &dir.path().join("copy.pqda"));
    let segments = ok(&mut a, "segments.list", json!({}));
    let segments = segments["segments"].as_array().unwrap();

    let reliability = segments.iter().find(|s| s["source_name"] == INT_001 && s["code_name"] == "Reliability").expect("the Reliability coding in INT-001");
    assert_eq!(reliability["rating"], 4);
    let rated: Vec<&Value> = segments.iter().filter(|s| !s["rating"].is_null()).collect();
    assert_eq!(rated.len(), 1, "only that one is rated");

    let memos = ok(&mut a, "memo.list", json!({ "scope": "coding" }));
    assert_eq!(memos.as_array().unwrap().len(), 1);
    let memo = &memos[0];
    assert_eq!(memo["title"], "Service reliability and daily stress");
    assert_eq!(memo["target_id"], reliability["coding_id"]);
    assert_eq!(memo["scope"], "coding");
    let wanted = sample()["codings"].as_array().unwrap().iter().find_map(|c| c.get("memo").cloned()).unwrap();
    assert_eq!(memo["body"], wanted["body"]);

    // How the rating got there is on the record.
    let history = ok(&mut a, "coding.history", json!({ "id": reliability["coding_id"] }));
    let actions: Vec<&str> = history.as_array().unwrap().iter().map(|h| h["action"].as_str().unwrap()).collect();
    assert_eq!(actions, ["coding.add", "coding.rating"]);
}

#[test]
fn a_normal_project_has_no_sample_id_and_a_copy_keeps_its_own() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    let normal = ok(&mut a, "project.create", json!({ "path": dir.path().join("normal.pqda"), "name": "Mine" }));
    assert!(normal.as_object().unwrap().contains_key("sample_id"), "the key is always there");
    assert_eq!(normal["sample_id"], Value::Null);

    let copy_path = dir.path().join("copy.pqda");
    make(&mut a, &copy_path);
    assert_eq!(ok(&mut a, "project.info", Value::Null)["sample_id"], "urban-mobility");

    // It is stored in the file: closing, opening it again, and a different session all see it.
    ok(&mut a, "project.close", Value::Null);
    assert_eq!(ok(&mut a, "project.open", json!({ "path": copy_path }))["sample_id"], "urban-mobility");
    assert_eq!(ok(&mut api(), "project.open", json!({ "path": copy_path }))["sample_id"], "urban-mobility");
    assert_eq!(ok(&mut a, "project.open", json!({ "path": dir.path().join("normal.pqda") }))["sample_id"], Value::Null);
}

#[test]
fn remembering_the_sample_adds_no_table_and_no_format_version() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("normal.pqda"), "name": "Mine" }));
    make(&mut a, &dir.path().join("copy.pqda"));
    ok(&mut a, "project.close", Value::Null);

    let shape = |file: &str| -> (BTreeSet<String>, i64) {
        let conn = rusqlite::Connection::open_with_flags(dir.path().join(file), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'index', 'trigger')").unwrap();
        let names = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect();
        (names, conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap())
    };
    let (normal, copy) = (shape("normal.pqda"), shape("copy.pqda"));
    assert_eq!(copy, normal);
    assert_eq!(copy.1, i64::from(prismqda_engine::schema::SCHEMA_VERSION));
}

#[test]
fn an_existing_path_is_refused_and_left_alone_and_the_open_project_stays_open() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("open.pqda"), "name": "Open one" }));

    let taken = dir.path().join("taken.pqda");
    fs::write(&taken, b"somebody's work").unwrap();
    let before = names_in(dir.path());
    let e = err(&mut a, "sample.create", json!({ "id": "urban-mobility", "path": taken }));
    assert_eq!(e.code, "already_exists");
    assert_eq!(fs::read(&taken).unwrap(), b"somebody's work");
    assert_eq!(names_in(dir.path()), before);
    assert_eq!(ok(&mut a, "project.info", Value::Null)["name"], "Open one");

    // Also when what is there is a real project of someone's, even one made from this very sample.
    let mut other = api();
    let theirs = dir.path().join("theirs.pqda");
    make(&mut other, &theirs);
    ok(&mut other, "project.close", Value::Null);
    let bytes = fs::read(&theirs).unwrap();
    assert_eq!(err(&mut a, "sample.create", json!({ "id": "urban-mobility", "path": theirs })).code, "already_exists");
    assert_eq!(fs::read(&theirs).unwrap(), bytes);
    assert_eq!(ok(&mut a, "project.info", Value::Null)["name"], "Open one", "the project that was open is still open");
}

#[test]
fn an_unknown_id_is_not_found_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("open.pqda"), "name": "Open one" }));
    let before = names_in(dir.path());

    let e = err(&mut a, "sample.create", json!({ "id": "no-such-sample", "path": dir.path().join("x.pqda") }));
    assert_eq!(e.code, "not_found");
    assert!(e.message.contains("no-such-sample"), "{}", e.message);
    assert_eq!(names_in(dir.path()), before, "no file was made");
    assert_eq!(ok(&mut a, "project.info", Value::Null)["name"], "Open one");

    assert_eq!(err(&mut a, "sample.create", json!({ "path": dir.path().join("x.pqda") })).code, "bad_args");
    assert_eq!(err(&mut a, "sample.create", json!({ "id": "urban-mobility" })).code, "bad_args");
    assert_eq!(err(&mut a, "sample.create", json!({ "id": "urban-mobility", "path": "  " })).code, "invalid");
}

#[test]
fn a_copy_that_cannot_be_saved_leaves_no_file_and_no_open_project_is_lost() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("open.pqda"), "name": "Open one" }));
    let missing_folder = dir.path().join("not").join("there").join("copy.pqda");
    let e = err(&mut a, "sample.create", json!({ "id": "urban-mobility", "path": missing_folder }));
    assert_eq!(e.code, "database", "{e:?}");
    assert!(!dir.path().join("not").exists());
    assert_eq!(ok(&mut a, "project.info", Value::Null)["name"], "Open one");
}

#[test]
fn two_copies_are_independent_and_the_original_cannot_be_changed() {
    let dir = TempDir::new().unwrap();
    let (first, second, third) = (dir.path().join("one.pqda"), dir.path().join("two.pqda"), dir.path().join("three.pqda"));
    let mut a = api();
    let one = make(&mut a, &first);
    let two = make(&mut a, &second);
    assert_ne!(one["project_uid"], two["project_uid"], "two projects, not two names for one");
    assert_eq!((one["codings"].clone(), two["codings"].clone()), (json!(10), json!(10)));

    // Pull the second one apart: a new code, a coding gone, the memo gone, the rating cleared, a source edited.
    ok(&mut a, "code.add", json!({ "name": "Mine alone" }));
    let segments = ok(&mut a, "segments.list", json!({}));
    let reliability = segments["segments"].as_array().unwrap().iter().find(|s| s["code_name"] == "Reliability" && s["source_name"] == INT_001).unwrap().clone();
    ok(&mut a, "coding.rating", json!({ "id": reliability["coding_id"], "rating": null }));
    let memo = ok(&mut a, "memo.list", json!({}))[0]["id"].clone();
    ok(&mut a, "memo.delete", json!({ "id": memo }));
    ok(&mut a, "coding.remove", json!({ "id": segments["segments"][0]["coding_id"] }));
    let changed = ok(&mut a, "project.info", Value::Null);
    assert_eq!((changed["codes"].clone(), changed["codings"].clone()), (json!(16), json!(9)));
    ok(&mut a, "project.close", Value::Null);

    // The first is exactly as it was made.
    let reopened = ok(&mut a, "project.open", json!({ "path": first }));
    assert_eq!((reopened["codes"].clone(), reopened["codings"].clone(), reopened["sample_id"].clone()), (json!(15), json!(10), json!("urban-mobility")));
    let kept = ok(&mut a, "segments.list", json!({}));
    let kept = kept["segments"].as_array().unwrap().iter().find(|s| s["code_name"] == "Reliability" && s["source_name"] == INT_001).unwrap().clone();
    assert_eq!(kept["rating"], 4);
    assert_eq!(ok(&mut a, "memo.list", json!({})).as_array().unwrap().len(), 1);

    // And a copy made after all that is the sample as shipped.
    let fresh = make(&mut a, &third);
    assert_eq!((fresh["codes"].clone(), fresh["codings"].clone(), fresh["sources"].clone()), (json!(15), json!(10), json!(7)));
    assert_eq!(ok(&mut a, "memo.list", json!({})).as_array().unwrap().len(), 1);
    assert_eq!(ok(&mut a, "sample.list", Value::Null)[0]["codes"], 15);
}

#[test]
fn a_fresh_copy_has_nothing_to_undo_and_a_later_change_undoes_alone() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    make(&mut a, &dir.path().join("copy.pqda"));
    assert_eq!(ok(&mut a, "undo.next", Value::Null), json!({ "next": null }));
    assert_eq!(ok(&mut a, "undo", Value::Null), json!({ "nothing_to_undo": true }));

    ok(&mut a, "code.add", json!({ "name": "Mine" }));
    assert_eq!(ok(&mut a, "project.info", Value::Null)["codes"], 16);
    ok(&mut a, "undo", Value::Null);
    assert_eq!(ok(&mut a, "project.info", Value::Null)["codes"], 15);
    assert_eq!(ok(&mut a, "undo", Value::Null), json!({ "nothing_to_undo": true }), "nothing earlier to take apart");
}

#[test]
fn a_copy_is_an_ordinary_editable_project() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("copy.pqda");
    let mut a = api();
    let info = make(&mut a, &path);
    let me = info["me"].clone();

    // Code a new passage, write a note, close, and open it again as any project.
    let sources = ok(&mut a, "source.list", Value::Null);
    let source = sources.as_array().unwrap().iter().find(|s| s["name"] == INT_001).unwrap()["id"].clone();
    let code = ok(&mut a, "code.add", json!({ "name": "My own code" }))["id"].clone();
    let coding = ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 0, "end": 11, "code_id": code, "rationale": "mine" }));
    assert_eq!(coding["quote"], "Interviewer");
    ok(&mut a, "events.annotate", json!({ "note": "Started from the sample." }));
    ok(&mut a, "project.close", Value::Null);

    let mut b = Api::new("Ada Lovelace");
    let reopened = ok(&mut b, "project.open", json!({ "path": path }));
    assert_eq!((reopened["codes"].clone(), reopened["codings"].clone(), reopened["me"].clone()), (json!(16), json!(11), me));
    assert_eq!(reopened["read_only"], false);

    // The first event on record says where it came from; nothing else about the file is special.
    let first = &ok(&mut b, "events.list", json!({ "limit": 1000 }))[0];
    assert_eq!(first["action"], "project.create");
    assert_eq!(first["after"]["sample_id"], "urban-mobility");
}

#[test]
fn every_listed_sample_can_be_copied() {
    let dir = TempDir::new().unwrap();
    let mut a = api();
    let list = ok(&mut a, "sample.list", Value::Null);
    for (i, sample) in list.as_array().unwrap().iter().enumerate() {
        let info = ok(&mut a, "sample.create", json!({ "id": sample["id"], "path": dir.path().join(format!("s{i}.pqda")) }));
        assert_eq!(info["sample_id"], sample["id"]);
        assert_eq!(info["name"], sample["name"]);
        assert_eq!((info["sources"].clone(), info["codes"].clone(), info["codings"].clone()), (sample["documents"].clone(), sample["codes"].clone(), sample["codings"].clone()));
    }
}
