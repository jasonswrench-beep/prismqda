//! The data editor's later tools through the command layer: sorting the cases, tables with a row for each group, tables stood on end or
//! laid across, and variables merged in from a file by a key. A table may come from the project's cases or from a file the person names
//! (a spreadsheet, a text file of values, or an SPSS file); the engine works on tables of text and this layer reads the files.

use std::fs;
use std::path::PathBuf;

use prismqda_engine::{long_to_wide, wide_to_long, DataTable, GroupTable, LongToWide, MergeColumn, MergeSpec, SortKey, WideToLong, MAX_TABLE_ROWS};
use prismqda_import::{open_bytes, Cell, ImportOptions, Opened, Table};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{export_target, parse, to_json, write_file, Api, ApiError, ApiResult};

/// The most rows of a table shown in the window; the rest are in the file that is saved.
const SHOWN_ROWS: usize = 50;

#[derive(Deserialize)]
struct SortArgs {
    #[serde(default)]
    keys: Vec<SortKey>,
}

/// Where the rows of a restructured table come from.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum From {
    /// The variables of the project's cases, one row for each case (the first column is `Case`, the case's name).
    Cases,
    /// A spreadsheet, text file of values, or SPSS file.
    File { path: String },
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum TableSpec {
    /// A row for each group of cases, with summaries.
    Groups(GroupTable),
    /// The cases (or a file's rows) stood on end: a row for each measure of each row.
    StandOnEnd {
        from: From,
        #[serde(flatten)]
        spec: WideToLong,
    },
    /// Observations laid across: a row for each who, a column for each observation.
    LayAcross {
        from: From,
        #[serde(flatten)]
        spec: LongToWide,
    },
}

#[derive(Deserialize)]
struct TableArgs {
    spec: TableSpec,
}

#[derive(Deserialize)]
struct SaveTableArgs {
    spec: TableSpec,
    path: String,
    /// `csv` or `xlsx`.
    format: String,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
struct SplitArgs {
    variable: String,
}

#[derive(Deserialize)]
struct FilePath {
    path: String,
}

#[derive(Deserialize)]
struct MergeArgs {
    path: String,
    /// The column of the file that holds the key.
    key_column: String,
    /// What the keys are matched with: a variable of the cases, or the case's own name (the default).
    #[serde(default)]
    match_on: Option<String>,
    columns: Vec<MergeFileColumn>,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
struct MergeFileColumn {
    /// The column of the file.
    name: String,
    /// The variable to make from it (the column's name when left out).
    #[serde(default, rename = "as")]
    make: Option<String>,
    /// The level of measurement of the variable; the guess from the values when left out.
    #[serde(default)]
    level: Option<String>,
}

/// A file read as a table, or the reason it is not one.
pub(crate) fn read_table(path: &str) -> Result<Table, ApiError> {
    let p = PathBuf::from(path.trim());
    if !p.is_absolute() {
        return Err(ApiError::new("invalid", "type the full path of the file, for example C:\\Data\\wave2.xlsx"));
    }
    let bytes = fs::read(&p).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", p.display())))?;
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".to_string());
    match open_bytes(&name, &bytes, &ImportOptions::default())? {
        Opened::Table(t) => Ok(t),
        Opened::Batch(_) => Err(ApiError::new("invalid", format!("{name} is a document, not a table of values. Choose a spreadsheet (.xlsx), a text file of values (.csv or .tsv), or an SPSS file (.sav)."))),
    }
}

fn to_data_table(t: &Table) -> DataTable {
    DataTable {
        columns: t.columns.iter().map(|c| c.name.clone()).collect(),
        rows: t.rows.iter().map(|r| (0..t.columns.len()).map(|c| r.get(c).filter(|cell| !cell.is_empty()).map(Cell::text)).collect()).collect(),
    }
}

impl Api {
    fn source_table(&mut self, from: &From) -> Result<DataTable, ApiError> {
        match from {
            From::Cases => Ok(self.project()?.case_table()?),
            From::File { path } => Ok(to_data_table(&read_table(path)?)),
        }
    }

    /// Works out the table a spec asks for: the table, and how many cases a group table had to leave out (those with no value in a variable
    /// that makes the groups).
    fn make_table(&mut self, spec: &TableSpec) -> Result<(DataTable, Option<usize>), ApiError> {
        match spec {
            TableSpec::Groups(g) => {
                let (table, left_out) = self.project()?.group_table(g)?;
                Ok((table, Some(left_out)))
            }
            TableSpec::StandOnEnd { from, spec } => {
                let source = self.source_table(from)?;
                Ok((wide_to_long(&source, spec)?, None))
            }
            TableSpec::LayAcross { from, spec } => {
                let source = self.source_table(from)?;
                Ok((long_to_wide(&source, spec)?, None))
            }
        }
    }

