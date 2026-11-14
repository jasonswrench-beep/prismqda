//! Crosstabs of two case variables, and the matrix of codes by the month codings were made in, against a small project whose
//! every number was worked out by hand.
//!
//! Six cases, one document each:
//!   case:   1     2     3     4     5     6
//!   Group:  A     A     A     B     B     (none)
//!   Site:   North South North North South South
//!   Age:    23    31    37    45    52    n/a

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, CrosstabSpec, ExploreScope, ImportBatch, ImportedCase, ImportedSource, Level, MatrixSpec, NewCode, NewCoder, NewCoding, NewProject, OpenOptions,
    Project, RowSpec, SourceType, UnitKind,
};
use serde_json::Value;
use tempfile::TempDir;

fn source(name: &str) -> ImportedSource {
    ImportedSource {
        name: name.to_string(),
        source_type: SourceType::Document,
        folder: String::new(),
        text: "Some words to code in this document.".to_string(),
        runs: vec![],
        units: vec![],
        default_unit_kind: UnitKind::Sentence,
        unit_attributes: vec![],
        original_file_hash: None,
        settings: Value::Null,
        warnings: vec![],
    }
}

/// The project, its folder, the document numbers, the code, the coder, and the project's path.
fn world() -> (Project, TempDir, Vec<i64>, i64, i64, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(&path, &NewProject::new("P"), &o).unwrap();
    let group = [Some("A"), Some("A"), Some("A"), Some("B"), Some("B"), None];
    let site = ["North", "South", "North", "North", "South", "South"];
    let age = ["23", "31", "37", "45", "52", "n/a"];
    let batch = ImportBatch {
        sources: (1..=6).map(|i| source(&format!("doc{i}"))).collect(),
        cases: (0..6)
            .map(|i| {
                let mut attrs = vec![("Site".to_string(), site[i].to_string()), ("Age".to_string(), age[i].to_string())];
                if let Some(g) = group[i] {
                    attrs.push(("Group".to_string(), g.to_string()));
                }
                ImportedCase { name: format!("Case {}", i + 1), attrs, members: vec![CaseMember { source: i, unit: None }] }
            })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![
            AttributeSpec::new("Group", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Site", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Age", AppliesTo::Case, Level::Ratio),
        ],
    };
    let out = p.apply_import(&batch).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Welcome")).unwrap();
    (p, dir, out.source_ids, code, ada, path)
}

fn spec(row: &str, column: &str) -> CrosstabSpec {
    CrosstabSpec { row_variable: row.to_string(), column_variable: column.to_string(), row_width: None, column_width: None, case_ids: None }
}

#[test]
fn a_crosstab_counts_cases_for_each_pair_of_values_and_every_case_is_counted_once() {
    let (p, _dir, _, _, _, _) = world();
    let t = p.crosstab(&spec("Group", "Site")).unwrap();
    let rows: Vec<(&str, i64)> = t.rows.values.iter().map(|v| (v.label.as_str(), v.total)).collect();
    let cols: Vec<(&str, i64)> = t.columns.values.iter().map(|v| (v.label.as_str(), v.total)).collect();
    assert_eq!(rows, [("A", 3), ("B", 2), ("(no value)", 1)], "values in order, the cases with none last");
    assert_eq!(cols, [("North", 3), ("South", 3)]);
    assert_eq!(t.cells, vec![vec![2, 1], vec![1, 1], vec![0, 1]]);
    assert_eq!(t.total, 6);
    assert_eq!(t.cells.iter().flatten().sum::<i64>(), 6, "the cells add up to the cases counted");
    assert_eq!((t.rows.level.as_str(), t.rows.variable.as_str()), ("nominal", "Group"));
    assert!(t.notes[0].contains("(no value)"));
}

#[test]
fn numbers_can_be_grouped_into_ranges_and_values_that_are_not_numbers_stay_as_they_are() {
    let (p, _dir, _, _, _, _) = world();
    let mut s = spec("Age", "Site");
    // Without grouping the six ages are six rows, in number order, and "n/a" is just another value, after the numbers.
    let plain = p.crosstab(&s).unwrap();
    assert_eq!(plain.rows.values.iter().map(|v| v.label.as_str()).collect::<Vec<_>>(), ["23", "31", "37", "45", "52", "n/a"]);
    // Ten years wide: 23 is 20 to under 30, 31 and 37 are 30 to under 40, 45 and 52 each have a range of their own.
    s.row_width = Some(10.0);
    let t = p.crosstab(&s).unwrap();
    assert_eq!(
        t.rows.values.iter().map(|v| (v.label.as_str(), v.total)).collect::<Vec<_>>(),
        [("20 to under 30", 1), ("30 to under 40", 2), ("40 to under 50", 1), ("50 to under 60", 1), ("n/a", 1)]
    );
    assert_eq!(t.cells, vec![vec![1, 0], vec![1, 1], vec![1, 0], vec![0, 1], vec![0, 1]]);
}

