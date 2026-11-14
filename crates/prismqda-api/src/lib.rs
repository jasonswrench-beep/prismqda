//! The command layer between PrismQDA's interface and its engine.
//!
//! The interface calls one function: [`Api::call`], with a command name and JSON arguments, and gets
//! JSON back or an [`ApiError`]. The desktop app and the development bridge both go through it, so
//! the screens are tested against exactly what ships. Nothing here decides anything the engine does
//! not already enforce; it only translates.

mod autocode;
mod class;
mod datatools;
mod deid;
mod files;
mod media;
mod samples;
mod text_files;

pub use media::{parse_pdf_page_path, MediaReply};

use std::fs;
use std::path::PathBuf;

use prismqda_engine::{
    check_backup_dir, sha256_hex, DEFAULT_KEEP_BACKUPS, AgreementRequest, CodeEdit, CoderRole, CodingMode, CrosstabSpec, Error as EngineError, ExploreScope, ImportBatch, MatrixSpec, MemoScope, NewCode, NewCoder, NewCoding, NewMemo, NewProject,
    NewRegion, OpenOptions, Origin, Project, QuerySpec, RelationsSpec, SearchFilter, SegmentFilter, Shape, SourceType, Stratify, UnitKind,
};
use prismqda_import::social::SocialMapping;
use prismqda_import::{import_table, open_bytes, ImportError, ImportOptions, Opened, SurveyMapping, Table};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Why a command failed. `code` is stable and meant for the interface to branch on; `message` is for people.
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_string(), message: message.into() }
    }
}

impl From<EngineError> for ApiError {
    fn from(e: EngineError) -> Self {
        let code = match &e {
            EngineError::Invalid(_) => "invalid",
            EngineError::NotFound { .. } => "not_found",
            EngineError::ExclusiveGroup(_) => "exclusive_group",
            EngineError::ReadOnly(_) => "read_only",
            EngineError::NotAProject(_) => "not_a_project",
            EngineError::AlreadyExists(_) => "already_exists",
            EngineError::Io(_) => "io",
            EngineError::Database(_) => "database",
            EngineError::Json(_) => "json",
            EngineError::NeedsPassphrase(_) => "needs_passphrase",
            EngineError::WrongPassphrase => "wrong_passphrase",
        };
        ApiError::new(code, e.to_string())
    }
}

impl From<prismqda_export::ExportError> for ApiError {
    fn from(e: prismqda_export::ExportError) -> Self {
        match e {
            prismqda_export::ExportError::Invalid(m) => ApiError::new("invalid", m),
            prismqda_export::ExportError::TooLarge(m) => ApiError::new("too_large", m),
            prismqda_export::ExportError::Io(m) => ApiError::new("io", m),
            prismqda_export::ExportError::Engine(e) => e.into(),
        }
    }
}

impl From<ImportError> for ApiError {
    fn from(e: ImportError) -> Self {
        let code = match &e {
            ImportError::Io(_) => "io",
            ImportError::Format(_) => "bad_file",
            ImportError::Unsupported(_) => "unsupported_file",
            ImportError::Engine(_) => "invalid",
        };
        ApiError::new(code, e.to_string())
    }
}

type ApiResult = Result<Value, ApiError>;

fn parse<T: DeserializeOwned>(cmd: &str, args: Value) -> Result<T, ApiError> {
    serde_json::from_value(args).map_err(|e| ApiError::new("bad_args", format!("{cmd}: the arguments are not right: {e}")))
}

fn to_json<T: Serialize>(v: &T) -> ApiResult {
    serde_json::to_value(v).map_err(|e| ApiError::new("json", e.to_string()))
}

#[derive(Deserialize)]
struct Id {
    id: i64,
}

#[derive(Deserialize)]
struct SourceId {
    source_id: i64,
}

