import assert from 'node:assert/strict';
import test from 'node:test';
import { DRAG_THRESHOLD, isDrag } from './pointerDrag';

test('a press becomes a drag only after the pointer has moved a few pixels, so a click on a row is not a drag', () => {
  assert.equal(isDrag(0, 0), false);
  assert.equal(isDrag(DRAG_THRESHOLD - 1, 0), false);
  assert.equal(isDrag(0, -(DRAG_THRESHOLD - 1)), false);
  assert.equal(isDrag(DRAG_THRESHOLD, 0), true);
  assert.equal(isDrag(-DRAG_THRESHOLD, 0), true, 'either way');
  assert.equal(isDrag(4, 4), false, 'a diagonal move of 4 and 4 is about 5.7 pixels, a little under the threshold');
  assert.equal(isDrag(5, 5), true, 'and 5 and 5 is about 7 pixels');
});
