import assert from 'node:assert/strict';
import test from 'node:test';
import { pageWidthFor, PAGE_WIDTHS } from '../bridge';
import { OffsetIndex } from '../offsets';
import type { FormatRun } from '../types';
import { brief, clampPage, pagesWithAreas, pageWords, pagePixels, PIXELS_PER_POINT } from './pdfPages';

test('a page is shown at the size a PDF reader shows it at 100 percent', () => {
  assert.equal(PIXELS_PER_POINT, 96 / 72);
  assert.deepEqual(pagePixels({ width: 612, height: 792 }), { width: 816, height: 1056 }, 'US Letter is 816 by 1056 pixels');
  assert.deepEqual(pagePixels({ width: 0, height: 0 }), { width: 1, height: 1 }, 'a page is never nothing');
});

test('a page number is kept inside the PDF', () => {
  assert.equal(clampPage(0, 3), 1);
  assert.equal(clampPage(2.4, 3), 2);
  assert.equal(clampPage(9, 3), 3);
  assert.equal(clampPage(Number.NaN, 3), 1);
  assert.equal(clampPage(5, 0), 1);
});

test('the words of a page are the stretch the importer marked as that page', () => {
  const text = 'First page words.\n\nThird page words.';
  const runs: FormatRun[] = [
    { start: 0, end: 17, kind: 'page', page: 1 },
    { start: 19, end: 36, kind: 'page', page: 3 },
    { start: 0, end: 5, kind: 'bold' },
  ];
  const index = new OffsetIndex(text);
  assert.equal(pageWords(runs, text, index, 1), 'First page words.');
  assert.equal(pageWords(runs, text, index, 3), 'Third page words.');
  assert.equal(pageWords(runs, text, index, 2), '', 'a page with no words says nothing');
});

test('the pages that have areas are listed in order with a count', () => {
  const area = (page: number | undefined) => ({ region: { shape: 'rectangle' as const, x: 0, y: 0, w: 0.1, h: 0.1 }, page });
  assert.deepEqual(pagesWithAreas([area(3), area(1), area(3), { region: undefined, page: undefined }, area(undefined)]), [
    { page: 1, areas: 2 },
    { page: 3, areas: 2 },
  ]);
  assert.deepEqual(pagesWithAreas([]), []);
});

test('a long text is cut at a word with an ellipsis', () => {
  assert.equal(brief('short', 20), 'short');
  assert.equal(brief('one two three four five six', 14), 'one two three…');
  assert.equal(brief('  spaced\n\nout  ', 20), 'spaced out');
});

test('a page is asked for at the first width that is at least as wide as it is shown', () => {
  assert.equal(pageWidthFor(300), PAGE_WIDTHS[0]);
  assert.equal(pageWidthFor(480), 480);
  assert.equal(pageWidthFor(481), 720);
  assert.equal(pageWidthFor(816, 1.5), 1400, 'a sharp screen asks for more pixels');
  assert.equal(pageWidthFor(99999), PAGE_WIDTHS[PAGE_WIDTHS.length - 1], 'never more than the most the engine draws');
  assert.equal(pageWidthFor(0), PAGE_WIDTHS[0]);
});
