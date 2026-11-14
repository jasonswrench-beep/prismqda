//! Binary logistic regression must match R (specification 17.4).
//!
//! `reference/logistic_reference.json` is made by `reference/make_logistic_reference.R` with base R 4.5.3 (`glm` with the
//! binomial family, `summary`, `confint.default`, `anova` for models in steps, `hatvalues`, `cooks.distance`, `residuals`,
//! `rstandard`; the profile-likelihood intervals, the pseudo R squared measures, the Hosmer-Lemeshow test, the classification
//! table, and the area under the curve are written out in plain R, as the script says). The data are invented. R's own convergence
//! rule is tightened to 1e-14 in the script, so the reference is the maximum likelihood fit to nearly full precision. Everything is
//! compared by relative error 1e-8, except the profile-likelihood limits (1e-7, since both sides find them by root finding).

use prismqda_stats::logistic::{compare, drop_test, logistic};
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/logistic_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_logistic_reference.R to create the reference file")).unwrap()
}

/// A number, or a list of numbers; R's JSON writer gives a single value without the list around it.
fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect(),
        other => vec![other.as_f64().unwrap_or(f64::NAN)],
    }
}

fn columns(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(nums).collect()
}

fn close_with(tol: f64, case: &str, what: &str, got: f64, want: f64) {
    let diff = (got - want).abs();
    assert!(diff <= 1e-10 || diff <= tol * want.abs(), "{case}: {what} is {got:e} but R says {want:e} (relative difference {:e})", diff / want.abs());
}

fn close(case: &str, what: &str, got: f64, want: f64) {
    close_with(1e-8, case, what, got, want);
}

fn close_all(case: &str, what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{case}: {what} has {} values, R has {}", got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        close(case, &format!("{what}[{i}]"), *g, *w);
    }
}

