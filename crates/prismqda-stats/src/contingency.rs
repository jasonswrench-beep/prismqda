//! Tables of counts: the chi-square test of independence with its effect sizes and residuals, and Fisher's exact test for a
//! two-by-two table.

use crate::dist::{chisq_sf, ln_choose};

/// The chi-square test of independence on a table of counts (rows by columns).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ChiSquare {
    pub rows: usize,
    pub cols: usize,
    pub n: f64,
    pub expected: Vec<Vec<f64>>,
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
    /// Yates' continuity correction, for a two-by-two table only (R's default for that shape).
    pub yates: Option<(f64, f64)>,
    /// The likelihood-ratio statistic G squared, and its p-value.
    pub g2: f64,
    pub g2_p: f64,
    /// For a two-by-two table, the signed phi; otherwise None.
    pub phi: Option<f64>,
    pub cramers_v: f64,
    pub contingency_c: f64,
    /// (observed - expected) / sqrt(expected), cell by cell.
    pub standardized_residuals: Vec<Vec<f64>>,
    /// The adjusted residuals, which are approximately standard normal when the variables are independent.
    pub adjusted_residuals: Vec<Vec<f64>>,
    pub smallest_expected: f64,
    /// The share of cells whose expected count is below 5 (the usual caution for the chi-square approximation).
    pub share_expected_below_5: f64,
}

/// The chi-square test of independence. None for a table with fewer than two rows or columns, ragged rows, negative or
/// non-finite counts, or an empty row or column.
pub fn chi_square(table: &[Vec<f64>]) -> Option<ChiSquare> {
    let r = table.len();
    let c = table.first()?.len();
    if r < 2 || c < 2 || table.iter().any(|row| row.len() != c || row.iter().any(|v| !v.is_finite() || *v < 0.0)) {
        return None;
    }
    let row_sum: Vec<f64> = table.iter().map(|row| row.iter().sum()).collect();
    let col_sum: Vec<f64> = (0..c).map(|j| table.iter().map(|row| row[j]).sum()).collect();
    let n: f64 = row_sum.iter().sum();
    if n <= 0.0 || row_sum.contains(&0.0) || col_sum.contains(&0.0) {
        return None;
    }
    let expected: Vec<Vec<f64>> = row_sum.iter().map(|rs| col_sum.iter().map(|cs| rs * cs / n).collect()).collect();
    let mut chi2 = 0.0;
    let mut g2 = 0.0;
    let mut standardized = vec![vec![0.0; c]; r];
    let mut adjusted = vec![vec![0.0; c]; r];
    for i in 0..r {
        for j in 0..c {
            let (o, e) = (table[i][j], expected[i][j]);
            chi2 += (o - e) * (o - e) / e;
            if o > 0.0 {
                g2 += o * (o / e).ln();
            }
            standardized[i][j] = (o - e) / e.sqrt();
            adjusted[i][j] = (o - e) / (e * (1.0 - row_sum[i] / n) * (1.0 - col_sum[j] / n)).sqrt();
        }
    }
    g2 *= 2.0;
    let df = ((r - 1) * (c - 1)) as f64;
    let yates = (r == 2 && c == 2).then(|| {
        let mut stat = 0.0;
        // R: the correction is the smaller of 0.5 and the smallest |observed - expected|.
        let correction = (0..2).flat_map(|i| (0..2).map(move |j| (i, j))).map(|(i, j)| (table[i][j] - expected[i][j]).abs()).fold(0.5f64, f64::min);
        for i in 0..2 {
            for j in 0..2 {
                stat += ((table[i][j] - expected[i][j]).abs() - correction).powi(2) / expected[i][j];
            }
        }
        (stat, chisq_sf(stat, 1.0))
    });
    let phi = (r == 2 && c == 2).then(|| (table[0][0] * table[1][1] - table[0][1] * table[1][0]) / (row_sum[0] * row_sum[1] * col_sum[0] * col_sum[1]).sqrt());
    let smallest_expected = expected.iter().flatten().copied().fold(f64::INFINITY, f64::min);
    let below = expected.iter().flatten().filter(|&&e| e < 5.0).count() as f64 / (r * c) as f64;
    Some(ChiSquare {
        rows: r,
        cols: c,
        n,
        expected,
        chi2,
        df,
        p: chisq_sf(chi2, df),
        yates,
        g2,
        g2_p: chisq_sf(g2, df),
        phi,
        cramers_v: (chi2 / (n * (r.min(c) - 1) as f64)).sqrt(),
        contingency_c: (chi2 / (chi2 + n)).sqrt(),
        standardized_residuals: standardized,
        adjusted_residuals: adjusted,
        smallest_expected,
        share_expected_below_5: below,
    })
}

/// Fisher's exact test for a two-by-two table [[a, b], [c, d]].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Fisher {
    /// The two-sided exact p-value: the total probability of tables no more likely than the one observed.
    pub p: f64,
    /// The sample odds ratio a d / (b c), infinite when b c is zero.
    pub odds_ratio_sample: f64,
    /// The conditional maximum-likelihood odds ratio (what R reports), 0 or infinite at the edges of the support.
    pub odds_ratio: f64,
    pub level: f64,
    pub ci: (f64, f64),
}

