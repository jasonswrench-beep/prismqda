//! Adjudication (specification 18.4): settling the units where coders gave different codes. The queue lists every unit that two or more of
//! the chosen coders coded and did not code the same way. A person decides what codes the unit should carry, and says why. The decision
//! is kept apart from every coder's own coding, which is never touched, and together the decisions are the final layer.
//!
//! A decision may be made blind: the coders are then named "Coder A", "Coder B", and so on, in an order that changes from unit to unit, so
//! the person settling the case cannot tell who coded what or learn each coder's habits from the order.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::event;
use crate::project::Project;
use crate::{Error, Result};

/// The longest part of a unit's words sent with a disagreement.
const MAX_TEXT_CHARS: usize = 600;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeRef {
    pub id: i64,
    pub name: String,
}

/// What one coder gave a unit. When the queue is blind, `coder_id` and `role` are left out and `label` is "Coder A" and so on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoderCodes {
    pub coder_id: Option<i64>,
    pub label: String,
    pub role: Option<String>,
    pub codes: Vec<CodeRef>,
}

/// What was decided for a unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decision {
    pub id: i64,
    pub unit_id: i64,
    pub adjudicator_id: i64,
    pub adjudicator_name: String,
    pub reason: String,
    /// The codes the unit carries now. Empty says that none of the codes applies.
    pub codes: Vec<CodeRef>,
    pub created_at: String,
}

/// A unit the chosen coders did not code the same way.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Disagreement {
    pub unit_id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    /// The unit's words, cut to a length a person can read at a glance.
    pub text: String,
    pub by_coder: Vec<CoderCodes>,
    pub decision: Option<Decision>,
}

/// A decision with where it was made, for lists and reports.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DecisionRow {
    #[serde(flatten)]
    pub decision: Decision,
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    pub text: String,
}

fn cut(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_TEXT_CHARS {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX_TEXT_CHARS - 1).collect();
    out.push('\u{2026}');
    out
}

