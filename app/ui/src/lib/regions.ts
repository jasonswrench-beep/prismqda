// Coded areas of a picture: the shapes, where they are on the screen, how they are drawn with the pointer or the keys, and how they
// are told in words. All numbers of a shape are fractions of the picture as shown (0 is the left or top edge, 1 the right or bottom);
// this file turns them to and from screen positions and never needs the picture itself. Kept apart from the screen so it can be tested.

import type { AreaShape, CodingRow, RegionRow, RegionShape } from '../types';
import { t } from '../i18n';
import { spanShort, spanWords } from './clips';

/** The kinds of area drawn on a picture or a page (a stretch of a recording is not drawn). */
export type ShapeKind = AreaShape['shape'];
export type Point = [number, number];

/** The smallest side of a rectangle or ellipse the engine takes, as a fraction of the picture. */
export const MIN_SIDE = 0.002;
/** The most corners the engine takes for a polygon. */
export const MAX_POINTS = 500;

export const SHAPE_LABELS: Record<ShapeKind, string> = { get rectangle() { return t('Rectangle'); }, get ellipse() { return t('Ellipse'); }, get polygon() { return t('Outline'); } };

const clamp01 = (v: number): number => Math.min(1, Math.max(0, v));

/** A number rounded as the engine rounds it (five decimals), so a shape compares equal before and after it is stored. */
export function round5(v: number): number {
  return Math.round(v * 100_000) / 100_000;
}

/** A shape with every number kept inside the picture and rounded. A rectangle or ellipse is cut to fit; a polygon's corners are moved inside. */
export function fitShape(s: AreaShape): AreaShape;
export function fitShape(s: RegionShape): RegionShape;
export function fitShape(s: RegionShape): RegionShape {
  // A stretch of a recording is whole milliseconds, not before the start.
  if (s.shape === 'span') return { shape: 'span', start_ms: Math.max(0, Math.round(s.start_ms)), end_ms: Math.max(0, Math.round(s.end_ms)) };
  if (s.shape === 'polygon') return { shape: 'polygon', points: s.points.map(([x, y]) => [round5(clamp01(x)), round5(clamp01(y))] as Point) };
  const x = round5(clamp01(s.x));
  const y = round5(clamp01(s.y));
  return { shape: s.shape, x, y, w: round5(Math.min(s.w, 1 - x)), h: round5(Math.min(s.h, 1 - y)) };
}

/** The box around a shape: left, top, right, bottom. */
export function boundsOf(s: AreaShape): [number, number, number, number] {
  if (s.shape === 'polygon') {
    let [x0, y0, x1, y1] = [1, 1, 0, 0];
    for (const [x, y] of s.points) {
      x0 = Math.min(x0, x);
      y0 = Math.min(y0, y);
      x1 = Math.max(x1, x);
      y1 = Math.max(y1, y);
    }
    return [x0, y0, x1, y1];
  }
  return [s.x, s.y, s.x + s.w, s.y + s.h];
}

/** The area enclosed, as a fraction of the picture. */
export function areaOf(s: AreaShape): number {
  if (s.shape === 'rectangle') return s.w * s.h;
  if (s.shape === 'ellipse') return (Math.PI * s.w * s.h) / 4;
  let sum = 0;
  for (let i = 0; i < s.points.length; i++) {
    const [ax, ay] = s.points[i];
    const [bx, by] = s.points[(i + 1) % s.points.length];
    sum += ax * by - bx * ay;
  }
  return Math.abs(sum) / 2;
}

/** Whether a shape could be stored: big enough, and a polygon with enough corners enclosing some ground. */
export function isCodable(s: AreaShape): boolean {
  if (s.shape === 'polygon') {
    const [x0, y0, x1, y1] = boundsOf(s);
    return s.points.length >= 3 && s.points.length <= MAX_POINTS && x1 - x0 >= MIN_SIDE && y1 - y0 >= MIN_SIDE && areaOf(s) >= 1e-6;
  }
  return s.w >= MIN_SIDE && s.h >= MIN_SIDE;
}

