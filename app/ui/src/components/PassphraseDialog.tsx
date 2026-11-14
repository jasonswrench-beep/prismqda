import { useState, type FormEvent } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, TextField } from 'react-aria-components';
import { toApiError } from '../bridge';
import { projectLabel } from '../lib/passphrase';
import { openProject } from '../state/actions';
import { appStore } from '../state/app';
import { Notice } from './common';
import { Tick } from './Tick';
import { t } from '../i18n';

/**
 * Asks for the passphrase of an encrypted project when it is opened (decision 157). A wrong passphrase is said so and asked again; the
 * passphrase is given to the engine once and is not kept anywhere by the window.
 */
export function PassphraseDialog(props: { path: string }) {
  const [pass, setPass] = useState('');
  const [shown, setShown] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const close = () => appStore.set({ dialog: null });

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (pass === '') {
      setError(t('Type the passphrase of the project.'));
      return;
    }
    setError(null);
    setBusy(true);
    try {
      await openProject(props.path, pass);
    } catch (err) {
      const e2 = toApiError(err);
      setBusy(false);
      setPass('');
      setError(e2.code === 'wrong_passphrase' ? t('That passphrase does not open this project. Check it and try again.') : e2.message);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && close()} className="overlay">
      <Modal className="modal">
        <Dialog className="dialog" aria-labelledby="pp-title" data-testid="passphrase-dialog">
          <Heading slot="title" id="pp-title" className="dialog-title">{t('This project is encrypted')}</Heading>
          <p className="muted">{t('Type the passphrase of {name} to open it.', { name: projectLabel(props.path) })}</p>
          {error ? <Notice>{error}</Notice> : null}
          <form onSubmit={(e) => void submit(e)} noValidate>
            <TextField value={pass} onChange={setPass} className="field" type={shown ? 'text' : 'password'} autoFocus>
              <Label>{t('Passphrase')}</Label>
              <Input autoComplete="off" spellCheck={false} data-testid="open-passphrase" />
            </TextField>
            <Tick isSelected={shown} onChange={setShown} testId="open-passphrase-show">{t('Show the passphrase')}</Tick>
            <div className="actions">
              <Button type="button" onPress={close}>{t('Cancel')}</Button>
              <Button type="submit" className="primary" isDisabled={busy} data-testid="open-passphrase-submit">{t('Open Project')}</Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
