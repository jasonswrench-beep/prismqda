//! `prismqda`: a command-line harness for the engine (specification section 3.2).
//!
//! It exists so the engine can be exercised and tested before any interface is built. It is
//! also a plain way to script a project.

mod import_cmd;
#[cfg(feature = "dev-server")]
mod serve;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use prismqda_engine::{
    schema, sha256_hex, CoderRole, CodingMode, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions,
    Origin, Project, SourceType, PROJECT_EXTENSION,
};

#[derive(Parser)]
#[command(name = "prismqda", version, about = "Command-line harness for the PrismQDA engine")]
struct Cli {
    /// Who is working; recorded on every event. Defaults to `cli`, and never to the account name of the computer.
    #[arg(long, global = true, env = "PRISMQDA_ACTOR")]
    actor: Option<String>,

    /// Make a dated backup copy each time a project opens (the desktop app does this; the harness does not by default).
    #[arg(long, global = true)]
    backup_on_open: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new, empty project file.
    New {
        file: PathBuf,
        #[arg(long)]
        name: String,
        /// Code whole predefined units instead of free spans (content analysis).
        #[arg(long)]
        unit_mode: bool,
        #[arg(long, default_value = "free")]
        method: String,
    },
    /// Show a project's settings and counts.
    Info { file: PathBuf },
    /// Add a plain-text file as a source.
    ImportText {
        file: PathBuf,
        text_file: PathBuf,
        /// Name for the source; defaults to the file name.
        #[arg(long)]
        name: Option<String>,
        /// document, pdf, survey, transcript, social, or other.
        #[arg(long = "type", default_value = "document")]
        source_type: SourceType,
        /// Import even if a file with the same contents was already imported.
        #[arg(long)]
        force: bool,
    },
    /// Bring a file into the project: documents, spreadsheets, transcripts, captions, and more.
    Import {
        file: PathBuf,
        /// The file to import.
        source: PathBuf,
        #[command(flatten)]
        args: import_cmd::ImportArgs,
        /// Import even if a file with the same contents was already imported.
        #[arg(long)]
        force: bool,
    },
    /// Bring in codings made elsewhere: a spreadsheet with a unit identifier and a code on each row.
    ImportCodings {
        file: PathBuf,
        /// The coded spreadsheet.
        source: PathBuf,
        #[command(flatten)]
        args: import_cmd::CodingArgs,
    },
    /// Show what importing a file would do, without needing a project.
    Inspect {
        source: PathBuf,
        #[command(flatten)]
        args: import_cmd::ImportArgs,
    },
    /// Add a coder (a person, or a model with --model).
    AddCoder {
        file: PathBuf,
        name: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long = "model-version")]
        model_version: Option<String>,
    },
    /// Add a code to the codebook.
    AddCode {
        file: PathBuf,
        name: String,
        #[arg(long)]
        parent: Option<i64>,
        #[arg(long, default_value = "")]
        definition: String,
        /// A theme groups other codes.
        #[arg(long)]
        theme: bool,
    },
    /// Apply a code to a span of a source (offsets count characters, end exclusive).
    Code {
        file: PathBuf,
        #[arg(long)]
        source: i64,
        #[arg(long)]
        start: i64,
        #[arg(long)]
        end: i64,
        #[arg(long)]
        code: i64,
        #[arg(long)]
        coder: i64,
        #[arg(long, default_value = "")]
        rationale: String,
        #[arg(long)]
        unit: Option<i64>,
    },
    /// Remove a coding (the removal is logged).
    Uncode { file: PathBuf, coding: i64 },
    /// Add a note to the audit trail.
    Annotate { file: PathBuf, note: String },
    /// Show the newest events, oldest first.
    Events {
        file: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Also print what each event changed (before and after, as JSON).
        #[arg(long)]
        details: bool,
    },
    /// Make a dated backup copy now.
    Backup { file: PathBuf },
    /// Start the local development bridge for testing the interface (built with --features dev-server).
    #[cfg(feature = "dev-server")]
    Serve {
        #[arg(long, default_value_t = 4891)]
        port: u16,
        /// A project to open at start.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Let an assistant application that speaks the Model Context Protocol read a project (and, if allowed, propose codings), over a pipe
    /// to this program, with no network connection (decision 203). The assistant's application starts this; nothing here listens.
    Mcp {
        file: PathBuf,
        /// What the assistant may do: read (the default), or propose (suggest codings, which wait in the review queue).
        #[arg(long, default_value = "read")]
        permission: String,
    },
    /// Print the project file schema: schema/v1.sql, then each later step (schema/v2.sql and so on).
    Schema,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    // Never the account name of the computer: what is recorded on every change goes into the project, and into copies that are shared.
    let explicit_actor = cli.actor.as_deref().is_some_and(|n| !n.trim().is_empty());
    let actor = cli.actor.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "cli".to_string());
    let mut opts = OpenOptions::new(actor);
    opts.backup_on_open = cli.backup_on_open;
    // The passphrase of an encrypted project is taken from the environment, not from the command line, which other programs can read.
    opts.passphrase = std::env::var("PRISMQDA_PASSPHRASE").ok().filter(|p| !p.is_empty());
    let open = |file: &Path| -> Result<Project, String> {
        let project = Project::open(file, &opts).map_err(|e| e.to_string())?;
        for notice in project.notices() {
            eprintln!("notice: {notice}");
        }
        Ok(project)
    };
    let fail = |e: prismqda_engine::Error| e.to_string();

    match cli.command {
        Command::New { file, name, unit_mode, method } => {
            if file.extension().and_then(|e| e.to_str()) != Some(PROJECT_EXTENSION) {
                eprintln!("note: project files normally end in .{PROJECT_EXTENSION}");
            }
            let mut new = NewProject::new(name);
            new.method_template = method;
            if unit_mode {
                new.coding_mode = CodingMode::Unit;
            }
            Project::create(&file, &new, &opts).map_err(fail)?;
            println!("created {}", file.display());
        }
        Command::Info { file } => {
            let info = open(&file)?.info().map_err(fail)?;
            println!("project:        {}", info.name);
            println!("file:           {}", info.path.display());
            println!("project id:     {}", info.project_uid);
            println!("method:         {}", info.method_template);
            println!("coding mode:    {}", info.coding_mode);
            println!("format version: {} (this build writes {})", info.schema_version, schema::SCHEMA_VERSION);
            println!("read-only:      {}", if info.read_only { "yes" } else { "no" });
            println!(
                "sources {}  units {}  codes {}  coders {}  codings {}  events {}",
                info.sources, info.units, info.codes, info.coders, info.codings, info.events
            );
        }
        Command::ImportText { file, text_file, name, source_type, force } => {
            let bytes = fs::read(&text_file).map_err(|e| format!("cannot read {}: {e}", text_file.display()))?;
            let text = String::from_utf8(bytes.clone())
                .map_err(|_| format!("{} is not UTF-8 text; re-save it as UTF-8 first", text_file.display()))?;
            let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
            let hash = sha256_hex(&bytes);
            let mut project = open(&file)?;
            if let Some(existing) = project.find_source_by_hash(&hash).map_err(fail)? {
                if !force {
                    return Err(format!(
                        "this file was already imported as source {existing}; use --force to import it again"
                    ));
                }
                eprintln!("warning: importing again; the same file is already source {existing}");
            }
            let name = name.unwrap_or_else(|| {
                text_file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "source".to_string())
            });
            let mut source = NewSource::new(name, source_type, text);
            source.original_file_hash = Some(hash);
            let id = project.add_source(&source).map_err(fail)?;
            println!("source {id}");
        }
        Command::Import { file, source, args, force } => {
            let mut project = open(&file)?;
            for line in import_cmd::run_import(&mut project, &source, &args, force)? {
                println!("{line}");
            }
        }
        Command::ImportCodings { file, source, args } => {
            let mut project = open(&file)?;
            for line in import_cmd::run_import_codings(&mut project, &source, &args)? {
                println!("{line}");
            }
        }
        Command::Inspect { source, args } => {
            for line in import_cmd::run_inspect(&source, &args)? {
                println!("{line}");
            }
        }
        Command::AddCoder { file, name, model, provider, model_version } => {
            let mut coder = NewCoder::human(name);
            if model.is_some() {
                coder.role = CoderRole::Model;
            }
            coder.model_name = model;
            coder.provider = provider;
            coder.model_version = model_version;
            let id = open(&file)?.add_coder(&coder).map_err(fail)?;
            println!("coder {id}");
        }
        Command::AddCode { file, name, parent, definition, theme } => {
            let mut code = NewCode::new(name);
            code.parent_id = parent;
            code.definition = definition;
            code.theme = theme;
            let id = open(&file)?.add_code(&code).map_err(fail)?;
            println!("code {id}");
        }
        Command::Code { file, source, start, end, code, coder, rationale, unit } => {
            let mut project = open(&file)?;
            let mut coding = NewCoding::new(source, start, end, code, coder);
            coding.rationale = rationale;
            coding.unit_id = unit;
            coding.origin = Origin::Human;
            let id = project.apply_coding(&coding).map_err(fail)?;
            let quote = project.text_of_span(source, start, end).map_err(fail)?;
            println!("coding {id}: {quote:?}");
        }
        Command::Uncode { file, coding } => {
            open(&file)?.remove_coding(coding).map_err(fail)?;
            println!("removed coding {coding}");
        }
        Command::Annotate { file, note } => {
            open(&file)?.annotate(&note).map_err(fail)?;
            println!("noted");
        }
        Command::Events { file, limit, details } => {
            for e in open(&file)?.events(limit).map_err(fail)? {
                let target = match (&e.target_type, e.target_id) {
                    (Some(t), Some(id)) => format!("{t} {id}"),
                    _ => String::new(),
                };
                println!("{:>5}  {}  {:<10}  {:<14}  {}", e.id, e.ts, e.actor, e.action, target);
                if details {
                    if let Some(before) = &e.before {
                        println!("       before: {before}");
                    }
                    if let Some(after) = &e.after {
                        println!("       after:  {after}");
                    }
                }
            }
        }
        Command::Backup { file } => {
            let path = open(&file)?.backup_now().map_err(fail)?;
            println!("backup {}", path.display());
        }
        #[cfg(feature = "dev-server")]
        Command::Serve { port, project } => {
            serve::run(port, &opts.actor, project.as_deref())?;
        }
        Command::Mcp { file, permission } => {
            // Suggestions that are accepted later are coded by this person, so a session that proposes must say who that is.
            let mut server = prismqda_api::mcp::Server::open(&file, explicit_actor.then_some(opts.actor.as_str()), opts.passphrase.clone(), &permission)?;
            let stdin = std::io::stdin();
            server.run(stdin.lock(), std::io::stdout().lock()).map_err(|e| e.to_string())?;
        }
        Command::Schema => print!("{}", schema::published_schema()),
    }
    Ok(())
}
