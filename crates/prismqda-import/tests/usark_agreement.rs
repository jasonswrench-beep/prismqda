//! A real-study agreement check, run only when `PRISMQDA_USARK_DIR` points at the study's folder.
//!
//! The same YouTube comments were coded by two models: one coded 4,908 of them for the thesis, and a second
//! model coded a larger batch for comparison. This test imports both sets of codings as two model coders and
//! runs PrismQDA's agreement report on the units both coded. It then recomputes percent agreement and Cohen's
//! kappa from the stored codings with separate, plain arithmetic and requires the report to match.
//!
//! Nothing from the folder is copied anywhere: the second model's files are read from their archive in memory,
//! and the project is built in a temporary folder that is deleted when the test ends. The report is printed so a
//! person can read it (run with `--nocapture`); it holds code names and counts only.
//!
//! Without the variable the test does nothing, so it never fails on another computer.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;

use prismqda_engine::{AgreementRequest, Level, NewProject, OpenOptions, Project};
use prismqda_import::{
    import_codings, import_table, open_bytes, AttributeColumn, Cell, CoderSpec, CodingMapping, Column, ImportOptions, Opened,
    SurveyMapping, Table,
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

/// `UOC123-4`: three capital letters, digits, a dash, digits.
fn looks_like_unit_id(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 6
        && b[..3].iter().all(u8::is_ascii_uppercase)
        && s[3..].split_once('-').is_some_and(|(a, c)| !a.is_empty() && !c.is_empty() && a.bytes().all(|x| x.is_ascii_digit()) && c.bytes().all(|x| x.is_ascii_digit()))
}

/// The second model's codes, one line per unit: `id | Code | Confidence | note`. Returns the table and how many
/// unit identifiers appeared more than once (the later line is kept).
fn second_model_table(archive_path: &PathBuf) -> (Table, usize) {
    let bytes = fs::read(archive_path).expect("the second model's archive");
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("a zip archive");
    let mut names: Vec<String> = (0..zip.len()).filter_map(|i| zip.by_index(i).ok().and_then(|f| f.name().ok().map(|n| n.to_string()))).filter(|n| n.contains("model_b-batch-") && n.ends_with(".txt")).collect();
    names.sort();
    assert!(names.len() > 100, "expected hundreds of batch files, found {}", names.len());

    let mut latest: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut duplicates = 0;
    for name in names {
        let mut text = String::new();
        zip.by_name(&name).unwrap().read_to_string(&mut text).unwrap_or_else(|e| panic!("{name}: {e}"));
        for line in text.lines() {
            let parts: Vec<&str> = line.split('|').map(str::trim).collect();
            if parts.len() < 3 || !looks_like_unit_id(parts[0]) || parts[1].is_empty() {
                continue;
            }
            if latest.insert(parts[0].to_string(), (parts[1].to_string(), parts[2].to_string())).is_some() {
                duplicates += 1;
            }
        }
    }
    let columns = ["unit_id", "code", "coder", "confidence"].map(Column::new).to_vec();
    let rows = latest
        .into_iter()
        .map(|(id, (code, confidence))| vec![Cell::Text(id), Cell::Text(code), Cell::Text("GPT".to_string()), Cell::Text(confidence)])
        .collect();
    (Table { name: "second model".to_string(), columns, rows, warnings: Vec::new() }, duplicates)
}

#[test]
fn two_models_coded_the_same_comments_and_the_report_matches_plain_arithmetic() {
    let Some(base) = folder() else {
        eprintln!("PRISMQDA_USARK_DIR is not set: skipping the real-study agreement check");
        return;
    };
    let units_table = table(&base.join("USARK Study").join("units-all.csv"));
    let first = table(&base.join("Final Data").join("coded-CLAUDE-full.csv"));
    let (second, duplicates) = second_model_table(&base.join("Final Data").join("Data for Andrew.zip"));

    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("check");
    o.backup_on_open = false;
    let mut project = Project::create(dir.path().join("usark.pqda"), &NewProject::new("Agreement check"), &o).unwrap();
    let mapping = SurveyMapping {
        id_column: Some("case_id".into()),
        unit_id_column: Some("id".into()),
        text_columns: vec!["content".into()],
        unit_attribute_columns: vec![AttributeColumn { name: "speaker".into(), level: Some(Level::Nominal) }],
        ..Default::default()
    };
    project.apply_import(&import_table(&units_table, &mapping, &ImportOptions::default()).unwrap()).unwrap();

    let mut cm = CodingMapping { unit_id_column: "unit_id".into(), code_column: "code".into(), coder_column: Some("coder".into()), rationale_column: Some("note".into()), confidence_column: Some("confidence".into()), ..Default::default() };
    cm.coders.insert("CLAUDE".into(), CoderSpec { model: true, provider: Some("Provider A".into()), model_name: Some("Model A".into()), model_version: None });
    let r1 = import_codings(&mut project, &first, &cm).unwrap();
    assert_eq!((r1.codings, r1.unmatched_rows, r1.codes_created), (4908, 0, 12));

    let mut cm2 = CodingMapping { unit_id_column: "unit_id".into(), code_column: "code".into(), coder_column: Some("coder".into()), confidence_column: Some("confidence".into()), ..Default::default() };
    cm2.coders.insert("GPT".into(), CoderSpec { model: true, provider: Some("Provider B".into()), model_name: Some("Model B".into()), model_version: None });
    let r2 = import_codings(&mut project, &second, &cm2).unwrap();
    eprintln!("second model: {} codings stored, {} lines named a unit outside these comments, {duplicates} repeated unit ids", r2.codings, r2.unmatched_rows);
    assert_eq!(r2.codes_created, 0, "both models used the same 12 codes");

    // Plain arithmetic from the stored codings.
    let source = project.list_sources().unwrap()[0].id;
    let names: HashMap<i64, String> = project.list_codes().unwrap().into_iter().map(|c| (c.id, c.name)).collect();
    let coders = project.list_coders().unwrap();
    let model_a = coders.iter().find(|c| c.name == "CLAUDE").unwrap().id;
    let model_b = coders.iter().find(|c| c.name == "GPT").unwrap().id;
    let mut by_unit: HashMap<i64, [Option<String>; 2]> = HashMap::new();
    for c in project.list_codings(source).unwrap() {
        let slot = if c.coder_id == model_a { 0 } else { 1 };
        let entry = by_unit.entry(c.unit_id.unwrap()).or_default();
        assert!(entry[slot].is_none(), "each model gave each unit one code");
        entry[slot] = Some(names[&c.code_id].clone());
    }
    let both: Vec<(&String, &String)> = by_unit.values().filter_map(|p| Some((p[0].as_ref()?, p[1].as_ref()?))).collect();
    let n = both.len() as f64;
    let agree = both.iter().filter(|(a, b)| a == b).count() as f64;
    let mut marginal: [HashMap<&String, f64>; 2] = Default::default();
    for (a, b) in &both {
        *marginal[0].entry(a).or_default() += 1.0;
        *marginal[1].entry(b).or_default() += 1.0;
    }
    let chance: f64 = marginal[0].iter().map(|(k, c)| c / n * marginal[1].get(k).copied().unwrap_or(0.0) / n).sum();
    let (p_o, kappa) = (agree / n, (agree / n - chance) / (1.0 - chance));

    let mut req: AgreementRequest = serde_json::from_value(serde_json::json!({ "coder_ids": [model_a, model_b], "bootstrap_replications": 1000 })).unwrap();
    req.save = false;
    let report = project.agreement_run(&req).unwrap();
    eprintln!("\n{}", report.report_text);

    assert_eq!(report.units_in_sample, both.len());
    assert_eq!(both.len(), 4901, "the two models share 4,901 of the thesis's 4,908 units");
    let o = report.overall.as_ref().unwrap();
    assert!((o.percent_agreement.unwrap() - p_o).abs() < 1e-12, "{:?} vs {p_o}", o.percent_agreement);
    assert!((o.cohen_kappa.unwrap() - kappa).abs() < 1e-12, "{:?} vs {kappa}", o.cohen_kappa);
    assert_eq!(report.reporting_term, "concordance", "two models are compared, so this is concordance");
    assert!(report.notes.iter().any(|n| n.starts_with("Limitation: 2 of the coders are models")));
    let table = report.confusion.as_ref().unwrap();
    assert_eq!(table.iter().flatten().sum::<usize>(), both.len());
    assert_eq!((0..table.len()).map(|i| table[i][i]).sum::<usize>() as f64, agree);

    // Each code on its own, for the same units, in the study's own terms: both applied / either applied.
    let mut each = req.clone();
    each.mode = "per_code".to_string();
    let per_code = project.agreement_run(&each).unwrap();
    eprintln!("\n{}", per_code.report_text);
    assert_eq!(per_code.by_code.len(), 12);
}
