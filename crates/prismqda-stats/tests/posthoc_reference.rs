//! The post-hoc comparisons, the Shapiro-Wilk test, the exact Spearman and Kendall tests, the noncentral t, and the exact
//! interval of Cohen's d must match R (specification 17.4).
//!
//! `reference/posthoc_reference.json` is made by `reference/make_posthoc_reference.R` with base R 4.5.3. Tukey's test is R's own
//! `TukeyHSD`; Games-Howell and Scheffe are not in base R, so the script writes them from their published formulas with R's
//! `ptukey`, `qtukey`, `pt`, and `pf`.

use prismqda_stats::anova::{compare_pairs, one_way, PostHoc};
use prismqda_stats::correlation::{kendall, spearman};
use prismqda_stats::noncentral::{noncentral_t_interval, t_cdf_noncentral};
use prismqda_stats::normality::shapiro_wilk;
use prismqda_stats::studentized::{ptukey, qtukey, qtukey_like_r};
use prismqda_stats::ttest::{independent, one_sample, paired};
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/posthoc_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_posthoc_reference.R to create the reference file")).unwrap()
}

fn floats(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().map(|n| n.as_f64().unwrap()).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap()],
        other => panic!("not numbers: {other}"),
    }
}

fn rows(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(floats).collect()
}

fn relative(got: f64, want: f64) -> f64 {
    if got == want {
        return 0.0;
    }
    (got - want).abs() / want.abs().max(1e-300)
}

/// Two p-values of the studentized range agree within 1e-8 of the value plus 1e-12 absolute: the tail is found as 1 - P
/// with P close to 1, so it cannot be more accurate than about 1e-15 in absolute terms, which is true of R as well.
fn tail_close(what: &str, got: f64, want: f64) {
    assert!((got - want).abs() <= 1e-12 + 1e-8 * want.abs(), "{what}: PrismQDA {got:e}, R {want:e}");
}

/// The worst relative error over the cases, which must be within `tol`.
fn within(name: &str, tol: f64, cases: impl Iterator<Item = (String, f64, f64)>) {
    let (mut worst, mut at, mut n) = (0.0f64, String::new(), 0);
    for (what, got, want) in cases {
        n += 1;
        let e = relative(got, want);
        if e > worst {
            worst = e;
            at = format!("{what}: PrismQDA {got:e}, R {want:e}");
        }
    }
    assert!(n > 4, "{name}: only {n} values compared");
    assert!(worst <= tol, "{name}: the worst relative error is {worst:e} (tolerance {tol:e}) at {at}");
}

#[test]
fn the_studentized_range_matches_r() {
    let d = load();
    within("ptukey", 1e-9, rows(&d["ptukey"]).into_iter().map(|r| (format!("q={} k={} df={}", r[0], r[1], r[2]), ptukey(r[0], r[1], r[2]), r[3])));
    // R's qtukey stops at about 1e-4 in q, so PrismQDA's loop of the same kind matches R's answer, and the full-accuracy
    // solution matches the root of R's own ptukey.
    within("qtukey, R's loop", 1e-9, rows(&d["qtukey"]).into_iter().map(|r| (format!("p={} k={} df={}", r[0], r[1], r[2]), qtukey_like_r(r[0], r[1], r[2]), r[3])));
    within("qtukey, solved", 1e-9, rows(&d["qtukey"]).into_iter().map(|r| (format!("p={} k={} df={}", r[0], r[1], r[2]), qtukey(r[0], r[1], r[2]), r[4])));
    // Against R's own qtukey the difference is its stopping rule: well under 1e-3 of the value.
    within("qtukey against R's printed value", 2e-4, rows(&d["qtukey"]).into_iter().map(|r| (format!("p={} k={} df={}", r[0], r[1], r[2]), qtukey(r[0], r[1], r[2]), r[3])));
}

