// Every command the person can run, in one list. The ribbon, the command palette (Ctrl+K), and the keyboard
// shortcuts all read it, so a command is named, described, and switched on or off in exactly one place.
//
// A command that cannot run right now stays visible and says why ("Open a source first"), instead of
// vanishing or being greyed out without a reason. The tabs of the ribbon row only choose which commands are on
// show (RIBBON below, in the order of the screen specification book).

import type { IconName } from '../assets/icons.generated';
import { scopeChip } from '../lib/retrieval';
import { isApplePlatform } from '../lib/platform';
import { closeProject, goAi, goAutoCode, goExplore, goTo, inVivo, repeatLastCode, resetScope, saveBackup, saveProject, setExploreMatrix, setPane, showPassages, undoLast } from './actions';
import { announce } from './announce';
import { resetWorkingSet } from './workingSet';
import { appStore, focusApplyBox, focusCenter, jumpUncoded, type AppState, type RibbonTabId, type Workspace } from './app';
import { N_, t } from '../i18n';

export type CommandGroup = 'project' | 'documents' | 'add' | 'coding' | 'explore' | 'analyze' | 'find' | 'view' | 'go' | 'help';

export const GROUP_LABELS: Record<CommandGroup, string> = {
  get project() { return t('Project'); },
  get documents() { return t('Documents'); },
  get add() { return t('Add'); },
  get coding() { return t('Coding'); },
  get explore() { return t('Explore'); },
  get analyze() { return t('Analyze & Report'); },
  get find() { return t('Find'); },
  get view() { return t('View'); },
  get go() { return t('Go to'); },
  get help() { return t('Help'); },
};

export interface Command {
  id: string;
  /** Short, for under an icon on the ribbon. */
  label: string;
  /** The whole sentence, for the palette and the tooltip. */
  title: string;
  group: CommandGroup;
  icon: IconName;
  /** The shortcut as the person presses it on Windows; a Mac shows its own key in `shortcutFor`. */
  shortcut?: string;
  /** For a command whose words depend on the project (the scope chip): the label to show now. */
  dynamicLabel?: (s: AppState) => string;
  /** Extra words the palette also matches, so "save" finds Backup. */
  keywords?: string;
  /** Null when the command can run now; otherwise the reason it cannot, in words for the person. */
  unavailable: (s: AppState) => string | null;
  run: () => void | Promise<unknown>;
}

/** The shortcut as it is pressed on this computer. */
export function shortcutFor(c: Pick<Command, 'shortcut'>, apple: boolean = isApplePlatform()): string | undefined {
  if (!c.shortcut) return undefined;
  return apple ? c.shortcut.replace(/^Ctrl\+/, '⌘') : c.shortcut;
}

const readOnly = (s: AppState): string | null => (s.project?.read_only ? t('This project is read only.') : null);

const needsSource = (s: AppState): string | null => {
  if (s.project?.read_only) return t('This project is read only.');
  if (s.workspace !== 'code') return t('Go to the coding workspace and open a document first.');
  if (s.currentSourceId === null) return t('Open a document first.');
  return null;
};

/** For parts of the book's ribbon that are not built yet: the command is on show, and says so. */
const notBuilt = (what: string) => (): string => t('{what} is not built yet. It is planned for a later version.', { what });

