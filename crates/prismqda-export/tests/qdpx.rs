//! Whole-project exchange in the REFI-QDA format (decision 159): a project written out and read back, a file made as the standard's own
//! example is made, and files that are not what they say.

use std::io::Write;

use prismqda_engine::{
    CellChange, CoderRow, ImportBatch, ImportedSource, ImportedUnit, MemoKind, MemoScope, NewCode, NewCoder, NewCoding, NewMemo, NewProject, NewRegion, NewSource, OpenOptions, Project, Shape, SourceType, UnitKind, VariableSpec,
};
use prismqda_export::{export_qdpx, export_qdpx_with, import_qdpx, QdpxOptions};

fn create(dir: &std::path::Path, file: &str, name: &str) -> Project {
    Project::create(dir.join(file), &NewProject::new(name), &OpenOptions::new("test")).unwrap()
}

/// Where `needle` is in `text`, counted in Unicode code points.
fn at(text: &str, needle: &str) -> (i64, i64) {
    let byte = text.find(needle).expect("the words are in the text");
    let start = text[..byte].chars().count() as i64;
    (start, start + needle.chars().count() as i64)
}

/// A PNG that is only its first bytes: the signature and a header that says 200 by 100 pixels. PrismQDA reads a picture's kind and size and
/// does not decode it.
fn png() -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
    b.extend_from_slice(&200u32.to_be_bytes());
    b.extend_from_slice(&100u32.to_be_bytes());
    b.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
    b
}

fn coder_name(coders: &[CoderRow], id: i64) -> String {
    coders.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default()
}

#[test]
fn a_project_written_as_qdpx_and_read_back_keeps_what_the_format_holds() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "a.pqda", "Round trip");
    let ana = p.add_coder(&NewCoder::human("Ana")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();

    // codes: a theme with two codes under it, and a code of its own
    let mut theme = NewCode::new("Experience");
    theme.theme = true;
    theme.color = Some("#aa3355".to_string());
    let theme_id = p.add_code(&theme).unwrap();
    let mut trust = NewCode::new("Trust");
    trust.parent_id = Some(theme_id);
    trust.definition = "Feeling safe".to_string();
    trust.inclusion_criteria = "Says they trust someone".to_string();
    trust.exclusion_criteria = "Talks about fear".to_string();
    trust.color = Some("#00aa00".to_string());
    let trust_id = p.add_code(&trust).unwrap();
    let mut fear = NewCode::new("Fear");
    fear.parent_id = Some(theme_id);
    let fear_id = p.add_code(&fear).unwrap();
    let place_id = p.add_code(&NewCode::new("Place")).unwrap();

    // a document with a letter outside the basic plane, so that a count in bytes or in UTF-16 units would be wrong
    let text = "Caf\u{e9} \u{1F600} I trust them, and yet fear comes. The place was quiet.";
    let a = p.add_source(&NewSource::new("Interview 1", SourceType::Document, text)).unwrap();
    let (t0, t1) = at(text, "trust");
    let (f0, f1) = at(text, "fear");
    let first = p.apply_coding(&NewCoding::new(a, t0, t1, trust_id, ana)).unwrap();
    p.apply_coding(&NewCoding::new(a, t0, t1, trust_id, bo)).unwrap();
    let mut with_reason = NewCoding::new(a, f0, f1, fear_id, ana);
    with_reason.rationale = "She says it herself".to_string();
    let fear_coding = p.apply_coding(&with_reason).unwrap();

    // a picture with one coded area
    let opened = prismqda_import::open_bytes("photo.png", &png(), &prismqda_import::options::ImportOptions::default()).unwrap();
    let prismqda_import::Opened::Batch(batch) = opened else { panic!("a picture is a batch") };
    let outcome = p.apply_import(&batch).unwrap();
    let pic = outcome.source_ids[0];
    let area = p.apply_region(&NewRegion::new(pic, Shape::Rectangle { x: 0.1, y: 0.2, w: 0.4, h: 0.4 }, place_id, ana)).unwrap();
    // links between passages: words to words, and words to an area of the picture (an area is its number made negative)
    p.add_passage_link(first, fear_coding, "is undone by").unwrap();
    p.add_passage_link(fear_coding, -area, "echoes").unwrap();

    // a case with variables, holding the document
    let case = p.add_case("Participant 1", "Interviewed twice").unwrap();
    p.assign_source_to_case(case.id, a).unwrap();
    p.define_variable(&VariableSpec { name: "age".to_string(), level: "ratio".to_string(), value_labels: Default::default(), missing_codes: Vec::new() }, None).unwrap();
    p.define_variable(&VariableSpec { name: "group".to_string(), level: "nominal".to_string(), value_labels: Default::default(), missing_codes: Vec::new() }, None).unwrap();
    p.set_values("age", &[CellChange { case_id: case.id, value: Some("34".to_string()) }]).unwrap();
    p.set_values("group", &[CellChange { case_id: case.id, value: Some("A".to_string()) }]).unwrap();

    // memos on the project, a document, a code, and a coding; and a set of codes
    let memo = |scope, target, title: &str| NewMemo { kind: MemoKind::Analytic, scope, target_id: target, phase: None, title: title.to_string(), body: format!("About {title}") };
    p.add_memo(&memo(MemoScope::Project, None, "Plan")).unwrap();
    p.add_memo(&memo(MemoScope::Source, Some(a), "On the interview")).unwrap();
    p.add_memo(&memo(MemoScope::Code, Some(trust_id), "On trust")).unwrap();
    p.add_memo(&memo(MemoScope::Coding, Some(first), "On a coding")).unwrap();
    let set = p.add_code_set("Core", "").unwrap();
    p.set_code_in_set(set, trust_id, true).unwrap();
    p.set_code_in_set(set, fear_id, true).unwrap();

    let (bytes, report) = export_qdpx(&p).unwrap();
    // PRISMQDA_KEEP_QDPX=<file> keeps the file, so that its project.qde can be checked against the standard's schema by hand.
    if let Ok(keep) = std::env::var("PRISMQDA_KEEP_QDPX") {
        std::fs::write(keep, &bytes).unwrap();
    }
    assert!(report.carried.iter().any(|l| l.contains("4 codes")), "{report:?}");
    assert!(report.carried.iter().any(|l| l.contains("2 links between passages")), "{report:?}");
    assert!(!report.left_out.is_empty(), "what the file does not hold is always said");

    // a ZIP with the project file and a flat folder of sources, named by GUID
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect();
    assert!(names.contains(&"project.qde".to_string()), "{names:?}");
    assert!(names.iter().filter(|n| n.starts_with("sources/")).all(|n| !n[8..].contains('/')), "the folder is flat: {names:?}");
    assert!(names.iter().any(|n| n.ends_with(".png")) && names.iter().any(|n| n.ends_with(".txt")));

    // exporting twice gives the same file contents (the identifiers are repeatable)
    let (again, _) = export_qdpx(&p).unwrap();
    let qde = |b: &[u8]| {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(b)).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("project.qde").unwrap(), &mut s).unwrap();
        s
    };
    assert_eq!(qde(&bytes), qde(&again));

    // every identifier in the file is its own: a GUID names exactly one thing
    let file = qde(&bytes);
    let guids: Vec<&str> = file.split(" guid=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    let unique: std::collections::BTreeSet<&str> = guids.iter().copied().collect();
    assert!(guids.len() > 15, "{} identifiers", guids.len());
    assert_eq!(guids.len(), unique.len(), "two things share an identifier");

    // read back into a new project
    let target = dir.path().join("b.pqda");
    let back_report = import_qdpx(&bytes, &target, None).unwrap();
    assert!(back_report.carried.iter().any(|l| l.contains("4 codes")), "{back_report:?}");
    assert!(back_report.carried.iter().any(|l| l.contains("2 links between passages")), "{back_report:?}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    assert_eq!(q.info().unwrap().name, "Round trip");

    let codes = q.list_codes().unwrap();
    let find = |n: &str| codes.iter().find(|c| c.name == n).unwrap_or_else(|| panic!("no code {n}"));
    assert_eq!(find("Experience").kind, "theme", "a code that cannot be applied comes back as a theme");
    assert_eq!(find("Experience").color, "#aa3355");
    assert_eq!(find("Trust").parent_id, Some(find("Experience").id), "the tree is kept");
    assert_eq!(find("Fear").parent_id, Some(find("Experience").id));
    assert_eq!(find("Place").parent_id, None);
    let t = find("Trust");
    assert_eq!((t.definition.as_str(), t.inclusion_criteria.as_str(), t.exclusion_criteria.as_str()), ("Feeling safe", "Says they trust someone", "Talks about fear"));

    let coders = q.list_coders().unwrap();
    for name in ["Ana", "Bo"] {
        assert!(coders.iter().any(|c| c.name == name), "coder {name}");
    }

    let sources = q.list_sources().unwrap();
    let qa = sources.iter().find(|s| s.name == "Interview 1").expect("the document is back");
    assert_eq!(q.source_text(qa.id).unwrap(), text, "the text is the same, letter for letter");
    let mut got: Vec<(i64, i64, String, String, String)> =
        q.list_codings(qa.id).unwrap().iter().map(|c| (c.start, c.end, codes.iter().find(|x| x.id == c.code_id).unwrap().name.clone(), coder_name(&coders, c.coder_id), c.rationale.clone())).collect();
    got.sort();
    let mut want = vec![
        (t0, t1, "Trust".to_string(), "Ana".to_string(), String::new()),
        (t0, t1, "Trust".to_string(), "Bo".to_string(), String::new()),
        (f0, f1, "Fear".to_string(), "Ana".to_string(), "She says it herself".to_string()),
    ];
    want.sort();
    assert_eq!(got, want, "every coding is on the same words, from the same coder, with its reason");
    let words: String = q.source_text(qa.id).unwrap().chars().skip(t0 as usize).take((t1 - t0) as usize).collect();
    assert_eq!(words, "trust");

    // the picture and its area
    let qp = sources.iter().find(|s| s.name == "photo").or_else(|| sources.iter().find(|s| s.image.is_some())).expect("the picture is back");
    assert!(qp.image.is_some());
    let areas = q.list_regions(qp.id).unwrap();
    assert_eq!(areas.len(), 1);
    let (x0, y0, x1, y1) = areas[0].shape.bbox();
    for (got, want) in [(x0, 0.1), (y0, 0.2), (x1, 0.5), (y1, 0.6)] {
        assert!((got - want).abs() < 0.01, "{got} against {want}");
    }

    // the links, between the same passages, with the same relations
    let links = q.list_passage_links(None).unwrap();
    assert_eq!(links.len(), 2, "{links:?}");
    let one = links.iter().find(|l| l.relation == "is undone by").expect("the first link");
    assert_eq!((one.from.text.as_str(), one.to.text.as_str()), ("trust", "fear"));
    let two = links.iter().find(|l| l.relation == "echoes").expect("the second link");
    assert_eq!(two.from.text.as_str(), "fear");
    assert_eq!(two.to.kind, "image", "a link to an area of a picture comes back as one");

    // the case, its variables, and its document
    let data = q.case_data().unwrap();
    assert_eq!(data.cases.len(), 1);
    assert_eq!(data.cases[0].name, "Participant 1");
    let value = |n: &str| data.variables.iter().position(|v| v.name == n).and_then(|i| data.values[0][i].clone());
    assert_eq!(value("age").as_deref(), Some("34"));
    assert_eq!(value("group").as_deref(), Some("A"));
    assert_eq!(q.list_cases().unwrap()[0].source_ids, vec![qa.id]);

    // memos and the set
    let memos = q.list_memos(None, None).unwrap();
    assert_eq!(memos.len(), 4);
    let scope_of = |title: &str| memos.iter().find(|m| m.title == title).map(|m| m.scope.clone()).unwrap_or_default();
    assert_eq!(scope_of("Plan"), "project");
    assert_eq!(scope_of("On the interview"), "source");
    assert_eq!(scope_of("On trust"), "code");
    assert_eq!(scope_of("On a coding"), "coding");
    let sets = q.list_code_sets().unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].1, "Core");
    assert_eq!(sets[0].2.len(), 2);
}