#[test]
fn tukey_games_howell_and_scheffe_match_r() {
    let d = load();
    let mut seen = 0;
    for set in d["posthoc"].as_array().unwrap() {
        let name = set["name"].as_str().unwrap();
        let groups: Vec<Vec<f64>> = set["groups"].as_array().unwrap().iter().map(floats).collect();
        let a = one_way(&groups).unwrap();
        let tukey = compare_pairs(&a, PostHoc::Tukey, 0.95);
        let gh = compare_pairs(&a, PostHoc::GamesHowell, 0.95);
        let sc = compare_pairs(&a, PostHoc::Scheffe, 0.95);
        let pairs = set["pairs"].as_array().unwrap();
        assert_eq!(pairs.len(), tukey.len(), "{name}: the number of pairs");
        for (i, e) in pairs.iter().enumerate() {
            let at = format!("{name} pair {}-{}", e["i"], e["j"]);
            // R's differences are the later group minus the earlier; PrismQDA's are the earlier minus the later.
            let diff = e["diff"].as_f64().unwrap();
            assert!((tukey[i].difference + diff).abs() < 1e-12, "{at}: difference");
            let tk = floats(&e["tukey"]);
            // [lwr, upr, p adj] as R printed them (4 digits of the quantile), then the full-accuracy interval.
            tail_close(&format!("{at} Tukey p"), tukey[i].p_adjusted, tk[2]);
            within(&format!("{at} Tukey interval (full accuracy)"), 1e-8, [(at.clone(), -tukey[i].ci.1, tk[3]), (at.clone(), -tukey[i].ci.0, tk[4])].into_iter().chain(std::iter::repeat_with(|| (at.clone(), 1.0, 1.0)).take(5)));
            // Against R's printed interval the difference is R's 1e-4 stopping rule.
            assert!((-tukey[i].ci.1 - tk[0]).abs() < 2e-3 * (1.0 + tk[0].abs()), "{at}: Tukey lower limit {} against R's {}", -tukey[i].ci.1, tk[0]);
            let g = floats(&e["games_howell"]);
            within(&format!("{at} Games-Howell"), 1e-8, [(format!("{at} t"), -gh[i].t, g[0]), (format!("{at} df"), gh[i].df, g[1]), (format!("{at} lo"), -gh[i].ci.1, g[3]), (format!("{at} hi"), -gh[i].ci.0, g[4])].into_iter().chain(std::iter::repeat_with(|| (at.clone(), 1.0, 1.0)).take(5)));
            tail_close(&format!("{at} Games-Howell p"), gh[i].p_adjusted, g[2]);
            let s = floats(&e["scheffe"]);
            within(&format!("{at} Scheffe"), 1e-9, [(format!("{at} F"), sc[i].t * sc[i].t / (groups.len() as f64 - 1.0), s[0]), (format!("{at} p"), sc[i].p_adjusted, s[1]), (format!("{at} lo"), -sc[i].ci.1, s[2]), (format!("{at} hi"), -sc[i].ci.0, s[3])].into_iter().chain(std::iter::repeat_with(|| (at.clone(), 1.0, 1.0)).take(5)));
            seen += 1;
        }
    }
    assert!(seen >= 20, "{seen} pairs compared");
}

#[test]
fn shapiro_wilk_matches_r() {
    let d = load();
    let samples = d["shapiro"].as_array().unwrap();
    assert!(samples.len() >= 15);
    for s in samples {
        let x = floats(&s["x"]);
        let r = shapiro_wilk(&x).unwrap_or_else(|| panic!("no result for n = {}", x.len()));
        let (w, p) = (s["w"].as_f64().unwrap(), s["p"].as_f64().unwrap());
        assert!((r.w - w).abs() < 1e-9, "n = {}: W {} against R's {w}", x.len(), r.w);
        assert!(relative(r.p, p) < 1e-6, "n = {}: p {:e} against R's {p:e}", x.len(), r.p);
    }
    assert!(shapiro_wilk(&[1.0, 2.0]).is_none(), "two values");
    assert!(shapiro_wilk(&[4.0, 4.0, 4.0, 4.0]).is_none(), "no spread");
}

