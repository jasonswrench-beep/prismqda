//! The data editor's later tools: the parts of a date, a random sample of cases, a summary of each group given back to its cases, the order of
//! the cases, tables with a row for each group, a table stood on end or laid across, and variables merged in from a file by a key.
//!
//!   case:   P1  P2  P3  P4  P5  P6  P7  P8
//!   Site:    N   N   N   N   S   S   S   S    (P8 has no Site)
//!   Score:  10  12  14  16  20  22  24  26    (P4 has none)
//!   Seen:   2024-03-14, 2024-12-30, 2021-01-03, 2020-12-31, 2024-03-14 09:30, (none), "14 March 2024", 2022-01-01
//!   T1,T2,T3: 1 2 3 / 4 5 6 / 7 - 9 / ... (the waves, for standing on end)
//!   Key:    k1 .. k8

use prismqda_engine::{
    long_to_wide, wide_to_long, AppliesTo, AttributeSpec, BootstrapOptions, CaseMember, CellChange, DataTable, DatePart, GroupFunction, GroupTable, ImportBatch, ImportedCase, ImportedSource, Level, LongToWide,
    MergeColumn, MergeSpec, NewProject, OpenOptions, Procedure, Project, SampleSize, SortKey, SourceType, StatOptions, StatRequest, Summary, TransformKind, TransformSpec, UnitKind, VariableSpec, WideToLong,
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

const SITE: [&str; 8] = ["N", "N", "N", "N", "S", "S", "S", ""];
const SCORE: [&str; 8] = ["10", "12", "14", "", "20", "22", "24", "26"];
const SEEN: [&str; 8] = ["2024-03-14", "2024-12-30", "2021-01-03", "2020-12-31", "2024-03-14 09:30", "", "14 March 2024", "2022-01-01"];
const T1: [&str; 8] = ["1", "4", "7", "10", "13", "16", "19", "22"];
const T2: [&str; 8] = ["2", "5", "", "11", "14", "17", "20", "23"];
const T3: [&str; 8] = ["3", "6", "9", "12", "15", "18", "21", "24"];

fn world() -> (Project, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let batch = ImportBatch {
        sources: (0..8).map(|i| source(&format!("doc{i}"))).collect(),
        cases: (0..8)
            .map(|i| {
                let mut attrs = vec![("Key".to_string(), format!("k{}", i + 1)), ("T1".to_string(), T1[i].to_string()), ("T3".to_string(), T3[i].to_string())];
                for (name, v) in [("Site", SITE[i]), ("Score", SCORE[i]), ("Seen", SEEN[i]), ("T2", T2[i])] {
                    if !v.is_empty() {
                        attrs.push((name.to_string(), v.to_string()));
                    }
                }
                ImportedCase { name: format!("P{}", i + 1), attrs, members: vec![CaseMember { source: i, unit: None }] }
            })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![
            AttributeSpec::new("Key", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Site", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Score", AppliesTo::Case, Level::Ratio),
            AttributeSpec::new("Seen", AppliesTo::Case, Level::Date),
            AttributeSpec::new("T1", AppliesTo::Case, Level::Ratio),
            AttributeSpec::new("T2", AppliesTo::Case, Level::Ratio),
            AttributeSpec::new("T3", AppliesTo::Case, Level::Ratio),
        ],
    };
    p.apply_import(&batch).unwrap();
    (p, dir)
}

fn column(p: &Project, name: &str) -> Vec<Option<String>> {
    let d = p.case_data().unwrap();
    let i = d.variables.iter().position(|v| v.name == name).unwrap_or_else(|| panic!("no variable {name}"));
    d.values.iter().map(|r| r[i].clone()).collect()
}

fn s(v: &[&str]) -> Vec<Option<String>> {
    v.iter().map(|x| if x.is_empty() { None } else { Some((*x).to_string()) }).collect()
}

fn make(kind: TransformKind, target: &str) -> TransformSpec {
    TransformSpec { target: target.to_string(), replace: false, level: None, kind }
}

fn part(part: DatePart) -> TransformKind {
    TransformKind::DatePart { source: "Seen".to_string(), part }
}

// ---------------------------------------------------------------- the parts of a date

#[test]
fn the_parts_of_a_date_are_made_as_variables_with_their_levels_and_words() {
    let (mut p, _d) = world();
    let year = p.transform(&make(part(DatePart::Year), "Seen year")).unwrap();
    assert_eq!((year.level.as_str(), year.cases_with_value, year.cases_missing), ("interval", 6, 2), "the case with no date and the one written another way have none");
    assert_eq!(column(&p, "Seen year"), s(&["2024", "2024", "2021", "2020", "2024", "", "", "2022"]));
    p.transform(&make(part(DatePart::Month), "Seen month")).unwrap();
    assert_eq!(column(&p, "Seen month"), s(&["3", "12", "1", "12", "3", "", "", "1"]));
    let month = p.case_data().unwrap().variables.into_iter().find(|v| v.name == "Seen month").unwrap();
    assert_eq!(month.level, "ordinal");
    assert_eq!(month.value_labels["3"], "March", "a month carries its name");
    p.transform(&make(part(DatePart::Weekday), "Seen weekday")).unwrap();
    assert_eq!(column(&p, "Seen weekday"), s(&["4", "1", "7", "4", "4", "", "", "6"]));
    p.transform(&make(part(DatePart::Week), "Seen week")).unwrap();
    assert_eq!(column(&p, "Seen week"), s(&["11", "1", "53", "53", "11", "", "", "52"]), "the ISO week, which crosses the new year");
    p.transform(&make(part(DatePart::Quarter), "Seen quarter")).unwrap();
    assert_eq!(column(&p, "Seen quarter"), s(&["1", "4", "1", "4", "1", "", "", "1"]));
    p.transform(&make(part(DatePart::DayOfYear), "Seen doy")).unwrap();
    assert_eq!(column(&p, "Seen doy"), s(&["74", "365", "3", "366", "74", "", "", "1"]));
    p.transform(&make(part(DatePart::Day), "Seen day")).unwrap();
    assert_eq!(column(&p, "Seen day"), s(&["14", "30", "3", "31", "14", "", "", "1"]));
    let hour = p.transform(&make(part(DatePart::Hour), "Seen hour")).unwrap();
    assert_eq!(column(&p, "Seen hour"), s(&["", "", "", "", "9", "", "", ""]), "only a date written with a time has an hour");
    assert_eq!(hour.cases_with_value, 1);
}

#[test]
fn a_part_can_only_be_taken_from_a_variable_of_dates_and_can_be_made_again() {
    let (mut p, _d) = world();
    let e = p.transform(&make(TransformKind::DatePart { source: "Score".to_string(), part: DatePart::Year }, "X")).unwrap_err().to_string();
    assert!(e.contains("'Score' is a variable of numbers and not of dates"), "{e}");
    let e = p.transform(&make(TransformKind::DatePart { source: "Site".to_string(), part: DatePart::Year }, "X")).unwrap_err().to_string();
    assert!(e.contains("categories"), "{e}");
    assert!(p.transform(&make(TransformKind::DatePart { source: "Nope".to_string(), part: DatePart::Year }, "X")).unwrap_err().to_string().contains("no case variable"));
    p.transform(&make(part(DatePart::Year), "Year")).unwrap();
    // Change a date; making the variable again follows it.
    let ids: Vec<i64> = p.case_data().unwrap().cases.iter().map(|c| c.id).collect();
    p.set_values("Seen", &[prismqda_engine::CellChange { case_id: ids[0], value: Some("1999-05-01".to_string()) }]).unwrap();
    p.recompute_variable("Year").unwrap();
    assert_eq!(column(&p, "Year")[0].as_deref(), Some("1999"));
    // One Undo takes the making of the variable back whole.
    p.undo().unwrap().unwrap();
    assert_eq!(column(&p, "Year")[0].as_deref(), Some("2024"));
}

// ---------------------------------------------------------------- a random sample

#[test]
fn a_random_sample_is_a_variable_of_one_and_zero_that_the_seed_fixes() {
    let (mut p, _d) = world();
    let kind = |seed| TransformKind::Sample { size: SampleSize::Count(3), seed, within: None };
    let out = p.transform(&make(kind(5), "Pick")).unwrap();
    assert_eq!((out.level.as_str(), out.cases_with_value, out.cases_missing), ("nominal", 8, 0));
    let pick = column(&p, "Pick");
    assert_eq!(pick.iter().filter(|v| v.as_deref() == Some("1")).count(), 3, "three cases are in the sample");
    assert_eq!(pick.iter().filter(|v| v.as_deref() == Some("0")).count(), 5);
    let def = p.case_data().unwrap().variables.into_iter().find(|v| v.name == "Pick").unwrap();
    assert_eq!((def.value_labels["1"].as_str(), def.value_labels["0"].as_str()), ("In the sample", "Not in the sample"));
    // The same seed gives the same sample, however it is made.
    p.transform(&make(kind(5), "Pick again")).unwrap();
    assert_eq!(column(&p, "Pick again"), pick);
    p.recompute_variable("Pick").unwrap();
    assert_eq!(column(&p, "Pick"), pick, "making it again draws the same cases");
    p.transform(&make(kind(6), "Pick other")).unwrap();
    assert_ne!(column(&p, "Pick other"), pick, "another seed draws other cases");
    let e = p.transform(&make(TransformKind::Sample { size: SampleSize::Count(9), seed: 1, within: None }, "Too many")).unwrap_err().to_string();
    assert!(e.contains("cannot be drawn from 8"), "{e}");
}

#[test]
fn a_sample_drawn_within_groups_takes_its_share_of_each_and_leaves_out_a_case_with_no_group() {
    let (mut p, _d) = world();
    p.transform(&make(TransformKind::Sample { size: SampleSize::Percent(50.0), seed: 3, within: Some("Site".to_string()) }, "Half")).unwrap();
    let half = column(&p, "Half");
    let in_group = |range: std::ops::Range<usize>| half[range].iter().filter(|v| v.as_deref() == Some("1")).count();
    assert_eq!((in_group(0..4), in_group(4..7)), (2, 2), "half of the four in N, and half of the three in S rounded to two");
    assert_eq!(half[7], None, "a case with no site is not eligible, so it has no value");
}

// ---------------------------------------------------------------- the summary of a group

#[test]
fn a_summary_of_each_group_is_given_to_the_cases_of_the_group() {
    let (mut p, _d) = world();
    let agg = |function| TransformKind::Aggregate { source: "Score".to_string(), by: vec!["Site".to_string()], function };
    let mean = p.transform(&make(agg(GroupFunction::Mean), "Site mean")).unwrap();
    assert_eq!((mean.level.as_str(), mean.cases_with_value, mean.cases_missing), ("interval", 7, 1), "the case with no site is in no group");
    assert_eq!(column(&p, "Site mean"), s(&["12", "12", "12", "12", "22", "22", "22", ""]), "N has 10, 12, 14 and a missing score; S has 20, 22, 24");
    p.transform(&make(agg(GroupFunction::Sum), "Site sum")).unwrap();
    assert_eq!(column(&p, "Site sum"), s(&["36", "36", "36", "36", "66", "66", "66", ""]));
    p.transform(&make(agg(GroupFunction::Count), "Site count")).unwrap();
    assert_eq!(column(&p, "Site count"), s(&["3", "3", "3", "3", "3", "3", "3", ""]), "how many of the group have a score");
    p.transform(&make(agg(GroupFunction::Cases), "Site cases")).unwrap();
    assert_eq!(column(&p, "Site cases"), s(&["4", "4", "4", "4", "3", "3", "3", ""]), "how many cases are in the group");
    let sd = p.transform(&make(agg(GroupFunction::Sd), "Site sd")).unwrap();
    assert_eq!(sd.cases_with_value, 7);
    assert_eq!(column(&p, "Site sd")[0].as_deref(), Some("2"));
    p.transform(&make(agg(GroupFunction::Median), "Site median")).unwrap();
    assert_eq!(column(&p, "Site median")[4].as_deref(), Some("22"));
    // Two variables make the groups: a case needs a value of both.
    p.transform(&make(TransformKind::Aggregate { source: "Score".to_string(), by: vec!["Site".to_string(), "T3".to_string()], function: GroupFunction::Mean }, "Pair mean")).unwrap();
    assert_eq!(column(&p, "Pair mean")[0].as_deref(), Some("10"), "every case has its own T3, so each group is one case");
}

#[test]
fn a_summary_needs_groups_and_numbers_and_says_so() {
    let (mut p, _d) = world();
    let e = p.transform(&make(TransformKind::Aggregate { source: "Score".to_string(), by: vec![], function: GroupFunction::Mean }, "X")).unwrap_err().to_string();
    assert!(e.contains("at least one variable that makes the groups"), "{e}");
    let e = p.transform(&make(TransformKind::Aggregate { source: "Seen".to_string(), by: vec!["Site".to_string()], function: GroupFunction::Mean }, "X")).unwrap_err().to_string();
    assert!(e.contains("dates") && e.contains("needs numbers"), "{e}");
    // A count works on anything: how many of the group have a date.
    p.transform(&make(TransformKind::Aggregate { source: "Seen".to_string(), by: vec!["Site".to_string()], function: GroupFunction::Count }, "Dated")).unwrap();
    assert_eq!(column(&p, "Dated")[0].as_deref(), Some("4"));
    assert_eq!(column(&p, "Dated")[4].as_deref(), Some("2"), "S has two values (P5's date, and P7's, written in words, which still counts as a value); P6 has none");
}

// ---------------------------------------------------------------- the order of the cases

#[test]
fn the_cases_are_put_in_order_by_the_keys_and_nothing_is_moved() {
    let (p, _d) = world();
    let names = |ids: Vec<i64>| -> Vec<String> {
        let d = p.case_data().unwrap();
        ids.iter().map(|id| d.cases.iter().find(|c| c.id == *id).unwrap().name.clone()).collect()
    };
    let key = |v: &str, descending| SortKey { variable: v.to_string(), descending };
    assert_eq!(names(p.sorted_case_ids(&[key("Score", true)]).unwrap()), ["P8", "P7", "P6", "P5", "P3", "P2", "P1", "P4"], "highest first, the case with no score last");
    assert_eq!(names(p.sorted_case_ids(&[key("Score", false)]).unwrap()), ["P1", "P2", "P3", "P5", "P6", "P7", "P8", "P4"]);
    assert_eq!(names(p.sorted_case_ids(&[key("Site", true), key("Score", false)]).unwrap()), ["P5", "P6", "P7", "P1", "P2", "P3", "P4", "P8"], "S before N going down, the score within each, and no site last");
    assert_eq!(names(p.sorted_case_ids(&[key("Seen", false)]).unwrap()), ["P4", "P3", "P8", "P1", "P5", "P2", "P6", "P7"], "dates by the day; the one with a time follows the same day, and the two that are not read as dates are last");
    assert_eq!(names(p.sorted_case_ids(&[key("Case", true)]).unwrap()), ["P8", "P7", "P6", "P5", "P4", "P3", "P2", "P1"], "by the name of the case");
    assert!(p.sorted_case_ids(&[key("Nope", false)]).unwrap_err().to_string().contains("no case variable"));
    assert_eq!(names(p.sorted_case_ids(&[]).unwrap()), ["P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8"], "no key, no change");
    assert_eq!(p.events(100).unwrap().iter().filter(|e| e.action == "variable.apply").count(), 0, "sorting writes nothing to the project");
}

// ---------------------------------------------------------------- a table with a row for each group

#[test]
fn a_table_with_a_row_for_each_group_has_the_summaries_and_says_how_many_cases_were_left_out() {
    let (p, _d) = world();
    let spec = GroupTable {
        by: vec!["Site".to_string()],
        summaries: vec![Summary { source: "Score".to_string(), function: GroupFunction::Mean }, Summary { source: "Score".to_string(), function: GroupFunction::Count }, Summary { source: "Score".to_string(), function: GroupFunction::Max }],
    };
    let (table, left_out) = p.group_table(&spec).unwrap();
    assert_eq!(table.columns, ["Site", "Cases", "mean of Score", "count of Score", "max of Score"]);
    assert_eq!(table.rows, vec![s(&["N", "4", "12", "3", "14"]), s(&["S", "3", "22", "3", "24"])]);
    assert_eq!(left_out, 1, "P8 has no site");
    // Two variables, numbers in order by size.
    let (two, _) = p.group_table(&GroupTable { by: vec!["Site".to_string(), "T1".to_string()], summaries: vec![] }).unwrap();
    assert_eq!(two.rows.len(), 7);
    assert_eq!(two.rows[0], s(&["N", "1", "1"]), "each group is one case here, and the groups are in order");
    let e = p.group_table(&GroupTable { by: vec![], summaries: vec![] }).unwrap_err().to_string();
    assert!(e.contains("at least one variable"), "{e}");
    let e = p.group_table(&GroupTable { by: vec!["Site".to_string()], summaries: vec![Summary { source: "Seen".to_string(), function: GroupFunction::Mean }] }).unwrap_err().to_string();
    assert!(e.contains("needs numbers"), "{e}");
    let twice = GroupTable { by: vec!["Site".to_string()], summaries: vec![Summary { source: "Score".to_string(), function: GroupFunction::Mean }, Summary { source: "Score".to_string(), function: GroupFunction::Mean }] };
    assert!(p.group_table(&twice).unwrap_err().to_string().contains("asked for twice"));
}

// ---------------------------------------------------------------- standing a table on end and laying it across

#[test]
fn the_cases_stood_on_end_are_a_row_for_each_wave_of_each_case_and_laid_across_again_they_are_the_same() {
    let (p, _d) = world();
    let cases = p.case_table().unwrap();
    assert_eq!(cases.columns[0], "Case");
    let spec = WideToLong { keep: vec!["Case".to_string(), "Site".to_string()], measures: vec!["T1".to_string(), "T2".to_string(), "T3".to_string()], index_name: "Wave".to_string(), value_name: "Score".to_string(), labels: vec![], drop_missing: false };
    let long = wide_to_long(&cases, &spec).unwrap();
    assert_eq!(long.columns, ["Case", "Site", "Wave", "Score"]);
    assert_eq!(long.rows.len(), 24, "eight cases, three waves");
    assert_eq!(long.rows[0], s(&["P1", "N", "T1", "1"]));
    assert_eq!(long.rows[5], s(&["P2", "N", "T3", "6"]));
    assert_eq!(long.rows[7], s(&["P3", "N", "T2", ""]), "P3 has no second wave, and the row is there with no value");
    let dropped = wide_to_long(&cases, &WideToLong { drop_missing: true, ..spec.clone() }).unwrap();
    assert_eq!(dropped.rows.len(), 23, "a row with no value can be left out");
    let named = wide_to_long(&cases, &WideToLong { labels: vec!["Before".into(), "During".into(), "After".into()], ..spec.clone() }).unwrap();
    assert_eq!(named.rows[1][2].as_deref(), Some("During"));

    // And across again: a row for each case and a column for each wave, the same values as the cases began with.
    let wide = long_to_wide(&long, &LongToWide { id: vec!["Case".to_string(), "Site".to_string()], index: "Wave".to_string(), values: vec!["Score".to_string()] }).unwrap();
    assert_eq!(wide.columns, ["Case", "Site", "T1", "T2", "T3"]);
    assert_eq!(wide.rows.len(), 8);
    assert_eq!(wide.rows[2], s(&["P3", "N", "7", "", "9"]));
    for (i, row) in wide.rows.iter().enumerate() {
        assert_eq!(row[2].as_deref(), Some(T1[i]));
        assert_eq!(row[4].as_deref(), Some(T3[i]));
    }
}

#[test]
fn standing_a_table_on_end_or_laying_it_across_refuses_what_it_cannot_do_and_says_why() {
    let (p, _d) = world();
    let cases = p.case_table().unwrap();
    let base = WideToLong { keep: vec!["Case".to_string()], measures: vec!["T1".to_string(), "T2".to_string()], index_name: "Wave".to_string(), value_name: "Value".to_string(), labels: vec![], drop_missing: false };
    let err = |t: &DataTable, s: &WideToLong| wide_to_long(t, s).unwrap_err().to_string();
    assert!(err(&cases, &WideToLong { measures: vec!["T1".into()], ..base.clone() }).contains("at least two columns"));
    assert!(err(&cases, &WideToLong { measures: vec!["T1".into(), "Nope".into()], ..base.clone() }).contains("no column named 'Nope'"));
    assert!(err(&cases, &WideToLong { keep: vec!["T1".into()], ..base.clone() }).contains("both kept on every row and stood on end"));
    assert!(err(&cases, &WideToLong { index_name: "Value".into(), ..base.clone() }).contains("names that differ"));
    assert!(err(&cases, &WideToLong { index_name: "Site".into(), keep: vec!["Site".into()], ..base.clone() }).contains("names of columns of the table already"));
    assert!(err(&cases, &WideToLong { labels: vec!["a".into()], ..base.clone() }).contains("a word for each column"));

    // Two rows for the same case and wave cannot be laid across.
    let long = DataTable {
        columns: vec!["Who".into(), "Wave".into(), "Score".into()],
        rows: vec![s(&["a", "1", "5"]), s(&["a", "2", "6"]), s(&["a", "1", "7"])],
    };
    let e = long_to_wide(&long, &LongToWide { id: vec!["Who".into()], index: "Wave".into(), values: vec!["Score".into()] }).unwrap_err().to_string();
    assert!(e.contains("a has two rows for 1") && e.contains("rows 1 and 3"), "{e}");
    // Several columns of values are laid across with their names.
    let two = DataTable { columns: vec!["Who".into(), "Wave".into(), "Score".into(), "Mood".into()], rows: vec![s(&["a", "1", "5", "up"]), s(&["a", "2", "6", "down"]), s(&["b", "1", "7", ""])] };
    let wide = long_to_wide(&two, &LongToWide { id: vec!["Who".into()], index: "Wave".into(), values: vec!["Score".into(), "Mood".into()] }).unwrap();
    assert_eq!(wide.columns, ["Who", "Score 1", "Mood 1", "Score 2", "Mood 2"]);
    assert_eq!(wide.rows, vec![s(&["a", "5", "up", "6", "down"]), s(&["b", "7", "", "", ""])], "a case with no row for a wave has no value there");
    assert!(long_to_wide(&two, &LongToWide { id: vec![], index: "Wave".into(), values: vec!["Score".into()] }).is_err());
    assert!(long_to_wide(&two, &LongToWide { id: vec!["Who".into()], index: "Who".into(), values: vec!["Score".into()] }).unwrap_err().to_string().contains("only be one of"));
}

// ---------------------------------------------------------------- merging variables from a file

fn merge(keys: &[&str], columns: Vec<(&str, &str, &[&str])>) -> MergeSpec {
    MergeSpec {
        keys: s(keys),
        match_on: None,
        columns: columns.into_iter().map(|(name, level, values)| MergeColumn { name: name.to_string(), level: level.to_string(), values: s(values) }).collect(),
        overwrite: false,
    }
}

#[test]
fn variables_are_merged_in_by_the_name_of_the_case_and_a_case_with_no_row_has_no_value() {
    let (mut p, _d) = world();
    let spec = merge(&["p1", " P2 ", "P3", "P9", "P2", "", "P5"], vec![("Age", "ratio", &["31", "40", "52", "99", "41", "50", "29"]), ("Mood", "nominal", &["up", "", "down", "up", "x", "y", "up"])]);
    let out = p.merge_variables(&spec).unwrap();
    assert_eq!((out.cases_matched, out.cases_unmatched), (4, 4), "p1, P2, P3 and P5 match, without regard to capitals or spaces");
    assert_eq!((out.rows_unmatched, out.unmatched_keys.clone()), (1, vec!["P9".to_string()]));
    assert_eq!((out.rows_without_key, out.rows_repeated), (1, 1), "a row with no key, and a second row for P2 that is not used");
    assert_eq!(out.variables_made, ["Age", "Mood"]);
    assert_eq!(column(&p, "Age"), s(&["31", "40", "52", "", "29", "", "", ""]), "the first row for P2 is used");
    assert_eq!(column(&p, "Mood"), s(&["up", "", "down", "", "up", "", "", ""]));
    assert_eq!(out.unmatched_case_names, ["P4", "P6", "P7", "P8"]);
    let age = p.case_data().unwrap().variables.into_iter().find(|v| v.name == "Age").unwrap();
    assert_eq!(age.level, "ratio");
    // Each variable is a step in the history that Undo takes back.
    let before = p.case_data().unwrap().variables.len();
    p.undo().unwrap().unwrap();
    p.undo().unwrap().unwrap();
    assert_eq!(p.case_data().unwrap().variables.len(), before - 1, "Mood's values and then Mood itself are gone after two steps back");
}

#[test]
fn variables_can_be_merged_by_the_values_of_a_variable_and_an_existing_one_can_be_filled_in() {
    let (mut p, _d) = world();
    let mut spec = merge(&["k8", "k1", "k3"], vec![("Score", "ratio", &["100", "101", "102"])]);
    spec.match_on = Some("Key".to_string());
    let e = p.merge_variables(&spec).unwrap_err().to_string();
    assert!(e.contains("already a variable named 'Score'"), "{e}");
    spec.overwrite = true;
    let out = p.merge_variables(&spec).unwrap();
    assert_eq!((out.cases_matched, out.variables_filled.clone(), out.variables_made.len()), (3, vec!["Score".to_string()], 0));
    assert_eq!(column(&p, "Score"), s(&["101", "12", "102", "", "20", "22", "24", "100"]), "the cases the file has are filled in; the others keep what they had");
}

#[test]
fn a_merge_that_would_match_nothing_or_bring_what_cannot_be_kept_changes_nothing_and_says_why() {
    let (mut p, _d) = world();
    let variables = p.case_data().unwrap().variables.len();
    let e = p.merge_variables(&merge(&["x", "y"], vec![("Age", "ratio", &["1", "2"])])).unwrap_err().to_string();
    assert!(e.contains("none of the 2 rows of the file matches a case") && e.contains("names of the cases"), "{e}");
    let e = p.merge_variables(&merge(&["P1", "P2"], vec![("Age", "ratio", &["1", "old"])])).unwrap_err().to_string();
    assert!(e.contains("not numbers (old)"), "{e}");
    let e = p.merge_variables(&merge(&["P1", "P2"], vec![("When", "date", &["2024-01-02", "March"])])).unwrap_err().to_string();
    assert!(e.contains("not dates written year-month-day (March)"), "{e}");
    assert!(p.merge_variables(&merge(&["P1"], vec![])).unwrap_err().to_string().contains("at least one column"));
    let mut short = merge(&["P1", "P2"], vec![("Age", "ratio", &["1", "2"])]);
    short.columns[0].values.pop();
    assert!(p.merge_variables(&short).unwrap_err().to_string().contains("does not have a value for each row"));
    assert_eq!(p.case_data().unwrap().variables.len(), variables, "nothing was made");
    let mut bad_match = merge(&["P1"], vec![("Age", "ratio", &["1"])]);
    bad_match.match_on = Some("Nope".to_string());
    assert!(p.merge_variables(&bad_match).unwrap_err().to_string().contains("no case variable"));
}

// ---------------------------------------------------------------- counting cases by a weight

/// Gives every case a weight: P1 2, P2 1, P3 0, P4 none, P5 3, P6 1, P7 2, P8 1.
fn weigh(p: &mut Project, name: &str, weights: [&str; 8]) {
    p.define_variable(&VariableSpec { name: name.to_string(), level: "ratio".to_string(), value_labels: Default::default(), missing_codes: Vec::new() }, None).unwrap();
    let ids: Vec<i64> = p.case_data().unwrap().cases.iter().map(|c| c.id).collect();
    let changes: Vec<CellChange> = ids.iter().zip(weights).map(|(id, w)| CellChange { case_id: *id, value: if w.is_empty() { None } else { Some(w.to_string()) } }).collect();
    p.set_values(name, &changes).unwrap();
}

fn run(p: &Project, procedure: Procedure, variables: &[&str], options: StatOptions) -> Result<prismqda_engine::StatResult, prismqda_engine::Error> {
    p.run_statistic(&StatRequest { procedure, variables: variables.iter().map(|v| (*v).to_string()).collect(), options })
}

#[test]
fn a_weighted_analysis_counts_each_case_as_many_times_as_its_weight_says() {
    let (mut p, _d) = world();
    weigh(&mut p, "W", ["2", "1", "0", "", "3", "1", "2", "1"]);
    let weighted = StatOptions { weight: Some("W".to_string()), ..StatOptions::default() };
    let r = run(&p, Procedure::Describe, &["attr:Score"], weighted.clone()).unwrap();
    let w = r.weight.clone().expect("the result says it was weighted");
    assert_eq!((w.variable.as_str(), w.cases, w.counted, w.left_out), ("W", 8, 10, 2), "P3 has 0 and P4 none: eight cases counted as ten");
    // The scores with their weights: 10 x2, 12, 20 x3, 22, 24 x2, 26 (P4 has no score), so ten scores in all with the mean 18.8.
    assert_eq!(r.n_used, 10);
    let mean = r.details["descriptives"]["mean"].as_f64().unwrap();
    assert!((mean - 18.8).abs() < 1e-12, "{mean}");
    // An analysis that is not weighted does not say so.
    assert!(run(&p, Procedure::Describe, &["attr:Score"], StatOptions::default()).unwrap().weight.is_none());
}

#[test]
fn a_weighted_analysis_is_exactly_the_analysis_of_the_cases_written_out_that_many_times() {
    // Project A holds the eight cases with their weights; project B holds the cases written out: P1 twice, P2, P5 three times, P6, P7 twice, P8.
    let (mut a, _da) = world();
    weigh(&mut a, "W", ["2", "1", "0", "", "3", "1", "2", "1"]);
    let (b, _db) = {
        let dir = TempDir::new().unwrap();
        let mut o = OpenOptions::new("tester");
        o.backup_on_open = false;
        let mut p = Project::create(dir.path().join("b.pqda"), &NewProject::new("B"), &o).unwrap();
        let rows: Vec<usize> = vec![0, 0, 1, 4, 4, 4, 5, 6, 6, 7];
        let batch = ImportBatch {
            sources: (0..rows.len()).map(|i| source(&format!("doc{i}"))).collect(),
            cases: rows
                .iter()
                .enumerate()
                .map(|(n, &i)| {
                    let mut attrs = vec![("T1".to_string(), T1[i].to_string())];
                    for (name, v) in [("Site", SITE[i]), ("Score", SCORE[i])] {
                        if !v.is_empty() {
                            attrs.push((name.to_string(), v.to_string()));
                        }
                    }
                    ImportedCase { name: format!("row{n}"), attrs, members: vec![CaseMember { source: n, unit: None }] }
                })
                .collect(),
            images: Vec::new(),
            pdfs: Vec::new(),
            case_attributes: vec![AttributeSpec::new("Site", AppliesTo::Case, Level::Nominal), AttributeSpec::new("Score", AppliesTo::Case, Level::Ratio), AttributeSpec::new("T1", AppliesTo::Case, Level::Ratio)],
        };
        p.apply_import(&batch).unwrap();
        (p, dir)
    };
    let weighted = StatOptions { weight: Some("W".to_string()), ..StatOptions::default() };
    for (procedure, variables) in [
        (Procedure::Describe, vec!["attr:Score"]),
        (Procedure::IndependentT, vec!["attr:Score", "attr:Site"]),
        (Procedure::Pearson, vec!["attr:Score", "attr:T1"]),
        (Procedure::MannWhitney, vec!["attr:Score", "attr:Site"]),
    ] {
        let x = run(&a, procedure, &variables, weighted.clone()).unwrap_or_else(|e| panic!("{procedure:?}: {e}"));
        let y = run(&b, procedure, &variables, StatOptions::default()).unwrap_or_else(|e| panic!("{procedure:?}: {e}"));
        assert_eq!(x.details, y.details, "{procedure:?}: the numbers are the same");
        assert_eq!((x.n_used, x.cases_total), (y.n_used, y.cases_total), "{procedure:?}");
        assert_eq!(x.groups.iter().map(|g| (g.value.clone(), g.n, g.mean)).collect::<Vec<_>>(), y.groups.iter().map(|g| (g.value.clone(), g.n, g.mean)).collect::<Vec<_>>(), "{procedure:?}: the groups");
    }
}

#[test]
fn a_weight_must_be_a_whole_number_from_zero_up_and_says_which_case_is_wrong() {
    let (mut p, _d) = world();
    weigh(&mut p, "W", ["2", "2.5", "1", "1", "1", "1", "1", "1"]);
    let with = |w: &str| StatOptions { weight: Some(w.to_string()), ..StatOptions::default() };
    let e = run(&p, Procedure::Describe, &["attr:Score"], with("W")).unwrap_err().to_string();
    assert!(e.contains("P2 has the weight 2.5") && e.contains("whole number"), "{e}");
    weigh(&mut p, "Neg", ["1", "-1", "1", "1", "1", "1", "1", "1"]);
    assert!(run(&p, Procedure::Describe, &["attr:Score"], with("Neg")).unwrap_err().to_string().contains("P2 has the weight -1"));
    weigh(&mut p, "Zero", ["0", "0", "", "0", "0", "0", "0", "0"]);
    assert!(run(&p, Procedure::Describe, &["attr:Score"], with("Zero")).unwrap_err().to_string().contains("no case has a weight of 1 or more"));
    let e = run(&p, Procedure::Describe, &["attr:Score"], with("Site")).unwrap_err().to_string();
    assert!(e.contains("not a variable of numbers"), "{e}");
    let e = run(&p, Procedure::Describe, &["attr:Score"], with("Nope")).unwrap_err().to_string();
    assert!(e.contains("no case variable is named 'Nope'"), "{e}");
    // A weight and a bootstrap cannot be combined: the bootstrap resamples cases, and a weight counts a case several times.
    weigh(&mut p, "Ones", ["1"; 8]);
    let both = StatOptions { weight: Some("Ones".to_string()), bootstrap: Some(BootstrapOptions::default()), ..StatOptions::default() };
    assert!(run(&p, Procedure::Describe, &["attr:Score"], both).unwrap_err().to_string().contains("cannot be combined"));
    // A weight of one for every case changes nothing, but the result still says it was weighted.
    let ones = run(&p, Procedure::Describe, &["attr:Score"], with("Ones")).unwrap();
    assert_eq!(ones.weight.unwrap().counted, 8);
    assert_eq!(ones.details, run(&p, Procedure::Describe, &["attr:Score"], StatOptions::default()).unwrap().details);
}

#[test]
fn a_weight_and_a_rule_work_together_the_rule_picks_the_cases_and_the_weights_count_them() {
    let (mut p, _d) = world();
    weigh(&mut p, "W", ["2", "1", "0", "", "3", "1", "2", "1"]);
    let both = StatOptions { weight: Some("W".to_string()), filter: Some("Site == \"S\"".to_string()), ..StatOptions::default() };
    let r = run(&p, Procedure::Describe, &["attr:Score"], both).unwrap();
    assert_eq!(r.weight.unwrap().cases, 3, "the rule picks P5, P6 and P7 first");
    assert_eq!(r.n_used, 6, "3 + 1 + 2");
}

// ---------------------------------------------------------------- splitting the file by groups

#[test]
fn the_groups_an_analysis_is_split_into_come_with_a_rule_that_picks_each() {
    let (mut p, _d) = world();
    let plan = p.split_plan("Site").unwrap();
    assert_eq!(plan.groups.iter().map(|g| (g.value.as_str(), g.cases)).collect::<Vec<_>>(), [("N", 4), ("S", 3)]);
    assert_eq!(plan.left_out, 1, "P8 has no site");
    assert_eq!(plan.groups[0].rule, "`Site` == \"N\"");
    let picked = p.filter_cases(&plan.groups[1].rule).unwrap();
    assert_eq!(picked.len(), 3, "the rule picks the cases of the group");
    // A variable of numbers is split by number, in order of size, and the rule compares numbers.
    let by_number = p.split_plan("T3").unwrap();
    assert_eq!(by_number.groups.len(), 8);
    assert_eq!(by_number.groups[0].rule, "`T3` == 3");
    assert_eq!(p.filter_cases(&by_number.groups[0].rule).unwrap().len(), 1);
    // A value label is the group's label.
    p.define_variable(&VariableSpec { name: "Site".to_string(), level: "nominal".to_string(), value_labels: [("N".to_string(), "North".to_string())].into(), missing_codes: Vec::new() }, Some("Site")).unwrap();
    assert_eq!(p.split_plan("Site").unwrap().groups[0].label, "North");
    // What cannot make groups.
    assert!(p.split_plan("Seen").unwrap_err().to_string().contains("holds dates"));
    assert!(p.split_plan("Nope").unwrap_err().to_string().contains("no case variable"));
    // A value with a quotation mark and a backslash is written so the rule still picks it.
    p.define_variable(&VariableSpec { name: "Q".to_string(), level: "nominal".to_string(), value_labels: Default::default(), missing_codes: Vec::new() }, None).unwrap();
    let ids: Vec<i64> = p.case_data().unwrap().cases.iter().map(|c| c.id).collect();
    p.set_values("Q", &[CellChange { case_id: ids[0], value: Some("say \"hi\" \\ ok".to_string()) }, CellChange { case_id: ids[1], value: Some("plain".to_string()) }]).unwrap();
    let q = p.split_plan("Q").unwrap();
    let odd = q.groups.iter().find(|g| g.value.contains("hi")).unwrap();
    assert_eq!(p.filter_cases(&odd.rule).unwrap(), vec![ids[0]], "{}", odd.rule);
}
