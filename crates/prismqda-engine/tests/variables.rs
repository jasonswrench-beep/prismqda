//! The data editor's engine: defining variables of cases, typing values, and making new variables by a formula, a recoding, standardizing,
//! ranking, or cutting into ranges, each logged and taken back whole by one Undo. The project is the one of the statistics tests:
//!
//!   case:    0   1   2   3   4   5   6   7   8   9  10  11  12  13
//!   Group:   A   A   A   A   B   B   B   B   C   C   C   -   -   -
//!   Score:  10  12  11  13  15  17  16  18  20  22  21  99  14  "unknown"
//!   Age:    23  31 -99  45  52  38  41  29  33  ""  36  47  50  44      (-99 is the project's missing code)
//!   Site:    N   S   N   S   N   S   N   S   N   S   N   S   N   S

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, CellChange, ImportBatch, ImportedCase, ImportedSource, Level, NewProject, OpenOptions, Otherwise, Procedure, Project, RecodeRule, SourceType, StatOptions, StatRequest,
    TransformKind, TransformSpec, UnitKind, VariableSpec,
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

fn world() -> (Project, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let group = ["A", "A", "A", "A", "B", "B", "B", "B", "C", "C", "C", "", "", ""];
    let score = ["10", "12", "11", "13", "15", "17", "16", "18", "20", "22", "21", "99", "14", "unknown"];
    let age = ["23", "31", "-99", "45", "52", "38", "41", "29", "33", "", "36", "47", "50", "44"];
    let mut age_spec = AttributeSpec::new("Age", AppliesTo::Case, Level::Ratio);
    age_spec.missing_codes = vec!["-99".to_string()];
    let batch = ImportBatch {
        sources: (0..14).map(|i| source(&format!("doc{i}"))).collect(),
        cases: (0..14)
            .map(|i| {
                let mut attrs = vec![("Score".to_string(), score[i].to_string()), ("Age".to_string(), age[i].to_string()), ("Site".to_string(), if i % 2 == 0 { "N" } else { "S" }.to_string())];
                if !group[i].is_empty() {
                    attrs.push(("Group".to_string(), group[i].to_string()));
                }
                ImportedCase { name: format!("Case {i}"), attrs, members: vec![CaseMember { source: i, unit: None }] }
            })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![AttributeSpec::new("Group", AppliesTo::Case, Level::Nominal), AttributeSpec::new("Score", AppliesTo::Case, Level::Ratio), age_spec, AttributeSpec::new("Site", AppliesTo::Case, Level::Nominal)],
    };
    p.apply_import(&batch).unwrap();
    (p, dir)
}

fn spec(name: &str, level: &str) -> VariableSpec {
    VariableSpec { name: name.to_string(), level: level.to_string(), value_labels: Default::default(), missing_codes: Vec::new() }
}

/// The values of one variable, case by case.
fn column(p: &Project, name: &str) -> Vec<Option<String>> {
    let d = p.case_data().unwrap();
    let i = d.variables.iter().position(|v| v.name == name).unwrap_or_else(|| panic!("no variable {name}"));
    d.values.iter().map(|r| r[i].clone()).collect()
}

fn compute(target: &str, formula: &str) -> TransformSpec {
    TransformSpec { target: target.to_string(), replace: false, level: None, kind: TransformKind::Compute { formula: formula.to_string() } }
}

fn as_numbers(v: &[Option<String>]) -> Vec<Option<f64>> {
    v.iter().map(|x| x.as_ref().map(|s| s.parse::<f64>().unwrap())).collect()
}

#[test]
fn the_data_view_has_every_case_and_variable_with_its_values() {
    let (p, _dir) = world();
    let d = p.case_data().unwrap();
    assert_eq!((d.cases.len(), d.variables.len()), (14, 4));
    assert_eq!(d.values.len(), 14);
    let names: Vec<&str> = d.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Score", "Age", "Site", "Group"], "in the order they were made, which for an import is the order they first appear");
    let age = d.variables.iter().find(|v| v.name == "Age").unwrap();
    assert_eq!((age.level.as_str(), age.missing_codes.clone()), ("ratio", vec!["-99".to_string()]));
    assert_eq!(column(&p, "Group")[11], None, "a case with no group has no value");
    assert_eq!(column(&p, "Age")[2].as_deref(), Some("-99"), "a missing code is still shown as it was typed");
}

