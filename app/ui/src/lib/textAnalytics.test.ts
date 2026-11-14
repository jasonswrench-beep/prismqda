import assert from 'node:assert/strict';
import test from 'node:test';
import type { WordContexts, WordFrequencies } from '../types';
import { barWidth, contextSummary, frequencySummary, shareOf } from './textAnalytics';

const freq = (patch: Partial<WordFrequencies> = {}): WordFrequencies => ({ words: [], total_words: 1234, counted_words: 567, distinct_words: 89, documents: 3, ...patch });
const contexts = (patch: Partial<WordContexts> = {}): WordContexts => ({ items: [], total: 0, documents: 0, ...patch });

test('what was counted is one sentence, and an empty corpus says why', () => {
  assert.equal(frequencySummary(freq()), '567 of 1,234 words were counted, 89 of them different, in 3 documents.');
  assert.equal(frequencySummary(freq({ documents: 1 })), '567 of 1,234 words were counted, 89 of them different, in 1 document.');
  assert.equal(frequencySummary(freq({ documents: 0, total_words: 0 })), 'There is no document of words to read.');
  assert.equal(frequencySummary(freq({ total_words: 0, counted_words: 0, distinct_words: 0 })), 'No words were found in 3 documents.');
});

test('a share is a percentage of the words counted, and a bar is never invisible', () => {
  assert.equal(shareOf(15, 100), '15%');
  assert.equal(shareOf(1, 3), '33.3%');
  assert.equal(shareOf(1, 0), '0%');
  assert.equal(barWidth(50, 100), 50);
  assert.equal(barWidth(1, 1000), 2, 'a rare word still shows a sliver');
  assert.equal(barWidth(5, 0), 0);
});

test('where a word is used is one sentence, and says when the list was cut', () => {
  const item = { source_id: 1, source_name: 'A', start: 0, end: 1, left: '', hit: 'x', right: '' };
  assert.equal(contextSummary(contexts()), 'That word is not used in these documents.');
  assert.equal(contextSummary(contexts({ total: 1, documents: 1, items: [item] })), 'Used 1 time in 1 document.');
  assert.equal(contextSummary(contexts({ total: 1500, documents: 4, items: [item, item] })), 'Used 1,500 times in 4 documents. The first 2 are listed.');
});
