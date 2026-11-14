// AI through prompt packets: turning the form into a request, and saying in words what a packet holds, what was left out of it,
// and what became of an answer. Plain functions, so they can be tested without a screen.

import type { AiAnswerOutcome, AiPacket, AiRunRow, PacketArgs } from '../types';
import { num, t, tn } from '../i18n';

export const DEFAULT_LIMIT = 40;
export const MAX_LIMIT = 200;

/** Everything on the packet form. It is kept while the person moves between pages. */
export interface AiSetup {
  scope: 'all' | 'documents' | 'case';
  sourceIds: number[];
  caseId: number | null;
  /** The codes to ask about; none chosen means every code that may be sent. */
  codeIds: number[];
  onlyUncoded: boolean;
  /** How many passages, as typed. */
  limit: string;
  /** How many passages to pass over first (set when going on from an earlier packet). */
  skip: number;
}

export function emptyAiSetup(): AiSetup {
  return { scope: 'all', sourceIds: [], caseId: null, codeIds: [], onlyUncoded: true, limit: String(DEFAULT_LIMIT), skip: 0 };
}

const count = (n: number, one: string, many = `${one}s`): string => `${num(n)} ${n === 1 ? one : many}`;

/** The request the form stands for, or the one thing still missing, in words. */
export function packetArgs(s: AiSetup): { args: PacketArgs } | { problem: string } {
  const args: PacketArgs = { only_uncoded: s.onlyUncoded };
  if (s.scope === 'documents') {
    if (s.sourceIds.length === 0) return { problem: t('Choose at least one document, or use every document.') };
    args.source_ids = s.sourceIds;
  }
  if (s.scope === 'case') {
    if (s.caseId === null) return { problem: t('Choose a case, or use every document.') };
    args.case_id = s.caseId;
  }
  const typed = s.limit.trim();
  const n = typed === '' ? DEFAULT_LIMIT : Number(typed);
  if (!Number.isInteger(n) || n < 1 || n > MAX_LIMIT) return { problem: t('Say how many passages to send, a whole number from 1 to {MAX_LIMIT}.', { MAX_LIMIT }) };
  args.limit = n;
  if (s.codeIds.length > 0) args.code_ids = s.codeIds;
  if (s.skip > 0) args.skip = s.skip;
  return { args };
}

/** What a packet holds and what was kept out of it, one fact to a sentence. */
export function packetSentences(p: AiPacket): string[] {
  const out: string[] = [];
  // A packet reopened from the log has its text but not the names of its codes.
  out.push(
    p.codes.length === 0
      ? t('The packet has {count}, {count2} in all.', { count: tn(p.passages, { one: '{count} passage', other: '{count} passages' }), count2: tn(p.characters, { one: '{count} character', other: '{count} characters' }) })
      : t('The packet has {count} and {count2} ({list}), {count3} in all.', { count: tn(p.passages, { one: '{count} passage', other: '{count} passages' }), count2: tn(p.codes.length, { one: '{count} code', other: '{count} codes' }), list: p.codes.map((c) => c.name).join(', '), count3: tn(p.characters, { one: '{count} character', other: '{count} characters' }) }),
  );
  const l = p.left_out;
  if (l.documents_not_shared > 0) out.push(tn(l.documents_not_shared, { one: '{count} marked do not share was left out.', other: '{count} marked do not share were left out.' }, { count: tn(l.documents_not_shared, { one: '{count} document', other: '{count} documents' }) }));
  if (l.passages_not_shared > 0) out.push(tn(l.passages_not_shared, { one: '{count} marked do not share was left out.', other: '{count} marked do not share were left out.' }, { count: tn(l.passages_not_shared, { one: '{count} passage', other: '{count} passages' }) }));
  if (l.already_coded > 0) out.push(tn(l.already_coded, { one: '{count} you have already coded was left out.', other: '{count} you have already coded were left out.' }, { count: tn(l.already_coded, { one: '{count} passage', other: '{count} passages' }) }));
  if (l.too_long > 0) out.push(tn(l.too_long, { one: '{count} too long to quote exactly was left out.', other: '{count} too long to quote exactly were left out.' }, { count: tn(l.too_long, { one: '{count} passage', other: '{count} passages' }) }));
  if (l.pictures > 0) out.push(tn(l.pictures, { one: '{count} was left out, because a packet carries words only.', other: '{count} were left out, because a packet carries words only.' }, { count: tn(l.pictures, { one: '{count} picture', other: '{count} pictures' }) }));
  if (l.codes.length > 0) out.push(tn(l.codes.length, { one: '{count} may not be sent and was left out: {list}.', other: '{count} may not be sent and were left out: {list}.' }, { count: tn(l.codes.length, { one: '{count} code', other: '{count} codes' }), list: l.codes.join(', ') }));
  if (p.more_after > 0) out.push(t('{count} could go in another packet.', { count: tn(p.more_after, { one: '{count} more passage', other: '{count} more passages' }) }));
  return out;
}

