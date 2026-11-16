//! Text analytics, the first part (specification 20): which words the material uses, and where a word is used. Nothing is guessed. A word
//! is a run of letters or digits, with an apostrophe or a hyphen allowed inside it, written in lower case; a count is a count. Words
//! are told apart without any dictionary, so the lists of common words to leave out are English only, and the person can turn them off.
//!
//! Documents that are pictures are left out: their text is a description for a screen reader, not the material.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::search::SearchFilter;
use crate::{Error, Result};

/// The most words one answer lists, and the most contexts one answer returns.
pub const MAX_WORDS: usize = 5_000;
pub const MAX_CONTEXTS: usize = 1_000;

/// The common English words left out of a count of words when the person asks for that (and by default).
pub(crate) const COMMON_WORDS: &str = "a about above after again against all am an and any are aren't as at be because been before being below between both but by can can't cannot could couldn't did didn't do does doesn't doing don't down during each few for from further had hadn't has hasn't have haven't having he he'd he'll he's her here here's hers herself him himself his how how's i i'd i'll i'm i've if in into is isn't it it's its itself let's me more most mustn't my myself no nor not of off on once only or other ought our ours ourselves out over own same shan't she she'd she'll she's should shouldn't so some such than that that's the their theirs them themselves then there there's these they they'd they'll they're they've this those through to too under until up very was wasn't we we'd we'll we're we've were weren't what what's when when's where where's which while who who's whom why why's with won't would wouldn't you you'd you'll you're you've your yours yourself yourselves";

fn yes() -> bool {
    true
}
fn default_min_length() -> usize {
    3
}
fn default_limit() -> usize {
    100
}
fn default_around() -> usize {
    6
}
fn default_contexts() -> usize {
    200
}

/// What is known of a word while the words are counted: how often, in which documents, and in which forms.
type Tally = (usize, HashSet<i64>, HashMap<String, usize>);

/// What to count.
#[derive(Debug, Clone, Deserialize)]
pub struct WordSpec {
    /// Only these documents.
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    /// Only the documents of this case.
    #[serde(default)]
    pub case_id: Option<i64>,
    /// Only the words inside passages given one of these codes (a code includes its sub-codes). Empty means every word.
    #[serde(default)]
    pub code_ids: Vec<i64>,
    /// Leave out the common English words (the default).
    #[serde(default = "yes")]
    pub leave_out_common: bool,
    /// Leave out words shorter than this many letters.
    #[serde(default = "default_min_length")]
    pub min_length: usize,
    /// List only words used at least this many times.
    #[serde(default)]
    pub min_count: usize,
    /// List at most this many words (the commonest first).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Count the forms of an English word as one ("connect", "connected", "connecting"), by its stem (decision 202).
    #[serde(default)]
    pub stem: bool,
}

impl Default for WordSpec {
    fn default() -> Self {
        Self { source_ids: None, case_id: None, code_ids: Vec::new(), leave_out_common: true, min_length: 3, min_count: 0, limit: 100, stem: false }
    }
}

/// One form of a word that was counted with others under one stem.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordForm {
    pub word: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordCount {
    /// The word; when forms are counted together, the form used most (the first alphabetically among equals).
    pub word: String,
    pub count: usize,
    /// In how many documents the word is used.
    pub documents: usize,
    /// When forms are counted together and there was more than one: every form and how often it was used, the commonest first. Empty otherwise.
    pub forms: Vec<WordForm>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordFrequencies {
    /// The commonest words first, then alphabetically.
    pub words: Vec<WordCount>,
    /// Every word in the documents counted, before any were left out.
    pub total_words: usize,
    /// The words counted: all of them less those left out for being common or short.
    pub counted_words: usize,
    /// How many different words were counted, whether or not all are listed.
    pub distinct_words: usize,
    /// How many documents were read.
    pub documents: usize,
}

/// Where to find a word or phrase.
#[derive(Debug, Clone, Deserialize)]
pub struct ContextSpec {
    /// The word, or the words of a phrase in order.
    pub query: String,
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub case_id: Option<i64>,
    /// How many words to show on each side.
    #[serde(default = "default_around")]
    pub words_around: usize,
    #[serde(default = "default_contexts")]
    pub limit: usize,
}

/// One place a word is used, with the words round it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Context {
    pub source_id: i64,
    pub source_name: String,
    /// Where the words found start and end in the document, counted in characters.
    pub start: i64,
    pub end: i64,
    pub left: String,
    pub hit: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contexts {
    pub items: Vec<Context>,
    /// How many places there are in all, which may be more than were returned.
    pub total: usize,
    /// In how many documents the word or phrase is used.
    pub documents: usize,
}

/// A word and where it sits in the text, counted in characters.
pub(crate) struct Token {
    pub(crate) word: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// The words of a text. A word is letters and digits; an apostrophe or hyphen between two of them stays in it ("don't", "well-being"),
/// and a curly apostrophe is made straight so the same word is counted once.
pub(crate) fn tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        let mut word = String::new();
        while i < chars.len() {
            let c = chars[i];
            if c.is_alphanumeric() {
                word.extend(c.to_lowercase());
                i += 1;
            } else if matches!(c, '\'' | '\u{2019}' | '-') && i + 1 < chars.len() && chars[i + 1].is_alphanumeric() && i > start {
                word.push(if c == '\u{2019}' { '\'' } else { c });
                i += 1;
            } else {
                break;
            }
        }
        out.push(Token { word, start, end: i });
    }
    out
}

