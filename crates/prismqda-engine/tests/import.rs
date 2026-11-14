//! Batch import through the public API.

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, Error, FormatRun, ImportBatch, ImportWarning, ImportedCase, ImportedSource,
    ImportedUnit, Level, NewProject, OpenOptions, Project, RunKind, SourceType, UnitKind,
};
use rusqlite::Connection;
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn source(name: &str, text: &str) -> ImportedSource {
    ImportedSource {
        name: name.to_string(),
        source_type: SourceType::Social,
        folder: String::new(),
        text: text.to_string(),
        runs: vec![],
        units: vec![],
        default_unit_kind: UnitKind::Comment,
        unit_attributes: vec![],
        original_file_hash: None,
        settings: serde_json::Value::Null,
        warnings: vec![],
    }
}

fn raw(p: &Project) -> Connection {
    Connection::open(p.path()).unwrap()
}

fn count(p: &Project, table: &str) -> i64 {
    raw(p).query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

#[test]
fn a_thread_with_replies_attributes_and_cases_is_stored_whole() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);

    //            0         1         2
    //            0123456789012345678901234
    let text = "First post\n\nA reply\n\nAnother";
    let mut s = source("Thread 1", text);
    s.units = vec![
        ImportedUnit { attrs: vec![("author".into(), "ann".into()), ("depth".into(), "0".into())], ..ImportedUnit::new(0, 10, UnitKind::Comment) },
        ImportedUnit {
            parent: Some(0),
            speaker: Some("bob".into()),
            attrs: vec![("author".into(), "bob".into()), ("depth".into(), "1".into())],
            ..ImportedUnit::new(12, 19, UnitKind::Comment)
        },
        ImportedUnit { parent: Some(0), attrs: vec![("author".into(), "cy".into())], ..ImportedUnit::new(21, 28, UnitKind::Comment) },
    ];
    s.unit_attributes = vec![
        AttributeSpec::new("author", AppliesTo::Unit, Level::Nominal),
        AttributeSpec::new("depth", AppliesTo::Unit, Level::Ratio),
    ];
    s.runs = vec![FormatRun { start: 0, end: 10, kind: RunKind::Heading, level: Some(1), page: None }];
    s.warnings = vec![ImportWarning::new("relative_timestamps", "dates were '2 years ago'")];
    s.original_file_hash = Some("ab".repeat(32));

    let batch = ImportBatch {
        sources: vec![s],
        cases: vec![ImportedCase {
            name: "Video A".into(),
            attrs: vec![("channel".into(), "Official".into())],
            members: vec![CaseMember { source: 0, unit: None }, CaseMember { source: 0, unit: Some(1) }],
        }],
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![AttributeSpec::new("channel", AppliesTo::Case, Level::Nominal)],
    };
    let out = p.apply_import(&batch).unwrap();

    assert_eq!(out.source_ids.len(), 1);
    assert_eq!(out.units_created, 3);
    assert_eq!(out.attribute_values_created, 6); // 2 + 2 + 1 unit values, plus 1 case value
    assert_eq!(count(&p, "unit"), 3);
    assert_eq!(count(&p, "research_case"), 1);
    assert_eq!(count(&p, "case_member"), 2);

    let c = raw(&p);
    let parents: Vec<Option<i64>> = c
        .prepare("SELECT parent_unit_id FROM unit ORDER BY start_offset")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let first_id: i64 = c.query_row("SELECT id FROM unit ORDER BY start_offset LIMIT 1", [], |r| r.get(0)).unwrap();
    assert_eq!(parents, vec![None, Some(first_id), Some(first_id)]);

    let (report, runs, hash): (String, String, String) = c
        .query_row("SELECT import_report_json, formatting_runs_json, original_file_hash FROM source", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert!(report.contains("relative_timestamps"));
    assert!(runs.contains("\"heading\"") && runs.contains("\"level\":1"));
    assert_eq!(hash.len(), 64);

    // The speaker, and the attribute value, can be read back by joining.
    let author: String = c
        .query_row(
            "SELECT value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id JOIN unit u ON u.id = av.target_id
             WHERE a.name = 'author' AND u.speaker = 'bob'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(author, "bob");

    let events = p.events(10).unwrap();
    let actions: Vec<&str> = events.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(actions, ["project.create", "source.import", "case.import"]);
    assert_eq!(events[1].after.as_ref().unwrap()["units"], 3);
}

#[test]
fn a_bad_batch_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let good = source("Good", "hello world");
    let mut bad = source("Bad", "short");
    bad.units = vec![ImportedUnit::new(0, 99, UnitKind::Comment)];
    let err = p.apply_import(&ImportBatch { sources: vec![good, bad], ..Default::default() }).unwrap_err();
    assert!(matches!(err, Error::Invalid(_)), "{err}");
    assert_eq!(count(&p, "source"), 0, "the good source must not be kept when its batch-mate is bad");
    assert_eq!(p.events(10).unwrap().len(), 1, "only the project.create event");
}

