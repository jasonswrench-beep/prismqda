//! The bootstrap must match R's `boot` package where the two can be compared, and keep its promises where they cannot.
//!
//! `reference/bootstrap_reference.json` is made by `reference/make_bootstrap_reference.R` with R 4.5.3 and `boot` 1.3.32. R's
//! random numbers are not PrismQDA's, so the resamples cannot be compared; what is compared is everything after them. Given the
//! values R resampled, PrismQDA must give the same standard error, bias, percentile interval, and BCa interval as `boot.ci`
//! does, and its jackknife influence values must be R's. What the resampling itself promises is tested separately: the same seed
//! repeats, the standard error of a mean matches its formula, and a 90% interval covers about 90% of the time.

use prismqda_stats::bootstrap::{bootstrap, bootstrap_groups, intervals_from_replicates, jackknife_influence, BootstrapSpec};
use prismqda_stats::SplitMix64;
use serde_json::Value;

fn load() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/bootstrap_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_bootstrap_reference.R to create the reference file")).unwrap()
}

fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
        other => vec![other.as_f64().unwrap()],
    }
}

fn close(case: &str, what: &str, got: f64, want: f64, tol: f64) {
    let diff = (got - want).abs();
    assert!(diff <= 1e-12 || diff <= tol * want.abs(), "{case}: {what} is {got:e} but R says {want:e} (relative difference {:e}, tolerance {tol:e})", diff / want.abs());
}

#[test]
fn given_the_same_resampled_values_the_intervals_are_those_of_boot_ci() {
    let mut bca_seen = 0;
    for c in load() {
        let name = c["name"].as_str().unwrap();
        let (t0, level) = (nums(&c["estimate"])[0], nums(&c["level"])[0]);
        let reps = nums(&c["replicates"]);
        let r = intervals_from_replicates(t0, &reps, Some(&nums(&c["influence"])), level, reps.len()).unwrap();
        assert_eq!((r.used, r.failures), (reps.len(), 0), "{name}");
        close(name, "bootstrap standard error", r.se, nums(&c["se"])[0], 1e-11);
        close(name, "bias", r.bias, nums(&c["bias"])[0], 1e-9);
        let (p, b) = (nums(&c["percentile"]), nums(&c["bca"]));
        close(name, "percentile low", r.percentile.0, p[0], 1e-9);
        close(name, "percentile high", r.percentile.1, p[1], 1e-9);
        let bca = r.bca.unwrap_or_else(|| panic!("{name}: no BCa interval"));
        close(name, "BCa low", bca.0, b[0], 1e-9);
        close(name, "BCa high", bca.1, b[1], 1e-9);
        bca_seen += 1;
    }
    assert_eq!(bca_seen, 6);
}

/// A statistic as a function of which cases to use.
type Statistic = Box<dyn Fn(&[usize]) -> Option<f64>>;

/// The statistic of each reference case.
fn statistic_of(name: &str, data: &Value) -> Statistic {
    let cols: Vec<Vec<f64>> = match data.as_array().unwrap()[0] {
        Value::Array(_) => data.as_array().unwrap().iter().map(nums).collect(),
        _ => vec![nums(data)],
    };
    let name = name.to_string();
    Box::new(move |idx: &[usize]| {
        let x: Vec<f64> = idx.iter().map(|&i| cols[0][i]).collect();
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        match name.as_str() {
            "mean_of_a_skewed_variable" | "small_sample_few_resamples" => Some(mean),
            "median_of_a_skewed_variable" => prismqda_stats::describe::percentile_sorted(&prismqda_stats::describe::sorted_finite(&x), 0.5),
            "standard_deviation" => Some((x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()),
            "correlation_of_pairs" => {
                let y: Vec<f64> = idx.iter().map(|&i| cols[1][i]).collect();
                let my = y.iter().sum::<f64>() / n;
                let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
                for (a, b) in x.iter().zip(&y) {
                    sxy += (a - mean) * (b - my);
                    sxx += (a - mean).powi(2);
                    syy += (b - my).powi(2);
                }
                (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx * syy).sqrt())
            }
            "mean_difference_of_pairs" => Some(idx.iter().map(|&i| cols[0][i] - cols[1][i]).sum::<f64>() / n),
            other => panic!("{other}"),
        }
    })
}

#[test]
fn the_jackknife_influence_values_are_those_r_computes() {
    for c in load() {
        let name = c["name"].as_str().unwrap();
        let stat = statistic_of(name, &c["data"]);
        let n = nums(&c["influence"]).len();
        let all: Vec<usize> = (0..n).collect();
        let t0 = stat(&all).unwrap();
        close(name, "the estimate", t0, nums(&c["estimate"])[0], 1e-12);
        let ours = jackknife_influence(n, t0, &stat).unwrap();
        for (i, (g, w)) in ours.iter().zip(nums(&c["influence"])).enumerate() {
            close(name, &format!("influence[{i}]"), *g, w, 1e-9);
        }
    }
}

