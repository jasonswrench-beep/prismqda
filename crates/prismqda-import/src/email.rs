//! E-mail import: single messages (`.eml`), mailboxes (`.mbox`), and saved web pages (`.mhtml`), which are MIME messages too.
//!
//! Every message becomes one source (its header lines and its text) and one case whose variables hold From, To, Date, and
//! Subject. Attachments are not read; each one is named in the import report.

use prismqda_engine::{AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportWarning, ImportedCase, Level};

use crate::error::{ImportError, Result};
use crate::options::ImportOptions;
use crate::text::{clean, parse_html, parse_txt};

/// Messages nested deeper than this are not followed (a hostile file can nest forever).
const MAX_DEPTH: usize = 12;
/// A mailbox with more messages than this is refused, so one file cannot fill a project by accident.
const MAX_MESSAGES: usize = 20_000;

// ---------------------------------------------------------------- bytes to text

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() || from > hay.len() - needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// The header block and the body of a message or part: split at the first blank line.
fn split_head_body(raw: &[u8]) -> (&[u8], &[u8]) {
    let crlf = find(raw, b"\r\n\r\n", 0).map(|i| (i, 4));
    let lf = find(raw, b"\n\n", 0).map(|i| (i, 2));
    let cut = match (crlf, lf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (a, b) => a.or(b),
    };
    match cut {
        Some((i, len)) => (&raw[..i], &raw[i + len..]),
        // No blank line: a message with headers only (or with no headers at all).
        None => (raw, &[]),
    }
}

fn decode_base64(input: &[u8]) -> Vec<u8> {
    let value = |c: u8| -> Option<u8> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        })
    };
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in input {
        if c == b'=' {
            break;
        }
        if let Some(v) = value(c) {
            acc = (acc << 6) | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
                acc &= (1 << bits) - 1;
            }
        }
    }
    out
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

