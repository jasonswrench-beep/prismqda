// Turns an audit event ("coding.add" with a few numbers) into a sentence a student can read:
// "Coded 'felt welcome' as Welcome". The same wording is used for the history timeline and for
// the screen-reader announcements after an action.

import type { EventRow } from '../types';
import { getLocaleState, num, t, tn } from '../i18n';
import { engineText } from '../i18n/engine';

export type EventCategory = 'coding' | 'codebook' | 'memo' | 'data' | 'project' | 'note' | 'other';

export interface WordingLookups {
  codeName(id: number | null | undefined): string | null;
  sourceName(id: number | null | undefined): string | null;
  /** The text of a span, if it is known; null if not. */
  quote(sourceId: number, start: number, end: number): string | null;
}

export interface Described {
  text: string;
  /** Extra plain words, such as which source the action was in. */
  detail: string;
  category: EventCategory;
}

export const CATEGORY_LABELS: Record<EventCategory, string> = {
  get coding() { return t('Coding'); },
  get codebook() { return t('Codebook'); },
  get memo() { return t('Memos'); },
  get data() { return t('Sources and cases'); },
  get project() { return t('Project'); },
  get note() { return t('Notes'); },
  get other() { return t('Other'); },
};

const MAX_QUOTE = 80;

/** A quote shortened for a sentence, with quotation marks. Spaces and line breaks are made single. */
export function quoted(text: string, max = MAX_QUOTE): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  const chars = Array.from(flat);
  const shown = chars.length > max ? chars.slice(0, max - 1).join('').trimEnd() + '...' : flat;
  return `'${shown}'`;
}

/** "Coded 'felt welcome' as Welcome": the sentence after a coding is made. */
export function codedSentence(quote: string | null, codeName: string): string {
  return quote ? t('Coded {quote} as {codeName}', { quote: quoted(quote), codeName }) : t('Coded some text as {codeName}', { codeName });
}

function asNumber(v: unknown): number | null {
  return typeof v === 'number' ? v : null;
}

function str(v: unknown): string {
  return typeof v === 'string' ? v : '';
}

/** True for the record of a coded stretch of a recording: its geometry is a `span`. */
function isStretch(o: { geometry?: unknown }): boolean {
  const g = o.geometry;
  return typeof g === 'object' && g !== null && (g as { shape?: unknown }).shape === 'span';
}

const FIELD_WORDS: Record<string, string> = {
  get name() { return t('name'); },
  get definition() { return t('definition'); },
  get inclusion_criteria() { return t('inclusion criteria'); },
  get exclusion_criteria() { return t('exclusion criteria'); },
  get color() { return t('color'); },
  get kind() { return t('type'); },
  get exclusive_group() { return t('group'); },
  get rated() { return t('rating setting'); },
  get rating_label() { return t('rating name'); },
  get rating_low() { return t('scale'); },
  get rating_high() { return t('scale'); },
};

function joinWords(words: string[]): string {
  if (words.length <= 1) return words.join('');
  return t('{list} and {value}', { list: words.slice(0, -1).join(', '), value: words[words.length - 1] });
}

export function categoryOf(action: string): EventCategory {
  if (action === 'annotation') return 'note';
  const area = action.split('.')[0];
  switch (area) {
    case 'coding':
    case 'region':
    case 'anchor':
    case 'counterexample':
    case 'proposal':
    case 'ai':
      return 'coding';
    case 'code':
    case 'codebook':
    case 'code_set':
      return 'codebook';
    case 'memo':
    case 'link':
      return 'memo';
    case 'source':
    case 'case':
    case 'unit':
    case 'variable':
      return 'data';
    case 'project':
    case 'coder':
      return 'project';
    default:
      return 'other';
  }
}

/**
 * Makes a list of events (oldest first) readable when many suggestions were decided at once: the coding each accepted
 * suggestion made is left out (the acceptance says it), the extra lines an undo writes for a bulk decision are left
 * out, and the decisions of one call are told as one line with their number.
 */
export function foldEvents(events: EventRow[]): EventRow[] {
  const out: EventRow[] = [];
  for (const e of events) {
    const after = e.after ?? {};
    if (e.action === 'coding.add' && after.via_proposal !== undefined && after.via_proposal !== null) continue;
    if (e.action === 'undo' && after.with !== undefined && after.with !== null) continue;
    const last = out[out.length - 1];
    const token = typeof after.decision === 'string' ? after.decision : null;
    if (token !== null && last && last.action === e.action && last.after?.decision === token) {
      out[out.length - 1] = { ...e, after: { ...after, count: (typeof last.after?.count === 'number' ? last.after.count : 1) + 1 } };
      continue;
    }
    out.push(e);
  }
  return out;
}

