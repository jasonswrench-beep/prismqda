import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  addTo, barShares, clausesOf, codesOfDraft, draftKey, emptyDraft, intensity, joinOr, limitKind, limitNumber, mapItem, metricNoun, monthSpan, newClause, newGroup, niceMax, normalizeDay, percentModes, percentSentence, percentText, percentWhole,
  problemWith, rangeWidth, readback, revive, toQuerySpec, crosstabPercentSentence, crosstabWhole, CROSSTAB_PERCENTS, type Clause, type Draft, type Names,
} from './explore';
import type { MatrixResult } from '../types';

const names: Names = {
  code: (id) => ({ 1: 'Reliability', 2: 'Cost', 3: 'Comfort' })[id] ?? `code ${id}`,
  source: (id) => `doc ${id}`,
  case: (id) => `case ${id}`,
  coder: (id) => (id === 1 ? 'Ada' : 'Bo'),
};

function withClauses(...clauses: Clause[]): Draft {
  const d = emptyDraft();
  d.root = { ...d.root, items: clauses };
  return d;
}

const code = (ids: number[], negate = false): Clause => ({ ...(newClause('code') as Extract<Clause, { type: 'code' }>), codeIds: ids, negate });

test('an empty draft cannot run, and says to add a condition', () => {
  assert.match(problemWith(emptyDraft()) ?? '', /no conditions yet/);
});

test('each kind of condition names what it is missing', () => {
  assert.match(problemWith(withClauses(newClause('code'))) ?? '', /no code chosen/);
  assert.match(problemWith(withClauses(newClause('also'))) ?? '', /no code chosen/);
  assert.match(problemWith(withClauses(newClause('text'))) ?? '', /no text/);
  assert.match(problemWith(withClauses(newClause('source'))) ?? '', /no document/);
  assert.match(problemWith(withClauses(newClause('case'))) ?? '', /no case/);
  assert.match(problemWith(withClauses(newClause('coder'))) ?? '', /no coder/);
  assert.match(problemWith(withClauses(newClause('variable'))) ?? '', /needs a variable and a value/);
  assert.match(problemWith(withClauses(newClause('rating'))) ?? '', /lowest or a highest/);
  const bad = newClause('rating') as Extract<Clause, { type: 'rating' }>;
  assert.match(problemWith(withClauses({ ...bad, min: '4', max: '2' })) ?? '', /lowest rating is higher/);
  assert.match(problemWith(withClauses({ ...bad, min: '9' })) ?? '', /1 to 5/);
  assert.equal(problemWith(withClauses(code([1]))), null);
});

test('an empty group is a problem even when everything else is fine', () => {
  const d = withClauses(code([1]));
  const g = newGroup('all');
  assert.match(problemWith({ ...d, root: addTo(d.root, d.root.id, g) }) ?? '', /group has no conditions/);
});

test('the draft becomes the rule the engine reads', () => {
  const rating = { ...(newClause('rating') as Extract<Clause, { type: 'rating' }>), min: '3' };
  const d = withClauses(code([1, 2]), { ...(newClause('text') as Extract<Clause, { type: 'text' }>), contains: ' delay ', negate: true }, rating);
  const spec = toQuerySpec(d);
  assert.equal(spec.result_unit, 'excerpts');
  assert.deepEqual(spec.rule, {
    type: 'group',
    match: 'any',
    items: [
      { type: 'code', code_ids: [1, 2], include_descendants: true },
      { type: 'text', contains: 'delay', negate: true },
      { type: 'rating', min: 3 },
    ],
  });
  const also = { ...(newClause('also') as Extract<Clause, { type: 'also' }>), codeIds: [2], within: 25 };
  const nested = withClauses(also);
  nested.sameCoder = false;
  assert.deepEqual(toQuerySpec(nested).rule, { type: 'group', match: 'any', items: [{ type: 'also_coded', code_ids: [2], within_chars: 25, include_descendants: true, same_coder: false }] });
});

