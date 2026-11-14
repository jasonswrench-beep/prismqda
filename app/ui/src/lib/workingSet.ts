// The working set (decision 163): the documents, cases, and codes the person has chosen to work with, switched on once so that every
// tool that can follow does. It narrows what is asked of the engine, never what the project holds: nothing is hidden or deleted, and
// "show everything" puts every request back as it was written.
//
// It is applied to the arguments of a command on their way to the engine (here, one place), by command name, because each tool already
// takes a document or case scope of its own. A request that has to see everything (the examples of a code, the evidence behind a number, the
// passages a link can go to) is not narrowed: Retrieved Segments and the document portrait opt in with `workingSet: true`.

import { t, tn } from '../i18n';

/** What to work with. `null` for an area means every one of it; an empty list means none. */
export interface WorkingSet {
  sourceIds: number[] | null;
  caseIds: number[] | null;
  codeIds: number[] | null;
  /** Use the final layer (the decisions, and what the coders agreed on) instead of every coder's own codings, where a page can (decision 173). */
  finalLayer?: boolean;
}

export const EVERYTHING: WorkingSet = { sourceIds: null, caseIds: null, codeIds: null };

/** True when anything is narrowed. */
export function narrows(w: WorkingSet): boolean {
  return w.sourceIds !== null || w.caseIds !== null || w.codeIds !== null || w.finalLayer === true;
}

/** The documents a case holds, for the tools that take documents and not cases. */
export interface CaseDocs {
  id: number;
  source_ids?: number[];
}

/**
 * Thrown when the working set leaves nothing for a request to ask about and the engine would read an empty list as "everything" (the words of
 * every code, a packet of every code). The request is refused in words instead of widening the question.
 */
export class WorkingSetEmpty extends Error {
  constructor() {
    super('the working set leaves no code to ask about');
    this.name = 'WorkingSetEmpty';
  }
}

/** What is in both: the list the request already had, cut to the working set; the working set itself when the request had none. */
function meet(existing: unknown, allowed: number[] | null): number[] | undefined {
  const had = Array.isArray(existing) ? (existing as number[]) : undefined;
  if (allowed === null) return had;
  if (had === undefined) return allowed;
  const ok = new Set(allowed);
  return had.filter((x) => ok.has(x));
}

/** Like `meet`, for a list that the engine reads as "every one" when it is empty: an empty result is refused, not sent. */
function meetUnlessEmpty(existing: unknown, allowed: number[] | null): number[] | undefined {
  const had = Array.isArray(existing) ? (existing as number[]) : undefined;
  if (allowed === null) return had;
  const out = meet(had !== undefined && had.length === 0 ? undefined : had, allowed);
  if (out !== undefined && out.length === 0) throw new WorkingSetEmpty();
  return out;
}

/** The documents the pages that read documents should look at: the chosen documents, and, when cases are chosen, the documents those cases hold. */
export function documentsOf(w: WorkingSet, cases: readonly CaseDocs[]): number[] | null {
  if (w.caseIds === null) return w.sourceIds;
  const wanted = new Set(w.caseIds);
  const held = [...new Set(cases.filter((c) => wanted.has(c.id)).flatMap((c) => c.source_ids ?? []))];
  if (w.sourceIds === null) return held;
  const chosen = new Set(w.sourceIds);
  return held.filter((x) => chosen.has(x));
}

type Args = Record<string, unknown>;

function scoped(scope: unknown, w: WorkingSet): Args {
  const s: Args = { ...((scope as Args | undefined) ?? {}) };
  if (w.sourceIds !== null) s.source_ids = meet(s.source_ids, w.sourceIds);
  if (w.caseIds !== null) s.case_ids = meet(s.case_ids, w.caseIds);
  if (w.finalLayer === true && s.layer === undefined) s.layer = 'final';
  return s;
}

function specWithScope(spec: unknown, w: WorkingSet, emptyMeansAll: boolean): Args {
  const s: Args = { ...((spec as Args | undefined) ?? {}) };
  s.scope = scoped(s.scope, w);
  if (w.codeIds !== null && (Array.isArray(s.code_ids) || emptyMeansAll)) s.code_ids = emptyMeansAll ? meetUnlessEmpty(s.code_ids, w.codeIds) : meet(s.code_ids, w.codeIds);
  return s;
}

/** Commands whose arguments the working set rewrites. */
export const FOLLOWED = [
  'segments.list',
  'search.run',
  'analytics.words',
  'analytics.kwic',
  'ai.packet',
  'sample.draw',
  'explore.summary',
  'explore.profile',
  'theme.overview',
  'explore.matrix',
  'explore.matrix_evidence',
  'explore.relations',
  'explore.relations_evidence',
  'explore.query',
  'explore.crosstab',
  'explore.crosstab_cases',
  'stats.run',
  'stats.notebook.run',
] as const;

/**
 * The arguments of a command, narrowed to the working set. Anything it does not reach is returned as it came. `cases` are the cases of the
 * project, so that a set of cases can narrow the tools that read documents. Throws `WorkingSetEmpty` where an empty code list would be read as
 * every code.
 */
