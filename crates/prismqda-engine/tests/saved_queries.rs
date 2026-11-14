//! Saved and frozen queries, and the date and range clauses of a query, against a small project whose every number was
//! worked out by hand.
//!
//! Documents and cases: alex, priya, jordan (one case each). Case variables: Group (Residents, Residents, Visitors),
//! "Interview date" (2024-02-10, 2024/03/05, "March 3rd": the last is not a date and matches nothing), and Age (30, 45, "n/a").
//! Codings in the order they were made (their numbers are 1 to 10), with the day each was made on:
//!   1 alex Reliability Ada 2024-03-01     2 alex Reliability Bo 2024-03-01    3 alex Cost Ada 2024-03-10
//!   4 alex Cost Ada 2024-03-10            5 alex Comfort Ada 2024-04-02       6 alex Cost Bo 2024-04-02
//!   7 priya Reliability Ada 2024-04-20    8 priya Comfort Ada 2024-04-20      9 jordan Cost Bo 2024-05-05
//!  10 jordan Comfort Bo 2024-05-05

use std::path::PathBuf;

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, Error, ImportBatch, ImportedCase, ImportedSource, Level, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, QuerySpec, SourceType,
    UnitKind,
};
use serde_json::{json, Value};
use tempfile::TempDir;

#[allow(dead_code)]
struct W {
    p: Project,
    path: PathBuf,
    _dir: TempDir,
    alex: i64,
    reliability: i64,
    cost: i64,
    comfort: i64,
    ada: i64,
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
    let path = dir.path().join("p.pqda");
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(&path, &NewProject::new("P"), &o).unwrap();
    let values = [("Residents", "2024-02-10", "30"), ("Residents", "2024/03/05", "45"), ("Visitors", "March 3rd", "n/a")];
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
                attrs: vec![
                    ("Group".to_string(), values[i].0.to_string()),
                    ("Interview date".to_string(), values[i].1.to_string()),
                    ("Age".to_string(), values[i].2.to_string()),
                ],
                members: vec![CaseMember { source: i, unit: None }],
            })
            .collect(),
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![
            AttributeSpec::new("Group", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Interview date", AppliesTo::Case, Level::Date),
            AttributeSpec::new("Age", AppliesTo::Case, Level::Ratio),
        ],
    };
    let out = p.apply_import(&batch).unwrap();
    let (alex, priya, jordan) = (out.source_ids[0], out.source_ids[1], out.source_ids[2]);
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();
    let reliability = p.add_code(&NewCode::new("Reliability")).unwrap();
    let cost = p.add_code(&NewCode::new("Cost")).unwrap();
    let comfort = p.add_code(&NewCode::new("Comfort")).unwrap();
    let mut code = |source: i64, start: i64, end: i64, code: i64, coder: i64| p.apply_coding(&NewCoding::new(source, start, end, code, coder)).unwrap();
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

    // Give each coding the day it is said to have been made on (a second connection, as a person's file tools would).
    let days = ["2024-03-01", "2024-03-01", "2024-03-10", "2024-03-10", "2024-04-02", "2024-04-02", "2024-04-20", "2024-04-20", "2024-05-05", "2024-05-05"];
    let raw = rusqlite::Connection::open(&path).unwrap();
    for (i, d) in days.iter().enumerate() {
        raw.execute("UPDATE coding SET created_at = ?1 WHERE id = ?2", rusqlite::params![format!("{d}T10:00:00.000Z"), i as i64 + 1]).unwrap();
    }
    W { p, path, _dir: dir, alex, reliability, cost, comfort, ada }
}

fn spec_of(rule: Value) -> Value {
    json!({ "rule": rule, "result_unit": "assignments" })
}

fn run(w: &W, rule: Value) -> Result<prismqda_engine::QueryResult, Error> {
    let spec: QuerySpec = serde_json::from_value(spec_of(rule)).unwrap();
    w.p.run_query(&spec)
}

fn ids(w: &W, rule: Value) -> Vec<i64> {
    let mut v: Vec<i64> = run(w, rule).unwrap().items.iter().flat_map(|i| i.codings.iter().map(|c| c.coding_id)).collect();
    v.sort_unstable();
    v
}

fn any(items: Vec<Value>) -> Value {
    json!({ "type": "group", "match": "any", "items": items })
}

// ---- dates ----

