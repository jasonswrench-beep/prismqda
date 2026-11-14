import assert from 'node:assert/strict';
import test from 'node:test';
import { applyWorkingSet, describeWorkingSet, documentsOf, EVERYTHING, narrows, pruneWorkingSet, WorkingSetEmpty, type WorkingSet } from './workingSet';

const docs = (ids: number[]): WorkingSet => ({ ...EVERYTHING, sourceIds: ids });
const cases = (ids: number[]): WorkingSet => ({ ...EVERYTHING, caseIds: ids });
const codes = (ids: number[]): WorkingSet => ({ ...EVERYTHING, codeIds: ids });
const follow = { workingSet: true };

test('with nothing narrowed every request goes through exactly as written, and the marker never reaches the engine', () => {
  assert.equal(narrows(EVERYTHING), false);
  const args = { limit: 5, source_ids: [1, 2] };
  assert.equal(applyWorkingSet('analytics.words', args, EVERYTHING), args);
  assert.equal(applyWorkingSet('anything.else', args, docs([1])), args, 'a command the set does not reach is untouched');
  assert.deepEqual(applyWorkingSet('segments.list', { limit: 5, ...follow }, EVERYTHING), { limit: 5 }, 'the marker is taken off');
});

test('a list of passages is narrowed only for the pages that ask, so the examples of a code and the evidence behind a number see every passage', () => {
  assert.deepEqual(applyWorkingSet('segments.list', { limit: 9 }, docs([1, 2])), { limit: 9 }, 'not asked: not narrowed');
  assert.deepEqual(applyWorkingSet('segments.list', { coding_ids: [3, 4] }, codes([5])), { coding_ids: [3, 4] }, 'the evidence of a number is whole');
  assert.deepEqual(applyWorkingSet('segments.list', { limit: 9, ...follow }, docs([1, 2])), { limit: 9, source_ids: [1, 2] });
});

test('documents narrow the lists a tool asks for, and meet what the person already chose', () => {
  assert.deepEqual(applyWorkingSet('segments.list', { source_ids: [2, 3], ...follow }, docs([1, 2])), { source_ids: [2] }, 'only what is in both');
  assert.deepEqual(applyWorkingSet('segments.list', { source_ids: [3], ...follow }, docs([1, 2])), { source_ids: [] }, 'nothing in common is nothing');
  assert.deepEqual(applyWorkingSet('analytics.words', { limit: 3 }, docs([4])), { limit: 3, source_ids: [4] });
  assert.deepEqual(applyWorkingSet('search.run', { query: 'x' }, docs([4])), { query: 'x', source_ids: [4] });
  assert.deepEqual(applyWorkingSet('analytics.kwic', { word: 'x' }, docs([4])), { word: 'x', source_ids: [4] });
});

test('a set of cases narrows the pages that read documents to the documents those cases hold', () => {
  const held = [{ id: 1, source_ids: [10, 11] }, { id: 2, source_ids: [11, 12] }, { id: 3, source_ids: [13] }];
  assert.deepEqual(documentsOf(cases([1, 2]), held), [10, 11, 12]);
  assert.deepEqual(documentsOf({ sourceIds: [11, 13], caseIds: [1, 2], codeIds: null }, held), [11], 'documents and cases together: what is in both');
  assert.deepEqual(documentsOf(docs([5]), held), [5]);
  assert.equal(documentsOf(EVERYTHING, held), null);
  assert.deepEqual(applyWorkingSet('analytics.words', {}, cases([3]), held), { source_ids: [13] });
  assert.deepEqual(applyWorkingSet('ai.packet', { limit: 1 }, cases([9]), held), { limit: 1, source_ids: [] }, 'a case that holds no document is no document');
});

test('the original arguments are never changed', () => {
  const args = { source_ids: [1, 2, 3], workingSet: true };
  applyWorkingSet('segments.list', args, docs([1]));
  assert.deepEqual(args, { source_ids: [1, 2, 3], workingSet: true });
});

test('codes are asked for exactly when the person asked for none, so a parent does not bring in sub-codes that are not in the set', () => {
  assert.deepEqual(applyWorkingSet('segments.list', follow, codes([5, 6])), { code_ids: [5, 6], exact_codes: true });
  assert.deepEqual(applyWorkingSet('segments.list', { code_ids: [5, 9], ...follow }, codes([5, 6])), { code_ids: [5] }, 'what the person chose, cut to the set');
  assert.deepEqual(applyWorkingSet('ai.packet', { limit: 1 }, codes([7])), { limit: 1, code_ids: [7] });
});

test('where the engine reads an empty list of codes as every code, an empty answer is refused and not sent', () => {
  assert.throws(() => applyWorkingSet('analytics.words', { code_ids: [9] }, codes([1, 2])), WorkingSetEmpty, 'a code outside the set');
  assert.throws(() => applyWorkingSet('ai.packet', {}, codes([])), WorkingSetEmpty, 'a set with no code');
  assert.deepEqual(applyWorkingSet('analytics.words', { code_ids: [] }, codes([1, 2])), { code_ids: [1, 2] }, 'no code chosen is the set');
  const relations = applyWorkingSet('explore.relations', { code_ids: [], include_descendants: false }, codes([4, 5])) as { code_ids: number[] };
  assert.deepEqual(relations.code_ids, [4, 5], 'Code Relations with no code chosen follows the set');
  assert.throws(() => applyWorkingSet('explore.relations', { code_ids: [8] }, codes([4, 5])), WorkingSetEmpty);
  // segments.list reads an empty list as none, so it is narrowed, not refused
  assert.deepEqual(applyWorkingSet('segments.list', { code_ids: [9], ...follow }, codes([1])), { code_ids: [] });
});