/** The pages of the program that open in the work area. The ribbon, the title bar gear, and the palette reach them. */
const GO: { id: Workspace; label: string; title: string; icon: IconName; keywords: string }[] = [
  { id: 'home', get label() { return t('Project overview'); }, get title() { return t('Go to the project overview: counts, a next step, and the activity history'); }, icon: 'ws-home', get keywords() { return t('home dashboard history activity'); } },
  { id: 'import', get label() { return t('Import Data'); }, get title() { return t('Go to Import Data: add files, see what will come in, and import them'); }, icon: 'cmd-import', get keywords() { return t('import add files documents queue drop spreadsheet'); } },
  { id: 'data', get label() { return t('Data table'); }, get title() { return t('Go to the table of documents and cases'); }, icon: 'ws-data', get keywords() { return t('data sources cases files import'); } },
  { id: 'code', get label() { return t('Coding workspace'); }, get title() { return t('Go back to the coding workspace: documents, codes, the reader, and details'); }, icon: 'ws-code', get keywords() { return t('code coding codebook read home'); } },
  { id: 'codebook', get label() { return t('Codebook Editor'); }, get title() { return t('Go to the Codebook Editor: what each code means, with its examples, usage, and history'); }, icon: 'code-set', get keywords() { return t('codebook definition criteria hierarchy compare versions code editor'); } },
  { id: 'themes', get label() { return t('Themes page'); }, get title() { return t('Go to Themes: the themes of your codebook with the codes under each, what they hold, and quotations that show them'); }, icon: 'exp-concept-map', get keywords() { return t('themes thematic analysis candidate review define name group codes map braun clarke'); } },
  { id: 'explore', get label() { return t('Explore'); }, get title() { return t('Go to Explore: queries, the code matrix, how codes occur together, a summary of each code, and charts'); }, icon: 'exp-query', get keywords() { return t('explore query matrix relations summary chart visualize compare cases'); } },
  { id: 'autocode', get label() { return t('Auto Code'); }, get title() { return t('Go to Auto Code: find passages by a rule and review the suggested codes'); }, icon: 'ai-suggest-codes', get keywords() { return t('auto code automatic suggest review queue speakers headings'); } },
  { id: 'ai', get label() { return t('AI Assistant'); }, get title() { return t('Go to the AI Assistant: build a prompt packet for a chatbot, paste the answer back, and see the log of AI use'); }, icon: 'cmd-ai-assist', get keywords() { return t('ai assistant chatbot prompt packet suggest model log disclosure'); } },
  { id: 'text', get label() { return t('Text Analytics page'); }, get title() { return t('Go to Text Analytics: which words your documents use, and where a word is used'); }, icon: 'ws-text', get keywords() { return t('text analytics words frequency count keyword context kwic concordance'); } },
  { id: 'stats', get label() { return t('Statistics page'); }, get title() { return t('Go to Statistics: describe variables and test them with the test chooser, with an output notebook of APA tables'); }, icon: 'ws-stats', get keywords() { return t('statistics stats test t-test anova chi-square correlation regression mann-whitney kruskal wilcoxon fisher describe mean notebook apa'); } },
  { id: 'variables', get label() { return t('Data editor'); }, get title() { return t('Go to the Data editor: the variables of cases and their values, and new variables made by formulas and recodings'); }, icon: 'ws-data', get keywords() { return t('data editor variables values compute recode filter standardize z-score rank cut missing labels cases spss'); } },
  { id: 'mixed', get label() { return t('Mixed Methods page'); }, get title() { return t('Go to Mixed Methods: turn codes into variables of cases to test, and turn the values of a variable into codes'); }, icon: 'ws-mixed', get keywords() { return t('mixed methods quantitize qualitize quantitizing qualitizing codes into variables variables into codes present count dominant'); } },
  { id: 'team', get label() { return t('Team page'); }, get title() { return t('Go to the Team page: how well the coders agree'); }, icon: 'ws-team', get keywords() { return t('team coders agreement reliability kappa alpha concordance'); } },
  { id: 'synthesize', get label() { return t('Memos page'); }, get title() { return t('Go to the memos page'); }, icon: 'ws-synth', get keywords() { return t('synthesize memos notes write'); } },
  { id: 'report', get label() { return t('Report page'); }, get title() { return t('Go to the report page: export the codebook and the coded passages'); }, icon: 'ws-report', get keywords() { return t('report export codebook passages'); } },
  { id: 'privacy', get label() { return t('Pseudonymize and redact'); }, get title() { return t('Go to Pseudonymize and redact: a list of names and numbers to replace, in what you export or in the project itself'); }, icon: 'set-privacy', get keywords() { return t('privacy pseudonymize pseudonym redact anonymize anonymise de-identify deidentify names confidential identifiers mask'); } },
  { id: 'settings', get label() { return t('Settings'); }, get title() { return t('Go to Settings: colors, text size, reading, and the screen reader'); }, icon: 'ws-settings', get keywords() { return t('appearance theme dark light contrast text size font zoom accessibility'); } },
];

