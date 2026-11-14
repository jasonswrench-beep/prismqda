//! Cases made by hand: adding one, putting sources in it and taking them out, and undoing each step.

use prismqda_engine::{
    CaseMember, Error, ImportBatch, ImportedCase, ImportedSource, ImportedUnit, NewProject, NewSource, OpenOptions, Project, SourceType,
    UnitKind,
};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

struct W {
    p: Project,
    a: i64,
    b: i64,
    c: i64,
}

fn world(dir: &TempDir) -> W {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let a = p.add_source(&NewSource::new("A", SourceType::Document, "alpha text")).unwrap();
    let b = p.add_source(&NewSource::new("B", SourceType::Document, "bravo text")).unwrap();
    let c = p.add_source(&NewSource::new("C", SourceType::Document, "charlie text")).unwrap();
    W { p, a, b, c }
}

fn raw(p: &Project) -> Connection {
    Connection::open(p.path()).unwrap()
}

fn members(p: &Project, case: i64) -> i64 {
    raw(p).query_row("SELECT COUNT(*) FROM case_member WHERE case_id = ?1", [case], |r| r.get(0)).unwrap()
}

fn actions(p: &Project) -> Vec<String> {
    p.events(1000).unwrap().into_iter().map(|e| e.action).collect()
}

#[test]
fn adding_a_case_returns_its_row_and_logs_it() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("  Urban Residents ", "Lives in a city of 50,000 or more").unwrap();
    assert_eq!(case.name, "Urban Residents", "the name is trimmed");
    assert_eq!(case.notes, "Lives in a city of 50,000 or more");
    assert_eq!((case.sources, case.units), (0, 0));
    assert!(case.source_ids.is_empty());
    assert_eq!(w.p.list_cases().unwrap(), vec![case.clone()]);

    let event = w.p.events(1).unwrap().remove(0);
    assert_eq!(event.action, "case.add");
    assert_eq!((event.target_type.as_deref(), event.target_id), (Some("case"), Some(case.id)));
    assert_eq!(event.before, None);
    assert_eq!(event.after, Some(json!({ "name": "Urban Residents", "notes": "Lives in a city of 50,000 or more" })));
}

#[test]
fn notes_are_optional() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    assert_eq!(w.p.add_case("Rural", "").unwrap().notes, "");
}

#[test]
fn a_case_needs_a_name() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    for blank in ["", "   ", "\t\n"] {
        assert!(matches!(w.p.add_case(blank, "").err().unwrap(), Error::Invalid(_)));
    }
    assert!(w.p.list_cases().unwrap().is_empty());
}

#[test]
fn two_cases_cannot_share_a_name_ignoring_capitals() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    w.p.add_case("Urban Residents", "").unwrap();
    for clash in ["urban residents", "URBAN RESIDENTS", " Urban Residents "] {
        let e = w.p.add_case(clash, "").err().unwrap();
        assert!(matches!(e, Error::Invalid(_)), "{clash}: {e:?}");
        assert!(e.to_string().contains("Urban Residents"), "the message names the case: {e}");
    }
    // Capitals beyond ASCII count too.
    w.p.add_case("Élan", "").unwrap();
    assert!(w.p.add_case("élan", "").is_err());
    assert_eq!(w.p.list_cases().unwrap().len(), 2, "refusals add nothing");
    assert_eq!(actions(&w.p).iter().filter(|a| *a == "case.add").count(), 2);
}

#[test]
fn assigning_a_source_puts_it_in_the_cases_source_ids() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    w.p.assign_source_to_case(case.id, w.c).unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();

    let listed = &w.p.list_cases().unwrap()[0];
    assert_eq!(listed.source_ids, vec![w.a, w.c], "source ids, smallest first");
    assert_eq!((listed.sources, listed.units), (2, 0));

    let events: Vec<_> = w.p.events(1000).unwrap().into_iter().filter(|e| e.action == "case.assign").collect();
    assert_eq!(events.len(), 2);
    assert_eq!((events[0].target_type.as_deref(), events[0].target_id), (Some("case"), Some(case.id)));
    assert_eq!(events[0].before, Some(json!({ "source_id": w.c, "member": false })));
    assert_eq!(events[0].after, Some(json!({ "source_id": w.c, "member": true })));
}

