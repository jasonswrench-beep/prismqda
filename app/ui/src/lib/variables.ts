// What the data editor needs that is not drawing: reading what a person typed into the boxes (value labels, missing codes, cut points,
// the values of a recoding rule), how a stored value is shown, and how a made variable says what made it. The engine checks everything
// again; these only turn typing into the shapes the commands take, and say what is wrong in words when it cannot be done.

import { t } from '../i18n';
import type { DatePartId, GroupFunctionId, Level, TransformKindId, TransformSpec, VariableDef } from '../types';

/** The most values that can mean missing in one variable (the engine refuses a fourth). */
export const MAX_MISSING_CODES = 3;

/** The most rows of the data view drawn at once. A project can hold tens of thousands of cases. */
export const ROWS_PER_PAGE = 100;

// ---------------------------------------------------------------- value labels

/** One label to a line: `1 = Agree`. */
export function labelsToText(labels: Record<string, string>): string {
  return Object.entries(labels)
    .map(([value, label]) => `${value} = ${label}`)
    .join('\n');
}

export function parseLabels(text: string): { labels: Record<string, string>; error: string | null } {
  const labels: Record<string, string> = {};
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (line === '') continue;
    const at = line.indexOf('=');
    if (at < 0) return { labels, error: t('Line {n} has no equals sign. Write each label as a value, an equals sign, then the words, for example 1 = Agree.', { n: i + 1 }) };
    const value = line.slice(0, at).trim();
    const label = line.slice(at + 1).trim();
    if (value === '') return { labels, error: t('Line {n} has no value before the equals sign.', { n: i + 1 }) };
    if (label === '') return { labels, error: t('Line {n} has no words after the equals sign.', { n: i + 1 }) };
    if (Object.prototype.hasOwnProperty.call(labels, value)) return { labels, error: t('The value {value} is labeled twice.', { value }) };
    labels[value] = label;
  }
  return { labels, error: null };
}

// ---------------------------------------------------------------- missing values

export function codesToText(codes: string[]): string {
  return codes.join(', ');
}

/** Values that mean missing, separated by commas or lines. */
export function parseCodes(text: string): { codes: string[]; error: string | null } {
  const codes = text
    .split(/[,\n;]/)
    .map((c) => c.trim())
    .filter((c) => c !== '');
  const unique = [...new Set(codes)];
  if (unique.length > MAX_MISSING_CODES) {
    return { codes: unique, error: t('At most {max} values can mean missing. Write them separated by commas.', { max: MAX_MISSING_CODES }) };
  }
  return { codes: unique, error: null };
}

// ---------------------------------------------------------------- a recoding's values and a cut's points

/** The values of one recoding rule: separated by commas or lines. */
export function parseValueList(text: string): string[] {
  return text
    .split(/[,\n;]/)
    .map((v) => v.trim())
    .filter((v) => v !== '');
}

/** Cut points: numbers, in any order, separated by commas, spaces, or semicolons. */
export function parseCuts(text: string): { cuts: number[]; error: string | null } {
  const parts = text.split(/[\s,;]+/).filter((p) => p !== '');
  if (parts.length === 0) return { cuts: [], error: t('Write at least one cut point, a number where one range ends and the next begins.') };
  const cuts: number[] = [];
  for (const p of parts) {
    const n = Number(p);
    if (!Number.isFinite(n)) return { cuts: [], error: t('{p} is not a number.', { p }) };
    cuts.push(n);
  }
  cuts.sort((a, b) => a - b);
  for (let i = 1; i < cuts.length; i++) {
    if (cuts[i] === cuts[i - 1]) return { cuts: [], error: t('The cut point {n} is written twice.', { n: cuts[i] }) };
  }
  return { cuts, error: null };
}

// ---------------------------------------------------------------- showing a value

export interface CellShown {
  /** What is drawn: the label when the value has one, the value itself otherwise, and nothing for no value. */
  text: string;
  /** The value is one of the codes that mean missing. */
  missing: boolean;
}

export function cellShown(def: VariableDef, raw: string | null): CellShown {
  if (raw === null || raw === '') return { text: '', missing: false };
  const missing = def.missing_codes.includes(raw);
  const label = def.value_labels[raw];
  return { text: label !== undefined && label !== '' ? label : raw, missing };
}

