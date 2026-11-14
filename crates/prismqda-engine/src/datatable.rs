//! Tables the data editor works out and hands back: the cases sorted, a table with a row for each group, a table of the cases stood on end
//! (wide to long) or a table of observations laid out across (long to wide), and variables merged in from a file by a key. None of these
//! makes a second dataset inside the project: rows of Statistics are cases (decision 130), so a table with a row for each group or each
//! observation leaves the project as a file, and a merge adds variables to the cases that are there.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::calendar::{parse_day, parse_number};
use crate::dataprep::{sort_order, summarize, GroupFunction, SortColumn};
use crate::project::Project;
use crate::variables::{CellChange, VariableDef, VariableSpec};
use crate::{Error, Result};

/// The most rows a table made here may have.
pub const MAX_TABLE_ROWS: usize = 200_000;

/// A table of text: the names of its columns, and for each row one value (or nothing) for each column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

impl DataTable {
    fn column(&self, name: &str) -> Result<usize> {
        self.columns.iter().position(|c| c == name).ok_or_else(|| Error::Invalid(format!("the table has no column named '{name}'")))
    }
}

// ---------------------------------------------------------------- stood on end: wide to long

/// What is stood on end.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WideToLong {
    /// The columns kept on every row (who the row is about).
    #[serde(default)]
    pub keep: Vec<String>,
    /// The columns whose values become rows: one row for each of these, for each row of the table.
    pub measures: Vec<String>,
    /// What the new column that says which measure a row came from is called.
    #[serde(default = "default_index_name")]
    pub index_name: String,
    /// What the new column of values is called.
    #[serde(default = "default_value_name")]
    pub value_name: String,
    /// The words that say which measure a row came from, one for each measure in order; the names of the columns when empty.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Leave out a row that has no value.
    #[serde(default)]
    pub drop_missing: bool,
}

fn default_index_name() -> String {
    "Measure".to_string()
}

fn default_value_name() -> String {
    "Value".to_string()
}

/// One row for each measure of each row: the kept columns, which measure it is, and its value.
pub fn wide_to_long(table: &DataTable, spec: &WideToLong) -> Result<DataTable> {
    if spec.measures.len() < 2 {
        return Err(Error::Invalid("choose at least two columns to stand on end: the columns that are the same thing measured more than once".to_string()));
    }
    let (index_name, value_name) = (spec.index_name.trim(), spec.value_name.trim());
    if index_name.is_empty() || value_name.is_empty() || index_name == value_name {
        return Err(Error::Invalid("the two new columns need names that differ".to_string()));
    }
    if spec.keep.iter().chain(&spec.measures).any(|c| c == index_name || c == value_name) {
        return Err(Error::Invalid(format!("'{index_name}' and '{value_name}' are names of columns of the table already; choose other names for the new columns")));
    }
    let keep: Vec<usize> = spec.keep.iter().map(|c| table.column(c)).collect::<Result<_>>()?;
    let measures: Vec<usize> = spec.measures.iter().map(|c| table.column(c)).collect::<Result<_>>()?;
    if let Some(shared) = spec.keep.iter().find(|k| spec.measures.contains(k)) {
        return Err(Error::Invalid(format!("'{shared}' is both kept on every row and stood on end; it can only be one")));
    }
    if !spec.labels.is_empty() && spec.labels.len() != spec.measures.len() {
        return Err(Error::Invalid("give a word for each column stood on end, or none at all".to_string()));
    }
    let labels: Vec<String> = if spec.labels.is_empty() { spec.measures.clone() } else { spec.labels.iter().map(|l| l.trim().to_string()).collect() };
    let mut columns: Vec<String> = spec.keep.clone();
    columns.push(index_name.to_string());
    columns.push(value_name.to_string());
    let mut rows = Vec::new();
    for row in &table.rows {
        for (m, &c) in measures.iter().enumerate() {
            let value = row.get(c).cloned().flatten().filter(|v| !v.trim().is_empty());
            if value.is_none() && spec.drop_missing {
                continue;
            }
            let mut out: Vec<Option<String>> = keep.iter().map(|&k| row.get(k).cloned().flatten()).collect();
            out.push(Some(labels[m].clone()));
            out.push(value);
            rows.push(out);
            if rows.len() > MAX_TABLE_ROWS {
                return Err(Error::Invalid(format!("standing the table on end would make more than {MAX_TABLE_ROWS} rows")));
            }
        }
    }
    Ok(DataTable { columns, rows })
}

