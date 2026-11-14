//! The probability distributions must match R (specification 17.4).
//!
//! `reference/distribution_reference.json` is made by `reference/make_distribution_reference.R` with base R 4.5.3. Every
//! value is compared by relative error, because a p-value of 1e-20 must be right to its digits and not only to 1e-15 of
//! one. The reference file holds numbers with 15 significant digits (what R's JSON writer gives), so no tolerance below
//! 1e-13 could be asked of it; the tolerances here are the table in the validation report.

use prismqda_stats::dist::*;
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/distribution_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_distribution_reference.R to create the reference file")).unwrap()
}

fn rows(v: &Value, key: &str) -> Vec<Vec<f64>> {
    v[key].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect()).collect()
}

/// The relative error, with a floor so that a true zero is compared by absolute error.
fn relative(got: f64, want: f64) -> f64 {
    if got == want {
        return 0.0;
    }
    (got - want).abs() / want.abs().max(1e-300)
}

fn check(name: &str, tol: f64, cases: impl Iterator<Item = (String, f64, f64)>) {
    let mut worst = 0.0f64;
    let mut worst_at = String::new();
    let mut n = 0;
    for (at, got, want) in cases {
        n += 1;
        let e = relative(got, want);
        if e > worst {
            worst = e;
            worst_at = format!("{at}: PrismQDA {got:e}, R {want:e}");
        }
    }
    assert!(n > 5, "{name}: only {n} values compared");
    assert!(worst <= tol, "{name}: the worst relative error is {worst:e} (tolerance {tol:e}) at {worst_at}");
}

#[test]
fn log_gamma_matches_r() {
    let d = load();
    check("lgamma", 1e-13, rows(&d, "lgamma").into_iter().map(|r| (format!("x = {}", r[0]), ln_gamma(r[0]), r[1])));
}

#[test]
fn the_normal_matches_r() {
    let d = load();
    check("pnorm lower", 1e-12, rows(&d, "pnorm").into_iter().map(|r| (format!("z = {}", r[0]), norm_cdf(r[0]), r[1])));
    check("pnorm upper", 1e-12, rows(&d, "pnorm").into_iter().map(|r| (format!("z = {}", r[0]), norm_sf(r[0]), r[2])));
    check("qnorm", 1e-12, rows(&d, "qnorm").into_iter().map(|r| (format!("p = {}", r[0]), norm_quantile(r[0]), r[1])));
}

#[test]
fn student_t_matches_r() {
    let d = load();
    check("pt", 1e-11, rows(&d, "pt").into_iter().map(|r| (format!("t = {}, df = {}", r[0], r[1]), t_cdf(r[0], r[1]), r[2])));
    check("two-sided t tail", 1e-11, rows(&d, "pt").into_iter().map(|r| (format!("t = {}, df = {}", r[0], r[1]), t_two_sided(r[0], r[1]), r[3])));
    check("qt", 1e-10, rows(&d, "qt").into_iter().map(|r| (format!("p = {}, df = {}", r[0], r[1]), t_quantile(r[0], r[1]), r[2])));
}

#[test]
fn chi_square_matches_r() {
    let d = load();
    check("pchisq", 1e-11, rows(&d, "pchisq").into_iter().map(|r| (format!("x = {}, df = {}", r[0], r[1]), chisq_cdf(r[0], r[1]), r[2])));
    check("pchisq upper", 1e-11, rows(&d, "pchisq").into_iter().map(|r| (format!("x = {}, df = {}", r[0], r[1]), chisq_sf(r[0], r[1]), r[3])));
    check("qchisq", 1e-10, rows(&d, "qchisq").into_iter().map(|r| (format!("p = {}, df = {}", r[0], r[1]), chisq_quantile(r[0], r[1]), r[2])));
}

#[test]
fn f_matches_r() {
    let d = load();
    check("pf", 1e-11, rows(&d, "pf").into_iter().map(|r| (format!("x = {}, df = {}, {}", r[0], r[1], r[2]), f_cdf(r[0], r[1], r[2]), r[3])));
    check("pf upper", 1e-11, rows(&d, "pf").into_iter().map(|r| (format!("x = {}, df = {}, {}", r[0], r[1], r[2]), f_sf(r[0], r[1], r[2]), r[4])));
    check("qf", 1e-10, rows(&d, "qf").into_iter().map(|r| (format!("p = {}, df = {}, {}", r[0], r[1], r[2]), f_quantile(r[0], r[1], r[2]), r[3])));
}

#[test]
fn undefined_arguments_are_nan_and_the_edges_are_exact() {
    assert!(ln_gamma(0.0).is_nan() && ln_gamma(-1.0).is_nan());
    assert!(norm_quantile(-0.1).is_nan() && norm_quantile(1.1).is_nan());
    assert_eq!(norm_quantile(0.0), f64::NEG_INFINITY);
    assert_eq!(norm_quantile(1.0), f64::INFINITY);
    assert_eq!(norm_quantile(0.5), 0.0);
    assert!(t_cdf(1.0, 0.0).is_nan() && t_quantile(0.9, -2.0).is_nan());
    assert_eq!(t_cdf(f64::INFINITY, 5.0), 1.0);
    assert_eq!(chisq_cdf(0.0, 3.0), 0.0);
    assert_eq!(chisq_sf(0.0, 3.0), 1.0);
    assert_eq!(f_cdf(0.0, 2.0, 3.0), 0.0);
    assert_eq!(f_sf(-1.0, 2.0, 3.0), 1.0);
    assert!(f_quantile(0.5, 0.0, 3.0).is_nan());
}
