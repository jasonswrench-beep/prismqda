//! The code by variable bubble plot, and the table of it that serves as the descriptor by descriptor by code chart (spec 14.2): how often
//! one code is applied across two crossed variables.
//!
//! Each cell is a value of the first variable and a value of the second. Its size is how many distinct excerpts of the cases in it carry
//! the code, and its colour is the mean weight (the rating) of the codings that have one. A cell shows also how many of its cases have
//! the code at all, so a large cell can be told from a large group.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::joint::{is_missing, number, variable, yes, TypologyCategory};
use super::{natural, MatrixSpec, RowSpec, Scope};
use crate::project::Project;
use crate::variables::VariableDef;
use crate::{Error, Result};

/// The most different values an axis may have.
const MOST_VALUES: usize = 12;

#[derive(Debug, Clone, Deserialize)]
pub struct BubbleSpec {
    pub code_id: i64,
    #[serde(default = "yes")]
    pub include_descendants: bool,
    /// The variable along the bottom, and the one up the side.
    pub x: String,
    pub y: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BubbleAxis {
    pub name: String,
    pub categories: Vec<TypologyCategory>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BubbleCell {
    /// The positions on the two axes (indexes into the categories).
    pub x: usize,
    pub y: usize,
    /// How many cases that hold text are in the cell, and how many of them have the code.
    pub cases: usize,
    pub with_code: usize,
    /// How many distinct excerpts of the cases in the cell carry the code.
    pub excerpts: usize,
    /// How many codings have a weight, and their mean.
    pub rated: usize,
    pub mean_rating: Option<f64>,
    /// One coding for each excerpt: the evidence behind the size.
    pub coding_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bubble {
    pub code: String,
    pub x: BubbleAxis,
    pub y: BubbleAxis,
    /// The cells that hold at least one case, from the bottom row up and from left to right.
    pub cells: Vec<BubbleCell>,
    /// The cases that hold text and have a value of both variables, which is what the cells are made of.
    pub cases_used: usize,
    /// The cases that hold text and are left out because they have no value of one of the variables.
    pub left_out: usize,
    /// How many codings in the plot have a weight (when none has, the cells have no colour to tell apart).
    pub rated: usize,
}

/// The values of a variable as the axis of a plot: numbers by number, other values by word, with the label of each.
fn axis(data: &crate::variables::CaseData, i: usize, def: &VariableDef, wanted: &HashSet<usize>) -> Result<BubbleAxis> {
    let mut seen: Vec<String> = Vec::new();
    for row in wanted {
        let v = data.values[*row][i].as_deref();
        if !is_missing(def, v) {
            let v = v.unwrap_or_default().trim().to_string();
            if !seen.contains(&v) {
                seen.push(v);
            }
        }
    }
    if seen.len() > MOST_VALUES {
        return Err(Error::Invalid(format!("'{}' has {} different values, more than a plot can show along an axis; make groups of them first with Cut in the Data editor", def.name, seen.len())));
    }
    if seen.iter().all(|v| number(v).is_some()) {
        seen.sort_by(|a, b| number(a).unwrap_or(0.0).total_cmp(&number(b).unwrap_or(0.0)));
    } else {
        seen.sort_by_key(|v| natural(v));
    }
    Ok(BubbleAxis { name: def.name.clone(), categories: seen.into_iter().map(|v| TypologyCategory { label: def.value_labels.get(&v).cloned().unwrap_or_else(|| v.clone()), value: v }).collect() })
}

impl Project {
    /// One code across the values of two variables.
    pub fn bubble_plot(&self, spec: &BubbleSpec) -> Result<Bubble> {
        if spec.x == spec.y {
            return Err(Error::Invalid("choose two different variables for the two axes".to_string()));
        }
        let data = self.case_data()?;
        let (xi, xdef) = variable(&data, &spec.x)?;
        let (yi, ydef) = variable(&data, &spec.y)?;
        for def in [xdef, ydef] {
            if def.level == "text" {
                return Err(Error::Invalid(format!("'{}' holds free text, which cannot be an axis; choose a variable of categories or numbers", def.name)));
            }
        }
        let code = self.code_name(spec.code_id)?;
        let held = self.cases_with_members()?;
        let with_text: Vec<usize> = (0..data.cases.len()).filter(|i| held.contains(&data.cases[*i].id)).collect();
        let value = |row: usize, i: usize, def: &VariableDef| -> Option<String> {
            let v = data.values[row][i].as_deref();
            (!is_missing(def, v)).then(|| v.unwrap_or_default().trim().to_string())
        };
        let used: HashSet<usize> = with_text.iter().copied().filter(|r| value(*r, xi, xdef).is_some() && value(*r, yi, ydef).is_some()).collect();
        let left_out = with_text.len() - used.len();
        let (x, y) = (axis(&data, xi, xdef, &used)?, axis(&data, yi, ydef, &used)?);
        let position = |axis: &BubbleAxis, v: &str| axis.categories.iter().position(|c| c.value == v);

        // The cell each case is in.
        let mut cell_of: HashMap<i64, (usize, usize)> = HashMap::new();
        let mut cases: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for row in &used {
            let (Some(cx), Some(cy)) = (value(*row, xi, xdef).and_then(|v| position(&x, &v)), value(*row, yi, ydef).and_then(|v| position(&y, &v))) else { continue };
            cell_of.insert(data.cases[*row].id, (cx, cy));
            *cases.entry((cy, cx)).or_default() += 1;
        }

        let mspec = MatrixSpec { rows: RowSpec::Cases, code_ids: vec![spec.code_id], include_descendants: spec.include_descendants, scope: Scope { layer: Some("coder".to_string()), ..Default::default() } };
        let (hits, _, _) = self.matrix_hits(&mspec)?;
        #[derive(Default)]
        struct Tally {
            excerpts: BTreeMap<(i64, i64, i64), i64>,
            cases: HashSet<i64>,
            ratings: HashMap<i64, i64>,
        }
        let mut tallies: HashMap<(usize, usize), Tally> = HashMap::new();
        let mut rated_all: HashSet<i64> = HashSet::new();
        for (r, keys, _) in &hits {
            for (key, _, _) in keys {
                let Some(case) = key.strip_prefix("case:").and_then(|n| n.parse::<i64>().ok()) else { continue };
                let Some(&(cx, cy)) = cell_of.get(&case) else { continue };
                let tally = tallies.entry((cy, cx)).or_default();
                tally.excerpts.entry((r.source_id, r.start, r.end)).or_insert(r.id);
                tally.cases.insert(case);
                if let Some(rating) = r.rating {
                    tally.ratings.insert(r.id, rating);
                    rated_all.insert(r.id);
                }
            }
        }
        let cells = cases
            .into_iter()
            .map(|((cy, cx), n)| {
                let tally = tallies.remove(&(cy, cx)).unwrap_or_default();
                let rated = tally.ratings.len();
                BubbleCell {
                    x: cx,
                    y: cy,
                    cases: n,
                    with_code: tally.cases.len(),
                    excerpts: tally.excerpts.len(),
                    rated,
                    mean_rating: (rated > 0).then(|| tally.ratings.values().sum::<i64>() as f64 / rated as f64),
                    coding_ids: tally.excerpts.values().copied().collect(),
                }
            })
            .collect();
        Ok(Bubble { code, x, y, cells, cases_used: used.len(), left_out, rated: rated_all.len() })
    }
}
