//! The assistant server's rules (decision 203): what an AI assistant that a researcher has connected over the Model Context Protocol may read
//! of a project, and what it may propose. The protocol itself is in the command-line program (`prismqda mcp`); the rules are here, so they can
//! be tested without a protocol and cannot be skipped by one.
//!
//! * The assistant reaches the project through a pipe to a program the researcher's own assistant application starts. PrismQDA opens no
//!   port and makes no connection (decision 91 stands: it never connects to an AI service). Nothing runs until the researcher sets their
//!   application to start it.
//! * Documents marked do not share, codes marked do not share, and the words of units marked do not share are never given. Tools that count
//!   or search across documents leave out every document that has a unit marked, because a count would give those words away.
//! * The offline lock, the switch that turns AI off for a project, and the list of names to replace each close the route.
//! * There are two levels. *Read* gives the documents, the codebook, the codings, a search and the word counts. *Propose* adds suggested
//!   codings, which wait in the review queue like Auto Code's and an AI answer's, and become codings only when the person accepts them.
//!   An assistant never applies a coding itself.
//! * Memos and the audit trail are not given: they hold what the researcher thinks and decides, which is not what an assistant was asked to
//!   code.

use std::collections::HashSet;
use std::str::FromStr;

use rusqlite::params;
use serde_json::{json, Value};

use crate::event;
use crate::ops::sha256_hex;
use crate::proposal::NewProposal;
use crate::project::Project;
use crate::text_analytics::{ContextSpec, WordSpec};
use crate::{Error, Result};

/// The most characters of a document one read gives, and the most suggestions one call takes.
pub const MAX_READ_CHARS: usize = 20_000;
pub const MAX_PROPOSALS: usize = 50;
const MAX_CODINGS_LISTED: usize = 300;
const EXCERPT_CHARS: usize = 160;

/// What an assistant may do in one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantPermission {
    Read,
    Propose,
}

impl FromStr for AssistantPermission {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "read" => Ok(AssistantPermission::Read),
            "propose" => Ok(AssistantPermission::Propose),
            other => Err(Error::Invalid(format!("the permission is read or propose, not '{other}'"))),
        }
    }
}

impl AssistantPermission {
    pub fn as_str(self) -> &'static str {
        match self {
            AssistantPermission::Read => "read",
            AssistantPermission::Propose => "propose",
        }
    }
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str], writes: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
        "annotations": { "readOnlyHint": !writes, "destructiveHint": false, "openWorldHint": false },
    })
}

/// The tools an assistant at this level is offered, as the protocol describes them.
pub fn assistant_tools(permission: AssistantPermission) -> Vec<Value> {
    let mut tools = vec![
        tool("project_overview", "What this project is: its name, method, research questions, how many documents and codes it has, and what is withheld from you.", json!({}), &[], false),
        tool("list_documents", "The documents you may read: id, name, kind, length in characters, and number of passages.", json!({}), &[], false),
        tool(
            "read_document",
            "The words of a document, from a character offset, at most 20000 characters at a time. Words of passages marked do not share come back as block characters of the same length, so offsets stay true. Say next_start to go on.",
            json!({ "document_id": { "type": "integer" }, "start": { "type": "integer", "description": "First character to read (default 0)." }, "end": { "type": "integer", "description": "Where to stop (default: 20000 characters on)." } }),
            &["document_id"],
            false,
        ),
        tool("list_codes", "The codebook: each active code with its id, parent, definition, when to use it, and when not to.", json!({}), &[], false),
        tool(
            "list_codings",
            "The codings people made in a document: where, with which code, by whom, and the words coded.",
            json!({ "document_id": { "type": "integer" } }),
            &["document_id"],
            false,
        ),
        tool(
            "find_text",
            "Where a word or phrase is used, with the words around it, across the documents that can be searched.",
            json!({ "query": { "type": "string" }, "words_around": { "type": "integer" }, "limit": { "type": "integer" } }),
            &["query"],
            false,
        ),
        tool(
            "word_frequencies",
            "The words the documents use most, across the documents that can be counted.",
            json!({ "limit": { "type": "integer" }, "leave_out_common": { "type": "boolean" }, "stem": { "type": "boolean" } }),
            &[],
            false,
        ),
    ];
    if permission == AssistantPermission::Propose {
        tools.push(tool(
            "propose_codings",
            "Suggest codings. Each item names a document, words quoted from it exactly (capitals and spelling as written), and the name of an active code. The suggestions wait for the researcher in the review queue; nothing is coded until they accept it. At most 50 items a call.",
            json!({ "items": { "type": "array", "items": { "type": "object", "properties": { "document_id": { "type": "integer" }, "quote": { "type": "string" }, "code": { "type": "string" }, "reason": { "type": "string" } }, "required": ["document_id", "quote", "code"] } } }),
            &["items"],
            true,
        ));
    }
    tools
}

