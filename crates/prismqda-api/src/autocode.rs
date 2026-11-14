//! Auto Code: finding passages by rule and suggesting a code for them (specification 12.3).
//!
//! Three ways to find passages need no model: the words a search finds (widened to a sentence, a paragraph, a unit, or a
//! number of words around), the turns of the speakers a person picks, and the sections under headings. Each one is
//! planned here, shown as a preview, and then sent to the review queue; nothing is coded until a person accepts it.

use std::collections::{BTreeMap, HashMap, HashSet};

use prismqda_engine::{speaker_key, CodeRow, FoundSpan, NewCode, NewProposal, Project, ProposalOrigin, SearchFilter};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{to_json, ApiError};

/// How many places one run may find. A run that finds more says it was cut short.
const MAX_FOUND: usize = 20_000;
/// How many words each side "words around" may add.
const MAX_AROUND: usize = 50;
/// The same palette the interface offers for codes, in the same order (`app/ui/src/lib/labels.ts`).
const PALETTE: [&str; 22] = [
    "#E69F00", "#56B4E9", "#009E73", "#F0E442", "#0072B2", "#D55E00", "#CC79A7", "#999999", "#D62839", "#F0388F", "#F7A8C8", "#8E44AD", "#A99BE6", "#4B3FBF", "#1F3A93",
    "#1B998B", "#7ED3A9", "#8FBF2A", "#FFB38A", "#8D6E63", "#5F7A8A", "#8C2F4B",
];

fn default_method() -> String {
    "words".to_string()
}

fn default_target() -> String {
    "hit".to_string()
}

/// One speaker picked for autocoding, with the code to suggest for their turns.
#[derive(Deserialize)]
pub(crate) struct SpeakerChoice {
    /// The speaker's key from `autocode.speakers`.
    key: String,
    #[serde(default)]
    code_id: Option<i64>,
    /// Name a new code (or an existing code's name) instead of picking one.
    #[serde(default)]
    new_code: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AutocodeArgs {
    /// `words` (the default), `speakers`, or `headings`.
    #[serde(default = "default_method")]
    method: String,
    /// Words: the search to run, in the search language.
    #[serde(default)]
    query: String,
    /// Words: what each suggestion covers: `hit` (the matched words), `words` (a number of words around them),
    /// `sentence`, `paragraph`, or `unit`.
    #[serde(default = "default_target")]
    target: String,
    /// Words, with the target `words`: how many words each side (1 to 50).
    #[serde(default)]
    around: Option<usize>,
    /// Headings: which heading levels (1 to 6).
    #[serde(default)]
    levels: Vec<u8>,
    /// Headings: only headings whose words match this search.
    #[serde(default)]
    heading_query: Option<String>,
    /// Headings: whether the suggestion covers the heading too (the default) or only what is under it.
    #[serde(default = "crate::yes")]
    include_heading: bool,
    /// Speakers: who to code, and with which code.
    #[serde(default)]
    speakers: Vec<SpeakerChoice>,
    /// Words and headings: the code to suggest, or a name for a new one.
    #[serde(default)]
    code_id: Option<i64>,
    #[serde(default)]
    new_code: Option<String>,
    /// Defaults to the person using the project.
    #[serde(default)]
    pub(crate) coder_id: Option<i64>,
    /// Where to look: `source_ids`, `case_id`, and for words `with_code`, `without_code`, `limit`.
    #[serde(flatten)]
    filter: SearchFilter,
}

#[derive(Deserialize)]
pub(crate) struct ScopeArgs {
    #[serde(flatten)]
    filter: SearchFilter,
}

/// Which codes a run will suggest. A code that does not exist yet is a negative number until the run is sent: -1 is the
/// first new name, -2 the second.
struct Chooser<'a> {
    all: &'a [CodeRow],
    new_codes: Vec<String>,
}

impl<'a> Chooser<'a> {
    fn resolve(&mut self, code_id: Option<i64>, new_code: Option<&str>) -> Result<i64, ApiError> {
        if let Some(id) = code_id {
            let code = self.all.iter().find(|c| c.id == id).ok_or_else(|| ApiError::new("not_found", format!("code {id} does not exist")))?;
            if code.status != "active" {
                return Err(ApiError::new("invalid", format!("the code {} is {}, so it cannot be suggested", code.name, code.status)));
            }
            return Ok(id);
        }
        let name = new_code.map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| ApiError::new("bad_args", "choose a code, or give a name for a new one"))?;
        let lower = name.to_lowercase();
        // A name that is already a code is that code: running speakers again must not fail on "Priya" existing.
        if let Some(code) = self.all.iter().find(|c| c.status == "active" && c.name.to_lowercase() == lower) {
            return Ok(code.id);
        }
        if let Some(i) = self.new_codes.iter().position(|n| n.to_lowercase() == lower) {
            return Ok(-(i as i64) - 1);
        }
        self.new_codes.push(name.to_string());
        Ok(-(self.new_codes.len() as i64))
    }

    fn name_of(&self, id: i64) -> String {
        if id < 0 {
            self.new_codes.get((-id - 1) as usize).cloned().unwrap_or_default()
        } else {
            self.all.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default()
        }
    }
}

