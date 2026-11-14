import { useEffect, useMemo, useState } from 'react';
import { Button, Radio, RadioGroup } from 'react-aria-components';
import { call } from '../bridge';
import { cellShown } from '../lib/variables';
import { fmt } from '../lib/statistics';
import { num, t, tn } from '../i18n';
import { goExplore, openAt } from '../state/actions';
import { useApp } from '../state/app';
import type { CaseDisplay, CaseDisplaySpec, Typology, TypologyGroup, TypologySpec } from '../types';
import { Notice } from './common';
import { CheckPicker, EvidenceList, codeChoices } from './explore/shared';
import { useCaseVariables } from './explore/Matrix';
import { Bubbles, Deviations, SideBySides, Similarities } from './JointMore';
import { MOST_CATEGORIES, SCALE, useDisplay, useKept, variableChoices } from './jointShared';
import { Tick } from './Tick';

/** The displays, in the order they are offered. */
const DISPLAYS: { id: string; name: () => string }[] = [
  { id: 'ordered', name: () => t('Case-ordered display') },
  { id: 'typology', name: () => t('Typology table') },
  { id: 'side', name: () => t('Side-by-side display') },
  { id: 'deviation', name: () => t('Deviation table') },
  { id: 'similarity', name: () => t('Similarity analysis') },
  { id: 'bubble', name: () => t('Code by variable bubble plot') },
];

/**
 * Joint displays (spec 14.2) that are worked out from the project alone: each puts what people said beside what is known about them, in
 * one way or another. Every figure is shown with its n, and the words and cases behind it can be opened.
 */
export function JointDisplays(props: { which: string; onWhich: (id: string) => void }) {
  const { which, onWhich: setWhich } = props;
  return (
    <div className="jd-page" data-testid="joint-displays">
      <p className="help">
        {t('A joint display puts what people said beside what is known about them, on one page. These are made from the project as it is, and nothing in them is kept in the file: they are worked out again whenever you look.')}
      </p>
      <section className="ac-card mixed-card" aria-labelledby="h-jd-elsewhere" data-testid="jd-elsewhere">
        <h3 id="h-jd-elsewhere" className="bk-h3">{t('Displays that are in Explore')}</h3>
        <p className="muted">{t('The crosstab and the segment matrix are already built, in Explore. The cells of the code matrix open the passages behind them.')}</p>
        <div className="actions">
          <Button onPress={() => goExplore('crosstab')} data-testid="jd-to-crosstab">{t('Open Crosstabs')}</Button>
          <Button onPress={() => goExplore('matrix')} data-testid="jd-to-matrix">{t('Open the Code Matrix')}</Button>
        </div>
      </section>
      <div className="field jd-which">
        <label htmlFor="jd-which">{t('Which display')}</label>
        <select id="jd-which" value={which} onChange={(e) => setWhich(e.target.value)} data-testid="jd-which">
          {DISPLAYS.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name()}
            </option>
          ))}
        </select>
      </div>
      {which === 'typology' ? <TypologyTable /> : which === 'side' ? <SideBySides /> : which === 'deviation' ? <Deviations /> : which === 'similarity' ? <Similarities /> : which === 'bubble' ? <Bubbles /> : <CaseOrdered />}
    </div>
  );
}

// ---------------------------------------------------------------- the case-ordered display

