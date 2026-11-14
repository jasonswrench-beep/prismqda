//! Code properties: mutually exclusive groups, rated codes with a defined scale, and counterexamples.

use prismqda_engine::{CodeEdit, Error, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SourceType};
use tempfile::TempDir;

struct W {
    p: Project,
    source: i64,
    ada: i64,
    bo: i64,
    positive: i64,
    negative: i64,
    neutral: i64,
}

/// A source of 26 letters, two coders, and three codes that are all in the group "Sentiment".
fn world(dir: &TempDir) -> W {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let source = p.add_source(&NewSource::new("S", SourceType::Document, "abcdefghijklmnopqrstuvwxyz")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();
    let mut code = |name: &str| {
        let mut c = NewCode::new(name);
        c.exclusive_group = "Sentiment".to_string();
        p.add_code(&c).unwrap()
    };
    let positive = code("Positive");
    let negative = code("Negative");
    let neutral = code("Neutral");
    W { p, source, ada, bo, positive, negative, neutral }
}

fn apply(w: &mut W, coder: i64, start: i64, end: i64, code: i64) -> Result<i64, Error> {
    w.p.apply_coding(&NewCoding::new(w.source, start, end, code, coder))
}

fn code_of(w: &W, coding: i64) -> Option<i64> {
    w.p.list_codings(w.source).unwrap().into_iter().find(|c| c.id == coding).map(|c| c.code_id)
}

// ---- mutually exclusive groups ----------------------------------------------------------------------

#[test]
fn a_second_code_of_a_group_on_overlapping_words_is_refused_and_says_why() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative) = (w.ada, w.positive, w.negative);
    apply(&mut w, ada, 0, 10, positive).unwrap();
    let err = apply(&mut w, ada, 5, 15, negative).unwrap_err();
    assert!(matches!(err, Error::ExclusiveGroup(_)), "{err:?}");
    assert_eq!(
        err.to_string(),
        "'Negative' is in the group 'Sentiment', which allows one code at a time, and these words already have 'Positive'."
    );
    assert_eq!(w.p.list_codings(w.source).unwrap().len(), 1, "nothing was stored");
}

#[test]
fn words_that_only_touch_do_not_overlap_and_another_coder_is_free() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, bo, positive, negative) = (w.ada, w.bo, w.positive, w.negative);
    apply(&mut w, ada, 0, 10, positive).unwrap();
    apply(&mut w, ada, 10, 15, negative).expect("the next words, not the same ones");
    apply(&mut w, bo, 0, 10, negative).expect("each coder codes on their own");
}

#[test]
fn a_code_outside_any_group_is_never_in_the_way() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let free = w.p.add_code(&NewCode::new("Free")).unwrap();
    let (ada, positive) = (w.ada, w.positive);
    apply(&mut w, ada, 0, 10, positive).unwrap();
    apply(&mut w, ada, 0, 10, free).expect("a code with no group");
    let same_again = apply(&mut w, ada, 2, 4, positive);
    assert!(same_again.is_ok(), "the same code twice is not a group matter: {same_again:?}");
}

#[test]
fn replacing_takes_the_other_codings_away_and_logs_each_one() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative, neutral) = (w.ada, w.positive, w.negative, w.neutral);
    let a = apply(&mut w, ada, 0, 4, positive).unwrap();
    let b = apply(&mut w, ada, 6, 10, neutral).unwrap();
    // One coding that covers both: it conflicts with the two.
    let err = apply(&mut w, ada, 0, 12, negative).unwrap_err();
    assert!(err.to_string().ends_with("already have 'Positive' and 'Neutral'."), "{err}");

    let mut coding = NewCoding::new(w.source, 0, 12, negative, ada);
    coding.replace_exclusive = true;
    let id = w.p.apply_coding(&coding).unwrap();
    let left: Vec<i64> = w.p.list_codings(w.source).unwrap().iter().map(|c| c.id).collect();
    assert_eq!(left, vec![id], "the two old codings are gone");
    assert_eq!(code_of(&w, id), Some(negative));

    let events = w.p.events(100).unwrap();
    let removals: Vec<_> = events.iter().filter(|e| e.action == "coding.remove").collect();
    assert_eq!(removals.len(), 2, "each removal is its own event");
    assert!(removals.iter().all(|e| e.after.as_ref().unwrap()["replaced_by_code"] == negative));
    assert_eq!(removals.iter().map(|e| e.target_id.unwrap()).collect::<Vec<_>>(), vec![a, b]);
}