struct Planned {
    method: &'static str,
    label: String,
    /// Everything found, before suggestions that repeat earlier work were taken out.
    found: usize,
    truncated: bool,
    left_out: usize,
    already_coded: usize,
    already_waiting: usize,
    kept: Vec<NewProposal>,
    new_codes: Vec<String>,
    /// The name of each code the run suggests, by id (negative for a new code).
    names: BTreeMap<i64, String>,
}

/// A place a run found: the document, where the words start and end, and the unit they sit in.
type Place = (i64, i64, i64, Option<i64>);

fn cut(text: &str, max: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(max).collect::<String>().trim_end())
    }
}

fn list_names(names: &[String]) -> String {
    match names.len() {
        0 => String::new(),
        1..=3 => names.join(", "),
        n => format!("{}, and {} more", names[..3].join(", "), n - 3),
    }
}

fn plan(project: &Project, a: &AutocodeArgs, coder: i64) -> Result<Planned, ApiError> {
    let all = project.list_codes()?;
    let mut chooser = Chooser { all: &all, new_codes: Vec::new() };
    let unit_mode = project.info()?.coding_mode == "unit";
    let mut left_out = 0;
    let mut truncated = false;

    let (method, label, found, items): (&'static str, String, usize, Vec<NewProposal>) = match a.method.as_str() {
        "words" => {
            if !matches!(a.target.as_str(), "hit" | "words" | "sentence" | "paragraph" | "unit") {
                return Err(ApiError::new("bad_args", format!("autocode: the target '{}' should be hit, words, sentence, paragraph, or unit", a.target)));
            }
            let code = chooser.resolve(a.code_id, a.new_code.as_deref())?;
            if unit_mode && !matches!(a.target.as_str(), "hit" | "unit") {
                return Err(ApiError::new("invalid", "this project codes whole units, so a suggestion can cover the matched words or the whole unit, not a sentence, paragraph, or number of words"));
            }
            let query = a.query.trim();
            let mut filter = a.filter.clone();
            filter.limit = Some(filter.limit.unwrap_or(MAX_FOUND).min(MAX_FOUND));
            let results = project.search(query, &filter)?;
            truncated = results.truncated;
            let spans = words_spans(project, a, &results.hits)?;
            let found = spans.len();
            let items = spans
                .into_iter()
                .map(|(source_id, start, end, unit_id)| NewProposal {
                    source_id,
                    start,
                    end,
                    code_id: code,
                    coder_id: coder,
                    unit_id,
                    rationale: format!("autocoded from search: {query}"),
                })
                .collect();
            ("words", format!("Words: {}", cut(query, 60)), found, items)
        }
        "speakers" => {
            if a.speakers.is_empty() {
                return Err(ApiError::new("bad_args", "choose at least one speaker"));
            }
            let mut codes: HashMap<String, i64> = HashMap::new();
            for s in &a.speakers {
                codes.insert(speaker_key(&s.key), chooser.resolve(s.code_id, s.new_code.as_deref())?);
            }
            let keys: Vec<String> = codes.keys().cloned().collect();
            let groups = project.speaker_groups(&a.filter)?;
            left_out = groups.iter().filter(|g| codes.contains_key(&g.key)).map(|g| g.left_out).sum();
            let mut turns = project.speaker_turns(&a.filter, &keys)?;
            if turns.len() > MAX_FOUND {
                turns.truncate(MAX_FOUND);
                truncated = true;
            }
            let found = turns.len();
            let items = turns
                .into_iter()
                .map(|t| NewProposal {
                    source_id: t.source_id,
                    start: t.start,
                    end: t.end,
                    code_id: codes[&t.key],
                    coder_id: coder,
                    unit_id: Some(t.unit_id),
                    rationale: format!("autocoded from speaker: {}", t.speaker),
                })
                .collect();
            let names: Vec<String> = groups.iter().filter(|g| codes.contains_key(&g.key)).map(|g| g.label.clone()).collect();
            ("speakers", format!("Speakers: {}", list_names(&names)), found, items)
        }
        "headings" => {
            if a.levels.is_empty() || a.levels.iter().any(|l| !(1..=6).contains(l)) {
                return Err(ApiError::new("bad_args", "choose which heading levels to use (1 to 6)"));
            }
            if unit_mode {
                return Err(ApiError::new("invalid", "this project codes whole units, and a section under a heading is a stretch of words, not a unit"));
            }
            let code = chooser.resolve(a.code_id, a.new_code.as_deref())?;
            let mut sections = project.heading_sections(&a.filter, &a.levels, a.heading_query.as_deref(), a.include_heading)?;
            if sections.len() > MAX_FOUND {
                sections.truncate(MAX_FOUND);
                truncated = true;
            }
            let found = sections.len();
            let items = sections
                .into_iter()
                .map(|s| NewProposal {
                    source_id: s.source_id,
                    start: s.start,
                    end: s.end,
                    code_id: code,
                    coder_id: coder,
                    unit_id: None,
                    rationale: format!("autocoded from heading: {}", cut(&s.heading, 60)),
                })
                .collect();
            let mut levels = a.levels.clone();
            levels.sort_unstable();
            levels.dedup();
            let which = levels.iter().map(u8::to_string).collect::<Vec<_>>().join(", ");
            let label = match a.heading_query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
                Some(q) => format!("Headings: level {which} containing {}", cut(q, 40)),
                None => format!("Headings: level {which}"),
            };
            ("headings", label, found, items)
        }
        other => return Err(ApiError::new("bad_args", format!("autocode: the method '{other}' should be words, speakers, or headings"))),
    };

    let dropped = project.drop_known(items)?;
    let names = dropped.kept.iter().map(|i| i.code_id).collect::<HashSet<_>>().into_iter().map(|id| (id, chooser.name_of(id))).collect();
    Ok(Planned {
        method,
        label,
        found,
        truncated,
        left_out,
        already_coded: dropped.already_coded,
        already_waiting: dropped.already_waiting,
        kept: dropped.kept,
        new_codes: chooser.new_codes,
        names,
    })
}

