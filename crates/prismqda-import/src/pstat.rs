//! Opening a `.pstat` file, the project format of PrismStatistics, as a table (PSTAT-1 in the shared core).
//!
//! The table goes through the same window as a spreadsheet or an SPSS file: the person says which variable names the respondent, which
//! hold comments to code, and which are values for the cases. An evaluation or a course survey kept in PrismStatistics can then be
//! coded here, with its ratings as variables of the cases, and the case table that PrismQDA sends back (`export.pstat`) carries those
//! variables and the codes together.
//!
//! This is a reader of the **dataset only**: the variables (name, label, level of measurement, value labels, missing codes) and the
//! columns. PrismStatistics's own records in the file (its operations, results, report) and a profile's extra part are not read, and the
//! file is never written to. It applies the rules of the specification that a reader must follow (Z1, Z3, J1, J2, P1 to P3, D1 to D4,
//! D6, D7, S1; section 8). It is PrismQDA's own small piece until the core has a Rust implementation (`CORE-REQUESTS.md`, R2), and the
//! core's conformance files are run through it by `tools/pstat-check/conformance.mjs`.

use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read};

use prismqda_engine::{ImportWarning, Level};
use serde_json::{Map, Value};

use crate::error::{ImportError, Result};
use crate::table::{Cell, Column, Table};

/// The most `project.json` may hold once unpacked. The specification lets a reader refuse more than 1 GiB; a table of this window has to fit in
/// memory several times over, so the limit here is lower.
const MAX_UNPACKED: u64 = 256 << 20;

/// The most values of one missing range that are written out as separate codes.
const MAX_RANGE_CODES: i64 = 200;

const NOT_A_PROJECT: &str = "this is not a PrismStatistics project file (a .pstat file is a ZIP archive that holds project.json)";

fn damaged(why: impl AsRef<str>) -> ImportError {
    ImportError::format(format!("this PrismStatistics project file is damaged: {}", why.as_ref()))
}

fn is_whole(v: &Value) -> bool {
    v.is_i64() || v.is_u64()
}

/// The program that wrote the file and its release, from `app_version`, for a message.
fn writer(project: &Map<String, Value>) -> String {
    match project.get("app_version").and_then(Value::as_str).map(str::trim) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => "an unknown program".to_string(),
    }
}

/// A name by the specification's rule: it starts with a letter, then letters, digits and underscores, at most 64 characters.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts = chars.next().is_some_and(char::is_alphabetic);
    starts && name.chars().count() <= 64 && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Reads `project.json` out of the archive and parses it, with the rules about the container and the JSON (Z1, Z3, J1).
fn read_project(bytes: &[u8]) -> Result<Map<String, Value>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| ImportError::format(NOT_A_PROJECT))?;
    let entry = zip.by_name("project.json").map_err(|_| ImportError::format(NOT_A_PROJECT))?;
    let mut raw = Vec::new();
    entry.take(MAX_UNPACKED + 1).read_to_end(&mut raw).map_err(|e| damaged(format!("project.json cannot be read ({e})")))?;
    if raw.len() as u64 > MAX_UNPACKED {
        return Err(ImportError::unsupported(format!("this project holds more than {} MB of data once unpacked, which is more than the import window reads", MAX_UNPACKED >> 20)));
    }
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(damaged("project.json begins with a byte-order mark, which the format does not allow"));
    }
    match serde_json::from_slice::<Value>(&raw) {
        Ok(Value::Object(o)) => Ok(o),
        Ok(_) => Err(damaged("project.json is not one JSON object")),
        Err(e) => Err(damaged(format!("project.json is not valid JSON ({e})"))),
    }
}

fn level_of(measure: &str) -> Option<Level> {
    match measure {
        "nominal" => Some(Level::Nominal),
        "ordinal" => Some(Level::Ordinal),
        "scale" => Some(Level::Ratio),
        _ => None,
    }
}

/// A number as a cell and as the text a value label or a missing code is keyed by: whole numbers have no decimal point.
fn number_text(x: f64) -> String {
    Cell::Number(x).text()
}

