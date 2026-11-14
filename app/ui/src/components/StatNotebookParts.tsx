import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { Button } from 'react-aria-components';
import { toApiError } from '../bridge';
import { dateTime, num, t, tn } from '../i18n';
import { compareTable } from '../lib/statCompare';
import { resultTitle } from '../lib/statistics';
import { announce } from '../state/announce';
import { NO_SECTION, addNotePage, analyses, loadEarlierRun, movePage, removeFromNotebook, rerunAll, rerunPage, statsStore, updatePage, useStats, type NotePage, type NotebookEntry, type NotebookItem } from '../state/statistics';
import { Notice } from './common';

/**
 * The parts of the notebook that belong to its being kept in the project: the bar above the pages (add a page of notes, run every
 * analysis again, the index, and the sections), and what each page carries beside its tables (the author's note, its section, running
 * it again and comparing the runs, and moving it).
 */

function when(iso: string | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateTime(d, { dateStyle: 'medium', timeStyle: 'short' });
}

function jumpTo(id: number): void {
  const el = document.getElementById(`stats-page-${id}`);
  if (!el) return;
  el.scrollIntoView({ block: 'start' });
  el.querySelector<HTMLElement>('h4')?.focus();
}

/** The sections the pages are in, in the order they first appear. */
export function sectionsOf(items: NotebookItem[]): string[] {
  const out: string[] = [];
  for (const i of items) if (i.section !== '' && !out.includes(i.section)) out.push(i.section);
  return out;
}

export function inSection(item: NotebookItem, shown: string): boolean {
  return shown === '' || (shown === NO_SECTION ? item.section === '' : item.section === shown);
}

// ---------------------------------------------------------------- the bar

export function NotebookBar({ items, onExport }: { items: NotebookItem[]; onExport: () => void }) {
  const summary = useStats((s) => s.rerunSummary);
  const shown = useStats((s) => s.sectionShown);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pages = analyses(items);
  const sections = sectionsOf(items);
  const number = new Map(pages.map((e, i) => [e.id, i + 1]));
  const run = async (what: () => Promise<void>) => {
    setError(null);
    setBusy(true);
    try {
      await what();
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <div className="actions">
        {pages.length > 0 ? <Button onPress={onExport} data-testid="stats-export">{t('Export the notebook…')}</Button> : null}
        <Button isDisabled={busy} data-testid="stats-add-note" onPress={() => void run(async () => void (await addNotePage()))}>{t('Add a page of notes')}</Button>
        {pages.length > 0 ? (
          <Button
            isDisabled={busy}
            data-testid="stats-rerun-all"
            onPress={() =>
              void run(async () => {
                const s = await rerunAll();
                announce(t('Ran {n} again: {changed} changed, {failed} could not be run.', { n: tn(s.total, { one: '{count} analysis', other: '{count} analyses' }), changed: num(s.changed), failed: num(s.failed) }));
              })
            }
          >
            {t('Run all the analyses again')}
          </Button>
        ) : null}
      </div>
      {busy ? <p role="status" className="muted" data-testid="stats-busy">{t('Running the analyses. A bootstrap or a model on many cases can take a while.')}</p> : null}
      {error ? <Notice>{error}</Notice> : null}
      {summary ? (
        <p role="status" className={summary.failed > 0 ? 've-problem' : 'muted'} data-testid="stats-rerun-summary">
          {tn(summary.total, { one: '{count} analysis was run again on the project as it is now.', other: '{count} analyses were run again on the project as it is now.' })}{' '}
          {summary.changed === 0 ? t('None of them changed.') : tn(summary.changed, { one: '{count} changed; its page shows what it said before.', other: '{count} changed; each page shows what it said before.' })}
          {summary.failed > 0 ? ` ${tn(summary.failed, { one: '{count} could not be run any more, and its page is as it was.', other: '{count} could not be run any more, and their pages are as they were.' })}` : ''}
        </p>
      ) : null}
      {items.length > 1 ? (
        <nav aria-label={t('The pages of the notebook')} data-testid="stats-index">
          <h4 className="stats-h4">{t('Pages')}</h4>
          <ol className="stats-index">
            {items.map((it) => (
              <li key={it.id}>
                <Button
                  className="link-button"
                  data-testid="stats-index-item"
                  onPress={() => {
                    if (!inSection(it, shown)) statsStore.set({ sectionShown: '' });
                    setTimeout(() => jumpTo(it.id), 0);
                  }}
                >
                  {it.kind === 'analysis' ? t('Page {n}: {title}', { n: num(number.get(it.id) ?? 0), title: resultTitle(it.result) }) : t('Notes: {title}', { title: it.title.trim() === '' ? t('untitled') : it.title })}
                </Button>
                {it.section !== '' ? <span className="muted"> ({it.section})</span> : null}
              </li>
            ))}
          </ol>
        </nav>
      ) : null}
      {sections.length > 0 ? (
        <div role="group" aria-label={t('Show the pages of one section')} className="stats-sections" data-testid="stats-sections">
          {[{ key: '', label: t('All pages') }, ...sections.map((s) => ({ key: s, label: s })), ...(items.some((i) => i.section === '') ? [{ key: NO_SECTION, label: t('No section') }] : [])].map((o) => (
            <Button key={o.key} aria-pressed={shown === o.key} className={`stats-section-tab${shown === o.key ? ' is-on' : ''}`} data-testid={`stats-section-${o.key === '' ? 'all' : o.key === NO_SECTION ? 'none' : o.label}`} onPress={() => statsStore.set({ sectionShown: o.key })}>
              {o.label}
            </Button>
          ))}
        </div>
      ) : null}
    </>
  );
}

// ---------------------------------------------------------------- a page's own tools

function useSaved<T extends string>(value: T, save: (v: T) => Promise<void>): [T, (v: T) => void, () => void, string | null] {
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (draft === value) return;
    setError(null);
    save(draft).catch((e) => setError(toApiError(e).message));
  };
  return [draft, setDraft, commit, error];
}

