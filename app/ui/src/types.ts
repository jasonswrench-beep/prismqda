// Types for every command in docs/API.md. Offsets (`start`, `end`) always count Unicode scalar
// values, never JavaScript UTF-16 units: convert in src/offsets.ts and nowhere else.

export type CodingMode = 'free_span' | 'unit';
export type SourceType = 'document' | 'pdf' | 'survey' | 'transcript' | 'social' | 'other';
export type UnitKind = 'sentence' | 'paragraph' | 'speaker_turn' | 'survey_response' | 'post' | 'comment';
export type Level = 'nominal' | 'ordinal' | 'interval' | 'ratio' | 'date' | 'text';
export type CodeStatus = 'active' | 'merged' | 'retired';
export type CodeKind = 'code' | 'theme';
export type Origin = 'human' | 'ai_suggestion_accepted' | 'ai_applied';
export type RunKind = 'heading' | 'bold' | 'italic' | 'list_item' | 'table_cell' | 'page';
export type MemoKind = 'analytic' | 'reflexive' | 'method_decision' | 'todo';
export type MemoScope = 'project' | 'source' | 'code' | 'coding' | 'case' | 'phase' | 'standalone';

/** The error every failed command throws. `code` is stable, `message` is for people. */
export interface ApiErrorShape {
  code: string;
  message: string;
}

export interface PingResult {
  name: string;
  version: string;
}

export interface ProjectInfo {
  name: string;
  path: string;
  project_uid: string;
  method_template: string;
  /** The research question given when the project was made; empty when there was none. */
  research_questions: string;
  coding_mode: CodingMode;
  schema_version: number;
  read_only: boolean;
  sources: number;
  units: number;
  codes: number;
  coders: number;
  codings: number;
  events: number;
  notices: string[];
  /** The coder id of the person using the project; null when the project is read-only. */
  me: number | null;
  /** The sample this project was copied from, or null for an ordinary project. */
  sample_id?: string | null;
  /** `packet` when AI prompt packets can be built, `off` when this project does not allow them. */
  ai_mode?: 'packet' | 'off';
  /** True when every page of the project file is encrypted (decision 157). */
  encrypted?: boolean;
  /** Who this project is a packet for (decision 161); null or absent for an ordinary project. */
  packet?: CoderPacketInfo | null;
  /** True when the offline lock is on (decision 165): no prompt packet can be built for an AI service. */
  offline_lock?: boolean;
}

/** A project made for another coder to work in: who it is for and where it came from. */
export interface CoderPacketInfo {
  coder_name: string;
  coder_local_id: string;
  from_project: string;
  made_at: string;
}

/** What a packet for a coder holds. */
export interface CoderPacketReport {
  path: string;
  coder: string;
  coder_known: boolean;
  documents: string[];
  codes: number;
  codings: number;
  left_out: string[];
}

/** What became of one student's packet (decision 183). */
export interface ClassPacketOutcome {
  student: string;
  path: string;
  made: boolean;
  error: string | null;
}

export interface ClassPacketsReport {
  folder: string;
  packets: ClassPacketOutcome[];
  made: number;
  /** Names left out because the same name came twice. */
  repeated: string[];
  documents: string[];
  codes: number;
}

export type ClassCollectStatus = 'merged' | 'partly' | 'nothing_new' | 'not_a_packet' | 'other_study' | 'failed';

export interface ClassCollectOutcome {
  file: string;
  student: string | null;
  status: ClassCollectStatus;
  message: string;
  codings_added: number;
  codings_already_here: number;
  codings_skipped: number;
  /** Documents whose words here are not the words in the packet, so no coding on them was brought in. */
  documents_in_conflict: string[];
  /** Codes the student added to the codebook in the packet. */
  codes_added: string[];
}

export interface ClassCollectReport {
  folder: string;
  files: ClassCollectOutcome[];
  packets: number;
  codings_added: number;
  backup: string | null;
}

/** How one student's coding compares with the instructor's. */
export interface ClassStudentReview {
  coder_id: number | null;
  label: string;
  passages_coded: number;
  same: number;
  partly: number;
  different: number;
  left_uncoded: number;
  extra: number;
  found: number;
  missed: number;
  added: number;
  share_same: number | null;
  share_found: number | null;
  share_added: number | null;
}

export interface ClassCodeReview {
  code_id: number;
  name: string;
  instructor_passages: number;
  mean_found: number | null;
  added: number;
  mean_kappa: number | null;
  kappa_students: number;
}

export interface ClassHardPassage {
  unit_id: number;
  source_id: number;
  source_name: string;
  start: number;
  end: number;
  text: string;
  instructor_codes: { id: number; name: string }[];
  students_same: number;
  students_total: number;
  added_codes: [{ id: number; name: string }, number][];
  missed_codes: [{ id: number; name: string }, number][];
}

export interface ClassReview {
  instructor: { id: number; name: string; role: string };
  anonymous: boolean;
  instructor_passages: number;
  passages_compared: number;
  students: ClassStudentReview[];
  by_code: ClassCodeReview[];
  hardest: ClassHardPassage[];
  notes: string[];
}

export interface ClassReviewArgs {
  instructor_id?: number;
  student_ids?: number[];
  source_ids?: number[];
  anonymous?: boolean;
  hardest?: number;
}

/** What a list of documents knows about a document that is a picture. */
export interface SourceImage {
  mime: string;
  file_name: string;
  /** Pixels, as the picture is shown (a camera's turn marker is applied). */
  width: number;
  height: number;
  size_bytes: number;
}

/**
 * An area of a picture. Every number is a fraction of the picture as shown: 0 is the left or top edge, 1 the right or bottom.
 * A rectangle or an ellipse is given by its top left corner and its size; a polygon by its corners in order. A `span` is not an
 * area but a stretch of a recording: from `start_ms` to `end_ms`, in whole milliseconds from the start of the recording.
 */
export type RegionShape =
  | { shape: 'rectangle'; x: number; y: number; w: number; h: number }
  | { shape: 'ellipse'; x: number; y: number; w: number; h: number }
  | { shape: 'polygon'; points: [number, number][] }
  | { shape: 'span'; start_ms: number; end_ms: number };

/** The shapes that are drawn on a picture or a page: every shape but a stretch of a recording. */
export type AreaShape = Exclude<RegionShape, { shape: 'span' }>;

/** A stretch of a recording. */
export type SpanShape = Extract<RegionShape, { shape: 'span' }>;

/** One code given to one area of a picture or of a page of a PDF, as the engine lists it. */
export type RegionRow = RegionShape & {
  id: number;
  source_id: number;
  /** The page of the PDF the area is on, counting from 1; always 1 for a picture. */
  page: number;
  code_id: number;
  coder_id: number;
  origin: Origin;
  layer: string;
  rationale: string;
  rating: number | null;
  created_at: string;
  /** The area in words. */
  text: string;
};

export interface SourceRow {
  id: number;
  name: string;
  source_type: SourceType;
  folder: string;
  characters: number;
  default_unit_kind: UnitKind;
  text_revision: number;
  units: number;
  codings: number;
  do_not_share: boolean;
  created_at: string;
  /** Set when the document is a picture. */
  image?: SourceImage | null;
  /** Set when the document keeps the PDF it was read from, so its pages can be drawn and areas of them coded. */
  pdf?: SourcePdf | null;
  /** Set when a recording is linked to the document, so stretches of it can be coded. */
  recording?: SourceRecording | null;
}

/** What a list of documents knows about a document that has a recording linked. */
export interface SourceRecording {
  kind: 'audio' | 'video';
  file_name: string;
  size_bytes: number;
  /** True for a recording imported without captions: its document's text is a description of the recording. */
  stands_alone: boolean;
}

/** What a list of documents knows about a document that keeps its PDF. */
export interface SourcePdf {
  file_name: string;
  page_count: number;
  size_bytes: number;
}

/** The size of one page of a PDF as it is drawn, in points (1/72 inch). */
export interface PdfPageSize {
  page: number;
  width: number;
  height: number;
}

/** A kept PDF with the size of every page. */
export interface PdfRow extends SourcePdf {
  source_id: number;
  created_at: string;
  pages: PdfPageSize[];
}

export interface FormatRun {
  start: number;
  end: number;
  kind: RunKind;
  level?: number;
  page?: number;
}

export interface ImportWarning {
  kind: string;
  message: string;
  location?: string | null;
}

export interface SourceDetail {
  id: number;
  name: string;
  text: string;
  formatting_runs: FormatRun[];
  import_report: ImportWarning[];
  text_revision: number;
}

export interface UnitRow {
  id: number;
  source_id: number;
  kind: UnitKind;
  start: number;
  end: number;
  speaker: string | null;
  parent_id: number | null;
  excluded: boolean;
  /** Where in the recording the unit starts and ends, in milliseconds, for a unit made from captions. */
  time_start_ms?: number | null;
  time_end_ms?: number | null;
}

/** A recording linked to a document. The project keeps where it is on this computer, not the recording. */
export interface MediaRow {
  source_id: number;
  kind: 'audio' | 'video';
  path: string;
  file_name: string;
  size_bytes: number;
  fingerprint: string;
  /** The last time any unit names, in milliseconds. */
  duration_ms: number | null;
  created_at: string;
  /** True for a recording imported without captions, which is the document itself; false for one linked to a transcript. */
  stands_alone: boolean;
  /** False when the recording is not at its path now. */
  found: boolean;
  /** False when the file at the path is not the size it was when linked. */
  unchanged: boolean;
}

export interface AttributeValueRow {
  target_id: number;
  attribute: string;
  level: Level;
  value: string | null;
}

export interface CaseRow {
  id: number;
  name: string;
  sources: number;
  units: number;
  /** The documents that belong to the case (listed by newer engines). */
  source_ids?: number[];
}

export interface CodeRow {
  id: number;
  name: string;
  parent_id: number | null;
  kind: CodeKind;
  color: string;
  definition: string;
  inclusion_criteria: string;
  exclusion_criteria: string;
  status: CodeStatus;
  merged_into_id: number | null;
  version: number;
  sort_order: number;
  do_not_share: boolean;
  /** The mutually exclusive group the code is in ("" for none): one coder cannot give two codes of a group to overlapping words. */
  exclusive_group: string;
  /** True when codings of the code can be rated 1 to 5; the label and the two ends describe the scale. */
  rated: boolean;
  rating_label: string;
  rating_low: string;
  rating_high: string;
  /** False when no AI suggestion may use this code: it is left out of every prompt packet. */
  ai_suggestible?: boolean;
  /** For a theme: where it stands in its development (decision 167). */
  theme_stage?: ThemeStage;
  usage: number;
}

export type ThemeStage = 'candidate' | 'reviewed' | 'defined';

/** A code under a theme, or under none, with how much it holds. */
export interface ThemeCode {
  id: number;
  name: string;
  color: string;
  parent_id: number | null;
  passages: number;
  documents: number;
}

/** A passage that shows a theme. */
export interface ThemeQuote {
  coding_id: number;
  source_id: number;
  source_name: string;
  start: number;
  end: number;
  code_id: number;
  code_name: string;
  text: string;
  /** True when the passage was marked as an example of its code. */
  example: boolean;
}

export interface ThemeCard {
  id: number;
  name: string;
  color: string;
  parent_id: number | null;
  definition: string;
  inclusion_criteria: string;
  exclusion_criteria: string;
  stage: ThemeStage;
  version: number;
  codes: ThemeCode[];
  sub_themes: number[];
  passages: number;
  documents: number;
  cases: number;
  quotes: ThemeQuote[];
  memos: number;
}

/** The text to take to a chatbot to ask for candidate themes, and the run that records it (decision 168). */
export interface ThemePacket {
  run_id: number;
  prompt: string;
  characters: number;
  codes: { id: number; name: string }[];
  left_out: string[];
}

/** A theme an AI suggested that passed the check. */
export interface ThemeSuggestion {
  index: number;
  name: string;
  definition: string;
  code_ids: number[];
  code_names: string[];
  problems: string[];
  status: 'pending' | 'accepted' | 'rejected';
  theme_id: number | null;
}

export interface ThemeAnswerOutcome {
  read: number;
  suggestions: ThemeSuggestion[];
  rejected: { number: number; passage: string; code: string; why: string }[];
}

export interface ThemeRun {
  run_id: number;
  model: string;
  answered: boolean;
  suggestions: ThemeSuggestion[];
  rejected: { number: number; passage: string; code: string; why: string }[];
}

export interface ThemeOverview {
  themes: ThemeCard[];
  unassigned: ThemeCode[];
}

export interface CodeEditArgs {
  name?: string;
  color?: string;
  definition?: string;
  inclusion_criteria?: string;
  exclusion_criteria?: string;
  theme?: boolean;
  /** Where a theme stands: candidate, reviewed, or defined. */
  theme_stage?: ThemeStage;
  do_not_share?: boolean;
  ai_suggestible?: boolean;
  exclusive_group?: string;
  rated?: boolean;
  rating_label?: string;
  rating_low?: string;
  rating_high?: string;
  /** With rated: false, also clears the ratings already given (otherwise the engine refuses while any exist). */
  clear_ratings?: boolean;
}

