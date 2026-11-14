// The tests of counts, ranks, and distributions added to the Statistics page after the first set: the names, the tables, the results
// sentences, the plain readings, and which of them the chooser offers for the variables picked. statistics.ts hands the procedures
// it does not know to the functions here.

import { num, t, tn } from '../i18n';
import type { StatCurveError, StatCurveFitResult, StatCurveKind, StatLogisticDetails, StatResult, StatVariable } from '../types';
import { ciText, fmt, fmtNoZero, fmtOdds, groupable, levelText, magnitude, magnitudeWord, numericUse, pCell, pText, percentText, termLabel, usable, valueLabel, variableLabel } from './statistics';
import type { ApaTable, Goal, GoalInfo, Method } from './statistics';

const NOT_SIGNIFICANT = () => t('This does not show a difference that can be told from chance. Report it as not statistically significant, with the size of the effect and its interval, and do not describe it as marginal.');

function names(r: StatResult): string[] {
  return r.variables.map((v) => variableLabel(v));
}

function caution(p: number): string {
  return p >= 0.05 ? ' ' + NOT_SIGNIFICANT() : '';
}

/** "differed from" or "did not differ from", by the .05 line. */
function differed(p: number): string {
  return p < 0.05 ? t('differed from') : t('did not differ from');
}

export function extraProcedureName(p: string): string | null {
  switch (p) {
    case 'shapiro_wilk':
      return t('Shapiro-Wilk test of normality');
    case 'ks_normal':
      return t('Kolmogorov-Smirnov test against a normal curve');
    case 'runs':
      return t('Runs test');
    case 'binomial':
      return t('Binomial test');
    case 'goodness_of_fit':
      return t('One-sample chi-square test');
    case 'sign':
      return t('Sign test');
    case 'friedman':
      return t("Friedman's test");
    case 'ks_two':
      return t('Kolmogorov-Smirnov test of two groups');
    case 'median_test':
      return t("Mood's median test");
    case 'moses':
      return t('Moses test of extreme reactions');
    case 'mcnemar':
      return t("McNemar's test");
    case 'ordinal_association':
      return t('Association between ordered categories');
    case 'partial_correlation':
      return t('Partial correlation');
    case 'two_way_anova':
      return t('Two-way analysis of variance');
    case 'cronbach_alpha':
      return t("Cronbach's alpha");
    case 'logistic_regression':
      return t('Logistic regression');
    case 'curve_fit':
      return t('Curve fit');
    default:
      return null;
  }
}

/** The title of a page for a procedure of this file, or null for one it does not know. */
export function extraTitle(r: StatResult): string | null {
  const name = extraProcedureName(r.procedure);
  if (name === null) return null;
  const n = names(r);
  switch (r.procedure) {
    case 'sign':
      return r.variables.length === 2 ? `${name}: ${t('{a} and {b}', { a: n[0], b: n[1] })}` : `${name}: ${n[0]}`;
    case 'friedman':
      return `${name}: ${n.join(', ')}`;
    case 'ks_two':
    case 'median_test':
    case 'moses':
      return `${name}: ${t('{a} by {b}', { a: n[0], b: n[1] })}`;
    case 'mcnemar':
    case 'ordinal_association':
      return `${name}: ${t('{a} and {b}', { a: n[0], b: n[1] })}`;
    case 'partial_correlation':
      return `${name}: ${t('{a} and {b}', { a: n[0], b: n[1] })}, ${t('holding constant {controls}', { controls: (r.details.controls ?? []).join(', ') })}`;
    case 'two_way_anova':
      return `${name}: ${t('{outcome} by {a} and {b}', { outcome: n[0], a: n[1], b: n[2] })}`;
    case 'cronbach_alpha':
      return `${name}: ${n.join(', ')}`;
    case 'logistic_regression':
      return `${name}: ${t('{outcome} from {predictors}', { outcome: n[0], predictors: n.slice(1).join(', ') })}`;
    case 'curve_fit':
      return `${name}: ${t('{y} against {x}', { y: n[0], x: n[1] })}`;
    default:
      return `${name}: ${n[0]}`;
  }
}

function groupRows(r: StatResult, extra?: (i: number) => string[]): string[][] {
  return r.groups.map((g, i) => [g.label, num(g.n), fmt(g.median), ...(extra ? extra(i) : [])]);
}

