//! RTF import (specification section 5), written from the public RTF specification. There is no usable
//! permissive RTF reader, so this file has its own tokenizer and group-stack parser.
//!
//! The reader keeps headings (`\outlinelevelN`, or a paragraph style named like "heading 1"), bold
//! and italic, list items, and table text (`\cell`, `\row`, and nested tables with `\nestcell`).
//! It understands Word, WordPad and LibreOffice files: `\'hh` bytes are decoded with the code page of
//! the current font or the document (`\ansicpg`), `\uN` is followed by its `\ucN` fallback characters
//! (and surrogate pairs are joined), and destinations that are not text (font and color tables, the
//! stylesheet, document info, headers and footers, field codes, list labels) are skipped.
//! Pictures and objects become the placeholder `[image]`; footnotes, comments, equations, shapes and
//! text boxes are counted and reported as warnings.

use std::collections::{BTreeSet, HashMap};

use encoding_rs::Encoding;
use prismqda_engine::{ImportWarning, SourceType, UnitKind};

use crate::builder::{ParsedText, TextBuilder};
use crate::docx::{script_of, sort_runs, Counts, Layout, ParaBuf, Script};
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;

/// How deeply braces may nest. Real files stay far below this (Word nests about twenty levels).
const MAX_GROUP_DEPTH: usize = 200;

/// Reads the whole file and returns its text with formatting runs (headings, bold, italic, list items).
pub fn parse(bytes: &[u8], _opts: &ImportOptions) -> Result<ParsedText> {
    parse_with_depth(bytes, MAX_GROUP_DEPTH)
}

pub(crate) fn parse_with_depth(bytes: &[u8], max_depth: usize) -> Result<ParsedText> {
    let mut data = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let lead = data.iter().take_while(|b| b.is_ascii_whitespace()).count();
    data = &data[lead..];
    if !data.starts_with(b"{\\rtf") {
        let hint = if data.starts_with(b"PK\x03\x04") {
            "it looks like a Word (DOCX) or OpenDocument file with an .rtf name"
        } else if data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
            "it looks like an older binary Word file with an .rtf name"
        } else {
            "it does not start the way an RTF file does"
        };
        return Err(ImportError::format(format!("this is not an RTF file: {hint}")));
    }
    let mut rtf = Rtf::new(max_depth);
    rtf.run(data)?;
    Ok(rtf.finish())
}

// --- tokenizer -------------------------------------------------------------------------------

enum Tok<'a> {
    Open,
    Close,
    /// A control word and its numeric parameter.
    Word(&'a [u8], Option<i32>),
    /// A control symbol such as `\~` or `\{`.
    Sym(u8),
    /// `\'hh`: one byte in the current code page.
    Hex(u8),
    /// Plain text bytes.
    Text(&'a [u8]),
}

struct Lexer<'a> {
    data: &'a [u8],
    pos: usize,
}

fn hex_digit(b: Option<&u8>) -> Option<u8> {
    match b? {
        c @ b'0'..=b'9' => Some(c - b'0'),
        c @ b'a'..=b'f' => Some(c - b'a' + 10),
        c @ b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl<'a> Lexer<'a> {
    fn next_token(&mut self) -> Option<Tok<'a>> {
        loop {
            let c = *self.data.get(self.pos)?;
            match c {
                // Line ends in the file are not text; neither is a stray NUL.
                b'\r' | b'\n' | 0 => self.pos += 1,
                b'{' => {
                    self.pos += 1;
                    return Some(Tok::Open);
                }
                b'}' => {
                    self.pos += 1;
                    return Some(Tok::Close);
                }
                b'\\' => return self.control(),
                _ => {
                    let start = self.pos;
                    while self.data.get(self.pos).is_some_and(|b| !matches!(b, b'{' | b'}' | b'\\' | b'\r' | b'\n' | 0))
                    {
                        self.pos += 1;
                    }
                    return Some(Tok::Text(&self.data[start..self.pos]));
                }
            }
        }
    }

    fn control(&mut self) -> Option<Tok<'a>> {
        self.pos += 1; // the backslash
        let c = *self.data.get(self.pos)?;
        if c.is_ascii_alphabetic() {
            let start = self.pos;
            while self.data.get(self.pos).is_some_and(u8::is_ascii_alphabetic) {
                self.pos += 1;
            }
            let name = &self.data[start..self.pos];
            let negative =
                self.data.get(self.pos) == Some(&b'-') && self.data.get(self.pos + 1).is_some_and(u8::is_ascii_digit);
            if negative {
                self.pos += 1;
            }
            let mut param = None;
            if self.data.get(self.pos).is_some_and(u8::is_ascii_digit) {
                let mut n: i64 = 0;
                while let Some(d) = self.data.get(self.pos).filter(|d| d.is_ascii_digit()) {
                    n = (n * 10 + i64::from(d - b'0')).min(i64::from(i32::MAX));
                    self.pos += 1;
                }
                let n = n as i32;
                param = Some(if negative { -n } else { n });
            }
            // One space after a control word belongs to the word.
            if self.data.get(self.pos) == Some(&b' ') {
                self.pos += 1;
            }
            // Binary data follows \binN: it can hold braces and backslashes, so it is skipped by count.
            if name == b"bin" {
                if let Some(n) = param.filter(|n| *n > 0) {
                    self.pos = self.pos.saturating_add(n as usize).min(self.data.len());
                }
            }
            return Some(Tok::Word(name, param));
        }
        self.pos += 1;
        Some(match c {
            b'\'' => match (hex_digit(self.data.get(self.pos)), hex_digit(self.data.get(self.pos + 1))) {
                (Some(hi), Some(lo)) => {
                    self.pos += 2;
                    Tok::Hex(hi * 16 + lo)
                }
                _ => Tok::Sym(b'\''),
            },
            // A backslash before a line end means a paragraph end.
            b'\r' | b'\n' => Tok::Word(b"par", None),
            b'*' => Tok::Word(b"*", None),
            other => Tok::Sym(other),
        })
    }
}

