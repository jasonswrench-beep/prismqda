//! A coding can carry a rating from 1 to 5. No rating is not a number: it is its own state.

use prismqda_engine::{CodeEdit, Error, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SegmentFilter, SourceType};
use serde_json::json;
use tempfile::TempDir;

struct W {
    p: Project,
    source: i64,
    coder: i64,
    code: i64,
}

fn world(dir: &TempDir) -> W {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let source = p.add_source(&NewSource::new("S", SourceType::Document, "abcdefghijklmnopqrstuvwxyz")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let code = p.add_code(&NewCode::new("C")).unwrap();
    rated(&mut p, code);
    W { p, source, coder, code }
}

/// Ratings only live on rated codes, so the codes these tests rate are rated first.
fn rated(p: &mut Project, code: i64) {
    p.update_code(code, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
}

fn code_one(w: &mut W, start: i64, end: i64) -> i64 {
    w.p.apply_coding(&NewCoding::new(w.source, start, end, w.code, w.coder)).unwrap()
}

fn rating_of(w: &W, id: i64) -> Option<i64> {
    w.p.list_codings(w.source).unwrap().into_iter().find(|c| c.id == id).unwrap().rating
}

#[test]
fn a_new_coding_is_not_rated() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    assert_eq!(rating_of(&w, id), None);
    let json = serde_json::to_value(&w.p.list_codings(w.source).unwrap()[0]).unwrap();
    assert!(json.as_object().unwrap().contains_key("rating"), "an unrated coding still carries the key");
    assert!(json["rating"].is_null());
}

#[test]
fn every_rating_from_one_to_five_is_stored_and_listed() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    for r in 1..=5 {
        w.p.set_coding_rating(id, Some(r)).unwrap();
        assert_eq!(rating_of(&w, id), Some(r));
    }
    assert_eq!(serde_json::to_value(&w.p.list_codings(w.source).unwrap()[0]).unwrap()["rating"], 5);
}

#[test]
fn ratings_outside_one_to_five_are_refused_and_change_nothing() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, Some(3)).unwrap();
    let events_before = w.p.events(1000).unwrap().len();
    for bad in [0, 6, -1, 100, i64::MAX, i64::MIN] {
        let e = w.p.set_coding_rating(id, Some(bad)).err().unwrap();
        assert!(matches!(e, Error::Invalid(_)), "{bad}: {e:?}");
        assert!(e.to_string().contains("1 to 5"), "the message says what is allowed: {e}");
    }
    assert_eq!(rating_of(&w, id), Some(3));
    assert_eq!(w.p.events(1000).unwrap().len(), events_before, "a refused rating leaves no event");
}

#[test]
fn none_clears_the_rating_and_is_not_the_same_as_any_number() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, Some(1)).unwrap();
    w.p.set_coding_rating(id, None).unwrap();
    assert_eq!(rating_of(&w, id), None);
    assert_ne!(rating_of(&w, id), Some(1));
    let json = serde_json::to_value(&w.p.list_codings(w.source).unwrap()[0]).unwrap();
    assert!(json["rating"].is_null());
}

#[test]
fn an_unknown_coding_is_not_found() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    assert!(matches!(w.p.set_coding_rating(99, Some(3)).err().unwrap(), Error::NotFound { what: "coding", id: 99 }));
    assert!(matches!(w.p.set_coding_rating(99, None).err().unwrap(), Error::NotFound { .. }));
}

#[test]
fn a_rating_is_logged_with_what_it_was_and_what_it_became() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, Some(4)).unwrap();
    w.p.set_coding_rating(id, Some(2)).unwrap();
    w.p.set_coding_rating(id, None).unwrap();

    let events: Vec<_> = w.p.events(1000).unwrap().into_iter().filter(|e| e.action == "coding.rating").collect();
    assert_eq!(events.len(), 3);
    for e in &events {
        assert_eq!((e.target_type.as_deref(), e.target_id), (Some("coding"), Some(id)));
        assert_eq!(e.actor, "tester");
    }
    assert_eq!(events[0].before, Some(json!({ "rating": null })));
    assert_eq!(events[0].after, Some(json!({ "rating": 4 })));
    assert_eq!(events[1].before, Some(json!({ "rating": 4 })));
    assert_eq!(events[1].after, Some(json!({ "rating": 2 })));
    assert_eq!(events[2].before, Some(json!({ "rating": 2 })));
    assert_eq!(events[2].after, Some(json!({ "rating": null })));
}

