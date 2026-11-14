//! Mixed Methods kept in the project (spec 14.2 and 14.3): the design of the study, the triangulation matrix with the researcher's marks of
//! convergence, complementarity, or discordance, and the integration log.
//!
//! What the researcher decides is kept; what the data say is not. A triangulation row names a code and a variable of cases, and the words
//! and the numbers beside the row are worked out from the project each time, so they never go stale. The mark, and what each strand
//! says, are the researcher's own words and are never written by the program.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::event;
use crate::explore::{SideBySideSpec, TopicQuote, TypologyCell, TypologyColumn};
use crate::project::Project;
use crate::variables::VariableSpec;
use crate::{Error, NewCode, Result};

const MAX_TOPIC: usize = 160;
const MAX_TEXT: usize = 4_000;

pub const DESIGN_KINDS: [&str; 4] = ["convergent", "explanatory_sequential", "exploratory_sequential", "embedded"];
const PRIORITIES: [&str; 3] = ["equal", "quantitative", "qualitative"];
const MARKS: [&str; 4] = ["", "convergence", "complementarity", "discordance"];
const STAGES: [&str; 5] = ["design", "data_collection", "analysis", "interpretation", "reporting"];
const HOWS: [&str; 5] = ["connecting", "building", "merging", "embedding", "transforming"];

fn clean(text: &str, max: usize, what: &str) -> Result<String> {
    let text = text.trim();
    if text.chars().count() > max {
        return Err(Error::Invalid(format!("{what} can be at most {max} characters")));
    }
    Ok(text.to_string())
}

fn one_of(value: &str, allowed: &[&str], what: &str) -> Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(Error::Invalid(format!("'{value}' is not one of the choices for {what}: {}", allowed.iter().filter(|a| !a.is_empty()).copied().collect::<Vec<_>>().join(", "))))
    }
}

