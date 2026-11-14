//! Samples, agreement runs, reports, and the lock on sampled units (specification 18.3 and 6.1).

use std::collections::HashSet;

use prismqda_engine::{
    AgreementRequest, CoderRole, Error, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SourceType, Stratify,
    UnitKind,
};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

struct Fx {
    p: Project,
    source: i64,
    units: Vec<i64>,
    codes: Vec<i64>,
}

/// One source of `n` lines (`u000`, `u001`, ...), one unit per line, and the named codes.
fn fixture(dir: &TempDir, n: usize, code_names: &[&str]) -> Fx {
    let mut p = Project::create(dir.path().join("rel.pqda"), &NewProject::new("Reliability"), &opts()).unwrap();
    let text: String = (0..n).map(|i| format!("u{i:03}\n")).collect();
    let source = p.add_source(&NewSource::new("Doc", SourceType::Document, text)).unwrap();
    let units = (0..n as i64).map(|i| p.add_unit(source, UnitKind::Paragraph, 5 * i, 5 * i + 4, None).unwrap()).collect();
    let codes = code_names.iter().map(|c| p.add_code(&NewCode::new(*c)).unwrap()).collect();
    Fx { p, source, units, codes }
}

/// Coder `coder` puts code number `code` (an index into the fixture's codes) on unit number `unit`.
fn code(f: &mut Fx, unit: usize, code: usize, coder: i64) {
    let mut c = NewCoding::new(f.source, 5 * unit as i64, 5 * unit as i64 + 4, f.codes[code], coder);
    c.unit_id = Some(f.units[unit]);
    f.p.apply_coding(&c).unwrap();
}

fn human(f: &mut Fx, name: &str) -> i64 {
    f.p.add_coder(&NewCoder::human(name)).unwrap()
}

fn model(f: &mut Fx, name: &str, model_name: &str, version: &str) -> i64 {
    f.p.add_coder(&NewCoder {
        name: name.to_string(),
        role: CoderRole::Model,
        local_id: None,
        provider: Some("test".to_string()),
        model_name: Some(model_name.to_string()),
        model_version: Some(version.to_string()),
    })
    .unwrap()
}

fn request(coders: &[i64]) -> AgreementRequest {
    serde_json::from_value(json!({ "coder_ids": coders, "bootstrap_replications": 200 })).unwrap()
}

/// Twenty units, two codes: the coders agree on sixteen and swap A and B on four.
fn twenty_with_four_disagreements(dir: &TempDir) -> (Fx, i64, i64) {
    let mut f = fixture(dir, 20, &["A", "B"]);
    let x = human(&mut f, "Ada");
    let y = human(&mut f, "Ben");
    for u in 0..20 {
        let first = if u < 10 { 0 } else { 1 };
        code(&mut f, u, first, x);
        let second = match u {
            0 | 1 => 1,
            10 | 11 => 0,
            _ => first,
        };
        code(&mut f, u, second, y);
    }
    (f, x, y)
}

// ---- agreement --------------------------------------------------------------------------------

#[test]
fn two_people_who_agree_on_sixteen_of_twenty_units() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    let r = f.p.agreement_run(&request(&[x, y])).unwrap();

    assert_eq!(r.units_in_sample, 20);
    let o = r.overall.as_ref().unwrap();
    assert!((o.percent_agreement.unwrap() - 0.8).abs() < 1e-12);
    // Both coders use A and B equally often, so chance agreement is one half and kappa is (0.8 - 0.5) / 0.5.
    assert!((o.cohen_kappa.unwrap() - 0.6).abs() < 1e-12);
    // Worked by hand: 1 - 39 * 8 / (20 * 20 + 20 * 20) = 0.61.
    assert!((o.krippendorff_alpha.unwrap() - 0.61).abs() < 1e-12);
    assert_eq!(r.confusion, Some(vec![vec![8, 2], vec![2, 8]]));
    assert_eq!(r.confusion_codes, ["A", "B"]);
    assert_eq!(r.disagreements, 4);
    assert_eq!(r.reporting_term, "intercoder_reliability");
    assert!(r.notes.is_empty(), "{:?}", r.notes);

    let iv = &r.intervals["krippendorff_alpha"];
    assert!(iv.low < 0.61 && 0.61 < iv.high, "{iv:?}");
    assert_eq!(iv.replications, 200);
}

