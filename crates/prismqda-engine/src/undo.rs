//! Undo (specification 8.5): unlimited, within a session, built on the audit trail.
//!
//! Every change is already logged with what it changed and what it was before, so undoing it is
//! reading that record backwards. An undo is itself logged (as one `undo` event naming the change it
//! reversed), so the trail stays complete and honest. Changes go back in reverse order, newest first.
//!
//! Some changes are not reversed by undo: importing a source, editing its text, merging or splitting
//! codes. They are big enough that reversing them blindly could lose work. When the newest change is
//! one of those, undo says so instead of skipping past it, because skipping would make earlier undos
//! act on a project that no longer looks the way they expect.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Value};

use crate::event;
use crate::project::Project;
use crate::{Error, Result};

/// What an undo reversed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UndoOutcome {
    /// The audit event that was reversed.
    pub undone_event: i64,
    pub action: String,
    /// A plain-language description, such as "Removed the coding you added".
    pub what: String,
}

struct Pending {
    id: i64,
    action: String,
    target_id: Option<i64>,
    before: Value,
    after: Value,
}

fn plan(action: &str) -> Option<&'static str> {
    Some(match action {
        "coding.add" => "Removed the coding you added",
        "coding.remove" => "Put back the coding you removed",
        "coding.change_code" => "Changed the coding back to its earlier code",
        "coding.rationale" => "Restored the coding's earlier note",
        "coding.rating" => "Restored the coding's earlier rating",
        "region.add" => "Removed the coded area you added",
        "region.remove" => "Put back the coded area you removed",
        "region.change_code" => "Changed the coded area back to its earlier code",
        "region.reshape" => "Moved the coded area back",
        "region.rationale" => "Restored the coded area's earlier note",
        "region.rating" => "Restored the coded area's earlier rating",
        "anchor.add" => "Took away the anchor example",
        "anchor.remove" => "Made it an anchor example again",
        "counterexample.add" => "Took away the counterexample",
        "counterexample.remove" => "Made it a counterexample again",
        "code.add" => "Removed the code you added",
        "code.update" => "Restored the code's earlier name and definition",
        "code.move" => "Moved the code back",
        "code.retire" => "Brought the retired code back",
        "code.restore" => "Retired the code again",
        "memo.add" => "Removed the memo you added",
        "memo.update" => "Restored the memo's earlier words",
        "memo.delete" => "Put back the memo you deleted",
        "project.offline_lock" => "Changed the offline lock back",
        "link.add" => "Removed the link you added",
        "link.update" => "Restored the link's earlier relation",
        "link.delete" => "Put back the link you deleted",
        "adjudication.set" => "Took back the decision on the unit",
        "adjudication.region.set" => "Took back the decision on the area or stretch",
        "proposal.accept" => "Took back an accepted suggestion",
        "proposal.reject" => "Put a rejected suggestion back in the queue",
        "proposal.create" => "Took back the suggestions that were made",
        "proposal.adjust" => "Restored the suggestion's earlier words and code",
        "query.save" => "Removed the saved query",
        "query.update" => "Restored the saved query's earlier name, note, and question",
        "query.delete" => "Put back the saved query you deleted",
        "query.freeze" => "Took back the freeze of the query",
        "query.unfreeze" => "Froze the query again",
        "project.ai_mode" => "Changed the project's AI setting back",
        "source.share" => "Restored whether the document may be sent to an AI",
        "media.link" => "Took back the link to the recording",
        "media.unlink" => "Linked the recording to the document again",
        "map.create" => "Removed the map you made",
        "map.update" => "Restored the map's earlier name and notes",
        "map.delete" => "Put back the map you deleted, with everything on it",
        "map.node.add" => "Took the node off the map",
        "map.node.move" => "Moved the node back",
        "map.node.update" => "Restored the note's earlier words",
        "map.node.delete" => "Put the node back on the map, with its links",
        "map.link.add" => "Removed the link",
        "map.link.update" => "Restored the link's earlier words",
        "map.link.delete" => "Put the link back",
        "map.layout" => "Moved every node back to where it was",
        "map.add_codes" => "Took the codes off the map",
        "codebook.snapshot" => "Removed the codebook version you saved",
        "case.add" => "Removed the case you added",
        "variable.apply" => "Took back the change to the variable",
        "case.assign" => "Took the source out of the case",
        "case.unassign" => "Put the source back in the case",
        _ => return None,
    })
}

