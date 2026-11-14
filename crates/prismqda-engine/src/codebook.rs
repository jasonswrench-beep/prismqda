//! The codebook (specification 9): a tree of codes, each with a full definition, edited without
//! ever losing history. Merging and retiring keep every old coding and log the mapping.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::event::{self, HistoryEntry};
use crate::ops::{NewCode, NewCoder};
use crate::project::Project;
use crate::{Error, Result};

/// One code with how often it is used.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeRow {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    /// `code`, or `theme` for a code that groups others.
    pub kind: String,
    pub color: String,
    pub definition: String,
    pub inclusion_criteria: String,
    pub exclusion_criteria: String,
    /// `active`, `merged`, or `retired`.
    pub status: String,
    pub merged_into_id: Option<i64>,
    /// Counts up each time the code's name or definition changes.
    pub version: i64,
    pub sort_order: i64,
    pub do_not_share: bool,
    /// The mutually exclusive group the code belongs to, or empty when it is in none. Two codes of one group
    /// cannot be given by the same coder to overlapping words.
    pub exclusive_group: String,
    /// True when codings of this code can be rated from 1 to 5.
    pub rated: bool,
    /// What the rating measures ("Intensity"), and what a 1 and a 5 mean. All three may be empty.
    pub rating_label: String,
    pub rating_low: String,
    pub rating_high: String,
    /// False when no AI suggestion may use this code (it is left out of every packet).
    pub ai_suggestible: bool,
    /// For a theme: `candidate`, `reviewed`, or `defined` (decision 167). A code that is not a theme stays at `candidate`.
    pub theme_stage: String,
    /// Codings that use this code.
    pub usage: i64,
}

/// Changes to a code. Fields left as `None` stay as they are.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CodeEdit {
    pub name: Option<String>,
    pub color: Option<String>,
    pub definition: Option<String>,
    pub inclusion_criteria: Option<String>,
    pub exclusion_criteria: Option<String>,
    pub theme: Option<bool>,
    pub do_not_share: Option<bool>,
    /// The group name; an empty text takes the code out of its group.
    pub exclusive_group: Option<String>,
    /// Turns rating on or off. Turning it off while codings carry ratings is refused unless `clear_ratings`
    /// says the ratings may go.
    pub rated: Option<bool>,
    pub rating_label: Option<String>,
    pub rating_low: Option<String>,
    pub rating_high: Option<String>,
    #[serde(default)]
    pub clear_ratings: bool,
    /// Whether an AI suggestion may use this code.
    pub ai_suggestible: Option<bool>,
    /// Where a theme stands: `candidate`, `reviewed`, or `defined`.
    pub theme_stage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoderRow {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub local_id: String,
    pub provider: Option<String>,
    pub model_name: Option<String>,
    pub model_version: Option<String>,
}

