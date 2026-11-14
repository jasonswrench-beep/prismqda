//! The tests of counts and ranks of the 1.0 plan, and partial correlation, as the Statistics page runs them, on a small project
//! whose every expected number was found with R 4.5.3 (`binom.test`, `mcnemar.test`, `friedman.test`, `ks.test`, `chisq.test`,
//! `shapiro.test`, `cor`, and `lm`), so that this checks the wiring of the engine to the numbers, which are compared with R on their
//! own in `prismqda-stats`.
//!
//!   case:    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
//!   Before:  five no and no, six no then yes, two yes then no, three yes and yes (After in the same order, no no / no yes / yes no / yes yes)
//!   Q1:      3  5  4  6  2  7  5  6  4  3  5  6  7  4  5  6
//!   Q2:      Q1 + 1 except case 7, which is 6
//!   Q3:      Q1 - 1
//!   Site:    N  S  N  S ...  (alternating)
//!   Cat:     A for 8 cases, B for 5, C for 3
//!   Sat:     1 1 1 1 1 1 2 2 2 2 2 2 3 3 3 3      Inc: 1 1 1 1 2 2 1 2 2 2 3 3 2 3 3 3

use prismqda_engine::{AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportedCase, ImportedSource, Level, NewProject, OpenOptions, Procedure, Project, RunsCut, SourceType, StatOptions, StatRequest, StatResult, UnitKind};
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
    world_with(|i| if i % 2 == 0 { "N" } else { "S" }.to_string())
}

/// The same study with the Site of each case given by `site`, for spellings that differ.
fn world_with(site: impl Fn(usize) -> String) -> (Project, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let before: Vec<&str> = [vec!["no"; 11], vec!["yes"; 5]].concat();
    let after: Vec<&str> = [vec!["no"; 5], vec!["yes"; 6], vec!["no"; 2], vec!["yes"; 3]].concat();
    let q1 = [3, 5, 4, 6, 2, 7, 5, 6, 4, 3, 5, 6, 7, 4, 5, 6];
    let sat = [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3];
    let inc = [1, 1, 1, 1, 2, 2, 1, 2, 2, 2, 3, 3, 2, 3, 3, 3];
    let x = ["3.1", "5.2", "4.4", "6.9", "7.7", "2.5", "8.8", "5.1", "6", "3.9", "7.2", "9.9", "4.4", "6.6", "8.1", "7.5"];
    let y = ["1.9", "3.4", "2.2", "4.1", "5.5", "1.2", "6.4", "3", "3.8", "2", "4.9", "6.1", "2.6", "4.2", "5.6", "4.7"];
    let z = ["2", "4", "3", "5", "6", "2", "7", "4", "5", "3", "6", "8", "4", "5", "7", "6"];
    let ratio = |n: &str| AttributeSpec::new(n, AppliesTo::Case, Level::Ratio);
    let nominal = |n: &str| AttributeSpec::new(n, AppliesTo::Case, Level::Nominal);
    let ordinal = |n: &str| AttributeSpec::new(n, AppliesTo::Case, Level::Ordinal);
    let batch = ImportBatch {
        sources: (0..16).map(|i| source(&format!("doc{i}"))).collect(),
        cases: (0..16)
            .map(|i| {
                let q2 = if i == 7 { 6 } else { q1[i] + 1 };
                let attrs = vec![
                    ("Before".to_string(), before[i].to_string()),
                    ("After".to_string(), after[i].to_string()),
                    ("Q1".to_string(), q1[i].to_string()),
                    ("Q2".to_string(), q2.to_string()),
                    ("Q3".to_string(), (q1[i] - 1).to_string()),
                    ("Site".to_string(), site(i)),
                    ("Cat".to_string(), if i < 8 { "A" } else if i < 13 { "B" } else { "C" }.to_string()),
                    ("Sat".to_string(), sat[i].to_string()),
                    ("Inc".to_string(), inc[i].to_string()),
                    ("X".to_string(), x[i].to_string()),
                    ("Y".to_string(), y[i].to_string()),
                    ("Z".to_string(), z[i].to_string()),
                ];
                ImportedCase { name: format!("Case {i}"), attrs, members: vec![CaseMember { source: i, unit: None }] }
            })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![nominal("Before"), nominal("After"), ratio("Q1"), ratio("Q2"), ratio("Q3"), nominal("Site"), nominal("Cat"), ordinal("Sat"), ordinal("Inc"), ratio("X"), ratio("Y"), ratio("Z")],
    };
    p.apply_import(&batch).unwrap();
    (p, dir)
}

