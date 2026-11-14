//! The report: sections made from the project's own records, in one Word document. Nothing is written that the project does not hold: a
//! figure comes from a count, a sentence from a record, and a section with nothing to say says so instead of inventing something.

use std::collections::BTreeMap;

use prismqda_engine::{ExploreScope, Project, ProjectInfo, SegmentFilter};
use serde::Deserialize;
use serde_json::Value;

use super::{codebook_blocks, codebook_lines, segment_blocks, today, Block, CodebookOptions, ExportError, Pictures, Run};

/// A part of the report, in the order the report puts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportSection {
    /// What the project is and how much is in it.
    Study,
    /// The documents, with their kinds and how much each holds.
    Documents,
    /// Every active code with its definition, rules, and examples.
    Codebook,
    /// The coded passages, grouped by code, with areas of pictures cropped out of their pictures.
    Passages,
    /// The agreement between coders, as each run reported it.
    Agreement,
    /// The units coders coded differently, and what a person decided for each, with the reason.
    Adjudication,
    /// The memos, oldest first.
    Memos,
    /// The paragraph about the use of AI that a methods section needs.
    AiUse,
    /// A draft methods appendix built from the records.
    Methods,
    /// The themes: each with where it stands, its definition, the codes under it with their counts, and quotations that show it.
    Themes,
    /// Where the data is kept, whether it is encrypted, the offline lock, what was shared with an AI service, what is marked do not share, whether names were replaced, who coded, and the audit trail: the facts an ethics board asks for.
    Ethics,
    /// The design of a mixed methods study, where and how the two kinds of data were brought together, and the triangulation matrix.
    MixedMethods,
    /// What was done and when: notes, imports, runs of agreement, and the like.
    Timeline,
}

impl ReportSection {
    /// What a report holds when no choice is made.
    pub fn defaults() -> Vec<ReportSection> {
        vec![ReportSection::Study, ReportSection::Documents, ReportSection::Codebook, ReportSection::Agreement, ReportSection::AiUse, ReportSection::Methods]
    }
}

/// What to put in a report.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReportOptions {
    /// The title; the project's name is used when absent or empty.
    #[serde(default)]
    pub title: Option<String>,
    /// The sections to include; `ReportSection::defaults()` when absent.
    #[serde(default)]
    pub sections: Option<Vec<ReportSection>>,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A list in words: "a", "a and b", "a, b, and c".
fn say_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        _ => format!("{}, and {}", items[..items.len() - 1].join(", "), items[items.len() - 1]),
    }
}

/// The date part of a time stamp (`2026-11-07T12:00:00Z` is `2026-11-07`).
fn day(ts: &str) -> &str {
    ts.get(..10).unwrap_or(ts)
}

fn lines_of(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty())
}

fn study_blocks(project: &Project, info: &ProjectInfo) -> Result<Vec<Block>, ExportError> {
    let cases = project.list_cases()?.len();
    let memos = project.list_memos(None, None)?.len();
    let mut rows: Vec<Vec<String>> = vec![vec!["Project".into(), info.name.clone()]];
    if !info.method_template.trim().is_empty() {
        rows.push(vec!["Method template".into(), info.method_template.clone()]);
    }
    if !info.research_questions.trim().is_empty() {
        rows.push(vec!["Research questions".into(), info.research_questions.trim().to_string()]);
    }
    rows.push(vec!["Coding is done on".into(), if info.coding_mode == "unit" { "whole units".to_string() } else { "any stretch of words".to_string() }]);
    rows.push(vec!["Documents".into(), info.sources.to_string()]);
    rows.push(vec!["Units".into(), info.units.to_string()]);
    rows.push(vec!["Cases".into(), cases.to_string()]);
    rows.push(vec!["Codes".into(), info.codes.to_string()]);
    rows.push(vec!["Coders".into(), info.coders.to_string()]);
    rows.push(vec!["Codings of words".into(), info.codings.to_string()]);
    rows.push(vec!["Memos".into(), memos.to_string()]);
    Ok(vec![Block::Heading(2, "The study".into()), Block::Table { header: vec!["What".into(), "Count or setting".into()], rows }])
}

fn documents_blocks(project: &Project) -> Result<Vec<Block>, ExportError> {
    let sources = project.list_sources()?;
    let mut blocks = vec![Block::Heading(2, "Documents".into())];
    if sources.is_empty() {
        blocks.push(Block::paragraph("The project holds no documents."));
        return Ok(blocks);
    }
    blocks.push(Block::paragraph(format!("{} in all.", plural(sources.len(), "document", "documents"))));
    let rows: Vec<Vec<String>> = sources
        .iter()
        .map(|s| {
            let kind = if s.image.is_some() { "picture".to_string() } else { s.source_type.clone() };
            vec![s.name.clone(), kind, s.folder.clone(), s.characters.to_string(), s.units.to_string(), s.codings.to_string()]
        })
        .collect();
    blocks.push(Block::Table { header: ["Name", "Kind", "Folder", "Characters", "Units", "Codings"].map(String::from).to_vec(), rows });
    Ok(blocks)
}

