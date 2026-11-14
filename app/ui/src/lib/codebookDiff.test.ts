import assert from 'node:assert/strict';
import { test } from 'node:test';
import { diffCodebooks, isSame, summarizeDiff, type SnapCode } from './codebookDiff';

const code = (id: number, name: string, extra: Partial<SnapCode> = {}): SnapCode => ({
  id,
  name,
  parent_id: null,
  kind: 'code',
  color: '#0072B2',
  definition: '',
  inclusion_criteria: '',
  exclusion_criteria: '',
  status: 'active',
  ...extra,
});

test('the same codebook has no differences', () => {
  const a = [code(1, 'Cost'), code(2, 'Reliability', { parent_id: 1 })];
  const d = diffCodebooks(a, a.map((c) => ({ ...c })));
  assert.equal(isSame(d), true);
  assert.equal(summarizeDiff(d), 'No differences.');
});

test('codes that are only in the later codebook are added, and only in the earlier are removed', () => {
  const d = diffCodebooks([code(1, 'Cost'), code(2, 'Old')], [code(1, 'Cost'), code(3, 'Safety')]);
  assert.deepEqual(d.added.map((c) => c.name), ['Safety']);
  assert.deepEqual(d.removed.map((c) => c.name), ['Old']);
  assert.deepEqual(d.changed, []);
  assert.equal(summarizeDiff(d), '1 code added, 1 code removed.');
});

test('a renamed code is one change, because a code keeps its identity when its name changes', () => {
  const d = diffCodebooks([code(1, 'Cost')], [code(1, 'Affordability')]);
  assert.equal(d.added.length + d.removed.length, 0);
  assert.equal(d.changed.length, 1);
  assert.deepEqual(d.changed[0].changes, [{ field: 'name', label: 'Title', from: 'Cost', to: 'Affordability' }]);
  assert.equal(d.changed[0].name, 'Affordability');
});

test('a changed description and a changed place in the hierarchy are both found, with the hierarchy told in words', () => {
  const before = [code(1, 'Barriers'), code(2, 'Access'), code(3, 'Reliability', { parent_id: 1, definition: 'Late buses.' })];
  const after = [code(1, 'Barriers'), code(2, 'Access'), code(3, 'Reliability', { parent_id: 2, definition: 'Delayed, canceled, or unpredictable service.' })];
  const d = diffCodebooks(before, after);
  assert.equal(d.changed.length, 1);
  const fields = d.changed[0].changes.map((c) => [c.label, c.from, c.to]);
  assert.deepEqual(fields, [
    ['Place in the hierarchy', 'under Barriers', 'under Access'],
    ['Description', 'Late buses.', 'Delayed, canceled, or unpredictable service.'],
  ]);
});

test('retiring a code, and moving it to the top level, are told in words', () => {
  const d = diffCodebooks([code(1, 'A'), code(2, 'B', { parent_id: 1 })], [code(1, 'A'), code(2, 'B', { status: 'retired' })]);
  const fields = d.changed[0].changes.map((c) => [c.label, c.from, c.to]);
  assert.deepEqual(fields, [
    ['Place in the hierarchy', 'under A', '(top level)'],
    ['State', 'Active', 'Retired'],
  ]);
});

test('a code whose group or rating scale changed is a change, and an older version with no properties reads as the defaults', () => {
  const d = diffCodebooks(
    [code(1, 'Positive')],
    [code(1, 'Positive', { exclusive_group: 'Sentiment', rated: true, rating_label: 'Intensity', rating_low: 'A hint', rating_high: 'Overwhelming' })],
  );
  assert.deepEqual(
    d.changed[0].changes.map((c) => [c.label, c.from, c.to]),
    [
      ['Mutually Exclusive Group', 'None', 'Sentiment'],
      ['Weighted / Rated', 'No', 'Yes'],
      ['Rating name', '', 'Intensity'],
      ['A 1 means', '', 'A hint'],
      ['A 5 means', '', 'Overwhelming'],
    ],
  );
  // A version saved before the properties existed (no fields) against today's defaults is not a change.
  assert.equal(isSame(diffCodebooks([code(1, 'Cost')], [code(1, 'Cost', { exclusive_group: '', rated: false, rating_label: '' })])), true);
});
