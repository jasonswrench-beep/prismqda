//! Agreement between coders (specification 18.3), written from the published definitions and checked
//! against R's `irr` and `irrCAC` packages (see `tests/reference`).
//!
//! The data is a table of ratings: one row per unit, one column per coder, each cell the category that
//! coder gave the unit, or `None` if they did not rate it. Categories are numbered `0..q`.
//!
//! What counts as a rating, and which units are used, follows the conventions of the reference packages:
//!
//! * A unit needs at least two ratings to count towards **observed agreement**.
//! * The **chance-agreement** terms of Scott's pi and Gwet's AC1 use the share of each category among
//!   every unit that has at least one rating.
//! * **Krippendorff's alpha** uses only values that can be paired, so a unit rated once contributes nothing.
//! * **Cohen's kappa** is for exactly two coders and uses the units both rated; **Fleiss' kappa** uses the
//!   units every coder rated.
//!
//! Every coefficient is `None` when it is undefined, such as when chance agreement is perfect because
//! everyone used one category.

use serde::Serialize;

use crate::rng::SplitMix64;

/// A table of ratings. See the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct Ratings {
    categories: usize,
    units: Vec<Vec<Option<usize>>>,
}

impl Ratings {
    /// Checks that the table is rectangular, has at least two coders, and uses only categories below `categories`.
    pub fn new(categories: usize, units: Vec<Vec<Option<usize>>>) -> Result<Ratings, String> {
        if categories < 2 {
            return Err("agreement needs at least two categories".to_string());
        }
        let coders = units.first().map(Vec::len).unwrap_or(0);
        if coders < 2 {
            return Err("agreement needs at least two coders".to_string());
        }
        for (i, row) in units.iter().enumerate() {
            if row.len() != coders {
                return Err(format!("unit {} has {} ratings, but the first unit has {coders}", i + 1, row.len()));
            }
            if let Some(c) = row.iter().flatten().find(|c| **c >= categories) {
                return Err(format!("unit {} uses category {c}, but there are only {categories} categories", i + 1));
            }
        }
        Ok(Ratings { categories, units })
    }

    pub fn categories(&self) -> usize {
        self.categories
    }

    pub fn coders(&self) -> usize {
        self.units.first().map(Vec::len).unwrap_or(0)
    }

    pub fn units(&self) -> &[Vec<Option<usize>>] {
        &self.units
    }
}

/// Every coefficient for one table of ratings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Agreement {
    pub units: usize,
    /// Units with at least two ratings, which count towards observed agreement.
    pub units_compared: usize,
    pub coders: usize,
    /// Observed agreement: the average, over units, of the share of pairs of ratings that match.
    /// With two coders this is the plain percent agreement.
    pub percent_agreement: Option<f64>,
    pub cohen_kappa: Option<f64>,
    pub scott_pi: Option<f64>,
    pub fleiss_kappa: Option<f64>,
    pub fleiss_units: usize,
    pub brennan_prediger: Option<f64>,
    pub gwet_ac1: Option<f64>,
    pub krippendorff_alpha: Option<f64>,
}

/// Which coefficient to bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Coefficient {
    PercentAgreement,
    CohenKappa,
    ScottPi,
    FleissKappa,
    BrennanPrediger,
    GwetAc1,
    KrippendorffAlpha,
}

impl Agreement {
    pub fn get(&self, which: Coefficient) -> Option<f64> {
        match which {
            Coefficient::PercentAgreement => self.percent_agreement,
            Coefficient::CohenKappa => self.cohen_kappa,
            Coefficient::ScottPi => self.scott_pi,
            Coefficient::FleissKappa => self.fleiss_kappa,
            Coefficient::BrennanPrediger => self.brennan_prediger,
            Coefficient::GwetAc1 => self.gwet_ac1,
            Coefficient::KrippendorffAlpha => self.krippendorff_alpha,
        }
    }
}

