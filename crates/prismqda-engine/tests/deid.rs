//! Pseudonymizing and redacting: the list, what it does to a project for good, and the switch for what leaves the project.

use prismqda_engine::{
    CellChange, CodeEdit, ImportBatch, ImportedSource, ImportedUnit, MemoKind, MemoScope, NewCode, NewCoder, NewCoding, NewDeidRules, NewMemo, NewProject, NewSource, OpenOptions,
    Project, SourceType, UnitKind, VariableSpec,
};
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn rules(terms: &[&str], replacement: &str) -> NewDeidRules {
    NewDeidRules { terms: terms.iter().map(|t| t.to_string()).collect(), replacement: replacement.to_string(), kind: "pseudonym".to_string(), match_case: false, whole_word: true, note: String::new() }
}

fn at(text: &str, needle: &str) -> (i64, i64) {
    let byte = text.find(needle).unwrap_or_else(|| panic!("{needle} not in {text}"));
    let start = text[..byte].chars().count() as i64;
    (start, start + needle.chars().count() as i64)
}

const TEXT: &str = "Sarah Jones told me that Sarah was late. Sarah's car broke down near Boston.";

#[test]
fn the_list_is_kept_checked_and_changed() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let added = p.add_deid_rules(&rules(&["Sarah Jones", "Sarah", "sarah"], "Participant 3")).unwrap();
    assert_eq!(added.added.len(), 2, "sarah is Sarah when case does not matter");
    assert_eq!(added.already_there, ["sarah"]);
    let again = p.add_deid_rules(&rules(&["SARAH"], "Participant 9")).unwrap();
    assert!(again.added.is_empty());

    assert!(p.add_deid_rules(&rules(&["Bo"], "  ")).is_err(), "an empty replacement would delete words");
    assert!(p.add_deid_rules(&rules(&["  "], "X")).is_err());
    assert!(p.add_deid_rules(&NewDeidRules { kind: "blur".into(), ..rules(&["Bo"], "X") }).is_err());
    assert!(p.add_deid_rules(&rules(&["Bo"], "\u{301}x")).is_err(), "a mark at the start would join with the letter before it");

    let id = added.added[0].id;
    let changed = p.update_deid_rule(id, &prismqda_engine::DeidRulePatch { replacement: Some("Participant 4".into()), match_case: Some(true), ..Default::default() }).unwrap();
    assert_eq!((changed.replacement.as_str(), changed.match_case), ("Participant 4", true));
    assert!(p.update_deid_rule(id, &prismqda_engine::DeidRulePatch { term: Some("Sarah".into()), match_case: Some(false), ..Default::default() }).is_err(), "it would be the same as the other one");
    assert!(p.update_deid_rule(999, &Default::default()).is_err());

    assert_eq!(p.remove_deid_rules(&[id, 999]).unwrap(), 1);
    assert_eq!(p.deid_rules().unwrap().len(), 1);
}

#[test]
fn the_audit_trail_records_that_the_list_changed_but_never_the_words() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    p.add_deid_rules(&rules(&["Sarah Jones"], "Participant 3")).unwrap();
    p.set_deid_exports(true).unwrap();
    let all = serde_json::to_string(&p.events(1000).unwrap()).unwrap();
    assert!(!all.contains("Sarah"), "{all}");
    assert!(!all.contains("Participant"), "{all}");
    assert!(all.contains("deid.rule") && all.contains("deid.exports"));
}

#[test]
fn the_switch_needs_a_list_and_undo_walks_past_changes_to_the_list() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    assert!(p.set_deid_exports(true).is_err(), "there is nothing to apply");
    assert!(p.export_scrubber().unwrap().is_none());

    let source = p.add_source(&NewSource::new("S", SourceType::Document, TEXT)).unwrap();
    let code = p.add_code(&NewCode::new("C")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_coding(&NewCoding::new(source, 0, 5, code, coder)).unwrap();
    p.add_deid_rules(&rules(&["Sarah"], "P1")).unwrap();
    p.set_deid_exports(true).unwrap();
    let s = p.export_scrubber().unwrap().expect("on");
    assert_eq!(s.scrub("Hi Sarah"), "Hi P1");
    p.set_deid_exports(false).unwrap();
    assert!(p.export_scrubber().unwrap().is_none());
    assert_eq!(p.list_scrubber().unwrap().scrub("Hi Sarah"), "Hi P1", "the list itself is still there");

    let undone = p.undo().unwrap().expect("the coding is still undoable: list changes do not stand in the way");
    assert_eq!(undone.action, "coding.add");
}

