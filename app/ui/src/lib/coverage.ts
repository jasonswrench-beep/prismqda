// How much of a source has been coded, for the small bar beside each source in the list.

import type { CodingMode, CodingRow, UnitRow } from '../types';
import { num, t, tn } from '../i18n';

export interface Coverage {
  /** What was counted: whole units, or characters of text. */
  by: 'units' | 'characters';
  coded: number;
  total: number;
  /** 0 to 1. */
  fraction: number;
}

type Span = Pick<CodingRow, 'start' | 'end' | 'unit_id'>;

/** The number of characters that lie inside at least one coding. */
export function coveredLength(codings: readonly Pick<CodingRow, 'start' | 'end'>[]): number {
  const spans = codings.map((c) => [c.start, c.end] as const).sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  let total = 0;
  let curStart = -1;
  let curEnd = -1;
  for (const [s, e] of spans) {
    if (e <= s) continue;
    if (s > curEnd) {
      if (curEnd > curStart) total += curEnd - curStart;
      curStart = s;
      curEnd = e;
    } else if (e > curEnd) {
      curEnd = e;
    }
  }
  if (curEnd > curStart) total += curEnd - curStart;
  return total;
}

/**
 * In a unit project, a source counts by the units that have a coding; otherwise by the characters
 * covered. `units` may be left out when only the codings are known: the units that appear in the
 * codings are counted against the unit total from the source list.
 */
export function computeCoverage(
  source: { characters: number; units: number },
  codings: readonly Span[],
  mode: CodingMode,
  units?: readonly Pick<UnitRow, 'id' | 'start' | 'end' | 'excluded'>[],
): Coverage {
  if (mode === 'unit' && (source.units > 0 || (units && units.length > 0))) {
    let total = source.units;
    let coded = 0;
    if (units && units.length > 0) {
      const open = units.filter((u) => !u.excluded);
      total = open.length;
      const withCoding = new Set<number>();
      for (const c of codings) if (c.unit_id !== null) withCoding.add(c.unit_id);
      coded = open.filter((u) => withCoding.has(u.id)).length;
    } else {
      coded = new Set(codings.filter((c) => c.unit_id !== null).map((c) => c.unit_id)).size;
    }
    coded = Math.min(coded, total);
    return { by: 'units', coded, total, fraction: total === 0 ? 0 : coded / total };
  }
  const total = source.characters;
  const coded = Math.min(coveredLength(codings), total);
  return { by: 'characters', coded, total, fraction: total === 0 ? 0 : coded / total };
}

/** Words for the bar that a screen reader can read: "3 of 12 units coded". */
export function describeCoverage(c: Coverage | null, codingCount: number): string {
  if (!c) return codingCount === 0 ? t('Nothing coded yet') : `${codingCount} ${codingCount === 1 ? 'coding' : 'codings'}`;
  if (c.coded === 0) return t('Nothing coded yet');
  if (c.by === 'units') return tn(c.total, { one: '{coded} of {count} unit coded', other: '{coded} of {count} units coded' }, { coded: num(c.coded) });
  return t('{fraction}% of the text coded', { fraction: Math.round(c.fraction * 100) });
}
