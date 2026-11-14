//! Rank-based tests that do not assume a normal distribution: Mann-Whitney U, Wilcoxon signed-rank, and Kruskal-Wallis
//! with Dunn's comparisons.
//!
//! The choices of R's `wilcox.test` and `kruskal.test` are followed: the exact p-value is used when both samples have
//! fewer than 50 values and there are no ties (and, for the signed-rank test, no zero differences); otherwise a normal
//! approximation with a continuity correction and the correction for ties is used.

use crate::adjust::{p_adjust, Adjustment};
use crate::dist::{chisq_sf, ln_choose, norm_cdf};
use crate::rank::{rank_average, tie_sizes};

/// The Mann-Whitney U test (the Wilcoxon rank-sum test) of two independent samples.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MannWhitney {
    pub n1: usize,
    pub n2: usize,
    /// R's W: the sum of the first sample's ranks minus n1 (n1 + 1) / 2.
    pub u: f64,
    pub mean_rank_1: f64,
    pub mean_rank_2: f64,
    /// The normal-approximation z, or None when the exact test was used.
    pub z: Option<f64>,
    pub p: f64,
    pub exact: bool,
    /// The rank-biserial correlation, from -1 (every value of the first sample is below the second) to 1.
    pub rank_biserial: f64,
    /// The chance that a value of the first sample is above one of the second (ties count half).
    pub probability_of_superiority: f64,
}

/// The sign of a number, with zero as zero (Rust's own `signum` calls zero positive).
fn sign(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else {
        x.signum()
    }
}

/// How many ways each U from 0 to m n can arise from m and n values with no ties, as probabilities.
fn rank_sum_distribution(m: usize, n: usize) -> Vec<f64> {
    let len = m * n + 1;
    let mut poly = vec![0i128; len];
    poly[0] = 1;
    // The generating function is the Gaussian binomial: the product over i of (1 - q^(n + i)) / (1 - q^i).
    for i in 1..=m {
        for k in (n + i..len).rev() {
            poly[k] -= poly[k - (n + i)];
        }
        for k in i..len {
            poly[k] += poly[k - i];
        }
    }
    let total: f64 = poly.iter().map(|&c| c as f64).sum();
    poly.into_iter().map(|c| c as f64 / total).collect()
}

/// The Mann-Whitney U test of `x` against `y`, two-sided. None when either sample is empty or every value is equal.
pub fn mann_whitney(x: &[f64], y: &[f64]) -> Option<MannWhitney> {
    let (n1, n2) = (x.len(), y.len());
    if n1 == 0 || n2 == 0 {
        return None;
    }
    let all: Vec<f64> = x.iter().chain(y).copied().collect();
    let ranks = rank_average(&all);
    let sum1: f64 = ranks[..n1].iter().sum();
    let sum2: f64 = ranks[n1..].iter().sum();
    let (n1f, n2f) = (n1 as f64, n2 as f64);
    let u = sum1 - n1f * (n1f + 1.0) / 2.0;
    let ties = tie_sizes(&all);
    let exact = n1 < 50 && n2 < 50 && ties.is_empty();
    let (p, z) = if exact {
        let dist = rank_sum_distribution(n1, n2);
        let at_most = |k: f64| -> f64 {
            if k < 0.0 {
                0.0
            } else {
                dist.iter().take(k as usize + 1).sum::<f64>().min(1.0)
            }
        };
        let tail = if u > n1f * n2f / 2.0 { 1.0 - at_most(u - 1.0) } else { at_most(u) };
        ((2.0 * tail).min(1.0), None)
    } else {
        let big_n = n1f + n2f;
        let tie_term: f64 = ties.iter().map(|&t| (t as f64).powi(3) - t as f64).sum();
        let sigma = (n1f * n2f / 12.0 * ((big_n + 1.0) - tie_term / (big_n * (big_n - 1.0)))).sqrt();
        if sigma == 0.0 {
            return None;
        }
        let centred = u - n1f * n2f / 2.0;
        let z = (centred - 0.5 * sign(centred)) / sigma;
        ((2.0 * norm_cdf(z).min(norm_cdf(-z))).min(1.0), Some(z))
    };
    Some(MannWhitney {
        n1,
        n2,
        u,
        mean_rank_1: sum1 / n1f,
        mean_rank_2: sum2 / n2f,
        z,
        p,
        exact,
        rank_biserial: 2.0 * u / (n1f * n2f) - 1.0,
        probability_of_superiority: u / (n1f * n2f),
    })
}