function CaseOrdered() {
  const codes = useApp((s) => s.codes);
  const sub0 = useApp((s) => s.settings.subcodes);
  const vars = useCaseVariables();
  const { list: sortable } = useMemo(() => variableChoices(vars, (v) => v.level !== 'text'), [vars]);
  const { choices: all } = useMemo(() => variableChoices(vars, () => true), [vars]);
  const codeOptions = useMemo(() => codeChoices(codes), [codes]);
  const [sortBy, setSortBy] = useKept('ordered.sortBy', '');
  const [descending, setDescending] = useKept('ordered.descending', false);
  const [also, setAlso] = useKept<number[]>('ordered.also', []);
  const [codeIds, setCodeIds] = useKept<number[]>('ordered.codes', []);
  const [quotes, setQuotes] = useKept('ordered.quotes', 2);
  const [onlyCoded, setOnlyCoded] = useKept('ordered.onlyCoded', true);
  const [sub, setSub] = useKept('ordered.sub', sub0);
  const [evidence, setEvidence] = useState<{ title: string; ids: number[] } | null>(null);

  const spec: CaseDisplaySpec | null = useMemo(
    () => (sortBy === '' || codeIds.length === 0 ? null : { sort_by: sortBy, descending, variables: also.map((i) => all[i]?.label).filter((n): n is string => !!n && n !== sortBy), code_ids: codeIds, include_descendants: sub, quotes, only_coded: onlyCoded }),
    [sortBy, descending, also, all, codeIds, sub, quotes, onlyCoded],
  );
  const { result, error, loading } = useDisplay<CaseDisplay>(spec, () => call('joint.case_ordered', spec as CaseDisplaySpec));
  useEffect(() => setEvidence(null), [spec && JSON.stringify(spec)]); // eslint-disable-line react-hooks/exhaustive-deps

  const why = sortBy === '' ? t('Choose the variable to put the cases in order by.') : codeIds.length === 0 ? t('Choose the codes whose words are shown.') : null;
  return (
    <section className="ac-card mixed-card" aria-labelledby="h-jd-ordered" data-testid="jd-ordered">
      <h3 id="h-jd-ordered" className="bk-h3">{t('Case-ordered display')}</h3>
      <p className="muted">{t('The cases in the order of one variable, each with the words coded with the codes you choose. Read down the table to see whether what people said changes as the number does.')}</p>
      <div className="field">
        <label htmlFor="jd-sort">{t('Put the cases in order by')}</label>
        <select id="jd-sort" value={sortBy} onChange={(e) => setSortBy(e.target.value)} data-testid="jd-sort">
          <option value="">{t('Choose a variable')}</option>
          {sortable.map((v) => (
            <option key={v.name} value={v.name}>
              {v.name}
            </option>
          ))}
        </select>
      </div>
      <RadioGroup value={descending ? 'down' : 'up'} onChange={(v) => setDescending(v === 'down')} className="field ac-radios ac-inline" aria-label={t('Which way')} orientation="horizontal" data-testid="jd-way">
        <Radio value="up">{t('Lowest first, or first in the alphabet')}</Radio>
        <Radio value="down">{t('Highest first, or last in the alphabet')}</Radio>
      </RadioGroup>
      <div className="field">
        <span className="ex-field-label">{t('Codes whose words are shown')}</span>
        <CheckPicker noun={t('codes')} label={t('Codes whose words are shown')} choices={codeOptions} value={codeIds} onChange={setCodeIds} testId="jd-codes" />
      </div>
      <div className="field">
        <span className="ex-field-label">{t('Other variables to show')}</span>
        <CheckPicker noun={t('variables')} label={t('Other variables to show')} choices={all} value={also} onChange={setAlso} none={t('None besides the one it is ordered by')} testId="jd-also" />
      </div>
      <div className="field">
        <label htmlFor="jd-quotes">{t('Quotations in each cell')}</label>
        <select id="jd-quotes" value={quotes} onChange={(e) => setQuotes(Number(e.target.value))} data-testid="jd-quotes">
          {[0, 1, 2, 3, 4, 5].map((n) => (
            <option key={n} value={n}>
              {n === 0 ? t('None, only the counts') : num(n)}
            </option>
          ))}
        </select>
      </div>
      <Tick isSelected={onlyCoded} onChange={setOnlyCoded} testId="jd-only-coded">{t('Only the cases that have one of these codes')}</Tick>
      <Tick isSelected={sub} onChange={setSub} testId="jd-sub">{t('A code also stands for its sub-codes')}</Tick>
      {error ? <Notice>{error}</Notice> : null}
      {why ? <p className="muted" data-testid="jd-ordered-why">{why}</p> : null}
      {result ? <OrderedTable display={result} sortBy={sortBy} loading={loading} onEvidence={setEvidence} /> : null}
      {evidence ? <EvidenceList title={evidence.title} codingIds={evidence.ids} onClose={() => setEvidence(null)} testId="jd-evidence" /> : null}
    </section>
  );
}

