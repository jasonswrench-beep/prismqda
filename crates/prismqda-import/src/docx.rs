//! DOCX import (specification section 5), written from the public ECMA-376 / ISO 29500 description of
//! WordprocessingML.
//!
//! What is kept: headings (by style or outline level), bold and italic, list items, and table text
//! (cells separated by a tab, rows by a newline, one table-cell run per cell). Embedded images become
//! the placeholder `[image]`. Tracked insertions are kept as text and tracked deletions are left out.
//! Everything that cannot be carried over (text boxes, footnotes, comments, equations, hidden text and
//! so on) is counted and reported as a warning, never dropped silently.
//!
//! This file also holds the pieces that the ODT and RTF importers share: reading zip packages with a
//! size guard, a namespace-aware XML walker, and [`ParaBuf`], which turns one paragraph of styled text
//! into builder text and format runs with honest offsets.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Cursor, Read};

use prismqda_engine::{ImportWarning, RunKind, SourceType, UnitKind};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::{NsReader, XmlVersion};
use unicode_normalization::char::canonical_combining_class;
use unicode_normalization::{is_nfc, is_nfc_quick, IsNormalized, UnicodeNormalization};
use zip::result::ZipError;
use zip::ZipArchive;

use crate::builder::{ParsedText, TextBuilder};
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;

/// The most that a package may inflate to, summed over the parts this importer reads. A zip bomb
/// (a tiny file that expands to gigabytes) is refused instead of filling the computer's memory.
pub(crate) const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

/// How deeply XML elements may nest. Real documents stay far below this.
const MAX_XML_DEPTH: usize = 400;

/// How deeply tables may nest inside table cells.
pub(crate) const MAX_TABLE_DEPTH: usize = 64;

// ---------------------------------------------------------------------------------------------
// Shared: zip packages
// ---------------------------------------------------------------------------------------------

/// An opened zip package (DOCX or ODT) that hands out parts as bytes, within a total size budget.
pub(crate) struct Package<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    remaining: u64,
    what: &'static str,
    /// Set when a part was refused for being too large (as opposed to merely damaged).
    refused: bool,
}

impl<'a> Package<'a> {
    /// `what` names the kind of file in messages, for example "Word document (DOCX)".
    pub(crate) fn open(bytes: &'a [u8], what: &'static str, limit: u64) -> Result<Self> {
        if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
            return Err(ImportError::format(format!(
                "this is an older binary Office file or a password-protected one, not a {what}; \
                 open it in the program that made it and save a copy in the open format"
            )));
        }
        let archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| zip_error(what, &e))?;
        Ok(Package { archive, remaining: limit, what, refused: false })
    }

    fn find(&self, name: &str) -> Option<String> {
        let want = normalize_part_name(name);
        // Since zip 9 a name that cannot be read (damaged or not text) comes back as an error; such a part is never matched.
        self.archive.file_names().filter_map(|n| n.ok()).find(|n| normalize_part_name(n).eq_ignore_ascii_case(&want)).map(|n| n.into_owned())
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    /// True when the last failed read was a refusal because the part would unpack too large.
    pub(crate) fn refused(&self) -> bool {
        self.refused
    }

    /// The raw bytes of a part, or `None` when the package has no such part.
    pub(crate) fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(actual) = self.find(name) else { return Ok(None) };
        let what = self.what;
        let limit = self.remaining;
        let mut file = self.archive.by_name(&actual).map_err(|e| zip_error(what, &e))?;
        if file.is_dir() {
            return Ok(None);
        }
        let declared = file.size();
        if declared > limit {
            self.refused = true;
            return Err(too_big(what, name));
        }
        let mut out = Vec::with_capacity(declared.min(1 << 20) as usize);
        // Read at most one byte past the limit: a part that lies about its size cannot get further.
        (&mut file)
            .take(limit + 1)
            .read_to_end(&mut out)
            .map_err(|e| ImportError::format(format!("the part '{name}' inside the {what} is damaged ({e})")))?;
        if out.len() as u64 > limit {
            self.refused = true;
            return Err(too_big(what, name));
        }
        self.remaining -= out.len() as u64;
        Ok(Some(out))
    }

    /// A part decoded as XML text (UTF-8 or UTF-16).
    pub(crate) fn read_xml(&mut self, name: &str) -> Result<Option<String>> {
        match self.read(name)? {
            Some(bytes) => decode_xml(bytes, self.what, name).map(Some),
            None => Ok(None),
        }
    }
}

fn normalize_part_name(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches('/').to_string()
}

fn too_big(what: &str, part: &str) -> ImportError {
    ImportError::format(format!(
        "the part '{part}' inside the {what} would unpack to more than {} MB, which is far larger than any real document; \
         the file is refused in case it is damaged or hostile",
        MAX_UNPACKED_BYTES / (1024 * 1024)
    ))
}

fn zip_error(what: &str, err: &ZipError) -> ImportError {
    match err {
        ZipError::UnsupportedArchive(m) if *m == ZipError::PASSWORD_REQUIRED => ImportError::format(format!(
            "the {what} is password-protected, so PrismQDA cannot read it; remove the password and try again"
        )),
        _ => ImportError::format(format!(
            "the file could not be opened as a {what}: it is damaged, incomplete, or not a {what} at all ({err})"
        )),
    }
}

/// Turns the bytes of an XML part into text. XML parts in Office files are UTF-8 (sometimes with a
/// byte order mark) or UTF-16.
fn decode_xml(bytes: Vec<u8>, what: &str, part: &str) -> Result<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let enc = if bytes[0] == 0xFF { encoding_rs::UTF_16LE } else { encoding_rs::UTF_16BE };
        let (text, had_errors) = enc.decode_without_bom_handling(&bytes[2..]);
        if had_errors {
            return Err(bad_text(what, part));
        }
        return Ok(text.into_owned());
    }
    let start = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let mut bytes = bytes;
    if start > 0 {
        bytes.drain(..start);
    }
    String::from_utf8(bytes).map_err(|_| bad_text(what, part))
}

fn bad_text(what: &str, part: &str) -> ImportError {
    ImportError::format(format!(
        "the part '{part}' inside the {what} is not valid text (it is not UTF-8), so the file is damaged"
    ))
}

// ---------------------------------------------------------------------------------------------
// Shared: XML walking
// ---------------------------------------------------------------------------------------------

/// The XML namespaces the importers care about, found by their URIs so that any prefix works.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ns {
    /// No namespace, or one that is not listed below.
    Other,
    Word,
    Math,
    Mc,
    Office,
    Text,
    Table,
    Draw,
    Style,
    Fo,
}

fn classify_ns(uri: &[u8]) -> Ns {
    let Ok(s) = std::str::from_utf8(uri) else { return Ns::Other };
    if s.ends_with("/wordprocessingml/2006/main") || s.ends_with("/ooxml/wordprocessingml/main") {
        Ns::Word
    } else if s.ends_with("/officeDocument/2006/math") || s.ends_with("/ooxml/math") {
        Ns::Math
    } else if s.ends_with("/markup-compatibility/2006") {
        Ns::Mc
    } else if let Some(rest) = s.strip_prefix("urn:oasis:names:tc:opendocument:xmlns:") {
        match rest {
            "office:1.0" => Ns::Office,
            "text:1.0" => Ns::Text,
            "table:1.0" => Ns::Table,
            "drawing:1.0" => Ns::Draw,
            "style:1.0" => Ns::Style,
            "xsl-fo-compatible:1.0" => Ns::Fo,
            _ => Ns::Other,
        }
    } else {
        Ns::Other
    }
}

