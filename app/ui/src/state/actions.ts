// Everything the person can do, as functions: each calls the engine, updates the shared state, and
// says what happened (the status line and screen readers). Screens call these and render state.

import { ApiError, call, setWriteObserver, toApiError } from '../bridge';
import { codedSentence } from '../lib/eventWording';
import { computeCoverage } from '../lib/coverage';
import { MAX_FAVORITES, normalizeFavorites, pin, unpin, type Favorites } from '../lib/favorites';
import { colorName, nextColor } from '../lib/labels';
import { toggleId, type RetrievalMode } from '../lib/retrieval';
import { activateLocale, chooseLocale, listLanguages } from '../i18n/locales';
import { bumpLanguages, removeTranslation } from '../i18n/manage';
import { applySettings, DEFAULT_SETTINGS, normalizeSettings, SETTINGS_KEY, themeLabel, type Settings } from '../lib/settings';
import { forgetRecent, RECENT_KEY, rememberRecent, type RecentEntry } from '../lib/recent';
import { readJson, removeKey, writeJson } from '../lib/storage';
import type { MovePlan } from '../lib/tree';
import { describeCount } from '../lib/targets';
import { nextUncodedUnit, unitIndexForCaret, unitIndicesTouching } from '../lib/units';
import { OffsetIndex, sliceScalar } from '../offsets';
import { regionToCoding, sameShape } from '../lib/regions';
import { notYetCoded, sameWordsWith } from '../lib/recode';
import { clockText, orderedSpan, spanProblem, spanShort, spanWords } from '../lib/clips';
import type { AreaShape, SpanShape } from '../types';
import type { AiAnswerOutcome, AiPacket, AutocodeArgs, ImportOptionsArgs, ReviseResult, AutocodeProposed, CodeEditArgs, CodeRow, CodingRow, DecisionOutcome, PacketArgs, ProjectInfo, ProposalRow, SourceRow } from '../types';
import { emptySetup, type AutoSetup } from '../lib/autocode';
import type { AiSetup } from '../lib/ai';
import { defaultSplitTarget } from '../lib/split';
import { announce } from './announce';
import { appStore, currentTarget, emptyAiState, emptyThemeAi, isBigSource, setSelectionProvider, type AiState, type AiTab, type AutoCodeTab, type CrosstabSetup, type ExploreTool, type MapState, type MatrixSetup, type PaneVisibility, type PendingReplace, type QueryState, type RibbonTabId, type SourceData, type ViewKind } from './app';
import { emptyDraft } from '../lib/explore';
import { EVERYTHING } from '../lib/workingSet';
import { analysisDefaults, statsStore } from './statistics';
import { t, tn } from '../i18n';
import { engineText } from '../i18n/engine';

// ---------------------------------------------------------------- the Saved indicator

// Every write to the project file moves the indicator: Saving while it runs, Saved with the time when it is done, and
// Failed (with the reason) when the file or the engine could not take it. A refusal the person can fix leaves it alone.
setWriteObserver((e) => {
  if (e.phase === 'saving') appStore.set({ saveState: 'saving' });
  else if (e.phase === 'saved') appStore.set({ saveState: 'saved', savedAt: Date.now(), saveError: null });
  else if (e.phase === 'unchanged') appStore.set((s) => ({ saveState: s.saveError ? 'failed' : 'saved' }));
  else {
    appStore.set({ saveState: 'failed', saveError: e.error.message });
    announce(t('The last change could not be saved: {message}', { message: e.error.message }), 'assertive');
  }
});

/** The Save command. Every change is written as it is made, so there is nothing waiting; it says so, and shows a failed write again. */
export function saveProject(): void {
  const s = appStore.get();
  if (s.saveState === 'failed') announce(t('The last change could not be saved: {saveError}. Try the change again.', { saveError: s.saveError ?? t('the file could not be written') }), 'assertive');
  else announce(t('Nothing to save: every change is written as you make it.'));
}

export function setRibbonTab(tab: RibbonTabId): void {
  appStore.set({ ribbonTab: tab });
}

export function setPane(pane: keyof PaneVisibility, visible: boolean): void {
  appStore.set((s) => ({ panes: { ...s.panes, [pane]: visible } }));
}

// ---------------------------------------------------------------- small helpers

/** Runs a step; if it fails, says why and returns undefined. */
export async function attempt<T>(fn: () => Promise<T>): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return undefined;
  }
}

function favoritesKey(uid: string): string {
  return t('prismqda.favorites.{uid}', { uid });
}

function saveRecent(list: RecentEntry[]): void {
  appStore.set({ recent: list });
  writeJson(RECENT_KEY, list);
}

/** Puts a project that opened successfully at the top of the recent list. A failed open never gets here, so a bad file is never remembered. */
function rememberProject(path: string, info: ProjectInfo): void {
  saveRecent(rememberRecent(appStore.get().recent, { path, name: info.name, uid: info.project_uid, openedAt: Date.now() }));
}

/** Takes a project off the recent list. Only the list changes; the file stays where it is. */
export function removeRecentProject(path: string): void {
  const name = appStore.get().recent.find((r) => r.path === path)?.name ?? t('The project');
  saveRecent(forgetRecent(appStore.get().recent, path));
  announce(t('{name} was removed from the recent list. Its file was not touched.', { name }));
}

/**
 * Opens a project from its new place after it was moved. The project's identity is checked against the recent entry
 * first, so a different project is never filed under the old name; only after that is the saved path updated.
 */
export async function locateRecentProject(oldPath: string, newPath: string): Promise<void> {
  const entry = appStore.get().recent.find((r) => r.path === oldPath);
  const info = await call('project.open', { path: newPath });
  if (entry?.uid && info.project_uid !== entry.uid) {
    await call('project.close');
    throw new ApiError('wrong_project', t('That file is the project {name}, not {name2}. The recent list was left as it was.', { name: info.name, name2: entry.name }));
  }
  saveRecent(rememberRecent(forgetRecent(appStore.get().recent, oldPath), { path: newPath, name: info.name, uid: info.project_uid, openedAt: Date.now() }));
  await enterWorkspace(info);
  announce(t('Found {name} at its new place and opened it.', { name: info.name }));
}

/** The ".pqda" extension is added when the person types a name without one. */
export function withProjectExtension(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  return name.includes('.') ? path : `${path}.pqda`;
}

const textCache = new Map<number, { revision: number; text: string; index: OffsetIndex }>();

/** The text of any source, fetched once. Used to show quotes in the history. */
export async function sourceText(sourceId: number): Promise<{ text: string; index: OffsetIndex }> {
  const open = appStore.get().sourceData;
  if (open && open.id === sourceId) return { text: open.text, index: open.index };
  const cached = textCache.get(sourceId);
  if (cached) return cached;
  const detail = await call('source.get', { id: sourceId });
  const entry = { revision: detail.text_revision, text: detail.text, index: new OffsetIndex(detail.text) };
  textCache.set(sourceId, entry);
  return entry;
}

export function quoteFromOpenSource(sourceId: number, start: number, end: number): string | null {
  const d = appStore.get().sourceData;
  if (d && d.id === sourceId) return sliceScalar(d.text, d.index, start, end);
  const c = textCache.get(sourceId);
  return c ? sliceScalar(c.text, c.index, start, end) : null;
}

// ---------------------------------------------------------------- starting and stopping

let coverageRun = 0;

export async function initApp(): Promise<void> {
  try {
    await tellEngineBackups(appStore.get().settings.keepBackups);
    await tellEngineBackupFolder(appStore.get().settings.backupFolder);
    await tellEngineName(appStore.get().settings.historyName);
    const info = await call('project.info');
    await enterWorkspace(info);
  } catch (e) {
    const err = toApiError(e);
    // No project open is the normal start. Anything else means the engine could not be reached: the splash says so, with Retry.
    appStore.set(err.code === 'no_project' ? { phase: 'start', bootError: null } : { phase: 'blocked', bootError: err.message });
  }
}

