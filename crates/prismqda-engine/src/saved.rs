//! Saved queries (Explore, Query Builder): a research question kept under a name, run again whenever it is opened, or
//! frozen so that its answer stops changing as the coding grows. Freezing keeps the numbers of the codings the query found
//! at that moment; looking at a frozen query looks at those codings as they are now (a coding that was removed since is
//! gone from the answer, and the comparison says so).

use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::event;
use crate::explore::{validate_spec, Node, QueryResult, QuerySpec};
use crate::project::Project;
use crate::{Error, Result};

const MAX_NAME: usize = 80;
/// How many numbers of added codings a comparison lists (the count is always whole).
const LISTED: usize = 200;

/// One saved query in a list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SavedQueryRow {
    pub id: i64,
    pub name: String,
    pub note: String,
    pub frozen_at: Option<String>,
    /// How many codings it found when it was frozen.
    pub frozen_count: Option<usize>,
    pub created_at: String,
    pub updated_at: String,
}

/// A saved query with what is needed to open it again.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SavedQuery {
    pub id: i64,
    pub name: String,
    pub note: String,
    /// The query as the interface built it.
    pub draft: Value,
    /// The query as the engine runs it.
    pub spec: Value,
    pub frozen_at: Option<String>,
    pub frozen_count: Option<usize>,
    pub created_at: String,
    pub updated_at: String,
}

/// What freezing a query kept.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Frozen {
    pub frozen_at: String,
    /// How many codings it found.
    pub codings: usize,
    /// How many results those make (distinct excerpts, or assignments, as the query asks).
    pub results: usize,
}

/// A frozen query set beside what the same question finds now.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FrozenComparison {
    pub frozen_at: String,
    /// Codings it found when frozen.
    pub frozen: usize,
    /// Codings the question finds now.
    pub now: usize,
    /// Found then and now.
    pub still_found: usize,
    /// Found now and not then (the first ones are listed).
    pub added: usize,
    pub added_ids: Vec<i64>,
    /// Found then and not now.
    pub no_longer_found: usize,
    /// Of those, how many codings no longer exist at all (they were removed).
    pub removed: usize,
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("give the query a name".to_string()));
    }
    if name.chars().count() > MAX_NAME {
        return Err(Error::Invalid(format!("a query's name can be at most {MAX_NAME} characters")));
    }
    Ok(name.to_string())
}

/// The spec as the engine runs it, refused when it could not be run.
fn read_spec(spec: &Value) -> Result<QuerySpec> {
    let parsed: QuerySpec = serde_json::from_value(spec.clone()).map_err(|e| Error::Invalid(format!("the query cannot be run: {e}")))?;
    validate_spec(&parsed)?;
    Ok(parsed)
}

fn frozen_count(frozen_json: Option<&str>) -> Option<usize> {
    let v: Value = serde_json::from_str(frozen_json?).ok()?;
    v["coding_ids"].as_array().map(Vec::len)
}

const COLUMNS: &str = "id, name, note, draft_json, spec_json, frozen_at, frozen_json, created_at, updated_at";

type Raw = (i64, String, String, String, String, Option<String>, Option<String>, String, String);

fn raw_from(r: &rusqlite::Row) -> rusqlite::Result<Raw> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?))
}

fn saved_from(raw: Raw) -> Result<SavedQuery> {
    Ok(SavedQuery {
        id: raw.0,
        name: raw.1,
        note: raw.2,
        draft: serde_json::from_str(&raw.3)?,
        spec: serde_json::from_str(&raw.4)?,
        frozen_count: frozen_count(raw.6.as_deref()),
        frozen_at: raw.5,
        created_at: raw.7,
        updated_at: raw.8,
    })
}

/// Everything about a saved query as one value, to put back after a delete is undone.
fn snapshot(raw: &Raw) -> Value {
    json!({
        "id": raw.0, "name": raw.1, "note": raw.2, "draft": raw.3, "spec": raw.4, "frozen_at": raw.5, "frozen_json": raw.6,
        "created_at": raw.7, "updated_at": raw.8,
    })
}

