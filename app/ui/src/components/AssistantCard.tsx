import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { call } from '../bridge';
import { assistantConfig, type AssistantPermission } from '../lib/assistant';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import { t } from '../i18n';

/**
 * Connecting an assistant (decision 203, in Settings > AI Assistant): the settings an assistant application needs so that it can start this
 * program as its pipe to the open project. PrismQDA itself connects to nothing: the application starts it, and talks to it over a pipe.
 */
export function AssistantCard() {
  const project = useApp((s) => s.project);
  const coders = useApp((s) => s.coders);
  const me = coders.find((c) => c.id === project?.me)?.name ?? '';
  const [permission, setPermission] = useState<AssistantPermission>('read');
  const [actor, setActor] = useState(me);
  const [command, setCommand] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Where the program is, when this is the installed program; a browser has none.
  useEffect(() => {
    let live = true;
    call('assistant.command', {})
      .then((r) => live && setCommand(r.command))
      .catch(() => live && setCommand(null));
    return () => {
      live = false;
    };
  }, []);
  useEffect(() => setActor((a) => (a === '' ? me : a)), [me]);

  const closed = project?.offline_lock ? t('The offline lock is on for this project, so no assistant can be connected to it.') : project?.ai_mode === 'off' ? t('AI is switched off for this project, which closes this route too. Turn it on above to connect an assistant.') : null;
  const shown = assistantConfig(command ?? t('<the path of PrismQDA.exe>'), project?.path ?? '', permission, actor);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(shown);
      setCopied(true);
      announce(t('The settings were copied. Nothing has left your computer.'));
    } catch {
      announce(t('The settings could not be copied for you. Select them and press Control and C.'), 'assertive');
    }
  };

  return (
    <div data-testid="assistant-card">
      <p className="muted">{t('An assistant application on this computer that speaks the Model Context Protocol can be set to start PrismQDA as a pipe to this project. PrismQDA opens no port and connects to nothing: the application starts it and talks to it, and it never reaches out. What the application and its service then do with what they read is theirs, under their own terms, so connect one only if the project’s ethics approval allows it.')}</p>
      <ul className="bk-set-list">
        <li>{t('It can read the documents, the codebook, the codings and a search and word counts. It is not given memos or the history of the project.')}</li>
        <li>{t('A document, a passage or a code marked do not share is never given, and a search or a count leaves out any document with such a passage.')}</li>
        <li>{t('It can never code anything itself. With the permission to propose it can suggest codings, which wait in the review queue until you accept them.')}</li>
        <li>{t('It does not run at all while the offline lock is on, while AI is switched off here, or while the list of names to replace is applied to exports.')}</li>
      </ul>
      {closed ? <p className="muted" data-testid="assistant-closed">{closed}</p> : null}
      <fieldset className="field" disabled={closed !== null}>
        <legend>{t('What the assistant may do')}</legend>
        <div className="ac-inline">
          <label>
            <input type="radio" name="assistant-permission" checked={permission === 'read'} onChange={() => setPermission('read')} data-testid="assistant-read" /> {t('Read')}
          </label>
          <label>
            <input type="radio" name="assistant-permission" checked={permission === 'propose'} onChange={() => setPermission('propose')} data-testid="assistant-propose" /> {t('Read and propose codings')}
          </label>
        </div>
        {permission === 'propose' ? (
          <div className="field">
            <label htmlFor="assistant-actor">{t('Your name as you code in this project')}</label>
            <input id="assistant-actor" type="text" value={actor} onChange={(e) => setActor(e.target.value)} autoComplete="off" data-testid="assistant-actor" />
            <span className="help">{t('The suggestions you accept are coded by this name.')}</span>
          </div>
        ) : null}
      </fieldset>
      <label htmlFor="assistant-settings">{t('Put this in the assistant application’s settings for MCP servers')}</label>
      <textarea id="assistant-settings" readOnly rows={9} value={shown} data-testid="assistant-settings" />
      {command === null ? <p className="help">{t('The path of PrismQDA.exe is shown here in the installed program; use the full path of the program you installed.')}</p> : null}
      <p className="help">{t('Close the project in PrismQDA before the assistant starts, so the two never write to the file at once. If the project is encrypted, the assistant application must pass the passphrase in the PRISMQDA_PASSPHRASE setting, never on the command line.')}</p>
      <p className="help" data-testid="assistant-passphrase-note">{t('The passphrase in the PRISMQDA_PASSPHRASE setting is kept in the assistant application’s settings, where other programs that run as the same Windows user can read it. Connect a project with its passphrase only on a computer you control.')}</p>
      <div className="actions">
        <Button onPress={() => void copy()} isDisabled={closed !== null} data-testid="assistant-copy">{copied ? t('Copied') : t('Copy the settings')}</Button>
      </div>
    </div>
  );
}