// --- parser state ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Text of the document.
    Normal,
    /// A destination whose content is not text of the document.
    Skip,
    FontTable,
    StyleSheet,
    StyleEntry,
    /// `\*\nesttableprops`: only the end of a nested row is of interest.
    NestProps,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Close {
    Image,
    Scan,
}

#[derive(Clone)]
struct Group {
    mode: Mode,
    bold: bool,
    italic: bool,
    /// Bold and italic for complex scripts (`\ab`, `\ai`); fall back to the plain settings when unset.
    abold: Option<bool>,
    aitalic: Option<bool>,
    hidden: bool,
    font: Option<i32>,
    /// How many characters after `\uN` are a fallback to skip (`\ucN`).
    uc: u32,
    /// No word has been seen in this group yet.
    first: bool,
    /// The group started with `\*`: a destination to skip unless it is known.
    ignorable: bool,
    close: Option<Close>,
}

impl Group {
    fn root() -> Group {
        Group {
            mode: Mode::Normal,
            bold: false,
            italic: false,
            abold: None,
            aitalic: None,
            hidden: false,
            font: None,
            uc: 1,
            first: false,
            ignorable: false,
            close: None,
        }
    }
}

/// Paragraph properties; these belong to the paragraph, not to a brace group, and `\pard` clears them.
#[derive(Default)]
struct ParaProps {
    style: Option<i32>,
    /// `Some(None)` is an explicit body text (`\outlinelevel9`).
    outline: Option<Option<u8>>,
    list: bool,
    intbl: bool,
    itap: usize,
}

#[derive(Default)]
struct StyleEntry {
    id: Option<i32>,
    /// A character or table style, which is not a paragraph style.
    other: bool,
    based_on: Option<i32>,
    outline: Option<u8>,
    name: String,
    name_done: bool,
}

/// What is inside a shape or drawing object that is being skipped.
#[derive(Default)]
struct Scan {
    text_box: bool,
    picture: bool,
}

struct Rtf {
    stack: Vec<Group>,
    max_depth: usize,
    b: TextBuilder,
    layout: Layout,
    counts: Counts,
    buf: ParaBuf,
    pp: ParaProps,
    in_row: bool,
    last_script: Script,

    ansicpg: u32,
    deff: Option<i32>,
    font_codepages: HashMap<i32, u32>,
    font_def: Option<i32>,
    styles: HashMap<i32, StyleEntry>,
    style_entry: StyleEntry,
    scan: Option<Scan>,

    /// Bytes from `\'hh` or raw text that wait to be decoded together (multi-byte code pages).
    pending: Vec<u8>,
    pending_cp: u32,
    high_surrogate: Option<u32>,
    skip_fallback: u32,
    unsupported_codepages: BTreeSet<u32>,
}