export function extraTables(r: StatResult): ApaTable[] {
  const d = r.details;
  const out: ApaTable[] = [];
  const [first] = names(r);
  switch (r.procedure) {
    case 'shapiro_wilk': {
      const s = d.shapiro_wilk;
      if (!s) break;
      out.push({
        title: t('Shapiro-Wilk test of {name}', { name: first }),
        head: ['W', 'n', 'p'],
        rows: [[fmtNoZero(s.w, 3), num(s.n), pCell(s.p)]],
        note: t('A normal sample gives a W close to 1. The p-value is for the hypothesis that the values come from a normal distribution.'),
      });
      break;
    }
    case 'ks_normal': {
      const k = d.ks;
      if (!k) break;
      out.push({
        title: t('Kolmogorov-Smirnov test of {name} against a normal curve', { name: first }),
        head: ['D', 'n', 'p', t('Mean of the curve'), t('SD of the curve')],
        rows: [[fmtNoZero(k.d, 3), num(k.n1), pCell(k.p), fmt(d.mean), fmt(d.sd)]],
        note:
          (d.estimated
            ? t('The mean and the standard deviation of the curve were taken from the sample, so this p-value is too large (the test needs a different table when they are estimated). Use the Shapiro-Wilk test for the question of normality, or give the mean and standard deviation of a curve you have a reason for.')
            : t('The curve is the normal distribution with the mean and standard deviation given.')) +
          ' ' +
          (k.exact ? t('The p-value is exact (fewer than 100 values, no ties).') : t('The p-value is the limiting distribution for large samples.')),
      });
      break;
    }
    case 'runs': {
      const x = d.runs;
      if (!x) break;
      const exact = (d.n ?? 0) <= 50;
      out.push({
        title: t('Runs test of {name}', { name: first }),
        head: [t('Runs'), t('Expected'), 'SD', 'z', 'p', t('Cases in the upper part'), t('Cases in the lower part')],
        rows: [[num(x.runs), fmt(x.mean), fmt(x.sd), fmt(x.z), pCell(exact ? x.p_exact : x.p_normal), num(x.n1), num(x.n0)]],
        note: [
          d.cut_kind ? t('{name} was cut at {kind}, {cut}; cases at or above the cut are in the upper part.', { name: first, kind: d.cut_kind === 'median' ? t('the median') : d.cut_kind === 'mean' ? t('the mean') : t('the value'), cut: fmt(d.cut ?? null) }) : t('The cases take two values; the runs are counted in the order of the cases.'),
          exact ? t('The p-value is exact (50 cases or fewer).') : t('The p-value is the normal approximation with a continuity correction.'),
          t('The order is the order of the cases in the project.'),
        ].join(' '),
      });
      break;
    }
    case 'binomial': {
      const b = d.binomial;
      if (!b) break;
      out.push({
        title: t('Binomial test of {name}', { name: first }),
        head: [t('Counted'), t('Cases'), 'n', t('Share'), t('Compared with'), 'p', t('{level} CI', { level: levelText(b.level) })],
        rows: [[d.success_label ?? '', num(b.successes), num(b.n), fmtNoZero(b.estimate, 3), fmtNoZero(b.p0, 3), pCell(b.p), ciText(b.ci, 3)]],
        note: t('The p-value is exact, and the interval is the Clopper-Pearson interval of the share.'),
      });
      break;
    }
    case 'goodness_of_fit': {
      const g = d.goodness_of_fit;
      if (!g || !d.counts) break;
      out.push({
        title: t('Counts of {name} against expected shares', { name: first }),
        head: [t('Category'), t('Observed'), t('Expected'), t('Share expected'), t('Residual')],
        rows: d.counts.map((c) => [c.label, num(c.count), fmt(c.expected), percentText(c.share ?? 0), fmt(c.residual)]),
        note: t('A residual is (observed - expected) divided by the square root of the expected count; beyond 2 in size marks a category with noticeably more or fewer cases than expected.'),
      });
      out.push({ title: t('Chi-square test'), head: ['χ²', 'df', 'n', 'p'], rows: [[fmt(g.chi2), fmt(g.df, 0), num(g.n), pCell(g.p)]] });
      break;
    }
    case 'sign': {
      const s = d.sign;
      if (!s) break;
      out.push({
        title: d.paired ? t('Signs of the differences') : t('Signs of the differences from {mu}', { mu: fmt(d.mu ?? 0) }),
        head: [t('Above'), t('Below'), t('Equal (set aside)'), d.paired ? t('Median difference') : t('Median'), t('Share above'), t('{level} CI', { level: levelText(s.level) }), 'p'],
        rows: [[num(s.positive), num(s.negative), num(s.zeros), fmt(s.median), fmtNoZero(s.proportion_positive, 3), ciText(s.ci, 3), pCell(s.p)]],
        note: t('The sign test counts only the direction of each difference. The p-value is exact (a binomial test of the signs at one half).'),
      });
      break;
    }
    case 'friedman': {
      const f = d.friedman;
      if (!f) break;
      out.push({
        title: t('The measures'),
        head: [t('Measure'), t('Mean rank'), t('Median')],
        rows: r.groups.map((g, i) => [g.label, fmt(f.mean_ranks[i], 2), fmt(g.median)]),
        note: t('Each case ranks the measures from lowest to highest; the mean rank is the average of those places.'),
      });
      out.push({ title: t("Friedman's test"), head: ['χ²', 'df', 'n', 'p', t("Kendall's W")], rows: [[fmt(f.chi2), fmt(f.df, 0), num(f.n), pCell(f.p), fmtNoZero(f.kendall_w, 3)]], note: t('The statistic is corrected for ties.') });
      break;
    }
    case 'ks_two': {
      const k = d.ks;
      if (!k) break;
      out.push({ title: t('{outcome} by group', { outcome: first }), head: [t('Group'), 'n', t('Median')], rows: groupRows(r) });
      out.push({
        title: t('Kolmogorov-Smirnov test of two groups'),
        head: ['D', 'p'],
        rows: [[fmtNoZero(k.d, 3), pCell(k.p)]],
        note: k.exact ? t('The p-value is exact (the groups together have fewer than 10,000 pairs of cases), with ties handled as R does.') : t('The p-value is the limiting distribution for large samples.'),
      });
      break;
    }
    case 'median_test': {
      const m = d.median_test;
      if (!m) break;
      out.push({
        title: t('Cases above the pooled median, {median}', { median: fmt(m.median) }),
        head: [t('Group'), 'n', t('Above'), t('At or below')],
        rows: r.groups.map((g, i) => [g.label, num(g.n), num(m.above[i]), num(m.at_or_below[i])]),
      });
      out.push({
        title: t("Mood's median test"),
        head: ['χ²', 'df', 'p', ...(m.fisher_p !== null ? [t("Fisher's exact p")] : [])],
        rows: [[fmt(m.chi2), fmt(m.df, 0), pCell(m.p), ...(m.fisher_p !== null ? [pCell(m.fisher_p)] : [])]],
        note: t('A case equal to the median counts as at or below it.'),
      });
      break;
    }
    case 'moses': {
      const m = d.moses;
      if (!m) break;
      out.push({
        title: t('Moses test of {name}', { name: first }),
        head: [t('Control group'), t('Control n'), t('Other n'), t('Span'), 'p', t('Trimmed'), t('Trimmed span'), t('Trimmed p')],
        rows: [[d.control ?? '', num(m.n_control), num(m.n_test), num(m.span), pCell(m.p), num(m.trim), num(m.trimmed_span), pCell(m.trimmed_p)]],
        note: t('The span is how many places in the pooled order the control cases cover, from the lowest to the highest after trimming. The p-values are one-sided and exact: a small span means the other group is more extreme at both ends.'),
      });
      break;
    }
    case 'mcnemar': {
      const m = d.mcnemar;
      if (!m || !d.table || !d.labels) break;
      out.push({
        title: t('{a} by {b}', { a: names(r)[0], b: names(r)[1] }),
        head: [names(r)[0], ...d.labels],
        rows: d.labels.map((l, i) => [l, num(d.table![i][0]), num(d.table![i][1])]),
        note: t('Rows are the first answer and columns the second.'),
      });
      out.push({
        title: t("McNemar's test"),
        head: [t('Changed one way'), t('Changed the other way'), 'χ²', 'df', 'p', t('Exact p')],
        rows: [[num(m.b), num(m.c), fmt(m.chi2_corrected), '1', pCell(m.p_corrected), pCell(m.p_exact)]],
        note: t('χ² and its p-value use the continuity correction. With fewer than 25 cases that changed, the exact p-value is the one to use.'),
      });
      break;
    }
    case 'ordinal_association': {
      const o = d.ordinal;
      if (!o) break;
      const row = (name: string, m: { estimate: number; ase: number; z: number | null; p: number | null }) => [name, fmt(m.estimate, 3), fmt(m.ase, 3), m.z === null ? '—' : fmt(m.z), m.p === null ? '—' : pCell(m.p)];
      out.push({
        title: t('Association between {a} and {b}', { a: names(r)[0], b: names(r)[1] }),
        head: [t('Measure'), t('Value'), t('Standard error'), 'z', 'p'],
        rows: [
          row(t("Goodman and Kruskal's gamma"), o.gamma),
          row(t("Kendall's tau-b"), o.tau_b),
          row(t("Stuart's tau-c"), o.tau_c),
          row(t("Somers' d ({column} depends on {row})", { column: names(r)[1], row: names(r)[0] }), o.somers_columns_dependent),
          row(t("Somers' d ({row} depends on {column})", { row: names(r)[0], column: names(r)[1] }), o.somers_rows_dependent),
        ],
        note: t('{c} pairs of cases rank the same way on both variables and {d} rank opposite ways. The p-value tests whether the measure is zero. The symmetric Somers\' d is {s}.', { c: num(o.concordant), d: num(o.discordant), s: fmt(o.somers_symmetric, 3) }),
      });
      break;
    }
    case 'partial_correlation': {
      const p = d.partial;
      if (!p) break;
      out.push({
        title: t('Partial correlation, holding constant {controls}', { controls: (d.controls ?? []).join(', ') }),
        head: ['r', 'n', 'df', 't', 'p', ...(p.ci ? [t('{level} CI', { level: levelText(p.level) })] : [])],
        rows: [[fmtNoZero(p.r), num(p.n), fmt(p.df, 0), fmt(p.t), pCell(p.p), ...(p.ci ? [ciText(p.ci)] : [])]],
        note: d.zero_order ? t('Without holding anything constant the correlation of the two is {r}.', { r: fmtNoZero(d.zero_order.r) }) : undefined,
      });
      break;
    }
    case 'two_way_anova':
      out.push(...factorialTables(r));
      break;
    case 'cronbach_alpha':
      out.push(...alphaTables(r));
      break;
    case 'logistic_regression':
      out.push(...logisticTables(r));
      break;
    case 'curve_fit':
      out.push(...curveTables(r));
      break;
  }
  return out;
}

