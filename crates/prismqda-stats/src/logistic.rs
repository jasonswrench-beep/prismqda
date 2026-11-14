//! Binary logistic regression: a yes-or-no outcome (0 or 1) from one or more predictors (numbers, or 0/1 columns standing for the
//! categories of a categorical variable), with an intercept.
//!
//! The model is fitted by iteratively reweighted least squares, the way R's `glm` does it (the same starting values and the same
//! working response), with each step a weighted least squares solved by the QR decomposition of [`crate::regression`]. The loop
//! stops when the deviance no longer changes at a tolerance of 1e-12 (R stops at 1e-8), so the estimates agree with R's to far
//! more digits than the tolerance of R's own stopping rule would give.
//!
//! Alongside the coefficients and their Wald tests the fit gives the odds ratios with Wald and profile-likelihood intervals, the
//! likelihood-ratio test of the model, the deviance, AIC and BIC, three pseudo R squared measures, the Hosmer-Lemeshow test, a
//! classification table, the area under the ROC curve, and a case-by-case look at leverage and influence. Models entered in steps
//! are compared by the change in deviance.
//!
//! When the predictors separate the two outcomes perfectly (or all but perfectly), the estimates do not exist: the likelihood keeps
//! rising as a coefficient grows without end. That is reported as an error, not as a result.

use crate::dist::{chisq_quantile, chisq_sf, norm_quantile, norm_two_sided};
use crate::rank::rank_average;
use crate::regression::fit;

/// Why a logistic regression could not be fitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogisticError {
    /// There are not more cases than parameters to estimate.
    TooFewCases,
    /// The outcome is the same for every case.
    NoVariationInOutcome,
    /// The outcome is not made of 0s and 1s.
    NotBinary,
    /// A predictor is exactly (or all but exactly) a combination of the others; the index is that predictor's place in the list.
    Collinear(usize),
    /// A value is missing or not a finite number.
    NotFinite,
    /// The predictors separate the outcomes (or the estimates do not settle), so the likelihood has no maximum.
    Separation,
}

/// One estimated coefficient and its tests.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LogisticTerm {
    /// The coefficient: the change in the log odds of the outcome that goes with one more unit of the predictor.
    pub estimate: f64,
    pub se: f64,
    /// The Wald statistic, the estimate over its standard error (a standard normal under the hypothesis of no effect).
    pub z: f64,
    pub wald: f64,
    pub p: f64,
    /// The coefficient's Wald interval.
    pub ci: (f64, f64),
    /// e to the estimate: the factor by which the odds are multiplied by one more unit of the predictor.
    pub odds_ratio: f64,
    pub or_ci: (f64, f64),
    /// The profile-likelihood interval of the coefficient and of the odds ratio, where it was asked for and exists (an end is
    /// missing when the likelihood does not fall far enough on that side).
    pub profile_ci: Option<(Option<f64>, Option<f64>)>,
    pub or_profile_ci: Option<(Option<f64>, Option<f64>)>,
}

/// A test of a change in deviance.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct DevianceTest {
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
}

/// One group of cases in the Hosmer-Lemeshow test.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct HlGroup {
    pub n: usize,
    pub observed: f64,
    pub expected: f64,
}

/// The Hosmer-Lemeshow test of fit: the cases are cut into groups of about equal size by their predicted probability, and the
/// counts of the outcome in each group are compared with what the model predicts.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct HosmerLemeshow {
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
    pub groups: Vec<HlGroup>,
}

/// The cases sorted into predicted and actual outcome at a cut-off probability.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Classification {
    pub cutoff: f64,
    pub true_positive: usize,
    pub false_positive: usize,
    pub true_negative: usize,
    pub false_negative: usize,
    pub accuracy: f64,
    pub sensitivity: Option<f64>,
    pub specificity: Option<f64>,
    pub positive_predictive_value: Option<f64>,
    pub negative_predictive_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Logistic {
    pub n: usize,
    /// How many predictors (not counting the intercept).
    pub k: usize,
    /// Cases with the outcome (1) and without it (0).
    pub events: usize,
    pub non_events: usize,
    pub df_residual: f64,
    /// The intercept first, then one for each predictor in the order given.
    pub terms: Vec<LogisticTerm>,
    /// The deviance of the model with only an intercept, and of this model (minus twice the log likelihood).
    pub null_deviance: f64,
    pub deviance: f64,
    pub log_likelihood: f64,
    pub null_log_likelihood: f64,
    pub aic: f64,
    pub bic: f64,
    /// The likelihood-ratio test of the model against the one with only an intercept.
    pub lr: DevianceTest,
    pub mcfadden: f64,
    pub cox_snell: f64,
    pub nagelkerke: f64,
    pub hosmer_lemeshow: Option<HosmerLemeshow>,
    pub classification: Classification,
    /// The area under the ROC curve: the chance that a case with the outcome gets a higher predicted probability than one without.
    pub auc: f64,
    pub iterations: usize,
    pub level: f64,
    // Case by case, in the order of the data.
    /// The predicted probability of the outcome.
    pub fitted: Vec<f64>,
    pub leverage: Vec<f64>,
    pub cooks_distance: Vec<f64>,
    pub pearson_residuals: Vec<f64>,
    pub deviance_residuals: Vec<f64>,
    /// The deviance residual divided by the square root of one minus the leverage.
    pub standardized_residuals: Vec<f64>,
}

