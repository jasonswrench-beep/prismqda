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

/// The pages of the program an instructor can close to a student (the others, the coding page and the settings, are always open).
pub const CLOSABLE_PAGES: [&str; 14] = [
    "data", "import", "codebook", "themes", "explore", "autocode", "ai", "team", "text", "stats", "variables", "mixed", "synthesize", "report",
];

fn yes() -> bool {
    true
}

/// What an instructor settled for an assignment (decision 200), kept in the packet each student is given: whether AI prompt packets are
/// allowed, whether the coach stays on, and which pages of the program are closed. It is a classroom convention, not a lock against a
/// determined student: the file is the student's, and a program that opens it can change what is in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    /// What the assignment is called (shown to the student); may be empty.
    #[serde(default)]
    pub title: String,
    /// False closes AI prompt packets in the packet, and the program will not open them again there.
    #[serde(default = "yes")]
    pub ai_allowed: bool,
    /// True keeps the coach on while the student codes.
    #[serde(default)]
    pub coach_locked: bool,
    /// The pages (see [`CLOSABLE_PAGES`]) closed to the student.
    #[serde(default)]
    pub closed: Vec<String>,
}

impl Default for Assignment {
    fn default() -> Self {
        Assignment { title: String::new(), ai_allowed: true, coach_locked: false, closed: Vec::new() }
    }
}

impl Assignment {
    /// Tidies the settings and refuses what makes no sense: a page that is not one of those that can be closed, or the Team page (which holds
    /// the coach) closed while the coach is kept on.
    pub fn checked(mut self) -> Result<Assignment> {
        self.title = self.title.split_whitespace().collect::<Vec<_>>().join(" ");
        if self.title.chars().count() > 100 {
            return Err(Error::Invalid("the name of the assignment is longer than 100 characters".to_string()));
        }
        let mut seen = HashSet::new();
        let mut closed = Vec::new();
        for page in &self.closed {
            let page = page.trim().to_lowercase();
            if !CLOSABLE_PAGES.contains(&page.as_str()) {
                return Err(Error::Invalid(format!("'{page}' is not a page that can be closed (the coding page and the settings stay open)")));
            }
            if seen.insert(page.clone()) {
                closed.push(page);
            }
        }
        if self.coach_locked && closed.iter().any(|p| p == "team") {
            return Err(Error::Invalid("the coach is on the Team page, so that page cannot be closed while the coach is kept on".to_string()));
        }
        self.closed = closed;
        Ok(self)
    }
}

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
    /// What the instructor settled for the assignment (decision 200); none for an ordinary packet.
    #[serde(default)]
    pub assignment: Option<Assignment>,
    /// Bring only this coder's coding (an answer key, decision 200); not something a caller outside the engine chooses.
    #[serde(skip)]
    pub(crate) keep_coder: Option<i64>,
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
    /// What the instructor settled for the assignment, when the packet was made with settings.
    pub assignment: Option<Assignment>,
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
        let assignment = match &spec.assignment {
            Some(a) => Some(a.clone().checked()?),
            None => None,
        };
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
        let is_key = spec.keep_coder.is_some();
        let new = NewProject { name: if is_key { format!("{} (answer key)", info.name) } else { format!("{} (packet for {coder_name})", info.name) }, method_template: "free".to_string(), research_questions: String::new(), coding_mode, sample_id: None };
        let mut packet = Project::create(path, &new, &quiet())?;
        // What the sender decided about AI goes with the packet: the coder cannot turn it into a route the sender closed.
        let sender_locked = self.offline_lock()?;
        let made = (|| -> Result<CoderPacketReport> {
            let scope = MergeScope { sources: Some(chosen.iter().copied().collect::<HashSet<i64>>()), codings: spec.include_codings, coders: spec.keep_coder.map(|id| HashSet::from([id])), memos: false, recordings: false, cases: false, choices: Default::default() };
            let merged = packet.merge_from(self, &scope)?;
            if info.ai_mode != "packet" {
                packet.set_ai_mode(&info.ai_mode)?;
            }
            // An assignment that does not allow AI closes it in the packet before the settings are written down, so the closing is not refused.
            if assignment.as_ref().is_some_and(|a| !a.ai_allowed) {
                packet.set_ai_mode("off")?;
            }
            if sender_locked {
                packet.set_offline_lock(true)?;
            }
            if !packet.list_coders()?.iter().any(|c| c.local_id == local_id) {
                packet.add_coder(&NewCoder { name: coder_name.clone(), role: CoderRole::Human, local_id: Some(local_id.clone()), provider: None, model_name: None, model_version: None })?;
            }
            let given = spec.instructions.trim();
            let settings = match &assignment {
                Some(a) => {
                    let mut lines = vec!["The instructor's settings for this assignment:".to_string()];
                    lines.push(if a.ai_allowed { "- AI prompt packets are allowed.".to_string() } else { "- AI prompt packets are turned off.".to_string() });
                    if a.coach_locked {
                        lines.push("- The coach (Team > Coach) stays on while you code, and asks questions about your own coding.".to_string());
                    }
                    if !a.closed.is_empty() {
                        lines.push(format!("- Closed for this assignment: {}.", a.closed.join(", ")));
                    }
                    format!("\n\n{}", lines.join("\n"))
                }
                None => String::new(),
            };
            let body = if is_key {
                "This file holds the instructor's coding of the documents in it, and nothing else: no student's coding, no memo, no note. Keep it apart from the files students are given. To use it again, put it in the folder with the students' packets and bring them all in with Class > Bring the students' work in.".to_string()
            } else if given.is_empty() {
                "Code the documents in this project with the codes in the codebook, then close it and send this file back to whoever sent it to you. It is brought into the study with Team > Merge, and your coding stays yours.".to_string()
            } else {
                format!("{given}\n\nWhen you have finished, close the project and send this file back to whoever sent it to you. It is brought into the study with Team > Merge, and your coding stays yours.")
            };
            packet.add_memo(&NewMemo { kind: MemoKind::MethodDecision, scope: MemoScope::Project, target_id: None, phase: None, title: if is_key { format!("Answer key from {coder_name}") } else { format!("Instructions for {coder_name}") }, body: format!("{body}{settings}") })?;
            packet.record_coder_packet(&coder_name, &local_id, &info.name, assignment.as_ref())?;
            let mut left_out = vec![
                "the study's other documents, its memos, its notes on the analysis, and its history".to_string(),
                "recordings (a recording is linked by where it is on the computer, so it does not travel; the coder links their own copy)".to_string(),
            ];
            if !spec.include_codings {
                left_out.push("the codings made so far, so the coder works blind".to_string());
            }
            if is_key {
                left_out.push("every other coder's coding".to_string());
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
    fn record_coder_packet(&mut self, coder_name: &str, local_id: &str, from_project: &str, assignment: Option<&Assignment>) -> Result<()> {
        let tx = self.conn.transaction()?;
        let mut after = json!({ "coder_name": coder_name, "coder_local_id": local_id, "from_project": from_project });
        if let Some(a) = assignment {
            after["assignment"] = serde_json::to_value(a).unwrap_or(Value::Null);
        }
        event::insert(&tx, &self.actor, &self.session, "packet.create", None, None, Some(&after))?;
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
                assignment: serde_json::from_value::<Assignment>(v["assignment"].clone()).ok(),
            })
        }))
    }
}