/// The places a words search turns into suggestions, as `(document, start, end, unit)`, in document order, with places
/// that overlap joined into one.
fn words_spans(project: &Project, a: &AutocodeArgs, hits: &[prismqda_engine::SearchHit]) -> Result<Vec<Place>, ApiError> {
    let mut spans: Vec<Place> = Vec::new();
    match a.target.as_str() {
        "unit" => {
            for hit in hits {
                spans.push((hit.source_id, hit.scope_start, hit.scope_end, hit.unit_id));
            }
        }
        "hit" => {
            for hit in hits {
                for (s, e) in &hit.spans {
                    spans.push((hit.source_id, *s, *e, hit.unit_id));
                }
            }
        }
        "words" => {
            let n = a.around.unwrap_or(5);
            if !(1..=MAX_AROUND).contains(&n) {
                return Err(ApiError::new("bad_args", format!("autocode: the number of words around should be 1 to {MAX_AROUND}")));
            }
            let found: Vec<FoundSpan> = hits
                .iter()
                .flat_map(|h| {
                    h.spans.iter().map(move |(s, e)| FoundSpan { source_id: h.source_id, scope_start: h.scope_start, scope_end: h.scope_end, start: *s, end: *e, unit_id: h.unit_id })
                })
                .collect();
            for f in project.widen_by_words(&found, n)? {
                spans.push((f.source_id, f.start, f.end, f.unit_id));
            }
        }
        _ => {
            // A sentence or a paragraph: the piece of the document each match sits in.
            let mut pieces: HashMap<i64, Vec<prismqda_engine::ImportedUnit>> = HashMap::new();
            for hit in hits {
                if let std::collections::hash_map::Entry::Vacant(slot) = pieces.entry(hit.source_id) {
                    let text = project.source_detail(hit.source_id)?.text;
                    slot.insert(if a.target == "sentence" { prismqda_import::segment::sentences(&text) } else { prismqda_import::segment::paragraphs(&text) });
                }
                let units = &pieces[&hit.source_id];
                for (s, _) in &hit.spans {
                    let i = units.partition_point(|u| u.end <= *s);
                    if let Some(u) = units.get(i).filter(|u| u.start <= *s) {
                        spans.push((hit.source_id, u.start, u.end, hit.unit_id));
                    }
                }
            }
        }
    }
    spans.sort();
    spans.dedup();
    let mut joined: Vec<Place> = Vec::with_capacity(spans.len());
    for (source_id, start, end, unit_id) in spans {
        if let Some(last) = joined.last_mut() {
            if last.0 == source_id && start < last.2 {
                last.2 = last.2.max(end);
                continue;
            }
        }
        joined.push((source_id, start, end, unit_id));
    }
    Ok(joined)
}

