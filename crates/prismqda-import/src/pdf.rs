//! PDF import, text layer only (specification section 5).
//!
//! What a PDF stores is not text but glyphs placed at positions, so reading it is two jobs:
//!
//! 1. Get every glyph with its position and size. This uses the `pdf-extract` crate, which reads the
//!    fonts and character maps (including ToUnicode maps, so accented letters, CJK, and emoji come
//!    out as the characters they are) and reports each glyph through its [`OutputDev`] hook.
//! 2. Rebuild lines and paragraphs from those positions. This part is ours: glyphs on one baseline
//!    make a line, hard-wrapped lines of one paragraph are joined with spaces (so sentences are not
//!    cut at the end of every line), a clear gap, a new indent, a list marker, or a change of type
//!    size starts a new paragraph, and a word cut by a hyphen at the end of a line is put back
//!    together when that is clearly what happened.
//!
//! Every page that has text gets a page run, so quotes can cite pages. A page with no text layer
//! (a scanned image, or a blank page) gets a warning and is never dropped silently.
//!
//! The reading code runs on its own thread with a large stack and every page is read inside
//! `catch_unwind`, so a damaged file is reported as a problem with that page or file and never
//! brings the program down.

use std::panic::{catch_unwind, AssertUnwindSafe};

use pdf_extract::content::{Content, Operation};
use pdf_extract::{Dictionary, Document, MediaBox, Object, ObjectId, OutputDev, OutputError, Stream, Transform};
use prismqda_engine::{ImportWarning, SourceType, UnitKind};

use crate::builder::{ParsedText, TextBuilder};
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;

/// Stack for the reading thread. Deeply nested objects in a hostile file would otherwise be able to
/// overflow the caller's stack, which cannot be caught.
const READER_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Reads the text layer, page by page, with a page run for each page so quotes can cite pages
/// (specification section 5). A page with no text layer is reported as a warning, never silently dropped.
pub fn parse(bytes: &[u8], _opts: &ImportOptions) -> Result<ParsedText> {
    let pages = on_reader_thread(|| read_pages(bytes))??;
    assemble(pages)
}

/// Keeps the PDF file itself with the document that was read from it, so its pages can be drawn and areas of them coded. When the file
/// cannot be drawn (it is damaged in a way only the text reader forgives, needs a password, or is too large to keep), the words are still imported
/// and the import report says that areas of pages cannot be coded on this one. Only a document that stays a PDF document is kept with its
/// file: a PDF the person chose to read as a transcript is words only.
pub fn keep_file(opened: crate::Opened, file_name: &str, bytes: &[u8]) -> crate::Opened {
    use prismqda_engine::MAX_PDF_BYTES;
    let sizes = if bytes.len() > MAX_PDF_BYTES {
        Err(format!("it is {} MB and PrismQDA keeps PDFs up to {} MB inside the project", bytes.len() / (1024 * 1024), MAX_PDF_BYTES / (1024 * 1024)))
    } else {
        prismqda_pdfview::page_sizes(bytes).map_err(|e| e.to_string())
    };
    keep_with(opened, file_name, bytes, sizes)
}

/// [`keep_file`] once it is known whether the pages can be drawn and how large each is (or why they cannot be).
fn keep_with(opened: crate::Opened, file_name: &str, bytes: &[u8], sizes: std::result::Result<Vec<prismqda_pdfview::PageSize>, String>) -> crate::Opened {
    use prismqda_engine::{ImportWarning, PdfAttachment, PdfData};
    let crate::Opened::Batch(mut batch) = opened else { return opened };
    if batch.sources.len() != 1 || batch.sources[0].source_type != SourceType::Pdf {
        return crate::Opened::Batch(batch);
    }
    let shown = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    match sizes {
        Ok(sizes) => {
            let pages = sizes.iter().map(|s| (s.width, s.height)).collect();
            batch.pdfs.push(PdfAttachment { source: 0, pdf: PdfData { file_name: shown.to_string(), pages, bytes: bytes.to_vec() } });
        }
        Err(why) => batch.sources[0].warnings.push(ImportWarning::new(
            "pdf_pages_not_kept",
            format!("The words were read, but the pages of this PDF were not kept, so an area of a page cannot be coded: {why}."),
        )),
    }
    crate::Opened::Batch(batch)
}

// ---------------------------------------------------------------------------------------------
// Reading glyphs
// ---------------------------------------------------------------------------------------------

/// What one page gave us.
struct PageContent {
    /// Position in the file, counting from 1. This is the number quotes cite.
    number: u32,
    lines: Vec<Line>,
    /// Reading the page failed (an error or a panic inside the PDF library) after `lines`.
    failed: bool,
}

