// Adding, saving, and removing translations: what the Language part of Settings does. The file system is reached through the
// engine's text.read and text.write, like every other file the window touches, so the same steps work in the desktop window and
// in the browser check.

import { call } from '../bridge';
import { createStore } from '../state/store';
import type { TextFile } from '../types';
import { installedCatalog, installedInfo, addInstalled, removeInstalled, type AddResult } from './installed';
import { canonicalCode } from './languages';
import { checkDraft, codeFromFileName, packToJson, parsePack, translatorCsv, translatorJson, type Checked, type Draft, type MadeBy, type PackInfo, type SourceMessage } from './pack';

/** Changes when a translation is added or removed, so the list on the Settings page draws again. */
export const languagesStore = createStore({ version: 0 });
export const bumpLanguages = (): void => languagesStore.set((s) => ({ version: s.version + 1 }));

let source: SourceMessage[] | null = null;

/** Every message of the program, loaded when it is first needed (the list is large and only this part uses it). */
export async function sourceMessages(): Promise<SourceMessage[]> {
  if (!source) {
    const file = await import('./messages.json');
    source = (file.default as unknown as { messages: SourceMessage[] }).messages;
  }
  return source;
}

export interface ReadResult {
  fileName: string;
  encoding: TextFile['encoding'];
  draft: Draft;
  checked: Checked;
  /** The language the file suggests, from what it says or from its name, if that is a language of the standard list. */
  suggestedCode: string | null;
}

/** Reads a translation file and checks it against the program. Throws an ApiError or a PackError with a reason for the person. */
export async function readTranslation(path: string): Promise<ReadResult> {
  const file = await call('text.read', { path });
  const draft = parsePack(file.text, file.name);
  const checked = checkDraft(draft, await sourceMessages());
  const suggestedCode = (draft.code && canonicalCode(draft.code)) || codeFromFileName(file.name);
  return { fileName: file.name, encoding: file.encoding, draft, checked, suggestedCode };
}

export interface NewTranslation {
  code: string;
  label: string;
  credit: string;
  madeBy: MadeBy;
  /** The id of a translation to take out in its place. */
  replace?: string;
}

/** Keeps a checked translation. */
export function installTranslation(checked: Checked, how: NewTranslation): AddResult {
  const code = canonicalCode(how.code);
  if (!code) return { ok: false, reason: 'Choose the language this translation is for.' };
  const added = addInstalled({ code, label: how.label.trim() || code, credit: how.credit.trim(), madeBy: how.madeBy, done: checked.done, total: checked.total }, checked.catalog, how.replace);
  if (added.ok) bumpLanguages();
  return added;
}

export function removeTranslation(id: string): void {
  removeInstalled(id);
  bumpLanguages();
}

/** The text of a spreadsheet or JSON file for a translator, for a language, optionally with only what is missing. */
export async function translatorFile(format: 'csv' | 'json', code: string, onlyMissing: boolean, id?: string): Promise<{ text: string; rows: number }> {
  const src = await sourceMessages();
  const existing = id ? (installedCatalog(id) ?? undefined) : undefined;
  if (format === 'csv') return translatorCsv(src, code, existing, onlyMissing);
  const info = id ? installedInfo().find((p) => p.id === id) : undefined;
  const text = translatorJson(src, code, info, existing, onlyMissing);
  return { text, rows: (text.match(/\n/g) ?? []).length };
}

/** Writes a file to a path the person chose. `replace` is true when the native save dialog has already asked about an existing file. */
export async function writeFile(path: string, text: string, replace: boolean): Promise<string> {
  const r = await call('text.write', { path, text, replace });
  return r.path;
}

/** A translation already added, as the JSON file to keep or to give to someone. */
export function translationAsJson(info: PackInfo): string | null {
  const catalog = installedCatalog(info.id);
  return catalog ? packToJson(info, catalog) : null;
}