/// Actions that are never the subject of an undo and do not stand in its way.
fn invisible(action: &str) -> bool {
    matches!(action, "annotation" | "undo" | "project.create" | "project.migrate" | "coder.add" | "phase.tick" | "phase.move" | "reliability.run" | "reliability.archive" | "ai.packet" | "ai.answer" | "deid.rule" | "deid.exports" | "project.encrypt" | "project.decrypt" | "project.passphrase" | "project.backups_deleted")
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}

fn cannot(what: &str) -> Error {
    Error::Invalid(format!("{what} cannot be undone any more because later changes depend on it"))
}

impl Project {
    /// Reverses the newest change made in this session. Returns `None` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<Option<UndoOutcome>> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let newest: Option<Pending> = tx
            .query_row(
                "SELECT id, action, target_id, before_json, after_json FROM event
                 WHERE session = ?1
                   AND action NOT IN ('annotation', 'undo', 'project.create', 'project.migrate', 'coder.add', 'phase.tick', 'phase.move', 'reliability.run', 'reliability.archive', 'ai.packet', 'ai.answer', 'deid.rule', 'deid.exports', 'project.encrypt', 'project.decrypt', 'project.passphrase', 'project.backups_deleted')
                   AND json_extract(after_json, '$.via_proposal') IS NULL
                   AND id NOT IN (SELECT CAST(json_extract(after_json, '$.undone_event') AS INTEGER) FROM event WHERE action = 'undo')
                 ORDER BY id DESC LIMIT 1",
                [&self.session],
                |r| {
                    let parse = |s: Option<String>| s.and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
                    Ok(Pending { id: r.get(0)?, action: r.get(1)?, target_id: r.get(2)?, before: parse(r.get(3)?), after: parse(r.get(4)?) })
                },
            )
            .optional()?;
        let Some(p) = newest else { return Ok(None) };
        if invisible(&p.action) {
            return Ok(None);
        }
        let Some(what) = plan(&p.action) else {
            return Err(Error::Invalid(format!(
                "the newest change ({}) is too large to undo automatically; earlier changes cannot be undone past it",
                describe(&p.action)
            )));
        };
        let not_possible = |e: Error| match e {
            Error::NotFound { .. } => cannot(what),
            other => other,
        };
        reverse(&tx, &p).map_err(not_possible)?;
        // Accepting or rejecting several suggestions at once is one change to take back, not one per suggestion.
        let mut also: Vec<Pending> = Vec::new();
        let mut said = what.to_string();
        if let (true, Some(token)) = (matches!(p.action.as_str(), "proposal.accept" | "proposal.reject"), p.after["decision"].as_str()) {
            also = same_decision(&tx, &self.session, token, p.id)?;
            for q in &also {
                reverse(&tx, q).map_err(not_possible)?;
            }
            if !also.is_empty() {
                let n = also.len() + 1;
                said = if p.action == "proposal.accept" { format!("Took back {n} accepted suggestions") } else { format!("Put {n} rejected suggestions back in the queue") };
            }
        }
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "undo",
            None,
            None,
            Some(&json!({ "undone_event": p.id, "undone_action": p.action, "what": said })),
        )?;
        for q in &also {
            event::insert(&tx, &self.actor, &self.session, "undo", None, None, Some(&json!({ "undone_event": q.id, "undone_action": q.action, "with": p.id })))?;
        }
        tx.commit()?;
        Ok(Some(UndoOutcome { undone_event: p.id, action: p.action, what: said }))
    }

    /// What the next undo would do, without doing it.
    pub fn next_undo(&self) -> Result<Option<String>> {
        let row: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT action, after_json FROM event
                 WHERE session = ?1
                   AND action NOT IN ('annotation', 'undo', 'project.create', 'project.migrate', 'coder.add', 'phase.tick', 'phase.move', 'reliability.run', 'reliability.archive', 'ai.packet', 'ai.answer', 'deid.rule', 'deid.exports', 'project.encrypt', 'project.decrypt', 'project.passphrase', 'project.backups_deleted')
                   AND json_extract(after_json, '$.via_proposal') IS NULL
                   AND id NOT IN (SELECT CAST(json_extract(after_json, '$.undone_event') AS INTEGER) FROM event WHERE action = 'undo')
                 ORDER BY id DESC LIMIT 1",
                [&self.session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((action, after)) = row else { return Ok(None) };
        let Some(what) = plan(&action) else {
            return Ok(Some(format!("Cannot undo: {} is too large to undo automatically", describe(&action))));
        };
        // Several suggestions decided together are taken back together, and the answer says how many.
        let token = after.and_then(|a| serde_json::from_str::<Value>(&a).ok()).and_then(|v| v["decision"].as_str().map(str::to_string));
        if let (true, Some(token)) = (matches!(action.as_str(), "proposal.accept" | "proposal.reject"), token) {
            let n: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM event
                 WHERE session = ?1 AND action IN ('proposal.accept', 'proposal.reject') AND json_extract(after_json, '$.decision') = ?2
                   AND id NOT IN (SELECT CAST(json_extract(after_json, '$.undone_event') AS INTEGER) FROM event WHERE action = 'undo')",
                params![self.session, token],
                |r| r.get(0),
            )?;
            if n > 1 {
                return Ok(Some(if action == "proposal.accept" { format!("Took back {n} accepted suggestions") } else { format!("Put {n} rejected suggestions back in the queue") }));
            }
        }
        Ok(Some(what.to_string()))
    }
}

