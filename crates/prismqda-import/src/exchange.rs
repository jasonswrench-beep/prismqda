//! The exchange file of Prism Reference Manager (`prism-family-exchange`, version 1.0): sources and the excerpts taken from them.
//!
//! A literature review can be coded straight from the library. Each source of the file becomes one document, holding its details and
//! abstract (the same text a reference library import makes) and then its **excerpts**, one paragraph each: what kind of excerpt it is
//! and where in the source, the exact words taken from the source, and the person's own note. Each source also becomes a case whose
//! variables hold its details, its citation key, and its id in the library. An excerpt that names no source (or one that is not in the
//! file) goes into one document of its own, and the report says how many.
//!
//! The file is the Reference Manager's, and is read from what it writes (`export_family_exchange` in its `prm-core`) and from the five
//! lines of its `docs/library-format.md`; the specification and a conformance set are asked of the shared core (`CORE-REQUESTS.md`, R5).
//! PrismQDA keeps, and writes nothing back: each source's library id is a variable of its case, and each excerpt's id and place are in
//! the settings of its document, so that a return can name them later.
//!
//! No network is used: the file names no address that is opened, and a source's link is only text.

use prismqda_engine::{AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportWarning, ImportedCase, ImportedSource, Level, RunKind, SourceType, UnitKind};
use serde_json::{json, Value};

use crate::builder::TextBuilder;
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;
use crate::reference::{csl_reference, label, write_reference};
use crate::segment::units_for;

/// The name the Reference Manager gives this file's format.
pub const FORMAT: &str = "prism-family-exchange";

/// A file with more sources or excerpts than this is refused, so that one file cannot fill a project by accident.
const MAX_SOURCES: usize = 50_000;
const MAX_EXCERPTS: usize = 200_000;

/// True when a parsed JSON value is an exchange file: it has a `manifest` that names the format.
pub fn looks_like_exchange(value: &Value) -> bool {
    value.get("manifest").and_then(|m| m.get("format")).and_then(Value::as_str) == Some(FORMAT)
}

fn text_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

/// A passage of text on one line or a few: lines kept, but no blank line inside, so that an excerpt stays one paragraph.
fn one_paragraph(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<&str> = Vec::new();
    for line in s.split('\n') {
        let line = line.trim();
        if !line.is_empty() {
            out.push(line);
        }
    }
    out.join("\n")
}

fn first_upper(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The excerpts of one document, in the order the file has them.
struct Excerpt<'a> {
    value: &'a Value,
    /// Where the excerpt is written in the document, set when it is.
    start: i64,
    end: i64,
}