fn codebook_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let lines = codebook_lines(project, &CodebookOptions { include_inactive: false })?;
    let mut blocks = vec![Block::Heading(2, "Codebook".into())];
    if lines.is_empty() {
        blocks.push(Block::paragraph("The codebook has no active codes."));
        return Ok(blocks);
    }
    blocks.push(Block::paragraph(format!("{} in the codebook.", plural(lines.len(), "active code", "active codes"))));
    blocks.extend(codebook_blocks(&lines, 3));
    Ok(blocks)
}

fn passages_section(project: &Project) -> Result<(Vec<Block>, Vec<super::Media>), ExportError> {
    let segments = project.coded_segments(&SegmentFilter::default())?;
    let pictures = Pictures::gather(project, &segments)?;
    let mut blocks = vec![Block::Heading(2, "Coded passages".into())];
    if segments.is_empty() {
        blocks.push(Block::paragraph("Nothing has been coded."));
        return Ok((blocks, pictures.media));
    }
    blocks.push(Block::paragraph(format!("{} grouped by code.", plural(segments.len(), "coded segment", "coded segments"))));
    blocks.extend(segment_blocks(project, &segments, &pictures, 3)?);
    Ok((blocks, pictures.media))
}

fn agreement_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let runs = project.list_reliability_runs()?;
    let mut blocks = vec![Block::Heading(2, "Agreement between coders".into())];
    if runs.is_empty() {
        blocks.push(Block::paragraph("No agreement between coders was measured in this project."));
        return Ok(blocks);
    }
    for r in runs.iter().rev() {
        let record = project.get_reliability_run(r.id)?;
        blocks.push(Block::Heading(3, format!("Run {}, {}{}", r.id, day(&r.created_at), if r.archived { " (archived)" } else { "" })));
        for line in lines_of(&record.text) {
            blocks.push(Block::paragraph(line));
        }
    }
    Ok(blocks)
}

fn adjudication_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let decisions = project.decisions()?;
    let mut blocks = vec![Block::Heading(2, "Adjudicated decisions".into())];
    if decisions.is_empty() {
        blocks.push(Block::paragraph("No unit that coders coded differently has been settled."));
        return Ok(blocks);
    }
    blocks.push(Block::paragraph(format!("{} settled. Each decision is kept apart from every coder's own coding, which was not changed.", plural(decisions.len(), "unit was", "units were"))));
    let rows: Vec<Vec<String>> = decisions
        .iter()
        .map(|d| {
            let codes = if d.decision.codes.is_empty() { "No code applies".to_string() } else { d.decision.codes.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join(", ") };
            vec![d.source_name.clone(), d.text.clone(), codes, d.decision.reason.clone(), d.decision.adjudicator_name.clone(), day(&d.decision.created_at).to_string()]
        })
        .collect();
    blocks.push(Block::Table { header: ["Document", "Words", "Final codes", "Reason", "Decided by", "Date"].map(String::from).to_vec(), rows });
    Ok(blocks)
}

fn memos_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let mut memos = project.list_memos(None, None)?;
    memos.sort_by_key(|m| m.id);
    let sources: BTreeMap<i64, String> = project.list_sources()?.into_iter().map(|s| (s.id, s.name)).collect();
    let codes: BTreeMap<i64, String> = project.list_codes()?.into_iter().map(|c| (c.id, c.name)).collect();
    let cases: BTreeMap<i64, String> = project.list_cases()?.into_iter().map(|c| (c.id, c.name)).collect();
    let mut blocks = vec![Block::Heading(2, "Memos".into())];
    if memos.is_empty() {
        blocks.push(Block::paragraph("No memo has been written."));
        return Ok(blocks);
    }
    for m in &memos {
        let about = match (m.scope.as_str(), m.target_id) {
            ("project", _) => "the project".to_string(),
            ("source", Some(id)) => format!("the document {}", sources.get(&id).cloned().unwrap_or_else(|| "that was removed".into())),
            ("code", Some(id)) => format!("the code {}", codes.get(&id).cloned().unwrap_or_else(|| "that was removed".into())),
            ("case", Some(id)) => format!("the case {}", cases.get(&id).cloned().unwrap_or_else(|| "that was removed".into())),
            ("coding", Some(id)) if id < 0 => "an area of a picture".to_string(),
            ("coding", _) => "a coding".to_string(),
            ("phase", _) => format!("the phase {}", m.phase.clone().unwrap_or_default()),
            _ => "no one thing".to_string(),
        };
        let kind = match m.kind.as_str() {
            "analytic" => "Analytic memo",
            "reflexive" => "Reflexive memo",
            "method_decision" => "Method decision",
            "todo" => "To do",
            other => other,
        };
        blocks.push(Block::Heading(3, if m.title.trim().is_empty() { "Untitled memo".to_string() } else { m.title.trim().to_string() }));
        blocks.push(Block::Paragraph(vec![Run::italic(format!("{kind} about {about}, written {}.", day(&m.created_at)))]));
        for para in m.body.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
            blocks.push(Block::paragraph(para));
        }
    }
    Ok(blocks)
}

