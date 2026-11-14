//! Tables: the shape that spreadsheets, survey exports, and SPSS files share, and the mapping that
//! turns a table into sources, cases, and attributes (specification 5 and 7.2).

use std::collections::{BTreeMap, HashMap, HashSet};

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportWarning, ImportedCase, ImportedSource, ImportedUnit, Level,
    SourceType, UnitKind,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::builder::TextBuilder;
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;

/// One cell of a table.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    Number(f64),
    Bool(bool),
    /// An ISO 8601 date or date and time, as text.
    Date(String),
}

impl Cell {
    pub fn is_empty(&self) -> bool {
        match self {
            Cell::Empty => true,
            Cell::Text(s) => s.trim().is_empty(),
            _ => false,
        }
    }

    /// The cell as text. Whole numbers have no decimal point.
    pub fn text(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Text(s) => s.clone(),
            Cell::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Cell::Bool(b) => b.to_string(),
            Cell::Date(d) => d.clone(),
        }
    }
}

/// A column and what the file said about it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Column {
    pub name: String,
    /// A longer description, for example an SPSS variable label.
    pub label: Option<String>,
    /// Level of measurement when the file states it (SPSS does). `None` means: guess from the values.
    pub level: Option<Level>,
    pub value_labels: BTreeMap<String, String>,
    pub missing_codes: Vec<String>,
}

impl Column {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    /// Used to name the sources made from it.
    pub name: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    /// Things the reader could not carry over; they go into the import report of the first source made.
    pub warnings: Vec<ImportWarning>,
}

impl Table {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    fn cell(&self, row: usize, col: usize) -> &Cell {
        self.rows[row].get(col).unwrap_or(&Cell::Empty)
    }

    /// The level of measurement for a column: what the file said, or a guess from the values.
    pub fn level_of(&self, col: usize) -> Level {
        self.columns[col].level.unwrap_or_else(|| self.guess_level(col))
    }

    /// Guesses a level from the values. Only a guess: the person confirms it in the import wizard.
    pub fn guess_level(&self, col: usize) -> Level {
        let mut distinct: HashSet<String> = HashSet::new();
        let (mut non_empty, mut numbers, mut dates, mut bools, mut words) = (0usize, 0usize, 0usize, 0usize, 0usize);
        for r in 0..self.rows.len() {
            let c = self.cell(r, col);
            if c.is_empty() {
                continue;
            }
            non_empty += 1;
            let t = c.text();
            words += t.split_whitespace().count();
            distinct.insert(t);
            match c {
                Cell::Number(_) => numbers += 1,
                Cell::Date(_) => dates += 1,
                Cell::Bool(_) => bools += 1,
                _ => {}
            }
        }
        if non_empty == 0 {
            return Level::Text;
        }
        if dates == non_empty {
            return Level::Date;
        }
        if bools == non_empty {
            return Level::Nominal;
        }
        if numbers == non_empty {
            return if distinct.len() <= 7 { Level::Ordinal } else { Level::Ratio };
        }
        // Sentences are text whatever the count; short repeated values are categories.
        if words >= 5 * non_empty {
            return Level::Text;
        }
        if distinct.len() <= 12 || distinct.len() * 5 <= non_empty {
            Level::Nominal
        } else {
            Level::Text
        }
    }

