// Whether words already have a code, so that the same code is not given twice to the same words by the same coder: one coding counted
// twice would make the counts, the agreement and the reports wrong. Kept apart from the screen so it can be tested.

import type { CodingRow } from '../types';
import { sameShape } from './regions';

interface Span {
  start: number;
  end: number;
}

/** The spans that do not already have this code, from this coder, on exactly these words. */
export function notYetCoded<T extends Span>(codings: readonly CodingRow[], spans: readonly T[], codeId: number, coderId: number): T[] {
  const has = new Set(codings.filter((c) => !c.region && c.layer === 'coder' && c.code_id === codeId && c.coder_id === coderId).map((c) => `${c.start}:${c.end}`));
  return spans.filter((s) => !has.has(`${s.start}:${s.end}`));
}

/** Whether another coding by the same coder gives the same words (or area, or stretch) this code. */
export function sameWordsWith(codings: readonly CodingRow[], here: CodingRow, codeId: number): boolean {
  return codings.some(
    (c) =>
      c.id !== here.id &&
      c.code_id === codeId &&
      c.coder_id === here.coder_id &&
      c.layer === here.layer &&
      (here.region ? !!c.region && (c.page ?? 1) === (here.page ?? 1) && sameShape(c.region, here.region) : !c.region && c.start === here.start && c.end === here.end),
  );
}
