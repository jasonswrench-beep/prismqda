// The logic behind the Explore page that needs no screen: the draft of a query and what it says in plain words, what a draft
// is missing, how it becomes the rule the engine reads, and the numbers a chart is drawn from.

import type { Counts, MatrixResult, Metric, QueryNode, QuerySpec } from '../types';
import { num, t, tn } from '../i18n';

// ---------------------------------------------------------------- metrics

export const METRICS: { id: Metric; label: string; help: string }[] = [
  { id: 'assignments', get label() { return t('Assignments'); }, get help() { return t('Codings: each time a code was applied to words, by each coder.'); } },
  { id: 'excerpts', get label() { return t('Distinct excerpts'); }, get help() { return t('Stretches of text: two coders, or two codes, on the same words count once.'); } },
  { id: 'documents', get label() { return t('Documents with evidence'); }, get help() { return t('How many documents have at least one coding.'); } },
  { id: 'cases', get label() { return t('Cases with evidence'); }, get help() { return t('How many cases have at least one coding.'); } },
];

export function metricLabel(m: Metric): string {
  return METRICS.find((x) => x.id === m)?.label ?? m;
}

/** What one metric counts, in the plural, for a sentence ("12 excerpts"). */
export function metricNoun(m: Metric, n: number): string {
  switch (m) {
    case 'assignments':
      return tn(n, { one: '{count} assignment', other: '{count} assignments' });
    case 'excerpts':
      return tn(n, { one: '{count} excerpt', other: '{count} excerpts' });
    case 'documents':
      return tn(n, { one: '{count} document', other: '{count} documents' });
    default:
      return tn(n, { one: '{count} case', other: '{count} cases' });
  }
}

/** What one metric counts, in the plural and with no number, to fill the blank of a sentence ("assignments"). */
export function metricWord(m: Metric): string {
  switch (m) {
    case 'assignments':
      return t('assignments');
    case 'excerpts':
      return t('excerpts');
    case 'documents':
      return t('documents');
    default:
      return t('cases');
  }
}

export function pick(c: Counts, m: Metric): number {
  return c[m];
}

// ---------------------------------------------------------------- the draft of a query

export type ClauseType = 'code' | 'also' | 'text' | 'source' | 'case' | 'variable' | 'coder' | 'rating' | 'coded' | 'between';

export type Clause =
  | { kind: 'clause'; id: number; type: 'code'; codeIds: number[]; negate: boolean }
  | { kind: 'clause'; id: number; type: 'also'; codeIds: number[]; within: number; negate: boolean }
  | { kind: 'clause'; id: number; type: 'text'; contains: string; negate: boolean }
  | { kind: 'clause'; id: number; type: 'source'; ids: number[]; negate: boolean }
  | { kind: 'clause'; id: number; type: 'case'; ids: number[]; negate: boolean }
  | { kind: 'clause'; id: number; type: 'variable'; name: string; value: string; negate: boolean }
  | { kind: 'clause'; id: number; type: 'coder'; ids: number[]; negate: boolean }
  | { kind: 'clause'; id: number; type: 'rating'; min: string; max: string; negate: boolean }
  | { kind: 'clause'; id: number; type: 'coded'; from: string; to: string; negate: boolean }
  | { kind: 'clause'; id: number; type: 'between'; name: string; from: string; to: string; negate: boolean };

export interface Group {
  kind: 'group';
  id: number;
  combine: 'any' | 'all';
  items: (Clause | Group)[];
}

export interface Draft {
  root: Group;
  unit: 'excerpts' | 'assignments';
  /** A code stands for its sub-codes too. */
  descendants: boolean;
  /** `also` clauses look at the same coder's other codings only. */
  sameCoder: boolean;
}

let counter = 1;
export const nextId = (): number => counter++;