#[test]
fn a_query_can_ask_for_the_days_codings_were_made_on() {
    let w = world();
    let between = |from: Option<&str>, to: Option<&str>, negate: bool| any(vec![json!({ "type": "coded_between", "from": from, "to": to, "negate": negate })]);
    assert_eq!(ids(&w, between(Some("2024-03-10"), Some("2024-04-02"), false)), [3, 4, 5, 6], "both ends count");
    assert_eq!(ids(&w, between(Some("2024-04-20"), None, false)), [7, 8, 9, 10], "from a day on");
    assert_eq!(ids(&w, between(None, Some("2024-03-01"), false)), [1, 2], "up to a day");
    assert_eq!(ids(&w, between(Some("2024-03-10"), Some("2024-04-02"), true)), [1, 2, 7, 8, 9, 10], "turned round");
    assert_eq!(ids(&w, between(Some("2024/3/10"), Some("2024-3-10"), false)), [3, 4], "the slashes and a missing zero are read the same way");
    for (from, to, why) in [(None, None, "no day at all"), (Some("3/10/2024"), None, "month first is not read"), (Some("2024-04-02"), Some("2024-03-10"), "the first day is after the last")] {
        let e = run(&w, between(from, to, false)).unwrap_err();
        assert!(matches!(e, Error::Invalid(_)), "{why}: {e}");
    }
    // Combined with another condition: Cost, coded in March.
    let both = json!({ "type": "group", "match": "all", "items": [
        { "type": "code", "code_ids": [w.cost] },
        { "type": "coded_between", "from": "2024-03-01", "to": "2024-03-31" },
    ] });
    assert_eq!(ids(&w, both), [3, 4]);
}