test('Explore specs get the scope inside them, and their code lists are cut to the set', () => {
  const m = applyWorkingSet('explore.matrix', { rows: { kind: 'cases' }, code_ids: [1, 2, 3], include_descendants: false }, { sourceIds: [10], caseIds: [20, 21], codeIds: [2, 3, 4] }) as Record<string, unknown>;
  assert.deepEqual(m.scope, { source_ids: [10], case_ids: [20, 21] });
  assert.deepEqual(m.code_ids, [2, 3]);
  const e = applyWorkingSet('explore.matrix_evidence', { spec: { code_ids: [1], scope: { coder_ids: [3], source_ids: [10, 11] } }, row_key: 'a' }, docs([11, 12])) as { spec: { scope: Record<string, unknown> }; row_key: string };
  assert.deepEqual(e.spec.scope, { coder_ids: [3], source_ids: [11] }, 'the scope the person set is kept and cut');
  assert.equal(e.row_key, 'a');
  assert.deepEqual((applyWorkingSet('explore.summary', {}, cases([1])) as { scope: unknown }).scope, { case_ids: [1] });
  assert.deepEqual((applyWorkingSet('theme.overview', {}, docs([3])) as { scope: unknown }).scope, { source_ids: [3] }, 'the Themes page counts only what is in the set');
  assert.deepEqual((applyWorkingSet('explore.query', { rule: {}, result_unit: 'excerpts' }, docs([2])) as { scope: unknown }).scope, { source_ids: [2] });
});

test('cases narrow a cross-tabulation and the statistics, including a page kept in the notebook', () => {
  assert.deepEqual(applyWorkingSet('explore.crosstab', { row_variable: 'a', column_variable: 'b' }, cases([1, 2])), { row_variable: 'a', column_variable: 'b', case_ids: [1, 2] });
  assert.deepEqual(applyWorkingSet('explore.crosstab_cases', { spec: { row_variable: 'a', case_ids: [2, 3] } }, cases([1, 2])), { spec: { row_variable: 'a', case_ids: [2] } });
  assert.deepEqual(applyWorkingSet('stats.run', { procedure: 'describe', variables: ['x'] }, cases([5])), { procedure: 'describe', variables: ['x'], options: { case_ids: [5] } });
  assert.deepEqual(applyWorkingSet('stats.run', { procedure: 'describe', variables: ['x'], options: { level: 0.9, case_ids: [5, 6] } }, cases([6, 7])), { procedure: 'describe', variables: ['x'], options: { level: 0.9, case_ids: [6] } });
  const nb = applyWorkingSet('stats.notebook.run', { request: { procedure: 'describe', variables: ['x'] }, section: 's' }, cases([5])) as { request: { options: { case_ids: number[] } }; section: string };
  assert.deepEqual(nb.request.options.case_ids, [5], 'the page records the cases it used');
  assert.equal(nb.section, 's');
  assert.deepEqual(applyWorkingSet('stats.run', { procedure: 'describe', variables: ['x'] }, docs([1])), { procedure: 'describe', variables: ['x'] }, 'documents do not touch the statistics');
});

test('the final layer is asked for where a page can use it, and only when the person did not ask for a layer', () => {
  const fin: WorkingSet = { ...EVERYTHING, finalLayer: true };
  assert.equal(narrows(fin), true, 'choosing it narrows');
  assert.deepEqual(applyWorkingSet('segments.list', { limit: 9, ...follow }, fin), { limit: 9, layer: 'final' });
  assert.deepEqual(applyWorkingSet('segments.list', { limit: 9 }, fin), { limit: 9 }, 'the examples of a code and the evidence of a number are not asked of it');
  assert.deepEqual(applyWorkingSet('segments.list', { layer: 'coder', ...follow }, fin), { layer: 'coder' }, 'a layer that was asked for stays');
  assert.deepEqual((applyWorkingSet('explore.summary', {}, fin) as { scope: unknown }).scope, { layer: 'final' });
  const m = applyWorkingSet('explore.matrix', { rows: { kind: 'cases' }, code_ids: [1] }, { sourceIds: [3], caseIds: null, codeIds: null, finalLayer: true }) as { scope: Record<string, unknown> };
  assert.deepEqual(m.scope, { source_ids: [3], layer: 'final' });
  assert.deepEqual(describeWorkingSet(fin, { sources: 3, cases: 2, codes: 4 }), ['the final layer']);
  assert.equal(pruneWorkingSet(fin, { sources: [], cases: [], codes: [] }).finalLayer, true, 'the choice is kept when others are pruned');
});

test('the set is described in words, and forgets what is gone from the project', () => {
  assert.deepEqual(describeWorkingSet({ sourceIds: [1, 2, 3], caseIds: null, codeIds: [4] }, { sources: 12, cases: 5, codes: 20 }), ['3 of 12 documents', '1 of 20 codes']);
  assert.deepEqual(describeWorkingSet({ sourceIds: [1], caseIds: [], codeIds: null }, { sources: 1, cases: 4, codes: 1 }), ['1 of 1 document', '0 of 4 cases']);
  assert.deepEqual(pruneWorkingSet({ sourceIds: [1, 2, 9], caseIds: null, codeIds: [3, 4] }, { sources: [1, 2], cases: [], codes: [4] }), { sourceIds: [1, 2], caseIds: null, codeIds: [4] });
});
