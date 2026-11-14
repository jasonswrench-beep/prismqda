//! Rules that suggest codes (speakers, headings, widening a phrase), what the review queue shows about a suggestion,
//! correcting a suggestion before it is accepted, and taking a whole decision back with one undo.

use prismqda_engine::{
    speaker_key, AppliesTo, AttributeSpec, FormatRun, FoundSpan, ImportBatch, ImportedSource, ImportedUnit, Level, NewCode, NewCoder, NewCoding, NewProject, NewProposal,
    NewSource, OpenOptions, Project, ProposalOrigin, RunKind, SearchFilter, SourceType, UnitKind,
};
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

/// The character position where `needle` starts (the test texts are plain ASCII).
fn at(text: &str, needle: &str) -> i64 {
    text.find(needle).unwrap_or_else(|| panic!("{needle:?} is not in the text")) as i64
}

/// A transcript with two interviewer spellings, two Priya spellings, and Sam.
fn transcript(p: &mut Project) -> i64 {
    let text = "Interviewer: First question?\nPriya: The bus is late.\nINTERVIEWER: Next?\npriya: It costs a lot.\nSam: I walk.";
    let turn = |label: &str, words: &str, excluded: bool| {
        let start = at(text, words);
        ImportedUnit { speaker: Some(label.to_string()), excluded, ..ImportedUnit::new(start, start + words.len() as i64, UnitKind::SpeakerTurn) }
    };
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Interview".into(),
            source_type: SourceType::Transcript,
            folder: String::new(),
            text: text.into(),
            runs: vec![],
            units: vec![
                turn("Interviewer", "First question?", true),
                turn("Priya", "The bus is late.", false),
                turn("INTERVIEWER", "Next?", true),
                turn("priya", "It costs a lot.", false),
                turn("Sam", "I walk.", false),
            ],
            default_unit_kind: UnitKind::SpeakerTurn,
            unit_attributes: vec![AttributeSpec::new("speaker", AppliesTo::Unit, Level::Nominal)],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap().source_ids[0]
}

const REPORT: &str = "Intro\nSome intro words.\n\nMethods\nWe asked people.\n\nResults\nIt rained.\n\nMore results\nStill rain.\n";

/// A report with three level-1 headings and a level-2 heading inside the last one.
fn report(p: &mut Project) -> i64 {
    let heading = |name: &str, level: u8| {
        let start = at(REPORT, name);
        FormatRun { start, end: start + name.len() as i64, kind: RunKind::Heading, level: Some(level), page: None }
    };
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Report".into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text: REPORT.into(),
            runs: vec![heading("Intro", 1), heading("Methods", 1), heading("Results", 1), heading("More results", 2)],
            units: vec![],
            default_unit_kind: UnitKind::Sentence,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap().source_ids[0]
}

fn spans_of(p: &Project, source: i64, spans: &[(i64, i64)]) -> Vec<String> {
    spans.iter().map(|(s, e)| p.text_of_span(source, *s, *e).unwrap()).collect()
}

// ---- speakers ----

#[test]
fn speakers_are_gathered_by_key_and_interviewers_are_left_out() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    transcript(&mut p);

    let groups = p.speaker_groups(&SearchFilter::default()).unwrap();
    let summary: Vec<(String, usize, usize)> = groups.iter().map(|g| (g.key.clone(), g.turns, g.left_out)).collect();
    // Priya and priya are one speaker with two turns; Interviewer and INTERVIEWER are one with two turns left out of coding.
    assert_eq!(summary, [("interviewer".into(), 0, 2), ("priya".into(), 2, 0), ("sam".into(), 1, 0)]);
    let priya = groups.iter().find(|g| g.key == "priya").unwrap();
    assert_eq!(priya.variants, ["Priya", "priya"], "every spelling is listed, most used first, then alphabetical");
    assert_eq!((priya.label.as_str(), priya.sources), ("Priya", 1));
    assert_eq!(speaker_key("Priya:"), "priya");
}

#[test]
fn speaker_turns_come_back_in_order_and_skip_the_ones_left_out() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = transcript(&mut p);

    let turns = p.speaker_turns(&SearchFilter::default(), &["priya".to_string(), "interviewer".to_string()]).unwrap();
    assert_eq!(spans_of(&p, src, &turns.iter().map(|t| (t.start, t.end)).collect::<Vec<_>>()), ["The bus is late.", "It costs a lot."]);
    assert!(turns.iter().all(|t| t.key == "priya" && t.source_id == src));
    assert!(p.speaker_turns(&SearchFilter::default(), &[]).unwrap().is_empty());

    // The same filter that narrows a search narrows the speakers.
    let other = p.add_source(&NewSource::new("Other", SourceType::Document, "Nothing here.")).unwrap();
    let only_other = SearchFilter { source_ids: Some(vec![other]), ..Default::default() };
    assert!(p.speaker_groups(&only_other).unwrap().is_empty());
}

