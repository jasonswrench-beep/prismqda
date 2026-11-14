import assert from 'node:assert/strict';
import { test } from 'node:test';
import { describeHistory, initialsOf, memoNumber, summarizeHistory } from './provenance';

test('a coding nothing has touched reads "1 code added, 0 removed"', () => {
  const h = summarizeHistory([{ ts: '2026-11-06T10:24:00Z', actor: 'Ada', action: 'coding.add' }]);
  assert.deepEqual(h, { firstAt: '2026-11-06T10:24:00Z', lastAt: '2026-11-06T10:24:00Z', added: 1, removed: 0 });
  assert.equal(describeHistory(h), '1 code added, 0 removed');
});

test('changing the code counts as one removed and one added, and a note or rating only moves the last change', () => {
  const h = summarizeHistory([
    { ts: '2026-11-06T12:00:00Z', actor: 'Ada', action: 'coding.rationale' },
    { ts: '2026-11-06T10:00:00Z', actor: 'Ada', action: 'coding.add' },
    { ts: '2026-11-06T11:00:00Z', actor: 'Ada', action: 'coding.change_code' },
    { ts: '2026-11-06T13:00:00Z', actor: 'Ada', action: 'coding.rating' },
  ]);
  assert.equal(h.added, 2);
  assert.equal(h.removed, 1);
  assert.equal(h.firstAt, '2026-11-06T10:00:00Z');
  assert.equal(h.lastAt, '2026-11-06T13:00:00Z');
  assert.equal(describeHistory(h), '2 codes added, 1 removed');
});

test('no events, no dates, and unreadable dates are skipped', () => {
  assert.deepEqual(summarizeHistory([]), { firstAt: null, lastAt: null, added: 0, removed: 0 });
  const h = summarizeHistory([{ ts: 'not a date', actor: 'x', action: 'coding.add' }]);
  assert.equal(h.added, 1);
  assert.equal(h.firstAt, null);
});

test('memos go by a number with at least three digits, and names by their initials', () => {
  assert.equal(memoNumber(14), 'M-014');
  assert.equal(memoNumber(1234), 'M-1234');
  assert.equal(initialsOf('Rosa Martin'), 'RM');
  assert.equal(initialsOf('Ada'), 'A');
  assert.equal(initialsOf('  '), '?');
  assert.equal(initialsOf('Ada Byron King'), 'AB');
});