#[test]
fn the_report_names_every_coder_the_term_and_the_target() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    let r = f.p.agreement_run(&request(&[x, y])).unwrap();
    let t = &r.report_text;
    assert!(t.contains("intercoder reliability"), "{t}");
    assert!(t.contains("Ada (human)") && t.contains("Ben (human)"), "{t}");
    assert!(t.contains("Units compared: 20"), "{t}");
    assert!(t.contains("0.800"), "{t}");
    assert!(t.contains("is below the project target of 0.80"), "alpha 0.61 is under the default target: {t}");
    assert!(t.contains("Units the coders disagreed on: 4"), "{t}");
    assert!(!t.contains('\u{2014}'), "no em dashes in generated prose");
}

#[test]
fn the_same_request_gives_the_same_interval_every_time() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    let a = f.p.agreement_run(&request(&[x, y])).unwrap();
    let b = f.p.agreement_run(&request(&[x, y])).unwrap();
    assert_eq!(a.intervals, b.intervals);
    assert_ne!(a.run_id, b.run_id);
    let mut other = request(&[x, y]);
    other.seed = 7;
    let c = f.p.agreement_run(&other).unwrap();
    assert_ne!(a.intervals["krippendorff_alpha"], c.intervals["krippendorff_alpha"], "a different seed resamples differently");
}

#[test]
fn a_model_in_the_comparison_makes_it_concordance_not_reliability() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 10, &["A", "B"]);
    let x = human(&mut f, "Ada");
    let m = model(&mut f, "Model A", "Model A", "5.5");
    for u in 0..10 {
        code(&mut f, u, u % 2, x);
        code(&mut f, u, u % 2, m);
    }
    let r = f.p.agreement_run(&request(&[x, m])).unwrap();
    assert_eq!(r.reporting_term, "concordance");
    assert!(r.report_text.contains("concordance"), "{}", r.report_text);
    assert!(r.report_text.contains("Model A (model, Model A version 5.5)"), "{}", r.report_text);
    assert!(r.notes.iter().all(|n| !n.contains("Limitation")), "one model is not the independence problem: {:?}", r.notes);

    // Choosing the other word is allowed, with a note that says why people usually do not.
    let mut forced = request(&[x, m]);
    forced.reporting_term = Some("intercoder_reliability".to_string());
    let r = f.p.agreement_run(&forced).unwrap();
    assert_eq!(r.reporting_term, "intercoder_reliability");
    assert!(r.notes.iter().any(|n| n.contains("not an independent human judge")), "{:?}", r.notes);
}

#[test]
fn two_models_carry_the_independence_warning() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 10, &["A", "B"]);
    let a = model(&mut f, "Model A", "Model A", "5.5");
    let b = model(&mut f, "GPT", "GPT", "x");
    for u in 0..10 {
        code(&mut f, u, u % 2, a);
        code(&mut f, u, u % 2, b);
    }
    let r = f.p.agreement_run(&request(&[a, b])).unwrap();
    assert_eq!(r.reporting_term, "concordance");
    assert!(r.notes.iter().any(|n| n.starts_with("Limitation: 2 of the coders are models")), "{:?}", r.notes);
}

#[test]
fn high_agreement_with_one_dominant_code_is_explained_not_celebrated() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 100, &["Common", "Rare"]);
    let x = human(&mut f, "Ada");
    let y = human(&mut f, "Ben");
    for u in 0..100 {
        let (a, b) = match u {
            90..=94 => (0, 1),
            95..=99 => (1, 0),
            _ => (0, 0),
        };
        code(&mut f, u, a, x);
        code(&mut f, u, b, y);
    }
    let r = f.p.agreement_run(&request(&[x, y])).unwrap();
    let o = r.overall.as_ref().unwrap();
    assert!((o.percent_agreement.unwrap() - 0.9).abs() < 1e-12);
    assert!(o.cohen_kappa.unwrap() < 0.1, "the kappa paradox: {o:?}");
    assert!(r.notes.iter().any(|n| n.contains("Percent agreement is high") && n.contains("Gwet")), "{:?}", r.notes);
}

#[test]
fn three_coders_with_gaps_use_every_unit_two_or_more_of_them_coded() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 12, &["A", "B", "C"]);
    let x = human(&mut f, "Ada");
    let y = human(&mut f, "Ben");
    let z = human(&mut f, "Cal");
    for u in 0..12 {
        code(&mut f, u, u % 3, x);
        if u < 8 {
            code(&mut f, u, u % 3, y);
        }
        if u >= 6 {
            code(&mut f, u, if u == 11 { 0 } else { u % 3 }, z);
        }
    }
    let r = f.p.agreement_run(&request(&[x, y, z])).unwrap();
    // Units 0-5: Ada and Ben. 6-7: all three. 8-11: Ada and Cal. Every unit has at least two raters.
    assert_eq!(r.units_in_sample, 12);
    let o = r.overall.as_ref().unwrap();
    assert_eq!(o.fleiss_units, 2, "Fleiss' kappa needs units every coder rated");
    assert!(r.confusion.is_none(), "a confusion table is for exactly two coders");
    assert!(o.krippendorff_alpha.unwrap() < 1.0 && o.krippendorff_alpha.unwrap() > 0.8);
}