async function enterWorkspace(info: ProjectInfo): Promise<void> {
  textCache.clear();
  const stored = readJson<unknown>(favoritesKey(info.project_uid), []);
  appStore.set({
    saveState: 'saved',
    savedAt: Date.now(),
    saveError: null,
    ribbonTab: 'home',
    phase: 'workspace',
    bootError: null,
    project: info,
    sources: [],
    codes: [],
    coders: [],
    currentSourceId: null,
    openTabs: [],
    sourceData: null,
    loadingSourceId: null,
    sourceError: null,
    selectedCodeId: null,
    selectedCodingId: null,
    detailTab: 'coding',
    workspace: 'code',
    exploreTool: 'query',
    exploreMatrix: { rowKind: 'cases', variable: '', codeIds: [], descendants: appStore.get().settings.subcodes, metric: 'assignments', coderIds: [], percent: 'none' },
    exploreQuery: { draft: { ...emptyDraft(), descendants: appStore.get().settings.subcodes }, ran: null, saved: null },
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
    activeCodeIds: [],
    workingSet: EVERYTHING,
    retrievalMode: 'all',
    retrievalView: 'list',
    favorites: normalizeFavorites(stored),
    lastCodeId: null,
    anchors: new Set(),
    coverage: {},
    selection: null,
    selectionText: '',
    dialog: null,
    viewChoice: {},
    allowBigDocument: {},
  });
  await Promise.all([refreshSources(), refreshCodes(), refreshCoders()]);
  // The cases only fill in a column and a filter, so the workspace does not wait for them: a survey with thousands of
  // respondents makes thousands of cases, and listing them must not hold up the first screen.
  void refreshCases();
  const ids = new Set(appStore.get().codes.map((c) => c.id));
  appStore.set({ favorites: normalizeFavorites(stored, ids) });
  void loadAnchors();
  const first = appStore.get().sources[0];
  if (first) await selectSource(first.id);
  void measureCoverage();
}

export async function createProject(args: { name: string; path: string; mode: 'free_span' | 'unit'; researchQuestions?: string; method?: string; passphrase?: string }): Promise<void> {
  const path = withProjectExtension(args.path);
  const info = await call('project.create', {
    path,
    name: args.name.trim(),
    coding_mode: args.mode,
    ...(args.method ? { method: args.method } : {}),
    ...(args.researchQuestions?.trim() ? { research_questions: args.researchQuestions.trim() } : {}),
    ...(args.passphrase ? { passphrase: args.passphrase } : {}),
  });
  rememberProject(path, info);
  await enterWorkspace(info);
  announce(t('Created the project {name}.', { name: info.name }));
}

/** Makes an editable copy of a bundled sample at `path` and opens it. The bundled sample itself is never touched. */
export async function createSampleCopy(id: string, path: string): Promise<void> {
  const target = withProjectExtension(path);
  const info = await call('sample.create', { id, path: target });
  rememberProject(target, info);
  await enterWorkspace(info);
  announce(t('Made your practice copy {name} and opened it. The sample itself is unchanged.', { name: info.name }));
}

/**
 * Opens a project. An encrypted one asks for its passphrase first: without one, the engine says so and the passphrase window opens for this
 * path (which then calls this again with what was typed). A wrong passphrase is an error the window shows.
 */
export async function openProject(path: string, passphrase?: string): Promise<void> {
  let info: ProjectInfo;
  try {
    info = await call('project.open', { path, ...(passphrase !== undefined ? { passphrase } : {}) });
  } catch (e) {
    if (passphrase === undefined && toApiError(e).code === 'needs_passphrase') {
      appStore.set({ dialog: { kind: 'passphrase', path } });
      return;
    }
    throw e;
  }
  rememberProject(path, info);
  await enterWorkspace(info);
  announce(t('Opened the project {name}.', { name: info.name }));
}

export async function closeProject(): Promise<void> {
  coverageRun++;
  await call('project.close');
  setSelectionProvider(null);
  appStore.set({ phase: 'start', project: null, sourceData: null, currentSourceId: null, openTabs: [], sources: [], codes: [], splitSourceId: null, workingSet: EVERYTHING, themeAi: emptyThemeAi() });
  announce(t('Closed the project.'));
}

// ---------------------------------------------------------------- lists

export async function refreshSources(): Promise<void> {
  const sources = await call('source.list');
  appStore.set({ sources });
}

export async function refreshCodes(): Promise<void> {
  const codes = await call('code.list');
  appStore.set({ codes });
}

/** Loads the project's own facts again (an undo can change a setting such as the AI one). A failure leaves what is shown. */
export async function refreshProjectInfo(): Promise<void> {
  try {
    const info = await call('project.info');
    appStore.set({ project: info });
  } catch {
    /* the project facts stay as they were */
  }
}

export async function refreshCoders(): Promise<void> {
  const coders = await call('coder.list');
  appStore.set({ coders });
}

/** Loads the cases. A failure here only means the case line is left out of the reader's header. */
export async function refreshCases(): Promise<void> {
  try {
    appStore.set({ cases: await call('case.list') });
  } catch {
    appStore.set({ cases: [] });
  }
}

/** The engine does not say which codings are examples, so replay the audit trail to find out. */
async function loadAnchors(): Promise<void> {
  try {
    const events = await call('events.list', { limit: 20000 });
    const anchors = new Set<number>();
    for (const e of events) {
      if (e.target_id === null) continue;
      if (e.action === 'anchor.add') anchors.add(e.target_id);
      else if (e.action === 'anchor.remove') anchors.delete(e.target_id);
      else if (e.action === 'coding.remove') anchors.delete(e.target_id);
    }
    appStore.set({ anchors });
  } catch {
    // Anchors simply start empty; marking one still works.
  }
}

/** Fills in the "how much is coded" bar for every source that has codings, one at a time. */
async function measureCoverage(): Promise<void> {
  const run = ++coverageRun;
  const mode = appStore.get().project?.coding_mode ?? 'free_span';
  // A picture has no text to count the coded part of.
  const todo = appStore.get().sources.filter((s) => s.codings > 0 && !s.image && !s.recording?.stands_alone && !appStore.get().coverage[s.id]);
  let batch: Record<number, ReturnType<typeof computeCoverage>> = {};
  let count = 0;
  for (const s of todo) {
    if (run !== coverageRun) return;
    try {
      const codings = await call('coding.list', { source_id: s.id });
      batch[s.id] = computeCoverage(s, codings, mode);
    } catch {
      // Leave this source without a bar; the list still shows its coding count.
    }
    if (++count % 8 === 0) {
      appStore.set((st) => ({ coverage: { ...st.coverage, ...batch } }));
      batch = {};
    }
  }
  if (run === coverageRun) appStore.set((st) => ({ coverage: { ...st.coverage, ...batch } }));
}

function recomputeCurrentCoverage(): void {
  const st = appStore.get();
  const d = st.sourceData;
  const src = st.sources.find((s) => s.id === d?.id);
  if (!d || !src || !st.project || d.image || src.recording?.stands_alone) return;
  // The coded areas of a PDF's pages and the coded stretches of a recording are not words, so they are not part of how much of the text is coded.
  const cov = computeCoverage(src, d.codings.filter((c) => !c.region), st.project.coding_mode, d.units);
  appStore.set((s) => ({ coverage: { ...s.coverage, [d.id]: cov } }));
}

// ---------------------------------------------------------------- sources

export function defaultViewFor(source: Pick<SourceRow, 'units' | 'characters'>, hasReplies: boolean): ViewKind {
  if (hasReplies) return 'thread';
  return isBigSource(source) ? 'grid' : 'document';
}

export async function selectSource(id: number): Promise<void> {
  const st = appStore.get();
  if (st.currentSourceId === id && st.sourceData?.id === id) return;
  appStore.set((s) => ({
    currentSourceId: id,
    openTabs: s.openTabs.includes(id) ? s.openTabs : [...s.openTabs, id],
    loadingSourceId: id,
    sourceError: null,
    selectedCodingId: null,
    imageDraft: null,
    clipDraft: null,
    selection: null,
    selectionText: '',
  }));
  try {
    const row = st.sources.find((s) => s.id === id);
    const image = row?.image ?? null;
    const [detail, units, codings, pdf] = await Promise.all([
      call('source.get', { id }),
      call('unit.list', { source_id: id }),
      codingsOf(id, image !== null || !!row?.pdf || !!row?.recording),
      row?.pdf ? call('pdf.get', { source_id: id }) : Promise.resolve(null),
    ]);
    if (appStore.get().currentSourceId !== id) return;
    if (pdf) appStore.set((s) => ({ pdfInfo: { ...s.pdfInfo, [id]: pdf } }));
    const data: SourceData = {
      id,
      name: detail.name,
      text: detail.text,
      index: new OffsetIndex(detail.text),
      runs: detail.formatting_runs ?? [],
      report: detail.import_report ?? [],
      units,
      codings,
      textRevision: detail.text_revision,
      image,
      pdf,
    };
    appStore.set({ sourceData: data, loadingSourceId: null });
    recomputeCurrentCoverage();
  } catch (e) {
    appStore.set({ sourceData: null, loadingSourceId: null, sourceError: toApiError(e).message });
  }
}

/**
 * Closes the tab of a document. If it was the one showing, the neighbor to its right (or left, at the end) is shown;
 * with no tab left the reader is empty and the person can choose from Documents.
 */