// ---- headings ----

#[test]
fn sections_run_to_the_next_heading_of_the_same_or_a_higher_level() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = report(&mut p);
    let all = SearchFilter::default();

    let levels = p.heading_levels(&all).unwrap();
    assert_eq!(levels.iter().map(|l| (l.level, l.count, l.sources)).collect::<Vec<_>>(), [(1, 3, 1), (2, 1, 1)]);

    let sections = p.heading_sections(&all, &[1], None, true).unwrap();
    let texts = spans_of(&p, src, &sections.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>());
    assert_eq!(
        texts,
        ["Intro\nSome intro words.", "Methods\nWe asked people.", "Results\nIt rained.\n\nMore results\nStill rain."],
        "a level-1 section holds the level-2 heading inside it"
    );
    assert_eq!(sections.iter().map(|s| s.heading.as_str()).collect::<Vec<_>>(), ["Intro", "Methods", "Results"]);

    // Without the heading, the section is its words only.
    let bodies = p.heading_sections(&all, &[1], Some("methods"), false).unwrap();
    assert_eq!(spans_of(&p, src, &[(bodies[0].start, bodies[0].end)]), ["We asked people."]);

    // A level-2 section ends at the end of the document (nothing follows that is as high).
    let deep = p.heading_sections(&all, &[2], None, true).unwrap();
    assert_eq!(spans_of(&p, src, &[(deep[0].start, deep[0].end)]), ["More results\nStill rain."]);

    // Choosing both levels and a heading search that matches both gives one joined section, not two overlapping ones.
    let joined = p.heading_sections(&all, &[1, 2], Some("results"), true).unwrap();
    assert_eq!(joined.len(), 1);
    assert_eq!(spans_of(&p, src, &[(joined[0].start, joined[0].end)]), ["Results\nIt rained.\n\nMore results\nStill rain."]);

    // A heading search that matches nothing, and a document without headings, give nothing.
    assert!(p.heading_sections(&all, &[1], Some("conclusion"), true).unwrap().is_empty());
    p.add_source(&NewSource::new("Plain", SourceType::Document, "No headings at all.")).unwrap();
    assert_eq!(p.heading_levels(&all).unwrap().len(), 2, "the plain document adds no headings");
    // A bad heading search is refused before anything is read.
    assert!(p.heading_sections(&all, &[1], Some("(oops"), true).is_err());
}

// ---- widening ----

#[test]
fn a_found_phrase_widens_by_words_inside_its_own_scope() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let text = "One two three four five six.\n\nSeven eight nine ten.";
    let src = p.add_source(&NewSource::new("Doc", SourceType::Document, text)).unwrap();
    let found = |needle: &str, scope: (&str, &str)| FoundSpan {
        source_id: src,
        scope_start: at(text, scope.0),
        scope_end: at(text, scope.1) + scope.1.len() as i64,
        start: at(text, needle),
        end: at(text, needle) + needle.len() as i64,
        unit_id: None,
    };
    let first = found("three four", ("One", "six."));
    let second = found("eight", ("Seven", "ten."));
    let widened = p.widen_by_words(&[first.clone(), second.clone()], 1).unwrap();
    assert_eq!(widened.len(), 2);
    assert_eq!(p.text_of_span(src, widened[0].start, widened[0].end).unwrap(), "two three four five");
    assert_eq!(p.text_of_span(src, widened[1].start, widened[1].end).unwrap(), "Seven eight nine");
    // Far more words than the scope has: the whole scope, and never the next paragraph.
    let wide = p.widen_by_words(&[first], 50).unwrap();
    assert_eq!(p.text_of_span(src, wide[0].start, wide[0].end).unwrap(), "One two three four five six");
    // No widening leaves the phrase as found.
    let same = p.widen_by_words(std::slice::from_ref(&second), 0).unwrap();
    assert_eq!((same[0].start, same[0].end), (second.start, second.end));
}

// ---- suggestions that repeat earlier work ----

fn proposal(source: i64, start: i64, end: i64, code: i64, coder: i64) -> NewProposal {
    NewProposal { source_id: source, start, end, code_id: code, coder_id: coder, unit_id: None, rationale: "test".into() }
}

