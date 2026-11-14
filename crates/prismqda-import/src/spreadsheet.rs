//! CSV, TSV, and XLSX reading into a [`Table`] (specification section 5).
//!
//! Cell types are inferred conservatively. A column becomes numeric, or a date, only when every
//! non-empty cell is. "007" stays text, so identifiers with leading zeros are never mangled.

use std::collections::HashSet;
use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};
use prismqda_engine::ImportWarning;

use crate::error::{ImportError, Result};
use crate::table::{Cell, Column, Table};
use crate::text;

/// Reads delimited text. With no delimiter given it picks comma, semicolon, tab, or bar by counting
/// them in the first line outside quotes.
pub fn read_csv(bytes: &[u8], name: &str, delimiter: Option<u8>) -> Result<Table> {
    let (decoded, warning) = text::decode(bytes);
    let decoded = text::clean(&decoded);
    let delimiter = delimiter.unwrap_or_else(|| sniff_delimiter(&decoded));
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(decoded.as_bytes());

    let mut records: Vec<Vec<String>> = Vec::new();
    for (i, record) in reader.records().enumerate() {
        let record = record.map_err(|e| ImportError::format(format!("the file could not be read as delimited text near row {}: {e}", i + 1)))?;
        records.push(record.iter().map(str::to_string).collect());
    }
    let mut warnings: Vec<ImportWarning> = warning.into_iter().collect();
    let table = records_to_table(name, records, &mut warnings)?;
    Ok(Table { warnings, ..table })
}

fn sniff_delimiter(text: &str) -> u8 {
    let first: String = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(8192).collect();
    let mut counts = [(b',', 0usize), (b';', 0), (b'\t', 0), (b'|', 0)];
    let mut quoted = false;
    for c in first.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if !quoted {
            for (d, n) in counts.iter_mut() {
                if c == *d as char {
                    *n += 1;
                }
            }
        }
    }
    counts.iter().max_by_key(|(_, n)| *n).filter(|(_, n)| *n > 0).map(|(d, _)| *d).unwrap_or(b',')
}

fn records_to_table(name: &str, mut records: Vec<Vec<String>>, warnings: &mut Vec<ImportWarning>) -> Result<Table> {
    let first = records.iter().position(|r| r.iter().any(|c| !c.trim().is_empty()));
    let Some(first) = first else {
        return Err(ImportError::format("the file has no rows"));
    };
    let header = records[first].clone();
    records.drain(..=first);

    let names = unique_names(&header);
    let width = names.len();
    let mut rows: Vec<Vec<Cell>> = Vec::with_capacity(records.len());
    let (mut blank, mut extra) = (0usize, 0usize);
    for r in records {
        if r.iter().all(|c| c.trim().is_empty()) {
            blank += 1;
            continue;
        }
        if r.len() > width && r[width..].iter().any(|c| !c.trim().is_empty()) {
            extra += 1;
        }
        let mut row: Vec<Cell> = r.into_iter().take(width).map(|c| if c.trim().is_empty() { Cell::Empty } else { Cell::Text(c) }).collect();
        row.resize(width, Cell::Empty);
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(ImportError::format("the file has a header row but no data rows"));
    }
    if blank > 0 {
        warnings.push(ImportWarning::new("blank_rows", format!("{blank} blank row(s) were skipped")));
    }
    if extra > 0 {
        warnings.push(ImportWarning::new(
            "extra_cells",
            format!("{extra} row(s) had more cells than the header; the extra cells were not imported. A comma inside unquoted text is the usual cause."),
        ));
    }
    infer_types(&mut rows, width);
    Ok(Table { name: name.to_string(), columns: names.into_iter().map(Column::new).collect(), rows, warnings: Vec::new() })
}

fn unique_names(header: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    header
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let base = if h.trim().is_empty() { format!("column {}", i + 1) } else { h.trim().to_string() };
            let mut candidate = base.clone();
            let mut n = 2;
            while !seen.insert(candidate.clone()) {
                candidate = format!("{base} ({n})");
                n += 1;
            }
            candidate
        })
        .collect()
}

