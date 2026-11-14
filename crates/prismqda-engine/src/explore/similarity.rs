//! Similarity analysis (spec 14.2): which cases are most alike on the codes and the variables chosen.
//!
//! The measure is Gower's similarity, which is made for a mixture of kinds of data. For each feature two cases are compared on a scale
//! of 0 (unlike) to 1 (alike), and their similarity is the mean of those over the features both can be compared on:
//!
//! * a **code** is present in a case when the case has at least one excerpt of it: both have it, 1; one has it, 0; neither has it, nothing
//!   to compare (two cases do not become alike by both lacking a code), unless shared absence is asked for, and then 1;
//! * a **number** is 1 less the difference of the two values over the range of the values in all the cases looked at;
//! * a **category** is 1 when the two are the same and 0 when not;
//! * a value that is missing, in either case, leaves that feature out for that pair.

use serde::{Deserialize, Serialize};

use super::joint::{is_missing, number, variable, yes, TypologyCase};
use super::natural;
use crate::project::Project;
use crate::{Error, Result};

/// The most cases whose whole matrix of similarities is sent.
const MATRIX_CASES: usize = 60;

fn fifteen() -> usize {
    15
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimilaritySpec {
    #[serde(default)]
    pub code_ids: Vec<i64>,
    #[serde(default = "yes")]
    pub include_descendants: bool,
    #[serde(default)]
    pub variables: Vec<String>,
    /// Two cases that both lack a code are alike on it (otherwise they are not compared on it).
    #[serde(default)]
    pub shared_absence: bool,
    /// How many of the most alike pairs to list (1 to 50).
    #[serde(default = "fifteen")]
    pub pairs: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SimilarityFeature {
    pub name: String,
    /// `code`, `number`, or `category`.
    pub kind: String,
}

/// One feature of one pair: what each case has, and how alike they are on it (none where they are not compared on it).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PairPart {
    pub feature: usize,
    /// For a code `1` (the case has it) or `0`; for a number the number; for a category its label; none where there is no value.
    pub a: Option<String>,
    pub b: Option<String>,
    pub similarity: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlikePair {
    pub a: TypologyCase,
    pub b: TypologyCase,
    pub similarity: f64,
    /// On how many features the two were compared.
    pub compared: usize,
    pub parts: Vec<PairPart>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Similarity {
    pub features: Vec<SimilarityFeature>,
    /// The cases that hold text, in the order of their names.
    pub cases: Vec<TypologyCase>,
    /// The similarity of every pair of cases, in the order of `cases`; none where the pair has nothing to be compared on. Sent only for
    /// up to 60 cases.
    pub matrix: Option<Vec<Vec<Option<f64>>>>,
    pub pairs: Vec<AlikePair>,
    /// How many pairs there are with something to compare.
    pub pairs_compared: usize,
}

/// What a case has for one feature.
#[derive(Debug, Clone, PartialEq)]
enum Has {
    Missing,
    Code(bool),
    Number(f64),
    Category(String, String),
}

impl Has {
    fn shown(&self) -> Option<String> {
        match self {
            Has::Missing => None,
            Has::Code(p) => Some(if *p { "1" } else { "0" }.to_string()),
            Has::Number(x) => Some(format!("{x}")),
            Has::Category(_, label) => Some(label.clone()),
        }
    }
}

impl Project {
    /// The cases that hold text, compared on the codes and the variables chosen.
    pub fn similarity_analysis(&self, spec: &SimilaritySpec) -> Result<Similarity> {
        let mut code_ids: Vec<i64> = Vec::new();
        for c in &spec.code_ids {
            if !code_ids.contains(c) {
                code_ids.push(*c);
            }
        }
        if code_ids.is_empty() && spec.variables.is_empty() {
            return Err(Error::Invalid("choose at least one code or variable to compare the cases on".to_string()));
        }
        let data = self.case_data()?;
        let mut vars = Vec::new();
        for n in &spec.variables {
            let (i, def) = variable(&data, n)?;
            if def.level == "text" {
                return Err(Error::Invalid(format!("'{n}' holds free text, which cannot be compared; choose another variable")));
            }
            if !vars.iter().any(|(j, _)| *j == i) {
                vars.push((i, def));
            }
        }
        let names: Vec<String> = code_ids.iter().map(|c| self.code_name(*c)).collect::<Result<_>>()?;
        let held = self.cases_with_members()?;
        let counts = self.case_code_excerpts(&code_ids, spec.include_descendants)?;
        let mut at: Vec<usize> = (0..data.cases.len()).filter(|i| held.contains(&data.cases[*i].id)).collect();
        at.sort_by(|a, b| natural(&data.cases[*a].name).cmp(&natural(&data.cases[*b].name)).then(data.cases[*a].id.cmp(&data.cases[*b].id)));
        if at.len() < 2 {
            return Err(Error::Invalid("at least two cases that hold text are needed to compare them".to_string()));
        }

        let mut features: Vec<SimilarityFeature> = names.iter().map(|n| SimilarityFeature { name: n.clone(), kind: "code".to_string() }).collect();
        // `has[case][feature]`, and the range of each number.
        let mut has: Vec<Vec<Has>> = at.iter().map(|i| (0..code_ids.len()).map(|k| Has::Code(counts.get(&data.cases[*i].id).is_some_and(|c| c[k] > 0))).collect()).collect();
        let mut ranges: Vec<f64> = vec![0.0; code_ids.len()];
        for (i, def) in &vars {
            let numeric = def.level == "interval" || def.level == "ratio" || (def.level == "ordinal" && at.iter().all(|c| data.values[*c][*i].as_deref().is_none_or(|v| is_missing(def, Some(v)) || number(v).is_some())));
            let value = |case: usize| -> Option<&str> {
                let v = data.values[case][*i].as_deref();
                if is_missing(def, v) {
                    None
                } else {
                    v.map(str::trim)
                }
            };
            for (row, case) in at.iter().enumerate() {
                has[row].push(match value(*case) {
                    None => Has::Missing,
                    Some(v) if numeric => number(v).map_or(Has::Missing, Has::Number),
                    Some(v) => Has::Category(v.to_string(), def.value_labels.get(v).cloned().unwrap_or_else(|| v.to_string())),
                });
            }
            let column_index = features.len();
            let xs: Vec<f64> = has.iter().filter_map(|h| if let Has::Number(x) = &h[column_index] { Some(*x) } else { None }).collect();
            ranges.push(if numeric && !xs.is_empty() { xs.iter().copied().fold(f64::MIN, f64::max) - xs.iter().copied().fold(f64::MAX, f64::min) } else { 0.0 });
            features.push(SimilarityFeature { name: def.name.clone(), kind: if numeric { "number" } else { "category" }.to_string() });
        }

        let part = |a: &Has, b: &Has, k: usize| -> Option<f64> {
            match (a, b) {
                (Has::Code(x), Has::Code(y)) => match (x, y) {
                    (true, true) => Some(1.0),
                    (false, false) => spec.shared_absence.then_some(1.0),
                    _ => Some(0.0),
                },
                (Has::Number(x), Has::Number(y)) => Some(if ranges[k] > 0.0 { 1.0 - (x - y).abs() / ranges[k] } else { 1.0 }),
                (Has::Category(x, _), Has::Category(y, _)) => Some(if x == y { 1.0 } else { 0.0 }),
                _ => None,
            }
        };
        let compare = |a: usize, b: usize| -> (Option<f64>, usize) {
            let parts: Vec<f64> = (0..features.len()).filter_map(|k| part(&has[a][k], &has[b][k], k)).collect();
            (if parts.is_empty() { None } else { Some(parts.iter().sum::<f64>() / parts.len() as f64) }, parts.len())
        };

        let n = at.len();
        let mut matrix = vec![vec![None; n]; n];
        for (i, row) in matrix.iter_mut().enumerate() {
            row[i] = Some(1.0);
        }
        let mut found: Vec<(usize, usize, f64, usize)> = Vec::new();
        for (a, b) in (0..n).flat_map(|a| (a + 1..n).map(move |b| (a, b))) {
            let (s, compared) = compare(a, b);
            matrix[a][b] = s;
            matrix[b][a] = s;
            if let Some(s) = s {
                found.push((a, b, s, compared));
            }
        }
        let pairs_compared = found.len();
        found.sort_by(|x, y| y.2.total_cmp(&x.2).then(y.3.cmp(&x.3)).then(x.0.cmp(&y.0)).then(x.1.cmp(&y.1)));
        found.truncate(spec.pairs.clamp(1, 50));
        let case = |row: usize| TypologyCase { id: data.cases[at[row]].id, name: data.cases[at[row]].name.clone() };
        let pairs = found
            .into_iter()
            .map(|(a, b, similarity, compared)| AlikePair {
                a: case(a),
                b: case(b),
                similarity,
                compared,
                parts: (0..features.len()).map(|k| PairPart { feature: k, a: has[a][k].shown(), b: has[b][k].shown(), similarity: part(&has[a][k], &has[b][k], k) }).collect(),
            })
            .collect();
        Ok(Similarity { features, cases: (0..n).map(case).collect(), matrix: (n <= MATRIX_CASES).then_some(matrix), pairs, pairs_compared })
    }
}
