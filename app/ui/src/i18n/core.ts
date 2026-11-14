// The part of the language catalog that has no page and no window in it: how a message is found, how its
// blanks are filled, how a plural is chosen, and how the test language (pseudo-locale) bends a message.
//
// A message is its English text. The English text is also what shows when a language has no translation of it,
// so a missing translation never leaves a blank or a key on the screen. In a message, {name} is a blank to fill,
// <b>...</b> marks a stretch that the program styles (see rich in index.ts), and {{ and }} are a plain { and }.

export type Arg = string | number;
export type Args = Record<string, Arg>;

/** The forms of a message that has a count. Which of them a language needs follows its plural rules (CLDR). */
export type PluralForms = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string };

export type Entry = string | PluralForms;

/** What a translator provides for one language. Keys are the English texts. */
export interface Catalog {
  messages: Record<string, Entry>;
  /** Messages that need a note to tell two uses of the same English words apart, by that note. */
  contexts?: Record<string, Record<string, Entry>>;
}

export const EMPTY_CATALOG: Catalog = { messages: {} };

/** A message, a blank, or a mark: the pieces a message is cut into. */
const TOKEN = /\{\{|\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}|<(\/?)([a-z][a-z0-9]*)(\s*\/)?>/g;

export function interpolate(template: string, args?: Args): string {
  if (!template.includes('{')) return template;
  return template.replace(/\{\{|\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}/g, (whole, name?: string) => {
    if (whole === '{{') return '{';
    if (whole === '}}') return '}';
    if (name && args && Object.prototype.hasOwnProperty.call(args, name)) return String(args[name]);
    return whole; // a blank nobody filled stays visible, so it is found
  });
}

/** Names of the blanks in a message, in order of first use. */
export function placeholdersOf(template: string): string[] {
  const names: string[] = [];
  for (const m of template.matchAll(TOKEN)) {
    if (m[1] && !names.includes(m[1])) names.push(m[1]);
  }
  return names;
}

/** Names of the marks (<b>...</b>) in a message, in order of first use. */
export function tagsOf(template: string): string[] {
  const names: string[] = [];
  for (const m of template.matchAll(TOKEN)) {
    if (m[3] && !names.includes(m[3])) names.push(m[3]);
  }
  return names;
}

/** Looks a message up. A context is a note that tells two uses of the same English words apart. */
export function lookup(catalog: Catalog, text: string, context?: string): Entry | undefined {
  const own = (o: object | undefined, key: string) => (o !== undefined && Object.prototype.hasOwnProperty.call(o, key) ? (o as Record<string, Entry>)[key] : undefined);
  const entry = context === undefined ? own(catalog.messages, text) : own(own(catalog.contexts, context) as object | undefined, text);
  if (entry === undefined) return undefined;
  if (typeof entry === 'string') return entry === '' ? undefined : entry;
  return entry;
}

const ENGLISH_RULES = new Intl.PluralRules('en');

/** Chooses the text of a count message: from the translation if there is one (by the language's rules), else the English forms. */
export function pickPlural(count: number, forms: PluralForms, translated: Entry | undefined, rules: Intl.PluralRules): string {
  if (translated && typeof translated === 'object') {
    const category = rules.select(count);
    const form = translated[category] ?? translated.other;
    if (form) return form;
  }
  if (typeof translated === 'string') return translated;
  return forms[ENGLISH_RULES.select(count)] ?? forms.other;
}

// ---------------------------------------------------------------- the test language

const PSEUDO_FROM = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ';
const PSEUDO_TO = 'àƀçðéƒĝĥîĵķľɱñôþǫŕšťûṽŵẋýžÀßÇÐÉƑĜĤÎĴĶĽṀÑÔÞǪŔŠŤÛṼŴẊÝŽ';

/**
 * Bends an English message into the test language: every letter becomes an accented one, the whole is wrapped in
 * ⟦ ⟧, and it is padded by about a third, as many languages are longer than English. The blanks and marks are kept.
 * A screen in the test language that still shows plain English letters has a string that is not in the catalog.
 */
export function pseudo(template: string): string {
  let out = '';
  let last = 0;
  let letters = 0;
  for (const m of template.matchAll(TOKEN)) {
    const at = m.index ?? 0;
    out += bend(template.slice(last, at));
    letters += countLetters(template.slice(last, at));
    out += m[0];
    last = at + m[0].length;
  }
  out += bend(template.slice(last));
  letters += countLetters(template.slice(last));
  const pad = letters >= 4 ? ' ' + '~'.repeat(Math.ceil(letters / 3)) : '';
  return `⟦${out}${pad}⟧`;
}

function countLetters(s: string): number {
  let n = 0;
  for (const ch of s) if (/\p{L}/u.test(ch)) n++;
  return n;
}

function bend(s: string): string {
  let out = '';
  for (const ch of s) {
    const i = PSEUDO_FROM.indexOf(ch);
    out += i >= 0 ? PSEUDO_TO[i] : ch;
  }
  return out;
}

// ---------------------------------------------------------------- choosing a language

/**
 * The language to use: the person's choice if it is one that exists, else the first of the computer's languages
 * that has a translation (an exact match, or the same language), else English.
 */
export function resolveLocale(choice: string, available: readonly string[], system: readonly string[]): string {
  const has = (code: string) => available.includes(code);
  if (choice && choice !== 'auto') return has(choice) ? choice : 'en';
  for (const raw of system) {
    const tag = raw.replace('_', '-');
    if (has(tag)) return tag;
    const base = tag.split('-')[0] ?? '';
    const exact = available.find((a) => a.toLowerCase() === tag.toLowerCase());
    if (exact) return exact;
    const sameLanguage = available.find((a) => a.split('-')[0]?.toLowerCase() === base.toLowerCase() && a !== 'en-XA');
    if (sameLanguage) return sameLanguage;
  }
  return 'en';
}

/** The tag the browser's formatting (numbers, dates, lists) is asked for. The English of this program is US English. */
export function formatTag(code: string): string {
  if (code === 'en' || code === 'en-XA') return 'en-US';
  return code;
}