#[test]
fn codings_follow_a_replaced_name_and_the_whole_text_changes_in_one_step() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let source = p.add_source(&NewSource::new("Interview", SourceType::Document, TEXT)).unwrap();
    let code = p.add_code(&NewCode::new("Lateness")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let (s0, e0) = at(TEXT, "Sarah Jones");
    let (s1, e1) = at(TEXT, "Sarah was late");
    let (s2, e2) = at(TEXT, "told me");
    p.apply_coding(&NewCoding::new(source, s0, e0, code, coder)).unwrap();
    p.apply_coding(&NewCoding::new(source, s1, e1, code, coder)).unwrap();
    p.apply_coding(&NewCoding::new(source, s2, e2, code, coder)).unwrap();

    p.add_deid_rules(&rules(&["Sarah Jones", "Sarah"], "Participant 3")).unwrap();
    p.add_deid_rules(&NewDeidRules { kind: "redact".into(), ..rules(&["Boston"], "[place]") }).unwrap();
    let out = p.deid_apply(None).unwrap();

    assert_eq!((out.documents_changed, out.replacements, out.codings_removed), (1, 4, 0), "{out:?}");
    assert_eq!(p.source_text(source).unwrap(), "Participant 3 told me that Participant 3 was late. Participant 3's car broke down near [place].");
    let quotes: Vec<String> = p.list_codings(source).unwrap().iter().map(|c| p.text_of_span(source, c.start, c.end).unwrap()).collect();
    assert_eq!(quotes, ["Participant 3", "told me", "Participant 3 was late"], "a coding on a name is a coding on its replacement, and the others stay on their own words");
    assert!(p.deid_rules().unwrap().is_empty(), "the list is the key to the names, so the project does not keep it");
    assert_eq!(out.rules_removed, 3);
    assert!(std::path::Path::new(&out.backup).exists(), "a backup was made first");
    let backup = Project::open(&out.backup, &OpenOptions::new("tester")).unwrap();
    assert_eq!(backup.source_text(source).unwrap(), TEXT, "the backup has the names as they were");
    let state = p.deid_state().unwrap();
    assert_eq!(state.last_applied.unwrap().replacements, 4);
    assert!(!state.exports);
}

