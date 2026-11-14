//! Pseudonymizing and redacting through the command layer: the list and its suggestions, the switch that applies it to everything
//! exported (checked inside the files), and replacing the names in the project for good with the key kept as a file.

use std::io::Read;

use prismqda_api::Api;
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

const TEXT: &str = "Sarah Jones told me the bus was late. I called her on 555-123-4567 and wrote to sarah@example.org about it.\n\n\
                    We met in Boston because Sarah lives there. Later Sarah Jones said Boston was far, and the Boston trip took an hour.";

fn setup(dir: &TempDir) -> (Api, i64, i64) {
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Deid test" }));
    let text = dir.path().join("Interview with Sarah Jones.txt");
    std::fs::write(&text, TEXT).unwrap();
    let source = ok(&mut api, "import.run", json!({ "path": text }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let code = ok(&mut api, "code.add", json!({ "name": "Lateness", "definition": "Talk about delay, as Sarah Jones put it." }))["id"].as_i64().unwrap();
    let start = TEXT.find("the bus was late").unwrap() as i64;
    ok(&mut api, "coding.apply", json!({ "source_id": source, "start": start, "end": start + 16, "code_id": code }));
    let start = TEXT.find("Sarah Jones told me").unwrap() as i64;
    ok(&mut api, "coding.apply", json!({ "source_id": source, "start": start, "end": start + 11, "code_id": code, "rationale": "Sarah is named" }));
    (api, source, code)
}

fn zip_text(path: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut all = String::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            all.push_str(&s);
        }
    }
    all
}

fn add_list(api: &mut Api) {
    ok(api, "deid.add", json!({ "terms": ["Sarah Jones", "Sarah"], "replacement": "Participant 3" }));
    ok(api, "deid.add", json!({ "terms": ["sarah@example.org"], "replacement": "[email]", "kind": "redact", "whole_word": false }));
    ok(api, "deid.add", json!({ "terms": ["555-123-4567"], "replacement": "[phone]", "kind": "redact" }));
    ok(api, "deid.add", json!({ "terms": ["Boston"], "replacement": "City A", "match_case": true }));
}

#[test]
fn the_program_suggests_names_and_numbers_and_a_full_name_shares_a_label_with_its_parts() {
    let dir = TempDir::new().unwrap();
    let (mut api, ..) = setup(&dir);
    let out = ok(&mut api, "deid.suggest", json!({}));
    let list = out["suggestions"].as_array().unwrap();
    let find = |term: &str| list.iter().find(|s| s["term"] == term).unwrap_or_else(|| panic!("{term} not suggested in {list:?}")).clone();
    assert_eq!(find("sarah@example.org")["kind"], "email");
    assert_eq!(find("sarah@example.org")["replacement"], "[email]");
    assert_eq!(find("555-123-4567")["kind"], "phone");
    let (full, part) = (find("Sarah Jones"), find("Sarah"));
    assert_eq!(full["kind"], "name");
    assert_eq!(full["replacement"], part["replacement"], "the first name and the full name are one person");
    assert_eq!(full["count"], 2, "{full}");
    assert_eq!(part["count"], 1, "Sarah alone, outside the full name: {part}");
    assert_eq!(find("Boston")["count"], 3);
    assert!(list.iter().all(|s| s["term"] != "Later" && s["term"] != "We"), "words that begin a sentence are not names");

    // What is already in the list is not suggested again.
    ok(&mut api, "deid.add", json!({ "terms": ["Boston"], "replacement": "City A" }));
    let again = ok(&mut api, "deid.suggest", json!({}));
    assert!(again["suggestions"].as_array().unwrap().iter().all(|s| s["term"] != "Boston"));
}

#[test]
fn the_preview_counts_what_the_list_would_find_and_shows_the_words_around_it() {
    let dir = TempDir::new().unwrap();
    let (mut api, ..) = setup(&dir);
    add_list(&mut api);
    let p = ok(&mut api, "deid.preview", json!({}));
    assert_eq!(p["total"], 8, "{p}");
    assert_eq!(p["sources"].as_array().unwrap().len(), 1);
    let sample = p["samples"].as_array().unwrap().iter().find(|s| s["before"].as_str().unwrap().contains("555-123-4567")).unwrap();
    assert!(sample["after"].as_str().unwrap().contains("[phone]") && !sample["after"].as_str().unwrap().contains("555"));
    let places: Vec<&str> = p["elsewhere"].as_array().unwrap().iter().map(|e| e["place"].as_str().unwrap()).collect();
    assert!(places.contains(&"Document names") && places.contains(&"The codebook"), "{places:?}");
    // Nothing was changed.
    assert_eq!(ok(&mut api, "deid.state", json!({}))["rules"].as_array().unwrap().len(), 5);
}

