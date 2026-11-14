import assert from 'node:assert/strict';
import test from 'node:test';
import type { AgreementCoefficients, AgreementReport } from '../types';
import { canMeasure, codeMeasure, coderLabel, disagreementShare, fixed, headline, intervalNote, intervalText, termLabel } from './agreement';

const coefficients = (patch: Partial<AgreementCoefficients> = {}): AgreementCoefficients => ({
  units: 20,
  units_compared: 20,
  coders: 2,
  percent_agreement: 0.8,
  cohen_kappa: 0.6,
  scott_pi: 0.6,
  fleiss_kappa: null,
  fleiss_units: 0,
  brennan_prediger: 0.6,
  gwet_ac1: 0.62,
  krippendorff_alpha: 0.61,
  ...patch,
});

const report = (patch: Partial<AgreementReport> = {}): AgreementReport => ({
  run_id: 1,
  mode: 'single_label',
  reporting_term: 'intercoder_reliability',
  coders: [],
  units_in_sample: 20,
  overall: coefficients(),
  intervals: {},
  target: 0.8,
  by_code: [],
  confusion: null,
  confusion_codes: [],
  disagreements: 4,
  notes: [],
  report_text: '',
  ...patch,
});

test('a coefficient is written to three places, or said to be undefined', () => {
  assert.equal(fixed(0.81234), '0.812');
  assert.equal(fixed(1), '1.000');
  assert.equal(fixed(-0.05), '-0.050');
  assert.equal(fixed(null), 'not defined');
  assert.equal(fixed(undefined), 'not defined');
  assert.equal(fixed(Number.NaN), 'not defined');
});

test('an interval is told as two numbers, and its level once under the table', () => {
  assert.equal(intervalText({ low: 0.5, high: 0.7234, level: 0.95, replications: 1000, undefined: 0 }), '0.500 to 0.723');
  assert.equal(intervalText(undefined), '');
  assert.equal(intervalNote(report()), '');
  const r = report({ intervals: { krippendorff_alpha: { low: 0.4, high: 0.8, level: 0.95, replications: 1000, undefined: 0 } } });
  assert.equal(intervalNote(r), 'Intervals cover 95 percent of 1,000 resamples of the units.');
});

test('a model is named as one, and its agreement is called concordance', () => {
  assert.equal(coderLabel({ name: 'Ada', role: 'human' }), 'Ada');
  assert.equal(coderLabel({ name: 'Model A', role: 'model' }), 'Model A (AI model)');
  assert.equal(termLabel('concordance'), 'concordance');
  assert.equal(termLabel('intercoder_reliability'), 'intercoder reliability');
});

test('two different coders are needed before anything can be measured', () => {
  assert.equal(canMeasure([]), false);
  assert.equal(canMeasure([1]), false);
  assert.equal(canMeasure([1, 1]), false);
  assert.equal(canMeasure([1, 2]), true);
});

test('the headline says whether alpha meets the target, and what to do when it cannot be worked out', () => {
  assert.equal(headline(report()), "20 units compared. Krippendorff's alpha is 0.610, which is below the target of 0.800.");
  assert.equal(headline(report({ units_in_sample: 1, overall: coefficients({ krippendorff_alpha: 0.9 }) })), "1 unit compared. Krippendorff's alpha is 0.900, which meets the target of 0.800.");
  assert.equal(headline(report({ overall: coefficients({ krippendorff_alpha: null, percent_agreement: 1 }) })), "20 units compared. Krippendorff's alpha is not defined for these ratings; percent agreement is 1.000.");
});

test('a code-by-code run says how many codes are below the target', () => {
  const code = (below: boolean) => ({ code_id: 1, name: 'x', below_target: below, detail: { agreement: coefficients(), both: 0, first_only: 0, second_only: 0, neither: 0, overlap: null, prevalence_index: null, bias_index: null, pabak: null } });
  assert.equal(headline(report({ mode: 'per_code', overall: null, by_code: [] })), '20 units compared, but no code was applied.');
  assert.equal(headline(report({ mode: 'per_code', overall: null, by_code: [code(false), code(false)] })), '20 units compared, code by code. Every code meets the target of 0.800.');
  assert.equal(headline(report({ mode: 'per_code', overall: null, by_code: [code(true), code(false), code(true)] })), '20 units compared, code by code. 2 of 3 codes are below the target of 0.800.');
  assert.equal(codeMeasure({ ...code(false), detail: { ...code(false).detail, agreement: coefficients({ krippendorff_alpha: null, percent_agreement: 0.5 }) } }), 0.5);
});

test('the disagreements are told as a count and a share', () => {
  assert.equal(disagreementShare(report()), '4 of 20 units, 20 percent.');
  assert.equal(disagreementShare(report({ overall: null, units_in_sample: 0 })), '');
});
