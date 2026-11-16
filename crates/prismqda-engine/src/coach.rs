//! Coach mode (decision 199): questions about a person's own coding, from rules that can be read and checked, so a student learning to code
//! sees what an instructor would ask before the instructor sees it. The checks are plain counts over the codings the person made:
//!
//! * `no_definition`: a code the person used that has no definition in the codebook;
//! * `used_once`: a code the person gave to one passage only (asked only once the person has made at least [`MIN_CODINGS_FOR_ONCE`] codings,
//!   because every code is "used once" on the first day);
//! * `overlap`: two codes the person gave to the same words again and again, so it is not clear what each adds;
//! * `inconsistent`: words the person coded with a code in one place and left uncoded where the same words stand elsewhere.
//!
//! The engine returns the numbers and the places; the window words each as a question, because a coach asks and does not answer. Nothing
//! here changes the project, and nothing here is a model: the optional help from a chatbot is a prompt the person copies (decision 91),
//! built by [`Project::coach_prompt`] from the same findings.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::adjudication::CodeRef;
use crate::project::Project;
use crate::reliability::CoderInfo;
use crate::{Error, Result};

/// The fewest codings a person must have made before "used once" is asked about.
pub const MIN_CODINGS_FOR_ONCE: usize = 10;
/// The fewest times two codes must have been given to the same words before they are asked about, and the share of the rarer code's
/// codings that must overlap the other's.
pub const OVERLAP_MIN_SHARED: usize = 3;
pub const OVERLAP_MIN_SHARE: f64 = 0.8;
/// The most findings listed for one rule (the report says how many there were).
const MAX_PER_RULE: usize = 20;
/// The most places shown with a finding.
const MAX_EXAMPLES: usize = 3;
/// More text than this in the documents checked is not scanned for repeated words.
const MAX_SCAN_CHARS: usize = 3_000_000;
/// How many coded phrases are looked for elsewhere.
const MAX_PHRASES: usize = 300;
const PHRASE_MIN_CHARS: usize = 3;
const PHRASE_MAX_CHARS: usize = 60;
const PHRASE_MAX_WORDS: usize = 8;
/// How much of the words around a place to show.
const CONTEXT_CHARS: usize = 50;

/// A place in a document: the document, and where the words start and end.
type Place = (i64, i64, i64);
/// Coded words as they are compared, with the code they were coded with; and the words as written, with how many times they were coded.
type PhraseKey = (Vec<char>, i64);
type PhraseCount = (String, usize);
/// Words coded with a code that stand uncoded elsewhere: the code, the words, how many places stand uncoded, how many were coded, and the places.
type Uncoded = (i64, String, usize, usize, Vec<Place>);

/// Whose coding to look at, and in which documents.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoachSpec {
    pub coder_id: i64,
    /// The documents to look at; empty for all of them.
    #[serde(default)]
    pub source_ids: Vec<i64>,
}

/// A place a finding points to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachExample {
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    /// The words of the place (cut when long), with a little around them for a place that was not coded.
    pub text: String,
}

/// One thing the coach noticed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachFinding {
    /// `no_definition`, `used_once`, `overlap`, or `inconsistent`.
    pub rule: String,
    /// The code (or the two codes, for `overlap`) the finding is about.
    pub codes: Vec<CodeRef>,
    /// What the rule counted: the codings of the code (`no_definition`), 1 (`used_once`), the codings of the two codes that overlap
    /// (`overlap`), or the places where the words stand uncoded (`inconsistent`).
    pub count: usize,
    /// The number it is set against: the codings of the rarer of the two codes (`overlap`), or the places where the words were coded
    /// (`inconsistent`); 0 for the other rules.
    pub of: usize,
    /// The words, for `inconsistent`.
    pub phrase: Option<String>,
    pub examples: Vec<CoachExample>,
}

/// How many findings a rule had.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachRuleCount {
    pub rule: String,
    pub findings: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachReport {
    pub coder: CoderInfo,
    pub documents: usize,
    /// The codings of this person in the documents looked at.
    pub codings: usize,
    pub codes_used: usize,
    /// Every rule, in order, with how many findings it had before the list was cut.
    pub rules: Vec<CoachRuleCount>,
    pub findings: Vec<CoachFinding>,
    pub notes: Vec<String>,
}