/// The Wilcoxon signed-rank test.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Wilcoxon {
    /// The pairs with a difference (zero differences are set aside).
    pub n: usize,
    pub zeros: usize,
    /// R's V: the sum of the ranks of the positive differences.
    pub v: f64,
    pub z: Option<f64>,
    pub p: f64,
    pub exact: bool,
    /// The matched-pairs rank-biserial correlation: (positive rank sum - negative rank sum) / total rank sum.
    pub rank_biserial: f64,
}

/// The Wilcoxon signed-rank test of the differences `x - y` (paired) or of `x - mu` (one sample, pass `y = None`).
pub fn wilcoxon(x: &[f64], y: Option<&[f64]>, mu: f64) -> Option<Wilcoxon> {
    let diffs: Vec<f64> = match y {
        Some(y) if y.len() == x.len() => x.iter().zip(y).map(|(a, b)| a - b - mu).collect(),
        Some(_) => return None,
        None => x.iter().map(|a| a - mu).collect(),
    };
    let zeros = diffs.iter().filter(|&&d| d == 0.0).count();
    let nonzero: Vec<f64> = diffs.into_iter().filter(|&d| d != 0.0).collect();
    let n = nonzero.len();
    if n == 0 {
        return None;
    }
    let abs: Vec<f64> = nonzero.iter().map(|d| d.abs()).collect();
    let ranks = rank_average(&abs);
    let v: f64 = nonzero.iter().zip(&ranks).filter(|(d, _)| **d > 0.0).map(|(_, r)| r).sum();
    let nf = n as f64;
    let total = nf * (nf + 1.0) / 2.0;
    let ties = tie_sizes(&abs);
    let exact = n < 50 && ties.is_empty() && zeros == 0;
    let (p, z) = if exact {
        // The number of ways each sum 0..total arises as the sum of a subset of 1..=n.
        let mut counts = vec![0u128; total as usize + 1];
        counts[0] = 1;
        for i in 1..=n {
            for s in (i..counts.len()).rev() {
                counts[s] += counts[s - i];
            }
        }
        let all = 2f64.powi(n as i32);
        let at_most = |k: f64| -> f64 {
            if k < 0.0 {
                0.0
            } else {
                counts.iter().take(k as usize + 1).map(|&c| c as f64).sum::<f64>() / all
            }
        };
        let tail = if v > total / 2.0 { 1.0 - at_most(v - 1.0) } else { at_most(v) };
        ((2.0 * tail).min(1.0), None)
    } else {
        let tie_term: f64 = ties.iter().map(|&t| (t as f64).powi(3) - t as f64).sum();
        let sigma = (nf * (nf + 1.0) * (2.0 * nf + 1.0) / 24.0 - tie_term / 48.0).sqrt();
        if sigma == 0.0 {
            return None;
        }
        let centred = v - total / 2.0;
        let z = (centred - 0.5 * sign(centred)) / sigma;
        ((2.0 * norm_cdf(z).min(norm_cdf(-z))).min(1.0), Some(z))
    };
    Some(Wilcoxon { n, zeros, v, z, p, exact, rank_biserial: (2.0 * v - total) / total })
}

/// One group's size and mean rank in a Kruskal-Wallis test.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct RankedGroup {
    pub n: usize,
    pub mean_rank: f64,
}

/// The Kruskal-Wallis rank-sum test of several independent groups.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct KruskalWallis {
    pub n: usize,
    pub groups: Vec<RankedGroup>,
    /// The statistic with the correction for ties, compared with chi-square on `df` degrees of freedom.
    pub h: f64,
    pub df: f64,
    pub p: f64,
    /// Epsilon squared, H / (N - 1): the share of rank variability the groups account for.
    pub epsilon_squared: f64,
    /// Eta squared based on H: (H - k + 1) / (N - k).
    pub eta_squared_h: f64,
    /// The size of every group of tied values over all the data, for the tie corrections.
    #[serde(skip)]
    ties: Vec<usize>,
}

