//! Adjudication: the queue of units coders coded differently, deciding each with a reason, the decisions kept apart from the codings, and
//! undo.

use prismqda_engine::{ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, SourceType, UnitKind};
use tempfile::TempDir;

const PARAGRAPHS: [&str; 4] = ["Fares are too high for me.", "The stop is far from my house.", "I love the new buses.", "The driver was rude to me."];

struct W {
    p: Project,
    _dir: TempDir,
    cost: i64,
    access: i64,
    ada: i64,
    ben: i64,
    cy: i64,
    units: Vec<i64>,
}

/// Three coders and four units. Ada and Ben agree on every unit but the third; Cy disagrees with them on the first and codes nothing else.
fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("a.pqda"), &NewProject::new("A"), &o).unwrap();
    let mut text = String::new();
    let mut units = Vec::new();
    for para in PARAGRAPHS {
        let start = text.chars().count() as i64;
        text.push_str(para);
        units.push(ImportedUnit::new(start, text.chars().count() as i64, UnitKind::Paragraph));
        text.push_str("\n\n");
    }
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Riders".into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text,
            runs: vec![],
            units,
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let source = p.apply_import(&batch).unwrap().source_ids[0];
    let cost = p.add_code(&NewCode::new("Cost")).unwrap();
    let access = p.add_code(&NewCode::new("Access")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let ben = p.add_coder(&NewCoder::human("Ben")).unwrap();
    let cy = p.add_coder(&NewCoder::human("Cy")).unwrap();
    let rows = p.list_units(source).unwrap();
    let mut units = Vec::new();
    for (i, u) in rows.iter().enumerate() {
        let (a, b) = [(cost, cost), (access, access), (access, cost), (cost, cost)][i];
        for (code, coder) in [(a, ada), (b, ben)] {
            let mut c = NewCoding::new(source, u.start, u.end, code, coder);
            c.unit_id = Some(u.id);
            p.apply_coding(&c).unwrap();
        }
        units.push(u.id);
    }
    // Cy codes the first unit as Access, where Ada and Ben said Cost.
    let mut c = NewCoding::new(source, rows[0].start, rows[0].end, access, cy);
    c.unit_id = Some(rows[0].id);
    p.apply_coding(&c).unwrap();
    W { p, _dir: dir, cost, access, ada, ben, cy, units }
}

#[test]
fn the_queue_lists_the_units_coders_coded_differently_with_what_each_gave() {
    let w = world();
    let queue = w.p.disagreements(&[w.ada, w.ben], false).unwrap();
    assert_eq!(queue.len(), 1, "Ada and Ben differ only on the third unit");
    let d = &queue[0];
    assert_eq!((d.unit_id, d.source_name.as_str(), d.text.as_str()), (w.units[2], "Riders", "I love the new buses."));
    assert!(d.decision.is_none());
    let who: Vec<(&str, Vec<&str>)> = d.by_coder.iter().map(|c| (c.label.as_str(), c.codes.iter().map(|k| k.name.as_str()).collect())).collect();
    assert_eq!(who, [("Ada", vec!["Access"]), ("Ben", vec!["Cost"])]);
    assert!(d.by_coder.iter().all(|c| c.coder_id.is_some() && c.role.as_deref() == Some("human")));

    // A third coder who differs on another unit brings that unit in; a unit only one coder coded is never in the queue.
    let three = w.p.disagreements(&[w.ada, w.ben, w.cy], false).unwrap();
    assert_eq!(three.iter().map(|d| d.unit_id).collect::<Vec<_>>(), [w.units[0], w.units[2]], "in reading order");
    assert_eq!(w.p.disagreements(&[w.ada, w.cy], false).unwrap().iter().map(|d| d.unit_id).collect::<Vec<_>>(), [w.units[0]], "Cy coded only the first unit");

    assert!(w.p.disagreements(&[w.ada], false).is_err(), "two coders are needed");
    assert!(w.p.disagreements(&[w.ada, w.ada], false).is_err(), "each once");
}

#[test]
fn a_blind_queue_hides_who_coded_what() {
    let w = world();
    let queue = w.p.disagreements(&[w.ada, w.ben, w.cy], true).unwrap();
    for d in &queue {
        let labels: Vec<&str> = d.by_coder.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.iter().enumerate().all(|(i, l)| *l == format!("Coder {}", (b'A' + i as u8) as char)), "{labels:?}");
        assert!(d.by_coder.iter().all(|c| c.coder_id.is_none() && c.role.is_none()), "no name, id, or role is given");
    }
    let json = serde_json::to_string(&queue).unwrap();
    assert!(!json.contains("Ada") && !json.contains("Ben") && !json.contains("Cy"), "no coder's name is anywhere in what is sent: {json}");
}

