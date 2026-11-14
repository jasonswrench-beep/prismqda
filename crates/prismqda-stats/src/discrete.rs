//! Tests on counts and on sequences: the binomial and sign tests, McNemar's test, the one-sample chi-square (goodness of fit),
//! Mood's median test, and the runs test.
//!
//! The binomial test follows R's `binom.test` (the two-sided p-value adds the probabilities of every outcome no more likely than
//! the one seen, with R's tolerance of 1e-7, and the interval is Clopper-Pearson), and the sign test is a binomial test on the
//! signs of the differences.

use crate::contingency::{chi_square, fisher_exact};
use crate::describe::{percentile_sorted, sorted_finite};
use crate::dist::{beta_inc, chisq_sf, ln_choose, norm_cdf};

// ---------------------------------------------------------------- the binomial

/// P(X = x) for a binomial of n trials with chance p.
fn dbinom(x: u64, n: u64, p: f64) -> f64 {
    if p <= 0.0 {
        return if x == 0 { 1.0 } else { 0.0 };
    }
    if p >= 1.0 {
        return if x == n { 1.0 } else { 0.0 };
    }
    (ln_choose(n as f64, x as f64) + x as f64 * p.ln() + (n - x) as f64 * (-p).ln_1p()).exp()
}

/// P(X <= x).
fn pbinom_le(x: i64, n: u64, p: f64) -> f64 {
    if x < 0 {
        return 0.0;
    }
    if x as u64 >= n {
        return 1.0;
    }
    beta_inc(1.0 - p, (n - x as u64) as f64, x as f64 + 1.0)
}

/// P(X >= x).
fn pbinom_ge(x: i64, n: u64, p: f64) -> f64 {
    if x <= 0 {
        return 1.0;
    }
    if x as u64 > n {
        return 0.0;
    }
    beta_inc(p, x as f64, (n - x as u64) as f64 + 1.0)
}

/// The p quantile of the beta distribution with shapes a and b, by bisection on its distribution function.
fn beta_quantile(p: f64, a: f64, b: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid == lo || mid == hi {
            break;
        }
        if beta_inc(mid, a, b) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// An exact test of a proportion.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Binomial {
    pub n: u64,
    pub successes: u64,
    pub p0: f64,
    /// The sample proportion.
    pub estimate: f64,
    /// The two-sided exact p-value of the proportion being `p0`.
    pub p: f64,
    pub level: f64,
    /// The Clopper-Pearson interval of the proportion.
    pub ci: (f64, f64),
}

/// The exact binomial test of `successes` out of `n` against the proportion `p0`, with the Clopper-Pearson interval at `level`.
/// None for no trials, more successes than trials, or a `p0` outside 0 to 1.
pub fn binomial_test(successes: u64, n: u64, p0: f64, level: f64) -> Option<Binomial> {
    if n == 0 || successes > n || !(0.0..=1.0).contains(&p0) || !(level > 0.0 && level < 1.0) {
        return None;
    }
    let x = successes;
    let p = if p0 == 0.0 {
        if x == 0 {
            1.0
        } else {
            0.0
        }
    } else if p0 == 1.0 {
        if x == n {
            1.0
        } else {
            0.0
        }
    } else {
        let rel_err = 1.0 + 1e-7;
        let m = n as f64 * p0;
        let d = dbinom(x, n, p0);
        if x as f64 == m {
            1.0
        } else if (x as f64) < m {
            let y = (m.ceil() as u64..=n).filter(|&i| dbinom(i, n, p0) <= d * rel_err).count() as i64;
            pbinom_le(x as i64, n, p0) + pbinom_ge(n as i64 - y + 1, n, p0)
        } else {
            let y = (0..=m.floor() as u64).filter(|&i| dbinom(i, n, p0) <= d * rel_err).count() as i64;
            pbinom_le(y - 1, n, p0) + pbinom_ge(x as i64, n, p0)
        }
        .min(1.0)
    };
    let alpha = (1.0 - level) / 2.0;
    let lo = if x == 0 { 0.0 } else { beta_quantile(alpha, x as f64, (n - x) as f64 + 1.0) };
    let hi = if x == n { 1.0 } else { beta_quantile(1.0 - alpha, x as f64 + 1.0, (n - x) as f64) };
    Some(Binomial { n, successes, p0, estimate: x as f64 / n as f64, p, level, ci: (lo, hi) })
}

