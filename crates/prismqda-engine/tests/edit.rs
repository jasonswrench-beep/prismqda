//! Editing a source's text after coding: concrete cases, then randomized properties.

use prismqda_engine::{
    AppliesTo, AttributeSpec, FormatRun, ImportBatch, ImportedSource, ImportedUnit, Level, NewCode, NewCoder, NewCoding, NewProject,
    NewSource, OpenOptions, Project, RunKind, SourceType, UnitKind,
};
use proptest::prelude::*;
use rusqlite::Connection;
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

struct World {
    p: Project,
    source: i64,
    code: i64,
    coder: i64,
}

fn world(dir: &TempDir, text: &str) -> World {
    let mut p = project(dir);
    let source = p.add_source(&NewSource::new("S", SourceType::Document, text)).unwrap();
    let code = p.add_code(&NewCode::new("C")).unwrap();
    let coder = p.add_coder(&NewCoder::human("Ada")).unwrap();
    World { p, source, code, coder }
}

impl World {
    fn code_span(&mut self, s: i64, e: i64) -> i64 {
        self.p.apply_coding(&NewCoding::new(self.source, s, e, self.code, self.coder)).unwrap()
    }

    fn quotes(&self) -> Vec<String> {
        self.p
            .list_codings(self.source)
            .unwrap()
            .iter()
            .map(|c| self.p.text_of_span(self.source, c.start, c.end).unwrap())
            .collect()
    }
}

#[test]
fn codings_follow_their_words_through_an_edit_and_the_revision_counts_up() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir, "I felt welcome from the first day.");
    w.code_span(2, 14); // "felt welcome"
    w.code_span(24, 33); // "first day"
    assert_eq!(w.quotes(), ["felt welcome", "first day"]);

    let out = w.p.replace_source_text(w.source, "Honestly I felt very welcome from the first day.").unwrap();
    assert_eq!((out.text_revision, out.codings_removed, out.codings_moved), (2, 0, 2));
    assert_eq!(w.quotes(), ["felt very welcome", "first day"], "an insertion inside the first coding joins it");
    assert_eq!(w.p.source_text(w.source).unwrap(), "Honestly I felt very welcome from the first day.");
    assert_eq!(w.p.list_sources().unwrap()[0].text_revision, 2);
    assert_eq!(w.p.list_sources().unwrap()[0].characters, 48);
}

#[test]
fn a_no_change_edit_does_nothing_and_a_bad_edit_is_refused() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir, "Some text here.");
    let events = w.p.events(1000).unwrap().len();
    let out = w.p.replace_source_text(w.source, "Some text here.").unwrap();
    assert_eq!((out.text_revision, out.codings_moved), (1, 0));
    assert_eq!(w.p.events(1000).unwrap().len(), events, "no event for a no-op");
    assert!(w.p.replace_source_text(w.source, "").is_err());
    assert!(w.p.replace_source_text(w.source, "a\0b").is_err());
    assert!(w.p.replace_source_text(999, "x").is_err());
    // Decomposed accents are stored composed.
    w.p.replace_source_text(w.source, "cafe\u{301}").unwrap();
    assert_eq!(w.p.source_text(w.source).unwrap(), "caf\u{e9}");
}

#[test]
fn a_coding_whose_text_is_deleted_is_removed_and_everything_about_it_is_logged() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir, "keep this and drop that part, then keep going");
    let drop = w.code_span(14, 23); // "drop that"
    let keep = w.code_span(0, 9); // "keep this"
    w.p.set_anchor(drop, true).unwrap();
    w.p.set_coding_rationale(drop, "an example worth remembering").unwrap();

    let out = w.p.replace_source_text(w.source, "keep this and part, then keep going").unwrap();
    assert_eq!((out.codings_removed, out.text_revision), (1, 2));
    let left = w.p.list_codings(w.source).unwrap();
    assert_eq!(left.iter().map(|c| c.id).collect::<Vec<_>>(), [keep]);
    assert!(w.p.list_anchors(w.code).unwrap().is_empty(), "an anchor never points at a removed coding");

    let event = w.p.events(1).unwrap().remove(0);
    assert_eq!(event.action, "source.edit");
    let removed = &event.before.unwrap()["removed_codings"][0];
    assert_eq!(removed["id"], drop);
    assert_eq!(removed["text"], "drop that");
    assert_eq!(removed["rationale"], "an example worth remembering");
    assert_eq!(removed["code_id"], w.code);
    assert_eq!(event.after.unwrap()["codings_removed"], 1);
}

