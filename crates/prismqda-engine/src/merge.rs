//! Merging a copy of a project into this one (specification 18.2). Coders who worked apart, each on a copy of the same project, bring
//! their work back together here: the documents the copy has and this project lacks are added, and the codings, areas, and memos of the
//! documents both have are combined, each kept with the coder who made it. Nothing this project holds is changed or removed.
//!
//! What the merge does not decide for the person it reports: a document with the same name but different words is left out, and a code
//! both have with a different definition keeps this project's. A coding already here (same words, code, and coder) is not added twice.
//! Cases are the same case when their names are, and a variable the same variable when its name is: the cases and variables the copy has
//! and this project lacks are added with the documents they hold and their values; a value this project already has is kept, and a
//! different one in the copy is reported.
//!
//! The copy is never touched: it is read from a scratch copy of its file. A preview is the same merge run on a scratch copy of this
//! project, so what it reports is exactly what would happen.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::event;
use crate::ops::sha256_hex;
use crate::project::{OpenOptions, Project};
use crate::{CellChange, CoderRole, Error, ImageAttachment, ImageData, ImportBatch, ImportedSource, ImportedUnit, MemoKind, MemoScope, NewCode, NewCoder, NewCoding, NewMemo, NewRegion, Origin, Result, SourceType, UnitKind, VariableSpec};

/// Which of the differences a merge finds the person settled in favour of the copy. Everything not named here keeps this project's.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MergeChoices {
    /// Values of cases where the copy's wins, by the name of the case and of the variable (case ignored).
    #[serde(default)]
    pub values_from_copy: Vec<ValueKey>,
    /// Codes whose definition the copy's wins, by their place in the codebook as a conflict lists it (case ignored).
    #[serde(default)]
    pub definitions_from_copy: Vec<String>,
}

/// A value of a case, by the names a person sees.
#[derive(Debug, Clone, Deserialize)]
pub struct ValueKey {
    pub case: String,
    pub variable: String,
}

impl MergeChoices {
    fn takes_value(&self, case: &str, variable: &str) -> bool {
        self.values_from_copy.iter().any(|k| k.case.eq_ignore_ascii_case(case) && k.variable.eq_ignore_ascii_case(variable))
    }

    fn takes_definition(&self, path: &str) -> bool {
        self.definitions_from_copy.iter().any(|p| p.eq_ignore_ascii_case(path))
    }
}

/// A value of a case that both projects have, and that differs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueConflict {
    pub case: String,
    pub variable: String,
    pub ours: String,
    pub theirs: String,
}

/// A code both projects have whose definitions differ.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeConflict {
    pub path: String,
    pub ours: String,
    pub theirs: String,
}

/// Codings left out for one reason.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Skipped {
    /// `code_retired` (the code was merged or retired in the copy), `code_unmatched`, `coder_unmatched`, `no_unit` (the words fit no unit of
    /// this project, which codes whole units), `exclusive_group` (a clash with a mutually exclusive group), or `refused` (anything else).
    pub reason: String,
    pub count: usize,
}

/// What a merge did, or (for a preview) would do.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MergeReport {
    /// Documents the copy has and this project lacks, added whole.
    pub documents_added: Vec<String>,
    /// Documents both have, with the same words.
    pub documents_matched: usize,
    /// Documents with the same name and different words, which are left out.
    pub documents_in_conflict: Vec<String>,
    pub coders_added: Vec<String>,
    /// The codes the copy has and this project lacks, by their place in the codebook.
    pub codes_added: Vec<String>,
    pub codes_in_conflict: Vec<CodeConflict>,
    pub codings_added: usize,
    pub codings_already_here: usize,
    /// Codings left out, each reason with how many.
    pub codings_skipped: Vec<Skipped>,
    pub areas_added: usize,
    pub areas_already_here: usize,
    /// Links between passages (decision 162) brought over, and ones this project already had.
    pub links_added: usize,
    pub links_already_here: usize,
    pub memos_added: usize,
    pub memos_already_here: usize,
    /// Cases the copy has and this project lacks, added; and cases both have (the same name).
    pub cases_added: Vec<String>,
    pub cases_matched: usize,
    /// Documents put in a case here because they are in it in the copy.
    pub case_members_added: usize,
    /// Variables of cases the copy has and this project lacks, added.
    pub variables_added: Vec<String>,
    /// Variables both have at another level, or whose values this project's variable cannot hold: their values are not brought.
    pub variables_in_conflict: Vec<String>,
    pub values_added: usize,
    /// Values both have that differ. This project's is kept.
    pub values_in_conflict: Vec<ValueConflict>,
    /// Values both have that differ and that the person chose to take from the copy (`MergeChoices`), and the codes whose wording was.
    pub values_replaced: usize,
    pub definitions_replaced: Vec<String>,
    /// Where the backup made before merging was saved; empty for a preview.
    pub backup: Option<String>,
    /// False for a preview.
    pub applied: bool,
}