    /// A summary of each column for the import wizard: what it looks like and how it might be used.
    pub fn preview(&self, max_samples: usize) -> TablePreview {
        let columns = (0..self.columns.len())
            .map(|c| {
                let mut distinct: HashSet<String> = HashSet::new();
                let mut samples: Vec<String> = Vec::new();
                let (mut empty, mut words, mut filled) = (0usize, 0usize, 0usize);
                let mut numeric = true;
                let mut dateish = true;
                for r in 0..self.rows.len() {
                    let cell = self.cell(r, c);
                    if cell.is_empty() {
                        empty += 1;
                        continue;
                    }
                    filled += 1;
                    let t = cell.text();
                    words += t.split_whitespace().count();
                    numeric &= matches!(cell, Cell::Number(_));
                    dateish &= matches!(cell, Cell::Date(_));
                    if samples.len() < max_samples && !samples.contains(&t) {
                        samples.push(t.chars().take(120).collect());
                    }
                    distinct.insert(t);
                }
                let mean_words = if filled == 0 { 0.0 } else { words as f64 / filled as f64 };
                let looks_like = if filled == 0 {
                    "empty"
                } else if numeric {
                    "number"
                } else if dateish {
                    "date"
                } else {
                    "text"
                };
                let name_says_id = {
                    let n = self.columns[c].name.to_lowercase();
                    n.split(|ch: char| !ch.is_alphanumeric())
                        .any(|w| matches!(w, "id" | "pid" | "uid" | "respondent" | "participant" | "subject" | "case"))
                };
                let all_unique = filled > 0 && distinct.len() == filled && empty == 0;
                let suggested_role = if name_says_id && all_unique {
                    "id"
                } else if looks_like == "text" && mean_words >= 4.0 {
                    "text"
                } else {
                    "attribute"
                };
                ColumnPreview {
                    name: self.columns[c].name.clone(),
                    label: self.columns[c].label.clone(),
                    guessed_level: self.level_of(c),
                    looks_like: looks_like.to_string(),
                    distinct: distinct.len(),
                    empty,
                    mean_words,
                    samples,
                    suggested_role: suggested_role.to_string(),
                }
            })
            .collect();
        TablePreview { name: self.name.clone(), row_count: self.rows.len(), columns }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnPreview {
    pub name: String,
    pub label: Option<String>,
    pub guessed_level: Level,
    /// `text`, `number`, `date`, or `empty`.
    pub looks_like: String,
    pub distinct: usize,
    pub empty: usize,
    pub mean_words: f64,
    pub samples: Vec<String>,
    /// `id`, `text`, or `attribute`: what the wizard suggests this column is for.
    pub suggested_role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TablePreview {
    pub name: String,
    pub row_count: usize,
    pub columns: Vec<ColumnPreview>,
}

/// One attribute column and the level of measurement the person chose for it (`None` accepts the guess).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeColumn {
    pub name: String,
    #[serde(default)]
    pub level: Option<Level>,
}

/// What the import wizard collects: which column identifies a respondent, which hold open-ended text,
/// and which describe the respondent.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SurveyMapping {
    /// With an ID column, each distinct ID becomes a case and attribute columns describe the case.
    /// Without one, attribute columns describe each response instead.
    #[serde(default)]
    pub id_column: Option<String>,
    /// Each open-ended column becomes its own source, a "response set".
    pub text_columns: Vec<String>,
    #[serde(default)]
    pub attribute_columns: Vec<AttributeColumn>,
    /// A column holding each row's own identifier (for example "UOC028-0009"). It is kept on each
    /// unit as the `external_id` attribute, so codings made elsewhere can be matched to units later.
    #[serde(default)]
    pub unit_id_column: Option<String>,
    /// Columns that describe each response itself (a comment's author, say), whatever the ID column
    /// does. They are always unit attributes.
    #[serde(default)]
    pub unit_attribute_columns: Vec<AttributeColumn>,
}

impl SurveyMapping {
    /// The wizard's starting point: the suggestions in [`Table::preview`].
    pub fn from_guess(table: &Table) -> Self {
        let preview = table.preview(0);
        let mut m = SurveyMapping::default();
        for c in preview.columns {
            match c.suggested_role.as_str() {
                "id" if m.id_column.is_none() => m.id_column = Some(c.name),
                "text" => m.text_columns.push(c.name),
                _ => m.attribute_columns.push(AttributeColumn { name: c.name, level: None }),
            }
        }
        m
    }
}

/// Turns a table into one source per open-ended column, plus a case per respondent when there is an
/// ID column. Empty responses are skipped and counted in the import report.
pub fn import_table(table: &Table, mapping: &SurveyMapping, opts: &ImportOptions) -> Result<ImportBatch> {
    let find = |name: &str| {
        table
            .column_index(name)
            .ok_or_else(|| ImportError::format(format!("there is no column named '{name}'")))
    };
    if mapping.text_columns.is_empty() {
        return Err(ImportError::format("choose at least one column that holds open-ended text"));
    }
    let id_col = mapping.id_column.as_deref().map(find).transpose()?;
    let unit_id_col = mapping.unit_id_column.as_deref().map(find).transpose()?;
    let text_cols: Vec<usize> = mapping.text_columns.iter().map(|n| find(n)).collect::<Result<_>>()?;
    let mut attr_cols: Vec<(usize, Level)> = Vec::new();
    for a in &mapping.attribute_columns {
        let i = find(&a.name)?;
        attr_cols.push((i, a.level.unwrap_or_else(|| table.level_of(i))));
    }
    let mut unit_attr_cols: Vec<(usize, Level)> = Vec::new();
    for a in &mapping.unit_attribute_columns {
        let i = find(&a.name)?;
        unit_attr_cols.push((i, a.level.unwrap_or_else(|| table.level_of(i))));
    }
    let unit_specs: Vec<AttributeSpec> = unit_attr_cols
        .iter()
        .map(|(i, level)| {
            let col = &table.columns[*i];
            AttributeSpec {
                name: col.name.clone(),
                applies_to: AppliesTo::Unit,
                level: *level,
                value_labels: col.value_labels.clone(),
                missing_codes: col.missing_codes.clone(),
            }
        })
        .collect();
    let attr_applies = if id_col.is_some() { AppliesTo::Case } else { AppliesTo::Unit };
    let attr_specs: Vec<AttributeSpec> = attr_cols
        .iter()
        .map(|(i, level)| {
            let col = &table.columns[*i];
            AttributeSpec {
                name: col.name.clone(),
                applies_to: attr_applies,
                level: *level,
                value_labels: col.value_labels.clone(),
                missing_codes: col.missing_codes.clone(),
            }
        })
        .collect();

    let mut batch = ImportBatch::default();
    // For each source, the table row behind each unit, so cases can be built afterwards.
    let mut rows_of_unit: Vec<Vec<usize>> = Vec::new();

    for &tc in &text_cols {
        let column = &table.columns[tc];
        let mut b = TextBuilder::new();
        let mut units: Vec<ImportedUnit> = Vec::new();
        let mut rows: Vec<usize> = Vec::new();
        let mut skipped = 0usize;
        for r in 0..table.rows.len() {
            let cell = table.cell(r, tc);
            let text = cell.text();
            let text = text.replace('\0', "");
            let text = text.trim();
            if cell.is_empty() || text.is_empty() {
                skipped += 1;
                continue;
            }
            b.paragraph_break();
            let (start, end) = b.push(text);
            let mut unit = ImportedUnit::new(start, end, UnitKind::SurveyResponse);
            unit.attrs.push(("source_row".to_string(), (r + 2).to_string()));
            if let Some(uc) = unit_id_col {
                let id = table.cell(r, uc);
                if !id.is_empty() {
                    unit.attrs.push(("external_id".to_string(), id.text().trim().to_string()));
                }
            }
            for ((i, _), spec) in unit_attr_cols.iter().zip(&unit_specs) {
                let c = table.cell(r, *i);
                if !c.is_empty() {
                    unit.attrs.push((spec.name.clone(), c.text()));
                }
            }
            if id_col.is_none() {
                for ((i, _), spec) in attr_cols.iter().zip(&attr_specs) {
                    let c = table.cell(r, *i);
                    if !c.is_empty() {
                        unit.attrs.push((spec.name.clone(), c.text()));
                    }
                }
            }
            units.push(unit);
            rows.push(r);
        }
        let mut warnings = Vec::new();
        if skipped > 0 {
            warnings.push(ImportWarning::new("empty_responses", format!("{skipped} rows had no text in this column and were skipped")));
        }
        if units.is_empty() {
            batch.sources.push(ImportedSource {
                name: String::new(),
                source_type: SourceType::Survey,
                folder: String::new(),
                text: String::new(),
                runs: vec![],
                units: vec![],
                default_unit_kind: UnitKind::SurveyResponse,
                unit_attributes: vec![],
                original_file_hash: None,
                settings: serde_json::Value::Null,
                warnings: vec![ImportWarning::new("no_text", format!("column '{}' has no text", column.name))],
            });
            rows_of_unit.push(vec![]);
            continue;
        }
        let parsed = b.finish(SourceType::Survey, UnitKind::SurveyResponse);
        let mut unit_attributes = vec![AttributeSpec::new("source_row", AppliesTo::Unit, Level::Text)];
        if unit_id_col.is_some() {
            unit_attributes.push(AttributeSpec::new("external_id", AppliesTo::Unit, Level::Text));
        }
        unit_attributes.extend(unit_specs.iter().cloned());
        if id_col.is_none() {
            unit_attributes.extend(attr_specs.iter().cloned());
        }
        let label = column.label.clone().unwrap_or_else(|| column.name.clone());
        let base = opts.name.clone().unwrap_or_else(|| table.name.clone());
        batch.sources.push(ImportedSource {
            name: if text_cols.len() == 1 && opts.name.is_some() { base } else { format!("{base} - {label}") },
            source_type: opts.source_type.unwrap_or(SourceType::Survey),
            folder: opts.folder.clone(),
            text: parsed.text,
            runs: vec![],
            units,
            default_unit_kind: UnitKind::SurveyResponse,
            unit_attributes,
            original_file_hash: opts.file_hash.clone(),
            settings: json!({
                "table": table.name,
                "text_column": column.name,
                "id_column": mapping.id_column,
                "attribute_columns": mapping.attribute_columns.iter().map(|a| &a.name).collect::<Vec<_>>(),
            }),
            warnings,
        });
        rows_of_unit.push(rows);
    }
    // A column with no text was only a placeholder to keep indexes aligned; drop those now.
    let keep: Vec<bool> = batch.sources.iter().map(|s| !s.name.is_empty()).collect();
    if !keep.iter().any(|k| *k) {
        return Err(ImportError::format("none of the chosen text columns has any text"));
    }
    let skipped_columns: Vec<ImportWarning> = batch
        .sources
        .iter()
        .filter(|s| s.name.is_empty())
        .flat_map(|s| s.warnings.clone())
        .collect();
    let mut index_map: Vec<Option<usize>> = Vec::new();
    let mut next = 0;
    for k in &keep {
        index_map.push(if *k { next += 1; Some(next - 1) } else { None });
    }
    batch.sources.retain(|s| !s.name.is_empty());
    if let Some(first) = batch.sources.first_mut() {
        first.warnings.extend(table.warnings.iter().cloned());
        first.warnings.extend(skipped_columns);
    }

    if let Some(id) = id_col {
        batch.case_attributes = attr_specs;
        let mut order: Vec<String> = Vec::new();
        let mut cases: HashMap<String, ImportedCase> = HashMap::new();
        let mut conflicts = 0usize;
        for r in 0..table.rows.len() {
            let raw = table.cell(r, id);
            let key = if raw.is_empty() { format!("Row {}", r + 2) } else { raw.text().trim().to_string() };
            let case = cases.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                ImportedCase { name: key.clone(), attrs: vec![], members: vec![] }
            });
            for ((i, _), spec) in attr_cols.iter().zip(&batch.case_attributes) {
                let c = table.cell(r, *i);
                if c.is_empty() {
                    continue;
                }
                match case.attrs.iter().find(|(n, _)| n == &spec.name) {
                    None => case.attrs.push((spec.name.clone(), c.text())),
                    Some((_, existing)) if existing != &c.text() => conflicts += 1,
                    Some(_) => {}
                }
            }
        }
        for (old_si, rows) in rows_of_unit.iter().enumerate() {
            let Some(si) = index_map[old_si] else { continue };
            for (ui, &r) in rows.iter().enumerate() {
                let raw = table.cell(r, id);
                let key = if raw.is_empty() { format!("Row {}", r + 2) } else { raw.text().trim().to_string() };
                cases.get_mut(&key).expect("case exists").members.push(CaseMember { source: si, unit: Some(ui) });
            }
        }
        batch.cases = order.into_iter().map(|k| cases.remove(&k).expect("case exists")).collect();
        if conflicts > 0 {
            let w = ImportWarning::new(
                "conflicting_case_values",
                format!("{conflicts} values differed between rows with the same ID; the first value was kept"),
            );
            batch.sources[0].warnings.push(w);
        }
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Cell {
        Cell::Text(s.to_string())
    }

