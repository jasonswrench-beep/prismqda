import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OffsetIndex } from '../offsets';
import { codingsAtCaret, codingsIn, paragraphsOf, scopeAt } from './scope';
import type { CodingRow, UnitRow } from '../types';

const text = 'First paragraph here.\nStill the first.\n\nSecond \u{1F600} paragraph.\n\n\nThird one.';
const index = new OffsetIndex(text);

const coding = (id: number, start: number, end: number): CodingRow => ({ id, source_id: 1, start, end, code_id: 1, coder_id: 1, origin: 'human', layer: 'coder', rationale: '', unit_id: null });

test('paragraphs are separated by blank lines, and offsets are in scalar values', () => {
  const p = paragraphsOf(text, index);
  assert.equal(p.length, 3);
  assert.equal(Array.from(text).slice(p[0].start, p[0].end).join(''), 'First paragraph here.\nStill the first.');
  assert.equal(Array.from(text).slice(p[1].start, p[1].end).join(''), 'Second \u{1F600} paragraph.');
  assert.equal(Array.from(text).slice(p[2].start, p[2].end).join(''), 'Third one.');
  assert.deepEqual(paragraphsOf('', new OffsetIndex('')), []);
  assert.deepEqual(paragraphsOf('   \n\n  ', new OffsetIndex('   \n\n  ')), []);
});

test('the scope is the paragraph at the caret in a free-span project', () => {
  const paragraphs = paragraphsOf(text, index);
  const s = scopeAt({ paragraphs, units: [], mode: 'free_span', caret: 5 });
  assert.equal(s.kind, 'paragraph');
  assert.equal(s.label, 'Paragraph 1 of 3');
  const second = scopeAt({ paragraphs, units: [], mode: 'free_span', caret: paragraphs[1].start + 3 });
  assert.equal(second.label, 'Paragraph 2 of 3');
  // A caret in the blank line between paragraphs belongs to the paragraph before it.
  const gap = scopeAt({ paragraphs, units: [], mode: 'free_span', caret: paragraphs[0].end + 1 });
  assert.equal(gap.label, 'Paragraph 1 of 3');
  assert.equal(scopeAt({ paragraphs: [], units: [], mode: 'free_span', caret: 0 }).kind, 'none');
});

test('the scope is the unit at the caret in a unit project', () => {
  const units: UnitRow[] = [
    { id: 1, source_id: 1, kind: 'speaker_turn', start: 0, end: 21, speaker: 'Interviewer', parent_id: null, excluded: true },
    { id: 2, source_id: 1, kind: 'speaker_turn', start: 22, end: 38, speaker: null, parent_id: null, excluded: false },
  ];
  const s = scopeAt({ paragraphs: paragraphsOf(text, index), units, mode: 'unit', caret: 3 });
  assert.equal(s.kind, 'unit');
  assert.equal(s.label, 'Unit 1 of 2 (Interviewer)');
  assert.equal(scopeAt({ paragraphs: paragraphsOf(text, index), units, mode: 'unit', caret: 30 }).label, 'Unit 2 of 2');
  const outside = scopeAt({ paragraphs: paragraphsOf(text, index), units, mode: 'unit', caret: 50 });
  assert.equal(outside.kind, 'paragraph', 'outside every unit it falls back to the paragraph');
});

test('codings in a span and at a caret', () => {
  const cs = [coding(1, 0, 10), coding(2, 5, 8), coding(3, 20, 30)];
  assert.deepEqual(codingsIn(cs, { start: 6, end: 22 }).map((c) => c.id), [1, 2, 3]);
  assert.deepEqual(codingsIn(cs, { start: 10, end: 20 }).map((c) => c.id), []);
  assert.deepEqual(codingsAtCaret(cs, 6, 100).map((c) => c.id), [2, 1], 'the smallest coding comes first');
  assert.deepEqual(codingsAtCaret(cs, 10, 100).map((c) => c.id), []);
  assert.deepEqual(codingsAtCaret(cs, 30, 30).map((c) => c.id), [3], 'at the very end of the text');
});
