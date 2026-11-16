//! Exporters (specification 8.5 and 23.2): the codebook and the coded segments, as Excel, Word, CSV, and the
//! REFI-QDA codebook format. Each writes to bytes; the caller decides where they go.
//!
//! Everything is made from what the project stores and nothing else: no number, name, or quotation is added,
//! and each file says at its top what it holds and when it was made.

pub mod base64;
mod csv;
mod docx;
mod feedback;
mod notebook;
mod pstat;
mod qdc;
mod qdc_import;
mod qdpx;
mod qdpx_import;
mod report;
mod scrub;
mod xlsx;
mod xml;

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use prismqda_engine::{exif_orientation, CodeRow, CodedSegment, Project, SegmentFilter};

pub use crate::csv::write_csv;
pub use crate::docx::{write_docx, write_docx_with_media, Block, Crop, Media, Run};
pub use crate::feedback::{feedback_file_name, student_feedback_docx};
pub use crate::notebook::{export_notebook, Labels as NotebookLabels, Notebook, NotebookChart, NotebookFormat, NotebookPage, NotebookTable};
pub use crate::pstat::{attribute_name, attribute_names, export_case_table, PstatOptions, PstatReport, PROFILE as PSTAT_CASE_TABLE_PROFILE};
pub use crate::qdc::{guid, write_qdc, QdcCode};
pub use crate::qdc_import::{import_qdc, QdcImportReport};
pub use crate::qdpx::{export_qdpx, export_qdpx_with, QdpxOptions, QdpxReport};
pub use crate::qdpx_import::import_qdpx;
pub use crate::report::{export_report, ReportOptions, ReportSection};
pub use crate::scrub::{scrub_scope, ScrubGuard};
pub use crate::xlsx::{write_xlsx, write_xlsx_with_media, Cell, Sheet, SheetPicture, MAX_CELL_CHARS, MAX_ROWS};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    TooLarge(String),
    #[error("could not build the file: {0}")]
    Io(String),
    #[error(transparent)]
    Engine(#[from] prismqda_engine::Error),
}

/// Runs `fill` against a new zip archive and returns its bytes.
pub(crate) fn zip_package(fill: impl FnOnce(&mut zip::ZipWriter<Cursor<Vec<u8>>>, zip::write::SimpleFileOptions) -> Result<(), ExportError>) -> Result<Vec<u8>, ExportError> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    fill(&mut zip, options)?;
    Ok(zip.finish().map_err(|e| ExportError::Io(e.to_string()))?.into_inner())
}

/// The file kinds the exporters can make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Xlsx,
    Docx,
    Csv,
    /// The REFI-QDA codebook format; for the codebook only.
    Qdc,
}

impl Format {
    pub fn from_name(name: &str) -> Option<Format> {
        match name {
            "xlsx" => Some(Format::Xlsx),
            "docx" => Some(Format::Docx),
            "csv" => Some(Format::Csv),
            "qdc" => Some(Format::Qdc),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Xlsx => "xlsx",
            Format::Docx => "docx",
            Format::Csv => "csv",
            Format::Qdc => "qdc",
        }
    }
}

/// Today's date in UTC as `YYYY-MM-DD`, from the system clock.
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

// ---- the codebook ----------------------------------------------------------------------------------

/// What to put in a codebook export.
#[derive(Debug, Clone, Default)]
pub struct CodebookOptions {
    /// Include merged and retired codes (default: only active ones).
    pub include_inactive: bool,
}

struct CodebookLine {
    code: CodeRow,
    path: String,
    depth: usize,
    examples: Vec<CodedSegment>,
}

