//! Pseudonymizing and redacting (specification 6.4, decisions 138 and 156).
//!
//! The researcher writes a list of things to find (names, places, a phone number) and what takes their place. The list is used in
//! two ways, and the researcher chooses:
//!
//! * **For what leaves the project.** While the switch is on, every Word, Excel, and CSV file, the report, and every packet taken to a
//!   chatbot has the list applied on its way out. The project itself is not changed, so nothing is lost and the list can be changed
//!   and applied again. A redaction here replaces the words themselves: no black box is drawn over words that are still in the file.
//! * **For good.** The project's own text is changed, in one step, after a backup. Codings, units, and formatting follow the
//!   words (a coding on "Sarah" is a coding on "Participant 3" afterwards), and the names are also taken out of the places that copy
//!   text: document names, speaker labels, memos, cases and their values, the codebook, saved searches, the AI log, and the audit
//!   trail. The list is the key to the names, so it is not kept in the project afterwards; the researcher may keep it as a file.
//!
//! Finding is exact. It does not guess at spelling, and it finds only what the list says. A "whole word" rule does not find "Ann" in
//! "Anne". A run of spaces in a term matches any run of spaces and line breaks, and a curly apostrophe matches a straight one. The
//! longest term wins where two overlap, so "Sarah Jones" is replaced as one before "Sarah" is looked at.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use unicode_normalization::char::canonical_combining_class;
use unicode_normalization::UnicodeNormalization;

use crate::edit::{runs_from_edits, replace_text_in, Run};
use crate::event;
use crate::project::Project;
use crate::{Error, Result};

/// The most things a list can hold.
pub const MAX_RULES: usize = 5_000;
const MAX_TERM_CHARS: usize = 200;
const MAX_REPLACEMENT_CHARS: usize = 200;

/// One thing to find and what replaces it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidRule {
    pub id: i64,
    pub term: String,
    pub replacement: String,
    /// `pseudonym` or `redact`.
    pub kind: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub note: String,
    pub created_at: String,
}

/// Rules to add: each term gets the same replacement.
#[derive(Debug, Clone, Deserialize)]
pub struct NewDeidRules {
    pub terms: Vec<String>,
    pub replacement: String,
    #[serde(default = "pseudonym")]
    pub kind: String,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default = "yes")]
    pub whole_word: bool,
    #[serde(default)]
    pub note: String,
}

fn pseudonym() -> String {
    "pseudonym".to_string()
}

fn yes() -> bool {
    true
}

/// What to change in one rule. Whatever is left out stays as it is.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeidRulePatch {
    #[serde(default)]
    pub term: Option<String>,
    #[serde(default)]
    pub replacement: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub match_case: Option<bool>,
    #[serde(default)]
    pub whole_word: Option<bool>,
    #[serde(default)]
    pub note: Option<String>,
}

/// What adding rules did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidAdded {
    pub added: Vec<DeidRule>,
    /// Terms that were already in the list (the same words with the same case rule), so were not added again.
    pub already_there: Vec<String>,
}

/// The list and the switch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidState {
    pub rules: Vec<DeidRule>,
    /// True while the list is applied to everything that leaves the project.
    pub exports: bool,
    /// When names were last replaced for good in this project, and how many, or `None` if never.
    pub last_applied: Option<DeidApplied>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidApplied {
    pub at: String,
    pub replacements: i64,
    pub documents: i64,
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Finding
// ---------------------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum P {
    Lit(char),
    Space,
}

#[derive(Debug, Clone)]
struct Compiled {
    id: i64,
    pattern: Vec<P>,
    match_case: bool,
    whole_word: bool,
    replacement: String,
    replacement_chars: usize,
    /// The first and last character of the term as typed, to know whether a word boundary matters at that edge.
    first_word: bool,
    last_word: bool,
}

fn is_unspaced_script(c: char) -> bool {
    matches!(c as u32, 0x0E00..=0x0E7F | 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FFFF)
}

/// A letter or digit of a script written with spaces between words (so a boundary is meaningful).
fn is_word(c: char) -> bool {
    (c.is_alphanumeric() || c == '_') && !is_unspaced_script(c)
}

fn fold(c: char, match_case: bool) -> char {
    let c = match c {
        '\u{2018}' | '\u{2019}' | '\u{201B}' | '\u{2032}' | '\u{02BC}' => '\'',
        '\u{201C}' | '\u{201D}' | '\u{201F}' | '\u{2033}' => '"',
        other => other,
    };
    if match_case {
        c
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

fn compile(rule: &DeidRule) -> Option<Compiled> {
    let term: String = rule.term.trim().nfc().collect();
    let mut pattern = Vec::new();
    for c in term.chars() {
        if c.is_whitespace() {
            if pattern.last() != Some(&P::Space) {
                pattern.push(P::Space);
            }
        } else {
            pattern.push(P::Lit(fold(c, rule.match_case)));
        }
    }
    if pattern.is_empty() {
        return None;
    }
    let first = term.chars().next()?;
    let last = term.chars().last()?;
    Some(Compiled {
        id: rule.id,
        pattern,
        match_case: rule.match_case,
        whole_word: rule.whole_word,
        replacement: rule.replacement.clone(),
        replacement_chars: rule.replacement.chars().count(),
        first_word: is_word(first),
        last_word: is_word(last),
    })
}

/// One thing found in a text, as character positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub start: usize,
    pub end: usize,
    /// The place of the rule in the matcher (not its id).
    pub rule: usize,
}

/// A stretch of the old text that became a stretch of the new text, as character positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Edit {
    pub old_start: usize,
    pub old_end: usize,
    pub new_start: usize,
    pub new_end: usize,
    pub rule: usize,
}

/// A text with the list applied, and where each replacement went.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub text: String,
    pub edits: Vec<Edit>,
}

/// The list, ready to find things in text. Also what is used on the way out of the project, so it is called the scrubber there.
#[derive(Debug, Clone, Default)]
pub struct Scrubber {
    rules: Vec<Compiled>,
    by_first: HashMap<char, Vec<usize>>,
}



