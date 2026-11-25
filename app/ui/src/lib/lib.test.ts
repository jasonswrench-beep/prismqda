import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildSegments } from './segments';
import { assignLanes } from './lanes';
import { PALETTE, abbreviations, blendOver, colorName, contrastRatio, highlightBackground, highlightColor, nextColor, patternFor, textColorOn, PATTERN_COUNT } from './labels';
import { buildCodeTree, filterCodeTree, flattenTree, planDrop, planMove, compareCodes, childrenOf, descendantIds, pathLabel } from './tree';
import { interpretAreaKey, interpretCodebookKey, interpretDocumentKey, SHORTCUT_SECTIONS, undoesTyping, type KeyLike } from './shortcuts';
import { codedSentence, describeEvent, foldEvents, quoted, categoryOf, formatWhen, type WordingLookups } from './eventWording';
import { computeCoverage, coveredLength, describeCoverage } from './coverage';
import { nextUncodedUnit, unitIndexAt, unitIndexForCaret, unitIndicesTouching } from './units';
import { threadRows, hasReplies } from './thread';
import { choicesFromSuggestion, mappingFromChoices, setLevel, setRole, validateChoices } from './importMapping';
import { normalizeFavorites, pin, unpin, codeForSlot, slotOf } from './favorites';
import { cleanTypedPath } from '../pickFile';
import type { CodeRow, EventRow, TablePreview, UnitRow } from '../types';

function code(partial: Partial<CodeRow> & { id: number; name: string }): CodeRow {
  return {
    parent_id: null, kind: 'code', color: '#808080', definition: '', inclusion_criteria: '', exclusion_criteria: '',
    status: 'active', merged_into_id: null, version: 1, sort_order: 0, do_not_share: false, exclusive_group: '', rated: false, rating_label: '', rating_low: '', rating_high: '', usage: 0, ...partial,
  };
}

const key = (k: string, extra: Partial<KeyLike> = {}): KeyLike => ({ key: k, ctrlKey: false, altKey: false, metaKey: false, shiftKey: false, ...extra });

// ---------------------------------------------------------------- segments

test('segments split at every coding, format, and unit boundary', () => {
  const segs = buildSegments({
    length: 20,
    codings: [{ id: 1, start: 2, end: 8 }, { id: 2, start: 5, end: 12 }],
    runs: [{ start: 0, end: 4, kind: 'heading', level: 2 }, { start: 3, end: 6, kind: 'bold' }],
    units: [{ start: 0, end: 10 }, { start: 10, end: 20 }],
  });
  assert.deepEqual(segs.map((s) => [s.start, s.end]), [[0, 2], [2, 3], [3, 4], [4, 5], [5, 6], [6, 8], [8, 10], [10, 12], [12, 20]]);
  const at = (n: number) => segs.find((s) => s.start === n)!;
  assert.deepEqual(at(0).codings, []);
  assert.deepEqual(at(2).codings, [1]);
  assert.deepEqual(at(5).codings.sort(), [1, 2]);
  assert.deepEqual(at(8).codings, [2]);
  assert.deepEqual(at(12).codings, []);
  assert.equal(at(0).heading, 2);
  assert.equal(at(3).bold, true);
  assert.equal(at(3).heading, 2);
  assert.equal(at(4).heading, 0);
  assert.equal(at(0).unitStart, true);
  assert.equal(at(2).unitStart, false);
  assert.equal(at(10).unitStart, true);
  assert.equal(at(10).unit, 1);
});

test('segments cover the whole text with no gaps or overlaps', () => {
  const segs = buildSegments({ length: 50, codings: [{ id: 1, start: 10, end: 30 }, { id: 2, start: 30, end: 40 }], runs: [], units: [{ start: 5, end: 45 }] });
  assert.equal(segs[0].start, 0);
  assert.equal(segs[segs.length - 1].end, 50);
  for (let i = 1; i < segs.length; i++) assert.equal(segs[i].start, segs[i - 1].end);
  const gap = segs.find((s) => s.start === 0)!;
  assert.equal(gap.unit, -1);
  assert.deepEqual(segs.find((s) => s.start === 30)!.codings, [2], 'a coding that ended is not carried into the next piece');
});

test('segments ignore empty and out-of-range spans, and an empty text has none', () => {
  assert.deepEqual(buildSegments({ length: 0, codings: [], runs: [], units: [] }), []);
  const segs = buildSegments({ length: 10, codings: [{ id: 1, start: 4, end: 4 }, { id: 2, start: -5, end: 99 }], runs: [], units: [] });
  assert.deepEqual(segs.map((s) => [s.start, s.end, s.codings]), [[0, 10, [2]]]);
});

test('page runs give the piece a page number', () => {
  const segs = buildSegments({ length: 10, codings: [], runs: [{ start: 0, end: 5, kind: 'page', page: 3 }], units: [] });
  assert.equal(segs[0].page, 3);
  assert.equal(segs[1].page, null);
});

// ---------------------------------------------------------------- lanes

