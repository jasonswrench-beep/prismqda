import assert from 'node:assert/strict';
import test from 'node:test';
import { progressPercent, shortNotes } from './updates';

test('the share of a download is a whole number from 0 to 100, and unknown when the total is', () => {
  assert.equal(progressPercent(0, 1000), 0);
  assert.equal(progressPercent(499, 1000), 49);
  assert.equal(progressPercent(1000, 1000), 100);
  assert.equal(progressPercent(2000, 1000), 100, 'never more than all of it');
  assert.equal(progressPercent(10, null), null);
  assert.equal(progressPercent(10, undefined), null);
  assert.equal(progressPercent(10, 0), null);
  assert.equal(progressPercent(Number.NaN, 100), null);
});

test('release notes are tidied and cut to a length the window can show', () => {
  assert.equal(shortNotes(undefined), '');
  assert.equal(shortNotes('  Fixes.\r\n\r\n\r\n\r\nMore.  '), 'Fixes.\n\nMore.');
  const long = 'word '.repeat(1000);
  const cut = shortNotes(long, 100);
  assert.equal(Array.from(cut).length, 100, 'ninety-nine characters and the ellipsis, or a hundred with it');
  assert.ok(cut.endsWith('…'));
  assert.equal(shortNotes('short', 100), 'short');
});
