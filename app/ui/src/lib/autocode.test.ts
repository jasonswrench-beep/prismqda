import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { AutocodePreview, BatchSummary } from '../types';
import { batchLine, buildArgs, describeOutcome, emptySetup, moveEnd, moveStart, previewSentences, readback, setupKey, snippet, spanOf, windowOf, windowParts, type AutoSetup, type Names } from './autocode';

const names: Names = {
  code: (id) => ({ 1: 'Cost', 2: 'Access' })[id] ?? `code ${id}`,
  source: (id) => `doc ${id}`,
  case: (id) => `case ${id}`,
};

const words = (patch: Partial<AutoSetup> = {}): AutoSetup => ({ ...emptySetup(), query: 'cost', codeId: 1, ...patch });

test('the words form says what is missing, one thing at a time', () => {
  assert.deepEqual(buildArgs({ ...emptySetup(), codeId: 1 }), { problem: 'Type the words to find.' });
  assert.match((buildArgs({ ...emptySetup(), query: 'cost' }) as { problem: string }).problem, /Choose the code/);
  assert.match((buildArgs({ ...emptySetup(), query: 'cost', codeMode: 'new' }) as { problem: string }).problem, /name for the new code/);
  assert.match((buildArgs(words({ target: 'words', around: 0 })) as { problem: string }).problem, /from 1 to 50/);
  assert.match((buildArgs(words({ scope: 'documents' })) as { problem: string }).problem, /at least one document/);
  assert.match((buildArgs(words({ scope: 'case' })) as { problem: string }).problem, /Choose a case/);
});

test('the words form becomes a request with only what the method uses', () => {
  assert.deepEqual(buildArgs(words()), { args: { method: 'words', query: 'cost', target: 'sentence', code_id: 1 } });
  assert.deepEqual(buildArgs(words({ query: '  cost NEAR/5 access ', target: 'words', around: 3, scope: 'documents', sourceIds: [4, 5] })), {
    args: { method: 'words', query: 'cost NEAR/5 access', target: 'words', around: 3, code_id: 1, source_ids: [4, 5] },
  });
  assert.deepEqual(buildArgs(words({ codeMode: 'new', newCode: ' Money ', scope: 'case', caseId: 7 })), {
    args: { method: 'words', query: 'cost', target: 'sentence', new_code: 'Money', case_id: 7 },
  });
});

test('speakers need a code each, and only the chosen ones are sent', () => {
  const s: AutoSetup = {
    ...emptySetup(),
    method: 'speakers',
    speakers: {
      priya: { label: 'Priya', include: true, mode: 'new', codeId: null, newCode: 'Priya' },
      sam: { label: 'Sam', include: false, mode: 'existing', codeId: null, newCode: '' },
      alex: { label: 'Alex', include: true, mode: 'existing', codeId: 2, newCode: '' },
    },
  };
  assert.deepEqual(buildArgs(s), { args: { method: 'speakers', speakers: [{ key: 'priya', new_code: 'Priya' }, { key: 'alex', code_id: 2 }] } });
  s.speakers.alex.codeId = null;
  assert.match((buildArgs(s) as { problem: string }).problem, /Choose the code for Alex/);
  assert.deepEqual(buildArgs({ ...emptySetup(), method: 'speakers' }), { problem: 'Choose at least one speaker.' });
});

test('headings need a level; the heading words are sent only when typed', () => {
  assert.deepEqual(buildArgs({ ...emptySetup(), method: 'headings', codeId: 1 }), { problem: 'Choose at least one heading level.' });
  assert.deepEqual(buildArgs({ ...emptySetup(), method: 'headings', codeId: 1, levels: [2, 1] }), { args: { method: 'headings', levels: [1, 2], include_heading: true, code_id: 1 } });
  assert.deepEqual(buildArgs({ ...emptySetup(), method: 'headings', codeId: 1, levels: [1], headingQuery: ' methods ', includeHeading: false }), {
    args: { method: 'headings', levels: [1], include_heading: false, heading_query: 'methods', code_id: 1 },
  });
});

test('the readback names the code, the place, and how much each suggestion covers', () => {
  const a = (s: AutoSetup) => (buildArgs(s) as { args: Parameters<typeof readback>[0] }).args;
  assert.equal(readback(a(words()), names), 'Find cost in every document, and suggest the code Cost for the whole sentence.');
  assert.equal(readback(a(words({ target: 'words', around: 4, scope: 'documents', sourceIds: [3] })), names), 'Find cost in doc 3, and suggest the code Cost for the matched words and 4 words either side.');
  assert.equal(readback(a(words({ target: 'hit', codeMode: 'new', newCode: 'Money', scope: 'case', caseId: 2 })), names), 'Find cost in the documents of the case case 2, and suggest a new code named Money for the matched words.');
  assert.equal(
    readback(a({ ...emptySetup(), method: 'headings', codeId: 2, levels: [1, 2], headingQuery: 'methods' }), names),
    'Suggest the code Access for each section under a levels 1 and 2 heading whose heading contains methods in every document, with the heading itself.',
  );
  assert.match(
    readback({ method: 'speakers', speakers: [{ key: 'priya', new_code: 'Priya' }, { key: 'sam', code_id: 1 }] }, names),
    /^Suggest codes for every turn in every document by priya as Priya and sam as Cost\.$/,
  );
  assert.equal(
    readback({ method: 'speakers', speakers: [{ key: 'priya', new_code: 'Priya' }] }, { ...names, speaker: (k) => ({ priya: 'Priya B.' })[k] ?? k }),
    'Suggest codes for every turn in every document by Priya B. as Priya.',
    'a speaker is named as written, not by the key'
  );
});

