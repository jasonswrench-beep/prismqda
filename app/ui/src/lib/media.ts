// Recordings: clock text, and the way between a place in a recording and a place in its transcript. Plain functions, so they can
// be tested without a screen or a player.

import type { UnitRow } from '../types';

/** A unit that says when in the recording it is spoken. */
export interface TimedUnit extends UnitRow {
  time_start_ms: number;
  time_end_ms: number;
}

const timed = (u: UnitRow): u is TimedUnit => typeof u.time_start_ms === 'number' && typeof u.time_end_ms === 'number';

/** The units that have times, in the order they are spoken. */
export function timedUnits(units: readonly UnitRow[]): TimedUnit[] {
  return units.filter(timed).sort((a, b) => a.time_start_ms - b.time_start_ms || a.start - b.start);
}

/** `1:02` or `1:02:03`: whole seconds, with hours only when there are some. A time that is not a number reads as `0:00`. */
export function formatClock(ms: number | null | undefined): string {
  const total = Math.max(0, Math.floor((Number.isFinite(ms ?? NaN) ? (ms as number) : 0) / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${m}:${two(s)}`;
}

/**
 * The unit being spoken at `ms`: the last one that has started by then (a pause between two turns still belongs to the turn before it).
 * `null` before the first turn starts, and when there are no times.
 */
export function unitAt(units: readonly TimedUnit[], ms: number): TimedUnit | null {
  let found: TimedUnit | null = null;
  for (const u of units) {
    if (u.time_start_ms <= ms) found = u;
    else break;
  }
  return found;
}

/** Where in the recording the words at `offset` (a character position in the transcript) are spoken: the start of the unit they are in. */
export function timeForOffset(units: readonly TimedUnit[], offset: number): number | null {
  const hit = units.find((u) => u.start <= offset && offset < u.end) ?? [...units].reverse().find((u) => u.start <= offset);
  return hit ? hit.time_start_ms : null;
}

/** A turn as one line for a list: when it is spoken, who speaks, and its first words. */
export function turnLabel(u: TimedUnit, text: string, max = 60): string {
  const words = Array.from(text.slice(0, 400).replace(/\s+/g, ' ').trim());
  const shown = words.length > max ? `${words.slice(0, max - 1).join('').trimEnd()}…` : words.join('');
  return `${formatClock(u.time_start_ms)} ${shown}`;
}
