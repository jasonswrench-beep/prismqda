import { useEffect, useId, useMemo, useState } from 'react';
import { Button, Dialog, DialogTrigger, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components';
import { call } from '../bridge';
import { areaChart, busiestDay, changesPerDay, friendlyTime, ringSlices, typeShares } from '../lib/dashboard';
import { PALETTE } from '../lib/labels';
import { goTo, showPassages } from '../state/actions';
import { appStore, useApp } from '../state/app';
import { commandById, runCommand } from '../state/commands';
import { Icon, type IconName } from './Icon';
import { SOURCE_TYPE_LABELS, plural } from './common';
import { num, t, tn } from '../i18n';

const DAYS = 14;

/**
 * The top of the project dashboard, as in the screen specification book: the study's name and what it is about, when it
 * was last saved, four counts that lead to their lists, the changes made each day, and what the documents are made of.
 * Every number says what it counts, and the charts have the same numbers as a table for screen readers.
 */
export function DashboardTop({ memoCount }: { memoCount: number | null }) {
  const project = useApp((s) => s.project);
  const sources = useApp((s) => s.sources);
  const codes = useApp((s) => s.codes);
  const cases = useApp((s) => s.cases);
  const version = useApp((s) => s.dataVersion);

  const activeCodes = codes.filter((c) => c.status === 'active').length;
  const codings = sources.reduce((n, s) => n + s.codings, 0);
  const units = sources.reduce((n, s) => n + s.units, 0);

  const cards: { label: string; value: number | null; icon: IconName; what: string; open: () => void }[] = [
    { label: t('Documents'), value: sources.length, icon: 'src-text', what: t('Every file imported into this project. Press to see them in the data table.'), open: () => goTo('data') },
    { label: t('Cases'), value: cases.length, icon: 'src-case', what: t('The people, sites, or groups that documents belong to. Press to see them in the data table.'), open: () => goTo('data') },
    { label: t('Codes'), value: activeCodes, icon: 'code-code', what: t('Active codes in the codebook, parents included. Press to go to the codes.'), open: () => goTo('code') },
    { label: t('Coded Segments'), value: codings, icon: 'cmd-retrieve', what: t('Every coding made: one code on one stretch of text. Press to see them in Retrieved Segments.'), open: () => showPassages() },
  ];

  return (
    <>
      <header className="bk-dash-head">
        <div className="bk-dash-titles">
          <h2 id="h-home" className="bk-dash-crumb">{t('Project Dashboard – {name}', { name: project?.name ?? t('Project') })}
          </h2>
          <div className="bk-dash-title">
            <span className="bk-dash-name">{project?.name ?? t('Project')}</span>
            <ProjectInfo memoCount={memoCount} units={units} />
          </div>
          <p className="bk-dash-about" data-testid="dashboard-about">
            {project?.research_questions?.trim() ? project.research_questions : t('No research question was recorded when this project was made. Study details can be written as a memo.')}
          </p>
        </div>
        <div className="bk-dash-side">
          <SavedStamp />
          <ProjectDetailsMenu />
        </div>
      </header>

      <ul className="bk-dash-cards" aria-label={t('Counts in this project')}>
        {cards.map((c) => (
          <li key={c.label}>
            <Button className="bk-dash-card" onPress={c.open} aria-describedby={`dash-what-${c.label.replace(/\W/g, '')}`}>
              <Icon name={c.icon} size={30} className="bk-dash-icon" />
              <span className="bk-dash-text">
                <span className="bk-dash-value">{c.value === null ? '…' : num(c.value)}</span>
                <span className="bk-dash-label">{c.label}</span>
              </span>
            </Button>
            <span id={`dash-what-${c.label.replace(/\W/g, '')}`} className="visually-hidden">
              {c.what}
            </span>
          </li>
        ))}
      </ul>

      <div className="bk-dash-charts">
        <ActivityChart version={version} />
        <TypeRing />
      </div>

      <div className="bk-dash-actions">
        <Button onPress={() => void runCommand(commandById('new-project'))}>{t('New Project')}</Button>
        <Button onPress={() => void runCommand(commandById('import'))}>{t('Import Data')}</Button>
      </div>
    </>
  );
}

function SavedStamp() {
  const state = useApp((s) => s.saveState);
  const savedAt = useApp((s) => s.savedAt);
  const error = useApp((s) => s.saveError);
  let text = t('Last saved: …');
  if (state === 'saving') text = t('Saving…');
  else if (state === 'failed') text = error ? t('Not saved: {error}', { error }) : t('Not saved');
  else if (savedAt !== null) text = t('Last saved: {when}', { when: friendlyTime(savedAt, Date.now()) });
  return (
    <p className={`bk-dash-saved${state === 'failed' ? ' failed' : ''}`} data-testid="dashboard-saved">
      <Icon name={state === 'failed' ? 'st-save-failed' : 'st-saved'} size={16} />
      <span>{text}</span>
    </p>
  );
}

function ProjectDetailsMenu() {
  const run = (key: string) => {
    if (key === 'study') goTo('synthesize');
    else if (key === 'backup') void runCommand(commandById('backup'));
    else if (key === 'export') void runCommand(commandById('export'));
    else if (key === 'activity') appStore.set({ homeView: 'activity' });
  };
  return (
    <MenuTrigger>
      <Button className="bk-dash-details">
        <span>{t('Project Details')}</span>
        <Icon name="ui-chevron-down" size={16} />
      </Button>
      <Popover className="popover" placement="bottom end">
        <Menu className="menu" aria-label={t('Project Details')} onAction={(key) => run(String(key))}>
          <MenuItem id="study" className="menu-item">{t('Study details (memo)')}</MenuItem>
          <MenuItem id="activity" className="menu-item">{t('All activity')}</MenuItem>
          <MenuItem id="backup" className="menu-item">{t('Save a backup copy now')}</MenuItem>
          <MenuItem id="export" className="menu-item">{t('Export the codebook or passages…')}</MenuItem>
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

/** The (i) beside the name: what kind of project this is and where its file is. */
function ProjectInfo({ memoCount, units }: { memoCount: number | null; units: number }) {
  const project = useApp((s) => s.project);
  const coders = useApp((s) => s.coders);
  return (
    <DialogTrigger>
      <Button className="bk-dash-info" aria-label={t('About {name}: where its file is and how it codes', { name: project?.name ?? t('this project') })}>
        <Icon name="st-info" size={18} />
      </Button>
      <Popover className="popover bk-popover" placement="bottom start">
        <Dialog className="bk-menu-dialog" aria-label={t('About this project')}>
          <dl className="bk-facts bk-dash-facts">
            <div>
              <dt>{t('File')}</dt>
              <dd className="bk-path">{project?.path}</dd>
            </div>
            <div>
              <dt>{t('Coding')}</dt>
              <dd>{project?.coding_mode === 'unit' ? t('Whole units') : t('Selected words')}</dd>
            </div>
            <div>
              <dt>{t('Units')}</dt>
              <dd>{num(units)}</dd>
            </div>
            <div>
              <dt>{t('Memos')}</dt>
              <dd>{memoCount === null ? '…' : num(memoCount)}</dd>
            </div>
            <div>
              <dt>{t('Coders')}</dt>
              <dd>{num(coders.length)}</dd>
            </div>
            {project?.method_template && project.method_template !== 'free' ? (
              <div>
                <dt>{t('Method')}</dt>
                <dd>{project.method_template}</dd>
              </div>
            ) : null}
            {project?.sample_id ? (
              <div>
                <dt>{t('Sample')}</dt>
                <dd>{t('A practice copy. The bundled study is unchanged.')}</dd>
              </div>
            ) : null}
            {project?.read_only ? (
              <div>
                <dt>{t('Access')}</dt>
                <dd>{t('Read only: it was made by a newer version.')}</dd>
              </div>
            ) : null}
          </dl>
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}

// ---------------------------------------------------------------- Project Activity

const PLOT = { width: 560, height: 230, left: 40, right: 14, top: 14, bottom: 32 };

function ActivityChart({ version }: { version: number }) {
  const [stamps, setStamps] = useState<string[] | null>(null);
  const gradient = useId();
  useEffect(() => {
    let live = true;
    call('events.list', { limit: 5000 })
      .then((e) => live && setStamps(e.map((x) => x.ts)))
      .catch(() => live && setStamps([]));
    return () => {
      live = false;
    };
  }, [version]);

  const days = useMemo(() => (stamps ? changesPerDay(stamps, new Date(), DAYS) : null), [stamps]);
  const chart = useMemo(() => (days ? areaChart(days, PLOT) : null), [days]);
  const busiest = days ? busiestDay(days) : null;
  const total = days ? days.reduce((n, d) => n + d.count, 0) : 0;

  return (
    <section className="bk-dash-panel" aria-labelledby="h-activity">
      <h3 id="h-activity">{t('Project Activity')}</h3>
      <p className="muted bk-dash-sub">{t('Changes made to the project each day, over the last {DAYS} days. An import, a coding, an edit, or a removal each count as one; opening the project does not.', { DAYS })}</p>
      {!chart || !days ? (
        <p className="muted" role="status">{t('Loading…')}</p>
      ) : (
        <>
          <svg viewBox={`0 0 ${PLOT.width} ${PLOT.height}`} className="bk-chart" aria-hidden="true" focusable="false" data-testid="activity-chart">
            <defs>
              <linearGradient id={gradient} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="currentColor" stopOpacity="0.28" />
                <stop offset="100%" stopColor="currentColor" stopOpacity="0.02" />
              </linearGradient>
            </defs>
            {[0, 0.5, 1].map((f) => {
              const y = PLOT.top + (PLOT.height - PLOT.top - PLOT.bottom) * (1 - f);
              return (
                <g key={f}>
                  <line x1={PLOT.left} x2={PLOT.width - PLOT.right} y1={y} y2={y} className="bk-grid" />
                  <text x={PLOT.left - 8} y={y + 4} textAnchor="end" className="bk-axis">
                    {Math.round(chart.top * f)}
                  </text>
                </g>
              );
            })}
            <path d={chart.area} fill={`url(#${gradient})`} />
            <path d={chart.line} className="bk-line-path" />
            {chart.points.map((p) => (
              <circle key={p.day.date} cx={p.x} cy={p.y} r={p.day.count > 0 ? 3.2 : 2} className={p.day.count > 0 ? 'bk-dot' : 'bk-dot quiet'} />
            ))}
            {chart.points.map((p, i) =>
              // Every third day, and the last; the third from the end is skipped so it does not run into "Today".
              (i % 3 === 0 && i <= chart.points.length - 3) || i === chart.points.length - 1 ? (
                <text key={p.day.date} x={p.x} y={PLOT.height - 10} textAnchor={i === chart.points.length - 1 ? 'end' : 'middle'} className="bk-axis">
                  {p.day.label}
                </text>
              ) : null,
            )}
          </svg>
          <p className="bk-dash-note" data-testid="activity-summary">
            {total === 0
              ? t('No changes in the last {DAYS} days.', { DAYS })
              : (busiest
                ? t('{changes} in the last {DAYS} days; the busiest day was {day}, with {count}.', { changes: tn(total, { one: '{count} change', other: '{count} changes' }), DAYS, day: busiest.isToday ? t('today') : busiest.label, count: num(busiest.count) })
                : t('{changes} in the last {DAYS} days.', { changes: tn(total, { one: '{count} change', other: '{count} changes' }), DAYS }))}
          </p>
          <table className="visually-hidden" data-testid="activity-table">
            <caption>{t('Changes made to the project each day, last {DAYS} days', { DAYS })}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Day')}</th>
                <th scope="col">{t('Changes')}</th>
              </tr>
            </thead>
            <tbody>
              {days.map((d) => (
                <tr key={d.date}>
                  <th scope="row">{d.label}</th>
                  <td>{d.count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </section>
  );
}

// ---------------------------------------------------------------- Data by Type

const RING = { r: 52, stroke: 22, size: 150 };

function TypeRing() {
  const sources = useApp((s) => s.sources);
  const shares = useMemo(() => typeShares(sources.map((s) => s.source_type), SOURCE_TYPE_LABELS), [sources]);
  const slices = useMemo(() => ringSlices(shares, RING.r), [shares]);
  const colors = PALETTE.slice(0, 6).map((p) => p.hex);
  const center = RING.size / 2;

  return (
    <section className="bk-dash-panel" aria-labelledby="h-bytype">
      <h3 id="h-bytype">{t('Data by Type')}</h3>
      <p className="muted bk-dash-sub">{t('The share of the {documents} that each kind makes up. A document is counted once, under its own kind, so the shares add up to 100.', { documents: tn(sources.length, { one: '{count} document', other: '{count} documents' }) })}</p>
      {sources.length === 0 ? (
        <p className="muted">{t('No documents yet. Import some and their kinds appear here.')}</p>
      ) : (
        <div className="bk-dash-ring">
          <svg viewBox={`0 0 ${RING.size} ${RING.size}`} className="bk-donut" aria-hidden="true" focusable="false" data-testid="type-ring">
            <circle cx={center} cy={center} r={RING.r} className="bk-ring-track" strokeWidth={RING.stroke} />
            <g transform={`rotate(-90 ${center} ${center})`}>
              {slices.map((s, i) => (
                <circle key={shares[i].type} cx={center} cy={center} r={RING.r} fill="none" stroke={colors[i % colors.length]} strokeWidth={RING.stroke} strokeDasharray={s.dash} strokeDashoffset={s.offset} />
              ))}
            </g>
            <text x={center} y={center - 2} textAnchor="middle" className="bk-ring-total">
              {num(sources.length)}
            </text>
            <text x={center} y={center + 16} textAnchor="middle" className="bk-axis">
              {tn(sources.length, { one: 'document', other: 'documents' })}
            </text>
          </svg>
          <ul className="bk-legend" aria-label={t('Documents by kind')}>
            {shares.map((s, i) => (
              <li key={s.type}>
                <span className="bk-swatch" style={{ background: colors[i % colors.length] }} aria-hidden="true" />
                <span className="bk-legend-name">{s.label}</span>
                <span className="bk-legend-value">
                  {s.percent}% <span className="muted">({num(s.count)})</span>
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
