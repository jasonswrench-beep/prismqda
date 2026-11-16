//! What the case table sent to PrismStatistics is made from (PSTAT-1, section 7.3, the profile `PrismQDA case table 1`).
//!
//! One row per case, and never a row per coding: the same rule as the Statistics page ([`crate::statistics`]). A case carries the
//! variables the project holds for it (its attributes), and for every active code whether any of the case's documents carry it and how
//! many distinct excerpts do. The numbers are the ones the Statistics page shows, because they come from the same counting.
//!
//! This module only gathers. The file format (the names of the variables, the zip, the JSON) is written by `prismqda-export`, so
//! the rules of the profile live in one place. Excerpt text, document text, memos and coders' names are never gathered.

use std::collections::{BTreeMap, HashMap};

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::project::Project;
use crate::Result;

/// One case variable (an attribute that applies to cases) and its values as the project stores them.
#[derive(Debug, Clone, Serialize)]
pub struct CaseTableAttribute {
    /// PrismQDA's id of the attribute.
    pub id: i64,
    pub name: String,
    /// `nominal`, `ordinal`, `interval`, `ratio`, `date` or `text`.
    pub level: String,
    /// The project's value labels: a value as typed, and what it stands for.
    pub value_labels: BTreeMap<String, String>,
    /// The project's missing codes, as typed.
    pub missing_codes: Vec<String>,
    /// One value for each case, in the order of `CaseTable::case_ids`: the text as stored, trimmed, or `None` when the case has none.
    pub values: Vec<Option<String>>,
}

/// An active code and how many excerpts of each case carry it.
#[derive(Debug, Clone, Serialize)]
pub struct CaseTableCode {
    pub id: i64,
    /// The names from the top of the codebook down to this code.
    pub path: Vec<String>,
    /// `code` or `theme`.
    pub kind: String,
    /// For each case, in the order of `CaseTable::case_ids`: the number of distinct excerpts of its documents that carry the code,
    /// or `None` for a case that holds no document.
    pub excerpts: Vec<Option<i64>>,
}

/// The cases of a project with their variables and the counts of their codes.
#[derive(Debug, Clone, Serialize)]
pub struct CaseTable {
    pub project_name: String,
    /// The project's 32-character id.
    pub project_uid: String,
    /// The latest codebook version the person saved, or `None` when none was saved.
    pub codebook_version: Option<i64>,
    /// True when an excerpt coded with a sub-code also counts for every code above it.
    pub sub_codes_counted: bool,
    /// When the table was made, in UTC: `YYYY-MM-DDThh:mm:ssZ`.
    pub exported: String,
    /// The cases, by id.
    pub case_ids: Vec<i64>,
    pub case_names: Vec<String>,
    /// The case variables, by name.
    pub attributes: Vec<CaseTableAttribute>,
    /// The active codes, by id.
    pub codes: Vec<CaseTableCode>,
    /// How many codes were left out because they are retired or merged into another.
    pub codes_left_out: usize,
}

impl Project {
    /// Gathers the case table: every case, its variables, and for each active code the number of excerpts of the case's documents that
    /// carry it (with the sub-codes' excerpts when `include_descendants`). Fails with a plain reason when the project has no cases.
    pub fn statistics_case_table(&self, include_descendants: bool) -> Result<CaseTable> {
        let info = self.info()?;
        let (case_ids, case_names): (Vec<i64>, Vec<String>) = {
            let mut stmt = self.conn.prepare("SELECT id, name FROM research_case ORDER BY id")?;
            let rows: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
            rows.into_iter().unzip()
        };
        if case_ids.is_empty() {
            return Err(crate::Error::Invalid(
                "This project has no cases, so there is no case table to send. A case is a person, a site or a family: add cases when you import a spreadsheet, or on the Data page.".to_string(),
            ));
        }

        let mut attributes = Vec::new();
        let rows: Vec<(i64, String, String, String, String)> = {
            let mut stmt =
                self.conn.prepare("SELECT id, name, level, value_labels_json, missing_codes_json FROM attribute WHERE applies_to = 'case' ORDER BY name")?;
            let collected = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<std::result::Result<_, _>>()?;
            collected
        };
        for (id, name, level, labels_json, missing_json) in rows {
            let by_case: HashMap<i64, String> = {
                let mut stmt = self.conn.prepare("SELECT target_id, value FROM attribute_value WHERE attribute_id = ?1")?;
                let pairs: Vec<(i64, Option<String>)> = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
                pairs.into_iter().filter_map(|(case, v)| v.map(|v| (case, v.trim().to_string())).filter(|(_, v)| !v.is_empty())).collect()
            };
            attributes.push(CaseTableAttribute {
                id,
                name,
                level,
                value_labels: serde_json::from_str(&labels_json).unwrap_or_default(),
                missing_codes: serde_json::from_str(&missing_json).unwrap_or_default(),
                values: case_ids.iter().map(|c| by_case.get(c).cloned()).collect(),
            });
        }

        let all = self.list_codes()?;
        let by_id: HashMap<i64, &crate::codebook::CodeRow> = all.iter().map(|c| (c.id, c)).collect();
        let mut active: Vec<&crate::codebook::CodeRow> = all.iter().filter(|c| c.status == "active").collect();
        active.sort_by_key(|c| c.id);
        let codes_left_out = all.len() - active.len();
        let ids: Vec<i64> = active.iter().map(|c| c.id).collect();
        let counts = self.case_code_excerpts(&ids, include_descendants)?;
        let mut codes = Vec::with_capacity(active.len());
        for (at, code) in active.iter().enumerate() {
            // The path from the top of the codebook, stopping at a loop if a damaged file ever had one.
            let mut path = vec![code.name.clone()];
            let mut up = code.parent_id;
            let mut guard = 0;
            while let Some(parent) = up.and_then(|p| by_id.get(&p)) {
                path.push(parent.name.clone());
                up = parent.parent_id;
                guard += 1;
                if guard > 64 {
                    break;
                }
            }
            path.reverse();
            codes.push(CaseTableCode {
                id: code.id,
                path,
                kind: code.kind.clone(),
                excerpts: case_ids.iter().map(|case| counts.get(case).map(|per_code| per_code[at])).collect(),
            });
        }

        let codebook_version: Option<i64> = self.conn.query_row("SELECT MAX(version) FROM codebook_version", [], |r| r.get(0)).optional()?.flatten();
        let exported: String = self.conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |r| r.get(0))?;
        Ok(CaseTable {
            project_name: info.name,
            project_uid: info.project_uid,
            codebook_version,
            sub_codes_counted: include_descendants,
            exported,
            case_ids,
            case_names,
            attributes,
            codes,
            codes_left_out,
        })
    }
}
