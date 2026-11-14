import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OffsetIndex } from '../offsets';
import { documentTarget, unitsTarget } from './targets';
import type { UnitRow } from '../types';

const text = 'I felt welcome. The cost was hard. \u{1F600} Thanks.';
const index = new OffsetIndex(text);
const unit = (id: number, start: number, end: number, excluded = false): UnitRow => ({
  id, source_id: 1, kind: 'sentence', start, end, speaker: null, parent_id: null, excluded,
});
// Scalar offsets: "I felt welcome." is 0..15, "The cost was hard." is 16..34, then the emoji sentence.
const units = [unit(1, 0, 15), unit(2, 16, 34), unit(3, 35, 47, true)];

test('free-span: the selected words are coded, without the spaces at the ends', () => {
  const r = documentTarget({ text, index, selection: { start: 1, end: 14 }, mode: 'free_span', units });
  assert.ok(r.ok);
  if (r.ok) assert.deepEqual(r.spans, [{ start: 2, end: 14, unit_id: null }]);
});

test('free-span: nothing selected, or only spaces, gives a plain message', () => {
  const none = documentTarget({ text, index, selection: { start: 5, end: 5 }, mode: 'free_span', units });
  assert.equal(none.ok, false);
  if (!none.ok) assert.match(none.message, /Select some words first/);
  const spaces = documentTarget({ text, index, selection: { start: 15, end: 16 }, mode: 'free_span', units });
  assert.equal(spaces.ok, false);
  if (!spaces.ok) assert.match(spaces.message, /Only spaces/);
  assert.equal(documentTarget({ text, index, selection: null, mode: 'free_span', units }).ok, false);
});

test('unit mode: a caret with nothing selected codes the whole unit', () => {
  const r = documentTarget({ text, index, selection: { start: 20, end: 20 }, mode: 'unit', units });
  assert.ok(r.ok);
  if (r.ok) assert.deepEqual(r.spans, [{ start: 16, end: 34, unit_id: 2 }]);
});

test('unit mode: words inside one unit are coded as a part of that unit', () => {
  const r = documentTarget({ text, index, selection: { start: 2, end: 14 }, mode: 'unit', units });
  assert.ok(r.ok);
  if (r.ok) assert.deepEqual(r.spans, [{ start: 2, end: 14, unit_id: 1 }]);
});

test('unit mode: a selection across units codes each unit it touches, clipped to the unit', () => {
  const r = documentTarget({ text, index, selection: { start: 10, end: 25 }, mode: 'unit', units });
  assert.ok(r.ok);
  if (r.ok) assert.deepEqual(r.spans, [{ start: 10, end: 15, unit_id: 1 }, { start: 16, end: 24, unit_id: 2 }]);
});

test('unit mode: excluded units are skipped and counted; all-excluded gives a message', () => {
  const r = documentTarget({ text, index, selection: { start: 20, end: 45 }, mode: 'unit', units });
  assert.ok(r.ok);
  if (r.ok) {
    assert.equal(r.spans.length, 1);
    assert.equal(r.skipped, 1);
  }
  const only = documentTarget({ text, index, selection: { start: 36, end: 40 }, mode: 'unit', units });
  assert.equal(only.ok, false);
  if (!only.ok) assert.match(only.message, /left out of coding/);
  const caret = documentTarget({ text, index, selection: { start: 40, end: 40 }, mode: 'unit', units });
  assert.equal(caret.ok, false);
});

test('unit mode: no units, caret outside a unit, no selection', () => {
  assert.equal(documentTarget({ text, index, selection: { start: 1, end: 2 }, mode: 'unit', units: [] }).ok, false);
  const gap = documentTarget({ text, index, selection: { start: 15, end: 15 }, mode: 'unit', units: [unit(1, 0, 14), unit(2, 16, 34)] });
  assert.equal(gap.ok, false);
  assert.equal(documentTarget({ text, index, selection: null, mode: 'unit', units }).ok, false);
});

test('grid rows become whole-unit spans', () => {
  const r = unitsTarget(units);
  assert.ok(r.ok);
  if (r.ok) {
    assert.deepEqual(r.spans.map((s) => s.unit_id), [1, 2]);
    assert.equal(r.skipped, 1);
  }
  assert.equal(unitsTarget([]).ok, false);
  assert.equal(unitsTarget([unit(3, 35, 47, true)]).ok, false);
});
