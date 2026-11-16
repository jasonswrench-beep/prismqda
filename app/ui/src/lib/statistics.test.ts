import assert from 'node:assert/strict';
import test from 'node:test';
import type { StatAssumption, StatCorrelation, StatResult, StatVariable } from '../types';
import {
  assumptionLine,
  canBootstrap,
  chosenCasesLine,
  ciText,
  fmt,
  fmtNoZero,
  fmtOdds,
  goals,
  goalsInScope,
  groupable,
  isCategorical,
  magnitude,
  methodsFor,
  methodsInScope,
  numericUse,
  pText,
  predictorAccepts,
  resultReading,
  resultSentence,
  resultTables,
  resultTitle,
  usedLine,
  valueLabel,
  variableLabel,
} from './statistics';
import { analysisRows, analysisTables } from './statBoot';
import { compareTable, numberAt, resultsDiffer } from './statCompare';

const variable = (patch: Partial<StatVariable> = {}): StatVariable => ({
  key: 'attr:X',
  name: 'X',
  kind: 'attribute',
  code_id: null,
  measure: 'scale',
  valid: 10,
  missing: 0,
  not_a_number: 0,
  numeric: true,
  distinct: 10,
  categories: [],
  ...patch,
});
const categorical = (n: number, patch: Partial<StatVariable> = {}) =>
  variable({ key: 'attr:G', name: 'G', measure: 'nominal', numeric: false, distinct: n, categories: Array.from({ length: n }, (_, i) => ({ value: `v${i}`, label: `Value ${i}`, count: 5 })), ...patch });

const result = (patch: Partial<StatResult>): StatResult => ({
  procedure: 'describe',
  variables: [],
  cases_total: 20,
  n_used: 20,
  n_excluded: 0,
  groups: [],
  assumptions: [],
  charts: [],
  details: {},
  ...patch,
});
const vref = (name: string, kind: StatResult['variables'][0]['kind'] = 'attribute') => ({ key: `attr:${name}`, name, kind, code_id: null, measure: 'scale' as const });

test('numbers are written to a fixed count of decimals, and p-values and correlations have no zero in front', () => {
  assert.equal(fmt(3.14159), '3.14');
  assert.equal(fmt(1234.5, 1), '1,234.5');
  assert.equal(fmt(null), '—');
  assert.equal(fmt(Infinity), '∞');
  assert.equal(fmtNoZero(0.034, 3), '.034');
  assert.equal(fmtNoZero(-0.42), '-.42');
  assert.equal(fmtNoZero(1.5), '1.50', 'a value that can be over 1 keeps its zero or digit');
  assert.equal(pText(0.0341), 'p = .034');
  assert.equal(pText(0.0004), 'p < .001');
  assert.equal(pText(0.99999), 'p > .999');
  assert.equal(pText(0.05), 'p = .050');
  assert.equal(ciText([1.234, 5.678]), '[1.23, 5.68]');
  assert.equal(ciText(null), '—');
  assert.equal(fmtOdds(null), '∞', 'an infinite odds ratio, from a table with an empty cell');
  assert.equal(fmtOdds(4.5867), '4.59');
  assert.equal(fmtOdds(1234.5), '1,235');
});

test('the usual benchmarks name the size of an effect', () => {
  assert.equal(magnitude('d', 0.1), 'negligible');
  assert.equal(magnitude('d', -0.3), 'small', 'the sign does not matter');
  assert.equal(magnitude('d', 0.6), 'medium');
  assert.equal(magnitude('d', 1.4), 'large');
  assert.equal(magnitude('r', 0.35), 'medium');
  assert.equal(magnitude('share', 0.02), 'small');
  assert.equal(magnitude('share', 0.2), 'large');
  assert.equal(magnitude('v', 0.3, 1), 'medium');
  assert.equal(magnitude('v', 0.3, 4), 'large', 'a table with more degrees of freedom needs a smaller V for the same effect');
  assert.equal(magnitude('v', 0.15, 4), 'medium');
});

test('a code variable is named for what it counts, and its values read Yes and No', () => {
  assert.equal(variableLabel({ name: 'Age', kind: 'attribute' }), 'Age');
  assert.equal(variableLabel({ name: 'Trust', kind: 'code_present' }), 'Has the code: Trust');
  assert.equal(variableLabel({ name: 'Trust', kind: 'code_count' }), 'Excerpts coded: Trust');
  assert.equal(valueLabel('code_present', '1', '1'), 'Yes');
  assert.equal(valueLabel('code_present', '0', '0'), 'No');
  assert.equal(valueLabel('attribute', 'N', 'North site'), 'North site');
});

test('a variable is a number, a rank, or neither, and groups need at least two categories', () => {
  assert.equal(numericUse(variable()), 'scale');
  assert.equal(numericUse(variable({ measure: 'ordinal', numeric: true })), 'rank');
  assert.equal(numericUse(variable({ measure: 'ordinal', numeric: false })), null, 'an ordinal variable whose values are words has no numbers to rank');
  assert.equal(numericUse(variable({ measure: 'nominal', numeric: false })), null);
  assert.equal(numericUse(variable({ measure: 'nominal', kind: 'code_present', numeric: true })), 'rank');
  assert.equal(numericUse(variable({ measure: 'date' })), null);
  assert.equal(numericUse(variable({ valid: 0 })), null);
  assert.equal(groupable(categorical(3)), true);
  assert.equal(groupable(categorical(1)), false);
  assert.equal(groupable(categorical(3, { measure: 'text' })), false);
});

test('only the tests that are valid for the variables are offered', () => {
  const scale = variable();
  const rank = variable({ measure: 'ordinal', numeric: true });
  const two = categorical(2);
  // The tests of the first set: the later ones are in the test that follows.
  const first = new Set(['describe', 'one_sample_t', 'independent_t', 'paired_t', 'one_way_anova', 'chi_square', 'fisher', 'pearson', 'spearman', 'kendall', 'mann_whitney', 'wilcoxon', 'kruskal_wallis', 'linear_regression']);
  const names = (g: Parameters<typeof methodsFor>[0], v: Parameters<typeof methodsFor>[1], n = 0) => methodsFor(g, v, n).map((m) => m.procedure).filter((p) => first.has(p));
  assert.deepEqual(names('compare_groups', [scale, two], 2), ['independent_t', 'mann_whitney']);
  assert.deepEqual(names('compare_groups', [scale, categorical(3)], 3), ['one_way_anova', 'kruskal_wallis']);
  assert.deepEqual(names('compare_groups', [rank, two], 2), ['mann_whitney'], 'ranks cannot be averaged, so no t test');
  assert.deepEqual(names('compare_groups', [scale, categorical(3)], 1), [], 'one group is nothing to compare');
  assert.deepEqual(names('compare_groups', [scale, undefined], 2), [], 'a role not yet filled offers nothing');
  assert.deepEqual(names('compare_paired', [scale, scale]), ['paired_t', 'wilcoxon']);
  assert.deepEqual(names('compare_paired', [scale, rank]), ['wilcoxon']);
  assert.deepEqual(names('against_value', [scale]), ['one_sample_t', 'wilcoxon']);
  assert.deepEqual(names('against_value', [rank]), ['wilcoxon']);
  assert.deepEqual(names('association', [two, two]), ['chi_square', 'fisher']);
  assert.deepEqual(names('association', [two, categorical(3)]), ['chi_square'], 'Fisher is for two by two');
  assert.deepEqual(names('correlate', [scale, scale]), ['pearson', 'spearman', 'kendall']);
  assert.deepEqual(names('correlate', [scale, rank]), ['spearman', 'kendall']);
  assert.deepEqual(names('describe', [scale]), ['describe']);
});