/// The Kruskal-Wallis test. None with fewer than two groups, an empty group, or every value equal.
pub fn kruskal_wallis(groups: &[Vec<f64>]) -> Option<KruskalWallis> {
    let k = groups.len();
    if k < 2 || groups.iter().any(|g| g.is_empty()) {
        return None;
    }
    let all: Vec<f64> = groups.iter().flatten().copied().collect();
    let n = all.len();
    let nf = n as f64;
    let ranks = rank_average(&all);
    let mut start = 0;
    let mut ranked = Vec::with_capacity(k);
    let mut sum_sq = 0.0;
    for g in groups {
        let sum: f64 = ranks[start..start + g.len()].iter().sum();
        sum_sq += sum * sum / g.len() as f64;
        ranked.push(RankedGroup { n: g.len(), mean_rank: sum / g.len() as f64 });
        start += g.len();
    }
    let ties = tie_sizes(&all);
    let tie_term: f64 = ties.iter().map(|&t| (t as f64).powi(3) - t as f64).sum();
    let correction = 1.0 - tie_term / (nf * nf * nf - nf);
    if correction == 0.0 {
        return None;
    }
    let h = (12.0 * sum_sq / (nf * (nf + 1.0)) - 3.0 * (nf + 1.0)) / correction;
    let df = (k - 1) as f64;
    Some(KruskalWallis { n, groups: ranked, h, df, p: chisq_sf(h, df), epsilon_squared: h / (nf - 1.0), eta_squared_h: (h - k as f64 + 1.0) / (nf - k as f64), ties })
}

/// One comparison of two groups by Dunn's test after a Kruskal-Wallis test.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Dunn {
    pub a: usize,
    pub b: usize,
    /// Mean rank of group a minus mean rank of group b.
    pub rank_difference: f64,
    pub z: f64,
    pub p: f64,
    pub p_adjusted: f64,
}

/// Dunn's pairwise comparisons of mean ranks, with the correction for ties and the p-values adjusted as chosen.
pub fn dunn(kw: &KruskalWallis, adjustment: Adjustment) -> Vec<Dunn> {
    let nf = kw.n as f64;
    let tie_term: f64 = kw.ties.iter().map(|&t| (t as f64).powi(3) - t as f64).sum();
    let base = nf * (nf + 1.0) / 12.0 - tie_term / (12.0 * (nf - 1.0));
    let mut out = Vec::new();
    for a in 0..kw.groups.len() {
        for b in (a + 1)..kw.groups.len() {
            let (ga, gb) = (kw.groups[a], kw.groups[b]);
            let rank_difference = ga.mean_rank - gb.mean_rank;
            let z = rank_difference / (base * (1.0 / ga.n as f64 + 1.0 / gb.n as f64)).sqrt();
            out.push(Dunn { a, b, rank_difference, z, p: 2.0 * norm_cdf(-z.abs()), p_adjusted: f64::NAN });
        }
    }
    let adjusted = p_adjust(&out.iter().map(|d| d.p).collect::<Vec<_>>(), adjustment);
    for (d, adj) in out.iter_mut().zip(adjusted) {
        d.p_adjusted = adj;
    }
    out
}

// ---------------------------------------------------------------- Friedman's test

/// Friedman's test of several measures on the same cases (the rank-based counterpart of repeated measures).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Friedman {
    /// The cases, and the measures.
    pub n: usize,
    pub k: usize,
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
    /// Kendall's W, the agreement of the cases' rankings from 0 to 1 (chi-square over n (k - 1)).
    pub kendall_w: f64,
    /// The mean of the ranks each measure gets across the cases.
    pub mean_ranks: Vec<f64>,
}

/// Friedman's test of `columns` (one for each measure, every column as long as the number of cases). Cases that lack any of the
/// values should be removed first. The statistic is corrected for ties as R's `friedman.test` does. None under 2 cases or 2 measures.
pub fn friedman(columns: &[Vec<f64>]) -> Option<Friedman> {
    let k = columns.len();
    let n = columns.first()?.len();
    if k < 2 || n < 2 || columns.iter().any(|c| c.len() != n || c.iter().any(|v| !v.is_finite())) {
        return None;
    }
    let mut rank_sums = vec![0.0; k];
    let mut tie_term = 0.0;
    for case in 0..n {
        let row: Vec<f64> = columns.iter().map(|c| c[case]).collect();
        let ranks = rank_average(&row);
        for (sum, r) in rank_sums.iter_mut().zip(&ranks) {
            *sum += r;
        }
        tie_term += tie_sizes(&row).iter().map(|&u| (u * u * u - u) as f64).sum::<f64>();
    }
    let (nf, kf) = (n as f64, k as f64);
    let numerator = 12.0 * rank_sums.iter().map(|s| (s - nf * (kf + 1.0) / 2.0).powi(2)).sum::<f64>();
    let denominator = nf * kf * (kf + 1.0) - tie_term / (kf - 1.0);
    if denominator <= 0.0 {
        return None;
    }
    let chi2 = numerator / denominator;
    let df = kf - 1.0;
    Some(Friedman { n, k, chi2, df, p: chisq_sf(chi2, df), kendall_w: chi2 / (nf * df), mean_ranks: rank_sums.iter().map(|s| s / nf).collect() })
}