#[test]
fn a_variable_is_defined_renamed_and_refused_when_it_is_wrong_and_undo_takes_each_back() {
    let (mut p, _dir) = world();
    let made = p.define_variable(&VariableSpec { value_labels: [("1".to_string(), "Yes".to_string())].into(), missing_codes: vec![" 99 ".to_string(), "99".to_string()], ..spec("Trained", "nominal") }, None).unwrap();
    assert_eq!((made.name.as_str(), made.missing_codes.clone()), ("Trained", vec!["99".to_string()]));
    assert_eq!(p.case_data().unwrap().variables.len(), 5);

    let renamed = p.define_variable(&spec("Was trained", "ordinal"), Some("Trained")).unwrap();
    assert_eq!((renamed.id, renamed.level.as_str()), (made.id, "ordinal"), "the same variable, with a new name and level");
    assert!(renamed.value_labels.is_empty(), "the definition is what was given");

    assert!(p.define_variable(&spec("score", "ratio"), None).unwrap_err().to_string().contains("already a variable named 'Score'"), "names are compared without regard to capitals");
    assert!(p.define_variable(&spec("  ", "ratio"), None).unwrap_err().to_string().contains("needs a name"));
    assert!(p.define_variable(&spec("Bad`name", "ratio"), None).unwrap_err().to_string().contains("no backtick"));
    assert!(p.define_variable(&spec("X", "colour"), None).unwrap_err().to_string().contains("one of nominal"));
    let four = VariableSpec { missing_codes: vec!["1".into(), "2".into(), "3".into(), "4".into()], ..spec("X", "ratio") };
    assert!(p.define_variable(&four, None).unwrap_err().to_string().contains("up to 3"));
    assert!(p.define_variable(&spec("X", "ratio"), Some("Nothing")).unwrap_err().to_string().contains("no case variable"));
    assert_eq!(p.define_variable(&spec("Was trained", "ordinal"), Some("Was trained")).unwrap().id, made.id, "no change, no error");

    // Undo takes the rename back, then the making of the variable.
    p.undo().unwrap().unwrap();
    assert_eq!(p.case_data().unwrap().variables.last().unwrap().name, "Trained");
    p.undo().unwrap().unwrap();
    assert_eq!(p.case_data().unwrap().variables.len(), 4, "the variable is gone again");
}

#[test]
fn values_are_typed_in_cleared_and_checked_against_the_level_and_undo_puts_them_back() {
    let (mut p, _dir) = world();
    let ids: Vec<i64> = p.case_data().unwrap().cases.iter().map(|c| c.id).collect();
    let n = p.set_values("Score", &[CellChange { case_id: ids[0], value: Some(" 30 ".to_string()) }, CellChange { case_id: ids[1], value: None }]).unwrap();
    assert_eq!(n, 2);
    assert_eq!(&column(&p, "Score")[..3], &[Some("30".to_string()), None, Some("11".to_string())]);

    assert_eq!(p.set_values("Score", &[CellChange { case_id: ids[0], value: Some("30".to_string()) }]).unwrap(), 0, "the same value changes nothing");
    let e = p.set_values("Score", &[CellChange { case_id: ids[0], value: Some("high".to_string()) }]).unwrap_err().to_string();
    assert!(e.contains("takes a number") && e.contains("'high'"), "{e}");
    assert_eq!(column(&p, "Score")[0].as_deref(), Some("30"), "a refused change leaves the project as it was");
    assert!(p.set_values("Age", &[CellChange { case_id: ids[0], value: Some("-99".to_string()) }]).is_ok(), "a variable's own missing code is allowed");
    assert!(p.set_values("Score", &[CellChange { case_id: 9999, value: Some("1".to_string()) }]).is_err(), "no such case");

    p.define_variable(&spec("Joined", "date"), None).unwrap();
    assert!(p.set_values("Joined", &[CellChange { case_id: ids[0], value: Some("14 March 2024".to_string()) }]).unwrap_err().to_string().contains("year-month-day"));
    p.set_values("Joined", &[CellChange { case_id: ids[0], value: Some("2024-03-14".to_string()) }]).unwrap();

    // One Undo takes back one edit: the date, the making of the date variable, the missing code on Age, then both changes of Score together.
    p.undo().unwrap().unwrap();
    assert_eq!(column(&p, "Joined")[0], None);
    p.undo().unwrap().unwrap();
    assert!(p.case_data().unwrap().variables.iter().all(|v| v.name != "Joined"));
    p.undo().unwrap().unwrap();
    assert_eq!(column(&p, "Age")[0].as_deref(), Some("23"));
    p.undo().unwrap().unwrap();
    assert_eq!(&column(&p, "Score")[..3], &[Some("10".to_string()), Some("12".to_string()), Some("11".to_string())], "both values of that edit came back together");
}