    /// The commands of the data editor's later tools; `None` for a command that is not one of them.
    pub(crate) fn call_datatools(&mut self, cmd: &str, args: Value) -> Option<ApiResult> {
        Some(match cmd {
            "data.sort" => (|| {
                let a: SortArgs = parse(cmd, args)?;
                let ids = self.project()?.sorted_case_ids(&a.keys)?;
                Ok(json!({ "case_ids": ids }))
            })(),
            "data.table" => (|| {
                let a: TableArgs = parse(cmd, args)?;
                let (table, left_out) = self.make_table(&a.spec)?;
                let total = table.rows.len();
                Ok(json!({ "columns": table.columns, "rows": table.rows.iter().take(SHOWN_ROWS).collect::<Vec<_>>(), "total_rows": total, "shown_rows": total.min(SHOWN_ROWS), "left_out": left_out }))
            })(),
            "data.table_save" => (|| {
                let a: SaveTableArgs = parse(cmd, args)?;
                let format = match a.format.as_str() {
                    "csv" => prismqda_export::Format::Csv,
                    "xlsx" => prismqda_export::Format::Xlsx,
                    other => return Err(ApiError::new("invalid", format!("a table is saved as csv or xlsx, not {other}"))),
                };
                let (table, _) = self.make_table(&a.spec)?;
                let _scrub = self.scrub_scope()?;
                let bytes = prismqda_export::export_table("Table", &table.columns, &table.rows, format)?;
                let target = export_target(&a.path, format.extension(), a.overwrite)?;
                write_file(&target, &bytes)?;
                Ok(json!({ "path": target.display().to_string(), "rows": table.rows.len(), "columns": table.columns.len() }))
            })(),
            "stats.split_groups" => (|| {
                let a: SplitArgs = parse(cmd, args)?;
                to_json(&self.project()?.split_plan(&a.variable)?)
            })(),
            "data.inspect_file" => (|| {
                let a: FilePath = parse(cmd, args)?;
                let table = read_table(&a.path)?;
                let columns: Vec<Value> = (0..table.columns.len())
                    .map(|c| {
                        let mut seen: Vec<String> = Vec::new();
                        let mut distinct = std::collections::BTreeSet::new();
                        for row in &table.rows {
                            if let Some(cell) = row.get(c).filter(|cell| !cell.is_empty()) {
                                let text = cell.text();
                                if seen.len() < 3 && !seen.contains(&text) {
                                    seen.push(text.clone());
                                }
                                distinct.insert(text);
                            }
                        }
                        json!({ "name": table.columns[c].name, "level": table.level_of(c).as_str(), "examples": seen, "distinct": distinct.len() })
                    })
                    .collect();
                Ok(json!({ "columns": columns, "rows": table.rows.len(), "file_name": PathBuf::from(&a.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() }))
            })(),
            "data.merge" => (|| {
                let a: MergeArgs = parse(cmd, args)?;
                let table = read_table(&a.path)?;
                if table.rows.len() > MAX_TABLE_ROWS {
                    return Err(ApiError::new("invalid", format!("the file has more than {MAX_TABLE_ROWS} rows")));
                }
                let key = table.column_index(&a.key_column).ok_or_else(|| ApiError::new("invalid", format!("the file has no column named '{}'", a.key_column)))?;
                let text_of = |row: &Vec<Cell>, c: usize| row.get(c).filter(|cell| !cell.is_empty()).map(Cell::text);
                let keys: Vec<Option<String>> = table.rows.iter().map(|r| text_of(r, key)).collect();
                let mut columns = Vec::with_capacity(a.columns.len());
                for c in &a.columns {
                    let i = table.column_index(&c.name).ok_or_else(|| ApiError::new("invalid", format!("the file has no column named '{}'", c.name)))?;
                    if i == key {
                        return Err(ApiError::new("invalid", format!("'{}' is the key, so it cannot also be brought in as a variable", c.name)));
                    }
                    let level = c.level.clone().unwrap_or_else(|| table.level_of(i).as_str().to_string());
                    columns.push(MergeColumn { name: c.make.clone().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| c.name.clone()), level, values: table.rows.iter().map(|r| text_of(r, i)).collect() });
                }
                let spec = MergeSpec { keys, match_on: a.match_on, columns, overwrite: a.overwrite };
                to_json(&self.project()?.merge_variables(&spec)?)
            })(),
            _ => return None,
        })
    }
}