test('stripes that share lines go in different lanes, and later ones reuse free lanes', () => {
  const r = assignLanes([
    { id: 1, top: 0, bottom: 50 },
    { id: 2, top: 20, bottom: 70 },
    { id: 3, top: 60, bottom: 90 },
    { id: 4, top: 100, bottom: 120 },
  ]);
  assert.equal(r.lanes.get(1), 0);
  assert.equal(r.lanes.get(2), 1);
  assert.equal(r.lanes.get(3), 0);
  assert.equal(r.lanes.get(4), 0);
  assert.equal(r.count, 2);
});

test('when more stripes overlap than lanes exist, the extra ones are reported', () => {
  const boxes = Array.from({ length: 5 }, (_, i) => ({ id: i + 1, top: 0, bottom: 10 }));
  const r = assignLanes(boxes, 3);
  assert.equal(r.count, 3);
  assert.deepEqual(r.overflow, [4, 5]);
  assert.equal(r.lanes.get(5), 2);
});

// ---------------------------------------------------------------- labels

test('every palette color has a pattern, a unique name, and readable label text', () => {
  // The first eight colors each have a pattern of their own; later colors reuse them, and the short text label
  // on every code is what tells two codes with the same pattern apart.
  const first = new Set(PALETTE.slice(0, PATTERN_COUNT).map((p) => patternFor(p.hex)));
  assert.equal(first.size, PATTERN_COUNT, 'each of the first eight colors has its own pattern');
  assert.equal(new Set(PALETTE.map((p) => p.name)).size, PALETTE.length, 'names are unique');
  assert.equal(new Set(PALETTE.map((p) => p.hex.toLowerCase())).size, PALETTE.length, 'colors are unique');
  for (const p of PALETTE) {
    assert.ok(contrastRatio(p.hex, textColorOn(p.hex)) >= 4.5, `${p.name} text contrast`);
    assert.equal(colorName(p.hex), p.name);
  }
  assert.equal(colorName('#123456'), '#123456');
  assert.ok(PALETTE.length >= 16, 'a codebook of a dozen codes does not run out of colors');
});

test('words highlighted in any palette color stay readable in light, dark, and high-contrast colors', () => {
  const pages: [string, string, string][] = [
    ['light', '#ffffff', '#1c1f26'],
    ['dark', '#16181d', '#e8eaee'],
    ['high contrast', '#000000', '#ffffff'],
  ];
  for (const p of PALETTE) {
    for (const [theme, page, text] of pages) {
      const seen = blendOver(p.hex, page);
      assert.ok(contrastRatio(seen, text) >= 4.5, `${p.name} in ${theme} colors: ${contrastRatio(seen, text).toFixed(2)} to 1`);
    }
  }
});

test('highlighter backgrounds: none, one color, or bands for overlapping codes', () => {
  assert.equal(highlightBackground([]), '');
  assert.equal(highlightColor('#E69F00'), 'rgba(230, 159, 0, 0.32)');
  assert.equal(highlightBackground(['#E69F00']), 'rgba(230, 159, 0, 0.32)');
  const two = highlightBackground(['#E69F00', '#0072B2']);
  assert.match(two, /^linear-gradient\(to bottom, rgba\(230, 159, 0, 0\.32\) 0\.00% 50\.00%, rgba\(0, 114, 178, 0\.32\) 50\.00% 100\.00%\)$/);
  assert.equal(highlightColor('not a color'), 'rgba(128, 128, 128, 0.32)');
});

test('textColorOn always reaches 4.5 to 1', () => {
  for (let r = 0; r < 256; r += 51) for (let g = 0; g < 256; g += 51) for (let b = 0; b < 256; b += 51) {
    const hex = '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('');
    assert.ok(contrastRatio(hex, textColorOn(hex)) >= 4.5, hex);
  }
});

test('contrast ratio matches the WCAG values for black and white', () => {
  assert.equal(Math.round(contrastRatio('#000000', '#ffffff')), 21);
  assert.equal(contrastRatio('#ffffff', '#ffffff'), 1);
});

test('a custom color still gets a stable pattern', () => {
  assert.equal(patternFor('#123456'), patternFor('#123456'));
  assert.ok(patternFor('#abcdef') >= 0 && patternFor('#abcdef') < PATTERN_COUNT);
  assert.equal(patternFor('not a color'), patternFor('#808080'));
});

test('a new code gets the palette color that is used least', () => {
  const codes = [code({ id: 1, name: 'a', color: PALETTE[0].hex }), code({ id: 2, name: 'b', color: PALETTE[1].hex }), code({ id: 3, name: 'c', color: PALETTE[0].hex, status: 'retired' })];
  const next = nextColor(codes);
  assert.notEqual(next, PALETTE[0].hex);
  assert.notEqual(next, PALETTE[1].hex);
  assert.equal(nextColor([]), PALETTE[0].hex);
});

test('short labels are unique, stable, and readable', () => {
  const labels = abbreviations([
    { id: 1, name: 'Welcome' },
    { id: 2, name: 'Cost of care' },
    { id: 3, name: 'Welcoming staff' },
    { id: 4, name: 'Wellness' },
    { id: 5, name: 'Welcome' },
    { id: 6, name: '!!!' },
    { id: 7, name: '\u6211\u611f\u5230\u5f88\u53d7\u6b22\u8fce' },
  ]);
  assert.equal(labels.get(1), 'WEL');
  assert.equal(labels.get(2), 'COC');
  const all = [...labels.values()];
  assert.equal(new Set(all).size, all.length, 'no two codes share a label');
  assert.equal(labels.get(6), '?');
  assert.equal(labels.get(7), '\u6211\u611f\u5230');
  // Adding a code later does not change an earlier code's label.
  const later = abbreviations([{ id: 1, name: 'Welcome' }, { id: 2, name: 'Cost of care' }, { id: 9, name: 'Welcome back' }]);
  assert.equal(later.get(1), 'WEL');
  assert.notEqual(later.get(9), 'WEL');
});