/// Quoted-printable: `=XX` is a byte and `=` at the end of a line joins it to the next. `header` also reads `_` as a space.
fn decode_qp(input: &[u8], header: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let c = input[i];
        if c == b'=' {
            if input.get(i + 1) == Some(&b'\r') && input.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if input.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            if let (Some(&a), Some(&b)) = (input.get(i + 1), input.get(i + 2)) {
                if let (Some(h), Some(l)) = (hex(a), hex(b)) {
                    out.push(h * 16 + l);
                    i += 3;
                    continue;
                }
            }
            out.push(c);
        } else if header && c == b'_' {
            out.push(b' ');
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// Bytes in a named character set to text. An unknown or missing name is read as UTF-8; whether that was a guess is reported.
fn decode_charset(bytes: &[u8], label: Option<&str>) -> (String, bool) {
    let enc = label.and_then(|l| encoding_rs::Encoding::for_label(l.trim().trim_matches('"').as_bytes()));
    match enc {
        Some(e) => (e.decode(bytes).0.into_owned(), false),
        None => match std::str::from_utf8(bytes) {
            Ok(s) => (s.to_string(), false),
            // Not UTF-8 and no usable name: Windows-1252 is what mail without a charset almost always is.
            Err(_) => (encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(), true),
        },
    }
}

/// RFC 2047: `=?utf-8?B?...?=` and `=?iso-8859-1?Q?...?=` words in a header value. Words separated only by space run together.
fn decode_words(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut last_was_word = false;
    while i < bytes.len() {
        if bytes[i] == b'=' && bytes.get(i + 1) == Some(&b'?') {
            // =?charset?enc?text?=
            let rest = &value[i + 2..];
            let parts: Vec<&str> = rest.splitn(4, '?').collect();
            if parts.len() == 4 && parts[3].starts_with('=') && matches!(parts[1], "B" | "b" | "Q" | "q") {
                // The text ends at the first "?=" after the encoding.
                let text_and_more = parts[2];
                let raw = if parts[1].eq_ignore_ascii_case("b") { decode_base64(text_and_more.as_bytes()) } else { decode_qp(text_and_more.as_bytes(), true) };
                // A charset may carry a language (`utf-8*en`).
                let charset = parts[0].split('*').next().unwrap_or(parts[0]);
                out.push_str(&decode_charset(&raw, Some(charset)).0);
                i += 2 + parts[0].len() + 1 + parts[1].len() + 1 + parts[2].len() + 2;
                last_was_word = true;
                continue;
            }
        }
        if last_was_word && bytes[i].is_ascii_whitespace() {
            // Space between two encoded words is not part of the text; look ahead to see whether another word follows.
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if bytes.get(j) == Some(&b'=') && bytes.get(j + 1) == Some(&b'?') {
                i = j;
                continue;
            }
        }
        last_was_word = false;
        // Push the whole character, not one byte of it.
        let ch = value[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

// ---------------------------------------------------------------- parts and headers

#[derive(Debug, Clone, Default)]
struct Part {
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Part {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

fn parse_part(raw: &[u8]) -> Part {
    let (head, body) = split_head_body(raw);
    let head = String::from_utf8_lossy(head);
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in head.lines() {
        if line.starts_with([' ', '\t']) {
            if let Some(last) = headers.last_mut() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            if !name.is_empty() && !name.contains(' ') {
                headers.push((name.to_lowercase(), value.trim().to_string()));
            }
        }
    }
    Part { headers, body: body.to_vec() }
}

/// A parameter of a header such as Content-Type: `charset`, `boundary`, `name`.
fn parameter(value: &str, name: &str) -> Option<String> {
    let mut parts = Vec::new();
    let (mut current, mut quoted) = (String::new(), false);
    for c in value.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts.into_iter().skip(1).find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k.trim().eq_ignore_ascii_case(name)).then(|| v.trim().trim_matches('"').to_string())
    })
}

fn media_type(part: &Part) -> (String, String) {
    let raw = part.header("content-type").unwrap_or("text/plain");
    let main = raw.split(';').next().unwrap_or("text/plain").trim().to_lowercase();
    match main.split_once('/') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => ("text".to_string(), "plain".to_string()),
    }
}

/// The part's body with its transfer encoding undone.
fn transfer_decoded(part: &Part) -> Vec<u8> {
    match part.header("content-transfer-encoding").map(|v| v.trim().to_lowercase()).as_deref() {
        Some("base64") => decode_base64(&part.body),
        Some("quoted-printable") => decode_qp(&part.body, false),
        _ => part.body.clone(),
    }
}

/// The pieces of a multipart body, between its boundary lines.
fn split_multipart(body: &[u8], boundary: &str) -> Vec<Vec<u8>> {
    let delimiter = format!("--{boundary}");
    let closing = format!("--{boundary}--");
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<Vec<u8>> = None;
    for line in body.split_inclusive(|&b| b == b'\n') {
        let trimmed: &[u8] = {
            let mut end = line.len();
            while end > 0 && matches!(line[end - 1], b'\n' | b'\r' | b' ' | b'\t') {
                end -= 1;
            }
            &line[..end]
        };
        if trimmed == closing.as_bytes() {
            if let Some(p) = current.take() {
                parts.push(p);
            }
            return parts;
        }
        if trimmed == delimiter.as_bytes() {
            if let Some(p) = current.take() {
                parts.push(p);
            }
            current = Some(Vec::new());
            continue;
        }
        if let Some(p) = current.as_mut() {
            p.extend_from_slice(line);
        }
    }
    if let Some(p) = current.take() {
        parts.push(p);
    }
    parts
}

/// What was read from the readable parts of a message.
#[derive(Debug, Default)]
struct Extract {
    /// The text of each readable part, in order.
    texts: Vec<String>,
    /// The first HTML part found, for callers that want the page itself (saved web pages).
    html: Option<String>,
    /// Names of the parts that were left out.
    skipped: Vec<String>,
    guessed_charset: bool,
}

fn html_to_text(html: &str, opts: &ImportOptions) -> String {
    parse_html(html.as_bytes(), opts).map(|p| p.text).unwrap_or_default()
}

fn extract(part: &Part, depth: usize, opts: &ImportOptions, out: &mut Extract) {
    if depth > MAX_DEPTH {
        out.skipped.push("a part nested too deeply to read".to_string());
        return;
    }
    let (main, sub) = media_type(part);
    let disposition = part.header("content-disposition").unwrap_or("").to_lowercase();
    let filename = part
        .header("content-disposition")
        .and_then(|d| parameter(d, "filename"))
        .or_else(|| part.header("content-type").and_then(|t| parameter(t, "name")))
        .map(|n| decode_words(&n));
    let attached = disposition.trim_start().starts_with("attachment");

    if main == "multipart" {
        let Some(boundary) = part.header("content-type").and_then(|t| parameter(t, "boundary")) else {
            out.skipped.push("a multipart part with no boundary".to_string());
            return;
        };
        let children: Vec<Part> = split_multipart(&part.body, &boundary).iter().map(|raw| parse_part(raw)).collect();
        if sub == "alternative" {
            // Alternatives say the same thing in different forms: take the plain text, else the HTML, never both.
            let mut best: Option<Extract> = None;
            let mut html_first: Option<String> = None;
            for child in &children {
                let mut one = Extract::default();
                extract(child, depth + 1, opts, &mut one);
                if html_first.is_none() {
                    html_first = one.html.clone();
                }
                let (cm, cs) = media_type(child);
                let is_plain = cm == "text" && cs == "plain";
                if best.is_none() || is_plain || (!best.as_ref().unwrap().texts.iter().any(|t| !t.trim().is_empty()) && !one.texts.is_empty()) {
                    best = Some(one);
                } else {
                    out.skipped.extend(one.skipped);
                }
            }
            if let Some(b) = best {
                out.texts.extend(b.texts);
                out.skipped.extend(b.skipped);
                out.guessed_charset |= b.guessed_charset;
            }
            if out.html.is_none() {
                out.html = html_first;
            }
        } else {
            for child in &children {
                extract(child, depth + 1, opts, out);
            }
        }
        return;
    }

    if attached || main != "text" {
        let what = filename.unwrap_or_else(|| format!("{main}/{sub}"));
        out.skipped.push(what);
        return;
    }

    // text/plain, text/html, and other text parts.
    let bytes = transfer_decoded(part);
    let charset = part.header("content-type").and_then(|t| parameter(t, "charset"));
    let (text, guessed) = decode_charset(&bytes, charset.as_deref());
    out.guessed_charset |= guessed;
    if sub == "html" || sub == "xhtml" {
        if out.html.is_none() {
            out.html = Some(text.clone());
        }
        out.texts.push(html_to_text(&text, opts));
    } else if sub == "plain" || sub.is_empty() {
        out.texts.push(clean(&text));
    } else {
        out.skipped.push(filename.unwrap_or_else(|| format!("{main}/{sub}")));
    }
}

// ---------------------------------------------------------------- dates and names

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// "Tue, 3 Nov 2026 14:05:00 +0000" to "2026-11-03".
pub(crate) fn iso_date(value: &str) -> Option<String> {
    let tokens: Vec<&str> = value.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).collect();
    let month_of = |t: &str| t.get(..3).and_then(|p| MONTHS.iter().position(|x| *x == p.to_lowercase())).map(|i| i + 1);
    let m = tokens.iter().position(|t| month_of(t).is_some())?;
    let month = month_of(tokens[m])?;
    let day: u32 = tokens.get(m.checked_sub(1)?)?.parse().ok()?;
    let year_text = tokens.get(m + 1)?;
    let mut year: i32 = year_text.parse().ok()?;
    if year_text.len() == 2 {
        year += if year < 70 { 2000 } else { 1900 };
    } else if year_text.len() == 3 {
        year += 1900;
    }
    ((1..=31).contains(&day) && (1000..=2999).contains(&year)).then(|| format!("{year:04}-{month:02}-{day:02}"))
}

/// A person's name from an address header: "Maria Lopez <maria@example.org>" is "Maria Lopez".
fn display_name(address: &str) -> String {
    let address = address.trim();
    if let Some((name, _)) = address.split_once('<') {
        let name = name.trim().trim_matches('"').trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    address.trim_matches(['<', '>']).to_string()
}

fn shorten(s: &str, max: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        return s;
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('\u{2026}');
    t
}

// ---------------------------------------------------------------- messages

/// One message ready to become a source and a case.
struct Message {
    name: String,
    text: String,
    from: String,
    to: String,
    date: Option<String>,
    subject: String,
    warnings: Vec<ImportWarning>,
}

fn read_message(raw: &[u8], number: usize, opts: &ImportOptions) -> Option<Message> {
    let part = parse_part(raw);
    // Something with no headers at all is not a message.
    let known = ["from", "to", "subject", "date", "message-id", "received", "mime-version"];
    if !part.headers.iter().any(|(n, _)| known.contains(&n.as_str())) {
        return None;
    }
    let get = |n: &str| part.header(n).map(decode_words).map(|v| v.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
    let (from, to, cc, subject, date_raw) = (get("from"), get("to"), get("cc"), get("subject"), get("date"));
    let mut found = Extract::default();
    extract(&part, 0, opts, &mut found);
    let body = found.texts.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n\n");

    let mut header_lines = Vec::new();
    for (label, value) in [("From", &from), ("To", &to), ("Cc", &cc), ("Date", &date_raw), ("Subject", &subject)] {
        if !value.is_empty() {
            header_lines.push(format!("{label}: {value}"));
        }
    }
    let text = if body.is_empty() { header_lines.join("\n") } else { format!("{}\n\n{}", header_lines.join("\n"), body) };

    let mut warnings = Vec::new();
    if body.is_empty() {
        warnings.push(ImportWarning::new("no_text", "This message has no readable text. Its header lines were imported."));
    }
    if found.guessed_charset {
        warnings.push(ImportWarning::new("assumed_encoding", "A part of this message did not say how it was written, so it was read as Windows-1252 (Western European)."));
    }
    if !found.skipped.is_empty() {
        let shown: Vec<String> = found.skipped.iter().take(5).cloned().collect();
        let more = found.skipped.len().saturating_sub(shown.len());
        let mut m = format!("{} attachment{} not imported: {}", found.skipped.len(), if found.skipped.len() == 1 { " was" } else { "s were" }, shown.join(", "));
        if more > 0 {
            m.push_str(&format!(" and {more} more"));
        }
        m.push('.');
        warnings.push(ImportWarning::new("skipped_attachment", m));
    }
    let date = iso_date(&date_raw);
    let who = display_name(&from);
    let subject_label = if subject.is_empty() { "(no subject)".to_string() } else { subject.clone() };
    let name = shorten(
        &[subject_label, who, date.clone().unwrap_or_default()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" \u{2013} "),
        90,
    );
    let name = if name.is_empty() { format!("Message {number}") } else { name };
    Some(Message { name, text, from, to, date, subject, warnings })
}

fn batch_of(messages: Vec<Message>, format: &str, opts: &ImportOptions) -> Result<ImportBatch> {
    if messages.is_empty() {
        return Err(ImportError::format("the file has no e-mail messages in it"));
    }
    let folder = if opts.folder.is_empty() { "Email".to_string() } else { opts.folder.clone() };
    let mut batch = ImportBatch::default();
    let mut specs: Vec<AttributeSpec> = Vec::new();
    for (n, m) in messages.into_iter().enumerate() {
        let one_opts = ImportOptions { name: None, folder: folder.clone(), ..opts.clone() };
        let parsed = parse_txt(m.text.as_bytes(), &one_opts)?;
        let mut sources = crate::parsed_to_batch(&m.name, parsed, &one_opts)?.sources;
        let mut source = sources.remove(0);
        source.warnings.extend(m.warnings);
        source.settings = serde_json::json!({ "format": format, "message": n + 1 });
        batch.sources.push(source);
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut add = |name: &str, level: Level, value: Option<String>| {
            if let Some(v) = value.filter(|v| !v.is_empty()) {
                if !specs.iter().any(|s| s.name == name) {
                    specs.push(AttributeSpec::new(name, AppliesTo::Case, level));
                }
                attrs.push((name.to_string(), v));
            }
        };
        add("From", Level::Text, Some(m.from));
        add("To", Level::Text, Some(m.to));
        add("Date", Level::Date, m.date);
        add("Subject", Level::Text, Some(m.subject));
        batch.cases.push(ImportedCase { name: batch.sources[batch.sources.len() - 1].name.clone(), attrs, members: vec![CaseMember { source: batch.sources.len() - 1, unit: None }] });
    }
    batch.case_attributes = specs;
    Ok(batch)
}

/// Reads one message (`.eml`).
pub fn parse_eml(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let message = read_message(bytes, 1, opts).ok_or_else(|| ImportError::format("this does not look like an e-mail message: it has no From, To, Subject, or Date line"))?;
    batch_of(vec![message], "eml", opts)
}

/// Reads a mailbox (`.mbox`): messages that each start with a line beginning "From ".
pub fn parse_mbox(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let mut raws: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<Vec<u8>> = None;
    let mut previous_blank = true;
    for line in bytes.split_inclusive(|&b| b == b'\n') {
        if line.starts_with(b"From ") && previous_blank {
            if let Some(m) = current.take() {
                raws.push(m);
            }
            current = Some(Vec::new());
            previous_blank = false;
            continue;
        }
        previous_blank = line.iter().all(|b| b.is_ascii_whitespace());
        if let Some(m) = current.as_mut() {
            m.extend_from_slice(line);
        }
    }
    if let Some(m) = current.take() {
        raws.push(m);
    }
    if raws.is_empty() {
        // No separators: a single message saved with a .mbox name.
        raws.push(bytes.to_vec());
    }
    if raws.len() > MAX_MESSAGES {
        return Err(ImportError::unsupported(format!("this mailbox has {} messages; PrismQDA imports up to {MAX_MESSAGES} at a time. Split it first.", raws.len())));
    }
    let mut messages = Vec::new();
    let mut skipped = 0usize;
    for (i, raw) in raws.iter().enumerate() {
        match read_message(raw, i + 1, opts) {
            Some(m) => messages.push(m),
            None => skipped += 1,
        }
    }
    let mut batch = batch_of(messages, "mbox", opts)?;
    if skipped > 0 {
        if let Some(first) = batch.sources.first_mut() {
            first.warnings.push(ImportWarning::new("skipped_message", format!("{skipped} entr{} in the mailbox had no message headers and {} left out.", if skipped == 1 { "y" } else { "ies" }, if skipped == 1 { "was" } else { "were" })));
        }
    }
    Ok(batch)
}

// ---------------------------------------------------------------- saved web pages

/// Reads a saved web page (`.mhtml`, `.mht`): the page's text, with its address and date as variables.
pub fn parse_mhtml(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let part = parse_part(bytes);
    if part.headers.is_empty() {
        return Err(ImportError::format("this does not look like a saved web page: it has no header lines"));
    }
    let mut found = Extract::default();
    extract(&part, 0, opts, &mut found);
    let page = found.html.as_deref().map(|h| html_to_text(h, opts)).unwrap_or_default();
    let body = if page.trim().is_empty() { found.texts.iter().map(|t| t.trim()).collect::<Vec<_>>().join("\n\n") } else { page };
    if body.trim().is_empty() {
        return Err(ImportError::format("the saved page has no text to import"));
    }
    let decode = |n: &str| part.header(n).map(decode_words).map(|v| v.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
    let title = decode("subject");
    let address = [decode("snapshot-content-location"), decode("content-location")].into_iter().find(|s| !s.is_empty()).unwrap_or_default();
    let saved = iso_date(&decode("date"));

    let one_opts = ImportOptions { name: None, folder: if opts.folder.is_empty() { "Web pages".to_string() } else { opts.folder.clone() }, ..opts.clone() };
    let parsed = parse_txt(body.as_bytes(), &one_opts)?;
    let name = if title.is_empty() { shorten(&address, 80) } else { shorten(&title, 90) };
    let name = if name.is_empty() { "Saved web page".to_string() } else { name };
    let mut batch = crate::parsed_to_batch(&name, parsed, &one_opts)?;
    let mut warnings = Vec::new();
    if !found.skipped.is_empty() {
        warnings.push(ImportWarning::new("skipped_resource", format!("{} picture{}, style sheet{}, or other file{} saved with the page {} not imported.", found.skipped.len(), if found.skipped.len() == 1 { "" } else { "s" }, if found.skipped.len() == 1 { "" } else { "s" }, if found.skipped.len() == 1 { "" } else { "s" }, if found.skipped.len() == 1 { "was" } else { "were" })));
    }
    batch.sources[0].warnings.extend(warnings);
    batch.sources[0].settings = serde_json::json!({ "format": "mhtml", "address": address });
    let mut specs = Vec::new();
    let mut attrs = Vec::new();
    for (n, level, v) in [("Title", Level::Text, title), ("Address", Level::Text, address), ("Saved", Level::Date, saved.unwrap_or_default())] {
        if !v.is_empty() {
            specs.push(AttributeSpec::new(n, AppliesTo::Case, level));
            attrs.push((n.to_string(), v));
        }
    }
    batch.cases.push(ImportedCase { name, attrs, members: vec![CaseMember { source: 0, unit: None }] });
    batch.case_attributes = specs;
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_and_quoted_printable_decode() {
        assert_eq!(decode_base64(b"SGVsbG8s\r\nIHdvcmxk"), b"Hello, world");
        assert_eq!(decode_base64(b"aGk="), b"hi");
        assert_eq!(decode_qp(b"caf=C3=A9 =\r\nau lait", false), "caf\u{e9} au lait".as_bytes());
        assert_eq!(decode_qp(b"a_b=20c", true), b"a b c");
    }

    #[test]
    fn encoded_words_in_headers_are_read_and_run_together() {
        assert_eq!(decode_words("=?UTF-8?B?Q2Fmw6k=?= menu"), "Caf\u{e9} menu");
        assert_eq!(decode_words("=?iso-8859-1?Q?Andr=E9_?= =?iso-8859-1?Q?Lopez?="), "Andr\u{e9} Lopez");
        assert_eq!(decode_words("plain subject"), "plain subject");
        assert_eq!(decode_words("na\u{ef}ve =?utf-8?Q?=E2=9C=93?="), "na\u{ef}ve \u{2713}");
    }

    #[test]
    fn dates_become_iso() {
        assert_eq!(iso_date("Tue, 3 Nov 2026 14:05:00 +0000").as_deref(), Some("2026-11-03"));
        assert_eq!(iso_date("3 Nov 26 14:05 GMT").as_deref(), Some("2026-11-03"));
        assert_eq!(iso_date("not a date"), None);
    }

    #[test]
    fn names_come_out_of_address_headers() {
        assert_eq!(display_name("Maria Lopez <maria@example.org>"), "Maria Lopez");
        assert_eq!(display_name("\"Lopez, Maria\" <m@example.org>"), "Lopez, Maria");
        assert_eq!(display_name("<m@example.org>"), "m@example.org");
    }

    #[test]
    fn parameters_are_found_inside_quotes() {
        let v = "multipart/mixed; boundary=\"a;b\"; charset=utf-8";
        assert_eq!(parameter(v, "boundary").as_deref(), Some("a;b"));
        assert_eq!(parameter(v, "charset").as_deref(), Some("utf-8"));
    }
}
