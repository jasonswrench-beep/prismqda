//! The parts of a date and the summary of each group, compared with R 4.5.3 (`tests/reference/make_dataprep_reference.R` made the two files).
//! Every part of every date, and every summary of every group, must be what R gives, to the last digit R printed.

use prismqda_engine::{
    date_part, AppliesTo, AttributeSpec, CaseMember, DatePart, GroupFunction, ImportBatch, ImportedCase, ImportedSource, Level, NewProject, OpenOptions, Project, SourceType, TransformKind, TransformSpec, UnitKind,
};
use tempfile::TempDir;

fn reference(name: &str) -> Vec<Vec<String>> {
    let path = format!("{}/tests/reference/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    text.lines().skip(1).map(|l| l.split('\t').map(|c| c.trim().to_string()).collect()).collect()
}

#[test]
fn every_part_of_every_date_is_what_r_gives() {
    let rows = reference("dateparts.tsv");
    assert!(rows.len() >= 24, "{} dates", rows.len());
    let parts = [
        (DatePart::Year, 1),
        (DatePart::Quarter, 2),
        (DatePart::Month, 3),
        (DatePart::Week, 4),
        (DatePart::Day, 5),
        (DatePart::DayOfYear, 6),
        (DatePart::Weekday, 7),
    ];
    for row in &rows {
        for (part, column) in parts {
            let want: f64 = row[column].parse().unwrap();
            assert_eq!(date_part(&row[0], part), Some(want), "{} {part:?}", row[0]);
        }
    }
}

#[test]
fn every_summary_of_every_group_is_what_r_gives() {
    let rows = reference("groupsummary.tsv");
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let batch = ImportBatch {
        sources: (0..rows.len())
            .map(|i| ImportedSource {
                name: format!("doc{i}"),
                source_type: SourceType::Document,
                folder: String::new(),
                text: "Some words.".to_string(),
                runs: vec![],
                units: vec![],
                default_unit_kind: UnitKind::Sentence,
                unit_attributes: vec![],
                original_file_hash: None,
                settings: serde_json::Value::Null,
                warnings: vec![],
            })
            .collect(),
        cases: rows
            .iter()
            .enumerate()
            .map(|(i, r)| ImportedCase { name: format!("c{i}"), attrs: vec![("X".to_string(), r[0].clone()), ("G".to_string(), r[1].clone())], members: vec![CaseMember { source: i, unit: None }] })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![AttributeSpec::new("X", AppliesTo::Case, Level::Ratio), AttributeSpec::new("G", AppliesTo::Case, Level::Nominal)],
    };
    p.apply_import(&batch).unwrap();
    let functions = [
        (GroupFunction::Mean, 2),
        (GroupFunction::Sum, 3),
        (GroupFunction::Median, 4),
        (GroupFunction::Min, 5),
        (GroupFunction::Max, 6),
        (GroupFunction::Sd, 7),
        (GroupFunction::Count, 8),
        (GroupFunction::Cases, 8),
    ];
    for (function, column) in functions {
        let name = format!("v_{}", function.word());
        p.transform(&TransformSpec { target: name.clone(), replace: false, level: None, kind: TransformKind::Aggregate { source: "X".to_string(), by: vec!["G".to_string()], function } }).unwrap();
        let data = p.case_data().unwrap();
        let at = data.variables.iter().position(|v| v.name == name).unwrap();
        for (i, row) in rows.iter().enumerate() {
            let got: f64 = data.values[i][at].as_ref().unwrap().parse().unwrap();
            let want: f64 = row[column].parse().unwrap();
            assert!((got - want).abs() <= 1e-12 * want.abs().max(1.0), "{} for case {i}: {got} against R's {want}", function.word());
        }
    }
}