fn finite(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

/// A chance-corrected coefficient from observed and expected agreement.
fn corrected(observed: f64, expected: f64) -> Option<f64> {
    finite((observed - expected) / (1.0 - expected))
}

/// Everything the coefficients need, as sums over units. Every coefficient is a function of these sums, so the
/// point estimate and each bootstrap resample go through the same arithmetic, and a resample costs one pass over
/// its units instead of several.
struct Sums {
    q: usize,
    coders: usize,
    units: usize,
    /// Units with at least two ratings, and the sum over them of the share of matching pairs.
    compared: f64,
    sum_pa: f64,
    /// Units with at least one rating, and each category's share among them (summed).
    rated: f64,
    pi: Vec<f64>,
    /// For two coders: how often the first put a unit in `a` and the second in `b`, at `a * q + b`.
    table: Vec<f64>,
    table_n: f64,
    /// Units every coder rated: how many, the sum of their pairwise agreement, and each category's total.
    full: f64,
    full_p: f64,
    full_cat: Vec<f64>,
    /// The coincidence matrix of Krippendorff's alpha, at `a * q + b`.
    coincidence: Vec<f64>,
}

impl Sums {
    fn new(q: usize, coders: usize) -> Sums {
        Sums {
            q,
            coders,
            units: 0,
            compared: 0.0,
            sum_pa: 0.0,
            rated: 0.0,
            pi: vec![0.0; q],
            table: if coders == 2 { vec![0.0; q * q] } else { Vec::new() },
            table_n: 0.0,
            full: 0.0,
            full_p: 0.0,
            full_cat: vec![0.0; q],
            coincidence: vec![0.0; q * q],
        }
    }

    /// Adds one unit's row of ratings, counted `weight` times.
    fn add(&mut self, unit: &[Option<usize>], weight: f64) {
        let q = self.q;
        self.units += weight as usize;
        let n = unit.iter().flatten().count();
        if n == 0 {
            return;
        }
        let nf = n as f64;
        self.rated += weight;
        for cat in unit.iter().flatten() {
            self.pi[*cat] += weight / nf;
        }
        if n >= 2 {
            // Ordered pairs of ratings in this unit that match, and the coincidence matrix.
            let share = weight / (nf - 1.0);
            let mut same = 0usize;
            for (i, a) in unit.iter().enumerate() {
                let Some(a) = a else { continue };
                for (j, b) in unit.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let Some(b) = b else { continue };
                    self.coincidence[a * q + b] += share;
                    if a == b {
                        same += 1;
                    }
                }
            }
            self.compared += weight;
            self.sum_pa += weight * same as f64 / (nf * (nf - 1.0));
            if n == self.coders {
                let m = self.coders as f64;
                self.full += weight;
                self.full_p += weight * same as f64 / (m * (m - 1.0));
                for cat in unit.iter().flatten() {
                    self.full_cat[*cat] += weight;
                }
            }
        }
        if self.coders == 2 {
            if let (Some(a), Some(b)) = (unit[0], unit[1]) {
                self.table[a * q + b] += weight;
                self.table_n += weight;
            }
        }
    }

    fn finish(&self) -> Agreement {
        let q = self.q;
        let q_f = q as f64;
        let pa = (self.compared > 0.0).then(|| self.sum_pa / self.compared);
        let pi_k: Vec<f64> = self.pi.iter().map(|p| if self.rated > 0.0 { p / self.rated } else { *p }).collect();
        let pe_pi: f64 = pi_k.iter().map(|p| p * p).sum();
        let pe_ac1: f64 = pi_k.iter().map(|p| p * (1.0 - p)).sum::<f64>() / (q_f - 1.0);
        let (fleiss_kappa, fleiss_units) = self.fleiss();
        Agreement {
            units: self.units,
            units_compared: self.compared as usize,
            coders: self.coders,
            percent_agreement: pa,
            cohen_kappa: self.cohen(),
            scott_pi: pa.and_then(|p| corrected(p, pe_pi)),
            fleiss_kappa,
            fleiss_units,
            brennan_prediger: pa.and_then(|p| corrected(p, 1.0 / q_f)),
            gwet_ac1: pa.and_then(|p| corrected(p, pe_ac1)),
            krippendorff_alpha: self.krippendorff(),
        }
    }

    /// Cohen's kappa for two coders, over the units both rated. The two coders' own category shares give the chance agreement.
    fn cohen(&self) -> Option<f64> {
        if self.coders != 2 || self.table_n == 0.0 {
            return None;
        }
        let (q, n) = (self.q, self.table_n);
        let observed: f64 = (0..q).map(|k| self.table[k * q + k]).sum::<f64>() / n;
        let expected: f64 = (0..q)
            .map(|k| {
                let row: f64 = (0..q).map(|j| self.table[k * q + j]).sum();
                let col: f64 = (0..q).map(|j| self.table[j * q + k]).sum();
                (row / n) * (col / n)
            })
            .sum();
        corrected(observed, expected)
    }

    /// Fleiss' kappa over the units every coder rated, as `irr::kappam.fleiss` does. Returns the value and how many units it used.
    fn fleiss(&self) -> (Option<f64>, usize) {
        let m = self.coders as f64;
        let n = self.full as usize;
        if n < 2 || m < 2.0 {
            return (None, n);
        }
        let p_bar = self.full_p / self.full;
        let pe: f64 = self.full_cat.iter().map(|c| (c / (self.full * m)).powi(2)).sum();
        (corrected(p_bar, pe), n)
    }

    /// Krippendorff's alpha for nominal categories, from the coincidence matrix. Any number of coders, with missing ratings.
    fn krippendorff(&self) -> Option<f64> {
        let q = self.q;
        let o = &self.coincidence;
        let nc: Vec<f64> = (0..q).map(|c| o[c * q..(c + 1) * q].iter().sum()).collect();
        let n: f64 = nc.iter().sum();
        if n <= 1.0 {
            return None;
        }
        let total_observed: f64 = (0..q).map(|c| nc[c] - o[c * q + c]).sum();
        let total_expected: f64 = (0..q).map(|c| nc[c] * (n - nc[c])).sum();
        if total_expected == 0.0 {
            return None;
        }
        finite(1.0 - (n - 1.0) * total_observed / total_expected)
    }
}