export const COMMANDS: Command[] = [
  // ---- Project
  {
    id: 'new-project',
    get label() { return t('New Project'); },
    get title() { return t('Start a new project'); },
    group: 'project',
    icon: 'ui-new-file',
    get keywords() { return t('create study blank template'); },
    unavailable: () => null,
    run: () => appStore.set({ dialog: { kind: 'new-project' } }),
  },
  {
    id: 'open-project',
    get label() { return t('Open Project'); },
    get title() { return t('Open another project file'); },
    group: 'project',
    icon: 'src-folder',
    get keywords() { return t('file switch study'); },
    unavailable: () => null,
    run: () => appStore.set({ dialog: { kind: 'open-project' } }),
  },
  {
    id: 'import-qdpx',
    get label() { return t('Import a QDPX project'); },
    get title() { return t('Bring in a whole project from another qualitative analysis program, as a new project'); },
    group: 'project',
    icon: 'cmd-import',
    get keywords() { return t('refi qda exchange other program'); },
    unavailable: () => null,
    run: () => appStore.set({ dialog: { kind: 'import-qdpx' } }),
  },
  {
    id: 'save',
    get label() { return t('Save'); },
    get title() { return t('Save the project (every change is written as it is made)'); },
    group: 'project',
    icon: 'ui-save',
    shortcut: 'Ctrl+S',
    get keywords() { return t('write commit persist'); },
    unavailable: () => null,
    run: () => saveProject(),
  },
  {
    id: 'undo',
    get label() { return t('Undo'); },
    get title() { return t('Undo the last change'); },
    group: 'project',
    icon: 'cmd-undo',
    shortcut: 'Ctrl+Z',
    get keywords() { return t('back revert'); },
    unavailable: readOnly,
    run: () => undoLast(),
  },
  {
    id: 'backup',
    get label() { return t('Backup'); },
    get title() { return t('Save a dated backup copy of the whole project now'); },
    group: 'project',
    icon: 'cmd-snapshot',
    get keywords() { return t('save snapshot copy'); },
    unavailable: readOnly,
    run: () => saveBackup(),
  },
  // ---- Documents
  {
    id: 'import',
    get label() { return t('Add Documents'); },
    get title() { return t('Add documents: import a file'); },
    group: 'documents',
    icon: 'cmd-import',
    get keywords() { return t('import add file interview document spreadsheet survey'); },
    unavailable: readOnly,
    run: () => goTo('import'),
  },
  {
    id: 'import-folder',
    get label() { return t('Add Folder'); },
    get title() { return t('Add documents: import a whole folder of files'); },
    group: 'documents',
    icon: 'src-folder',
    get keywords() { return t('import many batch'); },
    unavailable: readOnly,
    run: () => appStore.set({ dialog: { kind: 'import-folder' } }),
  },
  // ---- Add
  {
    id: 'new-code',
    get label() { return t('New Code'); },
    get title() { return t('Add a code to the codebook'); },
    group: 'add',
    icon: 'cmd-new-code',
    get keywords() { return t('add create codebook'); },
    unavailable: readOnly,
    run: () => appStore.set({ dialog: { kind: 'new-code', parentId: null } }),
  },
  {
    id: 'new-theme',
    get label() { return t('New Theme'); },
    get title() { return t('Add a theme that groups other codes, and put codes under it'); },
    group: 'add',
    icon: 'cmd-new-code',
    get keywords() { return t('theme thematic group candidate add create'); },
    unavailable: readOnly,
    run: () => appStore.set({ dialog: { kind: 'new-theme', codeIds: [] } }),
  },
  {
    id: 'import-qdc',
    get label() { return t('Import Codebook'); },
    get title() { return t('Add the codes of a codebook file (REFI-QDA .qdc) from another program to the codebook'); },
    group: 'add',
    icon: 'cmd-import',
    get keywords() { return t('qdc codes refi qda exchange other program codebook'); },
    unavailable: readOnly,
    run: () => appStore.set({ dialog: { kind: 'import-qdc' } }),
  },
  {
    id: 'memo',
    get label() { return t('New Memo'); },
    get title() { return t('Write a memo'); },
    group: 'add',
    icon: 'cmd-memo',
    get keywords() { return t('note write reflect'); },
    unavailable: () => null,
    run: () => goTo('synthesize'),
  },
  {
    id: 'new-case',
    get label() { return t('New Case'); },
    get title() { return t('Add a case: a person, site, or group that documents belong to'); },
    group: 'add',
    icon: 'src-case',
    get keywords() { return t('participant respondent group'); },
    unavailable: readOnly,
    run: () => appStore.set({ dialog: { kind: 'new-case' } }),
  },
  // ---- Coding
  {
    id: 'apply-code',
    get label() { return t('Code Selection'); },
    get title() { return t('Apply a code to the selected words (type to search)'); },
    group: 'coding',
    icon: 'cmd-code',
    get keywords() { return t('tag apply'); },
    unavailable: needsSource,
    run: () => focusApplyBox(),
  },
  {
    id: 'auto-code',
    get label() { return t('Auto Code'); },
    get title() { return t('Find passages by a rule (words, speakers, or headings) and suggest a code for them, to review before anything is applied'); },
    group: 'coding',
    icon: 'ai-suggest-codes',
    get keywords() { return t('automatic suggest search speakers headings review queue'); },
    unavailable: readOnly,
    run: () => goAutoCode('setup'),
  },
  {
    id: 'review-queue',
    get label() { return t('Review Queue'); },
    get title() { return t('Review the codes Auto Code has suggested: accept, reject, or correct each one'); },
    group: 'coding',
    icon: 'ai-review-queue',
    get keywords() { return t('auto code suggestions accept reject pending'); },
    unavailable: readOnly,
    run: () => goAutoCode('queue'),
  },
  {
    id: 'ai-log',
    get label() { return t('AI Use Log'); },
    get title() { return t('See every prompt packet, the answer pasted for it, what the check refused, and what you decided; and get a sentence for your methods section'); },
    group: 'coding',
    icon: 'ai-log',
    get keywords() { return t('ai log disclosure methods record prompt packet model'); },
    unavailable: () => null,
    run: () => goAi('log'),
  },
  {
    id: 'in-vivo',
    get label() { return t('Code In Vivo'); },
    get title() { return t('Make a code from the selected words and apply it'); },
    group: 'coding',
    icon: 'cmd-invivo',
    shortcut: 'I',
    get keywords() { return t('new code from words'); },
    unavailable: needsSource,
    run: () => inVivo().then(() => focusCenter()),
  },
  {
    id: 'repeat-code',
    get label() { return t('Quick Code'); },
    get title() { return t('Apply the code used last to the selected words'); },
    group: 'coding',
    icon: 'cmd-quick-code',
    shortcut: 'Ctrl+D',
    get keywords() { return t('repeat last again recent'); },
    unavailable: (s) => needsSource(s) ?? (s.lastCodeId === null ? t('Apply a code first.') : null),
    run: () => repeatLastCode().then(() => focusCenter()),
  },
  {
    id: 'prev-uncoded',
    get label() { return t('Previous Uncoded'); },
    get title() { return t('Jump to the previous unit that has no coding yet'); },
    group: 'coding',
    icon: 'cmd-previous-gap',
    shortcut: 'Shift+N',
    get keywords() { return t('find gap missing'); },
    unavailable: needsSource,
    run: () => jumpUncoded(-1),
  },
  {
    id: 'next-uncoded',
    get label() { return t('Next Uncoded'); },
    get title() { return t('Jump to the next unit that has no coding yet'); },
    group: 'coding',
    icon: 'cmd-next-gap',
    shortcut: 'N',
    get keywords() { return t('find gap missing'); },
    unavailable: needsSource,
    run: () => jumpUncoded(1),
  },
  // ---- Explore
  {
    id: 'code-summary',
    get label() { return t('Code Summary'); },
    get title() { return t('Summarize one code: how often, in which documents, by which cases'); },
    group: 'explore',
    icon: 'syn-summary-grid',
    get keywords() { return t('frequency counts'); },
    unavailable: () => null,
    run: () => goExplore('summary'),
  },
  {
    id: 'code-relations',
    get label() { return t('Code Relations'); },
    get title() { return t('See how codes occur together'); },
    group: 'explore',
    icon: 'exp-relations',
    get keywords() { return t('co-occurrence overlap network'); },
    unavailable: () => null,
    run: () => goExplore('relations'),
  },
  {
    id: 'query',
    get label() { return t('Query'); },
    get title() { return t('Build a query over codes, documents, and cases'); },
    group: 'explore',
    icon: 'exp-query',
    get keywords() { return t('search matrix boolean'); },
    unavailable: () => null,
    run: () => goExplore('query'),
  },
  // ---- Analyze & Report
  {
    id: 'visualize',
    get label() { return t('Visualize'); },
    get title() { return t('Draw a chart or map of the coding'); },
    group: 'analyze',
    icon: 'exp-concept-map',
    get keywords() { return t('chart graph map'); },
    unavailable: () => null,
    run: () => goExplore('visualize'),
  },
  {
    id: 'document-portrait',
    get label() { return t('Document Portrait'); },
    get title() { return t('See where the coded passages fall along each document, or along one document code by code'); },
    group: 'analyze',
    icon: 'exp-concept-map',
    get keywords() { return t('portrait codeline code line document strip where coded position along'); },
    unavailable: () => null,
    run: () => goExplore('portrait'),
  },
  {
    id: 'compare-cases',
    get label() { return t('Compare Cases'); },
    get title() { return t('Compare cases side by side'); },
    group: 'analyze',
    icon: 'mix-case-compare',
    get keywords() { return t('case matrix'); },
    unavailable: () => null,
    run: () => {
      setExploreMatrix({ rowKind: 'cases' });
      goExplore('matrix');
    },
  },
  {
    id: 'report-builder',
    get label() { return t('Report Builder'); },
    get title() { return t('Open the report page to build a report from the project, or to export the codebook and the coded passages'); },
    group: 'analyze',
    icon: 'rep-outline',
    get keywords() { return t('write document export'); },
    unavailable: () => null,
    run: () => goTo('report'),
  },
  {
    id: 'export',
    get label() { return t('Export'); },
    get title() { return t('Export the codebook or the coded passages'); },
    group: 'analyze',
    icon: 'cmd-export',
    get keywords() { return t('save excel word csv report'); },
    unavailable: () => null,
    run: () => appStore.set({ dialog: { kind: 'export' } }),
  },
  // ---- Find and filters
  {
    id: 'search',
    get label() { return t('Search project'); },
    get title() { return t('Search the text of every document'); },
    group: 'find',
    icon: 'cmd-search',
    shortcut: 'Ctrl+K',
    get keywords() { return t('find words phrase'); },
    unavailable: () => null,
    run: () => appStore.set({ dialog: { kind: 'palette', mode: 'search' } }),
  },
  {
    id: 'passages',
    get label() { return t('Retrieved Segments'); },
    get title() { return t('Go to Retrieved Segments: every passage coded with the codes you choose'); },
    group: 'find',
    icon: 'cmd-retrieve',
    get keywords() { return t('retrieve passages filter codes'); },
    unavailable: () => null,
    run: () => showPassages(),
  },
  {
    id: 'scope',
    get label() { return t('No filter'); },
    get title() { return t('Which codes are chosen for Retrieved Segments; press to go there'); },
    group: 'find',
    icon: 'cmd-scope',
    get keywords() { return t('active chosen filter scope'); },
    dynamicLabel: (s) => scopeChip(s.activeCodeIds.length, s.retrievalMode),
    unavailable: () => null,
    run: () => showPassages(),
  },
  {
    id: 'working-set',
    get label() { return t('Working Set'); },
    get title() { return t('Choose the documents, cases, and codes to work with; every page that can follow does'); },
    group: 'find',
    icon: 'cmd-scope',
    get keywords() { return t('activate activation filter focus subset documents cases codes only these'); },
    unavailable: () => null,
    run: () => appStore.set({ dialog: { kind: 'working-set' } }),
  },
  {
    id: 'reset-working-set',
    get label() { return t('Show Everything'); },
    get title() { return t('Work with the whole project again: stop narrowing the pages to a working set'); },
    group: 'find',
    icon: 'cmd-reset-scope',
    get keywords() { return t('clear working set activation filter all documents cases codes'); },
    unavailable: (s) => (s.workingSet.sourceIds === null && s.workingSet.caseIds === null && s.workingSet.codeIds === null ? t('The whole project is already in use.') : null),
    run: () => resetWorkingSet(),
  },
  {
    id: 'reset-scope',
    get label() { return t('Reset'); },
    get title() { return t('Clear every filter on Retrieved Segments, so nothing is filtered'); },
    group: 'find',
    icon: 'cmd-reset-scope',
    get keywords() { return t('clear filter scope active'); },
    unavailable: (s) => (s.activeCodeIds.length === 0 && s.retrievalMode === 'all' && s.retrievalCase === null && s.retrievalQuery.trim() === '' ? t('Nothing is filtered.') : null),
    run: () => resetScope(),
  },
  // ---- View: the four areas of the coding workspace
  ...(
    [
      ['documents', N_('Documents'), 'src-folder'],
      ['codes', N_('Codes'), 'code-code'],
      ['results', N_('Retrieved Segments'), 'cmd-retrieve'],
      ['details', N_('Segment Details'), 'ws-synth'],
    ] as const
  ).map<Command>(([pane, label, icon]) => ({
    id: `pane-${pane}`,
    get label() { return t(label); },
    get title() { return t('Show or hide {label}', { label: t(label) }); },
    group: 'view',
    icon,
    keywords: t('panel pane area layout toggle'),
    dynamicLabel: undefined,
    unavailable: () => null,
    run: () => {
      const now = appStore.get().panes[pane];
      setPane(pane, !now);
      announce(now ? t('{label} hidden.', { label: t(label) }) : t('{label} shown.', { label: t(label) }));
    },
  })),
  ...GO.map<Command>((g) => ({
    id: `go-${g.id}`,
    get label() { return g.label; },
    get title() { return g.title; },
    group: 'go',
    icon: g.icon,
    get keywords() { return g.keywords; },
    unavailable: () => null,
    run: () => goTo(g.id),
  })),
  {
    id: 'shortcuts',
    get label() { return t('Shortcuts'); },
    get title() { return t('Show the keyboard shortcuts'); },
    group: 'help',
    icon: 'ui-help',
    shortcut: 'F1',
    get keywords() { return t('help keys keyboard'); },
    unavailable: () => null,
    run: () => appStore.set({ dialog: { kind: 'help' } }),
  },
  {
    id: 'close-project',
    get label() { return t('Close project'); },
    get title() { return t('Close this project and go back to the start screen'); },
    group: 'help',
    icon: 'ui-close',
    get keywords() { return t('quit exit open another'); },
    unavailable: () => null,
    run: () => closeProject().catch((e) => announce(String(e?.message ?? e), 'assertive')),
  },
];