// ---------------------------------------------------------------- tree

const sample: CodeRow[] = [
  code({ id: 1, name: 'Support', sort_order: 0 }),
  code({ id: 2, name: 'Family', parent_id: 1, sort_order: 0 }),
  code({ id: 3, name: 'Friends', parent_id: 1, sort_order: 1 }),
  code({ id: 4, name: 'Cost', sort_order: 1 }),
  code({ id: 5, name: 'Old idea', parent_id: 1, sort_order: 2, status: 'retired' }),
  code({ id: 6, name: 'Merged away', sort_order: 3, status: 'merged', merged_into_id: 4 }),
];

test('the tree orders siblings like the engine does and hides retired and merged codes by default', () => {
  const tree = buildCodeTree(sample, { showInactive: false });
  assert.deepEqual(tree.map((n) => n.code.name), ['Support', 'Cost']);
  assert.deepEqual(tree[0].children.map((n) => n.code.name), ['Family', 'Friends']);
  const all = buildCodeTree(sample, { showInactive: true });
  assert.deepEqual(all.map((n) => n.code.name), ['Support', 'Cost', 'Merged away']);
  assert.equal(all[0].children.length, 3);
  assert.equal(flattenTree(all).length, 6);
});

test('a retired code with an active code below it stays visible as a parent', () => {
  const codes = [code({ id: 1, name: 'Old', status: 'retired' }), code({ id: 2, name: 'Still used', parent_id: 1 })];
  const tree = buildCodeTree(codes, { showInactive: false });
  assert.equal(tree.length, 1);
  assert.equal(tree[0].onlyAsParent, true);
  assert.equal(tree[0].children[0].code.name, 'Still used');
});

test('siblings with the same order are sorted by name, ignoring case, then id', () => {
  const codes = [code({ id: 3, name: 'beta' }), code({ id: 2, name: 'Alpha' }), code({ id: 1, name: 'alpha' })];
  assert.deepEqual(childrenOf(codes, null).map((c) => c.id), [1, 2, 3]);
  assert.ok(compareCodes(codes[1], codes[0]) < 0);
});

test('filtering keeps matches and the parents that lead to them, and opens those parents', () => {
  const tree = buildCodeTree(sample, { showInactive: false });
  const f = filterCodeTree(tree, 'fri', () => '');
  assert.deepEqual(f.nodes.map((n) => n.code.name), ['Support']);
  assert.deepEqual(f.nodes[0].children.map((n) => n.code.name), ['Friends']);
  assert.deepEqual([...f.matches], [3]);
  assert.deepEqual([...f.expanded], [1]);
  assert.equal(filterCodeTree(tree, '', () => '').nodes.length, 2);
  assert.equal(filterCodeTree(tree, 'zzz', () => '').nodes.length, 0);
  assert.equal(filterCodeTree(tree, 'COC', (id) => (id === 4 ? 'COC' : '')).nodes.length, 1, 'a short label finds its code');
});

test('descendants, paths, and loops', () => {
  assert.deepEqual([...descendantIds(sample, 1)].sort(), [2, 3, 5]);
  assert.equal(pathLabel(sample, 2), 'Support > Family');
});

test('move up and down step over hidden codes and give the engine a position', () => {
  const down = planMove(sample, 2, 'down', false);
  assert.ok(down.ok);
  if (down.ok) assert.deepEqual([down.plan.parent_id, down.plan.position], [1, 1]);
  const up = planMove(sample, 3, 'up', false);
  assert.ok(up.ok);
  if (up.ok) assert.deepEqual([up.plan.parent_id, up.plan.position], [1, 0]);
  const last = planMove(sample, 3, 'down', false);
  assert.equal(last.ok, false, 'Old idea is retired and hidden, so Friends is last');
  const lastShown = planMove(sample, 3, 'down', true);
  assert.ok(lastShown.ok);
  const first = planMove(sample, 2, 'up', false);
  assert.equal(first.ok, false);
  if (!first.ok) assert.match(first.reason, /already first/);
});

test('indent moves a code under the one above it; outdent moves it after its parent', () => {
  const indent = planMove(sample, 4, 'indent', false);
  assert.ok(indent.ok);
  if (indent.ok) assert.deepEqual([indent.plan.parent_id, indent.plan.position, indent.plan.where], [1, undefined, 'under Support']);
  assert.equal(planMove(sample, 1, 'indent', false).ok, false);
  const out = planMove(sample, 3, 'outdent', false);
  assert.ok(out.ok);
  if (out.ok) assert.deepEqual([out.plan.parent_id, out.plan.position], [null, 1]);
  const top = planMove(sample, 1, 'outdent', false);
  assert.equal(top.ok, false);
});