/// One coding, as listed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodingRow {
    pub id: i64,
    pub source_id: i64,
    pub start: i64,
    pub end: i64,
    pub code_id: i64,
    pub coder_id: i64,
    pub origin: String,
    pub layer: String,
    pub rationale: String,
    /// A rating from 1 to 5, or `None` (JSON `null`) when the coding has not been rated. "Not rated"
    /// is its own state, never a number.
    pub rating: Option<i64>,
    pub unit_id: Option<i64>,
    /// When the coding was made, as the engine recorded it (UTC).
    pub created_at: String,
    /// The AI run whose suggestion this coding came from, for a coding made by accepting an AI suggestion.
    pub ai_run_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeOutcome {
    pub codings_moved: usize,
    pub children_moved: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodebookVersion {
    pub version: i64,
    pub note: String,
    pub codes: usize,
    pub created_at: String,
}

const CODE_COLUMNS: &str = "c.id, c.name, c.parent_id, c.kind, c.color, c.definition, c.inclusion_criteria, c.exclusion_criteria,
     c.status, c.merged_into_id, c.version, c.sort_order, c.do_not_share,
     c.exclusive_group, c.rated, c.rating_label, c.rating_low, c.rating_high, c.ai_suggestible, c.theme_stage,
     (SELECT COUNT(*) FROM coding g WHERE g.code_id = c.id) + (SELECT COUNT(*) FROM region rg WHERE rg.code_id = c.id)";

fn code_from_row(r: &rusqlite::Row) -> rusqlite::Result<CodeRow> {
    Ok(CodeRow {
        id: r.get(0)?,
        name: r.get(1)?,
        parent_id: r.get(2)?,
        kind: r.get(3)?,
        color: r.get(4)?,
        definition: r.get(5)?,
        inclusion_criteria: r.get(6)?,
        exclusion_criteria: r.get(7)?,
        status: r.get(8)?,
        merged_into_id: r.get(9)?,
        version: r.get(10)?,
        sort_order: r.get(11)?,
        do_not_share: r.get::<_, i64>(12)? != 0,
        exclusive_group: r.get(13)?,
        rated: r.get::<_, i64>(14)? != 0,
        rating_label: r.get(15)?,
        rating_low: r.get(16)?,
        rating_high: r.get(17)?,
        ai_suggestible: r.get::<_, i64>(18)? != 0,
        theme_stage: r.get(19)?,
        usage: r.get(20)?,
    })
}

fn code_json(c: &CodeRow) -> Value {
    json!({
        "name": c.name, "parent_id": c.parent_id, "kind": c.kind, "color": c.color, "definition": c.definition,
        "inclusion_criteria": c.inclusion_criteria, "exclusion_criteria": c.exclusion_criteria,
        "status": c.status, "merged_into_id": c.merged_into_id, "version": c.version,
        "exclusive_group": c.exclusive_group, "rated": c.rated,
        "rating_label": c.rating_label, "rating_low": c.rating_low, "rating_high": c.rating_high,
        "ai_suggestible": c.ai_suggestible, "theme_stage": c.theme_stage,
    })
}

/// What a coding's code change needs to know about it: its code, rating, source, span, coder, and layer.
type CodingPlace = (i64, Option<i64>, i64, i64, i64, i64, String);

/// The longest a group name or a rating phrase may be.
const MAX_GROUP_NAME: usize = 80;
const MAX_RATING_TEXT: usize = 120;

/// A group name as it will be stored: trimmed, one line, and in the spelling the group already has when a group
/// of that name exists (names are compared without regard to case, so "sentiment" joins "Sentiment").
pub(crate) fn clean_group_name(tx: &Transaction, raw: &str, id: i64) -> Result<String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.chars().count() > MAX_GROUP_NAME {
        return Err(Error::Invalid(format!("a group name is at most {MAX_GROUP_NAME} characters")));
    }
    if name.is_empty() {
        return Ok(name);
    }
    let existing: Option<String> = tx
        .query_row(
            "SELECT exclusive_group FROM code WHERE id <> ?1 AND exclusive_group <> '' AND lower(exclusive_group) = lower(?2) LIMIT 1",
            params![id, name],
            |r| r.get(0),
        )
        .optional()?;
    Ok(existing.unwrap_or(name))
}

pub(crate) fn clean_rating_text(raw: &str, what: &str) -> Result<String> {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > MAX_RATING_TEXT {
        return Err(Error::Invalid(format!("{what} is at most {MAX_RATING_TEXT} characters")));
    }
    Ok(text)
}

fn get_code(tx: &Transaction, id: i64) -> Result<CodeRow> {
    tx.query_row(&format!("SELECT {CODE_COLUMNS} FROM code c WHERE c.id = ?1"), [id], code_from_row)
        .optional()?
        .ok_or(Error::NotFound { what: "code", id })
}

/// True when `ancestor` is `id` itself or one of its ancestors.
fn is_ancestor(tx: &Transaction, ancestor: i64, id: i64) -> Result<bool> {
    let mut cur = Some(id);
    let mut steps = 0;
    while let Some(c) = cur {
        if c == ancestor {
            return Ok(true);
        }
        steps += 1;
        if steps > 10_000 {
            return Err(Error::Invalid("the codebook has a loop in it".to_string()));
        }
        cur = tx.query_row("SELECT parent_id FROM code WHERE id = ?1", [c], |r| r.get(0)).optional()?.flatten();
    }
    Ok(false)
}

fn sibling_clash(tx: &Transaction, parent: Option<i64>, name: &str, except: Option<i64>) -> Result<bool> {
    let clash: Option<i64> = tx
        .query_row(
            "SELECT id FROM code WHERE status = 'active' AND parent_id IS ?1 AND lower(name) = lower(?2) AND id IS NOT ?3",
            params![parent, name, except],
            |r| r.get(0),
        )
        .optional()?;
    Ok(clash.is_some())
}

