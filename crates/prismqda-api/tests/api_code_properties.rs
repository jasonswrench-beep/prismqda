//! Code properties through the commands the interface uses: groups, rated codes, counterexamples.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

struct Study {
    api: Api,
    source: i64,
    positive: i64,
    negative: i64,
}

fn study(dir: &TempDir) -> Study {
    let mut a = Api::new("Ada Lovelace").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Study" }));
    let path = dir.path().join("int.txt");
    std::fs::write(&path, "I felt welcome from the first day, but the fares were a worry.").unwrap();
    let source = ok(&mut a, "import.run", json!({ "path": path }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let positive = ok(&mut a, "code.add", json!({ "name": "Positive", "exclusive_group": "Sentiment" }))["id"].as_i64().unwrap();
    let negative = ok(&mut a, "code.add", json!({ "name": "Negative", "exclusive_group": "sentiment" }))["id"].as_i64().unwrap();
    Study { api: a, source, positive, negative }
}

fn apply(s: &mut Study, start: i64, end: i64, code: i64, replace: bool) -> Result<Value, ApiError> {
    s.api.call("coding.apply", json!({ "source_id": s.source, "start": start, "end": end, "code_id": code, "replace_exclusive": replace }))
}

#[test]
fn a_code_made_with_a_group_and_a_scale_lists_them() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let rated = ok(
        &mut s.api,
        "code.add",
        json!({ "name": "Worry", "rated": true, "rating_label": "Intensity", "rating_low": "A flicker", "rating_high": "Overwhelming" }),
    );
    assert_eq!((rated["rated"].clone(), rated["rating_label"].clone(), rated["rating_high"].clone()), (json!(true), json!("Intensity"), json!("Overwhelming")));
    let listed = ok(&mut s.api, "code.list", Value::Null);
    let negative = listed.as_array().unwrap().iter().find(|c| c["name"] == "Negative").unwrap();
    assert_eq!(negative["exclusive_group"], "Sentiment", "the group keeps the spelling it was first given");
    assert_eq!(negative["rated"], false);

    let edited = ok(&mut s.api, "code.update", json!({ "id": s.negative, "exclusive_group": "", "rated": true }));
    assert_eq!((edited["exclusive_group"].clone(), edited["rated"].clone()), (json!(""), json!(true)));
}

#[test]
fn a_clash_in_a_group_is_its_own_error_and_replacing_resolves_it() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let (positive, negative) = (s.positive, s.negative);
    ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 2, "end": 14, "code_id": positive }));

    let e = apply(&mut s, 5, 20, negative, false).unwrap_err();
    assert_eq!(e.code, "exclusive_group");
    assert_eq!(e.message, "'Negative' is in the group 'Sentiment', which allows one code at a time, and these words already have 'Positive'.");

    let done = apply(&mut s, 5, 20, negative, true).unwrap();
    assert!(done["id"].as_i64().is_some());
    let rows = ok(&mut s.api, "coding.list", json!({ "source_id": s.source }));
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["code_id"], negative);
}

#[test]
fn changing_the_code_of_a_coding_into_a_clash_is_refused_the_same_way() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let (positive, negative) = (s.positive, s.negative);
    // The same code twice on overlapping words is fine; moving one of them to the other code of the group is not.
    ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 2, "end": 14, "code_id": positive }));
    let b = ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 10, "end": 30, "code_id": positive }))["id"].as_i64().unwrap();
    let e = err(&mut s.api, "coding.change_code", json!({ "id": b, "code_id": negative }));
    assert_eq!(e.code, "exclusive_group", "{e:?}");
    assert!(e.message.contains("already have 'Positive'"), "{e:?}");
}

#[test]
fn the_check_counts_passages_that_already_break_a_group() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let (positive, negative) = (s.positive, s.negative);
    ok(&mut s.api, "code.update", json!({ "id": negative, "exclusive_group": "" }));
    ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 2, "end": 14, "code_id": positive }));
    ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 2, "end": 14, "code_id": negative }));
    ok(&mut s.api, "code.update", json!({ "id": negative, "exclusive_group": "Sentiment" }));
    let check = ok(&mut s.api, "code.group_check", json!({ "group": "Sentiment" }));
    assert_eq!(check["violations"], 1);
    assert_eq!(check["first"].as_array().unwrap().len(), 1);
    assert_eq!(ok(&mut s.api, "code.group_check", json!({ "group": "Nothing" }))["violations"], 0);
}

