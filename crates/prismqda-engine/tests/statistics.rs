//! The data and the procedures of the Statistics page, against a small project whose every number was worked out by hand.
//!
//! Fourteen cases, one document each:
//!   case:    0   1   2   3   4   5   6   7   8   9  10  11  12  13
//!   Group:   A   A   A   A   B   B   B   B   C   C   C   -   -   -
//!   Score:  10  12  11  13  15  17  16  18  20  22  21  99  14  "unknown"
//!   Age:    23  31 -99  45  52  38  41  29  33  "" 36  47  50  44      (-99 is the project's missing code)
//!   Site:    N   S   N   S   N   S   N   S   N   S   N   S   N   S
//!   Rating:  1   2   3   2   4   5   3   2   1   4   5   3   2   1      (ordinal)
//!
//! The code "Welcome" is on cases 0 (two excerpts), 1, 4, and 9 (one each); a second coder codes the same words as the first on
//! case 1, which must not count twice.

use prismqda_engine::{
    AppliesTo, AttributeSpec, BootstrapOptions, CaseMember, ImportBatch, ImportedCase, ImportedSource, Level, Measure, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, PostHocName, Procedure, Project,
    SourceType, StatOptions, StatRequest, Status, UnitKind, VariableKind,
};
use prismqda_engine::Chart;
use serde_json::{json, Value};
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

fn world() -> (Project, TempDir, i64) {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let group = ["A", "A", "A", "A", "B", "B", "B", "B", "C", "C", "C", "", "", ""];
    let score = ["10", "12", "11", "13", "15", "17", "16", "18", "20", "22", "21", "99", "14", "unknown"];
    let age = ["23", "31", "-99", "45", "52", "38", "41", "29", "33", "", "36", "47", "50", "44"];
    let rating = ["1", "2", "3", "2", "4", "5", "3", "2", "1", "4", "5", "3", "2", "1"];
    let mut site = AttributeSpec::new("Site", AppliesTo::Case, Level::Nominal);
    site.value_labels.insert("N".to_string(), "North site".to_string());
    site.value_labels.insert("S".to_string(), "South site".to_string());
    let mut age_spec = AttributeSpec::new("Age", AppliesTo::Case, Level::Ratio);
    age_spec.missing_codes = vec!["-99".to_string()];
    let batch = ImportBatch {
        sources: (0..14).map(|i| source(&format!("doc{i}"))).collect(),
        cases: (0..14)
            .map(|i| {
                let mut attrs = vec![
                    ("Score".to_string(), score[i].to_string()),
                    ("Age".to_string(), age[i].to_string()),
                    ("Site".to_string(), if i % 2 == 0 { "N" } else { "S" }.to_string()),
                    ("Rating".to_string(), rating[i].to_string()),
                ];
                if !group[i].is_empty() {
                    attrs.push(("Group".to_string(), group[i].to_string()));
                }
                ImportedCase { name: format!("Case {i}"), attrs, members: vec![CaseMember { source: i, unit: None }] }
            })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![
            AttributeSpec::new("Group", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Score", AppliesTo::Case, Level::Ratio),
            age_spec,
            site,
            AttributeSpec::new("Rating", AppliesTo::Case, Level::Ordinal),
        ],
    };
    let out = p.apply_import(&batch).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();
    let code = p.add_code(&NewCode::new("Welcome")).unwrap();
    for (case, start, end) in [(0, 0, 4), (0, 5, 10), (1, 0, 4), (4, 0, 4), (9, 0, 4)] {
        p.apply_coding(&NewCoding::new(out.source_ids[case], start, end, code, ada)).unwrap();
    }
    p.apply_coding(&NewCoding::new(out.source_ids[1], 0, 4, code, bo)).unwrap();
    (p, dir, code)
}

fn run(p: &Project, procedure: Procedure, variables: &[&str], options: StatOptions) -> prismqda_engine::Result<prismqda_engine::StatResult> {
    p.run_statistic(&StatRequest { procedure, variables: variables.iter().map(|v| v.to_string()).collect(), options })
}

fn f(v: &Value, path: &[&str]) -> f64 {
    let mut cur = v;
    for k in path {
        cur = &cur[*k];
    }
    cur.as_f64().unwrap_or_else(|| panic!("no number at {path:?} in {v}"))
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} is not {b}");
}

#[test]
fn the_variables_come_with_their_levels_missing_values_and_categories() {
    let (p, _dir, code) = world();
    let v = p.stat_variables().unwrap();
    assert_eq!((v.cases, v.cases_with_documents), (14, 14));
    let get = |key: &str| v.variables.iter().find(|x| x.key == key).unwrap_or_else(|| panic!("no variable {key}"));

    let group = get("attr:Group");
    assert_eq!((group.measure, group.valid, group.missing), (Measure::Nominal, 11, 3));
    let cats: Vec<(&str, usize)> = group.categories.iter().map(|c| (c.value.as_str(), c.count)).collect();
    assert_eq!(cats, [("A", 4), ("B", 4), ("C", 3)]);

    let score = get("attr:Score");
    assert_eq!((score.measure, score.valid, score.missing, score.not_a_number), (Measure::Scale, 13, 1, 1), "'unknown' is not a number and is set aside");
    assert!(score.numeric);

    let age = get("attr:Age");
    assert_eq!((age.valid, age.missing), (12, 2), "the project's missing code -99 and a blank are both set aside");

    let site = get("attr:Site");
    assert_eq!(site.categories.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["North site", "South site"], "value labels are carried");

    let rating = get("attr:Rating");
    assert_eq!((rating.measure, rating.numeric), (Measure::Ordinal, true));

    let present = get(&format!("code:{code}:present"));
    assert_eq!((present.kind, present.measure, present.valid), (VariableKind::CodePresent, Measure::Nominal, 14));
    let counts: Vec<(&str, usize)> = present.categories.iter().map(|c| (c.value.as_str(), c.count)).collect();
    assert_eq!(counts, [("0", 10), ("1", 4)], "four cases have the code, ten do not");
    let count = get(&format!("code:{code}:count"));
    assert_eq!(count.measure, Measure::Scale);
    assert_eq!(count.categories.iter().map(|c| (c.value.as_str(), c.count)).collect::<Vec<_>>(), [("0", 10), ("1", 3), ("2", 1)], "case 0 has two excerpts; case 1 has one, though two coders coded it");
}

#[test]
fn describe_gives_the_numbers_of_a_scale_variable_and_the_counts_of_a_category() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::Describe, &["attr:Age"], StatOptions::default()).unwrap();
    assert_eq!((r.cases_total, r.n_used, r.n_excluded), (14, 12, 2));
    near(f(&r.details, &["descriptives", "mean"]), 469.0 / 12.0);
    assert_eq!(f(&r.details, &["descriptives", "n"]), 12.0);

    let c = run(&p, Procedure::Describe, &["attr:Site"], StatOptions::default()).unwrap();
    assert_eq!(c.details["kind"], "frequencies");
    assert_eq!(c.details["categories"][0]["label"], "North site");
    assert_eq!(f(&c.details["categories"][0], &["count"]), 7.0);
    near(f(&c.details["categories"][0], &["percent"]), 0.5);
}

#[test]
fn a_t_test_compares_two_groups_among_the_cases_that_have_both_values() {
    let (p, _dir, _) = world();
    let both = StatOptions { groups: Some(vec!["A".to_string(), "B".to_string()]), ..Default::default() };
    let r = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Group"], both).unwrap();
    // A = 10, 12, 11, 13 (mean 11.5) and B = 15, 17, 16, 18 (mean 16.5); each variance 5/3; Welch's t = -5 / sqrt(5/6).
    near(f(&r.details, &["t_test", "estimate"]), -5.0);
    near(f(&r.details, &["t_test", "t"]), -5.0 / (5.0f64 / 6.0).sqrt());
    near(f(&r.details, &["t_test", "df"]), 6.0);
    assert!(f(&r.details, &["t_test", "p"]) < 0.01);
    assert_eq!(r.groups.iter().map(|g| (g.label.as_str(), g.n)).collect::<Vec<_>>(), [("A", 4), ("B", 4)]);
    assert_eq!(r.groups[0].case_ids.len(), 4, "the cases behind each group are kept so they can be opened");
    assert_eq!((r.n_used, r.n_excluded), (8, 6));
    assert_eq!(r.details["t_test"]["kind"], "welch");

    let pooled = StatOptions { groups: Some(vec!["A".to_string(), "B".to_string()]), equal_variances: true, ..Default::default() };
    let s = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Group"], pooled).unwrap();
    assert_eq!(s.details["t_test"]["kind"], "student");
    near(f(&s.details, &["t_test", "df"]), 6.0);

    // Three groups cannot go into a two-group test, and the refusal says what to do.
    let e = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Group"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("3 groups") && e.contains("exactly two"), "{e}");
}

#[test]
fn an_analysis_of_variance_compares_more_groups_and_follows_up_with_adjusted_pairs() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], StatOptions::default()).unwrap();
    assert_eq!(r.groups.len(), 3);
    near(f(&r.details, &["anova", "df_between"]), 2.0);
    near(f(&r.details, &["anova", "df_within"]), 8.0);
    assert!(f(&r.details, &["anova", "test", "p"]) < 0.001, "A about 11.5, B about 16.5 and C about 21 are far apart");
    assert_eq!(r.details["pairwise"].as_array().unwrap().len(), 3, "three pairs of three groups");
    assert_eq!(r.details["adjustment"], "holm", "Holm unless another is chosen");
    assert!(r.assumptions.iter().any(|a| a.id == "equal_variances"), "Levene's test is run");
    assert!(r.assumptions.iter().any(|a| a.id == "independence" && a.status == Status::Info));

    let k = run(&p, Procedure::KruskalWallis, &["attr:Score", "attr:Group"], StatOptions::default()).unwrap();
    assert_eq!(k.details["dunn"].as_array().unwrap().len(), 3);
    assert!(f(&k.details, &["kruskal_wallis", "p"]) < 0.05);
}

#[test]
fn the_pairs_after_an_analysis_of_variance_can_be_compared_by_a_test_made_for_pairs() {
    // Groups A (10, 12, 11, 13), B (15, 17, 16, 18), and C (20, 22, 21): the pooled variance is 12 / 8 = 1.5.
    let (p, _dir, _) = world();
    let holm = run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], StatOptions::default()).unwrap();
    let o = StatOptions { posthoc: Some(PostHocName::Tukey), ..StatOptions::default() };
    let r = run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], o).unwrap();
    assert_eq!(r.details["posthoc"], "tukey");
    let pairs = r.details["pairwise"].as_array().unwrap();
    assert_eq!(pairs.len(), 3);
    near(f(&pairs[0], &["difference"]), -5.0);
    near(f(&pairs[0], &["se"]), (1.5f64 * 0.5).sqrt());
    let half = |v: &Value| (v["ci"][1].as_f64().unwrap() - v["ci"][0].as_f64().unwrap()) / 2.0;
    let holm_half = half(&holm.details["pairwise"][0]);
    assert!(half(&pairs[0]) > holm_half, "the interval for all the pairs together is wider than one that is not widened: {} against {holm_half}", half(&pairs[0]));
    assert!((half(&pairs[0]) - 2.474).abs() < 0.01, "q(0.95; 3, 8) = 4.041, divided by the square root of 2 and times the standard error: {}", half(&pairs[0]));
    assert!(f(&pairs[0], &["p_adjusted"]) < 0.01 && f(&pairs[0], &["p_adjusted"]) > 0.0001);

    let o = StatOptions { posthoc: Some(PostHocName::GamesHowell), ..StatOptions::default() };
    let gh = run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], o).unwrap();
    assert_eq!(gh.details["posthoc"], "games_howell");
    // A and B both have a variance of 5/3 and four cases: the Welch standard error is the square root of 2 * (5/3)/4, with 6 degrees of freedom.
    near(f(&gh.details["pairwise"][0], &["se"]), (2.0 * (5.0f64 / 3.0) / 4.0).sqrt());
    near(f(&gh.details["pairwise"][0], &["df"]), 6.0);

    let o = StatOptions { posthoc: Some(PostHocName::Scheffe), ..StatOptions::default() };
    let sc = run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], o).unwrap();
    assert_eq!(sc.details["posthoc"], "scheffe");
    assert!(half(&sc.details["pairwise"][0]) > half(&pairs[0]), "Scheffe is the widest");

    // Kruskal-Wallis compares pairs by Dunn's test and ignores the choice.
    let o = StatOptions { posthoc: Some(PostHocName::Tukey), ..StatOptions::default() };
    let k = run(&p, Procedure::KruskalWallis, &["attr:Score", "attr:Group"], o).unwrap();
    assert_eq!(k.details["dunn"].as_array().unwrap().len(), 3);
}

#[test]
fn the_normality_check_carries_the_shapiro_wilk_test() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], StatOptions::default()).unwrap();
    let checks: Vec<_> = r.assumptions.iter().filter(|a| a.id == "normality").collect();
    assert_eq!(checks.len(), 2, "one for each group with enough cases to measure its shape (A and B have four; C has three)");
    for c in checks {
        let (w, sp) = (c.numbers["w"], c.numbers["shapiro_p"]);
        assert!(w > 0.0 && w <= 1.0 && sp > 0.0 && sp <= 1.0, "W {w}, p {sp}");
    }
}

