//! Exporting through the command layer: files land where asked, are never replaced by accident, and read back.

use prismqda_api::Api;
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

/// A project with one source, two codes (one inside the other), and three codings by two coders.
fn setup(dir: &TempDir) -> (Api, i64, i64) {
    let mut api = Api::new("Ada").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Export test" }));
    let text = dir.path().join("t.txt");
    std::fs::write(&text, "The cost was high. We stayed anyway.\n\nAccess was easy.").unwrap();
    let source = ok(&mut api, "import.run", json!({ "path": text }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let theme = ok(&mut api, "code.add", json!({ "name": "Money", "theme": true }))["id"].as_i64().unwrap();
    let cost = ok(&mut api, "code.add", json!({ "name": "Cost", "parent_id": theme, "definition": "Talk about money." }))["id"].as_i64().unwrap();
    let ben = ok(&mut api, "coder.add", json!({ "name": "Ben" }))["id"].as_i64().unwrap();
    ok(&mut api, "coding.apply", json!({ "source_id": source, "start": 4, "end": 8, "code_id": cost }));
    ok(&mut api, "coding.apply", json!({ "source_id": source, "start": 4, "end": 17, "code_id": cost, "coder_id": ben }));
    (api, source, cost)
}

#[test]
fn the_codebook_is_saved_in_every_format_and_the_ending_is_added() {
    let dir = TempDir::new().unwrap();
    let (mut api, _, _) = setup(&dir);
    for format in ["xlsx", "docx", "csv", "qdc"] {
        let r = ok(&mut api, "export.codebook", json!({ "format": format, "path": dir.path().join("codebook") }));
        let path = r["path"].as_str().unwrap();
        assert!(path.ends_with(&format!("codebook.{format}")), "{path}");
        assert_eq!(std::fs::metadata(path).unwrap().len(), r["bytes"].as_u64().unwrap());
        assert!(!dir.path().join(format!("codebook.{format}.part")).exists(), "no half-written file is left behind");
    }
    // The ending is not doubled when it is already there, whatever its case.
    let r = ok(&mut api, "export.codebook", json!({ "format": "xlsx", "path": dir.path().join("Again.XLSX") }));
    assert!(r["path"].as_str().unwrap().ends_with("Again.XLSX"));
    let qdc = std::fs::read_to_string(dir.path().join("codebook.qdc")).unwrap();
    assert!(qdc.contains("name=\"Cost\"") && qdc.contains("isCodable=\"false\""), "{qdc}");
}

#[test]
fn an_existing_file_is_never_replaced_unless_asked() {
    let dir = TempDir::new().unwrap();
    let (mut api, _, _) = setup(&dir);
    let path = dir.path().join("out.csv");
    std::fs::write(&path, b"precious").unwrap();
    let e = api.call("export.codebook", json!({ "format": "csv", "path": path })).unwrap_err();
    assert_eq!(e.code, "already_exists");
    assert_eq!(std::fs::read(&path).unwrap(), b"precious");
    ok(&mut api, "export.codebook", json!({ "format": "csv", "path": path, "overwrite": true }));
    assert_ne!(std::fs::read(&path).unwrap(), b"precious");
}

#[test]
fn segments_can_be_listed_filtered_and_exported() {
    let dir = TempDir::new().unwrap();
    let (mut api, source, cost) = setup(&dir);
    let all = ok(&mut api, "segments.list", json!({}));
    assert_eq!(all["total"], 2);
    assert_eq!(all["segments"][0]["text"], "cost");
    assert_eq!(all["segments"][1]["text"], "cost was high");
    assert_eq!(ok(&mut api, "segments.list", json!({ "limit": 1 }))["truncated"], true);
    let ben = ok(&mut api, "coder.list", json!({}))[1]["id"].clone();
    let ben_only = ok(&mut api, "segments.list", json!({ "coder_ids": [ben] }));
    assert_eq!(ben_only["total"], 1);
    assert_eq!(ok(&mut api, "segments.list", json!({ "code_ids": [cost], "source_ids": [source] }))["total"], 2);
    assert_eq!(ok(&mut api, "segments.list", json!({ "source_ids": [999] }))["total"], 0);

    for format in ["xlsx", "docx", "csv"] {
        let r = ok(&mut api, "export.segments", json!({ "format": format, "path": dir.path().join("segments"), "code_ids": [cost] }));
        assert!(r["bytes"].as_u64().unwrap() > 100, "{format}");
    }
}

#[test]
fn bad_requests_get_plain_errors() {
    let dir = TempDir::new().unwrap();
    let (mut api, _, _) = setup(&dir);
    let code = |cmd: &str, args: Value, api: &mut Api| api.call(cmd, args).unwrap_err().code;
    assert_eq!(code("export.codebook", json!({ "format": "pdf", "path": dir.path().join("x") }), &mut api), "bad_args");
    assert_eq!(code("export.segments", json!({ "format": "qdc", "path": dir.path().join("x") }), &mut api), "invalid");
    assert_eq!(code("export.codebook", json!({ "format": "xlsx", "path": "" }), &mut api), "invalid");
    assert_eq!(code("export.codebook", json!({ "format": "xlsx", "path": dir.path().join("nope").join("x") }), &mut api), "io");
    assert_eq!(code("export.codebook", json!({ "format": "xlsx" }), &mut api), "bad_args");
    let mut bare = Api::new("Ada");
    assert_eq!(bare.call("export.codebook", json!({ "format": "xlsx", "path": dir.path().join("y") })).unwrap_err().code, "no_project");
}

fn notebook(png: &str) -> Value {
    json!({
        "title": "Notebook",
        "labels": { "table": "Table", "figure": "Figure", "note": "Note.", "contents": "Contents", "checks": "What the test depends on", "alternative": "Text alternative" },
        "pages": [{
            "title": "Page 1: Frequencies of Site",
            "sentence": "Half of the cases were N.",
            "tables": [{ "title": "Frequencies", "head": ["Value", "n"], "rows": [["N", "6"], ["S", "6"]], "note": "Percent of valid cases." }],
            "charts": [{ "title": "Bars of Site", "alt": "Two bars of six cases each.", "png": png, "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"/>", "width": 100, "height": 50 }]
        }]
    })
}

#[test]
fn the_notebook_is_saved_as_word_excel_and_a_web_page_and_a_chart_as_a_picture() {
    let dir = TempDir::new().unwrap();
    // The notebook belongs to no project: it is what the page shows, so no project has to be open.
    let mut api = Api::new("Ada");
    let png = prismqda_export::base64::encode(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4]);
    for (format, ending) in [("docx", "docx"), ("xlsx", "xlsx"), ("html", "html")] {
        let r = ok(&mut api, "export.notebook", json!({ "format": format, "path": dir.path().join("notebook"), "notebook": notebook(&png) }));
        assert!(r["bytes"].as_u64().unwrap() > 300, "{format}");
        assert!(r["path"].as_str().unwrap().ends_with(&format!("notebook.{ending}")), "the ending is added: {r}");
        assert!(dir.path().join(format!("notebook.{ending}")).exists());
    }
    let page = std::fs::read_to_string(dir.path().join("notebook.html")).unwrap();
    assert!(page.contains("Half of the cases were N.") && page.contains("alt=\"Two bars of six cases each.\""));

    let again = api.call("export.notebook", json!({ "format": "html", "path": dir.path().join("notebook"), "notebook": notebook(&png) })).unwrap_err();
    assert_eq!(again.code, "already_exists", "a file is never replaced unless that was said");
    ok(&mut api, "export.notebook", json!({ "format": "html", "path": dir.path().join("notebook"), "overwrite": true, "notebook": notebook(&png) }));

    let saved = ok(&mut api, "export.picture", json!({ "format": "png", "path": dir.path().join("chart"), "data": png }));
    assert_eq!(std::fs::read(dir.path().join("chart.png")).unwrap()[..8], [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    assert_eq!(saved["bytes"], 12);
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\" fill=\"#123456\"/></svg>";
    ok(&mut api, "export.picture", json!({ "format": "svg", "path": dir.path().join("chart"), "data": svg }));
    assert_eq!(std::fs::read_to_string(dir.path().join("chart.svg")).unwrap(), svg);
}

#[test]
fn a_notebook_or_picture_that_cannot_be_saved_gets_a_plain_error() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Ada");
    let png = prismqda_export::base64::encode(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 9]);
    let code = |cmd: &str, args: Value, api: &mut Api| api.call(cmd, args).unwrap_err().code;
    let path = dir.path().join("x");
    assert_eq!(code("export.notebook", json!({ "format": "pdf", "path": path, "notebook": notebook(&png) }), &mut api), "invalid");
    let mut empty = notebook(&png);
    empty["pages"] = json!([]);
    assert_eq!(code("export.notebook", json!({ "format": "docx", "path": path, "notebook": empty }), &mut api), "invalid");
    assert_eq!(code("export.notebook", json!({ "format": "docx", "path": path, "notebook": notebook("bm90IGEgcGljdHVyZQ==") }), &mut api), "invalid");
    assert_eq!(code("export.notebook", json!({ "format": "docx", "path": "", "notebook": notebook(&png) }), &mut api), "invalid");
    assert_eq!(code("export.notebook", json!({ "format": "docx", "path": dir.path().join("nope").join("x"), "notebook": notebook(&png) }), &mut api), "io");
    assert_eq!(code("export.notebook", json!({ "format": "docx", "path": path }), &mut api), "bad_args");
    assert_eq!(code("export.picture", json!({ "format": "png", "path": path, "data": "bm90IGEgcGljdHVyZQ==" }), &mut api), "invalid");
    assert_eq!(code("export.picture", json!({ "format": "png", "path": path, "data": "!!!" }), &mut api), "invalid");
    assert_eq!(code("export.picture", json!({ "format": "svg", "path": path, "data": "<script>alert(1)</script>" }), &mut api), "invalid");
    assert_eq!(code("export.picture", json!({ "format": "gif", "path": path, "data": png }), &mut api), "invalid");
    assert!(!dir.path().join("x.docx").exists() && !dir.path().join("x.png").exists(), "nothing was left behind");
}

#[test]
fn a_script_is_saved_beside_its_data_and_neither_is_replaced_by_accident() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Ada");
    let script = "data <- read.csv(\"{{data-file}}\")\nprint(mean(data$x))\n";
    let r = ok(&mut api, "export.script", json!({ "path": dir.path().join("analysis"), "script": script, "data": "x\n1\n2\n3\n" }));
    assert!(r["path"].as_str().unwrap().ends_with("analysis.R"), "the ending is added: {r}");
    assert!(r["data_path"].as_str().unwrap().ends_with("analysis-data.csv"));
    let written = std::fs::read_to_string(dir.path().join("analysis.R")).unwrap();
    assert_eq!(written, "data <- read.csv(\"analysis-data.csv\")\nprint(mean(data$x))\n", "the script names the data file it was saved with");
    assert_eq!(std::fs::read_to_string(dir.path().join("analysis-data.csv")).unwrap(), "x\n1\n2\n3\n");

    let again = api.call("export.script", json!({ "path": dir.path().join("analysis.R"), "script": script, "data": "x\n9\n" })).unwrap_err();
    assert_eq!(again.code, "already_exists");
    assert_eq!(std::fs::read_to_string(dir.path().join("analysis-data.csv")).unwrap(), "x\n1\n2\n3\n", "the data was not touched either");

    // The script alone being free is not enough: the data file is checked as well, and nothing is written.
    std::fs::remove_file(dir.path().join("analysis.R")).unwrap();
    let blocked = api.call("export.script", json!({ "path": dir.path().join("analysis"), "script": script, "data": "x\n9\n" })).unwrap_err();
    assert_eq!(blocked.code, "already_exists");
    assert!(!dir.path().join("analysis.R").exists());

    ok(&mut api, "export.script", json!({ "path": dir.path().join("analysis"), "overwrite": true, "script": script, "data": "x\n9\n" }));
    assert_eq!(std::fs::read_to_string(dir.path().join("analysis-data.csv")).unwrap(), "x\n9\n");

    let code = |args: Value, api: &mut Api| api.call("export.script", args).unwrap_err().code;
    assert_eq!(code(json!({ "path": dir.path().join("b"), "script": "print(1)", "data": "x" }), &mut api), "invalid", "a script that does not say where its data is");
    assert_eq!(code(json!({ "path": "", "script": script, "data": "x" }), &mut api), "invalid");
    assert_eq!(code(json!({ "path": dir.path().join("no").join("b"), "script": script, "data": "x" }), &mut api), "io");
}
