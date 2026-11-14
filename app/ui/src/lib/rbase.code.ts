// What the files that write R code share: how a string or a number is written for R, what a page of the notebook is to the code
// (RPage), and the small pieces of R that many pages begin with. Nothing in this file is words for the person, so it is not audited
// for language (see scripts/i18n/ast.mjs).

import type { StatChart, StatOptions, StatProcedure } from '../types';

/** A string as R reads it, with every character outside plain ASCII written as an escape, so the file reads the same on any computer. */
export function q(text: string): string {
  let out = '"';
  for (const ch of text) {
    const c = ch.codePointAt(0) ?? 0;
    if (ch === '\\') out += '\\\\';
    else if (ch === '"') out += '\\"';
    else if (ch === '\n') out += '\\n';
    else if (ch === '\r') out += '\\r';
    else if (ch === '\t') out += '\\t';
    else if (c < 32 || c > 126) out += c > 0xffff ? `\\U{${c.toString(16)}}` : `\\u${c.toString(16).padStart(4, '0')}`;
    else out += ch;
  }
  return `${out}"`;
}

/** A number as R reads it. */
export function rn(n: number): string {
  return Number.isFinite(n) ? String(n) : 'NA';
}

export const vec = (items: string[]) => `c(${items.map(q).join(', ')})`;

/** A variable the analysis uses. */
export interface RVar {
  /** What the request asks for it by (`attr:Name`), which the references of a regression are keyed by. */
  key: string;
  /** The name of its column in the data. */
  label: string;
  /** True when this analysis reads it as numbers (a scale variable, or a ranked one whose values are all numbers). */
  numeric: boolean;
  /** Its values in the order the page uses them (the natural order of the values). */
  categories: string[];
}

/** A chart of the page, with its title and the names of what it is about, in the words of the window (the script only copies them). */
export interface RChart {
  chart: StatChart;
  title: string;
  /** The names of the subjects the chart draws: one, or for a chart of two numbers the one along the bottom and the one up the side. */
  names: string[];
}

/** One number the bootstrap of a whole analysis reports: how the page words it, and whether it is a p-value. */
export interface RBootRow {
  /** Where the engine found the number in its result (`t_test/p`, `models/0/terms/1/estimate`). */
  path: string;
  label: string;
  p: boolean;
  /** For a curve fit, the curve the number is about. */
  key?: string;
}

/** What one page of the notebook asked for, and what the engine settled that the script needs to repeat. */
export interface RPage {
  procedure: StatProcedure;
  /** The variables, in the order of the request. */
  vars: RVar[];
  options: StatOptions;
  /** The values of the groups compared, in order (the groups of the result). */
  groups: string[];
  /** A description of a number or of the counts of categories. */
  kind?: string;
  /** The values of the rows and columns of a table of counts, and the two values of McNemar's table. */
  rowValues?: string[];
  colValues?: string[];
  values?: string[];
  /** The categories the page counted, in order (binomial test, one-sample chi-square, frequencies). */
  counts?: string[];
  /** The values of the two groupings of a two-way analysis of variance, in the order of its cells. */
  levelsA?: string[];
  levelsB?: string[];
  /** For a logistic regression, the value of the outcome that was modelled, and whether profile-likelihood intervals were worked out. */
  event?: string;
  profile?: boolean;
  /** For a curve fit, the curves that were fitted, in order. */
  curveKinds?: string[];
  /** The mean and standard deviation of the curve of the Kolmogorov-Smirnov test, and whether they came from the sample. */
  mean?: number;
  sd?: number;
  estimated?: boolean;
  /** The charts the page drew. */
  charts?: RChart[];
  /** For a bootstrap of the whole analysis: the numbers the page's table gives, in order. */
  bootRows?: RBootRow[];
  /** For a bootstrap of the whole analysis resampled by groups of cases: the variable they share a value of (its column in the data). */
  clusterLabel?: string;
}

export interface Helper {
  needs?: string[];
  code: string;
}

export const ADJUST: Record<string, string> = { none: 'none', bonferroni: 'bonferroni', holm: 'holm', benjamini_hochberg: 'BH' };

export const col = (v: RVar) => `d[[${q(v.label)}]]`;
export const asNum = (v: RVar) => `num(${col(v)})`;
export const asVal = (v: RVar) => `val(${col(v)})`;
export const level = (o: StatOptions) => rn(o.level ?? 0.95);

