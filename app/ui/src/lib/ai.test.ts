import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { AiAnswerOutcome, AiPacket, AiRunRow } from '../types';
import { emptyAiSetup, outcomeSentences, packetArgs, packetSentences, runLine, runName } from './ai';

const packet = (patch: Partial<AiPacket> = {}): AiPacket => ({
  run_id: 3,
  prompt: 'text',
  passages: 12,
  characters: 3400,
  codes: [
    { id: 1, name: 'Cost' },
    { id: 2, name: 'Access' },
  ],
  left_out: { documents_not_shared: 0, passages_not_shared: 0, already_coded: 0, too_long: 0, pictures: 0, codes: [] },
  more_after: 0,
  next_skip: 12,
  ...patch,
});

test('the form is complete by default and becomes a small request', () => {
  assert.deepEqual(packetArgs(emptyAiSetup()), { args: { only_uncoded: true, limit: 40 } });
  assert.deepEqual(packetArgs({ ...emptyAiSetup(), scope: 'documents', sourceIds: [4, 5], codeIds: [1], onlyUncoded: false, limit: ' 25 ', skip: 40 }), {
    args: { only_uncoded: false, source_ids: [4, 5], limit: 25, code_ids: [1], skip: 40 },
  });
  assert.deepEqual(packetArgs({ ...emptyAiSetup(), scope: 'case', caseId: 7, limit: '' }), { args: { only_uncoded: true, case_id: 7, limit: 40 } });
});

test('the form says what is missing or out of range', () => {
  assert.match((packetArgs({ ...emptyAiSetup(), scope: 'documents' }) as { problem: string }).problem, /at least one document/);
  assert.match((packetArgs({ ...emptyAiSetup(), scope: 'case' }) as { problem: string }).problem, /Choose a case/);
  for (const bad of ['0', '201', '2.5', 'many', '-3']) {
    assert.match((packetArgs({ ...emptyAiSetup(), limit: bad }) as { problem: string }).problem, /from 1 to 200/, bad);
  }
});

test('a packet is described by what it holds and what was kept out', () => {
  assert.deepEqual(packetSentences(packet()), ['The packet has 12 passages and 2 codes (Cost, Access), 3,400 characters in all.']);
  assert.deepEqual(packetSentences(packet({ codes: [], passages: 1, characters: 80 })), ['The packet has 1 passage, 80 characters in all.']);
  const s = packetSentences(
    packet({
      passages: 1,
      codes: [{ id: 1, name: 'Cost' }],
      characters: 1,
      more_after: 1,
      left_out: { documents_not_shared: 1, passages_not_shared: 2, already_coded: 1, too_long: 1, pictures: 2, codes: ['Secret'] },
    }),
  );
  assert.deepEqual(s, [
    'The packet has 1 passage and 1 code (Cost), 1 character in all.',
    '1 document marked do not share was left out.',
    '2 passages marked do not share were left out.',
    '1 passage you have already coded was left out.',
    '1 passage too long to quote exactly was left out.',
    '2 pictures were left out, because a packet carries words only.',
    '1 code may not be sent and was left out: Secret.',
    '1 more passage could go in another packet.',
  ]);
});

test('an answer is described by what passed, what was refused, and what repeated earlier work', () => {
  const outcome = (patch: Partial<AiAnswerOutcome>): AiAnswerOutcome => ({ read: 0, suggested: 0, rejected: [], already_coded: 0, already_waiting: 0, batch: null, ...patch });
  assert.deepEqual(outcomeSentences(outcome({})), ['The answer held no suggestions.']);
  assert.deepEqual(outcomeSentences(outcome({ read: 2, suggested: 0, rejected: [{ number: 1, passage: 'P1', code: 'Cost', why: 'x' }, { number: 2, passage: 'P2', code: 'Cost', why: 'y' }] })), [
    'The answer held 2 suggestions.',
    'None passed the check, so nothing was sent to the review queue.',
    '2 suggestions were refused by the check.',
  ]);
  assert.deepEqual(outcomeSentences(outcome({ read: 5, suggested: 1, already_coded: 1, already_waiting: 2, batch: 'ai-ab12' })), [
    'The answer held 5 suggestions.',
    '1 suggestion passed the check and is waiting in the review queue. Nothing is coded until you accept it.',
    '1 suggestion repeated coding you already did and was left out.',
    '2 suggestions were already waiting in the review queue and were left out.',
  ]);
});

test('a run is one line in the log', () => {
  const run: AiRunRow = { id: 4, route: 'packet', model: 'Test Bot', created_at: '', answered: false, passages: 12, codes: 2, read: 0, suggested: 0, checker_rejected: 0, pending: 0, accepted: 0, rejected: 0, prompt_hash: '', batch: null };
  assert.equal(runLine(run), '12 passages and 2 codes sent; no answer pasted yet');
  assert.equal(runName(run), 'Packet 4, not answered yet');
  const done = { ...run, answered: true, read: 5, suggested: 4, checker_rejected: 1, accepted: 2, rejected: 1, pending: 1 };
  assert.equal(runLine(done), '4 suggestions sent to the review queue, 1 refused by the check, 2 accepted, 1 rejected, 1 waiting');
  assert.equal(runName(done), 'Packet 4, answered by Test Bot');
});
