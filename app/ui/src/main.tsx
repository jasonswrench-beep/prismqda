import { Component, StrictMode, type ErrorInfo, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { LocaleBoundary } from './i18n/LocaleBoundary';
import * as actions from './state/actions';
import './state/workingSet';
import { activateLocale, chooseLocale } from './i18n/locales';
import { applySettings } from './lib/settings';
import { appStore } from './state/app';
import './styles.css';
import './book.css';
import { t } from './i18n';

// The person's theme and text size are on the page before anything is drawn, so nothing flashes.
applySettings(appStore.get().settings, document.documentElement);

/** If something unexpected breaks a screen, say so in plain words instead of leaving a blank page. */
class Fallback extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('The interface stopped:', error, info.componentStack);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <main className="start" role="alert">
        <h1>{t('PrismQDA hit a problem')}</h1>
        <p>{t('Your project file is safe: every change is saved as you make it. Reload the window to continue.')}</p>
        <p className="muted">{this.state.error.message}</p>
        <button type="button" onClick={() => window.location.reload()}>{t('Reload')}</button>
      </main>
    );
  }
}

const root = document.getElementById('root');
if (!root) throw new Error('The page has no #root element.');

async function start(container: HTMLElement) {
  // The words of the window are in the person's language before anything is drawn. A page address that ends in ?lang=en-XA
  // asks for the test language, in which every word that is in the catalog is bent and framed (see docs/LOCALIZATION.md).
  const asked = new URLSearchParams(window.location.search).get('lang');
  const choice = asked ?? appStore.get().settings.language;
  await activateLocale(chooseLocale(choice, navigator.languages ?? []), document.documentElement);
  // For the browser check only: with ?lang= in the address, a script can move the window between screens without looking for
  // buttons by their words (which the test language has changed).
  if (asked !== null) (window as unknown as { __prismqda?: unknown }).__prismqda = { appStore, actions };
  createRoot(container).render(
    <StrictMode>
      <LocaleBoundary>
        <Fallback>
          <App />
        </Fallback>
      </LocaleBoundary>
    </StrictMode>,
  );
}
void start(root);