/// One visual line of text on a page.
#[derive(Debug, Clone, PartialEq)]
struct Line {
    text: String,
    /// Distance down the page, in the direction perpendicular to the writing direction.
    v: f64,
    /// Where the line starts and ends along the writing direction.
    u0: f64,
    u1: f64,
    /// Writing direction in radians (0 for ordinary horizontal text).
    theta: f64,
    /// The largest glyph height on the line.
    size: f64,
    /// The line holds cells separated by wide gaps (a table row).
    tabular: bool,
}

fn on_reader_thread<T: Send>(work: impl FnOnce() -> T + Send) -> Result<T> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("pdf-reader".to_string())
            .stack_size(READER_STACK_BYTES)
            .spawn_scoped(scope, work)
            .map_err(|_| ImportError::format("the computer did not have enough memory to read this PDF"))?;
        handle.join().map_err(|_| damaged())
    })
}

fn damaged() -> ImportError {
    ImportError::format(
        "this PDF could not be read; it may be damaged, cut short, or saved in a way PrismQDA does not understand yet",
    )
}

fn read_pages(bytes: &[u8]) -> Result<Vec<PageContent>> {
    let mut doc = Document::load_mem(bytes).map_err(|_| damaged())?;
    if doc.is_encrypted() {
        // Many publisher files are encrypted only to restrict printing; they open with no password.
        if doc.decrypt("").is_err() {
            return Err(ImportError::format(
                "this PDF is protected by a password, so PrismQDA cannot read it; open it in a PDF reader that has \
                 the password, save an unprotected copy, and import that copy",
            ));
        }
    }
    let numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
    if numbers.is_empty() {
        return Err(ImportError::format("this PDF has no pages, or its page list is damaged"));
    }
    clean_streams(&mut doc);
    let mut pages = Vec::with_capacity(numbers.len());
    for number in numbers {
        let mut collector = Collector::default();
        let outcome = catch_unwind(AssertUnwindSafe(|| pdf_extract::output_doc_page(&doc, &mut collector, number)));
        let failed = !matches!(outcome, Ok(Ok(())));
        // Whatever was read before a failure is kept; the warning says the page is incomplete.
        let lines = collector.into_lines();
        pages.push(PageContent { number, lines, failed });
    }
    Ok(pages)
}

/// Makes the document safe and complete for `pdf-extract`, which reads text only through the
/// operators `Tj` and `TJ`:
///
/// * the operators `'` and `"` ("move to the next line and show this") are rewritten as `T*` and
///   `Tj`, because without this every line shown that way would be missing from the text;
/// * every operator that does not place or show text is dropped (paths, colours, clipping, inline
///   images), as are text operators with the wrong kind of operands, because the library panics on
///   those;
/// * images are emptied, because the library tries to read every `Do` target as a content stream.
///
/// Nothing is written back to the file; this only changes the copy in memory.
fn clean_streams(doc: &mut Document) {
    let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
    for id in pages {
        let Ok(raw) = doc.get_page_content(id) else { continue };
        let Some(clean) = clean_content(&raw) else { continue };
        let stream_id = doc.add_object(Stream::new(Dictionary::new(), clean));
        if let Ok(Object::Dictionary(page)) = doc.get_object_mut(id) {
            page.set("Contents", Object::Reference(stream_id));
        }
    }
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        let Some(Object::Stream(stream)) = doc.objects.get_mut(&id) else { continue };
        let subtype = stream.dict.get(b"Subtype").and_then(Object::as_name).ok().map(<[u8]>::to_vec);
        match subtype.as_deref() {
            Some(b"Image") => stream.set_plain_content(Vec::new()),
            Some(b"Form") => {
                if let Some(clean) = stream.decompressed_content().ok().and_then(|raw| clean_content(&raw)) {
                    stream.set_plain_content(clean);
                }
            }
            _ => {}
        }
    }
}

fn is_number(o: &Object) -> bool {
    matches!(o, Object::Integer(_) | Object::Real(_))
}

/// Rewrites one content stream as described in [`clean_streams`]. `None` means it could not be
/// parsed, so it is left as it is.
fn clean_content(raw: &[u8]) -> Option<Vec<u8>> {
    let content = Content::decode(raw).ok()?;
    let mut kept: Vec<Operation> = Vec::with_capacity(content.operations.len());
    for op in content.operations {
        let numbers = op.operands.iter().all(is_number);
        let n = op.operands.len();
        match op.operator.as_str() {
            "'" if matches!(op.operands.as_slice(), [Object::String(..)]) => {
                kept.push(Operation::new("T*", vec![]));
                kept.push(Operation::new("Tj", op.operands));
            }
            "\"" if n == 3
                && is_number(&op.operands[0])
                && is_number(&op.operands[1])
                && matches!(op.operands[2], Object::String(..)) =>
            {
                let mut operands = op.operands;
                let text = operands.pop().expect("three operands");
                let char_spacing = operands.pop().expect("three operands");
                let word_spacing = operands.pop().expect("three operands");
                kept.push(Operation::new("Tw", vec![word_spacing]));
                kept.push(Operation::new("Tc", vec![char_spacing]));
                kept.push(Operation::new("T*", vec![]));
                kept.push(Operation::new("Tj", vec![text]));
            }
            "Tj" if matches!(op.operands.as_slice(), [Object::String(..)]) => kept.push(op),
            "TJ" if matches!(op.operands.as_slice(), [Object::Array(_)]) => kept.push(op),
            "Tf" if matches!(op.operands.as_slice(), [Object::Name(_), size] if is_number(size)) => kept.push(op),
            "Tm" | "cm" if n == 6 && numbers => kept.push(op),
            "Td" | "TD" if n == 2 && numbers => kept.push(op),
            "Tc" | "Tw" | "Tz" | "TL" | "Ts" if n == 1 && numbers => kept.push(op),
            "Do" if matches!(op.operands.as_slice(), [Object::Name(_)]) => kept.push(op),
            "BT" | "ET" | "T*" | "q" | "Q" if n == 0 => kept.push(op),
            _ => {}
        }
    }
    Content { operations: kept }.encode().ok()
}

