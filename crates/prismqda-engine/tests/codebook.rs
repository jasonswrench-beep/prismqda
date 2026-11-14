//! Codebook operations, memos, links, and bulk coding through the public API.

use prismqda_engine::{
    CodeEdit, Error, MemoKind, MemoScope, NewCode, NewCoder, NewCoding, NewMemo, NewProject, NewSource, OpenOptions, Project,
    SourceType,
};
use rusqlite::Connection;
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn code(p: &mut Project, name: &str, parent: Option<i64>) -> i64 {
    let mut c = NewCode::new(name);
    c.parent_id = parent;
    p.add_code(&c).unwrap()
}

/// A project with a 30-character source, one coder, and three codings.
fn coded(dir: &TempDir) -> (Project, i64, i64, Vec<i64>) {
    let mut p = project(dir);
    let source = p.add_source(&NewSource::new("S", SourceType::Document, "abcdefghijklmnopqrstuvwxyz0123")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let a = code(&mut p, "Access", None);
    let b = code(&mut p, "Barrier", None);
    let ids = p
        .apply_codings(&[
            NewCoding::new(source, 0, 5, a, coder),
            NewCoding::new(source, 5, 10, a, coder),
            NewCoding::new(source, 10, 15, b, coder),
        ])
        .unwrap();
    (p, source, coder, ids)
}

fn count(p: &Project, sql: &str) -> i64 {
    Connection::open(p.path()).unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
}

// ---- reading ----

#[test]
fn codes_are_listed_with_usage_and_codings_in_text_order() {
    let dir = TempDir::new().unwrap();
    let (p, source, _, ids) = coded(&dir);
    let codes = p.list_codes().unwrap();
    assert_eq!(codes.iter().map(|c| (c.name.as_str(), c.usage)).collect::<Vec<_>>(), [("Access", 2), ("Barrier", 1)]);
    let codings = p.list_codings(source).unwrap();
    assert_eq!(codings.iter().map(|c| (c.start, c.end)).collect::<Vec<_>>(), [(0, 5), (5, 10), (10, 15)]);
    assert_eq!(codings.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
    assert_eq!(p.list_coders().unwrap()[0].name, "Ada");
}

#[test]
fn ensure_human_coder_reuses_the_coder_with_that_local_id() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let a = p.ensure_human_coder("Ada", "ada-1").unwrap();
    let b = p.ensure_human_coder("Ada Again", "ada-1").unwrap();
    assert_eq!(a, b);
    assert_eq!(p.list_coders().unwrap().len(), 1);
}

// ---- bulk coding ----

#[test]
fn bulk_coding_is_all_or_nothing_and_logs_each_coding() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, coder, _) = coded(&dir);
    let a = p.list_codes().unwrap()[0].id;
    let before_events = p.events(10_000).unwrap().len();
    let bad = [NewCoding::new(source, 20, 25, a, coder), NewCoding::new(source, 99, 120, a, coder)];
    let err = p.apply_codings(&bad).unwrap_err();
    assert!(err.to_string().contains("coding 2 of 2"), "{err}");
    assert_eq!(p.list_codings(source).unwrap().len(), 3, "the first coding must not survive the failed batch");
    assert_eq!(p.events(10_000).unwrap().len(), before_events);

    let good = [NewCoding::new(source, 20, 25, a, coder), NewCoding::new(source, 25, 30, a, coder)];
    assert_eq!(p.apply_codings(&good).unwrap().len(), 2);
    assert_eq!(p.events(10_000).unwrap().len(), before_events + 2);
}

// ---- editing codes ----

#[test]
fn editing_a_code_counts_versions_only_when_the_meaning_changes() {
    let dir = TempDir::new().unwrap();
    let (mut p, ..) = coded(&dir);
    let id = p.list_codes().unwrap()[0].id;
    assert_eq!(p.get_code(id).unwrap().version, 1);

    let edited = p.update_code(id, &CodeEdit { definition: Some("Getting in the door".into()), ..Default::default() }).unwrap();
    assert_eq!((edited.version, edited.definition.as_str()), (2, "Getting in the door"));
    let recolored = p.update_code(id, &CodeEdit { color: Some("#aa3300".into()), ..Default::default() }).unwrap();
    assert_eq!((recolored.version, recolored.color.as_str()), (2, "#aa3300"), "color is not part of the meaning");
    let renamed = p.update_code(id, &CodeEdit { name: Some("Access to care".into()), ..Default::default() }).unwrap();
    assert_eq!((renamed.version, renamed.name.as_str()), (3, "Access to care"));

    let update_events = p.events(1000).unwrap().into_iter().filter(|e| e.action == "code.update").count();
    assert_eq!(update_events, 3);
    let last = p.events(1).unwrap().remove(0);
    assert_eq!(last.before.unwrap()["name"], "Access");
    assert_eq!(last.after.unwrap()["name"], "Access to care");

    // A no-op edit logs nothing.
    p.update_code(id, &CodeEdit::default()).unwrap();
    assert_eq!(p.events(1000).unwrap().into_iter().filter(|e| e.action == "code.update").count(), 3);
}

