// Saved queries: a research question kept under a name, opened again later, frozen so its answer stops changing, and compared
// with what the same question finds now.

import { useEffect, useState } from 'react';
import { Button, Input, Label, TextField } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { draftKey, problemWith, revive, toQuerySpec } from '../../lib/explore';
import { formatWhen } from '../../lib/eventWording';
import { bumpData, setExploreQuery, undoLast } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import type { FrozenComparison, SavedQueryRow } from '../../types';
import { Notice, plural } from '../common';
import { EvidenceList, resultCount } from './shared';
import { rich, t, tn } from '../../i18n';

type Form = { mode: 'save' | 'saveas' | 'rename'; id: number | null; name: string; note: string };

export function SavedQueries() {
  const { draft, saved } = useApp((s) => s.exploreQuery);
  const version = useApp((s) => s.dataVersion);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [list, setList] = useState<SavedQueryRow[] | null>(null);
  const [form, setForm] = useState<Form | null>(null);
  const [note, setNote] = useState<{ text: string; canUndo: boolean } | null>(null);
  const [compare, setCompare] = useState<{ name: string; cmp: FrozenComparison } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    call('query.list')
      .then((r) => live && setList(r))
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version]);

  const problem = problemWith(draft);
  const key = problem ? null : draftKey(draft);
  // The builder has changed since the saved query was opened or saved, even if what it asks is not complete yet.
  const dirty = saved !== null && draftKey(draft) !== saved.key;
  const current = list?.find((q) => q.id === saved?.id) ?? null;

  const fail = (e: unknown) => {
    const m = toApiError(e).message;
    setError(m);
    announce(m, 'assertive');
  };

  const run = async <T,>(fn: () => Promise<T>): Promise<T | undefined> => {
    setError(null);
    try {
      return await fn();
    } catch (e) {
      fail(e);
      return undefined;
    }
  };

  const say = (text: string, canUndo = false) => {
    setNote({ text, canUndo });
    announce(text);
  };

  const save = async () => {
    if (!form || key === null) return;
    const spec = toQuerySpec(draft);
    if (form.mode === 'rename' && form.id !== null) {
      const r = await run(() => call('query.update', { id: form.id!, name: form.name, note: form.note }));
      if (!r) return;
      if (saved?.id === r.id) setExploreQuery({ saved: { ...saved, name: r.name } });
      setForm(null);
      bumpData();
      say(t('Renamed the query to {name}.', { name: r.name }), true);
      return;
    }
    const r = await run(() => call('query.save', { name: form.name, note: form.note, draft, spec }));
    if (!r) return;
    setExploreQuery({ saved: { id: r.id, key, name: r.name, frozen: false } });
    setForm(null);
    bumpData();
    say(t('Saved the query {name}.', { name: r.name }), true);
  };

  const update = async () => {
    if (!saved || key === null) return;
    const r = await run(() => call('query.update', { id: saved.id, draft, spec: toQuerySpec(draft) }));
    if (!r) return;
    setExploreQuery({ saved: { ...saved, key } });
    bumpData();
    say(t('Updated the saved query {name} with what is in the builder now.', { name: r.name }), true);
  };

  const open = async (id: number) => {
    const q = await run(() => call('query.get', { id }));
    if (!q) return;
    const restored = revive(q.draft);
    if (!restored) {
      setError(t('The saved query {name} cannot be opened in the builder: it was saved by another version of the program.', { name: q.name }));
      return;
    }
    const frozen = q.frozen_at !== null;
    const k = draftKey(restored);
    setExploreQuery({ draft: restored, saved: { id: q.id, key: k, name: q.name, frozen }, ran: null });
    const setThen = appStore.get().workingSet;
    const result = await run(() => (frozen ? call('query.run_saved', { id }).then((r) => r.result) : call('explore.query', toQuerySpec(restored))));
    if (result) setExploreQuery({ ran: { key: k, result, at: Date.now(), workingSet: setThen } });
    setCompare(null);
    say(
      frozen
        ? t('Opened {name}. It is frozen: it shows the {codings} it found on {when}, as they are now.', { name: q.name, codings: tn(q.frozen_count ?? 0, { one: '{count} coding', other: '{count} codings' }), when: formatWhen(q.frozen_at ?? '') })
        : result ? t('Opened {name} and ran it: {results}.', { name: q.name, results: resultCount(result.total, restored.unit) }) : t('Opened {name} and ran it: no answer.', { name: q.name }),
    );
  };

  const freeze = async (row: SavedQueryRow) => {
    const r = await run(() => call('query.freeze', { id: row.id }));
    if (!r) return;
    if (saved?.id === row.id) setExploreQuery({ saved: { ...saved, frozen: true } });
    bumpData();
    say(t('Froze {name}: it found {codings} ({results}), and now keeps that answer.', { name: row.name, codings: tn(r.codings, { one: '{count} coding', other: '{count} codings' }), results: tn(r.results, { one: '{count} result', other: '{count} results' }) }), true);
  };

  const unfreeze = async (row: SavedQueryRow) => {
    const r = await run(() => call('query.unfreeze', { id: row.id }));
    if (!r) return;
    if (saved?.id === row.id) setExploreQuery({ saved: { ...saved, frozen: false } });
    setCompare(null);
    bumpData();
    say(t('{name} follows the coding again.', { name: row.name }), true);
  };

  const remove = async (row: SavedQueryRow) => {
    const r = await run(() => call('query.delete', { id: row.id }));
    if (!r) return;
    if (saved?.id === row.id) setExploreQuery({ saved: null });
    setCompare(null);
    bumpData();
    say(t('Deleted the saved query {name}.', { name: row.name }), true);
  };

  const compareWith = async (row: SavedQueryRow) => {
    const cmp = await run(() => call('query.compare', { id: row.id }));
    if (!cmp) return;
    setCompare({ name: row.name, cmp });
    announce(t('{name}: {codings} found now and not when it was frozen, {codings2} no longer found.', { name: row.name, codings: tn(cmp.added, { one: '{count} coding', other: '{count} codings' }), codings2: tn(cmp.no_longer_found, { one: '{count} coding', other: '{count} codings' }) }));
  };

  return (
    <section className="ex-saved" aria-labelledby="h-saved" data-testid="saved-queries">
      <h3 id="h-saved" className="bk-h3">{t('Saved queries')}</h3>
      {saved ? (
        <p className="ex-saved-banner" data-testid="saved-banner">{saved.frozen
            ? current?.frozen_at
              ? rich('This is the saved query <strong>{name}</strong> (frozen on {when}: its answer does not follow the coding, and what it asks cannot be changed until it is unfrozen).', { name: saved.name, when: formatWhen(current.frozen_at), strong: (inside) => <strong>{inside}</strong> })
              : rich('This is the saved query <strong>{name}</strong> (frozen: its answer does not follow the coding, and what it asks cannot be changed until it is unfrozen).', { name: saved.name, strong: (inside) => <strong>{inside}</strong> })
            : rich('This is the saved query <strong>{name}</strong>.', { name: saved.name, strong: (inside) => <strong>{inside}</strong> })}{' '}
          {dirty ? t('The builder has changes that are not saved in it.') : t('The builder matches it.')}
        </p>
      ) : (
        <p className="muted">{t('Keep a query under a name to run it again later, or freeze it so its answer stops changing as the coding grows.')}</p>
      )}
      <div className="actions">
        <Button
          onPress={() => setForm({ mode: 'save', id: null, name: '', note: '' })}
          isDisabled={readOnly || problem !== null}
          data-testid="save-query"
        >
          {saved ? t('Save as a new query') : t('Save this query')}
        </Button>
        {saved ? (
          <Button onPress={() => void update()} isDisabled={readOnly || !dirty || saved.frozen || problem !== null} data-testid="update-query">{t('Update {name}', { name: saved.name })}
          </Button>
        ) : null}
        {problem !== null ? <span className="muted">{t('A query that cannot be run yet cannot be saved: {problem}', { problem })}</span> : null}
      </div>

      {form ? (
        <form
          className="ex-saved-form"
          aria-label={form.mode === 'rename' ? t('Rename the query') : t('Save the query')}
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <TextField value={form.name} onChange={(v) => setForm({ ...form, name: v })} className="field" autoFocus>
            <Label>{t('Name of the query')}</Label>
            <Input data-testid="query-name" />
          </TextField>
          <TextField value={form.note} onChange={(v) => setForm({ ...form, note: v })} className="field">
            <Label>{t('A note about it (optional)')}</Label>
            <Input />
          </TextField>
          <div className="actions">
            <Button type="submit" className="primary" isDisabled={form.name.trim() === ''} data-testid="query-name-save">
              {form.mode === 'rename' ? t('Rename') : t('Save')}
            </Button>
            <Button onPress={() => setForm(null)}>{t('Cancel')}</Button>
          </div>
        </form>
      ) : null}

      {error ? <Notice>{error}</Notice> : null}
      {note ? (
        <div className="ac-result" role="status" data-testid="saved-note">
          <p>{note.text}</p>
          <div className="actions">
            {note.canUndo ? (
              <Button
                onPress={() => {
                  setNote(null);
                  void undoLast();
                }}
                isDisabled={readOnly}
                data-testid="saved-undo"
              >{t('Undo')}</Button>
            ) : null}
            <Button onPress={() => setNote(null)}>{t('Dismiss')}</Button>
          </div>
        </div>
      ) : null}

      {list === null ? (
        <p className="muted" role="status">{t('Loading the saved queries…')}</p>
      ) : list.length === 0 ? (
        <p className="help" data-testid="saved-none">{t('Nothing has been saved yet.')}</p>
      ) : (
        <ul className="ex-saved-list" aria-label={t('Saved queries')}>
          {list.map((q) => (
            <li key={q.id} className={q.id === saved?.id ? 'current' : undefined} data-testid="saved-row">
              <div className="ex-saved-head">
                <strong>{q.name}</strong>
                <span className={`ac-status${q.frozen_at ? ' warn' : ''}`}>{q.frozen_at ? t('Frozen: {codings}, {when}', { codings: tn(q.frozen_count ?? 0, { one: '{count} coding', other: '{count} codings' }), when: formatWhen(q.frozen_at) }) : t('Follows the coding')}</span>
              </div>
              {q.note ? <p className="muted">{q.note}</p> : null}
              <div className="actions">
                <Button onPress={() => void open(q.id)} aria-label={t('Open the saved query {name}', { name: q.name })}>{t('Open')}</Button>
                <Button onPress={() => setForm({ mode: 'rename', id: q.id, name: q.name, note: q.note })} isDisabled={readOnly || key === null} aria-label={t('Rename {name}', { name: q.name })}>{t('Rename')}</Button>
                {q.frozen_at ? (
                  <>
                    <Button onPress={() => void compareWith(q)} aria-label={t('Compare {name} with what the same question finds now', { name: q.name })}>{t('Compare with now')}</Button>
                    <Button onPress={() => void unfreeze(q)} isDisabled={readOnly} aria-label={t('Unfreeze {name}', { name: q.name })}>{t('Unfreeze')}</Button>
                    <Button onPress={() => void freeze(q)} isDisabled={readOnly} aria-label={t('Freeze {name} again, as the coding is now', { name: q.name })}>{t('Freeze again')}</Button>
                  </>
                ) : (
                  <Button onPress={() => void freeze(q)} isDisabled={readOnly} aria-label={t('Freeze {name}', { name: q.name })}>{t('Freeze')}</Button>
                )}
                <Button onPress={() => void remove(q)} isDisabled={readOnly} aria-label={t('Delete {name}', { name: q.name })}>{t('Delete')}</Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {compare ? (
        <div className="ex-compare" role="region" aria-label={t('{name} compared with now', { name: compare.name })} data-testid="saved-compare">
          <h4 className="bk-h3">{t('{name}: then and now', { name: compare.name })}</h4>
          <ul>
            <li>{t('Frozen on {when}: {codings} found.', { when: formatWhen(compare.cmp.frozen_at), codings: tn(compare.cmp.frozen, { one: '{count} coding', other: '{count} codings' }) })}</li>
            <li>{t('The same question finds {codings} now.', { codings: tn(compare.cmp.now, { one: '{count} coding', other: '{count} codings' }) })}</li>
            <li>{tn(compare.cmp.still_found, { one: '{codings} is found both times.', other: '{codings} are found both times.' }, { codings: tn(compare.cmp.still_found, { one: '{count} coding', other: '{count} codings' }) })}</li>
            <li>{tn(compare.cmp.added, { one: '{codings} is found now and not then.', other: '{codings} are found now and not then.' }, { codings: tn(compare.cmp.added, { one: '{count} coding', other: '{count} codings' }) })}</li>
            <li>
              {tn(compare.cmp.no_longer_found, { one: '{codings} is no longer found.', other: '{codings} are no longer found.' }, { codings: tn(compare.cmp.no_longer_found, { one: '{count} coding', other: '{count} codings' }) })}
              {compare.cmp.no_longer_found > 0 ? ' ' + tn(compare.cmp.removed, { one: '{codings} of them was removed from the project; the others were changed so they no longer match.', other: '{codings} of them were removed from the project; the others were changed so they no longer match.' }, { codings: tn(compare.cmp.removed, { one: '{count} coding', other: '{count} codings' }) }) : ''}</li>
          </ul>
          {compare.cmp.added > 0 ? <EvidenceList title={t('Found now and not when {name} was frozen', { name: compare.name })} codingIds={compare.cmp.added_ids} onClose={() => setCompare(null)} testId="saved-compare-added" /> : null}
        </div>
      ) : null}
    </section>
  );
}