/// Words that start a destination whose content is not text of the document.
fn is_skipped_destination(name: &[u8]) -> bool {
    matches!(
        name,
        b"colortbl"
            | b"info"
            | b"header"
            | b"headerl"
            | b"headerr"
            | b"headerf"
            | b"footer"
            | b"footerl"
            | b"footerr"
            | b"footerf"
            | b"listtable"
            | b"listoverridetable"
            | b"rsidtbl"
            | b"revtbl"
            | b"generator"
            | b"themedata"
            | b"colorschememapping"
            | b"latentstyles"
            | b"datastore"
            | b"xmlnstbl"
            | b"filetbl"
            | b"pgdsctbl"
            | b"fldinst"
            | b"bkmkstart"
            | b"bkmkend"
            | b"xe"
            | b"txe"
            | b"rxe"
            | b"tc"
            | b"tcn"
            | b"ud"
            | b"upr"
            | b"nonshppict"
            | b"objdata"
            | b"ftnsep"
            | b"ftnsepc"
            | b"aftnsep"
            | b"aftnsepc"
            | b"ftncn"
            | b"aftncn"
            | b"datafield"
            | b"formfield"
    )
}

/// The Windows code page a font character set (`\fcharsetN`) implies; `None` for the default set.
fn charset_codepage(charset: i32) -> Option<u32> {
    Some(match charset {
        77 => 10000,
        128 => 932,
        129 => 949,
        130 => 1361,
        134 => 936,
        136 => 950,
        161 => 1253,
        162 => 1254,
        163 => 1258,
        177 => 1255,
        178 => 1256,
        186 => 1257,
        204 => 1251,
        222 => 874,
        238 => 1250,
        254 => 437,
        255 => 850,
        _ => return None,
    })
}

/// The decoder for a Windows code page number, when one is available.
fn codepage_encoding(cp: u32) -> Option<&'static Encoding> {
    Some(match cp {
        874 => encoding_rs::WINDOWS_874,
        866 => encoding_rs::IBM866,
        932 => encoding_rs::SHIFT_JIS,
        936 => encoding_rs::GBK,
        949 => encoding_rs::EUC_KR,
        950 => encoding_rs::BIG5,
        1250 => encoding_rs::WINDOWS_1250,
        1251 => encoding_rs::WINDOWS_1251,
        1252 => encoding_rs::WINDOWS_1252,
        1253 => encoding_rs::WINDOWS_1253,
        1254 => encoding_rs::WINDOWS_1254,
        1255 => encoding_rs::WINDOWS_1255,
        1256 => encoding_rs::WINDOWS_1256,
        1257 => encoding_rs::WINDOWS_1257,
        1258 => encoding_rs::WINDOWS_1258,
        10000 => encoding_rs::MACINTOSH,
        10007 => encoding_rs::X_MAC_CYRILLIC,
        20866 => encoding_rs::KOI8_R,
        21866 => encoding_rs::KOI8_U,
        28592 => encoding_rs::ISO_8859_2,
        28595 => encoding_rs::ISO_8859_5,
        28597 => encoding_rs::ISO_8859_7,
        28598 => encoding_rs::ISO_8859_8,
        54936 => encoding_rs::GB18030,
        65001 => encoding_rs::UTF_8,
        _ => return None,
    })
}

/// The heading level a style name implies ("heading 1", "Title").
fn level_from_name(name: &str) -> Option<u8> {
    let lower = name.trim().to_ascii_lowercase();
    if lower == "title" {
        return Some(1);
    }
    match lower.strip_prefix("heading")?.trim().parse::<u8>() {
        Ok(n) if (1..=9).contains(&n) => Some(n.min(6)),
        _ => None,
    }
}

/// Longest chain of parent styles that is followed; a loop in a damaged file stops here.
const MAX_STYLE_CHAIN: usize = 24;

impl Rtf {
    fn new(max_depth: usize) -> Rtf {
        Rtf {
            // The first group is a stand-in for "outside the file": the real root group sits on top of it.
            stack: vec![Group::root()],
            max_depth,
            b: TextBuilder::new(),
            layout: Layout::default(),
            counts: Counts::default(),
            buf: ParaBuf::default(),
            pp: ParaProps::default(),
            in_row: false,
            last_script: Script::Western,
            ansicpg: 1252,
            deff: None,
            font_codepages: HashMap::new(),
            font_def: None,
            styles: HashMap::new(),
            style_entry: StyleEntry::default(),
            scan: None,
            pending: Vec::new(),
            pending_cp: 1252,
            high_surrogate: None,
            skip_fallback: 0,
            unsupported_codepages: BTreeSet::new(),
        }
    }

    fn top(&mut self) -> &mut Group {
        let last = self.stack.len() - 1;
        &mut self.stack[last]
    }

