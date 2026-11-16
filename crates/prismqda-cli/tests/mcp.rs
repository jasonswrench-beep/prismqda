//! `prismqda mcp` (decision 203): the assistant route spoken over a pipe, as an assistant application would start it.

use std::io::Write;
use std::process::{Command, Stdio};

use prismqda_engine::{ImportBatch, ImportedSource, ImportedUnit, NewCode, NewProject, OpenOptions, Project, SourceType, UnitKind};
use serde_json::{json, Value};
use tempfile::TempDir;

fn options() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn project(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("pipe.pqda");
    let mut p = Project::create(&path, &NewProject::new("Pipe"), &options()).unwrap();
    let text = "Fares are too high for me.";
    let n = text.chars().count() as i64;
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Riders".into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text: text.into(),
            runs: vec![],
            units: vec![ImportedUnit::new(0, n, UnitKind::Paragraph)],
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap();
    p.add_code(&NewCode::new("Cost")).unwrap();
    path
}

/// Starts `prismqda mcp` with `args`, sends `lines`, closes the pipe, and gives what came back on each stream.
fn session(path: &std::path::Path, args: &[&str], lines: &[String]) -> (bool, Vec<Value>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prismqda"))
        .arg("mcp")
        .arg(path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for l in lines {
            writeln!(stdin, "{l}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    let replies = String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("not JSON: {l}"))).collect();
    (out.status.success(), replies, String::from_utf8_lossy(&out.stderr).to_string())
}

fn request(id: i64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

#[test]
fn an_assistant_application_can_start_it_list_the_tools_read_and_is_refused_what_it_may_not_do() {
    let dir = TempDir::new().unwrap();
    let path = project(&dir);
    let lines = vec![
        request(1, "initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "Test client", "version": "1" } })),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
        request(2, "tools/list", json!({})),
        request(3, "tools/call", json!({ "name": "list_documents", "arguments": {} })),
        request(4, "tools/call", json!({ "name": "propose_codings", "arguments": { "items": [] } })),
        request(5, "no/such/method", json!({})),
        "this is not json".to_string(),
        request(6, "ping", json!({})),
    ];
    let (ok, replies, stderr) = session(&path, &[], &lines);
    assert!(ok, "{stderr}");
    assert_eq!(replies.len(), 7, "the notification got no reply: {replies:?}");
    assert_eq!(replies[0]["id"], 1);
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "prismqda");
    assert!(replies[0]["result"]["capabilities"]["tools"].is_object());
    let tools: Vec<&str> = replies[1]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(tools.len(), 7);
    assert!(!tools.contains(&"propose_codings"), "a read session is not offered it");
    let docs: Value = serde_json::from_str(replies[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(docs["documents"][0]["name"], "Riders");
    assert_eq!(replies[2]["result"]["isError"], false);
    assert_eq!(replies[3]["result"]["isError"], true, "a tool this session may not use is an error the assistant can read");
    assert!(replies[3]["result"]["content"][0]["text"].as_str().unwrap().contains("read only"));
    assert_eq!(replies[4]["error"]["code"], -32601);
    assert_eq!(replies[5]["error"]["code"], -32700);
    assert!(replies[5]["id"].is_null());
    assert_eq!(replies[6]["result"], json!({}));
    assert!(stderr.contains("2 tool calls, 0 suggestions"), "{stderr}");
}

#[test]
fn a_session_that_proposes_must_say_whose_coding_it_would_become_and_then_can_suggest() {
    let dir = TempDir::new().unwrap();
    let path = project(&dir);
    let (ok, _, stderr) = session(&path, &["--permission", "propose"], &[]);
    assert!(!ok && stderr.contains("say whose coding"), "{stderr}");

    let docs = request(1, "tools/call", json!({ "name": "list_documents", "arguments": {} }));
    let (_, first, _) = session(&path, &["--permission", "propose", "--actor", "Reader"], &[docs]);
    let id = serde_json::from_str::<Value>(first[0]["result"]["content"][0]["text"].as_str().unwrap()).unwrap()["documents"][0]["document_id"].as_i64().unwrap();
    let suggest = request(2, "tools/call", json!({ "name": "propose_codings", "arguments": { "items": [{ "document_id": id, "quote": "Fares are too high", "code": "Cost" }] } }));
    let list = request(3, "tools/list", json!({}));
    let (ok, replies, stderr) = session(&path, &["--permission", "propose", "--actor", "Reader"], &[list, suggest]);
    assert!(ok, "{stderr}");
    assert_eq!(replies[0]["result"]["tools"].as_array().unwrap().len(), 8);
    let result: Value = serde_json::from_str(replies[1]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(result["suggested"], 1, "{result}");
    assert!(stderr.contains("1 suggestion sent to the review queue"), "{stderr}");

    // The suggestion waits in the project for the person.
    let p = Project::open(&path, &options()).unwrap();
    assert_eq!(p.list_proposals(Some("pending"), None, 10).unwrap().len(), 1);
    assert_eq!(p.list_codings(id).unwrap().len(), 0, "nothing was coded");
}

#[test]
fn the_route_does_not_open_when_the_project_says_no_or_the_permission_is_not_one() {
    let dir = TempDir::new().unwrap();
    let path = project(&dir);
    let (ok, _, stderr) = session(&path, &["--permission", "apply", "--actor", "Reader"], &[]);
    assert!(!ok && stderr.contains("read or propose"), "{stderr}");
    {
        let mut p = Project::open(&path, &options()).unwrap();
        p.set_ai_mode("off").unwrap();
    }
    let (ok, replies, stderr) = session(&path, &[], &[request(1, "ping", json!({}))]);
    assert!(!ok && replies.is_empty() && stderr.contains("AI is switched off"), "{stderr}");
    {
        let mut p = Project::open(&path, &options()).unwrap();
        p.set_ai_mode("packet").unwrap();
        p.set_offline_lock(true).unwrap();
    }
    let (ok, _, stderr) = session(&path, &[], &[]);
    assert!(!ok && stderr.contains("offline lock"), "{stderr}");
}
