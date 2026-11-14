//! Importing codings that were made elsewhere: a spreadsheet with one row per coding, naming the unit
//! (by the identifier it had in the file it came from), the code, and optionally the coder, a note,
//! and a confidence. Moving finished work into PrismQDA should not mean coding it again.

use std::collections::{BTreeMap, HashMap};

use prismqda_engine::{CoderRole, NewCode, NewCoder, NewCoding, Origin, Project};
use serde::{Deserialize, Serialize};

use crate::error::{ImportError, Result};
use crate::table::{Cell, Table};

/// Who a coder named in the file is. Coders not listed are people.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CoderSpec {
    #[serde(default)]
    pub model: bool,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub model_version: Option<String>,
}

/// Which columns of the coded spreadsheet mean what.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CodingMapping {
    /// The column holding the unit's own identifier, matching the `external_id` of imported units.
    pub unit_id_column: String,
    pub code_column: String,
    /// When absent, every coding goes to `default_coder`.
    #[serde(default)]
    pub coder_column: Option<String>,
    /// The coder to use when there is no coder column.
    #[serde(default)]
    pub default_coder: Option<String>,
    #[serde(default)]
    pub rationale_column: Option<String>,
    #[serde(default)]
    pub confidence_column: Option<String>,
    /// Details for coders that are models, by the name used in the file.
    #[serde(default)]
    pub coders: BTreeMap<String, CoderSpec>,
}

/// What an import of codings did.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct CodingImportReport {
    pub codings: usize,
    pub codes_created: usize,
    pub coders_created: usize,
    /// Rows whose unit identifier matches no imported unit.
    pub unmatched_rows: usize,
    pub unmatched_examples: Vec<String>,
    pub empty_rows: usize,
    /// Rows naming a unit that is left out of coding (for example an interviewer turn).
    pub excluded_unit_rows: usize,
}

