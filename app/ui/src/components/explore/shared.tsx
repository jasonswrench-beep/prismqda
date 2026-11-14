// Pieces the Explore tools share: a picker for several things at once, the evidence behind a number, and name lookups.

import { useEffect, useMemo, useState } from 'react';
import { Button, Checkbox, CheckboxGroup, Dialog, DialogTrigger, Input, Label, Popover, SearchField } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { flattenTree, buildCodeTree } from '../../lib/tree';
import type { Names } from '../../lib/explore';
import { openAt } from '../../state/actions';
import { useApp } from '../../state/app';
import type { CodeRow, SegmentRow } from '../../types';
import { Notice, plural } from '../common';
import { SegmentQuote } from '../SegmentQuote';
import { num, t, tn } from '../../i18n';

export interface Choice {
  id: number;
  label: string;
  /** How deep in a tree the choice is, to show with indent. */
  depth?: number;
  color?: string;
}

/** The codes in the order of the codebook, with how deep each is, for a picker. Retired and merged codes are left out. */
/** "3 excerpts" or "3 assignments": the count of what a query returned, by the unit it was asked in. */
export function resultCount(n: number, unit: string): string {
  return unit === 'excerpts' ? tn(n, { one: '{count} excerpt', other: '{count} excerpts' }) : tn(n, { one: '{count} assignment', other: '{count} assignments' });
}

export function codeChoices(codes: readonly CodeRow[]): Choice[] {
  return flattenTree(buildCodeTree(codes, { showInactive: false })).map(({ node, depth }) => ({ id: node.code.id, label: node.code.name, depth, color: node.code.color }));
}

/** Names for ids, taken from what is open. */
export function useNames(): Names {
  const codes = useApp((s) => s.codes);
  const sources = useApp((s) => s.sources);
  const cases = useApp((s) => s.cases);
  const coders = useApp((s) => s.coders);
  return useMemo(
    () => ({
      code: (id: number) => codes.find((c) => c.id === id)?.name ?? t('code {id}', { id }),
      source: (id: number) => sources.find((s) => s.id === id)?.name ?? t('document {id}', { id }),
      case: (id: number) => cases.find((c) => c.id === id)?.name ?? t('case {id}', { id }),
      coder: (id: number) => coders.find((c) => c.id === id)?.name ?? t('coder {id}', { id }),
    }),
    [codes, sources, cases, coders],
  );
}

/**
 * A button that says how many things are chosen and opens a list of check boxes with a search box. Used wherever the
 * Explore tools ask for several codes, documents, cases, or coders.
 */
export function CheckPicker(props: {
  /** What is being chosen, for the button's name ("codes"). */
  noun: string;
  /** The accessible name of the button, which says what it is for ("Codes for the columns"). */
  label: string;
  choices: Choice[];
  value: number[];
  onChange: (ids: number[]) => void;
  /** What the button says when nothing is chosen. */
  none?: string;
  testId?: string;
}) {
  const [query, setQuery] = useState('');
  const q = query.trim().toLowerCase();
  const shown = props.choices.filter((c) => q === '' || c.label.toLowerCase().includes(q));
  const chosen = props.choices.filter((c) => props.value.includes(c.id));
  const summary = chosen.length === 0 ? (props.none ?? t('Choose {noun}', { noun: props.noun })) : chosen.length <= 2 ? chosen.map((c) => c.label).join(', ') : t('{count} {noun}', { count: num(chosen.length), noun: props.noun });
  return (
    <DialogTrigger>
      <Button className="ex-picker" aria-label={chosen.length === 0 ? t('{label}: none chosen', { label: props.label }) : t('{label}: {chosen}', { label: props.label, chosen: chosen.map((c) => c.label).join(', ') })} data-testid={props.testId}>
        <span className="ex-picker-text">{summary}</span>
        <span aria-hidden="true">▾</span>
      </Button>
      <Popover className="popover ex-picker-pop" placement="bottom start">
        <Dialog aria-label={props.label} className="ex-picker-dialog">
          <SearchField value={query} onChange={setQuery} className="field search" aria-label={t('Find in {noun}', { noun: props.noun })}>
            <Label className="visually-hidden">{t('Find in {noun}', { noun: props.noun })}</Label>
            <Input placeholder={t('Find in {noun}', { noun: props.noun })} />
          </SearchField>
          <div className="ex-picker-actions">
            <Button className="bk-link" onPress={() => props.onChange(props.choices.map((c) => c.id))}>{t('All')}</Button>
            <Button className="bk-link" onPress={() => props.onChange([])}>{t('None')}</Button>
          </div>
          <CheckboxGroup aria-label={props.label} value={props.value.map(String)} onChange={(v) => props.onChange(v.map(Number))} className="ex-picker-list">
            {shown.length === 0 ? <p className="muted">{t('Nothing matches that search.')}</p> : null}
            {shown.map((c) => (
              <Checkbox key={c.id} value={String(c.id)} className="check" style={{ paddingInlineStart: `${(c.depth ?? 0) * 1}rem` }}>
                <div className="box" aria-hidden="true">
                  <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                    <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                  </svg>
                </div>
                {c.color ? <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: c.color }} /> : null}
                {c.label}
              </Checkbox>
            ))}
          </CheckboxGroup>
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}

/** The codings behind a number: the words, the code, the document, and a way back to the place in the document. */
export function EvidenceList(props: { title: string; codingIds: number[]; onClose: () => void; testId?: string }) {
  const [rows, setRows] = useState<SegmentRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const key = props.codingIds.join(',');
  useEffect(() => {
    let live = true;
    setRows(null);
    setError(null);
    if (props.codingIds.length === 0) {
      setRows([]);
      return;
    }
    call('segments.list', { coding_ids: props.codingIds, limit: 200 })
      .then((r) => live && setRows(r.segments))
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // The list is read again when the set of codings changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return (
    <section className="ex-evidence" aria-labelledby="h-evidence" data-testid={props.testId ?? 'evidence'}>
      <div className="ex-evidence-head">
        <h3 id="h-evidence" className="bk-h3">
          {props.title}
        </h3>
        <Button className="bk-link" onPress={props.onClose}>{t('Close the evidence')}</Button>
      </div>
      {error ? <Notice>{error}</Notice> : null}
      {rows === null ? (
        <p className="muted" role="status">{t('Loading the evidence…')}</p>
      ) : rows.length === 0 ? (
        <p className="muted">{t('There is nothing behind this number.')}</p>
      ) : (
        <>
          <p className="muted" data-testid="evidence-count">
            {tn(props.codingIds.length, { one: '{count} coding', other: '{count} codings' })}
            {props.codingIds.length > rows.length ? t('; the first {count} are shown', { count: rows.length }) : ''}.
          </p>
          <ul className="bk-example-list">
            {rows.map((s) => (
              <li key={s.coding_id}>
                <SegmentQuote text={s.text} sourceId={s.source_id} region={s.region} page={s.page} max={400} />
                <p className="muted">
                  {s.code_name} · {s.source_name} · {s.coder_name}
                </p>
                <Button onPress={() => void openAt(s.source_id, s.start, s.end, s.coding_id)} aria-label={s.kind === 'image' ? t('Open {source_name} at this area, coded {code_name}', { source_name: s.source_name, code_name: s.code_name }) : s.kind === 'clip' ? t('Open {source_name} at this stretch, coded {code_name}', { source_name: s.source_name, code_name: s.code_name }) : t('Open {source_name} at these words, coded {code_name}', { source_name: s.source_name, code_name: s.code_name })}>{t('Open at source')}</Button>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