fn ai_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let mut blocks = vec![Block::Heading(2, "Use of AI".into())];
    for line in lines_of(&project.ai_disclosure()?) {
        blocks.push(Block::paragraph(line));
    }
    Ok(blocks)
}

/// What a run of agreement said, in one sentence, from the report the engine kept.
fn agreement_sentence(date: &str, report: &Value) -> String {
    let names: Vec<String> = report["coders"]
        .as_array()
        .map(|a| a.iter().map(|c| format!("{}{}", c["name"].as_str().unwrap_or("a coder"), if c["role"] == "model" { " (an AI model)" } else { "" })).collect())
        .unwrap_or_default();
    let units = report["units_in_sample"].as_u64().unwrap_or(0) as usize;
    let how = if report["mode"] == "per_code" { "code by code" } else { "with one code for each unit" };
    let term = if report["reporting_term"] == "concordance" { "concordance" } else { "intercoder reliability" };
    let mut s = format!("On {date}, agreement between {} was measured on {} {how}, and is called {term}.", say_list(&names), plural(units, "unit", "units"));
    if let Some(alpha) = report["overall"]["krippendorff_alpha"].as_f64() {
        s.push_str(&format!(" Krippendorff's alpha was {alpha:.3}"));
        let iv = &report["intervals"]["krippendorff_alpha"];
        if let (Some(low), Some(high), Some(level)) = (iv["low"].as_f64(), iv["high"].as_f64(), iv["level"].as_f64()) {
            s.push_str(&format!(" ({:.0}% interval {low:.3} to {high:.3})", level * 100.0));
        }
        s.push('.');
    } else if let Some(codes) = report["by_code"].as_array() {
        let below = codes.iter().filter(|c| c["below_target"] == true).count();
        let target = report["target"].as_f64().unwrap_or(0.8);
        s.push_str(&format!(" {} of {} were below the target of {target:.2}.", below, plural(codes.len(), "code", "codes")));
    }
    s
}

/// The themes with what they hold (decision 167), each with the passages that show it.
fn themes_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let overview = project.theme_overview(&ExploreScope::default())?;
    let mut blocks = vec![Block::Heading(2, "Themes".into())];
    if overview.themes.is_empty() {
        blocks.push(Block::paragraph("The codebook has no theme yet."));
        return Ok(blocks);
    }
    let count = |stage: &str| overview.themes.iter().filter(|t| t.stage == stage).count();
    blocks.push(Block::paragraph(format!(
        "{} in all: {} candidate, {} reviewed, {} defined. A theme is a code that groups other codes; the passages counted are those coded with a code under it, each once.",
        plural(overview.themes.len(), "theme", "themes"),
        count("candidate"),
        count("reviewed"),
        count("defined")
    )));
    for theme in &overview.themes {
        blocks.push(Block::Heading(3, format!("{} ({})", crate::scrub::text(&theme.name), theme.stage)));
        if theme.definition.trim().is_empty() {
            blocks.push(Block::paragraph("No definition has been written for this theme yet."));
        } else {
            for line in lines_of(&theme.definition) {
                blocks.push(Block::paragraph(crate::scrub::text(line).into_owned()));
            }
        }
        if !theme.inclusion_criteria.trim().is_empty() {
            blocks.push(Block::paragraph(format!("Include when: {}", crate::scrub::text(theme.inclusion_criteria.trim()))));
        }
        if !theme.exclusion_criteria.trim().is_empty() {
            blocks.push(Block::paragraph(format!("Do not include when: {}", crate::scrub::text(theme.exclusion_criteria.trim()))));
        }
        blocks.push(Block::paragraph(format!(
            "{} in {} and {}.",
            plural(theme.passages as usize, "passage", "passages"),
            plural(theme.documents as usize, "document", "documents"),
            plural(theme.cases as usize, "case", "cases")
        )));
        if theme.codes.is_empty() {
            blocks.push(Block::paragraph("No code is under this theme yet."));
        } else {
            let rows: Vec<Vec<String>> = theme.codes.iter().map(|c| vec![crate::scrub::text(&c.name).into_owned(), c.passages.to_string(), c.documents.to_string()]).collect();
            blocks.push(Block::Table { header: vec!["Code".into(), "Passages".into(), "Documents".into()], rows });
        }
        for q in &theme.quotes {
            blocks.push(Block::Quote {
                text: crate::scrub::text(q.text.trim()).into_owned(),
                source: Some(format!("{}, coded {}{}", crate::scrub::text(&q.source_name), crate::scrub::text(&q.code_name), if q.example { " (marked as an example)" } else { "" })),
            });
        }
    }
    if !overview.unassigned.is_empty() {
        let names: Vec<String> = overview.unassigned.iter().map(|c| crate::scrub::text(&c.name).into_owned()).collect();
        blocks.push(Block::Heading(3, "Codes under no theme".into()));
        blocks.push(Block::paragraph(format!("{}: {}.", plural(names.len(), "code is not under any theme yet", "codes are not under any theme yet"), say_list(&names))));
    }
    Ok(blocks)
}