export function closeTab(id: number): void {
  const st = appStore.get();
  const at = st.openTabs.indexOf(id);
  if (at < 0) return;
  const tabs = st.openTabs.filter((t) => t !== id);
  appStore.set({ openTabs: tabs });
  if (st.currentSourceId !== id) return;
  const next = tabs[at] ?? tabs[at - 1] ?? null;
  if (next !== null) void selectSource(next);
  else appStore.set({ currentSourceId: null, sourceData: null, loadingSourceId: null, sourceError: null, selectedCodingId: null, imageDraft: null, selection: null, selectionText: '' });
  announce(st.sources.find((s) => s.id === id)?.name ? t('Closed the tab {name}.', { name: st.sources.find((s) => s.id === id)?.name ?? '' }) : t('Closed the tab.'));
}

export function setView(sourceId: number, view: ViewKind): void {
  appStore.set((s) => ({ viewChoice: { ...s.viewChoice, [sourceId]: view } }));
}

/** Shows page `page` of a PDF in the pages view (counting from 1, kept inside the PDF's pages). Whatever area was being drawn is let go. */
export function setPdfPage(sourceId: number, page: number): void {
  const count = appStore.get().pdfInfo[sourceId]?.page_count ?? appStore.get().sources.find((s) => s.id === sourceId)?.pdf?.page_count ?? 1;
  const next = Math.min(Math.max(1, Math.round(page)), Math.max(1, count));
  if ((appStore.get().pdfPage[sourceId] ?? 1) === next) return;
  appStore.set((s) => ({ pdfPage: { ...s.pdfPage, [sourceId]: next }, imageDraft: null }));
}

/** Learns the size of every page of a PDF the list of documents says is kept (once), so an area of one of its pages can be drawn in a list. */
const pdfAsked = new Set<string>();
export async function loadPdfInfo(sourceId: number): Promise<void> {
  const row = appStore.get().sources.find((s) => s.id === sourceId);
  if (!row?.pdf) return;
  const key = `${sourceId}:${row.pdf.size_bytes}:${row.pdf.file_name}`;
  if (pdfAsked.has(key)) return;
  pdfAsked.add(key);
  try {
    const pdf = await call('pdf.get', { source_id: sourceId });
    if (pdf) appStore.set((s) => ({ pdfInfo: { ...s.pdfInfo, [sourceId]: pdf } }));
  } catch {
    pdfAsked.delete(key);
  }
}

/** Keeps a PDF with a document that was imported without it (a PDF imported before the file was kept), then reloads the document. */
export async function attachPdf(sourceId: number, path: string): Promise<boolean> {
  try {
    const row = await call('pdf.attach', { source_id: sourceId, path });
    await refreshSources();
    appStore.set((s) => ({ pdfInfo: { ...s.pdfInfo, [sourceId]: row }, sourceData: s.sourceData?.id === sourceId ? { ...s.sourceData, pdf: row } : s.sourceData }));
    announce(t('Kept the PDF with {name}. Its {count} pages can be shown now.', { name: appStore.get().sources.find((s) => s.id === sourceId)?.name ?? '', count: row.page_count }));
    return true;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return false;
  }
}

/** Reloads the codings of the open source from the engine (after a bulk change). */
export async function reloadCodings(): Promise<void> {
  const d = appStore.get().sourceData;
  if (!d) return;
  const codings = await codingsOf(d.id, d.image !== null || d.pdf !== null || !!appStore.get().sources.find((s) => s.id === d.id)?.recording);
  const now = appStore.get().sourceData;
  if (now && now.id === d.id) {
    appStore.set({ sourceData: { ...now, codings } });
    recomputeCurrentCoverage();
  }
}

/** The codings of a document: its stretches of coded text, and for a picture or a PDF its coded areas, which are made to look like codings (see {@link regionToCoding}). */
async function codingsOf(sourceId: number, isPicture: boolean): Promise<CodingRow[]> {
  const text = await call('coding.list', { source_id: sourceId });
  if (!isPicture) return text;
  const areas = await call('region.list', { source_id: sourceId });
  return [...text, ...areas.map(regionToCoding)];
}

async function afterCodingChange(): Promise<void> {
  await Promise.all([refreshCodes(), refreshSources()]);
  recomputeCurrentCoverage();
}

// ---------------------------------------------------------------- applying codes

function readOnlyMessage(): string {
  return t('This project was made by a newer version of PrismQDA, so it can be read but not changed.');
}

function patchCodings(change: (codings: CodingRow[]) => CodingRow[]): void {
  const d = appStore.get().sourceData;
  if (!d) return;
  appStore.set({ sourceData: { ...d, codings: change(d.codings) } });
}

/**
 * Applies a code to whatever is selected. Returns true if anything was coded. When the code is in a mutually exclusive group
 * and the same words already have another code of it, nothing is replaced behind the person's back: a dialog asks, and
 * its Replace answer comes back here with what was left to code (`resume`).
 */
export async function applyCode(codeId: number, resume?: PendingReplace): Promise<boolean> {
  const st = appStore.get();
  const d = st.sourceData;
  const project = st.project;
  if (!d || !project) {
    announce(t('Open a source first.'), 'assertive');
    return false;
  }
  if (project.read_only) {
    announce(readOnlyMessage(), 'assertive');
    return false;
  }
  const code = st.codes.find((c) => c.id === codeId);
  if (!code) {
    announce(t('That code is not in the codebook any more.'), 'assertive');
    return false;
  }
  if (code.status !== 'active') {
    announce(t('{name} is {status}, so it cannot be applied. Restore it first.', { name: code.name, status: code.status }), 'assertive');
    return false;
  }
  // A picture, or a PDF shown as its pages, is coded by area; a PDF shown as words is coded like any document of words.
  if (d.image || (d.pdf && st.viewChoice[d.id] === 'pages')) return applyCodeToArea(codeId);
  // A recording that stands for its document is coded by stretches of time. A transcript with a recording is coded by its words, unless
  // a stretch has been marked, or a coded stretch is chosen and no words are selected.
  const recording = st.sources.find((s) => s.id === d.id)?.recording ?? null;
  if (recording) {
    const chosenStretch = st.selectedCodingId !== null && st.selectedCodingId < 0 && d.codings.some((c) => c.id === st.selectedCodingId && c.region?.shape === 'span');
    if (recording.stands_alone || st.clipDraft !== null || (chosenStretch && st.selection === null)) return applyCodeToClip(codeId);
  }
  const target = resume ? { ok: true as const, spans: resume.spans, skipped: resume.skipped, selected: true } : currentTarget();
  if (!target.ok) {
    announce(target.message, 'assertive');
    return false;
  }
  // A code the words already have from the same coder is not given twice (an area and a stretch are the same).
  const spans = notYetCoded(d.codings, target.spans, codeId, project.me ?? 0);
  if (spans.length === 0) {
    announce(target.spans.length > 1 ? t('All of these already have {name}.', { name: code.name }) : t('These words already have {name}.', { name: code.name }), 'assertive');
    return false;
  }
  const made: CodingRow[] = [];
  let lastQuote = '';
  try {
    for (const span of spans) {
      const r = await call('coding.apply', {
        source_id: d.id,
        start: span.start,
        end: span.end,
        code_id: codeId,
        ...(span.unit_id !== null ? { unit_id: span.unit_id } : {}),
        ...(resume ? { replace_exclusive: true } : {}),
      });
      lastQuote = r.quote;
      made.push({
        id: r.id,
        source_id: d.id,
        start: span.start,
        end: span.end,
        code_id: codeId,
        coder_id: project.me ?? 0,
        origin: 'human',
        layer: 'coder',
        rationale: '',
        unit_id: span.unit_id,
        // Known to the second; the list the engine gives next time has its own exact time.
        created_at: new Date().toISOString(),
      });
    }
  } catch (e) {
    const err = toApiError(e);
    if (made.length > 0) patchCodings((cs) => [...cs, ...made]);
    if (err.code === 'exclusive_group' && !resume) {
      appStore.set({
        dialog: { kind: 'replace-exclusive', pending: { codeId, message: err.message, spans: spans.slice(made.length), skipped: target.skipped } },
      });
      announce(t('{message} Choose Replace to take the other code away, or Cancel.', { message: err.message }), 'assertive');
    } else {
      announce(err.message, 'assertive');
    }
    await afterCodingChange();
    return made.length > 0;
  }
  // A replacement takes codings away as well as adding one, so the list is read again instead of patched.
  if (resume) await reloadCodings();
  else patchCodings((cs) => [...cs, ...made].sort((a, b) => a.start - b.start || a.end - b.end || a.id - b.id));
  appStore.set({ lastCodeId: codeId, selectedCodingId: made[made.length - 1]?.id ?? null });
  const allUnits = spans.every((s) => s.unit_id !== null);
  let message = made.length === 1 ? codedSentence(lastQuote, code.name) : t('Coded {count} as {name}', { count: allUnits ? tn(made.length, { one: '{count} unit', other: '{count} units' }) : tn(made.length, { one: '{count} part', other: '{count} parts' }), name: code.name });
  if (target.skipped > 0) message = t('{message}. Left out {count} that cannot be coded.', { message, count: tn(target.skipped, { one: '{count} unit', other: '{count} units' }) });
  announce(message);
  await afterCodingChange();
  return true;
}