#[test]
fn assigning_twice_is_harmless() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();
    assert_eq!(members(&w.p, case.id), 1, "no second row");
    assert_eq!(actions(&w.p).iter().filter(|a| *a == "case.assign").count(), 1, "and no second event");
    assert_eq!(w.p.list_cases().unwrap()[0].source_ids, vec![w.a]);
}

#[test]
fn a_source_can_belong_to_several_cases() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let one = w.p.add_case("One", "").unwrap();
    let two = w.p.add_case("Two", "").unwrap();
    w.p.assign_source_to_case(one.id, w.a).unwrap();
    w.p.assign_source_to_case(two.id, w.a).unwrap();
    let cases = w.p.list_cases().unwrap();
    assert_eq!(cases[0].source_ids, vec![w.a]);
    assert_eq!(cases[1].source_ids, vec![w.a]);
}

#[test]
fn unknown_cases_and_sources_are_not_found() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    assert!(matches!(w.p.assign_source_to_case(99, w.a).err().unwrap(), Error::NotFound { what: "case", id: 99 }));
    assert!(matches!(w.p.assign_source_to_case(case.id, 99).err().unwrap(), Error::NotFound { what: "source", id: 99 }));
    assert!(matches!(w.p.unassign_source_from_case(99, w.a).err().unwrap(), Error::NotFound { what: "case", id: 99 }));
    assert!(matches!(w.p.unassign_source_from_case(case.id, 99).err().unwrap(), Error::NotFound { what: "source", id: 99 }));
    assert_eq!(members(&w.p, case.id), 0);
}

#[test]
fn unassigning_takes_the_source_out_and_logs_it() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();
    w.p.assign_source_to_case(case.id, w.b).unwrap();
    w.p.unassign_source_from_case(case.id, w.a).unwrap();
    assert_eq!(w.p.list_cases().unwrap()[0].source_ids, vec![w.b]);

    let event = w.p.events(1).unwrap().remove(0);
    assert_eq!(event.action, "case.unassign");
    assert_eq!((event.target_type.as_deref(), event.target_id), (Some("case"), Some(case.id)));
    assert_eq!(event.before, Some(json!({ "source_id": w.a, "member": true })));
    assert_eq!(event.after, Some(json!({ "source_id": w.a, "member": false })));
}

#[test]
fn unassigning_a_source_that_is_not_in_the_case_is_harmless() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();
    w.p.unassign_source_from_case(case.id, w.b).unwrap();
    assert_eq!(w.p.list_cases().unwrap()[0].source_ids, vec![w.a]);
    assert!(!actions(&w.p).contains(&"case.unassign".to_string()), "nothing changed, so nothing is logged");
}

#[test]
fn unit_only_members_count_as_units_but_are_not_in_source_ids() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();
    let unit = w.p.add_unit(w.b, UnitKind::Sentence, 0, 5, None).unwrap();
    // Units are put in cases by the survey importer; do the same here directly.
    raw(&w.p).execute("INSERT INTO case_member (case_id, unit_id) VALUES (?1, ?2)", [case.id, unit]).unwrap();

    let listed = &w.p.list_cases().unwrap()[0];
    assert_eq!(listed.source_ids, vec![w.a], "the unit's source is not a member");
    assert_eq!((listed.sources, listed.units), (1, 1));

    // Taking the source out leaves the unit member alone.
    w.p.unassign_source_from_case(case.id, w.a).unwrap();
    let listed = &w.p.list_cases().unwrap()[0];
    assert!(listed.source_ids.is_empty());
    assert_eq!((listed.sources, listed.units), (0, 1));
}