/** The same cell spoken: the label with its value when they differ, and that it means missing. */
export function cellSpoken(def: VariableDef, raw: string | null): string {
  if (raw === null || raw === '') return t('empty');
  const shown = cellShown(def, raw);
  const base = shown.text === raw ? raw : t('{label}, value {value}', { label: shown.text, value: raw });
  return shown.missing ? t('{value}, counted as missing', { value: base }) : base;
}

// ---------------------------------------------------------------- formulas

const RESERVED = new Set(['TRUE', 'FALSE', 'NA', 'Inf', 'pi', 'T', 'F']);

/** A variable's name as a formula writes it: plain when it can be, in backticks when it has spaces or other marks. */
export function formulaName(name: string): string {
  return /^[\p{L}][\p{L}\p{N}._]*$/u.test(name) && !RESERVED.has(name) ? name : `\`${name}\``;
}

// ---------------------------------------------------------------- the kinds of transformation

export interface KindInfo {
  id: TransformKindId;
  label: string;
  /** One sentence on what it makes. */
  description: string;
  /** It starts from one chosen variable. */
  needsSource: boolean;
}

export function transformKinds(): KindInfo[] {
  return [
    { id: 'compute', label: t('Compute'), description: t('A new variable from a formula over the others, such as score + age or ifelse(age >= 30, 1, 0).'), needsSource: false },
    { id: 'recode', label: t('Recode'), description: t('A new variable that gives chosen values new ones, such as 1 and 2 becoming Low, or 99 becoming missing.'), needsSource: true },
    { id: 'standardize', label: t('Standardize'), description: t('Each case as how many standard deviations it is from the mean (a z-score).'), needsSource: true },
    { id: 'rank', label: t('Rank'), description: t('The place of each case in order, tied cases sharing the average place.'), needsSource: true },
    { id: 'cut', label: t('Cut into ranges'), description: t('Numbers cut into ranges at points you choose, such as age into under 30, 30 to 49, and 50 or more.'), needsSource: true },
    { id: 'date_part', label: t('Part of a date'), description: t('The year, quarter, month, week, day, weekday, or hour of a variable that holds dates.'), needsSource: true },
    { id: 'sample', label: t('Random sample'), description: t('Chooses some of the cases at random, so that a variable says 1 for a case in the sample and 0 for one that is not. The seed makes the same choice every time.'), needsSource: false },
    { id: 'aggregate', label: t('Group summary'), description: t('What each group of cases comes to (the mean, the sum, the most, how many), written on every case of the group.'), needsSource: true },
  ];
}

/** The parts of a date a variable can be made from, in the order they are offered. */
export function dateParts(): { id: DatePartId; label: string }[] {
  return [
    { id: 'year', label: t('Year') },
    { id: 'quarter', label: t('Quarter (1 to 4)') },
    { id: 'month', label: t('Month (1 to 12, named)') },
    { id: 'week', label: t('Week of the year (ISO, 1 to 53)') },
    { id: 'day', label: t('Day of the month') },
    { id: 'day_of_year', label: t('Day of the year (1 to 366)') },
    { id: 'weekday', label: t('Day of the week (Monday is 1)') },
    { id: 'hour', label: t('Hour (0 to 23, when a time is written)') },
  ];
}

/** What can be worked out for each group, in the order it is offered. `counts` is true for those that count instead of working numbers out. */
export function groupFunctions(): { id: GroupFunctionId; label: string; counts: boolean }[] {
  return [
    { id: 'mean', label: t('Mean'), counts: false },
    { id: 'sum', label: t('Sum'), counts: false },
    { id: 'median', label: t('Median'), counts: false },
    { id: 'min', label: t('Smallest'), counts: false },
    { id: 'max', label: t('Largest'), counts: false },
    { id: 'sd', label: t('Standard deviation'), counts: false },
    { id: 'count', label: t('How many of the group have a value'), counts: true },
    { id: 'cases', label: t('How many cases are in the group'), counts: true },
  ];
}

export function kindLabel(id: TransformKindId): string {
  return transformKinds().find((k) => k.id === id)?.label ?? id;
}

