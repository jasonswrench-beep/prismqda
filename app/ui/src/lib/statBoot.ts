// The bootstrap of a whole analysis: the page runs the procedure again on resamples of the cases, and the engine sends the spread of
// every number it reported, by its path in the result. This file chooses the numbers worth a row in the table (the ones the page leads
// with), words them, and tells how the resamples were drawn.

import { num, t, tn } from '../i18n';
import type { StatBootstrapAnalysis, StatBootstrapAnalysisItem, StatCurveFitResult, StatResult } from '../types';
import { curveName } from './statistics2';
import { ciText, fmt, fmtNoZero, levelText, pCell, percentText, termLabel, variableLabel } from './statistics';
import type { ApaTable } from './statistics';

interface Row {
  path: string;
  label: string;
  /** True for a p-value, which is told by the share of resamples below .05 as well. */
  p?: boolean;
  /** True for a number that cannot exceed 1 in size, written without the zero in front (an effect size, a share). */
  small?: boolean;
}

/** The numbers each procedure leads with, in the order the page gives them. */
export function analysisRows(r: StatResult): Row[] {
  const d = r.details;
  const names = r.variables.map((v) => variableLabel(v));
  const row = (path: string, label: string, extra: Partial<Row> = {}): Row => ({ path, label, ...extra });
  switch (r.procedure) {
    case 'describe':
      return d.descriptives ? [row('descriptives/mean', t('Mean')), row('descriptives/median', t('Median')), row('descriptives/sd', t('Standard deviation')), row('descriptives/skewness', t('Skewness')), row('descriptives/kurtosis', t('Excess kurtosis'))] : [];
    case 'one_sample_t':
    case 'independent_t':
    case 'paired_t': {
      const kind = d.t_test?.kind;
      return [
        row('t_test/estimate', kind === 'one_sample' ? t('Mean') : kind === 'paired' ? t('Mean difference') : t('Difference of means')),
        row('t_test/t', 't'),
        row('t_test/p', 'p', { p: true }),
        row('t_test/d', t("Cohen's d")),
        row('t_test/g', t("Hedges' g")),
      ];
    }
    case 'one_way_anova':
      return [row('anova/test/f', 'F'), row('anova/test/p', 'p', { p: true }), row('anova/eta_squared', 'η²', { small: true }), row('anova/omega_squared', 'ω²', { small: true })];
    case 'kruskal_wallis':
      return [row('kruskal_wallis/h', 'H'), row('kruskal_wallis/p', 'p', { p: true }), row('kruskal_wallis/epsilon_squared', 'ε²', { small: true })];
    case 'mann_whitney':
      return [row('mann_whitney/u', 'U'), row('mann_whitney/z', 'z'), row('mann_whitney/p', 'p', { p: true }), row('mann_whitney/rank_biserial', t('Rank-biserial correlation')), row('mann_whitney/probability_of_superiority', t('Probability of superiority'), { small: true })];
    case 'wilcoxon':
      return [row('wilcoxon/v', 'V'), row('wilcoxon/z', 'z'), row('wilcoxon/p', 'p', { p: true }), row('wilcoxon/rank_biserial', t('Rank-biserial correlation'))];
    case 'chi_square':
      return [row('chi_square/chi2', 'χ²'), row('chi_square/p', 'p', { p: true }), row('chi_square/cramers_v', t("Cramér's V"), { small: true })];
    case 'fisher':
      return [row('fisher/odds_ratio', t('Odds ratio')), row('fisher/p', 'p', { p: true })];
    case 'pearson':
    case 'spearman':
    case 'kendall':
      return [row('correlation/r', r.procedure === 'pearson' ? 'r' : r.procedure === 'spearman' ? 'ρ' : 'τ', { small: true }), row('correlation/p', 'p', { p: true })];
    case 'shapiro_wilk':
      return [row('shapiro_wilk/w', 'W', { small: true }), row('shapiro_wilk/p', 'p', { p: true })];
    case 'ks_normal':
    case 'ks_two':
      return [row('ks/d', 'D', { small: true }), row('ks/p', 'p', { p: true })];
    case 'runs':
      return [row('runs/runs', t('Runs')), row('runs/z', 'z'), row('runs/p_exact', t('p (exact)'), { p: true }), row('runs/p_normal', t('p (normal approximation)'), { p: true })];
    case 'binomial':
      return [row('binomial/estimate', t('Share'), { small: true }), row('binomial/p', 'p', { p: true })];
    case 'goodness_of_fit':
      return [row('goodness_of_fit/chi2', 'χ²'), row('goodness_of_fit/p', 'p', { p: true })];
    case 'sign':
      return [row('sign/proportion_positive', t('Share above'), { small: true }), row('sign/p', 'p', { p: true })];
    case 'friedman':
      return [row('friedman/chi2', 'χ²'), row('friedman/p', 'p', { p: true }), row('friedman/kendall_w', t("Kendall's W"), { small: true })];
    case 'median_test':
      return [row('median_test/chi2', 'χ²'), row('median_test/p', 'p', { p: true })];
    case 'moses':
      return [row('moses/p', 'p', { p: true }), row('moses/trimmed_p', t('Trimmed p'), { p: true })];
    case 'mcnemar':
      return [row('mcnemar/chi2_corrected', 'χ²'), row('mcnemar/p_corrected', 'p', { p: true })];
    case 'ordinal_association':
      return [row('ordinal/gamma/estimate', t('Gamma')), row('ordinal/tau_b/estimate', t("Kendall's tau-b")), row('ordinal/tau_b/p', t('p of tau-b'), { p: true })];
    case 'partial_correlation':
      return [row('partial/r', t('Partial r'), { small: true }), row('partial/p', 'p', { p: true })];
    case 'two_way_anova': {
      const [, a, b] = names;
      const of = [a, b, t('{a} \u00d7 {b}', { a, b })];
      return of.flatMap((who, i) => [row(`factorial/effects/${i}/f`, t('F for {name}', { name: who })), row(`factorial/effects/${i}/p`, t('p for {name}', { name: who }), { p: true }), row(`factorial/effects/${i}/partial_eta_squared`, t('ηp² for {name}', { name: who }), { small: true })]);
    }
    case 'cronbach_alpha':
      return [row('alpha/alpha', 'α', { small: true }), row('alpha/standardized_alpha', t('Standardized α'), { small: true }), row('alpha/mean_inter_item_r', t('Mean correlation between items'), { small: true })];
    case 'linear_regression': {
      const models = d.models ?? [];
      const last = models.length - 1;
      if (last < 0) return [];
      const terms = d.terms ?? [];
      const out = [row(`models/${last}/r_squared`, t('R²'), { small: true }), row(`models/${last}/adj_r_squared`, t('Adjusted R²'), { small: true }), row(`models/${last}/f/f`, 'F'), row(`models/${last}/f/p`, t('p of the model'), { p: true })];
      out.push(row(`models/${last}/terms/0/estimate`, t('B for the intercept')));
      terms.forEach((info, i) => {
        out.push(row(`models/${last}/terms/${i + 1}/estimate`, t('B for {term}', { term: termLabel(info) })));
        out.push(row(`models/${last}/terms/${i + 1}/p`, t('p for {term}', { term: termLabel(info) }), { p: true }));
      });
      return out;
    }
    case 'logistic_regression': {
      const models = (d.models ?? []) as unknown[];
      const last = models.length - 1;
      if (last < 0) return [];
      const terms = d.terms ?? [];
      const out = [row(`models/${last}/lr/chi2`, 'χ²'), row(`models/${last}/lr/p`, t('p of the model'), { p: true }), row(`models/${last}/nagelkerke`, t('Nagelkerke R²'), { small: true }), row(`models/${last}/auc`, t('Area under the curve'), { small: true })];
      terms.forEach((info, i) => {
        out.push(row(`models/${last}/terms/${i + 1}/estimate`, t('B for {term}', { term: termLabel(info) })));
        out.push(row(`models/${last}/terms/${i + 1}/odds_ratio`, t('Odds ratio for {term}', { term: termLabel(info) })));
        out.push(row(`models/${last}/terms/${i + 1}/p`, t('p for {term}', { term: termLabel(info) }), { p: true }));
      });
      return out;
    }
    case 'curve_fit': {
      const fits = (d.fits ?? []) as StatCurveFitResult[];
      const y = names[0] ?? '';
      return fits.flatMap((f, i) => (f.error === undefined ? [row(`fits/${i}/r_squared_original`, t('R² on the scale of {y}: {curve}', { y, curve: curveName(f.kind) }), { small: true })] : []));
    }
    default:
      return [];
  }
}

