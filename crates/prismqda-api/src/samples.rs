//! Bundled sample projects: small made-up studies a researcher can make an editable copy of.
//!
//! A sample is a JSON file in `crates/prismqda-api/samples/`, embedded in the program, so the original can
//! never be changed. Making a copy builds a new, ordinary project file with the same engine operations a
//! person would use (import, cases, codes, codings, ratings, memos), then reopens it so the copy starts a
//! fresh session: Undo then has nothing to take apart. The copy remembers which sample it came from in
//! its creation event, which needs no extra table and no new format version.
//!
//! To add a sample, put its JSON in `samples/` and add one line to [`SAMPLES`]. The tests build every
//! entry, so a phrase that does not match its document is caught when the program is tested.
//!
//! Everything that can be checked without a file is checked first: the whole sample is planned in memory
//! (documents imported, phrases located, names resolved) and nothing is written until the plan holds. If
//! anything still fails once the file exists, the half-made file is deleted.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use prismqda_engine::{CodingMode, ImportBatch, MemoKind, MemoScope, NewCode, NewCoding, NewMemo, NewProject, OpenOptions, Project};
use prismqda_import::text::parse_txt;
use prismqda_import::{parsed_to_batch, ImportOptions};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{actor_local_id, ApiError};

/// Every bundled sample as (id, JSON). Adding a sample is one more line here.
pub(crate) const SAMPLES: &[(&str, &str)] = &[("urban-mobility", include_str!("../samples/urban-mobility.json"))];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleDef {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    /// The research question the project is made with (shown on the dashboard); empty when there is none.
    #[serde(default)]
    research_question: String,
    /// `free_span` (the default) or `unit`.
    #[serde(default)]
    coding_mode: Option<CodingMode>,
    #[serde(default)]
    cases: Vec<CaseDef>,
    documents: Vec<DocumentDef>,
    #[serde(default)]
    codes: Vec<CodeDef>,
    #[serde(default)]
    codings: Vec<CodingDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseDef {
    name: String,
    /// Document names.
    #[serde(default)]
    documents: Vec<String>,
}

/// Plain text, imported the way a `.txt` file is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentDef {
    name: String,
    #[serde(default)]
    folder: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeDef {
    name: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    definition: String,
    #[serde(default)]
    theme: bool,
    /// A code whose codings can be rated from 1 to 5; the label says what the rating measures and the two
    /// ends say what a 1 and a 5 mean.
    #[serde(default)]
    rated: bool,
    #[serde(default)]
    rating_label: String,
    #[serde(default)]
    rating_low: String,
    #[serde(default)]
    rating_high: String,
    /// A mutually exclusive group the code belongs to.
    #[serde(default)]
    exclusive_group: String,
    #[serde(default)]
    children: Vec<CodeDef>,
}

/// One coding: the exact words in a document, the code, and optionally a note, a rating, and a memo.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodingDef {
    /// A document name.
    document: String,
    /// Must occur exactly once in the document's text.
    phrase: String,
    /// A code name. Code names are unique across the whole sample.
    code: String,
    /// Kept as the coding's note (its rationale).
    #[serde(default)]
    note: String,
    /// 1 to 5. The code must be a rated code.
    #[serde(default)]
    rating: Option<i64>,
    /// Marks the coding as an anchor example of its own code.
    #[serde(default)]
    example: bool,
    /// Marks the coding as a counterexample of this other code (a code name): words that look as if they
    /// belong to that code but do not.
    #[serde(default)]
    counterexample_of: Option<String>,
    #[serde(default)]
    memo: Option<MemoDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemoDef {
    title: String,
    #[serde(default)]
    body: String,
}

fn parse(id: &str, json: &str) -> Result<SampleDef, ApiError> {
    let def: SampleDef = serde_json::from_str(json).map_err(|e| ApiError::new("json", format!("the bundled sample '{id}' is damaged: {e}")))?;
    if def.id != id {
        return Err(ApiError::new("json", format!("the bundled sample '{id}' says its id is '{}'", def.id)));
    }
    Ok(def)
}

fn count_codes(codes: &[CodeDef]) -> usize {
    codes.iter().map(|c| 1 + count_codes(&c.children)).sum()
}

/// `sample.list`: every bundled sample and how big it is.
pub(crate) fn list() -> Result<Value, ApiError> {
    let mut out = Vec::new();
    for (id, json) in SAMPLES {
        let def = parse(id, json)?;
        out.push(json!({
            "id": def.id,
            "name": def.name,
            "description": def.description,
            "documents": def.documents.len(),
            "codes": count_codes(&def.codes),
            "codings": def.codings.len(),
        }));
    }
    Ok(Value::Array(out))
}

/// `sample.create`: makes an editable copy of a bundled sample at `path` and returns it, open and fresh.
/// An existing file at `path` is refused and left alone. On any other failure no file is left behind.
pub(crate) fn create(id: &str, path: &str, opts: &OpenOptions) -> Result<Project, ApiError> {
    let json = SAMPLES
        .iter()
        .find(|(sample_id, _)| *sample_id == id)
        .map(|(_, json)| *json)
        .ok_or_else(|| ApiError::new("not_found", format!("there is no sample named '{id}'")))?;
    let def = parse(id, json)?;
    if path.trim().is_empty() {
        return Err(ApiError::new("invalid", "say where to save the new project"));
    }
    build(&def, Path::new(path), opts)
}

fn build(def: &SampleDef, path: &Path, opts: &OpenOptions) -> Result<Project, ApiError> {
    let plan = plan(def)?;
    let mut new = NewProject::new(def.name.clone());
    new.coding_mode = def.coding_mode.unwrap_or(CodingMode::FreeSpan);
    new.sample_id = Some(def.id.clone());
    new.research_questions = def.research_question.clone();
    // A file that is already there is refused here, before anything is written, and is not touched.
    let mut project = Project::create(path, &new, opts)?;
    let filled = fill(&mut project, def, &plan, &opts.actor);
    // The connection must be closed before the file can be reopened or, on Windows, deleted.
    drop(project);
    filled
        .and_then(|()| {
            let mut again = opts.clone();
            again.backup_on_open = false;
            Project::open(path, &again).map_err(ApiError::from)
        })
        .inspect_err(|_| remove_project_files(path))
}

/// Deletes a project file and the `-wal` and `-shm` files SQLite keeps beside it.
fn remove_project_files(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let _ = fs::remove_file(PathBuf::from(name));
    }
}