#[test]
fn undo_takes_a_source_back_out_and_puts_it_back_in() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();
    w.p.assign_source_to_case(case.id, w.b).unwrap();
    w.p.unassign_source_from_case(case.id, w.a).unwrap();

    assert_eq!(w.p.next_undo().unwrap().as_deref(), Some("Put the source back in the case"));
    let out = w.p.undo().unwrap().unwrap();
    assert_eq!((out.action.as_str(), out.what.as_str()), ("case.unassign", "Put the source back in the case"));
    assert_eq!(w.p.list_cases().unwrap()[0].source_ids, vec![w.a, w.b]);

    assert_eq!(w.p.next_undo().unwrap().as_deref(), Some("Took the source out of the case"));
    let out = w.p.undo().unwrap().unwrap();
    assert_eq!((out.action.as_str(), out.what.as_str()), ("case.assign", "Took the source out of the case"));
    assert_eq!(w.p.list_cases().unwrap()[0].source_ids, vec![w.a]);
    assert_eq!(members(&w.p, case.id), 1, "exactly one row, never a duplicate");

    // Each undo is one event of its own, and an undo is not logged as an assign or unassign.
    let log = actions(&w.p);
    assert_eq!(log.iter().filter(|a| *a == "undo").count(), 2);
    assert_eq!(log.iter().filter(|a| *a == "case.assign").count(), 2);
    assert_eq!(log.iter().filter(|a| *a == "case.unassign").count(), 1);
}

#[test]
fn undo_removes_a_case_that_nothing_was_added_to() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    assert_eq!(w.p.next_undo().unwrap().as_deref(), Some("Removed the case you added"));
    let out = w.p.undo().unwrap().unwrap();
    assert_eq!((out.action.as_str(), out.what.as_str()), ("case.add", "Removed the case you added"));
    assert!(w.p.list_cases().unwrap().is_empty());

    // The name is free again.
    w.p.add_case("Urban Residents", "").unwrap();
    let _ = case;
}

#[test]
fn undoing_an_assign_then_the_add_empties_the_case_step_by_step() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "").unwrap();
    w.p.assign_source_to_case(case.id, w.a).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "case.assign");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "case.add");
    assert!(w.p.list_cases().unwrap().is_empty());
}

#[test]
fn undo_will_not_remove_a_case_that_has_since_been_used() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);

    // A source put in by some other route (a case member that this session did not log).
    let with_member = w.p.add_case("With a source", "").unwrap();
    raw(&w.p).execute("INSERT INTO case_member (case_id, source_id) VALUES (?1, ?2)", [with_member.id, w.a]).unwrap();
    let e = w.p.undo().expect_err("must refuse");
    assert!(matches!(e, Error::Invalid(_)), "{e:?}");
    assert!(e.to_string().contains("cannot be removed by undo"), "{e}");
    assert_eq!(w.p.list_cases().unwrap().len(), 1, "the case is still there");
    assert_eq!(w.p.list_cases().unwrap()[0].source_ids, vec![w.a], "and so is its source");
    raw(&w.p).execute("DELETE FROM case_member", []).unwrap();
    w.p.undo().unwrap().unwrap();

    // A memo written about the case.
    let with_memo = w.p.add_case("With a memo", "").unwrap();
    raw(&w.p)
        .execute(
            "INSERT INTO memo (kind, scope, target_id, created_at, updated_at) VALUES ('analytic', 'case', ?1, 'x', 'x')",
            [with_memo.id],
        )
        .unwrap();
    assert!(matches!(w.p.undo().err().unwrap(), Error::Invalid(_)));
    assert_eq!(w.p.list_cases().unwrap().len(), 1);
    raw(&w.p).execute("DELETE FROM memo", []).unwrap();
    w.p.undo().unwrap().unwrap();

    // A value of a case attribute.
    let with_value = w.p.add_case("With an attribute", "").unwrap();
    {
        let c = raw(&w.p);
        c.execute("INSERT INTO attribute (name, applies_to, level) VALUES ('Age', 'case', 'ratio')", []).unwrap();
        c.execute("INSERT INTO attribute_value (attribute_id, target_id, value) VALUES (1, ?1, '34')", [with_value.id]).unwrap();
    }
    assert!(matches!(w.p.undo().err().unwrap(), Error::Invalid(_)));
    assert_eq!(w.p.list_cases().unwrap().len(), 1);
}

