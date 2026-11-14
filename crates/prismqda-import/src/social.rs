//! Social media and comment exports (specification 21.2).
//!
//! The model is one source per container (a video, a thread, or one value of the container column of a
//! table). Every post or comment is a unit of kind Comment (a thread's opening post is a Post), and
//! the comments are written into the source's text one after another, a blank line apart, in
//! depth-first thread order: a parent always comes before its replies, and siblings keep the order
//! they had in the file. A unit's span covers exactly its own text, so coding one comment never
//! touches another.
//!
//! Each unit carries what the export knew about it as unit attributes: `author`, `depth` (0 for a
//! top-level comment), `timestamp`, `likes`, `external_id`, and, only when the person mapped a column
//! for it, `author_role`. Each container also becomes a case, because comparing threads or videos is
//! the usual next step (specification 7.2).
//!
//! Three entry points read the three profiles: [`parse_youtube_json`] (the yt-dlp layout and the
//! YouTube Data API v3 layout), [`parse_reddit_json`] (the two listings of a thread's `.json`), and
//! [`from_table`] (one row per comment, with columns the person mapped).

use std::collections::HashMap;

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportWarning, ImportedCase, ImportedSource, ImportedUnit,
    Level, SourceType, UnitKind,
};
use serde_json::{json, Map, Value};

use crate::builder::TextBuilder;
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;
use crate::table::{Cell, Table};

/// Which columns of a generic comment table mean what. Author role is user-mapped, never guessed.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SocialMapping {
    /// The comment's own ID. Needed when replies are mapped; may be left empty otherwise.
    pub id_column: String,
    pub text_column: String,
    /// The ID of the comment this one replies to. Empty cells mean a top-level comment.
    pub parent_column: Option<String>,
    pub author_column: Option<String>,
    pub author_role_column: Option<String>,
    pub timestamp_column: Option<String>,
    /// Rows with the same value here belong to one source (one video, one thread). Without it the whole
    /// table is one source.
    pub container_column: Option<String>,
    /// A number of likes, upvotes, or reactions, kept as the `likes` attribute.
    #[serde(default)]
    pub likes_column: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// What every profile reduces to
// ---------------------------------------------------------------------------------------------

/// When a comment was written, as far as the export says.
#[derive(Debug, Clone, PartialEq)]
enum When {
    Unknown,
    /// An exact time, as ISO 8601 text.
    Exact(String),
    /// A time that the export itself calculated from relative text such as "2 years ago": an ISO 8601
    /// estimate and the text it came from.
    Estimated(String, String),
    /// Only relative text, or text that is not a date.
    Relative(String),
}

#[derive(Debug, Clone)]
struct Comment {
    /// How replies refer to this comment. `None` means nothing can reply to it.
    id: Option<String>,
    /// The comment this one replies to. `None` means top level.
    parent: Option<String>,
    text: String,
    author: Option<String>,
    role: Option<String>,
    when: When,
    likes: Option<String>,
    kind: UnitKind,
}

impl Comment {
    fn new(text: String) -> Self {
        Comment {
            id: None,
            parent: None,
            text,
            author: None,
            role: None,
            when: When::Unknown,
            likes: None,
            kind: UnitKind::Comment,
        }
    }
}

/// A value for a case attribute made from a container.
struct CaseValue {
    name: &'static str,
    level: Level,
    value: String,
}

/// One video, thread, or group of table rows: it becomes one source and one case.
struct Container {
    id: Option<String>,
    title: Option<String>,
    comments: Vec<Comment>,
    case_values: Vec<CaseValue>,
    /// Warnings found while reading, before the thread was put in order.
    warnings: Vec<ImportWarning>,
    /// Profile-specific facts for the source's settings.
    settings: Map<String, Value>,
}