function SectionField({ item }: { item: NotebookItem }) {
  const notebook = useStats((s) => s.notebook);
  const sections = useMemo(() => sectionsOf(notebook), [notebook]);
  const [draft, setDraft, commit, error] = useSaved(item.section, (v) => updatePage(item.id, { section: v }));
  return (
    <div className="field">
      <label htmlFor={`stats-section-${item.id}`}>{t('Section')}</label>
      <input
        id={`stats-section-${item.id}`}
        list={`stats-sections-${item.id}`}
        type="text"
        maxLength={60}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === 'Enter' && commit()}
        autoComplete="off"
        data-testid="stats-section-input"
      />
      <datalist id={`stats-sections-${item.id}`}>
        {sections.map((s) => (
          <option key={s} value={s} />
        ))}
      </datalist>
      {error ? <span className="ve-problem" role="alert">{error}</span> : null}
    </div>
  );
}

function Mover({ item, label }: { item: NotebookItem; label: (up: boolean) => string }) {
  const items = useStats((s) => s.notebook);
  const at = items.findIndex((i) => i.id === item.id);
  return (
    <>
      <Button isDisabled={at <= 0} aria-label={label(true)} data-testid="stats-move-up" onPress={() => void movePage(item.id, true)}>{t('Move up')}</Button>
      <Button isDisabled={at < 0 || at >= items.length - 1} aria-label={label(false)} data-testid="stats-move-down" onPress={() => void movePage(item.id, false)}>{t('Move down')}</Button>
    </>
  );
}

