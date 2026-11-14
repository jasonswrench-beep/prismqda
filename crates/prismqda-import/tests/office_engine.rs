//! The importers through the public entry point (`open_bytes`) and into the engine: a batch built from
//! a DOCX, ODT or RTF file must be accepted by `Project::apply_import`.

mod common;

use common::*;
use prismqda_engine::{ImportBatch, NewProject, OpenOptions, Project, RunKind, SourceType, UnitKind};
use prismqda_import::{open_bytes, ImportError, ImportOptions, Opened};
use tempfile::TempDir;

fn batch(file_name: &str, bytes: &[u8], opts: &ImportOptions) -> ImportBatch {
    match open_bytes(file_name, bytes, opts).unwrap() {
        Opened::Batch(b) => b,
        Opened::Table(_) => panic!("a document is not a table"),
    }
}

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("Office import"), &o).unwrap()
}

const RTF_FIXTURE: &[u8] = include_bytes!("fixtures/interview_excerpt.rtf");

#[test]
fn each_format_goes_through_open_bytes_to_a_batch_the_engine_accepts() {
    let docx_body = format!(
        "{}{}{}",
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Interview notes</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t xml:space="preserve">She said it was </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>very hard</w:t></w:r><w:r><w:t xml:space="preserve"> at first. Then it got easier.</w:t></w:r></w:p>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Day</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Shift</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
    let odt_body = concat!(
        r#"<text:h text:outline-level="1">Interview notes</text:h>"#,
        r#"<text:p>She said it was <text:span text:style-name="Bold">very hard</text:span> at first. Then it got easier.</text:p>"#,
        r#"<table:table><table:table-row><table:table-cell><text:p>Day</text:p></table:table-cell><table:table-cell><text:p>Shift</text:p></table:table-cell></table:table-row></table:table>"#,
    );
    let bold_style = r#"<style:style style:name="Bold" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style>"#;
    let docx_styles = r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style>"#;
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("interview.docx", docx_with_styles(&docx_body, docx_styles)),
        ("interview.odt", odt(bold_style, odt_body)),
        ("interview.rtf", RTF_FIXTURE.to_vec()),
    ];

    let dir = TempDir::new().unwrap();
    let mut project = project(&dir);
    for (name, bytes) in &files {
        let batch = batch(name, bytes, &ImportOptions::default());
        assert_eq!(batch.sources.len(), 1, "{name}");
        let source = &batch.sources[0];
        assert_eq!(source.source_type, SourceType::Document, "{name}");
        assert_eq!(source.default_unit_kind, UnitKind::Sentence, "{name}");
        assert_eq!(source.name, "interview", "the name is the file name without its extension");
        assert!(source.text.starts_with("Interview"), "{name}: {}", source.text);
        assert!(!source.units.is_empty(), "the caller segments the text into sentences");
        assert!(source.runs.iter().any(|r| r.kind == RunKind::Heading), "{name}");
        assert!(source.runs.iter().any(|r| r.kind == RunKind::Bold), "{name}");
        assert!(source.runs.iter().any(|r| r.kind == RunKind::TableCell), "{name}");
        // Units are sentences inside the text, in order.
        let chars = source.text.chars().count() as i64;
        for u in &source.units {
            assert!(0 <= u.start && u.start < u.end && u.end <= chars, "{name}: unit {u:?}");
        }

        let outcome =
            project.apply_import(&batch).unwrap_or_else(|e| panic!("{name}: the engine refused the batch: {e}"));
        assert_eq!(outcome.source_ids.len(), 1, "{name}");
        assert_eq!(outcome.units_created, source.units.len(), "{name}");
    }
}