// ---------------------------------------------------------------- the design

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MixedDesign {
    pub kind: String,
    pub priority: String,
    pub purpose: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DesignInput {
    pub kind: String,
    /// Which strand has priority; the design's usual one when absent.
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub purpose: String,
}

/// What a design template would set up, so it can be shown before anything is made.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannedVariable {
    pub name: String,
    pub level: String,
    /// True when the project already has a variable of that name, which is then left as it is.
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannedCode {
    pub name: String,
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DesignPlan {
    pub kind: String,
    /// The priority the design usually has.
    pub priority: String,
    /// How the strands are brought together in this design, for the first entry of the integration log.
    pub how: String,
    pub variables: Vec<PlannedVariable>,
    pub codes: Vec<PlannedCode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DesignApply {
    #[serde(flatten)]
    pub design: DesignInput,
    /// The sentence for the first entry of the integration log (the interface words it); no entry when empty.
    #[serde(default)]
    pub log: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesignOutcome {
    pub design: MixedDesign,
    pub variables_made: Vec<String>,
    pub variables_kept: Vec<String>,
    pub codes_made: Vec<String>,
    pub codes_kept: Vec<String>,
    pub entry_id: Option<i64>,
}

/// What each design needs in the project: the variables of cases and the theme codes it makes, the priority it usually has, and how it
/// brings the strands together.
struct Template {
    variables: &'static [(&'static str, &'static str)],
    codes: &'static [&'static str],
    priority: &'static str,
    how: &'static str,
}

fn template(kind: &str) -> Result<Template> {
    Ok(match kind {
        // The strands are collected and analysed separately and then merged: the matrix and the joint displays are the means, so nothing is made.
        "convergent" => Template { variables: &[], codes: &[], priority: "equal", how: "merging" },
        // The numbers come first and the interviews explain them: a variable for the group each case fell in, and a theme for the explanations.
        "explanatory_sequential" => Template { variables: &[("Quantitative result group", "nominal")], codes: &["Explaining the quantitative result"], priority: "quantitative", how: "connecting" },
        // The interviews come first and build the instrument: a theme for what becomes the items.
        "exploratory_sequential" => Template { variables: &[], codes: &["Themes for the instrument"], priority: "qualitative", how: "building" },
        // One strand sits inside the other: a variable for the group each case is in, and a theme for what the embedded strand found.
        "embedded" => Template { variables: &[("Study group", "nominal")], codes: &["Findings of the embedded strand"], priority: "quantitative", how: "embedding" },
        other => return Err(Error::Invalid(format!("'{other}' is not a design; the designs are {}", DESIGN_KINDS.join(", ")))),
    })
}

fn design_from(r: &rusqlite::Row) -> rusqlite::Result<MixedDesign> {
    Ok(MixedDesign { kind: r.get(0)?, priority: r.get(1)?, purpose: r.get(2)?, created_at: r.get(3)?, updated_at: r.get(4)? })
}

impl Project {
    /// The design of the study, once chosen.
    pub fn mixed_design(&self) -> Result<Option<MixedDesign>> {
        Ok(self.conn.query_row("SELECT kind, priority, purpose, created_at, updated_at FROM mixed_design WHERE id = 1", [], design_from).optional()?)
    }

    fn check_design(&self, input: &DesignInput) -> Result<(String, String, String)> {
        let template = template(&input.kind)?;
        let priority = input.priority.clone().unwrap_or_else(|| template.priority.to_string());
        one_of(&priority, &PRIORITIES, "which strand has priority")?;
        Ok((input.kind.clone(), priority, clean(&input.purpose, MAX_TEXT, "the purpose")?))
    }

    /// Chooses the design, or changes it. Nothing already made is touched.
    pub fn mixed_design_set(&mut self, input: &DesignInput) -> Result<MixedDesign> {
        self.ensure_writable()?;
        let (kind, priority, purpose) = self.check_design(input)?;
        let before = self.mixed_design()?;
        if let Some(b) = &before {
            if (&b.kind, &b.priority, &b.purpose) == (&kind, &priority, &purpose) {
                return Ok(b.clone());
            }
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO mixed_design (id, kind, priority, purpose, created_at, updated_at)
             VALUES (1, ?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT (id) DO UPDATE SET kind = ?1, priority = ?2, purpose = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![kind, priority, purpose],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "mixed.design.set",
            Some(("mixed_design", 1)),
            before.as_ref().map(|b| json!({ "kind": b.kind, "priority": b.priority })).as_ref(),
            Some(&json!({ "kind": kind, "priority": priority })),
        )?;
        tx.commit()?;
        Ok(self.mixed_design()?.expect("just written"))
    }

    /// Forgets the choice of design. The variables and codes a template made are ordinary ones and stay.
    pub fn mixed_design_clear(&mut self) -> Result<()> {
        self.ensure_writable()?;
        let Some(before) = self.mixed_design()? else { return Ok(()) };
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM mixed_design WHERE id = 1", [])?;
        event::insert(&tx, &self.actor, &self.session, "mixed.design.clear", Some(("mixed_design", 1)), Some(&json!({ "kind": before.kind, "priority": before.priority })), None)?;
        tx.commit()?;
        Ok(())
    }

    /// What applying the template of a design would make, and what is there already.
    pub fn design_plan(&self, kind: &str) -> Result<DesignPlan> {
        let t = template(kind)?;
        let data = self.case_data()?;
        let mut variables = Vec::new();
        for (name, level) in t.variables {
            variables.push(PlannedVariable { name: (*name).to_string(), level: (*level).to_string(), exists: data.variables.iter().any(|v| v.name == *name) });
        }
        let mut codes = Vec::new();
        for name in t.codes {
            codes.push(PlannedCode { name: (*name).to_string(), exists: self.active_code_named(name)?.is_some() });
        }
        Ok(DesignPlan { kind: kind.to_string(), priority: t.priority.to_string(), how: t.how.to_string(), variables, codes })
    }

    fn active_code_named(&self, name: &str) -> Result<Option<i64>> {
        Ok(self.conn.query_row("SELECT id FROM code WHERE name = ?1 AND status = 'active' LIMIT 1", [name], |r| r.get(0)).optional()?)
    }

    /// Chooses the design and sets up what its template needs: the variables and theme codes that are not in the project yet (one that is
    /// already there is left exactly as it is), and the first entry of the integration log. A second press makes nothing twice.
    pub fn design_apply(&mut self, apply: &DesignApply) -> Result<DesignOutcome> {
        self.ensure_writable()?;
        self.check_design(&apply.design)?;
        let plan = self.design_plan(&apply.design.kind)?;
        let t = template(&apply.design.kind)?;
        let mut out = DesignOutcome { design: self.mixed_design_set(&apply.design)?, variables_made: vec![], variables_kept: vec![], codes_made: vec![], codes_kept: vec![], entry_id: None };
        for v in &plan.variables {
            if v.exists {
                out.variables_kept.push(v.name.clone());
            } else {
                self.define_variable(&VariableSpec { name: v.name.clone(), level: v.level.clone(), value_labels: Default::default(), missing_codes: vec![] }, None)?;
                out.variables_made.push(v.name.clone());
            }
        }
        for c in &plan.codes {
            if c.exists {
                out.codes_kept.push(c.name.clone());
            } else {
                let mut code = NewCode::new(&c.name);
                code.theme = true;
                self.add_code(&code)?;
                out.codes_made.push(c.name.clone());
            }
        }
        let what = apply.log.trim();
        if !what.is_empty() {
            out.entry_id = Some(self.integration_add(&IntegrationInput { on_date: None, stage: "design".to_string(), how: t.how.to_string(), what: what.to_string(), why: out.design.purpose.clone() })?.id);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------- the triangulation matrix

#[derive(Debug, Clone, Deserialize)]
pub struct TriangulationInput {
    pub topic: String,
    #[serde(default)]
    pub code_id: Option<i64>,
    #[serde(default)]
    pub variable: String,
    #[serde(default)]
    pub qualitative: String,
    #[serde(default)]
    pub quantitative: String,
    #[serde(default)]
    pub mark: String,
    #[serde(default)]
    pub note: String,
}

/// The words behind a topic: what the cases that have its code say.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicWords {
    pub cases: usize,
    pub excerpts: usize,
    pub quotes: Vec<TopicQuote>,
    pub more: usize,
    pub coding_ids: Vec<i64>,
}

/// The numbers behind a topic: the variable for the cases that have its code and for the other cases that hold text (or, for a topic with
/// no code, for every case that holds text).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicNumbers {
    pub column: TypologyColumn,
    pub with: TypologyCell,
    pub without: Option<TypologyCell>,
    pub difference: Option<f64>,
    /// True when `with` describes every case that holds text, since the topic has no code.
    pub whole: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriangulationEvidence {
    pub words: Option<TopicWords>,
    pub numbers: Option<TopicNumbers>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriangulationRow {
    pub id: i64,
    pub position: i64,
    pub topic: String,
    pub code_id: Option<i64>,
    pub code_name: Option<String>,
    pub variable: String,
    pub qualitative: String,
    pub quantitative: String,
    pub mark: String,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
    /// The words and numbers worked out from the project now; none for a topic with neither a code nor a variable.
    pub evidence: Option<TriangulationEvidence>,
    /// Why part of the evidence is missing (the variable was removed or renamed, say).
    pub problem: Option<String>,
}

const ROW_COLUMNS: &str = "t.id, t.position, t.topic, t.code_id, c.name, t.variable, t.qualitative, t.quantitative, t.mark, t.note, t.created_at, t.updated_at";

fn row_from(r: &rusqlite::Row) -> rusqlite::Result<TriangulationRow> {
    Ok(TriangulationRow {
        id: r.get(0)?,
        position: r.get(1)?,
        topic: r.get(2)?,
        code_id: r.get(3)?,
        code_name: r.get(4)?,
        variable: r.get(5)?,
        qualitative: r.get(6)?,
        quantitative: r.get(7)?,
        mark: r.get(8)?,
        note: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        evidence: None,
        problem: None,
    })
}

impl Project {
    fn triangulation_checked(&self, input: &TriangulationInput) -> Result<(String, String, String, String)> {
        let topic = clean(&input.topic, MAX_TOPIC, "a topic")?;
        if topic.is_empty() {
            return Err(Error::Invalid("a topic needs a name".to_string()));
        }
        one_of(&input.mark, &MARKS, "the mark")?;
        if let Some(code) = input.code_id {
            self.code_name(code)?;
        }
        let variable = input.variable.trim().to_string();
        if !variable.is_empty() {
            let data = self.case_data()?;
            let def = data.variables.iter().find(|v| v.name == variable).ok_or_else(|| Error::Invalid(format!("no case variable is named '{variable}'")))?;
            if def.level == "text" {
                return Err(Error::Invalid(format!("'{variable}' holds free text, which cannot stand for numbers; choose another variable")));
            }
        }
        Ok((topic, variable, clean(&input.qualitative, MAX_TEXT, "what the qualitative strand says")?, clean(&input.quantitative, MAX_TEXT, "what the quantitative strand says")?))
    }

    /// The words and numbers behind one row, worked out from the project as it is now.
    fn triangulation_evidence(&self, code_id: Option<i64>, variable: &str) -> Result<(Option<TriangulationEvidence>, Option<String>)> {
        let mut problem = None;
        let mut usable = !variable.is_empty();
        if usable {
            let data = self.case_data()?;
            match data.variables.iter().find(|v| v.name == variable) {
                None => {
                    problem = Some(format!("The project has no variable named '{variable}' now, so there are no numbers to show."));
                    usable = false;
                }
                Some(def) if def.level == "text" => {
                    problem = Some(format!("'{variable}' holds free text now, so there are no numbers to show."));
                    usable = false;
                }
                Some(_) => {}
            }
        }
        match (code_id, usable) {
            (Some(code), _) => {
                let spec = SideBySideSpec { code_ids: vec![code], include_descendants: true, variables: if usable { vec![variable.to_string()] } else { vec![] }, quotes: 2 };
                let s = self.side_by_side_any(&spec)?;
                let topic = &s.topics[0];
                let words = TopicWords { cases: topic.with.cases.len(), excerpts: topic.excerpts, quotes: topic.quotes.clone(), more: topic.more, coding_ids: topic.coding_ids.clone() };
                let numbers = usable.then(|| TopicNumbers { column: s.columns[0].clone(), with: topic.with.cells[0].clone(), without: Some(topic.without.cells[0].clone()), difference: topic.differences[0], whole: false });
                Ok((Some(TriangulationEvidence { words: Some(words), numbers }), problem))
            }
            (None, true) => {
                let (column, cell, _) = self.describe_cases_with_text(variable)?;
                Ok((Some(TriangulationEvidence { words: None, numbers: Some(TopicNumbers { column, with: cell, without: None, difference: None, whole: true }) }), problem))
            }
            (None, false) => Ok((None, problem)),
        }
    }

    fn triangulation_row(&self, id: i64) -> Result<TriangulationRow> {
        let mut row = self
            .conn
            .query_row(&format!("SELECT {ROW_COLUMNS} FROM triangulation t LEFT JOIN code c ON c.id = t.code_id WHERE t.id = ?1"), [id], row_from)
            .optional()?
            .ok_or(Error::NotFound { what: "triangulation row", id })?;
        let (evidence, problem) = self.triangulation_evidence(row.code_id, &row.variable)?;
        row.evidence = evidence;
        row.problem = problem;
        Ok(row)
    }

    /// The rows of the triangulation matrix in order, each with the words and numbers behind it.
    pub fn triangulation(&self) -> Result<Vec<TriangulationRow>> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT id FROM triangulation ORDER BY position, id")?;
            let ids = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<Vec<_>, _>>()?;
            ids
        };
        ids.into_iter().map(|id| self.triangulation_row(id)).collect()
    }

    pub fn triangulation_add(&mut self, input: &TriangulationInput) -> Result<TriangulationRow> {
        self.ensure_writable()?;
        let (topic, variable, qualitative, quantitative) = self.triangulation_checked(input)?;
        let note = clean(&input.note, MAX_TEXT, "a note")?;
        let position: i64 = self.conn.query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM triangulation", [], |r| r.get(0))?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO triangulation (position, topic, code_id, variable, qualitative, quantitative, mark, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![position, topic, input.code_id, variable, qualitative, quantitative, input.mark, note],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "mixed.triangulation.add", Some(("triangulation", id)), None, Some(&json!({ "topic": topic, "mark": input.mark })))?;
        tx.commit()?;
        self.triangulation_row(id)
    }

    pub fn triangulation_update(&mut self, id: i64, input: &TriangulationInput) -> Result<TriangulationRow> {
        self.ensure_writable()?;
        let before = self.triangulation_row(id)?;
        let (topic, variable, qualitative, quantitative) = self.triangulation_checked(input)?;
        let note = clean(&input.note, MAX_TEXT, "a note")?;
        if (&topic, &input.code_id, &variable, &qualitative, &quantitative, &input.mark, &note) == (&before.topic, &before.code_id, &before.variable, &before.qualitative, &before.quantitative, &before.mark, &before.note) {
            return Ok(before);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE triangulation SET topic = ?1, code_id = ?2, variable = ?3, qualitative = ?4, quantitative = ?5, mark = ?6, note = ?7,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?8",
            params![topic, input.code_id, variable, qualitative, quantitative, input.mark, note, id],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "mixed.triangulation.update",
            Some(("triangulation", id)),
            Some(&json!({ "topic": before.topic, "mark": before.mark, "code_id": before.code_id, "variable": before.variable })),
            Some(&json!({ "topic": topic, "mark": input.mark, "code_id": input.code_id, "variable": variable })),
        )?;
        tx.commit()?;
        self.triangulation_row(id)
    }

    pub fn triangulation_remove(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let before = self.triangulation_row(id)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM triangulation WHERE id = ?1", [id])?;
        event::insert(&tx, &self.actor, &self.session, "mixed.triangulation.remove", Some(("triangulation", id)), Some(&json!({ "topic": before.topic, "mark": before.mark, "qualitative": before.qualitative, "quantitative": before.quantitative, "note": before.note })), None)?;
        tx.commit()?;
        Ok(())
    }

    /// Moves a row one place up or down.
    pub fn triangulation_move(&mut self, id: i64, up: bool) -> Result<()> {
        self.ensure_writable()?;
        let rows: Vec<(i64, i64)> = {
            let mut stmt = self.conn.prepare("SELECT id, position FROM triangulation ORDER BY position, id")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        let at = rows.iter().position(|(i, _)| *i == id).ok_or(Error::NotFound { what: "triangulation row", id })?;
        let other = if up { at.checked_sub(1) } else { Some(at + 1).filter(|n| *n < rows.len()) };
        let Some(other) = other else { return Ok(()) };
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE triangulation SET position = ?1 WHERE id = ?2", params![rows[other].1, rows[at].0])?;
        tx.execute("UPDATE triangulation SET position = ?1 WHERE id = ?2", params![rows[at].1, rows[other].0])?;
        tx.commit()?;
        Ok(())
    }
}

// ---------------------------------------------------------------- the integration log

#[derive(Debug, Clone, Deserialize)]
pub struct IntegrationInput {
    /// The day it was done as YYYY-MM-DD; today when absent.
    #[serde(default)]
    pub on_date: Option<String>,
    pub stage: String,
    pub how: String,
    pub what: String,
    #[serde(default)]
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntegrationEntry {
    pub id: i64,
    pub on_date: String,
    pub stage: String,
    pub how: String,
    pub what: String,
    pub why: String,
    pub actor: String,
    pub created_at: String,
    pub updated_at: String,
}

const ENTRY_COLUMNS: &str = "id, on_date, stage, how, what, why, actor, created_at, updated_at";

fn entry_from(r: &rusqlite::Row) -> rusqlite::Result<IntegrationEntry> {
    Ok(IntegrationEntry { id: r.get(0)?, on_date: r.get(1)?, stage: r.get(2)?, how: r.get(3)?, what: r.get(4)?, why: r.get(5)?, actor: r.get(6)?, created_at: r.get(7)?, updated_at: r.get(8)? })
}

impl Project {
    fn integration_checked(&self, input: &IntegrationInput) -> Result<(Option<String>, String, String)> {
        one_of(&input.stage, &STAGES, "the stage")?;
        one_of(&input.how, &HOWS, "how the two kinds of data were brought together")?;
        let what = clean(&input.what, MAX_TEXT, "what was done")?;
        if what.is_empty() {
            return Err(Error::Invalid("an entry needs to say what was done".to_string()));
        }
        let date = match input.on_date.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
            None => None,
            Some(d) => {
                if d.len() != 10 || crate::calendar::parse_day(d).is_none() {
                    return Err(Error::Invalid(format!("'{d}' is not a date; write it as year-month-day, for example 2026-11-08")));
                }
                Some(d.to_string())
            }
        };
        Ok((date, what, clean(&input.why, MAX_TEXT, "the reason")?))
    }

    /// The entries of the integration log, the oldest first.
    pub fn integration_log(&self) -> Result<Vec<IntegrationEntry>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {ENTRY_COLUMNS} FROM integration_entry ORDER BY on_date, id"))?;
        let entries = stmt.query_map([], entry_from)?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    fn integration_entry(&self, id: i64) -> Result<IntegrationEntry> {
        self.conn
            .query_row(&format!("SELECT {ENTRY_COLUMNS} FROM integration_entry WHERE id = ?1"), [id], entry_from)
            .optional()?
            .ok_or(Error::NotFound { what: "integration log entry", id })
    }

    pub fn integration_add(&mut self, input: &IntegrationInput) -> Result<IntegrationEntry> {
        self.ensure_writable()?;
        let (date, what, why) = self.integration_checked(input)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO integration_entry (on_date, stage, how, what, why, actor, created_at, updated_at)
             VALUES (COALESCE(?1, strftime('%Y-%m-%d', 'now')), ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![date, input.stage, input.how, what, why, self.actor],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "mixed.integration.add", Some(("integration_entry", id)), None, Some(&json!({ "stage": input.stage, "how": input.how })))?;
        tx.commit()?;
        self.integration_entry(id)
    }

    pub fn integration_update(&mut self, id: i64, input: &IntegrationInput) -> Result<IntegrationEntry> {
        self.ensure_writable()?;
        let before = self.integration_entry(id)?;
        let (date, what, why) = self.integration_checked(input)?;
        let date = date.unwrap_or_else(|| before.on_date.clone());
        if (&date, &input.stage, &input.how, &what, &why) == (&before.on_date, &before.stage, &before.how, &before.what, &before.why) {
            return Ok(before);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE integration_entry SET on_date = ?1, stage = ?2, how = ?3, what = ?4, why = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?6",
            params![date, input.stage, input.how, what, why, id],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "mixed.integration.update",
            Some(("integration_entry", id)),
            Some(&json!({ "on_date": before.on_date, "stage": before.stage, "how": before.how, "what": before.what, "why": before.why })),
            Some(&json!({ "on_date": date, "stage": input.stage, "how": input.how, "what": what, "why": why })),
        )?;
        tx.commit()?;
        self.integration_entry(id)
    }

    pub fn integration_remove(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let before = self.integration_entry(id)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM integration_entry WHERE id = ?1", [id])?;
        event::insert(&tx, &self.actor, &self.session, "mixed.integration.remove", Some(("integration_entry", id)), Some(&json!({ "on_date": before.on_date, "stage": before.stage, "how": before.how, "what": before.what, "why": before.why })), None)?;
        tx.commit()?;
        Ok(())
    }
}
