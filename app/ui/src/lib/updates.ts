// What the window knows about program updates (decision 189). The program checks for a newer version only when the person presses the button.

/** Where new versions are published, for a person who cannot or should not update in place. */
export const RELEASES_PAGE = 'https://github.com/jasonswrench-beep/prismqda/releases';

/** Why a check cannot be made now, as the shell says it. */
export type UpdateBlock = 'not_set_up' | 'offline_lock';

/** What the shell says before anything is asked. */
export interface UpdateState {
  /** The version of this program. */
  version: string;
  /** Whether this build has an update address and a signing key. */
  set_up: boolean;
  /** The computer a check would ask (the address's host), or empty. */
  host: string;
  /** A portable copy is never updated in place. */
  portable: boolean;
  blocked: UpdateBlock | null;
}

/** What a check found. */
export interface UpdateFound {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

/** The share of a download that has arrived, as a whole number from 0 to 100, or null when the total is not known. */
export function progressPercent(received: number, total: number | null | undefined): number | null {
  if (typeof total !== 'number' || !Number.isFinite(total) || total <= 0 || !Number.isFinite(received)) return null;
  return Math.max(0, Math.min(100, Math.floor((received / total) * 100)));
}

/** The notes of a release for showing in the window: trimmed, long runs of blank lines cut to one, and no more than `max` characters. */
export function shortNotes(notes: string | undefined, max = 1500): string {
  const clean = (notes ?? '').replace(/\r\n/g, '\n').replace(/\n{3,}/g, '\n\n').trim();
  if (clean.length <= max) return clean;
  const cut = Array.from(clean).slice(0, max).join('').trimEnd();
  return `${cut}…`;
}