fn run(p: &Project, procedure: Procedure, variables: &[&str], options: StatOptions) -> prismqda_engine::Result<StatResult> {
    p.run_statistic(&StatRequest { procedure, variables: variables.iter().map(|v| format!("attr:{v}")).collect(), options })
}

fn f(v: &Value, path: &[&str]) -> f64 {
    let mut cur = v;
    for k in path {
        cur = &cur[*k];
    }
    cur.as_f64().unwrap_or_else(|| panic!("no number at {path:?} in {v}"))
}

fn near(what: &str, a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * (1.0 + b.abs()), "{what}: {a} is not {b}");
}

#[test]
fn a_proportion_and_the_counts_of_categories_are_tested() {
    let (p, _dir) = world();
    let o = StatOptions { success: Some("yes".to_string()), ..StatOptions::default() };
    let b = run(&p, Procedure::Binomial, &["Before"], o).unwrap();
    assert_eq!(b.n_used, 16);
    assert_eq!(b.details["success"], "yes");
    near("binomial p", f(&b.details, &["binomial", "p"]), 0.210113525391);
    assert!((b.details["binomial"]["ci"][0].as_f64().unwrap() - 0.1101699547).abs() < 1e-8 && (b.details["binomial"]["ci"][1].as_f64().unwrap() - 0.5866206365).abs() < 1e-8);
    assert!(run(&p, Procedure::Binomial, &["Before"], StatOptions { success: Some("maybe".to_string()), ..StatOptions::default() }).unwrap_err().to_string().contains("not a value"));

    let g = run(&p, Procedure::GoodnessOfFit, &["Cat"], StatOptions::default()).unwrap();
    near("chi-square", f(&g.details, &["goodness_of_fit", "chi2"]), 2.375);
    near("p", f(&g.details, &["goodness_of_fit", "p"]), 0.304982768711);
    assert_eq!(g.details["equal_shares"], true);
    let expected = [("A".to_string(), 50.0), ("B".to_string(), 30.0), ("C".to_string(), 20.0)].into_iter().collect();
    let g = run(&p, Procedure::GoodnessOfFit, &["Cat"], StatOptions { expected: Some(expected), ..StatOptions::default() }).unwrap();
    near("chi-square with shares typed as percentages", f(&g.details, &["goodness_of_fit", "chi2"]), 0.0208333333333);
    near("p", f(&g.details, &["goodness_of_fit", "p"]), 0.989637398915);
    let missing = [("A".to_string(), 1.0)].into_iter().collect();
    assert!(run(&p, Procedure::GoodnessOfFit, &["Cat"], StatOptions { expected: Some(missing), ..StatOptions::default() }).unwrap_err().to_string().contains("no expected share"));
}

#[test]
fn mcnemar_friedman_and_the_sign_test_follow_r() {
    let (p, _dir) = world();
    let m = run(&p, Procedure::Mcnemar, &["Before", "After"], StatOptions::default()).unwrap();
    near("McNemar chi-square", f(&m.details, &["mcnemar", "chi2"]), 2.0);
    near("McNemar p", f(&m.details, &["mcnemar", "p"]), 0.15729920705);
    near("corrected chi-square", f(&m.details, &["mcnemar", "chi2_corrected"]), 1.125);
    near("corrected p", f(&m.details, &["mcnemar", "p_corrected"]), 0.288844366346);
    near("exact p", f(&m.details, &["mcnemar", "p_exact"]), 0.2890625);
    assert!(run(&p, Procedure::Mcnemar, &["Site", "Cat"], StatOptions::default()).unwrap_err().to_string().contains("two answers with the same two values"));

    let fr = run(&p, Procedure::Friedman, &["Q1", "Q2", "Q3"], StatOptions::default()).unwrap();
    near("Friedman chi-square", f(&fr.details, &["friedman", "chi2"]), 31.5238095238);
    near("Friedman p", f(&fr.details, &["friedman", "p"]), 1.42788008893e-07);
    near("mean rank of Q1", fr.details["friedman"]["mean_ranks"][0].as_f64().unwrap(), 2.03125);
    near("mean rank of Q3", fr.details["friedman"]["mean_ranks"][2].as_f64().unwrap(), 1.0);
    assert_eq!(fr.groups.len(), 3, "a median for each measure");
    assert!(run(&p, Procedure::Friedman, &["Q1"], StatOptions::default()).is_err(), "one measure is not a comparison");

    let s = run(&p, Procedure::Sign, &["Q1"], StatOptions { mu: 5.0, ..StatOptions::default() }).unwrap();
    assert_eq!((s.details["sign"]["positive"].as_u64(), s.details["sign"]["negative"].as_u64(), s.details["sign"]["zeros"].as_u64()), (Some(6), Some(6), Some(4)));
    near("sign p", f(&s.details, &["sign", "p"]), 1.0);
    let s2 = run(&p, Procedure::Sign, &["Q2", "Q1"], StatOptions::default()).unwrap();
    assert_eq!((s2.details["sign"]["positive"].as_u64(), s2.details["sign"]["negative"].as_u64(), s2.details["sign"]["zeros"].as_u64()), (Some(15), Some(0), Some(1)));
    near("paired sign p", f(&s2.details, &["sign", "p"]), 2.0 * 0.5f64.powi(15));
}