#[test]
fn a_query_can_ask_for_dates_and_numbers_between_limits_in_a_case_variable() {
    let w = world();
    let var = |name: &str, from: Option<&str>, to: Option<&str>, negate: bool| any(vec![json!({ "type": "case_variable_between", "name": name, "from": from, "to": to, "negate": negate })]);
    // Alex (2024-02-10) and priya (2024/03/05) are in range; jordan's "March 3rd" is not a date and matches nothing.
    assert_eq!(ids(&w, var("Interview date", Some("2024-02-01"), Some("2024-03-31"), false)), [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(ids(&w, var("Interview date", Some("2024-03-01"), None, false)), [7, 8], "from a day on");
    assert_eq!(ids(&w, var("Interview date", Some("2024-02-01"), Some("2024-03-31"), true)), [9, 10], "turned round, a value that is not a date is on the not side");
    // Numbers: age 30 and 45 are numbers, "n/a" is not.
    assert_eq!(ids(&w, var("Age", Some("40"), None, false)), [7, 8]);
    assert_eq!(ids(&w, var("Age", Some("30"), Some("50"), false)), [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(ids(&w, var("Age", None, Some("29"), false)), Vec::<i64>::new());
    // Limits that cannot be read, or mix dates and numbers, are refused.
    for (from, to) in [(Some("abc"), None), (Some("2024-01-01"), Some("50")), (None, None), (Some("2024-05-01"), Some("2024-04-01")), (Some("50"), Some("40"))] {
        let e = run(&w, var("Age", from, to, false)).unwrap_err();
        assert!(matches!(e, Error::Invalid(_)), "{from:?} {to:?}: {e}");
    }
}

// ---- saved queries ----

fn cost_rule(w: &W) -> Value {
    any(vec![json!({ "type": "code", "code_ids": [w.cost] })])
}

fn save(w: &mut W, name: &str) -> i64 {
    let spec = spec_of(cost_rule(w));
    w.p.save_query(name, "", &json!({ "draft": "as the interface keeps it" }), &spec).unwrap()
}

#[test]
fn a_query_is_saved_under_a_name_and_found_again() {
    let mut w = world();
    assert!(w.p.list_saved_queries().unwrap().is_empty());
    let id = save(&mut w, "Cost talk");
    let zed = save(&mut w, "A first look");
    let list = w.p.list_saved_queries().unwrap();
    assert_eq!(list.iter().map(|q| q.name.as_str()).collect::<Vec<_>>(), ["A first look", "Cost talk"], "in the order of their names");
    assert!(list.iter().all(|q| q.frozen_at.is_none() && q.frozen_count.is_none()));
    let got = w.p.get_saved_query(id).unwrap();
    assert_eq!((got.name.as_str(), got.draft["draft"].as_str()), ("Cost talk", Some("as the interface keeps it")));
    assert_eq!(got.spec["result_unit"], "assignments");
    assert_ne!(id, zed);

    // A name is used once, whatever the capitals; a nameless or very long name is refused; a query that cannot be run is not kept.
    let spec = spec_of(cost_rule(&w));
    assert!(w.p.save_query("cost TALK", "", &json!({}), &spec).unwrap_err().to_string().contains("already a saved query"));
    assert!(w.p.save_query("   ", "", &json!({}), &spec).is_err());
    assert!(w.p.save_query(&"x".repeat(81), "", &json!({}), &spec).is_err());
    assert!(w.p.save_query("Empty", "", &json!({}), &spec_of(any(vec![]))).unwrap_err().to_string().contains("no conditions"));
    assert!(w.p.save_query("Broken", "", &json!({}), &json!({ "rule": 7 })).unwrap_err().to_string().contains("cannot be run"));
    assert_eq!(w.p.list_saved_queries().unwrap().len(), 2, "nothing was kept by the refusals");

    // Open it: it is run again.
    let (_, result) = w.p.saved_query_result(id).unwrap();
    assert_eq!(result.assignments, 4, "four codings of Cost: 3, 4, 6, 9");

    // Rename it, change its note, and change what it asks.
    w.p.update_saved_query(id, Some("Money talk"), Some("  about prices  "), None).unwrap();
    assert!(w.p.update_saved_query(id, Some("a first look"), None, None).unwrap_err().to_string().contains("already a saved query"));
    let both = (&json!({ "draft": "new" }), &spec_of(any(vec![json!({ "type": "code", "code_ids": [w.cost, w.comfort] })])));
    let changed = w.p.update_saved_query(id, None, None, Some(both)).unwrap();
    assert_eq!((changed.name.as_str(), changed.note.as_str(), changed.draft["draft"].as_str()), ("Money talk", "about prices", Some("new")));
    assert_eq!(w.p.saved_query_result(id).unwrap().1.assignments, 7, "now Cost or Comfort: 3, 4, 5, 6, 8, 9, 10");
}

#[test]
fn a_frozen_query_keeps_its_answer_while_the_coding_changes_and_the_comparison_says_what_changed() {
    let mut w = world();
    let id = save(&mut w, "Cost talk");
    let frozen = w.p.freeze_saved_query(id).unwrap();
    assert_eq!((frozen.codings, frozen.results), (4, 4));
    let row = &w.p.list_saved_queries().unwrap()[0];
    assert_eq!((row.frozen_count, row.frozen_at.is_some()), (Some(4), true));
    assert!(w.p.update_saved_query(id, None, None, Some((&json!({}), &spec_of(cost_rule(&w)))))
        .unwrap_err()
        .to_string()
        .contains("frozen"), "a frozen query's question cannot change");
    w.p.update_saved_query(id, Some("Frozen cost talk"), Some("as of today"), None).unwrap();

    // The coding grows (one more Cost) and shrinks (one of the four is removed).
    let new = w.p.apply_coding(&NewCoding::new(w.alex, 30, 40, w.cost, w.ada)).unwrap();
    w.p.remove_coding(3).unwrap();
    let live = w.p.run_query(&serde_json::from_value::<QuerySpec>(spec_of(cost_rule(&w))).unwrap()).unwrap();
    assert_eq!(live.assignments, 4, "now: 4, 6, 9 and the new one");

    let (_, shown) = w.p.saved_query_result(id).unwrap();
    assert_eq!(shown.assignments, 3, "the frozen answer is the four it found, of which one is gone");
    assert!(!shown.items.iter().any(|i| i.codings.iter().any(|c| c.coding_id == new)), "the new coding is not in it");

    let cmp = w.p.compare_frozen_query(id).unwrap();
    assert_eq!((cmp.frozen, cmp.now, cmp.still_found, cmp.added, cmp.no_longer_found, cmp.removed), (4, 4, 3, 1, 1, 1));
    assert_eq!(cmp.added_ids, [new]);

    // Taking the picture again keeps the new state; letting it go follows the coding.
    assert_eq!(w.p.freeze_saved_query(id).unwrap().codings, 4);
    assert_eq!(w.p.compare_frozen_query(id).unwrap().added, 0);
    w.p.unfreeze_saved_query(id).unwrap();
    assert!(w.p.get_saved_query(id).unwrap().frozen_at.is_none());
    assert!(w.p.compare_frozen_query(id).unwrap_err().to_string().contains("not frozen"));
    w.p.unfreeze_saved_query(id).unwrap();
    w.p.update_saved_query(id, None, None, Some((&json!({}), &spec_of(cost_rule(&w))))).unwrap();
}

#[test]
fn saving_changing_deleting_and_freezing_can_all_be_undone() {
    let mut w = world();
    let id = save(&mut w, "Cost talk");
    assert_eq!(w.p.next_undo().unwrap().as_deref(), Some("Removed the saved query"));

    w.p.freeze_saved_query(id).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "query.freeze");
    assert!(w.p.get_saved_query(id).unwrap().frozen_at.is_none(), "taking back the freeze");

    w.p.update_saved_query(id, Some("Renamed"), Some("a note"), None).unwrap();
    w.p.undo().unwrap().unwrap();
    let back = w.p.get_saved_query(id).unwrap();
    assert_eq!((back.name.as_str(), back.note.as_str()), ("Cost talk", ""));

    w.p.freeze_saved_query(id).unwrap();
    w.p.unfreeze_saved_query(id).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "query.unfreeze");
    assert_eq!(w.p.get_saved_query(id).unwrap().frozen_count, Some(4), "freezing again as it was");
    w.p.undo().unwrap().unwrap();

    w.p.delete_saved_query(id).unwrap();
    assert!(w.p.get_saved_query(id).is_err());
    w.p.undo().unwrap().unwrap();
    assert_eq!(w.p.get_saved_query(id).unwrap().name, "Cost talk", "the query comes back with its own number");
    // The oldest step, saving, removes it.
    w.p.undo().unwrap().unwrap();
    assert!(w.p.list_saved_queries().unwrap().is_empty());
}

#[test]
fn a_saved_query_survives_closing_and_opening_the_project() {
    let mut w = world();
    let id = save(&mut w, "Cost talk");
    w.p.freeze_saved_query(id).unwrap();
    let path = w.path.clone();
    let W { p, _dir, .. } = w;
    drop(p);
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let reopened = Project::open(&path, &o).unwrap();
    let row = &reopened.list_saved_queries().unwrap()[0];
    assert_eq!((row.name.as_str(), row.frozen_count), ("Cost talk", Some(4)));
}
