//! Pseudonymizing and redacting through the command layer (decisions 138 and 156): the list of names to find, the program's suggestions,
//! a preview of what the list would do, the switch that applies it to everything exported, and the step that replaces the names in the
//! project for good (after a backup, with the key kept as a file if the researcher wants it).

use prismqda_engine::{DeidRule, DeidRulePatch, NewDeidRules};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{export_target, parse, to_json, write_file, Api, ApiError, ApiResult};

/// The columns of the key file, in order.
const KEY_HEADER: [&str; 6] = ["Original", "Replacement", "Kind", "Match case", "Whole word", "Note"];

#[derive(Deserialize)]
struct UpdateArgs {
    id: i64,
    #[serde(flatten)]
    patch: DeidRulePatch,
}

#[derive(Deserialize)]
struct IdsArgs {
    ids: Vec<i64>,
}

#[derive(Deserialize)]
struct OnArgs {
    on: bool,
}

#[derive(Deserialize)]
struct ScopeArgs {
    #[serde(default)]
    source_ids: Option<Vec<i64>>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct ApplyArgs {
    #[serde(default)]
    source_ids: Option<Vec<i64>>,
    /// Where to save the key (the list of names and what replaced them) before the list is removed from the project. Without it the
    /// names cannot be recovered except from the backup.
    #[serde(default)]
    key_path: Option<String>,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
struct CopyArgs {
    /// Where the copy goes; `.pqda` is added. It must not exist.
    path: String,
    /// Where to save the key before the copy is made (the copy does not hold the list).
    #[serde(default)]
    key_path: Option<String>,
    #[serde(default)]
    overwrite_key: bool,
}

#[derive(Deserialize)]
struct KeyArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
struct LoadArgs {
    path: String,
}

fn yes_no(b: bool) -> String {
    if b { "yes" } else { "no" }.to_string()
}

/// The key as CSV bytes. Not scrubbed: it is the list itself.
fn key_bytes(rules: &[DeidRule]) -> Vec<u8> {
    let header: Vec<String> = KEY_HEADER.iter().map(|s| s.to_string()).collect();
    let rows: Vec<Vec<String>> =
        rules.iter().map(|r| vec![r.term.clone(), r.replacement.clone(), r.kind.clone(), yes_no(r.match_case), yes_no(r.whole_word), r.note.clone()]).collect();
    prismqda_export::write_csv(&header, &rows)
}

/// A value read back from a key file: the apostrophe that the writer puts before text a spreadsheet would run as a formula is taken off.
fn unguard(s: &str) -> String {
    match s.strip_prefix('\'') {
        Some(rest) if rest.starts_with(['=', '@', '+', '-', '\t', '\r']) => rest.to_string(),
        _ => s.to_string(),
    }
}

impl Api {
    /// The commands for pseudonymizing and redacting; `None` for a command that is not one of them.
    pub(crate) fn call_deid(&mut self, cmd: &str, args: Value) -> Option<ApiResult> {
        Some(match cmd {
            "deid.state" => (|| to_json(&self.project()?.deid_state()?))(),
            "deid.add" => (|| {
                let a: NewDeidRules = parse(cmd, args)?;
                to_json(&self.project()?.add_deid_rules(&a)?)
            })(),
            "deid.update" => (|| {
                let a: UpdateArgs = parse(cmd, args)?;
                to_json(&self.project()?.update_deid_rule(a.id, &a.patch)?)
            })(),
            "deid.remove" => (|| {
                let a: IdsArgs = parse(cmd, args)?;
                let removed = self.project()?.remove_deid_rules(&a.ids)?;
                Ok(json!({ "removed": removed }))
            })(),
            "deid.set_exports" => (|| {
                let a: OnArgs = parse(cmd, args)?;
                self.project()?.set_deid_exports(a.on)?;
                to_json(&self.project()?.deid_state()?)
            })(),
            "deid.suggest" => (|| {
                let a: ScopeArgs = parse(cmd, args)?;
                let suggestions = self.project()?.deid_suggest(a.source_ids.as_deref(), a.limit.unwrap_or(150))?;
                Ok(json!({ "suggestions": suggestions }))
            })(),
            "deid.preview" => (|| {
                let a: ScopeArgs = parse(cmd, args)?;
                to_json(&self.project()?.deid_preview(a.source_ids.as_deref())?)
            })(),
            "deid.leftovers" => (|| {
                let places = self.project()?.deid_leftovers()?;
                Ok(json!({ "places": places }))
            })(),
            "deid.key" => (|| {
                let a: KeyArgs = parse(cmd, args)?;
                let rules = self.project()?.deid_rules()?;
                if rules.is_empty() {
                    return Err(ApiError::new("invalid", "the list is empty, so there is no key to save"));
                }
                let target = export_target(&a.path, "csv", a.overwrite)?;
                let mut saved = write_file(&target, &key_bytes(&rules))?;
                saved["rows"] = json!(rules.len());
                Ok(saved)
            })(),
            "deid.load_key" => (|| {
                let a: LoadArgs = parse(cmd, args)?;
                let table = crate::datatools::read_table(&a.path)?;
                let column = |name: &str| table.columns.iter().position(|c| c.name.trim().eq_ignore_ascii_case(name));
                let (Some(orig), Some(repl)) = (column("Original"), column("Replacement")) else {
                    return Err(ApiError::new("invalid", "this does not look like a key file: it needs the columns Original and Replacement"));
                };
                let (kind, case, whole, note) = (column("Kind"), column("Match case"), column("Whole word"), column("Note"));
                let get = |row: &Vec<prismqda_import::Cell>, i: Option<usize>| i.and_then(|i| row.get(i)).map(|c| unguard(c.text().trim())).unwrap_or_default();
                let flag = |s: &str, default: bool| match s.to_lowercase().as_str() {
                    "yes" | "true" | "1" => true,
                    "no" | "false" | "0" => false,
                    _ => default,
                };
                // Rows with the same replacement and settings are added together.
                let mut groups: Vec<NewDeidRules> = Vec::new();
                for row in &table.rows {
                    let (term, replacement) = (get(row, Some(orig)), get(row, Some(repl)));
                    if term.is_empty() || replacement.is_empty() {
                        continue;
                    }
                    let kind_text = get(row, kind);
                    let rule_kind = if kind_text == "redact" { "redact" } else { "pseudonym" }.to_string();
                    let (match_case, whole_word) = (flag(&get(row, case), false), flag(&get(row, whole), true));
                    let note_text = get(row, note);
                    match groups.iter_mut().find(|g| g.replacement == replacement && g.kind == rule_kind && g.match_case == match_case && g.whole_word == whole_word && g.note == note_text) {
                        Some(g) => g.terms.push(term),
                        None => groups.push(NewDeidRules { terms: vec![term], replacement, kind: rule_kind, match_case, whole_word, note: note_text }),
                    }
                }
                if groups.is_empty() {
                    return Err(ApiError::new("invalid", "the key file has no rows with both an original and a replacement"));
                }
                let (mut added, mut already) = (0usize, Vec::new());
                for g in groups {
                    let out = self.project()?.add_deid_rules(&g)?;
                    added += out.added.len();
                    already.extend(out.already_there);
                }
                Ok(json!({ "added": added, "already_there": already }))
            })(),
            "deid.apply" => (|| {
                let a: ApplyArgs = parse(cmd, args)?;
                let rules = self.project()?.deid_rules()?;
                if rules.is_empty() {
                    return Err(ApiError::new("invalid", "the list is empty, so there is nothing to replace; add the names to find first"));
                }
                // The key is written first, and nothing is changed if it cannot be: the names are about to leave the project.
                let key = match &a.key_path {
                    Some(path) if !path.trim().is_empty() => {
                        let target = export_target(path, "csv", a.overwrite)?;
                        write_file(&target, &key_bytes(&rules))?;
                        Some(target.display().to_string())
                    }
                    _ => None,
                };
                let outcome = self.project()?.deid_apply(a.source_ids.as_deref())?;
                let mut result = to_json(&outcome)?;
                result["key_path"] = json!(key);
                Ok(result)
            })(),
            "deid.copy" => (|| {
                let a: CopyArgs = parse(cmd, args)?;
                let rules = self.project()?.deid_rules()?;
                if rules.is_empty() {
                    return Err(ApiError::new("invalid", "the list is empty, so there is nothing to replace; add the names to find first"));
                }
                let target = export_target(&a.path, "pqda", false)?;
                // The key is written first: the copy does not hold the list, and the names are what the key is for.
                let key = match &a.key_path {
                    Some(path) if !path.trim().is_empty() => {
                        let key_target = export_target(path, "csv", a.overwrite_key)?;
                        write_file(&key_target, &key_bytes(&rules))?;
                        Some(key_target.display().to_string())
                    }
                    _ => None,
                };
                let made = self.project()?.deid_copy(&target)?;
                let mut result = to_json(&made)?;
                result["key_path"] = json!(key);
                Ok(result)
            })(),
            _ => return None,
        })
    }

    /// Opens the scope in which what is exported has the researcher's list applied to it, when the switch is on.
    pub(crate) fn scrub_scope(&mut self) -> Result<prismqda_export::ScrubGuard, ApiError> {
        let scrubber = self.project()?.export_scrubber()?;
        Ok(prismqda_export::scrub_scope(scrubber))
    }
}