#[derive(Deserialize)]
struct OpenArgs {
    path: String,
    /// Make a dated backup as the project opens (the default).
    #[serde(default = "yes")]
    backup: bool,
    /// The passphrase of an encrypted project. Without it, opening one is refused with `needs_passphrase`; a wrong one is `wrong_passphrase`.
    #[serde(default)]
    passphrase: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct CreateArgs {
    path: String,
    name: String,
    #[serde(default)]
    coding_mode: Option<CodingMode>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    research_questions: Option<String>,
    /// Encrypts the new project with this passphrase (at least eight characters).
    #[serde(default)]
    passphrase: Option<String>,
}

#[derive(Deserialize)]
struct VersionArgs {
    version: i64,
}

#[derive(Deserialize)]
struct KeepArgs {
    keep: usize,
}

#[derive(Deserialize)]
struct HistoryNameArgs {
    name: String,
}

#[derive(Deserialize)]
struct ScopeArgs {
    #[serde(default)]
    scope: ExploreScope,
}

#[derive(Deserialize)]
struct ProfileArgs {
    code_id: i64,
    #[serde(default = "yes")]
    include_descendants: bool,
    #[serde(default)]
    scope: ExploreScope,
}

#[derive(Deserialize)]
struct MatrixEvidenceArgs {
    spec: MatrixSpec,
    /// A row of the matrix by its `key`; left out for a whole column.
    #[serde(default)]
    row_key: Option<String>,
    /// A column's code; left out for a whole row.
    #[serde(default)]
    code_id: Option<i64>,
}

#[derive(Deserialize)]
struct CrosstabCasesArgs {
    spec: CrosstabSpec,
    /// A row of the table by its `key` (an empty key is the cases with no value); left out for a whole column.
    #[serde(default)]
    row_key: Option<String>,
    /// A column by its `key`; left out for a whole row.
    #[serde(default)]
    column_key: Option<String>,
}

#[derive(Deserialize)]
struct MapCreate {
    name: String,
}

#[derive(Deserialize)]
struct MapUpdate {
    id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
struct MapNodeAdd {
    map_id: i64,
    /// `code`, `case`, `memo`, or `note`.
    kind: String,
    /// Which code, case, or memo (not for a note).
    #[serde(default)]
    ref_id: Option<i64>,
    /// The words of a note.
    #[serde(default)]
    label: String,
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
}

#[derive(Deserialize)]
struct MapNodeMove {
    id: i64,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
struct MapNodeNote {
    id: i64,
    label: String,
}

#[derive(Deserialize)]
struct MapLinkAdd {
    map_id: i64,
    from: i64,
    to: i64,
    #[serde(default)]
    label: String,
    #[serde(default = "yes")]
    directed: bool,
}

#[derive(Deserialize)]
struct MapLinkUpdate {
    id: i64,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    directed: Option<bool>,
}

#[derive(Deserialize)]
struct MapLayoutArgs {
    map_id: i64,
    /// `circle`, `grid`, or `layers`.
    how: String,
}

#[derive(Deserialize)]
struct MapAddCodes {
    map_id: i64,
    code_ids: Vec<i64>,
    /// Join a parent to its children, when both are on the map.
    #[serde(default)]
    hierarchy: bool,
    /// Join codes that overlap at least this many times.
    #[serde(default)]
    min_together: Option<usize>,
}

#[derive(Deserialize)]
struct NotebookRunArgs {
    request: Value,
    #[serde(default)]
    section: String,
}

#[derive(Deserialize)]
struct NotebookNoteArgs {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    section: String,
}

#[derive(Deserialize)]
struct NotebookUpdateArgs {
    id: i64,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    section: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
struct NotebookMoveArgs {
    id: i64,
    up: bool,
}

#[derive(Deserialize)]
struct SavedQuerySave {
    name: String,
    #[serde(default)]
    note: String,
    /// The query as the interface builds it, kept so it can be opened again.
    #[serde(default)]
    draft: Value,
    /// The query as the engine runs it.
    spec: Value,
}

#[derive(Deserialize)]
struct SavedQueryUpdate {
    id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    note: Option<String>,
    /// To change what the query asks, give `draft` and `spec` together.
    #[serde(default)]
    draft: Option<Value>,
    #[serde(default)]
    spec: Option<Value>,
}

#[derive(Deserialize)]
struct RelationsEvidenceArgs {
    spec: RelationsSpec,
    a: i64,
    b: i64,
}

#[derive(Deserialize)]
struct BackupDirArgs {
    /// The folder backups go to; null or empty for beside the project.
    #[serde(default)]
    dir: Option<String>,
    /// Refuse a folder that is missing, is a file, or cannot be written in (the default). The interface sends `false` once
    /// when the program starts, to hand over a folder it saved earlier without refusing it: a backup then falls back to
    /// beside the project and says why.
    #[serde(default = "yes")]
    check: bool,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct TextWrite {
    path: String,
    text: String,
    #[serde(default)]
    replace: bool,
}

#[derive(Deserialize)]
struct BundledSampleArgs {
    id: String,
    path: String,
}

/// Everything in a folder that PrismQDA can read, imported one file at a time.
#[derive(Deserialize)]
struct FolderImport {
    path: String,
    /// Look inside subfolders too (default true). Each subfolder becomes a folder of sources.
    #[serde(default = "yes")]
    recursive: bool,
    /// A folder name to file every source under; subfolders go beneath it.
    #[serde(default)]
    folder: String,
    #[serde(default)]
    source_type: Option<SourceType>,
    #[serde(default)]
    unit_kind: Option<UnitKind>,
    #[serde(default)]
    interviewers: Vec<String>,
    /// Spreadsheets and SPSS files need a person to say which columns are which, so they are skipped unless this is true,
    /// in which case the wizard's guess is used for each.
    #[serde(default)]
    include_tables: bool,
    #[serde(default)]
    force: bool,
    /// Pictures: which of each photograph's own facts (when, camera, place) to keep as variables of a case made for it.
    #[serde(default)]
    picture_variables: prismqda_import::PictureVariables,
}

#[derive(Deserialize, Default)]
struct ImportRequest {
    path: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    source_type: Option<SourceType>,
    #[serde(default)]
    unit_kind: Option<UnitKind>,
    #[serde(default)]
    interviewers: Vec<String>,
    /// For a spreadsheet: how its columns map to cases, text, and attributes. Absent means: use the wizard's guess.
    #[serde(default)]
    mapping: Option<SurveyMapping>,
    /// For a spreadsheet of posts or comments: a social media mapping instead of a survey one.
    #[serde(default)]
    social_mapping: Option<SocialMapping>,
    /// Import even if a file with the same contents was already imported.
    #[serde(default)]
    force: bool,
    /// Pictures: which of the photograph's own facts (when, camera, place) to keep as variables of a case made for it.
    #[serde(default)]
    picture_variables: prismqda_import::PictureVariables,
}

/// A coded area of a picture to add: the shape is given by its kind (`shape`: rectangle, ellipse, or polygon) and its numbers.
#[derive(Deserialize)]
struct RegionAdd {
    source_id: i64,
    /// The page of a PDF the area is on; a picture has one page, the first.
    #[serde(default = "first_page")]
    page: i64,
    #[serde(flatten)]
    shape: Shape,
    code_id: i64,
    #[serde(default)]
    coder_id: Option<i64>,
    #[serde(default)]
    rationale: String,
}

fn first_page() -> i64 {
    1
}

#[derive(Deserialize)]
struct RegionReshape {
    id: i64,
    #[serde(flatten)]
    shape: Shape,
}

#[derive(Deserialize)]
struct ImageCopy {
    source_id: i64,
    path: String,
}

/// The file of a PDF to keep with a document that was imported without it.
#[derive(Deserialize)]
struct PdfAttach {
    source_id: i64,
    path: String,
}

#[derive(Deserialize)]
struct MediaLink {
    source_id: i64,
    /// The full path of the recording on this computer.
    path: String,
}

/// A file to take as the new version of a document that is already in the project.
#[derive(Deserialize)]
struct ReviseRequest {
    /// The document the file revises.
    source_id: i64,
    /// Which file, and how it is read (the same choices as an import).
    #[serde(flatten)]
    file: ImportRequest,
}

#[derive(Deserialize)]
struct CodeAdd {
    name: String,
    #[serde(default)]
    parent_id: Option<i64>,
    #[serde(default)]
    theme: bool,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    definition: String,
    #[serde(default)]
    inclusion_criteria: String,
    #[serde(default)]
    exclusion_criteria: String,
    #[serde(default)]
    exclusive_group: String,
    #[serde(default)]
    rated: bool,
    #[serde(default)]
    rating_label: String,
    #[serde(default)]
    rating_low: String,
    #[serde(default)]
    rating_high: String,
}

#[derive(Deserialize)]
struct AiAnswer {
    run_id: i64,
    /// What the researcher says the model was; kept with the run.
    model: String,
    /// The text the model gave, pasted as it came.
    answer: String,
}

#[derive(Deserialize)]
struct AiMode {
    /// `packet` or `off`.
    mode: String,
}

#[derive(Deserialize)]
struct DoNotShare {
    id: i64,
    do_not_share: bool,
}

#[derive(Deserialize)]
struct CodeUpdate {
    id: i64,
    #[serde(flatten)]
    edit: CodeEdit,
}

#[derive(Deserialize)]
struct CodeMove {
    id: i64,
    #[serde(default)]
    parent_id: Option<i64>,
    #[serde(default)]
    position: Option<usize>,
}

#[derive(Deserialize)]
struct CodeMerge {
    from: i64,
    into: i64,
}

#[derive(Deserialize)]
struct CodeSplit {
    from: i64,
    name: String,
    coding_ids: Vec<i64>,
    #[serde(default)]
    definition: String,
}

#[derive(Deserialize)]
struct CodingApply {
    source_id: i64,
    start: i64,
    end: i64,
    code_id: i64,
    /// Defaults to the person using the project.
    #[serde(default)]
    coder_id: Option<i64>,
    #[serde(default)]
    rationale: String,
    #[serde(default)]
    unit_id: Option<i64>,
    /// Take away the codings that stand in the way because their codes are in the same mutually exclusive
    /// group, and apply this one in the same step. Without it such a coding is refused (`exclusive_group`).
    #[serde(default)]
    replace_exclusive: bool,
}

#[derive(Deserialize)]
struct CodingChange {
    id: i64,
    code_id: i64,
}

#[derive(Deserialize)]
struct CodingRationale {
    id: i64,
    rationale: String,
}

#[derive(Deserialize)]
struct CodingAnchor {
    id: i64,
    anchor: bool,
}

#[derive(Deserialize)]
struct CodingCounterexample {
    /// The code the passage is a counterexample of.
    code_id: i64,
    /// The coding (made with another code).
    id: i64,
    on: bool,
}

#[derive(Deserialize)]
struct GroupName {
    group: String,
}

#[derive(Deserialize)]
struct CodingRating {
    id: i64,
    /// A whole number from 1 to 5, or null to clear the rating. Required: leaving it out is a mistake in
    /// the caller, and must not quietly clear a rating, so it is read as a raw value rather than an `Option`.
    rating: Value,
}

#[derive(Deserialize)]
struct CaseAdd {
    name: String,
    #[serde(default)]
    notes: String,
}

#[derive(Deserialize)]
struct CaseMembership {
    case_id: i64,
    source_id: i64,
}

#[derive(Deserialize)]
struct MemoFilter {
    #[serde(default)]
    scope: Option<MemoScope>,
    #[serde(default)]
    target_id: Option<i64>,
}

#[derive(Deserialize)]
struct MemoUpdate {
    id: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    #[serde(flatten)]
    filter: SearchFilter,
}

#[derive(Deserialize)]
struct ProposalList {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    batch: Option<String>,
    #[serde(default = "default_proposal_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_proposal_limit() -> usize {
    500
}

#[derive(Deserialize)]
struct ProposalDecide {
    /// The suggestions to decide, or...
    #[serde(default)]
    ids: Vec<i64>,
    /// ...every suggestion of this batch that still waits.
    #[serde(default)]
    batch: Option<String>,
    accept: bool,
}

#[derive(Deserialize)]
struct ProposalAdjust {
    id: i64,
    start: i64,
    end: i64,
    code_id: i64,
}

#[derive(Deserialize)]
struct CoderAdd {
    name: String,
    #[serde(default = "human_role")]
    role: CoderRole,
    #[serde(default)]
    local_id: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    model_version: Option<String>,
}

fn human_role() -> CoderRole {
    CoderRole::Human
}

#[derive(Deserialize)]
struct MergeArgs {
    /// The copy of the project to bring in.
    path: String,
    /// The passphrase of the copy, when it is encrypted. Without one, this project's own passphrase is tried.
    #[serde(default)]
    passphrase: Option<String>,
    /// The differences the person settled in favour of the copy.
    #[serde(flatten)]
    choices: prismqda_engine::MergeChoices,
}

#[derive(Deserialize)]
struct PassphraseArgs {
    passphrase: String,
}

#[derive(Deserialize)]
struct ChangePassphraseArgs {
    old: String,
    new: String,
}

#[derive(Deserialize)]
struct DefineVariableArgs {
    #[serde(flatten)]
    spec: prismqda_engine::VariableSpec,
    /// The name the variable has now, when it is being changed and not made.
    #[serde(default)]
    existing: Option<String>,
}

#[derive(Deserialize)]
struct VariableName {
    name: String,
}

#[derive(Deserialize)]
struct DesignKind {
    kind: String,
}

#[derive(Deserialize)]
struct RowId {
    id: i64,
}

#[derive(Deserialize)]
struct RowMove {
    id: i64,
    up: bool,
}

#[derive(Deserialize)]
struct TriangulationUpdate {
    id: i64,
    #[serde(flatten)]
    row: prismqda_engine::TriangulationInput,
}

#[derive(Deserialize)]
struct IntegrationUpdate {
    id: i64,
    #[serde(flatten)]
    entry: prismqda_engine::IntegrationInput,
}

#[derive(Deserialize)]
struct SetValuesArgs {
    variable: String,
    changes: Vec<prismqda_engine::CellChange>,
}

#[derive(Deserialize)]
struct RuleArgs {
    formula: String,
}

#[derive(Deserialize)]
struct QueueArgs {
    coder_ids: Vec<i64>,
    /// Hide who coded what: the coders are called Coder A, Coder B, and so on.
    #[serde(default)]
    blind: bool,
}

#[derive(Deserialize)]
struct DecideArgs {
    unit_id: i64,
    /// The codes the unit should carry. None at all says that no code applies.
    #[serde(default)]
    code_ids: Vec<i64>,
    reason: String,
    /// Who decides; the person using the project when absent.
    #[serde(default)]
    coder_id: Option<i64>,
}

/// The output notebook of the Statistics page as a Word, Excel, or web page file. The interface sends the words and the charts.
#[derive(Deserialize)]
struct NotebookArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    /// `docx`, `xlsx`, or `html`.
    format: String,
    notebook: prismqda_export::Notebook,
}

/// The values of variables for every case, for a script that reads them.
#[derive(Deserialize)]
struct TableArgs {
    keys: Vec<String>,
    /// A code variable also counts the code's sub-codes (the default), as in a run.
    #[serde(default = "table_descendants")]
    include_descendants: bool,
}

fn table_descendants() -> bool {
    true
}

/// A recorded script and the data it reads: two files side by side, `name.R` and `name-data.csv`.
#[derive(Deserialize)]
struct ScriptArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    /// The script, with `{{data-file}}` where the name of the data file goes.
    script: String,
    /// The data, as CSV text.
    data: String,
}

/// One coded stretch of a recording saved as a WAV file (the data is its base 64).
#[derive(Deserialize)]
struct ClipFileArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    /// The WAV file, as base 64.
    data: String,
}

/// One chart saved as a picture: a PNG (the data is its base 64) or an SVG (the data is its text).
#[derive(Deserialize)]
struct PictureArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    /// `png` or `svg`.
    format: String,
    data: String,
}

/// A report: a Word document with the sections asked for, made from the project's records.
#[derive(Deserialize)]
struct ReportArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    title: Option<String>,
    /// Which sections: study, documents, codebook, passages, agreement, adjudication, memos, ai_use, methods, ethics, mixed_methods, timeline. When absent, the usual six.
    #[serde(default)]
    sections: Option<Vec<prismqda_export::ReportSection>>,
}

#[derive(Deserialize)]
struct QdpxExportArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    /// Put the files of the linked recordings inside the file (up to a size).
    #[serde(default)]
    include_recordings: bool,
}

#[derive(Deserialize)]
struct DecideRegionArgs {
    /// The number the case goes by (`key` of a case in the queue).
    key: i64,
    /// Accept the area or stretch into the final layer, or leave it out.
    accept: bool,
    reason: String,
    /// Who decides; the person using the project when absent.
    #[serde(default)]
    adjudicator_id: Option<i64>,
}

#[derive(Deserialize)]
struct ThemeAnswerArgs {
    run_id: i64,
    /// Which model gave the answer; kept with the run.
    model: String,
    answer: String,
}

#[derive(Deserialize)]
struct ThemeRunArgs {
    run_id: i64,
}

#[derive(Deserialize)]
struct ThemeDecideArgs {
    run_id: i64,
    /// Which suggestion of the run, counting from 1.
    index: usize,
    accept: bool,
    /// A name to use instead of the suggested one.
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct ThemeOverviewArgs {
    /// Narrows what is counted and quoted, as the other Explore commands are (the working set).
    #[serde(default)]
    scope: ExploreScope,
}

#[derive(Deserialize)]
struct OfflineLockArgs {
    on: bool,
}

#[derive(Deserialize)]
struct LinkAddArgs {
    /// The passage the link starts at (a coding's id; an area of a picture or a stretch of a recording is its number made negative).
    from_coding: i64,
    to_coding: i64,
    /// How the first passage relates to the second, in a word or two.
    relation: String,
}

#[derive(Deserialize)]
struct LinkUpdateArgs {
    id: i64,
    relation: String,
}

#[derive(Deserialize)]
struct LinkListArgs {
    /// Only the links that start or end at this passage; all of them when left out.
    #[serde(default)]
    coding_id: Option<i64>,
}

#[derive(Deserialize)]
struct PacketMakeArgs {
    /// Where the packet goes; `.pqda` is added.
    path: String,
    #[serde(flatten)]
    spec: prismqda_engine::CoderPacketSpec,
}

#[derive(Deserialize)]
struct QdcImportArgs {
    /// The `.qdc` codebook file to read into the open project.
    path: String,
}

#[derive(Deserialize)]
struct QdpxImportArgs {
    /// The `.qdpx` file to read.
    path: String,
    /// Where the new project goes.
    project_path: String,
    /// A name for the project; the file's own name is used when this is left out.
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct ExportArgs {
    /// `xlsx`, `docx`, `csv`, or (codebook only) `qdc`.
    format: String,
    path: String,
    /// Codebook only: include merged and retired codes.
    #[serde(default)]
    include_inactive: bool,
    /// Replace a file that is already there. Off by default, so a typed path never overwrites by accident.
    #[serde(default)]
    overwrite: bool,
    /// Coded segments only: which codings to include.
    #[serde(flatten)]
    filter: SegmentFilter,
}

#[derive(Deserialize)]
struct SegmentsArgs {
    #[serde(default = "default_segment_limit")]
    limit: usize,
    #[serde(flatten)]
    filter: SegmentFilter,
}

fn default_segment_limit() -> usize {
    1000
}

#[derive(Deserialize)]
struct SampleArgs {
    size: usize,
    #[serde(default = "default_sample_seed")]
    seed: u64,
    #[serde(default)]
    stratify: Stratify,
    #[serde(default)]
    source_ids: Option<Vec<i64>>,
}

fn default_sample_seed() -> u64 {
    1
}

#[derive(Deserialize)]
struct SourceEdit {
    id: i64,
    text: String,
}

#[derive(Deserialize)]
struct Limit {
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    100
}

/// The stable id of the coder who stands for the person using the project.
pub(crate) fn actor_local_id(actor: &str) -> String {
    format!("user-{}", actor.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect::<String>())
}

/// A project that may be open, and the person using it.
pub struct Api {
    project: Option<Project>,
    actor: String,
    /// The name the program started with, which `history.name` goes back to when the person has not chosen one.
    default_actor: String,
    /// Whether opening a project makes a backup; tests turn this off.
    backup_on_open: bool,
    /// How many dated backups a project keeps (a setting of the person, passed on to every project that is opened).
    keep_backups: usize,
    /// Where backups go instead of beside the project, when the person chose a folder.
    backup_dir: Option<std::path::PathBuf>,
}

impl Api {
    /// `actor` is the name recorded on every audit event.
    pub fn new(actor: impl Into<String>) -> Self {
        let actor = actor.into();
        Self { project: None, default_actor: actor.clone(), actor, backup_on_open: true, keep_backups: DEFAULT_KEEP_BACKUPS, backup_dir: None }
    }

