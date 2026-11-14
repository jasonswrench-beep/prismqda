//! Linear regression by least squares: one outcome (a number) from one or more predictors (numbers, or 0/1 columns standing for
//! the categories of a categorical variable), with an intercept.
//!
//! The fit is by a QR decomposition (modified Gram-Schmidt with a second pass, which keeps the columns orthogonal to machine
//! precision), not by the normal equations, so correlated predictors do not lose digits. Alongside the coefficients it gives what
//! is needed to judge the model: the share of variance explained, the F test, standardized coefficients, variance inflation
//! factors, a case-by-case look at leverage and influence, and the Breusch-Pagan test of equal error variance. Models entered in
//! steps (hierarchical regression) are compared with the F test of the change.

use crate::describe::{mean, variance};
use crate::dist::{chisq_sf, f_sf, t_quantile, t_two_sided};

/// Why a regression could not be fitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegressionError {
    /// There are not more cases than parameters to estimate.
    TooFewCases,
    /// The outcome is the same for every case.
    NoVariationInOutcome,
    /// A predictor is exactly (or all but exactly) a combination of the others; the index is that predictor's place in the list.
    Collinear(usize),
    /// A value is missing or not a finite number.
    NotFinite,
}

/// One estimated coefficient and its test.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Term {
    pub estimate: f64,
    pub se: f64,
    pub t: f64,
    pub p: f64,
    pub ci: (f64, f64),
    /// The coefficient if the outcome and the predictor were both standardized; None for the intercept.
    pub beta: Option<f64>,
    /// The variance inflation factor: 1 when the predictor is unrelated to the others, large when the others nearly predict it.
    pub vif: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct FStat {
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

/// The test of equal error variance (Koenker's studentized form of the Breusch-Pagan test).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct BreuschPagan {
    pub statistic: f64,
    pub df: f64,
    pub p: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Regression {
    pub n: usize,
    /// How many predictors (not counting the intercept).
    pub k: usize,
    pub df_residual: f64,
    /// The intercept first, then one for each predictor in the order given.
    pub terms: Vec<Term>,
    pub r_squared: f64,
    pub adj_r_squared: f64,
    pub f: FStat,
    /// The residual standard error.
    pub sigma: f64,
    pub rss: f64,
    pub tss: f64,
    pub level: f64,
    pub breusch_pagan: Option<BreuschPagan>,
    // Case by case, in the order of the data.
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    /// How far each case sits from the others in the predictors, from 1/n to 1.
    pub leverage: Vec<f64>,
    /// How much the fit would change if the case were left out.
    pub cooks_distance: Vec<f64>,
    /// The residual divided by its standard error (internally studentized).
    pub standardized_residuals: Vec<f64>,
    /// The residual with the case left out of the fit (externally studentized).
    pub studentized_residuals: Vec<f64>,
}

/// The change from one model to a larger one that contains it.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct ModelChange {
    pub r2_change: f64,
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

struct Qr {
    /// The orthonormal columns.
    q: Vec<Vec<f64>>,
    /// Upper triangular, `r[i][j]` for i <= j.
    r: Vec<Vec<f64>>,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn qr(columns: &[Vec<f64>]) -> Result<Qr, usize> {
    let p = columns.len();
    let mut q: Vec<Vec<f64>> = Vec::with_capacity(p);
    let mut r = vec![vec![0.0; p]; p];
    for (j, col) in columns.iter().enumerate() {
        let mut v = col.clone();
        let norm0 = dot(&v, &v).sqrt();
        for _ in 0..2 {
            for (i, qi) in q.iter().enumerate() {
                let c = dot(qi, &v);
                r[i][j] += c;
                for (vk, qk) in v.iter_mut().zip(qi) {
                    *vk -= c * qk;
                }
            }
        }
        let rjj = dot(&v, &v).sqrt();
        if norm0 == 0.0 || rjj <= 1e-10 * norm0 {
            return Err(j);
        }
        r[j][j] = rjj;
        q.push(v.into_iter().map(|x| x / rjj).collect());
    }
    Ok(Qr { q, r })
}

/// The inverse of an upper triangular matrix.
fn invert_upper(r: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let p = r.len();
    let mut inv = vec![vec![0.0; p]; p];
    #[allow(clippy::needless_range_loop)] // rows and columns of two matrices are walked together
    for col in 0..p {
        for row in (0..=col).rev() {
            let rhs = if row == col { 1.0 } else { 0.0 };
            let mut s = rhs;
            for k in (row + 1)..=col {
                s -= r[row][k] * inv[k][col];
            }
            inv[row][col] = s / r[row][row];
        }
    }
    inv
}

pub(crate) struct Fit {
    pub(crate) coef: Vec<f64>,
    pub(crate) fitted: Vec<f64>,
    pub(crate) leverage: Vec<f64>,
    /// The diagonal of (X'X) inverse.
    pub(crate) xtx_inv_diag: Vec<f64>,
}

/// Least squares of `y` on the columns of `design` (which holds its own intercept column when one is wanted). The error is the
/// index of the first column that is (all but exactly) a combination of the ones before it.
pub(crate) fn fit(design: &[Vec<f64>], y: &[f64]) -> Result<Fit, usize> {
    let Qr { q, r } = qr(design)?;
    let p = design.len();
    let qty: Vec<f64> = q.iter().map(|qj| dot(qj, y)).collect();
    // Solve R b = Q'y by back substitution.
    let mut coef = vec![0.0; p];
    for j in (0..p).rev() {
        let mut s = qty[j];
        for k in (j + 1)..p {
            s -= r[j][k] * coef[k];
        }
        coef[j] = s / r[j][j];
    }
    let n = y.len();
    let mut fitted = vec![0.0; n];
    let mut leverage = vec![0.0; n];
    for (qj, c) in q.iter().zip(&qty) {
        for i in 0..n {
            fitted[i] += c * qj[i];
            leverage[i] += qj[i] * qj[i];
        }
    }
    let inv = invert_upper(&r);
    let xtx_inv_diag = inv.iter().map(|row| row.iter().map(|v| v * v).sum()).collect();
    Ok(Fit { coef, fitted, leverage, xtx_inv_diag })
}

/// The share of variance of `y` that the columns explain (with an intercept), or None when they cannot be fitted.
fn r_squared_of(y: &[f64], columns: &[Vec<f64>]) -> Option<f64> {
    let n = y.len();
    let mut design = vec![vec![1.0; n]];
    design.extend(columns.iter().cloned());
    let f = fit(&design, y).ok()?;
    let my = mean(y)?;
    let tss: f64 = y.iter().map(|v| (v - my).powi(2)).sum();
    if tss == 0.0 {
        return None;
    }
    let rss: f64 = y.iter().zip(&f.fitted).map(|(a, b)| (a - b).powi(2)).sum();
    Some(1.0 - rss / tss)
}

/// Fits `y` on the predictor columns (each as long as `y`) with an intercept, and gives intervals at `level`.
pub fn linear(y: &[f64], predictors: &[Vec<f64>], level: f64) -> Result<Regression, RegressionError> {
    let n = y.len();
    let k = predictors.len();
    if predictors.iter().any(|c| c.len() != n) || y.iter().chain(predictors.iter().flatten()).any(|v| !v.is_finite()) {
        return Err(RegressionError::NotFinite);
    }
    let p = k + 1;
    if n <= p {
        return Err(RegressionError::TooFewCases);
    }
    let my = mean(y).ok_or(RegressionError::TooFewCases)?;
    let tss: f64 = y.iter().map(|v| (v - my).powi(2)).sum();
    if tss == 0.0 {
        return Err(RegressionError::NoVariationInOutcome);
    }
    let mut design = vec![vec![1.0; n]];
    design.extend(predictors.iter().cloned());
    let f = fit(&design, y).map_err(|j| RegressionError::Collinear(j.saturating_sub(1)))?;

    let residuals: Vec<f64> = y.iter().zip(&f.fitted).map(|(a, b)| a - b).collect();
    let rss: f64 = residuals.iter().map(|e| e * e).sum();
    let df_residual = (n - p) as f64;
    let sigma2 = rss / df_residual;
    let sigma = sigma2.sqrt();
    let q = t_quantile(1.0 - (1.0 - level) / 2.0, df_residual);
    let sd_y = variance(y).map(f64::sqrt);

    let mut terms = Vec::with_capacity(p);
    for j in 0..p {
        let se = (sigma2 * f.xtx_inv_diag[j]).sqrt();
        let est = f.coef[j];
        let t = est / se;
        let (beta, vif) = if j == 0 {
            (None, None)
        } else {
            let col = &predictors[j - 1];
            let sd_x = variance(col).map(f64::sqrt);
            let beta = sd_x.zip(sd_y).map(|(sx, sy)| est * sx / sy);
            let others: Vec<Vec<f64>> = predictors.iter().enumerate().filter(|(i, _)| *i != j - 1).map(|(_, c)| c.clone()).collect();
            let vif = if others.is_empty() { Some(1.0) } else { r_squared_of(col, &others).map(|r2| 1.0 / (1.0 - r2)) };
            (beta, vif)
        };
        terms.push(Term { estimate: est, se, t, p: t_two_sided(t, df_residual), ci: (est - q * se, est + q * se), beta, vif });
    }

    let r_squared = 1.0 - rss / tss;
    let nf = n as f64;
    let adj = 1.0 - (1.0 - r_squared) * (nf - 1.0) / df_residual;
    let fv = ((tss - rss) / k as f64) / sigma2;
    let (df1, df2) = (k as f64, df_residual);

    let mut cooks = Vec::with_capacity(n);
    let mut standardized = Vec::with_capacity(n);
    let mut studentized = Vec::with_capacity(n);
    for (&h, &e) in f.leverage.iter().zip(&residuals) {
        let r_int = e / (sigma * (1.0 - h).sqrt());
        standardized.push(r_int);
        cooks.push(e * e * h / (p as f64 * sigma2 * (1.0 - h) * (1.0 - h)));
        studentized.push(r_int * ((df_residual - 1.0) / (df_residual - r_int * r_int)).sqrt());
    }

    // Koenker's form of the Breusch-Pagan test: n times the R squared of the squared residuals on the predictors.
    let breusch_pagan = {
        let sq: Vec<f64> = residuals.iter().map(|e| e * e).collect();
        r_squared_of(&sq, predictors).map(|r2| {
            let statistic = nf * r2;
            BreuschPagan { statistic, df: k as f64, p: chisq_sf(statistic, k as f64) }
        })
    };

    Ok(Regression {
        n,
        k,
        df_residual,
        terms,
        r_squared,
        adj_r_squared: adj,
        f: FStat { f: fv, df1, df2, p: f_sf(fv, df1, df2) },
        sigma,
        rss,
        tss,
        level,
        breusch_pagan,
        fitted: f.fitted,
        residuals,
        leverage: f.leverage,
        cooks_distance: cooks,
        standardized_residuals: standardized,
        studentized_residuals: studentized,
    })
}

/// The F test of the change from `smaller` to `larger`, where the larger model contains every predictor of the smaller one and
/// both were fitted to the same cases.
pub fn compare(smaller: &Regression, larger: &Regression) -> Option<ModelChange> {
    if smaller.n != larger.n || larger.k <= smaller.k {
        return None;
    }
    let df1 = smaller.df_residual - larger.df_residual;
    let f = ((smaller.rss - larger.rss) / df1) / (larger.rss / larger.df_residual);
    Some(ModelChange { r2_change: larger.r_squared - smaller.r_squared, f, df1, df2: larger.df_residual, p: f_sf(f, df1, larger.df_residual) })
}

/// A dummy-coded 0/1 column for each category of a categorical variable except the reference one.
pub fn dummy_columns(values: &[String], categories: &[String], reference: &str) -> Vec<Vec<f64>> {
    categories.iter().filter(|c| c.as_str() != reference).map(|c| values.iter().map(|v| if v == c { 1.0 } else { 0.0 }).collect()).collect()
}
