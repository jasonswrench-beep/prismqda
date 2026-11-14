// The arithmetic and the words of the Statistics page's charts: scales and ticks, the title and the text alternative of each chart,
// and the table of what is drawn (which stands beside every chart, so that nothing is told by a picture alone).

import type { StatChart, StatResult, StatSubject } from '../types';
import { num, t, tn } from '../i18n';
import { fmt, fmtNoZero, percentText, valueLabel, variableLabel } from './statistics';
import { curveName } from './statistics2';

// ---------------------------------------------------------------- scales

/** A step of 1, 2, 2.5, or 5 times a power of ten, at least as big as `raw`. */
function niceStep(raw: number): number {
  if (!(raw > 0) || !Number.isFinite(raw)) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const r = raw / magnitude;
  const m = r <= 1 ? 1 : r <= 2 ? 2 : r <= 2.5 ? 2.5 : r <= 5 ? 5 : 10;
  return m * magnitude;
}

/** Round-number ticks that cover `min` to `max`, with the domain stretched to the first and last of them. */
export function niceTicks(min: number, max: number, count = 5): { ticks: number[]; lo: number; hi: number } {
  let a = min;
  let b = max;
  if (!Number.isFinite(a) || !Number.isFinite(b)) return { ticks: [0, 1], lo: 0, hi: 1 };
  if (a === b) {
    const pad = a === 0 ? 1 : Math.abs(a) * 0.1;
    a -= pad;
    b += pad;
  }
  const step = niceStep((b - a) / Math.max(1, count - 1));
  const lo = Math.floor(a / step + 1e-9) * step;
  const hi = Math.ceil(b / step - 1e-9) * step;
  const ticks: number[] = [];
  for (let v = lo; v <= hi + step * 1e-9; v += step) ticks.push(Number(v.toPrecision(12)));
  return { ticks, lo: ticks[0], hi: ticks[ticks.length - 1] };
}

/** Whole-number ticks from 0 to at least `max` (for counts). */
export function countTicks(max: number): { ticks: number[]; hi: number } {
  const top = Math.max(1, Math.ceil(max));
  const step = Math.max(1, niceStep(top / 4));
  const ticks: number[] = [];
  for (let v = 0; v <= top + 1e-9; v += step) ticks.push(v);
  if (ticks[ticks.length - 1] < top) ticks.push(ticks[ticks.length - 1] + step);
  return { ticks, hi: ticks[ticks.length - 1] };
}

/** A function that maps a value of the domain onto the range. */
export function scale(domain: [number, number], range: [number, number]): (v: number) => number {
  const [d0, d1] = domain;
  const [r0, r1] = range;
  const span = d1 - d0 || 1;
  return (v) => r0 + ((v - d0) / span) * (r1 - r0);
}

/** A number on an axis, as short as it can be. */
export function axisNumber(v: number): string {
  return num(v, { maximumFractionDigits: 6 });
}

/** Shortens a long label for an axis; the whole label stays in the tooltip and the table. */
export function shorten(text: string, max: number): string {
  return text.length <= max ? text : `${text.slice(0, Math.max(1, max - 1))}…`;
}

/** The share of each row that each column holds, row by row (a row with no cases is all zeros). */
export function rowShares(counts: number[][]): number[][] {
  return counts.map((row) => {
    const total = row.reduce((a, b) => a + b, 0);
    return row.map((v) => (total > 0 ? v / total : 0));
  });
}

// ---------------------------------------------------------------- words

/** What a chart is about, as a person reads it. */
export function subjectName(s: StatSubject, r: StatResult): string {
  switch (s.type) {
    case 'variable': {
      const v = r.variables[s.index];
      return v ? variableLabel(v) : '';
    }
    case 'differences':
      return t('the differences');
    case 'residuals':
      return t('the residuals');
    default:
      return t('the fitted values');
  }
}

/** A value of the grouping variable, written as the page writes it. */
function groupLabel(r: StatResult, by: number, value: string, label: string): string {
  const v = r.variables[by];
  return v ? valueLabel(v.kind, value, label) : label;
}

export function chartTitle(c: StatChart, r: StatResult): string {
  switch (c.kind) {
    case 'histogram':
      return t('Histogram of {name}', { name: subjectName(c.subject, r) });
    case 'qq':
      return t('Normal probability plot of {name}', { name: subjectName(c.subject, r) });
    case 'bars':
      return t('Counts of {name}', { name: subjectName(c.subject, r) });
    case 'box':
      return t('Box plot of {name} by {by}', { name: subjectName(c.subject, r), by: subjectName({ type: 'variable', index: c.by }, r) });
    case 'means':
      return t('Mean of {name} by {by}, with {level} intervals', { name: subjectName(c.subject, r), by: subjectName({ type: 'variable', index: c.by }, r), level: percentText(c.level).replace('.0', '') });
    case 'stacked':
      return t('{col} within each {row}', { col: subjectName({ type: 'variable', index: c.col }, r), row: subjectName({ type: 'variable', index: c.row }, r) });
    default:
      return t('{y} against {x}', { y: subjectName(c.y, r), x: subjectName(c.x, r) });
  }
}