#[test]
fn suggestions_that_repeat_earlier_work_are_dropped_and_counted() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = p.add_source(&NewSource::new("Doc", SourceType::Document, "The cost was high. Access was easy. The cost again.")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();
    let cost = p.add_code(&NewCode::new("Cost")).unwrap();
    let access = p.add_code(&NewCode::new("Access")).unwrap();
    // Ada coded the whole first sentence as Cost; Bo coded "cost" in the third.
    p.apply_coding(&NewCoding::new(src, 0, 18, cost, ada)).unwrap();
    p.apply_coding(&NewCoding::new(src, 40, 44, cost, bo)).unwrap();
    // A suggestion already waits for "Access".
    p.create_proposals(ProposalOrigin::Autocode, "earlier", &[proposal(src, 19, 25, access, ada)]).unwrap();

    let items = vec![
        proposal(src, 4, 8, cost, ada),    // inside Ada's coding of the first sentence: dropped
        proposal(src, 0, 18, cost, ada),   // the same words: dropped
        proposal(src, 0, 30, cost, ada),   // wider than Ada's coding: kept
        proposal(src, 40, 44, cost, ada),  // Bo coded these words, not Ada: kept
        proposal(src, 19, 25, access, ada), // already waiting: dropped
        proposal(src, 26, 30, access, ada), // new: kept
    ];
    let out = p.drop_known(items).unwrap();
    assert_eq!((out.already_coded, out.already_waiting), (2, 1));
    assert_eq!(out.kept.iter().map(|i| (i.start, i.end, i.code_id)).collect::<Vec<_>>(), [(0, 30, cost), (40, 44, cost), (26, 30, access)]);
}

// ---- the queue ----

fn queue_project(dir: &TempDir) -> (Project, i64, i64, i64, i64) {
    let mut p = project(dir);
    let text = "First line of the interview.\nThe cost was high and access was hard. We talked more about it later on.\nLast line.";
    let src = p.add_source(&NewSource::new("Interview", SourceType::Document, text)).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let cost = p.add_code(&NewCode::new("Cost")).unwrap();
    let access = p.add_code(&NewCode::new("Access")).unwrap();
    (p, src, coder, cost, access)
}

#[test]
fn the_queue_shows_the_words_their_surroundings_and_whether_they_are_already_coded() {
    let dir = TempDir::new().unwrap();
    let (mut p, src, ada, cost, access) = queue_project(&dir);
    let text = p.source_detail(src).unwrap().text;
    let s = at(&text, "cost");
    p.create_proposals_by(ProposalOrigin::Autocode, "Words: cost", "words", &[proposal(src, s, s + 4, cost, ada)]).unwrap();

    let row = &p.list_proposals(Some("pending"), None, 10).unwrap()[0];
    assert_eq!((row.text.as_str(), row.source_name.as_str(), row.truncated), ("cost", "Interview", false));
    assert_eq!(row.before, "The ", "the surroundings stop at the line break, so another line is never mixed in");
    assert!(row.after.starts_with(" was high and access was hard."));
    assert!(row.after.chars().count() <= 70);
    assert!(!row.already_coded);

    // When the same coder codes these words by hand afterwards, the queue says so.
    p.apply_coding(&NewCoding::new(src, s, s + 4, cost, ada)).unwrap();
    assert!(p.get_proposal(row.id).unwrap().already_coded);
    // ...and accepting it is refused, and it stays in the queue.
    let out = p.decide_proposals(&[row.id], true).unwrap();
    assert_eq!((out.accepted, out.failed.len()), (0, 1));
    assert!(out.failed[0].1.contains("already have this code"));
    assert_eq!(p.list_proposals(Some("pending"), None, 10).unwrap().len(), 1);

    // A long suggestion is cut and says so; a page of the queue starts where the last one stopped.
    p.create_proposals(ProposalOrigin::Autocode, "long", &[proposal(src, 0, text.chars().count() as i64, access, ada)]).unwrap();
    let all = p.list_proposals_page(None, None, 10, 0).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(p.list_proposals_page(None, None, 10, 1).unwrap()[0].id, all[1].id);
    assert!(p.list_proposals_page(None, None, 10, 2).unwrap().is_empty());
}