fn spec(seed: u64) -> BootstrapSpec {
    BootstrapSpec { resamples: 2000, seed, level: 0.95 }
}

fn skewed_sample(n: usize, seed: u64) -> Vec<f64> {
    let mut r = SplitMix64::new(seed);
    (0..n).map(|_| -((r.below(1_000_000) as f64 + 0.5) / 1_000_000.0).ln()).collect()
}

fn mean_of(data: &[f64]) -> impl Fn(&[usize]) -> Option<f64> + '_ {
    move |idx: &[usize]| Some(idx.iter().map(|&i| data[i]).sum::<f64>() / idx.len() as f64)
}

#[test]
fn the_same_seed_repeats_and_another_seed_does_not() {
    let data = skewed_sample(30, 7);
    let a = bootstrap(30, &spec(11), mean_of(&data)).unwrap();
    let b = bootstrap(30, &spec(11), mean_of(&data)).unwrap();
    let c = bootstrap(30, &spec(12), mean_of(&data)).unwrap();
    assert_eq!(a, b, "a saved seed gives the same intervals every time");
    assert_ne!(a.percentile, c.percentile);
    assert!((a.percentile.0 - c.percentile.0).abs() < 0.2, "yet the two are about the same");
}

#[test]
fn the_bootstrap_standard_error_of_a_mean_matches_its_formula() {
    let data = skewed_sample(40, 3);
    let n = 40.0;
    let mean = data.iter().sum::<f64>() / n;
    let sd = (data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
    // The bootstrap resamples the cases as they are, so its standard error is the plug-in one: sd * sqrt((n - 1) / n) / sqrt(n).
    let expected = sd * ((n - 1.0) / n).sqrt() / n.sqrt();
    let r = bootstrap(40, &BootstrapSpec { resamples: 10_000, seed: 5, level: 0.95 }, mean_of(&data)).unwrap();
    assert!((r.se - expected).abs() / expected < 0.03, "bootstrap {} against formula {}", r.se, expected);
    assert!(r.bias.abs() < 0.05 * expected * 3.0, "the mean has almost no bias: {}", r.bias);
    assert!(r.percentile.0 < mean && mean < r.percentile.1);
    let bca = r.bca.unwrap();
    assert!(bca.0 < mean && mean < bca.1);
}

#[test]
fn a_90_percent_interval_covers_the_truth_about_90_percent_of_the_time() {
    // Data from a normal distribution (Box-Muller from the seeded generator), true mean 0, n = 20.
    let mut rng = SplitMix64::new(2024);
    let (mut percentile_hits, mut bca_hits, trials) = (0, 0, 300);
    for t in 0..trials {
        let data: Vec<f64> = (0..20)
            .map(|_| {
                let (u1, u2) = ((rng.below(1_000_000) as f64 + 0.5) / 1e6, (rng.below(1_000_000) as f64 + 0.5) / 1e6);
                (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
            })
            .collect();
        let r = bootstrap(20, &BootstrapSpec { resamples: 1000, seed: t as u64, level: 0.90 }, mean_of(&data)).unwrap();
        percentile_hits += usize::from(r.percentile.0 <= 0.0 && 0.0 <= r.percentile.1);
        bca_hits += usize::from(r.bca.is_some_and(|b| b.0 <= 0.0 && 0.0 <= b.1));
    }
    let (p, b) = (percentile_hits as f64 / trials as f64, bca_hits as f64 / trials as f64);
    assert!((0.84..=0.96).contains(&p), "the percentile interval covered {p}");
    assert!((0.84..=0.96).contains(&b), "the BCa interval covered {b}");
}

#[test]
fn a_statistic_that_is_sometimes_undefined_is_counted_not_hidden() {
    // A correlation is undefined for a resample in which one variable happens to be the same in every case drawn. With few
    // distinct values that happens often enough to count.
    let x = [1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 3.0];
    let y = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
    let cor = |idx: &[usize]| {
        let n = idx.len() as f64;
        let (a, b): (Vec<f64>, Vec<f64>) = (idx.iter().map(|&i| x[i]).collect(), idx.iter().map(|&i| y[i]).collect());
        let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
        let (sab, saa, sbb) = a.iter().zip(&b).fold((0.0, 0.0, 0.0), |s, (p, q)| (s.0 + (p - ma) * (q - mb), s.1 + (p - ma).powi(2), s.2 + (q - mb).powi(2)));
        (saa > 0.0 && sbb > 0.0).then(|| sab / (saa * sbb).sqrt())
    };
    let r = bootstrap(10, &spec(1), cor).unwrap();
    assert!(r.failures > 0 && r.used + r.failures == 2000, "{} failed of 2000", r.failures);
}

#[test]
fn refusals_and_the_groups_bootstrap() {
    let data = skewed_sample(10, 1);
    assert!(bootstrap(2, &spec(1), mean_of(&data)).is_none(), "two cases are too few");
    assert!(bootstrap(10, &BootstrapSpec { resamples: 50, seed: 1, level: 0.95 }, mean_of(&data)).is_none(), "fewer than 100 resamples");
    assert!(bootstrap(10, &BootstrapSpec { resamples: 500, seed: 1, level: 1.5 }, mean_of(&data)).is_none());
    assert!(bootstrap(10, &spec(1), |_| None).is_none(), "undefined on the cases as they are");

    // The difference of two group means, resampling each group within itself: every resample keeps the groups' sizes.
    let (a, b) = (skewed_sample(12, 21), skewed_sample(15, 22).iter().map(|v| v + 1.0).collect::<Vec<_>>());
    let diff = |idx: &[Vec<usize>]| {
        assert_eq!((idx[0].len(), idx[1].len()), (12, 15), "each resample keeps the group sizes");
        let m = |g: &[f64], i: &[usize]| i.iter().map(|&k| g[k]).sum::<f64>() / i.len() as f64;
        Some(m(&a, &idx[0]) - m(&b, &idx[1]))
    };
    let r = bootstrap_groups(&[12, 15], &spec(9), diff).unwrap();
    assert!(r.bca.is_none(), "BCa is not offered when groups are resampled separately");
    assert!(r.percentile.0 < r.estimate && r.estimate < r.percentile.1);
    assert!(r.estimate < 0.0, "the second group is a unit higher on average");
    assert!(bootstrap_groups(&[12, 1], &spec(9), |_| Some(0.0)).is_none(), "a group of one cannot be resampled");
}

#[test]
fn several_statistics_from_the_same_resamples_equal_each_one_alone() {
    use prismqda_stats::bootstrap::bootstrap_many;
    let data = skewed_sample(25, 4);
    let mean = |idx: &[usize]| idx.iter().map(|&i| data[i]).sum::<f64>() / idx.len() as f64;
    let max = |idx: &[usize]| idx.iter().map(|&i| data[i]).fold(f64::NEG_INFINITY, f64::max);
    let both = bootstrap_many(25, 2, &spec(8), |idx| Some(vec![mean(idx), max(idx)]));
    let a = bootstrap(25, &spec(8), |idx| Some(mean(idx))).unwrap();
    let b = bootstrap(25, &spec(8), |idx| Some(max(idx))).unwrap();
    assert_eq!(both[0].as_ref().unwrap(), &a);
    assert_eq!(both[1].as_ref().unwrap(), &b);
    assert!(bootstrap_many(25, 2, &spec(8), |_| Some(vec![1.0])).iter().all(Option::is_none), "a wrong number of values is undefined");
}

#[test]
fn a_resample_that_equals_the_estimate_to_rounding_is_not_below_it() {
    // A resample that is the cases in another order gives the estimate again, up to the order of the sums. Whether such a value counts as below
    // the estimate (the bias correction of BCa) must not depend on that rounding, or two programs, or two summation orders, would disagree.
    let estimate = 20.0;
    let influence: Vec<f64> = (0..12).map(|i| (i as f64 - 5.5) / 3.0).collect();
    let exact: Vec<f64> = (0..41).map(|i| i as f64).collect();
    let mut rounded = exact.clone();
    rounded[20] = 20.0 - 4e-15;
    let mut above = exact.clone();
    above[20] = 20.0 + 4e-15;
    let bca = |reps: &[f64]| intervals_from_replicates(estimate, reps, Some(&influence), 0.95, reps.len()).unwrap().bca.unwrap();
    let (a, b, c) = (bca(&exact), bca(&rounded), bca(&above));
    for other in [b, c] {
        assert!((a.0 - other.0).abs() < 1e-9 && (a.1 - other.1).abs() < 1e-9, "{a:?} against {other:?}");
    }
    // A value that is really below the estimate still counts.
    let mut lower = exact.clone();
    lower[20] = 19.0;
    let d = bca(&lower);
    assert!((a.0 - d.0).abs() > 1e-6 || (a.1 - d.1).abs() > 1e-6, "{a:?} against {d:?}");
}
