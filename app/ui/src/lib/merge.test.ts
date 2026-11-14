import assert from 'node:assert/strict';
import test from 'node:test';
import type { MergeReport } from '../types';
import { addsNothing, mergeSentences, skippedSentence } from './merge';

const report = (patch: Partial<MergeReport> = {}): MergeReport => ({
  documents_added: [],
  documents_matched: 0,
  documents_in_conflict: [],
  coders_added: [],
  codes_added: [],
  codes_in_conflict: [],
  codings_added: 0,
  codings_already_here: 0,
  codings_skipped: [],
  areas_added: 0,
  areas_already_here: 0,
  memos_added: 0,
  memos_already_here: 0,
  links_added: 0,
  links_already_here: 0,
  cases_added: [],
  cases_matched: 0,
  case_members_added: 0,
  variables_added: [],
  variables_in_conflict: [],
  values_added: 0,
  values_in_conflict: [],
  values_replaced: 0,
  definitions_replaced: [],
  backup: null,
  applied: false,
  ...patch,
});

test('links between passages are counted, and a merge that adds only a link is not empty', () => {
  assert.ok(mergeSentences(report({ links_added: 2, links_already_here: 1 })).includes('Links between passages added: 2. Already here: 1.'));
  assert.equal(addsNothing(report({ links_added: 1 })), false);
  assert.equal(mergeSentences(report()).some((s) => s.startsWith('Links')), false, 'no sentence when there are no links');
});

test('cases, variables, and values are said, with what differs named and the one here kept', () => {
  const r = report({
    cases_added: ['Bo'],
    case_members_added: 1,
    variables_added: ['Wave'],
    variables_in_conflict: ['Rank'],
    values_added: 4,
    values_in_conflict: [{ case: 'Ana', variable: 'Age', ours: '30', theirs: '31' }],
  });
  const s = mergeSentences(r);
  assert.ok(s.includes('Cases added: Bo.'));
  assert.ok(s.includes('1 document was put in a case here, because it is in that case in the copy.'));
  assert.ok(s.includes('Variables of cases added: Wave.'));
  assert.ok(s.includes('4 values of cases were added.'));
  assert.ok(s.some((x) => x.startsWith('The values of these variables were left out') && x.endsWith('Rank.')));
  assert.ok(s.includes('1 value of a case differs from the one in the copy, and the one here was kept: Ana, Age (here 30, in the copy 31).'));
  assert.equal(addsNothing(r), false);
  assert.equal(addsNothing(report({ cases_added: ['Bo'] })), false);
  assert.equal(addsNothing(report({ values_added: 1 })), false);
  assert.equal(addsNothing(report({ cases_matched: 2, values_in_conflict: [{ case: 'Ana', variable: 'Age', ours: '30', theirs: '31' }] })), true, 'a difference that is kept adds nothing');
  const many = report({ values_in_conflict: Array.from({ length: 7 }, (_, i) => ({ case: `C${i}`, variable: 'V', ours: 'a', theirs: 'b' })) });
  assert.ok(mergeSentences(many).some((x) => x.includes('and 2 more')), 'at most five are named');
});

test('what the person settled in favour of the copy is said, and is something a merge does', () => {
  const s = mergeSentences(report({ values_replaced: 2, definitions_replaced: ['Cost'] }));
  assert.ok(s.includes('2 values of cases were changed to the ones in the copy, as you chose.'));
  assert.ok(s.includes('Wording taken from the copy, as you chose: Cost.'));
  assert.equal(addsNothing(report({ values_replaced: 1 })), false);
  assert.equal(addsNothing(report({ definitions_replaced: ['Cost'] })), false);
});

test('a merge that brings in nothing says so, in the order a person reads it', () => {
  assert.deepEqual(mergeSentences(report()), ['No document is added.', 'No coder is added.', 'No code is added.', 'Codings added: 0. Already here: 0.']);
  assert.equal(addsNothing(report({ documents_matched: 3, codings_already_here: 10 })), true);
});

test('what is brought in is named, and counts are written with their separators', () => {
  const r = report({
    documents_added: ['Notes', 'Interview 2'],
    documents_matched: 1,
    coders_added: ['Ben'],
    codes_added: ['Staff', 'Fees > Late fees', 'Welcome'],
    codings_added: 1234,
    codings_already_here: 4,
    areas_added: 2,
    memos_added: 1,
    memos_already_here: 3,
  });
  assert.deepEqual(mergeSentences(r), [
    'Documents added: Notes and Interview 2.',
    '1 document is in both projects with the same words.',
    'Coders added: Ben.',
    'Codes added: Staff, Fees > Late fees, and Welcome.',
    'Codings added: 1,234. Already here: 4.',
    'Areas of pictures added: 2. Already here: 0.',
    'Memos added: 1. Already here: 3.',
  ]);
  assert.equal(addsNothing(r), false);
});

test('documents that clash and codings that were left out are told with their reason', () => {
  const r = report({ documents_in_conflict: ['Shared'], codings_skipped: [{ reason: 'code_retired', count: 1 }, { reason: 'no_unit', count: 3 }] });
  const s = mergeSentences(r);
  assert.ok(s.includes('Left out, because a document of the same name has other words here: Shared. Rename one of them to bring it in.'), s.join(' | '));
  assert.ok(s.includes('1 coding was left out, because its code was merged or retired in the copy.'));
  assert.ok(s.includes('3 codings were left out, because their words fit no unit of this project, which codes whole units.'));
  assert.equal(skippedSentence('something new', 2), '2 codings were left out, because this project would not take them.');
});
