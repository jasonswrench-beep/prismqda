import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  clockPair,
  clockText,
  fractionOf,
  isCodableSpan,
  lengthText,
  orderedSpan,
  parseClock,
  sameSpan,
  spanProblem,
  spanShort,
  spanWords,
  stripeLayout,
  timeAtFraction,
} from './clips';

test('a time is a clock, with tenths only when asked for', () => {
  assert.equal(clockText(0), '0:00');
  assert.equal(clockText(65_000), '1:05');
  assert.equal(clockText(3_723_400), '1:02:03');
  assert.equal(clockText(3_723_400, true), '1:02:03.4');
  assert.equal(clockText(7_999, true), '0:07.9', 'tenths are cut, not rounded up into the next second');
  assert.equal(clockText(-5), '0:00');
  assert.equal(clockText(Number.NaN), '0:00');
});

test('the two ends of a stretch have tenths together, as the engine says them', () => {
  assert.deepEqual(clockPair(12_000, 45_000), ['0:12', '0:45']);
  assert.deepEqual(clockPair(12_400, 45_000), ['0:12.4', '0:45.0']);
  assert.equal(lengthText(12_400, 45_000), '0:32.6');
  assert.equal(spanWords(12_000, 45_000), 'Recording, 0:12 to 0:45 (0:33 long)');
  assert.equal(spanWords(12_400, 45_000), 'Recording, 0:12.4 to 0:45.0 (0:32.6 long)');
  assert.equal(spanWords(1_000, 3_723_000), 'Recording, 0:01 to 1:02:03 (1:02:02 long)');
  assert.equal(spanShort(5_000, 9_500), '0:05.0 to 0:09.5');
});

test('a typed time is read in the forms people write and nothing else is guessed', () => {
  assert.equal(parseClock('1:05'), 65_000);
  assert.equal(parseClock(' 1:02:03 '), 3_723_000);
  assert.equal(parseClock('0:07.4'), 7_400);
  assert.equal(parseClock('65'), 65_000);
  assert.equal(parseClock('7.4'), 7_400);
  assert.equal(parseClock('0'), 0);
  for (const bad of ['', ' ', 'abc', '1:75', '1:60', '1:2:3:4', '-5', '1:-5', '1,5', '1:05pm', '1:61:00', '99999999999']) {
    assert.equal(parseClock(bad), null, JSON.stringify(bad));
  }
  assert.equal(parseClock('5:00:00'), 18_000_000);
});

test('a stretch is codable when it is whole milliseconds, at least a tenth of a second, and ends within seven days', () => {
  assert.equal(isCodableSpan(1000, 1100), true);
  assert.equal(isCodableSpan(1000, 1099), false);
  assert.equal(isCodableSpan(-1, 1000), false);
  assert.equal(isCodableSpan(0, 8 * 24 * 3600 * 1000), false);
  assert.equal(isCodableSpan(1000.5, 3000), false);
  assert.match(spanProblem(null, 5000) ?? '', /Mark where/);
  assert.match(spanProblem(5000, 5000) ?? '', /end after it starts/);
  assert.match(spanProblem(5000, 5050) ?? '', /too short/);
  assert.equal(spanProblem(5000, 9000), null);
});

test('two marks make a stretch whichever was made first', () => {
  assert.deepEqual(orderedSpan(9000.4, 2000.2), { shape: 'span', start_ms: 2000, end_ms: 9000 });
  assert.deepEqual(orderedSpan(-3, 1000), { shape: 'span', start_ms: 0, end_ms: 1000 });
  assert.equal(sameSpan(orderedSpan(1, 5000), orderedSpan(5000, 1)), true);
});

test('stretches that overlap are put in rows so none hides another', () => {
  const layout = stripeLayout(
    [
      { id: 1, start_ms: 0, end_ms: 4000 },
      { id: 2, start_ms: 2000, end_ms: 6000 },
      { id: 3, start_ms: 4000, end_ms: 8000 },
      { id: 4, start_ms: 9000, end_ms: 9050 },
    ],
    10_000,
  );
  const by = (id: number) => layout.find((s) => s.id === id)!;
  assert.deepEqual([by(1).lane, by(2).lane, by(3).lane, by(4).lane], [0, 1, 0, 0], 'a stretch takes the first row that is free where it starts');
  assert.equal(by(1).left, 0);
  assert.equal(by(1).width, 0.4);
  assert.equal(by(3).left, 0.4);
  assert.ok(by(4).width >= 0.004, 'a very short stretch can still be seen');
  assert.ok(by(4).left + by(4).width <= 1);
  assert.deepEqual(stripeLayout([], 1000), []);
  assert.equal(stripeLayout([{ id: 1, start_ms: 0, end_ms: 500 }], 0)[0].width <= 1, true, 'an unknown length does not divide by zero');
});

test('a place on the timeline is a time and the other way round', () => {
  assert.equal(fractionOf(2500, 10_000), 0.25);
  assert.equal(fractionOf(20_000, 10_000), 1);
  assert.equal(timeAtFraction(0.25, 10_000), 2500);
  assert.equal(timeAtFraction(-1, 10_000), 0);
  assert.equal(timeAtFraction(2, 10_000), 10_000);
});
