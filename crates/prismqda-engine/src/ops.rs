//! Changes to a project. Each one validates first, then writes the change and its event in a
//! single transaction, so the audit trail can never disagree with the data.

use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

use crate::event::{self, Event};
use crate::model::{CoderRole, CodingMode, Origin, SourceType, UnitKind};
use crate::project::Project;
use crate::{Error, Result};

/// A coder to add. Model coders also record which model, so reports can name it (specification 18.3).
#[derive(Debug, Clone)]
pub struct NewCoder {
    pub name: String,
    pub role: CoderRole,
    /// A stable identifier for this coder; one is generated when left empty.
    pub local_id: Option<String>,
    pub provider: Option<String>,
    pub model_name: Option<String>,
    pub model_version: Option<String>,
}

impl NewCoder {
    pub fn human(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            role: CoderRole::Human,
            local_id: None,
            provider: None,
            model_name: None,
            model_version: None,
        }
    }
}

/// A document, transcript, survey, or other text to add.
#[derive(Debug, Clone)]
pub struct NewSource {
    pub name: String,
    pub source_type: SourceType,
    pub text: String,
    /// SHA-256 of the file the text came from, lowercase hex; used to warn about duplicate imports.
    pub original_file_hash: Option<String>,
    pub folder: String,
    /// Overrides the default unit for this source type.
    pub default_unit_kind: Option<UnitKind>,
}

impl NewSource {
    pub fn new(name: impl Into<String>, source_type: SourceType, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source_type,
            text: text.into(),
            original_file_hash: None,
            folder: String::new(),
            default_unit_kind: None,
        }
    }
}

/// A code to add to the codebook.
#[derive(Debug, Clone, Default)]
pub struct NewCode {
    pub name: String,
    pub parent_id: Option<i64>,
    /// A theme groups other codes (specification 9.1).
    pub theme: bool,
    pub color: Option<String>,
    pub definition: String,
    pub inclusion_criteria: String,
    pub exclusion_criteria: String,
    /// The mutually exclusive group the code joins; empty for none.
    pub exclusive_group: String,
    /// A rated code: its codings can carry a rating from 1 to 5, described by the label and the two ends.
    pub rated: bool,
    pub rating_label: String,
    pub rating_low: String,
    pub rating_high: String,
}

impl NewCode {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }
}

/// One code applied to one span. Offsets count Unicode scalar values into the source's text.
#[derive(Debug, Clone)]
pub struct NewCoding {
    pub source_id: i64,
    pub start: i64,
    pub end: i64,
    pub code_id: i64,
    pub coder_id: i64,
    pub origin: Origin,
    pub rationale: String,
    pub unit_id: Option<i64>,
    /// When the code belongs to a mutually exclusive group and the same coder has already given another code of
    /// the group to words that overlap, `false` refuses the coding; `true` takes those codings away (each removal
    /// is logged) and applies this one in the same step.
    pub replace_exclusive: bool,
    /// Set when the coding is made by accepting a suggestion from the review queue. The decision is the change a person
    /// made and can undo, so the coding's own log entry carries the suggestion's number and is never undone on its own.
    pub via_proposal: Option<i64>,
    /// The AI run whose suggestion this coding came from, kept on the coding for good.
    pub ai_run_id: Option<i64>,
}

impl NewCoding {
    pub fn new(source_id: i64, start: i64, end: i64, code_id: i64, coder_id: i64) -> Self {
        Self {
            source_id,
            start,
            end,
            code_id,
            coder_id,
            origin: Origin::Human,
            rationale: String::new(),
            unit_id: None,
            replace_exclusive: false,
            via_proposal: None,
            ai_run_id: None,
        }
    }
}

/// A coding that stands in the way of another because both codes belong to the same exclusive group.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GroupConflict {
    pub coding_id: i64,
    pub code_id: i64,
    pub code_name: String,
    pub group: String,
}

