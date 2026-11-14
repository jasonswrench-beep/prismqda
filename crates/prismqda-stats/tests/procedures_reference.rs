//! Every statistical procedure must match R (specification 17.4).
//!
//! `reference/procedures_reference.json` is made by `reference/make_procedures_reference.R` with base R 4.5.3 (see that
//! script for which values come from an R function and which are written out in plain R). The data are invented. The
//! tolerance table is in `docs/VALIDATION-REPORT.md`; the loose ones are explained where they are used:
//!
//! * most values are compared by relative error 1e-11, because the reference file has 15 significant digits;
//! * the odds ratio and its interval in Fisher's test are compared to 1e-8 with the same calculation in R with the roots
//!   found to 1e-14, and to 3e-2 with `fisher.test` itself, because R finds them with `uniroot` at its default tolerance
//!   (about 1.2e-4), which leaves a large odds ratio good to only a percent or so.

use prismqda_stats::adjust::{p_adjust, Adjustment};
use prismqda_stats::anova::{one_way, pairwise};
use prismqda_stats::contingency::{chi_square, fisher_exact};
use prismqda_stats::correlation::{kendall, pearson, spearman};
use prismqda_stats::describe::descriptives;
use prismqda_stats::nonparametric::{dunn, kruskal_wallis, mann_whitney, wilcoxon};
use prismqda_stats::ttest::{independent, one_sample, paired, TTestKind};
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/procedures_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_procedures_reference.R to create the reference file")).unwrap()
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|n| n.as_f64().unwrap_or(f64::INFINITY)).collect()
}

fn group_list(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(floats).collect()
}

/// Compare by relative error; a value R reports as null (infinite) must be infinite here too.
fn close(case: &str, what: &str, got: f64, want: &Value, tol: f64) {
    let want = want.as_f64().unwrap_or(f64::INFINITY);
    if want.is_infinite() || got.is_infinite() {
        assert_eq!(got, want, "{case}: {what} is {got} but R says {want}");
        return;
    }
    let diff = (got - want).abs();
    assert!(diff <= 1e-14 || diff <= tol * want.abs(), "{case}: {what} is {got:e} but R says {want:e} (relative difference {:e}, tolerance {tol:e})", diff / want.abs());
}

fn eq(case: &str, what: &str, got: f64, e: &Value, key: &str, tol: f64) {
    close(case, &format!("{what} ({key})"), got, &e[key], tol);
}

const TOL: f64 = 1e-11;

#[test]
fn descriptives_match_r() {
    let d = load();
    let mut compared = 0;
    for c in d["descriptives"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let r = descriptives(&floats(&c["x"]), 0.95);
        assert_eq!(r.n as u64, e["n"].as_u64().unwrap(), "{name}: n");
        assert_eq!(r.outliers as u64, e["outliers"].as_u64().unwrap(), "{name}: outliers");
        for (key, got) in [
            ("sum", Some(r.sum)),
            ("mean", r.mean),
            ("variance", r.variance),
            ("sd", r.sd),
            ("se", r.se),
            ("ci_lo", r.ci_mean.map(|c| c.0)),
            ("ci_hi", r.ci_mean.map(|c| c.1)),
            ("min", r.min),
            ("q1", r.q1),
            ("median", r.median),
            ("q3", r.q3),
            ("max", r.max),
            ("iqr", r.iqr),
            ("skewness", r.skewness),
            ("se_skewness", r.se_skewness),
            ("kurtosis", r.kurtosis),
            ("se_kurtosis", r.se_kurtosis),
        ] {
            eq(name, "descriptive", got.unwrap_or_else(|| panic!("{name}: {key} is undefined")), e, key, TOL);
            compared += 1;
        }
    }
    assert!(compared >= 60, "compared {compared}");
    assert!(d["descriptives"].as_array().unwrap().iter().any(|c| c["expected"]["outliers"].as_u64().unwrap() > 0), "the reference must include an outlier");
}

#[test]
fn t_tests_match_r() {
    let d = load();
    let mut kinds = std::collections::HashSet::new();
    for c in d["ttests"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let r = match c["kind"].as_str().unwrap() {
            "one_sample" => one_sample(&floats(&c["x"]), c["mu"].as_f64().unwrap(), 0.95),
            "paired" => paired(&floats(&c["x"]), &floats(&c["y"]), 0.95),
            "welch" => independent(&floats(&c["x"]), &floats(&c["y"]), true, 0.95),
            "student" => independent(&floats(&c["x"]), &floats(&c["y"]), false, 0.95),
            other => panic!("{other}"),
        }
        .unwrap_or_else(|| panic!("{name}: no result"));
        kinds.insert(format!("{:?}", r.kind));
        for (key, got) in [
            ("t", r.t),
            ("df", r.df),
            ("p", r.p),
            ("estimate", r.estimate),
            ("se", r.se),
            ("ci_lo", r.ci.0),
            ("ci_hi", r.ci.1),
            ("d", r.d),
            ("d_lo", r.d_ci.0),
            ("d_hi", r.d_ci.1),
            ("g", r.g),
            ("g_lo", r.g_ci.0),
            ("g_hi", r.g_ci.1),
        ] {
            eq(name, "t test", got, e, key, TOL);
        }
    }
    assert_eq!(kinds.len(), 4, "all four kinds of t test are compared: {kinds:?}");
}

