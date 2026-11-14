//! Describing one variable: counts, centre, spread, shape, and percentiles.
//!
//! The conventions are the common ones and are stated where they matter: the variance divides by n - 1; percentiles are
//! R's default (type 7, linear between order statistics); skewness is the adjusted Fisher-Pearson coefficient (G1) and
//! kurtosis is the excess kurtosis with its small-sample adjustment (G2), as the statistical packages report them.

use crate::dist::{norm_quantile, t_quantile};

/// A percentile of already sorted values, by R's default method (type 7). `p` is between 0 and 1.
pub fn percentile_sorted(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() || !(0.0..=1.0).contains(&p) {
        return None;
    }
    let h = (sorted.len() - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    Some(sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo]))
}

/// The sorted copy of finite values; anything else (missing, infinite) is dropped.
pub fn sorted_finite(x: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

/// The mean of values, or None when there are none.
pub fn mean(x: &[f64]) -> Option<f64> {
    if x.is_empty() {
        None
    } else {
        Some(x.iter().sum::<f64>() / x.len() as f64)
    }
}

/// The sample variance (divisor n - 1), by two passes. None below two values.
pub fn variance(x: &[f64]) -> Option<f64> {
    if x.len() < 2 {
        return None;
    }
    let m = mean(x)?;
    Some(x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (x.len() - 1) as f64)
}

/// What describes one variable.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Descriptives {
    pub n: usize,
    pub sum: f64,
    pub mean: Option<f64>,
    pub variance: Option<f64>,
    pub sd: Option<f64>,
    /// The standard error of the mean.
    pub se: Option<f64>,
    /// The confidence interval of the mean, at `level`, by Student's t.
    pub ci_mean: Option<(f64, f64)>,
    pub level: f64,
    pub min: Option<f64>,
    pub q1: Option<f64>,
    pub median: Option<f64>,
    pub q3: Option<f64>,
    pub max: Option<f64>,
    pub iqr: Option<f64>,
    /// Adjusted Fisher-Pearson skewness (G1), from three values.
    pub skewness: Option<f64>,
    pub se_skewness: Option<f64>,
    /// Excess kurtosis with the small-sample adjustment (G2), from four values.
    pub kurtosis: Option<f64>,
    pub se_kurtosis: Option<f64>,
    /// The values beyond the Tukey fences (1.5 times the interquartile range beyond the quartiles).
    pub outliers: usize,
}

/// Describe the finite values of `x`, with a confidence interval of the mean at `level` (for example 0.95).
pub fn descriptives(x: &[f64], level: f64) -> Descriptives {
    let sorted = sorted_finite(x);
    let n = sorted.len();
    let nf = n as f64;
    let m = mean(&sorted);
    let var = variance(&sorted);
    let sd = var.map(f64::sqrt);
    let se = sd.map(|s| s / nf.sqrt());
    let ci_mean = match (m, se) {
        (Some(m), Some(se)) if n >= 2 && level > 0.0 && level < 1.0 => {
            let q = t_quantile(1.0 - (1.0 - level) / 2.0, nf - 1.0);
            Some((m - q * se, m + q * se))
        }
        _ => None,
    };
    let q1 = percentile_sorted(&sorted, 0.25);
    let q3 = percentile_sorted(&sorted, 0.75);
    let iqr = q1.zip(q3).map(|(a, b)| b - a);
    let outliers = match (q1, q3, iqr) {
        (Some(a), Some(b), Some(i)) => sorted.iter().filter(|&&v| v < a - 1.5 * i || v > b + 1.5 * i).count(),
        _ => 0,
    };
    // Standardized moments need a spread to divide by.
    let (skewness, kurtosis) = match (m, sd) {
        (Some(m), Some(s)) if s > 0.0 => {
            let z3: f64 = sorted.iter().map(|v| ((v - m) / s).powi(3)).sum();
            let z4: f64 = sorted.iter().map(|v| ((v - m) / s).powi(4)).sum();
            let g1 = (n >= 3).then(|| nf / ((nf - 1.0) * (nf - 2.0)) * z3);
            let g2 = (n >= 4).then(|| nf * (nf + 1.0) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0)) * z4 - 3.0 * (nf - 1.0) * (nf - 1.0) / ((nf - 2.0) * (nf - 3.0)));
            (g1, g2)
        }
        _ => (None, None),
    };
    let se_skewness = (n >= 3).then(|| (6.0 * nf * (nf - 1.0) / ((nf - 2.0) * (nf + 1.0) * (nf + 3.0))).sqrt());
    let se_kurtosis = se_skewness.filter(|_| n >= 4).map(|s| 2.0 * s * ((nf * nf - 1.0) / ((nf - 3.0) * (nf + 5.0))).sqrt());
    Descriptives {
        n,
        sum: sorted.iter().sum(),
        mean: m,
        variance: var,
        sd,
        se,
        ci_mean,
        level,
        min: sorted.first().copied(),
        q1,
        median: percentile_sorted(&sorted, 0.5),
        q3,
        max: sorted.last().copied(),
        iqr,
        skewness,
        se_skewness,
        kurtosis,
        se_kurtosis,
        outliers,
    }
}

/// The two-sided critical value of the standard normal at a confidence level (1.96 at 0.95).
pub fn z_critical(level: f64) -> f64 {
    norm_quantile(1.0 - (1.0 - level) / 2.0)
}
