//! Batch import (specification section 5). Importers elsewhere turn a file into an [`ImportBatch`];
//! this module owns putting a batch into the project, all in one transaction, with one audit event
//! per source.
//!
//! Rules every importer must follow, which `apply_import` checks:
//! * `text` is already in Unicode NFC, because every offset counts scalar values of the stored text.
//! * Unit and format-run spans lie inside the text.
//! * Units are in text order and a unit's `parent` is the index of an earlier unit.
//! * Every attribute name used on a unit or case has an [`AttributeSpec`].

use std::collections::{BTreeMap, HashMap};

use rusqlite::{params, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use unicode_normalization::is_nfc;

use crate::event;
use crate::model::{AppliesTo, Level, RunKind, SourceType, UnitKind};
use crate::project::Project;
use crate::{Error, Result};

/// A stretch of formatting kept from the original document (headings, emphasis, lists, table cells, PDF pages).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatRun {
    pub start: i64,
    pub end: i64,
    pub kind: RunKind,
    /// Heading level (1 to 6), when `kind` is a heading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// Page number, when `kind` is a page run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

/// Something the importer could not carry over, shown in the import report (specification 5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportWarning {
    /// A short machine-readable kind, for example `skipped_image`, `unreadable_page`, `relative_timestamps`.
    pub kind: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

impl ImportWarning {
    pub fn new(kind: &str, message: impl Into<String>) -> Self {
        Self { kind: kind.to_string(), message: message.into(), location: None }
    }

    pub fn at(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }
}

/// A predefined analysis unit inside an imported source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedUnit {
    pub start: i64,
    pub end: i64,
    pub kind: UnitKind,
    #[serde(default)]
    pub speaker: Option<String>,
    /// Index (into the same source's unit list) of the unit this one replies to; must come earlier.
    #[serde(default)]
    pub parent: Option<usize>,
    /// Left out of coding, for example interviewer turns.
    #[serde(default)]
    pub excluded: bool,
    /// Values of unit-level attributes, by attribute name. Each name needs an [`AttributeSpec`].
    #[serde(default)]
    pub attrs: Vec<(String, String)>,
    /// Where in a recording this unit is spoken, in milliseconds from the start to the end (a unit made from captions has it).
    #[serde(default)]
    pub time_ms: Option<(i64, i64)>,
}

impl ImportedUnit {
    pub fn new(start: i64, end: i64, kind: UnitKind) -> Self {
        Self { start, end, kind, speaker: None, parent: None, excluded: false, attrs: Vec::new(), time_ms: None }
    }
}

/// Defines an attribute: its level of measurement, value labels, and up to three missing-value codes (specification 7.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeSpec {
    pub name: String,
    pub applies_to: AppliesTo,
    pub level: Level,
    #[serde(default)]
    pub value_labels: BTreeMap<String, String>,
    #[serde(default)]
    pub missing_codes: Vec<String>,
}

impl AttributeSpec {
    pub fn new(name: impl Into<String>, applies_to: AppliesTo, level: Level) -> Self {
        Self { name: name.into(), applies_to, level, value_labels: BTreeMap::new(), missing_codes: Vec::new() }
    }
}

/// One source ready to store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedSource {
    pub name: String,
    pub source_type: SourceType,
    #[serde(default)]
    pub folder: String,
    /// Unicode NFC text.
    pub text: String,
    #[serde(default)]
    pub runs: Vec<FormatRun>,
    #[serde(default)]
    pub units: Vec<ImportedUnit>,
    pub default_unit_kind: UnitKind,
    /// Definitions for every unit attribute named in `units`.
    #[serde(default)]
    pub unit_attributes: Vec<AttributeSpec>,
    /// SHA-256 of the original file, lowercase hex.
    #[serde(default)]
    pub original_file_hash: Option<String>,
    /// The settings used, so the import can be explained later.
    #[serde(default)]
    pub settings: Value,
    #[serde(default)]
    pub warnings: Vec<ImportWarning>,
}

