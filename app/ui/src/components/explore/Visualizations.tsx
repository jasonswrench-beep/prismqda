// Charts of the code matrix: the same numbers, the same setup, drawn as bars. A bar is one code in its own color, and its name
// and number are written beside it, so nothing is told by color alone; the table in Code Matrix has the same numbers.

import { metricLabel, metricNoun, niceMax, pick } from '../../lib/explore';
import { goExplore } from '../../state/actions';
import { useApp } from '../../state/app';
import { Button } from 'react-aria-components';
import { Notice } from '../common';
import { MatrixControls, MatrixNotes, useMatrix } from './Matrix';
import { num, rich, t } from '../../i18n';

interface Bar {
  key: string;
  label: string;
  value: number;
  color: string;
}

/** What a chart says, in a sentence, for anyone who cannot see it. */
export function describeBars(bars: readonly Bar[], metric: Parameters<typeof metricNoun>[0]): string {
  if (bars.length === 0) return t('There are no bars.');
  const top = bars.reduce((a, b) => (b.value > a.value ? b : a));
  const low = bars.reduce((a, b) => (b.value < a.value ? b : a));
  if (top.value === 0) return t('Every bar is zero.');
  return t('The longest bar is {label}, with {noun}. The shortest is {label2}, with {noun2}.', { label: top.label, noun: metricNoun(metric, top.value), label2: low.label, noun2: metricNoun(metric, low.value) });
}

/** Horizontal bars on one scale, each anchored at the baseline, with the name at the left and the number at the end. */
export function BarList(props: { id: string; title: string; bars: Bar[]; max: number; metric: Parameters<typeof metricNoun>[0]; testId?: string; level?: 3 | 4 }) {
  const Heading = props.level === 3 ? 'h3' : 'h4';
  const axisMax = niceMax(props.max);
  const ticks = axisMax <= 4 && Number.isInteger(axisMax) ? Array.from({ length: axisMax + 1 }, (_, i) => i) : [0, axisMax / 2, axisMax];
  return (
    <figure className="ex-chart" aria-labelledby={`${props.id}-title`} aria-describedby={`${props.id}-desc`} data-testid={props.testId}>
      <figcaption>
        <Heading id={`${props.id}-title`} className="bk-h3">
          {props.title}
        </Heading>
        <p id={`${props.id}-desc`} className="muted">
          {describeBars(props.bars, props.metric)}
        </p>
      </figcaption>
      <div className="ex-bars">
        <div className="ex-bars-axis" aria-hidden="true">
          {ticks.map((t) => (
            <span key={t} style={{ left: `${(t / axisMax) * 100}%` }}>
              {Number.isInteger(t) ? t : t.toFixed(1)}
            </span>
          ))}
        </div>
        <ul>
          {props.bars.map((b) => (
            <li key={b.key} className="ex-bar-row">
              <span className="ex-bar-label">{b.label}</span>
              <span className="ex-bar-track" aria-hidden="true">
                {b.value > 0 ? <span className="ex-bar" style={{ width: `${(b.value / axisMax) * 100}%`, background: b.color }} /> : null}
              </span>
              <span className="ex-bar-value">
                {num(b.value)}
                <span className="visually-hidden"> {metricNoun(props.metric, b.value).replace(/^[\d,]+ /, '')}</span>
              </span>
            </li>
          ))}
        </ul>
      </div>
    </figure>
  );
}

const MAX_GROUPS = 12;

export function Visualizations() {
  const { setup, result, error, loading, spec } = useMatrix();
  const codes = useApp((s) => s.codes);
  const metric = setup.metric;
  const colorOf = (id: number) => codes.find((c) => c.id === id)?.color ?? '#808080';
  const rowsName = setup.rowKind === 'cases' ? t('case') : setup.rowKind === 'documents' ? t('document') : setup.variable;

  const byCode: Bar[] = result ? result.columns.map((c) => ({ key: String(c.code_id), label: c.name, value: pick(c.total, metric), color: colorOf(c.code_id) })) : [];
  const groups = result ? result.rows.filter((r) => pick(r.total, metric) > 0).slice(0, MAX_GROUPS) : [];
  const shared = result ? Math.max(0, ...result.rows.flatMap((r) => r.cells.map((c) => pick(c, metric)))) : 0;

  return (
    <div data-testid="visualize-tool">
      <MatrixControls showPercent={false} />
      {error ? <Notice>{error}</Notice> : null}
      {!spec ? (
        <p className="help">{t('Choose the codes for the columns to draw them. The charts use the same setup as Code Matrix, so the bars and the table always agree.')}</p>
      ) : result ? (
        <>
          <p className="muted">
            {loading ? t('Updating…') + ' ' : ''}{rich('The metric is <strong>{metric}</strong>. A bar is one code; its name and number are written beside it.', { metric: metricLabel(metric), strong: (inside) => <strong>{inside}</strong> })}</p>
          <BarList id="chart-codes" title={t('{metric} by code', { metric: metricLabel(metric) })} bars={byCode} max={Math.max(0, ...byCode.map((b) => b.value))} metric={metric} testId="chart-by-code" level={3} />
          <h3 className="bk-h3">{t('By {rowsName}', { rowsName })}</h3>
          {groups.length === 0 ? (
            <p className="help">{t('No row has evidence for these codes.')}</p>
          ) : (
            <>
              <p className="muted">{t('One chart for each {rowsName}, all on the same scale so they can be compared.', { rowsName })}
                {result.rows.length > MAX_GROUPS ? ' ' + t('The first {MAX_GROUPS} with evidence are drawn; the table in Code Matrix has every row.', { MAX_GROUPS }) : ''}</p>
              <div className="ex-chart-grid">
                {groups.map((r) => (
                  <BarList
                    key={r.key}
                    id={`chart-${r.key.replace(/\W/g, '-')}`}
                    title={r.size !== null ? `${r.label} (N = ${r.size})` : r.label}
                    bars={r.cells.map((cell, i) => ({ key: String(result.columns[i].code_id), label: result.columns[i].name, value: pick(cell, metric), color: colorOf(result.columns[i].code_id) }))}
                    max={shared}
                    metric={metric}
                  />
                ))}
              </div>
            </>
          )}
          <MatrixNotes result={result} metric={metric} />
          <p className="actions">
            <Button onPress={() => goExplore('matrix')}>{t('See the numbers as a table')}</Button>
          </p>
        </>
      ) : loading ? (
        <p className="muted" role="status">{t('Counting…')}</p>
      ) : null}
    </div>
  );
}