/// A prompt a person can copy into a chatbot of their choice: what the coach found, and the request to ask questions and not to answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachPrompt {
    pub text: String,
    pub findings: usize,
    /// What was left out of the prompt, one line each.
    pub left_out: Vec<String>,
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// The words as they are compared: whitespace runs are one space, letters are lower case, and the ends are trimmed.
fn normal(text: &str) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    for c in text.trim().chars() {
        if c.is_whitespace() {
            if out.last() != Some(&' ') {
                out.push(' ');
            }
        } else {
            out.push(lower(c));
        }
    }
    out
}

/// Where `phrase` (as [`normal`] makes it) stands in `doc` from `p`, as the place just after it; a space of the phrase matches any run of whitespace.
fn match_at(doc: &[char], p: usize, phrase: &[char]) -> Option<usize> {
    let (mut i, mut k) = (p, 0);
    while k < phrase.len() {
        if phrase[k] == ' ' {
            if i >= doc.len() || !doc[i].is_whitespace() {
                return None;
            }
            while i < doc.len() && doc[i].is_whitespace() {
                i += 1;
            }
        } else {
            if i >= doc.len() || lower(doc[i]) != phrase[k] {
                return None;
            }
            i += 1;
        }
        k += 1;
    }
    Some(i)
}

fn overlaps(spans: &[(i64, i64)], start: i64, end: i64) -> bool {
    spans.iter().any(|(s, e)| *s < end && *e > start)
}

fn cut_words(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// The kind of thing a rule looks for, as a phrase for a note.
fn rule_words(rule: &str) -> &'static str {
    match rule {
        "no_definition" => "codes without a definition",
        "used_once" => "codes used once",
        "overlap" => "codes given to the same words",
        _ => "words coded in one place and not in another",
    }
}

impl Project {
    /// Reads a document's text into the cache, once.
    fn coach_fetch(&self, texts: &mut HashMap<i64, Vec<char>>, source: i64) -> Result<()> {
        if let std::collections::hash_map::Entry::Vacant(slot) = texts.entry(source) {
            slot.insert(self.source_text(source)?.chars().collect());
        }
        Ok(())
    }

