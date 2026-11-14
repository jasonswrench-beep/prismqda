//! The reliability of a scale made of several items: Cronbach's alpha.
//!
//! Every case must have a value for every item (cases are used whole, as the packages do by default). Alpha is
//! `k / (k - 1) * (1 - sum of the item variances / variance of the sum score)`, the same number as `1 - MS(residual) / MS(cases)`
//! of a two-way analysis of variance of the items by the cases without repeats (Hoyt's form, which R's `lm` gives).
//! The standardized alpha is `k * r / (1 + (k - 1) * r)` with `r` the mean of the correlations between items. The interval is
//! Feldt's, from the F distribution with `n - 1` and `(n - 1)(k - 1)` degrees of freedom. An item's correlation with the rest
//! is its correlation with the sum of the other items, and "alpha if the item were left out" is alpha of the other items.

use crate::dist::f_quantile;

/// What one item adds to the scale.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ScaleItem {
    pub mean: f64,
    pub sd: f64,
    /// Its correlation with the sum of the other items; None when either has no spread.
    pub item_rest_r: Option<f64>,
    /// Alpha of the other items; None with only two items, or when the others have no spread.
    pub alpha_if_deleted: Option<f64>,
}

/// Feldt's interval for alpha.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AlphaInterval {
    pub level: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Alpha {
    /// The cases, each with a value for every item.
    pub n: usize,
    pub k: usize,
    pub alpha: f64,
    /// None when an item has no spread.
    pub standardized_alpha: Option<f64>,
    /// The mean of the correlations between pairs of items; None when an item has no spread.
    pub mean_inter_item_r: Option<f64>,
    /// The mean and spread of the sum of the items.
    pub total_mean: f64,
    pub total_sd: f64,
    pub items: Vec<ScaleItem>,
    pub interval: Option<AlphaInterval>,
    /// How many pairs of items correlate negatively (an item scored the other way round gives this).
    pub negative_pairs: usize,
}