function plainAction(action: string): string {
  const words = action.replace(/[._]+/g, ' ').trim();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : t('Something happened');
}

export function describeEvent(e: EventRow, l: WordingLookups): Described {
  const category = categoryOf(e.action);
  const before = e.before ?? {};
  const after = e.after ?? {};
  const code = (id: unknown) => l.codeName(asNumber(id)) ?? t('a code');
  const done = (text: string, detail = ''): Described => ({ text, detail, category });

  switch (e.action) {
    case 'coding.add': {
      const sourceId = asNumber(after.source_id);
      const start = asNumber(after.start);
      const end = asNumber(after.end);
      const q = sourceId !== null && start !== null && end !== null ? l.quote(sourceId, start, end) : null;
      const ai = after.origin === 'ai_suggestion_accepted' ? ' ' + t('(accepted from an AI suggestion)') : after.origin === 'ai_applied' ? ' ' + t('(applied by AI)') : '';
      const where = l.sourceName(sourceId);
      return done(codedSentence(q, code(after.code_id)) + ai, where ? t('in {where}', { where }) : '');
    }
    case 'coding.remove': {
      const sourceId = asNumber(before.source_id);
      const start = asNumber(before.start);
      const end = asNumber(before.end);
      const q = sourceId !== null && start !== null && end !== null ? l.quote(sourceId, start, end) : null;
      const where = l.sourceName(sourceId);
      const text = q ? t('Removed the coding {quote} ({code})', { quote: quoted(q), code: code(before.code_id) }) : t('Removed a coding ({code})', { code: code(before.code_id) });
      return done(text, where ? t('in {where}', { where }) : '');
    }
    case 'coding.change_code':
      return done(t('Changed a coding from {code} to {code2}', { code: code(before.code_id), code2: code(after.code_id) }));
    case 'coding.rationale': {
      const note = str(after.rationale).trim();
      return done(note ? t('Wrote a note on a coding: {quote}', { quote: quoted(note, 60) }) : t('Cleared the note on a coding'));
    }
    case 'region.add': {
      const where = l.sourceName(asNumber(after.source_id));
      const text = isStretch(after) ? t('Coded a stretch of a recording as {code}', { code: code(after.code_id) }) : t('Coded an area of a picture as {code}', { code: code(after.code_id) });
      return done(text, where ? t('in {where}', { where }) : '');
    }
    case 'region.remove': {
      const where = l.sourceName(asNumber(before.source_id));
      const text = isStretch(before) ? t('Removed a coded stretch of a recording ({code})', { code: code(before.code_id) }) : t('Removed a coded area of a picture ({code})', { code: code(before.code_id) });
      return done(text, where ? t('in {where}', { where }) : '');
    }
    case 'region.change_code':
      return done(t('Changed a coded area from {code} to {code2}', { code: code(before.code_id), code2: code(after.code_id) }));
    case 'region.reshape':
      return done(isStretch(after) ? t('Moved a coded stretch of a recording') : t('Moved or resized a coded area of a picture'));
    case 'region.rationale': {
      const note = str(after.rationale).trim();
      return done(note ? t('Wrote a note on a coded area: {quote}', { quote: quoted(note, 60) }) : t('Cleared the note on a coded area'));
    }
    case 'region.rating':
      return done(after.rating === null || after.rating === undefined ? t('Cleared the rating of a coded area') : t('Rated a coded area {rating} out of 5', { rating: String(after.rating) }));
    case 'anchor.add':
      return done(t('Marked a coding as an example for {code}', { code: code(after.code_id) }));
    case 'anchor.remove':
      return done(t('Removed a coding from the examples for {code}', { code: code(after.code_id) }));
    case 'counterexample.add':
      return done(t('Marked a coding as a counterexample of {code}', { code: code(after.code_id) }));
    case 'counterexample.remove':
      return done(t('Took a counterexample away from {code}', { code: code(after.code_id) }));
    case 'code.add': {
      const parent = asNumber(after.parent_id);
      const name = str(after.name) || code(e.target_id);
      const what = after.kind === 'theme' ? 'theme' : 'code';
      return done(parent !== null ? t('Added the {what} {name} under {code}', { what, name, code: code(parent) }) : t('Added the {what} {name}', { what, name }));
    }
    case 'code.update': {
      const oldName = str(before.name);
      const newName = str(after.name);
      const changed = Object.keys(FIELD_WORDS).filter((k) => k !== 'name' && JSON.stringify(before[k]) !== JSON.stringify(after[k])).map((k) => FIELD_WORDS[k]);
      if (oldName && newName && oldName !== newName) {
        return done(changed.length ? t('Renamed the code {oldName} to {newName} and changed its {what}', { oldName, newName, what: joinWords(changed) }) : t('Renamed the code {oldName} to {newName}', { oldName, newName }));
      }
      return done(changed.length ? t('Edited the code {name}: changed its {what}', { name: newName || oldName || code(e.target_id), what: joinWords(changed) }) : t('Edited the code {name}', { name: newName || oldName || code(e.target_id) }));
    }
    case 'code.move': {
      const parent = asNumber(after.parent_id);
      const name = code(e.target_id);
      const sameParent = asNumber(before.parent_id) === parent;
      if (sameParent) return done(t('Moved the code {name} to a new place among its neighbors', { name }));
      return done(parent === null ? t('Moved the code {name} to the top level', { name }) : t('Moved the code {name} under {code}', { name, code: code(parent) }));
    }
    case 'code.retire':
      return done(t('Retired the code {code}', { code: code(e.target_id) }));
    case 'code.restore':
      return done(t('Restored the code {code}', { code: code(e.target_id) }));
    case 'code.merge': {
      const moved = Array.isArray(after.coding_ids) ? after.coding_ids.length : 0;
      const into = str(after.merged_into_name) || code(after.merged_into);
      const from = str(before.name) || code(e.target_id);
      return done(t('Merged the code {from} into {into}', { from, into }), moved === 1 ? t('1 coding moved') : t('{moved} codings moved', { moved }));
    }
    case 'code.split': {
      const moved = Array.isArray(after.coding_ids) ? after.coding_ids.length : 0;
      return done(t('Split the code {code}: {code2} is the new code', { code: code(e.target_id), code2: code(after.new_code_id) }), moved === 1 ? t('1 coding moved') : t('{moved} codings moved', { moved }));
    }
    case 'codebook.snapshot': {
      const note = str(after.note);
      const version = asNumber(after.version);
      const title = version === null ? t('Saved codebook version') : t('Saved codebook version {version}', { version });
      return done(note ? t('{title}: {note}', { title, note }) : title, tn(asNumber(after.codes) ?? 0, { one: '{count} code', other: '{count} codes' }));
    }
    case 'memo.add': {
      const kind = memoKindWord(str(after.kind));
      const title = str(after.title);
      return done(title ? t('Added {kind} memo: {title}', { kind, title: quoted(title, 60) }) : t('Added {kind} memo', { kind }));
    }
    case 'memo.update': {
      const title = str(after.title) || str(before.title);
      return done(title ? t('Edited a memo: {title}', { title: quoted(title, 60) }) : t('Edited a memo'));
    }
    case 'memo.delete': {
      const title = str(before.title);
      return done(title ? t('Deleted a memo: {title}', { title: quoted(title, 60) }) : t('Deleted a memo'));
    }
    case 'source.import': {
      const name = str(after.name);
      const chars = asNumber(after.characters);
      return done(name ? t('Imported the source {name}', { name: quoted(name, 60) }) : t('Imported the source'), chars !== null ? tn(chars, { one: '{count} character', other: '{count} characters' }) : '');
    }
    case 'case.import':
      return done(str(after.name) ? t('Imported a case: {name}', { name: quoted(str(after.name), 60) }) : t('Imported a case'));
    case 'unit.add':
      return done(t('Added an analysis unit'));
    case 'coder.add':
      return done(str(after.name) ? t('Added a coder: {name}', { name: str(after.name) }) : t('Added a coder'));
    case 'project.create':
      return done(str(after.name) ? t('Created the project {name}', { name: quoted(str(after.name), 60) }) : t('Created the project'));
    case 'project.migrate':
      return done(t('Updated the project file to the newest format'));
    case 'annotation': {
      const note = str(after.note).trim();
      return done(note ? t('Note: {note}', { note }) : t('Added a note'));
    }
    case 'proposal.create': {
      const n = asNumber(after.suggestions) ?? 0;
      const label = str(after.label);
      return done(label ? tn(n, { one: 'Auto Code suggested a code for {count} place ({label})', other: 'Auto Code suggested a code for {count} places ({label})' }, { label: quoted(label, 60) }) : tn(n, { one: 'Auto Code suggested a code for {count} place', other: 'Auto Code suggested a code for {count} places' }));
    }
    case 'proposal.accept': {
      const n = asNumber(after.count) ?? 1;
      return done(n > 1 ? t('Accepted {count} suggested codings', { count: num(n) }) : t('Accepted a suggested coding'));
    }
    case 'proposal.reject': {
      const n = asNumber(after.count) ?? 1;
      return done(n > 1 ? t('Rejected {count} suggested codings', { count: num(n) }) : t('Rejected a suggested coding'));
    }
    case 'proposal.adjust':
      return done(t('Changed a suggestion before reviewing it'));
    case 'query.save':
      return done(str(after.name) ? t('Saved the query {name}', { name: str(after.name) }) : t('Saved the query under a name'));
    case 'query.update':
      return done(str(after.name) || str(before.name) ? t('Changed the saved query {name}', { name: str(after.name) || str(before.name) }) : t('Changed the saved query'));
    case 'query.delete':
      return done(str(before.name) ? t('Deleted the saved query {name}', { name: str(before.name) }) : t('Deleted the saved query'));
    case 'query.freeze': {
      const n = asNumber(after.codings);
      return done(n === null ? t('Froze a saved query') : tn(n, { one: 'Froze a saved query at {count} coding', other: 'Froze a saved query at {count} codings' }));
    }
    case 'query.unfreeze':
      return done(t('Let a frozen query follow the coding again'));
    case 'source.edit': {
      const where = l.sourceName(e.target_id);
      const moved = asNumber(after.codings_moved) ?? 0;
      const removed = asNumber(after.codings_removed) ?? 0;
      return done(t('Edited the text of a document'), where ? tn(moved, { one: '{where}: {count} coding moved, {removed} removed', other: '{where}: {count} codings moved, {removed} removed' }, { where, removed: num(removed) }) : tn(moved, { one: '{count} coding moved, {removed} removed', other: '{count} codings moved, {removed} removed' }, { removed: num(removed) }));
    }
    case 'source.revise': {
      const where = l.sourceName(e.target_id);
      const file = str(after.from_file);
      const moved = asNumber(after.codings_moved) ?? 0;
      const removed = asNumber(after.codings_removed) ?? 0;
      return done((where ? (file ? t('Replaced the text of {name} with a newer version ({file})', { name: quoted(where, 60), file }) : t('Replaced the text of {name} with a newer version', { name: quoted(where, 60) })) : file ? t('Replaced the text of a document with a newer version ({file})', { file }) : t('Replaced the text of a document with a newer version')), tn(moved, { one: '{count} coding moved, {removed} removed', other: '{count} codings moved, {removed} removed' }, { removed: num(removed) }));
    }
    case 'source.share': {
      const where = l.sourceName(e.target_id);
      return done(after.do_not_share === true ? (where ? t('Marked {name} as never to be put in an AI prompt packet', { name: quoted(where, 60) }) : t('Marked a document as never to be put in an AI prompt packet')) : where ? t('Allowed {name} in AI prompt packets again', { name: quoted(where, 60) }) : t('Allowed a document in AI prompt packets again'));
    }
    case 'project.ai_mode':
      return done(str(after.ai_mode) === 'off' ? t('Switched AI prompt packets off for this project') : t('Allowed AI prompt packets in this project'));
    case 'ai.packet': {
      const n = asNumber(after.passages) ?? 0;
      const codes = asNumber(after.codes) ?? 0;
      return done(t('Built an AI prompt packet of {passages} and {codes}', { passages: tn(n, { one: '{count} passage', other: '{count} passages' }), codes: tn(codes, { one: '{count} code', other: '{count} codes' }) }), t('nothing was sent anywhere'));
    }
    case 'ai.answer': {
      const model = str(after.model);
      const suggested = asNumber(after.suggested) ?? 0;
      const rejected = asNumber(after.rejected) ?? 0;
      return done(model ? t('Pasted an answer from {model} to an AI prompt packet', { model }) : t('Pasted an answer to an AI prompt packet'), t('{suggested} passed the check, {refused} refused', { suggested: num(suggested), refused: num(rejected) }));
    }
    case 'undo': {
      const what = str(after.what);
      return done(what ? t('Undid: {what}', { what: engineText(what) }) : t('Undid the last change'));
    }
    case 'case.add': {
      const name = str(after.name);
      return done(name ? t('Added the case {name}', { name: quoted(name, 60) }) : t('Added a case'));
    }
    case 'variable.apply': {
      const was = before.def && typeof before.def === 'object' ? (before.def as Record<string, unknown>) : null;
      const now = after.def && typeof after.def === 'object' ? (after.def as Record<string, unknown>) : null;
      const values = Array.isArray(after.values) ? after.values.length : 0;
      if (now === null) return done(was ? t('Deleted the variable {name}', { name: quoted(str(was.name), 60) }) : t('Deleted a variable'));
      const name = quoted(str(now.name), 60);
      if (was === null) return values > 0 ? done(t('Made the variable {name}', { name }), tn(values, { one: '{count} case has a value', other: '{count} cases have a value' })) : done(t('Added the variable {name}', { name }));
      if (str(was.name) !== str(now.name)) return done(t('Renamed the variable {old} to {name}', { old: quoted(str(was.name), 60), name }));
      if (values === 0) return done(t('Changed what the variable {name} says', { name }));
      if (now.derived !== null && now.derived !== undefined) return done(t('Made the variable {name} again', { name }), tn(values, { one: '{count} case has a value', other: '{count} cases have a value' }));
      return done(tn(values, { one: 'Changed {name} for one case', other: 'Changed {name} for {count} cases' }, { name }));
    }
    case 'case.assign':
      return done(t('Put a document in a case'));
    case 'case.unassign':
      return done(t('Took a document out of a case'));
    case 'code_set.add':
      return done(t('Made a set of codes'));
    case 'code_set.member':
      return done(t('Changed the codes in a set of codes'));
    case 'coding.rating':
      return done(after.rating === null || after.rating === undefined ? t('Cleared the rating of a coding') : t('Rated a coding {rating} out of 5', { rating: String(after.rating) }));
    case 'map.node.add':
      return done(t('Put an item on a concept map'));
    case 'map.node.move':
      return done(t('Moved an item on a concept map'));
    case 'map.node.update':
      return done(t('Changed the words of a note on a concept map'));
    case 'map.node.delete':
      return done(t('Took an item off a concept map'));
    case 'map.link.add':
      return done(t('Joined two items on a concept map'));
    case 'map.link.update':
      return done(t('Changed the words of a link on a concept map'));
    case 'map.link.delete':
      return done(t('Took a link off a concept map'));
    case 'map.create':
      return done(t('Made a concept map'));
    case 'map.delete':
      return done(t('Deleted a concept map'));
    case 'map.update':
      return done(t('Changed a concept map'));
    case 'map.layout':
      return done(t('Arranged the items of a concept map'));
    case 'map.add_codes':
      return done(t('Put codes on a concept map'));
    case 'media.link':
      return done(t('Linked a recording to a document'));
    case 'media.unlink':
      return done(t('Took the link to a recording off a document'));
    case 'phase.move':
      return done(t('Moved a phase of the project'));
    case 'phase.tick':
      return done(t('Ticked off a step of a phase'));
    case 'reliability.run':
      return done(t('Ran an agreement calculation'));
    case 'reliability.archive':
      return done(t('Put an agreement calculation in the archive'));
    case 'template.set':
      return done(t('Chose a method template'));
    default:
      return done(plainAction(e.action));
  }
}

function memoKindWord(kind: string): string {
  switch (kind) {
    case 'analytic':
      return t('an analytic');
    case 'reflexive':
      return t('a reflexive journal');
    case 'method_decision':
      return t('a method decision');
    case 'todo':
      return t('a to-do');
    default:
      return t('a');
  }
}

/** A readable date and time such as "Nov 4, 2026, 3:05 PM", in the viewer's own time zone. */
export function formatWhen(ts: string, locale?: string, timeZone?: string): string {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return new Intl.DateTimeFormat(locale ?? getLocaleState().tag, { dateStyle: 'medium', timeStyle: 'short', timeZone }).format(d);
}