impl Scrubber {
    pub fn new(rules: &[DeidRule]) -> Scrubber {
        let compiled: Vec<Compiled> = rules.iter().filter_map(compile).collect();
        let mut by_first: HashMap<char, Vec<usize>> = HashMap::new();
        for (i, c) in compiled.iter().enumerate() {
            if let P::Lit(ch) = c.pattern[0] {
                by_first.entry(ch).or_default().push(i);
            }
        }
        Scrubber { rules: compiled, by_first }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// The id of the rule at a place in this matcher.
    pub fn rule_id(&self, place: usize) -> i64 {
        self.rules[place].id
    }

    fn match_at(&self, rule: &Compiled, t: &[char], i: usize) -> Option<usize> {
        let mut j = i;
        for p in &rule.pattern {
            match p {
                P::Space => {
                    if j >= t.len() || !t[j].is_whitespace() {
                        return None;
                    }
                    while j < t.len() && t[j].is_whitespace() {
                        j += 1;
                    }
                }
                P::Lit(c) => {
                    if j >= t.len() || fold(t[j], rule.match_case) != *c {
                        return None;
                    }
                    j += 1;
                }
            }
        }
        if rule.whole_word {
            if rule.first_word && i > 0 && is_word(t[i - 1]) {
                return None;
            }
            if rule.last_word && j < t.len() && is_word(t[j]) {
                return None;
            }
        }
        Some(j)
    }

    /// Everything the list finds in a text: left to right, never overlapping, the longest term where several start at one place.
    pub fn scan_chars(&self, t: &[char]) -> Vec<Hit> {
        let mut hits = Vec::new();
        if self.rules.is_empty() {
            return hits;
        }
        let mut i = 0;
        while i < t.len() {
            let mut best: Option<(usize, usize)> = None;
            // A rule that ignores case is found by the lowercase letter; one that keeps case, by the letter itself. Both are in `by_first`
            // under the folded letter the rule was compiled with, so look under the letter folded each way.
            let lower = fold(t[i], false);
            let exact = fold(t[i], true);
            let mut tried: [Option<&Vec<usize>>; 2] = [self.by_first.get(&lower), None];
            if exact != lower {
                tried[1] = self.by_first.get(&exact);
            }
            for list in tried.into_iter().flatten() {
                for &ri in list {
                    if let Some(end) = self.match_at(&self.rules[ri], t, i) {
                        let better = match best {
                            None => true,
                            Some((be, br)) => end > be || (end == be && ri < br),
                        };
                        if better {
                            best = Some((end, ri));
                        }
                    }
                }
            }
            match best {
                Some((end, ri)) => {
                    hits.push(Hit { start: i, end, rule: ri });
                    i = end;
                }
                None => i += 1,
            }
        }
        hits
    }

    pub fn scan(&self, text: &str) -> Vec<Hit> {
        if self.rules.is_empty() {
            return Vec::new();
        }
        let chars: Vec<char> = text.chars().collect();
        self.scan_chars(&chars)
    }

    /// The text with the list applied, and where each replacement went.
    pub fn apply(&self, text: &str) -> Applied {
        let chars: Vec<char> = text.chars().collect();
        let hits = self.scan_chars(&chars);
        if hits.is_empty() {
            return Applied { text: text.to_string(), edits: Vec::new() };
        }
        let mut out = String::with_capacity(text.len());
        let mut edits = Vec::with_capacity(hits.len());
        let (mut last, mut new_len) = (0usize, 0usize);
        for h in hits {
            out.extend(chars[last..h.start].iter());
            new_len += h.start - last;
            let rule = &self.rules[h.rule];
            out.push_str(&rule.replacement);
            edits.push(Edit { old_start: h.start, old_end: h.end, new_start: new_len, new_end: new_len + rule.replacement_chars, rule: h.rule });
            new_len += rule.replacement_chars;
            last = h.end;
        }
        out.extend(chars[last..].iter());
        Applied { text: out, edits }
    }

    /// The text with the list applied; the text itself, not copied, when the list finds nothing in it.
    pub fn scrub<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.rules.is_empty() || text.is_empty() {
            return Cow::Borrowed(text);
        }
        let applied = self.apply(text);
        if applied.edits.is_empty() {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(applied.text)
        }
    }

    /// Applies the list to every text inside a JSON value (not to its keys), and says how many texts changed.
    pub fn scrub_value(&self, v: &mut Value) -> usize {
        match v {
            Value::String(s) => {
                let applied = self.apply(s);
                if applied.edits.is_empty() {
                    0
                } else {
                    *s = applied.text;
                    1
                }
            }
            Value::Array(a) => a.iter_mut().map(|x| self.scrub_value(x)).sum(),
            Value::Object(o) => o.values_mut().map(|x| self.scrub_value(x)).sum(),
            _ => 0,
        }
    }
}

