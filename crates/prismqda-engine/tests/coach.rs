//! Coach mode (decision 199): the questions the rules ask about a person's own coding, worked out by hand on small documents.

use prismqda_engine::{CodeEdit, CoachSpec, Error, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, SourceType, UnitKind, MIN_CODINGS_FOR_ONCE};
use tempfile::TempDir;

fn options() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn import(p: &mut Project, name: &str, text: &str) -> i64 {
    let n = text.chars().count() as i64;
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: name.into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text: text.into(),
            runs: vec![],
            units: vec![ImportedUnit::new(0, n, UnitKind::Paragraph)],
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

struct W {
    p: Project,
    _dir: TempDir,
    coder: i64,
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("coach.pqda"), &NewProject::new("Coach"), &options()).unwrap();
    let coder = p.add_coder(&NewCoder::human("Student")).unwrap();
    W { p, _dir: dir, coder }
}

/// The place (in characters) of the `which`th (from 0) occurrence of `needle` in `text`.
fn at(text: &str, needle: &str, which: usize) -> (i64, i64) {
    let mut from = 0;
    let mut found = 0;
    loop {
        let byte = text[from..].find(needle).unwrap_or_else(|| panic!("{needle} #{which} is not in the text")) + from;
        if found == which {
            let start = text[..byte].chars().count() as i64;
            return (start, start + needle.chars().count() as i64);
        }
        found += 1;
        from = byte + needle.len();
    }
}

fn code_id(w: &W, name: &str) -> i64 {
    w.p.list_codes().unwrap().into_iter().find(|c| c.name == name).unwrap().id
}

fn add_code(w: &mut W, name: &str, definition: &str) -> i64 {
    let id = w.p.add_code(&NewCode::new(name)).unwrap();
    if !definition.is_empty() {
        w.p.update_code(id, &CodeEdit { definition: Some(definition.into()), ..Default::default() }).unwrap();
    }
    id
}

fn put(w: &mut W, source: i64, text: &str, needle: &str, which: usize, code: &str) {
    let (s, e) = at(text, needle, which);
    let id = code_id(w, code);
    w.p.apply_coding(&NewCoding::new(source, s, e, id, w.coder)).unwrap();
}

fn spec(w: &W) -> CoachSpec {
    CoachSpec { coder_id: w.coder, source_ids: vec![] }
}

fn rule<'a>(report: &'a prismqda_engine::CoachReport, name: &str) -> Vec<&'a prismqda_engine::CoachFinding> {
    report.findings.iter().filter(|f| f.rule == name).collect()
}

const TEXT: &str = "The fares are too high and the bus is late. I love the bus but the fares are rising. The airfares did not change. The bus stop is far from my house.";

#[test]
fn a_code_used_without_a_definition_is_asked_about_and_one_with_a_definition_is_not() {
    let mut w = world();
    let s = import(&mut w.p, "Riders", TEXT);
    add_code(&mut w, "Cost", "");
    add_code(&mut w, "Access", "Getting to the service.");
    add_code(&mut w, "Unused", "");
    put(&mut w, s, TEXT, "fares are too high", 0, "Cost");
    put(&mut w, s, TEXT, "far from my house", 0, "Access");
    put(&mut w, s, TEXT, "late", 0, "Cost");
    let report = w.p.coach_check(&spec(&w)).unwrap();
    let found = rule(&report, "no_definition");
    assert_eq!(found.len(), 1, "only a code that was used and has no definition");
    assert_eq!((found[0].codes[0].name.as_str(), found[0].count), ("Cost", 2));
    assert_eq!(report.rules.iter().map(|r| (r.rule.as_str(), r.findings)).collect::<Vec<_>>(), [("no_definition", 1), ("used_once", 0), ("overlap", 0), ("inconsistent", 0)]);
    assert_eq!((report.codings, report.codes_used, report.documents), (3, 2, 1));
}

#[test]
fn a_code_used_once_is_asked_about_only_after_enough_codings_to_make_that_fair() {
    let mut w = world();
    let s = import(&mut w.p, "Riders", TEXT);
    add_code(&mut w, "Cost", "Money.");
    add_code(&mut w, "Rare", "Seldom.");
    put(&mut w, s, TEXT, "fares are too high", 0, "Cost");
    put(&mut w, s, TEXT, "late", 0, "Rare");
    let early = w.p.coach_check(&spec(&w)).unwrap();
    assert!(rule(&early, "used_once").is_empty());
    assert!(early.notes.iter().any(|n| n.contains(&format!("until there are {MIN_CODINGS_FOR_ONCE} codings"))), "{:?}", early.notes);

    // Ten codings in all: the code given to one passage only is asked about.
    let many = "alpha. beta. gamma. delta. epsilon. zeta. eta. theta. iota. kappa.";
    let s2 = import(&mut w.p, "Greek", many);
    for word in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta"] {
        put(&mut w, s2, many, word, 0, "Cost");
    }
    let report = w.p.coach_check(&spec(&w)).unwrap();
    let once = rule(&report, "used_once");
    assert_eq!(once.len(), 1, "{once:?}");
    assert_eq!(once[0].codes[0].name, "Rare");
    assert_eq!(once[0].examples[0].text, "late", "the place where it was used");
    assert!(!report.notes.iter().any(|n| n.contains("until there are")));
}

