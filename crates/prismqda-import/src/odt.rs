//! ODT import (specification section 5), written from the public OpenDocument (ODF 1.2) description.
//!
//! The same things are kept and reported as in the DOCX importer, and the shared helpers in
//! `docx.rs` do the zip reading, XML walking, paragraph assembly, table layout, and warning counts.
//! ODF specifics handled here: styles that live in `content.xml` (automatic styles) and `styles.xml`
//! with parent styles, bold and italic settings per script (Western, East Asian, complex), the
//! XML white-space rules of paragraphs (`text:s`, `text:tab`, `text:line-break`), lists, tables with
//! covered cells, frames (images, text boxes, objects, equations), notes, annotations, and tracked changes.

use std::collections::HashMap;

use prismqda_engine::{ImportWarning, SourceType, UnitKind};
use quick_xml::events::BytesStart;

use crate::builder::{ParsedText, TextBuilder};
use crate::docx::{
    attr, script_of, sort_runs, walk_xml, Counts, Layout, Node, Ns, Package, ParaBuf, Script, MAX_UNPACKED_BYTES,
};
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;

const WHAT: &str = "OpenDocument text file (ODT)";

/// Reads the whole file and returns its text with formatting runs (headings, bold, italic, list items, table cells).
pub fn parse(bytes: &[u8], _opts: &ImportOptions) -> Result<ParsedText> {
    parse_with_limit(bytes, MAX_UNPACKED_BYTES)
}

pub(crate) fn parse_with_limit(bytes: &[u8], limit: u64) -> Result<ParsedText> {
    let mut pkg = Package::open(bytes, WHAT, limit)?;
    if let Some(mimetype) = pkg.read("mimetype")? {
        check_mimetype(&mimetype)?;
    }
    let content = pkg.read_xml("content.xml")?.ok_or_else(|| {
        ImportError::format("this is a zip file, but not an OpenDocument text file: it has no content.xml")
    })?;

    let unreadable = || {
        ImportWarning::new(
            "unreadable_styles",
            "The style definitions in this document could not be read, so headings and emphasis defined by named styles may be missing.",
        )
    };
    let mut warnings = Vec::new();
    let mut styles = Styles::default();
    match pkg.read_xml("styles.xml") {
        Ok(Some(xml)) => {
            if styles.collect(&xml).is_err() {
                warnings.push(unreadable());
            }
        }
        Ok(None) => {}
        // A damaged styles part is not worth refusing the document for, but a hostile one is.
        Err(e) if pkg.refused() => return Err(e),
        Err(_) => warnings.push(unreadable()),
    }
    styles.collect(&content)?;

    let mut doc = Doc::new(&styles);
    walk_xml(&content, WHAT, |node| doc.on(node))?;
    Ok(doc.finish(warnings))
}

fn check_mimetype(bytes: &[u8]) -> Result<()> {
    let text = String::from_utf8_lossy(bytes);
    let mimetype = text.trim();
    let Some(kind) = mimetype.strip_prefix("application/vnd.oasis.opendocument.") else { return Ok(()) };
    if matches!(kind, "text" | "text-template" | "text-master" | "text-web") {
        Ok(())
    } else {
        Err(ImportError::format(format!(
            "this is an OpenDocument {kind} file, not an OpenDocument text file; open it in the program that made it and save the text as a document"
        )))
    }
}

// --- styles ----------------------------------------------------------------------------------

/// Bold and italic settings for the three kinds of script: Western, East Asian, complex.
#[derive(Clone, Copy, Default, Debug)]
struct Fmt3 {
    bold: [Option<bool>; 3],
    italic: [Option<bool>; 3],
    hidden: Option<bool>,
}

impl Fmt3 {
    /// This formatting, with `other` filling in whatever is not set here.
    fn or(self, other: Fmt3) -> Fmt3 {
        let mut out = self;
        for i in 0..3 {
            out.bold[i] = self.bold[i].or(other.bold[i]);
            out.italic[i] = self.italic[i].or(other.italic[i]);
        }
        out.hidden = self.hidden.or(other.hidden);
        out
    }
}