/** A method template a project can start from. */
export interface TemplateRow {
  id: string;
  name: string;
  summary: string;
  seed_codes: { name: string; definition: string }[];
  phases: { id?: string; title?: string; name?: string }[];
}

/** What the launcher learns about a file without opening it as a project. */
/** A small text file as read by text.read; `encoding` says how the bytes were understood. */
export interface TextFile {
  name: string;
  text: string;
  bytes: number;
  encoding: 'utf-8' | 'utf-16' | 'windows-1252';
}

export interface FileStat {
  exists: boolean;
  is_file: boolean;
  name: string;
  modified: string | null;
  size: number;
  /** True when the file is an encrypted project, so it will ask for a passphrase. */
  encrypted?: boolean;
}

/** What turning encryption on or off, or changing the passphrase, did. */
export interface EncryptionOutcome {
  encrypted: boolean;
  old_file_erased: boolean;
  /** Earlier backups that the project's passphrase does not protect (plain, or under an earlier passphrase). */
  stale_backups: number;
  backup_folder: string;
  cipher: string;
}

export interface StaleBackup {
  path: string;
  why: 'plain' | 'other_passphrase';
  bytes: number;
}

/** A bundled sample study. */
export interface SampleRow {
  id: string;
  name: string;
  description: string;
  documents: number;
  codes: number;
  codings: number;
}

/** One frozen version of the codebook: every code as it was then. */
export interface CodebookSnapshot {
  version: number;
  note: string;
  created_at: string;
  codes: {
    id: number;
    name: string;
    parent_id: number | null;
    kind: string;
    color: string;
    definition: string;
    inclusion_criteria: string;
    exclusion_criteria: string;
    status: string;
    /** Properties came with format 3; a version saved before then does not have them. */
    exclusive_group?: string;
    rated?: boolean;
    rating_label?: string;
    rating_low?: string;
    rating_high?: string;
  }[];
}

export interface CodebookVersion {
  version: number;
  note: string;
  codes: number;
  created_at: string;
}

export interface CoderRow {
  id: number;
  name: string;
  role: 'human' | 'model';
  local_id: string;
  provider: string | null;
  model_name: string | null;
  model_version: string | null;
}

export interface CodingRow {
  id: number;
  source_id: number;
  start: number;
  end: number;
  code_id: number;
  coder_id: number;
  origin: Origin;
  layer: string;
  rationale: string;
  unit_id: number | null;
  /** When the coding was made. A coding just made on screen has none until the list is loaded again. */
  created_at?: string;
  /** An analytic rating from 1 to 5; null or missing means not rated, which is not the same as any number. */
  rating?: number | null;
  /** The AI run whose suggestion this coding came from, for a coding made by accepting an AI suggestion. */
  ai_run_id?: number | null;
  /** Set when the coding is of an area of a picture. Such a coding is numbered with its area's number made negative (area 7 is -7) and has no place in text (start and end are 0). */
  region?: RegionShape;
  /** For a coded area: the page of the PDF it is on (1 for a picture). */
  page?: number;
}

/** One entry of the audit trail for a coding. */
export interface CodingHistoryEvent {
  ts: string;
  actor: string;
  action: string;
  before: Record<string, unknown> | null;
  after: Record<string, unknown> | null;
}

export interface MemoRow {
  id: number;
  kind: MemoKind;
  scope: MemoScope;
  target_id: number | null;
  phase: string | null;
  title: string;
  body: string;
  created_at: string;
  updated_at: string;
}

export interface EventRow {
  id: number;
  ts: string;
  actor: string;
  session: string;
  action: string;
  target_type: string | null;
  target_id: number | null;
  before: Record<string, unknown> | null;
  after: Record<string, unknown> | null;
}

// ---- importing ----

export interface ColumnPreview {
  name: string;
  label: string | null;
  guessed_level: Level;
  looks_like: 'text' | 'number' | 'date' | 'empty';
  distinct: number;
  empty: number;
  mean_words: number;
  samples: string[];
  suggested_role: 'id' | 'text' | 'attribute';
}

export interface TablePreview {
  name: string;
  row_count: number;
  columns: ColumnPreview[];
}

export interface AttributeColumn {
  name: string;
  level?: Level | null;
}

export interface SurveyMapping {
  id_column?: string | null;
  text_columns: string[];
  attribute_columns?: AttributeColumn[];
  unit_id_column?: string | null;
  unit_attribute_columns?: AttributeColumn[];
}

export interface SummarySource {
  name: string;
  source_type: SourceType;
  characters: number;
  units: number;
  default_unit_kind: UnitKind;
  format_runs: number;
  /** The kind of file for formats that say: ris, bibtex, csl-json, eml, mbox, mhtml, feed. Otherwise null. */
  format?: string | null;
  /** The first few hundred characters of the text, so what the file turned into can be seen before it goes in. */
  preview?: string;
  warnings: ImportWarning[];
}

export interface ImportSummary {
  sources: SummarySource[];
  cases: number;
  case_attributes: number;
}

/** Which of a photograph's own facts to keep as variables of a case made for the picture. */
export interface PictureVariables {
  when?: boolean;
  camera?: boolean;
  place?: boolean;
}

/** What a photograph says about itself. The place is told only as present or not; its numbers are kept only when asked for. */
export interface PictureFacts {
  taken: string | null;
  camera: string | null;
  place: boolean;
}

/** Every coefficient for one table of ratings, as the engine works them out. A number is null when it cannot be worked out for these ratings. */
export interface AgreementCoefficients {
  units: number;
  units_compared: number;
  coders: number;
  percent_agreement: number | null;
  cohen_kappa: number | null;
  scott_pi: number | null;
  fleiss_kappa: number | null;
  fleiss_units: number;
  brennan_prediger: number | null;
  gwet_ac1: number | null;
  krippendorff_alpha: number | null;
}

export interface AgreementInterval {
  low: number;
  high: number;
  level: number;
  replications: number;
  undefined: number;
}

/** Agreement about one code that coders may apply or not. */
export interface AgreementCodeDetail {
  agreement: AgreementCoefficients;
  both: number;
  first_only: number;
  second_only: number;
  neither: number;
  overlap: number | null;
  prevalence_index: number | null;
  bias_index: number | null;
  pabak: number | null;
}

export interface AgreementCodeReport {
  code_id: number;
  name: string;
  detail: AgreementCodeDetail;
  below_target: boolean;
}

export interface AgreementCoder {
  id: number;
  name: string;
  role: 'human' | 'model';
  model_name: string | null;
  model_version: string | null;
}

export interface AgreementReport {
  run_id: number | null;
  mode: 'single_label' | 'per_code';
  reporting_term: string;
  coders: AgreementCoder[];
  units_in_sample: number;
  /** Across all codes (one code per unit), or null for a code-by-code run. */
  overall: AgreementCoefficients | null;
  intervals: Record<string, AgreementInterval>;
  target: number;
  /** Codes under the target come first. */
  by_code: AgreementCodeReport[];
  /** For two coders in a one-code-per-unit run: the first coder's codes down the side, the second's across the top. */
  confusion: number[][] | null;
  confusion_codes: string[];
  disagreements: number;
  notes: string[];
  report_text: string;
}

export interface AgreementRunRow {
  id: number;
  created_at: string;
  reporting_term: string;
  units: number;
  archived: boolean;
}

export interface AgreementRunRecord {
  id: number;
  report: AgreementReport;
  text: string;
}

export interface AgreementArgs {
  coder_ids: number[];
  unit_ids?: number[];
  mode?: 'single_label' | 'per_code';
  reporting_term?: 'intercoder_reliability' | 'concordance';
  bootstrap_replications?: number;
  seed?: number;
  save?: boolean;
  areas?: boolean;
}

export interface DrawnSample {
  seed: number;
  size: number;
  pool: number;
  unit_ids: number[];
}

/** A code named in a decision or in what a coder gave a unit. */
export interface AdjudicationCode {
  id: number;
  name: string;
}

/** What one coder gave a unit. In a blind queue there is no coder_id or role, and the label is "Coder A" and so on. */
export interface AdjudicationCoder {
  coder_id: number | null;
  label: string;
  role: 'human' | 'model' | null;
  codes: AdjudicationCode[];
}

export interface AdjudicationDecision {
  id: number;
  unit_id: number;
  adjudicator_id: number;
  adjudicator_name: string;
  reason: string;
  /** The codes the unit carries now; none at all says that no code applies. */
  codes: AdjudicationCode[];
  created_at: string;
}

/** A unit that two or more of the chosen coders coded differently. */
export interface AdjudicationItem {
  unit_id: number;
  source_id: number;
  source_name: string;
  start: number;
  end: number;
  text: string;
  by_coder: AdjudicationCoder[];
  decision: AdjudicationDecision | null;
}

/** What to count in Text Analytics. */
export interface WordSpecArgs {
  source_ids?: number[];
  case_id?: number;
  /** Only the words inside passages given one of these codes (a code includes its sub-codes). */
  code_ids?: number[];
  leave_out_common?: boolean;
  min_length?: number;
  min_count?: number;
  limit?: number;
}

export interface WordCount {
  word: string;
  count: number;
  /** In how many documents the word is used. */
  documents: number;
}

export interface WordFrequencies {
  words: WordCount[];
  total_words: number;
  counted_words: number;
  distinct_words: number;
  documents: number;
}

// ---------------------------------------------------------------- statistics

export type StatMeasure = 'nominal' | 'ordinal' | 'scale' | 'date' | 'text';
export type StatVariableKind = 'attribute' | 'code_present' | 'code_count';

export interface StatCategory {
  value: string;
  label: string;
  count: number;
}

export interface StatVariable {
  key: string;
  name: string;
  kind: StatVariableKind;
  code_id: number | null;
  measure: StatMeasure;
  valid: number;
  missing: number;
  not_a_number: number;
  numeric: boolean;
  distinct: number;
  categories: StatCategory[];
}

export interface StatVariables {
  cases: number;
  cases_with_documents: number;
  variables: StatVariable[];
}

export type StatProcedure =
  | 'describe'
  | 'one_sample_t'
  | 'independent_t'
  | 'paired_t'
  | 'one_way_anova'
  | 'chi_square'
  | 'fisher'
  | 'pearson'
  | 'spearman'
  | 'kendall'
  | 'mann_whitney'
  | 'wilcoxon'
  | 'kruskal_wallis'
  | 'linear_regression'
  | 'logistic_regression'
  | 'curve_fit'
  | 'shapiro_wilk'
  | 'ks_normal'
  | 'runs'
  | 'binomial'
  | 'goodness_of_fit'
  | 'sign'
  | 'friedman'
  | 'ks_two'
  | 'median_test'
  | 'moses'
  | 'mcnemar'
  | 'ordinal_association'
  | 'partial_correlation'
  | 'two_way_anova'
  | 'cronbach_alpha';

export type StatRunsCut = 'median' | 'mean' | 'value';

export type StatAdjustment = 'none' | 'bonferroni' | 'holm' | 'benjamini_hochberg';

export interface StatOptions {
  /** Count each case as many times as this variable (of whole numbers, 0 or more) says. */
  weight?: string;
  level?: number;
  mu?: number;
  equal_variances?: boolean;
  adjustment?: StatAdjustment;
  /** A test made for pairs of groups after an analysis of variance, used instead of the adjusted t tests. */
  posthoc?: StatPostHoc;
  groups?: string[];
  case_ids?: number[];
  include_descendants?: boolean;
  /** For a regression: the step each predictor is entered in, one for each predictor. */
  steps?: number[];
  /** For a regression: the category of a categorical predictor that the others are compared with, by variable key. */
  references?: Record<string, string>;
  /** For a logistic regression: the value of the outcome that is modelled (the one the odds are of). */
  event?: string;
  /** For a curve fit, the curves to fit (all of them when absent). */
  curves?: StatCurveKind[];
  /** `analysis` runs the whole procedure again on each resample; `cluster` (a variable's key) resamples whole groups of cases, and is the whole analysis too. */
  bootstrap?: { resamples?: number; seed?: number; analysis?: boolean; cluster?: string };
  /** Only the cases for which this formula (R's syntax) is true. */
  filter?: string;
  /** The standard deviation of the normal distribution of the Kolmogorov-Smirnov test (with `mu` as its mean); the sample's own when absent. */
  sd?: number;
  /** The category counted as a success in the binomial test. */
  success?: string;
  /** The share the binomial test compares with. */
  proportion?: number;
  /** The shares of the categories the one-sample chi-square compares with, by value (equal shares when absent). */
  expected?: Record<string, number>;
  /** The control group of the Moses test, by value. */
  control?: string;
  /** How many control cases the Moses test trims from each end. */
  trim?: number;
  /** Where the runs test cuts a number in two. */
  cut?: StatRunsCut;
}

export interface StatRequest {
  procedure: StatProcedure;
  variables: string[];
  options?: StatOptions;
}

