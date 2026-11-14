import { useEffect, useMemo, useState } from 'react';
import { Button } from 'react-aria-components';
import { call } from '../bridge';
import { describeCoverage } from '../lib/coverage';
import { describeEvent, foldEvents, formatWhen, type WordingLookups } from '../lib/eventWording';
import { goAutoCode, goTo, openAt, quoteFromOpenSource } from '../state/actions';
import { appStore, useApp } from '../state/app';
import type { EventRow } from '../types';
import { DashboardTop } from './Dashboard';
import { HistoryView } from './HistoryView';
import { plural } from './common';
import { compareText, N_, t, tn } from '../i18n';

const RECENT = 6;

/** Home: where the project stands, what to do next, and what was done last. History lives under it as Activity. */
export function HomeView() {
  const view = useApp((s) => s.homeView);
  return (
    <div className="home">
      <nav aria-label={t('Home views')} className="subnav">
        {(
          [
            ['overview', N_('Dashboard')],
            ['activity', N_('Activity')],
          ] as const
        ).map(([id, label]) => (
          <Button key={id} className={`subnav-button${view === id ? ' current' : ''}`} aria-current={view === id ? 'page' : undefined} onPress={() => appStore.set({ homeView: id })}>
            {t(label)}
          </Button>
        ))}
      </nav>
      {view === 'overview' ? <Overview /> : <HistoryView />}
    </div>
  );
}