/// A number that depends on both the unit and the coder, so the order of the coders in a blind queue differs from unit to unit.
fn blind_rank(unit_id: i64, coder_id: i64) -> u64 {
    let a = (unit_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let b = (coder_id as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let x = a ^ b.rotate_left(17);
    x ^ (x >> 29)
}

impl Project {
    /// The units that two or more of these coders coded and did not code the same way, in reading order, each with the decision made for it
    /// (if one was). `blind` hides who coded what. Only the coders' own codings are compared, never the decisions.
    pub fn disagreements(&self, coder_ids: &[i64], blind: bool) -> Result<Vec<Disagreement>> {
        if coder_ids.len() < 2 {
            return Err(Error::Invalid("choose at least two coders to compare".to_string()));
        }
        let distinct: HashSet<_> = coder_ids.iter().collect();
        if distinct.len() != coder_ids.len() {
            return Err(Error::Invalid("the same coder was chosen twice".to_string()));
        }
        let coders = self.coder_infos(coder_ids)?;
        let codes = self.codes_by_unit(coder_ids, false)?;
        let names: HashMap<i64, String> = self.list_codes()?.into_iter().map(|c| (c.id, c.name)).collect();

        let mut candidates: HashSet<i64> = HashSet::new();
        for per_coder in &codes {
            candidates.extend(per_coder.keys());
        }
        let mut out = Vec::new();
        let mut unit_stmt = self.conn.prepare("SELECT u.source_id, s.name, u.start_offset, u.end_offset FROM unit u JOIN source s ON s.id = u.source_id WHERE u.id = ?1")?;
        for unit in candidates {
            // The coders who coded this unit, with what each gave it.
            let mut given: Vec<(usize, Vec<i64>)> = Vec::new();
            for (i, per_coder) in codes.iter().enumerate() {
                if let Some(set) = per_coder.get(&unit) {
                    let mut ids: Vec<i64> = set.iter().copied().collect();
                    ids.sort_unstable();
                    given.push((i, ids));
                }
            }
            if given.len() < 2 || given.iter().all(|(_, ids)| *ids == given[0].1) {
                continue;
            }
            let Some((source_id, source_name, start, end)) = unit_stmt
                .query_row([unit], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))
                .optional()?
            else {
                continue;
            };
            if blind {
                given.sort_by_key(|(i, _)| blind_rank(unit, coders[*i].id));
            }
            let by_coder = given
                .iter()
                .enumerate()
                .map(|(n, (i, ids))| CoderCodes {
                    coder_id: if blind { None } else { Some(coders[*i].id) },
                    label: if blind { format!("Coder {}", (b'A' + n as u8) as char) } else { coders[*i].name.clone() },
                    role: if blind { None } else { Some(coders[*i].role.clone()) },
                    codes: ids.iter().map(|id| CodeRef { id: *id, name: names.get(id).cloned().unwrap_or_else(|| id.to_string()) }).collect(),
                })
                .collect();
            out.push(Disagreement {
                unit_id: unit,
                source_id,
                source_name,
                start,
                end,
                text: cut(&self.text_of_span(source_id, start, end)?),
                by_coder,
                decision: self.decision_for(unit)?,
            });
        }
        out.sort_by_key(|d| (d.source_id, d.start, d.unit_id));
        Ok(out)
    }

    /// The decision made for a unit, if there is one.
    pub fn decision_for(&self, unit_id: i64) -> Result<Option<Decision>> {
        let row: Option<(i64, i64, String, String, String)> = self
            .conn
            .query_row(
                "SELECT a.id, a.adjudicator_id, c.name, a.reason, a.created_at FROM adjudication a JOIN coder c ON c.id = a.adjudicator_id WHERE a.unit_id = ?1",
                [unit_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((id, adjudicator_id, adjudicator_name, reason, created_at)) = row else { return Ok(None) };
        let mut stmt = self
            .conn
            .prepare("SELECT c.id, c.name FROM adjudication_code ac JOIN code c ON c.id = ac.code_id WHERE ac.adjudication_id = ?1 ORDER BY c.name, c.id")?;
        let codes = stmt.query_map([id], |r| Ok(CodeRef { id: r.get(0)?, name: r.get(1)? }))?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Some(Decision { id, unit_id, adjudicator_id, adjudicator_name, reason, codes, created_at }))
    }

    /// Settles a unit: the codes it should carry (none at all is allowed and says that no code applies) and the reason, made by the
    /// adjudicator. A decision already made for the unit is replaced, and the log keeps what it said.
    pub fn decide(&mut self, unit_id: i64, code_ids: &[i64], reason: &str, adjudicator_id: i64) -> Result<Decision> {
        self.ensure_writable()?;
        let reason = reason.trim().to_string();
        if reason.is_empty() {
            return Err(Error::Invalid("a decision needs its reason: say why these codes are the right ones".to_string()));
        }
        let mut wanted: Vec<i64> = Vec::new();
        for id in code_ids {
            if !wanted.contains(id) {
                wanted.push(*id);
            }
        }
        let before = self.decision_for(unit_id)?;
        let tx = self.conn.transaction()?;
        let unit: Option<i64> = tx.query_row("SELECT id FROM unit WHERE id = ?1", [unit_id], |r| r.get(0)).optional()?;
        if unit.is_none() {
            return Err(Error::NotFound { what: "unit", id: unit_id });
        }
        let coder: Option<i64> = tx.query_row("SELECT id FROM coder WHERE id = ?1", [adjudicator_id], |r| r.get(0)).optional()?;
        if coder.is_none() {
            return Err(Error::NotFound { what: "coder", id: adjudicator_id });
        }
        for id in &wanted {
            let status: Option<String> = tx.query_row("SELECT status FROM code WHERE id = ?1", [id], |r| r.get(0)).optional()?;
            match status.as_deref() {
                None => return Err(Error::NotFound { what: "code", id: *id }),
                Some("active") => {}
                Some(other) => return Err(Error::Invalid(format!("code {id} is {other}, so a decision cannot use it"))),
            }
        }
        tx.execute("DELETE FROM adjudication_code WHERE adjudication_id IN (SELECT id FROM adjudication WHERE unit_id = ?1)", [unit_id])?;
        tx.execute("DELETE FROM adjudication WHERE unit_id = ?1", [unit_id])?;
        tx.execute(
            "INSERT INTO adjudication (unit_id, adjudicator_id, reason, created_at) VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![unit_id, adjudicator_id, reason],
        )?;
        let id = tx.last_insert_rowid();
        for code in &wanted {
            tx.execute("INSERT INTO adjudication_code (adjudication_id, code_id) VALUES (?1, ?2)", params![id, code])?;
        }
        let snapshot = |adjudicator: i64, reason: &str, codes: &[i64], at: Option<&str>| -> Value {
            json!({ "unit_id": unit_id, "adjudicator_id": adjudicator, "reason": reason, "codes": codes, "created_at": at })
        };
        let earlier = before.as_ref().map(|d| snapshot(d.adjudicator_id, &d.reason, &d.codes.iter().map(|c| c.id).collect::<Vec<_>>(), Some(&d.created_at)));
        event::insert(&tx, &self.actor, &self.session, "adjudication.set", Some(("unit", unit_id)), earlier.as_ref(), Some(&snapshot(adjudicator_id, &reason, &wanted, None)))?;
        tx.commit()?;
        Ok(self.decision_for(unit_id)?.expect("the decision was just stored"))
    }

    /// Every decision, in reading order, with where it was made.
    pub fn decisions(&self) -> Result<Vec<DecisionRow>> {
        let mut stmt = self.conn.prepare("SELECT a.unit_id, u.source_id, s.name, u.start_offset, u.end_offset FROM adjudication a JOIN unit u ON u.id = a.unit_id JOIN source s ON s.id = u.source_id ORDER BY u.source_id, u.start_offset, a.id")?;
        let rows: Vec<(i64, i64, String, i64, i64)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (unit_id, source_id, source_name, start, end) in rows {
            if let Some(decision) = self.decision_for(unit_id)? {
                out.push(DecisionRow { decision, source_id, source_name, start, end, text: cut(&self.text_of_span(source_id, start, end)?) });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blind_order_changes_from_unit_to_unit_but_not_from_call_to_call() {
        assert_eq!(blind_rank(4, 1), blind_rank(4, 1));
        let first_wins: Vec<bool> = (1..60).map(|u| blind_rank(u, 1) < blind_rank(u, 2)).collect();
        assert!(first_wins.iter().any(|b| *b) && first_wins.iter().any(|b| !*b), "neither coder is always first");
    }

    #[test]
    fn a_long_unit_is_cut_for_the_queue() {
        let long = "word ".repeat(400);
        let c = cut(&long);
        assert_eq!(c.chars().count(), MAX_TEXT_CHARS);
        assert!(c.ends_with('\u{2026}'));
        assert_eq!(cut("  short  "), "short");
    }
}