#[test]
fn every_model_matches_glm() {
    let d = load();
    let mut profiled = 0;
    for m in d["models"].as_array().unwrap() {
        let name = m["name"].as_str().unwrap();
        let e = &m["expected"];
        let (y, x) = (nums(&m["y"]), columns(&m["x"]));
        let r = logistic(&y, &x, 0.95, true).unwrap_or_else(|err| panic!("{name}: {err:?}"));

        close_all(name, "estimate", &r.terms.iter().map(|t| t.estimate).collect::<Vec<_>>(), &nums(&e["estimate"]));
        close_all(name, "standard error", &r.terms.iter().map(|t| t.se).collect::<Vec<_>>(), &nums(&e["se"]));
        close_all(name, "z", &r.terms.iter().map(|t| t.z).collect::<Vec<_>>(), &nums(&e["z"]));
        close_all(name, "p", &r.terms.iter().map(|t| t.p).collect::<Vec<_>>(), &nums(&e["p"]));
        close_all(name, "Wald low", &r.terms.iter().map(|t| t.ci.0).collect::<Vec<_>>(), &nums(&e["ci_lo"]));
        close_all(name, "Wald high", &r.terms.iter().map(|t| t.ci.1).collect::<Vec<_>>(), &nums(&e["ci_hi"]));
        for t in &r.terms {
            close(name, "odds ratio", t.odds_ratio, t.estimate.exp());
        }

        // The profile-likelihood limits, exact in R's script, and R's own spline-based interval as a loose check.
        let (plo, phi) = (nums(&e["profile_lo"]), nums(&e["profile_hi"]));
        let (rlo, rhi) = (nums(&e["confint_r_lo"]), nums(&e["confint_r_hi"]));
        assert!(r.terms[0].profile_ci.is_none(), "{name}: the intercept has no profile interval");
        for (j, t) in r.terms.iter().enumerate().skip(1) {
            let (lo, hi) = t.profile_ci.unwrap();
            let (lo, hi) = (lo.unwrap_or_else(|| panic!("{name}: profile low of term {j} missing")), hi.unwrap_or_else(|| panic!("{name}: profile high of term {j} missing")));
            close_with(1e-7, name, &format!("profile low[{j}]"), lo, plo[j]);
            close_with(1e-7, name, &format!("profile high[{j}]"), hi, phi[j]);
            assert!((lo - rlo[j]).abs() <= 0.01 * (1.0 + rlo[j].abs()), "{name}: profile low[{j}] {lo} is far from confint's {}", rlo[j]);
            assert!((hi - rhi[j]).abs() <= 0.01 * (1.0 + rhi[j].abs()), "{name}: profile high[{j}] {hi} is far from confint's {}", rhi[j]);
            let (olo, ohi) = t.or_profile_ci.unwrap();
            close(name, "odds ratio profile low", olo.unwrap(), lo.exp());
            close(name, "odds ratio profile high", ohi.unwrap(), hi.exp());
            profiled += 1;
        }

        close(name, "deviance", r.deviance, nums(&e["deviance"])[0]);
        close(name, "null deviance", r.null_deviance, nums(&e["null_deviance"])[0]);
        close(name, "log likelihood", r.log_likelihood, nums(&e["log_likelihood"])[0]);
        close(name, "AIC", r.aic, nums(&e["aic"])[0]);
        close(name, "BIC", r.bic, nums(&e["bic"])[0]);
        close(name, "LR chi-square", r.lr.chi2, nums(&e["lr_chi2"])[0]);
        close(name, "LR df", r.lr.df, nums(&e["lr_df"])[0]);
        close(name, "LR p", r.lr.p, nums(&e["lr_p"])[0]);
        close(name, "McFadden", r.mcfadden, nums(&e["mcfadden"])[0]);
        close(name, "Cox-Snell", r.cox_snell, nums(&e["cox_snell"])[0]);
        close(name, "Nagelkerke", r.nagelkerke, nums(&e["nagelkerke"])[0]);
        assert_eq!(r.events as f64, nums(&e["events"])[0]);

        let hl = r.hosmer_lemeshow.as_ref().unwrap_or_else(|| panic!("{name}: no Hosmer-Lemeshow test"));
        let want = &e["hosmer_lemeshow"];
        close(name, "Hosmer-Lemeshow chi-square", hl.chi2, nums(&want["chi2"])[0]);
        close(name, "Hosmer-Lemeshow df", hl.df, nums(&want["df"])[0]);
        close(name, "Hosmer-Lemeshow p", hl.p, nums(&want["p"])[0]);
        close_all(name, "Hosmer-Lemeshow group sizes", &hl.groups.iter().map(|g| g.n as f64).collect::<Vec<_>>(), &nums(&want["n"]));
        close_all(name, "Hosmer-Lemeshow observed", &hl.groups.iter().map(|g| g.observed).collect::<Vec<_>>(), &nums(&want["observed"]));
        close_all(name, "Hosmer-Lemeshow expected", &hl.groups.iter().map(|g| g.expected).collect::<Vec<_>>(), &nums(&want["expected"]));

        let c = &r.classification;
        assert_eq!([c.true_positive, c.false_positive, c.true_negative, c.false_negative].map(|v| v as f64), [nums(&e["tp"])[0], nums(&e["fp"])[0], nums(&e["tn"])[0], nums(&e["fn"])[0]], "{name}: classification table");
        close(name, "area under the curve", r.auc, nums(&e["auc"])[0]);

        close_all(name, "fitted", &r.fitted, &nums(&e["fitted"]));
        close_all(name, "leverage", &r.leverage, &nums(&e["leverage"]));
        close_all(name, "Cook's distance", &r.cooks_distance, &nums(&e["cooks"]));
        close_all(name, "Pearson residual", &r.pearson_residuals, &nums(&e["pearson"]));
        close_all(name, "deviance residual", &r.deviance_residuals, &nums(&e["deviance_residuals"]));
        close_all(name, "standardized residual", &r.standardized_residuals, &nums(&e["standardized"]));

        // Leaving a predictor, or the columns of one categorical variable, out.
        for b in e["blocks"].as_array().unwrap() {
            let cols: Vec<usize> = nums(&b["columns"]).iter().map(|c| *c as usize - 1).collect();
            let t = drop_test(&y, &x, r.deviance, &cols).unwrap();
            close(name, &format!("leave out {cols:?}: chi-square"), t.chi2, nums(&b["chi2"])[0]);
            close(name, &format!("leave out {cols:?}: df"), t.df, nums(&b["df"])[0]);
            close(name, &format!("leave out {cols:?}: p"), t.p, nums(&b["p"])[0]);
        }

        // Models in steps.
        for ch in m["changes"].as_array().unwrap() {
            let (small, big) = (nums(&ch["small"])[0] as usize, nums(&ch["big"])[0] as usize);
            let a = logistic(&y, &x[..small], 0.95, false).unwrap();
            let b = logistic(&y, &x[..big], 0.95, false).unwrap();
            let t = compare(&a, &b).unwrap();
            close(name, "change in deviance", t.chi2, nums(&ch["chi2"])[0]);
            close(name, "change df", t.df, nums(&ch["df"])[0]);
            close(name, "change p", t.p, nums(&ch["p"])[0]);
        }
    }
    assert!(profiled >= 8, "the profile intervals were compared for {profiled} coefficients");
}