/// Which source (and optionally which unit of it) belongs to a case. Indexes point into the batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseMember {
    pub source: usize,
    /// When `None`, the whole source belongs to the case.
    #[serde(default)]
    pub unit: Option<usize>,
}

/// A case: the participant, organization, video, or thread being compared (specification 7.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedCase {
    pub name: String,
    #[serde(default)]
    pub attrs: Vec<(String, String)>,
    #[serde(default)]
    pub members: Vec<CaseMember>,
}

/// Everything one import produces.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportBatch {
    pub sources: Vec<ImportedSource>,
    #[serde(default)]
    pub cases: Vec<ImportedCase>,
    /// Definitions for every case attribute named in `cases`.
    #[serde(default)]
    pub case_attributes: Vec<AttributeSpec>,
    /// The pictures that are kept with some of the sources (a source that is a picture has its file here).
    #[serde(default, skip_serializing)]
    pub images: Vec<crate::ImageAttachment>,
    /// The PDFs that are kept with some of the sources (a source read from a PDF has its file here when its pages can be drawn).
    #[serde(default, skip_serializing)]
    pub pdfs: Vec<crate::PdfAttachment>,
}

/// What `apply_import` created.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportOutcome {
    pub source_ids: Vec<i64>,
    pub case_ids: Vec<i64>,
    pub units_created: usize,
    pub attribute_values_created: usize,
}

impl Project {
    /// Stores a whole import in one transaction: either everything arrives or nothing does.
    pub fn apply_import(&mut self, batch: &ImportBatch) -> Result<ImportOutcome> {
        self.ensure_writable()?;
        validate(batch)?;

        let tx = self.conn.transaction()?;
        let mut outcome = ImportOutcome::default();
        let mut attribute_ids: HashMap<(AppliesTo, String), i64> = HashMap::new();
        let mut unit_ids: Vec<Vec<i64>> = Vec::with_capacity(batch.sources.len());

        for source in &batch.sources {
            let char_count = source.text.chars().count() as i64;
            let report = serde_json::to_string(&source.warnings)?;
            let runs = serde_json::to_string(&source.runs)?;
            let settings = if source.settings.is_null() { "{}".to_string() } else { source.settings.to_string() };
            tx.execute(
                "INSERT INTO source (name, source_type, folder, text, char_count, formatting_runs_json, original_file_hash,
                                     import_settings_json, import_report_json, default_unit_kind, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![
                    source.name,
                    source.source_type.as_str(),
                    source.folder,
                    source.text,
                    char_count,
                    runs,
                    source.original_file_hash,
                    settings,
                    report,
                    source.default_unit_kind.as_str(),
                ],
            )?;
            let source_id = tx.last_insert_rowid();
            outcome.source_ids.push(source_id);

            for attachment in batch.images.iter().filter(|a| a.source == outcome.source_ids.len() - 1) {
                crate::image::insert_image(&tx, source_id, &attachment.image)?;
            }
            for attachment in batch.pdfs.iter().filter(|a| a.source == outcome.source_ids.len() - 1) {
                crate::pdf::insert_pdf(&tx, source_id, &attachment.pdf)?;
            }

            let ids = insert_units(&tx, source_id, source)?;
            outcome.units_created += ids.len();
            outcome.attribute_values_created += insert_unit_attributes(&tx, source, &ids, &mut attribute_ids)?;

            event::insert(
                &tx,
                &self.actor,
                &self.session,
                "source.import",
                Some(("source", source_id)),
                None,
                Some(&json!({
                    "name": source.name,
                    "source_type": source.source_type.as_str(),
                    "characters": char_count,
                    "units": ids.len(),
                    "default_unit_kind": source.default_unit_kind.as_str(),
                    "original_file_hash": source.original_file_hash,
                    "warnings": source.warnings.len(),
                })),
            )?;
            unit_ids.push(ids);
        }

        for case in &batch.cases {
            tx.execute("INSERT INTO research_case (name) VALUES (?1)", [&case.name])?;
            let case_id = tx.last_insert_rowid();
            outcome.case_ids.push(case_id);
            for (name, value) in &case.attrs {
                let spec = batch.case_attributes.iter().find(|a| &a.name == name).expect("validated");
                let attribute_id = ensure_attribute(&tx, spec, &mut attribute_ids)?;
                tx.execute(
                    "INSERT OR REPLACE INTO attribute_value (attribute_id, target_id, value) VALUES (?1, ?2, ?3)",
                    params![attribute_id, case_id, value],
                )?;
                outcome.attribute_values_created += 1;
            }
            for member in &case.members {
                let source_id = outcome.source_ids[member.source];
                match member.unit {
                    None => tx.execute(
                        "INSERT INTO case_member (case_id, source_id) VALUES (?1, ?2)",
                        params![case_id, source_id],
                    )?,
                    Some(u) => tx.execute(
                        "INSERT INTO case_member (case_id, unit_id) VALUES (?1, ?2)",
                        params![case_id, unit_ids[member.source][u]],
                    )?,
                };
            }
        }
        if !batch.cases.is_empty() {
            event::insert(
                &tx,
                &self.actor,
                &self.session,
                "case.import",
                None,
                None,
                Some(&json!({ "cases": batch.cases.len(), "attribute_values": outcome.attribute_values_created })),
            )?;
        }
        tx.commit()?;
        Ok(outcome)
    }
}

