//! Exploring the coding: summaries of a code, matrices of codes against cases or documents or the values of a case variable,
//! how codes occur together, and queries that turn a research question into a rule.
//!
//! Counting is explicit everywhere. A number is always one of four things, and the caller says which it wants:
//! * **assignments**: how many codings there are (a coding is one code applied to one stretch of one document by one coder);
//! * **excerpts**: how many distinct stretches there are, so two coders (or two codes) on the same words count once;
//! * **documents** and **cases**: how many have at least one qualifying coding.
//!
//! A parent code counts the codings of its sub-codes too when asked to (`include_descendants`), but it never adds up the counts
//! of its children: it looks at the codings themselves, so evidence that two sub-codes share is counted once.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::regions::{overlaps, Shape};
use crate::{Error, Result};

mod bubble;
mod contrast;
mod joint;
mod similarity;
pub use bubble::{Bubble, BubbleAxis, BubbleCell, BubbleSpec};
pub use contrast::{Deviation, DeviationGroup, DeviationRow, DeviationSpec, SideBySide, SideBySideSpec, SideBySideTopic, SideGroup, TopicQuote};
pub use similarity::{AlikePair, PairPart, Similarity, SimilarityFeature, SimilaritySpec};
pub use joint::{CaseDisplay, CaseDisplaySpec, DisplayCell, DisplayCode, DisplayQuote, DisplayRow, Typology, TypologyCase, TypologyCategory, TypologyCell, TypologyColumn, TypologyGroup, TypologySpec};

// ---------------------------------------------------------------- what to look at

/// Which codings are looked at at all. Every field left out means "all".
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Scope {
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    /// Only codings of documents (or units) that belong to one of these cases.
    #[serde(default)]
    pub case_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub coder_ids: Option<Vec<i64>>,
    /// `coder` (each coder's own codings) or `final` (the adjudicated layer).
    #[serde(default)]
    pub layer: Option<String>,
}

fn yes() -> bool {
    true
}

/// A coding, with only what exploring needs.
#[derive(Debug, Clone)]
struct Rec {
    id: i64,
    source_id: i64,
    start: i64,
    end: i64,
    code_id: i64,
    coder_id: i64,
    layer: String,
    rating: Option<i64>,
    unit_id: Option<i64>,
    /// The day the coding was made (days since 1970-01-01, by the date part of its time stamp).
    day: Option<i64>,
    /// The area, when the coding is of a part of a picture. Such a coding has a negative `id` (area 7 is -7), and its `start` and `end`
    /// are one negative number and the next, the same for every coding of the same shape on the same picture, so that the shape counts
    /// as one excerpt wherever excerpts are counted.
    shape: Option<Arc<Shape>>,
}

/// The code tree, to know which codes a parent stands for.
struct Hierarchy {
    names: HashMap<i64, String>,
    /// A code and then each of its ancestors, nearest first.
    ancestors: HashMap<i64, Vec<i64>>,
}

impl Hierarchy {
    /// True when `code` is `of`, or (when asked for) one of its descendants.
    fn is_within(&self, code: i64, of: i64, descendants: bool) -> bool {
        if code == of {
            return true;
        }
        descendants && self.ancestors.get(&code).is_some_and(|a| a.contains(&of))
    }
}

/// Which cases each document and unit belong to.
struct CaseMap {
    names: HashMap<i64, String>,
    by_source: HashMap<i64, Vec<i64>>,
    by_unit: HashMap<i64, Vec<i64>>,
    /// For documents that have units in cases: their units' spans, to find the unit a free-span coding sits in.
    unit_spans: HashMap<i64, Vec<(i64, i64, i64)>>,
}

impl CaseMap {
    /// The cases a coding belongs to: through its document, and through its unit when units are in cases.
    fn cases_of(&self, r: &Rec) -> Vec<i64> {
        let mut out: Vec<i64> = self.by_source.get(&r.source_id).cloned().unwrap_or_default();
        let unit = r.unit_id.or_else(|| {
            self.unit_spans.get(&r.source_id).and_then(|spans| spans.iter().find(|(s, e, _)| *s <= r.start && r.start < *e).map(|(_, _, u)| *u))
        });
        if let Some(cs) = unit.and_then(|u| self.by_unit.get(&u)) {
            for c in cs {
                if !out.contains(c) {
                    out.push(*c);
                }
            }
        }
        out.sort_unstable();
        out
    }
}

impl Project {
    fn hierarchy(&self) -> Result<Hierarchy> {
        let mut parent: HashMap<i64, Option<i64>> = HashMap::new();
        let mut names: HashMap<i64, String> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT id, name, parent_id FROM code")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?)))? {
            let (id, name, p) = row?;
            parent.insert(id, p);
            names.insert(id, name);
        }
        let mut ancestors = HashMap::new();
        for &id in parent.keys() {
            let mut chain = vec![id];
            let mut cur = parent[&id];
            while let Some(p) = cur {
                if chain.contains(&p) || chain.len() > 1000 {
                    break;
                }
                chain.push(p);
                cur = parent.get(&p).copied().flatten();
            }
            ancestors.insert(id, chain);
        }
        Ok(Hierarchy { names, ancestors })
    }

    fn case_map(&self) -> Result<CaseMap> {
        let mut names = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT id, name FROM research_case")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            names.insert(id, name);
        }
        let mut by_source: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut by_unit: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT case_id, source_id, unit_id FROM case_member")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<i64>>(2)?)))? {
            let (case, source, unit) = row?;
            if let Some(s) = source {
                let v = by_source.entry(s).or_default();
                if !v.contains(&case) {
                    v.push(case);
                }
            }
            if let Some(u) = unit {
                let v = by_unit.entry(u).or_default();
                if !v.contains(&case) {
                    v.push(case);
                }
            }
        }
        let mut unit_spans: HashMap<i64, Vec<(i64, i64, i64)>> = HashMap::new();
        if !by_unit.is_empty() {
            let mut stmt = self.conn.prepare("SELECT id, source_id, start_offset, end_offset FROM unit ORDER BY source_id, start_offset")?;
            for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))? {
                let (id, source, start, end) = row?;
                if by_unit.contains_key(&id) {
                    unit_spans.entry(source).or_default().push((start, end, id));
                }
            }
        }
        Ok(CaseMap { names, by_source, by_unit, unit_spans })
    }

    /// The codings inside a scope, in document and text order.
    fn scoped_codings(&self, scope: &Scope, cases: &CaseMap) -> Result<Vec<Rec>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, source_id, start_offset, end_offset, code_id, coder_id, layer, rating, unit_id, substr(created_at, 1, 10) FROM coding ORDER BY source_id, start_offset, end_offset, id")?;
        let rows = stmt.query_map([], |r| {
            Ok(Rec {
                id: r.get(0)?,
                source_id: r.get(1)?,
                start: r.get(2)?,
                end: r.get(3)?,
                code_id: r.get(4)?,
                coder_id: r.get(5)?,
                layer: r.get(6)?,
                rating: r.get(7)?,
                unit_id: r.get(8)?,
                day: r.get::<_, Option<String>>(9)?.and_then(|d| crate::calendar::parse_day(&d)),
                shape: None,
            })
        })?;
        let sources: Option<HashSet<i64>> = scope.source_ids.as_ref().map(|v| v.iter().copied().collect());
        let coders: Option<HashSet<i64>> = scope.coder_ids.as_ref().map(|v| v.iter().copied().collect());
        let wanted_cases: Option<HashSet<i64>> = scope.case_ids.as_ref().map(|v| v.iter().copied().collect());
        // The final layer (decision 173) stands in for the codings: the decisions, and what the coders agreed on. It has no areas of pictures.
        let final_layer = scope.layer.as_deref() == Some("final");
        let mut all: Vec<Rec> = if final_layer {
            self.final_layer()?
                .rows
                .into_iter()
                .map(|r| Rec {
                    id: r.id,
                    source_id: r.source_id,
                    start: r.start,
                    end: r.end,
                    code_id: r.code_id,
                    coder_id: r.coder_id,
                    layer: "final".to_string(),
                    rating: r.rating,
                    unit_id: Some(r.unit_id),
                    day: crate::calendar::parse_day(r.created_at.get(..10).unwrap_or("")),
                    shape: None,
                })
                .collect()
        } else {
            rows.collect::<std::result::Result<_, _>>()?
        };
        // The coded areas of pictures join the codings of text. Each distinct shape on a picture gets one pair of negative numbers.
        let mut shape_keys: HashMap<(i64, i64, String), i64> = HashMap::new();
        let final_regions: HashSet<i64> = if final_layer { self.final_region_ids()?.0 } else { HashSet::new() };
        for g in self.all_regions()?.into_iter().filter(|g| !final_layer || final_regions.contains(&g.id)) {
            let next = -(shape_keys.len() as i64) - 1;
            let key = *shape_keys.entry((g.source_id, g.page, g.shape.canonical_json())).or_insert(next);
            all.push(Rec {
                id: -g.id,
                source_id: g.source_id,
                start: key,
                end: key + 1,
                code_id: g.code_id,
                coder_id: g.coder_id,
                layer: if final_layer { "final".to_string() } else { g.layer.clone() },
                rating: g.rating,
                unit_id: None,
                day: crate::calendar::parse_day(g.created_at.get(..10).unwrap_or("")),
                shape: Some(Arc::new(g.shape)),
            });
        }
        let mut out = Vec::new();
        for r in all {
            if sources.as_ref().is_some_and(|s| !s.contains(&r.source_id)) {
                continue;
            }
            if coders.as_ref().is_some_and(|c| !c.contains(&r.coder_id)) {
                continue;
            }
            if scope.layer.as_ref().is_some_and(|l| *l != r.layer) {
                continue;
            }
            if let Some(w) = &wanted_cases {
                if !cases.cases_of(&r).iter().any(|c| w.contains(c)) {
                    continue;
                }
            }
            out.push(r);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------- counts

/// What a cell holds: the four honest ways of counting the same evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub assignments: i64,
    pub excerpts: i64,
    pub documents: i64,
    pub cases: i64,
}

