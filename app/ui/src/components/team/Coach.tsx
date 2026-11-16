import { useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { announce } from '../../state/announce';
import { useApp } from '../../state/app';
import type { CoachFinding, CoachPrompt, CoachReport } from '../../types';
import { Notice } from '../common';
import { Tick } from '../Tick';
import { num, t, tn } from '../../i18n';

/** What the coach says about one finding: always a question, never an answer. */
export function coachQuestion(f: CoachFinding): string {
  const code = f.codes[0]?.name ?? '';
  switch (f.rule) {
    case 'no_definition':
      return tn(f.count, {
        one: '“{code}” has no definition, and you used it once. How would you tell someone else when to use it?',
        other: '“{code}” has no definition, and you used it {count} times. How would you tell someone else when to use it?',
      }, { code });
    case 'used_once':
      return t('You used “{code}” once. Does it stand on its own, or does it belong with another code?', { code });
    case 'overlap':
      return t('“{a}” and “{b}” were given to the same words {count} times. What does each one add that the other does not?', { a: f.codes[0]?.name ?? '', b: f.codes[1]?.name ?? '', count: num(f.count) });
    default:
      return tn(f.count, {
        one: 'You coded “{phrase}” as “{code}”, and the same words stand uncoded in one other place. Is there a difference in meaning, or should they be coded alike?',
        other: 'You coded “{phrase}” as “{code}”, and the same words stand uncoded in {count} other places. Is there a difference in meaning, or should they be coded alike?',
      }, { phrase: f.phrase ?? '', code });
  }
}

/** The name of a kind of question, for the list of what was checked. */
function ruleName(rule: string): string {
  switch (rule) {
    case 'no_definition':
      return t('Codes with no definition');
    case 'used_once':
      return t('Codes used once');
    case 'overlap':
      return t('Codes given to the same words');
    default:
      return t('The same words coded in one place and not in another');
  }
}

/**
 * Coach (decision 199): questions about your own coding, from rules that can be read and checked. The coach asks and does not answer, and it
 * changes nothing in the project. A chatbot's help is optional and is a prompt you copy; nothing is sent from here.
 */
export function CoachTab() {
  const project = useApp((s) => s.project);
  const sources = useApp((s) => s.sources);
  const assignment = project?.packet?.assignment ?? null;
  const aiOff = project?.ai_mode === 'off';
  const [docs, setDocs] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<CoachReport | null>(null);
  const [prompt, setPrompt] = useState<CoachPrompt | null>(null);
  const [copied, setCopied] = useState(false);
  const textRef = useRef<HTMLTextAreaElement>(null);

  const check = async () => {
    setError(null);
    setPrompt(null);
    setBusy(true);
    try {
      const r = await call('coach.check', { source_ids: docs });
      setReport(r);
      announce(r.findings.length === 0 ? t('The coach has no question for you.') : tn(r.findings.length, { one: 'The coach has {count} question for you.', other: 'The coach has {count} questions for you.' }));
    } catch (e) {
      setReport(null);
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const makePrompt = async () => {
    setError(null);
    setCopied(false);
    try {
      const r = await call('coach.prompt', { source_ids: docs });
      setPrompt(r);
      announce(t('The prompt is made. Nothing has been sent anywhere.'));
    } catch (e) {
      setPrompt(null);
      setError(toApiError(e).message);
    }
  };

  const copy = async () => {
    if (!prompt) return;
    try {
      await navigator.clipboard.writeText(prompt.text);
      setCopied(true);
      announce(t('The prompt was copied. Nothing has left your computer yet.'));
    } catch {
      textRef.current?.focus();
      textRef.current?.select();
      announce(t('The prompt could not be copied for you. It is selected: press Control and C to copy it.'), 'assertive');
    }
  };

  return (
    <div className="coach-tab" data-testid="coach-tab">
      <section className="ac-card" aria-labelledby="h-coach">
        <h3 id="h-coach" className="bk-h3">{t('Questions about your own coding')}</h3>
        <p className="help">{t('The coach looks at the codings you made and asks about what stands out: a code with no definition, a code you used only once, two codes you gave to the same words again and again, and words you coded in one place and left uncoded in another. It asks questions and gives no answers, and it changes nothing in your project. It reads text only.')}</p>
        {assignment?.coach_locked ? <p className="muted" data-testid="coach-locked">{t('Your instructor has asked that the coach stay on for this assignment, so it is shown while you code.')}</p> : null}
        {error ? <Notice>{error}</Notice> : null}
        {sources.length > 1 ? (
          <fieldset className="field team-coders class-docs" data-testid="coach-docs">
            <legend>{t('The documents to look at')}</legend>
            {sources.map((s) => (
              <Tick key={s.id} isSelected={docs.includes(s.id)} onChange={(on) => setDocs(on ? [...docs, s.id] : docs.filter((x) => x !== s.id))} testId={`coach-doc-${s.id}`}>{s.name}</Tick>
            ))}
            <p className="help">{docs.length === 0 ? t('None is ticked, so every document is used.') : tn(docs.length, { one: '{count} document is ticked.', other: '{count} documents are ticked.' })}</p>
          </fieldset>
        ) : null}
        <div className="actions">
          <Button className="primary" onPress={() => void check()} isDisabled={busy} data-testid="coach-check">{busy ? t('Looking…') : t('Check my coding')}</Button>
        </div>

        {report ? (
          <div data-testid="coach-result">
            <p data-testid="coach-summary">{tn(report.codings, { one: '{count} coding of yours in {documents} looked at, using {codes}.', other: '{count} codings of yours in {documents} looked at, using {codes}.' }, { documents: tn(report.documents, { one: '{count} document', other: '{count} documents' }), codes: tn(report.codes_used, { one: '{count} code', other: '{count} codes' }) })}</p>
            <ul className="coach-rules" data-testid="coach-rules">
              {report.rules.map((r) => (
                <li key={r.rule} data-rule={r.rule}>{ruleName(r.rule)}: {r.findings === 0 ? t('nothing to ask') : num(r.findings)}</li>
              ))}
            </ul>
            {report.findings.length === 0 ? <p className="muted" data-testid="coach-none">{t('The coach has no question for you. That does not mean the coding is right; it means these checks found nothing.')}</p> : null}
            <ol className="coach-questions" data-testid="coach-questions">
              {report.findings.map((f, i) => (
                <li key={`${f.rule}-${f.codes.map((c) => c.id).join('-')}-${f.phrase ?? i}`} data-testid="coach-question" data-rule={f.rule}>
                  <p>{coachQuestion(f)}</p>
                  {f.examples.length > 0 ? (
                    <ul className="coach-examples">
                      {f.examples.map((e) => (
                        <li key={`${e.source_name}-${e.start}`}><span className="muted">{e.source_name}:</span> {e.text}</li>
                      ))}
                    </ul>
                  ) : null}
                </li>
              ))}
            </ol>
            <ul className="class-notes">
              {report.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </div>
        ) : null}
      </section>

      <section className="ac-card" aria-labelledby="h-coach-ai" data-testid="coach-ai-card">
        <h3 id="h-coach-ai" className="bk-h3">{t('A chatbot’s questions (optional)')}</h3>
        <p className="help">{t('Everything above works without a chatbot. If you want one to put questions to you, this makes a prompt you can copy into a chatbot of your choice. The prompt asks the chatbot to ask questions and not to give you answers or code for you. Making it sends nothing, and what you paste into another service is that service’s to keep under its own terms.')}</p>
        {aiOff ? <p className="muted" data-testid="coach-ai-off">{assignment && assignment.ai_allowed === false ? t('Your instructor has turned this off for this assignment.') : t('AI prompt packets are switched off for this project (Settings).')}</p> : null}
        <div className="actions">
          <Button onPress={() => void makePrompt()} isDisabled={aiOff} data-testid="coach-prompt">{t('Make the prompt')}</Button>
        </div>
        {prompt ? (
          <div data-testid="coach-prompt-result">
            <label htmlFor="coach-prompt-text">{t('The prompt')}</label>
            <textarea id="coach-prompt-text" ref={textRef} readOnly rows={12} value={prompt.text} data-testid="coach-prompt-text" />
            <p className="help">{t('Left out of the prompt: {things}.', { things: prompt.left_out.join('; ') })}</p>
            <div className="actions">
              <Button onPress={() => void copy()} data-testid="coach-copy">{copied ? t('Copied') : t('Copy the prompt')}</Button>
            </div>
          </div>
        ) : null}
      </section>
    </div>
  );
}
