import { useState } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, Text, TextField } from 'react-aria-components';
import { passphraseHint } from '../lib/passphrase';
import { Tick } from './Tick';
import { t } from '../i18n';

/**
 * A new passphrase: typed twice, with a line that counts its length and a box that shows it. The two are told apart for a screen reader,
 * and nothing here ever sends the passphrase anywhere: the window gives it to the engine once, when the person presses the button.
 */
export function PassphraseFields(props: {
  value: string;
  again: string;
  onChange: (value: string, again: string) => void;
  /** Names the fields for the browser check and keeps two sets on one page apart. */
  idPrefix: string;
  label?: string;
}) {
  const [shown, setShown] = useState(false);
  const type = shown ? 'text' : 'password';
  return (
    <div className="enc-fields">
      <TextField value={props.value} onChange={(v) => props.onChange(v, props.again)} className="field" type={type}>
        <Label>{props.label ?? t('Passphrase')}</Label>
        <Input autoComplete="new-password" data-testid={`${props.idPrefix}-passphrase`} spellCheck={false} />
        <Text slot="description" data-testid={`${props.idPrefix}-hint`}>{passphraseHint(props.value) || t('At least 8 characters. A few unrelated words is a good passphrase.')}</Text>
      </TextField>
      <TextField value={props.again} onChange={(v) => props.onChange(props.value, v)} className="field" type={type}>
        <Label>{t('Type it again')}</Label>
        <Input autoComplete="new-password" data-testid={`${props.idPrefix}-again`} spellCheck={false} />
      </TextField>
      <Tick isSelected={shown} onChange={setShown} testId={`${props.idPrefix}-show`}>{t('Show the passphrase')}</Tick>
    </div>
  );
}

/**
 * The warning window that comes before a passphrase is put on a project (decision 137): in plain words, that a lost passphrase cannot be
 * recovered by anyone, that the project cannot then be opened, and that nobody, the author of this program included, can help.
 */
export function LostPassphraseWarning(props: { what: string; onConfirm: () => void; onCancel: () => void }) {
  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && props.onCancel()} className="overlay">
      <Modal className="modal">
        <Dialog className="dialog" aria-labelledby="lost-title" role="alertdialog" data-testid="lost-passphrase-warning">
          <Heading slot="title" id="lost-title" className="dialog-title">{t('Read this before you go on')}</Heading>
          <p><strong>{t('If you lose this passphrase, nobody can ever open this project again.')}</strong></p>
          <p>{t('There is no reset and no recovery. The project cannot be opened without the passphrase, and nobody can help: not you, not a friend, and not the author of this program.')}</p>
          <p>{t('Write the passphrase down and keep it somewhere safe, apart from the project. While the project is open, its text is in your computer\'s memory like any other open document.')}</p>
          <div className="actions">
            <Button type="button" onPress={props.onCancel} data-testid="lost-passphrase-cancel">{t('Cancel')}</Button>
            <Button type="button" className="primary" onPress={props.onConfirm} data-testid="lost-passphrase-confirm">{props.what}</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