#[test]
fn the_distribution_tests_compare_groups_and_a_normal_curve() {
    let (p, _dir) = world();
    let k = run(&p, Procedure::KsTwo, &["Q1", "Site"], StatOptions::default()).unwrap();
    near("D", f(&k.details, &["ks", "d"]), 0.5);
    near("p", f(&k.details, &["ks", "p"]), 0.195804195804);
    assert_eq!(k.details["ks"]["exact"], true);
    let m = run(&p, Procedure::MedianTest, &["Q1", "Site"], StatOptions::default()).unwrap();
    near("median", f(&m.details, &["median_test", "median"]), 5.0);
    near("chi-square", f(&m.details, &["median_test", "chi2"]), 4.26666666667);
    near("p", f(&m.details, &["median_test", "p"]), 0.0388671038124);
    let mo = run(&p, Procedure::Moses, &["Q1", "Site"], StatOptions::default()).unwrap();
    assert_eq!(mo.details["moses"]["n_control"], 8);
    assert_eq!(mo.details["control"], "N");
    let mo2 = run(&p, Procedure::Moses, &["Q1", "Site"], StatOptions { control: Some("S".to_string()), ..StatOptions::default() }).unwrap();
    assert_eq!(mo2.details["control"], "S");
    assert!(run(&p, Procedure::KsTwo, &["Q1", "Cat"], StatOptions::default()).unwrap_err().to_string().contains("exactly two"), "three groups are not two");

    let s = run(&p, Procedure::ShapiroWilk, &["Q1"], StatOptions::default()).unwrap();
    near("W", f(&s.details, &["shapiro_wilk", "w"]), 0.945456525914);
    near("p", f(&s.details, &["shapiro_wilk", "p"]), 0.421285677426);
    let n = run(&p, Procedure::KsNormal, &["Q1"], StatOptions { mu: 5.0, sd: Some(1.5), ..StatOptions::default() }).unwrap();
    assert_eq!(n.details["estimated"], false);
    let est = run(&p, Procedure::KsNormal, &["Q1"], StatOptions::default()).unwrap();
    assert_eq!(est.details["estimated"], true);
    assert!(run(&p, Procedure::KsNormal, &["Q1"], StatOptions { sd: Some(0.0), ..StatOptions::default() }).is_err());
}

#[test]
fn runs_ordered_categories_and_partial_correlation_follow_r() {
    let (p, _dir) = world();
    let r = run(&p, Procedure::Runs, &["Q1"], StatOptions::default()).unwrap();
    assert_eq!((r.details["runs"]["runs"].as_u64(), r.details["runs"]["n1"].as_u64(), r.details["runs"]["n0"].as_u64()), (Some(10), Some(10), Some(6)));
    assert_eq!(r.details["cut"].as_f64(), Some(5.0));
    let r2 = run(&p, Procedure::Runs, &["Q1"], StatOptions { cut: Some(RunsCut::Value), mu: 100.0, ..StatOptions::default() });
    assert!(r2.unwrap_err().to_string().contains("same side"));
    let r3 = run(&p, Procedure::Runs, &["Site"], StatOptions::default()).unwrap();
    assert_eq!(r3.details["runs"]["runs"], 16, "N and S alternate, so every case starts a new run");
    assert!(run(&p, Procedure::Runs, &["Cat"], StatOptions::default()).is_err(), "three values are neither a number nor two values");

    let o = run(&p, Procedure::OrdinalAssociation, &["Sat", "Inc"], StatOptions::default()).unwrap();
    near("tau-b", f(&o.details, &["ordinal", "tau_b", "estimate"]), 0.650898968955);
    near("gamma", f(&o.details, &["ordinal", "gamma", "estimate"]), 0.873015873016);
    near("concordant", f(&o.details, &["ordinal", "concordant"]), 59.0);
    near("discordant", f(&o.details, &["ordinal", "discordant"]), 4.0);
    assert!(run(&p, Procedure::OrdinalAssociation, &["Site", "Cat"], StatOptions::default()).unwrap_err().to_string().contains("has to be ordered"));

    let pc = run(&p, Procedure::PartialCorrelation, &["X", "Y", "Z"], StatOptions::default()).unwrap();
    near("partial r", f(&pc.details, &["partial", "r"]), 0.548505099574);
    near("partial t", f(&pc.details, &["partial", "t"]), 2.36520942629);
    near("partial p", f(&pc.details, &["partial", "p"]), 0.034240804627);
    near("zero-order r", f(&pc.details, &["zero_order", "r"]), 0.981198592644);
    assert_eq!(pc.details["controls"][0], "Z");
    assert!(run(&p, Procedure::PartialCorrelation, &["X", "Y"], StatOptions::default()).is_err(), "a control is needed");
}