function OrderedTable(props: { display: CaseDisplay; sortBy: string; loading: boolean; onEvidence: (e: { title: string; ids: number[] }) => void }) {
  const d = props.display;
  const order = d.by_number ? t('in the order of its numbers') : t('in the order of its words');
  return (
    <div data-testid="jd-ordered-result">
      <p role="status" data-testid="jd-ordered-summary">
        {d.rows.length === 0
          ? t('No case has a coding of these codes.')
          : `${t('{shown} of {total} cases, ordered by {name} {order}.', { shown: num(d.rows.length), total: num(d.cases), name: props.sortBy, order })}${d.without_value > 0 ? ` ${tn(d.without_value, { one: '{count} has no value of it and comes last.', other: '{count} have no value of it and come last.' })}` : ''}`}
      </p>
      {d.rows.length > 0 ? (
        <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The table of the cases in order')}>
          <table className="data jd-table">
            <caption className="visually-hidden">{t('The cases ordered by {name}, with the words coded with each code', { name: props.sortBy })}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Case')}</th>
                {d.variables.map((v) => (
                  <th key={v.name} scope="col">
                    {v.name}
                  </th>
                ))}
                {d.codes.map((c) => (
                  <th key={c.id} scope="col">
                    {c.name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {d.rows.map((r) => (
                <tr key={r.case_id} data-testid="jd-ordered-row">
                  <th scope="row">{r.case_name}</th>
                  {r.values.map((v, i) => (
                    <td key={i}>{v === null || v === '' ? <span className="muted">{'—'}</span> : cellShown(d.variables[i], v).text}</td>
                  ))}
                  {r.cells.map((cell, i) => (
                    <td key={i} className="jd-cell">
                      {cell.excerpts === 0 ? (
                        <span className="muted">{t('None')}</span>
                      ) : (
                        <>
                          <p className="jd-count">{tn(cell.excerpts, { one: '{count} excerpt', other: '{count} excerpts' })}</p>
                          <ul className="jd-quotes">
                            {cell.quotes.map((q) => (
                              <li key={q.coding_id}>
                                <q>{q.text ?? t('An area of a picture')}</q>{' '}
                                <Button className="bk-link" onPress={() => void openAt(q.source_id, q.start, q.end, q.coding_id)} aria-label={t('Open {source} at these words, from {case}', { source: q.source, case: r.case_name })}>
                                  {t('Open')}
                                </Button>
                              </li>
                            ))}
                          </ul>
                          {cell.more > 0 ? (
                            <Button className="bk-link" onPress={() => props.onEvidence({ title: `${r.case_name}: ${d.codes[i].name}`, ids: cell.coding_ids })} data-testid="jd-more">
                              {t('Show all, {count} more', { count: num(cell.more) })}
                            </Button>
                          ) : null}
                        </>
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------- the typology table

function groupLabel(g: TypologyGroup): string {
  if (g.key === 'all') return t('All cases');
  if (g.key === 'none') return t('Cases with text and none of these codes');
  return t('Cases that have {code}', { code: g.label });
}

function TypologyTable() {
  const codes = useApp((s) => s.codes);
  const sub0 = useApp((s) => s.settings.subcodes);
  const vars = useCaseVariables();
  const { list, choices } = useMemo(() => variableChoices(vars, (v) => v.level !== 'text' && (SCALE.includes(v.level) || v.values.length <= MOST_CATEGORIES)), [vars]);
  const codeOptions = useMemo(() => codeChoices(codes), [codes]);
  const [codeIds, setCodeIds] = useKept<number[]>('typology.codes', []);
  const [picked, setPicked] = useKept<number[]>('typology.variables', []);
  const [sub, setSub] = useKept('typology.sub', sub0);
  const [open, setOpen] = useState<string | null>(null);

  const spec: TypologySpec | null = useMemo(
    () => (codeIds.length === 0 || picked.length === 0 ? null : { code_ids: codeIds, include_descendants: sub, variables: picked.map((i) => list[i]?.name).filter((n): n is string => !!n) }),
    [codeIds, picked, list, sub],
  );
  const { result, error } = useDisplay<Typology>(spec, () => call('joint.typology', spec as TypologySpec));
  useEffect(() => setOpen(null), [spec && JSON.stringify(spec)]); // eslint-disable-line react-hooks/exhaustive-deps

  const why = codeIds.length === 0 ? t('Choose the codes that make the groups.') : picked.length === 0 ? t('Choose the variables to describe the groups with.') : null;
  return (
    <section className="ac-card mixed-card" aria-labelledby="h-jd-typology" data-testid="jd-typology">
      <h3 id="h-jd-typology" className="bk-h3">{t('Typology table')}</h3>
      <p className="muted">{t('Groups of cases formed by codes (the cases that have each code), described with variables of cases: for a number, how many cases have it, its mean and its spread; for categories, how many cases are in each. A case that has two of the codes is in both groups.')}</p>
      <div className="field">
        <span className="ex-field-label">{t('Codes that make the groups')}</span>
        <CheckPicker noun={t('codes')} label={t('Codes that make the groups')} choices={codeOptions} value={codeIds} onChange={setCodeIds} testId="jd-group-codes" />
      </div>
      <div className="field">
        <span className="ex-field-label">{t('Variables that describe the groups')}</span>
        <CheckPicker noun={t('variables')} label={t('Variables that describe the groups')} choices={choices} value={picked} onChange={setPicked} testId="jd-group-vars" />
      </div>
      <Tick isSelected={sub} onChange={setSub} testId="jd-group-sub">{t('A code also stands for its sub-codes')}</Tick>
      {error ? <Notice>{error}</Notice> : null}
      {why ? <p className="muted" data-testid="jd-typology-why">{why}</p> : null}
      {result ? <TypologyResult table={result} open={open} onOpen={setOpen} /> : null}
    </section>
  );
}

function TypologyResult(props: { table: Typology; open: string | null; onOpen: (key: string | null) => void }) {
  const tbl = props.table;
  const total = tbl.groups[0]?.cases.length ?? 0;
  const opened = tbl.groups.find((g) => g.key === props.open) ?? null;
  return (
    <div data-testid="jd-typology-result">
      <p className="muted" data-testid="jd-typology-note">
        {tn(tbl.cases_with_text, { one: '{count} of {total} cases holds text, and only a case that does can have a code, so the other cases are in the first row only.', other: '{count} of {total} cases hold text, and only a case that does can have a code, so the other cases are in the first row only.' }, { total: num(total) })}
      </p>
      <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The table of the groups')}>
        <table className="data apa jd-table">
          <caption className="visually-hidden">{t('Groups of cases formed by codes, described with variables of cases')}</caption>
          <thead>
            <tr>
              <th rowSpan={2} scope="col">{t('Group')}</th>
              {tbl.columns.map((c) => (
                <th key={c.name} colSpan={c.kind === 'scale' ? 3 : 1 + c.categories.length} scope="colgroup">
                  {c.name}
                </th>
              ))}
            </tr>
            <tr>
              {tbl.columns.flatMap((c) =>
                c.kind === 'scale'
                  ? [
                      <th key={`${c.name}-n`} scope="col"><i>{t('n')}</i></th>,
                      <th key={`${c.name}-m`} scope="col">{t('Mean')}</th>,
                      <th key={`${c.name}-sd`} scope="col">{t('SD')}</th>,
                    ]
                  : [<th key={`${c.name}-n`} scope="col"><i>{t('n')}</i></th>, ...c.categories.map((k) => <th key={`${c.name}-${k.value}`} scope="col">{k.label}</th>)],
              )}
            </tr>
          </thead>
          <tbody>
            {tbl.groups.map((g) => (
              <tr key={g.key} data-testid="jd-group-row">
                <th scope="row">
                  {groupLabel(g)}
                  <span className="muted jd-size"> {tn(g.cases.length, { one: '{count} case', other: '{count} cases' })}</span>{' '}
                  <Button className="bk-link" onPress={() => props.onOpen(props.open === g.key ? null : g.key)} aria-expanded={props.open === g.key} aria-label={t('Show the cases of {group}', { group: groupLabel(g) })} data-testid="jd-show-cases">
                    {props.open === g.key ? t('Hide the cases') : t('Show the cases')}
                  </Button>
                </th>
                {g.cells.flatMap((cell, i) => {
                  const column = tbl.columns[i];
                  if (cell.kind === 'scale') {
                    return [
                      <td key={`${i}-n`}>{num(cell.n)}</td>,
                      <td key={`${i}-m`}>{fmt(cell.mean)}</td>,
                      <td key={`${i}-sd`}>{fmt(cell.sd)}</td>,
                    ];
                  }
                  return [
                    <td key={`${i}-n`}>{num(cell.n)}</td>,
                    ...column.categories.map((_, k) => (
                      <td key={`${i}-${k}`}>{cell.n === 0 ? '—' : `${num(cell.counts[k])} (${num(Math.round((100 * cell.counts[k]) / cell.n))}%)`}</td>
                    )),
                  ];
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {opened ? (
        <section className="ex-evidence" aria-labelledby="h-jd-cases" data-testid="jd-cases">
          <div className="ex-evidence-head">
            <h4 id="h-jd-cases" className="bk-h3">{`${groupLabel(opened)}: ${tn(opened.cases.length, { one: '{count} case', other: '{count} cases' })}`}</h4>
            <Button className="bk-link" onPress={() => props.onOpen(null)}>{t('Close the list')}</Button>
          </div>
          {opened.cases.length === 0 ? <p className="muted">{t('No case is in this group.')}</p> : <ul className="jd-case-list">{opened.cases.map((c) => <li key={c.id}>{c.name}</li>)}</ul>}
        </section>
      ) : null}
    </div>
  );
}
