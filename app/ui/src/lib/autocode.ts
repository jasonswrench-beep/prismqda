// Auto Code: turning the setup form into a request, saying in words what a run will do, and moving the edges of a
// suggestion by whole words. All of it is plain functions so it can be tested without a screen.

import type { AutocodeArgs, AutocodePreview, AutoMethod, AutoTarget, BatchSummary, ProposalRow, SpeakerChoiceArg } from '../types';
import { num, t, tn } from '../i18n';

/** What the person chose for one speaker. */
export interface SpeakerSetup {
  label: string;
  include: boolean;
  mode: 'existing' | 'new';
  codeId: number | null;
  newCode: string;
}

/** Everything on the Set up form. It is kept while the person moves between pages. */
export interface AutoSetup {
  method: AutoMethod;
  scope: 'all' | 'documents' | 'case';
  sourceIds: number[];
  caseId: number | null;
  // Words
  query: string;
  target: AutoTarget;
  around: number;
  // Words and headings: the code to suggest.
  codeMode: 'existing' | 'new';
  codeId: number | null;
  newCode: string;
  // Headings
  levels: number[];
  headingQuery: string;
  includeHeading: boolean;
  // Speakers, by the key the engine gives each speaker.
  speakers: Record<string, SpeakerSetup>;
}

export const MAX_AROUND = 50;

export function emptySetup(): AutoSetup {
  return {
    method: 'words',
    scope: 'all',
    sourceIds: [],
    caseId: null,
    query: '',
    target: 'sentence',
    around: 5,
    codeMode: 'existing',
    codeId: null,
    newCode: '',
    levels: [],
    headingQuery: '',
    includeHeading: true,
    speakers: {},
  };
}

export const METHOD_LABELS: Record<AutoMethod, string> = { get words() { return t('Words'); }, get speakers() { return t('Speakers'); }, get headings() { return t('Headings'); } };

export const TARGET_LABELS: Record<AutoTarget, string> = {
  get hit() { return t('The matched words'); },
  get words() { return t('The matched words and some words around them'); },
  get sentence() { return t('The whole sentence'); },
  get paragraph() { return t('The whole paragraph'); },
  get unit() { return t('The whole unit (a turn, a response, or a post)'); },
};

export interface Names {
  code: (id: number) => string;
  source: (id: number) => string;
  case: (id: number) => string;
  /** A speaker's name from the key the engine gave them; the key itself when this is absent. */
  speaker?: (key: string) => string;
}

function scopeArgs(s: AutoSetup): { scope: Pick<AutocodeArgs, 'source_ids' | 'case_id'> } | { problem: string } {
  if (s.scope === 'documents') {
    return s.sourceIds.length === 0 ? { problem: t('Choose at least one document, or look in every document.') } : { scope: { source_ids: s.sourceIds } };
  }
  if (s.scope === 'case') {
    return s.caseId === null ? { problem: t('Choose a case, or look in every document.') } : { scope: { case_id: s.caseId } };
  }
  return { scope: {} };
}

function codeArgs(mode: 'existing' | 'new', codeId: number | null, newCode: string, what: string): { code: Pick<AutocodeArgs, 'code_id' | 'new_code'> } | { problem: string } {
  if (mode === 'existing') return codeId === null ? { problem: t('Choose the code for {what}, or name a new one.', { what }) } : { code: { code_id: codeId } };
  return newCode.trim() === '' ? { problem: t('Type a name for the new code, or choose a code for {what}.', { what }) } : { code: { new_code: newCode.trim() } };
}

/** The request the form stands for, or the one thing still missing, in words. */
export function buildArgs(s: AutoSetup): { args: AutocodeArgs } | { problem: string } {
  const where = scopeArgs(s);
  if ('problem' in where) return where;
  switch (s.method) {
    case 'words': {
      if (s.query.trim() === '') return { problem: t('Type the words to find.') };
      if (s.target === 'words' && !(Number.isInteger(s.around) && s.around >= 1 && s.around <= MAX_AROUND)) return { problem: t('Say how many words to add each side, from 1 to {MAX_AROUND}.', { MAX_AROUND }) };
      const code = codeArgs(s.codeMode, s.codeId, s.newCode, t('these passages'));
      if ('problem' in code) return code;
      return { args: { method: 'words', query: s.query.trim(), target: s.target, ...(s.target === 'words' ? { around: s.around } : {}), ...code.code, ...where.scope } };
    }
    case 'speakers': {
      const chosen = Object.entries(s.speakers).filter(([, v]) => v.include);
      if (chosen.length === 0) return { problem: t('Choose at least one speaker.') };
      const speakers: SpeakerChoiceArg[] = [];
      for (const [key, v] of chosen) {
        const code = codeArgs(v.mode, v.codeId, v.newCode, v.label);
        if ('problem' in code) return code;
        speakers.push({ key, ...code.code });
      }
      return { args: { method: 'speakers', speakers, ...where.scope } };
    }
    case 'headings': {
      if (s.levels.length === 0) return { problem: t('Choose at least one heading level.') };
      const code = codeArgs(s.codeMode, s.codeId, s.newCode, t('these sections'));
      if ('problem' in code) return code;
      const q = s.headingQuery.trim();
      return {
        args: { method: 'headings', levels: [...s.levels].sort((a, b) => a - b), include_heading: s.includeHeading, ...(q !== '' ? { heading_query: q } : {}), ...code.code, ...where.scope },
      };
    }
  }
}

