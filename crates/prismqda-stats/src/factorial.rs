//! Two-way analysis of variance: a number explained by two groupings and what they do together.
//!
//! The sums of squares are **type III** (each effect adjusted for every other effect, interaction included), with effects coded
//! to add to zero across the levels of a factor (R's `contr.sum`). That is the default of the statistical packages and the one
//! the question "does this factor matter, whatever the other is" asks; for a balanced design (the same number of cases in
//! every cell) it equals the sequential sums of squares of R's `aov`. They are found as the increase in the residual sum of
//! squares when the columns of one effect are taken out of the full model, which is how R's `drop1(fit, ~ ., test = "F")`
//! gives them for a model fitted with `contr.sum`. Every cell must hold at least one case, and some cell at least two.

use crate::anova::{one_way, FTest};
use crate::describe::variance;
use crate::dist::f_sf;
use crate::regression::fit;

/// One effect of the analysis: the first factor, the second, or the two together.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Effect {
    /// `a`, `b`, or `ab`.
    pub name: &'static str,
    pub ss: f64,
    pub df: f64,
    pub ms: f64,
    pub f: f64,
    pub p: f64,
    /// The sum of squares of the effect over the sum of squares of the effect and of the error.
    pub partial_eta_squared: f64,
}

/// One combination of a level of each factor.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Cell {
    pub a: usize,
    pub b: usize,
    pub n: usize,
    pub mean: f64,
    /// None for a cell of one case.
    pub sd: Option<f64>,
}

