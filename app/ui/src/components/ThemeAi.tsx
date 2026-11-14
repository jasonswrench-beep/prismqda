import { useEffect, useRef, useState } from 'react';
import { Button, Input, Label, TextArea, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { announce } from '../state/announce';
import { appStore, useApp, type ThemeAiState } from '../state/app';
import type { ThemePacket, ThemeRun, ThemeSuggestion } from '../types';
import { Notice } from './common';
import { num, t, tn } from '../i18n';

/** One suggestion: what the model grouped, what the checker dropped, and the choice to accept (with a name of the researcher's own) or reject. */
function Suggestion(props: { s: ThemeSuggestion; readOnly: boolean; onDecide: (index: number, accept: boolean, name: string) => Promise<void> }) {
  const { s } = props;
  const [name, setName] = useState(s.name);
  return (
    <li className="th-suggestion" data-testid="theme-suggestion">
      <h4 className="bk-h3">{s.name}</h4>
      {s.definition ? <p>{s.definition}</p> : null}
      <p className="muted">{`${t('Codes:')} ${s.code_names.join(', ')}`}</p>
      {s.problems.length > 0 ? (
        <ul className="muted" data-testid="suggestion-problems">
          {s.problems.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      ) : null}
      {s.status === 'pending' ? (
        props.readOnly ? null : (
          <>
            <TextField value={name} onChange={setName} className="field">
              <Label>{t('Name to give the theme')}</Label>
              <Input data-testid="suggestion-name" />
            </TextField>
            <div className="actions">
              <Button className="primary" onPress={() => void props.onDecide(s.index, true, name)} isDisabled={name.trim() === ''} data-testid="suggestion-accept">{t('Accept as a candidate theme')}</Button>
              <Button onPress={() => void props.onDecide(s.index, false, name)} data-testid="suggestion-reject">{t('Reject')}</Button>
            </div>
          </>
        )
      ) : (
        <p data-testid="suggestion-status"><strong>{s.status === 'accepted' ? t('Accepted: the theme was made.') : t('Rejected.')}</strong></p>
      )}
    </li>
  );
}

/**
 * Ask an AI for candidate themes (decision 168): a prompt packet made from the names, definitions, and uses of the codes (no passage of any
 * document), taken to a model of the researcher's choice, and the answer pasted back. A checker drops what the model made up; each suggestion
 * waits until it is accepted. PrismQDA calls no model.
 */
export function ThemeAi(props: { onChanged: () => void }) {
  const project = useApp((s) => s.project);
  const readOnly = project?.read_only ?? false;
  const locked = project?.offline_lock ?? false;
  const aiOff = project?.ai_mode === 'off';
  // The packet, the model's name, the pasted answer, and the run being decided are kept in the app state, so leaving the page (to look at a
  // code, say) does not lose an answer that was being pasted.
  const { packet, run, model, answer } = useApp((s) => s.themeAi);
  const patch = (p: Partial<ThemeAiState>) => appStore.set((s) => ({ themeAi: { ...s.themeAi, ...p } }));
  const setPacket = (v: ThemePacket | null) => patch({ packet: v });
  const setRun = (v: ThemeRun | null) => patch({ run: v });
  const setModel = (v: string) => patch({ model: v });
  const setAnswer = (v: string) => patch({ answer: v });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const textRef = useRef<HTMLTextAreaElement | null>(null);

  // A run that has suggestions still waiting is picked up again when the page is opened, unless the page already holds something.
  useEffect(() => {
    let live = true;
    call('ai.runs', {})
      .then(async (runs) => {
        const waiting = runs.find((r) => r.task === 'suggest_themes' && r.answered && r.pending > 0);
        if (!waiting || !live) return;
        const here = appStore.get().themeAi;
        if (here.run || here.packet) return;
        const found = await call('theme.ai_run', { run_id: waiting.id });
        if (live && !appStore.get().themeAi.run && !appStore.get().themeAi.packet) setRun(found);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  const build = async () => {
    setError(null);
    setBusy(true);
    try {
      const p = await call('theme.ai_packet', {});
      setPacket(p);
      setRun(null);
      setAnswer('');
      announce(t('The packet was built. Nothing has left your computer.'));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  const copy = async () => {
    if (!packet) return;
    try {
      await navigator.clipboard.writeText(packet.prompt);
      announce(t('The packet was copied. Nothing has left your computer yet.'));
    } catch {
      textRef.current?.focus();
      textRef.current?.select();
      announce(t('The packet could not be copied for you. It is selected: press Control and C to copy it.'), 'assertive');
    }
  };
  const check = async () => {
    if (!packet) return;
    setError(null);
    setBusy(true);
    try {
      const out = await call('theme.ai_answer', { run_id: packet.run_id, model, answer });
      setRun({ run_id: packet.run_id, model, answered: true, suggestions: out.suggestions, rejected: out.rejected });
      announce(tn(out.suggestions.length, { one: '{count} theme was suggested.', other: '{count} themes were suggested.' }));
      setPacket(null);
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  const decide = async (index: number, accept: boolean, name: string) => {
    if (!run) return;
    setError(null);
    try {
      setRun(await call('theme.ai_decide', { run_id: run.run_id, index, accept, ...(accept ? { name } : {}) }));
      announce(accept ? t('The theme was made.') : t('The suggestion was rejected.'));
      props.onChanged();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  return (
    <section className="ac-card" aria-labelledby="h-theme-ai" data-testid="theme-ai">
      <h3 id="h-theme-ai" className="bk-h3">{t('Ask an AI for candidate themes')}</h3>
      <p className="help">{t('PrismQDA calls no AI. It builds a packet of text from the names, definitions, and number of uses of your codes, and no passage of any document. You take it to a model of your own choice, paste the answer back, and PrismQDA checks it: a code the packet did not name is dropped, and a theme with fewer than two codes is refused. Each suggestion waits until you accept it.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      {locked ? <Notice kind="warning">{t('The offline lock is on, so no packet can be built. Turn it off on the Privacy page to ask an AI.')}</Notice> : null}
      {aiOff ? <Notice kind="warning">{t('AI is off in this project. Turn it on in Settings to build a packet.')}</Notice> : null}
      <div className="actions">
        <Button onPress={() => void build()} isDisabled={busy || readOnly || locked || aiOff} data-testid="theme-ai-build">{busy && !packet ? t('Building…') : t('Build a packet for a chatbot')}</Button>
      </div>

      {packet ? (
        <div data-testid="theme-ai-packet">
          <p className="muted">{tn(packet.codes.length, { one: 'The packet holds {count} code and {chars} characters. Nothing has left your computer.', other: 'The packet holds {count} codes and {chars} characters. Nothing has left your computer.' }, { chars: num(packet.characters) })}{packet.left_out.length > 0 ? ' ' + t('Left out, because they are marked do not share or not suggestible: {names}.', { names: packet.left_out.join(', ') }) : ''}</p>
          <TextField value={packet.prompt} isReadOnly className="field">
            <Label>{t('The packet: copy all of it into a chatbot')}</Label>
            <TextArea ref={textRef} rows={10} data-testid="theme-ai-prompt" />
          </TextField>
          <div className="actions">
            <Button onPress={() => void copy()} data-testid="theme-ai-copy">{t('Copy the packet')}</Button>
          </div>
          <TextField value={model} onChange={setModel} className="field">
            <Label>{t('Which model gave the answer')}</Label>
            <Input data-testid="theme-ai-model" autoComplete="off" />
          </TextField>
          <TextField value={answer} onChange={setAnswer} className="field">
            <Label>{t('Paste the answer here')}</Label>
            <TextArea rows={8} data-testid="theme-ai-answer" />
          </TextField>
          <div className="actions">
            <Button className="primary" onPress={() => void check()} isDisabled={busy || answer.trim() === '' || model.trim() === ''} data-testid="theme-ai-check">{busy ? t('Checking…') : t('Check the answer')}</Button>
          </div>
        </div>
      ) : null}

      {run ? (
        <div data-testid="theme-ai-run">
          <h4 className="bk-h3">{t('Suggested by {model}', { model: run.model })}</h4>
          {run.suggestions.length === 0 ? <p className="muted">{t('No theme passed the check.')}</p> : null}
          <ul className="th-suggestions">
            {run.suggestions.map((s) => (
              <Suggestion key={`${run.run_id}-${s.index}-${s.status}`} s={s} readOnly={readOnly} onDecide={decide} />
            ))}
          </ul>
          {run.rejected.length > 0 ? (
            <>
              <h4 className="bk-h3">{t('Left out by the checker')}</h4>
              <ul className="muted" data-testid="theme-ai-rejected">
                {run.rejected.map((r) => (
                  <li key={r.number}>{`${r.code ? `“${r.code}”: ` : ''}${r.why}`}</li>
                ))}
              </ul>
            </>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
