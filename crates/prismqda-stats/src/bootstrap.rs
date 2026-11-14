//! The bootstrap: how much a statistic would move if the study were repeated, found by drawing cases again and again from the
//! cases there are.
//!
//! A statistic is given as a function of which cases to use. Each resample draws the cases with replacement from a seeded
//! generator (the same seed gives the same intervals on every computer and in every later version), the statistic is worked out
//! on each, and the spread of those values gives the interval. Two kinds of interval are offered:
//!
//! * the **percentile** interval, the middle of the resampled values; and
//! * the **BCa** interval (bias-corrected and accelerated), which corrects the percentile interval for the bias and skew of the
//!   statistic. It needs the statistic's jackknife influence values, so it is offered when one set of cases is resampled and not
//!   when groups are resampled separately.
//!
//! The way an interval is read off the sorted resampled values (interpolating between neighbouring values on the normal scale
//! when the position is not a whole number) is the way R's `boot` package does it, so that given the same resampled values the two
//! agree. That is the comparison `tests/bootstrap_reference.rs` makes. R's own random numbers differ from this crate's, so the
//! resamples themselves cannot be compared with R's; they are tested for what they promise (the same seed repeats, the standard
//! error of a mean matches the formula, and an interval covers the truth about as often as its level says).

use crate::dist::{norm_cdf, norm_quantile};
use crate::rng::SplitMix64;

/// How a bootstrap is run.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BootstrapSpec {
    /// How many resamples to draw (at least 100).
    pub resamples: usize,
    pub seed: u64,
    /// The confidence level of the intervals.
    pub level: f64,
}

/// The intervals of one statistic.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BootstrapInterval {
    /// The statistic on the cases as they are.
    pub estimate: f64,
    /// The mean of the resampled values minus the estimate.
    pub bias: f64,
    /// The standard deviation of the resampled values: the bootstrap standard error.
    pub se: f64,
    /// How many resamples gave a value, and how many gave none (a resample in which the statistic is undefined, for example a
    /// correlation of a variable that happened to be the same in every case drawn).
    pub used: usize,
    pub failures: usize,
    pub level: f64,
    pub percentile: (f64, f64),
    pub bca: Option<(f64, f64)>,
}

/// The value at a position among the sorted resampled values, as R's `boot:::norm.inter` finds it: a whole position takes that
/// value, and anywhere else the two neighbours are joined in proportion on the normal scale. Past either end the extreme value is
/// used.
fn norm_inter(sorted: &[f64], alpha: f64) -> f64 {
    let r = sorted.len();
    let rk = (r as f64 + 1.0) * alpha;
    let k = rk.trunc().max(0.0) as usize;
    if k == 0 {
        return sorted[0];
    }
    if k >= r {
        return sorted[r - 1];
    }
    if k as f64 == rk {
        return sorted[k - 1];
    }
    let (tk, tk1) = (sorted[k - 1], sorted[k]);
    let rr = r as f64 + 1.0;
    let q0 = norm_quantile(alpha);
    let q1 = norm_quantile(k as f64 / rr);
    let q2 = norm_quantile((k as f64 + 1.0) / rr);
    tk + (q0 - q1) / (q2 - q1) * (tk1 - tk)
}

/// The percentile interval of resampled values.
fn percentile_interval(sorted: &[f64], level: f64) -> (f64, f64) {
    // The same arithmetic as R's (1 + c(-conf, conf)) / 2, so the positions agree to the last digit.
    let lo = (1.0 + -level) / 2.0;
    let hi = (1.0 + level) / 2.0;
    (norm_inter(sorted, lo), norm_inter(sorted, hi))
}

/// The BCa interval, from the resampled values, the estimate, and the jackknife influence of each case.
fn bca_interval(sorted: &[f64], estimate: f64, influence: &[f64], level: f64) -> Option<(f64, f64)> {
    // A resample that is the cases in another order gives the estimate again up to the order of the sums, and is not below it, whatever the
    // rounding says; otherwise two programs (or two orders of summing) would count it differently.
    let tie = 1e-9 * estimate.abs().max(1.0);
    let below = sorted.iter().filter(|&&v| v < estimate - tie).count() as f64 / sorted.len() as f64;
    let w = norm_quantile(below);
    if !w.is_finite() {
        return None;
    }
    let sum2: f64 = influence.iter().map(|l| l * l).sum();
    let sum3: f64 = influence.iter().map(|l| l * l * l).sum();
    let a = sum3 / (6.0 * sum2.powf(1.5));
    if !a.is_finite() {
        return None;
    }
    let adjust = |alpha: f64| {
        let z = norm_quantile(alpha);
        norm_cdf(w + (w + z) / (1.0 - a * (w + z)))
    };
    let lo = (1.0 + -level) / 2.0;
    let hi = (1.0 + level) / 2.0;
    Some((norm_inter(sorted, adjust(lo)), norm_inter(sorted, adjust(hi))))
}

/// The intervals from resampled values already in hand. `influence` is the jackknife influence of each case,
/// `(n - 1) * (estimate - the statistic without that case)`, and is needed for the BCa interval. None when there are fewer than
/// 20 usable values.
pub fn intervals_from_replicates(estimate: f64, replicates: &[f64], influence: Option<&[f64]>, level: f64, resamples_asked: usize) -> Option<BootstrapInterval> {
    if !(0.0 < level && level < 1.0) {
        return None;
    }
    let mut sorted: Vec<f64> = replicates.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.len() < 20 {
        return None;
    }
    sorted.sort_by(|a, b| a.total_cmp(b));
    let used = sorted.len();
    let mean = sorted.iter().sum::<f64>() / used as f64;
    let var = sorted.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (used - 1) as f64;
    Some(BootstrapInterval {
        estimate,
        bias: mean - estimate,
        se: var.sqrt(),
        used,
        failures: resamples_asked.saturating_sub(used),
        level,
        percentile: percentile_interval(&sorted, level),
        bca: influence.and_then(|l| bca_interval(&sorted, estimate, l, level)),
    })
}

