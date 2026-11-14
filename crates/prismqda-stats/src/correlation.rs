//! Correlation between two variables: Pearson's r, Spearman's rho, and Kendall's tau-b.

use crate::describe::z_critical;
use crate::dist::{norm_cdf, t_two_sided};
use crate::rank::{rank_average, tie_sizes};

/// Which correlation was measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationKind {
    Pearson,
    Spearman,
    Kendall,
}

/// A correlation and the test of it being zero (two-sided).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Correlation {
    pub kind: CorrelationKind,
    pub n: usize,
    /// r, rho, or tau-b.
    pub r: f64,
    /// The test statistic: t for Pearson and Spearman, z for Kendall, or None when the exact Kendall test was used.
    pub statistic: Option<f64>,
    pub df: Option<f64>,
    pub p: f64,
    /// True when the p-value is the exact permutation one (Kendall without ties and under 50 pairs; Spearman without ties and
    /// under 10 pairs).
    pub exact: bool,
    /// True when the p-value is the Edgeworth series approximation to Spearman's exact test (no ties, 10 to 1,290 pairs), which
    /// is what R's `cor.test` gives and calls exact.
    pub edgeworth: bool,
    /// A confidence interval from Fisher's z transformation (Pearson only), at `level`.
    pub ci: Option<(f64, f64)>,
    pub level: f64,
}

fn paired_finite(x: &[f64], y: &[f64]) -> Option<(Vec<f64>, Vec<f64>)> {
    if x.len() != y.len() {
        return None;
    }
    let (mut a, mut b) = (Vec::with_capacity(x.len()), Vec::with_capacity(x.len()));
    for (u, v) in x.iter().zip(y) {
        if u.is_finite() && v.is_finite() {
            a.push(*u);
            b.push(*v);
        }
    }
    Some((a, b))
}

fn pearson_r(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (u, v) in x.iter().zip(y) {
        sxy += (u - mx) * (v - my);
        sxx += (u - mx) * (u - mx);
        syy += (v - my) * (v - my);
    }
    if sxx == 0.0 || syy == 0.0 {
        return None;
    }
    Some((sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0))
}

fn t_test_of_r(r: f64, n: usize) -> (f64, f64, f64) {
    let df = n as f64 - 2.0;
    let t = r * (df / (1.0 - r * r)).sqrt();
    (t, df, if r.abs() >= 1.0 { 0.0 } else { t_two_sided(t, df) })
}

/// Pearson's product-moment correlation of the pairs where both values are present. None under three pairs, or when
/// either variable is constant.
pub fn pearson(x: &[f64], y: &[f64], level: f64) -> Option<Correlation> {
    let (x, y) = paired_finite(x, y)?;
    let n = x.len();
    if n < 3 {
        return None;
    }
    let r = pearson_r(&x, &y)?;
    let (t, df, p) = t_test_of_r(r, n);
    let ci = (n > 3 && r.abs() < 1.0).then(|| {
        let z = r.atanh();
        let half = z_critical(level) / (n as f64 - 3.0).sqrt();
        ((z - half).tanh(), (z + half).tanh())
    });
    Some(Correlation { kind: CorrelationKind::Pearson, n, r, statistic: Some(t), df: Some(df), p, exact: false, edgeworth: false, ci, level })
}

/// Spearman's rank correlation (the Pearson correlation of the average ranks, so ties are handled). With no ties and at most
/// 1,290 pairs the p-value is the exact test of AS 89, as R's `cor.test` gives it: exact by counting every ordering for up to
/// 9 pairs, and by the Edgeworth series for 10 or more. With ties, or more pairs, it is the t approximation with n - 2
/// degrees of freedom.
pub fn spearman(x: &[f64], y: &[f64], level: f64) -> Option<Correlation> {
    let (x, y) = paired_finite(x, y)?;
    let n = x.len();
    if n < 3 {
        return None;
    }
    let r = pearson_r(&rank_average(&x), &rank_average(&y))?;
    let ties = !tie_sizes(&x).is_empty() || !tie_sizes(&y).is_empty();
    if !ties && n <= 1290 {
        let nf = n as f64;
        // The sum of squared differences of ranks, S, which the exact distribution is of.
        let s = ((nf * nf * nf - nf) * (1.0 - r) / 6.0).round();
        let half = (nf * nf * nf - nf) / 6.0;
        let p = if s > half { prho(n, s - 1.0, false) } else { prho(n, s + 2.0, true) };
        return Some(Correlation { kind: CorrelationKind::Spearman, n, r, statistic: None, df: None, p: (2.0 * p).min(1.0), exact: n <= 9, edgeworth: n > 9, ci: None, level });
    }
    let (t, df, p) = t_test_of_r(r, n);
    Some(Correlation { kind: CorrelationKind::Spearman, n, r, statistic: Some(t), df: Some(df), p, exact: false, edgeworth: false, ci: None, level })
}