#[test]
fn units_replies_runs_and_waiting_suggestions_follow_the_edit() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    //            0         1         2
    //            0123456789012345678901234567
    let text = "Post one\n\nReply one\n\nLast";
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Thread".into(),
            source_type: SourceType::Social,
            folder: String::new(),
            text: text.into(),
            runs: vec![FormatRun { start: 0, end: 8, kind: RunKind::Bold, level: None, page: None }, FormatRun { start: 21, end: 25, kind: RunKind::Italic, level: None, page: None }],
            units: vec![
                ImportedUnit::new(0, 8, UnitKind::Comment),
                ImportedUnit { parent: Some(0), attrs: vec![("author".into(), "bo".into())], ..ImportedUnit::new(10, 19, UnitKind::Comment) },
                ImportedUnit::new(21, 25, UnitKind::Comment),
            ],
            default_unit_kind: UnitKind::Comment,
            unit_attributes: vec![AttributeSpec::new("author", AppliesTo::Unit, Level::Nominal)],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let out = p.apply_import(&batch).unwrap();
    let source = out.source_ids[0];
    let c = Connection::open(p.path()).unwrap();
    c.execute(
        "INSERT INTO proposal (kind, origin, batch, payload_json, created_at) VALUES ('coding', 'autocode', 'b1', ?1, 'now')",
        [format!("{{\"source_id\": {source}, \"start\": 0, \"end\": 4}}")],
    )
    .unwrap();

    // Delete the whole reply and add a line at the top.
    let out = p.replace_source_text(source, "Intro\n\nPost one\n\nLast").unwrap();
    assert_eq!((out.units_removed, out.units_moved, out.proposals_rejected), (1, 2, 1));
    let units = p.list_units(source).unwrap();
    assert_eq!(units.iter().map(|u| (u.start, u.end)).collect::<Vec<_>>(), [(7, 15), (17, 21)]);
    assert!(units.iter().all(|u| u.parent_id.is_none()));
    let detail = p.source_detail(source).unwrap();
    let runs = detail.formatting_runs.as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!((runs[0]["start"].as_i64(), runs[0]["end"].as_i64()), (Some(7), Some(15)));
    let status: String = c.query_row("SELECT status FROM proposal", [], |r| r.get(0)).unwrap();
    assert_eq!(status, "rejected", "a suggestion into the old text is turned down, not guessed at");
    let attrs = p.unit_attribute_values(source).unwrap();
    assert!(attrs.is_empty(), "the removed reply's attribute value went with it");
}

// ---- randomized properties ----

fn word_text(words: usize) -> String {
    (0..words).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")
}

/// A reference for where a span should end up after an edit; `None` means it should be removed.
type Reference = Box<dyn Fn(i64, i64) -> Option<(i64, i64)>>;

/// Character positions where each word starts. Edits at these positions keep every word whole.
fn word_starts(text: &str) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len()).filter(|&i| chars[i] != ' ' && (i == 0 || chars[i - 1] == ' ')).collect()
}

