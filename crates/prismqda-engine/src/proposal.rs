//! The review queue (specification 12.3 and 19.3): suggestions made by autocoding, and later by AI,
//! wait here until a person accepts or rejects each one. Nothing a machine proposes becomes a coding
//! without that decision, and every decision is logged.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::event;
use crate::model::{Origin, ProposalOrigin};
use crate::ops::{apply_coding_in, NewCoding};
use crate::project::Project;
use crate::search::CharCounter;
use crate::{Error, Result};

/// How many characters of the text around a suggestion the queue shows on each side.
const CONTEXT_CHARS: i64 = 70;
/// How many characters of the suggested words a row carries; longer passages say so.
const TEXT_CHARS: usize = 800;

/// One suggested coding.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NewProposal {
    pub source_id: i64,
    pub start: i64,
    pub end: i64,
    pub code_id: i64,
    pub coder_id: i64,
    #[serde(default)]
    pub unit_id: Option<i64>,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProposalRow {
    pub id: i64,
    pub origin: String,
    pub batch: String,
    pub status: String,
    pub source_id: i64,
    pub start: i64,
    pub end: i64,
    pub code_id: i64,
    pub coder_id: i64,
    pub unit_id: Option<i64>,
    pub rationale: String,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub decided_by: Option<String>,
    pub source_name: String,
    /// The suggested words (the first part of them when they are long).
    pub text: String,
    /// True when `text` stops before the end of the suggested words.
    pub truncated: bool,
    /// The words just before and just after, cut at the line break, so a suggestion can be read in place.
    pub before: String,
    pub after: String,
    /// True when this coder has already given this code to all of these words, so accepting would repeat a coding.
    pub already_coded: bool,
    /// The AI run that made it, for a suggestion from an AI.
    pub ai_run_id: Option<i64>,
}

/// Some words of a document with the text just before and after them.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Passage {
    pub source_name: String,
    pub text: String,
    /// True when `text` stops before the end of the words.
    pub truncated: bool,
    pub before: String,
    pub after: String,
}

