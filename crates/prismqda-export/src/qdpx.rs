//! A whole project in the REFI-QDA exchange format (`.qdpx`, decision 159), so it can move to another qualitative analysis program.
//!
//! The file is a ZIP holding `project.qde` (XML, namespace `urn:QDA-XML:project:1.0`) and a flat `sources` folder, where each document,
//! picture, and PDF is a file named by the GUID the XML gives it. Text selections count Unicode code points from 0 with the end left out
//! (the standard's own example makes a selection of 56 characters from 139 to 195), which is how PrismQDA counts, so the offsets of a
//! coding are written as they are.
//!
//! What the format cannot hold is not written, and the report says so: the standard asks a program to tell the researcher, at the point of
//! export and of import, which parts are discarded.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write as _;

use prismqda_engine::{sha256_hex, CodeRow, CodingRow, MemoRow, Project, RegionRow, Shape};
use serde::Serialize;

use crate::xml::escape;
use crate::{scrub, zip_package, ExportError};

/// What went into the file and what did not.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct QdpxReport {
    /// What the file holds, one line each.
    pub carried: Vec<String>,
    /// What the file does not hold, one line each.
    pub left_out: Vec<String>,
}

/// A repeatable identifier in the form `8-4-4-4-12`, made from the project's identifier, the kind of thing, and its number, so exporting
/// the same project twice gives the same identifiers (a code has the same one here as in the QDC file).
pub fn gid(project_uid: &str, kind: &str, id: i64) -> String {
    let hex = sha256_hex(format!("{project_uid}:{kind}:{id}").as_bytes());
    let h = &hex[..32];
    let variant = ['8', '9', 'a', 'b'][usize::from_str_radix(&h[16..17], 16).unwrap_or(0) % 4];
    format!("{}-{}-5{}-{variant}{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[17..20], &h[20..32])
}

/// The time as the schema's `dateTime` wants it, or nothing when it cannot be read.
fn date_time(s: &str) -> Option<String> {
    let s = s.trim().replace(' ', "T");
    let b = s.as_bytes();
    let ok = b.len() >= 19 && b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':' && b[16] == b':' && b[..4].iter().all(u8::is_ascii_digit);
    if !ok {
        return None;
    }
    let base = &s[..19];
    Some(if s.len() > 19 { s.clone() } else { format!("{base}Z") })
}

/// The sentence that opens a description carrying the kind and the units of a document.
pub(crate) const UNIT_MAP_INTRO: &str = "PrismQDA: the kind and the units of this document. ";

/// The kind of a document and where its units are, for its description, when they are not the sentences a plain document is split into.
fn unit_map(project: &Project, source_type: &str, default_unit_kind: &str, source_id: i64, text: &str) -> Result<Option<String>, ExportError> {
    let units = project.list_units(source_id)?;
    let sentences = prismqda_import::segment::units_for(prismqda_engine::UnitKind::Sentence, text);
    let plain = source_type == "document"
        && default_unit_kind == "sentence"
        && units.len() == sentences.len()
        && units.iter().zip(&sentences).all(|(u, d)| u.start == d.start && u.end == d.end && u.kind == "sentence" && u.speaker.is_none() && !u.excluded && u.time_start_ms.is_none());
    if plain {
        return Ok(None);
    }
    let rows: Vec<serde_json::Value> = units.iter().map(|u| serde_json::json!([u.start, u.end, u.kind, u.speaker, u.excluded as i64, u.time_start_ms, u.time_end_ms])).collect();
    Ok(Some(format!("{UNIT_MAP_INTRO}{}", serde_json::json!({ "v": 1, "type": source_type, "unit": default_unit_kind, "units": rows }))))
}

/// How the format writes where a recording is on this computer: `absolute:///C:/folder/file.mp3`.
fn recording_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    if p.starts_with('/') {
        format!("absolute://{p}")
    } else {
        format!("absolute:///{p}")
    }
}

/// Whether a colour is written as the format wants it: a hash and three or six hexadecimal digits.
fn is_rgb(c: &str) -> bool {
    let Some(rest) = c.strip_prefix('#') else { return false };
    (rest.len() == 3 || rest.len() == 6) && rest.bytes().all(|b| b.is_ascii_hexdigit())
}

fn attr(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, " {name}=\"{}\"", escape(value));
}

fn opt_time(out: &mut String, name: &str, value: &str) {
    if let Some(t) = date_time(value) {
        attr(out, name, &t);
    }
}

/// A short single-line name for a selection: its first words.
fn excerpt(chars: &[char], start: usize, end: usize) -> String {
    let end = end.min(chars.len());
    let start = start.min(end);
    let mut s: String = chars[start..end].iter().take(60).map(|c| if c.is_control() { ' ' } else { *c }).collect();
    if end - start > 60 {
        s.push('\u{2026}');
    }
    s.trim().to_string()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The kind of value a variable of cases holds in the file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VarType {
    Text,
    Integer,
    Float,
    Date,
    DateTime,
}

impl VarType {
    fn name(self) -> &'static str {
        match self {
            VarType::Text => "Text",
            VarType::Integer => "Integer",
            VarType::Float => "Float",
            VarType::Date => "Date",
            VarType::DateTime => "DateTime",
        }
    }
}

fn is_integer(v: &str) -> bool {
    let v = v.strip_prefix(['-', '+']).unwrap_or(v);
    !v.is_empty() && v.len() < 18 && v.bytes().all(|b| b.is_ascii_digit())
}

fn is_decimal(v: &str) -> bool {
    let v = v.strip_prefix(['-', '+']).unwrap_or(v);
    let (whole, frac) = v.split_once('.').unwrap_or((v, ""));
    !(whole.is_empty() && frac.is_empty()) && whole.bytes().all(|b| b.is_ascii_digit()) && frac.bytes().all(|b| b.is_ascii_digit()) && v.len() < 40
}

fn is_date(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) && matches!(&v[5..7], "01" | "02" | "03" | "04" | "05" | "06" | "07" | "08" | "09" | "10" | "11" | "12") && (1..=31).contains(&v[8..10].parse::<u32>().unwrap_or(0))
}

fn is_date_time(v: &str) -> bool {
    let b = v.as_bytes();
    let v = v.strip_suffix('Z').unwrap_or(v);
    v.len() == 19 && is_date(&v[..10]) && b[10] == b'T' && v.as_bytes()[11..].iter().enumerate().all(|(i, c)| if i == 2 || i == 5 { *c == b':' } else { c.is_ascii_digit() })
}

/// The type a variable can have given its level and what its values look like (a value that does not fit makes the variable text).
fn var_type(level: &str, values: &[&String]) -> VarType {
    let all = |f: fn(&str) -> bool| !values.is_empty() && values.iter().all(|v| f(v.trim()));
    match level {
        "interval" | "ratio" if all(is_integer) => VarType::Integer,
        "interval" | "ratio" if all(is_decimal) => VarType::Float,
        "date" if all(is_date) => VarType::Date,
        "date" if all(is_date_time) => VarType::DateTime,
        _ => VarType::Text,
    }
}