/** True when two shapes are the same shape (as the engine counts a shape as one excerpt). */
export function sameShape(a: RegionShape, b: RegionShape): boolean {
  return JSON.stringify(fitShape(a)) === JSON.stringify(fitShape(b));
}

const pct = (v: number): number => Math.round(v * 100);

/** The shape in words, as the engine says it: what it is and where it is on the picture (or, with `on` set to `page`, on the page of a PDF). */
export function describeShape(s: RegionShape, on: 'picture' | 'page' = 'picture'): string {
  if (s.shape === 'span') return spanWords(s.start_ms, s.end_ms);
  const [x0, y0, x1, y1] = boundsOf(s);
  const kind = s.shape === 'rectangle' ? t('Rectangle') : s.shape === 'ellipse' ? t('Ellipse') : t('Outline of {count} points', { count: s.points.length });
  const share = areaOf(s) > 0 ? Math.max(1, pct(areaOf(s))) : 0;
  const numbers = { kind, pct: pct(x0), pct2: pct(x1), pct3: pct(y0), pct4: pct(y1), share };
  return on === 'page'
    ? t('{kind}, {pct}% to {pct2}% across and {pct3}% to {pct4}% down, {share}% of the page', numbers)
    : t('{kind}, {pct}% to {pct2}% across and {pct3}% to {pct4}% down, {share}% of the picture', numbers);
}

/** The same, shorter, for a label that sits on a drawing. */
export function shortShape(s: RegionShape): string {
  if (s.shape === 'span') return spanShort(s.start_ms, s.end_ms);
  const [x0, y0, x1, y1] = boundsOf(s);
  return t('{value} at {pct}% across, {pct2}% down', { value: SHAPE_LABELS[s.shape].toLowerCase(), pct: pct((x0 + x1) / 2), pct2: pct((y0 + y1) / 2) });
}

// ---------------------------------------------------------------- rows

/** A coded area as a coding, so the lists and panels that show codings show it too. Its number is the area's number made negative. */
export function regionToCoding(r: RegionRow): CodingRow {
  const { id, source_id, code_id, coder_id, origin, layer, rationale, rating, created_at } = r;
  let region: RegionShape;
  if (r.shape === 'span') region = { shape: 'span', start_ms: r.start_ms, end_ms: r.end_ms };
  else if (r.shape === 'polygon') region = { shape: 'polygon', points: r.points };
  else region = { shape: r.shape, x: r.x, y: r.y, w: r.w, h: r.h };
  return { id: -id, source_id, start: 0, end: 0, code_id, coder_id, origin, layer, rationale, unit_id: null, created_at, rating, region, page: r.page ?? 1 };
}

/** True for a coding that is of an area of a picture. */
export function isRegionCoding(c: Pick<CodingRow, 'id'> & { region?: RegionShape }): boolean {
  return c.region !== undefined || c.id < 0;
}

// ---------------------------------------------------------------- the picture on the screen

/** How the picture sits in its window: the size it is drawn at, and how it is turned. Turning is in quarter turns, clockwise. */
export interface View {
  /** Pixels of the window on screen. */
  frameW: number;
  frameH: number;
  /** The picture's own size in pixels, as shown (unturned). */
  imageW: number;
  imageH: number;
  /** Quarter turns clockwise: 0, 1, 2, or 3. */
  turns: number;
  /** 1 draws one picture pixel as one screen pixel. */
  zoom: number;
}

/** The size of the picture on screen after the turn, at the zoom. */
export function shownSize(v: Pick<View, 'imageW' | 'imageH' | 'turns' | 'zoom'>): { w: number; h: number } {
  const sideways = v.turns % 2 === 1;
  const [w, h] = sideways ? [v.imageH, v.imageW] : [v.imageW, v.imageH];
  return { w: w * v.zoom, h: h * v.zoom };
}

