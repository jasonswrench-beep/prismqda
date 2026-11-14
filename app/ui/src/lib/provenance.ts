// What the audit trail says about one coding: when it was made, when it last changed, and how many codes were added to
// and taken off these words. The counts follow the book: "1 code added, 0 removed" for a coding nothing has touched.
import { t } from '../i18n';

export interface HistoryEvent {
  ts: string;
  actor: string;
  action: string;
}

export interface HistorySummary {
  /** When the oldest event happened, or null with no events. */
  firstAt: string | null;
  /** When the newest event happened (a note or a rating counts as a change too), or null with no events. */
  lastAt: string | null;
  added: number;
  removed: number;
}

/** Reads a list of events (in any order) into the numbers the provenance section shows. */
export function summarizeHistory(events: readonly HistoryEvent[]): HistorySummary {
  let added = 0;
  let removed = 0;
  let firstAt: string | null = null;
  let lastAt: string | null = null;
  for (const e of events) {
    // The engine writes "coding.add" for every new coding (applied by hand, imported, or accepted from a suggestion).
    if (e.action === 'coding.add' || e.action === 'coding.apply') added += 1;
    else if (e.action === 'coding.remove') removed += 1;
    else if (e.action === 'coding.change_code') {
      added += 1;
      removed += 1;
    }
    const t = Date.parse(e.ts);
    if (Number.isNaN(t)) continue;
    if (firstAt === null || t < Date.parse(firstAt)) firstAt = e.ts;
    if (lastAt === null || t > Date.parse(lastAt)) lastAt = e.ts;
  }
  return { firstAt, lastAt, added, removed };
}

/** "1 code added, 0 removed" */
export function describeHistory(h: Pick<HistorySummary, 'added' | 'removed'>): string {
  const codes = (n: number) => `${n} ${n === 1 ? 'code' : 'codes'}`;
  return t('{codes} added, {removed} removed', { codes: codes(h.added), removed: h.removed });
}

/** "M-014": the number a memo goes by in lists, from its id. */
export function memoNumber(id: number): string {
  return `M-${String(id).padStart(3, '0')}`;
}

/** The initials in an avatar: "RM" for Rosa Martin, "A" for Ada. */
export function initialsOf(name: string): string {
  const words = name.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  if (words.length === 0) return '?';
  return words
    .slice(0, 2)
    .map((w) => Array.from(w)[0]!.toUpperCase())
    .join('');
}
