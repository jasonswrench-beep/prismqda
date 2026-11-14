//! Crosstabs (Explore): how many cases have each pair of values of two case variables, so a table such as "age group by
//! site" can be read and every cell opened to the cases in it. Only counting is done here; a test of whether the pattern is
//! more than chance belongs to the statistics tool, and percentages are worked out from these counts by the interface,
//! which says what each is a percentage of.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::calendar::{parse_day, parse_number};
use crate::explore::natural;
use crate::project::Project;
use crate::{Error, Result};

/// The most different values one side of a crosstab may have; more is not a table a person can read.
const MAX_VALUES: usize = 100;

#[derive(Debug, Clone, Deserialize)]
pub struct CrosstabSpec {
    /// The case variable down the side.
    pub row_variable: String,
    /// The case variable across the top.
    pub column_variable: String,
    /// Group the side's numeric values into ranges this wide (0 to 10, 10 to 20, ...). A value that is not a number is left as it is.
    #[serde(default)]
    pub row_width: Option<f64>,
    #[serde(default)]
    pub column_width: Option<f64>,
    /// Only these cases (every case by default).
    #[serde(default)]
    pub case_ids: Option<Vec<i64>>,
}

/// One row or column of the table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrosstabValue {
    /// What to ask for the cases behind it (`""` is the cases with no value).
    pub key: String,
    pub label: String,
    /// How many cases have it.
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrosstabAxis {
    pub variable: String,
    pub level: String,
    pub values: Vec<CrosstabValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Crosstab {
    pub rows: CrosstabAxis,
    pub columns: CrosstabAxis,
    /// `cells[row][column]`: how many cases have that row value and that column value.
    pub cells: Vec<Vec<i64>>,
    /// How many cases the table counts.
    pub total: i64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaseRef {
    pub id: i64,
    pub name: String,
    /// A document the case is in (or has a unit of), and where in it, so the case can be opened.
    pub source_id: Option<i64>,
    pub start: Option<i64>,
    pub end: Option<i64>,
}

/// Where a case's value falls on one side: its key and label, and what to sort by.
struct Bucket {
    key: String,
    label: String,
    order: Option<f64>,
}

fn show(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

fn bucket(value: Option<&str>, width: Option<f64>) -> Bucket {
    let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Bucket { key: String::new(), label: "(no value)".to_string(), order: None };
    };
    if let (Some(w), Some(x)) = (width.filter(|w| *w > 0.0), parse_number(v)) {
        let lo = (x / w).floor() * w;
        return Bucket { key: format!("range:{}", show(lo)), label: format!("{} to under {}", show(lo), show(lo + w)), order: Some(lo) };
    }
    let order = parse_number(v).or_else(|| parse_day(v).map(|d| d as f64));
    Bucket { key: v.to_string(), label: v.to_string(), order }
}

impl Project {
    fn case_names(&self, only: Option<&[i64]>) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM research_case ORDER BY id")?;
        let rows: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(rows.into_iter().filter(|(id, _)| only.is_none_or_in(*id)).collect())
    }

    /// The value of a case variable for each case that has one, and the variable's level.
    fn variable_values(&self, name: &str) -> Result<(HashMap<i64, String>, String)> {
        let attr: Option<(i64, String)> = self
            .conn
            .query_row("SELECT id, level FROM attribute WHERE applies_to = 'case' AND name = ?1", [name], |r| Ok((r.get(0)?, r.get(1)?)))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let (id, level) = attr.ok_or_else(|| Error::Invalid(format!("no case variable is named '{name}'")))?;
        let mut stmt = self.conn.prepare("SELECT target_id, value FROM attribute_value WHERE attribute_id = ?1")?;
        let mut values = HashMap::new();
        for row in stmt.query_map(params![id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))? {
            let (case, v) = row?;
            if let Some(v) = v.filter(|v| !v.trim().is_empty()) {
                values.insert(case, v.trim().to_string());
            }
        }
        Ok((values, level))
    }

    /// Counts the cases that have each pair of values of two case variables.
    pub fn crosstab(&self, spec: &CrosstabSpec) -> Result<Crosstab> {
        let (table, _, _) = self.crosstab_inner(spec)?;
        Ok(table)
    }

    /// The cases in one cell (`row` and `column` keys), in a row or column (the other `None`), or in the whole table (both `None`).
    pub fn crosstab_cases(&self, spec: &CrosstabSpec, row: Option<&str>, column: Option<&str>) -> Result<Vec<CaseRef>> {
        let (_, placed, names) = self.crosstab_inner(spec)?;
        let mut out: Vec<CaseRef> = placed
            .into_iter()
            .filter(|(_, r, c)| row.is_none_or_eq(r) && column.is_none_or_eq(c))
            .map(|(id, _, _)| {
                // The first document or unit the case holds, for opening it.
                let place: Option<(Option<i64>, Option<i64>, Option<i64>)> = self
                    .conn
                    .query_row(
                        "SELECT COALESCE(m.source_id, u.source_id), u.start_offset, u.end_offset FROM case_member m LEFT JOIN unit u ON u.id = m.unit_id
                         WHERE m.case_id = ?1 AND COALESCE(m.source_id, u.source_id) IS NOT NULL ORDER BY m.rowid LIMIT 1",
                        [id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()
                    .ok()
                    .flatten();
                let (source_id, start, end) = place.unwrap_or((None, None, None));
                CaseRef { id, name: names.get(&id).cloned().unwrap_or_default(), source_id, start, end }
            })
            .collect();
        out.sort_by(|a, b| natural(&a.name).cmp(&natural(&b.name)).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// The table, each case with its row and column key, and the case names.
    #[allow(clippy::type_complexity)]
    fn crosstab_inner(&self, spec: &CrosstabSpec) -> Result<(Crosstab, Vec<(i64, String, String)>, HashMap<i64, String>)> {
        let cases = self.case_names(spec.case_ids.as_deref())?;
        let (row_values, row_level) = self.variable_values(&spec.row_variable)?;
        let (col_values, col_level) = self.variable_values(&spec.column_variable)?;
        let names: HashMap<i64, String> = cases.iter().cloned().collect();

        let mut placed: Vec<(i64, String, String)> = Vec::with_capacity(cases.len());
        let mut row_buckets: HashMap<String, Bucket> = HashMap::new();
        let mut col_buckets: HashMap<String, Bucket> = HashMap::new();
        for (id, _) in &cases {
            let r = bucket(row_values.get(id).map(String::as_str), spec.row_width);
            let c = bucket(col_values.get(id).map(String::as_str), spec.column_width);
            placed.push((*id, r.key.clone(), c.key.clone()));
            row_buckets.entry(r.key.clone()).or_insert(r);
            col_buckets.entry(c.key.clone()).or_insert(c);
        }
        let row_order = sorted_keys(&row_buckets);
        let col_order = sorted_keys(&col_buckets);
        for (order, variable) in [(&row_order, &spec.row_variable), (&col_order, &spec.column_variable)] {
            let distinct = order.iter().filter(|k| !k.is_empty()).count();
            if distinct > MAX_VALUES {
                return Err(Error::Invalid(format!(
                    "{variable} has {distinct} different values, which is too many for a table you can read (the limit is {MAX_VALUES}); group its numbers into ranges, or choose another variable"
                )));
            }
        }
        let row_index: HashMap<&str, usize> = row_order.iter().enumerate().map(|(i, k)| (k.as_str(), i)).collect();
        let col_index: HashMap<&str, usize> = col_order.iter().enumerate().map(|(i, k)| (k.as_str(), i)).collect();
        let mut cells = vec![vec![0i64; col_order.len()]; row_order.len()];
        let mut row_totals = vec![0i64; row_order.len()];
        let mut col_totals = vec![0i64; col_order.len()];
        for (_, r, c) in &placed {
            let (i, j) = (row_index[r.as_str()], col_index[c.as_str()]);
            cells[i][j] += 1;
            row_totals[i] += 1;
            col_totals[j] += 1;
        }
        let values = |order: &[String], buckets: &HashMap<String, Bucket>, totals: &[i64]| -> Vec<CrosstabValue> {
            order.iter().enumerate().map(|(i, k)| CrosstabValue { key: k.clone(), label: buckets[k].label.clone(), total: totals[i] }).collect()
        };
        let notes = vec![
            "Each number is a count of cases. A case with no value for a variable is counted in the row or column called (no value), so every case is counted once and the totals add up.".to_string(),
            "Values are compared as written, so \"Female\" and \"female\" are two values. Numbers and dates are put in order, other words in alphabetical order.".to_string(),
        ];
        let table = Crosstab {
            rows: CrosstabAxis { variable: spec.row_variable.clone(), level: row_level, values: values(&row_order, &row_buckets, &row_totals) },
            columns: CrosstabAxis { variable: spec.column_variable.clone(), level: col_level, values: values(&col_order, &col_buckets, &col_totals) },
            cells,
            total: placed.len() as i64,
            notes,
        };
        Ok((table, placed, names))
    }
}

/// The keys of the buckets in the order a table shows them: numbers and dates in order first, then other values
/// alphabetically, and the cases with no value last.
fn sorted_keys(buckets: &HashMap<String, Bucket>) -> Vec<String> {
    let mut keys: Vec<&String> = buckets.keys().collect();
    keys.sort_by(|a, b| {
        let (x, y) = (&buckets[*a], &buckets[*b]);
        x.key
            .is_empty()
            .cmp(&y.key.is_empty())
            .then_with(|| x.order.is_none().cmp(&y.order.is_none()))
            .then_with(|| match (x.order, y.order) {
                (Some(p), Some(q)) => p.partial_cmp(&q).unwrap_or(std::cmp::Ordering::Equal),
                _ => natural(&x.label).cmp(&natural(&y.label)),
            })
            .then_with(|| a.cmp(b))
    });
    keys.into_iter().cloned().collect()
}

/// Small helpers so the filters above read as what they mean.
trait Chosen {
    fn is_none_or_in(&self, id: i64) -> bool;
}

impl Chosen for Option<&[i64]> {
    fn is_none_or_in(&self, id: i64) -> bool {
        self.is_none_or(|ids| ids.contains(&id))
    }
}

trait Matches {
    fn is_none_or_eq(&self, key: &str) -> bool;
}

impl Matches for Option<&str> {
    fn is_none_or_eq(&self, key: &str) -> bool {
        self.is_none_or(|k| k == key)
    }
}
