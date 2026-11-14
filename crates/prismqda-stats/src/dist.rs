//! Probability distributions: the normal, Student's t, chi-square, and F, with the special functions they stand on.
//!
//! Everything here is written from published definitions (the log-gamma Stirling series, the regularized incomplete gamma
//! and beta functions by their series and continued fractions) and compared with R's `lgamma`, `pnorm`, `qnorm`, `pt`,
//! `qt`, `pchisq`, `qchisq`, `pf`, and `qf` in `tests/distribution_reference.rs`. A p-value is computed from the tail
//! it describes, never as one minus the other tail, so a very small p-value keeps its digits.

use std::f64::consts::PI;

const TINY: f64 = 1e-300;
const EPS: f64 = 3e-16;
const MAX_ITERATIONS: usize = 100_000;

/// The Stirling series tail of the log-gamma function, for z >= 10: lnGamma(z) = (z - 1/2) ln z - z + ln(2 pi)/2 + this.
fn stirling_correction(z: f64) -> f64 {
    const C: [f64; 7] = [1.0 / 12.0, -1.0 / 360.0, 1.0 / 1260.0, -1.0 / 1680.0, 1.0 / 1188.0, -691.0 / 360_360.0, 1.0 / 156.0];
    let inv = 1.0 / z;
    let inv2 = inv * inv;
    let mut acc = C[6];
    for c in C[..6].iter().rev() {
        acc = acc * inv2 + c;
    }
    inv * acc
}

/// The natural log of the gamma function, for x > 0. (The Stirling series after shifting the argument up to 10 or more.)
pub fn ln_gamma(x: f64) -> f64 {
    if x.is_nan() || x <= 0.0 {
        return f64::NAN;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    let mut z = x;
    let mut shift = 0.0;
    while z < 10.0 {
        shift += z.ln();
        z += 1.0;
    }
    (z - 0.5) * z.ln() - z + 0.5 * (2.0 * PI).ln() + stirling_correction(z) - shift
}

/// ln(1 + d) - d, without the loss of digits a direct subtraction has when d is small.
fn ln1pmx(d: f64) -> f64 {
    if d.abs() < 0.5 {
        // -d^2/2 + d^3/3 - d^4/4 + ...
        let mut term = d;
        let mut sum = 0.0;
        for k in 2..200 {
            term *= -d;
            let add = term / k as f64;
            sum += add;
            if add.abs() < 1e-18 * sum.abs() {
                break;
            }
        }
        sum
    } else {
        d.ln_1p() - d
    }
}

/// ln(r) - (r - 1), for r > 0, accurate near 1 (series) and far from it (the logarithm of r itself, not of 1 + d).
fn lnr_minus(r: f64) -> f64 {
    let d = r - 1.0;
    if d.abs() < 0.5 {
        ln1pmx(d)
    } else {
        r.ln() - d
    }
}

/// The log of the beta function B(a, b), for a, b > 0, kept accurate when one or both are large.
pub fn ln_beta(a: f64, b: f64) -> f64 {
    let (p, q) = if a < b { (a, b) } else { (b, a) };
    if p >= 10.0 {
        let s = p + q;
        // lnB = (p - 1/2) ln(p/s) + (q - 1/2) ln(q/s) - ln(s)/2 + ln(2 pi)/2 + corrections.
        (p - 0.5) * (p / s).ln() + (q - 0.5) * (q / s).ln() - 0.5 * s.ln() + 0.5 * (2.0 * PI).ln() + stirling_correction(p) + stirling_correction(q) - stirling_correction(s)
    } else if q >= 10.0 {
        // lnGamma(p) + lnGamma(q) - lnGamma(p + q), the two big terms combined before they are subtracted.
        let s = p + q;
        ln_gamma(p) + (q - 0.5) * -(p / q).ln_1p() - p * s.ln() + p + stirling_correction(q) - stirling_correction(s)
    } else {
        ln_gamma(p) + ln_gamma(q) - ln_gamma(p + q)
    }
}

/// The log of the binomial coefficient, for whole n >= k >= 0.
pub fn ln_choose(n: f64, k: f64) -> f64 {
    ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0)
}

/// The regularized lower incomplete gamma function P(a, x).
pub fn gamma_p(a: f64, x: f64) -> f64 {
    if x.is_nan() || a.is_nan() || a <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    if x.is_infinite() {
        return 1.0;
    }
    if x < a + 1.0 {
        gamma_series(a, x)
    } else {
        1.0 - gamma_fraction(a, x)
    }
}

/// The regularized upper incomplete gamma function Q(a, x) = 1 - P(a, x), computed from its own tail.
pub fn gamma_q(a: f64, x: f64) -> f64 {
    if x.is_nan() || a.is_nan() || a <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 1.0;
    }
    if x.is_infinite() {
        return 0.0;
    }
    if x < a + 1.0 {
        1.0 - gamma_series(a, x)
    } else {
        gamma_fraction(a, x)
    }
}