#[test]
fn the_table_a_script_reads_holds_every_case_with_numbers_as_numbers_and_names_as_names() {
    let (p, _dir) = world();
    let keys: Vec<String> = ["attr:Q1", "attr:Cat", "attr:Site", "attr:Sat", "attr:X"].iter().map(|k| k.to_string()).collect();
    let table = p.stat_table(&keys, true).unwrap();
    assert_eq!(table.cases.len(), 16, "every case, with no rule applied");
    let column = |name: &str| table.columns.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("no column {name}"));
    assert!(column("Q1").numeric && column("X").numeric && column("Sat").numeric, "a scale variable, and an ordered one whose values are all numbers, are numbers");
    assert!(!column("Cat").numeric && !column("Site").numeric, "names stay names");
    assert_eq!(column("Q1").values[..4], [Some("3".to_string()), Some("5".to_string()), Some("4".to_string()), Some("6".to_string())], "a whole number is written without a decimal point");
    assert_eq!(column("X").values[0].as_deref(), Some("3.1"));
    assert_eq!(column("X").values[8].as_deref(), Some("6"), "6 is written as the page writes it");
    assert_eq!(column("Cat").values[0].as_deref(), Some("A"));
    assert_eq!(column("Site").values[1].as_deref(), Some("S"));
    assert!(table.columns.iter().all(|c| c.values.len() == 16), "one value for each case");
    // The order of the columns is the order asked for.
    assert_eq!(table.columns.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["attr:Q1", "attr:Cat", "attr:Site", "attr:Sat", "attr:X"]);
    assert!(p.stat_table(&["attr:Nope".to_string()], true).is_err(), "a variable that is not there is refused");
}

