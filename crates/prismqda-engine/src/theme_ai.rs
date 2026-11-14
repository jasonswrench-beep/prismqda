//! Candidate themes suggested by an AI, through a prompt packet (decision 168): the same road as suggested codings (decisions 91 to 95), with the
//! same promises. PrismQDA calls no model. It builds text from the codebook (the name, definition, and number of uses of each code that may be
//! sent, and the names of the themes that already exist; no passage), the researcher takes it to a model of their own choice and pastes the
//! answer back, and a checker reads it before anything reaches the researcher: a code the packet did not name is dropped (it is not in the
//! codebook the model was shown), a code placed in two suggested themes keeps only its first, and a suggestion left with fewer than two codes
//! is refused. What passes waits as a suggestion; **nothing is made until the researcher accepts it**, and accepting makes a theme (a candidate)
//! with the codes under it and a memo that says where it came from. Each run keeps the exact packet, the answer, and the decisions in the AI log.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ai::{PacketCode, Rejected};
use crate::event;
use crate::ops::sha256_hex;
use crate::{Error, MemoKind, MemoScope, NewCode, NewMemo, Project, Result};

const MAX_DEFINITION_CHARS: usize = 400;
const MAX_THEME_NAME_CHARS: usize = 100;
/// A packet needs at least this many codes to be worth asking about.
pub const MIN_CODES: usize = 3;

/// The text to take to a chatbot, and the run that records it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemePacket {
    pub run_id: i64,
    /// Exactly the text to take to a chatbot.
    pub prompt: String,
    pub characters: usize,
    pub codes: Vec<PacketCode>,
    /// Codes that were kept out, because they are marked do not share, are not suggestible, or are themes.
    pub left_out: Vec<String>,
}

/// One suggested theme that passed the check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeSuggestion {
    /// Which suggestion of the run, counting from 1.
    pub index: usize,
    pub name: String,
    pub definition: String,
    pub code_ids: Vec<i64>,
    pub code_names: Vec<String>,
    /// What the checker dropped from it, one sentence each.
    pub problems: Vec<String>,
    /// `pending`, `accepted`, or `rejected`.
    pub status: String,
    /// The theme made, once it is accepted.
    pub theme_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeAnswerOutcome {
    /// How many suggestions the answer held.
    pub read: usize,
    pub suggestions: Vec<ThemeSuggestion>,
    pub rejected: Vec<Rejected>,
}

/// The suggestions of a run, as they stand.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeRun {
    pub run_id: i64,
    pub model: String,
    pub answered: bool,
    pub suggestions: Vec<ThemeSuggestion>,
    pub rejected: Vec<Rejected>,
}

fn cut(text: &str, max: usize) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= max {
        t
    } else {
        format!("{}…", t.chars().take(max).collect::<String>().trim_end())
    }
}

fn build_prompt(codes: &[(String, String, i64)], themes: &[String]) -> String {
    let mut out = String::new();
    out.push_str(
        "You are helping a researcher with a thematic analysis of qualitative data. Below is a codebook: each code has a name, sometimes a definition, and the number of \
         times it was applied. Group the codes into candidate themes. A theme is a pattern of meaning that several codes share. Use only the codes listed, exactly as \
         named; do not invent codes. Put each code in at most one theme, and leave a code out if it does not fit anywhere. Give each theme a short name and one sentence \
         that says what it means.\n\n",
    );
    out.push_str("CODEBOOK\n");
    for (name, description, uses) in codes {
        out.push_str(&format!("- {name}"));
        if !description.is_empty() {
            out.push_str(&format!(": {description}"));
        }
        out.push_str(&format!(" (applied {uses} time{})\n", if *uses == 1 { "" } else { "s" }));
    }
    if !themes.is_empty() {
        out.push_str("\nTHEMES THE RESEARCHER ALREADY HAS (do not suggest these again)\n");
        for t in themes {
            out.push_str(&format!("- {t}\n"));
        }
    }
    out.push_str(
        "\nANSWER\nReply with only a JSON list, one object for each theme you suggest, in exactly this form:\n\
         [{\"theme\": \"Name of the theme\", \"definition\": \"one sentence that says what the theme means\", \"codes\": [\"Name of a code\", \"Name of another code\"]}]\n\
         Copy each code name exactly as it is written in the codebook. A theme needs at least two codes. Reply with [] if no grouping seems right. Add nothing outside the list.\n",
    );
    out
}