/// One run of suggestions made together, with how far the decisions have got.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchSummary {
    pub batch: String,
    pub origin: String,
    /// How the suggestions were found: `words`, `speakers`, `headings`, or empty for a batch made before methods were named.
    pub method: String,
    /// What the person asked for, in words ("Words: cost").
    pub label: String,
    pub created_at: String,
    pub pending: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub code_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProposalBatch {
    pub batch: String,
    pub created: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct DecisionOutcome {
    pub accepted: usize,
    pub rejected: usize,
    /// Proposals that could not be accepted, with the reason. They stay pending.
    pub failed: Vec<(i64, String)>,
}

fn row_from(r: &rusqlite::Row) -> rusqlite::Result<ProposalRow> {
    let payload: String = r.get(4)?;
    let p: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
    let n = |k: &str| p[k].as_i64().unwrap_or(0);
    Ok(ProposalRow {
        id: r.get(0)?,
        origin: r.get(1)?,
        batch: r.get(2)?,
        status: r.get(3)?,
        source_id: n("source_id"),
        start: n("start"),
        end: n("end"),
        code_id: n("code_id"),
        coder_id: n("coder_id"),
        unit_id: p["unit_id"].as_i64(),
        rationale: p["rationale"].as_str().unwrap_or("").to_string(),
        created_at: r.get(5)?,
        decided_at: r.get(6)?,
        decided_by: r.get(7)?,
        source_name: String::new(),
        text: String::new(),
        truncated: false,
        before: String::new(),
        after: String::new(),
        already_coded: false,
        ai_run_id: r.get(8)?,
    })
}

const COLUMNS: &str = "id, origin, batch, status, payload_json, created_at, decided_at, decided_by, ai_run_id";

impl Project {
    /// Adds suggestions to the review queue as one batch. `label` names the batch (for example the
    /// search that made it). Each suggestion is checked now, so the queue never holds something that
    /// could not be accepted when it was made.
    pub fn create_proposals(&mut self, origin: ProposalOrigin, label: &str, items: &[NewProposal]) -> Result<ProposalBatch> {
        self.create_proposals_by(origin, label, "", items)
    }

    /// Like [`create_proposals`](Self::create_proposals), and records which method found the suggestions (`words`,
    /// `speakers`, or `headings`) so the queue can say how a batch came about.
    pub fn create_proposals_by(&mut self, origin: ProposalOrigin, label: &str, method: &str, items: &[NewProposal]) -> Result<ProposalBatch> {
        self.create_batch(origin, label, method, None, items)
    }

    /// Adds the suggestions an AI answer made, tied to the run that made them.
    pub fn create_ai_proposals(&mut self, run_id: i64, label: &str, items: &[NewProposal]) -> Result<ProposalBatch> {
        self.create_batch(ProposalOrigin::Ai, label, "ai", Some(run_id), items)
    }

    fn create_batch(&mut self, origin: ProposalOrigin, label: &str, method: &str, ai_run_id: Option<i64>, items: &[NewProposal]) -> Result<ProposalBatch> {
        self.ensure_writable()?;
        if items.is_empty() {
            return Err(Error::Invalid("there is nothing to suggest".to_string()));
        }
        let tx = self.conn.transaction()?;
        let suffix: String = tx.query_row("SELECT lower(hex(randomblob(3)))", [], |r| r.get(0))?;
        let batch = format!("{}-{suffix}", label.chars().filter(|c| c.is_alphanumeric()).take(20).collect::<String>().to_lowercase());
        for (i, item) in items.iter().enumerate() {
            let len: Option<i64> = tx.query_row("SELECT char_count FROM source WHERE id = ?1", [item.source_id], |r| r.get(0)).optional()?;
            let len = len.ok_or(Error::NotFound { what: "source", id: item.source_id })?;
            if item.start < 0 || item.end <= item.start || item.end > len {
                return Err(Error::Invalid(format!("suggestion {}: the span {}..{} does not fit source {}", i + 1, item.start, item.end, item.source_id)));
            }
            let status: Option<String> = tx.query_row("SELECT status FROM code WHERE id = ?1", [item.code_id], |r| r.get(0)).optional()?;
            if status.as_deref() != Some("active") {
                return Err(Error::Invalid(format!("suggestion {}: code {} is not an active code", i + 1, item.code_id)));
            }
            let coder: Option<i64> = tx.query_row("SELECT id FROM coder WHERE id = ?1", [item.coder_id], |r| r.get(0)).optional()?;
            if coder.is_none() {
                return Err(Error::NotFound { what: "coder", id: item.coder_id });
            }
            let payload = json!({
                "source_id": item.source_id, "start": item.start, "end": item.end, "code_id": item.code_id,
                "coder_id": item.coder_id, "unit_id": item.unit_id, "rationale": item.rationale,
            });
            tx.execute(
                "INSERT INTO proposal (kind, origin, batch, payload_json, ai_run_id, created_at)
                 VALUES ('coding', ?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![origin.as_str(), batch, payload.to_string(), ai_run_id],
            )?;
        }
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "proposal.create",
            None,
            None,
            Some(&json!({ "batch": batch, "origin": origin.as_str(), "label": label, "method": method, "suggestions": items.len() })),
        )?;
        tx.commit()?;
        Ok(ProposalBatch { batch, created: items.len() })
    }

    /// Suggestions in the queue, optionally only those with one status (`pending`, `accepted`,
    /// `rejected`) or from one batch, in the order they were made.
    pub fn list_proposals(&self, status: Option<&str>, batch: Option<&str>, limit: usize) -> Result<Vec<ProposalRow>> {
        self.list_proposals_page(status, batch, limit, 0)
    }

    /// One page of the queue: `limit` suggestions after the first `offset`, each with its words and the text around them.
    pub fn list_proposals_page(&self, status: Option<&str>, batch: Option<&str>, limit: usize, offset: usize) -> Result<Vec<ProposalRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM proposal WHERE (?1 IS NULL OR status = ?1) AND (?2 IS NULL OR batch = ?2) ORDER BY id LIMIT ?3 OFFSET ?4"
        ))?;
        let rows = stmt.query_map(params![status, batch, limit as i64, offset as i64], row_from)?;
        let mut rows: Vec<ProposalRow> = rows.collect::<std::result::Result<_, _>>()?;
        self.fill_passages(&mut rows)?;
        Ok(rows)
    }

    /// One suggestion, with its words and context.
    pub fn get_proposal(&self, id: i64) -> Result<ProposalRow> {
        let row = self
            .conn
            .query_row(&format!("SELECT {COLUMNS} FROM proposal WHERE id = ?1"), [id], row_from)
            .optional()?
            .ok_or(Error::NotFound { what: "suggestion", id })?;
        let mut rows = vec![row];
        self.fill_passages(&mut rows)?;
        Ok(rows.remove(0))
    }

    /// Every run of suggestions, newest first, with how many wait, were accepted, and were rejected.
    pub fn proposal_batches(&self) -> Result<Vec<BatchSummary>> {
        // One pass over the queue, however many suggestions it holds: the counts and the codes come out of the same grouping.
        let mut stmt = self.conn.prepare(
            "SELECT batch, origin, MIN(created_at), SUM(status = 'pending'), SUM(status = 'accepted'), SUM(status = 'rejected'),
                    group_concat(DISTINCT CAST(json_extract(payload_json, '$.code_id') AS INTEGER))
             FROM proposal GROUP BY batch ORDER BY MIN(id) DESC",
        )?;
        type Row = (String, String, String, i64, i64, i64, Option<String>);
        let rows: Vec<Row> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (batch, origin, created_at, pending, accepted, rejected, codes) in rows {
            let mut code_ids: Vec<i64> = codes.unwrap_or_default().split(',').filter_map(|c| c.trim().parse().ok()).collect();
            code_ids.sort_unstable();
            let (label, method): (Option<String>, Option<String>) = self
                .conn
                .query_row(
                    "SELECT json_extract(after_json, '$.label'), json_extract(after_json, '$.method') FROM event
                     WHERE action = 'proposal.create' AND json_extract(after_json, '$.batch') = ?1 ORDER BY id DESC LIMIT 1",
                    [&batch],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?
                .unwrap_or((None, None));
            out.push(BatchSummary {
                label: label.unwrap_or_else(|| batch.clone()),
                method: method.unwrap_or_default(),
                batch,
                origin,
                created_at,
                pending: pending as usize,
                accepted: accepted as usize,
                rejected: rejected as usize,
                code_ids,
            });
        }
        Ok(out)
    }

    /// Changes the words, or the code, of a suggestion that still waits, so a person can correct it before accepting it.
    /// The change is logged and can be undone.
    pub fn adjust_proposal(&mut self, id: i64, start: i64, end: i64, code_id: i64) -> Result<ProposalRow> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let row = tx
            .query_row(&format!("SELECT {COLUMNS} FROM proposal WHERE id = ?1"), [id], row_from)
            .optional()?
            .ok_or(Error::NotFound { what: "suggestion", id })?;
        if row.status != "pending" {
            return Err(Error::Invalid(format!("the suggestion was already {}, so it cannot be changed", row.status)));
        }
        let len: i64 = tx.query_row("SELECT char_count FROM source WHERE id = ?1", [row.source_id], |r| r.get(0))?;
        if start < 0 || end <= start || end > len {
            return Err(Error::Invalid(format!("the words {start}..{end} do not fit the document")));
        }
        let status: Option<String> = tx.query_row("SELECT status FROM code WHERE id = ?1", [code_id], |r| r.get(0)).optional()?;
        if status.as_deref() != Some("active") {
            return Err(Error::Invalid("choose an active code".to_string()));
        }
        if let Some(unit_id) = row.unit_id {
            let bounds: Option<(i64, i64)> = tx
                .query_row("SELECT start_offset, end_offset FROM unit WHERE id = ?1", [unit_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            if let Some((us, ue)) = bounds {
                if start < us || end > ue {
                    return Err(Error::Invalid("the words must stay inside the unit this suggestion belongs to".to_string()));
                }
            }
        }
        if (start, end, code_id) == (row.start, row.end, row.code_id) {
            drop(tx);
            return self.get_proposal(id);
        }
        tx.execute(
            "UPDATE proposal SET payload_json = json_set(payload_json, '$.start', ?1, '$.end', ?2, '$.code_id', ?3) WHERE id = ?4",
            params![start, end, code_id, id],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "proposal.adjust",
            Some(("proposal", id)),
            Some(&json!({ "start": row.start, "end": row.end, "code_id": row.code_id })),
            Some(&json!({ "start": start, "end": end, "code_id": code_id, "batch": row.batch })),
        )?;
        tx.commit()?;
        self.get_proposal(id)
    }

    /// Accepts or rejects every suggestion of one batch that still waits.
    pub fn decide_batch(&mut self, batch: &str, accept: bool) -> Result<DecisionOutcome> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT id FROM proposal WHERE batch = ?1 AND status = 'pending' ORDER BY id")?;
            let ids = stmt.query_map([batch], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            ids
        };
        self.decide_proposals(&ids, accept)
    }

    /// Accepts or rejects suggestions. An accepted one becomes a real coding, made by the person
    /// who accepted it (a coding accepted from an AI suggestion keeps that origin). One that cannot
    /// be accepted (its code was retired, say) is reported and left pending. Everything else in the
    /// call goes through together, and one undo takes the whole call back.
    pub fn decide_proposals(&mut self, ids: &[i64], accept: bool) -> Result<DecisionOutcome> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let decision: String = tx.query_row("SELECT lower(hex(randomblob(6)))", [], |r| r.get(0))?;
        let mut out = DecisionOutcome::default();
        for id in ids {
            let row = tx
                .query_row(&format!("SELECT {COLUMNS} FROM proposal WHERE id = ?1"), [id], row_from)
                .optional()?
                .ok_or(Error::NotFound { what: "suggestion", id: *id })?;
            if row.status != "pending" {
                out.failed.push((*id, format!("it was already {}", row.status)));
                continue;
            }
            let mut coding_id = None;
            if accept {
                let covered: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM coding WHERE source_id = ?1 AND code_id = ?2 AND coder_id = ?3 AND layer = 'coder' AND start_offset <= ?4 AND end_offset >= ?5 LIMIT 1",
                        params![row.source_id, row.code_id, row.coder_id, row.start, row.end],
                        |r| r.get(0),
                    )
                    .optional()?;
                if covered.is_some() {
                    out.failed.push((*id, "these words already have this code from the same coder".to_string()));
                    continue;
                }
                let mut coding = NewCoding::new(row.source_id, row.start, row.end, row.code_id, row.coder_id);
                coding.unit_id = row.unit_id;
                coding.rationale = row.rationale.clone();
                coding.origin = if row.origin == "ai" { Origin::AiSuggestionAccepted } else { Origin::Human };
                coding.via_proposal = Some(*id);
                coding.ai_run_id = row.ai_run_id;
                // A failed acceptance must not poison the batch, so try it inside a savepoint.
                tx.execute_batch("SAVEPOINT accept_one")?;
                match apply_coding_in(&tx, &self.actor, &self.session, &coding) {
                    Ok(cid) => {
                        tx.execute_batch("RELEASE accept_one")?;
                        coding_id = Some(cid);
                    }
                    Err(e) => {
                        tx.execute_batch("ROLLBACK TO accept_one; RELEASE accept_one")?;
                        out.failed.push((*id, e.to_string()));
                        continue;
                    }
                }
            }
            tx.execute(
                "UPDATE proposal SET status = ?1, decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), decided_by = ?2 WHERE id = ?3",
                params![if accept { "accepted" } else { "rejected" }, self.actor, id],
            )?;
            event::insert(
                &tx,
                &self.actor,
                &self.session,
                if accept { "proposal.accept" } else { "proposal.reject" },
                Some(("proposal", *id)),
                None,
                Some(&json!({ "batch": row.batch, "origin": row.origin, "coding_id": coding_id, "decision": decision })),
            )?;
            if accept {
                out.accepted += 1;
            } else {
                out.rejected += 1;
            }
        }
        tx.commit()?;
        Ok(out)
    }

    /// The words at each place `(source, start, end)`, with the document's name and the text just around them (cut at the
    /// line break), in the same order as asked. Each document's text is read once and walked in order, so a long list
    /// over one big document stays quick. A place that does not fit its document comes back empty.
    pub fn passages(&self, places: &[(i64, i64, i64)]) -> Result<Vec<Passage>> {
        let mut out = vec![Passage::default(); places.len()];
        let mut by_source: HashMap<i64, Vec<usize>> = HashMap::new();
        for (i, (source_id, _, _)) in places.iter().enumerate() {
            by_source.entry(*source_id).or_default().push(i);
        }
        for (source_id, mut indexes) in by_source {
            let found: Option<(String, String)> =
                self.conn.query_row("SELECT name, text FROM source WHERE id = ?1", [source_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            let Some((name, text)) = found else { continue };
            indexes.sort_by_key(|i| places[*i].1);
            let mut counter = CharCounter::new(&text);
            for i in indexes {
                let (_, start, end) = places[i];
                let b_before = counter.byte_of((start - CONTEXT_CHARS).max(0));
                let b_start = counter.byte_of(start);
                let b_end = counter.byte_of(end);
                let b_after = counter.byte_of(end + CONTEXT_CHARS);
                if b_start > text.len() || b_end > text.len() || b_before > b_start || b_end > b_after {
                    continue;
                }
                let words = &text[b_start..b_end];
                let before = &text[b_before..b_start];
                let after = &text[b_end..b_after];
                let before = before.rfind(['\n', '\r']).map_or(before, |p| &before[p + 1..]);
                let after = after.find(['\n', '\r']).map_or(after, |p| &after[..p]);
                out[i] = Passage {
                    source_name: name.clone(),
                    truncated: words.chars().count() > TEXT_CHARS,
                    text: words.chars().take(TEXT_CHARS).collect(),
                    before: before.to_string(),
                    after: after.to_string(),
                };
            }
        }
        Ok(out)
    }

    /// Fills in the document name, the suggested words, the text around them, and whether the coding already exists.
    fn fill_passages(&self, rows: &mut [ProposalRow]) -> Result<()> {
        let places: Vec<(i64, i64, i64)> = rows.iter().map(|r| (r.source_id, r.start, r.end)).collect();
        for (row, p) in rows.iter_mut().zip(self.passages(&places)?) {
            row.source_name = p.source_name;
            row.text = p.text;
            row.truncated = p.truncated;
            row.before = p.before;
            row.after = p.after;
        }
        for r in rows.iter_mut().filter(|r| r.status == "pending") {
            let covered: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM coding WHERE source_id = ?1 AND code_id = ?2 AND coder_id = ?3 AND layer = 'coder' AND start_offset <= ?4 AND end_offset >= ?5 LIMIT 1",
                    params![r.source_id, r.code_id, r.coder_id, r.start, r.end],
                    |row| row.get(0),
                )
                .optional()?;
            r.already_coded = covered.is_some();
        }
        Ok(())
    }
}