    fn survey() -> Table {
        let mut a = Column::new("age");
        a.missing_codes = vec!["-99".into()];
        Table {
            name: "Survey 1".into(),
            columns: vec![Column::new("respondent"), Column::new("group"), a, Column::new("why"), Column::new("advice")],
            rows: vec![
                vec![t("R1"), t("treatment"), Cell::Number(34.0), t("I felt welcome from the first day and kept coming back."), t("Bring a friend.")],
                vec![t("R2"), t("control"), Cell::Number(-99.0), Cell::Empty, t("   ")],
                vec![t("R3"), t("treatment"), Cell::Number(51.0), t("The cost was the hard part for me and my family overall."), Cell::Empty],
                vec![t("R1"), t("treatment"), Cell::Number(34.0), t("A second answer from the same person about the same thing."), t("Be patient.")],
            ],
            ..Default::default()
        }
    }

    #[test]
    fn levels_are_guessed_from_the_values() {
        let s = survey();
        assert_eq!(s.guess_level(1), Level::Nominal, "few repeated words are categories");
        assert_eq!(s.guess_level(2), Level::Ordinal, "few distinct numbers");
        assert_eq!(s.guess_level(3), Level::Text, "sentences are text, however few there are");
        let ratio = Table {
            columns: vec![Column::new("n")],
            rows: (0..30).map(|i| vec![Cell::Number(i as f64)]).collect(),
            ..Default::default()
        };
        assert_eq!(ratio.guess_level(0), Level::Ratio);
        let dates = Table {
            columns: vec![Column::new("d")],
            rows: vec![vec![Cell::Date("2026-01-02".into())], vec![Cell::Date("2026-02-03".into())]],
            ..Default::default()
        };
        assert_eq!(dates.guess_level(0), Level::Date);
    }