    pub fn without_backup_on_open(mut self) -> Self {
        self.backup_on_open = false;
        self
    }

    fn project(&mut self) -> Result<&mut Project, ApiError> {
        self.project.as_mut().ok_or_else(|| ApiError::new("no_project", "no project is open; create or open one first"))
    }

    fn open_options(&self, backup: bool) -> OpenOptions {
        let mut o = OpenOptions::new(self.actor.clone());
        o.backup_on_open = backup && self.backup_on_open;
        o.keep_backups = self.keep_backups;
        o.backup_dir = self.backup_dir.clone();
        o
    }

    /// The coder that stands for the person using the project, made on first use.
    fn me(&mut self) -> Result<i64, ApiError> {
        // In a coder packet the person using it is the coder it was made for (decision 161).
        if let Some(packet) = self.project()?.coder_packet_info()? {
            return Ok(self.project()?.ensure_human_coder(&packet.coder_name, &packet.coder_local_id)?);
        }
        let actor = self.actor.clone();
        Ok(self.project()?.ensure_human_coder(&actor, &actor_local_id(&actor))?)
    }

    fn info_json(&mut self) -> ApiResult {
        let me = if self.project.as_ref().is_some_and(|p| !p.is_read_only()) { Some(self.me()?) } else { None };
        let p = self.project()?;
        let info = p.info()?;
        let mut v = to_json(&info)?;
        v["notices"] = json!(p.notices());
        v["me"] = json!(me);
        Ok(v)
    }

    /// Runs one command. Commands are named `area.action`.
    pub fn call(&mut self, cmd: &str, args: Value) -> ApiResult {
        let args = if args.is_null() { json!({}) } else { args };
        match cmd {
            "ping" => Ok(json!({ "name": "PrismQDA", "version": env!("CARGO_PKG_VERSION") })),

            // ---- the project ----
            "project.create" => {
                let a: CreateArgs = parse(cmd, args)?;
                let mut new = NewProject::new(a.name);
                if let Some(m) = a.coding_mode {
                    new.coding_mode = m;
                }
                if let Some(m) = a.method {
                    new.method_template = m;
                }
                new.research_questions = a.research_questions.unwrap_or_default();
                let mut opts = self.open_options(false);
                opts.passphrase = a.passphrase;
                self.project = Some(Project::create(PathBuf::from(a.path), &new, &opts)?);
                self.info_json()
            }
            "project.open" => {
                let a: OpenArgs = parse(cmd, args)?;
                let mut opts = self.open_options(a.backup);
                opts.passphrase = a.passphrase;
                self.project = Some(Project::open(PathBuf::from(a.path), &opts)?);
                self.info_json()
            }
            // ---- encrypting the project file (decision 157) ----
            "project.encrypt" => {
                let a: PassphraseArgs = parse(cmd, args)?;
                let outcome = self.project()?.encrypt(&a.passphrase)?;
                let mut v = to_json(&outcome)?;
                v["info"] = self.info_json()?;
                Ok(v)
            }
            "project.decrypt" => {
                let a: PassphraseArgs = parse(cmd, args)?;
                let outcome = self.project()?.decrypt(&a.passphrase)?;
                let mut v = to_json(&outcome)?;
                v["info"] = self.info_json()?;
                Ok(v)
            }
            "project.change_passphrase" => {
                let a: ChangePassphraseArgs = parse(cmd, args)?;
                let outcome = self.project()?.change_passphrase(&a.old, &a.new)?;
                let mut v = to_json(&outcome)?;
                v["info"] = self.info_json()?;
                Ok(v)
            }
            "project.stale_backups" => Ok(json!({ "backups": self.project()?.stale_backups()? })),
            "project.delete_stale_backups" => Ok(json!({ "deleted": self.project()?.delete_stale_backups()? })),
            "project.close" => {
                self.project = None;
                Ok(json!({}))
            }
            "project.info" => self.info_json(),
            "project.backup" => {
                // `note` is there when the chosen backup folder could not be used and the copy was made beside the project.
                let (path, note) = self.project()?.backup_now_noted()?;
                Ok(json!({ "path": path.display().to_string(), "note": note }))
            }
            // Where backups go: a folder the person chose (checked first unless `check` is false), or null for beside the
            // project. For the open project and every project opened after it.
            "backup.dir" => {
                let a: BackupDirArgs = parse(cmd, args)?;
                let dir = a.dir.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).map(std::path::PathBuf::from);
                if let (Some(d), true) = (&dir, a.check) {
                    check_backup_dir(d)?;
                }
                self.backup_dir = dir.clone();
                if let Some(project) = self.project.as_mut() {
                    project.set_backup_dir(dir.clone());
                }
                Ok(json!({ "dir": dir.map(|d| d.display().to_string()) }))
            }
            // How many dated backups to keep (1 to 50): for the open project and every project opened after it.
            "backup.keep" => {
                let a: KeepArgs = parse(cmd, args)?;
                if !(1..=50).contains(&a.keep) {
                    return Err(ApiError::new("invalid", "keep between 1 and 50 backups"));
                }
                self.keep_backups = a.keep;
                if let Some(project) = self.project.as_mut() {
                    project.set_keep_backups(a.keep);
                }
                Ok(json!({ "keep": a.keep }))
            }
            // The name the person goes by in the history of a project: recorded on every change they make, and the name of the coder
            // they are. For the open project and every project opened after it. It is never taken from the Windows account: a project
            // or a copy that is shared would carry that name on every change.
            "history.name" => {
                let a: HistoryNameArgs = parse(cmd, args)?;
                let mut name = a.name.trim().to_string();
                if name.chars().count() > 60 || name.chars().any(|c| c.is_control()) {
                    return Err(ApiError::new("invalid", "a name for the project history is one to sixty characters, on one line"));
                }
                // No name chosen: the program's own, which is never the Windows account name.
                if name.is_empty() {
                    name = self.default_actor.clone();
                }
                self.actor = name.clone();
                if let Some(project) = self.project.as_mut() {
                    project.set_actor(&name);
                }
                Ok(json!({ "name": name }))
            }

            // ---- the launcher: recent files and bundled samples (none of these needs a project to be open) ----
            "file.stat" => {
                let a: PathArgs = parse(cmd, args)?;
                files::stat(&a.path)
            }
            "text.read" => {
                let a: PathArgs = parse(cmd, args)?;
                text_files::read(&a.path)
            }
            "text.write" => {
                let a: TextWrite = parse(cmd, args)?;
                text_files::write(&a.path, &a.text, a.replace)
            }
            "sample.list" => samples::list(),
            "sample.create" => {
                let a: BundledSampleArgs = parse(cmd, args)?;
                // The project that is open stays open unless the copy is made, like project.create.
                let project = samples::create(&a.id, &a.path, &self.open_options(false))?;
                self.project = Some(project);
                self.info_json()
            }

            // ---- importing ----
            "import.inspect" => self.import_inspect(parse(cmd, args)?),
            "import.run" => self.import_run(parse(cmd, args)?),
            "import.revise" => self.import_revise(parse(cmd, args)?),

            // ---- recordings linked to documents ----
            "media.get" => {
                let a: SourceId = parse(cmd, args)?;
                to_json(&self.project()?.get_media(a.source_id)?)
            }
            "media.list" => to_json(&self.project()?.list_media()?),
            "media.link" => {
                let a: MediaLink = parse(cmd, args)?;
                to_json(&self.project()?.link_media(a.source_id, &a.path)?)
            }
            "media.unlink" => {
                let a: SourceId = parse(cmd, args)?;
                self.project()?.unlink_media(a.source_id)?;
                Ok(json!({}))
            }
            "import.folder" => self.import_folder(parse(cmd, args)?),

            // ---- pictures and the coded areas of pictures ----
            "image.get" => {
                let a: SourceId = parse(cmd, args)?;
                to_json(&self.project()?.get_image(a.source_id)?)
            }
            "image.list" => to_json(&self.project()?.list_images()?),
            "image.save_copy" => {
                let a: ImageCopy = parse(cmd, args)?;
                let path = PathBuf::from(&a.path);
                if !path.is_absolute() {
                    return Err(ApiError::new("invalid", "type the full path of the file to write, for example C:\\Pictures\\street.png"));
                }
                self.project()?.save_image_copy(a.source_id, &path)?;
                Ok(json!({ "path": a.path }))
            }
            // ---- PDFs kept with documents, and the coded areas of their pages ----
            "pdf.get" => {
                let a: SourceId = parse(cmd, args)?;
                to_json(&self.project()?.get_pdf(a.source_id)?)
            }
            "pdf.attach" => {
                let a: PdfAttach = parse(cmd, args)?;
                let path = PathBuf::from(&a.path);
                if !path.is_absolute() {
                    return Err(ApiError::new("invalid", "type the full path of the PDF, for example C:\\Documents\\report.pdf"));
                }
                if !path.metadata().is_ok_and(|m| m.is_file() && m.len() > 0) {
                    return Err(ApiError::new("io", format!("the PDF {} cannot be read: it is missing, empty, or a folder", a.path)));
                }
                let bytes = std::fs::read(&path).map_err(|e| ApiError::new("io", format!("the PDF {} cannot be read: {e}", a.path)))?;
                if !bytes.windows(5).take(1024).any(|w| w == b"%PDF-") {
                    return Err(ApiError::new("invalid", format!("{} is not a PDF", a.path)));
                }
                let sizes = prismqda_pdfview::page_sizes(&bytes).map_err(|e| ApiError::new("invalid", e.to_string()))?;
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| a.path.clone());
                let pdf = prismqda_engine::PdfData { file_name: name, pages: sizes.iter().map(|s| (s.width, s.height)).collect(), bytes };
                to_json(&self.project()?.attach_pdf(a.source_id, &pdf)?)
            }
            "pdf.save_copy" => {
                let a: ImageCopy = parse(cmd, args)?;
                let path = PathBuf::from(&a.path);
                if !path.is_absolute() {
                    return Err(ApiError::new("invalid", "type the full path of the file to write, for example C:\\Documents\\report.pdf"));
                }
                self.project()?.save_pdf_copy(a.source_id, &path)?;
                Ok(json!({ "path": a.path }))
            }
            "region.list" => {
                let a: SourceId = parse(cmd, args)?;
                to_json(&self.project()?.list_regions(a.source_id)?)
            }
            "region.add" => {
                let a: RegionAdd = parse(cmd, args)?;
                let coder = match a.coder_id {
                    Some(c) => c,
                    None => self.me()?,
                };
                let mut new = NewRegion::new(a.source_id, a.shape, a.code_id, coder);
                new.page = a.page;
                new.rationale = a.rationale;
                let p = self.project()?;
                let id = p.apply_region(&new)?;
                let row = p.get_region(id)?;
                Ok(json!({ "id": id, "coding_id": -id, "region": row }))
            }
            "region.reshape" => {
                let a: RegionReshape = parse(cmd, args)?;
                to_json(&self.project()?.reshape_region(a.id, &a.shape)?)
            }

