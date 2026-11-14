//! Method templates (specification 24.1): a method turned into phases, prompts, checklists, and
//! codebook rules, so the tool behaves the way the method expects.
//!
//! A template is a data file, not code. The built-in ones are in `templates/` and are compiled in.
//! A project follows one of them (or "free", which has no phases and no rules). The project's
//! progress through the phases is saved in the project and every move is logged.

use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::event;
use crate::ops::NewCode;
use crate::project::Project;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    /// `"unit"` when the method requires coding predefined units; absent when it does not care.
    pub coding_mode: Option<String>,
    pub require_definition: bool,
    pub require_inclusion_exclusion: bool,
    pub require_anchor: bool,
    /// `"shown"` or `"hidden"`: whether agreement tools appear by default.
    pub reliability_tools: String,
    /// `"off"` or `"allowed"`: the starting setting for AI coding.
    pub ai_coding: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Phase {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub tools: Vec<String>,
    pub memo_prompts: Vec<String>,
    pub exit_checklist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedCode {
    pub name: String,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub summary: String,
    /// The method's own words for things, such as "category" for "theme".
    pub vocabulary: BTreeMap<String, String>,
    pub rules: Rules,
    /// Codes the method starts with, added when asked.
    pub seed_codes: Vec<SeedCode>,
    pub notes: Vec<String>,
    pub phases: Vec<Phase>,
}

const BUILT_IN: [&str; 6] = [
    include_str!("../templates/free.json"),
    include_str!("../templates/reflexive_ta.json"),
    include_str!("../templates/content_analysis.json"),
    include_str!("../templates/grounded_theory.json"),
    include_str!("../templates/ipa.json"),
    include_str!("../templates/narrative.json"),
];

impl Template {
    /// Reads a template from its JSON text, as an instructor's copy would be shared.
    pub fn from_json(text: &str) -> Result<Template> {
        let t: Template = serde_json::from_str(text)?;
        t.validate()?;
        Ok(t)
    }

    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err(Error::Invalid("a method template needs an id and a name".to_string()));
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.phases {
            if !seen.insert(p.id.as_str()) {
                return Err(Error::Invalid(format!("the template has two phases called '{}'", p.id)));
            }
            if p.title.trim().is_empty() {
                return Err(Error::Invalid(format!("phase '{}' has no title", p.id)));
            }
        }
        if !matches!(self.rules.reliability_tools.as_str(), "shown" | "hidden") || !matches!(self.rules.ai_coding.as_str(), "off" | "allowed") {
            return Err(Error::Invalid("the template's rules use a setting this version does not know".to_string()));
        }
        Ok(())
    }
}

/// Every built-in template, free mode first.
pub fn built_in_templates() -> Vec<Template> {
    BUILT_IN.iter().map(|t| Template::from_json(t).expect("a built-in template is valid; a test checks them all")).collect()
}