/// Turns a column's text cells into numbers or dates when every non-empty cell is one.
fn infer_types(rows: &mut [Vec<Cell>], width: usize) {
    for c in 0..width {
        let texts: Vec<&str> = rows
            .iter()
            .filter_map(|r| match &r[c] {
                Cell::Text(s) => Some(s.trim()),
                _ => None,
            })
            .collect();
        if texts.is_empty() {
            continue;
        }
        if texts.iter().all(|t| is_number(t)) {
            for r in rows.iter_mut() {
                if let Cell::Text(s) = &r[c] {
                    r[c] = Cell::Number(s.trim().parse().unwrap_or(f64::NAN));
                }
            }
        } else if texts.iter().all(|t| is_iso_date(t)) {
            for r in rows.iter_mut() {
                if let Cell::Text(s) = &r[c] {
                    r[c] = Cell::Date(s.trim().to_string());
                }
            }
        }
    }
}

/// A plain decimal number: optional sign, digits, optional fraction, optional exponent. No thousands
/// separators, no "NaN", and no leading zeros ("007" is an identifier, not a number).
fn is_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_len = i - int_start;
    let mut frac_len = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let f = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_len = i - f;
    }
    if int_len + frac_len == 0 {
        return false;
    }
    if int_len > 1 && b[int_start] == b'0' {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let e = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == e {
            return false;
        }
    }
    i == b.len()
}

/// `YYYY-MM-DD`, optionally followed by `T` or a space and a time, optionally with a zone.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 10 || !(b[4] == b'-' && b[7] == b'-') {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let (m, d) = (num(&b[5..7]), num(&b[8..10]));
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return false;
    }
    if b.len() == 10 {
        return true;
    }
    if b[10] != b'T' && b[10] != b' ' {
        return false;
    }
    let rest = &s[11..];
    let time: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':' || *c == '.').collect();
    let zone = &rest[time.len()..];
    let parts: Vec<&str> = time.split(':').collect();
    let time_ok = (2..=3).contains(&parts.len())
        && parts[0].len() == 2
        && parts[1].len() == 2
        && num(parts[0].as_bytes()) < 24
        && num(parts[1].as_bytes()) < 60;
    let zone_ok = zone.is_empty()
        || zone == "Z"
        || (zone.len() == 6 && (zone.starts_with('+') || zone.starts_with('-')) && zone.as_bytes()[3] == b':')
        || (zone.len() == 5 && (zone.starts_with('+') || zone.starts_with('-')));
    time_ok && zone_ok
}

fn num(b: &[u8]) -> u32 {
    b.iter().filter(|c| c.is_ascii_digit()).fold(0, |a, c| a * 10 + (c - b'0') as u32)
}

/// The names of the sheets in a workbook, in order.
pub fn xlsx_sheet_names(bytes: &[u8]) -> Result<Vec<String>> {
    let wb = open_xlsx(bytes)?;
    Ok(wb.sheet_names())
}

fn open_xlsx(bytes: &[u8]) -> Result<Xlsx<Cursor<&[u8]>>> {
    calamine::open_workbook_from_rs(Cursor::new(bytes))
        .map_err(|e| ImportError::format(format!("the workbook could not be opened; it may be damaged or password protected: {e}")))
}

/// Reads the first visible sheet.
pub fn read_xlsx(bytes: &[u8], name: &str) -> Result<Table> {
    read_xlsx_sheet(bytes, name, None)
}

