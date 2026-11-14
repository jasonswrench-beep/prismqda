// The Query Builder: a research question as a rule that can be read back in words before it is run, and results that open at their source.

import { useMemo, useState } from 'react';
import { Button, Checkbox, Input, Label, TextField } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import {
  CLAUSE_TYPES, addTo, clausesOf, codesOfDraft, draftKey, emptyDraft, mapItem, newClause, newGroup, problemWith, readback, toQuerySpec, type Clause, type ClauseType, type Draft, type Group,
} from '../../lib/explore';
import { goExplore, openAt, setExploreMatrix, setExploreQuery } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import { Notice, plural } from '../common';
import { useCaseVariables } from './Matrix';
import type { CaseVariable } from '../../types';
import { SegmentQuote } from '../SegmentQuote';
import { StaleSet } from '../WorkingSet';
import { SavedQueries } from './SavedQueries';
import { CheckPicker, codeChoices, resultCount, useNames } from './shared';
import { num, t, tn } from '../../i18n';

const typeOf = (c: Clause): ClauseType => c.type;

export function QueryBuilder() {
  const { draft, ran, saved } = useApp((s) => s.exploreQuery);
  const project = useApp((s) => s.project);
  const names = useNames();
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const problem = problemWith(draft);
  const key = problem ? null : draftKey(draft);
  const stale = ran !== null && key !== ran.key;
  const words = useMemo(() => readback(draft, names, project ? { codings: project.codings, documents: project.sources } : undefined), [draft, names, project]);

  const update = (next: Draft) => setExploreQuery({ draft: next });
  // The top group is not inside any group, so it is changed directly; everything else is found by its number.
  const change = (id: number, fn: (c: Clause | Group) => Clause | Group | null) => {
    if (id === draft.root.id) {
      const next = fn(draft.root);
      if (next !== null && next.kind === 'group') update({ ...draft, root: next });
      return;
    }
    update({ ...draft, root: mapItem(draft.root, id, fn) });
  };

  // Opened from a frozen saved query, the answer is the one it froze; asking the question afresh is a separate button.
  const frozen = saved?.frozen === true;
  const run = async (live = false) => {
    if (problem) {
      announce(problem, 'assertive');
      return;
    }
    const startKey = draftKey(draft);
    const setThen = appStore.get().workingSet;
    setRunning(true);
    setError(null);
    try {
      const result = frozen && !live && saved ? (await call('query.run_saved', { id: saved.id })).result : await call('explore.query', toQuerySpec(draft));
      // The key is the draft this answer is for, so an answer to an earlier draft is marked as one.
      setExploreQuery({ ran: { key: startKey, result, at: Date.now(), workingSet: setThen } });
      announce(result.truncated ? t('{results} found; the list is cut.', { results: resultCount(result.total, draft.unit) }) : t('{results} found.', { results: resultCount(result.total, draft.unit) }));
    } catch (e) {
      setError(toApiError(e).message);
      announce(toApiError(e).message, 'assertive');
    } finally {
      setRunning(false);
    }
  };

  const codeIds = codesOfDraft(draft);
  return (
    <div className="ex-query" data-testid="query-tool">
      <section className="ex-query-build" aria-labelledby="h-conditions">
        <h3 id="h-conditions" className="bk-h3">{t('Conditions')}</h3>
        <GroupView group={draft.root} top draft={draft} update={update} change={change} />
        <div className="ex-query-options">
          <div className="field">
            <label htmlFor="q-unit">{t('Return')}</label>
            <select id="q-unit" value={draft.unit} onChange={(e) => update({ ...draft, unit: e.target.value as Draft['unit'] })}>
              <option value="excerpts">{t('Distinct excerpts')}</option>
              <option value="assignments">{t('Assignments (each coding)')}</option>
            </select>
          </div>
          <Checkbox isSelected={draft.descendants} onChange={(v) => update({ ...draft, descendants: v })} className="check ex-explore-check">
            <Box />{t('A code also stands for its sub-codes')}</Checkbox>
          <Checkbox isSelected={draft.sameCoder} onChange={(v) => update({ ...draft, sameCoder: v })} className="check ex-explore-check">
            <Box />{t('“Also coded” looks at the same coder’s codings only')}</Checkbox>
        </div>
      </section>

      <section className="ex-readback" aria-labelledby="h-readback" data-testid="query-readback">
        <h3 id="h-readback" className="bk-h3">{t('The query in words')}</h3>
        <p>{words.summary}</p>
        {words.lines.length === 0 ? (
          <p className="muted">{t('(No conditions yet.)')}</p>
        ) : (
          <ul className="ex-readback-lines">
            {words.lines.map((l, i) => (
              <li key={i} style={{ marginInlineStart: `${l.depth * 1.2}rem` }}>
                {l.text}
              </li>
            ))}
          </ul>
        )}
        <p className="muted">{words.rules}</p>
        {problem ? <Notice kind="warning">{problem}</Notice> : null}
        {error ? <Notice>{error}</Notice> : null}
        <div className="actions">
          <Button className="primary" onPress={() => void run()} isDisabled={running || problem !== null} data-testid="run-query">
            {running ? t('Running…') : frozen ? t('Show the frozen answer') : t('Run Query')}
          </Button>
          {frozen ? (
            <Button onPress={() => void run(true)} isDisabled={running || problem !== null} data-testid="run-live">{t('Ask the question now instead')}</Button>
          ) : null}
          <Button onPress={() => setExploreQuery({ draft: { ...emptyDraft(), descendants: appStore.get().settings.subcodes }, saved: null })}>{t('Start again')}</Button>
          <Button isDisabled={codeIds.length === 0} onPress={() => { setExploreMatrix({ codeIds }); goExplore('matrix'); }}>{t('Open in Code Matrix')}</Button>
        </div>
      </section>

      <SavedQueries />

      <section className="ex-query-results" aria-labelledby="h-results" data-testid="query-results">
        <h3 id="h-results" className="bk-h3">{t('Results')}</h3>
        {ran === null ? (
          <p className="muted">{t('Nothing has been run yet. Build the query, read it back, and press Run Query.')}</p>
        ) : (
          <>
            {stale ? (
              <Notice kind="warning">
                <span data-testid="stale">{t('These results are from an earlier version of the query. Run it again to see the results of the one above.')}</span>
              </Notice>
            ) : null}
            <StaleSet since={ran.workingSet} what="results" />
            <p data-testid="query-count" role="status">
              {draft.unit === 'excerpts'
                ? t('{results} from {matching_codings}, looking through {codings} in {documents}.', { results: resultCount(ran.result.total, draft.unit), matching_codings: tn(ran.result.assignments, { one: '{count} matching coding', other: '{count} matching codings' }), codings: tn(ran.result.universe.assignments, { one: '{count} coding', other: '{count} codings' }), documents: tn(ran.result.universe.documents, { one: '{count} document', other: '{count} documents' }) })
                : t('{results}, looking through {codings} in {documents}.', { results: resultCount(ran.result.total, draft.unit), codings: tn(ran.result.universe.assignments, { one: '{count} coding', other: '{count} codings' }), documents: tn(ran.result.universe.documents, { one: '{count} document', other: '{count} documents' }) })}
              {ran.result.truncated ? ' ' + t('The list shows the first {count}.', { count: ran.result.items.length }) : ''}
            </p>
            {ran.result.total === 0 ? (
              <p className="help">{t('Nothing in the project matches. The query looked at every coding, so this is no match rather than a missing document.')}</p>
            ) : (
              <ol className="ex-query-list" aria-label={t('Query results')}>
                {ran.result.items.map((item) => (
                  <li key={`${item.source_id}-${item.start}-${item.end}-${item.codings[0].coding_id}`} className={stale ? 'stale' : undefined}>
                    <SegmentQuote text={item.text} sourceId={item.source_id} region={item.region} page={item.page} max={500} />
                    <p className="muted">
                      {item.source_name} · {item.kind === 'image' ? t('an area of the picture') : item.kind === 'clip' ? t('a stretch of the recording') : t('characters {start} to {end}', { start: num(item.start + 1), end: num(item.end) })}
                    </p>
                    <p className="ex-query-codes">
                      {item.codings.map((c) => (
                        <span key={c.coding_id} className="tag">
                          {c.code_name} ({c.coder_name})
                        </span>
                      ))}
                    </p>
                    <Button onPress={() => void openAt(item.source_id, item.start, item.end, item.codings[0].coding_id)} aria-label={item.kind === 'image' ? t('Open {source_name} at this area', { source_name: item.source_name }) : item.kind === 'clip' ? t('Open {source_name} at this stretch', { source_name: item.source_name }) : t('Open {source_name} at these words', { source_name: item.source_name })}>{t('Open at source')}</Button>
                  </li>
                ))}
              </ol>
            )}
          </>
        )}
      </section>
    </div>
  );
}