// ---------------------------------------------------------------- the ribbon

/** A ribbon button: a command, or a command with a menu of related ones beside it (the chevron in the book). */
export type RibbonItem = string | { id: string; menu: string[] };

export interface RibbonGroup {
  /** The caption under the buttons; empty for a group that has none in the book. */
  caption: string;
  items: RibbonItem[];
}

export const RIBBON_TABS: { id: RibbonTabId; label: string }[] = [
  { id: 'home', get label() { return t('Home'); } },
  { id: 'import', get label() { return t('Import'); } },
  { id: 'code', get label() { return t('Code'); } },
  { id: 'memos', get label() { return t('Memos'); } },
  { id: 'explore', get label() { return t('Explore'); } },
  { id: 'mixed', get label() { return t('Mixed Methods'); } },
  { id: 'visual', get label() { return t('Visual Tools'); } },
  { id: 'synthesize', get label() { return t('Synthesize'); } },
  { id: 'text', get label() { return t('Text Analytics'); } },
  { id: 'stats', get label() { return t('Statistics'); } },
  { id: 'report', get label() { return t('Report'); } },
  { id: 'team', get label() { return t('Team'); } },
  { id: 'ai', label: 'AI' },
  { id: 'view', get label() { return t('View'); } },
];

/**
 * What each tab shows. Home is the book's Home ribbon, button for button. The other tabs hold the commands that
 * used to be on the one ribbon (Undo, Backup, folder import, Retrieved Segments, the filter) and the pages that
 * open in the work area. A tab with no groups is for a part of the program that is not built yet.
 */
