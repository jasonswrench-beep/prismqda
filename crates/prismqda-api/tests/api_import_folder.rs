//! Importing a whole folder: what goes in, what is skipped and why, and the 500-file speed gate (roadmap M1).

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use prismqda_api::Api;
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn new_api(dir: &TempDir) -> Api {
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Folder import" }));
    api
}

const NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// A minimal Word file holding the given paragraphs.
fn docx(paragraphs: &[String]) -> Vec<u8> {
    let body: String = paragraphs.iter().map(|t| format!(r#"<w:p><w:r><w:t xml:space="preserve">{t}</w:t></w:r></w:p>"#)).collect();
    let parts: [(&str, String); 3] = [
        ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#.to_string()),
        ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_string()),
        ("word/document.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{NS}"><w:body>{body}</w:body></w:document>"#)),
    ];
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, content) in parts {
        w.start_file(name, options).unwrap();
        w.write_all(content.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn put(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_folder_goes_in_file_by_file_and_everything_else_is_reported() {
    let dir = TempDir::new().unwrap();
    let mut api = new_api(&dir);
    let data = dir.path().join("data");
    put(&data.join("interview-1.txt"), b"I came for the community.\n\nThe fees were hard.");
    put(&data.join("interview-2.md"), b"# Second\n\nShe felt welcome from the start.");
    put(&data.join("Wave 2").join("interview-3.docx"), &docx(&["Third interview.".into(), "It went well.".into()]));
    put(&data.join("zz").join("copy-of-1.txt"), b"I came for the community.\n\nThe fees were hard.");
    put(&data.join("Wave 2").join("broken.docx"), b"this is not a zip file at all");
    put(&data.join("scores.csv"), b"id,text\n1,hello\n");
    put(&data.join("photo.tiff"), b"II*\x00");
    put(&data.join("~$interview-3.docx"), b"owner file");

    let r = ok(&mut api, "import.folder", json!({ "path": data }));
    let imported: Vec<&str> = r["imported"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect();
    assert_eq!(imported, ["Wave 2/interview-3.docx", "interview-1.txt", "interview-2.md"], "files go in in path order: {r}");
    assert_eq!(r["sources_created"], 3);

    // The copy of interview 1 was recognised by its contents.
    let dup: Vec<&str> = r["duplicates"].as_array().unwrap().iter().map(|d| d["path"].as_str().unwrap()).collect();
    assert_eq!(dup, ["zz/copy-of-1.txt"]);

    // The broken file is reported with a reason and did not stop the others.
    let failed = r["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["path"], "Wave 2/broken.docx");
    assert_eq!(failed[0]["code"], "bad_file");

    // A spreadsheet, a picture, and Word's own lock file are not imported; the lock file is not even mentioned.
    let skipped: Vec<(&str, &str)> = r["skipped"].as_array().unwrap().iter().map(|s| (s["path"].as_str().unwrap(), s["reason"].as_str().unwrap())).collect();
    assert_eq!(skipped.len(), 2, "{skipped:?}");
    assert!(skipped.iter().any(|(p, why)| *p == "scores.csv" && why.contains("columns")));
    assert!(skipped.iter().any(|(p, why)| *p == "photo.tiff" && why.contains("PNG or JPEG")));

    // Subfolders became folders of sources.
    let sources = ok(&mut api, "source.list", json!({}));
    let mut by_folder: Vec<(String, String)> =
        sources.as_array().unwrap().iter().map(|s| (s["folder"].as_str().unwrap().to_string(), s["name"].as_str().unwrap().to_string())).collect();
    by_folder.sort();
    assert_eq!(by_folder, [("".into(), "interview-1".into()), ("".into(), "interview-2".into()), ("Wave 2".into(), "interview-3".into())]);

    // Running it again imports nothing and says why.
    let again = ok(&mut api, "import.folder", json!({ "path": data }));
    assert_eq!(again["imported"].as_array().unwrap().len(), 0);
    assert_eq!(again["duplicates"].as_array().unwrap().len(), 4, "the three files and the copy are all known now");
}

#[test]
fn options_name_the_folder_limit_the_depth_and_let_spreadsheets_in() {
    let dir = TempDir::new().unwrap();
    let mut api = new_api(&dir);
    let data = dir.path().join("data");
    put(&data.join("top.txt"), b"Top level text.");
    put(&data.join("deep").join("deeper").join("inner.txt"), b"Inner text.");
    put(&data.join("survey.csv"), b"id,comment\n1,I liked it a lot\n2,It was fine\n");

    let r = ok(&mut api, "import.folder", json!({ "path": data, "recursive": false, "folder": "Study A" }));
    assert_eq!(r["sources_created"], 1, "only the top level");
    assert_eq!(ok(&mut api, "source.list", json!({}))[0]["folder"], "Study A");

    let r = ok(&mut api, "import.folder", json!({ "path": data, "folder": "Study A/", "include_tables": true }));
    let imported: Vec<&str> = r["imported"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect();
    assert!(imported.contains(&"deep/deeper/inner.txt") && imported.contains(&"survey.csv"), "{r}");
    let folders: Vec<String> = ok(&mut api, "source.list", json!({})).as_array().unwrap().iter().map(|s| s["folder"].as_str().unwrap().to_string()).collect();
    assert!(folders.contains(&"Study A/deep/deeper".to_string()), "{folders:?}");
}

#[test]
fn a_folder_that_is_not_there_or_no_project_are_plain_errors() {
    let dir = TempDir::new().unwrap();
    let mut api = new_api(&dir);
    assert_eq!(api.call("import.folder", json!({ "path": dir.path().join("nope") })).unwrap_err().code, "invalid");
    assert_eq!(api.call("import.folder", json!({})).unwrap_err().code, "bad_args");
    let mut bare = Api::new("Ada");
    assert_eq!(bare.call("import.folder", json!({ "path": dir.path() })).unwrap_err().code, "no_project");
}

/// Roadmap M1 gate: 500 Word files in under 60 seconds. Run with:
///     cargo test --release -p prismqda-api --test api_import_folder -- --ignored --nocapture
#[test]
#[ignore = "slow: writes and imports 500 Word files"]
fn five_hundred_word_files_import_in_under_a_minute() {
    let dir = TempDir::new().unwrap();
    let mut api = new_api(&dir);
    let data = dir.path().join("data");
    for i in 0..500 {
        let paragraphs: Vec<String> = (0..40).map(|k| format!("Interview {i}, paragraph {k}: I remember the first week, the cost of the course, and how the staff treated us.")).collect();
        put(&data.join(format!("batch {}", i % 10)).join(format!("interview-{i:03}.docx")), &docx(&paragraphs));
    }
    let t = Instant::now();
    let r = ok(&mut api, "import.folder", json!({ "path": data }));
    let secs = t.elapsed().as_secs_f64();
    eprintln!("  500 Word files (40 paragraphs each): {} sources, {} units in {secs:.2} s (target 60 s)", r["sources_created"], r["units_created"]);
    assert_eq!(r["sources_created"], 500);
    assert_eq!(r["failed"].as_array().unwrap().len(), 0);
    assert!(secs < 60.0, "{secs:.1} s is over the 60 second target");
}