/// Where a stretch of scrubbed text came from in the original, for a stretch `[a, b)` of the new text. A stretch that starts or ends
/// inside a replacement takes the whole of what was replaced. Positions are characters.
pub fn map_back(edits: &[Edit], a: usize, b: usize) -> (usize, usize) {
    let mut start = None;
    let mut end = None;
    let (mut delta_a, mut delta_b) = (0i64, 0i64);
    for e in edits {
        let shrink = (e.old_end - e.old_start) as i64 - (e.new_end - e.new_start) as i64;
        if start.is_none() && a >= e.new_start && a < e.new_end {
            start = Some(e.old_start);
        }
        if end.is_none() && b > e.new_start && b <= e.new_end {
            end = Some(e.old_end);
        }
        if e.new_end <= a {
            delta_a += shrink;
        }
        if e.new_end <= b {
            delta_b += shrink;
        }
    }
    let s = start.unwrap_or_else(|| (a as i64 + delta_a).max(0) as usize);
    let e = end.unwrap_or_else(|| (b as i64 + delta_b).max(0) as usize);
    (s, e.max(s))
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The list
// ---------------------------------------------------------------------------------------------------------------------------------

fn clean_term(term: &str) -> String {
    term.split_whitespace().collect::<Vec<_>>().join(" ").nfc().collect()
}

fn check_replacement(replacement: &str) -> Result<String> {
    let r: String = replacement.trim().nfc().collect();
    if r.is_empty() {
        return Err(Error::Invalid("say what takes the place of the words: it cannot be empty, because an empty replacement would delete words".to_string()));
    }
    if r.chars().count() > MAX_REPLACEMENT_CHARS {
        return Err(Error::Invalid(format!("what takes the place of the words can be at most {MAX_REPLACEMENT_CHARS} characters")));
    }
    if r.contains('\0') {
        return Err(Error::Invalid("the replacement contains a NUL character, which cannot be stored".to_string()));
    }
    // A comma, a quote, or a line break in a replacement would break a spreadsheet's columns where a name stood in an unquoted cell.
    if r.contains([',', '"', '\n', '\r', '\t']) {
        return Err(Error::Invalid("what takes the place of the words cannot contain a comma, a double quote, a tab, or a line break, because it could break the columns of a spreadsheet".to_string()));
    }
    // A mark at either end could join with the letter next to it and change how the text is composed, which would shift every position after it.
    let first = r.chars().next().expect("not empty");
    let last = r.chars().last().expect("not empty");
    if canonical_combining_class(first) != 0 || canonical_combining_class(last) != 0 {
        return Err(Error::Invalid("the replacement cannot begin or end with an accent or other combining mark".to_string()));
    }
    Ok(r)
}

fn check_term(term: &str) -> Result<String> {
    let t = clean_term(term);
    if t.is_empty() {
        return Err(Error::Invalid("a term to find cannot be empty".to_string()));
    }
    if t.chars().count() > MAX_TERM_CHARS {
        return Err(Error::Invalid(format!("a term to find can be at most {MAX_TERM_CHARS} characters; for a longer passage, select less of it")));
    }
    if t.contains('\0') {
        return Err(Error::Invalid("the term contains a NUL character, which cannot be stored".to_string()));
    }
    Ok(t)
}

fn check_kind(kind: &str) -> Result<()> {
    if matches!(kind, "pseudonym" | "redact") {
        Ok(())
    } else {
        Err(Error::Invalid(format!("'{kind}' is not a kind of rule: use pseudonym or redact")))
    }
}

fn rule_from_row(r: &rusqlite::Row) -> rusqlite::Result<DeidRule> {
    Ok(DeidRule {
        id: r.get(0)?,
        term: r.get(1)?,
        replacement: r.get(2)?,
        kind: r.get(3)?,
        match_case: r.get::<_, i64>(4)? != 0,
        whole_word: r.get::<_, i64>(5)? != 0,
        note: r.get(6)?,
        created_at: r.get(7)?,
    })
}

const RULE_COLUMNS: &str = "id, term, replacement, kind, match_case, whole_word, note, created_at";

fn rules_in(conn: &Connection) -> Result<Vec<DeidRule>> {
    let mut stmt = conn.prepare(&format!("SELECT {RULE_COLUMNS} FROM deid_rule ORDER BY id"))?;
    let rows = stmt.query_map([], rule_from_row)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// A key to compare terms by, so "Sarah" and "sarah" are one term when case does not matter.
fn term_key(term: &str, match_case: bool) -> String {
    term.chars().map(|c| fold(c, match_case)).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Project {
    /// The list, in the order it was written.
    pub fn deid_rules(&self) -> Result<Vec<DeidRule>> {
        rules_in(&self.conn)
    }

    /// The list, the switch, and when names were last replaced for good.
    pub fn deid_state(&self) -> Result<DeidState> {
        let exports: i64 = self.conn.query_row("SELECT deid_exports FROM project WHERE id = 1", [], |r| r.get(0))?;
        let last_applied = self
            .conn
            .query_row("SELECT ts, after_json FROM event WHERE action = 'deid.apply' ORDER BY id DESC LIMIT 1", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))
            .ok()
            .map(|(ts, after)| {
                let v: Value = after.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
                DeidApplied { at: ts, replacements: v["replacements"].as_i64().unwrap_or(0), documents: v["documents"].as_i64().unwrap_or(0) }
            });
        Ok(DeidState { rules: self.deid_rules()?, exports: exports != 0, last_applied })
    }

    /// Adds things to find, all with the same replacement. A term already in the list (same words, same case rule) is not added twice.
    /// The audit trail records that rules were added and how many, never the words: the trail stays in the project after the names
    /// are taken out of it.
    pub fn add_deid_rules(&mut self, new: &NewDeidRules) -> Result<DeidAdded> {
        self.ensure_writable()?;
        check_kind(&new.kind)?;
        let replacement = check_replacement(&new.replacement)?;
        if new.terms.is_empty() {
            return Err(Error::Invalid("give at least one word or phrase to find".to_string()));
        }
        let mut terms = Vec::new();
        for t in &new.terms {
            if t.trim().is_empty() {
                continue;
            }
            terms.push(check_term(t)?);
        }
        if terms.is_empty() {
            return Err(Error::Invalid("give at least one word or phrase to find".to_string()));
        }
        let note: String = new.note.trim().chars().take(300).collect();
        let tx = self.conn.transaction()?;
        let existing = rules_in(&tx)?;
        if existing.len() + terms.len() > MAX_RULES {
            return Err(Error::Invalid(format!("the list can hold at most {MAX_RULES} things to find")));
        }
        let mut seen: HashSet<(String, bool)> = existing.iter().map(|r| (term_key(&r.term, r.match_case), r.match_case)).collect();
        let (mut added, mut already) = (Vec::new(), Vec::new());
        for term in terms {
            if !seen.insert((term_key(&term, new.match_case), new.match_case)) {
                already.push(term);
                continue;
            }
            tx.execute(
                "INSERT INTO deid_rule (term, replacement, kind, match_case, whole_word, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![term, replacement, new.kind, new.match_case as i64, new.whole_word as i64, note],
            )?;
            let id = tx.last_insert_rowid();
            added.push(tx.query_row(&format!("SELECT {RULE_COLUMNS} FROM deid_rule WHERE id = ?1"), [id], rule_from_row)?);
        }
        if !added.is_empty() {
            event::insert(&tx, &self.actor, &self.session, "deid.rule", None, None, Some(&json!({ "change": "add", "count": added.len(), "kind": new.kind })))?;
        }
        tx.commit()?;
        Ok(DeidAdded { added, already_there: already })
    }

    /// Changes one thing in the list.
    pub fn update_deid_rule(&mut self, id: i64, patch: &DeidRulePatch) -> Result<DeidRule> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let mut rule = tx
            .query_row(&format!("SELECT {RULE_COLUMNS} FROM deid_rule WHERE id = ?1"), [id], rule_from_row)
            .map_err(|_| Error::NotFound { what: "thing to find", id })?;
        if let Some(t) = &patch.term {
            rule.term = check_term(t)?;
        }
        if let Some(r) = &patch.replacement {
            rule.replacement = check_replacement(r)?;
        }
        if let Some(k) = &patch.kind {
            check_kind(k)?;
            rule.kind = k.clone();
        }
        if let Some(m) = patch.match_case {
            rule.match_case = m;
        }
        if let Some(w) = patch.whole_word {
            rule.whole_word = w;
        }
        if let Some(n) = &patch.note {
            rule.note = n.trim().chars().take(300).collect();
        }
        let key = (term_key(&rule.term, rule.match_case), rule.match_case);
        for other in rules_in(&tx)?.into_iter().filter(|o| o.id != id) {
            if (term_key(&other.term, other.match_case), other.match_case) == key {
                return Err(Error::Invalid("that is already in the list".to_string()));
            }
        }
        tx.execute(
            "UPDATE deid_rule SET term = ?1, replacement = ?2, kind = ?3, match_case = ?4, whole_word = ?5, note = ?6 WHERE id = ?7",
            params![rule.term, rule.replacement, rule.kind, rule.match_case as i64, rule.whole_word as i64, rule.note, id],
        )?;
        event::insert(&tx, &self.actor, &self.session, "deid.rule", Some(("deid_rule", id)), None, Some(&json!({ "change": "update" })))?;
        tx.commit()?;
        Ok(rule)
    }

    /// Takes things out of the list.
    pub fn remove_deid_rules(&mut self, ids: &[i64]) -> Result<usize> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let mut n = 0;
        for id in ids {
            n += tx.execute("DELETE FROM deid_rule WHERE id = ?1", [id])?;
        }
        if n > 0 {
            event::insert(&tx, &self.actor, &self.session, "deid.rule", None, None, Some(&json!({ "change": "remove", "count": n })))?;
        }
        tx.commit()?;
        Ok(n)
    }

    /// Turns the list on or off for everything that leaves the project.
    pub fn set_deid_exports(&mut self, on: bool) -> Result<()> {
        self.ensure_writable()?;
        if on && self.deid_rules()?.is_empty() {
            return Err(Error::Invalid("the list is empty, so there is nothing to apply to what you export; add the names to find first".to_string()));
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE project SET deid_exports = ?1 WHERE id = 1", [on as i64])?;
        event::insert(&tx, &self.actor, &self.session, "deid.exports", None, None, Some(&json!({ "on": on })))?;
        tx.commit()?;
        Ok(())
    }

    /// The list as it applies to what leaves the project: `None` while the switch is off or the list is empty.
    pub fn export_scrubber(&self) -> Result<Option<Scrubber>> {
        let on: i64 = self.conn.query_row("SELECT deid_exports FROM project WHERE id = 1", [], |r| r.get(0))?;
        if on == 0 {
            return Ok(None);
        }
        let s = self.list_scrubber()?;
        Ok(if s.is_empty() { None } else { Some(s) })
    }

    /// The whole list, whether or not the switch is on.
    pub fn list_scrubber(&self) -> Result<Scrubber> {
        Ok(Scrubber::new(&self.deid_rules()?))
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Which documents can change, and where else the words are
// ---------------------------------------------------------------------------------------------------------------------------------

/// A document that is left as it is, and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidSkipped {
    pub id: i64,
    pub name: String,
    pub why: String,
}

/// A kind of place in the project that holds text, with how many times the list finds something there.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidPlace {
    pub place: String,
    pub count: usize,
    /// True when changing names for good changes this place; false when the researcher has to deal with it.
    pub covered: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidRuleCount {
    pub id: i64,
    pub hits: usize,
    pub documents: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidSourceCount {
    pub id: i64,
    pub name: String,
    pub hits: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidSample {
    pub rule_id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub before: String,
    pub after: String,
}

/// What the list would do, without doing it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidPreview {
    pub rules: Vec<DeidRuleCount>,
    pub sources: Vec<DeidSourceCount>,
    pub samples: Vec<DeidSample>,
    pub total: usize,
    pub skipped: Vec<DeidSkipped>,
    pub elsewhere: Vec<DeidPlace>,
}

/// Text columns that changing names for good rewrites, as (table, column).
const PLAIN_COLUMNS: &[(&str, &str)] = &[
    ("research_case", "name"),
    ("research_case", "notes"),
    ("attribute_value", "value"),
    ("code", "definition"),
    ("code", "inclusion_criteria"),
    ("code", "exclusion_criteria"),
    ("code_set", "description"),
    ("memo", "title"),
    ("memo", "body"),
    ("concept_map", "notes"),
    ("map_node", "label"),
    ("map_link", "label"),
    ("link", "label"),
    ("codebook_version", "note"),
    ("reliability_run", "report_text"),
    ("analysis_run", "apa_text"),
    ("ai_run", "prompt"),
    ("ai_run", "raw_output"),
    ("adjudication", "reason"),
    ("stat_page", "title"),
    ("stat_page", "note"),
    ("triangulation", "topic"),
    ("triangulation", "qualitative"),
    ("triangulation", "quantitative"),
    ("triangulation", "note"),
    ("integration_entry", "what"),
    ("integration_entry", "why"),
    ("mixed_design", "purpose"),
    ("project", "research_questions"),
    ("saved_query", "note"),
];

/// JSON columns whose texts (not their keys) are rewritten.
const JSON_COLUMNS: &[(&str, &str)] = &[
    ("source", "import_settings_json"),
    ("source", "import_report_json"),
    ("proposal", "payload_json"),
    ("saved_query", "draft_json"),
    ("saved_query", "spec_json"),
    ("saved_query", "frozen_json"),
    ("codebook_version", "snapshot_json"),
    ("reliability_run", "results_json"),
    ("analysis_run", "result_tables_json"),
    ("ai_run", "units_sent_json"),
    ("ai_run", "parsed_result_json"),
    ("stat_page", "request_json"),
    ("stat_page", "result_json"),
    ("stat_page_run", "result_json"),
    ("event", "before_json"),
    ("event", "after_json"),
];

/// Columns of documents that are rewritten for the documents that change.
const SCOPED_COLUMNS: &[(&str, &str)] = &[("source", "name"), ("source", "folder"), ("unit", "speaker"), ("coding", "rationale")];

fn covered(table: &str, column: &str) -> bool {
    PLAIN_COLUMNS.iter().chain(JSON_COLUMNS).chain(SCOPED_COLUMNS).any(|(t, c)| *t == table && *c == column)
}

/// What a place is called to the researcher.
fn place_label(table: &str, column: &str) -> String {
    match (table, column) {
        ("source", "text") => "The text of documents",
        ("source", "name") => "Document names",
        ("source", "folder") => "Document folders",
        ("source", "import_report_json") | ("source", "import_settings_json") | ("source", "formatting_runs_json") => "How documents were imported",
        ("unit", "speaker") => "Speaker labels",
        ("coding", "rationale") => "Notes on codings",
        ("research_case", _) => "Cases",
        ("attribute_value", _) => "Values of variables",
        ("attribute", _) => "Names of variables",
        ("code", "name") => "Code names",
        ("code", _) | ("code_set", _) | ("codebook_version", _) | ("code_anchor", _) => "The codebook",
        ("memo", _) => "Memos",
        ("event", _) => "The audit trail",
        ("ai_run", _) | ("proposal", _) => "AI suggestions and the AI log",
        ("saved_query", _) => "Saved queries",
        ("concept_map", _) | ("map_node", _) | ("map_link", _) | ("link", _) => "Concept maps and links",
        ("reliability_run", _) | ("adjudication", _) => "Agreement and adjudication",
        ("analysis_run", _) | ("stat_page", _) | ("stat_page_run", _) => "The statistics notebook",
        ("triangulation", _) | ("integration_entry", _) | ("mixed_design", _) => "Mixed methods",
        ("project", "name") => "The project's name",
        ("project", _) => "The project's research questions",
        ("media", _) => "File names and places of recordings",
        ("pdf_file", _) | ("image", _) => "File names of PDFs and pictures",
        ("coder", _) => "Coder names",
        ("phase_progress", _) | ("deid_rule", _) => "Settings",
        _ => return format!("{table} ({column})"),
    }
    .to_string()
}

/// Every text column in the project, as (table, column). Whatever the list is looked for in, this is where it can be.
fn text_columns(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT m.name, p.name FROM sqlite_master m, pragma_table_info(m.name) p
         WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%' AND m.name <> 'deid_rule' AND (upper(p.type) LIKE '%TEXT%' OR upper(p.type) LIKE '%CHAR%')
         ORDER BY m.name, p.cid",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Every place in the project where the list finds something, other than the text of the documents that are listed in `except_sources`.
/// Counts are of things found. Places the list finds nothing in are left out.
fn find_everywhere(conn: &Connection, m: &Scrubber, without_document_text: bool) -> Result<Vec<DeidPlace>> {
    let mut counts: BTreeMap<(String, bool), usize> = BTreeMap::new();
    for (table, column) in text_columns(conn)? {
        if without_document_text && table == "source" && column == "text" {
            continue;
        }
        let mut stmt = conn.prepare(&format!("SELECT \"{column}\" FROM \"{table}\" WHERE \"{column}\" IS NOT NULL"))?;
        let mut rows = stmt.query([])?;
        let mut n = 0usize;
        while let Some(row) = rows.next()? {
            // A column declared TEXT can hold a number in SQLite; only real text can hold a name.
            if let Ok(s) = row.get::<_, String>(0) {
                n += m.scan(&s).len();
            }
        }
        if n > 0 {
            *counts.entry((place_label(&table, &column), covered(&table, &column))).or_insert(0) += n;
        }
    }
    Ok(counts.into_iter().map(|((place, covered), count)| DeidPlace { place, count, covered }).collect())
}

pub(crate) struct SourceInfo {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) text: String,
    /// Why the document cannot change, or `None` if it can.
    pub(crate) cannot: Option<String>,
}

impl Project {
    /// The documents (all, or the ones asked for), with whether each can change.
    pub(crate) fn deid_sources(&self, wanted: Option<&[i64]>) -> Result<Vec<SourceInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.text, s.source_type, EXISTS (SELECT 1 FROM pdf_file p WHERE p.source_id = s.id), EXISTS (SELECT 1 FROM image i WHERE i.source_id = s.id)
             FROM source s ORDER BY s.id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(4)? != 0, r.get::<_, i64>(5)? != 0)))?;
        let mut out = Vec::new();
        let want: Option<HashSet<i64>> = wanted.map(|w| w.iter().copied().collect());
        for row in rows {
            let (id, name, text, is_pdf, is_image) = row?;
            if want.as_ref().is_some_and(|w| !w.contains(&id)) {
                continue;
            }
            let cannot = if is_pdf {
                Some("It is a PDF kept as it was, and its pages would still show the words, so it is left as it is. Take it out of the project if it must not show them.".to_string())
            } else if is_image {
                Some("It is a picture; the picture itself cannot be changed.".to_string())
            } else {
                self.reliability_lock(id)?.map(|run| format!("It has units in agreement run {run}, which has not been archived, so its text cannot change; archive the run first."))
            };
            out.push(SourceInfo { id, name, text, cannot });
        }
        Ok(out)
    }

    /// What the list would do to the documents, and where else the words are, without changing anything.
    pub fn deid_preview(&self, source_ids: Option<&[i64]>) -> Result<DeidPreview> {
        let rules = self.deid_rules()?;
        let m = Scrubber::new(&rules);
        let mut by_rule: HashMap<usize, (usize, HashSet<i64>)> = HashMap::new();
        let mut sources = Vec::new();
        let mut samples: Vec<DeidSample> = Vec::new();
        let mut skipped = Vec::new();
        let mut per_rule_samples: HashMap<usize, usize> = HashMap::new();
        let mut total = 0usize;
        for s in self.deid_sources(source_ids)? {
            if let Some(why) = &s.cannot {
                skipped.push(DeidSkipped { id: s.id, name: s.name.clone(), why: why.clone() });
                continue;
            }
            let chars: Vec<char> = s.text.chars().collect();
            let hits = m.scan_chars(&chars);
            if hits.is_empty() {
                continue;
            }
            total += hits.len();
            sources.push(DeidSourceCount { id: s.id, name: s.name.clone(), hits: hits.len() });
            for h in &hits {
                let e = by_rule.entry(h.rule).or_default();
                e.0 += 1;
                e.1.insert(s.id);
                let taken = per_rule_samples.entry(h.rule).or_insert(0);
                if *taken < 3 && samples.len() < 80 {
                    *taken += 1;
                    let lo = h.start.saturating_sub(40);
                    let hi = (h.end + 40).min(chars.len());
                    let before: String = chars[lo..hi].iter().collect();
                    let after = m.apply(&before).text;
                    let mark = |text: String, lead: bool, trail: bool| format!("{}{}{}", if lead { "\u{2026}" } else { "" }, text.replace(['\n', '\r'], " "), if trail { "\u{2026}" } else { "" });
                    samples.push(DeidSample {
                        rule_id: m.rule_id(h.rule),
                        source_id: s.id,
                        source_name: s.name.clone(),
                        before: mark(before, lo > 0, hi < chars.len()),
                        after: mark(after, lo > 0, hi < chars.len()),
                    });
                }
            }
        }
        sources.sort_by(|a, b| b.hits.cmp(&a.hits).then(a.id.cmp(&b.id)));
        let counts = rules
            .iter()
            .enumerate()
            .map(|(i, r)| {
                // `by_rule` is keyed by the place in the compiled list, which skips a rule with an empty pattern; rules here never are empty.
                let (hits, docs) = by_rule.get(&i).map(|(n, d)| (*n, d.len())).unwrap_or((0, 0));
                DeidRuleCount { id: r.id, hits, documents: docs }
            })
            .collect();
        let elsewhere = find_everywhere(&self.conn, &m, true)?;
        Ok(DeidPreview { rules: counts, sources, samples, total, skipped, elsewhere })
    }

    /// Every place in the project where the list still finds something.
    pub fn deid_leftovers(&self) -> Result<Vec<DeidPlace>> {
        let m = self.list_scrubber()?;
        find_everywhere(&self.conn, &m, false)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Changing the project for good
// ---------------------------------------------------------------------------------------------------------------------------------

/// What changing names for good did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidOutcome {
    /// The backup made first. It holds the names as they were; keep it somewhere private or delete it.
    pub backup: String,
    /// A sentence to show when the backup could not go where it was meant to.
    pub backup_note: Option<String>,
    /// How many earlier backups sit in the same folder. They were made before the names were replaced, so they hold the names too.
    pub earlier_backups: usize,
    pub documents_changed: usize,
    pub replacements: usize,
    pub codings_moved: usize,
    pub codings_removed: usize,
    pub units_moved: usize,
    pub proposals_rejected: usize,
    /// Where else names were replaced, with how many texts changed.
    pub changed: Vec<DeidPlace>,
    /// Documents left as they were, and why.
    pub skipped: Vec<DeidSkipped>,
    /// Places where the list still finds something, found before the list was removed.
    pub remaining: Vec<DeidPlace>,
    pub rules_removed: usize,
    /// True when the file was rewritten so that the old text is not left in unused parts of it.
    pub compacted: bool,
}

