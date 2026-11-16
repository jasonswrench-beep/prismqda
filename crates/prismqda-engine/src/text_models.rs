//! Text analytics, the second part (decision 202): topics and sentiment, worked out from the words of the passages with nothing sent anywhere
//! and no model of language downloaded. Both are ways to find passages worth reading, and neither is a finding.
//!
//! *Topics* are groups of words that tend to appear in the same passages, found by non-negative matrix factorization of the passages by their
//! words (weighted so a word every passage uses counts for little). The number of topics is the person's choice. The start is a fixed
//! sequence and the steps are fixed, so the same passages and settings give the same topics every time.
//!
//! *Sentiment* counts the words of a list of positive and negative English words in each passage, turning a word round when one of the three
//! words before it is a word of denial ("not good"). The built-in list is a short one written for this program; the person can add to it and
//! take words out of it, because a list knows nothing of a field's own use of words.
//!
//! The units of the text (the passages as they were imported) are what is read, and pictures are left out.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::search::SearchFilter;
use crate::text_analytics::{tokens, COMMON_WORDS};
use crate::{Error, Result};

/// The most topics asked for, and the most passages read for topics.
pub const MAX_TOPICS: usize = 20;
pub const MAX_TOPIC_PASSAGES: usize = 10_000;
/// The most distinct words the topics are worked out from (the ones used in the most passages).
const VOCABULARY: usize = 2_000;
const ITERATIONS: usize = 200;
const EXAMPLES_PER_TOPIC: usize = 3;
const MAX_TEXT_CHARS: usize = 300;

fn yes() -> bool {
    true
}

fn five() -> usize {
    5
}

fn eight() -> usize {
    8
}

/// A passage to read: a unit of a document that is text.
struct Passage {
    unit_id: i64,
    source_id: i64,
    source_name: String,
    start: i64,
    end: i64,
    text: String,
}

fn cut(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= MAX_TEXT_CHARS {
        return text;
    }
    let mut out: String = text.chars().take(MAX_TEXT_CHARS - 1).collect();
    out.push('\u{2026}');
    out
}

impl Project {
    /// The passages of the documents chosen, in reading order. A picture has no passages that count.
    fn text_passages(&self, source_ids: &Option<Vec<i64>>, case_id: Option<i64>) -> Result<Vec<Passage>> {
        let filter = SearchFilter { source_ids: source_ids.clone(), case_id, ..Default::default() };
        let mut out = Vec::new();
        for (source_id, source_name) in self.scoped_sources(&filter)? {
            if self.get_image(source_id)?.is_some() {
                continue;
            }
            let chars: Vec<char> = self.source_text(source_id)?.chars().collect();
            for u in self.list_units(source_id)? {
                if u.excluded {
                    continue;
                }
                let (s, e) = (u.start.max(0) as usize, (u.end.max(0) as usize).min(chars.len()));
                if s >= e {
                    continue;
                }
                out.push(Passage { unit_id: u.id, source_id, source_name: source_name.clone(), start: u.start, end: u.end, text: chars[s..e].iter().collect() });
            }
        }
        Ok(out)
    }
}

// ---- topics ------------------------------------------------------------------------------------------------------------------------

/// What to find topics in, and how many.
#[derive(Debug, Clone, Deserialize)]
pub struct TopicSpec {
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub case_id: Option<i64>,
    /// How many topics to look for, from 2 to [`MAX_TOPICS`].
    #[serde(default = "five")]
    pub topics: usize,
    /// How many words to list for each topic.
    #[serde(default = "eight")]
    pub words_per_topic: usize,
    /// Leave out the common English words (the default).
    #[serde(default = "yes")]
    pub leave_out_common: bool,
    /// Treat the forms of an English word as one (decision 202).
    #[serde(default)]
    pub stem: bool,
}