/// The change from one model to a larger one that contains it.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct LogisticChange {
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
}

const MAX_ITERATIONS: usize = 100;
const TOLERANCE: f64 = 1e-12;

/// ln(1 + e^x), without overflow.
fn softplus(x: f64) -> f64 {
    x.max(0.0) + (-x.abs()).exp().ln_1p()
}

fn sigmoid(eta: f64) -> f64 {
    1.0 / (1.0 + (-eta).exp())
}

/// Minus twice the log likelihood of outcomes `y` (0 or 1) with log odds `eta`.
fn deviance_of(y: &[f64], eta: &[f64]) -> f64 {
    2.0 * y.iter().zip(eta).map(|(&yi, &e)| softplus(if yi == 1.0 { -e } else { e })).sum::<f64>()
}

struct Irls {
    coef: Vec<f64>,
    eta: Vec<f64>,
    mu: Vec<f64>,
    deviance: f64,
    iterations: usize,
    leverage: Vec<f64>,
    xtx_inv_diag: Vec<f64>,
}

/// The maximum likelihood fit of `y` on `columns` (the first of which is the intercept) with a fixed `offset` added to the log odds.
/// A step that would raise the deviance is shortened (halved until it does not), which keeps the iteration from running away when
/// it starts far from the maximum, as it does when the coefficient of one predictor is held at a value far from its estimate.
fn irls(y: &[f64], columns: &[Vec<f64>], offset: &[f64], start: Option<&[f64]>) -> Result<Irls, LogisticError> {
    let n = y.len();
    let p = columns.len();
    let eta_of = |b: &[f64]| -> Vec<f64> { (0..n).map(|i| offset[i] + columns.iter().zip(b).map(|(c, bj)| c[i] * bj).sum::<f64>()).collect() };
    let mut eta: Vec<f64> = match start {
        Some(b) => eta_of(b),
        None => y
            .iter()
            .map(|&v| {
                let mu = (v + 0.5) / 2.0;
                (mu / (1.0 - mu)).ln()
            })
            .collect(),
    };
    let mut dev_old = deviance_of(y, &eta);
    let mut previous: Option<Vec<f64>> = start.map(|b| b.to_vec());
    let mut coef = vec![0.0; p];
    let mut settled = false;
    for iteration in 1..=MAX_ITERATIONS {
        let mu: Vec<f64> = eta.iter().map(|&e| sigmoid(e)).collect();
        let w: Vec<f64> = mu.iter().map(|&m| (m * (1.0 - m)).max(f64::MIN_POSITIVE)).collect();
        let sw: Vec<f64> = w.iter().map(|v| v.sqrt()).collect();
        let z: Vec<f64> = (0..n).map(|i| (eta[i] - offset[i] + (y[i] - mu[i]) / w[i]) * sw[i]).collect();
        let design: Vec<Vec<f64>> = columns.iter().map(|c| c.iter().zip(&sw).map(|(v, s)| v * s).collect()).collect();
        let f = fit(&design, &z).map_err(LogisticError::Collinear)?;
        coef = f.coef.clone();
        eta = eta_of(&coef);
        let mut dev = deviance_of(y, &eta);
        if let Some(prev) = &previous {
            let mut step = 1.0;
            let mut halvings = 0;
            while (!dev.is_finite() || dev > dev_old + 1e-12 * (dev_old.abs() + 0.1)) && halvings < 40 {
                step *= 0.5;
                halvings += 1;
                coef = prev.iter().zip(&f.coef).map(|(a, b)| a + step * (b - a)).collect();
                eta = eta_of(&coef);
                dev = deviance_of(y, &eta);
            }
        }
        if !dev.is_finite() || eta.iter().any(|e| !e.is_finite()) {
            return Err(LogisticError::Separation);
        }
        // One more step once the deviance has settled, so that the weights behind the standard errors and the leverage are those of
        // the maximum and not of the iterate before it.
        if settled {
            let mu_final: Vec<f64> = eta.iter().map(|&e| sigmoid(e)).collect();
            let edge = 10.0 * f64::EPSILON;
            if mu_final.iter().any(|&m| m < edge || m > 1.0 - edge) {
                return Err(LogisticError::Separation);
            }
            return Ok(Irls { coef, eta, mu: mu_final, deviance: dev, iterations: iteration, leverage: f.leverage, xtx_inv_diag: f.xtx_inv_diag });
        }
        settled = (dev - dev_old).abs() / (dev.abs() + 0.1) < TOLERANCE;
        dev_old = dev;
        previous = Some(coef.clone());
    }
    Err(LogisticError::Separation)
}

