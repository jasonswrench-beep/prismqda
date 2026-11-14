import type { ReactNode } from 'react';
import { I18nProvider } from 'react-aria-components';
import { getLocaleState, useLocale } from './index';

/**
 * Tells the component library which language is in use, so the few words it supplies itself (the label of a dismiss button,
 * "0 options available") and the way it writes numbers follow the language of the window. The screens are drawn again by the
 * App component when the language changes (see App.tsx).
 */
export function LocaleBoundary({ children }: { children: ReactNode }) {
  useLocale();
  return <I18nProvider locale={getLocaleState().tag}>{children}</I18nProvider>;
}