#[test]
fn the_rtf_fixture_reads_the_way_a_person_would_read_it() {
    let b = batch("interview_excerpt.rtf", RTF_FIXTURE, &ImportOptions::default());
    let source = &b.sources[0];
    let text = &source.text;
    assert!(text.starts_with("Interview with a community nurse\n\nBackground\n\nInterviewer: How long"), "{text}");
    assert!(text.contains("caf\u{e9} downstairs"), "{text}");
    assert!(text.contains("ni\u{f1}o was small"), "{text}");
    assert!(text.contains("Visits to clients at home\n\nPaperwork in the evening"), "{text}");
    assert!(!text.contains('\u{b7}'), "list labels are not text");
    assert!(text.contains("Day\tShift\nMonday\tEarly"), "{text}");
    assert!(text.ends_with("worth it \u{1F600} and thanked the group."), "{text}");
    let covers = |kind: RunKind| -> Vec<String> {
        source
            .runs
            .iter()
            .filter(|r| r.kind == kind)
            .map(|r| text.chars().skip(r.start as usize).take((r.end - r.start) as usize).collect())
            .collect()
    };
    assert_eq!(covers(RunKind::Heading), vec!["Interview with a community nurse", "Background", "Daily tasks"]);
    assert_eq!(covers(RunKind::Bold), vec!["Nurse:", "very hard", "Day", "Shift"]);
    assert_eq!(covers(RunKind::Italic), vec!["moved"]);
    assert_eq!(covers(RunKind::ListItem), vec!["Visits to clients at home", "Paperwork in the evening"]);
    assert_eq!(covers(RunKind::TableCell), vec!["Day", "Shift", "Monday", "Early"]);
    assert!(source.warnings.is_empty(), "{:?}", source.warnings);
}

#[test]
fn warnings_reach_the_batch_and_the_engine_stores_them() {
    let body = r#"<w:p><w:r><w:t>Before</w:t></w:r><w:r><w:drawing><wp:inline><a:graphic><pic:pic/></a:graphic></wp:inline></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p>"#;
    let b = batch("pictures.docx", &docx(body), &ImportOptions::default());
    assert_eq!(b.sources[0].text, "Before[image]After");
    assert_eq!(b.sources[0].warnings.len(), 1);
    assert_eq!(b.sources[0].warnings[0].kind, "skipped_image");

    let dir = TempDir::new().unwrap();
    let mut project = project(&dir);
    project.apply_import(&b).unwrap();
}

#[test]
fn import_options_apply_to_documents() {
    let opts = ImportOptions {
        name: Some("Nurse interview".into()),
        folder: "Interviews".into(),
        unit_kind: Some(UnitKind::Paragraph),
        ..ImportOptions::default()
    };
    let b = batch("whatever.docx", &docx(&format!("{}{}", p("One. Two."), p("Three."))), &opts);
    let s = &b.sources[0];
    assert_eq!(
        (s.name.as_str(), s.folder.as_str(), s.default_unit_kind),
        ("Nurse interview", "Interviews", UnitKind::Paragraph)
    );
    assert_eq!(s.units.len(), 2, "paragraph units");
}

#[test]
fn a_file_whose_contents_do_not_match_its_name_is_refused_before_parsing() {
    let rtf_bytes = RTF_FIXTURE;
    let docx_bytes = docx(&p("text"));
    for (name, bytes) in
        [("x.docx", rtf_bytes), ("x.odt", rtf_bytes), ("x.rtf", &docx_bytes[..]), ("x.docx", b"plain text")]
    {
        match open_bytes(name, bytes, &ImportOptions::default()) {
            Err(ImportError::Format(m)) => assert!(m.contains("contents are not"), "{name}: {m}"),
            other => panic!("{name}: expected a format error, got {other:?}"),
        }
    }
}

#[test]
fn documents_with_no_text_are_refused_with_a_clear_message() {
    for (name, bytes) in [
        ("empty.docx", docx("<w:p/>")),
        ("empty.odt", odt("", "<text:p/>")),
        ("empty.rtf", br"{\rtf1\ansi\pard\par}".to_vec()),
    ] {
        match open_bytes(name, &bytes, &ImportOptions::default()) {
            Err(ImportError::Format(m)) => assert!(m.contains("no text"), "{name}: {m}"),
            other => panic!("{name}: expected a format error, got {other:?}"),
        }
    }
}
