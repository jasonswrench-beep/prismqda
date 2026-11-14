//! Curve fits must match R (specification 17.4).
//!
//! `reference/curve_reference.json` is made by `reference/make_curve_reference.R` with base R 4.5.3: each curve is R's `lm` of a
//! transformed outcome on transformed predictors (`summary`, `confint`), and the R squared and root mean squared error on the
//! original scale of the outcome are written out in plain R from the curve's predictions. The data are invented. Everything is
//! compared by relative error 1e-11, which is about what the 15 digits of the reference file allow.

use prismqda_stats::curve::{fit, predict, CurveError, CurveKind};
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/curve_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_curve_reference.R to create the reference file")).unwrap()
}

fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
        other => vec![other.as_f64().unwrap()],
    }
}

const TOL: f64 = 1e-11;

fn close(case: &str, what: &str, got: f64, want: f64) {
    let diff = (got - want).abs();
    assert!(diff <= 1e-12 || diff <= TOL * want.abs(), "{case}: {what} is {got:e} but R says {want:e} (relative difference {:e})", diff / want.abs());
}

fn close_all(case: &str, what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{case}: {what} has {} values, R has {}", got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        close(case, &format!("{what}[{i}]"), *g, *w);
    }
}

fn kind_of(name: &str) -> CurveKind {
    serde_json::from_value(Value::String(name.to_string())).unwrap()
}

#[test]
fn every_curve_matches_lm() {
    let d = load();
    let mut compared = std::collections::HashSet::new();
    for m in d["models"].as_array().unwrap() {
        let (x, y) = (nums(&m["x"]), nums(&m["y"]));
        for (name, e) in m["kinds"].as_object().unwrap() {
            let label = format!("{} / {name}", m["name"].as_str().unwrap());
            let kind = kind_of(name);
            compared.insert(kind);
            let r = fit(kind, &x, &y, 0.95).unwrap_or_else(|err| panic!("{label}: {err:?}"));
            let reg = &r.regression;
            assert_eq!(r.n, x.len());
            close_all(&label, "estimate", &reg.terms.iter().map(|t| t.estimate).collect::<Vec<_>>(), &nums(&e["estimate"]));
            close_all(&label, "standard error", &reg.terms.iter().map(|t| t.se).collect::<Vec<_>>(), &nums(&e["se"]));
            close_all(&label, "t", &reg.terms.iter().map(|t| t.t).collect::<Vec<_>>(), &nums(&e["t"]));
            close_all(&label, "p", &reg.terms.iter().map(|t| t.p).collect::<Vec<_>>(), &nums(&e["p"]));
            close_all(&label, "interval low", &reg.terms.iter().map(|t| t.ci.0).collect::<Vec<_>>(), &nums(&e["ci_lo"]));
            close_all(&label, "interval high", &reg.terms.iter().map(|t| t.ci.1).collect::<Vec<_>>(), &nums(&e["ci_hi"]));
            close(&label, "R squared", reg.r_squared, nums(&e["r_squared"])[0]);
            close(&label, "adjusted R squared", reg.adj_r_squared, nums(&e["adj_r_squared"])[0]);
            close(&label, "F", reg.f.f, nums(&e["f"])[0]);
            close(&label, "F df1", reg.f.df1, nums(&e["df1"])[0]);
            close(&label, "F df2", reg.f.df2, nums(&e["df2"])[0]);
            close(&label, "F p", reg.f.p, nums(&e["f_p"])[0]);
            close(&label, "residual standard error", reg.sigma, nums(&e["sigma"])[0]);
            close(&label, "R squared on the original scale", r.r_squared_original, nums(&e["r_squared_original"])[0]);
            close(&label, "root mean squared error on the original scale", r.rmse_original, nums(&e["rmse_original"])[0]);
            let b: Vec<f64> = reg.terms.iter().map(|t| t.estimate).collect();
            let predicted: Vec<f64> = x.iter().map(|xi| predict(kind, &b, *xi)).collect();
            close_all(&label, "predicted", &predicted, &nums(&e["predicted"]));
        }
    }
    assert_eq!(compared.len(), CurveKind::ALL.len(), "every curve is compared: {compared:?}");
}

#[test]
fn a_curve_that_cannot_be_fitted_says_why() {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let y = [2.0, 3.5, 5.1, 6.4, 8.2, 9.9];
    assert_eq!(fit(CurveKind::Logarithmic, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0], &y, 0.95).unwrap_err(), CurveError::NeedsPositiveX);
    assert_eq!(fit(CurveKind::Inverse, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0], &y, 0.95).unwrap_err(), CurveError::NeedsNonzeroX);
    assert_eq!(fit(CurveKind::Power, &x, &[2.0, 3.5, -5.1, 6.4, 8.2, 9.9], 0.95).unwrap_err(), CurveError::NeedsPositiveY);
    assert_eq!(fit(CurveKind::Linear, &x, &[4.0; 6], 0.95).unwrap_err(), CurveError::NoVariationInOutcome);
    assert_eq!(fit(CurveKind::Cubic, &[1.0, 1.0, 2.0, 2.0, 3.0, 3.0], &y, 0.95).unwrap_err(), CurveError::TooFewValues);
    assert_eq!(fit(CurveKind::Quadratic, &x[..3], &y[..3], 0.95).unwrap_err(), CurveError::TooFewCases);
    assert_eq!(fit(CurveKind::Linear, &x, &[1.0, f64::NAN, 3.0, 4.0, 5.0, 6.0], 0.95).unwrap_err(), CurveError::NotFinite);
    // The exponential curve's constants are in the curve's own form: a and b of a e^(b x).
    let ok = fit(CurveKind::Exponential, &x, &y, 0.95).unwrap();
    assert!((ok.constants[0] - ok.regression.terms[0].estimate.exp()).abs() < 1e-12 && ok.constants[1] == ok.regression.terms[1].estimate);
}
