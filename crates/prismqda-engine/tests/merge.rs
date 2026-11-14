//! Merging a copy of a project: what a preview says and changes, what a merge brings in and keeps apart, what it leaves out and says so,
//! that merging twice adds nothing twice, and that the copy itself is never changed.

use std::fs;
use std::path::PathBuf;

use prismqda_engine::{
    CellChange, MergeChoices, ValueKey, VariableSpec, CodeEdit, CoderRole, ImportBatch, ImportedSource, ImportedUnit, MemoKind, MemoScope, NewCode, NewCoder, NewCoding, NewMemo, NewProject, OpenOptions, Project, SourceType, UnitKind,
};
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

/// Adds a document of paragraphs, each a unit, and returns its id.
fn add_document(p: &mut Project, name: &str, paragraphs: &[&str]) -> i64 {
    let mut text = String::new();
    let mut units = Vec::new();
    for para in paragraphs {
        let start = text.chars().count() as i64;
        text.push_str(para);
        units.push(ImportedUnit::new(start, text.chars().count() as i64, UnitKind::Paragraph));
        text.push_str("\n\n");
    }
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: name.into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text,
            runs: vec![],
            units,
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap().source_ids[0]
}

fn code_units(p: &mut Project, source: i64, coder: i64, codes: &[i64]) {
    for (u, code) in p.list_units(source).unwrap().iter().zip(codes) {
        let mut c = NewCoding::new(source, u.start, u.end, *code, coder);
        c.unit_id = Some(u.id);
        p.apply_coding(&c).unwrap();
    }
}

struct W {
    _dir: TempDir,
    ours: Project,
    copy_path: PathBuf,
    cost: i64,
}

/// Ada's project with one document coded by Ada, and a copy of it in which Ben has worked: his codings of the same document, a new code,
/// a new document, a memo, a code that was retired, and a different wording for a code.
fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut ours = Project::create(dir.path().join("ours.pqda"), &NewProject::new("Riders"), &opts()).unwrap();
    let riders = add_document(&mut ours, "Riders", &["Fares are too high for me.", "The stop is far from my house.", "I love the new buses.", "The driver was rude to me."]);
    let cost = ours.add_code(&NewCode::new("Cost")).unwrap();
    let access = ours.add_code(&NewCode::new("Access")).unwrap();
    ours.add_code(&NewCode::new("Old")).unwrap();
    let ada = ours.add_coder(&NewCoder::human("Ada")).unwrap();
    code_units(&mut ours, riders, ada, &[cost, access, access, cost]);

    // The copy: a file made from ours, which Ben then works in.
    let backup = ours.backup_now().unwrap();
    let copy_path = dir.path().join("ben.pqda");
    fs::copy(backup, &copy_path).unwrap();
    {
        let mut theirs = Project::open(&copy_path, &opts()).unwrap();
        let ben = theirs.add_coder(&NewCoder { name: "Ben".into(), role: CoderRole::Human, local_id: Some("ben".into()), provider: None, model_name: None, model_version: None }).unwrap();
        let riders_there = theirs.list_sources().unwrap()[0].id;
        let their_cost = theirs.list_codes().unwrap().into_iter().find(|c| c.name == "Cost").unwrap().id;
        let their_access = theirs.list_codes().unwrap().into_iter().find(|c| c.name == "Access").unwrap().id;
        let their_old = theirs.list_codes().unwrap().into_iter().find(|c| c.name == "Old").unwrap().id;
        // Ben's codings of the shared document: the second paragraph is Old, and Old is then retired.
        code_units(&mut theirs, riders_there, ben, &[their_cost, their_old, their_cost, their_cost]);
        theirs.retire_code(their_old).unwrap();
        // His new code, and a new document coded with it.
        let mut staff = NewCode::new("Staff");
        staff.definition = "People who work there.".into();
        let staff = theirs.add_code(&staff).unwrap();
        let notes = add_document(&mut theirs, "Notes", &["The staff were kind.", "One driver helped me.", "Nobody greeted me."]);
        code_units(&mut theirs, notes, ben, &[staff, staff, staff]);
        // A different wording for Cost, and a memo about the project.
        theirs.update_code(their_cost, &CodeEdit { definition: Some("Talk about money.".into()), ..Default::default() }).unwrap();
        theirs
            .add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Project, target_id: None, phase: None, title: "Ben's thought".into(), body: "Money keeps coming up.".into() })
            .unwrap();
        let _ = their_access;
    }
    W { _dir: dir, ours, copy_path, cost }
}

