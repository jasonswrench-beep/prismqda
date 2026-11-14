// The code matrix: codes against cases, documents, or the values of a case variable, with the numbers behind every cell.

import { useEffect, useMemo, useState } from 'react';
import { Button, Checkbox } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { METRICS, metricLabel, metricNoun, pick, intensity, percentModes, percentSentence, percentText, percentWhole, type PercentMode } from '../../lib/explore';
import { setExploreMatrix, goExplore } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp, type MatrixSetup } from '../../state/app';
import type { CaseVariable, MatrixResult, MatrixSpec } from '../../types';
import { Notice, plural } from '../common';
import { CheckPicker, EvidenceList, codeChoices } from './shared';
import { compareText, num, rich, t, tn } from '../../i18n';
import { engineText } from '../../i18n/engine';

/** What the engine is asked, from the setup; null while something needed is not chosen yet. */
export function matrixSpecOf(setup: MatrixSetup): MatrixSpec | null {
  if (setup.codeIds.length === 0) return null;
  if (setup.rowKind === 'variable' && setup.variable === '') return null;
  return {
    rows: setup.rowKind === 'variable' ? { kind: 'case_variable', name: setup.variable } : setup.rowKind === 'months' ? { kind: 'month_coded' } : { kind: setup.rowKind },
    code_ids: setup.codeIds,
    include_descendants: setup.descendants,
    ...(setup.coderIds.length > 0 ? { scope: { coder_ids: setup.coderIds } } : {}),
  };
}

/** The case variables of the project, read again when something changes. */
export function useCaseVariables(): CaseVariable[] {
  const version = useApp((s) => s.dataVersion);
  const cases = useApp((s) => s.cases);
  const [vars, setVars] = useState<CaseVariable[]>([]);
  useEffect(() => {
    let live = true;
    call('case.variables')
      .then((v) => live && setVars(v))
      .catch(() => live && setVars([]));
    return () => {
      live = false;
    };
  }, [version, cases.length]);
  return vars;
}