// ---------------------------------------------------------------- laid across: long to wide

/// What is laid across.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LongToWide {
    /// The columns that say who a row is about: rows that share them are one row of the result.
    pub id: Vec<String>,
    /// The column that says which observation a row is (a time, a wave, a condition): each of its values becomes columns.
    pub index: String,
    /// The columns of values to lay across.
    pub values: Vec<String>,
}

/// One row for each different combination of the id columns, with the values laid across in one column for each observation (and for each
/// value column, when there are several). A row that has the same id and the same observation as another is refused, named, because
/// which of them to keep is not for the program to guess.
pub fn long_to_wide(table: &DataTable, spec: &LongToWide) -> Result<DataTable> {
    if spec.id.is_empty() || spec.values.is_empty() {
        return Err(Error::Invalid("choose the column that says who a row is about, the column that says which observation it is, and the column of values".to_string()));
    }
    let id: Vec<usize> = spec.id.iter().map(|c| table.column(c)).collect::<Result<_>>()?;
    let index = table.column(&spec.index)?;
    let values: Vec<usize> = spec.values.iter().map(|c| table.column(c)).collect::<Result<_>>()?;
    if spec.id.contains(&spec.index) || spec.values.contains(&spec.index) || spec.id.iter().any(|i| spec.values.contains(i)) {
        return Err(Error::Invalid("a column can only be one of: who the row is about, which observation it is, or the values".to_string()));
    }
    let key_of = |row: &Vec<Option<String>>| -> Vec<Option<String>> { id.iter().map(|&c| row.get(c).cloned().flatten().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())).collect() };
    // The observations and the ids, in the order they first appear.
    let mut observations: Vec<String> = Vec::new();
    let mut ids: Vec<Vec<Option<String>>> = Vec::new();
    let mut id_at: HashMap<Vec<Option<String>>, usize> = HashMap::new();
    let mut seen: HashMap<(usize, usize), usize> = HashMap::new();
    let mut cells: HashMap<(usize, usize, usize), Option<String>> = HashMap::new();
    for (r, row) in table.rows.iter().enumerate() {
        let Some(obs) = row.get(index).cloned().flatten().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) else { continue };
        let o = match observations.iter().position(|x| *x == obs) {
            Some(o) => o,
            None => {
                observations.push(obs.clone());
                observations.len() - 1
            }
        };
        let key = key_of(row);
        if key.iter().all(Option::is_none) {
            continue;
        }
        let i = *id_at.entry(key.clone()).or_insert_with(|| {
            ids.push(key.clone());
            ids.len() - 1
        });
        if let Some(first) = seen.insert((i, o), r) {
            let who = key.iter().map(|k| k.clone().unwrap_or_default()).collect::<Vec<_>>().join(", ");
            return Err(Error::Invalid(format!(
                "{who} has two rows for {obs} (rows {} and {} of the table). Remove or merge one of them first, because a table laid across has room for one value.",
                first + 1,
                r + 1
            )));
        }
        for (v, &c) in values.iter().enumerate() {
            cells.insert((i, o, v), row.get(c).cloned().flatten().filter(|x| !x.trim().is_empty()));
        }
    }
    let mut columns: Vec<String> = spec.id.clone();
    for obs in &observations {
        for (v, name) in spec.values.iter().enumerate() {
            columns.push(if spec.values.len() == 1 { obs.clone() } else { format!("{name} {obs}") });
            let _ = v;
        }
    }
    if let Some(dup) = columns.iter().enumerate().find(|(i, c)| columns[..*i].contains(c)) {
        return Err(Error::Invalid(format!("laying the table across would make two columns named '{}'", dup.1)));
    }
    let mut rows = Vec::with_capacity(ids.len());
    for (i, key) in ids.iter().enumerate() {
        let mut out: Vec<Option<String>> = key.clone();
        for o in 0..observations.len() {
            for v in 0..values.len() {
                out.push(cells.get(&(i, o, v)).cloned().flatten());
            }
        }
        rows.push(out);
    }
    Ok(DataTable { columns, rows })
}