fn mean_of(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

fn covariance(x: &[f64], y: &[f64]) -> f64 {
    let (mx, my) = (mean_of(x), mean_of(y));
    x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum::<f64>() / (x.len() - 1) as f64
}

fn alpha_of(items: &[&Vec<f64>]) -> Option<f64> {
    let k = items.len();
    if k < 2 {
        return None;
    }
    let n = items[0].len();
    let total: Vec<f64> = (0..n).map(|i| items.iter().map(|c| c[i]).sum()).collect();
    let vt = covariance(&total, &total);
    if vt.is_nan() || vt <= 0.0 {
        return None;
    }
    let sum_var: f64 = items.iter().map(|c| covariance(c, c)).sum();
    Some(k as f64 / (k as f64 - 1.0) * (1.0 - sum_var / vt))
}

fn correlation(x: &[f64], y: &[f64]) -> Option<f64> {
    let (vx, vy) = (covariance(x, x), covariance(y, y));
    if vx.is_nan() || vy.is_nan() || vx <= 0.0 || vy <= 0.0 {
        return None;
    }
    Some(covariance(x, y) / (vx * vy).sqrt())
}

/// Cronbach's alpha of `items` (each as long as the number of cases), with Feldt's interval at `level`. None with fewer than two
/// items or three cases, or when the sum of the items has no spread.
pub fn cronbach_alpha(items: &[Vec<f64>], level: f64) -> Option<Alpha> {
    let k = items.len();
    if k < 2 {
        return None;
    }
    let n = items[0].len();
    if n < 3 || items.iter().any(|c| c.len() != n || c.iter().any(|v| !v.is_finite())) {
        return None;
    }
    let refs: Vec<&Vec<f64>> = items.iter().collect();
    let alpha = alpha_of(&refs)?;
    let total: Vec<f64> = (0..n).map(|i| items.iter().map(|c| c[i]).sum()).collect();

    let mut rs: Vec<f64> = Vec::new();
    let mut all_defined = true;
    for i in 0..k {
        for j in (i + 1)..k {
            match correlation(&items[i], &items[j]) {
                Some(r) => rs.push(r),
                None => all_defined = false,
            }
        }
    }
    let mean_r = (all_defined && !rs.is_empty()).then(|| mean_of(&rs));
    let standardized = mean_r.map(|r| k as f64 * r / (1.0 + (k as f64 - 1.0) * r));

    let item_stats: Vec<ScaleItem> = (0..k)
        .map(|j| {
            let rest: Vec<f64> = (0..n).map(|i| total[i] - items[j][i]).collect();
            let others: Vec<&Vec<f64>> = (0..k).filter(|&m| m != j).map(|m| &items[m]).collect();
            ScaleItem {
                mean: mean_of(&items[j]),
                sd: covariance(&items[j], &items[j]).sqrt(),
                item_rest_r: correlation(&items[j], &rest),
                alpha_if_deleted: alpha_of(&others),
            }
        })
        .collect();

    let interval = (level > 0.0 && level < 1.0 && alpha < 1.0).then(|| {
        let tail = (1.0 - level) / 2.0;
        let (df1, df2) = ((n - 1) as f64, ((n - 1) * (k - 1)) as f64);
        AlphaInterval { level, lower: 1.0 - (1.0 - alpha) * f_quantile(1.0 - tail, df1, df2), upper: 1.0 - (1.0 - alpha) * f_quantile(tail, df1, df2) }
    });
    Some(Alpha {
        n,
        k,
        alpha,
        standardized_alpha: standardized,
        mean_inter_item_r: mean_r,
        total_mean: mean_of(&total),
        total_sd: covariance(&total, &total).sqrt(),
        items: item_stats,
        interval,
        negative_pairs: rs.iter().filter(|r| **r < 0.0).count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_of_items_that_agree_perfectly_is_one_and_of_unrelated_ones_is_low() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let same = cronbach_alpha(&[a.clone(), a.clone(), a.clone()], 0.95).unwrap();
        assert!((same.alpha - 1.0).abs() < 1e-12 && (same.standardized_alpha.unwrap() - 1.0).abs() < 1e-12);
        assert!(same.interval.is_none(), "an alpha of one has no interval");
        let b = vec![3.0, 1.0, 4.0, 1.0, 5.0];
        let c = vec![2.0, 7.0, 1.0, 8.0, 2.0];
        let low = cronbach_alpha(&[a, b, c], 0.95).unwrap();
        assert!(low.alpha < 0.5, "{}", low.alpha);
    }

    #[test]
    fn two_items_work_by_hand() {
        // items 1,2,3,4 and 2,2,4,4: variances 5/3 and 4/3; total 3,4,7,8 has variance 13/3 + ... computed below
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = vec![2.0, 2.0, 4.0, 4.0];
        let a = cronbach_alpha(&[x.clone(), y.clone()], 0.95).unwrap();
        let total: Vec<f64> = x.iter().zip(&y).map(|(p, q)| p + q).collect();
        let mt = total.iter().sum::<f64>() / 4.0;
        let vt = total.iter().map(|t| (t - mt).powi(2)).sum::<f64>() / 3.0;
        let want = 2.0 * (1.0 - (5.0 / 3.0 + 4.0 / 3.0) / vt);
        assert!((a.alpha - want).abs() < 1e-12);
        assert!(a.items[0].alpha_if_deleted.is_none(), "one item is not a scale");
    }

    #[test]
    fn what_has_no_spread_is_left_empty() {
        assert!(cronbach_alpha(&[vec![1.0, 1.0, 1.0], vec![2.0, 2.0, 2.0]], 0.95).is_none(), "the total has no spread");
        assert!(cronbach_alpha(&[vec![1.0, 2.0, 3.0]], 0.95).is_none());
        assert!(cronbach_alpha(&[vec![1.0, 2.0], vec![2.0, 1.0]], 0.95).is_none(), "two cases are too few");
    }
}