/// The facts an ethics board asks for, read from the project and nowhere else (decision 165).
fn ethics_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let f = project.privacy_facts()?;
    let mut blocks = vec![Block::Heading(2, "Ethics and data handling".into())];
    blocks.push(Block::Paragraph(vec![Run::italic(
        "This summary is made from the records of the project and nothing else. It states what the project says about where the data is kept, how it is protected, and what has been done with it inside PrismQDA. What a person did outside the program is not recorded, and so is not stated here.",
    )]));

    let mut rows: Vec<Vec<String>> = vec![
        vec!["Project".into(), f.project_name.clone()],
        vec!["Created".into(), day(&f.created_at).to_string()],
        vec!["Where it is kept".into(), format!("One file on the computer of the person using it: {}", f.project_path.display())],
        vec!["Backups".into(), f.backup_folder.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "beside the project file".to_string())],
        vec![
            "Encrypted at rest".into(),
            if f.encrypted { "Yes. Every page of the project file is encrypted with the passphrase the project was given (AES-256 in the SQLCipher 4 scheme), and so are its backups. A lost passphrase cannot be recovered.".to_string() } else { "No. The file is not encrypted; it is protected only by the computer and the folder it is kept in.".to_string() },
        ],
        vec![
            "Offline lock".into(),
            if f.offline_lock { "On. While it is on, no prompt packet can be built for an AI service from this project.".to_string() } else { "Off.".to_string() },
        ],
        vec!["Network".into(), "PrismQDA connects to nothing on its own: it has no account, no telemetry, and no license server. It looks for a newer version of the program only when a person presses Check for updates in Settings, and then asks one address over a secure connection (the request carries the version of the program and the kind of computer, as every web request does), and it does not do even that while a project is locked offline. It never sends a project, or a word of one, anywhere.".into()],
        vec!["What the project holds".into(), format!("{}, {}, {}", plural(f.documents as usize, "document", "documents"), plural(f.units as usize, "unit", "units"), plural(f.codes as usize, "code", "codes"))],
    ];
    rows.push(vec!["AI setting".into(), if f.ai_mode == "off" { "Off: no prompt packet can be built.".to_string() } else { "Prompt packets can be built (text the person takes to an AI service of their own choosing; PrismQDA calls no model).".to_string() }]);
    blocks.push(Block::Table { header: vec!["What".into(), "Where it stands".into()], rows });

    blocks.push(Block::Heading(3, "What was shared with an AI service".into()));
    if f.ai_runs.is_empty() {
        blocks.push(Block::paragraph("No prompt packet was ever built for an AI service from this project."));
    } else {
        blocks.push(Block::paragraph(format!(
            "{} built. A packet is text; PrismQDA does not send it. Each row says when it was built, how many passages it held, and whether an answer was pasted back, with the name of the model the person said they used.",
            plural(f.ai_runs.len(), "prompt packet was", "prompt packets were")
        )));
        let rows: Vec<Vec<String>> = f
            .ai_runs
            .iter()
            .map(|r| {
                vec![
                    day(&r.created_at).to_string(),
                    plural(r.passages, "passage", "passages"),
                    if r.model.trim().is_empty() { "not named".to_string() } else { r.model.clone() },
                    if r.answered { format!("answered; {} suggested, {} accepted, {} rejected", r.suggested, r.accepted, r.rejected) } else { "not answered".to_string() },
                ]
            })
            .collect();
        blocks.push(Block::Table { header: vec!["Built".into(), "Passages".into(), "Model".into(), "Answer".into()], rows });
    }

    blocks.push(Block::Heading(3, "What is kept out".into()));
    if f.not_shared.is_empty() && f.units_not_shared == 0 {
        blocks.push(Block::paragraph("Nothing is marked do not share."));
    } else {
        let docs: Vec<String> = f.not_shared.iter().filter(|n| n.kind == "document").map(|n| crate::scrub::text(&n.name).into_owned()).collect();
        let codes: Vec<String> = f.not_shared.iter().filter(|n| n.kind == "code").map(|n| crate::scrub::text(&n.name).into_owned()).collect();
        if !docs.is_empty() {
            blocks.push(Block::paragraph(format!("{} marked do not share, so never in a prompt packet: {}.", plural(docs.len(), "document is", "documents are"), say_list(&docs))));
        }
        if !codes.is_empty() {
            blocks.push(Block::paragraph(format!("{} marked do not share, so left out of every prompt packet: {}.", plural(codes.len(), "code is", "codes are"), say_list(&codes))));
        }
        if f.units_not_shared > 0 {
            blocks.push(Block::paragraph(format!("{} marked do not share.", plural(f.units_not_shared as usize, "unit is", "units are"))));
        }
    }

    blocks.push(Block::Heading(3, "Names".into()));
    if f.names_rules == 0 {
        blocks.push(Block::paragraph("No list of names to replace has been made, so nothing was pseudonymized or redacted in PrismQDA."));
    } else {
        blocks.push(Block::paragraph(format!(
            "A list of {} to replace is kept in the project (the words themselves are not repeated here). It is {} what is exported.{}",
            plural(f.names_rules as usize, "name or term", "names and terms"),
            if f.names_applied_to_exports { "applied to" } else { "not applied to" },
            match &f.names_last_applied {
                Some(a) => format!(" Names were replaced for good in the project on {}: {} in {}.", day(&a.at), plural(a.replacements as usize, "replacement", "replacements"), plural(a.documents as usize, "document", "documents")),
                None => " Names have not been replaced in the project itself.".to_string(),
            }
        )));
    }

    blocks.push(Block::Heading(3, "Who coded".into()));
    let rows: Vec<Vec<String>> = f.coders.iter().map(|c| vec![crate::scrub::text(&c.name).into_owned(), if c.role == "model" { "an AI model".to_string() } else { "a person".to_string() }]).collect();
    if rows.is_empty() {
        blocks.push(Block::paragraph("No coder is recorded."));
    } else {
        blocks.push(Block::Table { header: vec!["Coder".into(), "Is".into()], rows });
    }

    blocks.push(Block::Heading(3, "The record of what was done".into()));
    blocks.push(Block::paragraph(format!(
        "The project keeps an append-only record of every change: who made it and when, and nothing in it can be edited or removed. It holds {} so far{}.",
        plural(f.events as usize, "entry", "entries"),
        match (&f.first_event_at, &f.last_event_at) {
            (Some(a), Some(b)) => format!(", from {} to {}", day(a), day(b)),
            _ => String::new(),
        }
    )));
    Ok(blocks)
}


