//! Reference library import: RIS, BibTeX, and CSL JSON.
//!
//! Every reference becomes one source (its title, details, abstract, and keywords as text that can be coded) and one case
//! whose variables hold the same details (authors, year, where it was published, type, DOI, keywords), so a literature
//! review can be coded and then compared by year or kind of publication.

use prismqda_engine::{AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportWarning, ImportedCase, ImportedSource, Level, RunKind, SourceType, UnitKind};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::builder::TextBuilder;
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;
use crate::segment::units_for;
use crate::text::{clean, decode};

/// One author as shown ("Maria Hernandez") and the part used to name the reference ("Hernandez").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub display: String,
    pub family: String,
}

/// What every format is turned into before it becomes a source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reference {
    /// The kind of work in words: "Journal article", "Book", and so on.
    pub kind: String,
    pub authors: Vec<Author>,
    pub title: String,
    /// Four digits, or empty when the file gave no year.
    pub year: String,
    /// The journal, book, or series it appeared in.
    pub container: String,
    pub volume: String,
    pub issue: String,
    pub pages: String,
    pub publisher: String,
    pub doi: String,
    pub url: String,
    pub abstract_text: String,
    pub keywords: Vec<String>,
}

// ---------------------------------------------------------------- shared helpers

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Hernandez, Maria" and "Maria Hernandez" are the same person; shown the second way.
fn author_from(raw: &str) -> Option<Author> {
    let raw = squash(raw);
    if raw.is_empty() {
        return None;
    }
    if let Some((family, given)) = raw.split_once(',') {
        let (family, given) = (family.trim(), given.trim());
        // "Jr." and similar after a second comma stay with the given names.
        let display = if given.is_empty() { family.to_string() } else { format!("{given} {family}") };
        return Some(Author { display, family: family.to_string() });
    }
    let family = raw.rsplit(' ').next().unwrap_or(&raw).to_string();
    Some(Author { display: raw, family })
}

fn organization(raw: &str) -> Author {
    let name = squash(raw);
    Author { display: name.clone(), family: name }
}

/// Four digits from 1000 to 2999 in a date text such as "2021/03/15/" or "March 2021".
fn year_of(s: &str) -> String {
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let window: String = bytes[i..i + 4].iter().collect();
        let before_ok = i == 0 || !bytes[i - 1].is_ascii_digit();
        let after_ok = i + 4 == bytes.len() || !bytes[i + 4].is_ascii_digit();
        if before_ok && after_ok && window.chars().all(|c| c.is_ascii_digit()) && (window.starts_with('1') || window.starts_with('2')) {
            return window;
        }
        i += 1;
    }
    String::new()
}

fn clean_doi(s: &str) -> String {
    let s = s.trim();
    for prefix in ["https://doi.org/", "http://doi.org/", "https://dx.doi.org/", "http://dx.doi.org/", "doi:", "DOI:"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    s.to_string()
}

fn split_keywords(s: &str) -> Vec<String> {
    s.split([';', ',']).map(squash).filter(|k| !k.is_empty()).collect()
}

fn first_non_empty(values: &[&str]) -> String {
    values.iter().find(|v| !v.trim().is_empty()).map(|v| squash(v)).unwrap_or_default()
}

// ---------------------------------------------------------------- RIS

fn ris_kind(code: &str) -> String {
    match code {
        "JOUR" | "JFULL" | "EJOUR" => "Journal article",
        "BOOK" | "EBOOK" | "WHOLE" => "Book",
        "CHAP" | "ECHAP" => "Book chapter",
        "CONF" | "CPAPER" => "Conference paper",
        "THES" => "Thesis",
        "RPRT" => "Report",
        "ELEC" | "WEB" | "BLOG" => "Web page",
        "NEWS" | "MGZN" => "News or magazine article",
        "GEN" | "" => "Reference",
        other => other,
    }
    .to_string()
}

/// Splits RIS text into records of (tag, value). A record ends at `ER`; a line with no tag continues the value before it.
fn ris_records(text: &str) -> Vec<Vec<(String, String)>> {
    let mut records: Vec<Vec<(String, String)>> = Vec::new();
    let mut current: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        let chars: Vec<char> = line.chars().collect();
        let tagged = chars.len() >= 5
            && chars[0].is_ascii_uppercase()
            && (chars[1].is_ascii_uppercase() || chars[1].is_ascii_digit())
            && chars[2] == ' '
            && chars[3] == ' '
            && chars[4] == '-'
            && (chars.len() == 5 || chars[5] == ' ');
        if tagged {
            let tag: String = chars[0..2].iter().collect();
            let value: String = chars.iter().skip(6).collect();
            if tag == "ER" {
                if !current.is_empty() {
                    records.push(std::mem::take(&mut current));
                }
            } else {
                current.push((tag, value.trim().to_string()));
            }
        } else if !line.trim().is_empty() {
            if let Some(last) = current.last_mut() {
                if !last.1.is_empty() {
                    last.1.push(' ');
                }
                last.1.push_str(line.trim());
            }
        }
    }
    if !current.is_empty() {
        records.push(current);
    }
    records
}