export interface StatGroup {
  value: string;
  label: string;
  n: number;
  mean: number | null;
  median: number | null;
  case_ids: number[];
}

export type StatStatus = 'ok' | 'caution' | 'problem' | 'info';

export interface StatAssumption {
  id: string;
  status: StatStatus;
  group: string | null;
  numbers: Record<string, number>;
}

/** A number the engine could not write as a finite one (an odds ratio with a zero cell) arrives as null. */
export type StatNumber = number | null;

export interface StatDescriptives {
  n: number;
  sum: number;
  mean: StatNumber;
  variance: StatNumber;
  sd: StatNumber;
  se: StatNumber;
  ci_mean: [number, number] | null;
  level: number;
  min: StatNumber;
  q1: StatNumber;
  median: StatNumber;
  q3: StatNumber;
  max: StatNumber;
  iqr: StatNumber;
  skewness: StatNumber;
  se_skewness: StatNumber;
  kurtosis: StatNumber;
  se_kurtosis: StatNumber;
  outliers: number;
}

export interface StatTTest {
  kind: 'one_sample' | 'student' | 'welch' | 'paired';
  t: number;
  df: number;
  p: number;
  estimate: number;
  se: number;
  level: number;
  ci: [number, number];
  groups: { n: number; mean: number; sd: number }[];
  d: number;
  d_ci: [number, number];
  g: number;
  g_ci: [number, number];
}

export interface StatFTest {
  f: number;
  df1: number;
  df2: number;
  p: number;
}

export interface StatOneWay {
  groups: { n: number; mean: number; sd: number | null }[];
  n: number;
  ss_between: number;
  ss_within: number;
  ss_total: number;
  df_between: number;
  df_within: number;
  ms_between: number;
  ms_within: number;
  test: StatFTest;
  eta_squared: number;
  omega_squared: number;
  welch: StatFTest | null;
  levene: StatFTest | null;
}

export type StatPostHoc = 'tukey' | 'games_howell' | 'scheffe';

export interface StatPairwise {
  a: number;
  b: number;
  difference: number;
  se: number;
  t: number;
  df: number;
  p: number;
  p_adjusted: number;
  ci: [number, number];
}

export interface StatChiSquare {
  rows: number;
  cols: number;
  n: number;
  expected: number[][];
  chi2: number;
  df: number;
  p: number;
  yates: [number, number] | null;
  g2: number;
  g2_p: number;
  phi: StatNumber;
  cramers_v: number;
  contingency_c: number;
  standardized_residuals: number[][];
  adjusted_residuals: number[][];
  smallest_expected: number;
  share_expected_below_5: number;
}

export interface StatFisher {
  p: number;
  odds_ratio_sample: StatNumber;
  odds_ratio: StatNumber;
  level: number;
  ci: [StatNumber, StatNumber];
}

export interface StatCorrelation {
  kind: 'pearson' | 'spearman' | 'kendall';
  n: number;
  r: number;
  statistic: StatNumber;
  df: StatNumber;
  p: number;
  exact: boolean;
  /** Spearman's p-value is R's series approximation to the exact test (no ties, 10 to 1,290 pairs). */
  edgeworth: boolean;
  ci: [number, number] | null;
  level: number;
}

export interface StatMannWhitney {
  n1: number;
  n2: number;
  u: number;
  mean_rank_1: number;
  mean_rank_2: number;
  z: StatNumber;
  p: number;
  exact: boolean;
  rank_biserial: number;
  probability_of_superiority: number;
}

export interface StatWilcoxon {
  n: number;
  zeros: number;
  v: number;
  z: StatNumber;
  p: number;
  exact: boolean;
  rank_biserial: number;
}

export interface StatKruskalWallis {
  n: number;
  groups: { n: number; mean_rank: number }[];
  h: number;
  df: number;
  p: number;
  epsilon_squared: number;
  eta_squared_h: number;
}

export interface StatDunn {
  a: number;
  b: number;
  rank_difference: number;
  z: number;
  p: number;
  p_adjusted: number;
}

export interface StatRegressionTerm {
  estimate: number;
  se: number;
  t: number;
  p: number;
  ci: [number, number];
  beta: StatNumber;
  vif: StatNumber;
}

export interface StatRegressionModel {
  n: number;
  k: number;
  df_residual: number;
  terms: StatRegressionTerm[];
  r_squared: number;
  adj_r_squared: number;
  f: StatFTest;
  sigma: number;
  rss: number;
  tss: number;
  level: number;
  breusch_pagan: { statistic: number; df: number; p: number } | null;
}

/** What one coefficient of a regression stands for. */
export interface StatTermInfo {
  variable: string;
  name: string;
  kind: StatVariableKind;
  dummy: boolean;
  level: string | null;
  label: string | null;
  reference: string | null;
  step: number;
}

export interface StatModelChange {
  r2_change: number;
  f: number;
  df1: number;
  df2: number;
  p: number;
}

export interface StatInfluential {
  case_id: number;
  case_name: string;
  leverage: number;
  cooks_distance: number;
  studentized_residual: number;
  residual: number;
  fitted: number;
}

/** One coefficient of a logistic regression and its tests. A profile interval end is null when the likelihood does not fall far enough on that side. */
export interface StatLogisticTerm {
  estimate: number;
  se: number;
  z: number;
  wald: number;
  p: number;
  ci: [number, number];
  odds_ratio: StatNumber;
  or_ci: [StatNumber, StatNumber];
  profile_ci: [StatNumber, StatNumber] | null;
  or_profile_ci: [StatNumber, StatNumber] | null;
}

export interface StatDevianceTest {
  chi2: number;
  df: number;
  p: number;
}

export interface StatLogisticModel {
  n: number;
  k: number;
  events: number;
  non_events: number;
  df_residual: number;
  terms: StatLogisticTerm[];
  null_deviance: number;
  deviance: number;
  log_likelihood: number;
  null_log_likelihood: number;
  aic: number;
  bic: number;
  lr: StatDevianceTest;
  mcfadden: number;
  cox_snell: number;
  nagelkerke: number;
  hosmer_lemeshow: (StatDevianceTest & { groups: { n: number; observed: number; expected: number }[] }) | null;
  classification: {
    cutoff: number;
    true_positive: number;
    false_positive: number;
    true_negative: number;
    false_negative: number;
    accuracy: number;
    sensitivity: StatNumber;
    specificity: StatNumber;
    positive_predictive_value: StatNumber;
    negative_predictive_value: StatNumber;
  };
  auc: number;
  iterations: number;
  level: number;
}

/** What a logistic regression computed; the engine keeps its models under the same key as a linear regression's. */
export interface StatLogisticDetails {
  event: { value: string; label: string };
  other: { value: string; label: string };
  models: StatLogisticModel[];
  terms: StatTermInfo[];
  steps: { step: number; terms: number }[];
  changes: (StatDevianceTest | null)[];
  blocks: { variable: string; name: string; kind: StatVariableKind; step: number; terms: number; lr: StatDevianceTest | null }[];
  influential: { case_id: number; case_name: string; leverage: number; cooks_distance: number; standardized_residual: number; fitted: number; observed: number }[];
  profile: boolean;
  level: number;
  bootstrap?: StatBootstrap;
}

export type StatCurveKind = 'linear' | 'logarithmic' | 'inverse' | 'quadratic' | 'cubic' | 'power' | 'exponential' | 's_curve';

export type StatCurveError = 'too_few_cases' | 'not_finite' | 'needs_positive_x' | 'needs_nonzero_x' | 'needs_positive_y' | 'no_variation_in_outcome' | 'too_few_values';

export type StatCurveFitResult =
  | { kind: StatCurveKind; error: StatCurveError }
  | { kind: StatCurveKind; error?: undefined; n: number; model: StatRegressionModel; r_squared_original: number; rmse_original: number; constants: number[] };

/** A fitted curve as drawn: points along the range of x, and its value at the x of each point of the scatter. */
export interface StatCurveLine {
  kind: StatCurveKind;
  points: [number, number][];
  at_points: number[];
}

/** One page of the notebook kept in the project: an analysis (the request and the result it gave), or a page of notes. */
export interface StatNotebookPage {
  id: number;
  position: number;
  kind: 'analysis' | 'note';
  section: string;
  title: string;
  request: StatRequest | null;
  result: StatResult | null;
  ran_at: string | null;
  note: string;
  earlier_runs: number;
  created_at: string;
  updated_at: string;
}

/** What running one page of the notebook again did. */
export interface StatRerun {
  page: StatNotebookPage;
  before: StatResult | null;
  before_at: string | null;
  changed: boolean;
  error: string | null;
}

export interface StatBootstrapAnalysisItem {
  path: string;
  interval: StatBootstrapInterval;
  /** For a p-value, the share of resamples in which it fell below .05. */
  below_05: number | null;
}

export interface StatBootstrapAnalysis {
  seed: number;
  resamples: number;
  level: number;
  /** How many resamples the analysis could not be run on. */
  failed: number;
  cluster: { key: string | null; name: string; groups: number } | null;
  items: StatBootstrapAnalysisItem[];
}

export interface StatBootstrapInterval {
  estimate: number;
  bias: number;
  se: number;
  used: number;
  failures: number;
  level: number;
  percentile: [number, number];
  bca: [number, number] | null;
}

export interface StatBootstrapItem {
  statistic: 'mean' | 'median' | 'sd' | 'mean_difference' | 'median_difference' | 'correlation' | 'coefficient' | 'r_squared' | 'auc';
  /** For a coefficient, the term (0 is the intercept). */
  target: number | null;
  interval: StatBootstrapInterval | null;
}

export interface StatBootstrap {
  seed: number;
  resamples: number;
  level: number;
  items: StatBootstrapItem[];
}

/** What the procedure computed. Which fields are present depends on the procedure. */
export interface StatShapiroWilk {
  n: number;
  w: number;
  p: number;
}

export interface StatKs {
  n1: number;
  n2: number;
  d: number;
  p: number;
  exact: boolean;
}

export interface StatRuns {
  runs: number;
  n1: number;
  n0: number;
  mean: number;
  sd: number;
  z: number;
  p_normal: number;
  p_exact: number;
}

export interface StatBinomial {
  n: number;
  successes: number;
  p0: number;
  estimate: number;
  p: number;
  level: number;
  ci: [number, number];
}

export interface StatGoodnessOfFit {
  n: number;
  chi2: number;
  df: number;
  p: number;
  expected: number[];
  residuals: number[];
  smallest_expected: number;
}

export interface StatSign {
  positive: number;
  negative: number;
  zeros: number;
  median: number;
  p: number;
  proportion_positive: number;
  ci: [number, number];
  level: number;
}

export interface StatFriedman {
  n: number;
  k: number;
  chi2: number;
  df: number;
  p: number;
  kendall_w: number;
  mean_ranks: number[];
}

export interface StatMedianTest {
  median: number;
  above: number[];
  at_or_below: number[];
  chi2: number;
  df: number;
  p: number;
  fisher_p: number | null;
  smallest_expected: number;
}

export interface StatMoses {
  n_control: number;
  n_test: number;
  span: number;
  p: number;
  trim: number;
  trimmed_span: number;
  trimmed_p: number;
  ties: boolean;
}

export interface StatMcNemar {
  b: number;
  c: number;
  chi2: number;
  p: number;
  chi2_corrected: number;
  p_corrected: number;
  p_exact: number;
}

export interface StatOrdinalMeasure {
  estimate: number;
  ase: number;
  ase0: number;
  z: number | null;
  p: number | null;
}

export interface StatOrdinal {
  n: number;
  concordant: number;
  discordant: number;
  gamma: StatOrdinalMeasure;
  tau_b: StatOrdinalMeasure;
  tau_c: StatOrdinalMeasure;
  somers_columns_dependent: StatOrdinalMeasure;
  somers_rows_dependent: StatOrdinalMeasure;
  somers_symmetric: number;
}

export interface StatPartial {
  n: number;
  controls: number;
  r: number;
  t: number;
  df: number;
  p: number;
  ci: [number, number] | null;
  level: number;
}

/** One effect of a two-way analysis of variance: the first variable, the second, or the two together. */
export interface StatFactorialEffect {
  name: 'a' | 'b' | 'ab';
  ss: number;
  df: number;
  ms: number;
  f: number;
  p: number;
  partial_eta_squared: number;
}

export interface StatFactorial {
  n: number;
  levels_a: number;
  levels_b: number;
  effects: StatFactorialEffect[];
  error_ss: number;
  error_df: number;
  error_ms: number;
  total_ss: number;
  total_df: number;
  r_squared: number;
  adj_r_squared: number;
  grand_mean: number;
  balanced: boolean;
  /** Every combination of a level of the first variable and one of the second, the first variable's level changing slowest. */
  cells: { a: number; b: number; n: number; mean: number; sd: number | null }[];
  margins_a: { level: number; n: number; mean: number }[];
  margins_b: { level: number; n: number; mean: number }[];
  levene: StatFTest | null;
}