/// A QDPX made by hand in the shape of the standard's example, so that the reading of the standard is tested and not only our own writing.
fn handmade(project_file: &str, folder: &str, qde: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(project_file, options).unwrap();
    zip.write_all(qde.as_bytes()).unwrap();
    for (name, data) in files {
        zip.start_file(format!("{folder}/{name}"), options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_selection_is_counted_in_code_points_from_nought_with_its_end_left_out_as_the_standards_example_has_it() {
    // The standard's example: the selection "Oh, that was great. See you tomorrow? Eleven thirty. Mm." runs from 139 to 195, which is its 56 characters.
    let sentence = "Oh, that was great. See you tomorrow? Eleven thirty. Mm.";
    let text = format!("{}{sentence} And then she left.", "x".repeat(139));
    let qde = r##"<?xml version="1.0" encoding="utf-8" standalone="yes"?>
<Project xmlns="urn:QDA-XML:project:1.0" name="Standard example" origin="Another program">
  <Users><User guid="AD68FBE7-E1EE-4A82-A279-23CC698C89EB" name="Researcher"/></Users>
  <CodeBook><Codes>
    <Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="Farewell" isCodable="true" color="#f0a"><Description>Saying goodbye</Description></Code>
  </Codes></CodeBook>
  <Sources>
    <TextSource plainTextPath="internal://C8A57C71-D156-4A94-8407-2BD2FA57E2A4.txt" guid="57C6099B-934F-4C86-90F2-CC6C2F2A3CC3" name="1994 Diet Coke.docx" creatingUser="AD68FBE7-E1EE-4A82-A279-23CC698C89EB">
      <PlainTextSelection startPosition="139" endPosition="195" guid="4191294A-E287-4510-95B0-69BCE4638E56" name="Oh, that was great.">
        <Description>The words they use</Description>
        <Coding creatingUser="AD68FBE7-E1EE-4A82-A279-23CC698C89EB" guid="445A6AEC-1400-4502-919D-0867832FF211"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding>
      </PlainTextSelection>
    </TextSource>
  </Sources>
</Project>"##;
    // the project file and the folder are named as the standard's own example names them: in capitals
    let bytes = handmade("Project.QDE", "Sources", qde, &[("C8A57C71-D156-4A94-8407-2BD2FA57E2A4.txt", text.as_bytes())]);
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.pqda");
    let report = import_qdpx(&bytes, &target, None).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("1 coding")), "{report:?}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let src = &q.list_sources().unwrap()[0];
    let codings = q.list_codings(src.id).unwrap();
    assert_eq!(codings.len(), 1);
    assert_eq!((codings[0].start, codings[0].end), (139, 195));
    let words: String = q.source_text(src.id).unwrap().chars().skip(139).take(56).collect();
    assert_eq!(words, sentence, "the words are exactly the selection");
    assert_eq!(codings[0].rationale, "The words they use");
    let code = &q.list_codes().unwrap()[0];
    assert_eq!((code.name.as_str(), code.color.as_str(), code.definition.as_str()), ("Farewell", "#ff00aa", "Saying goodbye"), "a three-digit color is read");
}

#[test]
fn a_text_that_is_not_in_normal_form_keeps_its_codings_on_the_same_words() {
    // "e" and a combining acute accent are two code points in the file and one in the project.
    let text = "Cafe\u{301} was open. The cafe\u{301} was warm.";
    let (s, e) = at(text, "was warm");
    let qde = format!(
        r##"<Project xmlns="urn:QDA-XML:project:1.0" name="Normal form">
  <CodeBook><Codes><Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="Warmth" isCodable="true"/></Codes></CodeBook>
  <Sources><TextSource plainTextPath="internal://A.txt" guid="57C6099B-934F-4C86-90F2-CC6C2F2A3CC3" name="Cafe">
    <PlainTextSelection startPosition="{s}" endPosition="{e}" guid="4191294A-E287-4510-95B0-69BCE4638E56"><Coding guid="445A6AEC-1400-4502-919D-0867832FF211"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding></PlainTextSelection>
  </TextSource></Sources>
</Project>"##
    );
    let bytes = handmade("project.qde", "sources", &qde, &[("A.txt", text.as_bytes())]);
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("n.pqda");
    let report = import_qdpx(&bytes, &target, None).unwrap();
    assert!(report.left_out.iter().any(|l| l.contains("normal form")), "{report:?}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let src = &q.list_sources().unwrap()[0];
    let stored = q.source_text(src.id).unwrap();
    assert!(stored.contains("Caf\u{e9} was open"), "the text is in normal form");
    let c = &q.list_codings(src.id).unwrap()[0];
    let words: String = stored.chars().skip(c.start as usize).take((c.end - c.start) as usize).collect();
    assert_eq!(words, "was warm");
}

#[test]
fn a_link_to_one_of_several_codings_of_the_same_words_comes_back_to_that_coding() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "s.pqda", "Shared words");
    let ana = p.add_coder(&NewCoder::human("Ana")).unwrap();
    let trust_id = p.add_code(&NewCode::new("Trust")).unwrap();
    let place_id = p.add_code(&NewCode::new("Place")).unwrap();
    let fear_id = p.add_code(&NewCode::new("Fear")).unwrap();
    let text = "I trust them, and yet fear comes. The place was quiet.";
    let a = p.add_source(&NewSource::new("Talk", SourceType::Document, text)).unwrap();
    let (t0, t1) = at(text, "trust");
    let (f0, f1) = at(text, "fear");
    // two codings of the same words: the format keeps them in one selection, and a link names a selection
    let c_trust = p.apply_coding(&NewCoding::new(a, t0, t1, trust_id, ana)).unwrap();
    let c_place = p.apply_coding(&NewCoding::new(a, t0, t1, place_id, ana)).unwrap();
    let c_fear = p.apply_coding(&NewCoding::new(a, f0, f1, fear_id, ana)).unwrap();
    // the second of the two codings is an end of one link, and the two codings are joined to each other by another
    p.add_passage_link(c_fear, c_place, "echoes").unwrap();
    p.add_passage_link(c_trust, c_place, "goes with").unwrap();

    let (bytes, report) = export_qdpx(&p).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("2 links between passages")), "both links are in the file: {report:?}");
    assert!(!report.left_out.iter().any(|l| l.contains("same words")), "a link between two codings of the same words is no longer left out: {:?}", report.left_out);

    let target = dir.path().join("t.pqda");
    let back = import_qdpx(&bytes, &target, None).unwrap();
    assert!(!back.left_out.iter().any(|l| l.contains("more than one coding")), "each end is its own selection, so nothing was joined to a first coding: {:?}", back.left_out);
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let links = q.list_passage_links(None).unwrap();
    assert_eq!(links.len(), 2, "{links:?}");
    let echoes = links.iter().find(|l| l.relation == "echoes").expect("the first link");
    assert_eq!((echoes.from.code_name.as_str(), echoes.to.code_name.as_str()), ("Fear", "Place"), "it ends at the Place coding, not at the first coding of those words");
    let goes = links.iter().find(|l| l.relation == "goes with").expect("the second link");
    assert_eq!((goes.from.code_name.as_str(), goes.to.code_name.as_str()), ("Trust", "Place"));
}

