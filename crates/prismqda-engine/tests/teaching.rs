//! A class exercise: a blind packet for every student, bringing the packets back, and the review of the class against the instructor.

use std::fs;
use std::path::Path;

use prismqda_engine::{
    kappa_2x2, tidy_students, ClassPacketsSpec, ClassReviewSpec, Error, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, SourceType,
    UnitKind,
};
use prismqda_stats::{agreement, Ratings};
use tempfile::TempDir;

const RIDERS: [&str; 5] = ["Fares are too high for me.", "The stop is far from my house.", "I love the new buses.", "The driver was rude to me.", "Nothing to add here."];

fn options() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

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

/// Codes `unit` (by its position in the document) with the named codes, as `coder`.
fn code(p: &mut Project, source: i64, which: usize, names: &[&str], coder: i64) {
    let units = p.list_units(source).unwrap();
    let codes = p.list_codes().unwrap();
    for name in names {
        let id = codes.iter().find(|c| c.name == *name).unwrap().id;
        let mut c = NewCoding::new(source, units[which].start, units[which].end, id, coder);
        c.unit_id = Some(units[which].id);
        p.apply_coding(&c).unwrap();
    }
}

struct W {
    p: Project,
    dir: TempDir,
    source: i64,
    other: i64,
    teacher: i64,
}

/// A study with two documents (the exercise of five paragraphs, and another of one) and an instructor who has coded four paragraphs of the
/// exercise and the whole of the other document.
fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("class.pqda"), &NewProject::new("Class"), &options()).unwrap();
    let source = p.apply_import(&batch("Riders", &RIDERS)).unwrap().source_ids[0];
    let other = p.apply_import(&batch("Extra", &["One more paragraph."])).unwrap().source_ids[0];
    p.add_code(&NewCode::new("Cost")).unwrap();
    p.add_code(&NewCode::new("Access")).unwrap();
    let teacher = p.add_coder(&NewCoder::human("Teacher")).unwrap();
    code(&mut p, source, 0, &["Cost"], teacher);
    code(&mut p, source, 1, &["Access"], teacher);
    code(&mut p, source, 2, &["Cost", "Access"], teacher);
    code(&mut p, source, 3, &["Cost"], teacher);
    code(&mut p, other, 0, &["Cost"], teacher);
    W { p, dir, source, other, teacher }
}

fn student(w: &mut W, name: &str, work: &[(usize, &[&str])]) -> i64 {
    let id = w.p.add_coder(&NewCoder::human(name)).unwrap();
    for (unit, names) in work {
        code(&mut w.p, w.source, *unit, names, id);
    }
    id
}

fn near(got: Option<f64>, want: f64) -> bool {
    got.is_some_and(|g| (g - want).abs() < 1e-9)
}

#[test]
fn a_students_coding_is_compared_with_the_instructors_passage_by_passage() {
    let mut w = world();
    // Annika codes everything the instructor did, and one more paragraph. Bohdan agrees on one passage, half agrees on one, differs on one, and leaves one uncoded. Cyrus codes nothing.
    let annika = student(&mut w, "Annika", &[(0, &["Cost"]), (1, &["Access"]), (2, &["Cost", "Access"]), (3, &["Cost"]), (4, &["Cost"])]);
    let bohdan = student(&mut w, "Bohdan", &[(0, &["Cost"]), (1, &["Cost"]), (2, &["Cost"])]);
    let cyrus = w.p.add_coder(&NewCoder::human("Cyrus")).unwrap();
    let review = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: false, hardest: 10 }).unwrap();

    assert_eq!(review.instructor.name, "Teacher");
    assert_eq!((review.instructor_passages, review.passages_compared), (4, 5), "four passages the instructor coded, and the fifth that only Annika coded");
    let names: Vec<&str> = review.students.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(names, ["Annika", "Bohdan", "Cyrus"], "every other human coder is a student, by name");

    let a = &review.students[0];
    assert_eq!(a.coder_id, Some(annika));
    assert_eq!((a.passages_coded, a.same, a.partly, a.different, a.left_uncoded, a.extra), (5, 4, 0, 0, 0, 1));
    assert_eq!((a.found, a.missed, a.added), (5, 0, 1));
    assert!(near(a.share_same, 1.0) && near(a.share_found, 1.0) && near(a.share_added, 1.0 / 6.0));

    let b = &review.students[1];
    assert_eq!(b.coder_id, Some(bohdan));
    assert_eq!((b.passages_coded, b.same, b.partly, b.different, b.left_uncoded, b.extra), (3, 1, 1, 1, 1, 0));
    assert_eq!((b.found, b.missed, b.added), (2, 3, 1));
    assert!(near(b.share_same, 0.25) && near(b.share_found, 0.4) && near(b.share_added, 1.0 / 3.0));

    let c = &review.students[2];
    assert_eq!(c.coder_id, Some(cyrus));
    assert_eq!((c.passages_coded, c.same, c.left_uncoded, c.found, c.missed, c.added), (0, 0, 4, 0, 5, 0));
    assert!(near(c.share_same, 0.0) && near(c.share_found, 0.0) && c.share_added.is_none(), "nothing coded has no share of its codes the instructor did not give");
    assert!(review.notes.iter().any(|n| n.contains("1 student coded nothing")), "{:?}", review.notes);
}