test('the tests added later are offered only for the variables they are valid for', () => {
  const scale = variable();
  const rank = variable({ measure: 'ordinal', numeric: true, categories: [{ value: '1', label: '1', count: 4 }, { value: '2', label: '2', count: 3 }, { value: '3', label: '3', count: 3 }] });
  const yesNo = categorical(2, { categories: [{ value: 'no', label: 'No', count: 5 }, { value: 'yes', label: 'Yes', count: 5 }] });
  const other = categorical(2, { key: 'attr:H', name: 'H', categories: [{ value: 'no', label: 'No', count: 5 }, { value: 'yes', label: 'Yes', count: 5 }] });
  const three = categorical(3);
  const later = new Set(['shapiro_wilk', 'ks_normal', 'runs', 'binomial', 'goodness_of_fit', 'sign', 'friedman', 'ks_two', 'median_test', 'moses', 'mcnemar', 'ordinal_association', 'partial_correlation', 'two_way_anova', 'cronbach_alpha']);
  const names = (g: Parameters<typeof methodsFor>[0], v: Parameters<typeof methodsFor>[1], n = 0) => methodsFor(g, v, n).map((m) => m.procedure).filter((p) => later.has(p));
  assert.deepEqual(names('describe', [scale]), ['shapiro_wilk', 'ks_normal', 'runs'], 'a number can be tested for normality and for the order of its cases');
  assert.deepEqual(names('describe', [three]), ['binomial', 'goodness_of_fit'], 'categories give a share to test, and counts to compare with shares');
  assert.deepEqual(names('describe', [yesNo]), ['runs', 'binomial', 'goodness_of_fit'], 'two values can also be read as a sequence');
  const cats = (n: number) => Array.from({ length: n }, (_, i) => ({ value: String(i), label: String(i), count: 1 }));
  assert.deepEqual(names('describe', [variable({ valid: 12, distinct: 12, categories: cats(12) })]), ['shapiro_wilk', 'ks_normal', 'runs'], 'a number whose values are all different is not a set of categories');
  assert.deepEqual(names('describe', [variable({ valid: 40, distinct: 5, categories: cats(5) })]), ['shapiro_wilk', 'ks_normal', 'runs', 'binomial', 'goodness_of_fit'], 'a rating whose values repeat can be counted');
  assert.deepEqual(names('describe', [rank]), ['runs', 'binomial', 'goodness_of_fit'], 'a rank is cut in two for the runs test, has no normal curve to compare with, and its values are also categories to count');
  assert.deepEqual(names('against_value', [scale]), ['sign']);
  assert.deepEqual(names('compare_paired', [scale, scale]), ['sign']);
  assert.deepEqual(names('compare_groups', [scale, yesNo], 2), ['ks_two', 'median_test', 'moses']);
  assert.deepEqual(names('compare_groups', [scale, three], 3), ['median_test'], 'only the median test takes more than two groups');
  assert.deepEqual(names('compare_groups', [scale, three], 1), [], 'one group is nothing to compare');
  assert.deepEqual(names('association', [yesNo, other]), ['mcnemar'], 'the same two answers given twice');
  assert.deepEqual(names('association', [yesNo, three]), [], 'different categories are not an answer given twice');
  assert.deepEqual(names('association', [rank, rank]), ['ordinal_association'], 'two ordered scales');
  assert.deepEqual(names('association', [rank, three]), [], 'a nominal variable has no order');
  assert.deepEqual(names('repeated', [], 1), []);
  assert.deepEqual(names('repeated', [], 3), ['friedman']);
  assert.deepEqual(names('partial', [scale, scale], 0), [], 'nothing held constant');
  assert.deepEqual(names('partial', [scale, scale], 2), ['partial_correlation']);
  assert.deepEqual(names('two_factors', [scale, yesNo, three]), ['two_way_anova'], 'a number and two groupings');
  assert.deepEqual(names('two_factors', [scale, yesNo]), [], 'the second grouping is missing');
  assert.deepEqual(names('scale_reliability', [], 1), [], 'one item is not a scale');
  assert.deepEqual(names('scale_reliability', [], 3), ['cronbach_alpha']);
  assert.equal(goals().find((x) => x.id === 'scale_reliability')!.list?.min, 2);
  assert.deepEqual(goals().find((x) => x.id === 'two_factors')!.roles.map((r) => r.id), ['outcome', 'first', 'second']);
  const order = goals().map((g) => g.id);
  assert.ok(order.indexOf('two_factors') === order.indexOf('compare_groups') + 1 && order.indexOf('scale_reliability') === order.indexOf('repeated') + 1, 'the new goals sit beside the ones they are kin to');
  assert.equal(goals().find((x) => x.id === 'repeated')!.list?.min, 2);
  assert.equal(goals().find((x) => x.id === 'partial')!.list?.accepts(rank), false, 'partial correlation holds scale variables constant');
});

test('every goal has roles that accept what they should', () => {
  const g = goals();
  assert.equal(g.length, 13);
  assert.deepEqual(g.map((x) => x.id), ['describe', 'compare_groups', 'two_factors', 'compare_paired', 'repeated', 'scale_reliability', 'against_value', 'association', 'correlate', 'partial', 'predict', 'predict_binary', 'curve'], 'the goals added later sit beside their kin');
  const compare = g.find((x) => x.id === 'compare_groups')!;
  assert.equal(compare.roles[0].accepts(variable()), true);
  assert.equal(compare.roles[0].accepts(categorical(3)), false, 'a nominal variable is not a number to compare');
  assert.equal(compare.roles[1].accepts(categorical(3)), true);
  assert.equal(compare.roles[1].accepts(variable()), false, 'a scale variable is not a set of groups');
});

test('an independent t test reads as a report would, with the size of the effect, and a result that is not significant is not called marginal', () => {
  const r = result({
    procedure: 'independent_t',
    variables: [vref('Score'), vref('Group')],
    groups: [
      { value: 'A', label: 'A', n: 4, mean: 11.5, median: 11.5, case_ids: [] },
      { value: 'B', label: 'B', n: 4, mean: 16.5, median: 16.5, case_ids: [] },
    ],
    details: {
      t_test: {
        kind: 'welch',
        t: -5.477,
        df: 6,
        p: 0.0015,
        estimate: -5,
        se: 0.913,
        level: 0.95,
        ci: [-7.234, -2.766],
        groups: [
          { n: 4, mean: 11.5, sd: 1.29 },
          { n: 4, mean: 16.5, sd: 1.29 },
        ],
        d: -3.873,
        d_ci: [-6.5, -1.2],
        g: -3.3,
        g_ci: [-5.6, -1],
      },
    },
  });
  assert.equal(
    resultSentence(r),
    'Mean Score was lower in A (M = 11.50, SD = 1.29, n = 4) than in B (M = 16.50, SD = 1.29, n = 4). The difference was statistically significant, Welch t(6.00) = -5.48, p = .002, d = -3.87, 95% CI of the difference [-7.23, -2.77].',
  );
  assert.equal(resultTitle(r), "Welch's t test: Score by Group");
  assert.match(resultReading(r), /On average Score is 5\.00 lower in A than in B\. By the usual benchmarks that is a large difference \(d = -3\.87\)\.$/);

  const marginal = result({ ...r, details: { t_test: { ...r.details.t_test!, p: 0.053 } } });
  assert.match(resultSentence(marginal), /not statistically significant/);
  assert.match(resultReading(marginal), /Report it as not statistically significant, with the size of the effect and its interval, and do not describe it as marginal\./);
  assert.doesNotMatch(resultReading(r), /marginal/, 'a significant result gets no such caution');

  const tables = resultTables(r);
  assert.equal(tables.length, 2);
  assert.deepEqual(tables[0].head, ['Group', 'n', 'M', 'SD']);
  assert.deepEqual(tables[0].rows[0], ['A', '4', '11.50', '1.29']);
  assert.deepEqual(tables[1].head.slice(0, 3), ['t', 'df', 'p']);
  assert.equal(tables[1].rows[0][2], '.002', 'p without a zero');
});

test('a chi-square test says what the two variables are, the effect size, and the residuals table', () => {
  const r = result({
    procedure: 'chi_square',
    variables: [vref('Group'), vref('Site')],
    n_used: 11,
    details: {
      table: [[2, 2], [2, 2], [2, 1]],
      row_values: ['A', 'B', 'C'],
      col_values: ['N', 'S'],
      row_labels: ['A', 'B', 'C'],
      col_labels: ['North site', 'South site'],
      chi_square: {
        rows: 3,
        cols: 2,
        n: 11,
        expected: [[2.18, 1.82], [2.18, 1.82], [1.64, 1.36]],
        chi2: 0.1,
        df: 2,
        p: 0.95,
        yates: null,
        g2: 0.1,
        g2_p: 0.95,
        phi: null,
        cramers_v: 0.095,
        contingency_c: 0.094,
        standardized_residuals: [[-0.1, 0.1], [-0.1, 0.1], [0.3, -0.3]],
        adjusted_residuals: [[-0.2, 0.2], [-0.2, 0.2], [0.4, -0.4]],
        smallest_expected: 1.36,
        share_expected_below_5: 1,
      },
    },
  });
  assert.equal(resultSentence(r), "Group and Site were not statistically associated, χ²(2, N = 11) = 0.10, p = .950, Cramér's V = .10.");
  const tables = resultTables(r);
  assert.equal(tables.length, 3, 'the counts, the test, and the residuals');
  assert.deepEqual(tables[0].head, ['Group', 'North site', 'South site', 'Total']);
  assert.deepEqual(tables[0].rows[3], ['Total', '6', '5', '11']);
  assert.match(resultReading(r), /do not describe it as marginal/);
  assert.equal(usedLine(r), '11 cases were used of 20.'.replace('11 cases were used of 20.', usedLine(r)), 'the line is made');
  assert.match(usedLine(r), /^11 cases were used of 20; 0 had a missing value|^11 cases were used of 20\.$/);
});

test('a count of how many cases were used says how many were set aside', () => {
  assert.equal(usedLine(result({ n_used: 14, n_excluded: 6, cases_total: 20 })), '14 cases were used of 20; 6 had a missing value in a variable and were set aside.');
  assert.equal(usedLine(result({ n_used: 1, n_excluded: 0, cases_total: 1 })), '1 case was used of 1.');
});

test('a page run on some of the cases says so, and says that the working set cannot change it later', () => {
  assert.equal(chosenCasesLine(undefined), null);
  assert.equal(chosenCasesLine({}), null, 'every case: nothing to say');
  assert.equal(chosenCasesLine({ case_ids: [3, 4, 9] }), 'Run on 3 cases chosen when it was made (the working set), not on every case. Changing the working set later does not change this page.');
  assert.equal(chosenCasesLine({ case_ids: [3] }, true), 'Run on 1 case chosen in the working set when this page was made, not on every case.', 'the form for an export');
  assert.match(chosenCasesLine({ case_ids: [] }) ?? '', /^Run on 0 cases/, 'a set that left no case is still said');
});

