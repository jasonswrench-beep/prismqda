//! A class exercise through the command layer: a packet for every student, the packets brought back from one folder, the review of the class,
//! and the table of students saved as a file.

use prismqda_api::Api;
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

const PARAGRAPHS: [&str; 3] = ["Fares are too high for me.", "The stop is far from my house.", "I love the new buses."];

fn text() -> String {
    PARAGRAPHS.join("\n\n")
}

/// Codes the paragraph `which` of the open project's only document with the code named `code`, as the person using the project.
fn code_paragraph(api: &mut Api, which: usize, code: &str) {
    let source = ok(api, "source.list", json!({}))[0]["id"].as_i64().unwrap();
    let id = ok(api, "code.list", json!({})).as_array().unwrap().iter().find(|c| c["name"] == code).unwrap()["id"].as_i64().unwrap();
    let start: usize = PARAGRAPHS[..which].iter().map(|p| p.chars().count() + 2).sum();
    ok(api, "coding.apply", json!({ "source_id": source, "start": start, "end": start + PARAGRAPHS[which].chars().count(), "code_id": id }));
}

#[test]
fn a_class_is_given_packets_brings_them_back_and_is_reviewed() {
    let dir = TempDir::new().unwrap();
    let mut api = Api::new("Teacher").without_backup_on_open();
    ok(&mut api, "project.create", json!({ "path": dir.path().join("Exercise.pqda"), "name": "Exercise" }));
    let file = dir.path().join("Riders.txt");
    std::fs::write(&file, text()).unwrap();
    ok(&mut api, "import.run", json!({ "path": file }));
    ok(&mut api, "code.add", json!({ "name": "Cost" }));
    ok(&mut api, "code.add", json!({ "name": "Access" }));
    code_paragraph(&mut api, 0, "Cost");
    code_paragraph(&mut api, 1, "Access");
    code_paragraph(&mut api, 2, "Cost");

    // A packet for each student, in a folder that is made.
    let folder = dir.path().join("class packets");
    let made = ok(&mut api, "class.packets", json!({ "folder": folder, "students": ["Annika", "Bohdan"], "instructions": "Code each paragraph." }));
    assert_eq!(made["made"], 2);
    assert!(folder.join("Exercise - packet for Annika.pqda").is_file(), "named for the project and the student");
    // The packets are refused while the list of names is applied to exports (a packet holds the documents whole).
    ok(&mut api, "deid.add", json!({ "terms": ["Fares"], "replacement": "Prices" }));
    ok(&mut api, "deid.set_exports", json!({ "on": true }));
    let refused = api.call("class.packets", json!({ "folder": dir.path().join("other"), "students": ["Cyrus"] })).unwrap_err();
    assert!(refused.to_string().contains("list of names"), "{refused}");
    ok(&mut api, "deid.set_exports", json!({ "on": false }));
    assert_eq!(api.call("class.packets", json!({ "folder": "", "students": ["A"] })).unwrap_err().code, "invalid");

    // The students code, each in their own packet, as themselves.
    for (student, work) in [("Annika", vec![(0, "Cost"), (1, "Access"), (2, "Cost")]), ("Bohdan", vec![(0, "Cost"), (1, "Cost")])] {
        let mut theirs = Api::new("Student").without_backup_on_open();
        ok(&mut theirs, "project.open", json!({ "path": folder.join(format!("Exercise - packet for {student}.pqda")) }));
        for (which, code) in work {
            code_paragraph(&mut theirs, which, code);
        }
        ok(&mut theirs, "project.close", json!({}));
    }

    // Back in the instructor's project: one call brings both in.
    let back = ok(&mut api, "class.collect", json!({ "folder": folder }));
    assert_eq!(back["packets"], 2);
    assert_eq!(back["codings_added"], 5);
    assert!(back["backup"].is_string());
    assert!(ok(&mut api, "class.collect", json!({ "folder": folder }))["files"].as_array().unwrap().iter().any(|f| f["status"] == "nothing_new"), "the second time nothing is new");
    assert_eq!(api.call("class.collect", json!({ "folder": dir.path().join("nowhere") })).unwrap_err().code, "invalid");

    // The review: the instructor is the person using the project; the students are everyone else.
    let review = ok(&mut api, "class.review", json!({}));
    assert_eq!(review["instructor"]["name"], "Teacher");
    assert_eq!(review["instructor_passages"], 3);
    let students = review["students"].as_array().unwrap();
    assert_eq!(students.iter().map(|s| s["label"].as_str().unwrap()).collect::<Vec<_>>(), ["Annika", "Bohdan"]);
    assert_eq!(students[0]["same"], 3);
    assert_eq!([&students[1]["same"], &students[1]["different"], &students[1]["left_uncoded"]], [&json!(1), &json!(1), &json!(1)]);
    assert_eq!(review["hardest"][0]["text"], PARAGRAPHS[1], "the passage the fewest students coded as the instructor did");
    let anonymous = ok(&mut api, "class.review", json!({ "anonymous": true }));
    assert!(!anonymous["students"].to_string().contains("Annika"));

    // The table of students, as a file; a file that is there is not replaced.
    let csv = dir.path().join("class.csv");
    let saved = ok(&mut api, "export.class_review", json!({ "path": csv }));
    assert_eq!(saved["rows"], 2);
    let written = std::fs::read_to_string(&csv).unwrap();
    assert!(written.starts_with('\u{FEFF}') && written.contains("Student,Passages coded,Same codes as the instructor"), "{written}");
    assert!(written.contains("Annika,3,3,0,0,0,0,3,0,0,100.0,100.0,0.0"), "{written}");
    assert!(written.contains("Bohdan,2,1,0,1,1,0,1,2,1,33.3,33.3,50.0"), "{written}");
    assert_eq!(api.call("export.class_review", json!({ "path": csv })).unwrap_err().code, "already_exists");

    // The heat map comes with the review, and the side-by-side view and both can be saved as tables (decision 198).
    let heat = review["heat_map"].clone();
    assert_eq!(heat["codes"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Access", "Cost"], "the hardest code first");
    assert_eq!(heat["rows"].as_array().unwrap().len(), 2);
    assert_eq!(heat["rows"][1]["found"], json!([0.0, 0.5]), "Bohdan gave Cost where the instructor gave Access, and found one of the two passages with Cost");
    let side = ok(&mut api, "class.side_by_side", json!({ "anonymous": true }));
    assert_eq!(side["students"], json!(["Student 1", "Student 2"]));
    assert!(!side.to_string().contains("Annika") && !side.to_string().contains("Bohdan"), "{side}");
    assert_eq!(side["passages"][0]["text"], PARAGRAPHS[1]);
    assert_eq!(side["passages"].as_array().unwrap().len(), 2, "the two passages some student coded differently");
    let named = ok(&mut api, "class.side_by_side", json!({ "unit_ids": [side["passages"][1]["unit_id"]] }));
    assert_eq!(named["passages"].as_array().unwrap().len(), 1);
    assert_eq!(named["students"], json!(["Annika", "Bohdan"]));
    assert_eq!(api.call("class.side_by_side", json!({ "unit_ids": [987654] })).unwrap_err().code, "not_found");
    let heat_file = dir.path().join("heat.csv");
    let saved = ok(&mut api, "export.class_heat_map", json!({ "path": heat_file }));
    assert_eq!((saved["rows"].clone(), saved["columns"].clone()), (json!(2), json!(2)));
    let written = std::fs::read_to_string(&heat_file).unwrap();
    assert!(written.contains("Student,Access (% found),Cost (% found)") && written.contains("Annika,100.0,100.0") && written.contains("Bohdan,0.0,50.0"), "{written}");
    assert_eq!(api.call("export.class_heat_map", json!({ "path": heat_file })).unwrap_err().code, "already_exists");
    let side_file = dir.path().join("side.csv");
    assert_eq!(ok(&mut api, "export.class_side_by_side", json!({ "path": side_file }))["rows"], 2);
    let written = std::fs::read_to_string(&side_file).unwrap();
    assert!(written.contains("Document,Passage,Instructor,Annika,Bohdan") && written.contains("The stop is far from my house.,Access,Access,Cost"), "{written}");

    // A Word file of feedback for each student (decision 197), in a folder that is made; a student reads only their own.
    let feedback = dir.path().join("feedback");
    let made = ok(&mut api, "export.class_feedback", json!({ "folder": feedback, "anonymous": true }));
    assert_eq!(made["written"], 2);
    let files: Vec<(String, i64)> = made["files"].as_array().unwrap().iter().map(|f| (f["student"].as_str().unwrap().to_string(), f["passages"].as_i64().unwrap())).collect();
    assert_eq!(files, [("Annika".to_string(), 0), ("Bohdan".to_string(), 2)], "named for the student although the review may be anonymous; Annika matched on every passage, Bohdan on one");
    let bohdan = feedback.join("Bohdan - feedback.docx");
    assert!(bohdan.is_file() && feedback.join("Annika - feedback.docx").is_file());
    let text = prismqda_import::docx::parse(&std::fs::read(&bohdan).unwrap(), &prismqda_import::ImportOptions::default()).unwrap().text;
    assert!(text.contains("For Bohdan") && text.contains("The stop is far from my house.") && text.contains("Your codes: Cost") && text.contains("The instructor's codes: Access"), "{text}");
    assert!(!text.contains("Annika"), "a student's file holds nobody else's name");
    // A file that is there is not replaced, and nothing at all is written when one is.
    let refused = api.call("export.class_feedback", json!({ "folder": feedback })).unwrap_err();
    assert_eq!(refused.code, "already_exists");
    assert_eq!(ok(&mut api, "export.class_feedback", json!({ "folder": feedback, "overwrite": true }))["written"], 2);
    assert_eq!(api.call("export.class_feedback", json!({ "folder": "" })).unwrap_err().code, "invalid");
}
