//! From a file's bytes to units stored in a project: the whole path an import takes.

use prismqda_engine::{NewProject, OpenOptions, Project, SourceType, UnitKind};
use prismqda_import::{import_table, open_bytes, ImportOptions, Opened, SurveyMapping};
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn batch(name: &str, bytes: &[u8], opts: &ImportOptions) -> prismqda_engine::ImportBatch {
    match open_bytes(name, bytes, opts).unwrap() {
        Opened::Batch(b) => b,
        Opened::Table(_) => panic!("expected a batch"),
    }
}

fn words(p: &Project, source_id: i64) -> Vec<String> {
    let text = p.source_detail(source_id).unwrap().text;
    p.list_units(source_id)
        .unwrap()
        .iter()
        .map(|u| text.chars().skip(u.start as usize).take((u.end - u.start) as usize).collect())
        .collect()
}

#[test]
fn a_text_file_becomes_sentence_units() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let b = batch("notes.txt", "It rained all day. We stayed in!\n\nDr. Lee called. Nobody answered.".as_bytes(), &ImportOptions::default());
    let out = p.apply_import(&b).unwrap();
    let id = out.source_ids[0];
    assert_eq!(words(&p, id), ["It rained all day.", "We stayed in!", "Dr. Lee called.", "Nobody answered."]);
    let row = &p.list_sources().unwrap()[0];
    assert_eq!((row.name.as_str(), row.source_type.as_str(), row.default_unit_kind.as_str(), row.units), ("notes", "document", "sentence", 4));
}

#[test]
fn the_unit_kind_can_be_changed_at_import() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let opts = ImportOptions { unit_kind: Some(UnitKind::Paragraph), name: Some("Field notes".into()), folder: "Week 1".into(), ..Default::default() };
    let out = p.apply_import(&batch("a.md", b"# Day one\n\nWe met. We talked.\n\n- first\n- second\n", &opts)).unwrap();
    assert_eq!(words(&p, out.source_ids[0]), ["Day one", "We met. We talked.", "first", "second"]);
    let row = &p.list_sources().unwrap()[0];
    assert_eq!((row.name.as_str(), row.folder.as_str()), ("Field notes", "Week 1"));
    let detail = p.source_detail(out.source_ids[0]).unwrap();
    assert!(detail.formatting_runs.to_string().contains("\"heading\""));
}

#[test]
fn html_keeps_headings_in_the_stored_runs() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let out = p.apply_import(&batch("page.html", b"<h1>Title</h1><p>Body text. More text.</p>", &ImportOptions::default())).unwrap();
    assert_eq!(words(&p, out.source_ids[0]), ["Title", "Body text.", "More text."]);
}

#[test]
fn a_transcript_gets_turns_speakers_and_excluded_interviewer_turns() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let opts = ImportOptions {
        source_type: Some(SourceType::Transcript),
        interviewer_labels: vec!["Interviewer".into()],
        ..Default::default()
    };
    let text = "Interviewer: How was it?\n\nParticipant: Good, mostly.\nA little long.\n\nInterviewer: Why?\n\nParticipant: The cost.";
    let out = p.apply_import(&batch("int1.txt", text.as_bytes(), &opts)).unwrap();
    let id = out.source_ids[0];
    assert_eq!(words(&p, id), ["How was it?", "Good, mostly.\nA little long.", "Why?", "The cost."]);
    let units = p.list_units(id).unwrap();
    assert_eq!(units.iter().map(|u| u.excluded).collect::<Vec<_>>(), [true, false, true, false]);
    assert_eq!(units[1].speaker.as_deref(), Some("Participant"));
    assert!(units.iter().all(|u| u.kind == "speaker_turn"));
    let speakers: Vec<String> =
        p.unit_attribute_values(id).unwrap().into_iter().filter(|v| v.attribute == "speaker").filter_map(|v| v.value).collect();
    assert_eq!(speakers, ["Interviewer", "Participant", "Interviewer", "Participant"]);
}

