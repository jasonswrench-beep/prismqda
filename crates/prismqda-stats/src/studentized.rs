//! The distribution of the studentized range, which Tukey's test and the Games-Howell test use. `ptukey` is the algorithm of
//! Copenhaver and Holland (1988) as R implements it, so the probabilities agree with R's to many digits. `qtukey` begins as
//! R's does and then keeps going: R stops when two steps differ by less than 1e-4, and the quantiles here are solved to full
//! accuracy (the difference is far below what a table of results shows).

use crate::dist::{ln_gamma, norm_cdf};

const C1: f64 = -30.0;
const C2: f64 = -50.0;
const C3: f64 = 60.0;
const BB: f64 = 8.0;
const WLAR: f64 = 3.0;
const WINCR1: f64 = 2.0;
const WINCR2: f64 = 3.0;
const XLEG: [f64; 6] = [
    0.981_560_634_246_719_3,
    0.904_117_256_370_474_9,
    0.769_902_674_194_304_7,
    0.587_317_954_286_617_4,
    0.367_831_498_998_180_2,
    0.125_233_408_511_468_9,
];
const ALEG: [f64; 6] = [
    0.047_175_336_386_511_83,
    0.106_939_325_995_318_43,
    0.160_078_328_543_346_23,
    0.203_167_426_723_065_92,
    0.233_492_536_538_354_8,
    0.249_147_045_813_402_8,
];

/// The probability integral of Hartley's form of the range for `rr` ranges of `cc` means, at range `w`.
fn wprob(w: f64, rr: f64, cc: f64) -> f64 {
    let nleg = 12usize;
    let ihalf = 6usize;
    let qsqz = w * 0.5;
    // Above 16 the lower bound of the integral is 0.99999999999995, so the answer is one.
    if qsqz >= BB {
        return 1.0;
    }
    let mut pr_w = 2.0 * norm_cdf(qsqz) - 1.0;
    pr_w = if pr_w >= (C2 / cc).exp() { pr_w.powf(cc) } else { 0.0 };
    let wincr = if w > WLAR { WINCR1 } else { WINCR2 };
    let mut blb = qsqz;
    let binc = (BB - qsqz) / wincr;
    let mut bub = blb + binc;
    let mut einsum = 0.0;
    let cc1 = cc - 1.0;
    let mut wi = 1.0;
    while wi <= wincr {
        let mut elsum = 0.0;
        let a = 0.5 * (bub + blb);
        let b = 0.5 * (bub - blb);
        for jj in 1..=nleg {
            let (j, xx) = if ihalf < jj {
                let j = nleg - jj + 1;
                (j, XLEG[j - 1])
            } else {
                (jj, -XLEG[jj - 1])
            };
            let c = b * xx;
            let ac = a + c;
            let qexpo = ac * ac;
            if qexpo > C3 {
                break;
            }
            let pplus = 2.0 * norm_cdf(ac);
            let pminus = 2.0 * norm_cdf(ac - w);
            let mut rinsum = pplus * 0.5 - pminus * 0.5;
            if rinsum >= (C1 / cc1).exp() {
                rinsum = (ALEG[j - 1] * (-(0.5 * qexpo)).exp()) * rinsum.powf(cc1);
                elsum += rinsum;
            }
        }
        elsum *= ((2.0 * b) * cc) * 0.398_942_280_401_432_7;
        einsum += elsum;
        blb = bub;
        bub += binc;
        wi += 1.0;
    }
    pr_w += einsum;
    if pr_w <= (C1 / rr).exp() {
        return 0.0;
    }
    pr_w = pr_w.powf(rr);
    pr_w.min(1.0)
}

const XLEGQ: [f64; 8] = [
    0.989_400_934_991_649_9,
    0.944_575_023_073_232_6,
    0.865_631_202_387_831_7,
    0.755_404_408_355_003,
    0.617_876_244_402_643_7,
    0.458_016_777_657_227_4,
    0.281_603_550_779_258_9,
    0.095_012_509_837_637_44,
];
const ALEGQ: [f64; 8] = [
    0.027_152_459_411_754_095,
    0.062_253_523_938_647_89,
    0.095_158_511_682_492_78,
    0.124_628_971_255_533_87,
    0.149_595_988_816_576_73,
    0.169_156_519_395_002_54,
    0.182_603_415_044_923_6,
    0.189_450_610_455_068_5,
];

