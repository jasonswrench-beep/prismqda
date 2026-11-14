import assert from 'node:assert/strict';
import test from 'node:test';
import type { CodeRow, SegmentRow, SourceRow } from '../types';
import { assignLanes, buildPortraits, MAX_LANES, share, unionLength } from './portrait';

const code = (id: number, name: string, color: string): CodeRow => ({ id, name, color, status: 'active', parent_id: null }) as unknown as CodeRow;
const source = (id: number, name: string, characters: number): SourceRow => ({ id, name, characters }) as unknown as SourceRow;
const seg = (id: number, sourceId: number, codeId: number, start: number, end: number, kind: SegmentRow['kind'] = 'text'): SegmentRow =>
  ({ coding_id: id, source_id: sourceId, code_id: codeId, code_name: `code${codeId}`, start, end, text: `words ${id}`, kind }) as unknown as SegmentRow;

test('a position is a share of the document, kept inside 0 and 100', () => {
  assert.equal(share(50, 200), 25);
  assert.equal(share(-5, 200), 0);
  assert.equal(share(500, 200), 100);
  assert.equal(share(3, 0), 0);
});

test('the union of intervals counts a stretch once however many passages cover it', () => {
  assert.equal(unionLength([]), 0);
  assert.equal(unionLength([{ start: 0, end: 10 }, { start: 5, end: 15 }, { start: 20, end: 30 }]), 25);
  assert.equal(unionLength([{ start: 0, end: 10 }, { start: 2, end: 4 }]), 10);
  assert.equal(unionLength([{ start: 10, end: 20 }, { start: 0, end: 10 }]), 20, 'passages that touch add up');
});

test('overlapping passages go in different rows, and passages that do not overlap share one', () => {
  assert.deepEqual(assignLanes([{ start: 0, end: 10 }, { start: 20, end: 30 }]), [0, 0]);
  assert.deepEqual(assignLanes([{ start: 0, end: 10 }, { start: 5, end: 15 }, { start: 8, end: 9 }]), [0, 1, 2]);
  assert.deepEqual(assignLanes([{ start: 0, end: 10 }, { start: 10, end: 20 }]), [0, 0], 'one that starts where another ends does not overlap it');
  const many = Array.from({ length: 10 }, () => ({ start: 0, end: 10 }));
  const lanes = assignLanes(many);
  assert.equal(Math.max(...lanes), MAX_LANES - 1, 'beyond the limit they share the last row');
});

test('a portrait is made for each document with coded words, with the share coded and what each code does', () => {
  const codes = [code(1, 'Welcome', '#ff0000'), code(2, 'Cost', '#0000ff')];
  const sources = [source(10, 'first', 100), source(11, 'empty', 50), source(12, 'second', 200)];
  const segments = [seg(1, 10, 1, 0, 20), seg(2, 10, 2, 10, 30), seg(3, 10, 1, 60, 70), seg(4, 12, 2, 100, 150), seg(5, 12, 1, 0, 5, 'image'), seg(6, 12, 1, 0, 5, 'clip')];
  const p = buildPortraits(segments, sources, codes);
  assert.deepEqual(p.map((x) => x.name), ['first', 'second'], 'a document with no coded words has no portrait, and neither does a picture area or a stretch of a recording');
  const first = p[0];
  assert.equal(first.coverage, 40, '0 to 30 and 60 to 70 of 100');
  assert.equal(first.lanes, 2, 'the first two passages overlap');
  assert.deepEqual(first.marks.map((m) => [m.codingId, m.lane]), [[1, 0], [2, 1], [3, 0]]);
  assert.equal(first.marks[0].color, '#ff0000');
  const welcome = first.uses.find((u) => u.name === 'Welcome')!;
  assert.deepEqual([welcome.passages, welcome.characters, welcome.from, welcome.to], [2, 30, 0, 70]);
  assert.equal(p[1].coverage, 25);
});

test('codes can be left out, and a document with only those codes disappears', () => {
  const codes = [code(1, 'A', '#111111'), code(2, 'B', '#222222')];
  const sources = [source(10, 'one', 100)];
  const segments = [seg(1, 10, 1, 0, 10), seg(2, 10, 2, 20, 30)];
  assert.equal(buildPortraits(segments, sources, codes, new Set([1]))[0].uses.length, 1);
  assert.equal(buildPortraits(segments, sources, codes, new Set([1, 2])).length, 0);
});

test('a document with more passages than can be drawn says how many were not', () => {
  const codes = [code(1, 'A', '#111111')];
  const sources = [source(10, 'big', 10000)];
  const segments = Array.from({ length: 650 }, (_, i) => seg(i + 1, 10, 1, i * 10, i * 10 + 5));
  const p = buildPortraits(segments, sources, codes)[0];
  assert.equal(p.marks.length, 600);
  assert.equal(p.notDrawn, 50);
  assert.equal(p.uses[0].passages, 650, 'the table still counts every passage');
});
