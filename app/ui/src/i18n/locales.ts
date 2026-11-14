// The languages the window can be shown in, and how one is put in use. There are two kinds:
//   * those that come with the program: a file in ./locales (named by its code, for example fr.json) and a line in manifest.json;
//   * translations the person added on this computer (see installed.ts), each with a label the person gave it.
// English needs no file: it is the text in the code.

import manifest from './manifest.json';
import { resolveLocale, type Catalog } from './core';
import { applyDocumentLocale, setLocaleData } from './index';
import { installedCatalog, installedInfo } from './installed';
import { nativeName } from './languages';

export interface LocaleInfo {
  /** A BCP 47 code: 'fr', 'pt-BR', 'zh-Hans'. */
  code: string;
  /** The language's name in that language, as its speakers write it. */
  name: string;
  dir: 'ltr' | 'rtl';
}

export interface LocaleFile {
  locale: string;
  name: string;
  dir?: 'ltr' | 'rtl';
  messages?: Catalog['messages'];
  contexts?: Catalog['contexts'];
}

/** The test language: every message bent and framed, to see at a glance which words are not in the catalog yet. */
export const PSEUDO_CODE = 'en-XA';

/** The languages that come with the program. */
export const LOCALES: readonly LocaleInfo[] = manifest.locales as LocaleInfo[];

type Loader = () => Promise<LocaleFile>;
let loaders: Record<string, Loader> = {};
try {
  // Vite turns this call into a table of files at build time. Outside Vite (the unit tests) it is not there.
  loaders = import.meta.glob('./locales/*.json', { import: 'default' }) as Record<string, Loader>;
} catch {
  loaders = {};
}

/** One entry of the list the person chooses from. */
export interface LanguageChoice {
  /** What the setting holds: the code for a language that comes with the program, or the name of an added translation. */
  id: string;
  code: string;
  /** What the list shows. */
  label: string;
  dir: 'ltr' | 'rtl';
  source: 'built-in' | 'added';
}

/** English first, then the languages that come with the program, then the translations added on this computer. */
export function listLanguages(): LanguageChoice[] {
  const out: LanguageChoice[] = LOCALES.map((l) => ({ id: l.code, code: l.code, label: l.name, dir: l.dir, source: 'built-in' }));
  const taken = new Set(out.map((l) => l.id));
  for (const p of installedInfo()) {
    if (taken.has(p.id)) continue;
    out.push({ id: p.id, code: p.code, label: p.label, dir: p.dir, source: 'added' });
  }
  return out;
}

/** Codes of the languages that can be chosen. The test language is accepted but never listed. */
export function availableCodes(): string[] {
  return listLanguages().map((l) => l.code);
}

/**
 * What to use for a setting ('auto', or the id of a language) on a computer whose languages are given. The answer is an id; a
 * setting that names a language that is not there any more (a translation that was removed) gives English.
 */
export function chooseLocale(choice: string, system: readonly string[]): string {
  if (choice === PSEUDO_CODE) return PSEUDO_CODE;
  const all = listLanguages();
  if (choice !== 'auto') return all.some((l) => l.id === choice) ? choice : 'en';
  const code = resolveLocale('auto', all.map((l) => l.code), system);
  return all.find((l) => l.code === code)?.id ?? 'en';
}

/** The name of a language for the person, from the standard names where the file does not say. */
export function labelFor(code: string): string {
  return LOCALES.find((l) => l.code === code)?.name ?? nativeName(code);
}

/** Puts a language in use and tells the page. Falls back to English if it cannot be read. */
export async function activateLocale(id: string, root?: { lang: string; dir: string }): Promise<string> {
  let used = id;
  if (id === 'en' || id === PSEUDO_CODE) {
    setLocaleData(id, null, 'ltr');
  } else {
    const choice = listLanguages().find((l) => l.id === id);
    const load = choice?.source === 'built-in' ? loaders[`./locales/${id}.json`] : undefined;
    if (!choice) {
      used = 'en';
      setLocaleData('en', null, 'ltr');
    } else if (choice.source === 'added') {
      const catalog = installedCatalog(id);
      if (!catalog) {
        console.error(`The translation ${id} could not be read; English is used.`);
        used = 'en';
        setLocaleData('en', null, 'ltr');
      } else setLocaleData(choice.code, catalog, choice.dir, id);
    } else if (!load) {
      used = 'en';
      setLocaleData('en', null, 'ltr');
    } else {
      try {
        const file = await load();
        setLocaleData(choice.code, { messages: file.messages ?? {}, contexts: file.contexts }, file.dir ?? choice.dir, id);
      } catch (e) {
        console.error(`The ${id} language could not be read; English is used.`, e);
        used = 'en';
        setLocaleData('en', null, 'ltr');
      }
    }
  }
  if (root) applyDocumentLocale(root);
  return used;
}