function Overview() {
  const sources = useApp((s) => s.sources);
  const codes = useApp((s) => s.codes);
  const coverage = useApp((s) => s.coverage);
  const version = useApp((s) => s.dataVersion);
  const [memoCount, setMemoCount] = useState<number | null>(null);
  const [events, setEvents] = useState<EventRow[] | null>(null);
  /** Auto Code suggestions that wait for a decision. */
  const [waiting, setWaiting] = useState(0);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([call('memo.list', {}), call('events.list', { limit: RECENT * 4 }), call('proposal.batches', {})])
      .then(([memos, evs, batches]) => {
        if (cancelled) return;
        setMemoCount(memos.length);
        setEvents(evs);
        setWaiting(batches.reduce((n, b) => n + b.pending, 0));
      })
      .catch(() => {
        if (!cancelled) {
          setMemoCount(null);
          setEvents([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [version, sources.length, codes.length]);

  const activeCodes = useMemo(() => codes.filter((c) => c.status === 'active'), [codes]);
  const topCodes = useMemo(() => [...activeCodes].sort((a, b) => b.usage - a.usage || compareText(a.name, b.name)).filter((c) => c.usage > 0).slice(0, 6), [activeCodes]);
  const maxUsage = topCodes[0]?.usage ?? 1;
  const uncoded = sources.filter((s) => s.codings === 0);

  const lookups = useMemo<WordingLookups>(
    () => ({
      codeName: (id) => (id === null || id === undefined ? null : (codes.find((c) => c.id === id)?.name ?? null)),
      sourceName: (id) => (id === null || id === undefined ? null : (sources.find((s) => s.id === id)?.name ?? null)),
      quote: (sourceId, start, end) => quoteFromOpenSource(sourceId, start, end),
    }),
    [codes, sources],
  );
  const recent = useMemo(() => (events ? foldEvents(events).reverse().slice(0, RECENT).map((e) => ({ e, d: describeEvent(e, lookups) })) : []), [events, lookups]);

  // One suggestion, the most useful thing to do now.
  let next: { text: string; button: string; run: () => void } | null = null;
  if (sources.length === 0) next = { text: t('Bring in your first interview, document, or spreadsheet of survey answers.'), button: t('Import a file'), run: () => goTo('import') };
  else if (waiting > 0) next = { text: tn(waiting, { one: '{suggestions} from Auto Code or an AI is waiting for your decision. Nothing is coded until you accept it.', other: '{suggestions} from Auto Code or an AI are waiting for your decision. Nothing is coded until you accept it.' }, { suggestions: tn(waiting, { one: '{count} suggestion', other: '{count} suggestions' }) }), button: t('Review the suggestions'), run: () => goAutoCode('queue') };
  else if (activeCodes.length === 0) next = { text: t('Open a source in Code, select some words, and press I to make a code from them.'), button: t('Go to Code'), run: () => goTo('code') };
  else if (uncoded.length > 0) next = { text: tn(uncoded.length, { one: '{sources} has no coding yet. Start with {name}.', other: '{sources} have no coding yet. Start with {name}.' }, { sources: tn(uncoded.length, { one: '{count} source', other: '{count} sources' }), name: uncoded[0].name }), button: t('Open {name}', { name: uncoded[0].name }), run: () => void openAt(uncoded[0].id, 0, 0) };
  else next = { text: t('Every source has some coding. Write down what you are noticing, or export the coded passages.'), button: t('Write a memo'), run: () => goTo('synthesize') };

  return (
    <main id="main-content" tabIndex={-1} className="view-page home-overview" aria-labelledby="h-home">
      <DashboardTop memoCount={memoCount} />

      {next ? (
        <section aria-labelledby="h-next" className="card next-step">
          <h3 id="h-next">{t('Next step')}</h3>
          <p>{next.text}</p>
          <Button className="primary" onPress={next.run}>
            {next.button}
          </Button>
        </section>
      ) : null}

      <div className="home-columns">
        <section aria-labelledby="h-progress" className="card">
          <h3 id="h-progress">{t('Coding progress')}</h3>
          {sources.length === 0 ? (
            <p className="muted">{t('No sources yet.')}</p>
          ) : (
            <ul className="progress-list">
              {sources.slice(0, 12).map((s) => {
                const cov = coverage[s.id] ?? null;
                const pct = cov ? Math.round(cov.fraction * 100) : 0;
                return (
                  <li key={s.id}>
                    <Button className="link-button" onPress={() => void openAt(s.id, 0, 0)}>
                      {s.name}
                    </Button>
                    <span className="source-progress">
                      <span className="coverage" aria-hidden="true">
                        <span className="coverage-fill" style={{ width: `${pct}%` }} />
                      </span>
                      <span className="coverage-text">{describeCoverage(cov, s.codings)}</span>
                    </span>
                  </li>
                );
              })}
              {sources.length > 12 ? <li className="muted">{t('and {more_sources}. See them all in Data.', { more_sources: tn(sources.length - 12, { one: '{count} more source', other: '{count} more sources' }) })}</li> : null}
            </ul>
          )}
        </section>

        <section aria-labelledby="h-top" className="card">
          <h3 id="h-top">{t('Most used codes')}</h3>
          {topCodes.length === 0 ? (
            <p className="muted">{t('Nothing coded yet.')}</p>
          ) : (
            <ul className="bar-list">
              {topCodes.map((c) => (
                <li key={c.id}>
                  <span className="bar-name">{c.name}</span>
                  <span className="bar-track" aria-hidden="true">
                    <span className="bar-fill" style={{ width: `${Math.max(4, Math.round((c.usage / maxUsage) * 100))}%` }} />
                  </span>
                  <span className="bar-value">{tn(c.usage, { one: '{count} coding', other: '{count} codings' })}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      <section aria-labelledby="h-recent" className="card">
        <div className="card-head">
          <h3 id="h-recent">{t('Recent activity')}</h3>
          <Button className="small" onPress={() => appStore.set({ homeView: 'activity' })}>{t('See all activity')}</Button>
        </div>
        {events === null ? (
          <p className="muted" role="status">{t('Loading…')}</p>
        ) : recent.length === 0 ? (
          <p className="muted">{t('Nothing has happened in this project yet.')}</p>
        ) : (
          <ol className="recent-list">
            {recent.map(({ e, d }) => (
              <li key={e.id}>
                <time dateTime={e.ts} className="event-time">
                  {formatWhen(e.ts)}
                </time>
                <span>{d.text}</span>
              </li>
            ))}
          </ol>
        )}
      </section>
    </main>
  );
}


