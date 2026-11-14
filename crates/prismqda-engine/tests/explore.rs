//! Exploring the coding: code matrices, how codes occur together, summaries, and queries, against a small project whose
//! every number was worked out by hand.
//!
//! Documents and cases (one case each, with a variable "Group"):  alex (Residents), priya (Residents), jordan (Visitors).
//! Codes: Barriers (a parent) with Reliability and Cost under it, and Comfort. Coders: Ada and Bo.
//!
//! Codings (document, words, code, coder):
//!   alex   0..10 Reliability Ada      alex   0..10 Reliability Bo      alex   5..15 Cost Ada
//!   alex  20..30 Cost Ada             alex  40..50 Comfort Ada         alex   0..10 Cost Bo
//!   priya  0..10 Reliability Ada      priya 12..20 Comfort Ada
//!   jordan 0..10 Cost Bo              jordan 0..10 Comfort Bo

use prismqda_engine::{
    AppliesTo, AttributeSpec, CodeEdit, Combine, Counts, ExploreScope, ImportBatch, ImportedCase, ImportedSource, Level, MatrixSpec, NewCode, NewCoder, NewCoding, NewProject, OpenOptions,
    Project, QueryNode, QuerySpec, RelationsSpec, ResultUnit, RowSpec, SourceType, UnitKind, CaseMember,
};
use serde_json::{json, Value};
use tempfile::TempDir;

#[allow(dead_code)]
struct W {
    p: Project,
    _dir: TempDir,
    alex: i64,
    priya: i64,
    jordan: i64,
    barriers: i64,
    reliability: i64,
    cost: i64,
    comfort: i64,
    ada: i64,
    bo: i64,
}

fn source(name: &str, text: &str) -> ImportedSource {
    ImportedSource {
        name: name.to_string(),
        source_type: SourceType::Document,
        folder: String::new(),
        text: text.to_string(),
        runs: vec![],
        units: vec![],
        default_unit_kind: UnitKind::Sentence,
        unit_attributes: vec![],
        original_file_hash: None,
        settings: Value::Null,
        warnings: vec![],
    }
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let batch = ImportBatch {
        sources: vec![
            source("alex", "Delays make me anxious. Fares cost too much! Parks are nice."),
            source("priya", "Trains stop when the signals fail. Announcements would help."),
            source("jordan", "Walking is pleasant but the paths are dark after nine pm."),
        ],
        cases: ["Alex", "Priya", "Jordan"]
            .iter()
            .enumerate()
            .map(|(i, name)| ImportedCase {
                name: name.to_string(),
                attrs: vec![("Group".to_string(), if i < 2 { "Residents" } else { "Visitors" }.to_string())],
                members: vec![CaseMember { source: i, unit: None }],
            })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![AttributeSpec::new("Group", AppliesTo::Case, Level::Nominal)],
    };
    let out = p.apply_import(&batch).unwrap();
    let (alex, priya, jordan) = (out.source_ids[0], out.source_ids[1], out.source_ids[2]);
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();
    let barriers = p.add_code(&NewCode::new("Barriers")).unwrap();
    let mut child = |name: &str, parent: Option<i64>| {
        let mut c = NewCode::new(name);
        c.parent_id = parent;
        p.add_code(&c).unwrap()
    };
    let reliability = child("Reliability", Some(barriers));
    let cost = child("Cost", Some(barriers));
    let comfort = child("Comfort", None);
    let mut code = |source: i64, start: i64, end: i64, code: i64, coder: i64| {
        p.apply_coding(&NewCoding::new(source, start, end, code, coder)).unwrap()
    };
    code(alex, 0, 10, reliability, ada);
    code(alex, 0, 10, reliability, bo);
    code(alex, 5, 15, cost, ada);
    code(alex, 20, 30, cost, ada);
    code(alex, 40, 50, comfort, ada);
    code(alex, 0, 10, cost, bo);
    code(priya, 0, 10, reliability, ada);
    code(priya, 12, 20, comfort, ada);
    code(jordan, 0, 10, cost, bo);
    code(jordan, 0, 10, comfort, bo);
    W { p, _dir: dir, alex, priya, jordan, barriers, reliability, cost, comfort, ada, bo }
}