/// Where `quote` stands in `text` word for word (a run of white space in the quote matches any run in the text), as characters `(start, end)`.
fn locate(text: &[char], quote: &str) -> Option<(usize, usize)> {
    let q: Vec<char> = quote.split_whitespace().collect::<Vec<_>>().join(" ").chars().collect();
    if q.is_empty() {
        return None;
    }
    'outer: for p in 0..text.len() {
        if text[p] != q[0] {
            continue;
        }
        let (mut i, mut k) = (p, 0);
        while k < q.len() {
            if q[k] == ' ' {
                if i >= text.len() || !text[i].is_whitespace() {
                    continue 'outer;
                }
                while i < text.len() && text[i].is_whitespace() {
                    i += 1;
                }
            } else {
                if i >= text.len() || text[i] != q[k] {
                    continue 'outer;
                }
                i += 1;
            }
            k += 1;
        }
        return Some((p, i));
    }
    None
}

fn int(args: &Value, key: &str) -> Result<Option<i64>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_i64().map(Some).ok_or_else(|| Error::Invalid(format!("'{key}' must be a whole number"))),
    }
}

fn need_int(args: &Value, key: &str) -> Result<i64> {
    int(args, key)?.ok_or_else(|| Error::Invalid(format!("'{key}' is needed")))
}

impl Project {
    /// Refuses a connection the project does not allow: the offline lock, the AI switch turned off, or a list of names to replace that is
    /// applied to exports (an assistant would see the documents whole).
    pub fn assistant_check(&self) -> Result<()> {
        if self.offline_lock()? {
            return Err(Error::Invalid("the offline lock is on for this project, so no assistant can be connected to it".to_string()));
        }
        let mode: String = self.conn.query_row("SELECT ai_mode FROM project WHERE id = 1", [], |r| r.get(0))?;
        if mode != "packet" {
            return Err(Error::Invalid("AI is switched off for this project (Settings > AI Assistant), and that switch closes the assistant route too".to_string()));
        }
        if self.deid_state()?.exports {
            return Err(Error::Invalid("the list of names to replace is applied to exports, and an assistant would see the documents whole; turn that switch off, or replace the names in the project itself first".to_string()));
        }
        Ok(())
    }