/**
 * Applies a code to the area of the open picture that is being worked on: the area just drawn, or else the coded area that is selected
 * (so a second code can be given to an area that has one). A code the area already has is not given twice.
 */
async function applyCodeToArea(codeId: number): Promise<boolean> {
  const st = appStore.get();
  const d = st.sourceData;
  const code = st.codes.find((c) => c.id === codeId);
  if (!d || !code) return false;
  const chosenCoding = st.selectedCodingId !== null && st.selectedCodingId < 0 ? (d.codings.find((c) => c.id === st.selectedCodingId) ?? null) : null;
  const chosen = chosenCoding?.region ?? null;
  const shape = st.imageDraft ?? chosen;
  // On a PDF an area is on a page: the page being shown when it was drawn, or the page of the coded area that was chosen.
  const page = d.pdf ? (st.imageDraft ? (st.pdfPage[d.id] ?? 1) : (chosenCoding?.page ?? st.pdfPage[d.id] ?? 1)) : 1;
  if (!shape) {
    announce(d.pdf ? t('Draw a rectangle on the page first, or choose a coded area, and then apply the code.') : t('Draw an area of the picture first, or choose a coded area, and then apply the code.'), 'assertive');
    return false;
  }
  const me = st.project?.me ?? 0;
  if (d.codings.some((c) => c.region && (c.page ?? 1) === page && c.code_id === codeId && c.coder_id === me && c.layer === 'coder' && sameShape(c.region, shape))) {
    announce(t('This area already has {name}.', { name: code.name }), 'assertive');
    return false;
  }
  try {
    const r = await call('region.add', { ...shape, source_id: d.id, ...(d.pdf ? { page } : {}), code_id: codeId });
    const made = regionToCoding(r.region);
    patchCodings((cs) => [...cs, made]);
    // What was just made is the coding shown in Segment Details, and it is the area a second code goes to.
    appStore.set({ lastCodeId: codeId, selectedCodingId: made.id, imageDraft: null });
    announce(shape.shape === 'polygon' ? t('Coded the outline as {name}.', { name: code.name }) : shape.shape === 'ellipse' ? t('Coded the ellipse as {name}.', { name: code.name }) : t('Coded the rectangle as {name}.', { name: code.name }));
    await afterCodingChange();
    return true;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    await afterCodingChange();
    return false;
  }
}

/**
 * Applies a code to the stretch of the open recording that is being worked on: the one just marked, else the coded stretch that is chosen
 * (so a second code can be given to a stretch that has one). A code the stretch already has is not given twice.
 */
async function applyCodeToClip(codeId: number): Promise<boolean> {
  const st = appStore.get();
  const d = st.sourceData;
  const code = st.codes.find((c) => c.id === codeId);
  if (!d || !code) return false;
  const chosen = st.selectedCodingId !== null && st.selectedCodingId < 0 ? (d.codings.find((c) => c.id === st.selectedCodingId)?.region ?? null) : null;
  const draft = st.clipDraft;
  let shape: SpanShape | null = null;
  if (draft) {
    const problem = spanProblem(draft.start_ms, draft.end_ms);
    if (problem) {
      announce(problem, 'assertive');
      return false;
    }
    shape = orderedSpan(draft.start_ms as number, draft.end_ms as number);
  } else if (chosen && chosen.shape === 'span') {
    shape = chosen;
  }
  if (!shape) {
    announce(t('Mark where a stretch starts and where it ends, or choose a coded stretch, and then apply the code.'), 'assertive');
    return false;
  }
  const me = st.project?.me ?? 0;
  const here = shape;
  if (d.codings.some((c) => c.region?.shape === 'span' && c.code_id === codeId && c.coder_id === me && c.layer === 'coder' && c.region.start_ms === here.start_ms && c.region.end_ms === here.end_ms)) {
    announce(t('This stretch already has {name}.', { name: code.name }), 'assertive');
    return false;
  }
  try {
    const r = await call('region.add', { ...shape, source_id: d.id, code_id: codeId });
    const made = regionToCoding(r.region);
    patchCodings((cs) => [...cs, made]);
    // What was just made is the coding shown in Segment Details, and it is the stretch a second code goes to.
    appStore.set({ lastCodeId: codeId, selectedCodingId: made.id, clipDraft: null, selectionText: t('Selected: {span}', { span: spanWords(shape.start_ms, shape.end_ms) }) });
    announce(t('Coded {span} as {name}.', { span: spanShort(shape.start_ms, shape.end_ms), name: code.name }));
    await afterCodingChange();
    return true;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    await afterCodingChange();
    return false;
  }
}

/** Marks where the stretch being marked starts or ends (`ms` from the start of the recording). The other mark is kept. */
export function markClip(which: 'start' | 'end', ms: number): void {
  const whole = Math.max(0, Math.round(ms));
  appStore.set((s) => ({ clipDraft: { start_ms: s.clipDraft?.start_ms ?? null, end_ms: s.clipDraft?.end_ms ?? null, [which === 'start' ? 'start_ms' : 'end_ms']: whole } }));
  const d = appStore.get().clipDraft;
  const [from, to] = [d?.start_ms ?? null, d?.end_ms ?? null];
  // The line under the toolbar and the code box say what a code would be applied to.
  appStore.set({ selectionText: from !== null && to !== null && to > from ? t('Selected: {span} (not coded yet)', { span: spanWords(from, to) }) : '' });
  if (from !== null && to !== null && to > from) announce(t('Marked {span}. Choose a code to apply it.', { span: spanWords(from, to) }));
  else if (which === 'start') announce(t('The stretch starts at {time}. Now mark where it ends.', { time: clockText(whole, whole % 1000 >= 100) }));
  else announce(t('The stretch ends at {time}.', { time: clockText(whole, whole % 1000 >= 100) }));
}

/** Lets go of the marks of the stretch being marked. */
export function clearClipDraft(): void {
  if (appStore.get().clipDraft === null) return;
  appStore.set({ clipDraft: null, selectionText: '' });
  announce(t('Cleared the marks.'));
}

/** Asks the open recording to move to a time (without playing) or to play a stretch from its start to its end. */
export function commandClip(sourceId: number, kind: 'seek' | 'play', startMs: number, endMs: number): void {
  appStore.set({ clipCommand: { sourceId, kind, start_ms: Math.max(0, Math.round(startMs)), end_ms: Math.max(0, Math.round(endMs)), tick: Date.now() } });
}

/** Moves or resizes a coded stretch of the open recording (its times change; its code and number stay). */
export async function reshapeClip(codingId: number, shape: SpanShape): Promise<boolean> {
  try {
    const row = await call('region.reshape', { ...shape, id: -codingId });
    const updated = regionToCoding(row);
    patchCodings((cs) => cs.map((c) => (c.id === codingId ? { ...c, region: updated.region } : c)));
    bumpData();
    announce(t('The stretch is now {span}.', { span: spanWords(shape.start_ms, shape.end_ms) }));
    return true;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    await reloadCodings();
    return false;
  }
}

/** Moves or resizes a coded area of the open picture. The coding keeps its number. */
export async function reshapeArea(codingId: number, shape: AreaShape): Promise<boolean> {
  try {
    const row = await call('region.reshape', { ...shape, id: -codingId });
    const updated = regionToCoding(row);
    patchCodings((cs) => cs.map((c) => (c.id === codingId ? { ...c, region: updated.region } : c)));
    bumpData();
    return true;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    await reloadCodings();
    return false;
  }
}

/** Rewrites a picture's description (the words a screen reader speaks for it). */
export async function saveImageDescription(sourceId: number, text: string): Promise<boolean> {
  const clean = text.replace(/\r\n/g, '\n').trim();
  if (clean === '') {
    announce(t('A picture needs a description. Write a few words about what it shows.'), 'assertive');
    return false;
  }
  try {
    await call('source.edit', { id: sourceId, text: clean });
    const detail = await call('source.get', { id: sourceId });
    const d = appStore.get().sourceData;
    if (d && d.id === sourceId) appStore.set({ sourceData: { ...d, text: detail.text, index: new OffsetIndex(detail.text), textRevision: detail.text_revision } });
    await refreshSources();
    announce(t('Saved the description of the picture.'));
    return true;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return false;
  }
}

