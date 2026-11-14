import assert from 'node:assert/strict';
import { test } from 'node:test';
import { edgePoint, linkSentence, mapInWords, midpoint, moveBy, NODE_H, NODE_W, shorten, viewBoxFor } from './conceptMap';

test('the canvas holds every node with room round it and is never smaller than a usable size', () => {
  const empty = viewBoxFor([]);
  assert.deepEqual([empty.w, empty.h], [600, 360]);
  const small = viewBoxFor([{ x: 0, y: 0 }]);
  assert.deepEqual([small.w, small.h], [600, 360]);
  assert.ok(small.x < -NODE_W / 2 && small.x + small.w > NODE_W / 2, 'a single node sits inside the canvas');
  const wide = viewBoxFor([{ x: -1000, y: 0 }, { x: 1000, y: 0 }]);
  assert.ok(wide.x <= -1000 - NODE_W / 2 && wide.x + wide.w >= 1000 + NODE_W / 2, 'a wide map is not cut off');
  assert.equal(wide.h, 360);
  const tall = viewBoxFor([{ x: 0, y: -500 }, { x: 0, y: 500 }]);
  assert.ok(tall.y <= -500 - NODE_H / 2 && tall.y + tall.h >= 500 + NODE_H / 2);
});

test('a link meets a node at its edge, on the side that faces the other node', () => {
  const right = edgePoint({ x: 0, y: 0 }, { x: 1000, y: 0 });
  assert.deepEqual(right, { x: NODE_W / 2, y: 0 });
  const down = edgePoint({ x: 0, y: 0 }, { x: 0, y: 500 });
  assert.deepEqual(down, { x: 0, y: NODE_H / 2 });
  const diagonal = edgePoint({ x: 0, y: 0 }, { x: 400, y: 400 });
  assert.equal(diagonal.y, NODE_H / 2, 'a steep line leaves through the bottom');
  assert.ok(diagonal.x > 0 && diagonal.x < NODE_W / 2);
  assert.deepEqual(edgePoint({ x: 5, y: 5 }, { x: 5, y: 5 }), { x: 5, y: 5 }, 'two nodes in one place do not break it');
  assert.deepEqual(midpoint({ x: 0, y: 0 }, { x: 10, y: 20 }), { x: 5, y: 10 });
});

test('a long label is shortened by characters, not by code units, and keeps its whole text elsewhere', () => {
  assert.equal(shorten('Cost'), 'Cost');
  assert.equal(shorten('  many   spaces here  '), 'many spaces here');
  assert.equal(shorten('abcdefghijklmnopqrstuvwxyz', 10), 'abcdefghi…');
  assert.equal(shorten('\u{1F600}'.repeat(30), 5), '\u{1F600}'.repeat(4) + '…');
});

test('a link is a sentence, with a direction or without', () => {
  assert.equal(linkSentence({ from: 'Cost', to: 'Anxiety', label: 'leads to', directed: true }), 'Cost leads to Anxiety');
  assert.equal(linkSentence({ from: 'Cost', to: 'Anxiety', label: '', directed: true }), 'Cost is linked to Anxiety');
  assert.equal(linkSentence({ from: 'Cost', to: 'Anxiety', label: 'occur together', directed: false }), 'Cost and Anxiety: occur together');
  assert.equal(linkSentence({ from: 'Cost', to: 'Anxiety', label: ' ', directed: false }), 'Cost and Anxiety are linked');
});

test('arrow keys move a node a little, Shift a lot, and other keys not at all', () => {
  assert.deepEqual(moveBy({ x: 10, y: 10 }, 'ArrowRight', false), { x: 30, y: 10 });
  assert.deepEqual(moveBy({ x: 10, y: 10 }, 'ArrowLeft', true), { x: -90, y: 10 });
  assert.deepEqual(moveBy({ x: 10, y: 10 }, 'ArrowUp', false), { x: 10, y: -10 });
  assert.deepEqual(moveBy({ x: 10, y: 10 }, 'ArrowDown', true), { x: 10, y: 110 });
  assert.equal(moveBy({ x: 0, y: 0 }, 'a', false), null);
  assert.equal(moveBy({ x: 0, y: 0 }, 'Enter', true), null);
});

test('the map in words lists each link, and the nodes joined to nothing', () => {
  const nodes = [
    { id: 1, label: 'Cost', kind: 'code' },
    { id: 2, label: 'Anxiety', kind: 'code' },
    { id: 3, label: 'A thought', kind: 'note' },
  ];
  const words = mapInWords(nodes, [{ from_node: 1, to_node: 2, label: 'leads to', directed: true }]);
  assert.deepEqual(words.sentences, ['Cost leads to Anxiety']);
  assert.deepEqual(words.alone, ['A thought']);
  assert.deepEqual(mapInWords([], []), { sentences: [], alone: [] });
});
