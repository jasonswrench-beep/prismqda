// The translations that have been added on this computer. They are kept with the person's other settings (not in a project, since
// the language of the window belongs to the person and not to the study): an index of what is there, and each translation under
// its own key so that a damaged one cannot take the others with it.

import type { Catalog, Entry } from './core';
import { canonicalCode, directionOf } from './languages';
import type { MadeBy, PackInfo } from './pack';

const INDEX_KEY = 'prismqda.languages';
const packKey = (id: string) => `prismqda.language.${id}`;

function storage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

const isRecord = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === 'object' && !Array.isArray(v);
const text = (v: unknown, max = 200): string => (typeof v === 'string' ? v.slice(0, max) : '');

/** What is in the index, cleaned: anything that does not look like a translation's record is left out. */
export function installedInfo(): PackInfo[] {
  let raw: unknown;
  try {
    const s = storage()?.getItem(INDEX_KEY);
    raw = s ? JSON.parse(s) : [];
  } catch {
    raw = [];
  }
  if (!Array.isArray(raw)) return [];
  const out: PackInfo[] = [];
  for (const r of raw) {
    if (!isRecord(r)) continue;
    const id = text(r.id, 40);
    const code = canonicalCode(text(r.code, 40) || '');
    if (!/^[A-Za-z0-9-]+(~\d+)?$/.test(id) || !code) continue;
    const madeBy: MadeBy = r.madeBy === 'reviewed' || r.madeBy === 'machine' ? r.madeBy : 'person';
    out.push({
      id,
      code,
      label: text(r.label) || code,
      credit: text(r.credit),
      madeBy,
      added: text(r.added, 10),
      dir: r.dir === 'rtl' ? 'rtl' : 'ltr',
      done: typeof r.done === 'number' ? r.done : 0,
      total: typeof r.total === 'number' ? r.total : 0,
    });
  }
  return out;
}

/** The words of an added translation, or null if it is not there or cannot be read. */
export function installedCatalog(id: string): Catalog | null {
  try {
    const s = storage()?.getItem(packKey(id));
    if (!s) return null;
    const data: unknown = JSON.parse(s);
    if (!isRecord(data) || !isRecord(data.messages)) return null;
    const catalog: Catalog = { messages: data.messages as Record<string, Entry> };
    if (isRecord(data.contexts)) catalog.contexts = data.contexts as Record<string, Record<string, Entry>>;
    return catalog;
  } catch {
    return null;
  }
}

/** The name for the next translation of a language: fr, then fr~2, fr~3. */
export function freeId(code: string, taken: readonly string[]): string {
  if (!taken.includes(code)) return code;
  for (let n = 2; ; n++) {
    const id = `${code}~${n}`;
    if (!taken.includes(id)) return id;
  }
}

export type AddResult = { ok: true; info: PackInfo } | { ok: false; reason: string };

/**
 * Keeps a translation. `replace` names one that is taken out in its place. Storage can be full or blocked, and then nothing
 * changes and the reason is returned.
 */
export function addInstalled(entry: { code: string; label: string; credit: string; madeBy: MadeBy; done: number; total: number }, catalog: Catalog, replace?: string): AddResult {
  const s = storage();
  if (!s) return { ok: false, reason: 'This window cannot keep files on this computer, so a translation cannot be added.' };
  const have = installedInfo();
  const others = have.filter((p) => p.id !== replace);
  const id = replace && have.some((p) => p.id === replace) && have.find((p) => p.id === replace)?.code === entry.code ? replace : freeId(entry.code, others.map((p) => p.id));
  const info: PackInfo = { id, code: entry.code, label: entry.label, credit: entry.credit, madeBy: entry.madeBy, added: new Date().toISOString().slice(0, 10), dir: directionOf(entry.code), done: entry.done, total: entry.total };
  const body = JSON.stringify({ messages: catalog.messages, contexts: catalog.contexts ?? {} });
  try {
    s.setItem(packKey(id), body);
    if (s.getItem(packKey(id)) !== body) throw new Error('not kept');
    const next = [...others.filter((p) => p.id !== id), info];
    s.setItem(INDEX_KEY, JSON.stringify(next));
  } catch {
    try {
      s.removeItem(packKey(id));
    } catch {
      // nothing more to do
    }
    return { ok: false, reason: 'There is not enough room to keep this translation on this computer. Remove one you do not use, and try again.' };
  }
  if (replace && replace !== id) removeInstalled(replace);
  return { ok: true, info };
}

export function removeInstalled(id: string): void {
  const s = storage();
  if (!s) return;
  try {
    s.removeItem(packKey(id));
    s.setItem(INDEX_KEY, JSON.stringify(installedInfo().filter((p) => p.id !== id)));
  } catch {
    // nothing more to do
  }
}