/** The zoom at which the whole picture (turned as it is) just fits the window, never above 1 (a small picture is not blown up). */
export function fitZoom(v: Pick<View, 'frameW' | 'frameH' | 'imageW' | 'imageH' | 'turns'>): number {
  const { w, h } = shownSize({ ...v, zoom: 1 });
  if (w <= 0 || h <= 0 || v.frameW <= 0 || v.frameH <= 0) return 1;
  return Math.min(1, v.frameW / w, v.frameH / h);
}

export const ZOOM_MIN = 0.05;
export const ZOOM_MAX = 8;
/** The steps the zoom buttons and keys move through. */
export const ZOOM_STEPS = [0.05, 0.1, 0.15, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4, 6, 8];

/** The next step up or down from a zoom. */
export function stepZoom(zoom: number, direction: 1 | -1): number {
  if (direction === 1) return ZOOM_STEPS.find((s) => s > zoom + 1e-9) ?? ZOOM_MAX;
  for (let i = ZOOM_STEPS.length - 1; i >= 0; i--) if (ZOOM_STEPS[i] < zoom - 1e-9) return ZOOM_STEPS[i];
  return ZOOM_MIN;
}

/** A point of the picture (fractions) to a position in the turned picture (fractions of its shown width and height). */
export function turnPoint([x, y]: Point, turns: number): Point {
  switch (((turns % 4) + 4) % 4) {
    case 1:
      return [1 - y, x];
    case 2:
      return [1 - x, 1 - y];
    case 3:
      return [y, 1 - x];
    default:
      return [x, y];
  }
}

/** The inverse of {@link turnPoint}: a position in the turned picture back to a point of the picture. */
export function unturnPoint([u, v]: Point, turns: number): Point {
  switch (((turns % 4) + 4) % 4) {
    case 1:
      return [v, 1 - u];
    case 2:
      return [1 - u, 1 - v];
    case 3:
      return [1 - v, u];
    default:
      return [u, v];
  }
}

/**
 * A pointer position inside the shown picture (pixels from its top left corner, as drawn, turned) to a point of the picture. The
 * point is cut to lie on the picture, so dragging beyond an edge stops at the edge.
 */
export function pointFromScreen(px: number, py: number, shown: { w: number; h: number }, turns: number): Point {
  if (shown.w <= 0 || shown.h <= 0) return [0, 0];
  const p = unturnPoint([clamp01(px / shown.w), clamp01(py / shown.h)], turns);
  return [clamp01(p[0]), clamp01(p[1])];
}

// ---------------------------------------------------------------- drawing

/** The rectangle or ellipse with these two opposite corners. */
export function boxFrom(kind: 'rectangle' | 'ellipse', a: Point, b: Point): AreaShape {
  const x = Math.min(a[0], b[0]);
  const y = Math.min(a[1], b[1]);
  return { shape: kind, x, y, w: Math.abs(a[0] - b[0]), h: Math.abs(a[1] - b[1]) };
}

/** The point on a polyline farthest from the line through its ends, for simplifying a freehand line. */
function farthest(points: Point[], from: number, to: number): { index: number; distance: number } {
  const [ax, ay] = points[from];
  const [bx, by] = points[to];
  const dx = bx - ax;
  const dy = by - ay;
  const length = Math.hypot(dx, dy);
  let best = { index: from, distance: 0 };
  for (let i = from + 1; i < to; i++) {
    const [px, py] = points[i];
    const d = length === 0 ? Math.hypot(px - ax, py - ay) : Math.abs(dy * px - dx * py + bx * ay - by * ax) / length;
    if (d > best.distance) best = { index: i, distance: d };
  }
  return best;
}

