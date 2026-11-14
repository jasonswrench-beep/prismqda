//! The variables of cases, edited: the data editor's side of the project. A variable is defined (its name, level of measurement, value
//! labels, and the values that mean missing), its values are typed in case by case, and new variables are made from others by a formula,
//! a recoding, standardizing, ranking, or cutting into ranges. Every change is one logged event that holds what it changed and what
//! was there before, so a single Undo takes it back whole.

use std::collections::{BTreeMap, HashMap, HashSet};

use prismqda_stats::formula::{evaluate_columns, parse, Value};
use prismqda_stats::transform::{cut, range_labels, rank_values, standardize};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::calendar::{parse_day, parse_number};
use crate::dataprep::{date_part, draw_sample, summarize, DatePart, GroupFunction, SampleSize};
use crate::event;
use crate::project::Project;
use crate::{Error, Result};

const LEVELS: [&str; 6] = ["nominal", "ordinal", "interval", "ratio", "date", "text"];
/// The most cases a transformation is run on in one go, and the most a preview shows.
const PREVIEW_ROWS: usize = 8;

/// A variable of cases as the editor shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDef {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub level: String,
    #[serde(default)]
    pub value_labels: BTreeMap<String, String>,
    #[serde(default)]
    pub missing_codes: Vec<String>,
    /// How the variable was made, when it was made by a transformation (so it can be made again).
    #[serde(default)]
    pub derived: Option<Json>,
}