#[test]
fn a_preview_says_what_a_merge_would_do_and_changes_nothing() {
    let w = world();
    let before = (w.ours.info().unwrap().codings, w.ours.list_coders().unwrap().len(), w.ours.list_codes().unwrap().len(), w.ours.list_sources().unwrap().len());
    let copy_before = fs::read(&w.copy_path).unwrap();
    let report = w.ours.merge_preview(&w.copy_path).unwrap();
    assert!(!report.applied && report.backup.is_none());
    assert_eq!(report.documents_added, ["Notes"]);
    assert_eq!(report.documents_matched, 1);
    assert_eq!(report.coders_added, ["Ben"]);
    assert_eq!(report.codes_added, ["Staff"]);
    assert_eq!(report.codings_added, 4 + 3 - 1, "Ben's seven codings, less the one of the code that was retired");
    assert_eq!(report.codings_skipped.iter().map(|s| (s.reason.as_str(), s.count)).collect::<Vec<_>>(), [("code_retired", 1)]);
    assert_eq!(report.memos_added, 1);
    assert_eq!(report.codes_in_conflict.len(), 1);
    assert_eq!((report.codes_in_conflict[0].path.as_str(), report.codes_in_conflict[0].theirs.as_str()), ("Cost", "Talk about money."));
    let after = (w.ours.info().unwrap().codings, w.ours.list_coders().unwrap().len(), w.ours.list_codes().unwrap().len(), w.ours.list_sources().unwrap().len());
    assert_eq!(after, before, "a preview changes nothing here");
    assert_eq!(fs::read(&w.copy_path).unwrap(), copy_before, "and nothing in the copy");
}