#[test]
fn a_replacement_is_undone_one_step_at_a_time_and_the_old_coding_comes_back() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative) = (w.ada, w.positive, w.negative);
    let old = apply(&mut w, ada, 0, 10, positive).unwrap();
    let mut coding = NewCoding::new(w.source, 0, 10, negative, ada);
    coding.replace_exclusive = true;
    w.p.apply_coding(&coding).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "coding.add");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "coding.remove");
    // (The new coding may have taken the old one's number, which is free again once that is removed.)
    let rows = w.p.list_codings(w.source).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].id, rows[0].code_id), (old, positive), "the old coding is back with its own code");
}

#[test]
fn a_group_name_joins_the_group_that_has_it_whatever_the_capitals() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let mut c = NewCode::new("Mixed");
    c.exclusive_group = "  sentiment ".to_string();
    let mixed = w.p.add_code(&c).unwrap();
    assert_eq!(w.p.get_code(mixed).unwrap().exclusive_group, "Sentiment", "the group's own spelling");
    let (ada, positive) = (w.ada, w.positive);
    apply(&mut w, ada, 0, 5, positive).unwrap();
    assert!(matches!(apply(&mut w, ada, 0, 5, mixed), Err(Error::ExclusiveGroup(_))));
}

#[test]
fn changing_a_codings_code_obeys_the_group_too() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative, neutral) = (w.ada, w.positive, w.negative, w.neutral);
    let a = apply(&mut w, ada, 0, 10, positive).unwrap();
    let b = apply(&mut w, ada, 12, 20, negative).unwrap();
    // Moving the second onto words the first covers is not possible while they are in one group.
    let err = w.p.change_coding_code(b, positive);
    assert!(err.is_ok(), "there is no overlap between 12..20 and 0..10");
    let c = apply(&mut w, ada, 0, 10, neutral).unwrap_err();
    assert!(matches!(c, Error::ExclusiveGroup(_)));
    // The first coding can change to another code of the group, since it is the only one on its words.
    w.p.change_coding_code(a, neutral).unwrap();
    assert_eq!(code_of(&w, a), Some(neutral));
}

#[test]
fn taking_a_code_out_of_its_group_or_naming_a_group_later_is_a_plain_edit() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative) = (w.ada, w.positive, w.negative);
    apply(&mut w, ada, 0, 10, positive).unwrap();
    w.p.update_code(negative, &CodeEdit { exclusive_group: Some(String::new()), ..CodeEdit::default() }).unwrap();
    assert_eq!(w.p.get_code(negative).unwrap().exclusive_group, "");
    apply(&mut w, ada, 0, 10, negative).expect("no longer in the group");
    // Putting it back does not change what is already coded; the old pair is reported instead.
    w.p.update_code(negative, &CodeEdit { exclusive_group: Some("Sentiment".to_string()), ..CodeEdit::default() }).unwrap();
    assert_eq!(w.p.exclusive_group_violations("sentiment").unwrap().len(), 1);
    assert!(w.p.exclusive_group_violations("").unwrap().is_empty());
    assert!(w.p.exclusive_group_violations("Other").unwrap().is_empty());
}

#[test]
fn a_retired_code_does_not_hold_the_group() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative) = (w.ada, w.positive, w.negative);
    apply(&mut w, ada, 0, 10, positive).unwrap();
    w.p.retire_code(positive).unwrap();
    apply(&mut w, ada, 0, 10, negative).expect("the retired code's codings do not block");
}

#[test]
fn group_names_and_rating_words_have_a_length_limit() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let long = "x".repeat(81);
    let e = w.p.update_code(w.positive, &CodeEdit { exclusive_group: Some(long), ..CodeEdit::default() }).unwrap_err();
    assert!(e.to_string().contains("at most 80"), "{e}");
    let e = w.p.update_code(w.positive, &CodeEdit { rating_label: Some("y".repeat(121)), ..CodeEdit::default() }).unwrap_err();
    assert!(e.to_string().contains("at most 120"), "{e}");
}

// ---- rated codes ---------------------------------------------------------------------------------------

#[test]
fn only_a_rated_code_can_be_rated_and_its_scale_is_described() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive) = (w.ada, w.positive);
    let id = apply(&mut w, ada, 0, 10, positive).unwrap();
    let e = w.p.set_coding_rating(id, Some(3)).unwrap_err();
    assert!(e.to_string().contains("'Positive' is not a rated code"), "{e}");

    let edit = CodeEdit {
        rated: Some(true),
        rating_label: Some("  Strength of feeling ".to_string()),
        rating_low: Some("A hint".to_string()),
        rating_high: Some("Overwhelming".to_string()),
        ..CodeEdit::default()
    };
    let code = w.p.update_code(positive, &edit).unwrap();
    assert!(code.rated);
    assert_eq!((code.rating_label.as_str(), code.rating_low.as_str(), code.rating_high.as_str()), ("Strength of feeling", "A hint", "Overwhelming"));
    assert_eq!(code.version, 1, "properties are not part of what the code means, so its version stays");
    w.p.set_coding_rating(id, Some(3)).unwrap();
    assert_eq!(w.p.list_codings(w.source).unwrap()[0].rating, Some(3));
}

