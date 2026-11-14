// The review queue: every suggestion Auto Code has made waits here until a person accepts or rejects it. A suggestion can be
// corrected first (its words by whole words, or its code). One undo takes a whole decision back.

import { useEffect, useMemo, useRef, useState } from 'react';
import { Button, Checkbox, Radio, RadioGroup } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { batchLine, describeOutcome, moveEnd, moveStart, snippet, spanOf, windowOf, windowParts, type DecisionNote, type TextWindow } from '../../lib/autocode';
import { formatWhen } from '../../lib/eventWording';
import { adjustSuggestion, decideSuggestions, goAutoCode, openAt, setAutoBatch, undoLast } from '../../state/actions';
import { announce } from '../../state/announce';
import { useApp } from '../../state/app';
import type { BatchSummary, ProposalRow, ProposalStatus } from '../../types';
import { Notice, plural } from '../common';
import { codeChoices } from '../explore/shared';
import { Passage } from './Passage';
import { num, rich, t, tn } from '../../i18n';

const PAGE = 50;
const MAX_LOADED = 1000;

type Filter = ProposalStatus | 'all';

export function AutoCodeQueue(props: { batches: BatchSummary[] | null }) {
  const { batches } = props;
  const chosen = useApp((s) => s.autoCodeBatch);
  const version = useApp((s) => s.dataVersion);
  const codes = useApp((s) => s.codes);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [status, setStatus] = useState<Filter>('pending');
  const [rows, setRows] = useState<ProposalRow[] | null>(null);
  const [more, setMore] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [result, setResult] = useState<DecisionNote | null>(null);
  const [adjusting, setAdjusting] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const loaded = useRef(0);
  const focusAt = useRef<number | null>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  // A batch that is gone (taken back, or never there) means "every batch".
  const batch = chosen !== null ? (batches?.find((b) => b.batch === chosen) ?? null) : null;
  const effective = batch ? batch.batch : null;
  const counts = batch ?? (batches ?? []).reduce((a, b) => ({ pending: a.pending + b.pending, accepted: a.accepted + b.accepted, rejected: a.rejected + b.rejected }), { pending: 0, accepted: 0, rejected: 0 });
  const filters = useMemo(() => ({ ...(status !== 'all' ? { status } : {}), ...(effective ? { batch: effective } : {}) }), [status, effective]);
  const filterKey = JSON.stringify(filters);

  useEffect(() => {
    loaded.current = 0;
    setSelected(new Set());
    setAdjusting(null);
  }, [filterKey]);

  useEffect(() => {
    let live = true;
    const limit = Math.min(Math.max(loaded.current, PAGE), MAX_LOADED);
    call('proposal.list', { ...filters, limit: limit + 1, offset: 0 })
      .then((list) => {
        if (!live) return;
        const page = list.length > limit ? list.slice(0, limit) : list;
        loaded.current = page.length;
        setRows(page);
        setMore(list.length > limit);
        setError(null);
        setSelected((prev) => new Set([...prev].filter((id) => page.some((r) => r.id === id && r.status === 'pending'))));
        if (focusAt.current !== null) {
          const i = Math.min(focusAt.current, page.length - 1);
          focusAt.current = null;
          requestAnimationFrame(() => {
            const li = i >= 0 ? document.querySelectorAll<HTMLElement>('[data-ac-row]')[i] : null;
            (li?.querySelector<HTMLElement>('button') ?? headingRef.current)?.focus();
          });
        }
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // The list is read again when the filter, the run, or the project changes (a decision or an undo changes it).
  }, [filterKey, version, batches]);

  const showMore = async () => {
    if (rows === null) return;
    try {
      const list = await call('proposal.list', { ...filters, limit: PAGE + 1, offset: rows.length });
      loaded.current = rows.length + Math.min(list.length, PAGE);
      setRows([...rows, ...list.slice(0, PAGE)]);
      setMore(list.length > PAGE);
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };

  const decide = async (which: { ids?: number[]; batch?: string }, accept: boolean, at: number | null) => {
    setBusy(true);
    focusAt.current = at;
    const r = await decideSuggestions(which, accept);
    setBusy(false);
    if (!r) {
      focusAt.current = null;
      return;
    }
    const d = describeOutcome(r);
    setResult(d);
    setSelected(new Set());
    announce([d.text, ...d.problems].join(' '), r.failed.length > 0 ? 'assertive' : 'polite');
  };

  const undo = async () => {
    setResult(null);
    setBusy(true);
    await undoLast();
    setBusy(false);
  };

  const pendingShown = (rows ?? []).filter((r) => r.status === 'pending');
  const allSelected = pendingShown.length > 0 && pendingShown.every((r) => selected.has(r.id));
  const toggle = (id: number, on: boolean) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });

  if (batches !== null && batches.length === 0) {
    return (
      <div className="ac-queue" data-testid="autocode-queue">
        <p className="help" data-testid="ac-queue-empty">{t('Nothing has been suggested yet. Set up a run, press Find, and send what it finds here.')}</p>
        <Button onPress={() => goAutoCode('setup')}>{t('Go to Set up')}</Button>
      </div>
    );
  }

  return (
    <div className="ac-queue" data-testid="autocode-queue">
      <nav className="ac-runs" aria-label={t('Runs')}>
        <h3 className="bk-h3">{t('Runs')}</h3>
        <ul>
          <li>
            <Button className={`ac-run${effective === null ? ' current' : ''}`} aria-pressed={effective === null} onPress={() => setAutoBatch(null)} data-testid="ac-run-all">
              {rich('<span>Everything</span><span2>{batch}</span2>', { batch: batchLine(counts), span: (inside) => <span className="ac-run-name">{inside}</span>, span2: (inside) => <span className="ac-run-line">{inside}</span> })}
            </Button>
          </li>
          {(batches ?? []).map((b) => (
            <li key={b.batch}>
              <Button className={`ac-run${effective === b.batch ? ' current' : ''}`} aria-pressed={effective === b.batch} onPress={() => setAutoBatch(b.batch)} data-testid="ac-run">
                <span className="ac-run-name">{b.label}</span>
                <span className="ac-run-line">{batchLine(b)}</span>
                <span className="ac-run-when">{formatWhen(b.created_at)}</span>
              </Button>
            </li>
          ))}
        </ul>
      </nav>

      <div className="ac-main">
        <h3 className="bk-h3" tabIndex={-1} ref={headingRef} id="h-ac-queue">
          {batch ? batch.label : t('Everything that has been suggested')}
        </h3>
        <RadioGroup value={status} onChange={(v) => setStatus(v as Filter)} className="field ac-radios ac-inline" aria-label={t('Show suggestions that are')}>
          <Radio value="pending" className="react-aria-Radio">
            <span>{t('Waiting ({pending})', { pending: num(counts.pending) })}</span>
          </Radio>
          <Radio value="accepted" className="react-aria-Radio">
            <span>{t('Accepted ({accepted})', { accepted: num(counts.accepted) })}</span>
          </Radio>
          <Radio value="rejected" className="react-aria-Radio">
            <span>{t('Rejected ({rejected})', { rejected: num(counts.rejected) })}</span>
          </Radio>
          <Radio value="all" className="react-aria-Radio">
            <span>{t('All')}</span>
          </Radio>
        </RadioGroup>

        {result ? (
          <div className="ac-result" role="status" data-testid="ac-result">
            <p>{result.text}</p>
            {result.problems.length > 0 ? (
              <ul>
                {result.problems.map((p) => (
                  <li key={p}>{p}</li>
                ))}
              </ul>
            ) : null}
            <div className="actions">
              {result.canUndo ? (
                <Button onPress={() => void undo()} isDisabled={busy || readOnly} data-testid="ac-undo">{t('Undo')}</Button>
              ) : null}
              <Button onPress={() => setResult(null)}>{t('Dismiss')}</Button>
            </div>
          </div>
        ) : null}
        {error ? <Notice>{error}</Notice> : null}

        {status !== 'accepted' && status !== 'rejected' && counts.pending > 0 ? (
          <div className="ac-bar" role="group" aria-label={t('Decide several suggestions at once')}>
            <Checkbox
              isSelected={allSelected}
              onChange={(on) => setSelected(on ? new Set(pendingShown.map((r) => r.id)) : new Set())}
              className="check"
              isDisabled={pendingShown.length === 0}
              data-testid="ac-select-all"
            >
              <Box />{t('Select all {count} shown', { count: num(pendingShown.length) })}</Checkbox>
            <Button onPress={() => void decide({ ids: [...selected] }, true, 0)} isDisabled={busy || readOnly || selected.size === 0} data-testid="ac-accept-selected">{selected.size > 0 ? tn(selected.size, { one: 'Accept {count} selected suggestion', other: 'Accept {count} selected suggestions' }) : t('Accept selected')}
            </Button>
            <Button onPress={() => void decide({ ids: [...selected] }, false, 0)} isDisabled={busy || readOnly || selected.size === 0} data-testid="ac-reject-selected">{selected.size > 0 ? tn(selected.size, { one: 'Reject {count} selected suggestion', other: 'Reject {count} selected suggestions' }) : t('Reject selected')}
            </Button>
            {batch && batch.pending > 0 ? (
              <>
                <Button onPress={() => void decide({ batch: batch.batch }, true, 0)} isDisabled={busy || readOnly} data-testid="ac-accept-all">{t('Accept all {pending} waiting in this run', { pending: num(batch.pending) })}</Button>
                <Button onPress={() => void decide({ batch: batch.batch }, false, 0)} isDisabled={busy || readOnly} data-testid="ac-reject-all">{t('Reject all {pending} waiting in this run', { pending: num(batch.pending) })}</Button>
              </>
            ) : null}
          </div>
        ) : null}

        {rows === null ? (
          <p className="muted" role="status">{t('Loading the queue…')}</p>
        ) : rows.length === 0 ? (
          <p className="help" data-testid="ac-none-here">
            {status === 'pending' ? t('Nothing is waiting here.') : status === 'accepted' ? t('Nothing has been accepted here.') : status === 'rejected' ? t('Nothing has been rejected here.') : t('There are no suggestions here.')}
          </p>
        ) : (
          <>
            <p className="muted" data-testid="ac-shown">{more ? t('Showing {count} (more below).', { count: num(rows.length) }) : t('Showing {count}.', { count: num(rows.length) })}</p>
            <ol className="ac-list" aria-label={t('Suggestions')}>
              {rows.map((row, i) => (
                <li key={row.id} data-ac-row className={`ac-row ${row.status}`} data-testid="ac-row">
                  <SuggestionRow
                    row={row}
                    selected={selected.has(row.id)}
                    onSelect={(on) => toggle(row.id, on)}
                    busy={busy || readOnly}
                    onDecide={(accept) => void decide({ ids: [row.id] }, accept, i)}
                    adjusting={adjusting === row.id}
                    onAdjust={(on) => setAdjusting(on ? row.id : null)}
                    codeName={codes.find((c) => c.id === row.code_id)?.name ?? t('code {id}', { id: row.code_id })}
                    codeColor={codes.find((c) => c.id === row.code_id)?.color ?? '#808080'}
                  />
                </li>
              ))}
            </ol>
            {more ? (
              <Button onPress={() => void showMore()} data-testid="ac-more">{t('Show {PAGE} more', { PAGE })}</Button>
            ) : null}
          </>
        )}
      </div>
    </div>
  );
}

function Box() {
  return (
    <div className="box" aria-hidden="true">
      <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
        <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
      </svg>
    </div>
  );
}

function SuggestionRow(props: {
  row: ProposalRow;
  selected: boolean;
  onSelect: (on: boolean) => void;
  busy: boolean;
  onDecide: (accept: boolean) => void;
  adjusting: boolean;
  onAdjust: (on: boolean) => void;
  codeName: string;
  codeColor: string;
}) {
  const { row } = props;
  const pending = row.status === 'pending';
  const what = snippet(row.text);
  return (
    <>
      <div className="ac-row-head">
        {pending ? (
          <Checkbox isSelected={props.selected} onChange={props.onSelect} className="check" aria-label={t('Select the suggestion: {what}', { what })} data-testid="ac-select">
            <Box />
          </Checkbox>
        ) : null}
        <span className="ac-chip">
          <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: props.codeColor }} />
          {props.codeName}
        </span>
        {row.origin === 'ai' ? (
          <span className="ac-status ai" data-testid="ac-ai-badge">{t('AI suggestion')}</span>
        ) : null}
        {!pending ? <span className={`ac-status ${row.status}`}>{row.status === 'accepted' ? t('Accepted') : t('Rejected')}</span> : null}
        {pending && row.already_coded ? <span className="ac-status warn">{t('Already coded this way by the same coder')}</span> : null}
      </div>
      <Passage before={row.before} text={row.text} after={row.after} truncated={row.truncated} testId="ac-passage" />
      <p className="muted ac-meta">
        {t('{source_name} · characters {start} to {end} · {rationale}', { source_name: row.source_name, start: num(row.start + 1), end: num(row.end), rationale: row.rationale })}
      </p>
      {pending && !props.adjusting ? (
        <div className="actions ac-actions">
          <Button className="primary" onPress={() => props.onDecide(true)} isDisabled={props.busy} aria-label={t('Accept the suggestion: {what}', { what })} data-testid="ac-accept">{t('Accept')}</Button>
          <Button onPress={() => props.onDecide(false)} isDisabled={props.busy} aria-label={t('Reject the suggestion: {what}', { what })} data-testid="ac-reject">{t('Reject')}</Button>
          <Button id={`ac-adjust-btn-${row.id}`} onPress={() => props.onAdjust(true)} isDisabled={props.busy} aria-label={t('Adjust the suggestion: {what}', { what })} data-testid="ac-adjust">{t('Adjust')}</Button>
          <Button onPress={() => void openAt(row.source_id, row.start, row.end)} aria-label={t('Open {source_name} at the suggestion: {what}', { source_name: row.source_name, what })}>{t('Open at source')}</Button>
        </div>
      ) : null}
      {!pending ? (
        <div className="actions ac-actions">
          <Button onPress={() => void openAt(row.source_id, row.start, row.end)} aria-label={t('Open {source_name} at the suggestion: {what}', { source_name: row.source_name, what })}>{t('Open at source')}</Button>
        </div>
      ) : null}
      {props.adjusting ? <AdjustEditor row={row} onDone={() => props.onAdjust(false)} /> : null}
    </>
  );
}