#[test]
fn a_coding_with_no_unit_counts_for_the_unit_it_lies_inside() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 6, &["A", "B"]);
    let x = human(&mut f, "Ada");
    let y = human(&mut f, "Ben");
    for u in 0..6 {
        code(&mut f, u, u % 2, x);
        // Ben marks only the first two characters of each line, with no unit given.
        let c = NewCoding::new(f.source, 5 * u as i64, 5 * u as i64 + 2, f.codes[u % 2], y);
        f.p.apply_coding(&c).unwrap();
    }
    let r = f.p.agreement_run(&request(&[x, y])).unwrap();
    assert_eq!(r.units_in_sample, 6);
    assert_eq!(r.overall.unwrap().percent_agreement, Some(1.0));
}

#[test]
fn the_adjudicated_layer_is_never_part_of_a_coders_agreement() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    // A third coder's "final" coding must not change what Ada and Ben are measured on.
    let judge = human(&mut f, "Judge");
    let before = f.p.agreement_run(&request(&[x, y])).unwrap().overall;
    let c = Connection::open(f.p.path()).unwrap();
    c.execute(
        "INSERT INTO coding (source_id, start_offset, end_offset, code_id, coder_id, layer, unit_id, created_at)
         VALUES (?1, 0, 4, ?2, ?3, 'final', ?4, '2026-01-01T00:00:00Z')",
        rusqlite::params![f.source, f.codes[1], judge, f.units[0]],
    )
    .unwrap();
    let after = f.p.agreement_run(&request(&[x, y])).unwrap().overall;
    assert_eq!(before, after);
}

// ---- each code on its own ----------------------------------------------------------------------

#[test]
fn per_code_agreement_counts_both_either_and_neither() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 10, &["Hope", "Always"]);
    let x = human(&mut f, "Ada");
    let y = human(&mut f, "Ben");
    for u in 0..10 {
        // Both coders code every unit with "Always", so both have reviewed every unit.
        code(&mut f, u, 1, x);
        code(&mut f, u, 1, y);
        if u < 6 {
            code(&mut f, u, 0, x); // Ada applies Hope to units 0-5
        }
        if u < 4 || u == 8 {
            code(&mut f, u, 0, y); // Ben applies Hope to 0-3 and 8
        }
    }
    let mut req = request(&[x, y]);
    req.mode = "per_code".to_string();
    let r = f.p.agreement_run(&req).unwrap();
    assert!(r.overall.is_none());
    let hope = r.by_code.iter().find(|c| c.name == "Hope").unwrap();
    assert_eq!((hope.detail.both, hope.detail.first_only, hope.detail.second_only, hope.detail.neither), (4, 2, 1, 3));
    assert!((hope.detail.agreement.percent_agreement.unwrap() - 0.7).abs() < 1e-12);
    assert!((hope.detail.overlap.unwrap() - 4.0 / 7.0).abs() < 1e-12);

    // "Always" was applied identically everywhere: perfect agreement, so it must not be listed as the worst.
    let always = r.by_code.iter().find(|c| c.name == "Always").unwrap();
    assert!(!always.below_target);
    assert_eq!(r.by_code.last().unwrap().name, "Always");
    assert_eq!(r.by_code.first().unwrap().name, "Hope");
    assert!(r.by_code[0].below_target);
    assert!(r.report_text.contains("both applied 4 / either applied 7"), "{}", r.report_text);
    assert!(r.report_text.contains("(under target)"), "{}", r.report_text);
}

#[test]
fn several_codes_on_one_unit_need_the_per_code_view() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 4, &["A", "B"]);
    let x = human(&mut f, "Ada");
    let y = human(&mut f, "Ben");
    for u in 0..4 {
        code(&mut f, u, 0, x);
        code(&mut f, u, 1, x);
        code(&mut f, u, 0, y);
    }
    let err = f.p.agreement_run(&request(&[x, y])).unwrap_err().to_string();
    assert!(err.contains("Ada applied 2 codes") && err.contains("per-code view"), "{err}");
    let mut req = request(&[x, y]);
    req.mode = "per_code".to_string();
    assert!(f.p.agreement_run(&req).is_ok());
}

// ---- refusals ----------------------------------------------------------------------------------