fn is_bold(v: &str) -> bool {
    let v = v.trim();
    v.eq_ignore_ascii_case("bold") || v.parse::<u32>().is_ok_and(|n| n >= 600)
}

fn is_italic(v: &str) -> bool {
    let v = v.trim();
    v.eq_ignore_ascii_case("italic") || v.eq_ignore_ascii_case("oblique")
}

/// Reads the bold, italic and hidden settings of a `style:text-properties` element.
fn read_text_properties(e: &BytesStart<'_>) -> Fmt3 {
    let bold = |name: &str| attr(e, name).map(|v| is_bold(&v));
    let italic = |name: &str| attr(e, name).map(|v| is_italic(&v));
    Fmt3 {
        bold: [bold("font-weight"), bold("font-weight-asian"), bold("font-weight-complex")],
        italic: [italic("font-style"), italic("font-style-asian"), italic("font-style-complex")],
        hidden: attr(e, "display").map(|v| v.trim().eq_ignore_ascii_case("none")),
    }
}

#[derive(Default, Debug)]
struct StyleDef {
    parent: Option<String>,
    /// The human-readable name, when it differs from the internal one ("Heading 1" for "Heading_20_1").
    display: String,
    outline: Option<u8>,
    fmt: Fmt3,
}

#[derive(Default)]
struct Styles {
    paragraph: HashMap<String, StyleDef>,
    text: HashMap<String, StyleDef>,
}

/// Longest chain of parent styles that is followed; a loop in a damaged file stops here.
const MAX_STYLE_CHAIN: usize = 24;

/// The heading level a style name implies ("Heading_20_1", "Title").
fn level_from_name(name: &str) -> Option<u8> {
    let readable = name.replace("_20_", " ").to_ascii_lowercase();
    let readable = readable.trim();
    if readable == "title" {
        return Some(1);
    }
    let digits = readable.strip_prefix("heading")?.trim();
    match digits.parse::<u8>() {
        Ok(n) if (1..=9).contains(&n) => Some(n.min(6)),
        _ => None,
    }
}

impl Styles {
    /// Adds the style definitions found in an XML part (`styles.xml` or `content.xml`).
    fn collect(&mut self, xml: &str) -> Result<()> {
        let mut cur: Option<(String, String, StyleDef)> = None; // family, name, definition
        walk_xml(xml, "style definitions", |node| {
            match node {
                Node::Start(Ns::Style, b"style", e) | Node::Empty(Ns::Style, b"style", e) => {
                    let def = StyleDef {
                        parent: attr(e, "parent-style-name"),
                        display: attr(e, "display-name").unwrap_or_default(),
                        outline: attr(e, "default-outline-level")
                            .and_then(|v| v.trim().parse::<u8>().ok())
                            .filter(|n| *n >= 1)
                            .map(|n| n.min(6)),
                        fmt: Fmt3::default(),
                    };
                    let entry = (attr(e, "family").unwrap_or_default(), attr(e, "name").unwrap_or_default(), def);
                    if matches!(node, Node::Start(..)) {
                        cur = Some(entry);
                    } else {
                        self.insert(entry);
                    }
                }
                Node::Start(Ns::Style, b"text-properties", e) | Node::Empty(Ns::Style, b"text-properties", e) => {
                    if let Some((_, _, def)) = cur.as_mut() {
                        def.fmt = read_text_properties(e);
                    }
                }
                Node::End(Ns::Style, b"style") => {
                    if let Some(entry) = cur.take() {
                        self.insert(entry);
                    }
                }
                _ => {}
            }
            Ok(())
        })
    }

    fn insert(&mut self, (family, name, def): (String, String, StyleDef)) {
        if name.is_empty() {
            return;
        }
        match family.as_str() {
            "paragraph" => {
                self.paragraph.insert(name, def);
            }
            "text" => {
                self.text.insert(name, def);
            }
            _ => {}
        }
    }

