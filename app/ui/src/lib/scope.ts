// "Here" for the Codings here panel: the unit (in a unit project) or the paragraph at the caret.

import type { OffsetIndex } from '../offsets';
import type { CodingMode, CodingRow, UnitRow } from '../types';
import { unitIndexForCaret } from './units';
import { t } from '../i18n';

export interface Span {
  start: number;
  end: number;
}

export interface Scope extends Span {
  kind: 'unit' | 'paragraph' | 'none';
  /** Plain words such as "Paragraph 3 of 12" or "Unit 4 of 20 (Interviewer)". */
  label: string;
}

/** Paragraphs are runs of text separated by a blank line. Offsets are scalar values. */
export function paragraphsOf(text: string, index: OffsetIndex): Span[] {
  const out: Span[] = [];
  const breaks = /\n[ \t]*\n\s*/g;
  let from = 0;
  const push = (u0: number, u1: number) => {
    if (text.slice(u0, u1).trim() === '') return;
    out.push({ start: index.toScalar(u0), end: index.toScalar(u1, 'up') });
  };
  let m: RegExpExecArray | null;
  while ((m = breaks.exec(text)) !== null) {
    push(from, m.index);
    from = m.index + m[0].length;
  }
  push(from, text.length);
  return out;
}

function paragraphIndexAt(paragraphs: readonly Span[], caret: number): number {
  let lo = 0;
  let hi = paragraphs.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    if (paragraphs[mid].start <= caret) {
      found = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return found;
}

export function scopeAt(args: { paragraphs: readonly Span[]; units: readonly UnitRow[]; mode: CodingMode; caret: number }): Scope {
  const { paragraphs, units, mode, caret } = args;
  if (mode === 'unit' && units.length > 0) {
    const i = unitIndexForCaret(units, caret);
    if (i >= 0) {
      const u = units[i];
      const who = u.speaker ? ` (${u.speaker})` : '';
      return { kind: 'unit', start: u.start, end: u.end, label: t('Unit {i} of {count}{who}', { i: i + 1, count: units.length, who }) };
    }
  }
  const p = paragraphIndexAt(paragraphs, caret);
  if (p < 0) return { kind: 'none', start: 0, end: 0, label: t('Nothing here yet') };
  const para = paragraphs[p];
  return { kind: 'paragraph', start: para.start, end: para.end, label: t('Paragraph {number} of {count}', { number: p + 1, count: paragraphs.length }) };
}

/** Codings that touch a span, in text order. */
export function codingsIn(codings: readonly CodingRow[], span: Span): CodingRow[] {
  return codings.filter((c) => c.start < span.end && c.end > span.start).sort((a, b) => a.start - b.start || a.end - b.end || a.id - b.id);
}

/** Codings that contain the caret. A caret at the end of the text counts for a coding that ends there. */
export function codingsAtCaret(codings: readonly CodingRow[], caret: number, textLength: number): CodingRow[] {
  return codings
    .filter((c) => (c.start <= caret && caret < c.end) || (caret === textLength && c.end === textLength && c.start < caret))
    .sort((a, b) => a.end - a.start - (b.end - b.start) || a.id - b.id);
}
