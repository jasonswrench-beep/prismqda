import assert from 'node:assert/strict';
import test from 'node:test';
import type { TransformSpec, VariableDef } from '../types';
import { cellShown, cellSpoken, codesToText, dateParts, derivedLine, formulaName, groupFunctions, labelsToText, parseCodes, parseCuts, parseLabels, parseValueList, sampleSizeWords, specProblem, transformKinds } from './variables';

const def = (over: Partial<VariableDef> = {}): VariableDef => ({ id: 1, name: 'Trained', level: 'nominal', value_labels: { '1': 'Yes', '2': 'No' }, missing_codes: ['99'], derived: null, ...over });

test('value labels are written one to a line and read back', () => {
  const labels = { '1': 'Strongly agree', '2': 'Agree', '3': 'Neither = nor' };
  const r = parseLabels(labelsToText(labels));
  assert.equal(r.error, null);
  assert.deepEqual(r.labels, labels, 'an equals sign inside the words is kept');
  assert.deepEqual(parseLabels('\n 1 = Yes \n\n 2 = No\n').labels, { '1': 'Yes', '2': 'No' });
});

test('a label line that cannot be read says which line and why', () => {
  assert.match(parseLabels('1 = Yes\nNo').error ?? '', /Line 2 has no equals sign/);
  assert.match(parseLabels(' = Yes').error ?? '', /Line 1 has no value/);
  assert.match(parseLabels('1 =').error ?? '', /Line 1 has no words/);
  assert.match(parseLabels('1 = Yes\n1 = No').error ?? '', /labeled twice/);
});

test('missing values are read from commas or lines, at most three', () => {
  assert.deepEqual(parseCodes('99, -1\n 98;99').codes, ['99', '-1', '98'], 'a repeat counts once');
  assert.equal(parseCodes('99, -1, 98').error, null);
  assert.match(parseCodes('1, 2, 3, 4').error ?? '', /At most 3/);
  assert.equal(codesToText(['99', '-1']), '99, -1');
  assert.deepEqual(parseCodes('   ').codes, []);
});

test('cut points are numbers in order, with no repeats', () => {
  assert.deepEqual(parseCuts('50, 18 30 100').cuts, [18, 30, 50, 100]);
  assert.deepEqual(parseCuts('0.5;1.5').cuts, [0.5, 1.5]);
  assert.match(parseCuts('').error ?? '', /at least one cut point/);
  assert.match(parseCuts('18, abc').error ?? '', /abc is not a number/);
  assert.match(parseCuts('18, 30, 18').error ?? '', /written twice/);
});

test('the values of a recoding rule are split on commas and lines', () => {
  assert.deepEqual(parseValueList(' 1, 2\n3 ;; '), ['1', '2', '3']);
  assert.deepEqual(parseValueList(''), []);
});

test('a cell shows its label, and says when its value is counted as missing', () => {
  const v = def();
  assert.deepEqual(cellShown(v, '1'), { text: 'Yes', missing: false });
  assert.deepEqual(cellShown(v, '7'), { text: '7', missing: false }, 'a value with no label shows as it is');
  assert.deepEqual(cellShown(v, '99'), { text: '99', missing: true });
  assert.deepEqual(cellShown(v, null), { text: '', missing: false });
  assert.equal(cellSpoken(v, null), 'empty');
  assert.equal(cellSpoken(v, '1'), 'Yes, value 1');
  assert.equal(cellSpoken(v, '7'), '7');
  assert.equal(cellSpoken(v, '99'), '99, counted as missing');
});

test('a name is written plain in a formula only when R would read it as a name', () => {
  assert.equal(formulaName('score'), 'score');
  assert.equal(formulaName('Score_2.b'), 'Score_2.b');
  assert.equal(formulaName('Score z'), '`Score z`');
  assert.equal(formulaName('2nd'), '`2nd`');
  assert.equal(formulaName('TRUE'), '`TRUE`');
  assert.equal(formulaName('NA'), '`NA`');
  assert.equal(formulaName('année'), 'année');
});

test('a made variable says how it was made', () => {
  const made = (transform: TransformSpec) => def({ derived: { transform, made_with: 'formula' } });
  assert.equal(derivedLine(def()), null);
  assert.equal(derivedLine(made({ target: 'T', kind: 'compute', formula: 'a + b' })), 'Computed: a + b');
  assert.equal(derivedLine(made({ target: 'T', kind: 'standardize', source: 'score' })), 'Z-scores of score');
  assert.equal(derivedLine(made({ target: 'T', kind: 'rank', source: 'score', descending: true })), 'Ranks of score, highest first');
  assert.equal(derivedLine(made({ target: 'T', kind: 'cut', source: 'age', cuts: [18, 30, 50] })), 'age cut at 18, 30, 50');
  assert.equal(derivedLine(made({ target: 'T', kind: 'recode', source: 'group', rules: [] })), 'Recoded from group');
});

test('a variable made from the coding names the codes it follows', () => {
  const made = (transform: TransformSpec) => def({ derived: { transform, made_with: 'codes' } });
  const name = (id: number) => (id === 3 ? 'Trust' : id === 4 ? 'Doubt' : `code ${id}`);
  assert.equal(derivedLine(made({ target: 'T', kind: 'from_codes', codes: [3], measure: 'present' }), name), 'From the coding: whether a case has Trust');
  assert.equal(derivedLine(made({ target: 'T', kind: 'from_codes', codes: [3], measure: 'count' }), name), 'From the coding: how many excerpts carry Trust');
  assert.equal(derivedLine(made({ target: 'T', kind: 'from_codes', codes: [3, 4], measure: 'dominant' }), name), 'From the coding: which of Trust, Doubt a case has most');
  assert.equal(derivedLine(made({ target: 'T', kind: 'from_codes', codes: [9], measure: 'present' })), 'From the coding: whether a case has code 9', 'without a lookup the id is shown');
});