/** Where the cases of each resample came from, in words. */
function resampleWords(b: StatBootstrapAnalysis): string {
  const what = b.cluster
    ? t('the {groups} groups of cases that share a value of {name}, drawn with replacement and kept together', { groups: num(b.cluster.groups), name: b.cluster.name })
    : t('the cases, drawn with replacement');
  return t('{n} resamples of {what}, from seed {seed}.', { n: num(b.resamples), what, seed: String(b.seed) });
}

/** The table of a bootstrap of the whole analysis: the numbers the page leads with, each with the spread it showed across the resamples. */
export function analysisTables(r: StatResult): ApaTable[] {
  const b = r.details.bootstrap_analysis;
  if (!b) return [];
  const byPath = new Map<string, StatBootstrapAnalysisItem>(b.items.map((i) => [i.path, i]));
  const level = levelText(b.level);
  const rows = analysisRows(r)
    .map((row) => ({ row, item: byPath.get(row.path) }))
    .filter((x): x is { row: Row; item: StatBootstrapAnalysisItem } => x.item !== undefined);
  if (rows.length === 0) return [];
  return [
    {
      title: t('Bootstrap of the whole analysis'),
      head: [t('Statistic'), t('Estimate'), t('Bootstrap SE'), t('{level} percentile CI', { level }), t('Resamples with p < .05')],
      rows: rows.map(({ row, item }) => {
        const iv = item.interval;
        if (row.p) return [row.label, pCell(iv.estimate), fmtNoZero(iv.se, 3), `[${pCell(iv.percentile[0])}, ${pCell(iv.percentile[1])}]`, item.below_05 === null ? '—' : percentText(item.below_05)];
        const f = row.small ? (x: number) => fmtNoZero(x) : (x: number) => fmt(x);
        return [row.label, f(iv.estimate), fmt(iv.se, 3), ciText(iv.percentile), '—'];
      }),
      note: [
        resampleWords(b),
        t('Each resample runs the whole analysis again, so the intervals show how much each number moves with the cases drawn, and the share of resamples with p below .05 says how stable the conclusion is. They are percentile intervals, without the correction for bias that the other bootstrap offers.'),
        b.failed > 0 ? tn(b.failed, { one: '{count} resample could not be analyzed (for example, a group was left out of it) and is not counted.', other: '{count} resamples could not be analyzed (for example, a group was left out of them) and are not counted.' }) : '',
      ]
        .filter(Boolean)
        .join(' '),
    },
  ];
}