#[test]
fn every_place_that_copies_text_is_changed_and_the_file_holds_no_trace_of_the_name() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let (one, two, three) = ("Sarah: I was late.", "Interviewer: Why?", "Sarah: The bus broke down.");
    let text = format!("{one}\n\n{two}\n\n{three}");
    let (b0, b1, b2) = (0, one.len() + 2, one.len() + two.len() + 4);
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Sarah Jones interview.docx".into(),
            source_type: SourceType::Transcript,
            folder: "Sarah's group".into(),
            text: text.clone(),
            runs: vec![],
            units: vec![
                { let mut u = ImportedUnit::new(b0 as i64, (b0 + one.len()) as i64, UnitKind::SpeakerTurn); u.speaker = Some("Sarah".into()); u },
                { let mut u = ImportedUnit::new(b1 as i64, (b1 + two.len()) as i64, UnitKind::SpeakerTurn); u.speaker = Some("Interviewer".into()); u },
                { let mut u = ImportedUnit::new(b2 as i64, (b2 + three.len()) as i64, UnitKind::SpeakerTurn); u.speaker = Some("Sarah".into()); u },
            ],
            default_unit_kind: UnitKind::SpeakerTurn,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let source = p.apply_import(&batch).unwrap().source_ids[0];
    let code = p.add_code(&NewCode::new("Lateness")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let id = p.apply_coding(&NewCoding::new(source, 7, 18, code, coder)).unwrap();
    p.set_coding_rationale(id, "Sarah gives a reason").unwrap();
    p.update_code(code, &CodeEdit { definition: Some("Sarah's kind of delay".into()), ..Default::default() }).unwrap();
    p.add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Standalone, target_id: None, phase: None, title: "About Sarah".into(), body: "Sarah seemed nervous.".into() }).unwrap();
    let case = p.add_case("Sarah Jones", "lives in Boston").unwrap();
    p.define_variable(&VariableSpec { name: "Contact".into(), level: "text".into(), value_labels: Default::default(), missing_codes: vec![] }, None).unwrap();
    p.set_values("Contact", &[CellChange { case_id: case.id, value: Some("sarah@example.org".into()) }]).unwrap();

    p.add_deid_rules(&rules(&["Sarah Jones", "Sarah"], "Participant 3")).unwrap();
    p.add_deid_rules(&rules(&["sarah@example.org"], "[email]")).unwrap();
    p.add_deid_rules(&rules(&["Boston"], "[city]")).unwrap();

    // Before anything changes, the preview says what will and what will not be found.
    let preview = p.deid_preview(None).unwrap();
    assert_eq!(preview.total, 2, "the two speaker labels in the text: {preview:?}");
    assert!(preview.elsewhere.iter().any(|e| e.place == "Memos" && e.covered), "{preview:?}");

    let out = p.deid_apply(None).unwrap();
    assert!(out.remaining.is_empty(), "nothing should be left: {:?}", out.remaining);
    assert!(out.compacted);

    assert_eq!(p.list_sources().unwrap()[0].name, "Participant 3 interview.docx");
    assert_eq!(p.list_sources().unwrap()[0].folder, "Participant 3's group");
    let units = p.list_units(source).unwrap();
    assert_eq!(units.iter().filter_map(|u| u.speaker.clone()).collect::<Vec<_>>(), ["Participant 3", "Interviewer", "Participant 3"]);
    assert_eq!(p.list_cases().unwrap()[0].name, "Participant 3");
    assert_eq!(p.list_memos(None, None).unwrap()[0].body, "Participant 3 seemed nervous.");
    assert_eq!(p.get_code(code).unwrap().definition, "Participant 3's kind of delay");
    assert_eq!(p.list_codings(source).unwrap().len(), 1);
    let history = p.coding_history(id).unwrap();
    assert!(!serde_json::to_string(&history).unwrap().contains("Sarah"));

    // The file itself, read as bytes, holds neither name nor address, not even in space the database no longer uses.
    drop(p);
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            let bytes = std::fs::read(&path).unwrap();
            let hay = String::from_utf8_lossy(&bytes);
            assert!(!hay.contains("Sarah") && !hay.contains("sarah@") && !hay.contains("Boston"), "{} still holds a name", path.display());
        }
    }
}

#[test]
fn the_trail_ends_with_a_plain_account_and_still_refuses_to_be_edited() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let source = p.add_source(&NewSource::new("S", SourceType::Document, TEXT)).unwrap();
    let code = p.add_code(&NewCode::new("C")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_coding(&NewCoding::new(source, 0, 5, code, coder)).unwrap();
    p.add_deid_rules(&rules(&["Sarah"], "P1")).unwrap();
    p.deid_apply(None).unwrap();

    let events = p.events(10_000).unwrap();
    let last = events.last().unwrap();
    assert_eq!(last.action, "deid.apply");
    let after = last.after.as_ref().unwrap();
    assert_eq!(after["documents"], 1);
    assert!(after["note"].as_str().unwrap().contains("audit trail"));
    assert!(!serde_json::to_string(&events).unwrap().contains("Sarah"));

    // The triggers that make the trail append-only are back.
    drop(p);
    let c = rusqlite::Connection::open(dir.path().join("p.pqda")).unwrap();
    assert!(c.execute("UPDATE event SET actor = 'x'", []).is_err());
    assert!(c.execute("DELETE FROM event", []).is_err());
}

#[test]
fn undo_stops_at_a_replacement_for_good_instead_of_undoing_something_older() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let source = p.add_source(&NewSource::new("S", SourceType::Document, TEXT)).unwrap();
    let code = p.add_code(&NewCode::new("C")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_coding(&NewCoding::new(source, 0, 5, code, coder)).unwrap();
    p.add_deid_rules(&rules(&["Sarah"], "Participant 3")).unwrap();
    p.deid_apply(None).unwrap();
    let err = p.undo().unwrap_err().to_string();
    assert!(err.contains("replacing names for good"), "{err}");
    assert_eq!(p.list_codings(source).unwrap().len(), 1, "the earlier coding was not undone");
}