/// Receives glyphs from `pdf-extract` and groups them into lines.
#[derive(Default)]
struct Collector {
    lines: Vec<Line>,
    current: Option<Line>,
    last: Option<LastGlyph>,
}

struct LastGlyph {
    u: f64,
    v: f64,
    /// Where the next glyph would start if nothing came between.
    advance: f64,
    size: f64,
    text: String,
}

impl Collector {
    fn into_lines(mut self) -> Vec<Line> {
        self.finish_line();
        self.lines
    }

    fn finish_line(&mut self) {
        self.last = None;
        if let Some(mut line) = self.current.take() {
            let cleaned = tidy_line(&line.text);
            // The same line drawn twice in the same place is fake bold, not two lines.
            let drawn_again = self.lines.last().is_some_and(|prev| {
                prev.text == cleaned
                    && (prev.u0 - line.u0).abs() < 0.15 * line.size
                    && (prev.v - line.v).abs() < 0.15 * line.size
            });
            if !cleaned.is_empty() && !drawn_again {
                line.text = cleaned;
                self.lines.push(line);
            }
        }
    }
}

impl OutputDev for Collector {
    fn begin_page(
        &mut self,
        _page: u32,
        _media_box: &MediaBox,
        _art: Option<(f64, f64, f64, f64)>,
    ) -> std::result::Result<(), OutputError> {
        self.finish_line();
        Ok(())
    }

    fn end_page(&mut self) -> std::result::Result<(), OutputError> {
        self.finish_line();
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        spacing: f64,
        font_size: f64,
        text: &str,
    ) -> std::result::Result<(), OutputError> {
        let scale_x = trm.m11.hypot(trm.m12);
        let (cos, sin) = if scale_x > 1e-9 { (trm.m11 / scale_x, trm.m12 / scale_x) } else { (1.0, 0.0) };
        let nominal = if font_size.is_finite() && font_size.abs() > 1e-9 { font_size.abs() } else { 1.0 };
        let mut size = nominal * trm.m21.hypot(trm.m22);
        if !size.is_finite() || size < 0.01 {
            size = nominal;
        }
        let (x, y) = (trm.m31, trm.m32);
        if !x.is_finite() || !y.is_finite() {
            return Ok(());
        }
        let u = x * cos + y * sin;
        let v = x * sin - y * cos;
        let theta = sin.atan2(cos);
        let mut glyph = String::new();
        push_clean(&mut glyph, text);
        if glyph.is_empty() {
            return Ok(());
        }
        let is_space = glyph.chars().all(char::is_whitespace);
        // A font that does not say how wide its glyphs are would make every gap look like a space,
        // so assume half an em for those.
        let em_width = if width.is_finite() && width > 0.01 { width } else { 0.5 };
        let step = (em_width * nominal + spacing) * scale_x;
        let advance = if step.is_finite() { u + step } else { u };

        // Some generators draw each glyph twice, a hair apart, to fake bold. Keep one.
        if let Some(last) = &self.last {
            if !is_space && last.text == glyph && (u - last.u).abs() < 0.1 * size && (v - last.v).abs() < 0.1 * size {
                return Ok(());
            }
        }

        let continues = match (&self.current, &self.last) {
            (Some(line), Some(last)) => {
                let turned = (wrap_angle(theta - line.theta)).abs() > 0.2;
                let off_line = (v - line.v).abs() > 0.5 * line.size.max(size);
                let moved_back = u < last.u - 0.5 * size;
                !turned && !off_line && !moved_back
            }
            _ => false,
        };
        if !continues {
            self.finish_line();
            self.current = Some(Line { text: String::new(), v, u0: u, u1: u, theta, size, tabular: false });
        }
        let line = self.current.as_mut().expect("a line was just started");
        if let Some(last) = &self.last {
            let gap = u - last.advance;
            let reference = last.size.max(size);
            if gap > 1.5 * reference {
                // A wide gap: the next glyph begins another cell of a table row.
                let kept = line.text.trim_end().len();
                line.text.truncate(kept);
                line.text.push('\t');
                line.tabular = true;
            } else if gap > 0.12 * reference && !is_space && !line.text.ends_with(char::is_whitespace) {
                line.text.push(' ');
            }
        }
        line.text.push_str(&glyph);
        line.u1 = line.u1.max(advance);
        if !is_space {
            line.size = line.size.max(size);
        }
        self.last = Some(LastGlyph { u, v, advance, size, text: glyph });
        Ok(())
    }