/** The effect names of a two-way analysis, in the order of its table. */
function effectNames(r: StatResult): string[] {
  const [, a, b] = names(r);
  return [a, b, t('{a} \u00d7 {b}', { a, b })];
}

function factorialTables(r: StatResult): ApaTable[] {
  const w = r.details.factorial;
  if (!w) return [];
  const [y, a, b] = names(r);
  const labelsA = r.details.labels_a ?? [];
  const labelsB = r.details.labels_b ?? [];
  const sources = effectNames(r);
  const anova: ApaTable = {
    title: t('Two-way analysis of variance of {y} by {a} and {b}', { y, a, b }),
    head: [t('Source'), 'SS', 'df', 'MS', 'F', 'p', 'ηp²'],
    rows: [
      ...w.effects.map((e, i) => [sources[i], fmt(e.ss), fmt(e.df, 0), fmt(e.ms), fmt(e.f), pCell(e.p), fmtNoZero(e.partial_eta_squared, 3)]),
      [t('Error'), fmt(w.error_ss), fmt(w.error_df, 0), fmt(w.error_ms), '', '', ''],
      [t('Total'), fmt(w.total_ss), fmt(w.total_df, 0), '', '', '', ''],
    ],
    note: t('Type III sums of squares: each effect is tested with the other effects, the interaction included, taken into account. R² = {r2}, adjusted R² = {adj}. ηp² is the share of the variation left after the other effects that the effect accounts for.', { r2: fmtNoZero(w.r_squared, 3), adj: fmtNoZero(w.adj_r_squared, 3) }) + (w.balanced ? '' : ' ' + t('The combinations hold different numbers of cases, so the main effects are not simply differences between the marginal means below.')),
  };
  const cells: ApaTable = {
    title: t('Means and standard deviations of {y} for each combination of {a} and {b}', { y, a, b }),
    head: [a, b, 'n', 'M', 'SD'],
    rows: w.cells.map((c) => [labelsA[c.a] ?? String(c.a), labelsB[c.b] ?? String(c.b), num(c.n), fmt(c.mean), c.sd === null ? '—' : fmt(c.sd)]),
  };
  const margins: ApaTable = {
    title: t('Means of {y} for each value of {a} and of {b}, whatever the other is', { y, a, b }),
    head: [t('Variable'), t('Value'), 'n', 'M'],
    rows: [...w.margins_a.map((m) => [a, labelsA[m.level] ?? String(m.level), num(m.n), fmt(m.mean)]), ...w.margins_b.map((m) => [b, labelsB[m.level] ?? String(m.level), num(m.n), fmt(m.mean)])],
  };
  return [anova, cells, margins];
}

function alphaTables(r: StatResult): ApaTable[] {
  const a = r.details.alpha;
  if (!a) return [];
  const list = names(r);
  const level = a.interval?.level;
  const main: ApaTable = {
    title: t("Cronbach's alpha of {items}", { items: list.join(', ') }),
    head: ['α', t('Standardized α'), t('Mean correlation between items'), t('Items'), t('Cases'), ...(a.interval ? [t('{level} CI', { level: levelText(level ?? 0.95) })] : [])],
    rows: [[fmtNoZero(a.alpha, 3), a.standardized_alpha === null ? '—' : fmtNoZero(a.standardized_alpha, 3), a.mean_inter_item_r === null ? '—' : fmtNoZero(a.mean_inter_item_r, 3), num(a.k), num(a.n), ...(a.interval ? [`[${fmtNoZero(a.interval.lower, 3)}, ${fmtNoZero(a.interval.upper, 3)}]`] : [])]],
    note: t('Each case has a value for every item. The sum of the items has a mean of {m} and a standard deviation of {sd}. The interval is Feldt\'s.', { m: fmt(a.total_mean), sd: fmt(a.total_sd) }),
  };
  const items: ApaTable = {
    title: t("What each item adds to the scale"),
    head: [t('Item'), 'M', 'SD', t('Correlation with the rest'), t('α if the item is left out')],
    rows: a.items.map((it, i) => [list[i] ?? '', fmt(it.mean), fmt(it.sd), it.item_rest_r === null ? '—' : fmtNoZero(it.item_rest_r, 3), it.alpha_if_deleted === null ? '—' : fmtNoZero(it.alpha_if_deleted, 3)]),
    note: t("The correlation with the rest is the item's correlation with the sum of the other items."),
  };
  return [main, items];
}

