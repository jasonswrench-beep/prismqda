//! The `import` and `inspect` commands: bring a file into a project, or show what would come in.

use std::fs;
use std::path::Path;

use prismqda_engine::{ImportBatch, Level, Project, SourceType, UnitKind};
use prismqda_import::{import_codings, import_table, open_bytes, AttributeColumn, CoderSpec, CodingMapping, ImportOptions, Opened, SurveyMapping, Table};

/// Everything the person can say about an import on the command line.
#[derive(clap::Args)]
pub struct ImportArgs {
    /// Name for the source; defaults to the file name.
    #[arg(long)]
    pub name: Option<String>,
    /// The folder the source is filed under.
    #[arg(long, default_value = "")]
    pub folder: String,
    /// document, pdf, survey, transcript, social, or other.
    #[arg(long = "type")]
    pub source_type: Option<SourceType>,
    /// sentence, paragraph, speaker_turn, survey_response, post, or comment.
    #[arg(long = "unit")]
    pub unit_kind: Option<UnitKind>,
    /// Transcripts: a speaker whose turns are left out of coding (repeat for several).
    #[arg(long = "interviewer")]
    pub interviewers: Vec<String>,
    /// Spreadsheets: the column whose distinct values become cases.
    #[arg(long)]
    pub id_column: Option<String>,
    /// Spreadsheets: the column holding each row's own identifier.
    #[arg(long)]
    pub unit_id_column: Option<String>,
    /// Spreadsheets: a column of open-ended text (repeat for several).
    #[arg(long = "text-column")]
    pub text_columns: Vec<String>,
    /// Spreadsheets: a column describing the respondent, as `name` or `name:level` (repeat for several).
    #[arg(long = "attribute-column")]
    pub attribute_columns: Vec<String>,
    /// Spreadsheets: a column describing each response itself, such as a comment's author (repeat for several).
    #[arg(long = "unit-attribute-column")]
    pub unit_attribute_columns: Vec<String>,
}

fn read(path: &Path) -> Result<(String, Vec<u8>), String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "source".to_string());
    Ok((name, bytes))
}

fn options(args: &ImportArgs, bytes: &[u8]) -> ImportOptions {
    ImportOptions {
        name: args.name.clone(),
        folder: args.folder.clone(),
        source_type: args.source_type,
        unit_kind: args.unit_kind,
        interviewer_labels: args.interviewers.clone(),
        file_hash: Some(prismqda_engine::sha256_hex(bytes)),
        picture_variables: Default::default(),
    }
}

/// "name" or "name:level".
fn parse_attribute(spec: &str) -> Result<AttributeColumn, String> {
    match spec.rsplit_once(':') {
        Some((name, level)) if level.parse::<Level>().is_ok() => {
            Ok(AttributeColumn { name: name.to_string(), level: Some(level.parse().map_err(|e: String| e)?) })
        }
        _ => Ok(AttributeColumn { name: spec.to_string(), level: None }),
    }
}

fn mapping_for(table: &Table, args: &ImportArgs) -> Result<SurveyMapping, String> {
    let explicit = args.id_column.is_some() || args.unit_id_column.is_some() || !args.text_columns.is_empty() || !args.attribute_columns.is_empty() || !args.unit_attribute_columns.is_empty();
    if !explicit {
        return Ok(SurveyMapping::from_guess(table));
    }
    let mut mapping = SurveyMapping {
        id_column: args.id_column.clone(),
        unit_id_column: args.unit_id_column.clone(),
        text_columns: args.text_columns.clone(),
        attribute_columns: Vec::new(),
        unit_attribute_columns: Vec::new(),
    };
    for a in &args.attribute_columns {
        mapping.attribute_columns.push(parse_attribute(a)?);
    }
    for a in &args.unit_attribute_columns {
        mapping.unit_attribute_columns.push(parse_attribute(a)?);
    }
    Ok(mapping)
}

fn describe_batch(batch: &ImportBatch) -> Vec<String> {
    let mut lines = Vec::new();
    for s in &batch.sources {
        lines.push(format!(
            "  source '{}' ({}): {} characters, {} units ({}), {} format runs",
            s.name,
            s.source_type.as_str(),
            s.text.chars().count(),
            s.units.len(),
            s.default_unit_kind.as_str(),
            s.runs.len()
        ));
        for w in &s.warnings {
            lines.push(format!("    warning [{}]: {}", w.kind, w.message));
        }
    }
    if !batch.cases.is_empty() {
        lines.push(format!("  {} cases, {} case attributes", batch.cases.len(), batch.case_attributes.len()));
    }
    lines
}