fn validate(batch: &ImportBatch) -> Result<()> {
    for (i, source) in batch.sources.iter().enumerate() {
        let label = format!("source {} ('{}')", i + 1, source.name);
        if source.name.trim().is_empty() {
            return Err(Error::Invalid(format!("{label}: a source needs a name")));
        }
        if source.text.contains('\0') {
            return Err(Error::Invalid(format!("{label}: the text contains a NUL character")));
        }
        if !is_nfc(&source.text) {
            return Err(Error::Invalid(format!(
                "{label}: the text is not in Unicode NFC, so the offsets would not match what is stored"
            )));
        }
        let len = source.text.chars().count() as i64;
        if len == 0 {
            return Err(Error::Invalid(format!("{label}: the source has no text")));
        }
        for run in &source.runs {
            if run.start < 0 || run.end <= run.start || run.end > len {
                return Err(Error::Invalid(format!("{label}: a format run {}..{} is outside the text", run.start, run.end)));
            }
        }
        let mut last_start = 0;
        for (u, unit) in source.units.iter().enumerate() {
            if unit.start < 0 || unit.end <= unit.start || unit.end > len {
                return Err(Error::Invalid(format!("{label}: unit {} spans {}..{}, outside the text", u + 1, unit.start, unit.end)));
            }
            if unit.start < last_start {
                return Err(Error::Invalid(format!("{label}: unit {} starts before the unit before it", u + 1)));
            }
            last_start = unit.start;
            if let Some(p) = unit.parent {
                if p >= u {
                    return Err(Error::Invalid(format!("{label}: unit {} has a parent that does not come before it", u + 1)));
                }
            }
            for (name, _) in &unit.attrs {
                if !source.unit_attributes.iter().any(|a| &a.name == name && a.applies_to == AppliesTo::Unit) {
                    return Err(Error::Invalid(format!("{label}: unit attribute '{name}' has no definition")));
                }
            }
        }
        check_specs(&source.unit_attributes)?;
    }
    check_specs(&batch.case_attributes)?;
    for attachment in &batch.images {
        if attachment.source >= batch.sources.len() {
            return Err(Error::Invalid(format!("a picture belongs to source {} which is not in the batch", attachment.source + 1)));
        }
    }
    for attachment in &batch.pdfs {
        if attachment.source >= batch.sources.len() {
            return Err(Error::Invalid(format!("a PDF belongs to source {} which is not in the batch", attachment.source + 1)));
        }
    }
    for case in &batch.cases {
        for (name, _) in &case.attrs {
            if !batch.case_attributes.iter().any(|a| &a.name == name && a.applies_to == AppliesTo::Case) {
                return Err(Error::Invalid(format!("case '{}': attribute '{name}' has no definition", case.name)));
            }
        }
        for m in &case.members {
            let source = batch
                .sources
                .get(m.source)
                .ok_or_else(|| Error::Invalid(format!("case '{}' refers to source {} which is not in the batch", case.name, m.source)))?;
            if let Some(u) = m.unit {
                if u >= source.units.len() {
                    return Err(Error::Invalid(format!("case '{}' refers to unit {} which does not exist", case.name, u)));
                }
            }
        }
    }
    Ok(())
}

