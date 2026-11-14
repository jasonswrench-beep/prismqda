import { useEffect, useState } from 'react';
import { Button, Tab, TabList, TabPanel, Tabs } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { goTo, setAi, setAiMode } from '../state/actions';
import { useApp, type AiTab } from '../state/app';
import type { AiRunRow } from '../types';
import { AiLog } from './ai/Log';
import { AiPacketTab } from './ai/Packet';
import { Notice } from './common';
import { num, t, tn } from '../i18n';

/**
 * The AI Assistant, by prompt packet. PrismQDA never talks to a model: it builds a packet of text (the codebook and some
 * passages), the researcher takes it to any chatbot they choose, and pastes the answer back. The answer is checked against the
 * documents before anything of it reaches the review queue, where each suggestion waits for a decision.
 */
export function AiView() {
  const tab = useApp((s) => s.ai.tab);
  const mode = useApp((s) => s.project?.ai_mode ?? 'packet');
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const version = useApp((s) => s.dataVersion);
  const [runs, setRuns] = useState<AiRunRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    call('ai.runs', {})
      .then((r) => {
        if (!live) return;
        setRuns(r);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version]);

  const waiting = (runs ?? []).reduce((n, r) => n + r.pending, 0);
  return (
    <main id="main-content" tabIndex={-1} className="view-page ai-page" aria-labelledby="h-ai">
      <h2 id="h-ai">{t('AI Assistant')}</h2>
      <p className="help">{t('PrismQDA does not connect to any AI service. It builds a packet of text from your codebook and some of your passages. You take the packet to any chatbot you choose, and paste the answer back here. Every quoted word is checked against your documents, and what passes waits in the review queue. Nothing is coded until you accept it.')}</p>
      {mode === 'off' ? (
        <Notice kind="warning">
          <span data-testid="ai-off">{t('AI is off in this project, so no packet can be built.')}</span>{' '}
          <Button className="bk-link" onPress={() => void setAiMode('packet')} isDisabled={readOnly} data-testid="ai-turn-on">{t('Allow prompt packets again')}</Button>
        </Notice>
      ) : null}
      {error ? <Notice>{error}</Notice> : null}
      <Tabs selectedKey={tab} onSelectionChange={(k) => setAi({ tab: k as AiTab })} className="bk-editor-tabs">
        <TabList aria-label={t('AI Assistant')} className="bk-editor-tablist">
          <Tab id="packet" className="bk-tab" data-testid="ai-tab-packet">{t('Prompt packet')}</Tab>
          <Tab id="log" className="bk-tab" data-testid="ai-tab-log">{runs && runs.length > 0 ? t('AI use log ({count})', { count: num(runs.length) }) : t('AI use log')}
          </Tab>
        </TabList>
        <TabPanel id="packet" className="bk-editor-panel">
          <AiPacketTab mode={mode} />
        </TabPanel>
        <TabPanel id="log" className="bk-editor-panel">
          <AiLog runs={runs} />
        </TabPanel>
      </Tabs>
      {waiting > 0 ? (
        <p className="muted" data-testid="ai-waiting">
          {tn(waiting, { one: '{count} suggestion from an AI is waiting in the review queue.', other: '{count} suggestions from an AI are waiting in the review queue.' })}</p>
      ) : null}
      <p className="bk-editor-foot">
        <Button className="bk-link" onPress={() => goTo('code')}>{t('Back to the coding workspace')}</Button>
      </p>
    </main>
  );
}