/// Brings a file into the project. Returns the lines to print.
pub fn run_import(project: &mut Project, file: &Path, args: &ImportArgs, force: bool) -> Result<Vec<String>, String> {
    let (name, bytes) = read(file)?;
    let opts = options(args, &bytes);
    let hash = opts.file_hash.clone().unwrap_or_default();
    if let Some(existing) = project.find_source_by_hash(&hash).map_err(|e| e.to_string())? {
        if !force {
            return Err(format!("this file was already imported as source {existing}; use --force to import it again"));
        }
    }
    let mut out = Vec::new();
    let batch = match open_bytes(&name, &bytes, &opts).map_err(|e| e.to_string())? {
        Opened::Batch(b) => b,
        Opened::Table(table) => {
            let mapping = mapping_for(&table, args)?;
            out.push(format!(
                "table '{}': {} rows; id column {:?}, unit id column {:?}, text columns {:?}, {} attribute columns",
                table.name,
                table.rows.len(),
                mapping.id_column,
                mapping.unit_id_column,
                mapping.text_columns,
                mapping.attribute_columns.len()
            ));
            import_table(&table, &mapping, &opts).map_err(|e| e.to_string())?
        }
    };
    out.extend(describe_batch(&batch));
    let outcome = project.apply_import(&batch).map_err(|e| e.to_string())?;
    out.push(format!(
        "imported: sources {:?}, {} units, {} cases, {} attribute values",
        outcome.source_ids,
        outcome.units_created,
        outcome.case_ids.len(),
        outcome.attribute_values_created
    ));
    Ok(out)
}

/// Shows what an import would do, without a project.
pub fn run_inspect(file: &Path, args: &ImportArgs) -> Result<Vec<String>, String> {
    let (name, bytes) = read(file)?;
    let opts = options(args, &bytes);
    let mut out = Vec::new();
    match open_bytes(&name, &bytes, &opts).map_err(|e| e.to_string())? {
        Opened::Batch(b) => out.extend(describe_batch(&b)),
        Opened::Table(table) => {
            let preview = table.preview(3);
            out.push(format!("table '{}': {} rows, {} columns", preview.name, preview.row_count, preview.columns.len()));
            for w in &table.warnings {
                out.push(format!("  warning [{}]: {}", w.kind, w.message));
            }
            for c in &preview.columns {
                out.push(format!(
                    "  {:<28} {:<10} level {:<8} suggested {:<9} distinct {:<6} empty {:<5} e.g. {:?}",
                    c.name,
                    c.looks_like,
                    c.guessed_level.as_str(),
                    c.suggested_role,
                    c.distinct,
                    c.empty,
                    c.samples
                ));
            }
        }
    }
    Ok(out)
}

/// What the person can say about a coded spreadsheet.
#[derive(clap::Args)]
pub struct CodingArgs {
    /// The column holding each unit's own identifier.
    #[arg(long)]
    pub unit_id_column: String,
    #[arg(long)]
    pub code_column: String,
    #[arg(long)]
    pub coder_column: Option<String>,
    /// The coder when the file has no coder column.
    #[arg(long)]
    pub default_coder: Option<String>,
    #[arg(long)]
    pub rationale_column: Option<String>,
    #[arg(long)]
    pub confidence_column: Option<String>,
    /// A coder that is a model, as `NAME` or `NAME=provider/model` (repeat for several).
    #[arg(long = "model-coder")]
    pub model_coders: Vec<String>,
}

/// Brings existing codings into a project. Returns the lines to print.
pub fn run_import_codings(project: &mut Project, file: &Path, args: &CodingArgs) -> Result<Vec<String>, String> {
    let (name, bytes) = read(file)?;
    let table = match open_bytes(&name, &bytes, &ImportOptions::default()).map_err(|e| e.to_string())? {
        Opened::Table(t) => t,
        Opened::Batch(_) => return Err("codings come from a spreadsheet (CSV, TSV, or XLSX)".to_string()),
    };
    let mut mapping = CodingMapping {
        unit_id_column: args.unit_id_column.clone(),
        code_column: args.code_column.clone(),
        coder_column: args.coder_column.clone(),
        default_coder: args.default_coder.clone(),
        rationale_column: args.rationale_column.clone(),
        confidence_column: args.confidence_column.clone(),
        ..Default::default()
    };
    for spec in &args.model_coders {
        let (coder, detail) = spec.split_once('=').unwrap_or((spec.as_str(), ""));
        let (provider, model) = detail.split_once('/').map(|(p, m)| (Some(p.to_string()), Some(m.to_string()))).unwrap_or((None, if detail.is_empty() { None } else { Some(detail.to_string()) }));
        mapping.coders.insert(coder.to_string(), CoderSpec { model: true, provider, model_name: model, model_version: None });
    }
    let report = import_codings(project, &table, &mapping).map_err(|e| e.to_string())?;
    let mut out = vec![format!(
        "{} codings stored; {} codes and {} coders created; {} rows matched no unit{}; {} rows were empty; {} rows were for excluded units",
        report.codings,
        report.codes_created,
        report.coders_created,
        report.unmatched_rows,
        if report.unmatched_examples.is_empty() { String::new() } else { format!(" (for example {})", report.unmatched_examples.join(", ")) },
        report.empty_rows,
        report.excluded_unit_rows
    )];
    out.push(format!("  from {} rows in '{}'", table.rows.len(), table.name));
    Ok(out)
}