export interface StatAlpha {
  n: number;
  k: number;
  alpha: number;
  standardized_alpha: number | null;
  mean_inter_item_r: number | null;
  total_mean: number;
  total_sd: number;
  items: { mean: number; sd: number; item_rest_r: number | null; alpha_if_deleted: number | null }[];
  interval: { level: number; lower: number; upper: number } | null;
  negative_pairs: number;
}

export interface StatDetails {
  factorial?: StatFactorial;
  /** The labels and the values of the levels of the two variables of a two-way analysis, in the order of the cells. */
  labels_a?: string[];
  labels_b?: string[];
  levels_a?: string[];
  levels_b?: string[];
  alpha?: StatAlpha;
  shapiro_wilk?: StatShapiroWilk;
  /** The Kolmogorov-Smirnov test (one sample against a normal curve, or two samples). */
  ks?: StatKs;
  mean?: number;
  sd?: number;
  estimated?: boolean;
  runs?: StatRuns;
  cut?: number | null;
  cut_kind?: StatRunsCut | null;
  second_value?: string | null;
  /** The cases the runs test read. */
  n?: number;
  binomial?: StatBinomial;
  success?: string;
  success_label?: string;
  goodness_of_fit?: StatGoodnessOfFit;
  equal_shares?: boolean;
  sign?: StatSign;
  paired?: boolean;
  friedman?: StatFriedman;
  median_test?: StatMedianTest;
  moses?: StatMoses;
  control?: string;
  test?: string;
  mcnemar?: StatMcNemar;
  values?: string[];
  labels?: string[];
  ordinal?: StatOrdinal;
  partial?: StatPartial;
  zero_order?: StatCorrelation | null;
  controls?: string[];
  kind?: 'descriptives' | 'frequencies';
  descriptives?: StatDescriptives;
  categories?: { value: string; label: string; count: number; percent: number; valid_percent: number }[];
  /** The counts of a binomial test or a one-sample chi-square, with the count each category was expected to have. */
  counts?: { value: string; label: string; count: number; expected?: number; share?: number; residual?: number }[];
  valid?: number;
  missing?: number;
  t_test?: StatTTest;
  mu?: number;
  median?: StatNumber;
  median_difference?: StatNumber;
  anova?: StatOneWay;
  pairwise?: StatPairwise[];
  adjustment?: StatAdjustment;
  posthoc?: StatPostHoc | null;
  level?: number;
  chi_square?: StatChiSquare;
  fisher?: StatFisher;
  table?: number[][];
  row_values?: string[];
  col_values?: string[];
  row_labels?: string[];
  col_labels?: string[];
  correlation?: StatCorrelation;
  mann_whitney?: StatMannWhitney;
  wilcoxon?: StatWilcoxon;
  kruskal_wallis?: StatKruskalWallis;
  dunn?: StatDunn[];
  models?: StatRegressionModel[];
  terms?: StatTermInfo[];
  steps?: { step: number; terms: number }[];
  changes?: (StatModelChange | null)[];
  influential?: StatInfluential[];
  bootstrap?: StatBootstrap;
  /** The curves of a curve fit, each fitted or with the reason it could not be. */
  fits?: StatCurveFitResult[];
  /** The bootstrap of the whole analysis: the spread of every number the procedure reported, by its path. */
  bootstrap_analysis?: StatBootstrapAnalysis;
}

/** What a chart is about: a variable of the run (by its place in the list), or a quantity made from them. */
export type StatSubject = { type: 'variable'; index: number } | { type: 'differences' } | { type: 'residuals' } | { type: 'fitted' };

export interface StatBin {
  lo: number;
  hi: number;
  count: number;
}

export interface StatBoxGroup {
  value: string;
  label: string;
  n: number;
  lower_whisker: number;
  q1: number;
  median: number;
  q3: number;
  upper_whisker: number;
  mean: number;
  outliers: number[];
}

export interface StatMeanGroup {
  value: string;
  label: string;
  n: number;
  mean: number;
  lo: number | null;
  hi: number | null;
}

export interface StatPoint {
  x: number;
  y: number;
  case_id: number;
}

export interface StatLine {
  intercept: number;
  slope: number;
}

/** The data of one chart. The page draws it and words it. */
export type StatChart =
  | { kind: 'histogram'; subject: StatSubject; n: number; bins: StatBin[]; mean: number; sd: number | null; reference: number | null }
  | { kind: 'bars'; subject: StatSubject; valid: number; categories: { value: string; label: string; count: number }[] }
  | { kind: 'box'; subject: StatSubject; by: number; groups: StatBoxGroup[] }
  | { kind: 'means'; subject: StatSubject; by: number; level: number; groups: StatMeanGroup[] }
  | { kind: 'stacked'; row: number; col: number; row_values: string[]; row_labels: string[]; col_values: string[]; col_labels: string[]; counts: number[][] }
  | { kind: 'scatter'; x: StatSubject; y: StatSubject; points: StatPoint[]; omitted: number; fit: StatLine | null; zero_line: boolean }
  | { kind: 'curves'; x: StatSubject; y: StatSubject; points: StatPoint[]; omitted: number; curves: StatCurveLine[] }
  | { kind: 'qq'; subject: StatSubject; points: { theoretical: number; sample: number }[]; line: StatLine };

export interface StatResult {
  procedure: StatProcedure;
  variables: { key: string; name: string; kind: StatVariableKind; code_id: number | null; measure: StatMeasure }[];
  cases_total: number;
  n_used: number;
  n_excluded: number;
  groups: StatGroup[];
  assumptions: StatAssumption[];
  charts: StatChart[];
  details: StatDetails;
  /** When each case was counted by a weight: which variable, and how many cases that came to. */
  weight?: { variable: string; cases: number; counted: number; left_out: number } | null;
}

/** The groups an analysis can be run on one by one, each with the rule that picks it. */
export interface SplitPlan {
  variable: string;
  groups: { value: string; label: string; cases: number; rule: string }[];
  left_out: number;
}

export interface ContextArgs {
  query: string;
  source_ids?: number[];
  case_id?: number;
  words_around?: number;
  limit?: number;
}

/** One place a word is used, with the words round it. */
export interface WordContext {
  source_id: number;
  source_name: string;
  start: number;
  end: number;
  left: string;
  hit: string;
  right: string;
}

export interface WordContexts {
  items: WordContext[];
  total: number;
  documents: number;
}

/** A code both projects have whose definitions differ; this project's wording is kept. */
export interface MergeCodeConflict {
  path: string;
  ours: string;
  theirs: string;
}

/** What a merge did, or (for a preview) would do. */
/** The differences a merge finds that the person settled in favour of the copy; everything else keeps this project's. */
export interface MergeChoices {
  values_from_copy?: { case: string; variable: string }[];
  definitions_from_copy?: string[];
}

/** How the final layer stands (decision 173). */
export interface FinalSummary {
  decided: number;
  agreed: number;
  open: number;
  without_unit: number;
  /** The same for coded areas of pictures and pages and stretches of recordings (decision 176). */
  areas_decided: number;
  areas_agreed: number;
  areas_open: number;
}

/** One coder's area or stretch in an area in disagreement. */
export type RegionMember = RegionShape & {
  region_id: number;
  coder_id: number;
  coder_name: string;
  rationale: string;
};

/** What a person decided about an area or stretch in disagreement. */
export interface RegionDecision {
  accepted: boolean;
  reason: string;
  adjudicator_id: number;
  adjudicator_name: string;
  created_at: string;
}

/** An area of a picture or page, or a stretch of a recording, that some coders marked and others did not (decision 176). */
export interface RegionCase {
  /** The number the decision goes by. */
  key: number;
  source_id: number;
  source_name: string;
  /** The page of the PDF, counting from 1; 1 for a picture. */
  page: number;
  kind: 'image' | 'clip';
  code_id: number;
  code_name: string;
  members: RegionMember[];
  /** The coders of the document who did not mark it. */
  missing: string[];
  decision: RegionDecision | null;
}

export interface MergeReport {
  documents_added: string[];
  documents_matched: number;
  documents_in_conflict: string[];
  coders_added: string[];
  codes_added: string[];
  codes_in_conflict: MergeCodeConflict[];
  codings_added: number;
  codings_already_here: number;
  codings_skipped: { reason: string; count: number }[];
  areas_added: number;
  areas_already_here: number;
  memos_added: number;
  memos_already_here: number;
  /** Links between passages (decision 162). */
  links_added: number;
  links_already_here: number;
  /** Cases and the variables of cases (decision 171). */
  cases_added: string[];
  cases_matched: number;
  case_members_added: number;
  variables_added: string[];
  variables_in_conflict: string[];
  values_added: number;
  values_in_conflict: { case: string; variable: string; ours: string; theirs: string }[];
  /** Differences the person settled in favour of the copy (decision 174). */
  values_replaced: number;
  definitions_replaced: string[];
  backup: string | null;
  applied: boolean;
}

export interface ImportOptionsArgs {
  path: string;
  name?: string;
  folder?: string;
  source_type?: SourceType;
  unit_kind?: UnitKind;
  interviewers?: string[];
  mapping?: SurveyMapping;
  picture_variables?: PictureVariables;
}

/** Present when the file chosen was a recording: the transcript shown is read from the captions beside it. */
export interface InspectMedia {
  path: string;
  kind: 'audio' | 'video';
  file_name: string;
  /** The caption file beside the recording; null when there is none, and the recording comes in as a document of its own. */
  captions: string | null;
  size_bytes: number;
  stands_alone?: boolean;
}

export interface InspectBatch {
  kind: 'batch';
  duplicate_of: number | null;
  summary: ImportSummary;
  media?: InspectMedia;
  /** Present when the file is a picture. */
  picture?: PictureFacts | null;
}

export interface InspectTable {
  kind: 'table';
  duplicate_of: number | null;
  table: TablePreview;
  warnings: ImportWarning[];
  suggested_mapping: SurveyMapping;
  summary: ImportSummary | null;
}

export type InspectResult = InspectBatch | InspectTable;

export interface ImportOutcome {
  source_ids: number[];
  case_ids: number[];
  units_created: number;
  attribute_values_created: number;
}

export interface ImportRunResult {
  outcome: ImportOutcome;
  summary: ImportSummary;
  /** The recording that came in with its captions, when the file was a recording. */
  media?: MediaRow;
}

/** What taking a revised file as the new version of a document did. */
export interface RevisionOutcome {
  text_revision: number;
  characters: number;
  codings_moved: number;
  codings_removed: number;
  units_moved: number;
  units_removed: number;
  units_added: number;
  proposals_rejected: number;
}

export interface ReviseResult {
  outcome: RevisionOutcome;
  summary: ImportSummary;
  /** The backup copy of the project that was saved first. */
  backup: string;
  backup_note: string | null;
}

export interface FolderImportResult {
  imported: { path: string; source_ids: number[]; units: number }[];
  duplicates: { path: string; message: string }[];
  skipped: { path: string; reason: string }[];
  failed: { path: string; code: string; message: string }[];
  sources_created: number;
  units_created: number;
}

export type ExportFormat = 'xlsx' | 'docx' | 'csv' | 'qdc' | 'qdpx' | 'pstat';

/** What the case table for PrismStatistics holds (decision 193): the numbers, and plain sentences about what was left out or changed. */
export interface PstatReport {
  cases: number;
  variables: number;
  attributes: number;
  codes: number;
  /** An attribute's name and the variable name made from it, for each name that had to change. */
  renamed: [string, string][];
  notes: string[];
}

/** What a codebook file (.qdc) brought into the codebook (decision 160). */
export interface QdcImportReport {
  added: number;
  themes: number;
  already_there: number;
  left_out: string[];
}

/** What went into a QDPX file or came out of one, and what did not (the standard asks a program to say which parts are discarded). */
export interface QdpxReport {
  carried: string[];
  left_out: string[];
}

export interface ExportArgs {
  format: ExportFormat;
  path: string;
  overwrite?: boolean;
  include_inactive?: boolean;
  code_ids?: number[];
  coder_ids?: number[];
  source_ids?: number[];
}

export interface ExportResult {
  path: string;
  bytes: number;
  /** True for a picture saved while a list of names is applied to exports: a picture cannot have names taken out of it. */
  names_not_replaced?: boolean;
  /** A whole-project export: what the file holds and what it does not. */
  report?: QdpxReport;
}

/** One thing to find, and what takes its place (pseudonymizing and redacting). */
export interface DeidRule {
  id: number;
  term: string;
  replacement: string;
  kind: 'pseudonym' | 'redact';
  match_case: boolean;
  whole_word: boolean;
  note: string;
  created_at: string;
}

/** The list of names to replace, the switch for what is exported, and when names were last replaced for good. */
export interface DeidState {
  rules: DeidRule[];
  exports: boolean;
  last_applied: { at: string; replacements: number; documents: number } | null;
}

export interface DeidNew {
  terms: string[];
  replacement: string;
  kind?: 'pseudonym' | 'redact';
  match_case?: boolean;
  whole_word?: boolean;
  note?: string;
}

