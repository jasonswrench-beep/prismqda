//! Cronbach's alpha must match R (specification 17.4).
//!
//! `reference/scale_reference.json` is made by `reference/make_scale_reference.R` with base R 4.5.3. Alpha is written out two ways in R that agree
//! (from the covariance matrix, and Hoyt's `1 - MS(residual) / MS(cases)` of `anova(lm(...))`); the rest is written out in plain R as that script
//! says. The data are invented. Values are compared by relative error 1e-11.

use prismqda_stats::scale::cronbach_alpha;
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/scale_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_scale_reference.R to create the reference file")).unwrap()
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().map(|n| n.as_f64().unwrap_or(f64::NAN)).collect()).unwrap_or_else(|| vec![v.as_f64().unwrap()])
}

const TOL: f64 = 1e-11;

fn close(case: &str, what: &str, got: f64, want: f64) {
    if want.is_nan() {
        assert!(got.is_nan(), "{case}: {what} is {got} but R has no value");
        return;
    }
    let diff = (got - want).abs();
    assert!(diff <= 1e-13 || diff <= TOL * want.abs(), "{case}: {what} is {got:e} but R says {want:e} (relative difference {:e})", diff / want.abs());
}

#[test]
fn every_case_matches_r() {
    let reference = load();
    for c in reference["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let items: Vec<Vec<f64>> = c["items"].as_array().unwrap().iter().map(floats).collect();
        let a = cronbach_alpha(&items, c["level"].as_f64().unwrap()).unwrap_or_else(|| panic!("{name}: no alpha"));
        assert_eq!((a.n, a.k), (c["n"].as_u64().unwrap() as usize, c["k"].as_u64().unwrap() as usize), "{name}");
        close(name, "alpha", a.alpha, c["alpha"].as_f64().unwrap());
        close(name, "alpha (Hoyt's form in R)", a.alpha, c["alpha_hoyt"].as_f64().unwrap());
        close(name, "standardized alpha", a.standardized_alpha.unwrap(), c["standardized_alpha"].as_f64().unwrap());
        close(name, "mean inter-item r", a.mean_inter_item_r.unwrap(), c["mean_inter_item_r"].as_f64().unwrap());
        assert_eq!(a.negative_pairs as u64, c["negative_pairs"].as_u64().unwrap(), "{name}: negative pairs");
        close(name, "total mean", a.total_mean, c["total_mean"].as_f64().unwrap());
        close(name, "total sd", a.total_sd, c["total_sd"].as_f64().unwrap());
        for (j, item) in a.items.iter().enumerate() {
            close(name, &format!("item {j} mean"), item.mean, floats(&c["item_mean"])[j]);
            close(name, &format!("item {j} sd"), item.sd, floats(&c["item_sd"])[j]);
            close(name, &format!("item {j} correlation with the rest"), item.item_rest_r.unwrap(), floats(&c["item_rest_r"])[j]);
            close(name, &format!("alpha without item {j}"), item.alpha_if_deleted.unwrap_or(f64::NAN), floats(&c["alpha_if_deleted"])[j]);
        }
        let ci = a.interval.unwrap_or_else(|| panic!("{name}: no interval"));
        close(name, "lower limit", ci.lower, c["lower"].as_f64().unwrap());
        close(name, "upper limit", ci.upper, c["upper"].as_f64().unwrap());
    }
}
