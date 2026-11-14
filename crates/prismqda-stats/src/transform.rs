//! Transformations of one variable into another: standardizing, ranking, and cutting into ranges. Each leaves a missing value missing.
//! They are compared with R's `scale`, `rank`, and `cut` in `tests/formula_reference.rs`.

use crate::rank::rank_average;

/// The z-scores of the values, `(x - mean) / sd` with the usual n - 1 in the deviation (R's `scale`). None when fewer than two values are
/// present or they are all the same, since a spread of nothing cannot be divided by.
pub fn standardize(x: &[Option<f64>]) -> Option<Vec<Option<f64>>> {
    let present: Vec<f64> = x.iter().flatten().copied().collect();
    if present.len() < 2 {
        return None;
    }
    let n = present.len() as f64;
    let mean = present.iter().sum::<f64>() / n;
    let sd = (present.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0)).sqrt();
    if sd == 0.0 || !sd.is_finite() {
        return None;
    }
    Some(x.iter().map(|v| v.map(|v| (v - mean) / sd)).collect())
}

/// The ranks of the values, the lowest 1, with tied values sharing the average of their ranks (R's `rank`, `na.last = "keep"`). With
/// `descending` the highest is 1.
pub fn rank_values(x: &[Option<f64>], descending: bool) -> Vec<Option<f64>> {
    let present: Vec<f64> = x.iter().flatten().map(|v| if descending { -v } else { *v }).collect();
    let mut ranks = rank_average(&present).into_iter();
    x.iter().map(|v| v.and_then(|_| ranks.next())).collect()
}

/// Which range each value falls in, as a number from 0, given the cut points (R's `cut` with `include.lowest = TRUE`). With `right` a
/// range includes its upper cut point and not its lower (`(a, b]`), except that the first includes its lower one too; without it a range
/// includes its lower cut point (`[a, b)`), except that the last includes its upper one. A value outside every range is missing.
pub fn cut(x: &[Option<f64>], cuts: &[f64], right: bool) -> Result<Vec<Option<usize>>, String> {
    if cuts.len() < 2 {
        return Err("cutting needs at least two cut points, which make one range".to_string());
    }
    if cuts.iter().any(|c| !c.is_finite()) || cuts.windows(2).any(|w| w[1] <= w[0]) {
        return Err("the cut points must be numbers that rise, each larger than the one before".to_string());
    }
    let last = cuts.len() - 1;
    Ok(x.iter()
        .map(|v| {
            let v = (*v)?;
            if v.is_nan() || v < cuts[0] || v > cuts[last] {
                return None;
            }
            (0..last).find(|&i| {
                let (lo, hi) = (cuts[i], cuts[i + 1]);
                if right {
                    (v > lo && v <= hi) || (i == 0 && v == lo)
                } else {
                    (v >= lo && v < hi) || (i == last - 1 && v == hi)
                }
            })
        })
        .collect())
}

/// The names of the ranges that `cut` finds, as `(a, b]` or `[a, b)`, with the first (or last) closed on both sides.
pub fn range_labels(cuts: &[f64], right: bool) -> Vec<String> {
    let show = |x: f64| if x.fract() == 0.0 && x.abs() < 1e15 { format!("{}", x as i64) } else { format!("{x}") };
    let last = cuts.len().saturating_sub(2);
    (0..cuts.len().saturating_sub(1))
        .map(|i| {
            let (lo, hi) = (show(cuts[i]), show(cuts[i + 1]));
            match (right, i == 0, i == last) {
                (true, true, _) => format!("[{lo}, {hi}]"),
                (true, false, _) => format!("({lo}, {hi}]"),
                (false, _, true) => format!("[{lo}, {hi}]"),
                (false, _, false) => format!("[{lo}, {hi})"),
            }
        })
        .collect()
}
