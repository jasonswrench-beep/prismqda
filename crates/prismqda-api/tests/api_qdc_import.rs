//! A codebook file (`.qdc`) read into the open project through the command layer (decision 160): the tree, the themes, the colours and the
//! criteria come back, reading the same file twice adds nothing, and files that are not codebooks are refused in words.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn new_project(dir: &TempDir, file: &str) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join(file), "name": "Study" }));
    a
}

fn find<'a>(codes: &'a Value, name: &str) -> &'a Value {
    codes.as_array().unwrap().iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("no code named {name} in {codes}"))
}

#[test]
fn a_codebook_written_by_prismqda_comes_back_with_its_tree_themes_colours_and_criteria() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir, "a.pqda");
    let theme = ok(&mut a, "code.add", json!({ "name": "Belonging", "theme": true, "color": "#aa3355" }))["id"].as_i64().unwrap();
    ok(&mut a, "code.add", json!({ "name": "Welcome", "parent_id": theme, "definition": "Being greeted.", "inclusion_criteria": "A greeting is described.", "exclusion_criteria": "A complaint only." }));
    ok(&mut a, "code.add", json!({ "name": "Market", "color": "#11aa22" }));
    let file = dir.path().join("book").display().to_string();
    let out = ok(&mut a, "export.codebook", json!({ "format": "qdc", "path": file }));
    let qdc = out["path"].as_str().unwrap().to_string();

    let mut b = new_project(&dir, "b.pqda");
    let r = ok(&mut b, "import.qdc", json!({ "path": qdc }));
    assert_eq!(r["report"]["added"], 3, "{r}");
    assert_eq!(r["report"]["themes"], 1, "{r}");
    assert_eq!(r["report"]["already_there"], 0, "{r}");
    let codes = ok(&mut b, "code.list", json!({}));
    let belonging = find(&codes, "Belonging");
    assert_eq!(belonging["kind"], "theme");
    assert_eq!(belonging["color"], "#aa3355");
    let welcome = find(&codes, "Welcome");
    assert_eq!(welcome["parent_id"], belonging["id"], "the tree is kept");
    assert_eq!(welcome["definition"], "Being greeted.");
    assert_eq!(welcome["inclusion_criteria"], "A greeting is described.");
    assert_eq!(welcome["exclusion_criteria"], "A complaint only.");
    assert_eq!(find(&codes, "Market")["color"], "#11aa22");

    // the same file again adds nothing
    let again = ok(&mut b, "import.qdc", json!({ "path": qdc }));
    assert_eq!(again["report"]["added"], 0, "{again}");
    assert_eq!(again["report"]["already_there"], 3, "{again}");
    assert_eq!(ok(&mut b, "code.list", json!({})).as_array().unwrap().len(), 3);
}

#[test]
fn a_code_already_there_keeps_its_place_and_new_children_go_under_it() {
    let dir = TempDir::new().unwrap();
    let mut a = new_project(&dir, "a.pqda");
    let t = ok(&mut a, "code.add", json!({ "name": "Work" }))["id"].as_i64().unwrap();
    ok(&mut a, "code.add", json!({ "name": "Pay", "parent_id": t }));
    let file = dir.path().join("book").display().to_string();
    let qdc = ok(&mut a, "export.codebook", json!({ "format": "qdc", "path": file }))["path"].as_str().unwrap().to_string();

    let mut b = new_project(&dir, "b.pqda");
    let mine = ok(&mut b, "code.add", json!({ "name": "work", "color": "#010203" }))["id"].as_i64().unwrap();
    let r = ok(&mut b, "import.qdc", json!({ "path": qdc }));
    assert_eq!((r["report"]["added"].as_i64(), r["report"]["already_there"].as_i64()), (Some(1), Some(1)), "{r}");
    let codes = ok(&mut b, "code.list", json!({}));
    assert_eq!(find(&codes, "work")["color"], "#010203", "an existing code is left as it is");
    assert_eq!(find(&codes, "Pay")["parent_id"], mine);
}

#[test]
fn what_is_not_a_codebook_is_refused_in_words_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut b = new_project(&dir, "b.pqda");
    let write = |name: &str, bytes: &[u8]| {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p.display().to_string()
    };
    let e = err(&mut b, "import.qdc", json!({ "path": write("a.qdc", b"just words") }));
    assert!(e.message.contains("XML") || e.message.contains("well-formed") || e.message.contains("not"), "{}", e.message);
    let e = err(&mut b, "import.qdc", json!({ "path": write("b.qdc", b"<Project xmlns=\"urn:QDA-XML:project:1.0\"/>") }));
    assert!(e.message.contains("Import QDPX"), "{}", e.message);
    let e = err(&mut b, "import.qdc", json!({ "path": write("c.qdc", b"<Other/>") }));
    assert!(e.message.contains("CodeBook"), "{}", e.message);
    let e = err(&mut b, "import.qdc", json!({ "path": write("d.qdc", b"<CodeBook/>") }));
    assert!(e.message.contains("no Codes"), "{}", e.message);
    let e = err(&mut b, "import.qdc", json!({ "path": write("e.qdc", &[0xFF, 0xFE, 0x3C, 0x00]) }));
    assert!(e.message.contains("UTF-16"), "{}", e.message);
    let e = err(&mut b, "import.qdc", json!({ "path": dir.path().join("missing.qdc") }));
    assert_eq!(e.code, "io");
    assert_eq!(ok(&mut b, "code.list", json!({})).as_array().unwrap().len(), 0);
}

#[test]
fn a_codebook_from_another_program_with_a_byte_order_mark_and_odd_input_is_read_safely() {
    let dir = TempDir::new().unwrap();
    let mut b = new_project(&dir, "b.pqda");
    let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<CodeBook xmlns=\"urn:QDA-XML:codebook:1.0\"><Codes>\
        <Code guid=\"1\" name=\"  Joy  \" isCodable=\"true\" color=\"#0F0\"/>\
        <Code guid=\"2\" name=\"\" isCodable=\"false\"><Code guid=\"3\" name=\"Inside\"/></Code>\
        <Code guid=\"4\" name=\"Odd &amp; even\" color=\"nonsense\"/></Codes><Sets><Set name=\"S\"/></Sets></CodeBook>";
    let path = dir.path().join("x.qdc");
    std::fs::write(&path, xml).unwrap();
    let r = ok(&mut b, "import.qdc", json!({ "path": path }));
    assert_eq!(r["report"]["added"], 4, "{r}");
    assert_eq!(r["report"]["themes"], 1);
    assert!(r["report"]["left_out"][0].as_str().unwrap().contains("1 set"), "{r}");
    let codes = ok(&mut b, "code.list", json!({}));
    assert_eq!(find(&codes, "Joy")["color"], "#00ff00", "a three-digit colour is expanded");
    assert_eq!(find(&codes, "Code")["kind"], "theme", "a code with no name gets one");
    assert_eq!(find(&codes, "Odd & even")["color"], "#808080", "a colour that is not one is left out");
    assert_eq!(find(&codes, "Inside")["parent_id"], find(&codes, "Code")["id"]);
}
