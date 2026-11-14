//! SPSS .sav and .zsav import (specification section 5).
//!
//! The reader is written from the public description of the SPSS system-file layout and from
//! what files made by an independent writer (the ReadStat library, through the R package haven)
//! look like. No code from other readers was used. The layout, in short:
//!
//! * A 176-byte header: `$FL2` (or `$FL3` for zlib-compressed data), a layout code that also tells
//!   the byte order, the compression method, the weight variable, and the number of cases.
//! * Dictionary records: variables (type 2), value labels (types 3 and 4), a document (type 6),
//!   extension records (type 7), and an end marker (type 999).
//! * The data: one run of 8-byte values per case, stored plainly (compression 0), as bytecodes
//!   (compression 1), or as bytecodes inside zlib blocks (compression 2).
//!
//! Hostile or damaged files return a plain-language error. Every length is checked against the
//! bytes that are really there, nothing is allocated from a count before that check, and the
//! table size is capped.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use encoding_rs::Encoding;
use prismqda_engine::{ImportWarning, Level};

use crate::error::{ImportError, Result};
use crate::table::{Cell, Column, Table};

const HEADER_LEN: usize = 176;
/// The most cells (cases times variables) that will be loaded. About a gigabyte at the very most.
const MAX_CELLS: usize = 30_000_000;
/// The most label entries that may be copied onto variables (a label set shared by many variables
/// is copied once per variable).
const MAX_LABEL_WORK: usize = 4_000_000;
/// The most variables (including the parts of very long texts) a file may declare.
const MAX_VARIABLES: usize = 200_000;
/// The longest text variable SPSS allows.
const MAX_STRING_BYTES: usize = 32_767;
/// A very long text of `n` bytes is stored as ceil(n / 252) segments (all but the last 255 bytes wide).
const SEGMENT_BYTES: usize = 252;
/// The most bytes one zlib block of data may unpack to.
const MAX_BLOCK_BYTES: usize = 64 * 1024 * 1024;
/// Seconds from 1582-10-14 (the SPSS day zero) to 1970-01-01.
const SPSS_TO_UNIX_SECONDS: i64 = 12_219_379_200;
/// Days from 1582-10-14 to 1970-01-01.
const SPSS_TO_UNIX_DAYS: i64 = 141_428;

/// One 8-byte value of a case, exactly as the file holds it.
type Unit = [u8; 8];

fn cut_short(what: &str) -> ImportError {
    ImportError::format(format!(
        "the SPSS file ends too soon ({what}); it may be damaged or only partly copied"
    ))
}

fn damaged(what: impl std::fmt::Display) -> ImportError {
    ImportError::format(format!("the SPSS file looks damaged ({what})"))
}

fn not_spss() -> ImportError {
    ImportError::format("this is not an SPSS data file (.sav); it may be damaged or saved in a different format")
}

/// Reads a .sav (or zlib-compressed .zsav) file into a table: one column per variable (with its
/// label, value labels, missing values, and level of measurement) and one row per case. Value
/// labels stay as labels; cells hold the raw values.
pub fn read_table(bytes: &[u8]) -> Result<Table> {
    let header = parse_header(bytes)?;
    let mut cur = Cur::new(bytes, HEADER_LEN, header.le);
    let dict = read_dictionary(&mut cur)?;
    if dict.vars.is_empty() {
        return Err(ImportError::format("the SPSS file has no variables to import"));
    }
    if let Some(rep) = dict.floating_rep {
        if rep != 1 {
            return Err(ImportError::unsupported(
                "this SPSS file stores numbers in an old IBM or VAX format, which PrismQDA cannot read; \
                 open it in SPSS and save it again",
            ));
        }
    }

    let mut warnings: Vec<ImportWarning> = Vec::new();
    let mut dec = Decoder::new(choose_encoding(&dict, &mut warnings)?);
    let model = build_model(&dict, &header, &mut dec, &mut warnings)?;
    let (sysmis, _, _) = float_limits(&dict);

    let rows = read_cases(&mut cur, &header, &dict, &model, sysmis, &mut dec, &mut warnings)?;
    if rows.is_empty() {
        warnings.push(ImportWarning::new("no_cases", "the file lists its variables but holds no cases"));
    }
    if dec.lossy {
        warnings.push(ImportWarning::new(
            "text_decoding_errors",
            "some text in the file could not be read in the file's own character set and was replaced by a placeholder character",
        ));
    }
    Ok(Table { name: String::new(), columns: model.columns, rows, warnings })
}

// ---- reading bytes safely -------------------------------------------------------------------------

struct Cur<'a> {
    b: &'a [u8],
    pos: usize,
    le: bool,
}

impl<'a> Cur<'a> {
    fn new(b: &'a [u8], pos: usize, le: bool) -> Self {
        Cur { b, pos, le }
    }

    fn left(&self) -> usize {
        self.b.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        if n > self.left() {
            return Err(cut_short(what));
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn arr<const N: usize>(&mut self, what: &str) -> Result<[u8; N]> {
        let s = self.take(N, what)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }

    fn i32(&mut self, what: &str) -> Result<i32> {
        let a = self.arr::<4>(what)?;
        Ok(if self.le { i32::from_le_bytes(a) } else { i32::from_be_bytes(a) })
    }

    fn u32(&mut self, what: &str) -> Result<u32> {
        let a = self.arr::<4>(what)?;
        Ok(if self.le { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) })
    }

    fn i64(&mut self, what: &str) -> Result<i64> {
        let a = self.arr::<8>(what)?;
        Ok(if self.le { i64::from_le_bytes(a) } else { i64::from_be_bytes(a) })
    }

    fn f64(&mut self, what: &str) -> Result<f64> {
        let a = self.arr::<8>(what)?;
        Ok(unit_to_f64(self.le, a))
    }
}

fn unit_to_f64(le: bool, a: Unit) -> f64 {
    if le {
        f64::from_le_bytes(a)
    } else {
        f64::from_be_bytes(a)
    }
}

fn f64_to_unit(le: bool, v: f64) -> Unit {
    if le {
        v.to_le_bytes()
    } else {
        v.to_be_bytes()
    }
}

fn int_at(le: bool, data: &[u8], index: usize) -> Option<i32> {
    let s = data.get(index * 4..index * 4 + 4)?;
    let mut a = [0u8; 4];
    a.copy_from_slice(s);
    Some(if le { i32::from_le_bytes(a) } else { i32::from_be_bytes(a) })
}

fn to_usize(v: i64) -> Option<usize> {
    usize::try_from(v).ok()
}

/// The bytes without the spaces (and NUL padding) at the end.
fn trim_end_bytes(b: &[u8]) -> &[u8] {
    let mut n = b.len();
    while n > 0 && (b[n - 1] == b' ' || b[n - 1] == 0) {
        n -= 1;
    }
    &b[..n]
}

fn upper(b: &[u8]) -> Vec<u8> {
    b.to_ascii_uppercase()
}

// ---- the header -------------------------------------------------------------------------------------

struct Header {
    le: bool,
    compression: i32,
    /// 1-based position of the weight variable among the 8-byte slots, or 0.
    weight_slot: i32,
    /// Negative when the file does not say.
    ncases: i32,
    bias: f64,
}

fn parse_header(b: &[u8]) -> Result<Header> {
    if !(b.starts_with(b"$FL2") || b.starts_with(b"$FL3")) {
        return Err(not_spss());
    }
    if b.len() < HEADER_LEN {
        return Err(cut_short("the file header"));
    }
    let layout = |le: bool| {
        let mut a = [0u8; 4];
        a.copy_from_slice(&b[64..68]);
        if le {
            i32::from_le_bytes(a)
        } else {
            i32::from_be_bytes(a)
        }
    };
    let le = if matches!(layout(true), 2 | 3) {
        true
    } else if matches!(layout(false), 2 | 3) {
        false
    } else {
        return Err(not_spss());
    };
    let mut cur = Cur::new(b, 68, le);
    let _case_size = cur.i32("the file header")?;
    let compression = cur.i32("the file header")?;
    let weight_slot = cur.i32("the file header")?;
    let ncases = cur.i32("the file header")?;
    let bias = cur.f64("the file header")?;
    if !(0..=2).contains(&compression) {
        return Err(ImportError::unsupported(
            "this SPSS file uses a compression method PrismQDA does not read; open it in SPSS and save it again",
        ));
    }
    if !bias.is_finite() {
        return Err(damaged("the header is unreadable"));
    }
    Ok(Header { le, compression, weight_slot, ncases, bias })
}

// ---- the dictionary ---------------------------------------------------------------------------------

struct RawVar<'a> {
    name: &'a [u8],
    /// 0 for a number, otherwise the text width in bytes.
    width: usize,
    /// Index of the variable's first 8-byte slot.
    first_slot: usize,
    print: u32,
    label: Option<&'a [u8]>,
    /// As the file wrote it: 0 to 3 single values, -2 a range, -3 a range and one value.
    miss_kind: i32,
    miss: Vec<Unit>,
}

struct LabelSet<'a> {
    entries: Vec<(Unit, &'a [u8])>,
    /// 1-based slot numbers of the variables that share the labels.
    slots: Vec<i32>,
}

#[derive(Default)]
struct Dict<'a> {
    vars: Vec<RawVar<'a>>,
    nslots: usize,
    label_sets: Vec<LabelSet<'a>>,
    label_work: usize,
    code_page: Option<i32>,
    floating_rep: Option<i32>,
    encoding_name: Option<&'a [u8]>,
    /// System-missing, highest, and lowest.
    float_info: Option<(f64, f64, f64)>,
    display: Option<Vec<i32>>,
    long_names: Vec<&'a [u8]>,
    very_long: Vec<&'a [u8]>,
    long_labels: Vec<&'a [u8]>,
    long_missing: Vec<&'a [u8]>,
    ncases64: Option<i64>,
}

fn read_dictionary<'a>(cur: &mut Cur<'a>) -> Result<Dict<'a>> {
    let mut d = Dict::default();
    loop {
        let rt = cur.i32("the list of variables")?;
        match rt {
            2 => read_variable(cur, &mut d)?,
            3 => read_label_set(cur, &mut d)?,
            6 => {
                let lines = cur.i32("a document record")?;
                let bytes = usize::try_from(lines).ok().and_then(|n| n.checked_mul(80));
                match bytes {
                    Some(n) if n <= cur.left() => {
                        cur.take(n, "a document record")?;
                    }
                    _ => return Err(damaged("a document record has an impossible size")),
                }
            }
            7 => read_extension(cur, &mut d)?,
            999 => {
                cur.i32("the end of the variable list")?;
                return Ok(d);
            }
            4 => return Err(damaged("a value-label list has no labels before it")),
            other => return Err(damaged(format!("it holds a record of an unknown kind ({other})"))),
        }
    }
}

