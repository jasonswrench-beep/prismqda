//! Searching the text (specification 12.1): exact phrases, Boolean operators, wildcards, regular
//! expressions, and proximity, with every hit shown in context.
//!
//! The query language:
//! * `cost` finds the word, ignoring case. `"first day"` finds a phrase. `welcom*` and `wom?n` use wildcards.
//! * `/regex/` is a regular expression (case-insensitive unless it says otherwise).
//! * `AND`, `OR`, and `NOT` (capitals) combine terms; two terms side by side mean AND; parentheses group.
//! * `cost NEAR/5 access` finds the two within 5 words of each other (`NEAR` alone means 10).
//!
//! Boolean operators are decided inside one **scope**: a unit when the source has units, otherwise a
//! paragraph. So `cost AND access` finds scopes that hold both, not texts that hold both somewhere.

use regex::{Regex, RegexBuilder};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::{Error, Result};

/// A parsed query that can be run over many texts.
#[derive(Debug)]
pub struct Query {
    root: Node,
    /// Every term's pattern; `Node::Term(i)` refers to `terms[i]`.
    terms: Vec<Regex>,
    uses_near: bool,
}

#[derive(Debug)]
enum Node {
    Term(usize),
    And(Vec<Node>),
    Or(Vec<Node>),
    Not(Box<Node>),
    Near(Box<Node>, Box<Node>, usize),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    LParen,
    RParen,
    And,
    Or,
    Not,
    Near(usize),
    Word(String),
    Phrase(String),
    Regex(String),
}

const MAX_TERMS: usize = 40;

fn lex(q: &str) -> Result<Vec<Tok>> {
    let chars: Vec<char> = q.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '"' => {
                let start = i + 1;
                let end = chars[start..].iter().position(|c| *c == '"').ok_or_else(|| Error::Invalid("a quotation mark is not closed".to_string()))?;
                toks.push(Tok::Phrase(chars[start..start + end].iter().collect()));
                i = start + end + 1;
            }
            '/' => {
                let mut j = i + 1;
                let mut body = String::new();
                loop {
                    match chars.get(j) {
                        None => return Err(Error::Invalid("a regular expression is not closed with a second /".to_string())),
                        Some('\\') if chars.get(j + 1) == Some(&'/') => {
                            body.push('/');
                            j += 2;
                        }
                        Some('/') => break,
                        Some(ch) => {
                            body.push(*ch);
                            j += 1;
                        }
                    }
                }
                toks.push(Tok::Regex(body));
                i = j + 1;
            }
            _ => {
                let start = i;
                while i < chars.len() && !chars[i].is_whitespace() && !matches!(chars[i], '(' | ')' | '"') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                toks.push(match word.as_str() {
                    "AND" => Tok::And,
                    "OR" => Tok::Or,
                    "NOT" => Tok::Not,
                    "NEAR" => Tok::Near(10),
                    w if w.starts_with("NEAR/") => {
                        let n: usize = w[5..].parse().map_err(|_| Error::Invalid(format!("'{w}' should be NEAR/ and a number, such as NEAR/5")))?;
                        Tok::Near(n.clamp(0, 1000))
                    }
                    _ => Tok::Word(word),
                });
            }
        }
    }
    Ok(toks)
}

/// A word with `*` (any letters) and `?` (one letter) becomes a pattern that must match whole words.
fn word_pattern(word: &str) -> String {
    let mut p = String::from(r"(?i)\b");
    for c in word.chars() {
        match c {
            '*' => p.push_str(r"\w*"),
            '?' => p.push_str(r"\w"),
            c => p.push_str(&regex::escape(&c.to_string())),
        }
    }
    p.push_str(r"\b");
    p
}

fn phrase_pattern(phrase: &str) -> String {
    let words: Vec<String> = phrase.split_whitespace().map(|w| word_pattern(w).replace(r"(?i)\b", "").trim_end_matches(r"\b").to_string()).collect();
    format!(r"(?i)\b{}\b", words.join(r"\s+"))
}