/// The tail of the distribution of S, the sum of squared differences between the ranks of n pairs with no ties (R's `prho`,
/// algorithm AS 89). `lower_tail` is P(S < is); otherwise P(S >= is).
fn prho(n: usize, is: f64, lower_tail: bool) -> f64 {
    let n3 = {
        let f = n as f64;
        f * (f * f - 1.0) / 3.0
    };
    if is <= 0.0 {
        return if lower_tail { 0.0 } else { 1.0 };
    }
    if is > n3 {
        return if lower_tail { 1.0 } else { 0.0 };
    }
    if n <= 9 {
        // Exact: count the orderings of n items by the sum of squared displacements.
        let mut perm: Vec<usize> = (1..=n).collect();
        let mut total = 0u64;
        let mut at_least = 0u64;
        let mut count = |p: &[usize]| {
            total += 1;
            let ise: usize = p.iter().enumerate().map(|(i, &v)| (i + 1).abs_diff(v).pow(2)).sum();
            if is <= ise as f64 {
                at_least += 1;
            }
        };
        // Heap's algorithm visits every ordering once.
        let mut c = vec![0usize; n];
        count(&perm);
        let mut i = 0;
        while i < n {
            if c[i] < i {
                if i % 2 == 0 {
                    perm.swap(0, i);
                } else {
                    perm.swap(c[i], i);
                }
                count(&perm);
                c[i] += 1;
                i = 0;
            } else {
                c[i] = 0;
                i += 1;
            }
        }
        let nfac = total as f64;
        let upper = at_least as f64 / nfac;
        return if lower_tail { (nfac - at_least as f64) / nfac } else { upper };
    }
    // The Edgeworth series expansion.
    const C: [f64; 12] = [0.2274, 0.2531, 0.1745, 0.0758, 0.1033, 0.3932, 0.0879, 0.0151, 0.0072, 0.0831, 0.0131, 4.6e-4];
    let b = 1.0 / n as f64;
    let nf = n as f64;
    let x = (6.0 * (is - 1.0) * b / (nf * nf - 1.0) - 1.0) * (1.0 / b - 1.0).sqrt();
    let mut y = x * x;
    let u = x * b * (C[0] + b * (C[1] + C[2] * b) + y * (-C[3] + b * (C[4] + C[5] * b) - y * b * (C[6] + C[7] * b - y * (C[8] - C[9] * b + y * b * (C[10] - C[11] * y)))));
    y = u / (y / 2.0).exp();
    let tail = if lower_tail { norm_cdf(x) } else { crate::dist::norm_sf(x) };
    (if lower_tail { -y } else { y } + tail).clamp(0.0, 1.0)
}

/// The probability that a random permutation of n has at most `q` inversions (Kendall's exact null distribution).
fn kendall_cdf(q: i64, n: usize) -> f64 {
    if q < 0 {
        return 0.0;
    }
    let max = n * (n - 1) / 2;
    // prob[k] = probability of k inversions among i items; adding the i-th item adds 0 to i - 1 inversions evenly.
    let mut prob = vec![0.0; max + 1];
    prob[0] = 1.0;
    for i in 2..=n {
        let mut next = vec![0.0; max + 1];
        let mut window = 0.0;
        for (k, slot) in next.iter_mut().enumerate().take(i * (i - 1) / 2 + 1) {
            window += prob[k];
            if k >= i {
                window -= prob[k - i];
            }
            *slot = window / i as f64;
        }
        prob = next;
    }
    prob.iter().take(q.min(max as i64) as usize + 1).sum::<f64>().min(1.0)
}

