//! Suggestions for the list of names to find (decisions 138 and 156). Nothing here is sent anywhere and nothing is guessed by a model:
//! these are plain rules, and they are rough. A suggestion is only a place to start; the researcher reads the list, takes out what is
//! not a name, and adds what the rules missed. No tool finds everything, and this one does not claim to.
//!
//! What is suggested:
//! * the speaker labels of transcripts (not interviewers or generic labels such as "Speaker 2"),
//! * the names of cases (not codes such as "P07"),
//! * email addresses, web addresses, telephone numbers, and numbers written like a national identity number,
//! * words that begin with a capital letter in the middle of a sentence, which in most writing are names of people, places, and
//!   organizations. A word that begins a sentence is not counted, because every sentence begins with a capital.

use std::collections::{BTreeMap, HashMap, HashSet};

use regex::Regex;
use serde::Serialize;

use crate::deid::{DeidRule, Scrubber};
use crate::project::Project;
use crate::Result;

/// One thing the program thinks may identify someone.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeidSuggestion {
    pub term: String,
    /// `speaker`, `case`, `email`, `web`, `phone`, `number`, or `name`.
    pub kind: String,
    /// What would take its place. Terms that look like parts of one name (a first name and a full name) get the same label.
    pub replacement: String,
    /// Whether the capital letters matter when finding it (they do for a name found by its capital letter).
    pub match_case: bool,
    /// How many times the list would find it in the documents.
    pub count: usize,
    pub documents: usize,
}

const STOP: &[&str] = &[
    "the", "a", "an", "in", "on", "at", "of", "to", "for", "with", "by", "from", "as", "and", "but", "or", "so", "if", "when", "while", "what", "why", "how", "who", "which", "where",
    "that", "this", "these", "those", "then", "there", "here", "he", "she", "it", "they", "we", "you", "i", "my", "our", "your", "his", "her", "their", "its", "yes", "no", "well",
    "oh", "okay", "ok", "um", "uh", "hmm", "yeah", "sure", "right", "like", "maybe", "because", "after", "before", "also", "however", "just", "now", "not", "do", "did", "does",
    "is", "was", "were", "are", "be", "have", "has", "had", "can", "could", "would", "should", "all", "some", "any", "one", "two", "three", "every", "each", "both", "many", "most",
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday", "january", "february", "march", "april", "may", "june", "july", "august", "september", "october",
    "november", "december", "mr", "mrs", "ms", "dr", "prof", "god", "internet", "christmas", "easter", "thanks", "thank", "please", "sorry", "hi", "hello", "hey", "interviewer",
    "interviewee", "participant", "respondent", "speaker", "moderator", "facilitator", "question", "answer", "q", "a",
    // Words that begin a sentence in talk and writing, and are followed by a name often enough to be taken for part of it.
    "later", "today", "yesterday", "tomorrow", "recently", "finally", "first", "second", "third", "next", "last", "suddenly", "sometimes", "usually", "often", "always", "never",
    "still", "soon", "eventually", "meanwhile", "otherwise", "actually", "honestly", "basically", "personally", "generally", "overall", "instead", "besides", "anyway", "anyways",
    "plus", "again", "once", "since", "until", "though", "although", "whenever", "whatever", "everyone", "everybody", "someone", "somebody", "nobody", "nothing", "something",
    "everything", "another", "other", "such", "only", "even", "rather", "quite", "very", "really", "more", "less", "much", "few", "during", "between", "among", "about", "above",
    "across", "against", "along", "around", "behind", "below", "beside", "beyond", "despite", "except", "inside", "into", "near", "off", "onto", "outside", "over", "past",
    "through", "toward", "towards", "under", "unlike", "upon", "within", "without", "according", "later", "earlier", "morning", "afternoon", "evening", "tonight", "nowadays",
    "thing", "things", "people", "well", "good", "great", "nice", "fine", "wow", "ah", "huh", "mm", "mhm", "alright", "anyhow", "furthermore", "moreover", "therefore", "thus",
    "hence", "perhaps", "probably", "possibly", "certainly", "obviously", "clearly", "apparently", "luckily", "unfortunately", "fortunately", "hopefully", "mostly", "mainly",
];

fn is_stop(word: &str) -> bool {
    let w = word.to_lowercase();
    STOP.contains(&w.as_str())
}