/** The text alternative: what the chart shows, in a few sentences. */
export function chartDescription(c: StatChart, r: StatResult): string {
  switch (c.kind) {
    case 'histogram': {
      const top = c.bins.reduce((a, b) => (b.count > a.count ? b : a));
      const base = t('{n} in {k}, from {min} to {max}. The tallest bar, from {lo} to {hi}, holds {count}. The mean is {mean}.', {
        n: tn(c.n, { one: '{count} case', other: '{count} cases' }),
        k: tn(c.bins.length, { one: '{count} bar', other: '{count} bars' }),
        min: axisNumber(c.bins[0].lo),
        max: axisNumber(c.bins[c.bins.length - 1].hi),
        lo: axisNumber(top.lo),
        hi: axisNumber(top.hi),
        count: tn(top.count, { one: '{count} case', other: '{count} cases' }),
        mean: fmt(c.mean),
      });
      return c.reference === null ? base : `${base} ${t('A dashed line marks {value}.', { value: fmt(c.reference) })}`;
    }
    case 'qq': {
      const n = c.points.length;
      const mt = c.points.reduce((a, p) => a + p.theoretical, 0) / n;
      const ms = c.points.reduce((a, p) => a + p.sample, 0) / n;
      let sxy = 0;
      let sxx = 0;
      let syy = 0;
      for (const p of c.points) {
        sxy += (p.theoretical - mt) * (p.sample - ms);
        sxx += (p.theoretical - mt) ** 2;
        syy += (p.sample - ms) ** 2;
      }
      const r2 = sxx > 0 && syy > 0 ? sxy / Math.sqrt(sxx * syy) : 1;
      return t('{n} points, one for each value, against where each would sit if the values were normal. The closer the points lie to the line, the closer the values are to a normal distribution. The points and the line have a correlation of {r}.', {
        n: num(n),
        r: fmtNoZero(r2, 3),
      });
    }
    case 'bars': {
      const top = c.categories.reduce((a, b) => (b.count > a.count ? b : a));
      return t('The commonest is {label}, with {count} of {valid} cases.', { label: groupLabel(r, c.subject.type === 'variable' ? c.subject.index : 0, top.value, top.label), count: num(top.count), valid: num(c.valid) });
    }
    case 'box': {
      const hi = c.groups.reduce((a, b) => (b.median > a.median ? b : a));
      const lo = c.groups.reduce((a, b) => (b.median < a.median ? b : a));
      const outliers = c.groups.reduce((a, g) => a + g.outliers.length, 0);
      const base = t('{k} groups. The box holds the middle half of each group, with the median as a bar across it. The highest median is {hl}, {hm}, and the lowest is {ll}, {lm}.', {
        k: num(c.groups.length),
        hl: groupLabel(r, c.by, hi.value, hi.label),
        hm: fmt(hi.median),
        ll: groupLabel(r, c.by, lo.value, lo.label),
        lm: fmt(lo.median),
      });
      return outliers > 0 ? `${base} ${tn(outliers, { one: '{count} value lies beyond the whiskers.', other: '{count} values lie beyond the whiskers.' })}` : base;
    }
    case 'means': {
      const hi = c.groups.reduce((a, b) => (b.mean > a.mean ? b : a));
      const lo = c.groups.reduce((a, b) => (b.mean < a.mean ? b : a));
      return t('{k} groups, each with the mean and the interval of that mean. The highest mean is {hl}, {hm}, and the lowest is {ll}, {lm}. Intervals that do not overlap point to a difference; intervals that overlap do not always rule one out.', {
        k: num(c.groups.length),
        hl: groupLabel(r, c.by, hi.value, hi.label),
        hm: fmt(hi.mean),
        ll: groupLabel(r, c.by, lo.value, lo.label),
        lm: fmt(lo.mean),
      });
    }
    case 'stacked': {
      const shares = rowShares(c.counts);
      let best = { row: 0, col: 0, share: -1 };
      shares.forEach((row, i) => row.forEach((s, j) => s > best.share && (best = { row: i, col: j, share: s })));
      const rv = r.variables[c.row];
      const cv = r.variables[c.col];
      return t('Each bar is one {row}, split into the share of its cases in each {col}. The largest share is {col2} in {row2}: {share}.', {
        row: rv ? variableLabel(rv) : '',
        col: cv ? variableLabel(cv) : '',
        col2: cv ? valueLabel(cv.kind, c.col_values[best.col], c.col_labels[best.col]) : c.col_labels[best.col],
        row2: rv ? valueLabel(rv.kind, c.row_values[best.row], c.row_labels[best.row]) : c.row_labels[best.row],
        share: percentText(Math.max(0, best.share)),
      });
    }
    case 'curves': {
      const n = c.points.length + c.omitted;
      const left = c.omitted > 0 ? ` ${t('{count} are not drawn, to keep the picture readable; the numbers and the tests use them all.', { count: num(c.omitted) })}` : '';
      return (
        t('{n} cases, one point each, with {k} fitted: {list}. Each curve has its own color and line style, and the table beside the chart gives the value of each at every case.', {
          n: num(n),
          k: tn(c.curves.length, { one: '{count} curve', other: '{count} curves' }),
          list: c.curves.map((cv) => curveName(cv.kind)).join(', '),
        }) + left
      );
    }
    default: {
      const n = c.points.length + c.omitted;
      const left = c.omitted > 0 ? ` ${t('{count} are not drawn, to keep the picture readable; the numbers and the tests use them all.', { count: num(c.omitted) })}` : '';
      if (c.y.type === 'residuals' && c.x.type === 'fitted') {
        return t('{n} cases. A model that fits shows no pattern here: no curve and no funnel. The line marks zero, a residual of nothing.', { n: num(n) }) + left;
      }
      const base = t('{n} cases, one point each.', { n: num(n) });
      return c.fit ? `${base} ${t('The line rises or falls by {slope} for each unit of {x}.', { slope: fmt(c.fit.slope), x: subjectName(c.x, r) })}${left}` : `${base}${left}`;
    }
  }
}