fn methods_section(project: &Project, info: &ProjectInfo) -> Result<Vec<Block>, ExportError> {
    let sources = project.list_sources()?;
    let codes = project.list_codes()?;
    let coders = project.list_coders()?;
    let cases = project.list_cases()?;
    let events = project.events(100_000)?;
    let mut blocks = vec![Block::Heading(2, "Methods appendix (draft)".into())];
    blocks.push(Block::Paragraph(vec![Run::italic(
        "A draft built only from the project's records. It cannot know why choices were made, so read it, and add your reasons and anything that was done outside the program.",
    )]));

    // Data.
    blocks.push(Block::Heading(3, "Data".into()));
    if sources.is_empty() {
        blocks.push(Block::paragraph("The project holds no documents."));
    } else {
        let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
        for s in &sources {
            *kinds.entry(if s.image.is_some() { "picture" } else { s.source_type.as_str() }).or_default() += 1;
        }
        let described: Vec<String> = kinds
            .iter()
            .map(|(k, n)| match *k {
                "document" => plural(*n, "document", "documents"),
                "transcript" => plural(*n, "transcript", "transcripts"),
                "survey" => plural(*n, "survey file", "survey files"),
                "social" => plural(*n, "social media export", "social media exports"),
                "pdf" => plural(*n, "PDF", "PDFs"),
                "picture" => plural(*n, "picture", "pictures"),
                _ => plural(*n, "other file", "other files"),
            })
            .collect();
        let characters: i64 = sources.iter().map(|s| s.characters).sum();
        blocks.push(Block::paragraph(format!(
            "The data are {}, {} characters of text in all, divided into {} for coding.{}",
            say_list(&described),
            characters,
            plural(info.units as usize, "analysis unit", "analysis units"),
            if cases.is_empty() { String::new() } else { format!(" They are arranged in {}.", plural(cases.len(), "case", "cases")) }
        )));
    }

    // Coding.
    blocks.push(Block::Heading(3, "Coding".into()));
    let active: Vec<_> = codes.iter().filter(|c| c.status == "active").collect();
    let themes = active.iter().filter(|c| c.kind == "theme").count();
    let defined = active.iter().filter(|c| !c.definition.trim().is_empty()).count();
    let snapshots = events.iter().filter(|e| e.action == "codebook.snapshot").count();
    blocks.push(Block::paragraph(format!(
        "Coding was done on {}. The codebook has {}{}; {} of them {} a written definition.{} {} {} made in all.",
        if info.coding_mode == "unit" { "whole units" } else { "any stretch of words" },
        plural(active.len(), "active code", "active codes"),
        if themes > 0 { format!(" ({} of them {})", themes, if themes == 1 { "a theme that groups others" } else { "themes that group others" }) } else { String::new() },
        defined,
        if defined == 1 { "has" } else { "have" },
        if snapshots > 0 { format!(" The codebook was saved as a version {} time{}.", snapshots, if snapshots == 1 { "" } else { "s" }) } else { String::new() },
        info.codings,
        if info.codings == 1 { "coding of words was" } else { "codings of words were" },
    )));

    // Coders.
    blocks.push(Block::Heading(3, "Coders".into()));
    if coders.is_empty() {
        blocks.push(Block::paragraph("No coder is recorded."));
    } else {
        let named: Vec<String> = coders
            .iter()
            .map(|c| {
                if c.role == "model" {
                    format!(
                        "{} (an AI model: {}{})",
                        c.name,
                        c.model_name.clone().unwrap_or_default(),
                        c.model_version.as_ref().map(|v| format!(" version {v}")).unwrap_or_default()
                    )
                } else {
                    format!("{} (a person)", c.name)
                }
            })
            .collect();
        blocks.push(Block::paragraph(format!("{} {}: {}.", plural(coders.len(), "coder took part", "coders took part"), "in the project", say_list(&named))));
    }

    // Agreement.
    blocks.push(Block::Heading(3, "Agreement between coders".into()));
    let runs = project.list_reliability_runs()?;
    if runs.is_empty() {
        blocks.push(Block::paragraph("Agreement between coders was not measured."));
    } else {
        for r in runs.iter().rev() {
            let record = project.get_reliability_run(r.id)?;
            blocks.push(Block::paragraph(agreement_sentence(day(&r.created_at), &record.report)));
        }
    }

    // Adjudication.
    let decisions = project.decisions()?;
    if !decisions.is_empty() {
        blocks.push(Block::Heading(3, "Adjudication".into()));
        let mut people: Vec<String> = Vec::new();
        for d in &decisions {
            if !people.contains(&d.decision.adjudicator_name) {
                people.push(d.decision.adjudicator_name.clone());
            }
        }
        blocks.push(Block::paragraph(format!(
            "{} that coders had coded differently {} settled by {}, each with a written reason. The decisions are kept apart from every coder's own coding, which was not changed.",
            plural(decisions.len(), "unit", "units"),
            if decisions.len() == 1 { "was" } else { "were" },
            say_list(&people)
        )));
    }

    // AI.
    blocks.push(Block::Heading(3, "Use of AI".into()));
    for line in lines_of(&project.ai_disclosure()?) {
        blocks.push(Block::paragraph(line));
    }

    // Analysis.
    let queries = project.list_saved_queries()?;
    if !queries.is_empty() {
        blocks.push(Block::Heading(3, "Analysis".into()));
        let names: Vec<String> = queries.iter().map(|q| format!("\u{201C}{}\u{201D}", q.name)).collect();
        blocks.push(Block::paragraph(format!("{} kept in the project: {}.", plural(queries.len(), "query was", "queries were"), say_list(&names))));
    }

    // Software.
    blocks.push(Block::Heading(3, "Software".into()));
    blocks.push(Block::paragraph(format!("The analysis was done in PrismQDA {}, a free program that keeps all the data on the researcher's own computer.", env!("CARGO_PKG_VERSION"))));
    Ok(blocks)
}

