//! Builds the text of an imported source while keeping every offset honest.
//!
//! Offsets count Unicode scalar values (code points) of the stored text, and the stored text is in
//! Unicode NFC. So the builder normalizes each piece as it is appended and counts what it actually
//! stored. Pieces always end at a line or paragraph break, and NFC never composes characters across a
//! newline, so normalizing piece by piece gives the same text as normalizing the whole at the end.
//!
//! Layout conventions every importer uses:
//! * paragraphs are separated by one blank line (`"\n\n"`)
//! * a line break inside a paragraph is a single `"\n"`
//! * the cells of a table row are separated by a tab, and rows by `"\n"`

use prismqda_engine::{AttributeSpec, FormatRun, ImportWarning, ImportedUnit, RunKind, SourceType, UnitKind};
use unicode_normalization::UnicodeNormalization;

/// The text and formatting a format-specific importer found, before it is turned into a source.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedText {
    /// Unicode NFC.
    pub text: String,
    pub runs: Vec<FormatRun>,
    /// Units the format itself defines (transcript turns, for example). `None` means: split the text
    /// by `unit_kind` (sentences or paragraphs).
    pub units: Option<Vec<ImportedUnit>>,
    /// Definitions for every attribute named by the units above.
    pub unit_attributes: Vec<AttributeSpec>,
    pub warnings: Vec<ImportWarning>,
    pub source_type: SourceType,
    pub unit_kind: UnitKind,
}

#[derive(Debug, Default)]
pub struct TextBuilder {
    text: String,
    len: i64,
    runs: Vec<FormatRun>,
    warnings: Vec<ImportWarning>,
}

impl TextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of scalar values stored so far: the offset where the next piece will start.
    pub fn len(&self) -> i64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True when nothing is stored yet or the text ends with a line break, so the next text starts a line.
    pub fn at_line_start(&self) -> bool {
        self.text.is_empty() || self.text.ends_with('\n')
    }

    /// True when the stored text ends with a space or tab.
    pub fn ends_with_space(&self) -> bool {
        self.text.ends_with(' ') || self.text.ends_with('\t')
    }

    /// Appends text, normalized to NFC, and returns the span it now occupies.
    pub fn push(&mut self, s: &str) -> (i64, i64) {
        let start = self.len;
        for c in s.nfc() {
            self.text.push(c);
            self.len += 1;
        }
        (start, self.len)
    }

    /// Appends text and records a format run over exactly that text. Empty text records nothing.
    pub fn push_run(&mut self, s: &str, kind: RunKind, level: Option<u8>) -> (i64, i64) {
        let (start, end) = self.push(s);
        if end > start {
            self.runs.push(FormatRun { start, end, kind, level, page: None });
        }
        (start, end)
    }

    /// Records a format run over a span that is already stored.
    pub fn add_run(&mut self, start: i64, end: i64, kind: RunKind, level: Option<u8>) {
        if end > start {
            self.runs.push(FormatRun { start, end, kind, level, page: None });
        }
    }

    /// Records the start of a page (PDF import): the run covers the page's text.
    pub fn add_page(&mut self, start: i64, end: i64, page: u32) {
        if end > start {
            self.runs.push(FormatRun { start, end, kind: RunKind::Page, level: None, page: Some(page) });
        }
    }

    /// A single line break, unless the text is empty or already ends with one.
    pub fn line_break(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
            self.len += 1;
        }
    }

    /// Ensures a blank line separates what comes next from what came before. Does nothing at the start.
    pub fn paragraph_break(&mut self) {
        if self.text.is_empty() {
            return;
        }
        while !self.text.ends_with("\n\n") {
            self.text.push('\n');
            self.len += 1;
        }
    }

    pub fn warn(&mut self, warning: ImportWarning) {
        self.warnings.push(warning);
    }

    /// Finishes: removes trailing whitespace and clips any format run that reached into it.
    pub fn finish(mut self, source_type: SourceType, unit_kind: UnitKind) -> ParsedText {
        let trimmed = self.text.trim_end().len();
        self.text.truncate(trimmed);
        let len = self.text.chars().count() as i64;
        self.runs.retain_mut(|r| {
            r.end = r.end.min(len);
            r.end > r.start
        });
        ParsedText {
            text: self.text,
            runs: self.runs,
            units: None,
            unit_attributes: Vec::new(),
            warnings: self.warnings,
            source_type,
            unit_kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_count_scalar_values_after_normalization() {
        let mut b = TextBuilder::new();
        // "e" + combining acute composes to one character, so the span is one wide.
        assert_eq!(b.push("e\u{0301}"), (0, 1));
        assert_eq!(b.push("a\u{1F600}b"), (1, 4));
        assert_eq!(b.len(), 4);
        let p = b.finish(SourceType::Document, UnitKind::Sentence);
        assert_eq!(p.text, "\u{00e9}a\u{1F600}b");
        assert_eq!(p.text.chars().count(), 4);
    }

    #[test]
    fn breaks_do_not_pile_up_and_do_nothing_at_the_start() {
        let mut b = TextBuilder::new();
        b.paragraph_break();
        b.line_break();
        assert!(b.is_empty());
        b.push("one");
        b.paragraph_break();
        b.paragraph_break();
        b.push("two");
        b.line_break();
        b.line_break();
        b.push("three");
        assert_eq!(b.finish(SourceType::Document, UnitKind::Paragraph).text, "one\n\ntwo\nthree");
    }

    #[test]
    fn runs_cover_exactly_the_text_pushed() {
        let mut b = TextBuilder::new();
        b.push_run("Title", RunKind::Heading, Some(1));
        b.paragraph_break();
        let (s, e) = b.push("plain bold plain");
        b.add_run(s + 6, s + 10, RunKind::Bold, None);
        assert_eq!(e, 23);
        b.push_run("", RunKind::Italic, None);
        let p = b.finish(SourceType::Document, UnitKind::Sentence);
        assert_eq!(p.runs.len(), 2);
        assert_eq!((p.runs[0].start, p.runs[0].end, p.runs[0].level), (0, 5, Some(1)));
        assert_eq!(p.text.chars().skip(p.runs[1].start as usize).take(4).collect::<String>(), "bold");
    }

    #[test]
    fn finish_trims_trailing_space_and_clips_runs() {
        let mut b = TextBuilder::new();
        b.push("text");
        b.push_run("\n\n", RunKind::Bold, None);
        let p = b.finish(SourceType::Document, UnitKind::Sentence);
        assert_eq!(p.text, "text");
        assert!(p.runs.is_empty(), "a run that was only whitespace disappears");
    }

    #[test]
    fn pages_are_recorded_with_their_number() {
        let mut b = TextBuilder::new();
        let (s, e) = b.push("first page text");
        b.add_page(s, e, 1);
        let p = b.finish(SourceType::Pdf, UnitKind::Sentence);
        assert_eq!((p.runs[0].kind, p.runs[0].page), (RunKind::Page, Some(1)));
    }
}
