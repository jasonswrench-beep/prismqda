import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { inTauri } from '../bridge';
import { initApp } from '../state/actions';
import { useApp } from '../state/app';
import { BrandMark } from './BrandMark';
import { rich, t } from '../i18n';

/** How long startup may take before the splash says it is working. A quick start never shows the words. */
const STATUS_AFTER_MS = 800;

/**
 * The splash (screen specification book, Splash Screen): the prism, the name, what PrismQDA is, and a spectrum along the
 * lower edge. It is on screen only while the program starts, and the launcher replaces it the moment it can take input;
 * nothing here waits. If starting takes long, a short status appears with an indeterminate bar (still when the person
 * prefers reduced motion). If it fails, the splash becomes a message that names what failed, with Retry and Close.
 */
export function Splash() {
  const phase = useApp((s) => s.phase);
  const error = useApp((s) => s.bootError);
  const [slow, setSlow] = useState(false);
  const [retrying, setRetrying] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => setSlow(true), STATUS_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, []);

  const blocked = phase === 'blocked';
  const close = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().close();
    } catch {
      window.close();
    }
  };

  return (
    <main id="main-content" tabIndex={-1} className="bk-splash" aria-busy={!blocked} aria-labelledby="splash-title">
      <div className="bk-splash-body">
        <BrandMark size={150} className="bk-splash-mark" />
        <h1 id="splash-title" className="bk-splash-name">
          {rich('<span>Prism</span><span2>QDA</span2>', { span: (inside) => <span className="p">{inside}</span>, span2: (inside) => <span className="q">{inside}</span> })}
        </h1>
        <p className="bk-splash-line">{t('Qualitative • Quantitative • Mixed Methods')}</p>
        <p className="bk-splash-sub">{t('A complete research platform for coding, analysis, statistics, and synthesis.')}</p>
        {blocked ? (
          <div className="bk-splash-error" role="alert">
            <p>
              {rich('<strong>PrismQDA could not start.</strong> The step that failed: reaching the engine that keeps your projects.', { strong: (inside) => <strong>{inside}</strong> })}</p>
            <details>
              <summary>{t('Details')}</summary>
              <p className="bk-detail">{error ?? t('No further detail was given.')}</p>
            </details>
            <div className="actions">
              <Button
                className="primary"
                isDisabled={retrying}
                autoFocus
                onPress={async () => {
                  setRetrying(true);
                  await initApp();
                  setRetrying(false);
                }}
              >{t('Retry')}</Button>
              {inTauri() ? <Button onPress={() => void close()}>{t('Close')}</Button> : null}
            </div>
            <p className="bk-reassure">{t('Your project files were not touched.')}</p>
          </div>
        ) : slow ? (
          <p className="bk-splash-status" role="status">{t('Starting PrismQDA')}<span className="bk-bar" aria-hidden="true" />
          </p>
        ) : null}
      </div>
      <svg className="bk-spectrum" viewBox="0 0 1200 220" preserveAspectRatio="none" aria-hidden="true" focusable="false">
        <defs>
          <linearGradient id="sp1" x1="0" x2="1">
            <stop offset="0" stopColor="#13d8de" />
            <stop offset="0.35" stopColor="#2f7bff" />
            <stop offset="0.65" stopColor="#a855f7" />
            <stop offset="1" stopColor="#ff9645" />
          </linearGradient>
          <linearGradient id="sp2" x1="0" x2="1">
            <stop offset="0" stopColor="#4ff8e9" stopOpacity="0.15" />
            <stop offset="0.5" stopColor="#7d6bfa" stopOpacity="0.55" />
            <stop offset="1" stopColor="#ffe34b" stopOpacity="0.35" />
          </linearGradient>
        </defs>
        <path d="M0 190 C 250 120, 420 210, 650 150 S 1020 60, 1200 120 L1200 220 L0 220Z" fill="url(#sp2)" />
        <path d="M0 205 C 300 150, 460 215, 700 175 S 1040 120, 1200 165 L1200 220 L0 220Z" fill="url(#sp1)" opacity="0.85" />
      </svg>
    </main>
  );
}