/// What a run would do, without doing it: how many places it found, how many are new, and the first few in their words.
pub(crate) fn preview(project: &Project, a: &AutocodeArgs, coder: i64) -> Result<Value, ApiError> {
    let p = plan(project, a, coder)?;
    let places: Vec<(i64, i64, i64)> = p.kept.iter().take(6).map(|i| (i.source_id, i.start, i.end)).collect();
    let passages = project.passages(&places)?;
    let examples: Vec<Value> = p
        .kept
        .iter()
        .zip(passages)
        .map(|(i, text)| {
            json!({
                "source_id": i.source_id, "source_name": text.source_name, "start": i.start, "end": i.end,
                "text": text.text, "truncated": text.truncated, "before": text.before, "after": text.after,
                "code_name": p.names.get(&i.code_id).cloned().unwrap_or_default(),
            })
        })
        .collect();
    let mut per_code: BTreeMap<i64, usize> = BTreeMap::new();
    for i in &p.kept {
        *per_code.entry(i.code_id).or_default() += 1;
    }
    let codes: Vec<Value> = per_code
        .iter()
        .map(|(id, n)| json!({ "code_id": if *id > 0 { json!(id) } else { Value::Null }, "name": p.names.get(id).cloned().unwrap_or_default(), "new": *id < 0, "suggestions": n }))
        .collect();
    let documents = p.kept.iter().map(|i| i.source_id).collect::<HashSet<_>>().len();
    Ok(json!({
        "method": p.method,
        "label": p.label,
        "found": p.found,
        "suggestions": p.kept.len(),
        "documents": documents,
        "already_coded": p.already_coded,
        "already_waiting": p.already_waiting,
        "left_out": p.left_out,
        "truncated": p.truncated,
        "codes": codes,
        "examples": examples,
    }))
}

/// Sends what a run found to the review queue, making any new codes it needs first.
pub(crate) fn propose(project: &mut Project, a: &AutocodeArgs, coder: i64) -> Result<Value, ApiError> {
    let p = plan(project, a, coder)?;
    if p.kept.is_empty() {
        return Err(ApiError::new(
            "invalid",
            if p.found == 0 {
                "the search found nothing to suggest a code for".to_string()
            } else {
                "everything found is already coded this way or already waiting in the review queue".to_string()
            },
        ));
    }
    let mut made: HashMap<i64, i64> = HashMap::new();
    let mut created: Vec<Value> = Vec::new();
    for (i, name) in p.new_codes.iter().enumerate() {
        let placeholder = -(i as i64) - 1;
        if !p.kept.iter().any(|item| item.code_id == placeholder) {
            continue;
        }
        let mut counts = vec![0usize; PALETTE.len()];
        for c in project.list_codes()?.iter().filter(|c| c.status == "active") {
            if let Some(k) = PALETTE.iter().position(|h| h.eq_ignore_ascii_case(&c.color)) {
                counts[k] += 1;
            }
        }
        let best = (0..counts.len()).min_by_key(|k| counts[*k]).unwrap_or(0);
        let mut code = NewCode::new(name.clone());
        code.color = Some(PALETTE[best].to_string());
        let id = project.add_code(&code)?;
        made.insert(placeholder, id);
        created.push(json!({ "id": id, "name": name }));
    }
    let items: Vec<NewProposal> = p
        .kept
        .into_iter()
        .map(|mut i| {
            if let Some(id) = made.get(&i.code_id) {
                i.code_id = *id;
            }
            i
        })
        .collect();
    let batch = project.create_proposals_by(ProposalOrigin::Autocode, &p.label, p.method, &items)?;
    Ok(json!({
        "batch": batch.batch,
        "created": batch.created,
        "found": p.found,
        "scopes_found": p.found,
        "truncated": p.truncated,
        "left_out": p.left_out,
        "already_coded": p.already_coded,
        "already_waiting": p.already_waiting,
        "codes_created": created,
    }))
}

/// The speakers of the chosen documents, spelling variants gathered.
pub(crate) fn speakers(project: &Project, a: &ScopeArgs) -> Result<Value, ApiError> {
    to_json(&project.speaker_groups(&a.filter)?)
}

/// The heading levels the chosen documents have.
pub(crate) fn headings(project: &Project, a: &ScopeArgs) -> Result<Value, ApiError> {
    Ok(json!({ "levels": project.heading_levels(&a.filter)? }))
}