/// The codes in tree order, each with its path and anchor examples.
fn codebook_lines(project: &Project, options: &CodebookOptions) -> Result<Vec<CodebookLine>, ExportError> {
    let mut codes: Vec<CodeRow> = project.list_codes()?;
    if !options.include_inactive {
        codes.retain(|c| c.status == "active");
    }
    let known: std::collections::HashSet<i64> = codes.iter().map(|c| c.id).collect();
    let mut children: HashMap<Option<i64>, Vec<&CodeRow>> = HashMap::new();
    for c in &codes {
        let parent = c.parent_id.filter(|p| known.contains(p));
        children.entry(parent).or_default().push(c);
    }
    for list in children.values_mut() {
        list.sort_by_key(|c| (c.sort_order, c.name.to_lowercase(), c.id));
    }
    let mut out = Vec::new();
    fn walk(project: &Project, children: &HashMap<Option<i64>, Vec<&CodeRow>>, parent: Option<i64>, prefix: &str, depth: usize, out: &mut Vec<CodebookLine>) -> Result<(), ExportError> {
        for c in children.get(&parent).map(Vec::as_slice).unwrap_or(&[]) {
            let path = if prefix.is_empty() { c.name.clone() } else { format!("{prefix} > {}", c.name) };
            let examples = project.anchor_examples(c.id)?;
            out.push(CodebookLine { code: (*c).clone(), path: path.clone(), depth, examples });
            walk(project, children, Some(c.id), &path, depth + 1, out)?;
        }
        Ok(())
    }
    walk(project, &children, None, "", 0, &mut out)?;
    Ok(out)
}

fn example_text(e: &CodedSegment) -> String {
    format!("\u{201C}{}\u{201D} ({})", e.text.trim(), e.source_name)
}

/// The codes as blocks: each a heading (`first_level` for a top code, a level deeper for each step down, to the third), what it is,
/// how often it is used, its definition and rules, and its example quotations.
fn codebook_blocks(lines: &[CodebookLine], first_level: u8) -> Vec<Block> {
    let mut blocks = Vec::new();
    for l in lines {
        // Each code is a heading at its depth (one to three), so Word's navigation pane shows the tree.
        blocks.push(Block::Heading((l.depth as u8 + first_level).min(3), l.code.name.clone()));
        let mut facts = Vec::new();
        if l.code.kind == "theme" {
            facts.push("A theme: it groups other codes.".to_string());
        }
        if l.code.status != "active" {
            facts.push(format!("Status: {}.", l.code.status));
        }
        facts.push(format!("Used {} time{}.", l.code.usage, if l.code.usage == 1 { "" } else { "s" }));
        blocks.push(Block::paragraph(facts.join(" ")));
        for (label, text) in [("Definition", &l.code.definition), ("Include when", &l.code.inclusion_criteria), ("Do not include when", &l.code.exclusion_criteria)] {
            if !text.trim().is_empty() {
                blocks.push(Block::Paragraph(vec![Run::bold(format!("{label}: ")), Run::plain(text.trim())]));
            }
        }
        for e in &l.examples {
            blocks.push(Block::Quote { text: e.text.trim().to_string(), source: Some(format!("Example from {}", e.source_name)) });
        }
    }
    blocks
}

