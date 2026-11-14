//! Joint displays (spec 14.2) that are worked out from the project alone: nothing in them is kept in the file, and every figure is shown
//! with the n it stands on and can be traced to the cases and the words behind it.
//!
//! * The **case-ordered display** puts the cases in the order of one variable and shows, beside each, what it says in the words of the
//!   coding: for every chosen code, how many distinct excerpts of the case carry it and the first few of them, quoted.
//! * The **typology table** describes groups of cases formed by codes (the cases that have a code), for variables of cases: for a number,
//!   how many cases have a value, the mean and the standard deviation; for categories, how many cases are in each.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{natural, MatrixSpec, RowSpec, Scope};
use crate::project::Project;
use crate::variables::VariableDef;
use crate::{Error, Result};
use prismqda_stats::describe::{mean, variance};

pub(super) fn yes() -> bool {
    true
}

pub(super) fn two() -> usize {
    2
}

/// How many characters of a quotation a cell shows before it is cut.
const QUOTE_CHARS: usize = 240;

/// The most quotations one cell shows.
const MOST_QUOTES: usize = 5;

/// A stored value that is a missing one: nothing, or one of the variable's missing codes.
pub(super) fn is_missing(def: &VariableDef, value: Option<&str>) -> bool {
    match value.map(str::trim) {
        None | Some("") => true,
        Some(v) => def.missing_codes.iter().any(|m| m.trim() == v),
    }
}

pub(super) fn number(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

pub(super) fn variable<'a>(data: &'a crate::variables::CaseData, name: &str) -> Result<(usize, &'a VariableDef)> {
    data.variables.iter().position(|v| v.name == name).map(|i| (i, &data.variables[i])).ok_or_else(|| Error::Invalid(format!("no case variable is named '{name}'")))
}

// ---------------------------------------------------------------- the case-ordered display

