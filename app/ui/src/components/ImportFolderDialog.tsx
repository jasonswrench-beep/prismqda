import { useRef, useState, type FormEvent } from 'react';
import { Button, Checkbox, Dialog, Heading, Input, Label, Modal, ModalOverlay, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { refreshSources } from '../state/actions';
import { announce } from '../state/announce';
import { appStore } from '../state/app';
import type { FolderImportResult } from '../types';
import { Notice, plural } from './common';
import { Tick } from './Tick';
import { t, tn } from '../i18n';
import { engineText } from '../i18n/engine';

/** Imports everything readable in a folder, one file at a time, and says what it did with each file. */
export function ImportFolderDialog() {
  const [path, setPath] = useState('');
  const [label, setLabel] = useState('');
  const [deep, setDeep] = useState(true);
  const [pictureWhen, setPictureWhen] = useState(false);
  const [pictureCamera, setPictureCamera] = useState(false);
  const [picturePlace, setPicturePlace] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<FolderImportResult | null>(null);
  const [busy, setBusy] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);

  const close = () => appStore.set({ dialog: null });

  const run = async (typed?: string) => {
    setError(null);
    const target = cleanTypedPath(typed ?? path);
    if (target === '') {
      const picked = await browsePath('import-folder');
      if (!picked) return setError(t('Type the path of the folder to import, for example C:\\Data\\Interviews'));
      setPath(picked);
      return run(picked);
    }
    setBusy(true);
    try {
      const r = await call('import.folder', {
        path: target,
        recursive: deep,
        ...(label.trim() ? { folder: label.trim() } : {}),
        ...(pictureWhen || pictureCamera || picturePlace ? { picture_variables: { when: pictureWhen, camera: pictureCamera, place: picturePlace } } : {}),
      });
      await refreshSources();
      setResult(r);
      announce(t('Imported {sources} from the folder. {count} could not be read.', { sources: tn(r.sources_created, { one: '{count} source', other: '{count} sources' }), count: r.failed.length }));
      setTimeout(() => headingRef.current?.focus(), 0);
    } catch (err) {
      setError(toApiError(err).message);
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
        <Dialog className="dialog" aria-labelledby="folder-title">
          <Heading slot="title" id="folder-title" className="dialog-title" tabIndex={-1} ref={headingRef}>
            {result ? t('Folder import finished') : t('Import a whole folder')}
          </Heading>
          {error ? <Notice>{error}</Notice> : null}

          {result ? (
            <div>
              <p role="status">{[
          t('Imported {sources} with {units}.', { sources: tn(result.sources_created, { one: '{count} source', other: '{count} sources' }), units: tn(result.units_created, { one: '{count} unit', other: '{count} units' }) }),
          result.duplicates.length > 0 ? t('{files} already in the project.', { files: tn(result.duplicates.length, { one: '{count} file', other: '{count} files' }) }) : '',
          result.skipped.length > 0 ? t('{files} skipped.', { files: tn(result.skipped.length, { one: '{count} file', other: '{count} files' }) }) : '',
          result.failed.length > 0 ? t('{files} could not be read.', { files: tn(result.failed.length, { one: '{count} file', other: '{count} files' }) }) : '',
        ].filter(Boolean).join(' ')}
              </p>
              {result.failed.length > 0 ? (
                <section aria-labelledby="folder-failed">
                  <h3 id="folder-failed">{t('Could not be read')}</h3>
                  <ul>
                    {result.failed.map((f) => (
                      <li key={f.path}>
                        <span className="mono">{f.path}</span>: {f.message}
                      </li>
                    ))}
                  </ul>
                </section>
              ) : null}
              {result.skipped.length > 0 ? (
                <section aria-labelledby="folder-skipped">
                  <h3 id="folder-skipped">{t('Skipped')}</h3>
                  <ul>
                    {result.skipped.map((f) => (
                      <li key={f.path}>
                        <span className="mono">{f.path}</span>: {engineText(f.reason)}
                      </li>
                    ))}
                  </ul>
                </section>
              ) : null}
              {result.duplicates.length > 0 ? (
                <section aria-labelledby="folder-dups">
                  <h3 id="folder-dups">{t('Already in the project')}</h3>
                  <ul>
                    {result.duplicates.map((f) => (
                      <li key={f.path} className="mono">
                        {f.path}
                      </li>
                    ))}
                  </ul>
                </section>
              ) : null}
              <div className="actions">
                <Button className="primary" onPress={close} autoFocus>{t('Done')}</Button>
              </div>
            </div>
          ) : (
            <form onSubmit={submit} noValidate>
              <p className="help">{t('Every Word, OpenDocument, RTF, PDF, HTML, Markdown, text, and caption file in the folder is imported as its own source. A file that cannot be read is reported and the rest still go in. Spreadsheets and SPSS files are skipped, because each needs its columns set up: import those one at a time.')}</p>
              <TextField value={path} onChange={setPath} className="field" autoFocus>
                <Label>{t('Folder to import')}</Label>
                <Input placeholder={t('C:\\Data\\Interviews')} />
                <Text slot="description">{nativeDialogsAvailable ? t('Type the full path of the folder, or use Browse.') : t('Type the full path of the folder.')}</Text>
              </TextField>
              <TextField value={label} onChange={setLabel} className="field">
                <Label>{t('Put the sources in a folder called')}</Label>
                <Input />
                <Text slot="description">{t('Subfolders become folders beneath it. Leave empty to file them at the top.')}</Text>
              </TextField>
              <Checkbox isSelected={deep} onChange={setDeep} className="check">
                <div className="box" aria-hidden="true">
            <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
              <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
            </svg>
          </div>{t('Include the folders inside it')}</Checkbox>
              <fieldset className="bk-picture-facts" data-testid="folder-picture-facts">
                <legend>{t('Photographs in the folder')}</legend>
                <p className="help">{t('A photograph carries the facts its camera wrote into it. Tick any to keep them as variables of a case named for each picture. Nothing is kept unless you tick it, and a picture that does not say a thing gets no variable for it.')}</p>
                <Tick isSelected={pictureWhen} onChange={setPictureWhen} testId="folder-picture-when">{t('When each was taken')}</Tick>
                <Tick isSelected={pictureCamera} onChange={setPictureCamera} testId="folder-picture-camera">{t('What each was taken with')}</Tick>
                <Tick isSelected={picturePlace} onChange={setPicturePlace} testId="folder-picture-place">{t('Where each was taken, as latitude and longitude. A precise place can show where a person lives, so keep it only if your study needs it and your ethics approval allows it.')}</Tick>
              </fieldset>
              <div className="actions">
                <Button type="submit" className="primary" isDisabled={busy}>
                  {busy ? t('Importing…') : t('Import the folder')}
                </Button>
                {nativeDialogsAvailable ? (
                  <Button
                    type="button"
                    aria-label={t('Browse for a folder to import')}
                    isDisabled={busy}
                    onPress={async () => {
                      const picked = await browsePath('import-folder');
                      if (picked) setPath(picked);
                    }}
                  >{t('Browse…')}</Button>
                ) : null}
                <Button type="button" onPress={close} isDisabled={busy}>{t('Cancel')}</Button>
              </div>
            </form>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
