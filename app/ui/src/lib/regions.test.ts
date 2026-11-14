import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { AreaShape, RegionRow, RegionShape } from '../types';
import {
  areaOf,
  boundsOf,
  boxFrom,
  describeShape,
  fitShape,
  fitZoom,
  isCodable,
  moveShape,
  nudgePoint,
  outlineFromStroke,
  pointFromScreen,
  regionToCoding,
  resizeShape,
  sameShape,
  shownSize,
  simplify,
  startBox,
  stepZoom,
  thumbPlacement,
  turnPoint,
  turnShape,
  unturnPoint,
  type Point,
} from './regions';

const rect = (x: number, y: number, w: number, h: number): AreaShape => ({ shape: 'rectangle', x, y, w, h });

test('a shape is rounded and kept on the picture, and the same shape reads the same', () => {
  assert.deepEqual(fitShape(rect(0.123456789, -0.2, 0.5, 2)), { shape: 'rectangle', x: 0.12346, y: 0, w: 0.5, h: 1 });
  assert.ok(sameShape(rect(0.1, 0.1, 0.2, 0.2), rect(0.100001, 0.1, 0.2, 0.2)));
  assert.ok(!sameShape(rect(0.1, 0.1, 0.2, 0.2), { shape: 'ellipse', x: 0.1, y: 0.1, w: 0.2, h: 0.2 }));
});

test('what is too small, or too thin, cannot be coded', () => {
  assert.ok(isCodable(rect(0.1, 0.1, 0.01, 0.01)));
  assert.ok(!isCodable(rect(0.1, 0.1, 0.001, 0.3)));
  assert.ok(!isCodable({ shape: 'polygon', points: [[0.1, 0.1], [0.5, 0.5]] }));
  assert.ok(!isCodable({ shape: 'polygon', points: [[0.1, 0.1], [0.5, 0.5], [0.9, 0.9]] }), 'three points on a line enclose nothing');
  assert.ok(isCodable({ shape: 'polygon', points: [[0.1, 0.1], [0.5, 0.1], [0.3, 0.5]] }));
});

test('area and box agree for each kind of shape', () => {
  assert.equal(areaOf(rect(0, 0, 0.5, 0.4)), 0.2);
  assert.ok(Math.abs(areaOf({ shape: 'ellipse', x: 0, y: 0, w: 0.5, h: 0.5 }) - Math.PI / 16) < 1e-9);
  assert.equal(areaOf({ shape: 'polygon', points: [[0, 0], [0.5, 0], [0, 0.5]] }), 0.125);
  assert.deepEqual(boundsOf({ shape: 'polygon', points: [[0.2, 0.3], [0.7, 0.1], [0.4, 0.9]] }), [0.2, 0.1, 0.7, 0.9]);
});

test('a shape is told in words the way the engine tells it', () => {
  assert.equal(describeShape(rect(0.1, 0.2, 0.3, 0.4)), 'Rectangle, 10% to 40% across and 20% to 60% down, 12% of the picture');
  assert.equal(describeShape({ shape: 'polygon', points: [[0.1, 0.1], [0.5, 0.1], [0.3, 0.5]] }), 'Outline of 3 points, 10% to 50% across and 10% to 50% down, 8% of the picture');
});

test('a turned picture maps points both ways, and four quarter turns are no turn', () => {
  const p: Point = [0.2, 0.7];
  for (const turns of [0, 1, 2, 3]) {
    const [u, v] = turnPoint(p, turns);
    const back = unturnPoint([u, v], turns);
    assert.ok(Math.abs(back[0] - p[0]) < 1e-9 && Math.abs(back[1] - p[1]) < 1e-9, `turns ${turns}`);
  }
  assert.deepEqual(turnPoint([0, 0], 1), [1, 0], 'the top left corner goes to the top right after a clockwise quarter turn');
  assert.deepEqual(turnPoint([1, 0], 1), [1, 1]);
  assert.deepEqual(turnShape(turnShape(rect(0.1, 0.2, 0.3, 0.1), 1), 3), rect(0.1, 0.2, 0.3, 0.1));
});

