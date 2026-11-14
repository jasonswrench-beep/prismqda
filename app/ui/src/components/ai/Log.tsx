// The AI use log: every packet that was built, which model answered, what the check refused, and what became of each suggestion.
// It also writes the paragraph for a methods section from these records.

import { useEffect, useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { runLine, runName, suggestionLine } from '../../lib/ai';
import { formatWhen } from '../../lib/eventWording';
import { goAutoCode, goAi, setAi } from '../../state/actions';
import { announce } from '../../state/announce';
import { useApp } from '../../state/app';
import type { AiRunDetail, AiRunRow } from '../../types';
import { Notice } from '../common';
import { rich, t } from '../../i18n';

export function AiLog(props: { runs: AiRunRow[] | null }) {
  const { runs } = props;
  const open = useApp((s) => s.ai.openRun);
  const version = useApp((s) => s.dataVersion);
  const [detail, setDetail] = useState<AiRunDetail | null>(null);
  const [paragraph, setParagraph] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<'paragraph' | null>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    if (open === null) {
      setDetail(null);
      return;
    }
    let live = true;
    call('ai.run', { id: open })
      .then((r) => {
        if (!live) return;
        setDetail(r);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [open, version]);

  useEffect(() => {
    let live = true;
    call('ai.disclosure', {})
      .then((r) => live && setParagraph(r.text))
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version]);

  const copy = async (text: string, what: 'paragraph', said: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(what);
      announce(said);
    } catch {
      announce(t('It could not be copied for you. Select the text and press Control and C.'), 'assertive');
    }
  };

  const show = (id: number) => {
    setAi({ openRun: id });
    requestAnimationFrame(() => headingRef.current?.focus());
  };

  /** A packet that was built but never answered can have its answer pasted from the packet tab. */
  const continueWith = (d: AiRunDetail) => {
    setAi({
      tab: 'packet',
      packet: { run_id: d.id, prompt: d.prompt, passages: d.passages, characters: Array.from(d.prompt).length, codes: [], left_out: { documents_not_shared: 0, passages_not_shared: 0, already_coded: 0, too_long: 0, pictures: 0, codes: [] }, more_after: 0, next_skip: 0 },
      model: '',
      answer: '',
      outcome: null,
    });
    announce(t('This packet is open on the Prompt packet tab, ready for its answer.'));
  };

  return (
    <div className="ai-log" data-testid="ai-log">
      {error ? <Notice>{error}</Notice> : null}
      {runs === null ? (
        <p className="muted" role="status">{t('Loading the log…')}</p>
      ) : runs.length === 0 ? (
        <div>
          <p className="help" data-testid="ai-log-empty">{t('No packet has been built in this project. When one is, the log keeps the exact text that was sent, the answer you pasted, what the check refused, and what you decided.')}</p>
          <Button onPress={() => goAi('packet')}>{t('Go to Prompt packet')}</Button>
        </div>
      ) : (
        <div className="ai-log-grid">
          <div className="ai-log-list">
            <h3 className="bk-h3">{t('Runs')}</h3>
            <ul>
              {runs.map((r) => (
                <li key={r.id}>
                  <Button className={`ac-run${open === r.id ? ' current' : ''}`} aria-pressed={open === r.id} onPress={() => show(r.id)} data-testid="ai-run">
                    <span className="ac-run-name">{runName(r)}</span>
                    <span className="ac-run-line">{runLine(r)}</span>
                    <span className="ac-run-when">{formatWhen(r.created_at)}</span>
                  </Button>
                </li>
              ))}
            </ul>
          </div>

          <div className="ai-log-detail" data-testid="ai-run-detail">
            {detail === null ? (
              <p className="help">{t('Choose a run to see the exact packet, the answer, and what became of it.')}</p>
            ) : (
              <>
                <h3 className="bk-h3" tabIndex={-1} ref={headingRef} id="h-ai-run">
                  {runName(detail)}
                </h3>
                <ul className="ac-facts">
                  <li>{t('Built {when}. Route: prompt packet, so nothing was sent by PrismQDA.', { when: formatWhen(detail.created_at) })}</li>
                  <li>{runLine(detail)}.</li>
                  <li>{rich('A fingerprint of the packet text (SHA-256): <code>{prompt_hash}</code>', { prompt_hash: detail.prompt_hash, code: (inside) => <code className="ai-hash">{inside}</code> })}
                  </li>
                </ul>
                <div className="actions">
                  {detail.batch ? (
                    <Button onPress={() => goAutoCode('queue', detail.batch)} data-testid="ai-run-queue">{t('Open its suggestions in the review queue')}</Button>
                  ) : null}
                  {!detail.answered ? (
                    <Button className="primary" onPress={() => continueWith(detail)} data-testid="ai-run-continue">{t('Paste an answer for this packet')}</Button>
                  ) : null}
                </div>
                <div className="field">
                  <label htmlFor="ai-run-prompt">{t('The packet, exactly as it was built')}</label>
                  <textarea id="ai-run-prompt" className="ai-text" readOnly rows={8} value={detail.prompt} />
                </div>
                {detail.answered ? (
                  <div className="field">
                    <label htmlFor="ai-run-answer">{t('The answer, exactly as it was pasted')}</label>
                    <textarea id="ai-run-answer" className="ai-text" readOnly rows={8} value={detail.raw_output} />
                  </div>
                ) : null}
                {detail.checker_rejections.length > 0 ? (
                  <>
                    <p className="ac-examples-title">{t('What the check refused:')}</p>
                    <ul className="ai-refused" aria-label={t('Suggestions the check refused')}>
                      {detail.checker_rejections.map((r) => (
                        <li key={r.number}>{suggestionLine(r)}</li>
                      ))}
                    </ul>
                  </>
                ) : null}
              </>
            )}
          </div>
        </div>
      )}

      <section className="ac-card" aria-labelledby="h-ai-methods" data-testid="ai-disclosure">
        <h3 id="h-ai-methods" className="bk-h3">{t('Text for your methods section')}</h3>
        <p className="help">{t('Written from the records above. Read it, and change it to match what you did, before you use it.')}</p>
        <div className="field">
          <label htmlFor="ai-disclosure-text">{t('Statement about AI use')}</label>
          <textarea id="ai-disclosure-text" className="ai-text" readOnly rows={7} value={paragraph ?? ''} />
        </div>
        <div className="actions">
          <Button onPress={() => paragraph && void copy(paragraph, 'paragraph', t('The statement was copied.'))} isDisabled={!paragraph} data-testid="ai-copy-disclosure">
            {copied === 'paragraph' ? t('Copied') : t('Copy the statement')}
          </Button>
        </div>
      </section>
    </div>
  );
}