/// Reads one sheet (the first visible one when none is named). The first non-empty row is the header.
pub fn read_xlsx_sheet(bytes: &[u8], name: &str, sheet: Option<&str>) -> Result<Table> {
    let mut wb = open_xlsx(bytes)?;
    let names = wb.sheet_names();
    if names.is_empty() {
        return Err(ImportError::format("the workbook has no sheets"));
    }
    let mut warnings: Vec<ImportWarning> = Vec::new();
    let chosen = match sheet {
        Some(s) => names
            .iter()
            .find(|n| n.as_str() == s)
            .cloned()
            .ok_or_else(|| ImportError::format(format!("the workbook has no sheet named '{s}'")))?,
        None => wb
            .sheets_metadata()
            .iter()
            .find(|m| m.visible == calamine::SheetVisible::Visible)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| names[0].clone()),
    };
    if sheet.is_none() && names.len() > 1 {
        let others: Vec<&str> = names.iter().filter(|n| **n != chosen).map(String::as_str).collect();
        warnings.push(ImportWarning::new(
            "other_sheets_not_imported",
            format!("Only the sheet '{chosen}' was imported. Other sheets: {}.", others.join(", ")),
        ));
    }
    let range = wb
        .worksheet_range(&chosen)
        .map_err(|e| ImportError::format(format!("the sheet '{chosen}' could not be read: {e}")))?;

    let mut errors = 0usize;
    let records: Vec<Vec<Cell>> = range
        .rows()
        .map(|row| {
            row.iter()
                .map(|d| match d {
                    Data::Empty => Cell::Empty,
                    Data::String(s) => {
                        if s.trim().is_empty() {
                            Cell::Empty
                        } else {
                            Cell::Text(s.clone())
                        }
                    }
                    Data::Float(f) => Cell::Number(*f),
                    Data::Int(i) => Cell::Number(*i as f64),
                    Data::Bool(b) => Cell::Bool(*b),
                    // calamine's as_f64 already converts the old 1904 date system to the standard one.
                    Data::DateTime(dt) => Cell::Date(excel_serial_to_iso(dt.as_f64(), false)),
                    Data::DateTimeIso(s) => Cell::Date(s.clone()),
                    Data::DurationIso(s) => Cell::Text(s.clone()),
                    Data::Error(_) => {
                        errors += 1;
                        Cell::Empty
                    }
                })
                .collect()
        })
        .collect();
    if errors > 0 {
        warnings.push(ImportWarning::new("cell_errors", format!("{errors} cell(s) held a formula error and were left empty")));
    }

    let Some(first) = records.iter().position(|r| r.iter().any(|c| !c.is_empty())) else {
        return Err(ImportError::format(format!("the sheet '{chosen}' is empty")));
    };
    let names = unique_names(&records[first].iter().map(Cell::text).collect::<Vec<_>>());
    let width = names.len();
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut blank = 0usize;
    for r in &records[first + 1..] {
        if r.iter().all(Cell::is_empty) {
            blank += 1;
            continue;
        }
        let mut row: Vec<Cell> = r.iter().take(width).cloned().collect();
        row.resize(width, Cell::Empty);
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(ImportError::format(format!("the sheet '{chosen}' has a header row but no data rows")));
    }
    if blank > 0 {
        warnings.push(ImportWarning::new("blank_rows", format!("{blank} blank row(s) were skipped")));
    }
    Ok(Table { name: name.to_string(), columns: names.into_iter().map(Column::new).collect(), rows, warnings })
}

