//! The assistant route (decision 203): what an assistant connected over the Model Context Protocol may read and propose.

use prismqda_engine::{assistant_tools, AssistantPermission, CodeEdit, Error, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, SourceType, UnitKind, MAX_PROPOSALS};
use serde_json::{json, Value};
use tempfile::TempDir;

fn options() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn batch(name: &str, paragraphs: &[&str]) -> ImportBatch {
    let mut text = String::new();
    let mut units = Vec::new();
    for para in paragraphs {
        let start = text.chars().count() as i64;
        text.push_str(para);
        units.push(ImportedUnit::new(start, text.chars().count() as i64, UnitKind::Paragraph));
        text.push_str("\n\n");
    }
    ImportBatch {
        sources: vec![ImportedSource {
            name: name.into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text,
            runs: vec![],
            units,
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    }
}

const RIDERS: [&str; 4] = ["Fares are too high for me.", "The stop is far from my house.", "I love the new buses.", "Nothing to add here."];

struct W {
    p: Project,
    _dir: TempDir,
    riders: i64,
    secret: i64,
    reader: i64,
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("a.pqda"), &NewProject::new("Assistant"), &options()).unwrap();
    let riders = p.apply_import(&batch("Riders", &RIDERS)).unwrap().source_ids[0];
    let secret = p.apply_import(&batch("Secret", &["Hidden words here."])).unwrap().source_ids[0];
    p.set_source_do_not_share(secret, true).unwrap();
    for (name, def) in [("Cost", "Money."), ("Access", "Getting to the service."), ("Private", "Hidden."), ("Manual", "By hand only.")] {
        let id = p.add_code(&NewCode::new(name)).unwrap();
        p.update_code(id, &CodeEdit { definition: Some(def.into()), ..Default::default() }).unwrap();
    }
    let by = |p: &Project, name: &str| p.list_codes().unwrap().into_iter().find(|c| c.name == name).unwrap().id;
    let (private, manual) = (by(&p, "Private"), by(&p, "Manual"));
    p.update_code(private, &CodeEdit { do_not_share: Some(true), ..Default::default() }).unwrap();
    p.update_code(manual, &CodeEdit { ai_suggestible: Some(false), ..Default::default() }).unwrap();
    let reader = p.add_coder(&NewCoder::human("Reader")).unwrap();
    let (cost, units) = (by(&p, "Cost"), p.list_units(riders).unwrap());
    p.apply_coding(&NewCoding::new(riders, units[0].start, units[0].end, cost, reader)).unwrap();
    p.apply_coding(&NewCoding::new(riders, units[1].start, units[1].end, private, reader)).unwrap();
    W { p, _dir: dir, riders, secret, reader }
}

fn call(w: &mut W, tool: &str, args: Value, permission: AssistantPermission) -> Result<Value, Error> {
    let reader = w.reader;
    w.p.assistant_call(tool, &args, permission, "Test assistant", reader)
}

fn read(w: &mut W, tool: &str, args: Value) -> Value {
    call(w, tool, args, AssistantPermission::Read).unwrap_or_else(|e| panic!("{tool}: {e}"))
}

#[test]
fn a_read_session_is_offered_the_reading_tools_and_a_propose_session_one_more() {
    let reading = assistant_tools(AssistantPermission::Read);
    let names: Vec<&str> = reading.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["project_overview", "list_documents", "read_document", "list_codes", "list_codings", "find_text", "word_frequencies"]);
    assert!(reading.iter().all(|t| t["annotations"]["readOnlyHint"] == true && t["inputSchema"]["type"] == "object" && !t["description"].as_str().unwrap().is_empty()));
    let proposing = assistant_tools(AssistantPermission::Propose);
    assert_eq!(proposing.len(), 8);
    assert_eq!(proposing[7]["name"], "propose_codings");
    assert_eq!(proposing[7]["annotations"]["readOnlyHint"], false);
    assert!("Propose".parse::<AssistantPermission>().is_ok() && "apply".parse::<AssistantPermission>().is_err(), "there is no level that applies codings");
}

