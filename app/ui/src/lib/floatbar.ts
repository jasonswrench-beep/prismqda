// The floating coding bar: the small row of buttons that appears above words selected with the mouse.
// Where it goes and what is on it are decided here, with no browser needed, so they can be tested.

import type { Favorites } from './favorites';
import { compareCodes } from './tree';

/** A rectangle, in the same coordinates as the other boxes it is compared with. */
export interface Box {
  top: number;
  left: number;
  right: number;
  bottom: number;
}

export interface Size {
  width: number;
  height: number;
}

/** The part of the text area a person can see, and how wide it is, in the same coordinates as the boxes. */
export interface View {
  top: number;
  bottom: number;
  width: number;
}

export interface Placement {
  top: number;
  left: number;
  /** True when there was no room above the words, so the bar sits under them. */
  below: boolean;
}

const GAP = 8;
const PAD = 6;

/**
 * Puts the bar above the first line of the selection, centered on it. If there is no room above inside what
 * is visible, it goes under the last line instead. It never leaves the visible area or the sides of the text.
 */
export function placeBar(first: Box, last: Box, bar: Size, view: View): Placement {
  const aboveTop = first.top - GAP - bar.height;
  const below = aboveTop < view.top + PAD;
  let top = below ? last.bottom + GAP : aboveTop;
  // A selection that fills the whole view leaves no room on either side: keep the bar on screen anyway.
  if (top + bar.height > view.bottom - PAD) top = view.bottom - PAD - bar.height;
  if (top < view.top + PAD) top = view.top + PAD;
  const anchor = below ? last : first;
  const centre = (anchor.left + anchor.right) / 2;
  const maxLeft = Math.max(PAD, view.width - bar.width - PAD);
  const left = Math.min(Math.max(centre - bar.width / 2, PAD), maxLeft);
  return { top, left, below };
}

/** The first and last lines of a selection, from the rectangles the browser reports for it. */
export function edgeLines(rects: readonly Box[]): { first: Box; last: Box } | null {
  const real = rects.filter((r) => r.right - r.left > 0 && r.bottom - r.top > 0);
  if (real.length === 0) return null;
  let first = real[0];
  let last = real[0];
  for (const r of real) {
    if (r.top < first.top || (r.top === first.top && r.left < first.left)) first = r;
    if (r.bottom > last.bottom || (r.bottom === last.bottom && r.right > last.right)) last = r;
  }
  return { first, last };
}

/** How many favorites fit on the bar; the rest are still reached with their number keys. */
export const BAR_FAVORITES = 6;
/** How many codes the bar shows in all. With fewer favorites than this, it fills the rest from the codebook. */
export const BAR_TOTAL = 8;

export interface BarCode {
  id: number;
  name: string;
  color: string;
}

/** What the bar needs to know about a code: enough to put the codes in codebook order. */
export interface BarSource extends BarCode {
  status: string;
  parent_id: number | null;
  sort_order: number;
}

export interface BarChoices {
  /** The code used last, when it is not already one of the favorites shown. */
  last: BarCode | null;
  /** Favorites in key order, with their key number. */
  favorites: { slot: number; code: BarCode }[];
  /** Other codes with no key, in codebook order, so the bar is useful before any code is pinned as a favorite. */
  others: BarCode[];
}

/** Active codes in the order the codebook shows them: each code, then the codes under it. */
function codebookOrder(codes: readonly BarSource[]): BarSource[] {
  const ids = new Set(codes.map((c) => c.id));
  const byParent = new Map<number | null, BarSource[]>();
  for (const c of codes) {
    // A code whose parent is missing is treated as a top-level code rather than lost.
    const key = c.parent_id !== null && ids.has(c.parent_id) ? c.parent_id : null;
    const list = byParent.get(key);
    if (list) list.push(c);
    else byParent.set(key, [c]);
  }
  const out: BarSource[] = [];
  const seen = new Set<number>();
  const walk = (parent: number | null) => {
    for (const c of [...(byParent.get(parent) ?? [])].sort(compareCodes)) {
      if (seen.has(c.id)) continue;
      seen.add(c.id);
      if (c.status === 'active') out.push(c);
      walk(c.id);
    }
  };
  walk(null);
  return out;
}

/**
 * Which codes the bar offers: the last one used, the favorites with their keys, and then other codes in codebook
 * order until the bar is full. Codes that cannot be applied (retired, merged) are left out.
 */
export function barChoices(codes: readonly BarSource[], favorites: Favorites, lastCodeId: number | null): BarChoices {
  const active = new Map<number, BarCode>();
  for (const c of codes) if (c.status === 'active') active.set(c.id, { id: c.id, name: c.name, color: c.color });
  const shown: { slot: number; code: BarCode }[] = [];
  favorites.forEach((id, i) => {
    if (id === null || shown.length >= BAR_FAVORITES) return;
    const code = active.get(id);
    if (code) shown.push({ slot: i + 1, code });
  });
  const lastCode = lastCodeId === null ? null : (active.get(lastCodeId) ?? null);
  const last = lastCode && !shown.some((f) => f.code.id === lastCode.id) ? lastCode : null;
  const used = new Set<number>(shown.map((f) => f.code.id));
  if (last) used.add(last.id);
  const others: BarCode[] = [];
  for (const c of codebookOrder(codes)) {
    if (used.size + others.length >= BAR_TOTAL) break;
    if (!used.has(c.id)) others.push({ id: c.id, name: c.name, color: c.color });
  }
  return { last, favorites: shown, others };
}

/** The selection a bar was shown for, so a different selection (by keyboard, say) can put it away. */
export function sameSelection(a: { start: number; end: number } | null, b: { start: number; end: number } | null): boolean {
  return a !== null && b !== null && a.start === b.start && a.end === b.end;
}
