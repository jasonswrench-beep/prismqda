//! Reading a REFI-QDA project (`.qdpx`) into a new PrismQDA project (decision 159).
//!
//! The file is read as a tree and only what PrismQDA has a place for is taken; anything else is counted and named in the report, as the
//! standard asks of a program that imports. Nothing in the file is run or fetched, a file name inside the archive is never used as a path on
//! disk, and an entity the file defines is never expanded.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::Path;

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, ImportBatch, ImportedCase, ImportedSource, ImportedUnit, Level, MemoKind, MemoScope, NewCode, NewCoder, NewCoding, NewMemo, NewProject, NewRegion, OpenOptions, Origin, Project, Shape,
    SourceType, UnitKind,
};
use prismqda_import::options::ImportOptions;
use prismqda_import::segment::units_for;
use prismqda_import::{open_bytes, Opened};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use unicode_normalization::{is_nfc, UnicodeNormalization};

use crate::qdpx::QdpxReport;
use crate::ExportError;

pub(crate) const MAX_DEPTH: usize = 200;
/// The most a single file inside the archive may unpack to. The standard allows 2 GB; a project this size would not fit in memory.
const MAX_ENTRY: u64 = 1 << 30;
const MAX_XML: u64 = 256 << 20;

pub(crate) fn bad(message: impl Into<String>) -> ExportError {
    ExportError::Invalid(message.into())
}

// ---------------------------------------------------------------- the XML tree

#[derive(Debug, Default)]
pub(crate) struct Node {
    pub(crate) name: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) children: Vec<Node>,
    pub(crate) text: String,
}

impl Node {
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    pub(crate) fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
    pub(crate) fn kid(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn target(&self) -> Option<&str> {
        self.attr("targetGUID")
    }
}

fn start_node(e: &BytesStart) -> Node {
    let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
    let attrs = e
        .attributes()
        .flatten()
        .filter_map(|a| {
            let key = String::from_utf8_lossy(a.key.local_name().as_ref()).to_string();
            // `xmlns` declarations are not data.
            if a.key.as_ref().starts_with(b"xmlns") {
                return None;
            }
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| (key, v.into_owned()))
        })
        .collect();
    Node { name, attrs, children: Vec::new(), text: String::new() }
}