    fn begin_word(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }

    fn end_word(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }

    fn end_line(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }
}

fn wrap_angle(a: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    let mut a = a % tau;
    if a > std::f64::consts::PI {
        a -= tau;
    } else if a < -std::f64::consts::PI {
        a += tau;
    }
    a
}

/// Appends a decoded glyph, keeping only what belongs in analysis text: ligature glyphs become their
/// letters (so a search for "office" finds "o\u{FB03}ce"), odd spaces become plain spaces, and control
/// characters are dropped.
fn push_clean(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' | '\u{FB06}' => out.push_str("st"),
            '\u{00A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\t' | '\n' | '\r' => {
                out.push(' ')
            }
            // Invisible marks that only get in the way of searching. The zero width joiner and non-joiner
            // stay: Persian spelling and emoji sequences need them.
            '\u{200B}' | '\u{FEFF}' | '\u{2060}' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
}

/// Collapses runs of spaces and trims the ends, keeping tabs between table cells.
fn tidy_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for cell in text.split('\t') {
        let cell = cell.split_whitespace().collect::<Vec<_>>().join(" ");
        if cell.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\t');
        }
        out.push_str(&cell);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Lines to paragraphs
// ---------------------------------------------------------------------------------------------

/// Words that take a hyphen when they begin a compound ("self-sufficient"), so a hyphen after one of
/// them at the end of a line is kept.
///
/// Only words that are almost always hyphenated this way are listed. Prefixes such as "anti", "inter",
/// or "pre" are also the places where typesetters cut ordinary words ("anti-biotic"), so they are left
/// out and the hyphen is removed.
const COMPOUND_PREFIXES: &[&str] = &["self", "non", "well", "ill", "half", "all", "cross", "quasi", "vice"];

/// How a line follows the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// A new paragraph starts.
    Paragraph,
    /// The same paragraph, hard-wrapped.
    Wrapped,
    /// The same block, one row below the other (table rows).
    Row,
}

fn body_pitch(pages: &[PageContent]) -> f64 {
    // The usual distance between baselines, as a multiple of the type size: the most common value
    // among neighbouring lines of one size.
    let mut ratios: Vec<f64> = Vec::new();
    for page in pages {
        for pair in page.lines.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let size = a.size.max(b.size);
            if size <= 0.0 || (a.size - b.size).abs() > 0.1 * size || (a.theta - b.theta).abs() > 0.2 {
                continue;
            }
            let ratio = (b.v - a.v) / size;
            if (0.8..=3.0).contains(&ratio) {
                ratios.push(ratio);
            }
        }
    }
    if ratios.is_empty() {
        return 1.2;
    }
    let mut bins: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for r in &ratios {
        *bins.entry((r * 10.0).round() as i64).or_default() += 1;
    }
    let (&mode, _) = bins.iter().max_by_key(|(bin, count)| (**count, std::cmp::Reverse(**bin))).expect("not empty");
    let near: Vec<f64> = ratios.iter().copied().filter(|r| ((r * 10.0).round() as i64 - mode).abs() <= 1).collect();
    (near.iter().sum::<f64>() / near.len() as f64).clamp(1.0, 3.0)
}

fn starts_with_marker(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else { return false };
    let rest = chars.as_str();
    let followed_by_space = rest.starts_with(' ') || rest.is_empty();
    match first {
        '\u{2022}' | '\u{25E6}' | '\u{25AA}' | '\u{25AB}' | '\u{25CF}' | '\u{25CB}' | '\u{25A0}' | '\u{25A1}'
        | '\u{2023}' | '\u{2043}' | '\u{F0B7}' | '\u{F0A7}' | '\u{F076}' | '*' => followed_by_space,
        '-' | '\u{2013}' | '\u{2014}' => rest.starts_with(' '),
        '(' | '0'..='9' => {
            // "1. ", "12) ", "(3) "
            let body = text.strip_prefix('(').unwrap_or(text);
            let digits = body.chars().take_while(char::is_ascii_digit).count();
            if digits == 0 || digits > 3 {
                return false;
            }
            let after = &body[digits..];
            (after.starts_with(". ") || after.starts_with(") ")) && after.len() > 2
        }
        c if c.is_ascii_lowercase() => {
            // "a) "
            rest.starts_with(") ") && rest.len() > 2
        }
        _ => false,
    }
}