/// The deviance of the model of `y` on `columns` with an intercept, or None when it cannot be fitted. For testing the predictors one
/// by one (leave one out and compare deviances).
pub fn fit_deviance(y: &[f64], columns: &[Vec<f64>]) -> Option<f64> {
    let n = y.len();
    let mut design = vec![vec![1.0; n]];
    design.extend(columns.iter().cloned());
    irls(y, &design, &vec![0.0; n], None).ok().map(|f| f.deviance)
}

/// A quantile of sorted values the way R's `quantile` finds it by default (type 7), in the same arithmetic.
fn quantile7(sorted: &[f64], p: f64) -> f64 {
    let index = 1.0 + (sorted.len() as f64 - 1.0) * p;
    let lo = index.floor();
    let hi = index.ceil();
    let x_lo = sorted[lo as usize - 1];
    let x_hi = sorted[hi as usize - 1];
    if index > lo && x_hi != x_lo {
        let h = index - lo;
        (1.0 - h) * x_lo + h * x_hi
    } else {
        x_lo
    }
}

/// The Hosmer-Lemeshow test with `g` groups, as `ResourceSelection::hoslem.test` does it: the break points are the quantiles of the
/// predicted probabilities (duplicates dropped), the groups are the intervals they cut (open at the low end, closed at the high end,
/// the lowest including its low end), and the test has the number of groups minus two degrees of freedom.
fn hosmer_lemeshow(y: &[f64], fitted: &[f64], g: usize) -> Option<HosmerLemeshow> {
    let mut sorted = fitted.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mut breaks: Vec<f64> = Vec::new();
    for i in 0..=g {
        let q = quantile7(&sorted, i as f64 * (1.0 / g as f64));
        if breaks.last().is_none_or(|&last| last != q) {
            breaks.push(q);
        }
    }
    if breaks.len() < 4 {
        return None;
    }
    let groups_count = breaks.len() - 1;
    let mut n = vec![0usize; groups_count];
    let mut observed = vec![0.0; groups_count];
    let mut expected = vec![0.0; groups_count];
    for (&yi, &pi) in y.iter().zip(fitted) {
        let k = (1..breaks.len()).find(|&b| pi <= breaks[b]).map(|b| b - 1).unwrap_or(groups_count - 1);
        n[k] += 1;
        observed[k] += yi;
        expected[k] += pi;
    }
    let mut chi2 = 0.0;
    let mut groups = Vec::new();
    for k in 0..groups_count {
        if n[k] == 0 {
            continue;
        }
        let (o1, e1) = (observed[k], expected[k]);
        let (o0, e0) = (n[k] as f64 - o1, n[k] as f64 - e1);
        chi2 += (o1 - e1).powi(2) / e1 + (o0 - e0).powi(2) / e0;
        groups.push(HlGroup { n: n[k], observed: o1, expected: e1 });
    }
    let df = groups.len() as f64 - 2.0;
    if df < 1.0 || !chi2.is_finite() {
        return None;
    }
    Some(HosmerLemeshow { chi2, df, p: chisq_sf(chi2, df), groups })
}

fn auc_of(y: &[f64], fitted: &[f64]) -> f64 {
    let ranks = rank_average(fitted);
    let n1 = y.iter().filter(|&&v| v == 1.0).count() as f64;
    let n0 = y.len() as f64 - n1;
    let sum: f64 = y.iter().zip(&ranks).filter(|(&yi, _)| yi == 1.0).map(|(_, r)| r).sum();
    (sum - n1 * (n1 + 1.0) / 2.0) / (n1 * n0)
}