/// Computes every coefficient.
pub fn agreement(r: &Ratings) -> Agreement {
    let mut sums = Sums::new(r.categories, r.coders());
    for unit in &r.units {
        sums.add(unit, 1.0);
    }
    sums.finish()
}

/// A confidence interval from resampling units with replacement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
    pub level: f64,
    pub replications: usize,
    /// Resamples where the coefficient was undefined, which are left out.
    pub undefined: usize,
}

/// A percentile bootstrap interval for one coefficient. The same seed always gives the same interval.
pub fn bootstrap_interval(r: &Ratings, which: Coefficient, replications: usize, seed: u64, level: f64) -> Option<Interval> {
    bootstrap_intervals(r, &[which], replications, seed, level).into_iter().next().flatten()
}

/// Percentile bootstrap intervals for several coefficients from the same resamples, in the order asked for.
/// Each interval is exactly what [`bootstrap_interval`] gives for that coefficient and seed.
pub fn bootstrap_intervals(r: &Ratings, which: &[Coefficient], replications: usize, seed: u64, level: f64) -> Vec<Option<Interval>> {
    if r.units.is_empty() || replications == 0 || !(0.0 < level && level < 1.0) {
        return vec![None; which.len()];
    }
    let mut rng = SplitMix64::new(seed);
    let n = r.units.len();
    let mut values: Vec<Vec<f64>> = vec![Vec::with_capacity(replications); which.len()];
    let mut undefined = vec![0usize; which.len()];
    for _ in 0..replications {
        let mut sums = Sums::new(r.categories, r.coders());
        for _ in 0..n {
            sums.add(&r.units[rng.below(n)], 1.0);
        }
        let resample = sums.finish();
        for (k, coefficient) in which.iter().enumerate() {
            match resample.get(*coefficient) {
                Some(v) => values[k].push(v),
                None => undefined[k] += 1,
            }
        }
    }
    values.into_iter().zip(undefined).map(|(v, u)| percentile_interval(v, u, replications, level)).collect()
}

