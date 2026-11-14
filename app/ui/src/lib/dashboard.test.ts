import assert from 'node:assert/strict';
import { test } from 'node:test';
import { areaChart, busiestDay, changesPerDay, friendlyTime, niceMax, ringSlices, typeShares } from './dashboard';

const NOW = new Date(2026, 10, 6, 15, 0, 0); // Nov 6, 2026, 3 PM, in this computer's own time zone

test('changes are counted on the day they were made, with today last', () => {
  const noon = (day: number) => new Date(2026, 10, day, 12, 0, 0).toISOString();
  const days = changesPerDay([noon(6), noon(6), noon(5), noon(1), noon(1), noon(1)], NOW, 7);
  assert.equal(days.length, 7);
  assert.deepEqual(days.map((d) => d.count), [0, 3, 0, 0, 0, 1, 2], 'Oct 31 is the first day shown; the three changes on Nov 1 are the next');
  assert.equal(days[6].label, 'Today');
  assert.equal(days[0].label, 'Oct 31');
});

test('changes outside the window, and timestamps that are not dates, are left out', () => {
  const days = changesPerDay(['not a date', new Date(2026, 8, 1).toISOString(), new Date(2027, 0, 1).toISOString()], NOW, 14);
  assert.equal(days.reduce((n, d) => n + d.count, 0), 0);
});

test('the busiest day is the latest one when two tie, and there is none when nothing happened', () => {
  assert.equal(busiestDay([{ date: 'a', label: 'a', count: 0 }]), null);
  const best = busiestDay([
    { date: 'a', label: 'a', count: 4 },
    { date: 'b', label: 'b', count: 4 },
    { date: 'c', label: 'c', count: 1 },
  ]);
  assert.equal(best?.label, 'b');
});

test('the top of the scale is at least three and a round number', () => {
  assert.equal(niceMax(0), 3);
  assert.equal(niceMax(3), 3);
  assert.equal(niceMax(7), 7);
  assert.equal(niceMax(41), 45);
  assert.equal(niceMax(133), 140);
  assert.equal(niceMax(251), 300);
});

test('the chart puts the first day at the left edge and the highest day at the top', () => {
  const days = [
    { date: 'a', label: 'a', count: 0 },
    { date: 'b', label: 'b', count: 3 },
    { date: 'c', label: 'c', count: 1 },
  ];
  const c = areaChart(days, { width: 200, height: 100, left: 20, right: 10, top: 10, bottom: 20 });
  assert.equal(c.top, 3);
  assert.deepEqual(c.points.map((p) => [p.x, Math.round(p.y)]), [[20, 80], [105, 10], [190, 57]]);
  assert.match(c.line, /^M20 80 L105 10 L190 56\.7$/);
  assert.ok(c.area.endsWith('Z') && c.area.includes('L190 80 L20 80'), 'the area closes down to the axis');
});

test('shares add up to exactly one hundred percent and each document is counted once', () => {
  const labels = { transcript: 'Transcript', document: 'Document', survey: 'Survey responses' };
  const s = typeShares(['transcript', 'transcript', 'document', 'survey', 'survey', 'survey', 'document'], labels);
  assert.equal(s.reduce((n, x) => n + x.count, 0), 7);
  assert.equal(s.reduce((n, x) => n + x.percent, 0), 100);
  assert.deepEqual(s.map((x) => x.label), ['Survey responses', 'Document', 'Transcript'], 'biggest first');
  const thirds = typeShares(['a', 'b', 'c'], {});
  assert.equal(thirds.reduce((n, x) => n + x.percent, 0), 100, 'thirds still add to 100');
  assert.deepEqual(typeShares([], {}), []);
});

test('the ring has one slice for each share, side by side around the circle', () => {
  const shares = typeShares(['a', 'a', 'b', 'b'], {});
  const ring = ringSlices(shares, 50, 2);
  const c = 2 * Math.PI * 50;
  assert.equal(ring.length, 2);
  assert.equal(ring[0].offset, 0);
  assert.ok(Math.abs(ring[1].offset + c / 2) < 0.02, 'the second slice starts halfway round');
  const [shown] = ring[0].dash.split(' ').map(Number);
  assert.ok(Math.abs(shown - (c / 2 - 2)) < 0.02, 'a gap of two is taken from each slice');
  assert.equal(ringSlices(typeShares(['a'], {}), 50)[0].dash.split(' ')[0], String(Math.round(c * 100) / 100), 'one slice is the whole ring, with no gap');
  assert.deepEqual(ringSlices([], 50), []);
});

test('the time of the last save is told as today, yesterday, or a date', () => {
  const now = new Date(2026, 10, 6, 15, 0, 0).getTime();
  assert.match(friendlyTime(new Date(2026, 10, 6, 14, 14, 0).getTime(), now), /^Today, 2:14 PM$/);
  assert.match(friendlyTime(new Date(2026, 10, 5, 23, 59, 0).getTime(), now), /^Yesterday, 11:59 PM$/);
  assert.match(friendlyTime(new Date(2026, 9, 31, 9, 5, 0).getTime(), now), /^Oct 31, 2026, 9:05 AM$/);
});