#[test]
fn the_cases_behind_a_cell_a_row_a_column_or_the_whole_table_can_be_listed() {
    let (p, _dir, _, _, _, _) = world();
    let s = spec("Group", "Site");
    let names = |cases: Vec<prismqda_engine::CaseRef>| cases.into_iter().map(|c| c.name).collect::<Vec<_>>();
    assert_eq!(names(p.crosstab_cases(&s, Some("A"), Some("North")).unwrap()), ["Case 1", "Case 3"]);
    assert_eq!(names(p.crosstab_cases(&s, Some("B"), None).unwrap()), ["Case 4", "Case 5"]);
    assert_eq!(names(p.crosstab_cases(&s, None, Some("South")).unwrap()), ["Case 2", "Case 5", "Case 6"]);
    assert_eq!(names(p.crosstab_cases(&s, Some(""), None).unwrap()), ["Case 6"], "the cases with no value are asked for by an empty key");
    assert_eq!(p.crosstab_cases(&s, None, None).unwrap().len(), 6);
}

#[test]
fn a_crosstab_can_be_limited_to_some_cases_and_refuses_what_it_cannot_make() {
    let (p, _dir, _, _, _, _) = world();
    let some = p.list_cases().unwrap().into_iter().filter(|c| c.name == "Case 1" || c.name == "Case 4").map(|c| c.id).collect::<Vec<_>>();
    let mut s = spec("Group", "Site");
    s.case_ids = Some(some);
    let t = p.crosstab(&s).unwrap();
    assert_eq!(t.total, 2);
    assert_eq!(t.cells, vec![vec![1], vec![1]], "cases 1 (A, North) and 4 (B, North): two rows, one column");
    assert!(p.crosstab(&spec("Nope", "Site")).unwrap_err().to_string().contains("no case variable is named 'Nope'"));
    assert!(p.crosstab(&spec("Group", "Missing")).is_err());
}

#[test]
fn a_side_with_more_than_a_hundred_values_is_refused_with_advice() {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("many.pqda"), &NewProject::new("Many"), &o).unwrap();
    let batch = ImportBatch {
        sources: (0..101).map(|i| source(&format!("d{i}"))).collect(),
        cases: (0..101).map(|i| ImportedCase { name: format!("C{i}"), attrs: vec![("Id".to_string(), format!("{i}")), ("Site".to_string(), "North".to_string())], members: vec![CaseMember { source: i, unit: None }] }).collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![AttributeSpec::new("Id", AppliesTo::Case, Level::Ratio), AttributeSpec::new("Site", AppliesTo::Case, Level::Nominal)],
    };
    p.apply_import(&batch).unwrap();
    let e = p.crosstab(&spec("Id", "Site")).unwrap_err().to_string();
    assert!(e.contains("101 different values") && e.contains("into ranges"), "{e}");
    let mut grouped = spec("Id", "Site");
    grouped.row_width = Some(25.0);
    assert_eq!(p.crosstab(&grouped).unwrap().rows.values.len(), 5, "grouped, 0 to 100 make five ranges");
}

#[test]
fn a_matrix_can_have_a_row_for_each_month_codings_were_made_in() {
    let (mut p, _dir, sources, code, ada, path) = world();
    let ids: Vec<i64> = [(0, 0), (1, 1), (2, 2)].iter().map(|(s, i)| p.apply_coding(&NewCoding::new(sources[*s], *i * 5, *i * 5 + 4, code, ada)).unwrap()).collect();
    let raw = rusqlite::Connection::open(&path).unwrap();
    for (id, day) in ids.iter().zip(["2024-01-15T08:00:00.000Z", "2024-01-20T08:00:00.000Z", "2024-03-02T08:00:00.000Z"]) {
        raw.execute("UPDATE coding SET created_at = ?1 WHERE id = ?2", rusqlite::params![day, id]).unwrap();
    }
    let spec = MatrixSpec { rows: RowSpec::MonthCoded, code_ids: vec![code], include_descendants: true, scope: ExploreScope::default() };
    let m = p.code_matrix(&spec).unwrap();
    let rows: Vec<(&str, i64, i64)> = m.rows.iter().map(|r| (r.label.as_str(), r.cells[0].assignments, r.cells[0].documents)).collect();
    assert_eq!(rows, [("2024-01", 2, 2), ("2024-02", 0, 0), ("2024-03", 1, 1)], "January twice, February nothing but still a row, March once");
    assert!(m.rows.iter().all(|r| r.key != "month:"), "a row for codings with no date is only there when something is in it");
    assert!(m.notes.iter().any(|n| n.contains("month")));
    // The evidence behind the January row is its two codings.
    let evidence = p.code_matrix_evidence(&spec, Some("month:2024-01"), None).unwrap();
    assert_eq!(evidence.len(), 2);
}