fn ris_reference(record: &[(String, String)]) -> Reference {
    let all = |tags: &[&str]| -> Vec<&str> { record.iter().filter(|(t, _)| tags.contains(&t.as_str())).map(|(_, v)| v.as_str()).collect() };
    let one = |tags: &[&str]| -> String { first_non_empty(&all(tags)) };
    let pages = {
        let (start, end) = (one(&["SP"]), one(&["EP"]));
        match (start.is_empty(), end.is_empty()) {
            (false, false) => format!("{start}\u{2013}{end}"),
            (false, true) => start,
            (true, false) => end,
            (true, true) => String::new(),
        }
    };
    let year_source = [one(&["PY"]), one(&["Y1"]), one(&["DA"])].into_iter().find(|s| !year_of(s).is_empty()).unwrap_or_default();
    Reference {
        kind: ris_kind(&one(&["TY"])),
        authors: all(&["AU", "A1"]).into_iter().filter_map(author_from).collect(),
        title: one(&["TI", "T1"]),
        year: year_of(&year_source),
        container: one(&["JF", "JO", "T2", "BT", "J2", "JA"]),
        volume: one(&["VL"]),
        issue: one(&["IS"]),
        pages,
        publisher: one(&["PB"]),
        doi: clean_doi(&one(&["DO"])),
        url: one(&["UR"]),
        abstract_text: all(&["AB", "N2"]).into_iter().map(|s| s.trim().to_string()).find(|s| !s.is_empty()).unwrap_or_default(),
        keywords: all(&["KW"]).into_iter().flat_map(split_keywords_ris).collect(),
    }
}

/// In RIS each KW line is one keyword, but some tools put several in one line separated by semicolons.
fn split_keywords_ris(s: &str) -> Vec<String> {
    if s.contains(';') {
        split_keywords(s)
    } else {
        let k = squash(s);
        if k.is_empty() { Vec::new() } else { vec![k] }
    }
}

/// Reads a RIS file.
pub fn parse_ris(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let (text, warning) = decode(bytes);
    let text = clean(&text);
    let refs: Vec<Reference> = ris_records(&text).iter().map(|r| ris_reference(r)).collect();
    finish(refs, "ris", warning, opts)
}

// ---------------------------------------------------------------- BibTeX

/// One entry as written: its type, its citation key, and its fields (names in lower case, values not yet cleaned).
#[derive(Debug, Clone, PartialEq)]
struct BibEntry {
    kind: String,
    fields: Vec<(String, String)>,
}

