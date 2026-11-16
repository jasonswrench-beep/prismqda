//! Cases made by hand: a case is a person, site, or other whole that sources belong to. Importing a
//! spreadsheet makes cases too; these operations are for adding one in the interface, and putting
//! sources into it or taking them out. Each change is logged and can be undone.

use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::json;

use crate::event;
use crate::project::Project;
use crate::query::CaseRow;
use crate::{Error, Result};

fn require_case(tx: &Transaction, case_id: i64) -> Result<()> {
    let found: Option<i64> = tx.query_row("SELECT id FROM research_case WHERE id = ?1", [case_id], |r| r.get(0)).optional()?;
    found.map(|_| ()).ok_or(Error::NotFound { what: "case", id: case_id })
}

fn require_source(tx: &Transaction, source_id: i64) -> Result<()> {
    let found: Option<i64> = tx.query_row("SELECT id FROM source WHERE id = ?1", [source_id], |r| r.get(0)).optional()?;
    found.map(|_| ()).ok_or(Error::NotFound { what: "source", id: source_id })
}

fn is_member(tx: &Transaction, case_id: i64, source_id: i64) -> Result<bool> {
    let found: Option<i64> = tx
        .query_row("SELECT 1 FROM case_member WHERE case_id = ?1 AND source_id = ?2 LIMIT 1", params![case_id, source_id], |r| r.get(0))
        .optional()?;
    Ok(found.is_some())
}

impl Project {
    /// Adds a case. The name must not be empty and must not match another case's name, ignoring capitals.
    pub fn add_case(&mut self, name: &str, notes: &str) -> Result<CaseRow> {
        self.ensure_writable()?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a case needs a name".to_string()));
        }
        let tx = self.conn.transaction()?;
        // Compared in Rust rather than SQL, because SQLite only folds the capitals of plain English letters.
        let wanted = name.to_lowercase();
        let existing: Vec<String> = {
            let mut stmt = tx.prepare("SELECT name FROM research_case")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        if let Some(clash) = existing.iter().find(|n| n.trim().to_lowercase() == wanted) {
            return Err(Error::Invalid(format!("there is already a case named '{clash}'")));
        }
        // A case's number is never given to another case afterwards, even when Undo removes the case (project format 16).
        let id = crate::ids::next_id(&tx, crate::ids::Kind::Case)?;
        tx.execute("INSERT INTO research_case (id, name, notes) VALUES (?1, ?2, ?3)", params![id, name, notes])?;
        event::insert(&tx, &self.actor, &self.session, "case.add", Some(("case", id)), None, Some(&json!({ "name": name, "notes": notes })))?;
        tx.commit()?;
        self.get_case(id)
    }

    /// Makes a source a member of a case. A source already in the case stays as it is, and nothing is logged.
    pub fn assign_source_to_case(&mut self, case_id: i64, source_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        require_case(&tx, case_id)?;
        require_source(&tx, source_id)?;
        if is_member(&tx, case_id, source_id)? {
            return Ok(());
        }
        tx.execute("INSERT INTO case_member (case_id, source_id) VALUES (?1, ?2)", params![case_id, source_id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "case.assign",
            Some(("case", case_id)),
            Some(&json!({ "source_id": source_id, "member": false })),
            Some(&json!({ "source_id": source_id, "member": true })),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Takes a source out of a case. A source that is not in it is left alone, and nothing is logged.
    /// Units of the source that belong to the case are not touched.
    pub fn unassign_source_from_case(&mut self, case_id: i64, source_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        require_case(&tx, case_id)?;
        require_source(&tx, source_id)?;
        if !is_member(&tx, case_id, source_id)? {
            return Ok(());
        }
        tx.execute("DELETE FROM case_member WHERE case_id = ?1 AND source_id = ?2", params![case_id, source_id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "case.unassign",
            Some(("case", case_id)),
            Some(&json!({ "source_id": source_id, "member": true })),
            Some(&json!({ "source_id": source_id, "member": false })),
        )?;
        tx.commit()?;
        Ok(())
    }
}
