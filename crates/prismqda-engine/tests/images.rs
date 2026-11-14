//! Pictures and coded areas: a picture is a document whose bytes are kept in the project, an area of it is coded like words are, and
//! the counting, the lists, undo, and the exclusive groups treat the two kinds of coding side by side.

use prismqda_engine::{
    overlaps, read_image, sha256_hex, CodeEdit, ExploreScope, ImageAttachment, ImportBatch, ImportedSource, ImportedUnit, MatrixSpec, NewCode, NewCoder, NewCoding, NewProject, NewRegion,
    NewSource, OpenOptions, Project, QuerySpec, RelationsSpec, RowSpec, SegmentFilter, Shape, SourceType, UnitKind,
};
use serde_json::{json, Value};
use tempfile::TempDir;

fn png(w: u32, h: u32, salt: u8) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 2, 0, 0, 0, salt, salt, salt, salt]);
    b
}

struct W {
    p: Project,
    _dir: TempDir,
    photo: i64,
    other: i64,
    words: i64,
    joy: i64,
    fear: i64,
    ada: i64,
    bo: i64,
}

fn import_picture(p: &mut Project, name: &str, bytes: Vec<u8>) -> i64 {
    let image = read_image(&format!("{name}.png"), bytes.clone()).unwrap();
    let text = prismqda_engine::default_description(&image.file_name, image.width, image.height);
    let len = text.chars().count() as i64;
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: name.to_string(),
            source_type: SourceType::Other,
            folder: String::new(),
            text,
            runs: vec![],
            units: vec![ImportedUnit::new(0, len, UnitKind::Paragraph)],
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: Some(sha256_hex(&bytes)),
            settings: Value::Null,
            warnings: vec![],
        }],
        images: vec![ImageAttachment { source: 0, image }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap().source_ids[0]
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let photo = import_picture(&mut p, "street", png(800, 600, 1));
    let other = import_picture(&mut p, "yard", png(400, 400, 2));
    let words = p.add_source(&NewSource::new("Notes", SourceType::Document, "Some words about the street.")).unwrap();
    let joy = p.add_code(&NewCode::new("Joy")).unwrap();
    let fear = p.add_code(&NewCode::new("Fear")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();
    W { p, _dir: dir, photo, other, words, joy, fear, ada, bo }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Shape {
    Shape::Rectangle { x, y, w, h }
}

#[test]
fn a_picture_is_a_document_whose_bytes_are_kept_exactly_as_the_file_was() {
    let w = world();
    let bytes = png(800, 600, 1);
    let row = w.p.get_image(w.photo).unwrap().unwrap();
    assert_eq!((row.mime.as_str(), row.file_name.as_str(), row.width, row.height, row.size_bytes), ("image/png", "street.png", 800, 600, bytes.len() as i64));
    let (mime, kept) = w.p.read_image_bytes(w.photo).unwrap();
    assert_eq!((mime.as_str(), kept), ("image/png", bytes));
    assert_eq!(w.p.get_image(w.words).unwrap(), None, "a document of words is not a picture");
    assert!(w.p.read_image_bytes(w.words).unwrap_err().to_string().contains("not a picture"));

    let list = w.p.list_sources().unwrap();
    let street = list.iter().find(|s| s.id == w.photo).unwrap();
    assert_eq!(street.image.as_ref().map(|i| (i.width, i.height)), Some((800, 600)));
    assert!(list.iter().find(|s| s.id == w.words).unwrap().image.is_none());
    assert!(w.p.source_detail(w.photo).unwrap().text.contains("800 by 600 pixels"), "its text is the description a screen reader speaks");
    assert_eq!(w.p.list_images().unwrap().len(), 2);
}

#[test]
fn an_area_that_cannot_be_coded_is_refused_in_words() {
    let mut w = world();
    let (photo, joy, ada) = (w.photo, w.joy, w.ada);
    let mut refuse = |shape: Shape| w.p.apply_region(&NewRegion::new(photo, shape, joy, ada)).unwrap_err().to_string();
    assert!(refuse(rect(0.1, 0.1, 0.0005, 0.3)).contains("too small"));
    assert!(refuse(rect(0.8, 0.1, 0.4, 0.3)).contains("past the edge"));
    assert!(refuse(rect(-0.2, 0.1, 0.3, 0.3)).contains("between 0 and 1"));
    assert!(refuse(rect(f64::NAN, 0.1, 0.3, 0.3)).contains("between 0 and 1"));
    assert!(refuse(Shape::Polygon { points: vec![[0.1, 0.1], [0.5, 0.5]] }).contains("at least 3 points"));
    assert!(refuse(Shape::Polygon { points: vec![[0.1, 0.1], [0.5, 0.5], [0.9, 0.9]] }).contains("too thin"), "three points on a line enclose nothing");
    assert!(refuse(Shape::Polygon { points: (0..501).map(|i| [(i % 100) as f64 / 100.0, (i / 5 % 100) as f64 / 100.0]).collect() }).contains("at most 500"));

    // A document of words, a retired code, a coder that does not exist, and a picture that does not exist.
    let words = w.words;
    assert!(w.p.apply_region(&NewRegion::new(words, rect(0.1, 0.1, 0.2, 0.2), joy, ada)).unwrap_err().to_string().contains("not a picture"));
    w.p.retire_code(w.fear).unwrap();
    assert!(w.p.apply_region(&NewRegion::new(photo, rect(0.1, 0.1, 0.2, 0.2), w.fear, ada)).unwrap_err().to_string().contains("retired"));
    assert!(w.p.apply_region(&NewRegion::new(photo, rect(0.1, 0.1, 0.2, 0.2), joy, 999)).is_err());
    assert!(w.p.apply_region(&NewRegion::new(999, rect(0.1, 0.1, 0.2, 0.2), joy, ada)).is_err());
    assert!(w.p.all_regions().unwrap().is_empty(), "nothing was stored by any refusal");
}

#[test]
fn a_coded_area_is_stored_rounded_listed_in_reading_order_and_counted_with_the_codes_use() {
    let mut w = world();
    let low = w.p.apply_region(&NewRegion::new(w.photo, rect(0.5, 0.6, 0.2, 0.2), w.joy, w.ada)).unwrap();
    let high_right = w.p.apply_region(&NewRegion::new(w.photo, rect(0.6, 0.1, 0.123_456_789, 0.2), w.joy, w.ada)).unwrap();
    let high_left = w.p.apply_region(&NewRegion::new(w.photo, Shape::Ellipse { x: 0.1, y: 0.1, w: 0.2, h: 0.3 }, w.fear, w.bo)).unwrap();
    let rows = w.p.list_regions(w.photo).unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), [high_left, high_right, low], "top to bottom, then left to right");
    assert_eq!(rows[1].shape, rect(0.6, 0.1, 0.12346, 0.2), "five decimals");
    assert!(rows[0].text.starts_with("Ellipse, 10% to 30% across and 10% to 40% down"), "{}", rows[0].text);
    assert_eq!(w.p.list_regions(w.other).unwrap().len(), 0);

    // Words and areas are counted side by side.
    w.p.apply_coding(&NewCoding::new(w.words, 0, 4, w.joy, w.ada)).unwrap();
    assert_eq!(w.p.get_code(w.joy).unwrap().usage, 3);
    assert_eq!(w.p.get_code(w.fear).unwrap().usage, 1);
    assert_eq!(w.p.list_sources().unwrap().iter().find(|s| s.id == w.photo).unwrap().codings, 3);
    assert_eq!(w.p.info().unwrap().codings, 4, "the project's count has the words and the areas");
    assert_eq!(serde_json::to_value(&rows[0]).unwrap()["shape"], "ellipse", "the shape is named in the row");
}

