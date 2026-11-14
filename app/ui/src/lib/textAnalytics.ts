import type { WordContexts, WordFrequencies } from '../types';
import { num, t, tn } from '../i18n';

/** One sentence that says what was counted: how many words, out of how many, how many different, in how many documents. */
export function frequencySummary(f: WordFrequencies): string {
  if (f.documents === 0) return t('There is no document of words to read.');
  const documents = tn(f.documents, { one: '{count} document', other: '{count} documents' });
  if (f.total_words === 0) return t('No words were found in {documents}.', { documents });
  return t('{counted} of {total} words were counted, {distinct} of them different, in {documents}.', {
    counted: num(f.counted_words),
    total: num(f.total_words),
    distinct: num(f.distinct_words),
    documents,
  });
}

/** A word's share of the words counted, as the language writes a percentage. */
export function shareOf(count: number, counted: number): string {
  return num(counted > 0 ? count / counted : 0, { style: 'percent', maximumFractionDigits: 1 });
}

/** How wide a word's bar is, from 0 to 100, against the commonest word's. */
export function barWidth(count: number, max: number): number {
  return max > 0 ? Math.max(2, Math.round((count / max) * 100)) : 0;
}

/** One sentence that says how often a word is used and in how many documents, and that the list is cut when it is. */
export function contextSummary(c: WordContexts): string {
  if (c.total === 0) return t('That word is not used in these documents.');
  const used = tn(c.total, { one: 'Used {count} time in {documents}.', other: 'Used {count} times in {documents}.' }, { documents: tn(c.documents, { one: '{count} document', other: '{count} documents' }) });
  return c.items.length < c.total ? `${used} ${t('The first {shown} are listed.', { shown: num(c.items.length) })}` : used;
}