/** What made a variable, in a line; null for a variable typed in or imported. */
export function derivedLine(def: VariableDef, codeName: (id: number) => string = (id) => t('code {id}', { id })): string | null {
  const spec: TransformSpec | undefined = def.derived?.transform;
  if (!spec) return null;
  switch (spec.kind) {
    case 'from_codes':
      if (spec.measure === 'present') return t('From the coding: whether a case has {code}', { code: codeName(spec.codes[0]) });
      if (spec.measure === 'count') return t('From the coding: how many excerpts carry {code}', { code: codeName(spec.codes[0]) });
      return t('From the coding: which of {codes} a case has most', { codes: spec.codes.map(codeName).join(', ') });
    case 'compute':
      return t('Computed: {formula}', { formula: spec.formula });
    case 'recode':
      return t('Recoded from {source}', { source: spec.source });
    case 'standardize':
      return t('Z-scores of {source}', { source: spec.source });
    case 'rank':
      return spec.descending ? t('Ranks of {source}, highest first', { source: spec.source }) : t('Ranks of {source}', { source: spec.source });
    case 'cut':
      return t('{source} cut at {cuts}', { source: spec.source, cuts: spec.cuts.join(', ') });
    case 'date_part':
      return t('{part} of {source}', { part: dateParts().find((p) => p.id === spec.part)?.label ?? spec.part, source: spec.source });
    case 'sample':
      return spec.within
        ? t('A random sample ({size}) drawn within {within}, seed {seed}', { size: sampleSizeWords(spec.size), within: spec.within, seed: spec.seed ?? 1 })
        : t('A random sample ({size}), seed {seed}', { size: sampleSizeWords(spec.size), seed: spec.seed ?? 1 });
    case 'aggregate':
      return t('{function} of {source} for each group of {by}', { function: groupFunctions().find((f) => f.id === spec.function)?.label.toLowerCase() ?? spec.function, source: spec.source, by: spec.by.join(' and ') });
  }
}

/** A sample's size in words. */
export function sampleSizeWords(size: { by: 'count' | 'percent'; value: number }): string {
  return size.by === 'count' ? t('{count} cases', { count: size.value }) : t('{percent} percent', { percent: size.value });
}

/** The level a variable is offered in the "make it" dialog: the engine suggests one, and this lists what can be chosen instead. */
export const RESULT_LEVELS: Level[] = ['nominal', 'ordinal', 'interval', 'ratio', 'text'];

/** A spec is ready to send when what it needs is filled in; the words say what is missing. */
export function specProblem(spec: TransformSpec): string | null {
  if (spec.target.trim() === '') return t('Give the new variable a name.');
  switch (spec.kind) {
    case 'compute':
      return spec.formula.trim() === '' ? t('Write the formula.') : null;
    case 'recode':
      if (spec.source === '') return t('Choose the variable to recode.');
      return spec.rules.some((r) => r.from.length > 0) ? null : t('Add at least one rule: the values to change, and the value they become.');
    case 'cut':
      if (spec.source === '') return t('Choose the variable to cut.');
      return spec.cuts.length === 0 ? t('Write at least one cut point.') : null;
    case 'from_codes':
      if (spec.codes.length === 0) return t('Choose the code.');
      return spec.measure === 'dominant' && spec.codes.length < 2 ? t('Choose at least two codes to compare.') : null;
    case 'date_part':
      return spec.source === '' ? t('Choose the variable that holds the dates.') : null;
    case 'sample':
      if (!Number.isFinite(spec.size.value) || spec.size.value <= 0) return spec.size.by === 'count' ? t('Write how many cases the sample holds.') : t('Write the percent of the cases the sample holds.');
      if (spec.size.by === 'count' && !Number.isInteger(spec.size.value)) return t('A sample holds a whole number of cases.');
      if (spec.size.by === 'percent' && spec.size.value > 100) return t('A percent is at most 100.');
      return null;
    case 'aggregate':
      if (spec.source === '') return t('Choose the variable to summarize.');
      return spec.by.length === 0 ? t('Choose at least one variable that makes the groups.') : null;
    default:
      return spec.source === '' ? t('Choose the variable to use.') : null;
  }
}