#[test]
fn coded_segments_list_areas_with_their_own_negative_numbers_beside_the_words() {
    let mut w = world();
    let text = w.p.apply_coding(&NewCoding::new(w.words, 0, 4, w.joy, w.ada)).unwrap();
    let area = w.p.apply_region(&NewRegion::new(w.photo, rect(0.1, 0.2, 0.3, 0.4), w.joy, w.ada)).unwrap();
    let all = w.p.coded_segments(&SegmentFilter::default()).unwrap();
    assert_eq!(all.len(), 2);
    let words = all.iter().find(|s| s.kind == "text").unwrap();
    let picture = all.iter().find(|s| s.kind == "image").unwrap();
    assert_eq!((words.coding_id, words.text.as_str()), (text, "Some"));
    assert_eq!((picture.coding_id, picture.source_name.as_str()), (-area, "street"));
    assert_eq!(picture.region, Some(rect(0.1, 0.2, 0.3, 0.4)));
    assert!(picture.text.starts_with("Rectangle, 10% to 40% across and 20% to 60% down, 12% of the picture"), "{}", picture.text);
    assert_eq!((picture.start, picture.end), (0, 0));

    // Filters work on both kinds.
    let only_fear = w.p.coded_segments(&SegmentFilter { code_ids: Some(vec![w.fear]), ..SegmentFilter::default() }).unwrap();
    assert!(only_fear.is_empty());
    let by_number = w.p.coded_segments(&SegmentFilter { coding_ids: Some(vec![-area]), ..SegmentFilter::default() }).unwrap();
    assert_eq!(by_number.iter().map(|s| s.coding_id).collect::<Vec<_>>(), [-area]);
    let by_document = w.p.coded_segments(&SegmentFilter { source_ids: Some(vec![w.words]), ..SegmentFilter::default() }).unwrap();
    assert_eq!(by_document.iter().map(|s| s.kind.as_str()).collect::<Vec<_>>(), ["text"]);
}

