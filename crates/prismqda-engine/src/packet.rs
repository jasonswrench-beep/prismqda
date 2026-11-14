//! Coder packets (decision 161): a project file made for another coder to work in, and a way to bring what they coded back.
//!
//! A packet is a new, small project that holds the codebook, the documents chosen, and a memo of instructions, and a coder who stands for
//! the person it is for. It holds none of the study's other documents, memos, notes on the analysis, or earlier codings (unless the
//! sender asks for those, to be reviewed), so a coder works blind on what they were given. The coder opens it in PrismQDA, codes, and
//! sends the file back; the sender brings it in with Team > Merge, which already combines copies of a project and keeps each coder's
//! work with the coder. A coder who is already in the sender's project is the same coder in the packet (the same local id), so a second
//! packet for the same person returns to the same coder.
//!
//! The packet says who it is for in its own audit trail (the `packet.create` event) and not in a new table, so the file format is the
//! same as any project's. While a packet is open, the person using it is the coder it was made for.

use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::event;
use crate::merge::{quiet, remove_scratch, MergeScope};
use crate::ops::sha256_hex;
use crate::project::{NewProject, Project};
use crate::{CoderRole, CodingMode, Error, MemoKind, MemoScope, NewCoder, NewMemo, Result};

/// What goes into a packet.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoderPacketSpec {
    /// The coder the packet is for. A human coder of this project with the same name (any case) is the same coder in the packet.
    pub coder_name: String,
    /// The documents to put in; empty for all of them.
    #[serde(default)]
    pub source_ids: Vec<i64>,
    /// What the coder should do and how, written by the sender. Shown to the coder as a memo.
    #[serde(default)]
    pub instructions: String,
    /// Also bring the codings that are in the project now (and the coders who made them), for a coder to review. Off by default: a coder
    /// who is to code on their own is given none, so the coding is blind.
    #[serde(default)]
    pub include_codings: bool,
}

/// What a packet holds, in sentences and numbers.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CoderPacketReport {
    pub path: String,
    pub coder: String,
    /// True when the coder is one this project already has, so their work comes back to the same coder.
    pub coder_known: bool,
    pub documents: Vec<String>,
    pub codes: usize,
    pub codings: usize,
    /// What the packet does not hold that the person might expect, one line each.
    pub left_out: Vec<String>,
}

/// Who a packet is for, read from the packet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoderPacketInfo {
    pub coder_name: String,
    pub coder_local_id: String,
    /// The name of the project the packet was made from.
    pub from_project: String,
    pub made_at: String,
}

