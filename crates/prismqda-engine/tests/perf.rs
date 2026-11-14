//! Performance against the targets in specification section 28.1, on a project built at the largest
//! size the specification supports: 2,000 sources, 25 million words, 100,000 units, 500,000 codings,
//! and 2,000 codes. It is slow to build, so it only runs when asked:
//!
//!     cargo test --release -p prismqda-engine --test perf -- --ignored --nocapture
//!
//! Timings are printed. They are only checked against the targets when `PRISMQDA_PERF_STRICT` is set,
//! because the targets are for a reference laptop and a different computer proves nothing either way.
//! `PRISMQDA_PERF_SCALE` (default 1.0) shrinks the project for a quick run.

use std::time::Instant;

use prismqda_engine::{
    ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, NewProposal, OpenOptions, Project, ProposalOrigin,
    SearchFilter, SourceType, UnitKind,
};
use tempfile::TempDir;

/// A small deterministic random number generator, so every run builds the same project.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn scale() -> f64 {
    std::env::var("PRISMQDA_PERF_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0)
}

fn check(name: &str, seconds: f64, target: f64) {
    let verdict = if seconds <= target { "within target" } else { "OVER target" };
    eprintln!("  {name:<44} {seconds:>8.3} s   (target {target} s)   {verdict}");
    if std::env::var_os("PRISMQDA_PERF_STRICT").is_some() {
        assert!(seconds <= target, "{name}: {seconds:.3} s is over the {target} s target");
    }
}

#[test]
#[ignore = "slow: builds a project at the largest supported size"]
fn a_project_at_the_largest_supported_size_meets_the_targets() {
    let s = scale();
    let sources = (2_000.0 * s).max(2.0) as usize;
    let words_per_source = 12_500usize;
    let units_per_source = (50.0f64).max(1.0) as usize;
    let codes = (2_000.0 * s).max(5.0) as usize;
    let codings_per_source = 250usize;

    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("perf");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("big.pqda"), &NewProject::new("Largest supported"), &o).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();

    eprintln!("Building {sources} sources of {words_per_source} words ({} million words)...", sources * words_per_source / 1_000_000);
    let t = Instant::now();
    let mut rng = Lcg(42);
    let vocab: Vec<String> = (0..20_000).map(|i| format!("word{i}")).collect();
    let mut batch = ImportBatch::default();
    let mut total_words = 0usize;
    for i in 0..sources {
        let mut text = String::with_capacity(words_per_source * 8);
        let mut units = Vec::new();
        let mut chars = 0i64;
        let per_unit = words_per_source / units_per_source;
        for u in 0..units_per_source {
            let start = chars;
            for w in 0..per_unit {
                let r = rng.next();
                // About one word in 400 is "cost" and one in 600 is "access", so searches have real work to do.
                let word: &str = match r % 1200 {
                    0..=2 => "cost",
                    3..=4 => "access",
                    _ => &vocab[(r as usize / 7) % vocab.len()],
                };
                if w > 0 {
                    text.push(' ');
                    chars += 1;
                }
                text.push_str(word);
                chars += word.chars().count() as i64;
                total_words += 1;
            }
            units.push(ImportedUnit::new(start, chars, UnitKind::Comment));
            if u + 1 < units_per_source {
                text.push_str("\n\n");
                chars += 2;
            }
        }
        batch.sources.push(ImportedSource {
            name: format!("Source {i}"),
            source_type: SourceType::Social,
            folder: String::new(),
            text,
            runs: vec![],
            units,
            default_unit_kind: UnitKind::Comment,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        });
        // Store in chunks so the batch in memory stays small.
        if batch.sources.len() == 100 || i + 1 == sources {
            p.apply_import(&batch).unwrap();
            batch = ImportBatch::default();
        }
    }
    eprintln!("  built the text in {:.1} s ({total_words} words, {} units)", t.elapsed().as_secs_f64(), sources * units_per_source);

    let t = Instant::now();
    let code_ids: Vec<i64> = (0..codes).map(|i| p.add_code(&NewCode::new(format!("Code {i}"))).unwrap()).collect();
    eprintln!("  made {codes} codes in {:.1} s", t.elapsed().as_secs_f64());

    let t = Instant::now();
    let source_rows = p.list_sources().unwrap();
    let mut made = 0usize;
    let mut pending: Vec<NewCoding> = Vec::new();
    for src in &source_rows {
        let units = p.list_units(src.id).unwrap();
        for k in 0..codings_per_source {
            let u = &units[k % units.len()];
            let code = code_ids[(rng.next() as usize) % code_ids.len()];
            let mut c = NewCoding::new(src.id, u.start, u.end, code, coder);
            c.unit_id = Some(u.id);
            pending.push(c);
        }
        if pending.len() >= 20_000 {
            made += pending.len();
            p.apply_codings(&pending).unwrap();
            pending.clear();
        }
    }
    made += pending.len();
    p.apply_codings(&pending).unwrap();
    eprintln!("  stored {made} codings in {:.1} s", t.elapsed().as_secs_f64());

    let file_mb = std::fs::metadata(p.path()).unwrap().len() as f64 / 1_048_576.0;
    eprintln!("  the project file is {file_mb:.0} MB");
    let path = p.path().to_path_buf();
    drop(p);

    eprintln!("Measuring (specification section 28.1):");
    let t = Instant::now();
    let mut o2 = OpenOptions::new("perf");
    o2.backup_on_open = false;
    let mut p = Project::open(&path, &o2).unwrap();
    let info = p.info().unwrap();
    check("open the project and read its counts", t.elapsed().as_secs_f64(), 3.0);
    assert_eq!(info.sources as usize, sources);

    let src = source_rows[source_rows.len() / 2].id;
    let units = p.list_units(src).unwrap();
    let t = Instant::now();
    let id = p.apply_coding(&NewCoding { unit_id: Some(units[1].id), ..NewCoding::new(src, units[1].start, units[1].end, code_ids[0], coder) }).unwrap();
    check("apply one coding", t.elapsed().as_secs_f64(), 0.1);
    let t = Instant::now();
    p.remove_coding(id).unwrap();
    check("remove one coding", t.elapsed().as_secs_f64(), 0.1);

    let t = Instant::now();
    let r = p.search("cost", &SearchFilter { limit: Some(100), ..Default::default() }).unwrap();
    check(&format!("search one word ({} scopes)", r.total), t.elapsed().as_secs_f64(), 2.0);
    let t = Instant::now();
    let r = p.search("cost AND access", &SearchFilter { limit: Some(100), ..Default::default() }).unwrap();
    check(&format!("search two words with AND ({} scopes)", r.total), t.elapsed().as_secs_f64(), 2.0);
    let t = Instant::now();
    let r = p.search("\"cost word17\" OR /access\\s+word\\d+3\\b/", &SearchFilter { limit: Some(100), ..Default::default() }).unwrap();
    check(&format!("search a phrase or a pattern ({} scopes)", r.total), t.elapsed().as_secs_f64(), 2.0);
    let t = Instant::now();
    let r = p.search("cost NEAR/3 access", &SearchFilter { limit: Some(100), ..Default::default() }).unwrap();
    check(&format!("search with NEAR ({} scopes)", r.total), t.elapsed().as_secs_f64(), 2.0);

    let t = Instant::now();
    let rows = p.list_codes().unwrap();
    check(&format!("list {} codes with usage counts", rows.len()), t.elapsed().as_secs_f64(), 1.0);
    let t = Instant::now();
    let c = p.list_codings(src).unwrap();
    check(&format!("list one source's {} codings", c.len()), t.elapsed().as_secs_f64(), 0.5);
}

