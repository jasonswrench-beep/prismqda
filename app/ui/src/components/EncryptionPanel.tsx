import { useState } from 'react';
import { Button, Input, Label, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { passphraseProblem } from '../lib/passphrase';
import { refreshProjectInfo } from '../state/actions';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type { EncryptionOutcome, StaleBackup } from '../types';
import { Icon } from './Icon';
import { LostPassphraseWarning, PassphraseFields } from './PassphraseFields';
import { Notice } from './common';
import { t, tn } from '../i18n';

type Mode = 'idle' | 'encrypt' | 'change' | 'remove';

/**
 * Settings > Protecting a project (decisions 137 and 157): whether this project's file is encrypted, and turning encryption on, changing
 * the passphrase, and turning it off. A passphrase is put on a project only after a warning window that says, in plain words, that a lost
 * passphrase cannot be recovered by anyone.
 */
export function EncryptionPanel() {
  const project = useApp((s) => s.project);
  const [mode, setMode] = useState<Mode>('idle');
  const [old, setOld] = useState('');
  const [pass, setPass] = useState('');
  const [again, setAgain] = useState('');
  const [warn, setWarn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<EncryptionOutcome | null>(null);
  const [stale, setStale] = useState<StaleBackup[] | null>(null);
  const [deleted, setDeleted] = useState<number | null>(null);

  if (!project) {
    return (
      <>
        <p className="bk-set-state">
          <Icon name="st-encrypted" size={18} />
          <span>{t('Open a project to protect it with a passphrase.')}</span>
        </p>
        <p className="muted">{t('A project that is not encrypted is an ordinary file: anyone who can open it on this computer, or who is given a copy, can read it.')}</p>
      </>
    );
  }
  const encrypted = project.encrypted === true;

  const reset = () => {
    setMode('idle');
    setOld('');
    setPass('');
    setAgain('');
    setError(null);
    setWarn(false);
  };

  const start = (m: Mode) => {
    reset();
    setOutcome(null);
    setStale(null);
    setDeleted(null);
    setMode(m);
  };

  /** Checks what was typed, and opens the warning window when a passphrase is about to be put on the project. */
  const press = () => {
    setError(null);
    if (mode === 'encrypt' || mode === 'change') {
      const problem = passphraseProblem(pass, again);
      if (problem) return setError(problem);
      if (mode === 'change' && old === '') return setError(t('Type the current passphrase.'));
      setWarn(true);
      return;
    }
    if (mode === 'remove') {
      if (old === '') return setError(t('Type the passphrase of the project to turn encryption off.'));
      void run();
    }
  };

  const run = async () => {
    setWarn(false);
    setBusy(true);
    setError(null);
    try {
      const out =
        mode === 'encrypt'
          ? await call('project.encrypt', { passphrase: pass })
          : mode === 'change'
            ? await call('project.change_passphrase', { old, new: pass })
            : await call('project.decrypt', { passphrase: old });
      setOutcome(out);
      await refreshProjectInfo();
      reset();
      announce(mode === 'encrypt' ? t('The project is encrypted now.') : mode === 'change' ? t('The passphrase was changed.') : t('Encryption was turned off.'));
    } catch (e) {
      const err = toApiError(e);
      setError(err.code === 'wrong_passphrase' ? t('That passphrase is not the one this project has.') : err.message);
    } finally {
      setBusy(false);
    }
  };

  const look = async () => {
    setError(null);
    try {
      setStale((await call('project.stale_backups', {})).backups);
      setDeleted(null);
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  const deleteStale = async () => {
    setError(null);
    try {
      const r = await call('project.delete_stale_backups', {});
      setDeleted(r.deleted);
      setStale([]);
      setOutcome(outcome ? { ...outcome, stale_backups: 0 } : outcome);
      announce(tn(r.deleted, { one: 'Deleted {count} backup.', other: 'Deleted {count} backups.' }));
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  const readOnly = project.read_only;
  const staleCount = stale?.length ?? outcome?.stale_backups ?? 0;

  return (
    <div data-testid="encryption-panel">
      <p className="bk-set-state">
        <Icon name="st-encrypted" size={18} />
        <span data-testid="encryption-state">{encrypted ? t('This project is encrypted.') : t('This project is not encrypted.')}</span>
      </p>
      {encrypted ? (
        <p className="muted">{t('Every page of the project file is encrypted, and so are its backups. Without the passphrase the file cannot be read, and nobody can recover a lost passphrase: not you, not the author of this program, not anyone. Any program that opens SQLCipher 4 files opens it with the passphrase.')}</p>
      ) : (
        <p className="muted">{t('Anyone who can open this file on this computer, or who is given a copy of it, can read it. Encrypting it makes the file unreadable without a passphrase.')}</p>
      )}
      {error ? <Notice>{error}</Notice> : null}
      {readOnly ? <Notice kind="warning">{t('This project is read only, so it cannot be changed here.')}</Notice> : null}

      {mode === 'idle' ? (
        <div className="actions">
          {encrypted ? (
            <>
              <Button onPress={() => start('change')} isDisabled={readOnly} data-testid="encryption-change">{t('Change the passphrase…')}</Button>
              <Button onPress={() => start('remove')} isDisabled={readOnly} data-testid="encryption-remove">{t('Turn encryption off…')}</Button>
              <Button onPress={() => void look()} data-testid="encryption-check-backups">{t('Look for backups this passphrase does not protect')}</Button>
            </>
          ) : (
            <Button className="primary" onPress={() => start('encrypt')} isDisabled={readOnly} data-testid="encryption-start">{t('Encrypt this project…')}</Button>
          )}
        </div>
      ) : (
        <form
          className="enc-form"
          onSubmit={(e) => {
            e.preventDefault();
            press();
          }}
          noValidate
          aria-label={mode === 'encrypt' ? t('Encrypt this project') : mode === 'change' ? t('Change the passphrase') : t('Turn encryption off')}
        >
          {mode === 'encrypt' ? (
            <>
              <p>{t('Choose a passphrase and type it twice. PrismQDA writes a new, encrypted file, checks it, puts it in place of the old file, and overwrites the old file with zeros. Backups made before are not encrypted: the next step lists them and offers to delete them.')}</p>
              <PassphraseFields value={pass} again={again} onChange={(p, a) => { setPass(p); setAgain(a); }} idPrefix="enc" />
            </>
          ) : null}
          {mode === 'change' ? (
            <>
              <TextField value={old} onChange={setOld} className="field" type="password">
                <Label>{t('Current passphrase')}</Label>
                <Input autoComplete="off" data-testid="enc-old" spellCheck={false} />
              </TextField>
              <PassphraseFields value={pass} again={again} onChange={(p, a) => { setPass(p); setAgain(a); }} idPrefix="enc" label={t('New passphrase')} />
            </>
          ) : null}
          {mode === 'remove' ? (
            <>
              <Notice kind="warning">{t('Without encryption, the project file can be read by anyone who gets it. Backups already made stay encrypted.')}</Notice>
              <TextField value={old} onChange={setOld} className="field" type="password">
                <Label>{t('Passphrase of the project')}</Label>
                <Input autoComplete="off" data-testid="enc-old" spellCheck={false} />
              </TextField>
            </>
          ) : null}
          <div className="actions">
            <Button type="submit" className="primary" isDisabled={busy} data-testid="enc-go">
              {mode === 'encrypt' ? t('Encrypt the project…') : mode === 'change' ? t('Change the passphrase…') : t('Turn encryption off')}
            </Button>
            <Button type="button" onPress={reset} isDisabled={busy} data-testid="enc-cancel">{t('Cancel')}</Button>
          </div>
        </form>
      )}

      {warn ? (
        <LostPassphraseWarning what={mode === 'change' ? t('I understand. Change the passphrase') : t('I understand. Encrypt the project')} onConfirm={() => void run()} onCancel={() => setWarn(false)} />
      ) : null}

      {outcome ? (
        <div data-testid="enc-outcome">
          <Notice kind="info">
            {outcome.encrypted ? t('The project is encrypted with {cipher}.', { cipher: outcome.cipher }) : t('The project is no longer encrypted.')}{' '}
            {outcome.old_file_erased ? t('The old file was overwritten with zeros and deleted.') : t('The old file could not be overwritten, so it was deleted without that. A solid-state drive may still hold its bytes.')}
          </Notice>
        </div>
      ) : null}

      {staleCount > 0 || deleted !== null ? (
        <div data-testid="enc-stale">
          {deleted !== null ? (
            <Notice kind="info">{tn(deleted, { one: 'Deleted {count} backup that the passphrase did not protect.', other: 'Deleted {count} backups that the passphrase did not protect.' })}</Notice>
          ) : (
            <>
              <Notice kind="warning">
                {tn(staleCount, {
                  one: '{count} earlier backup is not protected by this passphrase: it is not encrypted, or it uses an earlier passphrase, so it still holds what the project held. It is in the project\'s backup folder.',
                  other: '{count} earlier backups are not protected by this passphrase: they are not encrypted, or they use an earlier passphrase, so they still hold what the project held. They are in the project\'s backup folder.',
                })}
              </Notice>
              <div className="actions">
                <Button onPress={() => void deleteStale()} data-testid="enc-delete-stale">{t('Overwrite and delete them')}</Button>
              </div>
            </>
          )}
        </div>
      ) : stale !== null ? (
        <p className="muted" data-testid="enc-stale-none">{t('Every backup of this project is protected by its passphrase.')}</p>
      ) : null}
      <p className="muted">{encrypted ? t('A backup of this project, and a copy of the whole project file, stay encrypted.') : t('You can also encrypt a project when you create it.')}</p>
    </div>
  );
}