pub fn find_template(id: &str) -> Option<Template> {
    built_in_templates().into_iter().find(|t| t.id == id)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemState {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseState {
    #[serde(flatten)]
    pub phase: Phase,
    pub items: Vec<ItemState>,
    /// `done` when every item is ticked, `current` for the phase being worked on, else `open`.
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemplateState {
    pub template: Template,
    pub current_phase: Option<String>,
    pub phases: Vec<PhaseState>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodebookIssue {
    pub code_id: Option<i64>,
    pub name: String,
    pub problem: String,
}

impl Project {
    fn template_id(&self) -> Result<String> {
        Ok(self.conn.query_row("SELECT method_template FROM project WHERE id = 1", [], |r| r.get(0))?)
    }

    /// The method the project follows.
    pub fn template(&self) -> Result<Template> {
        let id = self.template_id()?;
        find_template(&id).ok_or_else(|| Error::Invalid(format!("this project follows a method called '{id}' that this version does not have")))
    }

    /// Chooses the method. A method that codes whole units switches the project to unit mode, which
    /// is only allowed while the project has no free-span codings. With `seed_codes`, the method's
    /// starting codes are added (those already in the codebook are left alone).
    pub fn set_template(&mut self, id: &str, seed_codes: bool) -> Result<TemplateState> {
        self.ensure_writable()?;
        let t = find_template(id).ok_or_else(|| Error::Invalid(format!("there is no method template called '{id}'")))?;
        let (old, mode): (String, String) =
            self.conn.query_row("SELECT method_template, coding_mode FROM project WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let new_mode = match t.rules.coding_mode.as_deref() {
            Some("unit") if mode != "unit" => {
                let codings: i64 = self.conn.query_row("SELECT COUNT(*) FROM coding", [], |r| r.get(0))?;
                if codings > 0 {
                    return Err(Error::Invalid(format!(
                        "{} codes whole units, but this project already has {codings} free-span codings; start a new project to use this method",
                        t.name
                    )));
                }
                "unit".to_string()
            }
            _ => mode.clone(),
        };
        {
            let tx = self.conn.transaction()?;
            tx.execute("UPDATE project SET method_template = ?1, coding_mode = ?2 WHERE id = 1", params![t.id, new_mode])?;
            event::insert(
                &tx,
                &self.actor,
                &self.session,
                "template.set",
                Some(("project", 1)),
                Some(&json!({ "method_template": old, "coding_mode": mode })),
                Some(&json!({ "method_template": t.id, "coding_mode": new_mode, "seed_codes": seed_codes })),
            )?;
            tx.commit()?;
        }
        if seed_codes {
            let existing: Vec<String> = self.list_codes()?.into_iter().filter(|c| c.parent_id.is_none() && c.status == "active").map(|c| c.name.to_lowercase()).collect();
            for seed in &t.seed_codes {
                if !existing.contains(&seed.name.to_lowercase()) {
                    let mut code = NewCode::new(seed.name.clone());
                    code.definition = seed.definition.clone();
                    self.add_code(&code)?;
                }
            }
        }
        self.template_state()
    }

    /// The method, the phase the person is in, and every phase with its checklist.
    pub fn template_state(&self) -> Result<TemplateState> {
        let template = self.template()?;
        let ticked: std::collections::HashSet<(String, i64)> = {
            let mut stmt = self.conn.prepare("SELECT phase_id, item_index FROM phase_progress WHERE done = 1")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let moved: Option<String> = self
            .conn
            .query_row("SELECT json_extract(after_json, '$.phase') FROM event WHERE action = 'phase.move' ORDER BY id DESC LIMIT 1", [], |r| r.get(0))
            .optional()?
            .flatten();
        let current = moved.filter(|p| template.phases.iter().any(|ph| &ph.id == p)).or_else(|| template.phases.first().map(|p| p.id.clone()));
        let phases = template
            .phases
            .iter()
            .map(|p| {
                let items: Vec<ItemState> =
                    p.exit_checklist.iter().enumerate().map(|(i, text)| ItemState { text: text.clone(), done: ticked.contains(&(p.id.clone(), i as i64)) }).collect();
                let all = !items.is_empty() && items.iter().all(|i| i.done);
                let status = if all { "done" } else if current.as_deref() == Some(p.id.as_str()) { "current" } else { "open" };
                PhaseState { phase: p.clone(), items, status: status.to_string() }
            })
            .collect();
        Ok(TemplateState { template, current_phase: current, phases })
    }

    /// Ticks or unticks one item of a phase's exit checklist.
    pub fn tick_item(&mut self, phase_id: &str, item: usize, done: bool) -> Result<()> {
        self.ensure_writable()?;
        let template = self.template()?;
        let phase = template.phases.iter().find(|p| p.id == phase_id).ok_or_else(|| Error::Invalid(format!("this method has no phase called '{phase_id}'")))?;
        if item >= phase.exit_checklist.len() {
            return Err(Error::Invalid(format!("phase '{phase_id}' has only {} checklist items", phase.exit_checklist.len())));
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO phase_progress (phase_id, item_index, done, done_at) VALUES (?1, ?2, ?3, CASE WHEN ?3 = 1 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') END)
             ON CONFLICT (phase_id, item_index) DO UPDATE SET done = excluded.done, done_at = excluded.done_at",
            params![phase_id, item as i64, done as i64],
        )?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "phase.tick",
            None,
            None,
            Some(&json!({ "phase": phase_id, "item": item, "text": phase.exit_checklist[item], "done": done })),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Moves to a phase. Phases can be revisited; each move is logged (specification 24.1).
    pub fn move_to_phase(&mut self, phase_id: &str) -> Result<()> {
        self.ensure_writable()?;
        let template = self.template()?;
        if !template.phases.iter().any(|p| p.id == phase_id) {
            return Err(Error::Invalid(format!("this method has no phase called '{phase_id}'")));
        }
        let from = self.template_state()?.current_phase;
        let tx = self.conn.transaction()?;
        event::insert(&tx, &self.actor, &self.session, "phase.move", None, Some(&json!({ "phase": from })), Some(&json!({ "phase": phase_id })))?;
        tx.commit()?;
        Ok(())
    }

    /// Checks the codebook against the method's rules and lists what is missing, so the person can fix it
    /// before moving on. It never blocks anything by itself.
    pub fn check_codebook(&self) -> Result<Vec<CodebookIssue>> {
        let template = self.template()?;
        let mut issues = Vec::new();
        let mode: String = self.conn.query_row("SELECT coding_mode FROM project WHERE id = 1", [], |r| r.get(0))?;
        if template.rules.coding_mode.as_deref() == Some("unit") && mode != "unit" {
            issues.push(CodebookIssue { code_id: None, name: template.name.clone(), problem: "this method codes whole units, but the project codes free spans".to_string() });
        }
        let r = &template.rules;
        if !(r.require_definition || r.require_inclusion_exclusion || r.require_anchor) {
            return Ok(issues);
        }
        for c in self.list_codes()?.into_iter().filter(|c| c.status == "active" && c.kind == "code") {
            let mut missing = |p: &str| issues.push(CodebookIssue { code_id: Some(c.id), name: c.name.clone(), problem: p.to_string() });
            if r.require_definition && c.definition.trim().is_empty() {
                missing("has no definition");
            }
            if r.require_inclusion_exclusion {
                if c.inclusion_criteria.trim().is_empty() {
                    missing("has no inclusion criteria");
                }
                if c.exclusion_criteria.trim().is_empty() {
                    missing("has no exclusion criteria");
                }
            }
            if r.require_anchor && self.list_anchors(c.id)?.is_empty() {
                missing("has no anchor example");
            }
        }
        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_template_is_valid_and_has_a_unique_id() {
        let all = built_in_templates();
        assert_eq!(all.len(), 6);
        let ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["free", "reflexive_ta", "content_analysis", "grounded_theory", "ipa", "narrative"]);
        for t in &all {
            assert!(!t.summary.is_empty(), "{}", t.id);
            for p in &t.phases {
                assert!(!p.exit_checklist.is_empty() && !p.goal.is_empty(), "{} / {}", t.id, p.id);
            }
        }
        assert!(all[0].phases.is_empty(), "free mode has no phases");
    }

    #[test]
    fn the_method_rules_follow_the_specification() {
        let rta = find_template("reflexive_ta").unwrap();
        assert_eq!(rta.phases.len(), 6);
        assert_eq!((rta.rules.reliability_tools.as_str(), rta.rules.ai_coding.as_str()), ("hidden", "off"));
        let ca = find_template("content_analysis").unwrap();
        assert_eq!(ca.phases.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["codebook_development", "pilot_coding", "agreement_check", "codebook_revision", "full_coding", "frequency_analysis"]);
        assert!(ca.rules.require_definition && ca.rules.require_inclusion_exclusion && ca.rules.require_anchor);
        assert_eq!(ca.rules.coding_mode.as_deref(), Some("unit"));
        let narrative = find_template("narrative").unwrap();
        assert_eq!(narrative.seed_codes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Abstract", "Orientation", "Complicating action", "Evaluation", "Resolution", "Coda"]);
        let ipa = find_template("ipa").unwrap();
        assert_eq!(ipa.seed_codes.len(), 3, "descriptive, linguistic, and conceptual notes");
        assert_eq!(find_template("grounded_theory").unwrap().phases.len(), 5);
    }

    #[test]
    fn a_shared_template_is_checked_before_it_is_accepted() {
        assert!(Template::from_json("{}").is_err());
        let mut t = find_template("reflexive_ta").unwrap();
        t.id = "mine".into();
        let text = serde_json::to_string(&t).unwrap();
        assert_eq!(Template::from_json(&text).unwrap().id, "mine");
        t.phases[1].id = t.phases[0].id.clone();
        assert!(Template::from_json(&serde_json::to_string(&t).unwrap()).unwrap_err().to_string().contains("two phases"));
        let mut t = find_template("free").unwrap();
        t.rules.ai_coding = "sometimes".into();
        assert!(Template::from_json(&serde_json::to_string(&t).unwrap()).is_err());
    }
}