#[test]
fn a_decision_needs_a_reason_names_real_active_codes_and_leaves_the_codings_alone() {
    let mut w = world();
    let unit = w.units[2];
    let codings_before = w.p.coded_segments(&Default::default()).unwrap().len();
    assert!(w.p.decide(unit, &[w.cost], "   ", w.ada).unwrap_err().to_string().contains("reason"));
    assert!(w.p.decide(unit, &[9999], "Because.", w.ada).is_err(), "a code that is not there");
    assert!(w.p.decide(9999, &[w.cost], "Because.", w.ada).is_err(), "a unit that is not there");
    assert!(w.p.decide(unit, &[w.cost], "Because.", 9999).is_err(), "a decider that is not there");
    assert!(w.p.decisions().unwrap().is_empty(), "refusals leave nothing behind");

    let d = w.p.decide(unit, &[w.cost, w.cost], "  The words are about money.  ", w.cy).unwrap();
    assert_eq!((d.unit_id, d.adjudicator_name.as_str(), d.reason.as_str()), (unit, "Cy", "The words are about money."));
    assert_eq!(d.codes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Cost"], "a code named twice counts once");
    // The decision shows in the queue, and nothing any coder coded changed.
    let queue = w.p.disagreements(&[w.ada, w.ben], false).unwrap();
    assert_eq!(queue[0].decision.as_ref().map(|x| x.reason.as_str()), Some("The words are about money."));
    assert_eq!(w.p.coded_segments(&Default::default()).unwrap().len(), codings_before, "the coders' codings are the same");
    assert_eq!(w.p.decisions().unwrap().len(), 1);

    // Deciding again replaces the decision; no code at all says that none applies.
    let again = w.p.decide(unit, &[], "On a second look, neither fits.", w.ada).unwrap();
    assert!(again.codes.is_empty() && again.adjudicator_name == "Ada");
    let all = w.p.decisions().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].decision.reason.as_str(), all[0].source_name.as_str(), all[0].text.as_str()), ("On a second look, neither fits.", "Riders", "I love the new buses."));

    // A retired code cannot be used.
    w.p.retire_code(w.access).unwrap();
    assert!(w.p.decide(unit, &[w.access], "Because.", w.ada).unwrap_err().to_string().contains("retired"));
}

#[test]
fn undo_takes_a_decision_back_and_puts_the_earlier_one_back() {
    let mut w = world();
    let unit = w.units[2];
    w.p.decide(unit, &[w.cost], "First.", w.ada).unwrap();
    w.p.decide(unit, &[w.access], "Second.", w.ben).unwrap();
    let out = w.p.undo().unwrap().unwrap();
    assert_eq!(out.action, "adjudication.set");
    assert_eq!(out.what, "Took back the decision on the unit");
    let back = w.p.decision_for(unit).unwrap().unwrap();
    assert_eq!((back.reason.as_str(), back.adjudicator_name.as_str(), back.codes[0].name.as_str()), ("First.", "Ada", "Cost"), "the earlier decision is back, as it was");
    w.p.undo().unwrap().unwrap();
    assert!(w.p.decision_for(unit).unwrap().is_none(), "and the first one can be taken back too");
}

#[test]
fn the_final_layer_counts_words_coded_without_a_unit_in_the_unit_they_lie_in() {
    use prismqda_engine::SegmentFilter;
    // As after a merge in a project that codes any words: the owner's codings name no unit, and the merged coder's do.
    let mut w = world();
    let (source, rows) = {
        let source = w.p.list_sources().unwrap()[0].id;
        (source, w.p.list_units(source).unwrap())
    };
    let dan = w.p.add_coder(&NewCoder::human("Dan")).unwrap();
    // Dan's words are part of the second unit (which Ada and Ben agree is Access) and of the fourth (Cost), coded without a unit.
    w.p.apply_coding(&NewCoding::new(source, rows[1].start, rows[1].end - 1, w.access, dan)).unwrap();
    w.p.apply_coding(&NewCoding::new(source, rows[3].start, rows[3].end - 1, w.access, dan)).unwrap();
    // Words that reach over two units are in neither.
    w.p.apply_coding(&NewCoding::new(source, rows[0].start, rows[1].end, w.cost, dan)).unwrap();
    let s = w.p.final_summary().unwrap();
    assert_eq!((s.decided, s.agreed, s.open, s.without_unit), (0, 1, 3, 1), "Dan agrees on the second unit and differs on the fourth");
    let seg = w.p.coded_segments(&SegmentFilter { layer: Some("final".into()), ..Default::default() }).unwrap();
    assert_eq!(seg.len(), 1, "one coding for the unit they agree on, not one for each coder");
    assert_eq!((seg[0].code_name.as_str(), seg[0].coder_name.as_str()), ("Access", "Ada"));
    // It says what the disagreements say: the fourth unit is in the queue for Ada and Dan, and the second is not.
    let queue: Vec<i64> = w.p.disagreements(&[w.ada, dan], false).unwrap().iter().map(|d| d.unit_id).collect();
    assert_eq!(queue, [rows[3].id]);
}

