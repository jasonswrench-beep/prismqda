//! Undo: each kind of change goes back exactly, in reverse order, and the trail stays honest.

use prismqda_engine::{
    CodeEdit, MemoKind, MemoScope, NewCode, NewCoder, NewCoding, NewMemo, NewProject, NewSource, OpenOptions, Project, SourceType,
};
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

struct W {
    p: Project,
    source: i64,
    coder: i64,
}

fn world(dir: &TempDir) -> W {
    let mut p = project(dir);
    let source = p.add_source(&NewSource::new("S", SourceType::Document, "abcdefghijklmnopqrstuvwxyz")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    W { p, source, coder }
}

#[test]
fn there_is_nothing_to_undo_in_a_fresh_session() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    p.add_coder(&NewCoder::human("Ada")).unwrap();
    assert!(p.next_undo().unwrap().is_none(), "making a project and adding a coder do not count");
    assert!(p.undo().unwrap().is_none());
    p.annotate("a note").unwrap();
    assert!(p.undo().unwrap().is_none(), "nor does a note in the log");
}

#[test]
fn undoing_a_new_coding_removes_it_and_is_logged_once() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let code = w.p.add_code(&NewCode::new("C")).unwrap();
    let id = w.p.apply_coding(&NewCoding::new(w.source, 0, 5, code, w.coder)).unwrap();
    w.p.set_anchor(id, true).unwrap();
    assert_eq!(w.p.next_undo().unwrap().as_deref(), Some("Took away the anchor example"));

    let first = w.p.undo().unwrap().unwrap();
    assert_eq!(first.action, "anchor.add");
    assert!(w.p.list_anchors(code).unwrap().is_empty());
    let second = w.p.undo().unwrap().unwrap();
    assert_eq!((second.action.as_str(), second.what.as_str()), ("coding.add", "Removed the coding you added"));
    assert!(w.p.list_codings(w.source).unwrap().is_empty());

    let events = w.p.events(100).unwrap();
    let undos: Vec<_> = events.iter().filter(|e| e.action == "undo").collect();
    assert_eq!(undos.len(), 2, "one event per undo, and no extra events for the reversal itself");
    assert_eq!(undos[1].after.as_ref().unwrap()["undone_action"], "coding.add");
    assert!(events.iter().all(|e| e.action != "coding.remove"), "an undo does not pretend to be a removal");
}

#[test]
fn a_removed_coding_comes_back_with_its_id_and_every_detail() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let code = w.p.add_code(&NewCode::new("C")).unwrap();
    let mut c = NewCoding::new(w.source, 3, 9, code, w.coder);
    c.rationale = "because".into();
    let id = w.p.apply_coding(&c).unwrap();
    w.p.remove_coding(id).unwrap();
    assert!(w.p.list_codings(w.source).unwrap().is_empty());

    assert_eq!(w.p.undo().unwrap().unwrap().action, "coding.remove");
    let back = w.p.list_codings(w.source).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!((back[0].id, back[0].start, back[0].end, back[0].rationale.as_str(), back[0].origin.as_str()), (id, 3, 9, "because", "human"));
}

#[test]
fn edits_to_codes_and_codings_go_back_in_reverse_order() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let a = w.p.add_code(&NewCode::new("A")).unwrap();
    let b = w.p.add_code(&NewCode::new("B")).unwrap();
    let coding = w.p.apply_coding(&NewCoding::new(w.source, 0, 4, a, w.coder)).unwrap();

    w.p.update_code(a, &CodeEdit { name: Some("Access".into()), definition: Some("Getting in".into()), ..Default::default() }).unwrap();
    w.p.change_coding_code(coding, b).unwrap();
    w.p.set_coding_rationale(coding, "a note").unwrap();
    w.p.move_code(b, Some(a), None).unwrap();
    w.p.retire_code(b).unwrap();

    let codes = |p: &Project| p.list_codes().unwrap();
    let get = |p: &Project, id: i64| codes(p).into_iter().find(|c| c.id == id).unwrap();

    w.p.undo().unwrap(); // retire
    assert_eq!(get(&w.p, b).status, "active");
    w.p.undo().unwrap(); // move
    assert_eq!(get(&w.p, b).parent_id, None);
    w.p.undo().unwrap(); // rationale
    assert_eq!(w.p.list_codings(w.source).unwrap()[0].rationale, "");
    w.p.undo().unwrap(); // change code
    assert_eq!(w.p.list_codings(w.source).unwrap()[0].code_id, a);
    w.p.undo().unwrap(); // update
    let restored = get(&w.p, a);
    assert_eq!((restored.name.as_str(), restored.definition.as_str(), restored.version), ("A", "", 1));
}

