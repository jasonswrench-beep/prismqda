//! Taking a revised file as the new version of a document: the document stays the same document, its codings follow the words
//! that survive, the file's formatting replaces the old, units the revised text has are added, and what was lost is on record.

use prismqda_engine::{
    AppliesTo, AttributeSpec, FormatRun, ImportBatch, ImportedSource, ImportedUnit, Level, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project,
    RunKind, SourceType, UnitKind,
};
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn file(name: &str, text: &str, runs: Vec<FormatRun>, units: Vec<ImportedUnit>, hash: &str) -> ImportedSource {
    ImportedSource {
        name: name.into(),
        source_type: SourceType::Document,
        folder: String::new(),
        text: text.into(),
        runs,
        units,
        default_unit_kind: UnitKind::Sentence,
        unit_attributes: vec![],
        original_file_hash: Some(hash.into()),
        settings: serde_json::Value::Null,
        warnings: vec![],
    }
}

fn at(text: &str, needle: &str) -> i64 {
    text.find(needle).unwrap_or_else(|| panic!("{needle:?} is not in the text")) as i64
}

#[test]
fn a_revised_file_keeps_the_document_and_the_codings_that_survive_and_records_the_one_that_did_not() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let old = "Intro\nI felt welcome. The bus was late. Fares are high.";
    let new = "Intro\nI felt very welcome. Fares are high.\nMore\n";
    let src = p.add_source(&NewSource::new("Interview", SourceType::Document, old)).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Welcome")).unwrap();
    let bus = p.add_code(&NewCode::new("Bus")).unwrap();
    let cost = p.add_code(&NewCode::new("Cost")).unwrap();
    let span = |needle: &str| (at(old, needle), at(old, needle) + needle.chars().count() as i64);
    for (needle, c) in [("felt welcome", code), ("The bus was late.", bus), ("Fares are high", cost)] {
        let (s, e) = span(needle);
        p.apply_coding(&NewCoding::new(src, s, e, c, ada)).unwrap();
    }
    let before_revision = p.source_detail(src).unwrap().text_revision;

    let heading = |name: &str| FormatRun { start: at(new, name), end: at(new, name) + name.len() as i64, kind: RunKind::Heading, level: Some(1), page: None };
    let revised = file("interview-v2.txt", new, vec![heading("Intro"), heading("More")], vec![], "feedface");
    let out = p.revise_source(src, &revised).unwrap();

    assert_eq!(out.text_revision, before_revision + 1);
    assert_eq!((out.codings_removed, out.units_added), (1, 0), "the sentence about the bus is not in the new text");
    assert!(out.codings_moved >= 1);
    let detail = p.source_detail(src).unwrap();
    assert_eq!(detail.name, "Interview", "it is the same document, under its own name");
    assert_eq!(detail.text, new);
    assert_eq!(detail.formatting_runs.as_array().unwrap().len(), 2, "the new file's headings replace the old formatting");
    assert_eq!(p.find_source_by_hash("feedface").unwrap(), Some(src), "it now remembers the file it came from");

    let codings = p.list_codings(src).unwrap();
    let words: Vec<(i64, String)> = codings.iter().map(|c| (c.code_id, p.text_of_span(src, c.start, c.end).unwrap())).collect();
    assert_eq!(words.len(), 2);
    assert!(words.contains(&(code, "felt very welcome".to_string())), "{words:?}");
    assert!(words.contains(&(cost, "Fares are high".to_string())), "{words:?}");

    // The audit trail keeps the coding that went, with its words, and says it was a revision.
    let event = p.events(10).unwrap().into_iter().find(|e| e.action == "source.revise").expect("a source.revise event");
    let removed = event.before.as_ref().unwrap()["removed_codings"].as_array().unwrap();
    assert_eq!((removed.len(), removed[0]["text"].as_str()), (1, Some("The bus was late.")));
    assert_eq!(event.after.as_ref().unwrap()["from_file"], "interview-v2.txt");
    assert_eq!(event.after.as_ref().unwrap()["original_file_hash"], "feedface");
}

#[test]
fn units_the_revised_text_has_are_added_and_the_ones_with_codings_keep_their_numbers() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let old = "Priya: The bus is late.\nSam: I walk.\n";
    let new = "Priya: The bus is late.\nSam: I walk.\nPriya: It costs a lot.\n";
    let turn = |text: &str, label: &str, words: &str| {
        let start = at(text, words);
        ImportedUnit { speaker: Some(label.to_string()), ..ImportedUnit::new(start, start + words.chars().count() as i64, UnitKind::SpeakerTurn) }
    };
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Talk".into(),
            source_type: SourceType::Transcript,
            folder: String::new(),
            text: old.into(),
            runs: vec![],
            units: vec![turn(old, "Priya", "The bus is late."), turn(old, "Sam", "I walk.")],
            default_unit_kind: UnitKind::SpeakerTurn,
            unit_attributes: vec![AttributeSpec::new("speaker", AppliesTo::Unit, Level::Nominal)],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let src = p.apply_import(&batch).unwrap().source_ids[0];
    let units_before = p.list_units(src).unwrap();
    assert_eq!(units_before.len(), 2);

    let mut revised = file("talk-v2.txt", new, vec![], vec![turn(new, "Priya", "The bus is late."), turn(new, "Sam", "I walk."), turn(new, "Priya", "It costs a lot.")], "abc");
    revised.source_type = SourceType::Transcript;
    let out = p.revise_source(src, &revised).unwrap();
    assert_eq!(out.units_added, 1, "only the new turn is added; the two the document already had are not doubled");
    let units_after = p.list_units(src).unwrap();
    assert_eq!(units_after.len(), 3);
    for before in &units_before {
        assert!(units_after.iter().any(|u| u.id == before.id), "unit {} keeps its number, so what points at it still does", before.id);
    }
    let added = units_after.iter().find(|u| !units_before.iter().any(|b| b.id == u.id)).unwrap();
    assert_eq!(added.speaker.as_deref(), Some("Priya"));
}

#[test]
fn a_file_with_the_same_words_can_still_bring_new_formatting() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let text = "Findings\nIt rained.";
    let src = p.add_source(&NewSource::new("Report", SourceType::Document, text)).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Weather")).unwrap();
    p.apply_coding(&NewCoding::new(src, 9, 18, code, ada)).unwrap();
    let heading = FormatRun { start: 0, end: 8, kind: RunKind::Heading, level: Some(1), page: None };
    let out = p.revise_source(src, &file("report.docx", text, vec![heading], vec![], "h1")).unwrap();
    assert_eq!((out.codings_moved, out.codings_removed, out.units_added), (0, 0, 0));
    assert_eq!(out.text_revision, 2, "the revision still counts, so a view of the old one knows it is out of date");
    assert_eq!(p.source_detail(src).unwrap().formatting_runs.as_array().unwrap().len(), 1);
    assert_eq!(p.list_codings(src).unwrap().len(), 1);
}

#[test]
fn a_revision_with_no_text_or_for_no_document_is_refused() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = p.add_source(&NewSource::new("Doc", SourceType::Document, "Some words.")).unwrap();
    assert!(p.revise_source(src, &file("empty.txt", "", vec![], vec![], "e")).is_err());
    assert!(p.revise_source(9999, &file("x.txt", "Other words.", vec![], vec![], "x")).is_err());
    assert_eq!(p.source_detail(src).unwrap().text, "Some words.", "a refusal changes nothing");
}