struct Scanner<'a> {
    s: &'a [char],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }
    /// Reads `{...}` with nested braces, returning what is inside. The scanner is on the opening brace.
    fn braced(&mut self) -> String {
        let mut depth = 0usize;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '{' => {
                    depth += 1;
                    if depth > 1 {
                        out.push(c);
                    }
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return out;
                    }
                    out.push(c);
                }
                '\\' => {
                    out.push(c);
                    if let Some(n) = self.peek() {
                        out.push(n);
                        self.i += 1;
                    }
                }
                _ => out.push(c),
            }
        }
        out
    }
    /// Reads `"..."`; braces inside protect quotes. The scanner is on the opening quote.
    fn quoted(&mut self) -> String {
        self.i += 1;
        let mut depth = 0usize;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '{' => {
                    depth += 1;
                    out.push(c);
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    out.push(c);
                }
                '"' if depth == 0 => return out,
                '\\' => {
                    out.push(c);
                    if let Some(n) = self.peek() {
                        out.push(n);
                        self.i += 1;
                    }
                }
                _ => out.push(c),
            }
        }
        out
    }
    fn word(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.' | '+' | '/' | '\'') {
                out.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        out
    }
}

fn bib_entries(text: &str) -> Vec<BibEntry> {
    let chars: Vec<char> = text.chars().collect();
    let mut sc = Scanner { s: &chars, i: 0 };
    let mut macros: Vec<(String, String)> = Vec::new();
    let mut entries = Vec::new();
    while sc.i < chars.len() {
        if sc.peek() != Some('@') {
            sc.i += 1;
            continue;
        }
        sc.i += 1;
        sc.skip_space();
        let kind = sc.word().to_lowercase();
        sc.skip_space();
        let open = match sc.peek() {
            Some(c @ ('{' | '(')) => c,
            _ => continue,
        };
        if matches!(kind.as_str(), "comment" | "preamble") {
            if open == '{' {
                sc.braced();
            } else {
                while sc.peek().is_some_and(|c| c != ')') {
                    sc.i += 1;
                }
            }
            continue;
        }
        sc.i += 1; // the opening brace or parenthesis
        let close = if open == '{' { '}' } else { ')' };
        let mut fields: Vec<(String, String)> = Vec::new();
        if kind != "string" {
            sc.skip_space();
            let key = {
                let mut k = String::new();
                while let Some(c) = sc.peek() {
                    if c == ',' || c == close {
                        break;
                    }
                    k.push(c);
                    sc.i += 1;
                }
                squash(&k)
            };
            fields.push(("bibtex-key".to_string(), key));
        }
        loop {
            sc.skip_space();
            while sc.peek() == Some(',') {
                sc.i += 1;
                sc.skip_space();
            }
            match sc.peek() {
                None => break,
                Some(c) if c == close => {
                    sc.i += 1;
                    break;
                }
                _ => {}
            }
            let name = sc.word().to_lowercase();
            if name.is_empty() {
                sc.i += 1;
                continue;
            }
            sc.skip_space();
            if sc.peek() != Some('=') {
                continue;
            }
            sc.i += 1;
            let mut value = String::new();
            loop {
                sc.skip_space();
                match sc.peek() {
                    Some('{') => value.push_str(&sc.braced()),
                    Some('"') => value.push_str(&sc.quoted()),
                    Some(c) if c.is_alphanumeric() || c == '_' => {
                        let w = sc.word();
                        // A bare word is a number, or the name of a @string macro.
                        let known = macros.iter().find(|(n, _)| n.eq_ignore_ascii_case(&w)).map(|(_, v)| v.clone());
                        value.push_str(&known.unwrap_or(w));
                    }
                    _ => break,
                }
                sc.skip_space();
                if sc.peek() == Some('#') {
                    sc.i += 1;
                } else {
                    break;
                }
            }
            fields.push((name, value));
        }
        if kind == "string" {
            macros.extend(fields.into_iter().filter(|(n, _)| n != "bibtex-key"));
        } else {
            entries.push(BibEntry { kind, fields });
        }
    }
    entries
}