fn scrub_plain(tx: &Transaction, m: &Scrubber, table: &str, column: &str, only: Option<&HashSet<i64>>, source_column: Option<&str>) -> Result<usize> {
    let sql = match source_column {
        Some(sc) => format!("SELECT rowid, \"{column}\", \"{sc}\" FROM \"{table}\" WHERE \"{column}\" IS NOT NULL"),
        None => format!("SELECT rowid, \"{column}\", 0 FROM \"{table}\" WHERE \"{column}\" IS NOT NULL"),
    };
    let mut changes: Vec<(i64, String)> = Vec::new();
    {
        let mut stmt = tx.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let rowid: i64 = row.get(0)?;
            let Ok(text) = row.get::<_, String>(1) else { continue };
            if let (Some(only), Some(_)) = (only, source_column) {
                let sid: i64 = row.get(2)?;
                if !only.contains(&sid) {
                    continue;
                }
            }
            let applied = m.apply(&text);
            if !applied.edits.is_empty() {
                changes.push((rowid, applied.text));
            }
        }
    }
    let n = changes.len();
    if n > 0 {
        let mut update = tx.prepare(&format!("UPDATE \"{table}\" SET \"{column}\" = ?1 WHERE rowid = ?2"))?;
        for (rowid, text) in changes {
            update.execute(params![text, rowid])?;
        }
    }
    Ok(n)
}

