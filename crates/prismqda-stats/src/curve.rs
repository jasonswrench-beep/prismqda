//! Curve fits: a curve fitted to two numbers by a straight-line regression of a transformed outcome on one or more transformed
//! predictors, as the statistical packages do (linear, logarithmic, inverse, quadratic, cubic, power, exponential, and S curves).
//!
//! Each curve is an ordinary least-squares fit of [`crate::regression::linear`], so the coefficients, their standard errors, the
//! intervals, R squared, and the F test are R's `lm` of the transformed variables. Because the outcome of the power, exponential,
//! and S curves is fitted on its logarithm, the R squared of those curves is on the logarithmic scale and cannot be compared with
//! the others'; the fit also reports the R squared **on the original scale** (one minus the sum of squared differences between the
//! outcome and the curve over the sum of squared differences from the mean), which can, and the root mean squared error there.

use crate::regression::{linear, Regression, RegressionError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    /// y = b0 + b1 x
    Linear,
    /// y = b0 + b1 ln x
    Logarithmic,
    /// y = b0 + b1 / x
    Inverse,
    /// y = b0 + b1 x + b2 x^2
    Quadratic,
    /// y = b0 + b1 x + b2 x^2 + b3 x^3
    Cubic,
    /// y = a x^b, fitted as ln y = ln a + b ln x
    Power,
    /// y = a e^(b x), fitted as ln y = ln a + b x
    Exponential,
    /// y = e^(b0 + b1 / x), fitted as ln y = b0 + b1 / x
    SCurve,
}

impl CurveKind {
    pub const ALL: [CurveKind; 8] = [
        CurveKind::Linear,
        CurveKind::Logarithmic,
        CurveKind::Inverse,
        CurveKind::Quadratic,
        CurveKind::Cubic,
        CurveKind::Power,
        CurveKind::Exponential,
        CurveKind::SCurve,
    ];

    /// True when the outcome is fitted on its natural logarithm.
    pub fn log_outcome(self) -> bool {
        matches!(self, CurveKind::Power | CurveKind::Exponential | CurveKind::SCurve)
    }

    fn needs_positive_x(self) -> bool {
        matches!(self, CurveKind::Logarithmic | CurveKind::Power)
    }

    fn needs_nonzero_x(self) -> bool {
        matches!(self, CurveKind::Inverse | CurveKind::SCurve)
    }

    /// How many terms the curve has besides its constant.
    pub fn terms(self) -> usize {
        match self {
            CurveKind::Quadratic => 2,
            CurveKind::Cubic => 3,
            _ => 1,
        }
    }
}

/// Why a curve could not be fitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveError {
    TooFewCases,
    NotFinite,
    /// The curve takes the logarithm of the predictor, which must be above zero in every case.
    NeedsPositiveX,
    /// The curve divides by the predictor, which must not be zero in any case.
    NeedsNonzeroX,
    /// The curve takes the logarithm of the outcome, which must be above zero in every case.
    NeedsPositiveY,
    NoVariationInOutcome,
    /// The predictor does not take enough different values for the curve (a cubic needs four, a quadratic three, the others two).
    TooFewValues,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CurveFit {
    pub kind: CurveKind,
    pub n: usize,
    /// The regression of the transformed outcome on the transformed predictors, intercept first.
    pub regression: Regression,
    /// R squared and the root mean squared error of the curve on the scale the outcome was measured on.
    pub r_squared_original: f64,
    pub rmse_original: f64,
    /// The constants of the curve in its own form: `[a, b]` for the power curve `a x^b` and the exponential `a e^(b x)`, and the
    /// coefficients themselves for the others.
    pub constants: Vec<f64>,
}

fn columns(kind: CurveKind, x: &[f64]) -> Vec<Vec<f64>> {
    match kind {
        CurveKind::Linear | CurveKind::Exponential => vec![x.to_vec()],
        CurveKind::Logarithmic | CurveKind::Power => vec![x.iter().map(|v| v.ln()).collect()],
        CurveKind::Inverse | CurveKind::SCurve => vec![x.iter().map(|v| 1.0 / v).collect()],
        CurveKind::Quadratic => vec![x.to_vec(), x.iter().map(|v| v * v).collect()],
        CurveKind::Cubic => vec![x.to_vec(), x.iter().map(|v| v * v).collect(), x.iter().map(|v| v * v * v).collect()],
    }
}

/// The value of the curve at `x`, given the coefficients of the regression (the constant first).
pub fn predict(kind: CurveKind, b: &[f64], x: f64) -> f64 {
    match kind {
        CurveKind::Linear => b[0] + b[1] * x,
        CurveKind::Logarithmic => b[0] + b[1] * x.ln(),
        CurveKind::Inverse => b[0] + b[1] / x,
        CurveKind::Quadratic => b[0] + b[1] * x + b[2] * x * x,
        CurveKind::Cubic => b[0] + b[1] * x + b[2] * x * x + b[3] * x * x * x,
        CurveKind::Power => (b[0] + b[1] * x.ln()).exp(),
        CurveKind::Exponential => (b[0] + b[1] * x).exp(),
        CurveKind::SCurve => (b[0] + b[1] / x).exp(),
    }
}

/// Fits the curve of `kind` to the outcome `y` and the predictor `x`, with intervals at `level`.
pub fn fit(kind: CurveKind, x: &[f64], y: &[f64], level: f64) -> Result<CurveFit, CurveError> {
    let n = y.len();
    if x.len() != n || x.iter().chain(y).any(|v| !v.is_finite()) {
        return Err(CurveError::NotFinite);
    }
    if n <= kind.terms() + 1 {
        return Err(CurveError::TooFewCases);
    }
    if kind.needs_positive_x() && x.iter().any(|&v| v <= 0.0) {
        return Err(CurveError::NeedsPositiveX);
    }
    if kind.needs_nonzero_x() && x.contains(&0.0) {
        return Err(CurveError::NeedsNonzeroX);
    }
    if kind.log_outcome() && y.iter().any(|&v| v <= 0.0) {
        return Err(CurveError::NeedsPositiveY);
    }
    let mut distinct: Vec<f64> = x.to_vec();
    distinct.sort_by(|a, b| a.total_cmp(b));
    distinct.dedup();
    if distinct.len() < kind.terms() + 1 {
        return Err(CurveError::TooFewValues);
    }
    let transformed: Vec<f64> = if kind.log_outcome() { y.iter().map(|v| v.ln()).collect() } else { y.to_vec() };
    let regression = linear(&transformed, &columns(kind, x), level).map_err(|e| match e {
        RegressionError::NoVariationInOutcome => CurveError::NoVariationInOutcome,
        RegressionError::TooFewCases => CurveError::TooFewCases,
        RegressionError::Collinear(_) => CurveError::TooFewValues,
        RegressionError::NotFinite => CurveError::NotFinite,
    })?;
    let b: Vec<f64> = regression.terms.iter().map(|t| t.estimate).collect();
    let mean_y = y.iter().sum::<f64>() / n as f64;
    let (mut sse, mut sst) = (0.0, 0.0);
    for (xi, yi) in x.iter().zip(y) {
        let yhat = predict(kind, &b, *xi);
        sse += (yi - yhat).powi(2);
        sst += (yi - mean_y).powi(2);
    }
    let constants = match kind {
        CurveKind::Power | CurveKind::Exponential => vec![b[0].exp(), b[1]],
        _ => b.clone(),
    };
    Ok(CurveFit { kind, n, regression, r_squared_original: 1.0 - sse / sst, rmse_original: (sse / n as f64).sqrt(), constants })
}
