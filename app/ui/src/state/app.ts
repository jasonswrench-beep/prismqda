// Everything the screens share: the open project, its sources and codes, and what is selected.

import { OffsetIndex } from '../offsets';
import type { RetrievalMode } from '../lib/retrieval';
import type { Coverage } from '../lib/coverage';
import type { Favorites } from '../lib/favorites';
import { normalizeSettings, SETTINGS_KEY, type Settings } from '../lib/settings';
import { MAX_RECENT, OLD_RECENT_KEY, RECENT_KEY, migrateOldRecent, normalizeRecent, type RecentEntry } from '../lib/recent';
import { readJson } from '../lib/storage';
import { EVERYTHING, type WorkingSet } from '../lib/workingSet';
import type { Selection, TargetResult, TargetSpan } from '../lib/targets';
import { emptyDraft, type CrosstabPercent, type Draft, type PercentMode } from '../lib/explore';
import { emptySetup, type AutoSetup } from '../lib/autocode';
import { emptyAiSetup, type AiSetup } from '../lib/ai';
import type { ReportSectionId, AiAnswerOutcome, AiPacket, CaseRow, CodeRow, CoderRow, CodingRow, FormatRun, ImportWarning, Metric, PdfRow, ProjectInfo, QueryResult, AreaShape, SourceImage, SourceRow, ThemePacket, ThemeRun, UnitRow } from '../types';
import { createStore, useStore } from './store';
import { t } from '../i18n';

/** The places a project can be worked in. Built ones only; the interface design lists the rest (docs/INTERFACE.md). */
export type Workspace = 'home' | 'data' | 'import' | 'code' | 'codebook' | 'themes' | 'explore' | 'autocode' | 'ai' | 'team' | 'text' | 'stats' | 'variables' | 'mixed' | 'synthesize' | 'report' | 'privacy' | 'settings';

/** The two halves of the Auto Code page: setting a run up, and reviewing what it suggested. */
export type AutoCodeTab = 'setup' | 'queue';

/** The halves of the AI page: building a packet and pasting the answer back, and the log of every run. */
export type AiTab = 'packet' | 'log';

/** The packet being worked on: the form, the last packet built, and the answer being pasted for it. */
export interface AiState {
  tab: AiTab;
  setup: AiSetup;
  packet: AiPacket | null;
  /** The working set when the packet was built, to say when it has changed since. */
  packetSet: WorkingSet | null;
  /** What the person typed for the model's name and its answer. */
  model: string;
  answer: string;
  /** What became of the last answer pasted, for the packet that stays on screen. */
  outcome: AiAnswerOutcome | null;
  /** The run the log shows in full. */
  openRun: number | null;
}

export function emptyAiState(): AiState {
  return { tab: 'packet', setup: emptyAiSetup(), packet: null, packetSet: null, model: '', answer: '', outcome: null, openRun: null };
}

/**
 * Asking an AI for candidate themes (decision 168): the packet last built, the model's name and the answer being pasted, and the run being
 * decided. Kept in the app state and not in the component, so that leaving the Themes page does not lose an answer that was being pasted.
 */
export interface ThemeAiState {
  packet: ThemePacket | null;
  run: ThemeRun | null;
  model: string;
  answer: string;
}

export function emptyThemeAi(): ThemeAiState {
  return { packet: null, run: null, model: '', answer: '' };
}

/** The tools on the Explore page, in the order of the book's tab row. */
export type ExploreTool = 'query' | 'matrix' | 'crosstab' | 'relations' | 'summary' | 'visualize' | 'map' | 'portrait';

/** How the code matrix (and the charts drawn from it) is set up; it is kept while moving between the tools. */
export interface MatrixSetup {
  rowKind: 'cases' | 'documents' | 'variable' | 'months';
  /** The case variable, when the rows are the values of one. */
  variable: string;
  codeIds: number[];
  descendants: boolean;
  metric: Metric;
  /** Coders to look at; empty for all. */
  coderIds: number[];
  /** Whether the table shows numbers only or each as a percentage of a stated whole. */
  percent: PercentMode;
}

/** The concept map that is open, and the node chosen on it. */
export interface MapState {
  mapId: number | null;
  selected: number | null;
}

/** How the crosstab is set up; it is kept while moving between the tools. */
export interface CrosstabSetup {
  row: string;
  column: string;
  /** Group a side's numbers into ranges this wide (as typed; empty for none). */
  rowWidth: string;
  columnWidth: string;
  percent: CrosstabPercent;
}