export const CLAUSE_TYPES: { id: ClauseType; label: string }[] = [
  { id: 'code', get label() { return t('Code'); } },
  { id: 'also', get label() { return t('Also coded with'); } },
  { id: 'text', get label() { return t('Text contains'); } },
  { id: 'source', get label() { return t('Document'); } },
  { id: 'case', get label() { return t('Case'); } },
  { id: 'variable', get label() { return t('Case variable'); } },
  { id: 'coder', get label() { return t('Coder'); } },
  { id: 'rating', get label() { return t('Rating'); } },
  { id: 'coded', get label() { return t('Date coded'); } },
  { id: 'between', get label() { return t('Case variable between'); } },
];

export function newClause(type: ClauseType): Clause {
  const id = nextId();
  switch (type) {
    case 'code':
      return { kind: 'clause', id, type, codeIds: [], negate: false };
    case 'also':
      return { kind: 'clause', id, type, codeIds: [], within: 0, negate: false };
    case 'text':
      return { kind: 'clause', id, type, contains: '', negate: false };
    case 'source':
      return { kind: 'clause', id, type, ids: [], negate: false };
    case 'case':
      return { kind: 'clause', id, type, ids: [], negate: false };
    case 'variable':
      return { kind: 'clause', id, type, name: '', value: '', negate: false };
    case 'coder':
      return { kind: 'clause', id, type, ids: [], negate: false };
    case 'rating':
      return { kind: 'clause', id, type, min: '', max: '', negate: false };
    case 'coded':
      return { kind: 'clause', id, type, from: '', to: '', negate: false };
    case 'between':
      return { kind: 'clause', id, type, name: '', from: '', to: '', negate: false };
  }
}

// ---------------------------------------------------------------- dates and numbers as limits

/** What a limit written in a box is: a date (year-month-day), a number, or neither. Dates must be real days of the calendar. */
export function limitKind(text: string): 'date' | 'number' | null {
  const t = text.trim();
  const d = /^(\d{4})[-/](\d{1,2})[-/](\d{1,2})$/.exec(t);
  if (d) {
    const [y, m, day] = [Number(d[1]), Number(d[2]), Number(d[3])];
    const check = new Date(Date.UTC(y, m - 1, day));
    return check.getUTCFullYear() === y && check.getUTCMonth() === m - 1 && check.getUTCDate() === day ? 'date' : null;
  }
  return /^[+-]?\d{1,3}(,\d{3})*(\.\d+)?$|^[+-]?\d+(\.\d+)?$/.test(t) ? 'number' : null;
}

/** A date written with slashes or without zeros as year-month-day with zeros (2024/3/4 is 2024-03-04). */
export function normalizeDay(text: string): string {
  const m = /^(\d{4})[-/](\d{1,2})[-/](\d{1,2})$/.exec(text.trim());
  return m ? `${m[1]}-${m[2].padStart(2, '0')}-${m[3].padStart(2, '0')}` : text.trim();
}

/** The number a limit stands for (commas between thousands allowed). */
export function limitNumber(text: string): number {
  return Number(text.trim().replace(/,/g, ''));
}

/** What is wrong with the limits of a range clause, in words, or null. `what` names the clause. */
function rangeProblem(what: string, from: string, to: string, wantNumbersOrDates: boolean): string | null {
  const a = from.trim();
  const b = to.trim();
  if (a === '' && b === '') return t('A "{what}" condition needs a lowest or a highest limit.', { what });
  const kinds = [a, b].filter((v) => v !== '').map(limitKind);
  if (!wantNumbersOrDates) {
    if (kinds.some((k) => k !== 'date')) return t('Write dates year-month-day, such as 2024-03-14.');
  } else if (kinds.some((k) => k === null) || new Set(kinds).size > 1) {
    return t('Write both limits as dates (2024-03-14) or both as numbers.');
  }
  if (a !== '' && b !== '') {
    if (kinds[0] === 'date' ? normalizeDay(a) > normalizeDay(b) : limitNumber(a) > limitNumber(b)) return t('In a "{what}" condition the first limit is above the second.', { what });
  }
  return null;
}

export function newGroup(combine: 'any' | 'all' = 'any'): Group {
  return { kind: 'group', id: nextId(), combine, items: [] };
}

