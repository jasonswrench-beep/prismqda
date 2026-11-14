//! Encrypted projects through the command layer: made encrypted, opened only with the passphrase, turned on and off, and used as
//! any project is (imports, exports, merging, replacing names).

use prismqda_api::Api;
use serde_json::{json, Value};
use tempfile::TempDir;

const PASS: &str = "correct horse battery staple";

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn code(api: &mut Api, cmd: &str, args: Value) -> String {
    api.call(cmd, args).unwrap_err().code
}

fn holds(dir: &std::path::Path, needle: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_file() && std::fs::read(&p).unwrap().windows(needle.len()).any(|w| w == needle) {
            found.push(p.display().to_string());
        }
    }
    found
}

fn add_document(api: &mut Api, dir: &TempDir) -> i64 {
    let text = dir.path().join("Interview with Zelda Quimby.txt");
    std::fs::write(&text, "Zelda Quimby said the bus was late.").unwrap();
    ok(api, "import.run", json!({ "path": text }))["outcome"]["source_ids"][0].as_i64().unwrap()
}

#[test]
fn an_encrypted_project_opens_only_with_its_passphrase_and_the_launcher_can_tell() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut api = Api::new("Ada").without_backup_on_open();
    let info = ok(&mut api, "project.create", json!({ "path": path, "name": "Secret", "passphrase": PASS }));
    assert_eq!(info["encrypted"], true);
    add_document(&mut api, &dir);
    ok(&mut api, "project.close", json!({}));

    // The launcher asks the file system only; the first bytes tell it the file is encrypted.
    let stat = ok(&mut api, "file.stat", json!({ "path": path }));
    assert_eq!(stat["encrypted"], true);
    assert_eq!(ok(&mut api, "file.stat", json!({ "path": dir.path().join("nothing.pqda") }))["encrypted"], false);

    assert_eq!(code(&mut api, "project.open", json!({ "path": path })), "needs_passphrase");
    assert_eq!(code(&mut api, "project.open", json!({ "path": path, "passphrase": "not the right one" })), "wrong_passphrase");
    let info = ok(&mut api, "project.open", json!({ "path": path, "passphrase": PASS }));
    assert_eq!(info["encrypted"], true);
    assert_eq!(info["sources"], 1);
    assert!(holds(dir.path(), b"Zelda").len() == 1, "only the plain text file that was imported holds the name: {:?}", holds(dir.path(), b"Zelda"));
}

#[test]
fn a_short_passphrase_is_refused_with_a_reason_and_nothing_is_made() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut api = Api::new("Ada");
    let e = api.call("project.create", json!({ "path": path, "name": "P", "passphrase": "short" })).unwrap_err();
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("at least 8 characters"), "{}", e.message);
    assert!(!path.exists());
}