#[test]
fn every_change_to_a_coded_area_can_be_undone_and_is_in_its_history() {
    let mut w = world();
    let id = w.p.apply_region(&NewRegion::new(w.photo, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    w.p.change_region_code(id, w.fear).unwrap();
    w.p.reshape_region(id, &rect(0.2, 0.2, 0.3, 0.3)).unwrap();
    w.p.set_region_rationale(id, "a child laughing").unwrap();
    let row = w.p.get_region(id).unwrap();
    assert_eq!((row.code_id, row.shape.clone(), row.rationale.as_str()), (w.fear, rect(0.2, 0.2, 0.3, 0.3), "a child laughing"));

    assert_eq!(w.p.undo().unwrap().unwrap().what, "Restored the coded area's earlier note");
    assert_eq!(w.p.undo().unwrap().unwrap().what, "Moved the coded area back");
    assert_eq!(w.p.get_region(id).unwrap().shape, rect(0.1, 0.1, 0.3, 0.3));
    assert_eq!(w.p.undo().unwrap().unwrap().what, "Changed the coded area back to its earlier code");
    assert_eq!(w.p.get_region(id).unwrap().code_id, w.joy);

    let history: Vec<String> = w.p.region_history(id).unwrap().into_iter().map(|h| h.action).collect();
    assert_eq!(history, ["region.add", "region.change_code", "region.reshape", "region.rationale"]);

    // Removing it is logged with everything it was, and undo puts the same area back under the same number.
    w.p.remove_region(id).unwrap();
    assert!(w.p.get_region(id).is_err());
    assert_eq!(w.p.undo().unwrap().unwrap().what, "Put back the coded area you removed");
    assert_eq!(w.p.get_region(id).unwrap().shape, rect(0.1, 0.1, 0.3, 0.3));
    assert_eq!(w.p.get_region(id).unwrap().code_id, w.joy);

    // Undoing the coding itself takes the area away.
    while let Some(u) = w.p.undo().unwrap() {
        if u.action == "region.add" {
            break;
        }
    }
    assert!(w.p.all_regions().unwrap().is_empty());
    assert!(w.p.next_undo().unwrap().is_none() || !w.p.next_undo().unwrap().unwrap().contains("area"));
}

#[test]
fn a_rating_belongs_to_a_rated_code_and_goes_with_the_area() {
    let mut w = world();
    let id = w.p.apply_region(&NewRegion::new(w.photo, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    assert!(w.p.set_region_rating(id, Some(3)).unwrap_err().to_string().contains("not a rated code"));
    assert!(w.p.set_region_rating(id, Some(9)).unwrap_err().to_string().contains("1 to 5"));
    w.p.update_code(w.joy, &CodeEdit { rated: Some(true), ..CodeEdit::default() }).unwrap();
    w.p.set_region_rating(id, Some(4)).unwrap();
    assert_eq!(w.p.get_region(id).unwrap().rating, Some(4));
    w.p.change_region_code(id, w.fear).unwrap();
    assert_eq!(w.p.get_region(id).unwrap().rating, None, "the new code is not rated, so the rating goes");
    w.p.undo().unwrap();
    assert_eq!(w.p.get_region(id).unwrap().rating, Some(4), "and comes back with the earlier code");
    w.p.set_region_rating(id, None).unwrap();
    assert_eq!(w.p.get_region(id).unwrap().rating, None);
}

#[test]
fn a_mutually_exclusive_group_applies_to_areas_that_share_ground_not_to_apart_ones() {
    let mut w = world();
    w.p.update_code(w.joy, &CodeEdit { exclusive_group: Some("Mood".to_string()), ..CodeEdit::default() }).unwrap();
    w.p.update_code(w.fear, &CodeEdit { exclusive_group: Some("mood".to_string()), ..CodeEdit::default() }).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    let err = w.p.apply_region(&NewRegion::new(w.photo, rect(0.3, 0.3, 0.3, 0.3), w.fear, w.ada)).unwrap_err();
    assert!(err.to_string().contains("'Fear' is in the group 'Mood'") && err.to_string().contains("this area already has 'Joy'"), "{err}");
    // An area elsewhere on the picture, another coder's area, and another picture are all fine.
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.6, 0.6, 0.3, 0.3), w.fear, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.3, 0.3, 0.3, 0.3), w.fear, w.bo)).unwrap();
    w.p.apply_region(&NewRegion::new(w.other, rect(0.1, 0.1, 0.5, 0.5), w.fear, w.ada)).unwrap();
    // Moving an area onto ground the group forbids is refused too.
    let apart = w.p.list_regions(w.photo).unwrap().into_iter().find(|r| r.coder_id == w.ada && r.code_id == w.fear).unwrap();
    assert!(w.p.reshape_region(apart.id, &rect(0.2, 0.2, 0.3, 0.3)).is_err());
}

