//! The SPSS reader against files written by an independent writer.
//!
//! Every file in `tests/fixtures/sav/` was written by `haven::write_sav` (the ReadStat library) and
//! has an `.expected.json` that R wrote after reading the same file back with `haven::read_sav`.
//! `make_fixtures.R` in that folder builds them. These tests compare the Rust reader's table to
//! R's reading, cell by cell, and also check that damaged versions of the real files never panic.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use prismqda_engine::{Level, NewProject, OpenOptions, Project};
use prismqda_import::{import_table, open_bytes, sav, Cell, ImportOptions, Opened, SurveyMapping, Table};
use serde_json::Value;
use tempfile::TempDir;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("sav")
}

fn bytes_of(file: &str) -> Vec<u8> {
    fs::read(dir().join(file)).unwrap_or_else(|e| panic!("cannot read fixture {file}: {e}"))
}

fn expected_of(file: &str) -> Value {
    let name = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let text = fs::read_to_string(dir().join(format!("{name}.expected.json"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn read(file: &str) -> Table {
    sav::read_table(&bytes_of(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn level_of(measure: &str) -> Level {
    match measure {
        "nominal" => Level::Nominal,
        "ordinal" => Level::Ordinal,
        "scale" => Level::Ratio,
        "date" => Level::Date,
        other => panic!("unknown measure {other}"),
    }
}

/// Everything that differs between the table and what R read, as sentences.
fn differences(table: &Table, expected: &Value) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    let cols = expected["columns"].as_array().unwrap();
    let nrow = expected["nrow"].as_u64().unwrap() as usize;
    if table.columns.len() != cols.len() {
        bad.push(format!("{} columns, R read {}", table.columns.len(), cols.len()));
        return bad;
    }
    if table.rows.len() != nrow {
        bad.push(format!("{} rows, R read {nrow}", table.rows.len()));
    }
    let mut range_columns: BTreeSet<String> = BTreeSet::new();
    for (i, (col, want)) in table.columns.iter().zip(cols).enumerate() {
        let name = want["name"].as_str().unwrap();
        if col.name != name {
            bad.push(format!("column {i}: name {:?}, R read {name:?}", col.name));
        }
        let label = want["label"].as_str().map(str::to_string);
        if col.label != label {
            bad.push(format!("{name}: label {:?}, R read {label:?}", col.label));
        }
        let level = level_of(want["measure"].as_str().unwrap());
        if col.level != Some(level) {
            bad.push(format!("{name}: level {:?}, expected {level:?}", col.level));
        }
        let labels: BTreeMap<String, String> = want["value_labels"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect();
        if col.value_labels != labels {
            bad.push(format!("{name}: value labels {:?}, R read {labels:?}", col.value_labels));
        }
        let missing: Vec<String> = want["na_values"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        if col.missing_codes != missing {
            bad.push(format!("{name}: missing codes {:?}, R read {missing:?}", col.missing_codes));
        }
        if !want["na_range"].is_null() {
            range_columns.insert(name.to_string());
        }
        let kind = want["kind"].as_str().unwrap();
        let values = want["values"].as_array().unwrap();
        if values.len() != table.rows.len() {
            bad.push(format!("{name}: R read {} values but the table has {} rows", values.len(), table.rows.len()));
            continue;
        }
        for (r, want_cell) in values.iter().enumerate() {
            let got = &table.rows[r][i];
            let ok = match (kind, want_cell) {
                (_, Value::Null) => *got == Cell::Empty,
                ("number", Value::Number(n)) => *got == Cell::Number(n.as_f64().unwrap()),
                ("text", Value::String(s)) => *got == Cell::Text(s.clone()),
                ("date" | "datetime", Value::String(s)) => *got == Cell::Date(s.clone()),
                _ => false,
            };
            if !ok {
                bad.push(format!("{name}, row {r}: got {got:?}, R read {want_cell}"));
            }
        }
    }
    // A missing range cannot be carried into attributes, so each one must be reported by variable name.
    let reported: BTreeSet<String> = table
        .warnings
        .iter()
        .filter(|w| w.kind == "missing_values_not_fully_carried")
        .map(|w| w.location.clone().unwrap_or_default())
        .collect();
    if reported != range_columns {
        bad.push(format!("missing-range warnings name {reported:?}, but R shows ranges on {range_columns:?}"));
    }
    bad
}

fn warning_kinds(table: &Table) -> BTreeSet<&str> {
    table.warnings.iter().map(|w| w.kind.as_str()).collect()
}

fn check(file: &str, allowed_warnings: &[&str]) -> Table {
    let table = read(file);
    let bad = differences(&table, &expected_of(file));
    assert!(bad.is_empty(), "{file} differs from what R read:\n{}", bad.join("\n"));
    let kinds = warning_kinds(&table);
    let allowed: BTreeSet<&str> = allowed_warnings.iter().copied().collect();
    assert_eq!(kinds, allowed, "{file}: warnings {:?}", table.warnings);
    table
}

// ---- cell-by-cell against R ----------------------------------------------------------------------------

#[test]
fn basic_uncompressed_matches_r() {
    check("basic_none.sav", &[]);
}

#[test]
fn basic_byte_compressed_matches_r() {
    check("basic_byte.sav", &[]);
}

#[test]
fn basic_zlib_compressed_matches_r() {
    check("basic_zsav.zsav", &[]);
}

#[test]
fn missing_values_match_r_and_ranges_are_reported() {
    let t = check("missing_byte.sav", &["missing_values_not_fully_carried"]);
    let find = |n: &str| t.columns.iter().find(|c| c.name == n).unwrap();
    assert_eq!(find("m_one").missing_codes, ["99"]);
    assert_eq!(find("m_three").missing_codes, ["7", "8", "9"]);
    assert_eq!(find("m_range_one").missing_codes, ["0"], "the single value next to a range is kept");
    assert!(find("m_range").missing_codes.is_empty());
    let w = t.warnings.iter().find(|w| w.location.as_deref() == Some("m_range_one")).unwrap();
    assert!(w.message.contains("m_range_one") && w.message.contains("90 through 99"), "{}", w.message);
    let w = t.warnings.iter().find(|w| w.location.as_deref() == Some("m_low_range")).unwrap();
    assert!(w.message.contains("LOWEST through 0"), "{}", w.message);
    // Cells keep the raw value, even a value the file calls missing.
    assert_eq!(t.rows[2][0], Cell::Number(99.0));
    assert_eq!(t.rows[3][0], Cell::Empty, "system-missing is an empty cell");
}

#[test]
fn dates_match_r_and_times_stay_seconds() {
    let t = check("dates_none.sav", &["time_values_kept_as_seconds"]);
    let find = |n: &str| t.columns.iter().position(|c| c.name == n).unwrap();
    assert_eq!(t.rows[0][find("d_date")], Cell::Date("2024-03-05".into()));
    assert_eq!(t.rows[0][find("dt")], Cell::Date("2024-03-05T13:45:10".into()));
    assert_eq!(t.columns[find("dt")].level, Some(Level::Date));
    assert_eq!(t.columns[find("tm")].level, Some(Level::Ratio), "a time of day is not a date");
    assert_eq!(t.rows[0][find("tm")], Cell::Number(3661.0));
    assert_eq!(t.rows[0][find("d_month")], Cell::Number(13_928_976_000.0), "MONTH is a code, not a date");
}

#[test]
fn non_ascii_text_matches_r() {
    let t = check("unicode_byte.sav", &[]);
    let latin = &t.rows[0][0];
    assert_eq!(*latin, Cell::Text("Caf\u{e9} Zo\u{eb}".into()));
    assert_eq!(t.columns[7].name, "\u{540d}\u{524d}", "a name in Japanese");
    assert_eq!(t.columns[8].name.len(), 63, "a long name is kept whole");
    // Combining marks are left as the file has them; the importer normalises later.
    assert_eq!(t.rows[0][4], Cell::Text("e\u{301} (decomposed)".into()));
}

#[test]
fn long_texts_match_r_in_every_compression() {
    let none = check("longstrings_none.sav", &[]);
    let byte = check("longstrings_byte.sav", &[]);
    let zlib = check("longstrings_zsav.zsav", &[]);
    assert_eq!(none, byte);
    assert_eq!(none, zlib);
    // Text of exactly the widths where the segments change.
    for (col, len) in [("s255", 255usize), ("s256", 256), ("s300", 300), ("s504", 504), ("s505", 505), ("s1000", 1000)] {
        let i = none.columns.iter().position(|c| c.name == col).unwrap();
        match &none.rows[0][i] {
            Cell::Text(s) => assert_eq!(s.len(), len, "{col}"),
            other => panic!("{col}: {other:?}"),
        }
    }
    let i = none.columns.iter().position(|c| c.name == "s505").unwrap();
    match &none.rows[4][i] {
        Cell::Text(s) => assert!(s.ends_with("12") && s.len() == 505),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_wide_file_matches_r() {
    let t = check("wide_byte.sav", &[]);
    assert_eq!(t.columns.len(), 130);
    // Labels sit on columns past the hundredth, whose slot numbers differ from their positions.
    let last = &t.columns[129];
    assert_eq!(last.name, "v130");
    assert_eq!(last.label.as_deref(), Some("Variable 130"));
    assert!(t.columns.iter().filter(|c| !c.value_labels.is_empty()).count() >= 20);
    // v121 is the first labelled column past the hundredth (121 = 11 * 11).
    assert_eq!(t.columns[120].missing_codes, ["9"]);
}

#[test]
fn files_with_no_cases_read_as_empty_tables() {
    let a = check("empty_none.sav", &["no_cases"]);
    let b = check("empty_byte.sav", &["no_cases"]);
    assert_eq!(a, b);
    assert_eq!(a.columns.len(), 4);
    assert!(a.rows.is_empty());
    assert_eq!(a.columns[1].value_labels.len(), 2, "labels are still read");
}

#[test]
fn the_same_data_reads_the_same_in_every_compression() {
    let none = read("basic_none.sav");
    assert_eq!(none, read("basic_byte.sav"));
    assert_eq!(none, read("basic_zsav.zsav"));
}

#[test]
fn the_survey_fixture_matches_r() {
    let t = check("survey_byte.sav", &[]);
    assert_eq!(t.columns[0].name, "respondent_id");
}

#[test]
fn every_expected_file_has_a_test() {
    let covered: BTreeSet<&str> = [
        "basic_none", "basic_byte", "basic_zsav", "missing_byte", "dates_none", "unicode_byte", "longstrings_none",
        "longstrings_byte", "longstrings_zsav", "wide_byte", "empty_none", "empty_byte", "survey_byte",
    ]
    .into_iter()
    .collect();
    let mut found: BTreeSet<String> = BTreeSet::new();
    for entry in fs::read_dir(dir()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if let Some(stem) = name.strip_suffix(".expected.json") {
            found.insert(stem.to_string());
        }
    }
    let found_refs: BTreeSet<&str> = found.iter().map(String::as_str).collect();
    assert_eq!(found_refs, covered);
}

#[test]
fn every_fixture_is_small() {
    for entry in fs::read_dir(dir()).unwrap() {
        let entry = entry.unwrap();
        let size = entry.metadata().unwrap().len();
        assert!(size < 50 * 1024, "{:?} is {size} bytes", entry.file_name());
    }
}

// ---- damaged copies of the real files -----------------------------------------------------------------------

const FILES: [&str; 13] = [
    "basic_none.sav", "basic_byte.sav", "basic_zsav.zsav", "missing_byte.sav", "dates_none.sav", "unicode_byte.sav",
    "longstrings_none.sav", "longstrings_byte.sav", "longstrings_zsav.zsav", "wide_byte.sav", "empty_none.sav",
    "empty_byte.sav", "survey_byte.sav",
];

#[test]
fn a_file_cut_short_is_an_error_or_reads_exactly_as_before() {
    let started = Instant::now();
    for file in FILES {
        let bytes = bytes_of(file);
        let whole = sav::read_table(&bytes).unwrap();
        // Every length near the start (the dictionary), then a stride that hits every alignment.
        let lengths = (0..bytes.len().min(3000)).chain((3000..bytes.len()).step_by(13));
        for len in lengths {
            match sav::read_table(&bytes[..len]) {
                Err(e) => assert!(!e.to_string().is_empty()),
                Ok(t) => assert_eq!(t, whole, "{file} cut at {len} gave a different table instead of an error"),
            }
        }
    }
    assert!(started.elapsed() < Duration::from_secs(120));
}

#[test]
fn a_file_cut_inside_its_dictionary_is_always_an_error() {
    for file in FILES {
        let bytes = bytes_of(file);
        // Every real fixture has its data after byte 1500; none of these cuts can be valid.
        for len in (0..1500.min(bytes.len())).step_by(3) {
            assert!(sav::read_table(&bytes[..len]).is_err(), "{file} cut at {len}");
        }
    }
}

/// A small deterministic random-number generator, so a failure can be repeated.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn flipping_bytes_never_panics_or_hangs() {
    let started = Instant::now();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for file in FILES {
        let bytes = bytes_of(file);
        for _ in 0..1200 {
            let mut copy = bytes.clone();
            for _ in 0..=(rng.next() % 4) {
                // Most changes land in the dictionary and header, where the lengths and counts are.
                let limit = if rng.next() % 10 < 7 { copy.len().min(3000) } else { copy.len() };
                let at = (rng.next() as usize) % limit;
                match rng.next() % 3 {
                    0 => copy[at] = rng.next() as u8,
                    1 => copy[at] ^= 1 << (rng.next() % 8),
                    _ => copy[at] = [0u8, 0xFF, 0x7F, 0x80][(rng.next() % 4) as usize],
                }
            }
            let _ = sav::read_table(&copy);
        }
    }
    assert!(started.elapsed() < Duration::from_secs(300), "took {:?}", started.elapsed());
}

#[test]
fn files_that_are_not_sav_are_refused_in_plain_words() {
    for junk in [&b""[..], b"hello", b"PK\x03\x04 not a sav", &[0u8; 400][..], &[0xFFu8; 400][..]] {
        let e = sav::read_table(junk).unwrap_err().to_string();
        assert!(e.contains("SPSS") || e.contains("ends too soon"), "{e}");
    }
    let e = open_bytes("x.sav", b"PK\x03\x04", &ImportOptions::default()).unwrap_err().to_string();
    assert!(e.contains("name says"), "{e}");
}

// ---- the whole way into a project -------------------------------------------------------------------------------

#[test]
fn a_survey_file_goes_all_the_way_into_a_project() {
    let bytes = bytes_of("survey_byte.sav");
    let Opened::Table(table) = open_bytes("x.sav", &bytes, &ImportOptions::default()).unwrap() else {
        panic!("a .sav file should open as a table");
    };
    assert_eq!(table.name, "x", "the table is named after the file");

    let mapping = SurveyMapping::from_guess(&table);
    assert_eq!(mapping.id_column.as_deref(), Some("respondent_id"));
    assert_eq!(mapping.text_columns, ["why"]);
    assert!(mapping.attribute_columns.iter().any(|a| a.name == "age"));

    let batch = import_table(&table, &mapping, &ImportOptions::default()).unwrap();
    assert_eq!(batch.cases.len(), 12);
    assert_eq!(batch.sources.len(), 1);

    let dir = TempDir::new().unwrap();
    let mut open = OpenOptions::new("tester");
    open.backup_on_open = false;
    let mut project = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &open).unwrap();
    let out = project.apply_import(&batch).unwrap();
    assert_eq!(out.case_ids.len(), 12);
    assert_eq!(out.units_created, 12);

    // What SPSS said about each variable arrives in the project's attributes.
    let conn = rusqlite::Connection::open(project.path()).unwrap();
    let attr = |name: &str| -> (String, String, String) {
        conn.query_row(
            "SELECT level, value_labels_json, missing_codes_json FROM attribute WHERE applies_to = 'case' AND name = ?1",
            [name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    };
    let (level, labels, missing) = attr("age");
    assert_eq!((level.as_str(), missing.as_str()), ("ratio", r#"["-99"]"#));
    assert_eq!(labels, "{}");
    let (_, labels, _) = attr("group");
    assert_eq!(labels, r#"{"1":"Treatment","2":"Control"}"#);
    let (level, labels, _) = attr("satisfaction");
    assert_eq!(level, "ordinal");
    let labels: BTreeMap<String, String> = serde_json::from_str(&labels).unwrap();
    assert_eq!(labels.len(), 5);
    assert_eq!(labels["5"], "Very high");

    let value = |case: &str, attribute: &str| -> Option<String> {
        conn.query_row(
            "SELECT v.value FROM attribute_value v
               JOIN attribute a ON a.id = v.attribute_id AND a.applies_to = 'case'
               JOIN research_case c ON c.id = v.target_id
              WHERE a.name = ?1 AND c.name = ?2",
            [attribute, case],
            |r| r.get(0),
        )
        .ok()
    };
    assert_eq!(value("1001", "age").as_deref(), Some("34"));
    assert_eq!(value("1003", "age").as_deref(), Some("-99"), "the raw missing code is kept, flagged by the attribute");
    assert_eq!(value("1002", "group").as_deref(), Some("2"));
    let text: String = conn.query_row("SELECT text FROM source", [], |r| r.get(0)).unwrap();
    assert!(text.contains("I felt welcome from the first day"), "{text}");
    let units: i64 = conn.query_row("SELECT COUNT(*) FROM unit", [], |r| r.get(0)).unwrap();
    assert_eq!(units, 12);
}
