//! Coded segments: the codings with the words they cover (the base of every export).

use prismqda_engine::{NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SegmentFilter, SourceType, UnitKind};
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

struct Fx {
    p: Project,
    one: i64,
    two: i64,
    parent: i64,
    child: i64,
    other: i64,
    ada: i64,
    ben: i64,
}

/// Two sources, a code with a sub-code and one more code, and two coders.
fn fixture(dir: &TempDir) -> Fx {
    let mut p = Project::create(dir.path().join("s.pqda"), &NewProject::new("Segments"), &opts()).unwrap();
    // Characters outside the basic plane and combining marks make byte counts differ from character counts.
    let one = p.add_source(&NewSource::new("Interview", SourceType::Document, "Caf\u{e9} \u{2615} \u{1F600} nai\u{308}ve words.\nSecond line here.")).unwrap();
    let two = p.add_source(&NewSource::new("Survey", SourceType::Survey, "I felt welcome.")).unwrap();
    let parent = p.add_code(&NewCode::new("Belonging")).unwrap();
    let mut c = NewCode::new("Welcome");
    c.parent_id = Some(parent);
    let child = p.add_code(&c).unwrap();
    let other = p.add_code(&NewCode::new("Cost")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let ben = p.add_coder(&NewCoder::human("Ben")).unwrap();
    Fx { p, one, two, parent, child, other, ada, ben }
}

fn code_it(p: &mut Project, source: i64, start: i64, end: i64, code: i64, coder: i64) -> i64 {
    p.apply_coding(&NewCoding::new(source, start, end, code, coder)).unwrap()
}

#[test]
fn each_segment_carries_the_words_it_covers_counted_in_characters() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir);
    // "Café ☕ 😀 naïve": the coding covers the emoji and the word after it, and the é, ☕, and 😀 are single characters.
    let a = code_it(&mut f.p, f.one, 7, 14, f.child, f.ada);
    let b = code_it(&mut f.p, f.one, 0, 4, f.other, f.ada);
    let c = code_it(&mut f.p, f.two, 2, 6, f.child, f.ben);
    let segs = f.p.coded_segments(&SegmentFilter::default()).unwrap();
    assert_eq!(segs.len(), 3);

    // Source order, then text order.
    assert_eq!(segs.iter().map(|s| s.coding_id).collect::<Vec<_>>(), [b, a, c]);
    assert_eq!(segs[0].text, "Caf\u{e9}");
    assert_eq!(segs[1].text, "\u{1F600} na\u{ef}ve", "stored in NFC, so the combining mark became one character");
    assert_eq!(segs[2].text, "felt");
    assert_eq!((segs[1].start, segs[1].end), (7, 14));
    assert_eq!((segs[1].source_name.as_str(), segs[1].code_name.as_str(), segs[1].coder_name.as_str()), ("Interview", "Welcome", "Ada"));
    assert_eq!(segs[2].coder_role, "human");
    assert_eq!((segs[1].origin.as_str(), segs[1].layer.as_str()), ("human", "coder"));
}

#[test]
fn filters_by_source_code_coder_and_layer() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir);
    code_it(&mut f.p, f.one, 0, 4, f.parent, f.ada);
    code_it(&mut f.p, f.one, 7, 9, f.child, f.ada);
    code_it(&mut f.p, f.two, 2, 6, f.child, f.ben);
    code_it(&mut f.p, f.two, 7, 14, f.other, f.ben);

    let by_source = f.p.coded_segments(&SegmentFilter { source_ids: Some(vec![f.two]), ..Default::default() }).unwrap();
    assert_eq!(by_source.len(), 2);

    // A parent code includes its sub-codes unless exact.
    let family = f.p.coded_segments(&SegmentFilter { code_ids: Some(vec![f.parent]), ..Default::default() }).unwrap();
    assert_eq!(family.len(), 3);
    let exact = f.p.coded_segments(&SegmentFilter { code_ids: Some(vec![f.parent]), exact_codes: true, ..Default::default() }).unwrap();
    assert_eq!(exact.len(), 1);

    let ben = f.p.coded_segments(&SegmentFilter { coder_ids: Some(vec![f.ben]), ..Default::default() }).unwrap();
    assert_eq!(ben.len(), 2);
    let both = f.p.coded_segments(&SegmentFilter { coder_ids: Some(vec![f.ben]), code_ids: Some(vec![f.other]), ..Default::default() }).unwrap();
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].text, "welcome");

    assert_eq!(f.p.coded_segments(&SegmentFilter { layer: Some("final".into()), ..Default::default() }).unwrap().len(), 0);
    assert_eq!(f.p.coded_segments(&SegmentFilter { layer: Some("coder".into()), ..Default::default() }).unwrap().len(), 4);
}

#[test]
fn anchor_examples_come_back_with_their_text() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir);
    let a = code_it(&mut f.p, f.two, 2, 14, f.child, f.ada);
    code_it(&mut f.p, f.one, 0, 4, f.child, f.ada);
    assert!(f.p.anchor_examples(f.child).unwrap().is_empty());
    f.p.set_anchor(a, true).unwrap();
    let anchors = f.p.anchor_examples(f.child).unwrap();
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0].text, "felt welcome");
    assert_eq!(anchors[0].source_name, "Survey");
}

#[test]
fn units_give_their_speaker_and_their_own_identifier() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir);
    let unit = f.p.add_unit(f.two, UnitKind::SurveyResponse, 0, 15, Some("Respondent 7")).unwrap();
    let mut coding = NewCoding::new(f.two, 0, 15, f.child, f.ada);
    coding.unit_id = Some(unit);
    f.p.apply_coding(&coding).unwrap();
    code_it(&mut f.p, f.one, 0, 4, f.other, f.ada);
    let segs = f.p.coded_segments(&SegmentFilter::default()).unwrap();
    let with_unit = segs.iter().find(|s| s.unit_id.is_some()).unwrap();
    assert_eq!(with_unit.speaker.as_deref(), Some("Respondent 7"));
    assert_eq!(with_unit.unit_external_id, None, "no identifier was imported");
    assert!(segs.iter().filter(|s| s.unit_id.is_none()).all(|s| s.speaker.is_none()));
}
