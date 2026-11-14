import { useState } from 'react';
import { Button } from 'react-aria-components';
import { installedInfo } from '../i18n/installed';
import { englishName } from '../i18n/languages';
import { languagesStore } from '../i18n/manage';
import { listLanguages } from '../i18n/locales';
import type { MadeBy, PackInfo } from '../i18n/pack';
import { removeLanguage, setLanguage } from '../state/actions';
import { appStore, useApp } from '../state/app';
import { useStore } from '../state/store';
import { num, t } from '../i18n';

const madeByWords = (m: MadeBy): string => (m === 'person' ? t('translated by a person') : m === 'reviewed' ? t('a machine translation reviewed by a person') : t('a machine translation, not checked'));

/** What is said under the language choice: the choice, the translations that have been added, and how to add one. */
export function LanguageBody() {
  const settings = useApp((s) => s.settings);
  useStore(languagesStore, (s) => s.version);
  const [confirming, setConfirming] = useState<string | null>(null);
  const choices = listLanguages();
  const added = installedInfo();
  const several = choices.length > 1;
  // a choice that names a translation that is not here any more is shown as following the computer
  const chosen = settings.language === 'auto' || choices.some((l) => l.id === settings.language) ? settings.language : 'auto';

  return (
    <>
      <div className="field">
        <label htmlFor="set-language">{t('Interface language')}</label>
        <select id="set-language" disabled={!several} value={!several ? 'en' : chosen} onChange={(e) => void setLanguage(e.target.value)} aria-describedby="set-language-help">
          {!several ? (
            <option value="en">{t('English (US)')}</option>
          ) : (
            <>
              <option value="auto">{t('Same as Windows')}</option>
              {choices.map((l) => (
                <option key={l.id} value={l.id} lang={l.code}>
                  {l.label}
                </option>
              ))}
            </>
          )}
        </select>
      </div>
      <p className="muted" id="set-language-help">
        {several ? t('Words PrismQDA has no translation for are shown in English. The text you code can be in any language.') : t('PrismQDA is written in English. Add a translation file to see the window in another language. The text you code can be in any language.')}
      </p>

      <h4 id="h-translations" className="bk-h4">{t('Translations on this computer')}</h4>
      {added.length === 0 ? (
        <p className="muted" data-testid="no-translations">{t('None have been added. PrismQDA does not translate; a translation is a file someone made, which you add here.')}</p>
      ) : (
        <ul className="bk-translation-list" aria-labelledby="h-translations" data-testid="translations-list">
          {added.map((p) => (
            <TranslationRow key={p.id} p={p} confirming={confirming === p.id} ask={() => setConfirming(p.id)} cancel={() => setConfirming(null)} remove={() => { setConfirming(null); void removeLanguage(p.id); }} />
          ))}
        </ul>
      )}
      <div className="actions">
        <Button onPress={() => appStore.set({ dialog: { kind: 'add-language' } })} data-testid="add-translation">{t('Add a translation…')}</Button>
        <Button onPress={() => appStore.set({ dialog: { kind: 'translator-file' } })} data-testid="translator-file">{t('Save a file for a translator…')}</Button>
      </div>
    </>
  );
}

function TranslationRow(props: { p: PackInfo; confirming: boolean; ask: () => void; cancel: () => void; remove: () => void }) {
  const { p } = props;
  const percent = p.total > 0 ? Math.round((100 * p.done) / p.total) : 0;
  return (
    <li data-testid={`translation-${p.id}`}>
      <div className="bk-translation-head">
        <strong lang={p.code}>{p.label}</strong> <span className="muted">{englishName(p.code)}</span>
      </div>
      <div className="muted">
        {t('{done} of {total} messages translated ({percent}%)', { done: num(p.done), total: num(p.total), percent })}; {madeByWords(p.madeBy)}
        {p.credit ? `; ${t('by {credit}', { credit: p.credit })}` : ''}
        {p.added ? `; ${t('added {date}', { date: p.added })}` : ''}
      </div>
      <div className="actions">
        {props.confirming ? (
          <>
            <span role="alert">{t('Remove {label} from this computer?', { label: p.label })}</span>
            <Button onPress={props.remove} data-testid={`remove-yes-${p.id}`}>{t('Yes, remove it')}</Button>
            <Button onPress={props.cancel}>{t('Keep it')}</Button>
          </>
        ) : (
          <>
            <Button aria-label={t('Save a copy of {label}', { label: p.label })} onPress={() => appStore.set({ dialog: { kind: 'translator-file', id: p.id } })}>{t('Save a copy…')}</Button>
            <Button aria-label={t('Remove {label}', { label: p.label })} onPress={props.ask} data-testid={`remove-${p.id}`}>{t('Remove')}</Button>
          </>
        )}
      </div>
    </li>
  );
}
