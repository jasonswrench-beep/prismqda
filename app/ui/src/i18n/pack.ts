// Translation files: reading one (a spreadsheet saved as CSV, or a JSON file), checking it against the words of the window, and
// writing the files a translator starts from. PrismQDA does not translate; this is how a translation made elsewhere comes in.
//
// A translation is checked message by message. A message whose blanks or marks were changed is left out, with the reason, and the
// English shows in its place, so a file with a few mistakes is still usable and the mistakes are listed for the translator.

import { guardCell, parseCsv, toCsv, unguardCell } from './csv';
import { infoOf } from './tokens';
import { pluralCategoriesOf, standardCodeFor } from './languages';
import type { Catalog, Entry } from './core';

/** One message of the program, as written in messages.json. */
export interface SourceMessage {
  id: string;
  context?: string;
  /** The English forms of a message with a count. */
  forms?: Record<string, string>;
  blanks?: string[];
  marks?: string[];
  files: string[];
  engine?: boolean;
}

export type MadeBy = 'person' | 'reviewed' | 'machine';

/** What is known about a translation that has been added. */
export interface PackInfo {
  /** What it is called inside PrismQDA: the language code, and ~2, ~3 for a second translation of the same language. */
  id: string;
  /** The language: one of the standard list, such as fr or pt-BR. */
  code: string;
  /** What the person called it, shown in the list of languages. */
  label: string;
  /** Who made the translation, as the person wrote it. */
  credit: string;
  madeBy: MadeBy;
  /** The day it was added, as year-month-day. */
  added: string;
  dir: 'ltr' | 'rtl';
  /** How many messages it translates, of how many the program had when it was added. */
  done: number;
  total: number;
}

export interface Problem {
  message: string;
  what: string;
}

/** What was found in a file before anything is added. */
export interface Draft {
  format: 'csv' | 'json';
  /** The language the file says it is for (a JSON file may say), as written. */
  code: string | null;
  label: string | null;
  credit: string | null;
  madeBy: MadeBy | null;
  messages: Record<string, unknown>;
  contexts: Record<string, Record<string, unknown>>;
  /** Rows or entries that could not be understood at all. */
  unreadable: number;
}

export class PackError extends Error {}

const CATEGORIES = new Set(['zero', 'one', 'two', 'few', 'many', 'other']);
const MAX_MESSAGES = 20_000;
const MAX_LENGTH = 4_000;

// ---------------------------------------------------------------- reading a file

function isPlain(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === 'object' && !Array.isArray(v);
}

export function parsePackJson(text: string): Draft {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    throw new PackError('This is not a JSON file PrismQDA can read. A translation file is a JSON object with a "messages" part.');
  }
  if (!isPlain(data) || !isPlain(data.messages)) throw new PackError('This JSON file has no "messages" part, so it is not a translation file for PrismQDA.');
  const contexts: Record<string, Record<string, unknown>> = {};
  if (isPlain(data.contexts)) for (const [k, v] of Object.entries(data.contexts)) if (isPlain(v)) contexts[k] = v;
  const madeBy = data.madeBy === 'person' || data.madeBy === 'reviewed' || data.madeBy === 'machine' ? data.madeBy : null;
  const str = (v: unknown) => (typeof v === 'string' && v.trim() ? v.trim() : null);
  return { format: 'json', code: str(data.locale), label: str(data.label) ?? str(data.name), credit: str(data.credit), madeBy, messages: data.messages, contexts, unreadable: 0 };
}

const header = (row: string[], name: string) => row.findIndex((c) => c.trim().toLowerCase() === name);

