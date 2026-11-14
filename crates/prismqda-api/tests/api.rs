//! The command layer, driven the way an interface drives it: JSON in, JSON out.

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

fn new_project(dir: &TempDir) -> Api {
    let mut a = api();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Study" }));
    a
}

fn import_text(a: &mut Api, dir: &TempDir, text: &str, name: &str) -> i64 {
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    let out = ok(a, "import.run", json!({ "path": path }));
    out["outcome"]["source_ids"][0].as_i64().unwrap()
}

#[test]
fn nothing_works_until_a_project_is_open() {
    let mut a = api();
    for cmd in ["source.list", "code.list", "coding.list", "events.list", "project.info", "memo.list"] {
        let e = err(&mut a, cmd, json!({ "source_id": 1 }));
        assert_eq!(e.code, "no_project", "{cmd}");
    }
    assert_eq!(ok(&mut a, "ping", Value::Null)["name"], "PrismQDA");
    assert_eq!(err(&mut a, "nonsense.command", json!({})).code, "unknown_command");
}

#[test]
fn creating_a_project_makes_the_person_a_coder_and_reports_their_id() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let info = ok(&mut a, "project.info", Value::Null);
    assert_eq!(info["name"], "Study");
    assert_eq!(info["read_only"], false);
    let me = info["me"].as_i64().unwrap();
    let coders = ok(&mut a, "coder.list", Value::Null);
    assert_eq!(coders.as_array().unwrap().len(), 1);
    assert_eq!((coders[0]["id"].as_i64().unwrap(), coders[0]["name"].as_str().unwrap()), (me, "Ada Lovelace"));
    // Asking again does not make a second coder.
    ok(&mut a, "project.info", Value::Null);
    assert_eq!(ok(&mut a, "coder.list", Value::Null).as_array().unwrap().len(), 1);
}

#[test]
fn a_project_can_be_closed_and_opened_again() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    import_text(&mut a, &dir, "One. Two.", "a.txt");
    ok(&mut a, "project.close", Value::Null);
    assert_eq!(err(&mut a, "source.list", Value::Null).code, "no_project");
    let info = ok(&mut a, "project.open", json!({ "path": dir.path().join("p.pqda") }));
    assert_eq!(info["sources"], 1);
    assert_eq!(err(&mut a, "project.open", json!({ "path": dir.path().join("missing.pqda") })).code, "io");
    assert_eq!(err(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Again" })).code, "already_exists");
}

#[test]
fn importing_shows_a_preview_then_stores_and_refuses_duplicates() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "It rained. We stayed in.").unwrap();

    let look = ok(&mut a, "import.inspect", json!({ "path": path }));
    assert_eq!(look["kind"], "batch");
    assert_eq!(look["summary"]["sources"][0]["units"], 2);
    assert_eq!(look["duplicate_of"], Value::Null);
    assert_eq!(ok(&mut a, "source.list", Value::Null).as_array().unwrap().len(), 0, "a preview changes nothing");

    ok(&mut a, "import.run", json!({ "path": path, "folder": "Week 1" }));
    let again = err(&mut a, "import.run", json!({ "path": path }));
    assert_eq!(again.code, "duplicate");
    assert_eq!(ok(&mut a, "import.inspect", json!({ "path": path }))["duplicate_of"], 1);
    ok(&mut a, "import.run", json!({ "path": path, "force": true }));

    let sources = ok(&mut a, "source.list", Value::Null);
    assert_eq!(sources.as_array().unwrap().len(), 2);
    assert_eq!((sources[0]["folder"].as_str().unwrap(), sources[0]["units"].as_i64().unwrap()), ("", 2));
    let units = ok(&mut a, "unit.list", json!({ "source_id": 1 }));
    assert_eq!(units.as_array().unwrap().len(), 2);
    let detail = ok(&mut a, "source.get", json!({ "id": 1 }));
    assert_eq!(detail["text"], "It rained. We stayed in.");
}