/// Roadmap M3 gate: 10,000 units, 3 coders, 50 codes, agreement with bootstrap intervals in under 5 seconds.
#[test]
#[ignore = "slow: builds 30,000 codings and runs 3,000 bootstrap resamples"]
fn agreement_on_ten_thousand_units_with_three_coders_and_fifty_codes() {
    use prismqda_engine::AgreementRequest;

    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("perf");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("agree.pqda"), &NewProject::new("Agreement at size"), &o).unwrap();
    let coders: Vec<i64> = ["Ada", "Ben", "Cal"].iter().map(|n| p.add_coder(&NewCoder::human(*n)).unwrap()).collect();
    let codes: Vec<i64> = (0..50).map(|i| p.add_code(&NewCode::new(format!("Code {i}"))).unwrap()).collect();

    let n = 10_000usize;
    let text: String = (0..n).map(|i| format!("unit{i:05}\n")).collect();
    let mut batch = ImportBatch::default();
    batch.sources.push(ImportedSource {
        name: "Comments".to_string(),
        source_type: SourceType::Social,
        folder: String::new(),
        text,
        runs: vec![],
        units: (0..n as i64).map(|i| ImportedUnit::new(10 * i, 10 * i + 9, UnitKind::Comment)).collect(),
        default_unit_kind: UnitKind::Comment,
        unit_attributes: vec![],
        original_file_hash: None,
        settings: serde_json::Value::Null,
        warnings: vec![],
    });
    p.apply_import(&batch).unwrap();
    let source = p.list_sources().unwrap()[0].id;
    let units = p.list_units(source).unwrap();

    let mut rng = Lcg(7);
    let mut pending = Vec::new();
    for u in &units {
        let truth = (rng.next() as usize) % codes.len();
        for (k, coder) in coders.iter().enumerate() {
            // Each coder agrees with the "true" code about 85 percent of the time.
            let pick = if k == 0 || rng.next() % 100 < 85 { truth } else { (rng.next() as usize) % codes.len() };
            let mut c = NewCoding::new(source, u.start, u.end, codes[pick], *coder);
            c.unit_id = Some(u.id);
            pending.push(c);
        }
    }
    p.apply_codings(&pending).unwrap();

    let mut req: AgreementRequest = serde_json::from_value(serde_json::json!({ "coder_ids": coders })).unwrap();
    req.save = false;
    let t = Instant::now();
    let r = p.agreement_run(&req).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let o = r.overall.as_ref().unwrap();
    eprintln!("  units {}, alpha {:.3}, Fleiss {:.3}", r.units_in_sample, o.krippendorff_alpha.unwrap(), o.fleiss_kappa.unwrap());
    check("agreement, 10,000 units x 3 coders x 50 codes", secs, 5.0);

    req.bootstrap_replications = 0;
    let t = Instant::now();
    p.agreement_run(&req).unwrap();
    eprintln!("  (without intervals: {:.3} s)", t.elapsed().as_secs_f64());
}