fn gamma_prefactor(a: f64, x: f64) -> f64 {
    if a >= 10.0 {
        // x^a e^-x / Gamma(a), with the big terms combined: a (ln(x/a) - (x/a - 1)) + (ln(a / 2 pi))/2 - correction.
        (a * lnr_minus(x / a) + 0.5 * (a / (2.0 * PI)).ln() - stirling_correction(a)).exp()
    } else {
        (-x + a * x.ln() - ln_gamma(a)).exp()
    }
}

fn gamma_series(a: f64, x: f64) -> f64 {
    let mut term = 1.0 / a;
    let mut sum = term;
    let mut n = a;
    for _ in 0..MAX_ITERATIONS {
        n += 1.0;
        term *= x / n;
        sum += term;
        if term.abs() < sum.abs() * EPS {
            break;
        }
    }
    (sum * gamma_prefactor(a, x)).min(1.0)
}

fn gamma_fraction(a: f64, x: f64) -> f64 {
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..MAX_ITERATIONS {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS {
            break;
        }
    }
    (h * gamma_prefactor(a, x)).min(1.0)
}

/// The regularized incomplete beta function I_x(a, b), for a, b > 0 and 0 <= x <= 1.
pub fn beta_inc(x: f64, a: f64, b: f64) -> f64 {
    beta_inc_pair(x, 1.0 - x, a, b)
}

/// 1 - I_x(a, b), computed from its own tail.
pub fn beta_inc_upper(x: f64, a: f64, b: f64) -> f64 {
    beta_inc_pair(1.0 - x, x, b, a)
}

/// I_x(a, b) where the caller supplies y = 1 - x as well. A caller that has y to full precision (a t or F statistic gives
/// both x and y directly) avoids the loss of digits that 1 - x has when x is near 1.
pub fn beta_inc_pair(x: f64, y: f64, a: f64, b: f64) -> f64 {
    if x.is_nan() || y.is_nan() || a.is_nan() || b.is_nan() || a <= 0.0 || b <= 0.0 || x < 0.0 || y < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return 0.0;
    }
    if y == 0.0 {
        return 1.0;
    }
    let front = beta_front(x, y, a, b);
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(a, b, x) / a
    } else {
        1.0 - front * beta_fraction(b, a, y) / b
    }
}

/// x^a y^b / B(a, b) with y = 1 - x, the factor in front of the continued fraction.
fn beta_front(x: f64, y: f64, a: f64, b: f64) -> f64 {
    if a >= 10.0 && b >= 10.0 {
        // Both parameters large: combine the big terms. With s = a + b, x s / a and y s / b are near 1 together.
        let s = a + b;
        (a * lnr_minus(x * s / a) + b * lnr_minus(y * s / b) + 0.5 * (a * b / (s * 2.0 * PI)).ln() - stirling_correction(a) - stirling_correction(b) + stirling_correction(s)).exp()
    } else {
        // ln x and ln y, each from the number that is far from 1, so neither loses digits.
        let lx = if x > 0.5 { (-y).ln_1p() } else { x.ln() };
        let ly = if y > 0.5 { (-x).ln_1p() } else { y.ln() };
        (a * lx + b * ly - ln_beta(a, b)).exp()
    }
}