/// The first line of an excerpt: what it is, where in the source, and anything the reader should weigh before relying on it.
fn heading_line(e: &Value, also_from: &[String]) -> String {
    let kind = text_of(e, "type");
    let mut line = if kind.is_empty() { "Excerpt".to_string() } else { first_upper(&kind) };
    let locator = e.get("locator");
    let place = [locator.map(|l| text_of(l, "label")).unwrap_or_default(), locator.map(|l| text_of(l, "value")).unwrap_or_default()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if !place.is_empty() {
        line.push_str(&format!(", {place}"));
    }
    let mut cautions: Vec<String> = Vec::new();
    if text_of(e, "anchor_state") == "unresolved" {
        cautions.push("place in the source not confirmed".to_string());
    }
    if e.get("exact_text_edited").and_then(Value::as_bool) == Some(true) {
        cautions.push("the quoted words were edited after they were taken".to_string());
    }
    match text_of(e, "origin").as_str() {
        "ai" => cautions.push("written by an AI tool".to_string()),
        "agent" => cautions.push("written by an automated agent".to_string()),
        _ => {}
    }
    if !also_from.is_empty() {
        cautions.push(format!("also from {}", also_from.join("; ")));
    }
    if !cautions.is_empty() {
        line.push_str(&format!(" ({})", cautions.join("; ")));
    }
    line
}

/// Writes one excerpt as a paragraph and returns what it holds, or `None` when it holds no words at all.
fn write_excerpt(b: &mut TextBuilder, e: &Value, also_from: &[String]) -> Option<(i64, i64)> {
    let exact = one_paragraph(&text_of(e, "exact_text"));
    let own = one_paragraph(&text_of(e, "user_text"));
    if exact.is_empty() && own.is_empty() {
        return None;
    }
    b.paragraph_break();
    let start = b.len();
    b.push(&heading_line(e, also_from));
    if !exact.is_empty() {
        b.line_break();
        b.push(&format!("\u{201c}{exact}\u{201d}"));
    }
    if !own.is_empty() {
        b.line_break();
        b.push(&format!("Note: {own}"));
    }
    Some((start, b.len()))
}

/// Turns an exchange file into a batch: a document and a case for each source, and a document for the excerpts that have no source.
pub fn parse_exchange(value: &Value, opts: &ImportOptions) -> Result<ImportBatch> {
    let manifest = value.get("manifest").ok_or_else(|| ImportError::format("this is not an exchange file of Prism Reference Manager: it has no manifest"))?;
    let version = text_of(manifest, "version");
    let major: u32 = version.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0);
    if major != 1 {
        return Err(ImportError::unsupported(format!(
            "this exchange file is version {} of its format, and this version of PrismQDA reads version 1; the file was made by {}",
            if version.is_empty() { "(not named)" } else { &version },
            [text_of(manifest, "producer"), text_of(manifest, "producer_version")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
        )));
    }
    let sources = value.get("sources").and_then(Value::as_array).cloned().unwrap_or_default();
    let excerpts = value.get("excerpts").and_then(Value::as_array).cloned().unwrap_or_default();
    if sources.len() > MAX_SOURCES || excerpts.len() > MAX_EXCERPTS {
        return Err(ImportError::unsupported(format!("this exchange file holds {} sources and {} excerpts, more than one import takes (up to {MAX_SOURCES} and {MAX_EXCERPTS})", sources.len(), excerpts.len())));
    }
    if sources.is_empty() && excerpts.is_empty() {
        return Err(ImportError::format("the exchange file has no sources and no excerpts in it"));
    }

    // The sources, as references, and the place each library id has in the list.
    let refs: Vec<(String, String, crate::reference::Reference)> = sources
        .iter()
        .map(|s| (text_of(s, "id"), text_of(s, "citekey"), csl_reference(s.get("csl").unwrap_or(&Value::Null))))
        .collect();
    let position = |id: &str| refs.iter().position(|(rid, _, _)| !id.is_empty() && rid == id);

    // Each excerpt goes to the first of its sources that is in the file, or to the document of excerpts with no source.
    let mut per_source: Vec<Vec<(usize, Vec<String>)>> = vec![Vec::new(); refs.len()];
    let mut loose: Vec<usize> = Vec::new();
    let mut named_missing = 0usize;
    for (n, e) in excerpts.iter().enumerate() {
        let ids: Vec<String> = e.get("source_ids").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        let found: Vec<usize> = ids.iter().filter_map(|id| position(id)).collect();
        match found.first() {
            Some(&first) => {
                let also: Vec<String> = found.iter().skip(1).map(|&i| label(&refs[i].2, i + 1)).collect();
                per_source[first].push((n, also));
            }
            None => {
                if !ids.is_empty() {
                    named_missing += 1;
                }
                loose.push(n);
            }
        }
    }

    let folder = if opts.folder.is_empty() { "Library".to_string() } else { opts.folder.clone() };
    let kind = opts.unit_kind.unwrap_or(UnitKind::Paragraph);
    let mut batch = ImportBatch::default();
    let mut specs: Vec<AttributeSpec> = Vec::new();
    let mut want = |name: &str, level: Level| {
        if !specs.iter().any(|s| s.name == name) {
            specs.push(AttributeSpec::new(name, AppliesTo::Case, level));
        }
    };
    let mut used_names: Vec<String> = Vec::new();
    let mut left_empty = 0usize;
    let mut without_words = 0usize;

    for (i, ((library_id, citekey, r), mine)) in refs.iter().zip(&per_source).enumerate() {
        let mut b = TextBuilder::new();
        let has_abstract = write_reference(&mut b, r);
        let mut written: Vec<Excerpt> = Vec::new();
        if !mine.is_empty() {
            b.paragraph_break();
            b.push_run("Excerpts", RunKind::Heading, Some(2));
        }
        for (n, also) in mine {
            let e = &excerpts[*n];
            match write_excerpt(&mut b, e, also) {
                Some((start, end)) => written.push(Excerpt { value: e, start, end }),
                None => without_words += 1,
            }
        }
        let parsed = b.finish(SourceType::Document, kind);
        if parsed.text.trim().is_empty() {
            left_empty += 1;
            continue;
        }
        let mut name = label(r, i + 1);
        if used_names.contains(&name) {
            name = if citekey.is_empty() { format!("{name} ({})", i + 1) } else { format!("{name} ({citekey})") };
        }
        used_names.push(name.clone());
        let mut warnings = Vec::new();
        if !has_abstract && written.is_empty() {
            warnings.push(ImportWarning::new("no_text", "This source has no abstract and no excerpt, so there is little text to code. Its details are kept as variables of its case."));
        }
        let units = units_for(kind, &parsed.text);
        let excerpt_facts: Vec<Value> = written
            .iter()
            .map(|e| json!({ "id": text_of(e.value, "id"), "start": e.start, "end": e.end, "type": text_of(e.value, "type"), "annotation_id": text_of(e.value, "annotation_id") }))
            .collect();
        batch.sources.push(ImportedSource {
            name: name.clone(),
            source_type: opts.source_type.unwrap_or(SourceType::Document),
            folder: folder.clone(),
            text: parsed.text,
            runs: parsed.runs,
            units,
            default_unit_kind: kind,
            unit_attributes: Vec::new(),
            original_file_hash: opts.file_hash.clone(),
            settings: json!({ "format": FORMAT, "library_source_id": library_id, "citekey": citekey, "excerpts": excerpt_facts }),
            warnings,
        });

        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut add = |attr: &str, level: Level, value: String| {
            if !value.is_empty() {
                want(attr, level);
                attrs.push((attr.to_string(), value));
            }
        };
        add("Authors", Level::Text, r.authors.iter().map(|a| a.display.as_str()).collect::<Vec<_>>().join("; "));
        add("Year", Level::Interval, r.year.clone());
        add("Published in", Level::Text, r.container.clone());
        add("Reference type", Level::Nominal, r.kind.clone());
        add("DOI", Level::Text, r.doi.clone());
        add("Keywords", Level::Text, r.keywords.join("; "));
        add("Citation key", Level::Text, citekey.clone());
        add("Library source id", Level::Text, library_id.clone());
        add("Excerpts taken", Level::Ratio, written.len().to_string());
        batch.cases.push(ImportedCase { name, attrs, members: vec![CaseMember { source: batch.sources.len() - 1, unit: None }] });
    }

    // Excerpts that no source of the file holds.
    if !loose.is_empty() {
        let mut b = TextBuilder::new();
        b.push_run("Excerpts with no source in this file", RunKind::Heading, Some(1));
        let mut written: Vec<Excerpt> = Vec::new();
        for n in &loose {
            let e = &excerpts[*n];
            match write_excerpt(&mut b, e, &[]) {
                Some((start, end)) => written.push(Excerpt { value: e, start, end }),
                None => without_words += 1,
            }
        }
        if !written.is_empty() {
            let parsed = b.finish(SourceType::Document, kind);
            let units = units_for(kind, &parsed.text);
            let excerpt_facts: Vec<Value> = written.iter().map(|e| json!({ "id": text_of(e.value, "id"), "start": e.start, "end": e.end, "type": text_of(e.value, "type") })).collect();
            let mut warnings = Vec::new();
            if named_missing > 0 {
                warnings.push(ImportWarning::new(
                    "source_not_in_file",
                    format!("{named_missing} excerpt{} named a source that is not in this file, so {} here.", if named_missing == 1 { "" } else { "s" }, if named_missing == 1 { "it is" } else { "they are" }),
                ));
            }
            batch.sources.push(ImportedSource {
                name: "Excerpts with no source".to_string(),
                source_type: opts.source_type.unwrap_or(SourceType::Document),
                folder: folder.clone(),
                text: parsed.text,
                runs: parsed.runs,
                units,
                default_unit_kind: kind,
                unit_attributes: Vec::new(),
                original_file_hash: opts.file_hash.clone(),
                settings: json!({ "format": FORMAT, "library_source_id": "", "excerpts": excerpt_facts }),
                warnings,
            });
        }
    }

    if batch.sources.is_empty() {
        return Err(ImportError::format("none of the sources or excerpts in the file has any words to import"));
    }
    if without_words > 0 || left_empty > 0 {
        let first = batch.sources.first_mut().expect("checked above");
        if without_words > 0 {
            first.warnings.push(ImportWarning::new(
                "excerpt_without_words",
                format!("{without_words} excerpt{} in the file had neither quoted words nor a note, so {} left out.", if without_words == 1 { "" } else { "s" }, if without_words == 1 { "it was" } else { "they were" }),
            ));
        }
        if left_empty > 0 {
            first.warnings.push(ImportWarning::new(
                "source_without_words",
                format!("{left_empty} source{} in the file had no title, details, abstract, or excerpt, so {} left out.", if left_empty == 1 { "" } else { "s" }, if left_empty == 1 { "it was" } else { "they were" }),
            ));
        }
    }
    batch.case_attributes = specs;
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> Value {
        json!({
            "manifest": { "format": "prism-family-exchange", "version": "1.0", "producer": "Prism Reference Manager", "producer_version": "0.1.2", "project": { "id": "p1", "title": "Review" } },
            "sources": [
                { "id": "s-1", "citekey": "hernandez2021", "csl": { "id": "s-1", "type": "article-journal", "title": "Bus reliability and daily stress",
                    "author": [{ "family": "Hernandez", "given": "Maria" }, { "family": "Li", "given": "Wei" }], "issued": { "date-parts": [[2021]] },
                    "container-title": "Journal of Transit", "abstract": "Late buses raise stress." } },
                { "id": "s-2", "citekey": "okafor2019", "csl": { "id": "s-2", "type": "book", "title": "Waiting", "author": [{ "family": "Okafor", "given": "Ada" }], "issued": { "date-parts": [[2019]] } } }
            ],
            "excerpts": [
                { "id": "e-1", "type": "quotation", "source_ids": ["s-1"], "exact_text": "Riders waited a mean of 14 minutes.", "exact_text_edited": false,
                  "user_text": "Compare with our data.", "locator": { "label": "page", "value": "12" }, "anchor_state": "resolved", "origin": "user", "annotation_id": "a-1" },
                { "id": "e-2", "type": "paraphrase", "source_ids": ["s-1", "s-2"], "exact_text": "", "user_text": "Both link delay to stress.",
                  "locator": { "label": "", "value": "" }, "anchor_state": "unresolved", "origin": "ai", "annotation_id": "" },
                { "id": "e-3", "type": "thought", "source_ids": [], "exact_text": "", "user_text": "A question for the interviews.", "locator": { "label": "", "value": "" }, "anchor_state": "resolved", "origin": "user", "annotation_id": "" },
                { "id": "e-4", "type": "quotation", "source_ids": ["s-9"], "exact_text": "From a source that is not in the file.", "user_text": "", "locator": { "label": "", "value": "" }, "anchor_state": "resolved", "origin": "user", "annotation_id": "" },
                { "id": "e-5", "type": "note", "source_ids": ["s-2"], "exact_text": "", "user_text": "", "locator": { "label": "", "value": "" }, "anchor_state": "resolved", "origin": "user", "annotation_id": "" }
            ]
        })
    }

    #[test]
    fn the_file_is_recognised_by_its_manifest() {
        assert!(looks_like_exchange(&file()));
        assert!(!looks_like_exchange(&json!([{ "type": "book", "title": "x" }])));
        assert!(!looks_like_exchange(&json!({ "manifest": { "format": "something else" } })));
    }

    #[test]
    fn each_source_is_a_document_and_a_case_and_its_excerpts_are_paragraphs_in_it() {
        let batch = parse_exchange(&file(), &ImportOptions::default()).unwrap();
        assert_eq!(batch.sources.len(), 3, "two sources and the document of excerpts that have no source");
        assert_eq!(batch.cases.len(), 2);
        let first = &batch.sources[0];
        assert!(first.name.starts_with("Hernandez & Li 2021"), "{}", first.name);
        assert!(first.text.contains("Bus reliability and daily stress"));
        assert!(first.text.contains("Excerpts"));
        assert!(first.text.contains("Quotation, page 12\n\u{201c}Riders waited a mean of 14 minutes.\u{201d}\nNote: Compare with our data."), "{}", first.text);
        // The excerpt with two sources is written once, in the first, and says where else it came from.
        assert!(first.text.contains("Paraphrase (place in the source not confirmed; written by an AI tool; also from Okafor 2019 \u{2013} Waiting)\nNote: Both link delay to stress."), "{}", first.text);
        let second = &batch.sources[1];
        assert!(!second.text.contains("Both link delay"), "an excerpt is written only once");
        assert!(!second.text.contains("Excerpts\n"), "a source with no excerpt has no Excerpts heading: {}", second.text);
        // The empty note was left out and said so.
        assert!(batch.sources[0].warnings.iter().any(|w| w.message.contains("neither quoted words nor a note")), "{:?}", batch.sources[0].warnings);
        // Excerpts with no source (or one not in the file) are together in a document of their own.
        let loose = &batch.sources[2];
        assert_eq!(loose.name, "Excerpts with no source");
        assert!(loose.text.contains("A question for the interviews.") && loose.text.contains("From a source that is not in the file."));
        assert!(loose.warnings.iter().any(|w| w.message.contains("named a source that is not in this file")), "{:?}", loose.warnings);
    }

    #[test]
    fn the_ids_and_places_are_kept_so_a_return_can_name_them() {
        let batch = parse_exchange(&file(), &ImportOptions::default()).unwrap();
        let s = &batch.sources[0].settings;
        assert_eq!(s["format"], "prism-family-exchange");
        assert_eq!(s["library_source_id"], "s-1");
        assert_eq!(s["citekey"], "hernandez2021");
        let ex = s["excerpts"].as_array().unwrap();
        assert_eq!(ex.len(), 2);
        assert_eq!(ex[0]["id"], "e-1");
        assert_eq!(ex[0]["annotation_id"], "a-1");
        // The place is real: the document's text at those offsets (in code points) is the excerpt.
        let chars: Vec<char> = batch.sources[0].text.chars().collect();
        let at: String = chars[ex[0]["start"].as_i64().unwrap() as usize..ex[0]["end"].as_i64().unwrap() as usize].iter().collect();
        assert!(at.starts_with("Quotation, page 12") && at.ends_with("Compare with our data."), "{at}");
        let case = &batch.cases[0];
        assert!(case.attrs.contains(&("Library source id".to_string(), "s-1".to_string())));
        assert!(case.attrs.contains(&("Citation key".to_string(), "hernandez2021".to_string())));
        assert!(case.attrs.contains(&("Excerpts taken".to_string(), "2".to_string())));
        assert!(case.attrs.contains(&("Year".to_string(), "2021".to_string())));
        assert!(batch.cases[1].attrs.contains(&("Excerpts taken".to_string(), "0".to_string())), "a source whose only excerpt had no words says 0");
    }

    #[test]
    fn a_newer_major_version_or_an_empty_file_is_refused_in_words() {
        let mut newer = file();
        newer["manifest"]["version"] = json!("2.0");
        let e = parse_exchange(&newer, &ImportOptions::default()).unwrap_err();
        assert!(e.to_string().contains("version 2.0") && e.to_string().contains("Prism Reference Manager 0.1.2"), "{e}");
        let mut minor = file();
        minor["manifest"]["version"] = json!("1.3");
        assert!(parse_exchange(&minor, &ImportOptions::default()).is_ok(), "a later minor version is read");
        let empty = json!({ "manifest": { "format": "prism-family-exchange", "version": "1.0" }, "sources": [], "excerpts": [] });
        assert!(parse_exchange(&empty, &ImportOptions::default()).is_err());
    }

    #[test]
    fn two_sources_that_would_be_named_alike_are_told_apart() {
        let mut f = file();
        let twin = f["sources"][0].clone();
        f["sources"].as_array_mut().unwrap().push({
            let mut t = twin;
            t["id"] = json!("s-3");
            t["citekey"] = json!("hernandez2021b");
            t
        });
        let batch = parse_exchange(&f, &ImportOptions::default()).unwrap();
        let names: Vec<&str> = batch.cases.iter().map(|c| c.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "{names:?}");
    }
}
