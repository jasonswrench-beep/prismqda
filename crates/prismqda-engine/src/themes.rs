//! Theme development (decision 167): one place that shows every theme with the codes under it, what they hold, and the words that show it.
//!
//! A theme is a code that groups other codes (`code.kind = 'theme'`); the codes it groups are its children in the codebook tree, so moving a
//! code under a theme is the move that already exists. What this module adds is the view of the whole: for each theme its name, its
//! definition, where it stands (`theme_stage`: candidate, reviewed, defined), the codes under it with how many passages, documents, and cases
//! each has, and a few quotations (the passages marked as examples of its codes first, then the first passages of them); and the codes that are
//! under no theme yet, which are the ones left to place. It also makes a thematic map out of the themes with the concept map that already exists.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::explore::Scope;
use crate::segments::SegmentFilter;
use crate::{CodeRow, CodeSummary, MemoScope, Project, Result};

/// The most quotations shown for one theme.
pub const MAX_QUOTES: usize = 3;

/// A code that is under a theme, or under none, with how much it holds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeCode {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub parent_id: Option<i64>,
    pub passages: i64,
    pub documents: i64,
}

/// A passage that shows a theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeQuote {
    pub coding_id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    pub code_id: i64,
    pub code_name: String,
    pub text: String,
    /// True when the passage was marked as an example of its code; false when it is one of the first passages of the theme's codes.
    pub example: bool,
}

/// One theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeCard {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub parent_id: Option<i64>,
    pub definition: String,
    pub inclusion_criteria: String,
    pub exclusion_criteria: String,
    /// `candidate`, `reviewed`, or `defined`.
    pub stage: String,
    pub version: i64,
    /// The codes under the theme (and their sub-codes), not crossing into a theme inside it.
    pub codes: Vec<ThemeCode>,
    /// The themes inside this one.
    pub sub_themes: Vec<i64>,
    /// Passages coded with a code of the theme, each once, documents and cases they are in.
    pub passages: i64,
    pub documents: i64,
    pub cases: i64,
    pub quotes: Vec<ThemeQuote>,
    /// Memos written about the theme itself.
    pub memos: i64,
}

/// Every theme, and the codes that are in none.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeOverview {
    pub themes: Vec<ThemeCard>,
    /// Active codes that are not under any theme, still to be placed.
    pub unassigned: Vec<ThemeCode>,
}

impl Project {
    /// The themes of the codebook with what they hold, in the order of the tree. `scope` narrows what is counted and quoted (the working set).
    pub fn theme_overview(&self, scope: &Scope) -> Result<ThemeOverview> {
        let codes: Vec<CodeRow> = self.list_codes()?.into_iter().filter(|c| c.status == "active").collect();
        let by_id: HashMap<i64, &CodeRow> = codes.iter().map(|c| (c.id, c)).collect();
        let mut children: HashMap<Option<i64>, Vec<&CodeRow>> = HashMap::new();
        for c in &codes {
            // A code whose parent is not active hangs at the top.
            let parent = c.parent_id.filter(|p| by_id.contains_key(p));
            children.entry(parent).or_default().push(c);
        }
        let summaries: HashMap<i64, CodeSummary> = self.code_summaries(scope)?.into_iter().map(|s| (s.code_id, s)).collect();
        let to_code = |c: &CodeRow| {
            let s = summaries.get(&c.id);
            ThemeCode { id: c.id, name: c.name.clone(), color: c.color.clone(), parent_id: c.parent_id, passages: s.map_or(0, |s| s.assignments), documents: s.map_or(0, |s| s.documents) }
        };

        // the tree in order, to list the themes as the codebook does and to find the codes that are under none
        let mut order: Vec<&CodeRow> = Vec::new();
        let mut under_theme: HashSet<i64> = HashSet::new();
        fn walk<'a>(from: Option<i64>, in_theme: bool, children: &HashMap<Option<i64>, Vec<&'a CodeRow>>, order: &mut Vec<&'a CodeRow>, under: &mut HashSet<i64>) {
            for c in children.get(&from).into_iter().flatten() {
                order.push(c);
                let now = in_theme || c.kind == "theme";
                if now && c.kind != "theme" {
                    under.insert(c.id);
                }
                walk(Some(c.id), now, children, order, under);
            }
        }
        walk(None, false, &children, &mut order, &mut under_theme);

        let mut themes = Vec::new();
        for theme in order.iter().filter(|c| c.kind == "theme") {
            // the codes that belong to this theme: its descendants that are not themselves themes, not crossing into a theme inside it
            let mut members: Vec<&CodeRow> = Vec::new();
            let mut sub_themes: Vec<i64> = Vec::new();
            fn collect<'a>(from: i64, children: &HashMap<Option<i64>, Vec<&'a CodeRow>>, members: &mut Vec<&'a CodeRow>, subs: &mut Vec<i64>) {
                for c in children.get(&Some(from)).into_iter().flatten() {
                    if c.kind == "theme" {
                        subs.push(c.id);
                    } else {
                        members.push(c);
                        collect(c.id, children, members, subs);
                    }
                }
            }
            collect(theme.id, &children, &mut members, &mut sub_themes);
            let own = summaries.get(&theme.id);
            let member_ids: Vec<i64> = members.iter().map(|c| c.id).collect();