export function extraSentence(r: StatResult): string {
  const d = r.details;
  const n = names(r);
  switch (r.procedure) {
    case 'shapiro_wilk':
      return d.shapiro_wilk
        ? t('The distribution of {name} {verdict} a normal distribution by the Shapiro-Wilk test, W({n}) = {w}, {p}.', { name: n[0], verdict: d.shapiro_wilk.p < 0.05 ? t('departed from') : t('did not depart from'), n: num(d.shapiro_wilk.n), w: fmtNoZero(d.shapiro_wilk.w, 3), p: pText(d.shapiro_wilk.p) })
        : '';
    case 'ks_normal':
      return d.ks
        ? t('The distribution of {name} {verdict} a normal distribution with a mean of {mean} and a standard deviation of {sd}, D({n}) = {d}, {p}.', { name: n[0], verdict: differed(d.ks.p), mean: fmt(d.mean), sd: fmt(d.sd), n: num(d.ks.n1), d: fmtNoZero(d.ks.d, 3), p: pText(d.ks.p) }) + (d.estimated ? ' ' + t('(The mean and standard deviation came from the sample, so the p-value is too large.)') : '')
        : '';
    case 'runs': {
      const x = d.runs;
      if (!x) return '';
      const exact = (d.n ?? 0) <= 50;
      return t('The order of {name} {verdict} a random order: there were {runs} runs where {mean} are expected, {stat}.', {
        name: n[0],
        verdict: differed(exact ? x.p_exact : x.p_normal),
        runs: num(x.runs),
        mean: fmt(x.mean),
        stat: exact ? pText(x.p_exact) : `z = ${fmt(x.z)}, ${pText(x.p_normal)}`,
      });
    }
    case 'binomial': {
      const b = d.binomial;
      if (!b) return '';
      return t('{share} of the cases ({x} of {n}) were {label}, which {verdict} {p0}, exact {p}, {level} CI {ci}.', { share: percentText(b.estimate), x: num(b.successes), n: num(b.n), label: d.success_label ?? '', verdict: differed(b.p), p0: percentText(b.p0), p: pText(b.p), level: levelText(b.level), ci: ciText(b.ci, 3) });
    }
    case 'goodness_of_fit': {
      const g = d.goodness_of_fit;
      if (!g) return '';
      return t('The counts of {name} {verdict} {what}, χ²({df}, N = {n}) = {chi2}, {p}.', { name: n[0], verdict: differed(g.p), what: d.equal_shares ? t('equal shares') : t('the expected shares'), df: fmt(g.df, 0), n: num(g.n), chi2: fmt(g.chi2), p: pText(g.p) });
    }
    case 'sign': {
      const s = d.sign;
      if (!s) return '';
      const rest = { neg: num(s.negative), zeros: num(s.zeros), verdict: differed(s.p), p: pText(s.p) };
      return d.paired
        ? tn(s.positive, { one: '{a} was higher than {b} in {count} case', other: '{a} was higher than {b} in {count} cases' }, { a: n[0], b: n[1] ?? '' }) + ' ' + t('and lower in {neg}, with {zeros} equal and set aside; by the sign test that {verdict} an even split, {p}.', rest)
        : tn(s.positive, { one: '{count} value of {name} was above {mu}', other: '{count} values of {name} were above {mu}' }, { name: n[0], mu: fmt(d.mu ?? 0) }) + ' ' + t('and {neg} below, with {zeros} equal and set aside; by the sign test that {verdict} an even split, {p}.', rest);
    }
    case 'friedman': {
      const f = d.friedman;
      if (!f) return '';
      return t('The {k} measures {verdict} one another in their ranks, χ²({df}, N = {n}) = {chi2}, {p}, Kendall\'s W = {w}.', { k: num(f.k), verdict: f.p < 0.05 ? t('differed from') : t('did not differ from'), df: fmt(f.df, 0), n: num(f.n), chi2: fmt(f.chi2), p: pText(f.p), w: fmtNoZero(f.kendall_w, 3) });
    }
    case 'ks_two': {
      const k = d.ks;
      if (!k || r.groups.length < 2) return '';
      return t('The distributions of {outcome} in {a} and {b} {verdict} each other, D = {d}, {p}.', { outcome: n[0], a: r.groups[0].label, b: r.groups[1].label, verdict: differed(k.p), d: fmtNoZero(k.d, 3), p: pText(k.p) });
    }
    case 'median_test': {
      const m = d.median_test;
      if (!m) return '';
      return t('The share of cases above the pooled median of {outcome} ({median}) {verdict} between the groups of {group}, χ²({df}, N = {n}) = {chi2}, {p}.', { outcome: n[0], median: fmt(m.median), verdict: m.p < 0.05 ? t('differed') : t('did not differ'), group: n[1] ?? '', df: fmt(m.df, 0), n: num(r.n_used), chi2: fmt(m.chi2), p: pText(m.p) });
    }
    case 'moses': {
      const m = d.moses;
      if (!m) return '';
      return t('The control group, {control}, spanned {span} places in the pooled order of {outcome} ({p}); with {trim} trimmed from each end it spanned {tspan} ({tp}). A small span means {test} was more spread out.', { control: d.control ?? '', span: num(m.span), outcome: n[0], p: pText(m.p), trim: num(m.trim), tspan: num(m.trimmed_span), tp: pText(m.trimmed_p), test: d.test ?? '' });
    }
    case 'mcnemar': {
      const m = d.mcnemar;
      if (!m || !d.labels) return '';
      const [x, y] = d.labels;
      return (
        tn(m.b, { one: '{count} case changed from {x} to {y}', other: '{count} cases changed from {x} to {y}' }, { x, y }) +
        ' ' +
        t("and {c} from {y} to {x}; McNemar's χ²(1) = {chi2}, {p} (exact {pe}).", { c: num(m.c), x, y, chi2: fmt(m.chi2_corrected), p: pText(m.p_corrected), pe: pText(m.p_exact) })
      );
    }
    case 'ordinal_association': {
      const o = d.ordinal;
      if (!o) return '';
      return t('{a} and {b} were {direction}ly related by the order of their values, gamma = {g}, tau-b = {tb}, {p}.', { a: n[0], b: n[1], direction: o.gamma.estimate >= 0 ? t('positive') : t('negative'), g: fmtNoZero(o.gamma.estimate, 3), tb: fmtNoZero(o.tau_b.estimate, 3), p: o.tau_b.p === null ? '—' : pText(o.tau_b.p) });
    }
    case 'two_way_anova': {
      const w = d.factorial;
      if (!w) return '';
      const sources = effectNames(r);
      const clause = (i: number): string => {
        const e = w.effects[i];
        const sig = e.p < 0.05 ? t('statistically significant') : t('not statistically significant');
        const what = i < 2 ? t('The main effect of {name}', { name: sources[i] }) : t('The interaction of {a} and {b}', { a: n[1], b: n[2] });
        return t('{what} was {sig}, F({df1}, {df2}) = {f}, {p}, ηp² = {eta}', { what, sig, df1: fmt(e.df, 0), df2: fmt(w.error_df, 0), f: fmt(e.f), p: pText(e.p), eta: fmtNoZero(e.partial_eta_squared, 3) });
      };
      return `${clause(0)}; ${clause(1)}; ${clause(2)}.`;
    }
    case 'cronbach_alpha': {
      const a = d.alpha;
      if (!a) return '';
      return t("Cronbach's alpha for the {k} items was {alpha}{ci}, N = {n}.", {
        k: num(a.k),
        alpha: fmtNoZero(a.alpha, 2),
        ci: a.interval ? `, ${t('{level} CI', { level: levelText(a.interval.level) })} [${fmtNoZero(a.interval.lower, 2)}, ${fmtNoZero(a.interval.upper, 2)}]` : '',
        n: num(a.n),
      });
    }
    case 'partial_correlation': {
      const p = d.partial;
      if (!p) return '';
      return t('The {sign} relation between {a} and {b}, holding constant {controls}, was {sig}, partial r({df}) = {r}, {p}{ci}.', {
        sign: p.r >= 0 ? t('positive') : t('negative'),
        a: n[0],
        b: n[1],
        controls: (d.controls ?? []).join(', '),
        sig: p.p < 0.05 ? t('statistically significant') : t('not statistically significant'),
        df: fmt(p.df, 0),
        r: fmtNoZero(p.r),
        p: pText(p.p),
        ci: p.ci ? `, ${t('{level} CI', { level: levelText(p.level) })} ${ciText(p.ci)}` : '',
      });
    }
    case 'logistic_regression':
      return logisticSentence(r);
    case 'curve_fit':
      return curveSentence(r);
    default:
      return '';
  }
}

