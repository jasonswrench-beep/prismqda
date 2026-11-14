import { useEffect, useMemo, useState } from 'react';
import { Button, Dialog, Heading, Modal, ModalOverlay } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { diffCodebooks, summarizeDiff, type CodebookDiff, type SnapCode } from '../lib/codebookDiff';
import { formatWhen } from '../lib/eventWording';
import { appStore, useApp } from '../state/app';
import type { CodebookVersion } from '../types';
import { Notice } from './common';
import { rich, t } from '../i18n';

const TODAY = 'today';

/**
 * Compare: what changed between two states of the codebook. Either side is a version that was frozen with Save version,
 * or the codebook as it is now. Codes are matched by identity, so a code that was renamed or moved is one change, and the
 * description in effect then is shown beside the one in effect now.
 */
export function CodebookCompareDialog() {
  const codes = useApp((s) => s.codes);
  const [versions, setVersions] = useState<CodebookVersion[] | null>(null);
  const [from, setFrom] = useState<string>('');
  const [to, setTo] = useState<string>(TODAY);
  const [diff, setDiff] = useState<CodebookDiff | null>(null);
  const [error, setError] = useState<string | null>(null);
  const close = () => appStore.set({ dialog: null });

  const today = useMemo<SnapCode[]>(
    () =>
      codes.map((c) => ({
        id: c.id,
        name: c.name,
        parent_id: c.parent_id,
        kind: c.kind,
        color: c.color,
        definition: c.definition,
        inclusion_criteria: c.inclusion_criteria,
        exclusion_criteria: c.exclusion_criteria,
        status: c.status,
      })),
    [codes],
  );

  useEffect(() => {
    let live = true;
    call('codebook.versions')
      .then((v) => {
        if (!live) return;
        setVersions(v);
        if (v.length > 0) setFrom(String(v[v.length - 1].version));
      })
      .catch((e) => live && (setVersions([]), setError(toApiError(e).message)));
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    if (!versions || versions.length === 0 || from === '') return;
    let live = true;
    const load = async (which: string): Promise<SnapCode[]> => (which === TODAY ? today : (await call('codebook.version', { version: Number(which) })).codes);
    Promise.all([load(from), load(to)])
      .then(([a, b]) => {
        if (!live) return;
        setError(null);
        setDiff(diffCodebooks(a, b));
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [versions, from, to, today]);

  const label = (v: CodebookVersion) => (v.note ? t('Version {version}: {note} ({when})', { version: v.version, note: v.note, when: formatWhen(v.created_at) }) : t('Version {version} ({when})', { version: v.version, when: formatWhen(v.created_at) }));
  const nameOf = (id: number | null) => (id === null ? '' : (today.find((c) => c.id === id)?.name ?? ''));

  return (
    <ModalOverlay isOpen isDismissable onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal bk-wide-modal">
        <Dialog className="dialog" aria-labelledby="compare-title">
          <Heading slot="title" id="compare-title" className="dialog-title">{t('Compare codebook versions')}</Heading>
          {error ? <Notice>{error}</Notice> : null}
          {versions === null ? (
            <p role="status">{t('Loading…')}</p>
          ) : versions.length === 0 ? (
            <>
              <p className="help">{t('No version of the codebook has been saved yet, so there is nothing to compare with. Save version freezes the codebook as it is now; change it later and compare the two here.')}</p>
              <div className="actions">
                <Button className="primary" onPress={() => appStore.set({ dialog: { kind: 'snapshot' } })}>{t('Save version…')}</Button>
                <Button onPress={close}>{t('Close')}</Button>
              </div>
            </>
          ) : (
            <>
              <div className="bk-compare-pick">
                <div className="field">
                  <label htmlFor="cmp-from">{t('Earlier')}</label>
                  <select id="cmp-from" value={from} onChange={(e) => setFrom(e.target.value)}>
                    {versions.map((v) => (
                      <option key={v.version} value={String(v.version)}>
                        {label(v)}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label htmlFor="cmp-to">{t('Later')}</label>
                  <select id="cmp-to" value={to} onChange={(e) => setTo(e.target.value)}>
                    <option value={TODAY}>{t('The codebook today')}</option>
                    {versions.map((v) => (
                      <option key={v.version} value={String(v.version)}>
                        {label(v)}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {diff ? (
                <div className="bk-compare-result" data-testid="compare-result">
                  <p role="status" data-testid="compare-summary">
                    <strong>{summarizeDiff(diff)}</strong>
                  </p>
                  {diff.added.length > 0 ? (
                    <section aria-labelledby="cmp-added">
                      <h3 id="cmp-added" className="bk-h3">{t('Added')}</h3>
                      <ul>
                        {diff.added.map((c) => (
                          <li key={c.id}>
                            <strong>{c.name}</strong>
                            {c.parent_id !== null ? <span className="muted"> {t('under {name}', { name: nameOf(c.parent_id) })}</span> : null}
                            {c.definition.trim() ? <span className="muted">: {c.definition}</span> : null}
                          </li>
                        ))}
                      </ul>
                    </section>
                  ) : null}
                  {diff.removed.length > 0 ? (
                    <section aria-labelledby="cmp-removed">
                      <h3 id="cmp-removed" className="bk-h3">{t('Removed')}</h3>
                      <ul>
                        {diff.removed.map((c) => (
                          <li key={c.id}>
                            <strong>{c.name}</strong>
                          </li>
                        ))}
                      </ul>
                    </section>
                  ) : null}
                  {diff.changed.length > 0 ? (
                    <section aria-labelledby="cmp-changed">
                      <h3 id="cmp-changed" className="bk-h3">{t('Changed')}</h3>
                      <ul className="bk-changes">
                        {diff.changed.map((c) => (
                          <li key={c.id}>
                            <strong>{c.name}</strong>
                            <dl className="bk-facts">
                              {c.changes.map((f) => (
                                <div key={f.field}>
                                  <dt>{f.label}</dt>
                                  <dd>
                                    {rich('<span>{from}</span><span2> → </span2><span3> became </span3><span4>{to}</span4>', { from: f.from.trim() === '' ? t('(empty)') : f.from, to: f.to.trim() === '' ? t('(empty)') : f.to, span: (inside) => <span className="bk-was">{inside}</span>, span2: (inside) => <span aria-hidden="true">{inside}</span>, span3: (inside) => <span className="visually-hidden">{inside}</span>, span4: (inside) => <span className="bk-now">{inside}</span> })}
                                  </dd>
                                </div>
                              ))}
                            </dl>
                          </li>
                        ))}
                      </ul>
                    </section>
                  ) : null}
                </div>
              ) : (
                <p role="status">{t('Comparing…')}</p>
              )}
              <div className="actions">
                <Button onPress={close}>{t('Close')}</Button>
              </div>
            </>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