/// The codebook in the chosen format.
pub fn export_codebook(project: &Project, format: Format, options: &CodebookOptions) -> Result<Vec<u8>, ExportError> {
    let info = project.info()?;
    let lines = codebook_lines(project, options)?;
    let versions = project.list_codebook_versions()?;
    let version_note = versions.last().map(|v| format!("Codebook version {}{}", v.version, if v.note.trim().is_empty() { String::new() } else { format!(": {}", v.note.trim()) }));

    match format {
        Format::Qdc => {
            let codes: Vec<QdcCode> = lines
                .iter()
                .map(|l| {
                    let mut description = l.code.definition.trim().to_string();
                    for (label, text) in [("Include when", &l.code.inclusion_criteria), ("Do not include when", &l.code.exclusion_criteria)] {
                        if !text.trim().is_empty() {
                            if !description.is_empty() {
                                description.push('\n');
                            }
                            description.push_str(&format!("{label}: {}", text.trim()));
                        }
                    }
                    QdcCode { id: l.code.id, name: l.code.name.clone(), parent_id: l.code.parent_id, color: l.code.color.clone(), description, codable: l.code.kind != "theme" }
                })
                .collect();
            Ok(write_qdc(&codes, &info.project_uid, &format!("PrismQDA {}", env!("CARGO_PKG_VERSION"))).into_bytes())
        }
        Format::Csv | Format::Xlsx => {
            let header: Vec<String> =
                ["Code", "Name", "Parent", "Type", "Status", "Definition", "Include when", "Do not include when", "Examples", "Times used", "Color", "Version"].map(String::from).to_vec();
            let names: HashMap<i64, &str> = lines.iter().map(|l| (l.code.id, l.code.name.as_str())).collect();
            let text_rows: Vec<Vec<String>> = lines
                .iter()
                .map(|l| {
                    vec![
                        l.path.clone(),
                        l.code.name.clone(),
                        l.code.parent_id.and_then(|p| names.get(&p)).map(|s| s.to_string()).unwrap_or_default(),
                        l.code.kind.clone(),
                        l.code.status.clone(),
                        l.code.definition.clone(),
                        l.code.inclusion_criteria.clone(),
                        l.code.exclusion_criteria.clone(),
                        l.examples.iter().map(example_text).collect::<Vec<_>>().join("\n"),
                        l.code.usage.to_string(),
                        l.code.color.clone(),
                        l.code.version.to_string(),
                    ]
                })
                .collect();
            if format == Format::Csv {
                return Ok(write_csv(&header, &text_rows));
            }
            let rows: Vec<Vec<Cell>> = text_rows
                .iter()
                .zip(&lines)
                .map(|(r, l)| {
                    let mut cells: Vec<Cell> = r.iter().cloned().map(Cell::Text).collect();
                    cells[9] = Cell::Number(l.code.usage as f64);
                    cells[11] = Cell::Number(l.code.version as f64);
                    cells
                })
                .collect();
            write_xlsx(&[Sheet { name: "Codebook".into(), header, rows, widths: vec![32.0, 22.0, 18.0, 8.0, 9.0, 50.0, 34.0, 34.0, 50.0, 10.0, 10.0, 8.0], pictures: Vec::new() }])
        }
        Format::Docx => {
            let mut blocks = vec![Block::Heading(1, format!("Codebook: {}", info.name))];
            let mut about = format!("Made by PrismQDA on {}. {} codes", today(), lines.len());
            if let Some(v) = &version_note {
                about.push_str(&format!(". {v}"));
            }
            about.push('.');
            blocks.push(Block::paragraph(about));
            blocks.extend(codebook_blocks(&lines, 2));
            write_docx(&format!("Codebook: {}", info.name), &blocks)
        }
    }
}

// ---- coded segments --------------------------------------------------------------------------------

/// The most picture files one exported file keeps, added up. Past this the rest are told in words only.
const MAX_EMBEDDED_BYTES: usize = 100 * 1024 * 1024;
const EMU_PER_INCH: f64 = 914_400.0;
const EMU_PER_PIXEL: f64 = 9_525.0;
/// The largest a cropped area is shown, in English metric units: in a Word page and in an Excel cell.
const WORD_MAX: (i64, i64) = (4_114_800, 3_200_400);
const EXCEL_MAX: (i64, i64) = (3_657_600, 3_556_000);

/// An area of a picture ready to be cropped out of the file kept in the export.
struct PictureArea {
    media: usize,
    crop: Crop,
    width: i64,
    height: i64,
}

/// The pictures an export keeps, each once, and which areas of them it can show cropped. A cropped area is the whole file with the
/// outside of the area hidden, so the export holds one copy of each picture however many areas were coded on it.
struct Pictures {
    media: Vec<Media>,
    /// For each picture, by document and page (a picture has one page, the first): its place in `media` and its size in pixels as it
    /// is shown, or nothing when the file cannot hold it.
    by_source: HashMap<(i64, i64), Option<PictureSlot>>,
}

/// Where a picture is among the media of an export, and its size in pixels as it is shown.
type PictureSlot = (usize, i64, i64);

/// The ending and kind Word and Excel use for a picture, when they can show it as it is kept. A photograph its camera turned (EXIF
/// orientation) is left out: programs differ on whether they turn it, so a crop could land on the wrong part. WebP is left out because
/// older Office cannot read it.
fn kept_kind(mime: &str, bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    match mime {
        "image/png" => Some(("png", "image/png")),
        "image/jpeg" if matches!(exif_orientation(bytes), None | Some(1)) => Some(("jpg", "image/jpeg")),
        "image/gif" => Some(("gif", "image/gif")),
        "image/bmp" => Some(("bmp", "image/bmp")),
        _ => None,
    }
}

