//! Every agreement coefficient must match R (specification 17.4).
//!
//! `reference/agreement_reference.json` is made by `reference/make_agreement_reference.R` with R 4.5.3,
//! `irr` 0.85, and `irrCAC` 1.4. Three sources are compared, each to the precision it can give:
//!
//! * the `irr` package (Cohen, Fleiss, Krippendorff) is compared to 1e-9, except for one known case
//!   where `irr` 0.85 disagrees with the definition of Krippendorff's alpha (see the last test);
//! * the same coefficients written out in plain R (`manual_*`, which the script proved equal to the
//!   packages) are compared to 1e-12;
//! * the `irrCAC` package rounds what it returns to five decimals, so it is compared to 6e-6.

use prismqda_stats::{agreement, Ratings};
use serde_json::Value;

fn load() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/agreement_reference.json");
    let text = std::fs::read_to_string(path).expect("run make_agreement_reference.R to create the reference file");
    serde_json::from_str(&text).expect("the reference file is JSON")
}

fn ratings(d: &Value) -> Ratings {
    let q = d["categories"].as_u64().unwrap() as usize;
    let rows = d["ratings"].as_array().unwrap();
    let units = rows
        .iter()
        .map(|row| row.as_array().unwrap().iter().map(|c| c.as_u64().map(|n| n as usize - 1)).collect())
        .collect();
    Ratings::new(q, units).unwrap()
}

fn close(name: &str, dataset: &str, got: Option<f64>, want: Option<f64>, tol: f64) {
    match (got, want) {
        (Some(g), Some(w)) => assert!((g - w).abs() <= tol, "{dataset}: {name} is {g:.12} but R says {w:.12} (tolerance {tol})"),
        (None, Some(w)) => panic!("{dataset}: {name} is undefined but R says {w}"),
        // R has no value (the package refuses), so there is nothing to compare against.
        (_, None) => {}
    }
}

fn num(e: &Value, key: &str) -> Option<f64> {
    e.get(key).and_then(Value::as_f64)
}

#[test]
fn the_reference_covers_every_kind_of_table() {
    let names: Vec<String> = load().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
    for want in ["two_coders_five_codes", "kappa_paradox", "three_coders_complete", "four_coders_with_gaps", "three_coders_singletons", "perfect_agreement", "chance_level"] {
        assert!(names.iter().any(|n| n == want), "{want} is missing from the reference");
    }
}

#[test]
fn every_coefficient_matches_r() {
    let mut compared = 0;
    for d in load() {
        let name = d["name"].as_str().unwrap();
        let e = &d["expected"];
        let a = agreement(&ratings(&d));

        // Plain-R definitions, to 1e-12.
        close("percent agreement", name, a.percent_agreement, num(e, "manual_percent_agreement"), 1e-12);
        close("Scott's pi", name, a.scott_pi, num(e, "manual_scott_pi"), 1e-12);
        close("Brennan and Prediger's coefficient", name, a.brennan_prediger, num(e, "manual_brennan_prediger"), 1e-12);
        close("Gwet's AC1", name, a.gwet_ac1, num(e, "manual_gwet_ac1"), 1e-12);

        // The irr package, to 1e-9.
        close("Cohen's kappa", name, a.cohen_kappa, num(e, "cohen_kappa"), 1e-9);
        close("percent agreement (irr::agree)", name, a.percent_agreement, num(e, "percent_agreement_simple"), 1e-9);
        close("Fleiss' kappa", name, a.fleiss_kappa, num(e, "fleiss_kappa"), 1e-9);
        close("Krippendorff's alpha (plain R definition)", name, a.krippendorff_alpha, num(e, "manual_krippendorff_alpha"), 1e-12);
        if e["irr_alpha_differs_from_definition"] != true {
            close("Krippendorff's alpha (irr)", name, a.krippendorff_alpha, num(e, "krippendorff_alpha"), 1e-9);
        }
        if let Some(units) = e.get("fleiss_units").and_then(Value::as_u64) {
            assert_eq!(a.fleiss_units as u64, units, "{name}: Fleiss' kappa should use the units every coder rated");
        }

        // The irrCAC package, rounded to five decimals by the package.
        close("percent agreement (irrCAC)", name, a.percent_agreement, num(e, "percent_agreement"), 6e-6);
        close("Scott's pi (irrCAC)", name, a.scott_pi, num(e, "scott_pi"), 6e-6);
        close("Brennan and Prediger (irrCAC)", name, a.brennan_prediger, num(e, "brennan_prediger"), 6e-6);
        close("Gwet's AC1 (irrCAC)", name, a.gwet_ac1, num(e, "gwet_ac1"), 6e-6);
        close("Krippendorff's alpha (irrCAC)", name, a.krippendorff_alpha, num(e, "krippendorff_alpha_irrcac"), 6e-6);
        compared += e.as_object().unwrap().values().filter(|v| v.is_number()).count();
    }
    assert!(compared > 60, "expected to compare dozens of values, compared {compared}");
}

#[test]
fn the_kappa_paradox_shows_up_the_way_the_literature_says() {
    let all = load();
    let d = all.iter().find(|d| d["name"] == "kappa_paradox").unwrap();
    let a = agreement(&ratings(d));
    assert_eq!(a.percent_agreement, Some(0.91));
    assert!(a.cohen_kappa.unwrap() < 0.53, "high agreement but a modest kappa, because almost everything is in one category");
    assert!(a.gwet_ac1.unwrap() > 0.88, "AC1 and Brennan-Prediger are not pulled down by lopsided categories");
    assert!((a.brennan_prediger.unwrap() - 0.82).abs() < 1e-12);
}

#[test]
fn units_nobody_or_one_coder_rated_are_ignored_not_miscounted() {
    let all = load();
    let d = all.iter().find(|d| d["name"] == "three_coders_singletons").unwrap();
    let a = agreement(&ratings(d));
    assert_eq!(a.units, 50);
    assert!(a.units_compared < 40, "{} units were compared", a.units_compared);
    // R's own Krippendorff value agrees even though a third of the units are unusable.
    close("Krippendorff's alpha", "singletons", a.krippendorff_alpha, num(&d["expected"], "krippendorff_alpha"), 1e-9);
}

#[test]
fn the_one_place_the_irr_package_differs_from_the_definition_is_pinned() {
    // On `three_coders_complete`, irr::kripp.alpha 0.85 returns 0.300398, while the definition (worked in
    // exact fractions separately), irrCAC, and PrismQDA all give 0.302347. PrismQDA follows the definition.
    // This test records that disagreement exactly, so that if a newer irr changes it, or a second dataset
    // starts to differ, someone looks.
    let all = load();
    let differing: Vec<&str> = all.iter().filter(|d| d["expected"]["irr_alpha_differs_from_definition"] == true).map(|d| d["name"].as_str().unwrap()).collect();
    assert_eq!(differing, ["three_coders_complete"]);
    let d = all.iter().find(|d| d["name"] == "three_coders_complete").unwrap();
    let a = agreement(&ratings(d));
    assert!((a.krippendorff_alpha.unwrap() - 0.302346794925).abs() < 1e-11);
    let irr = num(&d["expected"], "krippendorff_alpha").unwrap();
    assert!((irr - 0.300398042955).abs() < 1e-9, "irr's own value has moved: {irr}");
}
