// The numbers behind the project dashboard: changes per day, the share of each kind of document, and the geometry of
// the two charts. Everything here is plain arithmetic on what the engine already says, so it can be checked on its own.
import { compareText, dateTime, t } from '../i18n';

export interface DayCount {
  /** The day as the person's calendar has it, year-month-day. */
  date: string;
  /** Short text for the axis and the table: "Nov 4", and "Today" for the last day. */
  label: string;
  /** True for the last day, which is today. */
  isToday?: boolean;
  count: number;
}

const pad = (n: number) => String(n).padStart(2, '0');

/** A date as year-month-day in the computer's own time zone. */
export function localDay(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/**
 * How many changes were made on each of the last `days` days, ending with today. A change is one row of the project's
 * history (an import, a coding, an edit, a removal), counted on the day it was made in the person's time zone.
 */
export function changesPerDay(timestamps: readonly string[], now: Date, days = 14): DayCount[] {
  const out: DayCount[] = [];
  const index = new Map<string, DayCount>();
  for (let back = days - 1; back >= 0; back--) {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - back);
    const row: DayCount = { date: localDay(d), label: back === 0 ? t('Today') : dateTime(d, { month: 'short', day: 'numeric' }), isToday: back === 0, count: 0 };
    out.push(row);
    index.set(row.date, row);
  }
  for (const ts of timestamps) {
    const when = new Date(ts);
    if (Number.isNaN(when.getTime())) continue;
    const row = index.get(localDay(when));
    if (row) row.count++;
  }
  return out;
}

/** The busiest of the days; the latest one wins a tie, and null when nothing happened. */
export function busiestDay(days: readonly DayCount[]): DayCount | null {
  let best: DayCount | null = null;
  for (const d of days) if (d.count > 0 && (!best || d.count >= best.count)) best = d;
  return best;
}

/** A round top for the chart's scale: at least 3, so a quiet project does not look wild. */
export function niceMax(max: number): number {
  if (max <= 3) return 3;
  const step = max <= 10 ? 1 : max <= 50 ? 5 : max <= 200 ? 10 : 50;
  return Math.ceil(max / step) * step;
}

export interface Chart {
  width: number;
  height: number;
  points: { x: number; y: number; day: DayCount }[];
  /** The line, as an SVG path. */
  line: string;
  /** The line closed down to the axis, for the shaded area under it. */
  area: string;
  top: number;
}

/** The line and area of a chart in a `width` by `height` box with the given margins around the plot. */
export function areaChart(days: readonly DayCount[], size: { width: number; height: number; left: number; right: number; top: number; bottom: number }): Chart {
  const top = niceMax(Math.max(0, ...days.map((d) => d.count)));
  const plotW = size.width - size.left - size.right;
  const plotH = size.height - size.top - size.bottom;
  const points = days.map((day, i) => ({
    x: size.left + (days.length <= 1 ? plotW / 2 : (plotW * i) / (days.length - 1)),
    y: size.top + plotH - (plotH * day.count) / top,
    day,
  }));
  const r = (n: number) => Math.round(n * 10) / 10;
  const line = points.map((p, i) => `${i === 0 ? 'M' : 'L'}${r(p.x)} ${r(p.y)}`).join(' ');
  const baseline = r(size.top + plotH);
  const area = points.length === 0 ? '' : `${line} L${r(points[points.length - 1].x)} ${baseline} L${r(points[0].x)} ${baseline} Z`;
  return { width: size.width, height: size.height, points, line, area, top };
}

export interface Share {
  type: string;
  label: string;
  count: number;
  /** Whole percent; the shares of one chart add up to exactly 100. */
  percent: number;
}

/**
 * The share of the documents that each kind makes up. Each document is counted once, under its own kind, so the shares
 * add up to the whole. Percentages are rounded so that they add up to 100 (the largest remainders get the extra points).
 */
export function typeShares(types: readonly string[], labels: Record<string, string>): Share[] {
  const counts = new Map<string, number>();
  for (const t of types) counts.set(t, (counts.get(t) ?? 0) + 1);
  const total = types.length;
  const rows = [...counts.entries()]
    .map(([type, count]) => ({ type, label: labels[type] ?? type, count, exact: total === 0 ? 0 : (count / total) * 100 }))
    .sort((a, b) => b.count - a.count || compareText(a.label, b.label));
  const floor = rows.map((r) => Math.floor(r.exact));
  let left = (total === 0 ? 0 : 100) - floor.reduce((a, b) => a + b, 0);
  const order = rows.map((r, i) => ({ i, rem: r.exact - Math.floor(r.exact) })).sort((a, b) => b.rem - a.rem || a.i - b.i);
  for (const o of order) {
    if (left <= 0) break;
    floor[o.i]++;
    left--;
  }
  return rows.map((r, i) => ({ type: r.type, label: r.label, count: r.count, percent: floor[i] }));
}

export interface Slice {
  /** SVG stroke-dasharray for a circle drawn with a thick stroke. */
  dash: string;
  /** SVG stroke-dashoffset: where the slice starts. */
  offset: number;
}

/** The slices of a ring of radius `r`, one for each share, with a small gap between them. */
export function ringSlices(shares: readonly Share[], r: number, gap = 1.5): Slice[] {
  const circumference = 2 * Math.PI * r;
  const total = shares.reduce((n, s) => n + s.count, 0);
  if (total === 0) return [];
  const round = (n: number) => Math.round(n * 100) / 100;
  let start = 0;
  return shares.map((s) => {
    const length = (s.count / total) * circumference;
    const shown = shares.length > 1 ? Math.max(0, length - gap) : length;
    const slice = { dash: `${round(shown)} ${round(circumference - shown)}`, offset: round(-start) || 0 };
    start += length;
    return slice;
  });
}

/** "Today, 2:14 PM", "Yesterday, 4:30 PM", or "Nov 4, 2026, 9:05 AM": when something was last saved, in the person's words. */
export function friendlyTime(ms: number, now: number): string {
  const when = new Date(ms);
  const today = new Date(now);
  const time = dateTime(when, { hour: 'numeric', minute: '2-digit' });
  const dayOf = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diff = Math.round((dayOf(today) - dayOf(when)) / 86_400_000);
  if (diff === 0) return t('Today, {time}', { time });
  if (diff === 1) return t('Yesterday, {time}', { time });
  return t('{date}, {time}', { date: dateTime(when, { month: 'short', day: 'numeric', year: 'numeric' }), time });
}
