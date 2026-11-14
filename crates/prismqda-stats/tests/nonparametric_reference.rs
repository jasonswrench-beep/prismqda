//! The tests of counts and ranks of the 1.0 plan, and partial correlation, must match R (specification 17.4).
//!
//! `reference/nonparametric_reference.json` is made by `reference/make_nonparametric_reference.R` with base R 4.5.3. Where base R
//! has the test (`binom.test`, `mcnemar.test`, `friedman.test`, `chisq.test`, `ks.test`) it is the reference; the runs test, the
//! median test, and partial correlation are written out there from their definitions, and the Moses test's exact distribution
//! is found by listing every arrangement of the ranks.

use prismqda_stats::correlation::partial_correlation;
use prismqda_stats::discrete::{binomial_test, goodness_of_fit, mcnemar, median_test, runs_test, sign_test};
use prismqda_stats::kolmogorov::{ks_normal, ks_two_sample};
use prismqda_stats::nonparametric::{friedman, moses};
use serde_json::Value;

/// R's limiting Kolmogorov distribution is summed only until a term is under 1e-6, and for a statistic under 1 only its first term is
/// kept, which leaves an error of up to about 1.5e-5 in the p-value near a statistic of 1. PrismQDA sums the series fully, so it is
/// the more accurate of the two, and the comparison allows R's error.
const ASYMPTOTIC: f64 = 5e-5;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/nonparametric_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_nonparametric_reference.R to create the reference file")).unwrap()
}

fn floats(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().map(|n| n.as_f64().unwrap()).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap()],
        other => panic!("not numbers: {other}"),
    }
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn close(what: &str, got: f64, want: f64, tol: f64) {
    let e = (got - want).abs() / want.abs().max(1e-300);
    assert!(e <= tol || (got - want).abs() <= 1e-14, "{what}: PrismQDA {got:e}, R {want:e} (relative error {e:e}, tolerance {tol:e})");
}

fn near(what: &str, got: f64, want: f64, tol: f64) {
    assert!((got - want).abs() <= tol, "{what}: PrismQDA {got:e}, R {want:e} (tolerance {tol:e})");
}

#[test]
fn the_binomial_and_sign_tests_match_r() {
    let d = load();
    let rows = d["binomial"].as_array().unwrap();
    assert!(rows.len() >= 20);
    for r in rows {
        let (x, n, p0, level) = (num(&r["x"]) as u64, num(&r["n"]) as u64, num(&r["p0"]), num(&r["level"]));
        let b = binomial_test(x, n, p0, level).unwrap();
        let at = format!("{x} of {n} against {p0} at {level}");
        close(&format!("{at}: p"), b.p, num(&r["p"]), 1e-8);
        close(&format!("{at}: estimate"), b.estimate, num(&r["estimate"]), 1e-14);
        near(&format!("{at}: lower limit"), b.ci.0, num(&r["lo"]), 1e-9);
        near(&format!("{at}: upper limit"), b.ci.1, num(&r["hi"]), 1e-9);
    }
    for s in d["sign"].as_array().unwrap() {
        let x = floats(&s["x"]);
        let t = sign_test(&x, 0.95).unwrap();
        assert_eq!((t.positive, t.negative, t.zeros), (num(&s["positive"]) as u64, num(&s["negative"]) as u64, num(&s["zeros"]) as u64));
        close("sign p", t.p, num(&s["p"]), 1e-8);
        near("sign median", t.median, num(&s["median"]), 1e-12);
    }
    assert!(binomial_test(3, 2, 0.5, 0.95).is_none() && binomial_test(0, 0, 0.5, 0.95).is_none() && binomial_test(1, 2, 1.5, 0.95).is_none());
    assert!(sign_test(&[0.0, 0.0], 0.95).is_none(), "no difference to count");
}