#[test]
fn nothing_in_a_file_that_is_exported_while_the_switch_is_on_names_anyone() {
    let dir = TempDir::new().unwrap();
    let (mut api, ..) = setup(&dir);
    add_list(&mut api);

    // With the switch off the files hold the names.
    let plain = ok(&mut api, "export.segments", json!({ "format": "csv", "path": dir.path().join("plain.csv") }));
    assert!(std::fs::read_to_string(plain["path"].as_str().unwrap()).unwrap().contains("Sarah"));

    ok(&mut api, "deid.set_exports", json!({ "on": true }));
    let mut files: Vec<String> = Vec::new();
    for format in ["csv", "xlsx", "docx"] {
        files.push(ok(&mut api, "export.segments", json!({ "format": format, "path": dir.path().join("segments") }))["path"].as_str().unwrap().to_string());
    }
    for format in ["csv", "xlsx", "docx", "qdc"] {
        files.push(ok(&mut api, "export.codebook", json!({ "format": format, "path": dir.path().join("codebook") }))["path"].as_str().unwrap().to_string());
    }
    files.push(ok(&mut api, "export.report", json!({ "title": "Report on Sarah Jones", "sections": ["study", "documents", "codebook", "passages", "memos"], "path": dir.path().join("report") }))["path"].as_str().unwrap().to_string());
    for path in &files {
        let text = if path.ends_with(".csv") || path.ends_with(".qdc") { std::fs::read_to_string(path).unwrap() } else { zip_text(path) };
        for name in ["Sarah", "sarah@", "555-123", "Boston"] {
            assert!(!text.contains(name), "{path} still holds {name}");
        }
    }
    let csv = std::fs::read_to_string(&files[0]).unwrap();
    assert!(csv.contains("Participant 3") && csv.contains("Interview with Participant 3"), "{csv}");
    let docx = zip_text(files.iter().find(|f| f.ends_with("segments.docx")).unwrap());
    assert!(docx.contains("Participant 3"));
    let qdc = std::fs::read_to_string(files.iter().find(|f| f.ends_with(".qdc")).unwrap()).unwrap();
    assert!(qdc.contains("as Participant 3 put it"), "{qdc}");

    // The project itself was not changed, and turning the switch off brings the names back to what is exported.
    let state = ok(&mut api, "deid.state", json!({}));
    assert_eq!(state["exports"], true);
    assert!(ok(&mut api, "source.get", json!({ "id": 1 }))["text"].as_str().unwrap().contains("Sarah Jones told me"));
    ok(&mut api, "deid.set_exports", json!({ "on": false }));
    let again = ok(&mut api, "export.segments", json!({ "format": "csv", "path": dir.path().join("again.csv") }));
    assert!(std::fs::read_to_string(again["path"].as_str().unwrap()).unwrap().contains("Sarah"));
}

#[test]
fn the_switch_cannot_be_turned_on_with_an_empty_list_and_a_picture_is_flagged() {
    let dir = TempDir::new().unwrap();
    let (mut api, ..) = setup(&dir);
    assert_eq!(api.call("deid.set_exports", json!({ "on": true })).unwrap_err().code, "invalid");
    add_list(&mut api);
    ok(&mut api, "deid.set_exports", json!({ "on": true }));
    let png = prismqda_export_png();
    let saved = ok(&mut api, "export.picture", json!({ "format": "png", "data": png, "path": dir.path().join("chart") }));
    assert_eq!(saved["names_not_replaced"], true, "a picture cannot be changed, and the person is told");
    ok(&mut api, "deid.set_exports", json!({ "on": false }));
    let saved = ok(&mut api, "export.picture", json!({ "format": "png", "data": prismqda_export_png(), "path": dir.path().join("chart2") }));
    assert!(saved.get("names_not_replaced").is_none());
}

/// A one-pixel PNG as base 64.
fn prismqda_export_png() -> String {
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".to_string()
}