export function extraReading(r: StatResult): string {
  const d = r.details;
  switch (r.procedure) {
    case 'shapiro_wilk':
      return t('A normal sample gives a W close to 1. With few cases the test has little power to notice a departure, and with many it notices departures too small to matter, so read it with the plot beside it.');
    case 'ks_normal':
      return t('The test looks for the largest gap between the cumulative distribution of the values and that of the normal curve. It is as sensitive to a difference in the middle as in the tails.');
    case 'runs':
      return t('More runs than a random order gives means the values switch back and forth too often; fewer means they come in streaks, as a trend or clusters would.');
    case 'binomial':
      return d.binomial ? t('The interval shows how far from {share} the true share could plausibly be, given {n} cases.', { share: percentText(d.binomial.estimate), n: num(d.binomial.n) }) + caution(d.binomial.p) : '';
    case 'goodness_of_fit':
      return t('Look at the residuals to see which categories have more or fewer cases than the expected shares give.') + (d.goodness_of_fit ? caution(d.goodness_of_fit.p) : '');
    case 'sign':
      return t('The sign test uses only the direction of each difference and not its size, so it has less power than the Wilcoxon test and needs no assumption about the shape.') + (d.sign ? caution(d.sign.p) : '');
    case 'friedman':
      return d.friedman
        ? t("Kendall's W says how much the cases agree in how they rank the measures: near 0 they disagree, near 1 they agree. Here it is {size} by the usual benchmarks ({w}).", { size: magnitudeWord(magnitude('share', d.friedman.kendall_w, 1)), w: fmtNoZero(d.friedman.kendall_w, 3) }) + caution(d.friedman.p)
        : '';
    case 'ks_two':
      return t('The test is sensitive to any difference between the two distributions, in the middle, the spread, or the shape, and so is less powerful against a difference in the middle alone than a test of the middle.') + (d.ks ? caution(d.ks.p) : '');
    case 'median_test':
      return t('Only whether each case is above the pooled median is used, so this is a weaker test than Kruskal-Wallis, but it needs the fewest assumptions.') + (d.median_test ? caution(d.median_test.p) : '');
    case 'moses':
      return t('This tests spread and not the middle: it asks whether the other group has more extreme cases at both ends than the control group does.') + (d.moses ? caution(Math.min(d.moses.p, d.moses.trimmed_p) * 2) : '');
    case 'mcnemar':
      return t('Only the cases that changed their answer matter. If they changed one way about as often as the other, there is no sign of a shift.') + (d.mcnemar ? caution(d.mcnemar.p_corrected) : '');
    case 'ordinal_association':
      return d.ordinal
        ? t("Each measure runs from -1 to 1, and 0 means no order. Gamma ignores pairs tied on either variable, so it is the largest of the four. Kendall's tau-b corrects for ties, tau-c suits tables whose rows and columns differ in number, and Somers' d treats one variable as the one being predicted. Here tau-b is {size} by the usual benchmarks.", { size: magnitudeWord(magnitude('r', d.ordinal.tau_b.estimate)) }) + (d.ordinal.tau_b.p !== null ? caution(d.ordinal.tau_b.p) : '')
        : '';
    case 'partial_correlation':
      return d.partial ? t('This is the relation that is left once the effect of the controls is taken out of both numbers: {size} by the usual benchmarks. A relation is not a cause.', { size: magnitudeWord(magnitude('r', d.partial.r)) }) + caution(d.partial.p) : '';
    case 'two_way_anova': {
      const w = d.factorial;
      if (!w) return '';
      const [, a, b] = names(r);
      const parts: string[] = [
        t('Each effect is tested with the others taken into account. The share of the variation that an effect accounts for (ηp²) is {size} for {a}, {sizeB} for {b}, and {sizeAB} for the two together, by the usual benchmarks.', {
          a,
          b,
          size: magnitudeWord(magnitude('share', w.effects[0].partial_eta_squared)),
          sizeB: magnitudeWord(magnitude('share', w.effects[1].partial_eta_squared)),
          sizeAB: magnitudeWord(magnitude('share', w.effects[2].partial_eta_squared)),
        }),
      ];
      if (w.effects[2].p < 0.05) parts.push(t('The interaction is statistically significant: the difference one variable makes depends on the other, so a main effect on its own can mislead. Read the means of each combination.'));
      parts.push(t('This shows that the means differ, not which ones: the means of each combination are in the table, and tests of single pairs of them are not part of this page. A difference is not a cause.'));
      if (w.effects.every((e) => e.p >= 0.05)) parts.push(NOT_SIGNIFICANT());
      return parts.join(' ');
    }
    case 'cronbach_alpha': {
      const a = d.alpha;
      if (!a) return '';
      const list = names(r);
      const parts: string[] = [
        t('Alpha says how well the items hang together as one scale: it rises with the number of items and with the correlations between them. A common rule of thumb calls .70 acceptable and .80 or more good, but what is enough depends on what the scale is for. A high alpha does not show that the items measure one thing.'),
      ];
      if (a.negative_pairs > 0) parts.push(tn(a.negative_pairs, { one: '{count} pair of items correlates negatively. An item scored the other way round lowers alpha: reverse it before adding the items up.', other: '{count} pairs of items correlate negatively. An item scored the other way round lowers alpha: reverse it before adding the items up.' }));
      const better = a.items.map((it, i) => ({ it, i })).filter(({ it }) => it.alpha_if_deleted !== null && it.alpha_if_deleted > a.alpha + 0.005);
      if (better.length > 0) parts.push(better.map(({ it, i }) => t('Leaving out {item} would raise alpha to {alpha}.', { item: list[i] ?? '', alpha: fmtNoZero(it.alpha_if_deleted ?? 0, 2) })).join(' '));
      return parts.join(' ');
    }
    case 'logistic_regression':
      return logisticReading(r);
    case 'curve_fit':
      return curveReading(r);
    default:
      return '';
  }
}

// ---------------------------------------------------------------- logistic regression

const logistic = (r: StatResult) => r.details as unknown as StatLogisticDetails;

/** The two values of the outcome as the person reads them: the one modelled, and the one it is compared with. */
function eventLabels(r: StatResult): { event: string; other: string } {
  const d = logistic(r);
  const kind = r.variables[0]?.kind ?? 'attribute';
  return { event: valueLabel(kind, d.event.value, d.event.label), other: valueLabel(kind, d.other.value, d.other.label) };
}

function shareText(x: number | null): string {
  return x === null ? '\u2014' : percentText(x);
}

function logisticTables(r: StatResult): ApaTable[] {
  const d = logistic(r);
  const models = d.models ?? [];
  const terms = d.terms ?? [];
  if (models.length === 0) return [];
  const [outcome] = names(r);
  const { event, other } = eventLabels(r);
  const level = levelText(d.level);
  const last = models[models.length - 1];
  const out: ApaTable[] = [];
  out.push({
    title: t('Model fit for {outcome}', { outcome }),
    head: [t('Model'), t('Deviance'), '\u03c7\u00b2', 'df', 'p', t('\u03c7\u00b2 change'), t('df of the change'), t('p of the change')],
    rows: models.map((m, i) => {
      const change = i > 0 ? d.changes?.[i - 1] : null;
      return [num(i + 1), fmt(m.deviance), fmt(m.lr.chi2), fmt(m.lr.df, 0), pCell(m.lr.p), change ? fmt(change.chi2) : '', change ? fmt(change.df, 0) : '', change ? pCell(change.p) : ''];
    }),
    note: t(
      'The model is of the odds of {event} against {other}, from {n} cases, {events} of them with {event}. The deviance is minus twice the log likelihood (a model with only an intercept has {nullDeviance}); \u03c7\u00b2 is how much lower the deviance is than that, and its p-value tests whether the predictors together explain anything.',
      { event, other, n: num(last.n), events: num(last.events), nullDeviance: fmt(last.null_deviance) },
    ),
  });
  out.push({
    title: t('How well the last model fits and predicts'),
    head: [t('McFadden R\u00b2'), t('Cox & Snell R\u00b2'), t('Nagelkerke R\u00b2'), 'AIC', 'BIC', t('Area under the curve'), t('Classified correctly')],
    rows: [[fmtNoZero(last.mcfadden, 3), fmtNoZero(last.cox_snell, 3), fmtNoZero(last.nagelkerke, 3), fmt(last.aic), fmt(last.bic), fmtNoZero(last.auc, 3), percentText(last.classification.accuracy)]],
    note: t(
      'The three R\u00b2 measures are pseudo R-squared values, not shares of variance; Nagelkerke runs from 0 to 1. AIC and BIC compare models fitted to the same cases (lower is better). The area under the curve is the chance that a case with {event} gets a higher predicted probability than one without it: 0.5 is chance and 1 is perfect.',
      { event },
    ),
  });
  const c = last.classification;
  out.push({
    title: t('Observed and predicted outcomes'),
    head: [t('Observed'), t('Predicted {value}', { value: other }), t('Predicted {value}', { value: event }), t('Classified correctly')],
    rows: [
      [other, num(c.true_negative), num(c.false_positive), shareText(c.specificity)],
      [event, num(c.false_negative), num(c.true_positive), shareText(c.sensitivity)],
    ],
    note: t('A case is predicted to have {event} when its predicted probability is {cutoff} or more. The share of the cases with {event} that are classified correctly is the sensitivity, and the share of the others is the specificity.', { event, cutoff: fmtNoZero(c.cutoff, 2) }),
  });
  if (last.hosmer_lemeshow) {
    const hl = last.hosmer_lemeshow;
    out.push({
      title: t('Hosmer-Lemeshow test of fit'),
      head: ['\u03c7\u00b2', 'df', 'p'],
      rows: [[fmt(hl.chi2), fmt(hl.df, 0), pCell(hl.p)]],
      note: t('The cases are cut into groups of about equal size by their predicted probability, and the count of {event} in each group is compared with the count the model predicts. A small p-value means the model does not match the outcomes well.', { event }),
    });
    out.push({
      title: t('Groups of the Hosmer-Lemeshow test'),
      head: [t('Group'), t('Cases'), t('Observed with {event}', { event }), t('Expected with {event}', { event })],
      rows: hl.groups.map((g, i) => [num(i + 1), num(g.n), num(g.observed), fmt(g.expected)]),
    });
  }
  models.forEach((m, mi) => {
    const withProfile = m.terms.some((tm) => tm.or_profile_ci !== null);
    const rows = m.terms.map((tm, i) => [
      i === 0 ? t('Intercept') : terms[i - 1] ? termLabel(terms[i - 1]) : String(i),
      fmt(tm.estimate),
      fmt(tm.se),
      fmt(tm.z),
      pCell(tm.p),
      fmtOdds(tm.odds_ratio),
      ciText(tm.or_ci, 2, true),
      ...(withProfile ? [i === 0 ? '\u2014' : ciText(tm.or_profile_ci, 2, true)] : []),
    ]);
    out.push({
      title: models.length > 1 ? t('Coefficients, model {n}', { n: num(mi + 1) }) : t('Coefficients'),
      head: [t('Predictor'), 'B', 'SE', 'z', 'p', t('Odds ratio'), t('{level} CI of the odds ratio (Wald)', { level }), ...(withProfile ? [t('{level} CI of the odds ratio (profile)', { level })] : [])],
      rows,
      note:
        mi === models.length - 1
          ? t(
              'B is the change in the log odds of {event} that goes with one more unit of the predictor, or with the category against its reference, while the other predictors are held fixed; the odds ratio is e to the power B. For the intercept it is the odds when every predictor is 0. The Wald interval assumes the estimate is normal; the profile-likelihood interval does not, and is the better of the two with few cases.',
              { event },
            )
          : undefined,
    });
  });
  if ((d.blocks ?? []).length > 0) {
    out.push({
      title: t('Each variable left out'),
      head: [t('Variable'), t('Terms'), '\u03c7\u00b2', 'df', 'p'],
      rows: d.blocks.map((b) => [variableLabel({ name: b.name, kind: b.kind }), num(b.terms), b.lr ? fmt(b.lr.chi2) : '\u2014', b.lr ? fmt(b.lr.df, 0) : '\u2014', b.lr ? pCell(b.lr.p) : '\u2014']),
      note: t('The likelihood-ratio test of what a variable adds to the model that has all the others: the deviance without it, minus the deviance with it. A categorical variable is left out with all its columns at once.'),
    });
  }
  if ((d.influential ?? []).length > 0) {
    out.push({
      title: t('Cases that pull the fit most'),
      head: [t('Case'), t('Observed'), t('Predicted probability'), t('Leverage'), t("Cook's distance"), t('Standardized residual')],
      rows: d.influential.map((c2) => [c2.case_name, c2.observed === 1 ? event : other, fmtNoZero(c2.fitted, 3), fmtNoZero(c2.leverage, 3), fmt(c2.cooks_distance, 3), fmt(c2.standardized_residual)]),
      note: t("Cases with a Cook's distance above 4 divided by the number of cases, or a standardized residual beyond 3."),
    });
  }
  return out;
}