#[test]
fn encryption_is_turned_on_changed_and_turned_off_and_the_project_keeps_working() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": path, "name": "Plain at first" }));
    let source = add_document(&mut api, &dir);
    ok(&mut api, "project.backup", json!({}));
    let kept = ok(&mut api, "code.add", json!({ "name": "Lateness" }))["id"].as_i64().unwrap();
    ok(&mut api, "coding.apply", json!({ "source_id": source, "start": 0, "end": 12, "code_id": kept }));

    assert_eq!(code(&mut api, "project.decrypt", json!({ "passphrase": PASS })), "invalid", "it is not encrypted yet");
    assert_eq!(code(&mut api, "project.encrypt", json!({ "passphrase": "short" })), "invalid");
    let out = ok(&mut api, "project.encrypt", json!({ "passphrase": PASS }));
    assert_eq!(out["encrypted"], true);
    assert_eq!(out["info"]["encrypted"], true);
    assert_eq!(out["stale_backups"], 1, "the backup made before is plain");
    assert!(out["cipher"].as_str().unwrap().contains("AES-256-CBC"));

    let stale = ok(&mut api, "project.stale_backups", json!({}));
    assert_eq!(stale["backups"][0]["why"], "plain");
    assert_eq!(ok(&mut api, "project.delete_stale_backups", json!({}))["deleted"], 1);
    assert!(ok(&mut api, "project.stale_backups", json!({}))["backups"].as_array().unwrap().is_empty());

    // Everything works in the encrypted file: segments are listed and exported, backups are made.
    let segments = ok(&mut api, "segments.list", json!({}));
    assert_eq!(segments["total"], 1);
    let csv = ok(&mut api, "export.segments", json!({ "format": "csv", "path": dir.path().join("out.csv") }));
    assert!(std::fs::read_to_string(csv["path"].as_str().unwrap()).unwrap().contains("Zelda"), "an export is a file you chose to make, and holds the words");
    let backup = ok(&mut api, "project.backup", json!({}));
    assert!(std::fs::read(backup["path"].as_str().unwrap()).unwrap().iter().take(16).collect::<Vec<_>>() != b"SQLite format 3\0".iter().collect::<Vec<_>>(), "a backup of an encrypted project is encrypted");

    // Changing the passphrase, then opening with the new one.
    assert_eq!(code(&mut api, "project.change_passphrase", json!({ "old": "wrong wrong wrong", "new": "another passphrase" })), "wrong_passphrase");
    let out = ok(&mut api, "project.change_passphrase", json!({ "old": PASS, "new": "another passphrase" }));
    assert_eq!(out["stale_backups"], 1, "the backup under the old passphrase is named");
    ok(&mut api, "project.close", json!({}));
    assert_eq!(code(&mut api, "project.open", json!({ "path": path, "passphrase": PASS })), "wrong_passphrase");
    ok(&mut api, "project.open", json!({ "path": path, "passphrase": "another passphrase" }));

    // Turning it off gives an ordinary file again.
    assert_eq!(code(&mut api, "project.decrypt", json!({ "passphrase": PASS })), "wrong_passphrase");
    let out = ok(&mut api, "project.decrypt", json!({ "passphrase": "another passphrase" }));
    assert_eq!(out["encrypted"], false);
    ok(&mut api, "project.close", json!({}));
    assert_eq!(ok(&mut api, "file.stat", json!({ "path": path }))["encrypted"], false);
    assert_eq!(ok(&mut api, "project.open", json!({ "path": path }))["encrypted"], false);
}

#[test]
fn a_copy_that_is_encrypted_can_be_merged_with_its_passphrase() {
    let dir = TempDir::new().unwrap();
    let ours = dir.path().join("ours.pqda");
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": ours, "name": "Shared" }));
    add_document(&mut api, &dir);
    let copy = ok(&mut api, "project.backup", json!({}))["path"].as_str().unwrap().to_string();
    let theirs = dir.path().join("theirs.pqda");
    std::fs::copy(&copy, &theirs).unwrap();

    let mut other = Api::new("Ben").without_backup_on_open();
    ok(&mut other, "project.open", json!({ "path": theirs }));
    let text = dir.path().join("another.txt");
    std::fs::write(&text, "Another voice entirely.").unwrap();
    ok(&mut other, "import.run", json!({ "path": text }));
    ok(&mut other, "project.encrypt", json!({ "passphrase": "ben's own passphrase" }));
    ok(&mut other, "project.close", json!({}));

    assert_eq!(code(&mut api, "merge.preview", json!({ "path": theirs })), "needs_passphrase");
    assert_eq!(code(&mut api, "merge.preview", json!({ "path": theirs, "passphrase": "not it at all" })), "wrong_passphrase");
    let r = ok(&mut api, "merge.preview", json!({ "path": theirs, "passphrase": "ben's own passphrase" }));
    assert_eq!(r["documents_added"].as_array().unwrap().len(), 1);
    ok(&mut api, "merge.run", json!({ "path": theirs, "passphrase": "ben's own passphrase" }));
    assert_eq!(ok(&mut api, "source.list", json!({})).as_array().unwrap().len(), 2);
}

#[test]
fn names_can_be_replaced_for_good_in_an_encrypted_project() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": path, "name": "Secret", "passphrase": PASS }));
    let source = add_document(&mut api, &dir);
    ok(&mut api, "deid.add", json!({ "terms": ["Zelda Quimby"], "replacement": "Participant 1" }));
    let out = ok(&mut api, "deid.apply", json!({ "key_path": dir.path().join("key") }));
    assert_eq!(out["documents_changed"], 1);
    assert_eq!(out["compacted"], true, "the file could be rewritten while encrypted");
    assert!(ok(&mut api, "source.get", json!({ "id": source }))["text"].as_str().unwrap().starts_with("Participant 1 said"));
    assert!(out["earlier_backups"].as_u64().is_some());
    // The backup made first is encrypted too, so it holds the names only for someone with the passphrase.
    let backup = out["backup"].as_str().unwrap();
    assert!(!std::fs::read(backup).unwrap().windows(5).any(|w| w == b"Zelda"));
}
