// The person's own settings: how the window looks and what it says aloud. They are kept on this computer
// (never in the project file, since they belong to the person and not to the study) and applied to the page.
import { t } from '../i18n';

export type Theme = 'system' | 'light' | 'dark' | 'contrast';

/** How far the lines of text run: across all the room there is, or no longer than is comfortable to read. */
export type TextWidth = 'fill' | 'comfortable';

export interface Settings {
  /** The colors: follow Windows, or always light, always dark, or the high-contrast colors. */
  theme: Theme;
  /** Text size as a percentage of the standard size (100 is 14 pixels; everything on the page scales with it). */
  textSize: number;
  /** Say, for a screen reader, which codes the caret is in as it moves through the text. */
  announceCaret: boolean;
  /** Number the lines of text as they are shown, in a column beside the text. */
  lineNumbers: boolean;
  /** Whether the text spreads across the whole middle of the window or keeps to a comfortable line length. */
  textWidth: TextWidth;
  /** Mark where each unit (sentence, speaker turn, answer) starts, with a thin line and the speaker's name. A project that codes whole units always shows them. */
  showUnits: boolean;
  /** How many dated backups each project keeps (the engine removes the oldest beyond this). */
  keepBackups: number;
  /** A folder the person chose for backups; empty means beside each project (the standard). Only a folder that was checked is kept. */
  backupFolder: string;
  /** Show the small bar of codes above words selected with the mouse. The keyboard never shows it. */
  floatingBar: boolean;
  /** How a new project codes unless its form says otherwise: any selected words, or whole units. */
  codingMode: 'free_span' | 'unit';
  /** The language of the words in the window: 'auto' follows the computer's languages, or a code such as 'fr'. */
  language: string;
  /** The name the person goes by in the history of a project (recorded on their changes, and the coder they are). Empty means the name the program made for this computer; it is never the Windows account name. */
  historyName: string;
  /** Statistics: the confidence level the intervals begin with. */
  confidenceLevel: number;
  /** Statistics: how many resamples a bootstrap begins with. */
  bootstrapResamples: number;
  /** Statistics: how pairs of groups are compared after an analysis of variance or Kruskal-Wallis, to begin with. */
  pairMethod: PairMethod;
  /** Statistics: the seed a bootstrap begins with, or null for a new one each time the notebook starts. */
  fixedSeed: number | null;
  /** Queries, matrices, and relations: a code also stands for its sub-codes, to begin with. */
  subcodes: boolean;
}

/** How pairs of groups can be compared: three tests made for pairs after an analysis of variance, and the adjustments of p-values. */
export type PairMethod = 'tukey' | 'games_howell' | 'scheffe' | 'holm' | 'bonferroni' | 'benjamini_hochberg' | 'none';

export const PAIR_METHOD_IDS: PairMethod[] = ['tukey', 'games_howell', 'scheffe', 'holm', 'bonferroni', 'benjamini_hochberg', 'none'];

/** The confidence levels and bootstrap sizes the settings offer (the Statistics page offers the same). */
export const CONFIDENCE_LEVELS = [0.9, 0.95, 0.99] as const;
export const RESAMPLE_CHOICES = [1000, 2000, 5000, 10000] as const;

export const THEMES: { id: Theme; label: string; help: string }[] = [
  { id: 'system', get label() { return t('Follow Windows'); }, get help() { return t('Light or dark, whichever Windows is set to.'); } },
  { id: 'light', get label() { return t('Light'); }, get help() { return t('Dark text on a light background.'); } },
  { id: 'dark', get label() { return t('Dark'); }, get help() { return t('Light text on a dark background.'); } },
  { id: 'contrast', get label() { return t('High Contrast'); }, get help() { return t('White text on black with a yellow accent and strong outlines.'); } },
];

/** How many backups a project can be told to keep. */
export const KEEP_BACKUP_CHOICES = [3, 5, 10, 20] as const;

/** The sizes offered, as percentages of the standard size. */
export const TEXT_SIZES = [90, 100, 115, 130, 150] as const;

/** The page is sized in rem from this: 87.5 percent of the browser's 16 pixels is the standard 14. */
export const STANDARD_FONT_PERCENT = 87.5;

export const SETTINGS_KEY = 'prismqda.settings';

export const DEFAULT_SETTINGS: Settings = { confidenceLevel: 0.95, bootstrapResamples: 2000, pairMethod: 'holm', fixedSeed: null, subcodes: true, theme: 'system', textSize: 100, announceCaret: true, lineNumbers: true, textWidth: 'fill', showUnits: false, keepBackups: 10, backupFolder: '', floatingBar: true, codingMode: 'free_span', language: 'auto', historyName: '' };