fn timeline_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let events = project.events(100_000)?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    for e in &events {
        let after = e.after.clone().unwrap_or(Value::Null);
        let text = match e.action.as_str() {
            "project.create" => "Started the project.".to_string(),
            "annotation" => after["note"].as_str().unwrap_or("").to_string(),
            "source.import" => format!("Imported {}.", after["name"].as_str().unwrap_or("a document")),
            "reliability.run" => format!("Measured agreement between coders on {} units.", after["units"].as_u64().unwrap_or(0)),
            "reliability.archive" => "Archived a run of agreement.".to_string(),
            "adjudication.set" => "Settled a unit that coders had coded differently.".to_string(),
            "project.merge" => format!(
                "Merged a copy of the project ({}): {} added, {} added.",
                after["from"].as_str().unwrap_or("a copy"),
                plural(after["documents_added"].as_u64().unwrap_or(0) as usize, "document", "documents"),
                plural(after["codings_added"].as_u64().unwrap_or(0) as usize, "coding", "codings")
            ),
            "ai.packet" => format!("Built a prompt packet of {}.", plural(after["passages"].as_u64().unwrap_or(0) as usize, "passage", "passages")),
            "codebook.snapshot" => "Saved a version of the codebook.".to_string(),
            "project.ai_mode" => "Changed whether AI may be used.".to_string(),
            "query.save" => format!("Saved the query {}.", after["name"].as_str().unwrap_or("")),
            "phase.move" => "Moved on to another phase of the method.".to_string(),
            _ => continue,
        };
        if !text.trim().is_empty() {
            rows.push(vec![day(&e.ts).to_string(), text]);
        }
    }
    let mut blocks = vec![Block::Heading(2, "Decision timeline".into())];
    if rows.is_empty() {
        blocks.push(Block::paragraph("The project holds no milestones or notes."));
    } else {
        blocks.push(Block::Table { header: vec!["Date".into(), "What happened".into()], rows });
    }
    Ok(blocks)
}