fn c(assignments: i64, excerpts: i64, documents: i64, cases: i64) -> Counts {
    Counts { assignments, excerpts, documents, cases }
}

fn matrix_spec(_w: &W, rows: RowSpec, codes: &[i64], descendants: bool) -> MatrixSpec {
    MatrixSpec { rows, code_ids: codes.to_vec(), include_descendants: descendants, scope: ExploreScope::default() }
}

fn row<'a>(m: &'a prismqda_engine::Matrix, label: &str) -> &'a prismqda_engine::MatrixRow {
    m.rows.iter().find(|r| r.label == label).unwrap_or_else(|| panic!("no row {label}: {:?}", m.rows.iter().map(|r| &r.label).collect::<Vec<_>>()))
}

// ---------------------------------------------------------------- the code matrix

#[test]
fn cases_against_codes_count_assignments_excerpts_documents_and_cases_apart() {
    let w = world();
    let m = w.p.code_matrix(&matrix_spec(&w, RowSpec::Cases, &[w.reliability, w.cost], true)).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Alex", "Jordan", "Priya"], "rows by name; 'Not in any case' only when something is in it");

    let alex = row(&m, "Alex");
    assert_eq!(alex.cells, vec![c(2, 1, 1, 1), c(3, 3, 1, 1)], "two coders on one excerpt: 2 assignments, 1 excerpt");
    assert_eq!(alex.total, c(5, 3, 1, 1), "over both columns each coding once, and each excerpt once: the words 0..10 are in both");
    assert_eq!(row(&m, "Priya").cells, vec![c(1, 1, 1, 1), c(0, 0, 0, 0)]);
    assert_eq!(row(&m, "Jordan").cells, vec![c(0, 0, 0, 0), c(1, 1, 1, 1)]);

    assert_eq!(m.columns[0].total, c(3, 2, 2, 2));
    assert_eq!(m.columns[1].total, c(4, 4, 2, 2));
    assert_eq!(m.total, c(7, 5, 3, 3));
    assert_eq!(m.universe, c(10, 7, 3, 3), "the universe is every coding looked at, before the columns were chosen");
    assert!(m.notes.iter().any(|n| n.contains("Assignments count codings")));
}

#[test]
fn a_parent_counts_the_codings_of_its_children_once_and_can_be_asked_not_to() {
    let w = world();
    let with = w.p.code_matrix(&matrix_spec(&w, RowSpec::Cases, &[w.barriers, w.reliability, w.cost], true)).unwrap();
    // Barriers has no codings of its own; with its sub-codes it has every Reliability and Cost coding, each once.
    assert_eq!(with.columns[0].total, c(7, 5, 3, 3));
    let sum_of_children = with.columns[1].total.excerpts + with.columns[2].total.excerpts;
    assert_eq!(sum_of_children, 6, "the children's excerpts add up to more than the parent has, because {{alex 0..10}} is in both");
    assert_eq!(with.columns[0].total.excerpts, 5, "so the parent does not add them up: it counts the stretches themselves");

    let without = w.p.code_matrix(&matrix_spec(&w, RowSpec::Cases, &[w.barriers], false)).unwrap();
    assert_eq!(without.columns[0].total, c(0, 0, 0, 0));
    assert!(without.notes.iter().any(|n| n.contains("only the codings of its own code")));
}

#[test]
fn a_case_variable_groups_the_cases_and_says_how_many_are_in_each_group() {
    let w = world();
    let m = w.p.code_matrix(&matrix_spec(&w, RowSpec::CaseVariable { name: "Group".to_string() }, &[w.reliability, w.cost], true)).unwrap();
    assert_eq!(m.rows.iter().map(|r| (r.label.as_str(), r.size)).collect::<Vec<_>>(), [("Residents", Some(2)), ("Visitors", Some(1))]);
    assert_eq!(row(&m, "Residents").cells, vec![c(3, 2, 2, 2), c(3, 3, 1, 1)], "the cases metric counts cases with evidence: both residents have Reliability, one has Cost");
    assert_eq!(row(&m, "Visitors").cells, vec![c(0, 0, 0, 0), c(1, 1, 1, 1)]);
    let missing = w.p.code_matrix(&matrix_spec(&w, RowSpec::CaseVariable { name: "Nope".to_string() }, &[w.cost], true)).unwrap_err();
    assert!(missing.to_string().contains("no case variable is named 'Nope'"), "{missing}");
}

