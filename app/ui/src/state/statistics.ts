// What the Statistics page keeps while a project is open: the choices in the test chooser, and the output notebook. The notebook is
// kept in the project file (each page with its request, its result, and the author's note), and this store holds the copy the page
// shows; it lives here and not in the page so that going to another page and coming back does not reload it.

import type { Goal } from '../lib/statistics';
import { call } from '../bridge';
import type { Settings } from '../lib/settings';
import { appStore } from './app';
import type { StatAdjustment, StatCurveKind, StatNotebookPage, StatPostHoc, StatProcedure, StatRequest, StatRerun, StatResult, StatRunsCut, StatVariable } from '../types';
import { createStore, useStore } from './store';

/** An analysis page: what was asked, what came back, and the author's note on it. */
export interface NotebookEntry {
  kind: 'analysis';
  /** The page's number in the project. */
  id: number;
  request: StatRequest;
  result: StatResult;
  /** When it last ran, as the engine wrote it (UTC). */
  ranAt: string;
  note: string;
  section: string;
  /** How many earlier runs are kept for it. */
  earlierRuns: number;
}

/** A page of notes. */
export interface NotePage {
  kind: 'note';
  id: number;
  title: string;
  note: string;
  section: string;
}

export type NotebookItem = NotebookEntry | NotePage;

/** What running an analysis again found, kept to show beside the page: what it said before, or why it could not be run. */
export interface Comparison {
  before: StatResult | null;
  beforeAt: string | null;
  changed: boolean;
  error: string | null;
}

/** What running every analysis again did, in numbers. */
export interface RerunSummary {
  total: number;
  changed: number;
  failed: number;
}

/** The analysis pages of a notebook, in order (a page of notes is not one). */
export function analyses(items: NotebookItem[]): NotebookEntry[] {
  return items.filter((e): e is NotebookEntry => e.kind === 'analysis');
}

/** The section value that stands for the pages that are in no section. */
export const NO_SECTION = '\u0000none';

export function itemFrom(p: StatNotebookPage): NotebookItem {
  if (p.kind === 'note') return { kind: 'note', id: p.id, title: p.title, note: p.note, section: p.section };
  return { kind: 'analysis', id: p.id, request: p.request as StatRequest, result: p.result as StatResult, ranAt: p.ran_at ?? '', note: p.note, section: p.section, earlierRuns: p.earlier_runs };
}

export interface StatsState {
  /** The project the notebook belongs to; another project starts a new notebook. */
  projectPath: string | null;
  notebook: NotebookItem[];
  /** What running a page again found, by page, for the pages run again since the project was opened (or compared on request). */
  comparisons: Record<number, Comparison>;
  rerunSummary: RerunSummary | null;
  /** The section the notebook shows: '' for every page, NO_SECTION for the pages in none, or a section's name. */
  sectionShown: string;
  /** The page this session just added, which takes the focus once. */
  justAdded: number | null;
  /** The variables the page offered when it last loaded them, which the script of a page needs for the order of their values. */
  variables: StatVariable[];
  goal: Goal;
  /** The variable chosen for each role of the goal, by key. */
  picks: Record<string, string>;
  /** The groups ticked for a comparison, or null for all of them. */
  groups: string[] | null;
  method: StatProcedure | null;
  level: number;
  mu: string;
  equalVariances: boolean;
  adjustment: StatAdjustment;
  /** A test made for pairs of groups (Tukey, Games-Howell, Scheffe), or '' to use the adjusted t tests. */
  posthoc: StatPostHoc | '';
  /** For a regression: the predictors chosen, each with the step it is entered in. */
  predictors: { key: string; step: number }[];
  useSteps: boolean;
  /** The reference category of each categorical predictor, by variable key (the first category when none is chosen). */
  references: Record<string, string>;
  bootstrap: boolean;
  resamples: number;
  /** The seed as typed; a whole number, so a saved seed gives the same intervals. */
  seed: string;
  /** A rule (R's syntax) that picks the cases to use, as typed; empty uses every case. */
  filter: string;
  /** The variable of whole numbers that says how many times each case is counted ('' counts every case once). */
  weight: string;
  /** The variable whose groups the analysis is run on one by one, a page for each ('' runs it once on all the cases). */
  split: string;
  /** The variables of a list that follows the roles of a goal (the measures compared, the variables held constant), by key. */
  list: string[];
  /** The standard deviation of the curve of the Kolmogorov-Smirnov test, as typed (empty: the sample's own). */
  sd: string;
  /** The category counted as a success in the binomial test ('' for the first one), or modelled by a logistic regression ('' for 1, else the last), and the share compared with, as typed. */
  success: string;
  proportion: string;
  /** For the one-sample chi-square: equal shares, or the shares typed for each category (by value). */
  equalShares: boolean;
  shares: Record<string, string>;
  /** The control group of the Moses test ('' for the first) and the number trimmed from each end, as typed. */
  control: string;
  trim: string;
  /** Where the runs test cuts a number in two. */
  cut: StatRunsCut;
  /** The curves a curve fit fits. */
  curves: StatCurveKind[];
  /** Bootstrap the whole analysis (run the procedure again on each resample) instead of the one statistic of a test. */
  bootstrapWhole: boolean;
  /** Resample whole groups of cases: the key of the variable that says which group a case is in ('' for single cases). */
  cluster: string;
}

/** A seed to begin with, different each time the notebook starts, and shown so that the run can be repeated. */
export function newSeed(): number {
  return Math.floor(Math.random() * 2_000_000_000) + 1;
}