#[test]
fn ratings_need_a_rated_code_and_switching_it_off_says_what_it_would_clear() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let positive = s.positive;
    let id = ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 2, "end": 14, "code_id": positive }))["id"].as_i64().unwrap();
    let e = err(&mut s.api, "coding.rating", json!({ "id": id, "rating": 3 }));
    assert!(e.message.contains("is not a rated code"), "{e:?}");

    ok(&mut s.api, "code.update", json!({ "id": positive, "rated": true }));
    ok(&mut s.api, "coding.rating", json!({ "id": id, "rating": 3 }));
    let e = err(&mut s.api, "code.update", json!({ "id": positive, "rated": false }));
    assert!(e.message.starts_with("1 coding of this code carries a rating."), "{e:?}");
    ok(&mut s.api, "code.update", json!({ "id": positive, "rated": false, "clear_ratings": true }));
    assert!(ok(&mut s.api, "coding.list", json!({ "source_id": s.source }))[0]["rating"].is_null());
}

#[test]
fn counterexamples_are_marked_listed_and_shown_with_their_words() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let (positive, negative) = (s.positive, s.negative);
    let pos = ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 2, "end": 14, "code_id": positive }))["id"].as_i64().unwrap();
    let neg = ok(&mut s.api, "coding.apply", json!({ "source_id": s.source, "start": 39, "end": 62, "code_id": negative }))["id"].as_i64().unwrap();

    let e = err(&mut s.api, "coding.counterexample", json!({ "code_id": positive, "id": pos, "on": true }));
    assert!(e.message.contains("an example of it"), "{e:?}");

    ok(&mut s.api, "coding.counterexample", json!({ "code_id": positive, "id": neg, "on": true }));
    let shown = ok(&mut s.api, "code.counterexamples", json!({ "id": positive }));
    let shown = shown.as_array().unwrap();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0]["code_name"], "Negative");
    assert_eq!(shown[0]["text"], "the fares were a worry.");
    assert_eq!(shown[0]["coding_id"], neg);

    ok(&mut s.api, "coding.counterexample", json!({ "code_id": positive, "id": neg, "on": false }));
    assert!(ok(&mut s.api, "code.counterexamples", json!({ "id": positive })).as_array().unwrap().is_empty());
}

#[test]
fn the_sample_shows_all_three() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada Lovelace").without_backup_on_open();
    ok(&mut a, "sample.create", json!({ "id": "urban-mobility", "path": dir.path().join("Urban.pqda") }));
    let codes = ok(&mut a, "code.list", Value::Null);
    let reliability = codes.as_array().unwrap().iter().find(|c| c["name"] == "Reliability").unwrap();
    assert_eq!(reliability["rated"], true);
    assert_eq!(reliability["rating_label"], "Intensity");
    let counter = ok(&mut a, "code.counterexamples", json!({ "id": reliability["id"] }));
    assert_eq!(counter.as_array().unwrap().len(), 1);
    assert_eq!(counter[0]["code_name"], "Cost");
    assert!(counter[0]["text"].as_str().unwrap().contains("monthly pass"));
}

// ---- the backup folder ---------------------------------------------------------------------------------

#[test]
fn a_backup_folder_is_checked_kept_used_and_can_be_cleared() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let chosen = dir.path().join("my backups");

    // A folder that is not there, or is a file, is refused with its reason and nothing changes.
    let e = err(&mut s.api, "backup.dir", json!({ "dir": chosen }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.ends_with("was not found."), "{e:?}");
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "x").unwrap();
    assert!(err(&mut s.api, "backup.dir", json!({ "dir": file })).message.ends_with("is a file, not a folder."));
    let beside = ok(&mut s.api, "project.backup", Value::Null);
    assert!(beside["path"].as_str().unwrap().contains(".backups"), "still beside the project: {beside}");
    assert!(!beside["path"].as_str().unwrap().contains("my backups"));
    assert_eq!(beside["note"], Value::Null);

    std::fs::create_dir(&chosen).unwrap();
    assert_eq!(ok(&mut s.api, "backup.dir", json!({ "dir": chosen }))["dir"], chosen.to_str().unwrap());
    let made = ok(&mut s.api, "project.backup", Value::Null);
    let path = std::path::PathBuf::from(made["path"].as_str().unwrap());
    assert!(path.starts_with(&chosen) && path.is_file(), "{made}");
    assert_eq!(made["note"], Value::Null);

    // Null (or empty) is beside the project again.
    assert_eq!(ok(&mut s.api, "backup.dir", json!({ "dir": null }))["dir"], Value::Null);
    assert!(!ok(&mut s.api, "project.backup", Value::Null)["path"].as_str().unwrap().contains("my backups"));
}

#[test]
fn a_folder_handed_over_unchecked_at_start_up_falls_back_with_a_note_when_it_is_gone() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    // The folder the person chose earlier is not there (a drive that is not plugged in): start-up hands it over without refusing it.
    let file = dir.path().join("blocker.txt");
    std::fs::write(&file, "x").unwrap();
    let gone = file.join("backups");
    ok(&mut s.api, "backup.dir", json!({ "dir": gone, "check": false }));
    let made = ok(&mut s.api, "project.backup", Value::Null);
    assert!(made["path"].as_str().unwrap().contains(".backups"), "{made}");
    assert!(made["note"].as_str().unwrap().contains("could not be used"), "{made}");
}