/** Fewer points that follow the same line to within `tolerance` (Ramer, Douglas, and Peucker), for a line drawn freehand. */
export function simplify(points: readonly Point[], tolerance: number): Point[] {
  if (points.length <= 2) return points.map((p) => [...p] as Point);
  const keep = new Array<boolean>(points.length).fill(false);
  keep[0] = true;
  keep[points.length - 1] = true;
  const stack: [number, number][] = [[0, points.length - 1]];
  while (stack.length > 0) {
    const [from, to] = stack.pop()!;
    const { index, distance } = farthest(points as Point[], from, to);
    if (distance > tolerance) {
      keep[index] = true;
      stack.push([from, index], [index, to]);
    }
  }
  return points.filter((_, i) => keep[i]).map((p) => [...p] as Point);
}

/** A freehand line, closed into an outline: simplified, and thinned further if it still has too many corners. */
export function outlineFromStroke(points: readonly Point[]): AreaShape | null {
  let tolerance = 0.004;
  let out = simplify(points, tolerance);
  while (out.length > 200 && tolerance < 0.1) {
    tolerance *= 1.6;
    out = simplify(points, tolerance);
  }
  // A closing point that repeats the first is dropped.
  if (out.length > 3) {
    const [fx, fy] = out[0];
    const [lx, ly] = out[out.length - 1];
    if (Math.hypot(fx - lx, fy - ly) < 0.002) out.pop();
  }
  const shape: AreaShape = fitShape({ shape: 'polygon', points: out });
  return isCodable(shape) ? shape : null;
}

// ---------------------------------------------------------------- the keys

/** How far one press of an arrow key moves, as a fraction of the picture: a small step, or a large one with Shift. */
export function keyStep(big: boolean): number {
  return big ? 0.05 : 0.01;
}

/** A shape turned with the picture: in quarter turns clockwise, so a shape can be changed as it appears on the screen. */
export function turnShape(s: AreaShape, turns: number): AreaShape {
  const k = ((turns % 4) + 4) % 4;
  if (k === 0) return s;
  if (s.shape === 'polygon') return { shape: 'polygon', points: s.points.map((p) => turnPoint(p, k).map(round5) as Point) };
  const [x0, y0, x1, y1] = boundsOf(s);
  const a = turnPoint([x0, y0], k);
  const b = turnPoint([x1, y1], k);
  return { shape: s.shape, x: round5(Math.min(a[0], b[0])), y: round5(Math.min(a[1], b[1])), w: round5(Math.abs(a[0] - b[0])), h: round5(Math.abs(a[1] - b[1])) };
}

/** The shape moved by `dx` and `dy` (fractions), kept on the picture: it stops at an edge instead of shrinking. */
export function translateShape(s: AreaShape, dx: number, dy: number): AreaShape {
  const [x0, y0, x1, y1] = boundsOf(s);
  const mx = Math.min(Math.max(dx, -x0), 1 - x1);
  const my = Math.min(Math.max(dy, -y0), 1 - y1);
  if (s.shape === 'polygon') return { shape: 'polygon', points: s.points.map(([x, y]) => [round5(x + mx), round5(y + my)] as Point) };
  return { shape: s.shape, x: round5(s.x + mx), y: round5(s.y + my), w: s.w, h: s.h };
}

export type Corner = 'nw' | 'ne' | 'se' | 'sw';

/** The corner of a rectangle's or ellipse's box, as a point. */
export function cornerOf(s: AreaShape, corner: Corner): Point {
  const [x0, y0, x1, y1] = boundsOf(s);
  return [corner === 'nw' || corner === 'sw' ? x0 : x1, corner === 'nw' || corner === 'ne' ? y0 : y1];
}

/** The box with the corner opposite `corner` held still and `corner` moved to `p`, whichever way round it ends up. */
export function dragCorner(s: AreaShape, corner: Corner, p: Point): AreaShape {
  const opposite: Record<Corner, Corner> = { nw: 'se', ne: 'sw', se: 'nw', sw: 'ne' };
  const still = cornerOf(s, opposite[corner]);
  return boxFrom(s.shape === 'ellipse' ? 'ellipse' : 'rectangle', still, p);
}