// ---------------------------------------------------------------- the Moses extreme reaction test

/// The Moses test of extreme reactions: whether the cases of a test group are more spread out than those of a control group, judged
/// by how far apart the control group's ranks are in the pooled order (its span). A small span means the test group is more extreme
/// on both sides. The p-value is the exact probability of a span at most as large, among all arrangements of the ranks.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Moses {
    pub n_control: usize,
    pub n_test: usize,
    /// The span of all the control cases, and the one-sided exact p-value of a span so small.
    pub span: usize,
    pub p: f64,
    /// How many control cases are trimmed from each end, the span of what is left, and its one-sided exact p-value.
    pub trim: usize,
    pub trimmed_span: usize,
    pub trimmed_p: f64,
    /// True when two cases have the same value across the groups, which the exact distribution does not allow for.
    pub ties: bool,
}

/// P(span <= s) when `nk` kept control cases are placed among `total` positions with `trim` more at each end (the trimmed ones).
fn span_distribution_upto(total: usize, n_control: usize, trim: usize, s: usize) -> f64 {
    let kept = n_control - 2 * trim;
    if kept < 2 {
        return f64::NAN;
    }
    let ln = |n: usize, k: usize| if k > n { f64::NEG_INFINITY } else { ln_choose(n as f64, k as f64) };
    let ln_all = ln(total, n_control);
    let mut p = 0.0;
    // The extremes of the kept cases sit at positions a and a + g - 1; the g - 2 positions between hold the other kept cases.
    for g in kept..=s.min(total - 2 * trim) {
        for a in 1..=(total + 1 - g) {
            let after = total + 1 - a - g;
            let t = ln(a - 1, trim) + ln(g - 2, kept - 2) + ln(after, trim) - ln_all;
            if t > f64::NEG_INFINITY {
                p += t.exp();
            }
        }
    }
    p.min(1.0)
}

/// The Moses test, trimming `trim` control cases from each end (5 percent of the control group, at least 1, when None). None
/// when either group is empty or too few control cases remain after trimming.
pub fn moses(control: &[f64], test: &[f64], trim: Option<usize>) -> Option<Moses> {
    let c: Vec<f64> = control.iter().copied().filter(|v| v.is_finite()).collect();
    let t: Vec<f64> = test.iter().copied().filter(|v| v.is_finite()).collect();
    let (nc, nt) = (c.len(), t.len());
    if nc < 3 || nt < 1 {
        return None;
    }
    let h = trim.unwrap_or_else(|| ((0.05 * nc as f64).floor() as usize).max(1));
    if nc < 2 * h + 2 {
        return None;
    }
    // The pooled order, with the control cases before test cases of the same value.
    let mut pooled: Vec<(f64, bool)> = c.iter().map(|v| (*v, true)).chain(t.iter().map(|v| (*v, false))).collect();
    pooled.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let ties = pooled.windows(2).any(|w| w[0].0 == w[1].0 && w[0].1 != w[1].1);
    let positions: Vec<usize> = pooled.iter().enumerate().filter(|(_, (_, is_c))| *is_c).map(|(i, _)| i + 1).collect();
    let span_with = |h: usize| positions[nc - h - 1] - positions[h] + 1;
    let total = nc + nt;
    let (span, trimmed_span) = (span_with(0), span_with(h));
    Some(Moses {
        n_control: nc,
        n_test: nt,
        span,
        p: span_distribution_upto(total, nc, 0, span),
        trim: h,
        trimmed_span,
        trimmed_p: span_distribution_upto(total, nc, h, trimmed_span),
        ties,
    })
}