impl Project {
    fn raw_saved_query(&self, id: i64) -> Result<Raw> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM saved_query WHERE id = ?1"), [id], raw_from)
            .optional()?
            .ok_or(Error::NotFound { what: "saved query", id })
    }

    fn name_is_free(&self, name: &str, except: Option<i64>) -> Result<bool> {
        let taken: Option<i64> =
            self.conn.query_row("SELECT id FROM saved_query WHERE lower(name) = lower(?1) AND id IS NOT ?2", params![name, except], |r| r.get(0)).optional()?;
        Ok(taken.is_none())
    }

    /// Keeps a query under a name. `draft` is the query as the interface builds it, `spec` the same query as the engine runs it;
    /// a query that cannot be run is refused.
    pub fn save_query(&mut self, name: &str, note: &str, draft: &Value, spec: &Value) -> Result<i64> {
        self.ensure_writable()?;
        let name = clean_name(name)?;
        read_spec(spec)?;
        if !self.name_is_free(&name, None)? {
            return Err(Error::Invalid(format!("there is already a saved query named '{name}'")));
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO saved_query (name, note, draft_json, spec_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![name, note.trim(), draft.to_string(), spec.to_string()],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "query.save", Some(("saved_query", id)), None, Some(&json!({ "name": name })))?;
        tx.commit()?;
        Ok(id)
    }

    /// Changes a saved query's name or note, or what it asks (`draft` and `spec` together). A frozen query's question cannot
    /// be changed: freezing keeps the answer to that question, so unfreeze it first.
    pub fn update_saved_query(&mut self, id: i64, name: Option<&str>, note: Option<&str>, question: Option<(&Value, &Value)>) -> Result<SavedQuery> {
        self.ensure_writable()?;
        let before = self.raw_saved_query(id)?;
        let new_name = match name {
            Some(n) => clean_name(n)?,
            None => before.1.clone(),
        };
        if new_name != before.1 && !self.name_is_free(&new_name, Some(id))? {
            return Err(Error::Invalid(format!("there is already a saved query named '{new_name}'")));
        }
        let new_note = note.map_or_else(|| before.2.clone(), |n| n.trim().to_string());
        let (draft, spec) = match question {
            Some((d, s)) => {
                if before.5.is_some() {
                    return Err(Error::Invalid("this query is frozen, so what it asks cannot be changed; unfreeze it first".to_string()));
                }
                read_spec(s)?;
                (d.to_string(), s.to_string())
            }
            None => (before.3.clone(), before.4.clone()),
        };
        if (&new_name, &new_note, &draft, &spec) == (&before.1, &before.2, &before.3, &before.4) {
            return saved_from(before);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE saved_query SET name = ?1, note = ?2, draft_json = ?3, spec_json = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?5",
            params![new_name, new_note, draft, spec, id],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "query.update",
            Some(("saved_query", id)),
            Some(&json!({ "name": before.1, "note": before.2, "draft": before.3, "spec": before.4 })),
            Some(&json!({ "name": new_name, "note": new_note, "draft": draft, "spec": spec })),
        )?;
        tx.commit()?;
        self.get_saved_query(id)
    }

    pub fn delete_saved_query(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let before = self.raw_saved_query(id)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM saved_query WHERE id = ?1", [id])?;
        event::insert(&tx, &self.actor, &self.session, "query.delete", Some(("saved_query", id)), Some(&snapshot(&before)), None)?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_saved_queries(&self) -> Result<Vec<SavedQueryRow>> {
        let mut stmt = self.conn.prepare("SELECT id, name, note, frozen_at, frozen_json, created_at, updated_at FROM saved_query ORDER BY lower(name)")?;
        let rows = stmt.query_map([], |r| {
            let frozen_json: Option<String> = r.get(4)?;
            Ok(SavedQueryRow {
                id: r.get(0)?,
                name: r.get(1)?,
                note: r.get(2)?,
                frozen_at: r.get(3)?,
                frozen_count: frozen_count(frozen_json.as_deref()),
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_saved_query(&self, id: i64) -> Result<SavedQuery> {
        saved_from(self.raw_saved_query(id)?)
    }

    /// Freezes a query: runs it now and keeps the numbers of the codings it found. Freezing again takes a new picture.
    pub fn freeze_saved_query(&mut self, id: i64) -> Result<Frozen> {
        self.ensure_writable()?;
        let before = self.raw_saved_query(id)?;
        let spec = read_spec(&serde_json::from_str(&before.4)?)?;
        let (ids, results) = self.matching_coding_ids(&spec)?;
        let frozen_json = json!({ "coding_ids": ids, "results": results }).to_string();
        let tx = self.conn.transaction()?;
        let at: String = tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| r.get(0))?;
        tx.execute("UPDATE saved_query SET frozen_at = ?1, frozen_json = ?2, updated_at = ?1 WHERE id = ?3", params![at, frozen_json, id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "query.freeze",
            Some(("saved_query", id)),
            Some(&json!({ "frozen_at": before.5, "frozen_json": before.6 })),
            Some(&json!({ "frozen_at": at, "codings": ids.len() })),
        )?;
        tx.commit()?;
        Ok(Frozen { frozen_at: at, codings: ids.len(), results })
    }

    /// Lets a frozen query follow the coding again.
    pub fn unfreeze_saved_query(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let before = self.raw_saved_query(id)?;
        if before.5.is_none() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE saved_query SET frozen_at = NULL, frozen_json = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [id])?;
        event::insert(&tx, &self.actor, &self.session, "query.unfreeze", Some(("saved_query", id)), Some(&json!({ "frozen_at": before.5, "frozen_json": before.6 })), None)?;
        tx.commit()?;
        Ok(())
    }

    /// The numbers of the codings a frozen query found, ascending.
    fn frozen_coding_ids(&self, raw: &Raw) -> Result<Option<Vec<i64>>> {
        let Some(json) = &raw.6 else { return Ok(None) };
        let v: Value = serde_json::from_str(json)?;
        let mut ids: Vec<i64> = v["coding_ids"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
        ids.sort_unstable();
        ids.dedup();
        Ok(Some(ids))
    }

    /// Opens a saved query: a frozen one shows the codings it found when it was frozen (as they are now), any other is run again.
    pub fn saved_query_result(&self, id: i64) -> Result<(SavedQuery, QueryResult)> {
        let raw = self.raw_saved_query(id)?;
        let mut spec = read_spec(&serde_json::from_str(&raw.4)?)?;
        if let Some(ids) = self.frozen_coding_ids(&raw)? {
            spec.rule = Node::CodingIds { ids, negate: false };
        }
        let result = self.run_query(&spec)?;
        Ok((saved_from(raw)?, result))
    }

    /// What a frozen query found then, beside what the same question finds now.
    pub fn compare_frozen_query(&self, id: i64) -> Result<FrozenComparison> {
        let raw = self.raw_saved_query(id)?;
        let frozen = self.frozen_coding_ids(&raw)?.ok_or_else(|| Error::Invalid("this query is not frozen, so there is nothing to compare".to_string()))?;
        let spec = read_spec(&serde_json::from_str(&raw.4)?)?;
        let (now, _) = self.matching_coding_ids(&spec)?;
        let then: HashSet<i64> = frozen.iter().copied().collect();
        let current: HashSet<i64> = now.iter().copied().collect();
        let added: Vec<i64> = now.iter().copied().filter(|i| !then.contains(i)).collect();
        let gone: Vec<i64> = frozen.iter().copied().filter(|i| !current.contains(i)).collect();
        let mut exist = self.conn.prepare("SELECT id FROM coding")?;
        let mut existing: HashSet<i64> = exist.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        // A coded area of a picture is numbered as its own number made negative.
        let mut areas = self.conn.prepare("SELECT id FROM region")?;
        for row in areas.query_map([], |r| r.get::<_, i64>(0))? {
            existing.insert(-row?);
        }
        Ok(FrozenComparison {
            frozen_at: raw.5.unwrap_or_default(),
            frozen: frozen.len(),
            now: now.len(),
            still_found: frozen.len() - gone.len(),
            added: added.len(),
            added_ids: added.into_iter().take(LISTED).collect(),
            no_longer_found: gone.len(),
            removed: gone.iter().filter(|i| !existing.contains(i)).count(),
        })
    }
}