impl Container {
    fn new(comments: Vec<Comment>) -> Self {
        Container {
            id: None,
            title: None,
            comments,
            case_values: Vec::new(),
            warnings: Vec::new(),
            settings: Map::new(),
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn skipped_message(n: usize, place: &str) -> String {
    if n == 1 {
        format!("1 entry in {place} was not a comment and was skipped")
    } else {
        format!("{n} entries in {place} were not comments and were skipped")
    }
}

// ---------------------------------------------------------------------------------------------
// Putting a thread in order
// ---------------------------------------------------------------------------------------------

/// Where one comment goes in the source's text and what it replies to (as positions in the output).
#[derive(Debug, PartialEq, Eq)]
struct Slot {
    /// Index into the container's comments.
    comment: usize,
    /// Index into the output of the comment this replies to.
    parent: Option<usize>,
    depth: u32,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Ordered {
    slots: Vec<Slot>,
    /// Replies whose parent is not in the file; they became top-level comments.
    orphans: usize,
    /// Loops of replies that were cut open.
    cycles: usize,
    /// Comments whose ID an earlier comment already had; replies go to the first one.
    duplicate_ids: usize,
    /// Comments with no text, which were left out.
    empty: usize,
}

/// Depth-first order: parents before children, siblings in their original order. Comments with no text
/// are left out and their replies move up to the nearest comment above that stays.
fn thread_order(comments: &[Comment]) -> Ordered {
    let n = comments.len();
    let mut out = Ordered::default();
    let mut index_of: HashMap<&str, usize> = HashMap::new();
    for (i, c) in comments.iter().enumerate() {
        if let Some(id) = &c.id {
            if index_of.contains_key(id.as_str()) {
                out.duplicate_ids += 1;
            } else {
                index_of.insert(id, i);
            }
        }
    }
    // `link[i]` is the comment that comment `i` replies to, when that comment is in the file.
    let mut link: Vec<Option<usize>> = vec![None; n];
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut roots: Vec<usize> = Vec::new();
    for (i, c) in comments.iter().enumerate() {
        match c.parent.as_deref() {
            None => roots.push(i),
            Some(p) => match index_of.get(p) {
                None => {
                    out.orphans += 1;
                    roots.push(i);
                }
                // A comment that replies to itself.
                Some(&parent) if parent == i => {
                    out.cycles += 1;
                    roots.push(i);
                }
                Some(&parent) => {
                    link[i] = Some(parent);
                    children[parent].push(i);
                }
            },
        }
    }

    let walk = |start: usize, out: &mut Ordered, visited: &mut [bool]| {
        // (comment, output position of the nearest kept ancestor, depth)
        let mut stack: Vec<(usize, Option<usize>, u32)> = vec![(start, None, 0)];
        while let Some((i, parent, depth)) = stack.pop() {
            if visited[i] {
                continue;
            }
            visited[i] = true;
            let (next_parent, next_depth) = if comments[i].text.is_empty() {
                out.empty += 1;
                (parent, depth)
            } else {
                out.slots.push(Slot { comment: i, parent, depth });
                (Some(out.slots.len() - 1), depth + 1)
            };
            for &child in children[i].iter().rev() {
                stack.push((child, next_parent, next_depth));
            }
        }
    };
    let mut visited = vec![false; n];
    for &root in &roots {
        walk(root, &mut out, &mut visited);
    }
    // Anything not reached lies on a loop of replies or hangs off one. Follow the replies-to links from
    // it until a comment repeats: that comment is on the loop, and the loop is cut open there.
    for i in 0..n {
        if visited[i] {
            continue;
        }
        let mut seen = std::collections::HashSet::new();
        let mut at = i;
        while seen.insert(at) {
            match link[at] {
                Some(parent) => at = parent,
                None => break,
            }
        }
        out.cycles += 1;
        walk(at, &mut out, &mut visited);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Cleaning text
// ---------------------------------------------------------------------------------------------

/// Comment text as it is stored: plain line breaks, no control characters, no surrounding space.
fn clean_text(s: &str) -> String {
    let unified = s.replace("\r\n", "\n").replace('\r', "\n");
    let kept: String = unified.chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).collect();
    kept.trim().to_string()
}

/// Replaces character references (`&amp;`, `&#39;`, `&#x1F600;`). With `all_named` false only the three
/// that web services always escape are replaced.
fn decode_entities(s: &str, all_named: bool) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|end| *end <= 10).and_then(|end| {
            let name = &rest[1..end];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" if all_named => Some('"'),
                "apos" if all_named => Some('\''),
                "nbsp" if all_named => Some(' '),
                _ => name.strip_prefix('#').and_then(|num| {
                    let code = match num.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => num.parse::<u32>().ok(),
                    };
                    code.and_then(char::from_u32)
                }),
            };
            ch.map(|c| (c, end + 1))
        });
        match decoded {
            Some((c, used)) => {
                out.push(c);
                rest = &rest[used..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text of a small piece of HTML such as the YouTube API's `textDisplay`: line breaks become new
/// lines, other tags are removed (a link keeps its words), and character references are replaced.
fn html_to_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let looks_like_tag = tail[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '/');
        match tail.find('>').filter(|end| looks_like_tag && *end < 2000) {
            Some(end) => {
                let name: String =
                    tail[1..end].trim_start_matches('/').chars().take_while(char::is_ascii_alphanumeric).collect();
                if name.eq_ignore_ascii_case("br") {
                    out.push('\n');
                }
                rest = &tail[end + 1..];
            }
            None => {
                out.push('<');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    decode_entities(&out, true)
}

// ---------------------------------------------------------------------------------------------
// Times
// ---------------------------------------------------------------------------------------------

/// Days since 1970-01-01 to a calendar date (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month as u32, day as u32)
}

/// Unix seconds (or milliseconds, when the number is too big to be seconds) as UTC ISO 8601 text.
/// `strict` only accepts the years 2000 to 2100, for a column of numbers that may not be times at all.
fn iso_from_unix(value: f64, strict: bool) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let seconds = if value.abs() >= 1e11 { value / 1000.0 } else { value };
    let (low, high) = if strict { (946_684_800.0, 4_102_444_800.0) } else { (1e8, 1e11) };
    if !(low..high).contains(&seconds) {
        return None;
    }
    let whole = seconds.floor() as i64;
    let (year, month, day) = civil_from_days(whole.div_euclid(86_400));
    let rest = whole.rem_euclid(86_400);
    Some(format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60))
}

/// Accepts `YYYY-MM-DD`, optionally followed by `T` (or a space) and `HH:MM`, `:SS`, a fraction, and a
/// zone (`Z`, `+HH:MM`, `-HHMM`). Returns it with a `T` separator, otherwise as written.
fn normalize_iso(s: &str) -> Option<String> {
    let s = s.trim();
    let b = s.as_bytes();
    let digits = |from: usize, to: usize| b.get(from..to).is_some_and(|d| d.iter().all(u8::is_ascii_digit));
    if !(b.len() >= 10 && digits(0, 4) && b[4] == b'-' && digits(5, 7) && b[7] == b'-' && digits(8, 10)) {
        return None;
    }
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if b.len() == 10 {
        return Some(s.to_string());
    }
    if !(b[10] == b'T' || b[10] == b' ') || !(digits(11, 13) && b.get(13) == Some(&b':') && digits(14, 16)) {
        return None;
    }
    let (hour, minute): (u32, u32) = (s[11..13].parse().ok()?, s[14..16].parse().ok()?);
    if hour > 23 || minute > 59 {
        return None;
    }
    let mut i = 16;
    if b.get(i) == Some(&b':') {
        if !digits(i + 1, i + 3) || s[i + 1..i + 3].parse::<u32>().ok()? > 60 {
            return None;
        }
        i += 3;
        if b.get(i) == Some(&b'.') || b.get(i) == Some(&b',') {
            let fraction = b[i + 1..].iter().take_while(|d| d.is_ascii_digit()).count();
            if fraction == 0 {
                return None;
            }
            i += 1 + fraction;
        }
    }
    let zone = &s[i..];
    if !(zone.is_empty() || (zone.is_ascii() && zone_is_valid(zone))) {
        return None;
    }
    Some(format!("{}T{}", &s[..10], &s[11..]))
}

/// `Z`, `+HH:MM`, `-HH:MM`, `+HHMM`, or `-HHMM`. The text is ASCII.
fn zone_is_valid(zone: &str) -> bool {
    let b = zone.as_bytes();
    let signed = matches!(b[0], b'+' | b'-');
    let all_digits = |part: &[u8]| part.iter().all(u8::is_ascii_digit);
    zone == "Z"
        || (signed && b.len() == 6 && b[3] == b':' && all_digits(&b[1..3]) && all_digits(&b[4..]))
        || (signed && b.len() == 5 && all_digits(&b[1..]))
}

/// The time in a JSON value: a unix time, an ISO 8601 string, or text.
fn when_from_json(v: Option<&Value>) -> When {
    match v {
        Some(Value::Number(n)) => n.as_f64().and_then(|x| iso_from_unix(x, false)).map_or(When::Unknown, When::Exact),
        Some(Value::String(s)) => when_from_text(s),
        _ => When::Unknown,
    }
}

fn when_from_text(s: &str) -> When {
    let s = s.trim();
    if s.is_empty() {
        When::Unknown
    } else if let Some(iso) = normalize_iso(s) {
        When::Exact(iso)
    } else if s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse::<f64>()
            .ok()
            .and_then(|x| iso_from_unix(x, false))
            .map_or_else(|| When::Relative(s.to_string()), When::Exact)
    } else {
        When::Relative(s.to_string())
    }
}

// ---------------------------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------------------------

fn read_json(bytes: &[u8]) -> Result<Value> {
    // Windows editors often save JSON with a byte order mark in front.
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    serde_json::from_slice(bytes).map_err(|e| {
        if e.to_string().contains("recursion limit") {
            ImportError::format(
                "this file is nested more deeply than PrismQDA can read (more than about 25 levels of replies); \
                 export the thread with fewer reply levels",
            )
        } else {
            ImportError::format(format!("the file is not valid JSON: {e}"))
        }
    })
}

/// A string, or a number written as text, with surrounding space removed. Empty means absent.
fn string_of(v: Option<&Value>) -> Option<String> {
    let s = match v? {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

/// A count such as likes: whole numbers print without a decimal point.
fn count_of(v: Option<&Value>) -> Option<String> {
    let x = match v? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().replace(',', "").parse::<f64>().ok()?,
        _ => return None,
    };
    number_text(x)
}

fn number_text(x: f64) -> Option<String> {
    if !x.is_finite() {
        None
    } else if x.fract() == 0.0 && x.abs() < 1e15 {
        Some(format!("{}", x as i64))
    } else {
        Some(x.to_string())
    }
}

// ---------------------------------------------------------------------------------------------
// Containers to a batch
// ---------------------------------------------------------------------------------------------

/// What a profile tells the common builder.
struct Profile<'a> {
    /// Names the layout in the source's settings: `youtube_ytdlp`, `youtube_api`, `reddit`, `table`.
    name: &'a str,
    /// The source's name when the container has no title and the person gave none.
    fallback_name: &'a str,
    /// The author role was mapped by the person, so it is defined as an attribute.
    has_role: bool,
    /// Settings that apply to every source made.
    settings: Map<String, Value>,
    /// Warnings about the whole file; they go to the first source.
    warnings: Vec<ImportWarning>,
}

fn build_batch(containers: Vec<Container>, profile: Profile, opts: &ImportOptions) -> Result<ImportBatch> {
    // One level for `timestamp` across the whole batch, so every source defines it the same way.
    let timestamp_level = if containers.iter().flat_map(|c| &c.comments).any(|c| matches!(c.when, When::Relative(_))) {
        Level::Text
    } else {
        Level::Date
    };
    let container_count = containers.len();

    let mut batch = ImportBatch::default();
    let mut leftover: Vec<ImportWarning> = profile.warnings;
    let mut case_specs: Vec<AttributeSpec> = Vec::new();

    for container in containers {
        let order = thread_order(&container.comments);
        let mut warnings = container.warnings;
        if order.slots.is_empty() {
            let name =
                container.title.clone().or(container.id.clone()).unwrap_or_else(|| profile.fallback_name.to_string());
            leftover.push(ImportWarning::new(
                "no_text",
                format!("'{name}' has no comments with any text, so it was left out"),
            ));
            continue;
        }

        let mut builder = TextBuilder::new();
        let mut units: Vec<ImportedUnit> = Vec::with_capacity(order.slots.len());
        let mut used = Used::default();
        let mut estimated = 0usize;
        let mut relative = 0usize;
        let mut example: Option<String> = None;
        for slot in &order.slots {
            let c = &container.comments[slot.comment];
            builder.paragraph_break();
            let (start, end) = builder.push(&c.text);
            let mut unit = ImportedUnit::new(start, end, c.kind);
            unit.parent = slot.parent;
            unit.speaker = c.author.clone();
            if let Some(a) = &c.author {
                unit.attrs.push(("author".to_string(), a.clone()));
                used.author = true;
            }
            unit.attrs.push(("depth".to_string(), slot.depth.to_string()));
            match &c.when {
                When::Unknown => {}
                When::Exact(iso) => {
                    unit.attrs.push(("timestamp".to_string(), iso.clone()));
                    used.timestamp = true;
                }
                When::Estimated(iso, text) => {
                    estimated += 1;
                    example.get_or_insert_with(|| text.clone());
                    unit.attrs.push(("timestamp".to_string(), iso.clone()));
                    used.timestamp = true;
                }
                When::Relative(text) => {
                    relative += 1;
                    example.get_or_insert_with(|| text.clone());
                    unit.attrs.push(("timestamp".to_string(), text.clone()));
                    used.timestamp = true;
                }
            }
            if let Some(l) = &c.likes {
                unit.attrs.push(("likes".to_string(), l.clone()));
                used.likes = true;
            }
            if let Some(id) = &c.id {
                unit.attrs.push(("external_id".to_string(), id.clone()));
                used.external_id = true;
            }
            if profile.has_role {
                if let Some(role) = &c.role {
                    unit.attrs.push(("author_role".to_string(), role.clone()));
                    used.role = true;
                }
            }
            units.push(unit);
        }
        let parsed = builder.finish(SourceType::Social, UnitKind::Comment);

        let mut unit_attributes = Vec::new();
        if used.author {
            unit_attributes.push(AttributeSpec::new("author", AppliesTo::Unit, Level::Nominal));
        }
        unit_attributes.push(AttributeSpec::new("depth", AppliesTo::Unit, Level::Ratio));
        if used.timestamp {
            unit_attributes.push(AttributeSpec::new("timestamp", AppliesTo::Unit, timestamp_level));
        }
        if used.likes {
            unit_attributes.push(AttributeSpec::new("likes", AppliesTo::Unit, Level::Ratio));
        }
        if used.external_id {
            unit_attributes.push(AttributeSpec::new("external_id", AppliesTo::Unit, Level::Text));
        }
        if used.role {
            unit_attributes.push(AttributeSpec::new("author_role", AppliesTo::Unit, Level::Nominal));
        }

        // Warnings about the thread, with counts rather than one line per comment.
        if order.orphans > 0 {
            let (subject, verb) = if order.orphans == 1 {
                ("1 reply points".to_string(), "it was")
            } else {
                (format!("{} replies point", order.orphans), "they were")
            };
            warnings.push(ImportWarning::new(
                "orphan_reply",
                format!(
                    "{subject} to a comment that is not in the file (it may have been deleted, or the export is incomplete); \
                     {verb} kept as top-level comments"
                ),
            ));
        }
        if order.cycles > 0 {
            warnings.push(ImportWarning::new(
                "parent_cycle",
                format!(
                    "{} in the file point at each other as replies; each loop was cut open at its first comment, which became a top-level comment",
                    plural(order.cycles, "loop of comments", "loops of comments")
                ),
            ));
        }
        if order.duplicate_ids > 0 {
            warnings.push(ImportWarning::new(
                "duplicate_ids",
                format!(
                    "{} an ID that an earlier comment already had; replies to that ID were attached to the first comment with it",
                    plural(order.duplicate_ids, "comment has", "comments have")
                ),
            ));
        }
        if order.empty > 0 {
            warnings.push(ImportWarning::new(
                "empty_comments",
                format!(
                    "{} no text and {} skipped; replies to {} were attached to the nearest comment above {} that was kept",
                    plural(order.empty, "comment had", "comments had"),
                    if order.empty == 1 { "was" } else { "were" },
                    if order.empty == 1 { "it" } else { "them" },
                    if order.empty == 1 { "it" } else { "them" },
                ),
            ));
        }
        if estimated + relative > 0 {
            let case_hint = if container.case_values.iter().any(|v| v.name == "published") {
                " The container's published date is stored on its case as 'published'."
            } else {
                ""
            };
            let example = example.unwrap_or_default();
            let message = if relative > 0 {
                format!(
                    "The export gives {} only as text such as '{example}', or in a form that is not a date, so 'timestamp' holds that text \
                     and cannot be used for time-based analysis. Use the container's published date for that.{case_hint}",
                    plural(relative, "comment's time", "comments' times")
                )
            } else {
                format!(
                    "The export gives times as text such as '{example}', and the dates in 'timestamp' ({}) were worked out from that \
                     text, so they are only as exact as the text is (a year, a month). For time-based analysis use the container's \
                     published date.{case_hint}",
                    plural(estimated, "comment", "comments")
                )
            };
            warnings.push(ImportWarning::new("relative_timestamps", message));
        }

        let container_name = container.title.clone().or_else(|| container.id.clone());
        let source_name = match (&opts.name, &container_name) {
            (Some(given), Some(own)) if container_count > 1 => format!("{given} - {own}"),
            (Some(given), _) => given.clone(),
            (None, Some(own)) => own.clone(),
            (None, None) => profile.fallback_name.to_string(),
        };
        let mut settings = profile.settings.clone();
        settings.extend(container.settings);
        settings.insert("profile".to_string(), json!(profile.name));
        settings.insert("container_id".to_string(), json!(container.id));
        settings.insert("container_title".to_string(), json!(container.title));
        settings.insert("comments".to_string(), json!(units.len()));
        settings.insert("order".to_string(), json!("depth_first"));
        settings.insert(
            "timestamps".to_string(),
            json!(if relative > 0 {
                "relative text"
            } else if estimated > 0 {
                "estimated from relative text"
            } else if used.timestamp {
                "exact"
            } else {
                "none"
            }),
        );

        let source_index = batch.sources.len();
        batch.sources.push(ImportedSource {
            name: source_name.clone(),
            source_type: opts.source_type.unwrap_or(SourceType::Social),
            folder: opts.folder.clone(),
            text: parsed.text,
            runs: Vec::new(),
            units,
            default_unit_kind: opts.unit_kind.unwrap_or(UnitKind::Comment),
            unit_attributes,
            original_file_hash: opts.file_hash.clone(),
            settings: Value::Object(settings),
            warnings,
        });

        let mut case = ImportedCase {
            name: container_name.unwrap_or(source_name),
            attrs: Vec::new(),
            members: vec![CaseMember { source: source_index, unit: None }],
        };
        for value in container.case_values {
            if !case_specs.iter().any(|s| s.name == value.name) {
                case_specs.push(AttributeSpec::new(value.name, AppliesTo::Case, value.level));
            }
            case.attrs.push((value.name.to_string(), value.value));
        }
        batch.cases.push(case);
    }

    if batch.sources.is_empty() {
        return Err(ImportError::format("none of the comments in the file has any text to import"));
    }
    batch.case_attributes = case_specs;
    if let Some(first) = batch.sources.first_mut() {
        first.warnings.extend(leftover);
    }
    Ok(batch)
}

#[derive(Default)]
struct Used {
    author: bool,
    timestamp: bool,
    likes: bool,
    external_id: bool,
    role: bool,
}

// ---------------------------------------------------------------------------------------------
// YouTube
// ---------------------------------------------------------------------------------------------

/// A YouTube comment export in JSON.
///
/// Two layouts are read, and which one it is gets detected from the file: the yt-dlp info file
/// (`comments` list) and the YouTube Data API v3 `commentThreads.list` or `comments.list` response
/// (`items` list).
pub fn parse_youtube_json(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let value = read_json(bytes)?;
    let Some(root) = value.as_object() else {
        return Err(ImportError::format(
            "this file is not a YouTube comment export: expected one JSON object (a yt-dlp info file or a YouTube API response)",
        ));
    };
    if root.get("comments").is_some_and(Value::is_array) {
        let container = ytdlp_container(root);
        let profile = Profile {
            name: "youtube_ytdlp",
            fallback_name: "YouTube comments",
            has_role: false,
            settings: Map::from_iter([("likes_from".to_string(), json!("like_count"))]),
            warnings: Vec::new(),
        };
        return build_batch(vec![container], profile, opts);
    }
    if root.get("items").is_some_and(Value::is_array) {
        let (containers, warnings) = api_containers(root);
        let profile = Profile {
            name: "youtube_api",
            fallback_name: "YouTube comments",
            has_role: false,
            settings: Map::from_iter([("likes_from".to_string(), json!("likeCount"))]),
            warnings,
        };
        return build_batch(containers, profile, opts);
    }
    if root.contains_key("comments") {
        return Err(ImportError::format(
            "this YouTube file has no comments in it; when you download with yt-dlp, add the option that writes comments \
             (--write-comments) and try again",
        ));
    }
    Err(ImportError::format(
        "this JSON file is not a YouTube comment export: it has neither a 'comments' list (yt-dlp) nor an 'items' list (YouTube API)",
    ))
}

fn ytdlp_container(root: &Map<String, Value>) -> Container {
    let mut skipped = 0usize;
    let mut comments = Vec::new();
    for entry in root.get("comments").and_then(Value::as_array).into_iter().flatten() {
        let Some(e) = entry.as_object() else {
            skipped += 1;
            continue;
        };
        let mut c = Comment::new(clean_text(e.get("text").and_then(Value::as_str).unwrap_or("")));
        c.id = string_of(e.get("id"));
        // yt-dlp writes "root" for a top-level comment.
        c.parent = string_of(e.get("parent")).filter(|p| p != "root");
        c.author = string_of(e.get("author"));
        c.likes = count_of(e.get("like_count"));
        let time_text = e.get("_time_text").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty());
        c.when = match (when_from_json(e.get("timestamp")), time_text) {
            // yt-dlp works the timestamp out from the relative text it was shown.
            (When::Exact(iso), Some(text)) if normalize_iso(text).is_none() => When::Estimated(iso, text.to_string()),
            (When::Unknown, Some(text)) => when_from_text(text),
            (when, _) => when,
        };
        comments.push(c);
    }
    let mut container = Container::new(comments);
    container.id = string_of(root.get("id"));
    container.title = string_of(root.get("title")).map(|t| clean_text(&t)).filter(|t| !t.is_empty());
    if let Some(id) = &container.id {
        container.case_values.push(CaseValue { name: "container", level: Level::Nominal, value: id.clone() });
    }
    if let Some(channel) = string_of(root.get("channel")).or_else(|| string_of(root.get("uploader"))) {
        container.case_values.push(CaseValue { name: "channel", level: Level::Nominal, value: channel });
    }
    let published = root
        .get("upload_date")
        .and_then(Value::as_str)
        .and_then(|d| {
            let d = d.trim();
            (d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()))
                .then(|| format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..8]))
        })
        .and_then(|d| normalize_iso(&d))
        .or_else(|| {
            root.get("timestamp")
                .and_then(Value::as_f64)
                .and_then(|t| iso_from_unix(t, false))
                .map(|t| t[..10].to_string())
        });
    if let Some(date) = published {
        container.case_values.push(CaseValue { name: "published", level: Level::Date, value: date });
    }
    if skipped > 0 {
        container.warnings.push(ImportWarning::new("skipped_entries", skipped_message(skipped, "the comments list")));
    }
    container
}