/** The table of what a chart draws: a header and rows of words. */
export function chartTable(c: StatChart, r: StatResult, caseName: (id: number) => string): { head: string[]; rows: string[][] } {
  switch (c.kind) {
    case 'histogram':
      return { head: [t('From (above)'), t('Up to and including'), t('Cases')], rows: c.bins.map((b, i) => [i === 0 ? `${axisNumber(b.lo)} (${t('included')})` : axisNumber(b.lo), axisNumber(b.hi), num(b.count)]) };
    case 'bars':
      return {
        head: [t('Value'), t('Cases'), t('Share of cases with a value')],
        rows: c.categories.map((k) => [groupLabel(r, c.subject.type === 'variable' ? c.subject.index : 0, k.value, k.label), num(k.count), percentText(c.valid > 0 ? k.count / c.valid : 0)]),
      };
    case 'box':
      return {
        head: [t('Group'), 'n', t('Lower whisker'), t('First quartile'), t('Median'), t('Third quartile'), t('Upper whisker'), t('Mean'), t('Beyond the whiskers')],
        rows: c.groups.map((g) => [groupLabel(r, c.by, g.value, g.label), num(g.n), fmt(g.lower_whisker), fmt(g.q1), fmt(g.median), fmt(g.q3), fmt(g.upper_whisker), fmt(g.mean), g.outliers.map((o) => fmt(o)).join(', ')]),
      };
    case 'means':
      return { head: [t('Group'), 'n', t('Mean'), t('Lower end'), t('Upper end')], rows: c.groups.map((g) => [groupLabel(r, c.by, g.value, g.label), num(g.n), fmt(g.mean), fmt(g.lo), fmt(g.hi)]) };
    case 'stacked': {
      const rv = r.variables[c.row];
      const cv = r.variables[c.col];
      const shares = rowShares(c.counts);
      return {
        head: [rv ? variableLabel(rv) : '', ...c.col_values.map((v, j) => (cv ? valueLabel(cv.kind, v, c.col_labels[j]) : c.col_labels[j]))],
        rows: c.counts.map((row, i) => [rv ? valueLabel(rv.kind, c.row_values[i], c.row_labels[i]) : c.row_labels[i], ...row.map((n, j) => `${num(n)} (${percentText(shares[i][j])})`)]),
      };
    }
    case 'qq':
      return { head: [t('Expected if normal'), t('Value')], rows: c.points.map((p) => [fmt(p.theoretical), fmt(p.sample)]) };
    case 'scatter':
      return { head: [t('Case'), subjectName(c.x, r), subjectName(c.y, r)], rows: c.points.map((p) => [caseName(p.case_id), fmt(p.x), fmt(p.y)]) };
    case 'curves':
      return {
        head: [t('Case'), subjectName(c.x, r), subjectName(c.y, r), ...c.curves.map((cv) => t('{curve} (fitted)', { curve: curveName(cv.kind) }))],
        rows: c.points.map((p, i) => [caseName(p.case_id), fmt(p.x), fmt(p.y), ...c.curves.map((cv) => fmt(cv.at_points[i]))]),
      };
    default:
      return { head: [], rows: [] };
  }
}