fn check_specs(specs: &[AttributeSpec]) -> Result<()> {
    for spec in specs {
        if spec.name.trim().is_empty() {
            return Err(Error::Invalid("an attribute needs a name".to_string()));
        }
        if spec.missing_codes.len() > 3 {
            return Err(Error::Invalid(format!(
                "attribute '{}' has {} missing-value codes; at most 3 are allowed",
                spec.name,
                spec.missing_codes.len()
            )));
        }
    }
    Ok(())
}

fn insert_units(tx: &Transaction, source_id: i64, source: &ImportedSource) -> Result<Vec<i64>> {
    let mut ids: Vec<i64> = Vec::with_capacity(source.units.len());
    let mut stmt = tx.prepare_cached(
        "INSERT INTO unit (source_id, kind, start_offset, end_offset, speaker, parent_unit_id, excluded, time_start_ms, time_end_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for unit in &source.units {
        let parent = unit.parent.map(|p| ids[p]);
        stmt.execute(params![source_id, unit.kind.as_str(), unit.start, unit.end, unit.speaker, parent, unit.excluded as i64, unit.time_ms.map(|t| t.0), unit.time_ms.map(|t| t.1)])?;
        ids.push(tx.last_insert_rowid());
    }
    Ok(ids)
}

fn insert_unit_attributes(
    tx: &Transaction,
    source: &ImportedSource,
    unit_ids: &[i64],
    attribute_ids: &mut HashMap<(AppliesTo, String), i64>,
) -> Result<usize> {
    let mut count = 0;
    let mut specs: HashMap<&str, i64> = HashMap::new();
    for spec in &source.unit_attributes {
        specs.insert(spec.name.as_str(), ensure_attribute(tx, spec, attribute_ids)?);
    }
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO attribute_value (attribute_id, target_id, value) VALUES (?1, ?2, ?3)",
    )?;
    for (unit, id) in source.units.iter().zip(unit_ids) {
        for (name, value) in &unit.attrs {
            stmt.execute(params![specs[name.as_str()], id, value])?;
            count += 1;
        }
    }
    Ok(count)
}

/// Finds the attribute with this name and target, or creates it. An existing attribute must have the same level.
fn ensure_attribute(
    tx: &Transaction,
    spec: &AttributeSpec,
    cache: &mut HashMap<(AppliesTo, String), i64>,
) -> Result<i64> {
    let key = (spec.applies_to, spec.name.clone());
    if let Some(id) = cache.get(&key) {
        return Ok(*id);
    }
    use rusqlite::OptionalExtension;
    let existing: Option<(i64, String)> = tx
        .query_row(
            "SELECT id, level FROM attribute WHERE applies_to = ?1 AND name = ?2",
            params![spec.applies_to.as_str(), spec.name],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let id = match existing {
        Some((id, level)) => {
            if level != spec.level.as_str() {
                return Err(Error::Invalid(format!(
                    "attribute '{}' already exists as {level}, so it cannot be imported as {}",
                    spec.name,
                    spec.level.as_str()
                )));
            }
            id
        }
        None => {
            tx.execute(
                "INSERT INTO attribute (name, applies_to, level, value_labels_json, missing_codes_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    spec.name,
                    spec.applies_to.as_str(),
                    spec.level.as_str(),
                    serde_json::to_string(&spec.value_labels)?,
                    serde_json::to_string(&spec.missing_codes)?
                ],
            )?;
            tx.last_insert_rowid()
        }
    };
    cache.insert(key, id);
    Ok(id)
}
