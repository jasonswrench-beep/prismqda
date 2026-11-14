//! Joint displays that set a code beside the other cases (spec 14.2), worked out from the project alone and kept nowhere.
//!
//! * The **side-by-side display** puts, for each topic (a code), what people said (how many excerpts, the first few quoted) in one column
//!   and what the numbers say in another: variables of the cases that have the code beside the same variables of the cases with text
//!   that do not, with the difference between the two means.
//! * The **deviation table** takes the groups of one variable of categories and says, for each group, which codes its cases have more
//!   often than all the other cases (the most typical) and less often (the least typical), with the adjusted standardized residual of
//!   the two by two table, which is what R's `chisq.test(..., correct = FALSE)$stdres` gives for it.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use super::joint::{column_of, cut, describe_members, is_missing, two, variable, yes, DisplayQuote, TypologyCase, TypologyCell, TypologyColumn};
use super::{MatrixSpec, RowSpec, Scope};
use crate::project::Project;
use crate::variables::{CaseData, VariableDef};
use crate::{Error, Result};

/// The most quotations a topic shows.
const MOST_QUOTES: usize = 5;

fn five() -> usize {
    5
}

/// For each topic, the distinct excerpts of the cases (a stretch of a document) with the first coding of each and the first case.
type TopicSpans = Vec<BTreeMap<(i64, i64, i64), (i64, i64)>>;

/// The variables a display describes cases with, by name and without repeats; free text cannot be described.
fn describing<'a>(data: &'a CaseData, names: &[String], what: &str) -> Result<Vec<(usize, &'a VariableDef)>> {
    let mut vars: Vec<(usize, &VariableDef)> = Vec::new();
    for n in names {
        let (i, def) = variable(data, n)?;
        if def.level == "text" {
            return Err(Error::Invalid(format!("'{n}' holds free text, which {what} cannot describe")));
        }
        if !vars.iter().any(|(j, _)| *j == i) {
            vars.push((i, def));
        }
    }
    Ok(vars)
}