fn read_variable<'a>(cur: &mut Cur<'a>, d: &mut Dict<'a>) -> Result<()> {
    const WHAT: &str = "a variable record";
    let ty = cur.i32(WHAT)?;
    let has_label = cur.i32(WHAT)?;
    let nmiss = cur.i32(WHAT)?;
    let print = cur.u32(WHAT)?;
    let _write = cur.u32(WHAT)?;
    let name = cur.take(8, WHAT)?;
    if ty == -1 {
        return Err(damaged("a text variable's continuation record appears with no text variable before it"));
    }
    let width = usize::try_from(ty).ok().filter(|w| *w <= 255).ok_or_else(|| damaged("a variable has an impossible type"))?;

    let label = match has_label {
        0 => None,
        1 => {
            let len = cur.i32(WHAT)?;
            let len = usize::try_from(len).map_err(|_| damaged("a variable label has a negative length"))?;
            let text = cur.take(len, "a variable label")?;
            cur.take((4 - len % 4) % 4, "a variable label")?;
            Some(text)
        }
        _ => return Err(damaged("a variable record has an impossible label flag")),
    };

    let count = match nmiss {
        0 => 0,
        1..=3 => nmiss as usize,
        -2 => 2,
        -3 => 3,
        _ => return Err(damaged("a variable declares an impossible number of missing values")),
    };
    let mut miss = Vec::with_capacity(count);
    for _ in 0..count {
        miss.push(cur.arr::<8>("a missing value")?);
    }

    if d.vars.len() >= MAX_VARIABLES {
        return Err(ImportError::format("the SPSS file declares more variables than this version can load"));
    }
    let slots = if width == 0 { 1 } else { width.div_ceil(8) };
    let first_slot = d.nslots;
    for _ in 1..slots {
        if cur.i32(WHAT)? != 2 || cur.i32(WHAT)? != -1 {
            return Err(damaged("a text variable is not followed by the continuation records it needs"));
        }
        cur.take(24, WHAT)?;
    }
    d.nslots += slots;
    d.vars.push(RawVar { name, width, first_slot, print, label, miss_kind: nmiss, miss });
    Ok(())
}

fn read_label_set<'a>(cur: &mut Cur<'a>, d: &mut Dict<'a>) -> Result<()> {
    const WHAT: &str = "a list of value labels";
    let n = cur.i32(WHAT)?;
    let n = usize::try_from(n).map_err(|_| damaged("a list of value labels has a negative count"))?;
    // Each label takes at least 16 bytes: 8 for the value and 8 for the length byte, text, and padding.
    if n > cur.left() / 16 {
        return Err(cut_short(WHAT));
    }
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let key = cur.arr::<8>(WHAT)?;
        let len = usize::from(cur.arr::<1>(WHAT)?[0]);
        let label = cur.take(len, WHAT)?;
        cur.take((8 - (1 + len) % 8) % 8, WHAT)?;
        entries.push((key, label));
    }
    if cur.i32(WHAT)? != 4 {
        return Err(damaged("a list of value labels is not followed by the variables it belongs to"));
    }
    let nv = cur.i32(WHAT)?;
    let nv = usize::try_from(nv).map_err(|_| damaged("a list of value labels names a negative number of variables"))?;
    if nv > cur.left() / 4 {
        return Err(cut_short(WHAT));
    }
    let mut slots = Vec::with_capacity(nv);
    for _ in 0..nv {
        slots.push(cur.i32(WHAT)?);
    }
    d.label_work = d.label_work.saturating_add(n.saturating_mul(nv.max(1)));
    if d.label_work > MAX_LABEL_WORK {
        return Err(ImportError::format("the SPSS file carries more value labels than this version can load"));
    }
    d.label_sets.push(LabelSet { entries, slots });
    Ok(())
}

fn read_extension<'a>(cur: &mut Cur<'a>, d: &mut Dict<'a>) -> Result<()> {
    const WHAT: &str = "an extension record";
    let subtype = cur.i32(WHAT)?;
    let size = cur.i32(WHAT)?;
    let count = cur.i32(WHAT)?;
    let (Ok(size), Ok(count)) = (usize::try_from(size), usize::try_from(count)) else {
        return Err(damaged("an extension record has a negative size"));
    };
    let total = (size as u64) * (count as u64);
    if total > cur.left() as u64 {
        return Err(cut_short(WHAT));
    }
    let data = cur.take(total as usize, WHAT)?;
    let le = cur.le;
    match subtype {
        // Integer info: version, machine, floating-point form, compression, byte order, code page.
        3 if size == 4 && count >= 8 => {
            d.floating_rep = int_at(le, data, 4);
            d.code_page = int_at(le, data, 7);
        }
        // Floating-point info: system-missing, highest, lowest.
        4 if size == 8 && count >= 3 => {
            let at = |i: usize| {
                let mut a = [0u8; 8];
                a.copy_from_slice(&data[i * 8..i * 8 + 8]);
                unit_to_f64(le, a)
            };
            d.float_info = Some((at(0), at(1), at(2)));
        }
        // Variable display parameters: measure, width, alignment for each variable.
        11 if size == 4 => {
            d.display = Some((0..count).filter_map(|i| int_at(le, data, i)).collect());
        }
        13 if size == 1 => d.long_names.push(data),
        14 if size == 1 => d.very_long.push(data),
        // Number of cases as a 64-bit number, for files with more than 2^31 cases.
        16 if size == 8 && count >= 2 => {
            let mut a = [0u8; 8];
            a.copy_from_slice(&data[8..16]);
            d.ncases64 = Some(if le { i64::from_le_bytes(a) } else { i64::from_be_bytes(a) });
        }
        20 if size == 1 => d.encoding_name = Some(data),
        21 if size == 1 => d.long_labels.push(data),
        22 if size == 1 => d.long_missing.push(data),
        _ => {}
    }
    Ok(())
}

// ---- text encoding ------------------------------------------------------------------------------------

struct Decoder {
    enc: &'static Encoding,
    lossy: bool,
}

impl Decoder {
    fn new(enc: &'static Encoding) -> Self {
        Decoder { enc, lossy: false }
    }

    /// Decodes text, dropping the padding (spaces and NULs) at its end.
    fn text(&mut self, b: &[u8]) -> String {
        let (s, bad) = self.enc.decode_without_bom_handling(trim_end_bytes(b));
        if bad {
            self.lossy = true;
        }
        s.into_owned()
    }
}

fn choose_encoding(d: &Dict, warnings: &mut Vec<ImportWarning>) -> Result<&'static Encoding> {
    let mut stated: Option<String> = None;
    if let Some(name) = d.encoding_name {
        let name = String::from_utf8_lossy(trim_end_bytes(name)).trim().to_string();
        if !name.is_empty() {
            if let Some(e) = encoding_by_name(&name) {
                return Ok(e);
            }
            stated = Some(name);
        }
    }
    if let Some(cp) = d.code_page {
        if cp == 1 {
            return Err(ImportError::unsupported(
                "this SPSS file stores its text in EBCDIC, which PrismQDA cannot read; open it in SPSS and save it again",
            ));
        }
        if let Some(e) = encoding_for_code_page(cp) {
            return Ok(e);
        }
        stated.get_or_insert_with(|| format!("code page {cp}"));
    }
    if let Some(name) = stated {
        warnings.push(ImportWarning::new(
            "encoding_assumed",
            format!(
                "the file says its text is stored as '{name}', which PrismQDA does not know; the text was read as Windows-1252, \
                 so accented or non-Latin letters may look wrong"
            ),
        ));
    }
    Ok(encoding_rs::WINDOWS_1252)
}

fn encoding_by_name(name: &str) -> Option<&'static Encoding> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "ms932" | "cp932" | "windows-31j" => Some(encoding_rs::SHIFT_JIS),
        "cp936" | "ms936" => Some(encoding_rs::GBK),
        "cp949" | "ms949" => Some(encoding_rs::EUC_KR),
        "cp950" | "ms950" => Some(encoding_rs::BIG5),
        _ => Encoding::for_label(lower.as_bytes()),
    }
}

fn encoding_for_code_page(cp: i32) -> Option<&'static Encoding> {
    Some(match cp {
        65001 => encoding_rs::UTF_8,
        // 2 and 3 are SPSS's own codes for 7-bit and 8-bit ASCII; 20127 is the Windows number for ASCII.
        2 | 3 | 20127 => encoding_rs::WINDOWS_1252,
        1250..=1258 | 874 => return Encoding::for_label(format!("windows-{cp}").as_bytes()),
        28591..=28599 | 28603 | 28605 => return Encoding::for_label(format!("iso-8859-{}", cp - 28590).as_bytes()),
        932 => encoding_rs::SHIFT_JIS,
        936 => encoding_rs::GBK,
        949 => encoding_rs::EUC_KR,
        950 => encoding_rs::BIG5,
        20932 | 51932 => encoding_rs::EUC_JP,
        54936 => encoding_rs::GB18030,
        20866 => encoding_rs::KOI8_R,
        21866 => encoding_rs::KOI8_U,
        10000 => encoding_rs::MACINTOSH,
        866 => encoding_rs::IBM866,
        _ => return None,
    })
}

// ---- turning the dictionary into columns ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum DateKind {
    /// Not a date.
    No,
    /// DATE, ADATE, EDATE, SDATE, JDATE, MOYR, QYR, WKYR: shown as a day.
    Day,
    /// DATETIME and YMDHMS.
    DayTime,
    /// TIME, DTIME, MTIME: a length of time in seconds, not a point in time.
    Duration,
}

fn date_kind(print: u32) -> DateKind {
    match (print >> 16) & 0xff {
        20 | 23 | 24 | 28 | 29 | 30 | 38 | 39 => DateKind::Day,
        22 | 41 => DateKind::DayTime,
        21 | 25 | 40 => DateKind::Duration,
        _ => DateKind::No,
    }
}

struct Seg {
    slot: usize,
    /// How many bytes of the segment carry text (it may be 0 for a spare last segment).
    take: usize,
}

enum Plan {
    Number { slot: usize, date: DateKind },
    Text { segs: Vec<Seg> },
}

struct Model {
    columns: Vec<Column>,
    plans: Vec<Plan>,
}

/// A variable as the person sees it. A very long text is several variable records but one variable.
struct LogicalVar {
    head: usize,
    plan: Plan,
}