/// The size an area is shown at: its own size at 96 pixels to the inch, no smaller than an inch along its longer side and no larger than the box.
fn fit(width_px: f64, height_px: f64, max: (i64, i64)) -> (i64, i64) {
    let (mut w, mut h) = (width_px * EMU_PER_PIXEL, height_px * EMU_PER_PIXEL);
    let longer = w.max(h);
    if longer > 0.0 && longer < EMU_PER_INCH {
        let up = EMU_PER_INCH / longer;
        w *= up;
        h *= up;
    }
    let down = (max.0 as f64 / w).min(max.1 as f64 / h).min(1.0);
    ((w * down).round().max(1.0) as i64, (h * down).round().max(1.0) as i64)
}

impl Pictures {
    /// Reads each picture the segments have an area of, once.
    fn gather(project: &Project, segments: &[CodedSegment]) -> Result<Pictures, ExportError> {
        let mut pictures = Pictures { media: Vec::new(), by_source: HashMap::new() };
        // A picture or a page of a PDF cannot have names taken out of it, so while a list of names is applied to what leaves the project
        // the areas are told in words and the pictures stay in the project.
        if crate::scrub::active() {
            return Ok(pictures);
        }
        let mut total = 0usize;
        let mut pdf_files: HashMap<i64, Option<Vec<u8>>> = HashMap::new();
        for s in segments.iter().filter(|s| s.kind == "image" && s.region.is_some()) {
            let key = (s.source_id, s.page.unwrap_or(1));
            if pictures.by_source.contains_key(&key) {
                continue;
            }
            let mut found = None;
            if let Some(page) = s.page {
                // An area of a PDF page: the page is drawn once and kept in the file, and the area is cropped out of it.
                let file = pdf_files.entry(s.source_id).or_insert_with(|| project.read_pdf_bytes(s.source_id).ok());
                if let Some(bytes) = file.as_deref().and_then(|pdf| prismqda_pdfview::render_page_png(pdf, page as u32, PAGE_PICTURE_WIDTH).ok()) {
                    let size = png_size(&bytes);
                    if let (Some((w, h)), true) = (size, total + bytes.len() <= MAX_EMBEDDED_BYTES) {
                        total += bytes.len();
                        pictures.media.push(Media { extension: "png", content_type: "image/png", bytes });
                        found = Some((pictures.media.len() - 1, w, h));
                    }
                }
            } else if let Some(row) = project.get_image(s.source_id)? {
                let (mime, bytes) = project.read_image_bytes(s.source_id)?;
                if let Some((extension, content_type)) = kept_kind(&mime, &bytes) {
                    if total + bytes.len() <= MAX_EMBEDDED_BYTES {
                        total += bytes.len();
                        pictures.media.push(Media { extension, content_type, bytes });
                        found = Some((pictures.media.len() - 1, row.width, row.height));
                    }
                }
            }
            pictures.by_source.insert(key, found);
        }
        Ok(pictures)
    }

    /// The area of a picture a segment names, cropped from the kept file, shown no larger than `max`. Nothing when the segment is not an
    /// area of a picture or the picture could not be kept.
    fn area(&self, s: &CodedSegment, max: (i64, i64)) -> Option<PictureArea> {
        if s.kind != "image" {
            return None;
        }
        let shape = s.region.as_ref()?;
        let (media, width, height) = (*self.by_source.get(&(s.source_id, s.page.unwrap_or(1)))?)?;
        let (x0, y0, x1, y1) = shape.bbox();
        let crop = Crop { x0: x0.clamp(0.0, 1.0), y0: y0.clamp(0.0, 1.0), x1: x1.clamp(0.0, 1.0), y1: y1.clamp(0.0, 1.0) };
        if !crop.is_valid() {
            return None;
        }
        let (w, h) = fit((crop.x1 - crop.x0) * width as f64, (crop.y1 - crop.y0) * height as f64, max);
        Some(PictureArea { media, crop, width: w, height: h })
    }
}