/// Excel stores dates as days since 1899-12-30 (or 1904-01-01 in the older Mac system) with the time as a fraction.
fn excel_serial_to_iso(serial: f64, is_1904: bool) -> String {
    if !serial.is_finite() {
        return String::new();
    }
    // Serial 0 is 1899-12-30 in the standard system and 1904-01-01 in the older Mac one.
    let days_from_epoch = if is_1904 { days_from_civil(1904, 1, 1) } else { days_from_civil(1899, 12, 30) };
    let whole = serial.floor();
    let secs = ((serial - whole) * 86_400.0).round() as i64;
    let (day_shift, secs) = if secs >= 86_400 { (1, secs - 86_400) } else { (0, secs) };
    let (y, m, d) = civil_from_days(days_from_epoch + whole as i64 + day_shift);
    if secs == 0 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

/// Days since 1970-01-01 for a calendar date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn t(s: &str) -> Cell {
        Cell::Text(s.to_string())
    }

    // ---- CSV ----

    #[test]
    fn csv_columns_become_numbers_dates_or_text() {
        let csv = "id,age,when,comment\n007,34,2026-01-02,\"Hello, world\"\n008,51,2026-02-03 10:30:00,\"Line one\nline two\"\n";
        let table = read_csv(csv.as_bytes(), "S", None).unwrap();
        assert_eq!(table.name, "S");
        let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["id", "age", "when", "comment"]);
        assert_eq!(table.rows[0][0], t("007"), "leading zeros keep an identifier as text");
        assert_eq!(table.rows[0][1], Cell::Number(34.0));
        assert_eq!(table.rows[1][2], Cell::Date("2026-02-03 10:30:00".into()));
        assert_eq!(table.rows[0][3], t("Hello, world"));
        assert_eq!(table.rows[1][3], t("Line one\nline two"));
    }

    #[test]
    fn the_delimiter_is_guessed_from_the_first_line() {
        for (sep, text) in [(";", "a;b\n1;2\n"), ("\t", "a\tb\n1\t2\n"), ("|", "a|b\n1|2\n"), (",", "a,b\n1,2\n")] {
            let table = read_csv(text.as_bytes(), "S", None).unwrap();
            assert_eq!(table.columns.len(), 2, "delimiter {sep:?}");
            assert_eq!(table.rows[0][1], Cell::Number(2.0));
        }
        // A comma inside quotes in the header does not count.
        let table = read_csv("\"a,b\";c\n1;2\n".as_bytes(), "S", None).unwrap();
        assert_eq!(table.columns.len(), 2);
        // An explicit delimiter wins.
        let table = read_csv(b"a;b,c\n1;2,3\n", "S", Some(b',')).unwrap();
        assert_eq!(table.columns.len(), 2);
    }

    #[test]
    fn csv_handles_bom_blank_names_duplicates_and_ragged_rows() {
        let csv = "\u{feff}name,,name\nAnn,x,Bo\n\n,,\nCy,y\nDi,z,Ed,extra\n";
        let table = read_csv(csv.as_bytes(), "S", None).unwrap();
        let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["name", "column 2", "name (2)"]);
        assert_eq!(table.rows.len(), 3, "the blank rows are skipped");
        assert_eq!(table.rows[1][2], Cell::Empty, "a short row is padded");
        let kinds: Vec<&str> = table.warnings.iter().map(|w| w.kind.as_str()).collect();
        assert!(kinds.contains(&"blank_rows") && kinds.contains(&"extra_cells"), "{kinds:?}");
    }

    #[test]
    fn csv_in_windows_1252_is_read_with_a_warning() {
        let table = read_csv(b"name\ncaf\xE9\n", "S", None).unwrap();
        assert_eq!(table.rows[0][0], t("caf\u{e9}"));
        assert!(table.warnings.iter().any(|w| w.kind == "assumed_encoding"));
    }

    #[test]
    fn empty_and_header_only_csv_files_are_explained() {
        assert!(read_csv(b"", "S", None).unwrap_err().to_string().contains("no rows"));
        assert!(read_csv(b"\n\n", "S", None).unwrap_err().to_string().contains("no rows"));
        assert!(read_csv(b"a,b\n", "S", None).unwrap_err().to_string().contains("no data rows"));
    }

    #[test]
    fn what_counts_as_a_number() {
        for ok in ["0", "-1", "+2.5", "3.", ".5", "1e5", "-1.5E-3", "10", "0.5"] {
            assert!(is_number(ok), "{ok}");
        }
        for bad in ["007", "1,000", "NaN", "inf", "1e", "", "-", ".", "1.2.3", "12a", "0x10", "00.5"] {
            assert!(!is_number(bad), "{bad}");
        }
    }

    #[test]
    fn what_counts_as_a_date() {
        for ok in ["2026-01-02", "2026-01-02T10:30", "2026-01-02 10:30:15", "2026-01-02T10:30:15.250Z", "2026-01-02T10:30:15+05:00", "2026-01-02T10:30:15-0500"] {
            assert!(is_iso_date(ok), "{ok}");
        }
        for bad in ["2026-13-02", "2026-01-32", "01/02/2026", "2026-1-2", "2026-01-02x", "2026-01-02T25:00", "last week"] {
            assert!(!is_iso_date(bad), "{bad}");
        }
    }

    #[test]
    fn mixed_columns_stay_text() {
        let table = read_csv(b"v\n1\ntwo\n3\n", "S", None).unwrap();
        assert_eq!(table.rows[0][0], t("1"));
    }

    // ---- dates ----

    #[test]
    fn excel_serials_become_iso_dates() {
        assert_eq!(excel_serial_to_iso(1.0, false), "1899-12-31");
        assert_eq!(excel_serial_to_iso(61.0, false), "1900-03-01");
        assert_eq!(excel_serial_to_iso(45_000.0, false), "2023-03-15");
        // 2025-01-01 is serial 45658, and 2025 has 365 days, so 46023 is 2026-01-01.
        assert_eq!(excel_serial_to_iso(45_658.0, false), "2025-01-01");
        assert_eq!(excel_serial_to_iso(46_023.0, false), "2026-01-01");
        assert_eq!(excel_serial_to_iso(46_023.5, false), "2026-01-01T12:00:00");
        assert_eq!(excel_serial_to_iso(46_023.999_999_9, false), "2026-01-02", "a time that rounds up to midnight rolls to the next day");
        assert_eq!(excel_serial_to_iso(0.0, true), "1904-01-01");
        assert_eq!(excel_serial_to_iso(f64::NAN, false), "");
    }

    #[test]
    fn calendar_arithmetic_round_trips() {
        for (y, m, d) in [(1970, 1, 1), (2000, 2, 29), (1900, 3, 1), (2026, 11, 4), (1899, 12, 30)] {
            assert_eq!(civil_from_days(days_from_civil(y, m, d)), (y, m, d));
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    // ---- XLSX ----

    /// A minimal workbook: sheets of (name, rows), where a cell is text, a number, or a date serial.
    enum X {
        S(&'static str),
        N(f64),
        D(f64),
        E,
    }

    fn workbook(sheets: &[(&str, Vec<Vec<X>>)], hidden_first: bool) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        let mut put = |name: &str, body: &str| {
            zip.start_file(name, o).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        };
        let overrides: String = (1..=sheets.len())
            .map(|i| format!("<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"))
            .collect();
        put("[Content_Types].xml", &format!("<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>{overrides}</Types>"));
        put("_rels/.rels", "<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>");
        let sheet_tags: String = sheets
            .iter()
            .enumerate()
            .map(|(i, (n, _))| {
                let state = if hidden_first && i == 0 { " state=\"hidden\"" } else { "" };
                format!("<sheet name=\"{n}\" sheetId=\"{}\"{state} r:id=\"rId{}\"/>", i + 1, i + 1)
            })
            .collect();
        put("xl/workbook.xml", &format!("<?xml version=\"1.0\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>{sheet_tags}</sheets></workbook>"));
        let rels: String = (1..=sheets.len())
            .map(|i| format!("<Relationship Id=\"rId{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"))
            .collect();
        put("xl/_rels/workbook.xml.rels", &format!("<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}</Relationships>"));
        put("xl/styles.xml", "<?xml version=\"1.0\"?><styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"1\"><font><sz val=\"11\"/></font></fonts><fills count=\"1\"><fill><patternFill patternType=\"none\"/></fill></fills><borders count=\"1\"><border/></borders><cellStyleXfs count=\"1\"><xf/></cellStyleXfs><cellXfs count=\"2\"><xf numFmtId=\"0\"/><xf numFmtId=\"14\" applyNumberFormat=\"1\"/></cellXfs></styleSheet>");
        for (i, (_, rows)) in sheets.iter().enumerate() {
            let mut body = String::new();
            for (r, row) in rows.iter().enumerate() {
                body.push_str(&format!("<row r=\"{}\">", r + 1));
                for (c, cell) in row.iter().enumerate() {
                    let at = format!("{}{}", (b'A' + c as u8) as char, r + 1);
                    match cell {
                        X::S(s) => body.push_str(&format!("<c r=\"{at}\" t=\"inlineStr\"><is><t>{s}</t></is></c>")),
                        X::N(n) => body.push_str(&format!("<c r=\"{at}\"><v>{n}</v></c>")),
                        X::D(n) => body.push_str(&format!("<c r=\"{at}\" s=\"1\"><v>{n}</v></c>")),
                        X::E => body.push_str(&format!("<c r=\"{at}\" t=\"e\"><v>#DIV/0!</v></c>")),
                    }
                }
                body.push_str("</row>");
            }
            put(&format!("xl/worksheets/sheet{}.xml", i + 1), &format!("<?xml version=\"1.0\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>{body}</sheetData></worksheet>"));
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn xlsx_cells_keep_their_types() {
        let wb = workbook(
            &[("Responses", vec![
                vec![X::S("id"), X::S("score"), X::S("when"), X::S("answer")],
                vec![X::S("R1"), X::N(4.0), X::D(46_023.0), X::S("I felt welcome &amp; safe")],
                vec![X::S("R2"), X::N(2.5), X::D(46_024.5), X::E],
            ])],
            false,
        );
        let table = read_xlsx(&wb, "Survey").unwrap();
        assert_eq!(table.name, "Survey");
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0][0], t("R1"));
        assert_eq!(table.rows[0][1], Cell::Number(4.0));
        assert_eq!(table.rows[0][2], Cell::Date("2026-01-01".into()));
        assert_eq!(table.rows[1][2], Cell::Date("2026-01-02T12:00:00".into()));
        assert_eq!(table.rows[0][3], t("I felt welcome & safe"));
        assert_eq!(table.rows[1][3], Cell::Empty);
        assert!(table.warnings.iter().any(|w| w.kind == "cell_errors"));
    }

    #[test]
    fn xlsx_picks_the_first_visible_sheet_and_names_the_others() {
        let wb = workbook(
            &[
                ("Hidden", vec![vec![X::S("a")], vec![X::S("x")]]),
                ("Data", vec![vec![X::S("b")], vec![X::S("y")]]),
                ("More", vec![vec![X::S("c")], vec![X::S("z")]]),
            ],
            true,
        );
        let table = read_xlsx(&wb, "W").unwrap();
        assert_eq!(table.columns[0].name, "b");
        let w = table.warnings.iter().find(|w| w.kind == "other_sheets_not_imported").unwrap();
        assert!(w.message.contains("More") && w.message.contains("Hidden"), "{}", w.message);
        assert_eq!(xlsx_sheet_names(&wb).unwrap(), ["Hidden", "Data", "More"]);
        let chosen = read_xlsx_sheet(&wb, "W", Some("More")).unwrap();
        assert_eq!(chosen.columns[0].name, "c");
        assert!(read_xlsx_sheet(&wb, "W", Some("Nope")).unwrap_err().to_string().contains("no sheet named 'Nope'"));
    }

    #[test]
    fn damaged_workbooks_are_refused_without_panicking() {
        assert!(read_xlsx(b"PK\x03\x04 not really", "W").is_err());
        assert!(read_xlsx(b"", "W").is_err());
        let good = workbook(&[("S", vec![vec![X::S("a")], vec![X::S("b")]])], false);
        assert!(read_xlsx(&good[..good.len() / 2], "W").is_err(), "a truncated workbook");
        let empty = workbook(&[("S", vec![])], false);
        assert!(read_xlsx(&empty, "W").unwrap_err().to_string().contains("empty"));
    }
}