fn ends_a_sentence(text: &str) -> bool {
    matches!(
        text.chars().last(),
        Some(
            '.' | '!'
                | '?'
                | ':'
                | ';'
                | '"'
                | '\''
                | '\u{201D}'
                | '\u{2019}'
                | '\u{00BB}'
                | ')'
                | ']'
                | '}'
                | '\u{2026}'
                | '\u{3002}'
                | '\u{FF01}'
                | '\u{FF1F}'
        )
    )
}

/// The edge most lines start at, in the writing direction.
fn left_edge(lines: &[Line]) -> f64 {
    let mut counts: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for l in lines {
        *counts.entry(l.u0.round() as i64).or_default() += 1;
    }
    counts
        .iter()
        .max_by_key(|(edge, count)| (**count, std::cmp::Reverse(**edge)))
        .map(|(edge, _)| *edge as f64)
        .unwrap_or(0.0)
}

fn flow(prev: &Line, next: &Line, pitch: f64, left: f64, right: f64) -> Flow {
    let size = prev.size.max(next.size).max(0.1);
    let dy = next.v - prev.v;
    let ratio = dy / size;

    if (prev.theta - next.theta).abs() > 0.2 {
        return Flow::Paragraph;
    }
    // Moved up or sideways: another column or block, not the next line.
    if ratio < 0.3 {
        return Flow::Paragraph;
    }
    // A larger or smaller type size: a heading begins or ends.
    if (prev.size - next.size).abs() > 0.15 * size {
        return Flow::Paragraph;
    }
    // Extra space between lines.
    if ratio > pitch * 1.3 && ratio - pitch > 0.25 {
        return Flow::Paragraph;
    }
    if starts_with_marker(&next.text) {
        return Flow::Paragraph;
    }
    if prev.tabular != next.tabular {
        return Flow::Paragraph;
    }
    if prev.tabular && next.tabular {
        return Flow::Row;
    }
    // First-line indent: this line starts further in than the margin and than the line above, and the
    // line above is not a list item whose continuation is indented under its text.
    if next.u0 > left + 0.9 * size && next.u0 - prev.u0 > 0.9 * size && !starts_with_marker(&prev.text) {
        return Flow::Paragraph;
    }
    // The line above stopped well short of the right edge on a full stop: the paragraph ended there.
    let width = right - left;
    if width > 0.0 && prev.u1 < right - 0.25 * width && ends_a_sentence(&prev.text) && prev.text.chars().count() < 100 {
        return Flow::Paragraph;
    }
    Flow::Wrapped
}

/// How two pieces of one paragraph are glued: what to cut from the end of the first, and what to put
/// between them.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Glue {
    cut_bytes: usize,
    between: &'static str,
}

fn glue(first: &str, second: &str) -> Glue {
    let space = Glue { cut_bytes: 0, between: " " };
    let none = Glue { cut_bytes: 0, between: "" };
    let Some(last) = first.chars().last() else { return space };
    let next_first = second.chars().next();
    let before: &str = &first[..first.len() - last.len_utf8()];
    let char_before = before.chars().last();
    match last {
        '\u{00AD}' => Glue { cut_bytes: last.len_utf8(), between: "" },
        // Some fonts map the hyphen to the minus sign (U+2212), so a minus after a letter at a line end is one too.
        '-' | '\u{2010}' | '\u{2212}' => match char_before {
            Some(c) if c.is_alphabetic() => {
                if next_first.is_some_and(char::is_lowercase) {
                    let fragment: String = before
                        .chars()
                        .rev()
                        .take_while(|c| c.is_alphabetic())
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    let lower = fragment.to_lowercase();
                    if fragment.chars().count() >= 2 && !COMPOUND_PREFIXES.contains(&lower.as_str()) {
                        return Glue { cut_bytes: last.len_utf8(), between: "" };
                    }
                }
                none
            }
            Some(c) if c.is_numeric() => none,
            _ => space,
        },
        '\u{2014}' if char_before.is_some_and(|c| !c.is_whitespace()) => none,
        _ => space,
    }
}

/// Builds a page's paragraphs from its lines.
fn paragraphs(lines: &[Line], pitch: f64) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let left = left_edge(lines);
    let right = lines.iter().filter(|l| !l.tabular).map(|l| l.u1).fold(f64::MIN, f64::max);
    let right = if right == f64::MIN { left } else { right };
    let mut paras: Vec<String> = Vec::new();
    let mut current = lines[0].text.clone();
    for pair in lines.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        match flow(prev, next, pitch, left, right) {
            Flow::Paragraph => {
                paras.push(std::mem::replace(&mut current, next.text.clone()));
            }
            Flow::Row => {
                current.push('\n');
                current.push_str(&next.text);
            }
            Flow::Wrapped => {
                let g = glue(&current, &next.text);
                current.truncate(current.len() - g.cut_bytes);
                current.push_str(g.between);
                current.push_str(&next.text);
            }
        }
    }
    paras.push(current);
    paras
}

