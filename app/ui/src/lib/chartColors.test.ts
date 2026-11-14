// The colors of the charts, read from the stylesheets themselves and held to the contrast the page promises: words 4.5 to 1, drawn
// marks 3 to 1, in the light, dark, and high contrast themes. Where a color of the categorical palette falls under 3 to 1 against the
// light page, a segment must be outlined and labelled, so that its color is never the only thing that tells it apart.

import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';

const read = (name: string) => fs.readFileSync(new URL(`../${name}`, import.meta.url), 'utf8');
const styles = read('styles.css');
const book = read('book.css');

/** The flat rules of a stylesheet: the text before `{` and the text inside. Rules inside a media query are found too. */
function rules(css: string): { selector: string; body: string }[] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ selector: m[1].replace(/\/\*[\s\S]*?\*\//g, '').trim(), body: m[2] }));
}

/** The custom properties set in the rules whose selector matches. */
function tokens(css: string, selector: (s: string) => boolean): Record<string, string> {
  const out: Record<string, string> = {};
  for (const r of rules(css).filter((x) => selector(x.selector))) {
    for (const m of r.body.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) out[m[1]] = m[2].trim();
  }
  return out;
}

function luminance(hex: string): number {
  const n = hex.replace('#', '');
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(n.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const light = { ...tokens(styles, (s) => s === ':root'), ...tokens(book, (s) => s === ':root') };
const dark = { ...tokens(styles, (s) => s === ":root[data-theme='dark']"), ...tokens(book, (s) => s.includes(":root[data-theme='dark']")) };
const high = { ...tokens(styles, (s) => s === ":root[data-theme='contrast']"), ...tokens(book, (s) => s.includes(":root[data-theme='contrast']")) };
const themes: [string, Record<string, string>][] = [
  ['light', light],
  ['dark', dark],
  ['high contrast', high],
];

test('the stylesheets give every theme the colors the charts are drawn in', () => {
  for (const [name, t] of themes) {
    for (const key of ['--panel', '--fg', '--muted', '--accent', '--border', ...Array.from({ length: 8 }, (_, i) => `--series-${i + 1}`)]) {
      assert.match(t[key] ?? '', /^#[0-9a-f]{6}$/i, `${name}: ${key} is a color (${t[key]})`);
    }
  }
});

test('words on a chart have at least 4.5 to 1 against the page, in every theme', () => {
  for (const [name, t] of themes) {
    assert.ok(contrast(t['--fg'], t['--panel']) >= 4.5, `${name}: the text color`);
    assert.ok(contrast(t['--muted'], t['--panel']) >= 4.5, `${name}: the muted text color`);
  }
});

test('the marks of a chart, its axis, and its focus ring have at least 3 to 1 against the page, in every theme', () => {
  for (const [name, t] of themes) {
    assert.ok(contrast(t['--accent'], t['--panel']) >= 3, `${name}: the accent the bars, boxes, and points are drawn in`);
    assert.ok(contrast(t['--border'], t['--panel']) >= 3, `${name}: the axis`);
    assert.ok(contrast(t['--fg'], t['--panel']) >= 3, `${name}: the median bar, the line, and the outline of a segment`);
    assert.ok(contrast(t['--focus'] ?? t['--accent'], t['--panel']) >= 3, `${name}: the focus ring`);
  }
});

test('the palette for categories has at least 3 to 1 against the page in the dark themes, and where it falls short in the light theme a segment is outlined and labelled', () => {
  for (const [name, t] of [['dark', dark], ['high contrast', high]] as const) {
    for (let i = 1; i <= 8; i++) assert.ok(contrast(t[`--series-${i}`], t['--panel']) >= 3, `${name}: series ${i}`);
  }
  const short = Array.from({ length: 8 }, (_, i) => i + 1).filter((i) => contrast(light[`--series-${i}`], light['--panel']) < 3);
  assert.deepEqual(short, [3, 4, 5], 'the light colors under 3 to 1 are the three the palette documents (aqua, yellow, magenta)');
  const seg = rules(book).find((r) => r.selector === '.chart-seg');
  assert.ok(seg && /stroke:\s*var\(--fg\)/.test(seg.body) && /stroke-width:\s*1\b/.test(seg.body), 'a segment is outlined in the text color, which has the contrast the fill lacks');
  const swatch = rules(book).find((r) => r.selector === '.chart-swatch');
  assert.ok(swatch && /border:\s*1px solid var\(--fg\)/.test(swatch.body), 'and so is the swatch in the legend');
  const pill = rules(book).find((r) => r.selector === '.chart-pill');
  assert.ok(pill && /fill:\s*var\(--panel\)/.test(pill.body), 'the share written in a segment sits on the page color, not on the series color');
});

test('under forced colors the marks take the system colors and the words the system text color', () => {
  const forced = book.slice(book.indexOf('@media (forced-colors: active)', book.indexOf('.stats-chart-table summary')));
  for (const want of ['Highlight', 'CanvasText', 'Canvas']) assert.ok(forced.includes(want), `forced colors use ${want}`);
});
