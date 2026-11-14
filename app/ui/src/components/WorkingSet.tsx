import { useEffect, useMemo, useState } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, TextField } from 'react-aria-components';
import { call } from '../bridge';
import { describeWorkingSet, followedNote, narrows, type WorkingSet } from '../lib/workingSet';
import { pathLabel } from '../lib/tree';
import { appStore, useApp } from '../state/app';
import { currentWorkingSet, resetWorkingSet, setWorkingSet } from '../state/workingSet';
import type { FinalSummary } from '../types';
import { compareText, num, t, tn } from '../i18n';
import { Notice } from './common';
import { Tick } from './Tick';

const close = () => appStore.set({ dialog: null });

interface Item {
  id: number;
  label: string;
}

/** One area of the working set: every one, or only the ones ticked. */
function Picker(props: { name: string; everyLabel: string; items: Item[]; all: boolean; chosen: ReadonlySet<number>; onAll: (all: boolean) => void; onChange: (ids: number[], on: boolean) => void; testId: string }) {
  const [search, setSearch] = useState('');
  const needle = search.trim().toLowerCase();
  const shown = props.items.filter((i) => needle === '' || i.label.toLowerCase().includes(needle));
  const ticked = props.items.filter((i) => props.chosen.has(i.id)).length;
  return (
    <fieldset className="ws-area" data-testid={props.testId}>
      <legend>{props.name}</legend>
      <Tick isSelected={props.all} onChange={props.onAll} testId={`${props.testId}-all`}>{props.everyLabel}</Tick>
      {props.all ? null : (
        <>
          {props.items.length > 12 ? (
            <TextField value={search} onChange={setSearch} className="field">
              <Label>{t('Find')}</Label>
              <Input autoComplete="off" data-testid={`${props.testId}-search`} />
            </TextField>
          ) : null}
          <div className="actions">
            <Button type="button" onPress={() => props.onChange(shown.map((i) => i.id), true)} data-testid={`${props.testId}-tick-shown`}>{t('Tick all shown')}</Button>
            <Button type="button" onPress={() => props.onChange(shown.map((i) => i.id), false)} data-testid={`${props.testId}-untick-shown`}>{t('Untick all shown')}</Button>
            <span className="muted" aria-live="polite">{t('{n} of {count} ticked', { n: ticked, count: props.items.length })}</span>
          </div>
          <ul className="ws-list" aria-label={props.name}>
            {shown.map((i) => (
              <li key={i.id}>
                <Tick isSelected={props.chosen.has(i.id)} onChange={(on) => props.onChange([i.id], on)} testId={`${props.testId}-item`}>{i.label}</Tick>
              </li>
            ))}
          </ul>
        </>
      )}
    </fieldset>
  );
}

