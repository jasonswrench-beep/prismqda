//! Stretches of a recording (decision 155, project format 12): a code is given to the time between two moments of a recording linked to a
//! document, as it is given to an area of a picture. These tests check which stretches the engine accepts, how they are told in words and
//! counted beside words and areas, how they overlap, and that undo, merge, and reading a project back keep them.

use std::fs;
use std::path::PathBuf;

use prismqda_engine::{
    clock, clock_pair, default_recording_description, overlaps, CodeEdit, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, NewRegion, NewSource,
    OpenOptions, Project, QuerySpec, SegmentFilter, Shape, SourceType, UnitKind,
};
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn span(a: i64, b: i64) -> Shape {
    Shape::Span { start_ms: a, end_ms: b }
}

fn recording(dir: &TempDir, name: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, (0..4000).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
    path
}

struct W {
    p: Project,
    dir: TempDir,
    /// A document that stands for a recording, imported without captions.
    talk: i64,
    /// A transcript with a recording linked to it afterwards.
    interview: i64,
    /// A document with no recording.
    notes: i64,
    joy: i64,
    fear: i64,
    ada: i64,
}

fn alone_batch(name: &str) -> ImportBatch {
    let text = default_recording_description("talk.wav", "audio", 4000);
    let len = text.chars().count() as i64;
    ImportBatch {
        sources: vec![ImportedSource {
            name: name.to_string(),
            source_type: SourceType::Other,
            folder: String::new(),
            text,
            runs: vec![],
            units: vec![ImportedUnit::new(0, len, UnitKind::Paragraph)],
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    }
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &opts()).unwrap();
    let talk = p.apply_import(&alone_batch("Talk")).unwrap().source_ids[0];
    let wav = recording(&dir, "talk.wav");
    p.link_media_as(talk, wav.to_str().unwrap(), Some(true)).unwrap();
    let interview = p.add_source(&NewSource::new("Interview", SourceType::Transcript, "Ann: Hello there.\nBo: Fine, thanks.")).unwrap();
    let mp3 = recording(&dir, "interview.mp3");
    p.link_media(interview, mp3.to_str().unwrap()).unwrap();
    let notes = p.add_source(&NewSource::new("Notes", SourceType::Document, "Some words.")).unwrap();
    let joy = p.add_code(&NewCode::new("Joy")).unwrap();
    let fear = p.add_code(&NewCode::new("Fear")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    W { p, dir, talk, interview, notes, joy, fear, ada }
}

#[test]
fn a_stretch_is_coded_on_a_document_that_has_a_recording_and_nowhere_else() {
    let mut w = world();
    let id = w.p.apply_region(&NewRegion::new(w.talk, span(12_000, 45_000), w.joy, w.ada)).unwrap();
    let row = w.p.get_region(id).unwrap();
    assert_eq!(row.shape, span(12_000, 45_000));
    assert_eq!(row.page, 1);
    assert_eq!(row.text, "Recording, 0:12 to 0:45 (0:33 long)");
    // A transcript with a recording linked can have stretches too, beside its words.
    w.p.apply_region(&NewRegion::new(w.interview, span(0, 2500), w.joy, w.ada)).unwrap();
    w.p.apply_coding(&NewCoding::new(w.interview, 0, 3, w.fear, w.ada)).unwrap();

    let why = |r: prismqda_engine::Result<i64>| r.unwrap_err().to_string();
    assert!(why(w.p.apply_region(&NewRegion::new(w.notes, span(0, 1000), w.joy, w.ada))).contains("no recording linked"));
    assert!(why(w.p.apply_region(&NewRegion::new(9999, span(0, 1000), w.joy, w.ada))).contains("source"), "no such document");
    let mut on_page_two = NewRegion::new(w.talk, span(0, 1000), w.joy, w.ada);
    on_page_two.page = 2;
    assert!(why(w.p.apply_region(&on_page_two)).contains("no pages"));
    // An area of a picture is not a stretch: a document with a recording and no picture refuses a rectangle.
    assert!(why(w.p.apply_region(&NewRegion::new(w.talk, Shape::Rectangle { x: 0.1, y: 0.1, w: 0.2, h: 0.2 }, w.joy, w.ada))).contains("not a picture"));
}

#[test]
fn the_times_of_a_stretch_must_be_times_on_a_recording() {
    let mut w = world();
    let why = |w: &mut W, a: i64, b: i64| w.p.apply_region(&NewRegion::new(w.talk, span(a, b), w.joy, w.ada)).unwrap_err().to_string();
    assert!(why(&mut w, -1, 1000).contains("before the recording"));
    assert!(why(&mut w, 2000, 2000).contains("end after it starts"));
    assert!(why(&mut w, 3000, 2000).contains("end after it starts"));
    assert!(why(&mut w, 1000, 1050).contains("too short"));
    assert!(why(&mut w, 0, 8 * 24 * 3600 * 1000).contains("seven days"));
    assert!(w.p.list_regions(w.talk).unwrap().is_empty(), "every refusal left the document as it was");
    // The shortest stretch and a long one are fine.
    w.p.apply_region(&NewRegion::new(w.talk, span(1000, 1100), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.talk, span(0, 3 * 3600 * 1000), w.joy, w.ada)).unwrap();
}

#[test]
fn a_stretch_is_told_in_words_with_tenths_only_when_it_needs_them() {
    assert_eq!(clock(0, false), "0:00");
    assert_eq!(clock(65_000, false), "1:05");
    assert_eq!(clock(3_723_400, false), "1:02:03");
    assert_eq!(clock(3_723_400, true), "1:02:03.4");
    assert_eq!(clock_pair(12_000, 45_000), ("0:12".to_string(), "0:45".to_string()));
    assert_eq!(clock_pair(12_400, 45_000), ("0:12.4".to_string(), "0:45.0".to_string()), "both ends have tenths when either does");
    assert_eq!(span(12_400, 45_000).describe(), "Recording, 0:12.4 to 0:45.0 (0:32.6 long)");
    assert_eq!(span(1_000, 3_723_000).describe(), "Recording, 0:01 to 1:02:03 (1:02:02 long)");
}

#[test]
fn stretches_come_in_time_order_and_are_counted_beside_words_and_areas() {
    let mut w = world();
    w.p.apply_region(&NewRegion::new(w.talk, span(30_000, 40_000), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.talk, span(5_000, 9_000), w.fear, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.talk, span(5_000, 9_000), w.joy, w.ada)).unwrap();
    let starts: Vec<i64> = w.p.list_regions(w.talk).unwrap().iter().map(|r| if let Shape::Span { start_ms, .. } = r.shape { start_ms } else { -1 }).collect();
    assert_eq!(starts, [5_000, 5_000, 30_000], "by the time they start");

    let row = w.p.list_sources().unwrap().into_iter().find(|s| s.id == w.talk).unwrap();
    assert_eq!(row.codings, 3);
    let rec = row.recording.expect("the document says it has a recording");
    assert_eq!((rec.kind.as_str(), rec.file_name.as_str(), rec.stands_alone), ("audio", "talk.wav", true));
    let other = w.p.list_sources().unwrap().into_iter().find(|s| s.id == w.interview).unwrap();
    assert!(!other.recording.unwrap().stands_alone, "a recording linked to a transcript does not stand for it");
    assert!(w.p.list_sources().unwrap().into_iter().find(|s| s.id == w.notes).unwrap().recording.is_none());

    let segments = w.p.coded_segments(&SegmentFilter::default()).unwrap();
    let clips: Vec<_> = segments.iter().filter(|s| s.kind == "clip").collect();
    assert_eq!(clips.len(), 3);
    assert!(clips.iter().all(|c| c.page.is_none() && c.region.as_ref().is_some_and(Shape::is_span) && c.coding_id < 0));
    assert!(clips[0].text.starts_with("Recording, 0:30 to 0:40") || clips.iter().any(|c| c.text.starts_with("Recording, 0:30 to 0:40")));

    // The code's count includes its stretches.
    let joy = w.p.list_codes().unwrap().into_iter().find(|c| c.id == w.joy).unwrap();
    assert_eq!(joy.usage, 2, "two stretches carry Joy");
}

#[test]
fn stretches_overlap_when_they_share_time_and_never_with_an_area() {
    assert!(overlaps(&span(0, 5000), &span(4999, 9000)));
    assert!(!overlaps(&span(0, 5000), &span(5000, 9000)), "stretches that only touch do not overlap");
    assert!(!overlaps(&span(0, 5000), &Shape::Rectangle { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }));
    assert!(!overlaps(&Shape::Rectangle { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }, &span(0, 5000)));

    // A mutually exclusive group is kept on stretches that share time.
    let mut w = world();
    for c in [w.joy, w.fear] {
        w.p.update_code(c, &CodeEdit { exclusive_group: Some("mood".to_string()), ..CodeEdit::default() }).unwrap();
    }
    w.p.apply_region(&NewRegion::new(w.talk, span(0, 5000), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.talk, span(5000, 9000), w.fear, w.ada)).unwrap();
    let e = w.p.apply_region(&NewRegion::new(w.talk, span(4000, 6000), w.fear, w.ada)).unwrap_err();
    assert!(matches!(e, prismqda_engine::Error::ExclusiveGroup(_)), "{e}");
}

#[test]
fn a_stretch_can_be_moved_and_its_code_changed_and_every_step_is_undone() {
    let mut w = world();
    let id = w.p.apply_region(&NewRegion::new(w.talk, span(10_000, 20_000), w.joy, w.ada)).unwrap();
    let moved = w.p.reshape_region(id, &span(11_500, 21_000)).unwrap();
    assert_eq!(moved.shape, span(11_500, 21_000));
    assert!(w.p.reshape_region(id, &span(5, 5)).is_err(), "it cannot be made empty");
    assert!(w.p.reshape_region(id, &Shape::Rectangle { x: 0.1, y: 0.1, w: 0.3, h: 0.3 }).is_err(), "a stretch does not become an area");
    w.p.change_region_code(id, w.fear).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "region.change_code");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "region.reshape");
    assert_eq!(w.p.get_region(id).unwrap().shape, span(10_000, 20_000), "it is back where it was");

    w.p.remove_region(id).unwrap();
    assert!(w.p.list_regions(w.talk).unwrap().is_empty());
    assert_eq!(w.p.undo().unwrap().unwrap().action, "region.remove");
    assert_eq!(w.p.get_region(id).unwrap().shape, span(10_000, 20_000), "removing a stretch is undone with the stretch intact");
}