test('a request is the same text every time, so a preview can tell it still matches the form', () => {
  const a = (buildArgs(words()) as { args: Parameters<typeof setupKey>[0] }).args;
  const b = (buildArgs(words()) as { args: Parameters<typeof setupKey>[0] }).args;
  assert.equal(setupKey(a), setupKey(b));
  assert.notEqual(setupKey(a), setupKey((buildArgs(words({ query: 'access' })) as { args: Parameters<typeof setupKey>[0] }).args));
});

const preview = (patch: Partial<AutocodePreview> = {}): AutocodePreview => ({
  method: 'words', label: 'Words: cost', found: 12, suggestions: 9, documents: 3, already_coded: 2, already_waiting: 1, left_out: 0, truncated: false, codes: [], examples: [], ...patch,
});

test('a preview is told as short facts, with the singular where there is one', () => {
  assert.deepEqual(previewSentences(preview()), [
    'Found 12 places.',
    '2 are already coded this way, so they are left out.',
    '1 is already waiting in the review queue, so it is left out.',
    '9 suggestions in 3 documents can be sent to the review queue.',
  ]);
  assert.deepEqual(previewSentences(preview({ method: 'speakers', found: 1, suggestions: 1, documents: 1, already_coded: 0, already_waiting: 0, left_out: 2 })), [
    'Found 1 turn.',
    "2 turns are left out of coding in these documents (an interviewer's, for example).",
    '1 suggestion in 1 document can be sent to the review queue.',
  ]);
  assert.deepEqual(previewSentences(preview({ found: 0, suggestions: 0, documents: 0, already_coded: 0, already_waiting: 0 })), ['Nothing was found.', 'There is nothing new to send.']);
  assert.ok(previewSentences(preview({ truncated: true })).some((s) => /only the first ones/.test(s)));
});

test('a batch is told as what waits and what was decided', () => {
  const b: BatchSummary = { batch: 'x', origin: 'autocode', method: 'words', label: 'Words: cost', created_at: '', pending: 3, accepted: 0, rejected: 0, code_ids: [1] };
  assert.equal(batchLine(b), '3 waiting');
  assert.equal(batchLine({ ...b, pending: 1, accepted: 1200, rejected: 4 }), '1 waiting, 1,200 accepted, 4 rejected');
});

// ---- moving the edges by whole words

const row = { before: 'We talked. The ', text: 'cost was high', after: ' and access was hard.', start: 100, truncated: false };

test('the edges move one word at a time and report the new place in the document', () => {
  const w = windowOf(row)!;
  assert.deepEqual(spanOf(w), { start: 100, end: 113 });
  const earlier = moveStart(w, -1);
  assert.deepEqual(windowParts(earlier), { before: 'We talked. ', words: 'The cost was high', after: ' and access was hard.' });
  assert.deepEqual(spanOf(earlier), { start: 96, end: 113 });
  const later = moveEnd(w, 1);
  assert.equal(windowParts(later).words, 'cost was high and');
  assert.deepEqual(spanOf(later), { start: 100, end: 117 });
  // Giving words up from either side.
  assert.equal(windowParts(moveStart(w, 1)).words, 'was high');
  assert.equal(windowParts(moveEnd(w, -1)).words, 'cost was');
});

test('an edge stops where there are no more words to give or take', () => {
  const one = windowOf({ ...row, text: 'cost' })!;
  assert.equal(moveStart(one, 1), one, 'the last word cannot be given up');
  assert.equal(moveEnd(one, -1), one);
  const edge = windowOf({ before: '', text: 'cost', after: '', start: 0, truncated: false })!;
  assert.equal(moveStart(edge, -1), edge, 'nothing before the first word');
  assert.equal(moveEnd(edge, 1), edge, 'nothing after the last word');
  assert.equal(windowOf({ ...row, truncated: true }), null, 'long passages are not moved by words here');
});

test('characters outside the basic range are counted as one each', () => {
  const w = windowOf({ before: '\u{1F600} ', text: 'happy day', after: '', start: 10, truncated: false })!;
  assert.deepEqual(spanOf(w), { start: 10, end: 19 }, 'the symbol before is one character, not two');
  assert.equal(windowParts(moveStart(w, -1)).words, '\u{1F600} happy day', 'a symbol is not a word, so it is taken with the step back');
});

test('a decision is told with its count, and a suggestion that could not go through says why', () => {
  assert.deepEqual(describeOutcome({ accepted: 1, rejected: 0, failed: [] }), { text: 'Accepted 1 suggestion; it is now coded.', problems: [], canUndo: true });
  assert.equal(describeOutcome({ accepted: 12, rejected: 0, failed: [] }).text, 'Accepted 12 suggestions; they are now coded.');
  assert.equal(describeOutcome({ accepted: 0, rejected: 2, failed: [] }).text, 'Rejected 2 suggestions.');
  const mixed = describeOutcome({ accepted: 3, rejected: 0, failed: [[7, 'it was already accepted'], [8, 'it was already accepted'], [9, 'the code is retired']] });
  assert.equal(mixed.text, 'Accepted 3 suggestions; they are now coded. 3 suggestions could not be accepted and stay in the queue.');
  assert.deepEqual(mixed.problems, ['it was already accepted', 'the code is retired'], 'the same reason is told once');
  const none = describeOutcome({ accepted: 0, rejected: 0, failed: [[1, 'it was already rejected']] });
  assert.equal(none.canUndo, false, 'nothing changed, so nothing to undo');
});

test('a snippet makes spaces single and cuts long passages', () => {
  assert.equal(snippet('  one \n two  '), 'one two');
  assert.equal(snippet('abcdefghij', 5), 'abcde…');
  assert.equal(snippet('\u{1F600}\u{1F600}\u{1F600}', 2), '\u{1F600}\u{1F600}…', 'a symbol is one character');
});
