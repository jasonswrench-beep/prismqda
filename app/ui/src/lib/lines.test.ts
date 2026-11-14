import assert from 'node:assert/strict';
import { test } from 'node:test';
import { firstLineBelow, groupLines, lineLabel, lineRangeFor, linesBetween } from './lines';

// Three lines of text 30 pixels apart, as the browser reports them: several pieces on the same line, a short piece
// at the end of a line, and an empty rectangle for a blank line.
const rects = [
  { top: 10, bottom: 32, left: 0, right: 200 },
  { top: 11, bottom: 31, left: 200, right: 380 },
  { top: 40, bottom: 62, left: 0, right: 90 },
  { top: 70, bottom: 92, left: 0, right: 0 }, // blank line: no width
  { top: 100, bottom: 122, left: 0, right: 310 },
  { top: 100, bottom: 122, left: 310, right: 340 },
];

test('pieces on one line become one line; an empty rectangle is not a line', () => {
  const lines = groupLines(rects);
  assert.deepEqual(lines.map((l) => [l.n, l.top, l.bottom]), [[1, 10, 32], [2, 40, 62], [3, 100, 122]]);
});

test('numbering counts lines as drawn, so a paragraph that wraps has a number for each line', () => {
  const wrapped = [10, 40, 70, 100].map((top) => ({ top, bottom: top + 22, left: 0, right: 300 }));
  assert.deepEqual(groupLines(wrapped).map((l) => l.n), [1, 2, 3, 4]);
});

test('the rectangles can come in any order', () => {
  const shuffled = [rects[4], rects[0], rects[2], rects[1], rects[3], rects[5]];
  assert.deepEqual(groupLines(shuffled).map((l) => [l.top, l.bottom]), [[10, 32], [40, 62], [100, 122]]);
});

test('no rectangles, no lines', () => {
  assert.deepEqual(groupLines([]), []);
  assert.deepEqual(groupLines([{ top: 0, bottom: 0, left: 0, right: 0 }]), []);
});

test('a big heading on the same line as smaller text is one line', () => {
  const lines = groupLines([{ top: 10, bottom: 50, left: 0, right: 100 }, { top: 24, bottom: 44, left: 100, right: 200 }]);
  assert.equal(lines.length, 1);
  assert.deepEqual([lines[0].top, lines[0].bottom], [10, 50]);
});

test('only the lines that can be seen are picked, with the first below a height found by search', () => {
  const lines = Array.from({ length: 1000 }, (_, i) => ({ n: i + 1, top: i * 30, bottom: i * 30 + 22 }));
  assert.equal(firstLineBelow(lines, 0), 0);
  assert.equal(firstLineBelow(lines, 300), 10);
  assert.equal(firstLineBelow(lines, 1e9), 1000);
  const seen = linesBetween(lines, 3000, 3100);
  assert.deepEqual(seen.map((l) => l.n), [101, 102, 103, 104]);
  assert.deepEqual(linesBetween(lines, 1e9, 2e9), []);
});

test('a stretch of text runs over the lines it touches', () => {
  const lines = groupLines(rects);
  assert.deepEqual(lineRangeFor(lines, 12, 30), { first: 1, last: 1 });
  assert.deepEqual(lineRangeFor(lines, 12, 120), { first: 1, last: 3 });
  assert.deepEqual(lineRangeFor(lines, 42, 60), { first: 2, last: 2 });
  assert.deepEqual(lineRangeFor(lines, 200, 300), { first: 3, last: 3 }, 'past the end it is the last line');
  assert.equal(lineRangeFor([], 0, 10), null);
});

test('numbers are shown with at least three digits', () => {
  assert.equal(lineLabel(7), '007');
  assert.equal(lineLabel(123), '123');
  assert.equal(lineLabel(1234), '1234');
});
