// Works out what a code should be applied to, from what the person has selected.
//
// Free-span projects code exactly the selected words. Unit projects code units: nothing selected
// means the unit at the caret, and a selection across several units codes each unit it touches.

import { OffsetIndex, trimSpan } from '../offsets';
import type { CodingMode, UnitRow } from '../types';
import { unitIndexForCaret, unitIndicesTouching } from './units';
import { t } from '../i18n';

export interface TargetSpan {
  start: number;
  end: number;
  unit_id: number | null;
}

export type TargetResult =
  /** `selected` is true when the person selected words (as opposed to coding the unit at the caret). */
  | { ok: true; spans: TargetSpan[]; skipped: number; selected: boolean }
  | { ok: false; message: string };

export interface Selection {
  start: number;
  end: number;
}

const nothingSelected = () => t('Select some words first. Use the mouse, or hold Shift and press the arrow keys, then apply the code.');

export function documentTarget(args: {
  text: string;
  index: OffsetIndex;
  selection: Selection | null;
  mode: CodingMode;
  units: readonly UnitRow[];
}): TargetResult {
  const { text, index, selection, mode, units } = args;
  const collapsed = !selection || selection.start === selection.end;

  if (mode === 'free_span') {
    if (!selection || collapsed) return { ok: false, message: nothingSelected() };
    const span = trimSpan(text, index, selection.start, selection.end);
    if (!span) return { ok: false, message: t('Only spaces are selected. Select some words and try again.') };
    return { ok: true, spans: [{ ...span, unit_id: null }], skipped: 0, selected: true };
  }

  // Unit mode.
  if (units.length === 0) {
    return { ok: false, message: t('This project codes whole units, but this source has no units to code.') };
  }
  if (!selection) {
    return { ok: false, message: t('Put the caret in a unit first, or select some of its words.') };
  }
  if (collapsed) {
    const at = unitIndexForCaret(units, selection.start);
    if (at < 0) return { ok: false, message: t('The caret is not inside a unit. Move it into the text of a unit.') };
    const u = units[at];
    if (u.excluded) return { ok: false, message: t('This unit is left out of coding, for example an interviewer turn.') };
    return { ok: true, spans: [{ start: u.start, end: u.end, unit_id: u.id }], skipped: 0, selected: false };
  }
  const touched = unitIndicesTouching(units, selection.start, selection.end);
  const spans: TargetSpan[] = [];
  let skipped = 0;
  for (const i of touched) {
    const u = units[i];
    if (u.excluded) {
      skipped++;
      continue;
    }
    const from = Math.max(selection.start, u.start);
    const to = Math.min(selection.end, u.end);
    const span = trimSpan(text, index, from, to);
    if (span) spans.push({ ...span, unit_id: u.id });
  }
  if (spans.length === 0) {
    return {
      ok: false,
      message: skipped > 0 ? t('The selected units are left out of coding, for example interviewer turns.') : t('The selection does not include any words of a unit.'),
    };
  }
  return { ok: true, spans, skipped, selected: true };
}

/** Whole units chosen in the grid. */
export function unitsTarget(selected: readonly UnitRow[]): TargetResult {
  if (selected.length === 0) return { ok: false, message: t('Select a row first.') };
  const spans: TargetSpan[] = [];
  let skipped = 0;
  for (const u of selected) {
    if (u.excluded) skipped++;
    else spans.push({ start: u.start, end: u.end, unit_id: u.id });
  }
  if (spans.length === 0) return { ok: false, message: t('The selected rows are left out of coding, for example interviewer turns.') };
  return { ok: true, spans, skipped, selected: false };
}

/** The words for "Coded 3 units as Welcome" style announcements. */
export function describeCount(n: number, one: string, many = one + 's'): string {
  return `${n} ${n === 1 ? one : many}`;
}