test('groups nest, can be changed and removed by number, and give their codes to the matrix', () => {
  let d = withClauses(code([1]));
  const inner = newGroup('all');
  d = { ...d, root: addTo(d.root, d.root.id, inner) };
  d = { ...d, root: addTo(d.root, inner.id, code([2, 3])) };
  assert.equal(clausesOf(d.root).length, 2);
  assert.deepEqual(codesOfDraft(d), [1, 2, 3]);
  const spec = toQuerySpec(d);
  assert.equal(spec.rule.type === 'group' && spec.rule.items[1].type === 'group' && spec.rule.items[1].match, 'all');
  const target = clausesOf(d.root)[1];
  d = { ...d, root: mapItem(d.root, target.id, (i) => ({ ...(i as Clause), negate: true }) as Clause) };
  assert.equal((clausesOf(d.root)[1] as { negate: boolean }).negate, true);
  d = { ...d, root: mapItem(d.root, inner.id, () => null) };
  assert.equal(clausesOf(d.root).length, 1, 'removing a group removes what is in it');
});

test('the readback says what will be returned and each condition, with negation and nesting', () => {
  let d = withClauses(code([1, 2]), code([3], true));
  const text = { ...(newClause('text') as Extract<Clause, { type: 'text' }>), contains: 'bus' };
  const inner = { ...newGroup('all'), items: [text] };
  d = { ...d, root: { ...d.root, items: [...d.root.items, inner] } };
  const r = readback(d, names, { codings: 42, documents: 7 });
  assert.equal(r.summary, 'Look through 42 codings in 7 documents. Return distinct excerpts (a stretch of text is returned once, with every coding of it that matched) where at least one of these is true:');
  assert.deepEqual(r.lines, [
    { depth: 0, text: 'the code is Reliability or Cost' },
    { depth: 0, text: 'the code is not Comfort' },
    { depth: 0, text: 'all of these are true:' },
    { depth: 1, text: 'the words contain “bus”' },
  ]);
  assert.match(r.rules, /sub-codes/);
  const a = readback({ ...d, unit: 'assignments' }, names);
  assert.match(a.summary, /^Look through every coding in the project\. Return assignments \(each coding that matches\)/);
});

test('"also coded" reads as overlapping words, or as near them', () => {
  const also = newClause('also') as Extract<Clause, { type: 'also' }>;
  const overlap = readback(withClauses({ ...also, codeIds: [2] }), names).lines[0].text;
  assert.equal(overlap, 'the same words are also coded Cost');
  assert.equal(readback(withClauses({ ...also, codeIds: [2], negate: true }), names).lines[0].text, 'the same words are not also coded Cost');
  assert.equal(readback(withClauses({ ...also, codeIds: [2], within: 25 }), names).lines[0].text, 'a coding of Cost lies within 25 characters of the words');
});

test('a changed draft has a different key, and an unchanged one has the same', () => {
  const a = withClauses(code([1]));
  assert.equal(draftKey(a), draftKey({ ...a }));
  assert.notEqual(draftKey(a), draftKey(withClauses(code([1, 2]))));
  assert.notEqual(draftKey(a), draftKey({ ...a, unit: 'assignments' }));
  assert.notEqual(draftKey(a), draftKey({ ...a, descendants: false }));
});

test('lists read as words, and counts name what they count', () => {
  assert.equal(joinOr(['a']), 'a');
  assert.equal(joinOr(['a', 'b']), 'a or b');
  assert.equal(joinOr(['a', 'b', 'c']), 'a, b, or c');
  assert.equal(metricNoun('excerpts', 1), '1 excerpt');
  assert.equal(metricNoun('cases', 1200), '1,200 cases');
});

test('chart numbers: bar lengths share the longest, axes end on round numbers, and an empty cell is never shaded', () => {
  assert.deepEqual(barShares([2, 4, 0]), [0.5, 1, 0]);
  assert.deepEqual(barShares([0, 0]), [0, 0]);
  assert.equal(niceMax(7), 10);
  assert.equal(niceMax(83), 100);
  assert.equal(niceMax(2.2), 2.5);
  assert.equal(niceMax(0), 1);
  assert.equal(intensity(0, 10), 0);
  assert.equal(intensity(10, 10), 1);
  assert.ok(intensity(1, 100) >= 0.12, 'a small count is still visibly shaded');
});

