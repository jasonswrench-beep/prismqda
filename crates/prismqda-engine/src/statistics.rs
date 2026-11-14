//! The Statistics page: the data it works on and the procedures it runs.
//!
//! **Rows are cases, and only cases.** A case is a person (or a site, or a family) with variables; a coding is never a row,
//! because many codings come from one person and a test that treats them as independent counts one person many times. What
//! a person says about a theme enters as one value per case: whether the case's documents carry the code at all, or how
//! many distinct excerpts of them do. That is the rule the interface book states ("never count codings as if they were
//! people") and the reason there is no dataset of codings here.
//!
//! The numbers themselves come from `prismqda-stats`, which is compared with R. This module chooses the cases, sets aside
//! what is missing, shapes the groups, checks the conditions a test depends on, and hands back numbers and facts. It writes
//! no sentences for the person: the interface words every result, so a translator can.

use std::collections::{BTreeMap, HashMap, HashSet};

use prismqda_stats::adjust::Adjustment;
use prismqda_stats::anova::{compare_pairs, one_way, pairwise, PostHoc};
use prismqda_stats::factorial::{two_way, TwoWayError};
use prismqda_stats::scale::cronbach_alpha;
use prismqda_stats::normality::shapiro_wilk;
use prismqda_stats::bootstrap::{bootstrap, bootstrap_groups, bootstrap_many, intervals_from_replicates, BootstrapInterval, BootstrapSpec};
use prismqda_stats::SplitMix64;
use prismqda_stats::contingency::{chi_square, fisher_exact, ordinal_association};
use prismqda_stats::discrete::{binomial_test, goodness_of_fit, mcnemar, median_test, runs_test, sign_test};
use prismqda_stats::kolmogorov::{ks_normal, ks_two_sample};
use prismqda_stats::correlation::{kendall, partial_correlation, pearson, spearman};
use prismqda_stats::describe::{descriptives, percentile_sorted, sorted_finite};
use prismqda_stats::nonparametric::{dunn, friedman, kruskal_wallis, mann_whitney, moses, wilcoxon};
use prismqda_stats::curve::{fit as fit_curve, CurveError, CurveFit, CurveKind};
use prismqda_stats::logistic::{compare as logistic_compare, drop_test, logistic, Logistic, LogisticError};
use prismqda_stats::regression::{compare, dummy_columns, linear, Regression, RegressionError};
use prismqda_stats::ttest::{independent, one_sample, paired};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::calendar::parse_number;
use crate::explore::{natural, MatrixSpec, RowSpec, Scope};
use crate::stat_charts::{box_chart, curves_chart, histogram_chart, means_chart, qq_chart, scatter_chart, scatter_with_line, BarCategory, Chart, Subject, MAX_STACK_COLS, MAX_STACK_ROWS};
use crate::project::Project;
use crate::{Error, Result};

/// The most different values a variable may have and still be offered as a set of categories.
const MAX_CATEGORIES: usize = 50;

/// How a variable's values are to be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Measure {
    Nominal,
    Ordinal,
    /// Interval or ratio: numbers that can be added and averaged.
    Scale,
    Date,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableKind {
    /// A variable the project's cases carry.
    Attribute,
    /// Whether any of the case's documents carry a code (1) or none do (0).
    CodePresent,
    /// How many distinct excerpts of the case's documents carry a code.
    CodeCount,
}

/// One value of a categorical variable, and how many cases have it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Category {
    pub value: String,
    /// The value label the project gives it, or the value itself.
    pub label: String,
    pub count: usize,
}

/// A variable the Statistics page can offer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatVariable {
    /// What to ask for it by: `attr:Name`, `code:12:present`, or `code:12:count`.
    pub key: String,
    pub name: String,
    pub kind: VariableKind,
    pub code_id: Option<i64>,
    pub measure: Measure,
    /// Cases with a usable value.
    pub valid: usize,
    /// Cases with none, or with a value the project marks as missing.
    pub missing: usize,
    /// Of a scale variable, the cases whose value is not a number (they are counted among the missing).
    pub not_a_number: usize,
    /// True when every usable value reads as a number.
    pub numeric: bool,
    pub distinct: usize,
    /// The values, when there are few enough to be categories.
    pub categories: Vec<Category>,
}

impl StatVariable {
    /// True for a variable a test can use. Dates and free text are not tested here.
    pub fn usable(&self) -> bool {
        matches!(self.measure, Measure::Nominal | Measure::Ordinal | Measure::Scale)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StatVariables {
    /// How many cases the project has.
    pub cases: usize,
    /// How many of them have at least one document (a case with none has no codes to count).
    pub cases_with_documents: usize,
    pub variables: Vec<StatVariable>,
}

/// One variable's values, case by case, in the order of the case list.
struct Column {
    var: StatVariable,
    text: Vec<Option<String>>,
    num: Vec<Option<f64>>,
}

/// The cases of a run and the columns it asked for, in the order asked.
struct Frame {
    cases: Vec<(i64, String)>,
    columns: Vec<Column>,
    /// What counting each case by a weight did, when the run is weighted.
    weight: Option<WeightInfo>,
}

/// How many times the cases of a weighted run were counted, and by which variable.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeightInfo {
    /// The variable that says how many times each case is counted.
    pub variable: String,
    /// The cases there were before they were counted by their weights.
    pub cases: usize,
    /// How many cases the analysis counts: the weights added up.
    pub counted: usize,
    /// Cases left out because they have no weight or a weight of 0.
    pub left_out: usize,
}

/// The most cases a weighted analysis may count in all.
const MAX_WEIGHTED: usize = 1_000_000;

fn measure_of(level: &str) -> Measure {
    match level {
        "nominal" => Measure::Nominal,
        "ordinal" => Measure::Ordinal,
        "interval" | "ratio" => Measure::Scale,
        "date" => Measure::Date,
        _ => Measure::Text,
    }
}

fn show_number(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// Fills in the summary of a column from its values.
fn finish(mut var: StatVariable, text: &[Option<String>], num: &[Option<f64>], labels: &BTreeMap<String, String>) -> StatVariable {
    var.valid = text.iter().filter(|t| t.is_some()).count();
    var.missing = text.len() - var.valid;
    var.numeric = var.valid > 0 && text.iter().zip(num).filter(|(t, _)| t.is_some()).all(|(_, n)| n.is_some());
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for v in text.iter().flatten() {
        *counts.entry(v.as_str()).or_default() += 1;
    }
    var.distinct = counts.len();
    if var.usable() && counts.len() <= MAX_CATEGORIES {
        let mut cats: Vec<Category> = counts
            .into_iter()
            .map(|(v, n)| Category { value: v.to_string(), label: labels.get(v).cloned().unwrap_or_else(|| v.to_string()), count: n })
            .collect();
        cats.sort_by_key(|c| natural(&c.value));
        var.categories = cats;
    }
    var
}

impl Project {
    fn stat_case_list(&self, only: Option<&[i64]>) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM research_case ORDER BY id")?;
        let rows: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(match only {
            None => rows,
            Some(ids) => {
                let keep: HashSet<i64> = ids.iter().copied().collect();
                rows.into_iter().filter(|(id, _)| keep.contains(id)).collect()
            }
        })
    }

    /// The cases that hold at least one document or unit.
    pub(crate) fn cases_with_members(&self) -> Result<HashSet<i64>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT case_id FROM case_member")?;
        let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?.collect::<std::result::Result<HashSet<_>, _>>()?;
        Ok(ids)
    }

    fn attribute_column(&self, name: &str, cases: &[(i64, String)]) -> Result<Column> {
        let row: Option<(i64, String, String, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, level, value_labels_json, missing_codes_json FROM attribute WHERE applies_to = 'case' AND name = ?1")?;
            let mut rows = stmt.query_map([name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            rows.next().transpose()?
        };
        let (id, level, labels_json, missing_json) = row.ok_or_else(|| Error::Invalid(format!("no case variable is named '{name}'")))?;
        let labels: BTreeMap<String, String> = serde_json::from_str(&labels_json).unwrap_or_default();
        let missing_codes: Vec<String> = serde_json::from_str(&missing_json).unwrap_or_default();
        let missing_numbers: Vec<f64> = missing_codes.iter().filter_map(|m| parse_number(m)).collect();
        let measure = measure_of(&level);
        let mut stmt = self.conn.prepare("SELECT target_id, value FROM attribute_value WHERE attribute_id = ?1")?;
        let values: HashMap<i64, String> = stmt
            .query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(case, v)| v.map(|v| (case, v.trim().to_string())).filter(|(_, v)| !v.is_empty()))
            .collect();
        let mut text = Vec::with_capacity(cases.len());
        let mut num = Vec::with_capacity(cases.len());
        let mut not_a_number = 0;
        for (case_id, _) in cases {
            let v = values.get(case_id).filter(|v| {
                let n = parse_number(v);
                !(missing_codes.iter().any(|m| m.trim() == v.as_str()) || n.is_some_and(|x| missing_numbers.contains(&x)))
            });
            match v {
                None => {
                    text.push(None);
                    num.push(None);
                }
                Some(v) if measure == Measure::Scale => match parse_number(v) {
                    Some(x) => {
                        text.push(Some(show_number(x)));
                        num.push(Some(x));
                    }
                    None => {
                        not_a_number += 1;
                        text.push(None);
                        num.push(None);
                    }
                },
                Some(v) => {
                    text.push(Some(v.clone()));
                    num.push(parse_number(v));
                }
            }
        }
        let var = StatVariable {
            key: format!("attr:{name}"),
            name: name.to_string(),
            kind: VariableKind::Attribute,
            code_id: None,
            measure,
            valid: 0,
            missing: 0,
            not_a_number,
            numeric: false,
            distinct: 0,
            categories: Vec::new(),
        };
        Ok(Column { var: finish(var, &text, &num, &labels), text, num })
    }

    /// For the codes asked for, each case's number of distinct excerpts of its documents that carry the code (and its
    /// sub-codes, when asked). Cases that hold no document have no entry.
    pub(crate) fn case_code_excerpts(&self, code_ids: &[i64], include_descendants: bool) -> Result<HashMap<i64, Vec<i64>>> {
        if code_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let spec = MatrixSpec { rows: RowSpec::Cases, code_ids: code_ids.to_vec(), include_descendants, scope: Scope { layer: Some("coder".to_string()), ..Default::default() } };
        let matrix = self.code_matrix(&spec)?;
        let members = self.cases_with_members()?;
        let mut out = HashMap::new();
        for row in matrix.rows {
            let Some(id) = row.key.strip_prefix("case:").and_then(|n| n.parse::<i64>().ok()) else { continue };
            if members.contains(&id) {
                out.insert(id, row.cells.iter().map(|c| c.excerpts).collect());
            }
        }
        Ok(out)
    }

    pub(crate) fn code_name(&self, id: i64) -> Result<String> {
        self.conn
            .query_row("SELECT name FROM code WHERE id = ?1", [id], |r| r.get::<_, String>(0))
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound { what: "code", id },
                other => other.into(),
            })
    }

    fn code_columns(&self, wanted: &[(i64, bool)], include_descendants: bool, cases: &[(i64, String)]) -> Result<Vec<Column>> {
        // `wanted` holds (code id, true for the presence variable and false for the count).
        let mut ids: Vec<i64> = wanted.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        let counts = self.case_code_excerpts(&ids, include_descendants)?;
        let mut out = Vec::new();
        for &(code_id, presence) in wanted {
            let at = ids.iter().position(|i| *i == code_id).expect("collected above");
            let name = self.code_name(code_id)?;
            let mut text = Vec::with_capacity(cases.len());
            let mut num = Vec::with_capacity(cases.len());
            for (case_id, _) in cases {
                match counts.get(case_id).map(|c| c[at]) {
                    None => {
                        text.push(None);
                        num.push(None);
                    }
                    Some(n) => {
                        let x = if presence { f64::from(u8::from(n > 0)) } else { n as f64 };
                        text.push(Some(show_number(x)));
                        num.push(Some(x));
                    }
                }
            }
            let var = StatVariable {
                key: format!("code:{code_id}:{}", if presence { "present" } else { "count" }),
                name,
                kind: if presence { VariableKind::CodePresent } else { VariableKind::CodeCount },
                code_id: Some(code_id),
                measure: if presence { Measure::Nominal } else { Measure::Scale },
                valid: 0,
                missing: 0,
                not_a_number: 0,
                numeric: false,
                distinct: 0,
                categories: Vec::new(),
            };
            out.push(Column { var: finish(var, &text, &num, &BTreeMap::new()), text, num });
        }
        Ok(out)
    }

    fn frame(&self, keys: &[String], options: &StatOptions) -> Result<Frame> {
        // Only the cases a rule picks, among the cases asked for.
        let picked: Option<Vec<i64>> = match options.filter.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
            Some(rule) => {
                let keep = self.filter_cases(rule)?;
                if keep.is_empty() {
                    return Err(Error::Invalid(format!("the rule {rule} picks no cases, so there is nothing to analyze")));
                }
                Some(keep.into_iter().filter(|c| options.case_ids.as_ref().is_none_or(|ids| ids.contains(c))).collect())
            }
            None => options.case_ids.clone(),
        };
        let cases = self.stat_case_list(picked.as_deref())?;
        let (cases, weighted) = match options.weight.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
            Some(name) if !cases.is_empty() => {
                let (counted, info) = self.weigh_cases(name, cases)?;
                (counted, Some(info))
            }
            _ => (cases, None),
        };
        if cases.is_empty() {
            return Err(Error::Invalid("This project has no cases to analyze. Statistics works on cases (a person, a site, a family) and the variables they carry; add cases when importing a spreadsheet, or on the Data page.".to_string()));
        }
        let mut columns = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(name) = key.strip_prefix("attr:") {
                columns.push(self.attribute_column(name, &cases)?);
            } else if let Some(rest) = key.strip_prefix("code:") {
                let (id, kind) = rest.split_once(':').ok_or_else(|| Error::Invalid(format!("'{key}' is not a variable")))?;
                let id: i64 = id.parse().map_err(|_| Error::Invalid(format!("'{key}' is not a variable")))?;
                let presence = match kind {
                    "present" => true,
                    "count" => false,
                    _ => return Err(Error::Invalid(format!("'{key}' is not a variable"))),
                };
                columns.extend(self.code_columns(&[(id, presence)], options.include_descendants, &cases)?);
            } else {
                return Err(Error::Invalid(format!("'{key}' is not a variable")));
            }
        }
        Ok(Frame { cases, columns, weight: weighted })
    }

    /// Counts each case as many times as a variable says. A weight is how many times a case is counted: a whole number, 0 or more (a case with
    /// 0 or with none is left out); the analysis then gives what it would give if each case were written out that many times.
    fn weigh_cases(&self, name: &str, cases: Vec<(i64, String)>) -> Result<(Vec<(i64, String)>, WeightInfo)> {
        let column = self.attribute_column(name, &cases)?;
        let numbers = column.var.measure == Measure::Scale || (column.var.usable() && column.var.numeric);
        if !numbers {
            return Err(Error::Invalid(format!("'{name}' is not a variable of numbers, so it cannot say how many times each case is counted")));
        }
        let before = cases.len();
        let mut counted: Vec<(i64, String)> = Vec::new();
        for ((case, w), text) in cases.iter().zip(&column.num).zip(&column.text) {
            match w {
                None if text.is_some() => {
                    return Err(Error::Invalid(format!("{} has the weight '{}' in '{name}', which is not a number. A weight is how many times a case is counted: a whole number, 0 or more.", case.1, text.clone().unwrap_or_default())))
                }
                None => {}
                Some(w) if *w < 0.0 || w.fract() != 0.0 || !w.is_finite() => {
                    return Err(Error::Invalid(format!("{} has the weight {} in '{name}'. A weight is how many times a case is counted: a whole number, 0 or more. Round it, or make a variable of whole numbers.", case.1, show_number(*w))))
                }
                Some(w) => {
                    if counted.len() as f64 + w > MAX_WEIGHTED as f64 {
                        return Err(Error::Invalid(format!("the weights in '{name}' add up to more than {MAX_WEIGHTED} cases, which is more than an analysis here counts")));
                    }
                    for _ in 0..(*w as usize) {
                        counted.push(case.clone());
                    }
                }
            }
        }
        if counted.is_empty() {
            return Err(Error::Invalid(format!("no case has a weight of 1 or more in '{name}', so there is nothing to analyze")));
        }
        let left_out = cases.iter().zip(&column.num).filter(|(_, w)| w.is_none_or(|w| w == 0.0)).count();
        let info = WeightInfo { variable: name.to_string(), cases: before, counted: counted.len(), left_out };
        Ok((counted, info))
    }

    /// Every variable the Statistics page can offer: the variables of cases, and for each active code whether the case has it and
    /// how many excerpts carry it.
    pub fn stat_variables(&self) -> Result<StatVariables> {
        let cases = self.stat_case_list(None)?;
        let with_documents = self.cases_with_members()?;
        let cases_with_documents = cases.iter().filter(|(id, _)| with_documents.contains(id)).count();
        let mut variables = Vec::new();
        let mut stmt = self.conn.prepare("SELECT name FROM attribute WHERE applies_to = 'case' ORDER BY name")?;
        let names: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        for name in names {
            variables.push(self.attribute_column(&name, &cases)?.var);
        }
        let mut stmt = self.conn.prepare("SELECT id FROM code WHERE status = 'active' ORDER BY id")?;
        let code_ids: Vec<i64> = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        if !cases.is_empty() && !code_ids.is_empty() {
            let wanted: Vec<(i64, bool)> = code_ids.iter().flat_map(|&id| [(id, true), (id, false)]).collect();
            for column in self.code_columns(&wanted, true, &cases)? {
                variables.push(column.var);
            }
        }
        Ok(StatVariables { cases: cases.len(), cases_with_documents, variables })
    }
}