/** Choose the documents, cases, and codes to work with. Applied to every page that can follow (decision 163). */
export function WorkingSetDialog() {
  const sources = useApp((s) => s.sources);
  const cases = useApp((s) => s.cases);
  const codes = useApp((s) => s.codes);
  const now = useMemo(() => currentWorkingSet(), []);
  const activeCodes = useMemo(() => codes.filter((c) => c.status === 'active'), [codes]);
  const [allSources, setAllSources] = useState(now.sourceIds === null);
  const [allCases, setAllCases] = useState(now.caseIds === null);
  const [allCodes, setAllCodes] = useState(now.codeIds === null);
  const [chosenSources, setChosenSources] = useState<Set<number>>(new Set(now.sourceIds ?? sources.map((s) => s.id)));
  const [chosenCases, setChosenCases] = useState<Set<number>>(new Set(now.caseIds ?? cases.map((c) => c.id)));
  const [chosenCodes, setChosenCodes] = useState<Set<number>>(new Set(now.codeIds ?? activeCodes.map((c) => c.id)));
  const [finalLayer, setFinalLayer] = useState(now.finalLayer === true);
  const [summary, setSummary] = useState<FinalSummary | null>(null);
  // How many units are decided, agreed, and still open, so the person knows what the final layer would hold.
  useEffect(() => {
    let live = true;
    call('adjudication.final_summary', {})
      .then((s) => live && setSummary(s))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  const sourceItems: Item[] = sources.map((s) => ({ id: s.id, label: s.name }));
  const caseItems: Item[] = cases.map((c) => ({ id: c.id, label: c.name }));
  const codeItems: Item[] = useMemo(() => activeCodes.map((c) => ({ id: c.id, label: pathLabel(codes, c.id) })).sort((a, b) => compareText(a.label, b.label)), [activeCodes, codes]);

  const change = (set: Set<number>, put: (s: Set<number>) => void) => (ids: number[], on: boolean) => {
    const next = new Set(set);
    for (const id of ids) (on ? next.add(id) : next.delete(id));
    put(next);
  };
  // Ticking a code ticks its sub-codes too (they are separate codes in the set); each can be unticked after.
  const changeCodes = (ids: number[], on: boolean) => {
    const next = new Set(chosenCodes);
    const kids = new Map<number, number[]>();
    for (const c of activeCodes) if (c.parent_id !== null) kids.set(c.parent_id, [...(kids.get(c.parent_id) ?? []), c.id]);
    const visit = (id: number) => {
      (on ? next.add(id) : next.delete(id));
      if (ids.length === 1) for (const k of kids.get(id) ?? []) visit(k);
    };
    for (const id of ids) visit(id);
    setChosenCodes(next);
  };

  const apply = () => {
    const w: WorkingSet = {
      sourceIds: allSources ? null : sources.filter((s) => chosenSources.has(s.id)).map((s) => s.id),
      caseIds: allCases ? null : cases.filter((c) => chosenCases.has(c.id)).map((c) => c.id),
      codeIds: allCodes ? null : activeCodes.filter((c) => chosenCodes.has(c.id)).map((c) => c.id),
      finalLayer,
    };
    setWorkingSet(w);
    close();
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog" aria-labelledby="ws-title">
          <Heading slot="title" id="ws-title" className="dialog-title">{t('Working set')}</Heading>
          <p className="muted">{t('Choose the documents, cases, and codes to work with. Every page that can follow does, until you show everything again. Nothing is hidden or removed from the project; the pages only ask about the ones you chose.')}</p>
          <p className="muted">{followedNote()}</p>
          <Picker name={t('Documents')} everyLabel={t('Every document')} items={sourceItems} all={allSources} chosen={chosenSources} onAll={setAllSources} onChange={change(chosenSources, setChosenSources)} testId="ws-documents" />
          <Picker name={t('Cases')} everyLabel={t('Every case')} items={caseItems} all={allCases} chosen={chosenCases} onAll={setAllCases} onChange={change(chosenCases, setChosenCases)} testId="ws-cases" />
          <Picker name={t('Codes')} everyLabel={t('Every code')} items={codeItems} all={allCodes} chosen={chosenCodes} onAll={setAllCodes} onChange={changeCodes} testId="ws-codes" />
          <fieldset className="ws-area" data-testid="ws-layer">
            <legend>{t('Codings')}</legend>
            <Tick isSelected={finalLayer} onChange={setFinalLayer} testId="ws-final-layer">{t('Use the final layer: what was decided in Team > Disagreements, and what the coders agreed on')}</Tick>
            <p className="muted" data-testid="ws-final-summary">
              {summary === null
                ? t('Looking at the decisions…')
                : t('Units of words: {decided} decided, {agreed} agreed, {open} still in disagreement (these have no final codes yet). Areas of pictures and stretches of recordings: {areasDecided} decided, {areasAgreed} agreed, {areasOpen} still in disagreement (these are left out until someone decides).', {
                    decided: tn(summary.decided, { one: '{count} unit', other: '{count} units' }),
                    agreed: tn(summary.agreed, { one: '{count} unit', other: '{count} units' }),
                    open: tn(summary.open, { one: '{count} unit', other: '{count} units' }),
                    areasDecided: num(summary.areas_decided),
                    areasAgreed: num(summary.areas_agreed),
                    areasOpen: num(summary.areas_open),
                  })}
            </p>
          </fieldset>
          <div className="actions">
            <Button type="button" onPress={close}>{t('Cancel')}</Button>
            <Button type="button" onPress={() => { resetWorkingSet(); close(); }} data-testid="ws-everything">{t('Show everything')}</Button>
            <Button type="button" className="primary" onPress={apply} data-testid="ws-apply">{t('Apply')}</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/** Under the ribbon on every page while the working set narrows anything, so the person is never working on a part without knowing. */
export function WorkingSetBar() {
  const stored = useApp((s) => s.workingSet);
  const sources = useApp((s) => s.sources);
  const cases = useApp((s) => s.cases);
  const codes = useApp((s) => s.codes);
  if (!narrows(stored)) return null;
  const w = currentWorkingSet();
  const parts = describeWorkingSet(w, { sources: sources.length, cases: cases.length, codes: codes.filter((c) => c.status === 'active').length });
  return (
    <section className="working-set-bar" aria-label={t('Working set')} data-testid="working-set-bar">
      <p>
        <strong>{t('Working with a part of the project:')}</strong> <span data-testid="working-set-parts">{parts.join('; ')}</span>
      </p>
      <div className="actions">
        <Button onPress={() => appStore.set({ dialog: { kind: 'working-set' } })} data-testid="working-set-change">{t('Change…')}</Button>
        <Button
          onPress={() => {
            resetWorkingSet();
            // The bar goes, and the button that was pressed with it: the keyboard goes to the page.
            window.setTimeout(() => document.getElementById('main-content')?.focus(), 0);
          }}
          data-testid="working-set-reset"
        >{t('Show everything')}</Button>
      </div>
      <p className="muted">{followedNote()}</p>
    </section>
  );
}


/**
 * A note on a page that holds something made before the working set last changed (the results of a query, the words of a count, a packet): it
 * keeps what it shows until it is run again, and says so. `since` is the working set when it was made; nothing is said when it has not changed.
 */
export function StaleSet(props: { since: WorkingSet | null | undefined; what: 'results' | 'packet' }) {
  const changed = useApp((s) => props.since != null && s.workingSet !== props.since);
  if (!changed) return null;
  return (
    <Notice kind="warning">
      <span data-testid="stale-set">
        {props.what === 'packet'
          ? t('The working set has changed since this packet was built. Build it again to make it for the current set.')
          : t('The working set has changed since these results were made. Run it again to see them for the current set.')}
      </span>
    </Notice>
  );
}
