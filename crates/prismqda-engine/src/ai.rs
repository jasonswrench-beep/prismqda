//! AI through prompt packets (specification 19.1, the first route). The program never talks to a model. It builds a packet of
//! text (a codebook and some passages, with what to answer and in what form), the person takes it to any chatbot they choose and
//! pastes the answer back, and the answer is checked before anything of it reaches the review queue: a passage or a code that
//! was not in the packet is refused, and every quoted word must appear in its passage, word for word. What the model suggested
//! waits in the review queue as an AI suggestion until a person accepts it, and every run keeps what was sent, what came back,
//! and what became of it.
//!
//! Nothing here is sent anywhere: the packet is only text in the project and on the screen. What leaves the computer is what the
//! person pastes into another program.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::deid::{map_back, Edit};
use crate::event;
use crate::ops::sha256_hex;
use crate::project::Project;
use crate::proposal::NewProposal;
use crate::search::{CharCounter, SearchFilter};
use crate::{Error, Result};

/// The most passages one packet holds; chatbots have limits, and a person has to be able to read what they send.
pub const MAX_PASSAGES: usize = 200;
const DEFAULT_PASSAGES: usize = 40;
/// A passage longer than this is left out of a packet and counted: it is too much to read and to quote exactly.
const MAX_PASSAGE_CHARS: usize = 1500;
const MAX_DEFINITION_CHARS: usize = 400;
const MAX_REASON_CHARS: usize = 300;

fn yes() -> bool {
    true
}