/// The design in words, as a clause.
fn design_words(kind: &str) -> &'static str {
    match kind {
        "convergent" => "a convergent design: the quantitative and the qualitative data are collected and analysed separately and then merged",
        "explanatory_sequential" => "an explanatory sequential design: the quantitative strand comes first and the qualitative strand is used to explain its results",
        "exploratory_sequential" => "an exploratory sequential design: the qualitative strand comes first and what it finds is used to build the quantitative strand",
        "embedded" => "an embedded design: one strand is carried out within the other",
        _ => "a mixed methods design",
    }
}

fn stage_words(stage: &str) -> &'static str {
    match stage {
        "design" => "design",
        "data_collection" => "data collection",
        "analysis" => "analysis",
        "interpretation" => "interpretation",
        "reporting" => "reporting",
        _ => "",
    }
}

fn how_words(how: &str) -> &'static str {
    match how {
        "connecting" => "connecting",
        "building" => "building",
        "merging" => "merging",
        "embedding" => "embedding",
        "transforming" => "transforming one kind of data into the other",
        _ => "",
    }
}

fn mark_words(mark: &str) -> &'static str {
    match mark {
        "convergence" => "convergence",
        "complementarity" => "complementarity",
        "discordance" => "discordance",
        _ => "not marked",
    }
}

fn number_words(n: f64) -> String {
    format!("{n:.2}")
}

/// What a triangulation row's code and variable show in the data now, in a line.
fn evidence_words(row: &prismqda_engine::TriangulationRow) -> String {
    use prismqda_engine::TypologyCell;
    let Some(e) = &row.evidence else { return String::new() };
    let mut parts = Vec::new();
    if let Some(w) = &e.words {
        parts.push(format!("{} from {}", plural(w.excerpts, "excerpt", "excerpts"), plural(w.cases, "case", "cases")));
    }
    if let Some(n) = &e.numbers {
        let name = &n.column.name;
        match (&n.with, &n.without) {
            (TypologyCell::Scale { n: a, mean: Some(ma), .. }, Some(TypologyCell::Scale { n: b, mean: Some(mb), .. })) => {
                parts.push(format!("{name}: mean {} (n = {a}) where the code is, {} (n = {b}) in the other cases", number_words(*ma), number_words(*mb)));
            }
            (TypologyCell::Scale { n: a, mean: Some(ma), .. }, None) => parts.push(format!("{name}: mean {} (n = {a}) in all the cases that hold text", number_words(*ma))),
            (TypologyCell::Categories { n: a, counts }, other) => {
                let list = |counts: &[usize]| n.column.categories.iter().zip(counts).map(|(k, c)| format!("{} {c}", k.label)).collect::<Vec<_>>().join(", ");
                let mut s = format!("{name}: {} (n = {a})", list(counts));
                if let Some(TypologyCell::Categories { n: b, counts: others }) = other {
                    s.push_str(&format!(" where the code is; {} (n = {b}) in the other cases", list(others)));
                }
                parts.push(s);
            }
            _ => {}
        }
    }
    parts.join("; ")
}

