import { useState, type FormEvent } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { openProject } from '../state/actions';
import { announce } from '../state/announce';
import { appStore } from '../state/app';
import type { QdpxReport } from '../types';
import { Notice } from './common';
import { QdpxReportView } from './QdpxReport';
import { t } from '../i18n';

const close = () => appStore.set({ dialog: null });

/** Where the new project goes when none is typed: beside the QDPX file, named like it. */
function suggestedProject(file: string): string {
  return file.replace(/\.qdpx$/i, '') + '.pqda';
}

/**
 * Import QDPX: a whole project made in another qualitative analysis program (the REFI-QDA format), read into a new PrismQDA project.
 * The project that is open is not touched. The report says what came in and what the format or PrismQDA has no place for.
 */
export function QdpxImportDialog() {
  const [file, setFile] = useState('');
  const [target, setTarget] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<{ project_path: string; report: QdpxReport } | null>(null);

  const run = async () => {
    setError(null);
    const source = cleanTypedPath(file);
    if (source === '') {
      setError(t('Type the path of the QDPX file, or press Browse.'));
      return;
    }
    const where = cleanTypedPath(target) || suggestedProject(source);
    setBusy(true);
    try {
      const r = await call('import.qdpx', { path: source, project_path: where });
      setDone(r);
      announce(t('The project was made at {path}.', { path: r.project_path }));
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

  const openIt = async () => {
    if (!done) return;
    setBusy(true);
    try {
      await openProject(done.project_path);
      close();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog" aria-labelledby="qdpx-title">
          <Heading slot="title" id="qdpx-title" className="dialog-title">{t('Import QDPX')}</Heading>
          <p className="muted">{t('Bring in a whole project from another qualitative analysis program (the REFI-QDA format) as a new project. The project that is open is not changed. What the format holds that PrismQDA has no place for is listed afterwards.')}</p>
          {error ? <Notice>{error}</Notice> : null}
          {done ? (
            <>
              <Notice kind="info">{t('The project was made at {path}.', { path: done.project_path })}</Notice>
              <QdpxReportView report={done.report} direction="in" />
              <div className="actions">
                <Button type="button" className="primary" onPress={() => void openIt()} isDisabled={busy} data-testid="qdpx-open-new">{t('Open the new project')}</Button>
                <Button type="button" onPress={close}>{t('Close')}</Button>
              </div>
            </>
          ) : (
            <form onSubmit={submit} noValidate>
              <TextField value={file} onChange={setFile} className="field" autoFocus>
                <Label>{t('QDPX file')}</Label>
                <div className="bk-location-row">
                  <Input placeholder={t('C:\\Users\\you\\Documents\\study.qdpx')} data-testid="qdpx-file" />
                  {nativeDialogsAvailable ? (
                    <Button
                      type="button"
                      aria-label={t('Browse for a QDPX file')}
                      isDisabled={busy}
                      onPress={async () => {
                        const chosen = await browsePath('qdpx-file');
                        if (chosen) setFile(chosen);
                      }}
                    >{t('Browse…')}</Button>
                  ) : null}
                </div>
                <Text slot="description">{t('Type the full path of a .qdpx file.')}</Text>
              </TextField>
              <TextField value={target} onChange={setTarget} className="field">
                <Label>{t('Save the new project as')}</Label>
                <div className="bk-location-row">
                  <Input placeholder={file.trim() ? suggestedProject(cleanTypedPath(file)) : t('C:\\Users\\you\\Documents\\study.pqda')} data-testid="qdpx-target" />
                  {nativeDialogsAvailable ? (
                    <Button
                      type="button"
                      aria-label={t('Browse for where to save the new project')}
                      isDisabled={busy}
                      onPress={async () => {
                        const chosen = await browsePath('new-project', cleanTypedPath(file).split(/[\\/]/).pop()?.replace(/\.qdpx$/i, '') ?? '');
                        if (chosen) setTarget(chosen);
                      }}
                    >{t('Browse…')}</Button>
                  ) : null}
                </div>
                <Text slot="description">{t('Leave it empty to save it beside the QDPX file, with the same name. A file that is already there is not replaced.')}</Text>
              </TextField>
              <div className="actions">
                <Button type="button" onPress={close}>{t('Cancel')}</Button>
                <Button type="submit" className="primary" isDisabled={busy} data-testid="qdpx-import">{busy ? t('Importing…') : t('Import')}</Button>
              </div>
            </form>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