fn classify(y: &[f64], fitted: &[f64], cutoff: f64) -> Classification {
    let (mut tp, mut fp, mut tn, mut fneg) = (0usize, 0usize, 0usize, 0usize);
    for (&yi, &pi) in y.iter().zip(fitted) {
        match (yi == 1.0, pi >= cutoff) {
            (true, true) => tp += 1,
            (false, true) => fp += 1,
            (false, false) => tn += 1,
            (true, false) => fneg += 1,
        }
    }
    let ratio = |a: usize, b: usize| (a + b > 0).then(|| a as f64 / (a + b) as f64);
    Classification {
        cutoff,
        true_positive: tp,
        false_positive: fp,
        true_negative: tn,
        false_negative: fneg,
        accuracy: (tp + tn) as f64 / y.len() as f64,
        sensitivity: ratio(tp, fneg),
        specificity: ratio(tn, fp),
        positive_predictive_value: ratio(tp, fp),
        negative_predictive_value: ratio(tn, fneg),
    }
}

/// One end of the profile-likelihood interval of the coefficient at `j` (the intercept is column 0): the value on the side `dir`
/// (+1 or -1) of the estimate at which the deviance, with that coefficient held at the value, has risen by `rise` above the best.
/// None when it does not rise that far (the interval is open on that side).
fn profile_end(y: &[f64], design: &[Vec<f64>], j: usize, full: &Irls, se: f64, rise: f64, dir: f64) -> Option<f64> {
    let rest: Vec<Vec<f64>> = design.iter().enumerate().filter(|(i, _)| *i != j).map(|(_, c)| c.clone()).collect();
    let start: Vec<f64> = full.coef.iter().enumerate().filter(|(i, _)| *i != j).map(|(_, b)| *b).collect();
    let target = full.deviance + rise;
    // The profile deviance at coefficient value b, minus the target; a value at which the rest cannot be fitted is past the target.
    let g = |b: f64| -> f64 {
        let offset: Vec<f64> = design[j].iter().map(|x| x * b).collect();
        match irls(y, &rest, &offset, Some(&start)) {
            Ok(f) => f.deviance - target,
            Err(_) => f64::INFINITY,
        }
    };
    let est = full.coef[j];
    let mut step = se.max(1e-6);
    let mut far = est + dir * step;
    let mut g_far = g(far);
    let mut doublings = 0;
    while g_far < 0.0 {
        step *= 2.0;
        far = est + dir * step;
        g_far = g(far);
        doublings += 1;
            if doublings > 40 {
            return None;
        }
    }
    // Bracketed between the estimate (below the target) and `far` (at or above it); regula falsi with the Illinois correction.
    let (mut a, mut ga) = (est, -rise);
    let (mut b, mut gb) = (far, g_far);
    if !gb.is_finite() {
        // Bisect until the far end is a point at which the rest can be fitted.
        for _ in 0..60 {
            let m = 0.5 * (a + b);
            let gm = g(m);
            if gm.is_finite() {
                b = m;
                gb = gm;
                break;
            }
            b = m;
        }
        if !gb.is_finite() {
            return None;
        }
    }
    let mut side = 0;
    for _ in 0..200 {
        let c = (a * gb - b * ga) / (gb - ga);
        let gc = g(c);
            if !gc.is_finite() {
            return None;
        }
        if gc.abs() < 1e-11 || (b - a).abs() < 1e-13 * (1.0 + c.abs()) {
            return Some(c);
        }
        if gc * gb > 0.0 {
            b = c;
            gb = gc;
            if side == -1 {
                ga /= 2.0;
            }
            side = -1;
        } else {
            a = c;
            ga = gc;
            if side == 1 {
                gb /= 2.0;
            }
            side = 1;
        }
    }
    Some(0.5 * (a + b))
}