#[test]
fn captions_become_turns() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n<v Ann>Hello there.\n\n00:00:02.000 --> 00:00:03.000\n<v Bo>Hi.\n";
    let out = p.apply_import(&batch("talk.vtt", vtt.as_bytes(), &ImportOptions::default())).unwrap();
    assert_eq!(words(&p, out.source_ids[0]), ["Hello there.", "Hi."]);
    assert_eq!(p.list_sources().unwrap()[0].source_type, "transcript");
}

#[test]
fn a_survey_csv_becomes_response_sets_cases_and_attributes() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let csv = "participant_id,group,age,why they came,advice\n\
               P01,treatment,34,\"I felt welcome, right away.\",Bring a friend.\n\
               P02,control,51,The cost was the hard part.,\n\
               P03,treatment,29,,Be patient.\n";
    let table = match open_bytes("survey.csv", csv.as_bytes(), &ImportOptions::default()).unwrap() {
        Opened::Table(t) => t,
        Opened::Batch(_) => panic!("a csv is a table"),
    };
    assert_eq!(table.rows.len(), 3);
    let preview = table.preview(3);
    assert_eq!(preview.columns[0].suggested_role, "id");

    let mut mapping = SurveyMapping::from_guess(&table);
    assert_eq!(mapping.id_column.as_deref(), Some("participant_id"));
    // The guess finds the long answers; the person adds the short "advice" column in the wizard.
    assert_eq!(mapping.text_columns, ["why they came"].map(String::from));
    mapping.text_columns.push("advice".to_string());
    mapping.attribute_columns.retain(|a| a.name != "advice");

    let out = p.apply_import(&import_table(&table, &mapping, &ImportOptions::default()).unwrap()).unwrap();
    assert_eq!(out.source_ids.len(), 2);
    assert_eq!(out.case_ids.len(), 3);
    let sources = p.list_sources().unwrap();
    assert_eq!(sources[0].name, "survey - why they came");
    assert_eq!((sources[0].units, sources[1].units), (2, 2));
    assert_eq!(words(&p, out.source_ids[0]), ["I felt welcome, right away.", "The cost was the hard part."]);
    let cases = p.list_cases().unwrap();
    assert_eq!(cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["P01", "P02", "P03"]);
    assert_eq!(cases.iter().map(|c| c.units).collect::<Vec<_>>(), [2, 1, 1]);
    let age: Vec<(String, String)> = p
        .case_attribute_values()
        .unwrap()
        .into_iter()
        .filter(|v| v.attribute == "age")
        .map(|v| (v.level, v.value.unwrap()))
        .collect();
    assert_eq!(age.len(), 3);
    assert_eq!(age[0].1, "34");
}

#[test]
fn files_that_are_not_what_their_name_says_are_refused_plainly() {
    let o = ImportOptions::default();
    let err = |name: &str, bytes: &[u8]| open_bytes(name, bytes, &o).unwrap_err().to_string();
    assert!(err("a.docx", b"just text").contains("not"));
    assert!(err("a.pdf", b"just text").contains("not"));
    assert!(err("a.xlsx", b"just text").contains("not"));
    assert!(err("a.unknown", b"x").contains("not a kind of file"));
    assert!(err("noextension", b"x").contains("not a kind of file"));
    assert!(err("a.txt", b"   \n").contains("no text"));
    assert!(err("a.json", b"{not json").contains("not valid JSON"));
}

#[test]
fn a_file_name_with_folders_and_dots_still_gives_a_clean_source_name() {
    let o = ImportOptions::default();
    let b = batch("C:\\study\\week.1\\interview.final.txt", b"One. Two.", &o);
    assert_eq!(b.sources[0].name, "interview.final");
    let b = batch("/home/x/notes.TXT", b"One. Two.", &o);
    assert_eq!(b.sources[0].name, "notes");
}
