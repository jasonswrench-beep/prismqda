// Line numbers for the text on screen. The numbers count the lines as they are drawn (a paragraph that wraps onto
// three lines has three numbers), so they follow the width of the window and the text size. They are for finding
// your place on the screen and reading "lines 7 to 9" in the details; the units and the character offsets are what
// the project itself keeps.

/** A rectangle, in the same coordinates as the other boxes it is compared with. */
export interface LineBox {
  top: number;
  bottom: number;
  /** Zero or more; only boxes with some width and height are lines of text. */
  left?: number;
  right?: number;
}

/** One line of text on screen: its number (from 1) and where it sits. */
export interface Line {
  n: number;
  top: number;
  bottom: number;
}

/**
 * Turns the rectangles the browser reports for a block of text (one per piece of text per line) into lines, top to
 * bottom. Pieces whose middle falls inside the line above belong to it. Empty rectangles (the end of a line, a blank
 * line) are not lines, so blank lines between paragraphs are not numbered.
 */
export function groupLines(rects: readonly LineBox[]): Line[] {
  const real = rects
    .filter((r) => r.bottom - r.top > 0.5 && (r.right === undefined || r.left === undefined || r.right - r.left > 0.5))
    .sort((a, b) => a.top - b.top || a.bottom - b.bottom);
  const lines: Line[] = [];
  for (const r of real) {
    const last = lines[lines.length - 1];
    const middle = (r.top + r.bottom) / 2;
    if (last && middle >= last.top && middle <= last.bottom) {
      last.top = Math.min(last.top, r.top);
      last.bottom = Math.max(last.bottom, r.bottom);
    } else {
      lines.push({ n: lines.length + 1, top: r.top, bottom: r.bottom });
    }
  }
  return lines;
}

/** The index of the first line whose bottom is at or below `y`, or lines.length when `y` is past the last line. */
export function firstLineBelow(lines: readonly Line[], y: number): number {
  let lo = 0;
  let hi = lines.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (lines[mid].bottom < y) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** The lines that show between two heights (with room to spare), for drawing only what can be seen. */
export function linesBetween(lines: readonly Line[], from: number, to: number): Line[] {
  const out: Line[] = [];
  for (let i = firstLineBelow(lines, from); i < lines.length && lines[i].top <= to; i++) out.push(lines[i]);
  return out;
}

/** The first and last line numbers that a stretch of text from height `top` to height `bottom` runs over, or null with no lines. */
export function lineRangeFor(lines: readonly Line[], top: number, bottom: number): { first: number; last: number } | null {
  if (lines.length === 0) return null;
  const mid = (l: Line) => (l.top + l.bottom) / 2;
  let first = firstLineBelow(lines, top);
  if (first >= lines.length) first = lines.length - 1;
  // The line found is the first that ends below the top; the stretch may start inside the gap before it.
  let last = first;
  while (last + 1 < lines.length && mid(lines[last + 1]) <= bottom) last++;
  return { first: lines[first].n, last: lines[Math.max(first, last)].n };
}

/** A line number as it is shown: three digits at least, so a column of them lines up. */
export function lineLabel(n: number): string {
  return String(n).padStart(3, '0');
}
