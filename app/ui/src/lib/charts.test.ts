import assert from 'node:assert/strict';
import test from 'node:test';
import type { StatChart, StatResult } from '../types';
import { axisNumber, chartDescription, chartTable, chartTitle, countTicks, niceTicks, rowShares, scale, shorten, subjectName } from './charts';

const result = (patch: Partial<StatResult> = {}): StatResult => ({
  procedure: 'describe',
  variables: [
    { key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale' },
    { key: 'attr:Group', name: 'Group', kind: 'attribute', code_id: null, measure: 'nominal' },
  ],
  cases_total: 12,
  n_used: 12,
  n_excluded: 0,
  groups: [],
  assumptions: [],
  charts: [],
  details: {},
  ...patch,
});

test('ticks are round numbers that cover the data, and a single value still gets a scale', () => {
  assert.deepEqual(niceTicks(0, 7).ticks, [0, 2, 4, 6, 8]);
  assert.deepEqual(niceTicks(21, 55, 6).ticks, [20, 30, 40, 50, 60]);
  const one = niceTicks(5, 5);
  assert.ok(one.lo < 5 && one.hi > 5, 'a value is given room on both sides');
  assert.deepEqual(niceTicks(0.1, 0.9, 5).ticks, [0, 0.2, 0.4, 0.6, 0.8, 1]);
  assert.deepEqual(niceTicks(0, 10, 5).ticks, [0, 2.5, 5, 7.5, 10], 'a step of 2.5 is allowed, so the ticks stay few');
  assert.deepEqual(niceTicks(-3, 3, 5).ticks, [-4, -2, 0, 2, 4]);
  assert.deepEqual(niceTicks(Number.NaN, 1).ticks, [0, 1], 'a number that is not one does not break the chart');
  const t = niceTicks(0.1, 0.7, 7);
  assert.ok(t.ticks.every((v) => Math.abs(v * 10 - Math.round(v * 10)) < 1e-9 || Math.abs(v * 100 - Math.round(v * 100)) < 1e-9), 'no 0.30000000000000004');
});

test('count ticks are whole numbers from zero', () => {
  assert.deepEqual(countTicks(4).ticks, [0, 1, 2, 3, 4]);
  assert.deepEqual(countTicks(12).ticks, [0, 5, 10, 15]);
  assert.deepEqual(countTicks(0).ticks, [0, 1], 'an empty chart still has an axis');
  assert.ok(countTicks(37).hi >= 37);
});

test('a scale maps the domain onto the range, upside down when the range is', () => {
  const x = scale([0, 10], [100, 300]);
  assert.equal(x(0), 100);
  assert.equal(x(5), 200);
  assert.equal(x(10), 300);
  const y = scale([0, 4], [200, 0]);
  assert.equal(y(1), 150);
  assert.equal(scale([3, 3], [0, 10])(3), 0, 'no spread, no division by zero');
});

test('long labels are shortened for an axis and numbers are written briefly', () => {
  assert.equal(shorten('Short', 10), 'Short');
  assert.equal(shorten('A very long group name', 10), 'A very lo…');
  assert.equal(axisNumber(0.1 + 0.2), '0.3');
  assert.equal(axisNumber(1234.5), '1,234.5');
});

test('the shares of a table are per row, and an empty row is zeros', () => {
  assert.deepEqual(rowShares([[1, 3], [0, 0], [2, 2]]), [[0.25, 0.75], [0, 0], [0.5, 0.5]]);
});

const histogram: StatChart = {
  kind: 'histogram',
  subject: { type: 'variable', index: 0 },
  n: 12,
  bins: [
    { lo: 20, hi: 30, count: 3 },
    { lo: 30, hi: 40, count: 4 },
    { lo: 40, hi: 50, count: 3 },
    { lo: 50, hi: 60, count: 2 },
  ],
  mean: 37.6667,
  sd: 10.5,
  reference: null,
};

test('a histogram has a title, a text alternative that says what it shows, and a table with a row for each bar', () => {
  const r = result();
  assert.equal(chartTitle(histogram, r), 'Histogram of Score');
  assert.equal(chartDescription(histogram, r), '12 cases in 4 bars, from 20 to 60. The tallest bar, from 30 to 40, holds 4 cases. The mean is 37.67.');
  assert.match(chartDescription({ ...(histogram as Extract<StatChart, { kind: 'histogram' }>), reference: 35 }, r), /A dashed line marks 35\.00\.$/);
  const table = chartTable(histogram, r, String);
  assert.deepEqual(table.head, ['From (above)', 'Up to and including', 'Cases']);
  assert.deepEqual(table.rows[0], ['20 (included)', '30', '3'], 'the first bar includes its lower edge, and the others do not');
  assert.deepEqual(table.rows[1], ['30', '40', '4']);
  assert.equal(table.rows.length, 4);
});

test('a box plot says which group is highest and lowest and how many values lie beyond the whiskers', () => {
  const box: StatChart = {
    kind: 'box',
    subject: { type: 'variable', index: 0 },
    by: 1,
    groups: [
      { value: 'A', label: 'A', n: 4, lower_whisker: 10, q1: 10.75, median: 11.5, q3: 12.25, upper_whisker: 13, mean: 11.5, outliers: [] },
      { value: 'B', label: 'B', n: 4, lower_whisker: 15, q1: 15.75, median: 16.5, q3: 17.25, upper_whisker: 18, mean: 16.5, outliers: [30, 31] },
    ],
  };
  const r = result();
  assert.equal(chartTitle(box, r), 'Box plot of Score by Group');
  assert.equal(
    chartDescription(box, r),
    '2 groups. The box holds the middle half of each group, with the median as a bar across it. The highest median is B, 16.50, and the lowest is A, 11.50. 2 values lie beyond the whiskers.',
  );
  const table = chartTable(box, r, String);
  assert.equal(table.head.length, 9);
  assert.deepEqual(table.rows[1], ['B', '4', '15.00', '15.75', '16.50', '17.25', '18.00', '16.50', '30.00, 31.00']);
});

test('means with intervals, counts of categories, and a table of cases are told in words too', () => {
  const r = result();
  const means: StatChart = { kind: 'means', subject: { type: 'variable', index: 0 }, by: 1, level: 0.95, groups: [{ value: 'A', label: 'A', n: 4, mean: 11.5, lo: 9.4, hi: 13.6 }, { value: 'B', label: 'B', n: 1, mean: 16, lo: null, hi: null }] };
  assert.equal(chartTitle(means, r), 'Mean of Score by Group, with 95% intervals');
  assert.match(chartDescription(means, r), /^2 groups, each with the mean and the interval of that mean\. The highest mean is B, 16\.00, and the lowest is A, 11\.50\./);
  assert.deepEqual(chartTable(means, r, String).rows[1], ['B', '1', '16.00', '—', '—'], 'a group of one has no interval');

  const bars: StatChart = { kind: 'bars', subject: { type: 'variable', index: 1 }, valid: 10, categories: [{ value: 'N', label: 'North', count: 7 }, { value: 'S', label: 'South', count: 3 }] };
  assert.equal(chartTitle(bars, r), 'Counts of Group');
  assert.equal(chartDescription(bars, r), 'The commonest is North, with 7 of 10 cases.');
  assert.deepEqual(chartTable(bars, r, String).rows[0], ['North', '7', '70.0%']);

  const stacked: StatChart = { kind: 'stacked', row: 1, col: 0, row_values: ['A', 'B'], row_labels: ['A', 'B'], col_values: ['0', '1'], col_labels: ['0', '1'], counts: [[3, 1], [1, 3]] };
  const coded = result({ variables: [{ key: 'code:1:present', name: 'Trust', kind: 'code_present', code_id: 1, measure: 'nominal' }, ...result().variables.slice(1)] });
  assert.equal(chartTitle(stacked, coded), 'Has the code: Trust within each Group');
  assert.match(chartDescription(stacked, coded), /The largest share is No in A: 75\.0%\.$/, 'whether a case has a code reads Yes and No');
  const table = chartTable(stacked, coded, String);
  assert.deepEqual(table.head, ['Group', 'No', 'Yes']);
  assert.deepEqual(table.rows[0], ['A', '3 (75.0%)', '1 (25.0%)']);
});

test('a scatter says how many cases and how many are left out, a residual plot says what to look for, and a normal plot gives its correlation', () => {
  const r = result();
  const points = [1, 2, 3, 4].map((i) => ({ x: i, y: 2 * i, case_id: i }));
  const scatter: StatChart = { kind: 'scatter', x: { type: 'variable', index: 1 }, y: { type: 'variable', index: 0 }, points, omitted: 0, fit: { intercept: 0, slope: 2 }, zero_line: false };
  assert.equal(chartTitle(scatter, r), 'Score against Group');
  assert.equal(chartDescription(scatter, r), '4 cases, one point each. The line rises or falls by 2.00 for each unit of Group.');
  assert.match(chartDescription({ ...scatter, omitted: 500 } as StatChart, r), /500 are not drawn, to keep the picture readable; the numbers and the tests use them all\.$/);
  assert.deepEqual(chartTable(scatter, r, (id) => `Case ${id}`).rows[2], ['Case 3', '3.00', '6.00']);

  const residuals: StatChart = { kind: 'scatter', x: { type: 'fitted' }, y: { type: 'residuals' }, points, omitted: 0, fit: null, zero_line: true };
  assert.equal(chartTitle(residuals, r), 'the residuals against the fitted values');
  assert.match(chartDescription(residuals, r), /^4 cases\. A model that fits shows no pattern here: no curve and no funnel\./);

  const line = { intercept: 0, slope: 1 };
  const qq: StatChart = { kind: 'qq', subject: { type: 'residuals' }, points: [-1, 0, 1].map((v) => ({ theoretical: v, sample: v })), line };
  assert.equal(chartTitle(qq, r), 'Normal probability plot of the residuals');
  assert.match(chartDescription(qq, r), /^3 points, one for each value.*correlation of 1\.000\.$/);
  assert.equal(subjectName({ type: 'differences' }, r), 'the differences');
});

test('a chart of curves says which curves are drawn, and its table gives each curve at every case', () => {
  const r = result();
  const points = [1, 2, 3, 4].map((i) => ({ x: i, y: 2 * i, case_id: i }));
  const curves: StatChart = {
    kind: 'curves',
    x: { type: 'variable', index: 1 },
    y: { type: 'variable', index: 0 },
    points,
    omitted: 0,
    curves: [
      { kind: 'linear', points: [[1, 2], [4, 8]], at_points: [2, 4, 6, 8] },
      { kind: 'power', points: [[1, 2], [4, 8]], at_points: [2.1, 4.2, 5.9, 8.1] },
    ],
  };
  assert.equal(chartTitle(curves, r), 'Score against Group');
  assert.equal(chartDescription(curves, r), '4 cases, one point each, with 2 curves fitted: Linear, Power. Each curve has its own color and line style, and the table beside the chart gives the value of each at every case.');
  const table = chartTable(curves, r, (id) => `Case ${id}`);
  assert.deepEqual(table.head, ['Case', 'Group', 'Score', 'Linear (fitted)', 'Power (fitted)']);
  assert.deepEqual(table.rows[2], ['Case 3', '3.00', '6.00', '6.00', '5.90']);
  assert.match(chartDescription({ ...curves, omitted: 500 } as StatChart, r), /500 are not drawn/);
});
