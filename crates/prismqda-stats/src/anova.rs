//! One-way analysis of variance: the classic F test, Welch's test for unequal variances, Levene's test of equal
//! variances, effect sizes, and pairwise comparisons.

use crate::adjust::{p_adjust, Adjustment};
use crate::describe::{mean, percentile_sorted, sorted_finite, variance};
use crate::dist::{f_quantile, f_sf, t_quantile, t_two_sided};
use crate::studentized::{ptukey_upper, qtukey};

/// One group's size, mean, and standard deviation (the deviation is None for a group of one).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct GroupStats {
    pub n: usize,
    pub mean: f64,
    pub sd: Option<f64>,
}

/// An F test with its degrees of freedom.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct FTest {
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

/// The one-way analysis of variance of several groups.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OneWay {
    pub groups: Vec<GroupStats>,
    pub n: usize,
    pub ss_between: f64,
    pub ss_within: f64,
    pub ss_total: f64,
    pub df_between: f64,
    pub df_within: f64,
    pub ms_between: f64,
    pub ms_within: f64,
    pub test: FTest,
    /// The share of the variance the groups explain (SS between over SS total).
    pub eta_squared: f64,
    /// The less biased estimate of the same share.
    pub omega_squared: f64,
    /// Welch's F test, which does not assume equal variances. Needs two values in every group and a spread in every group.
    pub welch: Option<FTest>,
    /// Levene's test of equal variances, with deviations from the group median (the Brown-Forsythe form).
    pub levene: Option<FTest>,
}

/// The one-way analysis of variance. None with fewer than two groups, an empty group, no residual degrees of freedom, or
/// no variation inside the groups.
pub fn one_way(groups: &[Vec<f64>]) -> Option<OneWay> {
    let k = groups.len();
    if k < 2 || groups.iter().any(|g| g.is_empty()) {
        return None;
    }
    let n: usize = groups.iter().map(Vec::len).sum();
    if n <= k {
        return None;
    }
    let stats: Vec<GroupStats> = groups.iter().map(|g| GroupStats { n: g.len(), mean: mean(g).unwrap_or(f64::NAN), sd: variance(g).map(f64::sqrt) }).collect();
    let grand = groups.iter().flatten().sum::<f64>() / n as f64;
    let ss_between: f64 = stats.iter().map(|s| s.n as f64 * (s.mean - grand).powi(2)).sum();
    let ss_within: f64 = groups.iter().zip(&stats).map(|(g, s)| g.iter().map(|v| (v - s.mean).powi(2)).sum::<f64>()).sum();
    if ss_within == 0.0 {
        return None;
    }
    let (df_between, df_within) = ((k - 1) as f64, (n - k) as f64);
    let (ms_between, ms_within) = (ss_between / df_between, ss_within / df_within);
    let f = ms_between / ms_within;
    let ss_total = ss_between + ss_within;
    Some(OneWay {
        groups: stats.clone(),
        n,
        ss_between,
        ss_within,
        ss_total,
        df_between,
        df_within,
        ms_between,
        ms_within,
        test: FTest { f, df1: df_between, df2: df_within, p: f_sf(f, df_between, df_within) },
        eta_squared: ss_between / ss_total,
        omega_squared: (ss_between - df_between * ms_within) / (ss_total + ms_within),
        welch: welch(groups, &stats),
        levene: levene(groups),
    })
}