/// One case of the table a script reads.
#[derive(Debug, Clone, Serialize)]
pub struct TableCase {
    pub id: i64,
    pub name: String,
}

/// One variable of the table a script reads: its values for every case, as the Statistics page reads them (a missing value, and a value
/// the project marks as missing, are `None`).
#[derive(Debug, Clone, Serialize)]
pub struct TableColumn {
    pub key: String,
    pub name: String,
    pub kind: VariableKind,
    pub measure: Measure,
    /// True when the values are numbers (a scale variable, or one whose every value reads as a number); the values are then written
    /// the way the page shows a number, so a program that reads them gets the numbers the page used.
    pub numeric: bool,
    pub values: Vec<Option<String>>,
}

/// The cases and the variables a recorded script reads, all the cases of the project and no case left out by a rule: the script applies
/// the rule itself.
#[derive(Debug, Clone, Serialize)]
pub struct StatTable {
    pub cases: Vec<TableCase>,
    pub columns: Vec<TableColumn>,
}

impl Project {
    /// The values of the variables asked for (by key, as for a run), for every case.
    pub fn stat_table(&self, keys: &[String], include_descendants: bool) -> Result<StatTable> {
        let options = StatOptions { include_descendants, ..StatOptions::default() };
        let f = self.frame(keys, &options)?;
        let columns = f
            .columns
            .iter()
            .map(|c| {
                let numeric = c.var.measure == Measure::Scale || (c.var.usable() && c.var.numeric);
                let values = if numeric { c.num.iter().map(|n| n.map(show_number)).collect() } else { c.text.clone() };
                TableColumn { key: c.var.key.clone(), name: c.var.name.clone(), kind: c.var.kind, measure: c.var.measure, numeric, values }
            })
            .collect();
        Ok(StatTable { cases: f.cases.iter().map(|(id, name)| TableCase { id: *id, name: name.clone() }).collect(), columns })
    }
}

// ---------------------------------------------------------------- the request

/// What can be run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Procedure {
    /// One variable: its numbers, or the counts of its values.
    Describe,
    /// The mean of one scale variable against a value.
    OneSampleT,
    /// The mean of a scale variable in two groups.
    IndependentT,
    /// Two scale variables measured on the same cases.
    PairedT,
    /// The mean of a scale variable in two or more groups.
    OneWayAnova,
    /// Two categorical variables.
    ChiSquare,
    /// Two categorical variables with two values each.
    Fisher,
    Pearson,
    Spearman,
    Kendall,
    MannWhitney,
    /// Two numeric variables on the same cases, or one against a value.
    Wilcoxon,
    KruskalWallis,
    /// A number explained by other variables (numbers, or categories turned into 0/1 columns), with an intercept.
    LinearRegression,
    /// A yes-or-no outcome (a variable with two values) explained by other variables, with an intercept: the odds of one value against the other.
    LogisticRegression,
    /// A number explained by another number through one or more curves (linear, logarithmic, inverse, quadratic, cubic, power, exponential, S).
    CurveFit,
    /// Whether one number could come from a normal distribution (Shapiro-Wilk).
    ShapiroWilk,
    /// One number against a normal distribution of a given mean and spread (Kolmogorov-Smirnov).
    KsNormal,
    /// Whether the order of the cases looks random, for a number cut in two or a variable with two values.
    Runs,
    /// The share of cases in one category against a share you name (exact).
    Binomial,
    /// Whether the counts in the categories of one variable follow shares you name (one-sample chi-square).
    GoodnessOfFit,
    /// Whether a number is above or below a value, or a number above or below another, as often as not (signs only).
    Sign,
    /// Three or more numbers on the same cases, by ranks.
    Friedman,
    /// The number in two groups: whether the whole distribution differs (Kolmogorov-Smirnov, two samples).
    KsTwo,
    /// The number in two or more groups: whether the same share of each is above the pooled median.
    MedianTest,
    /// Whether one group is more spread out than a control group (Moses).
    Moses,
    /// Two yes-or-no answers by the same cases: whether the answers changed one way more than the other.
    Mcnemar,
    /// Two ordered categories: gamma, tau-b, tau-c, and Somers' d.
    OrdinalAssociation,
    /// Two numbers with the effect of others taken out of both.
    PartialCorrelation,
    /// A number by two groupings and what they do together (two-way analysis of variance, type III sums of squares).
    TwoWayAnova,
    /// How well several items measure one thing: Cronbach's alpha, with each item's part in it.
    CronbachAlpha,
}

/// Where a number is cut in two for the runs test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunsCut {
    #[default]
    Median,
    Mean,
    /// The value in `mu`.
    Value,
}

fn half() -> f64 {
    0.5
}

/// The tests made for comparing pairs of groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostHocName {
    Tukey,
    GamesHowell,
    Scheffe,
}

impl PostHocName {
    fn method(self) -> PostHoc {
        match self {
            PostHocName::Tukey => PostHoc::Tukey,
            PostHocName::GamesHowell => PostHoc::GamesHowell,
            PostHocName::Scheffe => PostHoc::Scheffe,
        }
    }
}

fn yes() -> bool {
    true
}
fn confidence() -> f64 {
    0.95
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatOptions {
    /// The confidence level of intervals.
    #[serde(default = "confidence")]
    pub level: f64,
    /// The value a one-sample test compares with.
    #[serde(default)]
    pub mu: f64,
    /// Student's pooled t test, which assumes equal variances. Welch's test, which does not, is the default.
    #[serde(default)]
    pub equal_variances: bool,
    /// How pairwise comparisons after ANOVA or Kruskal-Wallis are adjusted (Holm by default).
    #[serde(default)]
    pub adjustment: Option<Adjustment>,
    /// A test made for comparing pairs of groups after an analysis of variance (Tukey, Games-Howell, or Scheffe), used instead of
    /// the adjusted t tests when given. Kruskal-Wallis compares pairs by Dunn's test and ignores it.
    #[serde(default)]
    pub posthoc: Option<PostHocName>,
    /// The standard deviation of the normal distribution of the Kolmogorov-Smirnov test (with `mu` as its mean). When absent, the
    /// sample's own mean and standard deviation are used, which makes the p-value too large.
    #[serde(default)]
    pub sd: Option<f64>,
    /// The category counted as a success in the binomial test (the first category by default).
    #[serde(default)]
    pub success: Option<String>,
    /// The share the binomial test compares with.
    #[serde(default = "half")]
    pub proportion: f64,
    /// The shares of the categories the one-sample chi-square compares with, by value (equal shares when absent). They are scaled to add up to 1.
    #[serde(default)]
    pub expected: Option<BTreeMap<String, f64>>,
    /// The control group of the Moses test, by value (the first group by default).
    #[serde(default)]
    pub control: Option<String>,
    /// How many control cases the Moses test trims from each end (5 percent, at least 1, by default).
    #[serde(default)]
    pub trim: Option<usize>,
    /// Where the runs test cuts a number in two (the median by default).
    #[serde(default)]
    pub cut: Option<RunsCut>,
    /// The values of the grouping variable to compare (all of them by default).
    #[serde(default)]
    pub groups: Option<Vec<String>>,
    /// Only these cases.
    #[serde(default)]
    pub case_ids: Option<Vec<i64>>,
    /// A code variable also counts the code's sub-codes (the default).
    #[serde(default = "yes")]
    pub include_descendants: bool,
    /// For a regression, the step each predictor is entered in (1, 2, ...), one for each predictor variable in the order given.
    /// Models are fitted for each step with every predictor of that step and the steps before it. All in one step by default.
    #[serde(default)]
    pub steps: Option<Vec<usize>>,
    /// For a regression, the category of a categorical predictor that the others are compared with, by variable key. The first
    /// category in order is the reference by default.
    #[serde(default)]
    pub references: BTreeMap<String, String>,
    /// For a logistic regression, the value of the outcome that is modelled (the one the odds are of). `1` when the outcome has it,
    /// else the last of the two values in order.
    #[serde(default)]
    pub event: Option<String>,
    /// For a curve fit, the curves to fit (all of them by default).
    #[serde(default)]
    pub curves: Option<Vec<CurveKind>>,
    /// Add bootstrap intervals (percentile, and BCa where it applies) for the statistic the procedure reports.
    #[serde(default)]
    pub bootstrap: Option<BootstrapOptions>,
    /// Only the cases a rule picks: a formula that gives true or false for each case, such as `age >= 18 & site == "North"`.
    #[serde(default)]
    pub filter: Option<String>,
    /// Count each case as many times as this variable says (a whole number, 0 or more), as if the case were written out that many times.
    /// Not combined with a bootstrap, which resamples cases.
    #[serde(default)]
    pub weight: Option<String>,
}

fn default_resamples() -> usize {
    2000
}
fn default_seed() -> u64 {
    1
}

/// How many resamples to draw and from which seed; the same seed gives the same intervals on every computer.
#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapOptions {
    #[serde(default = "default_resamples")]
    pub resamples: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// Bootstrap the whole analysis: run the procedure again on each resample of the cases, and give an interval for each number it
    /// reports (percentile intervals only), instead of for the one statistic the procedure is known for.
    #[serde(default)]
    pub analysis: bool,
    /// Resample whole groups of cases (the cases that share a value of this variable, by key) instead of single cases, so that cases
    /// that belong together stay together. Implies the whole analysis.
    #[serde(default)]
    pub cluster: Option<String>,
}

impl Default for BootstrapOptions {
    fn default() -> Self {
        Self { resamples: default_resamples(), seed: default_seed(), analysis: false, cluster: None }
    }
}