/// One comment of the API: a `comment` resource (the `snippet` holds the text).
fn api_comment(resource: &Value, fallback_parent: Option<&str>) -> Option<Comment> {
    let snippet = resource.get("snippet")?.as_object()?;
    let text = match snippet.get("textOriginal").and_then(Value::as_str) {
        Some(t) => t.to_string(),
        None => html_to_text(snippet.get("textDisplay").and_then(Value::as_str).unwrap_or("")),
    };
    let mut c = Comment::new(clean_text(&text));
    c.id = string_of(resource.get("id"));
    c.parent = string_of(snippet.get("parentId")).or_else(|| fallback_parent.map(str::to_string));
    c.author = string_of(snippet.get("authorDisplayName"));
    c.likes = count_of(snippet.get("likeCount"));
    c.when = when_from_json(snippet.get("publishedAt"));
    Some(c)
}

/// Comments of the API response, grouped by video (a response made with `allThreadsRelatedToChannelId`
/// mixes videos).
fn api_containers(root: &Map<String, Value>) -> (Vec<Container>, Vec<ImportWarning>) {
    let mut skipped = 0usize;
    let mut order: Vec<Option<String>> = Vec::new();
    let mut groups: HashMap<Option<String>, Vec<Comment>> = HashMap::new();
    let mut push = |video: Option<String>, c: Comment| {
        if !groups.contains_key(&video) {
            order.push(video.clone());
        }
        groups.entry(video).or_default().push(c);
    };
    for item in root.get("items").and_then(Value::as_array).into_iter().flatten() {
        let snippet = item.get("snippet");
        let video = snippet.and_then(|s| string_of(s.get("videoId")));
        if let Some(top) = snippet.and_then(|s| s.get("topLevelComment")) {
            // A comment thread: the top-level comment, then its replies in the order given.
            let video = video.or_else(|| top.get("snippet").and_then(|s| string_of(s.get("videoId"))));
            let Some(top_comment) = api_comment(top, None) else {
                skipped += 1;
                continue;
            };
            let top_id = top_comment.id.clone();
            push(video.clone(), Comment { parent: None, ..top_comment });
            for reply in item.pointer("/replies/comments").and_then(Value::as_array).into_iter().flatten() {
                match api_comment(reply, top_id.as_deref()) {
                    Some(c) => push(video.clone(), c),
                    None => skipped += 1,
                }
            }
        } else if let Some(c) = api_comment(item, None) {
            // A plain comment resource, from comments.list.
            let video = video.or_else(|| item.get("snippet").and_then(|s| string_of(s.get("videoId"))));
            push(video, c);
        } else {
            skipped += 1;
        }
    }
    let mut warnings = Vec::new();
    if skipped > 0 {
        warnings.push(ImportWarning::new("skipped_entries", skipped_message(skipped, "the list")));
    }
    let containers = order
        .into_iter()
        .map(|video| {
            let comments = groups.remove(&video).unwrap_or_default();
            let mut c = Container::new(comments);
            if let Some(v) = &video {
                c.case_values.push(CaseValue { name: "container", level: Level::Nominal, value: v.clone() });
            }
            c.id = video;
            c
        })
        .collect();
    (containers, warnings)
}

