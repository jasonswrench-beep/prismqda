// Cuts a text into pieces so that inside each piece the same codings, formatting, and unit apply.
// The document view draws one element per piece. Everything here counts scalar values.

import type { FormatRun } from '../types';

export interface SegmentCoding {
  id: number;
  start: number;
  end: number;
}

export interface SegmentUnit {
  start: number;
  end: number;
}

export interface Segment {
  start: number;
  end: number;
  /** Ids of the codings that cover this whole piece. */
  codings: number[];
  /** Heading level 1 to 6, or 0 when the piece is not part of a heading. */
  heading: number;
  bold: boolean;
  italic: boolean;
  listItem: boolean;
  tableCell: boolean;
  /** Page number when the importer marked pages, else null. */
  page: number | null;
  /** Index into the units list, or -1 when the piece lies outside every unit. */
  unit: number;
  /** True when a unit begins exactly where this piece begins. */
  unitStart: boolean;
  /** True when the piece is a speaker label at the start of a line ("Alex: "), drawn in a column of its own. */
  label: boolean;
}

export interface SegmentInput {
  /** Length of the text in scalar values. */
  length: number;
  codings: SegmentCoding[];
  runs: FormatRun[];
  /** Units in text order. */
  units: SegmentUnit[];
  /** Speaker labels in text order; each becomes pieces of its own. */
  labels?: { start: number; end: number }[];
}

type Change = { at: number; kind: 'coding' | 'run'; open: boolean; index: number };

export function buildSegments(input: SegmentInput): Segment[] {
  const { length } = input;
  if (length <= 0) return [];
  const clamp = (n: number) => Math.min(Math.max(n, 0), length);

  const cuts = new Set<number>([0, length]);
  const changes: Change[] = [];
  input.codings.forEach((c, index) => {
    const s = clamp(c.start);
    const e = clamp(c.end);
    if (e <= s) return;
    cuts.add(s);
    cuts.add(e);
    changes.push({ at: s, kind: 'coding', open: true, index }, { at: e, kind: 'coding', open: false, index });
  });
  input.runs.forEach((r, index) => {
    const s = clamp(r.start);
    const e = clamp(r.end);
    if (e <= s) return;
    cuts.add(s);
    cuts.add(e);
    changes.push({ at: s, kind: 'run', open: true, index }, { at: e, kind: 'run', open: false, index });
  });
  for (const u of input.units) {
    const s = clamp(u.start);
    const e = clamp(u.end);
    if (e <= s) continue;
    cuts.add(s);
    cuts.add(e);
  }
  const labels = (input.labels ?? []).filter((l) => l.end > l.start).sort((a, b) => a.start - b.start);
  for (const l of labels) {
    cuts.add(clamp(l.start));
    cuts.add(clamp(l.end));
  }

  const points = Array.from(cuts).sort((a, b) => a - b);
  // Closing before opening at the same point keeps a piece from inheriting a coding that just ended.
  changes.sort((a, b) => a.at - b.at || Number(a.open) - Number(b.open));

  const activeCodings = new Set<number>();
  const activeRuns = new Set<number>();
  const segments: Segment[] = [];
  let c = 0;
  let unitPointer = 0;
  let labelPointer = 0;
  for (let p = 0; p + 1 < points.length; p++) {
    const start = points[p];
    const end = points[p + 1];
    while (c < changes.length && changes[c].at <= start) {
      const ch = changes[c++];
      const set = ch.kind === 'coding' ? activeCodings : activeRuns;
      if (ch.open) set.add(ch.index);
      else set.delete(ch.index);
    }
    while (unitPointer < input.units.length && clamp(input.units[unitPointer].end) <= start) unitPointer++;
    let unit = -1;
    let unitStart = false;
    if (unitPointer < input.units.length) {
      const u = input.units[unitPointer];
      if (clamp(u.start) <= start && start < clamp(u.end)) {
        unit = unitPointer;
        unitStart = clamp(u.start) === start;
      }
    }
    while (labelPointer < labels.length && labels[labelPointer].end <= start) labelPointer++;
    const label = labelPointer < labels.length && labels[labelPointer].start <= start && end <= labels[labelPointer].end;
    let heading = 0;
    let bold = false;
    let italic = false;
    let listItem = false;
    let tableCell = false;
    let page: number | null = null;
    for (const ri of activeRuns) {
      const r = input.runs[ri];
      if (r.kind === 'heading') {
        const level = Math.min(Math.max(r.level ?? 1, 1), 6);
        heading = heading === 0 ? level : Math.min(heading, level);
      } else if (r.kind === 'bold') bold = true;
      else if (r.kind === 'italic') italic = true;
      else if (r.kind === 'list_item') listItem = true;
      else if (r.kind === 'table_cell') tableCell = true;
      else if (r.kind === 'page') page = r.page ?? page;
    }
    const codingIds: number[] = [];
    for (const ci of activeCodings) codingIds.push(input.codings[ci].id);
    segments.push({ start, end, codings: codingIds, heading, bold, italic, listItem, tableCell, page, unit, unitStart, label });
  }
  return segments;
}
