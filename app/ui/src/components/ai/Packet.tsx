// The AI Assistant, prompt packet: choose what to send, read and copy the exact text of the packet, then paste the answer back.
// Nothing is sent anywhere by this page. What leaves the computer is what the person pastes into another program.

import { useMemo, useRef, useState } from 'react';
import { Button, Checkbox, Radio, RadioGroup } from 'react-aria-components';
import { MAX_LIMIT, outcomeSentences, packetArgs, packetSentences, suggestionLine } from '../../lib/ai';
import { buildAiPacket, goAutoCode, sendAiAnswer, setAi, setAiSetup, setSourceDoNotShare } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import { Notice, plural } from '../common';
import { NamesNotice } from '../NamesNotice';
import { StaleSet } from '../WorkingSet';
import { CheckPicker, codeChoices } from '../explore/shared';
import { num, rich, t, tn } from '../../i18n';

export function AiPacketTab(props: { mode: 'packet' | 'off' }) {
  const ai = useApp((s) => s.ai);
  const sources = useApp((s) => s.sources);
  const cases = useApp((s) => s.cases);
  const codes = useApp((s) => s.codes);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const { setup, packet, model, answer, outcome } = ai;
  const [busy, setBusy] = useState<'building' | 'sending' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [answerError, setAnswerError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const packetHeading = useRef<HTMLHeadingElement>(null);

  const off = props.mode === 'off';
  const sendable = useMemo(() => codeChoices(codes).filter((c) => {
    const code = codes.find((x) => x.id === c.id);
    return code !== undefined && code.ai_suggestible !== false && !code.do_not_share;
  }), [codes]);
  const keptBack = codes.filter((c) => c.status === 'active' && (c.ai_suggestible === false || c.do_not_share)).length;
  const marked = sources.filter((s) => s.do_not_share).length;
  const sourceChoices = sources.map((s) => ({ id: s.id, label: s.do_not_share ? t('{name} (do not share)', { name: s.name }) : s.name }));
  const built = packetArgs(setup);

  const build = async (skip: number) => {
    setError(null);
    const form = packetArgs({ ...setup, skip });
    if ('problem' in form) {
      setError(form.problem);
      announce(form.problem, 'assertive');
      return;
    }
    setBusy('building');
    const setThen = appStore.get().workingSet;
    const r = await buildAiPacket(form.args);
    setBusy(null);
    if ('problem' in r) {
      setError(r.problem);
      announce(r.problem, 'assertive');
      return;
    }
    setAiSetup({ skip });
    setAi({ packet: r.packet, packetSet: setThen, model: '', answer: '', outcome: null });
    setAnswerError(null);
    setCopied(false);
    announce(t('The packet is built: {value} Nothing has been sent anywhere.', { value: packetSentences(r.packet)[0] }));
    requestAnimationFrame(() => packetHeading.current?.focus());
  };

  const copy = async () => {
    if (!packet) return;
    try {
      await navigator.clipboard.writeText(packet.prompt);
      setCopied(true);
      announce(t('The packet was copied. Nothing has left your computer yet.'));
    } catch {
      textRef.current?.focus();
      textRef.current?.select();
      announce(t('The packet could not be copied for you. It is selected: press Control and C to copy it.'), 'assertive');
    }
  };

  const send = async () => {
    if (!packet) return;
    setAnswerError(null);
    setBusy('sending');
    const r = await sendAiAnswer(packet.run_id, model, answer);
    setBusy(null);
    if ('problem' in r) {
      setAnswerError(r.problem);
      announce(r.problem, 'assertive');
      return;
    }
    setAi({ outcome: r.outcome });
    announce(outcomeSentences(r.outcome).join(' '));
  };

  const again = () => {
    setAi({ packet: null, model: '', answer: '', outcome: null });
    setAnswerError(null);
    setCopied(false);
  };

  return (
    <div className="ai-packet" data-testid="ai-packet-tab">
      <section className="ac-card" aria-labelledby="h-ai-choose">
        <h3 id="h-ai-choose" className="bk-h3">{t('Choose what to send')}</h3>
        <RadioGroup value={setup.scope} onChange={(v) => setAiSetup({ scope: v as 'all' | 'documents' | 'case' })} className="field ac-radios ac-inline" aria-label={t('Which documents')}>
          <Radio value="all" className="react-aria-Radio">
            <span>{t('Every document')}</span>
          </Radio>
          <Radio value="documents" className="react-aria-Radio">
            <span>{t('Documents I choose')}</span>
          </Radio>
          <Radio value="case" className="react-aria-Radio">
            <span>{t('The documents of one case')}</span>
          </Radio>
        </RadioGroup>
        {setup.scope === 'documents' ? (
          <div className="field">
            <CheckPicker noun="documents" label={t('Documents to send')} choices={sourceChoices} value={setup.sourceIds} onChange={(ids) => setAiSetup({ sourceIds: ids })} none={t('Choose documents')} testId="ai-documents" />
          </div>
        ) : null}
        {setup.scope === 'case' ? (
          <div className="field">
            <label htmlFor="ai-case">{t('Case')}</label>
            <select id="ai-case" value={setup.caseId === null ? '' : String(setup.caseId)} onChange={(e) => setAiSetup({ caseId: e.target.value === '' ? null : Number(e.target.value) })}>
              <option value="">{t('Choose a case…')}</option>
              {cases.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </div>
        ) : null}

        <div className="field">
          {rich("<span>Codes to ask about</span><x1/><span2>A code's name, definition, and criteria go in the packet.</span2>", { span: (inside) => <span id="ai-codes-label" className="ai-label">{inside}</span>, x1: <CheckPicker noun="codes" label={t('Codes to ask about')} choices={sendable} value={setup.codeIds} onChange={(ids) => setAiSetup({ codeIds: ids })} none={t('Every code that may be sent')} testId="ai-codes" />, span2: (inside) => <span className="help">{keptBack > 0 ? tn(keptBack, { one: '{codes} marked do not share or not for AI is never sent.', other: '{codes} marked do not share or not for AI are never sent.' }, { codes: tn(keptBack, { one: '{count} code', other: '{count} codes' }) }) + ' ' : ''}{inside}</span> })}
        </div>

        <Checkbox isSelected={setup.onlyUncoded} onChange={(on) => setAiSetup({ onlyUncoded: on })} className="check" data-testid="ai-uncoded">
          <Box />{t('Leave out passages I have already coded')}</Checkbox>

        <div className="field">
          <label htmlFor="ai-limit">{t('How many passages')}</label>
          <input id="ai-limit" type="text" inputMode="numeric" value={setup.limit} onChange={(e) => setAiSetup({ limit: e.target.value })} autoComplete="off" aria-describedby="ai-limit-help" />
          <span id="ai-limit-help" className="help">{t('From 1 to {MAX_LIMIT}. A smaller packet is easier to read and easier for a chatbot to answer well. A passage is a sentence, a turn, or a paragraph, as the document is divided.', { MAX_LIMIT })}</span>
        </div>

        <details className="ai-private" data-testid="ai-private">
          <summary>{marked > 0 ? t('Documents that must never be sent ({marked} marked)', { marked: num(marked) }) : t('Documents that must never be sent')}
          </summary>
          <p className="help">{t('A document marked here is left out of every packet, whatever else is chosen.')}</p>
          {sources.length === 0 ? (
            <p className="muted">{t('There are no documents yet.')}</p>
          ) : (
            <ul className="ai-private-list">
              {sources.map((s) => (
                <li key={s.id}>
                  <Checkbox isSelected={s.do_not_share} onChange={(on) => void setSourceDoNotShare(s.id, on)} isDisabled={readOnly} className="check" aria-label={t('Never send {name}', { name: s.name })}>
                    <Box />
                    {s.name}
                  </Checkbox>
                </li>
              ))}
            </ul>
          )}
        </details>

        <div className="actions">
          <Button className="primary" onPress={() => void build(0)} isDisabled={busy !== null || readOnly || off} data-testid="ai-build">
            {busy === 'building' ? t('Building…') : packet ? t('Build a new packet') : t('Build the packet')}
          </Button>
        </div>
        {'problem' in built && !error ? <p className="muted">{built.problem}</p> : null}
        <NamesNotice what="packet" />
        {error ? <Notice>{error}</Notice> : null}
        {readOnly ? <p className="muted">{t('This project is read only, so a packet cannot be built.')}</p> : null}
      </section>

      {packet ? (
        <>
          <section className="ac-card" aria-labelledby="h-ai-packet" data-testid="ai-packet">
            <h3 id="h-ai-packet" className="bk-h3" tabIndex={-1} ref={packetHeading}>{t('The packet')}</h3>
            <StaleSet since={ai.packetSet} what="packet" />
            <ul className="ac-facts" data-testid="ai-packet-facts">
              {packetSentences(packet).map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
            <p className="ai-reassure" data-testid="ai-nothing-sent">{t('Nothing has left your computer. This text is only here and in your project’s record of the run. When you paste it into a chatbot, that service receives it under its own terms, so check them if your documents are sensitive.')}</p>
            <div className="field">
              <label htmlFor="ai-packet-text">{t('The exact text to take to a chatbot')}</label>
              <textarea id="ai-packet-text" ref={textRef} className="ai-text" readOnly rows={14} value={packet.prompt} data-testid="ai-packet-text" />
            </div>
            <div className="actions">
              <Button className="primary" onPress={() => void copy()} data-testid="ai-copy">
                {copied ? t('Copied') : t('Copy the packet')}
              </Button>
              {packet.more_after > 0 ? (
                <Button onPress={() => void build(packet.next_skip)} isDisabled={busy !== null || readOnly || off} data-testid="ai-next">{t('Build the next packet ({passages} left)', { passages: tn(packet.more_after, { one: '{count} passage', other: '{count} passages' }) })}</Button>
              ) : null}
            </div>
          </section>

          <section className="ac-card" aria-labelledby="h-ai-answer" data-testid="ai-answer-card">
            <h3 id="h-ai-answer" className="bk-h3">{t('Paste the answer')}</h3>
            {outcome ? (
              <>
                <ul className="ac-facts" data-testid="ai-outcome">
                  {outcomeSentences(outcome).map((t) => (
                    <li key={t}>{t}</li>
                  ))}
                </ul>
                {outcome.rejected.length > 0 ? (
                  <>
                    <p className="ac-examples-title">{t('What the check refused:')}</p>
                    <ul className="ai-refused" aria-label={t('Suggestions the check refused')} data-testid="ai-refused">
                      {outcome.rejected.map((r) => (
                        <li key={r.number}>{suggestionLine(r)}</li>
                      ))}
                    </ul>
                  </>
                ) : null}
                <div className="actions">
                  {outcome.batch ? (
                    <Button className="primary" onPress={() => goAutoCode('queue', outcome.batch)} data-testid="ai-review">{t('Review the suggestions')}</Button>
                  ) : null}
                  <Button onPress={again} data-testid="ai-again">{t('Start another packet')}</Button>
                </div>
              </>
            ) : (
              <>
                <p className="help">{t('Copy the model’s whole reply, whatever words surround the list. Anything the packet did not ask about, a quote that is not word for word in its passage, or a code that was not in the packet is refused.')}</p>
                <div className="field">
                  <label htmlFor="ai-model">{t('Which model gave this answer')}</label>
                  <input id="ai-model" type="text" value={model} onChange={(e) => setAi({ model: e.target.value })} autoComplete="off" aria-describedby="ai-model-help" />
                  <span id="ai-model-help" className="help">{t('For example the chatbot’s name and version. It is kept with the run so you can say which model suggested what.')}</span>
                </div>
                <div className="field">
                  <label htmlFor="ai-answer">{t('The model’s answer')}</label>
                  <textarea id="ai-answer" className="ai-text" rows={10} value={answer} onChange={(e) => setAi({ answer: e.target.value })} spellCheck={false} />
                </div>
                <div className="actions">
                  <Button className="primary" onPress={() => void send()} isDisabled={busy !== null || readOnly || model.trim() === '' || answer.trim() === ''} data-testid="ai-send">
                    {busy === 'sending' ? t('Checking…') : t('Check the answer and send it to the review queue')}
                  </Button>
                </div>
                {answerError ? <Notice>{answerError}</Notice> : null}
              </>
            )}
          </section>
        </>
      ) : null}
    </div>
  );
}

function Box() {
  return (
    <div className="box" aria-hidden="true">
      <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
        <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
      </svg>
    </div>
  );
}