/** The query being built, and the results of the last time it was run (with the draft they came from). */
export interface QueryState {
  draft: Draft;
  ran: { key: string; result: QueryResult; at: number; workingSet?: WorkingSet } | null;
  /** The saved query this draft was opened from or saved as, with the draft's key at that moment (to tell when it has changed since). */
  saved: { id: number; key: string; name: string; frozen: boolean } | null;
}
export type HomeView = 'overview' | 'activity';
/** How a document is shown: as words (document, grid, thread), or, for a PDF that keeps its file, as its pages. */
export type ViewKind = 'document' | 'grid' | 'thread' | 'pages';
export type DetailTab = 'coding' | 'code' | 'memos';

/** The tabs of the ribbon row, in the order of the screen specification book. A tab changes which commands are shown, nothing else. */
export type RibbonTabId = 'home' | 'import' | 'code' | 'memos' | 'explore' | 'mixed' | 'visual' | 'synthesize' | 'text' | 'stats' | 'report' | 'team' | 'ai' | 'view';

/** Where the project file stands: every change written, a write in progress, or a write that did not succeed. */
export type SaveState = 'saved' | 'saving' | 'failed';

/** The four areas of the code workspace that can be hidden and brought back from the View tab. */
export interface PaneVisibility {
  documents: boolean;
  codes: boolean;
  results: boolean;
  details: boolean;
}

/** A source opened in the center pane, with everything needed to draw it. */
export interface SourceData {
  id: number;
  name: string;
  text: string;
  index: OffsetIndex;
  runs: FormatRun[];
  report: ImportWarning[];
  units: UnitRow[];
  /** The codings of the document: for a picture or a PDF, its coded areas too, each numbered with the area's number made negative. */
  codings: CodingRow[];
  textRevision: number;
  /** Set when the document is a picture. */
  image: SourceImage | null;
  /** Set when the document keeps the PDF it was read from: the size of each page. */
  pdf: PdfRow | null;
}

/** A coding the engine refused because its code shares a mutually exclusive group with a code already on those words; the person decides whether to replace. */
export interface PendingReplace {
  codeId: number;
  /** The engine's own sentence about the clash. */
  message: string;
  /** What is still to be coded (the spans before the clash were already coded). */
  spans: TargetSpan[];
  skipped: number;
}

export type DialogState =
  | { kind: 'import-folder' }
  | { kind: 'replace-exclusive'; pending: PendingReplace }
  | { kind: 'counterexample'; codeId: number }
  | { kind: 'export'; what?: 'codebook' | 'segments' | 'report' | 'project' | 'casetable'; sections?: ReportSectionId[] }
  | { kind: 'import-qdpx' }
  | { kind: 'import-qdc' }
  | { kind: 'link-passage'; codingId: number }
  | { kind: 'working-set' }
  | { kind: 'new-theme'; codeIds: number[] }
  | { kind: 'palette'; mode?: 'commands' | 'search' }
  | { kind: 'help' }
  | { kind: 'pick-code' }
  | { kind: 'codebook-compare' }
  | { kind: 'new-project' }
  | { kind: 'open-project' }
  | { kind: 'locate-project'; path: string; name: string }
  | { kind: 'passphrase'; path: string }
  | { kind: 'sample-projects' }
  | { kind: 'add-language' }
  | { kind: 'translator-file'; id?: string }
  | { kind: 'new-case' }
  | { kind: 'new-code'; parentId: number | null }
  | { kind: 'merge'; codeId: number }
  | { kind: 'split'; codeId: number }
  | { kind: 'snapshot' };

