// How codes occur together: a matrix of codes against codes, with the passages behind every cell.

import { useEffect, useMemo, useState } from 'react';
import { Button, Checkbox, Radio, RadioGroup, Label } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { intensity } from '../../lib/explore';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import type { RelationMode, RelationsResult, RelationsSpec } from '../../types';
import { Notice, plural } from '../common';
import { CheckPicker, EvidenceList, codeChoices } from './shared';
import { num, t, tn } from '../../i18n';
import { engineText } from '../../i18n/engine';

const MODES: { id: RelationMode; label: string; help: string }[] = [
  { id: 'overlap', get label() { return t('Overlapping words'); }, get help() { return t('Pairs of codings whose words overlap'); } },
  { id: 'document', get label() { return t('In the same document'); }, get help() { return t('Documents that have both codes'); } },
  { id: 'case', get label() { return t('In the same case'); }, get help() { return t('Cases that have both codes'); } },
];

export function CodeRelations() {
  const codes = useApp((s) => s.codes);
  const version = useApp((s) => s.dataVersion);
  const codings = useApp((s) => s.sources.reduce((n, x) => n + x.codings, 0));
  const [mode, setMode] = useState<RelationMode>('overlap');
  const [sameCoder, setSameCoder] = useState(true);
  const [descendants, setDescendants] = useState(() => appStore.get().settings.subcodes);
  const [codeIds, setCodeIds] = useState<number[]>([]);
  const [result, setResult] = useState<RelationsResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<{ title: string; ids: number[] } | null>(null);
  const choices = useMemo(() => codeChoices(codes), [codes]);

  const spec: RelationsSpec = useMemo(() => ({ code_ids: codeIds, include_descendants: descendants, mode, same_coder: sameCoder }), [codeIds, descendants, mode, sameCoder]);
  const key = JSON.stringify(spec);
  useEffect(() => {
    let live = true;
    setSelected(null);
    call('explore.relations', spec)
      .then((r) => {
        if (!live) return;
        setResult(r);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // The matrix is read again when the question or the coding changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version, codings]);

  const open = async (a: number, b: number, nameA: string, nameB: string) => {
    try {
      const r = await call('explore.relations_evidence', { spec, a, b });
      const title = a === b ? nameA : t('{a} with {b}', { a: nameA, b: nameB });
      setSelected({ title, ids: r.coding_ids });
      announce(t('{title}: {codings} shown below.', { title, codings: tn(r.coding_ids.length, { one: '{count} coding', other: '{count} codings' }) }));
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };

  const n = result?.codes.length ?? 0;
  const max = result ? Math.max(0, ...result.cells.flatMap((row, i) => row.filter((_, j) => j !== i))) : 0;
  const unit = mode === 'overlap' ? 'pair' : mode === 'document' ? 'document' : 'case';

  return (
    <div data-testid="relations-tool">
      <section className="ex-explore-controls" aria-label={t('Relations setup')}>
        <RadioGroup value={mode} onChange={(v) => setMode(v as RelationMode)} className="field ex-explore-radios" orientation="horizontal">
          <Label>{t('Count')}</Label>
          {MODES.map((m) => (
            <Radio key={m.id} value={m.id} className="react-aria-Radio">
              <span>{m.label}</span>
            </Radio>
          ))}
        </RadioGroup>
        <div className="field">
          <span className="ex-field-label">{t('Codes')}</span>
          <CheckPicker noun="codes" label={t('Codes to compare')} choices={choices} value={codeIds} onChange={setCodeIds} none={t('All codes in use')} testId="relations-codes" />
        </div>
        {mode === 'overlap' ? (
          <Checkbox isSelected={sameCoder} onChange={setSameCoder} className="check ex-explore-check">
            <div className="box" aria-hidden="true">
              <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
              </svg>
            </div>{t('Both codings by the same coder')}</Checkbox>
        ) : null}
        <Checkbox isSelected={descendants} onChange={setDescendants} className="check ex-explore-check">
          <div className="box" aria-hidden="true">
            <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
              <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
            </svg>
          </div>{t('A code also counts its sub-codes')}</Checkbox>
      </section>
      {error ? <Notice>{error}</Notice> : null}
      {result && n === 0 ? <p className="help">{t('There are no codings yet, so no code occurs with another.')}</p> : null}
      {result && n > 0 ? (
        <>
          <div className="ex-table-wrap">
            <table className="ex-matrix ex-relations" data-testid="relations-table">
              <caption>{t('How often each pair of codes occurs together, counting {unit}. The diagonal is how often the code occurs at all.', { unit: mode === 'overlap' ? t('pairs') : mode === 'document' ? t('documents') : t('cases') })}</caption>
              <thead>
                <tr>
                  <td />
                  {result.codes.map((c) => (
                    <th scope="col" key={c.code_id}>
                      {c.name}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {result.codes.map((rowCode, i) => (
                  <tr key={rowCode.code_id}>
                    <th scope="row">{rowCode.name}</th>
                    {result.codes.map((colCode, j) => {
                      const v = result.cells[i][j];
                      const diagonal = i === j;
                      return (
                        <td key={colCode.code_id} className={`ex-num ex-cell${diagonal ? ' ex-diagonal' : ''}`} style={!diagonal && v > 0 ? { ['--shade' as string]: String(intensity(v, max)) } : undefined}>
                          {v > 0 ? (
                            <Button
                              className="ex-cell-button"
                              aria-label={
                                diagonal
                                  ? t('{name} on its own: {count}. Show what is behind this number.', { name: rowCode.name, count: num(v) })
                                  : t('{name} with {name2}: {count}. Show what is behind this number.', { name: rowCode.name, name2: colCode.name, count: unit === 'pair' ? tn(v, { one: '{count} pair', other: '{count} pairs' }) : unit === 'document' ? tn(v, { one: '{count} document', other: '{count} documents' }) : tn(v, { one: '{count} case', other: '{count} cases' }) })
                              }
                              onPress={() => void open(rowCode.code_id, colCode.code_id, rowCode.name, colCode.name)}
                            >
                              {num(v)}
                            </Button>
                          ) : (
                            <span aria-label={t('{name} with {name2}: none', { name: rowCode.name, name2: colCode.name })}>0</span>
                          )}
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <ul className="ex-matrix-notes" data-testid="relations-notes">
            {result.notes.map((note) => (
              <li key={note}>{engineText(note)}</li>
            ))}
          </ul>
        </>
      ) : null}
      {selected ? <EvidenceList title={`${selected.title}: ${tn(selected.ids.length, { one: '{count} coding', other: '{count} codings' })}`} codingIds={selected.ids} onClose={() => setSelected(null)} testId="relations-evidence" /> : null}
    </div>
  );
}