fn list_names(names: &[String]) -> String {
    let shown: Vec<String> = names.iter().take(5).map(|n| format!("'{n}'")).collect();
    if names.len() > 5 {
        format!("{} and {} more", shown.join(", "), names.len() - 5)
    } else {
        shown.join(", ")
    }
}

/// A number as the importer writes value-label keys and missing codes: whole numbers have no decimal point.
fn number_text(v: f64, highest: f64, lowest: f64) -> String {
    if v >= highest {
        "HIGHEST".to_string()
    } else if v <= lowest {
        "LOWEST".to_string()
    } else {
        Cell::Number(v).text()
    }
}

/// Splits a text record into `key=value` entries. SPSS separates entries with a tab, and may end each with a NUL.
fn key_values(record: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    record.split(|b| *b == 9).filter_map(|entry| {
        let entry = trim_end_bytes(entry);
        let eq = entry.iter().position(|b| *b == b'=')?;
        Some((&entry[..eq], &entry[eq + 1..]))
    })
}

fn parse_very_long(records: &[&[u8]]) -> Result<HashMap<Vec<u8>, usize>> {
    let mut map = HashMap::new();
    for rec in records {
        for (key, value) in key_values(rec) {
            let digits = trim_end_bytes(value);
            let Some(len) = std::str::from_utf8(digits).ok().and_then(|s| s.trim().parse::<usize>().ok()) else {
                continue;
            };
            if len > MAX_STRING_BYTES {
                return Err(damaged("a text variable claims to be longer than SPSS allows"));
            }
            map.insert(upper(trim_end_bytes(key)), len);
        }
    }
    Ok(map)
}

/// The system-missing, highest, and lowest values. The file states them, but some writers state them
/// wrongly (one old writer gives NaN and swaps highest and lowest), so a stated value is used only when
/// it is plausible: at the very end of the range of doubles.
fn float_limits(dict: &Dict) -> (f64, f64, f64) {
    let (s, h, l) = dict.float_info.unwrap_or((f64::MIN, f64::MAX, f64::MIN));
    (
        if s.is_finite() && s < -1e300 { s } else { f64::MIN },
        if h.is_finite() && h > 1e300 { h } else { f64::MAX },
        if l.is_finite() && l < -1e300 { l } else { f64::MIN },
    )
}

fn build_model(dict: &Dict, header: &Header, dec: &mut Decoder, warnings: &mut Vec<ImportWarning>) -> Result<Model> {
    let le = header.le;
    let (_, highest, lowest) = float_limits(dict);

    // Group the variable records into variables.
    let very_long = parse_very_long(&dict.very_long)?;
    let mut lvars: Vec<LogicalVar> = Vec::new();
    let mut i = 0usize;
    while i < dict.vars.len() {
        let v = &dict.vars[i];
        let mut used = 1usize;
        let plan = if v.width == 0 {
            Plan::Number { slot: v.first_slot, date: date_kind(v.print) }
        } else {
            let len = very_long.get(&upper(trim_end_bytes(v.name))).copied().filter(|n| *n > 255);
            let mut segs = Vec::new();
            match len {
                Some(len) => {
                    // The count of segments follows the 252 rule, but the text is packed into each
                    // full segment's whole width (255 bytes), so the text is read segment by segment
                    // until `len` bytes are in hand.
                    let n = len.div_ceil(SEGMENT_BYTES);
                    if n > dict.vars.len() - i {
                        return Err(damaged("a very long text variable is missing some of its parts"));
                    }
                    let mut remaining = len;
                    for k in 0..n {
                        let part = &dict.vars[i + k];
                        if part.width == 0 {
                            return Err(damaged("a very long text variable has a part that is not text"));
                        }
                        let take = remaining.min(part.width);
                        remaining -= take;
                        segs.push(Seg { slot: part.first_slot, take });
                    }
                    if remaining != 0 {
                        return Err(damaged("a very long text variable has parts that are too small for its length"));
                    }
                    used = n;
                }
                None => segs.push(Seg { slot: v.first_slot, take: v.width }),
            }
            Plan::Text { segs }
        };
        lvars.push(LogicalVar { head: i, plan });
        i += used;
    }

    // Names: the long name when the file has one.
    let mut long_names: HashMap<Vec<u8>, String> = HashMap::new();
    for rec in &dict.long_names {
        for (short, long) in key_values(rec) {
            long_names.insert(upper(trim_end_bytes(short)), dec.text(long));
        }
    }
    let mut names: Vec<String> = Vec::with_capacity(lvars.len());
    let mut taken: HashSet<String> = HashSet::new();
    let mut renamed: Vec<String> = Vec::new();
    for (k, lv) in lvars.iter().enumerate() {
        let short = trim_end_bytes(dict.vars[lv.head].name);
        let mut name = long_names.get(&upper(short)).cloned().unwrap_or_else(|| dec.text(short));
        if name.trim().is_empty() {
            name = format!("V{}", k + 1);
        }
        if taken.contains(&name) {
            let base = name.clone();
            let mut n = 2;
            while taken.contains(&format!("{base}_{n}")) {
                n += 1;
            }
            name = format!("{base}_{n}");
            renamed.push(base);
        }
        taken.insert(name.clone());
        names.push(name);
    }
    if !renamed.is_empty() {
        warnings.push(ImportWarning::new(
            "duplicate_variable_names",
            format!("the file uses the same variable name more than once ({}); later ones were renamed with a number", list_names(&renamed)),
        ));
    }

    // Levels of measurement from the display parameters.
    let measures: Vec<i32> = {
        let mut out = vec![0; lvars.len()];
        if let Some(disp) = &dict.display {
            let (nrec, nlog) = (dict.vars.len(), lvars.len());
            let layout = [(3, nrec, true), (3, nlog, false), (2, nrec, true), (2, nlog, false)]
                .into_iter()
                .find(|(stride, count, _)| disp.len() == stride * count);
            if let Some((stride, _, by_record)) = layout {
                for (k, lv) in lvars.iter().enumerate() {
                    let at = if by_record { lv.head } else { k };
                    out[k] = disp.get(at * stride).copied().unwrap_or(0);
                }
            }
        }
        out
    };

    // Columns: label, level, missing codes.
    let mut columns: Vec<Column> = Vec::with_capacity(lvars.len());
    let mut durations: Vec<String> = Vec::new();
    for (k, lv) in lvars.iter().enumerate() {
        let v = &dict.vars[lv.head];
        let mut col = Column::new(names[k].clone());
        col.label = v.label.map(|l| dec.text(l)).filter(|l| !l.is_empty());
        let date = match &lv.plan {
            Plan::Number { date, .. } => *date,
            Plan::Text { .. } => DateKind::No,
        };
        col.level = if matches!(date, DateKind::Day | DateKind::DayTime) {
            Some(Level::Date)
        } else {
            match measures[k] {
                1 => Some(Level::Nominal),
                2 => Some(Level::Ordinal),
                3 => Some(Level::Ratio),
                _ => None,
            }
        };
        if date == DateKind::Duration {
            durations.push(names[k].clone());
        }

        let show = |u: Unit, dec: &mut Decoder| -> String {
            if v.width == 0 {
                number_text(unit_to_f64(le, u), highest, lowest)
            } else {
                dec.text(&u)
            }
        };
        let (discrete, range): (Vec<Unit>, Option<(Unit, Unit)>) = match v.miss_kind {
            -2 if v.miss.len() == 2 => (vec![], Some((v.miss[0], v.miss[1]))),
            -3 if v.miss.len() == 3 => (vec![v.miss[2]], Some((v.miss[0], v.miss[1]))),
            _ => (v.miss.clone(), None),
        };
        for u in &discrete {
            col.missing_codes.push(show(*u, dec));
        }
        if let Some((lo, hi)) = range {
            let (lo, hi) = (show(lo, dec), show(hi, dec));
            let kept = if col.missing_codes.is_empty() {
                "no single value".to_string()
            } else {
                format!("the single value {}", col.missing_codes.join(", "))
            };
            warnings.push(
                ImportWarning::new(
                    "missing_values_not_fully_carried",
                    format!(
                        "variable '{}' declares the range {lo} through {hi} as missing; PrismQDA keeps single missing values only, \
                         so {kept} was carried over and values inside the range are not marked as missing",
                        names[k]
                    ),
                )
                .at(names[k].clone()),
            );
        }
        columns.push(col);
    }
    if !durations.is_empty() {
        warnings.push(ImportWarning::new(
            "time_values_kept_as_seconds",
            format!(
                "{} {} a length of time (TIME format); the values were kept as a number of seconds",
                list_names(&durations),
                if durations.len() == 1 { "holds" } else { "hold" }
            ),
        ));
    }

    // Names that other records use to point at a variable.
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (k, n) in names.iter().enumerate() {
        by_name.entry(n.to_lowercase()).or_insert(k);
    }
    for (k, lv) in lvars.iter().enumerate() {
        let short = dec.text(trim_end_bytes(dict.vars[lv.head].name)).to_lowercase();
        by_name.entry(short).or_insert(k);
    }

    // Value labels: the lists shared by variables, then the ones for text longer than 8 bytes.
    let mut slot_to_var: HashMap<usize, usize> = HashMap::new();
    for (k, lv) in lvars.iter().enumerate() {
        slot_to_var.insert(dict.vars[lv.head].first_slot + 1, k);
    }
    for set in &dict.label_sets {
        for slot in &set.slots {
            let k = usize::try_from(*slot)
                .ok()
                .and_then(|s| slot_to_var.get(&s))
                .copied()
                .ok_or_else(|| damaged("value labels belong to a variable that does not exist"))?;
            let numeric = matches!(lvars[k].plan, Plan::Number { .. });
            for (key, label) in &set.entries {
                let key = if numeric {
                    let v = unit_to_f64(le, *key);
                    if !v.is_finite() {
                        continue;
                    }
                    number_text(v, highest, lowest)
                } else {
                    dec.text(key)
                };
                columns[k].value_labels.insert(key, dec.text(label));
            }
        }
    }
    let mut skipped: Vec<&str> = Vec::new();
    for rec in &dict.long_labels {
        if apply_long_labels(rec, le, &by_name, &mut columns, dec).is_err() {
            skipped.push("value labels");
            break;
        }
    }
    for rec in &dict.long_missing {
        if apply_long_missing(rec, le, &by_name, &mut columns, dec, warnings).is_err() {
            skipped.push("missing values");
            break;
        }
    }
    for what in skipped {
        warnings.push(ImportWarning::new(
            "extension_record_unreadable",
            format!("the part of the file that holds {what} for long text variables could not be read and was skipped"),
        ));
    }

    // The weight variable is not applied.
    if header.weight_slot > 0 {
        let wanted = header.weight_slot as usize - 1;
        if let Some(k) = lvars.iter().position(|lv| dict.vars[lv.head].first_slot == wanted) {
            warnings.push(ImportWarning::new(
                "weight_variable_ignored",
                format!(
                    "the file is weighted by variable '{}'; PrismQDA does not apply weights, so counts and percentages are unweighted",
                    names[k]
                ),
            ));
        }
    }

    let plans = lvars.into_iter().map(|lv| lv.plan).collect();
    Ok(Model { columns, plans })
}