#[test]
fn shapes_overlap_by_the_ground_they_share() {
    let square = rect(0.0, 0.0, 0.5, 0.5);
    assert!(overlaps(&square, &rect(0.4, 0.4, 0.5, 0.5)));
    assert!(!overlaps(&square, &rect(0.5, 0.0, 0.5, 0.5)), "touching along an edge is not sharing ground");
    // The box of a circle reaches its corner, but the circle does not.
    let circle = Shape::Ellipse { x: 0.0, y: 0.0, w: 0.5, h: 0.5 };
    let corner = rect(0.45, 0.45, 0.2, 0.2);
    assert!(!overlaps(&circle, &corner), "the corner of the box is outside the circle");
    assert!(overlaps(&circle, &rect(0.2, 0.2, 0.2, 0.2)));
    let triangle = Shape::Polygon { points: vec![[0.0, 0.0], [0.5, 0.0], [0.0, 0.5]] };
    assert!(!overlaps(&triangle, &rect(0.35, 0.35, 0.1, 0.1)), "the far corner of the box is outside the triangle");
    assert!(overlaps(&triangle, &rect(0.05, 0.05, 0.1, 0.1)));
    assert!(overlaps(&triangle, &triangle));
    assert!(triangle.contains(0.1, 0.1) && !triangle.contains(0.4, 0.4));
}