/// The sign test: whether the differences are as often above zero as below it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SignTest {
    pub positive: u64,
    pub negative: u64,
    /// Differences of exactly zero, which are set aside.
    pub zeros: u64,
    pub median: f64,
    pub p: f64,
    /// The share of the non-zero differences that are positive, with its Clopper-Pearson interval.
    pub proportion_positive: f64,
    pub ci: (f64, f64),
    pub level: f64,
}

/// The sign test of the differences (zeros set aside), by the exact binomial test of the signs. None when no difference is
/// non-zero.
pub fn sign_test(differences: &[f64], level: f64) -> Option<SignTest> {
    let d = sorted_finite(differences);
    let positive = d.iter().filter(|v| **v > 0.0).count() as u64;
    let negative = d.iter().filter(|v| **v < 0.0).count() as u64;
    let zeros = d.len() as u64 - positive - negative;
    let b = binomial_test(positive, positive + negative, 0.5, level)?;
    Some(SignTest { positive, negative, zeros, median: percentile_sorted(&d, 0.5)?, p: b.p, proportion_positive: b.estimate, ci: b.ci, level })
}

// ---------------------------------------------------------------- McNemar

/// McNemar's test of two paired yes-or-no answers, from the counts of the cases that changed: `b` went one way, `c` the other.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct McNemar {
    pub b: f64,
    pub c: f64,
    /// Chi-square without a continuity correction, and its p-value.
    pub chi2: f64,
    pub p: f64,
    /// Chi-square with the continuity correction (R's default), and its p-value.
    pub chi2_corrected: f64,
    pub p_corrected: f64,
    /// The exact two-sided p-value (a binomial test of b out of b + c at one half), best with few changed cases.
    pub p_exact: f64,
}

/// McNemar's test. None when no case changed.
pub fn mcnemar(b: f64, c: f64) -> Option<McNemar> {
    if !(b >= 0.0 && c >= 0.0) || b + c == 0.0 {
        return None;
    }
    let s = b + c;
    let chi2 = (b - c).powi(2) / s;
    let correction = if b != c { 1.0 } else { 0.0 };
    let chi2_corrected = ((b - c).abs() - correction).powi(2) / s;
    let p_exact = binomial_test(b as u64, s as u64, 0.5, 0.95)?.p;
    Some(McNemar { b, c, chi2, p: chisq_sf(chi2, 1.0), chi2_corrected, p_corrected: chisq_sf(chi2_corrected, 1.0), p_exact })
}

// ---------------------------------------------------------------- one-sample chi-square

/// The one-sample chi-square test: whether counts in categories follow given proportions.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct GoodnessOfFit {
    pub n: f64,
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
    pub expected: Vec<f64>,
    /// (observed - expected) / sqrt(expected), category by category.
    pub residuals: Vec<f64>,
    pub smallest_expected: f64,
}

/// The chi-square test that the counts follow `proportions` (equal shares when None). The proportions must add up to 1.
pub fn goodness_of_fit(observed: &[f64], proportions: Option<&[f64]>) -> Option<GoodnessOfFit> {
    let k = observed.len();
    if k < 2 || observed.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    let n: f64 = observed.iter().sum();
    if n <= 0.0 {
        return None;
    }
    let p: Vec<f64> = match proportions {
        Some(p) if p.len() == k && p.iter().all(|v| *v > 0.0 && v.is_finite()) && (p.iter().sum::<f64>() - 1.0).abs() < 1e-8 => p.to_vec(),
        Some(_) => return None,
        None => vec![1.0 / k as f64; k],
    };
    let expected: Vec<f64> = p.iter().map(|q| q * n).collect();
    let chi2: f64 = observed.iter().zip(&expected).map(|(o, e)| (o - e).powi(2) / e).sum();
    let df = (k - 1) as f64;
    let residuals = observed.iter().zip(&expected).map(|(o, e)| (o - e) / e.sqrt()).collect();
    let smallest_expected = expected.iter().copied().fold(f64::INFINITY, f64::min);
    Some(GoodnessOfFit { n, chi2, df, p: chisq_sf(chi2, df), expected, residuals, smallest_expected })
}

// ---------------------------------------------------------------- Mood's median test

/// The median test: whether groups have the same median, by counting the cases above the pooled median in each.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MedianTest {
    pub median: f64,
    /// For each group, the cases above the pooled median, and the cases at or below it.
    pub above: Vec<f64>,
    pub at_or_below: Vec<f64>,
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
    /// For two groups, the exact (Fisher) p-value of the same two-by-two table.
    pub fisher_p: Option<f64>,
    pub smallest_expected: f64,
}