#[test]
fn two_codes_given_to_the_same_words_again_and_again_are_asked_about_but_a_code_and_its_parent_are_not() {
    let mut w = world();
    let words = "one two three four five six";
    let s = import(&mut w.p, "Words", words);
    for (name, def) in [("Alike A", "A."), ("Alike B", "B."), ("Parent", "P."), ("Child", "C."), ("Once shared A", "x."), ("Once shared B", "y.")] {
        add_code(&mut w, name, def);
    }
    for word in ["one", "two", "three", "four"] {
        put(&mut w, s, words, word, 0, "Alike A");
        put(&mut w, s, words, word, 0, "Alike B");
    }
    // A code and the code it sits under on the same words: that is what a tree is for.
    let (parent, child) = (code_id(&w, "Parent"), code_id(&w, "Child"));
    w.p.move_code(child, Some(parent), None).unwrap();
    for word in ["five", "six", "one"] {
        put(&mut w, s, words, word, 0, "Parent");
        put(&mut w, s, words, word, 0, "Child");
    }
    // Two codes that share two places only are not enough to ask about.
    put(&mut w, s, words, "two", 0, "Once shared A");
    put(&mut w, s, words, "three", 0, "Once shared A");
    put(&mut w, s, words, "two", 0, "Once shared B");
    put(&mut w, s, words, "three", 0, "Once shared B");
    put(&mut w, s, words, "six", 0, "Once shared B");
    let report = w.p.coach_check(&spec(&w)).unwrap();
    let both = rule(&report, "overlap");
    assert_eq!(both.len(), 1, "{both:?}");
    assert_eq!(both[0].codes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Alike A", "Alike B"]);
    assert_eq!((both[0].count, both[0].of), (4, 4), "four places, and the rarer code was given four times");
    assert_eq!(both[0].examples.len(), 3, "at most three places are shown");
}

#[test]
fn words_coded_in_one_place_and_left_uncoded_in_another_are_found_by_whole_words() {
    let mut w = world();
    let s = import(&mut w.p, "Riders", TEXT);
    add_code(&mut w, "Cost", "Money.");
    put(&mut w, s, TEXT, "fares", 0, "Cost");
    let report = w.p.coach_check(&spec(&w)).unwrap();
    let left = rule(&report, "inconsistent");
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!(left[0].phrase.as_deref(), Some("fares"));
    assert_eq!((left[0].count, left[0].of), (1, 1), "one more place, where the words were coded once; 'airfares' is another word");
    assert!(left[0].examples[0].text.contains("fares are rising"), "{:?}", left[0].examples);
    assert_eq!(left[0].examples[0].start, at(TEXT, "fares", 1).0);

    // Coding the second place (with any code) answers the question.
    add_code(&mut w, "Other", "Another.");
    put(&mut w, s, TEXT, "fares", 1, "Other");
    let after = w.p.coach_check(&spec(&w)).unwrap();
    assert!(rule(&after, "inconsistent").is_empty(), "{:?}", rule(&after, "inconsistent"));
}

#[test]
fn another_persons_coding_is_not_the_coachs_business_and_a_missing_coder_or_document_is_refused() {
    let mut w = world();
    let s = import(&mut w.p, "Riders", TEXT);
    add_code(&mut w, "Cost", "");
    let other = w.p.add_coder(&NewCoder::human("Teacher")).unwrap();
    let id = code_id(&w, "Cost");
    let (a, b) = at(TEXT, "late", 0);
    w.p.apply_coding(&NewCoding::new(s, a, b, id, other)).unwrap();
    let mine = w.p.coach_check(&spec(&w)).unwrap();
    assert_eq!((mine.codings, mine.findings.len()), (0, 0), "the student has coded nothing, whatever the teacher did");
    assert!(matches!(w.p.coach_check(&CoachSpec { coder_id: 987_654, source_ids: vec![] }), Err(Error::NotFound { what: "coder", .. })));
    assert!(matches!(w.p.coach_check(&CoachSpec { coder_id: w.coder, source_ids: vec![987_654] }), Err(Error::NotFound { what: "document", .. })));
}

#[test]
fn the_prompt_asks_for_questions_and_leaves_out_what_is_marked_do_not_share_and_is_refused_when_ai_is_off() {
    let mut w = world();
    let s = import(&mut w.p, "Riders", TEXT);
    add_code(&mut w, "Cost", "");
    add_code(&mut w, "Secret", "");
    put(&mut w, s, TEXT, "fares are too high", 0, "Cost");
    put(&mut w, s, TEXT, "late", 0, "Secret");
    let secret = code_id(&w, "Secret");
    w.p.update_code(secret, &CodeEdit { do_not_share: Some(true), ..Default::default() }).unwrap();
    let prompt = w.p.coach_prompt(&spec(&w)).unwrap();
    assert!(prompt.text.contains("Do not tell them which code to use"), "{}", prompt.text);
    assert!(prompt.text.contains("- Cost: (no definition yet)"));
    assert!(prompt.text.contains("\"Cost\" has no definition and was used 1 time(s)."));
    assert!(!prompt.text.contains("Secret"), "a code marked do-not-share is not in the prompt: {}", prompt.text);
    assert_eq!(prompt.findings, 1);
    assert!(prompt.left_out.iter().any(|l| l.contains("do-not-share")));
    w.p.set_ai_mode("off").unwrap();
    assert!(matches!(w.p.coach_prompt(&spec(&w)), Err(Error::Invalid(_))));
    // The checks themselves need no AI.
    assert!(w.p.coach_check(&spec(&w)).is_ok());
}
