// The words and tables of the Statistics page. The engine returns numbers and facts; everything a person reads is made here,
// through the language catalog, so a translator can say it in their language. Tables follow the layout of APA 7 tables
// (no vertical lines, p-values and correlations without a leading zero), and a results sentence is made for each test.

import type {
  StatAssumption,
  StatCategory,
  StatGroup,
  StatNumber,
  StatBootstrapItem,
  StatProcedure,
  StatResult,
  StatTermInfo,
  StatVariable,
  StatVariableKind,
} from '../types';
import { num, t, tn } from '../i18n';
import { analysisTables } from './statBoot';
import { extraGoals, extraMethods, extraProcedureName, extraReading, extraSentence, extraTables, extraTitle } from './statistics2';

// ---------------------------------------------------------------- numbers

/** A number to a fixed count of decimals as the language writes it; null (no value) is a dash. */
export function fmt(x: StatNumber | undefined, digits = 2): string {
  if (x === null || x === undefined || Number.isNaN(x)) return '—';
  if (!Number.isFinite(x)) return x > 0 ? '∞' : '−∞';
  return num(x, { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

/** A value that cannot be larger than 1 in size (r, p, a share), without the zero before the decimal point, as APA writes it. */
export function fmtNoZero(x: StatNumber | undefined, digits = 2): string {
  const s = fmt(x, digits);
  return s.replace(/^([−-]?)0(?=[.,٫])/, '$1');
}

/** An odds ratio or an end of its interval: null is infinity, which is what the engine cannot write as a number. */
export function fmtOdds(x: StatNumber): string {
  return x === null ? '∞' : x >= 1000 ? fmt(x, 0) : fmt(x, 2);
}

/** The p-value as APA writes it: `p = .034`, `p < .001`, `p > .999`. */
export function pText(p: number): string {
  if (!Number.isFinite(p)) return 'p = —';
  if (p < 0.001) return `p < ${fmtNoZero(0.001, 3)}`;
  if (p > 0.999) return `p > ${fmtNoZero(0.999, 3)}`;
  return `p = ${pCell(p)}`;
}

/** A p-value in a table: `< .001` below a thousandth, else three decimals with no zero in front. */
export function pCell(p: number): string {
  return p < 0.001 ? `< ${fmtNoZero(0.001, 3)}` : fmtNoZero(p, 3);
}

/** `[lo, hi]` to the given decimals. */
export function ciText(ci: [StatNumber, StatNumber] | null | undefined, digits = 2, odds = false): string {
  if (!ci) return '—';
  const f = (x: StatNumber) => (odds ? fmtOdds(x) : fmt(x, digits));
  return `[${f(ci[0])}, ${f(ci[1])}]`;
}

/** A confidence level as a percentage: 0.95 is "95%". */
export function levelText(level: number): string {
  return num(level, { style: 'percent', maximumFractionDigits: 1 });
}

/** A share of cases as a percentage with one decimal. */
export function percentText(share: number): string {
  return num(share, { style: 'percent', minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

export type Magnitude = 'negligible' | 'small' | 'medium' | 'large';

/** The usual benchmarks for the size of an effect, which are conventions and not laws: Cohen's for d, r, and the share of variance. */
export function magnitude(kind: 'd' | 'r' | 'share' | 'v', value: number, df = 1): Magnitude {
  const x = Math.abs(value);
  const cut = kind === 'd' ? [0.2, 0.5, 0.8] : kind === 'share' ? [0.01, 0.06, 0.14] : kind === 'v' ? [0.1, 0.3, 0.5].map((c) => c / Math.sqrt(Math.max(1, df))) : [0.1, 0.3, 0.5];
  return x < cut[0] ? 'negligible' : x < cut[1] ? 'small' : x < cut[2] ? 'medium' : 'large';
}

export function magnitudeWord(m: Magnitude): string {
  switch (m) {
    case 'negligible':
      return t('negligible');
    case 'small':
      return t('small');
    case 'medium':
      return t('medium');
    default:
      return t('large');
  }
}

// ---------------------------------------------------------------- variables

/** The name of a variable as a person reads it: a case variable by its name, a code variable by what it counts. */
export function variableLabel(v: { name: string; kind: StatVariableKind }): string {
  if (v.kind === 'code_present') return t('Has the code: {name}', { name: v.name });
  if (v.kind === 'code_count') return t('Excerpts coded: {name}', { name: v.name });
  return v.name;
}

/** A value of a variable, labelled: the value's own label, or for whether a case has a code, Yes and No. */
export function valueLabel(kind: StatVariableKind, value: string, label: string): string {
  if (kind === 'code_present') return value === '1' ? t('Yes') : t('No');
  return label;
}

export function measureWord(m: StatVariable['measure']): string {
  switch (m) {
    case 'nominal':
      return t('nominal');
    case 'ordinal':
      return t('ordinal');
    case 'scale':
      return t('scale');
    case 'date':
      return t('date');
    default:
      return t('text');
  }
}

/** True when a variable can be tested at all (dates and free text cannot). */
export function usable(v: StatVariable): boolean {
  return v.measure === 'nominal' || v.measure === 'ordinal' || v.measure === 'scale';
}

/** How a variable can be read as numbers: 'scale' for averages and rank methods, 'rank' for rank methods only, or null for neither. */
export function numericUse(v: StatVariable): 'scale' | 'rank' | null {
  if (v.measure === 'scale') return v.valid > 0 ? 'scale' : null;
  if (v.measure === 'ordinal' && v.numeric) return 'rank';
  if (v.kind === 'code_present') return 'rank';
  return null;
}

/** True when a variable can sort cases into groups: a few categories among the usable kinds. */
export function groupable(v: StatVariable): boolean {
  return usable(v) && v.categories.length >= 2;
}

// ---------------------------------------------------------------- what to do, and which test is valid

export type Goal = 'describe' | 'compare_groups' | 'two_factors' | 'compare_paired' | 'repeated' | 'scale_reliability' | 'against_value' | 'association' | 'correlate' | 'partial' | 'predict' | 'predict_binary' | 'curve';

export interface Role {
  id: string;
  label: string;
  accepts: (v: StatVariable) => boolean;
}

export interface GoalInfo {
  id: Goal;
  label: string;
  description: string;
  roles: Role[];
  /** A list of variables of any length after the roles (the measures of a repeated comparison, the variables held constant). */
  list?: { label: string; itemLabel: (n: number) => string; add: string; min: number; accepts: (v: StatVariable) => boolean };
}

const anyUsable = (v: StatVariable) => usable(v) && v.valid > 0;
const numericAny = (v: StatVariable) => numericUse(v) !== null;
const categorical = (v: StatVariable) => groupable(v);

export function goals(): GoalInfo[] {
  const base: GoalInfo[] = [
    {
      id: 'describe',
      label: t('Describe one variable'),
      description: t('Counts for categories; mean, spread, and shape for numbers.'),
      roles: [{ id: 'x', label: t('Variable'), accepts: anyUsable }],
    },
    {
      id: 'compare_groups',
      label: t('Compare groups on a number'),
      description: t('Does the number differ between groups of cases?'),
      roles: [
        { id: 'outcome', label: t('The number to compare'), accepts: numericAny },
        { id: 'group', label: t('The groups'), accepts: categorical },
      ],
    },
    {
      id: 'compare_paired',
      label: t('Compare two numbers on the same cases'),
      description: t('A before and an after, or two ratings by the same people.'),
      roles: [
        { id: 'first', label: t('First number'), accepts: numericAny },
        { id: 'second', label: t('Second number'), accepts: numericAny },
      ],
    },
    {
      id: 'against_value',
      label: t('Compare a number with a value'),
      description: t('Is the typical value different from a figure you name?'),
      roles: [{ id: 'x', label: t('The number'), accepts: numericAny }],
    },
    {
      id: 'association',
      label: t('See whether two categories go together'),
      description: t('A table of cases: for example group by site.'),
      roles: [
        { id: 'row', label: t('Down the side'), accepts: categorical },
        { id: 'column', label: t('Across the top'), accepts: categorical },
      ],
    },
    {
      id: 'correlate',
      label: t('Relate two numbers'),
      description: t('Do cases that are high on one tend to be high or low on the other?'),
      roles: [
        { id: 'x', label: t('First number'), accepts: numericAny },
        { id: 'y', label: t('Second number'), accepts: numericAny },
      ],
    },
    {
      id: 'predict',
      label: t('Explain a number from other variables'),
      description: t('A regression: how much of the differences in a number the other variables account for, and what each adds.'),
      roles: [{ id: 'outcome', label: t('The number to explain'), accepts: (v) => numericUse(v) === 'scale' }],
    },
  ];
  // The goals added later sit beside the ones they are kin to: repeated measures after the paired comparison, partial correlation after correlation.
  const [repeated, partial, binary, curve, twoFactors, reliability] = extraGoals();
  const at = (id: Goal) => base.findIndex((g) => g.id === id);
  base.splice(at('compare_groups') + 1, 0, twoFactors);
  base.splice(at('compare_paired') + 1, 0, repeated);
  base.splice(at('repeated') + 1, 0, reliability);
  base.splice(at('correlate') + 1, 0, partial);
  base.splice(at('predict') + 1, 0, binary);
  base.splice(at('predict_binary') + 1, 0, curve);
  return base;
}

/** True for a variable that can be a predictor in a regression: a number, or a categorical variable turned into 0/1 columns. */
export function predictorAccepts(v: StatVariable): boolean {
  return usable(v) && v.valid > 0 && (numericUse(v) === 'scale' || (v.measure === 'ordinal' && v.numeric) || groupable(v));
}

/** True when a predictor is categorical (it becomes a 0/1 column for each category but the reference one). */
export function isCategorical(v: StatVariable): boolean {
  return !(v.measure === 'scale' || (v.measure === 'ordinal' && v.numeric));
}

/** True when a bootstrap interval can be added to a procedure, for the variables chosen. */
export function canBootstrap(procedure: StatProcedure | null, vars: (StatVariable | undefined)[]): boolean {
  switch (procedure) {
    case 'describe':
      return vars[0]?.measure === 'scale';
    case 'one_sample_t':
    case 'paired_t':
    case 'independent_t':
    case 'mann_whitney':
    case 'wilcoxon':
    case 'pearson':
    case 'spearman':
    case 'kendall':
    case 'linear_regression':
    case 'logistic_regression':
      return true;
    default:
      return false;
  }
}

export interface Method {
  procedure: StatProcedure;
  label: string;
  description: string;
}

export function procedureName(p: StatProcedure): string {
  switch (p) {
    case 'describe':
      return t('Description');
    case 'one_sample_t':
      return t('One-sample t test');
    case 'independent_t':
      return t('Independent-samples t test');
    case 'paired_t':
      return t('Paired-samples t test');
    case 'one_way_anova':
      return t('One-way analysis of variance');
    case 'chi_square':
      return t('Chi-square test of independence');
    case 'fisher':
      return t("Fisher's exact test");
    case 'pearson':
      return t("Pearson's correlation");
    case 'spearman':
      return t("Spearman's rank correlation");
    case 'kendall':
      return t("Kendall's tau-b");
    case 'mann_whitney':
      return t('Mann-Whitney U test');
    case 'wilcoxon':
      return t('Wilcoxon signed-rank test');
    case 'linear_regression':
      return t('Linear regression');
    case 'kruskal_wallis':
      return t('Kruskal-Wallis test');
    default:
      return extraProcedureName(p) ?? t('Kruskal-Wallis test');
  }
}

/**
 * The tests that are valid for the variables chosen, and only those. `vars` are in the order of the goal's roles (a role not
 * yet filled is undefined) and `groupCount` is how many groups of the grouping variable are being compared (or, for a regression, how
 * many predictors are chosen).
 */
export function methodsFor(goal: Goal, vars: (StatVariable | undefined)[], groupCount: number): Method[] {
  const scale = (v?: StatVariable) => v !== undefined && numericUse(v) === 'scale';
  const out: Method[] = [];
  const add = (procedure: StatProcedure, description: string) => out.push({ procedure, label: procedureName(procedure), description });
  switch (goal) {
    case 'describe':
      if (vars[0]) add('describe', t('Counts and shares for categories; mean, spread, quartiles, and shape for numbers.'));
      break;
    case 'against_value':
      if (vars[0]) {
        if (scale(vars[0])) add('one_sample_t', t('Compares the mean with the value. Assumes the numbers are roughly symmetric.'));
        add('wilcoxon', t('Compares the middle with the value using ranks. Needs no assumption about the shape.'));
      }
      break;
    case 'compare_groups':
      if (vars[0] && vars[1]) {
        if (groupCount === 2) {
          if (scale(vars[0])) add('independent_t', t('Compares the two means. Welch\'s version, which allows unequal spread, is the default.'));
          add('mann_whitney', t('Compares the two groups using ranks. Needs no assumption about the shape.'));
        } else if (groupCount >= 3) {
          if (scale(vars[0])) add('one_way_anova', t('Compares the means of the groups, then each pair with adjusted p-values.'));
          add('kruskal_wallis', t('Compares the groups using ranks, then each pair (Dunn) with adjusted p-values.'));
        }
      }
      break;
    case 'predict':
      if (vars[0] && groupCount > 0) add('linear_regression', t('Least squares, with an intercept. Categories become 0/1 columns against a reference. Predictors can be entered in steps.'));
      break;
    case 'compare_paired':
      if (vars[0] && vars[1]) {
        if (scale(vars[0]) && scale(vars[1])) add('paired_t', t('Tests whether the average difference between the two is zero.'));
        add('wilcoxon', t('Tests the differences using ranks. Needs no assumption about the shape.'));
      }
      break;
    case 'association':
      if (vars[0] && vars[1]) {
        add('chi_square', t('Compares the counts with what independence would give. Needs enough cases in each cell.'));
        if (vars[0].categories.length === 2 && vars[1].categories.length === 2) add('fisher', t('The exact version for a table of two by two. Safe with small counts.'));
      }
      break;
    case 'correlate':
      if (vars[0] && vars[1]) {
        if (scale(vars[0]) && scale(vars[1])) add('pearson', t('A straight-line relation between two numbers.'));
        add('spearman', t('A relation in the order of the values (ranks). Allows curves that rise or fall steadily.'));
        add('kendall', t('A relation in the order of the values, by pairs of cases. Suits small samples and many ties.'));
      }
      break;
  }
  out.push(...extraMethods(goal, vars, groupCount));
  return out;
}

// ---------------------------------------------------------------- assumptions

export interface AssumptionLine {
  status: StatAssumption['status'];
  /** The word that tells the state, so the state is never only a color. */
  badge: string;
  text: string;
}

export function statusWord(s: StatAssumption['status']): string {
  switch (s) {
    case 'ok':
      return t('Fine');
    case 'caution':
      return t('Note');
    case 'problem':
      return t('Problem');
    default:
      return t('Design');
  }
}

/** One assumption as a sentence. */
export function assumptionLine(a: StatAssumption): AssumptionLine {
  const n = a.numbers;
  const who = a.group ? `${a.group}: ` : '';
  let text: string;
  switch (a.id) {
    case 'order':
      text = t('The runs test reads the cases in the order they have in the project, and it tests that order. It means something only if the order does (such as the order of a series).');
      break;
    case 'independence':
      text = t('Each row is one case. Codings are never rows, so one person is not counted many times.');
      break;
    case 'sample_size':
      text =
        a.status === 'problem'
          ? t('The smallest group has {n}. At least 3 cases are needed.', { n: tn(n.n_min, { one: '{count} case', other: '{count} cases' }) })
          : a.status === 'caution'
            ? t('The smallest group has {n}. With fewer than 15 the result rests on little data, and the shape of the numbers matters more.', { n: tn(n.n_min, { one: '{count} case', other: '{count} cases' }) })
            : t('Every group has at least 15 cases.');
      break;
    case 'normality': {
      const shape = t('skewness {s}, excess kurtosis {k}', { s: fmt(n.skewness), k: fmt(n.kurtosis) });
      text =
        who +
        (a.status === 'problem'
          ? t('Strongly skewed or heavy-tailed ({shape}). Use a rank-based test.', { shape })
          : a.status === 'caution'
            ? t('Noticeably skewed or heavy-tailed ({shape}). A rank-based test is a safer choice.', { shape })
            : t('Close to symmetric ({shape}).', { shape })) +
        (n.w !== undefined && n.shapiro_p !== undefined ? ' ' + t('Shapiro-Wilk test: W = {w}, {p}. With few cases it has little power to notice a departure, and with many it notices very small ones, so read it with the shape and the plot.', { w: fmtNoZero(n.w, 3), p: pText(n.shapiro_p) }) : '');
      break;
    }
    case 'outliers':
      text = who + tn(n.count, { one: '{count} value lies', other: '{count} values lie' }) + ' ' + t('beyond 1.5 interquartile ranges from the quartiles. Look at them before trusting a mean.');
      break;
    case 'equal_variances': {
      const facts = t("Levene's test {p}; the largest standard deviation is {ratio} times the smallest", { p: pText(n.levene_p), ratio: fmt(n.sd_ratio, 1) });
      text = a.status === 'ok' ? t('The spread looks similar ({facts}).', { facts }) : t("The spread differs ({facts}). Welch's test does not assume equal spread.", { facts });
      break;
    }
    case 'expected_counts': {
      const facts = t('the smallest expected count is {min}, and {share} of the cells expect fewer than 5', { min: fmt(n.smallest_expected, 2), share: percentText(n.share_below_5) });
      text =
        a.status === 'problem'
          ? t('The chi-square approximation is unreliable ({facts}). Combine categories, or for two by two use Fisher\'s exact test.', { facts })
          : a.status === 'caution'
            ? t('Some counts are small ({facts}). Read the p-value with care.', { facts })
            : t('The counts are large enough ({facts}).', { facts });
      break;
    }
    case 'cases_per_predictor': {
      const facts = t('{n} cases for {terms} predictor terms', { n: num(n.n), terms: num(n.terms) });
      const per = fmt(n.n / n.terms, 1);
      text =
        a.status === 'problem'
          ? t('Fewer than 5 cases for each predictor term ({per}; {facts}). The estimates will be very unstable.', { per, facts })
          : a.status === 'caution'
            ? t('Only {per} cases for each predictor term ({facts}). A common guide is at least 15, and the estimates will be unstable.', { per, facts })
            : t('{per} cases for each predictor term ({facts}).', { per, facts });
      break;
    }
    case 'collinearity': {
      const v = fmt(n.max_vif, 1);
      text =
        a.status === 'problem'
          ? t('The predictors are highly related to each other (the largest variance inflation factor is {v}, 10 or more). Their separate effects cannot be told apart; leave one out.', { v })
          : a.status === 'caution'
            ? t('Some predictors are related to each other (the largest variance inflation factor is {v}). Their separate effects are uncertain.', { v })
            : t('The predictors are not strongly related to each other (the largest variance inflation factor is {v}).', { v });
      break;
    }
    case 'influence':
      text =
        a.status === 'problem'
          ? t("A case has a Cook's distance of {c}, above 1: the fit depends on it. See the table of influential cases.", { c: fmt(n.max_cooks_distance) })
          : a.status === 'caution'
            ? tn(n.count, { one: "{count} case has a Cook's distance above {cut}.", other: "{count} cases have a Cook's distance above {cut}." }, { cut: fmt(n.cut, 3) }) + ' ' + t('See the table of influential cases.')
            : t("No single case pulls the fit unduly (the largest Cook's distance is {c}).", { c: fmt(n.max_cooks_distance) });
      break;
    case 'large_residuals':
      text = tn(n.count, { one: '{count} case has a residual more than 3 standard errors from its fitted value.', other: '{count} cases have a residual more than 3 standard errors from their fitted value.' }) + ' ' + t('Look at them before trusting the model.');
      break;
    case 'residual_shape': {
      const shape = t('skewness {s}, excess kurtosis {k}', { s: fmt(n.skewness), k: fmt(n.kurtosis) });
      text =
        a.status === 'problem'
          ? t('The residuals are strongly skewed or heavy-tailed ({shape}). The p-values and intervals may be off; a bootstrap interval is safer.', { shape })
          : a.status === 'caution'
            ? t('The residuals are noticeably skewed or heavy-tailed ({shape}). A bootstrap interval is a safer check.', { shape })
            : t('The residuals are close to symmetric ({shape}).', { shape });
      break;
    }
    case 'equal_error_variance':
      text =
        a.status === 'ok'
          ? t('The spread of the errors looks even across the predictors (Breusch-Pagan {p}).', { p: pText(n.p) })
          : t('The spread of the errors changes with the predictors (Breusch-Pagan {p}). The standard errors and p-values may be off; a bootstrap interval is a safer check.', { p: pText(n.p) });
      break;
    case 'events_per_predictor': {
      const facts = t('{events} cases of the rarer outcome for {terms} predictor terms', { events: num(n.events), terms: num(n.terms) });
      const per = fmt(n.events / n.terms, 1);
      text =
        a.status === 'problem'
          ? t('Fewer than 5 cases of the rarer outcome for each predictor term ({per}; {facts}). The estimates will be very unstable, and may be too large.', { per, facts })
          : a.status === 'caution'
            ? t('Only {per} cases of the rarer outcome for each predictor term ({facts}). A common guide is at least 10, and the estimates and their intervals will be unreliable.', { per, facts })
            : t('{per} cases of the rarer outcome for each predictor term ({facts}).', { per, facts });
      break;
    }
    case 'hosmer_lemeshow':
      text =
        a.status === 'ok'
          ? t('The predicted probabilities match the outcomes across the groups of cases (Hosmer-Lemeshow {p}).', { p: pText(n.p) })
          : t('The predicted probabilities do not match the outcomes well in some groups of cases (Hosmer-Lemeshow {p}). The model may be missing a predictor, or a curved or interacting effect.', { p: pText(n.p) });
      break;
    case 'ordinal_as_number':
      text = t('{name} is ordinal, and is used here as a number: its steps are assumed to be equal.', { name: a.group ?? '' });
      break;
    case 'ties':
      text = t('{tied} values are tied, in {groups} groups of equal values. The p-value uses the normal approximation with a correction for ties.', { tied: num(n.tied_values), groups: num(n.groups_of_ties) });
      break;
    default:
      text = a.id;
  }
  return { status: a.status, badge: statusWord(a.status), text };
}

// ---------------------------------------------------------------- tables

export interface ApaTable {
  title: string;
  head: string[];
  rows: string[][];
  note?: string;
}

function variableNames(r: StatResult): string[] {
  return r.variables.map((v) => variableLabel(v));
}

function groupName(r: StatResult, g: StatGroup): string {
  const group = r.variables[1];
  return group ? valueLabel(group.kind, g.value, g.label) : g.label;
}

function groupsTable(r: StatResult, extra: 'ranks' | 'mean'): ApaTable {
  const [outcome] = variableNames(r);
  const rows = r.groups.map((g, i) => {
    const name = groupName(r, g);
    if (extra === 'ranks') {
      const meanRank = r.details.mann_whitney ? (i === 0 ? r.details.mann_whitney.mean_rank_1 : r.details.mann_whitney.mean_rank_2) : r.details.kruskal_wallis?.groups[i]?.mean_rank;
      return [name, num(g.n), fmt(meanRank, 1), fmt(g.median)];
    }
    const sd = r.details.t_test?.groups[i]?.sd ?? r.details.anova?.groups[i]?.sd ?? null;
    return [name, num(g.n), fmt(g.mean), fmt(sd)];
  });
  return {
    title: t('{outcome} by group', { outcome }),
    head: extra === 'ranks' ? [t('Group'), 'n', t('Mean rank'), t('Median')] : [t('Group'), 'n', 'M', 'SD'],
    rows,
  };
}

/** The tables of a result, in APA layout. */
export function resultTables(r: StatResult): ApaTable[] {
  const d = r.details;
  const out: ApaTable[] = [];
  const level = (x?: number) => levelText(x ?? 0.95);
  switch (r.procedure) {
    case 'linear_regression':
      out.push(...regressionTables(r));
      break;
    case 'describe': {
      const [name] = variableNames(r);
      if (d.kind === 'frequencies' && d.categories) {
        out.push({
          title: t('Frequencies of {name}', { name }),
          head: [t('Value'), 'n', '%', t('Valid %')],
          rows: d.categories.map((c) => [valueLabel(r.variables[0].kind, c.value, c.label), num(c.count), percentText(c.percent), percentText(c.valid_percent)]),
          note: t('{valid} cases have a value and {missing} do not.', { valid: num(d.valid ?? 0), missing: num(d.missing ?? 0) }),
        });
      } else if (d.descriptives) {
        const s = d.descriptives;
        out.push({
          title: t('Description of {name}', { name }),
          head: [t('Statistic'), t('Value')],
          rows: [
            [t('Cases with a value'), num(s.n)],
            [t('Cases without'), num(r.cases_total - s.n)],
            ['M', fmt(s.mean)],
            ['SD', fmt(s.sd)],
            [t('Standard error'), fmt(s.se)],
            [t('{level} CI of the mean', { level: level(s.level) }), ciText(s.ci_mean)],
            [t('Minimum'), fmt(s.min)],
            [t('First quartile'), fmt(s.q1)],
            [t('Median'), fmt(s.median)],
            [t('Third quartile'), fmt(s.q3)],
            [t('Maximum'), fmt(s.max)],
            [t('Skewness'), fmt(s.skewness)],
            [t('Excess kurtosis'), fmt(s.kurtosis)],
            [t('Values beyond the fences'), num(s.outliers)],
          ],
        });
      }
      break;
    }
    case 'one_sample_t':
    case 'independent_t':
    case 'paired_t': {
      const x = d.t_test;
      if (!x) break;
      if (r.procedure === 'independent_t') out.push(groupsTable(r, 'mean'));
      const kind = x.kind === 'welch' ? t("Welch's t test") : x.kind === 'student' ? t("Student's t test") : x.kind === 'paired' ? t('Paired t test') : t('One-sample t test');
      out.push({
        title: kind,
        head: ['t', 'df', 'p', x.kind === 'one_sample' ? t('Mean') : t('Mean difference'), t('{level} CI', { level: level(x.level) }), t("Cohen's d"), t('{level} CI of d', { level: level(x.level) })],
        rows: [[fmt(x.t), fmt(x.df), pCell(x.p), fmt(x.estimate), ciText(x.ci), fmt(x.d), ciText(x.d_ci)]],
        note: t("The interval of d is exact (from the noncentral t distribution). Hedges' g, the same measure corrected for small samples, is {g} {ci}.", { g: fmt(x.g), ci: ciText(x.g_ci) }),
      });
      break;
    }
    case 'one_way_anova': {
      const a = d.anova;
      if (!a) break;
      out.push(groupsTable(r, 'mean'));
      out.push({
        title: t('Analysis of variance'),
        head: [t('Source'), 'SS', 'df', 'MS', 'F', 'p', 'η²'],
        rows: [
          [t('Between groups'), fmt(a.ss_between), fmt(a.df_between, 0), fmt(a.ms_between), fmt(a.test.f), pCell(a.test.p), fmtNoZero(a.eta_squared)],
          [t('Within groups'), fmt(a.ss_within), fmt(a.df_within, 0), fmt(a.ms_within), '', '', ''],
          [t('Total'), fmt(a.ss_total), fmt(a.df_between + a.df_within, 0), '', '', '', ''],
        ],
        note: [
          t('ω² (the less biased share of variance explained) is {omega}.', { omega: fmtNoZero(a.omega_squared) }),
          a.welch ? t("Welch's F({df1}, {df2}) = {f}, {p}.", { df1: fmt(a.welch.df1, 0), df2: fmt(a.welch.df2), f: fmt(a.welch.f), p: pText(a.welch.p) }) : '',
        ]
          .filter(Boolean)
          .join(' '),
      });
      if (d.pairwise && d.pairwise.length > 0) out.push(pairsTable(r, d.pairwise, d.adjustment, d.level ?? 0.95, d.posthoc));
      break;
    }
    case 'chi_square':
    case 'fisher': {
      const c = d.chi_square;
      if (!c || !d.table) break;
      const [rowName, colName] = variableNames(r);
      const rowLabels = (d.row_labels ?? []).map((l, i) => valueLabel(r.variables[0].kind, d.row_values?.[i] ?? l, l));
      const colLabels = (d.col_labels ?? []).map((l, i) => valueLabel(r.variables[1].kind, d.col_values?.[i] ?? l, l));
      out.push({
        title: t('{row} by {column}: cases', { row: rowName, column: colName }),
        head: [rowName, ...colLabels, t('Total')],
        rows: [
          ...d.table.map((row, i) => [rowLabels[i], ...row.map((v) => num(v)), num(row.reduce((a, b) => a + b, 0))]),
          [t('Total'), ...colLabels.map((_, j) => num(d.table!.reduce((a, row) => a + row[j], 0))), num(c.n)],
        ],
      });
      if (r.procedure === 'chi_square') {
        out.push({
          title: t('Chi-square test'),
          head: ['χ²', 'df', 'N', 'p', c.phi !== null ? 'φ' : t("Cramér's V"), c.yates ? t('Yates-corrected χ² (p)') : t('G² (p)')],
          rows: [
            [
              fmt(c.chi2),
              fmt(c.df, 0),
              num(c.n),
              pCell(c.p),
              c.phi !== null ? fmtNoZero(Math.abs(c.phi)) : fmtNoZero(c.cramers_v),
              c.yates ? `${fmt(c.yates[0])} (${pCell(c.yates[1])})` : `${fmt(c.g2)} (${pCell(c.g2_p)})`,
            ],
          ],
          note: t("Cramér's V is {v}; the contingency coefficient is {c}.", { v: fmtNoZero(c.cramers_v), c: fmtNoZero(c.contingency_c) }),
        });
        out.push({
          title: t('Adjusted residuals'),
          head: [rowName, ...colLabels],
          rows: c.adjusted_residuals.map((row, i) => [rowLabels[i], ...row.map((v) => fmt(v))]),
          note: t('A residual beyond about ±2 marks a cell with more or fewer cases than independence would give.'),
        });
      } else if (d.fisher) {
        const f = d.fisher;
        out.push({
          title: t("Fisher's exact test"),
          head: ['p', t('Odds ratio'), t('{level} CI', { level: level(f.level) }), t('Sample odds ratio')],
          rows: [[pCell(f.p), fmtOdds(f.odds_ratio), ciText(f.ci, 2, true), fmtOdds(f.odds_ratio_sample)]],
          note: t('The odds ratio is the conditional maximum-likelihood estimate that R reports. An infinite value comes from an empty cell.'),
        });
      }
      break;
    }
    case 'pearson':
    case 'spearman':
    case 'kendall': {
      const c = d.correlation;
      if (!c) break;
      const symbol = c.kind === 'pearson' ? 'r' : c.kind === 'spearman' ? 'ρ' : 'τb';
      out.push({
        title: procedureName(r.procedure),
        head: [symbol, 'n', c.df !== null ? 'df' : '', c.kind === 'kendall' || c.statistic === null ? (c.exact || c.statistic === null ? t('Test') : 'z') : 't', 'p', c.ci ? t('{level} CI', { level: level(c.level) }) : ''].filter((h) => h !== ''),
        rows: [[fmtNoZero(c.r), num(c.n), ...(c.df !== null ? [fmt(c.df, 0)] : []), c.statistic === null ? (c.edgeworth ? t('exact (series)') : t('exact')) : fmt(c.statistic), pCell(c.p), ...(c.ci ? [ciText(c.ci)] : [])]],
        note: c.kind === 'spearman' ? (c.exact ? t('The p-value is exact (fewer than 10 pairs, no ties).') : c.edgeworth ? t("The p-value comes from the series approximation to the exact test that R uses (AS 89), for 10 to 1,290 pairs with no ties.") : t('The p-value is the t approximation (there are ties, or more than 1,290 pairs).')) : c.kind === 'kendall' ? (c.exact ? t('The p-value is exact (fewer than 50 pairs, no ties).') : t('The p-value is the normal approximation with a correction for ties.')) : undefined,
      });
      break;
    }
    case 'mann_whitney': {
      const m = d.mann_whitney;
      if (!m) break;
      out.push(groupsTable(r, 'ranks'));
      out.push({
        title: t('Mann-Whitney U test'),
        head: ['U', m.exact ? t('Test') : 'z', 'p', t('Rank-biserial r'), t('Chance first group is higher')],
        rows: [[fmt(m.u, 1), m.z === null ? t('exact') : fmt(m.z), pCell(m.p), fmt(m.rank_biserial), percentText(m.probability_of_superiority)]],
        note: m.exact ? t('The p-value is exact (fewer than 50 in each group, no ties).') : t('The p-value is the normal approximation with a continuity correction and a correction for ties.'),
      });
      break;
    }
    case 'wilcoxon': {
      const w = d.wilcoxon;
      if (!w) break;
      out.push({
        title: t('Wilcoxon signed-rank test'),
        head: [t('Pairs used'), t('Equal pairs'), 'V', w.exact ? t('Test') : 'z', 'p', t('Rank-biserial r')],
        rows: [[num(w.n), num(w.zeros), fmt(w.v, 1), w.z === null ? t('exact') : fmt(w.z), pCell(w.p), fmt(w.rank_biserial)]],
        note: w.exact ? t('The p-value is exact (fewer than 50 pairs, no ties, no equal pairs).') : t('The p-value is the normal approximation with a continuity correction and a correction for ties. Pairs with no difference are set aside.'),
      });
      break;
    }
    case 'kruskal_wallis': {
      const k = d.kruskal_wallis;
      if (!k) break;
      out.push(groupsTable(r, 'ranks'));
      out.push({
        title: t('Kruskal-Wallis test'),
        head: ['H', 'df', 'p', 'ε²'],
        rows: [[fmt(k.h), fmt(k.df, 0), pCell(k.p), fmtNoZero(k.epsilon_squared)]],
        note: t('ε² is H divided by N − 1, the share of rank variability the groups account for.'),
      });
      if (d.dunn && d.dunn.length > 0) {
        out.push({
          title: t("Dunn's comparisons, p-values adjusted by {method}", { method: adjustmentName(d.adjustment ?? 'holm') }),
          head: [t('Comparison'), t('Rank difference'), 'z', 'p', t('Adjusted p')],
          rows: d.dunn.map((c) => [`${groupName(r, r.groups[c.a])} − ${groupName(r, r.groups[c.b])}`, fmt(c.rank_difference, 1), fmt(c.z), pCell(c.p), pCell(c.p_adjusted)]),
        });
      }
      break;
    }
    default:
      out.push(...extraTables(r));
  }
  if (d.bootstrap) out.push(bootstrapTable(r));
  if (d.bootstrap_analysis) out.push(...analysisTables(r));
  return out;
}

export function adjustmentName(a: string): string {
  switch (a) {
    case 'none':
      return t('no adjustment');
    case 'bonferroni':
      return t('Bonferroni');
    case 'benjamini_hochberg':
      return t('Benjamini-Hochberg');
    default:
      return t('Holm');
  }
}

export function postHocName(m: string): string {
  switch (m) {
    case 'tukey':
      return t("Tukey's honestly significant difference");
    case 'games_howell':
      return t('Games-Howell');
    default:
      return t("Scheffé's test");
  }
}

function pairsTable(r: StatResult, pairs: NonNullable<StatResult['details']['pairwise']>, adjustment: string | undefined, level: number, posthoc?: string | null): ApaTable {
  if (posthoc) {
    // A test made for pairs gives one p-value and one interval for all the pairs together; the unadjusted t p-value is left out.
    const note =
      posthoc === 'tukey'
        ? t('The p-values and the intervals hold for all the pairs together. When the groups differ in size this is the Tukey-Kramer form. It assumes the groups have a similar spread.')
        : posthoc === 'games_howell'
          ? t("Each pair uses its own two variances and Welch's degrees of freedom, so it does not assume equal spread or equal sizes. The p-values and the intervals hold for all the pairs together.")
          : t('The p-values and the intervals hold for every comparison of groups, not only pairs, so they are the widest of these tests.');
    return {
      title: t('Pairs of groups, {method}', { method: postHocName(posthoc) }),
      head: [t('Comparison'), t('Mean difference'), t('{level} CI', { level: levelText(level) }), 't', 'df', 'p'],
      rows: pairs.map((c) => [`${groupName(r, r.groups[c.a])} − ${groupName(r, r.groups[c.b])}`, fmt(c.difference), ciText(c.ci), fmt(c.t), fmt(c.df, posthoc === 'games_howell' ? 1 : 0), pCell(c.p_adjusted)]),
      note,
    };
  }
  return {
    title: t('Pairs of groups, p-values adjusted by {method}', { method: adjustmentName(adjustment ?? 'holm') }),
    head: [t('Comparison'), t('Mean difference'), t('{level} CI', { level: levelText(level) }), 't', 'p', t('Adjusted p')],
    rows: pairs.map((c) => [`${groupName(r, r.groups[c.a])} − ${groupName(r, r.groups[c.b])}`, fmt(c.difference), ciText(c.ci), fmt(c.t), pCell(c.p), pCell(c.p_adjusted)]),
    note: adjustment === 'bonferroni' ? t('The intervals are widened for the number of comparisons (Bonferroni).') : t('The intervals are not widened for the number of comparisons.'),
  };
}

// ---------------------------------------------------------------- sentences

/** "statistically significant" or not, at the .05 level. Nothing is called marginal: a p-value either passes the line or does not. */
function significance(p: number): string {
  return p < 0.05 ? t('statistically significant') : t('not statistically significant');
}

/** The title of an analysis page. */
export function resultTitle(r: StatResult): string {
  const names = variableNames(r);
  const by = (a: string, b: string) => t('{a} by {b}', { a, b });
  switch (r.procedure) {
    case 'linear_regression':
      return `${procedureName(r.procedure)}: ${t('{outcome} from {predictors}', { outcome: names[0], predictors: predictorNames(r) })}`;
    case 'describe':
    case 'one_sample_t':
      return `${procedureName(r.procedure)}: ${names[0]}`;
    case 'paired_t':
    case 'wilcoxon':
      return r.variables.length === 2 ? `${procedureName(r.procedure)}: ${t('{a} and {b}', { a: names[0], b: names[1] })}` : `${procedureName(r.procedure)}: ${names[0]}`;
    case 'chi_square':
    case 'fisher':
    case 'pearson':
    case 'spearman':
    case 'kendall':
      return `${procedureName(r.procedure)}: ${t('{a} and {b}', { a: names[0], b: names[1] })}`;
    case 'independent_t': {
      const kind = r.details.t_test?.kind === 'student' ? t("Student's t test") : t("Welch's t test");
      return `${kind}: ${by(names[0], names[1])}`;
    }
    default:
      return extraTitle(r) ?? `${procedureName(r.procedure)}: ${by(names[0], names[1])}`;
  }
}

function higherOrLower(diff: number): string {
  return diff > 0 ? t('higher') : diff < 0 ? t('lower') : t('the same');
}

/** The results sentence, written as a report would write it. */
export function resultSentence(r: StatResult): string {
  const d = r.details;
  const names = variableNames(r);
  const gn = (i: number) => (r.groups[i] ? groupName(r, r.groups[i]) : '');
  switch (r.procedure) {
    case 'linear_regression':
      return regressionSentence(r);
    case 'describe': {
      if (d.kind === 'frequencies' && d.categories && d.categories.length > 0) {
        const top = d.categories.reduce((a, b) => (b.count > a.count ? b : a));
        return t('{name}: {valid} cases have a value. The commonest is {label}, with {count} cases ({share} of those with a value).', {
          name: names[0],
          valid: num(d.valid ?? 0),
          label: valueLabel(r.variables[0].kind, top.value, top.label),
          count: num(top.count),
          share: percentText(top.valid_percent),
        });
      }
      const s = d.descriptives;
      if (!s || s.mean === null) return t('{name}: there are too few values to describe.', { name: names[0] });
      return t('{name} has a mean of {m} (SD = {sd}) across {n}. The middle half of the cases lie between {q1} and {q3}.', {
        name: names[0],
        m: fmt(s.mean),
        sd: fmt(s.sd),
        n: tn(s.n, { one: '{count} case', other: '{count} cases' }),
        q1: fmt(s.q1),
        q3: fmt(s.q3),
      });
    }
    case 'one_sample_t': {
      const x = d.t_test;
      if (!x) return '';
      const mu = d.mu ?? 0;
      return t('Mean {name} was {m} (SD = {sd}, n = {n}), {where} the value {mu}. The difference was {sig}, t({df}) = {t}, {p}, d = {d}, {level} CI of the mean {ci}.', {
        name: names[0],
        m: fmt(x.estimate),
        sd: fmt(x.groups[0]?.sd),
        n: num(x.groups[0]?.n ?? 0),
        where: x.estimate > mu ? t('above') : x.estimate < mu ? t('below') : t('equal to'),
        mu: fmt(mu),
        sig: significance(x.p),
        df: fmt(x.df, 0),
        t: fmt(x.t),
        p: pText(x.p),
        d: fmt(x.d),
        level: levelText(x.level),
        ci: ciText(x.ci),
      });
    }
    case 'independent_t': {
      const x = d.t_test;
      if (!x || x.groups.length < 2) return '';
      return t('Mean {outcome} was {dir} in {a} (M = {ma}, SD = {sda}, n = {na}) than in {b} (M = {mb}, SD = {sdb}, n = {nb}). The difference was {sig}, {test}({df}) = {t}, {p}, d = {d}, {level} CI of the difference {ci}.', {
        outcome: names[0],
        dir: higherOrLower(x.estimate),
        a: gn(0),
        ma: fmt(x.groups[0].mean),
        sda: fmt(x.groups[0].sd),
        na: num(x.groups[0].n),
        b: gn(1),
        mb: fmt(x.groups[1].mean),
        sdb: fmt(x.groups[1].sd),
        nb: num(x.groups[1].n),
        sig: significance(x.p),
        test: x.kind === 'welch' ? t('Welch t') : 't',
        df: fmt(x.df, x.kind === 'welch' ? 2 : 0),
        t: fmt(x.t),
        p: pText(x.p),
        d: fmt(x.d),
        level: levelText(x.level),
        ci: ciText(x.ci),
      });
    }
    case 'paired_t': {
      const x = d.t_test;
      if (!x) return '';
      return t('The mean difference between {a} and {b} was {diff} (SD = {sd}, n = {n}). It was {sig}, t({df}) = {t}, {p}, d = {d}, {level} CI {ci}.', {
        a: names[0],
        b: names[1],
        diff: fmt(x.estimate),
        sd: fmt(x.groups[0]?.sd),
        n: num(x.groups[0]?.n ?? 0),
        sig: significance(x.p),
        df: fmt(x.df, 0),
        t: fmt(x.t),
        p: pText(x.p),
        d: fmt(x.d),
        level: levelText(x.level),
        ci: ciText(x.ci),
      });
    }
    case 'one_way_anova': {
      const a = d.anova;
      if (!a) return '';
      const main = t('Mean {outcome} {verb} between the {k} groups of {group}: F({df1}, {df2}) = {f}, {p}, η² = {eta}.', {
        outcome: names[0],
        verb: a.test.p < 0.05 ? t('differed significantly') : t('did not differ significantly'),
        k: num(a.groups.length),
        group: names[1],
        df1: fmt(a.df_between, 0),
        df2: fmt(a.df_within, 0),
        f: fmt(a.test.f),
        p: pText(a.test.p),
        eta: fmtNoZero(a.eta_squared),
      });
      if (a.levene && a.levene.p < 0.05 && a.welch) {
        return `${main} ${t("Because the spread differed between groups (Levene's test {lp}), Welch's F({df1}, {df2}) = {f}, {p} is the safer figure.", { lp: pText(a.levene.p), df1: fmt(a.welch.df1, 0), df2: fmt(a.welch.df2), f: fmt(a.welch.f), p: pText(a.welch.p) })}`;
      }
      return main;
    }
    case 'kruskal_wallis': {
      const k = d.kruskal_wallis;
      if (!k) return '';
      return t('The ranks of {outcome} {verb} between the {k} groups of {group}: H({df}) = {h}, {p}, ε² = {eps}.', {
        outcome: names[0],
        verb: k.p < 0.05 ? t('differed significantly') : t('did not differ significantly'),
        k: num(k.groups.length),
        group: names[1],
        df: fmt(k.df, 0),
        h: fmt(k.h),
        p: pText(k.p),
        eps: fmtNoZero(k.epsilon_squared),
      });
    }
    case 'chi_square': {
      const c = d.chi_square;
      if (!c) return '';
      return t('{row} and {column} were {assoc}, χ²({df}, N = {n}) = {chi}, {p}, {v} = {vv}.', {
        row: names[0],
        column: names[1],
        assoc: c.p < 0.05 ? t('statistically associated') : t('not statistically associated'),
        df: fmt(c.df, 0),
        n: num(c.n),
        chi: fmt(c.chi2),
        p: pText(c.p),
        v: c.phi !== null ? 'φ' : t("Cramér's V"),
        vv: c.phi !== null ? fmtNoZero(Math.abs(c.phi)) : fmtNoZero(c.cramers_v),
      });
    }
    case 'fisher': {
      const f = d.fisher;
      if (!f) return '';
      return t("{row} and {column} were {assoc} by Fisher's exact test, {p}. The odds ratio was {or}, {level} CI {ci}.", {
        row: names[0],
        column: names[1],
        assoc: f.p < 0.05 ? t('statistically associated') : t('not statistically associated'),
        p: pText(f.p),
        or: fmtOdds(f.odds_ratio),
        level: levelText(f.level),
        ci: ciText(f.ci, 2, true),
      });
    }
    case 'pearson':
    case 'spearman':
    case 'kendall': {
      const c = d.correlation;
      if (!c) return '';
      const sign = c.r > 0 ? t('positive') : c.r < 0 ? t('negative') : t('no');
      const symbol = c.kind === 'pearson' ? 'r' : c.kind === 'spearman' ? 'ρ' : 'τb';
      const df = c.df !== null ? `(${fmt(c.df, 0)})` : `(n = ${num(c.n)})`;
      const ci = c.ci ? t(', {level} CI {ci}', { level: levelText(c.level), ci: ciText(c.ci) }) : '';
      return t('The {sign} relation between {a} and {b} was {sig}, {symbol}{df} = {r}, {p}{ci}.', {
        sign,
        a: names[0],
        b: names[1],
        sig: significance(c.p),
        symbol,
        df,
        r: fmtNoZero(c.r),
        p: pText(c.p),
        ci,
      });
    }
    case 'mann_whitney': {
      const m = d.mann_whitney;
      if (!m) return '';
      return t('{outcome} ranked {dir} in {a} (mean rank {ra}) than in {b} (mean rank {rb}). The difference was {sig}, U = {u}, {p}, rank-biserial r = {r}.', {
        outcome: names[0],
        dir: higherOrLower(m.mean_rank_1 - m.mean_rank_2),
        a: gn(0),
        ra: fmt(m.mean_rank_1, 1),
        b: gn(1),
        rb: fmt(m.mean_rank_2, 1),
        sig: significance(m.p),
        u: fmt(m.u, 1),
        p: pText(m.p),
        r: fmt(m.rank_biserial),
      });
    }
    case 'wilcoxon': {
      const w = d.wilcoxon;
      if (!w) return '';
      return r.variables.length === 2
        ? t('The signed-rank test of {a} against {b} was {sig}, V = {v}, {p}, n = {n} pairs, matched-pairs rank-biserial r = {r}.', {
            a: names[0],
            b: names[1],
            sig: significance(w.p),
            v: fmt(w.v, 1),
            p: pText(w.p),
            n: num(w.n),
            r: fmt(w.rank_biserial),
          })
        : t('The signed-rank test of {a} against the value {mu} was {sig}, V = {v}, {p}, n = {n}, matched-pairs rank-biserial r = {r}.', {
            a: names[0],
            mu: fmt(d.mu ?? 0),
            sig: significance(w.p),
            v: fmt(w.v, 1),
            p: pText(w.p),
            n: num(w.n),
            r: fmt(w.rank_biserial),
          });
    }
  }
  return extraSentence(r);
}

/** A plain-language reading in a sentence or two: what the numbers say, how large the effect is, and what not to say. */
export function resultReading(r: StatResult): string {
  const d = r.details;
  const names = variableNames(r);
  const gn = (i: number) => (r.groups[i] ? groupName(r, r.groups[i]) : '');
  const caution = (p: number) =>
    p < 0.05 ? '' : ` ${t('This does not show a difference that can be told from chance. Report it as not statistically significant, with the size of the effect and its interval, and do not describe it as marginal.')}`;
  switch (r.procedure) {
    case 'linear_regression':
      return regressionReading(r);
    case 'independent_t': {
      const x = d.t_test;
      if (!x) return '';
      const m = magnitudeWord(magnitude('d', x.d));
      return t('On average {outcome} is {amount} {dir} in {a} than in {b}. By the usual benchmarks that is a {size} difference (d = {d}).', {
        outcome: names[0],
        amount: fmt(Math.abs(x.estimate)),
        dir: x.estimate >= 0 ? t('higher') : t('lower'),
        a: gn(0),
        b: gn(1),
        size: m,
        d: fmt(x.d),
      }) + caution(x.p);
    }
    case 'one_sample_t': {
      const x = d.t_test;
      if (!x) return '';
      return t('The average {name} is {amount} {dir} than {mu}. By the usual benchmarks that is a {size} difference (d = {d}).', { name: names[0], amount: fmt(Math.abs(x.estimate - (d.mu ?? 0))), dir: x.estimate >= (d.mu ?? 0) ? t('higher') : t('lower'), mu: fmt(d.mu ?? 0), size: magnitudeWord(magnitude('d', x.d)), d: fmt(x.d) }) + caution(x.p);
    }
    case 'paired_t': {
      const x = d.t_test;
      if (!x) return '';
      return t('On average {a} is {amount} {dir} than {b} for the same case. By the usual benchmarks that is a {size} difference (d = {d}).', { a: names[0], b: names[1], amount: fmt(Math.abs(x.estimate)), dir: x.estimate >= 0 ? t('higher') : t('lower'), size: magnitudeWord(magnitude('d', x.d)), d: fmt(x.d) }) + caution(x.p);
    }
    case 'one_way_anova': {
      const a = d.anova;
      if (!a) return '';
      return t('The groups account for {share} of the variation in {outcome}, which is a {size} effect by the usual benchmarks (η² = {eta}).', { share: percentText(a.eta_squared), outcome: names[0], size: magnitudeWord(magnitude('share', a.eta_squared)), eta: fmtNoZero(a.eta_squared) }) + caution(a.test.p);
    }
    case 'kruskal_wallis': {
      const k = d.kruskal_wallis;
      if (!k) return '';
      return t('The groups account for about {share} of the variability in the ranks of {outcome}, a {size} effect by the usual benchmarks (ε² = {eps}).', { share: percentText(k.epsilon_squared), outcome: names[0], size: magnitudeWord(magnitude('share', k.epsilon_squared)), eps: fmtNoZero(k.epsilon_squared) }) + caution(k.p);
    }
    case 'mann_whitney': {
      const m = d.mann_whitney;
      if (!m) return '';
      return t('If you pick one case from {a} and one from {b}, the one from {a} has the higher {outcome} about {share} of the time (ties count half). By the usual benchmarks that is a {size} effect (rank-biserial r = {r}).', { a: gn(0), b: gn(1), outcome: names[0], share: percentText(m.probability_of_superiority), size: magnitudeWord(magnitude('r', m.rank_biserial)), r: fmt(m.rank_biserial) }) + caution(m.p);
    }
    case 'wilcoxon': {
      const w = d.wilcoxon;
      if (!w) return '';
      return t('The differences lean {dir}. By the usual benchmarks that is a {size} effect (matched-pairs rank-biserial r = {r}).', { dir: w.rank_biserial >= 0 ? t('upward') : t('downward'), size: magnitudeWord(magnitude('r', w.rank_biserial)), r: fmt(w.rank_biserial) }) + caution(w.p);
    }
    case 'chi_square': {
      const c = d.chi_square;
      if (!c) return '';
      const v = c.phi !== null ? Math.abs(c.phi) : c.cramers_v;
      return t('How strongly the two go together: {size} by the usual benchmarks ({name} = {v}). Look at the adjusted residuals to see which cells have more or fewer cases than independence would give.', {
        size: magnitudeWord(magnitude('v', v, Math.min(c.rows, c.cols) - 1)),
        name: c.phi !== null ? 'φ' : t("Cramér's V"),
        v: fmtNoZero(v),
      }) + caution(c.p);
    }
    case 'fisher': {
      const f = d.fisher;
      if (!f) return '';
      return t('An odds ratio of 1 means no relation. Here it is {or}, and the interval {ci} shows how far the data allow it to be from 1.', { or: fmtOdds(f.odds_ratio), ci: ciText(f.ci, 2, true) }) + caution(f.p);
    }
    case 'pearson':
    case 'spearman':
    case 'kendall': {
      const c = d.correlation;
      if (!c) return '';
      return t('A {size} {sign} relation: {share} of the variation in the ranks or values is shared (r squared = {r2}). A relation is not a cause.', {
        size: magnitudeWord(magnitude('r', c.r)),
        sign: c.r >= 0 ? t('positive') : t('negative'),
        share: percentText(c.r * c.r),
        r2: fmtNoZero(c.r * c.r),
      }) + caution(c.p);
    }
    default:
      return extraReading(r);
  }
}

/** The plain facts about which cases were used. */
export function usedLine(r: StatResult): string {
  const used = tn(r.n_used, { one: '{count} case was used', other: '{count} cases were used' });
  return r.n_excluded > 0
    ? t('{used} of {total}; {excluded} had a missing value in a variable and were set aside.', { used, total: num(r.cases_total), excluded: num(r.n_excluded) })
    : t('{used} of {total}.', { used, total: num(r.cases_total) });
}

/**
 * What a page says when it was run on some of the cases and not on every case (the cases of the working set at the time). A page kept in the
 * notebook keeps those cases on purpose, so changing or clearing the working set later does not change the page; the sentence says so.
 */
export function chosenCasesLine(options: { case_ids?: number[] } | undefined, forExport = false): string | null {
  const ids = options?.case_ids;
  if (!ids) return null;
  const cases = tn(ids.length, { one: '{count} case', other: '{count} cases' });
  return forExport
    ? t('Run on {cases} chosen in the working set when this page was made, not on every case.', { cases })
    : t('Run on {cases} chosen when it was made (the working set), not on every case. Changing the working set later does not change this page.', { cases });
}

/** The categories of a variable as groups to pick from, labelled. */
export function categoryOptions(v: StatVariable): { value: string; label: string }[] {
  return v.categories.map((c: StatCategory) => ({ value: c.value, label: `${valueLabel(v.kind, c.value, c.label)} (${num(c.count)})` }));
}

// ---------------------------------------------------------------- regression and the bootstrap

/** What a coefficient stands for: a predictor by its name, or one category of a categorical predictor against its reference. */
export function termLabel(info: StatTermInfo): string {
  const base = variableLabel({ name: info.name, kind: info.kind });
  if (!info.dummy) return base;
  const level = valueLabel(info.kind, info.level ?? '', info.label ?? '');
  const reference = valueLabel(info.kind, info.reference ?? '', info.reference ?? '');
  return t('{variable}: {level} (compared with {reference})', { variable: base, level, reference });
}

/** The predictors of a regression by name, once each, as a list. */
function predictorNames(r: StatResult): string {
  return r.variables
    .slice(1)
    .map((v) => variableLabel(v))
    .join(', ');
}

function regressionTables(r: StatResult): ApaTable[] {
  const d = r.details;
  const models = d.models ?? [];
  const terms = d.terms ?? [];
  if (models.length === 0) return [];
  const [outcome] = variableNames(r);
  const level = levelText(models[0].level);
  const out: ApaTable[] = [];
  out.push({
    title: t('Model summary for {outcome}', { outcome }),
    head: [t('Model'), t('R\u00b2'), t('Adjusted R\u00b2'), 'F', 'df', 'p', t('\u0394R\u00b2'), t('F change'), t('p of the change')],
    rows: models.map((m, i) => {
      const change = i > 0 ? d.changes?.[i - 1] : null;
      return [
        num(i + 1),
        fmtNoZero(m.r_squared),
        fmtNoZero(m.adj_r_squared),
        fmt(m.f.f),
        `${fmt(m.f.df1, 0)}, ${fmt(m.f.df2, 0)}`,
        pCell(m.f.p),
        change ? fmtNoZero(change.r2_change) : '',
        change ? fmt(change.f) : '',
        change ? pCell(change.p) : '',
      ];
    }),
    note: t('The residual standard error of the last model is {sigma}, from {n} cases.', { sigma: fmt(models[models.length - 1].sigma), n: num(models[models.length - 1].n) }),
  });
  models.forEach((m, mi) => {
    const rows: string[][] = [[t('Intercept'), fmt(m.terms[0].estimate), fmt(m.terms[0].se), '', fmt(m.terms[0].t), pCell(m.terms[0].p), ciText(m.terms[0].ci), '']];
    m.terms.slice(1).forEach((tm, i) => {
      rows.push([terms[i] ? termLabel(terms[i]) : String(i + 1), fmt(tm.estimate), fmt(tm.se), fmt(tm.beta), fmt(tm.t), pCell(tm.p), ciText(tm.ci), fmt(tm.vif, 2)]);
    });
    out.push({
      title: models.length > 1 ? t('Coefficients, model {n}', { n: num(mi + 1) }) : t('Coefficients'),
      head: [t('Predictor'), 'B', 'SE', '\u03b2', 't', 'p', t('{level} CI of B', { level }), 'VIF'],
      rows,
      note: mi === models.length - 1 ? t('B is the difference in {outcome} that goes with one more unit of the predictor, or with the category against its reference, while the other predictors are held fixed. \u03b2 is the same on standardized scales. VIF is the variance inflation factor.', { outcome }) : undefined,
    });
  });
  if (d.influential && d.influential.length > 0) {
    out.push({
      title: t('Cases that pull the fit most'),
      head: [t('Case'), t('Leverage'), t("Cook's distance"), t('Studentized residual'), t('Residual')],
      rows: d.influential.map((c) => [c.case_name, fmtNoZero(c.leverage, 3), fmt(c.cooks_distance, 3), fmt(c.studentized_residual), fmt(c.residual)]),
      note: t("Cases with a Cook's distance above 4 divided by the number of cases, a residual beyond 3, or a leverage above twice the average. Leaving one out would change the fit by about its Cook's distance."),
    });
  }
  return out;
}

function direction(x: number): string {
  return x >= 0 ? t('higher') : t('lower');
}

function regressionSentence(r: StatResult): string {
  const d = r.details;
  const models = d.models ?? [];
  const terms = d.terms ?? [];
  const last = models[models.length - 1];
  if (!last) return '';
  const [outcome] = variableNames(r);
  const level = levelText(last.level);
  const first = t('The model with {predictors} explained {share} of the variance in {outcome} (adjusted R\u00b2 = {adj}), F({df1}, {df2}) = {f}, {p}.', {
    predictors: predictorNames(r),
    share: percentText(last.r_squared),
    outcome,
    adj: fmtNoZero(last.adj_r_squared),
    df1: fmt(last.f.df1, 0),
    df2: fmt(last.f.df2, 0),
    f: fmt(last.f.f),
    p: pText(last.f.p),
  });
  const clauses = terms.map((info, i) => {
    const tm = last.terms[i + 1];
    if (!tm) return '';
    const args = { amount: fmt(Math.abs(tm.estimate)), dir: direction(tm.estimate), outcome, b: fmt(tm.estimate), level, ci: ciText(tm.ci), p: pText(tm.p) };
    return info.dummy
      ? t('{label} had {amount} {dir} {outcome} than {reference} (B = {b}, {level} CI {ci}, {p}).', { ...args, label: valueLabel(info.kind, info.level ?? '', info.label ?? ''), reference: valueLabel(info.kind, info.reference ?? '', info.reference ?? '') })
      : t('Each extra unit of {name} went with {amount} {dir} {outcome} (B = {b}, {level} CI {ci}, {p}).', { ...args, name: variableLabel({ name: info.name, kind: info.kind }) });
  });
  return [first, ...clauses].filter(Boolean).join(' ');
}

function regressionReading(r: StatResult): string {
  const models = r.details.models ?? [];
  const last = models[models.length - 1];
  if (!last) return '';
  const [outcome] = variableNames(r);
  const reading = t('The predictors together account for about {share} of the variation in {outcome}, a {size} effect by the usual benchmarks (R\u00b2 = {r2}). Each coefficient is the difference that goes with one more unit of that predictor while the others are held fixed. It shows how the numbers go together, not that one causes the other.', {
    share: percentText(last.r_squared),
    outcome,
    size: magnitudeWord(magnitude('share', last.r_squared / Math.max(1e-9, 1 - last.r_squared))),
    r2: fmtNoZero(last.r_squared),
  });
  return last.f.p < 0.05
    ? reading
    : `${reading} ${t('The model as a whole does not explain more than chance would, by the usual line. Report it as not statistically significant, with the size of the effect and its interval, and do not describe it as marginal.')}`;
}

/** The name of a bootstrapped statistic: what it is, and for a coefficient, which one. */
function bootstrapStatName(item: StatBootstrapItem, r: StatResult): string {
  switch (item.statistic) {
    case 'mean':
      return t('Mean');
    case 'median':
      return t('Median');
    case 'sd':
      return t('Standard deviation');
    case 'mean_difference':
      return t('Difference of means');
    case 'median_difference':
      return t('Difference of medians');
    case 'correlation':
      return t('Correlation');
    case 'r_squared':
      return t('R\u00b2');
    case 'auc':
      return t('Area under the curve');
    default: {
      const i = item.target ?? 0;
      const info = i > 0 ? r.details.terms?.[i - 1] : undefined;
      return info ? t('B for {term}', { term: termLabel(info) }) : t('Intercept');
    }
  }
}

/** The bootstrap intervals of a result, as a table. */
export function bootstrapTable(r: StatResult): ApaTable {
  const b = r.details.bootstrap!;
  const level = levelText(b.level);
  const failed = b.items.reduce((m, i) => Math.max(m, i.interval?.failures ?? 0), 0);
  return {
    title: t('Bootstrap intervals'),
    head: [t('Statistic'), t('Estimate'), t('Bootstrap SE'), t('{level} percentile CI', { level }), t('{level} BCa CI', { level })],
    rows: b.items.map((item) => {
      const iv = item.interval;
      const name = bootstrapStatName(item, r);
      return iv ? [name, fmt(iv.estimate), fmt(iv.se), ciText(iv.percentile), iv.bca ? ciText(iv.bca) : '\u2014'] : [name, '\u2014', '\u2014', t('could not be computed'), '\u2014'];
    }),
    note: [
      t('{n} resamples of the cases, from seed {seed}; the same seed gives the same intervals. The BCa interval corrects the percentile interval for bias and skew, and is not given when groups are resampled separately.', { n: num(b.resamples), seed: String(b.seed) }),
      failed > 0 ? tn(failed, { one: '{count} resample gave no value and was left out.', other: '{count} resamples gave no value and were left out.' }) : '',
    ]
      .filter(Boolean)
      .join(' '),
  };
}
