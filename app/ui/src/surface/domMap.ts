// The document text as DOM, and the map between DOM positions and the engine's scalar offsets.
//
// The text is drawn as a row of <span class="seg"> elements, each holding one text node, with no
// other elements between them. That keeps the mapping exact: every character in the DOM is one
// character of the source, in order. (CSS white-space: pre-wrap keeps spaces and line breaks.)

import { OffsetIndex } from '../offsets';
import type { Segment } from '../lib/segments';
import type { UnitRow } from '../types';

export interface SurfaceMap {
  els: HTMLElement[];
  /** Scalar offset where each piece starts, then the total length. */
  startScalar: number[];
  /** UTF-16 offset where each piece starts, then the total length. */
  startUtf16: number[];
  index: OffsetIndex;
  segments: Segment[];
}

export interface BuildOptions {
  text: string;
  index: OffsetIndex;
  segments: Segment[];
  units: readonly UnitRow[];
  showUnits: boolean;
  /** Ids of codings that cover each piece are written to data-codings for tests and styling. */
  selectedCodingId: number | null;
  selectedRange: { start: number; end: number } | null;
  /** The CSS background for words covered by these codings (a highlighter color per code), or '' for none. */
  highlight?: (codingIds: readonly number[]) => string;
}

export function buildSurfaceDom(root: HTMLElement, o: BuildOptions): SurfaceMap {
  const frag = document.createDocumentFragment();
  const els: HTMLElement[] = [];
  const startScalar: number[] = [];
  const startUtf16: number[] = [];
  const { text, index, segments } = o;
  for (let i = 0; i < segments.length; i++) {
    const s = segments[i];
    const u0 = index.toUtf16(s.start);
    const u1 = index.toUtf16(s.end);
    const el = document.createElement('span');
    el.className = segmentClass(s, o);
    el.dataset.i = String(i);
    el.dataset.s = String(s.start);
    if (s.codings.length > 0) {
      el.dataset.n = String(s.codings.length);
      const background = o.highlight?.(s.codings);
      if (background) el.style.background = background;
    }
    if (o.showUnits && s.unitStart) {
      const unit = o.units[s.unit];
      if (unit) {
        el.dataset.unit = String(s.unit + 1);
        if (unit.speaker) el.dataset.speaker = unit.speaker;
        if (unit.excluded) el.dataset.excluded = 'true';
      }
    }
    el.appendChild(document.createTextNode(text.slice(u0, u1)));
    frag.appendChild(el);
    els.push(el);
    startScalar.push(s.start);
    startUtf16.push(u0);
  }
  startScalar.push(index.scalarLength);
  startUtf16.push(index.utf16Length);
  root.replaceChildren(frag);
  return { els, startScalar, startUtf16, index, segments };
}

function segmentClass(s: Segment, o: BuildOptions): string {
  const c = ['seg'];
  if (s.codings.length > 0) c.push('coded');
  if (s.codings.length > 1) c.push('multi');
  if (s.heading > 0) c.push(`h${s.heading}`);
  if (s.label) c.push('speaker');
  if (s.bold) c.push('b');
  if (s.italic) c.push('i');
  if (s.listItem) c.push('li');
  if (s.tableCell) c.push('td');
  if (o.showUnits && s.unit >= 0) c.push('in-unit');
  if (o.showUnits && s.unitStart) c.push('unit-start');
  if (o.selectedCodingId !== null && s.codings.includes(o.selectedCodingId)) c.push('chosen');
  return c.join(' ');
}

/** The piece that holds a scalar offset: the last one that starts at or before it. */
export function segmentAt(map: SurfaceMap, scalar: number): number {
  const a = map.startScalar;
  let lo = 0;
  let hi = map.els.length - 1;
  let found = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    if (a[mid] <= scalar) {
      found = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return found;
}

/** A DOM position (the way a selection reports it) as a scalar offset, or null if outside the text. */
export function pointToScalar(map: SurfaceMap, root: HTMLElement, node: Node | null, offset: number): number | null {
  if (!node || map.els.length === 0) return node === root ? 0 : null;
  if (node === root) {
    if (offset <= 0) return 0;
    if (offset >= map.els.length) return map.index.scalarLength;
    return map.startScalar[offset];
  }
  if (!root.contains(node)) return null;
  let el: Node | null = node.nodeType === Node.TEXT_NODE ? node.parentNode : node;
  while (el && el !== root && !(el instanceof HTMLElement && el.dataset.i !== undefined)) el = el.parentNode;
  if (!el || el === root) return null;
  const i = Number((el as HTMLElement).dataset.i);
  const base = map.startUtf16[i];
  const next = map.startUtf16[i + 1];
  let utf16: number;
  if (node.nodeType === Node.TEXT_NODE) utf16 = Math.min(base + offset, next);
  else utf16 = offset <= 0 ? base : next;
  return map.index.toScalar(utf16);
}

export interface DomPoint {
  node: Text;
  offset: number;
}

/**
 * A DOM position for a scalar offset. At the boundary between two pieces, `side` picks which piece
 * the position belongs to: 'start' for the beginning of a selection, 'end' for its end.
 */
export function scalarToPoint(map: SurfaceMap, scalar: number, side: 'start' | 'end'): DomPoint | null {
  if (map.els.length === 0) return null;
  const clamped = Math.min(Math.max(scalar, 0), map.index.scalarLength);
  let i = segmentAt(map, clamped);
  if (side === 'end' && clamped === map.startScalar[i] && i > 0) i--;
  const u = map.index.toUtf16(clamped);
  const node = map.els[i].firstChild as Text | null;
  if (!node) return null;
  const offset = Math.min(Math.max(u - map.startUtf16[i], 0), node.length);
  return { node, offset };
}

export interface ScalarSelection {
  start: number;
  end: number;
  anchor: number;
  focus: number;
}

/** The current selection as scalar offsets, if it lies inside the text. */
export function readSelection(map: SurfaceMap, root: HTMLElement): ScalarSelection | null {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || !sel.anchorNode || !sel.focusNode) return null;
  const anchor = pointToScalar(map, root, sel.anchorNode, sel.anchorOffset);
  const focus = pointToScalar(map, root, sel.focusNode, sel.focusOffset);
  if (anchor === null || focus === null) return null;
  return { start: Math.min(anchor, focus), end: Math.max(anchor, focus), anchor, focus };
}

/** Puts the selection (or just the caret, when start equals end) at the given offsets. */
export function writeSelection(map: SurfaceMap, start: number, end: number): boolean {
  const a = scalarToPoint(map, start, 'start');
  const b = scalarToPoint(map, end, start === end ? 'start' : 'end');
  const sel = window.getSelection();
  if (!a || !b || !sel) return false;
  sel.setBaseAndExtent(a.node, a.offset, b.node, b.offset);
  return true;
}

/** A DOM range over scalar offsets, for measuring where text sits on screen. */
export function rangeFor(map: SurfaceMap, start: number, end: number): Range | null {
  const a = scalarToPoint(map, start, 'start');
  const b = scalarToPoint(map, end, 'end');
  if (!a || !b) return null;
  const r = document.createRange();
  r.setStart(a.node, a.offset);
  r.setEnd(b.node, b.offset);
  return r;
}