#[test]
fn exact_spearman_and_kendall_match_r() {
    let d = load();
    let mut spearman_exact = 0;
    for c in d["exact_cor"].as_array().unwrap() {
        let (x, y) = (floats(&c["x"]), floats(&c["y"]));
        let n = x.len();
        let sp = spearman(&x, &y, 0.95).unwrap();
        assert!((sp.r - c["rho"].as_f64().unwrap()).abs() < 1e-12, "n = {n}: rho");
        assert!(relative(sp.p, c["p_spearman"].as_f64().unwrap()) < 1e-6, "n = {n}: Spearman p {:e} against R's {:e}", sp.p, c["p_spearman"].as_f64().unwrap());
        let tied = |v: &[f64]| {
            let mut s = v.to_vec();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            s.windows(2).any(|w| w[0] == w[1])
        };
        let ties = tied(&x) || tied(&y);
        assert_eq!((sp.exact, sp.edgeworth), (!ties && n <= 9, !ties && n > 9), "n = {n}: how the p-value was found");
        spearman_exact += 1;
        if n < 50 {
            let kd = kendall(&x, &y, 0.95).unwrap();
            assert!((kd.r - c["tau"].as_f64().unwrap()).abs() < 1e-12, "n = {n}: tau");
            assert!(relative(kd.p, c["p_kendall"].as_f64().unwrap()) < 1e-9, "n = {n}: Kendall p {:e} against R's {:e}", kd.p, c["p_kendall"].as_f64().unwrap());
            assert_eq!(kd.exact, !ties, "n = {n}: Kendall is exact only without ties");
        }
    }
    assert!(spearman_exact >= 10);
    // With ties the t approximation is used, as R does.
    let tied = spearman(&[1.0, 2.0, 2.0, 4.0, 5.0], &[2.0, 1.0, 3.0, 5.0, 4.0], 0.95).unwrap();
    assert!(!tied.exact && !tied.edgeworth && tied.statistic.is_some());
}

#[test]
fn the_noncentral_t_matches_r() {
    let d = load();
    within("pt(q, df, ncp)", 1e-8, rows(&d["pnt"]).into_iter().filter(|r| r[3] > 1e-6 && r[3] < 1.0 - 1e-6).map(|r| (format!("q={} df={} ncp={}", r[0], r[1], r[2]), t_cdf_noncentral(r[0], r[1], r[2]), r[3])));
    // Near 0 and 1 it is compared by absolute error: the series stops at an error of 1e-12 (R's own limit), and R warns that
    // it has lost its precision there.
    for r in rows(&d["pnt"]) {
        let got = t_cdf_noncentral(r[0], r[1], r[2]);
        assert!((got - r[3]).abs() < 1e-11, "q={} df={} ncp={}: {got:e} against {:e}", r[0], r[1], r[2], r[3]);
    }
    assert!(noncentral_t_interval(f64::NAN, 5.0, 0.95).is_none());
}

#[test]
fn the_exact_interval_of_d_matches_r() {
    let d = load();
    let mut compared = 0;
    for c in d["d_intervals"].as_array().unwrap() {
        let x = floats(&c["x"]);
        for level_row in c["intervals"].as_array().unwrap() {
            let row = floats(level_row);
            let level = row[0];
            let r = match c["kind"].as_str().unwrap() {
                "one" => one_sample(&x, c["mu"].as_f64().unwrap(), level),
                "paired" => paired(&x, &floats(&c["y"]), level),
                _ => independent(&x, &floats(&c["y"]), false, level),
            }
            .unwrap();
            assert!((r.d - c["d"].as_f64().unwrap()).abs() < 1e-12, "{}: d", c["kind"]);
            within(&format!("{} at {level}", c["kind"]), 1e-8, [("lower".to_string(), r.d_ci.0, row[1]), ("upper".to_string(), r.d_ci.1, row[2])].into_iter().chain(std::iter::repeat_with(|| ("-".to_string(), 1.0, 1.0)).take(5)));
            compared += 1;
        }
    }
    assert_eq!(compared, 12);
}