test('dropping a code on, before, or after another', () => {
  const on = planDrop(sample, 4, 2, 'on');
  assert.ok(on.ok);
  if (on.ok) assert.deepEqual([on.plan.parent_id, on.plan.position], [2, undefined]);
  const before = planDrop(sample, 4, 3, 'before');
  assert.ok(before.ok);
  if (before.ok) assert.deepEqual([before.plan.parent_id, before.plan.position], [1, 1]);
  const after = planDrop(sample, 4, 3, 'after');
  assert.ok(after.ok);
  if (after.ok) assert.deepEqual([after.plan.parent_id, after.plan.position], [1, 2]);
  assert.equal(planDrop(sample, 1, 2, 'on').ok, false, 'a code cannot go under its own sub-code');
  assert.equal(planDrop(sample, 1, 1, 'on').ok, false);
});

// ---------------------------------------------------------------- shortcuts

test('number keys 1 to 9 pick a favorite; other keys do not', () => {
  for (let n = 1; n <= 9; n++) assert.deepEqual(interpretDocumentKey(key(String(n))), { type: 'favorite', slot: n });
  assert.equal(interpretDocumentKey(key('0')), null);
  assert.equal(interpretDocumentKey(key('1', { ctrlKey: true })), null, 'Ctrl+1 is not ours');
  assert.equal(interpretDocumentKey(key('1', { shiftKey: true })), null);
  assert.equal(interpretDocumentKey(key('1', { isComposing: true })), null, 'no shortcuts while an input method is composing');
});

test("Ctrl+Z undoes typing in a box, and the project's last change in the document's read-only text", () => {
  const el = (tagName: string, isContentEditable = false, readonly: string | null = null) => ({ tagName, isContentEditable, getAttribute: (n: string) => (n === 'aria-readonly' ? readonly : null) });
  assert.equal(undoesTyping(el('INPUT')), true);
  assert.equal(undoesTyping(el('TEXTAREA')), true);
  assert.equal(undoesTyping(el('SELECT')), true);
  assert.equal(undoesTyping(el('DIV', true)), true, 'an area one can type in keeps its own undo');
  assert.equal(undoesTyping(el('DIV', true, 'true')), false, 'the document text has no typing to undo');
  assert.equal(undoesTyping(el('BUTTON')), false);
  assert.equal(undoesTyping(null), false);
});

test('letter shortcuts', () => {
  assert.deepEqual(interpretDocumentKey(key('Enter')), { type: 'pick' });
  assert.deepEqual(interpretDocumentKey(key('i')), { type: 'invivo' });
  assert.deepEqual(interpretDocumentKey(key('I')), { type: 'invivo' }, 'Caps Lock does not matter');
  assert.deepEqual(interpretDocumentKey(key('r')), { type: 'repeat' });
  assert.deepEqual(interpretDocumentKey(key('n')), { type: 'next' });
  assert.deepEqual(interpretDocumentKey(key('N', { shiftKey: true })), { type: 'previous' });
  assert.deepEqual(interpretDocumentKey(key('e')), { type: 'examine' });
  assert.deepEqual(interpretDocumentKey(key('u')), { type: 'selectUnit' });
  assert.equal(interpretDocumentKey(key('x')), null);
  assert.equal(interpretDocumentKey(key('ArrowLeft')), null);
  assert.equal(interpretDocumentKey(key('i', { altKey: true })), null);
  assert.equal(interpretDocumentKey(key('c', { ctrlKey: true })), null, 'Ctrl+C still copies');
});

test('codebook reorganize keys need Alt and an arrow', () => {
  assert.equal(interpretCodebookKey(key('ArrowUp', { altKey: true })), 'up');
  assert.equal(interpretCodebookKey(key('ArrowDown', { altKey: true })), 'down');
  assert.equal(interpretCodebookKey(key('ArrowRight', { altKey: true })), 'indent');
  assert.equal(interpretCodebookKey(key('ArrowLeft', { altKey: true })), 'outdent');
  assert.equal(interpretCodebookKey(key('ArrowLeft')), null);
  assert.equal(interpretCodebookKey(key('ArrowLeft', { altKey: true, shiftKey: true })), null);
});

test('area keys', () => {
  assert.equal(interpretAreaKey(key('F6')), 'next');
  assert.equal(interpretAreaKey(key('F6', { shiftKey: true })), 'previous');
  assert.equal(interpretAreaKey(key('F1')), 'help');
  assert.equal(interpretAreaKey(key('F5')), null);
});

test('the shortcut sheet explains every command the key handler can return', () => {
  const text = JSON.stringify(SHORTCUT_SECTIONS);
  for (const needle of ['1 to 9', 'Enter', 'In-vivo', 'Repeat', 'next or previous unit', 'F6', 'F1', 'Alt+Up', 'Alt+Right', 'Alt+Left', 'Shift+Arrow']) {
    assert.ok(text.includes(needle), `the sheet mentions ${needle}`);
  }
  for (const s of SHORTCUT_SECTIONS) {
    assert.ok(s.entries.length > 0);
    for (const e of s.entries) assert.ok(e.keys && e.does);
  }
});

// ---------------------------------------------------------------- wording

