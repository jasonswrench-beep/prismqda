import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MAX_PASSAGES, describeFilters, describePassages, passagesRequest, previewText, pruneActive, scopeChip, searchRows, sortRows, toggleId, type ResultRow } from './retrieval';

test('toggling adds a code that is missing and removes one that is there, keeping the order', () => {
  assert.deepEqual(toggleId([], 3), [3]);
  assert.deepEqual(toggleId([3, 5], 5), [3]);
  assert.deepEqual(toggleId([3, 5], 4), [3, 5, 4]);
});

test('codes that were retired, merged away, or deleted drop out of the chosen list', () => {
  const codes = [
    { id: 1, status: 'active' as const },
    { id: 2, status: 'retired' as const },
    { id: 3, status: 'merged' as const },
  ];
  assert.deepEqual(pruneActive([1, 2, 3, 9], codes), [1]);
  assert.deepEqual(pruneActive([], codes), []);
});

test('with no code chosen every coding is asked for, so the table is not empty for want of a choice', () => {
  assert.deepEqual(passagesRequest([], 'all', 4), { limit: MAX_PASSAGES, workingSet: true });
  assert.deepEqual(passagesRequest([], 'open', 4), { limit: MAX_PASSAGES, workingSet: true, source_ids: [4] });
});

test('the request names the codes, and the open document only when asked to; it asks to follow the working set', () => {
  assert.deepEqual(passagesRequest([2, 7], 'all', 4), { code_ids: [2, 7], limit: MAX_PASSAGES, workingSet: true });
  assert.deepEqual(passagesRequest([2], 'open', 4), { code_ids: [2], limit: MAX_PASSAGES, workingSet: true, source_ids: [4] });
  assert.deepEqual(passagesRequest([2], 'open', null), { code_ids: [2], limit: MAX_PASSAGES, workingSet: true, source_ids: [] }, 'no open document matches nothing');
});

test('the scope chip says what is chosen in few words', () => {
  assert.equal(scopeChip(0, 'all'), 'No filter');
  assert.equal(scopeChip(0, 'open'), 'Open source');
  assert.equal(scopeChip(1, 'all'), '1 code');
  assert.equal(scopeChip(3, 'all'), '3 codes');
  assert.equal(scopeChip(2, 'open'), '2 codes, open source');
});

test('the status bar names every filter that narrows the table, or nothing', () => {
  assert.equal(describeFilters({ codeCount: 0, mode: 'all', caseName: null, query: '' }), null);
  assert.equal(describeFilters({ codeCount: 0, mode: 'all', caseName: null, query: '   ' }), null);
  assert.equal(describeFilters({ codeCount: 2, mode: 'open', caseName: 'Urban Residents', query: ' bus ' }), '2 codes, open document, case Urban Residents, search "bus"');
  assert.equal(describeFilters({ codeCount: 1, mode: 'all', caseName: null, query: '' }), '1 code');
});

test('the sentence names the codes, the place, and says when the list is cut short', () => {
  assert.equal(describePassages({ total: 1, shown: 1, codeNames: ['Welcome'], mode: 'all', sourceName: null }), '1 segment coded Welcome, in every document.');
  assert.equal(describePassages({ total: 12, shown: 12, codeNames: ['Welcome', 'Cost'], mode: 'open', sourceName: 'interview' }), '12 segments coded Welcome, Cost, in interview.');
  assert.equal(describePassages({ total: 0, shown: 0, codeNames: ['A'], mode: 'open', sourceName: null }), '0 segments coded A, in the open document.');
  assert.equal(describePassages({ total: 5, shown: 5, codeNames: [], mode: 'all', sourceName: null }), '5 segments with any code, in every document.');
  assert.equal(
    describePassages({ total: 1200, shown: 500, codeNames: ['A', 'B', 'C', 'D', 'E'], mode: 'all', sourceName: null }),
    '1,200 segments coded A, B, C and 2 more, in every document. The first 500 are listed.',
  );
});

const row = (id: number, patch: Partial<ResultRow> = {}): ResultRow => ({
  coding_id: id,
  source_id: 1,
  source_name: 'INT-001 – Alex',
  code_id: 1,
  code_name: 'Reliability',
  text: 'The bus is often late',
  notes: 'Service reliability',
  case_name: 'Urban Residents',
  start: id * 10,
  end: id * 10 + 5,
  ...patch,
});

test('searching keeps rows that hold every typed word in any column, ignoring case', () => {
  const rows = [row(1), row(2, { code_name: 'Cost', text: 'The pass is not cheap', notes: 'Cost vs. car' }), row(3, { case_name: 'Commuters', source_name: 'INT-003 – Jordan' })];
  assert.deepEqual(searchRows(rows, 'bus').map((r) => r.coding_id), [1, 3]);
  assert.deepEqual(searchRows(rows, 'COST cheap').map((r) => r.coding_id), [2]);
  assert.deepEqual(searchRows(rows, 'commuters jordan').map((r) => r.coding_id), [3]);
  assert.deepEqual(searchRows(rows, '  ').map((r) => r.coding_id), [1, 2, 3]);
  assert.deepEqual(searchRows(rows, 'zebra'), []);
});

test('rows sort by a column in either direction, and ties keep document order', () => {
  const rows = [row(3, { code_name: 'Cost' }), row(1, { code_name: 'Reliability' }), row(2, { code_name: 'Cost', start: 5 })];
  assert.deepEqual(sortRows(rows, 'code', 'ascending').map((r) => r.coding_id), [2, 3, 1], 'the two Costs keep their order in the document');
  assert.deepEqual(sortRows(rows, 'code', 'descending').map((r) => r.coding_id), [1, 2, 3]);
  assert.deepEqual(sortRows(rows, null, 'ascending').map((r) => r.coding_id), [2, 1, 3], 'no column: document order');
});

test('stretches of one recording are in the order of their times, whatever order they were coded in', () => {
  const span = (id: number, from: number) => row(id, { source_name: 'Interview', start: 0, end: 0, region: { shape: 'span', start_ms: from, end_ms: from + 1000 } });
  const rows = [span(-3, 9000), span(-2, 1000), span(-1, 5000)];
  assert.deepEqual(sortRows(rows, null, 'ascending').map((r) => r.coding_id), [-2, -1, -3]);
});

test('numbers inside names sort as numbers', () => {
  const rows = [row(1, { source_name: 'Doc 10' }), row(2, { source_name: 'Doc 9' }), row(3, { source_name: 'Doc 2' })];
  assert.deepEqual(sortRows(rows, 'source', 'ascending').map((r) => r.source_name), ['Doc 2', 'Doc 9', 'Doc 10']);
});

test('an excerpt is shortened at a space with an ellipsis, and a short one is left alone', () => {
  assert.equal(previewText('A short line.'), 'A short line.');
  const long = 'It is generally okay, but reliability can be a problem. The bus is often late, especially in the mornings.';
  const cut = previewText(long, 60);
  assert.ok(cut.endsWith('…') && cut.length <= 61, cut);
  assert.ok(!/\s…$/.test(cut), 'no space before the ellipsis');
  assert.equal(previewText('  lots   of\n\nspace  '), 'lots of space');
});