fn scrub_json_column(tx: &Transaction, m: &Scrubber, table: &str, column: &str) -> Result<usize> {
    let mut changes: Vec<(i64, String)> = Vec::new();
    {
        let mut stmt = tx.prepare(&format!("SELECT rowid, \"{column}\" FROM \"{table}\" WHERE \"{column}\" IS NOT NULL"))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let rowid: i64 = row.get(0)?;
            let Ok(text) = row.get::<_, String>(1) else { continue };
            let Ok(mut value) = serde_json::from_str::<Value>(&text) else { continue };
            if m.scrub_value(&mut value) > 0 {
                changes.push((rowid, value.to_string()));
            }
        }
    }
    let n = changes.len();
    if n > 0 {
        let mut update = tx.prepare(&format!("UPDATE \"{table}\" SET \"{column}\" = ?1 WHERE rowid = ?2"))?;
        for (rowid, text) in changes {
            update.execute(params![text, rowid])?;
        }
    }
    Ok(n)
}

impl Project {
    /// Replaces the names in the project for good: the text of the documents (codings, units, and formatting follow the words), and
    /// every other place that copies text. A backup is made first, and nothing is changed if it cannot be. The list is removed from the
    /// project at the end (it holds the names), so write the key file before calling this if it is wanted.
    pub fn deid_apply(&mut self, source_ids: Option<&[i64]>) -> Result<DeidOutcome> {
        self.deid_apply_with(source_ids, true)
    }