proptest! {
    // 48 cases keep the normal test run quick; set PROPTEST_CASES to run many more.
    #![proptest_config(ProptestConfig { cases: std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(48), ..ProptestConfig::default() })]

    /// Any edit to any text: every coding left is a valid span, every coding that is gone is in the
    /// audit trail, and the stored text is exactly the new text.
    #[test]
    fn any_edit_leaves_valid_spans_and_loses_nothing_silently(
        base in "[a-e é你😀 ]{6,60}",
        spans in proptest::collection::vec((0usize..60, 1usize..20), 1..8),
        cut_a in 0usize..60,
        cut_len in 0usize..15,
        insert in "[a-e é你😀 ]{0,12}",
    ) {
        let base_len = base.chars().count();
        prop_assume!(base.trim().len() > 3);
        let dir = TempDir::new().unwrap();
        let mut w = world(&dir, &base);
        let stored_len = w.p.source_text(w.source).unwrap().chars().count();
        for (s, l) in &spans {
            let s = s % stored_len;
            let e = (s + l).min(stored_len);
            if e > s { w.code_span(s as i64, e as i64); }
        }
        let before = w.p.list_codings(w.source).unwrap().len();

        // Cut a range out and put new text in its place.
        let old: Vec<char> = w.p.source_text(w.source).unwrap().chars().collect();
        let a = cut_a % (base_len + 1);
        let b = (a + cut_len).min(old.len());
        let a = a.min(b);
        let mut new_text: String = old[..a].iter().collect();
        new_text.push_str(&insert);
        new_text.extend(old[b..].iter());
        prop_assume!(!new_text.trim().is_empty());

        let result = w.p.replace_source_text(w.source, &new_text);
        prop_assert!(result.is_ok(), "{:?}", result.err());
        let stored = w.p.source_text(w.source).unwrap();
        use unicode_normalization::UnicodeNormalization;
        prop_assert_eq!(&stored, &new_text.nfc().collect::<String>());
        let n = stored.chars().count() as i64;

        let after = w.p.list_codings(w.source).unwrap();
        for c in &after {
            prop_assert!(c.start >= 0 && c.start < c.end && c.end <= n, "span {}..{} outside 0..{}", c.start, c.end, n);
        }
        let removed = if stored != old.iter().collect::<String>() {
            let ev = w.p.events(1).unwrap().remove(0);
            if ev.action == "source.edit" { ev.before.unwrap()["removed_codings"].as_array().unwrap().len() } else { 0 }
        } else { 0 };
        prop_assert_eq!(after.len() + removed, before, "every coding is either still there or in the audit trail");
        prop_assert_eq!(w.p.list_sources().unwrap()[0].characters, n);

        // Doing the same edit again changes nothing.
        let again = w.p.replace_source_text(w.source, &new_text).unwrap();
        prop_assert_eq!(again.codings_moved + again.codings_removed, 0);
    }

    /// Insertions and deletions of whole words in text made of distinct words, checked against an
    /// independent reference of the rule: a span keeps the characters that survive.
    #[test]
    fn word_boundary_edits_match_the_reference_rule(
        words in 6usize..14,
        spans in proptest::collection::vec((0usize..60, 1usize..25), 1..8),
        which in 0usize..40,
        delete in proptest::bool::ANY,
        len_tokens in 1usize..4,
    ) {
        let base = word_text(words);
        let n = base.chars().count();
        let dir = TempDir::new().unwrap();
        let mut w = world(&dir, &base);
        let mut coded: Vec<(i64, i64, i64)> = Vec::new();
        for (s, l) in &spans {
            let s = s % n;
            let e = (s + l).min(n);
            if e > s {
                let id = w.code_span(s as i64, e as i64);
                coded.push((id, s as i64, e as i64));
            }
        }
        let starts = word_starts(&base);
        let chars: Vec<char> = base.chars().collect();

        // Build the edit and the reference mapping for it.
        let (new_text, map): (String, Reference) = if delete {
            let i = which % starts.len();
            let a = starts[i];
            let b = if i + len_tokens < starts.len() { starts[i + len_tokens] } else { chars.len() };
            let mut t: String = chars[..a].iter().collect();
            t.extend(chars[b..].iter());
            let (a, b) = (a as i64, b as i64);
            let shift = move |x: i64| if x < a { x } else { x - (b - a) };
            (t, Box::new(move |s, e| {
                // The surviving characters of [s, e) are those outside [a, b).
                let first = (s..e).find(|x| *x < a || *x >= b)?;
                let last = (s..e).rev().find(|x| *x < a || *x >= b)?;
                Some((shift(first), shift(last) + 1))
            }))
        } else {
            let p = starts[which % starts.len()];
            let ins = "ZZ ".repeat(len_tokens);
            let mut t: String = chars[..p].iter().collect();
            t.push_str(&ins);
            t.extend(chars[p..].iter());
            let (p, l) = (p as i64, ins.chars().count() as i64);
            (t, Box::new(move |s, e| Some((if s >= p { s + l } else { s }, if e > p { e + l } else { e }))))
        };
        prop_assume!(!new_text.trim().is_empty());

        w.p.replace_source_text(w.source, &new_text).unwrap();
        let now = w.p.list_codings(w.source).unwrap();
        for (id, s, e) in coded {
            let expected = map(s, e);
            let actual = now.iter().find(|c| c.id == id).map(|c| (c.start, c.end));
            prop_assert_eq!(actual, expected, "coding {} was {}..{} in {:?} -> {:?}", id, s, e, base, new_text);
        }
    }
}
