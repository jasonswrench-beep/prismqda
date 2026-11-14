// Colors, patterns, and short labels for codes. A code is never told apart by color alone: every
// code also has a pattern (from its color) and a short text label (from its name).

import type { CodeRow } from '../types';
import { t } from '../i18n';

/**
 * The colors a code can have. The first eight are the Okabe-Ito palette, designed to stay distinguishable for the
 * common kinds of color blindness, plus a neutral gray; the rest are there so a larger codebook is not made of
 * repeats. Each color has a plain name that is shown next to it. The order never changes, because a color's
 * pattern comes from its place in this list (see patternFor), so new colors only ever go at the end.
 */
export const PALETTE: ReadonlyArray<{ name: string; hex: string }> = [
  { get name() { return t('Orange'); }, hex: '#E69F00' },
  { get name() { return t('Sky blue'); }, hex: '#56B4E9' },
  { get name() { return t('Green'); }, hex: '#009E73' },
  { get name() { return t('Yellow'); }, hex: '#F0E442' },
  { get name() { return t('Blue'); }, hex: '#0072B2' },
  { get name() { return t('Red-orange'); }, hex: '#D55E00' },
  { get name() { return t('Pink'); }, hex: '#CC79A7' },
  { get name() { return t('Gray'); }, hex: '#999999' },
  { get name() { return t('Red'); }, hex: '#D62839' },
  { get name() { return t('Hot pink'); }, hex: '#F0388F' },
  { get name() { return t('Light pink'); }, hex: '#F7A8C8' },
  { get name() { return t('Purple'); }, hex: '#8E44AD' },
  { get name() { return t('Lavender'); }, hex: '#A99BE6' },
  { get name() { return t('Indigo'); }, hex: '#4B3FBF' },
  { get name() { return t('Navy'); }, hex: '#1F3A93' },
  { get name() { return t('Teal'); }, hex: '#1B998B' },
  { get name() { return t('Mint'); }, hex: '#7ED3A9' },
  { get name() { return t('Lime'); }, hex: '#8FBF2A' },
  { get name() { return t('Peach'); }, hex: '#FFB38A' },
  { get name() { return t('Brown'); }, hex: '#8D6E63' },
  { get name() { return t('Slate'); }, hex: '#5F7A8A' },
  { get name() { return t('Maroon'); }, hex: '#8C2F4B' },
];

/** How many different margin patterns exist (see .pattern-0 to .pattern-7 in styles.css). Colors past the eighth reuse them. */
export const PATTERN_COUNT = 8;
export const patternNames = (): string[] => [t('solid'), t('diagonal lines'), t('dots'), t('horizontal lines'), t('crosshatch'), t('vertical lines'), t('checks'), t('wide diagonal lines')];

/** The default color the engine gives a code with none chosen. */
export const ENGINE_DEFAULT_COLOR = '#808080';