#[test]
fn merging_codes_moves_their_areas_and_a_code_in_use_cannot_be_removed_by_undo() {
    let mut w = world();
    let id = w.p.apply_region(&NewRegion::new(w.photo, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    let outcome = w.p.merge_codes(w.joy, w.fear).unwrap();
    assert_eq!(outcome.codings_moved, 1);
    assert_eq!(w.p.get_region(id).unwrap().code_id, w.fear);

    let fresh = w.p.add_code(&NewCode::new("Awe")).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.5, 0.5, 0.2, 0.2), fresh, w.ada)).unwrap();
    // The newest change is the area, so undo takes that back first; the code is then free of use.
    assert_eq!(w.p.undo().unwrap().unwrap().action, "region.add");
}

#[test]
fn explore_counts_areas_with_the_words_two_codes_on_one_shape_are_one_excerpt() {
    let mut w = world();
    let shape = rect(0.1, 0.1, 0.4, 0.4);
    w.p.apply_region(&NewRegion::new(w.photo, shape.clone(), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, shape, w.fear, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.3, 0.3, 0.4, 0.4), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.other, rect(0.0, 0.0, 0.2, 0.2), w.joy, w.bo)).unwrap();
    w.p.apply_coding(&NewCoding::new(w.words, 0, 4, w.joy, w.ada)).unwrap();

    let spec = MatrixSpec { rows: RowSpec::Documents, code_ids: vec![w.joy, w.fear], include_descendants: true, scope: ExploreScope::default() };
    let m = w.p.code_matrix(&spec).unwrap();
    let row = |label: &str| m.rows.iter().find(|r| r.label == label).unwrap();
    let c = |a, e, d, k| prismqda_engine::Counts { assignments: a, excerpts: e, documents: d, cases: k };
    assert_eq!(row("street").cells, vec![c(2, 2, 1, 0), c(1, 1, 1, 0)]);
    assert_eq!(row("street").total, c(3, 2, 1, 0), "the first shape has both codes, so over both columns it is one excerpt");
    assert_eq!(row("yard").cells, vec![c(1, 1, 1, 0), c(0, 0, 0, 0)]);
    assert_eq!(row("Notes").cells[0], c(1, 1, 1, 0));
    assert_eq!(m.columns[0].total, c(4, 4, 3, 0));
    assert_eq!(m.universe, c(5, 4, 3, 0), "the words and the areas are looked at together");

    // The evidence of a cell names the areas by their negative numbers.
    let evidence = w.p.code_matrix_evidence(&spec, Some("source:1"), Some(w.joy)).unwrap();
    assert!(evidence.iter().all(|i| *i < 0) && evidence.len() == 2, "{evidence:?}");
}

#[test]
fn codes_on_areas_that_overlap_occur_together_and_apart_ones_do_not() {
    let mut w = world();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.1, 0.1, 0.4, 0.4), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.4, 0.4, 0.4, 0.4), w.fear, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.other, rect(0.0, 0.0, 0.2, 0.2), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.other, rect(0.7, 0.7, 0.2, 0.2), w.fear, w.ada)).unwrap();
    let spec: RelationsSpec = serde_json::from_value(json!({ "code_ids": [w.joy, w.fear], "mode": "overlap", "same_coder": true })).unwrap();
    let r = w.p.code_relations(&spec).unwrap();
    assert_eq!(r.cells, vec![vec![2, 1], vec![1, 2]], "one pair on the street picture, none on the yard picture");
    let ev = w.p.code_relations_evidence(&spec, w.joy, w.fear).unwrap();
    assert_eq!(ev.len(), 2);
    assert!(ev.iter().all(|i| *i < 0));
    let profile = w.p.code_profile(w.joy, true, &ExploreScope::default()).unwrap();
    assert_eq!(profile.together_with.iter().map(|c| (c.name.as_str(), c.assignments)).collect::<Vec<_>>(), [("Fear", 1)]);
}