/// The cases at one level of one factor, whatever the other is (the mean is of the cases, not of the cell means).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Margin {
    pub level: usize,
    pub n: usize,
    pub mean: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TwoWay {
    pub n: usize,
    pub levels_a: usize,
    pub levels_b: usize,
    /// The first factor, the second, and the interaction, in that order.
    pub effects: Vec<Effect>,
    pub error_ss: f64,
    pub error_df: f64,
    pub error_ms: f64,
    pub total_ss: f64,
    pub total_df: f64,
    pub r_squared: f64,
    pub adj_r_squared: f64,
    pub grand_mean: f64,
    /// Every cell holds the same number of cases.
    pub balanced: bool,
    pub cells: Vec<Cell>,
    pub margins_a: Vec<Margin>,
    pub margins_b: Vec<Margin>,
    /// Levene's test (about the median) on the cells, when every cell has at least two cases.
    pub levene: Option<FTest>,
    /// The residual of each case, in the order given.
    pub residuals: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoWayError {
    /// The lists are not as long as each other, or hold something that is not a number.
    Input,
    /// A factor has fewer than two levels.
    OneLevel,
    /// The combination of these two levels has no case.
    EmptyCell { a: usize, b: usize },
    /// There are not more cases than cells, so no error can be estimated.
    TooFewCases,
    /// Every value is the same.
    NoVariation,
}

/// The columns that code a factor of `levels` levels so that the effects add to zero: one for each level but the last, which is
/// minus one in all of them.
fn effect_columns(codes: &[usize], levels: usize) -> Vec<Vec<f64>> {
    (0..levels - 1).map(|j| codes.iter().map(|&c| if c == j { 1.0 } else if c == levels - 1 { -1.0 } else { 0.0 }).collect()).collect()
}

/// Analyzes `y` by two factors, each given as the level of every case counted from 0.
pub fn two_way(y: &[f64], a: &[usize], b: &[usize]) -> Result<TwoWay, TwoWayError> {
    let n = y.len();
    if n == 0 || a.len() != n || b.len() != n || y.iter().any(|v| !v.is_finite()) {
        return Err(TwoWayError::Input);
    }
    let (la, lb) = (a.iter().copied().max().unwrap() + 1, b.iter().copied().max().unwrap() + 1);
    if la < 2 || lb < 2 {
        return Err(TwoWayError::OneLevel);
    }
    let mut members: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); lb]; la];
    for i in 0..n {
        members[a[i]][b[i]].push(y[i]);
    }
    for (i, row) in members.iter().enumerate() {
        for (j, cell) in row.iter().enumerate() {
            if cell.is_empty() {
                return Err(TwoWayError::EmptyCell { a: i, b: j });
            }
        }
    }
    if n <= la * lb {
        return Err(TwoWayError::TooFewCases);
    }

    let ca = effect_columns(a, la);
    let cb = effect_columns(b, lb);
    let mut cab: Vec<Vec<f64>> = Vec::new();
    for x in &ca {
        for z in &cb {
            cab.push(x.iter().zip(z).map(|(p, q)| p * q).collect());
        }
    }
    let all = |skip: usize| -> Vec<Vec<f64>> {
        let mut cols = Vec::new();
        if skip != 0 {
            cols.extend(ca.iter().cloned());
        }
        if skip != 1 {
            cols.extend(cb.iter().cloned());
        }
        if skip != 2 {
            cols.extend(cab.iter().cloned());
        }
        cols
    };
    // Only the residual sum of squares of each model is needed, so the fit is the bare least-squares one (the bootstrap of the whole analysis
    // runs this on every resample).
    let residual_ss = |columns: Vec<Vec<f64>>| -> Option<(f64, Vec<f64>)> {
        let mut design = vec![vec![1.0; n]];
        design.extend(columns);
        let f = fit(&design, y).ok()?;
        let residuals: Vec<f64> = y.iter().zip(&f.fitted).map(|(v, w)| v - w).collect();
        Some((residuals.iter().map(|e| e * e).sum(), residuals))
    };
    let grand_mean = y.iter().sum::<f64>() / n as f64;
    let total_ss: f64 = y.iter().map(|v| (v - grand_mean).powi(2)).sum();
    if total_ss == 0.0 {
        return Err(TwoWayError::NoVariation);
    }
    let (error_ss, residuals) = residual_ss(all(3)).ok_or(TwoWayError::NoVariation)?;
    let error_df = (n - la * lb) as f64;
    let error_ms = error_ss / error_df;
    let dfs = [(la - 1) as f64, (lb - 1) as f64, ((la - 1) * (lb - 1)) as f64];
    let names = ["a", "b", "ab"];
    let mut effects = Vec::with_capacity(3);
    for (k, name) in names.iter().enumerate() {
        let (reduced_ss, _) = residual_ss(all(k)).ok_or(TwoWayError::NoVariation)?;
        let ss = (reduced_ss - error_ss).max(0.0);
        let ms = ss / dfs[k];
        let f = ms / error_ms;
        effects.push(Effect { name, ss, df: dfs[k], ms, f, p: f_sf(f, dfs[k], error_df), partial_eta_squared: ss / (ss + error_ss) });
    }

    let mut cells = Vec::with_capacity(la * lb);
    for (i, row) in members.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            cells.push(Cell { a: i, b: j, n: v.len(), mean: v.iter().sum::<f64>() / v.len() as f64, sd: variance(v).map(f64::sqrt) });
        }
    }
    let margin = |levels: usize, of: &[usize]| -> Vec<Margin> {
        (0..levels)
            .map(|level| {
                let v: Vec<f64> = (0..n).filter(|&i| of[i] == level).map(|i| y[i]).collect();
                Margin { level, n: v.len(), mean: v.iter().sum::<f64>() / v.len() as f64 }
            })
            .collect()
    };
    let balanced = cells.iter().all(|c| c.n == cells[0].n);
    let sets: Vec<Vec<f64>> = members.iter().flatten().cloned().collect();
    let levene = if sets.iter().all(|s| s.len() >= 2) { one_way(&sets).and_then(|o| o.levene) } else { None };
    Ok(TwoWay {
        n,
        levels_a: la,
        levels_b: lb,
        effects,
        error_ss,
        error_df,
        error_ms,
        total_ss,
        total_df: (n - 1) as f64,
        r_squared: 1.0 - error_ss / total_ss,
        adj_r_squared: 1.0 - (1.0 - (1.0 - error_ss / total_ss)) * (n - 1) as f64 / error_df,
        grand_mean,
        balanced,
        cells,
        margins_a: margin(la, a),
        margins_b: margin(lb, b),
        levene,
        residuals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_balanced_two_by_two_matches_the_sums_of_squares_worked_by_hand() {
        // cells: a0b0 = 1,3  a0b1 = 5,7  a1b0 = 2,4  a1b1 = 10,12 ; means 2, 6, 3, 11
        let y = [1.0, 3.0, 5.0, 7.0, 2.0, 4.0, 10.0, 12.0];
        let a = [0, 0, 0, 0, 1, 1, 1, 1];
        let b = [0, 0, 1, 1, 0, 0, 1, 1];
        let t = two_way(&y, &a, &b).unwrap();
        // grand mean 5.5; a means 4 and 7; b means 2.5 and 8.5
        assert!((t.effects[0].ss - 18.0).abs() < 1e-9, "{:?}", t.effects);
        assert!((t.effects[1].ss - 72.0).abs() < 1e-9);
        assert!((t.effects[2].ss - 8.0).abs() < 1e-9);
        assert!((t.error_ss - 8.0).abs() < 1e-9 && t.error_df == 4.0);
        assert!(t.balanced && (t.total_ss - 106.0).abs() < 1e-9);
    }

    #[test]
    fn what_cannot_be_analyzed_is_named() {
        assert_eq!(two_way(&[1.0, 2.0, 3.0], &[0, 0, 0], &[0, 1, 0]), Err(TwoWayError::OneLevel));
        assert_eq!(two_way(&[1.0, 2.0, 3.0, 4.0, 5.0], &[0, 0, 1, 1, 0], &[0, 0, 0, 0, 1]), Err(TwoWayError::EmptyCell { a: 1, b: 1 }));
        assert_eq!(two_way(&[1.0, 2.0, 3.0, 4.0], &[0, 0, 1, 1], &[0, 1, 0, 1]), Err(TwoWayError::TooFewCases));
        assert_eq!(two_way(&[1.0; 8], &[0, 0, 0, 0, 1, 1, 1, 1], &[0, 0, 1, 1, 0, 0, 1, 1]), Err(TwoWayError::NoVariation));
    }
}