#[test]
fn imported_cases_list_their_sources_and_can_be_added_to() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let thread = ImportedSource {
        name: "Thread".to_string(),
        source_type: SourceType::Social,
        folder: String::new(),
        text: "First post\n\nA reply".to_string(),
        runs: vec![],
        units: vec![ImportedUnit::new(0, 10, UnitKind::Comment), ImportedUnit::new(12, 19, UnitKind::Comment)],
        default_unit_kind: UnitKind::Comment,
        unit_attributes: vec![],
        original_file_hash: None,
        settings: serde_json::Value::Null,
        warnings: vec![],
    };
    let batch = ImportBatch {
        sources: vec![thread],
        cases: vec![ImportedCase {
            name: "Video A".into(),
            attrs: vec![],
            members: vec![CaseMember { source: 0, unit: None }, CaseMember { source: 0, unit: Some(1) }],
        }],
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![],
    };
    let out = w.p.apply_import(&batch).unwrap();
    let thread_id = out.source_ids[0];
    let imported = w.p.list_cases().unwrap().remove(0);
    assert_eq!(imported.name, "Video A");
    assert_eq!(imported.notes, "");
    assert_eq!(imported.source_ids, vec![thread_id]);
    assert_eq!((imported.sources, imported.units), (1, 1));

    w.p.assign_source_to_case(imported.id, w.a).unwrap();
    let mut want = vec![w.a, thread_id];
    want.sort();
    assert_eq!(w.p.list_cases().unwrap()[0].source_ids, want);
    assert!(w.p.add_case("video a", "").is_err(), "an imported case's name is taken too");
}

#[test]
fn the_cases_row_serializes_with_every_field() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let case = w.p.add_case("Urban Residents", "notes here").unwrap();
    w.p.assign_source_to_case(case.id, w.b).unwrap();
    let v = serde_json::to_value(&w.p.list_cases().unwrap()[0]).unwrap();
    assert_eq!(v, json!({ "id": case.id, "name": "Urban Residents", "notes": "notes here", "sources": 1, "units": 0, "source_ids": [w.b] }));
}

#[test]
fn a_read_only_project_refuses_every_case_change() {
    let dir = TempDir::new().unwrap();
    let path = {
        let mut w = world(&dir);
        w.p.add_case("Urban Residents", "").unwrap();
        w.p.path().to_path_buf()
    };
    raw_set_version(&path, prismqda_engine::schema::SCHEMA_VERSION + 1);
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::open(&path, &o).unwrap();
    assert!(p.is_read_only());
    assert!(matches!(p.add_case("Other", "").err().unwrap(), Error::ReadOnly(_)));
    assert!(matches!(p.assign_source_to_case(1, 1).err().unwrap(), Error::ReadOnly(_)));
    assert!(matches!(p.unassign_source_from_case(1, 1).err().unwrap(), Error::ReadOnly(_)));
    assert_eq!(p.list_cases().unwrap().len(), 1, "reading still works");
}

#[test]
fn listing_thousands_of_cases_reads_the_member_table_once() {
    // A survey with one case per respondent makes thousands of cases. Counting each case's members with a lookup per
    // case read the whole member table once for every case, which took half a minute at fifteen thousand cases and
    // held up opening the project.
    let dir = TempDir::new().unwrap();
    let w = world(&dir);
    {
        let mut c = raw(&w.p);
        let tx = c.transaction().unwrap();
        for i in 0..8000i64 {
            tx.execute("INSERT INTO research_case (id, name, notes) VALUES (?1, ?2, '')", rusqlite::params![i + 1, format!("Respondent {i}")]).unwrap();
            tx.execute("INSERT INTO case_member (case_id, source_id) VALUES (?1, ?2)", rusqlite::params![i + 1, w.a]).unwrap();
        }
        tx.commit().unwrap();
    }
    let started = std::time::Instant::now();
    let cases = w.p.list_cases().unwrap();
    assert!(started.elapsed().as_secs() < 5, "listing 8,000 cases took {:?}", started.elapsed());
    assert_eq!(cases.len(), 8000);
    assert!(cases.iter().all(|c| c.sources == 1 && c.units == 0 && c.source_ids == vec![w.a]), "every case holds the one source, counted once");
}

fn raw_set_version(path: &std::path::Path, v: i32) {
    Connection::open(path).unwrap().pragma_update(None, "user_version", v).unwrap();
}
