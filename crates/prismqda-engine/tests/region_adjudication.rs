//! The final layer for areas of pictures and stretches of recordings (decision 176, project format 15): which coded areas and stretches are the
//! same, which every coder marked, which are in disagreement, how a person decides one, and what the final layer then holds.

use prismqda_engine::{
    read_image, sha256_hex, ExploreScope, ImageAttachment, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewProject, NewRegion, NewSource, OpenOptions, Project, SegmentFilter, Shape,
    SourceType, UnitKind,
};
use tempfile::TempDir;

fn png(w: u32, h: u32, salt: u8) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 2, 0, 0, 0, salt, salt, salt, salt]);
    b
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
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        images: vec![ImageAttachment { source: 0, image }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap().source_ids[0]
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Shape {
    Shape::Rectangle { x, y, w, h }
}

struct W {
    p: Project,
    _dir: TempDir,
    street: i64,
    yard: i64,
    joy: i64,
    fear: i64,
    cy: i64,
    /// the regions made, by what they are
    agreed: Vec<i64>,
    lonely: i64,
    two_of_three: Vec<i64>,
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let street = import_picture(&mut p, "street", png(800, 600, 1));
    let yard = import_picture(&mut p, "yard", png(400, 400, 2));
    let joy = p.add_code(&NewCode::new("Joy")).unwrap();
    let fear = p.add_code(&NewCode::new("Fear")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let ben = p.add_coder(&NewCoder::human("Ben")).unwrap();
    let cy = p.add_coder(&NewCoder::human("Cy")).unwrap();
    let mut mark = |source: i64, shape: Shape, code: i64, coder: i64| p.apply_region(&NewRegion::new(source, shape, code, coder)).unwrap();
    // all three coders marked the same corner as Joy (their rectangles overlap by more than half)
    let agreed = vec![mark(street, rect(0.10, 0.10, 0.20, 0.20), joy, ada), mark(street, rect(0.12, 0.10, 0.20, 0.20), joy, ben), mark(street, rect(0.11, 0.11, 0.20, 0.19), joy, cy)];
    // only Ada marked this one
    let lonely = mark(street, rect(0.60, 0.60, 0.20, 0.20), joy, ada);
    // Ada and Ben marked this as Fear and Cy did not
    let two_of_three = vec![mark(street, rect(0.05, 0.50, 0.20, 0.20), fear, ada), mark(street, rect(0.05, 0.52, 0.20, 0.20), fear, ben)];
    // a second picture that only Ada coded
    mark(yard, rect(0.2, 0.2, 0.3, 0.3), fear, ada);
    W { p, _dir: dir, street, yard, joy, fear, cy, agreed, lonely, two_of_three }
}

#[test]
fn areas_every_coder_marked_are_agreed_and_the_rest_are_in_disagreement_with_who_marked_them() {
    let w = world();
    let cases = w.p.region_disagreements().unwrap();
    assert_eq!(cases.len(), 2, "the area only Ada marked, and the one Ada and Ben marked");
    let by_key = |k: i64| cases.iter().find(|c| c.key == k).unwrap();
    let lonely = by_key(w.lonely);
    assert_eq!((lonely.source_name.as_str(), lonely.code_name.as_str(), lonely.kind.as_str(), lonely.members.len()), ("street", "Joy", "image", 1));
    assert_eq!(lonely.missing, ["Ben", "Cy"]);
    assert!(lonely.decision.is_none());
    let two = by_key(w.two_of_three[0]);
    assert_eq!((two.code_name.as_str(), two.members.iter().map(|m| m.coder_name.as_str()).collect::<Vec<_>>()), ("Fear", vec!["Ada", "Ben"]), "two areas that overlap are one area, with each coder's own kept");
    assert_eq!(two.missing, ["Cy"]);

    let s = w.p.final_summary().unwrap();
    assert_eq!((s.areas_decided, s.areas_agreed, s.areas_open), (0, 2, 2), "the corner all three marked and the picture only Ada coded are final; two are open");
    // the final layer holds only those two, each once
    let fin = w.p.coded_segments(&SegmentFilter { layer: Some("final".into()), ..Default::default() }).unwrap();
    let areas: Vec<(i64, &str)> = fin.iter().filter(|s| s.kind == "image").map(|s| (s.source_id, s.code_name.as_str())).collect();
    assert_eq!(areas, [(w.street, "Joy"), (w.yard, "Fear")]);
    assert!(fin.iter().all(|s| s.layer == "final"));
    assert!(fin.iter().any(|s| s.coding_id == -w.agreed[0]), "the lowest coder's area stands for the three");
    // the coders' own areas are untouched: seven in all
    assert_eq!(w.p.coded_segments(&Default::default()).unwrap().len(), 7);
}

#[test]
fn a_decision_accepts_or_leaves_out_an_area_with_its_reason_and_can_be_taken_back() {
    let mut w = world();
    assert!(w.p.decide_region(w.lonely, true, "   ", w.cy).unwrap_err().to_string().contains("reason"));
    assert!(w.p.decide_region(w.agreed[0], true, "Because.", w.cy).unwrap_err().to_string().contains("not in disagreement"), "an area every coder marked needs no decision");
    assert!(w.p.decide_region(w.lonely, true, "Because.", 9999).is_err(), "a decider that is not there");
    assert!(w.p.decide_region(9999, true, "Because.", w.cy).is_err());

    let c = w.p.decide_region(w.lonely, true, "It is clearly joy.", w.cy).unwrap();
    assert_eq!(c.decision.as_ref().map(|d| (d.accepted, d.reason.as_str(), d.adjudicator_name.as_str())), Some((true, "It is clearly joy.", "Cy")));
    w.p.decide_region(w.two_of_three[0], false, "Only the shadow, not a face.", w.cy).unwrap();
    let s = w.p.final_summary().unwrap();
    assert_eq!((s.areas_decided, s.areas_agreed, s.areas_open), (2, 2, 0));
    let fin = w.p.coded_segments(&SegmentFilter { layer: Some("final".into()), ..Default::default() }).unwrap();
    let areas: Vec<(i64, &str)> = fin.iter().filter(|s| s.kind == "image").map(|s| (s.source_id, s.code_name.as_str())).collect();
    assert_eq!(areas, [(w.street, "Joy"), (w.street, "Joy"), (w.yard, "Fear")], "the accepted one joins; the one left out does not");
    assert!(fin.iter().any(|s| s.coding_id == -w.lonely), "the accepted area is the coder's own, as it was drawn");
    assert!(!fin.iter().any(|s| s.code_name == "Fear" && s.source_id == w.street));

    // Explore counts the final layer when it is asked to
    let scope = ExploreScope { layer: Some("final".into()), ..Default::default() };
    assert_eq!(w.p.code_profile(w.joy, false, &scope).unwrap().coding_ids.len(), 2, "the agreed corner and the accepted area");
    assert_eq!(w.p.code_profile(w.joy, false, &ExploreScope::default()).unwrap().coding_ids.len(), 4, "the coders' own: three corners and the lonely one");
    assert_eq!(w.p.code_profile(w.fear, false, &scope).unwrap().coding_ids.len(), 1, "only the picture one coder coded");

    // deciding again replaces the decision, and Undo takes it back
    w.p.decide_region(w.lonely, false, "On second thought, no.", w.cy).unwrap();
    assert_eq!(w.p.final_summary().unwrap().areas_decided, 2);
    let undone = w.p.undo().unwrap().unwrap();
    assert_eq!(undone.action, "adjudication.region.set");
    let back = w.p.region_disagreements().unwrap().into_iter().find(|c| c.key == w.lonely).unwrap();
    assert_eq!(back.decision.map(|d| (d.accepted, d.reason)), Some((true, "It is clearly joy.".to_string())), "the earlier decision is back");
    w.p.undo().unwrap();
    w.p.undo().unwrap();
    let all = w.p.region_disagreements().unwrap();
    assert!(all.iter().all(|c| c.decision.is_none()), "three undos, and nothing is decided");
}

#[test]
fn removing_a_decided_area_takes_its_decision_and_undo_puts_both_back() {
    let mut w = world();
    w.p.decide_region(w.lonely, true, "Joy.", w.cy).unwrap();
    w.p.remove_region(w.lonely).unwrap();
    assert!(!w.p.region_disagreements().unwrap().iter().any(|c| c.key == w.lonely));
    let conn = rusqlite::Connection::open(w.p.path()).unwrap();
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM region_decision", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 0, "no decision is left about an area that is gone");
    w.p.undo().unwrap();
    let back = w.p.region_disagreements().unwrap().into_iter().find(|c| c.key == w.lonely).expect("the area is back");
    assert_eq!(back.decision.map(|d| d.reason), Some("Joy.".to_string()), "and so is its decision");
}

#[test]
fn stretches_of_a_recording_are_the_same_when_they_overlap_by_half() {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("r.pqda"), &NewProject::new("R"), &o).unwrap();
    let talk = p.add_source(&NewSource::new("Talk", SourceType::Transcript, "Ann: Hello there.\nBo: Fine, thanks.")).unwrap();
    let wav = dir.path().join("talk.wav");
    std::fs::write(&wav, (0..4000).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
    p.link_media(talk, wav.to_str().unwrap()).unwrap();
    let joy = p.add_code(&NewCode::new("Joy")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let ben = p.add_coder(&NewCoder::human("Ben")).unwrap();
    let span = |a, b| Shape::Span { start_ms: a, end_ms: b };
    for (shape, coder) in [(span(1000, 3000), ada), (span(1200, 3000), ben), (span(5000, 6000), ada)] {
        p.apply_region(&NewRegion::new(talk, shape, joy, coder)).unwrap();
    }
    let cases = p.region_disagreements().unwrap();
    assert_eq!(cases.len(), 1, "the two overlapping stretches are one, and agreed; the one only Ada marked is open");
    assert_eq!((cases[0].kind.as_str(), cases[0].missing.clone()), ("clip", vec!["Ben".to_string()]));
    let s = p.final_summary().unwrap();
    assert_eq!((s.areas_agreed, s.areas_open), (1, 1));
    let fin = p.coded_segments(&SegmentFilter { layer: Some("final".into()), ..Default::default() }).unwrap();
    assert_eq!(fin.len(), 1);
    assert_eq!(fin[0].kind, "clip");
    p.decide_region(cases[0].key, true, "A real laugh.", ben).unwrap();
    assert_eq!(p.coded_segments(&SegmentFilter { layer: Some("final".into()), ..Default::default() }).unwrap().len(), 2);
}