#[test]
fn mcnemar_and_the_one_sample_chi_square_match_r() {
    let d = load();
    for r in d["mcnemar"].as_array().unwrap() {
        let (b, c) = (num(&r["b"]), num(&r["c"]));
        let m = mcnemar(b, c).unwrap();
        close("McNemar chi-square", m.chi2, num(&r["statistic"]), 1e-12);
        close("McNemar p", m.p, num(&r["p"]), 1e-10);
        close("McNemar corrected chi-square", m.chi2_corrected, num(&r["statistic_corrected"]), 1e-12);
        close("McNemar corrected p", m.p_corrected, num(&r["p_corrected"]), 1e-10);
        close("McNemar exact p", m.p_exact, num(&r["p_exact"]), 1e-8);
    }
    assert!(mcnemar(0.0, 0.0).is_none());
    for r in d["gof"].as_array().unwrap() {
        let x = floats(&r["x"]);
        let p = floats(&r["p_expected"]);
        let g = goodness_of_fit(&x, Some(&p)).unwrap();
        close("chi-square", g.chi2, num(&r["statistic"]), 1e-12);
        close("df", g.df, num(&r["df"]), 1e-12);
        close("p", g.p, num(&r["p"]), 1e-9);
        for (a, b) in g.expected.iter().zip(floats(&r["expected"])) {
            close("expected", *a, b, 1e-12);
        }
        for (a, b) in g.residuals.iter().zip(floats(&r["residuals"])) {
            close("residual", *a, b, 1e-12);
        }
    }
    assert!(goodness_of_fit(&[10.0, 10.0], Some(&[0.6, 0.6])).is_none(), "proportions that do not add to one");
    assert!(goodness_of_fit(&[10.0], None).is_none());
    assert!(goodness_of_fit(&[10.0, 12.0], None).is_some());
}

#[test]
fn friedman_and_the_median_test_match_r() {
    let d = load();
    for r in d["friedman"].as_array().unwrap() {
        // The data are written by rows (cases); the test takes columns (measures).
        let rows: Vec<Vec<f64>> = r["data"].as_array().unwrap().iter().map(floats).collect();
        let k = rows[0].len();
        let columns: Vec<Vec<f64>> = (0..k).map(|j| rows.iter().map(|row| row[j]).collect()).collect();
        let f = friedman(&columns).unwrap();
        close("Friedman chi-square", f.chi2, num(&r["statistic"]), 1e-11);
        close("Friedman df", f.df, num(&r["df"]), 1e-12);
        close("Friedman p", f.p, num(&r["p"]), 1e-9);
        close("Kendall's W", f.kendall_w, num(&r["kendall_w"]), 1e-11);
        for (a, b) in f.mean_ranks.iter().zip(floats(&r["mean_ranks"])) {
            close("mean rank", *a, b, 1e-12);
        }
    }
    assert!(friedman(&[vec![1.0, 2.0, 3.0]]).is_none(), "one measure");
    for r in d["median_test"].as_array().unwrap() {
        let groups: Vec<Vec<f64>> = r["groups"].as_array().unwrap().iter().map(floats).collect();
        let m = median_test(&groups).unwrap();
        close("median", m.median, num(&r["median"]), 1e-12);
        assert_eq!(m.above, floats(&r["above"]));
        assert_eq!(m.at_or_below, floats(&r["below"]));
        close("median chi-square", m.chi2, num(&r["statistic"]), 1e-11);
        close("median p", m.p, num(&r["p"]), 1e-9);
        match (&r["fisher_p"], m.fisher_p) {
            (Value::Null, None) => {}
            (v, Some(p)) => close("Fisher p", p, num(v), 1e-9),
            (v, got) => panic!("Fisher p: R {v}, PrismQDA {got:?}"),
        }
    }
}