function Box() {
  return (
    <div className="box" aria-hidden="true">
      <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
        <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
      </svg>
    </div>
  );
}

function GroupView(props: {
  group: Group;
  top?: boolean;
  draft: Draft;
  update: (d: Draft) => void;
  change: (id: number, fn: (c: Clause | Group) => Clause | Group | null) => void;
}) {
  const { group, draft } = props;
  const all = clausesOf(draft.root);
  const addClause = () => props.update({ ...draft, root: addTo(draft.root, group.id, newClause('code')) });
  const addGroup = () => props.update({ ...draft, root: addTo(draft.root, group.id, newGroup('all')) });
  return (
    <div className={props.top ? 'ex-group top' : 'ex-group'} role="group" aria-label={props.top ? t('The conditions') : t('A group of conditions')}>
      <div className="field ex-match">
        <label htmlFor={`g-${group.id}`}>{props.top ? t('Match') : t('In this group, match')}</label>
        <select id={`g-${group.id}`} value={group.combine} onChange={(e) => props.change(group.id, (g) => ({ ...(g as Group), combine: e.target.value as 'any' | 'all' }))} data-testid={props.top ? 'match-top' : undefined}>
          <option value="any">{t('Any of the conditions (at least one)')}</option>
          <option value="all">{t('All of the conditions')}</option>
        </select>
        {props.top ? null : (
          <Button className="bk-link" onPress={() => props.change(group.id, () => null)}>{t('Remove this group')}</Button>
        )}
      </div>
      <ol className="ex-clauses">
        {group.items.map((item) => (
          <li key={item.id}>
            {item.kind === 'group' ? (
              <GroupView group={item} draft={draft} update={props.update} change={props.change} />
            ) : (
              <ClauseView clause={item} number={all.findIndex((c) => c.id === item.id) + 1} change={props.change} />
            )}
          </li>
        ))}
      </ol>
      <div className="actions">
        <Button onPress={addClause} data-testid={props.top ? 'add-condition' : undefined}>{t('Add Condition')}</Button>
        <Button onPress={addGroup}>{t('Add Group')}</Button>
      </div>
    </div>
  );
}