/** Assigns the vectors and keeps the cases that have every one. */
export function take(parts: [string, string][]): string[] {
  const out = parts.map(([n, e]) => `${n} <- ${e}`);
  if (parts.length === 1) {
    const n = parts[0][0];
    out.push(`${n} <- ${n}[!is.na(${n})]`);
  } else {
    out.push(`ok <- ${parts.map(([n]) => `!is.na(${n})`).join(' & ')}`);
    out.push(parts.map(([n]) => `${n} <- ${n}[ok]`).join('; '));
  }
  return out;
}

/** The outcome and the groups of a comparison: the cases that have both, in the groups compared, and the numbers for each group. */
export function grouped(p: RPage, summary = true): string[] {
  const [y, g] = p.vars;
  const lv = p.groups;
  const out = [
    `y <- ${asNum(y)}; g <- ${asVal(g)}`,
    `ok <- !is.na(y) & !is.na(g) & g %in% ${vec(lv)}`,
    `y <- y[ok]; g <- factor(g[ok], levels = ${vec(lv)})`,
  ];
  if (summary) out.push('print(sapply(split(y, g), function(v) c(n = length(v), mean = mean(v), sd = sd(v), median = median(v))))');
  return out;
}

/**
 * The lines that make a regression's fits from `d`: the cases that have every variable, a category compared with its reference, and a fit for
 * each step in which predictors were entered (`fit1`, `fit2`, ...; `last` names the final one). A logistic regression models the value of the
 * outcome the page chose, as 1 against 0.
 */
export function modelSetup(p: RPage, logistic: boolean, strict = false): { lines: string[]; last: string; count: number; stepList: number[] } {
  const o = p.options;
  const [v0] = p.vars;
  const steps = o.steps ?? p.vars.slice(1).map(() => 1);
  const order = p.vars
    .slice(1)
    .map((v, i) => ({ v, step: steps[i] ?? 1, at: i }))
    .sort((a, b) => a.step - b.step || a.at - b.at);
  const lines: string[] = ['m <- data.frame(row.names = seq_len(nrow(d)))', `m[["y"]] <- ${logistic ? asVal(v0) : asNum(v0)}`];
  const names: { name: string; step: number }[] = [];
  for (const { v, step } of order) {
    lines.push(`m[[${q(v.label)}]] <- ${v.numeric ? asNum(v) : asVal(v)}`);
    names.push({ name: v.label, step });
  }
  lines.push('m <- m[complete.cases(m), , drop = FALSE]');
  // The outcome is 1 for the value modelled and 0 for the other.
  if (logistic) lines.push(`m[["y"]] <- as.numeric(m[["y"]] == ${q(p.event ?? o.event ?? '1')})`);
  // A categorical predictor is compared with a reference: the one chosen, or the first of the values the cases used have.
  for (const { v } of order) {
    if (v.numeric) continue;
    const ref = o.references?.[v.key];
    lines.push(`lv <- intersect(${vec(v.categories)}, unique(m[[${q(v.label)}]])); ref <- ${ref !== undefined ? q(ref) : 'lv[1]'}; if (!ref %in% lv) ref <- lv[1]`);
    lines.push(`m[[${q(v.label)}]] <- factor(m[[${q(v.label)}]], levels = c(ref, setdiff(lv, ref)))`);
  }
  const stepList = [...new Set(names.map((n) => n.step))].sort((a, b) => a - b);
  stepList.forEach((s, i) => {
    const terms = names.filter((n) => n.step <= s).map((n) => `\`${n.name.replace(/\\/g, '\\\\').replace(/`/g, '\\`')}\``);
    // A predictor that the others account for is left out of a fit when what is left of it is under 1e-7 of its size, or, strictly (the bootstrap), 1e-10, as PrismQDA does.
    lines.push(logistic ? `fit${i + 1} <- glm(y ~ ${terms.join(' + ')}, data = m, family = binomial)` : `fit${i + 1} <- lm(y ~ ${terms.join(' + ')}, data = m${strict ? ', tol = 1e-10' : ''})`);
  });
  return { lines, last: `fit${stepList.length}`, count: stepList.length, stepList };
}