export function emptyDraft(): Draft {
  return { root: newGroup('any'), unit: 'excerpts', descendants: true, sameCoder: true };
}

/**
 * A draft read back from a saved query: checked, and given fresh numbers so what is added to it later cannot share a number with
 * what was saved. Anything that is not a draft (a file from another version, or a damaged one) gives null.
 */
export function revive(value: unknown): Draft | null {
  const types = new Set<string>(CLAUSE_TYPES.map((t) => t.id));
  const group = (v: unknown, depth: number): Group | null => {
    if (typeof v !== 'object' || v === null || depth > 8) return null;
    const g = v as { kind?: unknown; combine?: unknown; items?: unknown };
    if (g.kind !== 'group' || (g.combine !== 'any' && g.combine !== 'all') || !Array.isArray(g.items)) return null;
    const items: (Clause | Group)[] = [];
    for (const item of g.items) {
      const rec = typeof item === 'object' && item !== null ? (item as { kind?: unknown; type?: unknown }) : null;
      if (rec?.kind === 'group') {
        const inner = group(item, depth + 1);
        if (!inner) return null;
        items.push(inner);
      } else if (rec?.kind === 'clause' && typeof rec.type === 'string' && types.has(rec.type)) {
        const base = newClause(rec.type as ClauseType) as unknown as Record<string, unknown>;
        const given = item as Record<string, unknown>;
        // Only the fields this kind of condition has, each as the same kind of value as its default.
        const merged: Record<string, unknown> = { ...base };
        for (const k of Object.keys(base)) if (k !== 'id' && k !== 'kind' && k !== 'type' && k in given && typeof given[k] === typeof base[k] && Array.isArray(given[k]) === Array.isArray(base[k])) merged[k] = given[k];
        items.push(merged as unknown as Clause);
      } else {
        return null;
      }
    }
    return { kind: 'group', id: nextId(), combine: g.combine, items };
  };
  if (typeof value !== 'object' || value === null) return null;
  const d = value as { root?: unknown; unit?: unknown; descendants?: unknown; sameCoder?: unknown };
  const root = group(d.root, 0);
  if (!root || (d.unit !== 'excerpts' && d.unit !== 'assignments')) return null;
  return { root, unit: d.unit, descendants: d.descendants !== false, sameCoder: d.sameCoder !== false };
}

/** Replaces an item (a clause or a group) wherever it is in the draft. */
export function mapItem(group: Group, id: number, change: (item: Clause | Group) => Clause | Group | null): Group {
  return {
    ...group,
    items: group.items.flatMap((item) => {
      if (item.id === id) {
        const next = change(item);
        return next === null ? [] : [next];
      }
      return [item.kind === 'group' ? mapItem(item, id, change) : item];
    }),
  };
}

/** Adds a clause or a group to the group with this id. */
export function addTo(group: Group, groupId: number, item: Clause | Group): Group {
  if (group.id === groupId) return { ...group, items: [...group.items, item] };
  return { ...group, items: group.items.map((i) => (i.kind === 'group' ? addTo(i, groupId, item) : i)) };
}

/** Every clause in the draft, in order. */
export function clausesOf(group: Group): Clause[] {
  return group.items.flatMap((i) => (i.kind === 'group' ? clausesOf(i) : [i]));
}

/** The codes a draft asks about, for carrying them over to the code matrix. */
export function codesOfDraft(draft: Draft): number[] {
  const out: number[] = [];
  for (const c of clausesOf(draft.root)) {
    if (c.type === 'code' || c.type === 'also') for (const id of c.codeIds) if (!out.includes(id)) out.push(id);
  }
  return out;
}

// ---------------------------------------------------------------- what is missing