export function applyWorkingSet(cmd: string, args: unknown, w: WorkingSet, cases: readonly CaseDocs[] = []): unknown {
  if (typeof args !== 'object' || args === null) return args;
  const a: Args = { ...(args as Args) };
  // `segments.list` is narrowed only for the pages that ask (Retrieved Segments, the document portrait); the marker never reaches the engine.
  let follow = true;
  if (cmd === 'segments.list') {
    follow = a.workingSet === true;
    delete a.workingSet;
  }
  if (!narrows(w) || !follow) return cmd === 'segments.list' ? a : args;
  const docs = documentsOf(w, cases);
  switch (cmd) {
    case 'segments.list':
      if (docs !== null) a.source_ids = meet(a.source_ids, docs);
      if (w.codeIds !== null) {
        const asked = Array.isArray(a.code_ids);
        a.code_ids = meet(a.code_ids, w.codeIds);
        // When no code was asked for, the working codes are asked for exactly: a parent that is in the set does not bring in sub-codes that are not.
        if (!asked) a.exact_codes = true;
      }
      if (w.finalLayer === true && a.layer === undefined) a.layer = 'final';
      return a;
    case 'search.run':
    case 'sample.draw':
    case 'analytics.kwic':
      if (docs !== null) a.source_ids = meet(a.source_ids, docs);
      return a;
    case 'analytics.words':
    case 'ai.packet':
      if (docs !== null) a.source_ids = meet(a.source_ids, docs);
      if (w.codeIds !== null) a.code_ids = meetUnlessEmpty(a.code_ids, w.codeIds);
      return a;
    case 'explore.summary':
    case 'explore.profile':
    case 'theme.overview':
      a.scope = scoped(a.scope, w);
      return a;
    case 'explore.matrix':
    case 'explore.query':
      return specWithScope(a, w, false);
    case 'explore.relations':
      return specWithScope(a, w, true);
    case 'explore.matrix_evidence':
      a.spec = specWithScope(a.spec, w, false);
      return a;
    case 'explore.relations_evidence':
      a.spec = specWithScope(a.spec, w, true);
      return a;
    case 'explore.crosstab':
      if (w.caseIds !== null) a.case_ids = meet(a.case_ids, w.caseIds);
      return a;
    case 'explore.crosstab_cases':
      if (w.caseIds !== null) a.spec = { ...((a.spec as Args | undefined) ?? {}), case_ids: meet((a.spec as Args | undefined)?.case_ids, w.caseIds) };
      return a;
    case 'stats.run':
      if (w.caseIds !== null) a.options = { ...((a.options as Args | undefined) ?? {}), case_ids: meet((a.options as Args | undefined)?.case_ids, w.caseIds) };
      return a;
    case 'stats.notebook.run': {
      const request: Args = { ...((a.request as Args | undefined) ?? {}) };
      if (w.caseIds !== null) request.options = { ...((request.options as Args | undefined) ?? {}), case_ids: meet((request.options as Args | undefined)?.case_ids, w.caseIds) };
      a.request = request;
      return a;
    }
    default:
      return args;
  }
}

/** The totals the sentences count against. */
export interface Totals {
  sources: number;
  cases: number;
  codes: number;
}

/** What is narrowed, in words, one part for each area that is: "3 of 12 documents". */
export function describeWorkingSet(w: WorkingSet, totals: Totals): string[] {
  const out: string[] = [];
  if (w.sourceIds !== null) out.push(tn(totals.sources, { one: '{n} of {count} document', other: '{n} of {count} documents' }, { n: w.sourceIds.length }));
  if (w.caseIds !== null) out.push(tn(totals.cases, { one: '{n} of {count} case', other: '{n} of {count} cases' }, { n: w.caseIds.length }));
  if (w.codeIds !== null) out.push(tn(totals.codes, { one: '{n} of {count} code', other: '{n} of {count} codes' }, { n: w.codeIds.length }));
  if (w.finalLayer === true) out.push(t('the final layer'));
  return out;
}

/** Drops what is no longer in the project (a document removed, a code retired) so the set never names a thing that is gone. */
export function pruneWorkingSet(w: WorkingSet, live: { sources: readonly number[]; cases: readonly number[]; codes: readonly number[] }): WorkingSet {
  const keep = (ids: number[] | null, alive: readonly number[]): number[] | null => {
    if (ids === null) return null;
    const ok = new Set(alive);
    return ids.filter((x) => ok.has(x));
  };
  return { sourceIds: keep(w.sourceIds, live.sources), caseIds: keep(w.caseIds, live.cases), codeIds: keep(w.codeIds, live.codes), ...(w.finalLayer === undefined ? {} : { finalLayer: w.finalLayer }) };
}

/** The sentence under the bar that says which pages follow the set. */
export function followedNote(): string {
  return t(
    'Retrieved Segments, Explore (summaries, matrices, relations, queries, cross-tabulations), the Themes page, Text Analytics, Statistics, the AI Assistant, and the sample for agreement follow it; a set of cases narrows the pages that read documents to the documents those cases hold. The Data table, the Team page, the Codebook Editor, exports, and reports do not. A page that shows results from before you changed the set (a query, a word count, a prompt packet) keeps them until you run it again, and says so. Choosing the final layer makes Retrieved Segments and Explore use the decisions made in Team > Disagreements and the codes the coders agreed on, instead of every coder\'s own codings.',
  );
}