#[test]
fn nothing_changes_when_the_list_is_empty_or_the_project_is_read_only() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    p.add_source(&NewSource::new("S", SourceType::Document, TEXT)).unwrap();
    assert!(p.deid_apply(None).is_err());
    assert!(p.deid_preview(None).unwrap().rules.is_empty());
}

#[test]
fn a_waiting_suggestion_is_turned_down_because_it_pointed_into_the_old_text() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let source = p.add_source(&NewSource::new("S", SourceType::Document, TEXT)).unwrap();
    p.add_deid_rules(&rules(&["Sarah"], "P1")).unwrap();
    let out = p.deid_apply(Some(&[source])).unwrap();
    assert_eq!(out.proposals_rejected, 0);
    assert_eq!(out.documents_changed, 1);
}

#[test]
fn only_the_documents_chosen_change_and_the_others_are_reported_as_still_holding_the_name() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let a = p.add_source(&NewSource::new("A", SourceType::Document, "Sarah in A.")).unwrap();
    let b = p.add_source(&NewSource::new("B", SourceType::Document, "Sarah in B.")).unwrap();
    p.add_deid_rules(&rules(&["Sarah"], "P1")).unwrap();
    let out = p.deid_apply(Some(&[a])).unwrap();
    assert_eq!(out.documents_changed, 1);
    assert_eq!(p.source_text(a).unwrap(), "P1 in A.");
    assert_eq!(p.source_text(b).unwrap(), "Sarah in B.");
    assert!(out.remaining.iter().any(|r| r.place == "The text of documents" && r.count == 1 && !r.covered), "{:?}", out.remaining);
}

#[test]
fn a_packet_is_sent_with_the_names_replaced_and_a_quote_with_a_replacement_lands_on_the_real_words() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let doc = "Sarah Jones said the fares are too high.\n\nShe told Sarah's sister about Boston.";
    let source = p.add_source(&NewSource::new("Riders", SourceType::Document, doc)).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let mut cost = NewCode::new("Cost");
    cost.definition = "Talk about money, as Sarah Jones put it.".into();
    let cost = p.add_code(&cost).unwrap();
    p.add_deid_rules(&rules(&["Sarah Jones", "Sarah"], "Participant 3")).unwrap();
    p.add_deid_rules(&rules(&["Boston"], "[city]")).unwrap();

    let spec: prismqda_engine::PacketSpec = serde_json::from_str("{}").unwrap();
    // With the switch off, the packet holds the names.
    let plain = p.build_packet(&spec, ada).unwrap();
    assert!(plain.prompt.contains("Sarah Jones said"), "{}", plain.prompt);

    p.set_deid_exports(true).unwrap();
    let packet = p.build_packet(&spec, ada).unwrap();
    assert!(!packet.prompt.contains("Sarah") && !packet.prompt.contains("Boston"), "{}", packet.prompt);
    assert!(packet.prompt.contains("Participant 3 said the fares are too high.") && packet.prompt.contains("[city]"), "{}", packet.prompt);
    assert!(packet.prompt.contains("as Participant 3 put it"), "the codebook text is scrubbed too");
    // What is kept in the project as having been sent is what was sent.
    let detail = p.ai_run(packet.run_id).unwrap();
    assert!(!detail.prompt.contains("Sarah"));

    // The model quotes the replacement; the suggestion lands on the real words, with the whole name taken.
    let answer = r#"[{"passage": "P1", "code": "Cost", "quote": "Participant 3 said the fares are too high", "reason": "fares"},
                     {"passage": "P2", "code": "Cost", "quote": "told Participant 3's sister", "reason": "x"}]"#;
    let out = p.answer_packet(packet.run_id, "a chatbot", answer).unwrap();
    assert_eq!(out.suggested, 2, "{:?}", out.rejected);
    let queue = p.list_proposals(Some("pending"), None, 10).unwrap();
    let first = queue.iter().find(|q| q.start == 0).expect("first");
    assert_eq!(p.text_of_span(source, first.start, first.end).unwrap(), "Sarah Jones said the fares are too high");
    let second = queue.iter().find(|q| q.start != 0).expect("second");
    assert_eq!(p.text_of_span(source, second.start, second.end).unwrap(), "told Sarah's sister");
    assert_eq!(p.list_codings(source).unwrap().len(), 0, "nothing is coded until a person accepts it");
    let _ = cost;

    // A quote the model made up from the real name does not exist in what it was shown, so it is refused.
    let second_packet = p.build_packet(&prismqda_engine::PacketSpec { only_uncoded: false, ..serde_json::from_str("{}").unwrap() }, ada).unwrap();
    let bad = r#"[{"passage": "P1", "code": "Cost", "quote": "Sarah Jones said the fares", "reason": "x"}]"#;
    let out = p.answer_packet(second_packet.run_id, "a chatbot", bad).unwrap();
    assert_eq!(out.suggested, 0);
    assert_eq!(out.rejected.len(), 1);

    let disclosure = p.ai_disclosure().unwrap();
    assert!(disclosure.contains("replaced with labels"), "{disclosure}");
}

