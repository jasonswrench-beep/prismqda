import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { Button, Checkbox, Dialog, Heading, Input, Label, Modal, ModalOverlay, SearchField, TextArea, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { PALETTE, nextColor } from '../lib/labels';
import { descendantIds, pathLabel } from '../lib/tree';
import { applyCode, addCode, mergeCodes, saveSnapshot, setCounterexample, splitCode } from '../state/actions';
import { appStore, focusCenter, useApp, type PendingReplace } from '../state/app';
import type { SegmentRow } from '../types';
import { sliceScalar } from '../offsets';
import { Notice, plural } from './common';
import { compareText, rich, t, tn } from '../i18n';

function Shell(props: { title: string; id: string; children: ReactNode }) {
  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && appStore.set({ dialog: null })} className="overlay">
      <Modal className="modal">
        <Dialog className="dialog" aria-labelledby={props.id}>
          <Heading slot="title" id={props.id} className="dialog-title">
            {props.title}
          </Heading>
          {props.children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

const close = () => appStore.set({ dialog: null });

// ---------------------------------------------------------------- add a code

export function NewCodeDialog({ parentId }: { parentId: number | null }) {
  const codes = useApp((s) => s.codes);
  const [name, setName] = useState('');
  const [parent, setParent] = useState<string>(parentId === null ? '' : String(parentId));
  const [color, setColor] = useState(() => nextColor(codes));
  const [definition, setDefinition] = useState('');
  const [theme, setTheme] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const parents = useMemo(
    () =>
      codes
        .filter((c) => c.status === 'active')
        .map((c) => ({ id: c.id, label: pathLabel(codes, c.id) }))
        .sort((a, b) => compareText(a.label, b.label)),
    [codes],
  );

  const submit = async () => {
    setError(null);
    if (!name.trim()) return setError(t('Give the code a name.'));
    setBusy(true);
    try {
      await addCode({ name, parentId: parent === '' ? null : Number(parent), color, definition, theme });
      close();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  return (
    <Shell title={t('Add a code')} id="new-code-title">
      {error ? <Notice>{error}</Notice> : null}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
        noValidate
      >
        <TextField value={name} onChange={setName} className="field" autoFocus>
          <Label>{t('Name')}</Label>
          <Input />
        </TextField>
        <div className="field">
          <label htmlFor="new-code-parent">{t('Place it under')}</label>
          <select id="new-code-parent" value={parent} onChange={(e) => setParent(e.target.value)}>
            <option value="">{t('Nothing: put it at the top level')}</option>
            {parents.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor="new-code-color">{t('Color')}</label>
          <select id="new-code-color" value={color} onChange={(e) => setColor(e.target.value)}>
            {PALETTE.map((p) => (
              <option key={p.hex} value={p.hex}>
                {p.name}
              </option>
            ))}
          </select>
          <p className="help">{t('You can change the color later. Every code also gets a short label in the margin, so codes never depend on color alone.')}</p>
        </div>
        <Checkbox isSelected={theme} onChange={setTheme} className="check">
          <div className="box" aria-hidden="true">
            <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
              <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
            </svg>
          </div>{t('This is a theme that groups other codes')}</Checkbox>
        <TextField value={definition} onChange={setDefinition} className="field">
          <Label>{t('Definition (you can add it later)')}</Label>
          <TextArea rows={3} />
        </TextField>
        <div className="actions">
          <Button type="submit" className="primary" isDisabled={busy}>{t('Add code')}</Button>
          <Button type="button" onPress={close}>{t('Cancel')}</Button>
        </div>
      </form>
    </Shell>
  );
}

// ---------------------------------------------------------------- merge

export function MergeDialog({ codeId }: { codeId: number }) {
  const codes = useApp((s) => s.codes);
  const from = codes.find((c) => c.id === codeId);
  const blocked = useMemo(() => descendantIds(codes, codeId), [codes, codeId]);
  const targets = useMemo(
    () =>
      codes
        .filter((c) => c.status === 'active' && c.id !== codeId && !blocked.has(c.id))
        .map((c) => ({ id: c.id, label: pathLabel(codes, c.id) }))
        .sort((a, b) => compareText(a.label, b.label)),
    [codes, codeId, blocked],
  );
  const [into, setInto] = useState<string>('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!from) return null;

  const submit = async () => {
    if (!into) return setError(t('Choose the code to merge into.'));
    setBusy(true);
    try {
      await mergeCodes(codeId, Number(into));
      close();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  return (
    <Shell title={t('Merge {name} into another code', { name: from.name })} id="merge-title">
      {error ? <Notice>{error}</Notice> : null}
      <p>{rich('Every coding with <strong>{name}</strong> ({codings}) moves to the code you choose, and its sub-codes move there too. {name} stays in the codebook marked as merged, so the history is kept. This cannot be undone from here.', { name: from.name, codings: tn(from.usage, { one: '{count} coding', other: '{count} codings' }), strong: (inside) => <strong>{inside}</strong> })}</p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="field">
          <label htmlFor="merge-into">{t('Merge into')}</label>
          <select id="merge-into" value={into} onChange={(e) => setInto(e.target.value)}>
            <option value="">{t('Choose a code')}</option>
            {targets.map((t) => (
              <option key={t.id} value={t.id}>
                {t.label}
              </option>
            ))}
          </select>
        </div>
        <div className="actions">
          <Button type="submit" className="primary" isDisabled={busy || targets.length === 0}>{t('Merge')}</Button>
          <Button type="button" onPress={close}>{t('Cancel')}</Button>
        </div>
        {targets.length === 0 ? <p className="muted">{t('There is no other active code to merge into.')}</p> : null}
      </form>
    </Shell>
  );
}

// ---------------------------------------------------------------- split

export function SplitDialog({ codeId }: { codeId: number }) {
  const codes = useApp((s) => s.codes);
  const data = useApp((s) => s.sourceData);
  const code = codes.find((c) => c.id === codeId);
  const mine = useMemo(() => (data ? data.codings.filter((c) => c.code_id === codeId) : []), [data, codeId]);
  const [name, setName] = useState('');
  const [definition, setDefinition] = useState('');
  const [chosen, setChosen] = useState<ReadonlySet<number>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!code) return null;

  const toggle = (id: number, on: boolean) =>
    setChosen((s) => {
      const n = new Set(s);
      if (on) n.add(id);
      else n.delete(id);
      return n;
    });

  const submit = async () => {
    setError(null);
    if (!name.trim()) return setError(t('Give the new code a name.'));
    if (chosen.size === 0) return setError(t('Choose at least one coding to move to the new code.'));
    setBusy(true);
    try {
      await splitCode(codeId, name, [...chosen], definition);
      close();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  return (
    <Shell title={t('Split {name}', { name: code.name })} id="split-title">
      {error ? <Notice>{error}</Notice> : null}
      <p>{rich('Make a new code next to <strong>{name}</strong> and move the codings you choose to it. The codings are listed from the source you have open ({name2}).', { name: code.name, name2: data?.name ?? t('none open'), strong: (inside) => <strong>{inside}</strong> })}</p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
        noValidate
      >
        <TextField value={name} onChange={setName} className="field" autoFocus>
          <Label>{t('Name of the new code')}</Label>
          <Input />
        </TextField>
        <TextField value={definition} onChange={setDefinition} className="field">
          <Label>{t('Definition of the new code (optional)')}</Label>
          <TextArea rows={2} />
        </TextField>
        <fieldset className="field">
          <legend>{t('Codings to move ({size} chosen)', { size: chosen.size })}</legend>
          {mine.length === 0 ? (
            <p className="muted">{t('This source has no codings with {name}. Open a source that has some, then try again.', { name: code.name })}</p>
          ) : (
            <ul className="check-list">
              {mine.slice(0, 200).map((c) => {
                const words = sliceScalar(data!.text, data!.index, c.start, Math.min(c.end, c.start + 120)).replace(/\s+/g, ' ');
                return (
                  <li key={c.id}>
                    <Checkbox isSelected={chosen.has(c.id)} onChange={(on) => toggle(c.id, on)} className="check">
                      <div className="box" aria-hidden="true">
                        <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                          <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                        </svg>
                      </div>
                      '{words}'
                    </Checkbox>
                  </li>
                );
              })}
            </ul>
          )}
        </fieldset>
        <div className="actions">
          <Button type="submit" className="primary" isDisabled={busy || mine.length === 0}>{t('Split')}</Button>
          <Button type="button" onPress={close}>{t('Cancel')}</Button>
        </div>
      </form>
    </Shell>
  );
}

// ---------------------------------------------------------------- codebook version

export function SnapshotDialog() {
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);
  const submit = async () => {
    try {
      await saveSnapshot(note);
      close();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };
  return (
    <Shell title={t('Save a version of the codebook')} id="snapshot-title">
      {error ? <Notice>{error}</Notice> : null}
      <p>{t('This freezes the codebook as it is now, with a number and your note, for example "after the pilot".')}</p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <TextField value={note} onChange={setNote} className="field" autoFocus>
          <Label>{t('Note')}</Label>
          <Input />
        </TextField>
        <div className="actions">
          <Button type="submit" className="primary">{t('Save version')}</Button>
          <Button type="button" onPress={close}>{t('Cancel')}</Button>
        </div>
      </form>
    </Shell>
  );
}

// ---------------------------------------------------------------- a clash in a mutually exclusive group

/**
 * The engine refused a coding because its code shares a group with a code already on those words. Nothing is replaced until
 * the person says so; Replace takes the other coding away and applies this one, and both steps are on the activity log.
 */
export function ReplaceExclusiveDialog({ pending }: { pending: PendingReplace }) {
  const code = useApp((s) => s.codes.find((c) => c.id === pending.codeId));
  const [busy, setBusy] = useState(false);
  const replace = async () => {
    setBusy(true);
    close();
    await applyCode(pending.codeId, pending);
    focusCenter();
  };
  return (
    <Shell title={t('Replace the other code?')} id="replace-title">
      <p data-testid="replace-message">{pending.message}</p>
      <p>{code ? t('Replace takes the other code off those words and applies {name} in its place. Each step is on the activity log, and Undo (Ctrl+Z) puts them back one at a time. Cancel leaves the words as they are.', { name: code.name }) : t('Replace takes the other code off those words and applies this code in its place. Each step is on the activity log, and Undo (Ctrl+Z) puts them back one at a time. Cancel leaves the words as they are.')}</p>
      <div className="actions">
        <Button className="primary" autoFocus isDisabled={busy} onPress={() => void replace()}>{t('Replace')}</Button>
        <Button
          onPress={() => {
            close();
            focusCenter();
          }}
        >{t('Cancel')}</Button>
      </div>
    </Shell>
  );
}

// ---------------------------------------------------------------- choose a counterexample

/** Picks a passage, coded with another code, that looks as if it belongs to this code and does not. */
export function CounterexampleDialog({ codeId }: { codeId: number }) {
  const code = useApp((s) => s.codes.find((c) => c.id === codeId));
  const [rows, setRows] = useState<SegmentRow[] | null>(null);
  const [taken, setTaken] = useState<ReadonlySet<number>>(new Set());
  const [query, setQuery] = useState('');
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    Promise.all([call('segments.list', { limit: 3000 }), call('code.counterexamples', { id: codeId })])
      .then(([all, already]) => {
        if (!live) return;
        setRows(all.segments);
        setTaken(new Set(already.map((s) => s.coding_id)));
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [codeId]);
  const q = query.trim().toLowerCase();
  const matches = useMemo(
    () =>
      (rows ?? []).filter(
        (s) => s.code_id !== codeId && !taken.has(s.coding_id) && (q === '' || `${s.text} ${s.source_name} ${s.code_name}`.toLowerCase().includes(q)),
      ),
    [rows, taken, codeId, q],
  );
  const shown = matches.slice(0, 40);
  const choose = async (s: SegmentRow) => {
    try {
      await setCounterexample(codeId, s.coding_id, true);
      close();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };
  return (
    <Shell title={t('Add a counterexample of {name}', { name: code?.name ?? t('the code') })} id="counter-title">
      <p>{t('A counterexample is a passage that looks as if it belongs to {name} but does not. Choose one of the passages already coded with another code.', { name: code?.name ?? t('this code') })}</p>
      {error ? <Notice>{error}</Notice> : null}
      <SearchField value={query} onChange={setQuery} className="field search" aria-label={t('Find a passage')}>
        <Label className="visually-hidden">{t('Find a passage')}</Label>
        <Input placeholder={t('Find a passage, a document, or a code')} autoFocus />
        <Button aria-label={t('Clear the search')}>×</Button>
      </SearchField>
      <div className="bk-counter-picker" data-testid="counter-picker">
        {rows === null ? (
          <p className="muted" role="status">{t('Loading…')}</p>
        ) : shown.length === 0 ? (
          <p className="muted">{matches.length === 0 && q === '' ? t('There is no other coded passage to choose from yet.') : t('No passage matches that search.')}</p>
        ) : (
          <ul className="bk-example-list">
            {shown.map((s) => (
              <li key={s.coding_id}>
                <blockquote className="bk-quote">{`“${s.text.trim().slice(0, 240)}”`}</blockquote>
                <p className="muted">{t('Coded {code_name} · {source_name}', { code_name: s.code_name, source_name: s.source_name })}
                </p>
                <Button onPress={() => void choose(s)} aria-label={t('Use this passage as a counterexample: {text}', { text: s.text.trim().slice(0, 60) })}>{t('Use as a counterexample')}</Button>
              </li>
            ))}
          </ul>
        )}
        {matches.length > shown.length ? <p className="muted">{t('Showing {count} of {count2}. Type to narrow the list.', { count: shown.length, count2: matches.length })}</p> : null}
      </div>
      <div className="actions">
        <Button onPress={close}>{t('Close')}</Button>
      </div>
    </Shell>
  );
}