/// How wide a page of a PDF is drawn when a coded area of it goes into a Word or Excel file, in pixels. Wide enough to read a figure's
/// labels when the area is cropped out of it.
const PAGE_PICTURE_WIDTH: u32 = 1400;

/// The width and height in pixels of a PNG, from its header.
fn png_size(png: &[u8]) -> Option<(i64, i64)> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    (w > 0 && h > 0).then_some((i64::from(w), i64::from(h)))
}

/// What a person who cannot see an area hears: which picture or PDF page, which code, and the area in words.
fn area_alt(s: &CodedSegment) -> String {
    match s.page {
        Some(page) => format!("Area of page {page} of {}, coded {}: {}", s.source_name, s.code_name, s.text.trim()),
        None => format!("Area of the picture {}, coded {}: {}", s.source_name, s.code_name, s.text.trim()),
    }
}

/// The start and end of a coded stretch of a recording, in seconds; `None` for any other segment.
fn clip_seconds(s: &CodedSegment) -> Option<(f64, f64)> {
    match (s.kind.as_str(), s.region.as_ref()) {
        ("clip", Some(prismqda_engine::Shape::Span { start_ms, end_ms })) => Some((*start_ms as f64 / 1000.0, *end_ms as f64 / 1000.0)),
        _ => None,
    }
}

/// Where an area is, in a sentence: "an area of the picture" or "an area of page 3".
fn area_place(s: &CodedSegment) -> String {
    match s.page {
        Some(page) => format!("an area of page {page}"),
        None => "an area of the picture".to_string(),
    }
}

/// The coded segments as blocks, grouped by code in the order of the codebook, each code a heading of `level`; an area of a picture is
/// shown cropped out of its picture when the file can hold it.
fn segment_blocks(project: &Project, segments: &[CodedSegment], pictures: &Pictures, level: u8) -> Result<Vec<Block>, ExportError> {
    let lines = codebook_lines(project, &CodebookOptions { include_inactive: true })?;
    let mut by_code: HashMap<i64, Vec<&CodedSegment>> = HashMap::new();
    for s in segments {
        by_code.entry(s.code_id).or_default().push(s);
    }
    let mut blocks = Vec::new();
    for l in &lines {
        let Some(list) = by_code.get(&l.code.id) else { continue };
        blocks.push(Block::Heading(level, format!("{} ({})", l.path, list.len())));
        if !l.code.definition.trim().is_empty() {
            blocks.push(Block::Paragraph(vec![Run::italic(l.code.definition.trim())]));
        }
        for s in list {
            let who = if s.coder_role == "model" { format!("{} (model)", s.coder_name) } else { s.coder_name.clone() };
            let mut where_from = if s.kind == "image" {
                // The area is cropped out of its picture when the file can hold it; otherwise it is told in words only.
                if let Some(a) = pictures.area(s, WORD_MAX) {
                    blocks.push(Block::Picture { media: a.media, crop: a.crop, width: a.width, height: a.height, alt: area_alt(s) });
                    format!("{}, {}, coded by {}", s.source_name, area_place(s), who)
                } else {
                    format!("{}, {} (the picture is not shown in this file), coded by {}", s.source_name, area_place(s), who)
                }
            } else if s.kind == "clip" {
                // A stretch of a recording is told by its times, which are also the words of the quote.
                format!("{}, a stretch of the recording, coded by {}", s.source_name, who)
            } else {
                format!("{}, characters {} to {}, coded by {}", s.source_name, s.start, s.end, who)
            };
            if let Some(speaker) = &s.speaker {
                where_from = format!("{where_from}, speaker {speaker}");
            }
            blocks.push(Block::Quote { text: s.text.trim().to_string(), source: Some(where_from) });
        }
    }
    Ok(blocks)
}