// ---------------------------------------------------------------- the project's side

/// One key of a sort: the name of a variable of cases (or `Case` for the case's own name), and which way.
#[derive(Debug, Clone, Deserialize)]
pub struct SortKey {
    pub variable: String,
    #[serde(default)]
    pub descending: bool,
}

/// What to summarize for the table with a row for each group.
#[derive(Debug, Clone, Deserialize)]
pub struct Summary {
    pub source: String,
    pub function: GroupFunction,
}

/// A table with a row for each group of cases.
#[derive(Debug, Clone, Deserialize)]
pub struct GroupTable {
    pub by: Vec<String>,
    pub summaries: Vec<Summary>,
}

fn numeric_level(def: &VariableDef) -> bool {
    matches!(def.level.as_str(), "interval" | "ratio")
}

fn show(x: f64) -> String {
    format!("{x}")
}

impl Project {
    /// The numbers of the cases in the order the keys give (the first key first, the next breaking its ties, and the order the cases have to
    /// break the rest); a case with no value comes last. The cases are not moved: this is the order to show them in.
    pub fn sorted_case_ids(&self, keys: &[SortKey]) -> Result<Vec<i64>> {
        let data = self.case_data()?;
        let mut columns = Vec::with_capacity(keys.len());
        for key in keys {
            if key.variable == "Case" {
                columns.push(SortColumn { numbers: None, texts: data.cases.iter().map(|c| Some(c.name.clone())).collect(), descending: key.descending });
                continue;
            }
            let (i, def) = data
                .variables
                .iter()
                .enumerate()
                .find(|(_, v)| v.name == key.variable)
                .ok_or_else(|| Error::Invalid(format!("no case variable is named '{}'", key.variable)))?;
            let raw: Vec<Option<String>> = data.values.iter().map(|row| row[i].clone()).collect();
            let kept: Vec<Option<String>> = raw
                .into_iter()
                .map(|v| v.filter(|v| !(def.missing_codes.iter().any(|m| m.trim() == v.as_str()) || def.missing_codes.iter().any(|m| matches!((parse_number(m), parse_number(v)), (Some(a), Some(b)) if a == b)))))
                .collect();
            // Numbers are put in numeric order; a date by its day, so a date written 2024-3-9 comes before 2024-03-14; the rest by their words.
            let all_numbers = kept.iter().flatten().all(|v| parse_number(v).is_some());
            let numbers = if numeric_level(def) || (all_numbers && !matches!(def.level.as_str(), "date" | "text")) {
                Some(kept.iter().map(|v| v.as_deref().and_then(parse_number)).collect())
            } else if def.level == "date" {
                // A value that is not a date the engine reads has no place in the order, so it comes last with the cases that have none.
                Some(kept.iter().map(|v| v.as_deref().and_then(parse_day).map(|d| d as f64)).collect())
            } else {
                None
            };
            columns.push(SortColumn { numbers, texts: kept, descending: key.descending });
        }
        Ok(sort_order(data.cases.len(), &columns).into_iter().map(|i| data.cases[i].id).collect())
    }

    /// The variables of the cases as a table, a row for each case: the case's name, then each variable's values as stored.
    pub fn case_table(&self) -> Result<DataTable> {
        let data = self.case_data()?;
        let mut columns = vec!["Case".to_string()];
        columns.extend(data.variables.iter().map(|v| v.name.clone()));
        let rows = data
            .cases
            .iter()
            .zip(&data.values)
            .map(|(case, values)| {
                let mut row = vec![Some(case.name.clone())];
                row.extend(values.iter().cloned());
                row
            })
            .collect();
        Ok(DataTable { columns, rows })
    }