// ---- planning: everything that can be checked before a file exists ----------------------------------

struct Plan<'a> {
    /// One source per document, in the order the sample lists them.
    batch: ImportBatch,
    /// Case name and the indexes of its documents.
    cases: Vec<(&'a str, Vec<usize>)>,
    /// Every code, parents before their children.
    codes: Vec<PlannedCode<'a>>,
    codings: Vec<PlannedCoding<'a>>,
}

struct PlannedCode<'a> {
    def: &'a CodeDef,
    /// Index into `Plan::codes`.
    parent: Option<usize>,
}

struct PlannedCoding<'a> {
    def: &'a CodingDef,
    document: usize,
    /// Index into `Plan::codes`.
    code: usize,
    /// Unicode scalar offsets into the stored text of the document.
    start: i64,
    end: i64,
}

fn flatten<'a>(codes: &'a [CodeDef], parent: Option<usize>, out: &mut Vec<PlannedCode<'a>>) {
    for def in codes {
        let index = out.len();
        out.push(PlannedCode { def, parent });
        flatten(&def.children, Some(index), out);
    }
}

/// Where a phrase is in a text.
enum Found {
    Once(i64, i64),
    Never,
    MoreThanOnce,
}

/// Finds a non-empty phrase in a text and reports its span in Unicode scalar values, which is what every
/// offset in a project counts, not bytes. A phrase that occurs again, even overlapping itself, is ambiguous.
fn locate(text: &str, phrase: &str) -> Found {
    let Some(first) = text.find(phrase) else { return Found::Never };
    let step = phrase.chars().next().map_or(1, char::len_utf8);
    if text[first + step..].contains(phrase) {
        return Found::MoreThanOnce;
    }
    let start = text[..first].chars().count() as i64;
    Found::Once(start, start + phrase.chars().count() as i64)
}