fn distinct(ids: &[i64]) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    for id in ids {
        if !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

// ---------------------------------------------------------------- the side-by-side display

#[derive(Debug, Clone, Deserialize)]
pub struct SideBySideSpec {
    /// The topics, one row for each.
    pub code_ids: Vec<i64>,
    #[serde(default = "yes")]
    pub include_descendants: bool,
    /// The variables of cases that stand for the numbers.
    pub variables: Vec<String>,
    /// How many quotations a topic shows (the rest are counted); at most five.
    #[serde(default = "two")]
    pub quotes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicQuote {
    #[serde(flatten)]
    pub quote: DisplayQuote,
    pub case_id: i64,
    pub case_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SideGroup {
    pub cases: Vec<TypologyCase>,
    pub cells: Vec<TypologyCell>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SideBySideTopic {
    pub code_id: i64,
    pub name: String,
    /// How many distinct excerpts of the cases carry the code (the same words coded twice count once).
    pub excerpts: usize,
    pub quotes: Vec<TopicQuote>,
    /// How many more excerpts there are than quotations shown.
    pub more: usize,
    /// One coding for each excerpt, in the order of the documents: the evidence behind the count.
    pub coding_ids: Vec<i64>,
    /// The cases that have the code, and the cases with text that do not.
    pub with: SideGroup,
    pub without: SideGroup,
    /// For each variable, the mean of the cases that have the code less the mean of the others (only for numbers, and only when both
    /// groups have a number).
    pub differences: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SideBySide {
    pub columns: Vec<TypologyColumn>,
    pub topics: Vec<SideBySideTopic>,
    pub cases_with_text: usize,
}

impl Project {
    /// For each topic, the words of the cases that have it beside the numbers of those cases and of the others.
    pub fn side_by_side(&self, spec: &SideBySideSpec) -> Result<SideBySide> {
        if spec.code_ids.is_empty() {
            return Err(Error::Invalid("choose at least one code to be a topic".to_string()));
        }
        if spec.variables.is_empty() {
            return Err(Error::Invalid("choose at least one variable to stand for the numbers".to_string()));
        }
        self.side_by_side_any(spec)
    }

    /// The same, with no variable required: just the words of the topics and which cases have them.
    pub(crate) fn side_by_side_any(&self, spec: &SideBySideSpec) -> Result<SideBySide> {
        let code_ids = distinct(&spec.code_ids);
        if code_ids.is_empty() {
            return Err(Error::Invalid("choose at least one code to be a topic".to_string()));
        }
        let data = self.case_data()?;
        let vars = describing(&data, &spec.variables, "a side-by-side display")?;
        let names: Vec<String> = code_ids.iter().map(|c| self.code_name(*c)).collect::<Result<_>>()?;
        let counts = self.case_code_excerpts(&code_ids, spec.include_descendants)?;
        let columns: Vec<TypologyColumn> = vars.iter().map(|(i, def)| column_of(&data, *i, def)).collect();

        // The excerpts of each topic: the distinct stretches that belong to a case, with the first coding of each and the first case.
        let case_names: HashMap<i64, &str> = data.cases.iter().map(|c| (c.id, c.name.as_str())).collect();
        let mspec = MatrixSpec { rows: RowSpec::Cases, code_ids: code_ids.clone(), include_descendants: spec.include_descendants, scope: Scope { layer: Some("coder".to_string()), ..Default::default() } };
        let (hits, _, _) = self.matrix_hits(&mspec)?;
        let mut spans: TopicSpans = vec![BTreeMap::new(); code_ids.len()];
        for (r, keys, columns_hit) in &hits {
            let Some(case) = keys.iter().find_map(|(key, _, _)| key.strip_prefix("case:").and_then(|n| n.parse::<i64>().ok())) else { continue };
            for &c in columns_hit {
                spans[c].entry((r.source_id, r.start, r.end)).or_insert((r.id, case));
            }
        }
        let source_names: HashMap<i64, String> = self.list_sources()?.into_iter().map(|s| (s.id, s.name)).collect();
        let quotes = spec.quotes.min(MOST_QUOTES);

        let group = |members: Vec<usize>| SideGroup {
            cells: describe_members(&data, &vars, &columns, &members),
            cases: members.iter().map(|m| TypologyCase { id: data.cases[*m].id, name: data.cases[*m].name.clone() }).collect(),
        };
        let mut topics = Vec::with_capacity(code_ids.len());
        for (k, (id, name)) in code_ids.iter().zip(&names).enumerate() {
            let with: Vec<usize> = (0..data.cases.len()).filter(|i| counts.get(&data.cases[*i].id).is_some_and(|c| c[k] > 0)).collect();
            let without: Vec<usize> = (0..data.cases.len()).filter(|i| counts.get(&data.cases[*i].id).is_some_and(|c| c[k] == 0)).collect();
            let (with, without) = (group(with), group(without));
            let differences = with
                .cells
                .iter()
                .zip(&without.cells)
                .map(|pair| match pair {
                    (TypologyCell::Scale { mean: Some(a), .. }, TypologyCell::Scale { mean: Some(b), .. }) => Some(a - b),
                    _ => None,
                })
                .collect();
            let mut shown = Vec::new();
            for ((source_id, start, end), (coding_id, case)) in spans[k].iter().take(quotes) {
                let text = if *coding_id < 0 { None } else { Some(cut(&self.text_of_span(*source_id, *start, *end)?)) };
                shown.push(TopicQuote {
                    quote: DisplayQuote { coding_id: *coding_id, source_id: *source_id, source: source_names.get(source_id).cloned().unwrap_or_default(), start: *start, end: *end, text },
                    case_id: *case,
                    case_name: case_names.get(case).copied().unwrap_or_default().to_string(),
                });
            }
            topics.push(SideBySideTopic {
                code_id: *id,
                name: name.clone(),
                excerpts: spans[k].len(),
                more: spans[k].len() - shown.len(),
                quotes: shown,
                coding_ids: spans[k].values().map(|(coding, _)| *coding).collect(),
                with,
                without,
                differences,
            });
        }
        Ok(SideBySide { columns, topics, cases_with_text: counts.len() })
    }
}

impl Project {
    /// One variable of cases described over every case that holds text: its column, the cell that describes the cases, and how many cases.
    pub(crate) fn describe_cases_with_text(&self, name: &str) -> Result<(TypologyColumn, TypologyCell, usize)> {
        let data = self.case_data()?;
        let vars = describing(&data, &[name.to_string()], "a table")?;
        let held = self.cases_with_members()?;
        let members: Vec<usize> = (0..data.cases.len()).filter(|i| held.contains(&data.cases[*i].id)).collect();
        let columns: Vec<TypologyColumn> = vars.iter().map(|(i, def)| column_of(&data, *i, def)).collect();
        let cell = describe_members(&data, &vars, &columns, &members).remove(0);
        Ok((columns.into_iter().next().expect("one variable"), cell, members.len()))
    }
}

// ---------------------------------------------------------------- the deviation table

#[derive(Debug, Clone, Deserialize)]
pub struct DeviationSpec {
    /// The variable of categories whose values make the groups.
    pub variable: String,
    /// The codes to look at.
    pub code_ids: Vec<i64>,
    #[serde(default = "yes")]
    pub include_descendants: bool,
    /// How many codes to list as the most and as the least typical of each group (1 to 20).
    #[serde(default = "five")]
    pub top: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviationRow {
    pub code_id: i64,
    pub name: String,
    /// How many cases of the group have the code, of how many in the group.
    pub in_cases: usize,
    pub in_total: usize,
    /// And how many of all the other cases that have a value and text.
    pub out_cases: usize,
    pub out_total: usize,
    pub in_share: f64,
    pub out_share: f64,
    /// The group's share less the others' share (a share is 0 to 1).
    pub difference: f64,
    /// The adjusted standardized residual of the group's cell, what R's `chisq.test(correct = FALSE)$stdres` gives for the two by two
    /// table of in the group or not and with the code or not; none when everyone or no one has the code.
    pub z: Option<f64>,
    /// The cases of the group that have the code.
    pub cases: Vec<TypologyCase>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviationGroup {
    pub value: String,
    pub label: String,
    /// How many cases are in the group.
    pub cases: usize,
    /// The codes the group has more often than the others, the largest residual first.
    pub most: Vec<DeviationRow>,
    /// The codes the group has less often than the others, the smallest residual first.
    pub least: Vec<DeviationRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Deviation {
    pub variable: String,
    pub groups: Vec<DeviationGroup>,
    /// The cases that hold text and have a value of the variable, which is what every share is a share of.
    pub cases_used: usize,
    /// The cases that hold text and have no value of it, which are left out.
    pub without_value: usize,
    pub codes: usize,
}

/// The adjusted standardized residual of the first cell of a two by two table: `a` of the `row` cases are in it, of `n` in all, where
/// `column` have the property.
fn adjusted_residual(a: usize, row: usize, column: usize, n: usize) -> Option<f64> {
    let (a, row, column, n) = (a as f64, row as f64, column as f64, n as f64);
    let expected = row * column / n;
    let denominator = (expected * (1.0 - row / n) * (1.0 - column / n)).sqrt();
    (denominator > 0.0 && denominator.is_finite()).then(|| (a - expected) / denominator)
}

impl Project {
    /// For each group of one variable, the codes its cases have most and least often compared with all the other cases.
    pub fn deviation_table(&self, spec: &DeviationSpec) -> Result<Deviation> {
        let code_ids = distinct(&spec.code_ids);
        if code_ids.is_empty() {
            return Err(Error::Invalid("choose at least one code to look at".to_string()));
        }
        let data = self.case_data()?;
        let (vi, def) = variable(&data, &spec.variable)?;
        if def.level == "text" {
            return Err(Error::Invalid(format!("'{}' holds free text, which cannot make groups; choose a variable of categories", spec.variable)));
        }
        let names: Vec<String> = code_ids.iter().map(|c| self.code_name(*c)).collect::<Result<_>>()?;
        let counts = self.case_code_excerpts(&code_ids, spec.include_descendants)?;
        let value_of = |i: usize| -> Option<String> {
            let v = data.values[i][vi].as_deref();
            (!is_missing(def, v)).then(|| v.unwrap_or_default().trim().to_string())
        };
        let held: Vec<usize> = (0..data.cases.len()).filter(|i| counts.contains_key(&data.cases[*i].id)).collect();
        let used: Vec<usize> = held.iter().copied().filter(|i| value_of(*i).is_some()).collect();
        let without_value = held.len() - used.len();
        let categories = column_of(&data, vi, def);
        // A number is a category here, one for each value, so a number with many values is refused with the rest.
        let values: Vec<(String, String)> = if categories.kind == "scale" {
            let mut seen: Vec<String> = Vec::new();
            for i in &used {
                let v = value_of(*i).unwrap_or_default();
                if !seen.contains(&v) {
                    seen.push(v);
                }
            }
            seen.sort_by(|a, b| a.parse::<f64>().unwrap_or(0.0).total_cmp(&b.parse::<f64>().unwrap_or(0.0)));
            seen.into_iter().map(|v| (v.clone(), v)).collect()
        } else {
            categories.categories.iter().map(|c| (c.value.clone(), c.label.clone())).collect()
        };
        let values: Vec<(String, String)> = values.into_iter().filter(|(v, _)| used.iter().any(|i| value_of(*i).as_deref() == Some(v.as_str()))).collect();
        if values.len() > 12 {
            return Err(Error::Invalid(format!("'{}' has {} different values, more than a table of groups can show; make groups of them first with Cut in the Data editor", spec.variable, values.len())));
        }
        if values.len() < 2 {
            return Err(Error::Invalid(format!("'{}' needs at least two different values among the cases that hold text to make groups", spec.variable)));
        }
        let top = spec.top.clamp(1, 20);
        let n = used.len();
        let with_total: Vec<usize> = (0..code_ids.len()).map(|k| used.iter().filter(|i| counts[&data.cases[**i].id][k] > 0).count()).collect();

        let mut groups = Vec::with_capacity(values.len());
        for (value, label) in &values {
            let members: Vec<usize> = used.iter().copied().filter(|i| value_of(*i).as_deref() == Some(value.as_str())).collect();
            let (in_total, out_total) = (members.len(), n - members.len());
            let mut rows: Vec<DeviationRow> = Vec::with_capacity(code_ids.len());
            for (k, (id, name)) in code_ids.iter().zip(&names).enumerate() {
                let having: Vec<usize> = members.iter().copied().filter(|i| counts[&data.cases[*i].id][k] > 0).collect();
                let in_cases = having.len();
                let out_cases = with_total[k] - in_cases;
                let (in_share, out_share) = (in_cases as f64 / in_total as f64, out_cases as f64 / out_total as f64);
                rows.push(DeviationRow {
                    code_id: *id,
                    name: name.clone(),
                    in_cases,
                    in_total,
                    out_cases,
                    out_total,
                    in_share,
                    out_share,
                    difference: in_share - out_share,
                    z: adjusted_residual(in_cases, in_total, with_total[k], n),
                    cases: having.iter().map(|m| TypologyCase { id: data.cases[*m].id, name: data.cases[*m].name.clone() }).collect(),
                });
            }
            let order = |a: &DeviationRow, b: &DeviationRow| b.z.unwrap_or(0.0).total_cmp(&a.z.unwrap_or(0.0)).then(b.difference.total_cmp(&a.difference)).then_with(|| super::natural(&a.name).cmp(&super::natural(&b.name)));
            let mut most: Vec<DeviationRow> = rows.iter().filter(|r| r.difference > 0.0).cloned().collect();
            most.sort_by(order);
            most.truncate(top);
            let mut least: Vec<DeviationRow> = rows.iter().filter(|r| r.difference < 0.0).cloned().collect();
            least.sort_by(|a, b| a.z.unwrap_or(0.0).total_cmp(&b.z.unwrap_or(0.0)).then(a.difference.total_cmp(&b.difference)).then_with(|| super::natural(&a.name).cmp(&super::natural(&b.name))));
            least.truncate(top);
            groups.push(DeviationGroup { value: value.clone(), label: label.clone(), cases: in_total, most, least });
        }
        Ok(Deviation { variable: spec.variable.clone(), groups, cases_used: n, without_value, codes: code_ids.len() })
    }
}