    /// A table with a row for each group of cases (the cases that share a value of every variable in `by`) and the summaries asked for, with how
    /// many cases are in each group first. A case with no value in one of the variables is in no group, and the table says how many were left out.
    pub fn group_table(&self, spec: &GroupTable) -> Result<(DataTable, usize)> {
        if spec.by.is_empty() {
            return Err(Error::Invalid("choose at least one variable that makes the groups".to_string()));
        }
        let data = self.case_data()?;
        let find = |name: &str| -> Result<(usize, &VariableDef)> {
            data.variables.iter().enumerate().find(|(_, v)| v.name == name).ok_or_else(|| Error::Invalid(format!("no case variable is named '{name}'")))
        };
        let texts = |i: usize| -> Vec<Option<String>> { data.values.iter().map(|row| row[i].clone()).collect() };
        let mut keys: Vec<Vec<Option<String>>> = Vec::new();
        for name in &spec.by {
            let (i, def) = find(name)?;
            keys.push(Self::group_keys(def, &texts(i)));
        }
        let mut sources: Vec<Vec<Option<f64>>> = Vec::new();
        for s in &spec.summaries {
            let (i, def) = find(&s.source)?;
            let typed = Self::typed_column(def, &texts(i));
            if s.function.counts() {
                sources.push(typed.iter().map(|v| if matches!(v, prismqda_stats::formula::Value::Na) { None } else { Some(1.0) }).collect());
            } else {
                if matches!(def.level.as_str(), "text" | "date") {
                    return Err(Error::Invalid(format!("'{}' holds {}, which a {} cannot be worked out from. It needs numbers.", s.source, if def.level == "date" { "dates" } else { "free text" }, s.function.word())));
                }
                sources.push(typed.iter().map(|v| if let prismqda_stats::formula::Value::Num(x) = v { Some(*x) } else { None }).collect());
            }
        }
        let mut members: BTreeMap<Vec<String>, Vec<usize>> = BTreeMap::new();
        let mut left_out = 0;
        for c in 0..data.cases.len() {
            match keys.iter().map(|k| k[c].clone()).collect::<Option<Vec<String>>>() {
                Some(g) => members.entry(g).or_default().push(c),
                None => left_out += 1,
            }
        }
        // The groups in the natural order of their values: numbers by size, words by letter.
        let mut groups: Vec<(&Vec<String>, &Vec<usize>)> = members.iter().collect();
        groups.sort_by(|a, b| {
            for (x, y) in a.0.iter().zip(b.0.iter()) {
                let ord = match (parse_number(x), parse_number(y)) {
                    (Some(p), Some(q)) => p.partial_cmp(&q).unwrap_or(std::cmp::Ordering::Equal),
                    _ => x.to_lowercase().cmp(&y.to_lowercase()).then_with(|| x.cmp(y)),
                };
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        });
        if groups.len() > MAX_TABLE_ROWS {
            return Err(Error::Invalid(format!("these variables make more than {MAX_TABLE_ROWS} groups")));
        }
        let mut columns: Vec<String> = spec.by.clone();
        columns.push("Cases".to_string());
        for s in &spec.summaries {
            columns.push(if s.function == GroupFunction::Cases { "Cases in the group".to_string() } else { format!("{} of {}", s.function.word(), s.source) });
        }
        // Two summaries asked for twice would make two columns of one name.
        if let Some(dup) = columns.iter().enumerate().find(|(i, c)| columns[..*i].contains(c)) {
            return Err(Error::Invalid(format!("'{}' is asked for twice", dup.1)));
        }
        let mut rows = Vec::with_capacity(groups.len());
        for (g, cases) in groups {
            let mut row: Vec<Option<String>> = g.iter().map(|v| Some(v.clone())).collect();
            row.push(Some(cases.len().to_string()));
            for (s, values) in spec.summaries.iter().zip(&sources) {
                let group_values: Vec<Option<f64>> = cases.iter().map(|&c| values[c]).collect();
                row.push(summarize(&group_values, s.function, cases.len()).map(show));
            }
            rows.push(row);
        }
        Ok((DataTable { columns, rows }, left_out))
    }
}

// ---------------------------------------------------------------- splitting the file by groups

/// The most groups an analysis is split into.
pub const MAX_SPLIT_GROUPS: usize = 30;

/// One group of cases an analysis can be run on by itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SplitGroup {
    /// The value of the variable that makes the group, as stored.
    pub value: String,
    /// The value's label, or the value itself.
    pub label: String,
    /// How many cases have this value.
    pub cases: usize,
    /// The rule (a formula, as the rule box of Statistics reads it) that picks exactly these cases.
    pub rule: String,
}