/// P(Q <= q) for the studentized range of `k` means with `df` degrees of freedom (`ranges` is 1 for the usual case).
pub fn ptukey_ranges(q: f64, ranges: f64, k: f64, df: f64) -> f64 {
    if q.is_nan() || df.is_nan() || k.is_nan() {
        return f64::NAN;
    }
    if q <= 0.0 {
        return 0.0;
    }
    if df < 2.0 || ranges < 1.0 || k < 2.0 {
        return f64::NAN;
    }
    if q.is_infinite() {
        return 1.0;
    }
    if df > 25000.0 {
        return wprob(q, ranges, k);
    }
    let nlegq = 16usize;
    let ihalfq = 8usize;
    let f2 = df * 0.5;
    let mut f2lf = ((f2 * df.ln()) - (df * std::f64::consts::LN_2)) - ln_gamma(f2);
    let f21 = f2 - 1.0;
    let ff4 = df * 0.25;
    let ulen: f64 = if df <= 100.0 {
        1.0
    } else if df <= 800.0 {
        0.5
    } else if df <= 5000.0 {
        0.25
    } else {
        0.125
    };
    f2lf += ulen.ln();
    let mut ans = 0.0;
    for i in 1..=50 {
        let mut otsum = 0.0;
        let twa1 = (2 * i - 1) as f64 * ulen;
        for jj in 1..=nlegq {
            let (j, t1) = if ihalfq < jj {
                let j = jj - ihalfq - 1;
                (j, (f2lf + (f21 * (twa1 + (XLEGQ[j] * ulen)).ln())) - (XLEGQ[j] * ulen + twa1) * ff4)
            } else {
                let j = jj - 1;
                (j, (f2lf + (f21 * (twa1 - (XLEGQ[j] * ulen)).ln())) + (XLEGQ[j] * ulen - twa1) * ff4)
            };
            if t1 >= -30.0 {
                let qsqz = if ihalfq < jj { q * ((XLEGQ[j] * ulen + twa1) * 0.5).sqrt() } else { q * ((-(XLEGQ[j] * ulen) + twa1) * 0.5).sqrt() };
                let wprb = wprob(qsqz, ranges, k);
                otsum += (wprb * ALEGQ[j]) * t1.exp();
            }
        }
        if i as f64 * ulen >= 1.0 && otsum <= 1.0e-14 {
            break;
        }
        ans += otsum;
    }
    ans.min(1.0)
}

/// P(Q <= q) for the studentized range of `k` means with `df` degrees of freedom.
pub fn ptukey(q: f64, k: f64, df: f64) -> f64 {
    ptukey_ranges(q, 1.0, k, df)
}

/// P(Q > q): the p-value of a Tukey or Games-Howell comparison whose studentized range is q.
pub fn ptukey_upper(q: f64, k: f64, df: f64) -> f64 {
    (1.0 - ptukey(q, k, df)).max(0.0)
}

fn qinv(p: f64, c: f64, v: f64) -> f64 {
    const P0: f64 = 0.322_232_421_088;
    const Q0: f64 = 0.099_348_462_606;
    const P1: f64 = -1.0;
    const Q1: f64 = 0.588_581_570_495;
    const P2: f64 = -0.342_242_088_547;
    const Q2: f64 = 0.531_103_462_366;
    const P3: f64 = -0.204_231_210_125;
    const Q3: f64 = 0.103_537_752_850;
    const P4: f64 = -0.453_642_210_148e-04;
    const Q4: f64 = 0.385_607_006_34e-02;
    const K1: f64 = 0.8832;
    const K2: f64 = 0.2368;
    const K3: f64 = 1.214;
    const K4: f64 = 1.208;
    // An empirical constant of R's starting guess (close to the square root of 2, but not it); the guess has to be R's for
    // `qtukey_like_r` to follow R's steps.
    #[allow(clippy::approx_constant)]
    const K5: f64 = 1.4142;
    const VMAX: f64 = 120.0;
    let ps = 0.5 - 0.5 * p;
    let yi = (1.0 / (ps * ps)).ln().sqrt();
    let mut t = yi + ((((yi * P4 + P3) * yi + P2) * yi + P1) * yi + P0) / ((((yi * Q4 + Q3) * yi + Q2) * yi + Q1) * yi + Q0);
    if v < VMAX {
        t += (t * t * t + t) / v / 4.0;
    }
    let mut q = K1 - K2 * t;
    if v < VMAX {
        q += -K3 / v + K4 * t / v;
    }
    t * (q * (c - 1.0).ln() + K5)
}

/// R's secant iteration for `qtukey`, stopping when two steps differ by less than `eps`.
fn qtukey_with(p: f64, k: f64, df: f64, eps: f64) -> f64 {
    let mut x0 = qinv(p, k, df);
    let mut valx0 = ptukey(x0, k, df) - p;
    let mut x1 = if valx0 > 0.0 { (x0 - 1.0).max(0.0) } else { x0 + 1.0 };
    let mut valx1 = ptukey(x1, k, df) - p;
    let mut ans = x1;
    for _ in 1..200 {
        if valx1 == valx0 {
            break;
        }
        ans = x1 - ((valx1 * (x1 - x0)) / (valx1 - valx0));
        valx0 = valx1;
        x0 = x1;
        if ans < 0.0 {
            ans = 0.0;
        }
        valx1 = ptukey(ans, k, df) - p;
        x1 = ans;
        if (x1 - x0).abs() < eps {
            return ans;
        }
    }
    ans
}

/// R's own `qtukey(p, k, df)`, which stops at an accuracy of about 1e-4 in q. Kept so the tests can show the two agree to
/// that accuracy.
pub fn qtukey_like_r(p: f64, k: f64, df: f64) -> f64 {
    qtukey_with(p, k, df, 1e-4)
}

/// The p quantile of the studentized range of `k` means with `df` degrees of freedom, solved to full accuracy.
pub fn qtukey(p: f64, k: f64, df: f64) -> f64 {
    if p.is_nan() || k.is_nan() || df.is_nan() || df < 2.0 || k < 2.0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    // Start from R's answer, then bracket the root around it and bisect: ptukey rises with q.
    let start = qtukey_with(p, k, df, 1e-4).max(1e-6);
    let (mut lo, mut hi) = (start * 0.98, start * 1.02);
    let mut guard = 0;
    while ptukey(lo, k, df) > p && guard < 60 {
        lo *= 0.5;
        guard += 1;
    }
    guard = 0;
    while ptukey(hi, k, df) < p && guard < 60 {
        hi *= 2.0;
        guard += 1;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid == lo || mid == hi || (hi - lo) < 1e-13 * hi.max(1.0) {
            break;
        }
        if ptukey(mid, k, df) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}