    /// The bold, italic and hidden settings of a style and its parents.
    fn fmt(&self, paragraph: bool, name: &str) -> Fmt3 {
        let map = if paragraph { &self.paragraph } else { &self.text };
        let mut out = Fmt3::default();
        let mut cur = Some(name);
        for _ in 0..MAX_STYLE_CHAIN {
            let Some(def) = cur.and_then(|n| map.get(n)) else { break };
            out = out.or(def.fmt);
            cur = def.parent.as_deref();
        }
        out
    }

    /// The heading level a paragraph style stands for, through its parents.
    fn heading_level(&self, name: &str) -> Option<u8> {
        let mut cur = Some(name);
        for _ in 0..MAX_STYLE_CHAIN {
            let n = cur?;
            let def = self.paragraph.get(n);
            if let Some(level) = level_from_name(n)
                .or_else(|| def.and_then(|d| level_from_name(&d.display)))
                .or_else(|| def.and_then(|d| d.outline))
            {
                return Some(level);
            }
            cur = def.and_then(|d| d.parent.as_deref());
        }
        None
    }
}

// --- the document body -----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum SkipKind {
    Silent,
    Note,
    Comment,
    Ruby,
    Tracked,
    Frame,
    Shape,
}

struct Skip {
    kind: SkipKind,
    /// The element depth of the skipped element itself.
    depth: usize,
    image: bool,
    text_box: bool,
    object: bool,
    math: bool,
}

struct Para {
    buf: ParaBuf,
    heading: Option<u8>,
    list: bool,
    base: Fmt3,
    /// The last character added was a space (or a break), so XML white space that follows is dropped.
    last_space: bool,
    last_script: Script,
}

impl Para {
    fn new(heading: Option<u8>, list: bool, base: Fmt3) -> Para {
        Para {
            buf: ParaBuf::default(),
            heading,
            list: list && heading.is_none(),
            base,
            last_space: true,
            last_script: Script::Western,
        }
    }
}

struct Doc<'s> {
    styles: &'s Styles,
    b: TextBuilder,
    counts: Counts,
    depth: usize,
    skip: Option<Skip>,
    para: Option<Para>,
    /// The effective formatting inside nested spans and links; empty means the paragraph's own.
    fmt_stack: Vec<Fmt3>,
    /// How many list items enclose the current place, and the value outside each open table cell.
    list_depth: usize,
    saved_list_depth: Vec<usize>,
    layout: Layout,
}

/// Drawing shapes that carry no picture: their text and look cannot be imported.
fn is_shape(name: &[u8]) -> bool {
    matches!(
        name,
        b"custom-shape"
            | b"rect"
            | b"ellipse"
            | b"line"
            | b"polyline"
            | b"polygon"
            | b"path"
            | b"circle"
            | b"connector"
            | b"caption"
            | b"g"
            | b"control"
            | b"measure"
            | b"regular-polygon"
            | b"page-thumbnail"
    )
}

impl<'s> Doc<'s> {
    fn new(styles: &'s Styles) -> Self {
        Doc {
            styles,
            b: TextBuilder::new(),
            counts: Counts::default(),
            depth: 0,
            skip: None,
            para: None,
            fmt_stack: Vec::new(),
            list_depth: 0,
            saved_list_depth: Vec::new(),
            layout: Layout::default(),
        }
    }