#[test]
fn a_query_finds_areas_by_code_and_returns_them_with_their_shape() {
    let mut w = world();
    let area = w.p.apply_region(&NewRegion::new(w.photo, rect(0.2, 0.2, 0.2, 0.2), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.5, 0.5, 0.2, 0.2), w.fear, w.ada)).unwrap();
    w.p.apply_coding(&NewCoding::new(w.words, 0, 4, w.joy, w.ada)).unwrap();
    let spec: QuerySpec = serde_json::from_value(json!({ "rule": { "type": "code", "code_ids": [w.joy] }, "result_unit": "excerpts" })).unwrap();
    let result = w.p.run_query(&spec).unwrap();
    assert_eq!((result.total, result.assignments), (2, 2));
    let picture = result.items.iter().find(|i| i.kind == "image").unwrap();
    assert_eq!((picture.source_name.as_str(), picture.region.clone(), picture.codings[0].coding_id), ("street", Some(rect(0.2, 0.2, 0.2, 0.2)), -area));
    assert!(picture.text.starts_with("Rectangle"));
    let words = result.items.iter().find(|i| i.kind == "text").unwrap();
    assert_eq!(words.text, "Some");
    let (ids, groups) = w.p.matching_coding_ids(&spec).unwrap();
    assert_eq!((ids.contains(&-area), groups), (true, 2));

    // "Also coded" for areas means they overlap.
    let near: QuerySpec = serde_json::from_value(json!({
        "rule": { "type": "group", "match": "all", "items": [
            { "type": "code", "code_ids": [w.joy] },
            { "type": "also_coded", "code_ids": [w.fear], "within_chars": 0, "include_descendants": true, "same_coder": true }
        ] },
        "result_unit": "assignments"
    }))
    .unwrap();
    assert_eq!(w.p.run_query(&near).unwrap().total, 0, "the two areas do not touch");
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.3, 0.3, 0.2, 0.2), w.fear, w.ada)).unwrap();
    let after = w.p.run_query(&near).unwrap();
    assert_eq!(after.total, 1);
}

#[test]
fn a_frozen_query_remembers_areas_by_their_numbers_and_says_when_one_is_removed() {
    let mut w = world();
    let area = w.p.apply_region(&NewRegion::new(w.photo, rect(0.2, 0.2, 0.2, 0.2), w.joy, w.ada)).unwrap();
    let spec: Value = json!({ "rule": { "type": "code", "code_ids": [w.joy] }, "result_unit": "assignments" });
    let saved = w.p.save_query("Joyful", "", &json!({}), &spec).unwrap();
    w.p.freeze_saved_query(saved).unwrap();
    w.p.remove_region(area).unwrap();
    let cmp = w.p.compare_frozen_query(saved).unwrap();
    assert_eq!((cmp.frozen, cmp.now, cmp.no_longer_found, cmp.removed), (1, 0, 1, 1), "it is gone because the area was removed, not recoded");
}

