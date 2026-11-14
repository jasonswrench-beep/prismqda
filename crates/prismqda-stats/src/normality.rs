//! The Shapiro-Wilk test of normality, by Royston's algorithm AS R94 (1995), as R's `shapiro.test` computes it. Good for
//! samples of 3 to 5,000 values.

use crate::dist::{norm_quantile, norm_sf};

/// The result of the Shapiro-Wilk test.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct ShapiroWilk {
    pub n: usize,
    /// W, between 0 and 1; values near 1 are what a normal sample gives.
    pub w: f64,
    /// The p-value of the hypothesis that the sample comes from a normal distribution.
    pub p: f64,
}

fn poly(cc: &[f64], x: f64) -> f64 {
    let nord = cc.len();
    let mut ret = cc[0];
    if nord > 1 {
        let mut p = x * cc[nord - 1];
        for j in (1..nord - 1).rev() {
            p = (p + cc[j]) * x;
        }
        ret += p;
    }
    ret
}

/// The Shapiro-Wilk test of the finite values of `x`. None under 3 values, over 5,000, or when every value is the same.
pub fn shapiro_wilk(x: &[f64]) -> Option<ShapiroWilk> {
    let mut x: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    x.sort_by(|a, b| a.partial_cmp(b).expect("finite values"));
    let n = x.len();
    if !(3..=5000).contains(&n) {
        return None;
    }
    let range = x[n - 1] - x[0];
    if range < 1e-19 {
        return None;
    }
    const G: [f64; 2] = [-2.273, 0.459];
    const C1: [f64; 6] = [0.0, 0.221_157, -0.147_981, -2.071_19, 4.434_685, -2.706_056];
    const C2: [f64; 6] = [0.0, 0.042_981, -0.293_762, -1.752_461, 5.682_633, -3.582_633];
    const C3: [f64; 4] = [0.544, -0.399_78, 0.025_054, -6.714e-4];
    const C4: [f64; 4] = [1.3822, -0.778_57, 0.062_767, -0.002_032_2];
    const C5: [f64; 4] = [-1.5861, -0.310_82, -0.083_751, 0.003_891_5];
    const C6: [f64; 3] = [-0.4803, -0.082_676, 0.003_030_2];
    let an = n as f64;
    let nn2 = n / 2;
    // The coefficients of the half that is above the middle, 1-based as in the algorithm.
    let mut a = vec![0.0; nn2 + 1];
    if n == 3 {
        a[1] = std::f64::consts::FRAC_1_SQRT_2;
    } else {
        let an25 = an + 0.25;
        let mut summ2 = 0.0;
        for (i, slot) in a.iter_mut().enumerate().skip(1) {
            *slot = norm_quantile((i as f64 - 0.375) / an25);
            summ2 += *slot * *slot;
        }
        summ2 *= 2.0;
        let ssumm2 = summ2.sqrt();
        let rsn = 1.0 / an.sqrt();
        let a1 = poly(&C1, rsn) - a[1] / ssumm2;
        let (i1, fac);
        if n > 5 {
            i1 = 3;
            let a2 = -a[2] / ssumm2 + poly(&C2, rsn);
            fac = ((summ2 - 2.0 * (a[1] * a[1]) - 2.0 * (a[2] * a[2])) / (1.0 - 2.0 * (a1 * a1) - 2.0 * (a2 * a2))).sqrt();
            a[2] = a2;
        } else {
            i1 = 2;
            fac = ((summ2 - 2.0 * (a[1] * a[1])) / (1.0 - 2.0 * (a1 * a1))).sqrt();
        }
        a[1] = a1;
        for slot in a.iter_mut().skip(i1) {
            *slot /= -fac;
        }
    }
    // The full antisymmetric vector of coefficients: the smallest values weigh negative, the largest positive.
    let mut c = vec![0.0; n];
    for i in 0..nn2 {
        c[i] = -a[i + 1];
        c[n - 1 - i] = a[i + 1];
    }
    let sa = c.iter().sum::<f64>() / an;
    let sx = x.iter().map(|v| v / range).sum::<f64>() / an;
    let (mut ssa, mut ssx, mut sax) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let asa = c[i] - sa;
        let xsx = x[i] / range - sx;
        ssa += asa * asa;
        ssx += xsx * xsx;
        sax += asa * xsx;
    }
    // 1 - W is computed on its own, so that a W very near 1 keeps its digits.
    let ssassx = (ssa * ssx).sqrt();
    let w1 = (ssassx - sax) * (ssassx + sax) / (ssa * ssx);
    let w = 1.0 - w1;
    if n == 3 {
        const PI6: f64 = 1.909_859_317_102_74;
        const STQR: f64 = std::f64::consts::FRAC_PI_3;
        let p = (PI6 * (w.sqrt().asin() - STQR)).max(0.0);
        return Some(ShapiroWilk { n, w, p });
    }
    let mut y = w1.ln();
    let xx = an.ln();
    let (m, s);
    if n <= 11 {
        let gamma = poly(&G, an);
        if y >= gamma {
            return Some(ShapiroWilk { n, w, p: 1e-99 });
        }
        y = -(gamma - y).ln();
        m = poly(&C3, an);
        s = poly(&C4, an).exp();
    } else {
        m = poly(&C5, xx);
        s = poly(&C6, xx).exp();
    }
    Some(ShapiroWilk { n, w, p: norm_sf((y - m) / s) })
}
