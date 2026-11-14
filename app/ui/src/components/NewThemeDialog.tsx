import { useState, type FormEvent } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, TextArea, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { refreshCodes, goTo } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import { Notice } from './common';
import { t, tn } from '../i18n';

const close = () => appStore.set({ dialog: null });

/**
 * Add a theme (decision 167): a name and a sentence of what it says, and the codes to put under it. Opened from the Themes page (empty, or
 * with the codes that were ticked) or from the menu of the Codes pane.
 */
export function NewThemeDialog(props: { codeIds: number[] }) {
  const codes = useApp((s) => s.codes);
  const [name, setName] = useState('');
  const [definition, setDefinition] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const chosen = props.codeIds.map((id) => codes.find((c) => c.id === id)).filter((c) => c !== undefined);

  const save = async () => {
    setError(null);
    if (name.trim() === '') return setError(t('A theme needs a name.'));
    setBusy(true);
    let made: { id: number; name: string } | null = null;
    try {
      const theme = await call('code.add', { name: name.trim(), theme: true, definition });
      made = theme;
      for (const c of chosen) await call('code.move', { id: c.id, parent_id: theme.id });
      await refreshCodes();
      appStore.set((s) => ({ dialog: null, dataVersion: s.dataVersion + 1 }));
      announce(chosen.length > 0 ? t('Added the theme {name} with {codes}.', { name: theme.name, codes: tn(chosen.length, { one: '{count} code', other: '{count} codes' }) }) : t('Added the theme {name}.', { name: theme.name }));
      if (appStore.get().workspace !== 'themes') goTo('themes');
    } catch (e) {
      if (made) {
        // The theme exists; the codes that could not be moved stay where they were. Show it, and say so.
        await refreshCodes();
        appStore.set((s) => ({ dialog: null, dataVersion: s.dataVersion + 1 }));
        announce(t('The theme {name} was added, but not every code could be moved under it: {problem}', { name: made.name, problem: toApiError(e).message }), 'assertive');
        if (appStore.get().workspace !== 'themes') goTo('themes');
        return;
      }
      setError(toApiError(e).message);
      setBusy(false);
    }
  };
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void save();
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog" aria-labelledby="newtheme-title">
          <Heading slot="title" id="newtheme-title" className="dialog-title">{t('Add a theme')}</Heading>
          <p className="muted">{t('A theme groups other codes. It begins as a candidate; you review it against its passages and define it on the Themes page.')}</p>
          {error ? <Notice>{error}</Notice> : null}
          <form onSubmit={submit} noValidate>
            <TextField value={name} onChange={setName} className="field" autoFocus>
              <Label>{t('Name of the theme')}</Label>
              <Input data-testid="newtheme-name" autoComplete="off" />
            </TextField>
            <TextField value={definition} onChange={setDefinition} className="field">
              <Label>{t('What it says (optional, you can write it later)')}</Label>
              <TextArea rows={3} data-testid="newtheme-definition" />
            </TextField>
            {chosen.length > 0 ? (
              <div data-testid="newtheme-codes">
                <p className="muted">{tn(chosen.length, { one: 'The {count} code that will go under it:', other: 'The {count} codes that will go under it:' })}</p>
                <ul>
                  {chosen.map((c) => (
                    <li key={c.id}>{c.name}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            <div className="actions">
              <Button type="button" onPress={close}>{t('Cancel')}</Button>
              <Button type="submit" className="primary" isDisabled={busy} data-testid="newtheme-save">{busy ? t('Adding…') : t('Add the theme')}</Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