test('a pointer position becomes a point of the picture, cut at the edges, whatever the turn', () => {
  const shown = { w: 400, h: 200 };
  assert.deepEqual(pointFromScreen(100, 50, shown, 0), [0.25, 0.25]);
  assert.deepEqual(pointFromScreen(-30, 500, shown, 0), [0, 1], 'past an edge stops at the edge');
  // Turned a quarter turn, the screen's top right is the picture's top left corner... the picture's (0,0) shows at the top right.
  assert.deepEqual(pointFromScreen(400, 0, { w: 200, h: 400 }, 1), [0, 0]);
  assert.deepEqual(pointFromScreen(0, 0, { w: 200, h: 400 }, 1), [0, 1]);
});

test('the zoom that fits never blows a small picture up and counts the turn', () => {
  assert.equal(fitZoom({ frameW: 1000, frameH: 500, imageW: 4000, imageH: 1000, turns: 0 }), 0.25);
  assert.equal(fitZoom({ frameW: 1000, frameH: 500, imageW: 100, imageH: 100, turns: 0 }), 1);
  assert.equal(fitZoom({ frameW: 1000, frameH: 500, imageW: 1000, imageH: 500, turns: 1 }), 0.5, 'turned, the picture is 500 wide and 1000 tall');
  assert.deepEqual(shownSize({ imageW: 400, imageH: 200, turns: 1, zoom: 0.5 }), { w: 100, h: 200 });
  assert.equal(stepZoom(1, 1), 1.25);
  assert.equal(stepZoom(1, -1), 0.75);
  assert.equal(stepZoom(8, 1), 8);
  assert.equal(stepZoom(0.05, -1), 0.05);
});

test('two corners make a box whichever way they were dragged', () => {
  assert.deepEqual(boxFrom('rectangle', [0.5, 0.6], [0.2, 0.1]), { shape: 'rectangle', x: 0.2, y: 0.1, w: 0.3, h: 0.5 });
});

test('a freehand line is made simple and becomes an outline when it encloses something', () => {
  const line: Point[] = [];
  for (let i = 0; i <= 100; i++) line.push([0.2 + (0.6 * i) / 100, 0.2]);
  assert.equal(simplify(line, 0.004).length, 2, 'a straight line needs only its ends');
  const square: Point[] = [];
  for (let i = 0; i <= 50; i++) square.push([0.2 + (0.4 * i) / 50, 0.2]);
  for (let i = 1; i <= 50; i++) square.push([0.6, 0.2 + (0.4 * i) / 50]);
  for (let i = 1; i <= 50; i++) square.push([0.6 - (0.4 * i) / 50, 0.6]);
  for (let i = 1; i <= 50; i++) square.push([0.2, 0.6 - (0.4 * i) / 50]);
  const outline = outlineFromStroke(square);
  assert.ok(outline && outline.shape === 'polygon');
  assert.ok(outline.shape === 'polygon' && outline.points.length >= 4 && outline.points.length <= 6, 'the four corners');
  assert.equal(outlineFromStroke(line), null, 'a line encloses nothing');
});

test('the arrow keys move a shape as it appears and stop it at the edge', () => {
  assert.deepEqual(moveShape(rect(0.1, 0.1, 0.2, 0.2), 1, 0, false, 0), rect(0.11, 0.1, 0.2, 0.2));
  assert.deepEqual(moveShape(rect(0.1, 0.1, 0.2, 0.2), 0, 1, true, 0), rect(0.1, 0.15, 0.2, 0.2), 'Shift is a large step');
  assert.deepEqual(moveShape(rect(0, 0, 0.2, 0.2), -1, -1, false, 0), rect(0, 0, 0.2, 0.2), 'at the top left it stays');
  assert.deepEqual(moveShape(rect(0.8, 0.8, 0.2, 0.2), 1, 1, true, 0), rect(0.8, 0.8, 0.2, 0.2), 'at the bottom right it stays');
  // Turned a quarter turn clockwise, screen-right is picture-up... a press of Right moves the shape right as it is shown.
  const turned = moveShape(rect(0.4, 0.4, 0.2, 0.2), 1, 0, false, 1);
  const shown = turnShape(turned, 1);
  assert.deepEqual(shown, turnShape(rect(0.4, 0.4, 0.2, 0.2), 1).shape === 'rectangle' ? { shape: 'rectangle', x: 0.41, y: 0.4, w: 0.2, h: 0.2 } : shown);
});