fn compile(pattern: &str) -> Result<Regex> {
    RegexBuilder::new(pattern)
        .size_limit(2_000_000)
        .build()
        .map_err(|e| Error::Invalid(format!("that search could not be understood: {}", e.to_string().lines().last().unwrap_or("bad pattern").trim())))
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    regexes: Vec<Regex>,
    uses_near: bool,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn or(&mut self) -> Result<Node> {
        let mut parts = vec![self.and()?];
        while self.peek() == Some(&Tok::Or) {
            self.pos += 1;
            parts.push(self.and()?);
        }
        Ok(if parts.len() == 1 { parts.pop().expect("one part") } else { Node::Or(parts) })
    }

    fn and(&mut self) -> Result<Node> {
        let mut parts = vec![self.unary()?];
        loop {
            match self.peek() {
                Some(Tok::And) => self.pos += 1,
                Some(Tok::Word(_)) | Some(Tok::Phrase(_)) | Some(Tok::Regex(_)) | Some(Tok::LParen) | Some(Tok::Not) => {}
                _ => break,
            }
            parts.push(self.unary()?);
        }
        Ok(if parts.len() == 1 { parts.pop().expect("one part") } else { Node::And(parts) })
    }

    fn unary(&mut self) -> Result<Node> {
        if self.peek() == Some(&Tok::Not) {
            self.pos += 1;
            return Ok(Node::Not(Box::new(self.unary()?)));
        }
        self.near()
    }

    fn near(&mut self) -> Result<Node> {
        let mut left = self.primary()?;
        while let Some(Tok::Near(n)) = self.peek().cloned() {
            self.pos += 1;
            self.uses_near = true;
            let right = self.primary()?;
            left = Node::Near(Box::new(left), Box::new(right), n);
        }
        Ok(left)
    }

    fn primary(&mut self) -> Result<Node> {
        let tok = self.toks.get(self.pos).cloned();
        self.pos += 1;
        match tok {
            Some(Tok::LParen) => {
                let inner = self.or()?;
                if self.peek() != Some(&Tok::RParen) {
                    return Err(Error::Invalid("a parenthesis is not closed".to_string()));
                }
                self.pos += 1;
                Ok(inner)
            }
            Some(Tok::Word(w)) => self.term(&word_pattern(&w)),
            Some(Tok::Phrase(p)) => {
                if p.split_whitespace().next().is_none() {
                    return Err(Error::Invalid("a quoted phrase is empty".to_string()));
                }
                self.term(&phrase_pattern(&p))
            }
            Some(Tok::Regex(r)) => {
                if r.is_empty() {
                    return Err(Error::Invalid("a regular expression is empty".to_string()));
                }
                self.term(&format!("(?i){r}"))
            }
            Some(other) => Err(Error::Invalid(format!("a search term was expected here, but found {}", describe(&other)))),
            None => Err(Error::Invalid("the search ends where a term was expected".to_string())),
        }
    }

    fn term(&mut self, pattern: &str) -> Result<Node> {
        if self.regexes.len() >= MAX_TERMS {
            return Err(Error::Invalid(format!("a search can have at most {MAX_TERMS} terms")));
        }
        self.regexes.push(compile(pattern)?);
        Ok(Node::Term(self.regexes.len() - 1))
    }
}

fn describe(t: &Tok) -> &'static str {
    match t {
        Tok::LParen => "(",
        Tok::RParen => ")",
        Tok::And => "AND",
        Tok::Or => "OR",
        Tok::Not => "NOT",
        Tok::Near(_) => "NEAR",
        _ => "a term",
    }
}

impl Query {
    pub fn parse(q: &str) -> Result<Query> {
        let toks = lex(q)?;
        if toks.is_empty() {
            return Err(Error::Invalid("type something to search for".to_string()));
        }
        let mut p = Parser { toks, pos: 0, regexes: Vec::new(), uses_near: false };
        let root = p.or()?;
        if p.pos < p.toks.len() {
            return Err(Error::Invalid(format!("the search has {} where it should end", describe(&p.toks[p.pos]))));
        }
        Ok(Query { root, terms: p.regexes, uses_near: p.uses_near })
    }

    /// Finds every term once in a whole text. Deciding which units satisfy the query then only
    /// looks up those matches, so a text with thousands of units is scanned once, not thousands of times.
    pub fn scan(&self, text: &str) -> Scan {
        Scan {
            matches: self.terms.iter().map(|re| re.find_iter(text).map(|m| (m.start(), m.end())).collect()).collect(),
            words: if self.uses_near { word_starts(text) } else { Vec::new() },
        }
    }