            // ---- sources, units, cases ----
            "source.list" => to_json(&self.project()?.list_sources()?),
            "source.get" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.source_detail(a.id)?)
            }
            "unit.list" => {
                let a: SourceId = parse(cmd, args)?;
                to_json(&self.project()?.list_units(a.source_id)?)
            }
            "unit.attributes" => {
                let a: SourceId = parse(cmd, args)?;
                to_json(&self.project()?.unit_attribute_values(a.source_id)?)
            }
            "case.list" => to_json(&self.project()?.list_cases()?),
            "case.variables" => to_json(&self.project()?.case_variables()?),

            // ---- exploring the coding ----
            "explore.summary" => {
                let a: ScopeArgs = parse(cmd, args)?;
                to_json(&self.project()?.code_summaries(&a.scope)?)
            }
            "explore.profile" => {
                let a: ProfileArgs = parse(cmd, args)?;
                to_json(&self.project()?.code_profile(a.code_id, a.include_descendants, &a.scope)?)
            }
            "explore.matrix" => {
                let spec: MatrixSpec = parse(cmd, args)?;
                to_json(&self.project()?.code_matrix(&spec)?)
            }
            "explore.matrix_evidence" => {
                let a: MatrixEvidenceArgs = parse(cmd, args)?;
                let ids = self.project()?.code_matrix_evidence(&a.spec, a.row_key.as_deref(), a.code_id)?;
                Ok(json!({ "coding_ids": ids }))
            }
            "explore.relations" => {
                let spec: RelationsSpec = parse(cmd, args)?;
                to_json(&self.project()?.code_relations(&spec)?)
            }
            "explore.relations_evidence" => {
                let a: RelationsEvidenceArgs = parse(cmd, args)?;
                let ids = self.project()?.code_relations_evidence(&a.spec, a.a, a.b)?;
                Ok(json!({ "coding_ids": ids }))
            }
            "explore.query" => {
                let spec: QuerySpec = parse(cmd, args)?;
                to_json(&self.project()?.run_query(&spec)?)
            }
            "explore.crosstab" => {
                let spec: CrosstabSpec = parse(cmd, args)?;
                to_json(&self.project()?.crosstab(&spec)?)
            }
            "explore.crosstab_cases" => {
                let a: CrosstabCasesArgs = parse(cmd, args)?;
                to_json(&self.project()?.crosstab_cases(&a.spec, a.row_key.as_deref(), a.column_key.as_deref())?)
            }

