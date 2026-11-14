import { useEffect, useState } from 'react';
import { Button, Tab, TabList, TabPanel, Tabs } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { goAi, goTo } from '../state/actions';
import { appStore, useApp, type AutoCodeTab } from '../state/app';
import type { BatchSummary } from '../types';
import { AutoCodeQueue } from './autocode/Queue';
import { AutoCodeSetup } from './autocode/Setup';
import { Notice } from './common';
import { num, t } from '../i18n';

/**
 * Auto Code: find passages by a rule and suggest a code for them (Set up), then accept, reject, or correct each
 * suggestion (Review queue). Nothing is coded until a person accepts it, and one undo takes a decision back.
 */
export function AutoCodeView() {
  const tab = useApp((s) => s.autoCodeTab);
  const version = useApp((s) => s.dataVersion);
  const [batches, setBatches] = useState<BatchSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    call('proposal.batches', {})
      .then((r) => {
        if (!live) return;
        setBatches(r);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version]);

  const waiting = (batches ?? []).reduce((n, b) => n + b.pending, 0);
  return (
    <main id="main-content" tabIndex={-1} className="view-page ac-page" aria-labelledby="h-autocode">
      <h2 id="h-autocode">{t('Auto Code')}</h2>
      {error ? <Notice>{error}</Notice> : null}
      <Tabs selectedKey={tab} onSelectionChange={(k) => appStore.set({ autoCodeTab: k as AutoCodeTab })} className="bk-editor-tabs">
        <TabList aria-label={t('Auto Code')} className="bk-editor-tablist">
          <Tab id="setup" className="bk-tab">{t('Set up')}</Tab>
          <Tab id="queue" className="bk-tab" data-testid="ac-tab-queue">{waiting > 0 ? t('Review queue ({count})', { count: num(waiting) }) : t('Review queue')}
          </Tab>
        </TabList>
        <TabPanel id="setup" className="bk-editor-panel">
          <AutoCodeSetup />
        </TabPanel>
        <TabPanel id="queue" className="bk-editor-panel">
          <AutoCodeQueue batches={batches} />
        </TabPanel>
      </Tabs>
      <p className="muted" data-testid="ac-ai-note">{t('Suggestions that come back from a chatbot wait in this same queue. ')}
        <Button className="bk-link" onPress={() => goAi('packet')}>{t('Open the AI Assistant')}</Button>
      </p>
      <p className="bk-editor-foot">
        <Button className="bk-link" onPress={() => goTo('code')}>{t('Back to the coding workspace')}</Button>
      </p>
    </main>
  );
}
