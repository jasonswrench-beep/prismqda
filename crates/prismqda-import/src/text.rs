//! Plain text, Markdown, and HTML import (specification section 5).

use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, ParseOpts};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use prismqda_engine::{ImportWarning, RunKind, SourceType, UnitKind};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::builder::{ParsedText, TextBuilder};
use crate::error::Result;
use crate::options::ImportOptions;

/// Turns bytes into text. Handles a byte-order mark, UTF-16, and UTF-8; anything else is read as
/// Windows-1252 (Western European) with a warning, since guessing wrong is worse than saying so.
pub(crate) fn decode(bytes: &[u8]) -> (String, Option<ImportWarning>) {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(rest).into_owned(), None);
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (s, _, _) = encoding_rs::UTF_16LE.decode(bytes);
        return (s.into_owned(), None);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (s, _, _) = encoding_rs::UTF_16BE.decode(bytes);
        return (s.into_owned(), None);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), None),
        Err(_) => {
            let (s, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            let warning = ImportWarning::new(
                "assumed_encoding",
                "The file is not valid UTF-8, so it was read as Windows-1252 (Western European). Accented letters may look wrong if it was saved in another encoding.",
            );
            (s.into_owned(), Some(warning))
        }
    }
}

/// Unifies line endings and removes characters that cannot be part of text (controls other than tab and newline).
pub(crate) fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\u{000c}' => out.push_str("\n\n"),
            '\t' | '\n' => out.push(c),
            '\u{feff}' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Plain text. Blank lines separate paragraphs; line breaks inside a paragraph are kept.
pub fn parse_txt(bytes: &[u8], _opts: &ImportOptions) -> Result<ParsedText> {
    let (text, warning) = decode(bytes);
    let text = clean(&text);
    let mut b = TextBuilder::new();
    if let Some(w) = warning {
        b.warn(w);
    }
    let mut block: Vec<&str> = Vec::new();
    let flush = |b: &mut TextBuilder, block: &mut Vec<&str>| {
        if !block.is_empty() {
            b.paragraph_break();
            b.push(&block.join("\n"));
            block.clear();
        }
    };
    for line in text.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            flush(&mut b, &mut block);
        } else {
            block.push(line);
        }
    }
    flush(&mut b, &mut block);
    Ok(b.finish(SourceType::Document, UnitKind::Sentence))
}

