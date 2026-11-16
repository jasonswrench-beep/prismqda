//! Two things that come into PrismQDA from the other programs of the family, through the command layer, and one that goes back:
//!
//! * a `.pstat` file (the project of PrismStatistics) with an evaluation in it: ratings and an open comment for each participant, to be
//!   coded beside the ratings (development list, item 5), and the case table that goes back with the ratings and the codes together;
//! * the exchange file of Prism Reference Manager: sources and the excerpts taken from them, coded straight from the library (item 4).

use std::io::Write;

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn zip_of(entries: &[(&str, String)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(data.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// An evaluation of a workshop as PrismStatistics keeps it: who, a rating from 1 to 5 (9 means no answer), hours of practice, a comment.
fn evaluation_file(dir: &TempDir) -> std::path::PathBuf {
    let project = json!({
        "format": "PrismStatistics project", "version": 1, "app_version": "0.1.1",
        "dataset": {
            "name": "Spring workshop",
            "variables": [
                { "id": 1, "name": "participant", "label": "Participant", "measure": "nominal", "value_labels": [], "missing_values": [] },
                { "id": 2, "name": "rating", "label": "Overall rating", "measure": "ordinal",
                  "value_labels": [{ "value": 1, "label": "Poor" }, { "value": 5, "label": "Excellent" }], "missing_values": [9] },
                { "id": 3, "name": "hours", "label": "Hours of practice", "measure": "scale", "value_labels": [], "missing_values": [] },
                { "id": 4, "name": "comment", "label": "What worked well?", "measure": "nominal", "value_labels": [], "missing_values": [] }
            ],
            "columns": [
                { "type": "text", "values": ["P01", "P02", "P03", "P04"] },
                { "type": "numeric", "values": [5, 2, 9, 4] },
                { "type": "numeric", "values": [3.5, 1, null, 2] },
                { "type": "text", "values": [
                    "The role plays helped me practise the hard conversations.",
                    "The pace was too fast and I got lost in the second half of the day.",
                    null,
                    "The facilitator gave clear examples and the role plays were useful."
                ] }
            ],
            "next_id": 5
        },
        "source": null, "operations": [], "results": []
    });
    let path = dir.path().join("spring-workshop.pstat");
    std::fs::write(&path, zip_of(&[("project.json", project.to_string())])).unwrap();
    path
}

#[test]
fn an_evaluation_kept_in_prismstatistics_is_coded_here_beside_its_ratings_and_goes_back_with_the_codes() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Workshop evaluation" }));
    let file = evaluation_file(&dir);

    // The window shows the file as a table, and guesses who the respondent is and which variable holds the comments.
    let look = ok(&mut a, "import.inspect", json!({ "path": file }));
    assert_eq!(look["kind"], "table", "{look}");
    assert_eq!(look["table"]["row_count"], 4);
    assert_eq!(look["suggested_mapping"]["id_column"], "participant");
    assert_eq!(look["suggested_mapping"]["text_columns"], json!(["comment"]));

    // The person chooses how the ratings come in: as variables of the cases, with their levels.
    let mapping = json!({
        "id_column": "participant",
        "text_columns": ["comment"],
        "attribute_columns": [{ "name": "rating", "level": "ordinal" }, { "name": "hours", "level": "ratio" }]
    });
    let out = ok(&mut a, "import.run", json!({ "path": file, "mapping": mapping }));
    assert_eq!(out["outcome"]["case_ids"].as_array().unwrap().len(), 4);
    let cases = ok(&mut a, "case.list", json!({}));
    assert_eq!(cases.as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect::<Vec<_>>(), ["P01", "P02", "P03", "P04"]);

    // The comments are there to code, and the ratings are variables of the cases, with the file's value labels and missing code.
    let sources = ok(&mut a, "source.list", json!({}));
    let source = sources.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let text = ok(&mut a, "source.get", json!({ "id": source }))["text"].as_str().unwrap().to_string();
    assert!(text.contains("The pace was too fast"), "{text}");
    let vars = ok(&mut a, "stats.variables", json!({}));
    let rating = vars["variables"].as_array().unwrap().iter().find(|v| v["key"] == "attr:rating").expect("the rating is a variable");
    assert_eq!(rating["measure"], "ordinal");
    assert_eq!(rating["valid"], 3, "the code 9 in the file is the file's own mark for no answer, and is set aside");
    assert!(rating["categories"].as_array().unwrap().iter().any(|c| c["label"] == "Excellent"), "the file's value labels came with it: {rating}");

    // Code the comments, then send the table back.
    let code = ok(&mut a, "code.add", json!({ "name": "Role plays" }))["id"].as_i64().unwrap();
    for needle in ["role plays helped", "role plays were useful"] {
        let at = text.find(needle).unwrap();
        let start = text[..at].chars().count() as i64;
        ok(&mut a, "coding.apply", json!({ "source_id": source, "start": start, "end": start + needle.chars().count() as i64, "code_id": code }));
    }
    let target = dir.path().join("back").display().to_string();
    let back = ok(&mut a, "export.pstat", json!({ "path": target, "app_version": "0.1.0-beta.2" }));
    assert_eq!(back["report"]["cases"], 4);

    let mut zip = zip::ZipArchive::new(std::fs::File::open(back["path"].as_str().unwrap()).unwrap()).unwrap();
    let mut json_text = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.json").unwrap(), &mut json_text).unwrap();
    let project: Value = serde_json::from_str(&json_text).unwrap();
    let variables = project["dataset"]["variables"].as_array().unwrap();
    let column = |name: &str| -> Value {
        let at = variables.iter().position(|v| v["name"] == name).unwrap_or_else(|| panic!("no {name}"));
        project["dataset"]["columns"][at]["values"].clone()
    };
    assert_eq!(column("case_name"), json!(["P01", "P02", "P03", "P04"]), "the participant's id is the case's name, so PrismStatistics can match its own data");
    assert_eq!(column("rating"), json!([5, 2, 9, 4]), "the ratings come back as they were, the missing code too");
    let rating_variable = variables.iter().find(|v| v["name"] == "rating").unwrap();
    assert_eq!(rating_variable["missing_values"], json!([9]));
    assert_eq!(rating_variable["value_labels"][1]["label"], "Excellent");
    assert_eq!(rating_variable["label"], "rating", "the label of a variable of cases is its name here");
    // Two excerpts of P01's and P04's comments carry the code; P02's carries none; every case has a document in a survey import.
    let present = column(&format!("code_{code}_present"));
    assert_eq!(present[0], 1);
    assert_eq!(present[1], 0);
    assert_eq!(present[3], 1);
}

#[test]
fn a_file_that_is_not_a_prismstatistics_project_is_told_so_and_nothing_comes_in() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Study" }));
    let bad = dir.path().join("fake.pstat");
    std::fs::write(&bad, "this is not a project").unwrap();
    let e = err(&mut a, "import.inspect", json!({ "path": bad }));
    assert_eq!(e.code, "bad_file");
    assert!(e.message.contains("PrismStatistics project"), "{}", e.message);
    // A newer file names the program that wrote it.
    let newer = dir.path().join("newer.pstat");
    let project = json!({ "format": "PrismStatistics project", "version": 2, "app_version": "OpenAssess 0.2.0", "dataset": {}, "operations": [], "results": [] });
    std::fs::write(&newer, zip_of(&[("project.json", project.to_string())])).unwrap();
    let e = err(&mut a, "import.inspect", json!({ "path": newer }));
    assert!(e.message.contains("OpenAssess 0.2.0"), "{}", e.message);
    assert_eq!(ok(&mut a, "source.list", json!({})).as_array().unwrap().len(), 0, "nothing came in");
}

fn exchange_file(dir: &TempDir) -> std::path::PathBuf {
    let file = json!({
        "manifest": { "format": "prism-family-exchange", "version": "1.0", "producer": "Prism Reference Manager", "producer_version": "0.1.2", "created": "2026-11-15T10:00:00Z", "project": { "id": "p1", "title": "Transit stress review" } },
        "sources": [
            { "id": "s-1", "citekey": "hernandez2021", "csl": { "id": "s-1", "type": "article-journal", "title": "Bus reliability and daily stress",
                "author": [{ "family": "Hernandez", "given": "Maria" }], "issued": { "date-parts": [[2021]] }, "container-title": "Journal of Transit", "abstract": "Late buses raise stress." } },
            { "id": "s-2", "citekey": "okafor2019", "csl": { "id": "s-2", "type": "book", "title": "Waiting", "author": [{ "family": "Okafor", "given": "Ada" }], "issued": { "date-parts": [[2019]] } } }
        ],
        "excerpts": [
            { "id": "e-1", "type": "quotation", "source_ids": ["s-1"], "exact_text": "Riders waited a mean of 14 minutes.", "exact_text_edited": false, "user_text": "Compare with our data.",
              "locator": { "label": "page", "value": "12" }, "anchor_state": "resolved", "origin": "user", "annotation_id": "a-1" },
            { "id": "e-2", "type": "summary", "source_ids": ["s-2"], "exact_text": "", "user_text": "Waiting is a social experience.", "locator": { "label": "", "value": "" }, "anchor_state": "resolved", "origin": "user", "annotation_id": "" }
        ]
    });
    let path = dir.path().join("review-exchange.json");
    std::fs::write(&path, file.to_string()).unwrap();
    path
}

#[test]
fn a_literature_review_is_coded_straight_from_the_reference_managers_exchange_file() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Review" }));
    let file = exchange_file(&dir);

    let look = ok(&mut a, "import.inspect", json!({ "path": file }));
    assert_eq!(look["kind"], "batch", "{look}");
    assert_eq!(look["summary"]["sources"].as_array().unwrap().len(), 2);
    assert_eq!(look["summary"]["sources"][0]["format"], "prism-family-exchange");
    assert_eq!(look["summary"]["cases"], 2);

    ok(&mut a, "import.run", json!({ "path": file }));
    let sources = ok(&mut a, "source.list", json!({}));
    assert_eq!(sources.as_array().unwrap().len(), 2);
    let first = sources.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let text = ok(&mut a, "source.get", json!({ "id": first }))["text"].as_str().unwrap().to_string();
    assert!(text.contains("Bus reliability and daily stress") && text.contains("\u{201c}Riders waited a mean of 14 minutes.\u{201d}"), "{text}");

    // The case holds the source's details and its id in the library.
    let cases = ok(&mut a, "case.list", json!({}));
    assert_eq!(cases.as_array().unwrap().len(), 2);
    let vars = ok(&mut a, "stats.variables", json!({}));
    let names: Vec<&str> = vars["variables"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
    for want in ["Citation key", "Library source id", "Excerpts taken", "Year"] {
        assert!(names.contains(&want), "{want} is a variable of the cases: {names:?}");
    }
}

#[test]
fn an_exchange_file_of_a_version_that_is_not_read_says_which_version_and_who_made_it() {
    let dir = TempDir::new().unwrap();
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Review" }));
    let path = dir.path().join("future.json");
    let file = json!({ "manifest": { "format": "prism-family-exchange", "version": "3.0", "producer": "Prism Reference Manager", "producer_version": "9.0" }, "sources": [], "excerpts": [] });
    std::fs::write(&path, file.to_string()).unwrap();
    let e = err(&mut a, "import.inspect", json!({ "path": path }));
    assert!(e.message.contains("version 3.0") && e.message.contains("Prism Reference Manager 9.0"), "{}", e.message);
}