// ---- dates and ranges in a query

const coded = (from: string, to: string, negate = false): Clause => ({ ...(newClause('coded') as Extract<Clause, { type: 'coded' }>), from, to, negate });
const between = (name: string, from: string, to: string, negate = false): Clause => ({ ...(newClause('between') as Extract<Clause, { type: 'between' }>), name, from, to, negate });

test('a limit is a date only when it is a real day written year-month-day, and a number only when it is plainly one', () => {
  assert.equal(limitKind('2024-03-14'), 'date');
  assert.equal(limitKind(' 2024/3/4 '), 'date');
  for (const bad of ['2024-02-30', '3/14/2024', '14 March 2024', '2024-13-01', '2024-03', '']) assert.equal(limitKind(bad), null, bad);
  assert.equal(limitKind('42'), 'number');
  assert.equal(limitKind('-3.5'), 'number');
  assert.equal(limitKind('1,200'), 'number');
  for (const bad of ['abc', '12px', '1e5', '1,20', '.']) assert.equal(limitKind(bad), null, bad);
  assert.equal(normalizeDay('2024/3/4'), '2024-03-04');
  assert.equal(limitNumber('1,200.5'), 1200.5);
});

test('a date condition says what is wrong with its days, and becomes the range the engine reads with days written in full', () => {
  assert.match(problemWith(withClauses(coded('', ''))) ?? '', /lowest or a highest/);
  assert.match(problemWith(withClauses(coded('3/14/2024', ''))) ?? '', /year-month-day/);
  assert.match(problemWith(withClauses(coded('2024-04-02', '2024-03-10'))) ?? '', /first limit is above/);
  assert.equal(problemWith(withClauses(coded('2024-03-10', ''))), null, 'one limit is enough');
  const spec = toQuerySpec(withClauses(coded('2024/3/1', '2024-03-31', true)));
  assert.deepEqual(spec.rule, { type: 'group', match: 'any', items: [{ type: 'coded_between', from: '2024-03-01', to: '2024-03-31', negate: true }] });
  assert.deepEqual(toQuerySpec(withClauses(coded('', '2024-03-31'))).rule, { type: 'group', match: 'any', items: [{ type: 'coded_between', to: '2024-03-31' }] });
});

test('a range of a case variable needs both limits to be dates or both to be numbers', () => {
  assert.match(problemWith(withClauses(between('', '1', '2'))) ?? '', /needs a variable/);
  assert.match(problemWith(withClauses(between('Age', '', ''))) ?? '', /lowest or a highest/);
  assert.match(problemWith(withClauses(between('Age', '2024-01-01', '50'))) ?? '', /both limits as dates .* or both as numbers/);
  assert.match(problemWith(withClauses(between('Age', 'abc', ''))) ?? '', /both limits as dates/);
  assert.match(problemWith(withClauses(between('Age', '50', '40'))) ?? '', /first limit is above/);
  assert.equal(problemWith(withClauses(between('Age', '40', ''))), null);
  assert.deepEqual(toQuerySpec(withClauses(between('Age', '30', '1,200'))).rule, { type: 'group', match: 'any', items: [{ type: 'case_variable_between', name: 'Age', from: '30', to: '1,200' }] });
  assert.deepEqual(toQuerySpec(withClauses(between('Visit', '2024/1/2', ''))).rule, { type: 'group', match: 'any', items: [{ type: 'case_variable_between', name: 'Visit', from: '2024-01-02' }] });
});

