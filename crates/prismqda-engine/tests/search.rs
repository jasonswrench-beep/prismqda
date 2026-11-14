//! Searching a project and reviewing suggested codings.

use prismqda_engine::{
    AppliesTo, AttributeSpec, ImportBatch, ImportedSource, ImportedUnit, Level, NewCode, NewCoder, NewCoding, NewProject, NewProposal,
    NewSource, OpenOptions, Project, ProposalOrigin, SearchFilter, SourceType, UnitKind,
};
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn docs(p: &mut Project) -> (i64, i64) {
    let a = p
        .add_source(&NewSource::new("Interview A", SourceType::Document, "The cost was high.\n\nAccess to care was easy.\n\nWe talked about the cost of access."))
        .unwrap();
    let b = p.add_source(&NewSource::new("Interview B", SourceType::Document, "Nothing relevant here.\n\nCost again, costly.")).unwrap();
    (a, b)
}

#[test]
fn search_finds_scopes_with_spans_and_context() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let (a, b) = docs(&mut p);

    let r = p.search("cost", &SearchFilter::default()).unwrap();
    assert_eq!(r.total, 3, "two paragraphs in A and one in B: 'costly' is not 'cost'");
    assert_eq!(r.hits.iter().map(|h| (h.source_id, h.unit_id)).collect::<Vec<_>>(), [(a, None), (a, None), (b, None)]);
    let first = &r.hits[0];
    assert_eq!((first.scope_start, first.scope_end), (0, 18));
    assert_eq!(first.spans, vec![(4, 8)]);
    assert!(first.snippet.contains("cost was high"));
    assert_eq!(r.hits[0].source_name, "Interview A");

    // Offsets are real: the span's text is the match.
    for h in &r.hits {
        for (s, e) in &h.spans {
            assert_eq!(p.text_of_span(h.source_id, *s, *e).unwrap().to_lowercase(), "cost");
        }
    }

    // Booleans are decided within a paragraph, not across the whole source.
    let both = p.search("cost AND access", &SearchFilter::default()).unwrap();
    assert_eq!(both.total, 1);
    assert_eq!((both.hits[0].scope_start, both.hits[0].spans.len()), (46, 2));
    assert_eq!(p.search("cost NOT access", &SearchFilter::default()).unwrap().total, 2);
    assert_eq!(p.search("cost NEAR/3 access", &SearchFilter::default()).unwrap().total, 1);
    assert_eq!(p.search("\"cost of access\"", &SearchFilter::default()).unwrap().total, 1);
    assert_eq!(p.search("cost*", &SearchFilter::default()).unwrap().total, 3, "cost* also matches costly, in the same paragraph as cost");
}

#[test]
fn units_are_the_scopes_when_a_source_has_them() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let text = "I love the cost.\n\nGreat access\n\nPoor access, high cost";
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Thread".into(),
            source_type: SourceType::Social,
            folder: String::new(),
            text: text.into(),
            runs: vec![],
            units: vec![
                ImportedUnit::new(0, 16, UnitKind::Comment),
                ImportedUnit::new(18, 30, UnitKind::Comment),
                ImportedUnit { excluded: true, ..ImportedUnit::new(32, 54, UnitKind::Comment) },
            ],
            default_unit_kind: UnitKind::Comment,
            unit_attributes: vec![AttributeSpec::new("x", AppliesTo::Unit, Level::Text)],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap();
    let r = p.search("cost", &SearchFilter::default()).unwrap();
    assert_eq!(r.total, 1, "the excluded unit is not searched");
    assert!(r.hits[0].unit_id.is_some());
    assert_eq!((r.hits[0].scope_start, r.hits[0].scope_end), (0, 16));
    assert_eq!(p.search("access", &SearchFilter::default()).unwrap().total, 1);
}

#[test]
fn filters_narrow_by_source_case_and_existing_code() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let (a, b) = docs(&mut p);
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Cost")).unwrap();
    p.apply_coding(&NewCoding::new(a, 4, 8, code, coder)).unwrap();

    let only_b = SearchFilter { source_ids: Some(vec![b]), ..Default::default() };
    assert_eq!(p.search("cost", &only_b).unwrap().total, 1);
    let uncoded = SearchFilter { without_code: Some(code), ..Default::default() };
    assert_eq!(p.search("cost", &uncoded).unwrap().total, 2, "the first paragraph of A already carries the code");
    let coded = SearchFilter { with_code: Some(code), ..Default::default() };
    assert_eq!(p.search("cost", &coded).unwrap().total, 1);
    let limited = SearchFilter { limit: Some(1), ..Default::default() };
    let r = p.search("cost", &limited).unwrap();
    assert_eq!((r.hits.len(), r.total, r.truncated), (1, 3, true), "the total is still counted");
}

#[test]
fn a_bad_query_is_refused_before_anything_is_searched() {
    let dir = TempDir::new().unwrap();
    let p = project(&dir);
    assert!(p.search("(cost", &SearchFilter::default()).is_err());
    assert!(p.search("", &SearchFilter::default()).is_err());
    assert_eq!(p.search("cost", &SearchFilter::default()).unwrap().total, 0, "an empty project has no hits");
}