impl Project {
    /// Every code, parents before children is not guaranteed; callers build the tree from `parent_id`.
    pub fn list_codes(&self) -> Result<Vec<CodeRow>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {CODE_COLUMNS} FROM code c ORDER BY COALESCE(c.parent_id, 0), c.sort_order, lower(c.name), c.id"))?;
        let rows = stmt.query_map([], code_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_code(&self, id: i64) -> Result<CodeRow> {
        self.conn
            .query_row(&format!("SELECT {CODE_COLUMNS} FROM code c WHERE c.id = ?1"), [id], code_from_row)
            .optional()?
            .ok_or(Error::NotFound { what: "code", id })
    }

    pub fn list_coders(&self) -> Result<Vec<CoderRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, role, local_id, provider, model_name, model_version FROM coder ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(CoderRow {
                id: r.get(0)?,
                name: r.get(1)?,
                role: r.get(2)?,
                local_id: r.get(3)?,
                provider: r.get(4)?,
                model_name: r.get(5)?,
                model_version: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Finds the coder with this local id, or adds a human coder. Used when the person who opens the project is the coder.
    pub fn ensure_human_coder(&mut self, name: &str, local_id: &str) -> Result<i64> {
        if let Some(id) = self.conn.query_row("SELECT id FROM coder WHERE local_id = ?1", [local_id], |r| r.get(0)).optional()? {
            return Ok(id);
        }
        // A coder that came in from another program's file has a placeholder for an identity (`qdpx:` and the file's identifier). When the
        // person who opens the project has that coder's name, it is that person: otherwise they would be two coders of one name, one of
        // them with all the codings.
        if let Some(id) = self
            .conn
            .query_row("SELECT id FROM coder WHERE role = 'human' AND local_id LIKE 'qdpx:%' AND lower(name) = lower(?1) ORDER BY id LIMIT 1", [name], |r| r.get::<_, i64>(0))
            .optional()?
        {
            self.conn.execute("UPDATE coder SET local_id = ?1 WHERE id = ?2", params![local_id, id])?;
            return Ok(id);
        }
        let mut coder = NewCoder::human(name);
        coder.local_id = Some(local_id.to_string());
        self.add_coder(&coder)
    }

    pub fn list_codings(&self, source_id: i64) -> Result<Vec<CodingRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, start_offset, end_offset, code_id, coder_id, origin, layer, rationale, unit_id, created_at, rating, ai_run_id
             FROM coding WHERE source_id = ?1 ORDER BY start_offset, end_offset, id",
        )?;
        let rows = stmt.query_map([source_id], |r| {
            Ok(CodingRow {
                id: r.get(0)?,
                source_id: r.get(1)?,
                start: r.get(2)?,
                end: r.get(3)?,
                code_id: r.get(4)?,
                coder_id: r.get(5)?,
                origin: r.get(6)?,
                layer: r.get(7)?,
                rationale: r.get(8)?,
                rating: r.get(11)?,
                unit_id: r.get(9)?,
                created_at: r.get(10)?,
                ai_run_id: r.get(12)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Edits a code. The version goes up when the name or any part of the definition changes.
    pub fn update_code(&mut self, id: i64, edit: &CodeEdit) -> Result<CodeRow> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let before = get_code(&tx, id)?;
        let name = match &edit.name {
            Some(n) => {
                let n = n.trim();
                if n.is_empty() {
                    return Err(Error::Invalid("a code needs a name".to_string()));
                }
                if before.status == "active" && sibling_clash(&tx, before.parent_id, n, Some(id))? {
                    return Err(Error::Invalid(format!("there is already a code named '{n}' at this place in the codebook")));
                }
                n.to_string()
            }
            None => before.name.clone(),
        };
        let definition = edit.definition.clone().unwrap_or_else(|| before.definition.clone());
        let inclusion = edit.inclusion_criteria.clone().unwrap_or_else(|| before.inclusion_criteria.clone());
        let exclusion = edit.exclusion_criteria.clone().unwrap_or_else(|| before.exclusion_criteria.clone());
        let color = edit.color.clone().unwrap_or_else(|| before.color.clone());
        let kind = match edit.theme {
            Some(true) => "theme",
            Some(false) => "code",
            None => before.kind.as_str(),
        };
        let shared = edit.do_not_share.unwrap_or(before.do_not_share);
        let ai_suggestible = edit.ai_suggestible.unwrap_or(before.ai_suggestible);
        let theme_stage = match edit.theme_stage.as_deref() {
            Some(s @ ("candidate" | "reviewed" | "defined")) => s.to_string(),
            Some(other) => return Err(Error::Invalid(format!("a theme is a candidate, reviewed, or defined, not '{other}'"))),
            None => before.theme_stage.clone(),
        };
        let group = match &edit.exclusive_group {
            Some(g) => clean_group_name(&tx, g, id)?,
            None => before.exclusive_group.clone(),
        };
        let rating_label = match &edit.rating_label {
            Some(t) => clean_rating_text(t, "the name of the rating")?,
            None => before.rating_label.clone(),
        };
        let rating_low = match &edit.rating_low {
            Some(t) => clean_rating_text(t, "the meaning of a 1")?,
            None => before.rating_low.clone(),
        };
        let rating_high = match &edit.rating_high {
            Some(t) => clean_rating_text(t, "the meaning of a 5")?,
            None => before.rating_high.clone(),
        };
        let rated = edit.rated.unwrap_or(before.rated);
        if before.rated && !rated {
            // Ratings only live on rated codes, so switching rating off has to say what happens to the ones given.
            let mut stmt = tx.prepare("SELECT id, rating FROM coding WHERE code_id = ?1 AND rating IS NOT NULL ORDER BY id")?;
            let given: Vec<(i64, i64)> = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
            drop(stmt);
            if !given.is_empty() {
                if !edit.clear_ratings {
                    return Err(Error::Invalid(format!(
                        "{} coding{} of this code {} a rating. Remove the ratings first, or turn rating off and clear them together.",
                        given.len(),
                        if given.len() == 1 { "" } else { "s" },
                        if given.len() == 1 { "carries" } else { "carry" }
                    )));
                }
                for (coding_id, old) in given {
                    tx.execute("UPDATE coding SET rating = NULL WHERE id = ?1", [coding_id])?;
                    event::insert(&tx, &self.actor, &self.session, "coding.rating", Some(("coding", coding_id)), Some(&json!({ "rating": old })), Some(&json!({ "rating": null })))?;
                }
            }
        }
        let meaning_changed =
            name != before.name || definition != before.definition || inclusion != before.inclusion_criteria || exclusion != before.exclusion_criteria;
        let version = before.version + meaning_changed as i64;
        tx.execute(
            "UPDATE code SET name = ?1, definition = ?2, inclusion_criteria = ?3, exclusion_criteria = ?4, color = ?5,
                             kind = ?6, do_not_share = ?7, version = ?8, exclusive_group = ?9, rated = ?10,
                             rating_label = ?11, rating_low = ?12, rating_high = ?13, ai_suggestible = ?14, theme_stage = ?16 WHERE id = ?15",
            params![name, definition, inclusion, exclusion, color, kind, shared as i64, version, group, rated as i64, rating_label, rating_low, rating_high, ai_suggestible as i64, id, theme_stage],
        )?;
        let after = get_code(&tx, id)?;
        if after != before {
            event::insert(&tx, &self.actor, &self.session, "code.update", Some(("code", id)), Some(&code_json(&before)), Some(&code_json(&after)))?;
        }
        tx.commit()?;
        Ok(after)
    }

    /// Moves a code under a new parent (or to the top) and to a position among its new siblings.
    pub fn move_code(&mut self, id: i64, new_parent: Option<i64>, position: Option<usize>) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let code = get_code(&tx, id)?;
        if let Some(p) = new_parent {
            get_code(&tx, p)?;
            if is_ancestor(&tx, id, p)? {
                return Err(Error::Invalid("a code cannot be moved under itself or one of its own sub-codes".to_string()));
            }
        }
        if code.status == "active" && sibling_clash(&tx, new_parent, &code.name, Some(id))? {
            return Err(Error::Invalid(format!("there is already a code named '{}' at the new place", code.name)));
        }
        // Renumber the new siblings so the code lands at the requested position.
        let mut siblings: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM code WHERE parent_id IS ?1 AND id != ?2 ORDER BY sort_order, lower(name), id")?;
            let rows = stmt.query_map(params![new_parent, id], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let at = position.unwrap_or(siblings.len()).min(siblings.len());
        siblings.insert(at, id);
        tx.execute("UPDATE code SET parent_id = ?1 WHERE id = ?2", params![new_parent, id])?;
        for (i, sid) in siblings.iter().enumerate() {
            tx.execute("UPDATE code SET sort_order = ?1 WHERE id = ?2", params![i as i64, sid])?;
        }
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "code.move",
            Some(("code", id)),
            Some(&json!({ "parent_id": code.parent_id, "sort_order": code.sort_order })),
            Some(&json!({ "parent_id": new_parent, "position": at })),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Retires a code: it can no longer be applied, but every coding that uses it stays.
    pub fn retire_code(&mut self, id: i64) -> Result<()> {
        self.set_code_status(id, "retired", "code.retire")
    }

    /// Brings a retired code back, unless an active code now has its name at the same place.
    pub fn restore_code(&mut self, id: i64) -> Result<()> {
        self.set_code_status(id, "active", "code.restore")
    }

    fn set_code_status(&mut self, id: i64, status: &str, action: &str) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let code = get_code(&tx, id)?;
        if code.status == "merged" {
            return Err(Error::Invalid("this code was merged into another one, so it cannot be changed".to_string()));
        }
        if status == "active" && sibling_clash(&tx, code.parent_id, &code.name, Some(id))? {
            return Err(Error::Invalid(format!("there is already an active code named '{}' at this place", code.name)));
        }
        tx.execute("UPDATE code SET status = ?1 WHERE id = ?2", params![status, id])?;
        event::insert(&tx, &self.actor, &self.session, action, Some(("code", id)), Some(&json!({ "status": code.status })), Some(&json!({ "status": status })))?;
        tx.commit()?;
        Ok(())
    }

    /// Merges `from` into `into`: every coding and anchor example moves, sub-codes move under `into`,
    /// and `from` stays in the codebook as "merged" so the mapping is never lost.
    pub fn merge_codes(&mut self, from: i64, into: i64) -> Result<MergeOutcome> {
        self.ensure_writable()?;
        if from == into {
            return Err(Error::Invalid("a code cannot be merged into itself".to_string()));
        }
        let tx = self.conn.transaction()?;
        let a = get_code(&tx, from)?;
        let b = get_code(&tx, into)?;
        if a.status != "active" || b.status != "active" {
            return Err(Error::Invalid("only active codes can be merged".to_string()));
        }
        if is_ancestor(&tx, from, into)? {
            return Err(Error::Invalid("a code cannot be merged into one of its own sub-codes".to_string()));
        }
        let moved_ids: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM coding WHERE code_id = ?1 ORDER BY id")?;
            let rows = stmt.query_map([from], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let moved_regions: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM region WHERE code_id = ?1 ORDER BY id")?;
            let rows = stmt.query_map([from], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        tx.execute("UPDATE coding SET code_id = ?1 WHERE code_id = ?2", params![into, from])?;
        tx.execute("UPDATE region SET code_id = ?1 WHERE code_id = ?2", params![into, from])?;
        tx.execute("INSERT OR IGNORE INTO code_anchor (code_id, coding_id, kind) SELECT ?1, coding_id, kind FROM code_anchor WHERE code_id = ?2", params![into, from])?;
        tx.execute("DELETE FROM code_anchor WHERE code_id = ?1", [from])?;
        tx.execute("INSERT OR IGNORE INTO code_set_member (code_set_id, code_id) SELECT code_set_id, ?1 FROM code_set_member WHERE code_id = ?2", params![into, from])?;
        tx.execute("DELETE FROM code_set_member WHERE code_id = ?1", [from])?;
        let children_moved = tx.execute("UPDATE code SET parent_id = ?1 WHERE parent_id = ?2", params![into, from])?;
        tx.execute("UPDATE code SET status = 'merged', merged_into_id = ?1 WHERE id = ?2", params![into, from])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "code.merge",
            Some(("code", from)),
            Some(&code_json(&a)),
            Some(&json!({ "merged_into": into, "merged_into_name": b.name, "coding_ids": moved_ids, "region_ids": moved_regions, "children_moved": children_moved })),
        )?;
        tx.commit()?;
        Ok(MergeOutcome { codings_moved: moved_ids.len() + moved_regions.len(), children_moved })
    }

    /// Splits a code: makes a new code beside it and moves the chosen codings to it.
    pub fn split_code(&mut self, from: i64, new: &NewCode, coding_ids: &[i64]) -> Result<i64> {
        self.ensure_writable()?;
        if coding_ids.is_empty() {
            return Err(Error::Invalid("choose at least one coding to move to the new code".to_string()));
        }
        let source = self.get_code(from)?;
        if source.status != "active" {
            return Err(Error::Invalid("only an active code can be split".to_string()));
        }
        let mut new = new.clone();
        new.parent_id = source.parent_id;
        // Check every coding belongs to the code first, so a mistake changes nothing.
        for cid in coding_ids {
            let owner: Option<i64> = self.conn.query_row("SELECT code_id FROM coding WHERE id = ?1", [cid], |r| r.get(0)).optional()?;
            match owner {
                None => return Err(Error::NotFound { what: "coding", id: *cid }),
                Some(o) if o != from => return Err(Error::Invalid(format!("coding {cid} does not use code {from}"))),
                _ => {}
            }
        }
        let new_id = self.add_code(&new)?;
        let tx = self.conn.transaction()?;
        for cid in coding_ids {
            tx.execute("UPDATE coding SET code_id = ?1 WHERE id = ?2", params![new_id, cid])?;
        }
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "code.split",
            Some(("code", from)),
            None,
            Some(&json!({ "new_code_id": new_id, "coding_ids": coding_ids })),
        )?;
        tx.commit()?;
        Ok(new_id)
    }

    /// Changes which code a coding uses.
    pub fn change_coding_code(&mut self, coding_id: i64, new_code_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let row: Option<CodingPlace> = tx
            .query_row(
                "SELECT code_id, rating, source_id, start_offset, end_offset, coder_id, layer FROM coding WHERE id = ?1",
                [coding_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .optional()?;
        let (old, rating, source_id, start, end, coder_id, layer) = row.ok_or(Error::NotFound { what: "coding", id: coding_id })?;
        let target: Option<(String, String, i64)> = tx
            .query_row("SELECT status, name, rated FROM code WHERE id = ?1", [new_code_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let (status, new_name, new_rated) = target.ok_or(Error::NotFound { what: "code", id: new_code_id })?;
        if status != "active" {
            return Err(Error::Invalid(format!("code {new_code_id} is {status}, so it cannot be applied")));
        }
        let conflicts = crate::ops::exclusive_conflicts(&tx, source_id, start, end, new_code_id, coder_id, &layer, Some(coding_id))?;
        if !conflicts.is_empty() {
            return Err(Error::ExclusiveGroup(crate::ops::exclusive_message(&new_name, &conflicts)));
        }
        // A rating belongs to a rated code: it goes with the coding to another rated code, and is cleared otherwise.
        let kept_rating = if new_rated != 0 { rating } else { None };
        tx.execute("UPDATE coding SET code_id = ?1, rating = ?2 WHERE id = ?3", params![new_code_id, kept_rating, coding_id])?;
        let (before, after) = if kept_rating == rating {
            (json!({ "code_id": old }), json!({ "code_id": new_code_id }))
        } else {
            // The rating went with the change, so the record says what it was and that it is gone.
            (json!({ "code_id": old, "rating": rating }), json!({ "code_id": new_code_id, "rating": kept_rating }))
        };
        event::insert(&tx, &self.actor, &self.session, "coding.change_code", Some(("coding", coding_id)), Some(&before), Some(&after))?;
        tx.commit()?;
        Ok(())
    }

    /// Sets the note that explains why a coding was made (specification 8.3).
    pub fn set_coding_rationale(&mut self, coding_id: i64, rationale: &str) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let old: Option<String> = tx.query_row("SELECT rationale FROM coding WHERE id = ?1", [coding_id], |r| r.get(0)).optional()?;
        let old = old.ok_or(Error::NotFound { what: "coding", id: coding_id })?;
        tx.execute("UPDATE coding SET rationale = ?1 WHERE id = ?2", params![rationale, coding_id])?;
        event::insert(&tx, &self.actor, &self.session, "coding.rationale", Some(("coding", coding_id)), Some(&json!({ "rationale": old })), Some(&json!({ "rationale": rationale })))?;
        tx.commit()?;
        Ok(())
    }

    /// Gives a coding a rating from 1 to 5, or clears it with `None`. A rating outside 1 to 5 is refused
    /// and changes nothing. Setting the rating a coding already has is not a change, so it is not logged:
    /// the audit trail (and "last modified" read from it) only records real changes.
    pub fn set_coding_rating(&mut self, coding_id: i64, rating: Option<i64>) -> Result<()> {
        self.ensure_writable()?;
        if let Some(r) = rating {
            if !(1..=5).contains(&r) {
                return Err(Error::Invalid(format!("a rating is a whole number from 1 to 5 (or none), not {r}")));
            }
        }
        let tx = self.conn.transaction()?;
        let row: Option<(Option<i64>, i64, String)> = tx
            .query_row(
                "SELECT g.rating, c.rated, c.name FROM coding g JOIN code c ON c.id = g.code_id WHERE g.id = ?1",
                [coding_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (old, code_rated, code_name) = row.ok_or(Error::NotFound { what: "coding", id: coding_id })?;
        if old == rating {
            return Ok(());
        }
        if rating.is_some() && code_rated == 0 {
            return Err(Error::Invalid(format!(
                "'{code_name}' is not a rated code, so its codings cannot be rated. Turn on Weighted / Rated for the code in the Codebook Editor first."
            )));
        }
        tx.execute("UPDATE coding SET rating = ?1 WHERE id = ?2", params![rating, coding_id])?;
        event::insert(&tx, &self.actor, &self.session, "coding.rating", Some(("coding", coding_id)), Some(&json!({ "rating": old })), Some(&json!({ "rating": rating })))?;
        tx.commit()?;
        Ok(())
    }

    /// Everything that happened to one coding, oldest first: the events that name it as their target.
    /// The first is the one that created it. Merging or splitting a code logs against the code (with the
    /// coding ids inside), so those are not listed here.
    pub fn coding_history(&self, coding_id: i64) -> Result<Vec<HistoryEntry>> {
        event::for_coding(&self.conn, coding_id)
    }

    /// Marks a coding as an anchor example for its code (specification 9.2).
    pub fn set_anchor(&mut self, coding_id: i64, anchor: bool) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let code: Option<i64> = tx.query_row("SELECT code_id FROM coding WHERE id = ?1", [coding_id], |r| r.get(0)).optional()?;
        let code = code.ok_or(Error::NotFound { what: "coding", id: coding_id })?;
        if anchor {
            tx.execute("INSERT OR IGNORE INTO code_anchor (code_id, coding_id, kind) VALUES (?1, ?2, 'example')", params![code, coding_id])?;
        } else {
            tx.execute("DELETE FROM code_anchor WHERE code_id = ?1 AND coding_id = ?2 AND kind = 'example'", params![code, coding_id])?;
        }
        event::insert(&tx, &self.actor, &self.session, if anchor { "anchor.add" } else { "anchor.remove" }, Some(("coding", coding_id)), None, Some(&json!({ "code_id": code })))?;
        tx.commit()?;
        Ok(())
    }

    /// The coding ids that are anchor examples for a code.
    pub fn list_anchors(&self, code_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare("SELECT coding_id FROM code_anchor WHERE code_id = ?1 AND kind = 'example' ORDER BY coding_id")?;
        let rows = stmt.query_map([code_id], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Marks a coding as a counterexample of a code, or takes the mark away: a passage that looks as if it belongs
    /// to the code but does not. The coding is one made with another code; a coding of the code itself is an
    /// example, not a counterexample.
    pub fn set_counterexample(&mut self, code_id: i64, coding_id: i64, on: bool) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let own: Option<i64> = tx.query_row("SELECT code_id FROM coding WHERE id = ?1", [coding_id], |r| r.get(0)).optional()?;
        let own = own.ok_or(Error::NotFound { what: "coding", id: coding_id })?;
        let exists: Option<i64> = tx.query_row("SELECT id FROM code WHERE id = ?1", [code_id], |r| r.get(0)).optional()?;
        if exists.is_none() {
            return Err(Error::NotFound { what: "code", id: code_id });
        }
        if on {
            if own == code_id {
                return Err(Error::Invalid("this passage is coded with the code itself, so it is an example of it, not a counterexample".to_string()));
            }
            let already: Option<i64> =
                tx.query_row("SELECT 1 FROM code_anchor WHERE code_id = ?1 AND coding_id = ?2 AND kind = 'counter'", params![code_id, coding_id], |r| r.get(0)).optional()?;
            if already.is_some() {
                return Ok(());
            }
            tx.execute("INSERT INTO code_anchor (code_id, coding_id, kind) VALUES (?1, ?2, 'counter')", params![code_id, coding_id])?;
        } else {
            let gone = tx.execute("DELETE FROM code_anchor WHERE code_id = ?1 AND coding_id = ?2 AND kind = 'counter'", params![code_id, coding_id])?;
            if gone == 0 {
                return Ok(());
            }
        }
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            if on { "counterexample.add" } else { "counterexample.remove" },
            Some(("coding", coding_id)),
            None,
            Some(&json!({ "code_id": code_id })),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The coding ids that are counterexamples of a code.
    pub fn list_counterexamples(&self, code_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare("SELECT coding_id FROM code_anchor WHERE code_id = ?1 AND kind = 'counter' ORDER BY coding_id")?;
        let rows = stmt.query_map([code_id], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Passages that already break a group's rule: pairs of overlapping codings by one coder whose codes are both in
    /// the group. A group can be made over codings that were made before it existed, so the rule stops new ones
    /// and this lists the old ones. Returns `(first coding id, second coding id)`, each pair once.
    pub fn exclusive_group_violations(&self, group: &str) -> Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, b.id
             FROM coding a JOIN code ca ON ca.id = a.code_id
                  JOIN coding b ON b.source_id = a.source_id AND b.coder_id = a.coder_id AND b.layer = a.layer AND b.id > a.id
                  JOIN code cb ON cb.id = b.code_id
             WHERE ca.status = 'active' AND cb.status = 'active' AND ?1 <> '' AND lower(ca.exclusive_group) = lower(?1) AND lower(cb.exclusive_group) = lower(?1)
               AND a.code_id <> b.code_id AND a.start_offset < b.end_offset AND a.end_offset > b.start_offset
             ORDER BY a.id, b.id",
        )?;
        let rows = stmt.query_map([group], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Freezes the codebook as a numbered version with a note, such as "v1.3 after pilot" (specification 9.3).
    pub fn snapshot_codebook(&mut self, note: &str) -> Result<i64> {
        self.ensure_writable()?;
        let codes = self.list_codes()?;
        let snapshot = Value::Array(
            codes
                .iter()
                .map(|c| {
                    let mut v = code_json(c);
                    v["id"] = json!(c.id);
                    v
                })
                .collect(),
        );
        let tx = self.conn.transaction()?;
        let next: i64 = tx.query_row("SELECT COALESCE(MAX(version), 0) + 1 FROM codebook_version", [], |r| r.get(0))?;
        tx.execute(
            "INSERT INTO codebook_version (version, note, snapshot_json, created_at) VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![next, note.trim(), snapshot.to_string()],
        )?;
        event::insert(&tx, &self.actor, &self.session, "codebook.snapshot", None, None, Some(&json!({ "version": next, "note": note.trim(), "codes": codes.len() })))?;
        tx.commit()?;
        Ok(next)
    }

    /// One frozen version of the codebook: its note, when it was made, and every code as it was then (the same fields a
    /// code has now, plus its id), so two versions, or a version and the codebook today, can be compared.
    pub fn get_codebook_version(&self, version: i64) -> Result<Value> {
        let (note, snapshot, created_at): (String, String, String) = self
            .conn
            .query_row("SELECT note, snapshot_json, created_at FROM codebook_version WHERE version = ?1", [version], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "codebook version", id: version })?;
        let codes: Value = serde_json::from_str(&snapshot).unwrap_or_else(|_| Value::Array(Vec::new()));
        Ok(json!({ "version": version, "note": note, "created_at": created_at, "codes": codes }))
    }

    pub fn list_codebook_versions(&self) -> Result<Vec<CodebookVersion>> {
        let mut stmt = self.conn.prepare("SELECT version, note, snapshot_json, created_at FROM codebook_version ORDER BY version")?;
        let rows = stmt.query_map([], |r| {
            let snapshot: String = r.get(2)?;
            let codes = serde_json::from_str::<Value>(&snapshot).ok().and_then(|v| v.as_array().map(Vec::len)).unwrap_or(0);
            Ok(CodebookVersion { version: r.get(0)?, note: r.get(1)?, codes, created_at: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Adds a code set, or returns the id of the one with this name.
    pub fn add_code_set(&mut self, name: &str, description: &str) -> Result<i64> {
        self.ensure_writable()?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a code set needs a name".to_string()));
        }
        let tx = self.conn.transaction()?;
        if let Some(id) = tx.query_row("SELECT id FROM code_set WHERE name = ?1", [name], |r| r.get(0)).optional()? {
            return Ok(id);
        }
        tx.execute("INSERT INTO code_set (name, description) VALUES (?1, ?2)", params![name, description])?;
        let id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "code_set.add", Some(("code_set", id)), None, Some(&json!({ "name": name })))?;
        tx.commit()?;
        Ok(id)
    }

    /// Puts a code in a set, or takes it out. A code can be in many sets without moving in the tree.
    pub fn set_code_in_set(&mut self, code_set_id: i64, code_id: i64, member: bool) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        get_code(&tx, code_id)?;
        if tx.query_row("SELECT id FROM code_set WHERE id = ?1", [code_set_id], |r| r.get::<_, i64>(0)).optional()?.is_none() {
            return Err(Error::NotFound { what: "code set", id: code_set_id });
        }
        if member {
            tx.execute("INSERT OR IGNORE INTO code_set_member (code_set_id, code_id) VALUES (?1, ?2)", params![code_set_id, code_id])?;
        } else {
            tx.execute("DELETE FROM code_set_member WHERE code_set_id = ?1 AND code_id = ?2", params![code_set_id, code_id])?;
        }
        event::insert(&tx, &self.actor, &self.session, "code_set.member", Some(("code_set", code_set_id)), None, Some(&json!({ "code_id": code_id, "member": member })))?;
        tx.commit()?;
        Ok(())
    }

    /// Every set with the ids of its codes.
    pub fn list_code_sets(&self) -> Result<Vec<(i64, String, Vec<i64>)>> {
        let mut out = Vec::new();
        let mut stmt = self.conn.prepare("SELECT id, name FROM code_set ORDER BY name")?;
        let sets: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        for (id, name) in sets {
            let mut m = self.conn.prepare("SELECT code_id FROM code_set_member WHERE code_set_id = ?1 ORDER BY code_id")?;
            let ids: Vec<i64> = m.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            out.push((id, name, ids));
        }
        Ok(out)
    }
}
