import assert from 'node:assert/strict';
import { test } from 'node:test';
import { BAR_FAVORITES, BAR_TOTAL, barChoices, edgeLines, placeBar, sameSelection } from './floatbar';

const bar = { width: 200, height: 32 };
const view = { top: 0, bottom: 400, width: 600 };

test('the bar goes above the first line, centered on it', () => {
  const first = { top: 100, bottom: 120, left: 200, right: 300 };
  const p = placeBar(first, first, bar, view);
  assert.equal(p.below, false);
  assert.equal(p.top, 100 - 8 - 32);
  assert.equal(p.left, 250 - 100);
});

test('with no room above, the bar goes under the last line', () => {
  const first = { top: 10, bottom: 30, left: 200, right: 300 };
  const last = { top: 50, bottom: 70, left: 50, right: 150 };
  const p = placeBar(first, last, bar, view);
  assert.equal(p.below, true);
  assert.equal(p.top, 70 + 8);
  assert.equal(p.left, 6, 'centered on the last line would be at 0, so it is kept inside the left edge');
});

test('the bar never leaves the sides of the text', () => {
  const near = { top: 200, bottom: 220, left: 0, right: 20 };
  assert.equal(placeBar(near, near, bar, view).left, 6);
  const far = { top: 200, bottom: 220, left: 590, right: 600 };
  assert.equal(placeBar(far, far, bar, view).left, 600 - 200 - 6);
});

test('a bar wider than the text sits at the left edge instead of going negative', () => {
  const first = { top: 200, bottom: 220, left: 10, right: 40 };
  const p = placeBar(first, first, { width: 800, height: 32 }, view);
  assert.equal(p.left, 6);
});

test('a selection that fills the view keeps the bar on screen', () => {
  const first = { top: -300, bottom: -280, left: 100, right: 200 };
  const last = { top: 700, bottom: 720, left: 100, right: 200 };
  const p = placeBar(first, last, bar, view);
  assert.ok(p.top >= view.top + 6 && p.top + bar.height <= view.bottom - 6, `on screen: ${p.top}`);
});

test('the first and last lines come from the rectangles, ignoring empty ones', () => {
  const rects = [
    { top: 40, bottom: 60, left: 10, right: 400 },
    { top: 0, bottom: 0, left: 0, right: 0 },
    { top: 20, bottom: 40, left: 300, right: 500 },
    { top: 60, bottom: 80, left: 10, right: 90 },
  ];
  const e = edgeLines(rects);
  assert.deepEqual(e?.first, rects[2]);
  assert.deepEqual(e?.last, rects[3]);
  assert.equal(edgeLines([{ top: 0, bottom: 0, left: 0, right: 0 }]), null);
  assert.equal(edgeLines([]), null);
});

const at = (id: number, name: string, status = 'active', parent_id: number | null = null, sort_order = id) => ({ id, name, color: '#E09F00', status, parent_id, sort_order });
const codes = [at(1, 'Welcome'), at(2, 'Cost'), at(3, 'Old', 'retired'), at(4, 'Merged', 'merged')];

test('the bar offers the favorites with their keys, and the last code when it is not one of them', () => {
  const c = barChoices(codes, [1, null, 2], 2);
  assert.deepEqual(c.favorites.map((f) => [f.slot, f.code.name]), [[1, 'Welcome'], [3, 'Cost']]);
  assert.equal(c.last, null, 'Cost is already shown as a favorite');
  const d = barChoices(codes, [1], 2);
  assert.equal(d.last?.name, 'Cost');
});

test('retired and merged codes are not offered, and a code that is gone is skipped', () => {
  const c = barChoices(codes, [3, 4, 99, 1], 3);
  assert.deepEqual(c.favorites.map((f) => f.code.id), [1]);
  assert.equal(c.last, null);
  assert.deepEqual(c.others.map((o) => o.id), [2], 'only the active code that is left is offered as another');
});

test('no more than six favorites are shown; the keys still say which they are', () => {
  const many = Array.from({ length: 9 }, (_, i) => at(i + 10, `Code ${i + 1}`));
  const c = barChoices(many, many.map((m) => m.id), null);
  assert.equal(c.favorites.length, BAR_FAVORITES);
  assert.equal(c.favorites[5].slot, 6);
});

test('with nothing pinned the bar offers the codes in codebook order, so it is useful from the first code', () => {
  const six = [at(1, 'Welcome'), at(2, 'Cost'), at(3, 'Financial aid', 'active', 2), at(4, 'Belonging'), at(5, 'Advising'), at(6, 'Schedule')];
  const c = barChoices(six, [], null);
  assert.equal(c.last, null);
  assert.deepEqual(c.favorites, []);
  assert.deepEqual(c.others.map((o) => o.name), ['Welcome', 'Cost', 'Financial aid', 'Belonging', 'Advising', 'Schedule'], 'a code is followed by the codes under it');
});

test('the bar holds eight codes at most, favorites first and then the last code, then the rest', () => {
  const many = Array.from({ length: 20 }, (_, i) => at(i + 1, `Code ${i + 1}`));
  const none = barChoices(many, [], null);
  assert.equal(none.others.length, BAR_TOTAL);
  const c = barChoices(many, [5, null, 9], 12);
  assert.deepEqual(c.favorites.map((f) => [f.slot, f.code.id]), [[1, 5], [3, 9]]);
  assert.equal(c.last?.id, 12);
  assert.equal(c.favorites.length + 1 + c.others.length, BAR_TOTAL, 'favorites, the last code, and the others fill the bar');
  assert.ok(!c.others.some((o) => o.id === 5 || o.id === 9 || o.id === 12), 'nothing is shown twice');
  assert.deepEqual(c.others.map((o) => o.id), [1, 2, 3, 4, 6]);
});

test('the others follow the sibling order the codebook uses, not the order the codes were made', () => {
  const list = [at(1, 'Zeta', 'active', null, 2), at(2, 'Alpha', 'active', null, 1), at(3, 'Beta', 'active', 2, 1)];
  assert.deepEqual(barChoices(list, [], null).others.map((o) => o.name), ['Alpha', 'Beta', 'Zeta']);
});

test('a selection is the same only when both ends match', () => {
  assert.equal(sameSelection({ start: 1, end: 5 }, { start: 1, end: 5 }), true);
  assert.equal(sameSelection({ start: 1, end: 5 }, { start: 1, end: 6 }), false);
  assert.equal(sameSelection(null, { start: 1, end: 5 }), false);
});