/// `what` names the file in the messages ("project.qde" inside a QDPX, "this file" for a codebook on its own).
pub(crate) fn parse_tree(xml: &str, what: &str) -> Result<Node, ExportError> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    loop {
        let event = reader.read_event().map_err(|e| bad(format!("{what} is not well-formed XML ({e})")))?;
        match event {
            Event::Start(e) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(bad(format!("{what} nests its elements too deeply, so it is damaged or hostile")));
                }
                stack.push(start_node(&e));
            }
            Event::Empty(e) => {
                let node = start_node(&e);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::End(_) => {
                if let Some(node) = stack.pop() {
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => root = Some(node),
                    }
                }
            }
            Event::Text(t) => {
                if let (Some(top), Ok(s)) = (stack.last_mut(), t.decode()) {
                    top.text.push_str(&s);
                }
            }
            Event::CData(c) => {
                if let (Some(top), Ok(s)) = (stack.last_mut(), c.decode()) {
                    top.text.push_str(&s);
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut() {
                    match r.resolve_char_ref() {
                        Ok(Some(ch)) => top.text.push(ch),
                        Ok(None) => {
                            if let Ok(name) = r.decode() {
                                match name.as_ref() {
                                    "amp" => top.text.push('&'),
                                    "lt" => top.text.push('<'),
                                    "gt" => top.text.push('>'),
                                    "quot" => top.text.push('"'),
                                    "apos" => top.text.push('\''),
                                    // A custom entity is never expanded: that is how "billion laughs" files attack.
                                    _ => {}
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| bad(format!("{what} holds no XML (it is empty, or it is not XML)")))
}

// ---------------------------------------------------------------- the archive

struct Archive<'a> {
    zip: zip::ZipArchive<std::io::Cursor<&'a [u8]>>,
    /// The lowercase name of each file in the `sources` folder, and its number in the archive.
    sources: HashMap<String, usize>,
}

impl<'a> Archive<'a> {
    fn open(bytes: &'a [u8]) -> Result<(Self, String), ExportError> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| bad("This is not a QDPX file: it is not a ZIP archive."))?;
        let mut qde: Option<usize> = None;
        // A project file under another name: ELAN 7.1 writes `<project name>.qde`, not `project.qde`. The one nearest the top is taken
        // when there is no `project.qde`.
        let mut other: Option<(usize, usize)> = None;
        let mut sources = HashMap::new();
        for i in 0..zip.len() {
            let Ok(entry) = zip.by_index_raw(i) else { continue };
            if entry.is_dir() {
                continue;
            }
            // Since zip 9 a name that cannot be read comes back as an error; such an entry is left out like a damaged one.
            let Ok(name) = entry.name() else { continue };
            let name = name.replace('\\', "/").to_lowercase();
            if name == "project.qde" || name.ends_with("/project.qde") {
                if qde.is_none() {
                    qde = Some(i);
                }
            } else if name.ends_with(".qde") {
                let depth = name.matches('/').count();
                if other.is_none_or(|(d, _)| depth < d) {
                    other = Some((depth, i));
                }
            } else if let Some((_, file)) = name.rsplit_once("sources/") {
                sources.insert(file.to_string(), i);
            }
        }
        let mut archive = Archive { zip, sources };
        let index = qde.or(other.map(|(_, i)| i)).ok_or_else(|| bad("This is not a QDPX file: the archive holds no project file (project.qde)."))?;
        let xml = archive.read_index(index, MAX_XML)?;
        let xml = String::from_utf8_lossy(xml.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&xml)).into_owned();
        Ok((archive, xml))
    }

    fn read_index(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, ExportError> {
        let mut entry = self.zip.by_index(index).map_err(|e| bad(format!("a file in the archive cannot be read ({e})")))?;
        let mut out = Vec::new();
        entry.by_ref().take(limit + 1).read_to_end(&mut out).map_err(|e| bad(format!("a file in the archive cannot be read ({e})")))?;
        if out.len() as u64 > limit {
            return Err(bad(format!("a file in the archive is larger than {} MB, so it is damaged or not a project", limit >> 20)));
        }
        Ok(out)
    }

    /// The bytes of a source file named by a `path`, `plainTextPath`, or `richTextPath` attribute. `None` for a file that is not in the archive
    /// (an external file, a missing one).
    fn file(&mut self, path: &str) -> Option<(String, Vec<u8>)> {
        let name = path.strip_prefix("internal://")?;
        let name = name.rsplit('/').next().unwrap_or(name);
        let index = *self.sources.get(&name.to_lowercase())?;
        let bytes = self.read_index(index, MAX_ENTRY).ok()?;
        Some((name.to_string(), bytes))
    }
}

// ---------------------------------------------------------------- the import

#[derive(Default)]
struct Left {
    /// Recordings that could not be found on this computer, by the name of their document.
    media_missing: Vec<String>,
    /// The codings on stretches of those recordings, which have nothing to be on.
    media_selections: usize,
    /// What a transcript has that is not read: the sync points (the times of its lines) and the selections made between them.
    sync_points: usize,
    transcript_selections: usize,
    pdf_areas: usize,
    links: usize,
    /// Links joined to the first coding of words that carry several codings (the format joins selections, not codings).
    links_shared: usize,
    /// Boxes of a graph that stand for something that did not come in (a document, a code that was left out), and are free notes.
    graph_notes: usize,
    /// Lines of a graph that join a box to itself, to a box that is not there, or to what they already join.
    graph_edges: usize,
    set_members: usize,
    source_values: usize,
    unplaced: usize,
    clamped: usize,
    no_text: Vec<String>,
    unreadable: Vec<String>,
    renamed_codes: usize,
    normalized: usize,
    unnormalized: Vec<String>,
    unit_members: usize,
    case_notes: usize,
}

/// One coding to make once the sources are stored.
struct PendingCoding {
    guid: String,
    source: usize,
    start: i64,
    end: i64,
    code: String,
    user: Option<String>,
    rationale: String,
}

struct PendingRegion {
    /// The guid of the picture selection it came from, so a link to it can be found; empty for the whole picture.
    selection: String,
    source: usize,
    rect: (f64, f64, f64, f64),
    /// The page, counted from 1 (a picture has one page); and whether the area is on a PDF.
    page: i64,
    on_pdf: bool,
    code: String,
    user: Option<String>,
    rationale: String,
}

/// A text, as read, with the way its offsets move if it had to be put in NFC.
struct ReadText {
    text: String,
    /// Where each boundary that a selection uses lands in the normalized text; empty when the text was already NFC.
    moved: BTreeMap<usize, usize>,
    changed: bool,
}

/// The units of a transcript made from its sync points: one from each sync point to the next, with the times of the two (the last has the time
/// of its own point only), and one before the first point when the text starts earlier. Also where each sync point is, by its GUID, after
/// `place` has moved the positions for a change to the text.
fn sync_units(sync: &[(String, i64, Option<i64>)], place: impl Fn(i64) -> Option<i64>, length: i64) -> (Vec<ImportedUnit>, HashMap<String, i64>) {
    let mut points: Vec<(i64, Option<i64>)> = Vec::new();
    let mut by_guid: HashMap<String, i64> = HashMap::new();
    for (guid, pos, ms) in sync {
        let Some(p) = place(*pos).filter(|p| *p >= 0 && *p <= length) else { continue };
        by_guid.insert(guid.clone(), p);
        points.push((p, *ms));
    }
    points.sort_by_key(|(p, _)| *p);
    points.dedup_by_key(|(p, _)| *p);
    let mut units: Vec<ImportedUnit> = Vec::new();
    if let Some(&(first, _)) = points.first() {
        if first > 0 {
            units.push(ImportedUnit::new(0, first, UnitKind::SpeakerTurn));
        }
    }
    for (i, &(start, ms)) in points.iter().enumerate() {
        let (end, next_ms) = match points.get(i + 1) {
            Some(&(e, m)) => (e, m),
            None => (length, None),
        };
        if end <= start {
            continue;
        }
        let mut unit = ImportedUnit::new(start, end, UnitKind::SpeakerTurn);
        if let Some(from) = ms {
            unit.time_ms = Some((from, next_ms.unwrap_or(from).max(from)));
        }
        units.push(unit);
    }
    (units, by_guid)
}

/// The kind and the units of a document, as PrismQDA writes them in its description.
struct UnitMap {
    source_type: SourceType,
    default_kind: UnitKind,
    units: Vec<UnitSpot>,
}

struct UnitSpot {
    start: i64,
    end: i64,
    kind: UnitKind,
    speaker: Option<String>,
    excluded: bool,
    time_ms: Option<(i64, i64)>,
}

/// Reads the kind and the units of a document from the description of its text source, when PrismQDA wrote it there.
fn unit_map_of(node: &Node) -> Option<UnitMap> {
    let d = node.kid("Description")?.text.as_str();
    let at = d.find(crate::qdpx::UNIT_MAP_INTRO)? + crate::qdpx::UNIT_MAP_INTRO.len();
    let v: serde_json::Value = serde_json::from_str(d[at..].trim()).ok()?;
    let source_type = v.get("type")?.as_str()?.parse::<SourceType>().ok()?;
    let default_kind = v.get("unit")?.as_str()?.parse::<UnitKind>().ok()?;
    let mut units = Vec::new();
    for row in v.get("units")?.as_array()? {
        let r = row.as_array()?;
        let time = match (r.get(5).and_then(|x| x.as_i64()), r.get(6).and_then(|x| x.as_i64())) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        };
        units.push(UnitSpot {
            start: r.first()?.as_i64()?,
            end: r.get(1)?.as_i64()?,
            kind: r.get(2)?.as_str()?.parse::<UnitKind>().ok()?,
            speaker: r.get(3).and_then(|x| x.as_str()).map(str::to_string),
            excluded: r.get(4).and_then(|x| x.as_i64()).unwrap_or(0) != 0,
            time_ms: time,
        });
    }
    Some(UnitMap { source_type, default_kind, units })
}

/// The GUID of the document a recording that PrismQDA wrote belongs to, from the sentence in its description.
fn media_marker(node: &Node) -> Option<String> {
    let d = node.kid("Description")?.text.as_str();
    let at = d.find("source GUID ")? + "source GUID ".len();
    let guid: String = d[at..].chars().take(36).collect();
    (guid.len() == 36 && guid.chars().all(|c| c.is_ascii_hexdigit() || c == '-')).then_some(guid)
}

/// What `%20` and the like in a path of the format stand for.
fn percent_decoded(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            // `get` is `None` when the two bytes after the `%` cut a character in two (a `%` before a letter outside ASCII), and the `%` is then only a `%`.
            if let Some(v) = s.get(i + 1..i + 3).filter(|h| h.bytes().all(|c| c.is_ascii_hexdigit())).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A path of the format (`absolute:///C:/folder/file.mp3`) as a path on this computer.
fn local_path(rest: &str) -> String {
    let p = percent_decoded(rest);
    // `/C:/x` is `C:\x` on Windows
    let p = match p.as_bytes() {
        [b'/', d, b':', ..] if d.is_ascii_alphabetic() => p[1..].to_string(),
        _ => p,
    };
    if cfg!(windows) {
        p.replace('/', "\\")
    } else {
        p
    }
}

/// Where the recording of an audio or video source is on this computer: a file the file names that is there, or one inside the archive, which
/// is written beside the new project (in a folder named for it). `None` when it cannot be found.
fn find_recording(node: &Node, archive: &mut Archive, base: &str, project_path: &Path) -> Option<String> {
    for key in ["currentPath", "path"] {
        let Some(raw) = node.attr(key).map(str::trim).filter(|p| !p.is_empty()) else { continue };
        if raw.starts_with("internal://") {
            let Some((file_name, bytes)) = archive.file(raw) else { continue };
            let stem = project_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "project".to_string());
            let folder = project_path.parent().unwrap_or_else(|| Path::new(".")).join(format!("{stem} recordings"));
            if std::fs::create_dir_all(&folder).is_err() {
                continue;
            }
            let clean = Path::new(&file_name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "recording".to_string());
            let mut target = folder.join(&clean);
            let mut k = 2;
            while target.exists() {
                target = folder.join(format!("{k} {clean}"));
                k += 1;
            }
            if std::fs::write(&target, &bytes).is_err() {
                continue;
            }
            return Some(target.to_string_lossy().to_string());
        }
        let path = if let Some(rest) = raw.strip_prefix("absolute://") {
            local_path(rest)
        } else if let Some(rest) = raw.strip_prefix("relative://") {
            let joined = Path::new(&local_path(base)).join(local_path(rest.trim_start_matches('/')));
            joined.to_string_lossy().to_string()
        } else {
            raw.to_string()
        };
        // A path written in the file may name a network address (`\\server\share\talk.mp3`); asking whether it exists would connect to that
        // server and offer this computer's sign-in. Only a place on a drive with a letter is looked at; anything else is a recording that
        // is not found, and the disk is not touched.
        if !prismqda_engine::paths::is_local_drive_path(&path) {
            continue;
        }
        let p = Path::new(&path);
        if p.is_absolute() && p.is_file() {
            return Some(path);
        }
    }
    None
}

fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Puts the text in NFC. The offsets in the file count the text as it was written, so each place a selection starts or ends is followed
/// through the change.
fn normalize(text: String, boundaries: &BTreeSet<usize>, left: &mut Left, name: &str) -> ReadText {
    if is_nfc(&text) {
        return ReadText { text, moved: BTreeMap::new(), changed: false };
    }
    let chars: Vec<char> = text.chars().collect();
    let mut cuts: Vec<usize> = boundaries.iter().copied().filter(|b| *b <= chars.len()).collect();
    cuts.push(0);
    cuts.push(chars.len());
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = String::new();
    let mut count = 0usize;
    let mut moved = BTreeMap::new();
    for w in cuts.windows(2) {
        moved.insert(w[0], count);
        let piece: String = chars[w[0]..w[1]].iter().collect::<String>().nfc().collect();
        count += piece.chars().count();
        out.push_str(&piece);
    }
    moved.insert(chars.len(), count);
    if !is_nfc(&out) {
        left.unnormalized.push(name.to_string());
        return ReadText { text: out.nfc().collect(), moved: BTreeMap::new(), changed: true };
    }
    left.normalized += 1;
    ReadText { text: out, moved, changed: true }
}

pub(crate) fn parse_color(c: Option<&str>) -> Option<String> {
    let c = c?.trim();
    let hex = c.strip_prefix('#')?;
    if !hex.chars().all(|x| x.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => Some(format!("#{}", hex.to_lowercase())),
        3 => Some(format!("#{}", hex.chars().flat_map(|x| [x, x]).collect::<String>().to_lowercase())),
        _ => None,
    }
}

pub(crate) fn split_description(d: &str) -> (String, String, String) {
    let (mut definition, mut include, mut exclude) = (Vec::new(), String::new(), String::new());
    for line in d.lines() {
        if let Some(v) = line.strip_prefix("Do not include when:") {
            exclude = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Include when:") {
            include = v.trim().to_string();
        } else {
            definition.push(line);
        }
    }
    (definition.join("\n").trim().to_string(), include, exclude)
}

fn level_of(kind: &str) -> Level {
    match kind {
        "Integer" | "Float" => Level::Ratio,
        "Date" | "DateTime" => Level::Date,
        "Boolean" => Level::Nominal,
        _ => Level::Text,
    }
}

fn value_of(n: &Node) -> Option<String> {
    for name in ["TextValue", "BooleanValue", "IntegerValue", "FloatValue", "DateValue", "DateTimeValue"] {
        if let Some(v) = n.kid(name) {
            let t = v.text.trim();
            return (!t.is_empty()).then(|| t.to_string());
        }
    }
    None
}

/// Reads a `.qdpx` file into the new project at `project_path`. Returns what was taken and what was not.
pub fn import_qdpx(qdpx: &[u8], project_path: &Path, name: Option<&str>) -> Result<QdpxReport, ExportError> {
    let (mut archive, xml) = Archive::open(qdpx)?;
    let root = parse_tree(&xml, "project.qde")?;
    if root.name != "Project" {
        return Err(bad(format!("This is not a QDPX project: project.qde starts with <{}>, not <Project>.", root.name)));
    }
    let mut left = Left::default();
    let mut report = QdpxReport::default();
    let qualcoder_origin = root.attr("origin").is_some_and(|o| o.trim_start().to_ascii_lowercase().starts_with("qualcoder"));
    let mut qualcoder_line_ends = 0usize;
    let project_name = name.map(str::to_string).or_else(|| root.attr("name").map(str::to_string)).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Imported project".to_string());

    // ---- the sources: read, and find every place a selection starts or ends, so the text can be put in NFC without moving a coding
    struct ReadSource<'n> {
        node: &'n Node,
        kind: &'static str,
        guid: String,
        name: String,
    }
    let mut read: Vec<ReadSource> = Vec::new();
    if let Some(sources) = root.kid("Sources") {
        for s in &sources.children {
            let kind = match s.name.as_str() {
                "TextSource" => "text",
                "PictureSource" => "picture",
                "PDFSource" => "pdf",
                "AudioSource" | "VideoSource" => "media",
                _ => continue,
            };
            let guid = s.attr("guid").unwrap_or("").to_string();
            let mut name = s.attr("name").map(str::to_string).unwrap_or_else(|| "Untitled".to_string());
            // ELAN 7.1 names a recording by the last part of its address as the annotation file wrote it, so a space is `%20` and so on in the
            // name as well as in the path. A name that is the last part of the source's own path is read as the address it is.
            if matches!(kind, "picture" | "pdf" | "media") && name.contains('%') {
                let last = |key: &str| s.attr(key).and_then(|p| p.rsplit('/').next()).map(str::to_string);
                if last("path").as_deref() == Some(name.as_str()) || last("currentPath").as_deref() == Some(name.as_str()) {
                    name = percent_decoded(&name);
                }
            }
            // PrismQDA names a picture, a PDF, or a recording without the ending of its file, and a file from it has the ending on the name
            // (QualCoder 4.0 builds the name of the stored file from it): an ending that is the file's own is taken off again.
            if matches!(kind, "picture" | "pdf" | "media") {
                if let Some(ext) = s.attr("path").and_then(|p| std::path::Path::new(p).extension()).and_then(|e| e.to_str()) {
                    let suffix = format!(".{ext}");
                    if name.len() > suffix.len() && name.is_char_boundary(name.len() - suffix.len()) && name[name.len() - suffix.len()..].eq_ignore_ascii_case(&suffix) {
                        name.truncate(name.len() - suffix.len());
                    }
                }
            }
            read.push(ReadSource { node: s, kind, guid, name });
        }
    }

    // A text source that names the same text file as the transcript of a recording is that transcript again: ELAN writes the codings of the
    // words of a transcript as a second source on the file the recording's transcript is made of. It is not made a second document; its
    // selections are read as the transcript's own (the places mean the same, the file being the same).
    let transcript_file = |p: &str| p.strip_prefix("internal://").map(|n| n.rsplit('/').next().unwrap_or(n).to_lowercase());
    let mut same_text_as: HashMap<usize, Vec<&Node>> = HashMap::new();
    let mut second_text_source: BTreeSet<usize> = BTreeSet::new();
    for (ti, ts) in read.iter().enumerate().filter(|(_, r)| r.kind == "text") {
        let Some(file) = ts.node.attr("plainTextPath").and_then(transcript_file) else { continue };
        let owner = read.iter().position(|m| m.kind == "media" && m.node.kids("Transcript").any(|t| t.attr("plainTextPath").and_then(transcript_file).as_deref() == Some(file.as_str())));
        if let Some(mi) = owner {
            same_text_as.entry(mi).or_default().push(ts.node);
            second_text_source.insert(ti);
        }
    }

    let mut batch = ImportBatch::default();
    // guid of a source -> index in the batch
    let mut source_index: HashMap<String, usize> = HashMap::new();
    // guid of a text selection -> (index in the batch, start, end) after any normalization
    let mut selection_place: HashMap<String, (usize, i64, i64)> = HashMap::new();
    let mut pending: Vec<PendingCoding> = Vec::new();
    let mut regions: Vec<PendingRegion> = Vec::new();
    // recordings: ones that PrismQDA wrote name the document they belong to (they are attached to it), and any other is a document of its own
    let mut media_attach: Vec<(String, &Node)> = Vec::new();
    let mut media_docs: Vec<(usize, &Node, bool)> = Vec::new();
    // transcripts whose sync points gave the units of the document their times
    let mut timed_transcripts = 0usize;
    // documents whose kind and units were read from their description
    let mut unit_maps_read = 0usize;
    // values the file puts on documents: the document's number, the variable's GUID, and the value
    let mut doc_values: Vec<(usize, String, String)> = Vec::new();
    // For a link: the first coding made on each selection, and the first area made on each picture selection (as its negative number).
    let mut selection_first: HashMap<String, String> = HashMap::new();
    let mut selection_codings: HashMap<String, usize> = HashMap::new();
    let mut region_of: HashMap<String, i64> = HashMap::new();
    // notes referenced from a selection or a source, to find the thing they belong to later
    let mut note_homes: Vec<(String, NoteHome)> = Vec::new();

    #[derive(Clone)]
    enum NoteHome {
        Project,
        Source(usize),
        Code(String),
        Coding(String),
    }

    for (ri, rs) in read.iter().enumerate() {
        if second_text_source.contains(&ri) {
            continue;
        }
        match rs.kind {
            "media" => {
                let target = media_marker(rs.node).filter(|g| read.iter().any(|o| o.kind != "media" && o.guid == *g));
                if let Some(target) = target {
                    media_attach.push((target, rs.node));
                    continue;
                }
            }
            "text" | "pdf" | "picture" => {}
            _ => continue,
        }
        // The node that holds the text selections: the source itself, the representation of a PDF, or the description of a picture.
        let text_node: Option<&Node> = match rs.kind {
            "text" => Some(rs.node),
            "media" => rs.node.kid("Transcript"),
            "pdf" => rs.node.kid("Representation"),
            _ => rs.node.kid("TextDescription"),
        };
        let mut plain: Option<String> = None;
        if let Some(tn) = text_node {
            if let Some(p) = tn.attr("plainTextPath") {
                plain = archive.file(p).map(|(_, b)| decode_text(&b));
            }
            if plain.is_none() {
                plain = tn.kid("PlainTextContent").map(|c| c.text.clone());
            }
        }
        // QualCoder 4.0 counts a line break as one character in every place it writes (it keeps its text with the break as one), but on
        // Windows it saves the text files with two (CR LF), so each place would be a character early for every break before it. Its text is
        // read with the breaks as it counted them. (Tried against its own export and import, 2026-11-12.)
        if qualcoder_origin && plain.as_deref().is_some_and(|t| t.contains("\r\n")) {
            plain = plain.map(|t| t.replace("\r\n", "\n"));
            qualcoder_line_ends += 1;
        }
        // A recording with no transcript is a document of its own: a few words about it, and the recording is what is coded.
        let mut stands_alone = false;
        if rs.kind == "media" && plain.as_deref().map(str::is_empty).unwrap_or(true) {
            let about = rs.node.kid("Description").map(|d| d.text.trim().to_string()).filter(|d| !d.is_empty());
            plain = Some(match about {
                Some(d) => format!("Recording: {}\n\n{d}", rs.name),
                None => format!("Recording: {}", rs.name),
            });
            stands_alone = true;
        }
        // The text selections of the document: those on its text node, and those of a second text source on the same file (see above).
        let second_sources: &[&Node] = same_text_as.get(&ri).map(Vec::as_slice).unwrap_or(&[]);
        let text_selections: Vec<&Node> = match text_node {
            Some(tn) => tn.kids("PlainTextSelection").chain(second_sources.iter().flat_map(|n| n.kids("PlainTextSelection"))).collect(),
            None => Vec::new(),
        };
        // the places in the text where a selection starts or ends
        let mut boundaries: BTreeSet<usize> = BTreeSet::new();
        for sel in &text_selections {
            for key in ["startPosition", "endPosition"] {
                if let Some(v) = sel.attr(key).and_then(|v| v.trim().parse::<i64>().ok()).filter(|v| *v >= 0) {
                    boundaries.insert(v as usize);
                }
            }
        }

        // The sync points of a transcript: where in its text (a position) each is, and when in the recording (in milliseconds).
        let mut sync: Vec<(String, i64, Option<i64>)> = Vec::new();
        if rs.kind == "media" {
            for tr in rs.node.kids("Transcript") {
                for sp in tr.kids("SyncPoint") {
                    match (sp.attr("guid"), sp.attr("position").and_then(|v| v.trim().parse::<i64>().ok()).filter(|v| *v >= 0)) {
                        (Some(g), Some(pos)) => {
                            sync.push((g.to_string(), pos, sp.attr("timeStamp").and_then(|v| v.trim().parse::<i64>().ok())));
                            boundaries.insert(pos as usize);
                        }
                        _ => left.sync_points += 1,
                    }
                }
            }
        }
        let mut sync_pos: HashMap<String, i64> = HashMap::new();
        // The kind and the units of a document that PrismQDA wrote: its description says where each unit is.
        let unit_map = if rs.kind == "text" { unit_map_of(rs.node) } else { None };
        if let Some(m) = &unit_map {
            for u in &m.units {
                boundaries.insert(u.start as usize);
                boundaries.insert(u.end as usize);
            }
        }

        // The source in the batch: a picture or PDF through the importer for its kind, a document built from its text as it is.
        let mut imported: Option<(ImportedSource, Option<prismqda_engine::ImageAttachment>, Option<prismqda_engine::PdfAttachment>)> = None;
        // how the pages of a PDF sit in its file, to put its coded rectangles where they were drawn
        let mut pdf_geometry: Option<Vec<prismqda_pdfview::PageGeometry>> = None;
        // the codings on the rectangles of a PDF, for what cannot be placed when the PDF cannot be read
        let pdf_codings: usize = if rs.kind == "pdf" { rs.node.kids("PDFSelection").map(|s| s.kids("Coding").count()).sum() } else { 0 };
        match rs.kind {
            "picture" | "pdf" => {
                let Some(path) = rs.node.attr("path") else {
                    left.unreadable.push(format!("{} (it names no file)", rs.name));
                    left.pdf_areas += pdf_codings;
                    continue;
                };
                let Some((file_name, bytes)) = archive.file(path) else {
                    left.unreadable.push(format!("{} (its file is not inside the archive)", rs.name));
                    left.pdf_areas += pdf_codings;
                    continue;
                };
                let opts = ImportOptions { name: Some(rs.name.clone()), ..ImportOptions::default() };
                match open_bytes(&file_name, &bytes, &opts) {
                    Ok(Opened::Batch(b)) if !b.sources.is_empty() => {
                        let mut it = b.sources.into_iter();
                        let source = it.next().unwrap();
                        imported = Some((source, b.images.into_iter().next(), b.pdfs.into_iter().next()));
                        if rs.kind == "pdf" {
                            pdf_geometry = prismqda_pdfview::page_geometries(&bytes).ok();
                        }
                    }
                    Ok(_) => {
                        left.unreadable.push(format!("{} (its file gave no text)", rs.name));
                        left.pdf_areas += pdf_codings;
                        continue;
                    }
                    Err(e) => {
                        left.unreadable.push(format!("{} ({e})", rs.name));
                        left.pdf_areas += pdf_codings;
                        continue;
                    }
                }
            }
            _ => {}
        }
        let (mut source, image, pdf) = match imported {
            Some(x) => x,
            None => {
                let Some(text) = plain.clone().filter(|t| !t.is_empty()) else {
                    left.no_text.push(rs.name.clone());
                    continue;
                };
                let s = ImportedSource {
                    name: rs.name.clone(),
                    source_type: SourceType::Document,
                    folder: String::new(),
                    text,
                    runs: Vec::new(),
                    units: Vec::new(),
                    default_unit_kind: UnitKind::Sentence,
                    unit_attributes: Vec::new(),
                    original_file_hash: None,
                    settings: serde_json::json!({}),
                    warnings: Vec::new(),
                };
                (s, None, None)
            }
        };
        source.name = rs.name.clone();
        // A picture or PDF takes the text the file gave unless the QDPX gave one of its own: that is the text its selections are made on.
        let mut moved: Option<ReadText> = None;
        if let Some(p) = plain.filter(|t| !t.is_empty()) {
            let r = normalize(p, &boundaries, &mut left, &rs.name);
            source.text = r.text.clone();
            source.runs = Vec::new();
            source.units = units_for(UnitKind::Sentence, &source.text);
            source.default_unit_kind = UnitKind::Sentence;
            // The times of a transcript's lines make its units: one from each sync point to the next, with the times of the two.
            if !sync.is_empty() {
                let length = source.text.chars().count() as i64;
                let (units, places) = sync_units(&sync, |p| if r.changed { r.moved.get(&(p as usize)).map(|v| *v as i64) } else { Some(p) }, length);
                if !units.is_empty() {
                    source.units = units;
                    source.default_unit_kind = UnitKind::SpeakerTurn;
                    source.source_type = SourceType::Transcript;
                    timed_transcripts += 1;
                }
                sync_pos = places;
            }
            if let Some(m) = &unit_map {
                let length = source.text.chars().count() as i64;
                let place = |p: i64| if r.changed { r.moved.get(&(p as usize)).map(|v| *v as i64) } else { Some(p) };
                let mapped: Option<Vec<ImportedUnit>> = m
                    .units
                    .iter()
                    .map(|u| {
                        let (a, b) = (place(u.start)?, place(u.end)?);
                        (a >= 0 && a < b && b <= length).then(|| {
                            let mut unit = ImportedUnit::new(a, b, u.kind);
                            unit.speaker = u.speaker.clone();
                            unit.excluded = u.excluded;
                            unit.time_ms = u.time_ms;
                            unit
                        })
                    })
                    .collect();
                // Only units that lie in order and do not overlap are taken; anything else keeps the sentences.
                if let Some(units) = mapped.filter(|us| !us.is_empty() && us.windows(2).all(|w| w[0].end <= w[1].start)) {
                    source.units = units;
                    source.default_unit_kind = m.default_kind;
                    source.source_type = m.source_type;
                    unit_maps_read += 1;
                }
            }
            moved = Some(r);
        } else if text_node.is_some() && rs.kind == "text" {
            continue;
        }
        source.settings = serde_json::json!({ "format": "qdpx", "kind": rs.node.name });
        let index = batch.sources.len();
        let length = source.text.chars().count() as i64;
        batch.sources.push(source);
        if let Some(mut i) = image {
            i.source = index;
            batch.images.push(i);
        }
        if let Some(mut p) = pdf {
            p.source = index;
            batch.pdfs.push(p);
        }
        source_index.insert(rs.guid.clone(), index);
        if rs.kind == "media" {
            media_docs.push((index, rs.node, stands_alone));
        }

        // text selections and their codings
        if let Some(tn) = text_node {
            let at = |p: i64| -> Option<i64> {
                match &moved {
                    Some(m) if m.changed => m.moved.get(&(p as usize)).map(|v| *v as i64),
                    _ => Some(p),
                }
            };
            for sel in text_selections {
                let sg = sel.attr("guid").unwrap_or("").to_string();
                let (Some(a), Some(b)) = (sel.attr("startPosition").and_then(|v| v.trim().parse::<i64>().ok()), sel.attr("endPosition").and_then(|v| v.trim().parse::<i64>().ok())) else {
                    left.unplaced += sel.kids("Coding").count().max(1);
                    continue;
                };
                let (Some(mut start), Some(mut end)) = (at(a), at(b)) else {
                    left.unplaced += sel.kids("Coding").count().max(1);
                    continue;
                };
                if end > length {
                    end = length;
                    left.clamped += 1;
                }
                if start < 0 || start >= end {
                    start = start.max(0);
                    if start >= end {
                        left.unplaced += sel.kids("Coding").count().max(1);
                        continue;
                    }
                }
                if !sg.is_empty() {
                    selection_place.insert(sg.clone(), (index, start, end));
                }
                let rationale = sel.kid("Description").map(|d| d.text.trim().to_string()).unwrap_or_default();
                for c in sel.kids("Coding") {
                    let Some(code) = c.kid("CodeRef").and_then(Node::target) else { continue };
                    let cg = c.attr("guid").unwrap_or("").to_string();
                    for nref in c.kids("NoteRef") {
                        if let Some(t) = nref.target() {
                            note_homes.push((t.to_string(), NoteHome::Coding(cg.clone())));
                        }
                    }
                    if !sg.is_empty() {
                        selection_first.entry(sg.clone()).or_insert_with(|| cg.clone());
                        *selection_codings.entry(sg.clone()).or_insert(0) += 1;
                    }
                    pending.push(PendingCoding { guid: cg, source: index, start, end, code: code.to_string(), user: c.attr("creatingUser").map(str::to_string), rationale: rationale.clone() });
                }
                for nref in sel.kids("NoteRef") {
                    if let (Some(t), Some(first)) = (nref.target(), sel.kid("Coding").and_then(|c| c.attr("guid"))) {
                        note_homes.push((t.to_string(), NoteHome::Coding(first.to_string())));
                    } else if let Some(t) = nref.target() {
                        note_homes.push((t.to_string(), NoteHome::Project));
                    }
                }
            }
            // the selections of a transcript lie between two sync points; their codings are codings of the words between
            if rs.kind == "media" {
                for tr in rs.node.kids("Transcript") {
                    for sel in tr.kids("TranscriptSelection") {
                        let sg = sel.attr("guid").unwrap_or("").to_string();
                        let (Some(&a), Some(&b)) = (sel.attr("fromSyncPoint").and_then(|g| sync_pos.get(g)), sel.attr("toSyncPoint").and_then(|g| sync_pos.get(g))) else {
                            left.transcript_selections += 1;
                            continue;
                        };
                        let (start, end) = (a.min(b), a.max(b).min(length));
                        if start >= end {
                            left.transcript_selections += 1;
                            continue;
                        }
                        if !sg.is_empty() {
                            selection_place.insert(sg.clone(), (index, start, end));
                        }
                        let rationale = sel.kid("Description").map(|d| d.text.trim().to_string()).unwrap_or_default();
                        for c in sel.kids("Coding") {
                            let Some(code) = c.kid("CodeRef").and_then(Node::target) else { continue };
                            let cg = c.attr("guid").unwrap_or("").to_string();
                            if !sg.is_empty() {
                                selection_first.entry(sg.clone()).or_insert_with(|| cg.clone());
                                *selection_codings.entry(sg.clone()).or_insert(0) += 1;
                            }
                            pending.push(PendingCoding { guid: cg, source: index, start, end, code: code.to_string(), user: c.attr("creatingUser").map(str::to_string), rationale: rationale.clone() });
                        }
                    }
                }
            }
            // a coding of the whole text
            for c in rs.node.kids("Coding").chain(tn.kids("Coding")).chain(second_sources.iter().flat_map(|n| n.kids("Coding"))) {
                if let Some(code) = c.kid("CodeRef").and_then(Node::target) {
                    pending.push(PendingCoding { guid: c.attr("guid").unwrap_or("").to_string(), source: index, start: 0, end: length, code: code.to_string(), user: c.attr("creatingUser").map(str::to_string), rationale: String::new() });
                }
            }
        }
        // areas of a picture
        if rs.kind == "picture" {
            let (w, h) = batch.images.last().map(|i| (i.image.width as f64, i.image.height as f64)).unwrap_or((1.0, 1.0));
            for sel in rs.node.kids("PictureSelection") {
                let nums: Vec<f64> = ["firstX", "firstY", "secondX", "secondY"].iter().filter_map(|k| sel.attr(k).and_then(|v| v.trim().parse::<f64>().ok())).collect();
                if nums.len() != 4 {
                    left.unplaced += 1;
                    continue;
                }
                let (x0, x1) = (nums[0].min(nums[2]) / w, nums[0].max(nums[2]) / w);
                let (y0, y1) = (nums[1].min(nums[3]) / h, nums[1].max(nums[3]) / h);
                let (x0, y0, x1, y1) = (x0.clamp(0.0, 1.0), y0.clamp(0.0, 1.0), x1.clamp(0.0, 1.0), y1.clamp(0.0, 1.0));
                if x1 <= x0 || y1 <= y0 {
                    left.unplaced += 1;
                    continue;
                }
                let rationale = sel.kid("Description").map(|d| d.text.trim().to_string()).unwrap_or_default();
                for c in sel.kids("Coding") {
                    if let Some(code) = c.kid("CodeRef").and_then(Node::target) {
                        regions.push(PendingRegion { selection: sel.attr("guid").unwrap_or("").to_string(), source: index, rect: (x0, y0, x1, y1), page: 1, on_pdf: false, code: code.to_string(), user: c.attr("creatingUser").map(str::to_string), rationale: rationale.clone() });
                    }
                }
            }
            // a coding of the whole picture is the whole picture
            for c in rs.node.kids("Coding") {
                if let Some(code) = c.kid("CodeRef").and_then(Node::target) {
                    regions.push(PendingRegion { selection: String::new(), source: index, rect: (0.0, 0.0, 1.0, 1.0), page: 1, on_pdf: false, code: code.to_string(), user: c.attr("creatingUser").map(str::to_string), rationale: String::new() });
                }
            }
        }
        if rs.kind == "pdf" {
            // A rectangle of a page is in points from the bottom-left of the media box, with the page counted from 0; it is turned into the
            // fraction of the page as drawn, from the top-left, that PrismQDA keeps (decision 175).
            for sel in rs.node.kids("PDFSelection") {
                let n = sel.kids("Coding").count();
                let number = |k: &str| sel.attr(k).and_then(|v| v.trim().parse::<f64>().ok());
                let placed = match (sel.attr("page").and_then(|v| v.trim().parse::<usize>().ok()), number("firstX"), number("firstY"), number("secondX"), number("secondY"), &pdf_geometry) {
                    // QualCoder 4.0 writes the corners in points from the top-left of the page as drawn ("page already rotated"); the standard
                    // says from the bottom-left of the media box, which is how every other file is read.
                    (Some(page), Some(a), Some(b), Some(c), Some(d), Some(geometry)) if qualcoder_origin => geometry.get(page).map(|g| {
                        let size = g.drawn_size();
                        (page, (a / size.width, b / size.height), (c / size.width, d / size.height))
                    }),
                    (Some(page), Some(a), Some(b), Some(c), Some(d), Some(geometry)) => geometry.get(page).map(|g| (page, g.from_media_points(a, b), g.from_media_points(c, d))),
                    _ => None,
                };
                let Some((page, p, q)) = placed else {
                    left.pdf_areas += n;
                    continue;
                };
                let (x0, x1) = (p.0.min(q.0).clamp(0.0, 1.0), p.0.max(q.0).clamp(0.0, 1.0));
                let (y0, y1) = (p.1.min(q.1).clamp(0.0, 1.0), p.1.max(q.1).clamp(0.0, 1.0));
                if x1 - x0 < 1e-4 || y1 - y0 < 1e-4 {
                    left.pdf_areas += n;
                    continue;
                }
                let rationale = sel.kid("Description").map(|d| d.text.trim().to_string()).unwrap_or_default();
                for c in sel.kids("Coding") {
                    if let Some(code) = c.kid("CodeRef").and_then(Node::target) {
                        regions.push(PendingRegion { selection: sel.attr("guid").unwrap_or("").to_string(), source: index, rect: (x0, y0, x1, y1), page: page as i64 + 1, on_pdf: true, code: code.to_string(), user: c.attr("creatingUser").map(str::to_string), rationale: rationale.clone() });
                    }
                }
            }
        }
        for nref in rs.node.kids("NoteRef").chain(second_sources.iter().flat_map(|n| n.kids("NoteRef"))) {
            if let Some(t) = nref.target() {
                note_homes.push((t.to_string(), NoteHome::Source(index)));
            }
        }
        // what refers to the second text source on this file refers to the document
        for second in second_sources {
            if let Some(g) = second.attr("guid").filter(|g| !g.is_empty()) {
                source_index.insert(g.to_string(), index);
            }
        }
        for v in rs.node.kids("VariableValue") {
            match (v.kid("VariableRef").and_then(Node::target), value_of(v)) {
                (Some(g), Some(value)) => doc_values.push((index, g.to_string(), value)),
                _ => left.source_values += 1,
            }
        }
    }

    // ---- variables and cases
    let mut var_name: HashMap<String, String> = HashMap::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    if let Some(vars) = root.kid("Variables") {
        for v in vars.kids("Variable") {
            let guid = v.attr("guid").unwrap_or("").to_string();
            let mut n = v.attr("name").unwrap_or("Variable").trim().to_string();
            if n.is_empty() {
                n = "Variable".to_string();
            }
            let mut unique = n.clone();
            let mut k = 2;
            while !taken.insert(unique.to_lowercase()) {
                unique = format!("{n} ({k})");
                k += 1;
            }
            var_name.insert(guid, unique.clone());
            batch.case_attributes.push(AttributeSpec::new(unique, AppliesTo::Case, level_of(v.attr("typeOfVariable").unwrap_or("Text"))));
        }
    }
    let mut case_guid_note: Vec<String> = Vec::new();
    if let Some(cases) = root.kid("Cases") {
        for c in cases.kids("Case") {
            let mut case = ImportedCase { name: c.attr("name").unwrap_or("Case").trim().to_string(), attrs: Vec::new(), members: Vec::new() };
            if case.name.is_empty() {
                case.name = "Case".to_string();
            }
            for vv in c.kids("VariableValue") {
                if let (Some(var), Some(value)) = (vv.kid("VariableRef").and_then(Node::target).and_then(|g| var_name.get(g)), value_of(vv)) {
                    case.attrs.push((var.clone(), value));
                }
            }
            for sr in c.kids("SourceRef") {
                if let Some(&i) = sr.target().and_then(|g| source_index.get(g)) {
                    case.members.push(CaseMember { source: i, unit: None });
                }
            }
            for sr in c.kids("SelectionRef") {
                let Some(&(i, start, end)) = sr.target().and_then(|g| selection_place.get(g)) else {
                    left.unit_members += 1;
                    continue;
                };
                match batch.sources[i].units.iter().position(|u| u.start == start && u.end == end) {
                    Some(u) => case.members.push(CaseMember { source: i, unit: Some(u) }),
                    None => left.unit_members += 1,
                }
            }
            case_guid_note.push(c.attr("guid").unwrap_or("").to_string());
            batch.cases.push(case);
        }
    }
    // PrismQDA keeps variables on cases, so a value the file puts on a document goes to the case that holds the document, and a document in no
    // case gets a case of its own, named for it. A case that already has a value for the variable keeps its own.
    let mut doc_values_placed = 0usize;
    let mut doc_cases_made = 0usize;
    for (index, guid, value) in &doc_values {
        let Some(var) = var_name.get(guid) else {
            left.source_values += 1;
            continue;
        };
        let at = match batch.cases.iter().position(|c| c.members.iter().any(|m| m.source == *index && m.unit.is_none())) {
            Some(at) => at,
            None => {
                batch.cases.push(ImportedCase { name: batch.sources[*index].name.clone(), attrs: Vec::new(), members: vec![CaseMember { source: *index, unit: None }] });
                case_guid_note.push(String::new());
                doc_cases_made += 1;
                batch.cases.len() - 1
            }
        };
        if batch.cases[at].attrs.iter().any(|(v, _)| v == var) {
            left.source_values += 1;
            continue;
        }
        batch.cases[at].attrs.push((var.clone(), value.clone()));
        doc_values_placed += 1;
    }
    // two cases with one name cannot both be kept; the second takes a number
    {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for c in &mut batch.cases {
            let base = c.name.clone();
            let mut unique = base.clone();
            let mut k = 2;
            while !seen.insert(unique.to_lowercase()) {
                unique = format!("{base} ({k})");
                k += 1;
            }
            c.name = unique;
        }
    }

    // ---- make the project
    if project_path.exists() {
        return Err(bad("There is already a file where the new project would go."));
    }
    let mut project = Project::create(project_path, &NewProject::new(project_name.clone()), &OpenOptions::new("QDPX import"))?;
    let outcome = project.apply_import(&batch)?;

    // coders
    let mut coder_of: HashMap<String, i64> = HashMap::new();
    let mut fallback: Option<i64> = None;
    if let Some(users) = root.kid("Users") {
        for u in users.kids("User") {
            let guid = u.attr("guid").unwrap_or("").to_string();
            let display = u.attr("name").filter(|n| !n.trim().is_empty()).or_else(|| u.attr("id")).unwrap_or("Coder").trim().to_string();
            let local = if guid.is_empty() { None } else { Some(format!("qdpx:{guid}")) };
            let id = project.add_coder(&NewCoder { local_id: local, ..NewCoder::human(display) })?;
            coder_of.insert(guid, id);
        }
    }
    let mut coder_for = |project: &mut Project, user: &Option<String>| -> Result<i64, ExportError> {
        if let Some(id) = user.as_ref().and_then(|u| coder_of.get(u)) {
            return Ok(*id);
        }
        if let Some(id) = fallback {
            return Ok(id);
        }
        let id = project.add_coder(&NewCoder::human("Imported (QDPX)"))?;
        fallback = Some(id);
        Ok(id)
    };

    // codes
    let mut code_of: HashMap<String, i64> = HashMap::new();
    fn add_codes(project: &mut Project, node: &Node, parent: Option<i64>, code_of: &mut HashMap<String, i64>, left: &mut Left, notes: &mut Vec<(String, NoteHome)>, depth: usize) -> Result<usize, ExportError> {
        if depth > MAX_DEPTH {
            return Err(bad("the codebook nests too deeply"));
        }
        let mut added = 0;
        for c in node.kids("Code") {
            let name = c.attr("name").unwrap_or("Code").trim();
            let name = if name.is_empty() { "Code" } else { name };
            let (definition, include, exclude) = split_description(c.kid("Description").map(|d| d.text.as_str()).unwrap_or(""));
            let mut spec = NewCode {
                name: name.to_string(),
                parent_id: parent,
                theme: c.attr("isCodable").map(|v| v.trim() == "false").unwrap_or(false),
                color: parse_color(c.attr("color")),
                definition,
                inclusion_criteria: include,
                exclusion_criteria: exclude,
                exclusive_group: String::new(),
                rated: false,
                rating_label: String::new(),
                rating_low: String::new(),
                rating_high: String::new(),
            };
            let mut id = None;
            for k in 1..50 {
                match project.add_code(&spec) {
                    Ok(i) => {
                        id = Some(i);
                        break;
                    }
                    Err(prismqda_engine::Error::Invalid(m)) if m.contains("already") => {
                        left.renamed_codes += 1;
                        spec.name = format!("{name} ({})", k + 1);
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            let Some(id) = id else { continue };
            added += 1;
            if let Some(g) = c.attr("guid") {
                code_of.insert(g.to_string(), id);
                for nref in c.kids("NoteRef") {
                    if let Some(t) = nref.target() {
                        notes.push((t.to_string(), NoteHome::Code(g.to_string())));
                    }
                }
            }
            added += add_codes(project, c, Some(id), code_of, left, notes, depth + 1)?;
        }
        Ok(added)
    }
    let mut code_count = 0;
    if let Some(codes) = root.kid("CodeBook").and_then(|b| b.kid("Codes")) {
        code_count = add_codes(&mut project, codes, None, &mut code_of, &mut left, &mut note_homes, 0)?;
    }

    // sets of codes
    let mut set_count = 0;
    if let Some(sets) = root.kid("Sets") {
        for s in sets.kids("Set") {
            let set_name = s.attr("name").unwrap_or("Set").trim().to_string();
            let description = s.kid("Description").map(|d| d.text.trim().to_string()).unwrap_or_default();
            let mut set_id = None;
            for k in 1..50 {
                let n = if k == 1 { set_name.clone() } else { format!("{set_name} ({k})") };
                match project.add_code_set(&n, &description) {
                    Ok(i) => {
                        set_id = Some(i);
                        break;
                    }
                    Err(prismqda_engine::Error::Invalid(_)) => continue,
                    Err(e) => return Err(e.into()),
                }
            }
            let Some(set_id) = set_id else { continue };
            set_count += 1;
            for m in s.kids("MemberCode") {
                if let Some(&code) = m.target().and_then(|g| code_of.get(g)) {
                    project.set_code_in_set(set_id, code, true)?;
                }
            }
            left.set_members += s.kids("MemberSource").count() + s.kids("MemberNote").count();
        }
    }

    // codings
    let source_ids = &outcome.source_ids;
    let mut coding_of: HashMap<String, i64> = HashMap::new();
    let mut made = 0usize;
    let mut new_codings: Vec<(String, NewCoding)> = Vec::new();
    for p in &pending {
        let (Some(&code_id), Some(&source_id)) = (code_of.get(&p.code), source_ids.get(p.source)) else {
            left.unplaced += 1;
            continue;
        };
        let coder_id = coder_for(&mut project, &p.user)?;
        new_codings.push((
            p.guid.clone(),
            NewCoding { source_id, start: p.start, end: p.end, code_id, coder_id, origin: Origin::Human, rationale: p.rationale.clone(), unit_id: None, replace_exclusive: false, via_proposal: None, ai_run_id: None },
        ));
    }
    for chunk in new_codings.chunks(2000) {
        let list: Vec<NewCoding> = chunk.iter().map(|(_, c)| c.clone()).collect();
        match project.apply_codings(&list) {
            Ok(ids) => {
                for ((g, _), id) in chunk.iter().zip(ids) {
                    coding_of.insert(g.clone(), id);
                    made += 1;
                }
            }
            Err(_) => {
                for (g, c) in chunk {
                    match project.apply_coding(c) {
                        Ok(id) => {
                            coding_of.insert(g.clone(), id);
                            made += 1;
                        }
                        Err(_) => left.unplaced += 1,
                    }
                }
            }
        }
    }
    let mut area_count = 0usize;
    let mut pdf_rectangles = 0usize;
    for r in &regions {
        let (Some(&code_id), Some(&source_id)) = (code_of.get(&r.code), source_ids.get(r.source)) else {
            left.unplaced += 1;
            continue;
        };
        let coder_id = coder_for(&mut project, &r.user)?;
        let (x0, y0, x1, y1) = r.rect;
        let mut new = NewRegion::new(source_id, Shape::Rectangle { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }, code_id, coder_id);
        new.rationale = r.rationale.clone();
        new.page = r.page;
        match project.apply_region(&new) {
            Ok(id) => {
                area_count += 1;
                if r.on_pdf {
                    pdf_rectangles += 1;
                }
                if !r.selection.is_empty() {
                    region_of.entry(r.selection.clone()).or_insert(-id);
                }
            }
            Err(_) => left.unplaced += 1,
        }
    }

    // recordings: a recording the file names is linked to its document when it can be found on this computer (or is inside the archive), and
    // the stretches coded on it are coded again. A stretch needs the recording, so without it the codings on it are left out.
    let base_path = root.attr("basePath").unwrap_or("").to_string();
    let attached: Vec<(usize, &Node, bool)> = media_docs.iter().copied().chain(media_attach.iter().filter_map(|(g, n)| source_index.get(g).map(|&i| (i, *n, false)))).collect();
    let mut media_linked = 0usize;
    let mut span_count = 0usize;
    for (index, node, stands_alone) in attached {
        let source_id = outcome.source_ids[index];
        let selections: Vec<&Node> = node.children.iter().filter(|c| c.name.ends_with("Selection")).collect();
        let found = find_recording(node, &mut archive, &base_path, project_path);
        let linked = found.as_ref().is_some_and(|p| project.link_media_as(source_id, p, Some(stands_alone)).is_ok());
        if !linked {
            left.media_missing.push(batch.sources[index].name.clone());
            left.media_selections += selections.iter().map(|s| s.kids("Coding").count().max(1)).sum::<usize>();
            continue;
        }
        media_linked += 1;
        for sel in selections {
            let (Some(a), Some(b)) = (sel.attr("begin").and_then(|v| v.trim().parse::<i64>().ok()), sel.attr("end").and_then(|v| v.trim().parse::<i64>().ok())) else {
                left.unplaced += sel.kids("Coding").count().max(1);
                continue;
            };
            let rationale = sel.kid("Description").map(|d| d.text.trim().to_string()).unwrap_or_default();
            for c in sel.kids("Coding") {
                let Some(code) = c.kid("CodeRef").and_then(Node::target).and_then(|g| code_of.get(g)).copied() else {
                    left.unplaced += 1;
                    continue;
                };
                let coder_id = coder_for(&mut project, &c.attr("creatingUser").map(str::to_string))?;
                let mut new = NewRegion::new(source_id, Shape::Span { start_ms: a, end_ms: b }, code, coder_id);
                new.rationale = rationale.clone();
                match project.apply_region(&new) {
                    Ok(id) => {
                        span_count += 1;
                        if let Some(g) = sel.attr("guid").filter(|g| !g.is_empty()) {
                            region_of.entry(g.to_string()).or_insert(-id);
                        }
                    }
                    Err(_) => left.unplaced += 1,
                }
            }
        }
    }

    // links between passages: one way, from the origin to the target, named as the file names it
    let mut link_count = 0usize;
    if let Some(links) = root.kid("Links") {
        for l in links.kids("Link") {
            let (Some(o), Some(t)) = (l.attr("originGUID"), l.attr("targetGUID")) else {
                left.links += 1;
                continue;
            };
            let find = |g: &str| selection_first.get(g).and_then(|c| coding_of.get(c)).copied().or_else(|| region_of.get(g).copied());
            let (Some(a), Some(b)) = (find(o), find(t)) else {
                left.links += 1;
                continue;
            };
            let name: String = l.attr("name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or("is linked to").chars().take(80).collect();
            match project.add_passage_link(a, b, &name) {
                Ok(_) => {
                    link_count += 1;
                    // The file joins selections; a selection with several codings is joined at its first.
                    if selection_codings.get(o).copied().unwrap_or(0) > 1 || selection_codings.get(t).copied().unwrap_or(0) > 1 {
                        left.links_shared += 1;
                    }
                }
                Err(_) => left.links += 1,
            }
        }
    }

    // notes become memos, once each
    let mut notes_by_guid: HashMap<String, (&Node, String)> = HashMap::new();
    if let Some(notes) = root.kid("Notes") {
        for n in notes.kids("Note") {
            let body = n.kid("PlainTextContent").map(|c| c.text.clone()).or_else(|| n.attr("plainTextPath").and_then(|p| archive.file(p)).map(|(_, b)| decode_text(&b))).unwrap_or_default();
            notes_by_guid.insert(n.attr("guid").unwrap_or("").to_string(), (n, body));
        }
    }
    for r in root.kids("NoteRef") {
        if let Some(t) = r.target() {
            note_homes.push((t.to_string(), NoteHome::Project));
        }
    }
    if let Some(d) = root.kid("Description").map(|d| d.text.trim().to_string()).filter(|d| !d.is_empty()) {
        project.add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Project, target_id: None, phase: None, title: "Project description".to_string(), body: d })?;
    }
    let mut memo_count = 0usize;
    let mut memo_of: HashMap<String, i64> = HashMap::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    for (guid, home) in &note_homes {
        if used.contains(guid) {
            continue;
        }
        let Some((node, body)) = notes_by_guid.get(guid) else { continue };
        let (scope, target) = match home {
            NoteHome::Project => (MemoScope::Project, None),
            NoteHome::Source(i) => match source_ids.get(*i) {
                Some(id) => (MemoScope::Source, Some(*id)),
                None => (MemoScope::Project, None),
            },
            NoteHome::Code(g) => match code_of.get(g) {
                Some(id) => (MemoScope::Code, Some(*id)),
                None => (MemoScope::Project, None),
            },
            NoteHome::Coding(g) => match coding_of.get(g) {
                Some(id) => (MemoScope::Coding, Some(*id)),
                None => (MemoScope::Project, None),
            },
        };
        used.insert(guid.clone());
        let id = project.add_memo(&NewMemo { kind: MemoKind::Analytic, scope, target_id: target, phase: None, title: node.attr("name").unwrap_or("").to_string(), body: body.clone() })?;
        memo_of.insert(guid.clone(), id);
        memo_count += 1;
    }
    for (guid, (node, body)) in &notes_by_guid {
        if used.contains(guid) {
            continue;
        }
        let id = project.add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Standalone, target_id: None, phase: None, title: node.attr("name").unwrap_or("").to_string(), body: body.clone() })?;
        memo_of.insert(guid.clone(), id);
        memo_count += 1;
    }

    // graphs become concept maps: a box that stands for a code, a case, or a memo that came in is that thing, and any other box is a free note
    // with its words; a line joins two boxes, with an arrow when the file says it runs one way
    let case_of: HashMap<&str, i64> = case_guid_note.iter().zip(outcome.case_ids.iter()).filter(|(g, _)| !g.is_empty()).map(|(g, id)| (g.as_str(), *id)).collect();
    let mut map_count = 0usize;
    let mut box_count = 0usize;
    let mut line_count = 0usize;
    if let Some(graphs) = root.kid("Graphs") {
        let mut map_names: BTreeSet<String> = BTreeSet::new();
        for g in graphs.kids("Graph") {
            let base = g.attr("name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Graph").to_string();
            let mut name = base.clone();
            let mut k = 2;
            while !map_names.insert(name.to_lowercase()) {
                name = format!("{base} ({k})");
                k += 1;
            }
            let map_id = project.create_concept_map(&name)?;
            map_count += 1;
            let mut node_of: HashMap<String, i64> = HashMap::new();
            for v in g.kids("Vertex") {
                let (Some(guid), Some(x), Some(y)) = (v.attr("guid"), v.attr("firstX").and_then(|s| s.trim().parse::<f64>().ok()), v.attr("firstY").and_then(|s| s.trim().parse::<f64>().ok())) else { continue };
                let represented = v.attr("representedGUID").unwrap_or("");
                let thing = if let Some(&id) = code_of.get(represented) {
                    Some(("code", id))
                } else if let Some(&id) = case_of.get(represented) {
                    Some(("case", id))
                } else {
                    memo_of.get(represented).map(|&id| ("memo", id))
                };
                let made = thing.and_then(|(kind, id)| project.add_map_node(map_id, kind, Some(id), "", x, y).ok());
                let node = match made {
                    Some(n) => n,
                    None => {
                        // not a code, case, or memo that came in (or already on this map): its words, as a free note
                        let words = v.attr("name").map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).or_else(|| source_index.get(represented).map(|&i| batch.sources[i].name.clone())).unwrap_or_else(|| "Box".to_string());
                        if !represented.is_empty() || thing.is_some() {
                            left.graph_notes += 1;
                        }
                        match project.add_map_node(map_id, "note", None, &words, x, y) {
                            Ok(n) => n,
                            Err(_) => continue,
                        }
                    }
                };
                node_of.insert(guid.to_string(), node.id);
                box_count += 1;
            }
            for e in g.kids("Edge") {
                let (Some(a), Some(b)) = (e.attr("sourceVertex").and_then(|g| node_of.get(g)), e.attr("targetVertex").and_then(|g| node_of.get(g))) else {
                    left.graph_edges += 1;
                    continue;
                };
                let words = e.attr("name").map(str::trim).unwrap_or("");
                let directed = e.attr("direction").map(|d| d.trim() == "OneWay").unwrap_or(false);
                match project.add_map_link(map_id, *a, *b, words, directed) {
                    Ok(_) => line_count += 1,
                    Err(_) => left.graph_edges += 1,
                }
            }
        }
    }

    // what the file held that is not taken
    left.case_notes = 0;

    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    if qualcoder_line_ends > 0 {
        report.carried.push(format!("{} saved by QualCoder with Windows line endings, read as QualCoder counts them (a line break as one character), so the codings are where they were made", plural(qualcoder_line_ends, "text", "texts")));
    }
    report.carried.push(plural(coder_of.len() + usize::from(fallback.is_some()), "coder", "coders"));
    report.carried.push(plural(code_count, "code", "codes"));
    report.carried.push(plural(batch.sources.len(), "document, picture, or PDF", "documents, pictures, and PDFs"));
    report.carried.push(format!("{} (words) and {} (areas of pictures)", plural(made, "coding", "codings"), area_count));
    report.carried.push(format!("{} with {}", plural(batch.cases.len(), "case", "cases"), plural(batch.case_attributes.len(), "variable", "variables")));
    report.carried.push(plural(memo_count, "memo", "memos"));
    if link_count > 0 {
        report.carried.push(plural(link_count, "link between passages", "links between passages"));
    }
    if map_count > 0 {
        report.carried.push(format!("{} (graphs) with {} and {}", plural(map_count, "concept map", "concept maps"), plural(box_count, "box", "boxes"), plural(line_count, "line", "lines")));
    }
    if pdf_rectangles > 0 {
        report.carried.push(format!("{} on the pages of PDFs", plural(pdf_rectangles, "coded rectangle", "coded rectangles")));
    }
    if unit_maps_read > 0 {
        report.carried.push(format!("The kind and the units of {} (read from its description)", plural(unit_maps_read, "document", "documents")));
    }
    if !second_text_source.is_empty() {
        report.carried.push(format!("{} on the same file as a recording's transcript, read as part of that transcript's document (ELAN writes the codings of a transcript this way)", plural(second_text_source.len(), "text source", "text sources")));
    }
    if timed_transcripts > 0 {
        report.carried.push(format!("{} with the times of its lines (from the sync points), each line a unit with its time", plural(timed_transcripts, "transcript", "transcripts")));
    }
    if media_linked > 0 {
        report.carried.push(format!("{} linked, with {} coded on them", plural(media_linked, "recording", "recordings"), plural(span_count, "stretch", "stretches")));
    }
    if doc_values_placed > 0 {
        let made = if doc_cases_made > 0 { format!(" ({} made, one for each document that was in no case)", plural(doc_cases_made, "case", "cases")) } else { String::new() };
        report.carried.push(format!("{} on documents, put on the case that holds each document{made}", plural(doc_values_placed, "variable value", "variable values")));
    }
    report.carried.push(plural(set_count, "set of codes", "sets of codes"));
    let l = &mut report.left_out;
    if !left.media_missing.is_empty() {
        l.push(format!(
            "{} that could not be found on this computer ({}), so each document is here without its recording and the stretches coded on it ({}) are left out. Link the recording to the document and code the stretches again.",
            plural(left.media_missing.len(), "recording", "recordings"),
            left.media_missing.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
            plural(left.media_selections, "coding", "codings"),
        ));
    }
    if left.sync_points > 0 || left.transcript_selections > 0 {
        l.push(format!(
            "{} and {} of transcripts that could not be placed: a sync point with no position in the text, and a selection between sync points that are not there or hold no words.",
            plural(left.sync_points, "sync point", "sync points"),
            plural(left.transcript_selections, "selection", "selections"),
        ));
    }
    if left.pdf_areas > 0 {
        l.push(format!("{} on the pages of PDFs that could not be placed (the page is not in the PDF, the PDF is not in the file, or the rectangle has no area).", plural(left.pdf_areas, "coding of a rectangle", "codings of rectangles")));
    }
    if !left.no_text.is_empty() {
        l.push(format!("{} with no plain text file, which the standard asks for so that selections can be made on it: {}.", plural(left.no_text.len(), "document", "documents"), left.no_text.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
    }
    if !left.unreadable.is_empty() {
        l.push(format!("{}: {}.", plural(left.unreadable.len(), "source that could not be read", "sources that could not be read"), left.unreadable.iter().take(5).cloned().collect::<Vec<_>>().join("; ")));
    }
    if left.unplaced > 0 {
        l.push(format!("{} whose place or code could not be found.", plural(left.unplaced, "coding or selection", "codings and selections")));
    }
    if left.clamped > 0 {
        l.push(format!("{} that ran past the end of its text, which were cut at the end.", plural(left.clamped, "selection", "selections")));
    }
    if left.normalized > 0 {
        l.push(format!("{} had to be put in Unicode normal form; the places of its selections were carried through the change.", plural(left.normalized, "document", "documents")));
    }
    if !left.unnormalized.is_empty() {
        l.push(format!("The codings on {} could not be kept in place when its text was normalized.", left.unnormalized.join(", ")));
    }
    if left.links > 0 {
        l.push(format!("{} that join something other than two coded passages that came in (a document, a code, or a passage that was left out).", plural(left.links, "link", "links")));
    }
    if left.links_shared > 0 {
        l.push(format!("{} that join words with more than one coding. The file joins the words and not a coding, so each was joined to the first coding of those words; check them.", plural(left.links_shared, "link", "links")));
    }
    if left.graph_notes > 0 {
        l.push(format!("{} in graphs that stand for a document or for something that did not come in, or that were already on the map: each is a free note with its words.", plural(left.graph_notes, "box", "boxes")));
    }
    if left.graph_edges > 0 {
        l.push(format!("{} in graphs that join a box to itself or to a box that is not there, or join two boxes that are already joined with those words.", plural(left.graph_edges, "line", "lines")));
    }
    if left.set_members > 0 {
        l.push(format!("{} of sets that are documents or notes (only codes are kept in sets).", plural(left.set_members, "member", "members")));
    }
    if left.source_values > 0 {
        l.push(format!("{} on documents that could not be put on a case: a variable the file does not define, a value with nothing in it, or a case that already had its own value for that variable, which was kept.", plural(left.source_values, "variable value", "variable values")));
    }
    if left.unit_members > 0 {
        l.push(format!("{} of cases that are a selection, not a whole document or one of its units.", plural(left.unit_members, "member", "members")));
    }
    if left.renamed_codes > 0 {
        l.push(format!("{} named like a code beside it, which were given a number.", plural(left.renamed_codes, "code", "codes")));
    }
    l.push("The kind and the units of a document that did not come from PrismQDA (it arrives as a document, split into sentences), its formatting, its styled-text file, and the dates and users of everything but codings.".to_string());
    Ok(report)
}

#[cfg(test)]
mod percent_tests {
    use super::percent_decoded;

    #[test]
    fn percent_codes_are_decoded_and_a_percent_before_other_characters_is_only_a_percent() {
        assert_eq!(percent_decoded("a%20b%2Fc"), "a b/c");
        assert_eq!(percent_decoded("%41"), "A", "a code at the very end");
        assert_eq!(percent_decoded("%4"), "%4", "too short");
        assert_eq!(percent_decoded("%zz%+1"), "%zz%+1", "not hexadecimal digits");
        // A character outside ASCII after the percent sign put the cut of the text where no character begins, which stopped the program.
        assert_eq!(percent_decoded("5%\u{540d}\u{524d}.mp3"), "5%\u{540d}\u{524d}.mp3");
        assert_eq!(percent_decoded("%a\u{e9}"), "%a\u{e9}");
        assert_eq!(percent_decoded("%e5%90%8d.mp3"), "\u{540d}.mp3", "percent codes that spell a character");
    }
}

/// A path written in an exchange file may be a network address, and asking whether it exists connects to that server (see
/// `prismqda_engine::paths`). The recording is looked for with no question asked of the disk, which these tests can see because the
/// same file, reached by a network-style name that needs no network, would otherwise be found.
#[cfg(all(test, windows))]
mod network_path_tests {
    use super::*;
    use std::io::Write;

    fn empty_archive() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("project.qde", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"<Project xmlns="urn:QDA-XML:project:1.0" name="T"><Sources/></Project>"#).unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn source(path: &str) -> Node {
        parse_tree(&format!(r#"<AudioSource path="{path}"/>"#), "test").unwrap()
    }

    #[test]
    fn a_recording_path_that_is_a_network_address_is_never_looked_at() {
        let bytes = empty_archive();
        let (mut archive, _) = Archive::open(&bytes).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("a.mp3");
        std::fs::write(&real, b"a file").unwrap();
        let project = dir.path().join("o.pqda");
        let local = format!("absolute:///{}", real.to_string_lossy().replace('\\', "/"));
        assert!(find_recording(&source(&local), &mut archive, "", &project).is_some(), "a place on a drive with a letter is found");

        let text = real.to_string_lossy().to_string();
        let unc = format!(r"\\localhost\{}${}", &text[..1], &text[2..]);
        if std::fs::metadata(&unc).is_ok() {
            for attr in [format!("absolute://{}", unc.replace('\\', "/")), unc.clone(), unc.replace('\\', "/")] {
                assert!(find_recording(&source(&attr), &mut archive, "", &project).is_none(), "{attr} is a file that is there, by a network address: not looked at");
            }
            assert!(find_recording(&source("relative://a.mp3"), &mut archive, &unc[..unc.rfind('\\').unwrap()].replace('\\', "/"), &project).is_none(), "nor through the file's base path");
        }
        for attr in ["absolute:////example.invalid/s/a.mp3", r"\\example.invalid\s\a.mp3", r"\\?\UNC\example.invalid\s\a.mp3"] {
            assert!(find_recording(&source(attr), &mut archive, "", &project).is_none(), "{attr}");
        }
        assert!(find_recording(&source("relative://a.mp3"), &mut archive, "//example.invalid/s", &project).is_none());
    }
}