#[test]
fn the_codes_are_listed_hardest_first_with_how_the_class_found_each() {
    let mut w = world();
    student(&mut w, "Annika", &[(0, &["Cost"]), (1, &["Access"]), (2, &["Cost", "Access"]), (3, &["Cost"]), (4, &["Cost"])]);
    student(&mut w, "Bohdan", &[(0, &["Cost"]), (1, &["Cost"]), (2, &["Cost"])]);
    w.p.add_coder(&NewCoder::human("Cyrus")).unwrap();
    let review = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: false, hardest: 10 }).unwrap();

    let order: Vec<&str> = review.by_code.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(order, ["Access", "Cost"], "Access, which the class found least, comes first");
    let access = &review.by_code[0];
    assert_eq!(access.instructor_passages, 2);
    assert!(near(access.mean_found, 1.0 / 3.0), "Annika found both, Bohdan neither, Cyrus neither: {:?}", access.mean_found);
    assert_eq!(access.added, 0);
    let cost = &review.by_code[1];
    assert_eq!(cost.instructor_passages, 3);
    assert!(near(cost.mean_found, (1.0 + 2.0 / 3.0 + 0.0) / 3.0));
    assert_eq!(cost.added, 2, "Bohdan gave Cost to the stop paragraph, and Annika to the fifth paragraph, where the instructor did not");
    // Annika's kappa for Cost is the one worked out by hand over five passages: both 3, only she 1, neither 1.
    assert!(cost.kappa_students >= 1 && cost.mean_kappa.is_some());
}

#[test]
fn kappa_of_two_yes_or_no_answers_is_the_same_as_the_statistics_crate_gives() {
    for (a, b, c, d) in [(3, 0, 1, 1), (10, 2, 3, 5), (0, 4, 4, 0), (7, 1, 1, 7)] {
        let mut rows = Vec::new();
        for _ in 0..a {
            rows.push(vec![Some(1), Some(1)]);
        }
        for _ in 0..b {
            rows.push(vec![Some(1), Some(0)]);
        }
        for _ in 0..c {
            rows.push(vec![Some(0), Some(1)]);
        }
        for _ in 0..d {
            rows.push(vec![Some(0), Some(0)]);
        }
        let theirs = agreement(&Ratings::new(2, rows).unwrap()).cohen_kappa.unwrap();
        let ours = kappa_2x2(a, b, c, d).unwrap();
        assert!((ours - theirs).abs() < 1e-9, "{a} {b} {c} {d}: {ours} against {theirs}");
    }
    assert!((kappa_2x2(3, 0, 1, 1).unwrap() - 0.545_454_545_454_545_4).abs() < 1e-9, "worked by hand: observed 0.8, by chance 0.56");
    assert!(kappa_2x2(5, 0, 0, 0).is_none(), "everyone said yes every time, so there is nothing to correct for chance");
    assert!(kappa_2x2(0, 0, 0, 0).is_none());
}

#[test]
fn the_hardest_passages_say_what_students_gave_and_left_out() {
    let mut w = world();
    student(&mut w, "Annika", &[(0, &["Cost"]), (1, &["Access"]), (2, &["Cost", "Access"]), (3, &["Cost"])]);
    student(&mut w, "Bohdan", &[(0, &["Cost"]), (1, &["Cost"]), (2, &["Cost"])]);
    w.p.add_coder(&NewCoder::human("Cyrus")).unwrap();
    let spec = ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: false, hardest: 2 };
    let review = w.p.class_review(&spec).unwrap();
    assert_eq!(review.hardest.len(), 2, "only as many as were asked for");
    let first = &review.hardest[0];
    assert_eq!(first.text, "The stop is far from my house.", "the passage the fewest students coded as the instructor did comes first, in reading order among equals");
    assert_eq!((first.students_same, first.students_total), (1, 3));
    assert_eq!(first.instructor_codes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Access"]);
    assert_eq!(first.added_codes.iter().map(|(c, n)| (c.name.as_str(), *n)).collect::<Vec<_>>(), [("Cost", 1)], "Bohdan gave Cost");
    assert_eq!(first.missed_codes.iter().map(|(c, n)| (c.name.as_str(), *n)).collect::<Vec<_>>(), [("Access", 2)], "Bohdan and Cyrus left Access out");
    // The passage everyone but Cyrus coded as the instructor did is the last of the hard ones; a passage nobody missed is not listed.
    let all = w.p.class_review(&ClassReviewSpec { hardest: 50, ..spec }).unwrap();
    assert_eq!(all.hardest.len(), 4, "the four passages the instructor coded in this document, each missed by someone");
    assert_eq!(all.hardest.last().unwrap().students_same, 2);
}