#[test]
fn deleting_a_variable_takes_its_values_and_undo_brings_it_back_whole() {
    let (mut p, _dir) = world();
    let before = p.case_data().unwrap();
    p.delete_variable("Score").unwrap();
    assert!(p.case_data().unwrap().variables.iter().all(|v| v.name != "Score"));
    assert!(p.delete_variable("Score").unwrap_err().to_string().contains("no case variable"));
    p.undo().unwrap().unwrap();
    let after = p.case_data().unwrap();
    assert_eq!(after.variables, before.variables, "the same definition and id");
    assert_eq!(after.values, before.values, "and every value");
}

#[test]
fn a_formula_makes_a_new_variable_and_says_what_is_wrong_when_it_cannot() {
    let (mut p, _dir) = world();
    let out = p.transform(&compute("Score doubled", "Score * 2")).unwrap();
    assert_eq!((out.created, out.level.as_str()), (true, "interval"));
    assert_eq!((out.cases_with_value, out.cases_missing), (13, 1), "the case whose score is the word 'unknown' has no value");
    assert_eq!(out.preview.len(), 8);
    assert_eq!(&column(&p, "Score doubled")[..3], &[Some("20".to_string()), Some("24".to_string()), Some("22".to_string())]);
    assert_eq!(column(&p, "Score doubled")[13], None);

    // A rule gives true or false, stored as 1 and 0, and a text result makes a nominal variable.
    let flag = p.transform(&compute("Older", "Age >= 40")).unwrap();
    assert_eq!(flag.level, "nominal");
    assert_eq!(column(&p, "Older")[3].as_deref(), Some("1"));
    assert_eq!(column(&p, "Older")[2], None, "a missing age makes the answer missing, as in R");
    let word = p.transform(&compute("Band", "ifelse(Score > 15, \"high\", \"low\")")).unwrap();
    assert_eq!(word.level, "nominal");
    assert_eq!(column(&p, "Band")[5].as_deref(), Some("high"));

    // Missing codes are missing inside a formula, and a computed variable keeps how it was made.
    let derived = p.case_data().unwrap().variables.into_iter().find(|v| v.name == "Score doubled").unwrap().derived.unwrap();
    assert_eq!(derived["transform"]["formula"], "Score * 2");

    let e = |s: &str| p.transform_preview(&compute("Z", s)).unwrap_err().to_string();
    assert!(e("Scor * 2").contains("'Scor', which is not a variable") && e("Scor * 2").contains("backticks"));
    assert!(e("Score *").contains("cannot be read") && e("Score *").contains("at character"));
    assert!(e("Group * 2").contains("is text"), "text is not a number: {}", e("Group * 2"));
    let same = p.transform(&compute("score doubled", "1")).unwrap_err().to_string();
    assert!(same.contains("already a variable named 'Score doubled'") && same.contains("replace"), "{same}");
}

#[test]
fn replacing_a_variable_keeps_its_place_and_undo_restores_its_old_values_and_definition() {
    let (mut p, _dir) = world();
    p.transform(&compute("Twice", "Score * 2")).unwrap();
    let before = p.case_data().unwrap();
    let again = TransformSpec { replace: true, ..compute("Twice", "Score * 3") };
    let out = p.transform(&again).unwrap();
    assert!(!out.created);
    assert_eq!(column(&p, "Twice")[0].as_deref(), Some("30"));
    assert_eq!(p.case_data().unwrap().variables.len(), 5, "no second variable");
    p.undo().unwrap().unwrap();
    let after = p.case_data().unwrap();
    assert_eq!(after.values, before.values);
    assert_eq!(after.variables, before.variables, "including how it was made");
    p.undo().unwrap().unwrap();
    assert!(p.case_data().unwrap().variables.iter().all(|v| v.name != "Twice"), "one more Undo takes the first making back");
}

#[test]
fn a_made_variable_can_be_made_again_so_it_follows_a_change_in_its_sources() {
    let (mut p, _dir) = world();
    p.transform(&compute("Twice", "Score * 2")).unwrap();
    let ids: Vec<i64> = p.case_data().unwrap().cases.iter().map(|c| c.id).collect();
    p.set_values("Score", &[CellChange { case_id: ids[0], value: Some("50".to_string()) }]).unwrap();
    assert_eq!(column(&p, "Twice")[0].as_deref(), Some("20"), "a made variable does not change by itself");
    let out = p.recompute_variable("Twice").unwrap();
    assert!(!out.created);
    assert_eq!(column(&p, "Twice")[0].as_deref(), Some("100"));
    assert!(p.recompute_variable("Score").unwrap_err().to_string().contains("was not made by a transformation"));
}

