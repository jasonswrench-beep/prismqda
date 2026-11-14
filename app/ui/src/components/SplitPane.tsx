// Split view: a second document, read-only, beside the one being read. Coding is done in the document on the left; this one is for
// reading, comparing, and finding a passage to match. Its coded words are marked as they are in the reader.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { abbreviations, patternFor } from '../lib/labels';
import { splitChoices, splitProblem } from '../lib/split';
import { OffsetIndex } from '../offsets';
import { setSplit, swapSplit, toggleSplit } from '../state/actions';
import { isBigSource, useApp } from '../state/app';
import { TextSurface, type CodeVisual, type SurfaceHandle } from '../surface/TextSurface';
import type { CodingRow, FormatRun, UnitRow } from '../types';
import { Notice, SOURCE_TYPE_LABELS, plural } from './common';
import { t, tn } from '../i18n';

interface Loaded {
  id: number;
  text: string;
  index: OffsetIndex;
  runs: FormatRun[];
  units: UnitRow[];
  codings: CodingRow[];
}

const nothing = () => {};

export function SplitPane(props: { currentId: number }) {
  const splitId = useApp((s) => s.splitSourceId);
  const sources = useApp((s) => s.sources);
  const codes = useApp((s) => s.codes);
  const version = useApp((s) => s.dataVersion);
  const lineNumbers = useApp((s) => s.settings.lineNumbers);
  const source = sources.find((s) => s.id === splitId) ?? null;
  const big = source !== null && isBigSource(source);
  const problem = splitProblem(source, props.currentId, big);
  const [data, setData] = useState<Loaded | null>(null);
  const [error, setError] = useState<string | null>(null);
  const handle = useRef<SurfaceHandle>(null);

  // The second document is read again when it changes (an undo, a revision, a suggestion accepted) and when another one is chosen.
  const revision = source?.text_revision;
  const codingCount = source?.codings;
  useEffect(() => {
    if (source === null || problem !== null) {
      setData(null);
      return;
    }
    const id = source.id;
    let live = true;
    Promise.all([call('source.get', { id }), call('unit.list', { source_id: id }), call('coding.list', { source_id: id })])
      .then(([detail, units, codings]) => {
        if (!live) return;
        setData({ id, text: detail.text, index: new OffsetIndex(detail.text), runs: detail.formatting_runs ?? [], units, codings });
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // `source` is read through its id, its text revision, and its number of codings; those are what make it new.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [splitId, revision, codingCount, version, problem]);

  const labels = useMemo(() => abbreviations(codes), [codes]);
  const visuals = useMemo(() => {
    const m = new Map<number, CodeVisual>();
    for (const c of codes) m.set(c.id, { id: c.id, name: c.name, label: labels.get(c.id) ?? '?', color: c.color, pattern: patternFor(c.color) });
    return m;
  }, [codes, labels]);
  const choices = splitChoices(sources, props.currentId);
  const shown = data !== null && source !== null && data.id === source.id ? data : null;

  const noop = useCallback(nothing, []);
  return (
    <section className="bk-split-pane" aria-labelledby="h-split" data-testid="split-pane">
      <header className="bk-split-head">
        {/* The name is the picker's own text; the heading is for the text below, which it names. */}
        <h3 id="h-split" className="visually-hidden">
          {source ? source.name : t('Second document')}
        </h3>
        <select
          id="split-doc"
          aria-label={t('Document shown beside')}
          value={splitId === null || splitId === props.currentId ? '' : String(splitId)}
          onChange={(e) => setSplit(e.target.value === '' ? null : Number(e.target.value))}
        >
          <option value="">{t('Choose a document…')}</option>
          {choices.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
        <Button onPress={() => void swapSplit()} isDisabled={source === null || source.id === props.currentId} aria-label={t('Swap sides')} data-testid="split-swap">{t('Swap')}</Button>
        <Button onPress={toggleSplit} aria-label={t('Close split view')} data-testid="split-close">{t('Close')}</Button>
      </header>
      {problem ? (
        <Notice kind="warning">{problem}</Notice>
      ) : error ? (
        <Notice>{error}</Notice>
      ) : shown === null ? (
        <p role="status">{t('Loading {name}…', { name: source?.name ?? '' })}</p>
      ) : (
        <div className="document-view">
          <p id="split-description" className="visually-hidden">{t('Read-only second document. Move the caret with the arrow keys to read. To code, use the document on the left.')}</p>
          <TextSurface
            text={shown.text}
            index={shown.index}
            runs={shown.runs}
            units={shown.units}
            codings={shown.codings}
            codes={visuals}
            showUnits={false}
            lineNumbers={lineNumbers}
            selectedCodingId={null}
            labelId="h-split"
            descriptionId="split-description"
            testId="split-surface"
            handleRef={handle}
            onSelection={noop}
            onCommand={noop}
            onStripeClick={noop}
          />
          {source ? (
            <p className="muted bk-split-note" data-testid="split-facts">
              {t('{value}, {characters}, {codings}. Read-only here: code in the document on the left.', { value: SOURCE_TYPE_LABELS[source.source_type] ?? source.source_type, characters: tn(source.characters, { one: '{count} character', other: '{count} characters' }), codings: tn(shown.codings.length, { one: '{count} coding', other: '{count} codings' }) })}</p>
          ) : null}
        </div>
      )}
    </section>
  );
}