fn percentile_interval(mut values: Vec<f64>, undefined: usize, replications: usize, level: f64) -> Option<Interval> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let tail = (1.0 - level) / 2.0;
    let pick = |p: f64| {
        let pos = p * (values.len() - 1) as f64;
        let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
        values[lo] + (values[hi] - values[lo]) * (pos - lo as f64)
    };
    Some(Interval { low: pick(tail), high: pick(1.0 - tail), level, replications, undefined })
}

/// Agreement about one code that coders may apply or not: each unit is "applied" or "not applied" per coder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeAgreement {
    pub agreement: Agreement,
    /// For two coders: units both applied the code, only the first did, only the second did, neither did.
    pub both: usize,
    pub first_only: usize,
    pub second_only: usize,
    pub neither: usize,
    /// Units where both applied it, out of units where either did (the study's "both applied / either applied").
    pub overlap: Option<f64>,
    /// How lopsided the table is towards "applied" or "not applied": large values explain a low kappa
    /// alongside high percent agreement. `|both - neither| / n`.
    pub prevalence_index: Option<f64>,
    /// How differently the two coders use the code: `|first_only - second_only| / n`.
    pub bias_index: Option<f64>,
    /// Prevalence-adjusted bias-adjusted kappa: `2 * observed agreement - 1`.
    pub pabak: Option<f64>,
}

/// Agreement on whether each coder applied one code to each unit. `applied[unit][coder]` is `None` if that
/// coder did not code the unit at all.
pub fn code_agreement(applied: &[Vec<Option<bool>>]) -> Result<CodeAgreement, String> {
    let units: Vec<Vec<Option<usize>>> = applied.iter().map(|row| row.iter().map(|c| c.map(usize::from)).collect()).collect();
    let r = Ratings::new(2, units)?;
    let a = agreement(&r);
    let (mut both, mut first_only, mut second_only, mut neither) = (0, 0, 0, 0);
    if r.coders() == 2 {
        for row in &r.units {
            if let (Some(x), Some(y)) = (row[0], row[1]) {
                match (x, y) {
                    (1, 1) => both += 1,
                    (1, 0) => first_only += 1,
                    (0, 1) => second_only += 1,
                    _ => neither += 1,
                }
            }
        }
    }
    let n = (both + first_only + second_only + neither) as f64;
    let two = r.coders() == 2 && n > 0.0;
    Ok(CodeAgreement {
        overlap: (both + first_only + second_only > 0 && two).then(|| both as f64 / (both + first_only + second_only) as f64),
        prevalence_index: two.then(|| (both as f64 - neither as f64).abs() / n),
        bias_index: two.then(|| (first_only as f64 - second_only as f64).abs() / n),
        pabak: a.percent_agreement.map(|p| 2.0 * p - 1.0),
        agreement: a,
        both,
        first_only,
        second_only,
        neither,
    })
}