/** Runs the matrix whenever the setup (or the coding) changes, and keeps only the answer to the latest question. */
export function useMatrix(): { setup: MatrixSetup; spec: MatrixSpec | null; result: MatrixResult | null; error: string | null; loading: boolean } {
  const setup = useApp((s) => s.exploreMatrix);
  const version = useApp((s) => s.dataVersion);
  const codings = useApp((s) => s.sources.reduce((n, x) => n + x.codings, 0));
  const spec = useMemo(() => matrixSpecOf(setup), [setup]);
  const key = JSON.stringify(spec);
  const [state, setState] = useState<{ result: MatrixResult | null; error: string | null; loading: boolean }>({ result: null, error: null, loading: false });
  useEffect(() => {
    if (!spec) {
      setState({ result: null, error: null, loading: false });
      return;
    }
    let live = true;
    setState((s) => ({ ...s, loading: true, error: null }));
    call('explore.matrix', spec)
      .then((result) => live && setState({ result, error: null, loading: false }))
      .catch((e) => live && setState({ result: null, error: toApiError(e).message, loading: false }));
    return () => {
      live = false;
    };
    // `key` is the spec; the matrix is also read again when the coding changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version, codings]);
  return { setup, spec, ...state };
}

/** A percentage choice is kept when it still makes sense for the new metric and rows, and put back to numbers only when it does not. */
function percentStillValid(mode: PercentMode, metric: MatrixSetup['metric'], rows: MatrixSetup['rowKind']): PercentMode {
  return percentModes(metric, rows).some((m) => m.id === mode) ? mode : 'none';
}

/** The first time the matrix is opened in a project it starts with the codes used most, so there is something to read. */
const started = new Set<string>();

export function MatrixControls(props: { showMetric?: boolean; showPercent?: boolean }) {
  const setup = useApp((s) => s.exploreMatrix);
  const codes = useApp((s) => s.codes);
  const coders = useApp((s) => s.coders);
  const uid = useApp((s) => s.project?.project_uid ?? '');
  const vars = useCaseVariables();
  const choices = useMemo(() => codeChoices(codes), [codes]);

  useEffect(() => {
    if (uid === '' || started.has(uid) || codes.length === 0) return;
    started.add(uid);
    if (appStore.get().exploreMatrix.codeIds.length > 0) return;
    const used = codes
      .filter((c) => c.status === 'active' && c.usage > 0 && !codes.some((k) => k.parent_id === c.id && k.status === 'active'))
      .sort((a, b) => b.usage - a.usage || compareText(a.name, b.name))
      .slice(0, 6)
      .map((c) => c.id);
    if (used.length > 0) setExploreMatrix({ codeIds: used });
  }, [uid, codes]);

  const rowsValue = setup.rowKind === 'variable' ? `var:${setup.variable}` : setup.rowKind;
  return (
    <section className="ex-explore-controls" aria-label={t('Matrix setup')} data-testid="matrix-controls">
      <div className="field">
        <label htmlFor="mx-rows">{t('Rows')}</label>
        <select
          id="mx-rows"
          value={rowsValue}
          onChange={(e) => {
            const v = e.target.value;
            if (v.startsWith('var:')) setExploreMatrix({ rowKind: 'variable', variable: v.slice(4) });
            else setExploreMatrix({ rowKind: v as 'cases' | 'documents' | 'months', variable: '', percent: percentStillValid(setup.percent, setup.metric, v as 'cases' | 'documents' | 'months') });
          }}
        >
          <option value="cases">{t('Cases')}</option>
          <option value="documents">{t('Documents')}</option>
          <option value="months">{t('Month the coding was made')}</option>
          {vars.map((v) => (
            <option key={v.name} value={`var:${v.name}`}>{t('Case variable: {name}', { name: v.name })}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <span className="ex-field-label" id="mx-codes-label">{t('Columns')}</span>
        <CheckPicker noun="codes" label={t('Codes for the columns')} choices={choices} value={setup.codeIds} onChange={(ids) => setExploreMatrix({ codeIds: ids })} none={t('Choose codes')} testId="matrix-codes" />
      </div>
      {props.showMetric === false ? null : (
        <div className="field">
          <label htmlFor="mx-metric">{t('Metric')}</label>
          <select
            id="mx-metric"
            value={setup.metric}
            onChange={(e) => setExploreMatrix({ metric: e.target.value as MatrixSetup['metric'], percent: percentStillValid(setup.percent, e.target.value as MatrixSetup['metric'], setup.rowKind) })}
          >
            {METRICS.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </div>
      )}
      {props.showMetric === false || props.showPercent === false ? null : (
        <div className="field">
          <label htmlFor="mx-percent">{t('Show as')}</label>
          <select id="mx-percent" value={setup.percent} onChange={(e) => setExploreMatrix({ percent: e.target.value as PercentMode })} data-testid="matrix-percent">
            {percentModes(setup.metric, setup.rowKind).map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </div>
      )}
      <div className="field">
        <span className="ex-field-label">{t('Coders')}</span>
        <CheckPicker
          noun="coders"
          label={t('Coders to look at')}
          choices={coders.map((c) => ({ id: c.id, label: c.name }))}
          value={setup.coderIds}
          onChange={(ids) => setExploreMatrix({ coderIds: ids.length === coders.length ? [] : ids })}
          none={t('All coders')}
        />
      </div>
      <Checkbox isSelected={setup.descendants} onChange={(v) => setExploreMatrix({ descendants: v })} className="check ex-explore-check">
        <div className="box" aria-hidden="true">
          <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
            <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
          </svg>
        </div>{t('A code also counts its sub-codes')}</Checkbox>
    </section>
  );
}

interface Selected {
  title: string;
  ids: number[];
}

export function CodeMatrix() {
  const { setup, spec, result, error, loading } = useMatrix();
  const codes = useApp((s) => s.codes);
  const [selected, setSelected] = useState<Selected | null>(null);
  const metric = setup.metric;
  const rowsName = setup.rowKind === 'cases' ? t('Case') : setup.rowKind === 'documents' ? t('Document') : setup.rowKind === 'months' ? t('Month coded') : setup.variable;
  const percent = setup.percent;

  // What was chosen belongs to the matrix it came from.
  useEffect(() => setSelected(null), [JSON.stringify(spec)]);

  const open = async (title: string, rowKey: string | null, codeId: number | null) => {
    if (!spec) return;
    try {
      const r = await call('explore.matrix_evidence', { spec, row_key: rowKey, code_id: codeId });
      setSelected({ title, ids: r.coding_ids });
      announce(t('{title}: {codings} shown below.', { title, codings: tn(r.coding_ids.length, { one: '{count} coding', other: '{count} codings' }) }));
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };

  const max = result ? Math.max(0, ...result.rows.flatMap((r) => r.cells.map((c) => pick(c, metric)))) : 0;
  const showN = result?.rows.some((r) => r.size !== null) ?? false;

  return (
    <div data-testid="matrix-tool">
      <MatrixControls />
      {error ? <Notice>{error}</Notice> : null}
      {!spec ? (
        <p className="help">
          {setup.codeIds.length === 0 ? t('Choose the codes for the columns to compare them.') : t('Choose a case variable for the rows.')} {t('A cell counts the codings of one code for one row, and opens the passages behind it.')}</p>
      ) : result ? (
        <>
          <p className="muted ex-matrix-universe" data-testid="matrix-universe">
            {loading ? t('Updating…') + ' ' : ''}
            {setup.coderIds.length > 0
              ? rich('Looked at {codings} in {documents} by the coders chosen. The metric is <strong>{metric}</strong>.', { codings: tn(result.universe.assignments, { one: '{count} coding', other: '{count} codings' }), documents: tn(result.universe.documents, { one: '{count} document', other: '{count} documents' }), metric: metricLabel(metric), strong: (inside) => <strong>{inside}</strong> })
              : rich('Looked at {codings} in {documents}. The metric is <strong>{metric}</strong>.', { codings: tn(result.universe.assignments, { one: '{count} coding', other: '{count} codings' }), documents: tn(result.universe.documents, { one: '{count} document', other: '{count} documents' }), metric: metricLabel(metric), strong: (inside) => <strong>{inside}</strong> })}</p>
          <div className="ex-table-wrap">
            <table className="ex-matrix" data-testid="matrix-table">
              <caption>
                {t('{metric} of each code, by {rowsName}', { metric: metricLabel(metric), rowsName: rowsName.toLowerCase() })}
              </caption>
              <thead>
                <tr>
                  <th scope="col">{rowsName}</th>
                  {showN ? <th scope="col">{t('N (cases)')}</th> : null}
                  {result.columns.map((c) => (
                    <th scope="col" key={c.code_id}>
                      {c.name}
                    </th>
                  ))}
                  <th scope="col">{t('Total (distinct)')}</th>
                </tr>
              </thead>
              <tbody>
                {result.rows.map((row) => (
                  <tr key={row.key}>
                    <th scope="row">{row.label}</th>
                    {showN ? <td className="ex-num">{row.size ?? ''}</td> : null}
                    {row.cells.map((cell, i) => {
                      const v = pick(cell, metric);
                      const col = result.columns[i];
                      const whole = percent === 'none' ? null : percentWhole(percent, metric, row, i, result);
                      const shown = percent === 'none' ? num(v) : `${num(v)} (${percentText(v, whole)})`;
                      const spoken = percent === 'none' ? metricNoun(metric, v) : whole === null ? t('{noun}, no percentage because there is nothing to take it of', { noun: metricNoun(metric, v) }) : t('{noun}, {percent} of {whole}', { noun: metricNoun(metric, v), percent: percentText(v, whole), whole: num(whole) });
                      return (
                        <td key={col.code_id} className="ex-num ex-cell" style={v > 0 ? { ['--shade' as string]: String(intensity(v, max)) } : undefined}>
                          {v > 0 ? (
                            <Button
                              className="ex-cell-button"
                              aria-label={t('{label}, {name}: {spoken}. Show what is behind this number.', { label: row.label, name: col.name, spoken })}
                              onPress={() => void open(`${row.label} · ${col.name}`, row.key, col.code_id)}
                            >
                              {shown}
                            </Button>
                          ) : (
                            <span aria-label={percent !== 'none' && whole === null ? t('{label}, {name}: none, no percentage', { label: row.label, name: col.name }) : t('{label}, {name}: none', { label: row.label, name: col.name })}>{percent === 'none' ? '0' : `0 (${percentText(0, whole)})`}</span>
                          )}
                        </td>
                      );
                    })}
                    <td className="ex-num ex-total">
                      {pick(row.total, metric) > 0 ? (
                        <Button
                          className="ex-cell-button"
                          aria-label={t('{label}, all the columns together: {noun}. Show what is behind this number.', { label: row.label, noun: metricNoun(metric, pick(row.total, metric)) })}
                          onPress={() => void open(`${row.label} · all columns`, row.key, null)}
                        >
                          {num(pick(row.total, metric))}
                        </Button>
                      ) : (
                        <span>0</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr>
                  <th scope="row">{t('All {rowsName}s (distinct)', { rowsName: rowsName.toLowerCase() })}</th>
                  {showN ? <td className="ex-num">{result.rows.reduce((n, r) => n + (r.size ?? 0), 0)}</td> : null}
                  {result.columns.map((c) => (
                    <td key={c.code_id} className="ex-num">
                      {pick(c.total, metric) > 0 ? (
                        <Button className="ex-cell-button" aria-label={t('{name}, all rows: {noun}. Show what is behind this number.', { name: c.name, noun: metricNoun(metric, pick(c.total, metric)) })} onPress={() => void open(`${c.name} · all rows`, null, c.code_id)}>
                          {num(pick(c.total, metric))}
                        </Button>
                      ) : (
                        <span>0</span>
                      )}
                    </td>
                  ))}
                  <td className="ex-num ex-total">
                    <Button className="ex-cell-button" aria-label={t('Everything shown together: {noun}. Show what is behind this number.', { noun: metricNoun(metric, pick(result.total, metric)) })} onPress={() => void open('Everything shown', null, null)}>
                      {num(pick(result.total, metric))}
                    </Button>
                  </td>
                </tr>
              </tfoot>
            </table>
          </div>
          {percent !== 'none' ? (
            <p className="muted ex-percent-note" data-testid="matrix-percent-note">
              {percentSentence(percent, metric, rowsName)}
            </p>
          ) : null}
          <MatrixNotes result={result} metric={metric} />
          <p className="actions">
            <Button
              onPress={() => {
                goExplore('visualize');
              }}
            >{t('Draw this as a chart')}</Button>
          </p>
        </>
      ) : loading ? (
        <p className="muted" role="status">{t('Counting…')}</p>
      ) : null}
      {selected ? <EvidenceList title={`${selected.title}: ${tn(selected.ids.length, { one: '{count} coding', other: '{count} codings' })}`} codingIds={selected.ids} onClose={() => setSelected(null)} testId="matrix-evidence" /> : null}
      {codes.length === 0 ? <Notice kind="info">{t('There are no codes yet, so there is nothing to compare.')}</Notice> : null}
    </div>
  );
}

/** The counting rules in words, and a warning when the totals are not the cells added up. */
export function MatrixNotes({ result, metric }: { result: MatrixResult; metric: 'assignments' | 'excerpts' | 'documents' | 'cases' }) {
  const cellSum = result.rows.reduce((n, r) => n + r.cells.reduce((m, c) => m + pick(c, metric), 0), 0);
  const total = pick(result.total, metric);
  return (
    <div className="ex-matrix-notes" data-testid="matrix-notes">
      <ul>
        {result.notes.map((n) => (
          <li key={n}>{engineText(n)}</li>
        ))}
        <li>{t('Totals count each {unit} once, so they can be smaller than the cells added up.', { unit: metric === 'assignments' ? t('coding') : metric === 'excerpts' ? t('excerpt') : metric === 'documents' ? t('document') : t('case') })}{' '}
          {cellSum !== total ? t('Here the cells add up to {cellSum} and the distinct total is {total}.', { cellSum: num(cellSum), total: num(total) }) : t('Here the two agree.')}
        </li>
      </ul>
    </div>
  );
}