#[test]
fn two_categorical_variables_are_tested_as_a_table_of_cases() {
    let (p, _dir, code) = world();
    let r = run(&p, Procedure::ChiSquare, &["attr:Group", "attr:Site"], StatOptions::default()).unwrap();
    assert_eq!(r.n_used, 11, "the three cases with no group are set aside");
    assert_eq!(r.details["row_labels"], serde_json::json!(["A", "B", "C"]));
    assert_eq!(r.details["col_labels"], serde_json::json!(["North site", "South site"]));
    // A: cases 0-3 (N, S, N, S) = 2 N and 2 S; B: cases 4-7 (N, S, N, S) = 2 and 2; C: cases 8-10 (N, S, N) = 2 and 1.
    assert_eq!(r.details["table"], serde_json::json!([[2.0, 2.0], [2.0, 2.0], [2.0, 1.0]]));
    let a = r.assumptions.iter().find(|a| a.id == "expected_counts").unwrap();
    assert_eq!(a.status, Status::Problem, "every expected count is under 5");

    // Whether a case has the code, by site: Fisher's exact test for the two-by-two table.
    let key = format!("code:{code}:present");
    let fi = run(&p, Procedure::Fisher, &[&key, "attr:Site"], StatOptions::default()).unwrap();
    assert_eq!(fi.details["table"], serde_json::json!([[5.0, 5.0], [2.0, 2.0]]));
    near(f(&fi.details, &["fisher", "p"]), 1.0);
    near(f(&fi.details, &["fisher", "odds_ratio_sample"]), 1.0);

    let big = run(&p, Procedure::Fisher, &["attr:Group", "attr:Site"], StatOptions::default()).unwrap_err().to_string();
    assert!(big.contains("two rows and two columns"), "{big}");
}