const lookups: WordingLookups = {
  codeName: (id) => ({ 1: 'Welcome', 2: 'Cost', 3: 'Access' } as Record<number, string>)[id ?? -1] ?? null,
  sourceName: (id) => (id === 7 ? 'Interview 1' : null),
  quote: (s, a, b) => (s === 7 && a === 2 && b === 14 ? 'felt welcome' : null),
};
const ev = (action: string, before: EventRow['before'], after: EventRow['after'], target_id: number | null = null): EventRow => ({
  id: 1, ts: '2026-11-04T15:05:00.000Z', actor: 'Ada', session: 's', action, target_type: null, target_id, before, after,
});

test('a coding is described the way the task asks', () => {
  const d = describeEvent(ev('coding.add', null, { source_id: 7, start: 2, end: 14, code_id: 1, origin: 'human' }), lookups);
  assert.equal(d.text, "Coded 'felt welcome' as Welcome");
  assert.equal(d.detail, 'in Interview 1');
  assert.equal(d.category, 'coding');
  assert.equal(codedSentence('felt welcome', 'Welcome'), "Coded 'felt welcome' as Welcome");
  assert.equal(codedSentence(null, 'Welcome'), 'Coded some text as Welcome');
});

test('events are described in plain words', () => {
  const t = (action: string, before: EventRow['before'], after: EventRow['after'], target?: number) => describeEvent(ev(action, before, after, target ?? null), lookups).text;
  assert.equal(t('coding.remove', { source_id: 7, start: 2, end: 14, code_id: 1 }, null), "Removed the coding 'felt welcome' (Welcome)");
  assert.equal(t('coding.remove', { source_id: 9, start: 0, end: 1, code_id: 2 }, null), 'Removed a coding (Cost)');
  assert.equal(t('coding.change_code', { code_id: 1 }, { code_id: 2 }), 'Changed a coding from Welcome to Cost');
  assert.equal(t('coding.rationale', { rationale: '' }, { rationale: 'explicit feeling' }), "Wrote a note on a coding: 'explicit feeling'");
  assert.equal(t('anchor.add', null, { code_id: 1 }), 'Marked a coding as an example for Welcome');
  assert.equal(t('counterexample.add', null, { code_id: 1 }), 'Marked a coding as a counterexample of Welcome');
  assert.equal(t('counterexample.remove', null, { code_id: 1 }), 'Took a counterexample away from Welcome');
  assert.equal(t('code.add', null, { name: 'Access', parent_id: 1, kind: 'code' }), 'Added the code Access under Welcome');
  assert.equal(t('code.add', null, { name: 'Access', parent_id: null, kind: 'theme' }), 'Added the theme Access');
  assert.equal(t('code.update', { name: 'X', definition: '' }, { name: 'Access', definition: 'Getting in' }), 'Renamed the code X to Access and changed its definition');
  assert.equal(t('code.update', { name: 'Access', color: '#111111' }, { name: 'Access', color: '#222222' }), 'Edited the code Access: changed its color');
  assert.equal(t('code.move', { parent_id: 1 }, { parent_id: null }, 3), 'Moved the code Access to the top level');
  assert.equal(t('code.move', { parent_id: 1 }, { parent_id: 2 }, 3), 'Moved the code Access under Cost');
  assert.equal(t('code.retire', null, null, 2), 'Retired the code Cost');
  assert.equal(t('code.merge', { name: 'Access' }, { merged_into_name: 'Cost', coding_ids: [1, 2] }, 3), 'Merged the code Access into Cost');
  assert.equal(describeEvent(ev('code.merge', { name: 'Access' }, { merged_into_name: 'Cost', coding_ids: [1, 2] }, 3), lookups).detail, '2 codings moved');
  assert.equal(t('code.split', null, { new_code_id: 3, coding_ids: [4] }, 1), 'Split the code Welcome: Access is the new code');
  assert.equal(t('codebook.snapshot', null, { version: 2, note: 'after pilot', codes: 8 }), 'Saved codebook version 2: after pilot');
  assert.equal(t('memo.add', null, { kind: 'reflexive', title: 'My stance' }), "Added a reflexive journal memo: 'My stance'");
  assert.equal(t('memo.delete', { title: 'Gone' }, null), "Deleted a memo: 'Gone'");
  const var1 = { name: 'Age band', level: 'ordinal', derived: null };
  const cell = (n: number) => Array.from({ length: n }, (_, i) => ({ case_id: i + 1, value: '1' }));
  assert.equal(t('variable.apply', { def: null, values: [] }, { def: var1, values: [] }), "Added the variable 'Age band'");
  assert.equal(t('variable.apply', { def: null, values: [] }, { def: { ...var1, derived: { made_with: 'formula' } }, values: cell(6) }), "Made the variable 'Age band'");
  assert.equal(describeEvent(ev('variable.apply', { def: null, values: [] }, { def: var1, values: cell(1) }, null), lookups).detail, '1 case has a value');
  assert.equal(t('variable.apply', { def: var1, values: cell(6) }, { def: null, values: [] }), "Deleted the variable 'Age band'");
  assert.equal(t('variable.apply', { def: var1, values: [] }, { def: { ...var1, name: 'Age group' }, values: [] }), "Renamed the variable 'Age band' to 'Age group'");
  assert.equal(t('variable.apply', { def: var1, values: [] }, { def: { ...var1, level: 'nominal' }, values: [] }), "Changed what the variable 'Age band' says");
  assert.equal(t('variable.apply', { def: var1, values: cell(1) }, { def: var1, values: cell(1) }), "Changed 'Age band' for one case");
  assert.equal(t('variable.apply', { def: var1, values: cell(3) }, { def: var1, values: cell(3) }), "Changed 'Age band' for 3 cases");
  assert.equal(t('variable.apply', { def: var1, values: cell(6) }, { def: { ...var1, derived: { made_with: 'formula' } }, values: cell(6) }), "Made the variable 'Age band' again");
  assert.equal(categoryOf('variable.apply'), 'data');
  assert.equal(t('source.import', null, { name: 'Interview 1', characters: 12345 }), "Imported the source 'Interview 1'");
  assert.equal(t('annotation', null, { note: 'Switched to unit coding after pilot.' }), 'Note: Switched to unit coding after pilot.');
  assert.equal(t('code_set.add', null, null), 'Made a set of codes');
  assert.equal(t('thing.made_up', null, null), 'Thing made up', 'an action with no wording of its own is told from its name');
  assert.equal(t('case.add', null, { name: 'Commuters' }), "Added the case 'Commuters'");
  assert.equal(t('case.assign', null, null), 'Put a document in a case');
});