#[test]
fn a_merge_brings_the_copys_work_in_with_its_authors_and_keeps_what_was_here() {
    let mut w = world();
    let report = w.ours.merge_copy(&w.copy_path).unwrap();
    assert!(report.applied);
    assert!(report.backup.as_deref().is_some_and(|b| fs::metadata(b).is_ok()), "a backup was saved first: {:?}", report.backup);
    assert_eq!((report.codings_added, report.memos_added), (6, 1));

    let coders: Vec<String> = w.ours.list_coders().unwrap().into_iter().map(|c| c.name).collect();
    assert!(coders.contains(&"Ada".to_string()) && coders.contains(&"Ben".to_string()));
    let codes: Vec<String> = w.ours.list_codes().unwrap().into_iter().filter(|c| c.status == "active").map(|c| c.name).collect();
    assert!(codes.contains(&"Staff".to_string()));
    let staff = w.ours.list_codes().unwrap().into_iter().find(|c| c.name == "Staff").unwrap();
    assert_eq!(staff.definition, "People who work there.", "a new code comes with its definition");
    assert_eq!(w.ours.list_codes().unwrap().iter().find(|c| c.id == w.cost).unwrap().definition, "", "a code both have keeps this project's wording");

    let sources = w.ours.list_sources().unwrap();
    assert_eq!(sources.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Riders", "Notes"]);
    let notes = sources.iter().find(|s| s.name == "Notes").unwrap();
    assert_eq!((notes.units, notes.codings), (3, 3), "the new document came with its units and its codings");
    let ben = w.ours.list_coders().unwrap().into_iter().find(|c| c.name == "Ben").unwrap().id;
    let riders = sources.iter().find(|s| s.name == "Riders").unwrap();
    let on_riders = w.ours.list_codings(riders.id).unwrap();
    assert_eq!(on_riders.len(), 4 + 3, "Ada's four are still there, and Ben's three are beside them");
    assert!(on_riders.iter().filter(|c| c.coder_id == ben).all(|c| c.unit_id.is_some()), "each of Ben's codings is in a unit of this project");
    let memos = w.ours.list_memos(None, None).unwrap();
    assert!(memos.iter().any(|m| m.title == "Ben's thought"));

    // The log says a merge happened, and Undo will not reach back past it.
    assert!(w.ours.events(10_000).unwrap().iter().any(|e| e.action == "project.merge"));
    assert!(w.ours.undo().is_err(), "a merge is too large to take back one step at a time");
}

#[test]
fn merging_twice_adds_nothing_twice() {
    let mut w = world();
    w.ours.merge_copy(&w.copy_path).unwrap();
    let again = w.ours.merge_copy(&w.copy_path).unwrap();
    assert!(again.documents_added.is_empty() && again.coders_added.is_empty() && again.codes_added.is_empty());
    assert_eq!((again.codings_added, again.areas_added, again.memos_added), (0, 0, 0));
    // Ada's four codings, which the copy was made with, and Ben's six that the first merge brought in.
    assert_eq!((again.documents_matched, again.codings_already_here, again.memos_already_here), (2, 10, 1));
}

#[test]
fn a_document_with_the_same_name_and_other_words_is_left_out_and_named() {
    let mut w = world();
    add_document(&mut w.ours, "Shared", &["Our words."]);
    {
        let mut theirs = Project::open(&w.copy_path, &opts()).unwrap();
        add_document(&mut theirs, "Shared", &["Their different words."]);
    }
    let report = w.ours.merge_preview(&w.copy_path).unwrap();
    assert_eq!(report.documents_in_conflict, ["Shared"]);
    assert!(!report.documents_added.contains(&"Shared".to_string()));
}

#[test]
fn the_project_itself_and_a_missing_file_are_refused_in_words() {
    let mut w = world();
    let own = w.ours.info().unwrap().path;
    assert!(w.ours.merge_copy(&own).unwrap_err().to_string().contains("this project itself"));
    assert!(w.ours.merge_preview(&own).unwrap_err().to_string().contains("this project itself"), "a preview refuses it too");
    assert!(w.ours.merge_preview(&w._dir.path().join("nothing.pqda")).unwrap_err().to_string().contains("not a file"));
}

fn variable(name: &str, level: &str) -> VariableSpec {
    VariableSpec { name: name.into(), level: level.into(), value_labels: Default::default(), missing_codes: Vec::new() }
}

fn value(p: &Project, case: &str, var: &str) -> Option<String> {
    let d = p.case_data().unwrap();
    let r = d.cases.iter().position(|c| c.name == case)?;
    let c = d.variables.iter().position(|v| v.name == var)?;
    d.values[r][c].clone()
}

#[test]
fn a_merge_brings_the_cases_variables_and_values_the_copy_has_and_keeps_the_ones_here() {
    let dir = TempDir::new().unwrap();
    let mut ours = Project::create(dir.path().join("ours.pqda"), &NewProject::new("Study"), &opts()).unwrap();
    let talk = add_document(&mut ours, "Talk", &["One.", "Two."]);
    let ana = ours.add_case("Ana", "").unwrap().id;
    ours.assign_source_to_case(ana, talk).unwrap();
    ours.define_variable(&variable("Age", "ratio"), None).unwrap();
    ours.define_variable(&variable("Site", "nominal"), None).unwrap();
    ours.set_values("Age", &[CellChange { case_id: ana, value: Some("30".into()) }]).unwrap();
    ours.set_values("Site", &[CellChange { case_id: ana, value: Some("North".into()) }]).unwrap();

    // the copy, made from ours; then each side defines a variable named Rank, at another level
    let backup = ours.backup_now().unwrap();
    let copy_path = dir.path().join("copy.pqda");
    fs::copy(backup, &copy_path).unwrap();
    ours.define_variable(&variable("Rank", "ordinal"), None).unwrap();
    {
        let mut theirs = Project::open(&copy_path, &opts()).unwrap();
        let notes = add_document(&mut theirs, "Notes", &["Three."]);
        let their_ana = theirs.case_data().unwrap().cases[0].id;
        let bo = theirs.add_case("Bo", "The second respondent").unwrap().id;
        theirs.assign_source_to_case(bo, notes).unwrap();
        theirs.define_variable(&variable("Wave", "ratio"), None).unwrap();
        theirs.define_variable(&variable("Rank", "nominal"), None).unwrap();
        theirs.set_values("Wave", &[CellChange { case_id: their_ana, value: Some("2".into()) }, CellChange { case_id: bo, value: Some("3".into()) }]).unwrap();
        theirs.set_values("Age", &[CellChange { case_id: their_ana, value: Some("31".into()) }, CellChange { case_id: bo, value: Some("40".into()) }]).unwrap();
        theirs.set_values("Site", &[CellChange { case_id: bo, value: Some("South".into()) }]).unwrap();
        theirs.set_values("Rank", &[CellChange { case_id: bo, value: Some("high".into()) }]).unwrap();
    }

    let preview = ours.merge_preview(&copy_path).unwrap();
    assert_eq!(preview.cases_added, ["Bo"]);
    assert_eq!(ours.case_data().unwrap().cases.len(), 1, "a preview adds no case here");

    let report = ours.merge_copy(&copy_path).unwrap();
    assert_eq!(report.cases_added, ["Bo"], "the case the copy has and this project lacks");
    assert_eq!(report.cases_matched, 1, "Ana is the same case in both");
    assert_eq!(report.case_members_added, 1, "Bo holds Notes, which came with the merge; Ana already held Talk");
    assert_eq!(report.variables_added, ["Wave"]);
    assert_eq!(report.variables_in_conflict, ["Rank"], "the same name at another level is not the same variable");
    assert_eq!(report.values_added, 4, "Wave for both, Bo's Age, and Bo's Site");
    assert_eq!(report.values_in_conflict.len(), 1);
    let c = &report.values_in_conflict[0];
    assert_eq!((c.case.as_str(), c.variable.as_str(), c.ours.as_str(), c.theirs.as_str()), ("Ana", "Age", "30", "31"));

    assert_eq!(value(&ours, "Ana", "Age").as_deref(), Some("30"), "the value that was here is kept");
    assert_eq!(value(&ours, "Ana", "Site").as_deref(), Some("North"));
    assert_eq!(value(&ours, "Ana", "Wave").as_deref(), Some("2"));
    assert_eq!(value(&ours, "Bo", "Age").as_deref(), Some("40"));
    assert_eq!(value(&ours, "Bo", "Site").as_deref(), Some("South"));
    assert_eq!(value(&ours, "Bo", "Rank"), None, "the values of a variable at another level are not brought");
    let cases = ours.list_cases().unwrap();
    let bo = cases.iter().find(|c| c.name == "Bo").unwrap();
    assert_eq!(bo.notes, "The second respondent");
    let notes_here = ours.list_sources().unwrap().into_iter().find(|s| s.name == "Notes").unwrap().id;
    assert_eq!(bo.source_ids, vec![notes_here], "Bo holds the document that came with the merge");

    // again: nothing is added twice
    let again = ours.merge_copy(&copy_path).unwrap();
    assert!(again.cases_added.is_empty() && again.variables_added.is_empty());
    assert_eq!((again.values_added, again.case_members_added, again.cases_matched), (0, 0, 2));
}

#[test]
fn a_difference_can_be_settled_in_favour_of_the_copy_and_the_rest_keep_what_is_here() {
    let dir = TempDir::new().unwrap();
    let mut ours = Project::create(dir.path().join("ours.pqda"), &NewProject::new("Study"), &opts()).unwrap();
    let talk = add_document(&mut ours, "Talk", &["One.", "Two."]);
    let ana = ours.add_case("Ana", "").unwrap().id;
    let bo = ours.add_case("Bo", "").unwrap().id;
    ours.assign_source_to_case(ana, talk).unwrap();
    ours.define_variable(&variable("Age", "ratio"), None).unwrap();
    ours.define_variable(&variable("Site", "nominal"), None).unwrap();
    for (case, age, site) in [(ana, "30", "North"), (bo, "40", "East")] {
        ours.set_values("Age", &[CellChange { case_id: case, value: Some(age.into()) }]).unwrap();
        ours.set_values("Site", &[CellChange { case_id: case, value: Some(site.into()) }]).unwrap();
    }
    let mut cost = NewCode::new("Cost");
    cost.definition = "Money.".into();
    ours.add_code(&cost).unwrap();
    let mut access = NewCode::new("Access");
    access.definition = "Getting there.".into();
    ours.add_code(&access).unwrap();

    let backup = ours.backup_now().unwrap();
    let copy_path = dir.path().join("copy.pqda");
    fs::copy(backup, &copy_path).unwrap();
    {
        let mut theirs = Project::open(&copy_path, &opts()).unwrap();
        let d = theirs.case_data().unwrap();
        let (their_ana, their_bo) = (d.cases[0].id, d.cases[1].id);
        theirs.set_values("Age", &[CellChange { case_id: their_ana, value: Some("31".into()) }, CellChange { case_id: their_bo, value: Some("41".into()) }]).unwrap();
        theirs.set_values("Site", &[CellChange { case_id: their_ana, value: Some("South".into()) }]).unwrap();
        for (name, definition) in [("Cost", "Talk about money."), ("Access", "Reaching a place.")] {
            let id = theirs.list_codes().unwrap().into_iter().find(|c| c.name == name).unwrap().id;
            theirs.update_code(id, &CodeEdit { definition: Some(definition.into()), ..Default::default() }).unwrap();
        }
    }

    // Without a choice, everything that differs keeps what is here and is listed.
    let plain = ours.merge_preview(&copy_path).unwrap();
    assert_eq!((plain.values_in_conflict.len(), plain.codes_in_conflict.len(), plain.values_replaced), (3, 2, 0));

    // With choices, a preview says what would change and changes nothing.
    let choices = MergeChoices {
        values_from_copy: vec![ValueKey { case: "ana".into(), variable: "AGE".into() }, ValueKey { case: "Ana".into(), variable: "Site".into() }],
        definitions_from_copy: vec!["cost".into()],
    };
    let preview = ours.merge_preview_choosing(&copy_path, None, &choices).unwrap();
    assert_eq!(preview.values_replaced, 2, "two values would take the copy's, whatever the case of the names");
    assert_eq!(preview.definitions_replaced, ["Cost"]);
    assert_eq!(preview.values_in_conflict.len(), 1, "Bo's age is still a difference, and is kept");
    assert_eq!(preview.codes_in_conflict.len(), 1, "and so is the wording of Access");
    assert_eq!(value(&ours, "Ana", "Age").as_deref(), Some("30"), "a preview changes nothing");

    let report = ours.merge_copy_choosing(&copy_path, None, &choices).unwrap();
    assert!(report.applied);
    assert_eq!((report.values_replaced, report.values_added), (2, 0), "replacing is not adding");
    assert_eq!(value(&ours, "Ana", "Age").as_deref(), Some("31"));
    assert_eq!(value(&ours, "Ana", "Site").as_deref(), Some("South"));
    assert_eq!(value(&ours, "Bo", "Age").as_deref(), Some("40"), "what was not chosen is what was here");
    let def = |name: &str| ours.list_codes().unwrap().into_iter().find(|c| c.name == name).unwrap().definition;
    assert_eq!(def("Cost"), "Talk about money.");
    assert_eq!(def("Access"), "Getting there.");
}