#[test]
fn the_kolmogorov_smirnov_tests_match_r() {
    let d = load();
    for r in d["ks_one"].as_array().unwrap() {
        let x = floats(&r["x"]);
        let k = ks_normal(&x, 0.0, 1.0).unwrap();
        close("one-sample D", k.d, num(&r["d"]), 1e-12);
        let n = x.len();
        let mut sorted = x.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let ties = sorted.windows(2).any(|w| w[0] == w[1]);
        assert_eq!(k.exact, n < 100 && !ties, "n = {n}: exact only under 100 values with no ties");
        if k.exact {
            close(&format!("one-sample exact p, n = {n}"), k.p, num(&r["p"]), 1e-8);
        } else {
            near(&format!("one-sample asymptotic p, n = {n}"), k.p, num(&r["p"]), ASYMPTOTIC);
        }
    }
    for r in d["ks_two"].as_array().unwrap() {
        let (x, y) = (floats(&r["x"]), floats(&r["y"]));
        let k = ks_two_sample(&x, &y).unwrap();
        close("two-sample D", k.d, num(&r["d"]), 1e-12);
        assert_eq!(k.exact, r["exact"].as_bool().unwrap());
        if k.exact {
            close(&format!("two-sample exact p, {} and {}", x.len(), y.len()), k.p, num(&r["p"]), 1e-8);
        } else {
            near(&format!("two-sample asymptotic p, {} and {}", x.len(), y.len()), k.p, num(&r["p"]), ASYMPTOTIC);
        }
    }
    assert!(ks_normal(&[1.0, 2.0], 0.0, 0.0).is_none() && ks_two_sample(&[], &[1.0]).is_none());
}

#[test]
fn the_runs_test_and_the_moses_test_match_r() {
    let d = load();
    for r in d["runs"].as_array().unwrap() {
        let seq: Vec<bool> = floats(&r["sequence"]).iter().map(|v| *v == 1.0).collect();
        let t = runs_test(&seq).unwrap();
        assert_eq!((t.runs, t.n1, t.n0), (num(&r["runs"]) as u64, num(&r["n1"]) as u64, num(&r["n0"]) as u64));
        close("runs mean", t.mean, num(&r["mean"]), 1e-12);
        close("runs sd", t.sd, num(&r["sd"]), 1e-12);
        near("runs z", t.z, num(&r["z"]), 1e-12);
        close("runs normal p", t.p_normal, num(&r["p_normal"]), 1e-10);
        close("runs exact p", t.p_exact, num(&r["p_exact"]), 1e-9);
    }
    assert!(runs_test(&[true, true, true]).is_none(), "one value only");
    for r in d["moses"].as_array().unwrap() {
        let (c, t) = (floats(&r["control"]), floats(&r["test"]));
        let m = moses(&c, &t, None).unwrap();
        assert_eq!(m.trim as f64, num(&r["h"]));
        assert_eq!(m.span as f64, num(&r["span_h0"]));
        close("Moses p", m.p, num(&r["p_h0"]), 1e-9);
        let key = format!("h{}", m.trim);
        assert_eq!(m.trimmed_span as f64, num(&r[&format!("span_{key}")]));
        close("Moses trimmed p", m.trimmed_p, num(&r[&format!("p_{key}")]), 1e-9);
        assert!(!m.ties);
    }
    assert!(moses(&[1.0, 2.0], &[3.0], None).is_none(), "too few control cases");
}

#[test]
fn partial_correlation_matches_r() {
    let d = load();
    for r in d["partial"].as_array().unwrap() {
        let (x, y) = (floats(&r["x"]), floats(&r["y"]));
        let z: Vec<Vec<f64>> = r["z"].as_array().unwrap().iter().map(floats).collect();
        let p = partial_correlation(&x, &y, &z, 0.95).unwrap();
        assert_eq!(p.n as f64, num(&r["n"]));
        assert_eq!(p.df, num(&r["df"]));
        close("partial r", p.r, num(&r["r"]), 1e-9);
        close("partial t", p.t, num(&r["t"]), 1e-9);
        close("partial p", p.p, num(&r["p"]), 1e-8);
        let (lo, hi) = p.ci.unwrap();
        near("interval lower", lo, num(&r["lo"]), 1e-9);
        near("interval upper", hi, num(&r["hi"]), 1e-9);
    }
    assert!(partial_correlation(&[1.0, 2.0, 3.0], &[1.0, 2.0, 4.0], &[vec![1.0, 1.0, 2.0]], 0.95).is_none(), "too few cases for the degrees of freedom");
}