/// Markdown, read as CommonMark with tables. Headings, emphasis, list items, and table cells are kept as format runs.
pub fn parse_markdown(bytes: &[u8], _opts: &ImportOptions) -> Result<ParsedText> {
    let (text, warning) = decode(bytes);
    let text = clean(&text);
    let mut b = TextBuilder::new();
    if let Some(w) = warning {
        b.warn(w);
    }

    #[derive(Clone, Copy)]
    enum Open {
        Heading(u8),
        Bold,
        Italic,
        Item,
        Cell,
    }
    let mut stack: Vec<(Open, i64)> = Vec::new();
    let mut first_cell = true;
    let mut in_image = 0usize;
    let (mut images, mut html) = (0usize, 0usize);

    let close = |b: &mut TextBuilder, stack: &mut Vec<(Open, i64)>| {
        if let Some((open, start)) = stack.pop() {
            let end = b.len();
            match open {
                Open::Heading(l) => b.add_run(start, end, RunKind::Heading, Some(l)),
                Open::Bold => b.add_run(start, end, RunKind::Bold, None),
                Open::Italic => b.add_run(start, end, RunKind::Italic, None),
                Open::Item => b.add_run(start, end, RunKind::ListItem, None),
                Open::Cell => b.add_run(start, end, RunKind::TableCell, None),
            }
        }
    };

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    for event in Parser::new_ext(&text, options) {
        match event {
            Event::Start(Tag::Paragraph) | Event::Start(Tag::BlockQuote(_)) | Event::Start(Tag::CodeBlock(_)) => {
                b.paragraph_break()
            }
            Event::Start(Tag::Heading { level, .. }) => {
                b.paragraph_break();
                let l = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                stack.push((Open::Heading(l), b.len()));
            }
            Event::End(TagEnd::Heading(_)) | Event::End(TagEnd::Item) | Event::End(TagEnd::Emphasis) | Event::End(TagEnd::Strong) | Event::End(TagEnd::TableCell) => {
                close(&mut b, &mut stack)
            }
            Event::End(TagEnd::Paragraph) | Event::End(TagEnd::CodeBlock) | Event::End(TagEnd::BlockQuote(_)) => b.paragraph_break(),
            Event::Start(Tag::Item) => {
                b.paragraph_break();
                stack.push((Open::Item, b.len()));
            }
            Event::Start(Tag::Emphasis) => stack.push((Open::Italic, b.len())),
            Event::Start(Tag::Strong) => stack.push((Open::Bold, b.len())),
            Event::Start(Tag::Table(_)) => b.paragraph_break(),
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                b.line_break();
                first_cell = true;
            }
            Event::Start(Tag::TableCell) => {
                if !first_cell {
                    b.push("\t");
                }
                first_cell = false;
                stack.push((Open::Cell, b.len()));
            }
            Event::Start(Tag::Image { .. }) => {
                images += 1;
                in_image += 1;
                b.push("[image]");
            }
            Event::End(TagEnd::Image) => in_image = in_image.saturating_sub(1),
            Event::Text(t) | Event::Code(t) if in_image == 0 => {
                b.push(&t);
            }
            Event::SoftBreak | Event::HardBreak if in_image == 0 => {
                b.push("\n");
            }
            Event::Html(_) | Event::InlineHtml(_) => html += 1,
            Event::Rule => b.paragraph_break(),
            _ => {}
        }
    }
    if images > 0 {
        b.warn(ImportWarning::new("skipped_image", format!("{images} image(s) were replaced by the placeholder [image]")));
    }
    if html > 0 {
        b.warn(ImportWarning::new("skipped_html", format!("{html} piece(s) of raw HTML inside the Markdown were not imported")));
    }
    Ok(b.finish(SourceType::Document, UnitKind::Sentence))
}

/// HTML. Scripts, styles, and the page head are skipped; block elements become paragraphs.
pub fn parse_html(bytes: &[u8], _opts: &ImportOptions) -> Result<ParsedText> {
    let (text, warning) = decode(bytes);
    let dom = parse_document(RcDom::default(), ParseOpts::default()).one(text.as_str());
    let mut w = HtmlWalker::default();
    if let Some(warning) = warning {
        w.b.warn(warning);
    }
    w.walk(&dom.document, 0);
    let HtmlWalker { mut b, images, too_deep, .. } = w;
    if images > 0 {
        b.warn(ImportWarning::new("skipped_image", format!("{images} image(s) were replaced by the placeholder [image]")));
    }
    if too_deep > 0 {
        b.warn(ImportWarning::new("too_deeply_nested", format!("{too_deep} deeply nested element(s) were not read")));
    }
    Ok(b.finish(SourceType::Document, UnitKind::Sentence))
}

#[derive(Default)]
struct HtmlWalker {
    b: TextBuilder,
    pre: bool,
    pending_space: bool,
    in_cell: bool,
    images: usize,
    too_deep: usize,
}

const MAX_DEPTH: usize = 400;

impl HtmlWalker {
    fn block_break(&mut self) {
        if !self.in_cell {
            self.b.paragraph_break();
        }
        self.pending_space = false;
    }