/// Stores the codings of a table. Codes and coders that do not exist yet are created first (each
/// logged on its own); then every coding goes in as one all-or-nothing batch, so a rejected row
/// stores no codings at all.
pub fn import_codings(project: &mut Project, table: &Table, mapping: &CodingMapping) -> Result<CodingImportReport> {
    let find = |name: &str| {
        table
            .column_index(name)
            .ok_or_else(|| ImportError::format(format!("there is no column named '{name}'")))
    };
    let unit_col = find(&mapping.unit_id_column)?;
    let code_col = find(&mapping.code_column)?;
    let coder_col = mapping.coder_column.as_deref().map(find).transpose()?;
    let note_col = mapping.rationale_column.as_deref().map(find).transpose()?;
    let conf_col = mapping.confidence_column.as_deref().map(find).transpose()?;
    if coder_col.is_none() && mapping.default_coder.as_deref().is_none_or(|c| c.trim().is_empty()) {
        return Err(ImportError::format("name the column that holds the coder, or say which coder made these codings"));
    }

    let (units, _duplicates) = project.unit_refs_by_external_id()?;
    if units.is_empty() {
        return Err(ImportError::format(
            "no imported unit has an identifier to match; import the text with a unit ID column first",
        ));
    }

    let mut report = CodingImportReport::default();
    let mut code_ids: HashMap<String, i64> = project
        .list_codes()?
        .into_iter()
        .filter(|c| c.status == "active" && c.parent_id.is_none())
        .map(|c| (c.name.to_lowercase(), c.id))
        .collect();
    let mut coder_ids: HashMap<String, i64> = HashMap::new();
    let text_of = |row: &Vec<Cell>, i: usize| row.get(i).map(Cell::text).unwrap_or_default().trim().to_string();

    let mut batch: Vec<NewCoding> = Vec::new();
    for row in &table.rows {
        let unit_key = text_of(row, unit_col);
        let code_name = text_of(row, code_col);
        if unit_key.is_empty() || code_name.is_empty() {
            report.empty_rows += 1;
            continue;
        }
        let Some(unit) = units.get(&unit_key) else {
            report.unmatched_rows += 1;
            if report.unmatched_examples.len() < 5 {
                report.unmatched_examples.push(unit_key);
            }
            continue;
        };
        if unit.excluded {
            report.excluded_unit_rows += 1;
            continue;
        }

        let key = code_name.to_lowercase();
        let code_id = match code_ids.get(&key) {
            Some(id) => *id,
            None => {
                let id = project.add_code(&NewCode::new(code_name.clone()))?;
                report.codes_created += 1;
                code_ids.insert(key, id);
                id
            }
        };

        let coder_name = match coder_col {
            Some(c) => {
                let n = text_of(row, c);
                if n.is_empty() { mapping.default_coder.clone().unwrap_or_default() } else { n }
            }
            None => mapping.default_coder.clone().unwrap_or_default(),
        };
        if coder_name.is_empty() {
            report.empty_rows += 1;
            continue;
        }
        let coder_id = match coder_ids.get(&coder_name) {
            Some(id) => *id,
            None => {
                let spec = mapping.coders.get(&coder_name).cloned().unwrap_or_default();
                let local = format!("import-{}", coder_name.to_lowercase().replace(' ', "-"));
                let existing = project.list_coders()?.into_iter().find(|c| c.local_id == local).map(|c| c.id);
                let id = match existing {
                    Some(id) => id,
                    None => {
                        let mut coder = NewCoder::human(coder_name.clone());
                        coder.local_id = Some(local);
                        if spec.model {
                            coder.role = CoderRole::Model;
                            coder.provider = spec.provider.clone();
                            coder.model_name = Some(spec.model_name.clone().unwrap_or_else(|| coder_name.clone()));
                            coder.model_version = spec.model_version.clone();
                        }
                        report.coders_created += 1;
                        project.add_coder(&coder)?
                    }
                };
                coder_ids.insert(coder_name.clone(), id);
                id
            }
        };

        let mut coding = NewCoding::new(unit.source_id, unit.start, unit.end, code_id, coder_id);
        coding.unit_id = Some(unit.unit_id);
        coding.origin = if mapping.coders.get(&coder_name).is_some_and(|s| s.model) { Origin::AiApplied } else { Origin::Human };
        let mut rationale = String::new();
        if let Some(c) = conf_col {
            let conf = text_of(row, c);
            if !conf.is_empty() {
                rationale.push_str(&format!("Confidence: {conf}. "));
            }
        }
        if let Some(c) = note_col {
            rationale.push_str(&text_of(row, c));
        }
        coding.rationale = rationale.trim().to_string();
        batch.push(coding);
    }
    project.apply_codings(&batch)?;
    report.codings = batch.len();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Column;
    use crate::{import_table, ImportOptions, SurveyMapping};
    use prismqda_engine::{NewProject, OpenOptions};
    use tempfile::TempDir;

    fn t(s: &str) -> Cell {
        Cell::Text(s.to_string())
    }

    fn project_with_units(dir: &TempDir) -> Project {
        let mut o = OpenOptions::new("tester");
        o.backup_on_open = false;
        let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
        let units = Table {
            name: "units".into(),
            columns: vec![Column::new("id"), Column::new("case_id"), Column::new("content")],
            rows: vec![
                vec![t("A-1"), t("A"), t("I felt welcome here.")],
                vec![t("A-2"), t("A"), t("The cost was too high.")],
                vec![t("B-1"), t("B"), t("No comment.")],
            ],
            ..Default::default()
        };
        let mapping = SurveyMapping {
            id_column: Some("case_id".into()),
            unit_id_column: Some("id".into()),
            text_columns: vec!["content".into()],
            ..Default::default()
        };
        p.apply_import(&import_table(&units, &mapping, &ImportOptions::default()).unwrap()).unwrap();
        p
    }

    fn coded() -> Table {
        Table {
            name: "coded".into(),
            columns: ["unit_id", "coder", "code", "confidence", "note"].map(Column::new).to_vec(),
            rows: vec![
                vec![t("A-1"), t("MODEL-A"), t("Welcome"), t("high"), t("explicit feeling")],
                vec![t("A-2"), t("MODEL-A"), t("Cost"), t("medium"), Cell::Empty],
                vec![t("B-1"), t("MODEL-A"), t("welcome"), t("low"), t("case differs")],
                vec![t("Z-9"), t("MODEL-A"), t("Cost"), Cell::Empty, Cell::Empty],
                vec![Cell::Empty, t("MODEL-A"), t("Cost"), Cell::Empty, Cell::Empty],
            ],
            ..Default::default()
        }
    }

    fn mapping() -> CodingMapping {
        let mut m = CodingMapping {
            unit_id_column: "unit_id".into(),
            code_column: "code".into(),
            coder_column: Some("coder".into()),
            rationale_column: Some("note".into()),
            confidence_column: Some("confidence".into()),
            ..Default::default()
        };
        m.coders.insert("MODEL-A".into(), CoderSpec { model: true, provider: Some("Provider A".into()), model_name: Some("Model A".into()), model_version: None });
        m
    }

    #[test]
    fn codings_land_on_the_matching_units_with_a_model_coder() {
        let dir = TempDir::new().unwrap();
        let mut p = project_with_units(&dir);
        let r = import_codings(&mut p, &coded(), &mapping()).unwrap();
        assert_eq!((r.codings, r.codes_created, r.coders_created, r.unmatched_rows, r.empty_rows), (3, 2, 1, 1, 1));
        assert_eq!(r.unmatched_examples, ["Z-9"]);
        let codes = p.list_codes().unwrap();
        assert_eq!(codes.iter().map(|c| (c.name.as_str(), c.usage)).collect::<Vec<_>>(), [("Cost", 1), ("Welcome", 2)], "codes match ignoring case");
        let coder = &p.list_coders().unwrap()[0];
        assert_eq!((coder.role.as_str(), coder.model_name.as_deref(), coder.provider.as_deref()), ("model", Some("Model A"), Some("Provider A")));
        let source = p.list_sources().unwrap()[0].id;
        let codings = p.list_codings(source).unwrap();
        assert_eq!(codings.len(), 3);
        assert!(codings.iter().all(|c| c.origin == "ai_applied"), "a model's codings keep that origin");
        let first = &codings[0];
        assert_eq!(first.rationale, "Confidence: high. explicit feeling");
        // The coding covers exactly its unit.
        let units = p.list_units(source).unwrap();
        assert_eq!((first.start, first.end, first.unit_id), (units[0].start, units[0].end, Some(units[0].id)));
    }

    #[test]
    fn importing_twice_with_the_same_coder_does_not_duplicate_the_coder() {
        let dir = TempDir::new().unwrap();
        let mut p = project_with_units(&dir);
        import_codings(&mut p, &coded(), &mapping()).unwrap();
        let r = import_codings(&mut p, &coded(), &mapping()).unwrap();
        assert_eq!((r.coders_created, r.codes_created), (0, 0));
        assert_eq!(p.list_coders().unwrap().len(), 1);
    }

    #[test]
    fn a_plain_default_coder_is_a_person() {
        let dir = TempDir::new().unwrap();
        let mut p = project_with_units(&dir);
        let m = CodingMapping { unit_id_column: "unit_id".into(), code_column: "code".into(), default_coder: Some("Ada".into()), ..Default::default() };
        import_codings(&mut p, &coded(), &m).unwrap();
        assert_eq!(p.list_coders().unwrap()[0].role, "human");
        let source = p.list_sources().unwrap()[0].id;
        assert!(p.list_codings(source).unwrap().iter().all(|c| c.origin == "human"));
    }

    #[test]
    fn mistakes_are_explained() {
        let dir = TempDir::new().unwrap();
        let mut p = project_with_units(&dir);
        let mut m = mapping();
        m.code_column = "nope".into();
        assert!(import_codings(&mut p, &coded(), &m).unwrap_err().to_string().contains("no column named 'nope'"));
        let no_coder = CodingMapping { unit_id_column: "unit_id".into(), code_column: "code".into(), ..Default::default() };
        assert!(import_codings(&mut p, &coded(), &no_coder).unwrap_err().to_string().contains("coder"));
        // A project with no identified units cannot match anything.
        let dir2 = TempDir::new().unwrap();
        let mut o = OpenOptions::new("t");
        o.backup_on_open = false;
        let mut empty = Project::create(dir2.path().join("e.pqda"), &NewProject::new("E"), &o).unwrap();
        assert!(import_codings(&mut empty, &coded(), &mapping()).unwrap_err().to_string().contains("no imported unit"));
        assert_eq!(p.list_codes().unwrap().len(), 0, "nothing was created by the failed attempts");
    }
}