#[test]
fn renaming_to_a_sibling_name_or_an_empty_name_is_refused() {
    let dir = TempDir::new().unwrap();
    let (mut p, ..) = coded(&dir);
    let codes = p.list_codes().unwrap();
    let err = p.update_code(codes[0].id, &CodeEdit { name: Some("barrier".into()), ..Default::default() }).unwrap_err();
    assert!(matches!(err, Error::Invalid(_)), "{err}");
    assert!(p.update_code(codes[0].id, &CodeEdit { name: Some("  ".into()), ..Default::default() }).is_err());
    assert!(matches!(p.update_code(999, &CodeEdit::default()).unwrap_err(), Error::NotFound { .. }));
}

// ---- tree ----

#[test]
fn moving_codes_reorders_siblings_and_never_makes_a_loop() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let a = code(&mut p, "A", None);
    let b = code(&mut p, "B", None);
    let c = code(&mut p, "C", None);
    let a1 = code(&mut p, "A1", Some(a));

    // Put C first among the top-level codes.
    p.move_code(c, None, Some(0)).unwrap();
    let order: Vec<String> = p.list_codes().unwrap().into_iter().filter(|x| x.parent_id.is_none()).map(|x| x.name).collect();
    assert_eq!(order, ["C", "A", "B"]);

    // Move B under A, between nothing and A1.
    p.move_code(b, Some(a), Some(0)).unwrap();
    let kids: Vec<String> = p.list_codes().unwrap().into_iter().filter(|x| x.parent_id == Some(a)).map(|x| x.name).collect();
    assert_eq!(kids, ["B", "A1"]);

    // A code cannot go under itself or its own sub-code.
    assert!(p.move_code(a, Some(a), None).is_err());
    assert!(p.move_code(a, Some(a1), None).is_err());
    assert!(p.move_code(a, Some(b), None).is_err());

    // A move that would make two active siblings share a name is refused.
    let other_a1 = code(&mut p, "A1", None);
    assert!(p.move_code(other_a1, Some(a), None).is_err());
    assert_eq!(p.list_codes().unwrap().iter().find(|x| x.id == other_a1).unwrap().parent_id, None);
}

// ---- retire, merge, split ----

#[test]
fn a_retired_code_keeps_its_codings_and_can_come_back() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, coder, _) = coded(&dir);
    let a = p.list_codes().unwrap()[0].id;
    p.retire_code(a).unwrap();
    assert_eq!(p.get_code(a).unwrap().status, "retired");
    assert_eq!(p.get_code(a).unwrap().usage, 2, "history stays");
    assert!(p.apply_coding(&NewCoding::new(source, 20, 25, a, coder)).is_err());
    p.restore_code(a).unwrap();
    assert!(p.apply_coding(&NewCoding::new(source, 20, 25, a, coder)).is_ok());
}