    #[test]
    fn a_stated_level_beats_the_guess() {
        let mut s = survey();
        s.columns[2].level = Some(Level::Ratio);
        assert_eq!(s.level_of(2), Level::Ratio);
    }

    #[test]
    fn the_wizard_suggests_roles() {
        let mut s = survey();
        s.columns[0].name = "respondent_id".into();
        let m = SurveyMapping::from_guess(&s);
        assert_eq!(m.id_column.as_deref(), None, "R1 appears twice, so it is not a unique ID");
        s.rows.pop();
        let m = SurveyMapping::from_guess(&s);
        assert_eq!(m.id_column.as_deref(), Some("respondent_id"));
        assert_eq!(m.text_columns, ["why"]);
        assert!(m.attribute_columns.iter().any(|a| a.name == "group"));
    }

    #[test]
    fn each_text_column_becomes_a_response_set_and_each_id_a_case() {
        let s = survey();
        let mapping = SurveyMapping {
            id_column: Some("respondent".into()),
            text_columns: vec!["why".into(), "advice".into()],
            attribute_columns: vec![
                AttributeColumn { name: "group".into(), level: None },
                AttributeColumn { name: "age".into(), level: Some(Level::Ratio) },
            ],
            ..Default::default()
        };
        let b = import_table(&s, &mapping, &ImportOptions::default()).unwrap();

        assert_eq!(b.sources.len(), 2);
        assert_eq!(b.sources[0].name, "Survey 1 - why");
        assert_eq!(b.sources[0].units.len(), 3, "the empty cell is skipped");
        assert!(b.sources[0].warnings.iter().any(|w| w.kind == "empty_responses"));
        assert_eq!(b.sources[1].units.len(), 2, "blank and empty cells are skipped");
        assert!(b.sources[0].units.iter().all(|u| u.kind == UnitKind::SurveyResponse));
        let first = &b.sources[0];
        let quote: String = first.text.chars().skip(first.units[0].start as usize).take((first.units[0].end - first.units[0].start) as usize).collect();
        assert_eq!(quote, "I felt welcome from the first day and kept coming back.");

        // R1 answered twice, so there are three cases, and R1 holds both of its answers.
        assert_eq!(b.cases.len(), 3);
        let r1 = b.cases.iter().find(|c| c.name == "R1").unwrap();
        assert_eq!(r1.members.iter().filter(|m| m.source == 0).count(), 2);
        assert_eq!(r1.members.iter().filter(|m| m.source == 1).count(), 2);
        assert!(r1.attrs.contains(&("group".to_string(), "treatment".to_string())));
        assert!(r1.attrs.contains(&("age".to_string(), "34".to_string())));

        let age = b.case_attributes.iter().find(|a| a.name == "age").unwrap();
        assert_eq!((age.level, age.applies_to), (Level::Ratio, AppliesTo::Case));
        assert_eq!(age.missing_codes, ["-99"]);
    }