/// Fits the outcome `y` (each 0 or 1) on the predictor columns (each as long as `y`) with an intercept, and gives intervals at
/// `level`. With `profile` the profile-likelihood intervals of the coefficients are worked out as well, which refits the model
/// many times for each predictor and so is for the model that is reported and not for a bootstrap.
pub fn logistic(y: &[f64], predictors: &[Vec<f64>], level: f64, profile: bool) -> Result<Logistic, LogisticError> {
    let n = y.len();
    let k = predictors.len();
    if predictors.iter().any(|c| c.len() != n) || y.iter().chain(predictors.iter().flatten()).any(|v| !v.is_finite()) {
        return Err(LogisticError::NotFinite);
    }
    if y.iter().any(|&v| v != 0.0 && v != 1.0) {
        return Err(LogisticError::NotBinary);
    }
    let p = k + 1;
    if n <= p {
        return Err(LogisticError::TooFewCases);
    }
    let events = y.iter().filter(|&&v| v == 1.0).count();
    let non_events = n - events;
    if events == 0 || non_events == 0 {
        return Err(LogisticError::NoVariationInOutcome);
    }
    let mut design = vec![vec![1.0; n]];
    design.extend(predictors.iter().cloned());
    let zero = vec![0.0; n];
    let f = irls(y, &design, &zero, None).map_err(|e| match e {
        LogisticError::Collinear(j) => LogisticError::Collinear(j.saturating_sub(1)),
        other => other,
    })?;

    let nf = n as f64;
    let share = events as f64 / nf;
    let null_deviance = -2.0 * (events as f64 * share.ln() + non_events as f64 * (1.0 - share).ln());
    let deviance = f.deviance;
    let log_likelihood = -deviance / 2.0;
    let null_log_likelihood = -null_deviance / 2.0;
    let z_crit = norm_quantile(1.0 - (1.0 - level) / 2.0);
    let rise = chisq_quantile(level, 1.0);

    let mut terms = Vec::with_capacity(p);
    for j in 0..p {
        let se = f.xtx_inv_diag[j].sqrt();
        let est = f.coef[j];
        let z = est / se;
        let ci = (est - z_crit * se, est + z_crit * se);
        let profile_ci = (profile && j > 0).then(|| (profile_end(y, &design, j, &f, se, rise, -1.0), profile_end(y, &design, j, &f, se, rise, 1.0)));
        let or_profile_ci = profile_ci.map(|(lo, hi)| (lo.map(f64::exp), hi.map(f64::exp)));
        terms.push(LogisticTerm { estimate: est, se, z, wald: z * z, p: norm_two_sided(z), ci, odds_ratio: est.exp(), or_ci: (ci.0.exp(), ci.1.exp()), profile_ci, or_profile_ci });
    }

    let lr_chi2 = null_deviance - deviance;
    let fitted = f.mu.clone();
    let pearson_residuals: Vec<f64> = (0..n).map(|i| (y[i] - fitted[i]) / (fitted[i] * (1.0 - fitted[i])).sqrt()).collect();
    let deviance_residuals: Vec<f64> = (0..n)
        .map(|i| {
            let d = 2.0 * softplus(if y[i] == 1.0 { -f.eta[i] } else { f.eta[i] });
            d.max(0.0).sqrt() * if y[i] == 1.0 { 1.0 } else { -1.0 }
        })
        .collect();
    let leverage = f.leverage.clone();
    let cooks_distance: Vec<f64> = (0..n).map(|i| pearson_residuals[i].powi(2) * leverage[i] / (p as f64 * (1.0 - leverage[i]).powi(2))).collect();
    let standardized_residuals: Vec<f64> = (0..n).map(|i| deviance_residuals[i] / (1.0 - leverage[i]).sqrt()).collect();

    Ok(Logistic {
        n,
        k,
        events,
        non_events,
        df_residual: (n - p) as f64,
        terms,
        null_deviance,
        deviance,
        log_likelihood,
        null_log_likelihood,
        aic: deviance + 2.0 * p as f64,
        bic: deviance + nf.ln() * p as f64,
        lr: DevianceTest { chi2: lr_chi2, df: k as f64, p: chisq_sf(lr_chi2, k as f64) },
        mcfadden: 1.0 - log_likelihood / null_log_likelihood,
        cox_snell: 1.0 - ((2.0 / nf) * (null_log_likelihood - log_likelihood)).exp(),
        nagelkerke: (1.0 - ((2.0 / nf) * (null_log_likelihood - log_likelihood)).exp()) / (1.0 - ((2.0 / nf) * null_log_likelihood).exp()),
        hosmer_lemeshow: hosmer_lemeshow(y, &fitted, 10),
        classification: classify(y, &fitted, 0.5),
        auc: auc_of(y, &fitted),
        iterations: f.iterations,
        level,
        fitted,
        leverage,
        cooks_distance,
        pearson_residuals,
        deviance_residuals,
        standardized_residuals,
    })
}