#[test]
fn merging_moves_codings_and_children_and_logs_the_mapping() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, _, ids) = coded(&dir);
    let codes = p.list_codes().unwrap();
    let (a, b) = (codes[0].id, codes[1].id);
    let child = code(&mut p, "Barrier child", Some(b));
    p.set_anchor(ids[2], true).unwrap();
    let set = p.add_code_set("Core", "").unwrap();
    p.set_code_in_set(set, b, true).unwrap();

    let out = p.merge_codes(b, a).unwrap();
    assert_eq!((out.codings_moved, out.children_moved), (1, 1));
    let merged = p.get_code(b).unwrap();
    assert_eq!((merged.status.as_str(), merged.merged_into_id), ("merged", Some(a)));
    assert_eq!(p.get_code(a).unwrap().usage, 3);
    assert_eq!(p.get_code(child).unwrap().parent_id, Some(a));
    assert_eq!(p.list_codings(source).unwrap().iter().filter(|c| c.code_id == a).count(), 3);
    assert_eq!(p.list_anchors(a).unwrap(), vec![ids[2]], "the anchor example travels with its coding");
    assert_eq!(p.list_code_sets().unwrap()[0].2, vec![a], "set membership moves too");

    let event = p.events(1).unwrap().remove(0);
    assert_eq!(event.action, "code.merge");
    assert_eq!(event.after.unwrap()["coding_ids"], serde_json::json!([ids[2]]));

    // A merged code cannot be changed or merged again, and nothing merges into itself.
    assert!(p.retire_code(b).is_err());
    assert!(p.merge_codes(b, a).is_err());
    assert!(p.merge_codes(a, a).is_err());
}

#[test]
fn a_code_cannot_be_merged_into_its_own_descendant() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let parent = code(&mut p, "Parent", None);
    let child = code(&mut p, "Child", Some(parent));
    assert!(p.merge_codes(parent, child).is_err());
    assert_eq!(p.get_code(child).unwrap().parent_id, Some(parent), "nothing changed");
}

#[test]
fn splitting_moves_only_the_chosen_codings_to_a_new_sibling() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, _, ids) = coded(&dir);
    let a = p.list_codes().unwrap()[0].id;
    let new = p.split_code(a, &NewCode::new("Access: physical"), &[ids[1]]).unwrap();
    let codings = p.list_codings(source).unwrap();
    assert_eq!(codings.iter().find(|c| c.id == ids[0]).unwrap().code_id, a);
    assert_eq!(codings.iter().find(|c| c.id == ids[1]).unwrap().code_id, new);
    // A coding of another code cannot be moved, and nothing is created when the request is wrong.
    let before = p.list_codes().unwrap().len();
    assert!(p.split_code(a, &NewCode::new("Oops"), &[ids[2]]).is_err());
    assert!(p.split_code(a, &NewCode::new("Oops"), &[]).is_err());
    assert_eq!(p.list_codes().unwrap().len(), before);
}

#[test]
fn a_coding_can_change_code_and_carry_a_rationale() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, _, ids) = coded(&dir);
    let b = p.list_codes().unwrap()[1].id;
    p.change_coding_code(ids[0], b).unwrap();
    p.set_coding_rationale(ids[0], "Closer to a barrier").unwrap();
    let c = p.list_codings(source).unwrap().into_iter().find(|c| c.id == ids[0]).unwrap();
    assert_eq!((c.code_id, c.rationale.as_str()), (b, "Closer to a barrier"));
    p.retire_code(b).unwrap();
    assert!(p.change_coding_code(ids[1], b).is_err(), "cannot change to a retired code");
}

// ---- anchors, snapshots, sets ----

#[test]
fn anchors_snapshots_and_sets() {
    let dir = TempDir::new().unwrap();
    let (mut p, _, _, ids) = coded(&dir);
    let a = p.list_codes().unwrap()[0].id;
    p.set_anchor(ids[0], true).unwrap();
    p.set_anchor(ids[0], true).unwrap();
    assert_eq!(p.list_anchors(a).unwrap(), vec![ids[0]], "marking twice keeps one");
    p.set_anchor(ids[0], false).unwrap();
    assert!(p.list_anchors(a).unwrap().is_empty());

    assert_eq!(p.snapshot_codebook("v1 after pilot").unwrap(), 1);
    p.update_code(a, &CodeEdit { name: Some("Access to care".into()), ..Default::default() }).unwrap();
    assert_eq!(p.snapshot_codebook("v2").unwrap(), 2);
    let versions = p.list_codebook_versions().unwrap();
    assert_eq!(versions.iter().map(|v| (v.version, v.note.as_str(), v.codes)).collect::<Vec<_>>(), [(1, "v1 after pilot", 2), (2, "v2", 2)]);

    let set = p.add_code_set("Pilot codes", "").unwrap();
    assert_eq!(p.add_code_set("Pilot codes", "").unwrap(), set, "same name, same set");
    p.set_code_in_set(set, a, true).unwrap();
    assert!(p.set_code_in_set(set, 999, true).is_err());
    assert!(p.set_code_in_set(999, a, true).is_err());
    assert_eq!(p.list_code_sets().unwrap(), vec![(set, "Pilot codes".to_string(), vec![a])]);
    p.set_code_in_set(set, a, false).unwrap();
    assert!(p.list_code_sets().unwrap()[0].2.is_empty());
}

