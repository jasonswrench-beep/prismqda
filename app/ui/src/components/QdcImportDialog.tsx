import { useState, type FormEvent } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { refreshCodes } from '../state/actions';
import { announce } from '../state/announce';
import { appStore } from '../state/app';
import type { QdcImportReport } from '../types';
import { tn, t } from '../i18n';
import { Notice } from './common';

const close = () => appStore.set({ dialog: null });

/**
 * Import a codebook: the codes of a REFI-QDA codebook file (.qdc), made by another program or by PrismQDA, added to this project's
 * codebook (decision 160). A code already at that place in the tree, by name, is left as it is, so reading a file twice adds nothing.
 */
export function QdcImportDialog() {
  const [file, setFile] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<QdcImportReport | null>(null);

  const run = async () => {
    setError(null);
    const source = cleanTypedPath(file);
    if (source === '') {
      setError(t('Type the path of the codebook file, or press Browse.'));
      return;
    }
    setBusy(true);
    try {
      const r = await call('import.qdc', { path: source });
      setDone(r.report);
      await refreshCodes();
      announce(tn(r.report.added, { one: '{count} code was added to the codebook.', other: '{count} codes were added to the codebook.' }));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run();
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog" aria-labelledby="qdc-title">
          <Heading slot="title" id="qdc-title" className="dialog-title">{t('Import a codebook')}</Heading>
          <p className="muted">{t('Add the codes of a codebook file in the REFI-QDA format (.qdc) to this project. The tree, the colours, and the definitions come with them. A code that is already at that place in the codebook, by name, is left as it is.')}</p>
          {error ? <Notice>{error}</Notice> : null}
          {done ? (
            <div data-testid="qdc-report">
              <Notice kind="info">
                {tn(done.added, { one: '{count} code was added', other: '{count} codes were added' })}
                {done.themes > 0 ? ' ' + tn(done.themes, { one: '({count} of them a theme).', other: '({count} of them themes).' }) : '.'}
                {done.already_there > 0 ? ' ' + tn(done.already_there, { one: '{count} was already in the codebook and was left as it is.', other: '{count} were already in the codebook and were left as they are.' }) : ''}
              </Notice>
              {done.left_out.length > 0 ? (
                <>
                  <h3 className="stats-h5">{t('What did not come in')}</h3>
                  <ul data-testid="qdc-left-out">
                    {done.left_out.map((line, i) => (
                      <li key={i}>{line}</li>
                    ))}
                  </ul>
                </>
              ) : null}
              <div className="actions">
                <Button type="button" className="primary" onPress={close} data-testid="qdc-close" autoFocus>{t('Close')}</Button>
              </div>
            </div>
          ) : (
            <form onSubmit={submit} noValidate>
              <TextField value={file} onChange={setFile} className="field" autoFocus>
                <Label>{t('Codebook file')}</Label>
                <div className="bk-location-row">
                  <Input placeholder={t('C:\\Users\\you\\Documents\\codebook.qdc')} data-testid="qdc-file" />
                  {nativeDialogsAvailable ? (
                    <Button
                      type="button"
                      aria-label={t('Browse for a codebook file')}
                      isDisabled={busy}
                      onPress={async () => {
                        const chosen = await browsePath('qdc-file');
                        if (chosen) setFile(chosen);
                      }}
                    >{t('Browse…')}</Button>
                  ) : null}
                </div>
                <Text slot="description">{t('Type the full path of a .qdc file.')}</Text>
              </TextField>
              <div className="actions">
                <Button type="button" onPress={close}>{t('Cancel')}</Button>
                <Button type="submit" className="primary" isDisabled={busy} data-testid="qdc-import">{busy ? t('Importing…') : t('Import')}</Button>
              </div>
            </form>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