/// A phrase in quotes for an error message, cut short when it is long.
fn quoted(phrase: &str) -> String {
    const SHOWN: usize = 100;
    if phrase.chars().count() <= SHOWN {
        format!("\"{phrase}\"")
    } else {
        format!("\"{}...\"", phrase.chars().take(SHOWN).collect::<String>())
    }
}

fn plan(def: &SampleDef) -> Result<Plan<'_>, ApiError> {
    let bad = |what: String| ApiError::new("invalid", format!("sample '{}': {what}", def.id));

    // Documents go through the plain-text importer, so they get the same units a .txt file would.
    let mut document_index: HashMap<&str, usize> = HashMap::new();
    let mut batch = ImportBatch::default();
    for (i, doc) in def.documents.iter().enumerate() {
        if document_index.insert(doc.name.as_str(), i).is_some() {
            return Err(bad(format!("two documents are named '{}'", doc.name)));
        }
        let options = ImportOptions { name: Some(doc.name.clone()), folder: doc.folder.clone(), ..ImportOptions::default() };
        let one = parse_txt(doc.text.as_bytes(), &options)
            .and_then(|parsed| parsed_to_batch(&doc.name, parsed, &options))
            .map_err(|e| bad(format!("the document '{}' cannot be imported: {e}", doc.name)))?;
        batch.sources.extend(one.sources);
    }

    let mut cases = Vec::new();
    for case in &def.cases {
        let mut members = Vec::new();
        for name in &case.documents {
            let index = document_index
                .get(name.as_str())
                .ok_or_else(|| bad(format!("the case '{}' lists '{name}', which is not one of the documents", case.name)))?;
            members.push(*index);
        }
        cases.push((case.name.as_str(), members));
    }

    let mut codes = Vec::new();
    flatten(&def.codes, None, &mut codes);
    let mut code_index: HashMap<&str, usize> = HashMap::new();
    for (i, code) in codes.iter().enumerate() {
        if code_index.insert(code.def.name.as_str(), i).is_some() {
            return Err(bad(format!("two codes are named '{}', and codings find their code by name", code.def.name)));
        }
    }

    // The engine stays the authority on its own rules (a name that differs only in capitals, a bad color);
    // only what is needed to find things is checked here.
    let mut codings = Vec::new();
    for (n, coding) in def.codings.iter().enumerate() {
        let n = n + 1;
        let document = *document_index
            .get(coding.document.as_str())
            .ok_or_else(|| bad(format!("coding {n} names the document '{}', which is not one of the documents", coding.document)))?;
        let code = *code_index
            .get(coding.code.as_str())
            .ok_or_else(|| bad(format!("coding {n} names the code '{}', which is not in the codebook", coding.code)))?;
        if coding.phrase.is_empty() {
            return Err(bad(format!("coding {n} in '{}' has no phrase", coding.document)));
        }
        if let Some(other) = &coding.counterexample_of {
            let other_index = *code_index
                .get(other.as_str())
                .ok_or_else(|| bad(format!("coding {n} is a counterexample of '{other}', which is not in the codebook")))?;
            if other_index == code {
                return Err(bad(format!("coding {n} is coded '{other}' and cannot also be its counterexample")));
            }
        }
        if let Some(r) = coding.rating {
            if !(1..=5).contains(&r) {
                return Err(bad(format!("the rating for {} in '{}' is {r}, but a rating is a whole number from 1 to 5", quoted(&coding.phrase), coding.document)));
            }
        }
        if coding.rating.is_some() && !codes[code].def.rated {
            return Err(bad(format!("coding {n} has a rating, but the code '{}' is not rated: set \"rated\": true on it", coding.code)));
        }
        let (start, end) = match locate(&batch.sources[document].text, &coding.phrase) {
            Found::Once(start, end) => (start, end),
            Found::Never => {
                return Err(bad(format!("the phrase {} for the code '{}' is not in '{}'", quoted(&coding.phrase), coding.code, coding.document)))
            }
            Found::MoreThanOnce => {
                return Err(bad(format!(
                    "the phrase {} for the code '{}' occurs more than once in '{}', so it does not say which words to code",
                    quoted(&coding.phrase),
                    coding.code,
                    coding.document
                )))
            }
        };
        codings.push(PlannedCoding { def: coding, document, code, start, end });
    }

    Ok(Plan { batch, cases, codes, codings })
}