            // ---- concept maps ----
            "map.list" => to_json(&self.project()?.list_concept_maps()?),
            "map.get" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.get_concept_map(a.id)?)
            }
            "map.create" => {
                let a: MapCreate = parse(cmd, args)?;
                let p = self.project()?;
                let id = p.create_concept_map(&a.name)?;
                to_json(&p.get_concept_map(id)?)
            }
            "map.update" => {
                let a: MapUpdate = parse(cmd, args)?;
                to_json(&self.project()?.update_concept_map(a.id, a.name.as_deref(), a.notes.as_deref())?)
            }
            "map.delete" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.delete_concept_map(a.id)?;
                Ok(json!({}))
            }
            "map.node_add" => {
                let a: MapNodeAdd = parse(cmd, args)?;
                to_json(&self.project()?.add_map_node(a.map_id, &a.kind, a.ref_id, &a.label, a.x, a.y)?)
            }
            "map.node_move" => {
                let a: MapNodeMove = parse(cmd, args)?;
                self.project()?.move_map_node(a.id, a.x, a.y)?;
                Ok(json!({}))
            }
            "map.node_note" => {
                let a: MapNodeNote = parse(cmd, args)?;
                self.project()?.update_map_note(a.id, &a.label)?;
                Ok(json!({}))
            }
            "map.node_delete" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.delete_map_node(a.id)?;
                Ok(json!({}))
            }
            "map.link_add" => {
                let a: MapLinkAdd = parse(cmd, args)?;
                to_json(&self.project()?.add_map_link(a.map_id, a.from, a.to, &a.label, a.directed)?)
            }
            "map.link_update" => {
                let a: MapLinkUpdate = parse(cmd, args)?;
                to_json(&self.project()?.update_map_link(a.id, a.label.as_deref(), a.directed)?)
            }
            "map.link_delete" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.delete_map_link(a.id)?;
                Ok(json!({}))
            }
            "map.layout" => {
                let a: MapLayoutArgs = parse(cmd, args)?;
                let p = self.project()?;
                p.layout_concept_map(a.map_id, &a.how)?;
                to_json(&p.get_concept_map(a.map_id)?)
            }
            "map.add_codes" => {
                let a: MapAddCodes = parse(cmd, args)?;
                to_json(&self.project()?.add_codes_to_map(a.map_id, &a.code_ids, a.hierarchy, a.min_together)?)
            }

            // ---- saved and frozen queries ----
            "query.list" => to_json(&self.project()?.list_saved_queries()?),
            "query.get" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.get_saved_query(a.id)?)
            }
            "query.save" => {
                let a: SavedQuerySave = parse(cmd, args)?;
                let p = self.project()?;
                let id = p.save_query(&a.name, &a.note, &a.draft, &a.spec)?;
                to_json(&p.get_saved_query(id)?)
            }
            "query.update" => {
                let a: SavedQueryUpdate = parse(cmd, args)?;
                let question = match (&a.draft, &a.spec) {
                    (Some(d), Some(s)) => Some((d, s)),
                    (None, None) => None,
                    _ => return Err(ApiError::new("bad_args", "query.update: to change what the query asks, give the draft and the spec together")),
                };
                to_json(&self.project()?.update_saved_query(a.id, a.name.as_deref(), a.note.as_deref(), question)?)
            }
            "query.delete" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.delete_saved_query(a.id)?;
                Ok(json!({}))
            }
            "query.freeze" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.freeze_saved_query(a.id)?)
            }
            "query.unfreeze" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.unfreeze_saved_query(a.id)?;
                Ok(json!({}))
            }
            "query.run_saved" => {
                let a: Id = parse(cmd, args)?;
                let (saved, result) = self.project()?.saved_query_result(a.id)?;
                Ok(json!({ "query": saved, "result": result }))
            }
            "query.compare" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.compare_frozen_query(a.id)?)
            }
            "case.add" => {
                let a: CaseAdd = parse(cmd, args)?;
                to_json(&self.project()?.add_case(&a.name, &a.notes)?)
            }
            "case.assign" => {
                let a: CaseMembership = parse(cmd, args)?;
                let p = self.project()?;
                p.assign_source_to_case(a.case_id, a.source_id)?;
                to_json(&p.get_case(a.case_id)?)
            }
            "case.unassign" => {
                let a: CaseMembership = parse(cmd, args)?;
                let p = self.project()?;
                p.unassign_source_from_case(a.case_id, a.source_id)?;
                to_json(&p.get_case(a.case_id)?)
            }

            // ---- codes ----
            "code.list" => to_json(&self.project()?.list_codes()?),
            "code.add" => {
                let a: CodeAdd = parse(cmd, args)?;
                let code = NewCode {
                    name: a.name,
                    parent_id: a.parent_id,
                    theme: a.theme,
                    color: a.color,
                    definition: a.definition,
                    inclusion_criteria: a.inclusion_criteria,
                    exclusion_criteria: a.exclusion_criteria,
                    exclusive_group: a.exclusive_group,
                    rated: a.rated,
                    rating_label: a.rating_label,
                    rating_low: a.rating_low,
                    rating_high: a.rating_high,
                };
                let id = self.project()?.add_code(&code)?;
                to_json(&self.project()?.get_code(id)?)
            }
            "code.update" => {
                let a: CodeUpdate = parse(cmd, args)?;
                to_json(&self.project()?.update_code(a.id, &a.edit)?)
            }
            "code.move" => {
                let a: CodeMove = parse(cmd, args)?;
                self.project()?.move_code(a.id, a.parent_id, a.position)?;
                Ok(json!({}))
            }
            "code.retire" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.retire_code(a.id)?;
                Ok(json!({}))
            }
            "code.restore" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.restore_code(a.id)?;
                Ok(json!({}))
            }
            "code.merge" => {
                let a: CodeMerge = parse(cmd, args)?;
                to_json(&self.project()?.merge_codes(a.from, a.into)?)
            }
            "code.split" => {
                let a: CodeSplit = parse(cmd, args)?;
                let mut new = NewCode::new(a.name);
                new.definition = a.definition;
                let id = self.project()?.split_code(a.from, &new, &a.coding_ids)?;
                to_json(&self.project()?.get_code(id)?)
            }
            "codebook.snapshot" => {
                let note = args.get("note").and_then(Value::as_str).unwrap_or("").to_string();
                Ok(json!({ "version": self.project()?.snapshot_codebook(&note)? }))
            }
            "codebook.versions" => to_json(&self.project()?.list_codebook_versions()?),
            "codebook.version" => {
                let a: VersionArgs = parse(cmd, args)?;
                Ok(self.project()?.get_codebook_version(a.version)?)
            }

            // ---- coders and codings ----
            "coder.list" => to_json(&self.project()?.list_coders()?),
            "coder.add" => {
                let a: CoderAdd = parse(cmd, args)?;
                let id = self.project()?.add_coder(&NewCoder {
                    name: a.name,
                    role: a.role,
                    local_id: a.local_id,
                    provider: a.provider,
                    model_name: a.model_name,
                    model_version: a.model_version,
                })?;
                Ok(json!({ "id": id }))
            }
            "coding.list" => {
                let a: SourceId = parse(cmd, args)?;
                to_json(&self.project()?.list_codings(a.source_id)?)
            }
            "coding.apply" => {
                let a: CodingApply = parse(cmd, args)?;
                let coder = match a.coder_id {
                    Some(c) => c,
                    None => self.me()?,
                };
                let mut coding = NewCoding::new(a.source_id, a.start, a.end, a.code_id, coder);
                coding.rationale = a.rationale;
                coding.unit_id = a.unit_id;
                coding.replace_exclusive = a.replace_exclusive;
                coding.origin = Origin::Human;
                let p = self.project()?;
                let id = p.apply_coding(&coding)?;
                let quote = p.text_of_span(a.source_id, a.start, a.end)?;
                Ok(json!({ "id": id, "quote": quote }))
            }
            // A negative number names a coded area of a picture (area 7 is -7); the commands below take either kind.
            "coding.remove" => {
                let a: Id = parse(cmd, args)?;
                if a.id < 0 {
                    self.project()?.remove_region(-a.id)?;
                } else {
                    self.project()?.remove_coding(a.id)?;
                }
                Ok(json!({}))
            }
            "coding.change_code" => {
                let a: CodingChange = parse(cmd, args)?;
                if a.id < 0 {
                    self.project()?.change_region_code(-a.id, a.code_id)?;
                } else {
                    self.project()?.change_coding_code(a.id, a.code_id)?;
                }
                Ok(json!({}))
            }
            "coding.rationale" => {
                let a: CodingRationale = parse(cmd, args)?;
                if a.id < 0 {
                    self.project()?.set_region_rationale(-a.id, &a.rationale)?;
                } else {
                    self.project()?.set_coding_rationale(a.id, &a.rationale)?;
                }
                Ok(json!({}))
            }
            "coding.anchor" => {
                let a: CodingAnchor = parse(cmd, args)?;
                if a.id < 0 {
                    return Err(ApiError::new("invalid", "anchor examples are not available for coded areas of pictures yet"));
                }
                self.project()?.set_anchor(a.id, a.anchor)?;
                Ok(json!({}))
            }
            "coding.counterexample" => {
                let a: CodingCounterexample = parse(cmd, args)?;
                if a.id < 0 {
                    return Err(ApiError::new("invalid", "counterexamples are not available for coded areas of pictures yet"));
                }
                self.project()?.set_counterexample(a.code_id, a.id, a.on)?;
                Ok(json!({}))
            }
            "code.counterexamples" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.counterexample_segments(a.id)?)
            }
            "code.group_check" => {
                // How many passages already have two codes of a group at once (made before the rule existed).
                let a: GroupName = parse(cmd, args)?;
                let pairs = self.project()?.exclusive_group_violations(&a.group)?;
                let first: Vec<&(i64, i64)> = pairs.iter().take(20).collect();
                Ok(json!({ "violations": pairs.len(), "first": first }))
            }
            "coding.rating" => {
                let a: CodingRating = parse(cmd, args)?;
                let rating = match &a.rating {
                    Value::Null => None,
                    Value::Number(n) => Some(n.as_i64().ok_or_else(|| {
                        ApiError::new("bad_args", format!("{cmd}: the rating must be a whole number from 1 to 5, or null, not {n}"))
                    })?),
                    other => {
                        return Err(ApiError::new("bad_args", format!("{cmd}: the rating must be a whole number from 1 to 5, or null, not {other}")))
                    }
                };
                if a.id < 0 {
                    self.project()?.set_region_rating(-a.id, rating)?;
                } else {
                    self.project()?.set_coding_rating(a.id, rating)?;
                }
                Ok(json!({}))
            }
            "coding.history" => {
                let a: Id = parse(cmd, args)?;
                if a.id < 0 {
                    to_json(&self.project()?.region_history(-a.id)?)
                } else {
                    to_json(&self.project()?.coding_history(a.id)?)
                }
            }

            // ---- memos ----
            "memo.list" => {
                let a: MemoFilter = parse(cmd, args)?;
                to_json(&self.project()?.list_memos(a.scope, a.target_id)?)
            }
            "memo.add" => {
                let memo: NewMemo = parse(cmd, args)?;
                let id = self.project()?.add_memo(&memo)?;
                Ok(json!({ "id": id }))
            }
            "memo.update" => {
                let a: MemoUpdate = parse(cmd, args)?;
                self.project()?.update_memo(a.id, a.title.as_deref(), a.body.as_deref())?;
                Ok(json!({}))
            }
            "memo.delete" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.delete_memo(a.id)?;
                Ok(json!({}))
            }

            // ---- searching, autocoding, and the review queue ----
            "search.run" => {
                let a: SearchArgs = parse(cmd, args)?;
                to_json(&self.project()?.search(&a.query, &a.filter)?)
            }
            "autocode.preview" => {
                let a: autocode::AutocodeArgs = parse(cmd, args)?;
                let coder = match a.coder_id {
                    Some(c) => c,
                    None => self.me()?,
                };
                autocode::preview(self.project()?, &a, coder)
            }
            "autocode.propose" => {
                let a: autocode::AutocodeArgs = parse(cmd, args)?;
                let coder = match a.coder_id {
                    Some(c) => c,
                    None => self.me()?,
                };
                autocode::propose(self.project()?, &a, coder)
            }
            "autocode.speakers" => {
                let a: autocode::ScopeArgs = parse(cmd, args)?;
                autocode::speakers(self.project()?, &a)
            }
            "autocode.headings" => {
                let a: autocode::ScopeArgs = parse(cmd, args)?;
                autocode::headings(self.project()?, &a)
            }
            "proposal.list" => {
                let a: ProposalList = parse(cmd, args)?;
                to_json(&self.project()?.list_proposals_page(a.status.as_deref(), a.batch.as_deref(), a.limit, a.offset)?)
            }
            "proposal.batches" => to_json(&self.project()?.proposal_batches()?),
            "proposal.decide" => {
                let a: ProposalDecide = parse(cmd, args)?;
                let project = self.project()?;
                match a.batch {
                    Some(batch) if a.ids.is_empty() => to_json(&project.decide_batch(&batch, a.accept)?),
                    _ => to_json(&project.decide_proposals(&a.ids, a.accept)?),
                }
            }
            "proposal.adjust" => {
                let a: ProposalAdjust = parse(cmd, args)?;
                to_json(&self.project()?.adjust_proposal(a.id, a.start, a.end, a.code_id)?)
            }

            // ---- AI through prompt packets (the program sends nothing anywhere) ----
            "ai.packet" => {
                let spec: prismqda_engine::PacketSpec = parse(cmd, args)?;
                let coder = match spec.coder_id {
                    Some(c) => c,
                    None => self.me()?,
                };
                to_json(&self.project()?.build_packet(&spec, coder)?)
            }
            "ai.answer" => {
                let a: AiAnswer = parse(cmd, args)?;
                to_json(&self.project()?.answer_packet(a.run_id, &a.model, &a.answer)?)
            }
            "ai.runs" => to_json(&self.project()?.ai_runs()?),
            "ai.run" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.ai_run(a.id)?)
            }
            "ai.disclosure" => Ok(json!({ "text": self.project()?.ai_disclosure()? })),
            "ai.set_mode" => {
                let a: AiMode = parse(cmd, args)?;
                self.project()?.set_ai_mode(&a.mode)?;
                self.info_json()
            }
            "source.set_do_not_share" => {
                let a: DoNotShare = parse(cmd, args)?;
                self.project()?.set_source_do_not_share(a.id, a.do_not_share)?;
                Ok(json!({}))
            }

            // ---- editing a source's text ----
            "source.edit" => {
                let a: SourceEdit = parse(cmd, args)?;
                to_json(&self.project()?.replace_source_text(a.id, &a.text)?)
            }

            // ---- method templates ----
            "template.list" => to_json(&prismqda_engine::built_in_templates()),
            "template.state" => to_json(&self.project()?.template_state()?),
            "template.set" => {
                let id = args.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let seed = args.get("seed_codes").and_then(Value::as_bool).unwrap_or(false);
                to_json(&self.project()?.set_template(&id, seed)?)
            }
            "template.tick" => {
                let phase = args.get("phase").and_then(Value::as_str).unwrap_or("").to_string();
                let item = args.get("item").and_then(Value::as_u64).unwrap_or(u64::MAX) as usize;
                let done = args.get("done").and_then(Value::as_bool).unwrap_or(true);
                self.project()?.tick_item(&phase, item, done)?;
                to_json(&self.project()?.template_state()?)
            }
            "template.move" => {
                let phase = args.get("phase").and_then(Value::as_str).unwrap_or("").to_string();
                self.project()?.move_to_phase(&phase)?;
                to_json(&self.project()?.template_state()?)
            }
            "template.check" => to_json(&self.project()?.check_codebook()?),

            // ---- undo ----
            "undo" => match self.project()?.undo()? {
                Some(outcome) => to_json(&outcome),
                None => Ok(json!({ "nothing_to_undo": true })),
            },
            "undo.next" => Ok(json!({ "next": self.project()?.next_undo()? })),

            // ---- coded segments and exports ----
            "segments.list" => {
                let a: SegmentsArgs = parse(cmd, args)?;
                let all = self.project()?.coded_segments(&a.filter)?;
                let total = all.len();
                let shown: Vec<_> = all.into_iter().take(a.limit).collect();
                Ok(json!({ "segments": shown, "total": total, "truncated": total > a.limit }))
            }
            "export.codebook" => {
                let a: ExportArgs = parse(cmd, args)?;
                let format = export_format(&a.format, true)?;
                let _scrub = self.scrub_scope()?;
                let bytes = prismqda_export::export_codebook(self.project()?, format, &prismqda_export::CodebookOptions { include_inactive: a.include_inactive })?;
                write_export(&a.path, format, a.overwrite, &bytes)
            }
            // ---- a whole project in the REFI-QDA exchange format (decision 159) ----
            "export.qdpx" => {
                let a: QdpxExportArgs = parse(cmd, args)?;
                // While the list of names to replace is applied to exports, the writer refuses: the text goes in whole.
                let _scrub = self.scrub_scope()?;
                let (bytes, report) = prismqda_export::export_qdpx_with(self.project()?, &prismqda_export::QdpxOptions { include_recordings: a.include_recordings })?;
                let mut out = write_export_as(&a.path, "qdpx", a.overwrite, &bytes)?;
                out["report"] = to_json(&report)?;
                Ok(out)
            }
            // ---- developing themes (decision 167) ----
            "theme.overview" => {
                let a: ThemeOverviewArgs = parse(cmd, args)?;
                to_json(&self.project()?.theme_overview(&a.scope)?)
            }
            "theme.ai_packet" => {
                let me = self.me()?;
                to_json(&self.project()?.build_theme_packet(me)?)
            }
            "theme.ai_answer" => {
                let a: ThemeAnswerArgs = parse(cmd, args)?;
                to_json(&self.project()?.answer_theme_packet(a.run_id, &a.model, &a.answer)?)
            }
            "theme.ai_run" => {
                let a: ThemeRunArgs = parse(cmd, args)?;
                to_json(&self.project()?.theme_run(a.run_id)?)
            }
            "theme.ai_decide" => {
                let a: ThemeDecideArgs = parse(cmd, args)?;
                to_json(&self.project()?.decide_theme_suggestion(a.run_id, a.index, a.accept, a.name.as_deref())?)
            }
            "theme.map" => {
                let id = self.project()?.make_thematic_map()?;
                Ok(json!({ "map_id": id }))
            }
            // ---- the offline lock and the facts for an ethics summary (decision 165) ----
            "privacy.set_offline_lock" => {
                let a: OfflineLockArgs = parse(cmd, args)?;
                self.project()?.set_offline_lock(a.on)?;
                self.info_json()
            }
            "privacy.facts" => to_json(&self.project()?.privacy_facts()?),
            // ---- links between passages with a named relation (decision 162) ----
            "link.add" => {
                let a: LinkAddArgs = parse(cmd, args)?;
                let id = self.project()?.add_passage_link(a.from_coding, a.to_coding, &a.relation)?;
                Ok(json!({ "id": id }))
            }
            "link.update" => {
                let a: LinkUpdateArgs = parse(cmd, args)?;
                self.project()?.update_passage_link(a.id, &a.relation)?;
                Ok(json!({}))
            }
            "link.delete" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.delete_link(a.id)?;
                Ok(json!({}))
            }
            "link.list" => {
                let a: LinkListArgs = parse(cmd, args)?;
                to_json(&self.project()?.list_passage_links(a.coding_id)?)
            }
            "link.relations" => {
                let used = self.project()?.passage_relations()?;
                Ok(json!({ "used": used, "common": prismqda_engine::COMMON_RELATIONS }))
            }
            // ---- a packet for another coder, and the way it comes back (decision 161) ----
            "packet.make" => {
                let a: PacketMakeArgs = parse(cmd, args)?;
                // The documents go into a packet whole, with any names in them, so the switch that replaces names in exports stops it (as it stops a QDPX).
                if self.project()?.deid_state()?.exports {
                    return Err(ApiError::new(
                        "invalid",
                        "the list of names to replace is applied to exports, and a packet holds the documents whole; turn that switch off, or replace the names in the project itself first",
                    ));
                }
                let mut target = PathBuf::from(a.path.trim());
                if target.as_os_str().is_empty() {
                    return Err(ApiError::new("invalid", "say where the packet should go"));
                }
                if target.extension().and_then(|e| e.to_str()).is_none_or(|e| !e.eq_ignore_ascii_case("pqda")) {
                    let mut name = target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    name.push_str(".pqda");
                    target.set_file_name(name);
                }
                check_target(&target, false)?;
                to_json(&self.project()?.make_coder_packet(&target, &a.spec)?)
            }
            // ---- a codebook from another program, read into the open project (decision 160) ----
            "import.qdc" => {
                let a: QdcImportArgs = parse(cmd, args)?;
                let source = PathBuf::from(a.path.trim());
                let meta = fs::metadata(&source).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", source.display())))?;
                if meta.len() > 64 << 20 {
                    return Err(ApiError::new("too_large", "this file is over 64 MB, far more than a codebook holds"));
                }
                let bytes = fs::read(&source).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", source.display())))?;
                let report = prismqda_export::import_qdc(self.project()?, &bytes)?;
                Ok(json!({ "report": to_json(&report)? }))
            }
            "import.qdpx" => {
                let a: QdpxImportArgs = parse(cmd, args)?;
                let source = PathBuf::from(a.path.trim());
                let meta = fs::metadata(&source).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", source.display())))?;
                if meta.len() > 1_500_000_000 {
                    return Err(ApiError::new("too_large", "this file is over 1.5 GB, more than PrismQDA reads into memory at once"));
                }
                let bytes = fs::read(&source).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", source.display())))?;
                let mut target = PathBuf::from(a.project_path.trim());
                if target.as_os_str().is_empty() {
                    return Err(ApiError::new("invalid", "say where the new project should go"));
                }
                if target.extension().and_then(|e| e.to_str()).is_none_or(|e| !e.eq_ignore_ascii_case("pqda")) {
                    let mut name = target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    name.push_str(".pqda");
                    target.set_file_name(name);
                }
                check_target(&target, false)?;
                let name = a.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).or_else(|| source.file_stem().map(|s| s.to_string_lossy().to_string()));
                let report = match prismqda_export::import_qdpx(&bytes, &target, name.as_deref()) {
                    Ok(r) => r,
                    Err(e) => {
                        // A refused file leaves no half-made project behind.
                        for suffix in ["", "-wal", "-shm"] {
                            let mut leftover = target.clone().into_os_string();
                            leftover.push(suffix);
                            let _ = fs::remove_file(PathBuf::from(leftover));
                        }
                        return Err(e.into());
                    }
                };
                Ok(json!({ "project_path": target.display().to_string(), "report": to_json(&report)? }))
            }
            // ---- text analytics ----
            "analytics.words" => {
                let a: prismqda_engine::WordSpec = parse(cmd, args)?;
                to_json(&self.project()?.word_frequencies(&a)?)
            }
            "analytics.kwic" => {
                let a: prismqda_engine::ContextSpec = parse(cmd, args)?;
                to_json(&self.project()?.keywords_in_context(&a)?)
            }

            // ---- the data editor: variables of cases, their values, and transformations ----
            "data.get" => to_json(&self.project()?.case_data()?),
            "variable.define" => {
                let a: DefineVariableArgs = parse(cmd, args)?;
                to_json(&self.project()?.define_variable(&a.spec, a.existing.as_deref())?)
            }
            "variable.delete" => {
                let a: VariableName = parse(cmd, args)?;
                self.project()?.delete_variable(&a.name)?;
                Ok(json!({}))
            }
            "variable.set_values" => {
                let a: SetValuesArgs = parse(cmd, args)?;
                let changed = self.project()?.set_values(&a.variable, &a.changes)?;
                Ok(json!({ "changed": changed }))
            }
            "joint.case_ordered" => {
                let a: prismqda_engine::CaseDisplaySpec = parse(cmd, args)?;
                to_json(&self.project()?.case_ordered_display(&a)?)
            }
            "joint.typology" => {
                let a: prismqda_engine::TypologySpec = parse(cmd, args)?;
                to_json(&self.project()?.typology_table(&a)?)
            }
            "joint.side_by_side" => {
                let a: prismqda_engine::SideBySideSpec = parse(cmd, args)?;
                to_json(&self.project()?.side_by_side(&a)?)
            }
            "joint.deviation" => {
                let a: prismqda_engine::DeviationSpec = parse(cmd, args)?;
                to_json(&self.project()?.deviation_table(&a)?)
            }
            "joint.similarity" => {
                let a: prismqda_engine::SimilaritySpec = parse(cmd, args)?;
                to_json(&self.project()?.similarity_analysis(&a)?)
            }
            "joint.bubble" => {
                let a: prismqda_engine::BubbleSpec = parse(cmd, args)?;
                to_json(&self.project()?.bubble_plot(&a)?)
            }
            "mixed.design" => to_json(&self.project()?.mixed_design()?),
            "mixed.design.plan" => {
                let a: DesignKind = parse(cmd, args)?;
                to_json(&self.project()?.design_plan(&a.kind)?)
            }
            "mixed.design.apply" => {
                let a: prismqda_engine::DesignApply = parse(cmd, args)?;
                to_json(&self.project()?.design_apply(&a)?)
            }
            "mixed.design.set" => {
                let a: prismqda_engine::DesignInput = parse(cmd, args)?;
                to_json(&self.project()?.mixed_design_set(&a)?)
            }
            "mixed.design.clear" => {
                self.project()?.mixed_design_clear()?;
                Ok(json!({}))
            }
            "mixed.triangulation" => to_json(&self.project()?.triangulation()?),
            "mixed.triangulation.add" => {
                let a: prismqda_engine::TriangulationInput = parse(cmd, args)?;
                to_json(&self.project()?.triangulation_add(&a)?)
            }
            "mixed.triangulation.update" => {
                let a: TriangulationUpdate = parse(cmd, args)?;
                to_json(&self.project()?.triangulation_update(a.id, &a.row)?)
            }
            "mixed.triangulation.remove" => {
                let a: RowId = parse(cmd, args)?;
                self.project()?.triangulation_remove(a.id)?;
                Ok(json!({}))
            }
            "mixed.triangulation.move" => {
                let a: RowMove = parse(cmd, args)?;
                self.project()?.triangulation_move(a.id, a.up)?;
                Ok(json!({}))
            }
            "mixed.integration" => to_json(&self.project()?.integration_log()?),
            "mixed.integration.add" => {
                let a: prismqda_engine::IntegrationInput = parse(cmd, args)?;
                to_json(&self.project()?.integration_add(&a)?)
            }
            "mixed.integration.update" => {
                let a: IntegrationUpdate = parse(cmd, args)?;
                to_json(&self.project()?.integration_update(a.id, &a.entry)?)
            }
            "mixed.integration.remove" => {
                let a: RowId = parse(cmd, args)?;
                self.project()?.integration_remove(a.id)?;
                Ok(json!({}))
            }
            "mixed.qualitize.preview" => {
                let a: prismqda_engine::QualitizeSpec = parse(cmd, args)?;
                to_json(&self.project()?.qualitize_preview(&a)?)
            }
            "mixed.qualitize" => {
                let a: prismqda_engine::QualitizeSpec = parse(cmd, args)?;
                let coder = self.me()?;
                to_json(&self.project()?.qualitize(&a, coder)?)
            }
            "mixed.qualitize.take_back" => {
                let a: prismqda_engine::QualitizeUndo = parse(cmd, args)?;
                to_json(&self.project()?.qualitize_take_back(&a)?)
            }
            "variable.transform" => {
                let a: prismqda_engine::TransformSpec = parse(cmd, args)?;
                to_json(&self.project()?.transform(&a)?)
            }
            "variable.preview" => {
                let a: prismqda_engine::TransformSpec = parse(cmd, args)?;
                to_json(&self.project()?.transform_preview(&a)?)
            }
            "variable.recompute" => {
                let a: VariableName = parse(cmd, args)?;
                to_json(&self.project()?.recompute_variable(&a.name)?)
            }
            "variable.rule" => {
                let a: RuleArgs = parse(cmd, args)?;
                let ids = self.project()?.filter_cases(&a.formula)?;
                let total = self.project()?.case_data()?.cases.len();
                Ok(json!({ "matched": ids.len(), "total": total, "case_ids": ids }))
            }

            // ---- statistics ----
            "stats.variables" => to_json(&self.project()?.stat_variables()?),
            "stats.table" => {
                let a: TableArgs = parse(cmd, args)?;
                to_json(&self.project()?.stat_table(&a.keys, a.include_descendants)?)
            }
            "stats.run" => {
                let a: prismqda_engine::StatRequest = parse(cmd, args)?;
                to_json(&self.project()?.run_statistic(&a)?)
            }
            "stats.notebook" => to_json(&self.project()?.notebook()?),
            "stats.notebook.run" => {
                let a: NotebookRunArgs = parse(cmd, args)?;
                to_json(&self.project()?.notebook_run(&a.request, &a.section)?)
            }
            "stats.notebook.note" => {
                let a: NotebookNoteArgs = parse(cmd, args)?;
                to_json(&self.project()?.notebook_add_note(&a.title, &a.body, &a.section)?)
            }
            "stats.notebook.update" => {
                let a: NotebookUpdateArgs = parse(cmd, args)?;
                to_json(&self.project()?.notebook_update(a.id, a.note.as_deref(), a.section.as_deref(), a.title.as_deref())?)
            }
            "stats.notebook.remove" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.notebook_remove(a.id)?;
                Ok(json!({}))
            }
            "stats.notebook.move" => {
                let a: NotebookMoveArgs = parse(cmd, args)?;
                self.project()?.notebook_move(a.id, a.up)?;
                to_json(&self.project()?.notebook()?)
            }
            "stats.notebook.rerun" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.notebook_rerun(a.id)?)
            }
            "stats.notebook.rerun_all" => to_json(&self.project()?.notebook_rerun_all()?),
            "stats.notebook.previous" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.notebook_previous(a.id)?.map(|(at, result)| json!({ "ran_at": at, "result": result })))
            }

            // ---- merging a copy of the project ----
            "merge.preview" => {
                let a: MergeArgs = parse(cmd, args)?;
                to_json(&self.project()?.merge_preview_choosing(std::path::Path::new(a.path.trim()), a.passphrase.as_deref(), &a.choices)?)
            }
            "merge.run" => {
                let a: MergeArgs = parse(cmd, args)?;
                to_json(&self.project()?.merge_copy_choosing(std::path::Path::new(a.path.trim()), a.passphrase.as_deref(), &a.choices)?)
            }

            // ---- adjudication ----
            "adjudication.queue" => {
                let a: QueueArgs = parse(cmd, args)?;
                to_json(&self.project()?.disagreements(&a.coder_ids, a.blind)?)
            }
            "adjudication.decide" => {
                let a: DecideArgs = parse(cmd, args)?;
                let who = match a.coder_id {
                    Some(c) => c,
                    None => self.me()?,
                };
                to_json(&self.project()?.decide(a.unit_id, &a.code_ids, &a.reason, who)?)
            }
            "adjudication.list" => to_json(&self.project()?.decisions()?),
            "adjudication.final_summary" => to_json(&self.project()?.final_summary()?),
            "adjudication.region_queue" => to_json(&self.project()?.region_disagreements()?),
            "adjudication.decide_region" => {
                let a: DecideRegionArgs = parse(cmd, args)?;
                let who = match a.adjudicator_id {
                    Some(c) => c,
                    None => self.me()?,
                };
                to_json(&self.project()?.decide_region(a.key, a.accept, &a.reason, who)?)
            }
            "export.report" => {
                let a: ReportArgs = parse(cmd, args)?;
                let _scrub = self.scrub_scope()?;
                let bytes = prismqda_export::export_report(self.project()?, &prismqda_export::ReportOptions { title: a.title, sections: a.sections })?;
                write_export(&a.path, prismqda_export::Format::Docx, a.overwrite, &bytes)
            }
            "export.notebook" => {
                let a: NotebookArgs = parse(cmd, args)?;
                let format = prismqda_export::NotebookFormat::from_name(&a.format).ok_or_else(|| ApiError::new("invalid", format!("'{}' is not a format for the notebook: use docx, xlsx, or html", a.format)))?;
                let _scrub = match self.project.as_ref() {
                    Some(p) => prismqda_export::scrub_scope(p.export_scrubber()?),
                    None => prismqda_export::scrub_scope(None),
                };
                let bytes = prismqda_export::export_notebook(format, &a.notebook)?;
                write_export_as(&a.path, format.extension(), a.overwrite, &bytes)
            }
            "export.script" => {
                let a: ScriptArgs = parse(cmd, args)?;
                if !a.script.contains("{{data-file}}") {
                    return Err(ApiError::new("invalid", "the script does not say where its data is"));
                }
                let script_target = export_target(&a.path, "R", a.overwrite)?;
                let stem = script_target.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let data_name = format!("{stem}-data.csv");
                let data_target = script_target.with_file_name(&data_name);
                // Neither file is written when either would replace one that is there.
                check_target(&data_target, a.overwrite)?;
                let script = a.script.replace("{{data-file}}", &data_name);
                // The values of the cases leave the project with the list of names applied, while the switch is on.
                let data_text = match self.project.as_ref().map(|p| p.export_scrubber()).transpose()?.flatten() {
                    Some(s) => s.scrub(&a.data).into_owned(),
                    None => a.data.clone(),
                };
                let data = write_file(&data_target, data_text.as_bytes())?;
                let mut saved = write_file(&script_target, script.as_bytes())?;
                saved["data_path"] = data["path"].clone();
                saved["data_bytes"] = data["bytes"].clone();
                Ok(saved)
            }
            "export.picture" => {
                let a: PictureArgs = parse(cmd, args)?;
                let (extension, bytes) = match a.format.as_str() {
                    "png" => {
                        let bytes = prismqda_export::base64::decode(&a.data).ok_or_else(|| ApiError::new("invalid", "the picture is not valid base 64"))?;
                        if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
                            return Err(ApiError::new("invalid", "the picture is not a PNG"));
                        }
                        ("png", bytes)
                    }
                    "svg" => {
                        if !a.data.trim_start().starts_with("<svg") {
                            return Err(ApiError::new("invalid", "the drawing is not an SVG"));
                        }
                        ("svg", a.data.into_bytes())
                    }
                    other => return Err(ApiError::new("invalid", format!("'{other}' is not a format for a picture: use png or svg"))),
                };
                let mut saved = write_export_as(&a.path, extension, a.overwrite, &bytes)?;
                // A picture cannot have names taken out of it, so a list of names applied to exports does not reach it.
                if self.project.as_ref().map(|p| p.export_scrubber()).transpose()?.flatten().is_some() {
                    saved["names_not_replaced"] = json!(true);
                }
                Ok(saved)
            }
            "export.clip" => {
                // A coded stretch of a recording, cut by the window and sent as a WAV file (decision 182).
                let a: ClipFileArgs = parse(cmd, args)?;
                let bytes = prismqda_export::base64::decode(&a.data).ok_or_else(|| ApiError::new("invalid", "the clip is not valid base 64"))?;
                if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
                    return Err(ApiError::new("invalid", "the clip is not a WAV file"));
                }
                let mut saved = write_export_as(&a.path, "wav", a.overwrite, &bytes)?;
                // A sound cannot have names taken out of it, so a list of names applied to exports does not reach it.
                if self.project.as_ref().map(|p| p.export_scrubber()).transpose()?.flatten().is_some() {
                    saved["names_not_replaced"] = json!(true);
                }
                Ok(saved)
            }
            "export.segments" => {
                let a: ExportArgs = parse(cmd, args)?;
                let format = export_format(&a.format, false)?;
                let _scrub = self.scrub_scope()?;
                let bytes = prismqda_export::export_segments(self.project()?, format, &a.filter)?;
                write_export(&a.path, format, a.overwrite, &bytes)
            }

            // ---- reliability: samples and agreement ----
            "sample.draw" => {
                let a: SampleArgs = parse(cmd, args)?;
                to_json(&self.project()?.draw_sample(a.size, a.seed, &a.stratify, a.source_ids.as_deref())?)
            }
            "reliability.run" => {
                let a: AgreementRequest = parse(cmd, args)?;
                to_json(&self.project()?.agreement_run(&a)?)
            }
            "reliability.list" => to_json(&self.project()?.list_reliability_runs()?),
            "reliability.get" => {
                let a: Id = parse(cmd, args)?;
                to_json(&self.project()?.get_reliability_run(a.id)?)
            }
            "reliability.archive" => {
                let a: Id = parse(cmd, args)?;
                self.project()?.archive_reliability_run(a.id)?;
                Ok(json!({}))
            }

            // ---- the audit trail ----
            "events.list" => {
                let a: Limit = parse(cmd, args)?;
                to_json(&self.project()?.events(a.limit)?)
            }
            "events.annotate" => {
                let note = args.get("note").and_then(Value::as_str).unwrap_or("").to_string();
                self.project()?.annotate(&note)?;
                Ok(json!({}))
            }

            other => match self.call_deid(other, args.clone()) {
                Some(result) => result,
                None => match self.call_class(other, args.clone()) {
                    Some(result) => result,
                    None => match self.call_datatools(other, args) {
                        Some(result) => result,
                        None => Err(ApiError::new("unknown_command", format!("there is no command named '{other}'"))),
                    },
                },
            },
        }
    }

    fn import_options(req: &ImportRequest, bytes: &[u8]) -> ImportOptions {
        ImportOptions {
            name: req.name.clone(),
            folder: req.folder.clone(),
            source_type: req.source_type,
            unit_kind: req.unit_kind,
            interviewer_labels: req.interviewers.clone(),
            file_hash: Some(sha256_hex(bytes)),
            picture_variables: req.picture_variables,
        }
    }

    fn read_file(req: &ImportRequest) -> Result<(String, Vec<u8>), ApiError> {
        let path = PathBuf::from(&req.path);
        let bytes = fs::read(&path).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", path.display())))?;
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "source".to_string());
        Ok((name, bytes))
    }

    fn batch_for_table(table: &Table, req: &ImportRequest, opts: &ImportOptions) -> Result<(ImportBatch, SurveyMapping), ApiError> {
        if let Some(social) = &req.social_mapping {
            let batch = prismqda_import::social::from_table(table, social, opts)?;
            return Ok((batch, SurveyMapping::default()));
        }
        let mapping = req.mapping.clone().unwrap_or_else(|| SurveyMapping::from_guess(table));
        let batch = import_table(table, &mapping, opts)?;
        Ok((batch, mapping))
    }

    /// Shows what a file is and what importing it would do, without changing the project. A recording is shown through the
    /// captions beside it, with a `media` part that says which recording and which captions.
    fn import_inspect(&mut self, req: ImportRequest) -> ApiResult {
        let Some((kind, captions)) = media::recording_to_import(&req.path)? else { return self.import_inspect_file(req) };
        let Some(captions) = captions else { return self.import_inspect_recording(req, kind) };
        let media_path = req.path.clone();
        let size = fs::metadata(&media_path).map(|m| m.len()).unwrap_or(0);
        let mut shown = self.import_inspect_file(ImportRequest { path: captions.to_string_lossy().to_string(), ..req })?;
        shown["media"] = json!({
            "path": media_path,
            "kind": kind,
            "file_name": PathBuf::from(&media_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            "captions": captions.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            "size_bytes": size,
        });
        Ok(shown)
    }

    /// The document a recording with no captions would make: a description to be written, with stretches of the recording to be coded by
    /// time. `media.captions` is `null`, which is how a caller tells this from a recording read through its captions.
    fn import_inspect_recording(&mut self, req: ImportRequest, kind: &'static str) -> ApiResult {
        let (batch, size) = Self::recording_batch(&req, kind)?;
        let hash = batch.sources[0].original_file_hash.clone().unwrap_or_default();
        let duplicate = match &self.project {
            Some(p) => p.find_source_by_hash(&hash)?,
            None => None,
        };
        Ok(json!({
            "kind": "batch",
            "duplicate_of": duplicate,
            "summary": summarize(&batch),
            "picture": Value::Null,
            "media": {
                "path": req.path,
                "kind": kind,
                "file_name": PathBuf::from(&req.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                "captions": Value::Null,
                "size_bytes": size,
                "stands_alone": true,
            },
        }))
    }

    fn recording_batch(req: &ImportRequest, kind: &str) -> Result<(ImportBatch, u64), ApiError> {
        let (fingerprint, size) = prismqda_engine::recording_fingerprint(&req.path)?;
        let name = PathBuf::from(&req.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "recording".to_string());
        let opts = ImportOptions { name: req.name.clone(), folder: req.folder.clone(), ..ImportOptions::default() };
        Ok((prismqda_import::recording::parse_recording(&name, kind, size, &fingerprint, &opts), size))
    }

    fn import_inspect_file(&mut self, req: ImportRequest) -> ApiResult {
        let (name, bytes) = Self::read_file(&req)?;
        let opts = Self::import_options(&req, &bytes);
        let duplicate = match &self.project {
            Some(p) => p.find_source_by_hash(opts.file_hash.as_deref().unwrap_or(""))?,
            None => None,
        };
        match open_bytes(&name, &bytes, &opts)? {
            Opened::Batch(batch) => {
                // A picture says what its camera wrote into it, so the page can offer to keep it. The place is told only as present or not.
                let picture = if batch.images.is_empty() {
                    Value::Null
                } else {
                    let d = prismqda_engine::picture_details(&bytes);
                    json!({ "taken": d.taken, "camera": d.camera, "place": d.has_place() })
                };
                Ok(json!({ "kind": "batch", "duplicate_of": duplicate, "summary": summarize(&batch), "picture": picture }))
            }
            Opened::Table(table) => {
                let suggested = SurveyMapping::from_guess(&table);
                let summary = if req.mapping.is_some() || req.social_mapping.is_some() {
                    Some(summarize(&Self::batch_for_table(&table, &req, &opts)?.0))
                } else {
                    None
                };
                Ok(json!({
                    "kind": "table",
                    "duplicate_of": duplicate,
                    "table": table.preview(5),
                    "warnings": table.warnings,
                    "suggested_mapping": suggested,
                    "summary": summary,
                }))
            }
        }
    }

    /// Brings a file into the project. A recording comes in as the transcript read from the captions beside it, with the recording
    /// linked to it and each turn knowing when in the recording it is spoken.
    fn import_run(&mut self, req: ImportRequest) -> ApiResult {
        let Some((kind, captions)) = media::recording_to_import(&req.path)? else { return self.import_run_file(req) };
        let Some(captions) = captions else { return self.import_run_recording(req, kind) };
        let media_path = req.path.clone();
        let mut done = self.import_run_file(ImportRequest { path: captions.to_string_lossy().to_string(), ..req })?;
        let source_id = done["outcome"]["source_ids"][0].as_i64().ok_or_else(|| ApiError::new("internal", "the captions made no document"))?;
        let row = self.project()?.link_media(source_id, &media_path)?;
        done["media"] = to_json(&row)?;
        Ok(done)
    }

    /// Brings in a recording that has no captions beside it as a document of its own, with the recording linked to it as the thing that
    /// is coded (by stretches of time). The recording stays where it is.
    fn import_run_recording(&mut self, req: ImportRequest, kind: &'static str) -> ApiResult {
        let (batch, _) = Self::recording_batch(&req, kind)?;
        if !req.force {
            let hash = batch.sources[0].original_file_hash.clone().unwrap_or_default();
            if let Some(existing) = self.project()?.find_source_by_hash(&hash)? {
                return Err(ApiError::new(
                    "duplicate",
                    format!("this recording was already imported as source {existing}; import it again only if that is what you mean"),
                ));
            }
        }
        let summary = summarize(&batch);
        let outcome = self.project()?.apply_import(&batch)?;
        let source_id = outcome.source_ids.first().copied().ok_or_else(|| ApiError::new("internal", "the recording made no document"))?;
        let row = self.project()?.link_media_as(source_id, &req.path, Some(true))?;
        Ok(json!({ "outcome": outcome, "summary": summary, "media": row }))
    }

    fn import_run_file(&mut self, req: ImportRequest) -> ApiResult {
        let (name, bytes) = Self::read_file(&req)?;
        let opts = Self::import_options(&req, &bytes);
        if !req.force {
            if let Some(existing) = self.project()?.find_source_by_hash(opts.file_hash.as_deref().unwrap_or(""))? {
                return Err(ApiError::new(
                    "duplicate",
                    format!("this file was already imported as source {existing}; import it again only if that is what you mean"),
                ));
            }
        }
        let batch = match open_bytes(&name, &bytes, &opts)? {
            Opened::Batch(b) => b,
            Opened::Table(table) => Self::batch_for_table(&table, &req, &opts)?.0,
        };
        let summary = summarize(&batch);
        let outcome = self.project()?.apply_import(&batch)?;
        Ok(json!({ "outcome": outcome, "summary": summary }))
    }

    /// Takes a file as the new version of a document already in the project: the document keeps its codings, memos, case, and
    /// attributes, its text becomes the file's text, and every coding follows the words that survive. A backup copy of the
    /// project is saved first, because this change cannot be undone with Undo.
    fn import_revise(&mut self, req: ReviseRequest) -> ApiResult {
        let (name, bytes) = Self::read_file(&req.file)?;
        let opts = Self::import_options(&req.file, &bytes);
        let batch = match open_bytes(&name, &bytes, &opts)? {
            Opened::Batch(b) => b,
            Opened::Table(_) => {
                return Err(ApiError::new("invalid", "a spreadsheet becomes several documents and cases, so it cannot be the new version of one document"));
            }
        };
        if batch.sources.len() != 1 || !batch.cases.is_empty() {
            return Err(ApiError::new("invalid", format!("this file makes {} documents, so it cannot be the new version of one document", batch.sources.len())));
        }
        if !batch.images.is_empty() {
            return Err(ApiError::new("invalid", "a picture cannot be the new version of a document of words, and a picture's coded areas are tied to the picture they were drawn on; import it as a new document"));
        }
        let project = self.project()?;
        project.source_detail(req.source_id)?;
        if project.get_pdf(req.source_id)?.is_some() {
            return Err(ApiError::new(
                "invalid",
                "this document keeps its PDF, and the areas coded on its pages are tied to that file's pages, so another file cannot replace its words; import the new PDF as a new document",
            ));
        }
        if project.find_source_by_hash(opts.file_hash.as_deref().unwrap_or(""))? == Some(req.source_id) {
            return Err(ApiError::new("invalid", "this is the file the document came from, so there is nothing new to carry over"));
        }
        let (backup, backup_note) = project.backup_now_noted()?;
        let summary = summarize(&batch);
        let outcome = project.revise_source(req.source_id, &batch.sources[0])?;
        // A new version that is a PDF whose pages can be drawn: the document did not keep a PDF, so it keeps this one now.
        if let Some(a) = batch.pdfs.first() {
            let _ = project.attach_pdf(req.source_id, &a.pdf);
        }
        Ok(json!({ "outcome": outcome, "summary": summary, "backup": backup.display().to_string(), "backup_note": backup_note }))
    }

    /// Imports every readable file in a folder. One file at a time, each in its own transaction, so a file that
    /// cannot be read is reported and the rest still go in, and a crash loses at most the file being written.
    fn import_folder(&mut self, req: FolderImport) -> ApiResult {
        let root = PathBuf::from(&req.path);
        if !root.is_dir() {
            return Err(ApiError::new("invalid", format!("{} is not a folder", root.display())));
        }
        self.project()?; // a project must be open before anything is read

        let mut files: Vec<PathBuf> = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(dir) = pending.pop() {
            let entries = fs::read_dir(&dir).map_err(|e| ApiError::new("io", format!("cannot read the folder {}: {e}", dir.display())))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if req.recursive {
                        pending.push(path);
                    }
                } else {
                    files.push(path);
                }
            }
        }
        files.sort();

        let mut imported = Vec::new();
        let mut duplicates = Vec::new();
        let mut skipped = Vec::new();
        let mut failed = Vec::new();
        let (mut sources, mut units) = (0usize, 0usize);
        for path in files {
            let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let shown = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            // Word's own lock files and hidden files are not documents.
            if file_name.starts_with("~$") || file_name.starts_with('.') {
                continue;
            }
            let Some(format) = prismqda_import::Format::from_name(&file_name) else {
                // A picture of a kind that cannot be shown says what to do about it.
                let reason = match prismqda_engine::image_kind_of_name(&file_name) {
                    Err(why) => why,
                    Ok(_) => "not a kind of file PrismQDA reads".to_string(),
                };
                skipped.push(json!({ "path": shown, "reason": reason }));
                continue;
            };
            if !req.include_tables && matches!(format, prismqda_import::Format::Csv | prismqda_import::Format::Tsv | prismqda_import::Format::Xlsx | prismqda_import::Format::Sav) {
                skipped.push(json!({ "path": shown, "reason": "a spreadsheet or data file needs its columns set up; import it on its own" }));
                continue;
            }
            // Subfolders become folders of sources, beneath the folder the person named.
            let sub = path.parent().and_then(|d| d.strip_prefix(&root).ok()).map(|d| d.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            let folder = [req.folder.trim_matches('/'), sub.trim_matches('/')].iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join("/");
            let one = ImportRequest {
                path: path.to_string_lossy().to_string(),
                name: None,
                folder,
                source_type: req.source_type,
                unit_kind: req.unit_kind,
                interviewers: req.interviewers.clone(),
                mapping: None,
                social_mapping: None,
                force: req.force,
                picture_variables: req.picture_variables,
            };
            match self.import_run(one) {
                Ok(done) => {
                    let ids = done["outcome"]["source_ids"].clone();
                    sources += ids.as_array().map_or(0, Vec::len);
                    units += done["outcome"]["units_created"].as_u64().unwrap_or(0) as usize;
                    imported.push(json!({ "path": shown, "source_ids": ids, "units": done["outcome"]["units_created"] }));
                }
                Err(e) if e.code == "duplicate" => duplicates.push(json!({ "path": shown, "message": e.message })),
                Err(e) => failed.push(json!({ "path": shown, "code": e.code, "message": e.message })),
            }
        }
        Ok(json!({
            "imported": imported,
            "duplicates": duplicates,
            "skipped": skipped,
            "failed": failed,
            "sources_created": sources,
            "units_created": units,
        }))
    }
}

