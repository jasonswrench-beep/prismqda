//! The Statistics notebook kept in the project (spec 17.3): every analysis on its own page with the author's note beside it, pages of
//! notes, sections, and the way to run the analyses again after the data change, keeping what each said before so the old and the new
//! results can be compared (spec 17.4).
//!
//! A page keeps the request that was run and the result it gave, as JSON, so the notebook can be opened, exported, and run again
//! without the screen that made it. Running a page again uses the project as it is now: the request names variables and the cases to
//! use (or a rule that picks them), and both are read again.

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::event;
use crate::project::Project;
use crate::statistics::StatRequest;
use crate::{Error, Result};

const MAX_TITLE: usize = 120;
const MAX_SECTION: usize = 60;
const MAX_NOTE: usize = 20_000;
/// How many earlier runs of a page are kept.
const KEEP_RUNS: i64 = 20;

/// One page of the notebook.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotebookPage {
    pub id: i64,
    pub position: i64,
    /// `analysis` or `note`.
    pub kind: String,
    pub section: String,
    /// The title of a page of notes (an analysis is titled by what it is).
    pub title: String,
    pub request: Option<Value>,
    pub result: Option<Value>,
    pub ran_at: Option<String>,
    /// The author's note on an analysis, or the body of a page of notes.
    pub note: String,
    /// How many earlier runs are kept.
    pub earlier_runs: usize,
    pub created_at: String,
    pub updated_at: String,
}

/// What running a page again did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rerun {
    pub page: NotebookPage,
    /// What the page said before (the whole result), and when. None when it could not be run again.
    pub before: Option<Value>,
    pub before_at: Option<String>,
    /// True when the new result is not the old one.
    pub changed: bool,
    /// Why it could not be run again (the project no longer has a variable it names, say); the page is then as it was.
    pub error: Option<String>,
}

fn clean(text: &str, max: usize, what: &str) -> Result<String> {
    let text = text.trim();
    if text.chars().count() > max {
        return Err(Error::Invalid(format!("{what} can be at most {max} characters")));
    }
    Ok(text.to_string())
}

const COLUMNS: &str = "p.id, p.position, p.kind, p.section, p.title, p.request_json, p.result_json, p.ran_at, p.note, p.created_at, p.updated_at,
     (SELECT COUNT(*) FROM stat_page_run r WHERE r.page_id = p.id)";

fn page_from(r: &rusqlite::Row) -> rusqlite::Result<NotebookPage> {
    let json = |i: usize| -> rusqlite::Result<Option<Value>> {
        let text: Option<String> = r.get(i)?;
        text.map(|t| serde_json::from_str(&t).map_err(|e| rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, Box::new(e)))).transpose()
    };
    Ok(NotebookPage {
        id: r.get(0)?,
        position: r.get(1)?,
        kind: r.get(2)?,
        section: r.get(3)?,
        title: r.get(4)?,
        request: json(5)?,
        result: json(6)?,
        ran_at: r.get(7)?,
        note: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
        earlier_runs: r.get::<_, i64>(11)? as usize,
    })
}