/** Cleans what was read from storage: anything unknown falls back to the default, and a size snaps to one that is offered. */
export function normalizeSettings(raw: unknown): Settings {
  const r = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  const theme = THEMES.some((t) => t.id === r.theme) ? (r.theme as Theme) : DEFAULT_SETTINGS.theme;
  let textSize: number = DEFAULT_SETTINGS.textSize;
  if (typeof r.textSize === 'number' && Number.isFinite(r.textSize)) {
    const size = r.textSize;
    textSize = TEXT_SIZES.reduce((best, s) => (Math.abs(s - size) < Math.abs(best - size) ? s : best), TEXT_SIZES[0]);
  }
  const announceCaret = typeof r.announceCaret === 'boolean' ? r.announceCaret : DEFAULT_SETTINGS.announceCaret;
  const lineNumbers = typeof r.lineNumbers === 'boolean' ? r.lineNumbers : DEFAULT_SETTINGS.lineNumbers;
  const textWidth: TextWidth = r.textWidth === 'comfortable' || r.textWidth === 'fill' ? r.textWidth : DEFAULT_SETTINGS.textWidth;
  const showUnits = typeof r.showUnits === 'boolean' ? r.showUnits : DEFAULT_SETTINGS.showUnits;
  let keepBackups: number = DEFAULT_SETTINGS.keepBackups;
  if (typeof r.keepBackups === 'number' && Number.isFinite(r.keepBackups)) {
    const keep = r.keepBackups;
    keepBackups = KEEP_BACKUP_CHOICES.reduce((best, k) => (Math.abs(k - keep) < Math.abs(best - keep) ? k : best), KEEP_BACKUP_CHOICES[0]);
  }
  const floatingBar = typeof r.floatingBar === 'boolean' ? r.floatingBar : DEFAULT_SETTINGS.floatingBar;
  const backupFolder = typeof r.backupFolder === 'string' ? r.backupFolder.trim() : DEFAULT_SETTINGS.backupFolder;
  const codingMode: Settings['codingMode'] = r.codingMode === 'unit' || r.codingMode === 'free_span' ? r.codingMode : DEFAULT_SETTINGS.codingMode;
  const language = typeof r.language === 'string' && /^(auto|[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*(~\d{1,3})?)$/.test(r.language) ? r.language : DEFAULT_SETTINGS.language;
  const nearest = <T extends number>(value: unknown, choices: readonly T[], fallback: T): T =>
    typeof value === 'number' && Number.isFinite(value) ? choices.reduce((best, c) => (Math.abs(c - value) < Math.abs(best - value) ? c : best), choices[0]) : fallback;
  const confidenceLevel = nearest(r.confidenceLevel, CONFIDENCE_LEVELS, 0.95);
  const bootstrapResamples = nearest(r.bootstrapResamples, RESAMPLE_CHOICES, 2000);
  const pairMethod: PairMethod = PAIR_METHOD_IDS.includes(r.pairMethod as PairMethod) ? (r.pairMethod as PairMethod) : DEFAULT_SETTINGS.pairMethod;
  const fixedSeed = typeof r.fixedSeed === 'number' && Number.isInteger(r.fixedSeed) && r.fixedSeed >= 0 && r.fixedSeed <= 4_000_000_000 ? r.fixedSeed : null;
  const subcodes = typeof r.subcodes === 'boolean' ? r.subcodes : DEFAULT_SETTINGS.subcodes;
  const historyName = typeof r.historyName === 'string' ? cleanHistoryName(r.historyName) : DEFAULT_SETTINGS.historyName;
  return { theme, textSize, announceCaret, lineNumbers, textWidth, showUnits, keepBackups, backupFolder, floatingBar, codingMode, language, historyName, confidenceLevel, bootstrapResamples, pairMethod, fixedSeed, subcodes };
}

/** A name for the project history as it is kept: on one line, with the spaces tidied, and at most sixty characters. */
export function cleanHistoryName(text: string): string {
  return Array.from(text.replace(/[\u0000-\u001f\u007f\s]+/g, ' ').trim()).slice(0, 60).join('').trim();
}

/** The root font size, as a percentage, for a text size. */
export function fontPercent(textSize: number): number {
  return Math.round(STANDARD_FONT_PERCENT * textSize) / 100;
}

/** What `applySettings` changes on the page's root element. */
export interface RootLike {
  dataset: Record<string, string | undefined>;
  style: { fontSize: string };
}

/** Puts the settings on the page: the theme and the reading choices as attributes the style sheet reads, and the size on the root. */
export function applySettings(s: Settings, root: RootLike): void {
  if (s.theme === 'system') delete root.dataset.theme;
  else root.dataset.theme = s.theme;
  if (s.textWidth === DEFAULT_SETTINGS.textWidth) delete root.dataset.textWidth;
  else root.dataset.textWidth = s.textWidth;
  root.style.fontSize = s.textSize === 100 ? '' : `${fontPercent(s.textSize)}%`;
}

export function themeLabel(theme: Theme): string {
  return THEMES.find((t) => t.id === theme)?.label ?? theme;
}