// ---- the review queue ----

fn suggestions(p: &mut Project) -> (Vec<NewProposal>, i64, i64, i64) {
    let (a, _) = docs(p);
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("Cost")).unwrap();
    let hits = p.search("cost", &SearchFilter { source_ids: Some(vec![a]), ..Default::default() }).unwrap();
    let items = hits
        .hits
        .iter()
        .flat_map(|h| h.spans.iter().map(move |(s, e)| (h.source_id, *s, *e)))
        .map(|(source_id, start, end)| NewProposal { source_id, start, end, code_id: code, coder_id: coder, unit_id: None, rationale: "autocoded from search: cost".into() })
        .collect();
    (items, a, code, coder)
}

#[test]
fn autocoded_suggestions_wait_for_a_decision_and_only_accepted_ones_become_codings() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let (items, a, code, _) = suggestions(&mut p);
    assert_eq!(items.len(), 2);
    let batch = p.create_proposals(ProposalOrigin::Autocode, "search: cost", &items).unwrap();
    assert_eq!(batch.created, 2);
    assert!(batch.batch.starts_with("searchcost-"));
    assert_eq!(p.list_codings(a).unwrap().len(), 0, "a suggestion is not a coding");

    let pending = p.list_proposals(Some("pending"), None, 100).unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!((pending[0].origin.as_str(), pending[0].start, pending[0].end, pending[0].code_id), ("autocode", 4, 8, code));

    let out = p.decide_proposals(&[pending[0].id], true).unwrap();
    assert_eq!((out.accepted, out.rejected, out.failed.len()), (1, 0, 0));
    let out = p.decide_proposals(&[pending[1].id], false).unwrap();
    assert_eq!((out.accepted, out.rejected), (0, 1));

    let codings = p.list_codings(a).unwrap();
    assert_eq!(codings.len(), 1);
    assert_eq!((codings[0].start, codings[0].end, codings[0].origin.as_str()), (4, 8, "human"));
    assert_eq!(codings[0].rationale, "autocoded from search: cost");
    assert_eq!(p.list_proposals(Some("accepted"), None, 10).unwrap().len(), 1);
    assert_eq!(p.list_proposals(Some("pending"), Some(&batch.batch), 10).unwrap().len(), 0);

    let actions: Vec<String> = p.events(100).unwrap().into_iter().map(|e| e.action).collect();
    for want in ["proposal.create", "proposal.accept", "proposal.reject", "coding.add"] {
        assert!(actions.iter().any(|a| a == want), "missing {want} in {actions:?}");
    }

    // Deciding twice is reported, not repeated.
    let again = p.decide_proposals(&[pending[0].id], true).unwrap();
    assert_eq!((again.accepted, again.failed.len()), (0, 1));
    assert!(again.failed[0].1.contains("already accepted"));
}

#[test]
fn an_ai_suggestion_keeps_its_origin_when_accepted() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let (items, a, ..) = suggestions(&mut p);
    p.create_proposals(ProposalOrigin::Ai, "model run", &items[..1]).unwrap();
    let id = p.list_proposals(None, None, 10).unwrap()[0].id;
    p.decide_proposals(&[id], true).unwrap();
    assert_eq!(p.list_codings(a).unwrap()[0].origin, "ai_suggestion_accepted");
}

#[test]
fn a_suggestion_that_can_no_longer_be_accepted_stays_pending_and_the_rest_go_through() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let (items, a, code, coder) = suggestions(&mut p);
    let other = p.add_code(&NewCode::new("Access")).unwrap();
    let mut mixed = items.clone();
    mixed[1].code_id = other;
    p.create_proposals(ProposalOrigin::Autocode, "mixed", &mixed).unwrap();
    p.retire_code(other).unwrap();

    let ids: Vec<i64> = p.list_proposals(Some("pending"), None, 10).unwrap().iter().map(|r| r.id).collect();
    let out = p.decide_proposals(&ids, true).unwrap();
    assert_eq!((out.accepted, out.failed.len()), (1, 1));
    assert!(out.failed[0].1.contains("retired"), "{:?}", out.failed);
    assert_eq!(p.list_proposals(Some("pending"), None, 10).unwrap().len(), 1, "the failed one stays pending");
    assert_eq!(p.list_codings(a).unwrap().len(), 1);
    let _ = (code, coder);
}

#[test]
fn bad_suggestions_are_refused_up_front() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let (items, ..) = suggestions(&mut p);
    assert!(p.create_proposals(ProposalOrigin::Autocode, "x", &[]).is_err());
    let mut bad = items.clone();
    bad[0].end = 9999;
    assert!(p.create_proposals(ProposalOrigin::Autocode, "x", &bad).unwrap_err().to_string().contains("does not fit"));
    let mut bad = items.clone();
    bad[0].code_id = 999;
    assert!(p.create_proposals(ProposalOrigin::Autocode, "x", &bad).is_err());
    assert_eq!(p.list_proposals(None, None, 10).unwrap().len(), 0, "a refused batch leaves nothing behind");
    assert!(p.decide_proposals(&[12345], true).is_err());
}
