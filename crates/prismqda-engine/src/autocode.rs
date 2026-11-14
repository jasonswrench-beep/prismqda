//! Rules that suggest codes without any model (specification 12.3): who spoke, which section a passage sits in, and how far
//! to widen a found phrase. They only read the project and work out spans; the suggestions go to the review queue
//! (`proposal.rs`), where a person accepts or rejects each one.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::import::FormatRun;
use crate::model::RunKind;
use crate::project::Project;
use crate::proposal::NewProposal;
use crate::search::{CharCounter, Query, SearchFilter};
use crate::Result;

/// One person who speaks in the chosen documents, with every spelling of their label gathered under one key.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeakerGroup {
    /// The label with case, punctuation, and extra spaces taken out ("Participant A:" and "participant a" are one speaker).
    pub key: String,
    /// The spelling used most often.
    pub label: String,
    /// Every spelling found, most used first, so the person can see what was gathered.
    pub variants: Vec<String>,
    /// Turns that can be coded.
    pub turns: usize,
    /// Turns that are left out of coding (an interviewer's, say).
    pub left_out: usize,
    /// In how many documents the speaker appears.
    pub sources: usize,
}

/// One turn of one speaker.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeakerTurn {
    pub source_id: i64,
    pub source_name: String,
    pub unit_id: i64,
    pub start: i64,
    pub end: i64,
    pub speaker: String,
    pub key: String,
}

/// How many headings of one level the chosen documents have.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeadingLevel {
    pub level: u8,
    pub count: usize,
    pub sources: usize,
}

/// The stretch of a document under one heading, up to the next heading of the same or a higher level.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    /// The heading's own words.
    pub heading: String,
    pub level: u8,
}

/// A phrase found by a search, with the unit or paragraph it was found in.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundSpan {
    pub source_id: i64,
    pub scope_start: i64,
    pub scope_end: i64,
    pub start: i64,
    pub end: i64,
    pub unit_id: Option<i64>,
}

/// What is left after suggestions that repeat earlier work are taken out.
#[derive(Debug, Clone, PartialEq)]
pub struct KnownDropped {
    pub kept: Vec<NewProposal>,
    /// Left out because the same coder already gave this code to all of those words.
    pub already_coded: usize,
    /// Left out because the same suggestion already waits in the queue.
    pub already_waiting: usize,
}

/// A speaker label with case, punctuation, and spacing taken out, so spelling variants gather: "INT.", "Int", and "int:"
/// are one speaker. A label with no letters or digits keeps its own text.
pub fn speaker_key(label: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in label.chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    if out.is_empty() {
        label.trim().to_lowercase()
    } else {
        out
    }
}

/// Where the words of a text start and end, as character positions. An apostrophe inside a word belongs to it ("don't").
fn word_bounds(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_alphanumeric() {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || ((chars[i] == '\'' || chars[i] == '\u{2019}') && i > start && chars.get(i + 1).is_some_and(|c| c.is_alphanumeric()))) {
                i += 1;
            }
            words.push((start, i));
        } else {
            i += 1;
        }
    }
    words
}

/// Widens a found phrase by `n` words on each side, counted from the phrase's first and last word. Never leaves the scope.
fn widen_in(words: &[(usize, usize)], s: usize, e: usize, n: usize) -> (usize, usize) {
    if words.is_empty() {
        return (s, e);
    }
    let first = words.partition_point(|w| w.1 <= s);
    let after_last = words.partition_point(|w| w.0 < e);
    if first >= words.len() || after_last == 0 || first >= after_last {
        return (s, e);
    }
    let i0 = first.saturating_sub(n);
    let i1 = (after_last - 1 + n).min(words.len() - 1);
    (words[i0].0.min(s), words[i1].1.max(e))
}

/// The number of leading and trailing white-space characters of a piece of text.
fn blank_edges(text: &str) -> (i64, i64) {
    let lead = text.chars().take_while(|c| c.is_whitespace()).count() as i64;
    if lead as usize == text.chars().count() {
        return (lead, 0);
    }
    let trail = text.chars().rev().take_while(|c| c.is_whitespace()).count() as i64;
    (lead, trail)
}