impl Default for StatOptions {
    fn default() -> Self {
        Self { level: 0.95, mu: 0.0, equal_variances: false, adjustment: None, posthoc: None, sd: None, success: None, proportion: 0.5, expected: None, control: None, trim: None, cut: None, groups: None, case_ids: None, include_descendants: true, steps: None, references: BTreeMap::new(), event: None, curves: None, bootstrap: None, filter: None, weight: None }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatRequest {
    pub procedure: Procedure,
    /// The variables, by key, in the order the procedure reads them (outcome then group; first then second; row then column).
    pub variables: Vec<String>,
    #[serde(default)]
    pub options: StatOptions,
}

// ---------------------------------------------------------------- the result

#[derive(Debug, Clone, Serialize)]
pub struct ResultVariable {
    pub key: String,
    pub name: String,
    pub kind: VariableKind,
    pub code_id: Option<i64>,
    pub measure: Measure,
}

/// One group of cases in a comparison.
#[derive(Debug, Clone, Serialize)]
pub struct ResultGroup {
    pub value: String,
    pub label: String,
    pub n: usize,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    /// The cases in the group, so a table can open them.
    pub case_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Nothing to worry about.
    Ok,
    /// Worth saying, and the result may still be sound.
    Caution,
    /// The test's conditions are not met; use another test or read the result as a guess.
    Problem,
    /// A fact about the design, not a check.
    Info,
}

/// A condition a test depends on, checked.
#[derive(Debug, Clone, Serialize)]
pub struct Assumption {
    /// `independence`, `sample_size`, `normality`, `outliers`, `equal_variances`, `expected_counts`, or `ties`.
    pub id: String,
    pub status: Status,
    /// The group it is about, when it is about one.
    pub group: Option<String>,
    pub numbers: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatResult {
    pub procedure: Procedure,
    pub variables: Vec<ResultVariable>,
    /// The cases looked at, the cases that had every value the procedure needed, and the cases set aside for a missing value.
    pub cases_total: usize,
    pub n_used: usize,
    pub n_excluded: usize,
    pub groups: Vec<ResultGroup>,
    pub assumptions: Vec<Assumption>,
    /// What is drawn beside the tables: the data of each chart, which the interface draws and words.
    pub charts: Vec<Chart>,
    /// The numbers: what the procedure in `prismqda-stats` returned, as it is.
    pub details: Value,
    /// When each case was counted by a weight: which variable, and how many cases that came to. `null` for an analysis that is not weighted.
    pub weight: Option<WeightInfo>,
}

fn assumption(id: &str, status: Status, group: Option<&str>, numbers: &[(&str, f64)]) -> Assumption {
    Assumption { id: id.to_string(), status, group: group.map(str::to_string), numbers: numbers.iter().map(|(k, v)| ((*k).to_string(), *v)).collect() }
}

/// The checks of how many cases there are: fewer than 3 cannot be tested, fewer than 15 is little to go on.
fn size_check(n_min: usize) -> Assumption {
    let status = if n_min < 3 {
        Status::Problem
    } else if n_min < 15 {
        Status::Caution
    } else {
        Status::Ok
    };
    assumption("sample_size", status, None, &[("n_min", n_min as f64)])
}

/// Whether a set of values looks too skewed or heavy-tailed for a method that assumes a normal distribution, and whether it has
/// outliers. The thresholds are the common working ones (skewness beyond 1 or excess kurtosis beyond 2 is worth a word, beyond 2
/// and 7 is a problem), with the Shapiro-Wilk test shown beside them.
fn shape_checks(values: &[f64], group: Option<&str>) -> Vec<Assumption> {
    let d = descriptives(values, 0.95);
    let mut out = Vec::new();
    // The Shapiro-Wilk test is reported next to the shape, and does not change the status: with few cases it has little power,
    // and with many it flags departures too small to matter, so the person is told both and shown the plot.
    let sw = shapiro_wilk(values);
    if let (Some(skew), Some(kurt)) = (d.skewness, d.kurtosis) {
        let status = if skew.abs() > 2.0 || kurt.abs() > 7.0 {
            Status::Problem
        } else if skew.abs() > 1.0 || kurt.abs() > 2.0 {
            Status::Caution
        } else {
            Status::Ok
        };
        let mut numbers = vec![("skewness", skew), ("kurtosis", kurt), ("n", values.len() as f64)];
        if let Some(s) = sw {
            numbers.push(("w", s.w));
            numbers.push(("shapiro_p", s.p));
        }
        out.push(assumption("normality", status, group, &numbers));
    }
    if d.outliers > 0 {
        out.push(assumption("outliers", Status::Caution, group, &[("count", d.outliers as f64), ("n", values.len() as f64)]));
    }
    out
}

fn mean_of(v: &[f64]) -> Option<f64> {
    prismqda_stats::describe::mean(v)
}

fn median_of(v: &[f64]) -> Option<f64> {
    percentile_sorted(&sorted_finite(v), 0.5)
}

/// A group of values with the cases they came from.
struct Group {
    value: String,
    label: String,
    values: Vec<f64>,
    case_ids: Vec<i64>,
}

impl Frame {
    fn variable(&self, i: usize) -> &StatVariable {
        &self.columns[i].var
    }

    fn name(&self, i: usize) -> String {
        self.variable(i).name.clone()
    }

    /// True when a column can be read as numbers for a procedure: a scale variable, or (for rank-based methods) an ordinal one
    /// whose values are all numbers.
    fn require_numeric(&self, i: usize, ordinal_ok: bool) -> Result<()> {
        let v = self.variable(i);
        let fine = match v.measure {
            Measure::Scale => true,
            Measure::Ordinal => ordinal_ok && v.numeric,
            Measure::Nominal => ordinal_ok && v.numeric && v.kind == VariableKind::CodePresent,
            _ => false,
        };
        if fine {
            return Ok(());
        }
        Err(Error::Invalid(format!(
            "'{}' is not a numeric variable, so this procedure cannot use it. {}",
            v.name,
            match v.measure {
                Measure::Nominal => "It is nominal: its values are names of groups. Use it to group cases, or choose a procedure for categories.",
                Measure::Ordinal => "It is ordinal and some of its values are not numbers. A rank-based procedure needs numbers it can put in order.",
                Measure::Date | Measure::Text => "Dates and free text are not analyzed here.",
                Measure::Scale => "",
            }
        )))
    }

    fn require_categorical(&self, i: usize) -> Result<()> {
        let v = self.variable(i);
        if !v.usable() {
            return Err(Error::Invalid(format!("'{}' holds dates or free text, which this procedure cannot group by.", v.name)));
        }
        if v.distinct > MAX_CATEGORIES || v.categories.is_empty() {
            return Err(Error::Invalid(format!("'{}' has {} different values, too many to use as groups. Choose a variable with at most {MAX_CATEGORIES}.", v.name, v.distinct)));
        }
        Ok(())
    }

    /// The rows where every listed column has a value.
    fn complete_rows(&self, columns: &[usize]) -> Vec<usize> {
        (0..self.cases.len()).filter(|&r| columns.iter().all(|&c| self.columns[c].text[r].is_some())).collect()
    }

    fn label_of(&self, i: usize, value: &str) -> String {
        self.variable(i).categories.iter().find(|c| c.value == value).map(|c| c.label.clone()).unwrap_or_else(|| value.to_string())
    }

    /// The outcome's values in each group of the grouping variable, among the cases that have both, in the order of the values.
    fn groups(&self, outcome: usize, by: usize, wanted: Option<&[String]>) -> Result<Vec<Group>> {
        self.require_categorical(by)?;
        let rows = self.complete_rows(&[outcome, by]);
        let mut map: BTreeMap<Vec<(bool, String, u64)>, Group> = BTreeMap::new();
        for r in rows {
            let value = self.columns[by].text[r].clone().expect("complete row");
            if wanted.is_some_and(|w| !w.contains(&value)) {
                continue;
            }
            let x = self.columns[outcome].num[r].expect("complete numeric row");
            let g = map.entry(natural(&value)).or_insert_with(|| Group { label: self.label_of(by, &value), value: value.clone(), values: Vec::new(), case_ids: Vec::new() });
            g.values.push(x);
            g.case_ids.push(self.cases[r].0);
        }
        Ok(map.into_values().collect())
    }
}

/// The place of a value among levels listed once for each natural key. Values that differ only by case or by leading zeros ("Female" and
/// "female", "07" and "7") are one level, as they are when the levels are listed, so a value is found by its key and not by its exact spelling.
fn level_of(levels: &[String], value: Option<&str>) -> Option<usize> {
    let key = natural(value?);
    levels.iter().position(|v| natural(v) == key)
}

fn result_groups(groups: &[Group]) -> Vec<ResultGroup> {
    groups.iter().map(|g| ResultGroup { value: g.value.clone(), label: g.label.clone(), n: g.values.len(), mean: mean_of(&g.values), median: median_of(&g.values), case_ids: g.case_ids.clone() }).collect()
}

fn group_checks(groups: &[Group], parametric: bool) -> Vec<Assumption> {
    let mut out = vec![assumption("independence", Status::Info, None, &[]), size_check(groups.iter().map(|g| g.values.len()).min().unwrap_or(0))];
    if parametric {
        for g in groups {
            out.extend(shape_checks(&g.values, Some(&g.label)));
        }
    }
    out
}

fn levene_check(groups: &[Group]) -> Option<Assumption> {
    let sets: Vec<Vec<f64>> = groups.iter().map(|g| g.values.clone()).collect();
    let a = one_way(&sets)?;
    let l = a.levene?;
    let sds: Vec<f64> = a.groups.iter().filter_map(|g| g.sd).collect();
    let ratio = sds.iter().copied().fold(0.0, f64::max) / sds.iter().copied().fold(f64::INFINITY, f64::min);
    Some(assumption("equal_variances", if l.p < 0.05 { Status::Caution } else { Status::Ok }, None, &[("levene_f", l.f), ("levene_p", l.p), ("sd_ratio", ratio)]))
}

fn need_two_or_more(groups: &[Group], wanted: usize, name: &str) -> Result<()> {
    if groups.len() < 2 {
        return Err(Error::Invalid(format!("'{name}' has fewer than two groups among the cases that have both values, so there is nothing to compare.")));
    }
    if wanted == 2 && groups.len() != 2 {
        let list: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        return Err(Error::Invalid(format!("'{name}' has {} groups ({}). This procedure compares exactly two: choose two of them, or use the analysis of variance (or Kruskal-Wallis) for more.", groups.len(), list.join(", "))));
    }
    Ok(())
}

impl Project {
    /// Runs one procedure on the variables of the cases.
    pub fn run_statistic(&self, req: &StatRequest) -> Result<StatResult> {
        if matches!(req.procedure, Procedure::LinearRegression | Procedure::LogisticRegression) && (req.variables.len() < 2 || req.variables.len() > MAX_PREDICTORS + 1) {
            return Err(Error::Invalid(format!("a regression needs an outcome and from 1 to {MAX_PREDICTORS} predictors, and {} variable{} {} chosen", req.variables.len(), if req.variables.len() == 1 { "" } else { "s" }, if req.variables.len() == 1 { "was" } else { "were" })));
        }
        if req.procedure == Procedure::Friedman && !(2..=MAX_PREDICTORS).contains(&req.variables.len()) {
            return Err(Error::Invalid(format!("Friedman's test compares from 2 to {MAX_PREDICTORS} numbers measured on the same cases, and {} {} chosen", req.variables.len(), if req.variables.len() == 1 { "was" } else { "were" })));
        }
        if req.procedure == Procedure::PartialCorrelation && !(3..=MAX_PREDICTORS).contains(&req.variables.len()) {
            return Err(Error::Invalid(format!("a partial correlation needs two numbers and from 1 to {} to hold constant, and {} variable{} {} chosen", MAX_PREDICTORS - 2, req.variables.len(), if req.variables.len() == 1 { "" } else { "s" }, if req.variables.len() == 1 { "was" } else { "were" })));
        }
        if req.procedure == Procedure::CronbachAlpha && !(2..=MAX_PREDICTORS).contains(&req.variables.len()) {
            return Err(Error::Invalid(format!("Cronbach's alpha needs from 2 to {MAX_PREDICTORS} items (numbers measured on the same cases), and {} {} chosen", req.variables.len(), if req.variables.len() == 1 { "was" } else { "were" })));
        }
        let expected = match req.procedure {
            Procedure::TwoWayAnova => 3,
            Procedure::Describe | Procedure::OneSampleT | Procedure::ShapiroWilk | Procedure::KsNormal | Procedure::Runs | Procedure::Binomial | Procedure::GoodnessOfFit => 1,
            Procedure::Wilcoxon | Procedure::Sign => req.variables.len().clamp(1, 2),
            Procedure::LinearRegression | Procedure::LogisticRegression | Procedure::Friedman | Procedure::PartialCorrelation | Procedure::CronbachAlpha => req.variables.len(),
            _ => 2,
        };
        if req.variables.len() != expected {
            return Err(Error::Invalid(format!("this procedure needs {expected} variable{}, and {} {} chosen", if expected == 1 { "" } else { "s" }, req.variables.len(), if req.variables.len() == 1 { "was" } else { "were" })));
        }
        let o = &req.options;
        if !(o.level > 0.0 && o.level < 1.0) {
            return Err(Error::Invalid("the confidence level must be between 0 and 1".to_string()));
        }
        if o.bootstrap.is_some() && o.weight.as_deref().is_some_and(|w| !w.trim().is_empty()) {
            return Err(Error::Invalid("A bootstrap resamples cases, and a weighted analysis counts a case several times, so the two cannot be combined. Turn the weights off, or the bootstrap.".to_string()));
        }
        let whole = o.bootstrap.as_ref().is_some_and(|b| b.analysis || b.cluster.is_some());
        let boot = match &o.bootstrap {
            None => None,
            Some(b) if (100..=100_000).contains(&b.resamples) => (!whole).then_some(BootstrapSpec { resamples: b.resamples, seed: b.seed, level: o.level }),
            Some(_) => return Err(Error::Invalid("the bootstrap needs between 100 and 100,000 resamples".to_string())),
        };
        let f = self.frame(&req.variables, o)?;
        let mut result = run_on_frame(&f, req, boot)?;
        if let Some(b) = o.bootstrap.as_ref().filter(|_| whole) {
            let clusters = match &b.cluster {
                Some(key) => Some(self.cluster_groups(key, o)?),
                None => None,
            };
            let summary = analysis_bootstrap(&f, req, &result, b, clusters)?;
            result.details["bootstrap_analysis"] = summary;
        }
        Ok(result)
    }

    /// The rows of the cases, grouped by their value of a variable: a case with no value is a group of its own.
    fn cluster_groups(&self, key: &str, options: &StatOptions) -> Result<(String, Vec<Vec<usize>>)> {
        let cf = self.frame(&[key.to_string()], options)?;
        let var = cf.variable(0).clone();
        if !var.usable() {
            return Err(Error::Invalid(format!("'{}' holds dates or free text, so cases cannot be grouped by it.", var.name)));
        }
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (row, value) in cf.columns[0].text.iter().enumerate() {
            groups.entry(value.clone().unwrap_or_else(|| format!("\u{0}case {row}"))).or_default().push(row);
        }
        if groups.len() < MIN_CLUSTERS {
            return Err(Error::Invalid(format!("Resampling whole groups of cases needs at least {MIN_CLUSTERS} groups, and '{}' gives {} among the cases used.", var.name, groups.len())));
        }
        Ok((var.name, groups.into_values().collect()))
    }
}

/// The part of a run that works on a frame of cases. The whole-analysis bootstrap calls it again for every resample.
fn run_on_frame(f: &Frame, req: &StatRequest, boot: Option<BootstrapSpec>) -> Result<StatResult> {
    let o = &req.options;
    {
        let adjustment = o.adjustment.unwrap_or(Adjustment::Holm);
        let mut boot_data: Option<BootData> = None;
        let mut groups: Vec<ResultGroup> = Vec::new();
        let mut assumptions: Vec<Assumption> = Vec::new();
        let mut charts: Vec<Chart> = Vec::new();
        let used: usize;
        let details: Value;

        match req.procedure {
            Procedure::Describe => {
                let v = f.variable(0).clone();
                if !v.usable() {
                    return Err(Error::Invalid(format!("'{}' holds dates or free text, which is not described here.", v.name)));
                }
                used = v.valid;
                if v.measure == Measure::Scale {
                    let values: Vec<f64> = f.columns[0].num.iter().flatten().copied().collect();
                    details = json!({ "kind": "descriptives", "descriptives": descriptives(&values, o.level) });
                    assumptions.extend(shape_checks(&values, None));
                    charts.extend(histogram_chart(Subject::Variable { index: 0 }, &values, None));
                    charts.extend(qq_chart(Subject::Variable { index: 0 }, &values));
                    boot_data = Some(BootData::Summary(values));
                } else {
                    let total = f.cases.len() as f64;
                    let valid = v.valid as f64;
                    let cats: Vec<Value> = v.categories.iter().map(|c| json!({ "value": c.value, "label": c.label, "count": c.count, "percent": c.count as f64 / total, "valid_percent": c.count as f64 / valid })).collect();
                    details = json!({ "kind": "frequencies", "categories": cats, "valid": v.valid, "missing": v.missing });
                    charts.push(Chart::Bars { subject: Subject::Variable { index: 0 }, valid: v.valid, categories: v.categories.iter().map(|c| BarCategory { value: c.value.clone(), label: c.label.clone(), count: c.count }).collect() });
                }
            }
            Procedure::OneSampleT => {
                f.require_numeric(0, false)?;
                let values: Vec<f64> = f.columns[0].num.iter().flatten().copied().collect();
                used = values.len();
                let t = one_sample(&values, o.mu, o.level).ok_or_else(|| Error::Invalid(format!("'{}' needs at least two cases whose values are not all the same.", f.name(0))))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(values.len()));
                assumptions.extend(shape_checks(&values, None));
                details = json!({ "t_test": t, "mu": o.mu });
                charts.extend(histogram_chart(Subject::Variable { index: 0 }, &values, Some(o.mu)));
                boot_data = Some(BootData::Mean(values));
            }
            Procedure::PairedT | Procedure::Wilcoxon if req.variables.len() == 2 => {
                let rank_based = req.procedure == Procedure::Wilcoxon;
                f.require_numeric(0, rank_based)?;
                f.require_numeric(1, rank_based)?;
                let rows = f.complete_rows(&[0, 1]);
                used = rows.len();
                let x: Vec<f64> = rows.iter().map(|&r| f.columns[0].num[r].expect("complete")).collect();
                let y: Vec<f64> = rows.iter().map(|&r| f.columns[1].num[r].expect("complete")).collect();
                let diffs: Vec<f64> = x.iter().zip(&y).map(|(a, b)| a - b).collect();
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                if rank_based {
                    let w = wilcoxon(&x, Some(&y), 0.0).ok_or_else(|| Error::Invalid("every pair has the same two values, so there is no difference to rank.".to_string()))?;
                    details = json!({ "wilcoxon": w, "median_difference": median_of(&diffs) });
                    charts.extend(histogram_chart(Subject::Differences, &diffs, Some(0.0)));
                    boot_data = Some(BootData::Median(diffs.clone()));
                } else {
                    assumptions.extend(shape_checks(&diffs, Some("differences")));
                    let t = paired(&x, &y, o.level).ok_or_else(|| Error::Invalid("the differences between the two variables are all the same, so a t test cannot be done.".to_string()))?;
                    details = json!({ "t_test": t });
                    charts.extend(histogram_chart(Subject::Differences, &diffs, Some(0.0)));
                    boot_data = Some(BootData::Mean(diffs.clone()));
                }
            }
            Procedure::Wilcoxon => {
                f.require_numeric(0, true)?;
                let values: Vec<f64> = f.columns[0].num.iter().flatten().copied().collect();
                used = values.len();
                let w = wilcoxon(&values, None, o.mu).ok_or_else(|| Error::Invalid("every value equals the value compared with, so there is nothing to rank.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(values.len()));
                details = json!({ "wilcoxon": w, "mu": o.mu, "median": median_of(&values) });
                charts.extend(histogram_chart(Subject::Variable { index: 0 }, &values, Some(o.mu)));
                boot_data = Some(BootData::Median(values));
            }
            Procedure::PairedT => unreachable!("handled with two variables above"),
            Procedure::IndependentT | Procedure::MannWhitney => {
                let rank_based = req.procedure == Procedure::MannWhitney;
                f.require_numeric(0, rank_based)?;
                let g = f.groups(0, 1, o.groups.as_deref())?;
                need_two_or_more(&g, 2, &f.name(1))?;
                used = g.iter().map(|x| x.values.len()).sum();
                groups = result_groups(&g);
                assumptions.extend(group_checks(&g, !rank_based));
                if rank_based {
                    let all: Vec<f64> = g.iter().flat_map(|x| x.values.iter().copied()).collect();
                    let ties = prismqda_stats::rank::tie_sizes(&all);
                    if !ties.is_empty() {
                        assumptions.push(assumption("ties", Status::Info, None, &[("groups_of_ties", ties.len() as f64), ("tied_values", ties.iter().sum::<usize>() as f64)]));
                    }
                    let m = mann_whitney(&g[0].values, &g[1].values).ok_or_else(|| Error::Invalid("every value is the same, so there is nothing to rank.".to_string()))?;
                    details = json!({ "mann_whitney": m });
                    charts.extend(box_chart(Subject::Variable { index: 0 }, 1, &group_triples(&g)));
                    boot_data = Some(BootData::Difference { a: g[0].values.clone(), b: g[1].values.clone(), medians: true });
                } else {
                    assumptions.extend(levene_check(&g));
                    let t = independent(&g[0].values, &g[1].values, !o.equal_variances, o.level)
                        .ok_or_else(|| Error::Invalid("each group needs at least two cases, and the values cannot all be the same.".to_string()))?;
                    details = json!({ "t_test": t });
                    charts.extend(box_chart(Subject::Variable { index: 0 }, 1, &group_triples(&g)));
                    charts.extend(means_chart(Subject::Variable { index: 0 }, 1, o.level, &group_triples(&g)));
                    boot_data = Some(BootData::Difference { a: g[0].values.clone(), b: g[1].values.clone(), medians: false });
                }
            }
            Procedure::OneWayAnova | Procedure::KruskalWallis => {
                let rank_based = req.procedure == Procedure::KruskalWallis;
                f.require_numeric(0, rank_based)?;
                let g = f.groups(0, 1, o.groups.as_deref())?;
                need_two_or_more(&g, 0, &f.name(1))?;
                used = g.iter().map(|x| x.values.len()).sum();
                groups = result_groups(&g);
                assumptions.extend(group_checks(&g, !rank_based));
                let sets: Vec<Vec<f64>> = g.iter().map(|x| x.values.clone()).collect();
                charts.extend(box_chart(Subject::Variable { index: 0 }, 1, &group_triples(&g)));
                if !rank_based {
                    charts.extend(means_chart(Subject::Variable { index: 0 }, 1, o.level, &group_triples(&g)));
                }
                if rank_based {
                    let k = kruskal_wallis(&sets).ok_or_else(|| Error::Invalid("every value is the same, so there is nothing to rank.".to_string()))?;
                    let comparisons = if sets.len() >= 3 { dunn(&k, adjustment) } else { Vec::new() };
                    details = json!({ "kruskal_wallis": k, "dunn": comparisons, "adjustment": adjustment });
                } else {
                    assumptions.extend(levene_check(&g));
                    let a = one_way(&sets).ok_or_else(|| Error::Invalid("the analysis of variance needs more cases than groups, and values that are not all the same within the groups.".to_string()))?;
                    let comparisons = if sets.len() < 3 {
                        Vec::new()
                    } else if let Some(name) = o.posthoc {
                        compare_pairs(&a, name.method(), o.level)
                    } else {
                        pairwise(&a, adjustment, o.level)
                    };
                    details = json!({ "anova": a, "pairwise": comparisons, "adjustment": adjustment, "posthoc": o.posthoc, "level": o.level });
                }
            }
            Procedure::ChiSquare | Procedure::Fisher => {
                f.require_categorical(0)?;
                f.require_categorical(1)?;
                let rows = f.complete_rows(&[0, 1]);
                used = rows.len();
                let mut row_values: BTreeMap<Vec<(bool, String, u64)>, String> = BTreeMap::new();
                let mut col_values: BTreeMap<Vec<(bool, String, u64)>, String> = BTreeMap::new();
                for &r in &rows {
                    let (a, b) = (f.columns[0].text[r].clone().expect("complete"), f.columns[1].text[r].clone().expect("complete"));
                    row_values.insert(natural(&a), a);
                    col_values.insert(natural(&b), b);
                }
                let row_values: Vec<String> = row_values.into_values().collect();
                let col_values: Vec<String> = col_values.into_values().collect();
                if row_values.len() < 2 || col_values.len() < 2 {
                    return Err(Error::Invalid("each variable needs at least two different values among the cases that have both, so there is no table to test.".to_string()));
                }
                let mut table = vec![vec![0.0; col_values.len()]; row_values.len()];
                for &r in &rows {
                    let a = level_of(&row_values, f.columns[0].text[r].as_deref()).expect("listed");
                    let b = level_of(&col_values, f.columns[1].text[r].as_deref()).expect("listed");
                    table[a][b] += 1.0;
                }
                let row_labels: Vec<String> = row_values.iter().map(|v| f.label_of(0, v)).collect();
                let col_labels: Vec<String> = col_values.iter().map(|v| f.label_of(1, v)).collect();
                let chi = chi_square(&table).expect("a table with every row and column occupied");
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                let status = if chi.smallest_expected < 1.0 || chi.share_expected_below_5 > 0.2 {
                    Status::Problem
                } else if chi.smallest_expected < 5.0 {
                    Status::Caution
                } else {
                    Status::Ok
                };
                assumptions.push(assumption("expected_counts", status, None, &[("smallest_expected", chi.smallest_expected), ("share_below_5", chi.share_expected_below_5)]));
                let base = json!({ "table": table, "row_values": row_values, "col_values": col_values, "row_labels": row_labels, "col_labels": col_labels });
                let mut d = base;
                if req.procedure == Procedure::Fisher {
                    if row_values.len() != 2 || col_values.len() != 2 {
                        return Err(Error::Invalid("Fisher's exact test is built for tables with two rows and two columns. This table is larger; use the chi-square test, and combine categories if the expected counts are small.".to_string()));
                    }
                    let c = |i: usize, j: usize| table[i][j] as u64;
                    let fi = fisher_exact(c(0, 0), c(0, 1), c(1, 0), c(1, 1), o.level).expect("a table with cases");
                    d["fisher"] = json!(fi);
                }
                d["chi_square"] = json!(chi);
                d["level"] = json!(o.level);
                if row_values.len() <= MAX_STACK_ROWS && col_values.len() <= MAX_STACK_COLS {
                    charts.push(Chart::Stacked { row: 0, col: 1, row_values: row_values.clone(), row_labels: row_labels.clone(), col_values: col_values.clone(), col_labels: col_labels.clone(), counts: table.clone() });
                }
                details = d;
            }
            Procedure::Pearson | Procedure::Spearman | Procedure::Kendall => {
                let rank_based = req.procedure != Procedure::Pearson;
                f.require_numeric(0, rank_based)?;
                f.require_numeric(1, rank_based)?;
                let rows = f.complete_rows(&[0, 1]);
                used = rows.len();
                let x: Vec<f64> = rows.iter().map(|&r| f.columns[0].num[r].expect("complete")).collect();
                let y: Vec<f64> = rows.iter().map(|&r| f.columns[1].num[r].expect("complete")).collect();
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                if !rank_based {
                    assumptions.extend(shape_checks(&x, Some(&f.name(0))));
                    assumptions.extend(shape_checks(&y, Some(&f.name(1))));
                }
                let c = match req.procedure {
                    Procedure::Pearson => pearson(&x, &y, o.level),
                    Procedure::Spearman => spearman(&x, &y, o.level),
                    _ => kendall(&x, &y, o.level),
                }
                .ok_or_else(|| Error::Invalid("a correlation needs at least three cases that have both values, and neither variable can be the same for every case.".to_string()))?;
                details = json!({ "correlation": c });
                let ids: Vec<i64> = rows.iter().map(|&r| f.cases[r].0).collect();
                charts.extend(scatter_chart(Subject::Variable { index: 0 }, Subject::Variable { index: 1 }, &x, &y, &ids, req.procedure == Procedure::Pearson, false));
                boot_data = Some(BootData::Correlation { x, y, kind: req.procedure });
            }
            Procedure::LinearRegression => {
                let (n_used, d, data) = regression_result(f, req, &mut assumptions, &mut charts)?;
                used = n_used;
                details = d;
                boot_data = Some(data);
            }
            Procedure::CurveFit => {
                f.require_numeric(0, false)?;
                f.require_numeric(1, false)?;
                let rows = f.complete_rows(&[0, 1]);
                used = rows.len();
                let y: Vec<f64> = rows.iter().map(|&r| f.columns[0].num[r].expect("complete")).collect();
                let x: Vec<f64> = rows.iter().map(|&r| f.columns[1].num[r].expect("complete")).collect();
                let ids: Vec<i64> = rows.iter().map(|&r| f.cases[r].0).collect();
                let mut kinds: Vec<CurveKind> = Vec::new();
                for k in o.curves.clone().filter(|c| !c.is_empty()).unwrap_or_else(|| CurveKind::ALL.to_vec()) {
                    if !kinds.contains(&k) {
                        kinds.push(k);
                    }
                }
                let mut fits: Vec<Value> = Vec::new();
                let mut drawn: Vec<(CurveKind, Vec<f64>)> = Vec::new();
                let mut first_error: Option<CurveError> = None;
                for kind in kinds {
                    match fit_curve(kind, &x, &y, o.level) {
                        Ok(c) => {
                            drawn.push((kind, c.regression.terms.iter().map(|t| t.estimate).collect()));
                            fits.push(curve_summary(&c));
                        }
                        Err(e) => {
                            first_error.get_or_insert(e);
                            fits.push(json!({ "kind": kind, "error": e }));
                        }
                    }
                }
                if drawn.is_empty() {
                    return Err(curve_error(first_error.expect("at least one curve was asked for"), &f.name(0), &f.name(1), used));
                }
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(used));
                charts.extend(curves_chart(Subject::Variable { index: 1 }, Subject::Variable { index: 0 }, &x, &y, &ids, &drawn));
                details = json!({ "fits": fits, "level": o.level });
            }
            Procedure::LogisticRegression => {
                let (n_used, d, data) = logistic_result(f, req, &mut assumptions, &mut charts)?;
                used = n_used;
                details = d;
                boot_data = Some(data);
            }
            Procedure::ShapiroWilk => {
                f.require_numeric(0, false)?;
                let values: Vec<f64> = f.columns[0].num.iter().flatten().copied().collect();
                used = values.len();
                let s = shapiro_wilk(&values).ok_or_else(|| Error::Invalid(format!("the Shapiro-Wilk test needs from 3 to 5,000 values that are not all the same, and '{}' has {}.", f.name(0), values.len())))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(values.len()));
                details = json!({ "shapiro_wilk": s });
                charts.extend(histogram_chart(Subject::Variable { index: 0 }, &values, None));
                charts.extend(qq_chart(Subject::Variable { index: 0 }, &values));
            }
            Procedure::KsNormal => {
                f.require_numeric(0, false)?;
                let values: Vec<f64> = f.columns[0].num.iter().flatten().copied().collect();
                used = values.len();
                let (mean, sd, estimated) = match o.sd {
                    Some(sd) => (o.mu, sd, false),
                    None => (
                        mean_of(&values).unwrap_or(f64::NAN),
                        prismqda_stats::describe::variance(&values).map(f64::sqrt).unwrap_or(f64::NAN),
                        true,
                    ),
                };
                if !(sd > 0.0 && sd.is_finite() && mean.is_finite()) {
                    return Err(Error::Invalid(format!("the normal distribution needs a standard deviation above zero, and '{}' has values that are not all the same only if it has at least two cases.", f.name(0))));
                }
                let k = ks_normal(&values, mean, sd).ok_or_else(|| Error::Invalid(format!("'{}' has no values to test.", f.name(0))))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(values.len()));
                details = json!({ "ks": k, "mean": mean, "sd": sd, "estimated": estimated });
                charts.extend(histogram_chart(Subject::Variable { index: 0 }, &values, None));
                charts.extend(qq_chart(Subject::Variable { index: 0 }, &values));
            }
            Procedure::Runs => {
                let v = f.variable(0).clone();
                let rows = f.complete_rows(&[0]);
                used = rows.len();
                let numeric = v.measure == Measure::Scale || (v.measure == Measure::Ordinal && v.numeric);
                let (sequence, cut_value, cut_kind): (Vec<bool>, Option<f64>, Option<RunsCut>) = if numeric {
                    let values: Vec<f64> = rows.iter().map(|&r| f.columns[0].num[r].expect("complete")).collect();
                    let kind = o.cut.unwrap_or_default();
                    let cut = match kind {
                        RunsCut::Median => median_of(&values),
                        RunsCut::Mean => mean_of(&values),
                        RunsCut::Value => Some(o.mu),
                    }
                    .ok_or_else(|| Error::Invalid(format!("'{}' has no values to cut.", f.name(0))))?;
                    (values.iter().map(|x| *x >= cut).collect(), Some(cut), Some(kind))
                } else if v.usable() && v.categories.len() == 2 {
                    let second = v.categories[1].value.clone();
                    (rows.iter().map(|&r| f.columns[0].text[r].as_deref() == Some(second.as_str())).collect(), None, None)
                } else {
                    return Err(Error::Invalid(format!("the runs test needs a number (which it cuts in two) or a variable with exactly two values, and '{}' is neither.", v.name)));
                };
                let r = runs_test(&sequence).ok_or_else(|| Error::Invalid("every case falls on the same side, so there is no change of value to count: try another cut.".to_string()))?;
                assumptions.push(assumption("order", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                details = json!({ "runs": r, "cut": cut_value, "cut_kind": cut_kind, "n": sequence.len(), "second_value": if numeric { Value::Null } else { json!(v.categories[1].label.clone()) } });
            }
            Procedure::Binomial => {
                f.require_categorical(0)?;
                let v = f.variable(0).clone();
                let rows = f.complete_rows(&[0]);
                used = rows.len();
                let success = o.success.clone().unwrap_or_else(|| v.categories[0].value.clone());
                if !v.categories.iter().any(|c| c.value == success) {
                    return Err(Error::Invalid(format!("'{success}' is not a value of '{}'.", v.name)));
                }
                let x = rows.iter().filter(|&&r| f.columns[0].text[r].as_deref() == Some(success.as_str())).count();
                let b = binomial_test(x as u64, used as u64, o.proportion, o.level).ok_or_else(|| Error::Invalid("the binomial test needs at least one case and a share between 0 and 1.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(used));
                let cats: Vec<Value> = v.categories.iter().map(|c| json!({ "value": c.value, "label": c.label, "count": c.count })).collect();
                details = json!({ "binomial": b, "success": success, "success_label": f.label_of(0, &success), "counts": cats });
                charts.push(Chart::Bars { subject: Subject::Variable { index: 0 }, valid: v.valid, categories: v.categories.iter().map(|c| BarCategory { value: c.value.clone(), label: c.label.clone(), count: c.count }).collect() });
            }
            Procedure::GoodnessOfFit => {
                f.require_categorical(0)?;
                let v = f.variable(0).clone();
                used = v.valid;
                let counts: Vec<f64> = v.categories.iter().map(|c| c.count as f64).collect();
                let shares: Option<Vec<f64>> = match &o.expected {
                    None => None,
                    Some(map) => {
                        let mut raw = Vec::new();
                        for c in &v.categories {
                            let s = map.get(&c.value).copied().ok_or_else(|| Error::Invalid(format!("no expected share was given for '{}'.", c.label)))?;
                            if !(s > 0.0 && s.is_finite()) {
                                return Err(Error::Invalid(format!("the expected share of '{}' must be above zero.", c.label)));
                            }
                            raw.push(s);
                        }
                        let total: f64 = raw.iter().sum();
                        Some(raw.iter().map(|s| s / total).collect())
                    }
                };
                let g = goodness_of_fit(&counts, shares.as_deref()).ok_or_else(|| Error::Invalid("the one-sample chi-square needs at least two categories with cases.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                let status = if g.smallest_expected < 1.0 { Status::Problem } else if g.smallest_expected < 5.0 { Status::Caution } else { Status::Ok };
                assumptions.push(assumption("expected_counts", status, None, &[("smallest_expected", g.smallest_expected), ("share_below_5", g.expected.iter().filter(|e| **e < 5.0).count() as f64 / g.expected.len() as f64)]));
                let cats: Vec<Value> = v
                    .categories
                    .iter()
                    .enumerate()
                    .map(|(i, c)| json!({ "value": c.value, "label": c.label, "count": c.count, "expected": g.expected[i], "share": g.expected[i] / g.n, "residual": g.residuals[i] }))
                    .collect();
                details = json!({ "goodness_of_fit": g, "counts": cats, "equal_shares": o.expected.is_none() });
                charts.push(Chart::Bars { subject: Subject::Variable { index: 0 }, valid: v.valid, categories: v.categories.iter().map(|c| BarCategory { value: c.value.clone(), label: c.label.clone(), count: c.count }).collect() });
            }
            Procedure::Sign if req.variables.len() == 2 => {
                f.require_numeric(0, true)?;
                f.require_numeric(1, true)?;
                let rows = f.complete_rows(&[0, 1]);
                used = rows.len();
                let diffs: Vec<f64> = rows.iter().map(|&r| f.columns[0].num[r].expect("complete") - f.columns[1].num[r].expect("complete")).collect();
                let s = sign_test(&diffs, o.level).ok_or_else(|| Error::Invalid("every pair has the same two values, so there is no sign to count.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                details = json!({ "sign": s, "paired": true });
                charts.extend(histogram_chart(Subject::Differences, &diffs, Some(0.0)));
            }
            Procedure::Sign => {
                f.require_numeric(0, true)?;
                let values: Vec<f64> = f.columns[0].num.iter().flatten().copied().collect();
                used = values.len();
                let diffs: Vec<f64> = values.iter().map(|x| x - o.mu).collect();
                let s = sign_test(&diffs, o.level).ok_or_else(|| Error::Invalid("every value equals the value compared with, so there is no sign to count.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(values.len()));
                details = json!({ "sign": s, "paired": false, "mu": o.mu, "median": median_of(&values) });
                charts.extend(histogram_chart(Subject::Variable { index: 0 }, &values, Some(o.mu)));
            }
            Procedure::Friedman => {
                let k = req.variables.len();
                for i in 0..k {
                    f.require_numeric(i, true)?;
                }
                let cols: Vec<usize> = (0..k).collect();
                let rows = f.complete_rows(&cols);
                used = rows.len();
                let columns: Vec<Vec<f64>> = (0..k).map(|c| rows.iter().map(|&r| f.columns[c].num[r].expect("complete")).collect()).collect();
                let fr = friedman(&columns).ok_or_else(|| Error::Invalid("Friedman's test needs at least two cases that have every value, and values that are not all the same within each case.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                let ids: Vec<i64> = rows.iter().map(|&r| f.cases[r].0).collect();
                groups = (0..k)
                    .map(|c| ResultGroup { value: f.variable(c).key.clone(), label: f.name(c), n: rows.len(), mean: mean_of(&columns[c]), median: median_of(&columns[c]), case_ids: ids.clone() })
                    .collect();
                details = json!({ "friedman": fr });
            }
            Procedure::KsTwo | Procedure::MedianTest | Procedure::Moses => {
                f.require_numeric(0, true)?;
                let g = f.groups(0, 1, o.groups.as_deref())?;
                need_two_or_more(&g, if req.procedure == Procedure::MedianTest { 0 } else { 2 }, &f.name(1))?;
                used = g.iter().map(|x| x.values.len()).sum();
                groups = result_groups(&g);
                assumptions.extend(group_checks(&g, false));
                charts.extend(box_chart(Subject::Variable { index: 0 }, 1, &group_triples(&g)));
                match req.procedure {
                    Procedure::KsTwo => {
                        let k = ks_two_sample(&g[0].values, &g[1].values).ok_or_else(|| Error::Invalid("each group needs at least one value.".to_string()))?;
                        details = json!({ "ks": k });
                    }
                    Procedure::MedianTest => {
                        let sets: Vec<Vec<f64>> = g.iter().map(|x| x.values.clone()).collect();
                        let m = median_test(&sets).ok_or_else(|| Error::Invalid("the median test needs some cases above the pooled median and some at or below it.".to_string()))?;
                        assumptions.push(assumption("expected_counts", if m.smallest_expected < 1.0 { Status::Problem } else if m.smallest_expected < 5.0 { Status::Caution } else { Status::Ok }, None, &[("smallest_expected", m.smallest_expected), ("share_below_5", 0.0)]));
                        details = json!({ "median_test": m });
                    }
                    _ => {
                        let control_at = match &o.control {
                            Some(c) => g.iter().position(|x| &x.value == c).ok_or_else(|| Error::Invalid(format!("'{c}' is not one of the groups.")))?,
                            None => 0,
                        };
                        let test_at = 1 - control_at;
                        let m = moses(&g[control_at].values, &g[test_at].values, o.trim).ok_or_else(|| Error::Invalid("the Moses test needs at least 3 cases in the control group (more if you trim) and 1 in the other.".to_string()))?;
                        if m.ties {
                            assumptions.push(assumption("ties", Status::Info, None, &[("groups_of_ties", 1.0), ("tied_values", 2.0)]));
                        }
                        details = json!({ "moses": m, "control": g[control_at].label, "test": g[test_at].label });
                    }
                }
            }
            Procedure::Mcnemar => {
                f.require_categorical(0)?;
                f.require_categorical(1)?;
                let rows = f.complete_rows(&[0, 1]);
                used = rows.len();
                let mut values: BTreeMap<Vec<(bool, String, u64)>, String> = BTreeMap::new();
                for &r in &rows {
                    for c in 0..2 {
                        let v = f.columns[c].text[r].clone().expect("complete");
                        values.insert(natural(&v), v);
                    }
                }
                let values: Vec<String> = values.into_values().collect();
                if values.len() != 2 {
                    return Err(Error::Invalid(format!("McNemar's test compares two answers with the same two values (such as yes and no) given by the same cases, and these have {} values between them.", values.len())));
                }
                let mut table = [[0.0f64; 2]; 2];
                for &r in &rows {
                    let a = level_of(&values, f.columns[0].text[r].as_deref()).expect("listed");
                    let b = level_of(&values, f.columns[1].text[r].as_deref()).expect("listed");
                    table[a][b] += 1.0;
                }
                let m = mcnemar(table[0][1], table[1][0]).ok_or_else(|| Error::Invalid("no case changed its answer between the two, so there is nothing to test.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                let labels: Vec<String> = values.iter().map(|v| f.label_of(0, v)).collect();
                details = json!({ "mcnemar": m, "table": table, "values": values, "labels": labels });
            }
            Procedure::OrdinalAssociation => {
                for i in 0..2 {
                    f.require_categorical(i)?;
                    let v = f.variable(i);
                    if !v.numeric || v.measure == Measure::Nominal {
                        return Err(Error::Invalid(format!("'{}' has to be ordered, with values that are numbers (1, 2, 3, ...) so that the order is known; it is {}.", v.name, if v.measure == Measure::Nominal { "nominal" } else { "ordinal, but not every value is a number" })));
                    }
                }
                let rows = f.complete_rows(&[0, 1]);
                used = rows.len();
                let (row_values, col_values, table) = contingency_table(f, &rows);
                if row_values.len() < 2 || col_values.len() < 2 {
                    return Err(Error::Invalid("each variable needs at least two different values among the cases that have both.".to_string()));
                }
                let oa = ordinal_association(&table).ok_or_else(|| Error::Invalid("no pair of cases differs on both variables, so there is no order to measure.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                let row_labels: Vec<String> = row_values.iter().map(|v| f.label_of(0, v)).collect();
                let col_labels: Vec<String> = col_values.iter().map(|v| f.label_of(1, v)).collect();
                details = json!({ "ordinal": oa, "table": table, "row_values": row_values, "col_values": col_values, "row_labels": row_labels, "col_labels": col_labels, "level": o.level });
            }
            Procedure::PartialCorrelation => {
                let k = req.variables.len();
                for i in 0..k {
                    f.require_numeric(i, false)?;
                }
                let cols: Vec<usize> = (0..k).collect();
                let rows = f.complete_rows(&cols);
                used = rows.len();
                let column = |c: usize| -> Vec<f64> { rows.iter().map(|&r| f.columns[c].num[r].expect("complete")).collect() };
                let (x, y) = (column(0), column(1));
                let controls: Vec<Vec<f64>> = (2..k).map(column).collect();
                let pc = partial_correlation(&x, &y, &controls, o.level).ok_or_else(|| Error::Invalid("a partial correlation needs more cases than the variables plus three, numbers that are not constant, and controls that do not predict each other perfectly.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                let names: Vec<String> = (2..k).map(|c| f.name(c)).collect();
                details = json!({ "partial": pc, "zero_order": pearson(&x, &y, o.level), "controls": names });
            }
            Procedure::TwoWayAnova => {
                f.require_numeric(0, false)?;
                f.require_categorical(1)?;
                f.require_categorical(2)?;
                let rows = f.complete_rows(&[0, 1, 2]);
                used = rows.len();
                // The levels of each factor among the cases used, in their natural order.
                let levels = |col: usize| -> Vec<String> {
                    let mut seen: BTreeMap<Vec<(bool, String, u64)>, String> = BTreeMap::new();
                    for &r in &rows {
                        let v = f.columns[col].text[r].clone().expect("complete");
                        seen.insert(natural(&v), v);
                    }
                    seen.into_values().collect()
                };
                let (la, lb) = (levels(1), levels(2));
                let index = |col: usize, values: &[String]| -> Vec<usize> { rows.iter().map(|&r| level_of(values, f.columns[col].text[r].as_deref()).expect("listed")).collect() };
                let (ia, ib) = (index(1, &la), index(2, &lb));
                let y: Vec<f64> = rows.iter().map(|&r| f.columns[0].num[r].expect("complete")).collect();
                let label_a = |i: usize| f.label_of(1, &la[i]);
                let label_b = |j: usize| f.label_of(2, &lb[j]);
                let tw = two_way(&y, &ia, &ib).map_err(|e| {
                    Error::Invalid(match e {
                        TwoWayError::Input => "the values could not be read as numbers and groups.".to_string(),
                        TwoWayError::OneLevel => format!("'{}' and '{}' each need at least two different values among the cases that have all three values, and one of them has only one.", f.name(1), f.name(2)),
                        TwoWayError::EmptyCell { a, b } => format!("no case has both '{}' of '{}' and '{}' of '{}': every combination of the two variables needs at least one case. Leave a value out of one of them, or choose other variables.", label_a(a), f.name(1), label_b(b), f.name(2)),
                        TwoWayError::TooFewCases => format!("there are {} cases and {} combinations of the two variables: there must be more cases than combinations, so that the differences within a combination can be measured.", rows.len(), la.len() * lb.len()),
                        TwoWayError::NoVariation => "every value of the number is the same, so there is nothing to explain.".to_string(),
                    })
                })?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(tw.cells.iter().map(|c| c.n).min().unwrap_or(0)));
                if let Some(l) = &tw.levene {
                    let sds: Vec<f64> = tw.cells.iter().filter_map(|c| c.sd).collect();
                    let ratio = sds.iter().copied().fold(0.0, f64::max) / sds.iter().copied().fold(f64::INFINITY, f64::min);
                    assumptions.push(assumption("equal_variances", if l.p < 0.05 { Status::Caution } else { Status::Ok }, None, &[("levene_f", l.f), ("levene_p", l.p), ("sd_ratio", ratio)]));
                }
                assumptions.extend(shape_checks(&tw.residuals, Some("the residuals")));
                // The cells, as groups: each opens its cases and is drawn in the charts.
                let mut cell_values: Vec<Vec<f64>> = vec![Vec::new(); la.len() * lb.len()];
                let mut cell_cases: Vec<Vec<i64>> = vec![Vec::new(); la.len() * lb.len()];
                for (i, &r) in rows.iter().enumerate() {
                    let c = ia[i] * lb.len() + ib[i];
                    cell_values[c].push(y[i]);
                    cell_cases[c].push(f.cases[r].0);
                }
                let triples: Vec<(String, String, Vec<f64>)> = (0..la.len() * lb.len())
                    .map(|c| (format!("{}\u{1f}{}", la[c / lb.len()], lb[c % lb.len()]), format!("{} \u{d7} {}", label_a(c / lb.len()), label_b(c % lb.len())), cell_values[c].clone()))
                    .collect();
                groups = triples
                    .iter()
                    .enumerate()
                    .map(|(c, (value, label, values))| ResultGroup { value: value.clone(), label: label.clone(), n: values.len(), mean: mean_of(values), median: median_of(values), case_ids: cell_cases[c].clone() })
                    .collect();
                charts.extend(box_chart(Subject::Variable { index: 0 }, 1, &triples));
                charts.extend(means_chart(Subject::Variable { index: 0 }, 1, o.level, &triples));
                let labels_a: Vec<String> = (0..la.len()).map(label_a).collect();
                let labels_b: Vec<String> = (0..lb.len()).map(label_b).collect();
                details = json!({ "factorial": tw, "labels_a": labels_a, "labels_b": labels_b, "levels_a": la, "levels_b": lb });
            }
            Procedure::CronbachAlpha => {
                let k = req.variables.len();
                for i in 0..k {
                    f.require_numeric(i, true)?;
                }
                let cols: Vec<usize> = (0..k).collect();
                let rows = f.complete_rows(&cols);
                used = rows.len();
                let columns: Vec<Vec<f64>> = (0..k).map(|c| rows.iter().map(|&r| f.columns[c].num[r].expect("complete")).collect()).collect();
                let a = cronbach_alpha(&columns, o.level).ok_or_else(|| Error::Invalid("Cronbach's alpha needs at least three cases that have a value for every item, and a sum of the items that is not the same for every case.".to_string()))?;
                assumptions.push(assumption("independence", Status::Info, None, &[]));
                assumptions.push(size_check(rows.len()));
                let ids: Vec<i64> = rows.iter().map(|&r| f.cases[r].0).collect();
                groups = (0..k)
                    .map(|c| ResultGroup { value: f.variable(c).key.clone(), label: f.name(c), n: rows.len(), mean: mean_of(&columns[c]), median: median_of(&columns[c]), case_ids: ids.clone() })
                    .collect();
                details = json!({ "alpha": a });
            }
        }

        let mut details = details;
        if let (Some(spec), Some(data)) = (&boot, &boot_data) {
            let items = bootstrap_items(data, spec, o.level)?;
            details["bootstrap"] = json!({ "seed": spec.seed, "resamples": spec.resamples, "level": spec.level, "items": items });
        }

        let variables = f
            .columns
            .iter()
            .map(|c| ResultVariable { key: c.var.key.clone(), name: c.var.name.clone(), kind: c.var.kind, code_id: c.var.code_id, measure: c.var.measure })
            .collect();
        Ok(StatResult { procedure: req.procedure, variables, cases_total: f.cases.len(), n_used: used, n_excluded: f.cases.len() - used, groups, assumptions, charts, details, weight: f.weight.clone() })
    }
}

/// The table of counts of two categorical columns over `rows`, with the values of each in their natural order.
fn contingency_table(f: &Frame, rows: &[usize]) -> (Vec<String>, Vec<String>, Vec<Vec<f64>>) {
    let mut row_values: BTreeMap<Vec<(bool, String, u64)>, String> = BTreeMap::new();
    let mut col_values: BTreeMap<Vec<(bool, String, u64)>, String> = BTreeMap::new();
    for &r in rows {
        let (a, b) = (f.columns[0].text[r].clone().expect("complete"), f.columns[1].text[r].clone().expect("complete"));
        row_values.insert(natural(&a), a);
        col_values.insert(natural(&b), b);
    }
    let row_values: Vec<String> = row_values.into_values().collect();
    let col_values: Vec<String> = col_values.into_values().collect();
    let mut table = vec![vec![0.0; col_values.len()]; row_values.len()];
    for &r in rows {
        let a = level_of(&row_values, f.columns[0].text[r].as_deref()).expect("listed");
        let b = level_of(&col_values, f.columns[1].text[r].as_deref()).expect("listed");
        table[a][b] += 1.0;
    }
    (row_values, col_values, table)
}

/// The groups of a comparison as (value, label, values), for the box and mean charts.
fn group_triples(g: &[Group]) -> Vec<(String, String, Vec<f64>)> {
    g.iter().map(|x| (x.value.clone(), x.label.clone(), x.values.clone())).collect()
}

// ---------------------------------------------------------------- regression

/// The most predictors a regression takes; more is not a model a person can read.
const MAX_PREDICTORS: usize = 20;
/// The most cases the bootstrap is run on (and for Kendall's tau, whose cost grows with the square of the cases).
const MAX_BOOTSTRAP_CASES: usize = 5000;
const MAX_BOOTSTRAP_CASES_KENDALL: usize = 500;

/// One predictor term of a regression: a number, or one category of a categorical variable against its reference.
struct TermInfo {
    key: String,
    name: String,
    kind: VariableKind,
    dummy: bool,
    level: Option<String>,
    label: Option<String>,
    reference: Option<String>,
    step: usize,
}

fn regression_error(e: RegressionError, terms: &[TermInfo], outcome: &str, n: usize, parameters: usize) -> Error {
    match e {
        RegressionError::TooFewCases => Error::Invalid(format!("There are {n} cases and the model has {parameters} parameters to estimate (the intercept and each predictor). A regression needs more cases than parameters; remove predictors, or use more cases.")),
        RegressionError::NoVariationInOutcome => Error::Invalid(format!("'{outcome}' has the same value for every case used, so there is nothing for the predictors to explain.")),
        RegressionError::Collinear(i) => {
            let what = terms.get(i).map(|t| match &t.label {
                Some(l) => format!("{} ({l})", t.name),
                None => t.name.clone(),
            });
            Error::Invalid(format!(
                "'{}' is the same for every case used, or can be worked out exactly from the other predictors, so the model cannot tell their effects apart. Leave it out, or leave out the predictor it repeats.",
                what.unwrap_or_else(|| "A predictor".to_string())
            ))
        }
        RegressionError::NotFinite => Error::Invalid("a value in the regression is missing or not a number".to_string()),
    }
}

/// A model as numbers, without the case-by-case columns (which can be thousands of values).
fn model_summary(m: &Regression) -> Value {
    let mut v = serde_json::to_value(m).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        for k in ["fitted", "residuals", "leverage", "cooks_distance", "standardized_residuals", "studentized_residuals"] {
            o.remove(k);
        }
    }
    v
}

/// The predictor columns, what each stands for, and (step, number of terms up to and including it) for each step.
type PredictorColumns = (Vec<Vec<f64>>, Vec<TermInfo>, Vec<(usize, usize)>);

/// The step each predictor is entered in (all in step 1 when none are given).
fn steps_for(o: &StatOptions, predictors: usize) -> Result<Vec<usize>> {
    match &o.steps {
        None => Ok(vec![1; predictors]),
        Some(s) if s.len() == predictors && s.iter().all(|&x| (1..=MAX_PREDICTORS).contains(&x)) => Ok(s.clone()),
        Some(_) => Err(Error::Invalid("give a step (1, 2, ...) for each predictor, or none at all".to_string())),
    }
}

/// The predictor columns of a regression, ordered by step (and by their place in the request within a step) so every model holds the
/// first terms: a number is one column, and a categorical variable is a 0/1 column for each of its values but the reference one.
/// Gives the columns, what each stands for, and (step, number of terms up to and including it) for each step.
fn predictor_columns(f: &Frame, o: &StatOptions, rows: &[usize], steps_given: &[usize], assumptions: &mut Vec<Assumption>) -> Result<PredictorColumns> {
    let nvars = f.columns.len();
    // The predictors, ordered by step (and by their place in the request within a step), so every model holds the first terms.
    let mut order: Vec<usize> = (1..nvars).collect();
    order.sort_by_key(|&v| (steps_given[v - 1], v));
    let mut columns: Vec<Vec<f64>> = Vec::new();
    let mut terms: Vec<TermInfo> = Vec::new();
    let mut step_ends: Vec<(usize, usize)> = Vec::new(); // (step, number of terms up to and including it)
    for &v in &order {
        let var = f.variable(v).clone();
        if !var.usable() {
            return Err(Error::Invalid(format!("'{}' holds dates or free text, which a regression cannot use.", var.name)));
        }
        let step = steps_given[v - 1];
        let numeric = var.measure == Measure::Scale || (var.measure == Measure::Ordinal && var.numeric);
        if numeric {
            if var.measure == Measure::Ordinal {
                assumptions.push(assumption("ordinal_as_number", Status::Caution, Some(&var.name), &[]));
            }
            columns.push(rows.iter().map(|&r| f.columns[v].num[r].expect("complete")).collect());
            terms.push(TermInfo { key: var.key.clone(), name: var.name.clone(), kind: var.kind, dummy: false, level: None, label: None, reference: None, step });
        } else {
            let values: Vec<String> = rows.iter().map(|&r| f.columns[v].text[r].clone().expect("complete")).collect();
            let mut cats: Vec<String> = values.iter().cloned().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            cats.sort_by_key(|c| natural(c));
            if cats.len() < 2 {
                return Err(Error::Invalid(format!("'{}' has only one value among the cases used, so it cannot explain anything.", var.name)));
            }
            let reference = o.references.get(&var.key).filter(|r| cats.contains(r)).cloned().unwrap_or_else(|| cats[0].clone());
            for (col, cat) in dummy_columns(&values, &cats, &reference).into_iter().zip(cats.iter().filter(|c| **c != reference)) {
                columns.push(col);
                terms.push(TermInfo {
                    key: var.key.clone(),
                    name: var.name.clone(),
                    kind: var.kind,
                    dummy: true,
                    level: Some(cat.clone()),
                    label: Some(f.label_of(v, cat)),
                    reference: Some(f.label_of(v, &reference)),
                    step,
                });
            }
        }
        match step_ends.last_mut() {
            Some(last) if last.0 == step => last.1 = columns.len(),
            _ => step_ends.push((step, columns.len())),
        }
    }

    Ok((columns, terms, step_ends))
}

/// Fits the regression the request asks for: dummy-codes categorical predictors, enters predictors in the steps given, and checks the
/// conditions the model depends on.
fn regression_result(f: &Frame, req: &StatRequest, assumptions: &mut Vec<Assumption>, charts: &mut Vec<Chart>) -> Result<(usize, Value, BootData)> {
    let o = &req.options;
    f.require_numeric(0, false)?;
    let nvars = f.columns.len();
    let predictors = nvars - 1;
    let steps_given = steps_for(o, predictors)?;
    let all: Vec<usize> = (0..nvars).collect();
    let rows = f.complete_rows(&all);
    let n = rows.len();
    let y: Vec<f64> = rows.iter().map(|&r| f.columns[0].num[r].expect("complete")).collect();
    let (columns, terms, step_ends) = predictor_columns(f, o, &rows, &steps_given, assumptions)?;

    let outcome = f.name(0);
    let mut models: Vec<Regression> = Vec::new();
    for &(_, upto) in &step_ends {
        models.push(linear(&y, &columns[..upto], o.level).map_err(|e| regression_error(e, &terms[..upto], &outcome, n, upto + 1))?);
    }
    let last = models.last().expect("at least one step");
    let changes: Vec<Value> = models.windows(2).map(|w| json!(compare(&w[0], &w[1]))).collect();

    // The conditions the model depends on.
    assumptions.push(assumption("independence", Status::Info, None, &[]));
    let per = n as f64 / terms.len() as f64;
    assumptions.push(assumption("cases_per_predictor", if per >= 15.0 { Status::Ok } else if per >= 5.0 { Status::Caution } else { Status::Problem }, None, &[("n", n as f64), ("terms", terms.len() as f64)]));
    if terms.len() >= 2 {
        let max_vif = last.terms.iter().filter_map(|t| t.vif).fold(0.0, f64::max);
        assumptions.push(assumption("collinearity", if max_vif >= 10.0 { Status::Problem } else if max_vif >= 5.0 { Status::Caution } else { Status::Ok }, None, &[("max_vif", max_vif)]));
    }
    let cut = 4.0 / n as f64;
    let max_cook = last.cooks_distance.iter().copied().fold(0.0, f64::max);
    let many = last.cooks_distance.iter().filter(|&&c| c > cut).count();
    assumptions.push(assumption("influence", if max_cook > 1.0 { Status::Problem } else if many > 0 { Status::Caution } else { Status::Ok }, None, &[("count", many as f64), ("max_cooks_distance", max_cook), ("cut", cut)]));
    let big = last.studentized_residuals.iter().filter(|r| r.abs() > 3.0).count();
    if big > 0 {
        assumptions.push(assumption("large_residuals", Status::Caution, None, &[("count", big as f64), ("n", n as f64)]));
    }
    for mut a in shape_checks(&last.residuals, None).into_iter().filter(|a| a.id == "normality") {
        a.id = "residual_shape".to_string();
        assumptions.push(a);
    }
    if let Some(bp) = last.breusch_pagan {
        assumptions.push(assumption("equal_error_variance", if bp.p < 0.05 { Status::Caution } else { Status::Ok }, None, &[("statistic", bp.statistic), ("df", bp.df), ("p", bp.p)]));
    }

    // The cases that pull the fit most, by Cook's distance.
    let flagged = |i: usize| last.cooks_distance[i] > cut || last.studentized_residuals[i].abs() > 3.0 || last.leverage[i] > 2.0 * (terms.len() + 1) as f64 / n as f64;
    let mut influential: Vec<usize> = (0..n).filter(|&i| flagged(i)).collect();
    influential.sort_by(|&a, &b| last.cooks_distance[b].total_cmp(&last.cooks_distance[a]));
    let influential: Vec<Value> = influential
        .into_iter()
        .take(10)
        .map(|i| {
            let case = &f.cases[rows[i]];
            json!({ "case_id": case.0, "case_name": case.1, "leverage": last.leverage[i], "cooks_distance": last.cooks_distance[i], "studentized_residual": last.studentized_residuals[i], "residual": last.residuals[i], "fitted": last.fitted[i] })
        })
        .collect();

    let term_json: Vec<Value> = terms
        .iter()
        .map(|t| json!({ "variable": t.key, "name": t.name, "kind": t.kind, "dummy": t.dummy, "level": t.level, "label": t.label, "reference": t.reference, "step": t.step }))
        .collect();
    let steps: Vec<Value> = step_ends.iter().map(|&(s, upto)| json!({ "step": s, "terms": upto })).collect();
    let details = json!({
        "models": models.iter().map(model_summary).collect::<Vec<_>>(),
        "terms": term_json,
        "steps": steps,
        "changes": changes,
        "influential": influential,
        "level": o.level,
    });
    // The charts: what the model predicts against what it leaves out, whether what it leaves out is close to normal, and, when there
    // is one number predicting, the data with the line.
    let ids: Vec<i64> = rows.iter().map(|&r| f.cases[r].0).collect();
    charts.extend(scatter_chart(Subject::Fitted, Subject::Residuals, &last.fitted, &last.residuals, &ids, false, true));
    charts.extend(qq_chart(Subject::Residuals, &last.residuals));
    if columns.len() == 1 && !terms[0].dummy && last.terms.len() == 2 {
        let line = prismqda_stats::charts::Line { intercept: last.terms[0].estimate, slope: last.terms[1].estimate };
        charts.extend(scatter_with_line(Subject::Variable { index: 1 }, Subject::Variable { index: 0 }, &columns[0], &y, &ids, line));
    }
    Ok((n, details, BootData::Regression { y, columns, level: o.level }))
}

// ---------------------------------------------------------------- curve fits

/// A fitted curve as numbers, without the case-by-case columns of its regression.
fn curve_summary(c: &CurveFit) -> Value {
    json!({
        "kind": c.kind,
        "n": c.n,
        "model": model_summary(&c.regression),
        "r_squared_original": c.r_squared_original,
        "rmse_original": c.rmse_original,
        "constants": c.constants,
    })
}

fn curve_error(e: CurveError, outcome: &str, predictor: &str, n: usize) -> Error {
    Error::Invalid(match e {
        CurveError::TooFewCases => format!("There are {n} cases with both '{outcome}' and '{predictor}', too few to fit a curve. A curve needs more cases than constants to estimate."),
        CurveError::NotFinite => "a value in the curve fit is missing or not a number".to_string(),
        CurveError::NeedsPositiveX => format!("This curve takes the logarithm of '{predictor}', which must be above zero in every case."),
        CurveError::NeedsNonzeroX => format!("This curve divides by '{predictor}', which must not be zero in any case."),
        CurveError::NeedsPositiveY => format!("This curve takes the logarithm of '{outcome}', which must be above zero in every case."),
        CurveError::NoVariationInOutcome => format!("'{outcome}' has the same value for every case used, so there is nothing for a curve to explain."),
        CurveError::TooFewValues => format!("'{predictor}' does not take enough different values for this curve (a straight line needs 2, a quadratic 3, a cubic 4)."),
    })
}

// ---------------------------------------------------------------- logistic regression

/// The most cases a bootstrap of a logistic regression is run on (every resample is a fit by iteration).
const MAX_BOOTSTRAP_CASES_LOGISTIC: usize = 1000;
/// The fits the profile-likelihood intervals may cost, counted as cases times parameters cubed.
const MAX_PROFILE_WORK: usize = 200_000_000;

fn logistic_error(e: LogisticError, terms: &[TermInfo], outcome: &str, n: usize, parameters: usize) -> Error {
    match e {
        LogisticError::TooFewCases => Error::Invalid(format!("There are {n} cases and the model has {parameters} parameters to estimate (the intercept and each predictor). A logistic regression needs more cases than parameters, and in practice at least ten cases of the rarer outcome for each predictor; remove predictors, or use more cases.")),
        LogisticError::NoVariationInOutcome => Error::Invalid(format!("'{outcome}' has the same value for every case used, so there is nothing for the predictors to explain.")),
        LogisticError::NotBinary => Error::Invalid(format!("'{outcome}' must have exactly two values among the cases used.")),
        LogisticError::Collinear(i) => {
            let what = terms.get(i).map(|t| match &t.label {
                Some(l) => format!("{} ({l})", t.name),
                None => t.name.clone(),
            });
            Error::Invalid(format!(
                "'{}' is the same for every case used, or can be worked out exactly from the other predictors, so the model cannot tell their effects apart. Leave it out, or leave out the predictor it repeats.",
                what.unwrap_or_else(|| "A predictor".to_string())
            ))
        }
        LogisticError::NotFinite => Error::Invalid("a value in the logistic regression is missing or not a number".to_string()),
        LogisticError::Separation => Error::Invalid("The predictors separate the two outcomes perfectly, or almost perfectly (for example, every case in one category has the same outcome), so the coefficients have no best value and any estimate would be meaningless. Leave out the predictor that does it, join categories, or use more cases.".to_string()),
    }
}

/// A logistic model as numbers, without the case-by-case columns (which can be thousands of values).
fn logistic_summary(m: &Logistic) -> Value {
    let mut v = serde_json::to_value(m).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        for k in ["fitted", "leverage", "cooks_distance", "pearson_residuals", "deviance_residuals", "standardized_residuals"] {
            o.remove(k);
        }
    }
    v
}

/// Fits the logistic regression the request asks for: the outcome has two values among the cases used (the one modelled is the
/// `event` option, or `1`, or else the later of the two), categorical predictors become 0/1 columns, predictors are entered in the
/// steps given, and the conditions the model depends on are checked.
fn logistic_result(f: &Frame, req: &StatRequest, assumptions: &mut Vec<Assumption>, _charts: &mut Vec<Chart>) -> Result<(usize, Value, BootData)> {
    let o = &req.options;
    let outcome_var = f.variable(0).clone();
    if !outcome_var.usable() {
        return Err(Error::Invalid(format!("'{}' holds dates or free text, which a logistic regression cannot explain.", outcome_var.name)));
    }
    let nvars = f.columns.len();
    let steps_given = steps_for(o, nvars - 1)?;
    let all: Vec<usize> = (0..nvars).collect();
    let rows = f.complete_rows(&all);
    let n = rows.len();
    let mut values: Vec<String> = rows.iter().map(|&r| f.columns[0].text[r].clone().expect("complete")).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    values.sort_by_key(|c| natural(c));
    let outcome = f.name(0);
    if values.len() != 2 {
        return Err(Error::Invalid(format!("A logistic regression explains an outcome with exactly two values, and '{outcome}' has {} among the cases used. Use a variable with two values (or cut a number in two in the data editor first), or use another procedure.", values.len())));
    }
    if let Some(e) = &o.event {
        if !values.contains(e) {
            return Err(Error::Invalid(format!("'{e}' is not one of the two values of '{outcome}' among the cases used.")));
        }
    }
    let event = o.event.clone().unwrap_or_else(|| if values.iter().any(|v| v == "1") { "1".to_string() } else { values[1].clone() });
    let other = values.iter().find(|v| **v != event).cloned().expect("two values");
    let y: Vec<f64> = rows.iter().map(|&r| if f.columns[0].text[r].as_deref() == Some(event.as_str()) { 1.0 } else { 0.0 }).collect();
    let (columns, terms, step_ends) = predictor_columns(f, o, &rows, &steps_given, assumptions)?;

    let profile = n * (terms.len() + 1).pow(3) <= MAX_PROFILE_WORK;
    let mut models: Vec<Logistic> = Vec::new();
    for (i, &(_, upto)) in step_ends.iter().enumerate() {
        let is_last = i + 1 == step_ends.len();
        models.push(logistic(&y, &columns[..upto], o.level, profile && is_last).map_err(|e| logistic_error(e, &terms[..upto], &outcome, n, upto + 1))?);
    }
    let last = models.last().expect("at least one step");
    let changes: Vec<Value> = models.windows(2).map(|w| json!(logistic_compare(&w[0], &w[1]))).collect();

    // Each variable left out in turn (all its columns together): the likelihood-ratio test of what it adds.
    let mut blocks: Vec<(String, String, VariableKind, usize, Vec<usize>)> = Vec::new();
    for (i, t) in terms.iter().enumerate() {
        match blocks.iter_mut().find(|b| b.0 == t.key) {
            Some(b) => b.4.push(i),
            None => blocks.push((t.key.clone(), t.name.clone(), t.kind, t.step, vec![i])),
        }
    }
    let block_json: Vec<Value> = blocks
        .iter()
        .map(|(key, name, kind, step, cols)| json!({ "variable": key, "name": name, "kind": kind, "step": step, "terms": cols.len(), "lr": drop_test(&y, &columns, last.deviance, cols) }))
        .collect();

    // The conditions the model depends on.
    assumptions.push(assumption("independence", Status::Info, None, &[]));
    let rarer = last.events.min(last.non_events) as f64;
    let per = rarer / terms.len() as f64;
    assumptions.push(assumption("events_per_predictor", if per >= 10.0 { Status::Ok } else if per >= 5.0 { Status::Caution } else { Status::Problem }, None, &[("events", rarer), ("terms", terms.len() as f64)]));
    if terms.len() >= 2 {
        // The variance inflation factors depend on the predictors alone, so they are read off a least-squares fit of the outcome.
        if let Ok(lin) = linear(&y, &columns, o.level) {
            let max_vif = lin.terms.iter().filter_map(|t| t.vif).fold(0.0, f64::max);
            assumptions.push(assumption("collinearity", if max_vif >= 10.0 { Status::Problem } else if max_vif >= 5.0 { Status::Caution } else { Status::Ok }, None, &[("max_vif", max_vif)]));
        }
    }
    let cut = 4.0 / n as f64;
    let max_cook = last.cooks_distance.iter().copied().fold(0.0, f64::max);
    let many = last.cooks_distance.iter().filter(|&&c| c > cut).count();
    assumptions.push(assumption("influence", if max_cook > 1.0 { Status::Problem } else if many > 0 { Status::Caution } else { Status::Ok }, None, &[("count", many as f64), ("max_cooks_distance", max_cook), ("cut", cut)]));
    let big = last.standardized_residuals.iter().filter(|r| r.abs() > 3.0).count();
    if big > 0 {
        assumptions.push(assumption("large_residuals", Status::Caution, None, &[("count", big as f64), ("n", n as f64)]));
    }
    if let Some(hl) = &last.hosmer_lemeshow {
        assumptions.push(assumption("hosmer_lemeshow", if hl.p < 0.05 { Status::Caution } else { Status::Ok }, None, &[("chi2", hl.chi2), ("df", hl.df), ("p", hl.p)]));
    }

    // The cases that pull the fit most, by Cook's distance.
    let flagged = |i: usize| last.cooks_distance[i] > cut || last.standardized_residuals[i].abs() > 3.0;
    let mut influential: Vec<usize> = (0..n).filter(|&i| flagged(i)).collect();
    influential.sort_by(|&a, &b| last.cooks_distance[b].total_cmp(&last.cooks_distance[a]));
    let influential: Vec<Value> = influential
        .into_iter()
        .take(10)
        .map(|i| {
            let case = &f.cases[rows[i]];
            json!({ "case_id": case.0, "case_name": case.1, "leverage": last.leverage[i], "cooks_distance": last.cooks_distance[i], "standardized_residual": last.standardized_residuals[i], "fitted": last.fitted[i], "observed": y[i] })
        })
        .collect();

    let term_json: Vec<Value> = terms
        .iter()
        .map(|t| json!({ "variable": t.key, "name": t.name, "kind": t.kind, "dummy": t.dummy, "level": t.level, "label": t.label, "reference": t.reference, "step": t.step }))
        .collect();
    let steps: Vec<Value> = step_ends.iter().map(|&(s, upto)| json!({ "step": s, "terms": upto })).collect();
    let details = json!({
        "event": { "value": event, "label": f.label_of(0, &event) },
        "other": { "value": other, "label": f.label_of(0, &other) },
        "models": models.iter().map(logistic_summary).collect::<Vec<_>>(),
        "terms": term_json,
        "steps": steps,
        "changes": changes,
        "blocks": block_json,
        "influential": influential,
        "profile": profile,
        "level": o.level,
    });
    Ok((n, details, BootData::Logistic { y, columns, level: o.level }))
}

// ---------------------------------------------------------------- the bootstrap of a whole analysis

/// The most cases a bootstrap of a whole analysis is run on, the most work it does (cases times resamples), and how long it may take.
const MAX_ANALYSIS_CASES: usize = 5000;
const MAX_ANALYSIS_WORK: usize = 5_000_000;
const MAX_ANALYSIS_SECONDS: u64 = 180;
/// Resampling whole groups of cases needs at least this many groups.
const MIN_CLUSTERS: usize = 5;

impl Frame {
    /// The frame of the rows `idx` (a row can be drawn more than once), with the summary of each column worked out again.
    fn resample(&self, idx: &[usize]) -> Frame {
        let columns = self
            .columns
            .iter()
            .map(|c| {
                let text: Vec<Option<String>> = idx.iter().map(|&i| c.text[i].clone()).collect();
                let num: Vec<Option<f64>> = idx.iter().map(|&i| c.num[i]).collect();
                let labels: BTreeMap<String, String> = c.var.categories.iter().map(|k| (k.value.clone(), k.label.clone())).collect();
                Column { var: finish(c.var.clone(), &text, &num, &labels), text, num }
            })
            .collect();
        Frame { cases: idx.iter().map(|&i| self.cases[i].clone()).collect(), columns, weight: self.weight.clone() }
    }
}

/// Every number in a result's details, by its path (`t_test/p`, `models/0/terms/1/estimate`). The bootstrap's own intervals and the
/// list of influential cases are not numbers of the analysis, and a long list of values is left out.
fn flatten_numbers(value: &Value, path: &mut Vec<String>, out: &mut BTreeMap<String, f64>) {
    match value {
        Value::Number(n) => {
            if let Some(x) = n.as_f64().filter(|x| x.is_finite()) {
                out.insert(path.join("/"), x);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                if matches!(k.as_str(), "bootstrap" | "bootstrap_analysis" | "influential") {
                    continue;
                }
                path.push(k.clone());
                flatten_numbers(v, path, out);
                path.pop();
            }
        }
        Value::Array(items) if items.len() <= 200 => {
            for (i, v) in items.iter().enumerate() {
                path.push(i.to_string());
                flatten_numbers(v, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

/// True for the path of a p-value.
fn is_p_value(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or("");
    last == "p" || last.starts_with("p_") || last.ends_with("_p")
}

/// Runs the whole analysis again on resamples of the cases (single cases, or whole groups of cases when `clusters` is given), and
/// gives the spread of every number the analysis reports: the estimate on the cases as they are, the bias, the standard error, the
/// percentile interval, and, for a p-value, the share of resamples in which it fell below .05.
fn analysis_bootstrap(f: &Frame, req: &StatRequest, original: &StatResult, b: &BootstrapOptions, clusters: Option<(String, Vec<Vec<usize>>)>) -> Result<Value> {
    let n = f.cases.len();
    let level = req.options.level;
    if n > MAX_ANALYSIS_CASES {
        return Err(Error::Invalid(format!("A bootstrap of the whole analysis is offered for up to {MAX_ANALYSIS_CASES} cases, and this analysis has {n}. Leave the bootstrap off, or analyze a smaller set of cases.")));
    }
    if n * b.resamples > MAX_ANALYSIS_WORK {
        return Err(Error::Invalid(format!("{n} cases resampled {} times is more than a bootstrap of the whole analysis will do ({} runs of the analysis in all). Use fewer resamples (at most {} for these cases).", b.resamples, n * b.resamples, MAX_ANALYSIS_WORK / n)));
    }
    if req.procedure == Procedure::Kendall && n > MAX_BOOTSTRAP_CASES_KENDALL {
        return Err(Error::Invalid(format!("Kendall's tau is bootstrapped for up to {MAX_BOOTSTRAP_CASES_KENDALL} cases, and this analysis has {n}.")));
    }
    let mut inner = req.clone();
    inner.options.bootstrap = None;
    let mut base: BTreeMap<String, f64> = BTreeMap::new();
    flatten_numbers(&original.details, &mut Vec::new(), &mut base);
    let mut replicates: BTreeMap<&str, Vec<f64>> = base.keys().map(|k| (k.as_str(), Vec::with_capacity(b.resamples))).collect();
    let mut rng = SplitMix64::new(b.seed);
    let started = std::time::Instant::now();
    let mut failed = 0usize;
    let mut idx: Vec<usize> = Vec::with_capacity(n);
    for _ in 0..b.resamples {
        idx.clear();
        match &clusters {
            Some((_, groups)) => {
                for _ in 0..groups.len() {
                    idx.extend_from_slice(&groups[rng.below(groups.len())]);
                }
            }
            None => idx.extend((0..n).map(|_| rng.below(n))),
        }
        match run_on_frame(&f.resample(&idx), &inner, None) {
            Ok(r) => {
                let mut got = BTreeMap::new();
                flatten_numbers(&r.details, &mut Vec::new(), &mut got);
                for (path, values) in replicates.iter_mut() {
                    if let Some(v) = got.get(*path) {
                        values.push(*v);
                    }
                }
            }
            Err(_) => failed += 1,
        }
        if started.elapsed().as_secs() > MAX_ANALYSIS_SECONDS {
            return Err(Error::Invalid(format!("The bootstrap of the whole analysis took longer than {} minutes and was stopped. Use fewer resamples or fewer cases.", MAX_ANALYSIS_SECONDS / 60)));
        }
    }
    if failed * 2 > b.resamples {
        return Err(Error::Invalid(format!("The analysis could not be run on {failed} of the {} resamples (for example, a group was left out of every one of them), so there is nothing sound to report. Use more cases, or leave the bootstrap off.", b.resamples)));
    }
    let items: Vec<Value> = base
        .iter()
        .filter_map(|(path, estimate)| {
            let values = &replicates[path.as_str()];
            let interval = intervals_from_replicates(*estimate, values, None, level, b.resamples)?;
            // A p-value that is .05 to within rounding (an exact test can give exactly .05) is not below it, however the rounding falls.
            let below = is_p_value(path).then(|| values.iter().filter(|v| **v < 0.05 - 1e-9).count() as f64 / values.len() as f64);
            Some(json!({ "path": path, "interval": interval, "below_05": below }))
        })
        .collect();
    let cluster = clusters.as_ref().map(|(name, groups)| json!({ "key": b.cluster, "name": name, "groups": groups.len() }));
    Ok(json!({ "seed": b.seed, "resamples": b.resamples, "level": level, "failed": failed, "cluster": cluster, "items": items }))
}

// ---------------------------------------------------------------- the bootstrap

/// What a bootstrap resamples, kept from the procedure that was run.
enum BootData {
    /// The mean, the median, and the standard deviation of one variable.
    Summary(Vec<f64>),
    Mean(Vec<f64>),
    Median(Vec<f64>),
    /// The difference of two groups' means (or medians), resampled within each group.
    Difference { a: Vec<f64>, b: Vec<f64>, medians: bool },
    /// A correlation, resampling the pairs.
    Correlation { x: Vec<f64>, y: Vec<f64>, kind: Procedure },
    /// The coefficients and R squared of a regression, resampling the cases.
    Regression { y: Vec<f64>, columns: Vec<Vec<f64>>, level: f64 },
    /// The coefficients and the area under the curve of a logistic regression, resampling the cases.
    Logistic { y: Vec<f64>, columns: Vec<Vec<f64>>, level: f64 },
}

fn mean_idx(v: &[f64], idx: &[usize]) -> f64 {
    idx.iter().map(|&i| v[i]).sum::<f64>() / idx.len() as f64
}

fn median_idx(v: &[f64], idx: &[usize]) -> Option<f64> {
    let picked: Vec<f64> = idx.iter().map(|&i| v[i]).collect();
    median_of(&picked)
}

fn item(statistic: &str, target: Option<usize>, interval: Option<BootstrapInterval>) -> Value {
    json!({ "statistic": statistic, "target": target, "interval": interval })
}

fn guard(n: usize, limit: usize) -> Result<()> {
    if n > limit {
        return Err(Error::Invalid(format!("Bootstrap intervals are offered for up to {limit} cases, and this analysis has {n}. Leave the bootstrap off, or analyze a smaller set of cases.")));
    }
    Ok(())
}

fn bootstrap_items(data: &BootData, spec: &BootstrapSpec, level: f64) -> Result<Vec<Value>> {
    Ok(match data {
        BootData::Summary(v) => {
            guard(v.len(), MAX_BOOTSTRAP_CASES)?;
            let sd_idx = |idx: &[usize]| {
                let picked: Vec<f64> = idx.iter().map(|&i| v[i]).collect();
                prismqda_stats::describe::variance(&picked).map(f64::sqrt)
            };
            let mut r = bootstrap_many(v.len(), 3, spec, |idx| Some(vec![mean_idx(v, idx), median_idx(v, idx)?, sd_idx(idx)?])).into_iter();
            vec![item("mean", None, r.next().flatten()), item("median", None, r.next().flatten()), item("sd", None, r.next().flatten())]
        }
        BootData::Mean(v) => {
            guard(v.len(), MAX_BOOTSTRAP_CASES)?;
            vec![item("mean", None, bootstrap(v.len(), spec, |idx| Some(mean_idx(v, idx))))]
        }
        BootData::Median(v) => {
            guard(v.len(), MAX_BOOTSTRAP_CASES)?;
            vec![item("median", None, bootstrap(v.len(), spec, |idx| median_idx(v, idx)))]
        }
        BootData::Difference { a, b, medians } => {
            guard(a.len() + b.len(), MAX_BOOTSTRAP_CASES)?;
            let stat = |idx: &[Vec<usize>]| {
                if *medians {
                    Some(median_idx(a, &idx[0])? - median_idx(b, &idx[1])?)
                } else {
                    Some(mean_idx(a, &idx[0]) - mean_idx(b, &idx[1]))
                }
            };
            vec![item(if *medians { "median_difference" } else { "mean_difference" }, None, bootstrap_groups(&[a.len(), b.len()], spec, stat))]
        }
        BootData::Correlation { x, y, kind } => {
            guard(x.len(), if *kind == Procedure::Kendall { MAX_BOOTSTRAP_CASES_KENDALL } else { MAX_BOOTSTRAP_CASES })?;
            let stat = |idx: &[usize]| {
                let xs: Vec<f64> = idx.iter().map(|&i| x[i]).collect();
                let ys: Vec<f64> = idx.iter().map(|&i| y[i]).collect();
                match kind {
                    Procedure::Pearson => pearson(&xs, &ys, level),
                    Procedure::Spearman => spearman(&xs, &ys, level),
                    _ => kendall(&xs, &ys, level),
                }
                .map(|c| c.r)
            };
            vec![item("correlation", None, bootstrap(x.len(), spec, stat))]
        }
        BootData::Regression { y, columns, level } => {
            guard(y.len(), MAX_BOOTSTRAP_CASES)?;
            let m = columns.len() + 2; // the intercept, each predictor, and R squared
            let intervals = bootstrap_many(y.len(), m, spec, |idx| {
                let ys: Vec<f64> = idx.iter().map(|&i| y[i]).collect();
                let cols: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
                let r = linear(&ys, &cols, *level).ok()?;
                let mut out: Vec<f64> = r.terms.iter().map(|t| t.estimate).collect();
                out.push(r.r_squared);
                Some(out)
            });
            let last = intervals.len() - 1;
            intervals.into_iter().enumerate().map(|(i, iv)| if i == last { item("r_squared", None, iv) } else { item("coefficient", Some(i), iv) }).collect()
        }
        BootData::Logistic { y, columns, level } => {
            guard(y.len(), MAX_BOOTSTRAP_CASES_LOGISTIC)?;
            let m = columns.len() + 2; // the intercept, each predictor, and the area under the curve
            let intervals = bootstrap_many(y.len(), m, spec, |idx| {
                let ys: Vec<f64> = idx.iter().map(|&i| y[i]).collect();
                let cols: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
                let r = logistic(&ys, &cols, *level, false).ok()?;
                let mut out: Vec<f64> = r.terms.iter().map(|t| t.estimate).collect();
                out.push(r.auc);
                Some(out)
            });
            let last = intervals.len() - 1;
            intervals.into_iter().enumerate().map(|(i, iv)| if i == last { item("auc", None, iv) } else { item("coefficient", Some(i), iv) }).collect()
        }
    })
}