#[test]
fn it_refuses_what_cannot_be_measured() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 4, &["A", "B"]);
    let x = human(&mut f, "Ada");
    let y = human(&mut f, "Ben");
    let z = human(&mut f, "Cal");

    let err = |e: Error| e.to_string();
    assert!(err(f.p.agreement_run(&request(&[x])).unwrap_err()).contains("at least two coders"));
    assert!(err(f.p.agreement_run(&request(&[x, x])).unwrap_err()).contains("twice"));
    assert!(matches!(f.p.agreement_run(&request(&[x, 999])).unwrap_err(), Error::NotFound { what: "coder", .. }));

    // Only Ada coded anything, so there is no unit two people coded.
    code(&mut f, 0, 0, x);
    code(&mut f, 1, 1, x);
    assert!(err(f.p.agreement_run(&request(&[x, y])).unwrap_err()).contains("nothing to compare"));

    // Both coders used a single code: no variation to measure.
    code(&mut f, 0, 0, y);
    code(&mut f, 2, 0, x);
    code(&mut f, 2, 0, y);
    code(&mut f, 1, 0, z);
    let e = err(f.p.agreement_run(&request(&[x, y])).unwrap_err());
    assert!(e.contains("fewer than two different codes") || e.contains("nothing"), "{e}");

    let mut bad = request(&[x, y]);
    bad.mode = "wide".to_string();
    assert!(err(f.p.agreement_run(&bad).unwrap_err()).contains("single_label or per_code"));
    let mut bad = request(&[x, y]);
    bad.reporting_term = Some("agreement".to_string());
    assert!(err(f.p.agreement_run(&bad).unwrap_err()).contains("reporting term"));
}

#[test]
fn a_run_that_is_not_saved_leaves_no_trace_and_no_lock() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    let mut req = request(&[x, y]);
    req.save = false;
    let r = f.p.agreement_run(&req).unwrap();
    assert_eq!(r.run_id, None);
    assert!(f.p.list_reliability_runs().unwrap().is_empty());
    f.p.replace_source_text(f.source, "changed\n").unwrap();
}

// ---- the lock on sampled units -----------------------------------------------------------------

#[test]
fn text_of_sampled_units_is_locked_until_the_run_is_archived() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    let other = f.p.add_source(&NewSource::new("Other", SourceType::Document, "Unrelated words\n")).unwrap();

    let r = f.p.agreement_run(&request(&[x, y])).unwrap();
    let id = r.run_id.unwrap();

    let new_text: String = (0..20).map(|i| format!("v{i:03}\n")).collect();
    let err = f.p.replace_source_text(f.source, &new_text).unwrap_err().to_string();
    assert!(err.contains(&format!("reliability run {id}")) && err.contains("archive the run"), "{err}");
    f.p.replace_source_text(other, "Different words\n").unwrap();
    assert_eq!(f.p.locked_sources().unwrap(), vec![(f.source, id)]);

    f.p.archive_reliability_run(id).unwrap();
    assert!(f.p.locked_sources().unwrap().is_empty());
    f.p.replace_source_text(f.source, &new_text).unwrap();
    assert!(f.p.archive_reliability_run(id).is_err(), "a run can be archived once");
    assert!(f.p.archive_reliability_run(999).is_err());

    // The run stays, with its report, as the record of what was measured.
    let runs = f.p.list_reliability_runs().unwrap();
    assert_eq!(runs.len(), 1);
    assert!(runs[0].archived, "marked archived");
    let kept = f.p.get_reliability_run(id).unwrap();
    assert!(kept.text.contains("Ada (human)"));
}

#[test]
fn a_run_adds_audit_events_that_do_not_block_undo() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    f.p.agreement_run(&request(&[x, y])).unwrap();
    let events = f.p.events(5).unwrap();
    assert_eq!(events.last().unwrap().action, "reliability.run");
    let undone = f.p.undo().unwrap().expect("the newest coding can still be undone after a run");
    assert_eq!(undone.action, "coding.add");
}

#[test]
fn runs_are_listed_newest_first() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    let a = f.p.agreement_run(&request(&[x, y])).unwrap().run_id.unwrap();
    let b = f.p.agreement_run(&request(&[x, y])).unwrap().run_id.unwrap();
    let ids: Vec<i64> = f.p.list_reliability_runs().unwrap().iter().map(|r| r.id).collect();
    assert_eq!(ids, [b, a]);
}

// ---- samples -----------------------------------------------------------------------------------