/// Kendall's tau-b of the pairs where both values are present, which corrects for ties. The p-value is exact for fewer
/// than 50 pairs with no ties and a normal approximation (with the tie-adjusted variance) otherwise, as R's `cor.test`.
pub fn kendall(x: &[f64], y: &[f64], level: f64) -> Option<Correlation> {
    let (x, y) = paired_finite(x, y)?;
    let n = x.len();
    if n < 3 {
        return None;
    }
    let (mut concordant, mut discordant) = (0i64, 0i64);
    for i in 0..n {
        for j in (i + 1)..n {
            let (dx, dy) = (x[i] - x[j], y[i] - y[j]);
            if dx == 0.0 || dy == 0.0 {
                continue;
            }
            if (dx > 0.0) == (dy > 0.0) {
                concordant += 1;
            } else {
                discordant += 1;
            }
        }
    }
    let nf = n as f64;
    let n0 = nf * (nf - 1.0) / 2.0;
    let xt = tie_sizes(&x);
    let yt = tie_sizes(&y);
    let t1: f64 = xt.iter().map(|&t| (t * (t - 1)) as f64 / 2.0).sum();
    let t2: f64 = yt.iter().map(|&t| (t * (t - 1)) as f64 / 2.0).sum();
    let denominator = ((n0 - t1) * (n0 - t2)).sqrt();
    if denominator == 0.0 {
        return None;
    }
    let tau = (concordant - discordant) as f64 / denominator;
    let ties = !xt.is_empty() || !yt.is_empty();
    if n < 50 && !ties {
        let q = ((tau + 1.0) * nf * (nf - 1.0) / 4.0).round() as i64;
        let p = if q as f64 > nf * (nf - 1.0) / 4.0 { 1.0 - kendall_cdf(q - 1, n) } else { kendall_cdf(q, n) };
        return Some(Correlation { kind: CorrelationKind::Kendall, n, r: tau, statistic: None, df: None, p: (2.0 * p).min(1.0), exact: true, edgeworth: false, ci: None, level });
    }
    let s = tau * denominator;
    let sum = |sizes: &[usize], f: &dyn Fn(f64) -> f64| -> f64 { sizes.iter().map(|&t| f(t as f64)).sum() };
    let v0 = nf * (nf - 1.0) * (2.0 * nf + 5.0);
    let vt = sum(&xt, &|t| t * (t - 1.0) * (2.0 * t + 5.0));
    let vu = sum(&yt, &|t| t * (t - 1.0) * (2.0 * t + 5.0));
    let v1 = sum(&xt, &|t| t * (t - 1.0)) * sum(&yt, &|t| t * (t - 1.0));
    let v2 = sum(&xt, &|t| t * (t - 1.0) * (t - 2.0)) * sum(&yt, &|t| t * (t - 1.0) * (t - 2.0));
    let var_s = (v0 - vt - vu) / 18.0 + v1 / (2.0 * nf * (nf - 1.0)) + v2 / (9.0 * nf * (nf - 1.0) * (nf - 2.0));
    let z = s / var_s.sqrt();
    let p = 2.0 * norm_cdf(z).min(norm_cdf(-z));
    Some(Correlation { kind: CorrelationKind::Kendall, n, r: tau, statistic: Some(z), df: None, p, exact: false, edgeworth: false, ci: None, level })
}

/// A partial correlation: the correlation of two variables once the linear effect of some others is taken out of both.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PartialCorrelation {
    pub n: usize,
    /// How many variables were controlled for.
    pub controls: usize,
    pub r: f64,
    pub t: f64,
    pub df: f64,
    /// The two-sided p-value, from t with n - 2 - controls degrees of freedom.
    pub p: f64,
    /// A Fisher z interval with n - 3 - controls in the standard error.
    pub ci: Option<(f64, f64)>,
    pub level: f64,
}

/// The partial correlation of `x` and `y` controlling for the columns of `controls` (each as long as `x`), among the cases that
/// have every value. None with too few cases for the degrees of freedom, a constant variable, or a control that is perfectly
/// predicted by the others.
pub fn partial_correlation(x: &[f64], y: &[f64], controls: &[Vec<f64>], level: f64) -> Option<PartialCorrelation> {
    let n_all = x.len();
    if y.len() != n_all || controls.iter().any(|c| c.len() != n_all) || controls.is_empty() {
        return None;
    }
    let keep: Vec<usize> = (0..n_all).filter(|&i| x[i].is_finite() && y[i].is_finite() && controls.iter().all(|c| c[i].is_finite())).collect();
    let n = keep.len();
    let k = controls.len();
    if n < k + 4 {
        return None;
    }
    let pick = |v: &[f64]| -> Vec<f64> { keep.iter().map(|&i| v[i]).collect() };
    let (xs, ys) = (pick(x), pick(y));
    let cs: Vec<Vec<f64>> = controls.iter().map(|c| pick(c)).collect();
    let rx = crate::regression::linear(&xs, &cs, level).ok()?.residuals;
    let ry = crate::regression::linear(&ys, &cs, level).ok()?.residuals;
    let r = pearson_r(&rx, &ry)?;
    let df = (n - 2 - k) as f64;
    let t = r * (df / (1.0 - r * r)).sqrt();
    let p = if r.abs() >= 1.0 { 0.0 } else { t_two_sided(t, df) };
    let ci = (n > k + 3 && r.abs() < 1.0).then(|| {
        let z = r.atanh();
        let half = z_critical(level) / ((n - 3 - k) as f64).sqrt();
        ((z - half).tanh(), (z + half).tanh())
    });
    Some(PartialCorrelation { n, controls: k, r, t, df, p, ci, level })
}
