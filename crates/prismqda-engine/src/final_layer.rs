//! The final layer (specification 18.4, decision 173): what the project says once the coders' disagreements are settled.
//!
//! For each unit of a document: where a person decided it (Team > Disagreements), the decision's codes are the unit's final codes; where
//! the coders who coded it gave it the same codes (or only one coder coded it), those codes are final, and are that coder's own codings;
//! where the coders disagree and nobody has decided, the unit is open and has no final codes yet. Nothing is stored: the layer is worked
//! out from the codings and the decisions each time it is asked for, so it is always what they say now, and no coder's coding is changed
//! or counted twice. It covers words coded in a unit; areas of pictures and stretches of recordings are not adjudicated and are not in it,
//! and neither are words coded without a unit.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Serialize;

use crate::project::Project;
use crate::Result;

/// Ids of the codings a decision makes are numbers no coding and no coded area has: below this, made from the unit and the code.
const FINAL_BASE: i64 = 1 << 50;

/// The number a decision's coding of a unit with a code goes by. It is negative and never equals a coding's number or an area's.
pub(crate) fn final_row_id(unit_id: i64, code_id: i64) -> i64 {
    -(FINAL_BASE + (unit_id << 24) + code_id)
}

/// Whether an id is one of a decision's codings (see `final_row_id`).
pub(crate) fn is_final_id(id: i64) -> bool {
    id <= -FINAL_BASE
}

/// One coding of the final layer.
#[derive(Debug, Clone)]
pub(crate) struct FinalRow {
    /// The coder's own coding when the coders agreed (`real`), or a number made from the decision.
    pub id: i64,
    pub source_id: i64,
    pub start: i64,
    pub end: i64,
    pub code_id: i64,
    /// The coder whose coding it is, or the person who decided.
    pub coder_id: i64,
    pub origin: String,
    /// The coder's reason, or the reason the decision gave.
    pub rationale: String,
    pub unit_id: i64,
    pub created_at: String,
    pub rating: Option<i64>,
    pub real: bool,
}

/// The final layer, and how it stands.
#[derive(Debug, Clone, Default)]
pub(crate) struct FinalLayer {
    pub rows: Vec<FinalRow>,
    pub decided: usize,
    pub agreed: usize,
    pub open: usize,
    pub loose: usize,
}

/// How the final layer stands, for a person deciding whether to use it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FinalSummary {
    /// Units a person decided.
    pub decided: usize,
    /// Units the coders agreed on (or only one coded) that nobody had to decide.
    pub agreed: usize,
    /// Units the coders disagree on that nobody has decided: they have no final codes yet.
    pub open: usize,
    /// Codings of words that are not in a unit, which the final layer does not cover.
    pub without_unit: usize,
    /// The same for coded areas of pictures and PDF pages and stretches of recordings (decision 176): decided, agreed, and still open.
    pub areas_decided: usize,
    pub areas_agreed: usize,
    pub areas_open: usize,
}

impl Project {
    pub(crate) fn final_layer(&self) -> Result<FinalLayer> {
        let mut out = FinalLayer::default();

        // the decisions: each carries its codes (none at all says that no code applies)
        let mut decided: HashSet<i64> = HashSet::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT a.unit_id, u.source_id, u.start_offset, u.end_offset, a.adjudicator_id, a.reason, a.created_at, ac.code_id
                 FROM adjudication a JOIN unit u ON u.id = a.unit_id LEFT JOIN adjudication_code ac ON ac.adjudication_id = a.id
                 ORDER BY a.unit_id, ac.code_id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?, r.get::<_, Option<i64>>(7)?))
            })?;
            for row in rows {
                let (unit, source, start, end, adjudicator, reason, created_at, code) = row?;
                decided.insert(unit);
                if let Some(code_id) = code {
                    out.rows.push(FinalRow { id: final_row_id(unit, code_id), source_id: source, start, end, code_id, coder_id: adjudicator, origin: "human".to_string(), rationale: reason, unit_id: unit, created_at, rating: None, real: false });
                }
            }
        }
        out.decided = decided.len();

        // the coders' own codings of the units nobody has decided
        type Raw = (i64, i64, i64, i64, i64, i64, String, String, Option<i64>, String);
        let mut by_unit: BTreeMap<i64, Vec<Raw>> = BTreeMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT unit_id, id, source_id, start_offset, end_offset, code_id, coder_id, origin, rationale, rating, created_at
                 FROM coding WHERE layer = 'coder' AND unit_id IS NOT NULL ORDER BY unit_id, coder_id, start_offset, end_offset, id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, i64>(6)?, r.get::<_, String>(7)?, r.get::<_, String>(8)?, r.get::<_, Option<i64>>(9)?, r.get::<_, String>(10)?)))
            })?;
            for row in rows {
                let (unit, raw) = row?;
                if !decided.contains(&unit) {
                    by_unit.entry(unit).or_default().push(raw);
                }
            }
        }
        for (unit, rows) in by_unit {
            // (id, source, start, end, code, coder, origin, rationale, rating, created_at)
            let mut sets: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
            for r in &rows {
                sets.entry(r.5).or_default().insert(r.4);
            }
            let Some(first) = sets.values().next() else { continue };
            if !sets.values().all(|s| s == first) {
                out.open += 1;
                continue;
            }
            out.agreed += 1;
            let lowest = *sets.keys().next().unwrap_or(&0);
            let mut seen: HashSet<(i64, i64, i64)> = HashSet::new();
            for r in rows.into_iter().filter(|r| r.5 == lowest) {
                if seen.insert((r.2, r.3, r.4)) {
                    out.rows.push(FinalRow { id: r.0, source_id: r.1, start: r.2, end: r.3, code_id: r.4, coder_id: r.5, origin: r.6, rationale: r.7, unit_id: unit, created_at: r.9, rating: r.8, real: true });
                }
            }
        }
        out.loose = self.conn.query_row("SELECT COUNT(*) FROM coding WHERE layer = 'coder' AND unit_id IS NULL", [], |r| r.get::<_, i64>(0))? as usize;
        out.rows.sort_by_key(|r| (r.source_id, r.start, r.end, r.id));
        Ok(out)
    }

    /// How many units are decided, agreed, and still open, for a person deciding whether to use the final layer.
    pub fn final_summary(&self) -> Result<FinalSummary> {
        let f = self.final_layer()?;
        let (_, a) = self.final_region_ids()?;
        Ok(FinalSummary { decided: f.decided, agreed: f.agreed, open: f.open, without_unit: f.loose, areas_decided: a.decided, areas_agreed: a.agreed, areas_open: a.open })
    }
}