function ClauseView(props: { clause: Clause; number: number; change: (id: number, fn: (c: Clause | Group) => Clause | Group | null) => void }) {
  const { clause: c } = props;
  const codes = useApp((s) => s.codes);
  const sources = useApp((s) => s.sources);
  const cases = useApp((s) => s.cases);
  const coders = useApp((s) => s.coders);
  const vars = useCaseVariables();
  const codeList = useMemo(() => codeChoices(codes), [codes]);
  const set = (patch: Partial<Clause>) => props.change(c.id, (x) => ({ ...(x as Clause), ...patch }) as Clause);
  const label = t('Condition {number}', { number: props.number });
  const valuesOf = vars.find((v) => v.name === (c.type === 'variable' ? c.name : ''))?.values ?? [];
  return (
    <div className="ex-clause" role="group" aria-label={label} data-testid="clause">
      <div className="field">
        <label htmlFor={`c-type-${c.id}`}>{t('{label}: ingredient', { label })}</label>
        <select id={`c-type-${c.id}`} value={typeOf(c)} onChange={(e) => props.change(c.id, () => newClause(e.target.value as ClauseType))}>
          {CLAUSE_TYPES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <label htmlFor={`c-not-${c.id}`}>{t('{label}: operator', { label })}</label>
        <select id={`c-not-${c.id}`} value={c.negate ? 'not' : 'is'} onChange={(e) => set({ negate: e.target.value === 'not' })}>
          {operatorWords(c).map(([v, text]) => (
            <option key={v} value={v}>
              {text}
            </option>
          ))}
        </select>
      </div>
      {c.type === 'code' || c.type === 'also' ? (
        <div className="field">
          <span className="ex-field-label">{t('{label}: codes', { label })}</span>
          <CheckPicker noun="codes" label={t('{label}, codes', { label })} choices={codeList} value={c.codeIds} onChange={(ids) => set({ codeIds: ids })} none={t('Choose codes')} />
        </div>
      ) : null}
      {c.type === 'also' ? (
        <div className="field">
          <label htmlFor={`c-within-${c.id}`}>{t('{label}: how near', { label })}</label>
          <select id={`c-within-${c.id}`} value={String(c.within)} onChange={(e) => set({ within: Number(e.target.value) })}>
            <option value="0">{t('Overlapping the same words')}</option>
            <option value="25">{t('Within 25 characters')}</option>
            <option value="100">{t('Within 100 characters')}</option>
            <option value="500">{t('Within 500 characters')}</option>
          </select>
        </div>
      ) : null}
      {c.type === 'text' ? (
        <TextField value={c.contains} onChange={(v) => set({ contains: v })} className="field">
          <Label>{t('{label}: the words', { label })}</Label>
          <Input />
        </TextField>
      ) : null}
      {c.type === 'source' ? (
        <div className="field">
          <span className="ex-field-label">{t('{label}: documents', { label })}</span>
          <CheckPicker noun="documents" label={t('{label}, documents', { label })} choices={sources.map((s) => ({ id: s.id, label: s.name }))} value={c.ids} onChange={(ids) => set({ ids })} none={t('Choose documents')} />
        </div>
      ) : null}
      {c.type === 'case' ? (
        <div className="field">
          <span className="ex-field-label">{t('{label}: cases', { label })}</span>
          <CheckPicker noun="cases" label={t('{label}, cases', { label })} choices={cases.map((k) => ({ id: k.id, label: k.name }))} value={c.ids} onChange={(ids) => set({ ids })} none={t('Choose cases')} />
        </div>
      ) : null}
      {c.type === 'coder' ? (
        <div className="field">
          <span className="ex-field-label">{t('{label}: coders', { label })}</span>
          <CheckPicker noun="coders" label={t('{label}, coders', { label })} choices={coders.map((k) => ({ id: k.id, label: k.name }))} value={c.ids} onChange={(ids) => set({ ids })} none={t('Choose coders')} />
        </div>
      ) : null}
      {c.type === 'variable' ? (
        <>
          <div className="field">
            <label htmlFor={`c-var-${c.id}`}>{t('{label}: variable', { label })}</label>
            <select id={`c-var-${c.id}`} value={c.name} onChange={(e) => set({ name: e.target.value, value: '' })}>
              <option value="">{t('Choose a variable')}</option>
              {vars.map((v) => (
                <option key={v.name} value={v.name}>
                  {v.name}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor={`c-val-${c.id}`}>{t('{label}: value', { label })}</label>
            <select id={`c-val-${c.id}`} value={c.value} onChange={(e) => set({ value: e.target.value })} disabled={c.name === ''}>
              <option value="">{t('Choose a value')}</option>
              {valuesOf.map(([v, n]) => (
                <option key={v} value={v}>
                  {v} ({tn(n, { one: '{count} case', other: '{count} cases' })})
                </option>
              ))}
            </select>
          </div>
        </>
      ) : null}
      {c.type === 'rating' ? (
        <>
          <div className="field">
            <label htmlFor={`c-min-${c.id}`}>{t('{label}: lowest rating', { label })}</label>
            <select id={`c-min-${c.id}`} value={c.min} onChange={(e) => set({ min: e.target.value })}>
              <option value="">{t('No lowest')}</option>
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={String(n)}>
                  {n}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor={`c-max-${c.id}`}>{t('{label}: highest rating', { label })}</label>
            <select id={`c-max-${c.id}`} value={c.max} onChange={(e) => set({ max: e.target.value })}>
              <option value="">{t('No highest')}</option>
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={String(n)}>
                  {n}
                </option>
              ))}
            </select>
          </div>
        </>
      ) : null}
      {c.type === 'coded' ? (
        <>
          <TextField value={c.from} onChange={(v) => set({ from: v })} className="field">
            <Label>{t('{label}: first day', { label })}</Label>
            <Input placeholder="2024-03-01" />
          </TextField>
          <TextField value={c.to} onChange={(v) => set({ to: v })} className="field">
            <Label>{t('{label}: last day', { label })}</Label>
            <Input placeholder="2024-03-31" />
          </TextField>
          <p className="help ex-clause-help">{t('Write days year-month-day (2024-03-14); leave one empty for no limit. The day is the date each coding was made, in UTC.')}</p>
        </>
      ) : null}
      {c.type === 'between' ? (
        <>
          <div className="field">
            <label htmlFor={`c-bvar-${c.id}`}>{t('{label}: variable', { label })}</label>
            <select id={`c-bvar-${c.id}`} value={c.name} onChange={(e) => set({ name: e.target.value })}>
              <option value="">{t('Choose a variable')}</option>
              {vars.map((v) => (
                <option key={v.name} value={v.name}>
                  {v.name}
                </option>
              ))}
            </select>
          </div>
          <TextField value={c.from} onChange={(v) => set({ from: v })} className="field">
            <Label>{t('{label}: lowest value or first day', { label })}</Label>
            <Input />
          </TextField>
          <TextField value={c.to} onChange={(v) => set({ to: v })} className="field">
            <Label>{t('{label}: highest value or last day', { label })}</Label>
            <Input />
          </TextField>
          <p className="help ex-clause-help" data-testid="between-help">
            {readableNote(vars.find((v) => v.name === c.name))}
          </p>
        </>
      ) : null}
      <Button className="bk-link" onPress={() => props.change(c.id, () => null)} aria-label={t('Remove {label}', { label: label.toLowerCase() })}>{t('Remove')}</Button>
    </div>
  );
}

/** How many of a variable's values a range can use, so a person knows what a limit leaves out. */
function readableNote(v: CaseVariable | undefined): string {
  if (!v) return t('Choose a variable, then give both limits as dates written year-month-day or both as numbers. A value that is neither is never between any limits.');
  const total = v.values.reduce((n, [, k]) => n + k, 0);
  return t('{name}: {cases} of {total} with a value read as a date (year-month-day), {cases2} as a number. Give both limits as dates or both as numbers; a value that cannot be read that way is never between any limits.', { name: v.name, cases: tn(v.dates, { one: '{count} case', other: '{count} cases' }), total: num(total), cases2: tn(v.numbers, { one: '{count} case', other: '{count} cases' }) });
}

/** The two choices of the operator box, in words that fit the ingredient. */
function operatorWords(c: Clause): [string, string][] {
  switch (c.type) {
    case 'code':
    case 'source':
    case 'case':
    case 'coder':
      return [['is', t('is one of')], ['not', t('is not one of')]];
    case 'also':
      return [['is', t('is also coded')], ['not', t('is not also coded')]];
    case 'text':
      return [['is', t('contains')], ['not', t('does not contain')]];
    case 'variable':
      return [['is', t('has the value')], ['not', t('does not have the value')]];
    case 'rating':
      return [['is', t('is within')], ['not', t('is not within')]];
    case 'coded':
      return [['is', t('was made')], ['not', t('was not made')]];
    case 'between':
      return [['is', t('is between')], ['not', t('is not between')]];
  }
}
