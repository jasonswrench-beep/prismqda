//! The offline lock and the facts an ethics summary states (decision 165).
//!
//! The offline lock is a switch in the project (the `offline_lock` column, there since format 1) that says this project must not send what it
//! holds out of the computer through PrismQDA. PrismQDA has one network feature, the Check for updates button of the desktop shell (decision 189), which sends no
//! part of a project and is refused while the lock is on. The lock is also what keeps the one place a person could send a project's words onward
//! closed: a prompt packet for an AI service is refused while it is on. Anything that ever reaches over a network must ask `offline_lock()` first.
//!
//! The ethics summary is the facts that a protocol, an amendment, or an ethics board asks for, read from the project and nowhere else: where
//! it is kept, whether it is encrypted, whether the lock is on, what was shared with an AI service (nothing is ever sent by PrismQDA; a packet is
//! text the person takes, and the log says when each was built and answered), what is marked do not share, whether names were replaced, who
//! coded, and the length of the audit trail.

use std::path::PathBuf;

use rusqlite::params;
use serde::Serialize;
use serde_json::json;

use crate::event;
use crate::{AiRunRow, DeidApplied, Error, Project, Result};

/// One document or code that is marked do not share.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotShared {
    pub kind: String,
    pub name: String,
}

/// A coder, with whether they are a person or an AI model.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoderFact {
    pub name: String,
    pub role: String,
}

/// What an ethics summary says, from the project.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrivacyFacts {
    pub project_name: String,
    pub project_path: PathBuf,
    pub backup_folder: Option<PathBuf>,
    pub created_at: String,
    pub encrypted: bool,
    pub offline_lock: bool,
    /// `packet` (prompt packets can be built) or `off`.
    pub ai_mode: String,
    pub documents: i64,
    pub units: i64,
    pub codes: i64,
    /// Documents and codes marked do not share, by name.
    pub not_shared: Vec<NotShared>,
    /// Units (passages of an imported survey, thread, or interview) marked do not share.
    pub units_not_shared: i64,
    pub names_rules: i64,
    pub names_applied_to_exports: bool,
    pub names_last_applied: Option<DeidApplied>,
    pub coders: Vec<CoderFact>,
    /// Every prompt packet that was built, and whether and how it was answered.
    pub ai_runs: Vec<AiRunRow>,
    pub events: i64,
    pub first_event_at: Option<String>,
    pub last_event_at: Option<String>,
}

impl Project {
    /// True when the offline lock is on.
    pub fn offline_lock(&self) -> Result<bool> {
        let on: i64 = self.conn.query_row("SELECT offline_lock FROM project WHERE id = 1", [], |r| r.get(0))?;
        Ok(on != 0)
    }

    /// Turns the offline lock on or off. The change is in the audit trail and Undo takes it back.
    pub fn set_offline_lock(&mut self, on: bool) -> Result<()> {
        self.ensure_writable()?;
        let before = self.offline_lock()?;
        if before == on {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE project SET offline_lock = ?1 WHERE id = 1", params![on as i64])?;
        event::insert(&tx, &self.actor, &self.session, "project.offline_lock", None, Some(&json!({ "offline_lock": before })), Some(&json!({ "offline_lock": on })))?;
        tx.commit()?;
        Ok(())
    }

    /// Refuses with a sentence when the offline lock is on; for the one thing that would send the project's words onward.
    pub(crate) fn refuse_if_offline(&self, what: &str) -> Result<()> {
        if self.offline_lock()? {
            return Err(Error::Invalid(format!("the offline lock is on for this project, so {what}; turn the lock off in Privacy if you want to")));
        }
        Ok(())
    }

    /// The facts an ethics summary states.
    pub fn privacy_facts(&self) -> Result<PrivacyFacts> {
        let info = self.info()?;
        let mut not_shared = Vec::new();
        for s in self.list_sources()?.into_iter().filter(|s| s.do_not_share) {
            not_shared.push(NotShared { kind: "document".to_string(), name: s.name });
        }
        for c in self.list_codes()?.into_iter().filter(|c| c.do_not_share) {
            not_shared.push(NotShared { kind: "code".to_string(), name: c.name });
        }
        let units_not_shared: i64 = self.conn.query_row("SELECT COUNT(*) FROM unit WHERE do_not_share = 1", [], |r| r.get(0))?;
        let deid = self.deid_state()?;
        let (events, first, last): (i64, Option<String>, Option<String>) =
            self.conn.query_row("SELECT COUNT(*), MIN(ts), MAX(ts) FROM event", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let created_at: String = self.conn.query_row("SELECT created_at FROM project WHERE id = 1", [], |r| r.get(0))?;
        Ok(PrivacyFacts {
            project_name: info.name.clone(),
            project_path: info.path.clone(),
            backup_folder: self.backup_folder().ok(),
            created_at,
            encrypted: info.encrypted,
            offline_lock: self.offline_lock()?,
            ai_mode: info.ai_mode.clone(),
            documents: info.sources,
            units: info.units,
            codes: info.codes,
            not_shared,
            units_not_shared,
            names_rules: deid.rules.len() as i64,
            names_applied_to_exports: deid.exports,
            names_last_applied: deid.last_applied,
            coders: self.list_coders()?.into_iter().map(|c| CoderFact { name: c.name, role: c.role }).collect(),
            ai_runs: self.ai_runs()?,
            events,
            first_event_at: first,
            last_event_at: last,
        })
    }
}