/// The groups an analysis is split into, with the rule that picks each.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SplitPlan {
    pub variable: String,
    pub groups: Vec<SplitGroup>,
    /// Cases with no value in the variable, which are in no group.
    pub left_out: usize,
}

/// A formula's backtick name and quoted text, so a rule can be written for any variable and value.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Project {
    /// The groups of cases that share a value of a variable, each with the rule that picks it, so the same analysis can be run on each group
    /// by itself (what other packages call splitting the file). The groups are in the natural order of their values.
    pub fn split_plan(&self, variable: &str) -> Result<SplitPlan> {
        let data = self.case_data()?;
        let (i, def) = data.variables.iter().enumerate().find(|(_, v)| v.name == variable).ok_or_else(|| Error::Invalid(format!("no case variable is named '{variable}'")))?;
        if matches!(def.level.as_str(), "text" | "date") {
            return Err(Error::Invalid(format!("'{variable}' holds {}, which cannot make groups. Choose a variable of categories or of a few whole numbers.", if def.level == "date" { "dates" } else { "free text" })));
        }
        let raw: Vec<Option<String>> = data.values.iter().map(|row| row[i].clone()).collect();
        let typed = Self::typed_column(def, &raw);
        let numeric = typed.iter().any(|v| matches!(v, prismqda_stats::formula::Value::Num(_))) && typed.iter().all(|v| matches!(v, prismqda_stats::formula::Value::Num(_) | prismqda_stats::formula::Value::Na));
        let keys = Self::group_keys(def, &raw);
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut left_out = 0;
        for k in &keys {
            match k {
                Some(k) => *counts.entry(k.clone()).or_default() += 1,
                None => left_out += 1,
            }
        }
        if counts.is_empty() {
            return Err(Error::Invalid(format!("no case has a value in '{variable}', so there are no groups")));
        }
        if counts.len() > MAX_SPLIT_GROUPS {
            return Err(Error::Invalid(format!("'{variable}' has {} different values. Splitting makes a page for each, and {MAX_SPLIT_GROUPS} is the most; choose a variable with fewer, or cut it into ranges first.", counts.len())));
        }
        let mut values: Vec<(String, usize)> = counts.into_iter().collect();
        values.sort_by(|a, b| match (parse_number(&a.0), parse_number(&b.0)) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()).then_with(|| a.0.cmp(&b.0)),
        });
        let name = format!("`{variable}`");
        let groups = values
            .into_iter()
            .map(|(value, cases)| SplitGroup {
                label: def.value_labels.get(&value).cloned().unwrap_or_else(|| value.clone()),
                rule: if numeric { format!("{name} == {value}") } else { format!("{name} == {}", quoted(&value)) },
                value,
                cases,
            })
            .collect();
        Ok(SplitPlan { variable: variable.to_string(), groups, left_out })
    }
}

// ---------------------------------------------------------------- merging variables from a file by a key

/// One column of a file to bring in as a variable of the cases.
#[derive(Debug, Clone, Deserialize)]
pub struct MergeColumn {
    /// The variable to make (or, with `overwrite`, to fill in).
    pub name: String,
    pub level: String,
    /// The value for each key, in the order of the file's rows.
    pub values: Vec<Option<String>>,
}