test('revisions and AI use are told in plain words, and an AI event is a coding event', () => {
  const d = (action: string, after: EventRow['after'], target: number | null = null) => describeEvent(ev(action, null, after, target), lookups);
  assert.equal(d('source.revise', { from_file: 'talk-v2.docx', codings_moved: 3, codings_removed: 1 }, 7).text, "Replaced the text of 'Interview 1' with a newer version (talk-v2.docx)");
  assert.equal(d('source.revise', { from_file: 'talk-v2.docx', codings_moved: 1, codings_removed: 0 }, 7).detail, '1 coding moved, 0 removed');
  assert.equal(d('source.edit', { codings_moved: 2, codings_removed: 0 }, 7).text, 'Edited the text of a document');
  assert.equal(d('ai.packet', { passages: 40, codes: 5 }).text, 'Built an AI prompt packet of 40 passages and 5 codes');
  assert.equal(d('ai.packet', { passages: 1, codes: 1 }).detail, 'nothing was sent anywhere');
  assert.equal(d('ai.answer', { model: 'Test Bot', suggested: 3, rejected: 1 }).text, 'Pasted an answer from Test Bot to an AI prompt packet');
  assert.equal(d('ai.answer', { model: 'Test Bot', suggested: 3, rejected: 1 }).detail, '3 passed the check, 1 refused');
  assert.equal(d('project.ai_mode', { ai_mode: 'off' }).text, 'Switched AI prompt packets off for this project');
  assert.equal(d('project.ai_mode', { ai_mode: 'packet' }).text, 'Allowed AI prompt packets in this project');
  assert.equal(d('source.share', { do_not_share: true }, 7).text, "Marked 'Interview 1' as never to be put in an AI prompt packet");
  assert.equal(categoryOf('ai.answer'), 'coding');
  assert.equal(categoryOf('source.revise'), 'data');
});

test('long quotes are shortened and white space is tidied', () => {
  assert.equal(quoted('  one \n two  '), "'one two'");
  const long = quoted('word '.repeat(60));
  assert.ok(long.length <= 85, long);
  assert.ok(long.endsWith("...'"));
});

test('events fall into categories', () => {
  assert.equal(categoryOf('coding.add'), 'coding');
  assert.equal(categoryOf('anchor.remove'), 'coding');
  assert.equal(categoryOf('counterexample.add'), 'coding');
  assert.equal(categoryOf('code.merge'), 'codebook');
  assert.equal(categoryOf('memo.update'), 'memo');
  assert.equal(categoryOf('annotation'), 'note');
  assert.equal(categoryOf('source.import'), 'data');
  assert.equal(categoryOf('mystery'), 'other');
});

test('times are shown in a readable way', () => {
  assert.equal(formatWhen('2026-11-04T15:05:00.000Z', 'en-US', 'UTC'), 'Nov 4, 2026, 3:05 PM');
  assert.equal(formatWhen('garbage'), 'garbage');
});

// ---------------------------------------------------------------- coverage and units

test('covered length merges overlapping codings', () => {
  assert.equal(coveredLength([{ start: 0, end: 5 }, { start: 3, end: 8 }, { start: 10, end: 12 }]), 10);
  assert.equal(coveredLength([]), 0);
  assert.equal(coveredLength([{ start: 5, end: 5 }]), 0);
});