/// The codings that `code_id` may not share words with: those by `coder_id` on the same layer that overlap
/// `start..end` and whose (active) code is in the same exclusive group. Empty when the code is in no group.
#[allow(clippy::too_many_arguments)]
pub(crate) fn exclusive_conflicts(
    tx: &rusqlite::Transaction,
    source_id: i64,
    start: i64,
    end: i64,
    code_id: i64,
    coder_id: i64,
    layer: &str,
    except_coding: Option<i64>,
) -> Result<Vec<GroupConflict>> {
    let mut stmt = tx.prepare(
        "SELECT g.id, g.code_id, c2.name, c1.exclusive_group
         FROM code c1, coding g JOIN code c2 ON c2.id = g.code_id
         WHERE c1.id = ?1 AND c1.exclusive_group <> ''
           AND g.source_id = ?2 AND g.coder_id = ?3 AND g.layer = ?4
           AND g.start_offset < ?6 AND g.end_offset > ?5
           AND g.code_id <> ?1 AND c2.status = 'active' AND lower(c2.exclusive_group) = lower(c1.exclusive_group)
           AND (?7 IS NULL OR g.id <> ?7)
         ORDER BY g.start_offset, g.id",
    )?;
    let rows = stmt.query_map(params![code_id, source_id, coder_id, layer, start, end, except_coding], |r| {
        Ok(GroupConflict { coding_id: r.get(0)?, code_id: r.get(1)?, code_name: r.get(2)?, group: r.get(3)? })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// The sentence that says why a coding was refused because of an exclusive group.
pub(crate) fn exclusive_message(new_code_name: &str, conflicts: &[GroupConflict]) -> String {
    let mut names: Vec<String> = Vec::new();
    for c in conflicts {
        let quoted = format!("'{}'", c.code_name);
        if !names.contains(&quoted) {
            names.push(quoted);
        }
    }
    let joined = match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        n => format!("{} and {}", names[..n - 1].join(", "), names[n - 1]),
    };
    format!(
        "'{}' is in the group '{}', which allows one code at a time, and these words already have {}.",
        new_code_name, conflicts[0].group, joined
    )
}

/// Lowercase hex SHA-256 of some bytes, the form stored in `source.original_file_hash`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

impl Project {
    /// Adds a coder (a person or a model).
    pub fn add_coder(&mut self, coder: &NewCoder) -> Result<i64> {
        self.ensure_writable()?;
        let name = require_text(&coder.name, "a coder needs a name")?;
        if coder.role == CoderRole::Model && coder.model_name.as_deref().is_none_or(|m| m.trim().is_empty()) {
            return Err(Error::Invalid("a model coder needs a model name so reports can name it".to_string()));
        }
        let tx = self.conn.transaction()?;
        let local_id = match &coder.local_id {
            Some(id) if !id.trim().is_empty() => id.trim().to_string(),
            _ => tx.query_row("SELECT lower(hex(randomblob(4)))", [], |r| r.get::<_, String>(0))?,
        };
        let duplicate: Option<i64> =
            tx.query_row("SELECT id FROM coder WHERE local_id = ?1", [&local_id], |r| r.get(0)).optional()?;
        if duplicate.is_some() {
            return Err(Error::Invalid(format!("a coder with the id '{local_id}' already exists")));
        }
        tx.execute(
            "INSERT INTO coder (name, role, local_id, provider, model_name, model_version, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![name, coder.role.as_str(), local_id, coder.provider, coder.model_name, coder.model_version],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "coder.add",
            Some(("coder", id)),
            None,
            Some(&json!({
                "name": name, "role": coder.role.as_str(), "local_id": local_id,
                "provider": coder.provider, "model_name": coder.model_name, "model_version": coder.model_version,
            })),
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Adds a source. The text is stored in Unicode NFC, and all offsets count its scalar values.
    pub fn add_source(&mut self, source: &NewSource) -> Result<i64> {
        self.ensure_writable()?;
        let name = require_text(&source.name, "a source needs a name")?;
        if source.text.contains('\0') {
            return Err(Error::Invalid("the text contains a NUL character, which cannot be stored".to_string()));
        }
        let text: String = source.text.nfc().collect();
        let char_count = text.chars().count() as i64;
        if char_count == 0 {
            return Err(Error::Invalid("the source has no text".to_string()));
        }
        let unit_kind = source.default_unit_kind.unwrap_or_else(|| source.source_type.default_unit_kind());

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO source (name, source_type, folder, text, char_count, original_file_hash, default_unit_kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![
                name,
                source.source_type.as_str(),
                source.folder,
                text,
                char_count,
                source.original_file_hash,
                unit_kind.as_str()
            ],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "source.import",
            Some(("source", id)),
            None,
            Some(&json!({
                "name": name, "source_type": source.source_type.as_str(), "characters": char_count,
                "original_file_hash": source.original_file_hash, "default_unit_kind": unit_kind.as_str(),
            })),
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// The id of a source already imported from a file with this hash, if any (specification 5.1).
    pub fn find_source_by_hash(&self, hash: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM source WHERE original_file_hash = ?1 ORDER BY id LIMIT 1", [hash], |r| r.get(0))
            .optional()?)
    }

    /// Adds an analysis unit covering `start..end` of a source.
    pub fn add_unit(
        &mut self,
        source_id: i64,
        kind: UnitKind,
        start: i64,
        end: i64,
        speaker: Option<&str>,
    ) -> Result<i64> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let char_count = source_char_count(&tx, source_id)?;
        check_span(start, end, char_count, "source", source_id)?;
        tx.execute(
            "INSERT INTO unit (source_id, kind, start_offset, end_offset, speaker) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![source_id, kind.as_str(), start, end, speaker],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "unit.add",
            Some(("unit", id)),
            None,
            Some(&json!({ "source_id": source_id, "kind": kind.as_str(), "start": start, "end": end, "speaker": speaker })),
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Adds a code to the codebook. Two active codes under the same parent cannot share a name.
    pub fn add_code(&mut self, code: &NewCode) -> Result<i64> {
        self.ensure_writable()?;
        let name = require_text(&code.name, "a code needs a name")?;
        let tx = self.conn.transaction()?;
        if let Some(parent) = code.parent_id {
            let exists: Option<i64> =
                tx.query_row("SELECT id FROM code WHERE id = ?1", [parent], |r| r.get(0)).optional()?;
            if exists.is_none() {
                return Err(Error::NotFound { what: "parent code", id: parent });
            }
        }
        let clash: Option<i64> = tx
            .query_row(
                "SELECT id FROM code WHERE status = 'active' AND parent_id IS ?1 AND lower(name) = lower(?2)",
                params![code.parent_id, name],
                |r| r.get(0),
            )
            .optional()?;
        if clash.is_some() {
            return Err(Error::Invalid(format!("there is already a code named '{name}' at this place in the codebook")));
        }
        let kind = if code.theme { "theme" } else { "code" };
        let group = crate::codebook::clean_group_name(&tx, &code.exclusive_group, 0)?;
        let rating_label = crate::codebook::clean_rating_text(&code.rating_label, "the name of the rating")?;
        let rating_low = crate::codebook::clean_rating_text(&code.rating_low, "the meaning of a 1")?;
        let rating_high = crate::codebook::clean_rating_text(&code.rating_high, "the meaning of a 5")?;
        tx.execute(
            "INSERT INTO code (name, parent_id, kind, color, definition, inclusion_criteria, exclusion_criteria,
                               exclusive_group, rated, rating_label, rating_low, rating_high, created_at)
             VALUES (?1, ?2, ?3, COALESCE(?4, '#808080'), ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![
                name,
                code.parent_id,
                kind,
                code.color,
                code.definition,
                code.inclusion_criteria,
                code.exclusion_criteria,
                group,
                code.rated as i64,
                rating_label,
                rating_low,
                rating_high
            ],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "code.add",
            Some(("code", id)),
            None,
            Some(&json!({ "name": name, "parent_id": code.parent_id, "kind": kind, "definition": code.definition })),
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Applies a code to a span. Refuses a span outside the text, an inactive code, or (in unit
    /// mode) a coding with no unit, instead of storing something that could not be reported.
    pub fn apply_coding(&mut self, coding: &NewCoding) -> Result<i64> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let id = apply_coding_in(&tx, &self.actor, &self.session, coding)?;
        tx.commit()?;
        Ok(id)
    }

    /// Applies many codings in one transaction: all of them are stored, or none. Each is checked
    /// exactly as `apply_coding` checks one, and each gets its own audit event, so importing
    /// existing coded data leaves the same trail as coding by hand.
    pub fn apply_codings(&mut self, codings: &[NewCoding]) -> Result<Vec<i64>> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let mut ids = Vec::with_capacity(codings.len());
        for (i, coding) in codings.iter().enumerate() {
            let id = apply_coding_in(&tx, &self.actor, &self.session, coding)
                .map_err(|e| Error::Invalid(format!("coding {} of {}: {e}", i + 1, codings.len())))?;
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    }

    /// Removes a coding. The removal is logged with the coding's full contents, never silent.
    pub fn remove_coding(&mut self, coding_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        remove_coding_in(&tx, &self.actor, &self.session, coding_id, None)?;
        tx.commit()?;
        Ok(())
    }
}

/// Removes one coding inside an open transaction and logs it with its full contents. `after` says why when
/// the removal is part of something larger (a replacement in an exclusive group).
pub(crate) fn remove_coding_in(tx: &rusqlite::Transaction, actor: &str, session: &str, coding_id: i64, after: Option<&Value>) -> Result<()> {
    let before: Option<Value> = tx
        .query_row(
            "SELECT source_id, start_offset, end_offset, code_id, coder_id, origin, layer, rationale, unit_id, rating
             FROM coding WHERE id = ?1",
            [coding_id],
            |r| {
                Ok(json!({
                    "source_id": r.get::<_, i64>(0)?, "start": r.get::<_, i64>(1)?, "end": r.get::<_, i64>(2)?,
                    "code_id": r.get::<_, i64>(3)?, "coder_id": r.get::<_, i64>(4)?,
                    "origin": r.get::<_, String>(5)?, "layer": r.get::<_, String>(6)?,
                    "rationale": r.get::<_, String>(7)?, "unit_id": r.get::<_, Option<i64>>(8)?,
                    "rating": r.get::<_, Option<i64>>(9)?,
                }))
            },
        )
        .optional()?;
    let mut before = before.ok_or(Error::NotFound { what: "coding", id: coding_id })?;
    tx.execute("DELETE FROM code_anchor WHERE coding_id = ?1", [coding_id])?;
    tx.execute("DELETE FROM coding WHERE id = ?1", [coding_id])?;
    // The passage's links go with it, and are kept here so that Undo can put them back.
    let links = crate::passage_links::take_links(tx, coding_id)?;
    if !links.is_empty() {
        before["links"] = Value::Array(links);
    }
    event::insert(tx, actor, session, "coding.remove", Some(("coding", coding_id)), Some(&before), after)?;
    Ok(())
}

impl Project {
    /// Adds a researcher's note to the audit trail to explain a decision. Annotations are events
    /// like any other: they can be added, never edited or removed.
    pub fn annotate(&mut self, note: &str) -> Result<()> {
        self.ensure_writable()?;
        let note = require_text(note, "an annotation cannot be empty")?;
        let tx = self.conn.transaction()?;
        event::insert(&tx, &self.actor, &self.session, "annotation", None, None, Some(&json!({ "note": note })))?;
        tx.commit()?;
        Ok(())
    }

    /// The newest `limit` events, oldest first.
    pub fn events(&self, limit: usize) -> Result<Vec<Event>> {
        event::recent(&self.conn, limit)
    }

    /// The text of a source, as stored (Unicode NFC).
    pub fn source_text(&self, source_id: i64) -> Result<String> {
        self.conn
            .query_row("SELECT text FROM source WHERE id = ?1", [source_id], |r| r.get(0))
            .optional()?
            .ok_or(Error::NotFound { what: "source", id: source_id })
    }

    /// The text covered by a span, using scalar-value offsets.
    pub fn text_of_span(&self, source_id: i64, start: i64, end: i64) -> Result<String> {
        let text = self.source_text(source_id)?;
        let count = text.chars().count() as i64;
        check_span(start, end, count, "source", source_id)?;
        Ok(text.chars().skip(start as usize).take((end - start) as usize).collect())
    }
}

fn require_text(value: &str, message: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(Error::Invalid(message.to_string()))
    } else {
        Ok(trimmed.to_string())
    }
}

fn source_char_count(conn: &rusqlite::Connection, source_id: i64) -> Result<i64> {
    conn.query_row("SELECT char_count FROM source WHERE id = ?1", [source_id], |r| r.get(0))
        .optional()?
        .ok_or(Error::NotFound { what: "source", id: source_id })
}

fn check_span(start: i64, end: i64, len: i64, what: &str, id: i64) -> Result<()> {
    if start < 0 || end <= start || end > len {
        return Err(Error::Invalid(format!(
            "the span {start}..{end} does not fit {what} {id}, which has {len} characters (a span needs 0 <= start < end <= {len})"
        )));
    }
    Ok(())
}

/// Validates and stores one coding inside an open transaction, and logs it.
pub(crate) fn apply_coding_in(tx: &rusqlite::Transaction, actor: &str, session: &str, coding: &NewCoding) -> Result<i64> {
    let char_count = source_char_count(tx, coding.source_id)?;
    check_span(coding.start, coding.end, char_count, "source", coding.source_id)?;

    let status: Option<String> =
        tx.query_row("SELECT status FROM code WHERE id = ?1", [coding.code_id], |r| r.get(0)).optional()?;
    match status.as_deref() {
        None => return Err(Error::NotFound { what: "code", id: coding.code_id }),
        Some("active") => {}
        Some(other) => {
            return Err(Error::Invalid(format!("code {} is {other}, so it cannot be applied", coding.code_id)))
        }
    }
    let coder_exists: Option<i64> =
        tx.query_row("SELECT id FROM coder WHERE id = ?1", [coding.coder_id], |r| r.get(0)).optional()?;
    if coder_exists.is_none() {
        return Err(Error::NotFound { what: "coder", id: coding.coder_id });
    }

    let mode: String = tx.query_row("SELECT coding_mode FROM project WHERE id = 1", [], |r| r.get(0))?;
    if mode == CodingMode::Unit.as_str() && coding.unit_id.is_none() {
        return Err(Error::Invalid(
            "this project codes whole units, so every coding needs a unit".to_string(),
        ));
    }
    if let Some(unit_id) = coding.unit_id {
        let unit: Option<(i64, i64, i64, bool)> = tx
            .query_row(
                "SELECT source_id, start_offset, end_offset, excluded FROM unit WHERE id = ?1",
                [unit_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? != 0)),
            )
            .optional()?;
        let (unit_source, unit_start, unit_end, excluded) =
            unit.ok_or(Error::NotFound { what: "unit", id: unit_id })?;
        if unit_source != coding.source_id {
            return Err(Error::Invalid(format!("unit {unit_id} belongs to a different source")));
        }
        if excluded {
            return Err(Error::Invalid(format!("unit {unit_id} is excluded from coding")));
        }
        if coding.start < unit_start || coding.end > unit_end {
            return Err(Error::Invalid(format!(
                "the span {}..{} is outside unit {unit_id} ({unit_start}..{unit_end})",
                coding.start, coding.end
            )));
        }
    }

    // A mutually exclusive group allows one of its codes at a time for the same coder on the same words.
    let conflicts = exclusive_conflicts(tx, coding.source_id, coding.start, coding.end, coding.code_id, coding.coder_id, "coder", None)?;
    if !conflicts.is_empty() {
        if !coding.replace_exclusive {
            let name: String = tx.query_row("SELECT name FROM code WHERE id = ?1", [coding.code_id], |r| r.get(0))?;
            return Err(Error::ExclusiveGroup(exclusive_message(&name, &conflicts)));
        }
        for c in &conflicts {
            remove_coding_in(tx, actor, session, c.coding_id, Some(&json!({ "replaced_by_code": coding.code_id })))?;
        }
    }

    tx.execute(
        "INSERT INTO coding (source_id, start_offset, end_offset, code_id, coder_id, origin, rationale, unit_id, ai_run_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![
            coding.source_id,
            coding.start,
            coding.end,
            coding.code_id,
            coding.coder_id,
            coding.origin.as_str(),
            coding.rationale,
            coding.unit_id,
            coding.ai_run_id
        ],
    )?;
    let id = tx.last_insert_rowid();
    let mut after = json!({
        "source_id": coding.source_id, "start": coding.start, "end": coding.end,
        "code_id": coding.code_id, "coder_id": coding.coder_id, "origin": coding.origin.as_str(),
        "unit_id": coding.unit_id, "rationale": coding.rationale,
    });
    if let Some(p) = coding.via_proposal {
        after["via_proposal"] = json!(p);
    }
    if let Some(r) = coding.ai_run_id {
        after["ai_run_id"] = json!(r);
    }
    event::insert(tx, actor, session, "coding.add", Some(("coding", id)), None, Some(&after))?;
    Ok(id)
}