    /// [`deid_apply`](Project::deid_apply), with no backup made when `make_backup` is false. A copy made to be shared (see
    /// [`deid_copy`](Project::deid_copy)) must not make one: it would sit beside the copy holding the names.
    fn deid_apply_with(&mut self, source_ids: Option<&[i64]>, make_backup: bool) -> Result<DeidOutcome> {
        self.ensure_writable()?;
        let rules = self.deid_rules()?;
        if rules.is_empty() {
            return Err(Error::Invalid("the list is empty, so there is nothing to replace; add the names to find first".to_string()));
        }
        let m = Scrubber::new(&rules);
        let sources = self.deid_sources(source_ids)?;
        if sources.is_empty() {
            return Err(Error::Invalid("there is no document to change".to_string()));
        }
        // Work out the new texts first, so a problem is found before anything is touched.
        struct Plan {
            id: i64,
            text: String,
            runs: Vec<Run>,
            replacements: usize,
        }
        let mut plans: Vec<Plan> = Vec::new();
        let mut skipped: Vec<DeidSkipped> = Vec::new();
        let mut in_scope: HashSet<i64> = HashSet::new();
        for s in &sources {
            if let Some(why) = &s.cannot {
                skipped.push(DeidSkipped { id: s.id, name: s.name.clone(), why: why.clone() });
                continue;
            }
            in_scope.insert(s.id);
            let applied = m.apply(&s.text);
            if applied.edits.is_empty() {
                continue;
            }
            if applied.text.nfc().collect::<String>() != applied.text {
                skipped.push(DeidSkipped { id: s.id, name: s.name.clone(), why: "A replacement would join with the letters next to it and change how they are composed, which would shift the codings.".to_string() });
                in_scope.remove(&s.id);
                continue;
            }
            let total = s.text.chars().count();
            plans.push(Plan { id: s.id, runs: runs_from_edits(&applied.edits, total), replacements: applied.edits.len(), text: applied.text });
        }

        // The backup: nothing changes if it cannot be made.
        let (backup, backup_note) = if make_backup { self.backup_now_noted()? } else { (std::path::PathBuf::new(), None) };
        let earlier_backups = if make_backup {
            backup
                .parent()
                .and_then(|dir| std::fs::read_dir(dir).ok())
                .map(|entries| entries.filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case(crate::project::PROJECT_EXTENSION))).count().saturating_sub(1))
                .unwrap_or(0)
        } else {
            0
        };

        self.conn.pragma_update(None, "secure_delete", true)?;
        let result = self.deid_apply_in_transaction(&m, &plans.iter().map(|p| (p.id, p.text.as_str(), p.runs.clone(), p.replacements)).collect::<Vec<_>>(), &in_scope, &rules);
        let _ = self.conn.pragma_update(None, "secure_delete", false);
        let mut outcome = result?;
        outcome.backup = if make_backup { backup.display().to_string() } else { String::new() };
        outcome.backup_note = backup_note;
        outcome.earlier_backups = earlier_backups;
        outcome.skipped = skipped;

        // Rewrite the file so the old text is not left in parts of it that are no longer used.
        let compacted = self.conn.execute_batch("VACUUM").is_ok();
        let _ = self.conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        outcome.compacted = compacted;
        Ok(outcome)
    }

    fn deid_apply_in_transaction(&mut self, m: &Scrubber, plans: &[(i64, &str, Vec<Run>, usize)], in_scope: &HashSet<i64>, rules: &[DeidRule]) -> Result<DeidOutcome> {
        let tx = self.conn.transaction()?;
        let (mut documents_changed, mut replacements) = (0usize, 0usize);
        let (mut codings_moved, mut codings_removed, mut units_moved, mut proposals_rejected) = (0usize, 0usize, 0usize, 0usize);
        for (id, text, runs, n) in plans {
            let out = replace_text_in(&tx, &self.actor, &self.session, *id, text, None, Some(runs.clone()), false)?;
            documents_changed += 1;
            replacements += n;
            codings_moved += out.codings_moved;
            codings_removed += out.codings_removed;
            units_moved += out.units_moved;
            proposals_rejected += out.proposals_rejected;
        }

        let mut changed: BTreeMap<(String, bool), usize> = BTreeMap::new();
        let mut note = |table: &str, column: &str, n: usize| {
            if n > 0 {
                *changed.entry((place_label(table, column), true)).or_insert(0) += n;
            }
        };
        for (table, column) in SCOPED_COLUMNS {
            let source_column = match *table {
                "source" => None,
                "unit" | "coding" => Some("source_id"),
                _ => None,
            };
            let n = if *table == "source" { scrub_plain_sources(&tx, m, column, in_scope)? } else { scrub_plain(&tx, m, table, column, Some(in_scope), source_column)? };
            note(table, column, n);
        }
        for (table, column) in PLAIN_COLUMNS {
            note(table, column, scrub_plain(&tx, m, table, column, None, None)?);
        }
        for (table, column) in JSON_COLUMNS {
            if *table == "event" {
                continue;
            }
            note(table, column, scrub_json_column(&tx, m, table, column)?);
        }

        // The audit trail refuses to be changed; the change to it is made here, once, on purpose, and the trail says so at its end.
        let triggers: Vec<(String, String)> = {
            let mut stmt = tx.prepare("SELECT name, sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'event' AND sql IS NOT NULL")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for (name, _) in &triggers {
            tx.execute_batch(&format!("DROP TRIGGER \"{name}\""))?;
        }
        for column in ["before_json", "after_json"] {
            note("event", column, scrub_json_column(&tx, m, "event", column)?);
        }
        for (_, sql) in &triggers {
            tx.execute_batch(sql)?;
        }

        // What the list still finds anywhere, before the list goes.
        let remaining = find_everywhere(&tx, m, false)?;

        let rules_removed = tx.execute("DELETE FROM deid_rule", [])?;
        tx.execute("UPDATE project SET deid_exports = 0 WHERE id = 1", [])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "deid.apply",
            None,
            None,
            Some(&json!({
                "documents": documents_changed,
                "replacements": replacements,
                "things_to_find": rules.len(),
                "codings_moved": codings_moved,
                "places_changed": changed.iter().map(|((p, _), n)| json!({ "place": p, "texts": n })).collect::<Vec<_>>(),
                "note": "Names were replaced for good, including in earlier entries of this audit trail.",
            })),
        )?;
        tx.commit()?;
        Ok(DeidOutcome {
            backup: String::new(),
            backup_note: None,
            earlier_backups: 0,
            documents_changed,
            replacements,
            codings_moved,
            codings_removed,
            units_moved,
            proposals_rejected,
            changed: changed.into_iter().map(|((place, covered), count)| DeidPlace { place, count, covered }).collect(),
            skipped: Vec::new(),
            remaining,
            rules_removed,
            compacted: false,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// A copy made to be shared, with the names replaced (decision 180)
// ---------------------------------------------------------------------------------------------------------------------------------

/// A picture or a PDF the copy no longer holds, because its words are in a picture the list cannot change.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidTakenOut {
    pub id: i64,
    pub name: String,
    /// `picture` or `pdf`.
    pub kind: String,
    /// The coded areas of it that were taken out with it.
    pub areas_removed: usize,
}

/// What making a de-identified copy did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidCopyOutcome {
    /// Where the copy is.
    pub path: String,
    /// What was replaced in the copy (the same account as for the project itself; there is no backup, so those parts are empty).
    pub outcome: DeidOutcome,
    /// The pictures and PDFs the copy does not hold, with the coded areas that went with them. Their text stays, with the names replaced.
    pub taken_out: Vec<DeidTakenOut>,
    /// How many links to recordings were taken out of the copy (a link holds the place of the file on this computer, and often a name).
    pub recordings_unlinked: usize,
    /// How many agreement runs in progress were archived in the copy, so the documents in them could be changed.
    pub runs_archived: usize,
}

/// Removes the files SQLite keeps beside a project (not the project).
fn remove_side_files(path: &std::path::Path) {
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let _ = std::fs::remove_file(std::path::PathBuf::from(name));
    }
}