#[test]
fn an_anonymous_review_names_no_student_and_does_not_follow_the_names() {
    let mut w = world();
    student(&mut w, "Annika Alder", &[(0, &["Cost"])]);
    student(&mut w, "Bohdan Brandt", &[(0, &["Cost"]), (1, &["Access"])]);
    student(&mut w, "Cyrus Castellano", &[]);
    let review = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: true, hardest: 10 }).unwrap();
    let labels: Vec<&str> = review.students.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["Student 1", "Student 2", "Student 3"]);
    assert!(review.students.iter().all(|s| s.coder_id.is_none()), "no number that leads to a student either");
    let json = serde_json::to_string(&review.students).unwrap() + &serde_json::to_string(&review.hardest).unwrap() + &serde_json::to_string(&review.by_code).unwrap();
    for name in ["Annika", "Bohdan", "Cyrus", "Alder", "Brandt", "Castellano"] {
        assert!(!json.contains(name), "{name} appears in an anonymous review");
    }
    // The same numbers as the named review, in another order.
    let named = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: false, hardest: 10 }).unwrap();
    let mut a: Vec<(usize, usize, usize)> = review.students.iter().map(|s| (s.passages_coded, s.same, s.found)).collect();
    let mut b: Vec<(usize, usize, usize)> = named.students.iter().map(|s| (s.passages_coded, s.same, s.found)).collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b);
    let again = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: true, hardest: 10 }).unwrap();
    assert_eq!(again, review, "the same review gives the same labels");
}

#[test]
fn the_documents_chosen_decide_which_passages_are_compared() {
    let mut w = world();
    student(&mut w, "Annika", &[(0, &["Cost"])]);
    let everything = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![], anonymous: false, hardest: 10 }).unwrap();
    assert_eq!(everything.instructor_passages, 5, "four in the exercise and one in the other document");
    let only = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: false, hardest: 10 }).unwrap();
    assert_eq!(only.instructor_passages, 4);
    let nothing = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.other], anonymous: false, hardest: 10 }).unwrap();
    assert_eq!(nothing.instructor_passages, 1);
}

#[test]
fn a_review_that_cannot_be_made_says_why() {
    let mut w = world();
    let spec = |instructor_id: i64, student_ids: Vec<i64>| ClassReviewSpec { instructor_id, student_ids, source_ids: vec![], anonymous: false, hardest: 10 };
    let e = w.p.class_review(&spec(w.teacher, vec![])).unwrap_err();
    assert!(e.to_string().contains("no student"), "{e}");
    let annika = student(&mut w, "Annika", &[(0, &["Cost"])]);
    assert!(matches!(w.p.class_review(&spec(9999, vec![])), Err(Error::NotFound { .. })));
    assert!(matches!(w.p.class_review(&spec(w.teacher, vec![9999])), Err(Error::NotFound { .. })));
    let e = w.p.class_review(&spec(w.teacher, vec![w.teacher])).unwrap_err();
    assert!(e.to_string().contains("cannot be one of the students"), "{e}");
    // An instructor who coded nothing has nothing to compare with.
    let e = w.p.class_review(&spec(annika, vec![w.teacher])).map(|r| r.instructor_passages);
    assert!(e.is_ok(), "the instructor is whoever is chosen, and the teacher's coding is then a student's");
    let blank = w.p.add_coder(&NewCoder::human("Blank")).unwrap();
    let e = w.p.class_review(&spec(blank, vec![annika])).unwrap_err();
    assert!(e.to_string().contains("not coded any passage"), "{e}");
}

#[test]
fn names_of_a_class_are_tidied_and_a_name_twice_is_made_once() {
    let (kept, repeated) = tidy_students(&["  Annika   Alder ".to_string(), String::new(), "bohdan".to_string(), "ANNIKA alder".to_string(), "   ".to_string()]);
    assert_eq!(kept, ["Annika Alder", "bohdan"]);
    assert_eq!(repeated, ["ANNIKA alder"]);
}