// ---- filling the new project ------------------------------------------------------------------------

fn fill(project: &mut Project, def: &SampleDef, plan: &Plan, actor: &str) -> Result<(), ApiError> {
    // The codings belong to the person using the project, like any coding they make by hand.
    let me = project.ensure_human_coder(actor, &actor_local_id(actor))?;
    let source_ids = project.apply_import(&plan.batch)?.source_ids;

    for (name, members) in &plan.cases {
        let case = project.add_case(name, "")?;
        for &document in members {
            project.assign_source_to_case(case.id, source_ids[document])?;
        }
    }

    let mut code_ids: Vec<i64> = Vec::with_capacity(plan.codes.len());
    for planned in &plan.codes {
        let mut new = NewCode::new(planned.def.name.clone());
        new.parent_id = planned.parent.map(|p| code_ids[p]);
        new.theme = planned.def.theme;
        new.color = planned.def.color.clone();
        new.definition = planned.def.definition.clone();
        new.rated = planned.def.rated;
        new.rating_label = planned.def.rating_label.clone();
        new.rating_low = planned.def.rating_low.clone();
        new.rating_high = planned.def.rating_high.clone();
        new.exclusive_group = planned.def.exclusive_group.clone();
        code_ids.push(project.add_code(&new)?);
    }
    // Siblings sort by position and then by name, so without this the codebook would come out alphabetical.
    // Moving each one to its place in turn leaves them in the order the sample lists them.
    let mut group_size: HashMap<Option<usize>, usize> = HashMap::new();
    for planned in &plan.codes {
        *group_size.entry(planned.parent).or_default() += 1;
    }
    let mut placed: HashMap<Option<usize>, usize> = HashMap::new();
    for (i, planned) in plan.codes.iter().enumerate() {
        let position = placed.entry(planned.parent).or_default();
        // The last one in a group lands at the end by itself.
        if *position + 1 < group_size[&planned.parent] {
            project.move_code(code_ids[i], planned.parent.map(|p| code_ids[p]), Some(*position))?;
        }
        *position += 1;
    }

    let by_unit = def.coding_mode == Some(CodingMode::Unit);
    let mut codings = Vec::with_capacity(plan.codings.len());
    for planned in &plan.codings {
        let source_id = source_ids[planned.document];
        let mut coding = NewCoding::new(source_id, planned.start, planned.end, code_ids[planned.code], me);
        coding.rationale = planned.def.note.clone();
        if by_unit {
            let unit = project
                .list_units(source_id)?
                .into_iter()
                .find(|u| !u.excluded && u.start <= planned.start && planned.end <= u.end)
                .ok_or_else(|| {
                    ApiError::new(
                        "invalid",
                        format!("sample '{}': the phrase {} in '{}' does not fall inside one unit, and this sample codes whole units", def.id, quoted(&planned.def.phrase), planned.def.document),
                    )
                })?;
            coding.unit_id = Some(unit.id);
        }
        codings.push(coding);
    }
    let coding_ids = project.apply_codings(&codings)?;

    for (planned, &coding_id) in plan.codings.iter().zip(&coding_ids) {
        if let Some(rating) = planned.def.rating {
            project.set_coding_rating(coding_id, Some(rating))?;
        }
        if planned.def.example {
            project.set_anchor(coding_id, true)?;
        }
        if let Some(other) = &planned.def.counterexample_of {
            let other_id = plan.codes.iter().position(|c| c.def.name == *other).map(|i| code_ids[i]).expect("checked when the sample was planned");
            project.set_counterexample(other_id, coding_id, true)?;
        }
        if let Some(memo) = &planned.def.memo {
            project.add_memo(&NewMemo {
                kind: MemoKind::Analytic,
                scope: MemoScope::Coding,
                target_id: Some(coding_id),
                phase: None,
                title: memo.title.clone(),
                body: memo.body.clone(),
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn opts() -> OpenOptions {
        let mut o = OpenOptions::new("Tester");
        o.backup_on_open = false;
        o
    }

    fn def(value: Value) -> SampleDef {
        serde_json::from_value(value).expect("a sample definition")
    }

    /// A tiny sample that builds, for tests to break one thing at a time.
    fn tiny() -> Value {
        json!({
            "id": "tiny", "name": "Tiny",
            "documents": [{ "name": "Doc", "folder": "Folder", "text": "One two three. Four five six." }],
            "cases": [{ "name": "Case A", "documents": ["Doc"] }],
            "codes": [{ "name": "Group", "children": [{ "name": "Alpha" }, { "name": "Beta" }] }],
            "codings": [{ "document": "Doc", "phrase": "two three", "code": "Alpha", "note": "n" }]
        })
    }

    fn names_in(dir: &TempDir) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        names
    }

    fn error_of(value: Value) -> (ApiError, Vec<String>) {
        let dir = TempDir::new().unwrap();
        let e = build(&def(value), &dir.path().join("t.pqda"), &opts()).err().expect("the sample should be refused");
        (e, names_in(&dir))
    }

    #[test]
    fn every_bundled_sample_parses_under_its_own_id_and_the_ids_differ() {
        let mut seen = Vec::new();
        for (id, json) in SAMPLES {
            let def = parse(id, json).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(!def.name.trim().is_empty() && !def.description.trim().is_empty(), "{id} needs a name and a description");
            assert!(!def.documents.is_empty(), "{id}");
            assert!(!seen.contains(id), "two samples share the id {id}");
            seen.push(id);
        }
    }

    #[test]
    fn every_bundled_sample_builds_and_reopens_with_the_counts_it_advertises() {
        for (id, json) in SAMPLES {
            let dir = TempDir::new().unwrap();
            let path = dir.path().join("copy.pqda");
            let def = parse(id, json).unwrap();
            let project = create(id, path.to_str().unwrap(), &opts()).unwrap_or_else(|e| panic!("{id}: {e}"));
            let info = project.info().unwrap();
            assert_eq!(info.sources as usize, def.documents.len(), "{id}");
            assert_eq!(info.codes as usize, count_codes(&def.codes), "{id}");
            assert_eq!(info.codings as usize, def.codings.len(), "{id}");
            assert_eq!(info.sample_id.as_deref(), Some(*id));
            assert!(info.units >= info.sources, "{id}: every document should have units");
            assert!(!info.read_only);
        }
    }

    #[test]
    fn a_phrase_that_is_not_there_names_itself_and_leaves_no_file() {
        let mut v = tiny();
        v["codings"][0]["phrase"] = json!("two three four");
        let (e, left) = error_of(v);
        assert_eq!(e.code, "invalid");
        assert!(e.message.contains("two three four") && e.message.contains("not in"), "{}", e.message);
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn a_phrase_found_twice_is_refused_even_when_the_two_copies_overlap() {
        let mut v = tiny();
        v["documents"][0]["text"] = json!("Go on. Go on. Stop.");
        v["codings"][0]["phrase"] = json!("Go on.");
        let (e, left) = error_of(v);
        assert_eq!(e.code, "invalid");
        assert!(e.message.contains("Go on.") && e.message.contains("more than once"), "{}", e.message);
        assert!(left.is_empty(), "{left:?}");

        let mut v = tiny();
        v["documents"][0]["text"] = json!("aaa");
        v["codings"][0]["phrase"] = json!("aa");
        let (e, left) = error_of(v);
        assert!(e.message.contains("more than once"), "{}", e.message);
        assert!(left.is_empty());
    }

    #[test]
    fn offsets_count_scalar_values_not_bytes() {
        let text = "Caf\u{e9} \u{1F600}\u{1F600} then ok here";
        let Found::Once(start, end) = locate(text, "ok") else { panic!("ok should be found once") };
        assert_eq!(text.chars().skip(start as usize).take((end - start) as usize).collect::<String>(), "ok");
        // The accented word and its space are 5 characters, the two emoji and the space after them 3 more, "then " 5 more.
        assert_eq!(start, 13);
        assert!(text.len() > text.chars().count(), "the text has multi-byte characters, so bytes and characters differ");
        assert!(matches!(locate(text, "\u{1F600}"), Found::MoreThanOnce));
        assert!(matches!(locate(text, "missing"), Found::Never));
        let Found::Once(s, e) = locate(text, "\u{e9} \u{1F600}\u{1F600}") else { panic!("found once") };
        assert_eq!((s, e), (3, 7));
    }

    #[test]
    fn a_coding_after_astral_characters_covers_exactly_its_words_in_the_finished_project() {
        let mut v = tiny();
        v["documents"][0]["text"] = json!("\u{1F600}\u{1F600} caf\u{e9} \u{1F4A1} idea here. Another line.");
        v["codings"][0]["phrase"] = json!("idea here.");
        let dir = TempDir::new().unwrap();
        let project = build(&def(v), &dir.path().join("t.pqda"), &opts()).unwrap();
        let sources = project.list_sources().unwrap();
        let codings = project.list_codings(sources[0].id).unwrap();
        assert_eq!(project.text_of_span(sources[0].id, codings[0].start, codings[0].end).unwrap(), "idea here.");
    }

    #[test]
    fn names_that_point_nowhere_are_refused_before_a_file_exists() {
        let cases: Vec<(Value, &str)> = vec![
            (json!({ "cases": [{ "name": "C", "documents": ["Nope"] }] }), "Nope"),
            (json!({ "codings": [{ "document": "Nope", "phrase": "two", "code": "Alpha" }] }), "Nope"),
            (json!({ "codings": [{ "document": "Doc", "phrase": "two", "code": "Nope" }] }), "Nope"),
            (json!({ "codings": [{ "document": "Doc", "phrase": "", "code": "Alpha" }] }), "no phrase"),
            (json!({ "codings": [{ "document": "Doc", "phrase": "two", "code": "Alpha", "rating": 6 }] }), "1 to 5"),
            (json!({ "codes": [{ "name": "Alpha" }, { "name": "G", "children": [{ "name": "Alpha" }] }] }), "two codes are named 'Alpha'"),
            (json!({ "documents": [{ "name": "Doc", "text": "a" }, { "name": "Doc", "text": "b" }] }), "two documents are named 'Doc'"),
            (json!({ "documents": [{ "name": "Doc", "text": "   \n  " }] }), "cannot be imported"),
        ];
        for (change, expect) in cases {
            let mut v = tiny();
            for (key, value) in change.as_object().unwrap() {
                v[key] = value.clone();
            }
            let (e, left) = error_of(v);
            assert_eq!(e.code, "invalid", "{expect}");
            assert!(e.message.contains(expect), "{expect}: {}", e.message);
            assert!(e.message.starts_with("sample 'tiny':"), "{}", e.message);
            assert!(left.is_empty(), "{expect}: {left:?}");
        }
    }

    #[test]
    fn a_failure_after_the_file_exists_removes_the_file_and_its_companions() {
        // Passes planning (the names differ), but the engine refuses two cases that differ only in capitals.
        let mut v = tiny();
        v["cases"] = json!([{ "name": "Case A", "documents": ["Doc"] }, { "name": "case a", "documents": [] }]);
        let (e, left) = error_of(v);
        assert_eq!(e.code, "invalid");
        assert!(e.message.contains("already a case"), "{}", e.message);
        assert!(left.is_empty(), "the half-made project must be gone, with its -wal and -shm: {left:?}");

        // The same for a codebook the engine refuses after planning.
        let mut v = tiny();
        v["codes"] = json!([{ "name": "Alpha", "children": [{ "name": "ALPHA2" }, { "name": "alpha2x" }] }, { "name": "Beta" }, { "name": "beta" }]);
        let (e, left) = error_of(v);
        assert_eq!(e.code, "invalid");
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn an_existing_file_is_refused_and_never_deleted() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mine.pqda");
        fs::write(&path, b"precious").unwrap();
        let e = build(&def(tiny()), &path, &opts()).err().unwrap();
        assert_eq!(e.code, "already_exists");
        assert_eq!(fs::read(&path).unwrap(), b"precious");
        assert_eq!(names_in(&dir), ["mine.pqda"]);
    }

    #[test]
    fn a_missing_folder_fails_cleanly() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("no-such-folder").join("t.pqda");
        assert!(build(&def(tiny()), &path, &opts()).is_err());
        assert!(names_in(&dir).is_empty());
    }

    #[test]
    fn siblings_keep_the_order_the_sample_lists_them_in() {
        let mut v = tiny();
        v["codes"] = json!([
            { "name": "Zulu", "children": [{ "name": "Yankee" }, { "name": "Xray" }, { "name": "Whiskey" }] },
            { "name": "Alpha" },
            { "name": "Mike" }
        ]);
        let dir = TempDir::new().unwrap();
        let project = build(&def(v), &dir.path().join("t.pqda"), &opts()).unwrap();
        let codes = project.list_codes().unwrap();
        let names = |parent: Option<i64>| codes.iter().filter(|c| c.parent_id == parent).map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(None), ["Zulu", "Alpha", "Mike"]);
        let zulu = codes.iter().find(|c| c.name == "Zulu").unwrap().id;
        assert_eq!(names(Some(zulu)), ["Yankee", "Xray", "Whiskey"]);
    }

    #[test]
    fn a_sample_that_codes_whole_units_gives_each_coding_its_unit() {
        let mut v = tiny();
        v["coding_mode"] = json!("unit");
        let dir = TempDir::new().unwrap();
        let project = build(&def(v.clone()), &dir.path().join("t.pqda"), &opts()).unwrap();
        assert_eq!(project.info().unwrap().coding_mode, "unit");
        let source = project.list_sources().unwrap()[0].id;
        let coding = &project.list_codings(source).unwrap()[0];
        let unit = project.list_units(source).unwrap().into_iter().find(|u| Some(u.id) == coding.unit_id).expect("the coding has a unit");
        assert!(unit.start <= coding.start && coding.end <= unit.end);

        // A phrase that runs across two sentences fits no single unit: refused, and nothing is left behind.
        v["codings"][0]["phrase"] = json!("three. Four");
        let (e, left) = error_of(v);
        assert_eq!(e.code, "invalid");
        assert!(e.message.contains("three. Four") && e.message.contains("one unit"), "{}", e.message);
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn unknown_keys_in_a_sample_are_caught_rather_than_ignored() {
        let mut v = tiny();
        v["codings"][0]["defintion"] = json!("typo");
        assert!(serde_json::from_value::<SampleDef>(v).is_err());
    }

    #[test]
    fn the_copy_starts_a_fresh_session_so_undo_cannot_take_the_sample_apart() {
        let dir = TempDir::new().unwrap();
        let mut project = build(&def(tiny()), &dir.path().join("t.pqda"), &opts()).unwrap();
        assert_eq!(project.undo().unwrap(), None);
        assert_eq!(project.next_undo().unwrap(), None);
    }
}