    /// Decides whether the part of the scanned text from byte `from` to byte `to` satisfies the query.
    /// Returns the byte spans (in the whole text) of what matched, or `None`.
    pub fn eval_in(&self, scan: &Scan, from: usize, to: usize) -> Option<Vec<(usize, usize)>> {
        let (ok, mut spans) = eval(&self.root, scan, from, to);
        if !ok {
            return None;
        }
        spans.sort_unstable();
        spans.dedup();
        Some(spans)
    }

    /// Runs the query over one piece of text. Returns the byte spans of what matched, or `None` if the
    /// text does not satisfy the query.
    pub fn find_in(&self, text: &str) -> Option<Vec<(usize, usize)>> {
        let scan = self.scan(text);
        self.eval_in(&scan, 0, text.len())
    }
}

/// Where every term of a query matched in a whole text.
pub struct Scan {
    matches: Vec<Vec<(usize, usize)>>,
    words: Vec<usize>,
}

/// Byte offsets where each word of the text starts, for measuring distance in words.
fn word_starts(text: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut in_word = false;
    for (i, c) in text.char_indices() {
        let w = c.is_alphanumeric();
        if w && !in_word {
            starts.push(i);
        }
        in_word = w;
    }
    starts
}

fn word_index(words: &[usize], byte: usize) -> usize {
    words.partition_point(|s| *s <= byte)
}

type Spans = Vec<(usize, usize)>;

fn eval(node: &Node, scan: &Scan, from: usize, to: usize) -> (bool, Spans) {
    match node {
        Node::Term(i) => {
            // Matches are in text order and never overlap, so the ones inside the range are a slice.
            let m = &scan.matches[*i];
            let lo = m.partition_point(|(s, _)| *s < from);
            let hi = m.partition_point(|(_, e)| *e <= to);
            let spans: Spans = if lo < hi { m[lo..hi].to_vec() } else { Vec::new() };
            (!spans.is_empty(), spans)
        }
        Node::And(parts) => {
            let mut all = Vec::new();
            for p in parts {
                let (ok, s) = eval(p, scan, from, to);
                if !ok {
                    return (false, Vec::new());
                }
                all.extend(s);
            }
            (true, all)
        }
        Node::Or(parts) => {
            let mut all = Vec::new();
            let mut any = false;
            for p in parts {
                let (ok, s) = eval(p, scan, from, to);
                any |= ok;
                all.extend(s);
            }
            (any, all)
        }
        Node::Not(inner) => (!eval(inner, scan, from, to).0, Vec::new()),
        Node::Near(a, b, n) => {
            let (ok_a, sa) = eval(a, scan, from, to);
            let (ok_b, sb) = eval(b, scan, from, to);
            if !ok_a || !ok_b {
                return (false, Vec::new());
            }
            let mut spans = Vec::new();
            for x in &sa {
                for y in &sb {
                    let (ix, iy) = (word_index(&scan.words, x.0), word_index(&scan.words, y.0));
                    if ix.abs_diff(iy) <= *n && x != y {
                        spans.push(*x);
                        spans.push(*y);
                    }
                }
            }
            (!spans.is_empty(), spans)
        }
    }
}

/// How to narrow a search.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchFilter {
    /// Only these sources.
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    /// Only sources that belong to this case.
    #[serde(default)]
    pub case_id: Option<i64>,
    /// Only scopes that already carry this code.
    #[serde(default)]
    pub with_code: Option<i64>,
    /// Only scopes that do not carry this code yet.
    #[serde(default)]
    pub without_code: Option<i64>,
    /// Stop collecting after this many hits (the total is still counted).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// One scope (a unit or a paragraph) that satisfied the query.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchHit {
    pub source_id: i64,
    pub source_name: String,
    pub unit_id: Option<i64>,
    pub scope_start: i64,
    pub scope_end: i64,
    /// What matched inside the scope, as character offsets into the source.
    pub spans: Vec<(i64, i64)>,
    /// The scope's text, shortened around the first match.
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    /// How many scopes satisfied the query, counting those past the limit.
    pub total: usize,
    pub truncated: bool,
}

