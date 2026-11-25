// How a cell of a table worked out by the engine is shown. The engine sends numbers in full (52.42857142857143), which the saved file
// keeps; on the screen two decimals are enough to read.

import { num } from '../i18n';

/** A cell as shown: a number with more than two decimals is rounded to two, and anything else is as it came. */
export function shownCell(cell: string | null): string | null {
  if (cell === null || !/^-?\d+\.\d{3,}$/.test(cell)) return cell;
  return num(Number(cell), { maximumFractionDigits: 2, useGrouping: false });
}
