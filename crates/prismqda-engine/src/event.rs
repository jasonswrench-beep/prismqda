//! The audit trail (specification 4.2 and 27): append-only, one row per action.

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

use crate::Result;

/// One logged action, as read back from the event table.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Event {
    pub id: i64,
    pub ts: String,
    pub actor: String,
    pub session: String,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// Appends one event. Callers pass the open transaction so the event commits or rolls back
/// together with the change it describes.
pub(crate) fn insert(
    conn: &Connection,
    actor: &str,
    session: &str,
    action: &str,
    target: Option<(&str, i64)>,
    before: Option<&Value>,
    after: Option<&Value>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO event (ts, actor, session, action, target_type, target_id, before_json, after_json)
         VALUES (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            actor,
            session,
            action,
            target.map(|t| t.0),
            target.map(|t| t.1),
            before.map(|v| v.to_string()),
            after.map(|v| v.to_string()),
        ],
    )?;
    Ok(())
}

/// One line of an item's history: what happened to it, when, and by whom.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HistoryEntry {
    pub ts: String,
    pub actor: String,
    pub action: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// The events whose target is one coding, oldest first.
///
/// A coding's number can be handed out again once the coding is gone (SQLite reuses the highest
/// number), so the history starts at the newest `coding.add` for that number. Without that, a new
/// coding would inherit the life of an old one. Undoing a removal puts the same coding back under the
/// same number without a new `coding.add`, so its history carries on unbroken.
pub(crate) fn for_coding(conn: &Connection, coding_id: i64) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT ts, actor, action, before_json, after_json FROM event
         WHERE target_type = 'coding' AND target_id = ?1
           AND id >= COALESCE((SELECT MAX(id) FROM event WHERE target_type = 'coding' AND target_id = ?1 AND action = 'coding.add'), 0)
         ORDER BY id",
    )?;
    let rows = stmt.query_map([coding_id], |r| {
        let before: Option<String> = r.get(3)?;
        let after: Option<String> = r.get(4)?;
        Ok(HistoryEntry {
            ts: r.get(0)?,
            actor: r.get(1)?,
            action: r.get(2)?,
            before: before.and_then(|s| serde_json::from_str(&s).ok()),
            after: after.and_then(|s| serde_json::from_str(&s).ok()),
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// The events whose target is one coded area of a picture, oldest first, starting at the newest `region.add` for that number.
pub(crate) fn for_region(conn: &Connection, region_id: i64) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT ts, actor, action, before_json, after_json FROM event
         WHERE target_type = 'region' AND target_id = ?1
           AND id >= COALESCE((SELECT MAX(id) FROM event WHERE target_type = 'region' AND target_id = ?1 AND action = 'region.add'), 0)
         ORDER BY id",
    )?;
    let rows = stmt.query_map([region_id], |r| {
        let before: Option<String> = r.get(3)?;
        let after: Option<String> = r.get(4)?;
        Ok(HistoryEntry {
            ts: r.get(0)?,
            actor: r.get(1)?,
            action: r.get(2)?,
            before: before.and_then(|s| serde_json::from_str(&s).ok()),
            after: after.and_then(|s| serde_json::from_str(&s).ok()),
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Reads the newest `limit` events, returned oldest first.
pub(crate) fn recent(conn: &Connection, limit: usize) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, ts, actor, session, action, target_type, target_id, before_json, after_json
         FROM event ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |r| {
        let before: Option<String> = r.get(7)?;
        let after: Option<String> = r.get(8)?;
        Ok(Event {
            id: r.get(0)?,
            ts: r.get(1)?,
            actor: r.get(2)?,
            session: r.get(3)?,
            action: r.get(4)?,
            target_type: r.get(5)?,
            target_id: r.get(6)?,
            // The columns are checked with json_valid() in the schema, so parsing cannot fail.
            before: before.and_then(|s| serde_json::from_str(&s).ok()),
            after: after.and_then(|s| serde_json::from_str(&s).ok()),
        })
    })?;
    let mut events = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    events.reverse();
    Ok(events)
}