function clauseProblem(c: Clause): string | null {
  switch (c.type) {
    case 'code':
    case 'also':
      return c.codeIds.length === 0 ? t('A condition about a code has no code chosen.') : null;
    case 'text':
      return c.contains.trim() === '' ? t('A "Text contains" condition has no text to look for.') : null;
    case 'source':
      return c.ids.length === 0 ? t('A "Document" condition has no document chosen.') : null;
    case 'case':
      return c.ids.length === 0 ? t('A "Case" condition has no case chosen.') : null;
    case 'coder':
      return c.ids.length === 0 ? t('A "Coder" condition has no coder chosen.') : null;
    case 'variable':
      return c.name === '' || c.value === '' ? t('A "Case variable" condition needs a variable and a value.') : null;
    case 'rating': {
      const lo = c.min.trim();
      const hi = c.max.trim();
      if (lo === '' && hi === '') return t('A "Rating" condition needs a lowest or a highest rating.');
      for (const v of [lo, hi]) if (v !== '' && !/^[1-5]$/.test(v)) return t('Ratings go from 1 to 5.');
      if (lo !== '' && hi !== '' && Number(lo) > Number(hi)) return t('In a "Rating" condition the lowest rating is higher than the highest.');
      return null;
    }
    case 'coded':
      return rangeProblem(t('Date coded'), c.from, c.to, false);
    case 'between':
      return c.name === '' ? t('A "Case variable between" condition needs a variable.') : rangeProblem(t('Case variable between'), c.from, c.to, true);
  }
}

/** The first thing that stops the query from running, in words, or null when it can run. */
export function problemWith(draft: Draft): string | null {
  if (draft.root.items.length === 0) return t('The query has no conditions yet. Add one to begin.');
  const walk = (g: Group): string | null => {
    for (const item of g.items) {
      if (item.kind === 'group') {
        if (item.items.length === 0) return t('A group has no conditions in it. Add one, or remove the group.');
        const p = walk(item);
        if (p) return p;
      } else {
        const p = clauseProblem(item);
        if (p) return p;
      }
    }
    return null;
  };
  return walk(draft.root);
}

// ---------------------------------------------------------------- from the draft to the rule

function nodeOf(item: Clause | Group, d: Draft): QueryNode {
  if (item.kind === 'group') return { type: 'group', match: item.combine, items: item.items.map((i) => nodeOf(i, d)) };
  const negate = item.negate ? { negate: true } : {};
  switch (item.type) {
    case 'code':
      return { type: 'code', code_ids: item.codeIds, include_descendants: d.descendants, ...negate };
    case 'also':
      return { type: 'also_coded', code_ids: item.codeIds, within_chars: item.within, include_descendants: d.descendants, same_coder: d.sameCoder, ...negate };
    case 'text':
      return { type: 'text', contains: item.contains.trim(), ...negate };
    case 'source':
      return { type: 'source', ids: item.ids, ...negate };
    case 'case':
      return { type: 'case', ids: item.ids, ...negate };
    case 'variable':
      return { type: 'case_variable', name: item.name, value: item.value, ...negate };
    case 'coder':
      return { type: 'coder', ids: item.ids, ...negate };
    case 'rating':
      return { type: 'rating', ...(item.min.trim() !== '' ? { min: Number(item.min) } : {}), ...(item.max.trim() !== '' ? { max: Number(item.max) } : {}), ...negate };
    case 'coded':
      return { type: 'coded_between', ...(item.from.trim() !== '' ? { from: normalizeDay(item.from) } : {}), ...(item.to.trim() !== '' ? { to: normalizeDay(item.to) } : {}), ...negate };
    case 'between': {
      const day = (v: string) => (limitKind(v) === 'date' ? normalizeDay(v) : v.trim());
      return { type: 'case_variable_between', name: item.name, ...(item.from.trim() !== '' ? { from: day(item.from) } : {}), ...(item.to.trim() !== '' ? { to: day(item.to) } : {}), ...negate };
    }
  }
}

/** The rule the engine reads. Only meaningful when `problemWith` says there is no problem. */
export function toQuerySpec(draft: Draft, limit = 500): QuerySpec {
  return { rule: nodeOf(draft.root, draft), result_unit: draft.unit, limit };
}

/** A key that changes whenever anything that changes the results changes, to tell results from an earlier draft. */
export function draftKey(draft: Draft): string {
  return JSON.stringify(toQuerySpec(draft));
}

// ---------------------------------------------------------------- the draft in plain words

