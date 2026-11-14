// Small pure helpers for the pages of a kept PDF: how large a page is on screen, the words that were read from a page, and which pages
// have coded areas. Nothing here draws or calls the engine.

import { sliceScalar, type OffsetIndex } from '../offsets';
import type { CodingRow, FormatRun, PdfPageSize } from '../types';

/** A page is drawn at its own size in points; on screen one point is 96/72 of a pixel, the size a PDF reader shows it at 100%. */
export const PIXELS_PER_POINT = 96 / 72;

/** The size of a page on screen at 100%, in pixels. */
export function pagePixels(size: Pick<PdfPageSize, 'width' | 'height'>): { width: number; height: number } {
  return { width: Math.max(1, Math.round(size.width * PIXELS_PER_POINT)), height: Math.max(1, Math.round(size.height * PIXELS_PER_POINT)) };
}

/** A page number kept inside the PDF (1 to `count`); anything that is not a number is the first page. */
export function clampPage(page: number, count: number): number {
  if (!Number.isFinite(page)) return 1;
  return Math.min(Math.max(1, Math.round(page)), Math.max(1, count));
}

/**
 * The words read from page `page` of a PDF: the stretch of the document's text the importer marked as that page. A page the importer
 * found no words on (a scan, or a blank page) gives an empty text.
 */
export function pageWords(runs: readonly FormatRun[], text: string, index: OffsetIndex, page: number): string {
  const run = runs.find((r) => r.kind === 'page' && r.page === page);
  if (!run) return '';
  return sliceScalar(text, index, run.start, run.end).trim();
}

/** The pages that have at least one coded area, in order, with how many areas each has. */
export function pagesWithAreas(codings: readonly Pick<CodingRow, 'region' | 'page'>[]): { page: number; areas: number }[] {
  const count = new Map<number, number>();
  for (const c of codings) {
    if (!c.region) continue;
    const p = c.page ?? 1;
    count.set(p, (count.get(p) ?? 0) + 1);
  }
  return [...count.entries()].sort((a, b) => a[0] - b[0]).map(([page, areas]) => ({ page, areas }));
}

/** The first characters of some words, cut at a word, with an ellipsis when something was left out. */
export function brief(words: string, max: number): string {
  const one = words.replace(/\s+/g, ' ').trim();
  if (one.length <= max) return one;
  const cut = one.slice(0, max);
  const space = cut.lastIndexOf(' ');
  return `${(space > max * 0.6 ? cut.slice(0, space) : cut).trimEnd()}…`;
}
