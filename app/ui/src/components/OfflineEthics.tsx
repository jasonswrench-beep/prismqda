import { useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { refreshProjectInfo } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import { Notice } from './common';
import { Tick } from './Tick';
import { t } from '../i18n';

/** On the Privacy page: the offline lock, and the ethics summary an ethics board asks for (decision 165). */
export function OfflineAndEthics() {
  const project = useApp((s) => s.project);
  const readOnly = project?.read_only ?? false;
  const locked = project?.offline_lock ?? false;
  const [error, setError] = useState<string | null>(null);

  const setLock = async (on: boolean) => {
    setError(null);
    try {
      await call('privacy.set_offline_lock', { on });
      await refreshProjectInfo();
      announce(on ? t('The offline lock is on. No prompt packet can be built for an AI service from this project.') : t('The offline lock is off.'));
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  return (
    <>
      <section className="card" aria-labelledby="h-priv-lock" data-testid="privacy-lock">
        <h3 id="h-priv-lock">{t('Offline lock')}</h3>
        <p>{t('PrismQDA connects to nothing on its own (the one exception is the Check for updates button in Settings, which you press), and never sends a project or a word of one anywhere. The offline lock is the project saying so, and holding to it: while it is on, no prompt packet can be built for an AI service, so nothing of this project is put into a form meant to be taken to one, and PrismQDA does not check for updates. The lock is kept in the project, shows at the bottom of the window, and is in the ethics summary.')}</p>
        {error ? <Notice>{error}</Notice> : null}
        <Tick isSelected={locked} onChange={(on) => void setLock(on)} testId="privacy-lock-switch" isDisabled={readOnly}>
          {t('Lock this project offline')}
        </Tick>
        {readOnly ? <Notice kind="warning">{t('This project is read-only, so the lock cannot be changed.')}</Notice> : null}
        {locked ? <Notice kind="info">{t('The offline lock is on.')}</Notice> : null}
      </section>

      <section className="card" aria-labelledby="h-priv-ethics" data-testid="privacy-ethics">
        <h3 id="h-priv-ethics">{t('Ethics summary')}</h3>
        <p>{t('A Word document that states what an ethics board or a protocol amendment asks for: where the data is kept, whether it is encrypted, the offline lock, every prompt packet that was built and whether it was answered, what is marked do not share, whether names were replaced, who coded, and the length of the record of what was done. It is made from the project and says nothing else.')}</p>
        <div className="actions">
          <Button className="primary" onPress={() => appStore.set({ dialog: { kind: 'export', what: 'report', sections: ['ethics'] } })} data-testid="privacy-ethics-make">{t('Make the ethics summary…')}</Button>
        </div>
      </section>
    </>
  );
}