#[test]
fn a_link_to_words_with_several_codings_in_a_file_from_another_program_is_joined_to_the_first_and_said() {
    let qde = r##"<Project xmlns="urn:QDA-XML:project:1.0" name="Shared">
  <CodeBook><Codes>
    <Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="A" isCodable="true"/>
    <Code guid="E299EFAE-5779-46DC-9827-A3D1A5D544BC" name="B" isCodable="true"/>
  </Codes></CodeBook>
  <Sources>
    <TextSource guid="33333333-3333-4333-8333-333333333333" name="Doc" plainTextPath="internal://A.txt">
      <PlainTextSelection guid="66666666-6666-4666-8666-666666666666" startPosition="0" endPosition="7">
        <Coding guid="77777777-7777-4777-8777-777777777777"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding>
        <Coding guid="88888888-8888-4888-8888-888888888888"><CodeRef targetGUID="E299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding>
      </PlainTextSelection>
      <PlainTextSelection guid="99999999-9999-4999-8999-999999999999" startPosition="8" endPosition="18">
        <Coding guid="AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding>
      </PlainTextSelection>
    </TextSource>
  </Sources>
  <Links><Link guid="55555555-5555-4555-8555-555555555555" name="explains" originGUID="66666666-6666-4666-8666-666666666666" targetGUID="99999999-9999-4999-8999-999999999999"/></Links>
</Project>"##;
    let bytes = handmade("project.qde", "sources", qde, &[("A.txt", b"One two three four five six.")]);
    let dir = tempfile::tempdir().unwrap();
    let report = import_qdpx(&bytes, &dir.path().join("o.pqda"), None).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("1 link between passages")), "{report:?}");
    let left = report.left_out.join("\n");
    assert!(left.contains("more than one coding") && left.contains("first coding"), "the person is told: {left}");
}

#[test]
fn a_value_a_file_puts_on_a_document_goes_to_the_case_that_holds_it_or_to_a_case_made_for_it() {
    let qde = r##"<Project xmlns="urn:QDA-XML:project:1.0" name="Document values">
  <Variables>
    <Variable guid="A1111111-1111-4111-8111-111111111111" name="Wave" typeOfVariable="Integer"/>
    <Variable guid="A2222222-2222-4222-8222-222222222222" name="Site" typeOfVariable="Text"/>
  </Variables>
  <Cases>
    <Case guid="C1111111-1111-4111-8111-111111111111" name="Ana">
      <VariableValue><VariableRef targetGUID="A2222222-2222-4222-8222-222222222222"/><TextValue>North</TextValue></VariableValue>
      <SourceRef targetGUID="33333333-3333-4333-8333-333333333333"/>
    </Case>
  </Cases>
  <Sources>
    <TextSource guid="33333333-3333-4333-8333-333333333333" name="Talk 1" plainTextPath="internal://A.txt">
      <VariableValue><VariableRef targetGUID="A1111111-1111-4111-8111-111111111111"/><IntegerValue>2</IntegerValue></VariableValue>
      <VariableValue><VariableRef targetGUID="A2222222-2222-4222-8222-222222222222"/><TextValue>South</TextValue></VariableValue>
    </TextSource>
    <TextSource guid="44444444-4444-4444-8444-444444444444" name="Talk 2" plainTextPath="internal://B.txt">
      <VariableValue><VariableRef targetGUID="A1111111-1111-4111-8111-111111111111"/><IntegerValue>3</IntegerValue></VariableValue>
      <VariableValue><VariableRef targetGUID="FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF"/><TextValue>nobody defined me</TextValue></VariableValue>
    </TextSource>
  </Sources>
</Project>"##;
    let bytes = handmade("project.qde", "sources", qde, &[("A.txt", b"First talk."), ("B.txt", b"Second talk.")]);
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("o.pqda");
    let report = import_qdpx(&bytes, &target, None).unwrap();
    let carried = report.carried.join("\n");
    assert!(carried.contains("2 variable values on documents") && carried.contains("1 case made"), "{carried}");
    let left = report.left_out.join("\n");
    assert!(left.contains("2 variable values on documents that could not be put on a case"), "a value for a variable the file does not define, and one the case already had: {left}");

    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let data = q.case_data().unwrap();
    let mut names: Vec<&str> = data.cases.iter().map(|c| c.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Ana", "Talk 2"], "a document in no case got a case named for it");
    let value = |case: &str, var: &str| {
        let row = data.cases.iter().position(|c| c.name == case).unwrap();
        data.variables.iter().position(|v| v.name == var).and_then(|i| data.values[row][i].clone())
    };
    assert_eq!(value("Ana", "Wave").as_deref(), Some("2"), "the value on the document is on the case that holds it");
    assert_eq!(value("Ana", "Site").as_deref(), Some("North"), "and the case's own value is kept");
    assert_eq!(value("Talk 2", "Wave").as_deref(), Some("3"));
}

#[test]
fn a_concept_map_is_written_as_a_graph_and_comes_back_with_its_boxes_places_and_lines() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "g.pqda", "Maps");
    let trust = p.add_code(&NewCode::new("Trust")).unwrap();
    let fear = p.add_code(&NewCode::new("Fear")).unwrap();
    let case = p.add_case("Participant 1", "").unwrap();
    let memo = p.add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Standalone, target_id: None, phase: None, title: "Hunch".to_string(), body: "Trust and fear go together".to_string() }).unwrap();
    let map = p.create_concept_map("Ideas").unwrap();
    let n_trust = p.add_map_node(map, "code", Some(trust), "", 10.0, 20.0).unwrap();
    let n_fear = p.add_map_node(map, "code", Some(fear), "", 210.0, 20.0).unwrap();
    let n_case = p.add_map_node(map, "case", Some(case.id), "", 10.0, 120.0).unwrap();
    let n_memo = p.add_map_node(map, "memo", Some(memo), "", 210.0, 120.0).unwrap();
    let n_note = p.add_map_node(map, "note", None, "A free thought", 110.0, 220.0).unwrap();
    p.add_map_link(map, n_trust.id, n_fear.id, "is undone by", true).unwrap();
    p.add_map_link(map, n_note.id, n_case.id, "comes from", false).unwrap();
    p.add_map_link(map, n_memo.id, n_trust.id, "explains", true).unwrap();

    let (bytes, report) = export_qdpx(&p).unwrap();
    // PRISMQDA_KEEP_QDPX_GRAPH=<file> keeps this file, so that its project.qde can be checked against the standard's schema by hand.
    if let Ok(keep) = std::env::var("PRISMQDA_KEEP_QDPX_GRAPH") {
        std::fs::write(keep, &bytes).unwrap();
    }
    assert!(report.carried.iter().any(|l| l.contains("1 concept map (graphs) with 5 boxes and 3 lines")), "{report:?}");
    assert!(!report.left_out.iter().any(|l| l.contains("concept maps")), "concept maps are no longer left out: {:?}", report.left_out);

    let target = dir.path().join("h.pqda");
    let back = import_qdpx(&bytes, &target, None).unwrap();
    assert!(back.carried.iter().any(|l| l.contains("1 concept map (graphs) with 5 boxes and 3 lines")), "{back:?}");
    assert!(!back.left_out.iter().any(|l| l.contains("in graphs")), "nothing in the graph was left: {:?}", back.left_out);
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let maps = q.list_concept_maps().unwrap();
    assert_eq!(maps.len(), 1);
    let m = q.get_concept_map(maps[0].id).unwrap();
    assert_eq!(m.name, "Ideas");
    let mut boxes: Vec<(String, String, f64, f64)> = m.nodes.iter().map(|n| (n.kind.clone(), n.label.clone(), n.x, n.y)).collect();
    boxes.sort_by(|a, b| a.1.cmp(&b.1));
    assert_eq!(
        boxes,
        [
            ("note".to_string(), "A free thought".to_string(), 110.0, 220.0),
            ("code".to_string(), "Fear".to_string(), 210.0, 20.0),
            ("memo".to_string(), "Hunch".to_string(), 210.0, 120.0),
            ("case".to_string(), "Participant 1".to_string(), 10.0, 120.0),
            ("code".to_string(), "Trust".to_string(), 10.0, 20.0),
        ],
        "each box is the same kind of thing, with the same words, in the same place"
    );
    let label_of = |id: i64| m.nodes.iter().find(|n| n.id == id).unwrap().label.clone();
    let mut lines: Vec<(String, String, String, bool)> = m.links.iter().map(|l| (label_of(l.from_node), l.label.clone(), label_of(l.to_node), l.directed)).collect();
    lines.sort();
    assert_eq!(
        lines,
        [
            ("A free thought".to_string(), "comes from".to_string(), "Participant 1".to_string(), false),
            ("Hunch".to_string(), "explains".to_string(), "Trust".to_string(), true),
            ("Trust".to_string(), "is undone by".to_string(), "Fear".to_string(), true),
        ],
        "each line joins the same two boxes, with the same words, and an arrow where it had one"
    );
}

