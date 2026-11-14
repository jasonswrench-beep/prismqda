import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { Button } from 'react-aria-components';
import { axisNumber, chartDescription, chartTable, chartTitle, countTicks, niceTicks, rowShares, scale, shorten, subjectName } from '../lib/charts';
import { fmt, percentText, valueLabel } from '../lib/statistics';
import { curveName } from '../lib/statistics2';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type { StatBoxGroup, StatChart, StatMeanGroup, StatResult } from '../types';
import { num, t, tn } from '../i18n';
import { StatExportDialog } from './StatExportDialog';

/**
 * The charts of the Statistics page, drawn as SVG from the data the engine sends. Each is a figure with a title, a text alternative
 * that says what it shows, and the table of what is drawn (so a chart is never the only way to get at a number). A mark can be
 * reached by hovering or, for charts with few marks, by the keyboard, and its numbers are written under the chart.
 */
export function StatChartFigure(props: { chart: StatChart; result: StatResult; id: string; page: number; caseName: (id: number) => string }) {
  const { chart, result, id } = props;
  const [readout, setReadout] = useState('');
  const [saving, setSaving] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  const textSize = useApp((s) => s.settings.textSize);
  // The picture is drawn on a canvas as wide as the room it has divided by the text size, and then shown that many times larger, so
  // its words are the size of the words round it, whatever size the person chose. A page too narrow for even the smallest canvas
  // scrolls the chart sideways inside its own box.
  const [layout, setLayout] = useState({ width: 640, k: 1, scrolls: false });
  useLayoutEffect(() => {
    const el = body.current;
    if (!el) return;
    const apply = () => {
      const room = Math.floor(el.getBoundingClientRect().width) || 640;
      const k = Math.max(0.75, (parseFloat(getComputedStyle(document.documentElement).fontSize) || 14) / 14);
      const width = Math.max(MIN_CANVAS, Math.min(MAX_CANVAS, Math.floor(room / k)));
      setLayout((was) => (was.width === width && was.k === k && was.scrolls === width * k > room + 1 ? was : { width, k, scrolls: width * k > room + 1 }));
    };
    apply();
    if (typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(apply);
    watch.observe(el);
    return () => watch.disconnect();
  }, [textSize]);
  const title = chartTitle(chart, result);
  const table = chartTable(chart, result, props.caseName);
  /** The handlers that make a mark show its words under the chart, on hover and on focus. */
  const mark = (words: string) => ({
    onMouseEnter: () => setReadout(words),
    onMouseLeave: () => setReadout(''),
    onFocus: () => {
      setReadout(words);
      announce(words);
    },
    onBlur: () => setReadout(''),
  });
  const keyed = (words: string) => ({ ...mark(words), tabIndex: 0, role: 'img' as const, 'aria-label': words });

  return (
    <figure id={id} className="stats-chart" aria-labelledby={`${id}-t`} aria-describedby={`${id}-d`} data-testid="stats-chart" data-chart={chart.kind}>
      <figcaption>
        <h6 id={`${id}-t`} className="stats-h5">{title}</h6>
        <p id={`${id}-d`} className="muted">{chartDescription(chart, result)}</p>
      </figcaption>
      <div ref={body} className="stats-chart-body" {...(layout.scrolls ? { tabIndex: 0, role: 'region', 'aria-label': t('{title}, page {n}, scrolls sideways', { title, n: num(props.page) }) } : {})}>
        <Body chart={chart} result={result} title={title} id={id} W={layout.width} k={layout.k} caseName={props.caseName} mark={mark} keyed={keyed} />
      </div>
      <p className="chart-readout" data-testid="chart-readout">{readout}</p>
      <div className="actions stats-chart-actions">
        <Button onPress={() => setSaving(true)} aria-label={t('Save the chart "{title}" as a picture', { title })} data-testid="stats-chart-save">
          {t('Save as picture…')}
        </Button>
      </div>
      {saving ? <StatExportDialog target={{ kind: 'chart', svg: () => body.current?.querySelector<SVGSVGElement>('svg.stats-svg') ?? null, name: title }} onClose={() => setSaving(false)} /> : null}
      {table.rows.length > 0 ? (
        <details className="stats-chart-table">
          <summary>{t('Show the numbers behind this chart')}</summary>
          <div className="table-wrap" tabIndex={0} role="region" aria-label={t('{title}, page {n}, numbers', { title, n: num(props.page) })}>
            <table className="data apa">
              <thead>
                <tr>
                  {table.head.map((h, i) => (
                    <th key={i} scope="col">{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {table.rows.map((row, ri) => (
                  <tr key={ri}>
                    {row.map((cell, ci) => (ci === 0 ? <th key={ci} scope="row">{cell}</th> : <td key={ci}>{cell}</td>))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </details>
      ) : null}
    </figure>
  );
}

/** The narrowest and widest canvas a chart is drawn on, in the units of the page's own text. */
const MIN_CANVAS = 300;
const MAX_CANVAS = 760;

type MarkProps = ReturnType<Props['mark']>;
type Props = {
  chart: StatChart;
  result: StatResult;
  title: string;
  id: string;
  /** The width of the canvas the picture is drawn on, and how many times larger it is shown. */
  W: number;
  k: number;
  caseName: (id: number) => string;
  mark: (words: string) => { onMouseEnter: () => void; onMouseLeave: () => void; onFocus: () => void; onBlur: () => void };
  keyed: (words: string) => MarkProps & { tabIndex: number; role: 'img'; 'aria-label': string };
};

function Body(p: Props): ReactNode {
  const c = p.chart;
  switch (c.kind) {
    case 'histogram':
      return <Histogram {...p} chart={c} />;
    case 'bars':
      return <Bars {...p} chart={c} />;
    case 'box':
      return <Boxes {...p} chart={c} />;
    case 'means':
      return <Means {...p} chart={c} />;
    case 'stacked':
      return <Stacked {...p} chart={c} />;
    case 'qq':
      return <Scatterish {...p} kind="qq" chart={c} />;
    case 'curves':
      return <Curves {...p} chart={c} />;
    default:
      return <Scatterish {...p} kind="scatter" chart={c} />;
  }
}

/** The frame of a chart: an SVG of a fixed shape that grows to the width it is given. */
function Svg(props: { width: number; k: number; height: number; label: string; interactive: boolean; children: ReactNode }) {
  return (
    <svg viewBox={`0 0 ${props.width} ${props.height}`} style={{ width: props.width * props.k }} className="stats-svg" role={props.interactive ? 'group' : 'img'} aria-label={props.label} focusable="false">
      {props.children}
    </svg>
  );
}

function XAxisTitle(props: { x: number; y: number; text: string }) {
  return (
    <text x={props.x} y={props.y} textAnchor="middle" className="chart-text" aria-hidden="true">
      {props.text}
    </text>
  );
}

function YAxisTitle(props: { y: number; text: string }) {
  return (
    <text transform={`translate(14 ${props.y}) rotate(-90)`} textAnchor="middle" className="chart-text" aria-hidden="true">
      {props.text}
    </text>
  );
}

// ---------------------------------------------------------------- histogram

function Histogram(p: Props & { chart: Extract<StatChart, { kind: 'histogram' }> }) {
  const W = p.W;
  const { chart: c } = p;
  const H = 320;
  const m = { l: 56, r: 16, t: 14, b: 54 };
  const lo = c.bins[0].lo;
  const hi = c.bins[c.bins.length - 1].hi;
  const x = scale([lo, hi], [m.l, W - m.r]);
  const counts = countTicks(Math.max(...c.bins.map((b) => b.count)));
  const y = scale([0, counts.hi], [H - m.b, m.t]);
  const edges = c.bins.length <= 10 ? [...c.bins.map((b) => b.lo), hi] : niceTicks(lo, hi, 6).ticks.filter((v) => v >= lo && v <= hi);
  const name = subjectName(c.subject, p.result);
  return (
    <Svg width={W} k={p.k} height={H} label={p.title} interactive={c.bins.length <= 12}>
      {counts.ticks.map((v) => (
        <g key={v} aria-hidden="true">
          <line x1={m.l} x2={W - m.r} y1={y(v)} y2={y(v)} className="chart-grid" />
          <text x={m.l - 8} y={y(v) + 4} textAnchor="end" className="chart-muted">{num(v)}</text>
        </g>
      ))}
      <line x1={m.l} x2={W - m.r} y1={H - m.b} y2={H - m.b} className="chart-axis" aria-hidden="true" />
      {edges.map((v) => (
        <text key={v} x={x(v)} y={H - m.b + 18} textAnchor="middle" className="chart-muted" aria-hidden="true">{axisNumber(v)}</text>
      ))}
      {c.bins.map((b, i) => {
        const words = t('{lo} to {hi}: {count}', { lo: axisNumber(b.lo), hi: axisNumber(b.hi), count: tn(b.count, { one: '{count} case', other: '{count} cases' }) });
        const props = c.bins.length <= 12 ? p.keyed(words) : p.mark(words);
        return b.count > 0 ? <rect key={i} x={x(b.lo) + 0.5} width={Math.max(1, x(b.hi) - x(b.lo) - 1)} y={y(b.count)} height={H - m.b - y(b.count)} className="chart-mark" {...props} /> : null;
      })}
      {c.reference !== null && c.reference >= lo && c.reference <= hi ? <line x1={x(c.reference)} x2={x(c.reference)} y1={m.t} y2={H - m.b} className="chart-ref" aria-hidden="true" /> : null}
      <XAxisTitle x={(m.l + W - m.r) / 2} y={H - 8} text={name} />
      <YAxisTitle y={(m.t + H - m.b) / 2} text={t('Cases')} />
    </Svg>
  );
}

// ---------------------------------------------------------------- bars of counts

function Bars(p: Props & { chart: Extract<StatChart, { kind: 'bars' }> }) {
  const W = p.W;
  const { chart: c } = p;
  const row = 30;
  const m = { l: 170, r: 90, t: 12, b: 36 };
  const H = m.t + m.b + c.categories.length * row;
  const max = Math.max(...c.categories.map((k) => k.count));
  const ticks = countTicks(max);
  const x = scale([0, ticks.hi], [m.l, W - m.r]);
  const v = p.result.variables[c.subject.type === 'variable' ? c.subject.index : 0];
  return (
    <Svg width={W} k={p.k} height={H} label={p.title} interactive={c.categories.length <= 12}>
      {ticks.ticks.map((tk) => (
        <g key={tk} aria-hidden="true">
          <line x1={x(tk)} x2={x(tk)} y1={m.t} y2={H - m.b} className="chart-grid" />
          <text x={x(tk)} y={H - m.b + 16} textAnchor="middle" className="chart-muted">{num(tk)}</text>
        </g>
      ))}
      {c.categories.map((k, i) => {
        const label = v ? valueLabel(v.kind, k.value, k.label) : k.label;
        const words = t('{label}: {count} of {valid} cases, {share}', { label, count: num(k.count), valid: num(c.valid), share: percentText(c.valid > 0 ? k.count / c.valid : 0) });
        const props = c.categories.length <= 12 ? p.keyed(words) : p.mark(words);
        const yy = m.t + i * row;
        return (
          <g key={k.value}>
            <text x={m.l - 8} y={yy + row / 2 + 4} textAnchor="end" className="chart-text" aria-hidden="true">{shorten(label, 24)}</text>
            {k.count > 0 ? <rect x={m.l} y={yy + 4} width={Math.max(1, x(k.count) - m.l)} height={row - 8} className="chart-mark" {...props} /> : null}
            <text x={x(k.count) + 6} y={yy + row / 2 + 4} className="chart-text" aria-hidden="true">{num(k.count)}</text>
          </g>
        );
      })}
      <XAxisTitle x={(m.l + W - m.r) / 2} y={H - 2} text={t('Cases')} />
    </Svg>
  );
}

// ---------------------------------------------------------------- box plots and means with intervals

/** The ticks of a range stretched by 4 percent each way, so that the lowest and highest marks do not sit on the edge of the plot. */
function paddedTicks(lo: number, hi: number, count: number) {
  const d = hi - lo || Math.abs(hi) * 0.1 || 1;
  return niceTicks(lo - d * 0.04, hi + d * 0.04, count);
}

/** The vertical scale and band positions shared by the charts with one mark for each group. */
function groupFrame(W: number, values: number[], k: number, height: number) {
  const m = { l: 62, r: 16, t: 14, b: 62 };
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  const ticks = paddedTicks(lo, hi, 6);
  const y = scale([ticks.lo, ticks.hi], [height - m.b, m.t]);
  const band = (W - m.l - m.r) / Math.max(1, k);
  const cx = (i: number) => m.l + band * (i + 0.5);
  return { m, ticks, y, band, cx };
}

function GroupLabels(props: { labels: string[]; ns: number[]; cx: (i: number) => number; height: number; bottom: number; band: number }) {
  return (
    <>
      {props.labels.map((label, i) => (
        <g key={i} aria-hidden="true">
          <text x={props.cx(i)} y={props.height - props.bottom + 18} textAnchor="middle" className="chart-text">{shorten(label, Math.max(6, Math.floor(props.band / 7)))}</text>
          <text x={props.cx(i)} y={props.height - props.bottom + 34} textAnchor="middle" className="chart-muted">{t('n = {n}', { n: num(props.ns[i]) })}</text>
        </g>
      ))}
    </>
  );
}

function Boxes(p: Props & { chart: Extract<StatChart, { kind: 'box' }> }) {
  const W = p.W;
  const { chart: c } = p;
  const H = 360;
  const all = c.groups.flatMap((g: StatBoxGroup) => [g.lower_whisker, g.upper_whisker, ...g.outliers]);
  const f = groupFrame(W, all, c.groups.length, H);
  const labelOf = (g: StatBoxGroup) => valueLabel(p.result.variables[c.by]?.kind ?? 'attribute', g.value, g.label);
  const w = Math.min(64, f.band * 0.5);
  return (
    <>
      <Svg width={W} k={p.k} height={H} label={p.title} interactive={c.groups.length <= 12}>
        {f.ticks.ticks.map((v) => (
          <g key={v} aria-hidden="true">
            <line x1={f.m.l} x2={W - f.m.r} y1={f.y(v)} y2={f.y(v)} className="chart-grid" />
            <text x={f.m.l - 8} y={f.y(v) + 4} textAnchor="end" className="chart-muted">{axisNumber(v)}</text>
          </g>
        ))}
        {c.groups.map((g, i) => {
          const words = t('{label}: median {median}, the middle half from {q1} to {q3}, whiskers from {lo} to {hi}, mean {mean}, {n}.', {
            label: labelOf(g),
            median: fmt(g.median),
            q1: fmt(g.q1),
            q3: fmt(g.q3),
            lo: fmt(g.lower_whisker),
            hi: fmt(g.upper_whisker),
            mean: fmt(g.mean),
            n: tn(g.n, { one: '{count} case', other: '{count} cases' }),
          });
          const cx = f.cx(i);
          const interact = c.groups.length <= 12 ? p.keyed(words) : p.mark(words);
          return (
            <g key={g.value}>
              <g aria-hidden="true">
                <line x1={cx} x2={cx} y1={f.y(g.upper_whisker)} y2={f.y(g.q3)} className="chart-whisker" />
                <line x1={cx} x2={cx} y1={f.y(g.q1)} y2={f.y(g.lower_whisker)} className="chart-whisker" />
                <line x1={cx - w / 4} x2={cx + w / 4} y1={f.y(g.upper_whisker)} y2={f.y(g.upper_whisker)} className="chart-whisker" />
                <line x1={cx - w / 4} x2={cx + w / 4} y1={f.y(g.lower_whisker)} y2={f.y(g.lower_whisker)} className="chart-whisker" />
              </g>
              <rect x={cx - w / 2} y={f.y(g.q3)} width={w} height={Math.max(1, f.y(g.q1) - f.y(g.q3))} className="chart-box" {...interact} />
              <g aria-hidden="true">
                <line x1={cx - w / 2} x2={cx + w / 2} y1={f.y(g.median)} y2={f.y(g.median)} className="chart-median" />
                <path d={`M ${cx} ${f.y(g.mean) - 5} l 5 5 l -5 5 l -5 -5 z`} className="chart-mean" />
                {g.outliers.map((o, k) => (
                  <circle key={k} cx={cx} cy={f.y(o)} r={3.5} className="chart-outlier" />
                ))}
              </g>
            </g>
          );
        })}
        <line x1={f.m.l} x2={W - f.m.r} y1={H - f.m.b} y2={H - f.m.b} className="chart-axis" aria-hidden="true" />
        <GroupLabels labels={c.groups.map(labelOf)} ns={c.groups.map((g) => g.n)} cx={f.cx} height={H} bottom={f.m.b} band={f.band} />
        <YAxisTitle y={(f.m.t + H - f.m.b) / 2} text={subjectName(c.subject, p.result)} />
      </Svg>
      <ul className="chart-legend" aria-label={t('How to read the box plot')}>
        <li>{t('The box holds the middle half of the cases, with a bar at the median.')}</li>
        <li>{t('A diamond marks the mean.')}</li>
        <li>{t('The whiskers reach the furthest values within 1.5 box lengths; a circle is a value beyond them.')}</li>
      </ul>
    </>
  );
}

function Means(p: Props & { chart: Extract<StatChart, { kind: 'means' }> }) {
  const W = p.W;
  const { chart: c } = p;
  const H = 340;
  const all = c.groups.flatMap((g: StatMeanGroup) => [g.mean, g.lo ?? g.mean, g.hi ?? g.mean]);
  const f = groupFrame(W, all, c.groups.length, H);
  const labelOf = (g: StatMeanGroup) => valueLabel(p.result.variables[c.by]?.kind ?? 'attribute', g.value, g.label);
  return (
    <Svg width={W} k={p.k} height={H} label={p.title} interactive={c.groups.length <= 12}>
      {f.ticks.ticks.map((v) => (
        <g key={v} aria-hidden="true">
          <line x1={f.m.l} x2={W - f.m.r} y1={f.y(v)} y2={f.y(v)} className="chart-grid" />
          <text x={f.m.l - 8} y={f.y(v) + 4} textAnchor="end" className="chart-muted">{axisNumber(v)}</text>
        </g>
      ))}
      {c.groups.map((g, i) => {
        const words = g.lo !== null && g.hi !== null
          ? t('{label}: mean {mean}, interval from {lo} to {hi}, {n}.', { label: labelOf(g), mean: fmt(g.mean), lo: fmt(g.lo), hi: fmt(g.hi), n: tn(g.n, { one: '{count} case', other: '{count} cases' }) })
          : t('{label}: mean {mean}, {n}.', { label: labelOf(g), mean: fmt(g.mean), n: tn(g.n, { one: '{count} case', other: '{count} cases' }) });
        const cx = f.cx(i);
        const interact = c.groups.length <= 12 ? p.keyed(words) : p.mark(words);
        return (
          <g key={g.value}>
            {g.lo !== null && g.hi !== null ? (
              <g aria-hidden="true">
                <line x1={cx} x2={cx} y1={f.y(g.lo)} y2={f.y(g.hi)} className="chart-whisker" />
                <line x1={cx - 8} x2={cx + 8} y1={f.y(g.lo)} y2={f.y(g.lo)} className="chart-whisker" />
                <line x1={cx - 8} x2={cx + 8} y1={f.y(g.hi)} y2={f.y(g.hi)} className="chart-whisker" />
              </g>
            ) : null}
            <circle cx={cx} cy={f.y(g.mean)} r={6} className="chart-mark" {...interact} />
          </g>
        );
      })}
      <line x1={f.m.l} x2={W - f.m.r} y1={H - f.m.b} y2={H - f.m.b} className="chart-axis" aria-hidden="true" />
      <GroupLabels labels={c.groups.map(labelOf)} ns={c.groups.map((g) => g.n)} cx={f.cx} height={H} bottom={f.m.b} band={f.band} />
      <YAxisTitle y={(f.m.t + H - f.m.b) / 2} text={t('Mean of {name}', { name: subjectName(c.subject, p.result) })} />
    </Svg>
  );
}

// ---------------------------------------------------------------- stacked bars of a table

function Stacked(p: Props & { chart: Extract<StatChart, { kind: 'stacked' }> }) {
  const W = p.W;
  const { chart: c } = p;
  const row = 36;
  const m = { l: 150, r: 64, t: 12, b: 40 };
  const H = m.t + m.b + c.counts.length * row;
  const shares = rowShares(c.counts);
  const x = scale([0, 1], [m.l, W - m.r]);
  const rv = p.result.variables[c.row];
  const cv = p.result.variables[c.col];
  const colName = (j: number) => (cv ? valueLabel(cv.kind, c.col_values[j], c.col_labels[j]) : c.col_labels[j]);
  const rowName = (i: number) => (rv ? valueLabel(rv.kind, c.row_values[i], c.row_labels[i]) : c.row_labels[i]);
  return (
    <>
      <ul className="chart-legend" aria-label={t('The colors, and what they stand for')} data-testid="chart-legend">
        {c.col_values.map((_, j) => (
          <li key={j}>
            <span className={`chart-swatch chart-series-${j + 1}`} aria-hidden="true" /> {colName(j)}
          </li>
        ))}
      </ul>
      <Svg width={W} k={p.k} height={H} label={p.title} interactive>
        {[0, 0.25, 0.5, 0.75, 1].map((v) => (
          <g key={v} aria-hidden="true">
            <line x1={x(v)} x2={x(v)} y1={m.t} y2={H - m.b} className="chart-grid" />
            <text x={x(v)} y={H - m.b + 16} textAnchor="middle" className="chart-muted">{percentText(v).replace('.0', '')}</text>
          </g>
        ))}
        {c.counts.map((counts, i) => {
          const total = counts.reduce((a, b) => a + b, 0);
          const words = t('{row}: {parts}', { row: rowName(i), parts: counts.map((n, j) => `${colName(j)} ${num(n)} (${percentText(shares[i][j])})`).join('; ') });
          let acc = 0;
          return (
            <g key={c.row_values[i]} {...p.keyed(words)}>
              <text x={m.l - 8} y={m.t + i * row + row / 2 + 4} textAnchor="end" className="chart-text" aria-hidden="true">{shorten(rowName(i), 20)}</text>
              {counts.map((n, j) => {
                const w = x(shares[i][j]) - x(0);
                const x0 = x(acc);
                acc += shares[i][j];
                return n > 0 ? (
                  <g key={j} aria-hidden="true">
                    <rect x={x0} y={m.t + i * row + 4} width={Math.max(0, w)} height={row - 8} className={`chart-seg chart-series-${(j % 8) + 1}`} />
                    {w >= 48 ? (
                      <>
                        <rect x={x0 + w / 2 - 20} y={m.t + i * row + row / 2 - 9} width={40} height={18} rx={9} className="chart-pill" />
                        <text x={x0 + w / 2} y={m.t + i * row + row / 2 + 4} textAnchor="middle" className="chart-seg-label">{percentText(shares[i][j]).replace('.0', '')}</text>
                      </>
                    ) : null}
                  </g>
                ) : null;
              })}
              <text x={W - m.r + 8} y={m.t + i * row + row / 2 + 4} className="chart-muted" aria-hidden="true">{t('n = {n}', { n: num(total) })}</text>
            </g>
          );
        })}
        <XAxisTitle x={(m.l + W - m.r) / 2} y={H - 2} text={t('Share of the cases in the row')} />
      </Svg>
    </>
  );
}


// ---------------------------------------------------------------- curves

/** The line styles of the curves, so that two curves are told apart by more than their color. */
const CURVE_DASH = ['', '8 4', '2 4', '10 4 2 4', '4 2', '12 3', '1 3', '6 3 1 3'];

function Curves(p: Props & { chart: Extract<StatChart, { kind: 'curves' }> }) {
  const W = p.W;
  const H = 360;
  const m = { l: 62, r: 16, t: 14, b: 52 };
  const { chart: c } = p;
  const xs = c.points.map((q) => q.x);
  const ys = c.points.map((q) => q.y);
  const xt = paddedTicks(Math.min(...xs), Math.max(...xs), 6);
  const yt = paddedTicks(Math.min(...ys), Math.max(...ys), 6);
  const x = scale([xt.lo, xt.hi], [m.l, W - m.r]);
  const y = scale([yt.lo, yt.hi], [H - m.b, m.t]);
  return (
    <>
      <ul className="chart-legend" aria-label={t('The curves, and how each is drawn')} data-testid="chart-legend">
        {c.curves.map((cv, j) => (
          <li key={cv.kind}>
            <svg className="chart-curve-swatch" width="30" height="10" aria-hidden="true" focusable="false">
              <line x1="1" x2="29" y1="5" y2="5" className={`chart-curve chart-curve-${(j % 8) + 1}`} strokeDasharray={CURVE_DASH[j % 8] || undefined} />
            </svg>{' '}
            {curveName(cv.kind)}
          </li>
        ))}
      </ul>
      <Svg width={W} k={p.k} height={H} label={p.title} interactive={false}>
        <defs>
          <clipPath id={`${p.id}-clip`}>
            <rect x={m.l} y={m.t} width={W - m.l - m.r} height={H - m.t - m.b} />
          </clipPath>
        </defs>
        {yt.ticks.map((v) => (
          <g key={`y${v}`} aria-hidden="true">
            <line x1={m.l} x2={W - m.r} y1={y(v)} y2={y(v)} className="chart-grid" />
            <text x={m.l - 8} y={y(v) + 4} textAnchor="end" className="chart-muted">{axisNumber(v)}</text>
          </g>
        ))}
        {xt.ticks.map((v) => (
          <g key={`x${v}`} aria-hidden="true">
            <line x1={x(v)} x2={x(v)} y1={m.t} y2={H - m.b} className="chart-grid" />
            <text x={x(v)} y={H - m.b + 16} textAnchor="middle" className="chart-muted">{axisNumber(v)}</text>
          </g>
        ))}
        <line x1={m.l} x2={W - m.r} y1={H - m.b} y2={H - m.b} className="chart-axis" aria-hidden="true" />
        <line x1={m.l} x2={m.l} y1={m.t} y2={H - m.b} className="chart-axis" aria-hidden="true" />
        <g clipPath={`url(#${p.id}-clip)`} aria-hidden="true">
          {c.curves.map((cv, j) => (
            <polyline key={cv.kind} points={cv.points.map(([a, b]) => `${x(a)},${y(b)}`).join(' ')} className={`chart-curve chart-curve-${(j % 8) + 1}`} strokeDasharray={CURVE_DASH[j % 8] || undefined} />
          ))}
          {c.points.map((q, i) => (
            <circle key={i} cx={x(q.x)} cy={y(q.y)} r={4} className="chart-point" {...p.mark(t('{case}: {x}, {y}', { case: p.caseName(q.case_id), x: fmt(q.x), y: fmt(q.y) }))} />
          ))}
        </g>
        <XAxisTitle x={(m.l + W - m.r) / 2} y={H - 8} text={subjectName(c.x, p.result)} />
        <YAxisTitle y={(m.t + H - m.b) / 2} text={subjectName(c.y, p.result)} />
      </Svg>
    </>
  );
}

// ---------------------------------------------------------------- scatter and normal probability plots

type ScatterProps = Props & ({ kind: 'scatter'; chart: Extract<StatChart, { kind: 'scatter' }> } | { kind: 'qq'; chart: Extract<StatChart, { kind: 'qq' }> });

function Scatterish(p: ScatterProps) {
  const W = p.W;
  const H = 360;
  const m = { l: 62, r: 16, t: 14, b: 52 };
  const pts = p.kind === 'qq' ? p.chart.points.map((q) => ({ x: q.theoretical, y: q.sample, id: 0 })) : p.chart.points.map((q) => ({ x: q.x, y: q.y, id: q.case_id }));
  const line = p.chart.kind === 'qq' ? p.chart.line : p.kind === 'scatter' ? p.chart.fit : null;
  const zero = p.kind === 'scatter' && p.chart.zero_line;
  const xs = pts.map((q) => q.x);
  const ys = pts.map((q) => q.y);
  const xt = paddedTicks(Math.min(...xs), Math.max(...xs), 6);
  const yt = paddedTicks(Math.min(...ys, ...(zero ? [0] : [])), Math.max(...ys, ...(zero ? [0] : [])), 6);
  const x = scale([xt.lo, xt.hi], [m.l, W - m.r]);
  const y = scale([yt.lo, yt.hi], [H - m.b, m.t]);
  const xName = p.kind === 'qq' ? t('Expected if the values were normal') : subjectName(p.chart.kind === 'scatter' ? p.chart.x : { type: 'fitted' }, p.result);
  const yName = p.kind === 'qq' ? subjectName(p.chart.subject, p.result) : subjectName((p.chart as Extract<StatChart, { kind: 'scatter' }>).y, p.result);
  return (
    <Svg width={W} k={p.k} height={H} label={p.title} interactive={false}>
      <defs>
        <clipPath id={`${p.id}-clip`}>
          <rect x={m.l} y={m.t} width={W - m.l - m.r} height={H - m.t - m.b} />
        </clipPath>
      </defs>
      {yt.ticks.map((v) => (
        <g key={`y${v}`} aria-hidden="true">
          <line x1={m.l} x2={W - m.r} y1={y(v)} y2={y(v)} className="chart-grid" />
          <text x={m.l - 8} y={y(v) + 4} textAnchor="end" className="chart-muted">{axisNumber(v)}</text>
        </g>
      ))}
      {xt.ticks.map((v) => (
        <g key={`x${v}`} aria-hidden="true">
          <line x1={x(v)} x2={x(v)} y1={m.t} y2={H - m.b} className="chart-grid" />
          <text x={x(v)} y={H - m.b + 16} textAnchor="middle" className="chart-muted">{axisNumber(v)}</text>
        </g>
      ))}
      <line x1={m.l} x2={W - m.r} y1={H - m.b} y2={H - m.b} className="chart-axis" aria-hidden="true" />
      <line x1={m.l} x2={m.l} y1={m.t} y2={H - m.b} className="chart-axis" aria-hidden="true" />
      <g clipPath={`url(#${p.id}-clip)`} aria-hidden="true">
        {zero ? <line x1={m.l} x2={W - m.r} y1={y(0)} y2={y(0)} className="chart-ref" /> : null}
        {line ? <line x1={x(xt.lo)} x2={x(xt.hi)} y1={y(line.intercept + line.slope * xt.lo)} y2={y(line.intercept + line.slope * xt.hi)} className="chart-line" /> : null}
        {pts.map((q, i) => (
          <circle
            key={i}
            cx={x(q.x)}
            cy={y(q.y)}
            r={4}
            className="chart-point"
            {...p.mark(p.kind === 'scatter' ? t('{case}: {x}, {y}', { case: p.caseName(q.id), x: fmt(q.x), y: fmt(q.y) }) : t('Expected {x}, value {y}', { x: fmt(q.x), y: fmt(q.y) }))}
          />
        ))}
      </g>
      <XAxisTitle x={(m.l + W - m.r) / 2} y={H - 8} text={xName} />
      <YAxisTitle y={(m.t + H - m.b) / 2} text={yName} />
    </Svg>
  );
}