#[test]
fn a_spreadsheet_gets_a_guessed_mapping_and_can_be_imported_with_it() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let path = dir.path().join("survey.csv");
    std::fs::write(&path, "pid,group,answer\nP1,a,I felt welcome right away and stayed.\nP2,b,The cost was the hard part for me.\nP3,a,\n").unwrap();

    let look = ok(&mut a, "import.inspect", json!({ "path": path }));
    assert_eq!(look["kind"], "table");
    assert_eq!(look["table"]["row_count"], 3);
    assert_eq!(look["suggested_mapping"]["id_column"], "pid");
    assert_eq!(look["suggested_mapping"]["text_columns"], json!(["answer"]));
    assert_eq!(look["summary"], Value::Null, "no summary until a mapping is chosen");

    let mapping = look["suggested_mapping"].clone();
    let with = ok(&mut a, "import.inspect", json!({ "path": path, "mapping": mapping }));
    assert_eq!(with["summary"]["sources"][0]["units"], 2);
    assert_eq!(with["summary"]["cases"], 3);

    let out = ok(&mut a, "import.run", json!({ "path": path, "mapping": mapping }));
    assert_eq!(out["outcome"]["case_ids"].as_array().unwrap().len(), 3);
    assert_eq!(ok(&mut a, "case.list", Value::Null).as_array().unwrap().len(), 3);
}

#[test]
fn problem_files_come_back_with_plain_codes() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let bad = dir.path().join("fake.docx");
    std::fs::write(&bad, "this is not a Word file").unwrap();
    assert_eq!(err(&mut a, "import.run", json!({ "path": bad })).code, "bad_file");
    let odd = dir.path().join("thing.xyz");
    std::fs::write(&odd, "x").unwrap();
    assert_eq!(err(&mut a, "import.run", json!({ "path": odd })).code, "unsupported_file");
    assert_eq!(err(&mut a, "import.run", json!({ "path": dir.path().join("nope.txt") })).code, "io");
    assert_eq!(err(&mut a, "import.run", json!({})).code, "bad_args");
}

#[test]
fn coding_by_hand_end_to_end() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let source = import_text(&mut a, &dir, "I felt welcome from the first day.", "int.txt");
    let code = ok(&mut a, "code.add", json!({ "name": "Welcome", "definition": "Feeling accepted" }));
    assert_eq!(code["version"], 1);
    let code_id = code["id"].as_i64().unwrap();

    let made = ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 2, "end": 14, "code_id": code_id }));
    assert_eq!(made["quote"], "felt welcome");
    let list = ok(&mut a, "coding.list", json!({ "source_id": source }));
    assert_eq!(list[0]["code_id"], code_id);
    assert_eq!(list[0]["origin"], "human");

    ok(&mut a, "coding.rationale", json!({ "id": made["id"], "rationale": "explicit feeling" }));
    ok(&mut a, "coding.anchor", json!({ "id": made["id"], "anchor": true }));
    assert_eq!(ok(&mut a, "coding.list", json!({ "source_id": source }))[0]["rationale"], "explicit feeling");

    let bad = err(&mut a, "coding.apply", json!({ "source_id": source, "start": 2, "end": 999, "code_id": code_id }));
    assert_eq!(bad.code, "invalid");
    assert!(bad.message.contains("does not fit"), "{}", bad.message);
    assert_eq!(err(&mut a, "coding.apply", json!({ "source_id": source, "start": 2, "end": 5, "code_id": 77 })).code, "not_found");

    ok(&mut a, "coding.remove", json!({ "id": made["id"] }));
    assert_eq!(ok(&mut a, "coding.list", json!({ "source_id": source })).as_array().unwrap().len(), 0);
    let events = ok(&mut a, "events.list", json!({ "limit": 50 }));
    let actions: Vec<&str> = events.as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
    assert!(actions.contains(&"coding.add") && actions.contains(&"coding.remove") && actions.contains(&"anchor.add"), "{actions:?}");
    assert!(events.as_array().unwrap().iter().all(|e| e["actor"] == "Ada Lovelace"));
}