export interface AppState {
  phase: 'loading' | 'blocked' | 'start' | 'workspace';
  /** Why the program could not start (for example the engine is not running). */
  bootError: string | null;
  project: ProjectInfo | null;
  sources: SourceRow[];
  codes: CodeRow[];
  coders: CoderRow[];
  /** The cases of the project, with the documents each holds. */
  cases: CaseRow[];
  currentSourceId: number | null;
  /** The documents that have a tab in the reader, in the order they were opened. */
  openTabs: number[];
  sourceData: SourceData | null;
  loadingSourceId: number | null;
  sourceError: string | null;
  viewChoice: Record<number, ViewKind>;
  /** The page of each PDF the pages view shows, by document (counting from 1; a PDF not in here is at its first page). */
  pdfPage: Record<number, number>;
  /** The size of every page of the PDFs this session has looked at, by document, for drawing areas of pages in lists. */
  pdfInfo: Record<number, PdfRow>;
  /** Large sources open in the document view only after the person agrees. */
  allowBigDocument: Record<number, boolean>;
  selectedCodeId: number | null;
  selectedCodingId: number | null;
  detailTab: DetailTab;
  workspace: Workspace;
  exploreTool: ExploreTool;
  exploreMatrix: MatrixSetup;
  exploreQuery: QueryState;
  exploreCrosstab: CrosstabSetup;
  exploreMap: MapState;
  /** Which half of the Auto Code page is open. */
  autoCodeTab: AutoCodeTab;
  /** The Set up form, kept while moving between pages. */
  autoCodeSetup: AutoSetup;
  /** The batch the review queue shows; null for every batch that has something waiting. */
  autoCodeBatch: string | null;
  /** The AI page: the packet form, the packet built, and the answer being pasted. */
  ai: AiState;
  /** The Themes page's request for candidate themes from an AI. */
  themeAi: ThemeAiState;
  /** Split view: the document shown, read-only, beside the one being read; null when split view is off. */
  splitSourceId: number | null;
  /** The turn of a recording that is playing: the text scrolls to it (without taking the focus) while the person follows the recording. */
  mediaFollow: { sourceId: number; offset: number; tick: number } | null;
  homeView: HomeView;
  /** Goes up whenever work done elsewhere (an undo, an import) means lists on screen should reload. */
  dataVersion: number;
  /** A place in a source to select as soon as that source's text is on screen (a search result, a retrieved passage). */
  pendingJump: { sourceId: number; start: number; end: number; codingId?: number } | null;
  /** The codes whose passages the lower pane shows. Choosing them to look at is not the same as selecting one to edit. */
  activeCodeIds: number[];
  /** The documents, cases, and codes the person has chosen to work with (decision 163); everything when nothing is narrowed. */
  workingSet: WorkingSet;
  /** Whether the lower pane looks in every source or only the open one. */
  retrievalMode: RetrievalMode;
  /** How Retrieved Segments shows what it found: a table of every segment, or a gallery of the areas of pictures, grouped by code. */
  retrievalView: 'list' | 'gallery';
  /** The case Retrieved Segments is narrowed to; null for every case. */
  retrievalCase: number | null;
  /** The words Retrieved Segments is searched for; empty for no search. */
  retrievalQuery: string;
  favorites: Favorites;
  lastCodeId: number | null;
  /** Codings marked as anchor examples (the engine does not list them, so they are tracked here). */
  anchors: ReadonlySet<number>;
  coverage: Record<number, Coverage>;
  showInactive: boolean;
  /** The person's own settings (theme, text size, what is said aloud), kept on this computer. */
  settings: Settings;
  /** Goes up when the document's lines are measured again, so anything that names lines draws again. */
  linesStamp: number;
  /** The recent projects on the launcher. */
  recent: RecentEntry[];
  /** Which page of the launcher shows: the welcome page, or Settings (reachable before a project exists). */
  launcherPage: 'home' | 'new' | 'settings';
  /** Which tab of the ribbon row is showing its commands. */
  ribbonTab: RibbonTabId;
  panes: PaneVisibility;
  saveState: SaveState;
  /** When the project was last written successfully, in milliseconds since 1970; null before the first write in this session. */
  savedAt: number | null;
  /** Why the last write failed, for the status bar. */
  saveError: string | null;
  /** An area drawn on the open picture that has no code yet: the code the person applies goes to it. */
  imageDraft: AreaShape | null;
  /** The stretch of a recording being marked, in milliseconds: where it starts and where it ends, each null until it is marked. */
  clipDraft: { start_ms: number | null; end_ms: number | null } | null;
  /** Something the open recording is asked to do from outside its player (move to a time, or play a stretch). `tick` makes a repeat count as new. */
  clipCommand: { sourceId: number; kind: 'seek' | 'play'; start_ms: number; end_ms: number; tick: number } | null;
  /** The current selection in the document, for the toolbar's "selected" line. */
  selection: Selection | null;
  selectionText: string;
  dialog: DialogState | null;
  /** A source to select after the import dialog closes. */
  busy: boolean;
}

/** The recent projects from storage; the list kept by the first alphas (paths only) is brought forward once. */
function loadRecent(): RecentEntry[] {
  const now = readJson<unknown>(RECENT_KEY, null);
  if (now !== null) return normalizeRecent(now).slice(0, MAX_RECENT);
  return migrateOldRecent(readJson<unknown>(OLD_RECENT_KEY, []), Date.now());
}