/// A path for a scratch file that no other call uses.
pub(crate) fn scratch_path(what: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("prismqda-{what}-{}-{nanos}.pqda", std::process::id()))
}

/// Removes a scratch project and any files SQLite keeps beside it.
pub(crate) fn remove_scratch(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let _ = fs::remove_file(PathBuf::from(name));
    }
}

pub(crate) fn quiet() -> OpenOptions {
    let mut o = OpenOptions::new("merge");
    o.backup_on_open = false;
    o
}

/// The same, with the passphrase of an encrypted project (ignored for a project that is not encrypted).
pub(crate) fn quiet_with(passphrase: Option<&str>) -> OpenOptions {
    let mut o = quiet();
    o.passphrase = passphrase.map(str::to_string);
    o
}

/// What a merge takes from the other project. Everything, for a merge of coders' copies; a packet for a coder is the same merge run the other
/// way into a new, empty project, with only the chosen documents and (usually) none of the codings or memos.
pub(crate) struct MergeScope {
    /// The documents to take, by their id in the project they come from; `None` for all of them.
    pub(crate) sources: Option<HashSet<i64>>,
    /// Whether to bring the coders, the codings, and the coded areas.
    pub(crate) codings: bool,
    /// Whose coding to bring, by their id in the project it comes from; `None` for every coder. Only meaningful with `codings`.
    pub(crate) coders: Option<HashSet<i64>>,
    /// Whether to bring the memos.
    pub(crate) memos: bool,
    /// Whether to link the recordings of the documents that come (a recording is linked by where it is on a computer, so a packet leaves them out).
    pub(crate) recordings: bool,
    /// Whether to bring the cases, the variables of cases, and their values.
    pub(crate) cases: bool,
    /// The differences the person settled in favour of the copy.
    pub(crate) choices: MergeChoices,
}

impl MergeScope {
    pub(crate) fn all() -> Self {
        Self { sources: None, codings: true, coders: None, memos: true, recordings: true, cases: true, choices: MergeChoices::default() }
    }
}

fn note(reasons: &mut HashMap<String, usize>, why: &str) {
    *reasons.entry(why.to_string()).or_default() += 1;
}

impl Project {
    /// What merging the copy at `copy` would do, worked out on a scratch copy of this project that is thrown away. Nothing here changes.
    pub fn merge_preview(&self, copy: &Path) -> Result<MergeReport> {
        self.merge_preview_with(copy, None)
    }

    /// Like [`merge_preview`](Project::merge_preview), with the passphrase of the copy when it is encrypted. Without one, this project's own
    /// passphrase is tried (a team often shares one).
    pub fn merge_preview_with(&self, copy: &Path, their_passphrase: Option<&str>) -> Result<MergeReport> {
        self.merge_preview_choosing(copy, their_passphrase, &MergeChoices::default())
    }

    /// The same, with the differences the person settled in favour of the copy.
    pub fn merge_preview_choosing(&self, copy: &Path, their_passphrase: Option<&str>, choices: &MergeChoices) -> Result<MergeReport> {
        self.refuse_own_file(copy)?;
        let scratch = scratch_path("preview");
        let own = self.key.as_ref().map(|k| k.as_str().to_string());
        let result = (|| {
            // The scratch copy of this project is encrypted with its passphrase when the project is, so nothing readable is left in the temporary folder.
            self.copy_database(&scratch)?;
            let mut trial = Project::open(&scratch, &quiet_with(own.as_deref()))?;
            trial.merge_in(copy, their_passphrase, choices)
        })();
        remove_scratch(&scratch);
        let mut report = result?;
        report.applied = false;
        Ok(report)
    }

    /// Merges the copy at `copy` into this project, after saving a backup of this project.
    pub fn merge_copy(&mut self, copy: &Path) -> Result<MergeReport> {
        self.merge_copy_with(copy, None)
    }

    /// Like [`merge_copy`](Project::merge_copy), with the passphrase of the copy when it is encrypted.
    pub fn merge_copy_with(&mut self, copy: &Path, their_passphrase: Option<&str>) -> Result<MergeReport> {
        self.merge_copy_choosing(copy, their_passphrase, &MergeChoices::default())
    }