fn packet_files(folder: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(folder).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".pqda")).collect();
    v.sort();
    v
}

/// Opens a student's packet, codes in it as the student, and closes it.
fn work_in(packet: &Path, which: usize, names: &[&str]) {
    let mut p = Project::open(packet, &options()).unwrap();
    let info = p.coder_packet_info().unwrap().expect("a packet says whom it is for");
    let coder = p.list_coders().unwrap().into_iter().find(|c| c.local_id == info.coder_local_id).unwrap().id;
    let source = p.list_sources().unwrap().into_iter().find(|s| s.name == "Riders").unwrap().id;
    code(&mut p, source, which, names, coder);
}

#[test]
fn a_blind_packet_is_made_for_each_student_and_the_work_comes_back_in_one_step() {
    let mut w = world();
    let folder = w.dir.path().join("packets");
    let spec = ClassPacketsSpec { students: vec!["Annika".into(), "bohdan".into(), "annika".into(), "Cyrus".into()], source_ids: vec![w.source], instructions: "Code each paragraph with one or two codes.".into() };
    let made = w.p.make_class_packets(&folder, "Class", &spec).unwrap();
    assert_eq!((made.made, made.packets.len()), (3, 3));
    assert_eq!(made.repeated, ["annika"]);
    assert_eq!(made.documents, ["Riders"], "only the document chosen is put in");
    assert_eq!(made.codes, 2);
    assert_eq!(packet_files(&folder), ["Class - packet for Annika.pqda", "Class - packet for Cyrus.pqda", "Class - packet for bohdan.pqda"]);
    // Blind: not a coding of the instructor's is in a packet.
    {
        let p = Project::open(folder.join("Class - packet for Annika.pqda"), &options()).unwrap();
        let source = p.list_sources().unwrap()[0].id;
        assert!(p.list_codings(source).unwrap().is_empty(), "the instructor's codings are not in the packet");
    }
    // Making them again changes nothing and says why for each.
    let again = w.p.make_class_packets(&folder, "Class", &spec).unwrap();
    assert_eq!(again.made, 0);
    assert!(again.packets.iter().all(|p| !p.made && p.error.as_deref().is_some_and(|e| e.contains("already exists"))), "{:?}", again.packets);

    // The students code in their packets (Cyrus hands his in blank), and a file that is not a packet and a packet of another study lie among them.
    work_in(&folder.join("Class - packet for Annika.pqda"), 0, &["Cost"]);
    work_in(&folder.join("Class - packet for Annika.pqda"), 1, &["Access"]);
    work_in(&folder.join("Class - packet for bohdan.pqda"), 0, &["Access"]);
    Project::create(folder.join("Stray.pqda"), &NewProject::new("Stray"), &options()).unwrap();
    fs::write(folder.join("notes.txt"), "not a project").unwrap();
    let elsewhere = {
        let dir = TempDir::new().unwrap();
        let mut q = Project::create(dir.path().join("elsewhere.pqda"), &NewProject::new("Elsewhere"), &options()).unwrap();
        q.apply_import(&batch("A different document", &["Not part of this study."])).unwrap();
        let made = q.make_class_packets(&folder, "Elsewhere", &ClassPacketsSpec { students: vec!["Dara".into()], ..Default::default() }).unwrap();
        assert_eq!(made.made, 1);
        dir
    };
    drop(elsewhere);

    let coders_before = w.p.list_coders().unwrap().len();
    let report = w.p.collect_class_packets(&folder, None).unwrap();
    let status = |file: &str| report.files.iter().find(|f| f.file.contains(file)).unwrap_or_else(|| panic!("{file} in {:?}", report.files)).status.clone();
    assert_eq!(status("Annika"), "merged");
    assert_eq!(status("bohdan"), "merged");
    assert_eq!(status("Cyrus"), "merged", "a blank packet is brought in too, so the student is in the class");
    assert_eq!(status("Stray"), "not_a_packet");
    assert_eq!(status("Dara"), "other_study");
    assert_eq!(report.packets, 3);
    assert_eq!(report.codings_added, 3);
    assert!(report.backup.is_some(), "one backup before anything is brought in");
    assert_eq!(w.p.list_coders().unwrap().len(), coders_before + 3, "the three students are coders now");
    assert_eq!(w.p.list_sources().unwrap().len(), 2, "no document came in with a packet");
    assert!(w.p.list_memos(None, None).unwrap().iter().all(|m| !m.title.starts_with("Instructions for")), "the instructions memo of a packet stays in the packet");

    // The class review now reads the three students.
    let review = w.p.class_review(&ClassReviewSpec { instructor_id: w.teacher, student_ids: vec![], source_ids: vec![w.source], anonymous: false, hardest: 10 }).unwrap();
    let by_name = |n: &str| review.students.iter().find(|s| s.label == n).unwrap();
    assert_eq!((by_name("Annika").same, by_name("Annika").left_uncoded), (2, 2));
    assert_eq!((by_name("bohdan").different, by_name("bohdan").left_uncoded), (1, 3));
    assert_eq!(by_name("Cyrus").passages_coded, 0);

    // Bringing them in a second time changes nothing.
    let second = w.p.collect_class_packets(&folder, None).unwrap();
    assert_eq!(second.codings_added, 0);
    assert_eq!(status_of(&second, "Annika"), "nothing_new");
    assert_eq!(w.p.list_coders().unwrap().len(), coders_before + 3);
}