            // quotations: the passages marked as examples of these codes first, then the first passages of them
            let mut quotes: Vec<ThemeQuote> = Vec::new();
            let mut seen: HashSet<i64> = HashSet::new();
            let mut anchored: Vec<i64> = Vec::new();
            for id in &member_ids {
                anchored.extend(self.list_anchors(*id)?);
            }
            let in_scope = |filter: SegmentFilter| SegmentFilter { source_ids: scope.source_ids.clone(), coder_ids: scope.coder_ids.clone(), layer: scope.layer.clone(), ..filter };
            // a passage coded twice (two codes under the theme, or two coders) is one passage
            let passages = self
                .coded_segments(&in_scope(SegmentFilter { code_ids: Some(vec![theme.id]), ..Default::default() }))?
                .into_iter()
                .map(|s| (s.source_id, s.start, s.end, if s.kind == "text" { 0 } else { s.coding_id }))
                .collect::<HashSet<_>>()
                .len() as i64;
            if !anchored.is_empty() {
                for s in self.coded_segments(&in_scope(SegmentFilter { coding_ids: Some(anchored), ..Default::default() }))? {
                    if quotes.len() < MAX_QUOTES && s.kind == "text" && seen.insert(s.coding_id) {
                        quotes.push(ThemeQuote { coding_id: s.coding_id, source_id: s.source_id, source_name: s.source_name, start: s.start, end: s.end, code_id: s.code_id, code_name: s.code_name, text: s.text, example: true });
                    }
                }
            }
            if quotes.len() < MAX_QUOTES && !member_ids.is_empty() {
                for s in self.coded_segments(&in_scope(SegmentFilter { code_ids: Some(member_ids.clone()), exact_codes: true, ..Default::default() }))? {
                    if quotes.len() >= MAX_QUOTES {
                        break;
                    }
                    if s.kind == "text" && seen.insert(s.coding_id) {
                        quotes.push(ThemeQuote { coding_id: s.coding_id, source_id: s.source_id, source_name: s.source_name, start: s.start, end: s.end, code_id: s.code_id, code_name: s.code_name, text: s.text, example: false });
                    }
                }
            }
            themes.push(ThemeCard {
                id: theme.id,
                name: theme.name.clone(),
                color: theme.color.clone(),
                parent_id: theme.parent_id,
                definition: theme.definition.clone(),
                inclusion_criteria: theme.inclusion_criteria.clone(),
                exclusion_criteria: theme.exclusion_criteria.clone(),
                stage: theme.theme_stage.clone(),
                version: theme.version,
                codes: members.iter().map(|c| to_code(c)).collect(),
                sub_themes,
                passages,
                documents: own.map_or(0, |s| s.documents),
                cases: own.map_or(0, |s| s.cases),
                quotes,
                memos: self.list_memos(Some(MemoScope::Code), Some(theme.id))?.len() as i64,
            });
        }
        let mut unassigned: Vec<ThemeCode> = order.iter().filter(|c| c.kind != "theme" && !under_theme.contains(&c.id)).map(|c| to_code(c)).collect();
        unassigned.sort_by_key(|a| a.name.to_lowercase());
        Ok(ThemeOverview { themes, unassigned })
    }

    /// Makes a concept map of the themes: each theme and the codes under it as nodes, with the codebook's own links between them, laid out
    /// in layers. The map is the one the Concept Map tool shows, so it can be moved about and added to. Returns its number.
    pub fn make_thematic_map(&mut self) -> Result<i64> {
        self.ensure_writable()?;
        let codes: Vec<CodeRow> = self.list_codes()?.into_iter().filter(|c| c.status == "active").collect();
        let themes: Vec<&CodeRow> = codes.iter().filter(|c| c.kind == "theme").collect();
        if themes.is_empty() {
            return Err(crate::Error::Invalid("there is no theme yet, so there is nothing to draw; add a theme and move some codes under it".to_string()));
        }
        let mut ids: Vec<i64> = Vec::new();
        let mut stack: Vec<i64> = themes.iter().map(|t| t.id).collect();
        while let Some(id) = stack.pop() {
            if ids.contains(&id) {
                continue;
            }
            ids.push(id);
            stack.extend(codes.iter().filter(|c| c.parent_id == Some(id)).map(|c| c.id));
        }
        let mut name = "Thematic map".to_string();
        let mut map_id = None;
        for k in 1..=60 {
            let tried = if k == 1 { name.clone() } else { format!("Thematic map ({k})") };
            match self.create_concept_map(&tried) {
                Ok(id) => {
                    map_id = Some(id);
                    name = tried;
                    break;
                }
                Err(crate::Error::Invalid(m)) if m.contains("already") => continue,
                Err(e) => return Err(e),
            }
        }
        let map_id = map_id.ok_or_else(|| crate::Error::Invalid(format!("there are too many maps named like '{name}'")))?;
        self.add_codes_to_map(map_id, &ids, true, None)?;
        self.layout_concept_map(map_id, "layers")?;
        Ok(map_id)
    }
}