    fn mode(&self) -> Mode {
        self.stack[self.stack.len() - 1].mode
    }

    fn run(&mut self, data: &[u8]) -> Result<()> {
        let mut lexer = Lexer { data, pos: 0 };
        while let Some(token) = lexer.next_token() {
            match token {
                Tok::Open => self.open()?,
                Tok::Close => {
                    if self.close() {
                        // The root group is closed; whatever follows it is not part of the document.
                        return self.end_document();
                    }
                }
                Tok::Word(name, param) => {
                    self.flush_bytes();
                    self.word(name, param)?;
                }
                Tok::Sym(c) => {
                    self.flush_bytes();
                    self.symbol(c);
                }
                Tok::Hex(byte) => self.hex(byte),
                Tok::Text(bytes) => self.text(bytes),
            }
        }
        Err(ImportError::format(
            "the RTF file ends unexpectedly: a group is never closed, so the file is cut off or damaged",
        ))
    }

    fn end_document(&mut self) -> Result<()> {
        self.flush_bytes();
        let level = self.level();
        self.place_paragraph(level)?;
        self.layout.leave_to(&mut self.b, 0);
        Ok(())
    }

    fn open(&mut self) -> Result<()> {
        self.flush_bytes();
        if self.stack.len() > self.max_depth {
            return Err(ImportError::format(format!(
                "the RTF file nests its groups more than {} levels deep, so it is damaged or hostile",
                self.max_depth
            )));
        }
        let parent = self.top().clone();
        let mut group = parent.clone();
        group.first = true;
        group.ignorable = false;
        group.close = None;
        if parent.mode == Mode::StyleSheet {
            group.mode = Mode::StyleEntry;
            self.style_entry = StyleEntry::default();
        }
        self.skip_fallback = 0;
        self.stack.push(group);
        Ok(())
    }

    /// Closes a group; returns true when that was the root group of the file.
    fn close(&mut self) -> bool {
        self.flush_bytes();
        self.skip_fallback = 0;
        if self.stack.len() <= 1 {
            return true;
        }
        let Some(group) = self.stack.pop() else { return true };
        if group.mode == Mode::StyleEntry && self.mode() == Mode::StyleSheet {
            let entry = std::mem::take(&mut self.style_entry);
            if let (Some(id), false) = (entry.id, entry.other) {
                self.styles.insert(id, entry);
            }
        }
        match group.close {
            Some(Close::Image) => self.image_placeholder(),
            Some(Close::Scan) => {
                if let Some(scan) = self.scan.take() {
                    if scan.text_box {
                        self.counts.text_boxes += 1;
                    }
                    if scan.picture {
                        self.image_placeholder();
                    }
                    if !scan.text_box && !scan.picture {
                        self.counts.shapes += 1;
                    }
                }
            }
            None => {}
        }
        self.stack.len() == 1
    }

    fn image_placeholder(&mut self) {
        self.counts.images += 1;
        self.buf.push_text("[image]", false, false);
    }

    // --- words ---------------------------------------------------------------------------

    fn word(&mut self, name: &[u8], param: Option<i32>) -> Result<()> {
        if name == b"*" && self.top().first {
            self.top().ignorable = true;
            return Ok(());
        }
        let first = std::mem::replace(&mut self.top().first, false);
        let mode = self.mode();
        if mode == Mode::Skip {
            self.scan_word(name);
            return Ok(());
        }
        if first {
            if mode == Mode::Normal && self.destination(name) {
                return Ok(());
            }
            if self.top().ignorable {
                self.top().mode = Mode::Skip;
                return Ok(());
            }
        }
        match mode {
            Mode::Normal => self.normal_word(name, param)?,
            Mode::FontTable => self.font_word(name, param),
            Mode::StyleEntry => self.style_word(name, param),
            Mode::NestProps => {
                if name == b"nestrow" {
                    self.end_nested_row()?;
                }
            }
            Mode::StyleSheet | Mode::Skip => {}
        }
        Ok(())
    }

    /// Inside a skipped shape, notes what it holds.
    fn scan_word(&mut self, name: &[u8]) {
        if let Some(scan) = self.scan.as_mut() {
            match name {
                b"shptxt" | b"dptxbxtext" => scan.text_box = true,
                b"pict" => scan.picture = true,
                _ => {}
            }
        }
    }

