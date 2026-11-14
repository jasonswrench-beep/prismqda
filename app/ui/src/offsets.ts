// Conversion between JavaScript string positions and the engine's offsets.
//
// JavaScript counts UTF-16 code units: an emoji or a rare CJK character takes two. The engine
// counts Unicode scalar values (code points): the same character is one. Every place where the
// interface crosses that boundary uses OffsetIndex, so the rule lives here and nowhere else.
//
// Only characters above U+FFFF are different between the two counts, so an index remembers where
// those sit and answers every question with a binary search. A document with none costs nothing.

/** Positions of every character above U+FFFF in a text, and conversions both ways. */
export class OffsetIndex {
  /** Length in UTF-16 code units, as `text.length`. */
  readonly utf16Length: number;
  /** Length in Unicode scalar values, as the engine counts. */
  readonly scalarLength: number;
  /** UTF-16 index of the first half of each surrogate pair, in order. */
  private readonly highs: number[] = [];
  /** Scalar index of each such character: highs[i] - i. Never decreases. */
  private readonly scalars: number[] = [];

  constructor(text: string) {
    this.utf16Length = text.length;
    const n = text.length;
    for (let i = 0; i < n; i++) {
      const c = text.charCodeAt(i);
      if (c >= 0xd800 && c <= 0xdbff && i + 1 < n) {
        const d = text.charCodeAt(i + 1);
        if (d >= 0xdc00 && d <= 0xdfff) {
          this.scalars.push(i - this.highs.length);
          this.highs.push(i);
          i++;
        }
      }
    }
    this.scalarLength = n - this.highs.length;
  }

  /** True when the text has no character above U+FFFF, so both counts are the same. */
  get isPlain(): boolean {
    return this.highs.length === 0;
  }

  /**
   * The scalar offset for a UTF-16 position. A position that falls between the two halves of one
   * character is moved to the start of that character (`bias: 'down'`) or to its end (`'up'`).
   */
  toScalar(utf16: number, bias: 'down' | 'up' = 'down'): number {
    const u = clampInt(utf16, this.utf16Length);
    if (this.highs.length === 0) return u;
    // Count the pairs that start before u (down) or end before u (up).
    const limit = bias === 'down' ? u : u - 1;
    return u - countLess(this.highs, limit);
  }

  /** The UTF-16 position for a scalar offset. */
  toUtf16(scalar: number): number {
    const s = clampInt(scalar, this.scalarLength);
    if (this.highs.length === 0) return s;
    return s + countLess(this.scalars, s);
  }
}

/** A whole number held between 0 and `max`; anything that is not a number counts as 0. */
function clampInt(n: number, max: number): number {
  if (!Number.isFinite(n)) return n === Infinity ? max : 0;
  return Math.min(Math.max(Math.trunc(n), 0), max);
}

/** How many entries of a sorted array are strictly below `limit`. */
function countLess(sorted: number[], limit: number): number {
  let lo = 0;
  let hi = sorted.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (sorted[mid] < limit) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Length of a string in Unicode scalar values. */
export function scalarLength(text: string): number {
  let n = text.length;
  for (let i = 0; i < text.length - 1; i++) {
    const c = text.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      const d = text.charCodeAt(i + 1);
      if (d >= 0xdc00 && d <= 0xdfff) {
        n--;
        i++;
      }
    }
  }
  return n;
}

/** The text between two scalar offsets. */
export function sliceScalar(text: string, index: OffsetIndex, start: number, end: number): string {
  return text.slice(index.toUtf16(start), index.toUtf16(end));
}

const WHITE = /^[\s ​‎‏]$/u;

/**
 * Narrows a span so it does not start or end on spaces or line breaks. Double-clicking a word on
 * Windows selects the space after it, and nobody means to code that. Returns null if only white
 * space was selected.
 */
export function trimSpan(text: string, index: OffsetIndex, start: number, end: number): { start: number; end: number } | null {
  let u0 = index.toUtf16(start);
  let u1 = index.toUtf16(end);
  while (u0 < u1 && WHITE.test(text.charAt(u0))) u0++;
  while (u1 > u0 && WHITE.test(text.charAt(u1 - 1))) u1--;
  if (u0 >= u1) return null;
  return { start: index.toScalar(u0), end: index.toScalar(u1, 'up') };
}