fn write_code(out: &mut String, uid: &str, code: &CodeRow, children: &HashMap<Option<i64>, Vec<&CodeRow>>, notes: &HashMap<i64, Vec<String>>, depth: usize) {
    let pad = "  ".repeat(depth + 3);
    let _ = write!(out, "{pad}<Code");
    attr(out, "guid", &gid(uid, "code", code.id));
    attr(out, "name", &scrub::text(&code.name));
    attr(out, "isCodable", if code.kind == "theme" { "false" } else { "true" });
    let color = code.color.trim().to_lowercase();
    if color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        attr(out, "color", &color);
    }
    // The description is the definition and the two rules, in the words the QDC file uses.
    let mut description = code.definition.trim().to_string();
    for (label, text) in [("Include when", &code.inclusion_criteria), ("Do not include when", &code.exclusion_criteria)] {
        if !text.trim().is_empty() {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str(&format!("{label}: {}", text.trim()));
        }
    }
    let kids = children.get(&Some(code.id)).map(Vec::as_slice).unwrap_or(&[]);
    let refs = notes.get(&code.id);
    if description.is_empty() && kids.is_empty() && refs.is_none() {
        out.push_str("/>\n");
        return;
    }
    out.push_str(">\n");
    if !description.is_empty() {
        let _ = writeln!(out, "{pad}  <Description>{}</Description>", escape(&description));
    }
    for g in refs.into_iter().flatten() {
        let _ = writeln!(out, "{pad}  <NoteRef targetGUID=\"{g}\"/>");
    }
    for kid in kids {
        write_code(out, uid, kid, children, notes, depth + 1);
    }
    let _ = writeln!(out, "{pad}</Code>");
}

/// The text of a memo's note: its title, if it has one, then the body.
fn note_name(m: &MemoRow) -> String {
    if !m.title.trim().is_empty() {
        return m.title.trim().to_string();
    }
    let first: String = m.body.trim().lines().next().unwrap_or("").chars().take(60).collect();
    if first.is_empty() { format!("Memo {}", m.id) } else { first }
}

/// Builds the file. Refuses while the list of names to replace is on, because the text of every document goes into the file whole and the
/// replacement would change its length, and with it the place of every coding.
pub fn export_qdpx(project: &Project) -> Result<(Vec<u8>, QdpxReport), ExportError> {
    export_qdpx_with(project, &QdpxOptions::default())
}

/// What else goes into a QDPX file besides the project.
#[derive(Debug, Clone, Default)]
pub struct QdpxOptions {
    /// Put the files of the linked recordings inside the file, where the format keeps them, up to a size (a recording is otherwise only named
    /// by where it is on this computer).
    pub include_recordings: bool,
}

/// The most one recording may be, and all of them together, for a file whose bytes are held in memory while it is made.
const MAX_RECORDING_BYTES: u64 = 512 << 20;
const MAX_RECORDINGS_TOTAL: u64 = 1 << 30;