test('a variable from codes needs a name and the right number of codes', () => {
  assert.match(specProblem({ target: 'X', kind: 'from_codes', codes: [], measure: 'present' }) ?? '', /Choose the code/);
  assert.equal(specProblem({ target: 'X', kind: 'from_codes', codes: [1], measure: 'count' }), null);
  assert.match(specProblem({ target: 'X', kind: 'from_codes', codes: [1], measure: 'dominant' }) ?? '', /at least two/);
  assert.equal(specProblem({ target: 'X', kind: 'from_codes', codes: [1, 2], measure: 'dominant' }), null);
  assert.match(specProblem({ target: ' ', kind: 'from_codes', codes: [1], measure: 'present' }) ?? '', /name/);
});

test('a transformation is ready only when what it needs is filled in', () => {
  assert.match(specProblem({ target: ' ', kind: 'compute', formula: 'a' }) ?? '', /name/);
  assert.match(specProblem({ target: 'X', kind: 'compute', formula: '  ' }) ?? '', /formula/);
  assert.equal(specProblem({ target: 'X', kind: 'compute', formula: 'a + 1' }), null);
  assert.match(specProblem({ target: 'X', kind: 'recode', source: '', rules: [] }) ?? '', /Choose the variable to recode/);
  assert.match(specProblem({ target: 'X', kind: 'recode', source: 's', rules: [] }) ?? '', /at least one rule/);
  assert.equal(specProblem({ target: 'X', kind: 'recode', source: 's', rules: [{ from: ['1'], to: 'Low' }] }), null);
  assert.match(specProblem({ target: 'X', kind: 'cut', source: 's', cuts: [] }) ?? '', /cut point/);
  assert.match(specProblem({ target: 'X', kind: 'rank', source: '' }) ?? '', /Choose the variable/);
  assert.equal(specProblem({ target: 'X', kind: 'standardize', source: 'score' }), null);
});

test('the data editor offers the date part, the random sample, and the group summary beside the first five ways', () => {
  assert.deepEqual(
    transformKinds().map((k) => k.id),
    ['compute', 'recode', 'standardize', 'rank', 'cut', 'date_part', 'sample', 'aggregate'],
  );
  const kind = (id: string) => transformKinds().find((k) => k.id === id)!;
  assert.equal(kind('date_part').needsSource, true);
  assert.equal(kind('aggregate').needsSource, true);
  assert.equal(kind('sample').needsSource, false, 'a sample draws from the cases, not from a variable');
  assert.equal(dateParts().length, 8);
  assert.deepEqual(groupFunctions().filter((f) => f.counts).map((f) => f.id), ['count', 'cases']);
});

test('the new transformations say how a variable was made, and what each needs before it is sent', () => {
  const def = (spec: TransformSpec): VariableDef => ({ id: 1, name: 'X', level: 'interval', value_labels: {}, missing_codes: [], derived: { transform: spec } });
  assert.equal(derivedLine(def({ target: 'X', kind: 'date_part', source: 'Seen', part: 'month' })), 'Month (1 to 12, named) of Seen');
  assert.equal(derivedLine(def({ target: 'X', kind: 'sample', size: { by: 'percent', value: 10 }, seed: 7 })), 'A random sample (10 percent), seed 7');
  assert.equal(derivedLine(def({ target: 'X', kind: 'sample', size: { by: 'count', value: 5 }, seed: 1, within: 'Site' })), 'A random sample (5 cases) drawn within Site, seed 1');
  assert.equal(derivedLine(def({ target: 'X', kind: 'aggregate', source: 'Score', by: ['Site', 'Wave'], function: 'mean' })), 'mean of Score for each group of Site and Wave');
  assert.equal(sampleSizeWords({ by: 'count', value: 3 }), '3 cases');

  assert.match(specProblem({ target: 'X', kind: 'date_part', source: '', part: 'year' }) ?? '', /dates/);
  assert.equal(specProblem({ target: 'X', kind: 'date_part', source: 'Seen', part: 'year' }), null);
  assert.match(specProblem({ target: 'X', kind: 'sample', size: { by: 'count', value: 0 } }) ?? '', /how many cases/);
  assert.match(specProblem({ target: 'X', kind: 'sample', size: { by: 'count', value: 2.5 } }) ?? '', /whole number/);
  assert.match(specProblem({ target: 'X', kind: 'sample', size: { by: 'percent', value: 150 } }) ?? '', /at most 100/);
  assert.equal(specProblem({ target: 'X', kind: 'sample', size: { by: 'percent', value: 25 } }), null);
  assert.match(specProblem({ target: 'X', kind: 'aggregate', source: '', by: ['Site'], function: 'mean' }) ?? '', /variable to summarize/);
  assert.match(specProblem({ target: 'X', kind: 'aggregate', source: 'Score', by: [], function: 'mean' }) ?? '', /groups/);
  assert.equal(specProblem({ target: 'X', kind: 'aggregate', source: 'Score', by: ['Site'], function: 'sd' }), null);
});