impl Project {
    /// Makes a copy of the project at `dest` with the names in the list replaced for good **in the copy only**: the project and its list are
    /// not touched, and nothing is left beside the copy (no backup). In the copy, a picture or a PDF (whose pixels the list cannot change) is
    /// taken out with the areas coded on it and its text kept, links to recordings are taken out, and agreement runs in progress are archived;
    /// then the list is applied to every place that holds text, as for the project itself, and the list is removed from the copy. If anything
    /// fails, the copy is removed. The copy is encrypted with this project's passphrase when this project is.
    pub fn deid_copy(&mut self, dest: &std::path::Path) -> Result<DeidCopyOutcome> {
        self.ensure_writable()?;
        if self.deid_rules()?.is_empty() {
            return Err(Error::Invalid("the list is empty, so there is nothing to replace; add the names to find first".to_string()));
        }
        if dest.exists() {
            return Err(Error::AlreadyExists(dest.to_path_buf()));
        }
        self.copy_database(dest)?;
        let made = match self.deid_copy_finish(dest) {
            Ok(made) => made,
            Err(e) => {
                crate::project::remove_project_files(dest);
                return Err(e);
            }
        };
        event::insert(
            &self.conn,
            &self.actor,
            &self.session,
            "deid.copy",
            None,
            None,
            Some(&json!({
                "copy": dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                "documents": made.outcome.documents_changed,
                "replacements": made.outcome.replacements,
                "pictures_and_pdfs_taken_out": made.taken_out.len(),
                "note": "A copy was made with the names replaced. This project was not changed.",
            })),
        )?;
        Ok(made)
    }

    fn deid_copy_finish(&self, dest: &std::path::Path) -> Result<DeidCopyOutcome> {
        let mut opts = crate::project::OpenOptions::new(self.actor.clone());
        opts.backup_on_open = false;
        opts.keep_backups = self.keep_backups;
        opts.passphrase = self.key.as_ref().map(|k| k.as_str().to_string());
        let mut copy = Project::open(dest, &opts)?;

        // Runs in progress lock their documents; in a copy to be shared they are archived.
        let mut runs_archived = 0usize;
        for run in copy.list_reliability_runs()? {
            if !run.archived {
                copy.archive_reliability_run(run.id)?;
                runs_archived += 1;
            }
        }

        // Pictures and PDFs: the file goes, with the coded areas of it; the document stays as text.
        copy.conn.pragma_update(None, "secure_delete", true)?;
        let mut taken_out: Vec<DeidTakenOut> = Vec::new();
        let recordings_unlinked;
        {
            let tx = copy.conn.transaction()?;
            let docs: Vec<(i64, String, bool)> = {
                let mut stmt = tx.prepare(
                    "SELECT s.id, s.name, EXISTS (SELECT 1 FROM image i WHERE i.source_id = s.id) FROM source s
                     WHERE EXISTS (SELECT 1 FROM image i WHERE i.source_id = s.id) OR EXISTS (SELECT 1 FROM pdf_file p WHERE p.source_id = s.id) ORDER BY s.id",
                )?;
                let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? != 0)))?;
                rows.collect::<std::result::Result<_, _>>()?
            };
            for (id, name, is_picture) in docs {
                // What goes with an area when one is removed on its own goes with it here: a decision about it, and its links (an area is its
                // number made negative among passages, and a number can be used again, so a link left behind could join a later area).
                let area_ids: Vec<i64> = {
                    let mut stmt = tx.prepare("SELECT id FROM region WHERE source_id = ?1")?;
                    let rows = stmt.query_map([id], |r| r.get::<_, i64>(0))?;
                    rows.collect::<std::result::Result<_, _>>()?
                };
                for area in &area_ids {
                    tx.execute("DELETE FROM region_decision WHERE region_id = ?1", [area])?;
                    crate::passage_links::take_links(&tx, -area)?;
                }
                let areas_removed = tx.execute("DELETE FROM region WHERE source_id = ?1", [id])?;
                tx.execute("DELETE FROM image WHERE source_id = ?1", [id])?;
                tx.execute("DELETE FROM pdf_page WHERE source_id = ?1", [id])?;
                tx.execute("DELETE FROM pdf_file WHERE source_id = ?1", [id])?;
                tx.execute("UPDATE source SET source_type = 'document' WHERE id = ?1 AND source_type = 'pdf'", [id])?;
                taken_out.push(DeidTakenOut { id, name, kind: if is_picture { "picture" } else { "pdf" }.to_string(), areas_removed });
            }
            recordings_unlinked = tx.execute("DELETE FROM media", [])?;
            tx.commit()?;
        }

        let outcome = copy.deid_apply_with(None, false)?;

        // The copy says what it is, and the file is left with nothing beside it.
        copy.conn.execute("UPDATE project SET name = name || ' (names replaced)' WHERE id = 1", [])?;
        event::insert(
            &copy.conn,
            &copy.actor,
            &copy.session,
            "deid.copy_made",
            None,
            None,
            Some(&json!({ "note": "This is a copy made with the names replaced for good. Pictures and PDFs, and the links to recordings, were taken out of it." })),
        )?;
        // Last of all, so that the entries written above are covered too: nobody's own name stays on a change in a copy to be shared.
        neutralize_actors(&mut copy.conn)?;
        let _ = copy.conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        drop(copy);
        remove_side_files(dest);
        Ok(DeidCopyOutcome { path: dest.display().to_string(), outcome, taken_out, recordings_unlinked, runs_archived })
    }
}

/// How a coder made from a person's name is told from the others: the same rule as `actor_local_id` in the API crate.
fn local_id_for_person(name: &str) -> String {
    format!("user-{}", name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect::<String>())
}