#[test]
fn the_same_seed_draws_the_same_sample_and_another_seed_a_different_one() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir, 200, &["A"]);
    let a = f.p.draw_sample(30, 42, &Stratify::None, None).unwrap();
    let b = f.p.draw_sample(30, 42, &Stratify::None, None).unwrap();
    let c = f.p.draw_sample(30, 43, &Stratify::None, None).unwrap();
    assert_eq!(a, b);
    assert_ne!(a.unit_ids, c.unit_ids);
    assert_eq!((a.size, a.pool, a.unit_ids.len()), (30, 200, 30));
    assert_eq!(a.unit_ids.iter().collect::<HashSet<_>>().len(), 30, "no unit is drawn twice");
    assert!(a.unit_ids.windows(2).all(|w| w[0] < w[1]), "returned in order");
}

#[test]
fn stratifying_by_source_keeps_each_sources_share() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir, 60, &["A"]);
    let second = f.p.add_source(&NewSource::new("Second", SourceType::Document, (0..40).map(|i| format!("w{i:03}\n")).collect::<String>())).unwrap();
    for i in 0..40 {
        f.p.add_unit(second, UnitKind::Paragraph, 5 * i, 5 * i + 4, None).unwrap();
    }
    let s = f.p.draw_sample(10, 5, &Stratify::Source, None).unwrap();
    let in_first = s.unit_ids.iter().filter(|u| f.units.contains(u)).count();
    assert_eq!((in_first, s.unit_ids.len() - in_first), (6, 4), "60 and 40 units give 6 and 4 of 10");
    assert_eq!(s, f.p.draw_sample(10, 5, &Stratify::Source, None).unwrap());

    // A size that does not divide evenly still adds up exactly.
    for size in [1, 3, 7, 11, 33, 99] {
        assert_eq!(f.p.draw_sample(size, 9, &Stratify::Source, None).unwrap().unit_ids.len(), size);
    }
}

#[test]
fn stratifying_by_a_unit_attribute_keeps_each_groups_share() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir, 50, &["A"]);
    let c = Connection::open(f.p.path()).unwrap();
    c.execute("INSERT INTO attribute (name, applies_to, level) VALUES ('speaker', 'unit', 'nominal')", []).unwrap();
    let attr = c.last_insert_rowid();
    for (i, u) in f.units.iter().enumerate() {
        let who = if i < 10 { "teacher" } else { "student" };
        c.execute("INSERT INTO attribute_value (attribute_id, target_id, value) VALUES (?1, ?2, ?3)", rusqlite::params![attr, u, who]).unwrap();
    }
    let s = f.p.draw_sample(10, 3, &Stratify::Attribute("speaker".to_string()), None).unwrap();
    let teachers = s.unit_ids.iter().filter(|u| f.units[..10].contains(u)).count();
    assert_eq!(teachers, 2, "10 of 50 units are teacher turns, so 2 of 10");
    assert_eq!(s.unit_ids.len(), 10);
}

#[test]
fn units_left_out_of_coding_are_never_drawn_and_the_pool_can_be_narrowed() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir, 20, &["A"]);
    let c = Connection::open(f.p.path()).unwrap();
    c.execute("UPDATE unit SET excluded = 1 WHERE id IN (SELECT id FROM unit ORDER BY id LIMIT 15)", []).unwrap();
    let s = f.p.draw_sample(5, 1, &Stratify::None, None).unwrap();
    assert_eq!(s.pool, 5);
    assert!(s.unit_ids.iter().all(|u| f.units[15..].contains(u)));
    let err = f.p.draw_sample(6, 1, &Stratify::None, None).unwrap_err().to_string();
    assert!(err.contains("only 5 are available"), "{err}");
    assert!(f.p.draw_sample(0, 1, &Stratify::None, None).is_err());
    assert!(f.p.draw_sample(1, 1, &Stratify::None, Some(&[12345])).is_err(), "no units in a source that is not there");
}

#[test]
fn a_drawn_sample_can_be_the_units_a_run_measures() {
    let dir = TempDir::new().unwrap();
    let (mut f, x, y) = twenty_with_four_disagreements(&dir);
    let sample = f.p.draw_sample(10, 99, &Stratify::None, None).unwrap();
    let mut req = request(&[x, y]);
    req.unit_ids = Some(sample.unit_ids.clone());
    req.seed = sample.seed;
    let r = f.p.agreement_run(&req).unwrap();
    assert_eq!(r.units_in_sample, 10);
    let saved = f.p.get_reliability_run(r.run_id.unwrap()).unwrap();
    assert_eq!(saved.report["units_in_sample"], 10);
    let c = Connection::open(f.p.path()).unwrap();
    let (units_json, seed): (String, i64) = c.query_row("SELECT units_json, sample_seed FROM reliability_run", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(serde_json::from_str::<Vec<i64>>(&units_json).unwrap(), sample.unit_ids);
    assert_eq!(seed, 99);
}