export interface Names {
  code: (id: number) => string;
  source: (id: number) => string;
  case: (id: number) => string;
  coder: (id: number) => string;
}

export function joinOr(items: string[]): string {
  if (items.length <= 1) return items.join('');
  if (items.length === 2) return t('{value} or {value2}', { value: items[0], value2: items[1] });
  return t('{list}, or {value}', { list: items.slice(0, -1).join(', '), value: items[items.length - 1] });
}

export interface ReadLine {
  depth: number;
  text: string;
}

function clauseWords(c: Clause, n: Names): string {
  const not = c.negate;
  // Each sentence is written whole, with and without "not", so that another language can order its words as it needs.
  const none = () => t('(none chosen)');
  const noVariable = () => t('(no variable)');
  switch (c.type) {
    case 'code': {
      const list = joinOr(c.codeIds.map(n.code)) || none();
      return not ? t('the code is not {list}', { list }) : t('the code is {list}', { list });
    }
    case 'also': {
      const codes = joinOr(c.codeIds.map(n.code)) || none();
      if (c.within > 0) return not ? t('no coding of {codes} lies within {within} characters of the words', { codes, within: num(c.within) }) : t('a coding of {codes} lies within {within} characters of the words', { codes, within: num(c.within) });
      return not ? t('the same words are not also coded {codes}', { codes }) : t('the same words are also coded {codes}', { codes });
    }
    case 'text': {
      const contains = c.contains.trim() || t('(nothing yet)');
      return not ? t('the words do not contain “{contains}”', { contains }) : t('the words contain “{contains}”', { contains });
    }
    case 'source': {
      const list = joinOr(c.ids.map(n.source)) || none();
      return not ? t('the document is not {list}', { list }) : t('the document is {list}', { list });
    }
    case 'case': {
      const list = joinOr(c.ids.map(n.case)) || none();
      return not ? t('the case is not {list}', { list }) : t('the case is {list}', { list });
    }
    case 'variable': {
      const name = c.name || noVariable();
      const value = c.value || t('(no value)');
      return not ? t('the case does not have {name} = {value}', { name, value }) : t('the case has {name} = {value}', { name, value });
    }
    case 'coder': {
      const list = joinOr(c.ids.map(n.coder)) || none();
      return not ? t('the coder is not {list}', { list }) : t('the coder is {list}', { list });
    }
    case 'rating': {
      const lo = c.min.trim();
      const hi = c.max.trim();
      const range = lo !== '' && hi !== '' ? t('from {from} to {to}', { from: lo, to: hi }) : lo !== '' ? t('{lo} or more', { lo: lo }) : hi !== '' ? t('{hi} or less', { hi: hi }) : t('(not set)');
      return not ? t('the rating is not {range}', { range }) : t('the rating is {range}', { range });
    }
    case 'coded': {
      const a = c.from.trim() !== '' ? normalizeDay(c.from) : '';
      const b = c.to.trim() !== '' ? normalizeDay(c.to) : '';
      const range = a !== '' && b !== '' ? t('from {from} to {to}', { from: a, to: b }) : a !== '' ? t('on or after {day}', { day: a }) : b !== '' ? t('on or before {day}', { day: b }) : t('(no day set)');
      return not ? t('the coding was not made {range}', { range }) : t('the coding was made {range}', { range });
    }
    case 'between': {
      const dates = [c.from, c.to].filter((v) => v.trim() !== '').every((v) => limitKind(v) === 'date');
      const a = c.from.trim() !== '' ? (dates ? normalizeDay(c.from) : c.from.trim()) : '';
      const b = c.to.trim() !== '' ? (dates ? normalizeDay(c.to) : c.to.trim()) : '';
      const range = a !== '' && b !== '' ? t('from {from} to {to}', { from: a, to: b }) : a !== '' ? (dates ? t('on or after {day}', { day: a }) : t('{a} or more', { a: a })) : b !== '' ? (dates ? t('on or before {day}', { day: b }) : t('{b} or less', { b: b })) : t('(no limit set)');
      const name = c.name || noVariable();
      return not ? t("the case's {name} is not {range}", { name, range }) : t("the case's {name} is {range}", { name, range });
    }
  }
}