/// What to put in a packet.
#[derive(Debug, Clone, Deserialize)]
pub struct PacketSpec {
    /// Only these documents (every document by default).
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    /// Only the documents of this case.
    #[serde(default)]
    pub case_id: Option<i64>,
    /// The codes to ask about; empty means every active code that may be suggested.
    #[serde(default)]
    pub code_ids: Vec<i64>,
    /// Leave out passages that the coder has already coded (the default).
    #[serde(default = "yes")]
    pub only_uncoded: bool,
    /// How many passages (at most 200; 40 by default).
    #[serde(default)]
    pub limit: Option<usize>,
    /// How many passages to pass over first, to go on from an earlier packet.
    #[serde(default)]
    pub skip: usize,
    /// Whose coding the suggestions are for; the person using the project by default.
    #[serde(default)]
    pub coder_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PacketCode {
    pub id: i64,
    pub name: String,
}

/// What was kept out of a packet, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct PacketLeftOut {
    /// Documents marked do not share.
    pub documents_not_shared: usize,
    /// Passages marked do not share.
    pub passages_not_shared: usize,
    /// Passages the coder has already coded.
    pub already_coded: usize,
    /// Passages longer than a packet holds.
    pub too_long: usize,
    /// Pictures. A packet carries words only; the description of a picture is for people who cannot see it, and a suggestion on it
    /// would code the description, not an area of the picture.
    pub pictures: usize,
    /// Codes that may not be sent (marked do not share, or not suggestible).
    pub codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Packet {
    /// The run that records this packet. Its answer is pasted back against this number.
    pub run_id: i64,
    /// Exactly the text to take to a chatbot.
    pub prompt: String,
    pub passages: usize,
    pub characters: usize,
    pub codes: Vec<PacketCode>,
    pub left_out: PacketLeftOut,
    /// How many more passages would have qualified beyond this packet.
    pub more_after: usize,
    /// Where the next packet starts (`skip`).
    pub next_skip: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejected {
    /// Which item of the answer, counting from 1.
    pub number: usize,
    pub passage: String,
    pub code: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnswerOutcome {
    /// How many suggestions the answer held.
    pub read: usize,
    /// How many passed the check and went to the review queue.
    pub suggested: usize,
    pub rejected: Vec<Rejected>,
    /// Passed the check but left out because this coder already has that code on those words.
    pub already_coded: usize,
    /// Passed the check but left out because the same suggestion already waits.
    pub already_waiting: usize,
    /// The review queue batch, when anything was suggested.
    pub batch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiRunRow {
    pub id: i64,
    pub route: String,
    /// What the person said the model was (they type it when they paste the answer).
    pub model: String,
    pub created_at: String,
    pub answered: bool,
    pub passages: usize,
    pub codes: usize,
    pub read: usize,
    pub suggested: usize,
    pub checker_rejected: usize,
    pub pending: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub prompt_hash: String,
    /// The review queue batch the run's suggestions went to, when any did.
    pub batch: Option<String>,
    /// What the packet asked for: `code_with_codebook` (suggested codings of passages) or `suggest_themes` (candidate themes from the codebook).
    pub task: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiRunDetail {
    #[serde(flatten)]
    pub run: AiRunRow,
    pub prompt: String,
    pub raw_output: String,
    pub checker_rejections: Vec<Rejected>,
}

struct Candidate {
    source_id: i64,
    unit_id: Option<i64>,
    start: i64,
    end: i64,
    text: String,
}

fn cut(text: &str, max: usize) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= max {
        t
    } else {
        format!("{}…", t.chars().take(max).collect::<String>().trim_end())
    }
}

/// A phrase that ends like a sentence, so definitions and criteria read one after another.
fn sentence(text: &str) -> String {
    let t = text.trim_end();
    if t.ends_with(['.', '!', '?', '…']) {
        t.to_string()
    } else {
        format!("{t}.")
    }
}

/// The text of a packet. Everything a model is told is here, so what a person reads is what is sent.
fn build_prompt(codes: &[(String, String)], passages: &[(String, &str)]) -> String {
    let mut out = String::new();
    out.push_str(
        "You are helping a researcher with qualitative coding. Below are a codebook and some passages of text. For each passage, decide which \
         codes from the codebook clearly apply. Use only the codes listed. Do not invent codes or passages. If no code clearly applies to a \
         passage, leave that passage out.\n\n",
    );
    out.push_str("CODEBOOK\n");
    for (name, description) in codes {
        out.push_str(&format!("- {name}"));
        if !description.is_empty() {
            out.push_str(&format!(": {description}"));
        }
        out.push('\n');
    }
    out.push_str("\nPASSAGES\n");
    for (label, text) in passages {
        out.push_str(&format!("[{label}] {}\n", text.split('\n').map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")));
    }
    out.push_str(
        "\nANSWER\nReply with only a JSON list, one object for each coding you suggest, in exactly this form:\n\
         [{\"passage\": \"P1\", \"code\": \"Name of a code\", \"quote\": \"the exact words from the passage that show the code\", \"reason\": \"one short sentence\"}]\n\
         Copy the quote from the passage word for word; do not change, shorten with dots, or correct it. The same passage may appear more than once with different codes. \
         Reply with [] if no code applies to any passage. Add nothing outside the list.\n",
    );
    out
}

/// How many of a theme packet's suggestions stand at `status`; none for any other packet.
fn theme_count(parsed: &Value, status: &str) -> usize {
    parsed["suggestions"].as_array().map_or(0, |a| a.iter().filter(|s| s["status"] == status).count())
}

/// Words and characters as a comparison reads them: runs of white space are one space, and typographic quotes are plain ones. Returns
/// the characters and, for each, where it was in the original text.
fn fold(text: &str) -> (Vec<char>, Vec<usize>) {
    let mut chars = Vec::new();
    let mut at = Vec::new();
    let mut gap = false;
    for (i, c) in text.chars().enumerate() {
        if c.is_whitespace() {
            if !gap && !chars.is_empty() {
                chars.push(' ');
                at.push(i);
            }
            gap = true;
            continue;
        }
        gap = false;
        chars.push(match c {
            '\u{2018}' | '\u{2019}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            other => other,
        });
        at.push(i);
    }
    while chars.last() == Some(&' ') {
        chars.pop();
        at.pop();
    }
    (chars, at)
}

/// Where a quote sits in a passage, word for word, as `(start, end)` in characters of the passage; `None` when it does not.
fn find_quote(passage: &str, quote: &str) -> Option<(usize, usize)> {
    let (hay, hay_at) = fold(passage);
    let (needle, _) = fold(quote);
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    let i = hay.windows(needle.len()).position(|w| w == needle.as_slice())?;
    Some((hay_at[i], hay_at[i + needle.len() - 1] + 1))
}

/// Takes the list out of an answer that may be wrapped in a code fence or in words.
fn answer_items(answer: &str) -> Result<Vec<Value>> {
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Err(Error::Invalid("paste the answer first".to_string()));
    }
    // A model may put words, a code fence, or a closing remark round the list, and the words may hold brackets of their own,
    // so each opening bracket is tried in turn and the first list that reads as JSON (and holds objects, or nothing) is the answer.
    let mut first_array: Option<Vec<Value>> = None;
    let mut tried = 0;
    for (at, _) in trimmed.match_indices('[') {
        tried += 1;
        if tried > 60 {
            break;
        }
        let mut values = serde_json::Deserializer::from_str(&trimmed[at..]).into_iter::<Value>();
        if let Some(Ok(Value::Array(items))) = values.next() {
            if items.is_empty() || items.iter().any(Value::is_object) {
                return Ok(items);
            }
            first_array.get_or_insert(items);
        }
    }
    if let Some(items) = first_array {
        return Ok(items);
    }
    if trimmed.contains('[') {
        Err(Error::Invalid("the list in the answer cannot be read as JSON; ask the model to reply with only the list".to_string()))
    } else {
        Err(Error::Invalid("the answer has no list in it: the packet asked for a JSON list such as [{\"passage\": \"P1\", ...}]".to_string()))
    }
}

impl Project {
    fn ai_mode_is_on(&self) -> Result<bool> {
        let mode: String = self.conn.query_row("SELECT ai_mode FROM project WHERE id = 1", [], |r| r.get(0))?;
        Ok(mode != "off")
    }

    /// True when the project codes whole units: a suggestion then covers a whole unit, never a part of it.
    pub(crate) fn codes_whole_units(&self) -> Result<bool> {
        let mode: String = self.conn.query_row("SELECT coding_mode FROM project WHERE id = 1", [], |r| r.get(0))?;
        Ok(mode == "unit")
    }

    /// The passages that could go in a packet, in document order, and what was kept out. With `exact`, a passage keeps the exact
    /// range of its unit (a project that codes whole units cannot take a part of one).
    fn packet_candidates(&self, filter: &SearchFilter, coder_id: i64, only_uncoded: bool, exact: bool) -> Result<(Vec<Candidate>, PacketLeftOut)> {
        let mut out = Vec::new();
        let mut left = PacketLeftOut::default();
        for (source_id, _) in self.scoped_sources(filter)? {
            let shared: i64 = self.conn.query_row("SELECT do_not_share FROM source WHERE id = ?1", [source_id], |r| r.get(0))?;
            if shared != 0 {
                left.documents_not_shared += 1;
                continue;
            }
            if self.get_image(source_id)?.is_some() {
                left.pictures += 1;
                continue;
            }
            let text: String = self.conn.query_row("SELECT text FROM source WHERE id = ?1", [source_id], |r| r.get(0))?;
            let mut not_shared: HashSet<i64> = HashSet::new();
            {
                let mut stmt = self.conn.prepare("SELECT id FROM unit WHERE source_id = ?1 AND do_not_share = 1")?;
                for row in stmt.query_map([source_id], |r| r.get::<_, i64>(0))? {
                    not_shared.insert(row?);
                }
            }
            // The coder's codings, by start with the furthest end reached so far, to tell whether a passage has any.
            let (starts, reach): (Vec<i64>, Vec<i64>) = {
                let mut stmt = self.conn.prepare("SELECT start_offset, end_offset FROM coding WHERE source_id = ?1 AND coder_id = ?2 AND layer = 'coder' ORDER BY start_offset")?;
                let mut starts = Vec::new();
                let mut reach: Vec<i64> = Vec::new();
                for row in stmt.query_map(params![source_id, coder_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                    let (s, e) = row?;
                    reach.push(reach.last().map_or(e, |m| (*m).max(e)));
                    starts.push(s);
                }
                (starts, reach)
            };
            let mut counter = CharCounter::new(&text);
            for (unit_id, b0, b1) in self.scopes_of(source_id, &text)? {
                if unit_id.is_some_and(|u| not_shared.contains(&u)) {
                    left.passages_not_shared += 1;
                    continue;
                }
                let whole = &text[b0..b1];
                let words = whole.trim();
                if words.is_empty() {
                    continue;
                }
                // The passage is the words without the blank space round them, so a suggestion that covers it all covers only words.
                let lead = if exact { 0 } else { whole.len() - whole.trim_start().len() };
                let (b0, b1) = if exact { (b0, b1) } else { (b0 + lead, b0 + lead + words.len()) };
                let (start, end) = (counter.at(b0), counter.at(b1));
                if only_uncoded {
                    let n = starts.partition_point(|s| *s < end);
                    if n > 0 && reach[n - 1] > start {
                        left.already_coded += 1;
                        continue;
                    }
                }
                if words.chars().count() > MAX_PASSAGE_CHARS {
                    left.too_long += 1;
                    continue;
                }
                out.push(Candidate { source_id, unit_id, start, end, text: words.to_string() });
            }
        }
        Ok((out, left))
    }

    /// Builds a prompt packet and records the run. The packet is text only: nothing is sent.
    pub fn build_packet(&mut self, spec: &PacketSpec, coder_id: i64) -> Result<Packet> {
        self.ensure_writable()?;
        self.refuse_if_offline("no prompt packet can be built for an AI service")?;
        if !self.ai_mode_is_on()? {
            return Err(Error::Invalid("AI is off in this project, so no packet can be built; it can be turned back on in Settings".to_string()));
        }
        let limit = spec.limit.unwrap_or(DEFAULT_PASSAGES).clamp(1, MAX_PASSAGES);
        // While the list of names is applied to what leaves the project, the passages are sent with the replacements in them.
        let scrubber = self.export_scrubber()?;
        let filter = SearchFilter { source_ids: spec.source_ids.clone(), case_id: spec.case_id, ..Default::default() };

        // The codes: those asked for (or every active one), minus any that may not be sent.
        let all = self.list_codes()?;
        let wanted: Vec<&crate::codebook::CodeRow> = if spec.code_ids.is_empty() {
            all.iter().filter(|c| c.status == "active").collect()
        } else {
            let mut picked = Vec::new();
            for id in &spec.code_ids {
                let c = all.iter().find(|c| c.id == *id).ok_or(Error::NotFound { what: "code", id: *id })?;
                if c.status != "active" {
                    return Err(Error::Invalid(format!("the code {} is {}, so it cannot be in a packet", c.name, c.status)));
                }
                picked.push(c);
            }
            picked
        };
        let mut left = PacketLeftOut::default();
        let mut codes: Vec<PacketCode> = Vec::new();
        let mut described: Vec<(String, String)> = Vec::new();
        for c in wanted {
            if c.do_not_share || !c.ai_suggestible {
                left.codes.push(c.name.clone());
                continue;
            }
            let mut parts = Vec::new();
            if !c.definition.trim().is_empty() {
                parts.push(sentence(&cut(&c.definition, MAX_DEFINITION_CHARS)));
            }
            if !c.inclusion_criteria.trim().is_empty() {
                parts.push(sentence(&format!("Use it when: {}", cut(&c.inclusion_criteria, MAX_DEFINITION_CHARS))));
            }
            if !c.exclusion_criteria.trim().is_empty() {
                parts.push(sentence(&format!("Do not use it when: {}", cut(&c.exclusion_criteria, MAX_DEFINITION_CHARS))));
            }
            codes.push(PacketCode { id: c.id, name: c.name.clone() });
            let description = parts.join(" ");
            described.push((c.name.clone(), scrubber.as_ref().map_or(description.clone(), |s| s.scrub(&description).into_owned())));
        }
        if codes.is_empty() {
            return Err(Error::Invalid("there is no code that can be sent: every code chosen is marked do not share or not suggestible".to_string()));
        }

        let exact = self.codes_whole_units()?;
        let (candidates, mut candidate_left) = self.packet_candidates(&filter, coder_id, spec.only_uncoded, exact)?;
        candidate_left.codes = left.codes;
        let total = candidates.len();
        if spec.skip >= total && total > 0 {
            return Err(Error::Invalid(format!("there are only {total} passages to send, so there is nothing after the first {}", spec.skip)));
        }
        let chosen: Vec<&Candidate> = candidates.iter().skip(spec.skip).take(limit).collect();
        if chosen.is_empty() {
            return Err(Error::Invalid("there is no passage to send: every passage is already coded, marked do not share, or too long".to_string()));
        }
        // What the model is shown for each passage, and (when names were replaced) where each replacement went, so that a quote it
        // sends back can be found in the real text.
        let shown: Vec<(String, Vec<Edit>)> = chosen
            .iter()
            .map(|c| match &scrubber {
                Some(s) => {
                    let applied = s.apply(c.text.trim());
                    (applied.text, applied.edits)
                }
                None => (c.text.trim().to_string(), Vec::new()),
            })
            .collect();
        let labelled: Vec<(String, &str)> = shown.iter().enumerate().map(|(i, (text, _))| (format!("P{}", i + 1), text.as_str())).collect();
        let prompt = build_prompt(&described, &labelled);
        let sent: Vec<Value> = chosen
            .iter()
            .zip(&shown)
            .enumerate()
            .map(|(i, (c, (text, edits)))| {
                let mut v = json!({ "ref": format!("P{}", i + 1), "source_id": c.source_id, "unit_id": c.unit_id, "start": c.start, "end": c.end });
                if scrubber.is_some() {
                    v["sent"] = json!(text);
                    v["edits"] = json!(edits.iter().map(|e| [e.old_start, e.old_end, e.new_start, e.new_end]).collect::<Vec<_>>());
                }
                v
            })
            .collect();
        let settings = json!({
            "task": "code_with_codebook", "coder_id": coder_id, "code_ids": codes.iter().map(|c| c.id).collect::<Vec<_>>(),
            "only_uncoded": spec.only_uncoded, "limit": limit, "skip": spec.skip, "names_replaced": scrubber.is_some(),
        });
        let hash = sha256_hex(prompt.as_bytes());
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO ai_run (route, model, settings_json, prompt, prompt_hash, units_sent_json, created_at)
             VALUES ('packet', '', ?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![settings.to_string(), prompt, hash, Value::Array(sent).to_string()],
        )?;
        let run_id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "ai.packet", Some(("ai_run", run_id)), None, Some(&json!({ "passages": chosen.len(), "codes": codes.len(), "prompt_hash": hash })))?;
        tx.commit()?;
        Ok(Packet {
            run_id,
            characters: prompt.chars().count(),
            passages: chosen.len(),
            codes,
            left_out: candidate_left,
            more_after: total.saturating_sub(spec.skip + chosen.len()),
            next_skip: spec.skip + chosen.len(),
            prompt,
        })
    }

    /// Checks the answer pasted back for a packet and sends what passes to the review queue as AI suggestions.
    pub fn answer_packet(&mut self, run_id: i64, model: &str, answer: &str) -> Result<AnswerOutcome> {
        self.ensure_writable()?;
        let (route, raw, settings_json, units_json): (String, String, String, String) = self
            .conn
            .query_row("SELECT route, raw_output, settings_json, units_sent_json FROM ai_run WHERE id = ?1", [run_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "AI run", id: run_id })?;
        if route != "packet" {
            return Err(Error::Invalid("only a prompt packet takes an answer pasted back".to_string()));
        }
        if serde_json::from_str::<Value>(&settings_json).is_ok_and(|s| s["task"] == "suggest_themes") {
            return Err(Error::Invalid("this packet asked for themes; paste its answer on the Themes page".to_string()));
        }
        if !raw.is_empty() {
            return Err(Error::Invalid("this packet already has an answer; build a new packet to ask again".to_string()));
        }
        let model = model.trim();
        if model.is_empty() {
            return Err(Error::Invalid("say which model gave the answer (for example the chatbot's name and version); it is kept with the run".to_string()));
        }
        let model: String = model.chars().take(100).collect();
        let items = answer_items(answer)?;
        let whole_units = self.codes_whole_units()?;
        let settings: Value = serde_json::from_str(&settings_json)?;
        let coder_id = settings["coder_id"].as_i64().ok_or_else(|| Error::Invalid("this packet has no coder".to_string()))?;
        let code_ids: Vec<i64> = settings["code_ids"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
        let mut codes: HashMap<String, (i64, String)> = HashMap::new();
        for id in &code_ids {
            if let Ok(c) = self.get_code(*id) {
                if c.status == "active" {
                    codes.insert(c.name.trim().to_lowercase(), (c.id, c.name.clone()));
                }
            }
        }
        let units: Value = serde_json::from_str(&units_json)?;
        let mut passages: HashMap<String, (i64, Option<i64>, i64, i64)> = HashMap::new();
        // For a packet whose names were replaced: the words the model saw, and where each replacement went.
        let mut shown: HashMap<String, (String, Vec<Edit>)> = HashMap::new();
        for u in units.as_array().into_iter().flatten() {
            if let (Some(r), Some(s), Some(a), Some(b)) = (u["ref"].as_str(), u["source_id"].as_i64(), u["start"].as_i64(), u["end"].as_i64()) {
                passages.insert(r.to_uppercase(), (s, u["unit_id"].as_i64(), a, b));
                if let Some(text) = u["sent"].as_str() {
                    let edits = u["edits"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|e| {
                            let n: Vec<usize> = e.as_array()?.iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect();
                            (n.len() == 4).then(|| Edit { old_start: n[0], old_end: n[1], new_start: n[2], new_end: n[3], rule: 0 })
                        })
                        .collect();
                    shown.insert(r.to_uppercase(), (text.to_string(), edits));
                }
            }
        }

        let mut rejected: Vec<Rejected> = Vec::new();
        let mut proposals: Vec<NewProposal> = Vec::new();
        let mut seen: HashSet<(i64, i64, i64, i64)> = HashSet::new();
        let mut texts: HashMap<i64, String> = HashMap::new();
        for (i, item) in items.iter().enumerate() {
            let number = i + 1;
            let shown_passage = item["passage"].as_str().unwrap_or("").trim().to_string();
            let shown_code = item["code"].as_str().unwrap_or("").trim().to_string();
            let mut refuse = |why: String| rejected.push(Rejected { number, passage: shown_passage.clone(), code: shown_code.clone(), why });
            if !item.is_object() || shown_passage.is_empty() || shown_code.is_empty() {
                refuse("it is not an object with a passage and a code".to_string());
                continue;
            }
            let label = shown_passage.trim_matches(|c| c == '[' || c == ']').trim().to_uppercase();
            let Some(&(source_id, unit_id, start, end)) = passages.get(&label) else {
                refuse(format!("there is no passage {shown_passage} in the packet"));
                continue;
            };
            let Some((code_id, code_name)) = codes.get(&shown_code.to_lowercase()).cloned() else {
                refuse(format!("the code {shown_code} is not one of the active codes that were sent"));
                continue;
            };
            // The passage's own words, to check the quote against and to place it.
            let source_text = match texts.get(&source_id) {
                Some(t) => t.clone(),
                None => {
                    let t: String = self.conn.query_row("SELECT text FROM source WHERE id = ?1", [source_id], |r| r.get(0))?;
                    texts.insert(source_id, t.clone());
                    t
                }
            };
            let mut counter = CharCounter::new(&source_text);
            let (b0, b1) = (counter.byte_of(start), counter.byte_of(end));
            let passage_text = &source_text[b0..b1.max(b0)];
            let quote = item["quote"].as_str().unwrap_or("").trim();
            let (s, e) = if quote.is_empty() {
                (start, end)
            } else {
                // A packet with names replaced is searched as the model saw it, and the place found is mapped back to the real words.
                let found = match shown.get(&label) {
                    Some((text, edits)) => {
                        let lead = passage_text.chars().take_while(|c| c.is_whitespace()).count();
                        find_quote(text, quote).map(|(a, b)| {
                            let (oa, ob) = map_back(edits, a, b);
                            (oa + lead, ob + lead)
                        })
                    }
                    None => find_quote(passage_text, quote),
                };
                match found {
                    // A project that codes whole units takes the whole unit; the quote still had to be there, word for word.
                    Some(_) if whole_units => (start, end),
                    Some((a, b)) => (start + a as i64, start + b as i64),
                    None => {
                        refuse(format!("the quoted words are not in passage {label}, word for word"));
                        continue;
                    }
                }
            };
            if !seen.insert((source_id, s, e, code_id)) {
                refuse("the same suggestion is already in the answer".to_string());
                continue;
            }
            let reason = cut(item["reason"].as_str().unwrap_or(""), MAX_REASON_CHARS);
            proposals.push(NewProposal {
                source_id,
                start: s,
                end: e,
                code_id,
                coder_id,
                unit_id,
                rationale: if reason.is_empty() { format!("AI suggestion ({model}) of {code_name}") } else { format!("AI suggestion ({model}): {reason}") },
            });
        }

        let dropped = self.drop_known(proposals)?;
        let batch = if dropped.kept.is_empty() { None } else { Some(self.create_ai_proposals(run_id, &format!("AI: {model}"), &dropped.kept)?.batch) };
        let outcome = AnswerOutcome {
            read: items.len(),
            suggested: dropped.kept.len(),
            rejected: rejected.clone(),
            already_coded: dropped.already_coded,
            already_waiting: dropped.already_waiting,
            batch: batch.clone(),
        };
        let result = json!({
            "read": outcome.read, "suggested": outcome.suggested, "rejected": rejected,
            "already_coded": outcome.already_coded, "already_waiting": outcome.already_waiting, "batch": batch,
        });
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE ai_run SET model = ?1, raw_output = ?2, parsed_result_json = ?3 WHERE id = ?4",
            params![model, answer, result.to_string(), run_id],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "ai.answer",
            Some(("ai_run", run_id)),
            None,
            Some(&json!({ "model": model, "read": outcome.read, "suggested": outcome.suggested, "rejected": outcome.rejected.len() })),
        )?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Every AI run, newest first, with what became of its suggestions.
    pub fn ai_runs(&self) -> Result<Vec<AiRunRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.route, r.model, r.created_at, r.raw_output <> '', r.settings_json, r.units_sent_json, r.parsed_result_json, r.prompt_hash,
                    (SELECT COUNT(*) FROM proposal p WHERE p.ai_run_id = r.id AND p.status = 'pending'),
                    (SELECT COUNT(*) FROM proposal p WHERE p.ai_run_id = r.id AND p.status = 'accepted'),
                    (SELECT COUNT(*) FROM proposal p WHERE p.ai_run_id = r.id AND p.status = 'rejected')
             FROM ai_run r ORDER BY r.id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            let (settings, units, parsed): (String, String, String) = (r.get(5)?, r.get(6)?, r.get(7)?);
            let settings: Value = serde_json::from_str(&settings).unwrap_or(Value::Null);
            let units: Value = serde_json::from_str(&units).unwrap_or(Value::Null);
            let parsed: Value = serde_json::from_str(&parsed).unwrap_or(Value::Null);
            Ok(AiRunRow {
                id: r.get(0)?,
                route: r.get(1)?,
                model: r.get(2)?,
                created_at: r.get(3)?,
                answered: r.get::<_, i64>(4)? != 0,
                passages: units.as_array().map_or(0, Vec::len),
                codes: settings["code_ids"].as_array().map_or(0, Vec::len),
                read: parsed["read"].as_u64().unwrap_or(0) as usize,
                suggested: parsed["suggested"].as_u64().unwrap_or(0) as usize,
                checker_rejected: parsed["rejected"].as_array().map_or(0, Vec::len),
                prompt_hash: r.get(8)?,
                batch: parsed["batch"].as_str().map(str::to_string),
                // A packet that asked for themes keeps its suggestions in the run itself, not in the review queue.
                pending: r.get::<_, i64>(9)? as usize + theme_count(&parsed, "pending"),
                accepted: r.get::<_, i64>(10)? as usize + theme_count(&parsed, "accepted"),
                rejected: r.get::<_, i64>(11)? as usize + theme_count(&parsed, "rejected"),
                task: settings["task"].as_str().unwrap_or("code_with_codebook").to_string(),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// One run with the exact packet, the raw answer, and what the checker refused.
    pub fn ai_run(&self, id: i64) -> Result<AiRunDetail> {
        let run = self.ai_runs()?.into_iter().find(|r| r.id == id).ok_or(Error::NotFound { what: "AI run", id })?;
        let (prompt, raw_output, parsed): (String, String, String) = self
            .conn
            .query_row("SELECT prompt, raw_output, parsed_result_json FROM ai_run WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let parsed: Value = serde_json::from_str(&parsed).unwrap_or(Value::Null);
        let checker_rejections: Vec<Rejected> = serde_json::from_value(parsed["rejected"].clone()).unwrap_or_default();
        Ok(AiRunDetail { run, prompt, raw_output, checker_rejections })
    }

    /// A paragraph for a methods section, drawn from the records of every run.
    pub fn ai_disclosure(&self) -> Result<String> {
        let runs = self.ai_runs()?;
        if runs.is_empty() {
            return Ok("No AI was used in this project.".to_string());
        }
        let answered: Vec<&AiRunRow> = runs.iter().filter(|r| r.answered && r.task != "suggest_themes").collect();
        let theme_runs: Vec<&AiRunRow> = runs.iter().filter(|r| r.task == "suggest_themes").collect();
        let mut models: Vec<String> = answered.iter().map(|r| r.model.clone()).collect();
        models.sort();
        models.dedup();
        let sum = |f: fn(&AiRunRow) -> usize| answered.iter().map(|r| f(r)).sum::<usize>();
        let first = runs.last().map_or("", |r| r.created_at.get(..10).unwrap_or(""));
        let last = runs.first().map_or("", |r| r.created_at.get(..10).unwrap_or(""));
        let when = if first == last { format!("on {first}") } else { format!("between {first} and {last}") };
        Ok(format!(
            "AI was used in this project only through prompt packets: PrismQDA built each packet of passages and codes, and the researcher took it to a model of their own choice and pasted the answer back; the program sent nothing itself. {} packet{} {} built {when}; {} {} answered{}. \
             The answers held {} suggested codings, of which {} passed a check that every passage and code was in the packet and every quoted word was in its passage. The researcher accepted {} and rejected {}; {} still wait. \
             A coding made from an accepted suggestion keeps the origin \"AI suggestion accepted\" in the project, and each run keeps the exact packet, the model's answer, and the researcher's decisions.{}{}",
            runs.len(),
            if runs.len() == 1 { "" } else { "s" },
            if runs.len() == 1 { "was" } else { "were" },
            answered.len(),
            if answered.len() == 1 { "was" } else { "were" },
            if models.is_empty() { String::new() } else { format!(" (by {})", models.join(", ")) },
            sum(|r| r.read),
            sum(|r| r.suggested),
            sum(|r| r.accepted),
            sum(|r| r.rejected),
            sum(|r| r.pending),
            if self.packets_with_names_replaced()? > 0 {
                format!(" In {} of the packets, the names and other identifiers on the researcher's list were replaced with labels before the packet was taken to the model.", self.packets_with_names_replaced()?)
            } else {
                String::new()
            },
            if theme_runs.is_empty() {
                String::new()
            } else {
                format!(
                    " {} of the packets asked for candidate themes: each held the names, definitions, and number of uses of the codes and no passage of any document, and the model's grouping was a suggestion that the researcher accepted or rejected one theme at a time ({} accepted, {} rejected).",
                    theme_runs.len(),
                    theme_runs.iter().map(|r| r.accepted).sum::<usize>(),
                    theme_runs.iter().map(|r| r.rejected).sum::<usize>()
                )
            },
        ))
    }

    /// How many packets were built while the list of names was applied.
    fn packets_with_names_replaced(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM ai_run WHERE route = 'packet' AND json_extract(settings_json, '$.names_replaced') = 1", [], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quote_is_found_word_for_word_through_spacing_and_typographic_quotes() {
        let passage = "I didn\u{2019}t feel  welcome\nat all, honestly.";
        let (a, b) = find_quote(passage, "didn't feel welcome at all").unwrap();
        assert_eq!(passage.chars().skip(a).take(b - a).collect::<String>(), "didn\u{2019}t feel  welcome\nat all");
        assert!(find_quote(passage, "felt welcome").is_none(), "a changed word is not the passage");
        assert!(find_quote(passage, "DIDN'T").is_none(), "capitals count");
        assert!(find_quote(passage, "").is_none());
        assert!(find_quote("short", "a much longer quote than the passage").is_none());
    }

    #[test]
    fn the_list_is_taken_out_of_an_answer_with_words_or_a_fence_round_it() {
        let items = answer_items("Sure! Here you go:\n```json\n[{\"passage\": \"P1\", \"code\": \"Cost\"}]\n```\nHope that helps.").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(answer_items("[]").unwrap().len(), 0);
        for bad in ["", "no list here", "[{\"passage\": ", "{\"passage\": \"P1\"}"] {
            assert!(answer_items(bad).is_err(), "{bad:?}");
        }
        // Brackets in the model's own words, or after the list, do not hide it.
        let wordy = answer_items("Here [is] the list [P1 first]: [{\"passage\": \"P1\", \"code\": \"Cost\"}] and that is all [ok].").unwrap();
        assert_eq!(wordy.len(), 1);
        assert_eq!(wordy[0]["code"], "Cost");
    }

    #[test]
    fn the_packet_says_everything_a_model_is_told_and_nothing_else() {
        let codes = vec![("Cost".to_string(), "Talk about money. Use it when: prices come up.".to_string()), ("Access".to_string(), String::new())];
        let text = build_prompt(&codes, &[("P1".to_string(), "Fares are high.\n  Really high."), ("P2".to_string(), "The stop is far.")]);
        assert!(text.contains("- Cost: Talk about money. Use it when: prices come up.\n- Access\n"));
        assert!(text.contains("[P1] Fares are high. Really high.\n[P2] The stop is far.\n"), "a passage is one line");
        assert!(text.contains("Use only the codes listed") && text.contains("word for word") && text.contains("Reply with []"));
    }
}
