//! PDF pages through the command layer: a PDF is imported with its file kept, a page is drawn when a window asks for it, a rectangle on a
//! page is coded, and what is refused says what to do. The PDF is the fixture the importer's own tests use: three pages, the second blank.

use std::fs;

use prismqda_api::{parse_pdf_page_path, Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn fixture() -> Vec<u8> {
    let path = format!("{}/../prismqda-import/tests/fixtures/pdf/three_pages.pdf", env!("CARGO_MANIFEST_DIR"));
    fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn project(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    a
}

fn write(dir: &TempDir, name: &str, bytes: &[u8]) -> String {
    let path = dir.path().join(name);
    fs::write(&path, bytes).unwrap();
    path.to_string_lossy().to_string()
}

fn code(a: &mut Api, name: &str) -> i64 {
    ok(a, "code.add", json!({ "name": name }))["id"].as_i64().unwrap()
}

#[test]
fn a_pdf_is_imported_with_its_file_and_a_page_is_drawn_when_asked_for() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let path = write(&dir, "Garden Notes.pdf", &fixture());
    let done = ok(&mut a, "import.run", json!({ "path": path }));
    let id = done["outcome"]["source_ids"][0].as_i64().unwrap();

    let list = ok(&mut a, "source.list", json!({}));
    assert_eq!(list[0]["pdf"]["page_count"], 3);
    assert_eq!(list[0]["pdf"]["file_name"], "Garden Notes.pdf");
    assert_eq!(list[0]["source_type"], "pdf");
    assert!(list[0]["image"].is_null(), "it is a PDF, not a picture");
    let pdf = ok(&mut a, "pdf.get", json!({ "source_id": id }));
    assert_eq!(pdf["pages"].as_array().unwrap().len(), 3);
    assert!(pdf["pages"][0]["width"].as_f64().unwrap() > 100.0);
    assert!(ok(&mut a, "source.get", json!({ "id": id }))["text"].as_str().unwrap().contains("Field Notes on a Neighbourhood Garden"));

    let reply = a.pdf_page_reply(id, 1, 600);
    assert_eq!((reply.status, reply.content_type.as_str(), reply.cacheable), (200, "image/png", true));
    assert_eq!(&reply.body[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(u32::from_be_bytes([reply.body[16], reply.body[17], reply.body[18], reply.body[19]]), 600, "drawn at the width asked for");
    // The blank page is drawn too (white paper), and a page that is not there is a 404 with words.
    assert_eq!(a.pdf_page_reply(id, 2, 400).status, 200);
    let gone = a.pdf_page_reply(id, 4, 600);
    assert_eq!(gone.status, 404);
    assert!(String::from_utf8_lossy(&gone.body).contains("3 pages"), "{}", String::from_utf8_lossy(&gone.body));
    assert_eq!(a.pdf_page_reply(9999, 1, 600).status, 404);
    // A picture is not a PDF and a PDF is not asked for as a picture.
    assert_eq!(a.image_reply(id).status, 404);

    // A copy of the file as it was imported.
    let copy = dir.path().join("copy.pdf");
    ok(&mut a, "pdf.save_copy", json!({ "source_id": id, "path": copy }));
    assert_eq!(fs::read(&copy).unwrap(), fixture(), "byte for byte");
    assert_eq!(err(&mut a, "pdf.save_copy", json!({ "source_id": id, "path": "copy.pdf" })).code, "invalid");
}

#[test]
fn a_rectangle_on_a_page_is_coded_listed_with_its_page_and_refused_off_the_page() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let id = ok(&mut a, "import.run", json!({ "path": write(&dir, "Garden Notes.pdf", &fixture()) }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let joy = code(&mut a, "Joy");

    let added = ok(&mut a, "region.add", json!({ "source_id": id, "page": 3, "shape": "rectangle", "x": 0.1, "y": 0.2, "w": 0.5, "h": 0.25, "code_id": joy }));
    assert_eq!(added["region"]["page"], 3);
    assert!(added["region"]["text"].as_str().unwrap().starts_with("Page 3, Rectangle"), "{}", added["region"]["text"]);
    // A page left out is the first, the way a picture's area is added.
    let first = ok(&mut a, "region.add", json!({ "source_id": id, "shape": "rectangle", "x": 0.0, "y": 0.0, "w": 0.2, "h": 0.2, "code_id": joy }));
    assert_eq!(first["region"]["page"], 1);

    let areas = ok(&mut a, "region.list", json!({ "source_id": id }));
    assert_eq!(areas.as_array().unwrap().iter().map(|r| r["page"].as_i64().unwrap()).collect::<Vec<_>>(), [1, 3], "by page");
    let segments = ok(&mut a, "segments.list", json!({}));
    let on_page: Vec<i64> = segments["segments"].as_array().unwrap().iter().filter(|s| s["kind"] == "image").map(|s| s["page"].as_i64().unwrap()).collect();
    assert_eq!(on_page, [1, 3]);
    assert_eq!(ok(&mut a, "source.list", json!({}))[0]["codings"], 2, "the document's count has its areas in it");

    let e = err(&mut a, "region.add", json!({ "source_id": id, "page": 9, "shape": "rectangle", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2, "code_id": joy }));
    assert!(e.message.contains("3 pages"), "{}", e.message);
    let e = err(&mut a, "region.add", json!({ "source_id": id, "page": 1, "shape": "ellipse", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2, "code_id": joy }));
    assert!(e.message.contains("rectangle"), "{}", e.message);
}

#[test]
fn a_pdf_read_as_a_transcript_or_that_is_not_a_pdf_keeps_no_file_and_says_so() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    // A PDF the person chose to read as a transcript is words only.
    let id = ok(&mut a, "import.run", json!({ "path": write(&dir, "Interview.pdf", &fixture()), "source_type": "transcript" }))["outcome"]["source_ids"][0].as_i64().unwrap();
    assert!(ok(&mut a, "pdf.get", json!({ "source_id": id })).is_null());
    assert!(ok(&mut a, "source.list", json!({}))[0]["pdf"].is_null());
    // A file that is not a PDF is refused before anything is kept.
    let e = err(&mut a, "pdf.attach", json!({ "source_id": id, "path": write(&dir, "fake.pdf", b"words, not a pdf") }));
    assert!(e.message.contains("not a PDF"), "{}", e.message);
    assert_eq!(err(&mut a, "pdf.attach", json!({ "source_id": id, "path": "report.pdf" })).code, "invalid", "the full path is asked for");
    assert_eq!(err(&mut a, "pdf.attach", json!({ "source_id": id, "path": dir.path().join("missing.pdf") })).code, "io");
}

#[test]
fn a_document_imported_before_pdfs_were_kept_can_have_its_file_attached_and_then_coded() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let bytes = fixture();
    let id = ok(&mut a, "import.run", json!({ "path": write(&dir, "Old.pdf", &bytes) }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let joy = code(&mut a, "Joy");
    // It already keeps its file, so another cannot be attached.
    let e = err(&mut a, "pdf.attach", json!({ "source_id": id, "path": write(&dir, "Again.pdf", &bytes) }));
    assert!(e.message.contains("already keeps"), "{}", e.message);
    // Make it what a project from before the files were kept holds: the words, and no file.
    let path = dir.path().join("p.pqda");
    ok(&mut a, "project.close", json!({}));
    rusqlite::Connection::open(&path).unwrap().execute_batch("DELETE FROM pdf_page; DELETE FROM pdf_file;").unwrap();
    ok(&mut a, "project.open", json!({ "path": path }));
    assert!(ok(&mut a, "pdf.get", json!({ "source_id": id })).is_null());
    let e = err(&mut a, "region.add", json!({ "source_id": id, "page": 1, "shape": "rectangle", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2, "code_id": joy }));
    assert!(e.message.contains("not a picture or a PDF whose pages are kept"), "{}", e.message);
    // Attaching the file brings the pages, and then an area can be coded on one.
    let kept = ok(&mut a, "pdf.attach", json!({ "source_id": id, "path": write(&dir, "Old again.pdf", &bytes) }));
    assert_eq!((kept["page_count"].as_i64(), kept["file_name"].as_str()), (Some(3), Some("Old again.pdf")));
    ok(&mut a, "region.add", json!({ "source_id": id, "page": 2, "shape": "rectangle", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2, "code_id": joy }));
    assert_eq!(a.pdf_page_reply(id, 2, 500).status, 200);
}

#[test]
fn a_new_version_of_a_document_that_keeps_its_pdf_is_refused_with_the_reason() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let id = ok(&mut a, "import.run", json!({ "path": write(&dir, "Notes.pdf", &fixture()) }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let mut other = fixture();
    other.extend_from_slice(b"\n% a different file\n");
    let e = err(&mut a, "import.revise", json!({ "source_id": id, "path": write(&dir, "Notes v2.pdf", &other) }));
    assert!(e.message.contains("keeps its PDF"), "{}", e.message);
}

#[test]
fn the_address_a_window_asks_for_a_page_by_is_document_page_and_width() {
    assert_eq!(parse_pdf_page_path("12/3/900"), Some((12, 3, 900)));
    assert_eq!(parse_pdf_page_path("12/3"), None);
}
