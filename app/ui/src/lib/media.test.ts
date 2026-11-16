import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { UnitRow } from '../types';
import { formatClock, stepFrames, timecode, timeForOffset, timedUnits, turnLabel, unitAt } from './media';

const unit = (id: number, start: number, end: number, from: number | null, to: number | null): UnitRow => ({
  id, source_id: 1, kind: 'speaker_turn', start, end, speaker: null, parent_id: null, excluded: false, time_start_ms: from, time_end_ms: to,
});

test('stepping a frame lands on the frame grid, never before the start or after the last frame', () => {
  assert.equal(stepFrames(1000, 1, 25, null), 1040, 'one frame at 25 a second is 40 ms');
  assert.equal(stepFrames(1000, -1, 25, null), 960);
  assert.equal(stepFrames(1013, 1, 25, null), 1040, 'from between two frames it lands on a frame');
  assert.equal(stepFrames(0, -1, 30, null), 0, 'not before the start');
  assert.equal(stepFrames(500, 10, 30, null), 833.3333333333334, 'ten frames at 30 a second');
  assert.equal(stepFrames(1960, 5, 25, 2000), 1960, 'not after the last frame of a two second video (frame 49)');
  assert.equal(stepFrames(900, 1, 0, null), stepFrames(900, 1, 30, null), 'a frame rate that is not a rate is taken as 30');
  assert.equal(stepFrames(Number.NaN, 1, 25, null), 40);
});

test('a timecode counts hours, minutes, seconds and the frame within the second', () => {
  assert.equal(timecode(0, 25), '0:00:00:00');
  assert.equal(timecode(1040, 25), '0:00:01:01');
  assert.equal(timecode(62280, 25), '0:01:02:07');
  assert.equal(timecode(3_600_000, 30), '1:00:00:00');
  assert.equal(timecode(999, 30), '0:00:00:29', 'the last frame of the first second');
  assert.equal(timecode(Number.NaN, 30), '0:00:00:00');
});

test('a time is written as minutes and seconds, with hours only when there are some', () => {
  assert.equal(formatClock(0), '0:00');
  assert.equal(formatClock(999), '0:00', 'whole seconds, never rounded up');
  assert.equal(formatClock(1000), '0:01');
  assert.equal(formatClock(62_500), '1:02');
  assert.equal(formatClock(3_599_999), '59:59');
  assert.equal(formatClock(3_723_000), '1:02:03');
  assert.equal(formatClock(-5), '0:00');
  assert.equal(formatClock(null), '0:00');
  assert.equal(formatClock(Number.NaN), '0:00');
});

test('only units with times count, in the order they are spoken', () => {
  const list = [unit(3, 40, 60, 9000, 12000), unit(1, 0, 20, 1000, 4000), unit(2, 20, 40, null, null), unit(4, 60, 80, 5000, null)];
  assert.deepEqual(timedUnits(list).map((u) => u.id), [1, 3]);
  assert.deepEqual(timedUnits([]), []);
});

test('the turn being spoken is the last one that has started', () => {
  const units = timedUnits([unit(1, 0, 20, 1000, 4000), unit(2, 20, 40, 5000, 8000), unit(3, 40, 60, 9000, 12000)]);
  assert.equal(unitAt(units, 0), null, 'before the first turn');
  assert.equal(unitAt(units, 1000)?.id, 1);
  assert.equal(unitAt(units, 4500)?.id, 1, 'a pause between turns still belongs to the turn before');
  assert.equal(unitAt(units, 5000)?.id, 2);
  assert.equal(unitAt(units, 99_000)?.id, 3, 'after the last turn');
  assert.equal(unitAt([], 5000), null);
});

test('the place in the recording for words in the text is where their turn starts', () => {
  const units = timedUnits([unit(1, 0, 20, 1000, 4000), unit(2, 20, 40, 5000, 8000)]);
  assert.equal(timeForOffset(units, 0), 1000);
  assert.equal(timeForOffset(units, 19), 1000);
  assert.equal(timeForOffset(units, 20), 5000);
  assert.equal(timeForOffset(units, 500), 5000, 'past the end of the text, the last turn');
  assert.equal(timeForOffset([], 5), null);
});

test('a turn is one line: when, and its first words', () => {
  const [u] = timedUnits([unit(1, 0, 40, 62_000, 70_000)]);
  assert.equal(turnLabel(u, 'Hello   there,\nfriend.'), '1:02 Hello there, friend.');
  const long = turnLabel(u, 'word '.repeat(40), 20);
  assert.ok(long.startsWith('1:02 word word') && long.endsWith('…'), long);
});