function AdjustEditor(props: { row: ProposalRow; onDone: () => void }) {
  const { row } = props;
  const codes = useApp((s) => s.codes);
  const choices = useMemo(() => codeChoices(codes), [codes]);
  const [w, setW] = useState<TextWindow | null>(() => windowOf(row));
  const [codeId, setCodeId] = useState(row.code_id);
  const [saving, setSaving] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  // The editor takes the focus as it opens (its first button may have nothing to do yet), and gives it back to Adjust when it closes.
  useEffect(() => {
    box.current?.focus();
  }, []);

  const parts = w ? windowParts(w) : { before: row.before, words: row.text, after: row.after };
  const span = w ? spanOf(w) : { start: row.start, end: row.end };
  const changed = span.start !== row.start || span.end !== row.end || codeId !== row.code_id;
  const move = (next: TextWindow) => {
    setW(next);
    announce(t('The suggestion now reads: {snippet}', { snippet: snippet(windowParts(next).words, 200) }));
  };
  const close = () => {
    props.onDone();
    requestAnimationFrame(() => document.getElementById(`ac-adjust-btn-${row.id}`)?.focus());
  };
  const save = async () => {
    setSaving(true);
    const r = await adjustSuggestion(row.id, span.start, span.end, codeId);
    setSaving(false);
    if (r) {
      announce(t('The suggestion was changed.'));
      close();
    }
  };
  const button = (label: string, to: TextWindow | null) => (
    <Button onPress={() => to && move(to)} isDisabled={w === null || to === w || saving}>
      {label}
    </Button>
  );
  return (
    <div className="ac-adjust" role="group" aria-label={t('Adjust this suggestion')} tabIndex={-1} ref={box} data-testid="ac-adjust-editor">
      <Passage before={parts.before} text={parts.words} after={parts.after} testId="ac-adjust-passage" />
      {w === null ? (
        <p className="help">{t('This passage is long, so only its code can be changed here.')}</p>
      ) : (
        <div className="ac-moves">
          {button(t('Start one word earlier'), w ? moveStart(w, -1) : null)}
          {button(t('Start one word later'), w ? moveStart(w, 1) : null)}
          {button(t('End one word earlier'), w ? moveEnd(w, -1) : null)}
          {button(t('End one word later'), w ? moveEnd(w, 1) : null)}
        </div>
      )}
      {w !== null ? <p className="help">{t('The buttons move one word at a time inside the text shown, which stops at the line break.')}</p> : null}
      <div className="field">
        <label htmlFor={`ac-adj-code-${row.id}`}>{t('Code')}</label>
        <select id={`ac-adj-code-${row.id}`} value={codeId} onChange={(e) => setCodeId(Number(e.target.value))}>
          {choices.map((c) => (
            <option key={c.id} value={c.id}>
              {'  '.repeat(c.depth ?? 0)}
              {c.label}
            </option>
          ))}
        </select>
      </div>
      <div className="actions">
        <Button className="primary" onPress={() => void save()} isDisabled={!changed || saving} data-testid="ac-adjust-save">{t('Save changes')}</Button>
        <Button onPress={close}>{t('Cancel')}</Button>
      </div>
    </div>
  );
}
