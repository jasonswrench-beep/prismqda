// Running an analysis again after the data changed: what it said before beside what it says now, for the numbers the page leads with
// (the same ones the bootstrap of a whole analysis reports), and the number of cases used.

import { num, t } from '../i18n';
import type { StatResult } from '../types';
import { analysisRows } from './statBoot';
import { fmt, fmtNoZero, pCell } from './statistics';
import type { ApaTable } from './statistics';

/** The number at a path of a result's details (`t_test/p`, `models/0/terms/1/estimate`), or null when there is none. */
export function numberAt(details: unknown, path: string): number | null {
  let at: unknown = details;
  for (const part of path.split('/')) {
    if (at === null || typeof at !== 'object') return null;
    at = (at as Record<string, unknown>)[part];
  }
  return typeof at === 'number' && Number.isFinite(at) ? at : null;
}

function signed(x: number, text: string): string {
  return x > 0 ? `+${text}` : text;
}

/** True when two results of the same analysis differ in any number the page leads with, or in the cases used. */
export function resultsDiffer(before: StatResult, now: StatResult): boolean {
  if (before.n_used !== now.n_used) return true;
  return analysisRows(now).some((row) => {
    const a = numberAt(before.details, row.path);
    const b = numberAt(now.details, row.path);
    return a !== b;
  });
}

/** The table that puts the earlier result beside this one. */
export function compareTable(before: StatResult, now: StatResult): ApaTable {
  const rows: string[][] = [[t('Cases used'), num(before.n_used), num(now.n_used), signed(now.n_used - before.n_used, num(now.n_used - before.n_used))]];
  for (const row of analysisRows(now)) {
    const a = numberAt(before.details, row.path);
    const b = numberAt(now.details, row.path);
    if (a === null || b === null) continue;
    if (row.p) rows.push([row.label, pCell(a), pCell(b), signed(b - a, fmtNoZero(b - a, 3))]);
    else {
      const f = row.small ? (x: number) => fmtNoZero(x) : (x: number) => fmt(x);
      rows.push([row.label, f(a), f(b), signed(b - a, f(b - a))]);
    }
  }
  return {
    title: t('This run compared with the earlier one'),
    head: [t('Statistic'), t('Earlier run'), t('This run'), t('Change')],
    rows,
    note: t('The numbers the page leads with, from the run before and from this run. Only the numbers that are in both are compared.'),
  };
}
