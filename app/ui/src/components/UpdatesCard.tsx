import { useEffect, useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, inTauri, toApiError } from '../bridge';
import { t } from '../i18n';
import { progressPercent, RELEASES_PAGE, shortNotes, type UpdateFound, type UpdateState } from '../lib/updates';
import { announce } from '../state/announce';
import { Notice } from './common';
import { Icon } from './Icon';

type Phase = 'idle' | 'checking' | 'uptodate' | 'found' | 'confirming' | 'installing' | 'failed';

/**
 * The Check for updates button (decision 189). PrismQDA makes no network connection of its own, with this one exception, which the
 * person starts: nothing is checked at start, in the background, or on a schedule. Installing is a second step that needs a go-ahead,
 * and the shell checks the download against the maintainer's signature before anything runs.
 */
export function UpdatesCard() {
  const [state, setState] = useState<UpdateState | null>(null);
  const [outside, setOutside] = useState(false);
  const [phase, setPhase] = useState<Phase>('idle');
  const [found, setFound] = useState<UpdateFound | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [percent, setPercent] = useState<number | null>(null);
  const live = useRef(true);

  useEffect(() => {
    live.current = true;
    call('update.state')
      .then((s) => live.current && setState(s))
      .catch(() => live.current && setOutside(true));
    return () => {
      live.current = false;
    };
  }, []);

  const say = (text: string) => announce(text);

  const check = async () => {
    setPhase('checking');
    setProblem(null);
    setFound(null);
    say(t('Checking for a newer version.'));
    try {
      const result = await call('update.check');
      if (!live.current) return;
      setFound(result);
      if (result.available) {
        setPhase('found');
        say(t('Version {version} is available.', { version: result.version ?? '' }));
      } else {
        setPhase('uptodate');
        say(t('This is the newest version.'));
      }
    } catch (e) {
      if (!live.current) return;
      const err = toApiError(e);
      setPhase('failed');
      const text = err.code === 'update_failed' ? t('PrismQDA could not check for updates ({detail}). Nothing was changed.', { detail: err.message }) : whyBlocked(err.code) ?? err.message;
      setProblem(text);
      say(text);
    }
  };

  const install = async () => {
    setPhase('installing');
    setPercent(null);
    setProblem(null);
    say(t('Downloading the update.'));
    let stop: (() => void) | null = null;
    try {
      if (inTauri()) {
        const { listen } = await import('@tauri-apps/api/event');
        stop = await listen<{ received: number; total: number | null }>('update-progress', (event) => setPercent(progressPercent(event.payload.received, event.payload.total)));
      }
      const result = await call('update.install');
      if (!live.current) return;
      if (result && (result as { installed?: boolean }).installed === false) {
        setPhase('uptodate');
        say(t('This is the newest version.'));
      }
    } catch (e) {
      if (!live.current) return;
      const err = toApiError(e);
      setPhase('failed');
      const text = err.code === 'update_failed' ? t('The update was not installed ({detail}). Nothing was changed.', { detail: err.message }) : whyBlocked(err.code) ?? err.message;
      setProblem(text);
      say(text);
    } finally {
      stop?.();
    }
  };

  const whyBlocked = (code: string): string | null => {
    if (code === 'not_set_up') return t('This version of PrismQDA cannot check for updates. New versions are published on the releases page.');
    if (code === 'offline_lock') return t('The offline lock is on for the open project, so PrismQDA does not connect to anything. Turn it off on the Privacy page, or close the project, to check for updates.');
    if (code === 'portable') return t('This is a portable copy, which is not updated in place. Download the new zip from the releases page and unzip it where you want it.');
    return null;
  };

  if (outside) {
    return (
      <>
        <p className="bk-set-state" data-testid="updates-outside">
          <Icon name="st-info" size={18} />
          <span>{t('Updates can only be checked in the installed program, not in this window.')}</span>
        </p>
      </>
    );
  }

  const blockedText = state?.blocked ? whyBlocked(state.blocked) : null;
  const busy = phase === 'checking' || phase === 'installing';
  const canCheck = state !== null && state.blocked === null && !busy;

  return (
    <>
      <p className="bk-set-state">
        <Icon name="st-info" size={18} />
        <span data-testid="updates-version">{t('This is version {version}.', { version: state?.version ?? '…' })}</span>
      </p>
      <p className="muted">
        {state && state.host
          ? t('PrismQDA never looks for updates by itself. When you press Check for updates, it asks one address, on {host}, whether there is a newer version, and sends only what every web request sends: the version of this program and the kind of computer. Nothing from any project is sent. A new version is installed only after you say so, and only if its signature matches the key built into this program.', { host: state.host })
          : t('PrismQDA never looks for updates by itself. Pressing Check for updates is the only time it connects to anything, and nothing from any project is sent.')}
      </p>
      {blockedText ? (
        <div data-testid="updates-blocked">
          <Notice kind="warning">
            {blockedText} <span className="muted">{RELEASES_PAGE}</span>
          </Notice>
        </div>
      ) : null}
      <div className="bk-set-actions">
        <Button onPress={() => void check()} isDisabled={!canCheck} data-testid="updates-check">
          {t('Check for updates')}
        </Button>
      </div>
      <div aria-live="polite" data-testid="updates-result">
        {phase === 'checking' ? <p>{t('Checking…')}</p> : null}
        {phase === 'uptodate' ? <p data-testid="updates-uptodate">{t('This is the newest version.')}</p> : null}
        {phase === 'found' || phase === 'confirming' ? (
          <div data-testid="updates-found">
            <p>
              <strong>{t('Version {version} is available.', { version: found?.version ?? '' })}</strong>
            </p>
            {found?.notes ? <pre className="bk-update-notes">{shortNotes(found.notes)}</pre> : null}
            {state?.portable ? <p className="muted">{whyBlocked('portable')} {RELEASES_PAGE}</p> : phase === 'found' ? (
              <Button onPress={() => setPhase('confirming')} data-testid="updates-install">
                {t('Download and install')}
              </Button>
            ) : (
              <div role="group" aria-label={t('Confirm the update')} data-testid="updates-confirm">
                <p>{t('PrismQDA will download version {version}, check its signature, close your open project (it is saved as you work), and run the installer. The program closes while it installs. Your projects are not touched.', { version: found?.version ?? '' })}</p>
                <Button onPress={() => void install()} data-testid="updates-go">{t('Yes, install it')}</Button>{' '}
                <Button onPress={() => setPhase('found')}>{t('Not now')}</Button>
              </div>
            )}
          </div>
        ) : null}
        {phase === 'installing' ? <p data-testid="updates-installing">{percent === null ? t('Downloading the update…') : t('Downloading the update… {percent} percent', { percent })}</p> : null}
        {phase === 'failed' && problem ? <div data-testid="updates-failed"><Notice kind="error">{problem}</Notice></div> : null}
      </div>
    </>
  );
}
