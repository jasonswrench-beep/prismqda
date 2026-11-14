//! The numbers behind the charts must match R (specification 17.4).
//!
//! `reference/charts_reference.json` is made by `reference/make_charts_reference.R` with base R 4.5.3. The bin edges follow
//! PrismQDA's rule, which the script writes out a second time in R; the counts in the bins are R's own (`hist`). The box is the
//! type 7 quartiles with the 1.5 interquartile range rule of `boxplot.stats`; the normal probability plot is `qqnorm` with the
//! line of `qqline`. The data are invented.

use prismqda_stats::charts::{box_stats, histogram, least_squares_line, normal_qq, ppoints};
use serde_json::Value;

fn load() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/charts_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_charts_reference.R to create the reference file")).unwrap()
}

/// A number, or a list of numbers (R's JSON writer gives a single value without the list around it).
fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
        Value::Null => Vec::new(),
        other => vec![other.as_f64().unwrap()],
    }
}

fn close(case: &str, what: &str, got: f64, want: f64, tol: f64) {
    let diff = (got - want).abs();
    assert!(diff <= 1e-12 || diff <= tol * want.abs(), "{case}: {what} is {got:e} but R says {want:e}");
}

#[test]
fn the_bins_have_the_edges_of_the_rule_and_the_counts_of_hist() {
    for c in load() {
        let name = c["name"].as_str().unwrap();
        let x = nums(&c["x"]);
        let bins = histogram(&x);
        let edges: Vec<f64> = bins.iter().map(|b| b.lo).chain(bins.last().map(|b| b.hi)).collect();
        assert_eq!(edges, nums(&c["histogram"]["edges"]), "{name}: the edges");
        let counts: Vec<f64> = bins.iter().map(|b| b.count as f64).collect();
        assert_eq!(counts, nums(&c["histogram"]["counts"]), "{name}: the counts");
        assert_eq!(bins.iter().map(|b| b.count).sum::<usize>(), x.len(), "{name}: every value is in a bin");
    }
}

#[test]
fn the_box_is_the_type_7_quartiles_with_whiskers_by_the_1_5_rule() {
    let mut outliers_seen = 0;
    for c in load() {
        let name = c["name"].as_str().unwrap();
        let b = box_stats(&nums(&c["x"])).unwrap();
        let e = &c["box"];
        for (what, got, key) in [("q1", b.q1, "q1"), ("median", b.median, "median"), ("q3", b.q3, "q3"), ("lower whisker", b.lower_whisker, "lower_whisker"), ("upper whisker", b.upper_whisker, "upper_whisker"), ("mean", b.mean, "mean")] {
            close(name, what, got, nums(&e[key])[0], 1e-12);
        }
        assert_eq!(b.outliers, nums(&e["outliers"]), "{name}: the outliers");
        outliers_seen += b.outliers.len();
    }
    assert!(outliers_seen >= 4, "the reference includes outliers on both sides");
}

#[test]
fn the_normal_probability_plot_is_qqnorm_with_the_line_of_qqline() {
    let mut compared = 0;
    for c in load() {
        let name = c["name"].as_str().unwrap();
        let (points, line) = normal_qq(&nums(&c["x"])).unwrap();
        let (theory, sample) = (nums(&c["qq"]["theoretical"]), nums(&c["qq"]["sample"]));
        assert_eq!(points.len(), theory.len(), "{name}");
        for (i, p) in points.iter().enumerate() {
            close(name, &format!("theoretical[{i}]"), p.theoretical, theory[i], 1e-11);
            close(name, &format!("sample[{i}]"), p.sample, sample[i], 1e-12);
            compared += 1;
        }
        close(name, "line slope", line.slope, nums(&c["qq"]["slope"])[0], 1e-11);
        close(name, "line intercept", line.intercept, nums(&c["qq"]["intercept"])[0], 1e-11);
    }
    assert!(compared > 150, "compared {compared} points");
}

#[test]
fn plotting_positions_use_3_8_up_to_ten_values_and_a_half_beyond() {
    let p = ppoints(5);
    assert!((p[0] - (1.0 - 0.375) / (5.0 + 0.25)).abs() < 1e-15);
    let q = ppoints(20);
    assert!((q[0] - 0.5 / 20.0).abs() < 1e-15 && (q[19] - 19.5 / 20.0).abs() < 1e-15);
}

#[test]
fn the_least_squares_line_goes_through_the_means_with_the_right_slope() {
    let line = least_squares_line(&[1.0, 2.0, 3.0, 4.0], &[2.0, 4.0, 5.0, 8.0]).unwrap();
    assert!((line.slope - 1.9).abs() < 1e-12 && (line.intercept - 0.0).abs() < 1e-12);
    assert!(least_squares_line(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]).is_none(), "x does not vary");
    assert!(least_squares_line(&[1.0], &[1.0]).is_none());
    // A missing value takes its pair out.
    let gap = least_squares_line(&[1.0, 2.0, f64::NAN, 4.0], &[2.0, 4.0, 9.0, 8.0]).unwrap();
    assert!((gap.slope - 2.0).abs() < 1e-12);
}

#[test]
fn edge_cases_are_handled() {
    assert!(histogram(&[]).is_empty());
    assert!(histogram(&[f64::NAN]).is_empty());
    assert_eq!(histogram(&[7.0]).len(), 1);
    assert!(box_stats(&[]).is_none());
    assert!(normal_qq(&[1.0, 2.0]).is_none(), "two values are too few for a plot");
    // A value on an edge goes in the bin below it, as hist does, and the lowest value in the first bin: 0 and 10 are both in the first.
    let bins = histogram(&[0.0, 10.0, 20.0, 30.0]);
    assert_eq!(bins.iter().map(|b| b.count).collect::<Vec<_>>(), vec![2, 1, 1]);
    // A sample that is the same everywhere still has a plot, a flat one.
    let (points, line) = normal_qq(&[4.0, 4.0, 4.0, 4.0]).unwrap();
    assert!(points.iter().all(|p| p.sample == 4.0) && line.slope == 0.0 && line.intercept == 4.0);
}