fn welch(groups: &[Vec<f64>], stats: &[GroupStats]) -> Option<FTest> {
    if groups.iter().any(|g| g.len() < 2) {
        return None;
    }
    let k = groups.len() as f64;
    let n: Vec<f64> = stats.iter().map(|s| s.n as f64).collect();
    let v: Vec<f64> = stats.iter().map(|s| s.sd.map(|d| d * d)).collect::<Option<_>>()?;
    if v.iter().any(|&v| v <= 0.0) {
        return None;
    }
    let w: Vec<f64> = n.iter().zip(&v).map(|(n, v)| n / v).collect();
    let sum_w: f64 = w.iter().sum();
    let weighted_mean = w.iter().zip(stats).map(|(w, s)| w * s.mean).sum::<f64>() / sum_w;
    let tmp = w.iter().zip(&n).map(|(w, n)| (1.0 - w / sum_w).powi(2) / (n - 1.0)).sum::<f64>() / (k * k - 1.0);
    let numerator: f64 = w.iter().zip(stats).map(|(w, s)| w * (s.mean - weighted_mean).powi(2)).sum();
    let f = numerator / ((k - 1.0) * (1.0 + 2.0 * (k - 2.0) * tmp));
    let df2 = 1.0 / (3.0 * tmp);
    Some(FTest { f, df1: k - 1.0, df2, p: f_sf(f, k - 1.0, df2) })
}

fn levene(groups: &[Vec<f64>]) -> Option<FTest> {
    let deviations: Vec<Vec<f64>> = groups
        .iter()
        .map(|g| {
            let m = percentile_sorted(&sorted_finite(g), 0.5)?;
            Some(g.iter().map(|v| (v - m).abs()).collect())
        })
        .collect::<Option<_>>()?;
    let n: usize = deviations.iter().map(Vec::len).sum();
    let k = deviations.len();
    if n <= k {
        return None;
    }
    let grand = deviations.iter().flatten().sum::<f64>() / n as f64;
    let ss_between: f64 = deviations.iter().map(|g| g.len() as f64 * (mean(g).unwrap_or(0.0) - grand).powi(2)).sum();
    let ss_within: f64 = deviations.iter().map(|g| {
        let m = mean(g).unwrap_or(0.0);
        g.iter().map(|v| (v - m).powi(2)).sum::<f64>()
    }).sum();
    if ss_within == 0.0 {
        return None;
    }
    let (df1, df2) = ((k - 1) as f64, (n - k) as f64);
    let f = (ss_between / df1) / (ss_within / df2);
    Some(FTest { f, df1, df2, p: f_sf(f, df1, df2) })
}

/// One comparison of two groups after an analysis of variance, using the pooled within-group variance.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Pairwise {
    /// Indexes of the two groups in the order they were given.
    pub a: usize,
    pub b: usize,
    /// Mean of group a minus mean of group b.
    pub difference: f64,
    pub se: f64,
    pub t: f64,
    pub df: f64,
    pub p: f64,
    /// The p-value adjusted for the number of comparisons by the chosen method.
    pub p_adjusted: f64,
    /// The interval of the difference, widened for the number of comparisons if Bonferroni was chosen.
    pub ci: (f64, f64),
}

/// All pairs of groups compared with t tests on the pooled variance of the analysis of variance (R's
/// `pairwise.t.test(pool.sd = TRUE)`), with the p-values adjusted by `adjustment`. The interval is at `level`, with the
/// Bonferroni widening when that adjustment is chosen and no widening otherwise.
pub fn pairwise(anova: &OneWay, adjustment: Adjustment, level: f64) -> Vec<Pairwise> {
    let k = anova.groups.len();
    let mut out = Vec::new();
    for a in 0..k {
        for b in (a + 1)..k {
            let (ga, gb) = (anova.groups[a], anova.groups[b]);
            let difference = ga.mean - gb.mean;
            let se = (anova.ms_within * (1.0 / ga.n as f64 + 1.0 / gb.n as f64)).sqrt();
            let t = difference / se;
            out.push(Pairwise { a, b, difference, se, t, df: anova.df_within, p: t_two_sided(t, anova.df_within), p_adjusted: f64::NAN, ci: (f64::NAN, f64::NAN) });
        }
    }
    let raw: Vec<f64> = out.iter().map(|c| c.p).collect();
    let adjusted = p_adjust(&raw, adjustment);
    let comparisons = out.len().max(1) as f64;
    let alpha = 1.0 - level;
    let alpha = if adjustment == Adjustment::Bonferroni { alpha / comparisons } else { alpha };
    let q = t_quantile(1.0 - alpha / 2.0, anova.df_within);
    for (c, adj) in out.iter_mut().zip(adjusted) {
        c.p_adjusted = adj;
        c.ci = (c.difference - q * c.se, c.difference + q * c.se);
    }
    out
}