impl Project {
    /// Makes a packet at `path` (which must not exist) from this project. Nothing in this project changes.
    pub fn make_coder_packet(&self, path: &Path, spec: &CoderPacketSpec) -> Result<CoderPacketReport> {
        let coder_name = spec.coder_name.split_whitespace().collect::<Vec<_>>().join(" ");
        if coder_name.is_empty() {
            return Err(Error::Invalid("say who the packet is for".to_string()));
        }
        if coder_name.chars().count() > 100 {
            return Err(Error::Invalid("the coder's name is longer than 100 characters".to_string()));
        }
        if path.exists() {
            return Err(Error::AlreadyExists(path.to_path_buf()));
        }
        let info = self.info()?;
        let all = self.list_sources()?;
        let mut chosen = Vec::new();
        if spec.source_ids.is_empty() {
            chosen.extend(all.iter().map(|s| s.id));
        } else {
            for id in &spec.source_ids {
                if !all.iter().any(|s| s.id == *id) {
                    return Err(Error::NotFound { what: "document", id: *id });
                }
                if !chosen.contains(id) {
                    chosen.push(*id);
                }
            }
        }
        if chosen.is_empty() {
            return Err(Error::Invalid("the project has no document to put in a packet".to_string()));
        }
        // The same person is the same local id, so what comes back lands on the coder who is already here.
        let existing = self.list_coders()?.into_iter().find(|c| c.role == "human" && c.name.trim().to_lowercase() == coder_name.to_lowercase());
        let coder_known = existing.is_some();
        let local_id = match &existing {
            Some(c) => c.local_id.clone(),
            None => format!("packet-{}", &sha256_hex(format!("{}:{}", info.project_uid, coder_name.to_lowercase()).as_bytes())[..12]),
        };
        let coding_mode = CodingMode::from_str(&info.coding_mode).unwrap_or(CodingMode::FreeSpan);
        let new = NewProject { name: format!("{} (packet for {coder_name})", info.name), method_template: "free".to_string(), research_questions: String::new(), coding_mode, sample_id: None };
        let mut packet = Project::create(path, &new, &quiet())?;
        // What the sender decided about AI goes with the packet: the coder cannot turn it into a route the sender closed.
        let sender_locked = self.offline_lock()?;
        let made = (|| -> Result<CoderPacketReport> {
            let scope = MergeScope { sources: Some(chosen.iter().copied().collect::<HashSet<i64>>()), codings: spec.include_codings, memos: false, recordings: false, cases: false, choices: Default::default() };
            let merged = packet.merge_from(self, &scope)?;
            if info.ai_mode != "packet" {
                packet.set_ai_mode(&info.ai_mode)?;
            }
            if sender_locked {
                packet.set_offline_lock(true)?;
            }
            if !packet.list_coders()?.iter().any(|c| c.local_id == local_id) {
                packet.add_coder(&NewCoder { name: coder_name.clone(), role: CoderRole::Human, local_id: Some(local_id.clone()), provider: None, model_name: None, model_version: None })?;
            }
            let given = spec.instructions.trim();
            let body = if given.is_empty() {
                "Code the documents in this project with the codes in the codebook, then close it and send this file back to whoever sent it to you. It is brought into the study with Team > Merge, and your coding stays yours.".to_string()
            } else {
                format!("{given}\n\nWhen you have finished, close the project and send this file back to whoever sent it to you. It is brought into the study with Team > Merge, and your coding stays yours.")
            };
            packet.add_memo(&NewMemo { kind: MemoKind::MethodDecision, scope: MemoScope::Project, target_id: None, phase: None, title: format!("Instructions for {coder_name}"), body })?;
            packet.record_coder_packet(&coder_name, &local_id, &info.name)?;
            let mut left_out = vec![
                "the study's other documents, its memos, its notes on the analysis, and its history".to_string(),
                "recordings (a recording is linked by where it is on the computer, so it does not travel; the coder links their own copy)".to_string(),
            ];
            if !spec.include_codings {
                left_out.push("the codings made so far, so the coder works blind".to_string());
            }
            if !merged.documents_in_conflict.is_empty() {
                left_out.insert(0, format!("{} not put in, because it has the same name as another chosen document with other words: {}", if merged.documents_in_conflict.len() == 1 { "a document was" } else { "documents were" }, merged.documents_in_conflict.join(", ")));
            }
            let omitted = all.len() - chosen.len();
            if omitted > 0 {
                left_out.insert(0, format!("{omitted} document{} of the project (not chosen)", if omitted == 1 { "" } else { "s" }));
            }
            Ok(CoderPacketReport {
                path: path.display().to_string(),
                coder: coder_name.clone(),
                coder_known,
                documents: merged.documents_added,
                codes: merged.codes_added.len(),
                codings: merged.codings_added,
                left_out,
            })
        })();
        drop(packet);
        match made {
            Ok(report) => Ok(report),
            Err(e) => {
                remove_scratch(path);
                Err(e)
            }
        }
    }

    /// Writes the event that says this project is a packet, and for whom.
    fn record_coder_packet(&mut self, coder_name: &str, local_id: &str, from_project: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        event::insert(&tx, &self.actor, &self.session, "packet.create", None, None, Some(&json!({ "coder_name": coder_name, "coder_local_id": local_id, "from_project": from_project })))?;
        tx.commit()?;
        Ok(())
    }

    /// Who this project is a packet for, or `None` when it is an ordinary project.
    pub fn coder_packet_info(&self) -> Result<Option<CoderPacketInfo>> {
        let row: Option<(String, Option<String>)> = self
            .conn
            .query_row("SELECT ts, after_json FROM event WHERE action = 'packet.create' ORDER BY id DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        Ok(row.and_then(|(ts, after)| {
            let v: Value = after.and_then(|s| serde_json::from_str(&s).ok())?;
            Some(CoderPacketInfo {
                coder_name: v["coder_name"].as_str()?.to_string(),
                coder_local_id: v["coder_local_id"].as_str()?.to_string(),
                from_project: v["from_project"].as_str().unwrap_or("").to_string(),
                made_at: ts,
            })
        }))
    }
}
