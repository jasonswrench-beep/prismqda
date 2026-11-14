//! Splitting text into analysis units (specification 6.3): paragraphs and sentences.
//!
//! Spans are in Unicode scalar values, matching the rest of the engine. Surrounding whitespace is
//! never part of a unit. Sentence breaks follow Unicode's sentence rules, with a small correction so
//! that "Dr. Smith" and "J. K. Rowling" are not split in half.

use prismqda_engine::{ImportedUnit, UnitKind};
use unicode_segmentation::UnicodeSegmentation;

/// Units for a kind that this module can make on its own: sentences, or paragraphs for everything else.
pub fn units_for(kind: UnitKind, text: &str) -> Vec<ImportedUnit> {
    match kind {
        UnitKind::Sentence => sentences(text),
        other => paragraphs_as(text, other),
    }
}

pub fn paragraphs(text: &str) -> Vec<ImportedUnit> {
    paragraphs_as(text, UnitKind::Paragraph)
}

/// Paragraphs are runs of lines separated by a blank line.
pub fn paragraphs_as(text: &str, kind: UnitKind) -> Vec<ImportedUnit> {
    let mut pos = CharPos::new(text);
    paragraph_ranges(text)
        .into_iter()
        .map(|(b0, b1)| ImportedUnit::new(pos.at(b0), pos.at(b1), kind))
        .collect()
}

pub fn sentences(text: &str) -> Vec<ImportedUnit> {
    let mut pos = CharPos::new(text);
    let mut out = Vec::new();
    for (p0, p1) in paragraph_ranges(text) {
        for (b0, b1) in sentence_ranges(&text[p0..p1], p0) {
            out.push(ImportedUnit::new(pos.at(b0), pos.at(b1), UnitKind::Sentence));
        }
    }
    out
}

/// Byte ranges of paragraphs, trimmed of surrounding whitespace.
fn paragraph_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut last_end = 0;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        if line.trim().is_empty() {
            if let Some(s) = start.take() {
                ranges.push(trim_range(text, s, last_end));
            }
        } else {
            if start.is_none() {
                start = Some(line_start);
            }
            last_end = offset;
        }
    }
    if let Some(s) = start {
        ranges.push(trim_range(text, s, last_end));
    }
    ranges.retain(|(a, b)| b > a);
    ranges
}

fn trim_range(text: &str, start: usize, end: usize) -> (usize, usize) {
    let slice = &text[start..end];
    let lead = slice.len() - slice.trim_start().len();
    let trail = slice.len() - slice.trim_end().len();
    (start + lead, end - trail)
}

/// Words that end in a period without ending a sentence.
const ABBREVIATIONS: &[&str] = &[
    "mr.", "mrs.", "ms.", "dr.", "prof.", "sr.", "jr.", "st.", "vs.", "e.g.", "i.e.", "inc.", "ltd.", "co.", "no.", "fig.",
    "approx.", "dept.",
];

fn ends_with_abbreviation(sentence: &str) -> bool {
    let last = sentence.trim_end().rsplit(char::is_whitespace).next().unwrap_or("");
    let lower = last.to_lowercase();
    if ABBREVIATIONS.contains(&lower.as_str()) {
        return true;
    }
    // A single capital letter and a period is an initial: "J." in "J. K. Rowling".
    let mut chars = last.chars();
    matches!((chars.next(), chars.next(), chars.next()), (Some(c), Some('.'), None) if c.is_uppercase())
}

/// Byte ranges (in the whole text, shifted by `base`) of the sentences inside one paragraph.
fn sentence_ranges(paragraph: &str, base: usize) -> Vec<(usize, usize)> {
    // Unicode ends a sentence at every line break, but a line break inside a paragraph is just a
    // wrapped line. A newline and a space are both one byte, so swapping them keeps every offset.
    let flat = paragraph.replace(['\n', '\r'], " ");
    let paragraph = flat.as_str();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut carry: Option<usize> = None;
    for (offset, piece) in paragraph.split_sentence_bound_indices() {
        let start = carry.unwrap_or(offset);
        let end = offset + piece.len();
        if ends_with_abbreviation(&paragraph[start..end]) && end < paragraph.len() {
            carry = Some(start);
            continue;
        }
        carry = None;
        merged.push((start, end));
    }
    if let Some(start) = carry {
        merged.push((start, paragraph.len()));
    }
    merged
        .into_iter()
        .map(|(s, e)| trim_range(paragraph, s, e))
        .filter(|(s, e)| e > s)
        .map(|(s, e)| (base + s, base + e))
        .collect()
}