export function parsePackCsv(text: string): Draft {
  const { rows } = parseCsv(text);
  const headAt = rows.findIndex((r) => header(r, 'key') >= 0 && header(r, 'translation') >= 0);
  if (headAt < 0) throw new PackError('This spreadsheet has no columns called Key and Translation, so it is not a translation file for PrismQDA. Save a blank translation file from Settings and fill in its Translation column.');
  const head = rows[headAt] as string[];
  const col = { key: header(head, 'key'), context: header(head, 'context'), form: header(head, 'form'), english: header(head, 'english'), translation: header(head, 'translation') };
  const messages: Record<string, unknown> = {};
  const contexts: Record<string, Record<string, unknown>> = {};
  let unreadable = 0;
  const forms = new Map<string, Record<string, string>>();
  const place = (context: string, key: string, value: unknown) => {
    if (context === '') messages[key] = value;
    else (contexts[context] ??= {})[key] = value;
  };
  for (const row of rows.slice(headAt + 1)) {
    const key = unguardCell(row[col.key] ?? '');
    if (key.trim() === '') {
      if (row.some((c) => c.trim() !== '')) unreadable++;
      continue;
    }
    const context = col.context >= 0 ? (row[col.context] ?? '').trim() : '';
    const form = col.form >= 0 ? (row[col.form] ?? '').trim().toLowerCase() : '';
    const english = col.english >= 0 ? unguardCell(row[col.english] ?? '') : '';
    let translation = unguardCell(row[col.translation] ?? '');
    // A spreadsheet adds spaces at the ends; keep an end the English has, drop one it does not.
    if (english !== '') {
      if (!/^\s/.test(english)) translation = translation.replace(/^\s+/, '');
      if (!/\s$/.test(english)) translation = translation.replace(/\s+$/, '');
    } else translation = translation.trim();
    if (form === '') {
      place(context, key, translation);
    } else {
      const id = `${context}\u0004${key}`;
      const got = forms.get(id) ?? {};
      got[form] = translation;
      forms.set(id, got);
      place(context, key, got);
    }
  }
  return { format: 'csv', code: null, label: null, credit: null, madeBy: null, messages, contexts, unreadable };
}

/** Reads a translation file by what it looks like: JSON starts with a brace. */
export function parsePack(text: string, fileName = ''): Draft {
  const trimmed = text.replace(/^﻿/, '').trimStart();
  if (trimmed.startsWith('{') || /\.json$/i.test(fileName)) return parsePackJson(trimmed);
  return parsePackCsv(text);
}

/** The language a file name suggests ("fr.csv", "pt-BR.json", "translation_es.csv"), if it is one of the standard list. */
export function codeFromFileName(name: string): string | null {
  const stem = name.replace(/\.[^.]*$/, '');
  for (const token of [stem, ...stem.split(/[\s_]+/)]) {
    const found = standardCodeFor(token);
    if (found) return found;
  }
  return null;
}

// ---------------------------------------------------------------- checking against the program

export interface Checked {
  catalog: Catalog;
  done: number;
  total: number;
  problems: Problem[];
  /** Entries for messages the program does not have (a message that was reworded, or from another version). */
  unknown: number;
}

const short = (s: string) => (s.length > 60 ? s.slice(0, 57) + '...' : s);

function sameSet(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((x) => b.includes(x));
}

/** Checks one translated text against a message; returns the reason it cannot be used, or null. */
function why(text: string, src: SourceMessage, form: string | null): string | null {
  if (text.length > MAX_LENGTH) return 'is much longer than any message of the program';
  const info = infoOf(text);
  if (info.stray) return 'has a single brace; a { or } that is not a blank is written {{ or }}';
  const allowed = [...(src.blanks ?? []), ...(src.forms ? ['count'] : [])];
  for (const p of info.placeholders) if (!allowed.includes(p)) return `uses {${p}}, which the English does not have`;
  if (!src.forms) for (const p of src.blanks ?? []) if (!info.placeholders.includes(p)) return `is missing {${p}}, which the English has`;
  if (!sameSet(info.tags, src.marks ?? [])) {
    const want = (src.marks ?? []).map((m) => `<${m}>`).join(' ') || 'no marks';
    return `has different marks from the English (the English has ${want})${form ? ` in the form ${form}` : ''}`;
  }
  return null;
}

