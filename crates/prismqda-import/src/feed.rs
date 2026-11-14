//! News feed import: RSS and Atom files that were saved from a feed address. Each item becomes one source and one case
//! whose variables hold its title, date, author, link, and the feed it came from. Nothing is fetched from the internet.

use prismqda_engine::{AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportWarning, ImportedCase, Level, RunKind, SourceType, UnitKind};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::builder::TextBuilder;
use crate::email::iso_date;
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;
use crate::segment::units_for;
use crate::text::{clean, decode, parse_html};

const MAX_DEPTH: usize = 200;
const MAX_ITEMS: usize = 20_000;

#[derive(Debug, Default)]
struct Item {
    title: String,
    link: String,
    published: String,
    author: String,
    description: String,
    content: String,
}

fn attribute(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.local_name().as_ref() == name.as_bytes()).and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.into_owned()))
}

/// "2026-11-03T10:00:00Z" or "Tue, 3 Nov 2026 10:00:00 GMT" to "2026-11-03".
fn feed_date(value: &str) -> String {
    let v = value.trim();
    let digits: Vec<char> = v.chars().collect();
    if digits.len() >= 10 && digits[4] == '-' && digits[7] == '-' && digits[..4].iter().all(char::is_ascii_digit) {
        return digits[..10].iter().collect();
    }
    iso_date(v).unwrap_or_default()
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

/// Reads an RSS 2.0, RSS 1.0 (RDF), or Atom file.
pub fn parse_feed(bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let (text, warning) = decode(bytes);
    let mut reader = Reader::from_str(&text);
    let mut path: Vec<String> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    let mut current: Option<Item> = None;
    let mut feed_title = String::new();
    let mut root: Option<String> = None;
    // The text being collected for the field the reader is inside, keyed by the path from the item down.
    let mut buffer = String::new();

    loop {
        let event = reader.read_event().map_err(|e| ImportError::format(format!("this file is not well-formed XML ({e}), so it cannot be read as a news feed")))?;
        match event {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if root.is_none() {
                    root = Some(name.clone());
                }
                if path.len() >= MAX_DEPTH {
                    return Err(ImportError::format("the feed nests its elements too deeply, so it is damaged or hostile"));
                }
                if (name == "item" || name == "entry") && current.is_none() {
                    current = Some(Item::default());
                }
                path.push(name);
                buffer.clear();
            }
            Event::Empty(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if root.is_none() {
                    root = Some(name.clone());
                }
                // Atom: <link href="..." rel="alternate"/>
                if name == "link" {
                    if let (Some(item), Some(href)) = (current.as_mut(), attribute(&e, "href")) {
                        let rel = attribute(&e, "rel").unwrap_or_else(|| "alternate".to_string());
                        if rel == "alternate" && item.link.is_empty() {
                            item.link = href;
                        }
                    }
                }
            }
            Event::Text(t) => {
                if let Ok(s) = t.decode() {
                    buffer.push_str(&s);
                }
            }
            Event::CData(c) => {
                if let Ok(s) = c.decode() {
                    buffer.push_str(&s);
                }
            }
            Event::GeneralRef(r) => match r.resolve_char_ref() {
                Ok(Some(ch)) => buffer.push(ch),
                Ok(None) => {
                    if let Ok(name) = r.decode() {
                        match name.as_ref() {
                            "amp" => buffer.push('&'),
                            "lt" => buffer.push('<'),
                            "gt" => buffer.push('>'),
                            "quot" => buffer.push('"'),
                            "apos" => buffer.push('\''),
                            // A custom entity is never expanded: that is how "billion laughs" files attack.
                            _ => {}
                        }
                    }
                }
                Err(_) => {}
            },
            Event::End(_) => {
                let Some(name) = path.pop() else { continue };
                let value = std::mem::take(&mut buffer);
                let parent = path.last().cloned().unwrap_or_default();
                if let Some(item) = current.as_mut() {
                    // Fields are direct children of the item; an Atom author's name is one level deeper.
                    let value = value.trim().to_string();
                    match (name.as_str(), parent.as_str()) {
                        ("title", "item" | "entry") => item.title = value,
                        ("link", "item") if item.link.is_empty() => item.link = value,
                        ("pubDate" | "published" | "date", "item" | "entry") if item.published.is_empty() => item.published = value,
                        ("updated", "entry") if item.published.is_empty() => item.published = value,
                        ("creator", "item") | ("author", "item") if item.author.is_empty() => item.author = value,
                        ("name", "author") if item.author.is_empty() => item.author = value,
                        ("description", "item") | ("summary", "entry") => item.description = value,
                        ("encoded", "item") | ("content", "entry") => item.content = value,
                        _ => {}
                    }
                    if (name == "item" || name == "entry") && !path.is_empty() {
                        if let Some(done) = current.take() {
                            items.push(done);
                            if items.len() > MAX_ITEMS {
                                return Err(ImportError::unsupported(format!("this feed has more than {MAX_ITEMS} items; PrismQDA imports up to that many at a time")));
                            }
                        }
                    }
                } else if name == "title" && feed_title.is_empty() && matches!(parent.as_str(), "channel" | "feed" | "RDF") {
                    feed_title = value.trim().to_string();
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    match root.as_deref() {
        Some("rss" | "feed" | "RDF") => {}
        Some(other) => {
            return Err(ImportError::unsupported(format!("this XML file starts with <{other}>, not <rss> or <feed>, so it is not a news feed PrismQDA can read")));
        }
        None => return Err(ImportError::format("this file has no XML in it")),
    }
    if items.is_empty() {
        return Err(ImportError::format("the feed has no items in it"));
    }

    let folder = if opts.folder.is_empty() { "News".to_string() } else { opts.folder.clone() };
    let mut batch = ImportBatch::default();
    let mut specs: Vec<AttributeSpec> = Vec::new();
    for (n, item) in items.iter().enumerate() {
        let raw_body = if !item.content.trim().is_empty() { &item.content } else { &item.description };
        let body = if raw_body.contains('<') { parse_html(raw_body.as_bytes(), opts).map(|p| p.text).unwrap_or_default() } else { clean(raw_body) };
        let date = feed_date(&item.published);
        let mut b = TextBuilder::new();
        if !item.title.is_empty() {
            b.push_run(&item.title, RunKind::Heading, Some(1));
            b.paragraph_break();
        }
        let mut lines: Vec<String> = Vec::new();
        if !item.author.is_empty() {
            lines.push(format!("Author: {}", item.author));
        }
        if !date.is_empty() {
            lines.push(format!("Published: {date}"));
        }
        if !feed_title.is_empty() {
            lines.push(format!("Feed: {feed_title}"));
        }
        if !item.link.is_empty() {
            lines.push(format!("Link: {}", item.link));
        }
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                b.line_break();
            }
            b.push(l);
        }
        if !body.trim().is_empty() {
            b.paragraph_break();
            b.push(body.trim());
        }
        let parsed = b.finish(SourceType::Document, UnitKind::Paragraph);
        if parsed.text.trim().is_empty() {
            continue;
        }
        let name = {
            let title = if item.title.is_empty() { format!("Item {}", n + 1) } else { shorten(&item.title, 80) };
            if date.is_empty() { title } else { format!("{title} \u{2013} {date}") }
        };
        let mut warnings = Vec::new();
        if let Some(w) = &warning {
            warnings.push(w.clone());
        }
        if body.trim().is_empty() {
            warnings.push(ImportWarning::new("no_text", "This item has only a title; the feed gave no text for it. Feeds often carry a summary instead of the whole article."));
        }
        let kind = opts.unit_kind.unwrap_or(UnitKind::Paragraph);
        let units = units_for(kind, &parsed.text);
        batch.sources.push(prismqda_engine::ImportedSource {
            name: name.clone(),
            source_type: opts.source_type.unwrap_or(SourceType::Document),
            folder: folder.clone(),
            text: parsed.text,
            runs: parsed.runs,
            units,
            default_unit_kind: kind,
            unit_attributes: Vec::new(),
            original_file_hash: opts.file_hash.clone(),
            settings: serde_json::json!({ "format": "feed", "item": n + 1 }),
            warnings,
        });
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut add = |attr: &str, level: Level, value: String| {
            if !value.is_empty() {
                if !specs.iter().any(|s| s.name == attr) {
                    specs.push(AttributeSpec::new(attr, AppliesTo::Case, level));
                }
                attrs.push((attr.to_string(), value));
            }
        };
        add("Title", Level::Text, item.title.clone());
        add("Published", Level::Date, date);
        add("Author", Level::Text, item.author.clone());
        add("Link", Level::Text, item.link.clone());
        add("Feed", Level::Nominal, feed_title.clone());
        batch.cases.push(ImportedCase { name, attrs, members: vec![CaseMember { source: batch.sources.len() - 1, unit: None }] });
    }
    if batch.sources.is_empty() {
        return Err(ImportError::format("none of the items in the feed has a title or text to import"));
    }
    batch.case_attributes = specs;
    Ok(batch)
}