/// A number as Excel should hold it: written plainly (no leading zeros, no plus sign, no exponent, no spaces), so an identifier such as
/// `007` or a phone number stays the text it is.
fn plain_number(v: &str) -> Option<f64> {
    let t = v.trim();
    if t.is_empty() || t != v || t.starts_with('+') || t.contains(['e', 'E', ' ']) {
        return None;
    }
    let digits = t.strip_prefix('-').unwrap_or(t);
    let whole = digits.split('.').next().unwrap_or("");
    if whole.is_empty() || (whole.len() > 1 && whole.starts_with('0')) || whole.len() > 15 {
        return None;
    }
    t.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// A table of text (what the data editor makes: a table with a row for each group, a table stood on end or laid across) as a file: CSV, or
/// an Excel workbook of one sheet in which values that are plainly numbers are numbers.
pub fn export_table(name: &str, header: &[String], rows: &[Vec<Option<String>>], format: Format) -> Result<Vec<u8>, ExportError> {
    match format {
        Format::Csv => {
            let plain: Vec<Vec<String>> = rows.iter().map(|r| r.iter().map(|c| c.clone().unwrap_or_default()).collect()).collect();
            Ok(write_csv(header, &plain))
        }
        Format::Xlsx => {
            let cells: Vec<Vec<Cell>> = rows
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|c| match c {
                            None => Cell::Empty,
                            Some(v) => plain_number(v).map_or_else(|| Cell::Text(v.clone()), Cell::Number),
                        })
                        .collect()
                })
                .collect();
            let widths = header.iter().map(|h| (h.chars().count() as f64 + 2.0).clamp(10.0, 40.0)).collect();
            write_xlsx(&[Sheet { name: name.to_string(), header: header.to_vec(), rows: cells, widths, pictures: Vec::new() }])
        }
        Format::Docx | Format::Qdc => Err(ExportError::Invalid("a table is saved as CSV or Excel".to_string())),
    }
}