/// Value labels for text longer than 8 bytes (extension record 21): for each variable, its name,
/// its width, and a list of value and label pairs.
fn apply_long_labels(
    rec: &[u8],
    le: bool,
    by_name: &HashMap<String, usize>,
    columns: &mut [Column],
    dec: &mut Decoder,
) -> Result<()> {
    let mut cur = Cur::new(rec, 0, le);
    let mut work = 0usize;
    while cur.left() > 0 {
        let name = read_sized(&mut cur)?;
        let _width = cur.i32("long value labels")?;
        let n = cur.i32("long value labels")?;
        let n = usize::try_from(n).map_err(|_| damaged("negative label count"))?;
        if n > cur.left() / 8 {
            return Err(cut_short("long value labels"));
        }
        let target = by_name.get(&dec.text(name).to_lowercase()).copied();
        for _ in 0..n {
            let value = read_sized(&mut cur)?;
            let label = read_sized(&mut cur)?;
            work += 1;
            if work > MAX_LABEL_WORK {
                return Err(damaged("too many labels"));
            }
            if let Some(k) = target {
                let (key, label) = (dec.text(value), dec.text(label));
                columns[k].value_labels.insert(key, label);
            }
        }
    }
    Ok(())
}

/// A length followed by that many bytes.
fn read_sized<'a>(cur: &mut Cur<'a>) -> Result<&'a [u8]> {
    let len = cur.i32("a name or value")?;
    let len = usize::try_from(len).map_err(|_| damaged("negative length"))?;
    cur.take(len, "a name or value")
}

/// Missing values for text longer than 8 bytes (extension record 22): for each variable, its name,
/// how many values there are, the length of each value, then the values.
fn apply_long_missing(
    rec: &[u8],
    le: bool,
    by_name: &HashMap<String, usize>,
    columns: &mut [Column],
    dec: &mut Decoder,
    warnings: &mut Vec<ImportWarning>,
) -> Result<()> {
    let mut cur = Cur::new(rec, 0, le);
    while cur.left() > 0 {
        let name = read_sized(&mut cur)?;
        let n = usize::from(cur.arr::<1>("missing values")?[0]);
        let value_len = cur.i32("missing values")?;
        let value_len = usize::try_from(value_len).map_err(|_| damaged("negative length"))?;
        if n.saturating_mul(value_len) > cur.left() {
            return Err(cut_short("missing values"));
        }
        let name = dec.text(name);
        let target = by_name.get(&name.to_lowercase()).copied();
        for _ in 0..n {
            let value = cur.take(value_len, "missing values")?;
            if let Some(k) = target {
                if columns[k].missing_codes.len() < 3 {
                    let text = dec.text(value);
                    columns[k].missing_codes.push(text);
                } else {
                    warnings.push(
                        ImportWarning::new(
                            "missing_values_not_fully_carried",
                            format!("variable '{}' declares more than three missing values; only the first three were carried over", columns[k].name),
                        )
                        .at(columns[k].name.clone()),
                    );
                    break;
                }
            }
        }
    }
    Ok(())
}

// ---- the cases ---------------------------------------------------------------------------------------------

struct Block {
    offset: usize,
    compressed: usize,
    unpacked: usize,
}

enum Data<'a> {
    Plain { bytes: &'a [u8], pos: usize },
    Zlib { file: &'a [u8], blocks: Vec<Block>, next: usize, buf: Vec<u8>, pos: usize },
}

impl Data<'_> {
    /// The next 8 bytes, or `None` at a clean end of the data.
    fn next(&mut self) -> Result<Option<Unit>> {
        match self {
            Data::Plain { bytes, pos } => {
                let left = bytes.len() - *pos;
                if left == 0 {
                    return Ok(None);
                }
                if left < 8 {
                    return Err(cut_short("the data ends in the middle of a value"));
                }
                let mut u = [0u8; 8];
                u.copy_from_slice(&bytes[*pos..*pos + 8]);
                *pos += 8;
                Ok(Some(u))
            }
            Data::Zlib { file, blocks, next, buf, pos } => {
                while buf.len() - *pos < 8 {
                    let Some(block) = blocks.get(*next) else {
                        return if buf.len() == *pos {
                            Ok(None)
                        } else {
                            Err(cut_short("the data ends in the middle of a value"))
                        };
                    };
                    *next += 1;
                    let packed = &file[block.offset..block.offset + block.compressed];
                    let mut out: Vec<u8> = buf[*pos..].to_vec();
                    let before = out.len();
                    let mut dec = flate2::read::ZlibDecoder::new(packed).take(block.unpacked as u64 + 1);
                    dec.read_to_end(&mut out).map_err(|_| damaged("a compressed block of data cannot be unpacked"))?;
                    if out.len() - before != block.unpacked {
                        return Err(damaged("a compressed block of data has the wrong size"));
                    }
                    *buf = out;
                    *pos = 0;
                }
                let mut u = [0u8; 8];
                u.copy_from_slice(&buf[*pos..*pos + 8]);
                *pos += 8;
                Ok(Some(u))
            }
        }
    }

    /// The most 8-byte values that the data can still hold.
    fn capacity_values(&self) -> u64 {
        match self {
            Data::Plain { bytes, pos } => ((bytes.len() - *pos) / 8) as u64,
            Data::Zlib { blocks, next, buf, pos, .. } => {
                let ahead: u64 = blocks.iter().skip(*next).map(|b| b.unpacked as u64).sum();
                (ahead + (buf.len() - *pos) as u64) / 8
            }
        }
    }
}

/// Bytecode compression: groups of eight command bytes, each standing for one 8-byte value.
struct Bytecode {
    ops: [u8; 8],
    next_op: usize,
    done: bool,
    bias: f64,
    le: bool,
    sysmis: Unit,
}

impl Bytecode {
    fn next_value(&mut self, data: &mut Data) -> Result<Option<Unit>> {
        loop {
            if self.done {
                return Ok(None);
            }
            if self.next_op >= 8 {
                match data.next()? {
                    None => {
                        self.done = true;
                        return Ok(None);
                    }
                    Some(ops) => {
                        self.ops = ops;
                        self.next_op = 0;
                    }
                }
            }
            let op = self.ops[self.next_op];
            self.next_op += 1;
            match op {
                0 => {}
                252 => {
                    self.done = true;
                    return Ok(None);
                }
                253 => {
                    return match data.next()? {
                        Some(u) => Ok(Some(u)),
                        None => Err(cut_short("the data ends where a value should be")),
                    }
                }
                254 => return Ok(Some([b' '; 8])),
                255 => return Ok(Some(self.sysmis)),
                code => return Ok(Some(f64_to_unit(self.le, f64::from(code) - self.bias))),
            }
        }
    }
}

struct CaseReader<'a> {
    data: Data<'a>,
    bytecode: Option<Bytecode>,
}

impl CaseReader<'_> {
    /// Fills `out` with the next case. `false` means there are no more cases.
    fn read_case(&mut self, out: &mut [Unit]) -> Result<bool> {
        for (i, slot) in out.iter_mut().enumerate() {
            let next = match &mut self.bytecode {
                None => self.data.next()?,
                Some(bc) => bc.next_value(&mut self.data)?,
            };
            match next {
                Some(u) => *slot = u,
                None if i == 0 => return Ok(false),
                None => return Err(cut_short("the last case is cut off")),
            }
        }
        Ok(true)
    }
}

fn open_zlib<'a>(cur: &mut Cur<'a>) -> Result<Data<'a>> {
    const WHAT: &str = "the compressed-data header";
    let file = cur.b;
    let start = cur.pos;
    if cur.left() == 0 {
        return Ok(Data::Plain { bytes: &[], pos: 0 });
    }
    let _header_offset = cur.i64(WHAT)?;
    let trailer_ofs = to_usize(cur.i64(WHAT)?).ok_or_else(|| damaged("the compressed data has an impossible layout"))?;
    let trailer_len = to_usize(cur.i64(WHAT)?).ok_or_else(|| damaged("the compressed data has an impossible layout"))?;
    let blocks_start = start + 24;
    let trailer_end = trailer_ofs
        .checked_add(trailer_len)
        .filter(|end| *end <= file.len() && trailer_ofs >= blocks_start && trailer_len >= 24)
        .ok_or_else(|| damaged("the compressed data has an impossible layout"))?;
    let mut t = Cur::new(&file[..trailer_end], trailer_ofs, cur.le);
    let _bias = t.i64(WHAT)?;
    let _zero = t.i64(WHAT)?;
    let _block_size = t.i32(WHAT)?;
    let n = t.i32(WHAT)?;
    let n = usize::try_from(n).map_err(|_| damaged("the compressed data has a negative block count"))?;
    if n > (trailer_len - 24) / 24 {
        return Err(damaged("the compressed data lists more blocks than it has room for"));
    }
    let mut blocks = Vec::with_capacity(n);
    for _ in 0..n {
        let _unpacked_offset = t.i64(WHAT)?;
        let offset = to_usize(t.i64(WHAT)?).ok_or_else(|| damaged("a compressed block has an impossible position"))?;
        let unpacked = usize::try_from(t.i32(WHAT)?).map_err(|_| damaged("a compressed block has a negative size"))?;
        let compressed = usize::try_from(t.i32(WHAT)?).map_err(|_| damaged("a compressed block has a negative size"))?;
        if offset < blocks_start || exceeds(offset.checked_add(compressed), trailer_ofs) || unpacked > MAX_BLOCK_BYTES {
            return Err(damaged("a compressed block lies outside the data"));
        }
        blocks.push(Block { offset, compressed, unpacked });
    }
    Ok(Data::Zlib { file, blocks, next: 0, buf: Vec::new(), pos: 0 })
}

/// True when a checked sum overflowed or is over the limit.
fn exceeds(v: Option<usize>, limit: usize) -> bool {
    match v {
        Some(v) => v > limit,
        None => true,
    }
}