#[test]
fn batches_say_how_they_were_made_and_how_far_the_decisions_have_got() {
    let dir = TempDir::new().unwrap();
    let (mut p, src, ada, cost, access) = queue_project(&dir);
    let first = p
        .create_proposals_by(ProposalOrigin::Autocode, "Words: cost", "words", &[proposal(src, 33, 37, cost, ada), proposal(src, 0, 5, cost, ada), proposal(src, 41, 47, access, ada)])
        .unwrap();
    let second = p.create_proposals_by(ProposalOrigin::Autocode, "Speakers", "speakers", &[proposal(src, 6, 10, access, ada)]).unwrap();
    let ids: Vec<i64> = p.list_proposals(None, Some(&first.batch), 10).unwrap().iter().map(|r| r.id).collect();
    p.decide_proposals(&ids[..1], true).unwrap();
    p.decide_proposals(&ids[1..2], false).unwrap();

    let batches = p.proposal_batches().unwrap();
    assert_eq!(batches.iter().map(|b| b.batch.as_str()).collect::<Vec<_>>(), [second.batch.as_str(), first.batch.as_str()], "newest first");
    let b = &batches[1];
    assert_eq!((b.label.as_str(), b.method.as_str(), b.origin.as_str()), ("Words: cost", "words", "autocode"));
    assert_eq!((b.pending, b.accepted, b.rejected), (1, 1, 1));
    assert_eq!(b.code_ids, [cost, access]);
    assert_eq!((batches[0].method.as_str(), batches[0].pending), ("speakers", 1));

    // Everything left waiting in one batch can be decided at once.
    let out = p.decide_batch(&first.batch, false).unwrap();
    assert_eq!((out.accepted, out.rejected), (0, 1));
    assert_eq!(p.proposal_batches().unwrap()[1].pending, 0);
    assert_eq!(p.proposal_batches().unwrap()[0].pending, 1, "the other batch is untouched");
}

// ---- adjusting ----

#[test]
fn a_waiting_suggestion_can_be_adjusted_and_the_change_undone() {
    let dir = TempDir::new().unwrap();
    let (mut p, src, ada, cost, access) = queue_project(&dir);
    p.create_proposals(ProposalOrigin::Autocode, "t", &[proposal(src, 33, 37, cost, ada)]).unwrap();
    let id = p.list_proposals(None, None, 10).unwrap()[0].id;

    let row = p.adjust_proposal(id, 29, 50, access).unwrap();
    assert_eq!((row.start, row.end, row.code_id, row.text.as_str()), (29, 50, access, "The cost was high and"));
    assert_eq!(p.next_undo().unwrap().as_deref(), Some("Restored the suggestion's earlier words and code"));
    // Setting what it already is changes nothing and logs nothing.
    p.adjust_proposal(id, 29, 50, access).unwrap();
    let events = p.events(100).unwrap();
    assert_eq!(events.iter().filter(|e| e.action == "proposal.adjust").count(), 1);

    p.undo().unwrap().unwrap();
    let back = p.get_proposal(id).unwrap();
    assert_eq!((back.start, back.end, back.code_id), (33, 37, cost));

    // Accepting uses the adjusted words and code.
    p.adjust_proposal(id, 29, 50, access).unwrap();
    p.decide_proposals(&[id], true).unwrap();
    let codings = p.list_codings(src).unwrap();
    assert_eq!((codings[0].start, codings[0].end, codings[0].code_id), (29, 50, access));

    // A decided suggestion, words outside the document, a retired code, and an empty span are all refused.
    assert!(p.adjust_proposal(id, 0, 5, cost).unwrap_err().to_string().contains("already accepted"));
    p.create_proposals(ProposalOrigin::Autocode, "t2", &[proposal(src, 0, 5, cost, ada)]).unwrap();
    let id2 = p.list_proposals(Some("pending"), None, 10).unwrap()[0].id;
    assert!(p.adjust_proposal(id2, 0, 100_000, cost).is_err());
    assert!(p.adjust_proposal(id2, 5, 5, cost).is_err());
    p.retire_code(access).unwrap();
    assert!(p.adjust_proposal(id2, 0, 5, access).unwrap_err().to_string().contains("active code"));
}

#[test]
fn a_suggestion_tied_to_a_unit_cannot_be_moved_out_of_it() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = transcript(&mut p);
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Speech")).unwrap();
    let text = p.source_detail(src).unwrap().text;
    let unit = p.list_units(src).unwrap().into_iter().find(|u| u.speaker.as_deref() == Some("Priya")).unwrap();
    p.create_proposals(
        ProposalOrigin::Autocode,
        "speakers",
        &[NewProposal { source_id: src, start: unit.start, end: unit.end, code_id: code, coder_id: ada, unit_id: Some(unit.id), rationale: String::new() }],
    )
    .unwrap();
    let id = p.list_proposals(None, None, 10).unwrap()[0].id;
    assert!(p.adjust_proposal(id, unit.start, unit.end + 5, code).unwrap_err().to_string().contains("inside the unit"));
    let narrower = p.adjust_proposal(id, unit.start + 4, unit.end, code).unwrap();
    assert_eq!(narrower.text, "bus is late.");
    let _ = text;
}