#[test]
fn a_graph_from_another_program_becomes_a_map_and_what_does_not_come_in_is_said() {
    let qde = r##"<Project xmlns="urn:QDA-XML:project:1.0" name="Their graph">
  <CodeBook><Codes><Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="Hope" isCodable="true"/></Codes></CodeBook>
  <Sources><TextSource guid="33333333-3333-4333-8333-333333333333" name="Diary" plainTextPath="internal://A.txt"/></Sources>
  <Graphs>
    <Graph guid="11111111-1111-4111-8111-111111111111" name="Sketch">
      <Vertex guid="21111111-1111-4111-8111-111111111111" representedGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="Hope" firstX="40" firstY="50" shape="Oval" color="#ff0000"/>
      <Vertex guid="22222222-2222-4222-8222-222222222222" representedGUID="33333333-3333-4333-8333-333333333333" firstX="140" firstY="50"/>
      <Vertex guid="23333333-3333-4333-8333-333333333333" name="A thought" firstX="90" firstY="150"/>
      <Edge guid="31111111-1111-4111-8111-111111111111" name="needs" sourceVertex="21111111-1111-4111-8111-111111111111" targetVertex="22222222-2222-4222-8222-222222222222" direction="OneWay"/>
      <Edge guid="32222222-2222-4222-8222-222222222222" name="loops" sourceVertex="23333333-3333-4333-8333-333333333333" targetVertex="23333333-3333-4333-8333-333333333333"/>
      <Edge guid="33333333-3333-4333-8333-333333333333" name="nowhere" sourceVertex="23333333-3333-4333-8333-333333333333" targetVertex="99999999-9999-4999-8999-999999999999"/>
    </Graph>
  </Graphs>
</Project>"##;
    let bytes = handmade("project.qde", "sources", qde, &[("A.txt", b"Dear diary.")]);
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("o.pqda");
    let report = import_qdpx(&bytes, &target, None).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("1 concept map (graphs) with 3 boxes and 1 line")), "{report:?}");
    let left = report.left_out.join("\n");
    assert!(left.contains("1 box in graphs that stand for a document"), "a box for a document is a free note, and is said: {left}");
    assert!(left.contains("2 lines in graphs that join a box to itself or to a box that is not there"), "{left}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let m = q.get_concept_map(q.list_concept_maps().unwrap()[0].id).unwrap();
    let mut kinds: Vec<(String, String)> = m.nodes.iter().map(|n| (n.kind.clone(), n.label.clone())).collect();
    kinds.sort();
    assert_eq!(kinds, [("code".to_string(), "Hope".to_string()), ("note".to_string(), "A thought".to_string()), ("note".to_string(), "Diary".to_string())], "a document's box is a note named for the document");
    assert_eq!(m.links.len(), 1);
    assert!(m.links[0].directed && m.links[0].label == "needs");
}

#[test]
fn a_recording_and_the_stretches_coded_on_it_are_written_and_come_back_on_the_same_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "r.pqda", "Recordings");
    let ana = p.add_coder(&NewCoder::human("Ana")).unwrap();
    let trust = p.add_code(&NewCode::new("Trust")).unwrap();
    let text = "I trust them. They were kind.";
    let doc = p.add_source(&NewSource::new("Interview", SourceType::Document, text)).unwrap();
    let (t0, t1) = at(text, "trust");
    p.apply_coding(&NewCoding::new(doc, t0, t1, trust, ana)).unwrap();
    let file = dir.path().join("interview.mp3");
    std::fs::write(&file, b"not really an mp3, but a file").unwrap();
    p.link_media(doc, &file.to_string_lossy()).unwrap();
    let mut stretch = NewRegion::new(doc, Shape::Span { start_ms: 1000, end_ms: 5000 }, trust, ana);
    stretch.rationale = "She sounds sure".to_string();
    p.apply_region(&stretch).unwrap();

    let (bytes, report) = export_qdpx(&p).unwrap();
    // PRISMQDA_KEEP_QDPX_REC=<file> keeps this file, so that its project.qde can be checked against the standard's schema by hand.
    if let Ok(keep) = std::env::var("PRISMQDA_KEEP_QDPX_REC") {
        std::fs::write(keep, &bytes).unwrap();
    }
    assert!(report.carried.iter().any(|l| l.contains("1 linked recording (as audio or video sources) with 1 stretch coded on them")), "{report:?}");
    assert!(report.left_out.iter().any(|l| l.contains("recording") && l.contains("not inside it")), "the recording is not in the file, and that is said: {:?}", report.left_out);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut qde = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.qde").unwrap(), &mut qde).unwrap();
    assert!(qde.contains("<AudioSource") && qde.contains("<AudioSelection") && qde.contains("begin=\"1000\"") && qde.contains("end=\"5000\""), "{qde}");
    assert!(qde.contains("absolute:///"), "the path is written as the format writes one");

    let target = dir.path().join("s.pqda");
    let back = import_qdpx(&bytes, &target, None).unwrap();
    assert!(back.carried.iter().any(|l| l.contains("1 recording linked, with 1 stretch coded on them")), "{back:?}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let sources = q.list_sources().unwrap();
    assert_eq!(sources.len(), 1, "the recording is not a second document: {sources:?}");
    let media = q.list_media().unwrap();
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].source_id, sources[0].id, "the recording is on its document");
    assert_eq!(std::fs::canonicalize(&media[0].path).unwrap(), std::fs::canonicalize(&file).unwrap(), "and it is the same file");
    let areas = q.list_regions(sources[0].id).unwrap();
    assert_eq!(areas.len(), 1);
    assert!(matches!(areas[0].shape, Shape::Span { start_ms: 1000, end_ms: 5000 }), "{:?}", areas[0].shape);
    assert_eq!(areas[0].rationale, "She sounds sure");
    assert_eq!(q.list_codings(sources[0].id).unwrap().len(), 1, "the coding on the words is still there");
}

#[test]
fn a_recording_from_another_program_becomes_a_document_with_the_recording_linked_and_what_is_missing_is_said() {
    let qde = r##"<Project xmlns="urn:QDA-XML:project:1.0" name="Their recordings">
  <CodeBook><Codes><Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="Hope" isCodable="true"/></Codes></CodeBook>
  <Sources>
    <AudioSource guid="33333333-3333-4333-8333-333333333333" name="Talk" path="internal://talk.mp3">
      <Transcript guid="44444444-4444-4444-8444-444444444444"><PlainTextContent>Good morning. How are you today?</PlainTextContent>
        <SyncPoint guid="55555555-5555-4555-8555-555555555555" timeStamp="0" position="0"/>
        <SyncPoint guid="66666666-6666-4666-8666-666666666666" timeStamp="2500" position="14"/>
        <TranscriptSelection guid="BBBBBBBB-1111-4111-8111-111111111111" fromSyncPoint="55555555-5555-4555-8555-555555555555" toSyncPoint="66666666-6666-4666-8666-666666666666"><Coding guid="CCCCCCCC-1111-4111-8111-111111111111"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding></TranscriptSelection>
      </Transcript>
      <AudioSelection guid="77777777-7777-4777-8777-777777777777" begin="500" end="2000"><Coding guid="88888888-8888-4888-8888-888888888888"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding></AudioSelection>
    </AudioSource>
    <VideoSource guid="99999999-9999-4999-8999-999999999999" name="Silent film" path="absolute:///Z:/nowhere/film.mp4">
      <Description>Two minutes of film.</Description>
      <VideoSelection guid="AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA" begin="0" end="900"><Coding guid="BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding></VideoSelection>
    </VideoSource>
  </Sources>
</Project>"##;
    let bytes = handmade("project.qde", "sources", qde, &[("talk.mp3", b"not really an mp3, but a file")]);
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("o.pqda");
    let report = import_qdpx(&bytes, &target, None).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("1 recording linked, with 1 stretch coded on them")), "{report:?}");
    let left = report.left_out.join("\n");
    assert!(left.contains("1 recording that could not be found on this computer (Silent film)") && left.contains("stretches coded on it (1 coding)"), "{left}");
    assert!(!left.contains("sync point"), "the sync points were used, so nothing is said of them: {left}");
    assert!(report.carried.iter().any(|l| l.contains("1 transcript with the times of its lines")), "{report:?}");

    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let sources = q.list_sources().unwrap();
    let talk = sources.iter().find(|s| s.name == "Talk").expect("the recording with a transcript is a document");
    assert_eq!(q.source_text(talk.id).unwrap(), "Good morning. How are you today?", "its text is the transcript");
    let media = q.list_media().unwrap();
    assert_eq!(media.len(), 1, "only the recording that was inside the archive could be linked");
    assert_eq!(media[0].source_id, talk.id);
    assert!(media[0].path.contains("o recordings"), "it was written beside the new project, in a folder named for it: {}", media[0].path);
    assert!(std::path::Path::new(&media[0].path).is_file());
    let spans = q.list_regions(talk.id).unwrap();
    assert_eq!(spans.len(), 1);
    assert!(matches!(spans[0].shape, Shape::Span { start_ms: 500, end_ms: 2000 }));
    let units = q.list_units(talk.id).unwrap();
    assert_eq!(units.iter().map(|u| (u.start, u.end, u.time_start_ms, u.time_end_ms)).collect::<Vec<_>>(), [(0, 14, Some(0), Some(2500)), (14, 32, Some(2500), Some(2500))], "a unit from each sync point to the next, with their times");
    let said = q.list_codings(talk.id).unwrap();
    assert_eq!(said.len(), 1, "the selection between the two sync points is a coding of the words between them");
    assert_eq!((said[0].start, said[0].end), (0, 14));
    let film = sources.iter().find(|s| s.name == "Silent film").expect("a recording with no transcript is a document of its own");
    assert!(q.source_text(film.id).unwrap().starts_with("Recording: Silent film"), "{}", q.source_text(film.id).unwrap());
    assert!(q.get_media(film.id).unwrap().is_none(), "with no recording linked, because it was not found");
    assert!(q.list_regions(film.id).unwrap().is_empty());
}

