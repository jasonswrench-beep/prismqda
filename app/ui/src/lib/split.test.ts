import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { SourceRow } from '../types';
import { defaultSplitTarget, splitChoices, splitProblem } from './split';

const src = (id: number, name: string, folder = ''): SourceRow => ({
  id, name, source_type: 'document', folder, characters: 100, default_unit_kind: 'sentence', text_revision: 1, units: 1, codings: 0, do_not_share: false, created_at: '',
});

test('the document beside is the next one in the folder, else the one before, else any other', () => {
  const list = [src(1, 'A'), src(2, 'B'), src(3, 'C'), src(4, 'D', 'Notes')];
  assert.equal(defaultSplitTarget(list, 1), 2, 'the next in its folder');
  assert.equal(defaultSplitTarget(list, 3), 2, 'the last has none after it, so the one before');
  assert.equal(defaultSplitTarget(list, 4), 1, 'alone in its folder: any other document');
  assert.equal(defaultSplitTarget([src(1, 'A')], 1), null, 'one document has nothing to be shown beside');
  assert.equal(defaultSplitTarget(list, 99), 1, 'an unknown current document starts at the first');
  assert.equal(defaultSplitTarget([], 1), null);
});

test('the choices are every document but the one being read', () => {
  const list = [src(1, 'A'), src(2, 'B')];
  assert.deepEqual(splitChoices(list, 1).map((s) => s.id), [2]);
  assert.deepEqual(splitChoices(list, null).map((s) => s.id), [1, 2]);
});

test('a pane that cannot show its document says why, in words', () => {
  const a = src(1, 'A');
  assert.match(splitProblem(null, 1, false) ?? '', /no longer in the project/);
  assert.match(splitProblem(a, 1, false) ?? '', /being read on the left/);
  assert.match(splitProblem(a, 2, true) ?? '', /A is a large document/);
  assert.equal(splitProblem(a, 2, false), null);
});
