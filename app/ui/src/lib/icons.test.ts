import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { ICON_DEFS } from '../assets/icons.generated';
import { ICON_ROOT, buildModule, isUpToDate, parseIcon } from '../../scripts/build-icons';

const OUTLINE = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">';

test('the generated icon module matches the SVG files in assets/icons', () => {
  assert.ok(isUpToDate(), 'run `npm run icons` after changing an icon file');
});

test('every SVG file has a generated icon, named like its file, and nothing else is in the module', () => {
  const files = ['regular', 'active'].flatMap((f) => readdirSync(join(ICON_ROOT, f)).filter((n) => n.endsWith('.svg')).map((n) => n.slice(0, -4)));
  // Batch 1 is 93 outline icons and 12 selected-tab icons; Batch 2 adds 120 outline icons; Batch 3 adds 14; and five more
  // were drawn on 2026-11-06 for controls the screen specification book shows that the library did not have (Save, New
  // Project's file, split view, next and previous gap, and a graduation cap for Sample Projects). They are drafts like the rest, pending review.
  assert.equal(files.length, 245);
  assert.deepEqual(Object.keys(ICON_DEFS).sort(), [...files].sort());
});

test('every workspace icon has a selected version drawn with fill, and the others are outlines', () => {
  const workspaces = Object.keys(ICON_DEFS).filter((n) => /^ws-[a-z]+$/.test(n));
  assert.equal(workspaces.length, 12);
  for (const w of workspaces) {
    const active = ICON_DEFS[`${w}-active` as keyof typeof ICON_DEFS];
    assert.ok(active, `${w} has no selected version`);
    assert.equal(active.filled, true, `${w}-active should be filled`);
    assert.equal(ICON_DEFS[w as keyof typeof ICON_DEFS].filled, false, `${w} should be an outline`);
  }
  const filled = Object.entries(ICON_DEFS).filter(([, d]) => d.filled).map(([n]) => n);
  assert.ok(filled.every((n) => n.endsWith('-active')), `only the selected-tab icons are filled: ${filled.join(', ')}`);
});

test('icon geometry stays inside the 24 pixel grid with its 2 pixel margin, roughly', () => {
  // A coarse guard on path coordinates: absolute numbers in the path data should sit within the 24 by 24 box.
  for (const [name, def] of Object.entries(ICON_DEFS)) {
    assert.equal(def.viewBox, '0 0 24 24', name);
    for (const p of def.paths) {
      // Relative commands carry small offsets and absolute ones carry positions, so only check absolute pairs after M, L, H, V.
      for (const m of p.d.matchAll(/[MLHV]\s*(-?\d+(?:\.\d+)?)(?:[\s,]+(-?\d+(?:\.\d+)?))?/g)) {
        for (const n of [m[1], m[2]].filter((x): x is string => x !== undefined)) {
          const v = Number(n);
          assert.ok(v >= -0.5 && v <= 24.5, `${name}: coordinate ${v} is outside the grid`);
        }
      }
    }
  }
});

test('the parser rejects anything the icon standard does not allow', () => {
  const ok = `${OUTLINE}<path d="M3 3L9 9"/></svg>`;
  assert.equal(parseIcon(ok, 'ok.svg').paths.length, 1);
  assert.equal(parseIcon(`${OUTLINE}<path d="M3 3L9 9" stroke-dasharray="2 3"/></svg>`, 'dash.svg').paths[0].dash, '2 3', 'a dashed line is allowed');
  const cases: [string, string][] = [
    ['a script', `${OUTLINE}<script>alert(1)</script><path d="M3 3L9 9"/></svg>`],
    ['an embedded image', `${OUTLINE}<image href="data:image/png;base64,AAAA"/><path d="M3 3L9 9"/></svg>`],
    ['a transform', `${OUTLINE}<path d="M3 3L9 9" transform="rotate(5)"/></svg>`],
    ['an event handler', `${OUTLINE}<path d="M3 3L9 9" onload="x()"/></svg>`],
    ['an external reference', `${OUTLINE}<path d="M3 3L9 9" style="fill:url(http://example.test/a)"/></svg>`],
    ['text in the path data', `${OUTLINE}<path d="M3 3L9 9 javascript:x"/></svg>`],
    ['a hard-coded color', `${OUTLINE}<path d="M3 3L9 9" stroke="#ff0000"/></svg>`],
    ['a different stroke width', ok.replace('1.75', '2')],
    ['a dash pattern that is not numbers', `${OUTLINE}<path d="M3 3L9 9" stroke-dasharray="url(#x)"/></svg>`],
    ['a different viewBox', ok.replace('0 0 24 24', '0 0 32 32')],
    ['no paths', `${OUTLINE}</svg>`],
    ['an unclosed svg', `${OUTLINE}<path d="M3 3L9 9"/>`],
  ];
  for (const [what, svg] of cases) {
    assert.throws(() => parseIcon(svg, 'bad.svg'), `a file with ${what} should be rejected`);
  }
});

test('every icon in the registry is in the repository: all 244 planned icons are drawn', () => {
  const registry = JSON.parse(readFileSync(join(ICON_ROOT, '..', '..', 'design', 'icons', 'registry', 'full-library-registry.json'), 'utf8')) as {
    total_canonical_svg_targets: number;
    assets: { filename?: string; path?: string; batch: number }[];
  };
  assert.equal(registry.total_canonical_svg_targets, 244);
  const folders = ['icons/regular', 'icons/active', 'brand'];
  const present = new Set(folders.flatMap((f) => readdirSync(join(ICON_ROOT, '..', ...f.split('/'))).filter((n) => n.endsWith('.svg'))));
  const drafted = registry.assets;
  assert.equal(drafted.length, 244, 'Batch 1 is 108, Batch 2 is 122, Batch 3 is 14');
  const missing = drafted.map((a) => a.filename ?? (a.path ?? '').split('/').pop() ?? '').filter((n) => !present.has(n));
  assert.deepEqual(missing, [], 'every planned icon has its file');
});

test('the brand files carry no scripts, links, or embedded images', () => {
  const brand = join(ICON_ROOT, '..', 'brand');
  for (const name of readdirSync(brand).filter((n) => n.endsWith('.svg'))) {
    const text = readFileSync(join(brand, name), 'utf8');
    assert.ok(!/<script|<image|<foreignObject|\son[a-z]+\s*=|href\s*=\s*["']?(?!#)/i.test(text), `${name} holds active or external content`);
  }
});

test('building the module twice gives the same text', () => {
  assert.equal(buildModule(), buildModule());
});
