//! The noncentral t distribution, and from it the exact interval of Cohen's d (Steiger and Fouladi, 1997).

use crate::dist::{beta_inc, ln_gamma, norm_cdf, norm_sf, t_cdf};

const LN_SQRT_PI: f64 = 0.572_364_942_924_700_1;
const SQRT_2_OVER_PI: f64 = 0.797_884_560_802_865_4;

/// P(T <= t) for the noncentral t distribution with `df` degrees of freedom and noncentrality `ncp`: the series of Lenth's
/// AS 243, which R's `pt(q, df, ncp)` uses.
pub fn t_cdf_noncentral(t: f64, df: f64, ncp: f64) -> f64 {
    if t.is_nan() || df.is_nan() || ncp.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    if ncp == 0.0 {
        return t_cdf(t, df);
    }
    if t.is_infinite() {
        return if t < 0.0 { 0.0 } else { 1.0 };
    }
    let (negdel, tt, del) = if t >= 0.0 {
        (false, t, ncp)
    } else {
        // The left tail is no larger than the normal tail at zero, which for a large ncp is zero.
        if ncp > 40.0 {
            return 0.0;
        }
        (true, -t, -ncp)
    };
    let lower = !negdel;
    // Very large df or ncp: the normal approximation of Abramowitz and Stegun 26.7.10.
    if df > 4e5 || del * del > 2.0 * std::f64::consts::LN_2 * 1021.0 {
        let s = 1.0 / (4.0 * df);
        let z = (tt * (1.0 - s) - del) / (1.0 + tt * tt * 2.0 * s).sqrt();
        return if lower { norm_cdf(z) } else { norm_sf(z) };
    }
    let mut x = t * t;
    x /= x + df;
    let mut tnc;
    if x > 0.0 {
        let lambda = del * del;
        let mut p = 0.5 * (-0.5 * lambda).exp();
        if p == 0.0 {
            return if lower { 0.0 } else { 1.0 };
        }
        let mut q = SQRT_2_OVER_PI * p * del;
        let mut s = 0.5 - p;
        if s < 1e-7 {
            s = -0.5 * (-0.5 * lambda).exp_m1();
        }
        let mut a = 0.5;
        let b = 0.5 * df;
        let rxb = (df / (t * t + df)).powf(b);
        let albeta = LN_SQRT_PI + ln_gamma(b) - ln_gamma(0.5 + b);
        let mut xodd = beta_inc(x, a, b);
        let mut godd = 2.0 * rxb * (a * x.ln() - albeta).exp();
        tnc = b * x;
        let mut xeven = if tnc < f64::EPSILON { tnc } else { 1.0 - rxb };
        let mut geven = tnc * rxb;
        tnc = p * xodd + q * xeven;
        for it in 1..=1000 {
            a += 1.0;
            xodd -= godd;
            xeven -= geven;
            godd *= x * (a + b - 1.0) / a;
            geven *= x * (a + b - 0.5) / (a + 0.5);
            p *= lambda / (2.0 * it as f64);
            q *= lambda / (2.0 * it as f64 + 1.0);
            tnc += p * xodd + q * xeven;
            s -= p;
            if s < -1e-10 {
                break;
            }
            if s >= 0.0 && (2.0 * s * (xodd - godd)).abs() < 1e-12 {
                break;
            }
        }
    } else {
        tnc = 0.0;
    }
    tnc += norm_cdf(-del);
    let v = tnc.min(1.0);
    if lower {
        v
    } else {
        0.5 - v + 0.5
    }
}

/// The root of a decreasing function between `lo` and `hi`, by bisection to full precision.
fn root_of_decreasing(f: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    for _ in 0..400 {
        let mid = 0.5 * (lo + hi);
        if mid == lo || mid == hi {
            break;
        }
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The confidence interval of the noncentrality parameter of a t statistic `t` with `df` degrees of freedom: the values of
/// ncp for which the observed t is the (1 - a) and the a quantile, with a = (1 - level) / 2. Multiplied by the scale that
/// turns a noncentrality parameter into an effect size (1 / sqrt(n) for one sample or pairs, sqrt(1/n1 + 1/n2) for two
/// groups) it is the exact interval of Cohen's d.
pub fn noncentral_t_interval(t: f64, df: f64, level: f64) -> Option<(f64, f64)> {
    if !(t.is_finite() && df > 0.0 && level > 0.0 && level < 1.0) {
        return None;
    }
    let a = (1.0 - level) / 2.0;
    let lim = (t.abs() + 40.0).max(40.0);
    // P(T <= t | ncp) falls as ncp rises, so each limit is where it crosses a fixed probability.
    let lo = root_of_decreasing(|ncp| t_cdf_noncentral(t, df, ncp) - (1.0 - a), -lim, lim);
    let hi = root_of_decreasing(|ncp| t_cdf_noncentral(t, df, ncp) - a, -lim, lim);
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}