    #[test]
    fn without_an_id_column_attributes_describe_each_response() {
        let s = survey();
        let mapping = SurveyMapping {
            id_column: None,
            text_columns: vec!["why".into()],
            attribute_columns: vec![AttributeColumn { name: "group".into(), level: Some(Level::Nominal) }],
            ..Default::default()
        };
        let b = import_table(&s, &mapping, &ImportOptions::default()).unwrap();
        assert!(b.cases.is_empty());
        assert_eq!(b.sources[0].units.len(), 3);
        assert!(b.sources[0].units[0].attrs.contains(&("group".into(), "treatment".into())));
        assert!(b.sources[0].unit_attributes.iter().any(|a| a.name == "group" && a.applies_to == AppliesTo::Unit));
    }

    #[test]
    fn each_rows_own_identifier_is_kept_on_its_unit() {
        let s = survey();
        let mapping = SurveyMapping {
            id_column: Some("group".into()),
            unit_id_column: Some("respondent".into()),
            text_columns: vec!["why".into()],
            ..Default::default()
        };
        let b = import_table(&s, &mapping, &ImportOptions::default()).unwrap();
        let ids: Vec<String> = b.sources[0]
            .units
            .iter()
            .map(|u| u.attrs.iter().find(|(n, _)| n == "external_id").unwrap().1.clone())
            .collect();
        assert_eq!(ids, ["R1", "R3", "R1"]);
        assert!(b.sources[0].unit_attributes.iter().any(|a| a.name == "external_id"));
        // Two groups, so two cases; the identifier is per unit and does not decide the cases.
        assert_eq!(b.cases.len(), 2);
        let bad = SurveyMapping { unit_id_column: Some("nope".into()), text_columns: vec!["why".into()], ..Default::default() };
        assert!(import_table(&s, &bad, &ImportOptions::default()).is_err());
    }