#[test]
fn a_query_for_a_code_finds_its_stretches_and_tells_them_as_clips() {
    let mut w = world();
    w.p.apply_region(&NewRegion::new(w.talk, span(2000, 6000), w.joy, w.ada)).unwrap();
    w.p.apply_region(&NewRegion::new(w.talk, span(2000, 6000), w.fear, w.ada)).unwrap();
    w.p.apply_coding(&NewCoding::new(w.notes, 0, 4, w.joy, w.ada)).unwrap();
    let spec: QuerySpec = serde_json::from_value(serde_json::json!({ "rule": { "type": "code", "code_ids": [w.joy] }, "result_unit": "excerpts" })).unwrap();
    let result = w.p.run_query(&spec).unwrap();
    let clips: Vec<_> = result.items.iter().filter(|i| i.kind == "clip").collect();
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].text, "Recording, 0:02 to 0:06 (0:04 long)");
    assert!(clips[0].page.is_none() && clips[0].region.as_ref().is_some_and(Shape::is_span));
    assert_eq!(result.items.iter().filter(|i| i.kind == "text").count(), 1);

    // Two codes on the same stretch of the same recording overlap.
    let both: QuerySpec = serde_json::from_value(serde_json::json!({
        "rule": { "type": "group", "match": "all", "items": [
            { "type": "code", "code_ids": [w.joy] },
            { "type": "also_coded", "code_ids": [w.fear], "within_chars": 0 }
        ] },
        "result_unit": "excerpts"
    }))
    .unwrap();
    let r = w.p.run_query(&both).unwrap();
    assert_eq!(r.items.iter().filter(|i| i.kind == "clip").count(), 1, "the stretch with both codes");
    assert_eq!(r.items.iter().filter(|i| i.kind == "text").count(), 0, "the words coded only Joy are not also Fear");
}

