//! The measures of association between ordered categories must match the values made by base R (specification 17.4).
//!
//! `reference/ordinal_reference.json` is made by `reference/make_ordinal_reference.R`. The point estimates are found there by
//! looking at every pair of cases (and tau-b is also R's own `cor(method = "kendall")`); the standard errors are the large-sample
//! formulas, each checked in the script against the spread of the measure over simulated tables.

use prismqda_stats::contingency::ordinal_association;
use serde_json::Value;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/ordinal_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_ordinal_reference.R to create the reference file")).unwrap()
}

fn table(v: &Value) -> Vec<Vec<f64>> {
    // A one-row or one-column table is written by R's JSON writer as a flat list; none of these is that.
    v.as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()).collect()
}

fn close(what: &str, got: f64, want: f64) {
    let e = (got - want).abs() / want.abs().max(1e-12);
    assert!(e < 1e-10 || (got - want).abs() < 1e-13, "{what}: PrismQDA {got:e}, R {want:e}");
}

#[test]
fn gamma_tau_and_somers_d_match_r() {
    let d = load();
    let sets = d.as_array().unwrap();
    assert!(sets.len() >= 8);
    for s in sets {
        let name = s["name"].as_str().unwrap();
        let e = &s["expected"];
        let r = ordinal_association(&table(&s["table"])).unwrap_or_else(|| panic!("{name}: no result"));
        let f = |k: &str| e[k].as_f64().unwrap_or_else(|| panic!("{name}: no {k}"));
        close(&format!("{name}: n"), r.n, f("n"));
        close(&format!("{name}: concordant"), r.concordant, f("concordant"));
        close(&format!("{name}: discordant"), r.discordant, f("discordant"));
        for (label, m, key) in [
            ("gamma", r.gamma, "gamma"),
            ("tau-b", r.tau_b, "tau_b"),
            ("tau-c", r.tau_c, "tau_c"),
        ] {
            close(&format!("{name}: {label}"), m.estimate, f(key));
            close(&format!("{name}: {label} ASE"), m.ase, f(&format!("{key}_ase")));
            close(&format!("{name}: {label} ASE0"), m.ase0, f(&format!("{key}_ase0")));
            close(&format!("{name}: {label} z"), m.z, f(&format!("{key}_z")));
            close(&format!("{name}: {label} p"), m.p, f(&format!("{key}_p")));
        }
        for (label, m, est, key) in [
            ("Somers d (columns dependent)", r.somers_columns_dependent, "somers_columns_dependent", "somers_cd"),
            ("Somers d (rows dependent)", r.somers_rows_dependent, "somers_rows_dependent", "somers_rd"),
        ] {
            close(&format!("{name}: {label}"), m.estimate, f(est));
            close(&format!("{name}: {label} ASE"), m.ase, f(&format!("{key}_ase")));
            close(&format!("{name}: {label} ASE0"), m.ase0, f(&format!("{key}_ase0")));
            close(&format!("{name}: {label} p"), m.p, f(&format!("{key}_p")));
        }
        close(&format!("{name}: symmetric d"), r.somers_symmetric, f("somers_symmetric"));
    }
}

#[test]
fn the_textbook_example_gives_the_textbook_gamma() {
    // Agresti (2002), Table 2.8, income by job satisfaction: gamma = 0.221.
    let t = vec![vec![1.0, 2.0, 1.0, 0.0], vec![3.0, 3.0, 6.0, 1.0], vec![10.0, 10.0, 14.0, 9.0], vec![6.0, 7.0, 12.0, 11.0]];
    let r = ordinal_association(&t).unwrap();
    assert!((r.gamma.estimate - 0.221).abs() < 0.0005, "gamma {}", r.gamma.estimate);
    assert_eq!(r.n, 96.0);
}

#[test]
fn a_table_that_cannot_be_measured_gives_nothing() {
    assert!(ordinal_association(&[vec![5.0, 3.0]]).is_none(), "one row");
    assert!(ordinal_association(&[vec![5.0, 0.0], vec![3.0, 0.0]]).is_none(), "one column holds every case");
    assert!(ordinal_association(&[vec![5.0, 3.0], vec![2.0]]).is_none(), "ragged");
    assert!(ordinal_association(&[vec![5.0, -3.0], vec![2.0, 1.0]]).is_none(), "negative");
    assert!(ordinal_association(&[vec![4.0, 0.0], vec![0.0, 6.0]]).is_some(), "a perfect table is measured");
}
