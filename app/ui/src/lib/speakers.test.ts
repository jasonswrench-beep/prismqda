import assert from 'node:assert/strict';
import { test } from 'node:test';
import { OffsetIndex } from '../offsets';
import { buildSegments } from './segments';
import { findSpeakerLabels, labelColumn } from './speakers';

const text = ['Interviewer: To start, how do you travel?', 'Alex: By bus, mostly.', 'More on the same turn without a label.', 'Participant 3: A longer one here.'].join('\n');
const index = new OffsetIndex(text);

test('a name and a colon at the start of a line is a label, with the spaces after it', () => {
  const labels = findSpeakerLabels(text, index);
  assert.equal(labels.length, 3);
  const words = labels.map((l) => text.slice(l.start, l.end));
  assert.deepEqual(words, ['Interviewer: ', 'Alex: ', 'Participant 3: ']);
});

test('a colon in the middle of a line, a lowercase start, a long run, or a known heading is not a label', () => {
  const t = 'Then he said: no.\nthe plan: later\nThis line has far too many words before the colon: nothing\nNote: remember\nhttp: odd\nTime: 5';
  assert.deepEqual(findSpeakerLabels(t, new OffsetIndex(t)), []);
});

test('labels are found after emoji and other text that counts double in UTF-16', () => {
  const t = '\u{1F600} hello\nAlex: hi';
  const labels = findSpeakerLabels(t, new OffsetIndex(t));
  assert.equal(labels.length, 1);
  assert.equal(Array.from(t).slice(labels[0].start, labels[0].end).join(''), 'Alex: ');
});

test('the label column is as wide as the longest label and a space, up to a limit', () => {
  assert.equal(labelColumn([]), 0);
  assert.equal(labelColumn(findSpeakerLabels(text, index)), 16);
  assert.equal(labelColumn([{ start: 0, end: 80 }]), 22);
});

test('the label is a piece of its own, so it can be drawn in a column, and a coding after it keeps its place', () => {
  const labels = findSpeakerLabels(text, index);
  const alex = text.indexOf('By bus');
  const segments = buildSegments({ length: index.scalarLength, codings: [{ id: 1, start: alex, end: alex + 6 }], runs: [], units: [], labels });
  const flagged = segments.filter((s) => s.label).map((s) => text.slice(s.start, s.end));
  assert.deepEqual(flagged, ['Interviewer: ', 'Alex: ', 'Participant 3: ']);
  const coded = segments.find((s) => s.codings.length > 0)!;
  assert.equal(text.slice(coded.start, coded.end), 'By bus');
  assert.equal(coded.label, false);
  // The pieces still cover the whole text with no gap or overlap.
  let at = 0;
  for (const s of segments) {
    assert.equal(s.start, at);
    at = s.end;
  }
  assert.equal(at, index.scalarLength);
});