impl Default for TopicSpec {
    fn default() -> Self {
        Self { source_ids: None, case_id: None, topics: 5, words_per_topic: 8, leave_out_common: true, stem: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicWord {
    pub word: String,
    /// Against the topic's strongest word, from 0 to 1.
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicPassage {
    pub unit_id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    pub text: String,
    /// How much of the passage belongs to this topic, from 0 to 1.
    pub strength: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Topic {
    /// From 1, the topic with the most passages first.
    pub number: usize,
    /// The three strongest words.
    pub label: String,
    pub words: Vec<TopicWord>,
    /// The passages whose strongest topic this is.
    pub passages: usize,
    /// Those over all the passages read.
    pub share: f64,
    /// The passages that belong to it most.
    pub examples: Vec<TopicPassage>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicReport {
    pub topics: Vec<Topic>,
    /// The passages read.
    pub passages_used: usize,
    /// Passages with too few of the words used to say anything about them.
    pub passages_left_out: usize,
    /// How many different words the topics were worked out from.
    pub vocabulary: usize,
    pub notes: Vec<String>,
}

/// A fixed sequence of numbers between 0 and 1, so the same text gives the same topics.
struct Sequence(u64);

impl Sequence {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

impl Project {
    /// Topics in the passages of the documents chosen.
    pub fn topics(&self, spec: &TopicSpec) -> Result<TopicReport> {
        let k = spec.topics;
        if !(2..=MAX_TOPICS).contains(&k) {
            return Err(Error::Invalid(format!("ask for from 2 to {MAX_TOPICS} topics")));
        }
        let per_topic = spec.words_per_topic.clamp(3, 20);
        let common: HashSet<&str> = if spec.leave_out_common { COMMON_WORDS.split_whitespace().collect() } else { HashSet::new() };
        let mut passages = self.text_passages(&spec.source_ids, spec.case_id)?;
        let mut notes = Vec::new();
        if passages.len() > MAX_TOPIC_PASSAGES {
            notes.push(format!("{} passages were more than topics are worked out from, so the first {MAX_TOPIC_PASSAGES} in reading order were read.", passages.len()));
            passages.truncate(MAX_TOPIC_PASSAGES);
        }

        // The words of each passage (three letters or more, not common), counted; and the forms each stem was written in.
        let mut counted: Vec<BTreeMap<String, u32>> = Vec::with_capacity(passages.len());
        let mut forms: HashMap<String, BTreeMap<String, usize>> = HashMap::new();
        for p in &passages {
            let mut counts: BTreeMap<String, u32> = BTreeMap::new();
            for t in tokens(&p.text) {
                if t.word.chars().count() < 3 || common.contains(t.word.as_str()) {
                    continue;
                }
                let key = if spec.stem { crate::stem::stem(&t.word) } else { t.word.clone() };
                if spec.stem {
                    *forms.entry(key.clone()).or_default().entry(t.word).or_default() += 1;
                }
                *counts.entry(key).or_default() += 1;
            }
            counted.push(counts);
        }

        // The vocabulary: words in at least two passages, and (when there are enough passages) in no more than half of them.
        let n_all = passages.len();
        let mut df: BTreeMap<&str, usize> = BTreeMap::new();
        for c in &counted {
            for w in c.keys() {
                *df.entry(w.as_str()).or_default() += 1;
            }
        }
        let upper = if n_all >= 10 { n_all / 2 } else { n_all };
        let mut vocab: Vec<(&str, usize)> = df.iter().filter(|(_, n)| **n >= 2 && **n <= upper).map(|(w, n)| (*w, *n)).collect();
        vocab.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        vocab.truncate(VOCABULARY);
        vocab.sort_by(|a, b| a.0.cmp(b.0));
        let index: HashMap<&str, usize> = vocab.iter().enumerate().map(|(j, (w, _))| (*w, j)).collect();

        // The passages that have enough of those words, as rows of weights: (1 + ln count) * ln(1 + N / df), each row of length 1.
        let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
        let mut kept: Vec<usize> = Vec::new();
        for (i, c) in counted.iter().enumerate() {
            let hits: u32 = c.iter().filter(|(w, _)| index.contains_key(w.as_str())).map(|(_, n)| *n).sum();
            if hits < 3 {
                continue;
            }
            let mut row: Vec<(usize, f64)> = c
                .iter()
                .filter_map(|(w, n)| {
                    let j = *index.get(w.as_str())?;
                    let idf = (1.0 + n_all as f64 / vocab[j].1 as f64).ln();
                    Some((j, (1.0 + (*n as f64).ln()) * idf))
                })
                .collect();
            let norm = row.iter().map(|(_, v)| v * v).sum::<f64>().sqrt();
            if norm > 0.0 {
                row.iter_mut().for_each(|(_, v)| *v /= norm);
            }
            rows.push(row);
            kept.push(i);
        }
        let n = rows.len();
        if n < (2 * k).max(6) || vocab.len() < k + 1 {
            return Err(Error::Invalid(format!("there is too little text for {k} topics: {n} passages with enough words and {} different words; ask for fewer topics, or choose more documents", vocab.len())));
        }
        let v = vocab.len();

        // The factors, started from a fixed sequence and improved by a fixed number of steps.
        let mut seq = Sequence(0x2545_F491_4F6C_DD1D);
        let mut w: Vec<Vec<f64>> = (0..n).map(|_| (0..k).map(|_| 0.05 + 0.95 * seq.next()).collect()).collect();
        let mut h: Vec<Vec<f64>> = (0..k).map(|_| (0..v).map(|_| 0.05 + 0.95 * seq.next()).collect()).collect();
        const EPS: f64 = 1e-9;
        for _ in 0..ITERATIONS {
            // H gets the passages' words that its topic explains.
            let mut num = vec![vec![0.0; v]; k];
            for (i, row) in rows.iter().enumerate() {
                for &(j, x) in row {
                    for t in 0..k {
                        num[t][j] += w[i][t] * x;
                    }
                }
            }
            let mut wtw = vec![vec![0.0; k]; k];
            for wi in &w {
                for a in 0..k {
                    for b in 0..k {
                        wtw[a][b] += wi[a] * wi[b];
                    }
                }
            }
            for t in 0..k {
                for j in 0..v {
                    let den: f64 = (0..k).map(|a| wtw[t][a] * h[a][j]).sum();
                    h[t][j] *= num[t][j] / (den + EPS);
                }
            }
            // W gets the topics each passage uses.
            let mut hht = vec![vec![0.0; k]; k];
            for a in 0..k {
                for b in 0..k {
                    hht[a][b] = (0..v).map(|j| h[a][j] * h[b][j]).sum();
                }
            }
            for (i, row) in rows.iter().enumerate() {
                let mut num_w = vec![0.0; k];
                for &(j, x) in row {
                    for t in 0..k {
                        num_w[t] += x * h[t][j];
                    }
                }
                let old = w[i].clone();
                for t in 0..k {
                    let den: f64 = (0..k).map(|a| old[a] * hht[a][t]).sum();
                    w[i][t] = old[t] * num_w[t] / (den + EPS);
                }
            }
        }

        // Each topic on one scale (the sum of its word weights), so a passage's strongest topic does not depend on the topic's scale.
        let scale: Vec<f64> = (0..k).map(|t| h[t].iter().sum::<f64>().max(EPS)).collect();
        let mut share: Vec<Vec<f64>> = Vec::with_capacity(n);
        let mut dominant: Vec<usize> = Vec::with_capacity(n);
        for wi in &w {
            let scaled: Vec<f64> = (0..k).map(|t| wi[t] * scale[t]).collect();
            let total: f64 = scaled.iter().sum();
            let mut best = 0;
            for t in 1..k {
                if scaled[t] > scaled[best] {
                    best = t;
                }
            }
            dominant.push(best);
            share.push(scaled.iter().map(|x| if total > 0.0 { x / total } else { 0.0 }).collect());
        }

        // Topics in the order of the passages they have, the most first; equal ones keep the order they were found in.
        let mut order: Vec<usize> = (0..k).collect();
        let size = |t: usize| dominant.iter().filter(|d| **d == t).count();
        order.sort_by(|a, b| size(*b).cmp(&size(*a)).then(a.cmp(b)));
        let shown = |stem: &str| -> String {
            forms.get(stem).and_then(|f| f.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(word, _)| word.clone())).unwrap_or_else(|| stem.to_string())
        };
        let mut topics = Vec::new();
        for (rank, t) in order.iter().enumerate() {
            let mut ranked: Vec<usize> = (0..v).collect();
            ranked.sort_by(|a, b| h[*t][*b].partial_cmp(&h[*t][*a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b)));
            let top = h[*t][ranked[0]].max(EPS);
            let words: Vec<TopicWord> = ranked.iter().take(per_topic).map(|j| TopicWord { word: shown(vocab[*j].0), weight: h[*t][*j] / top }).collect();
            let mut mine: Vec<usize> = (0..n).filter(|i| dominant[*i] == *t).collect();
            mine.sort_by(|a, b| share[*b][*t].partial_cmp(&share[*a][*t]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b)));
            let examples: Vec<TopicPassage> = mine
                .iter()
                .take(EXAMPLES_PER_TOPIC)
                .map(|i| {
                    let p = &passages[kept[*i]];
                    TopicPassage { unit_id: p.unit_id, source_id: p.source_id, source_name: p.source_name.clone(), start: p.start, end: p.end, text: cut(&p.text), strength: share[*i][*t] }
                })
                .collect();
            topics.push(Topic { number: rank + 1, label: words.iter().take(3).map(|w| w.word.clone()).collect::<Vec<_>>().join(", "), words, passages: mine.len(), share: mine.len() as f64 / n as f64, examples });
        }
        notes.push("A topic is a group of words that tend to appear in the same passages. It is a way to find passages to read, not a finding and not yet a theme: read the passages of each topic before you call it anything. Another number of topics gives other groups, so try more than one.".to_string());
        if spec.stem {
            notes.push("Forms of a word were counted together (connect, connected, connecting); each is shown by the form used most.".to_string());
        }
        Ok(TopicReport { topics, passages_used: n, passages_left_out: n_all - n, vocabulary: v, notes })
    }
}

// ---- sentiment ---------------------------------------------------------------------------------------------------------------------

const POSITIVE_WORDS: &str = "good great excellent wonderful amazing fantastic love loved loves happy glad pleased delighted enjoy enjoyed enjoyable helpful friendly kind easy easier comfortable convenient reliable safe clean fast quick better best improved improve improvement success successful positive nice pleasant beautiful perfect thank thanks grateful appreciate appreciated recommend satisfied satisfying supportive welcome welcoming confident hopeful excited exciting fun useful valuable worthwhile clear effective efficient affordable fair generous respectful polite courteous patient caring trust trusted trustworthy honest strong stronger smooth impressive impressed brilliant outstanding superb terrific awesome lovely joy joyful proud relief relieved calm peaceful rewarding empowering inspiring motivated motivating engaging interesting informative thorough organized prompt punctual accessible flexible thoughtful wise benefit beneficial advantage progress thrive thriving growth liked";

const NEGATIVE_WORDS: &str = "bad worse worst terrible awful horrible poor hate hated hates dislike disliked unhappy sad angry anger upset annoyed annoying frustrated frustrating frustration disappointed disappointing disappointment difficult confusing confused unclear slow late delayed delay expensive costly unfair rude unfriendly unhelpful dirty unsafe dangerous broken fail failed failure failing problem problems trouble worry worried worrying afraid fear scared stress stressed stressful anxious anxiety pain painful hurt harm harmful lonely isolated ignored neglected abandoned disrespect disrespectful discrimination biased mistake mistakes wrong useless waste wasted boring tedious tiring exhausted overwhelmed overwhelming struggle struggled struggling complain complaint complaints unacceptable inadequate insufficient lacking lack shortage cancel cancelled canceled crowded noisy uncomfortable unreliable inconvenient impossible refuse refused denied reject rejected blame blamed guilty ashamed shame hopeless helpless unsupported unwelcome threat threatened abuse abused violence violent sick";

/// Words that turn the meaning of one of the next three words round.
const DENIALS: &str = "not no never none nobody nothing neither nor cannot without hardly barely scarcely";

/// What to read for sentiment, and the person's own additions to the word lists.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SentimentSpec {
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub case_id: Option<i64>,
    /// Words to count as positive, over and above the built-in list.
    #[serde(default)]
    pub positive_words: Vec<String>,
    /// Words to count as negative, over and above the built-in list.
    #[serde(default)]
    pub negative_words: Vec<String>,
    /// Leave the built-in list out and use only the person's words.
    #[serde(default)]
    pub own_words_only: bool,
    /// How many of the most positive and most negative passages to give (up to 20).
    #[serde(default = "five")]
    pub examples: usize,
}

/// How many passages were of each kind: only positive words, only negative words, both, or neither.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SentimentCounts {
    pub positive: usize,
    pub negative: usize,
    pub mixed: usize,
    pub neutral: usize,
}

impl SentimentCounts {
    fn add(&mut self, kind: &str) {
        match kind {
            "positive" => self.positive += 1,
            "negative" => self.negative += 1,
            "mixed" => self.mixed += 1,
            _ => self.neutral += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SentimentGroup {
    pub name: String,
    pub counts: SentimentCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SentimentPassage {
    pub unit_id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    pub text: String,
    /// The words that counted as positive (a word turned round by a denial is written "not good" and counted as negative), and as negative.
    pub positive_words: Vec<String>,
    pub negative_words: Vec<String>,
    /// Positive words less negative ones.
    pub net: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SentimentReport {
    pub passages: usize,
    pub counts: SentimentCounts,
    pub by_document: Vec<SentimentGroup>,
    /// For each code anyone gave to a passage: how many of its passages were of each kind, the code with most passages first.
    pub by_code: Vec<SentimentGroup>,
    pub most_positive: Vec<SentimentPassage>,
    pub most_negative: Vec<SentimentPassage>,
    /// How many words each list had.
    pub positive_list: usize,
    pub negative_list: usize,
    pub notes: Vec<String>,
}

fn clean_words(words: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in words {
        for t in tokens(w) {
            if !out.contains(&t.word) {
                out.push(t.word);
            }
        }
    }
    out
}

impl Project {
    /// How positive or negative the passages of the documents chosen are, by the words in them (see the module's note on what that means).
    pub fn sentiment(&self, spec: &SentimentSpec) -> Result<SentimentReport> {
        let mut positive: HashSet<String> = if spec.own_words_only { HashSet::new() } else { POSITIVE_WORDS.split_whitespace().map(String::from).collect() };
        let mut negative: HashSet<String> = if spec.own_words_only { HashSet::new() } else { NEGATIVE_WORDS.split_whitespace().map(String::from).collect() };
        let (own_pos, own_neg) = (clean_words(&spec.positive_words), clean_words(&spec.negative_words));
        if let Some(both) = own_pos.iter().find(|w| own_neg.contains(w)) {
            return Err(Error::Invalid(format!("'{both}' is in both the positive and the negative words you gave")));
        }
        for w in &own_pos {
            negative.remove(w);
            positive.insert(w.clone());
        }
        for w in &own_neg {
            positive.remove(w);
            negative.insert(w.clone());
        }
        if positive.is_empty() && negative.is_empty() {
            return Err(Error::Invalid("there are no words to look for: use the built-in list, or give some".to_string()));
        }
        let denials: HashSet<&str> = DENIALS.split_whitespace().collect();
        let examples = spec.examples.min(20);

        let passages = self.text_passages(&spec.source_ids, spec.case_id)?;
        // The codes anyone gave each passage.
        let human: Vec<i64> = self.list_coders()?.into_iter().filter(|c| c.role == "human").map(|c| c.id).collect();
        let given = self.codes_by_unit(&human, false)?;
        let names: HashMap<i64, String> = self.list_codes()?.into_iter().map(|c| (c.id, c.name)).collect();

        let mut counts = SentimentCounts::default();
        let mut by_document: Vec<SentimentGroup> = Vec::new();
        let mut by_code: HashMap<i64, SentimentCounts> = HashMap::new();
        let mut scored: Vec<SentimentPassage> = Vec::new();
        for p in &passages {
            let toks = tokens(&p.text);
            let (mut pos_words, mut neg_words): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
            for (i, t) in toks.iter().enumerate() {
                let is_pos = positive.contains(&t.word);
                let is_neg = negative.contains(&t.word);
                if !is_pos && !is_neg {
                    continue;
                }
                let denied = toks[i.saturating_sub(3)..i].iter().any(|b| denials.contains(b.word.as_str()) || b.word.ends_with("n't"));
                match (is_pos, denied) {
                    (true, false) => pos_words.push(t.word.clone()),
                    (true, true) => neg_words.push(format!("not {}", t.word)),
                    (false, false) => neg_words.push(t.word.clone()),
                    (false, true) => pos_words.push(format!("not {}", t.word)),
                }
            }
            let kind = match (pos_words.is_empty(), neg_words.is_empty()) {
                (false, true) => "positive",
                (true, false) => "negative",
                (false, false) => "mixed",
                (true, true) => "neutral",
            };
            counts.add(kind);
            match by_document.iter_mut().find(|g| g.name == p.source_name) {
                Some(g) => g.counts.add(kind),
                None => {
                    let mut c = SentimentCounts::default();
                    c.add(kind);
                    by_document.push(SentimentGroup { name: p.source_name.clone(), counts: c });
                }
            }
            let mut coded: HashSet<i64> = HashSet::new();
            for g in &given {
                if let Some(codes) = g.get(&p.unit_id) {
                    coded.extend(codes);
                }
            }
            for code in coded {
                by_code.entry(code).or_default().add(kind);
            }
            if !pos_words.is_empty() || !neg_words.is_empty() {
                scored.push(SentimentPassage { unit_id: p.unit_id, source_id: p.source_id, source_name: p.source_name.clone(), start: p.start, end: p.end, text: cut(&p.text), net: pos_words.len() as i64 - neg_words.len() as i64, positive_words: pos_words, negative_words: neg_words });
            }
        }
        let mut codes: Vec<SentimentGroup> = by_code.into_iter().map(|(id, counts)| SentimentGroup { name: names.get(&id).cloned().unwrap_or_else(|| id.to_string()), counts }).collect();
        let total = |g: &SentimentGroup| g.counts.positive + g.counts.negative + g.counts.mixed + g.counts.neutral;
        codes.sort_by(|a, b| total(b).cmp(&total(a)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        let mut most_positive: Vec<SentimentPassage> = scored.iter().filter(|p| p.net > 0).cloned().collect();
        most_positive.sort_by(|a, b| b.net.cmp(&a.net).then(a.unit_id.cmp(&b.unit_id)));
        most_positive.truncate(examples);
        let mut most_negative: Vec<SentimentPassage> = scored.iter().filter(|p| p.net < 0).cloned().collect();
        most_negative.sort_by(|a, b| a.net.cmp(&b.net).then(a.unit_id.cmp(&b.unit_id)));
        most_negative.truncate(examples);

        let mut notes = vec![
            "A passage counts as positive when it has positive words and no negative ones, negative when it has negative words and no positive ones, mixed when it has both, and neutral when it has none of the words on the lists. A positive or negative word that follows a word of denial (not, never, no, don't and the like) within three words is counted the other way.".to_string(),
            "A word list cannot tell irony, a word used in its field's own way (a health study's 'sick' is not a complaint), or a sentence that denies in other words. Treat the counts as a way to find passages to read, and read a sample of each kind before you rely on them.".to_string(),
        ];
        if !spec.own_words_only {
            notes.push("The built-in list is a short one of everyday English written for PrismQDA; it is yours to add to and to take words out of.".to_string());
        }
        Ok(SentimentReport { passages: passages.len(), counts, by_document, by_code: codes, most_positive, most_negative, positive_list: positive.len(), negative_list: negative.len(), notes })
    }
}