/// Paragraph ranges (byte offsets), trimmed, for sources that have no units.
fn paragraph_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut last_end = 0;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        if line.trim().is_empty() {
            if let Some(s) = start.take() {
                out.push((s, last_end));
            }
        } else {
            if start.is_none() {
                start = Some(line_start);
            }
            last_end = line_start + line.trim_end().len();
        }
    }
    if let Some(s) = start {
        out.push((s, last_end));
    }
    out
}

fn snippet(text: &str, first_match: Option<(usize, usize)>) -> String {
    const WIDTH: usize = 160;
    let total = text.chars().count();
    if total <= WIDTH {
        return text.to_string();
    }
    let at = first_match.map(|(s, _)| text[..s].chars().count()).unwrap_or(0);
    let from = at.saturating_sub(WIDTH / 3);
    let piece: String = text.chars().skip(from).take(WIDTH).collect();
    format!("{}{}{}", if from > 0 { "…" } else { "" }, piece, if from + WIDTH < total { "…" } else { "" })
}

impl Project {
    /// Searches the text of the project's sources. See the module documentation for the query language.
    pub fn search(&self, query: &str, filter: &SearchFilter) -> Result<SearchResults> {
        let q = Query::parse(query)?;
        let limit = filter.limit.unwrap_or(1000);

        let mut out = SearchResults { hits: Vec::new(), total: 0, truncated: false };
        for (id, name) in self.scoped_sources(filter)? {
            let text: String = self.conn.query_row("SELECT text FROM source WHERE id = ?1", [id], |r| r.get(0))?;
            let scopes = self.scopes_of(id, &text)?;
            let coded = if filter.with_code.is_some() || filter.without_code.is_some() { self.codings_by_code(id)? } else { Default::default() };

            let scan = q.scan(&text);
            let mut counter = CharCounter::new(&text);
            for (unit_id, b0, b1) in scopes {
                let scope_text = &text[b0..b1];
                let Some(spans) = q.eval_in(&scan, b0, b1) else { continue };
                let (s0, s1) = (counter.at(b0), counter.at(b1));
                if let Some(c) = filter.with_code {
                    if !overlaps(&coded, c, s0, s1) {
                        continue;
                    }
                }
                if let Some(c) = filter.without_code {
                    if overlaps(&coded, c, s0, s1) {
                        continue;
                    }
                }
                out.total += 1;
                if out.hits.len() >= limit {
                    out.truncated = true;
                    continue;
                }
                let abs: Vec<(i64, i64)> = spans.iter().map(|(a, b)| (counter.at(*a), counter.at(*b))).collect();
                out.hits.push(SearchHit {
                    source_id: id,
                    source_name: name.clone(),
                    unit_id,
                    scope_start: s0,
                    scope_end: s1,
                    spans: abs,
                    snippet: snippet(scope_text, spans.first().map(|(a, b)| (a - b0, b - b0))),
                });
            }
        }
        Ok(out)
    }