/** Reads the draft back as a sentence of what will be returned and one line for each condition, before anything is run. */
export function readback(draft: Draft, n: Names, universe?: { codings: number; documents: number }): { summary: string; lines: ReadLine[]; rules: string } {
  const what = draft.unit === 'excerpts' ? t('distinct excerpts (a stretch of text is returned once, with every coding of it that matched)') : t('assignments (each coding that matches)');
  const lines: ReadLine[] = [];
  const walk = (g: Group, depth: number) => {
    for (const item of g.items) {
      if (item.kind === 'group') {
        lines.push({ depth, text: item.combine === 'any' ? t('any one of these is true:') : t('all of these are true:') });
        walk(item, depth + 1);
      } else {
        lines.push({ depth, text: clauseWords(item, n) });
      }
    }
  };
  walk(draft.root, 0);
  const top = draft.root.combine === 'any' ? t('at least one of these is true') : t('all of these are true');
  const rules = [draft.descendants ? t('A code also stands for its sub-codes.') : t('A code stands only for itself.'), draft.sameCoder ? t('A condition that is "also coded" looks at the same coder\'s other codings only.') : t('A condition that is "also coded" looks at every coder\'s codings.')].join(' ');
  return { summary: universe ? t('Look through {codings} in {documents}. Return {what} where {top}:', { codings: tn(universe.codings, { one: '{count} coding', other: '{count} codings' }), documents: tn(universe.documents, { one: '{count} document', other: '{count} documents' }), what, top }) : t('Look through every coding in the project. Return {what} where {top}:', { what, top }), lines, rules };
}

// ---------------------------------------------------------------- the numbers behind a chart

/** The length of each bar, as a share of the longest, so the bars use the room they have. */
export function barShares(values: readonly number[]): number[] {
  const max = Math.max(0, ...values);
  return values.map((v) => (max > 0 ? v / max : 0));
}

/** A round number at or above the largest value, for the end of an axis: 7 gives 8, 83 gives 100. */
export function niceMax(max: number): number {
  if (max <= 0) return 1;
  const pow = 10 ** Math.floor(Math.log10(max));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * pow >= max) return m * pow;
  return 10 * pow;
}

/** How to shade a cell by its share of the largest cell, from 0 (empty) to 1. Zero is never shaded, so empty is never mistaken for low. */
export function intensity(value: number, max: number): number {
  if (value <= 0 || max <= 0) return 0;
  return Math.max(0.12, Math.min(1, value / max));
}

// ---------------------------------------------------------------- percentages

/** What a percentage in a table is a percentage of. */
export type PercentMode = 'none' | 'row' | 'column' | 'all' | 'group';

export type RowKind = 'cases' | 'documents' | 'variable' | 'months';

/** The ways to show a matrix's numbers that make sense for this metric and these rows. */
export function percentModes(metric: Metric, rows: RowKind): { id: PercentMode; label: string }[] {
  const noun = metricWord(metric);
  const modes: { id: PercentMode; label: string }[] = [
    { id: 'none', label: t('Numbers only') },
    { id: 'row', label: t("Percent of the row's {noun}", { noun }) },
    { id: 'column', label: t("Percent of the column's {noun}", { noun }) },
    { id: 'all', label: t('Percent of all the {noun} shown', { noun }) },
  ];
  // The number of cases in a group is a stated denominator only when the rows are groups of cases and cases are what is counted.
  if (rows === 'variable' && metric === 'cases') modes.push({ id: 'group', label: t('Percent of the cases in each group') });
  return modes;
}

/** The whole a cell's percentage is taken of, or null when there is none (a row with nothing in it). */
export function percentWhole(mode: PercentMode, metric: Metric, row: MatrixResult['rows'][number], columnIndex: number, matrix: MatrixResult): number | null {
  let whole: number | null;
  switch (mode) {
    case 'row':
      whole = pick(row.total, metric);
      break;
    case 'column':
      whole = pick(matrix.columns[columnIndex].total, metric);
      break;
    case 'all':
      whole = pick(matrix.total, metric);
      break;
    case 'group':
      whole = row.size;
      break;
    default:
      return null;
  }
  return whole !== null && whole > 0 ? whole : null;
}