/// Reads the dataset of a `.pstat` file into a table. `stem` names the table when the dataset has no name.
pub fn read_table(bytes: &[u8], stem: &str) -> Result<Table> {
    let project = read_project(bytes)?;

    // P1, P2: the format and its version.
    if project.get("format").and_then(Value::as_str) != Some("PrismStatistics project") {
        return Err(ImportError::format("this file is a ZIP archive, but it is not a PrismStatistics project (its project.json does not say so)"));
    }
    let version = project.get("version").ok_or_else(|| damaged("it has no version"))?;
    if !is_whole(version) {
        return Err(damaged("the version of the format is not a whole number"));
    }
    let Some(version_number) = version.as_u64() else { return Err(damaged("the version of the format is a negative number")) };
    if version_number > 1 {
        return Err(ImportError::unsupported(format!(
            "this project was saved by {} in a newer project format (version {}) than this version of PrismQDA reads (version 1); open it in that program, or save it again from an older one",
            writer(&project),
            version
        )));
    }
    // P3: the members every writer writes.
    for member in ["operations", "results"] {
        if !project.get(member).is_some_and(Value::is_array) {
            return Err(damaged(format!("it has no {member} list")));
        }
    }
    let dataset = project.get("dataset").and_then(Value::as_object).ok_or_else(|| damaged("it has no dataset"))?;

    // S1: where the data came from, when the file says.
    if let Some(source) = project.get("source").filter(|s| !s.is_null()) {
        let ok = source.get("file_name").is_some_and(Value::is_string)
            && source.get("report").is_some_and(|r| {
                r.get("rows").is_some_and(is_whole) && r.get("variables").is_some_and(is_whole) && r.get("renamed").is_some_and(Value::is_array) && r.get("short_rows").is_some_and(is_whole)
            });
        if !ok {
            return Err(damaged("its record of where the data came from is incomplete"));
        }
    }

    let variables = dataset.get("variables").and_then(Value::as_array).ok_or_else(|| damaged("the dataset has no variables list"))?;
    let columns = dataset.get("columns").and_then(Value::as_array).ok_or_else(|| damaged("the dataset has no columns list"))?;
    if variables.len() != columns.len() {
        return Err(damaged(format!("it has {} variables but {} columns of data", variables.len(), columns.len())));
    }
    if !dataset.get("next_id").is_some_and(is_whole) {
        return Err(damaged("the dataset has no next_id, or it is not a whole number"));
    }
    let name = dataset.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or(stem).to_string();

    let mut warnings: Vec<ImportWarning> = Vec::new();
    let mut ids: HashSet<u64> = HashSet::new();
    let mut names: HashSet<String> = HashSet::new();
    let mut table = Table { name, columns: Vec::with_capacity(variables.len()), rows: Vec::new(), warnings: Vec::new() };
    let mut cases: Option<usize> = None;
    let mut cols: Vec<Vec<Cell>> = Vec::with_capacity(variables.len());
    let mut ranges_skipped = 0usize;

    for (v, c) in variables.iter().zip(columns) {
        let v = v.as_object().ok_or_else(|| damaged("a variable is not an object"))?;
        // D4: ids are distinct whole numbers from 0 to 4294967295.
        let id = v.get("id").filter(|i| is_whole(i)).and_then(Value::as_u64).filter(|i| *i <= u64::from(u32::MAX)).ok_or_else(|| damaged("a variable has no id, or an id that is not a whole number from 0 to 4294967295"))?;
        if !ids.insert(id) {
            return Err(damaged(format!("two variables have the id {id}")));
        }
        // D3: the name rule, and no two names alike but for the case of ASCII letters.
        let vname = v.get("name").and_then(Value::as_str).ok_or_else(|| damaged("a variable has no name"))?;
        if !valid_name(vname) {
            return Err(damaged(format!("the variable name '{vname}' is not allowed (a name starts with a letter and has letters, digits and underscores only, up to 64 characters)")));
        }
        if !names.insert(vname.to_ascii_lowercase()) {
            return Err(damaged(format!("two variables are named '{vname}' (names are the same when only the case of a letter differs)")));
        }
        // D6: label and measure.
        let label = v.get("label").and_then(Value::as_str).ok_or_else(|| damaged(format!("the variable '{vname}' has no label (an empty one is written \"\")")))?;
        let measure = v.get("measure").and_then(Value::as_str).unwrap_or("");
        let level = level_of(measure).ok_or_else(|| damaged(format!("the variable '{vname}' has the level of measurement '{measure}', which is not nominal, ordinal or scale")))?;

        // D2, D1: the column's type and values, and the same number of cases in every column.
        let c = c.as_object().ok_or_else(|| damaged("a column is not an object"))?;
        let kind = c.get("type").and_then(Value::as_str).unwrap_or("");
        let values = c.get("values").and_then(Value::as_array).ok_or_else(|| damaged(format!("the column of '{vname}' has no values list")))?;
        match cases {
            None => cases = Some(values.len()),
            Some(n) if n != values.len() => return Err(damaged(format!("the column of '{vname}' has {} values but the first column has {n}", values.len()))),
            Some(_) => {}
        }
        let numeric = match kind {
            "numeric" => true,
            "text" => false,
            other => return Err(damaged(format!("the column of '{vname}' has the type '{other}', which is not numeric or text"))),
        };
        let mut cells = Vec::with_capacity(values.len());
        for value in values {
            cells.push(match (numeric, value) {
                (_, Value::Null) => Cell::Empty,
                (true, Value::Number(n)) => Cell::Number(n.as_f64().filter(|x| x.is_finite()).ok_or_else(|| damaged(format!("the column of '{vname}' has a number that is not finite")))?),
                (false, Value::String(s)) if s.trim().is_empty() => Cell::Empty,
                (false, Value::String(s)) => Cell::Text(s.clone()),
                _ => return Err(damaged(format!("the {} column of '{vname}' has a value that is not {}", kind, if numeric { "a number" } else { "text" }))),
            });
        }

        // D7: value labels, missing codes and ranges are numbers.
        let mut value_labels: BTreeMap<String, String> = BTreeMap::new();
        for entry in v.get("value_labels").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let value = entry.get("value").and_then(Value::as_f64).ok_or_else(|| damaged(format!("a value label of '{vname}' does not name a number")))?;
            let text = entry.get("label").and_then(Value::as_str).ok_or_else(|| damaged(format!("a value label of '{vname}' has no text")))?;
            value_labels.insert(number_text(value), text.to_string());
        }
        let mut missing_codes: Vec<String> = Vec::new();
        for m in v.get("missing_values").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let x = m.as_f64().ok_or_else(|| damaged(format!("a missing code of '{vname}' is not a number")))?;
            missing_codes.push(number_text(x));
        }
        for range in v.get("missing_ranges").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let pair = range.as_array().filter(|p| p.len() == 2).and_then(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).ok_or_else(|| damaged(format!("a missing range of '{vname}' is not a pair of numbers")))?;
            // A range is written out as its whole numbers when there are few; PrismQDA's missing codes are values, not ranges.
            if pair.0.fract() == 0.0 && pair.1.fract() == 0.0 && pair.1 >= pair.0 && (pair.1 - pair.0) as i64 <= MAX_RANGE_CODES {
                missing_codes.extend((pair.0 as i64..=pair.1 as i64).map(|k| k.to_string()));
            } else {
                ranges_skipped += 1;
            }
        }
        missing_codes.sort();
        missing_codes.dedup();

        table.columns.push(Column {
            name: vname.to_string(),
            label: Some(label.trim().to_string()).filter(|l| !l.is_empty()),
            // Words have no level the file can know; their level is guessed from the values, as for a spreadsheet.
            level: if numeric { Some(level) } else { None },
            value_labels,
            missing_codes,
        });
        cols.push(cells);
    }

    let n = cases.unwrap_or(0);
    table.rows = (0..n).map(|r| cols.iter().map(|c| c[r].clone()).collect()).collect();

    if let Some(profile) = project.get("profile").and_then(Value::as_str).filter(|p| !p.is_empty()) {
        warnings.push(ImportWarning::new("pstat_profile", format!("This file follows the profile \"{profile}\". Only its data was read; the part the profile adds is not read.")));
    }
    if project.get("operations").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) || project.get("results").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
        warnings.push(ImportWarning::new("pstat_analyses", "The analyses and results saved in this file by PrismStatistics were not read; only the data was."));
    }
    if ranges_skipped > 0 {
        warnings.push(ImportWarning::new(
            "pstat_missing_range",
            format!("{ranges_skipped} range{} of missing values could not be written as separate codes, so {} not carried: those values are kept as they are.", if ranges_skipped == 1 { "" } else { "s" }, if ranges_skipped == 1 { "it was" } else { "they were" }),
        ));
    }
    table.warnings = warnings;
    Ok(table)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;

    use super::*;

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn file(project: &Value) -> Vec<u8> {
        archive(&[("project.json", project.to_string().as_bytes())])
    }

    /// An evaluation: who, a 1 to 5 rating of the course with value labels and a missing code, and an open comment.
    fn evaluation() -> Value {
        json!({
            "format": "PrismStatistics project", "version": 1, "app_version": "0.1.1",
            "dataset": {
                "name": "Spring workshop",
                "variables": [
                    { "id": 1, "name": "participant", "label": "Participant", "measure": "nominal", "value_labels": [], "missing_values": [] },
                    { "id": 2, "name": "rating", "label": "Overall rating", "measure": "ordinal",
                      "value_labels": [{ "value": 1, "label": "Poor" }, { "value": 5, "label": "Excellent" }], "missing_values": [9], "missing_ranges": [[98, 99]] },
                    { "id": 3, "name": "hours", "label": "", "measure": "scale", "value_labels": [], "missing_values": [] },
                    { "id": 4, "name": "comment", "label": "What worked well?", "measure": "nominal", "value_labels": [], "missing_values": [] }
                ],
                "columns": [
                    { "type": "text", "values": ["P01", "P02", "P03"] },
                    { "type": "numeric", "values": [5, 2, 9] },
                    { "type": "numeric", "values": [3.5, null, 4] },
                    { "type": "text", "values": ["The role plays helped me practise.", null, "   "] }
                ],
                "next_id": 5
            },
            "source": null, "operations": [], "results": []
        })
    }

    #[test]
    fn the_variables_their_labels_levels_and_codes_and_the_cases_come_through() {
        let t = read_table(&file(&evaluation()), "x").unwrap();
        assert_eq!(t.name, "Spring workshop");
        assert_eq!(t.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["participant", "rating", "hours", "comment"]);
        assert_eq!(t.columns[1].label.as_deref(), Some("Overall rating"));
        assert_eq!(t.columns[2].label, None, "an empty label is none");
        assert_eq!(t.columns[1].level, Some(Level::Ordinal));
        assert_eq!(t.columns[2].level, Some(Level::Ratio), "scale is a number that can be averaged");
        assert_eq!(t.columns[0].level, None, "words get a level guessed from the values");
        assert_eq!(t.columns[1].value_labels.get("5").map(String::as_str), Some("Excellent"));
        assert_eq!(t.columns[1].missing_codes, ["9", "98", "99"], "a missing range of whole numbers is written out as codes");
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.rows[0][1], Cell::Number(5.0));
        assert_eq!(t.rows[1][2], Cell::Empty);
        assert_eq!(t.rows[0][3], Cell::Text("The role plays helped me practise.".into()));
        assert_eq!(t.rows[1][3], Cell::Empty, "null is empty");
        assert_eq!(t.rows[2][3], Cell::Empty, "a blank answer is empty");
        // The wizard guesses the respondent and the comment, and the rest are variables of the case.
        let preview = t.preview(3);
        let roles: Vec<&str> = preview.columns.iter().map(|c| c.suggested_role.as_str()).collect();
        assert_eq!(roles[0], "id", "{roles:?}");
        assert_eq!(roles[1], "attribute");
    }

    #[test]
    fn what_is_left_behind_is_said() {
        let mut p = evaluation();
        p["profile"] = json!("OpenAssess item results 1");
        p["results"] = json!([{ "id": 1 }]);
        p["dataset"]["variables"][1]["missing_ranges"] = json!([[0.5, 7.5]]);
        let t = read_table(&file(&p), "x").unwrap();
        let kinds: Vec<&str> = t.warnings.iter().map(|w| w.kind.as_str()).collect();
        assert_eq!(kinds, ["pstat_profile", "pstat_analyses", "pstat_missing_range"], "{:?}", t.warnings);
        assert_eq!(t.columns[1].missing_codes, ["9"], "the range that cannot be written as codes is not guessed at");
    }

    #[test]
    fn a_file_that_is_not_a_project_is_refused_in_words() {
        let e = read_table(b"not a zip", "x").unwrap_err().to_string();
        assert!(e.contains("not a PrismStatistics project file"), "{e}");
        let e = read_table(&archive(&[("Project.json", b"{}")]), "x").unwrap_err().to_string();
        assert!(e.contains("not a PrismStatistics project file"), "the entry's name is matched with case: {e}");
        let mut p = evaluation();
        p["format"] = json!("Something else");
        assert!(read_table(&file(&p), "x").unwrap_err().to_string().contains("not a PrismStatistics project"));
    }

    #[test]
    fn a_newer_format_names_the_program_that_made_it() {
        let mut p = evaluation();
        p["version"] = json!(2);
        p["app_version"] = json!("OpenAssess 0.2.0");
        let e = read_table(&file(&p), "x").unwrap_err().to_string();
        assert!(e.contains("OpenAssess 0.2.0") && e.contains("newer project format"), "{e}");
    }

    #[test]
    fn a_damaged_dataset_is_refused_before_anything_uses_it() {
        type Break = (&'static str, Box<dyn Fn(&mut Value)>);
        let breaks: Vec<Break> = vec![
            ("columns of different lengths", Box::new(|p| p["dataset"]["columns"][1]["values"].as_array_mut().unwrap().push(json!(1)))),
            ("two variables, one column", Box::new(|p| {
                p["dataset"]["columns"].as_array_mut().unwrap().pop();
            })),
            ("names alike but for case", Box::new(|p| p["dataset"]["variables"][1]["name"] = json!("PARTICIPANT"))),
            ("a name starting with a digit", Box::new(|p| p["dataset"]["variables"][1]["name"] = json!("1st"))),
            ("two variables, one id", Box::new(|p| p["dataset"]["variables"][1]["id"] = json!(1))),
            ("text in a numeric column", Box::new(|p| p["dataset"]["columns"][1]["values"][0] = json!("5"))),
            ("an unknown measure", Box::new(|p| p["dataset"]["variables"][0]["measure"] = json!("interval"))),
            ("a value label whose value is text", Box::new(|p| p["dataset"]["variables"][1]["value_labels"][0]["value"] = json!("1"))),
            ("no next_id", Box::new(|p| {
                p["dataset"].as_object_mut().unwrap().remove("next_id");
            })),
            ("no results list", Box::new(|p| {
                p.as_object_mut().unwrap().remove("results");
            })),
            ("a source report without short_rows", Box::new(|p| p["source"] = json!({ "file_name": "a.csv", "report": { "rows": 3, "variables": 4, "renamed": [] } }))),
        ];
        for (what, change) in breaks {
            let mut p = evaluation();
            change(&mut p);
            let e = read_table(&file(&p), "x");
            assert!(e.is_err(), "{what} must be refused");
            assert!(e.unwrap_err().to_string().contains("damaged"), "{what} is said to be damage");
        }
    }

    #[test]
    fn numbers_written_with_a_fraction_where_a_whole_number_belongs_are_refused() {
        let text = evaluation().to_string().replacen("\"next_id\":5", "\"next_id\":5.0", 1);
        assert!(read_table(&archive(&[("project.json", text.as_bytes())]), "x").is_err());
        let text = evaluation().to_string().replacen("\"id\":1,", "\"id\":1.0,", 1);
        assert!(read_table(&archive(&[("project.json", text.as_bytes())]), "x").is_err());
    }

    #[test]
    fn a_byte_order_mark_and_text_that_is_not_json_are_refused() {
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(evaluation().to_string().as_bytes());
        assert!(read_table(&archive(&[("project.json", &with_bom)]), "x").unwrap_err().to_string().contains("byte-order mark"));
        assert!(read_table(&archive(&[("project.json", b"{ format: 1 }")]), "x").unwrap_err().to_string().contains("not valid JSON"));
    }

    #[test]
    fn unknown_members_other_entries_and_a_stored_entry_are_fine() {
        let mut p = evaluation();
        p["x_future"] = json!({ "a": 1 });
        p["dataset"]["variables"][0]["id"] = json!(0);
        p["dataset"]["next_id"] = json!(1);
        let bytes = archive(&[("project.json", p.to_string().as_bytes()), ("notes.txt", b"later parts")]);
        assert!(read_table(&bytes, "x").is_ok());
    }
}