/// One thing the XML walker found. Names are local names (without prefix).
pub(crate) enum Node<'a> {
    Start(Ns, &'a [u8], &'a BytesStart<'a>),
    Empty(Ns, &'a [u8], &'a BytesStart<'a>),
    End(Ns, &'a [u8]),
    /// Text, already decoded; entity and character references arrive as their own small pieces.
    Text(&'a str),
}

/// Walks an XML document, calling `on` for every element start, end, and piece of text. The walker
/// checks that the XML is well formed and not nested absurdly deep, and never expands custom entities.
pub(crate) fn walk_xml(xml: &str, what: &str, mut on: impl FnMut(Node<'_>) -> Result<()>) -> Result<()> {
    let mut reader = NsReader::from_str(xml);
    let mut depth = 0usize;
    let mut saw_root = false;
    loop {
        let pos = reader.buffer_position();
        let (resolved, event) = reader.read_resolved_event().map_err(|e| xml_error(what, &e.to_string(), pos))?;
        let ns = match &resolved {
            ResolveResult::Bound(ns) => classify_ns(ns.as_ref()),
            _ => Ns::Other,
        };
        match event {
            Event::Start(e) => {
                depth += 1;
                if depth > MAX_XML_DEPTH {
                    return Err(ImportError::format(format!(
                        "the {what} nests its elements more than {MAX_XML_DEPTH} levels deep, so it is damaged or hostile"
                    )));
                }
                saw_root = true;
                let local = e.local_name();
                on(Node::Start(ns, local.as_ref(), &e))?;
            }
            Event::Empty(e) => {
                saw_root = true;
                let local = e.local_name();
                on(Node::Empty(ns, local.as_ref(), &e))?;
            }
            Event::End(e) => {
                depth = depth.saturating_sub(1);
                let local = e.local_name();
                on(Node::End(ns, local.as_ref()))?;
            }
            Event::Text(t) => {
                let s = t.decode().map_err(|e| xml_error(what, &e.to_string(), pos))?;
                on(Node::Text(&s))?;
            }
            Event::CData(c) => {
                let s = c.decode().map_err(|e| xml_error(what, &e.to_string(), pos))?;
                on(Node::Text(&s))?;
            }
            Event::GeneralRef(r) => {
                let mut buf = [0u8; 4];
                match r.resolve_char_ref() {
                    // A character reference such as &#233; is the character itself.
                    Ok(Some(ch)) => on(Node::Text(ch.encode_utf8(&mut buf)))?,
                    Ok(None) => {
                        let name = r.decode().map_err(|e| xml_error(what, &e.to_string(), pos))?;
                        let ch = match name.as_ref() {
                            "amp" => Some('&'),
                            "lt" => Some('<'),
                            "gt" => Some('>'),
                            "quot" => Some('"'),
                            "apos" => Some('\''),
                            // A custom entity is never expanded: that is how "billion laughs" files attack.
                            _ => None,
                        };
                        if let Some(ch) = ch {
                            on(Node::Text(ch.encode_utf8(&mut buf)))?;
                        }
                    }
                    // An impossible character reference (such as &#0;) is ignored.
                    Err(_) => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if depth != 0 || !saw_root {
        return Err(ImportError::format(format!(
            "the {what} ends unexpectedly (it is empty or cut off), so the file is damaged or incomplete"
        )));
    }
    Ok(())
}

fn xml_error(what: &str, detail: &str, pos: u64) -> ImportError {
    ImportError::format(format!("the {what} is damaged: its XML is not well formed near byte {pos} ({detail})"))
}

/// The value of an attribute, matched by local name (prefix ignored).
pub(crate) fn attr(e: &BytesStart<'_>, local: &str) -> Option<String> {
    let mut attrs = e.attributes();
    attrs.with_checks(false);
    for a in attrs.flatten() {
        if a.key.local_name().as_ref() == local.as_bytes() {
            return a.normalized_value(XmlVersion::Implicit1_0).ok().map(Cow::into_owned);
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Shared: text of one paragraph
// ---------------------------------------------------------------------------------------------

/// The kind of break that comes before a paragraph's text when something precedes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sep {
    /// A blank line.
    Paragraph,
    /// A single line break (inside a table cell).
    Line,
}

/// Where a paragraph's text ended up: its span without leading and trailing whitespace, in scalar
/// values of the stored text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placed {
    pub trim_start: i64,
    pub trim_end: i64,
}

#[derive(Debug)]
struct Span {
    text: String,
    bold: bool,
    italic: bool,
}

/// Collects the styled text of one paragraph, then pushes it to the [`TextBuilder`] in one piece.
///
/// Why in one piece: the builder normalizes each pushed piece to NFC and its offsets are only honest
/// if pieces never split a character sequence (a letter and the accent that follows it may sit in
/// different runs of the source). So style spans are first moved so that every span after the first
/// starts at a point where normalization cannot reach back across the boundary.
#[derive(Debug, Default)]
pub(crate) struct ParaBuf {
    spans: Vec<Span>,
}

/// Removes characters that cannot live in stored text (NUL and other control characters), keeping
/// tabs and line breaks.
pub(crate) fn clean_into(out: &mut String, text: &str) {
    out.extend(text.chars().filter(|&c| c == '\t' || c == '\n' || !c.is_control()));
}

impl ParaBuf {
    pub(crate) fn push_text(&mut self, text: &str, bold: bool, italic: bool) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.spans.last_mut() {
            if last.bold == bold && last.italic == italic {
                clean_into(&mut last.text, text);
                return;
            }
        }
        let mut clean = String::with_capacity(text.len());
        clean_into(&mut clean, text);
        if !clean.is_empty() {
            self.spans.push(Span { text: clean, bold, italic });
        }
    }

    /// True when something other than whitespace has been collected.
    pub(crate) fn has_text(&self) -> bool {
        self.spans.iter().any(|s| s.text.chars().any(|c| !c.is_whitespace()))
    }

    /// Pushes the collected text to the builder and records bold and italic runs. Returns `None`
    /// (and pushes nothing) when the paragraph holds only whitespace.
    ///
    /// `sep` is put before the text, but only when the builder already holds more than `floor`
    /// scalar values, so the first paragraph of a document or table cell gets no leading break.
    /// With `heading` set, a bold or italic run that covers the whole heading is not recorded,
    /// because heading styles are usually bold and the heading run already says it.
    pub(crate) fn flush(&mut self, b: &mut TextBuilder, sep: Sep, floor: i64, heading: bool) -> Option<Placed> {
        if !self.has_text() {
            self.spans.clear();
            return None;
        }
        let spans = snap_to_safe_boundaries(std::mem::take(&mut self.spans));
        if b.len() > floor {
            match sep {
                Sep::Paragraph => b.paragraph_break(),
                Sep::Line => b.line_break(),
            }
        }
        let whole: String = spans.iter().map(|s| s.text.as_str()).collect();
        let (s0, e0) = b.push(&whole);

        // Offsets of every span relative to the start of the paragraph, in stored scalar values.
        let mut bounds: Vec<(i64, i64)> = Vec::with_capacity(spans.len());
        let mut at = 0i64;
        for s in &spans {
            let n = nfc_len(&s.text);
            bounds.push((at, at + n));
            at += n;
        }
        let total = (e0 - s0).max(0);

        let lead_all = leading_ws(spans.iter().map(|s| s.text.as_str()));
        let trail_all = trailing_ws(spans.iter().map(|s| s.text.as_str()));
        let whole_trim = (lead_all, total - trail_all);

        for (kind, pick) in [(RunKind::Bold, true), (RunKind::Italic, false)] {
            let mut i = 0;
            while i < spans.len() {
                let on = |s: &Span| if pick { s.bold } else { s.italic };
                if !on(&spans[i]) {
                    i += 1;
                    continue;
                }
                let mut j = i;
                while j + 1 < spans.len() && on(&spans[j + 1]) {
                    j += 1;
                }
                let group = || spans[i..=j].iter().map(|s| s.text.as_str());
                let a = bounds[i].0 + leading_ws(group());
                let z = bounds[j].1 - trailing_ws(group());
                let covers_heading = heading && a <= whole_trim.0 && z >= whole_trim.1;
                if z > a && !covers_heading {
                    b.add_run(s0 + a, s0 + z, kind, None);
                }
                i = j + 1;
            }
        }
        Some(Placed { trim_start: s0 + whole_trim.0, trim_end: s0 + whole_trim.1 })
    }
}

/// Moves, from the start of each span to the end of the one before it, the characters that could
/// combine with what comes before them.
fn snap_to_safe_boundaries(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for s in spans {
        let Some(prev) = out.last_mut() else {
            out.push(s);
            continue;
        };
        let k = s.text.char_indices().find(|&(_, c)| starts_safely(c)).map_or(s.text.len(), |(i, _)| i);
        prev.text.push_str(&s.text[..k]);
        if k < s.text.len() {
            out.push(Span { text: s.text[k..].to_string(), bold: s.bold, italic: s.italic });
        }
    }
    out
}

/// A character that normalization never combines with the character before it.
fn starts_safely(c: char) -> bool {
    canonical_combining_class(c) == 0 && matches!(is_nfc_quick(std::iter::once(c)), IsNormalized::Yes)
}

fn nfc_len(s: &str) -> i64 {
    if is_nfc(s) {
        s.chars().count() as i64
    } else {
        s.nfc().count() as i64
    }
}

/// Counts whitespace characters at the start of the concatenation of the pieces.
fn leading_ws<'a>(pieces: impl Iterator<Item = &'a str>) -> i64 {
    let mut n = 0;
    for p in pieces {
        for c in p.chars() {
            if !c.is_whitespace() {
                return n;
            }
            n += 1;
        }
    }
    n
}

/// Counts whitespace characters at the end of the concatenation of the pieces.
fn trailing_ws<'a>(pieces: impl DoubleEndedIterator<Item = &'a str>) -> i64 {
    let mut n = 0;
    for p in pieces.rev() {
        for c in p.chars().rev() {
            if !c.is_whitespace() {
                return n;
            }
            n += 1;
        }
    }
    n
}

// ---------------------------------------------------------------------------------------------
// Shared: scripts, plurals, run ordering
// ---------------------------------------------------------------------------------------------

/// The kind of writing system a piece of text is in. Office formats keep separate bold and italic
/// settings for complex scripts (Arabic, Hebrew, Thai, Indic) and, in ODF, for East Asian text.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Script {
    #[default]
    Western,
    Asian,
    Complex,
}

fn script_of_char(c: char) -> Option<Script> {
    if c.is_whitespace() {
        return None;
    }
    let u = c as u32;
    if matches!(
        u,
        0x0590..=0x08FF | 0x0900..=0x0DFF | 0x0E00..=0x109F | 0x1780..=0x17FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF
            | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF
    ) {
        return Some(Script::Complex);
    }
    if matches!(
        u,
        0x1100..=0x11FF | 0x2E80..=0xA4CF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF
            | 0x20000..=0x3FFFF
    ) {
        return Some(Script::Asian);
    }
    if c.is_alphabetic() {
        return Some(Script::Western);
    }
    None
}

/// The script of a piece of text, or `None` when it has only neutral characters (spaces, digits,
/// punctuation). Complex wins over East Asian, which wins over Western.
pub(crate) fn script_of(text: &str) -> Option<Script> {
    let mut found: Option<Script> = None;
    for c in text.chars() {
        match script_of_char(c) {
            Some(Script::Complex) => return Some(Script::Complex),
            Some(Script::Asian) => found = Some(Script::Asian),
            Some(Script::Western) if found.is_none() => found = Some(Script::Western),
            Some(Script::Western) | None => {}
        }
    }
    found
}

pub(crate) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Puts runs in a stable order: by start, longer runs first, then by kind.
pub(crate) fn sort_runs(parsed: &mut ParsedText) {
    parsed
        .runs
        .sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)).then(a.kind.as_str().cmp(b.kind.as_str())));
}

// ---------------------------------------------------------------------------------------------
// Shared: what could not be carried over
// ---------------------------------------------------------------------------------------------

/// Counts of everything an importer left out or replaced. Each non-zero count becomes one warning,
/// so a document with a hundred images still gives one line in the import report.
#[derive(Default, Debug)]
pub(crate) struct Counts {
    pub images: usize,
    pub text_boxes: usize,
    pub shapes: usize,
    pub footnotes: usize,
    pub comments: usize,
    pub equations: usize,
    pub phonetic: usize,
    pub hidden: usize,
    pub embedded: usize,
    pub insertions: usize,
    pub deletions: usize,
}

impl Counts {
    pub(crate) fn warnings(&self) -> Vec<ImportWarning> {
        let c = self;
        let mut out = Vec::new();
        let mut add = |kind: &str, n: usize, message: String| {
            if n > 0 {
                out.push(ImportWarning::new(kind, message));
            }
        };
        add(
            "skipped_image",
            c.images,
            format!(
                "{} replaced by the placeholder [image].",
                plural(c.images, "embedded image was", "embedded images were")
            ),
        );
        add(
            "skipped_text_box",
            c.text_boxes,
            format!(
                "{} could not be imported, so the text inside {} is missing.",
                plural(c.text_boxes, "text box", "text boxes"),
                if c.text_boxes == 1 { "it" } else { "them" }
            ),
        );
        add(
            "skipped_shape",
            c.shapes,
            format!(
                "{} (a shape, chart or diagram) could not be imported.",
                plural(c.shapes, "drawing object", "drawing objects")
            ),
        );
        add(
            "skipped_footnote",
            c.footnotes,
            format!("{} left out.", plural(c.footnotes, "footnote or endnote was", "footnotes or endnotes were")),
        );
        add("skipped_comment", c.comments, format!("{} left out.", plural(c.comments, "comment was", "comments were")));
        add(
            "skipped_equation",
            c.equations,
            format!("{} could not be imported.", plural(c.equations, "equation", "equations")),
        );
        add(
            "skipped_phonetic_guide",
            c.phonetic,
            format!(
                "{} left out; the text they belong to was kept.",
                plural(c.phonetic, "phonetic guide (ruby text) was", "phonetic guides (ruby text) were")
            ),
        );
        add(
            "skipped_hidden_text",
            c.hidden,
            format!("{} left out.", plural(c.hidden, "piece of hidden text was", "pieces of hidden text were")),
        );
        add(
            "skipped_embedded_document",
            c.embedded,
            format!("{} could not be imported.", plural(c.embedded, "embedded document", "embedded documents")),
        );
        add(
            "tracked_changes",
            c.insertions + c.deletions,
            format!(
                "The document has tracked changes: {} kept as text and {} left out, so the text reads as if every change were accepted.",
                plural(c.insertions, "insertion was", "insertions were"),
                plural(c.deletions, "deletion was", "deletions were"),
            ),
        );
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Shared: tables
// ---------------------------------------------------------------------------------------------

struct CellFrame {
    /// Where the cell's text starts.
    start: i64,
    /// The span of its non-blank content.
    first: Option<i64>,
    last: i64,
}

#[derive(Default)]
struct TableState {
    rows: usize,
    cells: usize,
    row_open: bool,
}

/// Lays text out in the conventions every importer shares: paragraphs are separated by a blank line;
/// in a table the cells of a row are separated by a tab, the rows by a line break, and each cell
/// gets one table-cell run; a cell with several paragraphs puts a line break between them.
#[derive(Default)]
pub(crate) struct Layout {
    tables: Vec<TableState>,
    cells: Vec<CellFrame>,
}

impl Layout {
    pub(crate) fn in_cell(&self) -> bool {
        !self.cells.is_empty()
    }

    pub(crate) fn begin_table(&mut self, b: &mut TextBuilder) -> Result<()> {
        if self.tables.len() >= MAX_TABLE_DEPTH {
            return Err(ImportError::format("the document nests tables too deeply, so it is damaged or hostile"));
        }
        match self.cells.last() {
            Some(c) => {
                if b.len() > c.start {
                    b.line_break();
                }
            }
            None => b.paragraph_break(),
        }
        self.tables.push(TableState::default());
        Ok(())
    }

    pub(crate) fn begin_row(&mut self, b: &mut TextBuilder) {
        if let Some(t) = self.tables.last_mut() {
            if t.rows > 0 {
                b.line_break();
            }
            t.rows += 1;
            t.cells = 0;
            t.row_open = true;
        }
    }

    pub(crate) fn begin_cell(&mut self, b: &mut TextBuilder) {
        if let Some(t) = self.tables.last_mut() {
            if t.cells > 0 {
                b.push("\t");
            }
            t.cells += 1;
        }
        self.cells.push(CellFrame { start: b.len(), first: None, last: 0 });
    }

    /// Closes the innermost open cell, recording its table-cell run.
    pub(crate) fn end_cell(&mut self, b: &mut TextBuilder) {
        if let Some(cell) = self.cells.pop() {
            if let Some(first) = cell.first {
                b.add_run(first, cell.last, RunKind::TableCell, None);
                if let Some(parent) = self.cells.last_mut() {
                    parent.first.get_or_insert(first);
                    parent.last = cell.last;
                }
            }
        }
    }

    /// True when the innermost table has a cell open.
    pub(crate) fn cell_is_open(&self) -> bool {
        !self.tables.is_empty() && self.cells.len() == self.tables.len()
    }

    pub(crate) fn end_row(&mut self, b: &mut TextBuilder) {
        if self.cell_is_open() {
            self.end_cell(b);
        }
        if let Some(t) = self.tables.last_mut() {
            t.row_open = false;
        }
    }

    pub(crate) fn end_table(&mut self, b: &mut TextBuilder) {
        if self.cell_is_open() {
            self.end_cell(b);
        }
        self.tables.pop();
    }

    /// Closes tables until at most `level` are open (0 is outside every table).
    pub(crate) fn leave_to(&mut self, b: &mut TextBuilder, level: usize) {
        while self.tables.len() > level {
            self.end_table(b);
        }
    }

    /// Makes the layout be at table nesting `level` (0 is outside every table) with a cell open to
    /// receive text, closing deeper tables and opening missing rows, cells and tables as needed.
    pub(crate) fn enter(&mut self, b: &mut TextBuilder, level: usize) -> Result<()> {
        self.leave_to(b, level);
        while self.tables.len() < level {
            self.begin_table(b)?;
            self.begin_row(b);
            if self.tables.len() < level {
                self.begin_cell(b);
            }
        }
        if let Some(t) = self.tables.last() {
            if !t.row_open {
                self.begin_row(b);
            }
            if self.cells.len() < self.tables.len() {
                self.begin_cell(b);
            }
        }
        Ok(())
    }

    /// Puts a finished paragraph into the text: after a blank line (or a line break inside a cell),
    /// with a heading or list item run when it is one, and widening the cell's content span.
    pub(crate) fn place(&mut self, b: &mut TextBuilder, buf: &mut ParaBuf, heading: Option<u8>, list: bool) {
        let (sep, floor) = match self.cells.last() {
            Some(c) => (Sep::Line, c.start),
            None => (Sep::Paragraph, 0),
        };
        if let Some(placed) = buf.flush(b, sep, floor, heading.is_some()) {
            if let Some(level) = heading {
                b.add_run(placed.trim_start, placed.trim_end, RunKind::Heading, Some(level));
            } else if list {
                b.add_run(placed.trim_start, placed.trim_end, RunKind::ListItem, None);
            }
            if let Some(c) = self.cells.last_mut() {
                c.first.get_or_insert(placed.trim_start);
                c.last = placed.trim_end;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// DOCX
// ---------------------------------------------------------------------------------------------

/// Reads the whole file and returns its text with formatting runs (headings, bold, italic, list items, table cells).
pub fn parse(bytes: &[u8], _opts: &ImportOptions) -> Result<ParsedText> {
    parse_with_limit(bytes, MAX_UNPACKED_BYTES)
}

const WHAT: &str = "Word document (DOCX)";

pub(crate) fn parse_with_limit(bytes: &[u8], limit: u64) -> Result<ParsedText> {
    let mut pkg = Package::open(bytes, WHAT, limit)?;

    // The main part is named by the package relationships; word/document.xml is the usual answer.
    let mut candidates: Vec<String> = Vec::new();
    if let Some(rels) = pkg.read_xml("_rels/.rels")? {
        if let Ok(Some(target)) = relationship_target(&rels, "/officeDocument") {
            candidates.push(resolve_target("", &target));
        }
    }
    candidates.push("word/document.xml".to_string());
    let Some(main) = candidates.into_iter().find(|c| pkg.has(c)) else {
        return Err(ImportError::format(
            "this is a zip file, but not a Word document: it has no main document part (word/document.xml)",
        ));
    };
    let document = pkg.read_xml(&main)?.ok_or_else(|| ImportError::format("the Word document has no main part"))?;

    let mut warnings: Vec<ImportWarning> = Vec::new();
    let base_dir = main.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    let file_name = main.rsplit('/').next().unwrap_or(&main).to_string();
    let styles_name = pkg
        .read_xml(&format!("{}_rels/{file_name}.rels", dir_prefix(&base_dir)))?
        .and_then(|rels| relationship_target(&rels, "/styles").ok().flatten())
        .map(|t| resolve_target(&base_dir, &t))
        .unwrap_or_else(|| format!("{}styles.xml", dir_prefix(&base_dir)));
    let styles = match pkg.read_xml(&styles_name) {
        Ok(Some(xml)) => match Styles::parse(&xml) {
            Ok(s) => s,
            Err(_) => {
                warnings.push(ImportWarning::new(
                    "unreadable_styles",
                    "The style definitions in this document could not be read, so headings may not have been recognised.",
                ));
                Styles::default()
            }
        },
        Ok(None) => Styles::default(),
        // A damaged styles part is not worth refusing the document for, but a hostile one is.
        Err(e) if pkg.refused() => return Err(e),
        Err(_) => {
            warnings.push(ImportWarning::new(
                "unreadable_styles",
                "The style definitions in this document could not be read, so headings may not have been recognised.",
            ));
            Styles::default()
        }
    };

    let mut doc = Doc::new(&styles);
    walk_xml(&document, WHAT, |node| doc.on(node))?;
    Ok(doc.finish(warnings))
}

fn dir_prefix(dir: &str) -> String {
    if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    }
}

/// The `Target` of the first internal relationship whose type ends with `type_suffix`.
fn relationship_target(rels_xml: &str, type_suffix: &str) -> Result<Option<String>> {
    let mut found: Option<String> = None;
    walk_xml(rels_xml, "package relationships", |node| {
        if let Node::Empty(_, b"Relationship", e) | Node::Start(_, b"Relationship", e) = node {
            let is_match = attr(e, "Type").is_some_and(|t| t.ends_with(type_suffix));
            let external = attr(e, "TargetMode").is_some_and(|m| m.eq_ignore_ascii_case("External"));
            if is_match && !external && found.is_none() {
                found = attr(e, "Target");
            }
        }
        Ok(())
    })?;
    Ok(found)
}

/// Resolves a relationship target against the folder of the part that holds the relationship.
pub(crate) fn resolve_target(base_dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let joined = if let Some(abs) = target.strip_prefix('/') {
        abs.to_string()
    } else {
        format!("{}{target}", dir_prefix(base_dir))
    };
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

// --- styles ----------------------------------------------------------------------------------

#[derive(Clone, Copy, Default, Debug)]
struct Fmt {
    bold: Option<bool>,
    bold_cs: Option<bool>,
    italic: Option<bool>,
    italic_cs: Option<bool>,
    hidden: Option<bool>,
}

impl Fmt {
    /// This formatting, with `other` filling in whatever is not set here.
    fn or(self, other: Fmt) -> Fmt {
        Fmt {
            bold: self.bold.or(other.bold),
            bold_cs: self.bold_cs.or(other.bold_cs),
            italic: self.italic.or(other.italic),
            italic_cs: self.italic_cs.or(other.italic_cs),
            hidden: self.hidden.or(other.hidden),
        }
    }
}

/// A toggle element such as `<w:b/>` is on unless its value says otherwise.
fn on_off(e: &BytesStart<'_>) -> bool {
    match attr(e, "val") {
        None => true,
        Some(v) => !matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "false" | "off"),
    }
}

/// Sets the bold, italic and hidden settings that a run-properties element names.
fn apply_run_prop(fmt: &mut Fmt, name: &[u8], e: &BytesStart<'_>) {
    match name {
        b"b" => fmt.bold = Some(on_off(e)),
        b"bCs" => fmt.bold_cs = Some(on_off(e)),
        b"i" => fmt.italic = Some(on_off(e)),
        b"iCs" => fmt.italic_cs = Some(on_off(e)),
        b"vanish" => fmt.hidden = Some(on_off(e)),
        _ => {}
    }
}

#[derive(Debug, Default)]
struct StyleDef {
    paragraph: bool,
    name: String,
    based_on: Option<String>,
    outline: Option<u8>,
    list: Option<bool>,
    fmt: Fmt,
}

#[derive(Debug, Default)]
struct Styles {
    by_id: HashMap<String, StyleDef>,
    default_paragraph: Option<String>,
}

/// Longest chain of parent styles that is followed; a loop in a damaged file stops here.
const MAX_STYLE_CHAIN: usize = 24;

/// The heading level a style's id or name implies ("Heading1", "heading 2", "Title").
fn level_from_name(id: &str, name: &str) -> Option<u8> {
    for s in [id, name] {
        let lower = s.trim().to_ascii_lowercase();
        if lower == "title" {
            return Some(1);
        }
        if let Some(rest) = lower.strip_prefix("heading") {
            let digits = rest.trim();
            if let Ok(n) = digits.parse::<u8>() {
                if (1..=9).contains(&n) {
                    return Some(n.min(6));
                }
            }
        }
    }
    None
}

impl Styles {
    fn parse(xml: &str) -> Result<Styles> {
        let mut styles = Styles::default();
        let mut cur: Option<(String, StyleDef, bool)> = None; // id, definition, is default
        let (mut in_ppr, mut in_rpr, mut in_numpr, mut ignore) = (false, false, false, 0usize);
        walk_xml(xml, "style definitions", |node| {
            match node {
                Node::Start(Ns::Word, name, e) | Node::Empty(Ns::Word, name, e) => {
                    let is_start = matches!(node, Node::Start(..));
                    if ignore > 0 {
                        if is_start {
                            ignore += 1;
                        }
                        return Ok(());
                    }
                    match name {
                        b"style" => {
                            let id = attr(e, "styleId").unwrap_or_default();
                            let def = StyleDef {
                                paragraph: attr(e, "type").is_none_or(|t| t == "paragraph"),
                                ..StyleDef::default()
                            };
                            let is_default =
                                attr(e, "default").is_some_and(|v| matches!(v.as_str(), "1" | "true" | "on"));
                            if is_start {
                                cur = Some((id, def, is_default));
                            } else if !id.is_empty() {
                                styles.by_id.insert(id, def);
                            }
                        }
                        // Conditional table formatting must not leak into the style itself.
                        b"tblStylePr" if is_start => ignore = 1,
                        _ if cur.is_none() => {}
                        b"name" => {
                            if let Some((_, def, _)) = cur.as_mut() {
                                def.name = attr(e, "val").unwrap_or_default();
                            }
                        }
                        b"basedOn" => {
                            if let Some((_, def, _)) = cur.as_mut() {
                                def.based_on = attr(e, "val");
                            }
                        }
                        b"pPr" if is_start => in_ppr = true,
                        b"rPr" if is_start => in_rpr = true,
                        b"numPr" if in_ppr && is_start => in_numpr = true,
                        b"numId" if in_numpr => {
                            if let Some((_, def, _)) = cur.as_mut() {
                                def.list = Some(attr(e, "val").is_some_and(|v| v.trim() != "0"));
                            }
                        }
                        b"outlineLvl" if in_ppr => {
                            if let Some((_, def, _)) = cur.as_mut() {
                                def.outline = attr(e, "val")
                                    .and_then(|v| v.trim().parse::<u8>().ok())
                                    .filter(|n| *n <= 8)
                                    .map(|n| (n + 1).min(6));
                            }
                        }
                        _ if in_rpr && !in_ppr => {
                            if let Some((_, def, _)) = cur.as_mut() {
                                apply_run_prop(&mut def.fmt, name, e);
                            }
                        }
                        _ => {}
                    }
                }
                Node::End(Ns::Word, name) => {
                    if ignore > 0 {
                        ignore -= 1;
                        return Ok(());
                    }
                    match name {
                        b"pPr" => in_ppr = false,
                        b"rPr" => in_rpr = false,
                        b"numPr" => in_numpr = false,
                        b"style" => {
                            if let Some((id, def, is_default)) = cur.take() {
                                if is_default && def.paragraph {
                                    styles.default_paragraph = Some(id.clone());
                                }
                                if !id.is_empty() {
                                    styles.by_id.insert(id, def);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Node::Start(..) if ignore > 0 => ignore += 1,
                Node::End(..) if ignore > 0 => ignore -= 1,
                _ => {}
            }
            Ok(())
        })?;
        Ok(styles)
    }

    /// Follows `id` and its parent styles, returning the first answer `f` gives.
    fn lookup<T>(&self, id: &str, f: impl Fn(&str, Option<&StyleDef>) -> Option<T>) -> Option<T> {
        let mut cur = Some(id.to_string());
        for _ in 0..MAX_STYLE_CHAIN {
            let id = cur?;
            let def = self.by_id.get(&id);
            if let Some(found) = f(&id, def) {
                return Some(found);
            }
            cur = def.and_then(|d| d.based_on.clone());
        }
        None
    }

    fn heading_level(&self, id: &str) -> Option<u8> {
        self.lookup(id, |id, def| match def {
            Some(d) => level_from_name(id, &d.name).or(d.outline),
            None => level_from_name(id, ""),
        })
    }

    fn list(&self, id: &str) -> Option<bool> {
        self.lookup(id, |_, def| def.and_then(|d| d.list))
    }

    /// Bold, italic and hidden settings of a style and its parents.
    fn fmt(&self, id: &str) -> Fmt {
        let mut out = Fmt::default();
        let mut cur = Some(id.to_string());
        for _ in 0..MAX_STYLE_CHAIN {
            let Some(id) = cur else { break };
            let def = self.by_id.get(&id);
            match def {
                Some(d) => out = out.or(d.fmt),
                None => out = out.or(builtin_char_fmt(&id)),
            }
            cur = def.and_then(|d| d.based_on.clone());
        }
        out
    }
}

/// What the built-in character styles mean when the document does not define them.
fn builtin_char_fmt(id: &str) -> Fmt {
    match id.to_ascii_lowercase().as_str() {
        "strong" => Fmt { bold: Some(true), bold_cs: Some(true), ..Fmt::default() },
        "emphasis" => Fmt { italic: Some(true), italic_cs: Some(true), ..Fmt::default() },
        "intenseemphasis" | "subtleemphasis" => Fmt { italic: Some(true), italic_cs: Some(true), ..Fmt::default() },
        _ => Fmt::default(),
    }
}

// --- the document body -----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum SkipKind {
    Silent,
    Deleted,
    Drawing,
    Equation,
    Ruby,
}

struct Skip {
    kind: SkipKind,
    /// The element depth of the skipped element itself.
    depth: usize,
    picture: bool,
    text_box: bool,
}

#[derive(Clone, Debug)]
struct ParaInfo {
    heading: Option<u8>,
    list: bool,
    base: Fmt,
}

#[derive(Default)]
struct Para {
    buf: ParaBuf,
    style: Option<String>,
    /// A direct outline level; `Some(None)` is an explicit "body text".
    outline: Option<Option<u8>>,
    num: Option<bool>,
    info: Option<ParaInfo>,
    last_script: Script,
}

impl Para {
    fn info(&mut self, styles: &Styles) -> ParaInfo {
        if let Some(i) = &self.info {
            return i.clone();
        }
        let style_id = self.style.clone().or_else(|| styles.default_paragraph.clone());
        let heading = match self.outline {
            Some(level) => level,
            None => style_id.as_deref().and_then(|id| styles.heading_level(id)),
        };
        let list = self.num.or_else(|| style_id.as_deref().and_then(|id| styles.list(id))).unwrap_or(false);
        // A heading style is usually bold; that is the heading's look, not emphasis in the text.
        let base = if heading.is_some() {
            Fmt::default()
        } else {
            style_id.as_deref().map(|id| styles.fmt(id)).unwrap_or_default()
        };
        let info = ParaInfo { heading, list, base };
        self.info = Some(info.clone());
        info
    }
}

#[derive(Default)]
struct RunState {
    fmt: Fmt,
    style: Option<String>,
    complex: bool,
    style_fmt: Option<Fmt>,
}

struct Doc<'s> {
    styles: &'s Styles,
    b: TextBuilder,
    counts: Counts,
    depth: usize,
    skip: Option<Skip>,
    /// Inside a properties container (table, row, cell, section or content-control properties).
    other_props: usize,
    in_ppr: bool,
    in_rpr: bool,
    in_numpr: bool,
    in_t: bool,
    t_buf: String,
    para: Option<Para>,
    run: Option<RunState>,
    layout: Layout,
}

/// Elements whose content is properties, not text.
fn is_props_container(name: &[u8]) -> bool {
    matches!(
        name,
        b"trPr" | b"tcPr" | b"tblPr" | b"tblPrEx" | b"sectPr" | b"sdtPr" | b"sdtEndPr" | b"tblGrid" | b"tblGridChange"
    ) || name.ends_with(b"PrChange")
}

impl<'s> Doc<'s> {
    fn new(styles: &'s Styles) -> Self {
        Doc {
            styles,
            b: TextBuilder::new(),
            counts: Counts::default(),
            depth: 0,
            skip: None,
            other_props: 0,
            in_ppr: false,
            in_rpr: false,
            in_numpr: false,
            in_t: false,
            t_buf: String::new(),
            para: None,
            run: None,
            layout: Layout::default(),
        }
    }

    fn on(&mut self, node: Node<'_>) -> Result<()> {
        match node {
            Node::Start(ns, name, e) => {
                self.depth += 1;
                if self.skip.is_some() {
                    self.scan_skipped(name);
                    return Ok(());
                }
                self.start(ns, name, e, false)
            }
            Node::Empty(ns, name, e) => {
                if self.skip.is_some() {
                    self.scan_skipped(name);
                    return Ok(());
                }
                self.start(ns, name, e, true)
            }
            Node::End(ns, name) => {
                if self.skip.as_ref().is_some_and(|s| s.depth == self.depth) {
                    if let Some(s) = self.skip.take() {
                        self.finish_skip(&s);
                    }
                    self.depth = self.depth.saturating_sub(1);
                    return Ok(());
                }
                let result = if self.skip.is_some() { Ok(()) } else { self.end(ns, name) };
                self.depth = self.depth.saturating_sub(1);
                result
            }
            Node::Text(t) => {
                if self.skip.is_none() && self.in_t {
                    self.t_buf.push_str(t);
                }
                Ok(())
            }
        }
    }

    fn begin_skip(&mut self, kind: SkipKind, empty: bool) {
        let skip = Skip { kind, depth: self.depth, picture: false, text_box: false };
        if empty {
            self.finish_skip(&skip);
        } else {
            self.skip = Some(skip);
        }
    }

    /// Looks inside a drawing that is being skipped, to tell a picture from a text box.
    fn scan_skipped(&mut self, name: &[u8]) {
        if let Some(s) = self.skip.as_mut() {
            if s.kind == SkipKind::Drawing {
                match name {
                    b"pic" | b"blip" | b"imagedata" => s.picture = true,
                    b"txbxContent" | b"txbx" | b"textbox" => s.text_box = true,
                    _ => {}
                }
            }
        }
    }

    fn finish_skip(&mut self, s: &Skip) {
        match s.kind {
            SkipKind::Silent => {}
            SkipKind::Deleted => self.counts.deletions += 1,
            SkipKind::Equation => self.counts.equations += 1,
            SkipKind::Ruby => self.counts.phonetic += 1,
            SkipKind::Drawing => {
                if s.text_box {
                    self.counts.text_boxes += 1;
                }
                if s.picture {
                    self.counts.images += 1;
                    self.ensure_para();
                    if let Some(p) = self.para.as_mut() {
                        p.buf.push_text("[image]", false, false);
                    }
                }
                if !s.text_box && !s.picture {
                    self.counts.shapes += 1;
                }
            }
        }
    }

    fn start(&mut self, ns: Ns, name: &[u8], e: &BytesStart<'_>, empty: bool) -> Result<()> {
        let word = ns == Ns::Word;
        if self.other_props > 0 {
            if word && !empty && is_props_container(name) {
                self.other_props += 1;
            }
            return Ok(());
        }
        if word && is_props_container(name) {
            if !empty {
                self.other_props = 1;
            }
            return Ok(());
        }
        match (ns, name) {
            (Ns::Mc, b"Fallback") => {
                self.begin_skip(SkipKind::Silent, empty);
                return Ok(());
            }
            (Ns::Math, b"oMathPara" | b"oMath") => {
                self.begin_skip(SkipKind::Equation, empty);
                return Ok(());
            }
            _ => {}
        }
        if word {
            match name {
                b"p" => {
                    self.finish_para();
                    self.para = Some(Para::default());
                }
                b"pPr" => self.in_ppr = true,
                b"pStyle" if self.in_ppr && !self.in_rpr => {
                    if let Some(p) = self.para.as_mut() {
                        p.style = attr(e, "val");
                    }
                }
                b"numPr" if self.in_ppr && !self.in_rpr => self.in_numpr = true,
                b"numId" if self.in_numpr => {
                    if let Some(p) = self.para.as_mut() {
                        p.num = Some(attr(e, "val").is_some_and(|v| v.trim() != "0"));
                    }
                }
                b"outlineLvl" if self.in_ppr && !self.in_rpr => {
                    if let Some(p) = self.para.as_mut() {
                        p.outline = Some(
                            attr(e, "val")
                                .and_then(|v| v.trim().parse::<u8>().ok())
                                .filter(|n| *n <= 8)
                                .map(|n| (n + 1).min(6)),
                        );
                    }
                }
                b"rPr" => self.in_rpr = true,
                b"rStyle" if self.in_rpr && !self.in_ppr => {
                    if let Some(r) = self.run.as_mut() {
                        r.style = attr(e, "val");
                    }
                }
                b"rtl" | b"cs" if self.in_rpr && !self.in_ppr => {
                    if let Some(r) = self.run.as_mut() {
                        r.complex = on_off(e);
                    }
                }
                b"b" | b"bCs" | b"i" | b"iCs" | b"vanish" if self.in_rpr && !self.in_ppr => {
                    if let Some(r) = self.run.as_mut() {
                        apply_run_prop(&mut r.fmt, name, e);
                    }
                }
                b"r" => self.run = Some(RunState::default()),
                b"t" if !self.in_ppr && !self.in_rpr => {
                    self.in_t = !empty;
                    self.t_buf.clear();
                }
                b"tab" | b"ptab" if !self.in_ppr && !self.in_rpr && self.run.is_some() => {
                    // Inside a table cell a tab would be mistaken for a cell separator.
                    let tab = if self.layout.in_cell() { " " } else { "\t" };
                    self.add_piece(tab);
                }
                b"br" if !self.in_ppr && !self.in_rpr => self.add_piece("\n"),
                b"cr" if !self.in_ppr && !self.in_rpr => self.add_piece("\n"),
                b"noBreakHyphen" if !self.in_ppr && !self.in_rpr => self.add_piece("-"),
                b"drawing" | b"pict" | b"object" => {
                    self.begin_skip(SkipKind::Drawing, empty);
                    return Ok(());
                }
                b"del" | b"moveFrom" if !self.in_rpr && !self.in_ppr => {
                    self.begin_skip(SkipKind::Deleted, empty);
                    return Ok(());
                }
                b"ins" | b"moveTo" if !self.in_rpr && !self.in_ppr => self.counts.insertions += 1,
                b"footnoteReference" | b"endnoteReference" => self.counts.footnotes += 1,
                b"commentReference" => self.counts.comments += 1,
                b"altChunk" => self.counts.embedded += 1,
                b"rt" => {
                    self.begin_skip(SkipKind::Ruby, empty);
                    return Ok(());
                }
                b"tbl" => {
                    self.finish_para();
                    self.layout.begin_table(&mut self.b)?;
                }
                b"tr" => self.layout.begin_row(&mut self.b),
                b"tc" => {
                    self.finish_para();
                    self.layout.begin_cell(&mut self.b);
                }
                _ => {}
            }
        }
        if empty {
            self.end(ns, name)?;
        }
        Ok(())
    }

    fn end(&mut self, ns: Ns, name: &[u8]) -> Result<()> {
        let word = ns == Ns::Word;
        if self.other_props > 0 {
            if word && is_props_container(name) {
                self.other_props -= 1;
            }
            return Ok(());
        }
        if !word {
            return Ok(());
        }
        match name {
            b"p" => self.finish_para(),
            b"pPr" => self.in_ppr = false,
            b"numPr" => self.in_numpr = false,
            b"rPr" => self.in_rpr = false,
            b"r" => self.run = None,
            b"t" => {
                if self.in_t {
                    self.in_t = false;
                    let text = std::mem::take(&mut self.t_buf);
                    let flat = flatten_whitespace(&text);
                    self.add_piece(&flat);
                }
            }
            b"tc" => {
                self.finish_para();
                self.layout.end_cell(&mut self.b);
            }
            b"tr" => self.layout.end_row(&mut self.b),
            b"tbl" => {
                self.finish_para();
                self.layout.end_table(&mut self.b);
            }
            _ => {}
        }
        Ok(())
    }

    fn ensure_para(&mut self) {
        if self.para.is_none() {
            self.para = Some(Para::default());
        }
    }

    /// Adds text to the paragraph being built, styled as the current run says.
    fn add_piece(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.ensure_para();
        let styles = self.styles;
        let Some(para) = self.para.as_mut() else { return };
        let info = para.info(styles);
        let (direct, forced_complex) = match self.run.as_ref() {
            Some(r) => (r.fmt, r.complex),
            None => (Fmt::default(), false),
        };
        let char_fmt = match self.run.as_mut() {
            Some(r) => match (r.style_fmt, r.style.as_deref()) {
                (Some(f), _) => f,
                (None, Some(id)) => {
                    let f = styles.fmt(id);
                    r.style_fmt = Some(f);
                    f
                }
                (None, None) => Fmt::default(),
            },
            None => Fmt::default(),
        };
        let script = script_of(text).unwrap_or(para.last_script);
        para.last_script = script;
        let complex = forced_complex || script == Script::Complex;
        let f = direct.or(char_fmt).or(info.base);
        if f.hidden == Some(true) {
            if text.chars().any(|c| !c.is_whitespace()) {
                self.counts.hidden += 1;
            }
            return;
        }
        let bold = if complex { f.bold_cs } else { f.bold }.unwrap_or(false);
        let italic = if complex { f.italic_cs } else { f.italic }.unwrap_or(false);
        para.buf.push_text(text, bold, italic);
    }

    /// Pushes the paragraph being built to the text, with its heading, list item, and cell bookkeeping.
    fn finish_para(&mut self) {
        let Some(mut para) = self.para.take() else { return };
        let info = para.info(self.styles);
        self.layout.place(&mut self.b, &mut para.buf, info.heading, info.list);
    }

    fn finish(mut self, warnings: Vec<ImportWarning>) -> ParsedText {
        self.finish_para();
        for w in warnings.into_iter().chain(self.counts.warnings()) {
            self.b.warn(w);
        }
        let mut parsed = self.b.finish(SourceType::Document, UnitKind::Sentence);
        sort_runs(&mut parsed);
        parsed
    }
}

/// Spaces, tabs and line ends inside a text element are all just spaces; a CR LF pair is one.
fn flatten_whitespace(s: &str) -> Cow<'_, str> {
    if !s.contains(['\r', '\n', '\t']) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(' ');
            }
            '\n' | '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn zip(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, content) in parts {
            w.start_file(*name, options).unwrap();
            w.write_all(content.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn doc_with_text(text: &str) -> String {
        format!(r#"<w:document xmlns:w="{W}"><w:body><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:body></w:document>"#)
    }

    #[test]
    fn a_part_that_unpacks_beyond_the_limit_is_refused_and_one_inside_it_is_read() {
        let bytes = zip(&[("word/document.xml", &doc_with_text(&"x".repeat(2_000_000)))]);
        assert!(bytes.len() < 100_000, "the zip itself is small, which is what makes a bomb dangerous");
        let err = parse_with_limit(&bytes, 1_000_000).unwrap_err().to_string();
        assert!(err.contains("would unpack to more than"), "{err}");
        assert_eq!(parse_with_limit(&bytes, 3_000_000).unwrap().text.chars().count(), 2_000_000);
    }

    #[test]
    fn the_budget_covers_all_parts_together() {
        let big = "y".repeat(600_000);
        let bytes = zip(&[
            ("word/document.xml", &doc_with_text(&big)),
            ("word/styles.xml", &format!(r#"<w:styles xmlns:w="{W}"><!-- {big} --></w:styles>"#)),
        ]);
        let err = parse_with_limit(&bytes, 1_000_000).unwrap_err().to_string();
        assert!(err.contains("would unpack to more than"), "{err}");
    }

    /// Changes the uncompressed size recorded in the zip headers of a package with one part.
    fn lie_about_size(mut bytes: Vec<u8>, claimed: u32) -> Vec<u8> {
        let find = |bytes: &[u8], sig: &[u8; 4]| bytes.windows(4).position(|w| w == sig);
        let local = find(&bytes, b"PK\x03\x04").unwrap();
        bytes[local + 22..local + 26].copy_from_slice(&claimed.to_le_bytes());
        let central = find(&bytes, b"PK\x01\x02").unwrap();
        bytes[central + 24..central + 28].copy_from_slice(&claimed.to_le_bytes());
        bytes
    }

    #[test]
    fn a_part_that_lies_about_its_size_is_still_stopped() {
        let honest = zip(&[("word/document.xml", &doc_with_text(&"z".repeat(3_000_000)))]);
        let liar = lie_about_size(honest, 100);
        // The headers say 100 bytes, so the early check passes; reading must still not run away.
        let result = parse_with_limit(&liar, 1_000_000);
        match result {
            Err(_) => {}
            Ok(parsed) => assert!(parsed.text.chars().count() <= 1_000_000),
        }
    }

    #[test]
    fn relationship_targets_resolve_against_the_part_that_holds_them() {
        assert_eq!(resolve_target("", "word/document.xml"), "word/document.xml");
        assert_eq!(resolve_target("word", "styles.xml"), "word/styles.xml");
        assert_eq!(resolve_target("word", "../customXml/item1.xml"), "customXml/item1.xml");
        assert_eq!(resolve_target("word", "/word/styles.xml"), "word/styles.xml");
        assert_eq!(resolve_target("word", "./media/../styles.xml"), "word/styles.xml");
        assert_eq!(resolve_target("a", "../../../x.xml"), "x.xml", "too many parent steps stop at the root");
    }

    #[test]
    fn the_main_part_is_found_through_the_package_relationships() {
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="/word/main.xml"/></Relationships>"#;
        let styles_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="theme/looks.xml"/></Relationships>"#;
        let body = r#"<w:p><w:pPr><w:pStyle w:val="Big"/></w:pPr><w:r><w:t>Found</w:t></w:r></w:p>"#;
        let main = format!(r#"<w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#);
        let styles = format!(
            r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="Big"><w:name w:val="heading 2"/></w:style></w:styles>"#
        );
        let bytes = zip(&[
            ("_rels/.rels", rels),
            ("word/main.xml", &main),
            ("word/_rels/main.xml.rels", styles_rels),
            ("word/theme/looks.xml", &styles),
        ]);
        let parsed = parse_with_limit(&bytes, MAX_UNPACKED_BYTES).unwrap();
        assert_eq!(parsed.text, "Found");
        assert_eq!(parsed.runs.len(), 1);
        assert_eq!(parsed.runs[0].level, Some(2));
    }

    #[test]
    fn part_names_match_whatever_the_case_and_slashes() {
        let bytes = zip(&[("Word\\Document.XML", &doc_with_text("Odd names"))]);
        assert_eq!(parse_with_limit(&bytes, MAX_UNPACKED_BYTES).unwrap().text, "Odd names");
    }

    fn build(spans: &[(&str, bool, bool)]) -> ParsedText {
        let mut b = TextBuilder::new();
        let mut buf = ParaBuf::default();
        for (text, bold, italic) in spans {
            buf.push_text(text, *bold, *italic);
        }
        buf.flush(&mut b, Sep::Paragraph, 0, false);
        b.finish(SourceType::Document, UnitKind::Sentence)
    }

    fn covered(p: &ParsedText, run: &prismqda_engine::FormatRun) -> String {
        p.text.chars().skip(run.start as usize).take((run.end - run.start) as usize).collect()
    }

    #[test]
    fn a_style_change_inside_a_letter_and_its_accent_keeps_offsets_and_nfc_honest() {
        // "e" bold, then a combining acute accent plain: the pair composes to one character, so the
        // accent joins the span before it and every offset still counts stored scalar values.
        let p = build(&[("a e", true, false), ("\u{301}x y", false, false)]);
        assert_eq!(p.text, "a \u{e9}x y");
        assert!(unicode_normalization::is_nfc(&p.text));
        assert_eq!(p.runs.len(), 1);
        assert_eq!(covered(&p, &p.runs[0]), "a \u{e9}");

        // A Hangul vowel jamo (which composes backward) at the start of a span.
        let p = build(&[("\u{1100}", true, false), ("\u{1161}\u{11a8}", false, true), ("z", false, false)]);
        assert_eq!(p.text, "\u{AC01}z");
        assert!(unicode_normalization::is_nfc(&p.text));
        assert_eq!(p.runs.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>(), vec![(0, 1)]);

        // Marks that reorder: the dot below belongs before the circumflex in NFC.
        let p = build(&[("a\u{302}", true, false), ("\u{323}!", false, false)]);
        assert_eq!(p.text, "\u{1EAD}!");
        assert!(unicode_normalization::is_nfc(&p.text));
    }

    #[test]
    fn emphasis_runs_are_trimmed_and_merged_and_whitespace_alone_is_not_emphasis() {
        let p = build(&[
            ("one ", false, false),
            ("two ", true, false),
            ("three", true, true),
            (" four", true, false),
            (" ", true, false),
            ("end", false, false),
        ]);
        assert_eq!(p.text, "one two three four end");
        let bold: Vec<String> = p.runs.iter().filter(|r| r.kind == RunKind::Bold).map(|r| covered(&p, r)).collect();
        assert_eq!(bold, vec!["two three four"]);
        let italic: Vec<String> = p.runs.iter().filter(|r| r.kind == RunKind::Italic).map(|r| covered(&p, r)).collect();
        assert_eq!(italic, vec!["three"]);

        let p = build(&[("a", false, false), (" ", true, true), ("b", false, false)]);
        assert!(p.runs.is_empty());
    }

    #[test]
    fn a_paragraph_of_only_white_space_adds_nothing_and_control_characters_are_dropped() {
        let p = build(&[(" \t\n ", true, false)]);
        assert_eq!(p.text, "");
        let p = build(&[("a\u{0}b\u{1}c\td", false, false)]);
        assert_eq!(p.text, "abc\td");
    }

    #[test]
    fn white_space_in_text_elements_becomes_single_spaces() {
        assert_eq!(flatten_whitespace("a\r\nb\rc\nd\te"), "a b c d e");
        assert_eq!(flatten_whitespace("plain"), "plain");
    }

    #[test]
    fn the_script_of_text_decides_which_formatting_applies() {
        assert_eq!(script_of("hello"), Some(Script::Western));
        assert_eq!(script_of("12, 34"), None);
        assert_eq!(script_of("日本語"), Some(Script::Asian));
        assert_eq!(script_of("мир"), Some(Script::Western));
        assert_eq!(script_of("hello مرحبا"), Some(Script::Complex));
        assert_eq!(script_of("שלום"), Some(Script::Complex));
        assert_eq!(script_of("สวัสดี"), Some(Script::Complex));
        assert_eq!(script_of("abc 日本"), Some(Script::Asian));
    }

    #[test]
    fn style_chains_that_loop_stop() {
        let xml = format!(
            r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="A"><w:basedOn w:val="B"/></w:style><w:style w:type="paragraph" w:styleId="B"><w:basedOn w:val="A"/><w:rPr><w:b/></w:rPr></w:style></w:styles>"#
        );
        let styles = Styles::parse(&xml).unwrap();
        assert_eq!(styles.heading_level("A"), None);
        assert_eq!(styles.fmt("A").bold, Some(true));
        assert_eq!(styles.list("A"), None);
    }
}