/// Labels that say a role and not a person.
fn is_generic_label(label: &str) -> bool {
    let l = label.trim().to_lowercase();
    let stripped: String = l.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '#').to_string();
    matches!(
        stripped.as_str(),
        "" | "speaker" | "participant" | "respondent" | "interviewer" | "interviewee" | "moderator" | "facilitator" | "unknown" | "host" | "guest" | "q" | "a" | "i" | "r" | "me" | "you" | "researcher" | "subject"
    )
}

/// Something like "P07" or "S3": already a code, not a name.
fn looks_like_a_code(label: &str) -> bool {
    let t = label.trim();
    let letters = t.chars().take_while(|c| c.is_alphabetic()).count();
    let rest = &t[t.char_indices().nth(letters).map(|(i, _)| i).unwrap_or(t.len())..];
    letters <= 3 && !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '_')
}

fn is_capitalized(word: &[char]) -> bool {
    let mut letters = word.iter().filter(|c| c.is_alphabetic());
    let Some(first) = letters.next() else { return false };
    if !first.is_uppercase() {
        return false;
    }
    let rest: Vec<&char> = letters.collect();
    // "I'm" and "I'll" are not names; a word in capitals ("NASA", "OK") is not a name of this kind either.
    if word.len() >= 2 && *first == 'I' && matches!(word[1], '\'' | '\u{2019}') {
        return false;
    }
    !rest.is_empty() && rest.iter().any(|c| c.is_lowercase())
}

/// Counts words that begin with a capital letter in the middle of a sentence, alone or in a run of two to four. Returns the phrases and
/// how many times each stood where a sentence does not begin (a run of two or more words also counts where one does, unless its first word
/// is a word that begins sentences).
fn capitalized_phrases(text: &str, counts: &mut HashMap<String, usize>) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut at_start = true;
    let word_end = |from: usize| -> usize {
        let mut j = from;
        while j < n && (chars[j].is_alphabetic() || ((chars[j] == '\'' || chars[j] == '\u{2019}' || chars[j] == '-') && j + 1 < n && chars[j + 1].is_alphabetic())) {
            j += 1;
        }
        j
    };
    while i < n {
        let c = chars[i];
        if c == '\n' || c == '\r' || matches!(c, '.' | '!' | '?' | ':') {
            at_start = true;
            i += 1;
        } else if c.is_whitespace() || matches!(c, '"' | '\u{201C}' | '\u{201D}' | '(' | '[' | '*' | '_') {
            i += 1;
        } else if c.is_alphabetic() {
            let j = word_end(i);
            let word = &chars[i..j];
            if is_capitalized(word) && !is_stop(&word.iter().collect::<String>()) {
                let mut end = j;
                let mut words = 1;
                while words < 4 && end + 1 < n && chars[end] == ' ' && chars[end + 1].is_uppercase() {
                    let e2 = word_end(end + 1);
                    let next = &chars[end + 1..e2];
                    if is_capitalized(next) && !is_stop(&next.iter().collect::<String>()) {
                        end = e2;
                        words += 1;
                    } else {
                        break;
                    }
                }
                if !at_start || words >= 2 {
                    let phrase: String = chars[i..end].iter().collect();
                    *counts.entry(phrase).or_insert(0) += 1;
                }
                i = end;
            } else {
                i = j;
            }
            at_start = false;
        } else {
            at_start = false;
            i += 1;
        }
    }
}

/// The number at the end of a label such as "Participant 3", if the label starts with `prefix`.
fn number_after(prefix: &str, label: &str) -> Option<u32> {
    label.strip_prefix(prefix)?.trim().parse().ok()
}