#[test]
fn rating_cannot_be_switched_off_while_ratings_exist_unless_they_are_cleared_with_it() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive) = (w.ada, w.positive);
    w.p.update_code(positive, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
    let a = apply(&mut w, ada, 0, 4, positive).unwrap();
    let b = apply(&mut w, ada, 6, 10, positive).unwrap();
    apply(&mut w, ada, 12, 16, positive).unwrap();
    w.p.set_coding_rating(a, Some(2)).unwrap();
    w.p.set_coding_rating(b, Some(5)).unwrap();

    let e = w.p.update_code(positive, &CodeEdit { rated: Some(false), ..CodeEdit::default() }).unwrap_err();
    assert!(e.to_string().starts_with("2 codings of this code carry a rating."), "{e}");
    assert!(w.p.get_code(positive).unwrap().rated, "nothing changed");
    assert_eq!(w.p.list_codings(w.source).unwrap().iter().filter(|c| c.rating.is_some()).count(), 2);

    w.p.update_code(positive, &CodeEdit { rated: Some(false), clear_ratings: true, ..CodeEdit::default() }).unwrap();
    assert!(!w.p.get_code(positive).unwrap().rated);
    assert!(w.p.list_codings(w.source).unwrap().iter().all(|c| c.rating.is_none()));
    let cleared: Vec<_> = w.p.events(100).unwrap().into_iter().filter(|e| e.action == "coding.rating" && e.after.as_ref().unwrap()["rating"].is_null()).collect();
    assert_eq!(cleared.len(), 2, "each cleared rating is logged with what it was");
    assert_eq!(cleared[0].before.as_ref().unwrap()["rating"], 2);
}

#[test]
fn undoing_the_switch_brings_back_the_property_and_then_the_ratings() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive) = (w.ada, w.positive);
    w.p.update_code(positive, &CodeEdit { rated: Some(true), rating_label: Some("Intensity".to_string()), ..CodeEdit::default() }).unwrap();
    let a = apply(&mut w, ada, 0, 4, positive).unwrap();
    w.p.set_coding_rating(a, Some(4)).unwrap();
    w.p.update_code(positive, &CodeEdit { rated: Some(false), rating_label: Some(String::new()), clear_ratings: true, ..CodeEdit::default() }).unwrap();

    assert_eq!(w.p.undo().unwrap().unwrap().action, "code.update");
    let code = w.p.get_code(positive).unwrap();
    assert!(code.rated);
    assert_eq!(code.rating_label, "Intensity");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "coding.rating");
    assert_eq!(w.p.list_codings(w.source).unwrap()[0].rating, Some(4));
}

#[test]
fn a_rating_goes_with_the_coding_to_another_rated_code_and_is_cleared_for_an_unrated_one() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative) = (w.ada, w.positive, w.negative);
    w.p.update_code(positive, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
    let id = apply(&mut w, ada, 0, 10, positive).unwrap();
    w.p.set_coding_rating(id, Some(4)).unwrap();

    // Negative is not rated, so the rating cannot go along.
    w.p.change_coding_code(id, negative).unwrap();
    assert_eq!(w.p.list_codings(w.source).unwrap()[0].rating, None);
    let change = w.p.events(100).unwrap().into_iter().rev().find(|e| e.action == "coding.change_code").unwrap();
    assert_eq!(change.before.as_ref().unwrap()["rating"], 4);
    assert!(change.after.as_ref().unwrap()["rating"].is_null());
    // The undo gives it back.
    w.p.undo().unwrap();
    let row = w.p.list_codings(w.source).unwrap().remove(0);
    assert_eq!((row.code_id, row.rating), (positive, Some(4)));
}

// ---- counterexamples ---------------------------------------------------------------------------------