// ---- deciding and undoing ----

#[test]
fn deciding_many_at_once_is_one_undo_and_the_batch_itself_can_be_taken_back() {
    let dir = TempDir::new().unwrap();
    let (mut p, src, ada, cost, _) = queue_project(&dir);
    let items: Vec<NewProposal> = [(0, 5), (6, 10), (11, 15)].iter().map(|(s, e)| proposal(src, *s, *e, cost, ada)).collect();
    p.create_proposals(ProposalOrigin::Autocode, "three", &items).unwrap();
    let ids: Vec<i64> = p.list_proposals(None, None, 10).unwrap().iter().map(|r| r.id).collect();

    p.decide_proposals(&ids, true).unwrap();
    assert_eq!(p.list_codings(src).unwrap().len(), 3);
    assert_eq!(p.next_undo().unwrap().as_deref(), Some("Took back 3 accepted suggestions"));

    let undone = p.undo().unwrap().unwrap();
    assert_eq!(undone.what, "Took back 3 accepted suggestions");
    assert!(p.list_codings(src).unwrap().is_empty(), "every coding made by the decision is gone");
    assert_eq!(p.list_proposals(Some("pending"), None, 10).unwrap().len(), 3, "and the three wait in the queue again");

    // The next undo reaches the one before: making the suggestions. It removes them from the queue.
    assert_eq!(p.next_undo().unwrap().as_deref(), Some("Took back the suggestions that were made"));
    assert_eq!(p.undo().unwrap().unwrap().action, "proposal.create");
    assert!(p.list_proposals(None, None, 10).unwrap().is_empty());
    assert!(p.proposal_batches().unwrap().is_empty());
}

#[test]
fn rejecting_many_at_once_is_one_undo_too() {
    let dir = TempDir::new().unwrap();
    let (mut p, src, ada, cost, _) = queue_project(&dir);
    let items: Vec<NewProposal> = [(0, 5), (6, 10)].iter().map(|(s, e)| proposal(src, *s, *e, cost, ada)).collect();
    let batch = p.create_proposals(ProposalOrigin::Autocode, "two", &items).unwrap();
    let out = p.decide_batch(&batch.batch, false).unwrap();
    assert_eq!((out.accepted, out.rejected), (0, 2));
    assert_eq!(p.next_undo().unwrap().as_deref(), Some("Put 2 rejected suggestions back in the queue"));
    assert_eq!(p.undo().unwrap().unwrap().what, "Put 2 rejected suggestions back in the queue");
    assert_eq!(p.list_proposals(Some("pending"), None, 10).unwrap().len(), 2);
}

#[test]
fn a_decision_is_one_undo_even_when_one_suggestion_in_it_failed() {
    let dir = TempDir::new().unwrap();
    let (mut p, src, ada, cost, access) = queue_project(&dir);
    p.create_proposals(ProposalOrigin::Autocode, "mixed", &[proposal(src, 0, 5, cost, ada), proposal(src, 6, 10, access, ada)]).unwrap();
    let ids: Vec<i64> = p.list_proposals(None, None, 10).unwrap().iter().map(|r| r.id).collect();
    p.retire_code(access).unwrap();
    let out = p.decide_proposals(&ids, true).unwrap();
    assert_eq!((out.accepted, out.failed.len()), (1, 1));
    assert_eq!(p.next_undo().unwrap().as_deref(), Some("Took back an accepted suggestion"), "one suggestion went through, so one is taken back");
    p.undo().unwrap().unwrap();
    assert!(p.list_codings(src).unwrap().is_empty());
}

#[test]
fn a_coding_made_by_accepting_keeps_its_history_and_is_not_undone_on_its_own() {
    let dir = TempDir::new().unwrap();
    let (mut p, src, ada, cost, _) = queue_project(&dir);
    p.create_proposals(ProposalOrigin::Autocode, "one", &[proposal(src, 0, 5, cost, ada)]).unwrap();
    let id = p.list_proposals(None, None, 10).unwrap()[0].id;
    p.decide_proposals(&[id], true).unwrap();
    let coding = p.list_codings(src).unwrap()[0].id;
    assert_eq!(p.coding_history(coding).unwrap()[0].action, "coding.add", "the first entry is still the one that created it");
    assert_eq!(p.undo().unwrap().unwrap().action, "proposal.accept");
    // The coding.add of the accepted suggestion does not stand in the way of the undo before it.
    assert_eq!(p.undo().unwrap().unwrap().action, "proposal.create");
}