const initial: StatsState = {
  projectPath: null,
  notebook: [],
  comparisons: {},
  rerunSummary: null,
  sectionShown: '',
  justAdded: null,
  variables: [],
  goal: 'describe',
  picks: {},
  groups: null,
  method: null,
  level: 0.95,
  mu: '0',
  equalVariances: false,
  adjustment: 'holm',
  posthoc: '',
  predictors: [{ key: '', step: 1 }],
  useSteps: false,
  references: {},
  bootstrap: false,
  resamples: 2000,
  seed: '1',
  filter: '',
  weight: '',
  split: '',
  list: [''],
  sd: '',
  success: '',
  proportion: '0.5',
  equalShares: true,
  shares: {},
  control: '',
  trim: '',
  cut: 'median',
  curves: ['linear', 'logarithmic', 'inverse', 'quadratic', 'cubic', 'power', 'exponential', 's_curve'],
  bootstrapWhole: false,
  cluster: '',
};

export const statsStore = createStore<StatsState>(initial);

export function useStats<T>(selector: (s: StatsState) => T): T {
  return useStore(statsStore, selector);
}

/** What the person's analysis settings say the Statistics page begins with: the confidence level, the bootstrap, and how pairs of groups are compared. */
export function analysisDefaults(s: Settings): Partial<StatsState> {
  const test = s.pairMethod === 'tukey' || s.pairMethod === 'games_howell' || s.pairMethod === 'scheffe';
  return {
    level: s.confidenceLevel,
    resamples: s.bootstrapResamples,
    posthoc: test ? (s.pairMethod as StatPostHoc) : '',
    adjustment: test ? 'holm' : (s.pairMethod as StatAdjustment),
    seed: s.fixedSeed === null ? String(newSeed()) : String(s.fixedSeed),
  };
}

/** Starts again for another project (or none). */
export function resetStats(projectPath: string | null): void {
  statsStore.set({ ...initial, ...analysisDefaults(appStore.get().settings), projectPath });
}

/** Reads the notebook of the open project into the store. */
export async function loadNotebook(path: string | null): Promise<void> {
  if (path === null) return;
  const pages = await call('stats.notebook', {});
  if (statsStore.get().projectPath !== path) return;
  statsStore.set({ notebook: pages.map(itemFrom) });
}

/** Runs an analysis and adds it to the notebook, in the project, as a new page at the end. */
export async function runIntoNotebook(request: StatRequest, sectionName?: string): Promise<NotebookEntry> {
  const section = sectionName ?? statsStore.get().sectionShown;
  const page = await call('stats.notebook.run', { request, section: section === '' || section === NO_SECTION ? '' : section });
  const item = itemFrom(page) as NotebookEntry;
  statsStore.set((s) => ({ notebook: [...s.notebook, item], justAdded: item.id }));
  return item;
}

export async function addNotePage(): Promise<NotePage> {
  const section = statsStore.get().sectionShown;
  const page = await call('stats.notebook.note', { section: section === '' || section === NO_SECTION ? '' : section });
  const item = itemFrom(page) as NotePage;
  statsStore.set((s) => ({ notebook: [...s.notebook, item] }));
  return item;
}

export async function removeFromNotebook(id: number): Promise<void> {
  await call('stats.notebook.remove', { id });
  statsStore.set((s) => {
    const { [id]: _gone, ...comparisons } = s.comparisons;
    return { notebook: s.notebook.filter((e) => e.id !== id), comparisons };
  });
}

/** Changes a page's note, section, or (for a page of notes) title, in the project. */
export async function updatePage(id: number, patch: { note?: string; section?: string; title?: string }): Promise<void> {
  const page = await call('stats.notebook.update', { id, ...patch });
  statsStore.set((s) => ({ notebook: s.notebook.map((e) => (e.id === id ? itemFrom(page) : e)) }));
}

export async function movePage(id: number, up: boolean): Promise<void> {
  const pages = await call('stats.notebook.move', { id, up });
  statsStore.set((s) => {
    const by = new Map(s.notebook.map((e) => [e.id, e]));
    return { notebook: pages.map((p) => by.get(p.id) ?? itemFrom(p)) };
  });
}

function applyRerun(r: StatRerun): void {
  statsStore.set((s) => ({
    notebook: s.notebook.map((e) => (e.id === r.page.id ? itemFrom(r.page) : e)),
    comparisons: { ...s.comparisons, [r.page.id]: { before: r.before, beforeAt: r.before_at, changed: r.changed, error: r.error } },
  }));
}

/** Runs one analysis again on the project as it is now. */
export async function rerunPage(id: number): Promise<StatRerun> {
  const r = await call('stats.notebook.rerun', { id });
  applyRerun(r);
  return r;
}

/** Runs every analysis again on the project as it is now. */
export async function rerunAll(): Promise<RerunSummary> {
  const all = await call('stats.notebook.rerun_all', {});
  all.forEach(applyRerun);
  const summary = { total: all.length, changed: all.filter((r) => r.changed).length, failed: all.filter((r) => r.error !== null).length };
  statsStore.set({ rerunSummary: summary });
  return summary;
}

/** Fetches what a page said the last time before it was run again, to compare with what it says now. */
export async function loadEarlierRun(id: number): Promise<void> {
  const previous = await call('stats.notebook.previous', { id });
  if (previous === null) return;
  statsStore.set((s) => ({ comparisons: { ...s.comparisons, [id]: { before: previous.result, beforeAt: previous.ran_at, changed: true, error: null } } }));
}