#[test]
fn codebook_edits_merge_and_split() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let source = import_text(&mut a, &dir, "abcdefghij klmnopqrst", "t.txt");
    let id = |v: &Value| v["id"].as_i64().unwrap();
    let x = id(&ok(&mut a, "code.add", json!({ "name": "X" })));
    let y = id(&ok(&mut a, "code.add", json!({ "name": "Y" })));
    let c1 = id(&ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 0, "end": 3, "code_id": x })));
    let c2 = id(&ok(&mut a, "coding.apply", json!({ "source_id": source, "start": 3, "end": 6, "code_id": x })));

    let renamed = ok(&mut a, "code.update", json!({ "id": x, "name": "Access", "definition": "Getting in" }));
    assert_eq!((renamed["name"].as_str().unwrap(), renamed["version"].as_i64().unwrap()), ("Access", 2));
    assert_eq!(err(&mut a, "code.update", json!({ "id": y, "name": "access" })).code, "invalid");

    let split = ok(&mut a, "code.split", json!({ "from": x, "name": "Access: physical", "coding_ids": [c2] }));
    assert_eq!(split["usage"], 1);
    let merged = ok(&mut a, "code.merge", json!({ "from": id(&split), "into": y }));
    assert_eq!(merged["codings_moved"], 1);
    let codes = ok(&mut a, "code.list", Value::Null);
    let by = |name: &str| codes.as_array().unwrap().iter().find(|c| c["name"] == name).unwrap().clone();
    assert_eq!(by("Access: physical")["status"], "merged");
    assert_eq!(by("Y")["usage"], 1);
    assert_eq!(by("Access")["usage"], 1);
    ok(&mut a, "code.move", json!({ "id": y, "parent_id": x }));
    assert_eq!(by_id(&ok(&mut a, "code.list", Value::Null), y)["parent_id"], x);
    assert_eq!(err(&mut a, "code.move", json!({ "id": x, "parent_id": y })).code, "invalid", "no loops");
    ok(&mut a, "code.retire", json!({ "id": y }));
    assert_eq!(err(&mut a, "coding.apply", json!({ "source_id": source, "start": 7, "end": 9, "code_id": y })).code, "invalid");
    let _ = c1;
    assert_eq!(ok(&mut a, "codebook.snapshot", json!({ "note": "after pilot" }))["version"], 1);
    assert_eq!(ok(&mut a, "codebook.versions", Value::Null)[0]["note"], "after pilot");
}

fn by_id(list: &Value, id: i64) -> Value {
    list.as_array().unwrap().iter().find(|c| c["id"] == id).unwrap().clone()
}

#[test]
fn memos_through_the_api() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let source = import_text(&mut a, &dir, "Some words here.", "m.txt");
    let memo = ok(&mut a, "memo.add", json!({ "kind": "analytic", "scope": "source", "target_id": source, "title": "First impressions", "body": "Warm tone." }));
    let id = memo["id"].as_i64().unwrap();
    assert_eq!(ok(&mut a, "memo.list", json!({ "scope": "source", "target_id": source }))[0]["title"], "First impressions");
    ok(&mut a, "memo.update", json!({ "id": id, "body": "Warm tone, hesitant." }));
    assert_eq!(ok(&mut a, "memo.list", json!({}))[0]["body"], "Warm tone, hesitant.");
    assert_eq!(err(&mut a, "memo.add", json!({ "kind": "analytic", "scope": "source", "title": "no target" })).code, "invalid");
    assert_eq!(err(&mut a, "memo.add", json!({ "kind": "gossip", "scope": "source" })).code, "bad_args");
    ok(&mut a, "memo.delete", json!({ "id": id }));
    assert_eq!(ok(&mut a, "memo.list", json!({})).as_array().unwrap().len(), 0);
}