/// Turns the LaTeX in a BibTeX value into plain text: accents, special letters, escaped symbols, and protective braces.
fn latex_to_text(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let accent = |c: char| -> Option<char> {
        Some(match c {
            '`' => '\u{0300}',
            '\'' => '\u{0301}',
            '^' => '\u{0302}',
            '"' => '\u{0308}',
            '~' => '\u{0303}',
            '=' => '\u{0304}',
            '.' => '\u{0307}',
            _ => return None,
        })
    };
    let letter_accent = |c: char| -> Option<char> {
        Some(match c {
            'c' => '\u{0327}',
            'v' => '\u{030C}',
            'u' => '\u{0306}',
            'H' => '\u{030B}',
            'k' => '\u{0328}',
            'r' => '\u{030A}',
            _ => return None,
        })
    };
    // The letter an accent applies to: the next character, or the one inside braces.
    let take_target = |i: &mut usize| -> Option<char> {
        while *i < chars.len() && chars[*i] == ' ' {
            *i += 1;
        }
        if *i < chars.len() && chars[*i] == '{' {
            *i += 1;
            let mut inner = String::new();
            let mut depth = 1;
            while *i < chars.len() {
                match chars[*i] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            *i += 1;
                            break;
                        }
                    }
                    ch => inner.push(ch),
                }
                *i += 1;
            }
            let inner = inner.trim().trim_start_matches('\\');
            // "\i" and "\j" are the dotless letters the accent sits on.
            return match inner {
                "i" => Some('i'),
                "j" => Some('j'),
                other => other.chars().next(),
            };
        }
        if *i < chars.len() {
            let c = chars[*i];
            *i += 1;
            if c == '\\' && *i < chars.len() && matches!(chars[*i], 'i' | 'j') {
                let l = chars[*i];
                *i += 1;
                return Some(l);
            }
            return Some(c);
        }
        None
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' | '}' => i += 1,
            '~' => {
                out.push(' ');
                i += 1;
            }
            '\\' => {
                i += 1;
                let Some(&n) = chars.get(i) else { break };
                if let Some(mark) = accent(n) {
                    i += 1;
                    if let Some(t) = take_target(&mut i) {
                        out.push(t);
                        out.push(mark);
                    }
                } else if n.is_ascii_alphabetic() {
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let name: String = chars[start..i].iter().collect();
                    match name.as_str() {
                        "ss" => out.push('ß'),
                        "o" => out.push('ø'),
                        "O" => out.push('Ø'),
                        "ae" => out.push('æ'),
                        "AE" => out.push('Æ'),
                        "oe" => out.push('œ'),
                        "OE" => out.push('Œ'),
                        "aa" => out.push('å'),
                        "AA" => out.push('Å'),
                        "l" => out.push('ł'),
                        "L" => out.push('Ł'),
                        "i" => out.push('ı'),
                        "textendash" => out.push('\u{2013}'),
                        "textemdash" => out.push('\u{2014}'),
                        "and" => out.push('&'),
                        _ if name.len() == 1 && letter_accent(name.chars().next().unwrap()).is_some() => {
                            let mark = letter_accent(name.chars().next().unwrap()).unwrap();
                            if let Some(t) = take_target(&mut i) {
                                out.push(t);
                                out.push(mark);
                            }
                        }
                        // Any other command is dropped; what it was applied to stays.
                        _ => {}
                    }
                    // A command name ends at the first non-letter; one space after it belongs to the command.
                    if i < chars.len() && chars[i] == ' ' && matches!(name.as_str(), "ss" | "o" | "O" | "ae" | "AE" | "oe" | "OE" | "aa" | "AA" | "l" | "L" | "i") {
                        i += 1;
                    }
                } else {
                    // An escaped symbol: \& \% \_ \# \$ and so on.
                    out.push(n);
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    squash(&out.nfc().collect::<String>())
}

/// Splits an author list on the word "and" that stands outside braces.
fn bib_authors(raw: &str) -> Vec<Author> {
    let chars: Vec<char> = raw.chars().collect();
    let mut names: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
        }
        let at_and = depth == 0
            && c.is_whitespace()
            && chars.get(i + 1..i + 4).is_some_and(|w| w.iter().collect::<String>().eq_ignore_ascii_case("and"))
            && chars.get(i + 4).is_some_and(|w| w.is_whitespace());
        if at_and {
            names.push(std::mem::take(&mut current));
            i += 4;
        } else {
            current.push(c);
        }
        i += 1;
    }
    names.push(current);
    names
        .into_iter()
        .filter_map(|n| {
            let n = n.trim();
            if n.is_empty() || n.eq_ignore_ascii_case("others") {
                return None;
            }
            // A whole name in braces is an organization, not "Family, Given".
            if n.starts_with('{') && n.ends_with('}') && !n[1..n.len() - 1].contains(['{', '}']) {
                return Some(organization(&latex_to_text(n)));
            }
            author_from(&latex_to_text(n))
        })
        .collect()
}