impl Project {
    /// What the program thinks may identify someone in the documents, most often found first. Terms already in the list are left out.
    pub fn deid_suggest(&self, source_ids: Option<&[i64]>, limit: usize) -> Result<Vec<DeidSuggestion>> {
        let limit = limit.clamp(1, 500);
        let rules = self.deid_rules()?;
        let known: HashSet<String> = rules.iter().map(|r| r.term.to_lowercase()).collect();
        let sources: Vec<(i64, String)> = self.deid_sources(source_ids)?.into_iter().filter(|s| s.cannot.is_none()).map(|s| (s.id, s.text)).collect();
        let in_scope: HashSet<i64> = sources.iter().map(|s| s.0).collect();

        // term -> (kind, match_case)
        let mut found: BTreeMap<String, (String, bool)> = BTreeMap::new();
        let mut add = |term: &str, kind: &str, match_case: bool| {
            let t = term.split_whitespace().collect::<Vec<_>>().join(" ");
            if t.chars().count() < 2 || known.contains(&t.to_lowercase()) {
                return;
            }
            found.entry(t).or_insert((kind.to_string(), match_case));
        };

        // Speaker labels of the units that are not interviewers' (those are marked excluded).
        {
            let mut stmt = self.conn.prepare("SELECT DISTINCT speaker, source_id FROM unit WHERE speaker IS NOT NULL AND trim(speaker) <> '' AND excluded = 0")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            for row in rows {
                let (label, sid) = row?;
                if in_scope.contains(&sid) && !is_generic_label(&label) && !looks_like_a_code(&label) {
                    add(&label, "speaker", true);
                }
            }
        }
        // The names of cases.
        {
            let mut stmt = self.conn.prepare("SELECT name FROM research_case")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for row in rows {
                let name = row?;
                if !is_generic_label(&name) && !looks_like_a_code(&name) && name.chars().any(|c| c.is_alphabetic()) {
                    add(&name, "case", true);
                }
            }
        }

        let email = Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z0-9\-]+)*\.[A-Za-z]{2,}").expect("pattern");
        let web = Regex::new(r#"(?i)\b(?:https?://|www\.)[^\s<>"')\]]+"#).expect("pattern");
        let ssn = Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("pattern");
        let phone = Regex::new(r"(?:\+\d{1,3}[\s.\-]?)?(?:\(\d{3}\)|\b\d{3})[\s.\-]?\d{3}[\s.\-]?\d{4}\b|\+\d[\d\s().\-]{8,}\d").expect("pattern");
        let iso_date = Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("pattern");
        let mut phrases: HashMap<String, usize> = HashMap::new();
        for (_, text) in &sources {
            for m in email.find_iter(text) {
                add(m.as_str(), "email", false);
            }
            for m in web.find_iter(text) {
                add(m.as_str().trim_end_matches(['.', ',', ';', ':']), "web", false);
            }
            for m in ssn.find_iter(text) {
                add(m.as_str(), "number", false);
            }
            for m in phone.find_iter(text) {
                let digits = m.as_str().chars().filter(|c| c.is_ascii_digit()).count();
                // A date such as 2026-11-09 has eight digits and no brackets; an identity number was taken above.
                if (7..=15).contains(&digits) && !iso_date.is_match(m.as_str()) && !ssn.is_match(m.as_str()) {
                    add(m.as_str(), "phone", false);
                }
            }
            capitalized_phrases(text, &mut phrases);
        }
        // Single words seen in the middle of a sentence, even once: the first or last name of a person whose full name was found.
        let singles: HashSet<String> = phrases.keys().filter(|p| !p.contains(' ')).cloned().collect();
        for (phrase, n) in &phrases {
            let words: Vec<&str> = phrase.split(' ').collect();
            if (words.len() >= 2 && *n >= 1) || *n >= 2 {
                add(phrase, "name", true);
            }
            if words.len() >= 2 {
                for part in [words[0], words[words.len() - 1]] {
                    if part.chars().count() >= 3 && singles.contains(part) {
                        add(part, "name", true);
                    }
                }
            }
        }

        // How often each would be found, with the same rules the list will use.
        let probe: Vec<DeidRule> = found
            .iter()
            .enumerate()
            .map(|(i, (term, (kind, mc)))| DeidRule {
                id: i as i64,
                term: term.clone(),
                replacement: "x".to_string(),
                kind: "pseudonym".to_string(),
                match_case: *mc,
                whole_word: !matches!(kind.as_str(), "email" | "web" | "phone" | "number"),
                note: String::new(),
                created_at: String::new(),
            })
            .collect();
        let terms: Vec<(&String, &(String, bool))> = found.iter().collect();
        let scrubber = Scrubber::new(&probe);
        let mut counts = vec![0usize; probe.len()];
        let mut docs: Vec<HashSet<i64>> = vec![HashSet::new(); probe.len()];
        for (sid, text) in &sources {
            for hit in scrubber.scan(text) {
                let idx = scrubber.rule_id(hit.rule) as usize;
                counts[idx] += 1;
                docs[idx].insert(*sid);
            }
        }

        let mut out: Vec<DeidSuggestion> = Vec::new();
        for (i, (term, (kind, mc))) in terms.iter().enumerate() {
            // A speaker or a case may not be written in the text at all (a label that is only in the unit table); it is still a name.
            if counts[i] == 0 && !matches!(kind.as_str(), "speaker" | "case") {
                continue;
            }
            out.push(DeidSuggestion { term: (*term).clone(), kind: kind.clone(), replacement: String::new(), match_case: *mc, count: counts[i], documents: docs[i].len() });
        }
        out.sort_by(|a, b| b.count.cmp(&a.count).then(a.term.cmp(&b.term)));
        out.truncate(limit);

        // Labels: one number per person, shared by a full name and the first or last name found alone.
        let next = |prefix: &str| -> u32 { rules.iter().filter_map(|r| number_after(prefix, &r.replacement)).max().unwrap_or(0) + 1 };
        let mut next_person = next("Person ");
        let mut next_participant = next("Participant ");
        let mut taken: HashMap<String, String> = HashMap::new();
        let multiword: Vec<String> = out.iter().filter(|s| s.term.contains(' ') && matches!(s.kind.as_str(), "name" | "speaker" | "case")).map(|s| s.term.clone()).collect();
        let mut label_of = |s: &DeidSuggestion| -> String {
            match s.kind.as_str() {
                "email" => "[email]".to_string(),
                "web" => "[web address]".to_string(),
                "phone" => "[phone]".to_string(),
                "number" => "[ID number]".to_string(),
                kind => {
                    // A single word that is the first or last word of a longer one found here shares that one's label.
                    let key = if !s.term.contains(' ') {
                        multiword.iter().find(|m| m.split(' ').next() == Some(&s.term) || m.split(' ').next_back() == Some(&s.term)).cloned().unwrap_or_else(|| s.term.clone())
                    } else {
                        s.term.clone()
                    };
                    taken
                        .entry(key)
                        .or_insert_with(|| {
                            if kind == "speaker" {
                                let l = format!("Participant {next_participant}");
                                next_participant += 1;
                                l
                            } else {
                                let l = format!("Person {next_person}");
                                next_person += 1;
                                l
                            }
                        })
                        .clone()
                }
            }
        };
        // Longer terms first so that the label is made for the full name and the short ones find it.
        let mut order: Vec<usize> = (0..out.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(out[i].term.chars().count()));
        let mut labels = vec![String::new(); out.len()];
        for i in order {
            labels[i] = label_of(&out[i]);
        }
        for (s, l) in out.iter_mut().zip(labels) {
            s.replacement = l;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrases(text: &str) -> BTreeMap<String, usize> {
        let mut m = HashMap::new();
        capitalized_phrases(text, &mut m);
        m.into_iter().collect()
    }

    #[test]
    fn a_capital_at_the_start_of_a_sentence_is_not_a_name() {
        let p = phrases("Yesterday I went home. Then Bob called. We met Bob and Alice Smith at the Station.");
        assert_eq!(p.get("Bob"), Some(&2), "Bob follows a word that is not a name, once after Then and once after met: {p:?}");
        assert_eq!(p.get("Alice Smith"), Some(&1));
        assert_eq!(p.get("Station"), Some(&1));
        assert!(!p.contains_key("Yesterday") && !p.contains_key("Then") && !p.contains_key("We"), "{p:?}");
    }

    #[test]
    fn a_run_of_names_at_the_start_of_a_sentence_still_counts() {
        let p = phrases("Sarah Jones told me. She left.");
        assert_eq!(p.get("Sarah Jones"), Some(&1), "{p:?}");
    }

    #[test]
    fn words_in_capitals_and_contractions_are_not_names() {
        let p = phrases("I think the NASA person said I'm fine. I'll go, said OK.");
        assert!(p.is_empty(), "{p:?}");
    }

    #[test]
    fn generic_labels_and_codes_are_not_names() {
        assert!(is_generic_label("Speaker 2") && is_generic_label("Interviewer") && is_generic_label("Participant 12"));
        assert!(!is_generic_label("Sarah"));
        assert!(looks_like_a_code("P07") && looks_like_a_code("S-3"));
        assert!(!looks_like_a_code("Sarah") && !looks_like_a_code("Participant 3"));
    }
}