function Comparison({ entry, number }: { entry: NotebookEntry; number: number }) {
  const cmp = useStats((s) => s.comparisons[entry.id]);
  const [error, setError] = useState<string | null>(null);
  if (cmp?.error) {
    return (
      <p role="alert" className="ve-problem" data-testid="stats-rerun-error">
        {t('This analysis could not be run again: {error} The page is as it was.', { error: cmp.error })}
      </p>
    );
  }
  if (cmp && !cmp.changed) {
    return (
      <p className="muted" role="status" data-testid="stats-rerun-same">
        {t('Run again {when}: nothing changed.', { when: when(entry.ranAt) })}
      </p>
    );
  }
  if (cmp?.before) {
    const table = compareTable(cmp.before, entry.result);
    return (
      <div className="table-wrap" tabIndex={0} role="region" aria-label={t('{title}, page {n}', { title: table.title, n: num(number) })} data-testid="stats-compare">
        <table className="data apa" data-testid="stats-compare-table">
          <caption>{t('This run ({now}) compared with the run of {before}', { now: when(entry.ranAt), before: when(cmp.beforeAt) })}</caption>
          <thead>
            <tr>
              {table.head.map((h, j) => (
                <th key={j} scope="col">{h}</th>
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
        {table.note ? <p className="muted stats-note">{table.note}</p> : null}
      </div>
    );
  }
  if (entry.earlierRuns > 0) {
    return (
      <>
        <Button data-testid="stats-compare-load" onPress={() => void loadEarlierRun(entry.id).catch((e) => setError(toApiError(e).message))}>{t('Compare with the earlier run')}</Button>
        {error ? <span className="ve-problem" role="alert">{error}</span> : null}
      </>
    );
  }
  return null;
}

/** What an analysis page carries below its tables: the author's note, the section, and running it again. */
export function PageTools({ entry, number }: { entry: NotebookEntry; number: number }): ReactNode {
  const [draft, setDraft, commit, error] = useSaved(entry.note, (v) => updatePage(entry.id, { note: v }).then(() => announce(t('Saved the note on page {n}.', { n: num(number) }))));
  const [busy, setBusy] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);
  return (
    <div className="stats-page-tools" data-testid="stats-page-tools">
      <div className="field">
        <label htmlFor={`stats-note-${entry.id}`}>{t('Your note on page {n}', { n: num(number) })}</label>
        <textarea id={`stats-note-${entry.id}`} rows={3} maxLength={20000} value={draft} onChange={(e) => setDraft(e.target.value)} onBlur={commit} data-testid="stats-note" />
        {error ? <span className="ve-problem" role="alert">{error}</span> : null}
      </div>
      <SectionField item={entry} />
      <div className="actions">
        <Button
          isDisabled={busy}
          data-testid="stats-rerun"
          onPress={async () => {
            setBusy(true);
            setRunError(null);
            try {
              const r = await rerunPage(entry.id);
              announce(r.error !== null ? t('Page {n} could not be run again.', { n: num(number) }) : r.changed ? t('Page {n} was run again, and it changed.', { n: num(number) }) : t('Page {n} was run again, and nothing changed.', { n: num(number) }));
            } catch (e) {
              setRunError(toApiError(e).message);
            } finally {
              setBusy(false);
            }
          }}
          aria-label={t('Run the analysis on page {n} again', { n: num(number) })}
        >
          {t('Run this analysis again')}
        </Button>
        <Mover item={entry} label={(up) => (up ? t('Move page {n} up', { n: num(number) }) : t('Move page {n} down', { n: num(number) }))} />
      </div>
      {runError ? <Notice>{runError}</Notice> : null}
      <Comparison entry={entry} number={number} />
    </div>
  );
}

// ---------------------------------------------------------------- a page of notes

export function NoteCard({ item }: { item: NotePage }) {
  const [title, setTitle, commitTitle, titleError] = useSaved(item.title, (v) => updatePage(item.id, { title: v }));
  const [body, setBody, commitBody, bodyError] = useSaved(item.note, (v) => updatePage(item.id, { note: v }));
  return (
    <li id={`stats-page-${item.id}`} className="ac-card stats-page-card stats-note-card" data-testid="stats-note-page">
      <h4 tabIndex={-1} className="stats-page-title">{t('Notes: {title}', { title: item.title.trim() === '' ? t('untitled') : item.title })}</h4>
      <div className="field">
        <label htmlFor={`stats-note-title-${item.id}`}>{t('Title of the page of notes')}</label>
        <input id={`stats-note-title-${item.id}`} type="text" maxLength={120} value={title} onChange={(e) => setTitle(e.target.value)} onBlur={commitTitle} onKeyDown={(e) => e.key === 'Enter' && commitTitle()} autoComplete="off" data-testid="stats-note-title" />
        {titleError ? <span className="ve-problem" role="alert">{titleError}</span> : null}
      </div>
      <div className="field">
        <label htmlFor={`stats-note-body-${item.id}`}>{t('Notes')}</label>
        <textarea id={`stats-note-body-${item.id}`} rows={6} maxLength={20000} value={body} onChange={(e) => setBody(e.target.value)} onBlur={commitBody} data-testid="stats-note-body" />
        {bodyError ? <span className="ve-problem" role="alert">{bodyError}</span> : null}
      </div>
      <SectionField item={item} />
      <div className="actions">
        <Mover item={item} label={(up) => (up ? t('Move the page of notes up') : t('Move the page of notes down'))} />
        <Button onPress={() => void removeFromNotebook(item.id)} aria-label={t('Remove the page of notes')} data-testid="stats-note-remove">{t('Remove')}</Button>
      </div>
    </li>
  );
}