/// In a copy that is to be shared, the name recorded on every change (the person who made it: on a project made before the name could be chosen
/// it is their Windows account name), on every entry of the mixed-methods log, and on the coder made from it, is replaced by a neutral label,
/// "Researcher 1", "Researcher 2" and so on, one for each name in the order the names first appear, so who did what stays apart. The audit trail
/// refuses to be changed, as in [`Project::deid_apply`]: its triggers come off for this one change, on this copy, and go back on.
fn neutralize_actors(conn: &mut rusqlite::Connection) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut names: Vec<String> = Vec::new();
    {
        let mut stmt = tx.prepare("SELECT actor FROM event GROUP BY actor ORDER BY MIN(id)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            names.push(row.get(0)?);
        }
        let mut stmt = tx.prepare("SELECT DISTINCT actor FROM integration_entry ORDER BY id")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    let triggers: Vec<String> = {
        let mut stmt = tx.prepare("SELECT name, sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'event' AND sql IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let all: Vec<(String, String)> = rows.collect::<std::result::Result<_, _>>()?;
        for (name, _) in &all {
            tx.execute_batch(&format!("DROP TRIGGER \"{name}\""))?;
        }
        all.into_iter().map(|(_, sql)| sql).collect()
    };
    // In two steps, through a place-holder, so a name that already looks like one of the labels ("Researcher 1") is not taken for a label.
    for (i, name) in names.iter().enumerate() {
        let hold = format!("\u{1}neutral-{i}");
        tx.execute("UPDATE event SET actor = ?1 WHERE actor = ?2", params![hold, name])?;
        tx.execute("UPDATE integration_entry SET actor = ?1 WHERE actor = ?2", params![hold, name])?;
        tx.execute("UPDATE coder SET name = ?1, local_id = ?1 WHERE local_id = ?2", params![hold, local_id_for_person(name)])?;
    }
    for i in 0..names.len() {
        let hold = format!("\u{1}neutral-{i}");
        let label = format!("Researcher {}", i + 1);
        tx.execute("UPDATE event SET actor = ?1 WHERE actor = ?2", params![label, hold])?;
        tx.execute("UPDATE integration_entry SET actor = ?1 WHERE actor = ?2", params![label, hold])?;
        tx.execute("UPDATE coder SET name = ?1, local_id = ?2 WHERE local_id = ?3", params![label, local_id_for_person(&label), hold])?;
    }
    for sql in &triggers {
        tx.execute_batch(sql)?;
    }
    tx.commit()?;
    Ok(names.len())
}

/// Rewrites the name or folder of the documents that are changing.
fn scrub_plain_sources(tx: &Transaction, m: &Scrubber, column: &str, in_scope: &HashSet<i64>) -> Result<usize> {
    let mut changes: Vec<(i64, String)> = Vec::new();
    {
        let mut stmt = tx.prepare(&format!("SELECT id, \"{column}\" FROM source"))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            if !in_scope.contains(&id) {
                continue;
            }
            let text: String = row.get(1)?;
            let applied = m.apply(&text);
            if !applied.edits.is_empty() && !applied.text.trim().is_empty() {
                changes.push((id, applied.text));
            }
        }
    }
    let n = changes.len();
    for (id, text) in changes {
        tx.execute(&format!("UPDATE source SET \"{column}\" = ?1 WHERE id = ?2"), params![text, id])?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, term: &str, replacement: &str, match_case: bool, whole_word: bool) -> DeidRule {
        DeidRule { id, term: term.to_string(), replacement: replacement.to_string(), kind: "pseudonym".to_string(), match_case, whole_word, note: String::new(), created_at: String::new() }
    }

    fn scrub(rules: &[DeidRule], text: &str) -> String {
        Scrubber::new(rules).scrub(text).into_owned()
    }

    #[test]
    fn a_whole_word_rule_does_not_find_part_of_another_word() {
        let r = [rule(1, "Ann", "P1", false, true)];
        assert_eq!(scrub(&r, "Ann met Anne and Hannah; ann left."), "P1 met Anne and Hannah; P1 left.");
        let part = [rule(1, "Ann", "P1", false, false)];
        assert_eq!(scrub(&part, "Anne"), "P1e");
    }

    #[test]
    fn case_matters_only_when_the_rule_says_so() {
        let r = [rule(1, "Will", "P1", true, true)];
        assert_eq!(scrub(&r, "I will ask Will."), "I will ask P1.");
        let any = [rule(1, "Will", "P1", false, true)];
        assert_eq!(scrub(&any, "I will ask Will."), "I P1 ask P1.");
    }

    #[test]
    fn the_longest_term_wins_where_two_start_together() {
        let r = [rule(1, "Sarah", "P1", false, true), rule(2, "Sarah Jones", "P2", false, true)];
        assert_eq!(scrub(&r, "Sarah Jones and Sarah"), "P2 and P1");
    }

    #[test]
    fn a_term_with_spaces_finds_a_line_break_and_a_curly_apostrophe_finds_a_straight_one() {
        let r = [rule(1, "Sarah Jones", "P2", false, true), rule(2, "O'Brien", "P3", false, true)];
        assert_eq!(scrub(&r, "Sarah\n  Jones met O\u{2019}Brien"), "P2 met P3");
    }

    #[test]
    fn possessives_and_hyphens_leave_the_rest_alone() {
        let r = [rule(1, "Sarah", "P1", false, true)];
        assert_eq!(scrub(&r, "Sarah's dog and Sarah-Jane"), "P1's dog and P1-Jane");
    }

    #[test]
    fn terms_that_are_not_words_need_no_boundary() {
        let r = [rule(1, "555-1234", "[phone]", false, true), rule(2, "(212)", "[area]", false, true)];
        assert_eq!(scrub(&r, "call 555-1234 or (212) now, not x555-1234"), "call [phone] or [area] now, not x555-1234");
    }

    #[test]
    fn languages_written_without_spaces_are_found_inside_a_sentence() {
        let r = [rule(1, "\u{5f20}\u{4e09}", "P1", false, true)];
        assert_eq!(scrub(&r, "\u{6211}\u{8ba4}\u{8bc6}\u{5f20}\u{4e09}\u{3002}"), "\u{6211}\u{8ba4}\u{8bc6}P1\u{3002}");
    }

    #[test]
    fn edits_say_where_each_replacement_went() {
        let r = [rule(1, "Sarah", "Participant 3", false, true)];
        let a = Scrubber::new(&r).apply("Hi Sarah, hi Sarah.");
        assert_eq!(a.text, "Hi Participant 3, hi Participant 3.");
        assert_eq!(a.edits.len(), 2);
        assert_eq!((a.edits[0].old_start, a.edits[0].old_end, a.edits[0].new_start, a.edits[0].new_end), (3, 8, 3, 16));
        assert_eq!((a.edits[1].old_start, a.edits[1].old_end, a.edits[1].new_start, a.edits[1].new_end), (13, 18, 21, 34));
    }

    #[test]
    fn a_stretch_of_the_new_text_maps_back_to_the_old() {
        let r = [rule(1, "Sarah", "Participant 3", false, true)];
        let a = Scrubber::new(&r).apply("Hi Sarah, hi Sarah.");
        // "hi" (the second one) is 18..20 in the new text and 16..18 in the old.
        let at = a.text.find("hi Participant").unwrap();
        assert_eq!(map_back(&a.edits, at, at + 2), (at - 8, at - 8 + 2));
        // A quote that ends inside a pseudonym takes the whole name.
        assert_eq!(map_back(&a.edits, 0, 6), (0, 8));
        // And one that starts inside it does too.
        assert_eq!(map_back(&a.edits, 6, 17), (3, 9));
        assert_eq!(map_back(&[], 4, 9), (4, 9));
    }

    #[test]
    fn json_texts_change_and_keys_do_not() {
        let r = [rule(1, "Sarah", "P1", false, true)];
        let mut v = json!({ "Sarah": "Sarah said", "list": ["no", "Sarah"], "n": 3 });
        assert_eq!(Scrubber::new(&r).scrub_value(&mut v), 2);
        assert_eq!(v, json!({ "Sarah": "P1 said", "list": ["no", "P1"], "n": 3 }));
    }

    #[test]
    fn an_empty_list_changes_nothing() {
        let s = Scrubber::new(&[]);
        assert!(matches!(s.scrub("Sarah"), Cow::Borrowed("Sarah")));
        assert!(s.scan("Sarah").is_empty());
    }

    #[test]
    fn a_replacement_cannot_be_empty_or_start_with_a_mark() {
        assert!(check_replacement("  ").is_err());
        assert!(check_replacement("\u{0301}a").is_err());
        assert_eq!(check_replacement(" Participant 3 ").unwrap(), "Participant 3");
    }
}
