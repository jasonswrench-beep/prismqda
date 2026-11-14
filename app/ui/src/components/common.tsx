import type { ReactNode } from 'react';
import type { IconName } from '../assets/icons.generated';
import { num, t } from '../i18n';

/** A message that screen readers read out as soon as it appears (errors and warnings). */
export function Notice(props: { children: ReactNode; kind?: 'error' | 'warning' | 'info' }) {
  const kind = props.kind ?? 'error';
  // Errors interrupt; the rest wait their turn.
  return (
    <div role={kind === 'info' ? 'status' : 'alert'} className={`notice notice-${kind}`}>
      <span className="notice-kind">{kind === 'error' ? t('Problem: ') : kind === 'warning' ? t('Note: ') : ''}</span>
      {props.children}
    </div>
  );
}

/** Pluralizes a count: plural(3, 'unit') is "3 units". */
export function plural(n: number, one: string, many = one + 's'): string {
  return `${num(n)} ${n === 1 ? one : many}`;
}

export const SOURCE_TYPE_LABELS: Record<string, string> = {
  get document() { return t('Document'); },
  get pdf() { return t('PDF'); },
  get survey() { return t('Survey responses'); },
  get transcript() { return t('Transcript'); },
  get social() { return t('Posts and comments'); },
  get other() { return t('Other'); },
};

/** The icon for each kind of source that has a fitting one; the others show words only. */
export const SOURCE_TYPE_ICONS: Partial<Record<string, IconName>> = {
  document: 'src-text',
  pdf: 'src-pdf',
  survey: 'src-survey',
  transcript: 'src-transcript',
  social: 'src-social',
};

export const UNIT_KIND_LABELS: Record<string, string> = {
  get sentence() { return t('sentence'); },
  get paragraph() { return t('paragraph'); },
  get speaker_turn() { return t('speaker turn'); },
  get survey_response() { return t('response'); },
  get post() { return t('post'); },
  get comment() { return t('comment'); },
};