#[test]
fn t_tests_refuse_what_cannot_be_tested() {
    assert!(one_sample(&[1.0], 0.0, 0.95).is_none());
    assert!(one_sample(&[2.0, 2.0, 2.0], 0.0, 0.95).is_none(), "no spread, no t");
    assert!(independent(&[1.0, 2.0], &[3.0], true, 0.95).is_none());
    assert!(independent(&[1.0, 1.0], &[2.0, 2.0], false, 0.95).is_none());
    assert!(paired(&[1.0, 2.0], &[1.0, 2.0, 3.0], 0.95).is_none());
    assert_eq!(independent(&[1.0, 2.0, 4.0], &[3.0, 5.0, 9.0], true, 0.95).unwrap().kind, TTestKind::Welch);
}

#[test]
fn one_way_anova_matches_r() {
    let d = load();
    for c in d["anova"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let a = one_way(&group_list(&c["groups"])).unwrap();
        for (key, got) in [
            ("ss_between", a.ss_between),
            ("ss_within", a.ss_within),
            ("df_between", a.df_between),
            ("df_within", a.df_within),
            ("f", a.test.f),
            ("p", a.test.p),
            ("eta_squared", a.eta_squared),
            ("omega_squared", a.omega_squared),
        ] {
            eq(name, "ANOVA", got, e, key, TOL);
        }
        let w = a.welch.unwrap();
        for (key, got) in [("welch_f", w.f), ("welch_df1", w.df1), ("welch_df2", w.df2), ("welch_p", w.p)] {
            eq(name, "Welch", got, e, key, TOL);
        }
        let l = a.levene.unwrap();
        for (key, got) in [("levene_f", l.f), ("levene_df1", l.df1), ("levene_df2", l.df2), ("levene_p", l.p)] {
            eq(name, "Levene", got, e, key, TOL);
        }
        for (adjustment, key) in [(Adjustment::None, "p_none"), (Adjustment::Bonferroni, "p_bonferroni"), (Adjustment::Holm, "p_holm"), (Adjustment::BenjaminiHochberg, "p_bh")] {
            let pairs = pairwise(&a, adjustment, 0.95);
            let want = floats(&e[key]);
            assert_eq!(pairs.len(), want.len());
            for (p, w) in pairs.iter().zip(&want) {
                close(name, key, p.p_adjusted, &Value::from(*w), TOL);
            }
        }
        let none = pairwise(&a, Adjustment::None, 0.95);
        let bonf = pairwise(&a, Adjustment::Bonferroni, 0.95);
        for (i, want) in e["pairs"].as_array().unwrap().iter().enumerate() {
            assert_eq!((none[i].a as u64, none[i].b as u64), (want["a"].as_u64().unwrap(), want["b"].as_u64().unwrap()));
            eq(name, "pair", none[i].difference, want, "difference", TOL);
            eq(name, "pair", none[i].se, want, "se", TOL);
            eq(name, "pair", none[i].t, want, "t", TOL);
            eq(name, "pair interval", none[i].ci.0, want, "ci_none_lo", TOL);
            eq(name, "pair interval", none[i].ci.1, want, "ci_none_hi", TOL);
            eq(name, "pair interval (Bonferroni)", bonf[i].ci.0, want, "ci_bonf_lo", TOL);
            eq(name, "pair interval (Bonferroni)", bonf[i].ci.1, want, "ci_bonf_hi", TOL);
        }
    }
}

#[test]
fn chi_square_matches_r() {
    let d = load();
    let mut shapes = std::collections::HashSet::new();
    for c in d["chi_square"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let table = group_list(&c["table"]);
        let r = chi_square(&table).unwrap_or_else(|| panic!("{name}: no result"));
        shapes.insert((r.rows, r.cols));
        for (key, got) in [
            ("chi2", r.chi2),
            ("df", r.df),
            ("p", r.p),
            ("g2", r.g2),
            ("g2_p", r.g2_p),
            ("cramers_v", r.cramers_v),
            ("contingency_c", r.contingency_c),
            ("smallest_expected", r.smallest_expected),
            ("share_expected_below_5", r.share_expected_below_5),
        ] {
            eq(name, "chi-square", got, e, key, TOL);
        }
        for (key, got) in [("expected", &r.expected), ("adjusted_residuals", &r.adjusted_residuals), ("standardized_residuals", &r.standardized_residuals)] {
            let want = group_list(&e[key]);
            for (i, row) in got.iter().enumerate() {
                for (j, v) in row.iter().enumerate() {
                    close(name, &format!("{key}[{i}][{j}]"), *v, &Value::from(want[i][j]), TOL);
                }
            }
        }
        if r.rows == 2 && r.cols == 2 {
            let (y, yp) = r.yates.unwrap();
            eq(name, "Yates", y, e, "yates_chi2", TOL);
            eq(name, "Yates", yp, e, "yates_p", TOL);
            eq(name, "phi", r.phi.unwrap(), e, "phi", TOL);
        }
    }
    assert!(shapes.len() >= 3, "{shapes:?}");
}