fn mixed_section(project: &Project) -> Result<Vec<Block>, ExportError> {
    let design = project.mixed_design()?;
    let log = project.integration_log()?;
    let rows = project.triangulation()?;
    let mut blocks = vec![Block::Heading(2, "Mixed methods".into())];
    if design.is_none() && log.is_empty() && rows.is_empty() {
        blocks.push(Block::paragraph("The project holds no mixed methods design, integration log, or triangulation matrix."));
        return Ok(blocks);
    }
    if let Some(d) = &design {
        blocks.push(Block::Heading(3, "Design".into()));
        let priority = match d.priority.as_str() {
            "quantitative" => " The quantitative strand has priority.",
            "qualitative" => " The qualitative strand has priority.",
            _ => " The two strands have equal priority.",
        };
        blocks.push(Block::paragraph(format!("The study follows {}.{priority}", design_words(&d.kind))));
        if !d.purpose.trim().is_empty() {
            blocks.push(Block::paragraph(format!("Purpose of mixing the two kinds of data: {}", d.purpose.trim())));
        }
    }
    if !log.is_empty() {
        blocks.push(Block::Heading(3, "Where and how the two kinds of data were brought together".into()));
        let rows: Vec<Vec<String>> = log.iter().map(|e| vec![e.on_date.clone(), stage_words(&e.stage).to_string(), how_words(&e.how).to_string(), e.what.clone(), e.why.clone()]).collect();
        blocks.push(Block::Table { header: vec!["Date".into(), "Stage".into(), "How".into(), "What was done".into(), "Why".into()], rows });
    }
    if !rows.is_empty() {
        blocks.push(Block::Heading(3, "Triangulation of the findings".into()));
        let count = |mark: &str| rows.iter().filter(|r| r.mark == mark).count();
        blocks.push(Block::paragraph(format!(
            "The findings of the two strands were compared on {}. The researcher marked {} as convergence, {} as complementarity, and {} as discordance; {} not marked.",
            plural(rows.len(), "topic", "topics"),
            count("convergence"),
            count("complementarity"),
            count("discordance"),
            count("")
        )));
        let table: Vec<Vec<String>> = rows
            .iter()
            .map(|r| vec![r.topic.clone(), r.qualitative.clone(), r.quantitative.clone(), evidence_words(r), mark_words(&r.mark).to_string(), r.note.clone()])
            .collect();
        blocks.push(Block::Table {
            header: vec!["Topic".into(), "Qualitative strand".into(), "Quantitative strand".into(), "In the data".into(), "Mark".into(), "Researcher's note".into()],
            rows: table,
        });
    }
    Ok(blocks)
}

/// The report as `.docx` bytes: a title, a line that says where it came from, and the sections asked for, in a fixed order.
pub fn export_report(project: &Project, options: &ReportOptions) -> Result<Vec<u8>, ExportError> {
    let info = project.info()?;
    let wanted = options.sections.clone().unwrap_or_else(ReportSection::defaults);
    if wanted.is_empty() {
        return Err(ExportError::Invalid("choose at least one section for the report".to_string()));
    }
    let has = |s: ReportSection| wanted.contains(&s);
    let title = options.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).unwrap_or_else(|| format!("Report: {}", info.name));
    let mut blocks = vec![
        Block::Heading(1, title.clone()),
        Block::paragraph(format!("Made by PrismQDA on {} from the records of the project {}. Every figure and sentence comes from those records; nothing is added.", today(), info.name)),
    ];
    let mut media = Vec::new();
    if has(ReportSection::Study) {
        blocks.extend(study_blocks(project, &info)?);
    }
    if has(ReportSection::Documents) {
        blocks.extend(documents_blocks(project)?);
    }
    if has(ReportSection::Codebook) {
        blocks.extend(codebook_section(project)?);
    }
    if has(ReportSection::Passages) {
        let (b, m) = passages_section(project)?;
        blocks.extend(b);
        media = m;
    }
    if has(ReportSection::Agreement) {
        blocks.extend(agreement_section(project)?);
    }
    if has(ReportSection::Adjudication) {
        blocks.extend(adjudication_section(project)?);
    }
    if has(ReportSection::Memos) {
        blocks.extend(memos_section(project)?);
    }
    if has(ReportSection::AiUse) {
        blocks.extend(ai_section(project)?);
    }
    if has(ReportSection::Methods) {
        blocks.extend(methods_section(project, &info)?);
    }
    if has(ReportSection::Themes) {
        blocks.extend(themes_section(project)?);
    }
    if has(ReportSection::Ethics) {
        blocks.extend(ethics_section(project)?);
    }
    if has(ReportSection::MixedMethods) {
        blocks.extend(mixed_section(project)?);
    }
    if has(ReportSection::Timeline) {
        blocks.extend(timeline_section(project)?);
    }
    super::write_docx_with_media(&title, &blocks, &media)
}
