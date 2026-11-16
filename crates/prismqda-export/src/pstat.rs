//! The case table as a `.pstat` file, the project format of PrismStatistics (PSTAT-1, section 7.3, the profile `PrismQDA case table 1`).
//!
//! The format and the profile are specified in the shared core (`github.com/jasonswrench-beep/openstage-core`, `specs/pstat/PSTAT-1.md`)
//! and checked there with `checkPstat` from `@openstage/pstat`; this module writes what that specification says and nothing of its own.
//! A `.pstat` file is a zip archive of two JSON entries: `project.json` (the data) and `case-table.json` (the profile's part, which
//! says which variable is which case attribute and which code, by variable id).
//!
//! One row for each case, never one for each coding. Never written: excerpt text, document text, memos, coders' names. While the list
//! of names to replace is applied to exports, it is applied to every text written here, as it is to the other exports.

use std::io::Write;

use prismqda_engine::{day_text, parse_day, parse_number, CaseTable, CaseTableAttribute, Project};
use serde::Serialize;
use serde_json::Value;

use crate::{scrub, zip_package, ExportError};

/// The name of the profile, as PSTAT-1 section 7.3 spells it.
pub const PROFILE: &str = "PrismQDA case table 1";

/// What the person chose.
#[derive(Debug, Clone)]
pub struct PstatOptions {
    /// Leave the cases' names out (the file then holds only each case's number).
    pub leave_out_names: bool,
    /// An excerpt coded with a sub-code also counts for every code above it.
    pub include_descendants: bool,
    /// `PrismQDA` and the release, as the spec says: `PrismQDA 0.1.0-beta.2`. The release is the program's, which this crate does not know.
    pub app_version: String,
}

/// What was written, for the window to say.
#[derive(Debug, Clone, Serialize)]
pub struct PstatReport {
    pub cases: usize,
    /// Variables in the file: the case number, the name (unless left out), the attributes, and two for each code.
    pub variables: usize,
    pub attributes: usize,
    pub codes: usize,
    /// Attributes whose variable name had to be made from the attribute's name: `[the attribute's name, the name used]`.
    pub renamed: Vec<[String; 2]>,
    /// Plain sentences about what was left out or changed on the way, also written in the file.
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------- JSON in a fixed order

/// A JSON object whose members keep the order they were put in: the profile's part must start with `part` and `version`.
#[derive(Default)]
struct Obj(Vec<(&'static str, String)>);

impl Obj {
    fn put<T: Serialize>(mut self, key: &'static str, value: T) -> Self {
        self.0.push((key, serde_json::to_string(&value).expect("a plain value always serializes")));
        self
    }

    fn raw(mut self, key: &'static str, json: String) -> Self {
        self.0.push((key, json));
        self
    }

    fn render(&self) -> String {
        let members: Vec<String> = self.0.iter().map(|(k, v)| format!("{}:{v}", serde_json::to_string(k).expect("a key serializes"))).collect();
        format!("{{{}}}", members.join(","))
    }
}

fn array(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
}

/// A number the way a person would write it: a whole number without a fraction.
fn number(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 9e15 {
        Value::from(x as i64)
    } else {
        serde_json::Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null)
    }
}

fn scrubbed(s: &str) -> String {
    scrub::text(s).into_owned()
}

// ---------------------------------------------------------------- variable names (PSTAT-1 section 7.3, item 3)

/// The variable name made from an attribute's name: every run of characters other than ASCII letters, digits and `_` becomes one `_`,
/// leading and trailing `_` go, an empty result is `a`, one that does not start with a letter gets `a_` in front, and it is cut to 64.
pub fn attribute_name(name: &str) -> String {
    let mut s = String::new();
    let mut in_run = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            s.push(c);
            in_run = false;
        } else if !in_run {
            s.push('_');
            in_run = true;
        }
    }
    let s = s.trim_matches('_').to_string();
    let s = if s.is_empty() {
        "a".to_string()
    } else if !s.starts_with(|c: char| c.is_ascii_alphabetic()) {
        format!("a_{s}")
    } else {
        s
    };
    s.chars().take(64).collect()
}

/// True for a name of the form `code_<digits>_present` or `code_<digits>_count`, in any case: those belong to the code variables.
fn is_code_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("code_") else { return false };
    let Some((digits, tail)) = rest.split_once('_') else { return false };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && (tail == "present" || tail == "count")
}