fn answer_list(answer: &str) -> Result<Vec<Value>> {
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Err(Error::Invalid("paste the answer first".to_string()));
    }
    // The first opening bracket from which a JSON list that holds objects (or nothing) can be read is the answer; a model may put words or a fence round it.
    for (tried, (at, _)) in trimmed.match_indices('[').enumerate() {
        if tried > 60 {
            break;
        }
        let mut values = serde_json::Deserializer::from_str(&trimmed[at..]).into_iter::<Value>();
        if let Some(Ok(Value::Array(items))) = values.next() {
            if items.is_empty() || items.iter().any(Value::is_object) {
                return Ok(items);
            }
        }
    }
    if trimmed.contains('[') {
        Err(Error::Invalid("the list in the answer cannot be read as JSON; ask the model to reply with only the list".to_string()))
    } else {
        Err(Error::Invalid("the answer has no list in it: the packet asked for a JSON list such as [{\"theme\": \"...\", \"codes\": [\"...\"]}]".to_string()))
    }
}

impl Project {
    /// Builds a prompt packet that asks a model to group the codebook's codes into candidate themes. Nothing is sent, and the packet holds no passage.
    pub fn build_theme_packet(&mut self, coder_id: i64) -> Result<ThemePacket> {
        self.ensure_writable()?;
        self.refuse_if_offline("no prompt packet can be built for an AI service")?;
        if self.conn.query_row::<String, _, _>("SELECT ai_mode FROM project WHERE id = 1", [], |r| r.get(0))? == "off" {
            return Err(Error::Invalid("AI is off in this project, so no packet can be built; it can be turned back on in Settings".to_string()));
        }
        let scrubber = self.export_scrubber()?;
        let all = self.list_codes()?;
        let mut codes: Vec<PacketCode> = Vec::new();
        let mut described: Vec<(String, String, i64)> = Vec::new();
        let mut left_out: Vec<String> = Vec::new();
        for c in all.iter().filter(|c| c.status == "active") {
            if c.do_not_share || !c.ai_suggestible {
                // a flagged theme is not named either, and not listed among the themes that already exist
                left_out.push(c.name.clone());
                continue;
            }
            if c.kind == "theme" {
                continue;
            }
            let description = cut(&c.definition, MAX_DEFINITION_CHARS);
            let scrub = |s: &str| scrubber.as_ref().map_or_else(|| s.to_string(), |x| x.scrub(s).into_owned());
            codes.push(PacketCode { id: c.id, name: c.name.clone() });
            described.push((scrub(&c.name), scrub(&description), c.usage));
        }
        if codes.len() < MIN_CODES {
            return Err(Error::Invalid(format!("there are only {} codes that can be sent, and grouping them into themes needs at least {MIN_CODES}", codes.len())));
        }
        let existing: Vec<String> = all.iter().filter(|c| c.status == "active" && c.kind == "theme" && !c.do_not_share && c.ai_suggestible).map(|c| scrubber.as_ref().map_or_else(|| c.name.clone(), |x| x.scrub(&c.name).into_owned())).collect();
        let prompt = build_prompt(&described, &existing);
        let settings = json!({ "task": "suggest_themes", "coder_id": coder_id, "code_ids": codes.iter().map(|c| c.id).collect::<Vec<_>>(), "names_replaced": scrubber.is_some() });
        let hash = sha256_hex(prompt.as_bytes());
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO ai_run (route, model, settings_json, prompt, prompt_hash, units_sent_json, created_at)
             VALUES ('packet', '', ?1, ?2, ?3, '[]', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![settings.to_string(), prompt, hash],
        )?;
        let run_id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "ai.packet", Some(("ai_run", run_id)), None, Some(&json!({ "task": "suggest_themes", "passages": 0, "codes": codes.len(), "prompt_hash": hash })))?;
        tx.commit()?;
        Ok(ThemePacket { run_id, characters: prompt.chars().count(), prompt, codes, left_out })
    }

    /// Checks the answer pasted back for a theme packet and keeps what passes as suggestions that wait for the researcher. Nothing is made yet.
    pub fn answer_theme_packet(&mut self, run_id: i64, model: &str, answer: &str) -> Result<ThemeAnswerOutcome> {
        self.ensure_writable()?;
        let (route, raw, settings_json): (String, String, String) = self
            .conn
            .query_row("SELECT route, raw_output, settings_json FROM ai_run WHERE id = ?1", [run_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "AI run", id: run_id })?;
        let settings: Value = serde_json::from_str(&settings_json)?;
        if route != "packet" || settings["task"] != "suggest_themes" {
            return Err(Error::Invalid("this is not a packet that asked for themes".to_string()));
        }
        if !raw.is_empty() {
            return Err(Error::Invalid("this packet already has an answer; build a new packet to ask again".to_string()));
        }
        let model = model.trim();
        if model.is_empty() {
            return Err(Error::Invalid("say which model gave the answer (for example the chatbot's name and version); it is kept with the run".to_string()));
        }
        let model: String = model.chars().take(100).collect();
        let items = answer_list(answer)?;
        // While names were replaced the model saw the codes under their replaced names, so a code is known by both spellings.
        let scrubber = if settings["names_replaced"] == true { self.list_scrubber().ok() } else { None };
        let mut codes: HashMap<String, (i64, String)> = HashMap::new();
        for id in settings["code_ids"].as_array().into_iter().flatten().filter_map(Value::as_i64) {
            if let Ok(c) = self.get_code(id) {
                if c.status == "active" && c.kind != "theme" {
                    codes.insert(c.name.trim().to_lowercase(), (c.id, c.name.clone()));
                    if let Some(s) = &scrubber {
                        codes.entry(s.scrub(c.name.trim()).trim().to_lowercase()).or_insert((c.id, c.name.clone()));
                    }
                }
            }
        }
        let mut used: HashMap<i64, String> = HashMap::new();
        let mut suggestions: Vec<ThemeSuggestion> = Vec::new();
        let mut rejected: Vec<Rejected> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let number = i + 1;
            let name = item["theme"].as_str().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
            let reject = |rejected: &mut Vec<Rejected>, why: &str| rejected.push(Rejected { number, passage: String::new(), code: name.clone(), why: why.to_string() });
            if !item.is_object() {
                reject(&mut rejected, "this item is not an object");
                continue;
            }
            if name.is_empty() {
                reject(&mut rejected, "the suggestion has no theme name");
                continue;
            }
            if name.chars().count() > MAX_THEME_NAME_CHARS {
                reject(&mut rejected, "the theme name is longer than 100 characters");
                continue;
            }
            let Some(listed) = item["codes"].as_array() else {
                reject(&mut rejected, "the suggestion has no list of codes");
                continue;
            };
            let mut problems = Vec::new();
            let mut ids: Vec<i64> = Vec::new();
            let mut names: Vec<String> = Vec::new();
            for c in listed {
                let Some(raw_name) = c.as_str() else {
                    problems.push("a code in the list is not a name".to_string());
                    continue;
                };
                let key = raw_name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
                match codes.get(&key) {
                    None => problems.push(format!("the code \"{}\" is not in the codebook the model was shown, so it was dropped", cut(raw_name, 60))),
                    Some((id, real)) => {
                        if let Some(earlier) = used.get(id) {
                            problems.push(format!("the code \"{real}\" was already placed in \"{earlier}\", so it stays there"));
                        } else if ids.contains(id) {
                            // named twice in the same theme: counted once, without a complaint
                        } else {
                            ids.push(*id);
                            names.push(real.clone());
                        }
                    }
                }
            }
            if ids.len() < 2 {
                reject(&mut rejected, "fewer than two codes from the codebook are left in this theme");
                continue;
            }
            for id in &ids {
                used.insert(*id, name.clone());
            }
            suggestions.push(ThemeSuggestion {
                index: number,
                name,
                definition: cut(item["definition"].as_str().unwrap_or(""), MAX_DEFINITION_CHARS),
                code_ids: ids,
                code_names: names,
                problems,
                status: "pending".to_string(),
                theme_id: None,
            });
        }
        let result = json!({ "task": "suggest_themes", "read": items.len(), "suggested": suggestions.len(), "rejected": rejected, "suggestions": suggestions });
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE ai_run SET model = ?1, raw_output = ?2, parsed_result_json = ?3 WHERE id = ?4", params![model, answer, result.to_string(), run_id])?;
        event::insert(&tx, &self.actor, &self.session, "ai.answer", Some(("ai_run", run_id)), None, Some(&json!({ "task": "suggest_themes", "model": model, "read": items.len(), "suggested": suggestions.len(), "rejected": rejected.len() })))?;
        tx.commit()?;
        Ok(ThemeAnswerOutcome { read: items.len(), suggestions, rejected })
    }

    /// The suggestions of one run, as they stand now.
    pub fn theme_run(&self, run_id: i64) -> Result<ThemeRun> {
        let (route, model, raw, settings, parsed): (String, String, String, String, String) = self
            .conn
            .query_row("SELECT route, model, raw_output, settings_json, parsed_result_json FROM ai_run WHERE id = ?1", [run_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "AI run", id: run_id })?;
        let settings: Value = serde_json::from_str(&settings)?;
        if route != "packet" || settings["task"] != "suggest_themes" {
            return Err(Error::Invalid("this is not a packet that asked for themes".to_string()));
        }
        let parsed: Value = serde_json::from_str(&parsed).unwrap_or(Value::Null);
        let mut suggestions: Vec<ThemeSuggestion> = serde_json::from_value(parsed["suggestions"].clone()).unwrap_or_default();
        // A theme that was made from a suggestion and then taken back with Undo is not a theme any more, so its suggestion waits again.
        for s in suggestions.iter_mut().filter(|s| s.status == "accepted") {
            if s.theme_id.is_none_or(|id| !self.get_code(id).is_ok_and(|c| c.status == "active")) {
                s.status = "pending".to_string();
                s.theme_id = None;
            }
        }
        Ok(ThemeRun { run_id, model, answered: !raw.is_empty(), suggestions, rejected: serde_json::from_value(parsed["rejected"].clone()).unwrap_or_default() })
    }

    /// Accepts or rejects one suggestion. Accepting makes a theme (a candidate) with the suggestion's definition, puts its codes under it, and writes a
    /// memo on the theme that says where it came from. `name` replaces the suggested name (for a name that is already a code). Returns the run as it now stands.
    pub fn decide_theme_suggestion(&mut self, run_id: i64, index: usize, accept: bool, name: Option<&str>) -> Result<ThemeRun> {
        self.ensure_writable()?;
        let mut run = self.theme_run(run_id)?;
        let pos = run.suggestions.iter().position(|s| s.index == index).ok_or(Error::NotFound { what: "suggestion", id: index as i64 })?;
        if run.suggestions[pos].status != "pending" {
            return Err(Error::Invalid("this suggestion has already been decided".to_string()));
        }
        let mut theme_id = None;
        if accept {
            let s = run.suggestions[pos].clone();
            let chosen = name.map(|n| n.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|n| !n.is_empty()).unwrap_or_else(|| s.name.clone());
            let id = self.add_code(&NewCode {
                name: chosen.clone(),
                parent_id: None,
                theme: true,
                color: None,
                definition: s.definition.clone(),
                inclusion_criteria: String::new(),
                exclusion_criteria: String::new(),
                exclusive_group: String::new(),
                rated: false,
                rating_label: String::new(),
                rating_low: String::new(),
                rating_high: String::new(),
            })?;
            let members: std::collections::HashSet<i64> = s.code_ids.iter().copied().collect();
            for code in &s.code_ids {
                // A code that has gone since the answer was read (retired, merged) is left where it is.
                let Ok(c) = self.get_code(*code) else { continue };
                if c.status != "active" {
                    continue;
                }
                // A sub-code of a code that is also in the theme comes with its parent: moving it out of its parent would flatten the tree.
                let (mut up, mut guard, mut with_parent) = (c.parent_id, 0, false);
                while let Some(p) = up {
                    if members.contains(&p) {
                        with_parent = true;
                        break;
                    }
                    guard += 1;
                    if guard > 64 {
                        break;
                    }
                    up = self.get_code(p).ok().and_then(|x| x.parent_id);
                }
                if !with_parent {
                    self.move_code(*code, Some(id), None)?;
                }
            }
            let model = run.model.clone();
            self.add_memo(&NewMemo {
                kind: MemoKind::MethodDecision,
                scope: MemoScope::Code,
                target_id: Some(id),
                phase: None,
                title: "Where this theme came from".to_string(),
                body: format!("Suggested by an AI model ({model}) in AI run {run_id} from the names and definitions of the codes, and accepted by the researcher. The model suggested it as \"{}\" with the codes {}. It stands as a candidate until it is reviewed against the passages.", s.name, s.code_names.join(", ")),
            })?;
            theme_id = Some(id);
        }
        run.suggestions[pos].status = if accept { "accepted" } else { "rejected" }.to_string();
        run.suggestions[pos].theme_id = theme_id;
        let parsed: Value = self.conn.query_row("SELECT parsed_result_json FROM ai_run WHERE id = ?1", [run_id], |r| r.get::<_, String>(0)).map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))?;
        let mut parsed = parsed;
        parsed["suggestions"] = serde_json::to_value(&run.suggestions)?;
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE ai_run SET parsed_result_json = ?1 WHERE id = ?2", params![parsed.to_string(), run_id])?;
        // The answer's own event kind is used, which Undo leaves alone: a decision is made in a few steps (the theme, each move, the memo) that are undone one at a time.
        event::insert(&tx, &self.actor, &self.session, "ai.answer", Some(("ai_run", run_id)), None, Some(&json!({ "task": "suggest_themes", "decision": if accept { "accepted" } else { "rejected" }, "suggestion": index, "theme_id": theme_id })))?;
        tx.commit()?;
        Ok(run)
    }
}