    /// The same, with the differences the person settled in favour of the copy.
    pub fn merge_copy_choosing(&mut self, copy: &Path, their_passphrase: Option<&str>, choices: &MergeChoices) -> Result<MergeReport> {
        self.ensure_writable()?;
        self.refuse_own_file(copy)?;
        let (backup, _) = self.backup_now_noted()?;
        let mut report = self.merge_in(copy, their_passphrase, choices)?;
        report.backup = Some(backup.display().to_string());
        report.applied = true;
        // One entry in the log says that a merge happened. It is also what stops Undo: a merge is too large to take back one step at a time.
        let tx = self.conn.transaction()?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "project.merge",
            None,
            None,
            Some(&json!({
                "from": copy.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                "documents_added": report.documents_added.len(),
                "coders_added": report.coders_added.len(),
                "codes_added": report.codes_added.len(),
                "codings_added": report.codings_added,
                "areas_added": report.areas_added,
                "links_added": report.links_added,
                "memos_added": report.memos_added,
                "values_replaced": report.values_replaced,
                "definitions_replaced": report.definitions_replaced.len(),
            })),
        )?;
        tx.commit()?;
        Ok(report)
    }

    /// A project cannot be merged into itself.
    fn refuse_own_file(&self, copy: &Path) -> Result<()> {
        if let (Ok(a), Ok(b)) = (fs::canonicalize(copy), fs::canonicalize(&self.path)) {
            if a == b {
                return Err(Error::Invalid("that file is this project itself; choose a copy of it".to_string()));
            }
        }
        Ok(())
    }

    /// The merge itself, applied to this project.
    fn merge_in(&mut self, copy: &Path, their_passphrase: Option<&str>, choices: &MergeChoices) -> Result<MergeReport> {
        if !copy.is_file() {
            return Err(Error::Invalid(format!("{} is not a file", copy.display())));
        }
        // The copy is read from a scratch copy of its file, so opening it (which may bring an older file up to date) never changes it.
        // If it is encrypted, the scratch copy is too (a file copy keeps its bytes), and it opens with the passphrase given, or else this
        // project's own.
        let pass = their_passphrase.map(str::to_string).or_else(|| self.key.as_ref().map(|k| k.as_str().to_string()));
        let scratch = scratch_path("copy");
        fs::copy(copy, &scratch)?;
        let result = (|| {
            let theirs = Project::open(&scratch, &quiet_with(pass.as_deref()))?;
            self.merge_from(&theirs, &MergeScope { choices: choices.clone(), ..MergeScope::all() })
        })();
        remove_scratch(&scratch);
        result
    }

    pub(crate) fn merge_from(&mut self, theirs: &Project, scope: &MergeScope) -> Result<MergeReport> {
        let mut report = MergeReport::default();

        // ---- coders: the same person is the same local id --------------------------------------------------------------------
        let mut coder_map: HashMap<i64, i64> = HashMap::new();
        let ours: HashMap<String, i64> = self.list_coders()?.into_iter().map(|c| (c.local_id, c.id)).collect();
        for c in theirs.list_coders()?.into_iter().filter(|c| scope.codings && scope.coders.as_ref().is_none_or(|only| only.contains(&c.id))) {
            if let Some(id) = ours.get(&c.local_id) {
                coder_map.insert(c.id, *id);
                continue;
            }
            let id = self.add_coder(&NewCoder {
                name: c.name.clone(),
                role: CoderRole::from_str(&c.role).unwrap_or(CoderRole::Human),
                local_id: Some(c.local_id.clone()),
                provider: c.provider.clone(),
                model_name: c.model_name.clone(),
                model_version: c.model_version.clone(),
            })?;
            coder_map.insert(c.id, id);
            report.coders_added.push(c.name);
        }

        // ---- codes: the same place in the codebook with the same name is the same code ---------------------------------------
        let mut code_map: HashMap<i64, i64> = HashMap::new();
        let mut inactive: HashSet<i64> = HashSet::new();
        let their_codes = theirs.list_codes()?;
        let by_id: HashMap<i64, &crate::CodeRow> = their_codes.iter().map(|c| (c.id, c)).collect();
        let path_of = |mut id: i64| -> String {
            let mut parts = Vec::new();
            while let Some(c) = by_id.get(&id) {
                parts.push(c.name.clone());
                match c.parent_id {
                    Some(p) if parts.len() < 64 => id = p,
                    _ => break,
                }
            }
            parts.reverse();
            parts.join(" > ")
        };
        let depth_of = |c: &crate::CodeRow| path_of(c.id).matches(" > ").count();
        let mut ordered: Vec<&crate::CodeRow> = their_codes.iter().collect();
        ordered.sort_by_key(|c| (depth_of(c), c.sort_order, c.id));
        let mut our_paths: HashMap<String, (i64, String)> = HashMap::new();
        {
            let mine = self.list_codes()?;
            let mine_by_id: HashMap<i64, &crate::CodeRow> = mine.iter().map(|c| (c.id, c)).collect();
            for c in mine.iter().filter(|c| c.status == "active") {
                let (mut parts, mut id) = (Vec::new(), c.id);
                while let Some(x) = mine_by_id.get(&id) {
                    parts.push(x.name.to_lowercase());
                    match x.parent_id {
                        Some(p) if parts.len() < 64 => id = p,
                        _ => break,
                    }
                }
                parts.reverse();
                our_paths.insert(parts.join(" > "), (c.id, c.definition.clone()));
            }
        }
        for c in ordered {
            if c.status != "active" {
                inactive.insert(c.id);
                continue;
            }
            let path = path_of(c.id);
            let key = path.to_lowercase();
            if let Some((id, definition)) = our_paths.get(&key) {
                code_map.insert(c.id, *id);
                if !c.definition.trim().is_empty() && c.definition.trim() != definition.trim() {
                    if scope.choices.takes_definition(&path) {
                        self.update_code(*id, &crate::CodeEdit { definition: Some(c.definition.clone()), ..Default::default() })?;
                        report.definitions_replaced.push(path);
                    } else {
                        report.codes_in_conflict.push(CodeConflict { path, ours: definition.clone(), theirs: c.definition.clone() });
                    }
                }
                continue;
            }
            // A parent that was merged or retired in the copy has no place here, so its code goes to the top.
            let parent = c.parent_id.and_then(|p| code_map.get(&p).copied());
            let id = self.add_code(&NewCode {
                name: c.name.clone(),
                parent_id: parent,
                theme: c.kind == "theme",
                color: Some(c.color.clone()),
                definition: c.definition.clone(),
                inclusion_criteria: c.inclusion_criteria.clone(),
                exclusion_criteria: c.exclusion_criteria.clone(),
                exclusive_group: c.exclusive_group.clone(),
                rated: c.rated,
                rating_label: c.rating_label.clone(),
                rating_low: c.rating_low.clone(),
                rating_high: c.rating_high.clone(),
            })?;
            // What keeps a code out of an AI packet comes with it.
            if c.do_not_share || !c.ai_suggestible {
                self.update_code(id, &crate::CodeEdit { do_not_share: Some(c.do_not_share), ai_suggestible: Some(c.ai_suggestible), ..Default::default() })?;
            }
            code_map.insert(c.id, id);
            our_paths.insert(key, (id, c.definition.clone()));
            report.codes_added.push(path);
        }

        // ---- documents --------------------------------------------------------------------------------------------------------
        let mut source_map: HashMap<i64, i64> = HashMap::new();
        let my_sources = self.list_sources()?;
        let mut text_docs: HashMap<(String, String), i64> = HashMap::new();
        let mut names: HashMap<String, i64> = HashMap::new();
        let mut picture_hashes: HashMap<String, i64> = HashMap::new();
        for s in &my_sources {
            if s.image.is_some() {
                let (_, bytes) = self.read_image_bytes(s.id)?;
                picture_hashes.insert(sha256_hex(&bytes), s.id);
            } else {
                text_docs.insert((s.name.to_lowercase(), sha256_hex(self.source_text(s.id)?.as_bytes())), s.id);
            }
            names.insert(s.name.to_lowercase(), s.id);
        }
        for s in theirs.list_sources()? {
            if scope.sources.as_ref().is_some_and(|only| !only.contains(&s.id)) {
                continue;
            }
            let text = theirs.source_text(s.id)?;
            if s.image.is_some() {
                let (mime, bytes) = theirs.read_image_bytes(s.id)?;
                if let Some(id) = picture_hashes.get(&sha256_hex(&bytes)) {
                    source_map.insert(s.id, *id);
                    report.documents_matched += 1;
                    continue;
                }
                if names.contains_key(&s.name.to_lowercase()) {
                    report.documents_in_conflict.push(s.name.clone());
                    continue;
                }
                let image = theirs.get_image(s.id)?.ok_or_else(|| Error::Invalid(format!("{} has no picture", s.name)))?;
                let data = ImageData { mime, file_name: image.file_name, width: image.width, height: image.height, bytes };
                let id = self.add_merged_document(theirs, &s, text, Some(data))?;
                source_map.insert(s.id, id);
                names.insert(s.name.to_lowercase(), id);
                report.documents_added.push(s.name);
                continue;
            }
            if let Some(id) = text_docs.get(&(s.name.to_lowercase(), sha256_hex(text.as_bytes()))) {
                source_map.insert(s.id, *id);
                report.documents_matched += 1;
                // Their copy keeps the PDF the words came from and ours does not: bring the file, so their coded areas have pages to be on.
                self.bring_pdf(theirs, s.id, *id);
                if scope.recordings {
                    self.bring_recording(theirs, s.id, *id);
                }
                continue;
            }
            if names.contains_key(&s.name.to_lowercase()) {
                report.documents_in_conflict.push(s.name.clone());
                continue;
            }
            let id = self.add_merged_document(theirs, &s, text, None)?;
            self.bring_pdf(theirs, s.id, id);
            if scope.recordings {
                self.bring_recording(theirs, s.id, id);
            }
            source_map.insert(s.id, id);
            names.insert(s.name.to_lowercase(), id);
            report.documents_added.push(s.name);
        }

        // ---- codings of words ------------------------------------------------------------------------------------------------
        let whole_units = self.info()?.coding_mode == "unit";
        let mut skipped: HashMap<String, usize> = HashMap::new();
        for (their_source, our_source) in source_map.iter().filter(|_| scope.codings) {
            let units = self.list_units(*our_source)?;
            let exact: HashMap<(i64, i64), i64> = units.iter().map(|u| ((u.start, u.end), u.id)).collect();
            let mut present: HashSet<(i64, i64, i64, i64)> = self.list_codings(*our_source)?.into_iter().filter(|c| c.layer == "coder").map(|c| (c.start, c.end, c.code_id, c.coder_id)).collect();
            for c in theirs.list_codings(*their_source)? {
                if c.layer != "coder" {
                    continue;
                }
                let Some(code) = code_map.get(&c.code_id) else {
                    note(&mut skipped, if inactive.contains(&c.code_id) { "code_retired" } else { "code_unmatched" });
                    continue;
                };
                let Some(coder) = coder_map.get(&c.coder_id) else {
                    note(&mut skipped, "coder_unmatched");
                    continue;
                };
                if present.contains(&(c.start, c.end, *code, *coder)) {
                    report.codings_already_here += 1;
                    continue;
                }
                let unit = exact.get(&(c.start, c.end)).copied().or_else(|| units.iter().find(|u| u.start <= c.start && u.end >= c.end && !u.excluded).map(|u| u.id));
                if whole_units && unit.is_none() {
                    note(&mut skipped, "no_unit");
                    continue;
                }
                let mut new = NewCoding::new(*our_source, c.start, c.end, *code, *coder);
                new.origin = Origin::from_str(&c.origin).unwrap_or(Origin::Human);
                new.rationale = c.rationale.clone();
                new.unit_id = unit;
                match self.apply_coding(&new) {
                    Ok(id) => {
                        if let Some(rating) = c.rating {
                            let _ = self.set_coding_rating(id, Some(rating));
                        }
                        present.insert((c.start, c.end, *code, *coder));
                        report.codings_added += 1;
                    }
                    Err(Error::ExclusiveGroup(_)) => note(&mut skipped, "exclusive_group"),
                    Err(_) => note(&mut skipped, "refused"),
                }
            }
        }
        let mut reasons: Vec<Skipped> = skipped.into_iter().map(|(reason, count)| Skipped { reason, count }).collect();
        reasons.sort_by(|a, b| b.count.cmp(&a.count).then(a.reason.cmp(&b.reason)));
        report.codings_skipped = reasons;

        // ---- areas of pictures ------------------------------------------------------------------------------------------------
        let mut area_keys: HashSet<(i64, i64, String, i64, i64)> = HashSet::new();
        for r in self.all_regions()? {
            if r.layer == "coder" {
                area_keys.insert((r.source_id, r.page, r.shape.canonical_json(), r.code_id, r.coder_id));
            }
        }
        for r in theirs.all_regions()?.into_iter().filter(|_| scope.codings) {
            let (Some(source), Some(code), Some(coder)) = (source_map.get(&r.source_id), code_map.get(&r.code_id), coder_map.get(&r.coder_id)) else { continue };
            if r.layer != "coder" {
                continue;
            }
            let key = (*source, r.page, r.shape.canonical_json(), *code, *coder);
            if area_keys.contains(&key) {
                report.areas_already_here += 1;
                continue;
            }
            let mut new = NewRegion::new(*source, r.shape.clone(), *code, *coder);
            new.page = r.page;
            new.rationale = r.rationale.clone();
            if self.apply_region(&new).is_ok() {
                area_keys.insert(key);
                report.areas_added += 1;
            }
        }

        // ---- links between passages -------------------------------------------------------------------------------------------
        if scope.codings {
            self.merge_passage_links(theirs, &source_map, &code_map, &coder_map, &mut report)?;
        }

        // ---- cases, the variables of cases, and their values ------------------------------------------------------------------
        if scope.cases {
            self.merge_cases(theirs, &source_map, &scope.choices, &mut report)?;
        }

        // ---- memos about the project, a document, or a code -------------------------------------------------------------------
        if !scope.memos {
            return Ok(report);
        }
        let mut memo_keys: HashSet<(String, String, Option<i64>, String, String)> =
            self.list_memos(None, None)?.into_iter().map(|m| (m.kind, m.scope, m.target_id, m.title, m.body)).collect();
        let mut memos = theirs.list_memos(None, None)?;
        memos.sort_by_key(|m| m.id);
        for m in memos {
            let target = match (m.scope.as_str(), m.target_id) {
                ("project" | "phase" | "standalone", _) => None,
                ("source", Some(id)) => match source_map.get(&id) {
                    Some(t) => Some(*t),
                    None => continue,
                },
                ("code", Some(id)) => match code_map.get(&id) {
                    Some(t) => Some(*t),
                    None => continue,
                },
                _ => continue,
            };
            let key = (m.kind.clone(), m.scope.clone(), target, m.title.clone(), m.body.clone());
            if memo_keys.contains(&key) {
                report.memos_already_here += 1;
                continue;
            }
            let (Ok(kind), Ok(scope)) = (MemoKind::from_str(&m.kind), MemoScope::from_str(&m.scope)) else { continue };
            if self.add_memo(&NewMemo { kind, scope, target_id: target, phase: m.phase.clone(), title: m.title.clone(), body: m.body.clone() }).is_ok() {
                memo_keys.insert(key);
                report.memos_added += 1;
            }
        }
        Ok(report)
    }

    /// Brings over the cases the copy has and this project lacks (a case is the same case when its name is, case ignored), the documents they
    /// hold, the variables of cases this project lacks, and their values. A value this project already has is kept; a different one in the
    /// copy is reported, with the case and the variable.
    fn merge_cases(&mut self, theirs: &Project, source_map: &HashMap<i64, i64>, choices: &MergeChoices, report: &mut MergeReport) -> Result<()> {
        let their_data = theirs.case_data()?;
        let their_cases = theirs.list_cases()?;
        let mut ours: HashMap<String, i64> = HashMap::new();
        let mut members: HashMap<i64, HashSet<i64>> = HashMap::new();
        for c in self.list_cases()? {
            ours.insert(c.name.to_lowercase(), c.id);
            members.insert(c.id, c.source_ids.into_iter().collect());
        }
        let mut case_map: HashMap<i64, i64> = HashMap::new();
        for c in &their_cases {
            let id = match ours.get(&c.name.to_lowercase()) {
                Some(id) => {
                    report.cases_matched += 1;
                    *id
                }
                None => {
                    let row = self.add_case(&c.name, &c.notes)?;
                    ours.insert(c.name.to_lowercase(), row.id);
                    members.insert(row.id, HashSet::new());
                    report.cases_added.push(c.name.clone());
                    row.id
                }
            };
            case_map.insert(c.id, id);
            for their_source in &c.source_ids {
                let Some(&our_source) = source_map.get(their_source) else { continue };
                if members.entry(id).or_default().insert(our_source) {
                    self.assign_source_to_case(id, our_source)?;
                    report.case_members_added += 1;
                }
            }
        }

        // the variables: the same name is the same variable, at the same level
        let mut mine: HashMap<String, String> = self.case_data()?.variables.into_iter().map(|v| (v.name.to_lowercase(), v.level)).collect();
        let mut usable: HashMap<String, String> = HashMap::new();
        for v in &their_data.variables {
            match mine.get(&v.name.to_lowercase()) {
                Some(level) if *level == v.level => {
                    usable.insert(v.name.clone(), v.name.clone());
                }
                Some(_) => report.variables_in_conflict.push(v.name.clone()),
                None => {
                    let made = self.define_variable(&VariableSpec { name: v.name.clone(), level: v.level.clone(), value_labels: v.value_labels.clone(), missing_codes: v.missing_codes.clone() }, None)?;
                    mine.insert(made.name.to_lowercase(), made.level.clone());
                    usable.insert(v.name.clone(), made.name);
                    report.variables_added.push(v.name.clone());
                }
            }
        }

        // the values
        let now = self.case_data()?;
        let row_of: HashMap<i64, usize> = now.cases.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
        let col_of: HashMap<String, usize> = now.variables.iter().enumerate().map(|(i, v)| (v.name.to_lowercase(), i)).collect();
        for (vi, v) in their_data.variables.iter().enumerate() {
            let Some(our_name) = usable.get(&v.name) else { continue };
            let Some(&col) = col_of.get(&our_name.to_lowercase()) else { continue };
            let mut changes: Vec<CellChange> = Vec::new();
            let mut replacing = 0usize;
            for (ri, c) in their_data.cases.iter().enumerate() {
                let Some(theirs_value) = their_data.values[ri][vi].clone() else { continue };
                let Some(&our_case) = case_map.get(&c.id) else { continue };
                let here = row_of.get(&our_case).and_then(|r| now.values[*r][col].clone());
                match here {
                    None => changes.push(CellChange { case_id: our_case, value: Some(theirs_value) }),
                    Some(ours_value) if ours_value == theirs_value => {}
                    Some(_) if choices.takes_value(&c.name, &v.name) => {
                        changes.push(CellChange { case_id: our_case, value: Some(theirs_value) });
                        replacing += 1;
                    }
                    Some(ours_value) => report.values_in_conflict.push(ValueConflict { case: c.name.clone(), variable: v.name.clone(), ours: ours_value, theirs: theirs_value }),
                }
            }
            if changes.is_empty() {
                continue;
            }
            match self.set_values(our_name, &changes) {
                Ok(n) => {
                    let replaced = replacing.min(n);
                    report.values_added += n - replaced;
                    report.values_replaced += replaced;
                }
                Err(_) => report.variables_in_conflict.push(v.name.clone()),
            }
        }
        Ok(())
    }

    /// Brings over the links between passages whose two ends are in both projects now. A passage is the same one in both when it is on the same
    /// words (or the same area) of the same document, has the same code, and was coded by the same person.
    fn merge_passage_links(&mut self, theirs: &Project, source_map: &HashMap<i64, i64>, code_map: &HashMap<i64, i64>, coder_map: &HashMap<i64, i64>, report: &mut MergeReport) -> Result<()> {
        #[derive(Hash, PartialEq, Eq)]
        enum Key {
            Words(i64, i64, i64, i64, i64),
            Area(i64, i64, String, i64, i64),
        }
        let mut ours: HashMap<Key, i64> = HashMap::new();
        {
            let mut stmt = self.conn.prepare("SELECT id, source_id, start_offset, end_offset, code_id, coder_id FROM coding WHERE layer = 'coder'")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?)))?;
            for row in rows {
                let (id, s, a, b, c, d) = row?;
                ours.insert(Key::Words(s, a, b, c, d), id);
            }
        }
        for r in self.all_regions()?.into_iter().filter(|r| r.layer == "coder") {
            ours.insert(Key::Area(r.source_id, r.page, r.shape.canonical_json(), r.code_id, r.coder_id), -r.id);
        }
        let their_regions: HashMap<i64, crate::RegionRow> = theirs.all_regions()?.into_iter().map(|r| (r.id, r)).collect();
        let key_of = |id: i64| -> Result<Option<Key>> {
            if id < 0 {
                let Some(r) = their_regions.get(&-id).filter(|r| r.layer == "coder") else { return Ok(None) };
                return Ok(match (source_map.get(&r.source_id), code_map.get(&r.code_id), coder_map.get(&r.coder_id)) {
                    (Some(s), Some(c), Some(d)) => Some(Key::Area(*s, r.page, r.shape.canonical_json(), *c, *d)),
                    _ => None,
                });
            }
            let row: Option<(i64, i64, i64, i64, i64, String)> = theirs
                .conn
                .query_row("SELECT source_id, start_offset, end_offset, code_id, coder_id, layer FROM coding WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
                .optional()?;
            Ok(match row {
                Some((s, a, b, c, d, layer)) if layer == "coder" => match (source_map.get(&s), code_map.get(&c), coder_map.get(&d)) {
                    (Some(s), Some(c), Some(d)) => Some(Key::Words(*s, a, b, *c, *d)),
                    _ => None,
                },
                _ => None,
            })
        };
        let links: Vec<(i64, i64, String)> = {
            let mut stmt = theirs.conn.prepare("SELECT from_id, to_id, label FROM link WHERE from_type = 'coding' AND to_type = 'coding' ORDER BY id")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for (from, to, label) in links {
            let (Some(a), Some(b)) = (key_of(from)?, key_of(to)?) else { continue };
            let (Some(&ours_a), Some(&ours_b)) = (ours.get(&a), ours.get(&b)) else { continue };
            let there: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM link WHERE from_type = 'coding' AND to_type = 'coding' AND from_id = ?1 AND to_id = ?2 AND lower(label) = lower(?3)",
                    params![ours_a, ours_b, label],
                    |r| r.get(0),
                )
                .optional()?;
            if there.is_some() {
                report.links_already_here += 1;
            } else if self.add_link("coding", ours_a, "coding", ours_b, &label).is_ok() {
                report.links_added += 1;
            }
        }
        Ok(())
    }

    /// When the copy keeps the PDF that one of its documents was read from and this project's matching document keeps none, keeps the
    /// file here too, so the areas coded on its pages have pages to be on. It says nothing when there is nothing to bring.
    fn bring_pdf(&mut self, theirs: &Project, their_id: i64, our_id: i64) {
        if self.get_pdf(our_id).ok().flatten().is_some() {
            return;
        }
        let Ok(Some(row)) = theirs.get_pdf(their_id) else { return };
        let Ok(bytes) = theirs.read_pdf_bytes(their_id) else { return };
        let pdf = crate::PdfData { file_name: row.file_name, pages: row.pages.iter().map(|p| (p.width, p.height)).collect(), bytes };
        let _ = self.attach_pdf(our_id, &pdf);
    }

    /// When the copy has a recording linked to one of its documents and this project's matching document has none, links the same
    /// recording here if the file is at the same place on this computer, so the stretches coded on it have a recording to be on. A
    /// recording that is not at that place is left unlinked (link it from where it is, and merge again to bring the stretches).
    fn bring_recording(&mut self, theirs: &Project, their_id: i64, our_id: i64) {
        if self.get_media(our_id).ok().flatten().is_some() {
            return;
        }
        let Ok(Some(row)) = theirs.get_media(their_id) else { return };
        let _ = self.link_media_as(our_id, &row.path, Some(row.stands_alone));
    }

    /// Adds a document of the copy to this project, with its units and (for a picture) its file. Its codings are brought over after.
    fn add_merged_document(&mut self, theirs: &Project, row: &crate::SourceRow, text: String, image: Option<ImageData>) -> Result<i64> {
        let mut units = Vec::new();
        for u in theirs.list_units(row.id)? {
            let kind = UnitKind::from_str(&u.kind).unwrap_or(UnitKind::Paragraph);
            let mut unit = ImportedUnit::new(u.start, u.end, kind);
            unit.speaker = u.speaker.clone();
            unit.excluded = u.excluded;
            units.push(unit);
        }
        let batch = ImportBatch {
            sources: vec![ImportedSource {
                name: row.name.clone(),
                source_type: SourceType::from_str(&row.source_type).unwrap_or(SourceType::Other),
                folder: row.folder.clone(),
                text,
                runs: vec![],
                units,
                default_unit_kind: UnitKind::from_str(&row.default_unit_kind).unwrap_or(UnitKind::Paragraph),
                unit_attributes: vec![],
                original_file_hash: None,
                settings: serde_json::Value::Null,
                warnings: vec![],
            }],
            images: image.map(|image| vec![ImageAttachment { source: 0, image }]).unwrap_or_default(),
            ..Default::default()
        };
        let id = self.apply_import(&batch)?.source_ids[0];
        // What keeps a document, or a passage of it, out of an AI packet comes with it.
        if row.do_not_share {
            self.set_source_do_not_share(id, true)?;
        }
        let flagged: Vec<(i64, i64)> = {
            let mut stmt = theirs.conn.prepare("SELECT start_offset, end_offset FROM unit WHERE source_id = ?1 AND do_not_share = 1")?;
            let rows = stmt.query_map([row.id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for (start, end) in flagged {
            self.conn.execute("UPDATE unit SET do_not_share = 1 WHERE source_id = ?1 AND start_offset = ?2 AND end_offset = ?3", params![id, start, end])?;
        }
        Ok(id)
    }
}