#[test]
fn replacing_for_good_writes_the_key_first_and_changes_nothing_when_it_cannot() {
    let dir = TempDir::new().unwrap();
    let (mut api, source, _) = setup(&dir);
    add_list(&mut api);
    let key = dir.path().join("key.csv");
    std::fs::write(&key, b"already here").unwrap();

    // The key file is there already, so nothing is written and nothing is changed.
    let e = api.call("deid.apply", json!({ "key_path": key })).unwrap_err();
    assert_eq!(e.code, "already_exists");
    assert!(ok(&mut api, "source.get", json!({ "id": source }))["text"].as_str().unwrap().contains("Sarah Jones"));
    assert_eq!(ok(&mut api, "deid.state", json!({}))["rules"].as_array().unwrap().len(), 5);

    ok(&mut api, "project.backup", json!({}));
    let out = ok(&mut api, "deid.apply", json!({ "key_path": dir.path().join("mykey") }));
    assert_eq!(out["earlier_backups"], 1, "the backup made before holds the names too, and the person is told");
    assert_eq!(out["documents_changed"], 1);
    assert_eq!(out["replacements"], 8);
    assert_eq!(out["remaining"].as_array().unwrap().len(), 0, "{out}");
    let saved_key = out["key_path"].as_str().unwrap();
    assert!(saved_key.ends_with("mykey.csv"));
    let key_text = std::fs::read_to_string(saved_key).unwrap();
    assert!(key_text.contains("Original,Replacement,Kind,Match case,Whole word,Note") && key_text.contains("Sarah Jones,Participant 3"), "{key_text}");
    assert!(std::path::Path::new(out["backup"].as_str().unwrap()).exists());

    let text = ok(&mut api, "source.get", json!({ "id": source }))["text"].as_str().unwrap().to_string();
    assert!(text.contains("Participant 3 told me") && text.contains("[email]") && !text.contains("Sarah") && !text.contains("Boston"), "{text}");
    let sources = ok(&mut api, "source.list", json!({}));
    assert_eq!(sources[0]["name"], "Interview with Participant 3");
    assert_eq!(ok(&mut api, "deid.state", json!({}))["rules"].as_array().unwrap().len(), 0, "the list is the key, so it is gone from the project");
    let codings = ok(&mut api, "segments.list", json!({}));
    let quotes: Vec<&str> = codings["segments"].as_array().unwrap().iter().map(|s| s["text"].as_str().unwrap()).collect();
    assert!(quotes.contains(&"Participant 3") && quotes.contains(&"the bus was late"), "{quotes:?}");

    // The key brings the list back (for a document added later that has the same names in it).
    let back = ok(&mut api, "deid.load_key", json!({ "path": saved_key }));
    assert_eq!(back["added"], 5);
    let state = ok(&mut api, "deid.state", json!({}));
    let rules = state["rules"].as_array().unwrap();
    assert!(rules.iter().any(|r| r["term"] == "Boston" && r["match_case"] == true && r["replacement"] == "City A"), "{rules:?}");
    assert!(rules.iter().any(|r| r["term"] == "sarah@example.org" && r["kind"] == "redact" && r["whole_word"] == false));
}

#[test]
fn a_replacement_that_could_break_a_spreadsheet_is_refused() {
    let dir = TempDir::new().unwrap();
    let (mut api, ..) = setup(&dir);
    for bad in ["Smith, J.", "Say \"hi\"", "two\nlines"] {
        let e = api.call("deid.add", json!({ "terms": ["Sarah"], "replacement": bad })).unwrap_err();
        assert_eq!(e.code, "invalid", "{bad}");
    }
    let e = api.call("deid.load_key", json!({ "path": dir.path().join("nothing.csv") })).unwrap_err();
    assert_eq!(e.code, "io");
}

// ---------------------------------------------------------------- a copy made in one step (decision 180)

/// The first bytes of a PNG that says it is `w` by `h` pixels.
fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 2, 0, 0, 0, 1, 2, 3, 4]);
    b
}

