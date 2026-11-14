//! The two-way analysis of variance must match R (specification 17.4).
//!
//! `reference/factorial_reference.json` is made by `reference/make_factorial_reference.R` with base R 4.5.3: the sums of squares, F, and p of each
//! effect are R's `drop1(lm(y ~ A * B, contrasts = contr.sum), scope = ~ A + B + A:B, test = "F")` (type III), and the rest is written out in plain
//! R as that script says. The data are invented. Values are compared by relative error 1e-9: the reference file has 15 significant digits, and the
//! two programs reach the same least-squares fit by different routes.

use prismqda_stats::factorial::{two_way, TwoWayError};
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/factorial_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_factorial_reference.R to create the reference file")).unwrap()
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().map(|n| n.as_f64().unwrap()).collect()).unwrap_or_else(|| vec![v.as_f64().unwrap()])
}

fn ints(v: &Value) -> Vec<usize> {
    floats(v).into_iter().map(|n| n as usize).collect()
}

const TOL: f64 = 1e-9;

fn close(case: &str, what: &str, got: f64, want: f64) {
    let diff = (got - want).abs();
    assert!(diff <= 1e-12 || diff <= TOL * want.abs(), "{case}: {what} is {got:e} but R says {want:e} (relative difference {:e})", diff / want.abs());
}

fn all_close(case: &str, what: &str, got: &[f64], want: &Value) {
    let want = floats(want);
    assert_eq!(got.len(), want.len(), "{case}: {what} has {} values but R has {}", got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        close(case, &format!("{what}[{i}]"), *g, *w);
    }
}

#[test]
fn every_case_matches_r() {
    let reference = load();
    for c in reference["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let (y, a, b) = (floats(&c["y"]), ints(&c["a"]), ints(&c["b"]));
        let t = two_way(&y, &a, &b).unwrap_or_else(|e| panic!("{name}: {e:?}"));

        all_close(name, "sum of squares", &t.effects.iter().map(|e| e.ss).collect::<Vec<_>>(), &c["ss"]);
        all_close(name, "df", &t.effects.iter().map(|e| e.df).collect::<Vec<_>>(), &c["df"]);
        all_close(name, "F", &t.effects.iter().map(|e| e.f).collect::<Vec<_>>(), &c["f"]);
        all_close(name, "p", &t.effects.iter().map(|e| e.p).collect::<Vec<_>>(), &c["p"]);
        all_close(name, "partial eta squared", &t.effects.iter().map(|e| e.partial_eta_squared).collect::<Vec<_>>(), &c["partial_eta_squared"]);
        close(name, "error ss", t.error_ss, c["error_ss"].as_f64().unwrap());
        close(name, "error df", t.error_df, c["error_df"].as_f64().unwrap());
        close(name, "error ms", t.error_ms, c["error_ms"].as_f64().unwrap());
        close(name, "total ss", t.total_ss, c["total_ss"].as_f64().unwrap());
        close(name, "R squared", t.r_squared, c["r_squared"].as_f64().unwrap());
        close(name, "adjusted R squared", t.adj_r_squared, c["adj_r_squared"].as_f64().unwrap());
        all_close(name, "cell n", &t.cells.iter().map(|x| x.n as f64).collect::<Vec<_>>(), &c["cell_n"]);
        all_close(name, "cell mean", &t.cells.iter().map(|x| x.mean).collect::<Vec<_>>(), &c["cell_mean"]);
        all_close(name, "cell sd", &t.cells.iter().map(|x| x.sd.unwrap_or(f64::NAN)).collect::<Vec<_>>(), &c["cell_sd"]);
        all_close(name, "margin a mean", &t.margins_a.iter().map(|x| x.mean).collect::<Vec<_>>(), &c["margin_a_mean"]);
        all_close(name, "margin b mean", &t.margins_b.iter().map(|x| x.mean).collect::<Vec<_>>(), &c["margin_b_mean"]);
        all_close(name, "margin a n", &t.margins_a.iter().map(|x| x.n as f64).collect::<Vec<_>>(), &c["margin_a_n"]);
        all_close(name, "margin b n", &t.margins_b.iter().map(|x| x.n as f64).collect::<Vec<_>>(), &c["margin_b_n"]);
        all_close(name, "residuals", &t.residuals, &c["residuals"]);
        let lev = t.levene.as_ref().unwrap_or_else(|| panic!("{name}: Levene's test is missing"));
        close(name, "Levene F", lev.f, c["levene_f"].as_f64().unwrap());
        close(name, "Levene df1", lev.df1, c["levene_df1"].as_f64().unwrap());
        close(name, "Levene df2", lev.df2, c["levene_df2"].as_f64().unwrap());
        close(name, "Levene p", lev.p, c["levene_p"].as_f64().unwrap());
    }
}

#[test]
fn the_order_of_the_cases_does_not_change_the_answer_and_a_swap_of_the_factors_swaps_the_effects() {
    let reference = load();
    let c = &reference["cases"][1];
    let (y, a, b) = (floats(&c["y"]), ints(&c["a"]), ints(&c["b"]));
    let t = two_way(&y, &a, &b).unwrap();
    // The same cases in another order.
    let order: Vec<usize> = (0..y.len()).rev().collect();
    let (y2, a2, b2): (Vec<f64>, Vec<usize>, Vec<usize>) = (order.iter().map(|&i| y[i]).collect(), order.iter().map(|&i| a[i]).collect(), order.iter().map(|&i| b[i]).collect());
    let u = two_way(&y2, &a2, &b2).unwrap();
    for k in 0..3 {
        close("reordered", "ss", u.effects[k].ss, t.effects[k].ss);
        close("reordered", "p", u.effects[k].p, t.effects[k].p);
    }
    // The factors swapped: the first effect is the second's and the interaction is the same.
    let s = two_way(&y, &b, &a).unwrap();
    close("swapped", "first", s.effects[0].ss, t.effects[1].ss);
    close("swapped", "second", s.effects[1].ss, t.effects[0].ss);
    close("swapped", "interaction", s.effects[2].ss, t.effects[2].ss);
}

#[test]
fn a_design_that_cannot_be_analyzed_is_refused_with_the_reason() {
    assert_eq!(two_way(&[1.0, 2.0, 3.0, 4.0, 5.0], &[0, 0, 1, 1, 0], &[0, 0, 0, 0, 1]), Err(TwoWayError::EmptyCell { a: 1, b: 1 }));
}
