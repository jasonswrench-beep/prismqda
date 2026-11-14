// Finding units: which unit a position is in, and where the next unit without a coding is.

import type { UnitRow } from '../types';

type Spanned = Pick<UnitRow, 'start' | 'end'>;

/** The index of the unit that contains the position (start inclusive, end exclusive), or -1. Units must be in text order. */
export function unitIndexAt(units: readonly Spanned[], offset: number): number {
  let lo = 0;
  let hi = units.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    const u = units[mid];
    if (offset < u.start) hi = mid - 1;
    else if (offset >= u.end) lo = mid + 1;
    else return mid;
  }
  return -1;
}

/**
 * The unit a caret belongs to. A caret at the very end of a unit still belongs to it, so a person
 * who has just selected a unit's last word is not sent to the next unit.
 */
export function unitIndexForCaret(units: readonly Spanned[], offset: number): number {
  const at = unitIndexAt(units, offset);
  if (at >= 0) return at;
  const before = unitIndexAt(units, offset - 1);
  return before;
}

/** Indices of the units that a span touches, in order. */
export function unitIndicesTouching(units: readonly Spanned[], start: number, end: number): number[] {
  const out: number[] = [];
  let first = unitIndexAt(units, start);
  if (first < 0) {
    // Start in a gap between units: begin at the first unit that starts after it.
    first = units.findIndex((u) => u.start >= start);
    if (first < 0) return out;
  }
  for (let i = first; i < units.length && units[i].start < end; i++) {
    if (units[i].end > start) out.push(i);
  }
  return out;
}

/**
 * The index of the next unit (or the previous one, for direction -1) that can be coded and has no
 * coding yet. Returns -1 when there is none.
 */
export function nextUncodedUnit(
  units: readonly Pick<UnitRow, 'id' | 'excluded'>[],
  codedUnitIds: ReadonlySet<number>,
  from: number,
  direction: 1 | -1 = 1,
): number {
  for (let i = from + direction; i >= 0 && i < units.length; i += direction) {
    const u = units[i];
    if (!u.excluded && !codedUnitIds.has(u.id)) return i;
  }
  return -1;
}