#[test]
fn documents_are_rows_too_and_a_scope_narrows_what_is_counted() {
    let w = world();
    let mut spec = matrix_spec(&w, RowSpec::Documents, &[w.reliability, w.cost], true);
    let m = w.p.code_matrix(&spec).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["alex", "jordan", "priya"]);
    assert_eq!(row(&m, "alex").total, c(5, 3, 1, 1));

    spec.scope = ExploreScope { coder_ids: Some(vec![w.ada]), ..ExploreScope::default() };
    let ada_only = w.p.code_matrix(&spec).unwrap();
    assert_eq!(ada_only.columns[0].total, c(2, 2, 2, 2), "Ada's Reliability: alex and priya");
    assert_eq!(ada_only.columns[1].total, c(2, 2, 1, 1), "Ada's Cost: two excerpts in alex");
    assert_eq!(ada_only.universe.assignments, 6, "Ada made six of the ten codings");

    spec.scope = ExploreScope { source_ids: Some(vec![w.jordan]), ..ExploreScope::default() };
    assert_eq!(w.p.code_matrix(&spec).unwrap().total, c(1, 1, 1, 1));
}

#[test]
fn a_cell_row_column_or_the_whole_matrix_gives_the_codings_behind_it() {
    let w = world();
    let spec = matrix_spec(&w, RowSpec::Cases, &[w.reliability, w.cost], true);
    let alex_row = format!("case:{}", w.p.list_cases().unwrap().iter().find(|c| c.name == "Alex").unwrap().id);
    let cell = w.p.code_matrix_evidence(&spec, Some(&alex_row), Some(w.cost)).unwrap();
    assert_eq!(cell.len(), 3, "Alex's three Cost codings");
    let row_all = w.p.code_matrix_evidence(&spec, Some(&alex_row), None).unwrap();
    assert_eq!(row_all.len(), 5);
    assert_eq!(w.p.code_matrix_evidence(&spec, None, Some(w.reliability)).unwrap().len(), 3);
    assert_eq!(w.p.code_matrix_evidence(&spec, None, None).unwrap().len(), 7);
    assert!(w.p.code_matrix_evidence(&spec, None, Some(w.comfort)).is_err(), "a code that is not a column has no cell");
    // The evidence is what the cell counted: every coding's text is the words it counts.
    let segments = w.p.coded_segments(&prismqda_engine::SegmentFilter { coding_ids: Some(cell), ..Default::default() }).unwrap();
    assert!(segments.iter().all(|s| s.code_name == "Cost" && s.source_name == "alex"));
}

#[test]
fn a_document_in_no_case_gets_a_row_of_its_own() {
    let mut w = world();
    let loose = w.p.apply_import(&ImportBatch { sources: vec![source("loose", "A document that no case holds, with words in it.")], ..Default::default() }).unwrap().source_ids[0];
    w.p.apply_coding(&NewCoding::new(loose, 0, 10, w.cost, w.ada)).unwrap();
    let m = w.p.code_matrix(&matrix_spec(&w, RowSpec::Cases, &[w.cost], true)).unwrap();
    assert_eq!(row(&m, "Not in any case").cells, vec![c(1, 1, 1, 0)], "it has evidence in one document and no case");
    assert_eq!(m.rows.len(), 4);
}

// ---------------------------------------------------------------- how codes occur together

fn relations(w: &W, mode: &str, same_coder: bool) -> RelationsSpec {
    serde_json::from_value(json!({ "code_ids": [w.reliability, w.cost, w.comfort], "mode": mode, "same_coder": same_coder })).unwrap()
}