function normalizeHex(hex: string): string {
  const h = hex.trim().toLowerCase();
  if (/^#[0-9a-f]{3}$/.test(h)) return '#' + h[1] + h[1] + h[2] + h[2] + h[3] + h[3];
  return /^#[0-9a-f]{6}$/.test(h) ? h : '#808080';
}

export function paletteIndex(hex: string): number {
  const h = normalizeHex(hex);
  return PALETTE.findIndex((p) => p.hex.toLowerCase() === h);
}

/** The plain name of a color: a palette name, or its hex code for a custom color. */
export function colorName(hex: string): string {
  const i = paletteIndex(hex);
  return i >= 0 ? PALETTE[i].name : normalizeHex(hex).toUpperCase();
}

/** The pattern number (0 to 7) a code's color goes with. A color outside the palette gets one from its value. */
export function patternFor(hex: string): number {
  const i = paletteIndex(hex);
  if (i >= 0) return i % PATTERN_COUNT;
  const h = normalizeHex(hex);
  let sum = 0;
  for (let k = 1; k < 7; k++) sum = (sum * 31 + h.charCodeAt(k)) >>> 0;
  return sum % PATTERN_COUNT;
}

function channel(v: number): number {
  const c = v / 255;
  return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

/** WCAG relative luminance of a #rrggbb color. */
export function luminance(hex: string): number {
  const h = normalizeHex(hex);
  const r = parseInt(h.slice(1, 3), 16);
  const g = parseInt(h.slice(3, 5), 16);
  const b = parseInt(h.slice(5, 7), 16);
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG contrast ratio between two colors, from 1 to 21. */
export function contrastRatio(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

/** How strongly a code's color shows behind its words: enough to see at a glance, not so much that the text is hard to read. */
export const HIGHLIGHT_STRENGTH = 0.32;

/** A code's color at highlighter strength, as a CSS color that lets the page show through. */
export function highlightColor(hex: string, strength = HIGHLIGHT_STRENGTH): string {
  const h = normalizeHex(hex);
  const r = parseInt(h.slice(1, 3), 16);
  const g = parseInt(h.slice(3, 5), 16);
  const b = parseInt(h.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${strength})`;
}

/**
 * The CSS background for words that carry these codes: one highlighter color for one code, and for several codes
 * bands of each color from top to bottom, so every code stays visible where they overlap.
 */
export function highlightBackground(hexes: readonly string[]): string {
  if (hexes.length === 0) return '';
  if (hexes.length === 1) return highlightColor(hexes[0]);
  const step = 100 / hexes.length;
  const bands = hexes.map((h, i) => `${highlightColor(h)} ${(i * step).toFixed(2)}% ${((i + 1) * step).toFixed(2)}%`);
  return t('linear-gradient(to bottom, {list})', { list: bands.join(', ') });
}

/** What a color becomes when it is laid over a page color at highlighter strength: the color the words are actually read against. */
export function blendOver(hex: string, page: string, strength = HIGHLIGHT_STRENGTH): string {
  const a = normalizeHex(hex);
  const p = normalizeHex(page);
  const mix = (i: number) => Math.round(parseInt(a.slice(i, i + 2), 16) * strength + parseInt(p.slice(i, i + 2), 16) * (1 - strength));
  return '#' + [1, 3, 5].map((i) => mix(i).toString(16).padStart(2, '0')).join('');
}

/** Black or white, whichever reads better on the given background. Always at least 4.5 to 1. */
export function textColorOn(background: string): '#000000' | '#ffffff' {
  return contrastRatio(background, '#000000') >= contrastRatio(background, '#ffffff') ? '#000000' : '#ffffff';
}

/** The palette color used by the fewest active codes, so a new code stands out from its neighbors. */
export function nextColor(codes: ReadonlyArray<Pick<CodeRow, 'color' | 'status'>>): string {
  const counts = PALETTE.map(() => 0);
  for (const c of codes) {
    if (c.status !== 'active') continue;
    const i = paletteIndex(c.color);
    if (i >= 0) counts[i]++;
  }
  let best = 0;
  for (let i = 1; i < counts.length; i++) if (counts[i] < counts[best]) best = i;
  return PALETTE[best].hex;
}

function letters(word: string): string[] {
  return Array.from(word).filter((ch) => /[\p{L}\p{N}]/u.test(ch));
}

/** The label a code would like, before checking that no other code has it. */
function wishes(name: string): string[] {
  const words = name
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
    .map(letters)
    .filter((w) => w.length > 0);
  if (words.length === 0) return ['?'];
  const out: string[] = [];
  if (words.length >= 2) {
    const initials = words.slice(0, 3).map((w) => w[0]).join('');
    out.push(initials.toUpperCase());
  }
  const flat = words.flat();
  for (const n of [3, 4, 5, 6]) out.push(flat.slice(0, n).join('').toUpperCase());
  return out;
}

/**
 * A short label for every code, such as "WEL" for "Welcome" and "COC" for "Cost of care". Labels are
 * different for different codes and do not change when other codes are added later (codes are
 * handled in id order).
 */
export function abbreviations(codes: ReadonlyArray<Pick<CodeRow, 'id' | 'name'>>): Map<number, string> {
  const taken = new Set<string>();
  const out = new Map<number, string>();
  const ordered = [...codes].sort((a, b) => a.id - b.id);
  for (const code of ordered) {
    const options = wishes(code.name);
    let label = options.find((o) => !taken.has(o));
    if (!label) {
      const base = options[options.length - 1];
      let n = 2;
      while (taken.has(base + n)) n++;
      label = base + n;
    }
    taken.add(label);
    out.set(code.id, label);
  }
  return out;
}