// ---------------------------------------------------------------------------------------------
// Reddit
// ---------------------------------------------------------------------------------------------

const REDDIT_GONE: [&str; 2] = ["[deleted]", "[removed]"];

/// A Reddit thread export in JSON: the `.json` of a thread, which is a list of two listings (the post,
/// then the comments).
pub fn parse_reddit_json(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let value = read_json(bytes)?;
    let Some(listings) = value.as_array() else {
        return Err(ImportError::format(
            "this file is not a Reddit thread: a thread's .json file is a list of two listings (the post, then its comments), \
             and this is a single object",
        ));
    };
    let mut post: Option<&Value> = None;
    let mut comment_roots: Vec<&Value> = Vec::new();
    for listing in listings {
        let Some(children) = listing.pointer("/data/children").and_then(Value::as_array) else { continue };
        for child in children {
            match child.get("kind").and_then(Value::as_str) {
                Some("t3") if post.is_none() => post = child.get("data"),
                Some("t1" | "more") => comment_roots.push(child),
                _ => {}
            }
        }
    }
    if post.is_none() && comment_roots.is_empty() {
        return Err(ImportError::format(
            "this file does not look like a Reddit thread: it has no post (kind t3) and no comments (kind t1) in its listings",
        ));
    }

    let post_data = post.and_then(Value::as_object);
    let post_id = post_data.and_then(|p| string_of(p.get("id")));
    let mut comments: Vec<Comment> = Vec::new();
    let mut deleted = 0usize;
    let mut deleted_post = false;
    let (mut stubs, mut hidden) = (0usize, 0usize);

    let mut container_title = None;
    let mut case_values = Vec::new();
    if let Some(p) = post_data {
        let title = clean_text(&decode_entities(p.get("title").and_then(Value::as_str).unwrap_or(""), false));
        let body = clean_text(&decode_entities(p.get("selftext").and_then(Value::as_str).unwrap_or(""), false));
        deleted_post = REDDIT_GONE.contains(&body.as_str());
        let text = match (title.is_empty(), body.is_empty()) {
            (false, false) => format!("{title}\n\n{body}"),
            (false, true) => title.clone(),
            _ => body,
        };
        let mut c = Comment::new(text);
        c.kind = UnitKind::Post;
        c.id = post_id.clone();
        c.author = string_of(p.get("author"));
        c.when = when_from_json(p.get("created_utc"));
        c.likes = count_of(p.get("score"));
        comments.push(c);
        container_title = Some(title).filter(|t| !t.is_empty());
        if let Some(id) = &post_id {
            case_values.push(CaseValue { name: "container", level: Level::Nominal, value: id.clone() });
        }
        if let Some(sub) = string_of(p.get("subreddit")) {
            case_values.push(CaseValue { name: "subreddit", level: Level::Nominal, value: sub });
        }
        if let When::Exact(iso) = when_from_json(p.get("created_utc")) {
            case_values.push(CaseValue { name: "published", level: Level::Date, value: iso[..10].to_string() });
        }
    }

    // Comments are nested; walk them with a stack so a deep thread cannot overflow ours. Each entry
    // remembers the comment it was nested under, which is the parent when `parent_id` is missing.
    let mut stack: Vec<(&Value, Option<String>)> = comment_roots.into_iter().rev().map(|c| (c, None)).collect();
    while let Some((node, nested_under)) = stack.pop() {
        let Some(data) = node.get("data").and_then(Value::as_object) else { continue };
        match node.get("kind").and_then(Value::as_str) {
            Some("more") => {
                stubs += 1;
                let counted = data.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
                let listed = data.get("children").and_then(Value::as_array).map_or(0, Vec::len);
                hidden += counted.max(listed);
            }
            Some("t1") => {
                let body = clean_text(&decode_entities(data.get("body").and_then(Value::as_str).unwrap_or(""), false));
                if REDDIT_GONE.contains(&body.as_str()) {
                    deleted += 1;
                }
                let mut c = Comment::new(body);
                c.id = string_of(data.get("id"));
                c.parent = match data.get("parent_id").and_then(Value::as_str) {
                    Some(p) if p.starts_with("t3_") => post_id.clone(),
                    Some(p) => p.strip_prefix("t1_").map(str::to_string),
                    None => nested_under.clone().or_else(|| post_id.clone()),
                };
                c.author = string_of(data.get("author"));
                c.when = when_from_json(data.get("created_utc"));
                c.likes = count_of(data.get("score"));
                let id = c.id.clone();
                comments.push(c);
                if let Some(children) = node.pointer("/data/replies/data/children").and_then(Value::as_array) {
                    for child in children.iter().rev() {
                        stack.push((child, id.clone()));
                    }
                }
            }
            _ => {}
        }
    }

    let mut container = Container::new(comments);
    container.id = post_id;
    container.title = container_title;
    container.case_values = case_values;
    if let Some(p) = post_data {
        container.settings.insert("permalink".to_string(), json!(string_of(p.get("permalink"))));
        container.settings.insert("subreddit".to_string(), json!(string_of(p.get("subreddit"))));
    }
    if deleted > 0 || deleted_post {
        let what = match (deleted, deleted_post) {
            (0, _) => "The post's text shows".to_string(),
            (n, false) => format!("{} show", plural(n, "comment", "comments")),
            (n, true) => format!("The post's text and {} show", plural(n, "comment", "comments")),
        };
        container.warnings.push(ImportWarning::new(
            "deleted_comments",
            format!(
                "{what} '[deleted]' or '[removed]': the author or a moderator took the text away. The placeholder was kept as the text, \
                 so these units say nothing about the topic; leave them out of coding."
            ),
        ));
    }
    if stubs > 0 {
        container.warnings.push(ImportWarning::new(
            "unloaded_replies",
            format!(
                "The export has {} ('load more comments' links) standing for {}{} that were not saved in the file. They are not in the import.",
                plural(stubs, "placeholder", "placeholders"),
                if hidden > 0 { "about " } else { "" },
                if hidden > 0 { plural(hidden, "reply", "replies") } else { "more replies".to_string() },
            ),
        ));
    }
    let profile = Profile {
        name: "reddit",
        fallback_name: "Reddit thread",
        has_role: false,
        settings: Map::from_iter([
            ("likes_from".to_string(), json!("score")),
            ("likes_meaning".to_string(), json!("net score: upvotes minus downvotes")),
            ("top_level_comments_reply_to".to_string(), json!("the post unit, which has depth 0")),
        ]),
        warnings: Vec::new(),
    };
    build_batch(vec![container], profile, opts)
}