export type DeidSuggestionKind = 'speaker' | 'case' | 'email' | 'web' | 'phone' | 'number' | 'name';

export interface DeidSuggestion {
  term: string;
  kind: DeidSuggestionKind;
  replacement: string;
  match_case: boolean;
  count: number;
  documents: number;
}

/** A kind of place in the project that holds text, with how many times the list finds something there. */
export interface DeidPlace {
  place: string;
  count: number;
  /** True when changing names for good changes this place. */
  covered: boolean;
}

export interface DeidSkipped {
  id: number;
  name: string;
  why: string;
}

export interface DeidPreview {
  rules: { id: number; hits: number; documents: number }[];
  sources: { id: number; name: string; hits: number }[];
  samples: { rule_id: number; source_id: number; source_name: string; before: string; after: string }[];
  total: number;
  skipped: DeidSkipped[];
  elsewhere: DeidPlace[];
}

export interface DeidOutcome {
  backup: string;
  backup_note: string | null;
  earlier_backups: number;
  documents_changed: number;
  replacements: number;
  codings_moved: number;
  codings_removed: number;
  units_moved: number;
  proposals_rejected: number;
  changed: DeidPlace[];
  skipped: DeidSkipped[];
  remaining: DeidPlace[];
  rules_removed: number;
  compacted: boolean;
  key_path: string | null;
}

/** A picture or a PDF a de-identified copy does not hold. */
export interface DeidTakenOut {
  id: number;
  name: string;
  kind: 'picture' | 'pdf';
  areas_removed: number;
}

/** What making a copy with the names replaced did (decision 180). */
export interface DeidCopyOutcome {
  path: string;
  outcome: DeidOutcome;
  taken_out: DeidTakenOut[];
  recordings_unlinked: number;
  runs_archived: number;
  key_path: string | null;
}

/** A part of the report: made from the project's records, in this order. */
export type ReportSectionId = 'study' | 'documents' | 'codebook' | 'passages' | 'agreement' | 'adjudication' | 'memos' | 'ai_use' | 'methods' | 'themes' | 'ethics' | 'mixed_methods' | 'timeline';

export interface ReportArgs {
  path: string;
  overwrite?: boolean;
  title?: string;
  sections?: ReportSectionId[];
}

/** One variable of the table a recorded script reads: its value for every case, null where it has none. */
export interface StatTableColumn {
  key: string;
  name: string;
  kind: StatVariableKind;
  measure: StatVariable['measure'];
  numeric: boolean;
  values: (string | null)[];
}

export interface StatTable {
  cases: { id: number; name: string }[];
  columns: StatTableColumn[];
}

/** One chart in an exported notebook: its words, and its drawing and picture where the file shows them. */
export interface NotebookPayloadChart {
  title: string;
  alt: string;
  png?: string;
  svg?: string;
  width: number;
  height: number;
}

/** The output notebook as the engine writes it into Word, Excel, or a web page; the interface supplies every word. */
export interface NotebookPayload {
  title: string;
  language: string;
  labels: { table: string; figure: string; note: string; contents: string; checks: string; alternative: string };
  footer: string;
  pages: {
    title: string;
    sentence: string;
    reading: string;
    notes: string[];
    tables: { title: string; head: string[]; rows: string[][]; note: string }[];
    checks: string[];
    charts: NotebookPayloadChart[];
  }[];
}

export type NotebookFileFormat = 'docx' | 'xlsx' | 'html';

export interface UndoResult {
  undone_event?: number;
  action?: string;
  what?: string;
  nothing_to_undo?: boolean;
}

export interface SearchHit {
  source_id: number;
  source_name: string;
  unit_id: number | null;
  scope_start: number;
  scope_end: number;
  spans: [number, number][];
  snippet: string;
}

export interface SearchResult {
  hits: SearchHit[];
  total: number;
  truncated: boolean;
}

// ---- Auto Code and the review queue

export type AutoMethod = 'words' | 'speakers' | 'headings';
/** What one suggestion covers when it is found by words. */
export type AutoTarget = 'hit' | 'words' | 'sentence' | 'paragraph' | 'unit';

/** Where Auto Code looks: the same narrowing a search takes. Absent means every document. */
export interface AutoScope {
  source_ids?: number[];
  case_id?: number;
}

export interface SpeakerChoiceArg {
  key: string;
  code_id?: number;
  new_code?: string;
}

export interface AutocodeArgs extends AutoScope {
  method: AutoMethod;
  query?: string;
  target?: AutoTarget;
  around?: number;
  levels?: number[];
  heading_query?: string;
  include_heading?: boolean;
  speakers?: SpeakerChoiceArg[];
  code_id?: number;
  new_code?: string;
}

export interface SpeakerGroup {
  key: string;
  label: string;
  variants: string[];
  turns: number;
  left_out: number;
  sources: number;
}

export interface HeadingLevel {
  level: number;
  count: number;
  sources: number;
}

export interface AutocodeExample {
  source_id: number;
  source_name: string;
  start: number;
  end: number;
  text: string;
  truncated: boolean;
  before: string;
  after: string;
  code_name: string;
}

export interface AutocodePreview {
  method: AutoMethod;
  label: string;
  /** Places found, before suggestions that repeat earlier work were taken out. */
  found: number;
  suggestions: number;
  documents: number;
  already_coded: number;
  already_waiting: number;
  left_out: number;
  truncated: boolean;
  codes: { code_id: number | null; name: string; new: boolean; suggestions: number }[];
  examples: AutocodeExample[];
}

export interface AutocodeProposed {
  batch: string;
  created: number;
  found: number;
  truncated: boolean;
  left_out: number;
  already_coded: number;
  already_waiting: number;
  codes_created: { id: number; name: string }[];
}

export type ProposalStatus = 'pending' | 'accepted' | 'rejected';

export interface ProposalRow {
  id: number;
  origin: string;
  batch: string;
  status: ProposalStatus;
  source_id: number;
  start: number;
  end: number;
  code_id: number;
  coder_id: number;
  unit_id: number | null;
  rationale: string;
  created_at: string;
  decided_at: string | null;
  decided_by: string | null;
  source_name: string;
  text: string;
  truncated: boolean;
  before: string;
  after: string;
  already_coded: boolean;
  /** The AI run that made it, for a suggestion from an AI. */
  ai_run_id?: number | null;
}

export interface BatchSummary {
  batch: string;
  origin: string;
  method: string;
  label: string;
  created_at: string;
  pending: number;
  accepted: number;
  rejected: number;
  code_ids: number[];
}

export interface DecisionOutcome {
  accepted: number;
  rejected: number;
  failed: [number, string][];
}

/** What goes in a prompt packet. Everything is optional: no choice means every document and every code that may be sent. */
export interface PacketArgs {
  source_ids?: number[];
  case_id?: number;
  code_ids?: number[];
  only_uncoded?: boolean;
  limit?: number;
  skip?: number;
}

export interface AiPacketLeftOut {
  documents_not_shared: number;
  passages_not_shared: number;
  already_coded: number;
  too_long: number;
  /** Pictures: a packet carries words only. */
  pictures: number;
  /** Codes that may not be sent, by name. */
  codes: string[];
}

export interface AiPacket {
  run_id: number;
  /** Exactly the text to take to a chatbot. */
  prompt: string;
  passages: number;
  characters: number;
  codes: { id: number; name: string }[];
  left_out: AiPacketLeftOut;
  more_after: number;
  next_skip: number;
}

export interface AiRejected {
  number: number;
  passage: string;
  code: string;
  why: string;
}

export interface AiAnswerOutcome {
  read: number;
  suggested: number;
  rejected: AiRejected[];
  already_coded: number;
  already_waiting: number;
  batch: string | null;
}

export interface AiRunRow {
  id: number;
  route: string;
  model: string;
  created_at: string;
  answered: boolean;
  passages: number;
  codes: number;
  read: number;
  suggested: number;
  checker_rejected: number;
  pending: number;
  accepted: number;
  rejected: number;
  prompt_hash: string;
  /** The review queue batch the run's suggestions went to, or null when none did. */
  batch: string | null;
  /** What the packet asked for: suggested codings of passages, or candidate themes from the codebook (decision 168). */
  task?: 'code_with_codebook' | 'suggest_themes';
}

export interface AiRunDetail extends AiRunRow {
  prompt: string;
  raw_output: string;
  checker_rejections: AiRejected[];
}

export interface SegmentRow {
  /** `text` for words, `image` for an area of a picture or a PDF page (then `region` is the area and `text` tells it in words), `clip` for a stretch of a recording. */
  kind: 'text' | 'image' | 'clip';
  region: RegionShape | null;
  /** The page of the PDF an area is on; null for words and for a picture. */
  page?: number | null;
  /** The number of the coding; negative for an area of a picture (area 7 is -7). */
  coding_id: number;
  source_id: number;
  source_name: string;
  folder: string;
  start: number;
  end: number;
  text: string;
  code_id: number;
  code_name: string;
  coder_id: number;
  coder_name: string;
  coder_role: string;
  origin: string;
  layer: string;
  rationale: string | null;
  unit_id: number | null;
  unit_external_id: string | null;
  speaker: string | null;
  created_at: string;
}

/** A link between two coded passages, with both (decision 162). */
export interface PassageLink {
  id: number;
  relation: string;
  from: SegmentRow;
  to: SegmentRow;
}

/** The relations the project uses, most used first, and some common ones to offer. */
export interface LinkRelations {
  used: { name: string; uses: number }[];
  common: string[];
}

export interface SegmentsArgs {
  limit?: number;
  /** True for the pages that follow the working set (Retrieved Segments, the document portrait); other callers see every passage. It is not sent to the engine. */
  workingSet?: boolean;
  /** Only these codings, by number: the evidence behind a count. */
  coding_ids?: number[];
  source_ids?: number[];
  code_ids?: number[];
  exact_codes?: boolean;
  coder_ids?: number[];
  layer?: 'coder' | 'final';
}

export interface SegmentsResult {
  segments: SegmentRow[];
  total: number;
  truncated: boolean;
}

// ---------------------------------------------------------------- exploring the coding

/** The four honest ways of counting the same evidence. */
export interface Counts {
  /** Codings: one code applied to one stretch of one document by one coder. */
  assignments: number;
  /** Distinct stretches of text: two coders, or two codes, on the same words count once. */
  excerpts: number;
  /** Documents with at least one qualifying coding. */
  documents: number;
  /** Cases with at least one qualifying coding. */
  cases: number;
}
export type Metric = keyof Counts;

export interface ExploreScope {
  source_ids?: number[];
  case_ids?: number[];
  coder_ids?: number[];
  layer?: 'coder' | 'final';
}

export type RowSpec = { kind: 'cases' } | { kind: 'documents' } | { kind: 'case_variable'; name: string } | { kind: 'month_coded' };

export interface MatrixSpec {
  rows: RowSpec;
  code_ids: number[];
  include_descendants: boolean;
  scope?: ExploreScope;
}

export interface MatrixResult {
  rows: { key: string; label: string; size: number | null; cells: Counts[]; total: Counts }[];
  columns: { code_id: number; name: string; total: Counts }[];
  total: Counts;
  universe: Counts;
  notes: string[];
}

export type RelationMode = 'overlap' | 'document' | 'case';

export interface RelationsSpec {
  code_ids: number[];
  include_descendants: boolean;
  mode: RelationMode;
  same_coder: boolean;
  scope?: ExploreScope;
}

export interface RelationsResult {
  codes: { code_id: number; name: string }[];
  cells: number[][];
  notes: string[];
}

export interface CodeSummaryRow {
  code_id: number;
  name: string;
  parent_id: number | null;
  status: string;
  own_assignments: number;
  assignments: number;
  excerpts: number;
  documents: number;
  cases: number;
  coders: number;
}

export interface CountedName {
  id: number;
  name: string;
  assignments: number;
  excerpts: number;
}

export interface CodeProfile {
  summary: CodeSummaryRow;
  by_document: CountedName[];
  by_case: CountedName[];
  by_coder: CountedName[];
  together_with: CountedName[];
  coding_ids: number[];
}

export interface CaseVariable {
  name: string;
  level: string;
  values: [string, number][];
  /** How many cases have a value that reads as a date (year-month-day). */
  dates: number;
  /** How many cases have a value that reads as a number. */
  numbers: number;
}

// ---- Concept maps

export type MapNodeKind = 'code' | 'case' | 'memo' | 'note';
export type MapLayout = 'circle' | 'grid' | 'layers';

export interface MapRow {
  id: number;
  name: string;
  notes: string;
  nodes: number;
  links: number;
  created_at: string;
  updated_at: string;
}

export interface MapNode {
  id: number;
  kind: MapNodeKind;
  ref_id: number | null;
  /** The words on the node: the name of what it stands for, or a note's own text. */
  label: string;
  x: number;
  y: number;
  /** The code, case, or memo it stood for no longer exists. */
  missing: boolean;
}

