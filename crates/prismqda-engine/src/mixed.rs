//! Moving between kinds of data (spec 14.1).
//!
//! **Quantitizing** (codes into numbers) is a transformation of case variables, made by `TransformKind::FromCodes` in `variables.rs`, so
//! the variable it makes is an ordinary variable of cases, saved with how it was made and made again on request.
//!
//! **Qualitizing** (variable values into codes) is here. The values of a variable of cases become codes under one parent code, and each
//! case's text is coded with the code of its value: whole documents in a project that codes any words, every unit of the documents in a
//! project that codes whole units. Afterwards a query or a matrix can say "the cases that are in group B" in the language of the coding.

use std::collections::{BTreeMap, HashSet};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::explore::natural;
use crate::ops::{remove_coding_in, NewCode, NewCoding};
use crate::project::Project;
use crate::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct QualitizeSpec {
    /// The variable of cases whose values become codes.
    pub variable: String,
    /// The name of the code the new codes go under (the variable's name when absent).
    #[serde(default)]
    pub parent: Option<String>,
    /// Only these values (every value some case has, by default).
    #[serde(default)]
    pub values: Option<Vec<String>>,
}

/// One value and what coding it would make.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QualitizeValue {
    pub value: String,
    /// The code's name: the value's label, or the value.
    pub label: String,
    pub cases: usize,
    /// The documents of those cases (or the units, in a project that codes whole units) that would be coded.
    pub places: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QualitizePlan {
    pub parent: String,
    pub values: Vec<QualitizeValue>,
    /// How many codings in all.
    pub codings: usize,
    /// True when the project codes whole units, so each unit of a document is coded and not the document as one.
    pub unit_mode: bool,
    /// Cases with a chosen value that hold no document, so nothing of theirs can be coded.
    pub cases_without_text: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QualitizeOutcome {
    pub parent_id: i64,
    pub code_ids: Vec<i64>,
    pub codings: usize,
    /// The codings that were made, so the whole step can be taken back.
    pub coding_ids: Vec<i64>,
}

/// What `qualitize` made, as it reported it: the one thing `qualitize_take_back` is asked to take back.
#[derive(Debug, Clone, Deserialize)]
pub struct QualitizeUndo {
    pub parent_id: i64,
    pub code_ids: Vec<i64>,
    pub coding_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QualitizeTakeBack {
    pub codings_removed: usize,
    pub codes_retired: usize,
}

/// A place to put the code of a value: a span of one source, and the unit it is (in a project that codes units).
#[derive(Debug, Clone)]
struct Place {
    source_id: i64,
    start: i64,
    end: i64,
    unit_id: Option<i64>,
}

impl Project {
    /// What qualitizing would do, without doing it: the codes it would make and how many codings each gets.
    pub fn qualitize_preview(&self, spec: &QualitizeSpec) -> Result<QualitizePlan> {
        Ok(self.qualitize_plan(spec)?.0)
    }

    /// The plan, and for each value of it (in the order of the plan) the places to code.
    fn qualitize_plan(&self, spec: &QualitizeSpec) -> Result<(QualitizePlan, Vec<Vec<Place>>)> {
        let data = self.case_data()?;
        let at = data.variables.iter().position(|v| v.name == spec.variable).ok_or_else(|| Error::Invalid(format!("no case variable is named '{}'", spec.variable)))?;
        let def = &data.variables[at];
        if def.level == "text" {
            return Err(Error::Invalid(format!("'{}' holds free text, which has too many different values to be codes.", def.name)));
        }
        // The cases of each value.
        let mut by_value: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for (case, row) in data.cases.iter().zip(&data.values) {
            if let Some(v) = row[at].as_ref().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
                by_value.entry(v).or_default().push(case.id);
            }
        }
        let mut values: Vec<String> = match &spec.values {
            Some(chosen) => {
                let mut out = Vec::new();
                for c in chosen {
                    if !by_value.contains_key(c) {
                        return Err(Error::Invalid(format!("no case has the value '{c}' of '{}'", def.name)));
                    }
                    if !out.contains(c) {
                        out.push(c.clone());
                    }
                }
                out
            }
            None => by_value.keys().cloned().collect(),
        };
        values.sort_by_key(|v| natural(v));
        if values.is_empty() {
            return Err(Error::Invalid(format!("no case has a value of '{}', so there are no codes to make", def.name)));
        }
        if values.len() > 60 {
            return Err(Error::Invalid(format!("'{}' has {} different values, too many to make into codes. Choose the values you want, or group them first (Recode in the data editor).", def.name, values.len())));
        }
        let unit_mode = self.info()?.coding_mode == "unit";
        let places_of = self.places_by_case(unit_mode)?;
        let mut plan_values = Vec::new();
        let mut places = Vec::new();
        let mut without = HashSet::new();
        let mut total = 0;
        for v in &values {
            let mut here: Vec<Place> = Vec::new();
            let mut seen: HashSet<(i64, i64, i64)> = HashSet::new();
            let cases = &by_value[v];
            for case in cases {
                match places_of.get(case) {
                    Some(list) if !list.is_empty() => {
                        for p in list {
                            if seen.insert((p.source_id, p.start, p.end)) {
                                here.push(p.clone());
                            }
                        }
                    }
                    _ => {
                        without.insert(*case);
                    }
                }
            }
            total += here.len();
            plan_values.push(QualitizeValue { value: v.clone(), label: def.value_labels.get(v).cloned().unwrap_or_else(|| v.clone()), cases: cases.len(), places: here.len() });
            places.push(here);
        }
        let parent = spec.parent.as_deref().map(str::trim).filter(|p| !p.is_empty()).unwrap_or(&def.name).to_string();
        Ok((QualitizePlan { parent, values: plan_values, codings: total, unit_mode, cases_without_text: without.len() }, places))
    }

    /// The places of each case that qualitizing codes: for a document it holds, the whole text (or each unit of it, in a project that codes
    /// whole units); for a unit it holds, that unit. Units set aside (an interviewer's turns) are left alone.
    fn places_by_case(&self, unit_mode: bool) -> Result<BTreeMap<i64, Vec<Place>>> {
        let mut out: BTreeMap<i64, Vec<Place>> = BTreeMap::new();
        let members: Vec<(i64, Option<i64>, Option<i64>)> = {
            let mut stmt = self.conn.prepare("SELECT case_id, source_id, unit_id FROM case_member ORDER BY case_id, source_id, unit_id")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        for (case, source, unit) in members {
            match (source, unit) {
                (Some(s), _) => {
                    if unit_mode {
                        let mut stmt = self.conn.prepare("SELECT id, start_offset, end_offset FROM unit WHERE source_id = ?1 AND excluded = 0 ORDER BY start_offset")?;
                        let units = stmt.query_map([s], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?.collect::<std::result::Result<Vec<_>, _>>()?;
                        for (id, start, end) in units {
                            out.entry(case).or_default().push(Place { source_id: s, start, end, unit_id: Some(id) });
                        }
                    } else {
                        let len = self.source_text(s)?.chars().count() as i64;
                        if len > 0 {
                            out.entry(case).or_default().push(Place { source_id: s, start: 0, end: len, unit_id: None });
                        }
                    }
                }
                (None, Some(u)) => {
                    let row: Option<(i64, i64, i64, i64)> = self
                        .conn
                        .query_row("SELECT source_id, start_offset, end_offset, excluded FROM unit WHERE id = ?1", params![u], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                        .ok();
                    if let Some((source_id, start, end, excluded)) = row.filter(|r| r.3 == 0) {
                        let _ = excluded;
                        out.entry(case).or_default().push(Place { source_id, start, end, unit_id: unit_mode.then_some(u) });
                    }
                }
                (None, None) => {}
            }
        }
        Ok(out)
    }

    /// Makes a code for each value of a variable of cases, under one parent code, and codes each case's text with the code of its value,
    /// as `coder_id`. Nothing is done when any step is refused.
    pub fn qualitize(&mut self, spec: &QualitizeSpec, coder_id: i64) -> Result<QualitizeOutcome> {
        self.ensure_writable()?;
        let (plan, places) = self.qualitize_plan(spec)?;
        if plan.codings == 0 {
            return Err(Error::Invalid("none of the cases with these values holds a document, so there is no text to code.".to_string()));
        }
        let parent_id = self.add_code(&NewCode::new(plan.parent.clone()))?;
        let mut code_ids: Vec<i64> = Vec::new();
        let mut codings: Vec<NewCoding> = Vec::new();
        let made = (|| -> Result<()> {
            for (value, here) in plan.values.iter().zip(&places) {
                let mut child = NewCode::new(value.label.clone());
                child.parent_id = Some(parent_id);
                let id = self.add_code(&child)?;
                code_ids.push(id);
                for p in here {
                    let mut c = NewCoding::new(p.source_id, p.start, p.end, id, coder_id);
                    c.unit_id = p.unit_id;
                    codings.push(c);
                }
            }
            Ok(())
        })();
        let result = made.and_then(|_| self.apply_codings(&codings));
        match result {
            Ok(ids) => Ok(QualitizeOutcome { parent_id, code_ids, codings: ids.len(), coding_ids: ids }),
            Err(e) => {
                // Leave no half-made codes behind: the codings were all-or-nothing, so only the codes need to go.
                for id in code_ids.iter().rev().chain(std::iter::once(&parent_id)) {
                    let _ = self.retire_code(*id);
                }
                Err(e)
            }
        }
    }

    /// Takes back what `qualitize` made: removes the codings it made (each removal logged with its full contents, as any removal is) and
    /// retires the codes it made, the children first. Only a coding of one of those codes is touched, so a coding made by hand since,
    /// even of the same code, stays; a coding that is already gone is passed over. The retired codes stay in the codebook.
    pub fn qualitize_take_back(&mut self, undo: &QualitizeUndo) -> Result<QualitizeTakeBack> {
        self.ensure_writable()?;
        let mut removed = 0;
        let tx = self.conn.transaction()?;
        for id in &undo.coding_ids {
            let code: Option<i64> = tx.query_row("SELECT code_id FROM coding WHERE id = ?1", [id], |r| r.get(0)).ok();
            if code.is_some_and(|c| undo.code_ids.contains(&c)) {
                remove_coding_in(&tx, &self.actor, &self.session, *id, None)?;
                removed += 1;
            }
        }
        tx.commit()?;
        let mut retired = 0;
        for id in undo.code_ids.iter().chain(std::iter::once(&undo.parent_id)) {
            let status: Option<String> = self.conn.query_row("SELECT status FROM code WHERE id = ?1", [id], |r| r.get(0)).ok();
            if status.as_deref() == Some("active") {
                self.retire_code(*id)?;
                retired += 1;
            }
        }
        Ok(QualitizeTakeBack { codings_removed: removed, codes_retired: retired })
    }
}