fn beta_fraction(a: f64, b: f64, x: f64) -> f64 {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..MAX_ITERATIONS {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

// ---------------------------------------------------------------- the normal distribution

/// The standard normal density.
pub fn norm_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// P(Z <= x) for a standard normal Z.
pub fn norm_cdf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < 0.0 {
        0.5 * gamma_q(0.5, x * x / 2.0)
    } else {
        1.0 - 0.5 * gamma_q(0.5, x * x / 2.0)
    }
}

/// P(Z > x), computed from the upper tail.
pub fn norm_sf(x: f64) -> f64 {
    norm_cdf(-x)
}

/// The two-sided tail: P(|Z| > |x|).
pub fn norm_two_sided(x: f64) -> f64 {
    gamma_q(0.5, x * x / 2.0)
}

/// The standard normal quantile (the inverse of `norm_cdf`), for 0 < p < 1.
pub fn norm_quantile(p: f64) -> f64 {
    if p.is_nan() || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if p == 0.5 {
        return 0.0;
    }
    // Work in the lower half and mirror, so the tail it solves is always the small one.
    let (q, sign) = if p < 0.5 { (p, -1.0) } else { (1.0 - p, 1.0) };
    // Abramowitz and Stegun 26.2.23 as the starting point (error under 5e-4), then Newton steps on the exact tail.
    let t = (-2.0 * q.ln()).sqrt();
    let mut x = t - (2.515_517 + 0.802_853 * t + 0.010_328 * t * t) / (1.0 + 1.432_788 * t + 0.189_269 * t * t + 0.001_308 * t * t * t);
    for _ in 0..60 {
        // x is positive here: solve sf(x) = q.
        let err = norm_sf(x) - q;
        let step = err / norm_pdf(x);
        // sf is decreasing, so x moves right when sf(x) is too large.
        let next = x + step;
        if (next - x).abs() <= 1e-16 * next.abs().max(1.0) {
            x = next;
            break;
        }
        x = next;
    }
    sign * x
}

// ---------------------------------------------------------------- Student's t

/// P(T <= t) for Student's t with `df` degrees of freedom.
pub fn t_cdf(t: f64, df: f64) -> f64 {
    if t.is_nan() || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    if t.is_infinite() {
        return if t > 0.0 { 1.0 } else { 0.0 };
    }
    let tail = 0.5 * t_two_sided(t, df);
    if t > 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

/// P(T > t), from the upper tail.
pub fn t_sf(t: f64, df: f64) -> f64 {
    t_cdf(-t, df)
}

/// The two-sided tail P(|T| > |t|), which is the p-value of a two-sided t test.
pub fn t_two_sided(t: f64, df: f64) -> f64 {
    if t.is_nan() || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    if t.is_infinite() {
        return 0.0;
    }
    // For huge df the beta form loses nothing, but df = infinity is the normal.
    if df.is_infinite() {
        return norm_two_sided(t);
    }
    let t2 = t * t;
    beta_inc_pair(df / (df + t2), t2 / (df + t2), df / 2.0, 0.5)
}

/// The t quantile (the inverse of `t_cdf`), for 0 < p < 1.
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if p.is_nan() || df.is_nan() || df <= 0.0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if p == 0.5 {
        return 0.0;
    }
    if df.is_infinite() {
        return norm_quantile(p);
    }
    let (q, sign) = if p < 0.5 { (p, -1.0) } else { (1.0 - p, 1.0) };
    // Find t > 0 with sf(t) = q, where sf is decreasing.
    let start = norm_quantile(q).abs().max(1.0);
    sign * invert_upper(q, |t| t_sf(t, df), start)
}

// ---------------------------------------------------------------- chi-square

/// P(X <= x) for a chi-square variable with `df` degrees of freedom.
pub fn chisq_cdf(x: f64, df: f64) -> f64 {
    gamma_p(df / 2.0, x / 2.0)
}

/// P(X > x), the p-value of a chi-square test, from the upper tail.
pub fn chisq_sf(x: f64, df: f64) -> f64 {
    gamma_q(df / 2.0, x / 2.0)
}

/// The chi-square quantile, for 0 <= p < 1.
pub fn chisq_quantile(p: f64, df: f64) -> f64 {
    if p.is_nan() || df.is_nan() || df <= 0.0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    invert_either(p, |x| chisq_cdf(x, df), |x| chisq_sf(x, df), df.max(1.0))
}

// ---------------------------------------------------------------- F

/// P(F <= x) for an F variable with `d1` and `d2` degrees of freedom.
pub fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x.is_nan() || d1 <= 0.0 || d2 <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    beta_inc_pair(d1 * x / (d1 * x + d2), d2 / (d1 * x + d2), d1 / 2.0, d2 / 2.0)
}

/// P(F > x), the p-value of an F test, from the upper tail.
pub fn f_sf(x: f64, d1: f64, d2: f64) -> f64 {
    if x.is_nan() || d1 <= 0.0 || d2 <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 1.0;
    }
    if x.is_infinite() {
        return 0.0;
    }
    beta_inc_pair(d2 / (d2 + d1 * x), d1 * x / (d2 + d1 * x), d2 / 2.0, d1 / 2.0)
}

/// The F quantile, for 0 <= p < 1.
pub fn f_quantile(p: f64, d1: f64, d2: f64) -> f64 {
    if p.is_nan() || d1 <= 0.0 || d2 <= 0.0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    invert_either(p, |x| f_cdf(x, d1, d2), |x| f_sf(x, d1, d2), 1.0)
}

/// The x > 0 whose upper tail `sf(x)` equals `target`, where `sf` decreases from 1.
fn invert_upper(target: f64, sf: impl Fn(f64) -> f64, start: f64) -> f64 {
    let mut lo = 0.0;
    let mut hi = start;
    while sf(hi) > target {
        lo = hi;
        hi *= 2.0;
        if hi > 1e300 {
            break;
        }
    }
    bisect(|x| sf(x) - target, lo, hi, false)
}

/// The x >= 0 with cdf(x) = p, solved from the tail that is the smaller one so that its digits are kept.
fn invert_either(p: f64, cdf: impl Fn(f64) -> f64, sf: impl Fn(f64) -> f64, start: f64) -> f64 {
    if p < 0.5 {
        let mut lo = 0.0;
        let mut hi = start;
        while cdf(hi) < p {
            lo = hi;
            hi *= 2.0;
            if hi > 1e300 {
                break;
            }
        }
        bisect(|x| cdf(x) - p, lo, hi, true)
    } else {
        invert_upper(1.0 - p, sf, start)
    }
}

/// The root of a monotone function that changes sign between `lo` and `hi`, by bisection to full precision.
fn bisect(f: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64, increasing: bool) -> f64 {
    for _ in 0..1200 {
        let mid = 0.5 * (lo + hi);
        if mid == lo || mid == hi {
            break;
        }
        let above = f(mid) > 0.0;
        // For an increasing function a positive value means the root is to the left; for a decreasing one, to the right.
        if above == increasing {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}