function logisticSentence(r: StatResult): string {
  const d = logistic(r);
  const models = d.models ?? [];
  const terms = d.terms ?? [];
  const last = models[models.length - 1];
  if (!last) return '';
  const [outcome, ...predictors] = names(r);
  const { event, other } = eventLabels(r);
  const level = levelText(last.level);
  const first = t('The model of {outcome} ({event} against {other}) with {predictors} {verdict}, \u03c7\u00b2({df}, N = {n}) = {chi2}, {p}; Nagelkerke R\u00b2 = {r2}, and it classified {correct} of the cases correctly (area under the curve = {auc}).', {
    outcome,
    event,
    other,
    predictors: predictors.join(', '),
    verdict: last.lr.p < 0.05 ? t('fitted better than a model with only an intercept') : t('did not fit better than a model with only an intercept'),
    df: fmt(last.lr.df, 0),
    n: num(last.n),
    chi2: fmt(last.lr.chi2),
    p: pText(last.lr.p),
    r2: fmtNoZero(last.nagelkerke, 3),
    correct: percentText(last.classification.accuracy),
    auc: fmtNoZero(last.auc, 3),
  });
  const clauses = terms.map((info, i) => {
    const tm = last.terms[i + 1];
    if (!tm) return '';
    const args = { event, or: fmtOdds(tm.odds_ratio), level, ci: ciText(tm.or_ci, 2, true), p: pText(tm.p) };
    return info.dummy
      ? t('The odds of {event} among {label} were {or} times those among {reference} ({level} CI {ci}, {p}).', { ...args, label: valueLabel(info.kind, info.level ?? '', info.label ?? ''), reference: valueLabel(info.kind, info.reference ?? '', info.reference ?? '') })
      : t('Each extra unit of {name} multiplied the odds of {event} by {or} ({level} CI {ci}, {p}).', { ...args, name: variableLabel({ name: info.name, kind: info.kind }) });
  });
  return [first, ...clauses].filter(Boolean).join(' ');
}

function aucWord(auc: number): string {
  return auc < 0.6 ? t('little better than guessing') : auc < 0.7 ? t('poor') : auc < 0.8 ? t('acceptable') : auc < 0.9 ? t('good') : t('excellent');
}

function logisticReading(r: StatResult): string {
  const d = logistic(r);
  const last = d.models?.[d.models.length - 1];
  if (!last) return '';
  const { event } = eventLabels(r);
  const reading = t(
    'An odds ratio above 1 means the odds of {event} rise with the predictor, below 1 that they fall, and 1 means no change. Odds are not probabilities: an odds ratio of 2 doubles the odds, and it doubles the probability only when the outcome is rare. The area under the curve is {auc}, which is {word} at telling the cases with {event} from the others. It shows how the variables go together, not that one causes the other.',
    { event, auc: fmtNoZero(last.auc, 3), word: aucWord(last.auc) },
  );
  return last.lr.p < 0.05 ? reading : `${reading} ${NOT_SIGNIFICANT()}`;
}

// ---------------------------------------------------------------- curve fits

export const CURVE_KINDS: StatCurveKind[] = ['linear', 'logarithmic', 'inverse', 'quadratic', 'cubic', 'power', 'exponential', 's_curve'];

export function curveName(k: StatCurveKind): string {
  switch (k) {
    case 'linear':
      return t('Linear');
    case 'logarithmic':
      return t('Logarithmic');
    case 'inverse':
      return t('Inverse');
    case 'quadratic':
      return t('Quadratic');
    case 'cubic':
      return t('Cubic');
    case 'power':
      return t('Power');
    case 'exponential':
      return t('Exponential');
    default:
      return t('S curve');
  }
}

/** What a curve looks like, in a line, for the person choosing which to fit. */
export function curveHint(k: StatCurveKind): string {
  switch (k) {
    case 'linear':
      return t('A straight line.');
    case 'logarithmic':
      return t('Rises or falls fast, then more slowly. The predictor must be above zero in every case.');
    case 'inverse':
      return t('Levels off toward a floor or a ceiling as the predictor grows. The predictor must not be zero.');
    case 'quadratic':
      return t('One bend: rises then falls, or falls then rises.');
    case 'cubic':
      return t('Up to two bends.');
    case 'power':
      return t('Changes by a constant share for each share the predictor changes. Both numbers must be above zero.');
    case 'exponential':
      return t('Changes by a constant share for each unit of the predictor. The outcome must be above zero.');
    default:
      return t('An S-shaped relation of the logarithm of the outcome to the inverse of the predictor. The outcome must be above zero, and the predictor must not be zero.');
  }
}