fn read_cases<'a>(
    cur: &mut Cur<'a>,
    header: &Header,
    dict: &Dict,
    model: &Model,
    sysmis: f64,
    dec: &mut Decoder,
    warnings: &mut Vec<ImportWarning>,
) -> Result<Vec<Vec<Cell>>> {
    let le = header.le;
    let nslots = dict.nslots;
    let ncols = model.columns.len();

    let data = if header.compression == 2 {
        open_zlib(cur)?
    } else {
        let rest = &cur.b[cur.pos..];
        Data::Plain { bytes: rest, pos: 0 }
    };
    // How many cases the file says it holds. A negative count means the file does not say.
    let declared: Option<usize> = if header.ncases >= 0 {
        Some(header.ncases as usize)
    } else {
        dict.ncases64.and_then(to_usize)
    };
    // Each case needs at least one value (one byte when bytecoded, eight when plain).
    let values = data.capacity_values();
    let slot_values = if header.compression == 0 { values } else { values.saturating_mul(8) };
    let max_cases = slot_values / nslots as u64;
    if let Some(n) = declared {
        if exceeds(n.checked_mul(ncols), MAX_CELLS) {
            return Err(ImportError::format(format!(
                "the SPSS file holds {n} cases of {ncols} variables, more than this version can load"
            )));
        }
        if n as u64 > max_cases {
            return Err(cut_short("the file says it holds more cases than its data can hold"));
        }
    }

    let bytecode = (header.compression != 0).then(|| Bytecode {
        ops: [0; 8],
        next_op: 8,
        done: false,
        bias: header.bias,
        le,
        sysmis: f64_to_unit(le, sysmis),
    });
    let mut reader = CaseReader { data, bytecode };
    let mut slots: Vec<Unit> = vec![[0u8; 8]; nslots];
    let mut rows: Vec<Vec<Cell>> = Vec::with_capacity(declared.unwrap_or(0).min(1 << 16));
    let mut buf: Vec<u8> = Vec::new();
    let mut out_of_range = vec![0usize; ncols];

    loop {
        if declared == Some(rows.len()) {
            break;
        }
        if !reader.read_case(&mut slots)? {
            if let Some(n) = declared {
                return Err(cut_short(&format!("the file says it holds {n} cases but only {} could be read", rows.len())));
            }
            break;
        }
        if (rows.len() + 1).saturating_mul(ncols) > MAX_CELLS {
            return Err(ImportError::format("the SPSS file holds more values than this version can load"));
        }
        let mut row: Vec<Cell> = Vec::with_capacity(ncols);
        for (c, plan) in model.plans.iter().enumerate() {
            row.push(match plan {
                Plan::Number { slot, date } => number_cell(unit_to_f64(le, slots[*slot]), sysmis, *date, &mut out_of_range[c]),
                Plan::Text { segs } => {
                    buf.clear();
                    for seg in segs {
                        let mut left = seg.take;
                        let mut s = seg.slot;
                        while left > 0 {
                            let n = left.min(8);
                            match slots.get(s) {
                                Some(u) => buf.extend_from_slice(&u[..n]),
                                None => break,
                            }
                            left -= n;
                            s += 1;
                        }
                    }
                    let text = dec.text(&buf);
                    if text.is_empty() {
                        Cell::Empty
                    } else {
                        Cell::Text(text)
                    }
                }
            });
        }
        rows.push(row);
    }

    let bad: Vec<String> = out_of_range
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(c, _)| model.columns[c].name.clone())
        .collect();
    if !bad.is_empty() {
        warnings.push(ImportWarning::new(
            "date_values_out_of_range",
            format!("{} hold dates outside the years 0 to 9999; those values were kept as numbers of seconds", list_names(&bad)),
        ));
    }
    Ok(rows)
}

fn number_cell(v: f64, sysmis: f64, date: DateKind, out_of_range: &mut usize) -> Cell {
    if !v.is_finite() || v == sysmis || v <= f64::MIN {
        return Cell::Empty;
    }
    match date {
        DateKind::Day | DateKind::DayTime => match spss_seconds_to_iso(v, date == DateKind::DayTime) {
            Some(text) => Cell::Date(text),
            None => {
                *out_of_range += 1;
                Cell::Number(v)
            }
        },
        _ => Cell::Number(v),
    }
}

// ---- dates ---------------------------------------------------------------------------------------------------

/// Turns SPSS seconds (counted from 1582-10-14 00:00:00) into ISO 8601 text: a day, or a day and time
/// (with milliseconds when there are any). `None` when the value is outside the years 0 to 9999.
fn spss_seconds_to_iso(secs: f64, with_time: bool) -> Option<String> {
    if !secs.is_finite() {
        return None;
    }
    if with_time {
        let ms = (secs * 1000.0).round();
        if ms.abs() > 1.0e15 {
            return None;
        }
        let unix_ms = (ms as i64).checked_sub(SPSS_TO_UNIX_SECONDS * 1000)?;
        let days = unix_ms.div_euclid(86_400_000);
        let in_day = unix_ms.rem_euclid(86_400_000);
        let (y, m, d) = civil_from_days(days);
        if !(0..=9999).contains(&y) {
            return None;
        }
        let (hh, mm, ss, milli) = (in_day / 3_600_000, in_day / 60_000 % 60, in_day / 1000 % 60, in_day % 1000);
        let mut text = format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}");
        if milli != 0 {
            text.push_str(format!(".{milli:03}").trim_end_matches('0'));
        }
        Some(text)
    } else {
        let day = (secs / 86_400.0).floor();
        if day.abs() > 1.0e12 {
            return None;
        }
        let (y, m, d) = civil_from_days((day as i64).checked_sub(SPSS_TO_UNIX_DAYS)?);
        if !(0..=9999).contains(&y) {
            return None;
        }
        Some(format!("{y:04}-{m:02}-{d:02}"))
    }
}

