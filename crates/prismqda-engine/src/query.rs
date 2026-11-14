//! Reading a project: the lists and details an interface or a script needs.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::project::Project;
use crate::{Error, Result};

/// One source in the source list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceRow {
    pub id: i64,
    pub name: String,
    pub source_type: String,
    pub folder: String,
    /// Length of the text in Unicode scalar values.
    pub characters: i64,
    pub default_unit_kind: String,
    pub text_revision: i64,
    pub units: i64,
    pub codings: i64,
    pub do_not_share: bool,
    pub created_at: String,
    /// What the picture is, when the document is a picture; `None` (JSON `null`) for any other document.
    pub image: Option<SourceImage>,
    /// The PDF kept with the document (its pages can be drawn and areas of them coded); `None` for a document with no PDF kept.
    pub pdf: Option<crate::SourcePdf>,
    /// The recording linked to the document (stretches of it can be coded); `None` for a document with no recording.
    pub recording: Option<crate::SourceRecording>,
}

/// What a list of documents needs to know about a document that is a picture.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceImage {
    pub mime: String,
    pub file_name: String,
    pub width: i64,
    pub height: i64,
    pub size_bytes: i64,
}

/// One analysis unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnitRow {
    pub id: i64,
    pub source_id: i64,
    pub kind: String,
    pub start: i64,
    pub end: i64,
    pub speaker: Option<String>,
    pub parent_id: Option<i64>,
    pub excluded: bool,
    /// Where in the recording the unit starts and ends, in milliseconds, when the document has a recording and the unit has times.
    pub time_start_ms: Option<i64>,
    pub time_end_ms: Option<i64>,
}

/// One value of an attribute on a unit, case, or source.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttributeValueRow {
    pub target_id: i64,
    pub attribute: String,
    pub level: String,
    pub value: Option<String>,
}

/// Where a unit sits, found by the identifier it had in the file it came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnitRef {
    pub unit_id: i64,
    pub source_id: i64,
    pub start: i64,
    pub end: i64,
    pub excluded: bool,
}

/// A case with how many sources and units belong to it, and which sources.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaseRow {
    pub id: i64,
    pub name: String,
    pub notes: String,
    /// How many sources are members of the case.
    pub sources: i64,
    /// How many units are members of the case (a survey respondent's answers, a comment).
    pub units: i64,
    /// The ids of the sources that are members, smallest first. A source that only appears through one
    /// of its units is not listed here; the unit is counted in `units`.
    pub source_ids: Vec<i64>,
}

/// A unit that belongs to a case (a survey respondent's answers, a comment), with where it is in its source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseUnitMember {
    pub case_id: i64,
    pub source_id: i64,
    pub unit_id: i64,
    pub start: i64,
    pub end: i64,
}

/// Everything an interface needs to show a source: its text, formatting, and import report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceDetail {
    pub id: i64,
    pub name: String,
    pub text: String,
    pub formatting_runs: serde_json::Value,
    pub import_report: serde_json::Value,
    pub text_revision: i64,
}