/// The coded segments the filter selects, in the chosen format. Word groups them by code (a retrieval report);
/// Excel and CSV list one segment per row.
pub fn export_segments(project: &Project, format: Format, filter: &SegmentFilter) -> Result<Vec<u8>, ExportError> {
    if format == Format::Qdc {
        return Err(ExportError::Invalid("the QDC format holds a codebook, not coded segments".to_string()));
    }
    let info = project.info()?;
    let segments = project.coded_segments(filter)?;

    match format {
        Format::Csv | Format::Xlsx => {
            let mut header: Vec<String> = [
                "Source", "Folder", "Unit ID", "Speaker", "Code", "Coder", "Coder type", "Layer", "How it was coded", "Text", "Start", "End", "Rationale", "Coded on",
            ]
            .map(String::from)
            .to_vec();
            // Stretches of recordings get two columns of their own, with the times in seconds, so they can be used by a program.
            let has_clips = segments.iter().any(|s| s.kind == "clip");
            if has_clips {
                header.push("Recording from (seconds)".to_string());
                header.push("Recording to (seconds)".to_string());
            }
            let text_row = |s: &CodedSegment| -> Vec<String> {
                let mut row = vec![
                    s.source_name.clone(),
                    s.folder.clone(),
                    s.unit_external_id.clone().unwrap_or_default(),
                    s.speaker.clone().unwrap_or_default(),
                    s.code_name.clone(),
                    s.coder_name.clone(),
                    s.coder_role.clone(),
                    s.layer.clone(),
                    s.origin.clone(),
                    // An area of a picture has no words: it is told in words, and has no place in a text to count from. Nor has a
                    // stretch of a recording; its times are in the columns at the end.
                    match s.kind.as_str() {
                        "image" => format!("[Area of {}] {}", if s.page.is_some() { "a PDF page" } else { "a picture" }, s.text),
                        "clip" => format!("[Stretch of a recording] {}", s.text),
                        _ => s.text.clone(),
                    },
                    if s.kind == "text" { s.start.to_string() } else { String::new() },
                    if s.kind == "text" { s.end.to_string() } else { String::new() },
                    s.rationale.clone(),
                    s.created_at.clone(),
                ];
                if has_clips {
                    let (from, to) = clip_seconds(s).map_or((String::new(), String::new()), |(a, b)| (format!("{a}"), format!("{b}")));
                    row.push(from);
                    row.push(to);
                }
                row
            };
            if format == Format::Csv {
                let rows: Vec<Vec<String>> = segments.iter().map(text_row).collect();
                return Ok(write_csv(&header, &rows));
            }
            let mut rows: Vec<Vec<Cell>> = segments
                .iter()
                .map(|s| {
                    let mut cells: Vec<Cell> = text_row(s).into_iter().map(Cell::Text).collect();
                    if s.kind == "text" {
                        cells[10] = Cell::Number(s.start as f64);
                        cells[11] = Cell::Number(s.end as f64);
                    }
                    if let Some((from, to)) = clip_seconds(s) {
                        let n = cells.len();
                        cells[n - 2] = Cell::Number(from);
                        cells[n - 1] = Cell::Number(to);
                    }
                    cells
                })
                .collect();
            // An area of a picture is shown in a column of its own, cropped out of the picture kept once in the workbook.
            let pictures = Pictures::gather(project, &segments)?;
            let mut widths = vec![24.0, 16.0, 14.0, 16.0, 22.0, 14.0, 10.0, 8.0, 14.0, 60.0, 8.0, 8.0, 36.0, 22.0];
            if has_clips {
                widths.extend([14.0, 14.0]);
            }
            let mut sheet_pictures: Vec<SheetPicture> = Vec::new();
            for (row, s) in segments.iter().enumerate() {
                if let Some(a) = pictures.area(s, EXCEL_MAX) {
                    sheet_pictures.push(SheetPicture { media: a.media, crop: a.crop, row, col: header.len(), width: a.width, height: a.height, alt: area_alt(s) });
                }
            }
            if !sheet_pictures.is_empty() {
                let widest = sheet_pictures.iter().map(|p| p.width).max().unwrap_or(0) as f64 / EMU_PER_PIXEL;
                header.push("Picture".to_string());
                widths.push((widest / 7.0 + 2.0).ceil());
                for row in rows.iter_mut() {
                    row.push(Cell::Empty);
                }
            }
            // A second sheet counts what is on the first, by code and coder.
            let mut counts: BTreeMap<(String, String), (usize, std::collections::BTreeSet<i64>)> = BTreeMap::new();
            for s in &segments {
                let e = counts.entry((s.code_name.clone(), s.coder_name.clone())).or_default();
                e.0 += 1;
                e.1.insert(s.source_id);
            }
            let summary_rows: Vec<Vec<Cell>> = counts
                .into_iter()
                .map(|((code, coder), (n, sources))| vec![Cell::Text(code), Cell::Text(coder), Cell::Number(n as f64), Cell::Number(sources.len() as f64)])
                .collect();
            write_xlsx_with_media(
                &[
                    Sheet { name: "Coded segments".into(), header, rows, widths, pictures: sheet_pictures },
                    Sheet {
                        name: "By code and coder".into(),
                        header: ["Code", "Coder", "Segments", "Sources"].map(String::from).to_vec(),
                        rows: summary_rows,
                        widths: vec![28.0, 18.0, 10.0, 10.0],
                        pictures: Vec::new(),
                    },
                ],
                &pictures.media,
            )
        }
        Format::Docx => {
            let pictures = Pictures::gather(project, &segments)?;
            let codes = segments.iter().map(|s| s.code_id).collect::<std::collections::HashSet<_>>().len();
            let mut blocks = vec![Block::Heading(1, format!("Coded segments: {}", info.name))];
            blocks.push(Block::paragraph(format!(
                "Made by PrismQDA on {}. {} segment{} under {} code{}.",
                today(),
                segments.len(),
                if segments.len() == 1 { "" } else { "s" },
                codes,
                if codes == 1 { "" } else { "s" }
            )));
            blocks.extend(segment_blocks(project, &segments, &pictures, 2)?);
            write_docx_with_media(&format!("Coded segments: {}", info.name), &blocks, &pictures.media)
        }
        Format::Qdc => unreachable!("handled above"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_a_plausible_iso_date() {
        let d = today();
        assert_eq!(d.len(), 10);
        let year: i32 = d[..4].parse().unwrap();
        assert!((2024..2100).contains(&year), "{d}");
    }

    #[test]
    fn formats_are_found_by_name() {
        assert_eq!(Format::from_name("xlsx"), Some(Format::Xlsx));
        assert_eq!(Format::from_name("pdf"), None);
        assert_eq!(Format::Qdc.extension(), "qdc");
    }
}