fn bib_kind(kind: &str) -> String {
    match kind {
        "article" => "Journal article",
        "book" | "booklet" | "proceedings" => "Book",
        "inbook" | "incollection" => "Book chapter",
        "inproceedings" | "conference" => "Conference paper",
        "phdthesis" | "mastersthesis" | "thesis" => "Thesis",
        "techreport" | "report" => "Report",
        "online" | "electronic" | "www" => "Web page",
        "misc" | "unpublished" | "" => "Reference",
        other => other,
    }
    .to_string()
}

fn bib_reference(entry: &BibEntry) -> Reference {
    let get = |name: &str| -> String {
        entry.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone()).unwrap_or_default()
    };
    let text = |name: &str| latex_to_text(&get(name));
    let date = get("year");
    let date = if year_of(&date).is_empty() { get("date") } else { date };
    Reference {
        kind: bib_kind(&entry.kind),
        authors: bib_authors(&get("author")),
        title: text("title"),
        year: year_of(&date),
        container: first_non_empty(&[&text("journal"), &text("journaltitle"), &text("booktitle"), &text("series")]),
        volume: text("volume"),
        issue: first_non_empty(&[&text("number"), &text("issue")]),
        pages: text("pages").replace("--", "\u{2013}").replace('-', "\u{2013}"),
        publisher: first_non_empty(&[&text("publisher"), &text("institution"), &text("school")]),
        doi: clean_doi(&text("doi")),
        url: get("url").trim().to_string(),
        abstract_text: latex_to_text(&get("abstract")),
        keywords: split_keywords(&text("keywords")),
    }
}

/// Reads a BibTeX file.
pub fn parse_bibtex(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let (text, warning) = decode(bytes);
    let text = clean(&text);
    let refs: Vec<Reference> = bib_entries(&text).iter().map(bib_reference).collect();
    finish(refs, "bibtex", warning, opts)
}

// ---------------------------------------------------------------- CSL JSON

/// True when a JSON value is a list of CSL items (what reference managers export as "CSL JSON"): each has a `type`
/// and a title or authors, and none is a Reddit listing (those carry a `kind` and `data`).
pub fn looks_like_csl_json(value: &Value) -> bool {
    let items: Vec<&Value> = match value {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) if o.get("items").is_some_and(Value::is_array) && o.get("type").is_some() => vec![value],
        Value::Object(_) => vec![value],
        _ => return false,
    };
    !items.is_empty()
        && items.iter().all(|v| {
            v.is_object() && v.get("type").is_some_and(Value::is_string) && (v.get("title").is_some() || v.get("author").is_some()) && v.get("kind").is_none() && v.get("data").is_none()
        })
}