/// The variable names of the attributes, in order: made from the names, then made distinct from each other and from `taken`.
pub fn attribute_names(names: &[String], taken: &[&str]) -> Vec<String> {
    let mut used: Vec<String> = taken.iter().map(|s| s.to_ascii_lowercase()).collect();
    names
        .iter()
        .map(|name| {
            let base = attribute_name(name);
            let mut out = base.clone();
            let mut k = 2;
            while used.contains(&out.to_ascii_lowercase()) || is_code_name(&out) {
                let tail = format!("_{k}");
                let keep = 64usize.saturating_sub(tail.len());
                out = format!("{}{tail}", base.chars().take(keep).collect::<String>());
                k += 1;
            }
            used.push(out.to_ascii_lowercase());
            out
        })
        .collect()
}

// ---------------------------------------------------------------- one attribute as a column

struct Column {
    /// `numeric` or `text`.
    kind: &'static str,
    values: Vec<Value>,
    value_labels: Vec<(f64, String)>,
    missing_values: Vec<f64>,
}

/// True for a value such as `007` or `-012`: a number with a zero in front of another digit, which an identifier uses and a number does not.
fn has_leading_zero(v: &str) -> bool {
    let digits = v.trim().trim_start_matches(['-', '+']);
    let mut chars = digits.chars();
    chars.next() == Some('0') && chars.next().is_some_and(|c| c.is_ascii_digit())
}

fn measure_of(level: &str) -> &'static str {
    match level {
        "ordinal" => "ordinal",
        "interval" | "ratio" => "scale",
        _ => "nominal",
    }
}

/// The column an attribute makes, with a plain sentence for anything it could not carry over as it was.
fn column_of(a: &CaseTableAttribute, label: &str, notes: &mut Vec<String>) -> Column {
    let date = a.level == "date";
    let text_level = a.level == "text";
    if date {
        let mut unread = 0usize;
        let values = a
            .values
            .iter()
            .map(|v| match v {
                None => Value::Null,
                Some(v) => match parse_day(v) {
                    Some(day) => Value::from(day_text(day)),
                    None => {
                        unread += 1;
                        Value::Null
                    }
                },
            })
            .collect();
        if unread > 0 {
            notes.push(format!(
                "{unread} value{} of \"{label}\" {} not written year-month-day, so {} left empty (a date such as 3/4/2024 cannot be told from the 3rd of April).",
                if unread == 1 { "" } else { "s" },
                if unread == 1 { "was" } else { "were" },
                if unread == 1 { "it was" } else { "they were" }
            ));
        }
        return Column { kind: "text", values, value_labels: Vec::new(), missing_values: Vec::new() };
    }
    let numbers: Vec<Option<f64>> = a.values.iter().map(|v| v.as_deref().map(parse_number).unwrap_or(None)).collect();
    // A column is numbers only when every value can be written as a number without changing it: a code such as 007 is text, or it would meet 7.
    let all_numbers = !text_level && a.values.iter().zip(&numbers).all(|(v, n)| v.is_none() || (n.is_some() && !has_leading_zero(v.as_deref().unwrap_or(""))));
    if all_numbers {
        let mut labels: Vec<(f64, String)> = Vec::new();
        let mut skipped = 0usize;
        for (key, text) in &a.value_labels {
            match parse_number(key) {
                Some(x) if !labels.iter().any(|(y, _)| *y == x) => labels.push((x, scrubbed(text))),
                Some(_) => {}
                None => skipped += 1,
            }
        }
        labels.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap_or(std::cmp::Ordering::Equal));
        if skipped > 0 {
            notes.push(format!("{skipped} value label{} of \"{label}\" did not belong to a number and {} left out.", if skipped == 1 { "" } else { "s" }, if skipped == 1 { "was" } else { "were" }));
        }
        let mut missing: Vec<f64> = a.missing_codes.iter().filter_map(|m| parse_number(m)).collect();
        missing.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
        missing.dedup();
        return Column { kind: "numeric", values: numbers.iter().map(|n| n.map_or(Value::Null, number)).collect(), value_labels: labels, missing_values: missing };
    }
    // A text column. A number-like attribute that holds words ("refused") is one, and its words stay as they are.
    if !text_level {
        notes.push(format!("\"{label}\" holds values that cannot be written as numbers without changing them (words, or numbers with a zero in front), so it is written as text and its value labels and missing codes stay in PrismQDA."));
    }
    Column { kind: "text", values: a.values.iter().map(|v| v.as_ref().map_or(Value::Null, |v| Value::from(scrubbed(v)))).collect(), value_labels: Vec::new(), missing_values: Vec::new() }
}

// ---------------------------------------------------------------- the file

struct Variable {
    id: i64,
    name: String,
    label: String,
    measure: &'static str,
    value_labels: Vec<(f64, String)>,
    missing_values: Vec<f64>,
}

