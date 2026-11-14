// Coded stretches of a recording ("clips"): their times as clocks and as words, the times a person types, how stretches are laid out on a
// timeline, and the checks that mirror the engine's. All times are whole milliseconds from the start of the recording. Plain functions,
// so they can be tested without a screen or a player. The words match the engine's own (`Shape::describe` in `regions.rs`).

import type { IconName } from '../assets/icons.generated';
import type { RegionShape, SourceRow, SpanShape } from '../types';
import { t } from '../i18n';

/** The shortest stretch the engine takes to code, in milliseconds. */
export const MIN_SPAN_MS = 100;
/** The latest a stretch may end (seven days), as in the engine. */
export const MAX_SPAN_END_MS = 7 * 24 * 60 * 60 * 1000;

/** A time as a clock: `1:05`, `0:07.4`, or `1:02:03` once it passes an hour. Tenths of a second are shown when `tenths` is set. */
export function clockText(ms: number, tenths = false): string {
  const whole = Math.max(0, Math.round(Number.isFinite(ms) ? ms : 0));
  const total = Math.floor(whole / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const fraction = tenths ? `.${Math.floor((whole % 1000) / 100)}` : '';
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(s)}${fraction}` : `${m}:${two(s)}${fraction}`;
}

/** Both ends of a stretch as clocks, with tenths when either end is not a whole second (so the two line up). */
export function clockPair(startMs: number, endMs: number): [string, string] {
  const tenths = startMs % 1000 >= 100 || endMs % 1000 >= 100;
  return [clockText(startMs, tenths), clockText(endMs, tenths)];
}

/** The length of a stretch as a clock. */
export function lengthText(startMs: number, endMs: number): string {
  return clockText(endMs - startMs, (endMs - startMs) % 1000 >= 100);
}

/** A stretch in words, as the engine says it: "Recording, 0:12 to 0:45 (0:33 long)". */
export function spanWords(startMs: number, endMs: number): string {
  const [from, to] = clockPair(startMs, endMs);
  return t('Recording, {from} to {to} ({length} long)', { from, to, length: lengthText(startMs, endMs) });
}

/** The same without the length, for a label with little room. */
export function spanShort(startMs: number, endMs: number): string {
  const [from, to] = clockPair(startMs, endMs);
  return t('{from} to {to}', { from, to });
}

/**
 * A time a person typed, in whole milliseconds, or `null` when it is not a time. Written as `1:05`, `1:02:03`, or `0:07.4` (minutes and
 * seconds below 60), or as plain seconds: `65` or `7.4`. Nothing else is guessed at.
 */
export function parseClock(text: string): number | null {
  const s = text.trim();
  if (s === '') return null;
  const parts = s.split(':');
  if (parts.length > 3) return null;
  const number = /^\d+(\.\d+)?$/;
  const whole = /^\d+$/;
  let seconds: number;
  if (parts.length === 1) {
    if (!number.test(s)) return null;
    seconds = Number(s);
  } else {
    const last = parts[parts.length - 1];
    const front = parts.slice(0, -1);
    if (!number.test(last) || front.some((p) => !whole.test(p))) return null;
    const secs = Number(last);
    const mins = Number(front[front.length - 1]);
    const hours = front.length === 2 ? Number(front[0]) : 0;
    if (secs >= 60 || (front.length === 2 && mins >= 60)) return null;
    seconds = hours * 3600 + mins * 60 + secs;
  }
  const ms = Math.round(seconds * 1000);
  return Number.isFinite(ms) && ms >= 0 && ms <= MAX_SPAN_END_MS ? ms : null;
}

/** True when the stretch could be stored. */
export function isCodableSpan(startMs: number, endMs: number): boolean {
  return Number.isInteger(startMs) && Number.isInteger(endMs) && startMs >= 0 && endMs - startMs >= MIN_SPAN_MS && endMs <= MAX_SPAN_END_MS;
}

/** Why a stretch cannot be coded, in words, or `null` when it can. */
export function spanProblem(startMs: number | null, endMs: number | null): string | null {
  if (startMs === null || endMs === null) return t('Mark where the stretch starts and where it ends first.');
  if (endMs <= startMs) return t('A stretch must end after it starts.');
  if (endMs - startMs < MIN_SPAN_MS) return t('The stretch is too short to code: it must be at least a tenth of a second.');
  if (!isCodableSpan(startMs, endMs)) return t('That is not a stretch of a recording.');
  return null;
}

/** A stretch with its ends in order and in whole milliseconds, however the two marks were made. */
export function orderedSpan(a: number, b: number): SpanShape {
  const x = Math.max(0, Math.round(a));
  const y = Math.max(0, Math.round(b));
  return { shape: 'span', start_ms: Math.min(x, y), end_ms: Math.max(x, y) };
}

/** The icon a document goes by in lists and tabs: a picture, a recording (audio or video), or words. */
export function sourceIcon(source: Pick<SourceRow, 'image' | 'recording'> | null | undefined): IconName {
  if (source?.image) return 'src-image';
  if (source?.recording?.stands_alone) return source.recording.kind === 'video' ? 'src-video' : 'src-audio';
  return 'src-transcript';
}

export function isSpan(shape: RegionShape | null | undefined): shape is SpanShape {
  return !!shape && shape.shape === 'span';
}

/** True when two stretches are the same stretch. */
export function sameSpan(a: SpanShape, b: SpanShape): boolean {
  return a.start_ms === b.start_ms && a.end_ms === b.end_ms;
}

/** One stretch laid out on a timeline: how far along it starts and how wide it is (both fractions of the timeline), and the row it is in. */
export interface Stripe {
  id: number;
  left: number;
  width: number;
  lane: number;
}

/**
 * Lays stretches out on a timeline of `total` milliseconds. Stretches that overlap are put in different rows (lanes), so none hides
 * another; a stretch takes the first lane that is free where it starts. Each is at least `minWidth` wide so a short one can still be seen.
 */
export function stripeLayout(spans: readonly { id: number; start_ms: number; end_ms: number }[], total: number, minWidth = 0.004): Stripe[] {
  const whole = Math.max(1, total);
  const sorted = [...spans].sort((a, b) => a.start_ms - b.start_ms || a.end_ms - b.end_ms || a.id - b.id);
  const laneEnds: number[] = [];
  const out: Stripe[] = [];
  for (const s of sorted) {
    let lane = laneEnds.findIndex((end) => end <= s.start_ms);
    if (lane < 0) {
      lane = laneEnds.length;
      laneEnds.push(0);
    }
    laneEnds[lane] = s.end_ms;
    const left = Math.min(1, Math.max(0, s.start_ms / whole));
    const width = Math.min(1 - left, Math.max(minWidth, (s.end_ms - s.start_ms) / whole));
    out.push({ id: s.id, left, width, lane });
  }
  return out;
}

/** The place on a timeline (a fraction from 0 to 1) a time is at. */
export function fractionOf(ms: number, total: number): number {
  return Math.min(1, Math.max(0, ms / Math.max(1, total)));
}

/** The time (whole milliseconds) a place on a timeline is, from a fraction of its width. */
export function timeAtFraction(fraction: number, total: number): number {
  return Math.round(Math.min(1, Math.max(0, fraction)) * Math.max(0, total));
}