fn csl_text(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => squash(s),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn csl_kind(t: &str) -> String {
    match t {
        "article-journal" | "article" => "Journal article",
        "book" => "Book",
        "chapter" => "Book chapter",
        "paper-conference" => "Conference paper",
        "thesis" => "Thesis",
        "report" => "Report",
        "webpage" | "post-weblog" => "Web page",
        "article-newspaper" | "article-magazine" => "News or magazine article",
        "" | "document" | "manuscript" => "Reference",
        other => other,
    }
    .to_string()
}

pub(crate) fn csl_reference(v: &Value) -> Reference {
    let authors = v
        .get("author")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    if let Some(lit) = a.get("literal").and_then(Value::as_str) {
                        return Some(organization(lit));
                    }
                    let family = a.get("family").and_then(Value::as_str).map(squash).unwrap_or_default();
                    let given = a.get("given").and_then(Value::as_str).map(squash).unwrap_or_default();
                    if family.is_empty() && given.is_empty() {
                        return None;
                    }
                    let display = if given.is_empty() { family.clone() } else if family.is_empty() { given.clone() } else { format!("{given} {family}") };
                    Some(Author { display, family: if family.is_empty() { given } else { family } })
                })
                .collect()
        })
        .unwrap_or_default();
    let year = v
        .get("issued")
        .and_then(|d| d.get("date-parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get(0))
        .and_then(|y| y.as_i64().map(|n| n.to_string()).or_else(|| y.as_str().map(str::to_string)))
        .map(|y| year_of(&y))
        .filter(|y| !y.is_empty())
        .or_else(|| v.get("issued").and_then(|d| d.get("raw")).and_then(Value::as_str).map(year_of))
        .unwrap_or_default();
    Reference {
        kind: csl_kind(&csl_text(v, "type")),
        authors,
        title: csl_text(v, "title"),
        year,
        container: first_non_empty(&[&csl_text(v, "container-title"), &csl_text(v, "collection-title")]),
        volume: csl_text(v, "volume"),
        issue: csl_text(v, "issue"),
        pages: csl_text(v, "page").replace('-', "\u{2013}"),
        publisher: csl_text(v, "publisher"),
        doi: clean_doi(&csl_text(v, "DOI")),
        url: csl_text(v, "URL"),
        abstract_text: v.get("abstract").and_then(Value::as_str).map(|s| s.trim().to_string()).unwrap_or_default(),
        keywords: split_keywords(&csl_text(v, "keyword")),
    }
}

/// Reads a CSL JSON file (a list of items, or one item).
pub fn parse_csl_json(value: &Value, opts: &ImportOptions) -> Result<ImportBatch> {
    let refs: Vec<Reference> = match value {
        Value::Array(list) => list.iter().map(csl_reference).collect(),
        one => vec![csl_reference(one)],
    };
    finish(refs, "csl-json", None, opts)
}

// ---------------------------------------------------------------- from references to a batch

const MAX_LABEL_TITLE: usize = 70;

/// "Hernandez & Li 2021 - Bus reliability and daily stress": how a reference is named in Documents and Cases.
pub fn label(r: &Reference, number: usize) -> String {
    let who = match r.authors.as_slice() {
        [] => String::new(),
        [a] => a.family.clone(),
        [a, b] => format!("{} & {}", a.family, b.family),
        [a, ..] => format!("{} et al.", a.family),
    };
    let year = if r.year.is_empty() { if who.is_empty() { String::new() } else { "n.d.".to_string() } } else { r.year.clone() };
    let mut title: String = r.title.chars().take(MAX_LABEL_TITLE).collect();
    if r.title.chars().count() > MAX_LABEL_TITLE {
        title.push('\u{2026}');
    }
    let head = [who, year].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    match (head.is_empty(), title.is_empty()) {
        (true, true) => format!("Reference {number}"),
        (true, false) => title,
        (false, true) => head,
        (false, false) => format!("{head} \u{2013} {title}"),
    }
}

/// Writes a reference's title, details, abstract, and keywords into a text, and says whether it has an abstract. The exchange file of the
/// Reference Manager goes on from here with the excerpts of the source.
pub(crate) fn write_reference(b: &mut TextBuilder, r: &Reference) -> bool {
    if !r.title.is_empty() {
        b.push_run(&r.title, RunKind::Heading, Some(1));
        b.paragraph_break();
    }
    let mut lines: Vec<String> = Vec::new();
    if !r.authors.is_empty() {
        lines.push(format!("Authors: {}", r.authors.iter().map(|a| a.display.as_str()).collect::<Vec<_>>().join("; ")));
    }
    if !r.year.is_empty() {
        lines.push(format!("Year: {}", r.year));
    }
    if !r.container.is_empty() {
        let mut place = r.container.clone();
        let detail = [
            (!r.volume.is_empty()).then(|| format!("volume {}", r.volume)),
            (!r.issue.is_empty()).then(|| format!("issue {}", r.issue)),
            (!r.pages.is_empty()).then(|| format!("pages {}", r.pages)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if !detail.is_empty() {
            place.push_str(&format!(" ({})", detail.join(", ")));
        }
        lines.push(format!("Published in: {place}"));
    }
    if !r.publisher.is_empty() {
        lines.push(format!("Publisher: {}", r.publisher));
    }
    lines.push(format!("Type: {}", r.kind));
    if !r.doi.is_empty() {
        lines.push(format!("DOI: {}", r.doi));
    }
    if !r.url.is_empty() {
        lines.push(format!("Link: {}", r.url));
    }
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            b.line_break();
        }
        b.push(line);
    }
    let has_abstract = !r.abstract_text.is_empty();
    if has_abstract {
        b.paragraph_break();
        b.push_run("Abstract", RunKind::Heading, Some(2));
        b.paragraph_break();
        b.push(&clean(&r.abstract_text));
    }
    if !r.keywords.is_empty() {
        b.paragraph_break();
        b.push(&format!("Keywords: {}", r.keywords.join("; ")));
    }
    has_abstract
}

fn reference_text(r: &Reference) -> (crate::builder::ParsedText, bool) {
    let mut b = TextBuilder::new();
    let has_abstract = write_reference(&mut b, r);
    (b.finish(SourceType::Document, UnitKind::Paragraph), has_abstract)
}

/// Turns references into a batch: a source and a case for each, and the variables the cases carry.
fn finish(refs: Vec<Reference>, format: &str, warning: Option<ImportWarning>, opts: &ImportOptions) -> Result<ImportBatch> {
    if refs.is_empty() {
        return Err(ImportError::format("the file has no references in it"));
    }
    let folder = if opts.folder.is_empty() { "References".to_string() } else { opts.folder.clone() };
    let mut batch = ImportBatch::default();
    let mut specs: Vec<AttributeSpec> = Vec::new();
    let mut want = |name: &str, level: Level| {
        if !specs.iter().any(|s| s.name == name) {
            specs.push(AttributeSpec::new(name, AppliesTo::Case, level));
        }
    };
    let mut used = 0usize;
    for (n, r) in refs.iter().enumerate() {
        let (parsed, has_abstract) = reference_text(r);
        if parsed.text.trim().is_empty() {
            continue;
        }
        used += 1;
        let kind = opts.unit_kind.unwrap_or(UnitKind::Paragraph);
        let name = label(r, n + 1);
        let mut warnings = Vec::new();
        if let Some(w) = &warning {
            warnings.push(w.clone());
        }
        if !has_abstract {
            warnings.push(ImportWarning::new("no_abstract", "This reference has no abstract, so there is little text to code. Its details are kept as variables of its case."));
        }
        let units = units_for(kind, &parsed.text);
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
            settings: serde_json::json!({ "format": format, "reference": n + 1 }),
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
        batch.cases.push(ImportedCase { name, attrs, members: vec![CaseMember { source: batch.sources.len() - 1, unit: None }] });
    }
    if used == 0 {
        return Err(ImportError::format("none of the references in the file has a title, authors, or an abstract to import"));
    }
    batch.case_attributes = specs;
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authors_are_shown_given_name_first() {
        let a = author_from("Hernandez, Maria").unwrap();
        assert_eq!((a.display.as_str(), a.family.as_str()), ("Maria Hernandez", "Hernandez"));
        let b = author_from("Maria Elena Hernandez").unwrap();
        assert_eq!((b.display.as_str(), b.family.as_str()), ("Maria Elena Hernandez", "Hernandez"));
        assert!(author_from("   ").is_none());
    }

    #[test]
    fn a_year_is_found_in_a_date_and_not_in_other_numbers() {
        assert_eq!(year_of("2021/03/15/"), "2021");
        assert_eq!(year_of("March 2021"), "2021");
        assert_eq!(year_of("12345"), "");
        assert_eq!(year_of("pp. 1-20"), "");
    }

    #[test]
    fn latex_accents_become_letters() {
        assert_eq!(latex_to_text(r#"M{\"u}ller and Gar\c{c}ia, {\'E}milie \ss{} {\o}"#), "M\u{fc}ller and Gar\u{e7}ia, \u{c9}milie \u{df} \u{f8}");
        assert_eq!(latex_to_text(r"R\&D \% {Protected Title}"), "R&D % Protected Title");
    }
}