#[test]
fn an_open_project_made_by_a_newer_version_is_read_only_through_the_api() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    {
        let mut a = new_project(&dir);
        ok(&mut a, "project.close", Value::Null);
    }
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
    }
    let mut a = api();
    let info = ok(&mut a, "project.open", json!({ "path": path }));
    assert_eq!(info["read_only"], true);
    assert!(info["notices"][0].as_str().unwrap().contains("read-only"));
    assert_eq!(info["me"], Value::Null);
    assert_eq!(err(&mut a, "code.add", json!({ "name": "X" })).code, "read_only");
    assert!(ok(&mut a, "code.list", Value::Null).as_array().unwrap().is_empty());
}

#[test]
fn the_number_of_backups_kept_is_a_setting_for_the_open_project_and_every_one_after() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    assert_eq!(err(&mut a, "backup.keep", json!({ "keep": 0 })).code, "invalid", "at least one");
    assert_eq!(err(&mut a, "backup.keep", json!({ "keep": 51 })).code, "invalid", "at most fifty");
    assert_eq!(ok(&mut a, "backup.keep", json!({ "keep": 2 }))["keep"], 2);
    for _ in 0..4 {
        ok(&mut a, "project.backup", json!({}));
    }
    let folder = dir.path().join("p.backups");
    let count = |folder: &std::path::Path| std::fs::read_dir(folder).unwrap().count();
    assert_eq!(count(&folder), 2, "the open project keeps the two newest");

    // A project opened afterwards keeps the same number.
    ok(&mut a, "project.close", json!({}));
    ok(&mut a, "project.open", json!({ "path": dir.path().join("p.pqda"), "backup": false }));
    ok(&mut a, "project.backup", json!({}));
    ok(&mut a, "project.backup", json!({}));
    assert_eq!(count(&folder), 2, "still two after reopening");

    // Asking for more keeps more from then on.
    ok(&mut a, "backup.keep", json!({ "keep": 5 }));
    for _ in 0..3 {
        ok(&mut a, "project.backup", json!({}));
    }
    assert_eq!(count(&folder), 5);
}

#[test]
fn a_frozen_codebook_version_can_be_read_back_to_compare_with_the_codebook_today() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let cost = ok(&mut a, "code.add", json!({ "name": "Cost", "definition": "Fares and fees." }))["id"].as_i64().unwrap();
    assert_eq!(err(&mut a, "codebook.version", json!({ "version": 1 })).code, "not_found", "nothing is frozen yet");
    assert_eq!(ok(&mut a, "codebook.snapshot", json!({ "note": "before the pilot" }))["version"], 1);
    ok(&mut a, "code.update", json!({ "id": cost, "definition": "What the trip costs the rider." }));
    ok(&mut a, "code.add", json!({ "name": "Safety" }));
    let v = ok(&mut a, "codebook.version", json!({ "version": 1 }));
    assert_eq!(v["note"], "before the pilot");
    assert_eq!(v["codes"].as_array().unwrap().len(), 1, "one code was in the codebook then");
    assert_eq!(v["codes"][0]["name"], "Cost");
    assert_eq!(v["codes"][0]["id"], cost);
    assert_eq!(v["codes"][0]["definition"], "Fares and fees.", "the definition in effect then, not today's");
    assert!(v["created_at"].as_str().unwrap().ends_with('Z'));
}

#[test]
fn the_import_preview_carries_the_first_words_of_each_source() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir);
    let long = format!("Opening words of the file. {}", "More text. ".repeat(100));
    let path = dir.path().join("preview.txt");
    std::fs::write(&path, &long).unwrap();
    let out = ok(&mut a, "import.inspect", json!({ "path": path }));
    let preview = out["summary"]["sources"][0]["preview"].as_str().unwrap();
    assert!(preview.starts_with("Opening words of the file."));
    assert_eq!(preview.chars().count(), 400, "a few hundred characters, not the whole file");
}