// ---- memos and links ----

#[test]
fn memos_attach_to_real_things_and_keep_their_history() {
    let dir = TempDir::new().unwrap();
    let (mut p, source, _, ids) = coded(&dir);
    let a = p.list_codes().unwrap()[0].id;
    let memo = |scope, target, kind| NewMemo { kind, scope, target_id: target, phase: None, title: "T".into(), body: "B".into() };

    let on_coding = p.add_memo(&memo(MemoScope::Coding, Some(ids[0]), MemoKind::Analytic)).unwrap();
    let on_code = p.add_memo(&memo(MemoScope::Code, Some(a), MemoKind::MethodDecision)).unwrap();
    p.add_memo(&memo(MemoScope::Project, None, MemoKind::Reflexive)).unwrap();
    p.add_memo(&memo(MemoScope::Source, Some(source), MemoKind::Todo)).unwrap();

    assert!(p.add_memo(&memo(MemoScope::Coding, Some(999), MemoKind::Analytic)).is_err(), "must point at something real");
    assert!(p.add_memo(&memo(MemoScope::Code, None, MemoKind::Analytic)).is_err(), "a code memo needs a code");
    assert!(p.add_memo(&memo(MemoScope::Project, Some(1), MemoKind::Analytic)).is_err(), "a project memo has no target");
    assert!(p.add_memo(&memo(MemoScope::Phase, None, MemoKind::Reflexive)).is_err(), "a phase memo needs a phase");
    let mut phase = memo(MemoScope::Phase, None, MemoKind::Reflexive);
    phase.phase = Some("initial coding".into());
    p.add_memo(&phase).unwrap();

    assert_eq!(p.list_memos(None, None).unwrap().len(), 5);
    assert_eq!(p.list_memos(Some(MemoScope::Code), Some(a)).unwrap().len(), 1);
    assert_eq!(p.list_memos(Some(MemoScope::Coding), None).unwrap()[0].id, on_coding);

    p.update_memo(on_code, Some("New title"), Some("Changed my mind")).unwrap();
    let e = p.events(1).unwrap().remove(0);
    assert_eq!((e.action.as_str(), e.before.unwrap()["body"].as_str().unwrap().to_string()), ("memo.update", "B".to_string()));
    p.delete_memo(on_code).unwrap();
    assert_eq!(p.events(1).unwrap()[0].before.as_ref().unwrap()["body"], "Changed my mind", "a deleted memo's words stay in the log");
    assert!(p.update_memo(on_code, None, Some("x")).is_err());
}

#[test]
fn links_connect_real_things_and_go_away_with_a_deleted_memo() {
    let dir = TempDir::new().unwrap();
    let (mut p, _, _, ids) = coded(&dir);
    let memo = p
        .add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Standalone, target_id: None, phase: None, title: "Idea".into(), body: String::new() })
        .unwrap();
    let link = p.add_link("memo", memo, "coding", ids[0], "supports").unwrap();
    let from_coding = p.list_links("coding", ids[0]).unwrap();
    assert_eq!(from_coding.len(), 1);
    assert_eq!((from_coding[0].label.as_str(), from_coding[0].from_type.as_str()), ("supports", "memo"));
    assert!(p.add_link("memo", memo, "coding", 999, "x").is_err());
    assert!(p.add_link("memo", memo, "banana", 1, "x").is_err());
    p.delete_memo(memo).unwrap();
    assert!(p.list_links("coding", ids[0]).unwrap().is_empty());
    assert!(p.delete_link(link).is_err(), "already gone with the memo");
    assert_eq!(count(&p, "SELECT COUNT(*) FROM link"), 0);
}

#[test]
fn a_listed_coding_says_when_it_was_made() {
    let dir = TempDir::new().unwrap();
    let (p, source, _, _) = coded(&dir);
    let codings = p.list_codings(source).unwrap();
    assert!(!codings.is_empty());
    for c in &codings {
        // A date and a time in the year numbers of this century, as the engine writes them.
        assert!(c.created_at.starts_with("20") && c.created_at.len() >= 19, "created_at is a timestamp: {:?}", c.created_at);
    }
}