/** The entries of a draft that can be used, as a catalog, and a report of the rest. */
export function checkDraft(draft: Draft, source: readonly SourceMessage[]): Checked {
  const byKey = new Map<string, SourceMessage>();
  for (const m of source) byKey.set(`${m.context ?? ''}\u0004${m.id}`, m);
  const messages: Record<string, Entry> = {};
  const contexts: Record<string, Record<string, Entry>> = {};
  const problems: Problem[] = [];
  let done = 0;
  let unknown = 0;
  let seen = 0;

  const visit = (context: string, id: string, value: unknown) => {
    if (++seen > MAX_MESSAGES) throw new PackError('This file has far more entries than PrismQDA has messages, so it is not a translation file for PrismQDA.');
    const src = byKey.get(`${context}\u0004${id}`);
    if (!src) {
      unknown++;
      return;
    }
    let accepted: Entry | null = null;
    if (typeof value === 'string') {
      if (value === '') return;
      const reason = why(value, src, null);
      if (reason) problems.push({ message: short(id), what: reason });
      else accepted = value;
    } else if (isPlain(value)) {
      const forms: Record<string, string> = {};
      let bad: string | null = null;
      let any = false;
      for (const [k, v] of Object.entries(value)) {
        if (typeof v !== 'string') {
          bad = `has a form (${k}) that is not text`;
          break;
        }
        if (v === '') continue;
        if (!CATEGORIES.has(k)) {
          bad = `has a form called ${k}; the forms are zero, one, two, few, many, other`;
          break;
        }
        const reason = why(v, src, k);
        if (reason) {
          bad = reason;
          break;
        }
        forms[k] = v;
        any = true;
      }
      if (bad) problems.push({ message: short(id), what: bad });
      else if (any) {
        // a message with a count needs the form "other" to fall back on
        if (src.forms && !forms.other) problems.push({ message: short(id), what: 'needs the form called other (the one for all the numbers no other form covers)' });
        else accepted = src.forms ? (forms as Entry) : (forms.other as string);
      }
    } else {
      problems.push({ message: short(id), what: 'must be text' });
    }
    if (accepted === null) return;
    if (context === '') messages[id] = accepted;
    else (contexts[context] ??= {})[id] = accepted;
    done++;
  };

  for (const [id, value] of Object.entries(draft.messages)) visit('', id, value);
  for (const [context, table] of Object.entries(draft.contexts)) for (const [id, value] of Object.entries(table)) visit(context, id, value);
  const catalog: Catalog = { messages };
  if (Object.keys(contexts).length > 0) catalog.contexts = contexts;
  return { catalog, done, total: source.length, problems, unknown };
}

// ---------------------------------------------------------------- the files a translator starts from

/** The screens a person meets first come first, so a translation can be useful before it is finished. */
const FIRST = [
  'components/StartScreen.tsx', 'components/NewProjectForm.tsx', 'components/ProjectDialogs.tsx', 'components/GetStarted.tsx', 'components/Splash.tsx',
  'components/TitleBar.tsx', 'components/Frame.tsx', 'components/ImportPage.tsx', 'components/Ribbon.tsx', 'state/commands.ts', 'components/SettingsView.tsx',
  'components/Reader.tsx', 'components/DocumentsPane.tsx', 'components/CodebookPane.tsx', 'components/CodeDialogs.tsx', 'components/DetailPanel.tsx',
  'components/RetrievedPane.tsx', 'components/Announcer.tsx', 'state/actions.ts', 'lib/shortcuts.ts', 'components/HelpDialog.tsx', 'components/Dashboard.tsx',
];

function rank(m: SourceMessage): number {
  let best = FIRST.length;
  for (const f of m.files) {
    const at = FIRST.indexOf(f);
    if (at >= 0 && at < best) best = at;
  }
  return best;
}

export function sortedForTranslator(source: readonly SourceMessage[]): SourceMessage[] {
  return [...source].sort((a, b) => rank(a) - rank(b) || a.id.localeCompare(b.id, 'en'));
}

export const CSV_HEAD = ['Key', 'Context', 'Form', 'English', 'Translation', 'Keep these as they are'];

function keepNote(m: SourceMessage): string {
  const bits = [...(m.blanks ?? []).map((b) => `{${b}}`), ...(m.forms ? ['{count}'] : []), ...(m.marks ?? []).map((t) => `<${t}>...</${t}>`)];
  return bits.length ? bits.join(' ') : '';
}