test('coverage by characters for free-span projects and by units for unit projects', () => {
  const codings = [{ start: 0, end: 25, unit_id: null }, { start: 20, end: 50, unit_id: null }];
  const free = computeCoverage({ characters: 100, units: 4 }, codings, 'free_span');
  assert.deepEqual([free.by, free.coded, free.total, free.fraction], ['characters', 50, 100, 0.5]);
  const unitCodings = [{ start: 0, end: 10, unit_id: 1 }, { start: 2, end: 5, unit_id: 1 }, { start: 10, end: 20, unit_id: 2 }];
  const byCount = computeCoverage({ characters: 40, units: 4 }, unitCodings, 'unit');
  assert.deepEqual([byCount.by, byCount.coded, byCount.total], ['units', 2, 4]);
  const units = [1, 2, 3, 4].map((id) => ({ id, start: (id - 1) * 10, end: id * 10, excluded: id === 4 }));
  const precise = computeCoverage({ characters: 40, units: 4 }, unitCodings, 'unit', units);
  assert.deepEqual([precise.coded, precise.total], [2, 3], 'excluded units do not count');
  assert.equal(describeCoverage(precise, 3), '2 of 3 units coded');
  assert.equal(describeCoverage(free, 2), '50% of the text coded');
  assert.equal(describeCoverage(null, 0), 'Nothing coded yet');
  assert.equal(describeCoverage(null, 4), '4 codings');
});

const units: UnitRow[] = [0, 1, 2, 3, 4].map((i) => ({ id: i + 10, source_id: 1, kind: 'sentence', start: i * 10, end: i * 10 + 8, speaker: null, parent_id: null, excluded: i === 3 }));

test('finding the unit at a position', () => {
  assert.equal(unitIndexAt(units, 0), 0);
  assert.equal(unitIndexAt(units, 7), 0);
  assert.equal(unitIndexAt(units, 8), -1, 'the gap between units');
  assert.equal(unitIndexAt(units, 25), 2);
  assert.equal(unitIndexAt(units, 999), -1);
  assert.equal(unitIndexForCaret(units, 8), 0, 'a caret right after a unit still belongs to it');
  assert.equal(unitIndexForCaret(units, 10), 1);
  assert.deepEqual(unitIndicesTouching(units, 5, 22), [0, 1, 2]);
  assert.deepEqual(unitIndicesTouching(units, 8, 10), []);
  assert.deepEqual(unitIndicesTouching(units, 9, 12), [1]);
});

test('next and previous uncoded unit skip coded and excluded units', () => {
  const coded = new Set([11]);
  assert.equal(nextUncodedUnit(units, coded, 0), 2);
  assert.equal(nextUncodedUnit(units, coded, 2), 4, 'unit 3 is excluded');
  assert.equal(nextUncodedUnit(units, coded, 4), -1);
  assert.equal(nextUncodedUnit(units, coded, 4, -1), 2);
  assert.equal(nextUncodedUnit(units, coded, 2, -1), 0);
  assert.equal(nextUncodedUnit(units, coded, 0, -1), -1);
});

// ---------------------------------------------------------------- threads

test('replies are listed under their parents and indented', () => {
  const mk = (id: number, parent: number | null): UnitRow => ({ id, source_id: 1, kind: 'comment', start: id * 10, end: id * 10 + 5, speaker: null, parent_id: parent, excluded: false });
  const rows = threadRows([mk(1, null), mk(2, 1), mk(3, null), mk(4, 2), mk(5, 1)]);
  assert.deepEqual(rows.map((r) => [r.unit.id, r.depth]), [[1, 0], [2, 1], [4, 2], [5, 1], [3, 0]]);
  assert.deepEqual(rows.map((r) => r.index), [0, 1, 3, 4, 2]);
  assert.equal(hasReplies([mk(1, null)]), false);
  assert.equal(hasReplies([mk(1, null), mk(2, 1)]), true);
});

test('a missing parent, a loop, and a very deep thread do not break the list', () => {
  const mk = (id: number, parent: number | null): UnitRow => ({ id, source_id: 1, kind: 'comment', start: id, end: id + 1, speaker: null, parent_id: parent, excluded: false });
  const orphan = threadRows([mk(1, 99), mk(2, 1)]);
  assert.deepEqual(orphan.map((r) => [r.unit.id, r.depth]), [[1, 0], [2, 1]]);
  const loop = threadRows([mk(1, 2), mk(2, 1), mk(3, 3)]);
  assert.equal(loop.length, 3, 'every unit is still listed');
  const deep = Array.from({ length: 20000 }, (_, i) => mk(i + 1, i === 0 ? null : i));
  const rows = threadRows(deep);
  assert.equal(rows.length, 20000);
  assert.equal(rows[19999].depth, 19999);
});

// ---------------------------------------------------------------- import mapping

const table: TablePreview = {
  name: 'survey',
  row_count: 3,
  columns: [
    { name: 'pid', label: null, guessed_level: 'nominal', looks_like: 'text', distinct: 3, empty: 0, mean_words: 1, samples: [], suggested_role: 'id' },
    { name: 'group', label: null, guessed_level: 'nominal', looks_like: 'text', distinct: 2, empty: 0, mean_words: 1, samples: [], suggested_role: 'attribute' },
    { name: 'age', label: null, guessed_level: 'ratio', looks_like: 'number', distinct: 3, empty: 0, mean_words: 1, samples: [], suggested_role: 'attribute' },
    { name: 'answer', label: null, guessed_level: 'text', looks_like: 'text', distinct: 3, empty: 0, mean_words: 9, samples: [], suggested_role: 'text' },
  ],
};