#[test]
fn the_final_layer_is_the_decisions_and_what_the_coders_agreed_on_and_leaves_open_units_out() {
    use prismqda_engine::{ExploreScope as Scope, SegmentFilter};
    let mut w = world();
    let final_only = || SegmentFilter { layer: Some("final".into()), ..Default::default() };
    let texts = |segments: &[prismqda_engine::CodedSegment]| -> Vec<(String, String)> { segments.iter().map(|s| (s.text.clone(), s.code_name.clone())).collect() };

    // Nothing is decided: the two units the coders agree on are final, and the two they differ on are open.
    let s = w.p.final_summary().unwrap();
    assert_eq!((s.decided, s.agreed, s.open, s.without_unit), (0, 2, 2, 0));
    let seg = w.p.coded_segments(&final_only()).unwrap();
    assert_eq!(texts(&seg), [("The stop is far from my house.".to_string(), "Access".to_string()), ("The driver was rude to me.".to_string(), "Cost".to_string())]);
    assert!(seg.iter().all(|s| s.layer == "final"));

    // Deciding settles a unit: its codes are the decision's, and none applies when it names none.
    w.p.decide(w.units[0], &[w.cost], "It is about money.", w.cy).unwrap();
    w.p.decide(w.units[2], &[], "Neither fits.", w.ada).unwrap();
    let s = w.p.final_summary().unwrap();
    assert_eq!((s.decided, s.agreed, s.open), (2, 2, 0));
    let seg = w.p.coded_segments(&final_only()).unwrap();
    assert_eq!(
        texts(&seg),
        [
            ("Fares are too high for me.".to_string(), "Cost".to_string()),
            ("The stop is far from my house.".to_string(), "Access".to_string()),
            ("The driver was rude to me.".to_string(), "Cost".to_string()),
        ],
        "the unit decided as none applies has no final code"
    );
    let decided = &seg[0];
    assert_eq!((decided.coder_name.as_str(), decided.rationale.as_str()), ("Cy", "It is about money."), "a decision's coding is the decider's, with the reason");
    assert!(decided.coding_id < 0, "and has a number of its own, which no coding and no area has");

    // The coders' own codings are the same as before, and the default list never holds the final layer.
    let all = w.p.coded_segments(&Default::default()).unwrap();
    assert_eq!(all.len(), 9, "Ada and Ben coded four units each, and Cy one");
    assert!(all.iter().all(|s| s.layer == "coder"));

    // Explore counts the final layer when it is asked to, and the numbers it gives for the evidence find their passages.
    let scope = Scope { layer: Some("final".into()), ..Default::default() };
    let profile = w.p.code_profile(w.cost, false, &scope).unwrap();
    assert_eq!(profile.coding_ids.len(), 2, "the decided unit and the agreed one");
    assert_eq!(w.p.code_profile(w.cost, false, &Scope::default()).unwrap().coding_ids.len(), 5, "the coders' own: five");
    let evidence = w.p.coded_segments(&SegmentFilter { coding_ids: Some(profile.coding_ids.clone()), ..Default::default() }).unwrap();
    assert_eq!(evidence.len(), 2, "both numbers are found, the decision's among them");
    assert!(evidence.iter().any(|s| s.rationale == "It is about money."));

    // Deciding again changes the layer at once.
    w.p.decide(w.units[0], &[w.access], "On reflection, access.", w.cy).unwrap();
    let seg = w.p.coded_segments(&final_only()).unwrap();
    assert_eq!(seg[0].code_name, "Access");
}