function curveErrorWords(e: StatCurveError): string {
  switch (e) {
    case 'needs_positive_x':
      return t('the predictor is not above zero in every case');
    case 'needs_nonzero_x':
      return t('the predictor is zero in some case');
    case 'needs_positive_y':
      return t('the outcome is not above zero in every case');
    case 'too_few_cases':
      return t('there are too few cases');
    case 'too_few_values':
      return t('the predictor has too few different values');
    case 'no_variation_in_outcome':
      return t('the outcome is the same in every case');
    default:
      return t('a value is missing or not a number');
  }
}

/** The curve as an equation in the names of the variables: the form the curve has, with the numbers fitted. */
function curveEquation(f: Extract<StatCurveFitResult, { n: number }>, y: string, x: string): string {
  const b = f.model.terms.map((tm) => tm.estimate);
  const c = f.constants;
  const sign = (v: number) => (v < 0 ? '\u2212' : '+');
  const a = (v: number) => fmt(Math.abs(v), 3);
  switch (f.kind) {
    case 'linear':
      return `${y} = ${fmt(b[0], 3)} ${sign(b[1])} ${a(b[1])} \u00d7 ${x}`;
    case 'logarithmic':
      return `${y} = ${fmt(b[0], 3)} ${sign(b[1])} ${a(b[1])} \u00d7 ln(${x})`;
    case 'inverse':
      return `${y} = ${fmt(b[0], 3)} ${sign(b[1])} ${a(b[1])} / ${x}`;
    case 'quadratic':
      return `${y} = ${fmt(b[0], 3)} ${sign(b[1])} ${a(b[1])} \u00d7 ${x} ${sign(b[2])} ${a(b[2])} \u00d7 ${x}\u00b2`;
    case 'cubic':
      return `${y} = ${fmt(b[0], 3)} ${sign(b[1])} ${a(b[1])} \u00d7 ${x} ${sign(b[2])} ${a(b[2])} \u00d7 ${x}\u00b2 ${sign(b[3])} ${a(b[3])} \u00d7 ${x}\u00b3`;
    case 'power':
      return `${y} = ${fmt(c[0], 3)} \u00d7 ${x}^${fmt(c[1], 3)}`;
    case 'exponential':
      return `${y} = ${fmt(c[0], 3)} \u00d7 e^(${fmt(c[1], 3)} \u00d7 ${x})`;
    default:
      return `${y} = e^(${fmt(b[0], 3)} ${sign(b[1])} ${a(b[1])} / ${x})`;
  }
}

/** The names of a curve's terms, as the coefficients table lists them. */
function curveTerms(kind: StatCurveKind, x: string): string[] {
  switch (kind) {
    case 'logarithmic':
    case 'power':
      return [`ln(${x})`];
    case 'inverse':
    case 's_curve':
      return [`1 / ${x}`];
    case 'quadratic':
      return [x, `${x}\u00b2`];
    case 'cubic':
      return [x, `${x}\u00b2`, `${x}\u00b3`];
    default:
      return [x];
  }
}

const loggedOutcome = (kind: StatCurveKind) => kind === 'power' || kind === 'exponential' || kind === 's_curve';

type FittedCurve = Extract<StatCurveFitResult, { n: number }>;

function fittedCurves(r: StatResult): FittedCurve[] {
  return (r.details.fits ?? []).filter((f): f is FittedCurve => f.error === undefined);
}

function bestCurve(r: StatResult): FittedCurve | undefined {
  return fittedCurves(r).reduce<FittedCurve | undefined>((a, b) => (a === undefined || b.r_squared_original > a.r_squared_original ? b : a), undefined);
}

function curveTables(r: StatResult): ApaTable[] {
  const d = r.details;
  const fits = d.fits ?? [];
  const ok = fittedCurves(r);
  if (ok.length === 0) return [];
  const [y, x] = names(r);
  const level = levelText(d.level ?? 0.95);
  const out: ApaTable[] = [];
  const failed = fits.filter((f) => f.error !== undefined) as Extract<StatCurveFitResult, { error: StatCurveError }>[];
  out.push({
    title: t('Curves fitted to {y} against {x}', { y, x }),
    head: [t('Curve'), t('Fitted on'), t('R\u00b2'), t('Adjusted R\u00b2'), 'F', 'df', 'p', t('R\u00b2 on the scale of {y}', { y }), t('Root mean squared error'), t('Equation')],
    rows: ok.map((f) => [
      curveName(f.kind),
      loggedOutcome(f.kind) ? `ln(${y})` : y,
      fmtNoZero(f.model.r_squared),
      fmtNoZero(f.model.adj_r_squared),
      fmt(f.model.f.f),
      `${fmt(f.model.f.df1, 0)}, ${fmt(f.model.f.df2, 0)}`,
      pCell(f.model.f.p),
      fmtNoZero(f.r_squared_original),
      fmt(f.rmse_original),
      curveEquation(f, y, x),
    ]),
    note:
      [
        t('Each curve is fitted by least squares. The power, exponential, and S curves are fitted to the logarithm of {y}, so the first R\u00b2 of those is on that scale and cannot be compared with the others; the R\u00b2 on the scale of {y}, worked out from what the curve predicts, can.', { y }),
        failed.length > 0 ? t('Not fitted: {list}.', { list: failed.map((f) => `${curveName(f.kind)} (${curveErrorWords(f.error)})`).join('; ') }) : '',
      ]
        .filter(Boolean)
        .join(' '),
  });
  for (const f of ok) {
    const terms = curveTerms(f.kind, x);
    const logged = loggedOutcome(f.kind);
    out.push({
      title: t('Coefficients: {curve}', { curve: curveName(f.kind) }),
      head: [t('Term'), 'B', 'SE', 't', 'p', t('{level} CI of B', { level })],
      rows: f.model.terms.map((tm, i) => [i === 0 ? t('Constant') : (terms[i - 1] ?? String(i)), fmt(tm.estimate), fmt(tm.se), fmt(tm.t), pCell(tm.p), ciText(tm.ci)]),
      note: logged ? t('The constant and the coefficients are on the scale of ln({y}).', { y }) : undefined,
    });
  }
  return out;
}

function curveSentence(r: StatResult): string {
  const best = bestCurve(r);
  if (!best) return '';
  const [y, x] = names(r);
  const ok = fittedCurves(r);
  const linear = ok.find((f) => f.kind === 'linear');
  const first = t('Of the {k} curves fitted to {y} against {x}, the {curve} curve came closest to the cases: R\u00b2 = {r2} on the scale of {y} (root mean squared error {rmse}), F({df1}, {df2}) = {f}, {p}.', {
    k: num(ok.length),
    y,
    x,
    curve: curveName(best.kind).toLowerCase(),
    r2: fmtNoZero(best.r_squared_original),
    rmse: fmt(best.rmse_original),
    df1: fmt(best.model.f.df1, 0),
    df2: fmt(best.model.f.df2, 0),
    f: fmt(best.model.f.f),
    p: pText(best.model.f.p),
  });
  const rest = linear && linear !== best ? t('The straight line gave R\u00b2 = {r2}.', { r2: fmtNoZero(linear.r_squared_original) }) : '';
  return [first, rest].filter(Boolean).join(' ');
}

function curveReading(r: StatResult): string {
  const best = bestCurve(r);
  if (!best) return '';
  const [y, x] = names(r);
  const reading = t('The curve with the highest R\u00b2 is not always the best description. A more flexible curve (quadratic, cubic) fits the cases at least as well as a simpler one on the same scale, and may bend to follow chance: compare the adjusted R\u00b2, look at the picture, and prefer the simpler curve unless the extra bend is clear. A curve describes the cases you have; it says little about values of {x} beyond them, where curves that agree here can part widely. The R\u00b2 to compare across curves is the one on the scale of {y}.', { x, y });
  return best.model.f.p < 0.05 ? reading : `${reading} ${NOT_SIGNIFICANT()}`;
}

