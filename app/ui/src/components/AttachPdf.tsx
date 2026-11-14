import { useState, type FormEvent } from 'react';
import { Button } from 'react-aria-components';
import { attachPdf } from '../state/actions';
import { Notice } from './common';
import { announce } from '../state/announce';
import { browseManyFiles, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { t } from '../i18n';

/**
 * Shown on a document that was read from a PDF but keeps no copy of the file (a PDF imported before the file was kept, or one whose
 * pages could not be drawn at the time). Choosing the PDF again keeps it with the document, so its pages can be shown and areas of
 * them coded. The words and every coding of them stay as they are.
 */
export function AttachPdf(props: { sourceId: number }) {
  const [path, setPath] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const attach = async (e?: FormEvent) => {
    e?.preventDefault();
    const typed = cleanTypedPath(path);
    if (typed === '') {
      setError(t('Type the full path of the PDF, for example C:\\Documents\\report.pdf'));
      return;
    }
    setBusy(true);
    const ok = await attachPdf(props.sourceId, typed);
    setBusy(false);
    if (ok) {
      setError(null);
      setPath('');
    } else {
      setError(t('That PDF could not be kept with this document. Check that it is the file the words came from, and that the path is right.'));
      announce(t('That PDF could not be kept with this document.'), 'assertive');
    }
  };

  const choose = async () => {
    const files = await browseManyFiles('documents');
    if (files[0]) setPath(files[0]);
  };

  return (
    <section className="bk-media-link attach-pdf" aria-labelledby="attach-pdf-h" data-testid="attach-pdf">
      <h3 id="attach-pdf-h" className="bk-h3">{t('The pages of this PDF are not kept')}</h3>
      <p className="muted">{t('This document was read from a PDF, but the file itself is not kept in the project, so an area of a page cannot be coded yet. Choose the PDF again to keep it here. The words and their codings stay as they are.')}</p>
      <form onSubmit={attach} noValidate>
        <div className="field">
          <label htmlFor="attach-pdf-path">{t('Full path of the PDF')}</label>
          <input id="attach-pdf-path" type="text" value={path} onChange={(e) => setPath(e.target.value)} autoComplete="off" spellCheck={false} placeholder={t('C:\\Documents\\report.pdf')} />
        </div>
        <div className="actions">
          <Button type="submit" className="primary" isDisabled={busy} data-testid="attach-pdf-go">{t('Keep this PDF with the document')}</Button>
          {nativeDialogsAvailable ? <Button onPress={() => void choose()} isDisabled={busy}>{t('Choose the PDF…')}</Button> : null}
        </div>
      </form>
      {error ? <Notice>{error}</Notice> : null}
    </section>
  );
}