#[test]
fn recoding_maps_values_keeps_or_drops_the_rest_and_carries_labels() {
    let (mut p, _dir) = world();
    p.define_variable(&VariableSpec { value_labels: [("N".to_string(), "North site".to_string()), ("S".to_string(), "South site".to_string())].into(), ..spec("Place", "nominal") }, None).unwrap();
    let ids: Vec<i64> = p.case_data().unwrap().cases.iter().map(|c| c.id).collect();
    p.set_values("Place", &ids.iter().enumerate().map(|(i, id)| CellChange { case_id: *id, value: Some(if i % 2 == 0 { "N" } else { "S" }.to_string()) }).collect::<Vec<_>>()).unwrap();

    let rules = vec![RecodeRule { from: vec!["A".into(), "B".into()], to: "AB".into() }];
    let keep = TransformSpec { target: "Group2".into(), replace: false, level: None, kind: TransformKind::Recode { source: "Group".into(), rules: rules.clone(), otherwise: Otherwise::Keep } };
    p.transform(&keep).unwrap();
    let g = column(&p, "Group2");
    assert_eq!((g[0].as_deref(), g[5].as_deref(), g[9].as_deref(), g[12].as_deref()), (Some("AB"), Some("AB"), Some("C"), None), "A and B become AB, C stays, and no group stays none");
    let drop = TransformSpec { target: "Group3".into(), kind: TransformKind::Recode { source: "Group".into(), rules, otherwise: Otherwise::Missing }, ..keep.clone() };
    p.transform(&drop).unwrap();
    assert_eq!(column(&p, "Group3")[9], None, "a value no rule names is missing when asked");

    let number_rule = TransformSpec { target: "Big".into(), kind: TransformKind::Recode { source: "Score".into(), rules: vec![RecodeRule { from: vec!["99.0".into()], to: "".into() }], otherwise: Otherwise::Keep }, ..keep.clone() };
    p.transform(&number_rule).unwrap();
    assert_eq!(column(&p, "Big")[11], None, "99 matches 99.0, and an empty new value makes it missing");
    assert_eq!(column(&p, "Big")[0].as_deref(), Some("10"));

    let place = TransformSpec { target: "Place2".into(), kind: TransformKind::Recode { source: "Place".into(), rules: vec![RecodeRule { from: vec!["N".into()], to: "1".into() }, RecodeRule { from: vec!["S".into()], to: "2".into() }], otherwise: Otherwise::Keep }, ..keep };
    p.transform(&place).unwrap();
    let labels = p.case_data().unwrap().variables.into_iter().find(|v| v.name == "Place2").unwrap().value_labels;
    assert_eq!((labels.get("1").map(String::as_str), labels.get("2").map(String::as_str)), (Some("North site"), Some("South site")), "a label follows the value it was given to");
}

#[test]
fn standardizing_ranking_and_cutting_make_the_variables_r_would() {
    let (mut p, _dir) = world();
    let s = |kind| TransformSpec { target: "T".into(), replace: true, level: None, kind };
    p.transform(&TransformSpec { replace: false, ..s(TransformKind::Standardize { source: "Age".into() }) }).unwrap();
    let z: Vec<f64> = as_numbers(&column(&p, "T")).into_iter().flatten().collect();
    assert_eq!(z.len(), 12, "the twelve ages that exist");
    assert!(z.iter().sum::<f64>().abs() < 1e-9, "z-scores add to zero");
    assert!(((z.iter().map(|v| v * v).sum::<f64>() / 11.0) - 1.0).abs() < 1e-9, "and have a standard deviation of one");

    p.transform(&s(TransformKind::Rank { source: "Score".into(), descending: false })).unwrap();
    let r = as_numbers(&column(&p, "T"));
    assert_eq!((r[0], r[1], r[13]), (Some(1.0), Some(3.0), None), "10 is the lowest, 12 the third; a word has no rank");
    p.transform(&s(TransformKind::Rank { source: "Score".into(), descending: true })).unwrap();
    assert_eq!(as_numbers(&column(&p, "T"))[11], Some(1.0), "99 is the highest");

    let c = p.transform(&s(TransformKind::Cut { source: "Age".into(), cuts: vec![20.0, 30.0, 40.0, 60.0], right: true })).unwrap();
    assert_eq!(c.level, "ordinal");
    assert_eq!(&column(&p, "T")[..4], &[Some("1".to_string()), Some("2".to_string()), None, Some("3".to_string())], "23 in (20, 30], 31 in (30, 40], missing stays, 45 in (40, 60]");
    let labels = p.case_data().unwrap().variables.into_iter().find(|v| v.name == "T").unwrap().value_labels;
    assert_eq!(labels.get("2").map(String::as_str), Some("(30, 40]"));

    let e = p.transform(&s(TransformKind::Standardize { source: "Group".into() })).unwrap_err().to_string();
    assert!(e.contains("fewer than two values") || e.contains("no spread") || e.contains("numbers"), "{e}");
    assert!(p.transform(&s(TransformKind::Cut { source: "Age".into(), cuts: vec![5.0, 1.0], right: true })).unwrap_err().to_string().contains("must be numbers that rise"));
}

