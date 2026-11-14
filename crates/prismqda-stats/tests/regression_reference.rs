//! Linear regression must match R (specification 17.4).
//!
//! `reference/regression_reference.json` is made by `reference/make_regression_reference.R` with base R 4.5.3 (`lm`,
//! `summary.lm`, `confint`, `hatvalues`, `cooks.distance`, `rstandard`, `rstudent`, `anova` for models in steps; the
//! standardized coefficients, variance inflation factors, and the Breusch-Pagan statistic are written out in plain R, as the
//! script says). The data are invented. Everything is compared by relative error 1e-11, which is about what the 15 digits of
//! the reference file allow.

use prismqda_stats::regression::{compare, dummy_columns, linear, RegressionError};
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/regression_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_regression_reference.R to create the reference file")).unwrap()
}

/// A number, or a list of numbers; R's JSON writer gives a single value without the list around it.
fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
        other => vec![other.as_f64().unwrap()],
    }
}

fn columns(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(nums).collect()
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

#[test]
fn every_model_matches_lm() {
    let d = load();
    let mut shapes = std::collections::HashSet::new();
    for m in d["models"].as_array().unwrap() {
        let name = m["name"].as_str().unwrap();
        let e = &m["expected"];
        let (y, x) = (nums(&m["y"]), columns(&m["x"]));
        shapes.insert(x.len());
        let r = linear(&y, &x, 0.95).unwrap_or_else(|err| panic!("{name}: {err:?}"));

        close_all(name, "estimate", &r.terms.iter().map(|t| t.estimate).collect::<Vec<_>>(), &nums(&e["estimate"]));
        close_all(name, "standard error", &r.terms.iter().map(|t| t.se).collect::<Vec<_>>(), &nums(&e["se"]));
        close_all(name, "t", &r.terms.iter().map(|t| t.t).collect::<Vec<_>>(), &nums(&e["t"]));
        close_all(name, "p", &r.terms.iter().map(|t| t.p).collect::<Vec<_>>(), &nums(&e["p"]));
        close_all(name, "interval low", &r.terms.iter().map(|t| t.ci.0).collect::<Vec<_>>(), &nums(&e["ci_lo"]));
        close_all(name, "interval high", &r.terms.iter().map(|t| t.ci.1).collect::<Vec<_>>(), &nums(&e["ci_hi"]));
        close_all(name, "standardized coefficient", &r.terms[1..].iter().map(|t| t.beta.unwrap()).collect::<Vec<_>>(), &nums(&e["beta"]));
        close_all(name, "variance inflation factor", &r.terms[1..].iter().map(|t| t.vif.unwrap()).collect::<Vec<_>>(), &nums(&e["vif"]));
        assert!(r.terms[0].beta.is_none() && r.terms[0].vif.is_none(), "the intercept has neither");

        close(name, "R squared", r.r_squared, nums(&e["r_squared"])[0]);
        close(name, "adjusted R squared", r.adj_r_squared, nums(&e["adj_r_squared"])[0]);
        close(name, "F", r.f.f, nums(&e["f"])[0]);
        close(name, "F df1", r.f.df1, nums(&e["df1"])[0]);
        close(name, "F df2", r.f.df2, nums(&e["df2"])[0]);
        close(name, "F p", r.f.p, nums(&e["f_p"])[0]);
        close(name, "residual standard error", r.sigma, nums(&e["sigma"])[0]);
        close(name, "residual sum of squares", r.rss, nums(&e["rss"])[0]);
        close(name, "total sum of squares", r.tss, nums(&e["tss"])[0]);

        close_all(name, "fitted", &r.fitted, &nums(&e["fitted"]));
        close_all(name, "residuals", &r.residuals, &nums(&e["residuals"]));
        close_all(name, "leverage", &r.leverage, &nums(&e["leverage"]));
        close_all(name, "Cook's distance", &r.cooks_distance, &nums(&e["cooks"]));
        close_all(name, "standardized residuals", &r.standardized_residuals, &nums(&e["standardized"]));
        close_all(name, "studentized residuals", &r.studentized_residuals, &nums(&e["studentized"]));

        let bp = r.breusch_pagan.expect("the Breusch-Pagan test is computed");
        close(name, "Breusch-Pagan statistic", bp.statistic, nums(&e["bp_statistic"])[0]);
        close(name, "Breusch-Pagan df", bp.df, nums(&e["bp_df"])[0]);
        close(name, "Breusch-Pagan p", bp.p, nums(&e["bp_p"])[0]);
    }
    assert!(shapes.contains(&1) && shapes.contains(&2) && shapes.contains(&3), "one, two, and three predictors are all compared: {shapes:?}");
}

#[test]
fn models_entered_in_steps_are_compared_as_anova_does() {
    let d = load();
    let s = &d["steps"];
    let (y, x) = (nums(&s["y"]), columns(&s["x"]));
    let models: Vec<_> = (1..=3).map(|k| linear(&y, &x[..k], 0.95).unwrap()).collect();
    for c in s["changes"].as_array().unwrap() {
        let (from, to) = (c["from"].as_u64().unwrap() as usize - 1, c["to"].as_u64().unwrap() as usize - 1);
        let change = compare(&models[from], &models[to]).unwrap();
        close("steps", "R squared change", change.r2_change, nums(&c["r2_change"])[0]);
        close("steps", "F change", change.f, nums(&c["f"])[0]);
        close("steps", "df1", change.df1, nums(&c["df1"])[0]);
        close("steps", "df2", change.df2, nums(&c["df2"])[0]);
        close("steps", "p of the change", change.p, nums(&c["p"])[0]);
    }
    assert!(compare(&models[1], &models[0]).is_none(), "the larger model must be the second");
}

#[test]
fn a_regression_that_cannot_be_fitted_says_why() {
    let y = [1.0, 2.0, 3.0, 4.0, 5.0, 7.0];
    let x = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    assert_eq!(linear(&y, &[x.to_vec(), x.iter().map(|v| 2.0 * v).collect()], 0.95).unwrap_err(), RegressionError::Collinear(1), "the second predictor repeats the first");
    assert_eq!(linear(&[2.0; 6], &[x.to_vec()], 0.95).unwrap_err(), RegressionError::NoVariationInOutcome);
    assert_eq!(linear(&y[..2], &[x[..2].to_vec()], 0.95).unwrap_err(), RegressionError::TooFewCases, "two cases cannot estimate two parameters with a residual");
    assert_eq!(linear(&y, &[vec![1.0, 2.0]], 0.95).unwrap_err(), RegressionError::NotFinite, "a column of the wrong length");
    assert_eq!(linear(&y, &[vec![1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0]], 0.95).unwrap_err(), RegressionError::NotFinite);
    assert_eq!(linear(&y, &[vec![3.0; 6]], 0.95).unwrap_err(), RegressionError::Collinear(0), "a constant predictor is the intercept again");
}

#[test]
fn a_categorical_predictor_becomes_a_column_for_each_category_but_the_reference() {
    let values: Vec<String> = ["a", "b", "c", "b", "a"].iter().map(|s| s.to_string()).collect();
    let cats: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    let cols = dummy_columns(&values, &cats, "a");
    assert_eq!(cols, vec![vec![0.0, 1.0, 0.0, 1.0, 0.0], vec![0.0, 0.0, 1.0, 0.0, 0.0]]);
}
