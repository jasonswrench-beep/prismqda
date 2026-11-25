import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { CodingRow } from '../types';
import { notYetCoded, sameWordsWith } from './recode';

let n = 0;
const coding = (start: number, end: number, code_id: number, coder_id = 1, extra: Partial<CodingRow> = {}): CodingRow =>
  ({ id: ++n, source_id: 1, start, end, code_id, coder_id, origin: 'human', layer: 'coder', rationale: '', unit_id: null, created_at: '', ...extra }) as CodingRow;

test('a code the words already have from the same coder is not given twice', () => {
  const all = [coding(10, 20, 5), coding(30, 40, 6)];
  assert.deepEqual(notYetCoded(all, [{ start: 10, end: 20 }], 5, 1), []);
  assert.deepEqual(notYetCoded(all, [{ start: 10, end: 20 }, { start: 30, end: 40 }], 5, 1), [{ start: 30, end: 40 }], 'the rest of several are still coded');
});

test('another code, another coder, other words and the final layer are not the same coding', () => {
  const all = [coding(10, 20, 5), coding(50, 60, 5, 1, { layer: 'final' })];
  assert.equal(notYetCoded(all, [{ start: 10, end: 20 }], 6, 1).length, 1);
  assert.equal(notYetCoded(all, [{ start: 10, end: 20 }], 5, 2).length, 1);
  assert.equal(notYetCoded(all, [{ start: 10, end: 21 }], 5, 1).length, 1);
  assert.equal(notYetCoded(all, [{ start: 50, end: 60 }], 5, 1).length, 1);
});

test('a coding is not changed to a code its words or its area already have', () => {
  const a = coding(10, 20, 5);
  const b = coding(10, 20, 6);
  const far = coding(30, 40, 6);
  assert.equal(sameWordsWith([a, b, far], a, 6), true);
  assert.equal(sameWordsWith([a, far], a, 6), false);
  assert.equal(sameWordsWith([a, coding(10, 20, 6, 2)], a, 6), false, "another coder's coding is theirs");
  const region = { shape: 'rectangle', x: 0.1, y: 0.1, w: 0.2, h: 0.2 } as const;
  const p = coding(0, 0, 5, 1, { region, page: 1 } as Partial<CodingRow>);
  const q = coding(0, 0, 6, 1, { region: { ...region }, page: 1 } as Partial<CodingRow>);
  const r = coding(0, 0, 6, 1, { region: { ...region }, page: 2 } as Partial<CodingRow>);
  assert.equal(sameWordsWith([p, q], p, 6), true);
  assert.equal(sameWordsWith([p, r], p, 6), false, 'the same place on another page is another area');
});
