//! Student's t tests: one sample, two independent samples (Student's pooled test and Welch's), and paired samples.
//!
//! All tests here are two-sided. Each comes with the effect size that belongs to it: Cohen's d (for two groups the mean
//! difference over the pooled standard deviation; for one sample or pairs the mean difference over the standard
//! deviation of the differences), its small-sample correction Hedges' g, and an approximate interval for d from the
//! large-sample variance of Hedges and Olkin (1985). The interval is approximate and says so in the report; the exact
//! interval needs the noncentral t distribution.

use crate::describe::{mean, variance, z_critical};
use crate::dist::{ln_gamma, t_quantile, t_two_sided};
use crate::noncentral::noncentral_t_interval;

/// Which test was run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TTestKind {
    OneSample,
    Student,
    Welch,
    Paired,
}

/// One group's size, mean, and standard deviation.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct GroupSummary {
    pub n: usize,
    pub mean: f64,
    pub sd: f64,
}

/// The result of a t test.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TTest {
    pub kind: TTestKind,
    pub t: f64,
    pub df: f64,
    /// The two-sided p-value.
    pub p: f64,
    /// The mean (one sample), the difference of the means (two groups), or the mean difference (pairs).
    pub estimate: f64,
    pub se: f64,
    pub level: f64,
    pub ci: (f64, f64),
    pub groups: Vec<GroupSummary>,
    /// Cohen's d, and Hedges' g, with an exact interval for each (from the noncentral t distribution).
    pub d: f64,
    pub d_ci: (f64, f64),
    pub g: f64,
    pub g_ci: (f64, f64),
}

/// Hedges' correction factor J(df) = Gamma(df/2) / (sqrt(df/2) Gamma((df - 1)/2)).
pub fn hedges_j(df: f64) -> f64 {
    if df <= 1.0 {
        return f64::NAN;
    }
    (ln_gamma(df / 2.0) - ln_gamma((df - 1.0) / 2.0)).exp() / (df / 2.0).sqrt()
}

fn summary(x: &[f64]) -> Option<GroupSummary> {
    Some(GroupSummary { n: x.len(), mean: mean(x)?, sd: variance(x)?.sqrt() })
}

/// Cohen's d, Hedges' g, and an exact interval for each (from the noncentral t distribution). `scale` turns a noncentrality
/// parameter into a d: 1 / sqrt(n) for one sample or pairs, sqrt(1/n1 + 1/n2) for two groups, and `df` is the degrees of
/// freedom of the pooled t. When the interval cannot be solved the large-sample normal interval (from `variance_d`) is used.
fn effect(d: f64, variance_d: f64, df: f64, scale: f64, level: f64) -> (f64, (f64, f64), f64, (f64, f64)) {
    let j = hedges_j(df);
    let g = d * j;
    let interval = noncentral_t_interval(d / scale, df, level).map(|(lo, hi)| (lo * scale, hi * scale)).unwrap_or_else(|| {
        let z = z_critical(level);
        let se = variance_d.sqrt();
        (d - z * se, d + z * se)
    });
    // Hedges' g is d shrunk by the same factor, and so is its interval.
    (d, interval, g, (interval.0 * j, interval.1 * j))
}

/// A one-sample t test of the mean of `x` against `mu`. None below two values or when every value is the same.
pub fn one_sample(x: &[f64], mu: f64, level: f64) -> Option<TTest> {
    let s = summary(x)?;
    if s.sd == 0.0 {
        return None;
    }
    let n = s.n as f64;
    let se = s.sd / n.sqrt();
    let df = n - 1.0;
    let t = (s.mean - mu) / se;
    let q = t_quantile(1.0 - (1.0 - level) / 2.0, df);
    let d = (s.mean - mu) / s.sd;
    let (d, d_ci, g, g_ci) = effect(d, 1.0 / n + d * d / (2.0 * n), df, 1.0 / n.sqrt(), level);
    Some(TTest { kind: TTestKind::OneSample, t, df, p: t_two_sided(t, df), estimate: s.mean, se, level, ci: (s.mean - q * se, s.mean + q * se), groups: vec![s], d, d_ci, g, g_ci })
}

/// A paired t test: the mean of the differences x - y against zero. The two must be the same length.
pub fn paired(x: &[f64], y: &[f64], level: f64) -> Option<TTest> {
    if x.len() != y.len() {
        return None;
    }
    let diffs: Vec<f64> = x.iter().zip(y).map(|(a, b)| a - b).collect();
    let s = summary(&diffs)?;
    if s.sd == 0.0 {
        return None;
    }
    let n = s.n as f64;
    let se = s.sd / n.sqrt();
    let df = n - 1.0;
    let t = s.mean / se;
    let q = t_quantile(1.0 - (1.0 - level) / 2.0, df);
    let d = s.mean / s.sd;
    let (d, d_ci, g, g_ci) = effect(d, 1.0 / n + d * d / (2.0 * n), df, 1.0 / n.sqrt(), level);
    Some(TTest { kind: TTestKind::Paired, t, df, p: t_two_sided(t, df), estimate: s.mean, se, level, ci: (s.mean - q * se, s.mean + q * se), groups: vec![s], d, d_ci, g, g_ci })
}

/// A t test of two independent samples: Welch's when `welch` is true (unequal variances allowed), Student's pooled test
/// otherwise. None when a group has fewer than two values or both groups are constant.
pub fn independent(x: &[f64], y: &[f64], welch: bool, level: f64) -> Option<TTest> {
    let a = summary(x)?;
    let b = summary(y)?;
    let (n1, n2) = (a.n as f64, b.n as f64);
    let (v1, v2) = (a.sd * a.sd, b.sd * b.sd);
    if v1 == 0.0 && v2 == 0.0 {
        return None;
    }
    let pooled = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0);
    let diff = a.mean - b.mean;
    let (se, df) = if welch {
        let se2 = v1 / n1 + v2 / n2;
        let df = se2 * se2 / ((v1 / n1).powi(2) / (n1 - 1.0) + (v2 / n2).powi(2) / (n2 - 1.0));
        (se2.sqrt(), df)
    } else {
        ((pooled * (1.0 / n1 + 1.0 / n2)).sqrt(), n1 + n2 - 2.0)
    };
    let t = diff / se;
    let q = t_quantile(1.0 - (1.0 - level) / 2.0, df);
    // Cohen's d uses the pooled standard deviation whichever test was run, so it can be compared across studies.
    let d = diff / pooled.sqrt();
    let (d, d_ci, g, g_ci) = effect(d, (n1 + n2) / (n1 * n2) + d * d / (2.0 * (n1 + n2)), n1 + n2 - 2.0, (1.0 / n1 + 1.0 / n2).sqrt(), level);
    Some(TTest {
        kind: if welch { TTestKind::Welch } else { TTestKind::Student },
        t,
        df,
        p: t_two_sided(t, df),
        estimate: diff,
        se,
        level,
        ci: (diff - q * se, diff + q * se),
        groups: vec![a, b],
        d,
        d_ci,
        g,
        g_ci,
    })
}
