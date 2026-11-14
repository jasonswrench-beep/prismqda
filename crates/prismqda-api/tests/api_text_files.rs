//! `text.read` and `text.write`: a small text file in and out, with the encodings spreadsheets write.

use std::fs;

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

#[test]
fn a_file_is_read_in_the_encoding_it_was_written_in() {
    let dir = TempDir::new().unwrap();
    let mut api = api();
    let utf8 = dir.path().join("utf8.csv");
    fs::write(&utf8, "Message,Traducci\u{f3}n\nSave,Guardar\n").unwrap();
    let r = ok(&mut api, "text.read", json!({ "path": utf8 }));
    assert_eq!(r["text"], "Message,Traducci\u{f3}n\nSave,Guardar\n");
    assert_eq!(r["encoding"], "utf-8");
    assert_eq!(r["name"], "utf8.csv");

    let bom = dir.path().join("bom.csv");
    fs::write(&bom, b"\xEF\xBB\xBFa,b").unwrap();
    assert_eq!(ok(&mut api, "text.read", json!({ "path": bom }))["text"], "a,b", "a byte-order mark is not part of the text");

    // Excel's plain "CSV (Comma delimited)" on Windows
    let ansi = dir.path().join("ansi.csv");
    fs::write(&ansi, b"Caf\xE9,caf\xE9").unwrap();
    let r = ok(&mut api, "text.read", json!({ "path": ansi }));
    assert_eq!(r["text"], "Caf\u{e9},caf\u{e9}");
    assert_eq!(r["encoding"], "windows-1252", "the window can tell the person that accents may have gone wrong");

    // Excel's "Unicode Text"
    let wide = dir.path().join("wide.txt");
    let mut bytes = vec![0xFF, 0xFE];
    for u in "Hi\t\u{e9}".encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    fs::write(&wide, bytes).unwrap();
    let r = ok(&mut api, "text.read", json!({ "path": wide }));
    assert_eq!(r["text"], "Hi\t\u{e9}");
    assert_eq!(r["encoding"], "utf-16");
}

#[test]
fn a_file_that_cannot_be_read_is_refused_with_a_reason() {
    let dir = TempDir::new().unwrap();
    let mut api = api();
    assert!(err(&mut api, "text.read", json!({ "path": "fr.csv" })).message.contains("full path"), "a relative path says what to type");
    assert!(err(&mut api, "text.read", json!({ "path": "" })).message.contains("full path"));
    let missing = dir.path().join("none.csv");
    assert!(err(&mut api, "text.read", json!({ "path": missing })).message.starts_with("cannot read"));
    assert!(err(&mut api, "text.read", json!({ "path": dir.path() })).message.contains("is not a file"));
}

#[test]
fn text_is_written_as_utf8_and_a_file_that_is_there_is_not_replaced_unasked() {
    let dir = TempDir::new().unwrap();
    let mut api = api();
    let target = dir.path().join("blank.csv");
    let r = ok(&mut api, "text.write", json!({ "path": target, "text": "\u{feff}Message,Translation\n" }));
    assert_eq!(r["bytes"], 3 + "Message,Translation\n".len());
    assert_eq!(fs::read(&target).unwrap(), b"\xEF\xBB\xBFMessage,Translation\n");

    let e = err(&mut api, "text.write", json!({ "path": target, "text": "other" }));
    assert!(e.message.contains("already exists"), "{}", e.message);
    assert_eq!(fs::read(&target).unwrap(), b"\xEF\xBB\xBFMessage,Translation\n", "nothing was changed");

    ok(&mut api, "text.write", json!({ "path": target, "text": "other", "replace": true }));
    assert_eq!(fs::read_to_string(&target).unwrap(), "other");

    let nowhere = dir.path().join("no folder").join("x.csv");
    assert!(err(&mut api, "text.write", json!({ "path": nowhere, "text": "x" })).message.contains("does not exist"));
    assert!(err(&mut api, "text.write", json!({ "path": "x.csv", "text": "x" })).message.contains("full path"));
}

#[test]
fn neither_command_needs_a_project() {
    let dir = TempDir::new().unwrap();
    let mut api = api();
    let p = dir.path().join("t.txt");
    ok(&mut api, "text.write", json!({ "path": p, "text": "x" }));
    assert_eq!(ok(&mut api, "text.read", json!({ "path": p }))["text"], "x");
}
