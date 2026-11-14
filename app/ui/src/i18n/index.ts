// The interface's words, found by language.
//
//   t('Save')                                      a message; its English text is its name
//   t('Delete {name}?', { name })                  with blanks to fill
//   tc('menu', 'Open')                             when the same English words mean different things, a note tells them apart
//   tn(count, { one: '{count} code', other: '{count} codes' })   a count; {count} is filled in, with the language's digits grouping
//   N_('Home')                                     marks a message in a table so it is collected, and returns it unchanged;
//                                                  where the table is used, t(thatValue) looks it up
//   rich('Press <kbd>Enter</kbd> to code {name}', { kbd: (inside) => <kbd>{inside}</kbd>, name })
//
// With no translation for a message, the English text shows, so a language can be added a message at a time.
// docs/LOCALIZATION.md says how a language is added and checked.

import { useSyncExternalStore, type ReactNode } from 'react';
import {
  EMPTY_CATALOG,
  formatTag,
  interpolate,
  lookup,
  pickPlural,
  pseudo,
  type Args,
  type Catalog,
  type PluralForms,
} from './core';
import { richNodes, type RichArgs } from './rich';

export type { Args, Catalog, PluralForms } from './core';
export type { RichArgs } from './rich';

export interface LocaleState {
  /** What the language is called in the list of languages: the code, or the name of an added translation (fr~2). */
  id: string;
  code: string;
  dir: 'ltr' | 'rtl';
  catalog: Catalog;
  /** Formatting (numbers, dates, lists) is asked for in this tag. */
  tag: string;
  rules: Intl.PluralRules;
  pseudo: boolean;
}

function makeState(code: string, catalog: Catalog, dir: 'ltr' | 'rtl', id: string = code): LocaleState {
  const tag = formatTag(code);
  let rules: Intl.PluralRules;
  try {
    rules = new Intl.PluralRules(tag);
  } catch {
    rules = new Intl.PluralRules('en');
  }
  return { id, code, dir, catalog, tag, rules, pseudo: code === 'en-XA' };
}

let state: LocaleState = makeState('en', EMPTY_CATALOG, 'ltr');
const listeners = new Set<() => void>();

export function getLocaleState(): LocaleState {
  return state;
}

/** The code of the language in use ('en', 'fr', ...). */
export function currentLocale(): string {
  return state.code;
}

/** Puts a language in use. Everything that shows words asks again the next time it draws (see useLocale). */
export function setLocaleData(code: string, catalog: Catalog | null, dir: 'ltr' | 'rtl' = 'ltr', id: string = code): void {
  state = makeState(code, catalog ?? EMPTY_CATALOG, dir, id);
  for (const l of listeners) l();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** For a screen to draw again when the language changes. Returns the id of the language in use. */
export function useLocale(): string {
  return useSyncExternalStore(subscribe, () => state.id, () => 'en');
}

function pick(text: string, context?: string): string {
  if (state.pseudo) return pseudo(text);
  const entry = lookup(state.catalog, text, context);
  return typeof entry === 'string' ? entry : text;
}

/** A message. */
export function t(text: string, args?: Args): string {
  return interpolate(pick(text), args);
}

/** A message that needs a note to tell it from other messages with the same English words. */
export function tc(context: string, text: string, args?: Args): string {
  return interpolate(pick(text, context), args);
}

/** A message with a count: the form is chosen by the language's rules, and {count} is filled in with the count. */
export function tn(count: number, forms: PluralForms, args?: Args): string {
  const key = forms.other;
  const translated = state.pseudo ? undefined : lookup(state.catalog, key);
  let template = pickPlural(count, forms, translated, state.rules);
  if (state.pseudo) template = pseudo(template);
  return interpolate(template, { count: num(count), ...args });
}

/** Marks a message that sits in a table (a list of labels) so that it is collected; the value is the same text. Show it with t(value). */
export function N_<T extends string>(text: T): T {
  return text;
}

/** A message with marks (<b>...</b>) and blanks that may be elements, as a row of nodes. */
export function rich(text: string, args: RichArgs = {}): ReactNode {
  const template = pick(text);
  return richNodes(template, args);
}

// ---------------------------------------------------------------- numbers, lists, dates

const numberFormats = new Map<string, Intl.NumberFormat>();

function numberFormat(tag: string, options?: Intl.NumberFormatOptions): Intl.NumberFormat {
  const key = tag + (options ? JSON.stringify(options) : '');
  let f = numberFormats.get(key);
  if (!f) {
    f = new Intl.NumberFormat(tag, options);
    numberFormats.set(key, f);
  }
  return f;
}

/** A number as the language writes it (US English: 1,234.5). */
export function num(n: number, options?: Intl.NumberFormatOptions): string {
  return numberFormat(state.tag, options).format(n);
}

const collators = new Map<string, Intl.Collator>();

/** Compares two texts in the order of the language in use (so "ä" and "z" fall where its speakers expect). Returns a number for sort(). */
export function compareText(a: string, b: string, options?: Intl.CollatorOptions): number {
  const key = state.tag + (options ? JSON.stringify(options) : '');
  let c = collators.get(key);
  if (!c) {
    c = new Intl.Collator(state.tag, options);
    collators.set(key, c);
  }
  return c.compare(a, b);
}

/** A list of words joined as the language does ("a, b, and c"). */
export function list(items: readonly string[], type: 'conjunction' | 'disjunction' = 'conjunction'): string {
  try {
    return new Intl.ListFormat(state.tag, { style: 'long', type }).format(items);
  } catch {
    return items.join(', ');
  }
}

/** A date and time as the language writes them. */
export function dateTime(d: Date | number, options: Intl.DateTimeFormatOptions): string {
  return new Intl.DateTimeFormat(state.tag, options).format(d);
}

/** Puts the language's code and direction on the page, so a screen reader speaks it right and a right-to-left language lays out right. */
export function applyDocumentLocale(root: { lang: string; dir: string }): void {
  root.lang = state.code === 'en-XA' ? 'en' : state.code;
  root.dir = state.dir;
}