#[test]
fn memos_come_back_and_go_away() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let memo = NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Standalone, target_id: None, phase: None, title: "Idea".into(), body: "first".into() };
    let id = w.p.add_memo(&memo).unwrap();
    w.p.update_memo(id, None, Some("second")).unwrap();
    w.p.delete_memo(id).unwrap();

    w.p.undo().unwrap(); // delete
    let m = w.p.list_memos(None, None).unwrap();
    assert_eq!((m.len(), m[0].id, m[0].body.as_str(), m[0].title.as_str()), (1, id, "second", "Idea"));
    w.p.undo().unwrap(); // update
    assert_eq!(w.p.list_memos(None, None).unwrap()[0].body, "first");
    w.p.undo().unwrap(); // add
    assert!(w.p.list_memos(None, None).unwrap().is_empty());
}

#[test]
fn a_code_in_use_cannot_be_undone_away_and_nothing_changes() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let code = w.p.add_code(&NewCode::new("C")).unwrap();
    // Another session (the project reopened) uses the code, then this session tries to undo its creation.
    w.p.apply_coding(&NewCoding::new(w.source, 0, 3, code, w.coder)).unwrap();
    // Undo the coding first, as a person would, and then the code goes away cleanly.
    w.p.undo().unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "code.add");
    assert!(w.p.list_codes().unwrap().is_empty());

    // Now make the reverse order impossible by reaching in from outside: code used, then undo the code.
    let code = w.p.add_code(&NewCode::new("D")).unwrap();
    let c = rusqlite::Connection::open(w.p.path()).unwrap();
    c.execute(
        "INSERT INTO coding (source_id, start_offset, end_offset, code_id, coder_id, created_at) VALUES (?1, 0, 2, ?2, ?3, 'now')",
        rusqlite::params![w.source, code, w.coder],
    )
    .unwrap();
    let err = w.p.undo().unwrap_err().to_string();
    assert!(err.contains("already in use"), "{err}");
    assert_eq!(w.p.list_codes().unwrap().len(), 1, "the failed undo changed nothing");
    assert_eq!(w.p.events(1000).unwrap().iter().filter(|e| e.action == "undo").count(), 2, "and logged nothing");
}

#[test]
fn a_big_change_is_a_barrier_that_undo_explains() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let code = w.p.add_code(&NewCode::new("C")).unwrap();
    w.p.apply_coding(&NewCoding::new(w.source, 0, 3, code, w.coder)).unwrap();
    w.p.replace_source_text(w.source, "abcXdefghijklmnopqrstuvwxyz").unwrap();

    let preview = w.p.next_undo().unwrap().unwrap();
    assert!(preview.contains("editing a source's text"), "{preview}");
    let err = w.p.undo().unwrap_err().to_string();
    assert!(err.contains("too large to undo"), "{err}");
    assert_eq!(w.p.list_codings(w.source).unwrap().len(), 1, "nothing was undone behind the person's back");
}

#[test]
fn undo_only_reaches_this_sessions_changes() {
    let dir = TempDir::new().unwrap();
    let path = {
        let mut w = world(&dir);
        let code = w.p.add_code(&NewCode::new("C")).unwrap();
        w.p.apply_coding(&NewCoding::new(w.source, 0, 3, code, w.coder)).unwrap();
        w.p.path().to_path_buf()
    };
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::open(&path, &o).unwrap();
    assert!(p.undo().unwrap().is_none(), "a new session starts with nothing to undo");
}

#[test]
fn accepted_suggestions_can_be_taken_back() {
    use prismqda_engine::{NewProposal, ProposalOrigin};
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let code = w.p.add_code(&NewCode::new("C")).unwrap();
    let item = NewProposal { source_id: w.source, start: 0, end: 4, code_id: code, coder_id: w.coder, unit_id: None, rationale: String::new() };
    w.p.create_proposals(ProposalOrigin::Autocode, "t", &[item]).unwrap();
    let id = w.p.list_proposals(Some("pending"), None, 10).unwrap()[0].id;
    w.p.decide_proposals(&[id], true).unwrap();
    assert_eq!(w.p.list_codings(w.source).unwrap().len(), 1);

    assert_eq!(w.p.undo().unwrap().unwrap().action, "proposal.accept");
    assert!(w.p.list_codings(w.source).unwrap().is_empty());
    assert_eq!(w.p.list_proposals(Some("pending"), None, 10).unwrap().len(), 1, "back in the queue");
}

#[test]
fn a_codebook_snapshot_can_be_undone() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    w.p.add_code(&NewCode::new("C")).unwrap();
    assert_eq!(w.p.snapshot_codebook("v1").unwrap(), 1);
    w.p.undo().unwrap();
    assert!(w.p.list_codebook_versions().unwrap().is_empty());
    assert_eq!(w.p.snapshot_codebook("again").unwrap(), 1, "the version number is free again");
}