export interface MapLink {
  id: number;
  from_node: number;
  to_node: number;
  label: string;
  directed: boolean;
}

export interface ConceptMap {
  id: number;
  name: string;
  notes: string;
  nodes: MapNode[];
  links: MapLink[];
  created_at: string;
  updated_at: string;
}

export interface AddedCodes {
  nodes_added: number;
  links_added: number;
  already_there: number;
}

// ---- Crosstabs and saved queries

export interface CrosstabSpec {
  row_variable: string;
  column_variable: string;
  row_width?: number;
  column_width?: number;
  case_ids?: number[];
}

export interface CrosstabValue {
  key: string;
  label: string;
  total: number;
}

export interface CrosstabAxis {
  variable: string;
  level: string;
  values: CrosstabValue[];
}

export interface Crosstab {
  rows: CrosstabAxis;
  columns: CrosstabAxis;
  cells: number[][];
  total: number;
  notes: string[];
}

export interface CaseRef {
  id: number;
  name: string;
  /** A document the case is in (or has a unit of), and where in it. */
  source_id: number | null;
  start: number | null;
  end: number | null;
}

export interface SavedQueryRow {
  id: number;
  name: string;
  note: string;
  frozen_at: string | null;
  frozen_count: number | null;
  created_at: string;
  updated_at: string;
}

export interface SavedQuery {
  id: number;
  name: string;
  note: string;
  /** The query as the Query Builder keeps it, to open it again. */
  draft: unknown;
  spec: QuerySpec;
  frozen_at: string | null;
  frozen_count: number | null;
  created_at: string;
  updated_at: string;
}

export interface FrozenComparison {
  frozen_at: string;
  frozen: number;
  now: number;
  still_found: number;
  added: number;
  added_ids: number[];
  no_longer_found: number;
  removed: number;
}

/** One clause of a query, or a group of clauses, in the form the engine reads. */
export type QueryNode =
  | { type: 'group'; match: 'any' | 'all'; items: QueryNode[]; negate?: boolean }
  | { type: 'code'; code_ids: number[]; include_descendants?: boolean; negate?: boolean }
  | { type: 'also_coded'; code_ids: number[]; within_chars?: number; include_descendants?: boolean; same_coder?: boolean; negate?: boolean }
  | { type: 'text'; contains: string; negate?: boolean }
  | { type: 'source'; ids: number[]; negate?: boolean }
  | { type: 'case'; ids: number[]; negate?: boolean }
  | { type: 'case_variable'; name: string; value: string; negate?: boolean }
  | { type: 'coder'; ids: number[]; negate?: boolean }
  | { type: 'rating'; min?: number; max?: number; negate?: boolean }
  | { type: 'coded_between'; from?: string; to?: string; negate?: boolean }
  | { type: 'case_variable_between'; name: string; from?: string; to?: string; negate?: boolean };

export interface QuerySpec {
  rule: QueryNode;
  result_unit: 'excerpts' | 'assignments';
  scope?: ExploreScope;
  limit?: number;
}

export interface QueryResultItem {
  /** `text`, `image` for an area of a picture or a PDF page (then `region` is the area and `text` tells it in words), or `clip` for a stretch of a recording. */
  kind: 'text' | 'image' | 'clip';
  region: RegionShape | null;
  /** The page of the PDF an area is on; null for words and for a picture. */
  page?: number | null;
  source_id: number;
  source_name: string;
  start: number;
  end: number;
  text: string;
  codings: { coding_id: number; code_id: number; code_name: string; coder_name: string; rating: number | null }[];
}

export interface QueryResult {
  items: QueryResultItem[];
  total: number;
  assignments: number;
  truncated: boolean;
  universe: Counts;
}

// ---- the data editor: variables of cases, their values, and the transformations ----

/** A variable of cases as the editor shows it. */
export interface VariableDef {
  id: number;
  name: string;
  level: Level;
  /** The words shown for stored values, by stored value. */
  value_labels: Record<string, string>;
  /** The stored values that mean missing (at most three). */
  missing_codes: string[];
  /** How the variable was made, when a transformation made it. */
  derived: { transform?: TransformSpec; made_with?: string } | null;
}

export interface VariableSpec {
  name: string;
  level: Level;
  value_labels: Record<string, string>;
  missing_codes: string[];
}

export interface CaseData {
  cases: { id: number; name: string }[];
  variables: VariableDef[];
  /** values[case][variable], in the order of cases and variables; null where there is none. */
  values: (string | null)[][];
}

export interface CellChange {
  case_id: number;
  value: string | null;
}

export interface RecodeRule {
  from: string[];
  to: string;
}

export type RecodeOtherwise = 'keep' | 'missing';

export type TransformSpec = { target: string; replace?: boolean; level?: Level | null } & (
  | { kind: 'compute'; formula: string }
  | { kind: 'recode'; source: string; rules: RecodeRule[]; otherwise?: RecodeOtherwise }
  | { kind: 'standardize'; source: string }
  | { kind: 'rank'; source: string; descending?: boolean }
  | { kind: 'cut'; source: string; cuts: number[]; right?: boolean }
  | { kind: 'from_codes'; codes: number[]; measure: CodeMeasure; include_descendants?: boolean }
  | { kind: 'date_part'; source: string; part: DatePartId }
  | { kind: 'sample'; size: SampleSize; seed?: number; within?: string | null }
  | { kind: 'aggregate'; source: string; by: string[]; function: GroupFunctionId }
);

/** What can be taken out of a date. */
export type DatePartId = 'year' | 'quarter' | 'month' | 'week' | 'day' | 'day_of_year' | 'weekday' | 'hour';

/** How many cases a random sample holds: this many, or this percent (of each group, when drawn within groups). */
export type SampleSize = { by: 'count'; value: number } | { by: 'percent'; value: number };

/** What is worked out for each group of cases. */
export type GroupFunctionId = 'mean' | 'sum' | 'median' | 'min' | 'max' | 'sd' | 'count' | 'cases';

// ---- the data editor's later tools: sorting, tables, restructuring, and merging ----

/** One key of a sort: a variable of cases (or `Case`, the case's own name), and which way. */
export interface SortKeySpec {
  variable: string;
  descending?: boolean;
}

/** Where a restructured table's rows come from: the project's cases, or a file. */
export type TableFrom = { kind: 'cases' } | { kind: 'file'; path: string };

/** A table the engine works out: a row for each group, the cases (or a file) stood on end, or observations laid across. */
export type DataTableSpec =
  | { op: 'groups'; by: string[]; summaries: { source: string; function: GroupFunctionId }[] }
  | { op: 'stand_on_end'; from: TableFrom; keep: string[]; measures: string[]; index_name: string; value_name: string; labels?: string[]; drop_missing?: boolean }
  | { op: 'lay_across'; from: TableFrom; id: string[]; index: string; values: string[] };

export interface DataTableResult {
  columns: string[];
  rows: (string | null)[][];
  total_rows: number;
  shown_rows: number;
  /** For a table of groups: how many cases were in no group (they have no value in a variable that makes the groups). */
  left_out: number | null;
}

/** A column of a file, as read for the dialogs that bring a file in. */
export interface FileColumn {
  name: string;
  /** The level of measurement the values suggest. */
  level: Level;
  examples: string[];
  distinct: number;
}

export interface InspectedFile {
  columns: FileColumn[];
  rows: number;
  file_name: string;
}

export interface MergeResult {
  cases_matched: number;
  cases_unmatched: number;
  unmatched_case_names: string[];
  rows_unmatched: number;
  unmatched_keys: string[];
  rows_without_key: number;
  rows_repeated: number;
  variables_made: string[];
  variables_filled: string[];
  values_written: number;
}

/** What a variable made from the coding says about a case: whether it has a code, how many excerpts carry it, or which of several codes it has most. */
export type CodeMeasure = 'present' | 'count' | 'dominant';

/** The kinds the "make a variable" dialog of the Data editor offers (a variable from codes is made on the Mixed Methods page). */
export type TransformKindId = Exclude<TransformSpec['kind'], 'from_codes'>;

// ---- Mixed Methods: values of a variable into codes ----

export interface QualitizeSpec {
  variable: string;
  parent?: string;
  values?: string[];
}

export interface QualitizeValue {
  value: string;
  label: string;
  cases: number;
  places: number;
}

export interface QualitizePlan {
  parent: string;
  values: QualitizeValue[];
  codings: number;
  unit_mode: boolean;
  cases_without_text: number;
}

export interface QualitizeOutcome {
  parent_id: number;
  code_ids: number[];
  codings: number;
  /** The codings that were made, which are what is taken back. */
  coding_ids: number[];
}

// ---- Mixed Methods: joint displays ----

export interface CaseDisplaySpec {
  sort_by: string;
  descending?: boolean;
  variables?: string[];
  code_ids: number[];
  include_descendants?: boolean;
  quotes?: number;
  only_coded?: boolean;
}

export interface DisplayQuote {
  coding_id: number;
  source_id: number;
  source: string;
  start: number;
  end: number;
  /** The words; null for a coded area of a picture. */
  text: string | null;
}

export interface DisplayCell {
  excerpts: number;
  quotes: DisplayQuote[];
  more: number;
  /** One coding for each excerpt, to open the evidence behind the count. */
  coding_ids: number[];
}

export interface DisplayRow {
  case_id: number;
  case_name: string;
  values: (string | null)[];
  cells: DisplayCell[];
}

export interface CaseDisplay {
  variables: VariableDef[];
  codes: { id: number; name: string }[];
  rows: DisplayRow[];
  by_number: boolean;
  cases: number;
  without_value: number;
}

export interface TypologySpec {
  code_ids: number[];
  include_descendants?: boolean;
  variables: string[];
}

export interface TypologyColumn {
  name: string;
  level: string;
  kind: 'scale' | 'categories';
  categories: { value: string; label: string }[];
}

export type TypologyCell = { kind: 'scale'; n: number; mean: number | null; sd: number | null } | { kind: 'categories'; n: number; counts: number[] };

export interface TypologyGroup {
  /** `all`, `code:<id>`, or `none`. */
  key: string;
  label: string;
  code_id: number | null;
  cases: { id: number; name: string }[];
  cells: TypologyCell[];
}

export interface Typology {
  columns: TypologyColumn[];
  groups: TypologyGroup[];
  cases_with_text: number;
}

export interface SideBySideSpec {
  code_ids: number[];
  include_descendants?: boolean;
  variables: string[];
  quotes?: number;
}

export interface TopicQuote extends DisplayQuote {
  case_id: number;
  case_name: string;
}

export interface SideGroup {
  cases: { id: number; name: string }[];
  cells: TypologyCell[];
}

export interface SideBySideTopic {
  code_id: number;
  name: string;
  excerpts: number;
  quotes: TopicQuote[];
  more: number;
  /** One coding for each excerpt, to open the evidence behind the count. */
  coding_ids: number[];
  with: SideGroup;
  without: SideGroup;
  /** For each variable, the mean of the cases that have the code less the mean of the others (numbers only). */
  differences: (number | null)[];
}

export interface SideBySide {
  columns: TypologyColumn[];
  topics: SideBySideTopic[];
  cases_with_text: number;
}

export interface DeviationSpec {
  variable: string;
  code_ids: number[];
  include_descendants?: boolean;
  top?: number;
}

export interface DeviationRow {
  code_id: number;
  name: string;
  in_cases: number;
  in_total: number;
  out_cases: number;
  out_total: number;
  in_share: number;
  out_share: number;
  difference: number;
  z: number | null;
  cases: { id: number; name: string }[];
}

export interface DeviationGroup {
  value: string;
  label: string;
  cases: number;
  most: DeviationRow[];
  least: DeviationRow[];
}

export interface Deviation {
  variable: string;
  groups: DeviationGroup[];
  cases_used: number;
  without_value: number;
  codes: number;
}

export interface SimilaritySpec {
  code_ids?: number[];
  include_descendants?: boolean;
  variables?: string[];
  shared_absence?: boolean;
  pairs?: number;
}

export interface SimilarityFeature {
  name: string;
  kind: 'code' | 'number' | 'category';
}

export interface PairPart {
  feature: number;
  a: string | null;
  b: string | null;
  similarity: number | null;
}

export interface AlikePair {
  a: { id: number; name: string };
  b: { id: number; name: string };
  similarity: number;
  compared: number;
  parts: PairPart[];
}

export interface Similarity {
  features: SimilarityFeature[];
  cases: { id: number; name: string }[];
  matrix: (number | null)[][] | null;
  pairs: AlikePair[];
  pairs_compared: number;
}

export interface BubbleSpec {
  code_id: number;
  include_descendants?: boolean;
  x: string;
  y: string;
}

export interface BubbleAxis {
  name: string;
  categories: { value: string; label: string }[];
}

export interface BubbleCell {
  x: number;
  y: number;
  cases: number;
  with_code: number;
  excerpts: number;
  rated: number;
  mean_rating: number | null;
  /** One coding for each excerpt, to open the evidence behind the size. */
  coding_ids: number[];
}