function translatedAlready(m: SourceMessage, catalog: Catalog | undefined): Entry | undefined {
  if (!catalog) return undefined;
  return m.context ? catalog.contexts?.[m.context]?.[m.id] : catalog.messages[m.id];
}

/**
 * The rows of a spreadsheet for translating into a language. With `existing`, what is translated already is filled in; with
 * `onlyMissing`, only the messages that still need a translation are listed.
 */
export function translatorRows(source: readonly SourceMessage[], code: string, existing?: Catalog, onlyMissing = false): string[][] {
  const categories = pluralCategoriesOf(code);
  const rows: string[][] = [];
  for (const m of sortedForTranslator(source)) {
    const have = translatedAlready(m, existing);
    if (m.forms) {
      const haveForms: Record<string, string> = typeof have === 'string' ? { other: have } : (have ?? {});
      const complete = categories.every((c) => (haveForms[c] ?? '') !== '') || (typeof have === 'string' && have !== '');
      if (onlyMissing && complete) continue;
      for (const c of categories) rows.push([guardCell(m.id), m.context ?? '', c, guardCell(m.forms[c] ?? m.forms.other ?? m.id), guardCell(haveForms[c] ?? ''), keepNote(m)]);
    } else {
      if (onlyMissing && typeof have === 'string' && have !== '') continue;
      rows.push([guardCell(m.id), m.context ?? '', '', guardCell(m.id), guardCell(typeof have === 'string' ? have : ''), keepNote(m)]);
    }
  }
  return rows;
}

/** The text of the spreadsheet: a first line that tells Excel the delimiter, then the head and the rows, as UTF-8 with a mark. */
export function translatorCsv(source: readonly SourceMessage[], code: string, existing?: Catalog, onlyMissing = false): { text: string; rows: number } {
  const rows = translatorRows(source, code, existing, onlyMissing);
  return { text: '﻿sep=,\r\n' + toCsv([CSV_HEAD, ...rows]), rows: rows.length };
}

/** The JSON a translator or a script can fill in: the same shape PrismQDA reads. */
export function translatorJson(source: readonly SourceMessage[], code: string, info?: Pick<PackInfo, 'label' | 'credit' | 'madeBy'>, existing?: Catalog, onlyMissing = false): string {
  const categories = pluralCategoriesOf(code);
  const messages: Record<string, unknown> = {};
  const contexts: Record<string, Record<string, unknown>> = {};
  for (const m of sortedForTranslator(source)) {
    const have = translatedAlready(m, existing);
    const complete = m.forms ? (typeof have === 'string' ? have !== '' : categories.every((c) => ((have as Record<string, string> | undefined)?.[c] ?? '') !== '')) : typeof have === 'string' && have !== '';
    if (onlyMissing && complete) continue;
    const value = m.forms ? Object.fromEntries(categories.map((c) => [c, typeof have === 'object' && have ? (have[c] ?? '') : c === 'other' && typeof have === 'string' ? have : ''])) : typeof have === 'string' ? have : '';
    if (m.context) (contexts[m.context] ??= {})[m.id] = value;
    else messages[m.id] = value;
  }
  const out: Record<string, unknown> = { locale: code, name: '', label: info?.label ?? '', credit: info?.credit ?? '', madeBy: info?.madeBy ?? 'person' };
  out.messages = messages;
  if (Object.keys(contexts).length) out.contexts = contexts;
  return JSON.stringify(out, null, 2) + '\n';
}

/** A translation already added, as a JSON file to keep or to give to someone. */
export function packToJson(info: PackInfo, catalog: Catalog): string {
  const sorted = (o: Record<string, Entry>) => Object.fromEntries(Object.entries(o).sort(([a], [b]) => a.localeCompare(b, 'en')));
  const out: Record<string, unknown> = { locale: info.code, label: info.label, credit: info.credit, madeBy: info.madeBy, added: info.added, dir: info.dir, messages: sorted(catalog.messages) };
  if (catalog.contexts) out.contexts = Object.fromEntries(Object.entries(catalog.contexts).map(([k, v]) => [k, sorted(v)]));
  return JSON.stringify(out, null, 2) + '\n';
}