/// The same, with the choices of `QdpxOptions`.
pub fn export_qdpx_with(project: &Project, options: &QdpxOptions) -> Result<(Vec<u8>, QdpxReport), ExportError> {
    if scrub::active() {
        return Err(ExportError::Invalid(
            "A whole project cannot be exported while the list of names to replace is applied to exports: the text of each document goes into the file whole, and replacing a name changes its length and so the place of every coding. Turn the switch off for this export, or apply the list to the project itself first (Privacy > Pseudonymize and redact).".to_string(),
        ));
    }
    let info = project.info()?;
    let uid = info.project_uid.clone();
    let mut report = QdpxReport::default();

    let coders = project.list_coders()?;
    let all_codes = project.list_codes()?;
    let codes: Vec<&CodeRow> = all_codes.iter().filter(|c| c.status != "merged").collect();
    let code_ids: HashSet<i64> = codes.iter().map(|c| c.id).collect();
    let sources = project.list_sources()?;
    let memos = project.list_memos(None, None)?;
    let case_data = project.case_data()?;
    let cases = project.list_cases()?;
    let unit_members = project.case_unit_members()?;
    let code_sets = project.list_code_sets()?;
    // where each linked recording is on this computer
    let media_of: HashMap<_, _> = project.list_media()?.into_iter().map(|m| (m.source_id, m)).collect();

    let final_user = gid(&uid, "user", 0);
    let mut needs_final_user = false;
    let coder_guid = |id: i64| gid(&uid, "user", id);

    // ---- notes: where each memo is attached
    let mut notes_on_code: HashMap<i64, Vec<String>> = HashMap::new();
    let mut notes_on_source: HashMap<i64, Vec<String>> = HashMap::new();
    let mut notes_on_coding: HashMap<i64, Vec<String>> = HashMap::new();
    let mut notes_on_project: Vec<String> = Vec::new();
    let source_ids: HashSet<i64> = sources.iter().map(|s| s.id).collect();
    let case_name: HashMap<i64, &str> = cases.iter().map(|c| (c.id, c.name.as_str())).collect();
    let mut loose_coding_memos = 0usize;
    let mut notes_xml = String::new();
    let mut note_files: Vec<(String, Vec<u8>)> = Vec::new();
    for m in &memos {
        let g = gid(&uid, "memo", m.id);
        let mut name = note_name(m);
        match (m.scope.as_str(), m.target_id) {
            ("code", Some(t)) if code_ids.contains(&t) => notes_on_code.entry(t).or_default().push(g.clone()),
            ("source", Some(t)) if source_ids.contains(&t) => notes_on_source.entry(t).or_default().push(g.clone()),
            ("coding", Some(t)) => notes_on_coding.entry(t).or_default().push(g.clone()),
            ("case", Some(t)) => {
                // The format has no place for a note on a case, so it is a note on the project that says whose it is.
                name = format!("About the case {}: {name}", case_name.get(&t).copied().unwrap_or("?"));
                notes_on_project.push(g.clone());
            }
            _ => notes_on_project.push(g.clone()),
        }
        let _ = write!(notes_xml, "    <Note");
        attr(&mut notes_xml, "guid", &g);
        attr(&mut notes_xml, "name", &scrub::text(&name));
        opt_time(&mut notes_xml, "creationDateTime", &m.created_at);
        opt_time(&mut notes_xml, "modifiedDateTime", &m.updated_at);
        // The words of a note are a file in the package, like the words of a document (the standard allows the text in the element or a path):
        // QualCoder 4.0 reads a memo only from the path, and leaves a note without one out.
        note_files.push((format!("sources/{g}.txt"), m.body.clone().into_bytes()));
        let _ = writeln!(notes_xml, " plainTextPath=\"internal://{g}.txt\"/>");
    }

    // ---- codings, grouped by the words they are on
    let mut files: Vec<(String, Vec<u8>)> = note_files;
    let mut sources_xml = String::new();
    let mut counts = Counts::default();
    let unit_by_source: HashMap<i64, Vec<&prismqda_engine::CaseUnitMember>> = {
        let mut m: HashMap<i64, Vec<&prismqda_engine::CaseUnitMember>> = HashMap::new();
        for u in &unit_members {
            m.entry(u.source_id).or_default().push(u);
        }
        m
    };
    let mut written_codings: HashSet<i64> = HashSet::new();
    let mut recordings_total = 0u64;
    // The element each coded passage is written as (a selection of words, or a picture's area, whose number is negative), so a link can name its ends.
    let mut ends: HashMap<i64, String> = HashMap::new();
    // A link joins selections, not codings. Words with several codings are one selection, so a coding that a link ends at is written as a
    // selection of its own: the link then names that coding, and reading the file back joins the same two codings.
    let linked_codings: HashSet<i64> = project.list_passage_links(None)?.iter().flat_map(|l| [l.from.coding_id, l.to.coding_id]).filter(|id| *id > 0).collect();
    for s in &sources {
        let text = project.source_text(s.id)?;
        let chars: Vec<char> = text.chars().collect();
        let codings: Vec<CodingRow> = project.list_codings(s.id)?;
        let regions: Vec<RegionRow> = project.list_regions(s.id)?;
        // A picture or a PDF of another kind than a document has no place in the file for its kind and units (a text source carries them in its description).
        let typed = s.default_unit_kind != "sentence" || s.source_type == "survey" || s.source_type == "transcript" || s.source_type == "social";
        if !s.folder.trim().is_empty() {
            counts.folders += 1;
        }
        if s.do_not_share {
            counts.do_not_share += 1;
        }
        let sguid = gid(&uid, "source", s.id);
        let source_notes = notes_on_source.get(&s.id).cloned().unwrap_or_default();
        let members = unit_by_source.get(&s.id).cloned().unwrap_or_default();

        // a picture
        if let Some(img) = project.get_image(s.id)? {
            let ext = match img.mime.as_str() {
                "image/png" => Some("png"),
                "image/jpeg" => Some("jpg"),
                _ => None,
            };
            let Some(ext) = ext else {
                report.left_out.push(format!("The picture \"{}\" is {}, which the standard does not allow (only JPEG and PNG), so it and its {} are not in the file.", s.name, img.mime.trim_start_matches("image/").to_uppercase(), plural(regions.len(), "coded area", "coded areas")));
                continue;
            };
            let (_, bytes) = project.read_image_bytes(s.id)?;
            if bytes.len() > i32::MAX as usize {
                report.left_out.push(format!("The picture \"{}\" is over 2 GB, which the standard does not allow, so it is not in the file.", s.name));
                continue;
            }
            files.push((format!("sources/{sguid}.{ext}"), bytes));
            counts.pictures += 1;
            if typed {
                counts.typed_sources += 1;
            }
            let _ = write!(sources_xml, "    <PictureSource");
            attr(&mut sources_xml, "guid", &sguid);
            attr(&mut sources_xml, "name", &name_with_extension(&scrub::text(&s.name), ext));
            attr(&mut sources_xml, "path", &format!("internal://{sguid}.{ext}"));
            opt_time(&mut sources_xml, "creationDateTime", &s.created_at);
            sources_xml.push_str(">\n");
            // the picture's description is a text of its own
            let dguid = gid(&uid, "description", s.id);
            files.push((format!("sources/{dguid}.txt"), text.clone().into_bytes()));
            let _ = write!(sources_xml, "      <TextDescription");
            attr(&mut sources_xml, "guid", &dguid);
            attr(&mut sources_xml, "plainTextPath", &format!("internal://{dguid}.txt"));
            sources_xml.push_str(">\n");
            write_text_selections(&mut sources_xml, &uid, &chars, &codings, &members, &coders_name(&coders), &coder_guid, &final_user, &mut needs_final_user, &code_ids, &notes_on_coding, &mut written_codings, &mut ends, &linked_codings, &mut counts, 8);
            sources_xml.push_str("      </TextDescription>\n");
            for r in &regions {
                if !code_ids.contains(&r.code_id) {
                    counts.orphans += 1;
                    continue;
                }
                let (x0, y0, x1, y1) = r.shape.bbox();
                if !matches!(r.shape, Shape::Rectangle { .. }) {
                    counts.shapes_boxed += 1;
                }
                let px = |v: f64, n: i64| ((v * n as f64).round() as i64).clamp(0, n);
                ends.insert(-r.id, gid(&uid, "region", r.id));
                let _ = write!(sources_xml, "      <PictureSelection");
                attr(&mut sources_xml, "guid", &gid(&uid, "region", r.id));
                // QualCoder 4.0 takes the memo of an area from the name (it is None there when there is none, and its own export stops on that).
                attr(&mut sources_xml, "name", &scrub::text(r.rationale.trim()));
                attr(&mut sources_xml, "firstX", &px(x0, img.width).to_string());
                attr(&mut sources_xml, "firstY", &px(y0, img.height).to_string());
                attr(&mut sources_xml, "secondX", &px(x1, img.width).to_string());
                attr(&mut sources_xml, "secondY", &px(y1, img.height).to_string());
                selection_creator(&mut sources_xml, &r.layer, r.coder_id, &coder_guid, &final_user, &mut needs_final_user);
                opt_time(&mut sources_xml, "creationDateTime", &r.created_at);
                sources_xml.push_str(">\n");
                if !r.rationale.trim().is_empty() {
                    let _ = writeln!(sources_xml, "        <Description>{}</Description>", escape(r.rationale.trim()));
                }
                write_coding(&mut sources_xml, &uid, "region-coding", r.id, r.coder_id, &r.layer, r.code_id, &r.created_at, &coder_guid, &final_user, &mut needs_final_user, &[], 8);
                if r.rating.is_some() {
                    counts.ratings += 1;
                }
                counts.codings += 1;
                sources_xml.push_str("      </PictureSelection>\n");
            }
            for g in &source_notes {
                let _ = writeln!(sources_xml, "      <NoteRef targetGUID=\"{g}\"/>");
            }
            sources_xml.push_str("    </PictureSource>\n");
            continue;
        }

        // a PDF kept with the document: the file, and its text as the representation that selections are made on
        let pdf = if s.pdf.is_some() { Some(project.read_pdf_bytes(s.id)?) } else { None };
        if let Some(bytes) = &pdf {
            if bytes.len() > i32::MAX as usize {
                report.left_out.push(format!("The PDF of \"{}\" is over 2 GB, which the standard does not allow, so only its text is in the file.", s.name));
            }
        }
        let pdf_ok = pdf.as_ref().is_some_and(|b| b.len() <= i32::MAX as usize);
        let tguid = if pdf_ok { gid(&uid, "representation", s.id) } else { sguid.clone() };
        files.push((format!("sources/{tguid}.txt"), text.clone().into_bytes()));
        for r in &regions {
            if !matches!(r.shape, Shape::Span { .. }) && !pdf_ok {
                counts.pdf_areas += 1;
            }
        }
        if pdf_ok {
            let bytes = pdf.unwrap_or_default();
            let geometry = prismqda_pdfview::page_geometries(&bytes).ok();
            files.push((format!("sources/{sguid}.pdf"), bytes));
            counts.pdfs += 1;
            if typed {
                counts.typed_sources += 1;
            }
            let _ = write!(sources_xml, "    <PDFSource");
            attr(&mut sources_xml, "guid", &sguid);
            attr(&mut sources_xml, "name", &scrub::text(&s.name));
            attr(&mut sources_xml, "path", &format!("internal://{sguid}.pdf"));
            opt_time(&mut sources_xml, "creationDateTime", &s.created_at);
            sources_xml.push_str(">\n");
            // The coded rectangles of the pages: a page is counted from 0, and a place is in points from the bottom-left of the media box, as
            // the standard names one (decision 175); the page's turn and crop box are taken into account, so the rectangle is where it was drawn.
            for r in &regions {
                if matches!(r.shape, Shape::Span { .. }) {
                    continue;
                }
                if !code_ids.contains(&r.code_id) {
                    counts.orphans += 1;
                    continue;
                }
                let Some(g) = geometry.as_ref().and_then(|g| g.get((r.page - 1).max(0) as usize)) else {
                    counts.pdf_areas += 1;
                    continue;
                };
                let (x0, y0, x1, y1) = r.shape.bbox();
                let (a, b) = (g.to_media_points(x0, y0), g.to_media_points(x1, y1));
                ends.insert(-r.id, gid(&uid, "region", r.id));
                let _ = write!(sources_xml, "      <PDFSelection");
                attr(&mut sources_xml, "guid", &gid(&uid, "region", r.id));
                attr(&mut sources_xml, "name", &scrub::text(r.rationale.trim()));
                attr(&mut sources_xml, "page", &(r.page - 1).max(0).to_string());
                attr(&mut sources_xml, "firstX", &a.0.min(b.0).round().to_string());
                attr(&mut sources_xml, "firstY", &a.1.min(b.1).round().to_string());
                attr(&mut sources_xml, "secondX", &a.0.max(b.0).round().to_string());
                attr(&mut sources_xml, "secondY", &a.1.max(b.1).round().to_string());
                selection_creator(&mut sources_xml, &r.layer, r.coder_id, &coder_guid, &final_user, &mut needs_final_user);
                opt_time(&mut sources_xml, "creationDateTime", &r.created_at);
                sources_xml.push_str(">\n");
                if !r.rationale.trim().is_empty() {
                    let _ = writeln!(sources_xml, "        <Description>{}</Description>", escape(r.rationale.trim()));
                }
                write_coding(&mut sources_xml, &uid, "region-coding", r.id, r.coder_id, &r.layer, r.code_id, &r.created_at, &coder_guid, &final_user, &mut needs_final_user, &[], 8);
                if r.rating.is_some() {
                    counts.ratings += 1;
                }
                counts.codings += 1;
                counts.pdf_rectangles += 1;
                sources_xml.push_str("      </PDFSelection>\n");
            }
            let _ = write!(sources_xml, "      <Representation");
            attr(&mut sources_xml, "guid", &tguid);
            attr(&mut sources_xml, "plainTextPath", &format!("internal://{tguid}.txt"));
            sources_xml.push_str(">\n");
            write_text_selections(&mut sources_xml, &uid, &chars, &codings, &members, &coders_name(&coders), &coder_guid, &final_user, &mut needs_final_user, &code_ids, &notes_on_coding, &mut written_codings, &mut ends, &linked_codings, &mut counts, 8);
            sources_xml.push_str("      </Representation>\n");
            for g in &source_notes {
                let _ = writeln!(sources_xml, "      <NoteRef targetGUID=\"{g}\"/>");
            }
            sources_xml.push_str("    </PDFSource>\n");
        } else {
            counts.documents += 1;
            let _ = write!(sources_xml, "    <TextSource");
            attr(&mut sources_xml, "guid", &sguid);
            attr(&mut sources_xml, "name", &scrub::text(&s.name));
            attr(&mut sources_xml, "plainTextPath", &format!("internal://{sguid}.txt"));
            opt_time(&mut sources_xml, "creationDateTime", &s.created_at);
            sources_xml.push_str(">\n");
            // The kind of the document and where its units are, when they are not the sentences a document is split into: a program that does
            // not know this reads a description, and PrismQDA reads its own file back with the same units.
            if let Some(map) = unit_map(project, &s.source_type, &s.default_unit_kind, s.id, &text)? {
                let _ = writeln!(sources_xml, "      <Description>{}</Description>", escape(&map));
                counts.unit_maps += 1;
            }
            write_text_selections(&mut sources_xml, &uid, &chars, &codings, &members, &coders_name(&coders), &coder_guid, &final_user, &mut needs_final_user, &code_ids, &notes_on_coding, &mut written_codings, &mut ends, &linked_codings, &mut counts, 6);
            for g in &source_notes {
                let _ = writeln!(sources_xml, "      <NoteRef targetGUID=\"{g}\"/>");
            }
            sources_xml.push_str("    </TextSource>\n");
        }
        // The recording itself: where it is on this computer, and the stretches coded on it. The words of the document, and the codings on
        // them, are in the text source above; the sentence in the description says which document this recording belongs to, so that a file
        // PrismQDA wrote can be read back with the recording on its document.
        if let Some(rec) = media_of.get(&s.id) {
            let (element, selection) = if rec.kind == "video" { ("VideoSource", "VideoSelection") } else { ("AudioSource", "AudioSelection") };
            counts.recordings += 1;
            let _ = write!(sources_xml, "    <{element}");
            attr(&mut sources_xml, "guid", &gid(&uid, "recording", s.id));
            let rec_ext = std::path::Path::new(&rec.path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            attr(&mut sources_xml, "name", &name_with_extension(&scrub::text(&format!("{} (recording)", s.name)), &rec_ext));
            // The file itself goes inside when it was asked for and it is not too large; otherwise only where it is on this computer.
            let mut where_it_is = recording_path(&rec.path);
            if options.include_recordings {
                // A recording path that is a network address (it can come from a project someone sent) is not opened (see `prismqda_engine::paths`).
                let size = if prismqda_engine::paths::is_local_drive_path(&rec.path) { std::fs::metadata(&rec.path).map(|m| m.len()).unwrap_or(u64::MAX) } else { u64::MAX };
                if size <= MAX_RECORDING_BYTES && recordings_total + size <= MAX_RECORDINGS_TOTAL {
                    if let Ok(bytes) = std::fs::read(&rec.path) {
                        let ext = std::path::Path::new(&rec.path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "bin".to_string());
                        let name = format!("{}.{ext}", gid(&uid, "recording", s.id));
                        recordings_total += bytes.len() as u64;
                        files.push((format!("sources/{name}"), bytes));
                        where_it_is = format!("internal://{name}");
                        counts.recording_files += 1;
                    }
                }
            }
            attr(&mut sources_xml, "path", &where_it_is);
            opt_time(&mut sources_xml, "creationDateTime", &rec.created_at);
            sources_xml.push_str(">\n");
            let _ = writeln!(sources_xml, "      <Description>{}</Description>", escape(&format!("PrismQDA: the recording of the document \"{}\" (source GUID {sguid}). The words of the document, and the codings on them, are in the text source of that document.", scrub::text(&s.name))));
            // The transcript, with the times of its lines, for programs that read one: a sync point at the start and at the end of each unit that
            // has times in the recording, and a selection between the two for the codings that are exactly on that unit. This project reads its
            // own file from the text source above and does not read this again.
            let units = project.list_units(s.id)?;
            let timed: Vec<&prismqda_engine::UnitRow> = units.iter().filter(|u| u.time_start_ms.is_some() && u.time_end_ms.is_some() && u.end > u.start).collect();
            if !timed.is_empty() {
                counts.timed_transcripts += 1;
                let point = |pos: i64, ms: i64| gid(&uid, &format!("syncpoint-{}-{pos}", s.id), ms);
                let mut points: std::collections::BTreeMap<(i64, i64), String> = std::collections::BTreeMap::new();
                for u in &timed {
                    points.insert((u.start, u.time_start_ms.unwrap_or(0)), point(u.start, u.time_start_ms.unwrap_or(0)));
                    points.insert((u.end, u.time_end_ms.unwrap_or(0)), point(u.end, u.time_end_ms.unwrap_or(0)));
                }
                // The words are a file in the package, the way a text source keeps them (the standard allows the text inside the element or a
                // path, and says "not both"): QualCoder 4.0 reads only the path, and stops on a transcript that has no creation date.
                let tguid = gid(&uid, "transcript", s.id);
                files.push((format!("sources/{tguid}.txt"), text.clone().into_bytes()));
                let _ = write!(sources_xml, "      <Transcript");
                attr(&mut sources_xml, "guid", &tguid);
                attr(&mut sources_xml, "name", &scrub::text(&format!("{} (transcript)", s.name)));
                attr(&mut sources_xml, "plainTextPath", &format!("internal://{tguid}.txt"));
                opt_time(&mut sources_xml, "creationDateTime", &rec.created_at);
                sources_xml.push_str(">\n");
                for ((pos, ms), guid) in &points {
                    let _ = write!(sources_xml, "        <SyncPoint");
                    attr(&mut sources_xml, "guid", guid);
                    attr(&mut sources_xml, "timeStamp", &ms.to_string());
                    attr(&mut sources_xml, "position", &pos.to_string());
                    sources_xml.push_str("/>\n");
                }
                for u in &timed {
                    let on_unit: Vec<&CodingRow> = codings.iter().filter(|c| c.start == u.start && c.end == u.end && code_ids.contains(&c.code_id)).collect();
                    if on_unit.is_empty() {
                        continue;
                    }
                    let _ = write!(sources_xml, "        <TranscriptSelection");
                    attr(&mut sources_xml, "guid", &gid(&uid, "transcript-selection", u.id));
                    attr(&mut sources_xml, "fromSyncPoint", &point(u.start, u.time_start_ms.unwrap_or(0)));
                    attr(&mut sources_xml, "toSyncPoint", &point(u.end, u.time_end_ms.unwrap_or(0)));
                    sources_xml.push_str(">\n");
                    for c in on_unit {
                        write_coding(&mut sources_xml, &uid, "transcript-coding", c.id, c.coder_id, &c.layer, c.code_id, &c.created_at, &coder_guid, &final_user, &mut needs_final_user, &[], 10);
                    }
                    sources_xml.push_str("        </TranscriptSelection>\n");
                }
                sources_xml.push_str("      </Transcript>\n");
            }
            for r in &regions {
                let Shape::Span { start_ms, end_ms } = &r.shape else { continue };
                if !code_ids.contains(&r.code_id) {
                    counts.orphans += 1;
                    continue;
                }
                ends.insert(-r.id, gid(&uid, "region", r.id));
                let _ = write!(sources_xml, "      <{selection}");
                attr(&mut sources_xml, "guid", &gid(&uid, "region", r.id));
                attr(&mut sources_xml, "begin", &start_ms.to_string());
                attr(&mut sources_xml, "end", &end_ms.to_string());
                selection_creator(&mut sources_xml, &r.layer, r.coder_id, &coder_guid, &final_user, &mut needs_final_user);
                opt_time(&mut sources_xml, "creationDateTime", &r.created_at);
                sources_xml.push_str(">\n");
                if !r.rationale.trim().is_empty() {
                    let _ = writeln!(sources_xml, "        <Description>{}</Description>", escape(r.rationale.trim()));
                }
                write_coding(&mut sources_xml, &uid, "region-coding", r.id, r.coder_id, &r.layer, r.code_id, &r.created_at, &coder_guid, &final_user, &mut needs_final_user, &[], 8);
                if r.rating.is_some() {
                    counts.ratings += 1;
                }
                counts.codings += 1;
                counts.spans += 1;
                let _ = writeln!(sources_xml, "      </{selection}>");
            }
            let _ = writeln!(sources_xml, "    </{element}>");
        }
    }
    // A memo on a coding that is not a coding of words (an area of a picture) has no place in the file but the project.
    for (id, guids) in &notes_on_coding {
        if !written_codings.contains(id) {
            loose_coding_memos += guids.len();
            notes_on_project.extend(guids.iter().cloned());
        }
    }

    // ---- users
    let mut users_xml = String::new();
    for c in &coders {
        let _ = write!(users_xml, "    <User");
        attr(&mut users_xml, "guid", &coder_guid(c.id));
        attr(&mut users_xml, "name", &scrub::text(&c.name));
        attr(&mut users_xml, "id", &c.local_id);
        users_xml.push_str("/>\n");
    }
    if needs_final_user {
        let _ = writeln!(users_xml, "    <User guid=\"{final_user}\" name=\"Adjudicated decisions\"/>");
    }

    // ---- the codebook
    let mut children: HashMap<Option<i64>, Vec<&CodeRow>> = HashMap::new();
    for c in &codes {
        children.entry(c.parent_id.filter(|p| code_ids.contains(p))).or_default().push(c);
    }
    for list in children.values_mut() {
        list.sort_by_key(|c| (c.sort_order, c.name.to_lowercase(), c.id));
    }
    let mut codebook_xml = String::new();
    for c in children.get(&None).map(Vec::as_slice).unwrap_or(&[]) {
        write_code(&mut codebook_xml, &uid, c, &children, &notes_on_code, 0);
    }
    for c in &codes {
        if !c.exclusive_group.trim().is_empty() {
            counts.exclusive += 1;
        }
        if c.rated {
            counts.rated_codes += 1;
        }
        if c.do_not_share {
            counts.do_not_share_codes += 1;
        }
        if !c.ai_suggestible {
            counts.no_ai += 1;
        }
        if c.status == "retired" {
            counts.retired += 1;
        }
    }

    // ---- variables and cases
    let mut variables_xml = String::new();
    let mut types: Vec<VarType> = Vec::new();
    for (vi, v) in case_data.variables.iter().enumerate() {
        let values: Vec<&String> = case_data.values.iter().filter_map(|row| row.get(vi).and_then(Option::as_ref)).collect();
        let t = var_type(&v.level, &values);
        types.push(t);
        let _ = write!(variables_xml, "    <Variable");
        attr(&mut variables_xml, "guid", &gid(&uid, "variable", vi as i64));
        attr(&mut variables_xml, "name", &scrub::text(&v.name));
        attr(&mut variables_xml, "typeOfVariable", t.name());
        variables_xml.push_str("/>\n");
        if !v.value_labels.is_empty() || !v.missing_codes.is_empty() {
            counts.labelled_variables += 1;
        }
    }
    let mut cases_xml = String::new();
    for c in &cases {
        let _ = write!(cases_xml, "    <Case");
        attr(&mut cases_xml, "guid", &gid(&uid, "case", c.id));
        attr(&mut cases_xml, "name", &scrub::text(&c.name));
        cases_xml.push_str(">\n");
        if !c.notes.trim().is_empty() {
            let _ = writeln!(cases_xml, "      <Description>{}</Description>", escape(c.notes.trim()));
        }
        if let Some(row_index) = case_data.cases.iter().position(|x| x.id == c.id) {
            for (vi, t) in types.iter().enumerate() {
                let Some(Some(value)) = case_data.values.get(row_index).and_then(|r| r.get(vi)) else { continue };
                let value = value.trim();
                let element = match t {
                    VarType::Text => "TextValue",
                    VarType::Integer => "IntegerValue",
                    VarType::Float => "FloatValue",
                    VarType::Date => "DateValue",
                    VarType::DateTime => "DateTimeValue",
                };
                let shown = if *t == VarType::Text { scrub::text(value).into_owned() } else { value.to_string() };
                let _ = writeln!(cases_xml, "      <VariableValue>\n        <VariableRef targetGUID=\"{}\"/>\n        <{element}>{}</{element}>\n      </VariableValue>", gid(&uid, "variable", vi as i64), escape(&shown));
            }
        }
        for sid in &c.source_ids {
            if source_ids.contains(sid) {
                let _ = writeln!(cases_xml, "      <SourceRef targetGUID=\"{}\"/>", gid(&uid, "source", *sid));
            }
        }
        for m in unit_members.iter().filter(|m| m.case_id == c.id) {
            let _ = writeln!(cases_xml, "      <SelectionRef targetGUID=\"{}\"/>", gid(&uid, "unit", m.unit_id));
        }
        cases_xml.push_str("    </Case>\n");
    }
    // Values kept on units are not a place the format has for a case.
    for s in &sources {
        counts.unit_values += project.unit_attribute_values(s.id)?.len();
    }

    // ---- sets of codes
    let mut sets_xml = String::new();
    for (id, name, members) in &code_sets {
        let members: Vec<&i64> = members.iter().filter(|m| code_ids.contains(m)).collect();
        let _ = write!(sets_xml, "    <Set");
        attr(&mut sets_xml, "guid", &gid(&uid, "set", *id));
        attr(&mut sets_xml, "name", &scrub::text(name));
        sets_xml.push_str(">\n");
        for m in members {
            let _ = writeln!(sets_xml, "      <MemberCode targetGUID=\"{}\"/>", gid(&uid, "code", *m));
        }
        sets_xml.push_str("    </Set>\n");
    }

    // ---- the project
    let mut qde = String::from("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n<Project xmlns=\"urn:QDA-XML:project:1.0\"");
    attr(&mut qde, "name", &scrub::text(&info.name));
    attr(&mut qde, "origin", &format!("PrismQDA {}", env!("CARGO_PKG_VERSION")));
    qde.push_str(">\n");
    if !users_xml.is_empty() {
        let _ = write!(qde, "  <Users>\n{users_xml}  </Users>\n");
    }
    if !codebook_xml.is_empty() {
        let _ = write!(qde, "  <CodeBook>\n    <Codes>\n{codebook_xml}    </Codes>\n  </CodeBook>\n");
    }
    if !variables_xml.is_empty() {
        let _ = write!(qde, "  <Variables>\n{variables_xml}  </Variables>\n");
    }
    if !cases_xml.is_empty() {
        let _ = write!(qde, "  <Cases>\n{cases_xml}  </Cases>\n");
    }
    if !sources_xml.is_empty() {
        let _ = write!(qde, "  <Sources>\n{sources_xml}  </Sources>\n");
    }
    if !notes_xml.is_empty() {
        let _ = write!(qde, "  <Notes>\n{notes_xml}  </Notes>\n");
    }
    // links between passages: one way, from the first passage to the second, named by the relation
    let mut links_xml = String::new();
    for l in project.list_passage_links(None)? {
        match (ends.get(&l.from.coding_id), ends.get(&l.to.coding_id)) {
            (Some(a), Some(b)) if a == b => counts.links_same += 1,
            (Some(a), Some(b)) => {
                let _ = write!(links_xml, "    <Link");
                attr(&mut links_xml, "guid", &gid(&uid, "link", l.id));
                attr(&mut links_xml, "name", &scrub::text(&l.relation));
                attr(&mut links_xml, "direction", "OneWay");
                attr(&mut links_xml, "originGUID", a);
                attr(&mut links_xml, "targetGUID", b);
                links_xml.push_str("/>\n");
                counts.links += 1;
            }
            _ => counts.links_left += 1,
        }
    }
    if !links_xml.is_empty() {
        let _ = write!(qde, "  <Links>\n{links_xml}  </Links>\n");
    }
    // concept maps are graphs: a vertex for each box and an edge for each line; a box that stands for a code, a case, or a memo says which
    let mut graphs_xml = String::new();
    {
        let code_color: HashMap<i64, &str> = codes.iter().map(|c| (c.id, c.color.as_str())).collect();
        let memo_ids: HashSet<i64> = memos.iter().map(|m| m.id).collect();
        let case_ids: HashSet<i64> = cases.iter().map(|c| c.id).collect();
        for row in project.list_concept_maps()? {
            let map = project.get_concept_map(row.id)?;
            let _ = write!(graphs_xml, "    <Graph");
            attr(&mut graphs_xml, "guid", &gid(&uid, "graph", map.id));
            attr(&mut graphs_xml, "name", &scrub::text(&map.name));
            graphs_xml.push_str(">\n");
            for n in &map.nodes {
                let (represented, shape, color): (Option<String>, &str, Option<&str>) = match (n.kind.as_str(), n.ref_id, n.missing) {
                    ("code", Some(id), false) if code_ids.contains(&id) => (Some(gid(&uid, "code", id)), "RoundedRectangle", code_color.get(&id).copied()),
                    ("case", Some(id), false) if case_ids.contains(&id) => (Some(gid(&uid, "case", id)), "Oval", None),
                    ("memo", Some(id), false) if memo_ids.contains(&id) => (Some(gid(&uid, "memo", id)), "Note", None),
                    _ => (None, "Note", None),
                };
                let _ = write!(graphs_xml, "      <Vertex");
                attr(&mut graphs_xml, "guid", &gid(&uid, "vertex", n.id));
                if let Some(g) = &represented {
                    attr(&mut graphs_xml, "representedGUID", g);
                }
                attr(&mut graphs_xml, "name", &scrub::text(&n.label));
                attr(&mut graphs_xml, "firstX", &(n.x.round() as i64).to_string());
                attr(&mut graphs_xml, "firstY", &(n.y.round() as i64).to_string());
                attr(&mut graphs_xml, "shape", shape);
                if let Some(c) = color.filter(|c| is_rgb(c)) {
                    attr(&mut graphs_xml, "color", c);
                }
                graphs_xml.push_str("/>\n");
                counts.graph_nodes += 1;
            }
            for l in &map.links {
                let _ = write!(graphs_xml, "      <Edge");
                attr(&mut graphs_xml, "guid", &gid(&uid, "edge", l.id));
                attr(&mut graphs_xml, "name", &scrub::text(&l.label));
                attr(&mut graphs_xml, "sourceVertex", &gid(&uid, "vertex", l.from_node));
                attr(&mut graphs_xml, "targetVertex", &gid(&uid, "vertex", l.to_node));
                attr(&mut graphs_xml, "direction", if l.directed { "OneWay" } else { "Associative" });
                graphs_xml.push_str("/>\n");
                counts.graph_edges += 1;
            }
            graphs_xml.push_str("    </Graph>\n");
            counts.graphs += 1;
        }
    }
    if !sets_xml.is_empty() {
        let _ = write!(qde, "  <Sets>\n{sets_xml}  </Sets>\n");
    }
    if !graphs_xml.is_empty() {
        let _ = write!(qde, "  <Graphs>\n{graphs_xml}  </Graphs>\n");
    }
    for g in &notes_on_project {
        let _ = writeln!(qde, "  <NoteRef targetGUID=\"{g}\"/>");
    }
    qde.push_str("</Project>\n");

    // ---- the report
    let coder_count = coders.len() + usize::from(needs_final_user);
    report.carried.push(plural(coder_count, "coder", "coders"));
    report.carried.push(plural(codes.len(), "code", "codes"));
    report.carried.push(format!("{}, {}, and {}", plural(counts.documents, "document", "documents"), plural(counts.pictures, "picture", "pictures"), plural(counts.pdfs, "PDF", "PDFs")));
    report.carried.push(plural(counts.codings, "coding", "codings"));
    report.carried.push(format!("{} with {}", plural(cases.len(), "case", "cases"), plural(case_data.variables.len(), "variable", "variables")));
    report.carried.push(plural(memos.len(), "memo", "memos"));
    if counts.pdf_rectangles > 0 {
        report.carried.push(format!("{} on the pages of PDFs", plural(counts.pdf_rectangles, "coded rectangle", "coded rectangles")));
    }
    if counts.unit_maps > 0 {
        report.carried.push(format!("The kind and the units of {} (in its description, for PrismQDA to read back)", plural(counts.unit_maps, "document", "documents")));
    }
    if counts.recording_files > 0 {
        report.carried.push(format!("The files of {} inside the file", plural(counts.recording_files, "recording", "recordings")));
    }
    if counts.timed_transcripts > 0 {
        report.carried.push(format!("{} with the times of their lines (sync points, and the codings on whole lines between them)", plural(counts.timed_transcripts, "transcript", "transcripts")));
    }
    if counts.recordings > 0 {
        report.carried.push(format!("{} (as audio or video sources) with {} coded on them", plural(counts.recordings, "linked recording", "linked recordings"), plural(counts.spans, "stretch", "stretches")));
    }
    if counts.links > 0 {
        report.carried.push(plural(counts.links, "link between passages", "links between passages"));
    }
    report.carried.push(plural(code_sets.len(), "set of codes", "sets of codes"));
    if counts.graphs > 0 {
        report.carried.push(format!("{} (graphs) with {} and {}", plural(counts.graphs, "concept map", "concept maps"), plural(counts.graph_nodes, "box", "boxes"), plural(counts.graph_edges, "line", "lines")));
    }
    let l = &mut report.left_out;
    if counts.pdf_areas > 0 {
        l.push(format!("{} on the pages of PDFs that could not be placed (the PDF is not in the file, or its pages could not be read).", plural(counts.pdf_areas, "coded rectangle", "coded rectangles")));
    }
    if counts.recordings > counts.recording_files {
        l.push(format!("The files of {}: the file says where each is on this computer, and the recording is not inside it. Whoever opens the file elsewhere needs the recordings and has to point their program at them.{}", plural(counts.recordings - counts.recording_files, "recording", "recordings"), if options.include_recordings { " (A recording over 512 MB, or past 1 GB in all, is left out of the file.)" } else { " (There is a choice to put the recordings inside the file.)" }));
    }
    if counts.shapes_boxed > 0 {
        l.push(format!("{} on pictures are in the file as the rectangle around them.", plural(counts.shapes_boxed, "ellipse or outline", "ellipses and outlines")));
    }
    if counts.ratings > 0 {
        l.push(format!("{} of codings. The format has no rating.", plural(counts.ratings, "rating", "ratings")));
    }
    if counts.typed_sources > 0 {
        l.push(format!("The kind of {} (survey, transcript, social post) that is a picture or a PDF, and its units.", plural(counts.typed_sources, "document", "documents")));
    }
    if counts.folders > 0 {
        l.push(format!("The folder of {}.", plural(counts.folders, "document", "documents")));
    }
    if counts.unit_values > 0 {
        l.push(format!("{} on units of documents. A unit is not a place the format gives a variable.", plural(counts.unit_values, "variable value", "variable values")));
    }
    if counts.labelled_variables > 0 {
        l.push(format!("The value labels and missing-value codes of {}.", plural(counts.labelled_variables, "variable", "variables")));
    }
    if counts.exclusive > 0 || counts.rated_codes > 0 || counts.no_ai > 0 {
        l.push(format!("Code settings the format has no place for: {}, {}, and {} kept out of AI packets.", plural(counts.exclusive, "code in an exclusive group", "codes in exclusive groups"), plural(counts.rated_codes, "rated code", "rated codes"), plural(counts.no_ai, "code", "codes")));
    }
    if counts.do_not_share > 0 || counts.do_not_share_codes > 0 {
        l.push(format!("The do-not-share marks on {} and {}. Everything is in the file: see to who may have it.", plural(counts.do_not_share, "document", "documents"), plural(counts.do_not_share_codes, "code", "codes")));
    }
    if counts.retired > 0 {
        l.push(format!("That {} retired (they are in the file as ordinary codes).", plural(counts.retired, "code is", "codes are")));
    }
    if loose_coding_memos > 0 {
        l.push(format!("{} on codings of areas. They are on the project in the file.", plural(loose_coding_memos, "memo", "memos")));
    }
    if counts.links_same > 0 {
        l.push(format!("{} between two codings of the same words, because the format joins selections and these are one selection.", plural(counts.links_same, "link", "links")));
    }
    if counts.links_left > 0 {
        l.push(format!("{} between passages, because one of its passages is not in the file (an area of a PDF page, a stretch of a recording, or a picture the format does not hold).", plural(counts.links_left, "link", "links")));
    }
    if counts.orphans > 0 {
        l.push(format!("{} of a merged code.", plural(counts.orphans, "coding", "codings")));
    }
    l.push("The kinds of memos, the formatting of documents (headings, bold), the history of the project and its codebook versions, the descriptions of sets, and the adjudicated layer as a layer (its codings are written under the coder \"Adjudicated decisions\").".to_string());
    l.push("Everything the format has no part for: saved and frozen queries, statistics pages and their results, the mixed methods design and records, reliability runs, adjudication records, AI runs, and the list of names to replace.".to_string());
    if !project.list_images().map(|i| i.is_empty()).unwrap_or(true) || counts.pdfs > 0 {
        l.push("The text of a document and the file of a picture or PDF are in the file whole, as are any names in them.".to_string());
    }

    let bytes = zip_package(|zip, options| {
        let io = |e: std::io::Error| ExportError::Io(e.to_string());
        let ze = |e: zip::result::ZipError| ExportError::Io(e.to_string());
        zip.start_file("project.qde", options).map_err(ze)?;
        zip.write_all(qde.as_bytes()).map_err(io)?;
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();
        for (name, data) in &files {
            if seen.insert(name.clone(), ()).is_some() {
                continue;
            }
            // a recording is already compressed; it is stored as it is
            let kind = if name.rsplit('.').next().is_some_and(|e| matches!(e, "mp3" | "m4a" | "aac" | "ogg" | "oga" | "opus" | "flac" | "wma" | "mp4" | "mov" | "mkv" | "webm" | "m4v" | "wav" | "avi")) { options.compression_method(zip::CompressionMethod::Stored).large_file(true) } else { options };
            zip.start_file(name.as_str(), kind).map_err(ze)?;
            zip.write_all(data).map_err(io)?;
        }
        Ok(())
    })?;
    Ok((bytes, report))
}

fn coders_name(coders: &[prismqda_engine::CoderRow]) -> HashMap<i64, String> {
    coders.iter().map(|c| (c.id, c.name.clone())).collect()
}

#[derive(Default)]
struct Counts {
    documents: usize,
    pictures: usize,
    pdfs: usize,
    codings: usize,
    pdf_areas: usize,
    recordings: usize,
    spans: usize,
    shapes_boxed: usize,
    ratings: usize,
    typed_sources: usize,
    folders: usize,
    unit_values: usize,
    labelled_variables: usize,
    exclusive: usize,
    rated_codes: usize,
    no_ai: usize,
    do_not_share: usize,
    do_not_share_codes: usize,
    retired: usize,
    orphans: usize,
    links: usize,
    links_left: usize,
    links_same: usize,
    graphs: usize,
    graph_nodes: usize,
    graph_edges: usize,
    timed_transcripts: usize,
    unit_maps: usize,
    recording_files: usize,
    pdf_rectangles: usize,
}

/// A source's name with the extension of its file: some programs build the name of the stored file from the name (QualCoder 4.0 does, and
/// with a name that has no extension it cannot find the file again when it exports, so a picture or a recording is lost in its own export).
fn name_with_extension(name: &str, ext: &str) -> String {
    if ext.is_empty() {
        return name.to_string();
    }
    let dotted = format!(".{ext}");
    if name.to_ascii_lowercase().ends_with(&dotted.to_ascii_lowercase()) {
        name.to_string()
    } else {
        format!("{name}{dotted}")
    }
}

/// The coder of a selection that holds one coding: some programs read the coder from the selection and not from the coding inside it.
fn selection_creator(out: &mut String, layer: &str, coder_id: i64, coder_guid: &dyn Fn(i64) -> String, final_user: &str, needs_final: &mut bool) {
    if layer == "final" {
        *needs_final = true;
        attr(out, "creatingUser", final_user);
    } else {
        attr(out, "creatingUser", &coder_guid(coder_id));
    }
}

/// One `Coding` element.
#[allow(clippy::too_many_arguments)]
fn write_coding(out: &mut String, uid: &str, kind: &str, id: i64, coder_id: i64, layer: &str, code_id: i64, created: &str, coder_guid: &dyn Fn(i64) -> String, final_user: &str, needs_final: &mut bool, notes: &[String], indent: usize) {
    let pad = " ".repeat(indent);
    let _ = write!(out, "{pad}<Coding");
    attr(out, "guid", &gid(uid, kind, id));
    if layer == "final" {
        *needs_final = true;
        attr(out, "creatingUser", final_user);
    } else {
        attr(out, "creatingUser", &coder_guid(coder_id));
    }
    opt_time(out, "creationDateTime", created);
    out.push_str(">\n");
    let _ = writeln!(out, "{pad}  <CodeRef targetGUID=\"{}\"/>", gid(uid, "code", code_id));
    for g in notes {
        let _ = writeln!(out, "{pad}  <NoteRef targetGUID=\"{g}\"/>");
    }
    let _ = writeln!(out, "{pad}</Coding>");
}

/// The text selections of a source: one for each stretch of words that has codings, with the codings in it, and one for each unit that
/// a case holds (so the case can refer to it).
#[allow(clippy::too_many_arguments)]
fn write_text_selections(
    out: &mut String,
    uid: &str,
    chars: &[char],
    codings: &[CodingRow],
    members: &[&prismqda_engine::CaseUnitMember],
    coder_names: &HashMap<i64, String>,
    coder_guid: &dyn Fn(i64) -> String,
    final_user: &str,
    needs_final: &mut bool,
    code_ids: &HashSet<i64>,
    notes_on_coding: &HashMap<i64, Vec<String>>,
    written: &mut HashSet<i64>,
    ends: &mut HashMap<i64, String>,
    linked: &HashSet<i64>,
    counts: &mut Counts,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let mut groups: BTreeMap<(i64, i64), Vec<&CodingRow>> = BTreeMap::new();
    for c in codings {
        if !code_ids.contains(&c.code_id) {
            counts.orphans += 1;
            continue;
        }
        groups.entry((c.start, c.end)).or_default().push(c);
    }
    for ((start, end), whole) in &groups {
        // The codings that a link ends at each get a selection of their own; the others of the same words stay together.
        let (own, rest): (Vec<&CodingRow>, Vec<&CodingRow>) = whole.iter().copied().partition(|c| linked.contains(&c.id));
        let mut parts: Vec<Vec<&CodingRow>> = Vec::new();
        if !rest.is_empty() {
            parts.push(rest);
        }
        parts.extend(own.into_iter().map(|c| vec![c]));
        // One selection for each coder: a program that keeps the coder on the selection (QualCoder 4.0 reads it there and nowhere else) would
        // otherwise give every coding in the selection to one person, and drop a second coder's coding of the same words with the same code.
        let mut split: Vec<Vec<&CodingRow>> = Vec::new();
        for list in &parts {
            let mut by_coder: BTreeMap<(bool, i64), Vec<&CodingRow>> = BTreeMap::new();
            for c in list {
                let final_layer = c.layer == "final";
                by_coder.entry((final_layer, if final_layer { 0 } else { c.coder_id })).or_default().push(c);
            }
            split.extend(by_coder.into_values());
        }
        for list in &split {
            let first = list.iter().map(|c| c.id).min().unwrap_or(0);
            let lead = list.iter().min_by_key(|c| c.id).copied();
            for c in list {
                ends.insert(c.id, gid(uid, "selection", first));
            }
            let _ = write!(out, "{pad}<PlainTextSelection");
            attr(out, "guid", &gid(uid, "selection", first));
            attr(out, "name", &scrub::text(&excerpt(chars, *start as usize, *end as usize)));
            attr(out, "startPosition", &start.to_string());
            attr(out, "endPosition", &end.to_string());
            if let Some(lead) = lead {
                if lead.layer == "final" {
                    *needs_final = true;
                    attr(out, "creatingUser", final_user);
                } else {
                    attr(out, "creatingUser", &coder_guid(lead.coder_id));
                }
                opt_time(out, "creationDateTime", &lead.created_at);
            }
            out.push_str(">\n");
            let rationales: Vec<String> = list
                .iter()
                .filter(|c| !c.rationale.trim().is_empty())
                .map(|c| if list.len() > 1 { format!("{}: {}", coder_names.get(&c.coder_id).map(String::as_str).unwrap_or("?"), c.rationale.trim()) } else { c.rationale.trim().to_string() })
                .collect();
            if !rationales.is_empty() {
                let _ = writeln!(out, "{pad}  <Description>{}</Description>", escape(&rationales.join("\n")));
            }
            for c in list {
                write_coding(out, uid, "coding", c.id, c.coder_id, &c.layer, c.code_id, &c.created_at, coder_guid, final_user, needs_final, notes_on_coding.get(&c.id).map(Vec::as_slice).unwrap_or(&[]), indent + 2);
                written.insert(c.id);
                counts.codings += 1;
                if c.rating.is_some() {
                    counts.ratings += 1;
                }
            }
            let _ = writeln!(out, "{pad}</PlainTextSelection>");
        }
    }
    for m in members {
        let _ = write!(out, "{pad}<PlainTextSelection");
        attr(out, "guid", &gid(uid, "unit", m.unit_id));
        attr(out, "name", &scrub::text(&excerpt(chars, m.start as usize, m.end as usize)));
        attr(out, "startPosition", &m.start.to_string());
        attr(out, "endPosition", &m.end.to_string());
        out.push_str("/>\n");
    }
}

