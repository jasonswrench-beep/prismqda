//! The Kolmogorov-Smirnov tests: one sample against a given distribution, and two samples against each other.
//!
//! The choices of R's `ks.test` are followed. The one-sample p-value is exact (the matrix method of Marsaglia, Tsang, and Wang,
//! 2003) for fewer than 100 values with no ties, and the limiting distribution otherwise. The two-sample p-value is exact, by
//! counting the paths of the empirical distributions that stay within the observed distance (with ties handled as R does, by
//! looking only where a block of equal values ends), when the product of the sizes is under 10,000, and the limiting
//! distribution otherwise. R stops its series for the limiting distribution at 1e-6; this one sums it fully.

use crate::dist::norm_cdf;

/// A Kolmogorov-Smirnov test.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct KolmogorovSmirnov {
    pub n1: usize,
    /// The second sample's size, or 0 for a test against a distribution.
    pub n2: usize,
    /// The largest distance between the two cumulative distributions.
    pub d: f64,
    pub p: f64,
    /// True when the p-value is exact.
    pub exact: bool,
}

/// P(D_n < d) for a sample of n from a continuous distribution (Marsaglia, Tsang, and Wang, 2003).
fn kolmogorov_exact(d: f64, n: usize) -> f64 {
    let nf = n as f64;
    let k = (nf * d) as usize + 1;
    let m = 2 * k - 1;
    let h = k as f64 - nf * d;
    let mut hm = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            hm[i * m + j] = if (i as i64) - (j as i64) + 1 < 0 { 0.0 } else { 1.0 };
        }
    }
    for i in 0..m {
        hm[i * m] -= h.powi(i as i32 + 1);
        hm[(m - 1) * m + i] -= h.powi((m - i) as i32);
    }
    hm[(m - 1) * m] += if 2.0 * h - 1.0 > 0.0 { (2.0 * h - 1.0).powi(m as i32) } else { 0.0 };
    for i in 0..m {
        for j in 0..m {
            let g = i as i64 - j as i64 + 1;
            if g > 0 {
                for f in 1..=g {
                    hm[i * m + j] /= f as f64;
                }
            }
        }
    }
    let (q, mut eq) = matrix_power(&hm, 0, m, n);
    let mut s = q[(k - 1) * m + k - 1];
    for i in 1..=n {
        s = s * i as f64 / nf;
        if s < 1e-140 {
            s *= 1e140;
            eq -= 140;
        }
    }
    s * 10f64.powi(eq)
}

fn multiply(a: &[f64], b: &[f64], m: usize) -> Vec<f64> {
    let mut c = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            let mut s = 0.0;
            for k in 0..m {
                s += a[i * m + k] * b[k * m + j];
            }
            c[i * m + j] = s;
        }
    }
    c
}

/// A^n with a separate power-of-ten exponent, so the entries cannot overflow.
fn matrix_power(a: &[f64], ea: i32, m: usize, n: usize) -> (Vec<f64>, i32) {
    if n == 1 {
        return (a.to_vec(), ea);
    }
    let (v, ev) = matrix_power(a, ea, m, n / 2);
    let b = multiply(&v, &v, m);
    let eb = 2 * ev;
    let (mut out, mut e) = if n.is_multiple_of(2) { (b, eb) } else { (multiply(a, &b, m), ea + eb) };
    if out[(m / 2) * m + (m / 2)] > 1e140 {
        for x in out.iter_mut() {
            *x *= 1e-140;
        }
        e += 140;
    }
    (out, e)
}

/// P(sqrt(n) D > x) in the limit of large samples: 2 sum (-1)^(k-1) exp(-2 k^2 x^2).
fn kolmogorov_limit_upper(x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    if x < 1.0 {
        // The other form of the same series converges fast for small x: 1 - sqrt(2 pi) / x * sum exp(-(2k-1)^2 pi^2 / (8 x^2)).
        let z = -(std::f64::consts::PI * std::f64::consts::PI) / (8.0 * x * x);
        let mut s = 0.0;
        for k in (1..60).step_by(2) {
            s += ((k * k) as f64 * z).exp();
        }
        return (1.0 - (2.0 * std::f64::consts::PI).sqrt() / x * s).clamp(0.0, 1.0);
    }
    let mut s = 0.0;
    let mut sign = 1.0;
    for k in 1..200 {
        let term = (-2.0 * (k * k) as f64 * x * x).exp();
        s += sign * term;
        sign = -sign;
        if term < 1e-18 {
            break;
        }
    }
    (2.0 * s).clamp(0.0, 1.0)
}