/** A part as a percentage of a whole, to one decimal place, or a dash when there is no whole to take it of. */
export function percentText(part: number, whole: number | null): string {
  if (whole === null || whole <= 0) return '–';
  const p = (part / whole) * 100;
  return `${p.toFixed(1).replace(/\.0$/, '')}%`;
}

/** The denominator in a sentence, to show with the table. */
export function percentSentence(mode: PercentMode, metric: Metric, rowsName: string): string {
  const noun = metricWord(metric);
  switch (mode) {
    case 'row':
      return t("Each percentage is a part of its row's total: the number of {noun} in that row. A row with nothing in it has no percentage (–). The percentages of one row can add up to more than 100 when columns share the same evidence.", { noun });
    case 'column':
      return t("Each percentage is a part of its column's total: the number of {noun} for that code, over all the rows. A column with nothing in it has no percentage (–). The percentages of one column can add up to more than 100 when rows share the same evidence.", { noun });
    case 'all':
      return t('Each percentage is a part of the distinct total of everything shown ({noun}), counted once. Cells can add up to more than 100 when they share the same evidence.', { noun });
    case 'group':
      return t('Each percentage is a part of N: the number of cases that have that value of {rowsName}, with or without any coding. A group of no cases has no percentage (–).', { rowsName });
    default:
      return '';
  }
}

/** Months between two `YYYY-MM` labels, to say how far a month-by-month table reaches. */
export function monthSpan(labels: readonly string[]): string {
  const months = labels.filter((l) => /^\d{4}-\d{2}$/.test(l));
  if (months.length === 0) return '';
  return months.length === 1 ? months[0] : t('{value} to {value2}', { value: months[0], value2: months[months.length - 1] });
}

// ---------------------------------------------------------------- crosstabs

/** What a percentage in a crosstab is a percentage of. */
export type CrosstabPercent = 'none' | 'row' | 'column' | 'all';

export const CROSSTAB_PERCENTS: { id: CrosstabPercent; label: string }[] = [
  { id: 'none', get label() { return t('Numbers only'); } },
  { id: 'row', get label() { return t("Percent of the row's cases"); } },
  { id: 'column', get label() { return t("Percent of the column's cases"); } },
  { id: 'all', get label() { return t('Percent of all the cases counted'); } },
];

/** The whole a crosstab cell is a part of, or null when there is none (an empty row or column). */
export function crosstabWhole(mode: CrosstabPercent, rowTotal: number, columnTotal: number, grandTotal: number): number | null {
  const whole = mode === 'row' ? rowTotal : mode === 'column' ? columnTotal : mode === 'all' ? grandTotal : 0;
  return whole > 0 ? whole : null;
}

export function crosstabPercentSentence(mode: CrosstabPercent, rowVariable: string, columnVariable: string): string {
  switch (mode) {
    case 'row':
      return t('Each percentage is a part of the cases that have that value of {rowVariable}: across a row the percentages add up to 100, counting the cases with no value of {columnVariable} as their own column.', { rowVariable, columnVariable });
    case 'column':
      return t('Each percentage is a part of the cases that have that value of {columnVariable}: down a column the percentages add up to 100, counting the cases with no value of {rowVariable} as their own row.', { columnVariable, rowVariable });
    case 'all':
      return t('Each percentage is a part of every case the table counts, including those with no value for either variable, so all the cells together add up to 100.');
    default:
      return '';
  }
}

/** The ranges a width makes, for the words beside the box ("0 to under 10, 10 to under 20, ..."). A width that is not a positive number is none. */
export function rangeWidth(text: string): number | undefined {
  const n = limitKind(text) === 'number' ? limitNumber(text) : NaN;
  return Number.isFinite(n) && n > 0 ? n : undefined;
}