impl Project {
    /// The speakers of the chosen documents, spelling variants gathered, busiest first.
    pub fn speaker_groups(&self, filter: &SearchFilter) -> Result<Vec<SpeakerGroup>> {
        struct Acc {
            spellings: HashMap<String, usize>,
            turns: usize,
            left_out: usize,
            sources: HashSet<i64>,
        }
        let mut groups: HashMap<String, Acc> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT speaker, excluded FROM unit WHERE source_id = ?1 AND speaker IS NOT NULL AND trim(speaker) <> ''")?;
        for (source_id, _) in self.scoped_sources(filter)? {
            let rows = stmt.query_map([source_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0)))?;
            for row in rows {
                let (label, excluded) = row?;
                let acc = groups.entry(speaker_key(&label)).or_insert_with(|| Acc { spellings: HashMap::new(), turns: 0, left_out: 0, sources: HashSet::new() });
                *acc.spellings.entry(label.trim().to_string()).or_default() += 1;
                if excluded {
                    acc.left_out += 1;
                } else {
                    acc.turns += 1;
                }
                acc.sources.insert(source_id);
            }
        }
        let mut out: Vec<SpeakerGroup> = groups
            .into_iter()
            .map(|(key, acc)| {
                let mut variants: Vec<(String, usize)> = acc.spellings.into_iter().collect();
                // The spelling used most often, then one written like a name (Interviewer) before one in capitals or in lower case.
                let written_like_a_name = |s: &str| {
                    let mut chars = s.chars().filter(|c| c.is_alphabetic());
                    chars.next().is_some_and(char::is_uppercase) && chars.any(char::is_lowercase)
                };
                variants.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| written_like_a_name(&b.0).cmp(&written_like_a_name(&a.0))).then_with(|| a.0.cmp(&b.0)));
                SpeakerGroup {
                    key,
                    label: variants[0].0.clone(),
                    variants: variants.into_iter().map(|v| v.0).collect(),
                    turns: acc.turns,
                    left_out: acc.left_out,
                    sources: acc.sources.len(),
                }
            })
            .collect();
        out.sort_by(|a, b| (b.turns + b.left_out).cmp(&(a.turns + a.left_out)).then_with(|| a.label.cmp(&b.label)));
        Ok(out)
    }

    /// The turns, left out of coding aside, of the speakers with these keys, in document order.
    pub fn speaker_turns(&self, filter: &SearchFilter, keys: &[String]) -> Result<Vec<SpeakerTurn>> {
        let wanted: HashSet<&str> = keys.iter().map(String::as_str).collect();
        let mut out = Vec::new();
        let mut stmt = self
            .conn
            .prepare("SELECT id, start_offset, end_offset, speaker FROM unit WHERE source_id = ?1 AND excluded = 0 AND speaker IS NOT NULL ORDER BY start_offset")?;
        for (source_id, source_name) in self.scoped_sources(filter)? {
            let rows = stmt.query_map([source_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?)))?;
            for row in rows {
                let (unit_id, start, end, speaker) = row?;
                let key = speaker_key(&speaker);
                if wanted.contains(key.as_str()) {
                    out.push(SpeakerTurn { source_id, source_name: source_name.clone(), unit_id, start, end, speaker: speaker.trim().to_string(), key });
                }
            }
        }
        Ok(out)
    }

    /// How many headings of each level the chosen documents have (from the formatting kept when they were imported).
    pub fn heading_levels(&self, filter: &SearchFilter) -> Result<Vec<HeadingLevel>> {
        let mut counts: HashMap<u8, (usize, usize)> = HashMap::new();
        for (source_id, _) in self.scoped_sources(filter)? {
            let runs = self.formatting_runs_of(source_id)?;
            let mut seen: HashSet<u8> = HashSet::new();
            for run in runs.iter().filter(|r| r.kind == RunKind::Heading) {
                let level = run.level.unwrap_or(1);
                let entry = counts.entry(level).or_default();
                entry.0 += 1;
                if seen.insert(level) {
                    entry.1 += 1;
                }
            }
        }
        let mut out: Vec<HeadingLevel> = counts.into_iter().map(|(level, (count, sources))| HeadingLevel { level, count, sources }).collect();
        out.sort_by_key(|h| h.level);
        Ok(out)
    }

    fn formatting_runs_of(&self, source_id: i64) -> Result<Vec<FormatRun>> {
        let json: String = self.conn.query_row("SELECT formatting_runs_json FROM source WHERE id = ?1", [source_id], |r| r.get(0))?;
        Ok(serde_json::from_str(&json)?)
    }

    /// The sections under headings of the chosen levels, optionally only those whose heading matches `heading_query`
    /// (the search language). A section runs from its heading to the next heading of the same or a higher level; with
    /// `include_heading` false it starts after the heading. Blank edges are trimmed, and sections that overlap (a level 1
    /// heading and a level 2 heading inside it, both chosen) are joined into one.
    pub fn heading_sections(&self, filter: &SearchFilter, levels: &[u8], heading_query: Option<&str>, include_heading: bool) -> Result<Vec<Section>> {
        let query = match heading_query.map(str::trim).filter(|q| !q.is_empty()) {
            Some(q) => Some(Query::parse(q)?),
            None => None,
        };
        let mut out: Vec<Section> = Vec::new();
        for (source_id, source_name) in self.scoped_sources(filter)? {
            let mut heads: Vec<FormatRun> = self.formatting_runs_of(source_id)?.into_iter().filter(|r| r.kind == RunKind::Heading).collect();
            if heads.is_empty() {
                continue;
            }
            heads.sort_by_key(|r| (r.start, r.end));
            let text: String = self.conn.query_row("SELECT text FROM source WHERE id = ?1", [source_id], |r| r.get(0))?;
            let total = text.chars().count() as i64;
            let mut counter = CharCounter::new(&text);
            let mut found: Vec<Section> = Vec::new();
            for (i, h) in heads.iter().enumerate() {
                let level = h.level.unwrap_or(1);
                if !levels.contains(&level) {
                    continue;
                }
                let (hs, he) = (h.start.clamp(0, total), h.end.clamp(0, total));
                let (b0, b1) = (counter.byte_of(hs), counter.byte_of(he));
                let title = text[b0..b1.max(b0)].trim().to_string();
                if let Some(q) = &query {
                    if q.find_in(&title).is_none() {
                        continue;
                    }
                }
                let section_end = heads[i + 1..].iter().find(|n| n.level.unwrap_or(1) <= level).map_or(total, |n| n.start.clamp(0, total));
                let begin = if include_heading { hs } else { he };
                if section_end <= begin {
                    continue;
                }
                let (c0, c1) = (counter.byte_of(begin), counter.byte_of(section_end));
                let (lead, trail) = blank_edges(&text[c0..c1]);
                let (start, end) = (begin + lead, section_end - trail);
                if end > start {
                    found.push(Section { source_id, source_name: source_name.clone(), start, end, heading: title, level });
                }
            }
            found.sort_by_key(|s| s.start);
            for s in found {
                match out.last_mut() {
                    Some(last) if last.source_id == source_id && s.start < last.end => last.end = last.end.max(s.end),
                    _ => out.push(s),
                }
            }
        }
        Ok(out)
    }

    /// Widens found phrases by `n` words on each side, staying inside the unit or paragraph each was found in.
    pub fn widen_by_words(&self, spans: &[FoundSpan], n: usize) -> Result<Vec<FoundSpan>> {
        let mut result: Vec<Option<FoundSpan>> = vec![None; spans.len()];
        let mut by_source: HashMap<i64, Vec<usize>> = HashMap::new();
        for (i, s) in spans.iter().enumerate() {
            by_source.entry(s.source_id).or_default().push(i);
        }
        for (source_id, indexes) in by_source {
            let text: String = self.conn.query_row("SELECT text FROM source WHERE id = ?1", [source_id], |r| r.get(0))?;
            let mut counter = CharCounter::new(&text);
            for i in indexes {
                let f = &spans[i];
                let (b0, b1) = (counter.byte_of(f.scope_start), counter.byte_of(f.scope_end));
                let words = word_bounds(&text[b0..b1.max(b0)]);
                let rel = |p: i64| (p - f.scope_start).max(0) as usize;
                let (s, e) = widen_in(&words, rel(f.start), rel(f.end), n);
                result[i] = Some(FoundSpan { start: f.scope_start + s as i64, end: f.scope_start + e as i64, ..f.clone() });
            }
        }
        Ok(result.into_iter().flatten().collect())
    }

    /// Takes out the suggestions that repeat earlier work: words the same coder already gave this code to, and the same
    /// suggestion already waiting in the queue. Says how many of each were left out.
    pub fn drop_known(&self, items: Vec<NewProposal>) -> Result<KnownDropped> {
        // For each (code, coder): per document, the coded spans by start with the furthest end reached so far, so "is
        // this span inside one already coded?" is a binary search.
        type Spans = HashMap<i64, (Vec<i64>, Vec<i64>)>;
        let mut coded: HashMap<(i64, i64), Spans> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT source_id, start_offset, end_offset FROM coding WHERE code_id = ?1 AND coder_id = ?2 AND layer = 'coder' ORDER BY source_id, start_offset")?;
        for key in items.iter().map(|i| (i.code_id, i.coder_id)).collect::<HashSet<_>>() {
            let mut spans: Spans = HashMap::new();
            let rows = stmt.query_map([key.0, key.1], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?;
            for row in rows {
                let (source, s, e) = row?;
                let entry = spans.entry(source).or_default();
                let reach = entry.1.last().map_or(e, |m| (*m).max(e));
                entry.0.push(s);
                entry.1.push(reach);
            }
            coded.insert(key, spans);
        }
        let mut waiting: HashSet<(i64, i64, i64, i64)> = HashSet::new();
        let mut pending = self.conn.prepare(
            "SELECT CAST(json_extract(payload_json, '$.source_id') AS INTEGER), CAST(json_extract(payload_json, '$.start') AS INTEGER),
                    CAST(json_extract(payload_json, '$.end') AS INTEGER), CAST(json_extract(payload_json, '$.code_id') AS INTEGER)
             FROM proposal WHERE status = 'pending'",
        )?;
        for row in pending.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))? {
            waiting.insert(row?);
        }
        let mut out = KnownDropped { kept: Vec::with_capacity(items.len()), already_coded: 0, already_waiting: 0 };
        for item in items {
            let is_coded = coded.get(&(item.code_id, item.coder_id)).and_then(|spans| spans.get(&item.source_id)).is_some_and(|(starts, reach)| {
                let n = starts.partition_point(|s| *s <= item.start);
                n > 0 && reach[n - 1] >= item.end
            });
            if is_coded {
                out.already_coded += 1;
            } else if waiting.contains(&(item.source_id, item.start, item.end, item.code_id)) {
                out.already_waiting += 1;
            } else {
                out.kept.push(item);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_labels_that_differ_only_in_case_or_punctuation_are_one_speaker() {
        assert_eq!(speaker_key("Participant A:"), speaker_key("participant a"));
        assert_eq!(speaker_key("INT."), speaker_key("Int"));
        assert_eq!(speaker_key("Mary-Jane"), speaker_key("Mary Jane"));
        assert_ne!(speaker_key("Interviewer"), speaker_key("Interviewer 2"));
        assert_eq!(speaker_key("???"), "???", "a label with no letters keeps its text");
    }

    #[test]
    fn words_are_found_with_their_apostrophes() {
        let w = word_bounds("I don't know, 'really'.");
        let text: Vec<String> = w.iter().map(|(a, b)| "I don't know, 'really'.".chars().skip(*a).take(b - a).collect()).collect();
        assert_eq!(text, ["I", "don't", "know", "really"]);
    }

    #[test]
    fn widening_adds_words_each_side_and_stops_at_the_scope() {
        let text = "one two three four five six";
        let words = word_bounds(text);
        let at = |s: usize, e: usize| text.chars().skip(s).take(e - s).collect::<String>();
        // "three four" is characters 8..18.
        assert_eq!(widen_in(&words, 8, 18, 0), (8, 18));
        let (s, e) = widen_in(&words, 8, 18, 1);
        assert_eq!(at(s, e), "two three four five");
        let (s, e) = widen_in(&words, 8, 18, 9);
        assert_eq!(at(s, e), text, "never past the start or end of the scope");
        // A phrase inside punctuation only is left alone.
        assert_eq!(widen_in(&word_bounds("... ---"), 1, 2, 3), (1, 2));
    }

    #[test]
    fn blank_edges_are_counted_from_both_ends() {
        assert_eq!(blank_edges("  hello \n"), (2, 2));
        assert_eq!(blank_edges("hello"), (0, 0));
        assert_eq!(blank_edges("   "), (3, 0));
    }
}