// ---------------------------------------------------------------- a copy made to be shared (decision 180)

/// A project with a transcript that names a person, a coding, a memo, and a case, and a list that finds the name.
fn project_with_a_name(dir: &TempDir) -> (Project, i64) {
    let mut p = project(dir);
    let text = "Sarah: I was late.\n\nInterviewer: Why?\n\nSarah: The bus broke down near Boston.";
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Sarah Jones interview.docx".into(),
            source_type: SourceType::Transcript,
            folder: String::new(),
            text: text.into(),
            runs: vec![],
            units: vec![ImportedUnit::new(0, text.chars().count() as i64, UnitKind::Sentence)],
            default_unit_kind: UnitKind::Sentence,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let source = p.apply_import(&batch).unwrap().source_ids[0];
    let code = p.add_code(&NewCode::new("Lateness")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_coding(&NewCoding::new(source, 7, 18, code, coder)).unwrap();
    p.add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Standalone, target_id: None, phase: None, title: "About Sarah".into(), body: "Sarah seemed nervous.".into() }).unwrap();
    p.add_case("Sarah Jones", "lives in Boston").unwrap();
    p.add_deid_rules(&rules(&["Sarah Jones", "Sarah"], "Participant 3")).unwrap();
    p.add_deid_rules(&rules(&["Boston"], "[city]")).unwrap();
    (p, source)
}

#[test]
fn a_copy_made_to_be_shared_carries_no_persons_name_on_its_changes_and_keeps_the_people_apart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("p.pqda");
    // The first person is known to the computer by an account name (what every earlier version recorded).
    let mut first = OpenOptions::new("jason-pc");
    first.backup_on_open = false;
    let mut p = Project::create(&path, &NewProject::new("P"), &first).unwrap();
    p.ensure_human_coder("jason-pc", "user-jason-pc").unwrap();
    p.add_code(&NewCode::new("First code")).unwrap();
    drop(p);
    // The second person goes by a name that looks like one of the labels a copy gives, to show that the labels are not taken for names.
    let mut second = OpenOptions::new("Researcher 1");
    second.backup_on_open = false;
    let mut p = Project::open(&path, &second).unwrap();
    p.ensure_human_coder("Researcher 1", "user-researcher-1").unwrap();
    p.add_code(&NewCode::new("Second code")).unwrap();
    p.add_source(&NewSource::new("Interview", SourceType::Document, "Sarah said hello.")).unwrap();
    p.add_deid_rules(&rules(&["Sarah"], "Participant 3")).unwrap();
    let copy_path = dir.path().join("shared.pqda");
    p.deid_copy(&copy_path).unwrap();
    // The project itself is not changed: its own history keeps the names.
    let own: std::collections::BTreeSet<String> = p.events(10_000).unwrap().into_iter().map(|e| e.actor).collect();
    assert!(own.contains("jason-pc") && own.contains("Researcher 1"), "{own:?}");
    drop(p);

    let conn = rusqlite::Connection::open(&copy_path).unwrap();
    let actors: Vec<String> = {
        let mut stmt = conn.prepare("SELECT actor FROM event GROUP BY actor ORDER BY MIN(id)").unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    };
    assert_eq!(actors, ["Researcher 1", "Researcher 2"], "one neutral label for each person, in the order they first appear");
    let first_changes: i64 = conn.query_row("SELECT COUNT(*) FROM event WHERE actor = 'Researcher 1' AND action = 'code.add'", [], |r| r.get(0)).unwrap();
    let second_changes: i64 = conn.query_row("SELECT COUNT(*) FROM event WHERE actor = 'Researcher 2' AND action = 'code.add'", [], |r| r.get(0)).unwrap();
    assert_eq!((first_changes, second_changes), (1, 1), "the first person's change is still apart from the second's");
    let coders: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT name, local_id FROM coder WHERE local_id LIKE 'user-%' ORDER BY id").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
    };
    assert_eq!(coders, [("Researcher 1".to_string(), "user-researcher-1".to_string()), ("Researcher 2".to_string(), "user-researcher-2".to_string())]);
    for table_text in ["SELECT group_concat(actor) FROM event", "SELECT group_concat(name || local_id) FROM coder"] {
        let all: String = conn.query_row(table_text, [], |r| r.get(0)).unwrap();
        assert!(!all.to_lowercase().contains("jason"), "{all}");
    }
    // The audit trail still refuses to be changed afterwards: the triggers are back.
    assert!(conn.execute("UPDATE event SET actor = 'x'", []).is_err());
    assert!(conn.execute("DELETE FROM event", []).is_err());
}

