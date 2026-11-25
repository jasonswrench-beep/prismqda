import assert from 'node:assert/strict';
import { test } from 'node:test';
import { shownCell } from './tableCells';

test('a summary is shown to two decimals, and nothing else is changed', () => {
  assert.equal(shownCell('52.42857142857143'), '52.43');
  assert.equal(shownCell('-0.333333'), '-0.33');
  assert.equal(shownCell('40.000000001'), '40');
  assert.equal(shownCell('12'), '12');
  assert.equal(shownCell('3.5'), '3.5');
  assert.equal(shownCell('3.25'), '3.25');
  assert.equal(shownCell('Bus'), 'Bus');
  assert.equal(shownCell('2026.11.25'), '2026.11.25');
  assert.equal(shownCell(''), '');
  assert.equal(shownCell(null), null);
});