#[test]
#[ignore = "slow: suggests, lists, accepts, and takes back twenty thousand suggestions"]
fn a_run_of_twenty_thousand_suggestions_can_be_made_listed_decided_and_taken_back() {
    let n = ((20_000.0 * scale()) as usize).max(100);
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("perf");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("run.pqda"), &NewProject::new("Run"), &o).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Cost")).unwrap();

    // One long document of n sentences, each a unit.
    let mut text = String::new();
    let mut units = Vec::with_capacity(n);
    for i in 0..n {
        let start = text.chars().count() as i64;
        text.push_str(&format!("Sentence {i} says the cost was high."));
        units.push(ImportedUnit::new(start, text.chars().count() as i64, UnitKind::Sentence));
        text.push(char::from(10));
    }
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Long".into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text,
            runs: vec![],
            units,
            default_unit_kind: UnitKind::Sentence,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let source = p.apply_import(&batch).unwrap().source_ids[0];

    eprintln!("A run of {n} suggestions:");
    let t = Instant::now();
    let hits = p.search("cost", &SearchFilter { limit: Some(n), ..Default::default() }).unwrap();
    check("search for a word found in every sentence", t.elapsed().as_secs_f64(), 3.0);
    assert_eq!(hits.hits.len(), n);

    let items: Vec<NewProposal> = hits
        .hits
        .iter()
        .flat_map(|h| h.spans.iter().map(move |(s, e)| NewProposal { source_id: h.source_id, start: *s, end: *e, code_id: code, coder_id: coder, unit_id: h.unit_id, rationale: "perf".into() }))
        .collect();
    let t = Instant::now();
    let kept = p.drop_known(items).unwrap().kept;
    check("leave out what is already coded or waiting", t.elapsed().as_secs_f64(), 2.0);
    let t = Instant::now();
    let made = p.create_proposals_by(ProposalOrigin::Autocode, "Words: cost", "words", &kept).unwrap();
    check("send them to the review queue", t.elapsed().as_secs_f64(), 5.0);

    let t = Instant::now();
    let page = p.list_proposals_page(Some("pending"), Some(&made.batch), 50, 10_000.min(n - 50)).unwrap();
    check("list one page of fifty, far down the queue", t.elapsed().as_secs_f64(), 1.0);
    assert_eq!(page.len(), 50);
    assert!(page[0].text == "cost" && page[0].before.starts_with("Sentence"));
    let t = Instant::now();
    assert_eq!(p.proposal_batches().unwrap()[0].pending, n);
    check("count the runs", t.elapsed().as_secs_f64(), 0.5);

    let t = Instant::now();
    let out = p.decide_batch(&made.batch, true).unwrap();
    check("accept the whole run", t.elapsed().as_secs_f64(), 40.0);
    assert_eq!((out.accepted, out.failed.len()), (n, 0));
    assert_eq!(p.list_codings(source).unwrap().len(), n);

    let t = Instant::now();
    let undone = p.undo().unwrap().unwrap();
    check("take the whole decision back with one undo", t.elapsed().as_secs_f64(), 20.0);
    assert!(undone.what.starts_with("Took back"));
    assert!(p.list_codings(source).unwrap().is_empty());
}