fn export_format(name: &str, codebook: bool) -> Result<prismqda_export::Format, ApiError> {
    let format = prismqda_export::Format::from_name(&name.to_lowercase())
        .ok_or_else(|| ApiError::new("bad_args", format!("the format '{name}' should be xlsx, docx, csv{}", if codebook { ", or qdc" } else { "" })))?;
    if !codebook && format == prismqda_export::Format::Qdc {
        return Err(ApiError::new("invalid", "the QDC format holds a codebook, not coded segments"));
    }
    Ok(format)
}

/// Writes an export to the path (adding the file ending if it is missing), without replacing an existing file unless asked to.
fn write_export(path: &str, format: prismqda_export::Format, overwrite: bool, bytes: &[u8]) -> ApiResult {
    write_export_as(path, format.extension(), overwrite, bytes)
}

/// Where an export goes: the name asked for, with the ending the kind of file needs added when it is missing. A file already there
/// is never replaced unless `overwrite` says so, and the folder has to exist.
fn export_target(path: &str, wanted: &str, overwrite: bool) -> Result<PathBuf, ApiError> {
    let mut target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err(ApiError::new("invalid", "say where to save the file"));
    }
    if target.extension().and_then(|e| e.to_str()).is_none_or(|e| !e.eq_ignore_ascii_case(wanted)) {
        let mut name = target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        name.push('.');
        name.push_str(wanted);
        target.set_file_name(name);
    }
    check_target(&target, overwrite)?;
    Ok(target)
}