export interface Bubble {
  code: string;
  x: BubbleAxis;
  y: BubbleAxis;
  cells: BubbleCell[];
  cases_used: number;
  left_out: number;
  rated: number;
}

// ---- Mixed Methods: the design, the triangulation matrix, and the integration log ----

export type DesignKind = 'convergent' | 'explanatory_sequential' | 'exploratory_sequential' | 'embedded';
export type DesignPriority = 'equal' | 'quantitative' | 'qualitative';

export interface MixedDesign {
  kind: DesignKind;
  priority: DesignPriority;
  purpose: string;
  created_at: string;
  updated_at: string;
}

export interface DesignPlan {
  kind: DesignKind;
  /** The priority the design usually has. */
  priority: DesignPriority;
  how: IntegrationHow;
  variables: { name: string; level: string; exists: boolean }[];
  codes: { name: string; exists: boolean }[];
}

export interface DesignApplyArgs {
  kind: DesignKind;
  priority?: DesignPriority;
  purpose?: string;
  /** The sentence for the first entry of the integration log; no entry when empty. */
  log?: string;
}

export interface DesignOutcome {
  design: MixedDesign;
  variables_made: string[];
  variables_kept: string[];
  codes_made: string[];
  codes_kept: string[];
  entry_id: number | null;
}

export type TriangulationMark = '' | 'convergence' | 'complementarity' | 'discordance';

export interface TriangulationInput {
  topic: string;
  code_id?: number | null;
  variable?: string;
  qualitative?: string;
  quantitative?: string;
  mark?: TriangulationMark;
  note?: string;
}

export interface TopicWords {
  cases: number;
  excerpts: number;
  quotes: TopicQuote[];
  more: number;
  coding_ids: number[];
}

export interface TopicNumbers {
  column: TypologyColumn;
  with: TypologyCell;
  without: TypologyCell | null;
  difference: number | null;
  /** True when `with` describes every case that holds text, since the topic has no code. */
  whole: boolean;
}

export interface TriangulationRow {
  id: number;
  position: number;
  topic: string;
  code_id: number | null;
  code_name: string | null;
  variable: string;
  qualitative: string;
  quantitative: string;
  mark: TriangulationMark;
  note: string;
  created_at: string;
  updated_at: string;
  evidence: { words: TopicWords | null; numbers: TopicNumbers | null } | null;
  problem: string | null;
}

export type IntegrationStage = 'design' | 'data_collection' | 'analysis' | 'interpretation' | 'reporting';
export type IntegrationHow = 'connecting' | 'building' | 'merging' | 'embedding' | 'transforming';

export interface IntegrationInput {
  on_date?: string | null;
  stage: IntegrationStage;
  how: IntegrationHow;
  what: string;
  why?: string;
}

export interface IntegrationEntry {
  id: number;
  on_date: string;
  stage: IntegrationStage;
  how: IntegrationHow;
  what: string;
  why: string;
  actor: string;
  created_at: string;
  updated_at: string;
}

export interface QualitizeTakeBack {
  codings_removed: number;
  codes_retired: number;
}

export interface TransformOutcome {
  variable: string;
  level: Level;
  created: boolean;
  cases_with_value: number;
  cases_missing: number;
  /** The words the new values carry (the ranges of a cut), by stored value. */
  value_labels: Record<string, string>;
  /** The first few cases with the value each would get. */
  preview: { case_name: string; value: string | null }[];
}

// ---- the command table: name, arguments, and result ----

export interface Commands {
  ping: { args: Record<string, never>; result: PingResult };

  'project.create': {
    args: { path: string; name: string; coding_mode?: CodingMode; method?: string; research_questions?: string; passphrase?: string };
    result: ProjectInfo;
  };
  'project.open': { args: { path: string; backup?: boolean; passphrase?: string }; result: ProjectInfo };
  'project.encrypt': { args: { passphrase: string }; result: EncryptionOutcome & { info: ProjectInfo } };
  'project.decrypt': { args: { passphrase: string }; result: EncryptionOutcome & { info: ProjectInfo } };
  'project.change_passphrase': { args: { old: string; new: string }; result: EncryptionOutcome & { info: ProjectInfo } };
  'project.stale_backups': { args: Record<string, never>; result: { backups: StaleBackup[] } };
  'project.delete_stale_backups': { args: Record<string, never>; result: { deleted: number } };
  'project.close': { args: Record<string, never>; result: Record<string, never> };
  'project.info': { args: Record<string, never>; result: ProjectInfo };
  /** `note` says so when the chosen backup folder could not be used and the copy was made beside the project. */
  'project.backup': { args: Record<string, never>; result: { path: string; note: string | null } };
  'backup.keep': { args: { keep: number }; result: { keep: number } };
  'history.name': { args: { name: string }; result: { name: string } };
  'update.state': { args: Record<string, never>; result: import('./lib/updates').UpdateState };
  'update.check': { args: Record<string, never>; result: import('./lib/updates').UpdateFound };
  'update.install': { args: Record<string, never>; result: { installed: boolean; version?: string } };
  /** Where backups go: a folder (checked first unless `check` is false) or null for beside the project. */
  'backup.dir': { args: { dir: string | null; check?: boolean }; result: { dir: string | null } };

  'import.inspect': { args: ImportOptionsArgs; result: InspectResult };
  'import.run': { args: ImportOptionsArgs & { force?: boolean }; result: ImportRunResult };
  'image.get': { args: { source_id: number }; result: (SourceImage & { source_id: number; created_at: string }) | null };
  'image.list': { args: Record<string, never>; result: (SourceImage & { source_id: number; created_at: string })[] };
  'image.save_copy': { args: { source_id: number; path: string }; result: { path: string } };
  'region.list': { args: { source_id: number }; result: RegionRow[] };
  'stats.split_groups': { args: { variable: string }; result: SplitPlan };
  'data.sort': { args: { keys: SortKeySpec[] }; result: { case_ids: number[] } };
  'data.table': { args: { spec: DataTableSpec }; result: DataTableResult };
  'data.table_save': { args: { spec: DataTableSpec; path: string; format: 'csv' | 'xlsx'; overwrite?: boolean }; result: { path: string; rows: number; columns: number } };
  'data.inspect_file': { args: { path: string }; result: InspectedFile };
  'data.merge': {
    args: { path: string; key_column: string; match_on?: string | null; columns: { name: string; as?: string; level?: Level }[]; overwrite?: boolean };
    result: MergeResult;
  };
  'pdf.get': { args: { source_id: number }; result: PdfRow | null };
  'pdf.attach': { args: { source_id: number; path: string }; result: PdfRow };
  'pdf.save_copy': { args: { source_id: number; path: string }; result: { path: string } };
  'region.add': { args: RegionShape & { source_id: number; page?: number; code_id: number; coder_id?: number; rationale?: string }; result: { id: number; coding_id: number; region: RegionRow } };
  'region.reshape': { args: RegionShape & { id: number }; result: RegionRow };
  'media.get': { args: { source_id: number }; result: MediaRow | null };
  'media.list': { args: Record<string, never>; result: MediaRow[] };
  'media.link': { args: { source_id: number; path: string }; result: MediaRow };
  'media.unlink': { args: { source_id: number }; result: Record<string, never> };
  'import.revise': { args: ImportOptionsArgs & { source_id: number }; result: ReviseResult };
  'import.folder': {
    args: { path: string; recursive?: boolean; folder?: string; include_tables?: boolean; force?: boolean; picture_variables?: PictureVariables };
    result: FolderImportResult;
  };

  'source.list': { args: Record<string, never>; result: SourceRow[] };
  'source.get': { args: { id: number }; result: SourceDetail };
  'source.edit': { args: { id: number; text: string }; result: { codings_moved: number; codings_removed: number } & Record<string, unknown> };
  'unit.list': { args: { source_id: number }; result: UnitRow[] };
  'unit.attributes': { args: { source_id: number }; result: AttributeValueRow[] };
  'case.list': { args: Record<string, never>; result: CaseRow[] };
  'case.variables': { args: Record<string, never>; result: CaseVariable[] };
  'explore.summary': { args: { scope?: ExploreScope }; result: CodeSummaryRow[] };
  'explore.profile': { args: { code_id: number; include_descendants?: boolean; scope?: ExploreScope }; result: CodeProfile };
  'explore.matrix': { args: MatrixSpec; result: MatrixResult };
  'explore.matrix_evidence': { args: { spec: MatrixSpec; row_key?: string | null; code_id?: number | null }; result: { coding_ids: number[] } };
  'explore.relations': { args: RelationsSpec; result: RelationsResult };
  'explore.relations_evidence': { args: { spec: RelationsSpec; a: number; b: number }; result: { coding_ids: number[] } };
  'explore.query': { args: QuerySpec; result: QueryResult };
  'explore.crosstab': { args: CrosstabSpec; result: Crosstab };
  'explore.crosstab_cases': { args: { spec: CrosstabSpec; row_key?: string | null; column_key?: string | null }; result: CaseRef[] };
  'query.list': { args: Record<string, never>; result: SavedQueryRow[] };
  'query.get': { args: { id: number }; result: SavedQuery };
  'query.save': { args: { name: string; note?: string; draft: unknown; spec: QuerySpec }; result: SavedQuery };
  'query.update': { args: { id: number; name?: string; note?: string; draft?: unknown; spec?: QuerySpec }; result: SavedQuery };
  'query.delete': { args: { id: number }; result: Record<string, never> };
  'query.freeze': { args: { id: number }; result: { frozen_at: string; codings: number; results: number } };
  'query.unfreeze': { args: { id: number }; result: Record<string, never> };
  'query.run_saved': { args: { id: number }; result: { query: SavedQuery; result: QueryResult } };
  'query.compare': { args: { id: number }; result: FrozenComparison };
  'map.list': { args: Record<string, never>; result: MapRow[] };
  'map.get': { args: { id: number }; result: ConceptMap };
  'map.create': { args: { name: string }; result: ConceptMap };
  'map.update': { args: { id: number; name?: string; notes?: string }; result: ConceptMap };
  'map.delete': { args: { id: number }; result: Record<string, never> };
  'map.node_add': { args: { map_id: number; kind: MapNodeKind; ref_id?: number; label?: string; x?: number; y?: number }; result: MapNode };
  'map.node_move': { args: { id: number; x: number; y: number }; result: Record<string, never> };
  'map.node_note': { args: { id: number; label: string }; result: Record<string, never> };
  'map.node_delete': { args: { id: number }; result: Record<string, never> };
  'map.link_add': { args: { map_id: number; from: number; to: number; label?: string; directed?: boolean }; result: MapLink };
  'map.link_update': { args: { id: number; label?: string; directed?: boolean }; result: MapLink };
  'map.link_delete': { args: { id: number }; result: Record<string, never> };
  'map.layout': { args: { map_id: number; how: MapLayout }; result: ConceptMap };
  'map.add_codes': { args: { map_id: number; code_ids: number[]; hierarchy?: boolean; min_together?: number }; result: AddedCodes };

  'code.list': { args: Record<string, never>; result: CodeRow[] };
  'code.add': {
    args: {
      name: string;
      parent_id?: number | null;
      theme?: boolean;
      color?: string;
      definition?: string;
      inclusion_criteria?: string;
      exclusion_criteria?: string;
      exclusive_group?: string;
      rated?: boolean;
      rating_label?: string;
      rating_low?: string;
      rating_high?: string;
    };
    result: CodeRow;
  };
  'code.update': { args: { id: number } & CodeEditArgs; result: CodeRow };
  'code.move': { args: { id: number; parent_id: number | null; position?: number }; result: Record<string, never> };
  'code.retire': { args: { id: number }; result: Record<string, never> };
  'code.restore': { args: { id: number }; result: Record<string, never> };
  'code.merge': { args: { from: number; into: number }; result: { codings_moved: number; children_moved: number } };
  'code.split': { args: { from: number; name: string; coding_ids: number[]; definition?: string }; result: CodeRow };
  'codebook.snapshot': { args: { note: string }; result: { version: number } };
  'codebook.versions': { args: Record<string, never>; result: CodebookVersion[] };
  'codebook.version': { args: { version: number }; result: CodebookSnapshot };