fn has_ties(sorted: &[f64]) -> bool {
    sorted.windows(2).any(|w| w[0] == w[1])
}

/// The one-sample test of the finite values of `x` against the distribution whose cumulative function is `cdf`. None under
/// one value.
pub fn ks_one_sample(x: &[f64], cdf: &dyn Fn(f64) -> f64) -> Option<KolmogorovSmirnov> {
    let mut v: Vec<f64> = x.iter().copied().filter(|a| a.is_finite()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n < 1 {
        return None;
    }
    let nf = n as f64;
    let mut d: f64 = 0.0;
    for (i, a) in v.iter().enumerate() {
        let f = cdf(*a);
        d = d.max((i as f64 + 1.0) / nf - f).max(f - i as f64 / nf);
    }
    let exact = n < 100 && !has_ties(&v);
    let p = if exact { (1.0 - kolmogorov_exact(d, n)).clamp(0.0, 1.0) } else { kolmogorov_limit_upper(nf.sqrt() * d) };
    Some(KolmogorovSmirnov { n1: n, n2: 0, d, p, exact })
}

/// The one-sample test against a normal distribution with the mean and standard deviation given (not estimated from the sample:
/// a normal with estimated parameters needs Lilliefors' table, and the Shapiro-Wilk test is the better test of normality).
pub fn ks_normal(x: &[f64], mean: f64, sd: f64) -> Option<KolmogorovSmirnov> {
    if sd.is_nan() || sd <= 0.0 {
        return None;
    }
    ks_one_sample(x, &|a| norm_cdf((a - mean) / sd))
}

/// The two-sample test of the finite values of `x` and `y`. None when either has no values.
pub fn ks_two_sample(x: &[f64], y: &[f64]) -> Option<KolmogorovSmirnov> {
    let xs: Vec<f64> = x.iter().copied().filter(|a| a.is_finite()).collect();
    let ys: Vec<f64> = y.iter().copied().filter(|a| a.is_finite()).collect();
    let (m, n) = (xs.len(), ys.len());
    if m == 0 || n == 0 {
        return None;
    }
    // Walk through the pooled values in order, tracking how many of each sample have been passed. The distance is measured where a
    // block of equal values ends, which is the only place the two cumulative distributions are defined.
    let mut pooled: Vec<(f64, bool)> = xs.iter().map(|v| (*v, true)).chain(ys.iter().map(|v| (*v, false))).collect();
    pooled.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total = m + n;
    let mut checkpoint = vec![false; total + 1];
    let (mut i, mut j) = (0usize, 0usize);
    let mut s_obs: i64 = 0;
    for (k, (v, is_x)) in pooled.iter().enumerate() {
        if *is_x {
            i += 1;
        } else {
            j += 1;
        }
        let end_of_block = k + 1 == total || pooled[k + 1].0 != *v;
        if end_of_block {
            checkpoint[k + 1] = true;
            s_obs = s_obs.max((i as i64 * n as i64 - j as i64 * m as i64).abs());
        }
    }
    let d = s_obs as f64 / (m as f64 * n as f64);
    let exact = (m as u64) * (n as u64) < 10_000;
    let p = if exact {
        // f[j] = the share of the paths to (i, j) that have stayed inside at every checkpoint so far.
        let mut f = vec![0.0f64; n + 1];
        let allowed = |a: usize, b: usize| !checkpoint[a + b] || ((a as i64 * n as i64 - b as i64 * m as i64).abs() < s_obs);
        f[0] = 1.0;
        for jj in 1..=n {
            f[jj] = if allowed(0, jj) { f[jj - 1] } else { 0.0 };
        }
        for ii in 1..=m {
            f[0] = if allowed(ii, 0) { f[0] } else { 0.0 };
            for jj in 1..=n {
                let w = ii as f64 / (ii + jj) as f64;
                // The old f[jj] is the value at (ii - 1, jj); f[jj - 1] has just been made for (ii, jj - 1).
                f[jj] = if allowed(ii, jj) { w * f[jj] + (1.0 - w) * f[jj - 1] } else { 0.0 };
            }
        }
        (1.0 - f[n]).clamp(0.0, 1.0)
    } else {
        kolmogorov_limit_upper((m as f64 * n as f64 / (m + n) as f64).sqrt() * d)
    };
    Some(KolmogorovSmirnov { n1: m, n2: n, d, p, exact })
}