#[test]
fn the_files_of_recordings_can_go_inside_the_file_and_come_back_beside_the_new_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "f.pqda", "Files");
    let ana = p.add_coder(&NewCoder::human("Ana")).unwrap();
    let trust = p.add_code(&NewCode::new("Trust")).unwrap();
    let doc = p.add_source(&NewSource::new("Interview", SourceType::Document, "I trust them.")).unwrap();
    let file = dir.path().join("interview.mp3");
    let audio: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&file, &audio).unwrap();
    p.link_media(doc, &file.to_string_lossy()).unwrap();
    p.apply_region(&NewRegion::new(doc, Shape::Span { start_ms: 100, end_ms: 900 }, trust, ana)).unwrap();

    // without the choice: only where it is, and the person is told there is a choice
    let (_, plain) = export_qdpx(&p).unwrap();
    assert!(plain.left_out.iter().any(|l| l.contains("not inside it") && l.contains("choice")), "{:?}", plain.left_out);

    let (bytes, report) = export_qdpx_with(&p, &QdpxOptions { include_recordings: true }).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("The files of 1 recording inside the file")), "{report:?}");
    assert!(!report.left_out.iter().any(|l| l.contains("not inside it")), "nothing is left out of the file: {:?}", report.left_out);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect();
    let inside = names.iter().find(|n| n.ends_with(".mp3")).expect("the recording is in the archive").clone();
    let mut got = Vec::new();
    std::io::Read::read_to_end(&mut zip.by_name(&inside).unwrap(), &mut got).unwrap();
    assert_eq!(got, audio, "the recording is inside, byte for byte");
    let mut qde = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.qde").unwrap(), &mut qde).unwrap();
    assert!(qde.contains("path=\"internal://") && qde.contains(".mp3\""), "{qde}");

    // read back elsewhere: the recording is written beside the new project, linked, with its stretch
    let elsewhere = tempfile::tempdir().unwrap();
    let target = elsewhere.path().join("g.pqda");
    let back = import_qdpx(&bytes, &target, None).unwrap();
    assert!(back.carried.iter().any(|l| l.contains("1 recording linked, with 1 stretch coded on them")), "{back:?}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let media = q.list_media().unwrap();
    assert_eq!(media.len(), 1);
    assert!(media[0].path.contains("g recordings"), "{}", media[0].path);
    assert_eq!(std::fs::read(&media[0].path).unwrap(), audio, "the same bytes, in a folder named for the new project");
}

/// A one-page PDF with a line of text, whose page is `media` turned `rotate` degrees clockwise.
fn one_page_pdf(media: [i32; 4], rotate: u16) -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 40 Td (Hello PDF world) Tj ET";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [{} {} {} {}] /Rotate {rotate} /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>", media[0], media[1], media[2], media[3]),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut at = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        at.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for a in &at {
        out.extend_from_slice(format!("{a:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

/// A project with one PDF, a code, a coder, and one coded rectangle on the first page; the rectangle as the project keeps it.
fn project_with_a_pdf(dir: &std::path::Path, media: [i32; 4], rotate: u16, rect: (f64, f64, f64, f64)) -> Project {
    let mut p = create(dir, "pdf.pqda", "Pages");
    let ana = p.add_coder(&NewCoder::human("Ana")).unwrap();
    let hope = p.add_code(&NewCode::new("Hope")).unwrap();
    let opened = prismqda_import::open_bytes("paper.pdf", &one_page_pdf(media, rotate), &prismqda_import::options::ImportOptions::default()).unwrap();
    let prismqda_import::Opened::Batch(batch) = opened else { panic!("a PDF is a batch") };
    let doc = p.apply_import(&batch).unwrap().source_ids[0];
    let mut area = NewRegion::new(doc, Shape::Rectangle { x: rect.0, y: rect.1, w: rect.2, h: rect.3 }, hope, ana);
    area.rationale = "A hopeful corner".to_string();
    p.apply_region(&area).unwrap();
    p
}

#[test]
fn a_coded_rectangle_on_a_pdf_page_is_written_in_points_from_the_bottom_left_and_comes_back_where_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let p = project_with_a_pdf(dir.path(), [0, 0, 612, 792], 0, (0.1, 0.2, 0.4, 0.3));
    let (bytes, report) = export_qdpx(&p).unwrap();
    if let Ok(keep) = std::env::var("PRISMQDA_KEEP_QDPX_PDF") {
        std::fs::write(keep, &bytes).unwrap();
    }
    assert!(report.carried.iter().any(|l| l.contains("1 coded rectangle on the pages of PDFs")), "{report:?}");
    assert!(!report.left_out.iter().any(|l| l.contains("rectangle")), "{:?}", report.left_out);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut qde = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.qde").unwrap(), &mut qde).unwrap();
    // 0.1 and 0.5 of 612 across; 0.2 and 0.5 of 792 down from the top, which is 634 and 396 up from the bottom
    assert!(qde.contains("page=\"0\"") && qde.contains("firstX=\"61\"") && qde.contains("secondX=\"306\"") && qde.contains("firstY=\"396\"") && qde.contains("secondY=\"634\""), "{qde}");

    let target = dir.path().join("back.pqda");
    let back = import_qdpx(&bytes, &target, None).unwrap();
    assert!(back.carried.iter().any(|l| l.contains("1 coded rectangle on the pages of PDFs")), "{back:?}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let doc = q.list_sources().unwrap().into_iter().find(|s| s.pdf.is_some()).expect("the PDF is back");
    let areas = q.list_regions(doc.id).unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0].page, 1);
    assert_eq!(areas[0].rationale, "A hopeful corner");
    let (x0, y0, x1, y1) = areas[0].shape.bbox();
    for (got, want) in [(x0, 0.1), (y0, 0.2), (x1, 0.5), (y1, 0.5)] {
        assert!((got - want).abs() < 0.005, "{got} against {want}");
    }
}

#[test]
fn a_rectangle_on_a_page_the_file_turns_comes_back_where_it_was_drawn() {
    let dir = tempfile::tempdir().unwrap();
    // a page 200 by 100 in the file, turned 90 degrees, so it is drawn 100 wide and 200 tall
    let p = project_with_a_pdf(dir.path(), [0, 0, 200, 100], 90, (0.25, 0.1, 0.5, 0.2));
    let (bytes, _) = export_qdpx(&p).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut qde = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.qde").unwrap(), &mut qde).unwrap();
    // drawn from the top-left: 25 to 75 across of 100 and 20 to 60 down of 200; turned back, that is 20 to 60 across of 200 and 25 to 75 up of 100
    assert!(qde.contains("firstX=\"20\"") && qde.contains("secondX=\"60\"") && qde.contains("firstY=\"25\"") && qde.contains("secondY=\"75\""), "{qde}");
    let target = dir.path().join("back.pqda");
    import_qdpx(&bytes, &target, None).unwrap();
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let doc = q.list_sources().unwrap().into_iter().find(|s| s.pdf.is_some()).unwrap();
    let (x0, y0, x1, y1) = q.list_regions(doc.id).unwrap()[0].shape.bbox();
    for (got, want) in [(x0, 0.25), (y0, 0.1), (x1, 0.75), (y1, 0.3)] {
        assert!((got - want).abs() < 0.011, "{got} against {want}");
    }
}

#[test]
fn a_transcript_with_the_times_of_its_lines_is_written_with_sync_points_and_read_back_without_a_second_copy_of_the_words() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "t.pqda", "Timed");
    let ana = p.add_coder(&NewCoder::human("Ana")).unwrap();
    let hope = p.add_code(&NewCode::new("Hope")).unwrap();
    let text = "Good morning. How are you today?";
    let mut first = ImportedUnit::new(0, 13, UnitKind::SpeakerTurn);
    first.time_ms = Some((0, 2500));
    let mut second = ImportedUnit::new(14, 32, UnitKind::SpeakerTurn);
    second.time_ms = Some((2500, 6000));
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Interview".into(),
            source_type: SourceType::Transcript,
            folder: String::new(),
            text: text.into(),
            runs: vec![],
            units: vec![first, second],
            default_unit_kind: UnitKind::SpeakerTurn,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let doc = p.apply_import(&batch).unwrap().source_ids[0];
    let unit = p.list_units(doc).unwrap()[0].clone();
    let mut c = NewCoding::new(doc, 0, 13, hope, ana);
    c.unit_id = Some(unit.id);
    p.apply_coding(&c).unwrap();
    let file = dir.path().join("interview.mp3");
    std::fs::write(&file, b"not really an mp3, but a file").unwrap();
    p.link_media(doc, &file.to_string_lossy()).unwrap();

    let (bytes, report) = export_qdpx(&p).unwrap();
    if let Ok(keep) = std::env::var("PRISMQDA_KEEP_QDPX_SYNC") {
        std::fs::write(keep, &bytes).unwrap();
    }
    assert!(report.carried.iter().any(|l| l.contains("1 transcript with the times of")), "{report:?}");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut qde = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.qde").unwrap(), &mut qde).unwrap();
    assert_eq!(qde.matches("<SyncPoint").count(), 4, "the start and the end of each of the two lines: {qde}");
    assert_eq!(qde.matches("<TranscriptSelection").count(), 1, "only the line that has a coding");
    assert!(qde.contains("timeStamp=\"2500\""));
    // the words of the transcript are a file in the package, and the transcript has a date (QualCoder 4.0 stops on one without)
    let head = qde.split("<Transcript ").nth(1).unwrap().split('>').next().unwrap();
    assert!(head.contains("plainTextPath=\"internal://") && head.contains("creationDateTime=\""), "{head}");
    assert!(zip.file_names().filter(|n| n.as_ref().is_ok_and(|n| n.ends_with(".txt"))).count() >= 2, "the text source and the transcript each have a file");
    let guids: Vec<&str> = qde.split(" guid=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    let unique: std::collections::BTreeSet<&str> = guids.iter().copied().collect();
    assert_eq!(guids.len(), unique.len(), "no identifier is used twice, though the coding is in the text source and in the transcript");

    // read back: one document, and the one coding once
    let target = dir.path().join("u.pqda");
    import_qdpx(&bytes, &target, None).unwrap();
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let sources = q.list_sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(q.list_codings(sources[0].id).unwrap().len(), 1, "the words are read from the text source, not from the transcript as well");
    // and the kind and the units come back as they were, with their times
    assert_eq!(sources[0].source_type, "transcript");
    assert_eq!(sources[0].default_unit_kind, "speaker_turn");
    let units = q.list_units(sources[0].id).unwrap();
    assert_eq!(units.iter().map(|u| (u.start, u.end, u.kind.as_str(), u.time_start_ms, u.time_end_ms)).collect::<Vec<_>>(), [(0, 13, "speaker_turn", Some(0), Some(2500)), (14, 32, "speaker_turn", Some(2500), Some(6000))]);
}

