// The document portrait and the codeline (decision 164): where the coded passages of a document fall along it, from its first word to its last.
// The portrait draws every document as one strip with a mark for each coding; the codeline draws one document with a row for each code. The
// words of the passage and the code are on every mark, and a table gives the same numbers, so nothing is told by color alone.
// Only passages of words are drawn: an area of a picture or a stretch of a recording has no place along a text.

import type { CodeRow, SegmentRow, SourceRow } from '../types';
import { compareText } from '../i18n';

/** The most marks drawn for one document; the table has every code's totals. */
export const MAX_MARKS = 600;
/** The most rows a strip is stacked into when passages overlap; passages beyond that share the last row. */
export const MAX_LANES = 6;

export interface PortraitMark {
  codingId: number;
  codeId: number;
  codeName: string;
  color: string;
  start: number;
  end: number;
  text: string;
  /** The row of the strip the mark is drawn in, from 0, so that marks that overlap do not hide each other. */
  lane: number;
}

/** What one code does in one document. */
export interface CodeUse {
  codeId: number;
  name: string;
  color: string;
  passages: number;
  /** Characters covered by this code (overlapping passages of one code counted once). */
  characters: number;
  /** Where its first passage starts and its last ends, as a share of the document, 0 to 100. */
  from: number;
  to: number;
}

export interface Portrait {
  sourceId: number;
  name: string;
  characters: number;
  marks: PortraitMark[];
  lanes: number;
  /** The share of the document, 0 to 100, covered by at least one coding. */
  coverage: number;
  uses: CodeUse[];
  /** Passages not drawn because the document has more than `MAX_MARKS`. */
  notDrawn: number;
}

/** A character position as a share of the document, kept inside 0 to 100. */
export function share(offset: number, characters: number): number {
  if (characters <= 0) return 0;
  return Math.min(100, Math.max(0, (offset / characters) * 100));
}

/** The length of the union of intervals, so a passage coded twice is counted once. */
export function unionLength(intervals: readonly { start: number; end: number }[]): number {
  const sorted = [...intervals].sort((a, b) => a.start - b.start || a.end - b.end);
  let total = 0;
  let from = -1;
  let to = -1;
  for (const i of sorted) {
    if (i.start > to) {
      if (to > from) total += to - from;
      from = i.start;
      to = i.end;
    } else if (i.end > to) {
      to = i.end;
    }
  }
  if (to > from) total += to - from;
  return total;
}

/** Gives each interval the first row in which it does not overlap one already there; beyond `maxLanes` it goes in the last row. */
export function assignLanes(intervals: readonly { start: number; end: number }[], maxLanes: number = MAX_LANES): number[] {
  const order = intervals.map((_, i) => i).sort((a, b) => intervals[a].start - intervals[b].start || intervals[b].end - intervals[a].end);
  const ends: number[] = [];
  const lane = new Array<number>(intervals.length).fill(0);
  for (const i of order) {
    let l = ends.findIndex((e) => e <= intervals[i].start);
    if (l === -1) l = ends.length < maxLanes ? ends.length : maxLanes - 1;
    ends[l] = Math.max(ends[l] ?? 0, intervals[i].end);
    lane[i] = l;
  }
  return lane;
}

/**
 * One portrait for each document that has coded words, in the order of the documents. `leaveOut` holds codes the person does not want drawn.
 * A document with no coded words is left out; so is anything that is not words (pictures' areas, stretches of recordings).
 */
export function buildPortraits(segments: readonly SegmentRow[], sources: readonly SourceRow[], codes: readonly CodeRow[], leaveOut: ReadonlySet<number> = new Set()): Portrait[] {
  const colorOf = new Map(codes.map((c) => [c.id, c.color]));
  const nameOf = new Map(codes.map((c) => [c.id, c.name]));
  const bySource = new Map<number, SegmentRow[]>();
  for (const s of segments) {
    if (s.kind !== 'text' || leaveOut.has(s.code_id)) continue;
    bySource.set(s.source_id, [...(bySource.get(s.source_id) ?? []), s]);
  }
  const out: Portrait[] = [];
  for (const src of sources) {
    const own = bySource.get(src.id);
    if (!own || own.length === 0 || src.characters <= 0) continue;
    const ordered = [...own].sort((a, b) => a.start - b.start || b.end - a.end);
    const drawn = ordered.slice(0, MAX_MARKS);
    const lanes = assignLanes(drawn);
    const marks: PortraitMark[] = drawn.map((s, i) => ({
      codingId: s.coding_id,
      codeId: s.code_id,
      codeName: nameOf.get(s.code_id) ?? s.code_name,
      color: colorOf.get(s.code_id) ?? '#808080',
      start: s.start,
      end: s.end,
      text: s.text,
      lane: lanes[i],
    }));
    const perCode = new Map<number, SegmentRow[]>();
    for (const s of ordered) perCode.set(s.code_id, [...(perCode.get(s.code_id) ?? []), s]);
    const uses: CodeUse[] = [...perCode.entries()]
      .map(([codeId, list]) => ({
        codeId,
        name: nameOf.get(codeId) ?? list[0].code_name,
        color: colorOf.get(codeId) ?? '#808080',
        passages: list.length,
        characters: unionLength(list),
        from: share(Math.min(...list.map((s) => s.start)), src.characters),
        to: share(Math.max(...list.map((s) => s.end)), src.characters),
      }))
      .sort((a, b) => a.from - b.from || compareText(a.name, b.name));
    out.push({
      sourceId: src.id,
      name: src.name,
      characters: src.characters,
      marks,
      lanes: Math.max(1, ...lanes.map((l) => l + 1)),
      coverage: share(unionLength(ordered), src.characters),
      uses,
      notDrawn: ordered.length - drawn.length,
    });
  }
  return out;
}