/// The median test of two or more groups. None when a group is empty, or every case is above (or none above) the median.
pub fn median_test(groups: &[Vec<f64>]) -> Option<MedianTest> {
    if groups.len() < 2 || groups.iter().any(|g| g.iter().all(|v| !v.is_finite())) {
        return None;
    }
    let all: Vec<f64> = sorted_finite(&groups.iter().flatten().copied().collect::<Vec<_>>());
    let median = percentile_sorted(&all, 0.5)?;
    let above: Vec<f64> = groups.iter().map(|g| g.iter().filter(|v| v.is_finite() && **v > median).count() as f64).collect();
    let at_or_below: Vec<f64> = groups.iter().map(|g| g.iter().filter(|v| v.is_finite() && **v <= median).count() as f64).collect();
    if above.iter().sum::<f64>() == 0.0 || at_or_below.iter().sum::<f64>() == 0.0 {
        return None;
    }
    let table = vec![above.clone(), at_or_below.clone()];
    let c = chi_square(&table)?;
    let fisher_p = (groups.len() == 2).then(|| fisher_exact(above[0] as u64, above[1] as u64, at_or_below[0] as u64, at_or_below[1] as u64, 0.95).map(|f| f.p)).flatten();
    Some(MedianTest { median, above, at_or_below, chi2: c.chi2, df: c.df, p: c.p, fisher_p, smallest_expected: c.smallest_expected })
}

// ---------------------------------------------------------------- the runs test

/// The Wald-Wolfowitz runs test: whether a two-valued sequence changes value more or less often than a random order would.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct RunsTest {
    /// How many runs (unbroken stretches of the same value) there are.
    pub runs: u64,
    pub n1: u64,
    pub n0: u64,
    /// The mean and standard deviation of the number of runs in a random order.
    pub mean: f64,
    pub sd: f64,
    /// The normal approximation with a continuity correction, and its two-sided p-value.
    pub z: f64,
    pub p_normal: f64,
    /// The exact two-sided p-value, from the distribution of the number of runs (twice the smaller tail).
    pub p_exact: f64,
}

/// The runs test of a sequence of two values. None when the sequence has fewer than 2 cases or only one of the values.
pub fn runs_test(sequence: &[bool]) -> Option<RunsTest> {
    let n = sequence.len() as u64;
    let n1 = sequence.iter().filter(|v| **v).count() as u64;
    let n0 = n - n1;
    if n < 2 || n1 == 0 || n0 == 0 {
        return None;
    }
    let runs = 1 + sequence.windows(2).filter(|w| w[0] != w[1]).count() as u64;
    let (n1f, n0f, nf) = (n1 as f64, n0 as f64, n as f64);
    let mean = 2.0 * n1f * n0f / nf + 1.0;
    let sd = (2.0 * n1f * n0f * (2.0 * n1f * n0f - nf) / (nf * nf * (nf - 1.0))).sqrt();
    let correction = if runs as f64 > mean {
        -0.5
    } else if (runs as f64) < mean {
        0.5
    } else {
        0.0
    };
    let z = if sd > 0.0 { (runs as f64 - mean + correction) / sd } else { 0.0 };
    // The probability of exactly r runs.
    let ln_total = ln_choose(nf, n1f);
    // A binomial coefficient is zero when the lower number is out of range.
    let lc = |top: f64, k: f64| if k < 0.0 || k > top { f64::NEG_INFINITY } else { ln_choose(top, k) };
    let density = |r: u64| -> f64 {
        let (a, b) = (n1f - 1.0, n0f - 1.0);
        if r.is_multiple_of(2) {
            let k = (r / 2) as f64;
            (std::f64::consts::LN_2 + lc(a, k - 1.0) + lc(b, k - 1.0) - ln_total).exp()
        } else {
            let k = ((r - 1) / 2) as f64;
            ((lc(a, k) + lc(b, k - 1.0) - ln_total).exp()) + ((lc(a, k - 1.0) + lc(b, k) - ln_total).exp())
        }
    };
    let max_runs = n.min(2 * n1.min(n0) + 1);
    let (mut lower, mut upper) = (0.0, 0.0);
    for r in 2..=max_runs {
        let d = density(r);
        if r <= runs {
            lower += d;
        }
        if r >= runs {
            upper += d;
        }
    }
    Some(RunsTest { runs, n1, n0, mean, sd, z, p_normal: 2.0 * norm_cdf(-z.abs()), p_exact: (2.0 * lower.min(upper)).min(1.0) })
}