#[test]
fn undoing_the_link_of_a_standing_recording_puts_it_back_as_standing() {
    let mut w = world();
    let other = recording(&w.dir, "better.wav");
    let row = w.p.link_media(w.talk, other.to_str().unwrap()).unwrap();
    assert!(row.stands_alone, "linking again from another place keeps what the document stands for");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "media.link");
    let back = w.p.get_media(w.talk).unwrap().unwrap();
    assert_eq!((back.file_name.as_str(), back.stands_alone), ("talk.wav", true));
    w.p.unlink_media(w.talk).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "media.unlink");
    assert!(w.p.get_media(w.talk).unwrap().unwrap().stands_alone);
}

#[test]
fn merging_a_copy_brings_its_stretches_and_a_second_merge_adds_nothing() {
    let mut w = world();
    let copy_dir = TempDir::new().unwrap();
    let copy_path = copy_dir.path().join("copy.pqda");
    {
        // The copy imported the same recording document (same name and words) and coded two stretches of it, and has a document of its
        // own that this project lacks, whose recording is at a place this computer has.
        let mut theirs = Project::create(&copy_path, &NewProject::new("Copy"), &opts()).unwrap();
        let src = theirs.apply_import(&alone_batch("Talk")).unwrap().source_ids[0];
        theirs.link_media_as(src, w.dir.path().join("talk.wav").to_str().unwrap(), Some(true)).unwrap();
        let fresh = theirs.apply_import(&alone_batch("Second talk")).unwrap().source_ids[0];
        theirs.link_media_as(fresh, w.dir.path().join("interview.mp3").to_str().unwrap(), Some(true)).unwrap();
        let joy = theirs.add_code(&NewCode::new("Joy")).unwrap();
        let ben = theirs.add_coder(&NewCoder::human("Ben")).unwrap();
        theirs.apply_region(&NewRegion::new(src, span(1000, 3000), joy, ben)).unwrap();
        theirs.apply_region(&NewRegion::new(fresh, span(500, 900), joy, ben)).unwrap();
    }
    let report = w.p.merge_copy(&copy_path).unwrap();
    assert_eq!(report.areas_added, 2, "{report:?}");
    assert_eq!(w.p.list_regions(w.talk).unwrap().len(), 1);
    let second = w.p.list_sources().unwrap().into_iter().find(|s| s.name == "Second talk").expect("the new document came");
    assert!(second.recording.is_some_and(|r| r.stands_alone), "its recording was linked here too");
    let again = w.p.merge_copy(&copy_path).unwrap();
    assert_eq!((again.areas_added, again.areas_already_here), (0, 2));
}

#[test]
fn a_stretch_survives_closing_and_opening_the_project() {
    let mut w = world();
    let id = w.p.apply_region(&NewRegion::new(w.talk, span(12_345, 23_456), w.joy, w.ada)).unwrap();
    let path = w.dir.path().join("p.pqda");
    drop(w.p);
    let p = Project::open(&path, &opts()).unwrap();
    assert_eq!(p.get_region(id).unwrap().shape, span(12_345, 23_456));
}