// ---------------------------------------------------------------------------------------------
// Generic table
// ---------------------------------------------------------------------------------------------

static EMPTY_CELL: Cell = Cell::Empty;

fn cell_at(table: &Table, row: usize, col: usize) -> &Cell {
    table.rows[row].get(col).unwrap_or(&EMPTY_CELL)
}

fn trimmed(cell: &Cell) -> Option<String> {
    let t = cell.text();
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn when_from_cell(cell: &Cell) -> When {
    match cell {
        Cell::Empty => When::Unknown,
        Cell::Date(d) => match normalize_iso(d) {
            Some(iso) => When::Exact(iso),
            None => when_from_text(d),
        },
        Cell::Number(n) => iso_from_unix(*n, true).map_or_else(|| When::Relative(cell.text()), When::Exact),
        other => match other.text().trim() {
            "" => When::Unknown,
            t => match normalize_iso(t) {
                Some(iso) => When::Exact(iso),
                None => match t.parse::<f64>().ok().and_then(|x| iso_from_unix(x, true)) {
                    Some(iso) => When::Exact(iso),
                    None => When::Relative(t.to_string()),
                },
            },
        },
    }
}

fn likes_from_cell(cell: &Cell) -> Option<String> {
    match cell {
        Cell::Number(n) => number_text(*n),
        Cell::Empty | Cell::Date(_) | Cell::Bool(_) => None,
        Cell::Text(s) => s.trim().replace(',', "").parse::<f64>().ok().and_then(number_text),
    }
}

/// A generic layout: one row per post or comment.
pub fn from_table(table: &Table, mapping: &SocialMapping, opts: &ImportOptions) -> Result<ImportBatch> {
    let named = |n: &str| -> Option<String> {
        let n = n.trim();
        (!n.is_empty()).then(|| n.to_string())
    };
    let optional = |n: &Option<String>| n.as_deref().and_then(named);

    let text_name = named(&mapping.text_column)
        .ok_or_else(|| ImportError::format("choose the column that holds the text of each comment"))?;
    let id_name = named(&mapping.id_column);
    let parent_name = optional(&mapping.parent_column);
    if parent_name.is_some() && id_name.is_none() {
        return Err(ImportError::format(
            "to keep replies, choose both the column that holds each comment's own ID and the column that holds the ID of the comment it replies to",
        ));
    }
    let author_name = optional(&mapping.author_column);
    let role_name = optional(&mapping.author_role_column);
    let time_name = optional(&mapping.timestamp_column);
    let container_name = optional(&mapping.container_column);
    let likes_name = optional(&mapping.likes_column);

    // Every mapped name must be a column of the table; say all the wrong ones at once.
    let wanted = [
        Some(&text_name),
        id_name.as_ref(),
        parent_name.as_ref(),
        author_name.as_ref(),
        role_name.as_ref(),
        time_name.as_ref(),
        container_name.as_ref(),
        likes_name.as_ref(),
    ];
    let missing: Vec<String> =
        wanted.iter().flatten().filter(|n| table.column_index(n).is_none()).map(|n| format!("'{n}'")).collect();
    if !missing.is_empty() {
        let have: Vec<String> = table.columns.iter().take(20).map(|c| format!("'{}'", c.name)).collect();
        return Err(ImportError::format(format!(
            "{} not a column of this file: {}. The columns are {}{}",
            if missing.len() == 1 { "this is" } else { "these are" },
            missing.join(", "),
            have.join(", "),
            if table.columns.len() > 20 { ", and more" } else { "" },
        )));
    }
    let col = |n: &Option<String>| n.as_deref().and_then(|n| table.column_index(n));
    let text_col = table.column_index(&text_name).expect("checked above");
    let (id_col, parent_col, author_col, role_col, time_col, container_col, likes_col) = (
        col(&id_name),
        col(&parent_name),
        col(&author_name),
        col(&role_name),
        col(&time_name),
        col(&container_name),
        col(&likes_name),
    );

    // Group the rows by container, in the order the containers first appear.
    let mut keys: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, Container> = HashMap::new();
    let mut without_container = 0usize;
    let mut bad_likes = 0usize;
    for r in 0..table.rows.len() {
        let key = match container_col {
            Some(c) => match trimmed(cell_at(table, r, c)) {
                Some(k) => k,
                None => {
                    without_container += 1;
                    String::new()
                }
            },
            None => String::new(),
        };
        let mut c = Comment::new(clean_text(&cell_at(table, r, text_col).text()));
        c.id = id_col.and_then(|i| trimmed(cell_at(table, r, i)));
        c.parent = parent_col.and_then(|i| trimmed(cell_at(table, r, i)));
        c.author = author_col.and_then(|i| trimmed(cell_at(table, r, i)));
        c.role = role_col.and_then(|i| trimmed(cell_at(table, r, i)));
        c.when = time_col.map_or(When::Unknown, |i| when_from_cell(cell_at(table, r, i)));
        if let Some(i) = likes_col {
            let cell = cell_at(table, r, i);
            c.likes = likes_from_cell(cell);
            if c.likes.is_none() && !cell.is_empty() {
                bad_likes += 1;
            }
        }
        by_key
            .entry(key.clone())
            .or_insert_with(|| {
                keys.push(key.clone());
                Container::new(Vec::new())
            })
            .comments
            .push(c);
    }

    let mut warnings = table.warnings.clone();
    if without_container > 0 {
        warnings.push(ImportWarning::new(
            "missing_container",
            format!(
                "{} no value in the container column, so they were put together in one source",
                plural(without_container, "row has", "rows have")
            ),
        ));
    }
    if bad_likes > 0 {
        warnings.push(ImportWarning::new(
            "unreadable_likes",
            format!(
                "{} in the likes column {} not a number and {} left blank",
                plural(bad_likes, "value", "values"),
                if bad_likes == 1 { "was" } else { "were" },
                if bad_likes == 1 { "was" } else { "were" }
            ),
        ));
    }

    let single = container_col.is_none();
    let containers: Vec<Container> = keys
        .into_iter()
        .map(|key| {
            let mut c = by_key.remove(&key).expect("every key has a container");
            if !key.is_empty() {
                c.title = Some(key.clone());
                c.id = Some(key.clone());
                c.case_values.push(CaseValue { name: "container", level: Level::Nominal, value: key });
            } else if single {
                c.title =
                    Some(opts.name.clone().unwrap_or_else(|| table.name.clone())).filter(|t| !t.trim().is_empty());
            } else {
                c.title = Some("No container".to_string());
            }
            c
        })
        .collect();

    let profile = Profile {
        name: "table",
        fallback_name: "Comments",
        has_role: role_col.is_some(),
        settings: Map::from_iter([
            ("table".to_string(), json!(table.name)),
            ("mapping".to_string(), serde_json::to_value(mapping).unwrap_or(Value::Null)),
        ]),
        warnings,
    };
    build_batch(containers, profile, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment(id: &str, parent: Option<&str>, text: &str) -> Comment {
        Comment { id: Some(id.to_string()), parent: parent.map(str::to_string), ..Comment::new(text.to_string()) }
    }

    fn placed(o: &Ordered) -> Vec<(usize, Option<usize>, u32)> {
        o.slots.iter().map(|s| (s.comment, s.parent, s.depth)).collect()
    }

    #[test]
    fn unix_times_become_utc_iso_text() {
        assert_eq!(iso_from_unix(1_700_000_000.0, false).as_deref(), Some("2023-11-14T22:13:20Z"));
        // Milliseconds are recognised by their size.
        assert_eq!(iso_from_unix(1_700_000_000_123.0, false).as_deref(), Some("2023-11-14T22:13:20Z"));
        // A leap day, and a month change.
        assert_eq!(iso_from_unix(951_782_400.0, false).as_deref(), Some("2000-02-29T00:00:00Z"));
        assert_eq!(iso_from_unix(1_709_251_200.0, false).as_deref(), Some("2024-03-01T00:00:00Z"));
        // Fractions of a second are dropped.
        assert_eq!(iso_from_unix(1_700_000_000.9, false).as_deref(), Some("2023-11-14T22:13:20Z"));
        // Numbers that cannot be times are refused; the strict range also refuses the 1990s.
        assert_eq!(iso_from_unix(42.0, false), None);
        assert_eq!(iso_from_unix(f64::NAN, false), None);
        assert_eq!(iso_from_unix(-5.0, false), None);
        assert_eq!(iso_from_unix(900_000_000.0, false).as_deref(), Some("1998-07-09T16:00:00Z"));
        assert_eq!(iso_from_unix(900_000_000.0, true), None);
    }

    #[test]
    fn iso_text_is_checked_and_given_a_t_separator() {
        for (input, want) in [
            ("2024-03-05", Some("2024-03-05")),
            ("2024-03-05T10:20", Some("2024-03-05T10:20")),
            ("2024-03-05 10:20:30", Some("2024-03-05T10:20:30")),
            ("2024-03-05T10:20:30Z", Some("2024-03-05T10:20:30Z")),
            ("2024-03-05T10:20:30.123456+01:00", Some("2024-03-05T10:20:30.123456+01:00")),
            ("2024-03-05T10:20:30-0500", Some("2024-03-05T10:20:30-0500")),
            ("  2024-03-05T10:20:30Z  ", Some("2024-03-05T10:20:30Z")),
            ("2 years ago", None),
            ("2024-13-05", None),
            ("2024-03-32", None),
            ("2024-03-05T25:00", None),
            ("2024-03-05T10:61", None),
            ("2024-03-05T10:20:30.Z", None),
            ("2024-03-05T10:20:30+1", None),
            ("2024-03-05T10:20:30\u{00E9}", None),
            ("2024-03-05x", None),
            ("March 5, 2024", None),
            ("", None),
        ] {
            assert_eq!(normalize_iso(input).as_deref(), want, "{input:?}");
        }
    }

    #[test]
    fn times_in_text_are_exact_or_relative() {
        assert_eq!(when_from_text("2024-03-05T10:20:30Z"), When::Exact("2024-03-05T10:20:30Z".into()));
        assert_eq!(when_from_text("1700000000"), When::Exact("2023-11-14T22:13:20Z".into()));
        assert_eq!(when_from_text("2 years ago"), When::Relative("2 years ago".into()));
        assert_eq!(when_from_text("12"), When::Relative("12".into()));
        assert_eq!(when_from_text("  "), When::Unknown);
        assert_eq!(when_from_json(None), When::Unknown);
        assert_eq!(when_from_json(Some(&json!(null))), When::Unknown);
        assert_eq!(when_from_json(Some(&json!(1_700_000_000))), When::Exact("2023-11-14T22:13:20Z".into()));
    }

    #[test]
    fn character_references_and_small_pieces_of_html_become_text() {
        assert_eq!(
            decode_entities("a &amp; b &lt;3 &gt; &#39;x&#x27; &#x1F600; &quot;", true),
            "a & b <3 > 'x' \u{1F600} \""
        );
        // Only the three escapes a web service always adds are replaced unless asked.
        assert_eq!(decode_entities("&amp; &quot; &nbsp; &#39;", false), "& &quot; &nbsp; '");
        // Things that are not references stay as they are, and a reference is replaced once only.
        assert_eq!(decode_entities("AT&T; &unknown; &#xZZ; &amp;lt; & end", true), "AT&T; &unknown; &#xZZ; &lt; & end");
        assert_eq!(
            html_to_text("one<br>two<br/>three <b>bold</b> <a href=\"x?a=1&amp;b=2\">link</a>"),
            "one\ntwo\nthree bold link"
        );
        // A less-than sign that does not start a tag is kept.
        assert_eq!(html_to_text("1 < 2 and <3 and a<b>c"), "1 < 2 and <3 and ac");
        assert_eq!(html_to_text("unfinished <a href=\"x"), "unfinished <a href=\"x");
    }

    #[test]
    fn comment_text_is_trimmed_and_cleaned() {
        assert_eq!(clean_text("  a\r\nb\rc\u{0}\u{7}d\te  \n"), "a\nb\ncd\te");
        assert_eq!(clean_text(" \n\t "), "");
    }

    #[test]
    fn counts_print_without_a_decimal_point() {
        assert_eq!(count_of(Some(&json!(12))).as_deref(), Some("12"));
        assert_eq!(count_of(Some(&json!(12.0))).as_deref(), Some("12"));
        assert_eq!(count_of(Some(&json!(-3))).as_deref(), Some("-3"));
        assert_eq!(count_of(Some(&json!(2.5))).as_deref(), Some("2.5"));
        assert_eq!(count_of(Some(&json!("1,234"))).as_deref(), Some("1234"));
        assert_eq!(count_of(Some(&json!("many"))), None);
        assert_eq!(count_of(Some(&json!(null))), None);
        assert_eq!(count_of(None), None);
    }

    #[test]
    fn threads_are_put_in_depth_first_order_with_parents_first() {
        let comments = vec![
            comment("a", None, "A"),
            comment("b", Some("a"), "B"),
            comment("c", None, "C"),
            comment("d", Some("a"), "D"),
            comment("e", Some("b"), "E"),
        ];
        let o = thread_order(&comments);
        // A, B, E (B's reply), D, then C.
        assert_eq!(placed(&o), [(0, None, 0), (1, Some(0), 1), (4, Some(1), 2), (3, Some(0), 1), (2, None, 0)]);
        assert_eq!((o.orphans, o.cycles, o.duplicate_ids, o.empty), (0, 0, 0, 0));
    }

    #[test]
    fn replies_to_a_missing_comment_become_top_level_and_are_counted() {
        let comments = vec![comment("a", None, "A"), comment("b", Some("missing"), "B"), comment("c", Some("b"), "C")];
        let o = thread_order(&comments);
        assert_eq!(o.orphans, 1);
        assert_eq!(placed(&o), [(0, None, 0), (1, None, 0), (2, Some(1), 1)]);
    }

    #[test]
    fn comments_without_text_are_skipped_and_their_replies_move_up() {
        let comments = vec![
            comment("a", None, "A"),
            comment("b", Some("a"), ""),
            comment("c", Some("b"), "C"),
            comment("d", Some("c"), "D"),
        ];
        let o = thread_order(&comments);
        assert_eq!(o.empty, 1);
        // C attaches to A, the nearest comment above it that stays, one level down.
        assert_eq!(placed(&o), [(0, None, 0), (2, Some(0), 1), (3, Some(1), 2)]);
    }

    #[test]
    fn every_comment_is_placed_exactly_once_even_in_a_tangle() {
        // Every comment replies to some other comment, in loops and chains.
        let n = 500;
        let comments: Vec<Comment> = (0..n)
            .map(|i| comment(&format!("c{i}"), Some(&format!("c{}", (i * 7 + 3) % n)), &format!("t{i}")))
            .collect();
        let o = thread_order(&comments);
        assert_eq!(o.slots.len(), n);
        let mut seen: Vec<usize> = o.slots.iter().map(|s| s.comment).collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..n).collect::<Vec<_>>());
        assert!(o.slots.iter().enumerate().all(|(i, s)| s.parent.is_none_or(|p| p < i)));
        assert!(o.cycles > 0);
    }
}