fn valid(spec: &BootstrapSpec) -> bool {
    (100..=100_000).contains(&spec.resamples) && 0.0 < spec.level && spec.level < 1.0
}

/// The jackknife influence of each of `n` cases: `(n - 1) * (estimate - the statistic without that case)`, which is how much the
/// statistic falls when the case is left out, scaled the way R's `boot::empinf(type = "jack")` scales it. None if the statistic is
/// undefined when any case is left out.
pub fn jackknife_influence<F: Fn(&[usize]) -> Option<f64>>(n: usize, estimate: f64, statistic: F) -> Option<Vec<f64>> {
    let mut without = Vec::with_capacity(n.saturating_sub(1));
    (0..n)
        .map(|i| {
            without.clear();
            without.extend((0..n).filter(|&j| j != i));
            statistic(&without).map(|t| (n as f64 - 1.0) * (estimate - t))
        })
        .collect()
}

/// Bootstraps a statistic of one set of `n` cases. The function is given the indices of the cases to use (a resample has `n`
/// of them, drawn with replacement) and returns the statistic, or None if it is undefined for those cases. Gives percentile and
/// BCa intervals. None when the statistic is undefined for the cases as they are, or for most resamples.
pub fn bootstrap<F: Fn(&[usize]) -> Option<f64>>(n: usize, spec: &BootstrapSpec, statistic: F) -> Option<BootstrapInterval> {
    bootstrap_many(n, 1, spec, |idx| statistic(idx).map(|v| vec![v])).pop().flatten()
}

/// Bootstraps `m` statistics of the same cases from the same resamples (the coefficients of a regression, say), so that one
/// pass of fitting serves them all. The function returns all `m` values, or None when they are undefined for the cases it was
/// given. The result has one entry for each statistic, in order; each is exactly what [`bootstrap`] gives for that statistic alone.
pub fn bootstrap_many<F: Fn(&[usize]) -> Option<Vec<f64>>>(n: usize, m: usize, spec: &BootstrapSpec, statistic: F) -> Vec<Option<BootstrapInterval>> {
    let none = || vec![None; m];
    if n < 3 || m == 0 || !valid(spec) {
        return none();
    }
    let all: Vec<usize> = (0..n).collect();
    let Some(estimates) = statistic(&all).filter(|v| v.len() == m) else { return none() };
    let mut rng = SplitMix64::new(spec.seed);
    let mut replicates: Vec<Vec<f64>> = vec![Vec::with_capacity(spec.resamples); m];
    let mut draw = vec![0usize; n];
    for _ in 0..spec.resamples {
        for slot in draw.iter_mut() {
            *slot = rng.below(n);
        }
        if let Some(values) = statistic(&draw).filter(|v| v.len() == m) {
            for (r, v) in replicates.iter_mut().zip(values) {
                r.push(v);
            }
        }
    }
    // The jackknife influence of each case on each statistic.
    let mut influence: Vec<Vec<f64>> = vec![Vec::with_capacity(n); m];
    let mut without = Vec::with_capacity(n - 1);
    let mut jackknife_ok = true;
    for i in 0..n {
        without.clear();
        without.extend((0..n).filter(|&j| j != i));
        match statistic(&without).filter(|v| v.len() == m) {
            Some(values) => {
                for ((l, e), v) in influence.iter_mut().zip(&estimates).zip(values) {
                    l.push((n as f64 - 1.0) * (e - v));
                }
            }
            None => {
                jackknife_ok = false;
                break;
            }
        }
    }
    (0..m)
        .map(|k| intervals_from_replicates(estimates[k], &replicates[k], jackknife_ok.then_some(influence[k].as_slice()), spec.level, spec.resamples))
        .collect()
}

/// Bootstraps a statistic of several groups of cases, drawing within each group so that every resample has the groups' own sizes
/// (a stratified bootstrap). The function is given, for each group, the indices to use. Gives the percentile interval only.
pub fn bootstrap_groups<F: Fn(&[Vec<usize>]) -> Option<f64>>(sizes: &[usize], spec: &BootstrapSpec, statistic: F) -> Option<BootstrapInterval> {
    if sizes.iter().any(|&n| n < 2) || !valid(spec) {
        return None;
    }
    let all: Vec<Vec<usize>> = sizes.iter().map(|&n| (0..n).collect()).collect();
    let estimate = statistic(&all)?;
    let mut rng = SplitMix64::new(spec.seed);
    let mut replicates = Vec::with_capacity(spec.resamples);
    let mut draw: Vec<Vec<usize>> = sizes.iter().map(|&n| vec![0usize; n]).collect();
    for _ in 0..spec.resamples {
        for (group, &n) in draw.iter_mut().zip(sizes) {
            for slot in group.iter_mut() {
                *slot = rng.below(n);
            }
        }
        if let Some(v) = statistic(&draw) {
            replicates.push(v);
        }
    }
    intervals_from_replicates(estimate, &replicates, None, spec.level, spec.resamples)
}