#[test]
fn chi_square_refuses_what_it_cannot_test() {
    assert!(chi_square(&[vec![1.0, 2.0]]).is_none(), "one row");
    assert!(chi_square(&[vec![1.0, 2.0], vec![3.0]]).is_none(), "ragged");
    assert!(chi_square(&[vec![0.0, 0.0], vec![3.0, 4.0]]).is_none(), "an empty row");
    assert!(chi_square(&[vec![1.0, -2.0], vec![3.0, 4.0]]).is_none(), "a negative count");
}

#[test]
fn fishers_exact_test_matches_r() {
    let d = load();
    for c in d["fisher"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let t: Vec<u64> = c["table"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        let f = fisher_exact(t[0], t[1], t[2], t[3], 0.95).unwrap();
        eq(name, "Fisher", f.p, e, "p", 1e-10);
        // fisher.test finds these with uniroot at its default tolerance, which leaves a large odds ratio or interval end
        // good only to a few thousandths to a percent (6.9e-3 on the "small" table's upper end, 1.2e-2 on the "rare" table's lower end). The R reference file therefore also holds
        // the same calculation with the roots found to 1e-14: PrismQDA must match that one closely, and R's own value
        // within the looser bound.
        for (key, precise, got) in [("odds_ratio", "odds_ratio_precise", f.odds_ratio), ("ci_lo", "ci_lo_precise", f.ci.0), ("ci_hi", "ci_hi_precise", f.ci.1)] {
            if e[key].is_null() || e[key].as_f64() == Some(0.0) {
                // An infinite or zero end is the same in every method.
                close(name, key, got, &e[key], 0.0);
                close(name, precise, got, &e[precise], 0.0);
            } else {
                eq(name, "Fisher", got, e, precise, 1e-8);
                eq(name, "Fisher (R's own value)", got, e, key, 3e-2);
            }
        }
    }
}

#[test]
fn correlations_match_r() {
    let d = load();
    let mut exact_kendall = 0;
    let mut approx_kendall = 0;
    for c in d["correlation"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let (x, y) = (floats(&c["x"]), floats(&c["y"]));
        let p = pearson(&x, &y, 0.95).unwrap();
        eq(name, "Pearson r", p.r, e, "pearson_r", TOL);
        eq(name, "Pearson t", p.statistic.unwrap(), e, "pearson_t", TOL);
        eq(name, "Pearson df", p.df.unwrap(), e, "pearson_df", TOL);
        eq(name, "Pearson p", p.p, e, "pearson_p", TOL);
        eq(name, "Pearson interval", p.ci.unwrap().0, e, "pearson_ci_lo", TOL);
        eq(name, "Pearson interval", p.ci.unwrap().1, e, "pearson_ci_hi", TOL);
        let s = spearman(&x, &y, 0.95).unwrap();
        eq(name, "Spearman rho", s.r, e, "spearman_rho", TOL);
        eq(name, "Spearman p (t approximation)", s.p, e, "spearman_p", TOL);
        let k = kendall(&x, &y, 0.95).unwrap();
        eq(name, "Kendall tau", k.r, e, "kendall_tau", TOL);
        eq(name, "Kendall p", k.p, e, "kendall_p", 1e-10);
        assert_eq!(k.exact, e["kendall_exact"].as_bool().unwrap(), "{name}: exact or approximate Kendall test");
        if k.exact {
            exact_kendall += 1;
        } else {
            approx_kendall += 1;
            eq(name, "Kendall z", k.statistic.unwrap(), e, "kendall_z", TOL);
        }
    }
    assert!(exact_kendall >= 1 && approx_kendall >= 1, "both Kendall paths are compared");
}

#[test]
fn correlations_refuse_what_cannot_be_correlated() {
    assert!(pearson(&[1.0, 2.0], &[3.0, 4.0], 0.95).is_none(), "two pairs");
    assert!(pearson(&[1.0, 1.0, 1.0], &[3.0, 4.0, 5.0], 0.95).is_none(), "a constant variable");
    assert!(spearman(&[1.0, 2.0, 3.0], &[1.0, 2.0], 0.95).is_none(), "different lengths");
    // Pairs with a missing value are set aside, not counted.
    let p = pearson(&[1.0, 2.0, f64::NAN, 4.0, 5.0], &[2.0, 1.0, 9.0, 5.0, 4.0], 0.95).unwrap();
    assert_eq!(p.n, 4);
}

#[test]
fn mann_whitney_matches_r() {
    let d = load();
    let (mut exact, mut approx) = (0, 0);
    for c in d["mann_whitney"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let r = mann_whitney(&floats(&c["x"]), &floats(&c["y"])).unwrap();
        assert_eq!(r.exact, e["exact"].as_bool().unwrap(), "{name}: exact or approximate");
        if r.exact {
            exact += 1;
        } else {
            approx += 1;
        }
        eq(name, "Mann-Whitney U", r.u, e, "w", TOL);
        eq(name, "Mann-Whitney p", r.p, e, "p", 1e-10);
        eq(name, "mean rank 1", r.mean_rank_1, e, "mean_rank_1", TOL);
        eq(name, "mean rank 2", r.mean_rank_2, e, "mean_rank_2", TOL);
        eq(name, "rank-biserial", r.rank_biserial, e, "rank_biserial", TOL);
    }
    assert!(exact >= 1 && approx >= 2, "exact {exact}, approximate {approx}");
}

#[test]
fn wilcoxon_signed_rank_matches_r() {
    let d = load();
    let (mut exact, mut approx) = (0, 0);
    for c in d["wilcoxon"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let x = floats(&c["x"]);
        let y = if c["y"].is_null() { None } else { Some(floats(&c["y"])) };
        let r = wilcoxon(&x, y.as_deref(), c["mu"].as_f64().unwrap()).unwrap();
        assert_eq!(r.exact, e["exact"].as_bool().unwrap(), "{name}: exact or approximate");
        if r.exact {
            exact += 1;
        } else {
            approx += 1;
        }
        assert_eq!(r.n as u64, e["n"].as_u64().unwrap(), "{name}: pairs used");
        assert_eq!(r.zeros as u64, e["zeros"].as_u64().unwrap(), "{name}: zero differences");
        eq(name, "Wilcoxon V", r.v, e, "v", TOL);
        eq(name, "Wilcoxon p", r.p, e, "p", 1e-10);
        eq(name, "rank-biserial", r.rank_biserial, e, "rank_biserial", TOL);
    }
    assert!(exact >= 1 && approx >= 2, "exact {exact}, approximate {approx}");
}

#[test]
fn kruskal_wallis_and_dunn_match_r() {
    let d = load();
    for c in d["kruskal"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let k = kruskal_wallis(&group_list(&c["groups"])).unwrap();
        for (key, got) in [("h", k.h), ("df", k.df), ("p", k.p), ("epsilon_squared", k.epsilon_squared), ("eta_squared_h", k.eta_squared_h)] {
            eq(name, "Kruskal-Wallis", got, e, key, TOL);
        }
        let ranks = floats(&e["mean_ranks"]);
        for (g, want) in k.groups.iter().zip(&ranks) {
            close(name, "mean rank", g.mean_rank, &Value::from(*want), TOL);
        }
        for (adjustment, key) in [(Adjustment::None, "dunn_p"), (Adjustment::Holm, "dunn_p_holm"), (Adjustment::Bonferroni, "dunn_p_bonferroni")] {
            let comparisons = dunn(&k, adjustment);
            let want = floats(&e[key]);
            for (cmp, w) in comparisons.iter().zip(&want) {
                close(name, key, cmp.p_adjusted, &Value::from(*w), TOL);
            }
        }
        let z = floats(&e["dunn_z"]);
        for (cmp, w) in dunn(&k, Adjustment::None).iter().zip(&z) {
            close(name, "Dunn z", cmp.z, &Value::from(*w), TOL);
        }
    }
}

#[test]
fn p_value_adjustments_match_r() {
    let d = load();
    let a = &d["p_adjust"];
    let p = floats(&a["p"]);
    for (adjustment, key) in [(Adjustment::None, "none"), (Adjustment::Bonferroni, "bonferroni"), (Adjustment::Holm, "holm"), (Adjustment::BenjaminiHochberg, "bh")] {
        let got = p_adjust(&p, adjustment);
        for (g, w) in got.iter().zip(floats(&a[key])) {
            close("p_adjust", key, *g, &Value::from(w), 1e-13);
        }
    }
    // A missing p-value stays missing and is not counted as a comparison.
    let with_gap = p_adjust(&[0.01, f64::NAN, 0.04], Adjustment::Bonferroni);
    assert!((with_gap[0] - 0.02).abs() < 1e-15 && with_gap[1].is_nan() && (with_gap[2] - 0.08).abs() < 1e-15);
}