test('the arrow keys with Alt grow and shrink a shape from its top left corner', () => {
  assert.deepEqual(resizeShape(rect(0.1, 0.1, 0.2, 0.2), 1, 1, false, 0), rect(0.1, 0.1, 0.21, 0.21));
  assert.deepEqual(resizeShape(rect(0.1, 0.1, 0.2, 0.2), -1, 0, true, 0), rect(0.1, 0.1, 0.15, 0.2));
  assert.deepEqual(resizeShape(rect(0.1, 0.1, 0.003, 0.2), -1, 0, true, 0), rect(0.1, 0.1, 0.002, 0.2), 'it cannot go below the smallest side');
  assert.deepEqual(resizeShape(rect(0.9, 0.9, 0.1, 0.1), 1, 1, true, 0), rect(0.9, 0.9, 0.1, 0.1), 'nor past the edge');
});

test('a point moves by the arrows and stays on the picture', () => {
  assert.deepEqual(nudgePoint([0.5, 0.5], 1, 0, false, 0), [0.51, 0.5]);
  assert.deepEqual(nudgePoint([0.995, 0.5], 1, 0, true, 0), [1, 0.5]);
  assert.deepEqual(nudgePoint([0, 0], -1, -1, false, 0), [0, 0]);
});

test('a keyboard shape starts in the middle of where the point is and stays on the picture', () => {
  assert.deepEqual(startBox('rectangle', [0.5, 0.5]), rect(0.4, 0.4, 0.2, 0.2));
  assert.deepEqual(startBox('rectangle', [0, 1]), rect(0, 0.8, 0.2, 0.2));
});

test('a thumbnail shows the shape in the middle and a margin round it', () => {
  const t = thumbPlacement(rect(0.4, 0.4, 0.2, 0.2), 1000, 1000, 100, 100);
  // The shape is 200 pixels of a 1000 pixel picture, plus a margin of eight percent each side: 232 pixels fill the 100 box.
  assert.ok(Math.abs(t.width - (1000 * 100) / 232) < 1e-6);
  assert.ok(Math.abs(t.left + t.width * 0.5 - 50) < 1e-6 && Math.abs(t.top + t.height * 0.5 - 50) < 1e-6, 'the shape is centred');
});

test('a coded area is a coding with a negative number and its shape', () => {
  const row: RegionRow = { id: 7, source_id: 3, shape: 'rectangle', page: 3, x: 0.1, y: 0.2, w: 0.3, h: 0.4, code_id: 2, coder_id: 1, origin: 'human', layer: 'coder', rationale: 'a note', rating: null, created_at: 'now', text: 'Rectangle' };
  const c = regionToCoding(row);
  assert.equal(c.id, -7);
  assert.deepEqual(c.region, { shape: 'rectangle', x: 0.1, y: 0.2, w: 0.3, h: 0.4 });
  assert.equal(c.rationale, 'a note');
  assert.equal(c.start, 0);
  assert.equal(c.page, 3, 'the page the area is on goes with it');
});

test('a stretch of a recording is a region that is told by its times and is kept as whole milliseconds', () => {
  const span: RegionShape = { shape: 'span', start_ms: 12_000, end_ms: 45_000 };
  assert.equal(describeShape(span), 'Recording, 0:12 to 0:45 (0:33 long)');
  assert.deepEqual(fitShape({ shape: 'span', start_ms: 12_000.4, end_ms: 45_000.6 }), { shape: 'span', start_ms: 12_000, end_ms: 45_001 });
  assert.deepEqual(fitShape({ shape: 'span', start_ms: -5, end_ms: 100 }), { shape: 'span', start_ms: 0, end_ms: 100 });
  assert.equal(sameShape(span, { shape: 'span', start_ms: 12_000, end_ms: 45_000 }), true);
  assert.equal(sameShape(span, { shape: 'span', start_ms: 12_000, end_ms: 45_001 }), false);
  const row = { id: 7, source_id: 3, page: 1, shape: 'span', start_ms: 12_000, end_ms: 45_000, code_id: 2, coder_id: 1, origin: 'human', layer: 'coder', rationale: '', rating: null, created_at: 'now', text: 'Recording, 0:12 to 0:45 (0:33 long)' } as unknown as RegionRow;
  const coding = regionToCoding(row);
  assert.equal(coding.id, -7, 'a stretch is numbered like an area: its number made negative');
  assert.deepEqual(coding.region, span);
});