    /// Handles a group's first word when it names a destination. Returns true when it did.
    fn destination(&mut self, name: &[u8]) -> bool {
        match name {
            b"fonttbl" => self.top().mode = Mode::FontTable,
            b"stylesheet" => self.top().mode = Mode::StyleSheet,
            b"nesttableprops" => self.top().mode = Mode::NestProps,
            b"pict" | b"object" => {
                let top = self.top();
                top.mode = Mode::Skip;
                top.close = Some(Close::Image);
            }
            b"shp" | b"do" => {
                if self.scan.is_none() {
                    self.scan = Some(Scan::default());
                    self.top().close = Some(Close::Scan);
                }
                self.top().mode = Mode::Skip;
            }
            b"footnote" => {
                self.counts.footnotes += 1;
                self.top().mode = Mode::Skip;
            }
            b"annotation" => {
                self.counts.comments += 1;
                self.top().mode = Mode::Skip;
            }
            b"mmath" | b"mmathPara" => {
                self.counts.equations += 1;
                self.top().mode = Mode::Skip;
            }
            // The generated label of a list item ("1." or a bullet): the paragraph is a list item.
            b"listtext" | b"pntext" => {
                self.pp.list = true;
                self.top().mode = Mode::Skip;
            }
            // Containers whose content is read like the rest of the body.
            b"shppict" | b"field" | b"fldrslt" => {}
            n if is_skipped_destination(n) => self.top().mode = Mode::Skip,
            _ => return false,
        }
        true
    }

    fn font_word(&mut self, name: &[u8], param: Option<i32>) {
        match (name, param) {
            (b"f", Some(n)) => self.font_def = Some(n),
            (b"fcharset", Some(cs)) => {
                if let (Some(id), Some(cp)) = (self.font_def, charset_codepage(cs)) {
                    self.font_codepages.insert(id, cp);
                }
            }
            (b"cpg", Some(cp)) if cp > 0 => {
                if let Some(id) = self.font_def {
                    self.font_codepages.insert(id, cp as u32);
                }
            }
            _ => {}
        }
    }

    fn style_word(&mut self, name: &[u8], param: Option<i32>) {
        let entry = &mut self.style_entry;
        match (name, param) {
            (b"s", Some(n)) if entry.id.is_none() => entry.id = Some(n),
            (b"cs" | b"ts", _) => entry.other = true,
            (b"sbasedon", Some(n)) => entry.based_on = Some(n),
            (b"outlinelevel", Some(n)) if (0..=8).contains(&n) => entry.outline = Some(((n + 1) as u8).min(6)),
            _ => {}
        }
    }

    fn normal_word(&mut self, name: &[u8], param: Option<i32>) -> Result<()> {
        let on = param != Some(0);
        match name {
            b"b" => self.top().bold = on,
            b"i" => self.top().italic = on,
            b"ab" => self.top().abold = Some(on),
            b"ai" => self.top().aitalic = Some(on),
            b"v" => self.top().hidden = on,
            b"plain" => {
                let deff = self.deff;
                let g = self.top();
                g.bold = false;
                g.italic = false;
                g.abold = None;
                g.aitalic = None;
                g.hidden = false;
                g.font = deff;
            }
            b"f" => self.top().font = param,
            b"uc" => self.top().uc = param.unwrap_or(1).max(0) as u32,
            b"u" => {
                if let Some(n) = param {
                    self.unicode(n);
                }
            }
            b"pard" => self.pp = ParaProps::default(),
            b"s" => self.pp.style = param,
            b"outlinelevel" => {
                self.pp.outline = param.map(|n| if (0..=8).contains(&n) { Some(((n + 1) as u8).min(6)) } else { None });
            }
            b"ls" => {
                if param.is_some_and(|n| n > 0) {
                    self.pp.list = true;
                }
            }
            b"intbl" => self.pp.intbl = true,
            b"itap" => self.pp.itap = param.unwrap_or(0).max(0) as usize,
            b"trowd" => self.in_row = true,
            b"par" | b"sect" | b"page" => {
                let level = self.level();
                self.place_paragraph(level)?;
                self.last_script = Script::Western;
            }
            b"cell" => self.end_cell(1)?,
            b"nestcell" => {
                let level = self.pp.itap.max(2);
                self.end_cell(level)?;
            }
            b"row" => {
                let level = self.level();
                self.place_paragraph(level)?;
                self.layout.leave_to(&mut self.b, 1);
                self.layout.end_row(&mut self.b);
                self.in_row = false;
            }
            b"nestrow" => self.end_nested_row()?,
            b"line" => self.emit("\n"),
            b"tab" => {
                let tab = if self.level() > 0 { " " } else { "\t" };
                self.emit(tab);
            }
            b"ansi" => self.ansicpg = 1252,
            b"mac" => self.ansicpg = 10000,
            b"pc" => self.ansicpg = 437,
            b"pca" => self.ansicpg = 850,
            b"ansicpg" => {
                if let Some(n) = param.filter(|n| *n > 0) {
                    self.ansicpg = n as u32;
                }
            }
            b"deff" => {
                self.deff = param;
                if self.top().font.is_none() {
                    self.top().font = param;
                }
            }
            b"emdash" => self.emit("\u{2014}"),
            b"endash" => self.emit("\u{2013}"),
            b"emspace" => self.emit("\u{2003}"),
            b"enspace" => self.emit("\u{2002}"),
            b"qmspace" => self.emit("\u{2005}"),
            b"bullet" => self.emit("\u{2022}"),
            b"lquote" => self.emit("\u{2018}"),
            b"rquote" => self.emit("\u{2019}"),
            b"ldblquote" => self.emit("\u{201C}"),
            b"rdblquote" => self.emit("\u{201D}"),
            b"zwj" => self.emit("\u{200D}"),
            b"zwnj" => self.emit("\u{200C}"),
            b"ltrmark" => self.emit("\u{200E}"),
            b"rtlmark" => self.emit("\u{200F}"),
            _ => {}
        }
        Ok(())
    }