/** Rewrites the description of a recording that stands for its document (the words that tell what it holds). */
export async function saveRecordingDescription(sourceId: number, text: string): Promise<boolean> {
  const clean = text.replace(/\r\n/g, '\n').trim();
  if (clean === '') {
    announce(t('A recording needs a description. Write a few words about what it holds.'), 'assertive');
    return false;
  }
  try {
    await call('source.edit', { id: sourceId, text: clean });
    const detail = await call('source.get', { id: sourceId });
    const d = appStore.get().sourceData;
    if (d && d.id === sourceId) appStore.set({ sourceData: { ...d, text: detail.text, index: new OffsetIndex(detail.text), textRevision: detail.text_revision } });
    await refreshSources();
    announce(t('Saved the description of the recording.'));
    return true;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return false;
  }
}

export async function applyFavorite(slot: number): Promise<boolean> {
  const id = appStore.get().favorites[slot - 1] ?? null;
  if (id === null) {
    announce(t('No favorite code is pinned to key {slot}. Pin one in the codebook.', { slot }), 'assertive');
    return false;
  }
  return applyCode(id);
}

export async function repeatLastCode(): Promise<boolean> {
  const id = appStore.get().lastCodeId;
  if (id === null) {
    announce(t('There is no earlier code to repeat. Apply a code first.'), 'assertive');
    return false;
  }
  return applyCode(id);
}

const IN_VIVO_MAX = 60;