fn check_target(target: &std::path::Path, overwrite: bool) -> Result<(), ApiError> {
    if target.exists() && !overwrite {
        return Err(ApiError::new("already_exists", format!("{} already exists, so nothing was saved; choose another name", target.display())));
    }
    if let Some(dir) = target.parent().filter(|d| !d.as_os_str().is_empty()) {
        if !dir.is_dir() {
            return Err(ApiError::new("io", format!("the folder {} does not exist", dir.display())));
        }
    }
    Ok(())
}

/// Writes an export under the name asked for, with the ending the kind of file needs added when it is missing.
fn write_export_as(path: &str, wanted: &str, overwrite: bool, bytes: &[u8]) -> ApiResult {
    let target = export_target(path, wanted, overwrite)?;
    write_file(&target, bytes)
}

fn write_file(target: &std::path::Path, bytes: &[u8]) -> ApiResult {
    // Write beside the target and rename, so a failure part way never leaves a half-written file under the real name.
    let mut temp_name = target.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    temp_name.push(".part");
    let temp = target.with_file_name(temp_name);
    fs::write(&temp, bytes).map_err(|e| ApiError::new("io", format!("cannot write {}: {e}", target.display())))?;
    if let Err(e) = fs::rename(&temp, target) {
        let _ = fs::remove_file(&temp);
        return Err(ApiError::new("io", format!("cannot save {}: {e}", target.display())));
    }
    Ok(json!({ "path": target.display().to_string(), "bytes": bytes.len() }))
}

fn summarize(batch: &ImportBatch) -> Value {
    json!({
        "sources": batch.sources.iter().map(|s| json!({
            "name": s.name,
            "source_type": s.source_type.as_str(),
            "characters": s.text.chars().count(),
            "units": s.units.len(),
            "default_unit_kind": s.default_unit_kind.as_str(),
            "format_runs": s.runs.len(),
            // The kind of file it came from, for those formats that say (reference libraries, e-mail, saved pages, feeds).
            "format": s.settings.get("format").cloned().unwrap_or(Value::Null),
            // The first few hundred characters, so the person can see what the file turned into before it goes in.
            "preview": s.text.chars().take(400).collect::<String>(),
            "warnings": s.warnings,
        })).collect::<Vec<_>>(),
        "cases": batch.cases.len(),
        "case_attributes": batch.case_attributes.len(),
    })
}