/// What to merge: the file's key for each of its rows, and the columns to bring.
#[derive(Debug, Clone, Deserialize)]
pub struct MergeSpec {
    /// The key of each row of the file.
    pub keys: Vec<Option<String>>,
    /// What the keys are matched with: a variable of the cases, or `Case` (the default) for the case's own name.
    #[serde(default)]
    pub match_on: Option<String>,
    pub columns: Vec<MergeColumn>,
    /// Fill in a variable that is already there (for the cases the file has) instead of refusing.
    #[serde(default)]
    pub overwrite: bool,
}

/// What a merge did.
#[derive(Debug, Clone, Serialize)]
pub struct MergeVariablesOutcome {
    /// How many cases had a row in the file.
    pub cases_matched: usize,
    /// How many cases had none (they have no value in the new variables).
    pub cases_unmatched: usize,
    /// The first few of those cases' names.
    pub unmatched_case_names: Vec<String>,
    /// How many rows of the file matched no case, and the first few keys.
    pub rows_unmatched: usize,
    pub unmatched_keys: Vec<String>,
    /// How many rows of the file had no key, and how many had a key an earlier row already had (the first is used).
    pub rows_without_key: usize,
    pub rows_repeated: usize,
    pub variables_made: Vec<String>,
    pub variables_filled: Vec<String>,
    pub values_written: usize,
}

fn key_text(v: &str) -> String {
    v.trim().to_lowercase()
}