#[test]
fn a_copy_made_to_be_shared_has_the_names_replaced_and_the_project_is_not_touched() {
    let dir = TempDir::new().unwrap();
    let (mut p, source) = project_with_a_name(&dir);
    let copy_path = dir.path().join("shared.pqda");
    let made = p.deid_copy(&copy_path).unwrap();
    assert!(made.outcome.remaining.is_empty(), "{:?}", made.outcome.remaining);
    assert_eq!((made.taken_out.len(), made.recordings_unlinked, made.runs_archived), (0, 0, 0));
    assert!(made.outcome.backup.is_empty(), "a copy to be shared makes no backup");
    assert!(made.outcome.documents_changed >= 1 && made.outcome.replacements >= 2);

    // The project: its words, its list, and its files are as they were.
    assert!(p.source_text(source).unwrap().contains("Sarah"));
    assert_eq!(p.deid_rules().unwrap().len(), 3, "the list stays with the project");
    assert_eq!(p.list_sources().unwrap()[0].name, "Sarah Jones interview.docx");
    assert!(p.events(1000).unwrap().iter().any(|e| e.action == "deid.copy"), "the project's trail says a copy was made");
    let trail = serde_json::to_string(&p.events(1000).unwrap()).unwrap();
    assert!(!trail.contains("shared.pqda\\"), "{trail}");

    // The copy: the words are replaced, the coding is where it was, the list is gone, and it says what it is.
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let copy = Project::open(&copy_path, &o).unwrap();
    assert!(copy.deid_rules().unwrap().is_empty());
    assert_eq!(copy.list_sources().unwrap()[0].name, "Participant 3 interview.docx");
    let text = copy.source_text(source).unwrap();
    assert!(text.starts_with("Participant 3: I was late.") && text.contains("[city]"), "{text}");
    assert_eq!(copy.list_codings(source).unwrap().len(), 1);
    assert_eq!(copy.list_memos(None, None).unwrap()[0].body, "Participant 3 seemed nervous.");
    assert_eq!(copy.list_cases().unwrap()[0].name, "Participant 3");
    assert!(copy.info().unwrap().name.ends_with("(names replaced)"));
    drop(copy);

    // Nothing but the copy was made beside it: no backup, no side files, and not a trace of a name in the file.
    let mut files: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).filter(|n| !n.starts_with("p.pqda")).collect();
    files.sort();
    assert_eq!(files, ["shared.pqda"], "the project's own files aside, only the copy is there: {files:?}");
    let bytes = std::fs::read(&copy_path).unwrap();
    let hay = String::from_utf8_lossy(&bytes);
    assert!(!hay.contains("Sarah") && !hay.contains("Boston"), "the copy still holds a name");
}

#[test]
fn a_copy_is_refused_without_a_list_or_over_a_file_and_leaves_nothing_behind() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    assert!(p.deid_copy(&dir.path().join("a.pqda")).is_err(), "no list, nothing to replace");
    assert!(!dir.path().join("a.pqda").exists());
    let other = TempDir::new().unwrap();
    let (mut q, _) = project_with_a_name(&other);
    let taken = dir.path().join("taken.pqda");
    std::fs::write(&taken, b"not yours to replace").unwrap();
    assert!(q.deid_copy(&taken).is_err());
    assert_eq!(std::fs::read(&taken).unwrap(), b"not yours to replace");
}
