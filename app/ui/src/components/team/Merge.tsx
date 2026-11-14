import { useState } from 'react';
import { Button, Input, Label, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { addsNothing, mergeSentences } from '../../lib/merge';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../../pickFile';
import { announce } from '../../state/announce';
import { refreshEverything } from '../../state/actions';
import { useApp } from '../../state/app';
import type { MergeChoices, MergeReport } from '../../types';
import { Notice } from '../common';
import { Tick } from '../Tick';
import { t, tn } from '../../i18n';

/**
 * Merge: bring the work of coders who worked apart on copies of this project back into it. The documents the copy has and this project
 * lacks are added; the codings, areas, and memos of documents both have are combined, each kept with the coder who made it. Nothing
 * here is changed or removed, a backup is saved first, and the copy is only read.
 */
export function MergeTab() {
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [path, setPath] = useState('');
  const [report, setReport] = useState<MergeReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // A copy that is encrypted needs its passphrase; the field appears when the engine says so, and is not kept.
  const [askPass, setAskPass] = useState(false);
  const [pass, setPass] = useState('');
  // The differences settled in favour of the copy: the position of a value in the list of values that differ, and the place of a code in the codebook.
  const [takeValues, setTakeValues] = useState<ReadonlySet<number>>(new Set());
  const [takeDefs, setTakeDefs] = useState<ReadonlySet<string>>(new Set());

  const target = cleanTypedPath(path);
  const choices = (r: MergeReport | null): MergeChoices => ({
    values_from_copy: (r?.values_in_conflict ?? []).filter((_, i) => takeValues.has(i)).map((c) => ({ case: c.case, variable: c.variable })),
    definitions_from_copy: (r?.codes_in_conflict ?? []).filter((c) => takeDefs.has(c.path)).map((c) => c.path),
  });
  const chosenCount = takeValues.size + takeDefs.size;
  const toggle = <T,>(set: ReadonlySet<T>, put: (s: ReadonlySet<T>) => void, key: T, on: boolean) => {
    const next = new Set(set);
    if (on) next.add(key);
    else next.delete(key);
    put(next);
  };

  /** Shows an error; one about the passphrase of the copy also opens the field for it. */
  const fail = (e: unknown) => {
    const err = toApiError(e);
    if (err.code === 'needs_passphrase' || err.code === 'wrong_passphrase') {
      setAskPass(true);
      setError(err.code === 'wrong_passphrase' ? t('That passphrase does not open the copy. Check it and try again.') : t('The copy is encrypted. Type its passphrase below, then check it again.'));
    } else {
      setError(err.message);
    }
  };

  const check = async (typed?: string) => {
    setError(null);
    setReport(null);
    setTakeValues(new Set());
    setTakeDefs(new Set());
    const chosen = cleanTypedPath(typed ?? path);
    if (chosen === '') return setError(t('Type the path of the copy to merge, for example C:\\Work\\Ben copy.pqda'));
    setBusy(true);
    try {
      const r = await call('merge.preview', { path: chosen, ...(pass !== '' ? { passphrase: pass } : {}) });
      setReport(r);
      announce(addsNothing(r) ? t('Checked the copy. It holds nothing this project does not have.') : t('Checked the copy. Nothing has been merged yet.'));
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const merge = async () => {
    setError(null);
    setBusy(true);
    try {
      const r = await call('merge.run', { path: target, ...(pass !== '' ? { passphrase: pass } : {}), ...choices(report) });
      setReport(r);
      await refreshEverything();
      announce(t('Merged the copy. A backup of this project was saved first.'));
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const browse = async () => {
    const picked = await browsePath('open-project');
    if (picked) {
      setPath(picked);
      void check(picked);
    }
  };

  return (
    <section className="ac-card" aria-labelledby="h-team-merge" data-testid="merge">
      <h3 id="h-team-merge" className="bk-h3">{t('Merge a copy of this project')}</h3>
      <p className="help">{t('Coders who worked apart, each on a copy of this project, can bring their work back here. The documents the copy has and this project lacks are added. The codings, areas of pictures, and memos of documents both have are combined, each kept with the coder who made it. Nothing here is changed or removed, a backup of this project is saved first, and the copy is only read.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      {readOnly ? <Notice kind="warning">{t('This project is read-only, so nothing can be merged into it.')}</Notice> : null}
      <TextField value={path} onChange={(v) => { setPath(v); setReport(null); }} className="field">
        <Label>{t('The copy to merge')}</Label>
        <Input placeholder={t('C:\\Work\\Ben copy.pqda')} data-testid="merge-path" />
        <Text slot="description">{nativeDialogsAvailable ? t('Type the full path of the copy, or use Browse.') : t('Type the full path of the copy.')}</Text>
      </TextField>
      {askPass ? (
        <TextField value={pass} onChange={(v) => { setPass(v); setReport(null); }} className="field" type="password">
          <Label>{t('Passphrase of the copy')}</Label>
          <Input autoComplete="off" spellCheck={false} data-testid="merge-passphrase" />
        </TextField>
      ) : null}
      <div className="actions">
        <Button onPress={() => void check()} isDisabled={busy} data-testid="merge-check">{busy && !report ? t('Looking…') : t('Check what would be merged')}</Button>
        {nativeDialogsAvailable ? <Button onPress={() => void browse()} isDisabled={busy}>{t('Browse…')}</Button> : null}
      </div>

      {report ? (
        <div data-testid="merge-result">
          <h4 className="bk-h3">{report.applied ? t('What the merge did') : t('What a merge would do')}</h4>
          <ul data-testid="merge-sentences">
            {mergeSentences(report).map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
          {report.codes_in_conflict.length > 0 ? (
            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="merge-conflicts">
                <caption>{report.applied ? t('Codes worded differently in the copy; this project\'s wording was kept') : t('Codes worded differently in the copy; this project\'s wording is kept unless you choose the copy\'s')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Code')}</th>
                    <th scope="col">{t('This project')}</th>
                    <th scope="col">{t('The copy')}</th>
                    {report.applied ? null : <th scope="col">{t('Which wording')}</th>}
                  </tr>
                </thead>
                <tbody>
                  {report.codes_in_conflict.map((c, i) => (
                    <tr key={c.path}>
                      <th scope="row">{c.path}</th>
                      <td>{c.ours || t('(no definition)')}</td>
                      <td>{c.theirs}</td>
                      {report.applied ? null : (
                        <td>
                          <Tick isSelected={takeDefs.has(c.path)} onChange={(on) => toggle(takeDefs, setTakeDefs, c.path, on)} testId={`merge-take-def-${i}`}>{t('Use the copy\'s')}</Tick>
                        </td>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
          {report.values_in_conflict.length > 0 ? (
            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="merge-value-conflicts">
                <caption>{report.applied ? t('Values of cases that differ in the copy; this project\'s were kept') : t('Values of cases that differ in the copy; this project\'s are kept unless you choose the copy\'s')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Case')}</th>
                    <th scope="col">{t('Variable')}</th>
                    <th scope="col">{t('This project')}</th>
                    <th scope="col">{t('The copy')}</th>
                    {report.applied ? null : <th scope="col">{t('Which value')}</th>}
                  </tr>
                </thead>
                <tbody>
                  {report.values_in_conflict.map((c, i) => (
                    <tr key={`${c.case}\u0000${c.variable}`}>
                      <th scope="row">{c.case}</th>
                      <td>{c.variable}</td>
                      <td>{c.ours}</td>
                      <td>{c.theirs}</td>
                      {report.applied ? null : (
                        <td>
                          <Tick isSelected={takeValues.has(i)} onChange={(on) => toggle(takeValues, setTakeValues, i, on)} testId={`merge-take-value-${i}`}>{t('Use the copy\'s')}</Tick>
                        </td>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
          {!report.applied && report.values_in_conflict.length + report.codes_in_conflict.length > 0 ? (
            <div className="actions">
              <Button
                onPress={() => {
                  setTakeValues(new Set(report.values_in_conflict.map((_, i) => i)));
                  setTakeDefs(new Set(report.codes_in_conflict.map((c) => c.path)));
                }}
                data-testid="merge-take-all"
              >{t('Use the copy\'s for all of them')}</Button>
              <Button onPress={() => { setTakeValues(new Set()); setTakeDefs(new Set()); }} data-testid="merge-take-none">{t('Keep this project\'s for all of them')}</Button>
              <span className="muted" data-testid="merge-chosen" aria-live="polite">
                {chosenCount === 0 ? t('Nothing is taken from the copy where it differs.') : tn(chosenCount, { one: '{count} difference will take the copy\'s when you merge.', other: '{count} differences will take the copy\'s when you merge.' })}
              </span>
            </div>
          ) : null}
          {report.applied ? (
            <p data-testid="merge-backup">{t('A backup of this project was saved first: {path}', { path: report.backup ?? '' })}</p>
          ) : (
            <div className="actions">
              <Button className="primary" onPress={() => void merge()} isDisabled={busy || readOnly || (addsNothing(report) && chosenCount === 0)} data-testid="merge-now">{busy ? t('Merging…') : t('Merge it now')}</Button>
              {addsNothing(report) && chosenCount === 0 ? <span className="muted">{t('There is nothing to merge.')}</span> : null}
            </div>
          )}
        </div>
      ) : null}
    </section>
  );
}