/// What the editor sets about a variable.
#[derive(Debug, Clone, Deserialize)]
pub struct VariableSpec {
    pub name: String,
    pub level: String,
    #[serde(default)]
    pub value_labels: BTreeMap<String, String>,
    #[serde(default)]
    pub missing_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseName {
    pub id: i64,
    pub name: String,
}

/// Every case and every variable of cases, with the value of each (None where there is none): `values[case][variable]`.
#[derive(Debug, Clone, Serialize)]
pub struct CaseData {
    pub cases: Vec<CaseName>,
    pub variables: Vec<VariableDef>,
    pub values: Vec<Vec<Option<String>>>,
}

/// One value to set (None clears it).
#[derive(Debug, Clone, Deserialize)]
pub struct CellChange {
    pub case_id: i64,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecodeRule {
    /// The values (as written) that are given the new value. A number matches a number written another way (1 and 1.0).
    pub from: Vec<String>,
    /// The new value; empty makes it missing.
    pub to: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Otherwise {
    /// A value no rule names stays as it is.
    #[default]
    Keep,
    /// A value no rule names becomes missing.
    Missing,
}

fn yes() -> bool {
    true
}

/// What a variable made from codes says about a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeMeasure {
    /// 1 when any of the case's documents carries the code, 0 when none does (the case must hold a document).
    Present,
    /// How many distinct excerpts of the case's documents carry the code.
    Count,
    /// Of a set of codes, the one carried by the most excerpts (the names of those that tie, joined by "and"; "None" when there is none).
    Dominant,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformKind {
    /// A variable of cases from the coding (quantitizing): whether a case has a code, how many excerpts carry it, or which of a set of codes it
    /// has most. A case that holds no document has no value. Made again, it follows the coding as it is then.
    FromCodes { codes: Vec<i64>, measure: CodeMeasure, #[serde(default = "yes")] include_descendants: bool },
    /// A new variable from a formula over the others (R's syntax, see `prismqda_stats::formula`).
    Compute { formula: String },
    Recode { source: String, rules: Vec<RecodeRule>, #[serde(default)] otherwise: Otherwise },
    /// The z-scores of a number: how many standard deviations each case is from the mean.
    Standardize { source: String },
    /// The ranks of a number, tied values sharing the average rank.
    Rank { source: String, #[serde(default)] descending: bool },
    /// A number cut into ranges at the cut points; each case gets the number of its range, and the range is the value's label.
    Cut { source: String, cuts: Vec<f64>, #[serde(default = "yes")] right: bool },
    /// One part of a date (its year, month, week, weekday, and so on) of a variable that holds dates.
    DatePart { source: String, part: DatePart },
    /// A random sample of the cases: 1 for a case in the sample and 0 for one that is not. With `within`, the sample is drawn within each
    /// group of that variable (a percent takes its share of every group; a count is shared out by the sizes of the groups). The seed fixes
    /// the sample, so making the variable again gives the same cases unless the cases changed.
    Sample { size: SampleSize, #[serde(default = "one_seed")] seed: u64, #[serde(default)] within: Option<String> },
    /// What a group of cases comes to, given to each case of the group: the mean, sum, median, smallest, largest, standard deviation, or how
    /// many of the group have a value, or how many cases the group has. The groups are the cases that share a value of each variable in `by`;
    /// a case with no value in one of them is in no group and gets none.
    Aggregate { source: String, by: Vec<String>, function: GroupFunction },
}

fn one_seed() -> u64 {
    1
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransformSpec {
    /// The variable to make. A variable of that name is replaced only when `replace` says so.
    pub target: String,
    #[serde(default)]
    pub replace: bool,
    /// The level of measurement of the result, instead of the one the transformation suggests.
    #[serde(default)]
    pub level: Option<String>,
    #[serde(flatten)]
    pub kind: TransformKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransformPreview {
    pub case_name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransformOutcome {
    pub variable: String,
    pub level: String,
    pub created: bool,
    pub cases_with_value: usize,
    pub cases_missing: usize,
    /// The words the new variable's values carry (a cut's ranges), by stored value.
    pub value_labels: BTreeMap<String, String>,
    pub preview: Vec<TransformPreview>,
}

/// A transformation worked out and not yet saved.
struct Computed {
    values: Vec<Option<String>>,
    level: String,
    labels: BTreeMap<String, String>,
}

// ---------------------------------------------------------------- reading

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn row_def(r: &rusqlite::Row) -> rusqlite::Result<(i64, String, String, String, String, Option<String>)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
}

fn make_def(row: (i64, String, String, String, String, Option<String>)) -> VariableDef {
    let (id, name, level, labels, missing, derived) = row;
    VariableDef {
        id,
        name,
        level,
        value_labels: serde_json::from_str(&labels).unwrap_or_default(),
        missing_codes: serde_json::from_str(&missing).unwrap_or_default(),
        derived: derived.and_then(|d| serde_json::from_str(&d).ok()),
    }
}

/// How a value is stored: a number with as many digits as it needs and no more.
fn store_number(x: f64) -> Option<String> {
    x.is_finite().then(|| format!("{x}"))
}

impl Project {
    fn case_variable_defs(&self) -> Result<Vec<VariableDef>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, level, value_labels_json, missing_codes_json, derived_json FROM attribute WHERE applies_to = 'case' ORDER BY id")?;
        let rows = stmt.query_map([], row_def)?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().map(make_def).collect())
    }

    /// Every case and variable of cases with its values, for the data view.
    pub fn case_data(&self) -> Result<CaseData> {
        let variables = self.case_variable_defs()?;
        let mut stmt = self.conn.prepare("SELECT id, name FROM research_case ORDER BY id")?;
        let cases: Vec<CaseName> = stmt.query_map([], |r| Ok(CaseName { id: r.get(0)?, name: r.get(1)? }))?.collect::<std::result::Result<_, _>>()?;
        let row_of: HashMap<i64, usize> = cases.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
        let col_of: HashMap<i64, usize> = variables.iter().enumerate().map(|(i, v)| (v.id, i)).collect();
        let mut values = vec![vec![None; variables.len()]; cases.len()];
        let mut stmt = self.conn.prepare("SELECT av.attribute_id, av.target_id, av.value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id WHERE a.applies_to = 'case'")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?)))? {
            let (attr, case, v) = row?;
            if let (Some(&r), Some(&c)) = (row_of.get(&case), col_of.get(&attr)) {
                values[r][c] = clean(v);
            }
        }
        Ok(CaseData { cases, variables, values })
    }

    fn variable_by_name(&self, name: &str) -> Result<VariableDef> {
        self.case_variable_defs()?.into_iter().find(|v| v.name == name).ok_or_else(|| Error::Invalid(format!("no case variable is named '{name}'")))
    }

    /// A column of values typed for formulas: missing where blank or a missing code; a number where the variable is a scale one (or every
    /// value reads as a number), else a text.
    pub(crate) fn typed_column(def: &VariableDef, texts: &[Option<String>]) -> Vec<Value> {
        let missing_numbers: Vec<f64> = def.missing_codes.iter().filter_map(|m| parse_number(m)).collect();
        let kept: Vec<Option<&String>> = texts
            .iter()
            .map(|t| t.as_ref().filter(|v| !(def.missing_codes.iter().any(|m| m.trim() == v.as_str()) || parse_number(v).is_some_and(|x| missing_numbers.contains(&x)))))
            .collect();
        let scale = matches!(def.level.as_str(), "interval" | "ratio");
        let all_numbers = kept.iter().flatten().all(|v| parse_number(v).is_some());
        let numeric = scale || (all_numbers && !matches!(def.level.as_str(), "date" | "text"));
        kept.iter()
            .map(|v| match v {
                None => Value::Na,
                Some(v) if numeric => parse_number(v).map_or(Value::Na, Value::Num),
                Some(v) => Value::Text((*v).clone()),
            })
            .collect()
    }

    /// The group each case is in by one variable: its value (a number written the way numbers are stored), or nothing when it has none.
    pub(crate) fn group_keys(def: &VariableDef, texts: &[Option<String>]) -> Vec<Option<String>> {
        Self::typed_column(def, texts)
            .into_iter()
            .map(|v| match v {
                Value::Na => None,
                Value::Num(x) => store_number(x),
                Value::Bool(b) => Some(if b { "1" } else { "0" }.to_string()),
                Value::Text(t) => Some(t),
            })
            .collect()
    }

    // ---------------------------------------------------------------- defining

    fn check_spec(&self, spec: &VariableSpec, existing: Option<&VariableDef>) -> Result<VariableDef> {
        let name = spec.name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a variable needs a name".to_string()));
        }
        if name.chars().count() > 100 || name.contains(['`', '\n', '\r']) {
            return Err(Error::Invalid("a variable's name can be up to 100 characters, with no backtick or line break in it".to_string()));
        }
        if !LEVELS.contains(&spec.level.as_str()) {
            return Err(Error::Invalid(format!("the level of measurement must be one of {}", LEVELS.join(", "))));
        }
        let wanted = name.to_lowercase();
        if let Some(clash) = self.case_variable_defs()?.into_iter().find(|v| v.name.to_lowercase() == wanted && existing.is_none_or(|e| e.id != v.id)) {
            return Err(Error::Invalid(format!("there is already a variable named '{}'", clash.name)));
        }
        let missing: Vec<String> = {
            let mut seen = HashSet::new();
            spec.missing_codes.iter().map(|m| m.trim().to_string()).filter(|m| !m.is_empty() && seen.insert(m.clone())).collect()
        };
        if missing.len() > 3 {
            return Err(Error::Invalid(format!("a variable can have up to 3 values that mean missing, and {} were given", missing.len())));
        }
        let labels: BTreeMap<String, String> = spec.value_labels.iter().map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).filter(|(k, _)| !k.is_empty()).collect();
        Ok(VariableDef { id: existing.map_or(0, |e| e.id), name: name.to_string(), level: spec.level.clone(), value_labels: labels, missing_codes: missing, derived: existing.and_then(|e| e.derived.clone()) })
    }

    /// Adds a variable of cases, or (with `existing`, the name it has now) changes its name, level, value labels, and missing values.
    pub fn define_variable(&mut self, spec: &VariableSpec, existing: Option<&str>) -> Result<VariableDef> {
        self.ensure_writable()?;
        let before = existing.map(|n| self.variable_by_name(n)).transpose()?;
        let mut def = self.check_spec(spec, before.as_ref())?;
        let tx = self.conn.transaction()?;
        match &before {
            None => {
                tx.execute(
                    "INSERT INTO attribute (name, applies_to, level, value_labels_json, missing_codes_json) VALUES (?1, 'case', ?2, ?3, ?4)",
                    params![def.name, def.level, serde_json::to_string(&def.value_labels)?, serde_json::to_string(&def.missing_codes)?],
                )?;
                def.id = tx.last_insert_rowid();
            }
            Some(b) => {
                // A changed definition that is the same as it was leaves nothing in the log.
                if *b == def {
                    return Ok(def);
                }
                tx.execute(
                    "UPDATE attribute SET name = ?2, level = ?3, value_labels_json = ?4, missing_codes_json = ?5 WHERE id = ?1",
                    params![def.id, def.name, def.level, serde_json::to_string(&def.value_labels)?, serde_json::to_string(&def.missing_codes)?],
                )?;
            }
        }
        event::insert(&tx, &self.actor, &self.session, "variable.apply", Some(("attribute", def.id)), Some(&json!({ "def": before, "values": [] })), Some(&json!({ "def": def, "values": [] })))?;
        tx.commit()?;
        Ok(def)
    }

    /// Takes a variable of cases away, with all its values. Undo brings it back whole.
    pub fn delete_variable(&mut self, name: &str) -> Result<()> {
        self.ensure_writable()?;
        let def = self.variable_by_name(name)?;
        let tx = self.conn.transaction()?;
        let before_values = values_of(&tx, def.id)?;
        tx.execute("DELETE FROM attribute_value WHERE attribute_id = ?1", [def.id])?;
        tx.execute("DELETE FROM attribute WHERE id = ?1", [def.id])?;
        event::insert(&tx, &self.actor, &self.session, "variable.apply", Some(("attribute", def.id)), Some(&json!({ "def": def, "values": before_values })), Some(&json!({ "def": null, "values": [] })))?;
        tx.commit()?;
        Ok(())
    }

    // ---------------------------------------------------------------- values

    /// Sets, changes, or clears the values of a variable for some cases. A scale variable takes a number, a date variable a date
    /// (year-month-day), unless the value is one of the variable's missing values. Returns how many values changed; nothing is logged when
    /// none did.
    pub fn set_values(&mut self, variable: &str, changes: &[CellChange]) -> Result<usize> {
        self.ensure_writable()?;
        let def = self.variable_by_name(variable)?;
        let tx = self.conn.transaction()?;
        let current: HashMap<i64, Option<String>> = values_of(&tx, def.id)?.into_iter().map(|c| (c.case_id, c.value)).collect();
        let case_ids: HashSet<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM research_case")?;
            let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?.collect::<std::result::Result<HashSet<_>, _>>()?;
            ids
        };
        let mut before: Vec<Cell> = Vec::new();
        let mut after: Vec<Cell> = Vec::new();
        for c in changes {
            if !case_ids.contains(&c.case_id) {
                return Err(Error::NotFound { what: "case", id: c.case_id });
            }
            let new = clean(c.value.clone());
            if let Some(v) = &new {
                check_value(&def, v)?;
            }
            let old = clean(current.get(&c.case_id).cloned().flatten());
            if old == new {
                continue;
            }
            before.push(Cell { case_id: c.case_id, value: old });
            after.push(Cell { case_id: c.case_id, value: new.clone() });
            write_cell(&tx, def.id, c.case_id, new.as_deref())?;
        }
        if after.is_empty() {
            return Ok(0);
        }
        let changed = after.len();
        event::insert(&tx, &self.actor, &self.session, "variable.apply", Some(("attribute", def.id)), Some(&json!({ "def": def, "values": before })), Some(&json!({ "def": def, "values": after })))?;
        tx.commit()?;
        Ok(changed)
    }

    // ---------------------------------------------------------------- transformations

    fn compute(&self, spec: &TransformSpec) -> Result<Computed> {
        let data = self.case_data()?;
        let column = |name: &str| -> Result<(usize, &VariableDef)> {
            data.variables.iter().enumerate().find(|(_, v)| v.name == name).ok_or_else(|| Error::Invalid(format!("no case variable is named '{name}'")))
        };
        let texts = |i: usize| -> Vec<Option<String>> { data.values.iter().map(|row| row[i].clone()).collect() };
        let numbers = |name: &str| -> Result<Vec<Option<f64>>> {
            let (i, def) = column(name)?;
            if matches!(def.level.as_str(), "text" | "date") {
                return Err(Error::Invalid(format!("'{name}' holds {}, which this transformation cannot use. It needs numbers.", if def.level == "date" { "dates" } else { "free text" })));
            }
            Ok(Self::typed_column(def, &texts(i)).into_iter().map(|v| if let Value::Num(x) = v { Some(x) } else { None }).collect())
        };
        let level_or = |suggested: &str| -> Result<String> {
            let level = spec.level.clone().unwrap_or_else(|| suggested.to_string());
            if !LEVELS.contains(&level.as_str()) {
                return Err(Error::Invalid(format!("the level of measurement must be one of {}", LEVELS.join(", "))));
            }
            Ok(level)
        };
        let numbered = |xs: Vec<Option<f64>>, level: &str| Computed { values: xs.into_iter().map(|v| v.and_then(store_number)).collect(), level: level.to_string(), labels: BTreeMap::new() };
        match &spec.kind {
            TransformKind::Compute { formula } => {
                let f = parse(formula).map_err(|e| Error::Invalid(match e.at {
                    Some(at) => format!("the formula cannot be read: {} (at character {})", e.message, at + 1),
                    None => format!("the formula cannot be read: {}", e.message),
                }))?;
                let names: Vec<String> = data.variables.iter().map(|v| v.name.clone()).collect();
                let cols: Vec<Vec<Value>> = data.variables.iter().enumerate().map(|(i, v)| Self::typed_column(v, &texts(i))).collect();
                let used = f.variables();
                if let Some(missing) = used.iter().find(|u| !names.contains(u)) {
                    return Err(Error::Invalid(format!("the formula uses '{missing}', which is not a variable of the cases. Variables with spaces in their names are written in backticks, like `Stat age`.")));
                }
                // A formula with no case to work on still has to be a formula.
                if data.cases.is_empty() {
                    return Ok(Computed { values: Vec::new(), level: level_or("interval")?, labels: BTreeMap::new() });
                }
                let results = evaluate_columns(&f, &names, &cols).map_err(|e| Error::Invalid(format!("the formula cannot be worked out: {}", e.message)))?;
                let all_bool = results.iter().filter(|v| !matches!(v, Value::Na)).all(|v| matches!(v, Value::Bool(_))) && results.iter().any(|v| matches!(v, Value::Bool(_)));
                let any_text = results.iter().any(|v| matches!(v, Value::Text(_)));
                let suggested = if all_bool || any_text { "nominal" } else { "interval" };
                let values = results
                    .iter()
                    .map(|v| match v {
                        Value::Na => None,
                        Value::Num(x) => store_number(*x),
                        Value::Bool(b) => Some(if *b { "1" } else { "0" }.to_string()),
                        Value::Text(t) => Some(t.clone()),
                    })
                    .collect();
                Ok(Computed { values, level: level_or(suggested)?, labels: BTreeMap::new() })
            }
            TransformKind::FromCodes { codes, measure, include_descendants } => {
                let mut wanted: Vec<i64> = Vec::new();
                for c in codes {
                    if !wanted.contains(c) {
                        wanted.push(*c);
                    }
                }
                if wanted.is_empty() {
                    return Err(Error::Invalid("choose at least one code".to_string()));
                }
                if *measure != CodeMeasure::Dominant && wanted.len() != 1 {
                    return Err(Error::Invalid("whether a case has a code, and how many excerpts carry it, are measures of one code; to put a set of codes in one variable, choose the code each case has most".to_string()));
                }
                let names: Vec<String> = wanted.iter().map(|c| self.code_name(*c)).collect::<Result<_>>()?;
                let per_case = self.case_code_excerpts(&wanted, *include_descendants)?;
                let values: Vec<Option<String>> = data
                    .cases
                    .iter()
                    .map(|case| {
                        per_case.get(&case.id).map(|counts| match measure {
                            CodeMeasure::Present => if counts[0] > 0 { "1" } else { "0" }.to_string(),
                            CodeMeasure::Count => counts[0].to_string(),
                            CodeMeasure::Dominant => {
                                let top = counts.iter().copied().max().unwrap_or(0);
                                if top == 0 {
                                    "None".to_string()
                                } else {
                                    names.iter().zip(counts).filter(|(_, n)| **n == top).map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(" and ")
                                }
                            }
                        })
                    })
                    .collect();
                match measure {
                    CodeMeasure::Present => Ok(Computed { values, level: level_or("nominal")?, labels: BTreeMap::from([("0".to_string(), "No".to_string()), ("1".to_string(), "Yes".to_string())]) }),
                    CodeMeasure::Count => Ok(Computed { values, level: level_or("ratio")?, labels: BTreeMap::new() }),
                    CodeMeasure::Dominant => Ok(Computed { values, level: level_or("nominal")?, labels: BTreeMap::new() }),
                }
            }
            TransformKind::Standardize { source } => {
                let z = standardize(&numbers(source)?).ok_or_else(|| Error::Invalid(format!("'{source}' has fewer than two values, or all its values are the same, so it has no spread to standardize by")))?;
                Ok(numbered(z, &level_or("interval")?))
            }
            TransformKind::Rank { source, descending } => Ok(numbered(rank_values(&numbers(source)?, *descending), &level_or("interval")?)),
            TransformKind::Cut { source, cuts, right } => {
                let codes = cut(&numbers(source)?, cuts, *right).map_err(Error::Invalid)?;
                let labels = range_labels(cuts, *right);
                Ok(Computed {
                    values: codes.into_iter().map(|c| c.map(|i| (i + 1).to_string())).collect(),
                    level: level_or("ordinal")?,
                    labels: labels.into_iter().enumerate().map(|(i, l)| ((i + 1).to_string(), l)).collect(),
                })
            }
            TransformKind::DatePart { source, part } => {
                let (i, def) = column(source)?;
                if def.level != "date" {
                    return Err(Error::Invalid(format!(
                        "'{source}' is a variable of {} and not of dates, so it has no year or month to take out. A variable of dates has the level 'date' in the variable view; its values are written year-month-day, like 2024-03-14.",
                        match def.level.as_str() {
                            "text" => "free text",
                            "nominal" | "ordinal" => "categories",
                            _ => "numbers",
                        }
                    )));
                }
                let typed = Self::typed_column(def, &texts(i));
                let values = typed
                    .iter()
                    .map(|v| match v {
                        Value::Text(t) => date_part(t, *part).and_then(store_number),
                        _ => None,
                    })
                    .collect();
                Ok(Computed { values, level: level_or(part.level())?, labels: part.labels() })
            }
            TransformKind::Sample { size, seed, within } => {
                let groups: Vec<Option<usize>> = match within {
                    None => vec![Some(0); data.cases.len()],
                    Some(name) => {
                        let (i, def) = column(name)?;
                        let keys = Self::group_keys(def, &texts(i));
                        let mut index: BTreeMap<String, usize> = BTreeMap::new();
                        for k in keys.iter().flatten() {
                            let next = index.len();
                            index.entry(k.clone()).or_insert(next);
                        }
                        keys.iter().map(|k| k.as_ref().map(|k| index[k])).collect()
                    }
                };
                let flags = draw_sample(data.cases.len(), &groups, *size, *seed).map_err(Error::Invalid)?;
                let values = flags.into_iter().map(|f| f.map(|f| if f { "1" } else { "0" }.to_string())).collect();
                let labels = BTreeMap::from([("0".to_string(), "Not in the sample".to_string()), ("1".to_string(), "In the sample".to_string())]);
                Ok(Computed { values, level: level_or("nominal")?, labels })
            }
            TransformKind::Aggregate { source, by, function } => {
                if by.is_empty() {
                    return Err(Error::Invalid("choose at least one variable that makes the groups".to_string()));
                }
                let mut keys: Vec<Vec<Option<String>>> = Vec::new();
                for name in by {
                    let (i, def) = column(name)?;
                    keys.push(Self::group_keys(def, &texts(i)));
                }
                // A case is in a group only when it has a value of every variable that makes the groups.
                let group_of: Vec<Option<String>> = (0..data.cases.len())
                    .map(|c| keys.iter().map(|k| k[c].clone()).collect::<Option<Vec<String>>>().map(|parts| parts.join("\u{1f}")))
                    .collect();
                let values: Vec<Option<f64>> = if function.counts() {
                    let (i, def) = column(source)?;
                    Self::typed_column(def, &texts(i)).into_iter().map(|v| if matches!(v, Value::Na) { None } else { Some(1.0) }).collect()
                } else {
                    numbers(source)?
                };
                let mut members: BTreeMap<&String, Vec<usize>> = BTreeMap::new();
                for (c, g) in group_of.iter().enumerate() {
                    if let Some(g) = g {
                        members.entry(g).or_default().push(c);
                    }
                }
                let mut out: Vec<Option<f64>> = vec![None; data.cases.len()];
                for cases in members.values() {
                    let group_values: Vec<Option<f64>> = cases.iter().map(|&c| values[c]).collect();
                    let result = summarize(&group_values, *function, cases.len());
                    for &c in cases {
                        out[c] = result;
                    }
                }
                Ok(numbered(out, &level_or(function.level())?))
            }
            TransformKind::Recode { source, rules, otherwise } => {
                let (i, def) = column(source)?;
                let raw = texts(i);
                let typed = Self::typed_column(def, &raw);
                let lookup = |v: &str| -> Option<&RecodeRule> {
                    rules.iter().find(|r| r.from.iter().any(|f| f.trim() == v || matches!((parse_number(f), parse_number(v)), (Some(a), Some(b)) if a == b)))
                };
                let values = raw
                    .iter()
                    .zip(&typed)
                    .map(|(r, t)| match (r, t) {
                        (None, _) | (_, Value::Na) => None,
                        (Some(v), _) => match lookup(v) {
                            Some(rule) => clean(Some(rule.to.clone())),
                            None => match otherwise {
                                Otherwise::Keep => Some(v.clone()),
                                Otherwise::Missing => None,
                            },
                        },
                    })
                    .collect();
                let mut labels = BTreeMap::new();
                for rule in rules {
                    // A label the source gave to a value a rule names carries over to the new value when the rule gives it a value.
                    if let Some(l) = rule.from.iter().find_map(|f| def.value_labels.get(f.trim())) {
                        labels.entry(rule.to.trim().to_string()).or_insert_with(|| l.clone());
                    }
                }
                if matches!(otherwise, Otherwise::Keep) {
                    labels.extend(def.value_labels.iter().filter(|(k, _)| !rules.iter().any(|r| r.from.iter().any(|f| f.trim() == k.as_str()))).map(|(k, v)| (k.clone(), v.clone())));
                }
                Ok(Computed { values, level: level_or(&def.level)?, labels })
            }
        }
    }

    /// What a transformation would give, for the first few cases, and how many cases would have a value. Nothing is saved.
    pub fn transform_preview(&self, spec: &TransformSpec) -> Result<TransformOutcome> {
        let computed = self.compute(spec)?;
        let data = self.case_data()?;
        Ok(outcome(spec, &computed, &data.cases, self.case_variable_defs()?.iter().all(|v| v.name != spec.target)))
    }

    /// Makes a variable of cases by a transformation (or, with `replace`, makes it again), saving how it was made with it.
    pub fn transform(&mut self, spec: &TransformSpec) -> Result<TransformOutcome> {
        self.ensure_writable()?;
        let target = spec.target.trim().to_string();
        let existing = self.case_variable_defs()?.into_iter().find(|v| v.name.to_lowercase() == target.to_lowercase());
        if existing.is_some() && !spec.replace {
            return Err(Error::Invalid(format!("there is already a variable named '{}'. Choose another name, or ask to replace it.", existing.map_or(target, |e| e.name))));
        }
        let computed = self.compute(spec)?;
        let data = self.case_data()?;
        // A variable made from itself is allowed (recoding in place); a variable made from one that is being replaced reads the old one.
        let vspec = VariableSpec { name: target.clone(), level: computed.level.clone(), value_labels: computed.labels.clone(), missing_codes: Vec::new() };
        let mut def = self.check_spec(&vspec, existing.as_ref())?;
        def.derived = Some(json!({ "transform": serde_json::to_value(spec)?, "made_with": "formula" }));
        let tx = self.conn.transaction()?;
        let (before_values, created) = match &existing {
            None => {
                tx.execute(
                    "INSERT INTO attribute (name, applies_to, level, value_labels_json, missing_codes_json, derived_json) VALUES (?1, 'case', ?2, ?3, '[]', ?4)",
                    params![def.name, def.level, serde_json::to_string(&def.value_labels)?, def.derived.as_ref().map(Json::to_string)],
                )?;
                def.id = tx.last_insert_rowid();
                (Vec::new(), true)
            }
            Some(e) => {
                let old = values_of(&tx, e.id)?;
                tx.execute(
                    "UPDATE attribute SET name = ?2, level = ?3, value_labels_json = ?4, missing_codes_json = '[]', derived_json = ?5 WHERE id = ?1",
                    params![def.id, def.name, def.level, serde_json::to_string(&def.value_labels)?, def.derived.as_ref().map(Json::to_string)],
                )?;
                tx.execute("DELETE FROM attribute_value WHERE attribute_id = ?1", [def.id])?;
                (old, false)
            }
        };
        let mut after_values: Vec<Cell> = Vec::new();
        for (case, value) in data.cases.iter().zip(&computed.values) {
            if let Some(v) = value {
                write_cell(&tx, def.id, case.id, Some(v))?;
                after_values.push(Cell { case_id: case.id, value: Some(v.clone()) });
            }
        }
        let before_def = existing.as_ref().map(|e| json!(e));
        event::insert(&tx, &self.actor, &self.session, "variable.apply", Some(("attribute", def.id)), Some(&json!({ "def": before_def, "values": before_values })), Some(&json!({ "def": def, "values": after_values })))?;
        tx.commit()?;
        Ok(outcome(spec, &computed, &data.cases, created))
    }

    /// Makes a transformed variable again from how it was made, so it follows a change in the variables it was made from.
    pub fn recompute_variable(&mut self, name: &str) -> Result<TransformOutcome> {
        let def = self.variable_by_name(name)?;
        let stored = def.derived.as_ref().and_then(|d| d.get("transform")).cloned().ok_or_else(|| Error::Invalid(format!("'{name}' was not made by a transformation, so there is nothing to make again")))?;
        let mut spec: TransformSpec = serde_json::from_value(stored)?;
        spec.target = def.name;
        spec.replace = true;
        spec.level = Some(def.level);
        self.transform(&spec)
    }

    /// The cases a rule (a formula that gives true or false) picks, in the order of the cases.
    pub fn filter_cases(&self, formula: &str) -> Result<Vec<i64>> {
        let spec = TransformSpec { target: String::new(), replace: false, level: None, kind: TransformKind::Compute { formula: formula.to_string() } };
        let computed = self.compute(&spec)?;
        let data = self.case_data()?;
        // The formula must give true or false: a number other than 0 and 1 is a calculation, not a rule.
        if computed.values.iter().flatten().any(|v| v != "0" && v != "1") {
            return Err(Error::Invalid("a rule must give true or false for each case, like age >= 18 or site == \"North\", and this formula gives other values".to_string()));
        }
        Ok(data.cases.iter().zip(&computed.values).filter(|(_, v)| v.as_deref() == Some("1")).map(|(c, _)| c.id).collect())
    }
}

fn outcome(spec: &TransformSpec, computed: &Computed, cases: &[CaseName], created: bool) -> TransformOutcome {
    // The first cases that get a value, since those are what the person wants to see; the first cases at all when none does.
    let mut preview: Vec<TransformPreview> = cases.iter().zip(&computed.values).filter(|(_, v)| v.is_some()).take(PREVIEW_ROWS).map(|(c, v)| TransformPreview { case_name: c.name.clone(), value: v.clone() }).collect();
    if preview.is_empty() {
        preview = cases.iter().zip(&computed.values).take(PREVIEW_ROWS).map(|(c, v)| TransformPreview { case_name: c.name.clone(), value: v.clone() }).collect();
    }
    TransformOutcome {
        variable: spec.target.trim().to_string(),
        level: computed.level.clone(),
        created,
        cases_with_value: computed.values.iter().flatten().count(),
        cases_missing: computed.values.iter().filter(|v| v.is_none()).count(),
        value_labels: computed.labels.clone(),
        preview,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cell {
    case_id: i64,
    value: Option<String>,
}

fn values_of(tx: &Transaction, attribute: i64) -> Result<Vec<Cell>> {
    let mut stmt = tx.prepare("SELECT target_id, value FROM attribute_value WHERE attribute_id = ?1 ORDER BY target_id")?;
    let rows = stmt.query_map([attribute], |r| Ok(Cell { case_id: r.get(0)?, value: clean(r.get(1)?) }))?.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn write_cell(tx: &Transaction, attribute: i64, case: i64, value: Option<&str>) -> Result<()> {
    match value {
        Some(v) => tx.execute("INSERT OR REPLACE INTO attribute_value (attribute_id, target_id, value) VALUES (?1, ?2, ?3)", params![attribute, case, v])?,
        None => tx.execute("DELETE FROM attribute_value WHERE attribute_id = ?1 AND target_id = ?2", params![attribute, case])?,
    };
    Ok(())
}

/// A value must suit the level of the variable it goes into, unless it is one of the variable's own missing values.
fn check_value(def: &VariableDef, v: &str) -> Result<()> {
    if def.missing_codes.iter().any(|m| m.trim() == v) {
        return Ok(());
    }
    match def.level.as_str() {
        "interval" | "ratio" if parse_number(v).is_none() => Err(Error::Invalid(format!("'{}' is a scale variable, so it takes a number, and '{v}' is not one. Clear the value to leave it missing.", def.name))),
        "date" if parse_day(v).is_none() => Err(Error::Invalid(format!("'{}' is a date variable, so it takes a date written year-month-day (2024-03-14), and '{v}' is not one.", def.name))),
        _ => Ok(()),
    }
}

/// Reverses a `variable.apply` event: puts the definition and the values back as they were.
pub(crate) fn reverse_apply(tx: &Transaction, attribute_id: i64, before: &Json, after: &Json) -> Result<()> {
    let put_def = |d: &Json| -> Result<()> {
        let labels = d["value_labels"].to_string();
        let missing = d["missing_codes"].to_string();
        let derived = if d["derived"].is_null() { None } else { Some(d["derived"].to_string()) };
        let (name, level) = (d["name"].as_str().unwrap_or_default(), d["level"].as_str().unwrap_or("nominal"));
        let exists: Option<i64> = tx.query_row("SELECT id FROM attribute WHERE id = ?1", [attribute_id], |r| r.get(0)).optional()?;
        if exists.is_some() {
            tx.execute(
                "UPDATE attribute SET name = ?2, level = ?3, value_labels_json = ?4, missing_codes_json = ?5, derived_json = ?6 WHERE id = ?1",
                params![attribute_id, name, level, labels, missing, derived],
            )?;
        } else {
            tx.execute(
                "INSERT INTO attribute (id, name, applies_to, level, value_labels_json, missing_codes_json, derived_json) VALUES (?1, ?2, 'case', ?3, ?4, ?5, ?6)",
                params![attribute_id, name, level, labels, missing, derived],
            )?;
        }
        Ok(())
    };
    let cells = |v: &Json| -> Vec<Cell> { serde_json::from_value(v["values"].clone()).unwrap_or_default() };
    if before["def"].is_null() {
        // The variable was made: take it, and its values, away.
        tx.execute("DELETE FROM attribute_value WHERE attribute_id = ?1", [attribute_id])?;
        let n = tx.execute("DELETE FROM attribute WHERE id = ?1", [attribute_id])?;
        if n == 0 {
            return Err(Error::NotFound { what: "variable", id: attribute_id });
        }
        return Ok(());
    }
    put_def(&before["def"])?;
    if after["def"].is_null() {
        // The variable was deleted: every value comes back.
        tx.execute("DELETE FROM attribute_value WHERE attribute_id = ?1", [attribute_id])?;
    } else if before["def"]["derived"] != after["def"]["derived"] || after["values"].as_array().is_some_and(|a| !a.is_empty()) {
        // A transformation that replaced a variable wrote every value; the ones that were not there before go.
        for c in cells(after) {
            write_cell(tx, attribute_id, c.case_id, None)?;
        }
    }
    for c in cells(before) {
        write_cell(tx, attribute_id, c.case_id, c.value.as_deref())?;
    }
    Ok(())
}
