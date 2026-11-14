//! Linking a revised file to a document, through the command layer: the codings that survive, the backup made first,
//! and every way it is refused.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

const FIRST: &str = "Fares are too high for me.\n\nThe stop is far from my house.\n\nI love the new buses.\n";
const SECOND: &str = "Fares are far too high for me.\n\nI love the new buses.\n";

fn setup(dir: &TempDir) -> (Api, i64, i64, std::path::PathBuf) {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let first = dir.path().join("riders.txt");
    std::fs::write(&first, FIRST).unwrap();
    let source = ok(&mut a, "import.run", json!({ "path": first }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let cost = ok(&mut a, "code.add", json!({ "name": "Cost" }))["id"].as_i64().unwrap();
    let start = FIRST.find("too high").unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": source, "start": start, "end": start + 8, "code_id": cost }));
    let start = FIRST.find("The stop is far").unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": source, "start": start, "end": start + 15, "code_id": cost }));
    let second = dir.path().join("riders-v2.txt");
    std::fs::write(&second, SECOND).unwrap();
    (a, source, cost, second)
}

#[test]
fn a_revised_file_becomes_the_new_text_and_the_codings_follow_it() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, _, second) = setup(&dir);
    let r = ok(&mut a, "import.revise", json!({ "source_id": source, "path": second }));
    assert_eq!(r["outcome"]["codings_removed"], 1, "the sentence about the stop is gone from the new file: {r}");
    assert_eq!(r["outcome"]["codings_moved"], 1, "the other coding moved with its words: {r}");
    assert!(std::path::Path::new(r["backup"].as_str().unwrap()).is_file(), "a backup copy was saved first");

    let detail = ok(&mut a, "source.get", json!({ "id": source }));
    assert_eq!(detail["text"], SECOND.trim_end(), "the importer keeps no trailing line break");
    let sources = ok(&mut a, "source.list", json!({}));
    assert_eq!(sources.as_array().unwrap().len(), 1, "no second document was made");
    assert_eq!(sources[0]["name"], "riders", "the document keeps its name");
    let codings = ok(&mut a, "coding.list", json!({ "source_id": source }));
    assert_eq!(codings.as_array().unwrap().len(), 1);
    let (s, e) = (codings[0]["start"].as_i64().unwrap() as usize, codings[0]["end"].as_i64().unwrap() as usize);
    assert_eq!(&SECOND[s..e], "too high", "the coding follows its words into the new text (words added at its edge are not part of it)");

    let events = ok(&mut a, "events.list", json!({ "limit": 10 }));
    assert!(events.as_array().unwrap().iter().any(|e| e["action"] == "source.revise"), "{events}");
    // Undo does not take this back: the backup is the way back.
    let undo = err(&mut a, "undo", json!({}));
    assert_eq!(undo.code, "invalid");
    assert!(undo.message.contains("linking a revised file"), "{}", undo.message);
}

#[test]
fn a_file_that_cannot_be_a_new_version_is_refused_and_nothing_changes() {
    let dir = TempDir::new().unwrap();
    let (mut a, source, _, second) = setup(&dir);

    let same = dir.path().join("riders.txt");
    let e = err(&mut a, "import.revise", json!({ "source_id": source, "path": same }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("the file the document came from"), "{}", e.message);

    let sheet = dir.path().join("answers.csv");
    std::fs::write(&sheet, "id,answer\n1,Fares are high\n2,The stop is far\n").unwrap();
    let e = err(&mut a, "import.revise", json!({ "source_id": source, "path": sheet }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("spreadsheet"), "{}", e.message);

    assert_eq!(err(&mut a, "import.revise", json!({ "source_id": 9999, "path": second })).code, "not_found");
    assert_eq!(err(&mut a, "import.revise", json!({ "source_id": source, "path": dir.path().join("missing.txt") })).code, "io");
    assert_eq!(err(&mut a, "import.revise", json!({ "path": second })).code, "bad_args");

    assert_eq!(ok(&mut a, "source.get", json!({ "id": source }))["text"], FIRST.trim_end(), "every refusal left the document alone");
    assert_eq!(ok(&mut a, "coding.list", json!({ "source_id": source })).as_array().unwrap().len(), 2);
}