impl Project {
    pub fn list_sources(&self) -> Result<Vec<SourceRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.source_type, s.folder, s.char_count, s.default_unit_kind, s.text_revision, s.do_not_share, s.created_at,
                    (SELECT COUNT(*) FROM unit u WHERE u.source_id = s.id),
                    (SELECT COUNT(*) FROM coding c WHERE c.source_id = s.id) + (SELECT COUNT(*) FROM region rg WHERE rg.source_id = s.id),
                    i.mime, i.file_name, i.width, i.height, i.size_bytes,
                    pf.file_name, pf.page_count, pf.size_bytes,
                    m.kind, m.file_name, m.size_bytes, m.stands_alone
             FROM source s LEFT JOIN image i ON i.source_id = s.id LEFT JOIN pdf_file pf ON pf.source_id = s.id LEFT JOIN media m ON m.source_id = s.id
             ORDER BY s.folder, s.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SourceRow {
                id: r.get(0)?,
                name: r.get(1)?,
                source_type: r.get(2)?,
                folder: r.get(3)?,
                characters: r.get(4)?,
                default_unit_kind: r.get(5)?,
                text_revision: r.get(6)?,
                do_not_share: r.get::<_, i64>(7)? != 0,
                created_at: r.get(8)?,
                units: r.get(9)?,
                codings: r.get(10)?,
                image: match r.get::<_, Option<String>>(11)? {
                    Some(mime) => Some(SourceImage { mime, file_name: r.get(12)?, width: r.get(13)?, height: r.get(14)?, size_bytes: r.get(15)? }),
                    None => None,
                },
                pdf: match r.get::<_, Option<String>>(16)? {
                    Some(file_name) => Some(crate::SourcePdf { file_name, page_count: r.get(17)?, size_bytes: r.get(18)? }),
                    None => None,
                },
                recording: match r.get::<_, Option<String>>(19)? {
                    Some(kind) => Some(crate::SourceRecording { kind, file_name: r.get(20)?, size_bytes: r.get(21)?, stands_alone: r.get::<_, i64>(22)? != 0 }),
                    None => None,
                },
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn source_detail(&self, source_id: i64) -> Result<SourceDetail> {
        use rusqlite::OptionalExtension;
        let row = self
            .conn
            .query_row(
                "SELECT id, name, text, formatting_runs_json, import_report_json, text_revision FROM source WHERE id = ?1",
                [source_id],
                |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, i64>(5)?))
                },
            )
            .optional()?
            .ok_or(Error::NotFound { what: "source", id: source_id })?;
        Ok(SourceDetail {
            id: row.0,
            name: row.1,
            text: row.2,
            formatting_runs: serde_json::from_str(&row.3)?,
            import_report: serde_json::from_str(&row.4)?,
            text_revision: row.5,
        })
    }

    pub fn list_units(&self, source_id: i64) -> Result<Vec<UnitRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, kind, start_offset, end_offset, speaker, parent_unit_id, excluded, time_start_ms, time_end_ms
             FROM unit WHERE source_id = ?1 ORDER BY start_offset, id",
        )?;
        let rows = stmt.query_map([source_id], |r| {
            Ok(UnitRow {
                id: r.get(0)?,
                source_id: r.get(1)?,
                kind: r.get(2)?,
                start: r.get(3)?,
                end: r.get(4)?,
                speaker: r.get(5)?,
                parent_id: r.get(6)?,
                excluded: r.get::<_, i64>(7)? != 0,
                time_start_ms: r.get(8)?,
                time_end_ms: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Attribute values for every unit of a source, in unit order.
    pub fn unit_attribute_values(&self, source_id: i64) -> Result<Vec<AttributeValueRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT av.target_id, a.name, a.level, av.value
             FROM attribute_value av
             JOIN attribute a ON a.id = av.attribute_id AND a.applies_to = 'unit'
             JOIN unit u ON u.id = av.target_id
             WHERE u.source_id = ?1 ORDER BY u.start_offset, a.name",
        )?;
        let rows = stmt.query_map(params![source_id], |r| {
            Ok(AttributeValueRow { target_id: r.get(0)?, attribute: r.get(1)?, level: r.get(2)?, value: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Attribute values for every case.
    pub fn case_attribute_values(&self) -> Result<Vec<AttributeValueRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT av.target_id, a.name, a.level, av.value
             FROM attribute_value av
             JOIN attribute a ON a.id = av.attribute_id AND a.applies_to = 'case'
             ORDER BY av.target_id, a.name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AttributeValueRow { target_id: r.get(0)?, attribute: r.get(1)?, level: r.get(2)?, value: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn list_cases(&self) -> Result<Vec<CaseRow>> {
        // The member counts are taken in two grouped passes over case_member, not one lookup per case: a survey with
        // thousands of respondents makes thousands of cases, and a lookup each would read the whole table that many times.
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, c.notes, COALESCE(s.sources, 0), COALESCE(u.units, 0)
             FROM research_case c
             LEFT JOIN (SELECT case_id, COUNT(DISTINCT source_id) AS sources FROM case_member WHERE source_id IS NOT NULL GROUP BY case_id) s ON s.case_id = c.id
             LEFT JOIN (SELECT case_id, COUNT(*) AS units FROM case_member WHERE unit_id IS NOT NULL GROUP BY case_id) u ON u.case_id = c.id
             ORDER BY c.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(CaseRow { id: r.get(0)?, name: r.get(1)?, notes: r.get(2)?, sources: r.get(3)?, units: r.get(4)?, source_ids: Vec::new() })
        })?;
        let mut cases: Vec<CaseRow> = rows.collect::<std::result::Result<_, _>>()?;

        // The member sources of every case, in one pass.
        let mut by_case: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut members = self
            .conn
            .prepare("SELECT DISTINCT case_id, source_id FROM case_member WHERE source_id IS NOT NULL ORDER BY case_id, source_id")?;
        for row in members.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (case_id, source_id) = row?;
            by_case.entry(case_id).or_default().push(source_id);
        }
        for case in &mut cases {
            case.source_ids = by_case.remove(&case.id).unwrap_or_default();
        }
        Ok(cases)
    }

    /// Every unit that belongs to a case, with its place in its source, in case order. A case that holds whole sources has none here.
    pub fn case_unit_members(&self) -> Result<Vec<CaseUnitMember>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.case_id, u.source_id, u.id, u.start_offset, u.end_offset
             FROM case_member m JOIN unit u ON u.id = m.unit_id
             ORDER BY m.case_id, u.source_id, u.start_offset",
        )?;
        let rows = stmt.query_map([], |r| Ok(CaseUnitMember { case_id: r.get(0)?, source_id: r.get(1)?, unit_id: r.get(2)?, start: r.get(3)?, end: r.get(4)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// One case, as `list_cases` shows it.
    pub fn get_case(&self, id: i64) -> Result<CaseRow> {
        let mut case = self
            .conn
            .query_row(
                "SELECT c.id, c.name, c.notes,
                        (SELECT COUNT(DISTINCT m.source_id) FROM case_member m WHERE m.case_id = c.id AND m.source_id IS NOT NULL),
                        (SELECT COUNT(*) FROM case_member m WHERE m.case_id = c.id AND m.unit_id IS NOT NULL)
                 FROM research_case c WHERE c.id = ?1",
                [id],
                |r| Ok(CaseRow { id: r.get(0)?, name: r.get(1)?, notes: r.get(2)?, sources: r.get(3)?, units: r.get(4)?, source_ids: Vec::new() }),
            )
            .optional()?
            .ok_or(Error::NotFound { what: "case", id })?;
        let mut members =
            self.conn.prepare("SELECT DISTINCT source_id FROM case_member WHERE case_id = ?1 AND source_id IS NOT NULL ORDER BY source_id")?;
        case.source_ids = members.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(case)
    }

    /// Units by the identifier they had in the imported file (the `external_id` attribute), so codings
    /// made elsewhere can be matched to them. If two units share an identifier the first one wins and
    /// the second is counted in the returned number.
    pub fn unit_refs_by_external_id(&self) -> Result<(HashMap<String, UnitRef>, usize)> {
        let mut stmt = self.conn.prepare(
            "SELECT av.value, u.id, u.source_id, u.start_offset, u.end_offset, u.excluded
             FROM attribute_value av
             JOIN attribute a ON a.id = av.attribute_id AND a.applies_to = 'unit' AND a.name = 'external_id'
             JOIN unit u ON u.id = av.target_id
             WHERE av.value IS NOT NULL ORDER BY u.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                UnitRef { unit_id: r.get(1)?, source_id: r.get(2)?, start: r.get(3)?, end: r.get(4)?, excluded: r.get::<_, i64>(5)? != 0 },
            ))
        })?;
        let mut map = HashMap::new();
        let mut duplicates = 0;
        for row in rows {
            let (id, unit) = row?;
            match map.entry(id) {
                std::collections::hash_map::Entry::Occupied(_) => duplicates += 1,
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(unit);
                }
            }
        }
        Ok((map, duplicates))
    }
}