// ---------------------------------------------------------------------------------------------
// Pages to one text
// ---------------------------------------------------------------------------------------------

/// How a page's text attaches to the text before it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Attach {
    NewParagraph,
    /// The sentence carries on from the page before: put this between the two texts.
    Continue(&'static str),
}

struct PageText {
    number: u32,
    paras: Vec<String>,
    failed: bool,
    attach: Attach,
}

fn assemble(pages: Vec<PageContent>) -> Result<ParsedText> {
    let pitch = body_pitch(&pages);
    let mut texts: Vec<PageText> = pages
        .iter()
        .map(|p| PageText {
            number: p.number,
            paras: paragraphs(&p.lines, pitch),
            failed: p.failed,
            attach: Attach::NewParagraph,
        })
        .collect();

    // A sentence that runs over a page break stays one sentence when the page before ends without
    // closing punctuation and the next page starts with a lowercase letter.
    for i in 1..texts.len() {
        let (before, after) = texts.split_at_mut(i);
        let (prev, page) = (&mut before[i - 1], &mut after[0]);
        if prev.number + 1 != page.number || prev.failed || page.failed {
            continue;
        }
        let (Some(tail), Some(head)) = (prev.paras.last(), page.paras.first()) else { continue };
        let joins = !ends_a_sentence(tail)
            && !tail.contains(['\t', '\n'])
            && !head.contains(['\t', '\n'])
            && head.chars().next().is_some_and(char::is_lowercase)
            && !starts_with_marker(head);
        if joins {
            let g = glue(tail, head);
            let tail = prev.paras.last_mut().expect("checked above");
            tail.truncate(tail.len() - g.cut_bytes);
            page.attach = Attach::Continue(g.between);
        }
    }

    let mut b = TextBuilder::new();
    let mut readable = 0usize;
    for page in &texts {
        let label = format!("page {}", page.number);
        if page.paras.is_empty() {
            let message = if page.failed {
                format!(
                    "Page {} could not be read: its content is damaged or uses a feature PrismQDA cannot read yet. \
                     Nothing from this page was imported.",
                    page.number
                )
            } else {
                format!(
                    "Page {} has no text that can be read. It may be a scanned picture of a page, or it may be blank. \
                     Scanned pages need OCR (text recognition), which PrismQDA will add in a later version.",
                    page.number
                )
            };
            b.warn(ImportWarning::new("unreadable_page", message).at(label));
            continue;
        }
        readable += 1;
        match &page.attach {
            Attach::NewParagraph => b.paragraph_break(),
            Attach::Continue(between) => {
                b.push(between);
            }
        }
        let start = b.len();
        for (k, para) in page.paras.iter().enumerate() {
            if k > 0 {
                b.paragraph_break();
            }
            b.push(para);
        }
        b.add_page(start, b.len(), page.number);
        if page.failed {
            b.warn(
                ImportWarning::new(
                    "partial_page",
                    format!("Page {} could only be read in part; some of its text may be missing.", page.number),
                )
                .at(label.clone()),
            );
        }
        if looks_garbled(&page.paras) {
            b.warn(
                ImportWarning::new(
                    "garbled_text",
                    format!(
                        "Much of the text on page {} could not be turned into readable characters (the PDF's fonts do \
                         not say which letters they hold). Check this page against the original before coding it.",
                        page.number
                    ),
                )
                .at(label),
            );
        }
    }
    if readable == 0 {
        return Err(ImportError::unsupported(
            "this PDF has no text layer: its pages are pictures of text. Scanned PDFs need OCR (text recognition), \
             which PrismQDA will add in a later version",
        ));
    }
    Ok(b.finish(SourceType::Pdf, UnitKind::Sentence))
}