    fn on(&mut self, node: Node<'_>) -> Result<()> {
        match node {
            Node::Start(ns, name, e) => {
                self.depth += 1;
                if self.skip.is_some() {
                    self.scan_skipped(ns, name);
                    return Ok(());
                }
                self.start(ns, name, e, false)
            }
            Node::Empty(ns, name, e) => {
                if self.skip.is_some() {
                    self.scan_skipped(ns, name);
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
                if self.skip.is_none() && self.para.is_some() {
                    self.add_text(t);
                }
                Ok(())
            }
        }
    }

    fn begin_skip(&mut self, kind: SkipKind, empty: bool) {
        let skip = Skip { kind, depth: self.depth, image: false, text_box: false, object: false, math: false };
        if empty {
            self.finish_skip(&skip);
        } else {
            self.skip = Some(skip);
        }
    }

    /// Looks inside a skipped element: what a frame holds, and which tracked changes there are.
    fn scan_skipped(&mut self, ns: Ns, name: &[u8]) {
        let Some(s) = self.skip.as_mut() else { return };
        match s.kind {
            SkipKind::Frame => match (ns, name) {
                (Ns::Draw, b"image") => s.image = true,
                (Ns::Draw, b"text-box") => s.text_box = true,
                (Ns::Draw, b"object" | b"object-ole" | b"plugin" | b"floating-frame" | b"applet") => s.object = true,
                (_, b"math") => s.math = true,
                _ => {}
            },
            SkipKind::Tracked => match (ns, name) {
                (Ns::Text, b"deletion") => self.counts.deletions += 1,
                (Ns::Text, b"insertion") => self.counts.insertions += 1,
                _ => {}
            },
            _ => {}
        }
    }

    fn finish_skip(&mut self, s: &Skip) {
        match s.kind {
            SkipKind::Silent | SkipKind::Tracked => {}
            SkipKind::Note => self.counts.footnotes += 1,
            SkipKind::Comment => self.counts.comments += 1,
            SkipKind::Ruby => self.counts.phonetic += 1,
            SkipKind::Shape => self.counts.shapes += 1,
            SkipKind::Frame => {
                if s.text_box {
                    self.counts.text_boxes += 1;
                }
                if s.object {
                    // An object's own preview picture is not a separate image.
                    if s.math {
                        self.counts.equations += 1;
                    } else {
                        self.counts.shapes += 1;
                    }
                } else if s.image {
                    self.image_placeholder();
                } else if !s.text_box {
                    self.counts.shapes += 1;
                }
            }
        }
    }

    fn image_placeholder(&mut self) {
        self.counts.images += 1;
        self.ensure_para();
        if let Some(p) = self.para.as_mut() {
            p.buf.push_text("[image]", false, false);
            p.last_space = false;
        }
    }

    fn start(&mut self, ns: Ns, name: &[u8], e: &BytesStart<'_>, empty: bool) -> Result<()> {
        match (ns, name) {
            (
                Ns::Office,
                b"automatic-styles" | b"styles" | b"master-styles" | b"font-face-decls" | b"scripts" | b"settings"
                | b"forms" | b"meta",
            ) => {
                self.begin_skip(SkipKind::Silent, empty);
                return Ok(());
            }
            (Ns::Office, b"annotation") => {
                self.begin_skip(SkipKind::Comment, empty);
                return Ok(());
            }
            (Ns::Text, b"note") => {
                self.begin_skip(SkipKind::Note, empty);
                return Ok(());
            }
            (Ns::Text, b"ruby-text") => {
                self.begin_skip(SkipKind::Ruby, empty);
                return Ok(());
            }
            (Ns::Text, b"tracked-changes") => {
                self.begin_skip(SkipKind::Tracked, empty);
                return Ok(());
            }
            (Ns::Text, b"sequence-decls" | b"variable-decls" | b"user-field-decls" | b"dde-connection-decls") => {
                self.begin_skip(SkipKind::Silent, empty);
                return Ok(());
            }
            // The template an index is built from, not text of the document.
            (Ns::Text, n) if n.ends_with(b"-source") => {
                self.begin_skip(SkipKind::Silent, empty);
                return Ok(());
            }
            (Ns::Draw, b"frame") => {
                self.begin_skip(SkipKind::Frame, empty);
                return Ok(());
            }
            (Ns::Draw, n) if is_shape(n) => {
                self.begin_skip(SkipKind::Shape, empty);
                return Ok(());
            }
            (Ns::Draw, b"image") => {
                // A picture outside any frame: unusual, but it is still a picture.
                self.image_placeholder();
                return Ok(());
            }
            _ => {}
        }
        match (ns, name) {
            (Ns::Text, b"p") => {
                self.finish_para();
                let style = attr(e, "style-name");
                let heading = style.as_deref().and_then(|s| self.styles.heading_level(s));
                self.begin_para(style.as_deref(), heading);
            }
            (Ns::Text, b"h") => {
                self.finish_para();
                let style = attr(e, "style-name");
                let level =
                    attr(e, "outline-level").and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(1).clamp(1, 6);
                self.begin_para(style.as_deref(), Some(level as u8));
            }
            (Ns::Text, b"span" | b"a") => {
                let current = self.current_fmt();
                let fmt = match attr(e, "style-name") {
                    Some(name) => self.styles.fmt(false, &name).or(current),
                    None => current,
                };
                self.fmt_stack.push(fmt);
            }
            (Ns::Text, b"s") => {
                let n = attr(e, "c").and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(1).min(1000);
                self.add_literal(&" ".repeat(n));
            }
            (Ns::Text, b"tab") => {
                // Inside a table cell a tab would be mistaken for a cell separator.
                let tab = if self.layout.in_cell() { " " } else { "\t" };
                self.add_literal(tab);
            }
            (Ns::Text, b"line-break") => self.add_literal("\n"),
            (Ns::Text, b"list-item" | b"list-header" | b"numbered-paragraph") => self.list_depth += 1,
            (Ns::Table, b"table") => {
                self.finish_para();
                self.layout.begin_table(&mut self.b)?;
            }
            (Ns::Table, b"table-row") => self.layout.begin_row(&mut self.b),
            (Ns::Table, b"table-cell" | b"covered-table-cell") => {
                self.finish_para();
                self.layout.begin_cell(&mut self.b);
                self.saved_list_depth.push(self.list_depth);
                self.list_depth = 0;
            }
            _ => {}
        }
        if empty {
            self.end(ns, name)?;
        }
        Ok(())
    }

    fn end(&mut self, ns: Ns, name: &[u8]) -> Result<()> {
        match (ns, name) {
            (Ns::Text, b"p" | b"h") => self.finish_para(),
            (Ns::Text, b"span" | b"a") => {
                self.fmt_stack.pop();
            }
            (Ns::Text, b"list-item" | b"list-header" | b"numbered-paragraph") => {
                self.finish_para();
                self.list_depth = self.list_depth.saturating_sub(1);
            }
            (Ns::Table, b"table") => {
                self.finish_para();
                self.layout.end_table(&mut self.b);
            }
            (Ns::Table, b"table-row") => self.layout.end_row(&mut self.b),
            (Ns::Table, b"table-cell" | b"covered-table-cell") => {
                self.finish_para();
                self.layout.end_cell(&mut self.b);
                self.list_depth = self.saved_list_depth.pop().unwrap_or(0);
            }
            _ => {}
        }
        Ok(())
    }

    fn begin_para(&mut self, style: Option<&str>, heading: Option<u8>) {
        // A heading style is usually bold; that is the heading's look, not emphasis in the text.
        let base = match (heading, style) {
            (None, Some(s)) => self.styles.fmt(true, s),
            _ => Fmt3::default(),
        };
        self.fmt_stack.clear();
        self.para = Some(Para::new(heading, self.list_depth > 0, base));
    }

    fn ensure_para(&mut self) {
        if self.para.is_none() {
            self.fmt_stack.clear();
            self.para = Some(Para::new(None, false, Fmt3::default()));
        }
    }

    fn current_fmt(&self) -> Fmt3 {
        match (self.fmt_stack.last(), self.para.as_ref()) {
            (Some(f), _) => *f,
            (None, Some(p)) => p.base,
            (None, None) => Fmt3::default(),
        }
    }

    /// Text from the document: runs of XML white space count as one space, and none is kept at
    /// the start of a paragraph.
    fn add_text(&mut self, raw: &str) {
        let Some(para) = self.para.as_mut() else { return };
        let mut out = String::with_capacity(raw.len());
        for c in raw.chars() {
            if matches!(c, ' ' | '\t' | '\r' | '\n') {
                if !para.last_space {
                    out.push(' ');
                    para.last_space = true;
                }
            } else {
                out.push(c);
                para.last_space = false;
            }
        }
        self.add_piece(&out);
    }

    /// Spaces, tabs and line breaks the document asked for explicitly.
    fn add_literal(&mut self, text: &str) {
        self.ensure_para();
        self.add_piece(text);
        if let Some(para) = self.para.as_mut() {
            para.last_space = true;
        }
    }

    fn add_piece(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let fmt = self.current_fmt();
        let Some(para) = self.para.as_mut() else { return };
        let script = script_of(text).unwrap_or(para.last_script);
        para.last_script = script;
        if fmt.hidden == Some(true) {
            if text.chars().any(|c| !c.is_whitespace()) {
                self.counts.hidden += 1;
            }
            return;
        }
        let i = match script {
            Script::Western => 0,
            Script::Asian => 1,
            Script::Complex => 2,
        };
        para.buf.push_text(text, fmt.bold[i].unwrap_or(false), fmt.italic[i].unwrap_or(false));
    }

    fn finish_para(&mut self) {
        let Some(mut para) = self.para.take() else { return };
        self.fmt_stack.clear();
        self.layout.place(&mut self.b, &mut para.buf, para.heading, para.list);
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

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0""#;

    fn zip(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, content) in parts {
            w.start_file(*name, options).unwrap();
            w.write_all(content.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn content(auto: &str, body: &str) -> String {
        format!(
            r#"<office:document-content {NS}><office:automatic-styles>{auto}</office:automatic-styles><office:body><office:text>{body}</office:text></office:body></office:document-content>"#
        )
    }

    #[test]
    fn a_part_that_unpacks_beyond_the_limit_is_refused() {
        let big = format!("<text:p>{}</text:p>", "x".repeat(2_000_000));
        let bytes = zip(&[("content.xml", &content("", &big))]);
        let err = parse_with_limit(&bytes, 1_000_000).unwrap_err().to_string();
        assert!(err.contains("would unpack to more than"), "{err}");
        assert_eq!(parse_with_limit(&bytes, 3_000_000).unwrap().text.chars().count(), 2_000_000);
    }

    #[test]
    fn style_chains_that_loop_stop() {
        let auto = r#"<style:style style:name="A" style:family="text" style:parent-style-name="B"/><style:style style:name="B" style:family="text" style:parent-style-name="A"><style:text-properties fo:font-weight="bold"/></style:style><style:style style:name="PA" style:family="paragraph" style:parent-style-name="PB"/><style:style style:name="PB" style:family="paragraph" style:parent-style-name="PA"/>"#;
        let body = r#"<text:p text:style-name="PA">x <text:span text:style-name="A">loop</text:span></text:p>"#;
        let parsed = parse_with_limit(&zip(&[("content.xml", &content(auto, body))]), MAX_UNPACKED_BYTES).unwrap();
        assert_eq!(parsed.text, "x loop");
        assert_eq!(parsed.runs.len(), 1);
    }

    #[test]
    fn absurd_numbers_in_attributes_are_harmless() {
        let body = r#"<text:p>a<text:s text:c="99999999999999999999"/>b<text:s text:c="4000000000"/>c</text:p><text:h text:outline-level="300">Deep</text:h>"#;
        let parsed = parse_with_limit(&zip(&[("content.xml", &content("", body))]), MAX_UNPACKED_BYTES).unwrap();
        // A count that does not fit means one space; a huge one is capped.
        assert_eq!(parsed.text.chars().count(), "a b".len() + 1000 + 1 + "\n\nDeep".len());
        assert_eq!(parsed.runs.len(), 1);
        assert_eq!(parsed.runs[0].level, Some(6));
    }

    #[test]
    fn the_weights_and_styles_odf_uses_are_understood() {
        assert!(is_bold("bold") && is_bold("700") && is_bold("600") && is_bold(" Bold "));
        assert!(!is_bold("normal") && !is_bold("400") && !is_bold("garbage"));
        assert!(is_italic("italic") && is_italic("oblique") && !is_italic("normal"));
        assert_eq!(level_from_name("Heading_20_3"), Some(3));
        assert_eq!(level_from_name("Heading 12"), None);
        assert_eq!(level_from_name("Title"), Some(1));
        assert_eq!(level_from_name("Heading"), None);
    }
}