    /// The sources a search or an autocoding run looks in: every source in the order the document list shows them,
    /// narrowed to the chosen ones and to the members of a case. Only the id and name are read: selecting the text
    /// here would copy every source into the sorter.
    pub(crate) fn scoped_sources(&self, filter: &SearchFilter) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM source ORDER BY folder, id")?;
        let sources: Vec<(i64, String)> = {
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let in_case: Option<std::collections::HashSet<i64>> = match filter.case_id {
            Some(case) => {
                let mut c = self.conn.prepare(
                    "SELECT source_id FROM case_member WHERE case_id = ?1 AND source_id IS NOT NULL
                     UNION SELECT u.source_id FROM case_member m JOIN unit u ON u.id = m.unit_id WHERE m.case_id = ?1",
                )?;
                let ids: std::collections::HashSet<i64> = c.query_map([case], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
                Some(ids)
            }
            None => None,
        };
        Ok(sources
            .into_iter()
            .filter(|(id, _)| !(filter.source_ids.as_ref().is_some_and(|ids| !ids.contains(id)) || in_case.as_ref().is_some_and(|set| !set.contains(id))))
            .collect())
    }

    /// The pieces of a source a query is decided within: its units, or its paragraphs. Byte ranges.
    pub(crate) fn scopes_of(&self, source_id: i64, text: &str) -> Result<Vec<(Option<i64>, usize, usize)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, start_offset, end_offset FROM unit WHERE source_id = ?1 AND excluded = 0 ORDER BY start_offset")?;
        let units: Vec<(i64, i64, i64)> =
            stmt.query_map(params![source_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<_, _>>()?;
        if units.is_empty() {
            return Ok(paragraph_ranges(text).into_iter().map(|(a, b)| (None, a, b)).collect());
        }
        let mut cursor = CharCounter::new(text);
        let mut out = Vec::with_capacity(units.len());
        for (id, s, e) in units {
            let (b0, b1) = (cursor.byte_of(s), cursor.byte_of(e));
            if b1 > b0 {
                out.push((Some(id), b0, b1));
            }
        }
        Ok(out)
    }

    fn codings_by_code(&self, source_id: i64) -> Result<std::collections::HashMap<i64, Vec<(i64, i64)>>> {
        let mut map: std::collections::HashMap<i64, Vec<(i64, i64)>> = std::collections::HashMap::new();
        let mut stmt = self.conn.prepare("SELECT code_id, start_offset, end_offset FROM coding WHERE source_id = ?1 AND layer = 'coder'")?;
        for row in stmt.query_map([source_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))? {
            let (c, s, e) = row?;
            map.entry(c).or_default().push((s, e));
        }
        Ok(map)
    }
}

fn overlaps(coded: &std::collections::HashMap<i64, Vec<(i64, i64)>>, code: i64, s: i64, e: i64) -> bool {
    coded.get(&code).is_some_and(|spans| spans.iter().any(|(a, b)| *a < e && *b > s))
}

/// Converts between byte positions and character positions in either direction, by remembering the
/// last position and walking from there, so asking in nearly increasing order is cheap.
pub(crate) struct CharCounter<'a> {
    text: &'a str,
    byte: usize,
    chars: i64,
}

impl<'a> CharCounter<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self { text, byte: 0, chars: 0 }
    }

    /// The character position of a byte position.
    pub(crate) fn at(&mut self, byte: usize) -> i64 {
        if byte >= self.byte {
            self.chars += self.text[self.byte..byte].chars().count() as i64;
        } else {
            self.chars -= self.text[byte..self.byte].chars().count() as i64;
        }
        self.byte = byte;
        self.chars
    }

    /// The byte position of a character position (clamped to the end of the text).
    pub(crate) fn byte_of(&mut self, target: i64) -> usize {
        if target >= self.chars {
            let mut need = target - self.chars;
            for (i, c) in self.text[self.byte..].char_indices() {
                if need == 0 {
                    self.byte += i;
                    self.chars = target;
                    return self.byte;
                }
                need -= 1;
                let _ = c;
            }
            self.chars += (target - self.chars) - need;
            self.byte = self.text.len();
        } else {
            let mut need = self.chars - target;
            let mut byte = self.byte;
            for (i, _) in self.text[..self.byte].char_indices().rev() {
                byte = i;
                need -= 1;
                if need == 0 {
                    break;
                }
            }
            self.byte = byte;
            self.chars = target;
        }
        self.byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(q: &str, text: &str) -> Option<Vec<String>> {
        Query::parse(q).unwrap().find_in(text).map(|spans| spans.iter().map(|(a, b)| text[*a..*b].to_string()).collect())
    }

    fn matches(q: &str, text: &str) -> bool {
        found(q, text).is_some()
    }

    #[test]
    fn words_ignore_case_and_need_whole_words() {
        assert_eq!(found("cost", "The COST was high; cost again."), Some(vec!["COST".into(), "cost".into()]));
        assert!(!matches("cost", "costly and accost"), "only whole words");
        assert!(matches("café", "A Café, please"));
    }

    #[test]
    fn phrases_match_across_any_spacing() {
        assert!(matches("\"first day\"", "on the first   day of school"));
        assert!(matches("\"first day\"", "on the first\nday"));
        assert!(!matches("\"first day\"", "the first and then the day"));
    }

    #[test]
    fn wildcards_stand_for_letters() {
        assert_eq!(found("welcom*", "welcome, welcomed, unwelcome"), Some(vec!["welcome".into(), "welcomed".into()]));
        assert!(matches("wom?n", "the women"));
        assert!(!matches("wom?n", "the womn"));
        assert!(matches("*ing", "walking home"));
    }

    #[test]
    fn regular_expressions_are_passed_through() {
        assert_eq!(found(r"/\b\d{3}-\d{4}\b/", "call 555-0199 now"), Some(vec!["555-0199".into()]));
        assert!(matches("/a\\/b/", "path a/b here"));
        assert!(Query::parse("/(/").is_err());
        assert!(Query::parse("/never closed").is_err());
    }

    #[test]
    fn boolean_operators_decide_inside_the_text_given() {
        let t = "The cost was high but access was easy.";
        assert!(matches("cost AND access", t));
        assert!(matches("cost access", t), "side by side means AND");
        assert!(!matches("cost AND parking", t));
        assert!(matches("cost OR parking", t));
        assert!(matches("cost NOT parking", t));
        assert!(!matches("cost NOT access", t));
        assert!(matches("NOT parking", t));
        assert!(matches("(cost OR parking) AND access", t));
        assert!(!matches("(parking OR lot) AND access", t));
    }

    #[test]
    fn not_leaves_no_spans_but_the_rest_still_show() {
        assert_eq!(found("cost NOT parking", "The cost was high."), Some(vec!["cost".into()]));
    }

    #[test]
    fn near_measures_distance_in_words() {
        let t = "The cost of getting access to care was high.";
        assert!(matches("cost NEAR/4 access", t));
        assert!(!matches("cost NEAR/2 access", t));
        assert!(matches("access NEAR/4 cost", t), "order does not matter");
        assert!(matches("cost NEAR access", t), "NEAR alone means within 10");
        assert!(!matches("cost NEAR/3 parking", t));
        assert_eq!(found("cost NEAR/4 access", t), Some(vec!["cost".into(), "access".into()]), "spans come back in text order");
    }

    #[test]
    fn mistakes_in_a_query_are_explained() {
        for (q, fragment) in [
            ("", "type something"),
            ("\"open phrase", "not closed"),
            ("(cost", "parenthesis"),
            ("cost)", "where it should end"),
            ("AND cost", "found AND"),
            ("cost AND", "ends where a term"),
            ("cost NEAR/x access", "number"),
            ("\"  \"", "empty"),
        ] {
            let err = Query::parse(q).unwrap_err().to_string();
            assert!(err.contains(fragment), "query {q:?} gave {err:?}, expected it to mention {fragment:?}");
        }
        let many = (0..45).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        assert!(Query::parse(&many).unwrap_err().to_string().contains("at most"));
    }

    #[test]
    fn paragraphs_are_found_and_trimmed() {
        let t = "\n  one two\nstill one\n\n\nthree  \n";
        let ranges = paragraph_ranges(t);
        let got: Vec<&str> = ranges.iter().map(|(a, b)| &t[*a..*b]).collect();
        assert_eq!(got, ["  one two\nstill one", "three"]);
    }

    #[test]
    fn the_snippet_stays_near_the_match() {
        let long = format!("{} needle {}", "a".repeat(400), "b".repeat(400));
        let at = long.find("needle");
        let s = snippet(&long, at.map(|a| (a, a + 6)));
        assert!(s.contains("needle") && s.starts_with('…') && s.ends_with('…'), "{s}");
        assert!(s.chars().count() <= 162);
        assert_eq!(snippet("short", None), "short");
    }

    #[test]
    fn byte_positions_can_be_found_from_character_positions() {
        let t = "a\u{1F600}b\u{4f60}c";
        let mut c = CharCounter::new(t);
        assert_eq!(c.byte_of(0), 0);
        assert_eq!(c.byte_of(2), 5);
        assert_eq!(c.byte_of(4), 9);
        assert_eq!(c.byte_of(1), 1, "backwards");
        assert_eq!(c.byte_of(5), t.len());
        assert_eq!(c.byte_of(99), t.len(), "past the end clamps");
        assert_eq!(c.byte_of(3), 6, "backwards after clamping");
    }

    #[test]
    fn character_counting_moves_both_ways() {
        let t = "a\u{1F600}b\u{4f60}c";
        let mut c = CharCounter::new(t);
        let end = t.len();
        assert_eq!(c.at(end), 5);
        assert_eq!(c.at(1), 1);
        assert_eq!(c.at(5), 2, "a, the emoji, then b starts at byte 5");
        assert_eq!(c.at(0), 0);
    }
}