#[test]
fn the_kind_and_the_units_of_a_document_are_carried_and_a_plain_document_carries_none() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "k.pqda", "Kinds");
    // a plain document, made the way an imported text file is
    let opened = prismqda_import::open_bytes("Plain.txt", b"One. Two.", &prismqda_import::options::ImportOptions::default()).unwrap();
    let prismqda_import::Opened::Batch(plain_batch) = opened else { panic!("a text file is a batch") };
    p.apply_import(&plain_batch).unwrap();
    let mut a = ImportedUnit::new(0, 6, UnitKind::SurveyResponse);
    a.speaker = Some("R1".into());
    let mut b = ImportedUnit::new(7, 14, UnitKind::SurveyResponse);
    b.excluded = true;
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Survey".into(),
            source_type: SourceType::Survey,
            folder: String::new(),
            text: "Answer one. Answer 2".into(),
            runs: vec![],
            units: vec![a, b],
            default_unit_kind: UnitKind::SurveyResponse,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap();
    let (bytes, report) = export_qdpx(&p).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("The kind and the units of 1 document")), "only the survey needs it: {report:?}");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut qde = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.qde").unwrap(), &mut qde).unwrap();
    assert_eq!(qde.matches("the kind and the units of this document").count(), 1, "a plain document has no such description");

    let target = dir.path().join("l.pqda");
    let back = import_qdpx(&bytes, &target, None).unwrap();
    assert!(back.carried.iter().any(|l| l.contains("The kind and the units of 1 document (read from its description)")), "{back:?}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let sources = q.list_sources().unwrap();
    let plain = sources.iter().find(|s| s.name == "Plain").unwrap();
    assert_eq!((plain.source_type.as_str(), plain.default_unit_kind.as_str()), ("document", "sentence"));
    let survey = sources.iter().find(|s| s.name == "Survey").unwrap();
    assert_eq!((survey.source_type.as_str(), survey.default_unit_kind.as_str()), ("survey", "survey_response"));
    let units = q.list_units(survey.id).unwrap();
    assert_eq!(units.iter().map(|u| (u.start, u.end, u.speaker.clone(), u.excluded)).collect::<Vec<_>>(), [(0, 6, Some("R1".to_string()), false), (7, 14, None, true)], "the units, with the speaker of one and the exclusion of the other");
}

#[test]
fn what_the_format_holds_and_the_program_has_no_place_for_is_counted_and_named() {
    let qde = r##"<Project xmlns="urn:QDA-XML:project:1.0" name="Odd things">
  <CodeBook><Codes><Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="A" isCodable="true"/></Codes></CodeBook>
  <Sources>
    <VideoSource guid="11111111-1111-4111-8111-111111111111" name="Group interview" path="absolute:///C:/Interviews/group.mp4">
      <VideoSelection guid="22222222-2222-4222-8222-222222222222" begin="1000" end="5000"/>
    </VideoSource>
    <TextSource guid="33333333-3333-4333-8333-333333333333" name="Styled only" richTextPath="internal://nothing.docx"/>
    <PDFSource guid="44444444-4444-4444-8444-444444444444" name="Report" path="internal://missing.pdf"/>
  </Sources>
  <Links><Link guid="55555555-5555-4555-8555-555555555555" name="explains" originGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC" targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Links>
</Project>"##;
    let bytes = handmade("project.qde", "sources", qde, &[]);
    let dir = tempfile::tempdir().unwrap();
    let report = import_qdpx(&bytes, &dir.path().join("o.pqda"), None).unwrap();
    let left = report.left_out.join("\n");
    assert!(left.contains("could not be found on this computer") && left.contains("Group interview"), "a recording that is not here is named: {left}");
    assert!(left.contains("no plain text"), "{left}");
    assert!(left.contains("Report"), "a PDF that is not in the archive is named: {left}");
    assert!(left.contains("1 link"), "{left}");
}

#[test]
fn files_that_are_not_qdpx_are_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let t = |bytes: Vec<u8>, name: &str| import_qdpx(&bytes, &dir.path().join(name), None).unwrap_err().to_string();
    assert!(t(b"just some text".to_vec(), "a.pqda").contains("not a ZIP"));
    assert!(t(handmade("something.xml", "sources", "<a/>", &[]), "b.pqda").contains("no project file"));
    assert!(t(handmade("project.qde", "sources", "<Other/>", &[]), "c.pqda").contains("not a QDPX project"));
    assert!(t(handmade("project.qde", "sources", "<Project><", &[]), "d.pqda").contains("well-formed"));
    let deep = format!("{}{}", "<a>".repeat(400), "</a>".repeat(400));
    assert!(t(handmade("project.qde", "sources", &deep, &[]), "e.pqda").contains("too deeply"));
    // no project is left behind by a refused file
    assert!(!dir.path().join("a.pqda").exists() && !dir.path().join("c.pqda").exists());
    // an entity the file defines is never expanded
    let bomb = r##"<?xml version="1.0"?><!DOCTYPE Project [<!ENTITY lol "lol">]><Project xmlns="urn:QDA-XML:project:1.0" name="&lol;&lol;"/>"##;
    let r = import_qdpx(&handmade("project.qde", "sources", bomb, &[]), &dir.path().join("f.pqda"), None);
    if let Ok(report) = r {
        assert!(!report.carried.is_empty());
        let q = Project::open(dir.path().join("f.pqda"), &OpenOptions::new("test")).unwrap();
        assert!(!q.info().unwrap().name.contains("lol"), "a defined entity is not expanded");
    }
}

#[test]
fn a_project_is_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let qde = r##"<Project xmlns="urn:QDA-XML:project:1.0" name="X"/>"##;
    let bytes = handmade("project.qde", "sources", qde, &[]);
    let target = dir.path().join("x.pqda");
    std::fs::write(&target, b"not yours to replace").unwrap();
    assert!(import_qdpx(&bytes, &target, None).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"not yours to replace");
}

// ---- what QualCoder 4.0 needs of a file, found by opening PrismQDA's file in it and reading its file in PrismQDA (2026-11-12)

fn qde_of(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut qde = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("project.qde").unwrap(), &mut qde).unwrap();
    qde
}

/// The same file with another program named as the one that made it.
fn with_origin(bytes: &[u8], origin: &str) -> Vec<u8> {
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut zout = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).unwrap();
        if f.is_dir() {
            continue;
        }
        let name = f.name().unwrap().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut data).unwrap();
        if name == "project.qde" {
            let text = String::from_utf8(data).unwrap();
            let start = text.find("origin=\"").unwrap() + 8;
            let end = start + text[start..].find('"').unwrap();
            data = format!("{}{}{}", &text[..start], origin, &text[end..]).into_bytes();
        }
        zout.start_file(name, options).unwrap();
        zout.write_all(&data).unwrap();
    }
    zout.finish().unwrap().into_inner()
}

#[test]
fn each_coder_of_the_same_words_has_a_selection_of_their_own_with_the_coder_on_it() {
    // QualCoder 4.0 reads the coder from the selection and nowhere else, and drops a second coder's coding of the same words and code.
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "c.pqda", "Coders");
    let ana = p.add_coder(&NewCoder::human("Ana")).unwrap();
    let bo = p.add_coder(&NewCoder::human("Bo")).unwrap();
    let trust = p.add_code(&NewCode::new("Trust")).unwrap();
    let text = "I trust them, and yet it is hard.";
    let doc = p.add_source(&NewSource::new("Interview", SourceType::Document, text)).unwrap();
    let (s, e) = at(text, "trust");
    p.apply_coding(&NewCoding::new(doc, s, e, trust, ana)).unwrap();
    p.apply_coding(&NewCoding::new(doc, s, e, trust, bo)).unwrap();
    let (bytes, _) = export_qdpx(&p).unwrap();
    let qde = qde_of(&bytes);
    assert_eq!(qde.matches("<PlainTextSelection").count(), 2, "one for each coder: {qde}");
    for chunk in qde.split("<PlainTextSelection").skip(1) {
        let head = &chunk[..chunk.find('>').unwrap()];
        assert!(head.contains("creatingUser=\"") && head.contains("creationDateTime=\""), "the selection names its coder and its time: {head}");
    }
    let target = dir.path().join("back.pqda");
    import_qdpx(&bytes, &target, None).unwrap();
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let coders = q.list_coders().unwrap();
    let src = &q.list_sources().unwrap()[0];
    let mut who: Vec<String> = q.list_codings(src.id).unwrap().iter().map(|c| coder_name(&coders, c.coder_id)).collect();
    who.sort();
    assert_eq!(who, ["Ana", "Bo"]);
}