#[test]
fn codes_that_overlap_are_counted_as_pairs_of_codings_by_the_same_coder() {
    let w = world();
    let r = w.p.code_relations(&relations(&w, "overlap", true)).unwrap();
    assert_eq!(r.codes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Reliability", "Cost", "Comfort"]);
    // Ada: Reliability alex 0..10 with Cost 5..15.  Bo: Reliability 0..10 with Cost 0..10.  Bo in jordan: Cost with Comfort.
    assert_eq!(r.cells, vec![vec![3, 2, 0], vec![2, 4, 1], vec![0, 1, 3]], "{:?}", r.cells);
    let anyone = w.p.code_relations(&relations(&w, "overlap", false)).unwrap();
    assert_eq!(anyone.cells[0][1], 4, "whoever made them, two more pairs (Ada's Reliability with Bo's Cost, and Bo's with Ada's)");
    assert_eq!(anyone.cells[0][1], anyone.cells[1][0], "a matrix of pairs is the same from either side");
}

#[test]
fn codes_in_the_same_document_or_case_do_not_need_to_overlap() {
    let w = world();
    let docs = w.p.code_relations(&relations(&w, "document", true)).unwrap();
    // Reliability: alex and priya. Cost: alex and jordan. Comfort: alex, priya and jordan.
    assert_eq!(docs.cells, vec![vec![2, 1, 2], vec![1, 2, 2], vec![2, 2, 3]]);
    assert_eq!(w.p.code_relations(&relations(&w, "case", true)).unwrap().cells, docs.cells, "each case holds one document here");
}

#[test]
fn leaving_the_codes_out_takes_every_active_code_with_a_coding_and_the_parents_of_those_codes() {
    let w = world();
    let spec: RelationsSpec = serde_json::from_value(json!({ "mode": "document" })).unwrap();
    let r = w.p.code_relations(&spec).unwrap();
    let mut names: Vec<&str> = r.codes.iter().map(|c| c.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Barriers", "Comfort", "Cost", "Reliability"]);
    let own: RelationsSpec = serde_json::from_value(json!({ "mode": "document", "include_descendants": false })).unwrap();
    assert_eq!(w.p.code_relations(&own).unwrap().codes.len(), 3, "without sub-codes counting, Barriers has no codings of its own, so it is left out");
}

#[test]
fn the_evidence_for_a_pair_is_the_codings_on_both_sides() {
    let w = world();
    let spec = relations(&w, "overlap", true);
    let ids = w.p.code_relations_evidence(&spec, w.reliability, w.cost).unwrap();
    assert_eq!(ids.len(), 4, "two pairs, so four codings");
    assert_eq!(w.p.code_relations_evidence(&spec, w.reliability, w.comfort).unwrap(), Vec::<i64>::new());
    assert_eq!(w.p.code_relations_evidence(&spec, w.comfort, w.comfort).unwrap().len(), 3, "a code with itself is its own codings");
    let docs = w.p.code_relations_evidence(&relations(&w, "document", true), w.reliability, w.cost).unwrap();
    assert_eq!(docs.len(), 5, "the codings of Reliability and Cost in the one document that has both (alex)");
}

// ---------------------------------------------------------------- a summary of each code

#[test]
fn every_code_has_a_summary_with_its_sub_codes_counted_in() {
    let w = world();
    let s = w.p.code_summaries(&ExploreScope::default()).unwrap();
    let by = |name: &str| s.iter().find(|x| x.name == name).unwrap();
    let barriers = by("Barriers");
    assert_eq!((barriers.own_assignments, barriers.assignments, barriers.excerpts, barriers.documents, barriers.cases, barriers.coders), (0, 7, 5, 3, 3, 2));
    let reliability = by("Reliability");
    assert_eq!((reliability.own_assignments, reliability.assignments, reliability.excerpts, reliability.documents, reliability.coders), (3, 3, 2, 2, 2));
    assert_eq!((by("Comfort").assignments, by("Comfort").excerpts, by("Comfort").documents), (3, 3, 3));
}

#[test]
fn a_profile_says_where_a_code_is_used_by_whom_and_what_it_occurs_with() {
    let w = world();
    let p = w.p.code_profile(w.reliability, true, &ExploreScope::default()).unwrap();
    assert_eq!(p.by_document.iter().map(|d| (d.name.as_str(), d.assignments, d.excerpts)).collect::<Vec<_>>(), [("alex", 2, 1), ("priya", 1, 1)]);
    assert_eq!(p.by_case.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["Alex", "Priya"]);
    assert_eq!(p.by_coder.iter().map(|d| (d.name.as_str(), d.assignments)).collect::<Vec<_>>(), [("Ada", 2), ("Bo", 1)]);
    assert_eq!(p.together_with.iter().map(|d| (d.name.as_str(), d.assignments)).collect::<Vec<_>>(), [("Cost", 2)]);
    assert_eq!(p.coding_ids.len(), 3);
    assert!(w.p.code_profile(9999, true, &ExploreScope::default()).is_err());
}

// ---------------------------------------------------------------- queries

fn query(rule: Value, unit: &str) -> QuerySpec {
    serde_json::from_value(json!({ "rule": rule, "result_unit": unit })).unwrap()
}

fn code_clause(ids: &[i64]) -> Value {
    json!({ "type": "code", "code_ids": ids })
}

#[test]
fn reliability_or_cost_returns_a_stretch_once_however_many_codings_it_has() {
    let w = world();
    let rule = json!({ "type": "group", "match": "any", "items": [code_clause(&[w.reliability]), code_clause(&[w.cost])] });
    let r = w.p.run_query(&query(rule.clone(), "excerpts")).unwrap();
    assert_eq!((r.total, r.assignments, r.truncated), (5, 7, false));
    let both = r.items.iter().find(|i| i.source_name == "alex" && i.start == 0 && i.end == 10).unwrap();
    assert_eq!(both.text, "Delays mak");
    assert_eq!(both.codings.len(), 3, "the stretch is returned once, with all three codings that matched");
    assert_eq!((r.universe.assignments, r.universe.excerpts, r.universe.documents), (10, 7, 3), "what was looked at before the rule");
    let assignments = w.p.run_query(&query(rule, "assignments")).unwrap();
    assert_eq!(assignments.total, 7, "as assignments each coding is a result");
    assert!(assignments.items.iter().all(|i| i.codings.len() == 1));
}

#[test]
fn and_means_every_clause_and_a_code_can_be_asked_for_together_with_another() {
    let w = world();
    // Reliability that is also coded Cost by the same coder, on overlapping words.
    let rule = json!({ "type": "group", "match": "all", "items": [code_clause(&[w.reliability]), { "type": "also_coded", "code_ids": [w.cost] }] });
    let r = w.p.run_query(&query(rule, "assignments")).unwrap();
    assert_eq!(r.total, 2, "Ada's and Bo's Reliability on alex 0..10, each with their own Cost");
    // And Reliability that is NOT also coded Cost: priya's.
    let not = json!({ "type": "group", "match": "all", "items": [code_clause(&[w.reliability]), { "type": "also_coded", "code_ids": [w.cost], "negate": true }] });
    let r = w.p.run_query(&query(not, "assignments")).unwrap();
    assert_eq!(r.items.iter().map(|i| i.source_name.as_str()).collect::<Vec<_>>(), ["priya"]);
    // Within some distance rather than overlapping: Cost at 20..30 is 5 characters from Reliability's 0..10 end? No: 10 to 20 is 10.
    let near = json!({ "type": "group", "match": "all", "items": [code_clause(&[w.comfort]), { "type": "also_coded", "code_ids": [w.cost], "within_chars": 10 }] });
    let r = w.p.run_query(&query(near, "assignments")).unwrap();
    assert_eq!(r.total, 2, "alex's Comfort 40..50 is 10 from Cost 20..30; jordan's overlaps Cost: {:?}", r.items.iter().map(|i| (&i.source_name, i.start)).collect::<Vec<_>>());
}

#[test]
fn text_documents_cases_variables_coders_and_ratings_can_all_be_asked_about() {
    let mut w = world();
    let text = json!({ "type": "text", "contains": "DELAYS" });
    assert_eq!(w.p.run_query(&query(text, "excerpts")).unwrap().total, 1, "capitals do not matter; only the one stretch holds the word");
    let visitors = json!({ "type": "case_variable", "name": "Group", "value": "Visitors" });
    let r = w.p.run_query(&query(visitors, "assignments")).unwrap();
    assert_eq!(r.items.iter().map(|i| i.source_name.as_str()).collect::<Vec<_>>(), ["jordan", "jordan"]);
    let by_bo = json!({ "type": "coder", "ids": [w.bo] });
    assert_eq!(w.p.run_query(&query(by_bo, "assignments")).unwrap().total, 4);
    let not_alex = json!({ "type": "source", "ids": [w.alex], "negate": true });
    assert_eq!(w.p.run_query(&query(not_alex, "assignments")).unwrap().total, 4, "priya's two and jordan's two");
    let in_case = json!({ "type": "case", "ids": [w.p.list_cases().unwrap().iter().find(|c| c.name == "Priya").unwrap().id] });
    assert_eq!(w.p.run_query(&query(in_case, "assignments")).unwrap().total, 2);

    // A rating can be asked for once a code is rated; an unrated coding never matches.
    w.p.update_code(w.reliability, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
    let first = w.p.list_codings(w.alex).unwrap().into_iter().find(|k| k.code_id == w.reliability && k.coder_id == w.ada).unwrap();
    w.p.set_coding_rating(first.id, Some(4)).unwrap();
    let rated = json!({ "type": "rating", "min": 3 });
    assert_eq!(w.p.run_query(&query(rated, "assignments")).unwrap().items[0].codings[0].rating, Some(4));
    let low = json!({ "type": "rating", "max": 2 });
    assert_eq!(w.p.run_query(&query(low, "assignments")).unwrap().total, 0);
}

#[test]
fn groups_nest_and_a_whole_group_can_be_turned_round() {
    let w = world();
    // (Reliability OR Comfort) AND coded by Ada.
    let rule = json!({ "type": "group", "match": "all", "items": [
        { "type": "group", "match": "any", "items": [code_clause(&[w.reliability]), code_clause(&[w.comfort])] },
        { "type": "coder", "ids": [w.ada] },
    ]});
    assert_eq!(w.p.run_query(&query(rule, "assignments")).unwrap().total, 4, "Ada's Reliability twice and her Comfort twice");
    let anything_but = json!({ "type": "group", "match": "any", "negate": true, "items": [code_clause(&[w.barriers]), code_clause(&[w.comfort])] });
    assert_eq!(w.p.run_query(&query(anything_but, "assignments")).unwrap().total, 0, "Barriers (with its sub-codes) and Comfort are every coding");
}

#[test]
fn a_query_that_cannot_run_says_what_to_fix_and_results_can_be_cut() {
    let w = world();
    let empty = w.p.run_query(&query(json!({ "type": "group", "match": "any", "items": [] }), "excerpts")).unwrap_err();
    assert!(empty.to_string().contains("no conditions yet"), "{empty}");
    let no_code = w.p.run_query(&query(json!({ "type": "group", "match": "all", "items": [code_clause(&[])] }), "excerpts")).unwrap_err();
    assert!(no_code.to_string().contains("looks for a code has none chosen"), "{no_code}");
    let no_text = w.p.run_query(&query(json!({ "type": "group", "match": "all", "items": [{ "type": "text", "contains": "  " }] }), "excerpts")).unwrap_err();
    assert!(no_text.to_string().contains("no text to look for"), "{no_text}");

    let mut spec = query(json!({ "type": "group", "match": "any", "items": [code_clause(&[w.reliability]), code_clause(&[w.cost])] }), "excerpts");
    spec.limit = 2;
    let r = w.p.run_query(&spec).unwrap();
    assert_eq!((r.items.len(), r.total, r.truncated), (2, 5, true), "the total is always counted, and the list says it was cut");
    let _ = (Combine::Any, ResultUnit::Excerpts, QueryNode::Text { contains: String::new(), negate: false });
}

#[test]
fn case_variables_are_listed_with_their_values_and_how_many_cases_have_each() {
    let w = world();
    let v = w.p.case_variables().unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!((v[0].name.as_str(), v[0].values.clone()), ("Group", vec![("Residents".to_string(), 2), ("Visitors".to_string(), 1)]));
}
