//! An assignment's settings travel in each student's packet, and the answer key is a file of its own (decision 200).

use std::fs;

use prismqda_engine::{Assignment, ClassPacketsSpec, Error, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, SourceType, UnitKind};
use tempfile::TempDir;

fn options() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

const RIDERS: [&str; 3] = ["Fares are too high for me.", "The stop is far from my house.", "I love the new buses."];

fn batch(name: &str, paragraphs: &[&str]) -> ImportBatch {
    let mut text = String::new();
    let mut units = Vec::new();
    for para in paragraphs {
        let start = text.chars().count() as i64;
        text.push_str(para);
        units.push(ImportedUnit::new(start, text.chars().count() as i64, UnitKind::Paragraph));
        text.push_str("\n\n");
    }
    ImportBatch {
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
    }
}

fn code(p: &mut Project, source: i64, which: usize, name: &str, coder: i64) {
    let units = p.list_units(source).unwrap();
    let id = p.list_codes().unwrap().iter().find(|c| c.name == name).unwrap().id;
    let mut c = NewCoding::new(source, units[which].start, units[which].end, id, coder);
    c.unit_id = Some(units[which].id);
    p.apply_coding(&c).unwrap();
}

struct W {
    p: Project,
    dir: TempDir,
    source: i64,
    teacher: i64,
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("class.pqda"), &NewProject::new("Class"), &options()).unwrap();
    let source = p.apply_import(&batch("Riders", &RIDERS)).unwrap().source_ids[0];
    p.add_code(&NewCode::new("Cost")).unwrap();
    p.add_code(&NewCode::new("Access")).unwrap();
    let teacher = p.add_coder(&NewCoder::human("Teacher")).unwrap();
    code(&mut p, source, 0, "Cost", teacher);
    code(&mut p, source, 1, "Access", teacher);
    code(&mut p, source, 2, "Cost", teacher);
    W { p, dir, source, teacher }
}

fn spec(students: &[&str], assignment: Option<Assignment>) -> ClassPacketsSpec {
    ClassPacketsSpec { students: students.iter().map(|s| s.to_string()).collect(), source_ids: vec![], instructions: "Code each paragraph.".into(), assignment }
}

#[test]
fn an_assignments_settings_are_in_each_packet_and_ai_stays_closed_there() {
    let w = world();
    let folder = w.dir.path().join("packets");
    let settings = Assignment { title: "  Week   3  ".into(), ai_allowed: false, coach_locked: true, closed: vec!["Explore".into(), "stats".into(), "explore".into()] };
    let report = w.p.make_class_packets(&folder, "Exercise", &spec(&["Annika"], Some(settings))).unwrap();
    assert_eq!(report.made, 1);
    let mut packet = Project::open(folder.join("Exercise - packet for Annika.pqda"), &options()).unwrap();
    let info = packet.coder_packet_info().unwrap().unwrap();
    assert_eq!(info.assignment, Some(Assignment { title: "Week 3".into(), ai_allowed: false, coach_locked: true, closed: vec!["explore".into(), "stats".into()] }), "tidied, a page named twice once");
    assert_eq!(packet.info().unwrap().ai_mode, "off", "AI is closed in the packet");
    let refused = packet.set_ai_mode("packet");
    assert!(matches!(refused, Err(Error::Invalid(ref m)) if m.contains("instructor")), "{refused:?}");
    assert!(packet.set_ai_mode("off").is_ok());
    let memos = packet.list_memos(None, None).unwrap();
    let text = memos.iter().map(|m| m.body.clone()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("AI prompt packets are turned off") && text.contains("Team > Coach") && text.contains("explore, stats"), "the student is told what was settled: {text}");
}

#[test]
fn a_packet_made_without_settings_has_none_and_keeps_the_senders_ai_choice() {
    let w = world();
    let folder = w.dir.path().join("plain");
    w.p.make_class_packets(&folder, "Exercise", &spec(&["Bohdan"], None)).unwrap();
    let mut packet = Project::open(folder.join("Exercise - packet for Bohdan.pqda"), &options()).unwrap();
    assert!(packet.coder_packet_info().unwrap().unwrap().assignment.is_none());
    assert_eq!(packet.info().unwrap().ai_mode, "packet");
    packet.set_ai_mode("off").unwrap();
    packet.set_ai_mode("packet").unwrap();
}