/** Makes a name for a code out of the selected words. */
export function inVivoName(words: string): string {
  const flat = words.replace(/\s+/g, ' ').trim().replace(/^[\s"'(\[]+|[\s"')\].,;:!?]+$/g, '');
  const chars = Array.from(flat);
  if (chars.length <= IN_VIVO_MAX) return flat;
  const cut = chars.slice(0, IN_VIVO_MAX).join('');
  const lastSpace = cut.lastIndexOf(' ');
  return (lastSpace > IN_VIVO_MAX / 2 ? cut.slice(0, lastSpace) : cut).trim();
}

export async function inVivo(): Promise<boolean> {
  const st = appStore.get();
  const d = st.sourceData;
  if (!d || !st.project) return false;
  if (st.project.read_only) {
    announce(readOnlyMessage(), 'assertive');
    return false;
  }
  const target = currentTarget();
  if (!target.ok) {
    announce(target.message, 'assertive');
    return false;
  }
  if (target.spans.length !== 1 || !target.selected) {
    announce(t('For in-vivo coding, select the words inside one place in the text that should name the new code.'), 'assertive');
    return false;
  }
  const span = target.spans[0];
  const name = inVivoName(sliceScalar(d.text, d.index, span.start, span.end));
  if (!name) {
    announce(t('Those words cannot be used as a code name. Select some words and try again.'), 'assertive');
    return false;
  }
  let code = st.codes.find((c) => c.status === 'active' && c.parent_id === null && c.name.toLowerCase() === name.toLowerCase());
  try {
    if (!code) {
      code = await call('code.add', { name, color: nextColor(st.codes) });
      await refreshCodes();
    }
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return false;
  }
  return applyCode(code.id);
}

export async function removeCoding(id: number): Promise<void> {
  await attempt(async () => {
    const d = appStore.get().sourceData;
    const c = d?.codings.find((x) => x.id === id);
    const quote = d && c ? sliceScalar(d.text, d.index, c.start, c.end) : '';
    await call('coding.remove', { id });
    patchCodings((cs) => cs.filter((x) => x.id !== id));
    appStore.set((s) => ({ selectedCodingId: s.selectedCodingId === id ? null : s.selectedCodingId }));
    const name = appStore.get().codes.find((k) => k.id === c?.code_id)?.name;
    announce(quote ? (name ? t("Removed the coding '{quote}' ({name}).", { quote: quote.replace(/\s+/g, ' ').slice(0, 80), name }) : t("Removed the coding '{quote}'.", { quote: quote.replace(/\s+/g, ' ').slice(0, 80) })) : t('Removed the coding.'));
    await afterCodingChange();
  });
}

export async function changeCodingCode(id: number, codeId: number): Promise<void> {
  // Two codings of the same words with the same code by the same coder would be one coding counted twice.
  const st = appStore.get();
  const here = st.sourceData?.codings.find((c) => c.id === id);
  if (here && here.code_id !== codeId && sameWordsWith(st.sourceData?.codings ?? [], here, codeId)) {
    const has = st.codes.find((k) => k.id === codeId)?.name ?? t('the new code');
    announce(here.region ? t('This area already has {name}.', { name: has }) : t('These words already have {name}.', { name: has }), 'assertive');
    return;
  }
  await attempt(async () => {
    await call('coding.change_code', { id, code_id: codeId });
    patchCodings((cs) => cs.map((c) => (c.id === id ? { ...c, code_id: codeId } : c)));
    const name = appStore.get().codes.find((k) => k.id === codeId)?.name ?? t('the new code');
    announce(t('Changed the coding to {name}.', { name }));
    appStore.set({ lastCodeId: codeId });
    await afterCodingChange();
  });
}

/** Sets the rating (1 to 5) of a coding, or clears it with null. Not rated is a state of its own, not zero. */
export async function setCodingRating(id: number, rating: number | null): Promise<void> {
  await attempt(async () => {
    await call('coding.rating', { id, rating });
    patchCodings((cs) => cs.map((c) => (c.id === id ? { ...c, rating } : c)));
    announce(rating === null ? t('The rating is cleared.') : t('Rated {rating} out of 5.', { rating }));
  });
}

export async function saveRationale(id: number, rationale: string): Promise<void> {
  await attempt(async () => {
    await call('coding.rationale', { id, rationale });
    patchCodings((cs) => cs.map((c) => (c.id === id ? { ...c, rationale } : c)));
    announce(rationale.trim() ? t('Saved the note on this coding.') : t('Cleared the note on this coding.'));
  });
}

/** Marks a passage (a coding made with another code) as a counterexample of a code, or takes the mark away. */
export async function setCounterexample(codeId: number, codingId: number, on: boolean): Promise<void> {
  await call('coding.counterexample', { code_id: codeId, id: codingId, on });
  const name = appStore.get().codes.find((k) => k.id === codeId)?.name ?? t('the code');
  announce(on ? t('Marked the passage as a counterexample of {name}.', { name }) : t('Took the counterexample away from {name}.', { name }));
  bumpData();
}

export async function setAnchor(id: number, anchor: boolean): Promise<void> {
  await attempt(async () => {
    await call('coding.anchor', { id, anchor });
    appStore.set((s) => {
      const next = new Set(s.anchors);
      if (anchor) next.add(id);
      else next.delete(id);
      return { anchors: next };
    });
    announce(anchor ? t('Marked this coding as an example for its code.') : t('Removed this coding from the examples.'));
  });
}

// ---------------------------------------------------------------- moving through units

export function selectCoding(id: number | null, opts?: { openDetails?: boolean }): void {
  appStore.set((s) => ({ selectedCodingId: id, detailTab: id !== null && opts?.openDetails !== false ? 'coding' : s.detailTab }));
}

export function selectCode(id: number | null, opts?: { openDetails?: boolean }): void {
  appStore.set((s) => ({ selectedCodeId: id, detailTab: id !== null && opts?.openDetails !== false ? 'code' : s.detailTab }));
}

/** The index of the next unit without a coding, counted from the unit at `fromOffset`. */
export function findUncodedUnit(fromOffset: number, direction: 1 | -1): number {
  const d = appStore.get().sourceData;
  if (!d || d.units.length === 0) return -1;
  // A unit counts as coded when any coding touches it, even if it was made without the unit.
  const touched = new Set<number>();
  for (const c of d.codings) {
    if (c.unit_id !== null) touched.add(c.unit_id);
    else for (const i of unitIndicesTouching(d.units, c.start, c.end)) touched.add(d.units[i].id);
  }
  let at = unitIndexForCaret(d.units, fromOffset);
  if (at < 0) {
    // The caret is between units: start from the gap, looking the way the person asked.
    const next = d.units.findIndex((u) => u.start > fromOffset);
    at = next < 0 ? d.units.length : direction === 1 ? next - 1 : next;
  }
  return nextUncodedUnit(d.units, touched, at, direction);
}

// ---------------------------------------------------------------- codebook

export function toggleFavorite(codeId: number): void {
  const st = appStore.get();
  const code = st.codes.find((c) => c.id === codeId);
  if (!code) return;
  const existing = st.favorites.indexOf(codeId);
  let next: Favorites | null;
  if (existing >= 0) {
    next = unpin(st.favorites, codeId);
    announce(t('{name} is no longer a favorite.', { name: code.name }));
  } else {
    next = pin(st.favorites, codeId);
    if (!next) {
      announce(t('All {MAX_FAVORITES} favorite slots are taken. Unpin one first.', { MAX_FAVORITES }), 'assertive');
      return;
    }
    announce(t('{name} is now favorite number {number}.', { name: code.name, number: next.indexOf(codeId) + 1 }));
  }
  appStore.set({ favorites: next });
  if (st.project) writeJson(favoritesKey(st.project.project_uid), next);
}

export async function addCode(args: {
  name: string;
  parentId: number | null;
  color?: string;
  definition?: string;
  inclusion?: string;
  exclusion?: string;
  theme?: boolean;
}): Promise<CodeRow> {
  const codes = appStore.get().codes;
  const code = await call('code.add', {
    name: args.name.trim(),
    ...(args.parentId !== null ? { parent_id: args.parentId } : {}),
    color: args.color ?? nextColor(codes),
    ...(args.definition ? { definition: args.definition } : {}),
    ...(args.inclusion ? { inclusion_criteria: args.inclusion } : {}),
    ...(args.exclusion ? { exclusion_criteria: args.exclusion } : {}),
    ...(args.theme ? { theme: true } : {}),
  });
  await refreshCodes();
  appStore.set({ selectedCodeId: code.id, detailTab: 'code' });
  announce(t('Added the code {name}.', { name: code.name }));
  return code;
}

export async function editCode(id: number, edit: CodeEditArgs): Promise<CodeRow> {
  const code = await call('code.update', { id, ...edit });
  await refreshCodes();
  announce(t('Saved the code {name}.', { name: code.name }));
  return code;
}

/** Changes a code's color at once: a color is a choice, not a draft, so it does not wait for "Save changes". */
export async function setCodeColor(id: number, color: string): Promise<void> {
  const before = appStore.get().codes.find((c) => c.id === id);
  if (!before || before.color.toLowerCase() === color.toLowerCase()) return;
  await attempt(async () => {
    await call('code.update', { id, color });
    await refreshCodes();
    announce(t('{name} is now {color}.', { name: before.name, color: colorName(color) }));
  });
}

export async function moveCode(id: number, plan: MovePlan): Promise<void> {
  const name = appStore.get().codes.find((c) => c.id === id)?.name ?? t('The code');
  await call('code.move', { id, parent_id: plan.parent_id, ...(plan.position !== undefined ? { position: plan.position } : {}) });
  await refreshCodes();
  announce(t('Moved {name} {where}.', { name, where: plan.where }));
}

export async function retireCode(id: number): Promise<void> {
  const name = appStore.get().codes.find((c) => c.id === id)?.name ?? t('The code');
  await call('code.retire', { id });
  await refreshCodes();
  announce(t('Retired {name}. Its codings are kept.', { name }));
}

export async function restoreCode(id: number): Promise<void> {
  const name = appStore.get().codes.find((c) => c.id === id)?.name ?? t('The code');
  await call('code.restore', { id });
  await refreshCodes();
  announce(t('Restored {name}.', { name }));
}

export async function mergeCodes(from: number, into: number): Promise<void> {
  const codes = appStore.get().codes;
  const a = codes.find((c) => c.id === from)?.name ?? t('The code');
  const b = codes.find((c) => c.id === into)?.name ?? t('the other code');
  const r = await call('code.merge', { from, into });
  await Promise.all([refreshCodes(), reloadCodings(), refreshSources()]);
  appStore.set({ selectedCodeId: into });
  announce(t('Merged {a} into {b}. {count} moved.', { a, b, count: tn(r.codings_moved, { one: '{count} coding', other: '{count} codings' }) }));
}

export async function splitCode(from: number, name: string, codingIds: number[], definition: string): Promise<CodeRow> {
  const code = await call('code.split', { from, name: name.trim(), coding_ids: codingIds, ...(definition.trim() ? { definition: definition.trim() } : {}) });
  await Promise.all([refreshCodes(), reloadCodings(), refreshSources()]);
  appStore.set({ selectedCodeId: code.id });
  announce(t('Split {count} off into the new code {name}.', { count: tn(codingIds.length, { one: '{count} coding', other: '{count} codings' }), name: code.name }));
  return code;
}

export async function saveSnapshot(note: string): Promise<number> {
  const r = await call('codebook.snapshot', { note: note.trim() });
  announce(t('Saved codebook version {version}.', { version: r.version }));
  return r.version;
}

export { ApiError };

// ---------------------------------------------------------------- the frame: where you are, undo, backup

export function goTo(workspace: import('./app').Workspace): void {
  appStore.set({ workspace });
  const names = { home: t('Home'), data: t('Data'), import: t('Import Data'), code: t('Code'), codebook: t('Codebook Editor'), themes: t('Themes'), explore: t('Explore'), autocode: t('Auto Code'), ai: t('AI Assistant'), team: t('Team'), text: t('Text Analytics'), stats: t('Statistics'), variables: t('Data editor'), mixed: t('Mixed Methods'), synthesize: t('Synthesize'), report: t('Report'), privacy: t('Pseudonymize and redact'), settings: t('Settings') } as const;
  announce(`${names[workspace]}.`);
}

/** Opens the Auto Code page on its set-up form or its review queue, optionally on one batch. */
export function goAutoCode(tab: AutoCodeTab, batch?: string | null): void {
  appStore.set((s) => ({ workspace: 'autocode', autoCodeTab: tab, autoCodeBatch: batch === undefined ? s.autoCodeBatch : batch }));
  announce(tab === 'queue' ? t('Auto Code: Review queue.') : t('Auto Code: Set up.'));
}

/** Changes the Auto Code form; it is kept while moving between pages. */
export function setAutoSetup(patch: Partial<AutoSetup>): void {
  appStore.set((s) => ({ autoCodeSetup: { ...s.autoCodeSetup, ...patch } }));
}

/** Turns split view on (showing another document, read-only, beside the one being read) or off. */
export function toggleSplit(): void {
  const st = appStore.get();
  if (st.splitSourceId !== null) {
    appStore.set({ splitSourceId: null });
    announce(t('Split view is off.'));
    return;
  }
  if (st.currentSourceId === null) {
    announce(t('Open a document first, then turn split view on.'), 'assertive');
    return;
  }
  const target = defaultSplitTarget(st.sources, st.currentSourceId);
  if (target === null) {
    announce(t('Split view needs a second document. This project has only one.'), 'assertive');
    return;
  }
  appStore.set({ splitSourceId: target });
  announce(t('Split view is on: {name} is shown beside this document.', { name: st.sources.find((s) => s.id === target)?.name ?? t('another document') }));
}

/** Chooses the document shown beside the one being read. */
export function setSplit(id: number | null): void {
  appStore.set({ splitSourceId: id });
  if (id !== null) announce(t('{name} is shown beside this document.', { name: appStore.get().sources.find((s) => s.id === id)?.name ?? t('The document') }));
}

/** Reads the document that was beside, and puts the one that was being read beside it. */
export async function swapSplit(): Promise<void> {
  const st = appStore.get();
  const was = st.currentSourceId;
  const beside = st.splitSourceId;
  if (was === null || beside === null) return;
  appStore.set({ splitSourceId: was });
  await selectSource(beside);
  announce(t('Swapped: {name} is now being read.', { name: st.sources.find((s) => s.id === beside)?.name ?? t('the other document') }));
}

/** Opens the AI page on its packet form or its log of runs. */
export function goAi(tab: AiTab): void {
  appStore.set((s) => ({ workspace: 'ai', ai: { ...s.ai, tab } }));
  announce(tab === 'log' ? t('AI Assistant: AI use log.') : t('AI Assistant: Prompt packet.'));
}

/** Changes the AI page's state (the form, the packet, the answer being pasted); it is kept while moving between pages. */
export function setAi(patch: Partial<AiState>): void {
  appStore.set((s) => ({ ai: { ...s.ai, ...patch } }));
}

/** Changes the packet form. */
export function setAiSetup(patch: Partial<AiSetup>): void {
  appStore.set((s) => ({ ai: { ...s.ai, setup: { ...s.ai.setup, ...patch } } }));
}

/** Builds a prompt packet (text only: nothing is sent anywhere). Says why when it cannot. */
export async function buildAiPacket(args: PacketArgs): Promise<{ packet: AiPacket } | { problem: string }> {
  try {
    const packet = await call('ai.packet', args);
    bumpData();
    return { packet };
  } catch (e) {
    return { problem: toApiError(e).message };
  }
}

/** Checks the answer pasted back for a packet and sends what passes to the review queue. Returns the message when it cannot. */
export async function sendAiAnswer(runId: number, model: string, answer: string): Promise<{ outcome: AiAnswerOutcome } | { problem: string }> {
  try {
    const outcome = await call('ai.answer', { run_id: runId, model, answer });
    bumpData();
    return { outcome };
  } catch (e) {
    return { problem: toApiError(e).message };
  }
}

/** Allows or forbids prompt packets in the open project. */
export async function setAiMode(mode: 'packet' | 'off'): Promise<void> {
  try {
    const info = await call('ai.set_mode', { mode });
    appStore.set({ project: info });
    bumpData();
    announce(mode === 'off' ? t('AI is off in this project. No packet can be built.') : t('AI prompt packets are allowed in this project.'));
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
  }
}

/** Marks a document as one that must never be put in a prompt packet (or lifts the mark). */
export async function setSourceDoNotShare(id: number, doNotShare: boolean): Promise<void> {
  try {
    await call('source.set_do_not_share', { id, do_not_share: doNotShare });
    await refreshSources();
    bumpData();
    const name = appStore.get().sources.find((s) => s.id === id)?.name ?? t('The document');
    announce(doNotShare ? t('{name} will never be put in a prompt packet.', { name }) : t('{name} may be put in a prompt packet again.', { name }));
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
  }
}

/** Chooses which batch the review queue shows (null for every batch with something waiting). */
export function setAutoBatch(batch: string | null): void {
  appStore.set({ autoCodeBatch: batch });
}

/** Sends what a run found to the review queue (making any new codes first). Says why when it cannot. */
export async function sendAutoCode(args: AutocodeArgs): Promise<AutocodeProposed | undefined> {
  try {
    const r = await call('autocode.propose', args);
    if (r.codes_created.length > 0) await refreshEverything();
    else bumpData();
    return r;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return undefined;
  }
}

/** Accepts or rejects suggestions (a list of them, or every one still waiting in a batch). Accepted ones become codings. */
export async function decideSuggestions(which: { ids?: number[]; batch?: string }, accept: boolean): Promise<DecisionOutcome | undefined> {
  try {
    const r = await call('proposal.decide', { ...which, accept });
    if (r.accepted > 0) await refreshEverything();
    else bumpData();
    return r;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return undefined;
  }
}

/** Corrects the words or the code of a suggestion that still waits. */
export async function adjustSuggestion(id: number, start: number, end: number, codeId: number): Promise<ProposalRow | undefined> {
  try {
    const r = await call('proposal.adjust', { id, start, end, code_id: codeId });
    bumpData();
    return r;
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
    return undefined;
  }
}

/** Opens the Explore page on one of its tools. */
export function goExplore(tool: ExploreTool): void {
  appStore.set({ workspace: 'explore', exploreTool: tool });
  const names: Record<ExploreTool, string> = { query: t('Query Builder'), matrix: t('Code Matrix'), crosstab: t('Crosstabs'), relations: t('Code Relations'), summary: t('Code Summary'), visualize: t('Visualizations'), map: t('Concept Map'), portrait: t('Document Portrait') };
  announce(t('Explore: {value}.', { value: names[tool] }));
}

/** Changes how the code matrix is set up; the table and the charts follow it. */
export function setExploreMatrix(patch: Partial<MatrixSetup>): void {
  appStore.set((s) => ({ exploreMatrix: { ...s.exploreMatrix, ...patch } }));
}

export function setExploreQuery(patch: Partial<QueryState>): void {
  appStore.set((s) => ({ exploreQuery: { ...s.exploreQuery, ...patch } }));
}

/** Chooses the concept map to show and the node chosen on it. */
export function setExploreMap(patch: Partial<MapState>): void {
  appStore.set((s) => ({ exploreMap: { ...s.exploreMap, ...patch } }));
}

/** Changes how the crosstab is set up. */
export function setExploreCrosstab(patch: Partial<CrosstabSetup>): void {
  appStore.set((s) => ({ exploreCrosstab: { ...s.exploreCrosstab, ...patch } }));
}

/** Changes one or more settings: remembers them on this computer, puts them on the page, and says what changed. */
export function updateSettings(patch: Partial<Settings>): void {
  const before = appStore.get().settings;
  const next = normalizeSettings({ ...before, ...patch });
  appStore.set({ settings: next });
  writeJson(SETTINGS_KEY, next);
  applySettings(next, document.documentElement);
  if (next.keepBackups !== before.keepBackups) {
    void tellEngineBackups(next.keepBackups);
    announce(t('Each project keeps its {keepBackups} newest backups.', { keepBackups: next.keepBackups }));
  }
  if (next.historyName !== before.historyName) {
    void tellEngineName(next.historyName);
    announce(next.historyName === '' ? t('Your changes are recorded under the name this program made for you.') : t('Your changes are now recorded under the name {name}.', { name: next.historyName }));
  }
  if (next.confidenceLevel !== before.confidenceLevel || next.bootstrapResamples !== before.bootstrapResamples || next.pairMethod !== before.pairMethod || next.fixedSeed !== before.fixedSeed) {
    statsStore.set(analysisDefaults(next));
    announce(t('The Statistics page now begins with these choices.'));
  }
  if (next.subcodes !== before.subcodes) {
    appStore.set((s) => ({ exploreMatrix: { ...s.exploreMatrix, descendants: next.subcodes }, exploreQuery: { ...s.exploreQuery, draft: { ...s.exploreQuery.draft, descendants: next.subcodes } } }));
    announce(next.subcodes ? t('A code also stands for its sub-codes, to begin with.') : t('A code stands only for itself, to begin with.'));
  }
  if (next.codingMode !== before.codingMode) announce(next.codingMode === 'unit' ? t('New projects code whole units unless their form says otherwise.') : t('New projects code any selected words unless their form says otherwise.'));
  if (next.theme !== before.theme) announce(t('Colors: {theme}.', { theme: themeLabel(next.theme) }));
  else if (next.textSize !== before.textSize) announce(t('Text size: {textSize} percent.', { textSize: next.textSize }));
  else if (next.announceCaret !== before.announceCaret) announce(next.announceCaret ? t('The caret will say which codes it is in.') : t('The caret will not say which codes it is in.'));
  else if (next.lineNumbers !== before.lineNumbers) announce(next.lineNumbers ? t('Lines of text are numbered.') : t('Lines of text are not numbered.'));
  else if (next.floatingBar !== before.floatingBar) announce(next.floatingBar ? t('The coding bar shows above words selected with the mouse.') : t('The coding bar is off. The keys and the Codes list still code.'));
  else if (next.showUnits !== before.showUnits) announce(next.showUnits ? t('The start of each unit is marked.') : t('The start of each unit is not marked.'));
  else if (next.textWidth !== before.textWidth) announce(next.textWidth === 'fill' ? t('The text fills the width of the window.') : t('The text keeps to a comfortable line length.'));
}

/** Changes the language of the words in the window: remembers the choice, loads the language, and redraws every screen in it. */
export async function setLanguage(choice: string): Promise<void> {
  updateSettings({ language: choice });
  const code = chooseLocale(appStore.get().settings.language, typeof navigator === 'undefined' ? [] : navigator.languages ?? []);
  await activateLocale(code, document.documentElement);
  announce(t('The language of the window is now {language}.', { language: listLanguages().find((l) => l.id === code)?.label ?? code }));
}

/** Takes a translation off this computer. If the window was showing it, the window goes back to following the computer's language. */
export async function removeLanguage(id: string): Promise<void> {
  const label = listLanguages().find((l) => l.id === id)?.label ?? id;
  const wasUsed = appStore.get().settings.language === id;
  removeTranslation(id);
  if (wasUsed) await setLanguage('auto');
  else bumpLanguages();
  announce(t('Removed the translation {label}.', { label }));
}

/**
 * Tells the engine the name the person goes by in the history of a project (empty: the one the program made for this computer, which is
 * never the Windows account name). It applies to the open project and every one opened after it.
 */
export async function tellEngineName(name: string): Promise<void> {
  try {
    await call('history.name', { name });
  } catch {
    // An engine that does not know the command keeps its own name; the setting is still remembered here.
  }
}

/** Tells the engine how many dated backups to keep (for the open project and every one opened after it). */
export async function tellEngineBackups(keep: number): Promise<void> {
  try {
    await call('backup.keep', { keep });
  } catch {
    // An engine that does not know the command keeps its own number; the setting is still remembered here.
  }
}

/**
 * Hands the engine the backup folder saved on this computer, without refusing it: a folder that has gone away (a drive
 * that is not plugged in) must not stop the program starting. A backup then falls back to beside the project and says why.
 */
export async function tellEngineBackupFolder(folder: string): Promise<void> {
  try {
    await call('backup.dir', { dir: folder === '' ? null : folder, check: false });
  } catch {
    // The engine keeps beside-the-project; the setting is still remembered here.
  }
}

/**
 * Chooses the folder backups go to (an empty text is back to beside each project). The engine checks that the folder
 * exists and can be written in first; when it cannot, this throws with its reason and the earlier choice is untouched.
 */
export async function setBackupFolder(folder: string): Promise<void> {
  await call('backup.dir', { dir: folder === '' ? null : folder });
  updateSettings({ backupFolder: folder });
  announce(folder === '' ? t('Backups are made beside each project.') : t('Backups of every project now go in {folder}.', { folder }));
}

/** Puts every setting back to how it was when the program was installed. */
export function resetSettings(): void {
  appStore.set({ settings: DEFAULT_SETTINGS });
  removeKey(SETTINGS_KEY);
  applySettings(DEFAULT_SETTINGS, document.documentElement);
  void tellEngineBackups(DEFAULT_SETTINGS.keepBackups);
  void tellEngineBackupFolder(DEFAULT_SETTINGS.backupFolder);
  void tellEngineName(DEFAULT_SETTINGS.historyName);
  announce(t('Settings are back to the standard ones.'));
}

/** Forgets the sizes the person gave the panels in Code; the next time Code opens it uses the standard layout. */
export function resetPanelSizes(): void {
  removeKey('prismqda.layout.v2');
  announce(t('The panels in Code are back to their standard sizes.'));
}

/** Tells lists that are not tied to one store field (activity, memos, the dashboard) to load again. */
export function bumpData(): void {
  appStore.set((s) => ({ dataVersion: s.dataVersion + 1 }));
}

/** After names were replaced for good: every text the screens had kept is old, so they are all fetched again. */
export async function afterNamesReplaced(): Promise<void> {
  textCache.clear();
  const open = appStore.get().currentSourceId;
  appStore.set({ sourceData: null });
  await refreshEverything();
  if (open !== null) await selectSource(open);
}

/** Reloads what an undo, or any change made behind the screens' back, can have touched. */
export async function refreshEverything(): Promise<void> {
  coverageRun++;
  appStore.set({ coverage: {} });
  await Promise.all([refreshSources(), refreshCodes(), refreshCoders(), refreshProjectInfo()]);
  void refreshCases();
  if (appStore.get().currentSourceId !== null) await reloadCodings();
  bumpData();
  void measureCoverage();
}

/** Undoes the newest change the person made, and says what it was. */
export async function undoLast(): Promise<void> {
  if (appStore.get().project?.read_only) {
    announce(t('This project is read only, so nothing can be undone.'), 'assertive');
    return;
  }
  try {
    const r = await call('undo');
    if (r.nothing_to_undo) {
      announce(t('Nothing to undo.'));
      return;
    }
    await refreshEverything();
    // A coding that the undo took away is not the selected one any more (Retrieved Segments said it was outside the rows shown).
    appStore.set((s) => (s.selectedCodingId !== null && s.sourceData && !s.sourceData.codings.some((c) => c.id === s.selectedCodingId) ? { selectedCodingId: null } : {}));
    announce(t('Undid: {what}.', { what: r.what ? engineText(r.what) : (r.action ?? t('the last change')) }));
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
  }
}

/** Makes a dated copy of the whole project now, next to it. */
export async function saveBackup(): Promise<void> {
  try {
    const { path, note } = await call('project.backup');
    const name = path.split(/[\\/]/).pop() ?? path;
    announce(note ? t('Saved a backup copy: {name}. {note}', { name, note }) : t('Saved a backup copy: {name}.', { name }), note ? 'assertive' : 'polite');
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
  }
}

/**
 * Takes a file as the new version of a document: the document keeps its codings, memos, case, and attributes, its text becomes
 * the file's text, and each coding follows the words that survive. A backup copy is saved first. Says why when it cannot.
 */
export async function reviseDocument(sourceId: number, args: ImportOptionsArgs): Promise<{ result: ReviseResult } | { problem: string }> {
  try {
    const result = await call('import.revise', { ...args, source_id: sourceId });
    // The text changed under anything that showed it: the cached copy, and the document if it is open.
    textCache.delete(sourceId);
    if (appStore.get().currentSourceId === sourceId) appStore.set({ sourceData: null });
    await refreshEverything();
    if (appStore.get().currentSourceId === sourceId) await selectSource(sourceId);
    return { result };
  } catch (e) {
    return { problem: toApiError(e).message };
  }
}

/** Opens a source in Code and selects a passage of it, for a search result or a retrieved passage. */
export async function openAt(sourceId: number, start: number, end: number, codingId?: number): Promise<void> {
  appStore.set({ workspace: 'code' });
  await selectSource(sourceId);
  const source = appStore.get().sources.find((s) => s.id === sourceId);
  if (!source) return;
  if (source.image) {
    // A picture has no words to select: the coded area that was asked for is the one chosen.
    if (codingId !== undefined && codingId < 0) selectCoding(codingId);
    return;
  }
  if (source.recording && codingId !== undefined && codingId < 0) {
    // A coded stretch of a recording has no words to select: the stretch is chosen, and the recording moves to where it starts.
    selectCoding(codingId);
    const where = appStore.get().sourceData?.codings.find((c) => c.id === codingId)?.region;
    if (where?.shape === 'span') commandClip(sourceId, 'seek', where.start_ms, where.end_ms);
    return;
  }
  if (source.pdf && codingId !== undefined && codingId < 0) {
    // An area of a PDF page: show that page, with the area chosen.
    const page = appStore.get().sourceData?.codings.find((c) => c.id === codingId)?.page ?? 1;
    setView(sourceId, 'pages');
    setPdfPage(sourceId, page);
    selectCoding(codingId);
    return;
  }
  if (isBigSource(source)) {
    // A very large source opens in the grid, which cannot select by position; say so rather than do nothing.
    announce(t('Opened {name}. It is large, so it is shown as a grid and the passage is not selected.', { name: source.name }));
    return;
  }
  setView(sourceId, 'document');
  appStore.set({ pendingJump: { sourceId, start, end, ...(codingId !== undefined ? { codingId } : {}) } });
}

// ---------------------------------------------------------------- the lower pane: which codes' passages to show

/** Shows or stops showing the passages of one code. */
export function toggleActiveCode(id: number): void {
  const st = appStore.get();
  const code = st.codes.find((c) => c.id === id);
  if (!code) return;
  const next = toggleId(st.activeCodeIds, id);
  appStore.set({ activeCodeIds: next });
  announce(next.includes(id) ? t('Showing the passages coded {name}.', { name: code.name }) : t('No longer showing the passages coded {name}.', { name: code.name }));
}

/** Replaces the whole list of chosen codes. */
export function setActiveCodes(ids: number[]): void {
  appStore.set({ activeCodeIds: ids });
}

export function setRetrievalMode(mode: RetrievalMode): void {
  appStore.set({ retrievalMode: mode });
  announce(mode === 'open' ? t('Looking in the open source only.') : t('Looking in every source.'));
}

export function setRetrievalView(view: 'list' | 'gallery'): void {
  appStore.set({ retrievalView: view });
  announce(view === 'gallery' ? t('Showing the areas of pictures as a gallery.') : t('Showing the segments as a list.'));
}

export function setRetrievalCase(id: number | null): void {
  appStore.set({ retrievalCase: id });
  const name = id === null ? null : appStore.get().cases.find((c) => c.id === id)?.name;
  announce(name ? t('Only the case {name}.', { name }) : t('Every case.'));
}

export function setRetrievalQuery(query: string): void {
  appStore.set({ retrievalQuery: query });
}

/** Clears every filter on Retrieved Segments: the chosen codes, the document, the case, and the search. */
export function resetScope(): void {
  appStore.set({ activeCodeIds: [], retrievalMode: 'all', retrievalCase: null, retrievalQuery: '' });
  announce(t('Filters cleared. Retrieved Segments shows every code in every document.'));
}

/** Takes the person to the passages: into Code if they are elsewhere, then the focus onto the pane. */
export function showPassages(): void {
  if (appStore.get().workspace !== 'code') appStore.set({ workspace: 'code' });
  window.setTimeout(() => document.getElementById('pane-retrieved')?.focus(), 60);
}