    /// What the coach notices in one person's coding of the documents chosen.
    pub fn coach_check(&self, spec: &CoachSpec) -> Result<CoachReport> {
        let coders = self.list_coders()?;
        let coder = coders.iter().find(|c| c.id == spec.coder_id).ok_or(Error::NotFound { what: "coder", id: spec.coder_id })?;
        let sources: Vec<_> = {
            let all = self.list_sources()?;
            for id in &spec.source_ids {
                if !all.iter().any(|s| s.id == *id) {
                    return Err(Error::NotFound { what: "document", id: *id });
                }
            }
            all.into_iter().filter(|s| spec.source_ids.is_empty() || spec.source_ids.contains(&s.id)).collect()
        };
        let source_name: HashMap<i64, String> = sources.iter().map(|s| (s.id, s.name.clone())).collect();
        let codes = self.list_codes()?;
        let code_by_id: HashMap<i64, &crate::CodeRow> = codes.iter().map(|c| (c.id, c)).collect();
        let code_ref = |id: i64| CodeRef { id, name: code_by_id.get(&id).map(|c| c.name.clone()).unwrap_or_else(|| id.to_string()) };

        // The person's codings of the text, by document (in the order of the text).
        let mut codings = Vec::new();
        for s in &sources {
            for c in self.list_codings(s.id)? {
                if c.coder_id == coder.id && c.layer == "coder" {
                    codings.push(c);
                }
            }
        }
        let mut per_code: HashMap<i64, usize> = HashMap::new();
        for c in &codings {
            *per_code.entry(c.code_id).or_default() += 1;
        }

        // The words of a place, kept in one cache so a document is read once.
        let mut texts: HashMap<i64, Vec<char>> = HashMap::new();
        let example = |texts: &HashMap<i64, Vec<char>>, source: i64, start: i64, end: i64, around: bool| -> CoachExample {
            let chars = texts.get(&source).map(|v| v.as_slice()).unwrap_or(&[]);
            let (s, e) = (start.max(0) as usize, (end.max(0) as usize).min(chars.len()));
            let (from, to) = if around { (s.saturating_sub(CONTEXT_CHARS), (e + CONTEXT_CHARS).min(chars.len())) } else { (s.min(chars.len()), e) };
            let text: String = chars[from.min(chars.len())..to.max(from.min(chars.len()))].iter().collect();
            CoachExample { source_name: source_name.get(&source).cloned().unwrap_or_default(), start, end, text: cut_words(&text, 160) }
        };

        let mut findings: Vec<CoachFinding> = Vec::new();
        let mut rules: Vec<CoachRuleCount> = Vec::new();
        let mut notes: Vec<String> = Vec::new();

        // 1. Codes used without a definition.
        let mut undefined: Vec<(i64, usize)> = per_code.iter().filter(|(id, _)| code_by_id.get(id).is_some_and(|c| c.definition.trim().is_empty())).map(|(id, n)| (*id, *n)).collect();
        undefined.sort_by(|a, b| b.1.cmp(&a.1).then(code_ref(a.0).name.to_lowercase().cmp(&code_ref(b.0).name.to_lowercase())));
        rules.push(CoachRuleCount { rule: "no_definition".into(), findings: undefined.len() });
        for (id, n) in undefined.iter().take(MAX_PER_RULE) {
            findings.push(CoachFinding { rule: "no_definition".into(), codes: vec![code_ref(*id)], count: *n, of: 0, phrase: None, examples: Vec::new() });
        }

        // 2. Codes used once.
        let mut once: Vec<i64> = Vec::new();
        if codings.len() >= MIN_CODINGS_FOR_ONCE {
            once = per_code.iter().filter(|(_, n)| **n == 1).map(|(id, _)| *id).collect();
            once.sort_by_key(|id| code_ref(*id).name.to_lowercase());
        } else if !codings.is_empty() {
            notes.push(format!("Codes used once are not asked about until there are {MIN_CODINGS_FOR_ONCE} codings; there are {} so far.", codings.len()));
        }
        rules.push(CoachRuleCount { rule: "used_once".into(), findings: once.len() });
        for id in once.iter().take(MAX_PER_RULE) {
            let place = codings.iter().find(|c| c.code_id == *id).cloned();
            let mut examples = Vec::new();
            if let Some(c) = place {
                self.coach_fetch(&mut texts, c.source_id)?;
                examples.push(example(&texts, c.source_id, c.start, c.end, false));
            }
            findings.push(CoachFinding { rule: "used_once".into(), codes: vec![code_ref(*id)], count: 1, of: 0, phrase: None, examples });
        }

        // 3. Two codes given to the same words again and again. A code and the code it sits under are left out: giving both is what a tree is for.
        let is_ancestor = |a: i64, mut of: i64| -> bool {
            let mut guard = 0;
            while let Some(parent) = code_by_id.get(&of).and_then(|c| c.parent_id) {
                if parent == a {
                    return true;
                }
                of = parent;
                guard += 1;
                if guard > 64 {
                    break;
                }
            }
            false
        };
        let mut pairs: HashMap<(i64, i64), Vec<Place>> = HashMap::new();
        for s in &sources {
            let here: Vec<&crate::CodingRow> = codings.iter().filter(|c| c.source_id == s.id).collect();
            for (i, a) in here.iter().enumerate() {
                for b in here.iter().skip(i + 1) {
                    if b.start >= a.end {
                        break;
                    }
                    if a.code_id == b.code_id {
                        continue;
                    }
                    let shared = a.end.min(b.end) - b.start.max(a.start);
                    let shorter = (a.end - a.start).min(b.end - b.start);
                    if shared * 2 < shorter {
                        continue;
                    }
                    let key = (a.code_id.min(b.code_id), a.code_id.max(b.code_id));
                    pairs.entry(key).or_default().push((s.id, a.start.max(b.start), a.end.min(b.end)));
                }
            }
        }
        let mut overlapping: Vec<((i64, i64), usize, usize)> = Vec::new();
        for ((a, b), places) in &pairs {
            let rarer = per_code[a].min(per_code[b]);
            if places.len() >= OVERLAP_MIN_SHARED && places.len() as f64 >= OVERLAP_MIN_SHARE * rarer as f64 && !is_ancestor(*a, *b) && !is_ancestor(*b, *a) {
                overlapping.push(((*a, *b), places.len(), rarer));
            }
        }
        overlapping.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
        rules.push(CoachRuleCount { rule: "overlap".into(), findings: overlapping.len() });
        for ((a, b), n, rarer) in overlapping.iter().take(MAX_PER_RULE) {
            let mut examples = Vec::new();
            for (source, start, end) in pairs[&(*a, *b)].iter().take(MAX_EXAMPLES) {
                self.coach_fetch(&mut texts, *source)?;
                examples.push(example(&texts, *source, *start, *end, false));
            }
            findings.push(CoachFinding { rule: "overlap".into(), codes: vec![code_ref(*a), code_ref(*b)], count: *n, of: *rarer, phrase: None, examples });
        }

        // 4. The same words coded in one place and left uncoded in another.
        let total_chars: i64 = sources.iter().map(|s| s.characters).sum();
        let mut left: Vec<Uncoded> = Vec::new();
        if total_chars as usize > MAX_SCAN_CHARS {
            notes.push("The documents are too long to look for repeated words in; that check was skipped.".to_string());
        } else {
            // The phrases: short coded stretches, with the code and the places they were coded.
            let mut phrases: HashMap<PhraseKey, PhraseCount> = HashMap::new();
            for c in &codings {
                if c.end - c.start > PHRASE_MAX_CHARS as i64 * 2 {
                    continue;
                }
                self.coach_fetch(&mut texts, c.source_id)?;
                let chars = &texts[&c.source_id];
                let (s, e) = (c.start as usize, (c.end as usize).min(chars.len()));
                if s >= e {
                    continue;
                }
                let raw: String = chars[s..e].iter().collect();
                let key = normal(&raw);
                let words = key.iter().filter(|ch| **ch == ' ').count() + 1;
                if key.len() < PHRASE_MIN_CHARS || key.len() > PHRASE_MAX_CHARS || words > PHRASE_MAX_WORDS || !key.iter().any(|ch| ch.is_alphanumeric()) {
                    continue;
                }
                let entry = phrases.entry((key, c.code_id)).or_insert_with(|| (cut_words(&raw, 80), 0));
                entry.1 += 1;
            }
            let mut ordered: Vec<(PhraseKey, PhraseCount)> = phrases.into_iter().collect();
            ordered.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(a.0 .0.cmp(&b.0 .0)).then(a.0 .1.cmp(&b.0 .1)));
            ordered.truncate(MAX_PHRASES);
            // What the person coded at all, by document, so a place already coded (with any code) is not called uncoded.
            let mut coded_anywhere: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
            for c in &codings {
                coded_anywhere.entry(c.source_id).or_default().push((c.start, c.end));
            }
            let mut first_char: HashMap<i64, HashMap<char, Vec<usize>>> = HashMap::new();
            for s in &sources {
                self.coach_fetch(&mut texts, s.id)?;
                let mut index: HashMap<char, Vec<usize>> = HashMap::new();
                for (i, ch) in texts[&s.id].iter().enumerate() {
                    index.entry(lower(*ch)).or_default().push(i);
                }
                first_char.insert(s.id, index);
            }
            for ((key, code), (shown, coded_places)) in &ordered {
                let mut uncoded: Vec<(i64, i64, i64)> = Vec::new();
                for s in &sources {
                    let doc = &texts[&s.id];
                    let Some(starts) = first_char[&s.id].get(&key[0]) else { continue };
                    for &p in starts {
                        let Some(e) = match_at(doc, p, key) else { continue };
                        if p > 0 && doc[p - 1].is_alphanumeric() || e < doc.len() && doc[e].is_alphanumeric() {
                            continue;
                        }
                        if !overlaps(coded_anywhere.get(&s.id).map(|v| v.as_slice()).unwrap_or(&[]), p as i64, e as i64) {
                            uncoded.push((s.id, p as i64, e as i64));
                        }
                    }
                }
                if !uncoded.is_empty() {
                    left.push((*code, shown.clone(), uncoded.len(), *coded_places, uncoded));
                }
            }
            left.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())).then(a.0.cmp(&b.0)));
        }
        rules.push(CoachRuleCount { rule: "inconsistent".into(), findings: left.len() });
        for (code, shown, n, coded_places, places) in left.iter().take(MAX_PER_RULE) {
            let examples = places.iter().take(MAX_EXAMPLES).map(|(source, start, end)| example(&texts, *source, *start, *end, true)).collect();
            findings.push(CoachFinding { rule: "inconsistent".into(), codes: vec![code_ref(*code)], count: *n, of: *coded_places, phrase: Some(shown.clone()), examples });
        }

        for r in &rules {
            if r.findings > MAX_PER_RULE {
                notes.push(format!("{} of the {} {} are listed.", MAX_PER_RULE, r.findings, rule_words(&r.rule)));
            }
        }
        let areas = self.all_regions()?.iter().filter(|r| r.layer == "coder" && r.coder_id == coder.id && sources.iter().any(|s| s.id == r.source_id)).count();
        if areas > 0 {
            notes.push(format!("{areas} coded area{} of pictures, PDF pages and recordings {} not looked at: the coach reads text only.", if areas == 1 { "" } else { "s" }, if areas == 1 { "was" } else { "were" }));
        }
        Ok(CoachReport {
            coder: CoderInfo { id: coder.id, name: coder.name.clone(), role: coder.role.clone(), model_name: coder.model_name.clone(), model_version: coder.model_version.clone() },
            documents: sources.len(),
            codings: codings.len(),
            codes_used: per_code.len(),
            rules,
            findings,
            notes,
        })
    }

    /// A prompt to copy into a chatbot, asking it to put questions to the person about what the coach noticed and not to answer them
    /// (decision 91: nothing is sent; the person decides what to paste and where). Codes and documents marked do-not-share are left out.
    pub fn coach_prompt(&self, spec: &CoachSpec) -> Result<CoachPrompt> {
        let mode: String = self.conn.query_row("SELECT ai_mode FROM project WHERE id = 1", [], |r| r.get(0))?;
        if mode != "packet" {
            return Err(Error::Invalid("AI prompt packets are switched off for this project".to_string()));
        }
        let report = self.coach_check(spec)?;
        let codes = self.list_codes()?;
        let shared: HashSet<i64> = codes.iter().filter(|c| !c.do_not_share && c.status == "active").map(|c| c.id).collect();
        let hidden_sources: HashSet<String> = self.list_sources()?.into_iter().filter(|s| s.do_not_share).map(|s| s.name).collect();
        let mut left_out = Vec::new();
        let dropped_codes = codes.iter().filter(|c| c.do_not_share).count();
        if dropped_codes > 0 {
            left_out.push(format!("{dropped_codes} code{} marked do-not-share", if dropped_codes == 1 { "" } else { "s" }));
        }
        if !hidden_sources.is_empty() {
            left_out.push(format!("the words of {} document{} marked do-not-share", hidden_sources.len(), if hidden_sources.len() == 1 { "" } else { "s" }));
        }
        left_out.push("every passage of a document except the short words the questions are about".to_string());

        let mut text = String::new();
        text.push_str("You are a coach for someone learning qualitative coding. They have coded some documents, and a simple checker noticed the things listed below in their own coding.\n");
        text.push_str("Help them see each point for themselves by asking questions. Do not tell them which code to use, do not write or rewrite a definition for them, do not code anything for them, and do not say whether a code is right or wrong. Ask one question at a time and wait for their answer.\n\n");
        text.push_str("Their codebook (code: definition):\n");
        for c in codes.iter().filter(|c| shared.contains(&c.id)) {
            let def = c.definition.trim();
            text.push_str(&format!("- {}: {}\n", c.name, if def.is_empty() { "(no definition yet)" } else { def }));
        }
        text.push_str("\nWhat the checker noticed:\n");
        let mut n = 0;
        for f in &report.findings {
            if f.codes.iter().any(|c| !shared.contains(&c.id)) {
                continue;
            }
            let names: Vec<String> = f.codes.iter().map(|c| format!("\"{}\"", c.name)).collect();
            let line = match f.rule.as_str() {
                "no_definition" => format!("{} has no definition and was used {} time(s).", names[0], f.count),
                "used_once" => format!("{} was used once.", names[0]),
                "overlap" => format!("{} and {} were given to the same words {} times (the rarer of the two was used {} times).", names[0], names[1], f.count, f.of),
                "inconsistent" => {
                    if f.examples.iter().any(|e| hidden_sources.contains(&e.source_name)) {
                        continue;
                    }
                    format!("The words \"{}\" were coded {} {} time(s) and stand uncoded in {} other place(s).", f.phrase.clone().unwrap_or_default(), names[0], f.of, f.count)
                }
                _ => continue,
            };
            n += 1;
            text.push_str(&format!("{n}. {line}\n"));
        }
        if n == 0 {
            text.push_str("(Nothing. Ask them what they are unsure about in their coding.)\n");
        }
        Ok(CoachPrompt { text, findings: n, left_out })
    }
}