/// Turns increasing byte positions into scalar-value positions without rescanning from the start.
struct CharPos<'a> {
    text: &'a str,
    byte: usize,
    chars: i64,
}

impl<'a> CharPos<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, byte: 0, chars: 0 }
    }

    fn at(&mut self, byte: usize) -> i64 {
        debug_assert!(byte >= self.byte, "positions must not go backwards");
        self.chars += self.text[self.byte..byte].chars().count() as i64;
        self.byte = byte;
        self.chars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(text: &str, units: &[ImportedUnit]) -> Vec<String> {
        units
            .iter()
            .map(|u| text.chars().skip(u.start as usize).take((u.end - u.start) as usize).collect())
            .collect()
    }

    #[test]
    fn paragraphs_split_on_blank_lines_and_ignore_extra_space() {
        let text = "\n  First line\nstill first.\n\n\n   Second one.  \n\t\nThird.";
        let u = paragraphs(text);
        assert_eq!(pieces(text, &u), ["First line\nstill first.", "Second one.", "Third."]);
        assert!(u.iter().all(|x| x.kind == UnitKind::Paragraph));
    }

    #[test]
    fn sentences_split_inside_and_across_paragraphs() {
        let text = "It rained. We stayed in!  Why not?\n\nNew paragraph, one sentence.";
        let u = sentences(text);
        assert_eq!(
            pieces(text, &u),
            ["It rained.", "We stayed in!", "Why not?", "New paragraph, one sentence."]
        );
        assert!(u.iter().all(|x| x.kind == UnitKind::Sentence));
    }

    #[test]
    fn a_wrapped_line_does_not_end_a_sentence() {
        let text = "This sentence was wrapped\nby an old text editor and\ncontinues here. A second one follows.";
        let got = pieces(text, &sentences(text));
        assert_eq!(
            got,
            [
                "This sentence was wrapped\nby an old text editor and\ncontinues here.",
                "A second one follows."
            ]
        );
    }

    #[test]
    fn titles_and_initials_do_not_end_a_sentence() {
        let text = "Dr. Smith met J. K. Rowling at St. Mary's. They talked e.g. about books. Then they left.";
        let got = pieces(text, &sentences(text));
        assert_eq!(
            got,
            ["Dr. Smith met J. K. Rowling at St. Mary's.", "They talked e.g. about books.", "Then they left."]
        );
    }

    #[test]
    fn a_trailing_abbreviation_is_still_kept() {
        let text = "He works at Acme Inc.";
        assert_eq!(pieces(text, &sentences(text)), ["He works at Acme Inc."]);
    }

    #[test]
    fn offsets_count_scalar_values_not_bytes() {
        let text = "Caf\u{00e9} \u{1F600} ouvert. Fermé demain.";
        let u = sentences(text);
        assert_eq!(pieces(text, &u), ["Caf\u{00e9} \u{1F600} ouvert.", "Ferm\u{00e9} demain."]);
        assert_eq!(u[0].start, 0);
        assert_eq!(u[0].end, 14);
        assert_eq!(u[1].start, 15);
    }

    #[test]
    fn right_to_left_and_cjk_text_segments_without_panicking() {
        let text = "\u{05e9}\u{05dc}\u{05d5}\u{05dd} \u{05e2}\u{05d5}\u{05dc}\u{05dd}. \u{4f60}\u{597d}\u{3002}\u{4e16}\u{754c}\u{3002}";
        let u = sentences(text);
        assert!(u.len() >= 2, "{u:?}");
        assert!(u.windows(2).all(|w| w[0].end <= w[1].start));
    }

    #[test]
    fn empty_and_blank_text_make_no_units() {
        assert!(sentences("").is_empty());
        assert!(sentences("  \n\n \t ").is_empty());
        assert!(paragraphs("").is_empty());
    }

    #[test]
    fn units_never_overlap_and_stay_in_range() {
        let text = "One. Two three. Four?\n\nFive! Six.\nSeven.";
        let n = text.chars().count() as i64;
        for kind in [UnitKind::Sentence, UnitKind::Paragraph] {
            let u = units_for(kind, text);
            assert!(!u.is_empty());
            assert!(u.iter().all(|x| x.start >= 0 && x.end > x.start && x.end <= n));
            assert!(u.windows(2).all(|w| w[0].end <= w[1].start));
        }
    }

    #[test]
    fn other_kinds_fall_back_to_paragraphs_labeled_with_that_kind() {
        let u = units_for(UnitKind::Comment, "a\n\nb");
        assert_eq!(u.len(), 2);
        assert!(u.iter().all(|x| x.kind == UnitKind::Comment));
    }
}