/** A stable text for a request, to tell whether a preview still matches the form. */
export function setupKey(a: AutocodeArgs): string {
  return JSON.stringify(a);
}

function joinList(items: string[], word = 'and'): string {
  if (items.length <= 1) return items.join('');
  if (items.length === 2) return `${items[0]} ${word} ${items[1]}`;
  return `${items.slice(0, -1).join(', ')}, ${word} ${items[items.length - 1]}`;
}

function codeWords(a: { code_id?: number; new_code?: string }, names: Names): string {
  return a.code_id !== undefined ? t('the code {code}', { code: names.code(a.code_id) }) : t('a new code named {new_code}', { new_code: a.new_code ?? '' });
}

/** What the request will do, as one sentence a person can check before anything is found. */
export function readback(a: AutocodeArgs, names: Names): string {
  const where = a.source_ids ? (a.source_ids.length === 1 ? t('in {name}', { name: names.source(a.source_ids[0]) }) : t('in {count} chosen documents', { count: a.source_ids.length })) : a.case_id !== undefined ? t('in the documents of the case {name}', { name: names.case(a.case_id) }) : t('in every document');
  switch (a.method) {
    case 'words': {
      const cover: Record<AutoTarget, string> = {
        hit: t('the matched words'),
        words: t('the matched words and {count} words either side', { count: a.around ?? 5 }),
        sentence: t('the whole sentence'),
        paragraph: t('the whole paragraph'),
        unit: t('the whole unit'),
      };
      return t('Find {query} {where}, and suggest {codeWords} for {value}.', { query: a.query ?? '', where, codeWords: codeWords(a, names), value: cover[a.target ?? 'hit'] });
    }
    case 'speakers': {
      const list = (a.speakers ?? []).map((s) => t('{speaker} as {code}', { speaker: names.speaker?.(s.key) ?? s.key, code: s.code_id !== undefined ? names.code(s.code_id) : (s.new_code ?? '') }));
      return t('Suggest codes for every turn {where} by {list}.', { where, list: joinList((a.speakers ?? []).length === 0 ? [t('no one yet')] : list) });
    }
    case 'headings': {
      const levels = a.levels ?? [];
      const which = levels.length === 1 ? t('level {level}', { level: levels[0] }) : t('levels {levels}', { levels: joinList(levels.map(String)) });
      const containing = a.heading_query ? ' ' + t('whose heading contains {query}', { query: a.heading_query }) : '';
      const query = a.heading_query;
      const withHeading = a.include_heading !== false;
      if (query) return withHeading ? t('Suggest {codeWords} for each section under a {which} heading whose heading contains {query} {where}, with the heading itself.', { codeWords: codeWords(a, names), which, query, where }) : t('Suggest {codeWords} for each section under a {which} heading whose heading contains {query} {where}, without the heading itself.', { codeWords: codeWords(a, names), which, query, where });
      return withHeading ? t('Suggest {codeWords} for each section under a {which} heading {where}, with the heading itself.', { codeWords: codeWords(a, names), which, where }) : t('Suggest {codeWords} for each section under a {which} heading {where}, without the heading itself.', { codeWords: codeWords(a, names), which, where });
    }
  }
}

function count(n: number, one: string, many = `${one}s`): string {
  return `${num(n)} ${n === 1 ? one : many}`;
}

/** What a preview found, one plain sentence per fact. */
export function previewSentences(p: AutocodePreview): string[] {
  const out: string[] = [];
  const unit = p.method === 'speakers' ? 'turn' : p.method === 'headings' ? 'section' : 'place';
  const be = (n: number) => (n === 1 ? 'is' : 'are');
  out.push(p.found === 0 ? t('Nothing was found.') : t('Found {count}.', { count: count(p.found, unit) }));
  if (p.already_coded > 0) out.push(tn(p.already_coded, { one: '{count} {be} already coded this way, so it is left out.', other: '{count} {be} already coded this way, so they are left out.' }, { be: be(p.already_coded) }));
  if (p.already_waiting > 0) out.push(tn(p.already_waiting, { one: '{count} {be} already waiting in the review queue, so it is left out.', other: '{count} {be} already waiting in the review queue, so they are left out.' }, { be: be(p.already_waiting) }));
  if (p.left_out > 0) out.push(t("{count} {be} left out of coding in these documents (an interviewer's, for example).", { count: tn(p.left_out, { one: '{count} turn', other: '{count} turns' }), be: be(p.left_out) }));
  if (p.truncated) out.push(t('There were more than one run can use, so only the first ones are included.'));
  out.push(p.suggestions === 0 ? t('There is nothing new to send.') : t('{count} in {count2} can be sent to the review queue.', { count: tn(p.suggestions, { one: '{count} suggestion', other: '{count} suggestions' }), count2: tn(p.documents, { one: '{count} document', other: '{count} documents' }) }));
  return out;
}