    fn text(&mut self, s: &str) {
        if self.pre {
            self.b.push(s);
            return;
        }
        let mut word = String::new();
        let flush = |this: &mut Self, word: &mut String| {
            if !word.is_empty() {
                if this.pending_space && !this.b.at_line_start() && !this.b.ends_with_space() {
                    this.b.push(" ");
                }
                this.pending_space = false;
                this.b.push(word);
                word.clear();
            }
        };
        for c in s.chars() {
            if c.is_whitespace() && c != '\u{a0}' {
                flush(self, &mut word);
                self.pending_space = true;
            } else {
                word.push(c);
            }
        }
        flush(self, &mut word);
    }

    /// Writes a waiting space now, so a format run that starts next does not begin with it.
    fn settle_space(&mut self) {
        if self.pending_space && !self.b.at_line_start() && !self.b.ends_with_space() {
            self.b.push(" ");
        }
        self.pending_space = false;
    }

    fn walk(&mut self, node: &Handle, depth: usize) {
        if depth > MAX_DEPTH {
            self.too_deep += 1;
            return;
        }
        match &node.data {
            NodeData::Text { contents } => {
                let t = contents.borrow();
                self.text(&t);
            }
            NodeData::Element { name, .. } => {
                let tag = name.local.as_ref();
                self.element(tag, node, depth);
            }
            _ => self.children(node, depth),
        }
    }

    fn children(&mut self, node: &Handle, depth: usize) {
        for child in node.children.borrow().iter() {
            self.walk(child, depth + 1);
        }
    }

