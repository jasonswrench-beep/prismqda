import { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, TextArea, TextField, Label } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { CATEGORY_LABELS, describeEvent, foldEvents, formatWhen, type EventCategory, type WordingLookups } from '../lib/eventWording';
import { announce } from '../state/announce';
import { quoteFromOpenSource, sourceText } from '../state/actions';
import { useApp } from '../state/app';
import type { EventRow } from '../types';
import { Notice, plural } from './common';
import { t, tn } from '../i18n';

const LIMIT = 500;
const MAX_SOURCES_FOR_QUOTES = 12;

export function HistoryView() {
  const codes = useApp((s) => s.codes);
  const sources = useApp((s) => s.sources);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [events, setEvents] = useState<EventRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<EventCategory | 'all'>('all');
  const [note, setNote] = useState('');
  const [ready, setReady] = useState(0);

  const load = useCallback(async () => {
    try {
      const list = await call('events.list', { limit: LIMIT });
      setEvents(list);
      setError(null);
    } catch (e) {
      setError(toApiError(e).message);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Fetch the text of the sources that coding events point at, so quotes can be shown.
  useEffect(() => {
    if (!events) return;
    const ids = new Set<number>();
    for (const e of [...events].reverse()) {
      const body = e.action === 'coding.add' ? e.after : e.action === 'coding.remove' ? e.before : null;
      const id = body && typeof body.source_id === 'number' ? body.source_id : null;
      if (id !== null) ids.add(id);
      if (ids.size >= MAX_SOURCES_FOR_QUOTES) break;
    }
    let cancelled = false;
    void Promise.all([...ids].map((id) => sourceText(id).catch(() => null))).then(() => {
      if (!cancelled) setReady((n) => n + 1);
    });
    return () => {
      cancelled = true;
    };
  }, [events]);

  const lookups = useMemo<WordingLookups>(
    () => ({
      codeName: (id) => (id === null || id === undefined ? null : (codes.find((c) => c.id === id)?.name ?? null)),
      sourceName: (id) => (id === null || id === undefined ? null : (sources.find((s) => s.id === id)?.name ?? null)),
      quote: (sourceId, start, end) => quoteFromOpenSource(sourceId, start, end),
    }),
    // `ready` changes when more source text has arrived.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [codes, sources, ready],
  );

  const described = useMemo(() => {
    if (!events) return [];
    return foldEvents(events).reverse().map((e) => ({ event: e, d: describeEvent(e, lookups) }));
  }, [events, lookups]);
  const shown = filter === 'all' ? described : described.filter((x) => x.d.category === filter);

  const addNote = async () => {
    if (!note.trim()) return setError(t('Write the note first.'));
    try {
      await call('events.annotate', { note: note.trim() });
      setNote('');
      announce(t('Added the note to the history.'));
      await load();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  return (
    <main id="main-content" tabIndex={-1} className="view-page" aria-labelledby="h-history">
      <h2 id="h-history">{t('History')}</h2>
      <p className="help">{t('Everything done in this project, newest first. It cannot be edited or deleted, but you can add a note to explain a decision.')}</p>
      {error ? <Notice>{error}</Notice> : null}

      {readOnly ? null : (
        <form
          className="note-form"
          onSubmit={(e) => {
            e.preventDefault();
            void addNote();
          }}
        >
          <TextField value={note} onChange={setNote} className="field">
            <Label>{t('Add a note to the history')}</Label>
            <TextArea rows={2} />
          </TextField>
          <Button type="submit" className="primary">{t('Add note')}</Button>
        </form>
      )}

      <div className="field filter">
        <label htmlFor="history-filter">{t('Show')}</label>
        <select id="history-filter" value={filter} onChange={(e) => setFilter(e.target.value as EventCategory | 'all')}>
          <option value="all">{t('Everything')}</option>
          {(Object.keys(CATEGORY_LABELS) as EventCategory[]).map((k) => (
            <option key={k} value={k}>
              {CATEGORY_LABELS[k]}
            </option>
          ))}
        </select>
        <span className="muted" role="status">
          {events === null ? t('Loading…') : events.length >= LIMIT ? t('{entries} (the newest {limit} are loaded)', { entries: tn(shown.length, { one: '{count} entry', other: '{count} entries' }), limit: LIMIT }) : tn(shown.length, { one: '{count} entry', other: '{count} entries' })}
        </span>
      </div>

      <ol className="timeline" aria-label={t('History')} data-testid="timeline">
        {shown.map(({ event, d }) => (
          <li key={event.id} className={`event event-${d.category}`} data-action={event.action}>
            <time dateTime={event.ts} className="event-time">
              {formatWhen(event.ts)}
            </time>
            <span className="event-text">{d.text}</span>
            {d.detail ? <span className="event-detail muted">{d.detail}</span> : null}
            <span className="event-who muted">{t('by {actor}', { actor: event.actor })}</span>
            {event.action === 'memo.add' && event.after && event.after.kind === 'method_decision' ? <span className="tag">{t('Method decision')}</span> : null}
            <span className="tag">{CATEGORY_LABELS[d.category]}</span>
          </li>
        ))}
      </ol>
    </main>
  );
}