#[test]
fn a_memo_is_a_file_in_the_package_and_a_picture_name_carries_the_ending_of_its_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = create(dir.path(), "m.pqda", "Names");
    let opened = prismqda_import::open_bytes("photo.png", &png(), &prismqda_import::options::ImportOptions::default()).unwrap();
    let prismqda_import::Opened::Batch(batch) = opened else { panic!("a picture is a batch") };
    let pic = p.apply_import(&batch).unwrap().source_ids[0];
    let name = p.list_sources().unwrap().into_iter().find(|s| s.id == pic).unwrap().name;
    assert!(!name.ends_with(".png"), "PrismQDA names a picture without the ending of its file: {name}");
    p.add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Project, target_id: None, phase: None, title: "Plan".to_string(), body: "Words of the plan".to_string() }).unwrap();
    let (bytes, _) = export_qdpx(&p).unwrap();
    let qde = qde_of(&bytes);
    assert!(qde.contains(&format!("name=\"{name}.png\"")), "the name has the ending: {qde}");
    let note = qde.split("<Note ").nth(1).unwrap().split('>').next().unwrap();
    assert!(note.contains("plainTextPath=\"internal://") && !qde.contains("<PlainTextContent>"), "the words of the memo are a file: {note}");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let path = note.split("plainTextPath=\"internal://").nth(1).unwrap().split('"').next().unwrap().to_string();
    let mut words = String::new();
    std::io::Read::read_to_string(&mut zip.by_name(&format!("sources/{path}")).unwrap(), &mut words).unwrap();
    assert_eq!(words, "Words of the plan");

    // back: the ending comes off again, and the memo has its words
    let target = dir.path().join("back.pqda");
    import_qdpx(&bytes, &target, None).unwrap();
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    assert!(q.list_sources().unwrap().iter().any(|s| s.name == name), "the picture is named {name} again");
    assert!(q.list_memos(None, None).unwrap().iter().any(|m| m.body == "Words of the plan"));
}

fn file_with_text_and_a_selection(origin: &str, text: &str) -> Vec<u8> {
    let qde = format!(
        r##"<?xml version="1.0" encoding="utf-8"?>
<Project xmlns="urn:QDA-XML:project:1.0" name="From elsewhere" origin="{origin}">
  <Users><User guid="AD68FBE7-E1EE-4A82-A279-23CC698C89EB" name="Researcher"/></Users>
  <CodeBook><Codes><Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="Words" isCodable="true"/></Codes></CodeBook>
  <Sources>
    <TextSource plainTextPath="internal://C8A57C71-D156-4A94-8407-2BD2FA57E2A4.txt" guid="57C6099B-934F-4C86-90F2-CC6C2F2A3CC3" name="Notes">
      <PlainTextSelection startPosition="4" endPosition="7" guid="4191294A-E287-4510-95B0-69BCE4638E56" creatingUser="AD68FBE7-E1EE-4A82-A279-23CC698C89EB">
        <Coding guid="445A6AEC-1400-4502-919D-0867832FF211" creatingUser="AD68FBE7-E1EE-4A82-A279-23CC698C89EB"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding>
      </PlainTextSelection>
    </TextSource>
  </Sources>
</Project>"##
    );
    handmade("project.qde", "sources", &qde, &[("C8A57C71-D156-4A94-8407-2BD2FA57E2A4.txt", text.as_bytes())])
}

#[test]
fn text_saved_by_qualcoder_with_windows_line_endings_is_read_as_qualcoder_counted_it() {
    // QualCoder 4.0 counts a line break as one character but saves it as two on Windows. The words "two" are at 4 to 7 as it counted.
    let crlf = "One\r\ntwo three\r\nfour";
    let words = |q: &Project| {
        let src = &q.list_sources().unwrap()[0];
        let text = q.source_text(src.id).unwrap();
        let c = &q.list_codings(src.id).unwrap()[0];
        (text.clone(), text.chars().skip(c.start as usize).take((c.end - c.start) as usize).collect::<String>())
    };
    let dir = tempfile::tempdir().unwrap();

    let from_qualcoder = dir.path().join("a.pqda");
    let report = import_qdpx(&file_with_text_and_a_selection("QualCoder 4.0", crlf), &from_qualcoder, None).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("saved by QualCoder with Windows line endings")), "{report:?}");
    let (text, coded) = words(&Project::open(&from_qualcoder, &OpenOptions::new("test")).unwrap());
    assert_eq!(text, "One\ntwo three\nfour");
    assert_eq!(coded, "two", "the coding is on the words it was made on");

    // a file from any other program is read as the file counts it
    let from_other = dir.path().join("b.pqda");
    let report = import_qdpx(&file_with_text_and_a_selection("Another program", crlf), &from_other, None).unwrap();
    assert!(!report.carried.iter().any(|l| l.contains("QualCoder")), "{report:?}");
    let (text, coded) = words(&Project::open(&from_other, &OpenOptions::new("test")).unwrap());
    assert!(text.contains("\r\n"));
    assert_eq!(coded, "\ntw");
}

#[test]
fn a_pdf_rectangle_in_a_file_from_qualcoder_is_read_from_the_top_left_of_the_page_as_it_writes_it() {
    // PrismQDA writes the standard's bottom-left origin; QualCoder 4.0 writes (and shows) its rectangles from the top-left of the page.
    let dir = tempfile::tempdir().unwrap();
    let p = project_with_a_pdf(dir.path(), [0, 0, 612, 792], 0, (0.1, 0.2, 0.4, 0.3));
    let (bytes, _) = export_qdpx(&p).unwrap();
    let area = |file: &[u8], name: &str| {
        let target = dir.path().join(name);
        import_qdpx(file, &target, None).unwrap();
        let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
        let doc = q.list_sources().unwrap().into_iter().find(|s| s.pdf.is_some()).unwrap();
        q.list_regions(doc.id).unwrap()[0].shape.bbox()
    };
    let standard = area(&bytes, "std.pqda");
    assert!((standard.1 - 0.2).abs() < 0.005 && (standard.3 - 0.5).abs() < 0.005, "{standard:?}");
    // the same numbers, read as QualCoder means them: 396 and 634 points down from the top of a page 792 high
    let qc = area(&with_origin(&bytes, "QualCoder 4.0"), "qc.pqda");
    assert!((qc.0 - 0.0997).abs() < 0.005 && (qc.1 - 0.5).abs() < 0.005 && (qc.2 - 0.5).abs() < 0.005 && (qc.3 - 0.8005).abs() < 0.005, "{qc:?}");
}

#[test]
fn the_person_who_opens_a_project_with_a_coder_of_their_name_from_another_programs_file_is_that_coder() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.pqda");
    import_qdpx(&file_with_text_and_a_selection("Another program", "One two"), &target, None).unwrap();
    let mut q = Project::open(&target, &OpenOptions::new("researcher")).unwrap();
    let imported = q.list_coders().unwrap().into_iter().find(|c| c.name == "Researcher").unwrap();
    let me = q.ensure_human_coder("researcher", "user-researcher").unwrap();
    assert_eq!(me, imported.id, "one coder, not two of one name");
    let coders = q.list_coders().unwrap();
    assert_eq!(coders.iter().filter(|c| c.name.eq_ignore_ascii_case("researcher")).count(), 1);
    assert_eq!(coders.iter().find(|c| c.id == me).unwrap().local_id, "user-researcher");
    // a coder of another name is a new coder
    assert_ne!(q.ensure_human_coder("Someone else", "user-someone-else").unwrap(), me);
}


// ---------------------------------------------------------------- a file written by ELAN 7.1 (its own writer, run on invented material)

const ELAN_FILE: &[u8] = include_bytes!("fixtures/elan-7.1-bus-ride-talk.qdpx");

/// ELAN's file with the folder of its recording pointed at a folder that exists (the file names `C:/Recordings`).
fn elan_file_with_the_recording_in(folder: &std::path::Path) -> Vec<u8> {
    let mut read = zip::ZipArchive::new(std::io::Cursor::new(ELAN_FILE)).unwrap();
    let mut write = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..read.len() {
        let mut entry = read.by_index(i).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
        if name.ends_with(".qde") {
            let there = folder.to_string_lossy().replace('\\', "/").replace(' ', "%20");
            data = String::from_utf8(data).unwrap().replace("C:/Recordings", &there).into_bytes();
        }
        write.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        write.write_all(&data).unwrap();
    }
    write.finish().unwrap().into_inner()
}

