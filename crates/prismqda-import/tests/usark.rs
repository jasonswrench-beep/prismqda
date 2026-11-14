//! A real-study check, run only when `PRISMQDA_USARK_DIR` points at the study's folder.
//!
//! The data is a completed qualitative study of YouTube comments: 4,908 units coded by one model
//! coder against a 12-code codebook. The study's own progress note reports how many units each code
//! got, separately for the two collections. PrismQDA must reproduce those numbers exactly from the
//! raw files. Nothing from the folder is copied anywhere; the test only reads it.
//!
//! Without the variable the test does nothing, so it never fails on another computer.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use prismqda_engine::{Level, NewProject, OpenOptions, Project};
use prismqda_import::{
    import_codings, import_table, open_bytes, AttributeColumn, CoderSpec, CodingMapping, ImportOptions, Opened, SurveyMapping, Table,
};
use tempfile::TempDir;

fn folder() -> Option<PathBuf> {
    std::env::var_os("PRISMQDA_USARK_DIR").map(PathBuf::from).filter(|p| p.is_dir())
}

fn table(path: &PathBuf) -> Table {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match open_bytes(&path.file_name().unwrap().to_string_lossy(), &bytes, &ImportOptions::default()).unwrap() {
        Opened::Table(t) => t,
        Opened::Batch(_) => panic!("expected a table"),
    }
}

/// The study's own numbers, from its progress note: units per code, USARK channel then cross-channel.
const USARK_CHANNEL: [(&str, usize); 12] = [
    ("NR", 929), ("Brand", 674), ("FWC", 467), ("Legis", 339), ("Mobil", 309), ("Comm", 192),
    ("Rights", 61), ("Ecol", 43), ("Regis", 36), ("Liveli", 31), ("AnimRts", 14), ("UNC", 10),
];
const CROSS_CHANNEL: [(&str, usize); 12] = [
    ("NR", 1247), ("Comm", 159), ("Brand", 121), ("Legis", 77), ("Liveli", 55), ("Ecol", 41),
    ("Regis", 35), ("UNC", 27), ("Mobil", 23), ("Rights", 11), ("AnimRts", 6), ("FWC", 1),
];

#[test]
fn the_real_study_reproduces_its_reported_code_counts() {
    let Some(base) = folder() else {
        eprintln!("PRISMQDA_USARK_DIR is not set: skipping the real-study check");
        return;
    };
    let units_table = table(&base.join("USARK Study").join("units-all.csv"));
    let coded_table = table(&base.join("Final Data").join("coded-CLAUDE-full.csv"));
    assert_eq!(units_table.rows.len(), 5320);
    assert_eq!(coded_table.rows.len(), 4908);

    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("check");
    o.backup_on_open = false;
    let mut project = Project::create(dir.path().join("usark.pqda"), &NewProject::new("Real-study check"), &o).unwrap();

    // The units: one source of comments, one case per video, each unit keeping its own identifier.
    let mapping = SurveyMapping {
        id_column: Some("case_id".into()),
        unit_id_column: Some("id".into()),
        text_columns: vec!["content".into()],
        unit_attribute_columns: vec![AttributeColumn { name: "speaker".into(), level: Some(Level::Nominal) }],
        ..Default::default()
    };
    let batch = import_table(&units_table, &mapping, &ImportOptions::default()).unwrap();
    assert_eq!(batch.sources.len(), 1);
    assert_eq!(batch.sources[0].units.len(), 5320, "every comment is a unit");
    project.apply_import(&batch).unwrap();

    // The codings, made by a model coder.
    let mut coding_mapping = CodingMapping {
        unit_id_column: "unit_id".into(),
        code_column: "code".into(),
        coder_column: Some("coder".into()),
        rationale_column: Some("note".into()),
        confidence_column: Some("confidence".into()),
        ..Default::default()
    };
    coding_mapping.coders.insert("CLAUDE".into(), CoderSpec { model: true, provider: Some("Provider A".into()), model_name: Some("Model A".into()), model_version: None });
    let report = import_codings(&mut project, &coded_table, &coding_mapping).unwrap();
    assert_eq!(report.codings, 4908);
    assert_eq!(report.unmatched_rows, 0, "every coded unit was found among the comments");
    assert_eq!(report.codes_created, 12);

    // Count each code overall and by collection, from what is stored.
    let source = project.list_sources().unwrap()[0].id;
    let (refs, duplicates) = project.unit_refs_by_external_id().unwrap();
    assert_eq!(duplicates, 0);
    let external_of: BTreeMap<i64, String> = refs.iter().map(|(k, v)| (v.unit_id, k.clone())).collect();
    let names: BTreeMap<i64, String> = project.list_codes().unwrap().into_iter().map(|c| (c.id, c.name)).collect();
    let mut by_collection: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    for c in project.list_codings(source).unwrap() {
        let external = &external_of[&c.unit_id.expect("every coding is on a unit")];
        let collection = match &external[..3] {
            "UOC" => "usark",
            "SOC" => "cross",
            other => panic!("unexpected unit id prefix {other}"),
        };
        *by_collection.entry(collection).or_default().entry(names[&c.code_id].clone()).or_default() += 1;
    }
    let expected = |list: &[(&str, usize)]| -> BTreeMap<String, usize> { list.iter().map(|(k, v)| (k.to_string(), *v)).collect() };
    assert_eq!(by_collection["usark"], expected(&USARK_CHANNEL), "USARK channel, 3,105 units");
    assert_eq!(by_collection["cross"], expected(&CROSS_CHANNEL), "cross-channel, 1,803 units");

    // The rates the study reports.
    let total: usize = by_collection.values().flat_map(|m| m.values()).sum();
    assert_eq!(total, 4908);
    let usark_total: usize = by_collection["usark"].values().sum();
    assert_eq!(usark_total, 3105);
    let nr_rate = 100.0 * by_collection["usark"]["NR"] as f64 / usark_total as f64;
    assert!((nr_rate - 29.9).abs() < 0.05, "the study reports an NR rate of 29.9% on the USARK channel, got {nr_rate:.2}");
    let both_nr = (by_collection["usark"]["NR"] + by_collection["cross"]["NR"]) as f64 / total as f64 * 100.0;
    assert!((both_nr - 44.3).abs() < 0.05, "44.3% overall, got {both_nr:.2}");
    let unc = (by_collection["usark"]["UNC"] + by_collection["cross"]["UNC"]) as f64 / total as f64 * 100.0;
    assert!((unc - 0.8).abs() < 0.05, "0.8% uncertain, got {unc:.2}");

    // The coder is recorded as a model, and the audit trail has an event for every coding.
    let coder = &project.list_coders().unwrap()[0];
    assert_eq!((coder.role.as_str(), coder.model_name.as_deref()), ("model", Some("Model A")));
    let events = project.events(1_000_000).unwrap();
    assert_eq!(events.iter().filter(|e| e.action == "coding.add").count(), 4908);
}