/// How often each pair of categories occurred between two coders, over the units both rated.
/// `table[a][b]` counts units the first coder put in `a` and the second in `b`.
pub fn confusion_table(r: &Ratings, first: usize, second: usize) -> Vec<Vec<usize>> {
    let q = r.categories;
    let mut t = vec![vec![0usize; q]; q];
    for unit in &r.units {
        if let (Some(Some(a)), Some(Some(b))) = (unit.get(first), unit.get(second)) {
            t[*a][*b] += 1;
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two(pairs: &[(usize, usize)], q: usize) -> Ratings {
        Ratings::new(q, pairs.iter().map(|(a, b)| vec![Some(*a), Some(*b)]).collect()).unwrap()
    }

    #[test]
    fn a_textbook_two_by_two_table() {
        // 20 both-yes, 5 first-only, 10 second-only, 15 both-no.
        let mut pairs = vec![(1, 1); 20];
        pairs.extend(vec![(1, 0); 5]);
        pairs.extend(vec![(0, 1); 10]);
        pairs.extend(vec![(0, 0); 15]);
        let a = agreement(&two(&pairs, 2));
        assert_eq!(a.percent_agreement, Some(0.7));
        // Expected agreement: 0.5 * 0.6 + 0.5 * 0.4 = 0.5, so kappa = (0.7 - 0.5) / 0.5 = 0.4.
        assert!((a.cohen_kappa.unwrap() - 0.4).abs() < 1e-12);
        assert!((a.brennan_prediger.unwrap() - 0.4).abs() < 1e-12, "with two categories, 1/q is 0.5 too");
    }

    #[test]
    fn perfect_and_no_agreement() {
        let perfect = agreement(&two(&[(0, 0), (1, 1), (2, 2), (1, 1)], 3));
        for v in [perfect.percent_agreement, perfect.cohen_kappa, perfect.scott_pi, perfect.brennan_prediger, perfect.gwet_ac1, perfect.krippendorff_alpha] {
            assert_eq!(v, Some(1.0));
        }
        // Everyone uses one category: chance agreement is perfect, so the corrected coefficients are undefined.
        let flat = agreement(&two(&[(0, 0); 10], 2));
        assert_eq!(flat.percent_agreement, Some(1.0));
        assert_eq!(flat.cohen_kappa, None);
        assert_eq!(flat.krippendorff_alpha, None);
        // Brennan-Prediger fixes chance at 1/q, so it stays defined.
        assert_eq!(flat.brennan_prediger, Some(1.0));
    }

    #[test]
    fn units_rated_once_or_never_contribute_nothing_to_agreement() {
        let r = Ratings::new(2, vec![vec![Some(0), Some(0)], vec![Some(1), None], vec![None, None], vec![Some(1), Some(1)]]).unwrap();
        let a = agreement(&r);
        assert_eq!((a.units, a.units_compared), (4, 2));
        assert_eq!(a.percent_agreement, Some(1.0));
    }

    #[test]
    fn three_coders_split_two_to_one() {
        let r = Ratings::new(2, vec![vec![Some(0), Some(0), Some(1)]]).unwrap();
        // Three pairs, one of which matches.
        assert!((agreement(&r).percent_agreement.unwrap() - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_tables_are_refused_with_plain_messages() {
        assert!(Ratings::new(1, vec![vec![Some(0), Some(0)]]).unwrap_err().contains("two categories"));
        assert!(Ratings::new(2, vec![vec![Some(0)]]).unwrap_err().contains("two coders"));
        assert!(Ratings::new(2, vec![]).unwrap_err().contains("two coders"));
        assert!(Ratings::new(2, vec![vec![Some(0), Some(1)], vec![Some(0)]]).unwrap_err().contains("unit 2"));
        assert!(Ratings::new(2, vec![vec![Some(0), Some(5)]]).unwrap_err().contains("category 5"));
    }

    #[test]
    fn the_bootstrap_is_repeatable_and_brackets_the_estimate() {
        let mut pairs = vec![(0, 0); 40];
        pairs.extend(vec![(1, 1); 30]);
        pairs.extend(vec![(0, 1); 10]);
        pairs.extend(vec![(1, 0); 10]);
        let r = two(&pairs, 2);
        let est = agreement(&r).krippendorff_alpha.unwrap();
        let a = bootstrap_interval(&r, Coefficient::KrippendorffAlpha, 500, 7, 0.95).unwrap();
        let b = bootstrap_interval(&r, Coefficient::KrippendorffAlpha, 500, 7, 0.95).unwrap();
        assert_eq!(a, b, "the same seed gives the same interval");
        assert!(a.low < est && est < a.high, "{a:?} vs {est}");
        assert!(a.high - a.low > 0.05 && a.high - a.low < 0.5);
        assert_ne!(a, bootstrap_interval(&r, Coefficient::KrippendorffAlpha, 500, 8, 0.95).unwrap(), "another seed gives another interval");
        assert!(bootstrap_interval(&r, Coefficient::CohenKappa, 0, 1, 0.95).is_none());
        assert!(bootstrap_interval(&r, Coefficient::CohenKappa, 10, 1, 1.5).is_none());
    }

    #[test]
    fn per_code_agreement_explains_a_low_kappa_beside_high_agreement() {
        // 85 both applied, 4 only the first, 5 only the second, 6 neither: the study-style "both / either" is 85 / 94.
        let mut rows: Vec<Vec<Option<bool>>> = vec![vec![Some(true), Some(true)]; 85];
        rows.extend(vec![vec![Some(true), Some(false)]; 4]);
        rows.extend(vec![vec![Some(false), Some(true)]; 5]);
        rows.extend(vec![vec![Some(false), Some(false)]; 6]);
        let c = code_agreement(&rows).unwrap();
        assert_eq!((c.both, c.first_only, c.second_only, c.neither), (85, 4, 5, 6));
        assert!((c.overlap.unwrap() - 85.0 / 94.0).abs() < 1e-12);
        assert!((c.prevalence_index.unwrap() - 0.79).abs() < 1e-12);
        assert!((c.bias_index.unwrap() - 0.01).abs() < 1e-12);
        assert!((c.pabak.unwrap() - 0.82).abs() < 1e-12);
        assert_eq!(c.agreement.percent_agreement, Some(0.91));
        assert!(c.agreement.cohen_kappa.unwrap() < 0.53 && c.agreement.gwet_ac1.unwrap() > 0.88);
    }

    #[test]
    fn the_confusion_table_counts_each_pair() {
        let r = two(&[(0, 0), (0, 1), (1, 1), (1, 1), (2, 0)], 3);
        assert_eq!(confusion_table(&r, 0, 1), vec![vec![1, 1, 0], vec![0, 2, 0], vec![1, 0, 0]]);
    }

    #[test]
    fn one_pass_of_resamples_gives_each_coefficient_the_interval_it_would_get_alone() {
        let r = Ratings::new(3, (0..60).map(|i| vec![Some(i % 3), Some(if i % 7 == 0 { (i + 1) % 3 } else { i % 3 }), if i % 5 == 0 { None } else { Some(i % 3) }]).collect()).unwrap();
        let which = [Coefficient::KrippendorffAlpha, Coefficient::FleissKappa, Coefficient::PercentAgreement, Coefficient::GwetAc1];
        let together = bootstrap_intervals(&r, &which, 300, 5, 0.95);
        for (c, iv) in which.iter().zip(together) {
            assert_eq!(iv, bootstrap_interval(&r, *c, 300, 5, 0.95), "{c:?}");
            assert!(iv.is_some());
        }
        assert_eq!(bootstrap_intervals(&r, &which, 0, 5, 0.95), vec![None; 4]);
    }

    #[test]
    fn the_sums_agree_with_a_direct_count_on_a_small_table() {
        // Three units, three coders; unit 3 has one rating and counts for nothing but the shares.
        let r = Ratings::new(2, vec![vec![Some(0), Some(0), Some(1)], vec![Some(1), Some(1), Some(1)], vec![Some(0), None, None]]).unwrap();
        let a = agreement(&r);
        // Unit 1: 1 of 3 pairs match. Unit 2: 3 of 3. Mean (1/3 + 1) / 2.
        assert!((a.percent_agreement.unwrap() - (1.0 / 3.0 + 1.0) / 2.0).abs() < 1e-15);
        assert_eq!((a.units, a.units_compared, a.fleiss_units), (3, 2, 2));
    }
}