    fn symbol(&mut self, c: u8) {
        if self.mode() != Mode::Normal {
            return;
        }
        match c {
            b'\\' => self.emit("\\"),
            b'{' => self.emit("{"),
            b'}' => self.emit("}"),
            b'~' => self.emit("\u{00A0}"),
            // A non-breaking hyphen reads as a plain hyphen.
            b'_' => self.emit("-"),
            _ => {}
        }
    }

    /// `\uN`: a UTF-16 code unit, written as a signed 16-bit number, followed by fallback characters.
    fn unicode(&mut self, n: i32) {
        let code = if n < 0 { i64::from(n) + 65536 } else { i64::from(n) };
        let code = code.max(0) as u32;
        let skip = self.top().uc;
        match code {
            0xD800..=0xDBFF => {
                if self.high_surrogate.is_some() {
                    self.emit("\u{FFFD}");
                }
                self.high_surrogate = Some(code);
            }
            0xDC00..=0xDFFF => {
                let ch = self
                    .high_surrogate
                    .take()
                    .and_then(|hi| char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (code - 0xDC00)))
                    .unwrap_or('\u{FFFD}');
                let mut buf = [0u8; 4];
                self.emit(ch.encode_utf8(&mut buf));
            }
            _ => {
                let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
                let mut buf = [0u8; 4];
                self.emit(ch.encode_utf8(&mut buf));
            }
        }
        if self.mode() == Mode::Normal {
            self.skip_fallback = skip;
        }
    }

    // --- text ----------------------------------------------------------------------------

    /// The code page for `\'hh` bytes: the current font's, or else the document's.
    fn current_codepage(&mut self) -> u32 {
        let font = self.top().font.or(self.deff);
        font.and_then(|f| self.font_codepages.get(&f).copied()).unwrap_or(self.ansicpg)
    }

    fn hex(&mut self, byte: u8) {
        if self.skip_fallback > 0 {
            self.skip_fallback -= 1;
            return;
        }
        if self.mode() != Mode::Normal {
            return;
        }
        self.push_byte(byte);
    }

    fn push_byte(&mut self, byte: u8) {
        if self.pending.is_empty() {
            self.pending_cp = self.current_codepage();
        }
        self.pending.push(byte);
    }

    /// Decodes the bytes that wait, as one piece, so multi-byte characters stay whole.
    fn flush_bytes(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let bytes = std::mem::take(&mut self.pending);
        let encoding = codepage_encoding(self.pending_cp).unwrap_or_else(|| {
            self.unsupported_codepages.insert(self.pending_cp);
            encoding_rs::WINDOWS_1252
        });
        let (text, _) = encoding.decode_without_bom_handling(&bytes);
        let text = text.into_owned();
        self.emit(&text);
    }

    fn text(&mut self, mut bytes: &[u8]) {
        // The characters after \uN are a fallback for readers without Unicode; skip them.
        while self.skip_fallback > 0 && !bytes.is_empty() {
            bytes = &bytes[1..];
            self.skip_fallback -= 1;
        }
        match self.mode() {
            Mode::Normal => {
                let mut start = 0;
                for (i, &b) in bytes.iter().enumerate() {
                    if b >= 0x80 {
                        if i > start {
                            self.flush_bytes();
                            self.emit(&String::from_utf8_lossy(&bytes[start..i]));
                        }
                        self.push_byte(b);
                        start = i + 1;
                    }
                }
                if start < bytes.len() {
                    self.flush_bytes();
                    self.emit(&String::from_utf8_lossy(&bytes[start..]));
                }
            }
            Mode::StyleEntry => {
                let entry = &mut self.style_entry;
                for &b in bytes {
                    if b == b';' {
                        entry.name_done = true;
                    } else if !entry.name_done && entry.name.len() < 200 {
                        entry.name.push(char::from(b));
                    }
                }
            }
            _ => {}
        }
    }

    /// Adds text to the paragraph being built, styled as the current group says.
    fn emit(&mut self, text: &str) {
        if self.high_surrogate.take().is_some() {
            self.emit_clean("\u{FFFD}");
        }
        self.emit_clean(text);
    }

    fn emit_clean(&mut self, text: &str) {
        if text.is_empty() || self.mode() != Mode::Normal {
            return;
        }
        let script = script_of(text).unwrap_or(self.last_script);
        self.last_script = script;
        let g = &self.stack[self.stack.len() - 1];
        if g.hidden {
            if text.chars().any(|c| !c.is_whitespace()) {
                self.counts.hidden += 1;
            }
            return;
        }
        let complex = script == Script::Complex;
        let bold = if complex { g.abold.unwrap_or(g.bold) } else { g.bold };
        let italic = if complex { g.aitalic.unwrap_or(g.italic) } else { g.italic };
        // Inside a table a tab would be mistaken for a cell separator.
        if self.level() > 0 && text.contains('\t') {
            self.buf.push_text(&text.replace('\t', " "), bold, italic);
        } else {
            self.buf.push_text(text, bold, italic);
        }
    }

    // --- paragraphs and tables ---------------------------------------------------------------

    /// How many tables deep the current paragraph is.
    fn level(&self) -> usize {
        if self.pp.intbl || self.in_row {
            self.pp.itap.max(1)
        } else {
            0
        }
    }

    fn style_heading(&self, id: i32) -> Option<u8> {
        let mut cur = Some(id);
        for _ in 0..MAX_STYLE_CHAIN {
            let entry = self.styles.get(&cur?)?;
            if let Some(level) = level_from_name(&entry.name).or(entry.outline) {
                return Some(level);
            }
            cur = entry.based_on;
        }
        None
    }

    /// Whether the paragraph being built is a heading (and which level) or a list item.
    fn classify(&self) -> (Option<u8>, bool) {
        let heading = match self.pp.outline {
            Some(direct) => direct,
            None => self.pp.style.and_then(|id| self.style_heading(id)),
        };
        (heading, self.pp.list && heading.is_none())
    }

    /// Puts the finished paragraph into the text, at table nesting `level`.
    fn place_paragraph(&mut self, level: usize) -> Result<()> {
        if self.buf.has_text() || level == 0 {
            self.layout.enter(&mut self.b, level)?;
        }
        let (heading, list) = self.classify();
        self.layout.place(&mut self.b, &mut self.buf, heading, list);
        Ok(())
    }

    fn end_cell(&mut self, level: usize) -> Result<()> {
        self.place_paragraph(level)?;
        self.layout.enter(&mut self.b, level)?;
        self.layout.end_cell(&mut self.b);
        Ok(())
    }

    fn end_nested_row(&mut self) -> Result<()> {
        let level = self.pp.itap.max(2);
        self.place_paragraph(level)?;
        self.layout.leave_to(&mut self.b, level);
        self.layout.end_row(&mut self.b);
        Ok(())
    }

    fn finish(mut self) -> ParsedText {
        let mut warnings = self.counts.warnings();
        if !self.unsupported_codepages.is_empty() {
            let list: Vec<String> = self.unsupported_codepages.iter().map(u32::to_string).collect();
            warnings.push(ImportWarning::new(
                "unsupported_codepage",
                format!(
                    "Some text uses the old code page {} that cannot be decoded exactly; it was read as Windows-1252, so some characters may be wrong.",
                    list.join(", ")
                ),
            ));
        }
        for w in warnings {
            self.b.warn(w);
        }
        let mut parsed = self.b.finish(SourceType::Document, UnitKind::Sentence);
        sort_runs(&mut parsed);
        parsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders the tokens of `data` in a compact form for comparison.
    fn lex(data: &[u8]) -> Vec<String> {
        let mut lexer = Lexer { data, pos: 0 };
        let mut out = Vec::new();
        while let Some(t) = lexer.next_token() {
            out.push(match t {
                Tok::Open => "{".to_string(),
                Tok::Close => "}".to_string(),
                Tok::Word(n, None) => format!("\\{}", String::from_utf8_lossy(n)),
                Tok::Word(n, Some(p)) => format!("\\{}={p}", String::from_utf8_lossy(n)),
                Tok::Sym(c) => format!("sym {}", char::from(c)),
                Tok::Hex(b) => format!("hex {b:02x}"),
                Tok::Text(t) => format!("text {}", String::from_utf8_lossy(t)),
            });
        }
        out
    }

    #[test]
    fn control_words_take_a_number_and_one_delimiting_space() {
        assert_eq!(lex(br"\b\i0 text"), ["\\b", "\\i=0", "text text"]);
        assert_eq!(lex(br"\fs24  two spaces"), ["\\fs=24", "text  two spaces"]);
        assert_eq!(lex(br"\li-720 x"), ["\\li=-720", "text x"]);
        assert_eq!(lex(br"\par-"), ["\\par", "text -"]);
        assert_eq!(lex(br"\u-10179?"), ["\\u=-10179", "text ?"]);
    }

    #[test]
    fn numbers_too_large_for_the_type_are_clamped_not_wrapped() {
        assert_eq!(lex(br"\li99999999999999999999 x"), [format!("\\li={}", i32::MAX), "text x".to_string()]);
    }

    #[test]
    fn symbols_hex_escapes_and_line_ends() {
        assert_eq!(lex(br"\{\}\\\~\-"), ["sym {", "sym }", "sym \\", "sym ~", "sym -"]);
        assert_eq!(lex(br"\'e9\'4A"), ["hex e9", "hex 4a"]);
        assert_eq!(lex(br"\'zz"), ["sym '", "text zz"], "an escape without two hex digits is not a byte");
        assert_eq!(lex(b"a\r\nb\\\r\nc"), ["text a", "text b", "\\par", "text c"]);
        assert_eq!(lex(b"ab\\"), ["text ab"], "a backslash at the very end is ignored");
        assert_eq!(lex(br"\*\generator"), ["\\*", "\\generator"]);
    }

    #[test]
    fn binary_data_is_skipped_by_count_whatever_it_holds() {
        assert_eq!(lex(b"\\bin6 }{\\ab}z"), ["\\bin=6", "text z"]);
        // A count that runs past the end just ends the data; it cannot loop or panic.
        assert_eq!(lex(b"\\bin99999999999 {{{"), ["\\bin=2147483647"]);
        assert_eq!(lex(b"\\bin0 {x"), ["\\bin=0", "{", "text x"]);
    }

    #[test]
    fn nesting_beyond_the_limit_is_refused_and_inside_it_is_fine() {
        let deep = |n: usize| format!("{{\\rtf1 {}x{}}}", "{".repeat(n), "}".repeat(n));
        assert_eq!(parse_with_depth(deep(10).as_bytes(), 20).unwrap().text, "x");
        let err = parse_with_depth(deep(30).as_bytes(), 20).unwrap_err().to_string();
        assert!(err.contains("more than 20 levels"), "{err}");
        // The real limit holds up against a hundred thousand braces.
        let hostile = format!("{{\\rtf1 {}", "{".repeat(100_000));
        assert!(parse(hostile.as_bytes(), &ImportOptions::default()).is_err());
    }

    #[test]
    fn a_file_that_is_cut_off_is_refused() {
        let err = parse(br"{\rtf1\ansi {\b bold text", &ImportOptions::default()).unwrap_err().to_string();
        assert!(err.contains("ends unexpectedly"), "{err}");
    }

    #[test]
    fn style_chains_that_loop_stop() {
        let file = br"{\rtf1{\stylesheet{\s1\sbasedon2 A;}{\s2\sbasedon1 B;}}\pard\s1 text\par}";
        assert_eq!(parse(file, &ImportOptions::default()).unwrap().text, "text");
    }

    #[test]
    fn code_pages_have_decoders_and_charsets_map_to_code_pages() {
        for cp in [874, 932, 936, 949, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258, 10000, 65001] {
            assert!(codepage_encoding(cp).is_some(), "code page {cp}");
        }
        assert!(codepage_encoding(437).is_none());
        assert_eq!(charset_codepage(134), Some(936));
        assert_eq!(charset_codepage(0), None);
        assert_eq!(level_from_name("Heading 3"), Some(3));
        assert_eq!(level_from_name("heading 12"), None);
    }
}
