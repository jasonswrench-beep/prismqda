//! The data behind the charts on the Statistics page. The engine works out what is drawn (bins, boxes, points, lines); the interface
//! draws it and words it, so a translator can, and the same data fill the table that stands beside each chart.
//!
//! The numbers come from `prismqda_stats::charts`, which is compared with R.

use prismqda_stats::charts::{box_stats, histogram, least_squares_line, normal_qq, Bin, BoxStats, Line, QqPoint};
use prismqda_stats::curve::{predict, CurveKind};
use prismqda_stats::describe::{descriptives, mean, variance};
use serde::Serialize;

/// The most points a scatter holds; beyond it every few cases are drawn (and the number left out is said).
const MAX_POINTS: usize = 3000;

/// What a chart is about: one of the variables of the run (by its place in the list), or a quantity made from them.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Subject {
    Variable { index: usize },
    /// The differences between two paired variables.
    Differences,
    /// What a regression leaves unexplained, case by case.
    Residuals,
    /// What a regression predicts, case by case.
    Fitted,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoxGroup {
    pub value: String,
    pub label: String,
    #[serde(flatten)]
    pub stats: BoxStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeanGroup {
    pub value: String,
    pub label: String,
    pub n: usize,
    pub mean: f64,
    /// The interval of the group's mean (Student's t), when the group has two or more cases.
    pub lo: Option<f64>,
    pub hi: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub case_id: i64,
}

/// One fitted curve as it is drawn: its points along the range of the predictor, and its value at each point of the scatter.
#[derive(Debug, Clone, Serialize)]
pub struct CurveLine {
    pub kind: CurveKind,
    /// `[x, y]` along the range of x, left out where the curve is not a finite number.
    pub points: Vec<[f64; 2]>,
    /// The curve's value at the x of each point of the scatter, in the same order.
    pub at_points: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BarCategory {
    pub value: String,
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Chart {
    Histogram { subject: Subject, n: usize, bins: Vec<Bin>, mean: f64, sd: Option<f64>, reference: Option<f64> },
    Bars { subject: Subject, valid: usize, categories: Vec<BarCategory> },
    Box { subject: Subject, by: usize, groups: Vec<BoxGroup> },
    Means { subject: Subject, by: usize, level: f64, groups: Vec<MeanGroup> },
    Stacked { row: usize, col: usize, row_values: Vec<String>, row_labels: Vec<String>, col_values: Vec<String>, col_labels: Vec<String>, counts: Vec<Vec<f64>> },
    Scatter { x: Subject, y: Subject, points: Vec<Point>, omitted: usize, fit: Option<Line>, zero_line: bool },
    /// The points of two numbers with one or more fitted curves through them.
    Curves { x: Subject, y: Subject, points: Vec<Point>, omitted: usize, curves: Vec<CurveLine> },
    Qq { subject: Subject, points: Vec<QqPoint>, line: Line },
}

/// The most rows and columns of a table that are drawn as bars.
pub const MAX_STACK_ROWS: usize = 12;
pub const MAX_STACK_COLS: usize = 8;

pub fn histogram_chart(subject: Subject, values: &[f64], reference: Option<f64>) -> Option<Chart> {
    let bins = histogram(values);
    if bins.is_empty() {
        return None;
    }
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    Some(Chart::Histogram { subject, n: finite.len(), bins, mean: mean(&finite)?, sd: variance(&finite).map(f64::sqrt), reference })
}

pub fn qq_chart(subject: Subject, values: &[f64]) -> Option<Chart> {
    let (points, line) = normal_qq(values)?;
    Some(Chart::Qq { subject, points, line })
}

/// A box for each group: `(value, label, values)`.
pub fn box_chart(subject: Subject, by: usize, groups: &[(String, String, Vec<f64>)]) -> Option<Chart> {
    let boxes: Vec<BoxGroup> = groups.iter().filter_map(|(value, label, v)| box_stats(v).map(|stats| BoxGroup { value: value.clone(), label: label.clone(), stats })).collect();
    (boxes.len() == groups.len() && !boxes.is_empty()).then_some(Chart::Box { subject, by, groups: boxes })
}

/// The mean of each group with the interval of that mean.
pub fn means_chart(subject: Subject, by: usize, level: f64, groups: &[(String, String, Vec<f64>)]) -> Option<Chart> {
    let out: Vec<MeanGroup> = groups
        .iter()
        .filter_map(|(value, label, v)| {
            let d = descriptives(v, level);
            Some(MeanGroup { value: value.clone(), label: label.clone(), n: d.n, mean: d.mean?, lo: d.ci_mean.map(|c| c.0), hi: d.ci_mean.map(|c| c.1) })
        })
        .collect();
    (out.len() == groups.len() && !out.is_empty()).then_some(Chart::Means { subject, by, level, groups: out })
}

/// A scatter of `(x, y)` with the case of each point. `fit` draws the least-squares line through all the points; `zero_line`
/// asks for a line at y = 0 (for residuals).
pub fn scatter_chart(x: Subject, y: Subject, xs: &[f64], ys: &[f64], case_ids: &[i64], fit: bool, zero_line: bool) -> Option<Chart> {
    if xs.len() != ys.len() || xs.len() != case_ids.len() || xs.is_empty() {
        return None;
    }
    let line = if fit { least_squares_line(xs, ys) } else { None };
    let step = xs.len().div_ceil(MAX_POINTS).max(1);
    let points: Vec<Point> = (0..xs.len()).step_by(step).map(|i| Point { x: xs[i], y: ys[i], case_id: case_ids[i] }).collect();
    let omitted = xs.len() - points.len();
    Some(Chart::Scatter { x, y, points, omitted, fit: line, zero_line })
}

/// How many positions along the range of x a curve is drawn at.
const CURVE_STEPS: usize = 80;

/// The points of `(x, y)` with each curve fitted, given as the kind and the coefficients of its regression.
pub fn curves_chart(x: Subject, y: Subject, xs: &[f64], ys: &[f64], case_ids: &[i64], fits: &[(CurveKind, Vec<f64>)]) -> Option<Chart> {
    let Chart::Scatter { x, y, points, omitted, .. } = scatter_chart(x, y, xs, ys, case_ids, false, false)? else { return None };
    let (lo, hi) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
    let curves: Vec<CurveLine> = fits
        .iter()
        .map(|(kind, b)| CurveLine {
            kind: *kind,
            points: (0..=CURVE_STEPS).map(|i| lo + (hi - lo) * i as f64 / CURVE_STEPS as f64).map(|px| [px, predict(*kind, b, px)]).filter(|p| p[1].is_finite()).collect(),
            at_points: points.iter().map(|p| predict(*kind, b, p.x)).collect(),
        })
        .collect();
    Some(Chart::Curves { x, y, points, omitted, curves })
}

/// A scatter drawn with a line given (a regression's own fit) rather than found.
pub fn scatter_with_line(x: Subject, y: Subject, xs: &[f64], ys: &[f64], case_ids: &[i64], line: Line) -> Option<Chart> {
    match scatter_chart(x, y, xs, ys, case_ids, false, false)? {
        Chart::Scatter { x, y, points, omitted, zero_line, .. } => Some(Chart::Scatter { x, y, points, omitted, fit: Some(line), zero_line }),
        other => Some(other),
    }
}