#[test]
fn a_counterexample_is_a_coding_of_another_code_linked_to_the_code_it_is_not() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative) = (w.ada, w.positive, w.negative);
    let looks_positive = apply(&mut w, ada, 0, 10, negative).unwrap();
    let truly = apply(&mut w, ada, 12, 20, positive).unwrap();

    w.p.set_counterexample(positive, looks_positive, true).unwrap();
    w.p.set_counterexample(positive, looks_positive, true).unwrap();
    assert_eq!(w.p.list_counterexamples(positive).unwrap(), vec![looks_positive], "marking twice keeps one");
    assert!(w.p.list_anchors(positive).unwrap().is_empty(), "it is not an example");
    let events: Vec<String> = w.p.events(100).unwrap().into_iter().map(|e| e.action).filter(|a| a.starts_with("counterexample")).collect();
    assert_eq!(events, ["counterexample.add"], "marking again is not a change, so it is not logged");

    // Its own code's coding is an example, never a counterexample.
    assert!(w.p.set_counterexample(positive, truly, true).unwrap_err().to_string().contains("an example of it"));
    w.p.set_anchor(truly, true).unwrap();
    assert_eq!(w.p.list_anchors(positive).unwrap(), vec![truly]);
    assert_eq!(w.p.list_counterexamples(positive).unwrap(), vec![looks_positive]);

    let segments = w.p.counterexample_segments(positive).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!((segments[0].text.as_str(), segments[0].code_name.as_str()), ("abcdefghij", "Negative"));

    w.p.set_counterexample(positive, looks_positive, false).unwrap();
    assert!(w.p.list_counterexamples(positive).unwrap().is_empty());
}

#[test]
fn counterexamples_can_be_undone_and_go_when_their_coding_goes() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative) = (w.ada, w.positive, w.negative);
    let id = apply(&mut w, ada, 0, 10, negative).unwrap();
    w.p.set_counterexample(positive, id, true).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "counterexample.add");
    assert!(w.p.list_counterexamples(positive).unwrap().is_empty());

    w.p.set_counterexample(positive, id, true).unwrap();
    w.p.set_counterexample(positive, id, false).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "counterexample.remove");
    assert_eq!(w.p.list_counterexamples(positive).unwrap(), vec![id]);

    w.p.remove_coding(id).unwrap();
    assert!(w.p.list_counterexamples(positive).unwrap().is_empty(), "a link never points at a removed coding");
}

#[test]
fn merging_a_code_carries_its_examples_and_counterexamples_with_their_kind() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let (ada, positive, negative, neutral) = (w.ada, w.positive, w.negative, w.neutral);
    let own = apply(&mut w, ada, 0, 4, positive).unwrap();
    let other = apply(&mut w, ada, 6, 10, negative).unwrap();
    w.p.set_anchor(own, true).unwrap();
    w.p.set_counterexample(positive, other, true).unwrap();
    w.p.merge_codes(positive, neutral).unwrap();
    assert_eq!(w.p.list_anchors(neutral).unwrap(), vec![own]);
    assert_eq!(w.p.list_counterexamples(neutral).unwrap(), vec![other]);
}

// ---- properties travel with the codebook -------------------------------------------------------------

#[test]
fn a_new_code_can_be_made_with_its_properties_and_a_saved_version_keeps_them() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let mut c = NewCode::new("Intensity of fear");
    c.rated = true;
    c.rating_label = "Fear".to_string();
    c.rating_low = "Uneasy".to_string();
    c.rating_high = "Terrified".to_string();
    c.exclusive_group = "Affect".to_string();
    let id = w.p.add_code(&c).unwrap();
    let code = w.p.get_code(id).unwrap();
    assert!(code.rated);
    assert_eq!((code.exclusive_group.as_str(), code.rating_label.as_str(), code.rating_low.as_str(), code.rating_high.as_str()), ("Affect", "Fear", "Uneasy", "Terrified"));

    let version = w.p.snapshot_codebook("with properties").unwrap();
    let frozen = w.p.get_codebook_version(version).unwrap();
    let in_version = frozen["codes"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap();
    assert_eq!(in_version["rated"], true);
    assert_eq!(in_version["exclusive_group"], "Affect");
    assert_eq!(in_version["rating_high"], "Terrified");
}

#[test]
fn undoing_an_edit_restores_the_group_and_the_scale() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let positive = w.positive;
    w.p.update_code(positive, &CodeEdit { exclusive_group: Some("Mood".to_string()), rated: Some(true), rating_high: Some("Very".to_string()), ..CodeEdit::default() }).unwrap();
    assert_eq!(w.p.get_code(positive).unwrap().exclusive_group, "Mood");
    w.p.undo().unwrap().unwrap();
    let code = w.p.get_code(positive).unwrap();
    assert_eq!((code.exclusive_group.as_str(), code.rated, code.rating_high.as_str()), ("Sentiment", false, ""));
}