#[test]
fn settings_that_make_no_sense_are_refused_before_any_packet_is_made() {
    let w = world();
    let folder = w.dir.path().join("refused");
    for (closed, locked) in [(vec!["code"], false), (vec!["settings"], false), (vec!["nonsense"], false), (vec!["team"], true)] {
        let a = Assignment { title: String::new(), ai_allowed: true, coach_locked: locked, closed: closed.iter().map(|s| s.to_string()).collect() };
        let r = w.p.make_class_packets(&folder, "Exercise", &spec(&["Cyrus"], Some(a)));
        assert!(matches!(r, Err(Error::Invalid(_))), "{closed:?}: {r:?}");
    }
    assert!(!folder.exists() || fs::read_dir(&folder).unwrap().count() == 0, "no packet was made");
    let long = Assignment { title: "x".repeat(101), ..Default::default() };
    assert!(matches!(w.p.make_class_packets(&folder, "Exercise", &spec(&["Cyrus"], Some(long))), Err(Error::Invalid(_))));
    // Closing the Team page is allowed when the coach is not kept on.
    let ok = Assignment { closed: vec!["team".into()], ..Default::default() };
    assert_eq!(w.p.make_class_packets(&folder, "Exercise", &spec(&["Cyrus"], Some(ok))).unwrap().made, 1);
}

#[test]
fn the_answer_key_holds_only_the_instructors_coding_and_comes_back_into_a_project_that_never_held_it() {
    let mut w = world();
    // A student has coded in the instructor's own project too: the key must not carry that.
    let annika = w.p.add_coder(&NewCoder::human("Annika")).unwrap();
    code(&mut w.p, w.source, 0, "Access", annika);
    let key = w.dir.path().join("keys").join("Exercise - answer key.pqda");
    fs::create_dir_all(key.parent().unwrap()).unwrap();
    let report = w.p.make_class_key(&key, w.teacher, &[]).unwrap();
    assert_eq!((report.codings, report.coder.as_str()), (3, "Teacher"), "the three codings of the instructor and none of Annika's");
    assert!(report.left_out.iter().any(|l| l.contains("every other coder's coding")));

    let opened = Project::open(&key, &options()).unwrap();
    let names: Vec<String> = opened.list_coders().unwrap().into_iter().map(|c| c.name).collect();
    assert!(names.contains(&"Teacher".to_string()) && !names.contains(&"Annika".to_string()), "{names:?}");
    let source = opened.list_sources().unwrap()[0].id;
    assert!(opened.list_codings(source).unwrap().iter().all(|c| c.layer != "coder" || names.iter().any(|n| n == "Teacher")));
    assert_eq!(opened.list_codings(source).unwrap().len(), 3);
    assert!(opened.list_memos(None, None).unwrap().iter().any(|m| m.title == "Answer key from Teacher"));
    drop(opened);

    // A new term: a project that holds the documents and the codebook and no coding at all (a blind packet is that), and the key brought in with
    // the students' files in one step.
    let folder = w.dir.path().join("term two");
    w.p.make_class_packets(&folder, "Exercise", &spec(&["Zed"], None)).unwrap();
    let fresh_path = w.dir.path().join("term two project.pqda");
    fs::copy(folder.join("Exercise - packet for Zed.pqda"), &fresh_path).unwrap();
    let mut fresh = Project::open(&fresh_path, &options()).unwrap();
    assert!(fresh.list_coders().unwrap().iter().all(|c| c.name != "Teacher"));
    let bring = w.dir.path().join("bring in");
    fs::create_dir_all(&bring).unwrap();
    fs::copy(&key, bring.join("answer key.pqda")).unwrap();
    let back = fresh.collect_class_packets(&bring, None).unwrap();
    assert_eq!(back.packets, 1, "{back:?}");
    let teacher = fresh.list_coders().unwrap().into_iter().find(|c| c.name == "Teacher").expect("the instructor is a coder of the new project").id;
    let fresh_source = fresh.list_sources().unwrap()[0].id;
    assert_eq!(fresh.list_codings(fresh_source).unwrap().iter().filter(|c| c.coder_id == teacher).count(), 3, "the key's coding is in the new project");
}

#[test]
fn an_answer_key_cannot_be_made_from_no_coding_or_a_model_or_over_a_file() {
    let mut w = world();
    let annika = w.p.add_coder(&NewCoder::human("Annika")).unwrap();
    let key = w.dir.path().join("none.pqda");
    let none = w.p.make_class_key(&key, annika, &[]);
    assert!(matches!(none, Err(Error::Invalid(_))), "{none:?}");
    assert!(!key.exists(), "a key with nothing in it is not left behind");
    let model = w.p.add_coder(&NewCoder { name: "Model".into(), role: prismqda_engine::CoderRole::Model, local_id: None, provider: Some("x".into()), model_name: Some("m".into()), model_version: Some("1".into()) }).unwrap();
    assert!(matches!(w.p.make_class_key(&w.dir.path().join("model.pqda"), model, &[]), Err(Error::Invalid(_))));
    assert!(matches!(w.p.make_class_key(&key, 987_654, &[]), Err(Error::NotFound { what: "coder", .. })));
    let there = w.dir.path().join("there.pqda");
    w.p.make_class_key(&there, w.teacher, &[]).unwrap();
    assert!(matches!(w.p.make_class_key(&there, w.teacher, &[]), Err(Error::AlreadyExists(_))), "a file that is there is not replaced");
}
