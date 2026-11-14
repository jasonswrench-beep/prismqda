// Turns the icon SVG files in assets/icons into one TypeScript module the interface can draw from.
//
//   npm run icons          rewrite src/assets/icons.generated.ts
//   npm run icons:check    fail if the file is out of date (the test run does this too)
//
// The SVG files stay the source of truth. This reads them with a deliberately strict parser: only <svg> and <path>
// elements, only the attributes the icon standard uses, and path data made of path commands and numbers. An icon that
// carries anything else (a script, a link, an embedded image, a transform) stops the build instead of reaching the
// window, and the interface draws the icons as ordinary elements, never as injected markup.

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
export const ICON_ROOT = resolve(HERE, '../../../assets/icons');
export const OUT_FILE = resolve(HERE, '../src/assets/icons.generated.ts');

export interface ParsedPath {
  d: string;
  fillRule?: 'evenodd' | 'nonzero';
  fill?: 'currentColor' | 'none';
  stroke?: 'currentColor' | 'none';
  /** A dashed line, written as numbers only (for example "2 3"). */
  dash?: string;
}

export interface ParsedIcon {
  viewBox: string;
  /** Filled icons (the selected-tab versions) paint with fill; the rest are 1.75 px outlines. */
  filled: boolean;
  paths: ParsedPath[];
}

const SVG_OUTLINE: Record<string, string> = {
  xmlns: 'http://www.w3.org/2000/svg',
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  'stroke-width': '1.75',
  'stroke-linecap': 'round',
  'stroke-linejoin': 'round',
};
const SVG_FILLED: Record<string, string> = {
  xmlns: 'http://www.w3.org/2000/svg',
  viewBox: '0 0 24 24',
  fill: 'currentColor',
  stroke: 'none',
};
const PATH_DATA = /^[MmLlHhVvCcSsQqTtAaZz0-9eE.,\s+-]+$/;
const DASH_ARRAY = /^[0-9. ,]+$/;

function attributesOf(text: string, where: string): Record<string, string> {
  const found: Record<string, string> = {};
  const rest = text.replace(/([A-Za-z:-]+)="([^"]*)"/g, (_m, name: string, value: string) => {
    if (name in found) throw new Error(`${where}: attribute ${name} appears twice`);
    found[name] = value;
    return '';
  });
  if (rest.trim() !== '') throw new Error(`${where}: unexpected text inside a tag: ${rest.trim()}`);
  return found;
}

/** Reads one icon file. Throws, naming the file, on anything outside the icon standard. */
export function parseIcon(source: string, where: string): ParsedIcon {
  const open = /^\s*<svg\s+([^>]*)>/.exec(source);
  if (!open) throw new Error(`${where}: does not start with an <svg> element`);
  const root = attributesOf(open[1], `${where} <svg>`);
  const sameAs = (expected: Record<string, string>) =>
    Object.keys(expected).length === Object.keys(root).length && Object.entries(expected).every(([k, v]) => root[k] === v);
  const filled = sameAs(SVG_FILLED);
  if (!filled && !sameAs(SVG_OUTLINE)) {
    throw new Error(`${where}: the <svg> attributes match neither the outline nor the filled icon contract: ${JSON.stringify(root)}`);
  }

  let body = source.slice(open[0].length);
  if (!/<\/svg>\s*$/.test(body)) throw new Error(`${where}: does not end with </svg>`);
  body = body.replace(/<\/svg>\s*$/, '');

  const paths: ParsedPath[] = [];
  const leftover = body.replace(/<path\s+([^>]*?)\s*\/>/g, (_m, attrText: string) => {
    const a = attributesOf(attrText, `${where} <path>`);
    for (const k of Object.keys(a)) {
      if (!['d', 'fill-rule', 'fill', 'stroke', 'stroke-dasharray'].includes(k)) throw new Error(`${where}: a <path> carries ${k}, which the icon standard does not allow`);
    }
    if (!a.d || !PATH_DATA.test(a.d)) throw new Error(`${where}: path data is missing or holds something other than path commands and numbers`);
    const p: ParsedPath = { d: a.d.replace(/\s+/g, ' ').trim() };
    if (a['fill-rule'] !== undefined) {
      if (a['fill-rule'] !== 'evenodd' && a['fill-rule'] !== 'nonzero') throw new Error(`${where}: fill-rule ${a['fill-rule']}`);
      p.fillRule = a['fill-rule'];
    }
    for (const k of ['fill', 'stroke'] as const) {
      if (a[k] !== undefined) {
        if (a[k] !== 'currentColor' && a[k] !== 'none') throw new Error(`${where}: ${k}="${a[k]}" is not currentColor or none`);
        p[k] = a[k] as 'currentColor' | 'none';
      }
    }
    if (a['stroke-dasharray'] !== undefined) {
      if (!DASH_ARRAY.test(a['stroke-dasharray'])) throw new Error(`${where}: stroke-dasharray must be numbers only`);
      p.dash = a['stroke-dasharray'];
    }
    paths.push(p);
    return '';
  });
  if (leftover.trim() !== '') throw new Error(`${where}: found something other than <path> inside the icon: ${leftover.trim().slice(0, 80)}`);
  if (paths.length === 0) throw new Error(`${where}: no paths`);
  return { viewBox: root.viewBox, filled, paths };
}

/** The text of the generated module, built from every icon file under assets/icons. */
export function buildModule(root: string = ICON_ROOT): string {
  const icons: Record<string, ParsedIcon> = {};
  for (const folder of ['regular', 'active']) {
    for (const file of readdirSync(join(root, folder)).filter((f) => f.endsWith('.svg')).sort()) {
      const name = file.slice(0, -'.svg'.length);
      if (name in icons) throw new Error(`the icon name ${name} appears in more than one folder`);
      if (!/^[a-z][a-z0-9-]*$/.test(name)) throw new Error(`${folder}/${file}: icon names are lowercase letters, digits, and hyphens`);
      icons[name] = parseIcon(readFileSync(join(root, folder, file), 'utf8'), `${folder}/${file}`);
    }
  }
  const names = Object.keys(icons).sort();
  const lines = names.map((n) => `  ${JSON.stringify(n)}: ${JSON.stringify(icons[n])},`);
  return [
    '// Generated by scripts/build-icons.ts from the SVG files in assets/icons. Do not edit by hand: run `npm run icons`.',
    '',
    'export interface IconPath {',
    '  d: string;',
    "  fillRule?: 'evenodd' | 'nonzero';",
    "  fill?: 'currentColor' | 'none';",
    "  stroke?: 'currentColor' | 'none';",
    '  dash?: string;',
    '}',
    '',
    'export interface IconDef {',
    '  viewBox: string;',
    '  filled: boolean;',
    '  paths: IconPath[];',
    '}',
    '',
    'export const ICON_DEFS = {',
    ...lines,
    '} as const satisfies Record<string, IconDef>;',
    '',
    'export type IconName = keyof typeof ICON_DEFS;',
    '',
  ].join('\n');
}

const normalize = (text: string) => text.replace(/\r\n/g, '\n');

/** True when the committed module matches what the SVG files would generate. */
export function isUpToDate(): boolean {
  try {
    return normalize(readFileSync(OUT_FILE, 'utf8')) === buildModule();
  } catch {
    return false;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.includes('--check')) {
    if (!isUpToDate()) {
      console.error('src/assets/icons.generated.ts is out of date. Run: npm run icons');
      process.exit(1);
    }
    console.log('icons.generated.ts is up to date');
  } else {
    writeFileSync(OUT_FILE, buildModule());
    console.log(`wrote ${OUT_FILE}`);
  }
}
