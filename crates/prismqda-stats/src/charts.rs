//! The numbers behind the charts: the bins of a histogram, the five numbers and outliers of a box plot, the points of a normal
//! probability plot, and the least-squares line through a scatter.
//!
//! None of these is an inference, but a chart that is drawn from wrong numbers misleads as surely as a wrong test, so each is
//! compared with R in `tests/charts_reference.rs`: the counts of the bins with `hist`, the points of the normal probability
//! plot with `qqnorm` and its line with the rule of `qqline`, and the box with the rule of `boxplot.stats` applied to the same
//! (type 7) quartiles as the tables use.

use crate::describe::{mean, percentile_sorted, sorted_finite};
use crate::dist::norm_quantile;

/// One bar of a histogram: the cases above `lo` and up to and including `hi` (the first bar includes `lo` too).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Bin {
    pub lo: f64,
    pub hi: f64,
    pub count: usize,
}

/// The step between bin edges: the width a histogram of `n` values would have by Sturges' rule, rounded up to 1, 2, 5, or 10 times a
/// power of ten, so the edges are numbers a person would write.
fn nice_width(range: f64, n: usize) -> f64 {
    let bins = ((n as f64).log2().ceil() + 1.0).max(3.0);
    let raw = range / bins;
    let magnitude = 10f64.powf(raw.log10().floor());
    let r = raw / magnitude;
    let m = if r <= 1.0 {
        1.0
    } else if r <= 2.0 {
        2.0
    } else if r <= 5.0 {
        5.0
    } else {
        10.0
    };
    m * magnitude
}

/// Rounds to 12 significant digits, so an edge such as 0.30000000000000004 is the 0.3 a value of 0.3 sits on.
fn tidy(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let digits = 11 - x.abs().log10().floor() as i32;
    let scale = 10f64.powi(digits);
    (x * scale).round() / scale
}

/// The bins of a histogram of the finite values. A single value (or all the same) gives one bin around it. Empty for no values.
pub fn histogram(values: &[f64]) -> Vec<Bin> {
    let sorted = sorted_finite(values);
    let (Some(&min), Some(&max)) = (sorted.first(), sorted.last()) else { return Vec::new() };
    if min == max {
        return vec![Bin { lo: tidy(min - 0.5), hi: tidy(min + 0.5), count: sorted.len() }];
    }
    let width = nice_width(max - min, sorted.len());
    let start = tidy((min / width).floor() * width);
    let mut edges = vec![start];
    let mut i = 1.0;
    while *edges.last().expect("not empty") < max {
        edges.push(tidy(start + i * width));
        i += 1.0;
    }
    let mut bins: Vec<Bin> = edges.windows(2).map(|w| Bin { lo: w[0], hi: w[1], count: 0 }).collect();
    for &v in &sorted {
        // The first bar whose upper edge is at or above the value: bars are (lo, hi], and the first includes its lower edge.
        if let Some(b) = bins.iter_mut().find(|b| v <= b.hi) {
            b.count += 1;
        }
    }
    bins
}

/// What a box plot draws for one set of values.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BoxStats {
    pub n: usize,
    /// The lowest value no further than 1.5 interquartile ranges below the first quartile.
    pub lower_whisker: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    /// The highest value no further than 1.5 interquartile ranges above the third quartile.
    pub upper_whisker: f64,
    pub mean: f64,
    /// The values beyond the whiskers, in order.
    pub outliers: Vec<f64>,
}

/// The box of the finite values: quartiles by R's default method (type 7), whiskers by the 1.5 interquartile range rule.
pub fn box_stats(values: &[f64]) -> Option<BoxStats> {
    let sorted = sorted_finite(values);
    let q1 = percentile_sorted(&sorted, 0.25)?;
    let median = percentile_sorted(&sorted, 0.5)?;
    let q3 = percentile_sorted(&sorted, 0.75)?;
    let iqr = q3 - q1;
    let (lo_fence, hi_fence) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
    let inside: Vec<f64> = sorted.iter().copied().filter(|&v| v >= lo_fence && v <= hi_fence).collect();
    let outliers = sorted.iter().copied().filter(|&v| v < lo_fence || v > hi_fence).collect();
    Some(BoxStats { n: sorted.len(), lower_whisker: *inside.first()?, q1, median, q3, upper_whisker: *inside.last()?, mean: mean(&sorted)?, outliers })
}

/// A straight line.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Line {
    pub intercept: f64,
    pub slope: f64,
}

/// One point of a normal probability plot: where the value would sit if the values were normal, and where it is.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct QqPoint {
    pub theoretical: f64,
    pub sample: f64,
}

/// The plotting positions R uses for a normal probability plot (`ppoints`): (i - a) / (n + 1 - 2a), with a = 3/8 for up to 10 values.
pub fn ppoints(n: usize) -> Vec<f64> {
    let a = if n <= 10 { 3.0 / 8.0 } else { 0.5 };
    (1..=n).map(|i| (i as f64 - a) / (n as f64 + 1.0 - 2.0 * a)).collect()
}

/// The points of a normal probability plot of the finite values (R's `qqnorm`), and the line through the quartiles (R's `qqline`).
pub fn normal_qq(values: &[f64]) -> Option<(Vec<QqPoint>, Line)> {
    let sorted = sorted_finite(values);
    if sorted.len() < 3 {
        return None;
    }
    let points: Vec<QqPoint> = ppoints(sorted.len()).into_iter().zip(&sorted).map(|(p, &s)| QqPoint { theoretical: norm_quantile(p), sample: s }).collect();
    let (y1, y3) = (percentile_sorted(&sorted, 0.25)?, percentile_sorted(&sorted, 0.75)?);
    let (x1, x3) = (norm_quantile(0.25), norm_quantile(0.75));
    let slope = (y3 - y1) / (x3 - x1);
    Some((points, Line { intercept: y1 - slope * x1, slope }))
}

/// The least-squares line of `y` on `x` for the pairs where both are finite. None below two pairs or when `x` does not vary.
pub fn least_squares_line(x: &[f64], y: &[f64]) -> Option<Line> {
    let pairs: Vec<(f64, f64)> = x.iter().zip(y).filter(|(a, b)| a.is_finite() && b.is_finite()).map(|(a, b)| (*a, *b)).collect();
    if pairs.len() < 2 {
        return None;
    }
    let n = pairs.len() as f64;
    let (mx, my) = (pairs.iter().map(|p| p.0).sum::<f64>() / n, pairs.iter().map(|p| p.1).sum::<f64>() / n);
    let sxx: f64 = pairs.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx == 0.0 {
        return None;
    }
    let sxy: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let slope = sxy / sxx;
    Some(Line { intercept: my - slope * mx, slope })
}