test('the readback tells a date or a range in words', () => {
  const lines = (c: Clause) => readback(withClauses(c), names).lines.map((l) => l.text);
  assert.deepEqual(lines(coded('2024-03-01', '2024-03-31')), ['the coding was made from 2024-03-01 to 2024-03-31']);
  assert.deepEqual(lines(coded('2024-03-01', '', true)), ['the coding was not made on or after 2024-03-01']);
  assert.deepEqual(lines(between('Age', '40', '')), ["the case's Age is 40 or more"]);
  assert.deepEqual(lines(between('Age', '', '29', true)), ["the case's Age is not 29 or less"]);
  assert.deepEqual(lines(between('Visit', '2024-01-01', '2024-06-30')), ["the case's Visit is from 2024-01-01 to 2024-06-30"]);
  assert.deepEqual(lines(between('Visit', '2024-01-01', '')), ["the case's Visit is on or after 2024-01-01"]);
});

// ---- percentages

const zero = { assignments: 0, excerpts: 0, documents: 0, cases: 0 };
const matrix: MatrixResult = {
  rows: [
    { key: 'value:A', label: 'A', size: 4, cells: [{ assignments: 6, excerpts: 5, documents: 3, cases: 2 }, { assignments: 2, excerpts: 2, documents: 1, cases: 1 }], total: { assignments: 7, excerpts: 6, documents: 3, cases: 2 } },
    { key: 'value:B', label: 'B', size: 0, cells: [zero, zero], total: zero },
  ],
  columns: [
    { code_id: 1, name: 'X', total: { assignments: 8, excerpts: 7, documents: 3, cases: 2 } },
    { code_id: 2, name: 'Y', total: { assignments: 2, excerpts: 2, documents: 1, cases: 1 } },
  ],
  total: { assignments: 9, excerpts: 8, documents: 3, cases: 2 },
  universe: { assignments: 12, excerpts: 10, documents: 4, cases: 3 },
  notes: [],
};

test('a percentage says what it is a percentage of, and there is none when that whole is nothing', () => {
  const a = matrix.rows[0];
  const b = matrix.rows[1];
  assert.equal(percentWhole('row', 'assignments', a, 0, matrix), 7);
  assert.equal(percentWhole('column', 'assignments', a, 0, matrix), 8);
  assert.equal(percentWhole('all', 'assignments', a, 0, matrix), 9);
  assert.equal(percentWhole('group', 'cases', a, 0, matrix), 4);
  assert.equal(percentWhole('none', 'cases', a, 0, matrix), null);
  assert.equal(percentWhole('row', 'assignments', b, 0, matrix), null, 'a row with nothing in it has no whole');
  assert.equal(percentWhole('group', 'cases', b, 0, matrix), null, 'a group of no cases has none either');
  assert.equal(percentText(6, 7), '85.7%');
  assert.equal(percentText(1, 4), '25%', 'no needless decimal');
  assert.equal(percentText(1, 3), '33.3%');
  assert.equal(percentText(0, 5), '0%');
  assert.equal(percentText(3, null), '–');
});