test('assumptions are said in words and carry a word for their state as well as a status', () => {
  const a = (patch: Partial<StatAssumption>): StatAssumption => ({ id: 'sample_size', status: 'ok', group: null, numbers: {}, ...patch });
  assert.equal(assumptionLine(a({ status: 'ok', numbers: { n_min: 30 } })).badge, 'Fine');
  assert.equal(assumptionLine(a({ status: 'problem', numbers: { n_min: 2 } })).text, 'The smallest group has 2 cases. At least 3 cases are needed.');
  assert.match(assumptionLine(a({ status: 'caution', numbers: { n_min: 9 } })).text, /^The smallest group has 9 cases\. With fewer than 15/);
  assert.equal(assumptionLine(a({ id: 'independence', status: 'info' })).badge, 'Design');
  assert.match(assumptionLine(a({ id: 'independence', status: 'info' })).text, /Codings are never rows/);
  const skew = assumptionLine(a({ id: 'normality', status: 'caution', group: 'A', numbers: { skewness: 1.5, kurtosis: 0.4, n: 20 } }));
  assert.equal(skew.badge, 'Note');
  assert.match(skew.text, /^A: Noticeably skewed or heavy-tailed \(skewness 1\.50, excess kurtosis 0\.40\)/);
  const chi = assumptionLine(a({ id: 'expected_counts', status: 'problem', numbers: { smallest_expected: 0.8, share_below_5: 0.5 } }));
  assert.equal(chi.badge, 'Problem');
  assert.match(chi.text, /unreliable.*the smallest expected count is 0\.80, and 50\.0% of the cells expect fewer than 5/);
  assert.match(assumptionLine(a({ id: 'outliers', status: 'caution', numbers: { count: 1, n: 12 } })).text, /^1 value lies beyond 1\.5 interquartile ranges/);
  assert.match(assumptionLine(a({ id: 'equal_variances', status: 'caution', numbers: { levene_p: 0.01, sd_ratio: 3 } })).text, /The spread differs \(Levene's test p = \.010; the largest standard deviation is 3\.0 times the smallest\)/);
});

test('the Shapiro-Wilk test is said beside the shape, and is read with it', () => {
  const a = (numbers: StatAssumption['numbers']): StatAssumption => ({ id: 'normality', status: 'ok', group: null, numbers });
  const line = assumptionLine(a({ skewness: 0.1, kurtosis: -0.3, n: 40, w: 0.9641, shapiro_p: 0.2312 }));
  assert.match(line.text, /^Close to symmetric \(skewness 0\.10, excess kurtosis -0\.30\)\. Shapiro-Wilk test: W = \.964, p = \.231\./);
  assert.match(line.text, /read it with the shape and the plot/);
  assert.match(assumptionLine(a({ skewness: 0.1, kurtosis: -0.3, n: 40, w: 0.8, shapiro_p: 0.0004 })).text, /W = \.800, p < \.001\./);
  assert.doesNotMatch(assumptionLine(a({ skewness: 0.1, kurtosis: -0.3, n: 2 })).text, /Shapiro/, 'no test under three values');
});

test('Spearman says how its p-value was found, and the pairs after an analysis of variance say which test made them', () => {
  const spearman = (patch: Partial<StatCorrelation>) =>
    resultTables(
      result({
        procedure: 'spearman',
        variables: [vref('A'), vref('B')],
        details: { correlation: { kind: 'spearman', n: 8, r: 0.71, statistic: null, df: null, p: 0.047, exact: true, edgeworth: false, ci: null, level: 0.95, ...patch } },
      }),
    )[0];
  assert.equal(spearman({}).note, 'The p-value is exact (fewer than 10 pairs, no ties).');
  assert.deepEqual(spearman({}).head.slice(2, 3), ['Test']);
  assert.match(spearman({ n: 30, exact: false, edgeworth: true }).note ?? '', /series approximation to the exact test that R uses/);
  assert.match(spearman({ n: 30, exact: false, edgeworth: false, statistic: 2.1, df: 28 }).note ?? '', /t approximation \(there are ties/);

  const anova = (posthoc: 'tukey' | 'games_howell' | 'scheffe' | null) =>
    resultTables(
      result({
        procedure: 'one_way_anova',
        variables: [vref('Score'), vref('Group')],
        groups: [
          { value: 'A', label: 'A', n: 5, mean: 10, median: 10, case_ids: [] },
          { value: 'B', label: 'B', n: 5, mean: 12, median: 12, case_ids: [] },
          { value: 'C', label: 'C', n: 5, mean: 15, median: 15, case_ids: [] },
        ],
        details: {
          anova: { groups: [], n: 15, ss_between: 1, ss_within: 1, ss_total: 2, df_between: 2, df_within: 12, ms_between: 1, ms_within: 1, test: { f: 5, df1: 2, df2: 12, p: 0.02 }, eta_squared: 0.5, omega_squared: 0.4, welch: null, levene: null },
          pairwise: [{ a: 0, b: 1, difference: -2, se: 0.7, t: -2.9, df: 12, p: 0.013, p_adjusted: 0.031, ci: [-4, -0.1] }],
          adjustment: 'holm',
          posthoc,
          level: 0.95,
        },
      }),
    ).find((x) => x.title.startsWith('Pairs of groups'))!;
  assert.equal(anova(null).title, 'Pairs of groups, p-values adjusted by Holm');
  const tukey = anova('tukey');
  assert.equal(tukey.title, "Pairs of groups, Tukey's honestly significant difference");
  assert.deepEqual(tukey.head, ['Comparison', 'Mean difference', '95% CI', 't', 'df', 'p']);
  assert.deepEqual(tukey.rows[0].slice(-1), ['.031'], 'one p-value, the test of the method');
  assert.match(tukey.note ?? '', /hold for all the pairs together/);
  assert.equal(anova('games_howell').title, 'Pairs of groups, Games-Howell');
  assert.match(anova('games_howell').note ?? '', /does not assume equal spread/);
  assert.equal(anova('scheffe').title, "Pairs of groups, Scheffé's test");
});

// ---------------------------------------------------------------- regression and the bootstrap

const regression = (patch: Partial<StatResult['details']> = {}): StatResult =>
  result({
    procedure: 'linear_regression',
    variables: [vref('Score'), vref('Age'), vref('Site')],
    n_used: 11,
    n_excluded: 3,
    details: {
      models: [
        {
          n: 11,
          k: 2,
          df_residual: 8,
          terms: [
            { estimate: 5.5, se: 10.2, t: 0.54, p: 0.6, ci: [-18, 29], beta: null, vif: null },
            { estimate: 0.31, se: 0.2, t: 1.55, p: 0.16, ci: [-0.15, 0.77], beta: 0.35, vif: 1.1 },
            { estimate: -7.2, se: 3, t: -2.4, p: 0.043, ci: [-14.1, -0.3], beta: -0.5, vif: 1.1 },
          ],
          r_squared: 0.42,
          adj_r_squared: 0.28,
          f: { f: 2.9, df1: 2, df2: 8, p: 0.11 },
          sigma: 6.5,
          rss: 340,
          tss: 590,
          level: 0.95,
          breusch_pagan: null,
        },
      ],
      terms: [
        { variable: 'attr:Age', name: 'Age', kind: 'attribute', dummy: false, level: null, label: null, reference: null, step: 1 },
        { variable: 'attr:Site', name: 'Site', kind: 'attribute', dummy: true, level: 'S', label: 'South site', reference: 'North site', step: 1 },
      ],
      steps: [{ step: 1, terms: 2 }],
      changes: [],
      influential: [{ case_id: 12, case_name: 'Case 11', leverage: 0.4, cooks_distance: 1.2, studentized_residual: 5.1, residual: 40, fitted: 59 }],
      level: 0.95,
      ...patch,
    },
  });

test('a regression is told as a model summary, its coefficients, and the cases that pull the fit', () => {
  const tables = resultTables(regression());
  assert.deepEqual(
    tables.map((x) => x.title),
    ['Model summary for Score', 'Coefficients', 'Cases that pull the fit most'],
  );
  assert.deepEqual(tables[0].rows[0].slice(0, 6), ['1', '.42', '.28', '2.90', '2, 8', '.110']);
  assert.deepEqual(tables[1].head, ['Predictor', 'B', 'SE', 'β', 't', 'p', '95% CI of B', 'VIF']);
  assert.deepEqual(tables[1].rows[0].slice(0, 2), ['Intercept', '5.50']);
  assert.deepEqual(tables[1].rows[1], ['Age', '0.31', '0.20', '0.35', '1.55', '.160', '[-0.15, 0.77]', '1.10']);
  assert.equal(tables[1].rows[2][0], 'Site: South site (compared with North site)', 'a category is named against its reference');
  assert.equal(tables[2].rows[0][0], 'Case 11');
  assert.equal(resultTitle(regression()), 'Linear regression: Score from Age, Site');
});

test('models entered in steps get a table of coefficients for each step and a column of what each step adds', () => {
  const base = regression();
  const model = base.details.models![0];
  const two = regression({
    models: [{ ...model, k: 1, r_squared: 0.2, adj_r_squared: 0.1, terms: model.terms.slice(0, 2) }, model],
    steps: [
      { step: 1, terms: 1 },
      { step: 2, terms: 2 },
    ],
    changes: [{ r2_change: 0.22, f: 3.0, df1: 1, df2: 8, p: 0.12 }],
  });
  const tables = resultTables(two);
  assert.deepEqual(
    tables.map((x) => x.title),
    ['Model summary for Score', 'Coefficients, model 1', 'Coefficients, model 2', 'Cases that pull the fit most'],
  );
  assert.deepEqual(tables[0].rows[1].slice(6), ['.22', '3.00', '.120'], 'what step 2 adds');
  assert.deepEqual(tables[0].rows[0].slice(6), ['', '', ''], 'the first step adds nothing to compare with');
});

test('a regression sentence gives the model, then what each predictor goes with, and never calls a weak model marginal', () => {
  const r = regression();
  assert.equal(
    resultSentence(r),
    'The model with Age, Site explained 42.0% of the variance in Score (adjusted R² = .28), F(2, 8) = 2.90, p = .110. Each extra unit of Age went with 0.31 higher Score (B = 0.31, 95% CI [-0.15, 0.77], p = .160). South site had 7.20 lower Score than North site (B = -7.20, 95% CI [-14.10, -0.30], p = .043).',
  );
  assert.match(resultReading(r), /do not describe it as marginal/);
  const strong = regression({ models: [{ ...r.details.models![0], f: { f: 9, df1: 2, df2: 8, p: 0.009 } }] });
  assert.doesNotMatch(resultReading(strong), /marginal/);
});

test('the checks of a regression are said in words', () => {
  const a = (patch: Partial<StatAssumption>): StatAssumption => ({ id: 'collinearity', status: 'ok', group: null, numbers: {}, ...patch });
  assert.match(assumptionLine(a({ numbers: { max_vif: 1.4 } })).text, /not strongly related.*1\.4/);
  assert.match(assumptionLine(a({ status: 'problem', numbers: { max_vif: 12.3 } })).text, /highly related.*12\.3.*leave one out/);
  assert.match(assumptionLine(a({ id: 'cases_per_predictor', status: 'caution', numbers: { n: 40, terms: 4 } })).text, /^Only 10\.0 cases for each predictor term \(40 cases for 4 predictor terms\)/);
  assert.match(assumptionLine(a({ id: 'influence', status: 'caution', numbers: { count: 2, max_cooks_distance: 0.7, cut: 0.1 } })).text, /^2 cases have a Cook's distance above 0\.100\./);
  assert.match(assumptionLine(a({ id: 'influence', status: 'problem', numbers: { count: 1, max_cooks_distance: 1.4, cut: 0.1 } })).text, /above 1: the fit depends on it/);
  assert.match(assumptionLine(a({ id: 'equal_error_variance', status: 'caution', numbers: { statistic: 9, df: 2, p: 0.01 } })).text, /changes with the predictors \(Breusch-Pagan p = \.010\)/);
  assert.match(assumptionLine(a({ id: 'residual_shape', status: 'ok', numbers: { skewness: 0.2, kurtosis: 0.1 } })).text, /^The residuals are close to symmetric/);
  assert.match(assumptionLine(a({ id: 'large_residuals', status: 'caution', numbers: { count: 1, n: 30 } })).text, /^1 case has a residual more than 3 standard errors/);
  assert.equal(assumptionLine(a({ id: 'ordinal_as_number', status: 'caution', group: 'Rating' })).text, 'Rating is ordinal, and is used here as a number: its steps are assumed to be equal.');
});

test('a regression is offered for a number and at least one predictor, and a predictor can be a number or a categorical variable', () => {
  const scale = variable();
  assert.deepEqual(methodsFor('predict', [scale], 2).map((m) => m.procedure), ['linear_regression']);
  assert.deepEqual(methodsFor('predict', [scale], 0), [], 'a predictor is needed');
  assert.deepEqual(methodsFor('predict', [undefined], 2), [], 'and an outcome');
  assert.equal(predictorAccepts(variable()), true);
  assert.equal(predictorAccepts(categorical(3)), true);
  assert.equal(predictorAccepts(categorical(1)), false, 'one category explains nothing');
  assert.equal(predictorAccepts(variable({ measure: 'text' })), false);
  assert.equal(isCategorical(variable()), false);
  assert.equal(isCategorical(variable({ measure: 'ordinal', numeric: true })), false, 'an ordinal variable of numbers is used as a number');
  assert.equal(isCategorical(categorical(3)), true);
  const outcome = goals().find((g) => g.id === 'predict')!.roles[0];
  assert.equal(outcome.accepts(variable()), true);
  assert.equal(outcome.accepts(variable({ measure: 'ordinal', numeric: true })), false, 'the number to explain must be a scale variable');
});

test('a bootstrap can be added where it means something, and its intervals are a table', () => {
  assert.equal(canBootstrap('describe', [variable()]), true);
  assert.equal(canBootstrap('describe', [categorical(3)]), false, 'there is nothing to resample in a count of categories');
  assert.equal(canBootstrap('independent_t', []), true);
  assert.equal(canBootstrap('linear_regression', []), true);
  assert.equal(canBootstrap('logistic_regression', []), true);
  assert.equal(canBootstrap('chi_square', []), false);
  assert.equal(canBootstrap('one_way_anova', []), false);
  assert.equal(canBootstrap(null, []), false);

  const r = regression({
    bootstrap: {
      seed: 42,
      resamples: 2000,
      level: 0.95,
      items: [
        { statistic: 'coefficient', target: 0, interval: { estimate: 5.5, bias: 0.1, se: 11.2, used: 2000, failures: 0, level: 0.95, percentile: [-15, 31], bca: [-14, 32] } },
        { statistic: 'coefficient', target: 2, interval: { estimate: -7.2, bias: 0, se: 3.1, used: 1990, failures: 10, level: 0.95, percentile: [-13.5, -0.8], bca: null } },
        { statistic: 'r_squared', target: null, interval: null },
      ],
    },
  });
  const tables = resultTables(r);
  const boot = tables[tables.length - 1];
  assert.equal(boot.title, 'Bootstrap intervals');
  assert.deepEqual(boot.head, ['Statistic', 'Estimate', 'Bootstrap SE', '95% percentile CI', '95% BCa CI']);
  assert.deepEqual(boot.rows[0], ['Intercept', '5.50', '11.20', '[-15.00, 31.00]', '[-14.00, 32.00]']);
  assert.equal(boot.rows[1][0], 'B for Site: South site (compared with North site)');
  assert.equal(boot.rows[1][4], '—', 'no BCa interval where groups were resampled apart or it could not be found');
  assert.deepEqual(boot.rows[2], ['R²', '—', '—', 'could not be computed', '—']);
  assert.match(boot.note ?? '', /2,000 resamples of the cases, from seed 42; the same seed gives the same intervals/);
  assert.match(boot.note ?? '', /10 resamples gave no value and were left out/);
});

// ---------------------------------------------------------------- the tests added after the first set

const vr = (name: string) => vref(name);

test('the tests of normality and of order are titled, tabled, and told in a sentence', () => {
  const sw = result({ procedure: 'shapiro_wilk', variables: [vr('Score')], details: { shapiro_wilk: { n: 40, w: 0.9641, p: 0.2312 } } });
  assert.equal(resultTitle(sw), 'Shapiro-Wilk test of normality: Score');
  const [swTable] = resultTables(sw);
  assert.deepEqual(swTable.head, ['W', 'n', 'p']);
  assert.deepEqual(swTable.rows, [['.964', '40', '.231']]);
  assert.match(resultSentence(sw), /^The distribution of Score did not depart from a normal distribution by the Shapiro-Wilk test, W\(40\) = \.964, p = \.231\./);
  assert.match(resultSentence(result({ procedure: 'shapiro_wilk', variables: [vr('Score')], details: { shapiro_wilk: { n: 40, w: 0.8, p: 0.0004 } } })), /departed from a normal distribution.*p < \.001/);

  const ksBase = { d: 0.1234, n1: 40, n2: 0, p: 0.4, exact: true };
  const estimated = result({ procedure: 'ks_normal', variables: [vr('Score')], details: { ks: ksBase, mean: 10, sd: 2, estimated: true } });
  assert.match(resultTables(estimated)[0].note ?? '', /taken from the sample, so this p-value is too large/);
  assert.match(resultSentence(estimated), /too large/);
  const given = result({ procedure: 'ks_normal', variables: [vr('Score')], details: { ks: { ...ksBase, exact: false }, mean: 10, sd: 2, estimated: false } });
  assert.match(resultTables(given)[0].note ?? '', /normal distribution with the mean and standard deviation given\. The p-value is the limiting distribution/);
  assert.doesNotMatch(resultSentence(given), /too large/);

  const runs = (n: number, pe: number, pn: number) => result({ procedure: 'runs', variables: [vr('Series')], details: { n, cut: 5, cut_kind: 'median', runs: { runs: 6, n1: 10, n0: 10, mean: 11, sd: 2.1, z: -2.2, p_normal: pn, p_exact: pe } } });
  const few = runs(20, 0.03, 0.04);
  assert.match(resultTables(few)[0].note ?? '', /cut at the median, 5\.00.*exact \(50 cases or fewer\)/);
  assert.match(resultSentence(few), /did not|differed from a random order/);
  assert.match(resultSentence(few), /differed from a random order: there were 6 runs where 11\.00 are expected, p = \.030/);
  const many = runs(80, 0.03, 0.04);
  assert.match(resultTables(many)[0].note ?? '', /normal approximation with a continuity correction/);
  assert.match(resultSentence(many), /z = -2\.20, p = \.040/);
  const twoValues = result({ procedure: 'runs', variables: [vr('Answer')], details: { n: 20, cut: null, cut_kind: null, runs: { runs: 9, n1: 10, n0: 10, mean: 11, sd: 2.1, z: -0.9, p_normal: 0.4, p_exact: 0.4 } } });
  assert.match(resultTables(twoValues)[0].note ?? '', /The cases take two values/);
});

test('the tests of counts say what was counted, against what, and a result that is not significant is not called marginal', () => {
  const binomial = (p: number) => result({ procedure: 'binomial', variables: [vr('Site')], details: { success_label: 'Yes', binomial: { n: 40, successes: 26, p0: 0.5, estimate: 0.65, p, level: 0.95, ci: [0.48, 0.79] } } });
  assert.equal(resultTitle(binomial(0.1)), 'Binomial test: Site');
  const bt = resultTables(binomial(0.1))[0];
  assert.deepEqual(bt.head.slice(0, 3), ['Counted', 'Cases', 'n']);
  assert.deepEqual(bt.rows[0].slice(0, 5), ['Yes', '26', '40', '.650', '.500']);
  assert.match(resultSentence(binomial(0.1)), /65\.0% of the cases \(26 of 40\) were Yes, which did not differ from 50\.0%, exact p = \.100/);
  assert.match(resultReading(binomial(0.1)), /do not describe it as marginal/);
  assert.doesNotMatch(resultReading(binomial(0.01)), /marginal/);

  const gof = result({
    procedure: 'goodness_of_fit',
    variables: [vr('Cat')],
    details: {
      equal_shares: true,
      goodness_of_fit: { n: 30, chi2: 4.2, df: 2, p: 0.1225, expected: [10, 10, 10], residuals: [1.26, -0.32, -0.95], smallest_expected: 10 },
      counts: [
        { value: 'a', label: 'A', count: 14, expected: 10, share: 1 / 3, residual: 1.26 },
        { value: 'b', label: 'B', count: 9, expected: 10, share: 1 / 3, residual: -0.32 },
        { value: 'c', label: 'C', count: 7, expected: 10, share: 1 / 3, residual: -0.95 },
      ],
    },
  });
  const [counts, chi] = resultTables(gof);
  assert.deepEqual(counts.head, ['Category', 'Observed', 'Expected', 'Share expected', 'Residual']);
  assert.equal(counts.rows.length, 3);
  assert.deepEqual(chi.head, ['χ²', 'df', 'n', 'p']);
  assert.match(resultSentence(gof), /did not differ from equal shares, χ²\(2, N = 30\) = 4\.20, p = \.123/);
  assert.match(resultSentence({ ...gof, details: { ...gof.details, equal_shares: false } }), /did not differ from the expected shares/);

  const sign = (paired: boolean) => result({ procedure: 'sign', variables: paired ? [vr('Before'), vr('After')] : [vr('Score')], details: { paired, mu: 5, sign: { positive: 12, negative: 4, zeros: 2, median: 1.5, p: 0.0768, proportion_positive: 0.75, ci: [0.48, 0.93], level: 0.95 } } });
  assert.equal(resultTitle(sign(true)), 'Sign test: Before and After');
  assert.equal(resultTitle(sign(false)), 'Sign test: Score');
  assert.match(resultTables(sign(true))[0].head.join('|'), /Median difference/);
  assert.match(resultTables(sign(false))[0].head.join('|'), /Median\|/);
  assert.match(resultSentence(sign(true)), /Before was higher than After in 12 cases and lower in 4, with 2 equal and set aside; by the sign test that did not differ from an even split, p = \.077/);
  assert.match(resultSentence(sign(false)), /12 values of Score were above 5\.00 and 4 below/);
  const one = result({ procedure: 'sign', variables: [vr('Score')], details: { paired: false, mu: 5, sign: { positive: 1, negative: 4, zeros: 0, median: 3, p: 0.3, proportion_positive: 0.2, ci: [0.01, 0.7], level: 0.95 } } });
  assert.match(resultSentence(one), /^1 value of Score was above 5\.00 and 4 below/, 'one value is not many values');
});

test('the tests that compare groups name the groups and the measure of how they differ', () => {
  const groups = [
    { label: 'North', n: 12, median: 4, mean: 4, sd: 1 },
    { label: 'South', n: 10, median: 6, mean: 6, sd: 1 },
  ] as unknown as StatResult['groups'];
  const ks = result({ procedure: 'ks_two', variables: [vr('Score'), vr('Site')], groups, details: { ks: { n1: 12, n2: 10, d: 0.55, p: 0.04, exact: true } } });
  assert.equal(resultTitle(ks), 'Kolmogorov-Smirnov test of two groups: Score by Site');
  assert.match(resultSentence(ks), /The distributions of Score in North and South differed from each other, D = \.550, p = \.040/);
  assert.match(resultTables(ks)[1].note ?? '', /exact/);

  const median = result({ procedure: 'median_test', variables: [vr('Score'), vr('Site')], groups, n_used: 22, details: { median_test: { median: 5, above: [3, 8], at_or_below: [9, 2], chi2: 6.5, df: 1, p: 0.011, fisher_p: 0.02, smallest_expected: 4.5 } } });
  const [mt, mtest] = resultTables(median);
  assert.deepEqual(mt.rows, [['North', '12', '3', '9'], ['South', '10', '8', '2']]);
  assert.deepEqual(mtest.head, ['χ²', 'df', 'p', "Fisher's exact p"]);
  assert.match(resultSentence(median), /pooled median of Score \(5\.00\) differed between the groups of Site, χ²\(1, N = 22\) = 6\.50, p = \.011/);
  const noFisher = resultTables({ ...median, details: { median_test: { ...median.details.median_test!, fisher_p: null } } });
  assert.deepEqual(noFisher[1].head, ['χ²', 'df', 'p'], 'Fisher belongs to a table of two groups only');

  const moses = result({ procedure: 'moses', variables: [vr('Score'), vr('Site')], groups, details: { control: 'North', test: 'South', moses: { n_control: 12, n_test: 10, span: 14, p: 0.2, trim: 1, trimmed_span: 11, trimmed_p: 0.3, ties: false } } });
  assert.match(resultSentence(moses), /The control group, North, spanned 14 places in the pooled order of Score \(p = \.200\); with 1 trimmed from each end it spanned 11 \(p = \.300\)\. A small span means South was more spread out/);
  assert.match(resultReading(moses), /do not describe it as marginal/);
});

test('the tests of repeated and paired answers name the measures, the changes, and the order', () => {
  const groups = [
    { label: 'Time 1', n: 20, median: 3, mean: 3, sd: 1 },
    { label: 'Time 2', n: 20, median: 4, mean: 4, sd: 1 },
    { label: 'Time 3', n: 20, median: 5, mean: 5, sd: 1 },
  ] as unknown as StatResult['groups'];
  const friedman = result({ procedure: 'friedman', variables: [vr('Time 1'), vr('Time 2'), vr('Time 3')], groups, details: { friedman: { n: 20, k: 3, chi2: 12.4, df: 2, p: 0.002, kendall_w: 0.31, mean_ranks: [1.5, 2, 2.5] } } });
  assert.equal(resultTitle(friedman), "Friedman's test: Time 1, Time 2, Time 3");
  const [measures, test] = resultTables(friedman);
  assert.deepEqual(measures.rows, [['Time 1', '1.50', '3.00'], ['Time 2', '2.00', '4.00'], ['Time 3', '2.50', '5.00']]);
  assert.deepEqual(test.rows, [['12.40', '2', '20', '.002', '.310']]);
  assert.match(resultSentence(friedman), /The 3 measures differed from one another in their ranks, χ²\(2, N = 20\) = 12\.40, p = \.002, Kendall's W = \.310/);
  assert.match(resultReading(friedman), /Kendall's W says how much the cases agree.*Here it is large by the usual benchmarks \(\.310\)/);

  const mcnemar = result({ procedure: 'mcnemar', variables: [vr('Before'), vr('After')], details: { labels: ['No', 'Yes'], table: [[10, 7], [2, 11]], mcnemar: { b: 7, c: 2, chi2: 3.6, p: 0.058, chi2_corrected: 2.78, p_corrected: 0.096, p_exact: 0.18 } } });
  const [crossing, mc] = resultTables(mcnemar);
  assert.deepEqual(crossing.head, ['Before', 'No', 'Yes'], 'the corner of the table is named, so a screen reader has a header for every column');
  assert.deepEqual(crossing.rows, [['No', '10', '7'], ['Yes', '2', '11']]);
  assert.deepEqual(mc.head, ['Changed one way', 'Changed the other way', 'χ²', 'df', 'p', 'Exact p']);
  assert.match(resultSentence(mcnemar), /7 cases changed from No to Yes and 2 from Yes to No; McNemar's χ²\(1\) = 2\.78, p = \.096 \(exact p = \.180\)/);
  assert.match(resultReading(mcnemar), /do not describe it as marginal/);
  const single = result({ procedure: 'mcnemar', variables: [vr('Before'), vr('After')], details: { labels: ['No', 'Yes'], table: [[10, 1], [5, 4]], mcnemar: { b: 1, c: 5, chi2: 2.67, p: 0.1, chi2_corrected: 1.5, p_corrected: 0.22, p_exact: 0.22 } } });
  assert.match(resultSentence(single), /^1 case changed from No to Yes and 5 from Yes to No; McNemar's/, 'one case is not many cases');
});

test('the measures of order and the partial correlation are tabled with their standard errors, and read with the controls named', () => {
  const m = (estimate: number, ase: number, z: number | null, p: number | null) => ({ estimate, ase, ase0: ase, z, p });
  const ordinal = result({
    procedure: 'ordinal_association',
    variables: [vr('Rank'), vr('Grade')],
    details: { ordinal: { n: 50, concordant: 600, discordant: 200, gamma: m(0.5, 0.1, 5, 0.0001), tau_b: m(0.3, 0.07, 4.3, 0.0001), tau_c: m(0.28, 0.07, 4, 0.0002), somers_columns_dependent: m(0.29, 0.07, 4.1, 0.0001), somers_rows_dependent: m(0.31, 0.07, 4.4, 0.0001), somers_symmetric: 0.3 } },
  });
  assert.equal(resultTitle(ordinal), 'Association between ordered categories: Rank and Grade');
  const ot = resultTables(ordinal)[0];
  assert.equal(ot.rows.length, 5);
  assert.deepEqual(ot.rows[0], ["Goodman and Kruskal's gamma", '0.500', '0.100', '5.00', '< .001']);
  assert.match(ot.rows[3][0], /Somers' d \(Grade depends on Rank\)/);
  assert.match(ot.note ?? '', /600 pairs of cases rank the same way on both variables and 200 rank opposite ways\./);
  assert.match(resultSentence(ordinal), /Rank and Grade were positively related by the order of their values, gamma = \.500, tau-b = \.300, p < \.001/);
  const negative = resultSentence({ ...ordinal, details: { ordinal: { ...ordinal.details.ordinal!, gamma: m(-0.5, 0.1, -5, 0.0001), tau_b: m(-0.3, 0.07, -4.3, 0.0001) } } });
  assert.match(negative, /negatively related.*gamma = -\.500/);

  const partial = (p: number) => result({ procedure: 'partial_correlation', variables: [vr('Age'), vr('Income')], details: { controls: ['Education', 'Region'], zero_order: { r: 0.55, p: 0.001, n: 50, df: 48, ci: [0.3, 0.72], level: 0.95 } as unknown as StatCorrelation, partial: { n: 50, controls: 2, r: 0.4, t: 2.9, df: 46, p, ci: [0.13, 0.62], level: 0.95 } } });
  assert.equal(resultTitle(partial(0.006)), 'Partial correlation: Age and Income, holding constant Education, Region');
  const pt = resultTables(partial(0.006))[0];
  assert.deepEqual(pt.head.slice(0, 3), ['r', 'n', 'df']);
  assert.equal(pt.rows[0][2], '46', 'the degrees of freedom lose one for each control');
  assert.match(pt.note ?? '', /Without holding anything constant the correlation of the two is \.55\./);
  assert.match(resultSentence(partial(0.006)), /The positive relation between Age and Income, holding constant Education, Region, was statistically significant, partial r\(46\) = \.40, p = \.006, 95% CI \[0\.13, 0\.62\]/);
  assert.match(resultSentence(partial(0.2)), /was not statistically significant/);
  assert.match(resultReading(partial(0.2)), /A relation is not a cause\..*do not describe it as marginal/);
});

// ---------------------------------------------------------------- logistic regression

const logistic = (patch: Record<string, unknown> = {}): StatResult =>
  result({
    procedure: 'logistic_regression',
    variables: [vref('Site'), vref('Age')],
    n_used: 12,
    n_excluded: 2,
    details: {
      event: { value: 'S', label: 'South site' },
      other: { value: 'N', label: 'North site' },
      models: [
        {
          n: 12,
          k: 1,
          events: 6,
          non_events: 6,
          df_residual: 10,
          terms: [
            { estimate: -2.1, se: 1.5, z: -1.4, wald: 1.96, p: 0.16, ci: [-5.04, 0.84], odds_ratio: 0.1225, or_ci: [0.0065, 2.32], profile_ci: null, or_profile_ci: null },
            { estimate: 0.07, se: 0.04, z: 1.75, wald: 3.06, p: 0.08, ci: [-0.01, 0.15], odds_ratio: 1.0725, or_ci: [0.99, 1.16], profile_ci: [-0.006, 0.157], or_profile_ci: [0.994, 1.17] },
          ],
          null_deviance: 16.64,
          deviance: 13.2,
          log_likelihood: -6.6,
          null_log_likelihood: -8.32,
          aic: 17.2,
          bic: 18.2,
          lr: { chi2: 3.44, df: 1, p: 0.064 },
          mcfadden: 0.2,
          cox_snell: 0.25,
          nagelkerke: 0.33,
          hosmer_lemeshow: {
            chi2: 2.1,
            df: 1,
            p: 0.147,
            groups: [
              { n: 4, observed: 1, expected: 1.4 },
              { n: 4, observed: 2, expected: 2 },
              { n: 4, observed: 3, expected: 2.6 },
            ],
          },
          classification: { cutoff: 0.5, true_positive: 5, false_positive: 2, true_negative: 4, false_negative: 1, accuracy: 0.75, sensitivity: 5 / 6, specificity: 4 / 6, positive_predictive_value: 5 / 7, negative_predictive_value: 0.8 },
          auc: 0.75,
          iterations: 4,
          level: 0.95,
        },
      ],
      terms: [{ variable: 'attr:Age', name: 'Age', kind: 'attribute', dummy: false, level: null, label: null, reference: null, step: 1 }],
      steps: [{ step: 1, terms: 1 }],
      changes: [],
      blocks: [{ variable: 'attr:Age', name: 'Age', kind: 'attribute', step: 1, terms: 1, lr: { chi2: 3.44, df: 1, p: 0.064 } }],
      influential: [{ case_id: 12, case_name: 'Case 11', leverage: 0.4, cooks_distance: 1.2, standardized_residual: 3.1, fitted: 0.9, observed: 0 }],
      profile: true,
      level: 0.95,
      ...patch,
    } as unknown as StatResult['details'],
  });

test('a logistic regression is told as its fit, how well it predicts, the checks, the odds ratios, and what each variable adds', () => {
  const tables = resultTables(logistic());
  assert.deepEqual(
    tables.map((x) => x.title),
    ['Model fit for Site', 'How well the last model fits and predicts', 'Observed and predicted outcomes', 'Hosmer-Lemeshow test of fit', 'Groups of the Hosmer-Lemeshow test', 'Coefficients', 'Each variable left out', 'Cases that pull the fit most'],
  );
  assert.deepEqual(tables[0].rows[0], ['1', '13.20', '3.44', '1', '.064', '', '', '']);
  assert.deepEqual(tables[1].rows[0], ['.200', '.250', '.330', '17.20', '18.20', '.750', '75.0%']);
  assert.deepEqual(tables[2].head, ['Observed', 'Predicted North site', 'Predicted South site', 'Classified correctly']);
  assert.deepEqual(tables[2].rows, [['North site', '4', '2', '66.7%'], ['South site', '1', '5', '83.3%']]);
  assert.deepEqual(tables[3].rows[0], ['2.10', '1', '.147']);
  assert.deepEqual(tables[4].rows[0], ['1', '4', '1', '1.40']);
  assert.deepEqual(tables[5].head, ['Predictor', 'B', 'SE', 'z', 'p', 'Odds ratio', '95% CI of the odds ratio (Wald)', '95% CI of the odds ratio (profile)']);
  assert.deepEqual(tables[5].rows[1], ['Age', '0.07', '0.04', '1.75', '.080', '1.07', '[0.99, 1.16]', '[0.99, 1.17]']);
  assert.equal(tables[5].rows[0][7], '\u2014', 'the intercept has no profile interval');
  assert.deepEqual(tables[6].rows[0], ['Age', '1', '3.44', '1', '.064']);
  assert.equal(tables[7].rows[0][1], 'North site', 'the observed outcome of a flagged case, named');
  assert.equal(resultTitle(logistic()), 'Logistic regression: Site from Age');
});

test('a logistic regression without a profile interval has no such column, and an interval open on one side shows an infinity', () => {
  const base = logistic();
  const m = base.details.models![0] as unknown as { terms: Record<string, unknown>[] };
  const open = logistic({
    models: [{ ...(base.details.models![0] as object), terms: [m.terms[0], { ...m.terms[1], profile_ci: null, or_profile_ci: null, or_ci: [0.99, null] }] }] as unknown as StatResult['details']['models'],
    profile: false,
  });
  const coefficients = resultTables(open).find((x) => x.title === 'Coefficients')!;
  assert.equal(coefficients.head.length, 7);
  assert.equal(coefficients.rows[1][6], '[0.99, \u221e]');
});

test('a logistic sentence gives the model, then what each predictor does to the odds, and never calls a weak model marginal', () => {
  const r = logistic();
  assert.equal(
    resultSentence(r),
    'The model of Site (South site against North site) with Age did not fit better than a model with only an intercept, \u03c7\u00b2(1, N = 12) = 3.44, p = .064; Nagelkerke R\u00b2 = .330, and it classified 75.0% of the cases correctly (area under the curve = .750). Each extra unit of Age multiplied the odds of South site by 1.07 (95% CI [0.99, 1.16], p = .080).',
  );
  const reading = resultReading(r);
  assert.match(reading, /^An odds ratio above 1 means the odds of South site rise/);
  assert.match(reading, /area under the curve is \.750, which is acceptable at telling/);
  assert.match(reading, /do not describe it as marginal/);
  const strong = logistic({ models: [{ ...(r.details.models![0] as object), lr: { chi2: 9.1, df: 1, p: 0.003 }, auc: 0.93 }] as unknown as StatResult['details']['models'] });
  assert.doesNotMatch(resultReading(strong), /marginal/);
  assert.match(resultReading(strong), /which is excellent at telling/);
  assert.match(resultSentence(strong), /fitted better than a model with only an intercept/);
});

test('a yes-or-no outcome is offered a logistic regression, and only an outcome with two values', () => {
  assert.deepEqual(methodsFor('predict_binary', [categorical(2)], 1).map((m) => m.procedure), ['logistic_regression']);
  assert.deepEqual(methodsFor('predict_binary', [categorical(2)], 0), [], 'a predictor is needed');
  assert.deepEqual(methodsFor('predict_binary', [undefined], 2), [], 'and an outcome');
  const outcome = goals().find((g) => g.id === 'predict_binary')!.roles[0];
  assert.equal(outcome.accepts(categorical(2)), true);
  assert.equal(outcome.accepts(categorical(3)), false, 'three values are not a yes-or-no');
  assert.equal(outcome.accepts(categorical(1)), false);
  assert.equal(outcome.accepts(variable()), false, 'a scale variable with many values is not a yes-or-no');
});

test('the checks of a logistic regression are put in words', () => {
  const a = (patch: Partial<StatAssumption>): StatAssumption => ({ id: 'x', status: 'ok', group: null, numbers: {}, ...patch });
  assert.match(assumptionLine(a({ id: 'events_per_predictor', status: 'caution', numbers: { events: 6, terms: 1 } })).text, /^Only 6\.0 cases of the rarer outcome for each predictor term/);
  assert.match(assumptionLine(a({ id: 'events_per_predictor', status: 'problem', numbers: { events: 3, terms: 2 } })).text, /^Fewer than 5 cases of the rarer outcome/);
  assert.match(assumptionLine(a({ id: 'hosmer_lemeshow', status: 'ok', numbers: { p: 0.4 } })).text, /^The predicted probabilities match the outcomes/);
  assert.match(assumptionLine(a({ id: 'hosmer_lemeshow', status: 'caution', numbers: { p: 0.01 } })).text, /do not match the outcomes well/);
});

// ---------------------------------------------------------------- curve fits

const curveFit = (patch: Record<string, unknown> = {}): StatResult => {
  const term = (estimate: number, se: number, tt: number, p: number, ci: [number, number]) => ({ estimate, se, t: tt, p, ci, beta: null, vif: null });
  const model = (r2: number, adj: number, f: number, p: number, terms: ReturnType<typeof term>[]) => ({
    n: 10, k: terms.length - 1, df_residual: 8, terms, r_squared: r2, adj_r_squared: adj, f: { f, df1: terms.length - 1, df2: 8, p }, sigma: 1, rss: 8, tss: 80, level: 0.95, breusch_pagan: null,
  });
  return result({
    procedure: 'curve_fit',
    variables: [vref('Score'), vref('Age')],
    n_used: 10,
    details: {
      fits: [
        { kind: 'linear', n: 10, model: model(0.8, 0.775, 32, 0.0005, [term(1.5, 0.4, 3.75, 0.006, [0.6, 2.4]), term(0.5, 0.09, 5.66, 0.0005, [0.3, 0.7])]), r_squared_original: 0.8, rmse_original: 0.9, constants: [1.5, 0.5] },
        { kind: 'power', n: 10, model: model(0.9, 0.89, 72, 0.00003, [term(0.4, 0.2, 2, 0.08, [-0.06, 0.86]), term(0.6, 0.07, 8.5, 0.00003, [0.44, 0.76])]), r_squared_original: 0.85, rmse_original: 0.7, constants: [1.49, 0.6] },
        { kind: 'logarithmic', error: 'needs_positive_x' },
      ],
      level: 0.95,
      ...patch,
    } as unknown as StatResult['details'],
  });
};

test('a curve fit is told as a comparison of the curves, the coefficients of each, and the ones that could not be fitted', () => {
  const tables = resultTables(curveFit());
  assert.deepEqual(
    tables.map((x) => x.title),
    ['Curves fitted to Score against Age', 'Coefficients: Linear', 'Coefficients: Power'],
  );
  assert.deepEqual(tables[0].head, ['Curve', 'Fitted on', 'R²', 'Adjusted R²', 'F', 'df', 'p', 'R² on the scale of Score', 'Root mean squared error', 'Equation']);
  assert.deepEqual(tables[0].rows[0], ['Linear', 'Score', '.80', '.78', '32.00', '1, 8', '< .001', '.80', '0.90', 'Score = 1.500 + 0.500 × Age']);
  assert.deepEqual(tables[0].rows[1].slice(0, 2), ['Power', 'ln(Score)']);
  assert.equal(tables[0].rows[1][9], 'Score = 1.490 × Age^0.600', 'the power curve is written in its own form');
  assert.match(tables[0].note ?? '', /Not fitted: Logarithmic \(the predictor is not above zero in every case\)\./);
  assert.deepEqual(tables[2].head, ['Term', 'B', 'SE', 't', 'p', '95% CI of B']);
  assert.equal(tables[2].rows[1][0], 'ln(Age)', 'a power curve is a line in the logarithm of the predictor');
  assert.match(tables[2].note ?? '', /on the scale of ln\(Score\)/);
  assert.equal(resultTitle(curveFit()), 'Curve fit: Score against Age');
});

test('the curve fit sentence names the curve that came closest on the scale of the outcome, and the line beside it', () => {
  const s = resultSentence(curveFit());
  assert.equal(s, 'Of the 2 curves fitted to Score against Age, the power curve came closest to the cases: R² = .85 on the scale of Score (root mean squared error 0.70), F(1, 8) = 72.00, p < .001. The straight line gave R² = .80.');
  assert.match(resultReading(curveFit()), /prefer the simpler curve unless the extra bend is clear/);
  assert.doesNotMatch(resultReading(curveFit()), /marginal/);
});

test('two numbers are offered a curve fit, with the curves to tick', () => {
  assert.deepEqual(methodsFor('curve', [variable(), variable()], 0).map((m) => m.procedure), ['curve_fit']);
  assert.deepEqual(methodsFor('curve', [variable(), undefined], 0), [], 'both numbers are needed');
  const roles = goals().find((g) => g.id === 'curve')!.roles;
  assert.deepEqual(roles.map((x) => x.id), ['outcome', 'predictor']);
  assert.equal(roles[1].accepts(variable()), true);
  assert.equal(roles[1].accepts(categorical(3)), false);
  assert.equal(canBootstrap('curve_fit', []), false, 'a bootstrap of a curve fit is not offered');
});

// ---------------------------------------------------------------- the bootstrap of a whole analysis

const interval = (estimate: number, se: number, lo: number, hi: number) => ({ estimate, bias: 0.1, se, used: 1000, failures: 0, level: 0.95, percentile: [lo, hi] as [number, number], bca: null });

const wholeBootstrap = (patch: Record<string, unknown> = {}): StatResult =>
  result({
    procedure: 'independent_t',
    variables: [vref('Score'), vref('Site')],
    details: {
      t_test: { kind: 'welch' },
      bootstrap_analysis: {
        seed: 7,
        resamples: 1000,
        level: 0.95,
        failed: 0,
        cluster: null,
        items: [
          { path: 't_test/estimate', interval: interval(-5, 1.2, -7.4, -2.6), below_05: null },
          { path: 't_test/t', interval: interval(-4.1, 1.1, -6.3, -2.2), below_05: null },
          { path: 't_test/p', interval: interval(0.002, 0.03, 0.0001, 0.07), below_05: 0.93 },
          { path: 't_test/n', interval: interval(12, 0, 12, 12), below_05: null },
          { path: 't_test/d', interval: interval(-2.1, 0.7, -3.6, -0.9), below_05: null },
        ],
        ...patch,
      },
    } as unknown as StatResult['details'],
  });

test('a bootstrap of the whole analysis is a table of the numbers the page leads with, each with its spread, and how often p fell below .05', () => {
  const [table] = analysisTables(wholeBootstrap());
  assert.equal(table.title, 'Bootstrap of the whole analysis');
  assert.deepEqual(table.head, ['Statistic', 'Estimate', 'Bootstrap SE', '95% percentile CI', 'Resamples with p < .05']);
  assert.deepEqual(table.rows[0], ['Difference of means', '-5.00', '1.200', '[-7.40, -2.60]', '\u2014']);
  assert.deepEqual(table.rows[2], ['p', '.002', '.030', '[< .001, .070]', '93.0%']);
  assert.deepEqual(table.rows.map((x) => x[0]), ['Difference of means', 't', 'p', "Cohen's d"], 'a number the page does not lead with (the count of cases) has no row, and one the engine did not send is not invented');
  assert.match(table.note ?? '', /^1,000 resamples of the cases, drawn with replacement, from seed 7\. Each resample runs the whole analysis again/);
  assert.doesNotMatch(table.note ?? '', /could not be analyzed/);
  assert.match(analysisTables(wholeBootstrap({ failed: 3 }))[0].note ?? '', /3 resamples could not be analyzed/);
  const clustered = analysisTables(wholeBootstrap({ cluster: { key: 'attr:Rating', name: 'Rating', groups: 5 } }))[0];
  assert.match(clustered.note ?? '', /^1,000 resamples of the 5 groups of cases that share a value of Rating, drawn with replacement and kept together, from seed 7\./);
  assert.deepEqual(analysisTables(result({ procedure: 'independent_t', details: { t_test: { kind: 'welch' } } as unknown as StatResult['details'] })), [], 'no bootstrap, no table');
});

test('each procedure leads with its own numbers, and the numbers of a model are named by the predictor', () => {
  const kinds = ['one_sample_t', 'independent_t', 'paired_t', 'one_way_anova', 'kruskal_wallis', 'mann_whitney', 'wilcoxon', 'chi_square', 'fisher', 'pearson', 'spearman', 'kendall', 'shapiro_wilk', 'ks_normal', 'ks_two', 'runs', 'binomial', 'goodness_of_fit', 'sign', 'friedman', 'median_test', 'moses', 'mcnemar', 'ordinal_association', 'partial_correlation'] as const;
  for (const k of kinds) {
    const rows = analysisRows(result({ procedure: k, details: { t_test: { kind: 'welch' } } as unknown as StatResult['details'] }));
    assert.ok(rows.length >= 2, `${k} leads with at least two numbers`);
    assert.ok(rows.every((x) => x.path.includes('/') && x.label.length > 0), `${k}: every row has a path and a label`);
    assert.ok(rows.some((x) => x.p), `${k} gives a p-value`);
  }
  const reg = regression();
  const regRows = analysisRows(reg);
  assert.ok(regRows.some((x) => x.path === 'models/0/terms/1/estimate' && x.label === 'B for Age'));
  assert.ok(regRows.some((x) => x.path === 'models/0/terms/2/p' && x.p), 'a p for each predictor');
  const log = analysisRows(logistic());
  assert.ok(log.some((x) => x.path === 'models/0/auc') && log.some((x) => x.path === 'models/0/terms/1/odds_ratio' && x.label === 'Odds ratio for Age'));
  const curves = analysisRows(curveFit());
  assert.deepEqual(curves.map((x) => x.path), ['fits/0/r_squared_original', 'fits/1/r_squared_original'], 'a curve that was not fitted has no row');
  assert.equal(curves[1].label, 'R\u00b2 on the scale of Score: Power');
});

// ---------------------------------------------------------------- running an analysis again

const tTest = (t: number, p: number, estimate: number, nUsed: number): StatResult =>
  result({ procedure: 'independent_t', variables: [vref('Score'), vref('Site')], n_used: nUsed, details: { t_test: { kind: 'welch', t, p, estimate, d: 1.1, g: 1.0 } } as unknown as StatResult['details'] });

test('a number is found in a result by its path, and a path that leads nowhere is not a number', () => {
  const r = regression();
  assert.equal(numberAt(r.details, 'models/0/terms/1/estimate'), 0.31);
  assert.equal(numberAt(r.details, 'models/0/r_squared'), 0.42);
  assert.equal(numberAt(r.details, 'models/0/nothing'), null);
  assert.equal(numberAt(r.details, 'models/9/r_squared'), null);
  assert.equal(numberAt(r.details, 'level/deeper'), null, 'a number has nothing below it');
});

test('two runs of the same analysis are told apart by the numbers the page leads with and by the cases used', () => {
  const a = tTest(-4.1, 0.002, -5, 12);
  assert.equal(resultsDiffer(a, tTest(-4.1, 0.002, -5, 12)), false);
  assert.equal(resultsDiffer(a, tTest(-3.0, 0.002, -5, 12)), true);
  assert.equal(resultsDiffer(a, tTest(-4.1, 0.002, -5, 14)), true, 'two more cases, the same numbers: the run still differs');
});

test('the comparison puts the earlier run beside this one, with the change', () => {
  const table = compareTable(tTest(-4.1, 0.002, -5, 12), tTest(-2.2, 0.06, -3, 14));
  assert.deepEqual(table.head, ['Statistic', 'Earlier run', 'This run', 'Change']);
  assert.deepEqual(table.rows[0], ['Cases used', '12', '14', '+2']);
  const byLabel = new Map(table.rows.map((row) => [row[0], row]));
  assert.deepEqual(byLabel.get('Difference of means'), ['Difference of means', '-5.00', '-3.00', '+2.00']);
  assert.deepEqual(byLabel.get('t'), ['t', '-4.10', '-2.20', '+1.90']);
  assert.deepEqual(byLabel.get('p'), ['p', '.002', '.060', '+.058']);
  assert.ok(!byLabel.has('Cohen\'s d') || byLabel.get("Cohen's d")![3] === '0.00', 'a number that did not move shows no change');
  const fewer = compareTable(tTest(-4.1, 0.002, -5, 12), tTest(-4.1, 0.002, -5, 10));
  assert.equal(fewer.rows[0][3], '-2', 'a loss of cases is a negative change');
});

// ---------------------------------------------------------------- two-way analysis of variance and Cronbach's alpha

const factorial = (p: [number, number, number]) =>
  result({
    procedure: 'two_way_anova',
    variables: [vref('Score'), vref('Site'), vref('Group')],
    n_used: 24,
    details: {
      labels_a: ['North', 'South'],
      labels_b: ['Control', 'Trained'],
      levels_a: ['N', 'S'],
      levels_b: ['c', 't'],
      factorial: {
        n: 24,
        levels_a: 2,
        levels_b: 2,
        effects: [
          { name: 'a', ss: 40.5, df: 1, ms: 40.5, f: 6.75, p: p[0], partial_eta_squared: 0.25 },
          { name: 'b', ss: 6, df: 1, ms: 6, f: 1, p: p[1], partial_eta_squared: 0.048 },
          { name: 'ab', ss: 13.5, df: 1, ms: 13.5, f: 2.25, p: p[2], partial_eta_squared: 0.1 },
        ],
        error_ss: 120,
        error_df: 20,
        error_ms: 6,
        total_ss: 180,
        total_df: 23,
        r_squared: 0.333,
        adj_r_squared: 0.233,
        grand_mean: 10,
        balanced: true,
        cells: [
          { a: 0, b: 0, n: 6, mean: 9, sd: 2.1 },
          { a: 0, b: 1, n: 6, mean: 10, sd: 2.4 },
          { a: 1, b: 0, n: 6, mean: 11, sd: 2.2 },
          { a: 1, b: 1, n: 6, mean: 13, sd: null },
        ],
        margins_a: [{ level: 0, n: 12, mean: 9.5 }, { level: 1, n: 12, mean: 12 }],
        margins_b: [{ level: 0, n: 12, mean: 10 }, { level: 1, n: 12, mean: 11.5 }],
        levene: null,
      },
    },
  });

test('a two-way analysis of variance is titled, tabled, worded, and read', () => {
  assert.equal(resultTitle(factorial([0.02, 0.33, 0.15])), 'Two-way analysis of variance: Score by Site and Group');
  const [anova, cells, margins] = resultTables(factorial([0.02, 0.33, 0.15]));
  assert.deepEqual(anova.head, ['Source', 'SS', 'df', 'MS', 'F', 'p', 'ηp²']);
  assert.deepEqual(anova.rows.map((r) => r[0]), ['Site', 'Group', 'Site × Group', 'Error', 'Total']);
  assert.deepEqual(anova.rows[0], ['Site', '40.50', '1', '40.50', '6.75', '.020', '.250']);
  assert.deepEqual(anova.rows[3].slice(0, 4), ['Error', '120.00', '20', '6.00'], 'the error row has no test');
  assert.match(anova.note ?? '', /Type III sums of squares/);
  assert.doesNotMatch(anova.note ?? '', /different numbers of cases/, 'a balanced design says nothing of unequal cells');
  assert.deepEqual(cells.rows[0], ['North', 'Control', '6', '9.00', '2.10']);
  assert.equal(cells.rows[3][4], '—', 'a cell of one case has no spread');
  assert.deepEqual(margins.rows.map((r) => r.slice(0, 3)), [['Site', 'North', '12'], ['Site', 'South', '12'], ['Group', 'Control', '12'], ['Group', 'Trained', '12']]);
  const unbalanced = factorial([0.02, 0.33, 0.15]);
  unbalanced.details.factorial!.balanced = false;
  assert.match(resultTables(unbalanced)[0].note ?? '', /different numbers of cases/);

  const said = resultSentence(factorial([0.02, 0.33, 0.15]));
  assert.match(said, /The main effect of Site was statistically significant, F\(1, 20\) = 6\.75, p = \.020, ηp² = \.250; /);
  assert.match(said, /The main effect of Group was not statistically significant, F\(1, 20\) = 1\.00, p = \.330, ηp² = \.048; /);
  assert.match(said, /The interaction of Site and Group was not statistically significant, F\(1, 20\) = 2\.25, p = \.150, ηp² = \.100\.$/);
  const reading = resultReading(factorial([0.02, 0.33, 0.15]));
  assert.match(reading, /\(ηp²\) is large for Site, small for Group, and medium for the two together/);
  assert.doesNotMatch(reading, /interaction is statistically significant/);
  assert.match(resultReading(factorial([0.02, 0.33, 0.01])), /The interaction is statistically significant: the difference one variable makes depends on the other/);
  assert.match(resultReading(factorial([0.4, 0.5, 0.6])), /do not describe it as marginal/, 'when nothing is significant the page says not to call it marginal');
  assert.match(resultReading(factorial([0.4, 0.5, 0.6])), /not which ones/);
});

const reliability = (patch: Partial<NonNullable<StatResult['details']['alpha']>> = {}): StatResult =>
  result({
    procedure: 'cronbach_alpha',
    variables: [vref('Q1'), vref('Q2'), vref('Q3')],
    n_used: 40,
    details: {
      alpha: {
        n: 40,
        k: 3,
        alpha: 0.7567,
        standardized_alpha: 0.76,
        mean_inter_item_r: 0.51,
        total_mean: 9.4,
        total_sd: 2.7,
        items: [
          { mean: 3.1, sd: 1.1, item_rest_r: 0.62, alpha_if_deleted: 0.61 },
          { mean: 3.2, sd: 1, item_rest_r: 0.55, alpha_if_deleted: 0.68 },
          { mean: 3.1, sd: 1.2, item_rest_r: 0.2, alpha_if_deleted: 0.81 },
        ],
        interval: { level: 0.95, lower: 0.6, upper: 0.86 },
        negative_pairs: 0,
        ...patch,
      },
    },
  });

test("Cronbach's alpha is titled, tabled, worded, and read, with what to do about an item that lowers it", () => {
  assert.equal(resultTitle(reliability()), "Cronbach's alpha: Q1, Q2, Q3");
  const [main, items] = resultTables(reliability());
  assert.deepEqual(main.head, ['α', 'Standardized α', 'Mean correlation between items', 'Items', 'Cases', '95% CI']);
  assert.deepEqual(main.rows[0], ['.757', '.760', '.510', '3', '40', '[.600, .860]']);
  assert.deepEqual(items.rows[2], ['Q3', '3.10', '1.20', '.200', '.810']);
  assert.match(resultSentence(reliability()), /^Cronbach's alpha for the 3 items was \.76, 95% CI \[\.60, \.86\], N = 40\.$/);
  const reading = resultReading(reliability());
  assert.match(reading, /rule of thumb calls \.70 acceptable/);
  assert.match(reading, /Leaving out Q3 would raise alpha to \.81\./);
  assert.doesNotMatch(reading, /Leaving out Q1/, 'an item whose leaving out lowers alpha is not named');
  assert.match(resultReading(reliability({ negative_pairs: 2 })), /2 pairs of items correlate negatively.*reverse it/);
  assert.match(resultReading(reliability({ negative_pairs: 1 })), /1 pair of items correlates negatively/);
  const two = reliability({ interval: null, standardized_alpha: null, mean_inter_item_r: null });
  assert.equal(resultTables(two)[0].head.length, 5, 'with no interval there is no column for it');
  assert.equal(resultTables(two)[0].rows[0][1], '—');
});

test('the bootstrap of a whole analysis leads with the numbers of these two procedures', () => {
  const rows = analysisRows(factorial([0.02, 0.33, 0.15]));
  assert.deepEqual(rows.map((r) => r.path), ['factorial/effects/0/f', 'factorial/effects/0/p', 'factorial/effects/0/partial_eta_squared', 'factorial/effects/1/f', 'factorial/effects/1/p', 'factorial/effects/1/partial_eta_squared', 'factorial/effects/2/f', 'factorial/effects/2/p', 'factorial/effects/2/partial_eta_squared']);
  assert.equal(rows[0].label, 'F for Site');
  assert.equal(rows[6].label, 'F for Site × Group');
  assert.ok(rows.filter((r) => r.p).length === 3);
  assert.deepEqual(analysisRows(reliability()).map((r) => r.path), ['alpha/alpha', 'alpha/standardized_alpha', 'alpha/mean_inter_item_r']);
});

test('while the page describes, it offers describing one variable and no test; with the tests on, it offers every goal', () => {
  const describing = goalsInScope('descriptive');
  assert.deepEqual(describing.map((g) => g.id), ['describe']);
  assert.deepEqual(goalsInScope('tests').map((g) => g.id), goals().map((g) => g.id));
  assert.ok(goalsInScope('tests').length > 10, 'every goal there is');

  // For a number, the page would offer the tests of normality, runs, and the rest beside describing; describing offers only describing.
  const number = { key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale', valid: 20, missing: 0, not_a_number: 0, numeric: true, distinct: 15, categories: [] } as unknown as StatVariable;
  const all = methodsFor('describe', [number], 0);
  assert.ok(all.length > 1, 'a number has tests beside describing');
  assert.deepEqual(methodsInScope('descriptive', all).map((m) => m.procedure), ['describe']);
  assert.deepEqual(methodsInScope('tests', all).map((m) => m.procedure), all.map((m) => m.procedure));
  // With no variable chosen there is nothing to offer in either scope.
  assert.deepEqual(methodsInScope('descriptive', methodsFor('describe', [undefined], 0)), []);
});