test('the wizard starts from the engine suggestion and builds the mapping back', () => {
  const suggested = { id_column: 'pid', text_columns: ['answer'], attribute_columns: [{ name: 'group', level: null }, { name: 'age', level: null }] };
  const choices = choicesFromSuggestion(table, suggested);
  assert.equal(choices.pid.role, 'id');
  assert.equal(choices.answer.role, 'text');
  assert.equal(choices.age.level, 'ratio');
  const mapping = mappingFromChoices(table, choices);
  assert.equal(mapping.id_column, 'pid');
  assert.deepEqual(mapping.text_columns, ['answer']);
  assert.deepEqual(mapping.attribute_columns, [{ name: 'group', level: 'nominal' }, { name: 'age', level: 'ratio' }]);
  assert.equal(validateChoices(choices), null);
});

test('choosing a new person ID column replaces the old one; levels can be changed', () => {
  const start = choicesFromSuggestion(table, { id_column: 'pid', text_columns: ['answer'] });
  const next = setRole(start, 'group', 'id');
  assert.equal(next.pid.role, 'unused');
  assert.equal(next.group.role, 'id');
  const level = setLevel(setRole(next, 'age', 'unit_attribute'), 'age', 'interval');
  const mapping = mappingFromChoices(table, level);
  assert.deepEqual(mapping.unit_attribute_columns, [{ name: 'age', level: 'interval' }]);
  assert.equal(mapping.id_column, 'group');
});

test('the wizard asks for at least one text column in plain words', () => {
  const none = choicesFromSuggestion(table, { text_columns: [] });
  assert.match(validateChoices(none) ?? '', /open-ended text/);
});

// ---------------------------------------------------------------- favorites and paths

test('favorites keep their numbers when one is removed', () => {
  let f = normalizeFavorites([10, 11, 12]);
  assert.equal(codeForSlot(f, 2), 11);
  f = unpin(f, 11);
  assert.deepEqual(f, [10, null, 12]);
  assert.equal(codeForSlot(f, 2), null);
  const re = pin(f, 20);
  assert.deepEqual(re, [10, 20, 12], 'the free slot is reused');
  assert.equal(slotOf(re!, 12), 3);
  assert.equal(pin([1, 2, 3, 4, 5, 6, 7, 8, 9], 10), null, 'only nine fit');
  assert.deepEqual(pin([1], 1), [1], 'pinning twice changes nothing');
  assert.deepEqual(unpin([5], 5), []);
});

test('stored favorites are cleaned: unknown, repeated, and non-number entries go', () => {
  assert.deepEqual(normalizeFavorites([1, 1, 'x', 2, null, 99], new Set([1, 2])), [1, null, null, 2]);
  assert.deepEqual(normalizeFavorites('nonsense'), []);
  assert.equal(normalizeFavorites([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]).length, 9);
});

test('typed paths lose the quotes Windows adds when copying a path', () => {
  assert.equal(cleanTypedPath('  "C:\\Data\\notes.txt"  '), 'C:\\Data\\notes.txt');
  assert.equal(cleanTypedPath("'/tmp/a b.txt'"), '/tmp/a b.txt');
  assert.equal(cleanTypedPath('plain.txt'), 'plain.txt');
  assert.equal(cleanTypedPath('   '), '');
});

test('suggestions are told in words, and a bulk decision is one line instead of two for every suggestion', () => {
  const t = (e: EventRow) => describeEvent(e, lookups).text;
  assert.equal(t(ev('proposal.create', null, { suggestions: 12, label: 'Words: cost' })), "Auto Code suggested a code for 12 places ('Words: cost')");
  assert.equal(t(ev('proposal.adjust', { start: 1 }, { start: 2 })), 'Changed a suggestion before reviewing it');
  assert.equal(t(ev('undo', null, { what: 'Took back 3 accepted suggestions' })), 'Undid: Took back 3 accepted suggestions');
  assert.equal(categoryOf('proposal.accept'), 'coding');

  const withId = (id: number, e: EventRow): EventRow => ({ ...e, id });
  const events: EventRow[] = [
    withId(1, ev('coding.add', null, { source_id: 7, start: 0, end: 4, code_id: 1, via_proposal: 10 })),
    withId(2, ev('proposal.accept', null, { coding_id: 1, decision: 'aa' })),
    withId(3, ev('coding.add', null, { source_id: 7, start: 5, end: 9, code_id: 1, via_proposal: 11 })),
    withId(4, ev('proposal.accept', null, { coding_id: 2, decision: 'aa' })),
    withId(5, ev('coding.add', null, { source_id: 7, start: 10, end: 12, code_id: 1 })),
    withId(6, ev('proposal.accept', null, { coding_id: 3, decision: 'bb' })),
    withId(7, ev('undo', null, { undone_event: 4, what: 'Took back 2 accepted suggestions' })),
    withId(8, ev('undo', null, { undone_event: 2, with: 7 })),
  ];
  const folded = foldEvents(events);
  assert.deepEqual(folded.map((e) => e.id), [4, 5, 6, 7], 'the codings made by suggestions and the undo extras are folded away');
  assert.equal(t(folded[0]), 'Accepted 2 suggested codings');
  assert.equal(t(folded[2]), 'Accepted a suggested coding', 'a different call stays its own line');
  assert.equal(t(folded[1]), "Coded some text as Welcome", 'a coding made by hand stays');
});
