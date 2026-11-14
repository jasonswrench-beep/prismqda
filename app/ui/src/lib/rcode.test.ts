import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import type { StatChart, StatOptions, StatProcedure, StatResult, StatTable, StatVariable } from '../types';
import { HELPERS, helpersFor, pageCode, q, rn, type RPage, type RVar } from './rcode.code';
import { dataCsv, pageSpec, scriptText, tableKeys } from './rScript';

const rscript = [process.env.PRISMQDA_RSCRIPT, 'C:\\Program Files\\R\\R-4.5.3\\bin\\Rscript.exe'].find((f) => f && fs.existsSync(f));

const v = (label: string, numeric: boolean, categories: string[] = []): RVar => ({ key: `attr:${label}`, label, numeric, categories });
const page = (procedure: StatProcedure, vars: RVar[], extra: Partial<RPage> = {}): RPage => ({ procedure, vars, options: {}, groups: [], ...extra });

test('a string is written as R reads it, and what is not plain ASCII is escaped so the file reads the same anywhere', () => {
  assert.equal(q('Stat score'), '"Stat score"');
  assert.equal(q('say "hi" \\ now'), '"say \\"hi\\" \\\\ now"');
  assert.equal(q('a\nb\tc'), '"a\\nb\\tc"');
  assert.equal(q('caf\u00e9'), '"caf\\u00e9"');
  assert.equal(q('\u{1F600}'), '"\\U{1f600}"');
  assert.equal(rn(1.5), '1.5');
  assert.equal(rn(Number.NaN), 'NA');
});

test('a script defines only the helpers its pages call, and the ones those need', () => {
  const none = helpersFor('print(t.test(a, b))');
  assert.equal(none, '');
  const some = helpersFor('x <- num(d[["A"]]); print(levene(y, g)); print(d_ci(1, 1, 1))');
  assert.ok(some.includes('num <- function') && some.includes('levene <- function') && some.includes('d_ci <- function'));
  assert.ok(!some.includes('moses_test') && !some.includes('ordinal_assoc'));
  assert.ok(!helpersFor('as.numeric(x); rownum(1); eval(y)').includes('num <- function'), 'a name inside another is not a call');
  for (const name of Object.keys(HELPERS)) for (const need of HELPERS[name].needs ?? []) assert.ok(HELPERS[need], `${name} needs ${need}, which exists`);
});

test('a comparison of groups takes the cases that have both values, in the groups compared, and names the test it ran', () => {
  const code = pageCode(page('independent_t', [v('Score', true), v('Group', false, ['A', 'B'])], { groups: ['A', 'B'], options: { equal_variances: true, level: 0.9 } }));
  assert.ok(code.startsWith('d <- data\n'));
  assert.ok(code.includes('g %in% c("A", "B")') && code.includes('factor(g[ok], levels = c("A", "B"))'));
  assert.ok(code.includes('t.test(a, b, var.equal = TRUE, conf.level = 0.9)'));
  const welch = pageCode(page('independent_t', [v('Score', true), v('Group', false)], { groups: ['A', 'B'] }));
  assert.ok(welch.includes('var.equal = FALSE'));
});

test('a rule that picks the cases is the same rule in the script', () => {
  const code = pageCode(page('describe', [v('Age', true)], { options: { filter: '`Age` >= 30 & site == "N"' } }));
  assert.ok(code.startsWith('d <- subset(data, `Age` >= 30 & site == "N")\n'), code);
});

test('the pairs after an analysis of variance are compared by the test that was chosen', () => {
  const base = { groups: ['A', 'B', 'C'] };
  const vars = [v('Score', true), v('Group', false)];
  assert.ok(pageCode(page('one_way_anova', vars, { ...base, options: { posthoc: 'games_howell' } })).includes('posthoc_pairs(y, g, "games_howell", 0.95)'));
  assert.ok(pageCode(page('one_way_anova', vars, { ...base, options: { adjustment: 'benjamini_hochberg' } })).includes('posthoc_pairs(y, g, "BH", 0.95)'));
  assert.ok(pageCode(page('one_way_anova', vars, { ...base })).includes('"holm"'), 'Holm is the default');
  assert.ok(!pageCode(page('one_way_anova', vars, { groups: ['A', 'B'] })).includes('posthoc_pairs'), 'two groups have one pair, and the page makes no table of pairs');
});

test('a regression enters the predictors in steps and compares a category with the reference chosen', () => {
  const code = pageCode(
    page('linear_regression', [v('Score', true), v('Age', true), { ...v('Group', false, ['A', 'B', 'C']), key: 'attr:Group' }], { options: { steps: [1, 2], references: { 'attr:Group': 'B' } } }),
  );
  assert.ok(code.includes('fit1 <- lm(y ~ `Age`, data = m)') && code.includes('fit2 <- lm(y ~ `Age` + `Group`, data = m)'), code);
  assert.ok(code.includes('ref <- "B"') && code.includes('levels = c(ref, setdiff(lv, ref))'));
  assert.ok(code.includes('anova(fit1, fit2)') && code.includes('r_squared_change'));
});

test('the data is written as CSV with a column for each variable, and says which columns are numbers', () => {
  const table: StatTable = {
    cases: [{ id: 1, name: 'A' }, { id: 2, name: 'B' }, { id: 3, name: 'C' }],
    columns: [
      { key: 'attr:Score', name: 'Score', kind: 'attribute', measure: 'scale', numeric: true, values: ['10', '12.5', null] },
      { key: 'attr:Note', name: 'Note', kind: 'attribute', measure: 'nominal', numeric: false, values: ['a, b', 'say "hi"', ' x'] },
    ],
  };
  const { csv, classes } = dataCsv(table);
  assert.equal(csv, 'Score,Note\n10,"a, b"\n12.5,"say ""hi"""\n," x"\n', 'a missing value is an empty field, and a field with a comma, a quote, or a space at an end is quoted');
  assert.deepEqual(classes, { Score: 'numeric', Note: 'character' });
});