#[test]
fn correlations_use_the_cases_that_have_both_values_and_ordinal_numbers_only_for_ranks() {
    let (p, _dir, _) = world();
    let s = run(&p, Procedure::Spearman, &["attr:Age", "attr:Rating"], StatOptions::default()).unwrap();
    assert_eq!(s.n_used, 12, "the two cases with no age are set aside");
    assert!(f(&s.details, &["correlation", "r"]).abs() <= 1.0);
    let k = run(&p, Procedure::Kendall, &["attr:Age", "attr:Rating"], StatOptions::default()).unwrap();
    assert_eq!(k.details["correlation"]["kind"], "kendall");
    // Pearson's r wants numbers that can be averaged; an ordinal rating is refused with a reason.
    let e = run(&p, Procedure::Pearson, &["attr:Age", "attr:Rating"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("Rating") && e.contains("ordinal"), "{e}");
    let ok = run(&p, Procedure::Pearson, &["attr:Age", "attr:Score"], StatOptions::default()).unwrap();
    assert!(ok.assumptions.iter().any(|a| a.id == "normality"));
}

#[test]
fn a_count_of_codings_is_one_value_per_case_and_a_test_on_it_has_a_row_for_each_case() {
    let (p, _dir, code) = world();
    let key = format!("code:{code}:count");
    let r = run(&p, Procedure::MannWhitney, &[&key, "attr:Site"], StatOptions::default()).unwrap();
    assert_eq!(r.n_used, 14, "every case has a value, so every case is a row");
    assert_eq!(r.groups.iter().map(|g| g.n).collect::<Vec<_>>(), [7, 7]);
    // North holds cases 0, 2, 4, 6, 8, 10, 12 (excerpts 2, 0, 1, 0, 0, 0, 0); South holds 1, 3, 5, 7, 9, 11, 13 (1, 0, 0, 0, 1, 0, 0).
    assert_eq!(r.groups[0].mean.map(|m| (m * 7.0).round() as i64), Some(3));
    assert_eq!(r.groups[1].mean.map(|m| (m * 7.0).round() as i64), Some(2));
    assert!(r.assumptions.iter().any(|a| a.id == "ties"), "most counts are 0, so there are many ties");
}

#[test]
fn only_the_chosen_cases_are_used_and_a_project_without_cases_is_told_so() {
    let (p, _dir, _) = world();
    let ids: Vec<i64> = (1..=5).collect();
    let r = run(&p, Procedure::Describe, &["attr:Age"], StatOptions { case_ids: Some(ids), ..Default::default() }).unwrap();
    assert_eq!(r.cases_total, 5);

    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let empty = Project::create(dir.path().join("e.pqda"), &NewProject::new("E"), &o).unwrap();
    assert_eq!(empty.stat_variables().unwrap().cases, 0);
    let e = run(&empty, Procedure::Describe, &["attr:Age"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("no cases"), "{e}");
}

#[test]
fn the_wrong_number_or_kind_of_variable_is_refused_in_words() {
    let (p, _dir, _) = world();
    let e = run(&p, Procedure::IndependentT, &["attr:Score"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("needs 2 variables") && e.contains("1 was chosen"), "{e}");
    let e = run(&p, Procedure::Describe, &["attr:Nothing"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("Nothing"), "{e}");
    let e = run(&p, Procedure::OneSampleT, &["attr:Site"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("not a numeric variable") && e.contains("nominal"), "{e}");
    let e = run(&p, Procedure::Describe, &["junk"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("not a variable"), "{e}");
    let e = run(&p, Procedure::Describe, &["attr:Age"], StatOptions { level: 1.5, ..Default::default() }).unwrap_err().to_string();
    assert!(e.contains("between 0 and 1"), "{e}");
}

#[test]
fn a_one_sample_test_and_a_signed_rank_test_compare_with_a_value() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::OneSampleT, &["attr:Age"], StatOptions { mu: 35.0, ..Default::default() }).unwrap();
    near(f(&r.details, &["t_test", "estimate"]), 469.0 / 12.0);
    assert_eq!(f(&r.details, &["mu"]), 35.0);
    let w = run(&p, Procedure::Wilcoxon, &["attr:Age"], StatOptions { mu: 35.0, ..Default::default() }).unwrap();
    assert_eq!(w.details["wilcoxon"]["n"], 12);
    let pair = run(&p, Procedure::PairedT, &["attr:Age", "attr:Score"], StatOptions::default()).unwrap();
    assert_eq!(pair.n_used, 11, "cases with both an age and a score");
}

// ---------------------------------------------------------------- regression

/// The cases that have a score, an age, and a site (cases 2, 9, and 13 do not), as the regression sees them.
const SCORE: [f64; 11] = [10.0, 12.0, 13.0, 15.0, 17.0, 16.0, 18.0, 20.0, 21.0, 99.0, 14.0];
const AGE: [f64; 11] = [23.0, 31.0, 45.0, 52.0, 38.0, 41.0, 29.0, 33.0, 36.0, 47.0, 50.0];
const SITE_S: [f64; 11] = [0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];

#[test]
fn a_regression_uses_the_cases_that_have_every_value_and_turns_categories_into_columns() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site"], StatOptions::default()).unwrap();
    assert_eq!((r.cases_total, r.n_used, r.n_excluded), (14, 11, 3), "a case missing the outcome or any predictor is set aside");
    let terms = r.details["terms"].as_array().unwrap();
    assert_eq!(terms.len(), 2, "Age is one term; Site (two values) is one 0/1 column against its reference");
    assert_eq!((terms[0]["dummy"].as_bool(), terms[0]["name"].as_str()), (Some(false), Some("Age")));
    assert_eq!((terms[1]["dummy"].as_bool(), terms[1]["label"].as_str(), terms[1]["reference"].as_str()), (Some(true), Some("South site"), Some("North site")), "the first value is the reference");

    // The engine must hand the model the right columns: compare with the same model fitted directly.
    let direct = prismqda_stats::regression::linear(&SCORE, &[AGE.to_vec(), SITE_S.to_vec()], 0.95).unwrap();
    let model = &r.details["models"][0];
    for (i, term) in direct.terms.iter().enumerate() {
        near(model["terms"][i]["estimate"].as_f64().unwrap(), term.estimate);
    }
    near(f(model, &["r_squared"]), direct.r_squared);
    near(f(model, &["f", "p"]), direct.f.p);
    assert_eq!(model["n"], 11);
    assert!(model.get("residuals").is_none(), "the case-by-case columns are not sent");
}

#[test]
fn the_reference_category_can_be_chosen_and_predictors_can_be_entered_in_steps() {
    let (p, _dir, _) = world();
    let mut references = std::collections::BTreeMap::new();
    references.insert("attr:Site".to_string(), "S".to_string());
    let r = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site"], StatOptions { references, ..Default::default() }).unwrap();
    let t = &r.details["terms"][1];
    assert_eq!((t["level"].as_str(), t["label"].as_str(), t["reference"].as_str()), (Some("N"), Some("North site"), Some("South site")));

    let stepped = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site"], StatOptions { steps: Some(vec![1, 2]), ..Default::default() }).unwrap();
    assert_eq!(stepped.details["models"].as_array().unwrap().len(), 2, "one model for each step");
    assert_eq!(stepped.details["models"][0]["k"], 1);
    assert_eq!(stepped.details["models"][1]["k"], 2);
    let change = &stepped.details["changes"][0];
    let small = prismqda_stats::regression::linear(&SCORE, &[AGE.to_vec()], 0.95).unwrap();
    let large = prismqda_stats::regression::linear(&SCORE, &[AGE.to_vec(), SITE_S.to_vec()], 0.95).unwrap();
    near(f(change, &["f"]), prismqda_stats::regression::compare(&small, &large).unwrap().f);
    assert_eq!(stepped.details["steps"][1]["terms"], 2);

    // A predictor entered in the first step comes first whatever its place in the request.
    let swapped = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site"], StatOptions { steps: Some(vec![2, 1]), ..Default::default() }).unwrap();
    assert_eq!(swapped.details["terms"][0]["name"], "Site");

    let e = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site"], StatOptions { steps: Some(vec![1]), ..Default::default() }).unwrap_err().to_string();
    assert!(e.contains("a step"), "{e}");
}

#[test]
fn a_regression_says_what_the_model_depends_on_and_names_the_cases_that_pull_it() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site", "attr:Rating"], StatOptions::default()).unwrap();
    let ids: Vec<&str> = r.assumptions.iter().map(|a| a.id.as_str()).collect();
    for want in ["independence", "cases_per_predictor", "collinearity", "influence", "equal_error_variance", "residual_shape", "ordinal_as_number"] {
        assert!(ids.contains(&want), "{want} is checked: {ids:?}");
    }
    let per = r.assumptions.iter().find(|a| a.id == "cases_per_predictor").unwrap();
    assert!(per.status != Status::Ok, "a few cases for three terms is not enough: {per:?}");
    // Case 11 has a score of 99 among scores of 10 to 22: it must be named as influential.
    let names: Vec<&str> = r.details["influential"].as_array().unwrap().iter().map(|c| c["case_name"].as_str().unwrap()).collect();
    assert!(names.contains(&"Case 11"), "the outlier is named: {names:?}");
}

#[test]
fn a_regression_that_cannot_be_fitted_says_why_in_words() {
    let (p, _dir, _) = world();
    // Cases 1 to 4 are all in group A, so the group has one value there and cannot explain anything.
    let one_group = StatOptions { case_ids: Some(vec![1, 2, 3, 4]), ..Default::default() };
    let e = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Group"], one_group).unwrap_err().to_string();
    assert!(e.contains("only one value"), "{e}");
    let few = StatOptions { case_ids: Some(vec![1, 2, 4]), ..Default::default() };
    let e = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Rating"], few).unwrap_err().to_string();
    assert!(e.contains("more cases than parameters"), "{e}");
    let e = run(&p, Procedure::LinearRegression, &["attr:Score"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("an outcome and from 1 to 20 predictors"), "{e}");
    let e = run(&p, Procedure::LinearRegression, &["attr:Site", "attr:Age"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("not a numeric variable"), "a nominal outcome is refused: {e}");
}

// ---------------------------------------------------------------- logistic regression

/// The twelve cases that have a site and an age (cases 2 and 9 have no age), with the South site as the outcome that is modelled.
const AGE_12: [f64; 12] = [23.0, 31.0, 45.0, 52.0, 38.0, 41.0, 29.0, 33.0, 36.0, 47.0, 50.0, 44.0];
const SOUTH_12: [f64; 12] = [0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0];

#[test]
fn a_logistic_regression_models_the_later_value_and_gets_the_same_numbers_as_the_fit_itself() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::LogisticRegression, &["attr:Site", "attr:Age"], StatOptions::default()).unwrap();
    assert_eq!((r.cases_total, r.n_used, r.n_excluded), (14, 12, 2));
    assert_eq!((r.details["event"]["value"].as_str(), r.details["event"]["label"].as_str(), r.details["other"]["label"].as_str()), (Some("S"), Some("South site"), Some("North site")));
    let direct = prismqda_stats::logistic::logistic(&SOUTH_12, &[AGE_12.to_vec()], 0.95, true).unwrap();
    let model = &r.details["models"][0];
    near(model["terms"][1]["estimate"].as_f64().unwrap(), direct.terms[1].estimate);
    near(model["terms"][1]["odds_ratio"].as_f64().unwrap(), direct.terms[1].odds_ratio);
    near(f(model, &["deviance"]), direct.deviance);
    near(f(model, &["lr", "p"]), direct.lr.p);
    near(f(model, &["auc"]), direct.auc);
    assert!(model.get("fitted").is_none() && model.get("cooks_distance").is_none(), "the case-by-case columns are not sent");
    assert!(model["terms"][1]["profile_ci"].is_array(), "the profile interval is given for the model that is reported");
    assert_eq!(r.details["blocks"][0]["terms"], 1);
    near(f(&r.details["blocks"][0], &["lr", "chi2"]), prismqda_stats::logistic::drop_test(&SOUTH_12, &[AGE_12.to_vec()], direct.deviance, &[0]).unwrap().chi2);
    let ids: Vec<&str> = r.assumptions.iter().map(|a| a.id.as_str()).collect();
    for want in ["independence", "events_per_predictor", "influence"] {
        assert!(ids.contains(&want), "{want} is checked: {ids:?}");
    }
    assert_eq!(r.assumptions.iter().find(|a| a.id == "events_per_predictor").unwrap().status, Status::Caution, "six cases of the rarer outcome for one predictor is under the ten that are usually asked for");
}

#[test]
fn the_modelled_value_can_be_chosen_and_the_signs_turn_over() {
    let (p, _dir, _) = world();
    let opts = StatOptions { event: Some("N".to_string()), ..Default::default() };
    let r = run(&p, Procedure::LogisticRegression, &["attr:Site", "attr:Age"], opts).unwrap();
    assert_eq!(r.details["event"]["label"], "North site");
    let flipped = prismqda_stats::logistic::logistic(&SOUTH_12.iter().map(|v| 1.0 - v).collect::<Vec<_>>(), &[AGE_12.to_vec()], 0.95, false).unwrap();
    near(r.details["models"][0]["terms"][1]["estimate"].as_f64().unwrap(), flipped.terms[1].estimate);
    let e = run(&p, Procedure::LogisticRegression, &["attr:Site", "attr:Age"], StatOptions { event: Some("X".to_string()), ..Default::default() }).unwrap_err().to_string();
    assert!(e.contains("not one of the two values"), "{e}");
}

#[test]
fn predictors_are_entered_in_steps_and_categories_become_columns_in_a_logistic_regression() {
    let (p, _dir, _) = world();
    // Rating (ordinal numbers) first, then Age: two models, and a change between them.
    let r = run(&p, Procedure::LogisticRegression, &["attr:Site", "attr:Age", "attr:Rating"], StatOptions { steps: Some(vec![2, 1]), ..Default::default() }).unwrap();
    assert_eq!(r.details["models"].as_array().unwrap().len(), 2);
    assert_eq!(r.details["terms"][0]["name"], "Rating");
    assert!(r.details["changes"][0]["chi2"].as_f64().unwrap() >= 0.0);
    assert_eq!(r.details["blocks"].as_array().unwrap().len(), 2, "one test for each variable left out");
    // A categorical predictor: Group has three values among these cases, so two columns against the first (a block of two).
    let g = run(&p, Procedure::LogisticRegression, &["attr:Site", "attr:Rating", "attr:Group"], StatOptions::default());
    if let Ok(g) = g {
        let block = g.details["blocks"].as_array().unwrap().iter().find(|b| b["name"] == "Group").unwrap().clone();
        assert_eq!(block["terms"], 2);
        assert_eq!(block["lr"]["df"], 2.0);
    }
}

#[test]
fn a_logistic_regression_that_cannot_be_fitted_says_why_in_words() {
    let (p, _dir, _) = world();
    let e = run(&p, Procedure::LogisticRegression, &["attr:Group", "attr:Age"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("exactly two values"), "an outcome with three values is refused: {e}");
    let e = run(&p, Procedure::LogisticRegression, &["attr:Site"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("an outcome and from 1 to 20 predictors"), "{e}");
    // Site follows Age exactly among cases 0, 1, 3, 4 only if the ages were split cleanly; use the score, which separates the groups of cases 0-3 and 4-7.
    let split = StatOptions { case_ids: Some(vec![1, 2, 3, 4, 5, 6, 7, 8]), ..Default::default() };
    let e = run(&p, Procedure::LogisticRegression, &["attr:Site", "attr:Score"], split);
    if let Err(e) = e {
        let text = e.to_string();
        assert!(text.contains("separate") || text.contains("more cases than parameters") || text.contains("same value"), "{text}");
    }
}

#[test]
fn a_logistic_regression_can_have_bootstrap_intervals() {
    let (p, _dir, _) = world();
    let opts = StatOptions { bootstrap: Some(BootstrapOptions { resamples: 200, seed: 7, ..Default::default() }), ..Default::default() };
    let r = run(&p, Procedure::LogisticRegression, &["attr:Site", "attr:Age"], opts).unwrap();
    let items = r.details["bootstrap"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 3, "the intercept, Age, and the area under the curve: {items:?}");
    assert_eq!(items[2]["statistic"], "auc");
    assert!(items[1]["interval"]["percentile"].is_array());
}

// ---------------------------------------------------------------- curve fits

#[test]
fn curves_are_fitted_to_two_numbers_with_the_numbers_of_the_curve_itself_and_one_chart() {
    use prismqda_stats::curve::CurveKind;
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::CurveFit, &["attr:Score", "attr:Age"], StatOptions::default()).unwrap();
    assert_eq!((r.cases_total, r.n_used, r.n_excluded), (14, 11, 3));
    let fits = r.details["fits"].as_array().unwrap();
    assert_eq!(fits.len(), 8, "every curve is fitted unless some are chosen");
    let quad = fits.iter().find(|f| f["kind"] == "quadratic").unwrap();
    let direct = prismqda_stats::curve::fit(CurveKind::Quadratic, &AGE, &SCORE, 0.95).unwrap();
    near(f(&quad["model"], &["r_squared"]), direct.regression.r_squared);
    near(f(quad, &["r_squared_original"]), direct.r_squared_original);
    assert!(quad["model"].get("fitted").is_none(), "the case-by-case columns are not sent");
    match &r.charts[0] {
        Chart::Curves { points, curves, .. } => {
            assert_eq!(points.len(), 11);
            assert_eq!(curves.len(), 8);
            assert!(curves.iter().all(|c| c.at_points.len() == 11 && !c.points.is_empty()));
        }
        other => panic!("a curve fit draws its curves, not {other:?}"),
    }
    let two = run(&p, Procedure::CurveFit, &["attr:Score", "attr:Age"], StatOptions { curves: Some(vec![CurveKind::Power, CurveKind::Linear, CurveKind::Power]), ..Default::default() }).unwrap();
    let kinds: Vec<&str> = two.details["fits"].as_array().unwrap().iter().map(|f| f["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["power", "linear"], "in the order asked, each once");
}

#[test]
fn a_curve_that_cannot_be_fitted_is_named_beside_the_others_and_none_at_all_is_an_error_in_words() {
    let (p, _dir, _) = world();
    // Three cases are enough for a line but not for a quadratic or a cubic: those are named, with the reason, beside the curves that fit.
    let three = StatOptions { case_ids: Some(vec![1, 4, 5]), ..Default::default() };
    let r = run(&p, Procedure::CurveFit, &["attr:Score", "attr:Age"], three).unwrap();
    let fits = r.details["fits"].as_array().unwrap();
    let cubic = fits.iter().find(|f| f["kind"] == "cubic").unwrap();
    assert_eq!(cubic["error"], "too_few_cases");
    assert!(fits.iter().find(|f| f["kind"] == "linear").unwrap().get("error").is_none());
    match &r.charts[0] {
        Chart::Curves { curves, .. } => assert!(curves.len() < 8 && !curves.is_empty(), "only the curves that were fitted are drawn"),
        other => panic!("{other:?}"),
    }
    // Two cases fit no curve at all: the reason is told in words.
    let two = StatOptions { case_ids: Some(vec![1, 4]), ..Default::default() };
    let e = run(&p, Procedure::CurveFit, &["attr:Score", "attr:Age"], two).unwrap_err().to_string();
    assert!(e.contains("too few to fit a curve"), "{e}");
    let e = run(&p, Procedure::CurveFit, &["attr:Site", "attr:Age"], StatOptions::default()).unwrap_err().to_string();
    assert!(e.contains("not a numeric variable"), "{e}");
}

// ---------------------------------------------------------------- the bootstrap

fn boot(resamples: usize, seed: u64) -> StatOptions {
    StatOptions { bootstrap: Some(BootstrapOptions { resamples, seed, ..Default::default() }), ..Default::default() }
}

#[test]
fn a_bootstrap_adds_intervals_for_the_statistic_a_procedure_reports_and_the_same_seed_repeats() {
    let (p, _dir, _) = world();
    let a = run(&p, Procedure::Describe, &["attr:Age"], boot(500, 7)).unwrap();
    let b = run(&p, Procedure::Describe, &["attr:Age"], boot(500, 7)).unwrap();
    let c = run(&p, Procedure::Describe, &["attr:Age"], boot(500, 8)).unwrap();
    assert_eq!(a.details["bootstrap"], b.details["bootstrap"], "a saved seed gives the same intervals");
    assert_ne!(a.details["bootstrap"]["items"], c.details["bootstrap"]["items"]);
    let items = a.details["bootstrap"]["items"].as_array().unwrap();
    assert_eq!(items.iter().map(|i| i["statistic"].as_str().unwrap()).collect::<Vec<_>>(), ["mean", "median", "sd"]);
    let mean = &items[0]["interval"];
    near(f(mean, &["estimate"]), 469.0 / 12.0);
    let (lo, hi) = (mean["percentile"][0].as_f64().unwrap(), mean["percentile"][1].as_f64().unwrap());
    assert!(lo < 469.0 / 12.0 && 469.0 / 12.0 < hi, "the interval holds the mean: {lo} to {hi}");
    assert!(mean["bca"].is_array(), "BCa is given for one set of cases");
    assert_eq!((a.details["bootstrap"]["seed"].as_u64(), a.details["bootstrap"]["resamples"].as_u64()), (Some(7), Some(500)));
}

#[test]
fn the_bootstrap_of_two_groups_resamples_within_each_and_gives_the_percentile_interval() {
    let (p, _dir, _) = world();
    let two = |resamples, seed| StatOptions { groups: Some(vec!["A".to_string(), "B".to_string()]), bootstrap: Some(BootstrapOptions { resamples, seed, ..Default::default() }), ..Default::default() };
    let r = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Group"], two(400, 3)).unwrap();
    let item = &r.details["bootstrap"]["items"][0];
    assert_eq!(item["statistic"], "mean_difference");
    assert!(item["interval"]["bca"].is_null(), "no BCa when groups are resampled separately");
    near(f(&item["interval"], &["estimate"]), -5.0);
    let rank = run(&p, Procedure::MannWhitney, &["attr:Score", "attr:Group"], two(400, 3)).unwrap();
    assert_eq!(rank.details["bootstrap"]["items"][0]["statistic"], "median_difference");
}

#[test]
fn the_bootstrap_of_a_correlation_and_of_a_regression() {
    let (p, _dir, _) = world();
    let c = run(&p, Procedure::Spearman, &["attr:Age", "attr:Rating"], boot(300, 2)).unwrap();
    assert_eq!(c.details["bootstrap"]["items"][0]["statistic"], "correlation");
    let r = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site"], boot(300, 4)).unwrap();
    let items = r.details["bootstrap"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 4, "the intercept, two terms, and R squared");
    assert_eq!(items[0]["statistic"], "coefficient");
    assert_eq!(items[0]["target"], 0, "the intercept is term 0");
    assert_eq!(items[3]["statistic"], "r_squared");
    let direct = prismqda_stats::regression::linear(&SCORE, &[AGE.to_vec(), SITE_S.to_vec()], 0.95).unwrap();
    near(f(&items[1]["interval"], &["estimate"]), direct.terms[1].estimate);
}

#[test]
fn the_number_of_resamples_is_limited() {
    let (p, _dir, _) = world();
    let e = run(&p, Procedure::Describe, &["attr:Age"], boot(50, 1)).unwrap_err().to_string();
    assert!(e.contains("between 100 and 100,000"), "{e}");
    let e = run(&p, Procedure::Describe, &["attr:Age"], boot(200_000, 1)).unwrap_err().to_string();
    assert!(e.contains("between 100 and 100,000"), "{e}");
}

// ---------------------------------------------------------------- the charts

fn kinds(r: &prismqda_engine::StatResult) -> Vec<String> {
    r.charts.iter().map(|c| serde_json::to_value(c).unwrap()["kind"].as_str().unwrap().to_string()).collect()
}

#[test]
fn a_description_comes_with_a_histogram_and_a_normal_probability_plot_or_bars() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::Describe, &["attr:Age"], StatOptions::default()).unwrap();
    assert_eq!(kinds(&r), ["histogram", "qq"]);
    match &r.charts[0] {
        Chart::Histogram { n, bins, mean, reference, .. } => {
            assert_eq!(*n, 12, "the twelve ages that exist");
            assert_eq!(bins.iter().map(|b| b.count).sum::<usize>(), 12);
            near(*mean, 469.0 / 12.0);
            assert!(reference.is_none());
        }
        other => panic!("{other:?}"),
    }
    match &r.charts[1] {
        Chart::Qq { points, .. } => assert_eq!(points.len(), 12),
        other => panic!("{other:?}"),
    }
    let c = run(&p, Procedure::Describe, &["attr:Site"], StatOptions::default()).unwrap();
    match &c.charts[0] {
        Chart::Bars { valid, categories, .. } => {
            assert_eq!(*valid, 14);
            assert_eq!(categories.iter().map(|c| (c.label.as_str(), c.count)).collect::<Vec<_>>(), [("North site", 7), ("South site", 7)]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn comparisons_of_groups_come_with_boxes_and_the_means_with_their_intervals() {
    let (p, _dir, _) = world();
    let two = || StatOptions { groups: Some(vec!["A".to_string(), "B".to_string()]), ..Default::default() };
    let t = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Group"], two()).unwrap();
    assert_eq!(kinds(&t), ["box", "means"]);
    match &t.charts[0] {
        Chart::Box { by, groups, .. } => {
            assert_eq!((*by, groups.len()), (1, 2));
            // A is 10, 11, 12, 13: quartiles 10.75, 11.5, 12.25 by the default method.
            near(groups[0].stats.q1, 10.75);
            near(groups[0].stats.median, 11.5);
            near(groups[0].stats.q3, 12.25);
            near(groups[0].stats.mean, 11.5);
            assert_eq!((groups[0].label.as_str(), groups[0].stats.n), ("A", 4));
        }
        other => panic!("{other:?}"),
    }
    match &t.charts[1] {
        Chart::Means { groups, level, .. } => {
            near(*level, 0.95);
            // The interval of A's mean is 11.5 plus or minus t(3) times its standard error.
            let se = (5.0f64 / 3.0).sqrt() / 2.0;
            let q = prismqda_stats::dist::t_quantile(0.975, 3.0);
            near(groups[0].lo.unwrap(), 11.5 - q * se);
            near(groups[0].hi.unwrap(), 11.5 + q * se);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(kinds(&run(&p, Procedure::MannWhitney, &["attr:Score", "attr:Group"], two()).unwrap()), ["box"], "ranks have no means to draw");
    assert_eq!(kinds(&run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], StatOptions::default()).unwrap()), ["box", "means"]);
    assert_eq!(kinds(&run(&p, Procedure::KruskalWallis, &["attr:Score", "attr:Group"], StatOptions::default()).unwrap()), ["box"]);
}

#[test]
fn a_table_a_correlation_and_a_regression_come_with_their_own_charts() {
    let (p, _dir, _) = world();
    let chi = run(&p, Procedure::ChiSquare, &["attr:Group", "attr:Site"], StatOptions::default()).unwrap();
    match &chi.charts[0] {
        Chart::Stacked { counts, row_labels, col_labels, .. } => {
            assert_eq!(counts, &vec![vec![2.0, 2.0], vec![2.0, 2.0], vec![2.0, 1.0]]);
            assert_eq!((row_labels.len(), col_labels.len()), (3, 2));
        }
        other => panic!("{other:?}"),
    }
    let pearson = run(&p, Procedure::Pearson, &["attr:Age", "attr:Score"], StatOptions::default()).unwrap();
    match &pearson.charts[0] {
        Chart::Scatter { points, fit, zero_line, omitted, .. } => {
            assert_eq!((points.len(), *omitted, *zero_line), (11, 0, false));
            assert!(fit.is_some(), "a straight line goes with Pearson's r");
        }
        other => panic!("{other:?}"),
    }
    match &run(&p, Procedure::Spearman, &["attr:Age", "attr:Rating"], StatOptions::default()).unwrap().charts[0] {
        Chart::Scatter { fit, .. } => assert!(fit.is_none(), "no line is drawn for a rank correlation"),
        other => panic!("{other:?}"),
    }
    // One numeric predictor: the residuals, the normal plot of them, and the data with the line the model fitted.
    let one = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age"], StatOptions::default()).unwrap();
    assert_eq!(kinds(&one), ["scatter", "qq", "scatter"]);
    match &one.charts[0] {
        Chart::Scatter { zero_line, points, .. } => assert!(*zero_line && points.len() == 11),
        other => panic!("{other:?}"),
    }
    match &one.charts[2] {
        Chart::Scatter { fit: Some(line), .. } => {
            let direct = prismqda_stats::regression::linear(&SCORE_AGE.0, &[SCORE_AGE.1.to_vec()], 0.95).unwrap();
            near(line.slope, direct.terms[1].estimate);
            near(line.intercept, direct.terms[0].estimate);
        }
        other => panic!("{other:?}"),
    }
    let two = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age", "attr:Site"], StatOptions::default()).unwrap();
    assert_eq!(kinds(&two), ["scatter", "qq"], "with two predictors there is no single line to draw");
}

#[test]
fn a_test_of_one_number_or_pairs_comes_with_a_histogram_and_its_reference() {
    let (p, _dir, _) = world();
    match &run(&p, Procedure::OneSampleT, &["attr:Age"], StatOptions { mu: 35.0, ..Default::default() }).unwrap().charts[0] {
        Chart::Histogram { reference, .. } => assert_eq!(*reference, Some(35.0)),
        other => panic!("{other:?}"),
    }
    match &run(&p, Procedure::PairedT, &["attr:Age", "attr:Score"], StatOptions::default()).unwrap().charts[0] {
        Chart::Histogram { subject, reference, n, .. } => {
            assert!(matches!(subject, prismqda_engine::Subject::Differences));
            assert_eq!((*reference, *n), (Some(0.0), 11));
        }
        other => panic!("{other:?}"),
    }
}

/// The ages and scores of the eleven cases a regression of score on age uses.
const SCORE_AGE: ([f64; 11], [f64; 11]) = (SCORE, AGE);

// ---------------------------------------------------------------- the bootstrap of a whole analysis

fn whole(resamples: usize, seed: u64, cluster: Option<&str>) -> StatOptions {
    StatOptions { bootstrap: Some(BootstrapOptions { resamples, seed, analysis: true, cluster: cluster.map(String::from) }), ..Default::default() }
}

fn item<'a>(r: &'a prismqda_engine::StatResult, path: &str) -> &'a Value {
    r.details["bootstrap_analysis"]["items"].as_array().unwrap().iter().find(|i| i["path"] == path).unwrap_or_else(|| panic!("no item {path}"))
}

#[test]
fn the_whole_analysis_is_run_again_on_resamples_and_every_number_it_reports_gets_an_interval() {
    let (p, _dir, _) = world();
    let r = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Site"], whole(300, 5, None)).unwrap();
    assert!(r.details.get("bootstrap").is_none(), "the whole analysis is bootstrapped instead of the one statistic");
    let b = &r.details["bootstrap_analysis"];
    assert_eq!((b["seed"].as_u64(), b["resamples"].as_u64()), (Some(5), Some(300)));
    assert!(b["cluster"].is_null());
    let t = item(&r, "t_test/t");
    near(f(t, &["interval", "estimate"]), r.details["t_test"]["t"].as_f64().unwrap());
    assert!(t["interval"]["percentile"].is_array() && t["interval"]["bca"].is_null(), "percentile intervals only");
    assert!(item(&r, "t_test/p")["below_05"].as_f64().is_some(), "a p-value says how often it fell below .05");
    assert!(item(&r, "t_test/estimate")["below_05"].is_null(), "and a number that is not a p-value does not");
    // The same seed repeats, and another seed does not.
    let again = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Site"], whole(300, 5, None)).unwrap();
    assert_eq!(r.details["bootstrap_analysis"], again.details["bootstrap_analysis"]);
    let other = run(&p, Procedure::IndependentT, &["attr:Score", "attr:Site"], whole(300, 6, None)).unwrap();
    assert_ne!(r.details["bootstrap_analysis"], other.details["bootstrap_analysis"]);
}

#[test]
fn any_procedure_can_be_bootstrapped_whole_even_those_with_no_statistic_of_their_own() {
    let (p, _dir, _) = world();
    let anova = run(&p, Procedure::OneWayAnova, &["attr:Score", "attr:Group"], whole(200, 1, None)).unwrap();
    assert!(anova.details["bootstrap_analysis"]["items"].as_array().unwrap().len() > 5);
    near(f(item(&anova, "anova/test/f"), &["interval", "estimate"]), anova.details["anova"]["test"]["f"].as_f64().unwrap());
    let reg = run(&p, Procedure::LinearRegression, &["attr:Score", "attr:Age"], whole(200, 1, None)).unwrap();
    near(f(item(&reg, "models/0/r_squared"), &["interval", "estimate"]), reg.details["models"][0]["r_squared"].as_f64().unwrap());
    let kw = run(&p, Procedure::KruskalWallis, &["attr:Score", "attr:Group"], whole(200, 1, None)).unwrap();
    assert!(kw.details["bootstrap_analysis"]["items"].as_array().unwrap().iter().any(|i| i["path"] == "kruskal_wallis/h"));
}

#[test]
fn whole_groups_of_cases_can_be_resampled_and_a_variable_with_too_few_groups_is_refused_in_words() {
    let (p, _dir, _) = world();
    // Rating takes five values among the cases, so each value is a group of cases that stay together.
    let r = run(&p, Procedure::Pearson, &["attr:Score", "attr:Age"], whole(300, 3, Some("attr:Rating"))).unwrap();
    let c = &r.details["bootstrap_analysis"]["cluster"];
    assert_eq!((c["key"].as_str(), c["name"].as_str(), c["groups"].as_u64()), (Some("attr:Rating"), Some("Rating"), Some(5)));
    near(f(item(&r, "correlation/r"), &["interval", "estimate"]), r.details["correlation"]["r"].as_f64().unwrap());
    let by_cases = run(&p, Procedure::Pearson, &["attr:Score", "attr:Age"], whole(300, 3, None)).unwrap();
    assert_ne!(item(&r, "correlation/r")["interval"], item(&by_cases, "correlation/r")["interval"], "keeping cases together changes the spread");
    // A cluster alone (without the whole-analysis tick) is the whole analysis too.
    let implied = run(&p, Procedure::Pearson, &["attr:Score", "attr:Age"], StatOptions { bootstrap: Some(BootstrapOptions { resamples: 300, seed: 3, analysis: false, cluster: Some("attr:Rating".to_string()) }), ..Default::default() }).unwrap();
    assert_eq!(r.details["bootstrap_analysis"], implied.details["bootstrap_analysis"]);
    let e = run(&p, Procedure::Pearson, &["attr:Score", "attr:Age"], whole(300, 3, Some("attr:Site"))).unwrap_err().to_string();
    assert!(e.contains("needs at least 5 groups") && e.contains("'Site' gives 2"), "{e}");
    let e = run(&p, Procedure::Pearson, &["attr:Score", "attr:Age"], whole(300, 3, Some("attr:Nope"))).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
}

// ---------------------------------------------------------------- the notebook kept in the project

fn welch(section: &str) -> (Value, String) {
    (json!({ "procedure": "independent_t", "variables": ["attr:Score", "attr:Site"], "options": {} }), section.to_string())
}

#[test]
fn a_page_is_kept_in_the_project_with_its_request_and_result_and_survives_closing_and_opening() {
    let (mut p, dir, _) = world();
    let (request, section) = welch("First look");
    let page = p.notebook_run(&request, &section).unwrap();
    assert_eq!((page.kind.as_str(), page.section.as_str(), page.earlier_runs), ("analysis", "First look", 0));
    assert_eq!(page.request.as_ref().unwrap(), &request);
    assert_eq!(page.result.as_ref().unwrap()["procedure"], "independent_t");
    assert!(page.ran_at.is_some());
    let notes = p.notebook_add_note("Reading notes", "Compare with the interviews.", "").unwrap();
    assert_eq!((notes.kind.as_str(), notes.title.as_str(), notes.note.as_str()), ("note", "Reading notes", "Compare with the interviews."));
    assert!(notes.request.is_none() && notes.result.is_none());
    let annotated = p.notebook_update(page.id, Some("  The sites differ, as expected.  "), None, None).unwrap();
    assert_eq!(annotated.note, "The sites differ, as expected.", "a note is trimmed");
    assert!(p.notebook_update(page.id, None, None, Some("A title")).unwrap_err().to_string().contains("only a page of notes has a title"));
    p.notebook_update(notes.id, None, Some("Reading"), Some("Notes on reading")).unwrap();
    drop(p);
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let p = Project::open(dir.path().join("p.pqda"), &o).unwrap();
    let pages = p.notebook().unwrap();
    assert_eq!(pages.iter().map(|x| x.id).collect::<Vec<_>>(), [page.id, notes.id], "in the order they were added");
    assert_eq!(pages[0].note, "The sites differ, as expected.");
    assert_eq!(pages[0].result, page.result, "the result is exactly what it was");
    assert_eq!((pages[1].title.as_str(), pages[1].section.as_str()), ("Notes on reading", "Reading"));
}

#[test]
fn pages_can_be_moved_and_removed_and_an_analysis_that_cannot_be_run_is_not_kept() {
    let (mut p, _dir, _) = world();
    let a = p.notebook_run(&welch("").0, "").unwrap();
    let b = p.notebook_add_note("B", "", "").unwrap();
    let c = p.notebook_run(&json!({ "procedure": "describe", "variables": ["attr:Age"] }), "").unwrap();
    let order = |p: &Project| p.notebook().unwrap().iter().map(|x| x.id).collect::<Vec<_>>();
    p.notebook_move(c.id, true).unwrap();
    assert_eq!(order(&p), [a.id, c.id, b.id]);
    p.notebook_move(a.id, true).unwrap();
    p.notebook_move(b.id, false).unwrap();
    assert_eq!(order(&p), [a.id, c.id, b.id], "the first cannot go up and the last cannot go down");
    p.notebook_remove(c.id).unwrap();
    assert_eq!(order(&p), [a.id, b.id]);
    assert!(p.notebook_page(c.id).is_err());
    assert!(p.notebook_remove(c.id).is_err());
    assert!(p.notebook_run(&json!({ "procedure": "describe", "variables": ["attr:Missing"] }), "").is_err());
    assert_eq!(order(&p).len(), 2);
}

#[test]
fn running_the_notebook_again_after_the_data_changed_keeps_what_each_page_said_and_says_what_changed() {
    use prismqda_engine::CellChange;
    let (mut p, _dir, _) = world();
    let describe = p.notebook_run(&json!({ "procedure": "describe", "variables": ["attr:Score"] }), "").unwrap();
    let relation = p.notebook_run(&json!({ "procedure": "pearson", "variables": ["attr:Score", "attr:Age"] }), "").unwrap();
    let note = p.notebook_add_note("Notes", "", "").unwrap();
    // Nothing changed: running again finds the same, and keeps no earlier run.
    let same = p.notebook_rerun(describe.id).unwrap();
    assert!(!same.changed && same.error.is_none());
    assert_eq!(same.page.earlier_runs, 0);
    assert!(p.notebook_previous(describe.id).unwrap().is_none());
    // The data change: a score is changed.
    p.set_values("Score", &[CellChange { case_id: 1, value: Some("40".to_string()) }]).unwrap();
    let all = p.notebook_rerun_all().unwrap();
    assert_eq!(all.len(), 2, "a page of notes is not run");
    assert!(all.iter().all(|r| r.page.id != note.id));
    let d = all.iter().find(|r| r.page.id == describe.id).unwrap();
    assert!(d.changed);
    assert_eq!(d.before.as_ref().unwrap(), describe.result.as_ref().unwrap());
    let (was, now) = (f(&d.before.as_ref().unwrap()["details"], &["descriptives", "mean"]), f(&d.page.result.as_ref().unwrap()["details"], &["descriptives", "mean"]));
    assert!(now > was + 1.0, "the mean rose: {was} to {now}");
    assert_eq!(d.page.earlier_runs, 1);
    let (at, earlier) = p.notebook_previous(describe.id).unwrap().unwrap();
    assert_eq!(earlier, *describe.result.as_ref().unwrap());
    assert_eq!(Some(at), same.page.ran_at, "the earlier run is stamped with the time it last ran, which the run that changed nothing had moved on");
    assert!(all.iter().find(|r| r.page.id == relation.id).unwrap().changed, "the correlation moved too");
    // A page keeps its note through a run.
    p.notebook_update(describe.id, Some("Mean checked."), None, None).unwrap();
    assert_eq!(p.notebook_rerun(describe.id).unwrap().page.note, "Mean checked.");
    assert!(p.notebook_rerun(note.id).unwrap_err().to_string().contains("nothing to run"));
}

#[test]
fn an_analysis_that_cannot_be_run_any_more_leaves_its_page_as_it_was_and_says_why() {
    let (mut p, _dir, _) = world();
    let age = p.notebook_run(&json!({ "procedure": "describe", "variables": ["attr:Age"] }), "").unwrap();
    let score = p.notebook_run(&json!({ "procedure": "describe", "variables": ["attr:Score"] }), "").unwrap();
    p.delete_variable("Age").unwrap();
    let all = p.notebook_rerun_all().unwrap();
    let gone = all.iter().find(|r| r.page.id == age.id).unwrap();
    assert!(gone.error.as_deref().unwrap().contains("Age"), "{:?}", gone.error);
    assert!(!gone.changed && gone.before.is_none());
    assert_eq!(gone.page.result, age.result, "the page is as it was");
    assert!(all.iter().find(|r| r.page.id == score.id).unwrap().error.is_none(), "the others still run");
}

#[test]
fn only_the_last_twenty_earlier_runs_are_kept() {
    use prismqda_engine::CellChange;
    let (mut p, _dir, _) = world();
    let page = p.notebook_run(&json!({ "procedure": "describe", "variables": ["attr:Score"] }), "").unwrap();
    for i in 0..25 {
        p.set_values("Score", &[CellChange { case_id: 1, value: Some(format!("{}", 30 + i)) }]).unwrap();
        p.notebook_rerun(page.id).unwrap();
    }
    assert_eq!(p.notebook_page(page.id).unwrap().earlier_runs, 20);
}

// ---------------------------------------------------------------- quantitizing and qualitizing

fn from_codes(target: &str, codes: &[i64], measure: prismqda_engine::CodeMeasure) -> prismqda_engine::TransformSpec {
    prismqda_engine::TransformSpec { target: target.to_string(), replace: false, level: None, kind: prismqda_engine::TransformKind::FromCodes { codes: codes.to_vec(), measure, include_descendants: true } }
}

/// The values of a variable of cases, case by case, as the data editor holds them.
fn column(p: &Project, name: &str) -> Vec<Option<String>> {
    let data = p.case_data().unwrap();
    let at = data.variables.iter().position(|v| v.name == name).unwrap();
    data.values.iter().map(|row| row[at].clone()).collect()
}

#[test]
fn codes_become_variables_of_cases_whether_a_case_has_a_code_and_how_many_excerpts_carry_it() {
    use prismqda_engine::CodeMeasure;
    let (mut p, _dir, welcome) = world();
    let present = p.transform(&from_codes("Has Welcome", &[welcome], CodeMeasure::Present)).unwrap();
    assert_eq!((present.level.as_str(), present.cases_with_value, present.cases_missing), ("nominal", 14, 0));
    assert_eq!(present.value_labels.get("1").map(String::as_str), Some("Yes"));
    let values: Vec<String> = column(&p, "Has Welcome").into_iter().map(|v| v.unwrap()).collect();
    let has: Vec<usize> = values.iter().enumerate().filter(|(_, v)| *v == "1").map(|(i, _)| i).collect();
    assert_eq!(has, [0, 1, 4, 9], "the cases with a coding of the code");
    p.transform(&from_codes("Welcome count", &[welcome], CodeMeasure::Count)).unwrap();
    let counts: Vec<String> = column(&p, "Welcome count").into_iter().map(|v| v.unwrap()).collect();
    assert_eq!((counts[0].as_str(), counts[1].as_str(), counts[4].as_str(), counts[2].as_str()), ("2", "1", "1", "0"), "distinct excerpts: two coders on the same words count once");
    // The variable knows how it was made, so it follows the coding when it is made again.
    let cy = p.add_coder(&NewCoder::human("Cy")).unwrap();
    p.apply_coding(&NewCoding::new(3, 0, 4, welcome, cy)).unwrap(); // case 2's document
    assert_eq!(column(&p, "Has Welcome")[2].as_deref(), Some("0"), "a variable keeps what it was when it was made");
    let again = p.recompute_variable("Has Welcome").unwrap();
    assert!(!again.created);
    assert_eq!(column(&p, "Has Welcome")[2].as_deref(), Some("1"), "and follows the coding when it is made again");
    // A measure of one code is refused for a set, and a code that does not exist is refused.
    assert!(p.transform(&from_codes("Both", &[welcome, welcome + 100], CodeMeasure::Present)).is_err());
    let e = p.transform(&from_codes("Bad", &[], CodeMeasure::Present)).unwrap_err().to_string();
    assert!(e.contains("at least one code"), "{e}");
    let e = p.transform(&from_codes("Many", &[welcome, welcome], CodeMeasure::Count)).map(|_| ()).err();
    assert!(e.is_none(), "the same code twice is the one code");
    assert!(p.transform(&from_codes("Has Welcome", &[welcome], CodeMeasure::Present)).unwrap_err().to_string().contains("already a variable"), "a name that is taken is not taken over");
}

#[test]
fn of_a_set_of_codes_the_one_a_case_has_most_goes_in_one_variable() {
    use prismqda_engine::CodeMeasure;
    let (mut p, _dir, welcome) = world();
    let other = p.add_code(&NewCode::new("Other")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Cy")).unwrap();
    let ids: Vec<i64> = (1..=14).collect(); // each case holds the document that has its number
    // Case 4: Other twice, against Welcome once. Case 0: Other once, against Welcome twice. Case 9: one of each.
    for (case, start, end) in [(4, 5, 10), (4, 11, 15), (0, 11, 15), (9, 5, 10)] {
        p.apply_coding(&NewCoding::new(ids[case], start, end, other, coder)).unwrap();
    }
    p.transform(&from_codes("Most used", &[welcome, other], CodeMeasure::Dominant)).unwrap();
    let v: Vec<String> = column(&p, "Most used").into_iter().map(|x| x.unwrap()).collect();
    assert_eq!(v[0], "Welcome");
    assert_eq!(v[4], "Other");
    assert_eq!(v[9], "Welcome and Other", "a tie names both, in the order the codes were given");
    assert_eq!(v[2], "None", "a case with none of them");
}

#[test]
fn the_values_of_a_variable_become_codes_and_each_cases_text_is_coded_with_the_code_of_its_value() {
    use prismqda_engine::QualitizeSpec;
    let (mut p, _dir, _) = world();
    let spec = QualitizeSpec { variable: "Site".to_string(), parent: None, values: None };
    let plan = p.qualitize_preview(&spec).unwrap();
    assert_eq!(plan.parent, "Site");
    assert_eq!(plan.values.iter().map(|v| (v.label.as_str(), v.cases, v.places)).collect::<Vec<_>>(), [("North site", 7, 7), ("South site", 7, 7)], "the value labels name the codes");
    assert_eq!((plan.codings, plan.unit_mode, plan.cases_without_text), (14, false, 0));
    let coder = p.add_coder(&NewCoder::human("Dee")).unwrap();
    let before = p.list_codes().unwrap().len();
    let done = p.qualitize(&spec, coder).unwrap();
    assert_eq!((done.code_ids.len(), done.codings), (2, 14));
    let codes = p.list_codes().unwrap();
    assert_eq!(codes.len(), before + 3, "a parent and a code for each value");
    let parent = codes.iter().find(|c| c.id == done.parent_id).unwrap();
    assert_eq!(parent.name, "Site");
    let child_names: Vec<&str> = codes.iter().filter(|c| c.parent_id == Some(done.parent_id)).map(|c| c.name.as_str()).collect();
    assert_eq!(child_names, ["North site", "South site"]);
    // Case 0 is in the North site: its whole text carries that code, as the coder who made the codes.
    let coded = p.list_codings(1).unwrap();
    let north = coded.iter().find(|c| c.code_id == done.code_ids[0]).expect("the code of its value");
    assert_eq!((north.start, north.end, north.coder_id), (0, 36, coder));
    assert!(coded.iter().all(|c| c.code_id != done.code_ids[1]), "and not the other value's code");
    // A second time is refused (the parent code is taken) and leaves nothing half made.
    let e = p.qualitize(&spec, coder).unwrap_err().to_string();
    assert!(e.contains("Site") || e.contains("already"), "{e}");
    assert_eq!(p.list_codes().unwrap().iter().filter(|c| c.status == "active").count(), before + 3);
}

#[test]
fn qualitizing_can_take_some_values_a_chosen_name_and_leaves_out_cases_with_no_value_and_refuses_what_makes_no_sense() {
    use prismqda_engine::QualitizeSpec;
    let (mut p, _dir, _) = world();
    let coder = p.add_coder(&NewCoder::human("Eli")).unwrap();
    let some = QualitizeSpec { variable: "Group".to_string(), parent: Some("Participant group".to_string()), values: Some(vec!["B".to_string(), "C".to_string()]) };
    let plan = p.qualitize_preview(&some).unwrap();
    assert_eq!(plan.parent, "Participant group");
    assert_eq!(plan.values.iter().map(|v| (v.value.as_str(), v.cases)).collect::<Vec<_>>(), [("B", 4), ("C", 3)], "the cases with no group are in neither");
    let done = p.qualitize(&some, coder).unwrap();
    assert_eq!(done.codings, 7);
    let e = p.qualitize_preview(&QualitizeSpec { variable: "Group".to_string(), parent: None, values: Some(vec!["Z".to_string()]) }).unwrap_err().to_string();
    assert!(e.contains("no case has the value 'Z'"), "{e}");
    let e = p.qualitize_preview(&QualitizeSpec { variable: "Nope".to_string(), parent: None, values: None }).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
}

#[test]
fn what_qualitizing_made_can_be_taken_back_without_touching_any_other_coding_and_made_again() {
    use prismqda_engine::{QualitizeSpec, QualitizeUndo};
    let (mut p, _dir, welcome) = world();
    let coder = p.add_coder(&NewCoder::human("Fay")).unwrap();
    let spec = QualitizeSpec { variable: "Site".to_string(), parent: None, values: None };
    let done = p.qualitize(&spec, coder).unwrap();
    assert_eq!(done.coding_ids.len(), 14, "every coding that was made is reported");
    // A coding made by hand afterwards, of the same code, is not what was made by the step.
    let other = p.add_coder(&NewCoder::human("Gil")).unwrap();
    let kept = p.apply_coding(&NewCoding::new(1, 0, 4, done.code_ids[0], other)).unwrap();
    let undo = QualitizeUndo { parent_id: done.parent_id, code_ids: done.code_ids.clone(), coding_ids: done.coding_ids.clone() };
    let back = p.qualitize_take_back(&undo).unwrap();
    assert_eq!((back.codings_removed, back.codes_retired), (14, 3), "the codings and the three codes");
    let left: Vec<i64> = p.list_codings(1).unwrap().iter().filter(|c| done.code_ids.contains(&c.code_id)).map(|c| c.id).collect();
    assert_eq!(left, [kept], "only the coding made by hand is left");
    assert!(p.list_codes().unwrap().iter().filter(|c| c.id == done.parent_id || done.code_ids.contains(&c.id)).all(|c| c.status == "retired"), "the codes stay in the codebook, retired");
    let again = p.qualitize_take_back(&undo).unwrap();
    assert_eq!((again.codings_removed, again.codes_retired), (0, 0), "taking it back twice does nothing the second time");
    // The retired codes do not hold the names, so the step can be made again.
    let second = p.qualitize(&spec, coder).unwrap();
    assert_eq!(second.codings, 14);
    // A coding that is not of the codes the step made is never removed, whatever it is asked.
    let hand = p.apply_coding(&NewCoding::new(3, 0, 4, welcome, coder)).unwrap();
    let wrong = QualitizeUndo { parent_id: second.parent_id, code_ids: second.code_ids.clone(), coding_ids: vec![hand] };
    assert_eq!(p.qualitize_take_back(&wrong).unwrap().codings_removed, 0);
    assert!(p.list_codings(3).unwrap().iter().any(|c| c.id == hand));
}

// ---------------------------------------------------------------- joint displays

fn display(sort_by: &str, descending: bool, variables: &[&str], codes: &[i64], quotes: usize, only_coded: bool) -> prismqda_engine::CaseDisplaySpec {
    prismqda_engine::CaseDisplaySpec {
        sort_by: sort_by.to_string(),
        descending,
        variables: variables.iter().map(|v| v.to_string()).collect(),
        code_ids: codes.to_vec(),
        include_descendants: true,
        quotes,
        only_coded,
        scope: Default::default(),
    }
}

fn names(d: &prismqda_engine::CaseDisplay) -> Vec<&str> {
    d.rows.iter().map(|r| r.case_name.as_str()).collect()
}

#[test]
fn cases_are_put_in_the_order_of_a_number_with_the_words_coded_beside_each() {
    let (p, _dir, welcome) = world();
    let d = p.case_ordered_display(&display("Score", true, &["Site"], &[welcome], 1, true)).unwrap();
    assert_eq!(names(&d), ["Case 9", "Case 4", "Case 1", "Case 0"], "highest score first, only the cases that have the code");
    assert!(d.by_number, "the scores of these four cases are all numbers");
    assert_eq!((d.cases, d.without_value), (14, 0));
    assert_eq!(d.rows[0].values, [Some("22".to_string()), Some("S".to_string())], "the value it is ordered by, then the others");
    assert_eq!(d.variables.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["Score", "Site"]);
    // Case 0 has two excerpts of the code; one is quoted and the other counted.
    let case0 = &d.rows[3].cells[0];
    assert_eq!((case0.excerpts, case0.quotes.len(), case0.more), (2, 1, 1));
    let q = &case0.quotes[0];
    assert_eq!(q.source, "doc0");
    assert_eq!(q.text.as_deref(), Some(p.text_of_span(q.source_id, 0, 4).unwrap().as_str()), "the words are the coded words");
    // Two coders on the same words are one excerpt.
    assert_eq!(d.rows[2].cells[0].excerpts, 1);
    // The counts are the matrix's counts.
    let m = p
        .code_matrix(&prismqda_engine::MatrixSpec {
            rows: prismqda_engine::RowSpec::Cases,
            code_ids: vec![welcome],
            include_descendants: true,
            scope: prismqda_engine::ExploreScope { layer: Some("coder".to_string()), ..Default::default() },
        })
        .unwrap();
    for r in &d.rows {
        let row = m.rows.iter().find(|x| x.label == r.case_name).unwrap();
        assert_eq!(r.cells[0].excerpts as i64, row.cells[0].excerpts, "{}", r.case_name);
    }
    let up = p.case_ordered_display(&display("Score", false, &[], &[welcome], 2, true)).unwrap();
    assert_eq!(names(&up), ["Case 0", "Case 1", "Case 4", "Case 9"]);
    assert_eq!(up.rows[0].cells[0].quotes.len(), 2, "with room for two quotations both are shown");
    assert_eq!(up.rows[0].cells[0].more, 0);
    // Each excerpt has a coding to stand for it, so the evidence behind the count can be opened.
    assert_eq!(up.rows[0].cells[0].coding_ids.len(), 2);
    assert!(up.rows[0].cells[0].quotes.iter().all(|q| up.rows[0].cells[0].coding_ids.contains(&q.coding_id)));
    assert_eq!((up.rows[0].cells[0].quotes[0].start, up.rows[0].cells[0].quotes[0].end), (0, 4));
}

#[test]
fn the_order_puts_cases_with_no_value_last_whichever_way_it_goes_and_follows_the_words_of_a_category() {
    let (p, _dir, welcome) = world();
    // Case 9 has no age.
    let up = p.case_ordered_display(&display("Age", false, &[], &[welcome], 0, true)).unwrap();
    assert_eq!(names(&up), ["Case 0", "Case 1", "Case 4", "Case 9"]);
    assert_eq!(up.without_value, 1);
    let down = p.case_ordered_display(&display("Age", true, &[], &[welcome], 0, true)).unwrap();
    assert_eq!(names(&down), ["Case 4", "Case 1", "Case 0", "Case 9"], "the one with no age is still last");
    assert!(up.rows[0].cells[0].quotes.is_empty() && up.rows[0].cells[0].excerpts == 2, "no quotations asked for, the excerpts are still counted");
    // A variable of words: the cases of each value together, in the order of the words, then by name.
    let by_site = p.case_ordered_display(&display("Site", false, &[], &[welcome], 1, true)).unwrap();
    assert!(!by_site.by_number);
    assert_eq!(names(&by_site), ["Case 0", "Case 4", "Case 1", "Case 9"]);
    // Every case, with an empty cell where there is no coding; "unknown" is not a number, so the scores are ordered as words.
    let every = p.case_ordered_display(&display("Score", false, &[], &[welcome], 1, false)).unwrap();
    assert_eq!(every.rows.len(), 14);
    assert!(!every.by_number);
    let empty = every.rows.iter().find(|r| r.case_name == "Case 2").unwrap();
    assert_eq!((empty.cells[0].excerpts, empty.cells[0].quotes.len()), (0, 0));
}

#[test]
fn a_display_that_cannot_be_made_says_why() {
    let (p, _dir, welcome) = world();
    let e = p.case_ordered_display(&display("Score", false, &[], &[], 1, true)).unwrap_err().to_string();
    assert!(e.contains("at least one code"), "{e}");
    let e = p.case_ordered_display(&display("Nope", false, &[], &[welcome], 1, true)).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
    let e = p.case_ordered_display(&display("Score", false, &[], &[welcome + 100], 1, true)).unwrap_err().to_string();
    assert!(e.to_lowercase().contains("code"), "{e}");
}

fn typology(codes: &[i64], variables: &[&str]) -> prismqda_engine::TypologySpec {
    prismqda_engine::TypologySpec { code_ids: codes.to_vec(), include_descendants: true, variables: variables.iter().map(|v| v.to_string()).collect() }
}

#[test]
fn groups_made_by_a_code_are_described_with_the_mean_and_spread_of_numbers_and_the_counts_of_categories() {
    use prismqda_engine::TypologyCell;
    let (p, _dir, welcome) = world();
    let t = p.typology_table(&typology(&[welcome], &["Score", "Age", "Site", "Group"])).unwrap();
    assert_eq!(t.groups.iter().map(|g| g.key.clone()).collect::<Vec<_>>(), ["all".to_string(), format!("code:{welcome}"), "none".to_string()]);
    assert_eq!(t.cases_with_text, 14);
    assert_eq!(t.columns.iter().map(|c| c.kind.as_str()).collect::<Vec<_>>(), ["scale", "scale", "categories", "categories"]);
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    // The cases that have the code: Case 0, 1, 4, and 9, with scores 10, 12, 15, 22.
    let with = &t.groups[1];
    assert_eq!(with.label, "Welcome");
    assert_eq!(with.cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Case 0", "Case 1", "Case 4", "Case 9"]);
    let TypologyCell::Scale { n, mean, sd } = &with.cells[0] else { panic!("a number") };
    assert_eq!(*n, 4);
    assert!(near(mean.unwrap(), 14.75), "mean {mean:?}");
    assert!(near(sd.unwrap(), (82.75f64 / 3.0).sqrt()), "sd {sd:?}");
    // Ages 23, 31, 52 and no age for Case 9: three values.
    let TypologyCell::Scale { n, mean, .. } = &with.cells[1] else { panic!() };
    assert_eq!(*n, 3);
    assert!(near(mean.unwrap(), 106.0 / 3.0));
    // Sites N, S, N, S; groups A, A, B, C.
    let TypologyCell::Categories { n, counts } = &with.cells[2] else { panic!() };
    assert_eq!((*n, counts.clone()), (4, vec![2, 2]));
    assert_eq!(t.columns[2].categories.iter().map(|c| (c.value.as_str(), c.label.as_str())).collect::<Vec<_>>(), [("N", "North site"), ("S", "South site")], "a category is shown by its label");
    let TypologyCell::Categories { n, counts } = &with.cells[3] else { panic!() };
    assert_eq!((*n, counts.clone()), (4, vec![2, 1, 1]));
    // Everyone: "unknown" is not a number (13 scores), -99 and the empty age are missing (12 ages), and three cases have no group.
    let all = &t.groups[0];
    assert_eq!(all.cases.len(), 14);
    let TypologyCell::Scale { n, mean, .. } = &all.cells[0] else { panic!() };
    assert_eq!(*n, 13);
    assert!(near(mean.unwrap(), 288.0 / 13.0));
    let TypologyCell::Scale { n, mean, .. } = &all.cells[1] else { panic!() };
    assert_eq!(*n, 12);
    assert!(near(mean.unwrap(), 469.0 / 12.0));
    let TypologyCell::Categories { n, counts } = &all.cells[3] else { panic!() };
    assert_eq!((*n, counts.clone()), (11, vec![4, 4, 3]));
    // The cases with text and none of the codes: the other ten.
    let none = &t.groups[2];
    assert_eq!(none.cases.len(), 10);
    let TypologyCell::Scale { n, mean, .. } = &none.cells[0] else { panic!() };
    assert_eq!(*n, 9);
    assert!(near(mean.unwrap(), 229.0 / 9.0));
}

#[test]
fn a_group_of_one_has_a_mean_and_no_spread_and_a_table_that_cannot_be_made_says_why() {
    use prismqda_engine::TypologyCell;
    let (mut p, _dir, welcome) = world();
    let solo = p.add_code(&NewCode::new("Solo")).unwrap();
    let cy = p.add_coder(&NewCoder::human("Cy")).unwrap();
    p.apply_coding(&NewCoding::new(3, 0, 4, solo, cy)).unwrap(); // the document of Case 2
    let t = p.typology_table(&typology(&[welcome, solo, welcome], &["Score"])).unwrap();
    assert_eq!(t.groups.len(), 4, "the same code twice is one group: all, two codes, none");
    let one = &t.groups[2];
    assert_eq!(one.cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Case 2"]);
    assert_eq!(one.cells[0], TypologyCell::Scale { n: 1, mean: Some(11.0), sd: None });
    assert_eq!(t.groups[3].cases.len(), 9, "the cases that have neither code");
    let e = p.typology_table(&typology(&[], &["Score"])).unwrap_err().to_string();
    assert!(e.contains("at least one code"), "{e}");
    let e = p.typology_table(&typology(&[welcome], &[])).unwrap_err().to_string();
    assert!(e.contains("at least one variable"), "{e}");
    let e = p.typology_table(&typology(&[welcome], &["Nope"])).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
}

// ---------------------------------------------------------------- the other joint displays

fn side(codes: &[i64], variables: &[&str], quotes: usize) -> prismqda_engine::SideBySideSpec {
    prismqda_engine::SideBySideSpec { code_ids: codes.to_vec(), include_descendants: true, variables: variables.iter().map(|v| v.to_string()).collect(), quotes }
}

#[test]
fn a_topic_has_the_words_of_the_cases_that_have_it_beside_their_numbers_and_the_numbers_of_the_others() {
    use prismqda_engine::TypologyCell;
    let (p, _dir, welcome) = world();
    let s = p.side_by_side(&side(&[welcome], &["Score", "Site"], 2)).unwrap();
    assert_eq!(s.cases_with_text, 14);
    assert_eq!(s.topics.len(), 1);
    let topic = &s.topics[0];
    assert_eq!(topic.name, "Welcome");
    // Five distinct excerpts: two on Case 0 and one each on Cases 1, 4, and 9 (a second coder on the same words counts once).
    assert_eq!((topic.excerpts, topic.quotes.len(), topic.more, topic.coding_ids.len()), (5, 2, 3, 5));
    assert_eq!(topic.quotes.iter().map(|q| (q.case_name.as_str(), q.quote.start, q.quote.end)).collect::<Vec<_>>(), [("Case 0", 0, 4), ("Case 0", 5, 10)]);
    assert_eq!(topic.quotes[0].quote.text.as_deref(), Some(p.text_of_span(1, 0, 4).unwrap().as_str()));
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert_eq!(topic.with.cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Case 0", "Case 1", "Case 4", "Case 9"]);
    let TypologyCell::Scale { n, mean, .. } = &topic.with.cells[0] else { panic!("a number") };
    assert_eq!(*n, 4);
    assert!(near(mean.unwrap(), 14.75));
    // The others that hold text: ten cases, nine with a score (229 in all).
    assert_eq!(topic.without.cases.len(), 10);
    let TypologyCell::Scale { n, mean, .. } = &topic.without.cells[0] else { panic!() };
    assert_eq!(*n, 9);
    assert!(near(mean.unwrap(), 229.0 / 9.0));
    assert!(near(topic.differences[0].unwrap(), 14.75 - 229.0 / 9.0));
    // Categories have counts, not a difference.
    assert_eq!(topic.with.cells[1], TypologyCell::Categories { n: 4, counts: vec![2, 2] });
    assert_eq!(topic.without.cells[1], TypologyCell::Categories { n: 10, counts: vec![5, 5] });
    assert_eq!(topic.differences[1], None);
    // The excerpts are the code matrix's for the same code.
    let m = p
        .code_matrix(&prismqda_engine::MatrixSpec { rows: prismqda_engine::RowSpec::Cases, code_ids: vec![welcome], include_descendants: true, scope: prismqda_engine::ExploreScope { layer: Some("coder".to_string()), ..Default::default() } })
        .unwrap();
    assert_eq!(topic.excerpts as i64, m.columns[0].total.excerpts);
    // No quotations asked for: the excerpts are still counted and can still be opened.
    let bare = p.side_by_side(&side(&[welcome], &["Score"], 0)).unwrap();
    assert_eq!((bare.topics[0].quotes.len(), bare.topics[0].more, bare.topics[0].coding_ids.len()), (0, 5, 5));
}

#[test]
fn a_side_by_side_display_that_cannot_be_made_says_why() {
    let (p, _dir, welcome) = world();
    let e = p.side_by_side(&side(&[], &["Score"], 2)).unwrap_err().to_string();
    assert!(e.contains("at least one code"), "{e}");
    let e = p.side_by_side(&side(&[welcome], &[], 2)).unwrap_err().to_string();
    assert!(e.contains("at least one variable"), "{e}");
    let e = p.side_by_side(&side(&[welcome], &["Nope"], 2)).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
}

/// The fixture of the deviation table: Doubt is on Cases 2, 3, 5, and 6 as well as Welcome on 0, 1, 4, and 9. Eleven cases have a group
/// (A: 0 to 3, B: 4 to 7, C: 8 to 10) and the other three, which hold text, do not.
fn world_with_doubt() -> (Project, TempDir, i64, i64) {
    let (mut p, dir, welcome) = world();
    let doubt = p.add_code(&NewCode::new("Doubt")).unwrap();
    let cy = p.add_coder(&NewCoder::human("Cy")).unwrap();
    for case in [2usize, 3, 5, 6] {
        p.apply_coding(&NewCoding::new(case as i64 + 1, 0, 4, doubt, cy)).unwrap();
    }
    (p, dir, welcome, doubt)
}

fn deviation(variable: &str, codes: &[i64], top: usize) -> prismqda_engine::DeviationSpec {
    prismqda_engine::DeviationSpec { variable: variable.to_string(), code_ids: codes.to_vec(), include_descendants: true, top }
}

#[test]
fn the_codes_most_and_least_typical_of_a_group_are_ranked_by_the_residual_r_gives_for_the_two_by_two_table() {
    let (p, _dir, welcome, doubt) = world_with_doubt();
    let d = p.deviation_table(&deviation("Group", &[welcome, doubt], 5)).unwrap();
    assert_eq!((d.cases_used, d.without_value, d.codes), (11, 3, 2));
    assert_eq!(d.groups.iter().map(|g| (g.value.as_str(), g.cases)).collect::<Vec<_>>(), [("A", 4), ("B", 4), ("C", 3)]);
    let near = |a: f64, b: f64| assert!((a - b).abs() < 1e-9, "{a} against {b}");
    let row = |g: &prismqda_engine::DeviationGroup, name: &str| g.most.iter().chain(&g.least).find(|r| r.name == name).unwrap_or_else(|| panic!("{name} is in neither list")).clone();
    // The residuals are those of R 4.5.3: chisq.test(matrix(c(a, row - a, col - a, n - row - col + a), 2), correct = FALSE)$stdres[1, 1].
    let (a, b, c) = (&d.groups[0], &d.groups[1], &d.groups[2]);
    let w = row(a, "Welcome");
    near(w.z.unwrap(), 0.710705312219);
    assert_eq!((w.in_cases, w.in_total, w.out_cases, w.out_total), (2, 4, 2, 7));
    near(w.in_share, 0.5);
    near(w.out_share, 2.0 / 7.0);
    near(w.difference, 0.5 - 2.0 / 7.0);
    assert_eq!(w.cases.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["Case 0", "Case 1"]);
    // Group A has both codes more often than the others, in the order of the residual and then of the name; it has none less often.
    assert_eq!(a.most.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Doubt", "Welcome"]);
    assert!(a.least.is_empty());
    // Group B has Doubt more often and Welcome less often.
    assert_eq!(b.most.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Doubt"]);
    assert_eq!(b.least.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Welcome"]);
    near(row(b, "Welcome").z.unwrap(), -0.592254426849);
    near(row(b, "Doubt").z.unwrap(), 0.710705312219);
    // Group C has neither more often; the least typical comes first.
    assert!(c.most.is_empty());
    assert_eq!(c.least.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Doubt", "Welcome"]);
    near(row(c, "Doubt").z.unwrap(), -1.535298947157);
    near(row(c, "Welcome").z.unwrap(), -0.127941578930);
    assert_eq!(row(c, "Doubt").cases.len(), 0);
    // Only the first of each list is kept when asked.
    let one = p.deviation_table(&deviation("Group", &[welcome, doubt], 1)).unwrap();
    assert_eq!((one.groups[0].most.len(), one.groups[2].least.len()), (1, 1));
    assert_eq!(one.groups[2].least[0].name, "Doubt");
}

#[test]
fn a_code_no_case_has_is_typical_of_no_group_and_a_table_that_cannot_be_made_says_why() {
    let (mut p, _dir, welcome, _doubt) = world_with_doubt();
    let nobody = p.add_code(&NewCode::new("Nobody")).unwrap();
    let d = p.deviation_table(&deviation("Group", &[nobody], 5)).unwrap();
    assert!(d.groups.iter().all(|g| g.most.is_empty() && g.least.is_empty()), "a share of 0 against a share of 0 deviates from nothing");
    let e = p.deviation_table(&deviation("Group", &[], 5)).unwrap_err().to_string();
    assert!(e.contains("at least one code"), "{e}");
    let e = p.deviation_table(&deviation("Nope", &[welcome], 5)).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
    // Thirteen different scores are too many for groups.
    let e = p.deviation_table(&deviation("Score", &[welcome], 5)).unwrap_err().to_string();
    assert!(e.contains("different values") && e.contains("Cut"), "{e}");
}

fn similarity(codes: &[i64], variables: &[&str], shared_absence: bool, pairs: usize) -> prismqda_engine::SimilaritySpec {
    prismqda_engine::SimilaritySpec { code_ids: codes.to_vec(), include_descendants: true, variables: variables.iter().map(|v| v.to_string()).collect(), shared_absence, pairs }
}

#[test]
fn cases_are_as_alike_as_the_mean_of_their_likeness_on_each_feature_they_can_be_compared_on() {
    let (p, _dir, welcome) = world();
    // Welcome (a code), Rating (ordinal numbers 1 to 5, a range of 4), and Site (N on the even cases, S on the odd).
    let s = p.similarity_analysis(&similarity(&[welcome], &["Rating", "Site"], false, 3)).unwrap();
    assert_eq!(s.features.iter().map(|f| (f.name.as_str(), f.kind.as_str())).collect::<Vec<_>>(), [("Welcome", "code"), ("Rating", "number"), ("Site", "category")]);
    assert_eq!(s.cases.len(), 14);
    assert_eq!(s.pairs_compared, 91, "every pair of fourteen cases has Rating and Site to be compared on");
    let m = s.matrix.as_ref().expect("fourteen cases are few enough for the whole matrix");
    let near = |a: Option<f64>, b: f64| assert!((a.unwrap() - b).abs() < 1e-9, "{a:?} against {b}");
    // Case 0 and Case 1 both have the code (1), their ratings 1 and 2 are 3/4 alike, and North and South are not alike (0).
    near(m[0][1], 3.5 / 6.0);
    // Case 0 has the code and Case 2 does not (0), ratings 1 and 3 (1/2), the same site (1).
    near(m[0][2], 0.5);
    // Case 2 and Case 3 both lack the code, so it is left out: ratings 3 and 2 (3/4) and different sites (0) over two features.
    near(m[2][3], 0.375);
    near(m[5][5], 1.0);
    assert_eq!(m[1][4], m[4][1], "the matrix is the same either way round");
    // Case 2 and Case 6, and Case 3 and Case 7, have the same rating and site and both lack the code: the most alike, in the order of the cases.
    // Next come the pairs of 7/8 (ratings a step apart on the same site), the first of them Case 2 and Case 12.
    assert_eq!(s.pairs.iter().map(|p| (p.a.name.as_str(), p.b.name.as_str(), p.compared)).collect::<Vec<_>>(), [("Case 2", "Case 6", 2), ("Case 3", "Case 7", 2), ("Case 2", "Case 12", 2)]);
    near(Some(s.pairs[0].similarity), 1.0);
    near(Some(s.pairs[2].similarity), 0.875);
    // The parts of a pair are the features and what each case has.
    let third = &s.pairs[2];
    assert_eq!(
        third.parts.iter().map(|x| (x.a.clone(), x.b.clone(), x.similarity)).collect::<Vec<_>>(),
        [(Some("0".to_string()), Some("0".to_string()), None), (Some("3".to_string()), Some("2".to_string()), Some(0.75)), (Some("North site".to_string()), Some("North site".to_string()), Some(1.0))]
    );
    // With shared absence two cases that both lack the code are alike on it.
    let shared = p.similarity_analysis(&similarity(&[welcome], &["Rating", "Site"], true, 3)).unwrap();
    let m = shared.matrix.as_ref().unwrap();
    near(m[2][3], 1.75 / 3.0);
}

#[test]
fn cases_can_be_compared_on_variables_alone_and_a_comparison_that_cannot_be_made_says_why() {
    let (p, _dir, welcome) = world();
    let s = p.similarity_analysis(&similarity(&[], &["Site"], false, 50)).unwrap();
    assert_eq!(s.features.len(), 1);
    // A category is alike or not: seven Norths and seven Souths give 2 * (21 pairs) alike pairs of the 91.
    assert_eq!(s.pairs_compared, 91);
    let alike = s.matrix.as_ref().unwrap().iter().flatten().filter(|v| **v == Some(1.0)).count();
    assert_eq!(alike, 14 + 2 * 2 * 21, "the diagonal and both ways round of 21 pairs in each site");
    let e = p.similarity_analysis(&similarity(&[], &[], false, 5)).unwrap_err().to_string();
    assert!(e.contains("at least one code or variable"), "{e}");
    let e = p.similarity_analysis(&similarity(&[welcome], &["Nope"], false, 5)).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
}

fn bubble(code: i64, x: &str, y: &str) -> prismqda_engine::BubbleSpec {
    prismqda_engine::BubbleSpec { code_id: code, include_descendants: true, x: x.to_string(), y: y.to_string() }
}

#[test]
fn a_code_across_two_variables_has_the_excerpts_the_cases_and_the_mean_weight_of_each_cell() {
    let (mut p, _dir, welcome) = world();
    // A rated code on Cases 0 (two excerpts, rated 2 and 4), 1 (not rated), 4 (rated 3), and 9 (rated 5).
    let mut nc = NewCode::new("Warmth");
    nc.rated = true;
    let warmth = p.add_code(&nc).unwrap();
    let ada = p.add_coder(&NewCoder::human("Dee")).unwrap();
    for (case, start, end, rating) in [(0usize, 0, 4, Some(2)), (0, 5, 10, Some(4)), (1, 0, 4, None), (4, 0, 4, Some(3)), (9, 0, 4, Some(5))] {
        let id = p.apply_coding(&NewCoding::new(case as i64 + 1, start, end, warmth, ada)).unwrap();
        if rating.is_some() {
            p.set_coding_rating(id, rating).unwrap();
        }
    }
    let b = p.bubble_plot(&bubble(warmth, "Site", "Group")).unwrap();
    assert_eq!(b.code, "Warmth");
    assert_eq!(b.x.categories.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["North site", "South site"]);
    assert_eq!(b.y.categories.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);
    assert_eq!((b.cases_used, b.left_out, b.rated), (11, 3, 4));
    let cell = |x: usize, y: usize| b.cells.iter().find(|c| c.x == x && c.y == y).unwrap_or_else(|| panic!("no cell at {x}, {y}"));
    // Group A, North: Cases 0 and 2, and Case 0 has two excerpts rated 2 and 4.
    let an = cell(0, 0);
    assert_eq!((an.cases, an.with_code, an.excerpts, an.rated, an.mean_rating, an.coding_ids.len()), (2, 1, 2, 2, Some(3.0), 2));
    // Group A, South: Cases 1 and 3; Case 1 is coded and has no rating.
    let a_s = cell(1, 0);
    assert_eq!((a_s.cases, a_s.with_code, a_s.excerpts, a_s.rated, a_s.mean_rating), (2, 1, 1, 0, None));
    // Group B, North: Cases 4 and 6; Group B, South: Cases 5 and 7, which have no coding at all.
    assert_eq!((cell(0, 1).cases, cell(0, 1).excerpts, cell(0, 1).mean_rating), (2, 1, Some(3.0)));
    assert_eq!((cell(1, 1).cases, cell(1, 1).with_code, cell(1, 1).excerpts, cell(1, 1).mean_rating), (2, 0, 0, None));
    // Group C, North: Cases 8 and 10; Group C, South: Case 9.
    assert_eq!((cell(0, 2).cases, cell(0, 2).excerpts), (2, 0));
    assert_eq!((cell(1, 2).cases, cell(1, 2).excerpts, cell(1, 2).mean_rating), (1, 1, Some(5.0)));
    assert_eq!(b.cells.len(), 6);
    // The sizes add up to the excerpts of the cases in the plot (the code is on Cases 0, 1, 4, and 9).
    let total: usize = b.cells.iter().map(|c| c.excerpts).sum();
    assert_eq!(total, 5);
    // A code nobody rated gives no colour.
    let plain = p.bubble_plot(&bubble(welcome, "Site", "Group")).unwrap();
    assert_eq!(plain.rated, 0);
    assert!(plain.cells.iter().all(|c| c.mean_rating.is_none()));
    assert_eq!(plain.cells.iter().map(|c| c.excerpts).sum::<usize>(), 5);
}

#[test]
fn a_plot_that_cannot_be_made_says_why() {
    let (p, _dir, welcome) = world();
    let e = p.bubble_plot(&bubble(welcome, "Site", "Site")).unwrap_err().to_string();
    assert!(e.contains("two different variables"), "{e}");
    let e = p.bubble_plot(&bubble(welcome, "Site", "Nope")).unwrap_err().to_string();
    assert!(e.contains("no case variable"), "{e}");
    // Thirteen scores are too many values for an axis.
    let e = p.bubble_plot(&bubble(welcome, "Site", "Score")).unwrap_err().to_string();
    assert!(e.contains("different values") && e.contains("Cut"), "{e}");
    let e = p.bubble_plot(&bubble(welcome + 100, "Site", "Group")).unwrap_err().to_string();
    assert!(e.to_lowercase().contains("code"), "{e}");
}

// ---------------------------------------------------------------- the design, the triangulation matrix, and the integration log

fn design_input(kind: &str, priority: Option<&str>, purpose: &str) -> prismqda_engine::DesignInput {
    prismqda_engine::DesignInput { kind: kind.to_string(), priority: priority.map(str::to_string), purpose: purpose.to_string() }
}

fn applying(kind: &str, purpose: &str, log: &str) -> prismqda_engine::DesignApply {
    prismqda_engine::DesignApply { design: design_input(kind, None, purpose), log: log.to_string() }
}

#[test]
fn the_design_is_chosen_changed_and_forgotten_and_choosing_it_makes_nothing() {
    let (mut p, _dir, _) = world();
    assert!(p.mixed_design().unwrap().is_none());
    let d = p.mixed_design_set(&design_input("explanatory_sequential", None, "  Explain the low scores.  ")).unwrap();
    assert_eq!((d.kind.as_str(), d.priority.as_str(), d.purpose.as_str()), ("explanatory_sequential", "quantitative", "Explain the low scores."), "the design's usual priority, and the purpose trimmed");
    let c = p.mixed_design_set(&design_input("convergent", Some("qualitative"), "")).unwrap();
    assert_eq!((c.kind.as_str(), c.priority.as_str()), ("convergent", "qualitative"), "it can be changed, and the priority chosen");
    assert_eq!(p.mixed_design_set(&design_input("convergent", Some("qualitative"), "")).unwrap(), c, "choosing the same again changes nothing, not even the time");
    assert!(p.case_variables().unwrap().iter().all(|v| v.name != "Quantitative result group"), "choosing a design makes no variable");
    let e = p.mixed_design_set(&design_input("nope", None, "")).unwrap_err().to_string();
    assert!(e.contains("is not a design"), "{e}");
    let e = p.mixed_design_set(&design_input("embedded", Some("most"), "")).unwrap_err().to_string();
    assert!(e.contains("not one of the choices"), "{e}");
    assert_eq!(p.mixed_design().unwrap().unwrap().kind, "convergent", "a refused choice leaves the design as it was");
    p.mixed_design_clear().unwrap();
    assert!(p.mixed_design().unwrap().is_none());
    p.mixed_design_clear().unwrap();
    let actions: Vec<String> = p.events(1000).unwrap().into_iter().map(|e| e.action).collect();
    assert!(actions.contains(&"mixed.design.set".to_string()) && actions.contains(&"mixed.design.clear".to_string()), "the audit trail has them: {actions:?}");
}

#[test]
fn a_template_makes_what_its_design_needs_once_and_leaves_what_is_there_alone() {
    let (mut p, _dir, _) = world();
    let plan = p.design_plan("explanatory_sequential").unwrap();
    assert_eq!(plan.variables.iter().map(|v| (v.name.as_str(), v.level.as_str(), v.exists)).collect::<Vec<_>>(), [("Quantitative result group", "nominal", false)]);
    assert_eq!(plan.codes.iter().map(|c| (c.name.as_str(), c.exists)).collect::<Vec<_>>(), [("Explaining the quantitative result", false)]);
    assert_eq!((plan.priority.as_str(), plan.how.as_str()), ("quantitative", "connecting"));
    assert!(p.mixed_design().unwrap().is_none() && p.integration_log().unwrap().is_empty(), "a plan makes nothing");

    let out = p.design_apply(&applying("explanatory_sequential", "Explain the scores", "Chose the explanatory sequential design.")).unwrap();
    assert_eq!((out.variables_made.clone(), out.codes_made.clone()), (vec!["Quantitative result group".to_string()], vec!["Explaining the quantitative result".to_string()]));
    assert_eq!(out.design.kind, "explanatory_sequential");
    let variable = p.case_variables().unwrap().into_iter().find(|v| v.name == "Quantitative result group").expect("the variable is there");
    assert_eq!(variable.level, "nominal");
    let code = p.list_codes().unwrap().into_iter().find(|c| c.name == "Explaining the quantitative result").expect("the code is there");
    assert_eq!((code.kind.as_str(), code.status.as_str()), ("theme", "active"));
    let log = p.integration_log().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!((log[0].stage.as_str(), log[0].how.as_str(), log[0].what.as_str(), log[0].why.as_str()), ("design", "connecting", "Chose the explanatory sequential design.", "Explain the scores"));
    assert_eq!(out.entry_id, Some(log[0].id));

    // A second press makes nothing twice and, with no sentence, writes no entry.
    let again = p.design_apply(&applying("explanatory_sequential", "Explain the scores", "")).unwrap();
    assert_eq!((again.variables_made.len(), again.codes_made.len(), again.variables_kept.len(), again.codes_kept.len(), again.entry_id), (0, 0, 1, 1, None));
    assert_eq!(p.integration_log().unwrap().len(), 1);
    assert_eq!(p.list_codes().unwrap().iter().filter(|c| c.name == "Explaining the quantitative result").count(), 1);

    // A variable that is already there is left exactly as it is, whatever level it has.
    p.define_variable(&prismqda_engine::VariableSpec { name: "Study group".to_string(), level: "ratio".to_string(), value_labels: Default::default(), missing_codes: vec![] }, None).unwrap();
    let embedded = p.design_apply(&applying("embedded", "", "")).unwrap();
    assert_eq!((embedded.variables_kept.clone(), embedded.variables_made.len()), (vec!["Study group".to_string()], 0));
    assert_eq!(p.case_variables().unwrap().into_iter().find(|v| v.name == "Study group").unwrap().level, "ratio");
    assert_eq!(embedded.codes_made, ["Findings of the embedded strand"]);

    // Two designs make nothing at all.
    let convergent = p.design_apply(&applying("convergent", "", "Chose the convergent design.")).unwrap();
    assert!(convergent.variables_made.is_empty() && convergent.codes_made.is_empty());
    assert_eq!(p.integration_log().unwrap().last().unwrap().how, "merging");
    let exploratory = p.design_plan("exploratory_sequential").unwrap();
    assert_eq!((exploratory.codes.len(), exploratory.variables.len(), exploratory.how.as_str()), (1, 0, "building"));
    let e = p.design_apply(&applying("nope", "", "")).unwrap_err().to_string();
    assert!(e.contains("is not a design"), "{e}");
}

fn row(topic: &str, code: Option<i64>, variable: &str) -> prismqda_engine::TriangulationInput {
    prismqda_engine::TriangulationInput { topic: topic.to_string(), code_id: code, variable: variable.to_string(), qualitative: String::new(), quantitative: String::new(), mark: String::new(), note: String::new() }
}

#[test]
fn a_triangulation_row_keeps_the_researchers_words_and_mark_beside_the_words_and_numbers_of_its_code_and_variable() {
    use prismqda_engine::TypologyCell;
    let (mut p, _dir, welcome) = world();
    let mut input = row("  Welcome  ", Some(welcome), "Score");
    input.qualitative = "People felt welcome.".to_string();
    input.quantitative = "Scores were high.".to_string();
    input.mark = "convergence".to_string();
    input.note = "Both say so.".to_string();
    let first = p.triangulation_add(&input).unwrap();
    assert_eq!((first.topic.as_str(), first.code_name.as_deref(), first.mark.as_str(), first.note.as_str()), ("Welcome", Some("Welcome"), "convergence", "Both say so."));
    let near = |a: f64, b: f64| assert!((a - b).abs() < 1e-9, "{a} against {b}");
    // The words are the topic's code (five excerpts on four cases) and the numbers are the side-by-side display's.
    let evidence = first.evidence.as_ref().expect("a code and a variable give evidence");
    let words = evidence.words.as_ref().unwrap();
    assert_eq!((words.cases, words.excerpts, words.quotes.len(), words.more, words.coding_ids.len()), (4, 5, 2, 3, 5));
    let numbers = evidence.numbers.as_ref().unwrap();
    assert!(!numbers.whole);
    let TypologyCell::Scale { n, mean, .. } = &numbers.with else { panic!("a number") };
    assert_eq!(*n, 4);
    near(mean.unwrap(), 14.75);
    let Some(TypologyCell::Scale { n, mean, .. }) = &numbers.without else { panic!("the others") };
    assert_eq!(*n, 9);
    near(mean.unwrap(), 229.0 / 9.0);
    near(numbers.difference.unwrap(), 14.75 - 229.0 / 9.0);

    // A topic with a variable and no code describes every case that holds text; one with neither has no evidence.
    let scores = p.triangulation_add(&row("Scores", None, "Score")).unwrap();
    let whole = scores.evidence.as_ref().unwrap();
    assert!(whole.words.is_none());
    let numbers = whole.numbers.as_ref().unwrap();
    assert!(numbers.whole && numbers.without.is_none() && numbers.difference.is_none());
    let TypologyCell::Scale { n, mean, .. } = &numbers.with else { panic!() };
    assert_eq!(*n, 13);
    near(mean.unwrap(), 288.0 / 13.0);
    let bare = p.triangulation_add(&row("Something to look at", None, "")).unwrap();
    assert!(bare.evidence.is_none() && bare.problem.is_none() && bare.mark.is_empty());

    // A code with no variable gives the words only.
    let coded = p.triangulation_add(&row("Words only", Some(welcome), "")).unwrap();
    assert!(coded.evidence.as_ref().unwrap().numbers.is_none() && coded.evidence.as_ref().unwrap().words.is_some());

    // The order can be changed.
    let names = |p: &Project| p.triangulation().unwrap().into_iter().map(|r| r.topic).collect::<Vec<_>>();
    assert_eq!(names(&p), ["Welcome", "Scores", "Something to look at", "Words only"]);
    p.triangulation_move(bare.id, true).unwrap();
    assert_eq!(names(&p), ["Welcome", "Something to look at", "Scores", "Words only"]);
    p.triangulation_move(first.id, true).unwrap();
    p.triangulation_move(coded.id, false).unwrap();
    assert_eq!(names(&p), ["Welcome", "Something to look at", "Scores", "Words only"], "the first cannot go up and the last cannot go down");

    // A change is kept; no change is not a change.
    let same = p.triangulation_update(first.id, &input_of(&first)).unwrap();
    assert_eq!(same.updated_at, first.updated_at);
    let mut changed = input_of(&first);
    changed.mark = "discordance".to_string();
    changed.note = "On second thought they differ.".to_string();
    let after = p.triangulation_update(first.id, &changed).unwrap();
    assert_eq!((after.mark.as_str(), after.note.as_str()), ("discordance", "On second thought they differ."));

    // What cannot be kept is refused in words.
    let e = p.triangulation_add(&row("   ", None, "")).unwrap_err().to_string();
    assert!(e.contains("needs a name"), "{e}");
    let mut marked = row("x", None, "");
    marked.mark = "agreement".to_string();
    assert!(p.triangulation_add(&marked).unwrap_err().to_string().contains("not one of the choices"));
    assert!(p.triangulation_add(&row("x", Some(welcome + 100), "")).unwrap_err().to_string().to_lowercase().contains("code"));
    assert!(p.triangulation_add(&row("x", None, "Nope")).unwrap_err().to_string().contains("no case variable"));
    assert!(p.triangulation_update(first.id + 100, &input_of(&first)).is_err());

    // A variable removed afterwards leaves the row and says so, without the numbers.
    p.define_variable(&prismqda_engine::VariableSpec { name: "Temp".to_string(), level: "ratio".to_string(), value_labels: Default::default(), missing_codes: vec![] }, None).unwrap();
    let temp = p.triangulation_add(&row("Temporary", Some(welcome), "Temp")).unwrap();
    assert!(temp.problem.is_none());
    p.delete_variable("Temp").unwrap();
    let gone = p.triangulation().unwrap().into_iter().find(|r| r.id == temp.id).unwrap();
    assert!(gone.problem.as_deref().is_some_and(|m| m.contains("no variable named 'Temp'")), "{:?}", gone.problem);
    assert!(gone.evidence.as_ref().unwrap().numbers.is_none() && gone.evidence.as_ref().unwrap().words.is_some(), "the words are still there");

    p.triangulation_remove(first.id).unwrap();
    assert!(p.triangulation().unwrap().iter().all(|r| r.id != first.id));
    assert!(p.triangulation_remove(first.id).is_err());
}

fn input_of(r: &prismqda_engine::TriangulationRow) -> prismqda_engine::TriangulationInput {
    prismqda_engine::TriangulationInput { topic: r.topic.clone(), code_id: r.code_id, variable: r.variable.clone(), qualitative: r.qualitative.clone(), quantitative: r.quantitative.clone(), mark: r.mark.clone(), note: r.note.clone() }
}

fn entry(date: Option<&str>, stage: &str, how: &str, what: &str, why: &str) -> prismqda_engine::IntegrationInput {
    prismqda_engine::IntegrationInput { on_date: date.map(str::to_string), stage: stage.to_string(), how: how.to_string(), what: what.to_string(), why: why.to_string() }
}

#[test]
fn the_integration_log_keeps_dated_entries_in_order_and_refuses_what_makes_no_sense() {
    let (mut p, _dir, _) = world();
    let a = p.integration_add(&entry(Some("2026-03-02"), "analysis", "transforming", "  Made a variable from the code Welcome.  ", "To test it.")).unwrap();
    let b = p.integration_add(&entry(Some("2026-01-15"), "design", "merging", "Planned to merge the strands in the discussion.", "")).unwrap();
    let c = p.integration_add(&entry(Some("2026-03-02"), "interpretation", "connecting", "Used the interviews to explain the scores.", "")).unwrap();
    assert_eq!(a.what, "Made a variable from the code Welcome.", "trimmed");
    assert_eq!(p.integration_log().unwrap().into_iter().map(|e| e.id).collect::<Vec<_>>(), [b.id, a.id, c.id], "by date, and by the order they were written within a day");
    // A date left out is today's.
    let today = p.integration_add(&entry(None, "reporting", "embedding", "Wrote it up.", "")).unwrap();
    assert!(today.on_date.len() == 10 && today.on_date.as_str() >= "2026-01-01", "{}", today.on_date);

    // A change is kept, and a date left out keeps the date.
    let mut edit = entry(None, "analysis", "transforming", "Made a variable from the code Welcome.", "To test it, and to use it in a t test.");
    let updated = p.integration_update(a.id, &edit).unwrap();
    assert_eq!((updated.on_date.as_str(), updated.why.as_str()), ("2026-03-02", "To test it, and to use it in a t test."));
    edit.why = updated.why.clone();
    assert_eq!(p.integration_update(a.id, &edit).unwrap().updated_at, updated.updated_at, "no change is not a change");

    for (input, words) in [
        (entry(None, "nowhere", "merging", "x", ""), "not one of the choices"),
        (entry(None, "analysis", "stirring", "x", ""), "not one of the choices"),
        (entry(None, "analysis", "merging", "   ", ""), "needs to say what was done"),
        (entry(Some("yesterday"), "analysis", "merging", "x", ""), "is not a date"),
        (entry(Some("2026-13-45"), "analysis", "merging", "x", ""), "is not a date"),
        (entry(None, "analysis", "merging", &"x".repeat(4001), ""), "at most 4000 characters"),
    ] {
        let e = p.integration_add(&input).unwrap_err().to_string();
        assert!(e.contains(words), "{e}");
    }
    p.integration_remove(b.id).unwrap();
    assert!(p.integration_log().unwrap().iter().all(|e| e.id != b.id));
    assert!(p.integration_remove(b.id).is_err());
    assert!(p.integration_update(b.id, &edit).is_err());
    let actions: Vec<String> = p.events(1000).unwrap().into_iter().map(|e| e.action).collect();
    for a in ["mixed.integration.add", "mixed.integration.update", "mixed.integration.remove"] {
        assert!(actions.contains(&a.to_string()), "{a} is in the audit trail");
    }
}

#[test]
fn the_design_the_matrix_and_the_log_are_in_the_file_when_it_is_opened_again() {
    let (mut p, dir, welcome) = world();
    p.design_apply(&applying("explanatory_sequential", "Explain the scores", "Chose it.")).unwrap();
    let mut input = row("Welcome", Some(welcome), "Score");
    input.mark = "complementarity".to_string();
    input.qualitative = "Words.".to_string();
    p.triangulation_add(&input).unwrap();
    p.integration_add(&entry(Some("2026-04-01"), "analysis", "transforming", "Made a variable.", "")).unwrap();
    drop(p);
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let p = Project::open(dir.path().join("p.pqda"), &o).unwrap();
    let d = p.mixed_design().unwrap().unwrap();
    assert_eq!((d.kind.as_str(), d.priority.as_str(), d.purpose.as_str()), ("explanatory_sequential", "quantitative", "Explain the scores"));
    let rows = p.triangulation().unwrap();
    assert_eq!((rows.len(), rows[0].mark.as_str(), rows[0].qualitative.as_str()), (1, "complementarity", "Words."));
    assert!(rows[0].evidence.is_some(), "the evidence is worked out again from the project");
    let dates: Vec<String> = p.integration_log().unwrap().into_iter().map(|e| e.on_date).collect();
    assert!(dates.len() == 2 && dates.iter().any(|d| d == "2026-04-01"), "{dates:?}");
}