    #[test]
    fn unit_attribute_columns_describe_each_response_even_with_an_id_column() {
        let s = survey();
        let mapping = SurveyMapping {
            id_column: Some("respondent".into()),
            text_columns: vec!["why".into()],
            attribute_columns: vec![AttributeColumn { name: "group".into(), level: None }],
            unit_attribute_columns: vec![AttributeColumn { name: "age".into(), level: Some(Level::Ratio) }],
            ..Default::default()
        };
        let b = import_table(&s, &mapping, &ImportOptions::default()).unwrap();
        let units = &b.sources[0].units;
        let ages: Vec<&str> = units.iter().map(|u| u.attrs.iter().find(|(n, _)| n == "age").unwrap().1.as_str()).collect();
        assert_eq!(ages, ["34", "51", "34"]);
        let age = b.sources[0].unit_attributes.iter().find(|a| a.name == "age").unwrap();
        assert_eq!((age.applies_to, age.level), (AppliesTo::Unit, Level::Ratio));
        // The case-level attribute is still on the cases, not the units.
        assert!(b.cases.iter().all(|c| c.attrs.iter().any(|(n, _)| n == "group")));
        assert!(!units[0].attrs.iter().any(|(n, _)| n == "group"));
    }

    #[test]
    fn the_source_row_points_back_to_the_spreadsheet_row() {
        let s = survey();
        let mapping = SurveyMapping { text_columns: vec!["why".into()], ..Default::default() };
        let b = import_table(&s, &mapping, &ImportOptions::default()).unwrap();
        let rows: Vec<&str> = b.sources[0].units.iter().map(|u| u.attrs.iter().find(|(n, _)| n == "source_row").unwrap().1.as_str()).collect();
        // Spreadsheet row 1 is the header, so the first data row is row 2.
        assert_eq!(rows, ["2", "4", "5"]);
    }

    #[test]
    fn mistakes_in_the_mapping_are_explained() {
        let s = survey();
        let none = SurveyMapping::default();
        assert!(import_table(&s, &none, &ImportOptions::default()).unwrap_err().to_string().contains("at least one"));
        let bad = SurveyMapping { text_columns: vec!["nope".into()], ..Default::default() };
        assert!(import_table(&s, &bad, &ImportOptions::default()).unwrap_err().to_string().contains("no column named 'nope'"));
        let empty_col = Table {
            name: "T".into(),
            columns: vec![Column::new("a")],
            rows: vec![vec![Cell::Empty]],
            ..Default::default()
        };
        let m = SurveyMapping { text_columns: vec!["a".into()], ..Default::default() };
        assert!(import_table(&empty_col, &m, &ImportOptions::default()).unwrap_err().to_string().contains("none of the chosen"));
    }

    #[test]
    fn conflicting_values_for_one_id_keep_the_first_and_say_so() {
        let mut s = survey();
        s.rows[3][1] = t("control"); // R1 now disagrees with itself on group
        let m = SurveyMapping {
            id_column: Some("respondent".into()),
            text_columns: vec!["why".into()],
            attribute_columns: vec![AttributeColumn { name: "group".into(), level: None }],
            ..Default::default()
        };
        let b = import_table(&s, &m, &ImportOptions::default()).unwrap();
        let r1 = b.cases.iter().find(|c| c.name == "R1").unwrap();
        assert!(r1.attrs.contains(&("group".into(), "treatment".into())));
        assert!(b.sources[0].warnings.iter().any(|w| w.kind == "conflicting_case_values"));
    }

    #[test]
    fn whole_numbers_print_without_a_decimal_point() {
        assert_eq!(Cell::Number(34.0).text(), "34");
        assert_eq!(Cell::Number(2.5).text(), "2.5");
        assert_eq!(Cell::Bool(true).text(), "true");
    }
}