/// The change in deviance from `smaller` to `larger`, where the larger model contains every predictor of the smaller one and both
/// were fitted to the same cases.
pub fn compare(smaller: &Logistic, larger: &Logistic) -> Option<LogisticChange> {
    if smaller.n != larger.n || larger.k <= smaller.k {
        return None;
    }
    let chi2 = smaller.deviance - larger.deviance;
    let df = (larger.k - smaller.k) as f64;
    Some(LogisticChange { chi2, df, p: chisq_sf(chi2, df) })
}

/// A test of leaving out the predictor columns `without` (their places among `predictors`): the change in deviance and its p-value.
pub fn drop_test(y: &[f64], predictors: &[Vec<f64>], full_deviance: f64, without: &[usize]) -> Option<LogisticChange> {
    let rest: Vec<Vec<f64>> = predictors.iter().enumerate().filter(|(i, _)| !without.contains(i)).map(|(_, c)| c.clone()).collect();
    let dev = fit_deviance(y, &rest)?;
    let chi2 = dev - full_deviance;
    let df = without.len() as f64;
    Some(LogisticChange { chi2, df, p: chisq_sf(chi2, df) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 40 invented cases: the outcome gets likelier as x rises, and a second number has nothing to do with it.
    fn data() -> (Vec<f64>, Vec<Vec<f64>>) {
        let mut s: u64 = 12345;
        let mut next = move || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as f64 / (1u64 << 31) as f64
        };
        let x: Vec<f64> = (0..40).map(|_| 10.0 * next()).collect();
        let y: Vec<f64> = x.iter().map(|&xi| if next() < sigmoid(-2.0 + 0.4 * xi) { 1.0 } else { 0.0 }).collect();
        (y, vec![x])
    }

    #[test]
    fn fits_and_reports_the_things_it_promises() {
        let (y, x) = data();
        let m = logistic(&y, &x, 0.95, true).unwrap();
        assert_eq!(m.events + m.non_events, 40);
        assert_eq!(m.terms.len(), 2);
        assert!(m.terms[1].estimate > 0.0 && m.terms[1].p < 0.05);
        assert!((m.terms[1].odds_ratio - m.terms[1].estimate.exp()).abs() < 1e-12);
        let (lo, hi) = m.terms[1].profile_ci.unwrap();
        assert!(lo.unwrap() < m.terms[1].estimate && m.terms[1].estimate < hi.unwrap());
        assert!((m.lr.chi2 - (m.null_deviance - m.deviance)).abs() < 1e-12);
        assert!((0.5..=1.0).contains(&m.auc));
        assert_eq!(m.classification.true_positive + m.classification.false_negative, m.events);
    }

    #[test]
    fn perfect_separation_is_an_error_and_not_a_result() {
        let x = vec![vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]];
        let y = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        assert_eq!(logistic(&y, &x, 0.95, false).unwrap_err(), LogisticError::Separation);
    }

    #[test]
    fn refuses_what_it_cannot_fit() {
        let (y, x) = data();
        assert_eq!(logistic(&[0.0, 2.0, 1.0, 0.0], &[vec![1.0, 2.0, 3.0, 4.0]], 0.95, false).unwrap_err(), LogisticError::NotBinary);
        assert_eq!(logistic(&[1.0; 6], &[vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]], 0.95, false).unwrap_err(), LogisticError::NoVariationInOutcome);
        assert_eq!(logistic(&[0.0, 1.0], &[vec![1.0, 2.0]], 0.95, false).unwrap_err(), LogisticError::TooFewCases);
        let twice = vec![x[0].clone(), x[0].iter().map(|v| v * 2.0).collect()];
        assert!(matches!(logistic(&y, &twice, 0.95, false), Err(LogisticError::Collinear(_))));
    }

    #[test]
    fn a_nested_model_never_fits_better_than_the_one_around_it() {
        let (y, x) = data();
        let noise: Vec<f64> = (0..40).map(|i| ((i * 7919) % 13) as f64).collect();
        let small = logistic(&y, &x, 0.95, false).unwrap();
        let big = logistic(&y, &[x[0].clone(), noise], 0.95, false).unwrap();
        let change = compare(&small, &big).unwrap();
        assert!(change.chi2 >= 0.0 && change.df == 1.0);
        assert!(drop_test(&y, &[x[0].clone()], small.deviance, &[0]).unwrap().chi2 > 0.0);
    }
}