#[test]
fn an_assistant_reads_what_is_shared_and_nothing_marked_do_not_share() {
    let mut w = world();
    let (riders, secret) = (w.riders, w.secret);
    let overview = read(&mut w, "project_overview", json!({}));
    assert_eq!((overview["documents"].as_i64(), overview["documents_withheld"].as_i64(), overview["codes"].as_i64(), overview["codes_withheld"].as_i64()), (Some(1), Some(1), Some(3), Some(1)));
    let docs = read(&mut w, "list_documents", json!({}));
    assert_eq!(docs["documents"].as_array().unwrap().len(), 1);
    assert_eq!(docs["documents"][0]["name"], "Riders");
    let secret = call(&mut w, "read_document", json!({ "document_id": secret }), AssistantPermission::Read);
    assert!(matches!(secret, Err(Error::Invalid(ref m)) if m.contains("do not share")), "{secret:?}");

    let page = read(&mut w, "read_document", json!({ "document_id": riders }));
    assert!(page["text"].as_str().unwrap().starts_with("Fares are too high for me."));
    assert!(page["next_start"].is_null(), "the whole document fits in one read");
    let window = read(&mut w, "read_document", json!({ "document_id": riders, "start": 28, "end": 40 }));
    assert_eq!(window["text"], "The stop is ");
    assert_eq!(window["next_start"], 40);

    let codes = read(&mut w, "list_codes", json!({}));
    let names: Vec<&str> = codes["codes"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Access", "Cost", "Manual"], "the code marked do not share is not listed");
    assert_eq!(codes["codes"][1]["definition"], "Money.");
    assert_eq!(codes["codes"][2]["may_be_suggested"], false);

    let codings = read(&mut w, "list_codings", json!({ "document_id": riders }));
    let listed: Vec<(&str, &str)> = codings["codings"].as_array().unwrap().iter().map(|c| (c["code"].as_str().unwrap(), c["words"].as_str().unwrap())).collect();
    assert_eq!(listed, [("Cost", "Fares are too high for me.")], "the coding with the hidden code is not listed");

    let found = read(&mut w, "find_text", json!({ "query": "fares" }));
    assert_eq!(found["total"], 1);
    let hidden = read(&mut w, "find_text", json!({ "query": "hidden" }));
    assert_eq!(hidden["total"], 0, "the words of a document marked do not share are not searched");
    let words = read(&mut w, "word_frequencies", json!({ "limit": 5 }));
    assert!(words["words"].as_array().unwrap().iter().all(|x| x["word"] != "hidden"));
    assert!(call(&mut w, "no_such_tool", json!({}), AssistantPermission::Read).is_err());
}

#[test]
fn the_offline_lock_and_the_ai_switch_close_the_route() {
    let mut w = world();
    let _ = &w;
    assert!(w.p.assistant_check().is_ok());
    w.p.set_offline_lock(true).unwrap();
    assert!(matches!(w.p.assistant_check(), Err(Error::Invalid(ref m)) if m.contains("offline lock")));
    assert!(call(&mut w, "list_documents", json!({}), AssistantPermission::Read).is_err(), "no tool runs while the route is closed");
    w.p.set_offline_lock(false).unwrap();
    w.p.set_ai_mode("off").unwrap();
    assert!(matches!(w.p.assistant_check(), Err(Error::Invalid(ref m)) if m.contains("AI is switched off")));
    w.p.set_ai_mode("packet").unwrap();
    assert!(w.p.assistant_check().is_ok());
}

#[test]
fn a_read_session_cannot_propose_and_a_propose_session_sends_checked_suggestions_to_the_review_queue() {
    let mut w = world();
    let (riders, secret) = (w.riders, w.secret);
    let item = json!({ "items": [{ "document_id": riders, "quote": "stop is far", "code": "Access", "reason": "about getting there" }] });
    let refused = call(&mut w, "propose_codings", item.clone(), AssistantPermission::Read);
    assert!(matches!(refused, Err(Error::Invalid(ref m)) if m.contains("read only")), "{refused:?}");

    let before = w.p.list_codings(riders).unwrap().len();
    let items = json!({ "items": [
        { "document_id": riders, "quote": "stop is far", "code": "Access", "reason": "about getting there" },
        { "document_id": riders, "quote": "love  the\nnew buses", "code": "access" },
        { "document_id": riders, "quote": "zebra", "code": "Access" },
        { "document_id": riders, "quote": "Fares", "code": "Private" },
        { "document_id": riders, "quote": "Fares", "code": "Manual" },
        { "document_id": secret, "quote": "Hidden", "code": "Access" },
        { "document_id": riders, "quote": "stop is far", "code": "Access" },
        { "quote": "no document" },
    ] });
    let out = call(&mut w, "propose_codings", items, AssistantPermission::Propose).unwrap();
    assert_eq!((out["read"].as_i64(), out["suggested"].as_i64()), (Some(8), Some(2)), "{out}");
    let why: Vec<(i64, String)> = out["rejected"].as_array().unwrap().iter().map(|r| (r["number"].as_i64().unwrap(), r["why"].as_str().unwrap().to_string())).collect();
    assert_eq!(why.iter().map(|(n, _)| *n).collect::<Vec<_>>(), [3, 4, 5, 6, 7, 8]);
    assert!(why[0].1.contains("word for word") && why[1].1.contains("not an active code you may suggest") && why[2].1.contains("not an active code you may suggest") && why[3].1.contains("not one you may read"));
    assert!(why[4].1.contains("already in this call") && why[5].1.contains("needs a document_id"));
    let batch = out["batch"].as_str().unwrap().to_string();

    // They wait for the researcher: nothing is coded, the review queue holds them, and the run is on record as an assistant's.
    assert_eq!(w.p.list_codings(riders).unwrap().len(), before, "an assistant never applies a coding");
    let queue = w.p.list_proposals(Some("pending"), Some(&batch), 50).unwrap();
    assert_eq!(queue.len(), 2);
    assert!(queue.iter().all(|p| p.origin == "ai" && p.coder_id == w.reader));
    assert!(queue.iter().any(|p| p.rationale.contains("about getting there")), "{:?}", queue.iter().map(|p| &p.rationale).collect::<Vec<_>>());
    let runs = w.p.ai_runs().unwrap();
    assert_eq!((runs[0].route.as_str(), runs[0].model.as_str()), ("mcp", "Test assistant"));
    assert!(w.p.events(5).unwrap().iter().any(|e| e.action == "ai.answer"));

    // The same again adds nothing.
    let again = call(&mut w, "propose_codings", item, AssistantPermission::Propose).unwrap();
    assert_eq!((again["suggested"].as_i64(), again["already_waiting"].as_i64()), (Some(0), Some(1)));
    assert!(again["batch"].is_null());
    let none = call(&mut w, "propose_codings", json!({ "items": [] }), AssistantPermission::Propose);
    assert!(matches!(none, Err(Error::Invalid(_))));
    let many: Vec<Value> = (0..=MAX_PROPOSALS).map(|_| json!({ "document_id": 1, "quote": "x", "code": "Cost" })).collect();
    assert!(matches!(call(&mut w, "propose_codings", json!({ "items": many }), AssistantPermission::Propose), Err(Error::Invalid(_))));
}