    fn element(&mut self, tag: &str, node: &Handle, depth: usize) {
        match tag {
            "script" | "style" | "head" | "noscript" | "template" | "iframe" | "svg" | "canvas" => {}
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.block_break();
                let start = self.b.len();
                self.children(node, depth);
                let level = tag.as_bytes()[1] - b'0';
                self.b.add_run(start, self.b.len(), RunKind::Heading, Some(level));
                self.block_break();
            }
            "li" => {
                self.block_break();
                let start = self.b.len();
                self.children(node, depth);
                self.b.add_run(start, self.b.len(), RunKind::ListItem, None);
                self.block_break();
            }
            "b" | "strong" => {
                self.settle_space();
                let start = self.b.len();
                self.children(node, depth);
                self.b.add_run(start, self.b.len(), RunKind::Bold, None);
            }
            "i" | "em" | "cite" => {
                self.settle_space();
                let start = self.b.len();
                self.children(node, depth);
                self.b.add_run(start, self.b.len(), RunKind::Italic, None);
            }
            "br" => {
                self.b.line_break();
                self.pending_space = false;
            }
            "pre" => {
                self.block_break();
                let was = self.pre;
                self.pre = true;
                self.children(node, depth);
                self.pre = was;
                self.block_break();
            }
            "table" => {
                self.block_break();
                self.children(node, depth);
                self.block_break();
            }
            "tr" => {
                self.b.line_break();
                self.pending_space = false;
                self.walk_cells(node, depth);
            }
            "td" | "th" => {
                // A cell outside a row (broken markup): treat it as a one-cell row.
                self.cell(node, depth, true);
            }
            "img" => {
                self.images += 1;
                if self.pending_space && !self.b.at_line_start() && !self.b.ends_with_space() {
                    self.b.push(" ");
                }
                self.pending_space = false;
                self.b.push("[image]");
            }
            "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "aside" | "nav" | "figure"
            | "figcaption" | "blockquote" | "address" | "ul" | "ol" | "dl" | "dt" | "dd" | "form" | "fieldset"
            | "details" | "summary" | "hr" => {
                self.block_break();
                self.children(node, depth);
                self.block_break();
            }
            _ => self.children(node, depth),
        }
    }

    fn walk_cells(&mut self, row: &Handle, depth: usize) {
        let mut first = true;
        for child in row.children.borrow().iter() {
            if let NodeData::Element { name, .. } = &child.data {
                if matches!(name.local.as_ref(), "td" | "th") {
                    self.cell(child, depth + 1, first);
                    first = false;
                    continue;
                }
            }
            self.walk(child, depth + 1);
        }
    }

    fn cell(&mut self, node: &Handle, depth: usize, first: bool) {
        if !first {
            self.b.push("\t");
        } else {
            self.b.line_break();
        }
        self.pending_space = false;
        let start = self.b.len();
        let was = self.in_cell;
        self.in_cell = true;
        self.children(node, depth);
        self.in_cell = was;
        self.b.add_run(start, self.b.len(), RunKind::TableCell, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> ImportOptions {
        ImportOptions::default()
    }

    fn slice(p: &ParsedText, r: &prismqda_engine::FormatRun) -> String {
        p.text.chars().skip(r.start as usize).take((r.end - r.start) as usize).collect()
    }

    // ---- decoding ----

    #[test]
    fn decoding_handles_boms_utf16_and_falls_back_with_a_warning() {
        assert_eq!(decode(b"\xEF\xBB\xBFhi").0, "hi");
        assert_eq!(decode(b"\xFF\xFEh\0i\0").0, "hi");
        assert_eq!(decode(b"\xFE\xFF\0h\0i").0, "hi");
        let (s, w) = decode(b"caf\xE9");
        assert_eq!(s, "caf\u{e9}");
        assert_eq!(w.unwrap().kind, "assumed_encoding");
        assert!(decode("caf\u{e9}".as_bytes()).1.is_none());
    }

    #[test]
    fn cleaning_unifies_line_endings_and_drops_controls() {
        assert_eq!(clean("a\r\nb\rc\u{0}\u{7}d\u{feff}"), "a\nb\ncd");
        assert_eq!(clean("a\u{c}b"), "a\n\nb");
        assert_eq!(clean("tab\there"), "tab\there");
    }

    // ---- plain text ----

    #[test]
    fn plain_text_keeps_paragraphs_and_wrapped_lines() {
        let p = parse_txt(b"\xEF\xBB\xBF  First line  \r\nsecond line\r\n\r\n\r\nNext paragraph.\r\n\r\n", &opts()).unwrap();
        assert_eq!(p.text, "First line\nsecond line\n\nNext paragraph.");
        assert_eq!(p.source_type, SourceType::Document);
        assert!(p.units.is_none());
    }

    #[test]
    fn plain_text_that_is_only_whitespace_is_empty() {
        assert_eq!(parse_txt(b" \r\n\t\r\n ", &opts()).unwrap().text, "");
    }

    #[test]
    fn plain_text_composes_accents_and_counts_scalar_values() {
        let p = parse_txt("cafe\u{301} \u{1F600}".as_bytes(), &opts()).unwrap();
        assert_eq!(p.text, "caf\u{e9} \u{1F600}");
        assert_eq!(p.text.chars().count(), 6);
    }

    // ---- Markdown ----

    #[test]
    fn markdown_keeps_headings_emphasis_lists_and_tables() {
        let md = "# Title\n\nSome *italic* and **bold** words.\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let p = parse_markdown(md.as_bytes(), &opts()).unwrap();
        assert_eq!(p.text, "Title\n\nSome italic and bold words.\n\none\n\ntwo\n\na\tb\n1\t2");
        let by = |k: RunKind| -> Vec<String> { p.runs.iter().filter(|r| r.kind == k).map(|r| slice(&p, r)).collect() };
        assert_eq!(by(RunKind::Heading), ["Title"]);
        assert_eq!(p.runs.iter().find(|r| r.kind == RunKind::Heading).unwrap().level, Some(1));
        assert_eq!(by(RunKind::Italic), ["italic"]);
        assert_eq!(by(RunKind::Bold), ["bold"]);
        assert_eq!(by(RunKind::ListItem), ["one", "two"]);
        assert_eq!(by(RunKind::TableCell), ["a", "b", "1", "2"]);
    }

    #[test]
    fn markdown_images_and_raw_html_are_reported_not_dropped_silently() {
        let p = parse_markdown(b"Look ![a cat](cat.png) here.\n\n<div>raw</div>\n", &opts()).unwrap();
        assert!(p.text.contains("Look [image] here."), "{}", p.text);
        assert!(!p.text.contains("a cat"), "alt text is not duplicated");
        let kinds: Vec<&str> = p.warnings.iter().map(|w| w.kind.as_str()).collect();
        assert!(kinds.contains(&"skipped_image") && kinds.contains(&"skipped_html"), "{kinds:?}");
    }

    #[test]
    fn markdown_runs_use_scalar_offsets_after_non_ascii_text() {
        let p = parse_markdown("\u{4f60}\u{597d} \u{1F600} **x**".as_bytes(), &opts()).unwrap();
        let bold = p.runs.iter().find(|r| r.kind == RunKind::Bold).unwrap();
        assert_eq!(slice(&p, bold), "x");
        assert_eq!(bold.start, 5);
    }

    // ---- HTML ----

    #[test]
    fn html_becomes_paragraphs_with_runs_and_skips_scripts() {
        let html = "<html><head><title>T</title><style>p{}</style></head><body>\
            <h2>Heading <em>two</em></h2><p>One   two\n three <b>bold</b>.</p><script>alert(1)</script>\
            <ul><li>a</li><li>b</li></ul><p>Line<br>break</p></body></html>";
        let p = parse_html(html.as_bytes(), &opts()).unwrap();
        assert_eq!(p.text, "Heading two\n\nOne two three bold.\n\na\n\nb\n\nLine\nbreak");
        let by = |k: RunKind| -> Vec<String> { p.runs.iter().filter(|r| r.kind == k).map(|r| slice(&p, r)).collect() };
        assert_eq!(by(RunKind::Heading), ["Heading two"]);
        assert_eq!(p.runs.iter().find(|r| r.kind == RunKind::Heading).unwrap().level, Some(2));
        assert_eq!(by(RunKind::Bold), ["bold"]);
        assert_eq!(by(RunKind::Italic), ["two"]);
        assert_eq!(by(RunKind::ListItem), ["a", "b"]);
        // The exact-text check above already proves the script, the style rule, and the page title are gone.
    }

    #[test]
    fn html_tables_use_tabs_and_newlines() {
        let html = "<table><tr><th>Name</th><th>Age</th></tr><tr><td>Ann</td><td>34</td></tr></table>";
        let p = parse_html(html.as_bytes(), &opts()).unwrap();
        assert_eq!(p.text, "Name\tAge\nAnn\t34");
        let cells: Vec<String> = p.runs.iter().filter(|r| r.kind == RunKind::TableCell).map(|r| slice(&p, r)).collect();
        assert_eq!(cells, ["Name", "Age", "Ann", "34"]);
    }

    #[test]
    fn html_preformatted_text_keeps_its_line_breaks() {
        let p = parse_html(b"<pre>line one\n  line two</pre><p>after</p>", &opts()).unwrap();
        assert_eq!(p.text, "line one\n  line two\n\nafter");
    }

    #[test]
    fn html_images_are_placeholders_and_entities_are_decoded() {
        let p = parse_html(b"<p>Fish &amp; chips <img src=x.png> and caf&eacute;</p>", &opts()).unwrap();
        assert_eq!(p.text, "Fish & chips [image] and caf\u{e9}");
        assert!(p.warnings.iter().any(|w| w.kind == "skipped_image"));
    }

    #[test]
    fn broken_and_very_deep_html_does_not_crash() {
        let broken = parse_html(b"<p>unclosed <b>bold <i>both", &opts()).unwrap();
        assert!(broken.text.contains("unclosed"));
        let deep = format!("{}text{}", "<div>".repeat(3000), "</div>".repeat(3000));
        let p = parse_html(deep.as_bytes(), &opts());
        assert!(p.is_ok());
        let nothing = parse_html(b"", &opts()).unwrap();
        assert_eq!(nothing.text, "");
    }
}