  'coder.list': { args: Record<string, never>; result: CoderRow[] };
  'template.list': { args: Record<string, never>; result: TemplateRow[] };
  'file.stat': { args: { path: string }; result: FileStat };
  'text.read': { args: { path: string }; result: TextFile };
  'text.write': { args: { path: string; text: string; replace?: boolean }; result: { path: string; bytes: number } };
  'analytics.words': { args: WordSpecArgs; result: WordFrequencies };
  'analytics.kwic': { args: ContextArgs; result: WordContexts };
  'stats.variables': { args: Record<string, never>; result: StatVariables };
  'stats.table': { args: { keys: string[]; include_descendants?: boolean }; result: StatTable };
  'export.script': { args: { path: string; overwrite?: boolean; script: string; data: string }; result: ExportResult & { data_path: string; data_bytes: number } };
  'stats.run': { args: StatRequest; result: StatResult };
  'stats.notebook': { args: Record<string, never>; result: StatNotebookPage[] };
  'stats.notebook.run': { args: { request: StatRequest; section?: string }; result: StatNotebookPage };
  'stats.notebook.note': { args: { title?: string; body?: string; section?: string }; result: StatNotebookPage };
  'stats.notebook.update': { args: { id: number; note?: string; section?: string; title?: string }; result: StatNotebookPage };
  'stats.notebook.remove': { args: { id: number }; result: Record<string, never> };
  'stats.notebook.move': { args: { id: number; up: boolean }; result: StatNotebookPage[] };
  'stats.notebook.rerun': { args: { id: number }; result: StatRerun };
  'stats.notebook.rerun_all': { args: Record<string, never>; result: StatRerun[] };
  'stats.notebook.previous': { args: { id: number }; result: { ran_at: string; result: StatResult } | null };

  'data.get': { args: Record<string, never>; result: CaseData };
  'variable.define': { args: VariableSpec & { existing?: string }; result: VariableDef };
  'variable.delete': { args: { name: string }; result: Record<string, never> };
  'variable.set_values': { args: { variable: string; changes: CellChange[] }; result: { changed: number } };
  'variable.transform': { args: TransformSpec; result: TransformOutcome };
  'variable.preview': { args: TransformSpec; result: TransformOutcome };
  'variable.recompute': { args: { name: string }; result: TransformOutcome };
  'joint.case_ordered': { args: CaseDisplaySpec; result: CaseDisplay };
  'joint.typology': { args: TypologySpec; result: Typology };
  'joint.side_by_side': { args: SideBySideSpec; result: SideBySide };
  'joint.deviation': { args: DeviationSpec; result: Deviation };
  'joint.similarity': { args: SimilaritySpec; result: Similarity };
  'joint.bubble': { args: BubbleSpec; result: Bubble };
  'mixed.design': { args: Record<string, never>; result: MixedDesign | null };
  'mixed.design.plan': { args: { kind: DesignKind }; result: DesignPlan };
  'mixed.design.apply': { args: DesignApplyArgs; result: DesignOutcome };
  'mixed.design.set': { args: DesignApplyArgs; result: MixedDesign };
  'mixed.design.clear': { args: Record<string, never>; result: Record<string, never> };
  'mixed.triangulation': { args: Record<string, never>; result: TriangulationRow[] };
  'mixed.triangulation.add': { args: TriangulationInput; result: TriangulationRow };
  'mixed.triangulation.update': { args: TriangulationInput & { id: number }; result: TriangulationRow };
  'mixed.triangulation.remove': { args: { id: number }; result: Record<string, never> };
  'mixed.triangulation.move': { args: { id: number; up: boolean }; result: Record<string, never> };
  'mixed.integration': { args: Record<string, never>; result: IntegrationEntry[] };
  'mixed.integration.add': { args: IntegrationInput; result: IntegrationEntry };
  'mixed.integration.update': { args: IntegrationInput & { id: number }; result: IntegrationEntry };
  'mixed.integration.remove': { args: { id: number }; result: Record<string, never> };
  'mixed.qualitize.preview': { args: QualitizeSpec; result: QualitizePlan };
  'mixed.qualitize': { args: QualitizeSpec; result: QualitizeOutcome };
  'mixed.qualitize.take_back': { args: { parent_id: number; code_ids: number[]; coding_ids: number[] }; result: QualitizeTakeBack };
  'variable.rule': { args: { formula: string }; result: { matched: number; total: number; case_ids: number[] } };
  'theme.overview': { args: { scope?: ExploreScope }; result: ThemeOverview };
  'theme.map': { args: Record<string, never>; result: { map_id: number } };
  'theme.ai_packet': { args: Record<string, never>; result: ThemePacket };
  'theme.ai_answer': { args: { run_id: number; model: string; answer: string }; result: ThemeAnswerOutcome };
  'theme.ai_run': { args: { run_id: number }; result: ThemeRun };
  'theme.ai_decide': { args: { run_id: number; index: number; accept: boolean; name?: string }; result: ThemeRun };
  'privacy.set_offline_lock': { args: { on: boolean }; result: ProjectInfo };
  'link.add': { args: { from_coding: number; to_coding: number; relation: string }; result: { id: number } };
  'link.update': { args: { id: number; relation: string }; result: Record<string, never> };
  'link.delete': { args: { id: number }; result: Record<string, never> };
  'link.list': { args: { coding_id?: number }; result: PassageLink[] };
  'link.relations': { args: Record<string, never>; result: LinkRelations };
  'packet.make': { args: { path: string; coder_name: string; source_ids?: number[]; instructions?: string; include_codings?: boolean }; result: CoderPacketReport };
  'adjudication.final_summary': { args: Record<string, never>; result: FinalSummary };
  'merge.preview': { args: { path: string; passphrase?: string } & MergeChoices; result: MergeReport };
  'merge.run': { args: { path: string; passphrase?: string } & MergeChoices; result: MergeReport };
  'adjudication.queue': { args: { coder_ids: number[]; blind?: boolean }; result: AdjudicationItem[] };
  'adjudication.region_queue': { args: Record<string, never>; result: RegionCase[] };
  'adjudication.decide_region': { args: { key: number; accept: boolean; reason: string; adjudicator_id?: number }; result: RegionCase };
  'adjudication.decide': { args: { unit_id: number; code_ids: number[]; reason: string; coder_id?: number }; result: AdjudicationDecision };
  'adjudication.list': { args: Record<string, never>; result: (AdjudicationDecision & { source_id: number; source_name: string; start: number; end: number; text: string })[] };
  'reliability.run': { args: AgreementArgs; result: AgreementReport };
  'reliability.list': { args: Record<string, never>; result: AgreementRunRow[] };
  'reliability.get': { args: { id: number }; result: AgreementRunRecord };
  'reliability.archive': { args: { id: number }; result: Record<string, never> };
  'sample.draw': { args: { size: number; seed?: number; source_ids?: number[] }; result: DrawnSample };
  'sample.list': { args: Record<string, never>; result: SampleRow[] };
  'sample.create': { args: { id: string; path: string }; result: ProjectInfo };
  'coding.list': { args: { source_id: number }; result: CodingRow[] };
  'coding.apply': {
    args: { source_id: number; start: number; end: number; code_id: number; coder_id?: number; rationale?: string; unit_id?: number | null; replace_exclusive?: boolean };
    result: { id: number; quote: string };
  };
  'coding.remove': { args: { id: number }; result: Record<string, never> };
  'coding.change_code': { args: { id: number; code_id: number }; result: Record<string, never> };
  'coding.rationale': { args: { id: number; rationale: string }; result: Record<string, never> };
  'coding.anchor': { args: { id: number; anchor: boolean }; result: Record<string, never> };
  /** Marks (or unmarks) a coding made with another code as a counterexample of `code_id`. */
  'coding.counterexample': { args: { code_id: number; id: number; on: boolean }; result: Record<string, never> };
  'code.counterexamples': { args: { id: number }; result: SegmentRow[] };
  /** How many passages already have two codes of one exclusive group at the same time. */
  'code.group_check': { args: { group: string }; result: { violations: number; first: [number, number][] } };
  'coding.rating': { args: { id: number; rating: number | null }; result: Record<string, never> };
  'coding.history': { args: { id: number }; result: CodingHistoryEvent[] };
  'case.add': { args: { name: string; notes?: string }; result: CaseRow };
  'case.assign': { args: { case_id: number; source_id: number }; result: Record<string, never> };
  'case.unassign': { args: { case_id: number; source_id: number }; result: Record<string, never> };

  'memo.list': { args: { scope?: MemoScope; target_id?: number }; result: MemoRow[] };
  'memo.add': {
    args: { kind: MemoKind; scope: MemoScope; target_id?: number | null; phase?: string | null; title?: string; body?: string };
    result: { id: number };
  };
  'memo.update': { args: { id: number; title?: string; body?: string }; result: Record<string, never> };
  'memo.delete': { args: { id: number }; result: Record<string, never> };

  'export.codebook': { args: ExportArgs; result: ExportResult };
  'export.segments': { args: ExportArgs; result: ExportResult };
  'export.report': { args: ReportArgs; result: ExportResult };
  'export.qdpx': { args: { path: string; overwrite?: boolean; include_recordings?: boolean }; result: ExportResult };
  'export.pstat': { args: { path: string; overwrite?: boolean; leave_out_names?: boolean; include_descendants?: boolean; app_version?: string }; result: { path: string; bytes: number; report: PstatReport } };
  'import.qdc': { args: { path: string }; result: { report: QdcImportReport } };
  'import.qdpx': { args: { path: string; project_path: string; name?: string }; result: { project_path: string; report: QdpxReport } };
  'export.notebook': { args: { path: string; overwrite?: boolean; format: NotebookFileFormat; notebook: NotebookPayload }; result: ExportResult };
  'export.picture': { args: { path: string; overwrite?: boolean; format: 'png' | 'svg'; data: string }; result: ExportResult };
  'class.packets': { args: { folder: string; students: string[]; source_ids?: number[]; instructions?: string }; result: ClassPacketsReport };
  'class.collect': { args: { folder: string; passphrase?: string }; result: ClassCollectReport };
  'class.review': { args: ClassReviewArgs; result: ClassReview };
  'export.class_review': { args: ClassReviewArgs & { path: string; overwrite?: boolean }; result: ExportResult & { rows: number } };
  'export.class_feedback': { args: ClassReviewArgs & { folder: string; overwrite?: boolean }; result: { folder: string; written: number; files: { student: string; path: string; bytes: number; passages: number }[]; notes: string[] } };
  /** One coded stretch of a recording saved as a WAV file; `data` is the file, as base 64 (the window cuts it from the decoded sound). */
  'export.clip': { args: { path: string; overwrite?: boolean; data: string }; result: ExportResult };
  'deid.state': { args: Record<string, never>; result: DeidState };
  'deid.add': { args: DeidNew; result: { added: DeidRule[]; already_there: string[] } };
  'deid.update': { args: { id: number } & Partial<Pick<DeidRule, 'term' | 'replacement' | 'kind' | 'match_case' | 'whole_word' | 'note'>>; result: DeidRule };
  'deid.remove': { args: { ids: number[] }; result: { removed: number } };
  'deid.set_exports': { args: { on: boolean }; result: DeidState };
  'deid.suggest': { args: { source_ids?: number[]; limit?: number }; result: { suggestions: DeidSuggestion[] } };
  'deid.preview': { args: { source_ids?: number[] }; result: DeidPreview };
  'deid.leftovers': { args: Record<string, never>; result: { places: DeidPlace[] } };
  'deid.key': { args: { path: string; overwrite?: boolean }; result: { path: string; bytes: number; rows: number } };
  'deid.load_key': { args: { path: string }; result: { added: number; already_there: string[] } };
  'deid.apply': { args: { source_ids?: number[]; key_path?: string; overwrite?: boolean }; result: DeidOutcome };
  'deid.copy': { args: { path: string; key_path?: string; overwrite_key?: boolean }; result: DeidCopyOutcome };

  undo: { args: Record<string, never>; result: UndoResult };
  'undo.next': { args: Record<string, never>; result: { next: string | null } };
  'search.run': { args: { query: string; source_ids?: number[]; limit?: number }; result: SearchResult };
  'autocode.preview': { args: AutocodeArgs; result: AutocodePreview };
  'autocode.propose': { args: AutocodeArgs; result: AutocodeProposed };
  'autocode.speakers': { args: AutoScope; result: SpeakerGroup[] };
  'autocode.headings': { args: AutoScope; result: { levels: HeadingLevel[] } };
  'proposal.list': { args: { status?: ProposalStatus; batch?: string; limit?: number; offset?: number }; result: ProposalRow[] };
  'proposal.batches': { args: Record<string, never>; result: BatchSummary[] };
  'proposal.decide': { args: { ids?: number[]; batch?: string; accept: boolean }; result: DecisionOutcome };
  'proposal.adjust': { args: { id: number; start: number; end: number; code_id: number }; result: ProposalRow };
  'ai.packet': { args: PacketArgs; result: AiPacket };
  'ai.answer': { args: { run_id: number; model: string; answer: string }; result: AiAnswerOutcome };
  'ai.runs': { args: Record<string, never>; result: AiRunRow[] };
  'ai.run': { args: { id: number }; result: AiRunDetail };
  'ai.disclosure': { args: Record<string, never>; result: { text: string } };
  'ai.set_mode': { args: { mode: 'packet' | 'off' }; result: ProjectInfo };
  'source.set_do_not_share': { args: { id: number; do_not_share: boolean }; result: Record<string, never> };
  'segments.list': { args: SegmentsArgs; result: SegmentsResult };

  'events.list': { args: { limit?: number }; result: EventRow[] };
  'events.annotate': { args: { note: string }; result: Record<string, never> };
}

export type CommandName = keyof Commands;