#[test]
fn setting_the_rating_it_already_has_is_not_a_change_and_is_not_logged() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, None).unwrap();
    w.p.set_coding_rating(id, Some(5)).unwrap();
    w.p.set_coding_rating(id, Some(5)).unwrap();
    let n = w.p.events(1000).unwrap().iter().filter(|e| e.action == "coding.rating").count();
    assert_eq!(n, 1, "only the one real change is in the trail, so 'last modified' stays honest");
}

#[test]
fn undo_restores_the_earlier_rating_including_none() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, Some(3)).unwrap();
    w.p.set_coding_rating(id, Some(5)).unwrap();
    w.p.set_coding_rating(id, None).unwrap();
    assert_eq!(w.p.next_undo().unwrap().as_deref(), Some("Restored the coding's earlier rating"));

    let out = w.p.undo().unwrap().unwrap();
    assert_eq!((out.action.as_str(), out.what.as_str()), ("coding.rating", "Restored the coding's earlier rating"));
    assert_eq!(rating_of(&w, id), Some(5), "undoing a clear brings the number back");
    w.p.undo().unwrap().unwrap();
    assert_eq!(rating_of(&w, id), Some(3));
    w.p.undo().unwrap().unwrap();
    assert_eq!(rating_of(&w, id), None, "undoing the first rating goes back to not rated");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "coding.add", "and the next undo is the coding itself");
}

#[test]
fn a_removed_coding_comes_back_with_its_rating() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, Some(4)).unwrap();
    w.p.remove_coding(id).unwrap();
    let removed = w.p.events(1000).unwrap().into_iter().find(|e| e.action == "coding.remove").unwrap();
    assert_eq!(removed.before.as_ref().unwrap()["rating"], 4, "the removal is logged in full, rating included");

    assert_eq!(w.p.undo().unwrap().unwrap().action, "coding.remove");
    assert_eq!(rating_of(&w, id), Some(4));

    // An unrated coding comes back unrated.
    let other = code_one(&mut w, 6, 9);
    w.p.remove_coding(other).unwrap();
    w.p.undo().unwrap().unwrap();
    assert_eq!(rating_of(&w, other), None);
}

#[test]
fn the_rating_stays_when_the_code_changes_or_codes_merge() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let id = code_one(&mut w, 0, 5);
    w.p.set_coding_rating(id, Some(2)).unwrap();
    let other = w.p.add_code(&NewCode::new("D")).unwrap();
    rated(&mut w.p, other);
    w.p.change_coding_code(id, other).unwrap();
    assert_eq!(rating_of(&w, id), Some(2));
    w.p.merge_codes(other, w.code).unwrap();
    assert_eq!(rating_of(&w, id), Some(2));
}

#[test]
fn coded_segments_carry_the_rating_for_exports_and_reports() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let rated = code_one(&mut w, 0, 5);
    let unrated = code_one(&mut w, 6, 9);
    w.p.set_coding_rating(rated, Some(5)).unwrap();
    let segments = w.p.coded_segments(&SegmentFilter::default()).unwrap();
    let by_id = |id: i64| segments.iter().find(|s| s.coding_id == id).unwrap();
    assert_eq!(by_id(rated).rating, Some(5));
    assert_eq!(by_id(unrated).rating, None);
    assert!(serde_json::to_value(by_id(unrated)).unwrap()["rating"].is_null());
}

#[test]
fn removing_text_that_deletes_a_coding_logs_its_rating_too() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let source = w.p.add_source(&NewSource::new("Words", SourceType::Document, "keep this and drop that part, then keep going")).unwrap();
    let id = w.p.apply_coding(&NewCoding::new(source, 14, 23, w.code, w.coder)).unwrap(); // "drop that"
    w.p.set_coding_rating(id, Some(3)).unwrap();
    // Cut the coded words out of the text entirely.
    let out = w.p.replace_source_text(source, "keep this and part, then keep going").unwrap();
    assert_eq!(out.codings_removed, 1);
    assert!(w.p.list_codings(source).unwrap().is_empty());
    let edit = w.p.events(1000).unwrap().into_iter().find(|e| e.action == "source.edit").unwrap();
    let gone = &edit.before.as_ref().unwrap()["removed_codings"];
    assert_eq!(gone[0]["id"], id);
    assert_eq!(gone[0]["rating"], 3);
}