/// How the pairs of groups are compared after an analysis of variance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostHoc {
    /// t tests on the pooled variance with the p-values adjusted by one of the usual methods (R's `pairwise.t.test`).
    Adjusted(Adjustment),
    /// Tukey's honestly significant difference (Tukey-Kramer when the groups differ in size), on the pooled variance.
    Tukey,
    /// Games-Howell, which does not assume equal variances or equal sizes.
    GamesHowell,
    /// Scheffe's test, the most cautious of these: it holds for every contrast, not only the pairs.
    Scheffe,
}

/// All pairs of groups compared by `method`, each as the mean of the first minus the mean of the second. For Tukey,
/// Games-Howell, and Scheffe `p` is the unadjusted two-sided t p-value and `p_adjusted` the p-value of the test itself, and
/// the interval is the simultaneous one at `level` for all the pairs together.
pub fn compare_pairs(anova: &OneWay, method: PostHoc, level: f64) -> Vec<Pairwise> {
    let k = anova.groups.len();
    let kf = k as f64;
    let mut out = Vec::new();
    match method {
        PostHoc::Adjusted(adjustment) => return pairwise(anova, adjustment, level),
        PostHoc::Tukey | PostHoc::Scheffe => {
            let crit = match method {
                PostHoc::Tukey => qtukey(level, kf, anova.df_within) / std::f64::consts::SQRT_2,
                _ => ((kf - 1.0) * f_quantile(level, kf - 1.0, anova.df_within)).sqrt(),
            };
            for a in 0..k {
                for b in (a + 1)..k {
                    let (ga, gb) = (anova.groups[a], anova.groups[b]);
                    let difference = ga.mean - gb.mean;
                    let se = (anova.ms_within * (1.0 / ga.n as f64 + 1.0 / gb.n as f64)).sqrt();
                    let t = difference / se;
                    let p_adjusted = if method == PostHoc::Tukey { ptukey_upper(t.abs() * std::f64::consts::SQRT_2, kf, anova.df_within) } else { f_sf(t * t / (kf - 1.0), kf - 1.0, anova.df_within) };
                    out.push(Pairwise { a, b, difference, se, t, df: anova.df_within, p: t_two_sided(t, anova.df_within), p_adjusted, ci: (difference - crit * se, difference + crit * se) });
                }
            }
        }
        PostHoc::GamesHowell => {
            for a in 0..k {
                for b in (a + 1)..k {
                    let (ga, gb) = (anova.groups[a], anova.groups[b]);
                    let difference = ga.mean - gb.mean;
                    let (Some(sa), Some(sb)) = (ga.sd, gb.sd) else {
                        out.push(Pairwise { a, b, difference, se: f64::NAN, t: f64::NAN, df: f64::NAN, p: f64::NAN, p_adjusted: f64::NAN, ci: (f64::NAN, f64::NAN) });
                        continue;
                    };
                    let (va, vb) = (sa * sa / ga.n as f64, sb * sb / gb.n as f64);
                    let se = (va + vb).sqrt();
                    let t = difference / se;
                    let df = (va + vb).powi(2) / (va * va / (ga.n as f64 - 1.0) + vb * vb / (gb.n as f64 - 1.0));
                    let crit = qtukey(level, kf, df) / std::f64::consts::SQRT_2;
                    out.push(Pairwise { a, b, difference, se, t, df, p: t_two_sided(t, df), p_adjusted: ptukey_upper(t.abs() * std::f64::consts::SQRT_2, kf, df), ci: (difference - crit * se, difference + crit * se) });
                }
            }
        }
    }
    out
}
