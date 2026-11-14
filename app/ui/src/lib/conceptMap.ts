// What a concept map needs that is not a screen: where its drawing's edges are, where a link meets a node, how to say a link in
// words, and how a key moves a node.
import { t } from '../i18n';

export const NODE_W = 176;
export const NODE_H = 48;
const PAD = 70;

export interface Point {
  x: number;
  y: number;
}

/** The part of the map's own space to draw: every node with room around it, and never smaller than a usable canvas. */
export function viewBoxFor(centers: readonly Point[]): { x: number; y: number; w: number; h: number } {
  if (centers.length === 0) return { x: -300, y: -180, w: 600, h: 360 };
  const xs = centers.map((p) => p.x);
  const ys = centers.map((p) => p.y);
  const minX = Math.min(...xs) - NODE_W / 2 - PAD;
  const maxX = Math.max(...xs) + NODE_W / 2 + PAD;
  const minY = Math.min(...ys) - NODE_H / 2 - PAD;
  const maxY = Math.max(...ys) + NODE_H / 2 + PAD;
  const w = Math.max(600, maxX - minX);
  const h = Math.max(360, maxY - minY);
  // A small map is centered in the minimum canvas.
  return { x: minX - (w - (maxX - minX)) / 2, y: minY - (h - (maxY - minY)) / 2, w, h };
}

/** Where a line from the middle of one node toward another leaves the first node's rectangle. */
export function edgePoint(from: Point, toward: Point): Point {
  const dx = toward.x - from.x;
  const dy = toward.y - from.y;
  if (dx === 0 && dy === 0) return { ...from };
  const sx = dx === 0 ? Infinity : NODE_W / 2 / Math.abs(dx);
  const sy = dy === 0 ? Infinity : NODE_H / 2 / Math.abs(dy);
  const t = Math.min(sx, sy);
  return { x: from.x + dx * t, y: from.y + dy * t };
}

export function midpoint(a: Point, b: Point): Point {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

/** A node's words shortened to fit its box; the whole words stay in its name for a screen reader and in a tooltip. */
export function shorten(text: string, max = 24): string {
  const chars = Array.from(text.replace(/\s+/g, ' ').trim());
  return chars.length > max ? `${chars.slice(0, max - 1).join('').trimEnd()}…` : chars.join('');
}

export const KIND_WORDS: Record<string, string> = { get code() { return t('code'); }, get case() { return t('case'); }, get memo() { return t('memo'); }, get note() { return t('note'); } };

export interface NamedLink {
  from: string;
  to: string;
  label: string;
  directed: boolean;
}

/** A link as a sentence: "Cost leads to Anxiety", or "Cost and Anxiety: occur together" when it has no direction. */
export function linkSentence(l: NamedLink): string {
  const words = l.label.trim();
  if (l.directed) return t('{from} {words} {to}', { from: l.from, words: words === '' ? t('is linked to') : words, to: l.to });
  return words === '' ? t('{from} and {to} are linked', { from: l.from, to: l.to }) : t('{from} and {to}: {words}', { from: l.from, to: l.to, words });
}

/** Where a node goes when an arrow key is pressed (a small step, or a large one with Shift); null for any other key. */
export function moveBy(at: Point, key: string, big: boolean): Point | null {
  const step = big ? 100 : 20;
  switch (key) {
    case 'ArrowLeft':
      return { x: at.x - step, y: at.y };
    case 'ArrowRight':
      return { x: at.x + step, y: at.y };
    case 'ArrowUp':
      return { x: at.x, y: at.y - step };
    case 'ArrowDown':
      return { x: at.x, y: at.y + step };
    default:
      return null;
  }
}

/** The map in words, for someone who does not see the drawing: each node, and what it is joined to. */
export function mapInWords(nodes: readonly { id: number; label: string; kind: string }[], links: readonly { from_node: number; to_node: number; label: string; directed: boolean }[]): { sentences: string[]; alone: string[] } {
  const name = new Map(nodes.map((n) => [n.id, n.label]));
  const joined = new Set<number>();
  const sentences = links.map((l) => {
    joined.add(l.from_node);
    joined.add(l.to_node);
    return linkSentence({ from: name.get(l.from_node) ?? '?', to: name.get(l.to_node) ?? '?', label: l.label, directed: l.directed });
  });
  return { sentences, alone: nodes.filter((n) => !joined.has(n.id)).map((n) => n.label) };
}
