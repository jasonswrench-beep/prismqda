// Retrieved Segments: the passages coded with the codes the person chose (every code when none is chosen), in every
// document or only the one that is open, narrowed by case and by a search. Kept apart from the screen so the rules
// can be tested.

import type { CodeRow, RegionShape, SegmentsArgs } from '../types';
import { compareText, num, t, tn } from '../i18n';

export type RetrievalMode = 'all' | 'open';

/** The most segments asked for at once; the pane says when there are more. */
export const MAX_PASSAGES = 500;

/** Adds the id if it is not in the list, removes it if it is. */
export function toggleId(ids: readonly number[], id: number): number[] {
  return ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
}

/** Drops chosen codes that are no longer active (retired, merged away, or gone). */
export function pruneActive(ids: readonly number[], codes: readonly Pick<CodeRow, 'id' | 'status'>[]): number[] {
  const live = new Set(codes.filter((c) => c.status === 'active').map((c) => c.id));
  return ids.filter((id) => live.has(id));
}

/**
 * The request for the segments. With no code chosen it asks for every coding, so the table is never empty just because
 * nothing was chosen. A parent code brings its sub-codes with it, as it does everywhere else.
 */
export function passagesRequest(codeIds: readonly number[], mode: RetrievalMode, openSourceId: number | null): SegmentsArgs {
  const args: SegmentsArgs = { limit: MAX_PASSAGES, workingSet: true };
  if (codeIds.length > 0) args.code_ids = [...codeIds];
  if (mode === 'open') {
    // "The open document" with no document open matches nothing, which the pane explains.
    args.source_ids = openSourceId === null ? [] : [openSourceId];
  }
  return args;
}

/** The short text on the ribbon's scope chip. */
export function scopeChip(codeCount: number, mode: RetrievalMode): string {
  if (codeCount === 0 && mode === 'all') return t('No filter');
  if (codeCount === 0) return t('Open source');
  const codes = `${codeCount} ${codeCount === 1 ? 'code' : 'codes'}`;
  return mode === 'open' ? t('{codes}, open source', { codes }) : codes;
}

/** What narrows the table, in few words, for the status bar; null when nothing does. */
export function describeFilters(f: { codeCount: number; mode: RetrievalMode; caseName: string | null; query: string }): string | null {
  const parts: string[] = [];
  if (f.codeCount > 0) parts.push(`${f.codeCount} ${f.codeCount === 1 ? 'code' : 'codes'}`);
  if (f.mode === 'open') parts.push(t('open document'));
  if (f.caseName) parts.push(t('case {caseName}', { caseName: f.caseName }));
  if (f.query.trim() !== '') parts.push(t('search "{query}"', { query: f.query.trim() }));
  return parts.length === 0 ? null : parts.join(', ');
}

/** One sentence that says what the table is showing and how much of it. */
export function describePassages(args: { total: number; shown: number; codeNames: readonly string[]; mode: RetrievalMode; sourceName: string | null }): string {
    const names = args.codeNames.length === 0 ? t('with any code') : args.codeNames.length <= 3 ? t('coded {codes}', { codes: args.codeNames.join(', ') }) : t('coded {codes} and {more} more', { codes: args.codeNames.slice(0, 3).join(', '), more: args.codeNames.length - 3 });
  const where = args.mode === 'open' ? (args.sourceName ? t('in {name}', { name: args.sourceName }) : t('in the open document')) : t('in every document');
  const segments = tn(args.total, { one: '{count} segment', other: '{count} segments' });
  return args.total > args.shown
    ? t('{segments} {names}, {where}. The first {shown} are listed.', { segments, names, where, shown: num(args.shown) })
    : t('{segments} {names}, {where}.', { segments, names, where });
}

// ---------------------------------------------------------------- the rows of the table

export interface ResultRow {
  coding_id: number;
  source_id: number;
  source_name: string;
  code_id: number;
  code_name: string;
  /** The coded words. */
  text: string;
  /** The note on the coding, for the Notes column. */
  notes: string;
  /** The cases the document belongs to, joined, for the Case column. */
  case_name: string;
  start: number;
  end: number;
  /** The area, when the row is of a part of a picture (then `text` tells it in words). */
  region?: RegionShape | null;
  /** The page of the PDF the area is on, when the row is of an area of a PDF page. */
  page?: number | null;
}

export type SortKey = 'source' | 'code' | 'case' | 'excerpt' | 'notes';
export type SortDirection = 'ascending' | 'descending';

/** Keeps the rows that contain every typed word in the document, code, case, excerpt, or notes (case does not matter). */
export function searchRows<T extends ResultRow>(rows: readonly T[], query: string): T[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...rows];
  return rows.filter((r) => {
    const hay = `${r.source_name} ${r.code_name} ${r.case_name} ${r.text} ${r.notes}`.toLowerCase();
    return words.every((w) => hay.includes(w));
  });
}

function compareNames(a: string, b: string): number {
  return compareText(a, b, { numeric: true, sensitivity: 'base' });
}

/**
 * The rows in the order of a column. Rows that tie keep their order in the document, so the table never shuffles.
 * With no column chosen the rows are in document order: by document name, then by position in it.
 */
export function sortRows<T extends ResultRow>(rows: readonly T[], key: SortKey | null, direction: SortDirection): T[] {
  const value = (r: T): string => {
    switch (key) {
      case 'source':
        return r.source_name;
      case 'code':
        return r.code_name;
      case 'case':
        return r.case_name;
      case 'excerpt':
        return r.text;
      case 'notes':
        return r.notes;
      default:
        return '';
    }
  };
  const flip = direction === 'descending' ? -1 : 1;
  // Stretches of one recording have no words to be in the order of, so they are in the order of their times.
  const moment = (r: T): number => (r.region && r.region.shape === 'span' ? r.region.start_ms : 0);
  return [...rows].sort((a, b) => {
    if (key !== null) {
      const d = compareNames(value(a), value(b));
      if (d !== 0) return d * flip;
    }
    return compareNames(a.source_name, b.source_name) || a.start - b.start || moment(a) - moment(b) || a.coding_id - b.coding_id;
  });
}

/** A shortened excerpt with an ellipsis, cut at a space where there is one. The full words are in Segment Details. */
export function previewText(text: string, max = 80): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  const chars = Array.from(flat);
  if (chars.length <= max) return flat;
  const cut = chars.slice(0, max).join('');
  const space = cut.lastIndexOf(' ');
  return `${(space > max / 2 ? cut.slice(0, space) : cut).trimEnd()}…`;
}