/** What became of a pasted answer. */
export function outcomeSentences(o: AiAnswerOutcome): string[] {
  const out: string[] = [];
  out.push(o.read === 0 ? t('The answer held no suggestions.') : t('The answer held {count}.', { count: tn(o.read, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  if (o.suggested > 0) out.push(tn(o.suggested, { one: '{count} passed the check and is waiting in the review queue. Nothing is coded until you accept it.', other: '{count} passed the check and are waiting in the review queue. Nothing is coded until you accept it.' }, { count: tn(o.suggested, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  else if (o.read > 0) out.push(t('None passed the check, so nothing was sent to the review queue.'));
  if (o.rejected.length > 0) out.push(tn(o.rejected.length, { one: '{count} was refused by the check.', other: '{count} were refused by the check.' }, { count: tn(o.rejected.length, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  if (o.already_coded > 0) out.push(tn(o.already_coded, { one: '{count} repeated coding you already did and was left out.', other: '{count} repeated coding you already did and were left out.' }, { count: tn(o.already_coded, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  if (o.already_waiting > 0) out.push(tn(o.already_waiting, { one: '{count} was already waiting in the review queue and was left out.', other: '{count} were already waiting in the review queue and were left out.' }, { count: tn(o.already_waiting, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  return out;
}

/** One line for a run in the log. */
export function runLine(r: AiRunRow): string {
  if (!r.answered) return t('{count} and {count2} sent; no answer pasted yet', { count: tn(r.passages, { one: '{count} passage', other: '{count} passages' }), count2: tn(r.codes, { one: '{count} code', other: '{count} codes' }) });
  const parts = [t('{suggestions} sent to the review queue', { suggestions: tn(r.suggested, { one: '{count} suggestion', other: '{count} suggestions' }) })];
  if (r.checker_rejected > 0) parts.push(t('{checker_rejected} refused by the check', { checker_rejected: num(r.checker_rejected) }));
  parts.push(t('{accepted} accepted', { accepted: num(r.accepted) }), t('{rejected} rejected', { rejected: num(r.rejected) }), t('{pending} waiting', { pending: num(r.pending) }));
  return parts.join(', ');
}

/** One refused suggestion, told by its number and, when it has them, the passage and code it named. */
export function suggestionLine(r: { number: number; passage?: string | number | null; code?: string | number | null; why: string }): string {
  if (r.passage && r.code) return t('Suggestion {number}, passage {passage}, code {code}: {why}.', { number: r.number, passage: r.passage, code: r.code, why: r.why });
  if (r.passage) return t('Suggestion {number}, passage {passage}: {why}.', { number: r.number, passage: r.passage, why: r.why });
  if (r.code) return t('Suggestion {number}, code {code}: {why}.', { number: r.number, code: r.code, why: r.why });
  return t('Suggestion {number}: {why}.', { number: r.number, why: r.why });
}

/** A short name for a run: the model, or what it still lacks. */
export function runName(r: Pick<AiRunRow, 'id' | 'model' | 'answered'>): string {
  return r.answered ? t('Packet {id}, answered by {model}', { id: r.id, model: r.model }) : t('Packet {id}, not answered yet', { id: r.id });
}
