import { useMemo, useRef, useState, type CSSProperties } from 'react';
import { Button } from 'react-aria-components';
import { call } from '../bridge';
import { fmt } from '../lib/statistics';
import { shorten } from '../lib/charts';
import { num, t, tn } from '../i18n';
import { openAt } from '../state/actions';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type { AlikePair, Bubble, BubbleCell, Deviation, DeviationRow, SideBySide, Similarity, TypologyCell, TypologyColumn } from '../types';
import { Notice } from './common';
import { CheckPicker, EvidenceList, codeChoices } from './explore/shared';
import { useCaseVariables } from './explore/Matrix';
import { MOST_CATEGORIES, SCALE, useDisplay, useKept, variableChoices } from './jointShared';
import { StatExportDialog } from './StatExportDialog';
import { Tick } from './Tick';

/**
 * The joint displays that set a code beside the other cases, or the cases beside one another (spec 14.2): the side-by-side display,
 * the deviation table, similarity analysis, and the code by variable bubble plot. Like the others they are worked out from the project
 * each time they are looked at, show the n of every figure, and open the words and the cases behind it.
 */

const pct = (share: number) => `${num(Math.round(share * 100))}%`;

/** The variables a table of groups can hold: not free text, and for categories not too many. */
const describable = (v: { level: string; values: unknown[] }) => v.level !== 'text' && (SCALE.includes(v.level) || v.values.length <= MOST_CATEGORIES);

// ---------------------------------------------------------------- the side-by-side display

/** What the cases of a group are like on one variable, in words: for a number, n, the mean and the spread; for categories, a list. */
export function Described(props: { cell: TypologyCell; column: TypologyColumn }) {
  const { cell, column } = props;
  if (cell.kind === 'scale') {
    return cell.n === 0 ? <span className="muted">{'—'}</span> : <span>{t('n = {n}, mean {mean}, SD {sd}', { n: num(cell.n), mean: fmt(cell.mean), sd: fmt(cell.sd) })}</span>;
  }
  if (cell.n === 0) return <span className="muted">{'—'}</span>;
  return (
    <ul className="jd-cats">
      {column.categories.map((k, i) => (
        <li key={k.value}>{`${k.label}: ${num(cell.counts[i])} (${pct(cell.counts[i] / cell.n)})`}</li>
      ))}
      <li className="muted">{t('n = {n}', { n: num(cell.n) })}</li>
    </ul>
  );
}