// ---------------------------------------------------------------- what the chooser offers

function numeric(v?: StatVariable): boolean {
  return v !== undefined && numericUse(v) !== null;
}
function scale(v?: StatVariable): boolean {
  return v !== undefined && numericUse(v) === 'scale';
}
function ordered(v?: StatVariable): boolean {
  return v !== undefined && usable(v) && v.measure === 'ordinal' && v.numeric && v.categories.length >= 2;
}
/** Whether a variable's values can be counted as categories: names and ranks always can, and a number can when its values repeat (such as a rating, or 0 and 1). */
function countable(v?: StatVariable): boolean {
  return v !== undefined && groupable(v) && (v.measure !== 'scale' || v.valid >= 2 * v.distinct);
}
function sameTwoValues(a?: StatVariable, b?: StatVariable): boolean {
  if (!a || !b || a.categories.length !== 2 || b.categories.length !== 2) return false;
  const left = new Set(a.categories.map((c) => c.value));
  return b.categories.every((c) => left.has(c.value));
}

/** The goals this file adds to the first seven. */
export function extraGoals(): GoalInfo[] {
  return [
    {
      id: 'repeated',
      label: t('Compare three or more numbers on the same cases'),
      description: t('The same people rated or measured several times, or on several items.'),
      roles: [],
      list: { label: t('The measures'), itemLabel: (n) => t('Measure {n}', { n: num(n) }), add: t('Add a measure'), min: 2, accepts: numeric },
    },
    {
      id: 'partial',
      label: t('Relate two numbers, holding others constant'),
      description: t('Do the two still go together once the effect of other numbers is taken out of both?'),
      roles: [
        { id: 'x', label: t('First number'), accepts: (v) => numericUse(v) === 'scale' },
        { id: 'y', label: t('Second number'), accepts: (v) => numericUse(v) === 'scale' },
      ],
      list: { label: t('Hold constant'), itemLabel: (n) => t('Control {n}', { n: num(n) }), add: t('Add a variable to hold constant'), min: 1, accepts: (v) => numericUse(v) === 'scale' },
    },
    {
      id: 'predict_binary',
      label: t('Explain a yes-or-no from other variables'),
      description: t('A logistic regression: how the odds of one of two outcomes change with other variables.'),
      roles: [{ id: 'outcome', label: t('The outcome with two values'), accepts: (v) => usable(v) && v.valid > 0 && v.categories.length === 2 }],
    },
    {
      id: 'curve',
      label: t('Fit a curve to two numbers'),
      description: t('How one number goes with another when a straight line is not the right shape: compare a line with curves that bend, level off, or grow by a constant share.'),
      roles: [
        { id: 'outcome', label: t('The number to explain'), accepts: (v) => numericUse(v) === 'scale' },
        { id: 'predictor', label: t('The number that explains it'), accepts: (v) => numericUse(v) === 'scale' },
      ],
    },
    {
      id: 'two_factors',
      label: t('Compare groups on a number, by two groupings'),
      description: t('Does the number differ by each grouping, and does the difference one makes depend on the other? A two-way analysis of variance.'),
      roles: [
        { id: 'outcome', label: t('The number to compare'), accepts: (v) => numericUse(v) === 'scale' },
        { id: 'first', label: t('The first grouping'), accepts: groupable },
        { id: 'second', label: t('The second grouping'), accepts: groupable },
      ],
    },
    {
      id: 'scale_reliability',
      label: t('Check how well several items measure one thing'),
      description: t("Cronbach's alpha of a scale made of several answers to the same kind of question."),
      roles: [],
      list: { label: t('The items'), itemLabel: (n) => t('Item {n}', { n: num(n) }), add: t('Add an item'), min: 2, accepts: numeric },
    },
  ];
}

/** The tests of this file that are valid for the variables chosen, in the order the chooser lists them after the first set. */
export function extraMethods(goal: Goal, vars: (StatVariable | undefined)[], groupCount: number): Method[] {
  const out: Method[] = [];
  const add = (procedure: Method['procedure'], description: string) => out.push({ procedure, label: extraProcedureName(procedure) ?? '', description });
  switch (goal) {
    case 'describe': {
      const v = vars[0];
      if (!v) break;
      if (scale(v)) {
        add('shapiro_wilk', t('Whether the values could come from a normal distribution. The best single test of normality for most samples.'));
        add('ks_normal', t('The largest gap between the values and a normal curve whose mean and spread you give.'));
      }
      if (numeric(v) || (usable(v) && v.categories.length === 2)) add('runs', t('Whether the order of the cases looks random: a number is cut in two, or a variable has two values.'));
      if (countable(v)) {
        add('binomial', t('An exact test of the share of cases in one category against a share you name.'));
        add('goodness_of_fit', t('Whether the counts in the categories follow shares you name (equal shares unless you say otherwise).'));
      }
      break;
    }
    case 'against_value':
      if (vars[0]) add('sign', t('Counts how many values are above the value and how many below. Uses only direction, so it is the most cautious of the three.'));
      break;
    case 'compare_paired':
      if (vars[0] && vars[1]) add('sign', t('Counts how often the first is higher than the second. Uses only direction, not size.'));
      break;
    case 'compare_groups':
      if (vars[0] && vars[1]) {
        if (groupCount === 2) {
          add('ks_two', t('Whether the whole distributions differ: the middle, the spread, or the shape.'));
          add('median_test', t('Whether the same share of each group is above the pooled median.'));
          add('moses', t('Whether one group is more spread out than a control group.'));
        } else if (groupCount >= 3) {
          add('median_test', t('Whether the same share of each group is above the pooled median.'));
        }
      }
      break;
    case 'association':
      if (vars[0] && vars[1]) {
        if (sameTwoValues(vars[0], vars[1])) add('mcnemar', t('For the same cases answering twice (such as before and after, yes or no): whether the answers changed one way more than the other.'));
        if (ordered(vars[0]) && ordered(vars[1])) add('ordinal_association', t('For two ordered scales: gamma, tau-b, tau-c, and Somers\' d, which measure whether higher on one goes with higher on the other.'));
      }
      break;
    case 'repeated':
      if (groupCount >= 2) add('friedman', t('The ranks of three or more measures on the same cases, with Kendall\'s W for how much the cases agree.'));
      break;
    case 'curve':
      if (vars[0] && vars[1]) add('curve_fit', t('Fits each curve you tick by least squares, and compares them by how close each comes to the cases on the scale the number was measured on.'));
      break;
    case 'predict_binary':
      if (vars[0] && groupCount > 0) add('logistic_regression', t('Fits the odds of one value of the outcome against the other. Categories become 0/1 columns against a reference. Predictors can be entered in steps.'));
      break;
    case 'two_factors':
      if (vars[0] && vars[1] && vars[2] && vars[1] !== vars[2]) add('two_way_anova', t('Tests each grouping and what the two do together on the number, with type III sums of squares. Every combination of the two groupings needs at least one case.'));
      break;
    case 'scale_reliability':
      if (groupCount >= 2) add('cronbach_alpha', t('Cronbach\'s alpha, its interval, and how each item correlates with the rest and what alpha would be without it. Only cases with a value for every item are used.'));
      break;
    case 'partial':
      if (vars[0] && vars[1] && groupCount >= 1) add('partial_correlation', t('Pearson\'s correlation of the two once the others have been taken out of both.'));
      break;
  }
  return out;
}