    /// The characters of a document that lie in a passage marked do not share.
    fn withheld_ranges(&self, source_id: i64) -> Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare("SELECT start_offset, end_offset FROM unit WHERE source_id = ?1 AND do_not_share = 1 ORDER BY start_offset")?;
        let rows = stmt.query_map([source_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// The documents an assistant may read, each with whether it has a passage that is withheld.
    fn assistant_documents(&self) -> Result<Vec<(crate::SourceRow, bool)>> {
        let mut out = Vec::new();
        for s in self.list_sources()? {
            if s.do_not_share {
                continue;
            }
            let withheld = !self.withheld_ranges(s.id)?.is_empty();
            out.push((s, withheld));
        }
        Ok(out)
    }

    fn assistant_document(&self, id: i64) -> Result<crate::SourceRow> {
        let all = self.list_sources()?;
        let s = all.into_iter().find(|s| s.id == id).ok_or(Error::NotFound { what: "document", id })?;
        if s.do_not_share {
            return Err(Error::Invalid(format!("document {id} is marked do not share, so it is not given to an assistant")));
        }
        Ok(s)
    }

    /// Runs one tool for an assistant. Everything an assistant is given or can change passes through here.
    pub fn assistant_call(&mut self, tool: &str, args: &Value, permission: AssistantPermission, client: &str, coder_id: i64) -> Result<Value> {
        self.assistant_check()?;
        match tool {
            "project_overview" => self.assistant_overview(),
            "list_documents" => {
                let docs: Vec<Value> = self
                    .assistant_documents()?
                    .iter()
                    .map(|(s, withheld)| json!({ "document_id": s.id, "name": s.name, "kind": s.source_type, "characters": s.characters, "passages": s.units, "has_withheld_passages": withheld }))
                    .collect();
                Ok(json!({ "documents": docs }))
            }
            "read_document" => {
                let id = need_int(args, "document_id")?;
                let doc = self.assistant_document(id)?;
                let start = int(args, "start")?.unwrap_or(0).clamp(0, doc.characters);
                let end = int(args, "end")?.unwrap_or(start + MAX_READ_CHARS as i64).clamp(start, doc.characters).min(start + MAX_READ_CHARS as i64);
                let withheld = self.withheld_ranges(id)?;
                let chars: Vec<char> = self.source_text(id)?.chars().collect();
                let text: String = (start..end).map(|i| if withheld.iter().any(|(a, b)| *a <= i && i < *b) { '\u{2588}' } else { chars[i as usize] }).collect();
                Ok(json!({ "document_id": id, "name": doc.name, "start": start, "end": end, "total_characters": doc.characters, "text": text, "next_start": if end < doc.characters { json!(end) } else { Value::Null }, "has_withheld_passages": !withheld.is_empty() }))
            }
            "list_codes" => {
                let codes: Vec<Value> = self
                    .list_codes()?
                    .iter()
                    .filter(|c| c.status == "active" && !c.do_not_share)
                    .map(|c| json!({ "code_id": c.id, "name": c.name, "parent_id": c.parent_id, "definition": c.definition, "when_to_use": c.inclusion_criteria, "when_not_to_use": c.exclusion_criteria, "may_be_suggested": c.ai_suggestible }))
                    .collect();
                Ok(json!({ "codes": codes }))
            }
            "list_codings" => {
                let id = need_int(args, "document_id")?;
                let doc = self.assistant_document(id)?;
                let hidden: HashSet<i64> = self.list_codes()?.iter().filter(|c| c.do_not_share).map(|c| c.id).collect();
                let names: std::collections::HashMap<i64, String> = self.list_codes()?.into_iter().map(|c| (c.id, c.name)).collect();
                let coders: std::collections::HashMap<i64, String> = self.list_coders()?.into_iter().map(|c| (c.id, c.name)).collect();
                let withheld = self.withheld_ranges(id)?;
                let chars: Vec<char> = self.source_text(id)?.chars().collect();
                let mut listed = Vec::new();
                let mut more = false;
                for c in self.list_codings(id)? {
                    if c.layer != "coder" || hidden.contains(&c.code_id) {
                        continue;
                    }
                    if listed.len() >= MAX_CODINGS_LISTED {
                        more = true;
                        break;
                    }
                    let (a, b) = (c.start.max(0) as usize, (c.end.max(0) as usize).min(chars.len()));
                    let words: String = (a..b).take(EXCERPT_CHARS).map(|i| if withheld.iter().any(|(x, y)| *x <= i as i64 && (i as i64) < *y) { '\u{2588}' } else { chars[i] }).collect();
                    listed.push(json!({ "start": c.start, "end": c.end, "code": names.get(&c.code_id), "coder": coders.get(&c.coder_id), "words": words }));
                }
                Ok(json!({ "document_id": id, "name": doc.name, "codings": listed, "more_than_listed": more }))
            }
            "find_text" => {
                let query = args.get("query").and_then(Value::as_str).unwrap_or("").to_string();
                let (ids, left_out) = self.searchable_documents()?;
                let r = self.keywords_in_context(&ContextSpec { query, source_ids: Some(ids), case_id: None, words_around: int(args, "words_around")?.unwrap_or(6).clamp(0, 50) as usize, limit: int(args, "limit")?.unwrap_or(30).clamp(1, 100) as usize })?;
                Ok(json!({ "total": r.total, "documents": r.documents, "places": r.items, "left_out_documents": left_out, "note": if left_out > 0 { "Documents with passages marked do not share are not searched." } else { "" } }))
            }
            "word_frequencies" => {
                let (ids, left_out) = self.searchable_documents()?;
                let r = self.word_frequencies(&WordSpec {
                    source_ids: Some(ids),
                    limit: int(args, "limit")?.unwrap_or(50).clamp(1, 200) as usize,
                    leave_out_common: args.get("leave_out_common").and_then(Value::as_bool).unwrap_or(true),
                    stem: args.get("stem").and_then(Value::as_bool).unwrap_or(false),
                    ..Default::default()
                })?;
                Ok(json!({ "words": r.words, "counted_words": r.counted_words, "distinct_words": r.distinct_words, "documents": r.documents, "left_out_documents": left_out }))
            }
            "propose_codings" => {
                if permission != AssistantPermission::Propose {
                    return Err(Error::Invalid("this session may read only; the researcher connected it without the permission to propose".to_string()));
                }
                self.assistant_propose(args, client, coder_id)
            }
            other => Err(Error::Invalid(format!("there is no tool named '{other}'"))),
        }
    }

    /// The documents a search or a count may read: shared documents with no passage marked do not share (a count would give those words away).
    fn searchable_documents(&self) -> Result<(Vec<i64>, usize)> {
        let all = self.assistant_documents()?;
        let left_out = all.iter().filter(|(_, withheld)| *withheld).count();
        let ids: Vec<i64> = all.iter().filter(|(_, withheld)| !*withheld).map(|(s, _)| s.id).collect();
        // A search over no document would read all of them; an empty list is refused by the caller's own rules, so say nothing was found.
        if ids.is_empty() {
            return Ok((vec![-1], left_out));
        }
        Ok((ids, left_out))
    }

    fn assistant_overview(&self) -> Result<Value> {
        let (name, method, questions, mode): (String, String, String, String) = self
            .conn
            .query_row("SELECT name, method_template, research_questions, coding_mode FROM project WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        let docs = self.list_sources()?;
        let codes = self.list_codes()?;
        Ok(json!({
            "name": name,
            "method": method,
            "research_questions": questions,
            "coding_mode": mode,
            "documents": docs.iter().filter(|s| !s.do_not_share).count(),
            "documents_withheld": docs.iter().filter(|s| s.do_not_share).count(),
            "codes": codes.iter().filter(|c| c.status == "active" && !c.do_not_share).count(),
            "codes_withheld": codes.iter().filter(|c| c.do_not_share).count(),
            "note": "You are given documents, the codebook, codings and counts. You are not given memos or the history of the project, and nothing marked do not share.",
        }))
    }

    /// Checks each suggestion the assistant made and sends what passes to the review queue (decisions 93 and 203).
    fn assistant_propose(&mut self, args: &Value, client: &str, coder_id: i64) -> Result<Value> {
        self.ensure_writable()?;
        let items = args.get("items").and_then(Value::as_array).ok_or_else(|| Error::Invalid("'items' must be a list of suggestions".to_string()))?;
        if items.is_empty() {
            return Err(Error::Invalid("there is no suggestion in 'items'".to_string()));
        }
        if items.len() > MAX_PROPOSALS {
            return Err(Error::Invalid(format!("at most {MAX_PROPOSALS} suggestions can come in one call")));
        }
        let client: String = client.trim().chars().take(100).collect::<String>();
        let client = if client.is_empty() { "an assistant".to_string() } else { client };
        let whole_units = self.codes_whole_units()?;
        let codes: std::collections::HashMap<String, crate::CodeRow> = self.list_codes()?.into_iter().filter(|c| c.status == "active" && !c.do_not_share && c.ai_suggestible).map(|c| (c.name.trim().to_lowercase(), c)).collect();
        let mut texts: std::collections::HashMap<i64, Vec<char>> = std::collections::HashMap::new();
        let mut rejected: Vec<Value> = Vec::new();
        let mut proposals: Vec<NewProposal> = Vec::new();
        let mut seen: HashSet<(i64, i64, i64, i64)> = HashSet::new();
        for (i, item) in items.iter().enumerate() {
            let number = i + 1;
            let mut refuse = |why: String| rejected.push(json!({ "number": number, "why": why }));
            let (Some(doc_id), Some(quote), Some(code_name)) = (item["document_id"].as_i64(), item["quote"].as_str(), item["code"].as_str()) else {
                refuse("it needs a document_id, a quote and a code".to_string());
                continue;
            };
            let Ok(doc) = self.assistant_document(doc_id) else {
                refuse(format!("document {doc_id} is not one you may read"));
                continue;
            };
            let Some(code) = codes.get(&code_name.trim().to_lowercase()) else {
                refuse(format!("'{code_name}' is not an active code you may suggest"));
                continue;
            };
            if let std::collections::hash_map::Entry::Vacant(slot) = texts.entry(doc_id) {
                slot.insert(self.source_text(doc_id)?.chars().collect());
            }
            let chars = &texts[&doc_id];
            let Some((a, b)) = locate(chars, quote) else {
                refuse(format!("the quoted words are not in {}, word for word", doc.name));
                continue;
            };
            let (mut start, mut end, mut unit_id) = (a as i64, b as i64, None);
            let units = self.list_units(doc_id)?;
            if let Some(u) = units.iter().find(|u| !u.excluded && u.start <= start && end <= u.end) {
                unit_id = Some(u.id);
                if whole_units {
                    (start, end) = (u.start, u.end);
                }
            } else if whole_units {
                refuse("in this project a coding is a whole passage, and the quoted words are not inside one".to_string());
                continue;
            }
            if self.withheld_ranges(doc_id)?.iter().any(|(x, y)| *x < end && *y > start) {
                refuse("the words are in a passage marked do not share".to_string());
                continue;
            }
            if !seen.insert((doc_id, start, end, code.id)) {
                refuse("the same suggestion is already in this call".to_string());
                continue;
            }
            let reason: String = item["reason"].as_str().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect();
            proposals.push(NewProposal {
                source_id: doc_id,
                start,
                end,
                code_id: code.id,
                coder_id,
                unit_id,
                rationale: if reason.is_empty() { format!("AI suggestion ({client}) of {}", code.name) } else { format!("AI suggestion ({client}): {reason}") },
            });
        }
        let dropped = self.drop_known(proposals)?;
        let mut batch: Option<String> = None;
        if !dropped.kept.is_empty() {
            let prompt = format!("Suggestions made by an assistant connected over the Model Context Protocol (client: {client}). The assistant was given only what the project allows an assistant to read.");
            let hash = sha256_hex(prompt.as_bytes());
            let settings = json!({ "permission": "propose", "tool": "propose_codings", "coder_id": coder_id, "client": client });
            self.conn.execute(
                "INSERT INTO ai_run (route, model, settings_json, prompt, prompt_hash, units_sent_json, raw_output, created_at) VALUES ('mcp', ?1, ?2, ?3, ?4, '[]', ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![client, settings.to_string(), prompt, hash, args.to_string()],
            )?;
            let run_id = self.conn.last_insert_rowid();
            batch = Some(self.create_ai_proposals(run_id, &format!("AI: {client}"), &dropped.kept)?.batch);
            let result = json!({ "read": items.len(), "suggested": dropped.kept.len(), "rejected": rejected, "already_coded": dropped.already_coded, "already_waiting": dropped.already_waiting, "batch": batch });
            let tx = self.conn.transaction()?;
            tx.execute("UPDATE ai_run SET parsed_result_json = ?1 WHERE id = ?2", params![result.to_string(), run_id])?;
            event::insert(&tx, &self.actor, &self.session, "ai.answer", Some(("ai_run", run_id)), None, Some(&json!({ "model": client, "read": items.len(), "suggested": dropped.kept.len(), "rejected": rejected.len() })))?;
            tx.commit()?;
        }
        Ok(json!({
            "read": items.len(),
            "suggested": dropped.kept.len(),
            "rejected": rejected,
            "already_coded": dropped.already_coded,
            "already_waiting": dropped.already_waiting,
            "batch": batch,
            "note": "Suggestions wait in the researcher's review queue (Auto Code > Review queue). Nothing is coded until they accept it.",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, SourceType, UnitKind};

    #[test]
    fn a_passage_marked_do_not_share_is_masked_and_leaves_its_document_out_of_searches_and_counts() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut o = OpenOptions::new("tester");
        o.backup_on_open = false;
        let mut p = Project::create(dir.path().join("w.pqda"), &NewProject::new("Withheld"), &o).unwrap();
        let (first, second) = ("Shared words about buses.", "Private words about Marta Quint.");
        let text = format!("{first}\n\n{second}");
        let (a, b) = (first.chars().count() as i64, second.chars().count() as i64);
        let units = vec![ImportedUnit::new(0, a, UnitKind::Paragraph), ImportedUnit::new(a + 2, a + 2 + b, UnitKind::Paragraph)];
        let batch = ImportBatch {
            sources: vec![ImportedSource {
                name: "Mixed".into(),
                source_type: SourceType::Document,
                folder: String::new(),
                text,
                runs: vec![],
                units,
                default_unit_kind: UnitKind::Paragraph,
                unit_attributes: vec![],
                original_file_hash: None,
                settings: serde_json::Value::Null,
                warnings: vec![],
            }],
            ..Default::default()
        };
        let source = p.apply_import(&batch).unwrap().source_ids[0];
        let code = p.add_code(&NewCode::new("Topic")).unwrap();
        let coder = p.add_coder(&NewCoder::human("Reader")).unwrap();
        let second_unit = p.list_units(source).unwrap()[1].clone();
        p.apply_coding(&NewCoding::new(source, second_unit.start, second_unit.end, code, coder)).unwrap();
        p.conn.execute("UPDATE unit SET do_not_share = 1 WHERE id = ?1", [second_unit.id]).unwrap();

        let mut call = |tool: &str, args: Value| p.assistant_call(tool, &args, AssistantPermission::Propose, "Test", coder).unwrap();
        let docs = call("list_documents", json!({}));
        assert_eq!(docs["documents"][0]["has_withheld_passages"], true);
        let page = call("read_document", json!({ "document_id": source }));
        let shown = page["text"].as_str().unwrap();
        assert!(shown.starts_with(first) && !shown.contains("Marta") && !shown.contains("Private"), "{shown}");
        assert_eq!(shown.chars().count(), (a + 2 + b) as usize, "the masked text is as long as the real text, so offsets stay true");
        assert_eq!(shown.chars().filter(|c| *c == '\u{2588}').count(), b as usize);

        let codings = call("list_codings", json!({ "document_id": source }));
        let words = codings["codings"][0]["words"].as_str().unwrap();
        assert!(words.chars().all(|c| c == '\u{2588}'), "the words of a coded passage that is withheld are masked: {words}");

        let found = call("find_text", json!({ "query": "buses" }));
        assert_eq!((found["total"].as_i64(), found["left_out_documents"].as_i64()), (Some(0), Some(1)), "a document with a withheld passage is not searched");
        let counted = call("word_frequencies", json!({}));
        assert_eq!(counted["words"].as_array().unwrap().len(), 0);
        assert_eq!(counted["left_out_documents"], 1);

        let out = call("propose_codings", json!({ "items": [{ "document_id": source, "quote": "Marta Quint", "code": "Topic" }, { "document_id": source, "quote": "buses", "code": "Topic" }] }));
        assert_eq!(out["suggested"], 1, "{out}");
        assert!(out["rejected"][0]["why"].as_str().unwrap().contains("do not share"), "{out}");
    }
}