impl Variable {
    fn render(&self) -> String {
        let labels = array(self.value_labels.iter().map(|(v, l)| Obj::default().put("value", number(*v)).put("label", l).render()));
        let missing = array(self.missing_values.iter().map(|m| serde_json::to_string(&number(*m)).expect("a number serializes")));
        Obj::default()
            .put("id", self.id)
            .put("name", &self.name)
            .put("label", &self.label)
            .put("measure", self.measure)
            .raw("value_labels", labels)
            .raw("missing_values", missing)
            .render()
    }
}

fn column_json(kind: &str, values: &[Value]) -> String {
    Obj::default().put("type", kind).raw("values", array(values.iter().map(|v| serde_json::to_string(v).expect("a value serializes")))).render()
}

/// Makes the case table of the open project as a `.pstat` file's bytes, and says what it holds.
pub fn export_case_table(project: &Project, options: &PstatOptions) -> Result<(Vec<u8>, PstatReport), ExportError> {
    let table: CaseTable = project.statistics_case_table(options.include_descendants)?;
    let mut notes: Vec<String> = Vec::new();
    let mut renamed: Vec<[String; 2]> = Vec::new();

    let rows = table.case_ids.len();
    let mut variables: Vec<Variable> = Vec::new();
    let mut columns: Vec<String> = Vec::new();
    let mut next = 1i64;
    let mut take_id = || {
        let id = next;
        next += 1;
        id
    };

    // The case's number, and its name unless the person left names out.
    let case_id_variable = take_id();
    variables.push(Variable { id: case_id_variable, name: "case_id".into(), label: "Case ID".into(), measure: "nominal", value_labels: Vec::new(), missing_values: Vec::new() });
    columns.push(column_json("numeric", &table.case_ids.iter().map(|id| Value::from(*id)).collect::<Vec<_>>()));
    let case_name_variable = if options.leave_out_names {
        notes.push("The cases' names were left out; each case is known by its number.".to_string());
        None
    } else {
        let id = take_id();
        variables.push(Variable { id, name: "case_name".into(), label: "Case name".into(), measure: "nominal", value_labels: Vec::new(), missing_values: Vec::new() });
        columns.push(column_json("text", &table.case_names.iter().map(|n| Value::from(scrubbed(n))).collect::<Vec<_>>()));
        Some(id)
    };

    // The attributes, by name.
    let labels: Vec<String> = table.attributes.iter().map(|a| scrubbed(&a.name)).collect();
    let taken: &[&str] = if options.leave_out_names { &["case_id"] } else { &["case_id", "case_name"] };
    let names = attribute_names(&labels, taken);
    let mut attribute_entries: Vec<String> = Vec::new();
    for ((a, label), name) in table.attributes.iter().zip(&labels).zip(&names) {
        if name != label {
            renamed.push([label.clone(), name.clone()]);
        }
        let column = column_of(a, label, &mut notes);
        let id = take_id();
        variables.push(Variable {
            id,
            name: name.clone(),
            label: label.clone(),
            measure: measure_of(&a.level),
            value_labels: column.value_labels.clone(),
            missing_values: column.missing_values.clone(),
        });
        columns.push(column_json(column.kind, &column.values));
        attribute_entries.push(Obj::default().put("variable", id).put("attribute_id", a.id).put("name", label).put("level", &a.level).render());
    }

    // The codes: two variables each.
    let mut code_entries: Vec<String> = Vec::new();
    for code in &table.codes {
        let path: Vec<String> = code.path.iter().map(|p| scrubbed(p)).collect();
        let label = path.join(" > ");
        let present_id = take_id();
        variables.push(Variable {
            id: present_id,
            name: format!("code_{}_present", code.id),
            label: label.clone(),
            measure: "nominal",
            value_labels: vec![(0.0, "No".into()), (1.0, "Yes".into())],
            missing_values: Vec::new(),
        });
        columns.push(column_json("numeric", &code.excerpts.iter().map(|n| n.map_or(Value::Null, |n| Value::from(i64::from(n > 0)))).collect::<Vec<_>>()));
        let count_id = take_id();
        variables.push(Variable { id: count_id, name: format!("code_{}_count", code.id), label: format!("{label} (excerpts)"), measure: "scale", value_labels: Vec::new(), missing_values: Vec::new() });
        columns.push(column_json("numeric", &code.excerpts.iter().map(|n| n.map_or(Value::Null, Value::from)).collect::<Vec<_>>()));
        code_entries.push(
            Obj::default()
                .put("code_id", code.id)
                .put("path", &path)
                .put("kind", &code.kind)
                .put("present_variable", present_id)
                .put("count_variable", count_id)
                .render(),
        );
    }

    // What the person should know about what is in the file.
    if table.codes_left_out > 0 {
        notes.push(format!(
            "{} retired or merged code{} {} left out; only active codes have variables.",
            table.codes_left_out,
            if table.codes_left_out == 1 { "" } else { "s" },
            if table.codes_left_out == 1 { "was" } else { "were" }
        ));
    }
    notes.push(
        if table.sub_codes_counted {
            "An excerpt coded with a sub-code counts for every code above it."
        } else {
            "An excerpt counts only for the code it is coded with, not for the codes above it."
        }
        .to_string(),
    );
    if table.codes.iter().any(|c| c.excerpts.iter().any(Option::is_none)) {
        notes.push("A case that holds no document has no value for any code.".to_string());
    }
    if scrub::active() {
        notes.push("The list of names to replace was applied to the names and the text in this file.".to_string());
    }
    notes.push("No excerpt, document text, memo or coder's name is written.".to_string());

    let project_name = scrubbed(&table.project_name);
    let variable_count = variables.len();
    let dataset = Obj::default()
        .put("name", &project_name)
        .raw("variables", array(variables.iter().map(Variable::render)))
        .raw("columns", array(columns))
        .put("next_id", next)
        .render();
    let report = Obj::default()
        .put("rows", rows)
        .put("variables", variable_count)
        .put("renamed", &renamed)
        .put("short_rows", 0)
        .put("notes", &notes)
        .render();
    let source = Obj::default().put("file_name", format!("{project_name} case table")).raw("report", report).render();
    let project_json = Obj::default()
        .put("format", "PrismStatistics project")
        .put("version", 1)
        .put("app_version", &options.app_version)
        .raw("dataset", dataset)
        .raw("source", source)
        .raw("operations", "[]".into())
        .raw("results", "[]".into())
        .put("profile", PROFILE)
        .render();
    let part = Obj::default()
        .put("part", "PrismQDA case table")
        .put("version", 1)
        .raw("project", Obj::default().put("name", &project_name).put("id", &table.project_uid).render())
        .put("exported", &table.exported)
        .put("codebook_version", table.codebook_version)
        .put("sub_codes_counted", table.sub_codes_counted)
        .put("case_id_variable", case_id_variable)
        .put("case_name_variable", case_name_variable)
        .raw("attributes", array(attribute_entries))
        .raw("codes", array(code_entries))
        .render();

    let bytes = zip_package(|zip, file_options| {
        for (name, body) in [("project.json", &project_json), ("case-table.json", &part)] {
            zip.start_file(name, file_options).map_err(|e| ExportError::Io(e.to_string()))?;
            zip.write_all(body.as_bytes()).map_err(|e| ExportError::Io(e.to_string()))?;
        }
        Ok(())
    })?;
    let report = PstatReport { cases: rows, variables: variable_count, attributes: table.attributes.len(), codes: table.codes.len(), renamed, notes };
    Ok((bytes, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_made_from_the_attribute_the_way_the_spec_says() {
        assert_eq!(attribute_name("Age group"), "Age_group");
        assert_eq!(attribute_name("  Years (in post)  "), "Years_in_post");
        assert_eq!(attribute_name("2nd visit"), "a_2nd_visit");
        assert_eq!(attribute_name("***"), "a");
        assert_eq!(attribute_name("Caf\u{e9} price"), "Caf_price");
        assert_eq!(attribute_name(&"x".repeat(80)).len(), 64);
        assert_eq!(attribute_name("_hidden_"), "hidden");
    }

    #[test]
    fn names_are_made_distinct_and_never_look_like_a_code_variable() {
        let names: Vec<String> = ["Age", "age", "Case ID", "code_12_present", "Case name"].iter().map(|s| s.to_string()).collect();
        assert_eq!(attribute_names(&names, &["case_id", "case_name"]), ["Age", "age_2", "Case_ID_2", "code_12_present_2", "Case_name_2"]);
        // With the names left out, `case_name` is free for an attribute.
        let names: Vec<String> = ["case name"].iter().map(|s| s.to_string()).collect();
        assert_eq!(attribute_names(&names, &["case_id"]), ["case_name"]);
        // A long name is cut so that the whole stays within 64 characters.
        let long = "y".repeat(70);
        let made = attribute_names(&[long.clone(), long], &[]);
        assert_eq!(made[0].len(), 64);
        assert_eq!(made[1].len(), 64);
        assert!(made[1].ends_with("_2"));
    }

    #[test]
    fn objects_keep_the_order_they_were_written_in() {
        let o = Obj::default().put("part", "x").put("version", 1).put("a", 2).render();
        assert_eq!(o, r#"{"part":"x","version":1,"a":2}"#);
    }

    #[test]
    fn whole_numbers_are_written_without_a_fraction() {
        assert_eq!(serde_json::to_string(&number(3.0)).unwrap(), "3");
        assert_eq!(serde_json::to_string(&number(2.5)).unwrap(), "2.5");
        assert_eq!(serde_json::to_string(&number(-0.0)).unwrap(), "0");
    }
}