/** One line for a batch in the list of runs. */
export function batchLine(b: Pick<BatchSummary, 'pending' | 'accepted' | 'rejected'>): string {
  const parts = [t('{count} waiting', { count: num(b.pending) })];
  if (b.accepted > 0) parts.push(t('{accepted} accepted', { accepted: num(b.accepted) }));
  if (b.rejected > 0) parts.push(t('{rejected} rejected', { rejected: num(b.rejected) }));
  return parts.join(', ');
}

/** What a decision did, for the note under the queue's heading. */
export interface DecisionNote {
  text: string;
  /** Why some suggestions stayed in the queue (at most three different reasons). */
  problems: string[];
  /** Whether there is anything to undo. */
  canUndo: boolean;
}

/** A shortened passage for a button's name: spaces made single, cut at `max` characters. */
export function snippet(text: string, max = 40): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  const chars = Array.from(flat);
  return chars.length > max ? `${chars.slice(0, max).join('').trimEnd()}…` : flat;
}

/** What a decision did, in words, with the reasons any suggestion could not go through. */
export function describeOutcome(r: { accepted: number; rejected: number; failed: [number, string][] }): DecisionNote {
  const parts: string[] = [];
  if (r.accepted > 0) parts.push(tn(r.accepted, { one: 'Accepted {count}; it is now coded.', other: 'Accepted {count}; they are now coded.' }, { count: tn(r.accepted, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  if (r.rejected > 0) parts.push(t('Rejected {count}.', { count: tn(r.rejected, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  const reasons = [...new Set(r.failed.map(([, why]) => why))];
  if (r.failed.length > 0) parts.push(tn(r.failed.length, { one: '{count} could not be accepted and stays in the queue.', other: '{count} could not be accepted and stay in the queue.' }, { count: tn(r.failed.length, { one: '{count} suggestion', other: '{count} suggestions' }) }));
  if (parts.length === 0) parts.push(t('Nothing changed.'));
  return { text: parts.join(' '), problems: reasons.slice(0, 3), canUndo: r.accepted + r.rejected > 0 };
}

// ---------------------------------------------------------------- moving the edges of a suggestion

/** A suggestion's words with the text around them, and where the words start and end inside that text (in characters, not UTF-16 units). */
export interface TextWindow {
  chars: string[];
  a: number;
  b: number;
  /** The document position of the first character of `chars`. */
  origin: number;
}

const WORD = /[\p{L}\p{N}]/u;
const isWord = (c: string | undefined): boolean => c !== undefined && (WORD.test(c) || c === "'" || c === '’');

/** The window for a suggestion, or null when its words are too long to have been sent whole. */
export function windowOf(row: Pick<ProposalRow, 'before' | 'text' | 'after' | 'start' | 'truncated'>): TextWindow | null {
  if (row.truncated) return null;
  const before = Array.from(row.before);
  const text = Array.from(row.text);
  return { chars: [...before, ...text, ...Array.from(row.after)], a: before.length, b: before.length + text.length, origin: row.start - before.length };
}

export function spanOf(w: TextWindow): { start: number; end: number } {
  return { start: w.origin + w.a, end: w.origin + w.b };
}

/** Moves the start by one word: -1 takes in the word before, +1 gives up the first word. */
export function moveStart(w: TextWindow, dir: -1 | 1): TextWindow {
  let i = w.a;
  if (dir < 0) {
    while (i > 0 && !isWord(w.chars[i - 1])) i--;
    while (i > 0 && isWord(w.chars[i - 1])) i--;
  } else {
    while (i < w.b && isWord(w.chars[i])) i++;
    while (i < w.b && !isWord(w.chars[i])) i++;
    if (i >= w.b) return w;
  }
  return i === w.a ? w : { ...w, a: i };
}

/** Moves the end by one word: +1 takes in the next word, -1 gives up the last word. */
export function moveEnd(w: TextWindow, dir: -1 | 1): TextWindow {
  let i = w.b;
  if (dir > 0) {
    while (i < w.chars.length && !isWord(w.chars[i])) i++;
    while (i < w.chars.length && isWord(w.chars[i])) i++;
  } else {
    while (i > w.a && !isWord(w.chars[i - 1])) i--;
    while (i > w.a && isWord(w.chars[i - 1])) i--;
    while (i > w.a && !isWord(w.chars[i - 1])) i--;
    if (i <= w.a) return w;
  }
  return i === w.b ? w : { ...w, b: i };
}

export function windowParts(w: TextWindow): { before: string; words: string; after: string } {
  return { before: w.chars.slice(0, w.a).join(''), words: w.chars.slice(w.a, w.b).join(''), after: w.chars.slice(w.b).join('') };
}