#[derive(Debug, Clone, Deserialize)]
pub struct CaseDisplaySpec {
    /// The variable of cases whose values put the cases in order.
    pub sort_by: String,
    #[serde(default)]
    pub descending: bool,
    /// More variables to show beside it.
    #[serde(default)]
    pub variables: Vec<String>,
    pub code_ids: Vec<i64>,
    #[serde(default = "yes")]
    pub include_descendants: bool,
    /// How many quotations a cell shows (the rest are counted); at most five.
    #[serde(default = "two")]
    pub quotes: usize,
    /// Only the cases that have a coding of one of the codes (the default); otherwise every case.
    #[serde(default = "yes")]
    pub only_coded: bool,
    #[serde(default)]
    pub scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisplayQuote {
    pub coding_id: i64,
    pub source_id: i64,
    pub source: String,
    /// Where the words are in the document, to open it there.
    pub start: i64,
    pub end: i64,
    /// The words; none for a coded area of a picture.
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisplayCell {
    /// How many distinct excerpts of the case carry the code (the same words coded twice count once).
    pub excerpts: usize,
    pub quotes: Vec<DisplayQuote>,
    /// How many more excerpts there are than quotations shown.
    pub more: usize,
    /// One coding for each excerpt (the evidence behind the count), in the order of the documents.
    pub coding_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisplayRow {
    pub case_id: i64,
    pub case_name: String,
    /// The case's value of each variable, the one it is ordered by first (None where it has none).
    pub values: Vec<Option<String>>,
    pub cells: Vec<DisplayCell>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisplayCode {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseDisplay {
    pub variables: Vec<VariableDef>,
    pub codes: Vec<DisplayCode>,
    pub rows: Vec<DisplayRow>,
    /// True when the cases are in the order of the numbers the variable holds, false when in the order of its words.
    pub by_number: bool,
    /// How many cases the project has, and how many of those are shown.
    pub cases: usize,
    /// Cases shown that have no value of the variable the display is ordered by (they come last).
    pub without_value: usize,
}

impl Project {
    /// The cases in the order of one variable, each with the words coded with the chosen codes.
    pub fn case_ordered_display(&self, spec: &CaseDisplaySpec) -> Result<CaseDisplay> {
        if spec.code_ids.is_empty() {
            return Err(Error::Invalid("choose at least one code to show".to_string()));
        }
        let data = self.case_data()?;
        let mut names = vec![spec.sort_by.clone()];
        for v in &spec.variables {
            if !names.contains(v) {
                names.push(v.clone());
            }
        }
        let mut at = Vec::new();
        for n in &names {
            let (i, def) = variable(&data, n)?;
            if def.level == "text" && *n == spec.sort_by {
                return Err(Error::Invalid(format!("'{n}' holds free text, which cannot put the cases in order; choose another variable")));
            }
            at.push(i);
        }
        let hier = self.hierarchy()?;
        let mut codes: Vec<DisplayCode> = Vec::new();
        for id in &spec.code_ids {
            if codes.iter().any(|c| c.id == *id) {
                continue;
            }
            let name = hier.names.get(id).ok_or(Error::NotFound { what: "code", id: *id })?.clone();
            codes.push(DisplayCode { id: *id, name });
        }
        let mut scope = spec.scope.clone();
        if scope.layer.is_none() {
            scope.layer = Some("coder".to_string());
        }
        let mspec = MatrixSpec { rows: RowSpec::Cases, code_ids: codes.iter().map(|c| c.id).collect(), include_descendants: spec.include_descendants, scope };
        let (hits, _, _) = self.matrix_hits(&mspec)?;

        // For each case and code, the distinct excerpts (a stretch of a document) and the first coding of each.
        let mut by_case: HashMap<i64, Excerpts> = HashMap::new();
        for (r, keys, columns) in &hits {
            for (key, _, _) in keys {
                let Some(case) = key.strip_prefix("case:").and_then(|n| n.parse::<i64>().ok()) else { continue };
                let cells = by_case.entry(case).or_insert_with(|| vec![BTreeMap::new(); codes.len()]);
                for &c in columns {
                    cells[c].entry((r.source_id, r.start, r.end)).or_insert(r.id);
                }
            }
        }

        // The order.
        let sort_def = &data.variables[at[0]];
        let mut shown: Vec<usize> = (0..data.cases.len()).filter(|i| !spec.only_coded || by_case.contains_key(&data.cases[*i].id)).collect();
        let valued = |i: usize| -> Option<&str> {
            let v = data.values[i][at[0]].as_deref();
            if is_missing(sort_def, v) {
                None
            } else {
                v.map(str::trim)
            }
        };
        let by_number = shown.iter().filter_map(|i| valued(*i)).all(|v| number(v).is_some()) && shown.iter().any(|i| valued(*i).is_some());
        let without_value = shown.iter().filter(|i| valued(**i).is_none()).count();
        shown.sort_by(|a, b| {
            let (va, vb) = (valued(*a), valued(*b));
            let by_name = || natural(&data.cases[*a].name).cmp(&natural(&data.cases[*b].name)).then(data.cases[*a].id.cmp(&data.cases[*b].id));
            match (va, vb) {
                (None, None) => by_name(),
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(x), Some(y)) => {
                    let o = if by_number { number(x).unwrap_or(0.0).total_cmp(&number(y).unwrap_or(0.0)) } else { natural(x).cmp(&natural(y)) };
                    (if spec.descending { o.reverse() } else { o }).then_with(by_name)
                }
            }
        });

        let source_names: HashMap<i64, String> = self.list_sources()?.into_iter().map(|s| (s.id, s.name)).collect();
        let quotes = spec.quotes.min(MOST_QUOTES);
        let mut rows = Vec::with_capacity(shown.len());
        for i in shown {
            let case = &data.cases[i];
            let mut cells = Vec::with_capacity(codes.len());
            for c in 0..codes.len() {
                let spans = by_case.get(&case.id).map(|v| &v[c]);
                let excerpts = spans.map_or(0, BTreeMap::len);
                let mut shown_quotes = Vec::new();
                for ((source_id, start, end), coding_id) in spans.into_iter().flatten().take(quotes) {
                    let text = if *coding_id < 0 { None } else { Some(cut(&self.text_of_span(*source_id, *start, *end)?)) };
                    shown_quotes.push(DisplayQuote { coding_id: *coding_id, source_id: *source_id, source: source_names.get(source_id).cloned().unwrap_or_default(), start: *start, end: *end, text });
                }
                let coding_ids: Vec<i64> = spans.map(|s| s.values().copied().collect()).unwrap_or_default();
                cells.push(DisplayCell { excerpts, more: excerpts - shown_quotes.len(), quotes: shown_quotes, coding_ids });
            }
            rows.push(DisplayRow { case_id: case.id, case_name: case.name.clone(), values: at.iter().map(|c| data.values[i][*c].clone()).collect(), cells });
        }
        Ok(CaseDisplay { variables: at.iter().map(|i| data.variables[*i].clone()).collect(), codes, rows, by_number, cases: data.cases.len(), without_value })
    }
}

/// For each code, the distinct excerpts (a stretch of a document) of one case and the first coding of each.
type Excerpts = Vec<BTreeMap<(i64, i64, i64), i64>>;

/// A quotation as a cell shows it: white space made single, and cut with an ellipsis when long.
pub(super) fn cut(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= QUOTE_CHARS {
        flat
    } else {
        let mut s: String = flat.chars().take(QUOTE_CHARS - 1).collect();
        s.push('\u{2026}');
        s
    }
}

// ---------------------------------------------------------------- the typology table

/// What a variable offers a table: a number (n, mean, standard deviation), or its categories in the order of the values (numbers as numbers).
pub(super) fn column_of(data: &crate::variables::CaseData, i: usize, def: &VariableDef) -> TypologyColumn {
    let scale = def.level == "interval" || def.level == "ratio";
    let mut categories: Vec<TypologyCategory> = Vec::new();
    if !scale {
        let mut seen: Vec<String> = Vec::new();
        for row in &data.values {
            let v = row[i].as_deref();
            if !is_missing(def, v) {
                let v = v.unwrap_or_default().trim().to_string();
                if !seen.contains(&v) {
                    seen.push(v);
                }
            }
        }
        if seen.iter().all(|v| number(v).is_some()) {
            seen.sort_by(|a, b| number(a).unwrap_or(0.0).total_cmp(&number(b).unwrap_or(0.0)));
        } else {
            seen.sort_by_key(|v| natural(v));
        }
        categories = seen.into_iter().map(|v| TypologyCategory { label: def.value_labels.get(&v).cloned().unwrap_or_else(|| v.clone()), value: v }).collect();
    }
    TypologyColumn { name: def.name.clone(), level: def.level.clone(), kind: if scale { "scale" } else { "categories" }.to_string(), categories }
}

/// The cells that describe some of the cases (by index into the case data) with each column: for a number, how many have one, the mean
/// and the standard deviation; for categories, how many have a value and how many have each.
pub(super) fn describe_members(data: &crate::variables::CaseData, vars: &[(usize, &VariableDef)], columns: &[TypologyColumn], members: &[usize]) -> Vec<TypologyCell> {
    vars.iter()
        .zip(columns)
        .map(|((i, def), column)| {
            let values: Vec<&str> = members.iter().filter_map(|m| data.values[*m][*i].as_deref()).filter(|v| !is_missing(def, Some(v))).map(str::trim).collect();
            if column.kind == "scale" {
                let x: Vec<f64> = values.iter().filter_map(|v| number(v)).collect();
                TypologyCell::Scale { n: x.len(), mean: mean(&x), sd: variance(&x).map(f64::sqrt) }
            } else {
                let counts: Vec<usize> = column.categories.iter().map(|c| values.iter().filter(|v| **v == c.value).count()).collect();
                TypologyCell::Categories { n: values.len(), counts }
            }
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypologySpec {
    /// The codes that make the groups: for each, the cases that have it.
    pub code_ids: Vec<i64>,
    #[serde(default = "yes")]
    pub include_descendants: bool,
    /// The variables of cases to describe, in order.
    pub variables: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypologyCategory {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypologyColumn {
    pub name: String,
    pub level: String,
    /// `scale` (a number: n, mean, standard deviation) or `categories` (a count for each).
    pub kind: String,
    pub categories: Vec<TypologyCategory>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypologyCell {
    /// `n` is how many of the group's cases have a number; the standard deviation needs two.
    Scale { n: usize, mean: Option<f64>, sd: Option<f64> },
    /// `counts[k]` is how many of the group's cases have the k-th category; `n` is how many have any value.
    Categories { n: usize, counts: Vec<usize> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypologyCase {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypologyGroup {
    /// `all`, `code:<id>`, or `none`.
    pub key: String,
    pub label: String,
    pub code_id: Option<i64>,
    pub cases: Vec<TypologyCase>,
    pub cells: Vec<TypologyCell>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Typology {
    pub columns: Vec<TypologyColumn>,
    pub groups: Vec<TypologyGroup>,
    /// How many cases hold text, which is what a case needs to have a code at all.
    pub cases_with_text: usize,
}

impl Project {
    /// Groups of cases formed by codes (all of them; the cases that have each code; the cases with text that have none of the codes),
    /// described by variables of cases. A case can be in several groups.
    pub fn typology_table(&self, spec: &TypologySpec) -> Result<Typology> {
        let mut code_ids: Vec<i64> = Vec::new();
        for c in &spec.code_ids {
            if !code_ids.contains(c) {
                code_ids.push(*c);
            }
        }
        if code_ids.is_empty() {
            return Err(Error::Invalid("choose at least one code to make the groups".to_string()));
        }
        if spec.variables.is_empty() {
            return Err(Error::Invalid("choose at least one variable to describe the groups".to_string()));
        }
        let data = self.case_data()?;
        let mut vars: Vec<(usize, &VariableDef)> = Vec::new();
        for n in &spec.variables {
            let (i, def) = variable(&data, n)?;
            if def.level == "text" {
                return Err(Error::Invalid(format!("'{n}' holds free text, which a table of groups cannot describe")));
            }
            if !vars.iter().any(|(j, _)| *j == i) {
                vars.push((i, def));
            }
        }
        let names: Vec<String> = code_ids.iter().map(|c| self.code_name(*c)).collect::<Result<_>>()?;
        let counts = self.case_code_excerpts(&code_ids, spec.include_descendants)?;

        let columns: Vec<TypologyColumn> = vars.iter().map(|(i, def)| column_of(&data, *i, def)).collect();
        let describe = |members: &[usize]| describe_members(&data, &vars, &columns, members);
        let group = |key: String, label: String, code_id: Option<i64>, members: Vec<usize>| -> TypologyGroup {
            let cells = describe(&members);
            TypologyGroup { key, label, code_id, cases: members.iter().map(|m| TypologyCase { id: data.cases[*m].id, name: data.cases[*m].name.clone() }).collect(), cells }
        };

        let mut groups = vec![group("all".to_string(), String::new(), None, (0..data.cases.len()).collect())];
        for (k, (id, name)) in code_ids.iter().zip(&names).enumerate() {
            let members: Vec<usize> = (0..data.cases.len()).filter(|i| counts.get(&data.cases[*i].id).is_some_and(|c| c[k] > 0)).collect();
            groups.push(group(format!("code:{id}"), name.clone(), Some(*id), members));
        }
        let held: HashSet<i64> = counts.keys().copied().collect();
        let none: Vec<usize> = (0..data.cases.len()).filter(|i| held.contains(&data.cases[*i].id) && counts[&data.cases[*i].id].iter().all(|n| *n == 0)).collect();
        groups.push(group("none".to_string(), String::new(), None, none));
        Ok(Typology { columns, groups, cases_with_text: held.len() })
    }
}