/// Fisher's exact test of [[a, b], [c, d]], with the interval of the odds ratio at `level`. None when the table is empty.
pub fn fisher_exact(a: u64, b: u64, c: u64, d: u64, level: f64) -> Option<Fisher> {
    let (m, n, k) = ((a + c) as f64, (b + d) as f64, (a + b) as f64);
    if m + n == 0.0 {
        return None;
    }
    let lo = (k - n).max(0.0) as u64;
    let hi = k.min(m) as u64;
    let x = a;
    let support: Vec<u64> = (lo..=hi).collect();
    let log_dc: Vec<f64> = support.iter().map(|&i| ln_choose(m, i as f64) + ln_choose(n, k - i as f64) - ln_choose(m + n, k)).collect();
    // The two-sided p-value at an odds ratio of 1, summing every table as unlikely as the observed one (R's 1e-7 slack).
    let observed = log_dc[(x - lo) as usize].exp();
    let p: f64 = log_dc.iter().map(|l| l.exp()).filter(|&v| v <= observed * (1.0 + 1e-7)).sum::<f64>().min(1.0);

    // The probabilities of the support for the odds ratio exp(log_psi), as weights that sum to one.
    let weights = |log_psi: f64| -> Vec<f64> {
        let raw: Vec<f64> = support.iter().zip(&log_dc).map(|(&i, l)| l + i as f64 * log_psi).collect();
        let top = raw.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let w: Vec<f64> = raw.iter().map(|v| (v - top).exp()).collect();
        let s: f64 = w.iter().sum();
        w.into_iter().map(|v| v / s).collect()
    };
    let mean_at = |log_psi: f64| -> f64 { support.iter().zip(weights(log_psi)).map(|(&i, w)| i as f64 * w).sum() };
    let lower_tail = |log_psi: f64| -> f64 { support.iter().zip(weights(log_psi)).filter(|(&i, _)| i <= x).map(|(_, w)| w).sum() };
    let upper_tail = |log_psi: f64| -> f64 { support.iter().zip(weights(log_psi)).filter(|(&i, _)| i >= x).map(|(_, w)| w).sum() };

    // Bisection on the log odds ratio, over a range wider than any odds ratio a table can carry.
    let solve = |f: &dyn Fn(f64) -> f64, increasing: bool| -> f64 {
        let (mut lo_p, mut hi_p) = (-700.0, 700.0);
        for _ in 0..200 {
            let mid = 0.5 * (lo_p + hi_p);
            if (f(mid) > 0.0) == increasing {
                hi_p = mid;
            } else {
                lo_p = mid;
            }
        }
        0.5 * (lo_p + hi_p)
    };
    let alpha = (1.0 - level) / 2.0;
    let odds_ratio = if x == lo {
        0.0
    } else if x == hi {
        f64::INFINITY
    } else {
        solve(&|t| mean_at(t) - x as f64, true).exp()
    };
    let ci_lower = if x == lo { 0.0 } else { solve(&|t| upper_tail(t) - alpha, true).exp() };
    let ci_upper = if x == hi { f64::INFINITY } else { solve(&|t| lower_tail(t) - alpha, false).exp() };
    let (bf, cf) = (b as f64, c as f64);
    let odds_ratio_sample = if bf * cf == 0.0 { f64::INFINITY } else { a as f64 * d as f64 / (bf * cf) };
    Some(Fisher { p, odds_ratio_sample, odds_ratio, level, ci: (ci_lower, ci_upper) })
}

// ---------------------------------------------------------------- ordered categories: gamma, tau-b, tau-c, Somers' d

/// One measure of association between two ordered categories, with its large-sample standard error and the test that it is zero.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct OrdinalMeasure {
    pub estimate: f64,
    /// The standard error to use for an interval (it is estimated with the measure as it is).
    pub ase: f64,
    /// The standard error when the measure is zero, which the test uses.
    pub ase0: f64,
    pub z: f64,
    /// The two-sided p-value of the hypothesis that the measure is zero, from the normal distribution.
    pub p: f64,
}

/// Measures of association between two ordered categories, from a table of counts whose rows and columns are in their order.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OrdinalAssociation {
    pub n: f64,
    /// The pairs of cases that rank the same way on both variables, and the pairs that rank opposite ways.
    pub concordant: f64,
    pub discordant: f64,
    /// Goodman and Kruskal's gamma: concordant against discordant pairs, ignoring ties.
    pub gamma: OrdinalMeasure,
    /// Kendall's tau-b: corrected for ties, so a square table can reach 1.
    pub tau_b: OrdinalMeasure,
    /// Stuart's tau-c: for tables whose rows and columns are not equal in number.
    pub tau_c: OrdinalMeasure,
    /// Somers' d when the column variable is the one being predicted from the rows.
    pub somers_columns_dependent: OrdinalMeasure,
    /// Somers' d when the row variable is the one being predicted from the columns.
    pub somers_rows_dependent: OrdinalMeasure,
    /// The symmetric Somers' d (the average of the two denominators); its standard error is not given.
    pub somers_symmetric: f64,
}