/// More than a quarter of the characters are replacement or private-use characters.
fn looks_garbled(paras: &[String]) -> bool {
    let (mut bad, mut total) = (0usize, 0usize);
    for c in paras.iter().flat_map(|p| p.chars()) {
        if c.is_whitespace() {
            continue;
        }
        total += 1;
        if c == '\u{FFFD}' || ('\u{E000}'..='\u{F8FF}').contains(&c) {
            bad += 1;
        }
    }
    total >= 8 && bad * 4 > total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, v: f64, u0: f64, u1: f64, size: f64) -> Line {
        Line { text: text.to_string(), v, u0, u1, theta: 0.0, size, tabular: false }
    }

    #[test]
    fn hyphens_are_put_back_only_when_clearly_a_split_word() {
        // A split word loses its hyphen.
        assert_eq!(glue("practical informa-", "tion"), Glue { cut_bytes: 1, between: "" });
        // A soft hyphen always goes.
        assert_eq!(glue("informa\u{00AD}", "Tion"), Glue { cut_bytes: 2, between: "" });
        // A compound prefix keeps its hyphen, with no space after it.
        assert_eq!(glue("become self-", "sufficient"), Glue { cut_bytes: 0, between: "" });
        // A one-letter start ("e-mail") keeps it too.
        assert_eq!(glue("send an e-", "mail"), Glue { cut_bytes: 0, between: "" });
        // An uppercase or numeric continuation is not a split word.
        assert_eq!(glue("the pre-", "Columbian era"), Glue { cut_bytes: 0, between: "" });
        assert_eq!(glue("in COVID-", "19 data"), Glue { cut_bytes: 0, between: "" });
        // A dash with a space before it is punctuation.
        assert_eq!(glue("and then -", "nothing"), Glue { cut_bytes: 0, between: " " });
        // Ordinary line ends get a space.
        assert_eq!(glue("the end of a", "line"), Glue { cut_bytes: 0, between: " " });
    }

    #[test]
    fn a_minus_sign_after_a_letter_at_a_line_end_is_a_hyphen() {
        // Some fonts map the hyphen glyph to U+2212.
        assert_eq!(glue("practical informa\u{2212}", "tion"), Glue { cut_bytes: 3, between: "" });
        // Before a number it stays what it is.
        assert_eq!(glue("a change of \u{2212}", "5 degrees"), Glue { cut_bytes: 0, between: " " });
    }

    #[test]
    fn the_quote_operators_become_a_new_line_and_show_and_other_operators_are_dropped() {
        let out =
            clean_content(b"BT /F1 12 Tf 14 TL (one) Tj (two) ' 3 4 (three) \" 0 0 m 10 10 l S (bad) Tw 1 0 0 rg ET")
                .unwrap();
        let ops: Vec<String> = Content::decode(&out).unwrap().operations.into_iter().map(|o| o.operator).collect();
        assert_eq!(ops, ["BT", "Tf", "TL", "Tj", "T*", "Tj", "Tw", "Tc", "T*", "Tj", "ET"]);
    }

    #[test]
    fn text_operators_with_the_wrong_operands_are_dropped_instead_of_crashing_the_reader() {
        let out = clean_content(b"BT 1 2 Tm /F1 Tf 5 Tj (x) Td [ (a) ] Tj (b) TJ ET").unwrap();
        let ops: Vec<String> = Content::decode(&out).unwrap().operations.into_iter().map(|o| o.operator).collect();
        assert_eq!(ops, ["BT", "ET"]);
    }

    #[test]
    fn list_markers_are_recognised() {
        for yes in
            ["1. Seeds", "12) Seeds", "(3) Seeds", "a) Seeds", "\u{2022} Seeds", "- Seeds", "* Seeds", "\u{F0B7} Seeds"]
        {
            assert!(starts_with_marker(yes), "{yes}");
        }
        for no in ["2019. The year", "1990-1995", "Seeds", "a seed", "-5 degrees", "3.5 million", "(see below)"] {
            assert!(!starts_with_marker(no), "{no}");
        }
    }

    #[test]
    fn lines_with_a_clear_gap_make_separate_paragraphs() {
        let lines = vec![
            line("First line of one paragraph that runs to the edge", 100.0, 72.0, 540.0, 12.0),
            line("second line of the same paragraph that also runs on", 114.4, 72.0, 540.0, 12.0),
            line("Next paragraph begins here after extra space", 143.0, 72.0, 540.0, 12.0),
        ];
        let p = paragraphs(&lines, 1.2);
        assert_eq!(
            p,
            [
                "First line of one paragraph that runs to the edge second line of the same paragraph that also runs on",
                "Next paragraph begins here after extra space"
            ]
        );
    }

    #[test]
    fn a_bigger_type_size_is_a_heading_of_its_own() {
        let lines = vec![
            line("A Heading", 100.0, 72.0, 200.0, 20.0),
            line("Body text follows directly below the heading line.", 130.0, 72.0, 540.0, 12.0),
        ];
        assert_eq!(paragraphs(&lines, 1.2).len(), 2);
    }

    #[test]
    fn an_indented_line_starts_a_paragraph_without_extra_space() {
        let lines = vec![
            line("The first paragraph is set solid and fills the line", 100.0, 72.0, 540.0, 12.0),
            line("up to its end here.", 114.4, 72.0, 200.0, 12.0),
            line("The second paragraph has an indent.", 128.8, 90.0, 400.0, 12.0),
        ];
        assert_eq!(paragraphs(&lines, 1.2).len(), 2);
    }

    #[test]
    fn table_rows_stay_together_with_line_breaks() {
        let mut a = line("Name\tAge", 100.0, 72.0, 300.0, 12.0);
        a.tabular = true;
        let mut b = line("Ana\t34", 114.4, 72.0, 300.0, 12.0);
        b.tabular = true;
        assert_eq!(paragraphs(&[a, b], 1.2), ["Name\tAge\nAna\t34"]);
    }

    #[test]
    fn a_sentence_over_a_page_break_stays_whole() {
        let page = |n: u32, text: &str| PageContent {
            number: n,
            lines: vec![line(text, 100.0, 72.0, 540.0, 12.0)],
            failed: false,
        };
        let parsed =
            assemble(vec![page(1, "The committee agreed that the"), page(2, "following steps would be taken.")])
                .unwrap();
        assert_eq!(parsed.text, "The committee agreed that the following steps would be taken.");
        assert_eq!(parsed.runs.len(), 2);
        assert_eq!((parsed.runs[0].start, parsed.runs[0].end, parsed.runs[0].page), (0, 29, Some(1)));
        assert_eq!((parsed.runs[1].start, parsed.runs[1].end, parsed.runs[1].page), (30, 61, Some(2)));
    }

    #[test]
    fn a_page_that_ends_a_sentence_is_not_joined_to_the_next() {
        let page = |n: u32, text: &str| PageContent {
            number: n,
            lines: vec![line(text, 100.0, 72.0, 540.0, 12.0)],
            failed: false,
        };
        let parsed =
            assemble(vec![page(1, "That was the end."), page(2, "and this is a stray lowercase start")]).unwrap();
        assert_eq!(parsed.text, "That was the end.\n\nand this is a stray lowercase start");
    }

    #[test]
    fn empty_and_failed_pages_are_reported_by_number() {
        let text =
            |n: u32, t: &str| PageContent { number: n, lines: vec![line(t, 100.0, 72.0, 540.0, 12.0)], failed: false };
        let empty = |n: u32, failed: bool| PageContent { number: n, lines: vec![], failed };
        let parsed =
            assemble(vec![text(1, "Page one."), empty(2, false), empty(3, true), text(4, "Page four.")]).unwrap();
        assert_eq!(parsed.text, "Page one.\n\nPage four.");
        let w: Vec<(&str, Option<&str>)> =
            parsed.warnings.iter().map(|w| (w.kind.as_str(), w.location.as_deref())).collect();
        assert_eq!(w, [("unreadable_page", Some("page 2")), ("unreadable_page", Some("page 3"))]);
        assert!(parsed.warnings[0].message.contains("OCR"));
        let pages: Vec<Option<u32>> = parsed.runs.iter().map(|r| r.page).collect();
        assert_eq!(pages, [Some(1), Some(4)]);
    }

    #[test]
    fn a_pdf_with_no_text_at_all_is_refused_with_a_plain_message() {
        let err = assemble(vec![PageContent { number: 1, lines: vec![], failed: false }]).unwrap_err();
        assert!(matches!(err, ImportError::Unsupported(_)));
        assert!(err.to_string().contains("OCR"));
    }

    #[test]
    fn garbled_pages_are_flagged() {
        let junk = "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD} ok";
        let page = PageContent { number: 1, lines: vec![line(junk, 100.0, 72.0, 540.0, 12.0)], failed: false };
        let parsed = assemble(vec![page]).unwrap();
        assert_eq!(parsed.warnings[0].kind, "garbled_text");
        assert_eq!(parsed.warnings[0].location.as_deref(), Some("page 1"));
    }

    #[test]
    fn ligature_glyphs_become_letters() {
        let mut s = String::new();
        push_clean(&mut s, "o\u{FB03}ce \u{FB01}le\u{00A0}x\u{200B}y\u{0007}\u{200D}z");
        assert_eq!(s, "office file xy\u{200D}z");
    }

    #[test]
    fn the_usual_line_pitch_is_found_even_with_paragraph_gaps() {
        let mk = |vs: &[f64]| PageContent {
            number: 1,
            lines: vs.iter().map(|v| line("x", *v, 72.0, 500.0, 12.0)).collect(),
            failed: false,
        };
        let p = body_pitch(&[mk(&[100.0, 114.4, 128.8, 157.6, 172.0, 186.4, 215.2])]);
        assert!((p - 1.2).abs() < 0.05, "{p}");
    }

    #[test]
    fn pages_that_cannot_be_drawn_are_not_kept_and_the_report_says_areas_cannot_be_coded() {
        // The batch as it is before the file is kept: the words, and nothing else.
        let crate::Opened::Batch(mut words_only) = crate::open_bytes("x.pdf", include_bytes!("../tests/fixtures/pdf/three_pages.pdf"), &ImportOptions::default()).unwrap() else { panic!("a batch") };
        words_only.pdfs.clear();
        let crate::Opened::Batch(batch) = keep_with(crate::Opened::Batch(words_only), "x.pdf", b"", Err("it needs a password".to_string())) else { panic!("a batch") };
        assert!(batch.pdfs.is_empty());
        let w = batch.sources[0].warnings.iter().find(|w| w.kind == "pdf_pages_not_kept").expect("a warning");
        assert!(w.message.contains("cannot be coded") && w.message.contains("it needs a password"), "{}", w.message);
    }
}
