import { useState } from 'react';
import { Button, Checkbox, CheckboxGroup, Dialog, Heading, Input, Label, Modal, ModalOverlay, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { refreshCases } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import { Notice } from './common';
import { t, tn } from '../i18n';

/**
 * New Case: a case is a person, site, or group that documents belong to, so they can be compared and filtered together.
 * Name it and choose the documents it holds now (the open document is already chosen); documents can be added later.
 */
export function NewCaseDialog() {
  const sources = useApp((s) => s.sources);
  const openId = useApp((s) => s.currentSourceId);
  const cases = useApp((s) => s.cases);
  const [name, setName] = useState('');
  const [chosen, setChosen] = useState<string[]>(openId === null ? [] : [String(openId)]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const close = () => appStore.set({ dialog: null });

  const submit = async () => {
    setError(null);
    const trimmed = name.trim();
    if (!trimmed) return setError(t('Give the case a name.'));
    if (cases.some((c) => c.name.toLowerCase() === trimmed.toLowerCase())) return setError(t('There is already a case named {trimmed}.', { trimmed }));
    setBusy(true);
    try {
      const made = await call('case.add', { name: trimmed });
      for (const id of chosen) await call('case.assign', { case_id: made.id, source_id: Number(id) });
      await refreshCases();
      announce(chosen.length > 0 ? tn(chosen.length, { one: 'Added the case {trimmed} with {count} document.', other: 'Added the case {trimmed} with {count} documents.' }, { trimmed }) : t('Added the case {trimmed}.', { trimmed }));
      close();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && close()} className="overlay">
      <Modal className="modal">
        <Dialog className="dialog" aria-labelledby="new-case-title">
          <Heading slot="title" id="new-case-title" className="dialog-title">{t('New case')}</Heading>
          {error ? <Notice>{error}</Notice> : null}
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
            noValidate
          >
            <TextField value={name} onChange={setName} className="field" autoFocus>
              <Label>{t('Name')}</Label>
              <Input />
            </TextField>
            {sources.length > 0 ? (
              <CheckboxGroup value={chosen} onChange={setChosen} className="field case-docs">
                <Label>{t('Documents in this case')}</Label>
                {sources.map((s) => (
                  <Checkbox key={s.id} value={String(s.id)} className="check">
                    <div className="box" aria-hidden="true">
                      <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                        <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                      </svg>
                    </div>
                    {s.name}
                  </Checkbox>
                ))}
              </CheckboxGroup>
            ) : (
              <p className="help">{t('There are no documents yet. The case can hold documents once they are added.')}</p>
            )}
            <div className="actions">
              <Button type="submit" className="primary" isDisabled={busy}>{t('Add case')}</Button>
              <Button type="button" onPress={close}>{t('Cancel')}</Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