fn measure(estimate: f64, ase: f64, ase0: f64) -> OrdinalMeasure {
    let z = if ase0 > 0.0 { estimate / ase0 } else { f64::NAN };
    let p = if z.is_finite() { 2.0 * crate::dist::norm_cdf(-z.abs()) } else { f64::NAN };
    OrdinalMeasure { estimate, ase, ase0, z, p }
}

/// Gamma, Kendall's tau-b, Stuart's tau-c, and Somers' d of a table of counts (the formulas of Agresti, 2002, section 2.4,
/// as the SAS/STAT documentation gives them). None for a table with fewer than two non-empty rows or columns, a ragged or
/// negative table, or one with no concordant or discordant pair at all.
pub fn ordinal_association(table: &[Vec<f64>]) -> Option<OrdinalAssociation> {
    let r = table.len();
    let k = table.first()?.len();
    if r < 2 || k < 2 || table.iter().any(|row| row.len() != k) || table.iter().flatten().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    let row_sum: Vec<f64> = table.iter().map(|row| row.iter().sum()).collect();
    let col_sum: Vec<f64> = (0..k).map(|j| table.iter().map(|row| row[j]).sum()).collect();
    let n: f64 = row_sum.iter().sum();
    if row_sum.iter().filter(|v| **v > 0.0).count() < 2 || col_sum.iter().filter(|v| **v > 0.0).count() < 2 {
        return None;
    }
    // For each cell, the cases in cells that rank the same way (a) and the opposite way (d).
    let mut a = vec![vec![0.0; k]; r];
    let mut d = vec![vec![0.0; k]; r];
    for i in 0..r {
        for j in 0..k {
            let (mut same, mut opposite) = (0.0, 0.0);
            for (u, row) in table.iter().enumerate() {
                for (v, &cell) in row.iter().enumerate() {
                    if (u > i && v > j) || (u < i && v < j) {
                        same += cell;
                    }
                    if (u > i && v < j) || (u < i && v > j) {
                        opposite += cell;
                    }
                }
            }
            a[i][j] = same;
            d[i][j] = opposite;
        }
    }
    // P and Q count each pair twice, as the standard errors are written.
    let mut p = 0.0;
    let mut q = 0.0;
    let mut sum_nd2 = 0.0;
    for i in 0..r {
        for j in 0..k {
            p += table[i][j] * a[i][j];
            q += table[i][j] * d[i][j];
            sum_nd2 += table[i][j] * (a[i][j] - d[i][j]).powi(2);
        }
    }
    if p + q <= 0.0 {
        return None;
    }
    let wr = n * n - row_sum.iter().map(|v| v * v).sum::<f64>();
    let wc = n * n - col_sum.iter().map(|v| v * v).sum::<f64>();
    let w = (wr * wc).sqrt();
    let m = r.min(k) as f64;
    let diff = p - q;
    // The part of the null standard errors that the measures share.
    let core = (sum_nd2 - diff * diff / n).max(0.0).sqrt();

    let gamma = diff / (p + q);
    let mut s = 0.0;
    for i in 0..r {
        for j in 0..k {
            s += table[i][j] * (q * a[i][j] - p * d[i][j]).powi(2);
        }
    }
    let gamma_m = measure(gamma, 4.0 / (p + q).powi(2) * s.sqrt(), 2.0 / (p + q) * core);

    let tau_b = diff / w;
    let mut s = 0.0;
    for i in 0..r {
        for j in 0..k {
            let dij = a[i][j] - d[i][j];
            let vij = row_sum[i] * wc + col_sum[j] * wr;
            s += table[i][j] * (2.0 * w * dij + tau_b * vij).powi(2);
        }
    }
    let tau_b_ase = (1.0 / (w * w)) * (s - n.powi(3) * tau_b * tau_b * (wr + wc).powi(2)).max(0.0).sqrt();
    let tau_b_m = measure(tau_b, tau_b_ase, 2.0 / w * core);

    let tau_c = diff * m / (n * n * (m - 1.0));
    let tau_c_ase = 2.0 * m / ((m - 1.0) * n * n) * core;
    let tau_c_m = measure(tau_c, tau_c_ase, tau_c_ase);

    let somers = |wd: f64, outside: &dyn Fn(usize, usize) -> f64| {
        let mut s = 0.0;
        for i in 0..r {
            for j in 0..k {
                s += table[i][j] * (wd * (a[i][j] - d[i][j]) - diff * (n - outside(i, j))).powi(2);
            }
        }
        measure(diff / wd, 2.0 / (wd * wd) * s.sqrt(), 2.0 / wd * core)
    };
    let columns_dependent = somers(wr, &|i, _| row_sum[i]);
    let rows_dependent = somers(wc, &|_, j| col_sum[j]);

    Some(OrdinalAssociation { n, concordant: p / 2.0, discordant: q / 2.0, gamma: gamma_m, tau_b: tau_b_m, tau_c: tau_c_m, somers_columns_dependent: columns_dependent, somers_rows_dependent: rows_dependent, somers_symmetric: diff / ((wr + wc) / 2.0) })
}
