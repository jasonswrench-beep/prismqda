//! Memos and links (specification 10.1 and 10.3): written analysis that travels with the thing it is about.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::event;
use crate::model::{MemoKind, MemoScope};
use crate::project::Project;
use crate::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct NewMemo {
    pub kind: MemoKind,
    pub scope: MemoScope,
    /// The source, code, coding, or case the memo is about. Not used for project, phase, or standalone memos.
    #[serde(default)]
    pub target_id: Option<i64>,
    /// The method phase, for a phase memo.
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoRow {
    pub id: i64,
    pub kind: String,
    pub scope: String,
    pub target_id: Option<i64>,
    pub phase: Option<String>,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinkRow {
    pub id: i64,
    pub from_type: String,
    pub from_id: i64,
    pub to_type: String,
    pub to_id: i64,
    pub label: String,
}

const MEMO_COLUMNS: &str = "id, kind, scope, target_id, phase, title, body, created_at, updated_at";

fn memo_from_row(r: &rusqlite::Row) -> rusqlite::Result<MemoRow> {
    Ok(MemoRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        scope: r.get(2)?,
        target_id: r.get(3)?,
        phase: r.get(4)?,
        title: r.get(5)?,
        body: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

/// The tables that entities of each kind live in, for checking that a memo or link points at something real.
fn table_for(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "source" => "source",
        "code" => "code",
        "coding" => "coding",
        "case" => "research_case",
        "unit" => "unit",
        "memo" => "memo",
        "coder" => "coder",
        _ => return None,
    })
}

fn exists(tx: &Transaction, kind: &str, id: i64) -> Result<bool> {
    // An area of a picture is numbered apart from the codings of words: area 7 is called -7 wherever the two kinds share a list, so a
    // memo or a link about an area names the coding -7 and it is looked for among the areas.
    if kind == "coding" && id < 0 {
        return Ok(tx.query_row("SELECT 1 FROM region WHERE id = ?1", [-id], |_| Ok(())).optional()?.is_some());
    }
    let table = table_for(kind).ok_or_else(|| Error::Invalid(format!("'{kind}' is not something a memo or link can point at")))?;
    Ok(tx.query_row(&format!("SELECT 1 FROM {table} WHERE id = ?1"), [id], |_| Ok(())).optional()?.is_some())
}

impl Project {
    pub fn add_memo(&mut self, memo: &NewMemo) -> Result<i64> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        match memo.scope {
            MemoScope::Project | MemoScope::Standalone => {
                if memo.target_id.is_some() {
                    return Err(Error::Invalid(format!("a {} memo is not attached to anything", memo.scope.as_str())));
                }
            }
            MemoScope::Phase => {
                if memo.phase.as_deref().is_none_or(|p| p.trim().is_empty()) {
                    return Err(Error::Invalid("a phase memo needs the name of the phase".to_string()));
                }
            }
            scope => {
                let id = memo.target_id.ok_or_else(|| Error::Invalid(format!("a {} memo needs something to attach to", scope.as_str())))?;
                if !exists(&tx, scope.as_str(), id)? {
                    return Err(Error::NotFound { what: "memo target", id });
                }
            }
        }
        tx.execute(
            "INSERT INTO memo (kind, scope, target_id, phase, title, body, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![memo.kind.as_str(), memo.scope.as_str(), memo.target_id, memo.phase, memo.title.trim(), memo.body],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "memo.add",
            Some(("memo", id)),
            None,
            Some(&json!({ "kind": memo.kind.as_str(), "scope": memo.scope.as_str(), "target_id": memo.target_id, "title": memo.title.trim() })),
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Changes a memo's title or body. The earlier text is kept in the event log.
    pub fn update_memo(&mut self, id: i64, title: Option<&str>, body: Option<&str>) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let old = tx
            .query_row(&format!("SELECT {MEMO_COLUMNS} FROM memo WHERE id = ?1"), [id], memo_from_row)
            .optional()?
            .ok_or(Error::NotFound { what: "memo", id })?;
        let new_title = title.map(str::trim).unwrap_or(&old.title);
        let new_body = body.unwrap_or(&old.body);
        if new_title == old.title && new_body == old.body {
            return Ok(());
        }
        tx.execute(
            "UPDATE memo SET title = ?1, body = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3",
            params![new_title, new_body, id],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "memo.update",
            Some(("memo", id)),
            Some(&json!({ "title": old.title, "body": old.body })),
            Some(&json!({ "title": new_title, "body": new_body })),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_memo(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let old = tx
            .query_row(&format!("SELECT {MEMO_COLUMNS} FROM memo WHERE id = ?1"), [id], memo_from_row)
            .optional()?
            .ok_or(Error::NotFound { what: "memo", id })?;
        tx.execute("DELETE FROM link WHERE (from_type = 'memo' AND from_id = ?1) OR (to_type = 'memo' AND to_id = ?1)", [id])?;
        tx.execute("DELETE FROM memo WHERE id = ?1", [id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "memo.delete",
            Some(("memo", id)),
            Some(&json!({ "kind": old.kind, "scope": old.scope, "target_id": old.target_id, "title": old.title, "body": old.body })),
            None,
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Memos, optionally only those of one scope, or attached to one thing. Newest first.
    pub fn list_memos(&self, scope: Option<MemoScope>, target_id: Option<i64>) -> Result<Vec<MemoRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {MEMO_COLUMNS} FROM memo
             WHERE (?1 IS NULL OR scope = ?1) AND (?2 IS NULL OR target_id = ?2)
             ORDER BY created_at DESC, id DESC"
        ))?;
        let rows = stmt.query_map(params![scope.map(|s| s.as_str()), target_id], memo_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Connects two things with a label, for example a memo to a coding so a quote travels with its idea.
    pub fn add_link(&mut self, from_type: &str, from_id: i64, to_type: &str, to_id: i64, label: &str) -> Result<i64> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        if !exists(&tx, from_type, from_id)? {
            return Err(Error::NotFound { what: "link start", id: from_id });
        }
        if !exists(&tx, to_type, to_id)? {
            return Err(Error::NotFound { what: "link end", id: to_id });
        }
        tx.execute(
            "INSERT INTO link (from_type, from_id, to_type, to_id, label) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![from_type, from_id, to_type, to_id, label.trim()],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "link.add",
            Some(("link", id)),
            None,
            Some(&json!({ "from": [from_type, from_id], "to": [to_type, to_id], "label": label.trim() })),
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Links that start or end at this thing.
    pub fn list_links(&self, kind: &str, id: i64) -> Result<Vec<LinkRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_type, from_id, to_type, to_id, label FROM link
             WHERE (from_type = ?1 AND from_id = ?2) OR (to_type = ?1 AND to_id = ?2) ORDER BY id",
        )?;
        let rows = stmt.query_map(params![kind, id], |r| {
            Ok(LinkRow { id: r.get(0)?, from_type: r.get(1)?, from_id: r.get(2)?, to_type: r.get(3)?, to_id: r.get(4)?, label: r.get(5)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn delete_link(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let old = tx
            .query_row("SELECT from_type, from_id, to_type, to_id, label FROM link WHERE id = ?1", [id], |r| {
                Ok(json!({ "from": [r.get::<_, String>(0)?, r.get::<_, i64>(1)?], "to": [r.get::<_, String>(2)?, r.get::<_, i64>(3)?], "label": r.get::<_, String>(4)? }))
            })
            .optional()?
            .ok_or(Error::NotFound { what: "link", id })?;
        tx.execute("DELETE FROM link WHERE id = ?1", [id])?;
        event::insert(&tx, &self.actor, &self.session, "link.delete", Some(("link", id)), Some(&old), None)?;
        tx.commit()?;
        Ok(())
    }
}