export const initialState: AppState = {
  phase: 'loading',
  bootError: null,
  project: null,
  sources: [],
  codes: [],
  coders: [],
  cases: [],
  currentSourceId: null,
  openTabs: [],
  sourceData: null,
  loadingSourceId: null,
  sourceError: null,
  viewChoice: {},
  pdfPage: {},
  pdfInfo: {},
  allowBigDocument: {},
  selectedCodeId: null,
  selectedCodingId: null,
  detailTab: 'coding',
  workspace: 'code',
  exploreTool: 'query',
  exploreMatrix: { rowKind: 'cases', variable: '', codeIds: [], descendants: true, metric: 'assignments', coderIds: [], percent: 'none' },
  exploreQuery: { draft: emptyDraft(), ran: null, saved: null },
  exploreCrosstab: { row: '', column: '', rowWidth: '', columnWidth: '', percent: 'none' },
  exploreMap: { mapId: null, selected: null },
  autoCodeTab: 'setup',
  autoCodeSetup: emptySetup(),
  autoCodeBatch: null,
  ai: emptyAiState(),
  themeAi: emptyThemeAi(),
  splitSourceId: null,
  mediaFollow: null,
  homeView: 'overview',
  dataVersion: 0,
  pendingJump: null,
  activeCodeIds: [],
  workingSet: EVERYTHING,
  retrievalMode: 'all',
  retrievalView: 'list',
  retrievalCase: null,
  retrievalQuery: '',
  favorites: [],
  lastCodeId: null,
  anchors: new Set(),
  coverage: {},
  showInactive: false,
  settings: normalizeSettings(readJson<unknown>(SETTINGS_KEY, null)),
  linesStamp: 0,
  recent: loadRecent(),
  launcherPage: 'home',
  ribbonTab: 'home',
  panes: { documents: true, codes: true, results: true, details: true },
  saveState: 'saved',
  savedAt: null,
  saveError: null,
  imageDraft: null,
  clipDraft: null,
  clipCommand: null,
  selection: null,
  selectionText: '',
  dialog: null,
  busy: false,
};

export const appStore = createStore<AppState>(initialState);

export function useApp<T>(selector: (s: AppState) => T): T {
  return useStore(appStore, selector);
}

/** What is selected in the center pane right now, supplied by whichever view is showing. */
export type SelectionProvider = () => TargetResult;
let provider: SelectionProvider | null = null;

export function setSelectionProvider(p: SelectionProvider | null): void {
  provider = p;
}

export function currentTarget(): TargetResult {
  return provider ? provider() : { ok: false, message: t('Open a source first.') };
}

// The center view tells the rest of the program how to put the focus back on it, and the toolbar
// does the same for its "Apply a code" box.
let centerFocus: ((keep?: Selection | null) => void) | null = null;

export function setCenterFocus(fn: ((keep?: Selection | null) => void) | null): void {
  centerFocus = fn;
}
/** Puts the focus back on the text. `keep` is the selection to give back, for a caller that moved the focus away and may have lost it. */
export function focusCenter(keep?: Selection | null): void {
  centerFocus?.(keep);
}
// The open document says how to read the words selected in it right now (the stored selection trails the live one by a frame).
let selectionReader: (() => Selection | null) | null = null;
let keptSelection: Selection | null = null;

export function setSelectionReader(fn: (() => Selection | null) | null): void {
  selectionReader = fn;
}
/** The words that were selected when Code Selection was opened; the dialog gives them back to the text when it closes. */
export function keptTextSelection(): Selection | null {
  return keptSelection;
}

/** Code Selection: opens the small dialog where a code is typed and applied (also what Enter does in the text). */
export function focusApplyBox(): void {
  // Read the selection now, before the focus moves into the dialog and the text stops being the active element.
  keptSelection = selectionReader?.() ?? appStore.get().selection;
  appStore.set({ dialog: { kind: 'pick-code' } });
}

// The open document (or grid) says how to jump to the next or previous unit that has no coding yet.
let jumpFn: ((direction: 1 | -1) => void) | null = null;
export function setJumpProvider(fn: ((direction: 1 | -1) => void) | null): void {
  jumpFn = fn;
}
export function jumpUncoded(direction: 1 | -1): void {
  jumpFn?.(direction);
}

// The document says which lines (as drawn on screen) a stretch of text runs over; the details panel shows them.
let lineProvider: ((start: number, end: number) => { first: number; last: number } | null) | null = null;

export function setLineProvider(fn: ((start: number, end: number) => { first: number; last: number } | null) | null): void {
  lineProvider = fn;
}
export function lineRangeOf(start: number, end: number): { first: number; last: number } | null {
  return lineProvider ? lineProvider(start, end) : null;
}

/** Moves the focus to one of the main areas by id (pane-sources, pane-center, pane-codebook, pane-details). */
export function focusPane(id: string): void {
  const el = document.getElementById(id);
  if (el) el.focus();
}

/** Thresholds beyond which the Grid view is the default (specification: large sources). */
export const BIG_UNITS = 1000;
export const BIG_CHARACTERS = 400_000;

export function isBigSource(s: Pick<SourceRow, 'units' | 'characters'>): boolean {
  return s.units > BIG_UNITS || s.characters > BIG_CHARACTERS;
}