test('only the percentages that have a stated whole are offered', () => {
  const ids = (metric: Parameters<typeof percentModes>[0], rows: Parameters<typeof percentModes>[1]) => percentModes(metric, rows).map((m) => m.id);
  assert.deepEqual(ids('assignments', 'cases'), ['none', 'row', 'column', 'all']);
  assert.deepEqual(ids('cases', 'documents'), ['none', 'row', 'column', 'all']);
  assert.deepEqual(ids('cases', 'variable'), ['none', 'row', 'column', 'all', 'group'], 'N is a whole only for cases in groups of cases');
  assert.deepEqual(ids('documents', 'variable'), ['none', 'row', 'column', 'all']);
  assert.deepEqual(ids('excerpts', 'months'), ['none', 'row', 'column', 'all']);
  assert.match(percentModes('excerpts', 'cases')[1].label, /row's excerpts/);
  assert.match(percentSentence('row', 'assignments', 'Group'), /its row's total: the number of assignments in that row/);
  assert.match(percentSentence('group', 'cases', 'Site'), /N: the number of cases that have that value of Site, with or without any coding/);
  assert.equal(percentSentence('none', 'cases', 'x'), '');
});

test('a month-by-month table says how far it reaches', () => {
  assert.equal(monthSpan(['2024-01', '2024-02', '2024-03']), '2024-01 to 2024-03');
  assert.equal(monthSpan(['2024-05']), '2024-05');
  assert.equal(monthSpan(['(no date)']), '');
});

// ---- crosstabs

test('a crosstab percentage is a part of its row, its column, or everything counted, and none when that is empty', () => {
  assert.equal(crosstabWhole('row', 3, 4, 8), 3);
  assert.equal(crosstabWhole('column', 3, 4, 8), 4);
  assert.equal(crosstabWhole('all', 3, 4, 8), 8);
  assert.equal(crosstabWhole('none', 3, 4, 8), null);
  assert.equal(crosstabWhole('row', 0, 4, 8), null, 'a row with no cases has no whole');
  assert.deepEqual(CROSSTAB_PERCENTS.map((m) => m.id), ['none', 'row', 'column', 'all']);
  assert.match(crosstabPercentSentence('row', 'Group', 'Site'), /cases that have that value of Group.*add up to 100/);
  assert.match(crosstabPercentSentence('column', 'Group', 'Site'), /that value of Site/);
  assert.match(crosstabPercentSentence('all', 'Group', 'Site'), /every case the table counts/);
  assert.equal(crosstabPercentSentence('none', 'Group', 'Site'), '');
});

test('a range width is a positive number, or nothing', () => {
  assert.equal(rangeWidth('10'), 10);
  assert.equal(rangeWidth(' 2.5 '), 2.5);
  for (const bad of ['', '0', '-5', 'ten', '2024-03-01']) assert.equal(rangeWidth(bad), undefined, bad);
});

// ---- opening a saved query

test('a saved draft comes back as a draft with fresh numbers, and anything that is not one is refused', () => {
  const original = withClauses(coded('2024-03-01', ''), between('Age', '30', '40', true));
  const nested = { ...original, root: { ...original.root, items: [...original.root.items, { ...newGroup('all'), items: [{ ...newClause('text'), contains: 'bus' } as Clause] }] } };
  const back = revive(JSON.parse(JSON.stringify(nested)));
  assert.ok(back, 'a draft that went through JSON comes back');
  assert.deepEqual(toQuerySpec(back!), toQuerySpec(nested as Draft), 'it asks the same question');
  const ids = (d: Draft) => [d.root.id, ...clausesOf(d.root).map((c) => c.id)];
  const before = new Set(ids(nested as Draft));
  assert.ok(ids(back!).every((id) => !before.has(id)), 'every number is new, so clauses added later cannot share one');
  assert.equal(new Set(ids(back!)).size, ids(back!).length, 'and no two share one');

  for (const bad of [null, 'text', 42, {}, { root: {}, unit: 'excerpts' }, { root: { kind: 'group', combine: 'sometimes', items: [] }, unit: 'excerpts' }, { root: { kind: 'group', combine: 'any', items: [{ kind: 'clause', type: 'telepathy' }] }, unit: 'excerpts' }, { root: { kind: 'group', combine: 'any', items: [] }, unit: 'pages' }]) {
    assert.equal(revive(bad), null, JSON.stringify(bad));
  }
  // A condition missing a field, or with the wrong kind of value, gets the default for that field rather than breaking the draft.
  const odd = revive({ root: { kind: 'group', combine: 'any', items: [{ kind: 'clause', type: 'code', codeIds: 'x', negate: true }, { kind: 'clause', type: 'coded', from: '2024-01-01' }] }, unit: 'assignments' });
  const [a, b] = clausesOf(odd!.root);
  assert.deepEqual([a.type === 'code' ? a.codeIds : null, a.negate], [[], true]);
  assert.deepEqual([b.type === 'coded' ? b.from : null, b.type === 'coded' ? b.to : null], ['2024-01-01', '']);
  assert.equal(odd!.unit, 'assignments');
  assert.equal(odd!.descendants, true, 'what is missing from a draft takes its usual value');
});