export function SideBySides() {
  const codes = useApp((s) => s.codes);
  const sub0 = useApp((s) => s.settings.subcodes);
  const vars = useCaseVariables();
  const { list, choices } = useMemo(() => variableChoices(vars, describable), [vars]);
  const codeOptions = useMemo(() => codeChoices(codes), [codes]);
  const [codeIds, setCodeIds] = useKept<number[]>('side.codes', []);
  const [picked, setPicked] = useKept<number[]>('side.variables', []);
  const [quotes, setQuotes] = useKept('side.quotes', 2);
  const [sub, setSub] = useKept('side.sub', sub0);
  const [evidence, setEvidence] = useState<{ title: string; ids: number[] } | null>(null);

  const spec = useMemo(
    () => (codeIds.length === 0 || picked.length === 0 ? null : { code_ids: codeIds, include_descendants: sub, quotes, variables: picked.map((i) => list[i]?.name).filter((n): n is string => !!n) }),
    [codeIds, picked, list, sub, quotes],
  );
  const { result, error } = useDisplay<SideBySide>(spec, () => call('joint.side_by_side', spec as NonNullable<typeof spec>));
  const why = codeIds.length === 0 ? t('Choose the codes that are the topics.') : picked.length === 0 ? t('Choose the variables that stand for the numbers.') : null;
  return (
    <section className="ac-card mixed-card" aria-labelledby="h-jd-side" data-testid="jd-side">
      <h3 id="h-jd-side" className="bk-h3">{t('Side-by-side display')}</h3>
      <p className="muted">{t('For each topic, what people said in one column and what the numbers say in the other: the variables for the cases that have the topic, beside the same variables for the other cases that hold text.')}</p>
      <div className="field">
        <span className="ex-field-label">{t('Codes that are the topics')}</span>
        <CheckPicker noun={t('codes')} label={t('Codes that are the topics')} choices={codeOptions} value={codeIds} onChange={setCodeIds} testId="jd-side-codes" />
      </div>
      <div className="field">
        <span className="ex-field-label">{t('Variables that stand for the numbers')}</span>
        <CheckPicker noun={t('variables')} label={t('Variables that stand for the numbers')} choices={choices} value={picked} onChange={setPicked} testId="jd-side-vars" />
      </div>
      <div className="field">
        <label htmlFor="jd-side-quotes">{t('Quotations for each topic')}</label>
        <select id="jd-side-quotes" value={quotes} onChange={(e) => setQuotes(Number(e.target.value))} data-testid="jd-side-quotes">
          {[0, 1, 2, 3, 4, 5].map((n) => (
            <option key={n} value={n}>
              {n === 0 ? t('None, only the counts') : num(n)}
            </option>
          ))}
        </select>
      </div>
      <Tick isSelected={sub} onChange={setSub} testId="jd-side-sub">{t('A code also stands for its sub-codes')}</Tick>
      {error ? <Notice>{error}</Notice> : null}
      {why ? <p className="muted" data-testid="jd-side-why">{why}</p> : null}
      {result ? (
        <div data-testid="jd-side-result">
          <p className="muted">
            {tn(result.cases_with_text, { one: '{count} case holds text, and only a case that does can have a code.', other: '{count} cases hold text, and only a case that does can have a code.' })}
          </p>
          <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The table of topics, words beside numbers')}>
            <table className="data jd-table">
              <caption className="visually-hidden">{t('For each topic, what people said beside what the numbers say')}</caption>
              <thead>
                <tr>
                  <th rowSpan={2} scope="col">{t('Topic')}</th>
                  <th rowSpan={2} scope="col">{t('What people said')}</th>
                  <th colSpan={result.columns.length} scope="colgroup">{t('What the numbers say')}</th>
                </tr>
                <tr>
                  {result.columns.map((c) => (
                    <th key={c.name} scope="col">
                      {c.name}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {result.topics.map((topic) => (
                  <tr key={topic.code_id} data-testid="jd-side-row">
                    <th scope="row">
                      {topic.name}
                      <span className="muted jd-size"> {tn(topic.with.cases.length, { one: '{count} case', other: '{count} cases' })}</span>
                    </th>
                    <td className="jd-cell">
                      {topic.excerpts === 0 ? (
                        <span className="muted">{t('None')}</span>
                      ) : (
                        <>
                          <p className="jd-count">{tn(topic.excerpts, { one: '{count} excerpt', other: '{count} excerpts' })}</p>
                          <ul className="jd-quotes">
                            {topic.quotes.map((q) => (
                              <li key={q.coding_id}>
                                <q>{q.text ?? t('An area of a picture')}</q> <span className="muted">({q.case_name})</span>{' '}
                                <Button className="bk-link" onPress={() => void openAt(q.source_id, q.start, q.end, q.coding_id)} aria-label={t('Open {source} at these words, from {case}', { source: q.source, case: q.case_name })}>
                                  {t('Open')}
                                </Button>
                              </li>
                            ))}
                          </ul>
                          {topic.more > 0 ? (
                            <Button className="bk-link" onPress={() => setEvidence({ title: topic.name, ids: topic.coding_ids })} data-testid="jd-side-more">
                              {t('Show all, {count} more', { count: num(topic.more) })}
                            </Button>
                          ) : null}
                        </>
                      )}
                    </td>
                    {result.columns.map((column, i) => (
                      <td key={column.name} className="jd-cell">
                        <p className="jd-count">{t('Cases that have it')}</p>
                        <Described cell={topic.with.cells[i]} column={column} />
                        <p className="jd-count">{t('The other cases')}</p>
                        <Described cell={topic.without.cells[i]} column={column} />
                        {topic.differences[i] !== null ? (
                          <p className="jd-diff" data-testid="jd-side-diff">
                            {t('Difference of the means: {difference}', { difference: `${topic.differences[i]! > 0 ? '+' : ''}${fmt(topic.differences[i])}` })}
                          </p>
                        ) : null}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      ) : null}
      {evidence ? <EvidenceList title={evidence.title} codingIds={evidence.ids} onClose={() => setEvidence(null)} testId="jd-side-evidence" /> : null}
    </section>
  );
}

// ---------------------------------------------------------------- the deviation table

function signed(x: number, digits: number): string {
  return `${x > 0 ? '+' : x < 0 ? '−' : ''}${num(Math.abs(x), { minimumFractionDigits: digits, maximumFractionDigits: digits })}`;
}

function DeviationList(props: { title: string; groupKey: string; rows: DeviationRow[]; open: string | null; onOpen: (key: string | null) => void; none: string }) {
  const { rows } = props;
  const label = `${props.title}, ${props.groupKey}`;
  return (
    <div className="jd-dev-list">
      <h5 className="stats-h5">{props.title}</h5>
      {rows.length === 0 ? (
        <p className="muted">{props.none}</p>
      ) : (
        <div className="table-wrap" tabIndex={0} role="region" aria-label={label}>
          <table className="data jd-table">
            <caption className="visually-hidden">{label}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Code')}</th>
                <th scope="col">{t('In this group')}</th>
                <th scope="col">{t('In all the other cases')}</th>
                <th scope="col">{t('Difference')}</th>
                <th scope="col"><i>{t('z')}</i></th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const key = `${props.groupKey}|${r.code_id}`;
                return (
                  <tr key={r.code_id} data-testid="jd-dev-row">
                    <th scope="row">
                      {r.name}{' '}
                      <Button className="bk-link" onPress={() => props.onOpen(props.open === key ? null : key)} aria-expanded={props.open === key} aria-label={t('Show the cases of {group} that have {code}', { group: props.groupKey, code: r.name })} data-testid="jd-dev-cases">
                        {props.open === key ? t('Hide the cases') : t('Show the cases')}
                      </Button>
                      {props.open === key ? (
                        r.cases.length === 0 ? (
                          <p className="muted">{t('No case of this group has it.')}</p>
                        ) : (
                          <ul className="jd-case-list">{r.cases.map((c) => <li key={c.id}>{c.name}</li>)}</ul>
                        )
                      ) : null}
                    </th>
                    <td>{t('{in} of {total} ({share})', { in: num(r.in_cases), total: num(r.in_total), share: pct(r.in_share) })}</td>
                    <td>{t('{in} of {total} ({share})', { in: num(r.out_cases), total: num(r.out_total), share: pct(r.out_share) })}</td>
                    <td>{t('{points} points', { points: signed(r.difference * 100, 0) })}</td>
                    <td>{r.z === null ? '—' : signed(r.z, 2)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

export function Deviations() {
  const codes = useApp((s) => s.codes);
  const sub0 = useApp((s) => s.settings.subcodes);
  const vars = useCaseVariables();
  const { list } = useMemo(() => variableChoices(vars, (v) => v.level !== 'text' && v.values.length <= MOST_CATEGORIES), [vars]);
  const codeOptions = useMemo(() => codeChoices(codes), [codes]);
  const [variable, setVariable] = useKept('deviation.variable', '');
  const [codeIds, setCodeIds] = useKept<number[]>('deviation.codes', []);
  const [top, setTop] = useKept('deviation.top', 5);
  const [sub, setSub] = useKept('deviation.sub', sub0);
  const [open, setOpen] = useState<string | null>(null);

  const spec = useMemo(() => (variable === '' || codeIds.length === 0 ? null : { variable, code_ids: codeIds, include_descendants: sub, top }), [variable, codeIds, sub, top]);
  const { result, error } = useDisplay<Deviation>(spec, () => call('joint.deviation', spec as NonNullable<typeof spec>));
  const why = variable === '' ? t('Choose the variable whose values make the groups.') : codeIds.length === 0 ? t('Choose the codes to look at.') : null;
  return (
    <section className="ac-card mixed-card" aria-labelledby="h-jd-deviation" data-testid="jd-deviation">
      <h3 id="h-jd-deviation" className="bk-h3">{t('Deviation table')}</h3>
      <p className="muted">{t('For each group of cases, the codes it has more often than all the other cases (the most typical of it) and less often (the least typical).')}</p>
      <div className="field">
        <label htmlFor="jd-dev-variable">{t('Groups made by the values of')}</label>
        <select id="jd-dev-variable" value={variable} onChange={(e) => setVariable(e.target.value)} data-testid="jd-dev-variable">
          <option value="">{t('Choose a variable')}</option>
          {list.map((v) => (
            <option key={v.name} value={v.name}>
              {v.name}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <span className="ex-field-label">{t('Codes to look at')}</span>
        <CheckPicker noun={t('codes')} label={t('Codes to look at')} choices={codeOptions} value={codeIds} onChange={setCodeIds} testId="jd-dev-codes" />
      </div>
      <div className="field">
        <label htmlFor="jd-dev-top">{t('Codes listed for each group, most and least')}</label>
        <select id="jd-dev-top" value={top} onChange={(e) => setTop(Number(e.target.value))} data-testid="jd-dev-top">
          {[1, 3, 5, 10, 20].map((n) => (
            <option key={n} value={n}>
              {num(n)}
            </option>
          ))}
        </select>
      </div>
      <Tick isSelected={sub} onChange={setSub} testId="jd-dev-sub">{t('A code also stands for its sub-codes')}</Tick>
      {error ? <Notice>{error}</Notice> : null}
      {why ? <p className="muted" data-testid="jd-dev-why">{why}</p> : null}
      {result ? (
        <div data-testid="jd-dev-result">
          <p role="status" data-testid="jd-dev-summary">
            {tn(result.cases_used, { one: 'Every share is a share of the {count} case that holds text and has a value of {name}.', other: 'Every share is a share of the {count} cases that hold text and have a value of {name}.' }, { name: result.variable })}
            {result.without_value > 0 ? ` ${tn(result.without_value, { one: '{count} case has no value of it and is left out.', other: '{count} cases have no value of it and are left out.' })}` : ''}
          </p>
          <p className="muted">{t('z is the adjusted standardized residual of the group and the code: how far the group is from what the other cases would lead you to expect, in standard errors. With many codes and groups some will pass 2 by chance, so read it as a guide to where to look, not as a test.')}</p>
          {result.groups.map((g, gi) => (
            <section key={g.value} className="jd-dev-group" aria-labelledby={`h-jd-dev-${gi}`} data-testid="jd-dev-group">
              <h4 id={`h-jd-dev-${gi}`} className="bk-h3">
                {g.label} <span className="muted jd-size">{tn(g.cases, { one: '{count} case', other: '{count} cases' })}</span>
              </h4>
              <DeviationList title={t('Most typical')} groupKey={g.label} rows={g.most} open={open} onOpen={setOpen} none={t('No code is more common here than in the other cases.')} />
              <DeviationList title={t('Least typical')} groupKey={g.label} rows={g.least} open={open} onOpen={setOpen} none={t('No code is less common here than in the other cases.')} />
            </section>
          ))}
        </div>
      ) : null}
    </section>
  );
}

// ---------------------------------------------------------------- similarity analysis

function PairParts(props: { pair: AlikePair; sim: Similarity }) {
  const { pair, sim } = props;
  const show = (kind: string, v: string | null) => (v === null ? '—' : kind === 'code' ? (v === '1' ? t('Has it') : t('Does not have it')) : v);
  return (
    <div className="table-wrap" tabIndex={0} role="region" aria-label={t('How {a} and {b} compare', { a: pair.a.name, b: pair.b.name })}>
      <table className="data jd-table">
        <caption className="visually-hidden">{t('How {a} and {b} compare on each feature', { a: pair.a.name, b: pair.b.name })}</caption>
        <thead>
          <tr>
            <th scope="col">{t('Feature')}</th>
            <th scope="col">{pair.a.name}</th>
            <th scope="col">{pair.b.name}</th>
            <th scope="col">{t('Alike, 0 to 1')}</th>
          </tr>
        </thead>
        <tbody>
          {pair.parts.map((p) => (
            <tr key={p.feature}>
              <th scope="row">{sim.features[p.feature].name}</th>
              <td>{show(sim.features[p.feature].kind, p.a)}</td>
              <td>{show(sim.features[p.feature].kind, p.b)}</td>
              <td>{p.similarity === null ? <span className="muted">{t('Not compared')}</span> : fmt(p.similarity)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function Similarities() {
  const codes = useApp((s) => s.codes);
  const sub0 = useApp((s) => s.settings.subcodes);
  const vars = useCaseVariables();
  const { list, choices } = useMemo(() => variableChoices(vars, describable), [vars]);
  const codeOptions = useMemo(() => codeChoices(codes), [codes]);
  const [codeIds, setCodeIds] = useKept<number[]>('similarity.codes', []);
  const [picked, setPicked] = useKept<number[]>('similarity.variables', []);
  const [shared, setShared] = useKept('similarity.shared', false);
  const [pairs, setPairs] = useKept('similarity.pairs', 15);
  const [sub, setSub] = useKept('similarity.sub', sub0);
  const [open, setOpen] = useState<number | null>(null);

  const spec = useMemo(
    () => (codeIds.length === 0 && picked.length === 0 ? null : { code_ids: codeIds, include_descendants: sub, variables: picked.map((i) => list[i]?.name).filter((n): n is string => !!n), shared_absence: shared, pairs }),
    [codeIds, picked, list, sub, shared, pairs],
  );
  const { result, error } = useDisplay<Similarity>(spec, () => call('joint.similarity', spec as NonNullable<typeof spec>));
  return (
    <section className="ac-card mixed-card" aria-labelledby="h-jd-similarity" data-testid="jd-similarity">
      <h3 id="h-jd-similarity" className="bk-h3">{t('Similarity analysis')}</h3>
      <p className="muted">{t('Which cases are most alike on the codes and variables you choose. Each pair is given a number from 0 (nothing alike) to 1 (alike on everything they are compared on): the mean of how alike they are on each feature, as Gower\'s similarity gives it, which suits a mixture of codes, numbers, and categories.')}</p>
      <div className="field">
        <span className="ex-field-label">{t('Codes to compare on')}</span>
        <CheckPicker noun={t('codes')} label={t('Codes to compare on')} choices={codeOptions} value={codeIds} onChange={setCodeIds} none={t('No codes')} testId="jd-sim-codes" />
      </div>
      <div className="field">
        <span className="ex-field-label">{t('Variables to compare on')}</span>
        <CheckPicker noun={t('variables')} label={t('Variables to compare on')} choices={choices} value={picked} onChange={setPicked} none={t('No variables')} testId="jd-sim-vars" />
      </div>
      <div className="field">
        <label htmlFor="jd-sim-pairs">{t('Most alike pairs to list')}</label>
        <select id="jd-sim-pairs" value={pairs} onChange={(e) => setPairs(Number(e.target.value))} data-testid="jd-sim-pairs">
          {[5, 10, 15, 25, 50].map((n) => (
            <option key={n} value={n}>
              {num(n)}
            </option>
          ))}
        </select>
      </div>
      <Tick isSelected={shared} onChange={setShared} testId="jd-sim-shared">{t('Two cases that both lack a code are alike on it')}</Tick>
      <Tick isSelected={sub} onChange={setSub} testId="jd-sim-sub">{t('A code also stands for its sub-codes')}</Tick>
      {error ? <Notice>{error}</Notice> : null}
      {spec === null ? <p className="muted" data-testid="jd-sim-why">{t('Choose at least one code or one variable.')}</p> : null}
      {result ? (
        <div data-testid="jd-sim-result">
          <p role="status" data-testid="jd-sim-summary">
            {t('{cases} cases that hold text are compared on {features}.', { cases: num(result.cases.length), features: tn(result.features.length, { one: '{count} feature', other: '{count} features' }) })}{' '}
            {tn(result.pairs_compared, { one: '{count} pair has something to be compared on.', other: '{count} pairs have something to be compared on.' })}
          </p>
          {!shared && result.features.some((f) => f.kind === 'code') ? <p className="muted">{t('Two cases that both lack a code are not compared on it, so they do not become alike by having nothing.')}</p> : null}
          <h4 className="bk-h3">{t('The most alike pairs')}</h4>
          {result.pairs.length === 0 ? (
            <p className="muted">{t('No pair of cases has anything to be compared on.')}</p>
          ) : (
            <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The table of the most alike pairs')}>
              <table className="data jd-table">
                <caption className="visually-hidden">{t('The pairs of cases that are most alike, with how alike they are')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Pair')}</th>
                    <th scope="col">{t('Alike, 0 to 1')}</th>
                    <th scope="col">{t('Compared on')}</th>
                  </tr>
                </thead>
                <tbody>
                  {result.pairs.map((pair, i) => (
                    <tr key={`${pair.a.id}-${pair.b.id}`} data-testid="jd-sim-pair">
                      <th scope="row">
                        {t('{a} and {b}', { a: pair.a.name, b: pair.b.name })}{' '}
                        <Button className="bk-link" onPress={() => setOpen(open === i ? null : i)} aria-expanded={open === i} aria-label={t('Show how {a} and {b} compare', { a: pair.a.name, b: pair.b.name })} data-testid="jd-sim-how">
                          {open === i ? t('Hide how') : t('Show how')}
                        </Button>
                        {open === i ? <PairParts pair={pair} sim={result} /> : null}
                      </th>
                      <td>{fmt(pair.similarity)}</td>
                      <td>{tn(pair.compared, { one: '{count} feature', other: '{count} features' })}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {result.matrix ? (
            <details className="jd-matrix" data-testid="jd-sim-matrix">
              <summary>{t('Show the table of every pair')}</summary>
              <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The table of every pair of cases')}>
                <table className="data jd-table jd-sim-table">
                  <caption className="visually-hidden">{t('How alike every pair of cases is, from 0 to 1')}</caption>
                  <thead>
                    <tr>
                      <td />
                      {result.cases.map((c) => (
                        <th key={c.id} scope="col">
                          {shorten(c.name, 12)}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {result.cases.map((c, i) => (
                      <tr key={c.id}>
                        <th scope="row">{c.name}</th>
                        {result.matrix![i].map((v, j) => (
                          <td key={j} className="jd-sim-cell" style={v === null ? undefined : ({ '--sim': String(i === j ? 0 : v) } as CSSProperties)}>
                            {v === null ? <span className="muted">{'—'}</span> : i === j ? <span className="muted">{'·'}</span> : fmt(v)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </details>
          ) : (
            <p className="muted">{t('There are more than 60 cases, so only the most alike pairs are listed; choose fewer features to find the rest.')}</p>
          )}
        </div>
      ) : null}
    </section>
  );
}

// ---------------------------------------------------------------- the code by variable bubble plot

const CANVAS = 640;

function BubbleChart(props: { bubble: Bubble; onEvidence: (e: { title: string; ids: number[] }) => void }) {
  const b = props.bubble;
  const [readout, setReadout] = useState('');
  const [saving, setSaving] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  const textSize = useApp((s) => s.settings.textSize);
  const k = useMemo(() => Math.max(0.75, (parseFloat(getComputedStyle(document.documentElement).fontSize) || 14) / 14), [textSize]); // eslint-disable-line react-hooks/exhaustive-deps
  const title = t('{code}: {x} by {y}', { code: b.code, x: b.x.name, y: b.y.name });
  const cols = b.x.categories.length;
  const rows = b.y.categories.length;
  const m = { l: 120, r: 24, t: 14, b: 96 };
  const cw = (CANVAS - m.l - m.r) / Math.max(1, cols);
  const rh = Math.max(44, Math.min(76, cw));
  const H = m.t + m.b + rows * rh;
  const most = Math.max(1, ...b.cells.map((c) => c.excerpts));
  const rmax = Math.min(cw, rh) / 2 - 5;
  const radius = (c: BubbleCell) => (c.excerpts === 0 ? 4 : Math.max(5, rmax * Math.sqrt(c.excerpts / most)));
  const step = (c: BubbleCell) => (c.mean_rating === null ? -1 : Math.max(0, Math.min(4, Math.round(c.mean_rating) - 1)));
  const words = (c: BubbleCell) =>
    t('{x}, {y}: {excerpts}, from {withCode} of {cases}{mean}', {
      x: b.x.categories[c.x].label,
      y: b.y.categories[c.y].label,
      excerpts: tn(c.excerpts, { one: '{count} excerpt', other: '{count} excerpts' }),
      withCode: num(c.with_code),
      cases: tn(c.cases, { one: '{count} case', other: '{count} cases' }),
      mean: c.mean_rating === null ? '' : `, ${t('mean weight {mean}', { mean: fmt(c.mean_rating) })}`,
    });
  const px = (x: number) => m.l + cw * (x + 0.5);
  const py = (y: number) => m.t + rh * (rows - 1 - y + 0.5);
  const legendY = H - 52;
  return (
    <figure className="stats-chart jd-bubble" aria-labelledby="jd-bubble-t" aria-describedby="jd-bubble-d" data-testid="jd-bubble-chart">
      <figcaption>
        <h4 id="jd-bubble-t" className="stats-h5">{title}</h4>
        <p id="jd-bubble-d" className="muted">
          {t('A bubble for each combination of {x} and {y}. Its size is how many excerpts of {code} the cases in it have{rated}. The numbers are in the table below the chart.', {
            x: b.x.name,
            y: b.y.name,
            code: b.code,
            rated: b.rated > 0 ? t(', and its colour is the mean weight of those codings, from 1 to 5') : '',
          })}
        </p>
      </figcaption>
      <div ref={body} className="stats-chart-body" tabIndex={0} role="region" aria-label={t('{title}, scrolls sideways', { title })}>
        <svg viewBox={`0 0 ${CANVAS} ${H}`} style={{ width: CANVAS * k }} className="stats-svg" role="group" aria-label={title} focusable="false">
          {b.x.categories.map((_, i) => (
            <line key={`gx${i}`} x1={m.l + cw * i} x2={m.l + cw * i} y1={m.t} y2={H - m.b} className="chart-grid" aria-hidden="true" />
          ))}
          {b.y.categories.map((_, j) => (
            <line key={`gy${j}`} x1={m.l} x2={CANVAS - m.r} y1={m.t + rh * j} y2={m.t + rh * j} className="chart-grid" aria-hidden="true" />
          ))}
          <rect x={m.l} y={m.t} width={cw * cols} height={rh * rows} className="jd-bubble-frame" aria-hidden="true" />
          {b.x.categories.map((c, i) => (
            <text key={`lx${i}`} x={px(i)} y={H - m.b + 18} textAnchor="middle" className="chart-text" aria-hidden="true">{shorten(c.label, Math.max(6, Math.floor(cw / 7)))}</text>
          ))}
          {b.y.categories.map((c, j) => (
            <text key={`ly${j}`} x={m.l - 8} y={py(j) + 4} textAnchor="end" className="chart-text" aria-hidden="true">{shorten(c.label, 16)}</text>
          ))}
          <text x={(m.l + CANVAS - m.r) / 2} y={H - m.b + 40} textAnchor="middle" className="chart-text" aria-hidden="true">{b.x.name}</text>
          <text transform={`translate(14 ${(m.t + H - m.b) / 2}) rotate(-90)`} textAnchor="middle" className="chart-text" aria-hidden="true">{b.y.name}</text>
          {b.cells.map((c) => {
            const w = words(c);
            const s = step(c);
            return (
              <circle
                key={`${c.x}-${c.y}`}
                cx={px(c.x)}
                cy={py(c.y)}
                r={radius(c)}
                className={`jd-bubble-mark ${c.excerpts === 0 ? 'jd-bubble-none' : s < 0 ? 'jd-seq-plain' : `jd-seq-${s}`}`}
                tabIndex={0}
                role="img"
                aria-label={w}
                onMouseEnter={() => setReadout(w)}
                onMouseLeave={() => setReadout('')}
                onFocus={() => {
                  setReadout(w);
                  announce(w);
                }}
                onBlur={() => setReadout('')}
              />
            );
          })}
          <text x={m.l} y={legendY} className="chart-muted" aria-hidden="true">{t('Size: the number of excerpts; the largest is {n}.', { n: num(most) })}</text>
          {b.rated > 0 ? (
            <g aria-hidden="true">
              <text x={m.l} y={legendY + 22} className="chart-muted">{t('Mean weight:')}</text>
              {[0, 1, 2, 3, 4].map((i) => (
                <g key={i}>
                  <rect x={m.l + 92 + i * 44} y={legendY + 10} width={36} height={16} className={`jd-seq-${i}`} />
                  <text x={m.l + 92 + i * 44 + 18} y={legendY + 40} textAnchor="middle" className="chart-muted">{num(i + 1)}</text>
                </g>
              ))}
            </g>
          ) : (
            <text x={m.l} y={legendY + 22} className="chart-muted" aria-hidden="true">{t('No coding of this code has a weight, so the bubbles have no colour.')}</text>
          )}
        </svg>
      </div>
      <p className="chart-readout" data-testid="chart-readout">{readout}</p>
      <div className="actions stats-chart-actions">
        <Button onPress={() => setSaving(true)} aria-label={t('Save the chart "{title}" as a picture', { title })} data-testid="jd-bubble-save">
          {t('Save as picture…')}
        </Button>
      </div>
      {saving ? <StatExportDialog target={{ kind: 'chart', svg: () => body.current?.querySelector<SVGSVGElement>('svg.stats-svg') ?? null, name: title }} onClose={() => setSaving(false)} /> : null}
      <h5 className="stats-h5">{t('The numbers behind the chart')}</h5>
      <div className="table-wrap" tabIndex={0} role="region" aria-label={t('{title}, the numbers', { title })}>
        <table className="data jd-table" data-testid="jd-bubble-table">
          <caption className="visually-hidden">{t('{code} across {y} and {x}: excerpts, cases, and mean weight', { code: b.code, x: b.x.name, y: b.y.name })}</caption>
          <thead>
            <tr>
              <th scope="col">{`${b.y.name} / ${b.x.name}`}</th>
              {b.x.categories.map((c) => (
                <th key={c.value} scope="col">
                  {c.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {b.y.categories.map((yc, j) => (
              <tr key={yc.value}>
                <th scope="row">{yc.label}</th>
                {b.x.categories.map((xc, i) => {
                  const c = b.cells.find((cell) => cell.x === i && cell.y === j);
                  return (
                    <td key={xc.value} className="jd-cell" data-testid="jd-bubble-cell">
                      {!c ? (
                        <span className="muted">{'—'}</span>
                      ) : (
                        <>
                          <p className="jd-count">{tn(c.excerpts, { one: '{count} excerpt', other: '{count} excerpts' })}</p>
                          <p className="muted">{tn(c.cases, { one: '{withCode} of {count} case', other: '{withCode} of {count} cases' }, { withCode: num(c.with_code) })}</p>
                          {c.mean_rating !== null ? <p className="muted">{t('mean weight {mean}, {n} rated', { mean: fmt(c.mean_rating), n: num(c.rated) })}</p> : null}
                          {c.excerpts > 0 ? (
                            <Button className="bk-link" onPress={() => props.onEvidence({ title: `${yc.label}, ${xc.label}`, ids: c.coding_ids })} aria-label={t('Show the excerpts for {y}, {x}', { y: yc.label, x: xc.label })} data-testid="jd-bubble-show">
                              {t('Show the excerpts')}
                            </Button>
                          ) : null}
                        </>
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </figure>
  );
}

export function Bubbles() {
  const codes = useApp((s) => s.codes);
  const sub0 = useApp((s) => s.settings.subcodes);
  const vars = useCaseVariables();
  const { list } = useMemo(() => variableChoices(vars, (v) => v.level !== 'text' && v.values.length <= MOST_CATEGORIES), [vars]);
  const options = useMemo(() => codeChoices(codes), [codes]);
  const [codeId, setCodeId] = useKept<number | null>('bubble.code', null);
  const [x, setX] = useKept('bubble.x', '');
  const [y, setY] = useKept('bubble.y', '');
  const [sub, setSub] = useKept('bubble.sub', sub0);
  const [evidence, setEvidence] = useState<{ title: string; ids: number[] } | null>(null);

  const spec = useMemo(() => (codeId === null || x === '' || y === '' ? null : { code_id: codeId, include_descendants: sub, x, y }), [codeId, x, y, sub]);
  const { result, error } = useDisplay<Bubble>(spec, () => call('joint.bubble', spec as NonNullable<typeof spec>));
  const why = codeId === null ? t('Choose the code.') : x === '' || y === '' ? t('Choose the two variables.') : null;
  const picker = (id: string, value: string, set: (v: string) => void, label: string, other: string) => (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <select id={id} value={value} onChange={(e) => set(e.target.value)} data-testid={id}>
        <option value="">{t('Choose a variable')}</option>
        {list
          .filter((v) => v.name !== other)
          .map((v) => (
            <option key={v.name} value={v.name}>
              {v.name}
            </option>
          ))}
      </select>
    </div>
  );
  return (
    <section className="ac-card mixed-card" aria-labelledby="h-jd-bubble" data-testid="jd-bubble">
      <h3 id="h-jd-bubble" className="bk-h3">{t('Code by variable bubble plot')}</h3>
      <p className="muted">{t('One code across two variables at once: a bubble for each combination of their values, its size how many excerpts of the code the cases in it have and its colour the mean weight of those codings. The table beneath it is the same numbers, one code across two crossed variables.')}</p>
      <div className="field">
        <label htmlFor="jd-bubble-code">{t('The code')}</label>
        <select id="jd-bubble-code" value={codeId ?? ''} onChange={(e) => setCodeId(e.target.value === '' ? null : Number(e.target.value))} data-testid="jd-bubble-code">
          <option value="">{t('Choose a code')}</option>
          {options.map((c) => (
            <option key={c.id} value={c.id}>
              {`${' '.repeat(c.depth ?? 0)}${c.label}`}
            </option>
          ))}
        </select>
      </div>
      {picker('jd-bubble-x', x, setX, t('Along the bottom'), y)}
      {picker('jd-bubble-y', y, setY, t('Up the side'), x)}
      <Tick isSelected={sub} onChange={setSub} testId="jd-bubble-sub">{t('A code also stands for its sub-codes')}</Tick>
      {error ? <Notice>{error}</Notice> : null}
      {why ? <p className="muted" data-testid="jd-bubble-why">{why}</p> : null}
      {result ? (
        <div data-testid="jd-bubble-result">
          <p role="status" data-testid="jd-bubble-summary">
            {tn(result.cases_used, { one: '{count} case that holds text and has a value of both variables is in the plot.', other: '{count} cases that hold text and have a value of both variables are in the plot.' })}
            {result.left_out > 0 ? ` ${tn(result.left_out, { one: '{count} case has no value of one of them and is left out.', other: '{count} cases have no value of one of them and are left out.' })}` : ''}
          </p>
          <BubbleChart bubble={result} onEvidence={setEvidence} />
        </div>
      ) : null}
      {evidence ? <EvidenceList title={evidence.title} codingIds={evidence.ids} onClose={() => setEvidence(null)} testId="jd-bubble-evidence" /> : null}
    </section>
  );
}