export const RIBBON: Record<RibbonTabId, RibbonGroup[]> = {
  home: [
    { get caption() { return t('Project'); }, items: ['new-project', 'open-project', 'save'] },
    { caption: '', items: [{ id: 'import', menu: ['import', 'import-folder'] }] },
    { get caption() { return t('Add'); }, items: ['new-code', 'memo', 'new-case'] },
    { get caption() { return t('Coding'); }, items: ['apply-code', 'auto-code', 'in-vivo', 'repeat-code'] },
    { get caption() { return t('Explore'); }, items: ['code-summary', 'code-relations', 'query'] },
    { get caption() { return t('Analyze & Report'); }, items: ['visualize', 'compare-cases', { id: 'report-builder', menu: ['report-builder', 'export'] }] },
  ],
  import: [
    { get caption() { return t('Add'); }, items: [{ id: 'import', menu: ['import', 'import-folder'] }, 'import-folder'] },
    { get caption() { return t('Look at'); }, items: ['go-data'] },
    { get caption() { return t('Prepare'); }, items: ['go-privacy'] },
  ],
  code: [
    { get caption() { return t('Coding'); }, items: ['apply-code', 'repeat-code', 'in-vivo', 'prev-uncoded', 'next-uncoded'] },
    { get caption() { return t('Auto Code'); }, items: ['auto-code', 'review-queue'] },
    { get caption() { return t('Codes'); }, items: ['new-code', 'new-theme', 'import-qdc', 'go-codebook', 'go-themes'] },
    { get caption() { return t('Project'); }, items: ['undo', 'backup'] },
  ],
  memos: [{ get caption() { return t('Memos'); }, items: ['memo', 'go-synthesize'] }],
  explore: [
    { get caption() { return t('Explore'); }, items: ['code-summary', 'code-relations', 'query'] },
    { get caption() { return t('Find'); }, items: ['passages', 'search'] },
  ],
  mixed: [{ get caption() { return t('Mixed Methods'); }, items: ['go-mixed', 'go-stats', 'go-variables'] }],
  visual: [{ get caption() { return t('Visual Tools'); }, items: ['visualize', 'document-portrait'] }],
  synthesize: [{ get caption() { return t('Synthesize'); }, items: ['go-synthesize'] }],
  text: [{ get caption() { return t('Text Analytics'); }, items: ['go-text'] }],
  stats: [{ get caption() { return t('Statistics'); }, items: ['go-stats', 'go-variables'] }],
  report: [{ get caption() { return t('Report'); }, items: ['report-builder', 'export', 'go-report'] }],
  team: [{ get caption() { return t('Team'); }, items: ['go-team'] }],
  ai: [
    { get caption() { return t('Suggestions'); }, items: ['auto-code', 'review-queue'] },
    { get caption() { return t('AI Assistant'); }, items: ['go-ai', 'ai-log'] },
  ],
  view: [
    { get caption() { return t('Areas'); }, items: ['pane-documents', 'pane-codes', 'pane-results', 'pane-details'] },
    { get caption() { return t('Filters'); }, items: ['scope', 'reset-scope', 'working-set', 'reset-working-set'] },
    { get caption() { return t('Pages'); }, items: ['go-code', 'go-codebook', 'go-themes', 'go-explore', 'go-ai', 'go-team', 'go-text', 'go-stats', 'go-variables', 'go-mixed', 'go-home', 'go-data', 'go-synthesize', 'go-report', 'go-privacy', 'go-settings'] },
    { get caption() { return t('Help'); }, items: ['shortcuts'] },
  ],
};

/** What to say on a tab with no commands: which part of the program it will be. */
export const NOT_BUILT_TABS: Partial<Record<RibbonTabId, string>> = {};

export function commandById(id: string): Command {
  const c = COMMANDS.find((x) => x.id === id);
  if (!c) throw new Error(`no command ${id}`);
  return c;
}

/** Runs a command, or says why it cannot run. */
export async function runCommand(c: Command): Promise<void> {
  const reason = c.unavailable(appStore.get());
  if (reason) {
    announce(t('{title} is not available right now. {reason}', { title: c.title, reason }));
    return;
  }
  await c.run();
}

/** Does a command match what was typed? Every typed word has to appear somewhere in its name, description, or keywords. */
export function commandMatches(c: Pick<Command, 'label' | 'title' | 'keywords'>, typed: string): boolean {
  const words = typed.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const hay = `${c.label} ${c.title} ${c.keywords ?? ''}`.toLowerCase();
  return words.every((w) => hay.includes(w));
}