#[test]
fn a_preview_saves_nothing() {
    let (p, _dir) = world();
    let out = p.transform_preview(&compute("Preview", "Score + Age")).unwrap();
    assert_eq!(out.preview[0].value.as_deref(), Some("33"));
    assert!(p.case_data().unwrap().variables.iter().all(|v| v.name != "Preview"));
}

#[test]
fn a_rule_picks_cases_and_must_give_true_or_false() {
    let (p, _dir) = world();
    let names = |ids: Vec<i64>| -> Vec<String> {
        let d = p.case_data().unwrap();
        ids.iter().map(|i| d.cases.iter().find(|c| c.id == *i).unwrap().name.clone()).collect()
    };
    assert_eq!(names(p.filter_cases("Group == \"B\" & Age > 40").unwrap()), ["Case 4", "Case 6"], "group B and over 40: ages 52 and 41");
    assert_eq!(names(p.filter_cases("Group == \"C\" & Age > 30").unwrap()), ["Case 8", "Case 10"], "case 9 has no age, so the rule is not true for it");
    assert!(p.filter_cases("Score * 2").unwrap_err().to_string().contains("must give true or false"));
    assert!(p.filter_cases("Nothing > 1").unwrap_err().to_string().contains("not a variable"));
}

#[test]
fn a_variable_made_in_the_editor_can_be_analyzed_and_a_rule_limits_an_analysis() {
    let (mut p, _dir) = world();
    p.transform(&compute("Score z", "(Score - 15) / 5")).unwrap();
    let vars = p.stat_variables().unwrap();
    assert!(vars.variables.iter().any(|v| v.key == "attr:Score z"), "a made variable is offered like any other");
    let run = |o: StatOptions| p.run_statistic(&StatRequest { procedure: Procedure::Describe, variables: vec!["attr:Score z".to_string()], options: o }).unwrap();
    assert_eq!(run(StatOptions::default()).n_used, 13);
    let limited = run(StatOptions { filter: Some("Group %in% c(\"A\", \"B\")".to_string()), ..Default::default() });
    assert_eq!((limited.cases_total, limited.n_used), (8, 8), "only the eight cases of groups A and B");
    let empty = p.run_statistic(&StatRequest { procedure: Procedure::Describe, variables: vec!["attr:Score z".to_string()], options: StatOptions { filter: Some("Score > 1000".to_string()), ..Default::default() } });
    assert!(empty.unwrap_err().to_string().contains("picks no cases"));
    let bad = p.run_statistic(&StatRequest { procedure: Procedure::Describe, variables: vec!["attr:Score z".to_string()], options: StatOptions { filter: Some("Score * 2".to_string()), ..Default::default() } });
    assert!(bad.unwrap_err().to_string().contains("must give true or false"));
}

#[test]
fn a_preview_shows_the_first_cases_that_get_a_value() {
    let (p, _dir) = world();
    let out = p.transform_preview(&compute("Some", "ifelse(Score >= 12, Score, NA)")).unwrap();
    assert!(out.cases_missing > 0, "the first case has no value, so the test means something");
    assert!(out.preview.iter().all(|r| r.value.is_some()), "{:?}", out.preview);
    assert_eq!(out.preview[0].value.as_deref(), Some("12"));
    // When no case gets a value, the first cases are shown, empty, so the screen still has something to say.
    let none = p.transform_preview(&compute("None", "ifelse(Score > 1000, Score, NA)")).unwrap();
    assert_eq!(none.cases_with_value, 0);
    assert_eq!(none.preview.len(), 8);
    assert!(none.preview.iter().all(|r| r.value.is_none()));
}