#[test]
fn a_file_written_by_elan_is_one_document_with_its_recording_its_times_and_its_codings() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Bus ride talk.wav"), b"not really a recording, but a file").unwrap();
    let bytes = elan_file_with_the_recording_in(dir.path());
    let target = dir.path().join("o.pqda");
    let report = import_qdpx(&bytes, &target, None).unwrap();
    assert!(report.carried.iter().any(|l| l.contains("1 document, picture, or PDF")), "ELAN names the transcript's file twice, and that is one document: {report:?}");
    assert!(report.carried.iter().any(|l| l.contains("1 text source on the same file as a recording's transcript")), "{report:?}");
    assert!(report.carried.iter().any(|l| l.contains("1 recording linked, with 1 stretch coded on them")), "{report:?}");
    assert!(report.carried.iter().any(|l| l.contains("1 transcript with the times of its lines")), "{report:?}");
    assert!(report.carried.iter().any(|l| l.contains("1 set of codes")), "{report:?}");

    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let sources = q.list_sources().unwrap();
    assert_eq!(sources.len(), 1, "{:?}", sources.iter().map(|s| &s.name).collect::<Vec<_>>());
    assert_eq!(sources[0].name, "Bus ride talk", "the %20 of ELAN's name is a space");
    let text = q.source_text(sources[0].id).unwrap();
    assert!(text.starts_with("INT: [00:00:00.000] How do you usually get to work?\nPAR: [00:00:04.200] "), "{text}");
    let media = q.list_media().unwrap();
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].source_id, sources[0].id);
    assert_eq!(media[0].file_name, "Bus ride talk.wav");

    // The codes of the coding tier under Participant are codings of that speaker's words, and no others; the code of the free tier is a stretch.
    let codes = q.list_codes().unwrap();
    let code = |id: i64| codes.iter().find(|c| c.id == id).unwrap().name.clone();
    let coders = q.list_coders().unwrap();
    let coder = |id: i64| coders.iter().find(|c| c.id == id).unwrap().name.clone();
    let codings = q.list_codings(sources[0].id).unwrap();
    let mut said: Vec<(String, String)> = codings.iter().map(|c| (code(c.code_id), text.chars().skip(c.start as usize).take((c.end - c.start) as usize).collect())).collect();
    said.sort();
    assert_eq!(
        said,
        [
            ("Cost".to_string(), "I take the bus, but it is often late and the fare keeps rising.".to_string()),
            ("Reliability".to_string(), "Not really: I missed a connection twice this month. Caf\u{e9} men\u{fc} \u{2615} \u{4e2d}\u{6587}".to_string()),
        ]
    );
    assert!(codings.iter().all(|c| coder(c.coder_id) == "Imported (QDPX)"), "ELAN puts no coder on the codings of words");
    assert_eq!(codes.len(), 4, "the vocabulary's entries are all codes, the one no annotation uses too");
    let spans = q.list_regions(sources[0].id).unwrap();
    assert_eq!(spans.len(), 1);
    assert!(matches!(spans[0].shape, Shape::Span { start_ms: 16000, end_ms: 19800 }));
    assert_eq!((code(spans[0].code_id), coder(spans[0].coder_id)), ("Safety".to_string(), "Sam Okafor".to_string()), "a stretch keeps its coder");
    let units = q.list_units(sources[0].id).unwrap();
    assert!(units.iter().any(|u| u.time_start_ms == Some(0) && u.time_end_ms == Some(4000)), "the first utterance runs from 0 to 4000 ms");
    assert!(units.iter().any(|u| u.time_start_ms == Some(4200) && u.time_end_ms == Some(8800)), "a unit has the times of the utterance: {:?}", units.iter().map(|u| (u.start, u.end, u.time_start_ms, u.time_end_ms)).collect::<Vec<_>>());
}

#[test]
fn a_file_written_by_elan_whose_recording_is_not_on_this_computer_says_so_and_keeps_the_transcript_and_its_codings() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("o.pqda");
    let report = import_qdpx(ELAN_FILE, &target, None).unwrap();
    let left = report.left_out.join("\n");
    assert!(left.contains("1 recording that could not be found on this computer (Bus ride talk)") && left.contains("stretches coded on it (1 coding)"), "{left}");
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    let sources = q.list_sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "Bus ride talk");
    assert_eq!(q.list_codings(sources[0].id).unwrap().len(), 2);
    assert!(q.list_media().unwrap().is_empty());
}

#[test]
fn a_project_file_that_is_not_called_project_qde_is_read_and_one_called_that_is_preferred() {
    let dir = tempfile::tempdir().unwrap();
    let named = |n: &str| format!(r##"<Project xmlns="urn:QDA-XML:project:1.0" name="{n}"/>"##);
    let name_of = |bytes: Vec<u8>, file: &str| {
        let target = dir.path().join(file);
        import_qdpx(&bytes, &target, None).unwrap();
        Project::open(&target, &OpenOptions::new("test")).unwrap().info().unwrap().name
    };
    assert_eq!(name_of(handmade("Their study.qde", "Sources", &named("From ELAN"), &[]), "a.pqda"), "From ELAN");
    // two files with the ending: the one called project.qde wins, wherever it lies; else the one nearest the top
    let options = zip::write::SimpleFileOptions::default();
    let archive = |files: &[(&str, &str)]| {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (file, name) in files {
            zip.start_file(*file, options).unwrap();
            zip.write_all(named(name).as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    };
    assert_eq!(name_of(archive(&[("old/backup.qde", "Backup"), ("top.qde", "Top"), ("deep/er/project.qde", "Named")]), "b.pqda"), "Named");
    assert_eq!(name_of(archive(&[("old/backup.qde", "Backup"), ("top.qde", "Top")]), "c.pqda"), "Top");
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// A recording path written in an exchange file may be a network address. Asking whether it exists would connect to that server and offer this
// computer's sign-in (NTLM) hash, so a path that is not on a drive with a letter is never looked at: the recording is "not found".
// ---------------------------------------------------------------------------------------------------------------------------------------------

fn qdpx_with_one_recording(path_attr: &str, base_path: Option<&str>) -> Vec<u8> {
    let base = base_path.map(|b| format!(" basePath=\"{b}\"")).unwrap_or_default();
    let qde = format!(
        r##"<Project xmlns="urn:QDA-XML:project:1.0" name="Theirs"{base}>
  <CodeBook><Codes><Code guid="D299EFAE-5779-46DC-9827-A3D1A5D544BC" name="Hope" isCodable="true"/></Codes></CodeBook>
  <Sources>
    <AudioSource guid="33333333-3333-4333-8333-333333333333" name="Talk" path="{path_attr}">
      <Description>A talk.</Description>
      <AudioSelection guid="77777777-7777-4777-8777-777777777777" begin="500" end="2000"><Coding guid="88888888-8888-4888-8888-888888888888"><CodeRef targetGUID="D299EFAE-5779-46DC-9827-A3D1A5D544BC"/></Coding></AudioSelection>
    </AudioSource>
  </Sources>
</Project>"##
    );
    handmade("project.qde", "sources", &qde, &[])
}

/// A file on this computer's own disk written as a network address through its administrative share (`\\localhost\C$\...`): reachable
/// with no network at all where administrative shares are on, so a recording linked through it shows that the check, not the lack of a
/// server, is what stops it. `None` when the share is not reachable on this computer (then only the invented-server cases are run).
#[cfg(windows)]
fn through_the_admin_share(file: &std::path::Path) -> Option<String> {
    let text = file.to_string_lossy().to_string();
    let (drive, rest) = (text.chars().next()?, text.get(3..)?);
    let unc = format!(r"\\localhost\{drive}$\{rest}");
    std::fs::metadata(&unc).ok().map(|_| unc)
}

#[cfg(windows)]
#[test]
fn a_recording_named_by_a_network_address_in_an_exchange_file_is_not_found_and_not_looked_at() {
    let dir = tempfile::tempdir().unwrap();
    let mut cases: Vec<(String, Option<&str>)> = vec![
        ("absolute:////example.invalid/s/a.mp3".to_string(), None),
        ("relative://a.mp3".to_string(), Some("//example.invalid/s")),
        (r"\\example.invalid\s\a.mp3".to_string(), None),
        ("//example.invalid/s/a.mp3".to_string(), None),
        (r"\\?\UNC\example.invalid\s\a.mp3".to_string(), None),
    ];
    // The same recording, really there, by a network-style name that needs no network.
    let real = dir.path().join("a.mp3");
    std::fs::write(&real, b"not really an mp3, but a file").unwrap();
    let alias = through_the_admin_share(&real);
    if let Some(unc) = &alias {
        cases.push((format!("absolute://{}", unc.replace('\\', "/")), None));
        cases.push((unc.clone(), None));
    }
    for (n, (path_attr, base)) in cases.iter().enumerate() {
        let bytes = qdpx_with_one_recording(path_attr, *base);
        let target = dir.path().join(format!("o{n}.pqda"));
        let report = import_qdpx(&bytes, &target, None).unwrap();
        let left = report.left_out.join("\n");
        assert!(left.contains("1 recording that could not be found on this computer (Talk)"), "{path_attr} (base {base:?}): {left}");
        let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
        assert!(q.list_media().unwrap().is_empty(), "{path_attr}: nothing was linked");
    }
    // The same file through its own drive letter is found, so the cases above were refused for the address and nothing else.
    let local = qdpx_with_one_recording(&format!("absolute:///{}", real.to_string_lossy().replace('\\', "/")), None);
    let target = dir.path().join("local.pqda");
    let report = import_qdpx(&local, &target, None).unwrap();
    assert!(!report.left_out.join("\n").contains("could not be found"), "{:?}", report.left_out);
    let q = Project::open(&target, &OpenOptions::new("test")).unwrap();
    assert_eq!(q.list_media().unwrap().len(), 1, "a recording on a drive with a letter is still linked");
}
