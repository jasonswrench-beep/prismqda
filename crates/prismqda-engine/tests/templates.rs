//! Following a method: choosing it, working through its phases, and checking the codebook against its rules.

use prismqda_engine::{CodeEdit, CodingMode, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SourceType};
use tempfile::TempDir;

fn project(dir: &TempDir, mode: CodingMode) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut np = NewProject::new("P");
    np.coding_mode = mode;
    Project::create(dir.path().join("p.pqda"), &np, &o).unwrap()
}

#[test]
fn a_new_project_follows_free_mode_with_no_phases() {
    let dir = TempDir::new().unwrap();
    let p = project(&dir, CodingMode::FreeSpan);
    let s = p.template_state().unwrap();
    assert_eq!(s.template.id, "free");
    assert!(s.phases.is_empty() && s.current_phase.is_none());
    assert!(p.check_codebook().unwrap().is_empty());
}

#[test]
fn choosing_a_method_starts_in_its_first_phase_and_progress_is_remembered() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::FreeSpan);
    let s = p.set_template("reflexive_ta", false).unwrap();
    assert_eq!(s.current_phase.as_deref(), Some("familiarization"));
    assert_eq!(s.phases[0].status, "current");
    assert!(s.phases.iter().all(|ph| ph.items.iter().all(|i| !i.done)));

    p.tick_item("familiarization", 0, true).unwrap();
    p.tick_item("familiarization", 1, true).unwrap();
    p.tick_item("familiarization", 2, true).unwrap();
    p.move_to_phase("initial_coding").unwrap();
    let s = p.template_state().unwrap();
    assert_eq!(s.current_phase.as_deref(), Some("initial_coding"));
    assert_eq!((s.phases[0].status.as_str(), s.phases[1].status.as_str(), s.phases[2].status.as_str()), ("done", "current", "open"));

    // Phases can be revisited, and unticking an item reopens a phase.
    p.move_to_phase("familiarization").unwrap();
    p.tick_item("familiarization", 1, false).unwrap();
    let s = p.template_state().unwrap();
    assert_eq!(s.current_phase.as_deref(), Some("familiarization"));
    assert_eq!(s.phases[0].status, "current");
    assert_eq!(s.phases[0].items.iter().map(|i| i.done).collect::<Vec<_>>(), [true, false, true]);

    // It all survives closing and reopening the project, and each step is in the audit trail.
    let path = p.path().to_path_buf();
    let actions: Vec<String> = p.events(100).unwrap().into_iter().map(|e| e.action).collect();
    for want in ["template.set", "phase.tick", "phase.move"] {
        assert!(actions.iter().any(|a| a == want), "{want} in {actions:?}");
    }
    drop(p);
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let again = Project::open(&path, &o).unwrap().template_state().unwrap();
    assert_eq!(again.current_phase.as_deref(), Some("familiarization"));
    assert_eq!(again.phases[0].items.iter().filter(|i| i.done).count(), 2);
}

#[test]
fn bad_phase_and_item_names_are_refused() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::FreeSpan);
    p.set_template("reflexive_ta", false).unwrap();
    assert!(p.move_to_phase("nope").unwrap_err().to_string().contains("no phase"));
    assert!(p.tick_item("nope", 0, true).is_err());
    assert!(p.tick_item("familiarization", 99, true).unwrap_err().to_string().contains("only 3"));
    assert!(p.set_template("not_a_method", false).is_err());
}

#[test]
fn undo_does_not_trip_over_phase_ticks() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::FreeSpan);
    p.set_template("reflexive_ta", false).unwrap();
    let c = p.add_code(&NewCode::new("C")).unwrap();
    p.tick_item("familiarization", 0, true).unwrap();
    p.move_to_phase("initial_coding").unwrap();
    assert_eq!(p.undo().unwrap().unwrap().action, "code.add", "ticks and moves are not part of the undo history");
    assert!(p.list_codes().unwrap().iter().all(|x| x.id != c));
}

#[test]
fn a_method_that_codes_units_switches_an_empty_project_to_unit_mode() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::FreeSpan);
    p.set_template("content_analysis", false).unwrap();
    assert_eq!(p.info().unwrap().coding_mode, "unit");
    assert_eq!(p.info().unwrap().method_template, "content_analysis");
}

#[test]
fn a_method_that_codes_units_is_refused_once_free_spans_exist() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::FreeSpan);
    let source = p.add_source(&NewSource::new("S", SourceType::Document, "some words to code")).unwrap();
    let code = p.add_code(&NewCode::new("C")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_coding(&NewCoding::new(source, 0, 4, code, coder)).unwrap();
    let err = p.set_template("content_analysis", false).unwrap_err().to_string();
    assert!(err.contains("free-span codings"), "{err}");
    assert_eq!(p.info().unwrap().method_template, "free", "nothing changed");
    assert_eq!(p.info().unwrap().coding_mode, "free_span");
}

#[test]
fn content_analysis_wants_a_full_definition_and_an_anchor_for_every_code() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::Unit);
    p.set_template("content_analysis", false).unwrap();
    let source = p.add_source(&NewSource::new("S", SourceType::Survey, "one two three")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let a = p.add_code(&NewCode::new("Access")).unwrap();
    let unit = p.add_unit(source, prismqda_engine::UnitKind::SurveyResponse, 0, 13, None).unwrap();

    let problems = |p: &Project| -> Vec<String> { p.check_codebook().unwrap().into_iter().map(|i| i.problem).collect() };
    assert_eq!(problems(&p), ["has no definition", "has no inclusion criteria", "has no exclusion criteria", "has no anchor example"]);

    p.update_code(
        a,
        &CodeEdit {
            definition: Some("Getting through the door".into()),
            inclusion_criteria: Some("Any mention of entry".into()),
            exclusion_criteria: Some("Cost alone".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(problems(&p), ["has no anchor example"]);
    let mut c = NewCoding::new(source, 0, 13, a, coder);
    c.unit_id = Some(unit);
    let coding = p.apply_coding(&c).unwrap();
    p.set_anchor(coding, true).unwrap();
    assert!(problems(&p).is_empty());

    // A retired code, or a theme that only groups others, is not held to the rules.
    p.add_code(&NewCode { theme: true, ..NewCode::new("A group") }).unwrap();
    assert!(problems(&p).is_empty());
}

#[test]
fn free_and_reflexive_methods_ask_nothing_of_the_codebook() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::FreeSpan);
    p.add_code(&NewCode::new("Bare code")).unwrap();
    p.set_template("reflexive_ta", false).unwrap();
    assert!(p.check_codebook().unwrap().is_empty());
}

#[test]
fn a_method_with_its_own_codes_adds_them_once() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir, CodingMode::FreeSpan);
    p.add_code(&NewCode::new("coda")).unwrap();
    p.set_template("narrative", true).unwrap();
    let names: Vec<String> = p.list_codes().unwrap().into_iter().map(|c| c.name).collect();
    assert_eq!(names.len(), 6, "five new structural codes, and the existing 'coda' is not duplicated: {names:?}");
    assert!(names.contains(&"Complicating action".to_string()));
    let defs: Vec<String> = p.list_codes().unwrap().into_iter().filter(|c| c.name == "Evaluation").map(|c| c.definition).collect();
    assert!(defs[0].contains("worth telling"));
    p.set_template("narrative", true).unwrap();
    assert_eq!(p.list_codes().unwrap().len(), 6, "choosing it again adds nothing");
}