/// The calendar date (proleptic Gregorian) of a day count from 1970-01-01.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds .sav bytes by hand, for the odd cases the independent writer cannot make: the other byte
    /// order, other character sets, unknown case counts, and damaged records. These files only show that
    /// the reader follows the layout as documented and survives bad input. That it reads real files
    /// faithfully is shown by the fixtures in tests/fixtures/sav, written by another program.
    struct W {
        le: bool,
        b: Vec<u8>,
    }

    impl W {
        fn new(le: bool) -> Self {
            W { le, b: Vec::new() }
        }
        fn i32(&mut self, v: i32) -> &mut Self {
            self.b.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
            self
        }
        fn f64(&mut self, v: f64) -> &mut Self {
            let u = f64_to_unit(self.le, v);
            self.b.extend_from_slice(&u);
            self
        }
        fn raw(&mut self, b: &[u8]) -> &mut Self {
            self.b.extend_from_slice(b);
            self
        }
        fn padded(&mut self, b: &[u8], n: usize) -> &mut Self {
            self.b.extend_from_slice(b);
            self.b.resize(self.b.len() + n.saturating_sub(b.len()), b' ');
            self
        }
        fn header(&mut self, compression: i32, ncases: i32, weight: i32) -> &mut Self {
            self.raw(b"$FL2").padded(b"@(#) hand-built test file", 60);
            self.i32(2).i32(0).i32(compression).i32(weight).i32(ncases).f64(100.0);
            self.padded(b"01 Jan 26", 9).padded(b"00:00:00", 8).padded(b"", 64).raw(&[0; 3])
        }
        /// A variable record (and its continuation records when it is text longer than 8 bytes).
        fn var(&mut self, ty: i32, name: &str, print: u32, label: Option<&str>, miss_kind: i32, miss: &[Unit]) -> &mut Self {
            self.i32(2).i32(ty).i32(i32::from(label.is_some())).i32(miss_kind).i32(print as i32).i32(print as i32);
            self.padded(name.as_bytes(), 8);
            if let Some(l) = label {
                self.i32(l.len() as i32).raw(l.as_bytes());
                self.b.resize(self.b.len() + (4 - l.len() % 4) % 4, b' ');
            }
            for m in miss {
                self.raw(m);
            }
            for _ in 1..(ty.max(0) as usize).div_ceil(8) {
                self.i32(2).i32(-1).i32(0).i32(0).i32(0).i32(0).padded(b"", 8);
            }
            self
        }
        fn num(&mut self, name: &str) -> &mut Self {
            self.var(0, name, 0x0005_0800, None, 0, &[])
        }
        fn text(&mut self, name: &str, width: i32) -> &mut Self {
            self.var(width, name, 0x0001_0000 | ((width as u32) << 8), None, 0, &[])
        }
        fn ext(&mut self, subtype: i32, size: i32, data: &[u8]) -> &mut Self {
            self.i32(7).i32(subtype).i32(size).i32(data.len() as i32 / size.max(1)).raw(data)
        }
        fn end(&mut self) -> &mut Self {
            self.i32(999).i32(0)
        }
        fn bytes(&self) -> Vec<u8> {
            self.b.clone()
        }
    }

    fn int_info(le: bool, floating_rep: i32, code_page: i32) -> Vec<u8> {
        let mut w = W::new(le);
        for v in [20, 0, 0, -1, floating_rep, 1, if le { 2 } else { 1 }, code_page] {
            w.i32(v);
        }
        w.b
    }

    /// Two numbers and a text per case, as plain values.
    fn small(le: bool) -> W {
        let mut w = W::new(le);
        w.header(0, 2, 0).num("A").num("B").text("T", 8).ext(3, 4, &int_info(le, 1, 65001)).end();
        w.f64(1.5).f64(2.0).raw(b"abc     ");
        w.f64(f64::MIN).f64(4.25).raw(b"ab\0\0\0\0\0\0");
        w
    }

    fn err_text(bytes: &[u8]) -> String {
        read_table(bytes).unwrap_err().to_string()
    }

    // ---- dates --------------------------------------------------------------------------------------------

    #[test]
    fn spss_seconds_become_iso_dates() {
        let day = |s: f64| spss_seconds_to_iso(s, false);
        let time = |s: f64| spss_seconds_to_iso(s, true);
        // Day zero of the SPSS calendar, the day after, the Unix epoch, and the start of 2000.
        assert_eq!(day(0.0).as_deref(), Some("1582-10-14"));
        assert_eq!(day(86_400.0).as_deref(), Some("1582-10-15"));
        assert_eq!(day(12_219_379_200.0).as_deref(), Some("1970-01-01"));
        assert_eq!(day(13_166_064_000.0).as_deref(), Some("2000-01-01"));
        // A leap day, and a day before the calendar starts.
        assert_eq!(day(13_928_544_000.0).as_deref(), Some("2024-02-29"));
        assert_eq!(day(-86_400.0).as_deref(), Some("1582-10-13"));
        assert_eq!(day(-1.0).as_deref(), Some("1582-10-13"), "a day is rounded down");
        assert_eq!(day(13_166_064_000.0 + 86_399.0).as_deref(), Some("2000-01-01"), "the time of day is dropped");
        // The last day with a four-digit year, and the first without.
        assert_eq!(day(265_621_593_600.0).as_deref(), Some("9999-12-31"));
        assert_eq!(day(265_621_680_000.0), None);

        assert_eq!(time(13_166_064_000.0 + 3661.0).as_deref(), Some("2000-01-01T01:01:01"));
        assert_eq!(time(13_166_064_000.0 - 1.0).as_deref(), Some("1999-12-31T23:59:59"));
        assert_eq!(time(13_166_064_000.25).as_deref(), Some("2000-01-01T00:00:00.25"));
        assert_eq!(time(13_166_064_000.1).as_deref(), Some("2000-01-01T00:00:00.1"));
        assert_eq!(time(13_166_064_000.5 + 3.0).as_deref(), Some("2000-01-01T00:00:03.5"));
        assert_eq!(time(13_928_544_000.0 + 43_200.0).as_deref(), Some("2024-02-29T12:00:00"));

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e30, -1e30, 1e300] {
            assert_eq!(day(bad), None, "{bad}");
            assert_eq!(time(bad), None, "{bad}");
        }
    }

    fn month_length(y: i64, m: u32) -> u32 {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m as usize - 1]
    }

    #[test]
    fn the_calendar_agrees_with_a_slow_day_count() {
        // Walk day by day from 1970-01-01 and compare with the closed-form conversion.
        let (mut y, mut m, mut d) = (1970i64, 1u32, 1u32);
        for day in 0..60_000i64 {
            assert_eq!(civil_from_days(day), (y, m, d), "day {day}");
            d += 1;
            if d > month_length(y, m) {
                d = 1;
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
            }
        }
        // And backwards, for the years before 1970.
        let (mut y, mut m, mut d) = (1969i64, 12u32, 31u32);
        for day in (-60_000..0i64).rev() {
            assert_eq!(civil_from_days(day), (y, m, d), "day {day}");
            if d > 1 {
                d -= 1;
            } else {
                if m > 1 {
                    m -= 1;
                } else {
                    m = 12;
                    y -= 1;
                }
                d = month_length(y, m);
            }
        }
    }

    // ---- layout variants the independent writer cannot make -----------------------------------------------------

    #[test]
    fn a_big_endian_file_reads_like_a_little_endian_one() {
        let le = read_table(&small(true).bytes()).unwrap();
        let be = read_table(&small(false).bytes()).unwrap();
        assert_eq!(le, be);
        assert_eq!(be.rows.len(), 2);
        assert_eq!(be.rows[0], [Cell::Number(1.5), Cell::Number(2.0), Cell::Text("abc".into())]);
        assert_eq!(be.rows[1], [Cell::Empty, Cell::Number(4.25), Cell::Text("ab".into())], "NUL padding is trimmed too");
        assert!(be.warnings.is_empty());
    }

    #[test]
    fn bytecoded_data_with_an_unknown_case_count_reads_to_its_end() {
        let mut w = W::new(true);
        w.header(1, -1, 0).num("N").text("T", 8).ext(3, 4, &int_info(true, 1, 65001)).end();
        // Case 1: 1 and "abc". Case 2: 2.75 (raw) and blanks. Case 3: system-missing and "z".
        // Case 4: -99 and blanks. Case 5: 151 and blanks. Then the end marker.
        w.raw(&[101, 253, 253, 254, 255, 253, 1, 254]);
        w.raw(b"abc     ").f64(2.75).raw(b"z       ");
        w.raw(&[251, 254, 252, 0, 0, 0, 0, 0]);
        let t = read_table(&w.bytes()).unwrap();
        assert_eq!(
            t.rows,
            vec![
                vec![Cell::Number(1.0), Cell::Text("abc".into())],
                vec![Cell::Number(2.75), Cell::Empty],
                vec![Cell::Empty, Cell::Text("z".into())],
                vec![Cell::Number(-99.0), Cell::Empty],
                vec![Cell::Number(151.0), Cell::Empty],
            ]
        );
    }

    #[test]
    fn a_case_can_run_across_blocks_of_command_bytes_and_padding_is_skipped() {
        let mut w = W::new(true);
        w.header(1, 3, 0).num("A").num("B").num("C").end();
        // Nine values over two blocks; 0 is padding and is ignored.
        w.raw(&[0, 101, 102, 0, 103, 104, 105, 106]);
        w.raw(&[107, 0, 108, 109, 252, 0, 0, 0]);
        let t = read_table(&w.bytes()).unwrap();
        let nums: Vec<f64> = t.rows.iter().flatten().map(|c| if let Cell::Number(n) = c { *n } else { f64::NAN }).collect();
        assert_eq!(nums, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn the_bias_in_the_header_is_used() {
        let mut w = W::new(true);
        w.header(1, 1, 0).num("A").end();
        // Replace the bias (offset 84) with 50.
        w.b[84..92].copy_from_slice(&50.0f64.to_le_bytes());
        w.raw(&[60, 252, 0, 0, 0, 0, 0, 0]);
        assert_eq!(read_table(&w.bytes()).unwrap().rows[0][0], Cell::Number(10.0));
    }

    #[test]
    fn a_header_case_count_longer_than_the_data_is_an_error_but_a_shorter_one_is_respected() {
        let full = small(true).bytes();
        let mut one = full.clone();
        one[80..84].copy_from_slice(&1i32.to_le_bytes());
        assert_eq!(read_table(&one).unwrap().rows.len(), 1, "the file says one case, so the second is not read");
        let mut three = full;
        three[80..84].copy_from_slice(&3i32.to_le_bytes());
        assert!(err_text(&three).contains("ends too soon"));
    }

    #[test]
    fn a_file_with_variables_but_no_cases_reads_as_an_empty_table() {
        let mut w = W::new(true);
        w.header(0, 0, 0).num("A").end();
        let t = read_table(&w.bytes()).unwrap();
        assert!(t.rows.is_empty());
        assert_eq!(t.columns.len(), 1);
        assert!(t.warnings.iter().any(|w| w.kind == "no_cases"));
    }

    #[test]
    fn a_document_record_is_skipped() {
        let mut w = W::new(true);
        w.header(0, 1, 0).num("A");
        w.i32(6).i32(2).padded(b"first line", 80).padded(b"second", 80);
        w.end().f64(7.0);
        assert_eq!(read_table(&w.bytes()).unwrap().rows[0][0], Cell::Number(7.0));
    }

    // ---- character sets ---------------------------------------------------------------------------------------------

    fn one_text_file(extensions: &[(i32, i32, Vec<u8>)], value: &[u8]) -> Vec<u8> {
        let mut w = W::new(true);
        w.header(0, 1, 0).text("T", 8);
        for (sub, size, data) in extensions {
            w.ext(*sub, *size, data);
        }
        w.end().padded(value, 8);
        w.bytes()
    }

    #[test]
    fn the_declared_character_set_is_used() {
        // Windows-1252 by name: byte E9 is e with an acute accent.
        let t = read_table(&one_text_file(&[(20, 1, b"windows-1252".to_vec())], &[b'c', b'a', b'f', 0xE9])).unwrap();
        assert_eq!(t.rows[0][0], Cell::Text("caf\u{e9}".into()));
        // Shift-JIS by code page number: the two characters meaning "Japan".
        let sjis = [0x93, 0xFA, 0x96, 0x7B];
        let t = read_table(&one_text_file(&[(3, 4, int_info(true, 1, 932))], &sjis)).unwrap();
        assert_eq!(t.rows[0][0], Cell::Text("\u{65e5}\u{672c}".into()));
        // UTF-8 by name, with the NUL padding and spaces some writers add to the name.
        let t = read_table(&one_text_file(&[(20, 1, b"UTF-8\0\0 ".to_vec())], "\u{e9}t\u{e9}".as_bytes())).unwrap();
        assert_eq!(t.rows[0][0], Cell::Text("\u{e9}t\u{e9}".into()));
        // The name wins over the code page.
        let both = [(3, 4, int_info(true, 1, 932)), (20, 1, b"windows-1252".to_vec())];
        let t = read_table(&one_text_file(&both, &[0xE9])).unwrap();
        assert_eq!(t.rows[0][0], Cell::Text("\u{e9}".into()));
    }

    #[test]
    fn with_no_declared_character_set_windows_1252_is_assumed_quietly() {
        let t = read_table(&one_text_file(&[], &[0x93, b'h', b'i', 0x94])).unwrap();
        assert_eq!(t.rows[0][0], Cell::Text("\u{201c}hi\u{201d}".into()));
        assert!(t.warnings.is_empty());
    }

    #[test]
    fn an_unknown_character_set_is_assumed_and_reported() {
        let t = read_table(&one_text_file(&[(20, 1, b"IBM01140".to_vec())], &[0xE9])).unwrap();
        assert_eq!(t.rows[0][0], Cell::Text("\u{e9}".into()));
        assert!(t.warnings.iter().any(|w| w.kind == "encoding_assumed" && w.message.contains("IBM01140")));
        let t = read_table(&one_text_file(&[(3, 4, int_info(true, 1, 437))], &[0xE9])).unwrap();
        assert!(t.warnings.iter().any(|w| w.kind == "encoding_assumed" && w.message.contains("437")));
    }

    #[test]
    fn text_that_is_not_valid_in_its_character_set_is_kept_with_a_warning() {
        let t = read_table(&one_text_file(&[(20, 1, b"UTF-8".to_vec())], &[b'a', 0xFF, b'b'])).unwrap();
        assert_eq!(t.rows[0][0], Cell::Text("a\u{fffd}b".into()));
        assert!(t.warnings.iter().any(|w| w.kind == "text_decoding_errors"));
    }

    #[test]
    fn an_ebcdic_file_is_refused_in_plain_words() {
        let e = err_text(&one_text_file(&[(3, 4, int_info(true, 1, 1))], b"a"));
        assert!(e.contains("EBCDIC"), "{e}");
    }

    #[test]
    fn ascii_code_pages_are_accepted_without_a_warning() {
        for cp in [2, 3, 20127] {
            let t = read_table(&one_text_file(&[(3, 4, int_info(true, 1, cp))], b"plain")).unwrap();
            assert_eq!(t.rows[0][0], Cell::Text("plain".into()));
            assert!(t.warnings.is_empty(), "code page {cp}");
        }
    }

    #[test]
    fn a_floating_point_record_with_wrong_values_does_not_spoil_labels_or_missing_cells() {
        // One old writer states NaN for system-missing and swaps highest and lowest.
        let mut info = W::new(true);
        info.f64(f64::NAN).f64(f64::MIN).f64(f64::MAX);
        let mut w = W::new(true);
        w.header(0, 2, 0).num("A").i32(3).i32(2);
        w.f64(1.0).raw(&[3]).raw(b"Yes\0\0\0\0").f64(2.0).raw(&[2]).raw(b"No\0\0\0\0\0");
        w.i32(4).i32(1).i32(1);
        w.ext(4, 8, &info.b).end().f64(2.0).f64(f64::MIN);
        let t = read_table(&w.bytes()).unwrap();
        assert_eq!(t.columns[0].value_labels.len(), 2, "{:?}", t.columns[0].value_labels);
        assert_eq!(t.columns[0].value_labels["1"], "Yes");
        assert_eq!(t.columns[0].value_labels["2"], "No");
        assert_eq!(t.rows[0][0], Cell::Number(2.0));
        assert_eq!(t.rows[1][0], Cell::Empty, "the usual system-missing value still counts as missing");
    }

    #[test]
    fn numbers_in_an_old_format_are_refused() {
        let e = err_text(&one_text_file(&[(3, 4, int_info(true, 2, 65001))], b"a"));
        assert!(e.contains("IBM or VAX"), "{e}");
    }

    // ---- missing values, weights, and names ----------------------------------------------------------------------------

    fn file_with_var(le: bool, kind: i32, miss: &[f64]) -> Vec<u8> {
        let mut w = W::new(le);
        let units: Vec<Unit> = miss.iter().map(|m| f64_to_unit(le, *m)).collect();
        w.header(0, 1, 0).var(0, "X", 0x0005_0800, Some("An X"), kind, &units).end().f64(1.0);
        w.bytes()
    }

    #[test]
    fn every_kind_of_missing_value_declaration_is_read() {
        for le in [true, false] {
            let t = read_table(&file_with_var(le, 0, &[])).unwrap();
            assert!(t.columns[0].missing_codes.is_empty());
            assert_eq!(t.columns[0].label.as_deref(), Some("An X"));
            let t = read_table(&file_with_var(le, 3, &[7.0, 8.0, 9.5])).unwrap();
            assert_eq!(t.columns[0].missing_codes, ["7", "8", "9.5"]);
            assert!(t.warnings.is_empty());
            let t = read_table(&file_with_var(le, -2, &[90.0, 99.0])).unwrap();
            assert!(t.columns[0].missing_codes.is_empty());
            assert_eq!(t.warnings.len(), 1);
            assert_eq!(t.warnings[0].kind, "missing_values_not_fully_carried");
            assert_eq!(t.warnings[0].location.as_deref(), Some("X"));
            let t = read_table(&file_with_var(le, -3, &[90.0, 99.0, -1.0])).unwrap();
            assert_eq!(t.columns[0].missing_codes, ["-1"]);
            assert_eq!(t.warnings.len(), 1);
        }
        for bad in [4, -1, -4, 100, i32::MIN] {
            assert!(read_table(&file_with_var(true, bad, &[1.0, 2.0, 3.0, 4.0])).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_values_of_short_text_sit_in_the_variable_record() {
        let mut w = W::new(true);
        w.header(0, 1, 0).var(3, "S", 0x0001_0300, None, 2, &[*b"N/A     ", *b"-       "]).end().raw(b"N/A     ");
        let t = read_table(&w.bytes()).unwrap();
        assert_eq!(t.columns[0].missing_codes, ["N/A", "-"]);
    }

    #[test]
    fn the_weight_variable_is_named_in_a_warning() {
        let mut w = W::new(true);
        w.header(0, 1, 2).num("A").num("WEIGHT").end().f64(1.0).f64(2.0);
        let t = read_table(&w.bytes()).unwrap();
        let warning = t.warnings.iter().find(|w| w.kind == "weight_variable_ignored").unwrap();
        assert!(warning.message.contains("'WEIGHT'"), "{}", warning.message);
        // A weight slot that points nowhere is ignored.
        let mut w = W::new(true);
        w.header(0, 1, 9).num("A").end().f64(1.0);
        assert!(read_table(&w.bytes()).unwrap().warnings.is_empty());
    }

    #[test]
    fn repeated_and_empty_names_are_made_unique_and_reported() {
        let mut w = W::new(true);
        w.header(0, 1, 0).num("X").num("X").num("").num("X").end().f64(1.0).f64(2.0).f64(3.0).f64(4.0);
        let t = read_table(&w.bytes()).unwrap();
        let names: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["X", "X_2", "V3", "X_3"]);
        assert!(t.warnings.iter().any(|w| w.kind == "duplicate_variable_names"));
    }

    #[test]
    fn long_names_replace_short_ones_whatever_their_case() {
        let mut w = W::new(true);
        w.header(0, 1, 0).num("AGE").num("OTHER").ext(13, 1, b"age=Age of respondent\tOTHER=Other thing").end();
        w.f64(1.0).f64(2.0);
        let t = read_table(&w.bytes()).unwrap();
        assert_eq!(t.columns[0].name, "Age of respondent");
        assert_eq!(t.columns[1].name, "Other thing");
    }

    #[test]
    fn measures_map_to_levels_and_a_date_format_beats_the_measure() {
        let mut w = W::new(true);
        w.header(0, 1, 0).num("N").num("O").num("S").num("U").var(0, "D", 20 << 16 | 11 << 8, None, 0, &[]).text("T", 8);
        let mut disp = W::new(true);
        for v in [1, 8, 1, 2, 8, 1, 3, 8, 1, 0, 8, 1, 3, 8, 1, 1, 8, 0] {
            disp.i32(v);
        }
        w.ext(11, 4, &disp.b).end();
        for _ in 0..5 {
            w.f64(1.0);
        }
        w.raw(b"x       ");
        let t = read_table(&w.bytes()).unwrap();
        let levels: Vec<Option<Level>> = t.columns.iter().map(|c| c.level).collect();
        assert_eq!(
            levels,
            [Some(Level::Nominal), Some(Level::Ordinal), Some(Level::Ratio), None, Some(Level::Date), Some(Level::Nominal)]
        );
    }

    #[test]
    fn display_records_with_two_fields_per_variable_are_understood() {
        let mut w = W::new(true);
        w.header(0, 1, 0).num("A").num("B");
        let mut disp = W::new(true);
        for v in [2, 1, 3, 1] {
            disp.i32(v);
        }
        w.ext(11, 4, &disp.b).end().f64(1.0).f64(2.0);
        let t = read_table(&w.bytes()).unwrap();
        assert_eq!((t.columns[0].level, t.columns[1].level), (Some(Level::Ordinal), Some(Level::Ratio)));
    }

    // ---- damaged and hostile files ----------------------------------------------------------------------------------------

    #[test]
    fn a_file_with_no_variables_is_refused() {
        let mut w = W::new(true);
        w.header(0, 5, 0).end();
        assert!(err_text(&w.bytes()).contains("no variables"));
    }

    #[test]
    fn odd_headers_are_refused() {
        let mut w = small(true);
        w.b[64..68].copy_from_slice(&5i32.to_le_bytes());
        assert!(err_text(&w.bytes()).contains("not an SPSS data file"), "wrong layout code");
        let mut w = small(true);
        w.b[72..76].copy_from_slice(&3i32.to_le_bytes());
        assert!(err_text(&w.bytes()).contains("compression"), "unknown compression");
        let mut w = small(true);
        w.b[84..92].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(read_table(&w.bytes()).is_err(), "bias that is not a number");
        let mut w = small(true);
        w.b[0] = b'@';
        assert!(err_text(&w.bytes()).contains("not an SPSS data file"));
        assert!(err_text(&small(true).bytes()[..100]).contains("ends too soon"));
        assert!(err_text(b"$FL2").contains("ends too soon"));
    }

    fn with_dictionary(f: impl FnOnce(&mut W)) -> Vec<u8> {
        let mut w = W::new(true);
        w.header(0, 1, 0).num("A");
        f(&mut w);
        w.bytes()
    }

    #[test]
    fn absurd_counts_and_lengths_are_refused_without_large_allocations() {
        // Value labels: a count far beyond the bytes in the file, or negative.
        for count in [i32::MAX, -1] {
            assert!(read_table(&with_dictionary(|w| {
                w.i32(3).i32(count);
            }))
            .is_err());
        }
        // The variable list that follows the labels: too many variables, negative, not a type-4 record.
        for (record, count) in [(4, i32::MAX), (4, -5), (5, 0)] {
            assert!(read_table(&with_dictionary(|w| {
                w.i32(3).i32(0).i32(record).i32(count);
            }))
            .is_err());
        }
        // Extension records with sizes that cannot be real.
        for (size, count) in [(i32::MAX, i32::MAX), (1, i32::MAX), (i32::MAX, 1), (-1, 4), (4, -1), (8, 1 << 29)] {
            let bytes = with_dictionary(|w| {
                w.i32(7).i32(13).i32(size).i32(count).end();
            });
            assert!(read_table(&bytes).is_err(), "{size} x {count}");
        }
        // A document with far more lines than bytes.
        assert!(read_table(&with_dictionary(|w| {
            w.i32(6).i32(i32::MAX);
        }))
        .is_err());
        // A label whose length is beyond the file, or negative.
        for len in [i32::MAX, -1] {
            let mut w = W::new(true);
            w.header(0, 1, 0).i32(2).i32(0).i32(1).i32(0).i32(0).i32(0).padded(b"X", 8).i32(len);
            assert!(read_table(&w.bytes()).is_err(), "label length {len}");
        }
    }

    #[test]
    fn a_file_declaring_an_enormous_number_of_variables_is_refused() {
        let mut w = W::new(true);
        w.header(0, 0, 0);
        for _ in 0..=MAX_VARIABLES {
            w.num("V");
        }
        w.end();
        let e = err_text(&w.bytes());
        assert!(e.contains("more variables"), "{e}");
    }

    #[test]
    fn variable_records_that_break_the_rules_are_refused() {
        // A type nobody defined, and a continuation with nothing before it.
        for ty in [-2, 256, 1000, i32::MIN, -1] {
            let mut w = W::new(true);
            w.header(0, 1, 0).i32(2).i32(ty).i32(0).i32(0).i32(0).i32(0).padded(b"X", 8).end();
            assert!(read_table(&w.bytes()).is_err(), "type {ty}");
        }
        // A bad label flag.
        let mut w = W::new(true);
        w.header(0, 1, 0).i32(2).i32(0).i32(7).i32(0).i32(0).i32(0).padded(b"X", 8).end();
        assert!(read_table(&w.bytes()).is_err());
        // A text of 16 bytes whose continuation record is missing, or is a real variable.
        let mut w = W::new(true);
        w.header(0, 1, 0).i32(2).i32(16).i32(0).i32(0).i32(0).i32(0).padded(b"T", 8).end();
        assert!(err_text(&w.bytes()).contains("continuation"));
        let mut w = W::new(true);
        w.header(0, 1, 0).i32(2).i32(16).i32(0).i32(0).i32(0).i32(0).padded(b"T", 8).num("B").end();
        assert!(err_text(&w.bytes()).contains("continuation"));
    }

    #[test]
    fn records_of_unknown_kinds_and_a_missing_end_are_refused() {
        let e = err_text(&with_dictionary(|w| {
            w.i32(5).i32(0);
        }));
        assert!(e.contains("unknown kind"), "{e}");
        let e = err_text(&with_dictionary(|w| {
            w.i32(4).i32(0);
        }));
        assert!(e.contains("no labels"), "{e}");
        let e = err_text(&with_dictionary(|_| {}));
        assert!(e.contains("ends too soon"), "{e}");
    }

    #[test]
    fn value_labels_for_a_variable_that_does_not_exist_are_refused() {
        for slot in [0, 2, 9, -1, i32::MAX] {
            let e = err_text(&with_dictionary(|w| {
                w.i32(3).i32(1).f64(1.0).raw(&[3]).raw(b"Yes\0\0\0\0").i32(4).i32(1).i32(slot).end().f64(1.0);
            }));
            assert!(e.contains("does not exist"), "slot {slot}: {e}");
        }
        // The one that does exist works.
        let t = read_table(&with_dictionary(|w| {
            w.i32(3).i32(1).f64(1.0).raw(&[3]).raw(b"Yes\0\0\0\0").i32(4).i32(1).i32(1).end().f64(1.0);
        }))
        .unwrap();
        assert_eq!(t.columns[0].value_labels["1"], "Yes");
    }

    #[test]
    fn a_label_list_shared_by_enormous_numbers_of_variables_is_refused() {
        // 3000 labels used by 3000 variables would be nine million copies.
        let mut w = W::new(true);
        w.header(0, 1, 0).num("A").i32(3).i32(3000);
        for k in 0..3000 {
            w.f64(f64::from(k)).raw(&[1]).raw(b"x      ");
        }
        w.i32(4).i32(3000);
        for _ in 0..3000 {
            w.i32(1);
        }
        w.end().f64(1.0);
        let e = err_text(&w.bytes());
        assert!(e.contains("more value labels"), "{e}");
    }

    #[test]
    fn case_counts_that_the_data_cannot_hold_are_refused_quickly() {
        let mut w = W::new(true);
        w.header(0, i32::MAX, 0);
        for k in 0..100 {
            w.num(&format!("V{k}"));
        }
        w.end().f64(1.0);
        let e = err_text(&w.bytes());
        assert!(e.contains("more than this version can load"), "{e}");
        // A count that fits the cell limit but not the bytes.
        let mut w = W::new(true);
        w.header(0, 1_000_000, 0).num("A").end().f64(1.0).f64(2.0);
        assert!(err_text(&w.bytes()).contains("ends too soon"));
        let mut w = W::new(true);
        w.header(1, 1_000_000, 0).num("A").end().raw(&[101, 102, 252, 0, 0, 0, 0, 0]);
        assert!(err_text(&w.bytes()).contains("ends too soon"));
    }

    #[test]
    fn data_that_stops_in_the_middle_of_a_case_or_value_is_refused() {
        let mut w = W::new(true);
        w.header(0, -1, 0).num("A").num("B").end().f64(1.0).f64(2.0).f64(3.0);
        assert!(err_text(&w.bytes()).contains("cut off"));
        let mut w = W::new(true);
        w.header(0, -1, 0).num("A").end().f64(1.0).raw(&[1, 2, 3]);
        assert!(err_text(&w.bytes()).contains("middle of a value"));
        // A raw value announced by the command bytes but missing from the file.
        let mut w = W::new(true);
        w.header(1, -1, 0).num("A").end().raw(&[253, 0, 0, 0, 0, 0, 0, 0]);
        assert!(err_text(&w.bytes()).contains("where a value should be"));
        // Bytecoded data cut inside a case.
        let mut w = W::new(true);
        w.header(1, -1, 0).num("A").num("B").end().raw(&[101, 252, 0, 0, 0, 0, 0, 0]);
        assert!(err_text(&w.bytes()).contains("cut off"));
    }

    fn very_long_file(claimed: &str, parts: &[i32]) -> Vec<u8> {
        let mut w = W::new(true);
        w.header(0, 0, 0);
        for (k, width) in parts.iter().enumerate() {
            w.text(&if k == 0 { "VL".to_string() } else { format!("VL{k}") }, *width);
        }
        w.ext(14, 1, claimed.as_bytes()).end();
        w.bytes()
    }

    #[test]
    fn very_long_text_records_must_make_sense() {
        // 300 bytes: 255 + 48 is fine.
        assert!(read_table(&very_long_file("VL=300\0\t", &[255, 48])).is_ok());
        // A length above what SPSS allows, parts missing, and parts too small.
        assert!(read_table(&very_long_file("VL=40000\0\t", &[255, 48])).is_err());
        assert!(read_table(&very_long_file("VL=300\0\t", &[255])).is_err());
        assert!(read_table(&very_long_file("VL=1000\0\t", &[255, 255, 8, 8])).is_err());
        // A part that is a number.
        let mut w = W::new(true);
        w.header(0, 0, 0).text("VL", 255).num("VL1").ext(14, 1, b"VL=300\0\t").end();
        assert!(err_text(&w.bytes()).contains("not text"));
        // Nonsense in the record is ignored, not fatal.
        assert!(read_table(&very_long_file("garbage\t=\t=5=\t\u{1}", &[255])).is_ok());
    }

    #[test]
    fn broken_long_text_extensions_become_warnings_not_failures() {
        let mut w = W::new(true);
        w.header(0, 0, 0).text("T", 12).ext(21, 1, &[1, 2, 3]).ext(22, 1, &[9, 0, 0, 0, b'T']).end();
        let t = read_table(&w.bytes()).unwrap();
        assert_eq!(t.warnings.iter().filter(|w| w.kind == "extension_record_unreadable").count(), 2);
        // Counts that exceed the record.
        let mut w = W::new(true);
        w.header(0, 0, 0).text("T", 12);
        let mut rec = W::new(true);
        rec.i32(1).raw(b"T").i32(16).i32(i32::MAX);
        w.ext(21, 1, &rec.b).end();
        assert!(read_table(&w.bytes()).unwrap().warnings.iter().any(|w| w.kind == "extension_record_unreadable"));
    }

    #[test]
    fn long_text_labels_and_missing_values_are_read_by_name() {
        let mut w = W::new(true);
        w.header(0, 0, 0).text("T", 12).num("N");
        let mut labels = W::new(true);
        labels.i32(1).raw(b"T").i32(12).i32(2);
        labels.i32(12).raw(b"alpha       ").i32(5).raw(b"First");
        labels.i32(3).raw(b"abc").i32(6).raw(b"Second");
        let mut missing = W::new(true);
        missing.i32(1).raw(b"t").raw(&[2]).i32(8).raw(b"NA      ").raw(b"-9      ");
        w.ext(21, 1, &labels.b).ext(22, 1, &missing.b).end();
        let t = read_table(&w.bytes()).unwrap();
        assert_eq!(t.columns[0].value_labels["alpha"], "First");
        assert_eq!(t.columns[0].value_labels["abc"], "Second");
        assert_eq!(t.columns[0].missing_codes, ["NA", "-9"]);
        assert!(t.columns[1].value_labels.is_empty());
    }

    // ---- zlib data -------------------------------------------------------------------------------------------------------

    fn zsav_fixture() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sav/basic_zsav.zsav");
        std::fs::read(path).unwrap()
    }

    fn data_start(bytes: &[u8]) -> usize {
        let header = parse_header(bytes).unwrap();
        let mut cur = Cur::new(bytes, HEADER_LEN, header.le);
        read_dictionary(&mut cur).unwrap();
        cur.pos
    }

    #[test]
    fn damaged_zlib_layouts_are_refused() {
        let good = zsav_fixture();
        assert!(read_table(&good).is_ok());
        let start = data_start(&good);
        // Each of the two offsets in the three-number header, set to something wrong.
        for (offset, value) in [(8usize, -1i64), (8, i64::MAX), (8, 0), (16, -1), (16, i64::MAX), (16, 5), (16, 1 << 40)] {
            let mut bad = good.clone();
            bad[start + offset..start + offset + 8].copy_from_slice(&value.to_le_bytes());
            assert!(read_table(&bad).is_err(), "header number at +{offset} set to {value}");
        }
        // The trailer: bias (8), zero (8), block size (4), block count (4), then 24 bytes per block.
        let trailer = i64::from_le_bytes(good[start + 8..start + 16].try_into().unwrap()) as usize;
        for (offset, value) in [(20usize, i32::MAX), (20, -1), (20, 1 << 20), (40, -1), (40, 1 << 30), (44, -1), (44, i32::MAX)] {
            let mut bad = good.clone();
            let at = trailer + offset;
            bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(read_table(&bad).is_err(), "trailer number at +{offset} set to {value}");
        }
        // A block that starts outside the data.
        for value in [1i64 << 50, -5, 0] {
            let mut bad = good.clone();
            let at = trailer + 32;
            bad[at..at + 8].copy_from_slice(&value.to_le_bytes());
            assert!(read_table(&bad).is_err(), "block offset set to {value}");
        }
        // Compressed bytes that are not zlib.
        let mut bad = good.clone();
        for b in &mut bad[start + 26..start + 60] {
            *b ^= 0x5A;
        }
        assert!(read_table(&bad).is_err());
        // A file that stops where the data should start, or just after the three numbers.
        assert!(read_table(&good[..start + 10]).is_err());
        assert!(read_table(&good[..start + 24]).is_err());
        // No data at all is the same as no cases, which this file contradicts.
        assert!(read_table(&good[..start]).is_err());
    }

    #[test]
    fn a_zlib_file_with_no_data_and_no_cases_is_an_empty_table() {
        let mut w = W::new(true);
        w.header(2, 0, 0).num("A").end();
        let t = read_table(&w.bytes()).unwrap();
        assert!(t.rows.is_empty());
    }
}