#[derive(Default)]
struct Acc {
    codings: HashSet<i64>,
    excerpts: HashSet<(i64, i64, i64)>,
    documents: HashSet<i64>,
    cases: HashSet<i64>,
}

impl Acc {
    fn add(&mut self, r: &Rec, cases: &[i64]) {
        self.codings.insert(r.id);
        self.excerpts.insert((r.source_id, r.start, r.end));
        self.documents.insert(r.source_id);
        for c in cases {
            self.cases.insert(*c);
        }
    }
    fn counts(&self) -> Counts {
        Counts { assignments: self.codings.len() as i64, excerpts: self.excerpts.len() as i64, documents: self.documents.len() as i64, cases: self.cases.len() as i64 }
    }
}

// ---------------------------------------------------------------- the code matrix

/// What the rows of a matrix are.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RowSpec {
    /// One row for each case.
    Cases,
    /// One row for each document.
    Documents,
    /// One row for each value of a case variable ("Participant group"); N is how many cases have the value.
    CaseVariable { name: String },
    /// One row for each month codings were made in (from the first to the last, a month with none is a row of zeros), by the
    /// date part of the coding's time stamp in UTC.
    MonthCoded,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatrixSpec {
    pub rows: RowSpec,
    /// The codes that are the columns, in order.
    pub code_ids: Vec<i64>,
    /// A column also counts the codings of the code's sub-codes (the default).
    #[serde(default = "yes")]
    pub include_descendants: bool,
    #[serde(default)]
    pub scope: Scope,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatrixColumn {
    pub code_id: i64,
    pub name: String,
    /// Over all rows, each coding counted once.
    pub total: Counts,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatrixRow {
    /// Stable name of the row for asking for its evidence: `case:12`, `source:3`, `value:Group A`, or `value:` for no value.
    pub key: String,
    pub label: String,
    /// How many cases the row stands for (for a group of cases, N), when that makes sense.
    pub size: Option<i64>,
    pub cells: Vec<Counts>,
    /// Over all the columns, each coding counted once (so it can be less than the cells added up, when columns share codings).
    pub total: Counts,
}

#[derive(Debug, Clone, Serialize)]
pub struct Matrix {
    pub rows: Vec<MatrixRow>,
    pub columns: Vec<MatrixColumn>,
    /// Over everything shown, each coding counted once.
    pub total: Counts,
    /// How many codings and documents were looked at before choosing columns: the universe of the counts.
    pub universe: Counts,
    /// The counting rules in words, to show with the numbers.
    pub notes: Vec<String>,
}

/// Every month from the first to the last of a sorted list of `YYYY-MM` months, so a month with nothing in it is still a row.
/// A list that spans more than fifty years is not filled in.
fn month_range(months: &[String]) -> Vec<String> {
    let parse = |m: &str| -> Option<(i64, i64)> {
        let (y, mo) = m.split_once('-')?;
        Some((y.parse().ok()?, mo.parse().ok()?))
    };
    let (Some(first), Some(last)) = (months.first().and_then(|m| parse(m)), months.last().and_then(|m| parse(m))) else {
        return Vec::new();
    };
    if (last.0 * 12 + last.1) - (first.0 * 12 + first.1) > 600 {
        return months.to_vec();
    }
    let mut out = Vec::new();
    let (mut y, mut m) = first;
    while (y, m) <= last {
        out.push(format!("{y:04}-{m:02}"));
        m += 1;
        if m > 12 {
            m = 1;
            y += 1;
        }
    }
    out
}

/// The rows a coding belongs to: each row's key and label, and the cases that row counts for this coding.
type RowKeys = Vec<(String, String, Vec<i64>)>;

/// Works out which rows a coding falls in, from the coding and the cases it belongs to.
type RowKeysFn = Box<dyn Fn(&Rec, &[i64]) -> RowKeys>;

struct RowPlan {
    /// Every row, in order, even those with no evidence: (key, label, size).
    rows: Vec<(String, String, Option<i64>)>,
    keys: RowKeysFn,
}

/// A coding with the rows and the matrix columns it falls in.
type Hit = (Rec, RowKeys, Vec<usize>);

/// How many codings, and the distinct stretches they cover.
type Tally = (i64, HashSet<(i64, i64, i64)>);

impl Project {
    fn plan_rows(&self, spec: &RowSpec, cases: &CaseMap) -> Result<RowPlan> {
        match spec {
            RowSpec::Cases => {
                let mut rows: Vec<(String, String, Option<i64>)> = cases.names.iter().map(|(id, n)| (format!("case:{id}"), n.clone(), None)).collect();
                rows.sort_by(|a, b| natural(&a.1).cmp(&natural(&b.1)).then(a.0.cmp(&b.0)));
                let names = cases.names.clone();
                // Documents that are in no case are a row of their own, shown only when something is in it.
                rows.push(("case:".to_string(), "Not in any case".to_string(), None));
                Ok(RowPlan {
                    rows,
                    keys: Box::new(move |_, cs| {
                        if cs.is_empty() {
                            vec![("case:".to_string(), "Not in any case".to_string(), Vec::new())]
                        } else {
                            cs.iter().map(|c| (format!("case:{c}"), names.get(c).cloned().unwrap_or_default(), vec![*c])).collect()
                        }
                    }),
                })
            }
            RowSpec::Documents => {
                let mut stmt = self.conn.prepare("SELECT id, name FROM source ORDER BY name, id")?;
                let rows: Vec<(String, String, Option<i64>)> =
                    stmt.query_map([], |r| Ok((format!("source:{}", r.get::<_, i64>(0)?), r.get::<_, String>(1)?, None)))?.collect::<std::result::Result<_, _>>()?;
                let names: HashMap<i64, String> = rows.iter().map(|(k, n, _)| (k[7..].parse().unwrap_or(0), n.clone())).collect();
                Ok(RowPlan { rows, keys: Box::new(move |r, cs| vec![(format!("source:{}", r.source_id), names.get(&r.source_id).cloned().unwrap_or_default(), cs.to_vec())]) })
            }
            RowSpec::CaseVariable { name } => {
                let attr: Option<i64> =
                    self.conn.query_row("SELECT id FROM attribute WHERE applies_to = 'case' AND name = ?1", [name], |r| r.get(0)).map(Some).or_else(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => Ok(None),
                        other => Err(other),
                    })?;
                let Some(attr) = attr else {
                    return Err(Error::Invalid(format!("no case variable is named '{name}'")));
                };
                // The value of the variable for every case; a case with none is in the group "(no value)".
                let mut value_of: HashMap<i64, String> = HashMap::new();
                let mut stmt = self.conn.prepare("SELECT target_id, value FROM attribute_value WHERE attribute_id = ?1")?;
                for row in stmt.query_map([attr], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))? {
                    let (case, v) = row?;
                    if let Some(v) = v.filter(|v| !v.trim().is_empty()) {
                        value_of.insert(case, v.trim().to_string());
                    }
                }
                let mut sizes: BTreeMap<String, i64> = BTreeMap::new();
                for case in cases.names.keys() {
                    *sizes.entry(value_of.get(case).cloned().unwrap_or_default()).or_default() += 1;
                }
                let mut values: Vec<(String, i64)> = sizes.into_iter().collect();
                values.sort_by_key(|a| (a.0.is_empty(), natural(&a.0)));
                let rows = values
                    .iter()
                    .map(|(v, n)| (format!("value:{v}"), if v.is_empty() { "(no value)".to_string() } else { v.clone() }, Some(*n)))
                    .collect();
                Ok(RowPlan {
                    rows,
                    keys: Box::new(move |_, cs| {
                        // The cases of this coding that have each value; a coding in no case is in (no value).
                        let mut groups: Vec<(String, Vec<i64>)> = Vec::new();
                        if cs.is_empty() {
                            groups.push((String::new(), Vec::new()));
                        }
                        for c in cs {
                            let v = value_of.get(c).cloned().unwrap_or_default();
                            match groups.iter_mut().find(|(g, _)| *g == v) {
                                Some((_, list)) => list.push(*c),
                                None => groups.push((v, vec![*c])),
                            }
                        }
                        groups.into_iter().map(|(v, list)| (format!("value:{v}"), if v.is_empty() { "(no value)".to_string() } else { v }, list)).collect()
                    }),
                })
            }
            RowSpec::MonthCoded => {
                let mut stmt = self.conn.prepare("SELECT DISTINCT substr(created_at, 1, 10) FROM coding")?;
                let mut months: Vec<String> = Vec::new();
                for row in stmt.query_map([], |r| r.get::<_, Option<String>>(0))? {
                    if let Some(day) = row?.and_then(|d| crate::calendar::parse_day(&d)) {
                        months.push(crate::calendar::month_text(day));
                    }
                }
                months.sort();
                months.dedup();
                let mut rows: Vec<(String, String, Option<i64>)> = month_range(&months).into_iter().map(|m| (format!("month:{m}"), m, None)).collect();
                rows.push(("month:".to_string(), "(no date)".to_string(), None));
                Ok(RowPlan {
                    rows,
                    keys: Box::new(|r, cs| match r.day {
                        Some(d) => {
                            let m = crate::calendar::month_text(d);
                            vec![(format!("month:{m}"), m, cs.to_vec())]
                        }
                        None => vec![("month:".to_string(), "(no date)".to_string(), cs.to_vec())],
                    }),
                })
            }
        }
    }

    /// Every coding that falls in a column, with the rows and columns it falls in.
    fn matrix_hits(&self, spec: &MatrixSpec) -> Result<(Vec<Hit>, RowPlan, Counts)> {
        let hier = self.hierarchy()?;
        let cases = self.case_map()?;
        let plan = self.plan_rows(&spec.rows, &cases)?;
        let all = self.scoped_codings(&spec.scope, &cases)?;
        let universe = {
            let mut acc = Acc::default();
            for r in &all {
                acc.add(r, &cases.cases_of(r));
            }
            acc.counts()
        };
        let mut hits = Vec::new();
        for r in all {
            let columns: Vec<usize> = spec.code_ids.iter().enumerate().filter(|(_, c)| hier.is_within(r.code_id, **c, spec.include_descendants)).map(|(i, _)| i).collect();
            if columns.is_empty() {
                continue;
            }
            let cs = cases.cases_of(&r);
            let keys = (plan.keys)(&r, &cs);
            hits.push((r, keys, columns));
        }
        Ok((hits, plan, universe))
    }

    /// A matrix of codes against cases, documents, or the values of a case variable.
    pub fn code_matrix(&self, spec: &MatrixSpec) -> Result<Matrix> {
        let hier = self.hierarchy()?;
        for c in &spec.code_ids {
            if !hier.names.contains_key(c) {
                return Err(Error::NotFound { what: "code", id: *c });
            }
        }
        let cases = self.case_map()?;
        let (hits, plan, universe) = self.matrix_hits(spec)?;
        let n = spec.code_ids.len();
        let mut cell: HashMap<(String, usize), Acc> = HashMap::new();
        let mut row_total: HashMap<String, Acc> = HashMap::new();
        let mut col_total: Vec<Acc> = (0..n).map(|_| Acc::default()).collect();
        let mut grand = Acc::default();
        for (r, keys, columns) in &hits {
            let cs = cases.cases_of(r);
            for (key, _, row_cases) in keys {
                for &c in columns {
                    cell.entry((key.clone(), c)).or_default().add(r, row_cases);
                }
                row_total.entry(key.clone()).or_default().add(r, row_cases);
            }
            for &c in columns {
                col_total[c].add(r, &cs);
            }
            grand.add(r, &cs);
        }
        let rows = plan
            .rows
            .iter()
            .map(|(key, label, size)| MatrixRow {
                key: key.clone(),
                label: label.clone(),
                size: *size,
                cells: (0..n).map(|c| cell.get(&(key.clone(), c)).map(Acc::counts).unwrap_or_default()).collect(),
                total: row_total.get(key).map(Acc::counts).unwrap_or_default(),
            })
            // "Not in any case" and "(no date)" are only rows when something is in them.
            .filter(|row| !((row.key == "case:" || row.key == "month:") && row.total == Counts::default()))
            .collect();
        let columns = spec
            .code_ids
            .iter()
            .enumerate()
            .map(|(i, id)| MatrixColumn { code_id: *id, name: hier.names.get(id).cloned().unwrap_or_default(), total: col_total[i].counts() })
            .collect();
        let mut notes = vec![
            "Assignments count codings. Excerpts count distinct stretches of text or areas of a picture, so two coders or two codes on the same words or the same area count once. Documents and cases count how many have at least one coding.".to_string(),
            if spec.include_descendants {
                "A column counts the codings of its code and of every sub-code. A parent's count looks at the codings themselves, so evidence two sub-codes share is counted once.".to_string()
            } else {
                "A column counts only the codings of its own code, not those of its sub-codes.".to_string()
            },
        ];
        notes.push(match &spec.rows {
            RowSpec::Cases => "A document that belongs to several cases is counted in each of them.".to_string(),
            RowSpec::Documents => "Each row is a document.".to_string(),
            RowSpec::CaseVariable { name } => format!("Each row is a value of {name}. A case with no value is in (no value), and N is how many cases have the value."),
            RowSpec::MonthCoded => "Each row is a month in which codings were made, by the date of each coding in UTC. A month with no coding is a row of zeros.".to_string(),
        });
        Ok(Matrix { rows, columns, total: grand.counts(), universe, notes })
    }

    /// The coding numbers behind a cell, a row (no `code_id`), a column (no `row_key`), or the whole matrix (neither).
    pub fn code_matrix_evidence(&self, spec: &MatrixSpec, row_key: Option<&str>, code_id: Option<i64>) -> Result<Vec<i64>> {
        let (hits, _, _) = self.matrix_hits(spec)?;
        let column = match code_id {
            Some(c) => Some(spec.code_ids.iter().position(|x| *x == c).ok_or(Error::NotFound { what: "code", id: c })?),
            None => None,
        };
        let mut ids: Vec<i64> = hits
            .iter()
            .filter(|(_, keys, cols)| row_key.is_none_or(|k| keys.iter().any(|(key, _, _)| key == k)) && column.is_none_or(|c| cols.contains(&c)))
            .map(|(r, _, _)| r.id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }
}

/// A text key that sorts "Group 2" before "Group 10".
pub(crate) fn natural(s: &str) -> Vec<(bool, String, u64)> {
    let mut out = Vec::new();
    let mut digits = String::new();
    let mut text = String::new();
    let flush_text = |text: &mut String, out: &mut Vec<(bool, String, u64)>| {
        if !text.is_empty() {
            out.push((false, text.to_lowercase(), 0));
            text.clear();
        }
    };
    for c in s.chars() {
        if c.is_ascii_digit() {
            flush_text(&mut text, &mut out);
            digits.push(c);
        } else {
            if !digits.is_empty() {
                out.push((true, String::new(), digits.parse().unwrap_or(0)));
                digits.clear();
            }
            text.push(c);
        }
    }
    flush_text(&mut text, &mut out);
    if !digits.is_empty() {
        out.push((true, String::new(), digits.parse().unwrap_or(0)));
    }
    out
}

// ---------------------------------------------------------------- how codes occur together

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationMode {
    /// A cell counts pairs of codings, one of each code, whose words overlap.
    Overlap,
    /// A cell counts the documents that have both codes.
    Document,
    /// A cell counts the cases that have both codes.
    Case,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelationsSpec {
    /// The codes to compare; empty means every active code that has a coding.
    #[serde(default)]
    pub code_ids: Vec<i64>,
    #[serde(default = "yes")]
    pub include_descendants: bool,
    pub mode: RelationMode,
    /// For `overlap`: both codings must be by the same coder (the default), as one person applying two codes to the same words.
    #[serde(default = "yes")]
    pub same_coder: bool,
    #[serde(default)]
    pub scope: Scope,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationCode {
    pub code_id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Relations {
    pub codes: Vec<RelationCode>,
    /// `cells[i][j]` is how often codes `i` and `j` occur together; on the diagonal, how often the code occurs at all
    /// (codings for `overlap`, documents for `document`, cases for `case`).
    pub cells: Vec<Vec<i64>>,
    pub notes: Vec<String>,
}

impl Project {
    fn relation_codes(&self, spec: &RelationsSpec, hier: &Hierarchy, recs: &[Rec]) -> Result<Vec<i64>> {
        if !spec.code_ids.is_empty() {
            for c in &spec.code_ids {
                if !hier.names.contains_key(c) {
                    return Err(Error::NotFound { what: "code", id: *c });
                }
            }
            return Ok(spec.code_ids.clone());
        }
        // Every active code that has a coding (or, when sub-codes count, whose sub-codes do).
        let mut included: HashSet<i64> = HashSet::new();
        for code in recs.iter().map(|r| r.code_id).collect::<HashSet<i64>>() {
            if spec.include_descendants {
                if let Some(chain) = hier.ancestors.get(&code) {
                    included.extend(chain.iter().copied());
                }
            } else {
                included.insert(code);
            }
        }
        let mut stmt = self.conn.prepare("SELECT id FROM code WHERE status = 'active' ORDER BY sort_order, name, id")?;
        let all: Vec<i64> = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(all.into_iter().filter(|id| included.contains(id)).collect())
    }

    /// Pairs of codings, one in code `i` and one in code `j`, that overlap. Returned as (index i, index j, first coding, second coding).
    fn overlap_pairs(&self, spec: &RelationsSpec, codes: &[i64], hier: &Hierarchy, recs: &[Rec]) -> Vec<(usize, usize, i64, i64)> {
        let member = |r: &Rec| -> Vec<usize> { codes.iter().enumerate().filter(|(_, c)| hier.is_within(r.code_id, **c, spec.include_descendants)).map(|(i, _)| i).collect() };
        // A document can hold codings of words and coded areas or stretches (a PDF, or a transcript with a recording), and the two never
        // overlap, so each document's codings are taken in two lists.
        type Coded<'a> = Vec<(&'a Rec, Vec<usize>)>;
        let mut by_source: BTreeMap<(i64, bool), Coded> = BTreeMap::new();
        for r in recs {
            let m = member(r);
            if !m.is_empty() {
                by_source.entry((r.source_id, r.shape.is_some())).or_default().push((r, m));
            }
        }
        let mut out = Vec::new();
        for ((_, areas), list) in &by_source {
            let areas = *areas;
            // `recs` is in text order, so a coding can only overlap those after it that start before it ends. A picture's areas are
            // not in an order like that, so each is compared with the others by shape.
            for (a, (ra, ma)) in list.iter().enumerate() {
                for (rb, mb) in list.iter().skip(a + 1) {
                    if areas {
                        match (&ra.shape, &rb.shape) {
                            (Some(x), Some(y)) if overlaps(x, y) => {}
                            _ => continue,
                        }
                    } else if rb.start >= ra.end {
                        break;
                    }
                    if spec.same_coder && (ra.coder_id != rb.coder_id || ra.layer != rb.layer) {
                        continue;
                    }
                    for &i in ma {
                        for &j in mb {
                            if i != j {
                                out.push((i, j, ra.id, rb.id));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// How codes occur together: pairs of overlapping codings, or documents or cases that have both.
    pub fn code_relations(&self, spec: &RelationsSpec) -> Result<Relations> {
        let hier = self.hierarchy()?;
        let cases = self.case_map()?;
        let recs = self.scoped_codings(&spec.scope, &cases)?;
        let ids = self.relation_codes(spec, &hier, &recs)?;
        let n = ids.len();
        let mut cells = vec![vec![0i64; n]; n];
        let codes: Vec<RelationCode> = ids.iter().map(|id| RelationCode { code_id: *id, name: hier.names.get(id).cloned().unwrap_or_default() }).collect();
        match spec.mode {
            RelationMode::Overlap => {
                for r in &recs {
                    for (i, c) in ids.iter().enumerate() {
                        if hier.is_within(r.code_id, *c, spec.include_descendants) {
                            cells[i][i] += 1;
                        }
                    }
                }
                for (i, j, _, _) in self.overlap_pairs(spec, &ids, &hier, &recs) {
                    cells[i][j] += 1;
                    cells[j][i] += 1;
                }
            }
            RelationMode::Document | RelationMode::Case => {
                // For each code, the set of documents (or cases) that have it.
                let mut having: Vec<HashSet<i64>> = vec![HashSet::new(); n];
                for r in &recs {
                    let unit: Vec<i64> = if matches!(spec.mode, RelationMode::Document) { vec![r.source_id] } else { cases.cases_of(r) };
                    for (i, c) in ids.iter().enumerate() {
                        if hier.is_within(r.code_id, *c, spec.include_descendants) {
                            having[i].extend(unit.iter().copied());
                        }
                    }
                }
                for i in 0..n {
                    for j in 0..n {
                        cells[i][j] = having[i].intersection(&having[j]).count() as i64;
                    }
                }
            }
        }
        let mut notes = Vec::new();
        notes.push(match spec.mode {
            RelationMode::Overlap if spec.same_coder => "A cell counts pairs of codings, one of each code, whose words or picture areas overlap and that were made by the same coder. The diagonal counts the codings of the code.".to_string(),
            RelationMode::Overlap => "A cell counts pairs of codings, one of each code, whose words or picture areas overlap, whoever made them. The diagonal counts the codings of the code.".to_string(),
            RelationMode::Document => "A cell counts the documents that have a coding of both codes, wherever in the document. The diagonal counts the documents that have the code.".to_string(),
            RelationMode::Case => "A cell counts the cases that have a coding of both codes. The diagonal counts the cases that have the code.".to_string(),
        });
        notes.push(if spec.include_descendants { "A code also counts the codings of its sub-codes.".to_string() } else { "A code counts only its own codings.".to_string() });
        Ok(Relations { codes, cells, notes })
    }

    /// The codings behind a cell of `code_relations`: those of both codes that occur together.
    pub fn code_relations_evidence(&self, spec: &RelationsSpec, a: i64, b: i64) -> Result<Vec<i64>> {
        let hier = self.hierarchy()?;
        let cases = self.case_map()?;
        let recs = self.scoped_codings(&spec.scope, &cases)?;
        let ids = vec![a, b];
        let mut out: HashSet<i64> = HashSet::new();
        match spec.mode {
            RelationMode::Overlap => {
                if a == b {
                    out.extend(recs.iter().filter(|r| hier.is_within(r.code_id, a, spec.include_descendants)).map(|r| r.id));
                } else {
                    for (_, _, x, y) in self.overlap_pairs(spec, &ids, &hier, &recs) {
                        out.insert(x);
                        out.insert(y);
                    }
                }
            }
            RelationMode::Document | RelationMode::Case => {
                let unit = |r: &Rec| -> Vec<i64> { if matches!(spec.mode, RelationMode::Document) { vec![r.source_id] } else { cases.cases_of(r) } };
                let mut sets: [HashSet<i64>; 2] = [HashSet::new(), HashSet::new()];
                for r in &recs {
                    for (k, c) in ids.iter().enumerate() {
                        if hier.is_within(r.code_id, *c, spec.include_descendants) {
                            sets[k].extend(unit(r));
                        }
                    }
                }
                let both: HashSet<i64> = sets[0].intersection(&sets[1]).copied().collect();
                for r in &recs {
                    let in_scope = unit(r).iter().any(|u| both.contains(u));
                    if in_scope && ids.iter().any(|c| hier.is_within(r.code_id, *c, spec.include_descendants)) {
                        out.insert(r.id);
                    }
                }
            }
        }
        let mut v: Vec<i64> = out.into_iter().collect();
        v.sort_unstable();
        Ok(v)
    }
}

// ---------------------------------------------------------------- a summary of each code

#[derive(Debug, Clone, Serialize)]
pub struct CodeSummary {
    pub code_id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub status: String,
    /// Codings of this code alone.
    pub own_assignments: i64,
    /// Codings of this code and its sub-codes, each once.
    pub assignments: i64,
    pub excerpts: i64,
    pub documents: i64,
    pub cases: i64,
    pub coders: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CountedName {
    pub id: i64,
    pub name: String,
    pub assignments: i64,
    pub excerpts: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeProfile {
    pub summary: CodeSummary,
    pub by_document: Vec<CountedName>,
    pub by_case: Vec<CountedName>,
    pub by_coder: Vec<CountedName>,
    /// The codes whose codings overlap this code's (the same coder), with how many pairs.
    pub together_with: Vec<CountedName>,
    pub coding_ids: Vec<i64>,
}

impl Project {
    /// One line for each code: how often it was used, in how many documents and cases, by how many coders.
    pub fn code_summaries(&self, scope: &Scope) -> Result<Vec<CodeSummary>> {
        let hier = self.hierarchy()?;
        let cases = self.case_map()?;
        let recs = self.scoped_codings(scope, &cases)?;
        let mut acc: HashMap<i64, (Acc, HashSet<i64>, i64)> = HashMap::new();
        for r in &recs {
            let cs = cases.cases_of(r);
            let chain = hier.ancestors.get(&r.code_id).cloned().unwrap_or_else(|| vec![r.code_id]);
            for c in chain {
                let e = acc.entry(c).or_default();
                e.0.add(r, &cs);
                e.1.insert(r.coder_id);
                if c == r.code_id {
                    e.2 += 1;
                }
            }
        }
        let mut stmt = self.conn.prepare("SELECT id, name, parent_id, status FROM code ORDER BY sort_order, name, id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, String>(3)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, parent_id, status) = row?;
            let (a, coders, own) = acc.remove(&id).map(|(a, c, o)| (a.counts(), c.len() as i64, o)).unwrap_or_default();
            out.push(CodeSummary { code_id: id, name, parent_id, status, own_assignments: own, assignments: a.assignments, excerpts: a.excerpts, documents: a.documents, cases: a.cases, coders });
        }
        Ok(out)
    }

    /// Everything about one code: where it was used, by whom, and what it occurs with.
    pub fn code_profile(&self, code_id: i64, include_descendants: bool, scope: &Scope) -> Result<CodeProfile> {
        let hier = self.hierarchy()?;
        if !hier.names.contains_key(&code_id) {
            return Err(Error::NotFound { what: "code", id: code_id });
        }
        let cases = self.case_map()?;
        let recs = self.scoped_codings(scope, &cases)?;
        let summary = self.code_summaries(scope)?.into_iter().find(|s| s.code_id == code_id).ok_or(Error::NotFound { what: "code", id: code_id })?;
        let mine: Vec<&Rec> = recs.iter().filter(|r| hier.is_within(r.code_id, code_id, include_descendants)).collect();

        let mut sources: HashMap<i64, String> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT id, name FROM source")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, n) = row?;
            sources.insert(id, n);
        }
        let mut coders: HashMap<i64, String> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT id, name FROM coder")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, n) = row?;
            coders.insert(id, n);
        }

        let counted = |groups: HashMap<i64, Tally>, names: &HashMap<i64, String>| -> Vec<CountedName> {
            let mut v: Vec<CountedName> =
                groups.into_iter().map(|(id, (n, ex))| CountedName { id, name: names.get(&id).cloned().unwrap_or_default(), assignments: n, excerpts: ex.len() as i64 }).collect();
            v.sort_by(|a, b| b.assignments.cmp(&a.assignments).then(natural(&a.name).cmp(&natural(&b.name))));
            v
        };
        let mut by_doc: HashMap<i64, Tally> = HashMap::new();
        let mut by_case: HashMap<i64, Tally> = HashMap::new();
        let mut by_coder: HashMap<i64, Tally> = HashMap::new();
        for r in &mine {
            let span = (r.source_id, r.start, r.end);
            let e = by_doc.entry(r.source_id).or_default();
            e.0 += 1;
            e.1.insert(span);
            let e = by_coder.entry(r.coder_id).or_default();
            e.0 += 1;
            e.1.insert(span);
            for c in cases.cases_of(r) {
                let e = by_case.entry(c).or_default();
                e.0 += 1;
                e.1.insert(span);
            }
        }

        // The codes that overlap this one, counting pairs by the same coder.
        let mut together: HashMap<i64, Tally> = HashMap::new();
        let mut per_source: BTreeMap<(i64, bool), Vec<&Rec>> = BTreeMap::new();
        for r in &recs {
            per_source.entry((r.source_id, r.shape.is_some())).or_default().push(r);
        }
        for ((_, areas), list) in &per_source {
            let areas = *areas;
            for (a, ra) in list.iter().enumerate() {
                for rb in list.iter().skip(a + 1) {
                    if areas {
                        match (&ra.shape, &rb.shape) {
                            (Some(x), Some(y)) if overlaps(x, y) => {}
                            _ => continue,
                        }
                    } else if rb.start >= ra.end {
                        break;
                    }
                    if ra.coder_id != rb.coder_id || ra.layer != rb.layer {
                        continue;
                    }
                    for (x, y) in [(ra, rb), (rb, ra)] {
                        if hier.is_within(x.code_id, code_id, include_descendants) && !hier.is_within(y.code_id, code_id, include_descendants) {
                            let e = together.entry(y.code_id).or_default();
                            e.0 += 1;
                            e.1.insert((y.source_id, y.start.min(x.start), y.end.max(x.end)));
                        }
                    }
                }
            }
        }
        let mut together_with = counted(together, &hier.names);
        together_with.truncate(10);

        Ok(CodeProfile {
            summary,
            by_document: counted(by_doc, &sources),
            by_case: counted(by_case, &cases.names),
            by_coder: counted(by_coder, &coders),
            together_with,
            coding_ids: mine.iter().map(|r| r.id).collect(),
        })
    }
}

// ---------------------------------------------------------------- queries

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Combine {
    /// At least one of the items matches.
    Any,
    /// Every item matches.
    All,
}

/// One clause of a query, or a group of them. Each clause is judged for one coding at a time; `negate` turns it round.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    Group {
        #[serde(rename = "match")]
        combine: Combine,
        items: Vec<Node>,
        #[serde(default)]
        negate: bool,
    },
    /// The coding's code is one of these (and, when asked, a sub-code of one).
    Code {
        code_ids: Vec<i64>,
        #[serde(default = "yes")]
        include_descendants: bool,
        #[serde(default)]
        negate: bool,
    },
    /// The words are also coded with one of these codes: overlapping them (`within_chars` 0), or within this many characters of them.
    AlsoCoded {
        code_ids: Vec<i64>,
        #[serde(default)]
        within_chars: i64,
        #[serde(default = "yes")]
        include_descendants: bool,
        #[serde(default = "yes")]
        same_coder: bool,
        #[serde(default)]
        negate: bool,
    },
    /// The words contain this text (capitals do not matter).
    Text {
        contains: String,
        #[serde(default)]
        negate: bool,
    },
    Source {
        ids: Vec<i64>,
        #[serde(default)]
        negate: bool,
    },
    /// The document (or unit) belongs to one of these cases.
    Case {
        ids: Vec<i64>,
        #[serde(default)]
        negate: bool,
    },
    /// A case the coding belongs to has this value of a case variable.
    CaseVariable {
        name: String,
        value: String,
        #[serde(default)]
        negate: bool,
    },
    Coder {
        ids: Vec<i64>,
        #[serde(default)]
        negate: bool,
    },
    /// The coding's rating is within these limits (a coding that has no rating never matches).
    Rating {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
        #[serde(default)]
        negate: bool,
    },
    /// The coding was made on or between these days (written year-month-day; the day is the date part of the time stamp,
    /// in UTC). Either limit may be left out.
    CodedBetween {
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        to: Option<String>,
        #[serde(default)]
        negate: bool,
    },
    /// A case the coding belongs to has a value of this case variable between these limits: dates (both limits written
    /// year-month-day) or numbers (both limits numbers). A value that cannot be read that way never matches.
    CaseVariableBetween {
        name: String,
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        to: Option<String>,
        #[serde(default)]
        negate: bool,
    },
    /// The coding is one of these (given in ascending order). Used to look again at what a frozen query found.
    CodingIds {
        ids: Vec<i64>,
        #[serde(default)]
        negate: bool,
    },
}

/// What kind of limits a range clause has.
enum Limits {
    Days(Option<i64>, Option<i64>),
    Numbers(Option<f64>, Option<f64>),
}

/// Reads the limits of a range clause: both dates, or both numbers. `None` when they are neither, or mixed.
fn read_limits(from: &Option<String>, to: &Option<String>) -> Option<Limits> {
    let bounds: Vec<&str> = [from, to].iter().filter_map(|b| b.as_deref()).map(str::trim).filter(|b| !b.is_empty()).collect();
    if bounds.is_empty() {
        return None;
    }
    let day = |b: &Option<String>| b.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(crate::calendar::parse_day);
    let number = |b: &Option<String>| b.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(crate::calendar::parse_number);
    if bounds.iter().all(|b| crate::calendar::parse_day(b).is_some()) {
        return Some(Limits::Days(day(from).flatten(), day(to).flatten()));
    }
    if bounds.iter().all(|b| crate::calendar::parse_number(b).is_some()) {
        return Some(Limits::Numbers(number(from).flatten(), number(to).flatten()));
    }
    None
}

fn above<T: PartialOrd + Copy>(bound: Option<T>, v: T) -> bool {
    bound.is_none_or(|b| v >= b)
}

fn below<T: PartialOrd + Copy>(bound: Option<T>, v: T) -> bool {
    bound.is_none_or(|b| v <= b)
}

/// Whether a case variable's value is inside the limits.
fn value_within(value: &str, limits: &Limits) -> bool {
    match limits {
        Limits::Days(a, b) => crate::calendar::parse_day(value).is_some_and(|d| above(*a, d) && below(*b, d)),
        Limits::Numbers(a, b) => crate::calendar::parse_number(value).is_some_and(|v| above(*a, v) && below(*b, v)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResultUnit {
    /// Each coding that matches is a result.
    Assignments,
    /// Each distinct stretch of text is a result, once, with every coding of it that matched.
    #[default]
    Excerpts,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuerySpec {
    pub rule: Node,
    #[serde(default)]
    pub result_unit: ResultUnit,
    #[serde(default)]
    pub scope: Scope,
    /// The most results to return (the total is always counted).
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    500
}

/// One coding that matched, inside a result.
#[derive(Debug, Clone, Serialize)]
pub struct QueryCoding {
    pub coding_id: i64,
    pub code_id: i64,
    pub code_name: String,
    pub coder_name: String,
    pub rating: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryItem {
    /// `text`, `image` for an area of a picture (then `region` is the area and `text` tells it in words), or `clip` for a stretch of a
    /// recording (then `region` is the stretch).
    pub kind: String,
    pub region: Option<Shape>,
    /// The page of the PDF the area is on, for an area of a PDF page; `null` otherwise.
    pub page: Option<i64>,
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    pub text: String,
    /// The codings of these words that matched: one when the result unit is assignments, one or more for excerpts.
    pub codings: Vec<QueryCoding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub items: Vec<QueryItem>,
    /// How many results there are in all (the list above may be cut at `limit`).
    pub total: usize,
    /// How many codings matched in all.
    pub assignments: usize,
    pub truncated: bool,
    /// What was looked at before the rule: how many codings, in how many documents.
    pub universe: Counts,
}

/// What a query needs to look at, loaded once.
struct QueryEnv<'a> {
    hier: &'a Hierarchy,
    cases: &'a CaseMap,
    recs: &'a [Rec],
    by_source: HashMap<i64, Vec<usize>>,
    case_values: HashMap<(String, i64), String>,
    texts: HashMap<i64, Vec<char>>,
}

impl QueryEnv<'_> {
    fn matches(&self, node: &Node, i: usize) -> bool {
        let r = &self.recs[i];
        match node {
            Node::Group { combine, items, negate } => {
                let hit = match combine {
                    Combine::Any => items.iter().any(|n| self.matches(n, i)),
                    Combine::All => items.iter().all(|n| self.matches(n, i)),
                };
                hit != *negate
            }
            Node::Code { code_ids, include_descendants, negate } => code_ids.iter().any(|c| self.hier.is_within(r.code_id, *c, *include_descendants)) != *negate,
            Node::AlsoCoded { code_ids, within_chars, include_descendants, same_coder, negate } => {
                let within = (*within_chars).max(0);
                let hit = self.by_source.get(&r.source_id).is_some_and(|list| {
                    list.iter().any(|&j| {
                        let o = &self.recs[j];
                        if o.id == r.id || (*same_coder && (o.coder_id != r.coder_id || o.layer != r.layer)) {
                            return false;
                        }
                        // The gap between the two stretches; negative when they overlap. Two areas of a picture are near when they
                        // overlap, whatever the distance asked for, because a distance in characters means nothing on a picture.
                        let near = match (&o.shape, &r.shape) {
                            (Some(a), Some(b)) => overlaps(a, b),
                            (None, None) => {
                                let gap = o.start.max(r.start) - o.end.min(r.end);
                                if within == 0 { gap < 0 } else { gap <= within }
                            }
                            _ => false,
                        };
                        near && code_ids.iter().any(|c| self.hier.is_within(o.code_id, *c, *include_descendants))
                    })
                });
                hit != *negate
            }
            Node::Text { contains, negate } => {
                let needle: Vec<char> = contains.to_lowercase().chars().collect();
                let hit = needle.is_empty()
                    || self.texts.get(&r.source_id).is_some_and(|t| {
                        // An area of a picture has no words of its own, so the picture's description is what is looked at.
                        let (s, e) = if r.shape.is_some() { (0, t.len()) } else { (r.start as usize, (r.end as usize).min(t.len())) };
                        let hay: Vec<char> = t[s.min(e)..e].iter().flat_map(|c| c.to_lowercase()).collect();
                        hay.windows(needle.len()).any(|w| w == needle.as_slice())
                    });
                hit != *negate
            }
            Node::Source { ids, negate } => ids.contains(&r.source_id) != *negate,
            Node::Case { ids, negate } => self.cases.cases_of(r).iter().any(|c| ids.contains(c)) != *negate,
            Node::CaseVariable { name, value, negate } => {
                let hit = self.cases.cases_of(r).iter().any(|c| self.case_values.get(&(name.clone(), *c)).is_some_and(|v| v == value));
                hit != *negate
            }
            Node::Coder { ids, negate } => ids.contains(&r.coder_id) != *negate,
            Node::Rating { min, max, negate } => {
                let hit = r.rating.is_some_and(|v| min.is_none_or(|m| v >= m) && max.is_none_or(|m| v <= m));
                hit != *negate
            }
            Node::CodedBetween { from, to, negate } => {
                let (a, b) = (from.as_deref().and_then(crate::calendar::parse_day), to.as_deref().and_then(crate::calendar::parse_day));
                let hit = r.day.is_some_and(|d| above(a, d) && below(b, d));
                hit != *negate
            }
            Node::CaseVariableBetween { name, from, to, negate } => {
                let hit = read_limits(from, to).is_some_and(|limits| {
                    self.cases.cases_of(r).iter().any(|c| self.case_values.get(&(name.clone(), *c)).is_some_and(|v| value_within(v, &limits)))
                });
                hit != *negate
            }
            Node::CodingIds { ids, negate } => ids.binary_search(&r.id).is_ok() != *negate,
        }
    }
}

fn uses_text(node: &Node) -> bool {
    match node {
        Node::Text { .. } => true,
        Node::Group { items, .. } => items.iter().any(uses_text),
        _ => false,
    }
}

fn uses_case_variable(node: &Node, out: &mut HashSet<String>) {
    match node {
        Node::CaseVariable { name, .. } | Node::CaseVariableBetween { name, .. } => {
            out.insert(name.clone());
        }
        Node::Group { items, .. } => items.iter().for_each(|n| uses_case_variable(n, out)),
        _ => {}
    }
}

/// A query that cannot be run says what to fix.
pub(crate) fn validate_spec(spec: &QuerySpec) -> Result<()> {
    check_rule(&spec.rule, 0)
}

/// A rule that cannot be run says what to fix.
fn check_rule(node: &Node, depth: usize) -> Result<()> {
    if depth > 6 {
        return Err(Error::Invalid("a query's groups are nested too deeply".to_string()));
    }
    let empty = |what: &str| Err(Error::Invalid(format!("a clause that looks for {what} has none chosen")));
    match node {
        Node::Group { items, .. } => {
            if items.is_empty() && depth == 0 {
                return Err(Error::Invalid("the query has no conditions yet".to_string()));
            }
            items.iter().try_for_each(|n| check_rule(n, depth + 1))
        }
        Node::Code { code_ids, .. } | Node::AlsoCoded { code_ids, .. } if code_ids.is_empty() => empty("a code"),
        Node::Text { contains, .. } if contains.trim().is_empty() => Err(Error::Invalid("a 'text contains' clause has no text to look for".to_string())),
        Node::Source { ids, .. } if ids.is_empty() => empty("a document"),
        Node::Case { ids, .. } if ids.is_empty() => empty("a case"),
        Node::Coder { ids, .. } if ids.is_empty() => empty("a coder"),
        Node::CaseVariable { name, .. } if name.trim().is_empty() => empty("a case variable"),
        Node::Rating { min: None, max: None, .. } => Err(Error::Invalid("a rating clause needs a lowest or a highest rating".to_string())),
        Node::CodedBetween { from, to, .. } => {
            fn given(b: &Option<String>) -> Option<&str> {
                b.as_deref().map(str::trim).filter(|t| !t.is_empty())
            }
            let (a, b) = (given(from), given(to));
            if a.is_none() && b.is_none() {
                return Err(Error::Invalid("a 'coded between' clause needs a first or a last day".to_string()));
            }
            let mut days = Vec::new();
            for d in [a, b].into_iter().flatten() {
                days.push(crate::calendar::parse_day(d).ok_or_else(|| Error::Invalid(format!("'{d}' is not a date; write dates year-month-day, such as 2024-03-14")))?);
            }
            if days.len() == 2 && days[0] > days[1] {
                return Err(Error::Invalid("the first day is after the last day".to_string()));
            }
            Ok(())
        }
        Node::CaseVariableBetween { name, from, to, .. } => {
            if name.trim().is_empty() {
                return empty("a case variable");
            }
            let some = |b: &Option<String>| b.as_deref().is_some_and(|t| !t.trim().is_empty());
            if !some(from) && !some(to) {
                return Err(Error::Invalid(format!("a clause about the values of {name} between limits needs a lowest or a highest")));
            }
            match read_limits(from, to) {
                Some(Limits::Days(Some(a), Some(b))) if a > b => Err(Error::Invalid("the first date is after the last date".to_string())),
                Some(Limits::Numbers(Some(a), Some(b))) if a > b => Err(Error::Invalid("the lowest number is above the highest".to_string())),
                Some(_) => Ok(()),
                None => Err(Error::Invalid("the limits must both be dates written year-month-day (2024-03-14) or both be numbers".to_string())),
            }
        }
        Node::CodingIds { ids, .. } => {
            if ids.windows(2).any(|w| w[0] >= w[1]) {
                return Err(Error::Invalid("the coding numbers must be in ascending order, each once".to_string()));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

impl Project {
    /// Works out which codings a query matches and how they group into results, without building the results.
    fn evaluate(&self, spec: &QuerySpec) -> Result<Evaluated> {
        check_rule(&spec.rule, 0)?;
        let hier = self.hierarchy()?;
        let cases = self.case_map()?;
        let recs = self.scoped_codings(&spec.scope, &cases)?;

        let universe = {
            let mut acc = Acc::default();
            for r in &recs {
                acc.add(r, &[]);
            }
            acc.counts()
        };

        let mut by_source: HashMap<i64, Vec<usize>> = HashMap::new();
        for (i, r) in recs.iter().enumerate() {
            by_source.entry(r.source_id).or_default().push(i);
        }
        let mut variables = HashSet::new();
        uses_case_variable(&spec.rule, &mut variables);
        let mut case_values: HashMap<(String, i64), String> = HashMap::new();
        for name in &variables {
            let mut stmt = self.conn.prepare(
                "SELECT av.target_id, av.value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id WHERE a.applies_to = 'case' AND a.name = ?1",
            )?;
            for row in stmt.query_map(params![name], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))? {
                let (case, v) = row?;
                if let Some(v) = v {
                    case_values.insert((name.clone(), case), v.trim().to_string());
                }
            }
        }
        let mut texts: HashMap<i64, Vec<char>> = HashMap::new();
        if uses_text(&spec.rule) {
            for source in by_source.keys() {
                texts.insert(*source, self.source_text(*source)?.chars().collect());
            }
        }
        let env = QueryEnv { hier: &hier, cases: &cases, recs: &recs, by_source, case_values, texts };
        let matched: Vec<usize> = (0..recs.len()).filter(|&i| env.matches(&spec.rule, i)).collect();

        // Group the matches into results.
        let mut groups: Vec<Vec<usize>> = Vec::new();
        match spec.result_unit {
            ResultUnit::Assignments => groups.extend(matched.iter().map(|&i| vec![i])),
            ResultUnit::Excerpts => {
                let mut index: HashMap<(i64, i64, i64), usize> = HashMap::new();
                for &i in &matched {
                    let r = &recs[i];
                    let key = (r.source_id, r.start, r.end);
                    match index.get(&key) {
                        Some(&g) => groups[g].push(i),
                        None => {
                            index.insert(key, groups.len());
                            groups.push(vec![i]);
                        }
                    }
                }
            }
        }
        Ok(Evaluated { hier, recs, matched, groups, universe })
    }

    /// Runs a query: which codings match the rule, returned as distinct excerpts or as assignments.
    pub fn run_query(&self, spec: &QuerySpec) -> Result<QueryResult> {
        let Evaluated { hier, recs, matched, mut groups, universe } = self.evaluate(spec)?;
        let total = groups.len();
        let truncated = total > spec.limit;
        groups.truncate(spec.limit);

        let mut coders: HashMap<i64, String> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT id, name FROM coder")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, n) = row?;
            coders.insert(id, n);
        }
        let mut sources: HashMap<i64, String> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT id, name FROM source")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, n) = row?;
            sources.insert(id, n);
        }
        let mut text_cache: HashMap<i64, Vec<char>> = HashMap::new();
        let mut items = Vec::with_capacity(groups.len());
        for g in &groups {
            let first = &recs[g[0]];
            if let Some(shape) = &first.shape {
                // A coded area has no words: it is told in words, and the shape goes with it so a screen can draw it.
                let on_page = if self.pdf_page_count(first.source_id)?.is_some() { self.get_region(-first.id).ok().map(|r| r.page) } else { None };
                items.push(QueryItem {
                    kind: if shape.is_span() { "clip" } else { "image" }.to_string(),
                    region: Some(shape.as_ref().clone()),
                    page: on_page,
                    source_id: first.source_id,
                    source_name: sources.get(&first.source_id).cloned().unwrap_or_default(),
                    start: 0,
                    end: 0,
                    text: shape.describe_on(on_page),
                    codings: g
                        .iter()
                        .map(|&i| QueryCoding {
                            coding_id: recs[i].id,
                            code_id: recs[i].code_id,
                            code_name: hier.names.get(&recs[i].code_id).cloned().unwrap_or_default(),
                            coder_name: coders.get(&recs[i].coder_id).cloned().unwrap_or_default(),
                            rating: recs[i].rating,
                        })
                        .collect(),
                });
                continue;
            }
            let chars = match text_cache.get(&first.source_id) {
                Some(c) => c,
                None => {
                    text_cache.insert(first.source_id, self.source_text(first.source_id)?.chars().collect());
                    &text_cache[&first.source_id]
                }
            };
            let (s, e) = (first.start as usize, (first.end as usize).min(chars.len()));
            let text: String = chars[s.min(e)..e].iter().collect();
            items.push(QueryItem {
                kind: "text".to_string(),
                region: None,
                page: None,
                source_id: first.source_id,
                source_name: sources.get(&first.source_id).cloned().unwrap_or_default(),
                start: first.start,
                end: first.end,
                text,
                codings: g
                    .iter()
                    .map(|&i| QueryCoding {
                        coding_id: recs[i].id,
                        code_id: recs[i].code_id,
                        code_name: hier.names.get(&recs[i].code_id).cloned().unwrap_or_default(),
                        coder_name: coders.get(&recs[i].coder_id).cloned().unwrap_or_default(),
                        rating: recs[i].rating,
                    })
                    .collect(),
            });
        }
        Ok(QueryResult { items, total, assignments: matched.len(), truncated, universe })
    }

    /// The numbers of the codings a query matches (ascending), and how many results they make, without the results themselves.
    pub fn matching_coding_ids(&self, spec: &QuerySpec) -> Result<(Vec<i64>, usize)> {
        let e = self.evaluate(spec)?;
        let mut ids: Vec<i64> = e.matched.iter().map(|&i| e.recs[i].id).collect();
        ids.sort_unstable();
        Ok((ids, e.groups.len()))
    }
}

/// A query worked out: the codings looked at, which of them matched, and how the matches group into results.
struct Evaluated {
    hier: Hierarchy,
    recs: Vec<Rec>,
    matched: Vec<usize>,
    groups: Vec<Vec<usize>>,
    universe: Counts,
}

// ---------------------------------------------------------------- the variables of cases

/// A variable cases carry (an attribute that applies to cases), with its values and how many cases have each.
#[derive(Debug, Clone, Serialize)]
pub struct CaseVariable {
    pub name: String,
    pub level: String,
    pub values: Vec<(String, i64)>,
    /// How many cases have a value that reads as a date (year-month-day), so a date range can say how many it can use.
    pub dates: i64,
    /// How many cases have a value that reads as a number.
    pub numbers: i64,
}

impl Project {
    /// The case variables, for choosing how to group cases or to ask about them.
    pub fn case_variables(&self) -> Result<Vec<CaseVariable>> {
        let mut stmt = self.conn.prepare("SELECT id, name, level FROM attribute WHERE applies_to = 'case' ORDER BY name")?;
        let attrs: Vec<(i64, String, String)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::new();
        for (id, name, level) in attrs {
            let mut stmt = self.conn.prepare("SELECT value, COUNT(*) FROM attribute_value WHERE attribute_id = ?1 AND value IS NOT NULL AND trim(value) <> '' GROUP BY value")?;
            let mut values: Vec<(String, i64)> = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
            values.sort_by_key(|a| natural(&a.0));
            let dates = values.iter().filter(|(v, _)| crate::calendar::parse_day(v).is_some()).map(|(_, n)| n).sum();
            let numbers = values.iter().filter(|(v, _)| crate::calendar::parse_number(v).is_some()).map(|(_, n)| n).sum();
            out.push(CaseVariable { name, level, values, dates, numbers });
        }
        Ok(out)
    }
}