#[test]
fn a_two_way_analysis_of_variance_and_cronbachs_alpha_follow_r() {
    // R 4.5.3: drop1(lm(Q1 ~ Site * Cat, contrasts = contr.sum), scope = ~ Site + Cat + Site:Cat, test = "F"), and alpha written out
    // from cov(), with Feldt's interval and the item statistics (see crates/prismqda-stats/tests/reference/make_scale_reference.R).
    let (p, _dir) = world();
    let t = run(&p, Procedure::TwoWayAnova, &["Q1", "Site", "Cat"], StatOptions::default()).unwrap();
    assert_eq!((t.n_used, t.groups.len()), (16, 6), "six cells, each of which opens its cases");
    assert_eq!((t.groups[0].label.as_str(), t.groups[0].n), ("N \u{d7} A", 4));
    let e = &t.details["factorial"]["effects"];
    near("site ss", e[0]["ss"].as_f64().unwrap(), 0.980392156863);
    near("site F", e[0]["f"].as_f64().unwrap(), 0.539665407447);
    near("site p", e[0]["p"].as_f64().unwrap(), 0.479443623941);
    near("cat ss", e[1]["ss"].as_f64().unwrap(), 0.160919540230);
    near("cat F", e[1]["f"].as_f64().unwrap(), 0.044289781715);
    near("cat p", e[1]["p"].as_f64().unwrap(), 0.956863267443);
    near("interaction ss", e[2]["ss"].as_f64().unwrap(), 9.195402298851);
    near("interaction F", e[2]["f"].as_f64().unwrap(), 2.530844669408);
    near("interaction p", e[2]["p"].as_f64().unwrap(), 0.129012434874);
    near("error ss", f(&t.details, &["factorial", "error_ss"]), 18.166666666667);
    assert_eq!(t.details["factorial"]["error_df"], 10.0);
    assert!(t.assumptions.iter().any(|a| a.id == "sample_size") && t.assumptions.iter().any(|a| a.id == "normality"));
    assert!(!t.assumptions.iter().any(|a| a.id == "equal_variances"), "one cell holds a single case, so its spread cannot be tested");
    assert!(!t.charts.is_empty(), "the cells are drawn");
    assert!(run(&p, Procedure::TwoWayAnova, &["Site", "Q1", "Cat"], StatOptions::default()).unwrap_err().to_string().contains("not a numeric variable"));
    assert!(run(&p, Procedure::TwoWayAnova, &["Q1", "Site"], StatOptions::default()).is_err(), "two groupings are needed");

    let a = run(&p, Procedure::CronbachAlpha, &["Q1", "Q2", "Q3"], StatOptions::default()).unwrap();
    assert_eq!((a.n_used, a.groups.len()), (16, 3));
    near("alpha", f(&a.details, &["alpha", "alpha"]), 0.996651038178);
    near("standardized alpha", f(&a.details, &["alpha", "standardized_alpha"]), 0.996676737800);
    near("mean inter-item r", f(&a.details, &["alpha", "mean_inter_item_r"]), 0.990096040309);
    near("lower", f(&a.details, &["alpha", "interval", "lower"]), 0.992273429728);
    near("upper", f(&a.details, &["alpha", "interval", "upper"]), 0.998733246250);
    let items = &a.details["alpha"]["items"];
    for (j, (rest, without)) in [(0.996357364407, 0.992405063291), (0.985144060464, 1.0), (0.996357364407, 0.992405063291)].iter().enumerate() {
        near("item with the rest", items[j]["item_rest_r"].as_f64().unwrap(), *rest);
        near("alpha without the item", items[j]["alpha_if_deleted"].as_f64().unwrap(), *without);
    }
    assert!(run(&p, Procedure::CronbachAlpha, &["Q1"], StatOptions::default()).unwrap_err().to_string().contains("needs from 2 to"));
    assert!(run(&p, Procedure::CronbachAlpha, &["Q1", "Cat"], StatOptions::default()).is_err(), "an item must be a number");
    // The whole analysis can be bootstrapped, as any other: its numbers are those of the details.
    let boot = StatOptions { bootstrap: Some(prismqda_engine::BootstrapOptions { resamples: 200, seed: 7, analysis: true, cluster: None }), ..StatOptions::default() };
    let b = run(&p, Procedure::CronbachAlpha, &["Q1", "Q2", "Q3"], boot).unwrap();
    assert!(b.details["bootstrap_analysis"]["items"].as_array().is_some_and(|i| !i.is_empty()), "{}", b.details["bootstrap_analysis"]);
    // The two-way analysis can be bootstrapped, but cells of a few cases are often left empty by a resample, and a bootstrap that cannot be run on half of its
    // resamples is refused rather than reported.
    let boot = StatOptions { bootstrap: Some(prismqda_engine::BootstrapOptions { resamples: 1000, seed: 5, analysis: true, cluster: None }), ..StatOptions::default() };
    let refused = run(&p, Procedure::TwoWayAnova, &["Q1", "Site", "Cat"], boot).unwrap_err().to_string();
    assert!(refused.contains("could not be run on"), "{refused}");
}

#[test]
fn values_that_differ_only_by_case_are_one_group_and_never_stop_the_engine() {
    // "N" and "n" are listed once, as the groups of a comparison are, so a case with either is found among the levels (before this it panicked).
    let (spelled, _a) = world_with(|i| if i % 4 == 0 { "N" } else if i % 4 == 2 { "n" } else { "S" }.to_string());
    let (plain, _b) = world();
    for (procedure, variables) in [(Procedure::TwoWayAnova, vec!["Q1", "Site", "Cat"]), (Procedure::ChiSquare, vec!["Site", "Cat"]), (Procedure::OneWayAnova, vec!["Q1", "Site"])] {
        let a = run(&spelled, procedure, &variables, StatOptions::default()).unwrap();
        let b = run(&plain, procedure, &variables, StatOptions::default()).unwrap();
        assert_eq!((a.n_used, a.groups.len()), (b.n_used, b.groups.len()), "{procedure:?}");
    }
    let a = run(&spelled, Procedure::TwoWayAnova, &["Q1", "Site", "Cat"], StatOptions::default()).unwrap();
    let b = run(&plain, Procedure::TwoWayAnova, &["Q1", "Site", "Cat"], StatOptions::default()).unwrap();
    near("site ss", a.details["factorial"]["effects"][0]["ss"].as_f64().unwrap(), b.details["factorial"]["effects"][0]["ss"].as_f64().unwrap());
}