fn status_of(report: &prismqda_engine::ClassCollectReport, file: &str) -> String {
    report.files.iter().find(|f| f.file.contains(file)).unwrap().status.clone()
}

#[test]
fn a_class_with_no_name_or_a_folder_that_is_a_file_is_refused() {
    let w = world();
    let e = w.p.make_class_packets(&w.dir.path().join("p"), "Class", &ClassPacketsSpec { students: vec!["  ".into()], ..Default::default() }).unwrap_err();
    assert!(e.to_string().contains("at least one student"), "{e}");
    let file = w.dir.path().join("a file");
    fs::write(&file, "x").unwrap();
    let e = w.p.make_class_packets(&file, "Class", &ClassPacketsSpec { students: vec!["Annika".into()], ..Default::default() }).unwrap_err();
    assert!(e.to_string().contains("is a file"), "{e}");
    let mut w = w;
    let e = w.p.collect_class_packets(&w.dir.path().join("no such folder"), None).unwrap_err();
    assert!(e.to_string().contains("not a folder"), "{e}");
}

#[test]
fn a_document_that_changed_and_a_code_a_student_added_are_said_when_the_packets_come_back() {
    let mut w = world();
    let folder = w.dir.path().join("packets");
    w.p.make_class_packets(&folder, "Class", &ClassPacketsSpec { students: vec!["Annika".into()], source_ids: vec![w.source], ..Default::default() }).unwrap();
    // Annika adds a code of her own to the codebook in her packet and uses it.
    {
        let packet = folder.join("Class - packet for Annika.pqda");
        let mut p = Project::open(&packet, &options()).unwrap();
        let info = p.coder_packet_info().unwrap().unwrap();
        let coder = p.list_coders().unwrap().into_iter().find(|c| c.local_id == info.coder_local_id).unwrap().id;
        let source = p.list_sources().unwrap().into_iter().find(|s| s.name == "Riders").unwrap().id;
        p.add_code(&NewCode::new("Safety")).unwrap();
        code(&mut p, source, 0, &["Safety"], coder);
    }
    let report = w.p.collect_class_packets(&folder, None).unwrap();
    let a = &report.files[0];
    assert_eq!((a.status.as_str(), a.codings_added), ("merged", 1));
    assert_eq!(a.codes_added, ["Safety"], "the code is in the codebook now, and the report says so");
    assert!(a.message.contains("added a code to the codebook: Safety"), "{}", a.message);
    assert!(w.p.list_codes().unwrap().iter().any(|c| c.name == "Safety"));

    // A packet from a copy of the study whose document of the same name has other words: its coding is not brought in, and the report says why.
    let dir = TempDir::new().unwrap();
    let mut q = Project::create(dir.path().join("q.pqda"), &NewProject::new("Q"), &options()).unwrap();
    q.apply_import(&batch("Riders", &["Entirely different words here."])).unwrap();
    q.add_code(&NewCode::new("Cost")).unwrap();
    q.make_class_packets(&folder, "Changed", &ClassPacketsSpec { students: vec!["Bohdan".into()], ..Default::default() }).unwrap();
    work_in(&folder.join("Changed - packet for Bohdan.pqda"), 0, &["Cost"]);
    let second = w.p.collect_class_packets(&folder, None).unwrap();
    let b = second.files.iter().find(|f| f.file.contains("Bohdan")).unwrap();
    assert_eq!((b.status.as_str(), b.codings_added), ("partly", 0), "{b:?}");
    assert_eq!(b.documents_in_conflict, ["Riders"]);
    assert!(b.message.contains("not the words in the packet"), "{}", b.message);
    let annika = second.files.iter().find(|f| f.file.contains("Annika")).unwrap();
    assert_eq!(annika.status, "nothing_new", "the first packet is not brought in twice");
}