impl Project {
    /// Every page of the notebook, in order.
    pub fn notebook(&self) -> Result<Vec<NotebookPage>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {COLUMNS} FROM stat_page p ORDER BY p.position, p.id"))?;
        let pages = stmt.query_map([], page_from)?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(pages)
    }

    pub fn notebook_page(&self, id: i64) -> Result<NotebookPage> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM stat_page p WHERE p.id = ?1"), [id], page_from)
            .optional()?
            .ok_or(Error::NotFound { what: "notebook page", id })
    }

    fn next_position(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM stat_page", [], |r| r.get(0))?)
    }

    /// Runs an analysis and puts it in the notebook as a new page, at the end.
    pub fn notebook_run(&mut self, request: &Value, section: &str) -> Result<NotebookPage> {
        self.ensure_writable()?;
        let section = clean(section, MAX_SECTION, "a section's name")?;
        let req: StatRequest = serde_json::from_value(request.clone()).map_err(|e| Error::Invalid(format!("the analysis cannot be run: {e}")))?;
        let result = serde_json::to_value(self.run_statistic(&req)?)?;
        let position = self.next_position()?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO stat_page (position, kind, section, request_json, result_json, ran_at, created_at, updated_at)
             VALUES (?1, 'analysis', ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![position, section, request.to_string(), result.to_string()],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "stats.page.add", Some(("stat_page", id)), None, Some(&json!({ "procedure": request["procedure"] })))?;
        tx.commit()?;
        self.notebook_page(id)
    }

    /// Adds a page of notes at the end.
    pub fn notebook_add_note(&mut self, title: &str, body: &str, section: &str) -> Result<NotebookPage> {
        self.ensure_writable()?;
        let title = clean(title, MAX_TITLE, "a title")?;
        let section = clean(section, MAX_SECTION, "a section's name")?;
        let body = clean(body, MAX_NOTE, "a note")?;
        let position = self.next_position()?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO stat_page (position, kind, section, title, note, created_at, updated_at)
             VALUES (?1, 'note', ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![position, section, title, body],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "stats.page.add", Some(("stat_page", id)), None, Some(&json!({ "kind": "note" })))?;
        tx.commit()?;
        self.notebook_page(id)
    }

    /// Changes a page's note (the body, for a page of notes), its section, or (for a page of notes) its title.
    pub fn notebook_update(&mut self, id: i64, note: Option<&str>, section: Option<&str>, title: Option<&str>) -> Result<NotebookPage> {
        self.ensure_writable()?;
        let before = self.notebook_page(id)?;
        if title.is_some() && before.kind != "note" {
            return Err(Error::Invalid("only a page of notes has a title; an analysis is titled by what it is".to_string()));
        }
        let note = match note {
            Some(n) => clean(n, MAX_NOTE, "a note")?,
            None => before.note.clone(),
        };
        let section = match section {
            Some(s) => clean(s, MAX_SECTION, "a section's name")?,
            None => before.section.clone(),
        };
        let title = match title {
            Some(t) => clean(t, MAX_TITLE, "a title")?,
            None => before.title.clone(),
        };
        if (&note, &section, &title) == (&before.note, &before.section, &before.title) {
            return Ok(before);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE stat_page SET note = ?1, section = ?2, title = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?4",
            params![note, section, title, id],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "stats.page.update",
            Some(("stat_page", id)),
            Some(&json!({ "note_length": before.note.chars().count(), "section": before.section, "title": before.title })),
            Some(&json!({ "note_length": note.chars().count(), "section": section, "title": title })),
        )?;
        tx.commit()?;
        self.notebook_page(id)
    }

    /// Takes a page out of the notebook, with the earlier runs that were kept for it.
    pub fn notebook_remove(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let before = self.notebook_page(id)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM stat_page_run WHERE page_id = ?1", [id])?;
        tx.execute("DELETE FROM stat_page WHERE id = ?1", [id])?;
        event::insert(&tx, &self.actor, &self.session, "stats.page.remove", Some(("stat_page", id)), Some(&json!({ "kind": before.kind, "request": before.request })), None)?;
        tx.commit()?;
        Ok(())
    }

    /// Moves a page one place up or down.
    pub fn notebook_move(&mut self, id: i64, up: bool) -> Result<()> {
        self.ensure_writable()?;
        let pages = self.notebook()?;
        let at = pages.iter().position(|p| p.id == id).ok_or(Error::NotFound { what: "notebook page", id })?;
        let other = if up { at.checked_sub(1) } else { Some(at + 1).filter(|n| *n < pages.len()) };
        let Some(other) = other else { return Ok(()) };
        let (a, b) = (&pages[at], &pages[other]);
        let tx = self.conn.transaction()?;
        // Positions are all different (a page is added after the last, and a removed page leaves a gap), so swapping two is enough.
        tx.execute("UPDATE stat_page SET position = ?1 WHERE id = ?2", params![b.position, a.id])?;
        tx.execute("UPDATE stat_page SET position = ?1 WHERE id = ?2", params![a.position, b.id])?;
        tx.commit()?;
        Ok(())
    }

    /// Runs one analysis again on the project as it is now. The page keeps what it said before as an earlier run (when the new result
    /// differs) and says what it says now. An analysis that cannot be run any more (a variable it names was removed) leaves the page
    /// as it was and says why.
    pub fn notebook_rerun(&mut self, id: i64) -> Result<Rerun> {
        self.ensure_writable()?;
        let page = self.notebook_page(id)?;
        let (Some(request), Some(old)) = (page.request.clone(), page.result.clone()) else {
            return Err(Error::Invalid("a page of notes has nothing to run".to_string()));
        };
        let old_at = page.ran_at.clone();
        let fresh = serde_json::from_value::<StatRequest>(request.clone()).map_err(|e| Error::Invalid(format!("the analysis cannot be run: {e}"))).and_then(|req| self.run_statistic(&req)).and_then(|r| Ok(serde_json::to_value(r)?));
        let new = match fresh {
            Ok(v) => v,
            Err(e) => return Ok(Rerun { page, before: None, before_at: None, changed: false, error: Some(e.to_string()) }),
        };
        // Compared as the project would give it back: a number written out and read in again can differ from the one in memory in its last
        // digit, which is no change in the result.
        let new: Value = serde_json::from_str(&new.to_string())?;
        let changed = new != old;
        let tx = self.conn.transaction()?;
        if changed {
            tx.execute("INSERT INTO stat_page_run (page_id, ran_at, result_json) VALUES (?1, ?2, ?3)", params![id, old_at, old.to_string()])?;
            tx.execute(
                "DELETE FROM stat_page_run WHERE page_id = ?1 AND id NOT IN (SELECT id FROM stat_page_run WHERE page_id = ?1 ORDER BY id DESC LIMIT ?2)",
                params![id, KEEP_RUNS],
            )?;
        }
        tx.execute(
            "UPDATE stat_page SET result_json = ?1, ran_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![new.to_string(), id],
        )?;
        event::insert(&tx, &self.actor, &self.session, "stats.page.rerun", Some(("stat_page", id)), None, Some(&json!({ "changed": changed })))?;
        tx.commit()?;
        Ok(Rerun { page: self.notebook_page(id)?, before: Some(old), before_at: old_at, changed, error: None })
    }

    /// Runs every analysis of the notebook again, in order. A page that cannot be run is reported and the others still run.
    pub fn notebook_rerun_all(&mut self) -> Result<Vec<Rerun>> {
        self.ensure_writable()?;
        let ids: Vec<i64> = self.notebook()?.into_iter().filter(|p| p.kind == "analysis").map(|p| p.id).collect();
        ids.into_iter().map(|id| self.notebook_rerun(id)).collect()
    }

    /// The most recent earlier run of a page (when it ran, and the whole result), if it has been run again with a different result.
    pub fn notebook_previous(&self, id: i64) -> Result<Option<(String, Value)>> {
        self.notebook_page(id)?;
        let row: Option<(String, String)> = self
            .conn
            .query_row("SELECT ran_at, result_json FROM stat_page_run WHERE page_id = ?1 ORDER BY id DESC LIMIT 1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        row.map(|(at, json)| Ok((at, serde_json::from_str(&json)?))).transpose()
    }
}
