// The list of recent projects on the launcher: where each file is, what the project is called, and when it was last
// opened here. One project is one entry however it was opened (the picker, the list, or a located copy).
import { getLocaleState, t } from '../i18n';

export interface RecentEntry {
  /** The path the project was last opened from. */
  path: string;
  /** The project's name as it was when last opened. */
  name: string;
  /** The project's own identity, so a moved file can be recognised; null for entries from an older version. */
  uid: string | null;
  /** When the project was last opened here, in milliseconds since 1970. */
  openedAt: number;
}

export const RECENT_KEY = 'prismqda.recent.v2';
/** The list kept by earlier alphas: paths only. */
export const OLD_RECENT_KEY = 'prismqda.recentProjects';
export const MAX_RECENT = 8;

/** The file name of a path, with either kind of slash. */
export function fileNameOf(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function samePath(a: string, b: string): boolean {
  // Windows paths do not care about capitals or which slash is used.
  const flat = (p: string) => p.replace(/\//g, '\\').toLowerCase();
  return flat(a) === flat(b);
}

/** Cleans what was read from storage: anything that is not a usable entry is dropped, repeats are merged, newest first. */
export function normalizeRecent(raw: unknown): RecentEntry[] {
  if (!Array.isArray(raw)) return [];
  const out: RecentEntry[] = [];
  for (const r of raw) {
    if (!r || typeof r !== 'object') continue;
    const e = r as Record<string, unknown>;
    if (typeof e.path !== 'string' || e.path.trim() === '') continue;
    const entry: RecentEntry = {
      path: e.path,
      name: typeof e.name === 'string' && e.name.trim() !== '' ? e.name : fileNameOf(e.path).replace(/\.[^.]+$/, ''),
      uid: typeof e.uid === 'string' && e.uid !== '' ? e.uid : null,
      openedAt: typeof e.openedAt === 'number' && Number.isFinite(e.openedAt) ? e.openedAt : 0,
    };
    if (out.some((x) => samePath(x.path, entry.path) || (entry.uid !== null && x.uid === entry.uid))) continue;
    out.push(entry);
  }
  return out.sort((a, b) => b.openedAt - a.openedAt).slice(0, MAX_RECENT);
}

/** Turns the old list of paths into entries; the order of the old list is kept by giving each a slightly earlier time. */
export function migrateOldRecent(raw: unknown, now: number): RecentEntry[] {
  if (!Array.isArray(raw)) return [];
  const paths = raw.filter((p): p is string => typeof p === 'string' && p.trim() !== '');
  return normalizeRecent(paths.map((path, i) => ({ path, openedAt: now - (i + 1) * 1000 })));
}

/** Puts a project at the top of the list. A project already there (by path or by identity) is replaced, so there is one entry. */
export function rememberRecent(list: readonly RecentEntry[], entry: RecentEntry): RecentEntry[] {
  const rest = list.filter((x) => !samePath(x.path, entry.path) && !(entry.uid !== null && x.uid === entry.uid));
  return [entry, ...rest].slice(0, MAX_RECENT);
}

/** Takes an entry off the list. Only the list changes; the file is never touched. */
export function forgetRecent(list: readonly RecentEntry[], path: string): RecentEntry[] {
  return list.filter((x) => !samePath(x.path, path));
}

/** "today, 2:14 PM", "yesterday, 6:37 PM", or "Jan 20, 2026". */
export function describeWhen(ts: number, now: number, locale?: string): string {
  const d = new Date(ts);
  const n = new Date(now);
  if (Number.isNaN(d.getTime())) return '';
  const midnight = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((midnight(n) - midnight(d)) / 86_400_000);
  const tag = locale ?? getLocaleState().tag;
  const time = new Intl.DateTimeFormat(tag, { hour: 'numeric', minute: '2-digit' }).format(d);
  if (days === 0) return t('today, {time}', { time });
  if (days === 1) return t('yesterday, {time}', { time });
  return new Intl.DateTimeFormat(tag, { dateStyle: 'medium' }).format(d);
}