#[test]
fn every_rule_an_importer_must_follow_is_enforced() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let mut try_one = |s: ImportedSource| p.apply_import(&ImportBatch { sources: vec![s], ..Default::default() }).unwrap_err().to_string();

    assert!(try_one(source("", "text")).contains("needs a name"));
    assert!(try_one(source("n", "")).contains("no text"));
    assert!(try_one(source("n", "a\0b")).contains("NUL"));
    assert!(try_one(source("n", "e\u{0301}")).contains("NFC"), "decomposed text is refused so offsets stay honest");

    let mut s = source("n", "abcdef");
    s.units = vec![ImportedUnit::new(3, 6, UnitKind::Comment), ImportedUnit::new(0, 2, UnitKind::Comment)];
    assert!(try_one(s).contains("starts before"));

    let mut s = source("n", "abcdef");
    let mut u = ImportedUnit::new(0, 2, UnitKind::Comment);
    u.parent = Some(0);
    s.units = vec![u];
    assert!(try_one(s).contains("parent"));

    let mut s = source("n", "abcdef");
    let mut u = ImportedUnit::new(0, 2, UnitKind::Comment);
    u.attrs = vec![("undefined".into(), "x".into())];
    s.units = vec![u];
    assert!(try_one(s).contains("no definition"));

    let mut s = source("n", "abcdef");
    s.runs = vec![FormatRun { start: 2, end: 99, kind: RunKind::Bold, level: None, page: None }];
    assert!(try_one(s).contains("outside the text"));

    let mut s = source("n", "abcdef");
    let mut spec = AttributeSpec::new("x", AppliesTo::Unit, Level::Nominal);
    spec.missing_codes = vec!["1".into(), "2".into(), "3".into(), "4".into()];
    s.unit_attributes = vec![spec];
    assert!(try_one(s).contains("at most 3"));
}

#[test]
fn cases_must_point_at_real_sources_and_units() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let batch = ImportBatch {
        sources: vec![source("s", "abc")],
        cases: vec![ImportedCase { name: "c".into(), attrs: vec![], members: vec![CaseMember { source: 3, unit: None }] }],
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![],
    };
    assert!(p.apply_import(&batch).unwrap_err().to_string().contains("not in the batch"));
}

#[test]
fn a_second_import_reuses_attributes_but_not_with_a_different_level() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let make = |name: &str, level: Level| {
        let mut s = source(name, "hi there");
        let mut u = ImportedUnit::new(0, 2, UnitKind::Comment);
        u.attrs = vec![("author".into(), "x".into())];
        s.units = vec![u];
        s.unit_attributes = vec![AttributeSpec::new("author", AppliesTo::Unit, level)];
        ImportBatch { sources: vec![s], ..Default::default() }
    };
    p.apply_import(&make("one", Level::Nominal)).unwrap();
    p.apply_import(&make("two", Level::Nominal)).unwrap();
    assert_eq!(count(&p, "attribute"), 1, "the attribute is shared, not duplicated");
    assert_eq!(count(&p, "attribute_value"), 2);

    let err = p.apply_import(&make("three", Level::Text)).unwrap_err();
    assert!(err.to_string().contains("already exists as nominal"), "{err}");
    assert_eq!(count(&p, "source"), 2);
}

#[test]
fn batches_survive_json_so_an_interface_can_hold_one() {
    let mut s = source("S", "text here");
    s.units = vec![ImportedUnit::new(0, 4, UnitKind::Paragraph)];
    let batch = ImportBatch { sources: vec![s], ..Default::default() };
    let json = serde_json::to_string(&batch).unwrap();
    let back: ImportBatch = serde_json::from_str(&json).unwrap();
    assert_eq!(back, batch);
    assert!(json.contains("\"paragraph\""), "enums travel as the same text the file stores: {json}");
}