impl Project {
    /// Brings variables in from a file's rows, matched to the cases by a key. A case with no row gets no value; a row that matches no case is
    /// counted and named, and brings nothing; two rows with the same key use the first. Each variable made is one step in the history, so
    /// Undo takes them back one at a time.
    pub fn merge_variables(&mut self, spec: &MergeSpec) -> Result<MergeVariablesOutcome> {
        self.ensure_writable()?;
        if spec.columns.is_empty() {
            return Err(Error::Invalid("choose at least one column to bring in".to_string()));
        }
        for c in &spec.columns {
            if c.values.len() != spec.keys.len() {
                return Err(Error::Invalid(format!("the column '{}' does not have a value for each row of the file", c.name)));
            }
        }
        let data = self.case_data()?;
        // What each case is called for the match.
        let case_keys: Vec<Option<String>> = match spec.match_on.as_deref() {
            None | Some("Case") | Some("") => data.cases.iter().map(|c| Some(key_text(&c.name))).collect(),
            Some(name) => {
                let (i, def) = data.variables.iter().enumerate().find(|(_, v)| v.name == name).ok_or_else(|| Error::Invalid(format!("no case variable is named '{name}'")))?;
                let raw: Vec<Option<String>> = data.values.iter().map(|row| row[i].clone()).collect();
                Self::group_keys(def, &raw).into_iter().map(|k| k.map(|k| key_text(&k))).collect()
            }
        };
        let mut case_at: HashMap<&String, Vec<usize>> = HashMap::new();
        for (c, k) in case_keys.iter().enumerate() {
            if let Some(k) = k {
                case_at.entry(k).or_default().push(c);
            }
        }
        // The first row of the file for each key.
        let mut row_for: HashMap<String, usize> = HashMap::new();
        let mut rows_without_key = 0;
        let mut rows_repeated = 0;
        for (r, k) in spec.keys.iter().enumerate() {
            match k.as_deref().map(key_text).filter(|k| !k.is_empty()) {
                None => rows_without_key += 1,
                Some(k) => match row_for.entry(k) {
                    std::collections::hash_map::Entry::Occupied(_) => rows_repeated += 1,
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert(r);
                    }
                },
            }
        }
        let cases_matched: usize = case_keys.iter().flatten().filter(|k| row_for.contains_key(*k)).count();
        let unmatched_cases: Vec<&str> = data.cases.iter().zip(&case_keys).filter(|(_, k)| !k.as_ref().is_some_and(|k| row_for.contains_key(k))).map(|(c, _)| c.name.as_str()).collect();
        // The rows (the first of each key) that match no case, counted, with the first few keys.
        let mut unmatched_keys: Vec<String> = Vec::new();
        let mut rows_unmatched = 0;
        for (r, k) in spec.keys.iter().enumerate() {
            let Some(text) = k.as_deref() else { continue };
            let key = key_text(text);
            if key.is_empty() || row_for.get(&key) != Some(&r) {
                continue;
            }
            if !case_at.contains_key(&key) {
                rows_unmatched += 1;
                if unmatched_keys.len() < 5 {
                    unmatched_keys.push(text.trim().to_string());
                }
            }
        }
        // Nothing is made when nothing would match: that is a wrong key, not a merge.
        if cases_matched == 0 {
            return Err(Error::Invalid(format!(
                "none of the {} rows of the file matches a case. The keys of the file ({}) are matched with {}; check that the column and the spelling are the ones that name the cases.",
                spec.keys.len(),
                spec.keys.iter().flatten().take(3).cloned().collect::<Vec<_>>().join(", "),
                spec.match_on.as_deref().filter(|m| *m != "Case").map_or("the names of the cases".to_string(), |m| format!("the values of {m}"))
            )));
        }
        // Check every column before anything is saved.
        let existing: HashSet<String> = data.variables.iter().map(|v| v.name.to_lowercase()).collect();
        for c in &spec.columns {
            let name = c.name.trim();
            if existing.contains(&name.to_lowercase()) && !spec.overwrite {
                return Err(Error::Invalid(format!("there is already a variable named '{name}'. Rename the column to bring it in, or ask to fill the variable in.")));
            }
            if matches!(c.level.as_str(), "interval" | "ratio") {
                let bad: Vec<&str> = c.values.iter().flatten().map(String::as_str).filter(|v| !v.trim().is_empty() && parse_number(v).is_none()).take(3).collect();
                if !bad.is_empty() {
                    return Err(Error::Invalid(format!("the column '{}' has values that are not numbers ({}), so it cannot be a variable of numbers. Bring it in as categories or as text, or correct the file.", c.name, bad.join(", "))));
                }
            }
            if c.level == "date" {
                let bad: Vec<&str> = c.values.iter().flatten().map(String::as_str).filter(|v| !v.trim().is_empty() && parse_day(v).is_none()).take(3).collect();
                if !bad.is_empty() {
                    return Err(Error::Invalid(format!("the column '{}' has values that are not dates written year-month-day ({}), so it cannot be a variable of dates.", c.name, bad.join(", "))));
                }
            }
        }
        let mut outcome = MergeVariablesOutcome {
            cases_matched,
            cases_unmatched: unmatched_cases.len(),
            unmatched_case_names: unmatched_cases.iter().take(5).map(|s| s.to_string()).collect(),
            rows_unmatched,
            unmatched_keys,
            rows_without_key,
            rows_repeated,
            variables_made: Vec::new(),
            variables_filled: Vec::new(),
            values_written: 0,
        };
        for c in &spec.columns {
            let name = c.name.trim();
            let existing_name = data.variables.iter().find(|v| v.name.to_lowercase() == name.to_lowercase()).map(|v| v.name.clone());
            let target = match &existing_name {
                Some(n) => n.clone(),
                None => {
                    self.define_variable(&VariableSpec { name: name.to_string(), level: c.level.clone(), value_labels: BTreeMap::new(), missing_codes: Vec::new() }, None)?;
                    outcome.variables_made.push(name.to_string());
                    name.to_string()
                }
            };
            if existing_name.is_some() {
                outcome.variables_filled.push(target.clone());
            }
            let mut changes: Vec<CellChange> = Vec::new();
            for (case, k) in data.cases.iter().zip(&case_keys) {
                let Some(r) = k.as_ref().and_then(|k| row_for.get(k)) else { continue };
                let value = c.values[*r].clone().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
                changes.push(CellChange { case_id: case.id, value });
            }
            outcome.values_written += self.set_values(&target, &changes)?;
        }
        Ok(outcome)
    }
}