/** The point moved by one press of an arrow key as the picture is shown (`dx` and `dy` are -1, 0, or 1), kept on the picture. */
export function nudgePoint(p: Point, dx: number, dy: number, big: boolean, turns: number): Point {
  const step = keyStep(big);
  const [u, v] = turnPoint(p, turns);
  return unturnPoint([round5(clamp01(u + dx * step)), round5(clamp01(v + dy * step))], turns).map(round5) as Point;
}

/** The shape moved by one press of an arrow key as the picture is shown. It stops at the edge of the picture instead of shrinking. */
export function moveShape(s: AreaShape, dx: number, dy: number, big: boolean, turns: number): AreaShape {
  const step = keyStep(big);
  const moved = translateShape(turnShape(s, turns), dx * step, dy * step);
  return turnShape(moved, 4 - (((turns % 4) + 4) % 4));
}

/**
 * The shape made wider or narrower, taller or shorter by one press of an arrow key as the picture is shown: Right and Down grow it, Left
 * and Up shrink it, from its top left corner. An outline is scaled the same way.
 */
export function resizeShape(s: AreaShape, dx: number, dy: number, big: boolean, turns: number): AreaShape {
  const step = keyStep(big);
  const shown = turnShape(s, turns);
  const [x0, y0, x1, y1] = boundsOf(shown);
  const w = Math.min(Math.max(x1 - x0 + dx * step, MIN_SIDE), 1 - x0);
  const h = Math.min(Math.max(y1 - y0 + dy * step, MIN_SIDE), 1 - y0);
  let resized: AreaShape;
  if (shown.shape === 'polygon') {
    const sx = x1 - x0 === 0 ? 1 : w / (x1 - x0);
    const sy = y1 - y0 === 0 ? 1 : h / (y1 - y0);
    resized = { shape: 'polygon', points: shown.points.map(([x, y]) => [round5(x0 + (x - x0) * sx), round5(y0 + (y - y0) * sy)] as Point) };
  } else {
    resized = { shape: shown.shape, x: shown.x, y: shown.y, w: round5(w), h: round5(h) };
  }
  return turnShape(resized, 4 - (((turns % 4) + 4) % 4));
}

/** A new rectangle or ellipse the size of a fifth of the picture, centred on a point (or as near as the picture allows), to start from with the keys. */
export function startBox(kind: 'rectangle' | 'ellipse', centre: Point): AreaShape {
  const w = 0.2;
  const h = 0.2;
  const x = Math.min(Math.max(centre[0] - w / 2, 0), 1 - w);
  const y = Math.min(Math.max(centre[1] - h / 2, 0), 1 - h);
  return { shape: kind, x: round5(x), y: round5(y), w, h };
}

// ---------------------------------------------------------------- drawing it

/** The points attribute of an SVG polygon for a shape's outline, in picture fractions scaled to `w` by `h`. */
export function polygonPoints(points: readonly Point[], w: number, h: number): string {
  return points.map(([x, y]) => `${round5(x * w)},${round5(y * h)}`).join(' ');
}

/**
 * Where to put a thumbnail of a shape: the picture is drawn `scale` times as large as the box, moved so the shape's own box (with
 * a margin) fills the thumbnail. All in fractions of the thumbnail's width and height, for a picture of `imageW` by `imageH`.
 */
export function thumbPlacement(s: AreaShape, imageW: number, imageH: number, boxW: number, boxH: number): { width: number; height: number; left: number; top: number } {
  const [x0, y0, x1, y1] = boundsOf(s);
  const margin = 0.08;
  const bw = Math.max((x1 - x0) * imageW, 1) * (1 + margin * 2);
  const bh = Math.max((y1 - y0) * imageH, 1) * (1 + margin * 2);
  const scale = Math.min(boxW / bw, boxH / bh);
  const width = imageW * scale;
  const height = imageH * scale;
  const cx = ((x0 + x1) / 2) * width;
  const cy = ((y0 + y1) / 2) * height;
  return { width, height, left: boxW / 2 - cx, top: boxH / 2 - cy };
}
