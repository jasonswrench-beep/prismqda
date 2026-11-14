// Crosstabs: how many cases have each pair of values of two case variables, with percentages that say what they are
// percentages of, and every cell opening to the cases in it.

import { useEffect, useMemo, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { CROSSTAB_PERCENTS, crosstabPercentSentence, crosstabWhole, intensity, percentText, rangeWidth, type CrosstabPercent } from '../../lib/explore';
import { openAt, setExploreCrosstab } from '../../state/actions';
import { announce } from '../../state/announce';
import { useApp } from '../../state/app';
import type { CaseRef, Crosstab, CrosstabSpec } from '../../types';
import { Notice, plural } from '../common';
import { useCaseVariables } from './Matrix';
import { num, t, tn } from '../../i18n';
import { engineText } from '../../i18n/engine';

interface Open {
  title: string;
  cases: CaseRef[];
}

/** The numeric-looking part of a variable: a width can only group numbers. */
const NUMERIC_LEVELS = new Set(['interval', 'ratio']);

export function Crosstabs() {
  const setup = useApp((s) => s.exploreCrosstab);
  const version = useApp((s) => s.dataVersion);
  const caseCount = useApp((s) => s.cases.length);
  const vars = useCaseVariables();
  const [table, setTable] = useState<Crosstab | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [open, setOpen] = useState<Open | null>(null);

  const rowWidth = rangeWidth(setup.rowWidth);
  const columnWidth = rangeWidth(setup.columnWidth);
  const spec: CrosstabSpec | null = useMemo(
    () =>
      setup.row !== '' && setup.column !== ''
        ? { row_variable: setup.row, column_variable: setup.column, ...(rowWidth ? { row_width: rowWidth } : {}), ...(columnWidth ? { column_width: columnWidth } : {}) }
        : null,
    [setup.row, setup.column, rowWidth, columnWidth],
  );
  const key = JSON.stringify(spec);

  useEffect(() => {
    setOpen(null);
    if (!spec) {
      setTable(null);
      setError(null);
      return;
    }
    let live = true;
    setLoading(true);
    call('explore.crosstab', spec)
      .then((t) => {
        if (!live) return;
        setTable(t);
        setError(null);
        setLoading(false);
      })
      .catch((e) => {
        if (!live) return;
        setTable(null);
        setError(toApiError(e).message);
        setLoading(false);
      });
    return () => {
      live = false;
    };
    // `key` is the spec; the table is also read again when the cases change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version, caseCount]);

  const show = async (title: string, row: string | null, column: string | null) => {
    if (!spec) return;
    try {
      const cases = await call('explore.crosstab_cases', { spec, row_key: row, column_key: column });
      setOpen({ title, cases });
      announce(t('{title}: {cases} shown below.', { title, cases: tn(cases.length, { one: '{count} case', other: '{count} cases' }) }));
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };

  const rowVar = vars.find((v) => v.name === setup.row);
  const colVar = vars.find((v) => v.name === setup.column);
  const max = table ? Math.max(0, ...table.cells.flat()) : 0;

  return (
    <div data-testid="crosstab-tool">
      <section className="ex-explore-controls" aria-label={t('Crosstab setup')} data-testid="crosstab-controls">
        <div className="field">
          <label htmlFor="xt-row">{t('Rows')}</label>
          <select id="xt-row" value={setup.row} onChange={(e) => setExploreCrosstab({ row: e.target.value, rowWidth: '' })}>
            <option value="">{t('Choose a case variable')}</option>
            {vars.map((v) => (
              <option key={v.name} value={v.name}>
                {v.name}
              </option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor="xt-col">{t('Columns')}</label>
          <select id="xt-col" value={setup.column} onChange={(e) => setExploreCrosstab({ column: e.target.value, columnWidth: '' })}>
            <option value="">{t('Choose a case variable')}</option>
            {vars.map((v) => (
              <option key={v.name} value={v.name}>
                {v.name}
              </option>
            ))}
          </select>
        </div>
        {rowVar && (NUMERIC_LEVELS.has(rowVar.level) || rowVar.numbers > 0) ? (
          <div className="field">
            <label htmlFor="xt-rw">{t('Group the row numbers into ranges this wide')}</label>
            <input id="xt-rw" type="text" inputMode="decimal" value={setup.rowWidth} onChange={(e) => setExploreCrosstab({ rowWidth: e.target.value })} placeholder="for example 10" />
          </div>
        ) : null}
        {colVar && (NUMERIC_LEVELS.has(colVar.level) || colVar.numbers > 0) ? (
          <div className="field">
            <label htmlFor="xt-cw">{t('Group the column numbers into ranges this wide')}</label>
            <input id="xt-cw" type="text" inputMode="decimal" value={setup.columnWidth} onChange={(e) => setExploreCrosstab({ columnWidth: e.target.value })} placeholder="for example 10" />
          </div>
        ) : null}
        <div className="field">
          <label htmlFor="xt-percent">{t('Show as')}</label>
          <select id="xt-percent" value={setup.percent} onChange={(e) => setExploreCrosstab({ percent: e.target.value as CrosstabPercent })} data-testid="crosstab-percent">
            {CROSSTAB_PERCENTS.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </div>
      </section>

      {error ? <Notice>{error}</Notice> : null}
      {vars.length === 0 ? (
        <p className="help">{t('This project has no case variables yet. They come from the columns of a spreadsheet of survey answers that describe each respondent, when it is imported with a column that names the case.')}</p>
      ) : !spec ? (
        <p className="help">{t('Choose a case variable for the rows and another for the columns. The table counts how many cases have each pair of values.')}</p>
      ) : table ? (
        <>
          <p className="muted" data-testid="crosstab-universe">
            {loading ? t('Updating…') + ' ' : ''}{t('Counting {cases}. Every case is counted once; a case with no value for a variable is in the row or column called (no value).', { cases: tn(table.total, { one: '{count} case', other: '{count} cases' }) })}</p>
          <div className="ex-table-wrap">
            <table className="ex-matrix ex-crosstab" data-testid="crosstab-table">
              <caption>{t('Cases by {variable} (rows) and {variable2} (columns)', { variable: table.rows.variable, variable2: table.columns.variable })}</caption>
              <thead>
                <tr>
                  <th scope="col">{table.rows.variable}</th>
                  {table.columns.values.map((c) => (
                    <th scope="col" key={c.key || 'none'}>
                      {c.label}
                    </th>
                  ))}
                  <th scope="col">{t('Total')}</th>
                </tr>
              </thead>
              <tbody>
                {table.rows.values.map((r, i) => (
                  <tr key={r.key || 'none'}>
                    <th scope="row">{r.label}</th>
                    {table.columns.values.map((c, j) => {
                      const n = table.cells[i][j];
                      const whole = setup.percent === 'none' ? null : crosstabWhole(setup.percent, r.total, c.total, table.total);
                      const shown = setup.percent === 'none' ? num(n) : `${num(n)} (${percentText(n, whole)})`;
                      const cases = tn(n, { one: '{count} case', other: '{count} cases' });
                      const spoken = setup.percent === 'none' ? cases : whole === null ? t('{cases}, no percentage', { cases }) : t('{cases}, {percent} of {whole}', { cases, percent: percentText(n, whole), whole: num(whole) });
                      return (
                        <td key={c.key || 'none'} className="ex-num ex-cell" style={n > 0 ? { ['--shade' as string]: String(intensity(n, max)) } : undefined}>
                          {n > 0 ? (
                            <Button className="ex-cell-button" aria-label={t('{label}, {label2}: {spoken}. Show these cases.', { label: r.label, label2: c.label, spoken })} onPress={() => void show(`${r.label} · ${c.label}`, r.key, c.key)}>
                              {shown}
                            </Button>
                          ) : (
                            <span aria-label={t('{label}, {label2}: no cases', { label: r.label, label2: c.label })}>{setup.percent === 'none' ? '0' : `0 (${percentText(0, whole)})`}</span>
                          )}
                        </td>
                      );
                    })}
                    <td className="ex-num ex-total">
                      {r.total > 0 ? (
                        <Button className="ex-cell-button" aria-label={t('{label}, all columns: {cases}. Show these cases.', { label: r.label, cases: tn(r.total, { one: '{count} case', other: '{count} cases' }) })} onPress={() => void show(t('{label} · all columns', { label: r.label }), r.key, null)}>
                          {num(r.total)}
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
                  <th scope="row">{t('Total')}</th>
                  {table.columns.values.map((c) => (
                    <td key={c.key || 'none'} className="ex-num">
                      {c.total > 0 ? (
                        <Button className="ex-cell-button" aria-label={t('{label}, all rows: {cases}. Show these cases.', { label: c.label, cases: tn(c.total, { one: '{count} case', other: '{count} cases' }) })} onPress={() => void show(t('{label} · all rows', { label: c.label }), null, c.key)}>
                          {num(c.total)}
                        </Button>
                      ) : (
                        <span>0</span>
                      )}
                    </td>
                  ))}
                  <td className="ex-num ex-total">
                    <Button className="ex-cell-button" aria-label={t('All the cases counted: {total}. Show these cases.', { total: num(table.total) })} onPress={() => void show(t('All the cases counted'), null, null)}>
                      {num(table.total)}
                    </Button>
                  </td>
                </tr>
              </tfoot>
            </table>
          </div>
          {setup.percent !== 'none' ? (
            <p className="muted ex-percent-note" data-testid="crosstab-percent-note">
              {crosstabPercentSentence(setup.percent, table.rows.variable, table.columns.variable)}
            </p>
          ) : null}
          <div className="ex-matrix-notes" data-testid="crosstab-notes">
            <ul>
              {table.notes.map((n) => (
                <li key={n}>{engineText(n)}</li>
              ))}
              <li>{t('This table counts cases. It does not test whether the pattern is more than chance; that belongs to the statistics tools, which are not built yet.')}</li>
            </ul>
          </div>
        </>
      ) : loading ? (
        <p className="muted" role="status">{t('Counting…')}</p>
      ) : null}

      {open ? (
        <section className="ex-evidence" aria-labelledby="h-xt-cases" data-testid="crosstab-cases">
          <div className="ex-evidence-head">
            <h3 id="h-xt-cases" className="bk-h3">
              {open.title}: {tn(open.cases.length, { one: '{count} case', other: '{count} cases' })}
            </h3>
            <Button className="bk-link" onPress={() => setOpen(null)}>{t('Close the list')}</Button>
          </div>
          <ul className="bk-example-list">
            {open.cases.map((c) => (
              <li key={c.id}>
                <span>{c.name}</span>{' '}
                {c.source_id !== null ? (
                  <Button onPress={() => void openAt(c.source_id!, c.start ?? 0, c.end ?? 0)} aria-label={t('Open the words of {name}', { name: c.name })}>{t('Open its words')}</Button>
                ) : (
                  <span className="muted">{t('(no document)')}</span>
                )}
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}