/// The other decisions made in the same call as the one being undone, newest first.
fn same_decision(tx: &Transaction, session: &str, token: &str, except: i64) -> Result<Vec<Pending>> {
    let mut stmt = tx.prepare(
        "SELECT id, action, target_id, before_json, after_json FROM event
         WHERE session = ?1 AND action IN ('proposal.accept', 'proposal.reject') AND id <> ?3 AND json_extract(after_json, '$.decision') = ?2
           AND id NOT IN (SELECT CAST(json_extract(after_json, '$.undone_event') AS INTEGER) FROM event WHERE action = 'undo')
         ORDER BY id DESC",
    )?;
    let rows = stmt.query_map(params![session, token, except], |r| {
        let parse = |s: Option<String>| s.and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
        Ok(Pending { id: r.get(0)?, action: r.get(1)?, target_id: r.get(2)?, before: parse(r.get(3)?), after: parse(r.get(4)?) })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn describe(action: &str) -> &str {
    match action {
        "source.import" => "importing a source",
        "source.edit" => "editing a source's text",
        "source.revise" => "linking a revised file to a document",
        "code.merge" => "merging codes",
        "code.split" => "splitting a code",
        "case.import" => "importing cases",
        "unit.add" => "adding a unit",
        "proposal.create" => "making suggestions",
        "deid.apply" => "replacing names for good",
        other => other,
    }
}

fn reverse(tx: &Transaction, p: &Pending) -> Result<()> {
    let target = p.target_id;
    let need = |id: Option<i64>| id.ok_or(Error::NotFound { what: "target", id: 0 });
    let changed = |n: usize| if n == 0 { Err(Error::NotFound { what: "target", id: 0 }) } else { Ok(()) };
    match p.action.as_str() {
        "coding.add" => {
            let id = need(target)?;
            tx.execute("DELETE FROM code_anchor WHERE coding_id = ?1", [id])?;
            changed(tx.execute("DELETE FROM coding WHERE id = ?1", [id])?)?;
        }
        "coding.remove" => {
            let id = need(target)?;
            let b = &p.before;
            tx.execute(
                "INSERT INTO coding (id, source_id, start_offset, end_offset, code_id, coder_id, origin, layer, rationale, unit_id, rating, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![
                    id,
                    b["source_id"].as_i64(),
                    b["start"].as_i64(),
                    b["end"].as_i64(),
                    b["code_id"].as_i64(),
                    b["coder_id"].as_i64(),
                    text(b, "origin"),
                    if text(b, "layer").is_empty() { "coder".to_string() } else { text(b, "layer") },
                    text(b, "rationale"),
                    b["unit_id"].as_i64(),
                    b["rating"].as_i64()
                ],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
            crate::passage_links::restore_links(tx, &b["links"])?;
        }
        "coding.change_code" => {
            let id = need(target)?;
            // A rating that was cleared by the change comes back with the earlier code.
            changed(tx.execute(
                "UPDATE coding SET code_id = ?1, rating = CASE WHEN ?3 THEN ?2 ELSE rating END WHERE id = ?4",
                params![p.before["code_id"].as_i64(), p.before["rating"].as_i64(), p.before.get("rating").is_some(), id],
            )?)?;
        }
        "coding.rationale" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE coding SET rationale = ?1 WHERE id = ?2", params![text(&p.before, "rationale"), id])?)?;
        }
        "coding.rating" => {
            let id = need(target)?;
            // A missing or null earlier rating goes back to "not rated".
            changed(tx.execute("UPDATE coding SET rating = ?1 WHERE id = ?2", params![p.before["rating"].as_i64(), id])?)?;
        }
        "region.add" => {
            let id = need(target)?;
            changed(tx.execute("DELETE FROM region WHERE id = ?1", [id])?)?;
        }
        "region.remove" => {
            let id = need(target)?;
            let b = &p.before;
            let shape: crate::Shape = serde_json::from_value(b["geometry"].clone()).map_err(|_| Error::NotFound { what: "target", id })?;
            crate::regions::insert_region_row(
                tx,
                Some(id),
                b["source_id"].as_i64().unwrap_or(0),
                b["page"].as_i64().unwrap_or(1),
                &shape,
                b["code_id"].as_i64().unwrap_or(0),
                b["coder_id"].as_i64().unwrap_or(0),
                &text(b, "origin"),
                &if text(b, "layer").is_empty() { "coder".to_string() } else { text(b, "layer") },
                &text(b, "rationale"),
                b["rating"].as_i64(),
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
            crate::passage_links::restore_links(tx, &b["links"])?;
            // the decision on the area, if one was made, comes back with it
            if let Some(d) = b.get("decision").filter(|d| d.is_object()) {
                tx.execute(
                    "INSERT OR IGNORE INTO region_decision (region_id, adjudicator_id, accepted, reason, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, d["adjudicator_id"].as_i64(), d["accepted"].as_bool().unwrap_or(false) as i64, text(d, "reason"), text(d, "created_at")],
                )?;
            }
        }
        "region.change_code" => {
            let id = need(target)?;
            changed(tx.execute(
                "UPDATE region SET code_id = ?1, rating = CASE WHEN ?3 THEN ?2 ELSE rating END WHERE id = ?4",
                params![p.before["code_id"].as_i64(), p.before["rating"].as_i64(), p.before.get("rating").is_some(), id],
            )?)?;
        }
        "region.reshape" => {
            let id = need(target)?;
            let shape: crate::Shape = serde_json::from_value(p.before["geometry"].clone()).map_err(|_| Error::NotFound { what: "target", id })?;
            let (x0, y0, x1, y1) = shape.bbox();
            changed(tx.execute(
                "UPDATE region SET geometry_json = ?1, x0 = ?2, y0 = ?3, x1 = ?4, y1 = ?5 WHERE id = ?6",
                params![shape.canonical_json(), x0, y0, x1, y1, id],
            )?)?;
        }
        "region.rationale" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE region SET rationale = ?1 WHERE id = ?2", params![text(&p.before, "rationale"), id])?)?;
        }
        "region.rating" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE region SET rating = ?1 WHERE id = ?2", params![p.before["rating"].as_i64(), id])?)?;
        }
        "anchor.add" => {
            let id = need(target)?;
            tx.execute("DELETE FROM code_anchor WHERE coding_id = ?1 AND code_id = ?2 AND kind = 'example'", params![id, p.after["code_id"].as_i64()])?;
        }
        "counterexample.add" => {
            let id = need(target)?;
            changed(tx.execute("DELETE FROM code_anchor WHERE coding_id = ?1 AND code_id = ?2 AND kind = 'counter'", params![id, p.after["code_id"].as_i64()])?)?;
        }
        "counterexample.remove" => {
            let id = need(target)?;
            tx.execute("INSERT OR IGNORE INTO code_anchor (code_id, coding_id, kind) VALUES (?1, ?2, 'counter')", params![p.after["code_id"].as_i64(), id])?;
        }
        "anchor.remove" => {
            let id = need(target)?;
            changed(tx.execute("INSERT OR IGNORE INTO code_anchor (code_id, coding_id) SELECT code_id, id FROM coding WHERE id = ?1", [id])?)?;
        }
        "code.add" => {
            let id = need(target)?;
            let used: i64 = tx.query_row(
                "SELECT (SELECT COUNT(*) FROM coding WHERE code_id = ?1) + (SELECT COUNT(*) FROM region WHERE code_id = ?1) + (SELECT COUNT(*) FROM code WHERE parent_id = ?1)",
                [id],
                |r| r.get(0),
            )?;
            if used > 0 {
                return Err(Error::Invalid("the code you added is already in use (it has codings or sub-codes), so it cannot be removed by undo".to_string()));
            }
            tx.execute("DELETE FROM code_set_member WHERE code_id = ?1", [id])?;
            changed(tx.execute("DELETE FROM code WHERE id = ?1", [id])?)?;
        }
        "code.update" => {
            let id = need(target)?;
            let b = &p.before;
            changed(tx.execute(
                "UPDATE code SET name = ?1, definition = ?2, inclusion_criteria = ?3, exclusion_criteria = ?4, color = ?5, kind = ?6, version = ?7,
                                 exclusive_group = COALESCE(?9, exclusive_group), rated = COALESCE(?10, rated),
                                 rating_label = COALESCE(?11, rating_label), rating_low = COALESCE(?12, rating_low), rating_high = COALESCE(?13, rating_high),
                                 ai_suggestible = COALESCE(?14, ai_suggestible), theme_stage = COALESCE(?15, theme_stage)
                 WHERE id = ?8",
                params![
                    text(b, "name"),
                    text(b, "definition"),
                    text(b, "inclusion_criteria"),
                    text(b, "exclusion_criteria"),
                    text(b, "color"),
                    text(b, "kind"),
                    b["version"].as_i64().unwrap_or(1),
                    id,
                    // The properties are in the record from format 3 on; an older record leaves them as they are.
                    b["exclusive_group"].as_str(),
                    b["rated"].as_bool(),
                    b["rating_label"].as_str(),
                    b["rating_low"].as_str(),
                    b["rating_high"].as_str(),
                    b["ai_suggestible"].as_bool().map(|v| v as i64),
                    b["theme_stage"].as_str(),
                ],
            )?)?;
        }
        "code.move" => {
            let id = need(target)?;
            changed(tx.execute(
                "UPDATE code SET parent_id = ?1, sort_order = ?2 WHERE id = ?3",
                params![p.before["parent_id"].as_i64(), p.before["sort_order"].as_i64().unwrap_or(0), id],
            )?)?;
        }
        "code.retire" | "code.restore" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE code SET status = ?1 WHERE id = ?2", params![text(&p.before, "status"), id])?)?;
        }
        "memo.add" => {
            let id = need(target)?;
            tx.execute("DELETE FROM link WHERE (from_type = 'memo' AND from_id = ?1) OR (to_type = 'memo' AND to_id = ?1)", [id])?;
            changed(tx.execute("DELETE FROM memo WHERE id = ?1", [id])?)?;
        }
        "memo.update" => {
            let id = need(target)?;
            changed(tx.execute(
                "UPDATE memo SET title = ?1, body = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3",
                params![text(&p.before, "title"), text(&p.before, "body"), id],
            )?)?;
        }
        "memo.delete" => {
            let id = need(target)?;
            let b = &p.before;
            tx.execute(
                "INSERT INTO memo (id, kind, scope, target_id, title, body, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![id, text(b, "kind"), text(b, "scope"), b["target_id"].as_i64(), text(b, "title"), text(b, "body")],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
        }
        "project.offline_lock" => {
            changed(tx.execute("UPDATE project SET offline_lock = ?1 WHERE id = 1", [p.before["offline_lock"].as_bool().unwrap_or(false) as i64])?)?;
        }
        "link.add" => {
            let id = need(target)?;
            changed(tx.execute("DELETE FROM link WHERE id = ?1", [id])?)?;
        }
        "link.update" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE link SET label = ?1 WHERE id = ?2", params![text(&p.before, "label"), id])?)?;
        }
        "link.delete" => {
            let id = need(target)?;
            let b = &p.before;
            tx.execute(
                "INSERT INTO link (id, from_type, from_id, to_type, to_id, label) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, b["from"][0].as_str().unwrap_or(""), b["from"][1].as_i64(), b["to"][0].as_str().unwrap_or(""), b["to"][1].as_i64(), text(b, "label")],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
        }
        "adjudication.set" => {
            let unit = need(target)?;
            tx.execute("DELETE FROM adjudication_code WHERE adjudication_id IN (SELECT id FROM adjudication WHERE unit_id = ?1)", [unit])?;
            changed(tx.execute("DELETE FROM adjudication WHERE unit_id = ?1", [unit])?)?;
            // A decision that replaced an earlier one puts the earlier one back.
            if !p.before.is_null() {
                let b = &p.before;
                tx.execute(
                    "INSERT INTO adjudication (unit_id, adjudicator_id, reason, created_at) VALUES (?1, ?2, ?3, COALESCE(?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')))",
                    params![unit, b["adjudicator_id"].as_i64(), text(b, "reason"), b["created_at"].as_str()],
                )
                .map_err(|_| Error::NotFound { what: "target", id: unit })?;
                let id = tx.last_insert_rowid();
                for code in b["codes"].as_array().into_iter().flatten().filter_map(Value::as_i64) {
                    tx.execute("INSERT INTO adjudication_code (adjudication_id, code_id) VALUES (?1, ?2)", params![id, code])
                        .map_err(|_| Error::NotFound { what: "target", id: code })?;
                }
            }
        }
        "adjudication.region.set" => {
            let key = need(target)?;
            changed(tx.execute("DELETE FROM region_decision WHERE region_id = ?1", [key])?)?;
            // A decision that replaced an earlier one puts the earlier one back.
            if !p.before.is_null() {
                let b = &p.before;
                tx.execute(
                    "INSERT INTO region_decision (region_id, adjudicator_id, accepted, reason, created_at) VALUES (?1, ?2, ?3, ?4, COALESCE(?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')))",
                    params![key, b["adjudicator_id"].as_i64(), b["accepted"].as_bool().unwrap_or(false) as i64, text(b, "reason"), b["created_at"].as_str()],
                )
                .map_err(|_| Error::NotFound { what: "target", id: key })?;
            }
        }
        "proposal.accept" => {
            let id = need(target)?;
            if let Some(coding) = p.after["coding_id"].as_i64() {
                tx.execute("DELETE FROM code_anchor WHERE coding_id = ?1", [coding])?;
                tx.execute("DELETE FROM coding WHERE id = ?1", [coding])?;
            }
            changed(tx.execute("UPDATE proposal SET status = 'pending', decided_at = NULL, decided_by = NULL WHERE id = ?1", [id])?)?;
        }
        "proposal.reject" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE proposal SET status = 'pending', decided_at = NULL, decided_by = NULL WHERE id = ?1", [id])?)?;
        }
        "proposal.create" => {
            let batch = text(&p.after, "batch");
            let accepted: i64 = tx.query_row("SELECT COUNT(*) FROM proposal WHERE batch = ?1 AND status = 'accepted'", [&batch], |r| r.get(0))?;
            if accepted > 0 {
                return Err(Error::NotFound { what: "target", id: 0 });
            }
            changed(tx.execute("DELETE FROM proposal WHERE batch = ?1", [&batch])?)?;
        }
        "proposal.adjust" => {
            let id = need(target)?;
            let b = &p.before;
            changed(tx.execute(
                "UPDATE proposal SET payload_json = json_set(payload_json, '$.start', ?1, '$.end', ?2, '$.code_id', ?3) WHERE id = ?4 AND status = 'pending'",
                params![b["start"].as_i64(), b["end"].as_i64(), b["code_id"].as_i64(), id],
            )?)?;
        }
        "query.save" => {
            let id = need(target)?;
            changed(tx.execute("DELETE FROM saved_query WHERE id = ?1", [id])?)?;
        }
        "query.update" => {
            let id = need(target)?;
            let b = &p.before;
            changed(tx.execute(
                "UPDATE saved_query SET name = ?1, note = ?2, draft_json = ?3, spec_json = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?5",
                params![text(b, "name"), text(b, "note"), text(b, "draft"), text(b, "spec"), id],
            )?)?;
        }
        "query.delete" => {
            let id = need(target)?;
            let b = &p.before;
            tx.execute(
                "INSERT INTO saved_query (id, name, note, draft_json, spec_json, frozen_at, frozen_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![id, text(b, "name"), text(b, "note"), text(b, "draft"), text(b, "spec"), b["frozen_at"].as_str(), b["frozen_json"].as_str(), text(b, "created_at"), text(b, "updated_at")],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
        }
        "query.freeze" | "query.unfreeze" => {
            let id = need(target)?;
            let b = &p.before;
            changed(tx.execute(
                "UPDATE saved_query SET frozen_at = ?1, frozen_json = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3",
                params![b["frozen_at"].as_str(), b["frozen_json"].as_str(), id],
            )?)?;
        }
        "project.ai_mode" => {
            changed(tx.execute("UPDATE project SET ai_mode = ?1 WHERE id = 1", [text(&p.before, "ai_mode")])?)?;
        }
        "media.link" | "media.unlink" => {
            // The link is put back as it was: taken away when there was none, or the earlier recording linked again.
            let id = need(target)?;
            tx.execute("DELETE FROM media WHERE source_id = ?1", [id])?;
            if p.before.is_object() {
                let b = &p.before;
                tx.execute(
                    "INSERT INTO media (source_id, kind, path, file_name, size_bytes, fingerprint, duration_ms, created_at, stands_alone) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![id, text(b, "kind"), text(b, "path"), text(b, "file_name"), b["size_bytes"].as_i64().unwrap_or(0), text(b, "fingerprint"), b["duration_ms"].as_i64(), text(b, "created_at"), b["stands_alone"].as_bool().unwrap_or(false) as i64],
                )?;
            } else if p.action == "media.unlink" {
                return Err(Error::NotFound { what: "target", id });
            }
        }
        "source.share" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE source SET do_not_share = ?1 WHERE id = ?2", params![p.before["do_not_share"].as_bool().map(|v| v as i64), id])?)?;
        }
        "map.create" => {
            let id = need(target)?;
            let nodes: i64 = tx.query_row("SELECT COUNT(*) FROM map_node WHERE map_id = ?1", [id], |r| r.get(0))?;
            if nodes > 0 {
                return Err(Error::NotFound { what: "target", id });
            }
            changed(tx.execute("DELETE FROM concept_map WHERE id = ?1", [id])?)?;
        }
        "map.update" => {
            let id = need(target)?;
            let b = &p.before;
            changed(tx.execute(
                "UPDATE concept_map SET name = ?1, notes = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3",
                params![text(b, "name"), text(b, "notes"), id],
            )?)?;
        }
        "map.delete" => {
            let id = need(target)?;
            let b = &p.before;
            tx.execute(
                "INSERT INTO concept_map (id, name, notes, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, text(b, "name"), text(b, "notes"), text(b, "created_at"), text(b, "updated_at")],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
            for n in b["nodes"].as_array().into_iter().flatten() {
                tx.execute(
                    "INSERT INTO map_node (id, map_id, kind, ref_id, label, x, y) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![n["id"].as_i64(), id, text(n, "kind"), n["ref_id"].as_i64(), text(n, "label"), n["x"].as_f64(), n["y"].as_f64()],
                )?;
            }
            for l in b["links"].as_array().into_iter().flatten() {
                tx.execute(
                    "INSERT INTO map_link (id, map_id, from_node, to_node, label, directed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![l["id"].as_i64(), id, l["from"].as_i64(), l["to"].as_i64(), text(l, "label"), l["directed"].as_i64()],
                )?;
            }
        }
        "map.node.add" => {
            let id = need(target)?;
            changed(tx.execute("DELETE FROM map_node WHERE id = ?1", [id])?)?;
        }
        "map.node.move" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE map_node SET x = ?1, y = ?2 WHERE id = ?3", params![p.before["x"].as_f64(), p.before["y"].as_f64(), id])?)?;
        }
        "map.node.update" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE map_node SET label = ?1 WHERE id = ?2", params![text(&p.before, "label"), id])?)?;
        }
        "map.node.delete" => {
            let id = need(target)?;
            let b = &p.before;
            tx.execute(
                "INSERT INTO map_node (id, map_id, kind, ref_id, label, x, y) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, b["map_id"].as_i64(), text(b, "kind"), b["ref_id"].as_i64(), text(b, "label"), b["x"].as_f64(), b["y"].as_f64()],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
            for l in b["links"].as_array().into_iter().flatten() {
                tx.execute(
                    "INSERT INTO map_link (id, map_id, from_node, to_node, label, directed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![l["id"].as_i64(), b["map_id"].as_i64(), l["from"].as_i64(), l["to"].as_i64(), text(l, "label"), l["directed"].as_i64()],
                )
                .map_err(|_| Error::NotFound { what: "target", id })?;
            }
        }
        "map.link.add" => {
            let id = need(target)?;
            changed(tx.execute("DELETE FROM map_link WHERE id = ?1", [id])?)?;
        }
        "map.link.update" => {
            let id = need(target)?;
            changed(tx.execute("UPDATE map_link SET label = ?1, directed = ?2 WHERE id = ?3", params![text(&p.before, "label"), p.before["directed"].as_i64(), id])?)?;
        }
        "map.link.delete" => {
            let id = need(target)?;
            let b = &p.before;
            tx.execute(
                "INSERT INTO map_link (id, map_id, from_node, to_node, label, directed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, b["map_id"].as_i64(), b["from"].as_i64(), b["to"].as_i64(), text(b, "label"), b["directed"].as_i64()],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
        }
        "map.layout" => {
            for n in p.before["nodes"].as_array().into_iter().flatten() {
                tx.execute("UPDATE map_node SET x = ?1, y = ?2 WHERE id = ?3", params![n["x"].as_f64(), n["y"].as_f64(), n["id"].as_i64()])?;
            }
        }
        "map.add_codes" => {
            for l in p.after["link_ids"].as_array().into_iter().flatten() {
                tx.execute("DELETE FROM map_link WHERE id = ?1", [l.as_i64()])?;
            }
            for n in p.after["node_ids"].as_array().into_iter().flatten() {
                tx.execute("DELETE FROM map_node WHERE id = ?1", [n.as_i64()])?;
            }
        }
        "codebook.snapshot" => {
            let version = p.after["version"].as_i64();
            changed(tx.execute("DELETE FROM codebook_version WHERE version = ?1", [version])?)?;
        }
        "case.add" => {
            let id = need(target)?;
            // Only an empty case can go: removing one that has since been used would lose that work.
            let used: i64 = tx.query_row(
                "SELECT (SELECT COUNT(*) FROM case_member WHERE case_id = ?1)
                      + (SELECT COUNT(*) FROM memo WHERE scope = 'case' AND target_id = ?1)
                      + (SELECT COUNT(*) FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id
                         WHERE a.applies_to = 'case' AND av.target_id = ?1)",
                [id],
                |r| r.get(0),
            )?;
            if used > 0 {
                return Err(Error::Invalid(
                    "the case you added already has sources, memos, or attribute values in it, so it cannot be removed by undo".to_string(),
                ));
            }
            changed(tx.execute("DELETE FROM research_case WHERE id = ?1", [id])?)?;
        }
        "variable.apply" => {
            let id = need(target)?;
            crate::variables::reverse_apply(tx, id, &p.before, &p.after)?;
        }
        "case.assign" => {
            let id = need(target)?;
            changed(tx.execute("DELETE FROM case_member WHERE case_id = ?1 AND source_id = ?2", params![id, p.after["source_id"].as_i64()])?)?;
        }
        "case.unassign" => {
            let id = need(target)?;
            let source = p.before["source_id"].as_i64();
            // Put it back once, even if it somehow came back already.
            tx.execute(
                "INSERT INTO case_member (case_id, source_id)
                 SELECT ?1, ?2 WHERE NOT EXISTS (SELECT 1 FROM case_member WHERE case_id = ?1 AND source_id = ?2)",
                params![id, source],
            )
            .map_err(|_| Error::NotFound { what: "target", id })?;
        }
        _ => {}
    }
    Ok(())
}