/// The spans of text that carry one of these codes (or a sub-code of one), joined so none overlaps another.
fn merged(mut spans: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    spans.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (s, e) in spans {
        match out.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// True when the word that starts at `start` and ends at `end` lies inside one of the spans.
fn inside(spans: &[(i64, i64)], start: usize, end: usize) -> bool {
    let (start, end) = (start as i64, end as i64);
    let n = spans.partition_point(|s| s.1 <= start);
    spans.get(n).is_some_and(|s| s.0 <= start && end <= s.1)
}

impl Project {
    /// The codes whose passages count for a code chosen: it and everything beneath it.
    fn code_family(&self, chosen: &[i64]) -> Result<HashSet<i64>> {
        let codes = self.list_codes()?;
        let mut family: HashSet<i64> = HashSet::new();
        for id in chosen {
            if !codes.iter().any(|c| c.id == *id) {
                return Err(Error::NotFound { what: "code", id: *id });
            }
            family.insert(*id);
        }
        loop {
            let before = family.len();
            for c in &codes {
                if c.parent_id.is_some_and(|p| family.contains(&p)) {
                    family.insert(c.id);
                }
            }
            if family.len() == before {
                break;
            }
        }
        Ok(family)
    }

    /// The words the documents use and how often, the commonest first.
    pub fn word_frequencies(&self, spec: &WordSpec) -> Result<WordFrequencies> {
        let limit = spec.limit.clamp(1, MAX_WORDS);
        let filter = SearchFilter { source_ids: spec.source_ids.clone(), case_id: spec.case_id, ..Default::default() };
        let family = if spec.code_ids.is_empty() { None } else { Some(self.code_family(&spec.code_ids)?) };
        let common: HashSet<&str> = if spec.leave_out_common { COMMON_WORDS.split_whitespace().collect() } else { HashSet::new() };

        let mut counts: HashMap<String, Tally> = HashMap::new();
        let (mut total, mut counted, mut documents) = (0usize, 0usize, 0usize);
        for (source_id, _) in self.scoped_sources(&filter)? {
            if self.get_image(source_id)?.is_some() {
                continue;
            }
            documents += 1;
            let text = self.source_text(source_id)?;
            let spans = match &family {
                None => None,
                Some(family) => {
                    let mut spans = Vec::new();
                    for c in self.list_codings(source_id)? {
                        if c.layer == "coder" && family.contains(&c.code_id) {
                            spans.push((c.start, c.end));
                        }
                    }
                    Some(merged(spans))
                }
            };
            for t in tokens(&text) {
                if spans.as_ref().is_some_and(|s| !inside(s, t.start, t.end)) {
                    continue;
                }
                total += 1;
                if t.word.chars().count() < spec.min_length || common.contains(t.word.as_str()) {
                    continue;
                }
                counted += 1;
                let key = if spec.stem { crate::stem::stem(&t.word) } else { t.word.clone() };
                let entry = counts.entry(key).or_default();
                entry.0 += 1;
                entry.1.insert(source_id);
                if spec.stem {
                    *entry.2.entry(t.word).or_default() += 1;
                }
            }
        }
        let distinct = counts.len();
        let mut words: Vec<WordCount> = counts
            .into_iter()
            .filter(|(_, (n, _, _))| *n >= spec.min_count)
            .map(|(word, (count, docs, forms))| {
                let mut forms: Vec<WordForm> = forms.into_iter().map(|(word, count)| WordForm { word, count }).collect();
                forms.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.word.cmp(&b.word)));
                let word = forms.first().map(|f| f.word.clone()).unwrap_or(word);
                if forms.len() < 2 {
                    forms.clear();
                }
                WordCount { word, count, documents: docs.len(), forms }
            })
            .collect();
        words.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.word.cmp(&b.word)));
        words.truncate(limit);
        Ok(WordFrequencies { words, total_words: total, counted_words: counted, distinct_words: distinct, documents })
    }

    /// Every place a word or phrase is used (whole words, without regard to capitals), with the words round it, in reading order.
    pub fn keywords_in_context(&self, spec: &ContextSpec) -> Result<Contexts> {
        let wanted: Vec<String> = tokens(&spec.query).into_iter().map(|t| t.word).collect();
        if wanted.is_empty() {
            return Err(Error::Invalid("give a word, or the words of a phrase, to look for".to_string()));
        }
        let limit = spec.limit.clamp(1, MAX_CONTEXTS);
        let around = spec.words_around.min(50);
        let filter = SearchFilter { source_ids: spec.source_ids.clone(), case_id: spec.case_id, ..Default::default() };
        let (mut items, mut total, mut documents) = (Vec::new(), 0usize, 0usize);
        for (source_id, source_name) in self.scoped_sources(&filter)? {
            if self.get_image(source_id)?.is_some() {
                continue;
            }
            let before = total;
            let text = self.source_text(source_id)?;
            let toks = tokens(&text);
            let chars: Vec<char> = text.chars().collect();
            let slice = |from: usize, to: usize| -> String { chars[from.min(chars.len())..to.min(chars.len())].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ") };
            let mut i = 0;
            while i + wanted.len() <= toks.len() {
                if toks[i..i + wanted.len()].iter().zip(&wanted).all(|(t, w)| &t.word == w) {
                    total += 1;
                    if items.len() < limit {
                        let (first, last) = (&toks[i], &toks[i + wanted.len() - 1]);
                        let from = toks[i.saturating_sub(around)].start;
                        let to = toks[(i + wanted.len() - 1 + around).min(toks.len() - 1)].end;
                        items.push(Context {
                            source_id,
                            source_name: source_name.clone(),
                            start: first.start as i64,
                            end: last.end as i64,
                            left: slice(from, first.start),
                            hit: slice(first.start, last.end),
                            right: slice(last.end, to),
                        });
                    }
                    i += wanted.len();
                } else {
                    i += 1;
                }
            }
            if total > before {
                documents += 1;
            }
        }
        Ok(Contexts { items, total, documents })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        tokens(text).into_iter().map(|t| t.word).collect()
    }

    #[test]
    fn a_word_is_letters_and_digits_with_an_apostrophe_or_hyphen_inside() {
        assert_eq!(words("Don't stop: well-being, 2024 \u{201C}quoted\u{201D}!"), ["don't", "stop", "well-being", "2024", "quoted"]);
        assert_eq!(words("it\u{2019}s fine"), ["it's", "fine"], "a curly apostrophe is made straight");
        assert_eq!(words("- dash -- and 'single' quotes"), ["dash", "and", "single", "quotes"], "marks at the edge are not part of a word");
        assert_eq!(words("\u{c9}COLE caf\u{e9} na\u{ef}ve"), ["\u{e9}cole", "caf\u{e9}", "na\u{ef}ve"], "letters beyond ASCII are letters");
    }

    #[test]
    fn positions_count_characters_not_bytes() {
        let t = tokens("caf\u{e9} au lait");
        assert_eq!((t[0].start, t[0].end), (0, 4));
        assert_eq!((t[1].start, t[1].end), (5, 7));
    }

    #[test]
    fn spans_are_joined_and_a_word_is_inside_only_when_all_of_it_is() {
        let spans = merged(vec![(10, 20), (0, 5), (4, 8), (30, 40)]);
        assert_eq!(spans, [(0, 8), (10, 20), (30, 40)]);
        assert!(inside(&spans, 0, 8) && inside(&spans, 12, 15));
        assert!(!inside(&spans, 6, 9), "a word that runs out of the span is not inside it");
        assert!(!inside(&spans, 21, 25));
    }
}