#[test]
fn a_copy_in_one_step_replaces_the_names_takes_out_pictures_and_recording_links_and_keeps_the_project_and_the_key() {
    let dir = TempDir::new().unwrap();
    let (mut api, source, code) = setup(&dir);
    add_list(&mut api);

    // A picture named after the person with a coded area, and a recording linked to the interview.
    let photo = dir.path().join("Sarah Jones at the stop.png");
    std::fs::write(&photo, png_bytes(640, 480)).unwrap();
    let picture = ok(&mut api, "import.run", json!({ "path": photo }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let area = ok(&mut api, "region.add", json!({ "source_id": picture, "shape": "rectangle", "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4, "code_id": code }))["coding_id"].as_i64().unwrap();
    // A link from that area to a passage of the interview: it goes with the area in the copy, and does not wait there for a later area that is given the same number.
    let passage = ok(&mut api, "coding.list", json!({ "source_id": source }))[0]["id"].as_i64().unwrap();
    ok(&mut api, "link.add", json!({ "from_coding": area, "to_coding": passage, "relation": "echoes" }));
    let recording = dir.path().join("sarah-jones-call.wav");
    std::fs::write(&recording, vec![0u8; 4000]).unwrap();
    ok(&mut api, "media.link", json!({ "source_id": source, "path": recording }));

    let key = dir.path().join("key.csv");
    let copy = dir.path().join("for-the-team");
    let made = ok(&mut api, "deid.copy", json!({ "path": copy, "key_path": key }));
    assert!(made["path"].as_str().unwrap().ends_with("for-the-team.pqda"), "the ending is added: {made}");
    assert_eq!(made["taken_out"].as_array().unwrap().len(), 1);
    assert_eq!((made["taken_out"][0]["kind"].as_str(), made["taken_out"][0]["areas_removed"].as_i64()), (Some("picture"), Some(1)));
    assert_eq!(made["recordings_unlinked"], 1);
    assert!(made["outcome"]["backup"].as_str().unwrap().is_empty());
    assert_eq!(made["key_path"].as_str().unwrap(), key.to_string_lossy());
    assert!(std::fs::read_to_string(&key).unwrap().contains("Sarah Jones"), "the key has the names");

    // The project is as it was: its picture, its area, its recording, and its list.
    assert_eq!(ok(&mut api, "image.list", json!({})).as_array().unwrap().len(), 1);
    assert_eq!(ok(&mut api, "media.list", json!({})).as_array().unwrap().len(), 1);
    assert_eq!(ok(&mut api, "link.relations", json!({}))["used"].as_array().unwrap().len(), 1, "and its link");
    assert_eq!(ok(&mut api, "deid.state", json!({}))["rules"].as_array().unwrap().len(), 5);
    assert!(ok(&mut api, "source.get", json!({ "id": source }))["text"].as_str().unwrap().contains("Sarah"));

    // The copy, opened by another program instance: no picture, no recording, no area, no list, and no name anywhere in the file.
    let mut other = Api::new("Ben").without_backup_on_open();
    ok(&mut other, "project.open", json!({ "path": copy.with_extension("pqda"), "backup": false }));
    assert!(ok(&mut other, "image.list", json!({})).as_array().unwrap().is_empty());
    assert!(ok(&mut other, "media.list", json!({})).as_array().unwrap().is_empty());
    assert!(ok(&mut other, "deid.state", json!({}))["rules"].as_array().unwrap().is_empty());
    assert!(ok(&mut other, "link.relations", json!({}))["used"].as_array().unwrap().is_empty(), "the link of the removed area is not left in the copy");
    let sources = ok(&mut other, "source.list", json!({}));
    let names: Vec<&str> = sources.as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.iter().all(|n| !n.contains("Sarah")), "{names:?}");
    assert!(ok(&mut other, "source.get", json!({ "id": source }))["text"].as_str().unwrap().contains("Participant 3 told me"));
    ok(&mut other, "project.close", json!({}));
    let bytes = std::fs::read(copy.with_extension("pqda")).unwrap();
    let hay = String::from_utf8_lossy(&bytes);
    assert!(!hay.contains("Sarah") && !hay.contains("sarah@") && !hay.contains("555-123") && !hay.contains("Boston"), "the copy still holds a name");
    // Nothing else was left beside the copy: no backup folder of the copy.
    assert!(!dir.path().join("for-the-team.backups").exists());
}

#[test]
fn a_copy_of_an_encrypted_project_is_encrypted_with_its_passphrase_and_a_copy_is_refused_over_a_file_or_without_a_list() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("e.pqda"), "name": "Secret", "passphrase": "correct horse battery" }));
    let text = dir.path().join("Sarah Jones notes.txt");
    std::fs::write(&text, TEXT).unwrap();
    ok(&mut api, "import.run", json!({ "path": text }));

    let refused = api.call("deid.copy", json!({ "path": dir.path().join("a") })).unwrap_err();
    assert_eq!(refused.code, "invalid", "no list, nothing to replace");
    assert!(!dir.path().join("a.pqda").exists());

    add_list(&mut api);
    let taken = dir.path().join("taken.pqda");
    std::fs::write(&taken, b"not yours to replace").unwrap();
    let refused = api.call("deid.copy", json!({ "path": taken })).unwrap_err();
    assert_eq!(refused.code, "already_exists");
    assert_eq!(std::fs::read(&taken).unwrap(), b"not yours to replace");

    ok(&mut api, "deid.copy", json!({ "path": dir.path().join("shared") }));
    let mut other = Api::new("Ben").without_backup_on_open();
    let no_passphrase = other.call("project.open", json!({ "path": dir.path().join("shared.pqda"), "backup": false })).unwrap_err();
    assert_eq!(no_passphrase.code, "needs_passphrase", "the copy is encrypted too");
    ok(&mut other, "project.open", json!({ "path": dir.path().join("shared.pqda"), "backup": false, "passphrase": "correct horse battery" }));
    assert!(ok(&mut other, "source.list", json!({}))[0]["name"].as_str().unwrap().contains("Participant 3"));
}