test('the data holds every variable of cases, and the codes the pages used', () => {
  const variable = (key: string, kind: StatVariable['kind']): StatVariable => ({ key, name: key, kind, code_id: null, measure: 'scale', valid: 1, missing: 0, not_a_number: 0, numeric: true, distinct: 1, categories: [] });
  const variables = [variable('attr:A', 'attribute'), variable('attr:B', 'attribute'), variable('code:1:present', 'code_present'), variable('code:2:count', 'code_count')];
  const entry = (keys: string[]) => ({ kind: 'analysis' as const, id: 1, ranAt: '', note: '', section: '', earlierRuns: 0, request: { procedure: 'describe' as const, variables: keys }, result: {} as StatResult });
  assert.deepEqual(tableKeys([entry(['attr:A'])], variables), ['attr:A', 'attr:B'], 'a rule may name any variable of cases');
  assert.deepEqual(tableKeys([entry(['code:2:count'])], variables), ['attr:A', 'attr:B', 'code:2:count']);
  assert.deepEqual(tableKeys([entry(['code:9:count'])], variables), ['attr:A', 'attr:B'], 'a code that is no longer there is left out');
});

test('a page records what the engine settled: its groups and the order of the values', () => {
  const variables = new Map<string, StatVariable>([
    ['attr:Score', { key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale', valid: 6, missing: 0, not_a_number: 0, numeric: true, distinct: 6, categories: [] }],
    ['attr:Group', { key: 'attr:Group', name: 'Group', kind: 'attribute', code_id: null, measure: 'nominal', valid: 6, missing: 0, not_a_number: 0, numeric: false, distinct: 2, categories: [{ value: 'A', label: 'Alpha', count: 3 }, { value: 'B', label: 'Beta', count: 3 }] }],
  ]);
  const result = {
    procedure: 'independent_t',
    variables: [{ key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale' }, { key: 'attr:Group', name: 'Group', kind: 'attribute', code_id: null, measure: 'nominal' }],
    groups: [{ value: 'A' }, { value: 'B' }],
    charts: [],
    details: {},
  } as unknown as StatResult;
  const spec = pageSpec({ kind: 'analysis', id: 1, ranAt: '', note: '', section: '', earlierRuns: 0, request: { procedure: 'independent_t', variables: ['attr:Score', 'attr:Group'] }, result }, variables);
  assert.deepEqual(spec.groups, ['A', 'B']);
  assert.deepEqual(spec.vars.map((x) => [x.label, x.numeric, x.categories]), [['Score', true, []], ['Group', false, ['A', 'B']]]);
});

test('a script has its data read in, then the helpers, then each page in order', () => {
  const variables: StatVariable[] = [{ key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale', valid: 6, missing: 0, not_a_number: 0, numeric: true, distinct: 6, categories: [] }];
  const result = { procedure: 'shapiro_wilk', variables: [{ key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale' }], groups: [], assumptions: [], charts: [], details: {} } as unknown as StatResult;
  const entry = { kind: 'analysis' as const, id: 1, ranAt: '', note: '', section: '', earlierRuns: 0, request: { procedure: 'shapiro_wilk' as const, variables: ['attr:Score'] }, result };
  const script = scriptText([entry, { ...entry, id: 2 }], variables, { Score: 'numeric' }, 'Check');
  assert.ok(script.startsWith('# Check\n'));
  assert.ok(script.includes('data <- read.csv("{{data-file}}", check.names = FALSE, na.strings = "", fileEncoding = "UTF-8", colClasses = c("Score" = "numeric"))'));
  assert.ok(script.indexOf('num <- function') < script.indexOf('# ---- Page 1'), 'helpers come before the pages');
  assert.ok(script.indexOf('# ---- Page 1') < script.indexOf('# ---- Page 2'));
  assert.equal((script.match(/^num <- function/gm) ?? []).length, 1, 'a helper is defined once');
});

/** The page, with a bootstrap of its statistic (or, with `analysis` or `cluster`, of the whole analysis). */
const withBoot = (p: RPage, extra: NonNullable<StatOptions['bootstrap']> = {}): RPage => ({ ...p, options: { ...p.options, bootstrap: { seed: 7, resamples: 100, ...extra } } });

const numV = (label: string) => v(label, true);
const catV = (label: string, categories: string[] = ['A', 'B', 'C']) => v(label, false, categories);

/** The pages whose statistic has a bootstrap of its own, one for each procedure that has one. */
const statBootPages = (): RPage[] => [
  withBoot(page('describe', [numV('X')], { kind: 'descriptives' })),
  withBoot(page('one_sample_t', [numV('X')], { options: { mu: 3 } })),
  withBoot(page('paired_t', [numV('X'), numV('Y')])),
  withBoot(page('wilcoxon', [numV('X'), numV('Y')])),
  withBoot(page('wilcoxon', [numV('X')], { options: { mu: 2 } })),
  withBoot(page('independent_t', [numV('X'), catV('G')], { groups: ['A', 'B'] })),
  withBoot(page('mann_whitney', [numV('X'), catV('G')], { groups: ['A', 'B'] })),
  withBoot(page('pearson', [numV('X'), numV('Y')])),
  withBoot(page('spearman', [numV('X'), numV('Y')])),
  withBoot(page('kendall', [numV('X'), numV('Y')])),
  withBoot(page('linear_regression', [numV('Y'), numV('X'), { ...catV('G'), key: 'attr:G' }], { options: { steps: [1, 2], references: { 'attr:G': 'B' } } })),
  withBoot(page('logistic_regression', [catV('Y', ['no', 'yes']), numV('X'), { ...catV('G'), key: 'attr:G' }], { event: 'yes', options: { steps: [1, 2], references: { 'attr:G': 'B' } } })),
];

/** What the bootstrap of a whole analysis is given for each procedure: the variables, and the numbers (by their paths) the page leads with. */
const wholeBootCases: [StatProcedure, RVar[], Partial<RPage>, string[]][] = [
  ['describe', [numV('X')], { kind: 'descriptives' }, ['descriptives/mean', 'descriptives/sd', 'descriptives/skewness']],
  ['one_sample_t', [numV('X')], { options: { mu: 3 } }, ['t_test/estimate', 't_test/p', 't_test/g']],
  ['paired_t', [numV('X'), numV('Y')], {}, ['t_test/estimate', 't_test/p']],
  ['independent_t', [numV('X'), catV('G')], { groups: ['A', 'B'] }, ['t_test/estimate', 't_test/t', 't_test/p', 't_test/d']],
  ['one_way_anova', [numV('X'), catV('G')], { groups: ['A', 'B', 'C'] }, ['anova/test/f', 'anova/test/p', 'anova/eta_squared', 'anova/omega_squared']],
  ['kruskal_wallis', [numV('X'), catV('G')], { groups: ['A', 'B', 'C'] }, ['kruskal_wallis/h', 'kruskal_wallis/p']],
  ['mann_whitney', [numV('X'), catV('G')], { groups: ['A', 'B'] }, ['mann_whitney/u', 'mann_whitney/z', 'mann_whitney/p']],
  ['wilcoxon', [numV('X'), numV('Y')], {}, ['wilcoxon/v', 'wilcoxon/z', 'wilcoxon/p']],
  ['chi_square', [catV('G'), catV('H')], { rowValues: ['A', 'B'], colValues: ['A', 'B'] }, ['chi_square/chi2', 'chi_square/p', 'chi_square/cramers_v']],
  ['fisher', [catV('G'), catV('H')], { rowValues: ['A', 'B'], colValues: ['A', 'B'] }, ['fisher/odds_ratio', 'fisher/p']],
  ['pearson', [numV('X'), numV('Y')], {}, ['correlation/r', 'correlation/p']],
  ['spearman', [numV('X'), numV('Y')], {}, ['correlation/r', 'correlation/p']],
  ['kendall', [numV('X'), numV('Y')], {}, ['correlation/r', 'correlation/p']],
  ['shapiro_wilk', [numV('X')], {}, ['shapiro_wilk/w', 'shapiro_wilk/p']],
  ['ks_normal', [numV('X')], { estimated: true }, ['ks/d', 'ks/p']],
  ['ks_two', [numV('X'), catV('G')], { groups: ['A', 'B'] }, ['ks/d', 'ks/p']],
  ['runs', [numV('X')], { options: { cut: 'mean' } }, ['runs/runs', 'runs/z', 'runs/p_exact']],
  ['binomial', [catV('G')], { counts: ['A', 'B', 'C'], options: { success: 'B', proportion: 0.4 } }, ['binomial/estimate', 'binomial/p']],
  ['goodness_of_fit', [catV('G')], { counts: ['A', 'B', 'C'], options: { expected: { A: 50, B: 30, C: 20 } } }, ['goodness_of_fit/chi2', 'goodness_of_fit/p']],
  ['sign', [numV('X'), numV('Y')], {}, ['sign/proportion_positive', 'sign/p']],
  ['friedman', [numV('X'), numV('Y'), numV('Z')], {}, ['friedman/chi2', 'friedman/p', 'friedman/kendall_w']],
  ['median_test', [numV('X'), catV('G')], { groups: ['A', 'B', 'C'] }, ['median_test/chi2', 'median_test/p']],
  ['moses', [numV('X'), catV('G')], { groups: ['A', 'B'], options: { control: 'B', trim: 2 } }, ['moses/p', 'moses/trimmed_p']],
  ['mcnemar', [catV('G', ['no', 'yes']), catV('H', ['no', 'yes'])], { values: ['no', 'yes'] }, ['mcnemar/chi2_corrected', 'mcnemar/p_corrected']],
  ['ordinal_association', [catV('G', ['1', '2', '3']), catV('H', ['1', '2', '3'])], { rowValues: ['1', '2', '3'], colValues: ['1', '2', '3'] }, ['ordinal/gamma/estimate', 'ordinal/tau_b/estimate', 'ordinal/tau_b/p']],
  ['partial_correlation', [numV('X'), numV('Y'), numV('Z'), numV('W')], {}, ['partial/r', 'partial/p']],
  ['two_way_anova', [numV('Y'), catV('G', ['A', 'B']), catV('H', ['x', 'y', 'z'])], { levelsA: ['A', 'B'], levelsB: ['x', 'y', 'z'] }, ['factorial/effects/0/f', 'factorial/effects/0/p', 'factorial/effects/2/partial_eta_squared']],
  ['cronbach_alpha', [numV('X'), numV('Y'), numV('Z')], {}, ['alpha/alpha', 'alpha/standardized_alpha', 'alpha/mean_inter_item_r']],
  [
    'linear_regression',
    [numV('Y'), numV('X'), { ...catV('G'), key: 'attr:G' }],
    { options: { steps: [1, 2], references: { 'attr:G': 'B' } } },
    ['models/0/r_squared', 'models/0/adj_r_squared', 'models/0/f/f', 'models/0/f/p', 'models/0/terms/1/estimate', 'models/0/terms/1/p'],
  ],
  [
    'logistic_regression',
    [catV('Y', ['no', 'yes']), numV('X'), { ...catV('G'), key: 'attr:G' }],
    { event: 'yes', options: { steps: [1, 2], references: { 'attr:G': 'B' } } },
    ['models/0/lr/chi2', 'models/0/lr/p', 'models/0/nagelkerke', 'models/0/auc', 'models/0/terms/1/estimate', 'models/0/terms/1/odds_ratio', 'models/0/terms/1/p'],
  ],
  ['curve_fit', [numV('Y'), numV('X')], { curveKinds: ['linear', 'power'] }, ['fits/0/r_squared', 'fits/1/r_squared']],
];

const wholeBootPages = (cluster?: string): RPage[] =>
  wholeBootCases.map(([procedure, vars, extra, paths]) => {
    const bootRows = paths.map((path) => ({ path, label: path, p: /\/p(_\w+)?$/.test(path), key: /^fits\/0\//.test(path) ? 'linear' : /^fits\/1\//.test(path) ? 'power' : undefined }));
    const withRows = page(procedure, vars, { ...extra, bootRows, clusterLabel: cluster });
    return withBoot(withRows, cluster ? { cluster: `attr:${cluster}` } : { analysis: true });
  });

const chart = (c: unknown): StatChart => c as StatChart;
const at = (index: number) => ({ type: 'variable' as const, index });

/** The pages that draw charts, one of each kind of chart. */
const chartPages = (): RPage[] => [
  page('describe', [numV('X')], {
    kind: 'descriptives',
    charts: [
      { chart: chart({ kind: 'histogram', subject: at(0), n: 10, bins: [], mean: 1, sd: 1, reference: 3 }), title: 'Histogram of X', names: ['X', 'Cases'] },
      { chart: chart({ kind: 'qq', subject: at(0), points: [], line: { intercept: 0, slope: 1 } }), title: 'Normal probability plot of X', names: ['Expected if normal', 'Value'] },
    ],
  }),
  page('describe', [catV('G')], { kind: 'frequencies', counts: ['A', 'B', 'C'], charts: [{ chart: chart({ kind: 'bars', subject: at(0), valid: 9, categories: [{ value: 'A', label: 'Alpha', count: 3 }, { value: 'B', label: 'Beta', count: 6 }] }), title: 'Bar chart of G', names: ['G', 'Cases'] }] }),
  page('one_way_anova', [numV('X'), catV('G')], {
    groups: ['A', 'B', 'C'],
    charts: [
      { chart: chart({ kind: 'box', subject: at(0), by: 1, groups: [] }), title: 'Box plot of X by G', names: ['G', 'X'] },
      { chart: chart({ kind: 'means', subject: at(0), by: 1, level: 0.95, groups: [] }), title: 'Means of X by G', names: ['G', 'X'] },
    ],
  }),
  page('chi_square', [catV('G'), catV('H')], { rowValues: ['A', 'B'], colValues: ['A', 'B'], charts: [{ chart: chart({ kind: 'stacked', row: 0, col: 1, row_values: [], row_labels: [], col_values: [], col_labels: [], counts: [] }), title: 'Stacked bars', names: ['G', 'H'] }] }),
  page('pearson', [numV('X'), numV('Y')], { charts: [{ chart: chart({ kind: 'scatter', x: at(0), y: at(1), points: [], omitted: 0, fit: { intercept: 0, slope: 1 }, zero_line: false }), title: 'Scatter plot', names: ['X', 'Y'] }] }),
  page('linear_regression', [numV('Y'), numV('X')], {
    charts: [
      { chart: chart({ kind: 'scatter', x: at(1), y: at(0), points: [], omitted: 0, fit: { intercept: 0, slope: 1 }, zero_line: false }), title: 'Y against X', names: ['X', 'Y'] },
      { chart: chart({ kind: 'scatter', x: { type: 'fitted' }, y: { type: 'residuals' }, points: [], omitted: 0, fit: null, zero_line: true }), title: 'Residuals against fitted', names: ['Fitted', 'Residuals'] },
      { chart: chart({ kind: 'qq', subject: { type: 'residuals' }, points: [], line: { intercept: 0, slope: 1 } }), title: 'Normal probability plot of the residuals', names: ['Expected if normal', 'Value'] },
    ],
  }),
  page('curve_fit', [numV('Y'), numV('X')], {
    curveKinds: ['linear', 'power'],
    charts: [{ chart: chart({ kind: 'curves', x: at(1), y: at(0), points: [], omitted: 0, curves: [{ kind: 'linear', points: [], at_points: [] }, { kind: 'power', points: [], at_points: [] }] }), title: 'Curves', names: ['X', 'Y'] }],
  }),
];

// Every kind of page, in the ways its options change the code, must be R that R can read. This asks R to parse them; it runs only
// where R is installed (the interface check sets PRISMQDA_RSCRIPT or finds it in its usual place).
test('the code of every kind of page is R that R can read', { skip: rscript ? false : 'R is not installed here' }, () => {
  const num = (l: string) => v(l, true);
  const cat = (l: string, c: string[] = ['A', 'B', 'C']) => v(l, false, c);
  const pages: RPage[] = [
    page('describe', [num('X')], { kind: 'descriptives' }),
    page('describe', [cat('G')], { kind: 'frequencies', counts: ['A', 'B', 'C'] }),
    page('one_sample_t', [num('X')], { options: { mu: 3, level: 0.9 } }),
    page('independent_t', [num('X'), cat('G')], { groups: ['A', 'B'], options: { equal_variances: true } }),
    page('paired_t', [num('X'), num('Y')]),
    page('one_way_anova', [num('X'), cat('G')], { groups: ['A', 'B', 'C'] }),
    page('one_way_anova', [num('X'), cat('G')], { groups: ['A', 'B', 'C'], options: { posthoc: 'tukey' } }),
    page('one_way_anova', [num('X'), cat('G')], { groups: ['A', 'B', 'C'], options: { posthoc: 'games_howell' } }),
    page('one_way_anova', [num('X'), cat('G')], { groups: ['A', 'B', 'C'], options: { posthoc: 'scheffe' } }),
    page('one_way_anova', [num('X'), cat('G')], { groups: ['A', 'B', 'C'], options: { adjustment: 'bonferroni' } }),
    page('chi_square', [cat('G'), cat('H')], { rowValues: ['A', 'B'], colValues: ['A', 'B'] }),
    page('fisher', [cat('G'), cat('H')], { rowValues: ['A', 'B'], colValues: ['A', 'B'] }),
    page('pearson', [num('X'), num('Y')]),
    page('spearman', [num('X'), num('Y')]),
    page('kendall', [num('X'), num('Y')]),
    page('mann_whitney', [num('X'), cat('G')], { groups: ['A', 'B'] }),
    page('wilcoxon', [num('X'), num('Y')]),
    page('wilcoxon', [num('X')], { options: { mu: 2 } }),
    page('kruskal_wallis', [num('X'), cat('G')], { groups: ['A', 'B', 'C'], options: { adjustment: 'holm' } }),
    page('linear_regression', [num('Y'), num('X'), { ...cat('G'), key: 'attr:G' }], { options: { steps: [1, 2], references: { 'attr:G': 'B' } } }),
    page('logistic_regression', [cat('Y', ['no', 'yes']), num('X'), { ...cat('G'), key: 'attr:G' }], { event: 'yes', profile: true, options: { steps: [1, 2], references: { 'attr:G': 'B' } } }),
    page('logistic_regression', [cat('Y', ['no', 'yes']), num('X')], { event: 'no', profile: false }),
    page('curve_fit', [num('Y'), num('X')], { curveKinds: ['linear', 'logarithmic', 'inverse', 'quadratic', 'cubic', 'power', 'exponential', 's_curve'] }),
    page('curve_fit', [num('Y'), num('X')], { curveKinds: ['linear', 'power'], options: { level: 0.9 } }),
    page('shapiro_wilk', [num('X')]),
    page('ks_normal', [num('X')], { estimated: true, mean: 1, sd: 2 }),
    page('ks_normal', [num('X')], { estimated: false, mean: 1, sd: 2 }),
    page('runs', [num('X')], { options: { cut: 'value', mu: 4 } }),
    page('runs', [cat('G', ['A', 'B'])]),
    page('binomial', [cat('G')], { options: { success: 'B', proportion: 0.4 }, counts: ['A', 'B', 'C'] }),
    page('goodness_of_fit', [cat('G')], { options: { expected: { A: 50, B: 30, C: 20 } }, counts: ['A', 'B', 'C'] }),
    page('goodness_of_fit', [cat('G')], { counts: ['A', 'B', 'C'] }),
    page('sign', [num('X'), num('Y')]),
    page('sign', [num('X')], { options: { mu: 1 } }),
    page('friedman', [num('X'), num('Y'), num('Z')]),
    page('ks_two', [num('X'), cat('G')], { groups: ['A', 'B'] }),
    page('median_test', [num('X'), cat('G')], { groups: ['A', 'B', 'C'] }),
    page('moses', [num('X'), cat('G')], { groups: ['A', 'B'], options: { control: 'B', trim: 2 } }),
    page('mcnemar', [cat('G', ['no', 'yes']), cat('H', ['no', 'yes'])], { values: ['no', 'yes'] }),
    page('ordinal_association', [cat('G', ['1', '2', '3']), cat('H', ['1', '2', '3'])], { rowValues: ['1', '2', '3'], colValues: ['1', '2', '3'] }),
    page('partial_correlation', [num('X'), num('Y'), num('Z'), num('W')]),
    page('two_way_anova', [num('X'), cat('G'), cat('H', ['no', 'yes'])], { levelsA: ['A', 'B', 'C'], levelsB: ['no', 'yes'] }),
    page('cronbach_alpha', [num('X'), num('Y'), num('Z')]),
    page('describe', [num('X')], { kind: 'descriptives', options: { filter: '`X` > 1 & `G` == "A"' } }),
  ];
  const chunks = [...pages, ...statBootPages(), ...wholeBootPages(), ...wholeBootPages('G'), ...chartPages()].map((p) => pageCode(p));
  assert.ok(chunks.some((c) => c.includes('pq_boot_whole(d0, function(d) {') && c.includes('cluster = val(d0[["G"]])')), 'the pages with a bootstrap by groups of cases are in the program');
  assert.ok(chunks.filter((c) => c.includes('pq_boot_whole(')).length >= wholeBootCases.length, 'a bootstrap of the whole analysis is written for every procedure that has one');
  const program = [helpersFor(chunks.join('\n')), ...chunks.map((c, i) => `f${i} <- function() {\n${c}\n}`)].join('\n');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'prismqda-rcode-'));
  try {
    fs.writeFileSync(path.join(dir, 'program.R'), program, 'utf8');
    fs.writeFileSync(path.join(dir, 'parse.R'), 'invisible(parse(file = "program.R", encoding = "UTF-8")); cat("parsed\\n")', 'utf8');
    const ran = spawnSync(rscript!, ['parse.R'], { cwd: dir, encoding: 'utf8', timeout: 60000 });
    assert.equal(ran.status, 0, `R could not read the code: ${ran.stderr}`);
    assert.match(ran.stdout, /parsed/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('a logistic regression models the value chosen, enters predictors in steps, and prints the odds ratios and the checks', () => {
  const code = pageCode(page('logistic_regression', [v('Site', false, ['N', 'S']), v('Age', true), v('Group', false, ['A', 'B', 'C'])], { event: 'S', profile: true, options: { steps: [1, 2], references: { 'attr:Group': 'B' } } }));
  assert.ok(code.includes('m[["y"]] <- as.numeric(m[["y"]] == "S")'), 'the outcome is 1 for the value modelled');
  assert.ok(code.includes('fit1 <- glm(y ~ `Age`, data = m, family = binomial)') && code.includes('fit2 <- glm(y ~ `Age` + `Group`, data = m, family = binomial)'));
  assert.ok(code.includes('ref <- "B"'), 'the reference chosen');
  assert.ok(code.includes('profile_ci(fit2, 0.95)') && !code.includes('profile_ci(fit1'), 'the profile intervals are for the last model');
  assert.ok(code.includes('anova(fit1, fit2, test = "Chisq")') && code.includes('drop1(fit2, test = "Chisq")'));
  assert.ok(code.includes('pseudo_r2(fit2)') && code.includes('hosmer_lemeshow(fit2)') && code.includes('classify_auc(fit2)'));
  const without = pageCode(page('logistic_regression', [v('Site', false, ['N', 'S']), v('Age', true)], { event: 'S', profile: false }));
  assert.ok(!without.includes('profile_ci('), 'no profile interval was worked out, so none is printed');
  const helpers = helpersFor(code);
  for (const name of ['profile_ci', 'pseudo_r2', 'hosmer_lemeshow', 'classify_auc']) assert.ok(helpers.includes(`${name} <- function`), `${name} is defined`);
});

test('a curve fit fits each curve that was fitted with the formula R reads, and prints the R squared on the original scale', () => {
  const code = pageCode(page('curve_fit', [v('Score', true), v('Age', true)], { curveKinds: ['linear', 'quadratic', 'power', 's_curve'], options: { level: 0.9 } }));
  assert.ok(code.includes('lm(y ~ x, data = d)') && code.includes('lm(y ~ x + I(x^2), data = d)'));
  assert.ok(code.includes('lm(log(y) ~ log(x), data = d)') && code.includes('lm(log(y) ~ I(1 / x), data = d)'));
  assert.ok(code.includes('confint(fit, level = 0.9)'));
  assert.ok(code.includes('curve_original(fit, y, TRUE)') && code.includes('curve_original(fit, y, FALSE)'), 'the curves fitted to the logarithm are told from the others');
  assert.ok(code.includes('exp(coef(fit)[1])'), 'the constant of a power curve is printed in the curve\'s own form');
  assert.ok(!code.includes('I(x^3)') && !code.includes('lm(log(y) ~ x,'), 'a curve that was not fitted is not in the script');
  assert.ok(helpersFor(code).includes('curve_original <- function'));
});

// ---------------------------------------------------------------- the bootstrap and the charts

test('the generator written in R draws the numbers PrismQDA draws', { skip: rscript ? false : 'R is not installed here' }, () => {
  // SplitMix64 with a reference written in BigInt: the draw k of a seed is the mix of seed + k * gamma, and the draw in 1..m is the high half of the product with m.
  const M = (1n << 64n) - 1n;
  const GAMMA = 0x9e3779b97f4a7c15n;
  const mix = (seed: bigint, k: bigint) => {
    let z = (seed + k * GAMMA) & M;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & M;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & M;
    return z ^ (z >> 31n);
  };
  assert.deepEqual([1n, 2n, 3n].map((k) => mix(1234567n, k)), [6457827717110365317n, 3203168211198807973n, 9817491932198370423n], 'the reference is the published SplitMix64');
  const seeds = [0, 1, 1234567, 4294967296, 4503599627370495];
  const sizes = [2, 10, 1000, 2000000];
  const lines = seeds.flatMap((s) => sizes.map((m) => `cat(sprintf("%.0f", ${s}), sprintf("%.0f", ${m}), paste(sprintf("%.0f", pq_draws(${s}, 7, 12, ${m})), collapse = " "), "\\n")`));
  const program = [helpersFor('pq_draws(1, 0, 1, 1)'), ...lines].join('\n');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'prismqda-rdraws-'));
  try {
    fs.writeFileSync(path.join(dir, 'draws.R'), program, 'utf8');
    const ran = spawnSync(rscript!, ['draws.R'], { cwd: dir, encoding: 'utf8', timeout: 60000 });
    assert.equal(ran.status, 0, ran.stderr);
    const got = ran.stdout.trim().split('\n').map((l) => l.trim());
    const want = seeds.flatMap((s) =>
      sizes.map((m) => `${s} ${m} ${Array.from({ length: 12 }, (_, i) => Number((mix(BigInt(s), BigInt(8 + i)) * BigInt(m)) >> 64n) + 1).join(' ')}`),
    );
    assert.deepEqual(got, want);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('a page with a bootstrap of its statistic repeats it with the seed, the resamples, and the level the page used', () => {
  const code = (p: RPage) => pageCode(p);
  const plain = code(withBoot(page('one_sample_t', [numV('X')], { options: { mu: 3, level: 0.9 } }), { seed: 11, resamples: 250 }));
  assert.ok(plain.includes('print(pq_boot(length(x), function(i) { c(mean = mean(x[i])) }, 11, 250, 0.9))'), plain);
  const groups = code(withBoot(page('independent_t', [numV('X'), catV('G')], { groups: ['A', 'B'] })));
  assert.ok(groups.includes('pq_boot_groups(c(length(a), length(b)), function(i) c(mean_difference = mean(a[i[[1]]]) - mean(b[i[[2]]])), 7, 100, 0.95)'), 'the groups are resampled each within itself, in the order the page does');
  const med = code(withBoot(page('mann_whitney', [numV('X'), catV('G')], { groups: ['A', 'B'] })));
  assert.ok(med.includes('median_difference = median(a[i[[1]]]) - median(b[i[[2]]])'));
  assert.ok(code(withBoot(page('paired_t', [numV('X'), numV('Y')]))).includes('dd <- x - y'));
  const reg = code(withBoot(page('linear_regression', [numV('Y'), numV('X'), catV('G')], { options: { steps: [1, 2] } })));
  assert.ok(reg.includes('lm.fit(Xb[i, , drop = FALSE], yb[i], tol = 1e-10)') && reg.includes('r_squared = 1 - sum(f$residuals^2) / tss'), 'a predictor the others account for is dropped at the tolerance PrismQDA uses');
  assert.ok(reg.includes('Xb <- model.matrix(fit2)'), 'the coefficients of the last step');
  const logit = code(withBoot(page('logistic_regression', [catV('Y', ['no', 'yes']), numV('X')], { event: 'yes' })));
  assert.ok(logit.includes('pq_logit(Xb[i, , drop = FALSE], yb[i])') && logit.includes('auc = '), 'the fit is the one PrismQDA makes, and the AUC is reported with the coefficients');
  assert.ok(!code(withBoot(page('describe', [catV('G')], { kind: 'frequencies', counts: ['A', 'B'] }))).includes('pq_boot'), 'counts of categories have no bootstrap');
  assert.ok(!code(page('one_sample_t', [numV('X')])).includes('pq_'), 'a page without a bootstrap has no bootstrap code');
  for (const p of statBootPages()) assert.ok(code(p).includes('pq_boot'), `${p.procedure} has its bootstrap`);
});

test('a bootstrap of the whole analysis works out the numbers the page leads with again on every resample', () => {
  const rows = [{ path: 't_test/estimate', label: 'Mean difference', p: false }, { path: 't_test/p', label: 'p', p: true }];
  const p = withBoot(page('independent_t', [numV('X'), catV('G')], { groups: ['A', 'B'], bootRows: rows, options: { equal_variances: true } }), { analysis: true, seed: 5, resamples: 300 });
  const code = pageCode(p);
  assert.ok(code.includes('d0 <- d'), 'the cases as they were are kept before the page changes anything');
  assert.ok(code.includes('pq_boot_whole(d0, function(d) {') && code.includes('c("Mean difference", "p")') && code.includes('c(FALSE, TRUE), 5, 300, 0.95)'), code);
  assert.ok(code.includes('tt <- t.test(a, b, var.equal = TRUE)') && code.includes('c(mean(a) - mean(b), tt$p.value)'));
  assert.ok(!code.includes('cluster ='), 'cases are resampled one by one');
  // Groups of cases are resampled whole.
  const by = pageCode(withBoot(page('independent_t', [numV('X'), catV('G')], { groups: ['A', 'B'], bootRows: rows, clusterLabel: 'Team' }), { cluster: 'attr:Team' }));
  assert.ok(by.includes('cluster = val(d0[["Team"]])'), by);
  // A page the engine gives no numbers for, or a number the script cannot repeat, is not guessed at.
  assert.ok(!pageCode(withBoot(page('independent_t', [numV('X'), catV('G')], { groups: ['A', 'B'] }), { analysis: true })).includes('pq_boot_whole'), 'no rows, no call');
  const odd = pageCode(withBoot(page('independent_t', [numV('X'), catV('G')], { groups: ['A', 'B'], bootRows: [{ path: 'nothing/here', label: 'Odd', p: false }] }), { analysis: true }));
  assert.ok(odd.includes('NA_real_'), 'a number it cannot repeat is NA, not a wrong number');
  // A resample the engine could not analyze is a failure in R too.
  const gof = pageCode(withBoot(page('goodness_of_fit', [catV('G')], { counts: ['A', 'B', 'C'], bootRows: [{ path: 'goodness_of_fit/p', label: 'p', p: true }] }), { analysis: true }));
  assert.ok(gof.includes('stop("fewer than two categories")'));
  const fisher = pageCode(withBoot(page('fisher', [catV('G'), catV('H')], { rowValues: ['A', 'B'], colValues: ['A', 'B'], bootRows: [{ path: 'fisher/p', label: 'p', p: true }] }), { analysis: true }));
  assert.ok(fisher.includes('stop("not a two by two table")'));
  const lg = pageCode(withBoot(page('logistic_regression', [catV('Y', ['no', 'yes']), numV('X')], { event: 'yes', bootRows: [{ path: 'models/0/auc', label: 'AUC', p: false }, { path: 'models/0/terms/1/p', label: 'p', p: true }] }), { analysis: true }));
  assert.ok(lg.includes('pq_logit(model.matrix(f), f$y)') && lg.includes('stop("no fit")'), 'a resample the engine could not fit is left out in the same cases');
  assert.ok(lg.includes('2 * pnorm(-abs(at(2) / se_at(2)))'), 'the p-value of a coefficient is the Wald one the page shows');
  for (const w of wholeBootPages()) assert.ok(pageCode(w).includes('pq_boot_whole(d0'), `${w.procedure} has its bootstrap of the whole analysis`);
});

test('the helpers a bootstrap or a chart calls come with it, and only those', () => {
  const stat = helpersFor(pageCode(withBoot(page('pearson', [numV('X'), numV('Y')]))));
  for (const name of ['pq_boot', 'pq_draws', 'pq_interval', 'print.pq_table']) assert.ok(stat.includes(`${name} <- function`), `${name} is defined`);
  assert.ok(!stat.includes('pq_boot_whole <- function') && !stat.includes('pq_logit <- function') && !stat.includes('pq_hist <- function'), 'what the page does not call is not written');
  const whole = helpersFor(pageCode(wholeBootPages()[3]));
  for (const name of ['pq_boot_whole', 'pq_draws', 'pq_interval']) assert.ok(whole.includes(`${name} <- function`), `${name} is defined`);
  const logit = helpersFor(pageCode(statBootPages().find((x) => x.procedure === 'logistic_regression')!));
  assert.ok(logit.includes('pq_logit <- function'));
  const charts = helpersFor(chartPages().map((x) => pageCode(x)).join('\n'));
  for (const name of ['pq_hist', 'pq_bars', 'pq_box', 'pq_means', 'pq_stacked', 'pq_qq', 'pq_scatter', 'pq_curves', 'pq_bins']) assert.ok(charts.includes(`${name} <- function`), `${name} is defined`);
  for (const kind of Object.keys(HELPERS)) assert.equal((helpersFor(`${kind}(1)`).match(new RegExp(`^${kind.replace('.', '\\.')} <- function`, 'gm')) ?? []).length, 1, `${kind} is defined once when it is called`);
});

test('a resample that ties with the estimate, or a p-value that is .05, is told by a tolerance the engine uses too, not by rounding', () => {
  const helpers = helpersFor(pageCode(wholeBootPages()[3]));
  assert.ok(helpers.includes('reps < est - 1e-9 * max(1, abs(est))'), 'the bias correction of BCa');
  assert.ok(helpers.includes('< 0.05 - 1e-9'), 'the share of resamples with p below .05');
  const compare = pageCode(withBoot(page('median_test', [numV('X'), catV('G')], { groups: ['A', 'B', 'C'], bootRows: [{ path: 'median_test/p', label: 'p', p: true }] }), { analysis: true }));
  assert.ok(compare.includes('if (nlevels(g) < 2) stop("fewer than two groups")'), 'a resample with one group left is not analyzed');
  const runs = pageCode(withBoot(page('runs', [numV('X')], { options: { cut: 'value', mu: 12.5 }, bootRows: [{ path: 'runs/runs', label: 'Runs', p: false }] }), { analysis: true }));
  assert.ok(runs.includes('if (length(unique(side)) < 2) stop("every case falls on the same side")'), 'nor one whose cases are all on one side of the cut');
  // The cases the engine will not analyze, which a project with empty cases makes common among the small resamples.
  const anova = pageCode(withBoot(page('one_way_anova', [numV('X'), catV('G')], { groups: ['A', 'B', 'C'], bootRows: [{ path: 'anova/test/f', label: 'F', p: false }] }), { analysis: true }));
  assert.ok(anova.includes('length(y) <= nlevels(g)') && anova.includes('== 0) stop("too few cases, or the cases of every group are alike")'), 'an analysis of variance whose groups are all alike has no F');
  const partial = pageCode(withBoot(page('partial_correlation', [numV('X'), numV('Y'), numV('Z')], { bootRows: [{ path: 'partial/r', label: 'r', p: false }] }), { analysis: true }));
  assert.ok(partial.includes('length(x) < ncol(ctrl) + 4') && partial.includes('lm(x ~ ctrl, tol = 1e-10)'), 'a partial correlation needs four cases more than controls, and controls that are not aliased');
  const reg = pageCode(withBoot(page('linear_regression', [numV('Y'), numV('X')], { bootRows: [{ path: 'models/0/r_squared', label: 'R squared', p: false }] }), { analysis: true }));
  assert.ok(reg.includes('nrow(m) <= length(coef(fit1))'), 'a regression needs more cases than parameters');
  assert.ok(pageCode(withBoot(page('linear_regression', [numV('Y'), numV('X')]))).includes('length(i) <= ncol(Xb)'), 'and so does the bootstrap of its coefficients');
});

test('a chart is drawn and its numbers are printed, from the same subjects the page charts', () => {
  const [num, cats, groups, stacked, corr, reg, curves] = chartPages().map((x) => pageCode(x));
  assert.ok(num.includes('pq_hist(x, "Histogram of X", "X", "Cases", 3)'), 'the reference line of a histogram is passed on');
  assert.ok(num.includes('pq_qq(x, "Normal probability plot of X", "Expected if normal", "Value")'));
  assert.ok(cats.includes('pq_bars(x, c("A", "B"), "Bar chart of G", "G", "Cases")'), 'the bars are the categories the page counted');
  assert.ok(groups.includes('pq_box(y, g, "Box plot of X by G", "G", "X")') && groups.includes('pq_means(y, g, 0.95, "Means of X by G", "G", "X")'));
  assert.ok(stacked.includes('pq_stacked(tab, "Stacked bars", "G", "H")'));
  assert.ok(corr.includes('pq_scatter(x, y, "Scatter plot", "X", "Y", line = unname(coef(lm(y ~ x))))'));
  assert.ok(reg.includes('pq_scatter(m[["X"]], m[["y"]], "Y against X", "X", "Y", line = unname(coef(fit1)))'), reg);
  assert.ok(reg.includes('pq_scatter(fitted(fit1), resid(fit1), "Residuals against fitted", "Fitted", "Residuals", zero = TRUE)'));
  assert.ok(reg.includes('pq_qq(resid(fit1), "Normal probability plot of the residuals", "Expected if normal", "Value")'));
  assert.ok(curves.includes('pq_curves(x, y, fits[c("linear", "power")], "Curves", "X", "Y")') && curves.includes('fits <- list()') && curves.includes('fits[["linear"]] <- fit') && curves.includes('fits[["power"]] <- fit'), 'each curve that was fitted is kept to be drawn');
  assert.ok(!pageCode(page('curve_fit', [numV('Y'), numV('X')], { curveKinds: ['linear'] })).includes('fits <- list()'), 'a page that draws no curves keeps none');
  assert.ok(!pageCode(page('describe', [numV('X')], { kind: 'descriptives' })).includes('pq_'), 'a page without charts draws none');
});

test('a script draws the charts of its pages into one PDF beside the data file, and has none when no page has a chart', () => {
  const variables: StatVariable[] = [{ key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale', valid: 6, missing: 0, not_a_number: 0, numeric: true, distinct: 6, categories: [] }];
  const base = { procedure: 'describe', variables: [{ key: 'attr:Score', name: 'Score', kind: 'attribute', code_id: null, measure: 'scale' }], groups: [], assumptions: [], details: { kind: 'descriptives' } };
  const withChart = { ...base, charts: [{ kind: 'histogram', subject: { type: 'variable', index: 0 }, n: 6, bins: [], mean: 1, sd: 1, reference: null }] } as unknown as StatResult;
  const without = { ...base, charts: [] } as unknown as StatResult;
  const entry = (result: StatResult, id = 1) => ({ kind: 'analysis' as const, id, ranAt: '', note: '', section: '', earlierRuns: 0, request: { procedure: 'describe' as const, variables: ['attr:Score'] }, result });
  const drawn = scriptText([entry(withChart), entry(without, 2)], variables, { Score: 'numeric' }, 'Check');
  assert.equal((drawn.match(/^pdf\(/gm) ?? []).length, 1, 'one file for all the pages');
  assert.ok(drawn.includes('"-charts.pdf"') && drawn.includes('tools::file_path_sans_ext("{{data-file}}")'), 'it is named after the data file');
  assert.ok(drawn.indexOf('pdf(') < drawn.indexOf('# ---- Page 1') && drawn.trimEnd().endsWith('invisible(dev.off())'), 'opened before the first page and closed after the last');
  assert.ok(drawn.includes('pq_hist(x, '), drawn);
  const plain = scriptText([entry(without)], variables, { Score: 'numeric' }, 'Check');
  assert.ok(!plain.includes('pdf(') && !plain.includes('dev.off()'), 'no chart, no file');
  const spec = pageSpec(entry(withChart), new Map(variables.map((x) => [x.key, x])));
  assert.deepEqual(spec.charts?.map((c) => c.chart.kind), ['histogram']);
  assert.ok((spec.charts?.[0].title ?? '').length > 0 && (spec.charts?.[0].names ?? []).length === 2, 'the title and the names are the words of the window');
});