#[test]
fn a_memo_or_a_link_can_be_about_an_area_and_not_about_one_that_is_not_there() {
    use prismqda_engine::{MemoKind, MemoScope, NewMemo};
    let mut w = world();
    let area = w.p.apply_region(&NewRegion::new(w.photo, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    // An area is numbered apart from the codings of words: area 3 is -3 wherever the two share a list.
    let about = -area;
    let memo = |target: i64, title: &str| NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Coding, target_id: Some(target), phase: None, title: title.to_string(), body: "Why this corner".to_string() };
    let id = w.p.add_memo(&memo(about, "The corner")).unwrap();
    let found = w.p.list_memos(Some(MemoScope::Coding), Some(about)).unwrap();
    assert_eq!((found.len(), found[0].id, found[0].target_id), (1, id, Some(about)));
    // The coding with the same number as the area is a different thing, and has no such memo.
    assert!(w.p.list_memos(Some(MemoScope::Coding), Some(area)).unwrap().is_empty());
    // An area that is not there, or was taken away, cannot be written about.
    assert!(w.p.add_memo(&memo(-(area + 100), "Nothing")).unwrap_err().to_string().contains("memo target"));
    w.p.remove_region(area).unwrap();
    assert!(w.p.add_memo(&memo(about, "Gone")).is_err(), "an area that was removed is not there to write about");
    // A link from a memo to an area works the same way.
    let other = w.p.apply_region(&NewRegion::new(w.photo, rect(0.5, 0.5, 0.2, 0.2), w.fear, w.ada)).unwrap();
    let link = w.p.add_link("memo", id, "coding", -other, "is about").unwrap();
    assert!(link > 0);
    assert!(w.p.add_link("memo", id, "coding", -(other + 100), "is about").is_err());
}

#[test]
fn coders_agreement_on_a_picture_is_measured_cell_by_cell_and_said_to_be() {
    use prismqda_engine::AgreementRequest;
    let mut w = world();
    // Ada coded the whole left half Joy. Bo coded its top half Joy and its bottom half Fear.
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.0, 0.0, 0.5, 1.0), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.0, 0.0, 0.5, 0.5), w.joy, w.bo)).unwrap();
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.0, 0.5, 0.5, 0.5), w.fear, w.bo)).unwrap();
    let ask = |mode: &str, areas: bool| -> AgreementRequest {
        serde_json::from_value(json!({ "coder_ids": [w.ada, w.bo], "mode": mode, "bootstrap_replications": 50, "save": false, "areas": areas })).unwrap()
    };

    // Single label: the left half is 8 columns of 16 cells, each cell coded by both; they agree on the top half and differ on the bottom.
    let one = w.p.agreement_run(&ask("single_label", true)).unwrap();
    assert_eq!(one.units_in_sample, 128, "only the cells both coders drew on are compared");
    assert_eq!(one.overall.as_ref().unwrap().percent_agreement, Some(0.5));
    assert_eq!(one.disagreements, 64);
    assert!(one.notes.iter().any(|n| n.contains("16 by 16 grid") && n.contains("not independent")), "{:?}", one.notes);
    assert!(one.report_text.contains("grid"), "the note is in the written report");

    // Each code on its own: Joy was applied by both on the top half, and by Ada alone on the bottom half.
    let per = w.p.agreement_run(&ask("per_code", true)).unwrap();
    let joy = per.by_code.iter().find(|c| c.code_id == w.joy).unwrap();
    assert_eq!(joy.detail.agreement.percent_agreement, Some(0.5));

    // Asked to leave areas out, a project coded only on a picture has nothing to compare.
    let none = w.p.agreement_run(&ask("single_label", false)).unwrap_err().to_string();
    assert!(none.contains("nothing to compare"), "{none}");

    // A cell neither coder drew on, or only one did, is never compared: the right half of the picture adds nothing.
    w.p.apply_region(&NewRegion::new(w.photo, rect(0.5, 0.0, 0.5, 1.0), w.fear, w.ada)).unwrap();
    assert_eq!(w.p.agreement_run(&ask("single_label", true)).unwrap().units_in_sample, 128);
}

#[test]
fn a_packet_for_a_chatbot_carries_words_only_and_counts_the_pictures_it_left_out() {
    use prismqda_engine::PacketSpec;
    let mut w = world();
    let spec: PacketSpec = serde_json::from_str("{}").unwrap();
    let packet = w.p.build_packet(&spec, w.ada).unwrap();
    assert_eq!(packet.passages, 1, "only the document of words is sent");
    assert_eq!(packet.left_out.pictures, 2, "the two pictures are counted, not silently dropped");
    assert!(!packet.prompt.contains("pixels"), "no picture description went into the packet");
}
