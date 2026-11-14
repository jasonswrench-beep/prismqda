import { useMemo, useState, type FormEvent } from 'react';
import { Button, Checkbox, Dialog, Heading, Input, Label, Modal, ModalOverlay, Radio, RadioGroup, Text, TextField } from 'react-aria-components';
import { toApiError } from '../bridge';
import { PackError } from '../i18n/pack';
import type { MadeBy } from '../i18n/pack';
import { languageEntries, nativeName } from '../i18n/languages';
import { installedInfo } from '../i18n/installed';
import { installTranslation, readTranslation, translatorFile, writeFile, type ReadResult } from '../i18n/manage';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { setLanguage } from '../state/actions';
import { announce } from '../state/announce';
import { appStore } from '../state/app';
import { Notice } from './common';
import { num, t, tn } from '../i18n';

const close = () => appStore.set({ dialog: null });

/** The standard list as the options of a choice: the name in the language itself, then in English. */
function LanguageOptions() {
  const entries = useMemo(() => languageEntries(), []);
  return (
    <>
      {entries.map((l) => (
        <option key={l.code} value={l.code} lang={l.code}>
          {l.native === l.english ? l.english : `${l.native} (${l.english})`}
        </option>
      ))}
    </>
  );
}

const MADE_BY: { id: MadeBy; label: () => string; help: () => string }[] = [
  { id: 'person', label: () => t('A person translated it'), help: () => t('The words were written, or written and checked, by someone who speaks the language.') },
  { id: 'reviewed', label: () => t('A person reviewed a machine translation'), help: () => t('A program made a first draft and a person read and corrected all of it.') },
  { id: 'machine', label: () => t('A machine translated it and nobody has checked it'), help: () => t('Say so, so that the people using it know. Mistakes are likely.') },
];

/**
 * Add a translation: a spreadsheet saved as CSV, or a JSON file, that someone made. It is read and checked first, and what
 * was found is told before anything is kept: how much it translates, and which entries were left out and why. Each translation
 * is given a label, who made it, and how.
 */
export function AddLanguageDialog() {
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [read, setRead] = useState<ReadResult | null>(null);
  const [code, setCode] = useState('');
  const [label, setLabel] = useState('');
  const [labelTouched, setLabelTouched] = useState(false);
  const [credit, setCredit] = useState('');
  const [madeBy, setMadeBy] = useState<MadeBy>('person');
  const [replace, setReplace] = useState('');
  const [useNow, setUseNow] = useState(true);

  const same = code ? installedInfo().filter((p) => p.code === code) : [];
  const keepBoth = replace === 'keep';
  const replaceId = keepBoth ? '' : same.some((p) => p.id === replace) ? replace : (same[0]?.id ?? '');

  const chooseCode = (next: string) => {
    setCode(next);
    if (!labelTouched) setLabel(next ? nativeName(next) : '');
    const first = installedInfo().find((p) => p.code === next);
    setReplace(first?.id ?? '');
  };

  const readFile = async (typed?: string) => {
    setError(null);
    setRead(null);
    const target = cleanTypedPath(typed ?? path);
    if (target === '') {
      setError(t('Type the full path of the translation file, or press Browse.'));
      return;
    }
    setBusy(true);
    try {
      const r = await readTranslation(target);
      setRead(r);
      const suggested = r.suggestedCode ?? '';
      chooseCode(suggested);
      if (r.draft.label && !labelTouched) setLabel(r.draft.label);
      if (r.draft.credit) setCredit(r.draft.credit);
      if (r.draft.madeBy) setMadeBy(r.draft.madeBy);
      announce(t('Read {file}: {done} of {total} messages translated.', { file: r.fileName, done: num(r.checked.done), total: num(r.checked.total) }));
    } catch (e) {
      setError(e instanceof PackError ? e.message : toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const add = (e: FormEvent) => {
    e.preventDefault();
    if (!read) return;
    setError(null);
    if (!code) return setError(t('Choose the language this translation is for.'));
    if (read.checked.done === 0) return setError(t('Nothing in this file could be used, so there is nothing to add.'));
    const added = installTranslation(read.checked, { code, label, credit, madeBy, replace: replaceId || undefined });
    if (!added.ok) return setError(added.reason);
    announce(t('Added the translation {label}: {done} of {total} messages.', { label: added.info.label, done: num(added.info.done), total: num(added.info.total) }));
    close();
    if (useNow) void setLanguage(added.info.id);
  };

  const checked = read?.checked ?? null;
  const percent = checked && checked.total > 0 ? Math.round((100 * checked.done) / checked.total) : 0;

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && close()} className="overlay">
      <Modal className="modal bk-wide-modal">
        <Dialog className="dialog" aria-labelledby="al-title">
          <Heading slot="title" id="al-title" className="dialog-title">{t('Add a translation')}</Heading>
          <p className="muted">{t('A translation is a file someone made with the words of this window in another language. PrismQDA does not translate; it reads the file, checks it, and shows the window in that language. Only add files from people you trust.')}</p>
          {error ? <Notice>{error}</Notice> : null}
          <form onSubmit={(e) => { e.preventDefault(); void readFile(); }} noValidate>
            <TextField value={path} onChange={(v) => { setPath(v); setRead(null); }} className="field" autoFocus>
              <Label>{t('Translation file')}</Label>
              <div className="bk-location-row">
                <Input placeholder={t('C:\\Translations\\fr.csv')} />
                {nativeDialogsAvailable ? (
                  <Button
                    type="button"
                    aria-label={t('Browse for a translation file')}
                    isDisabled={busy}
                    onPress={async () => {
                      const chosen = await browsePath('language-file');
                      if (chosen) {
                        setPath(chosen);
                        void readFile(chosen);
                      }
                    }}
                  >{t('Browse…')}</Button>
                ) : null}
              </div>
              <Text slot="description">{t('Type the full path of a spreadsheet saved as CSV, or of a JSON file.')}</Text>
            </TextField>
            <div className="actions">
              <Button type="submit" isDisabled={busy}>{busy ? t('Reading…') : t('Read this file')}</Button>
            </div>
          </form>

          {read && checked ? (
            <form onSubmit={add} noValidate>
              <div role="status" className="bk-translation-report" data-testid="translation-report">
                <p>
                  <strong>{read.fileName}</strong>: {t('{done} of {total} messages are translated ({percent}%).', { done: num(checked.done), total: num(checked.total), percent })}
                </p>
                {read.encoding === 'windows-1252' ? (
                  <Notice kind="warning">{t('This file was not saved as UTF-8, so letters with accents or in other scripts may look wrong. In Excel, save it again as "CSV UTF-8".')}</Notice>
                ) : null}
                {checked.unknown > 0 ? <p className="muted">{tn(checked.unknown, { one: '{count} entry is for a message this version of PrismQDA does not have, and was skipped.', other: '{count} entries are for messages this version of PrismQDA does not have, and were skipped.' })}</p> : null}
                {read.draft.unreadable > 0 ? <p className="muted">{tn(read.draft.unreadable, { one: '{count} row could not be read, and was skipped.', other: '{count} rows could not be read, and were skipped.' })}</p> : null}
                {checked.problems.length > 0 ? (
                  <div data-testid="translation-problems">
                    <p>{tn(checked.problems.length, { one: '{count} entry was left out, so the English shows for it:', other: '{count} entries were left out, so the English shows for them:' })}</p>
                    <ul>
                      {checked.problems.slice(0, 8).map((p, i) => (
                        <li key={i}>
                          <q>{p.message}</q> {p.what}
                        </li>
                      ))}
                    </ul>
                    {checked.problems.length > 8 ? <p className="muted">{t('and {count} more.', { count: num(checked.problems.length - 8) })}</p> : null}
                  </div>
                ) : null}
              </div>

              <div className="field">
                <label htmlFor="al-language">{t('Language')}</label>
                <select id="al-language" value={code} onChange={(e) => chooseCode(e.target.value)} data-testid="al-language">
                  <option value="">{t('Choose a language…')}</option>
                  <LanguageOptions />
                </select>
              </div>
              <TextField value={label} onChange={(v) => { setLabel(v); setLabelTouched(true); }} className="field">
                <Label>{t('What to call it')}</Label>
                <Input />
                <Text slot="description">{t('Shown in the list of languages, for example "Français" or "Español (Mexico), checked by the class of 2026".')}</Text>
              </TextField>
              <TextField value={credit} onChange={setCredit} className="field">
                <Label>{t('Translated by')}</Label>
                <Input />
                <Text slot="description">{t('A name or a group, so that people know who to ask about a word. Optional.')}</Text>
              </TextField>
              <RadioGroup value={madeBy} onChange={(v) => setMadeBy(v as MadeBy)} className="field" data-testid="al-made-by">
                <Label>{t('How it was made')}</Label>
                {MADE_BY.map((m) => (
                  <Radio key={m.id} value={m.id}>
                    <span className="radio-text">
                      <span className="radio-title">{m.label()}</span>
                      <span className="radio-help">{m.help()}</span>
                    </span>
                  </Radio>
                ))}
              </RadioGroup>
              {same.length > 0 ? (
                <div className="field">
                  <label htmlFor="al-replace">{t('A translation of this language is already here')}</label>
                  <select id="al-replace" value={keepBoth ? 'keep' : replaceId} onChange={(e) => setReplace(e.target.value)} data-testid="al-replace">
                    {same.map((p) => (
                      <option key={p.id} value={p.id}>
                        {t('Replace {label}', { label: p.label })}
                      </option>
                    ))}
                    <option value="keep">{t('Keep both')}</option>
                  </select>
                </div>
              ) : null}
              <Checkbox isSelected={useNow} onChange={setUseNow} className="check">
                <div className="box" aria-hidden="true">
                  <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                    <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                  </svg>
                </div>
                {t('Show the window in it now')}
              </Checkbox>
              <div className="actions">
                <Button type="button" onPress={close}>{t('Cancel')}</Button>
                <Button type="submit" className="primary" isDisabled={checked.done === 0 || !code} data-testid="al-add">{t('Add this translation')}</Button>
              </div>
            </form>
          ) : (
            <div className="actions">
              <Button type="button" onPress={close}>{t('Cancel')}</Button>
            </div>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/**
 * Save a file for a translator: a spreadsheet (or JSON) with the words of the window, the English in one column and a column to
 * fill in. For a language that already has a translation here, it can hold what is translated, or only what is still missing.
 * Also the way to keep a copy of a translation that was added.
 */
export function TranslatorFileDialog(props: { id?: string }) {
  const [only, setOnly] = useState(false);
  const have = props.id ? installedInfo().find((p) => p.id === props.id) : undefined;
  const [code, setCode] = useState(have?.code ?? '');
  const [format, setFormat] = useState<'csv' | 'json'>('csv');
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<{ path: string; rows: number } | null>(null);
  // a path chosen in Windows' own save dialog, which has already asked about a file that is there
  const [pickedPath, setPickedPath] = useState('');

  const existing = have ?? installedInfo().find((p) => p.code === code);

  const save = async (typed?: string, fromDialog = false) => {
    setError(null);
    if (!code) return setError(t('Choose the language the file is for.'));
    let target = cleanTypedPath(typed ?? path);
    if (target === '') {
      const picked = await browsePath('export-file', `${code}.${format}`, format);
      if (!picked) return setError(t('Type the full path of the file to write, for example C:\\Translations\\fr.csv'));
      target = picked;
      fromDialog = true;
      setPath(picked);
      setPickedPath(picked);
    }
    if (target === pickedPath) fromDialog = true;
    setBusy(true);
    try {
      const made = await translatorFile(format, code, only && !!existing, existing?.id);
      const where = await writeFile(target, made.text, fromDialog);
      setSaved({ path: where, rows: made.rows });
      announce(t('Saved {path}.', { path: where }));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void save();
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && close()} className="overlay">
      <Modal className="modal bk-wide-modal">
        <Dialog className="dialog" aria-labelledby="tf-title">
          <Heading slot="title" id="tf-title" className="dialog-title">
            {have ? t('Save a copy of {label}', { label: have.label }) : t('Save a file for a translator')}
          </Heading>
          <p className="muted">
            {have
              ? t('A copy keeps this translation outside PrismQDA, to give to someone or to correct in a spreadsheet and add again.')
              : t('The file lists the words of the window, the English in one column and an empty column to fill in. Give it to someone who speaks the language, and add the file they send back with Add a translation. PrismQDA does not translate.')}
          </p>
          {error ? <Notice>{error}</Notice> : null}
          {saved ? (
            <div role="status" data-testid="translator-file-saved">
              <Notice kind="info">{t('Saved {path} with {rows} lines.', { path: saved.path, rows: num(saved.rows) })}</Notice>
              <div className="actions">
                <Button className="primary" onPress={close}>{t('Done')}</Button>
              </div>
            </div>
          ) : (
            <form onSubmit={submit} noValidate>
              <div className="field">
                <label htmlFor="tf-language">{t('Language')}</label>
                <select id="tf-language" value={code} disabled={!!have} onChange={(e) => setCode(e.target.value)} data-testid="tf-language">
                  <option value="">{t('Choose a language…')}</option>
                  <LanguageOptions />
                </select>
              </div>
              <RadioGroup value={format} onChange={(v) => setFormat(v as 'csv' | 'json')} className="field">
                <Label>{t('Kind of file')}</Label>
                <Radio value="csv">
                  <span className="radio-text">
                    <span className="radio-title">{t('A spreadsheet (CSV)')}</span>
                    <span className="radio-help">{t('Opens in Excel and Google Sheets. Save it again as "CSV UTF-8" when the translation is done.')}</span>
                  </span>
                </Radio>
                <Radio value="json">
                  <span className="radio-text">
                    <span className="radio-title">{t('A JSON file')}</span>
                    <span className="radio-help">{t('For a person who works in a text editor or a translation program.')}</span>
                  </span>
                </Radio>
              </RadioGroup>
              {existing && !have ? (
                <Checkbox isSelected={only} onChange={setOnly} className="check">
                  <div className="box" aria-hidden="true">
                    <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                      <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                    </svg>
                  </div>
                  {t('Only the messages {label} does not translate yet', { label: existing.label })}
                </Checkbox>
              ) : null}
              {have ? (
                <Checkbox isSelected={only} onChange={setOnly} className="check">
                  <div className="box" aria-hidden="true">
                    <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                      <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                    </svg>
                  </div>
                  {t('Only the messages not translated yet')}
                </Checkbox>
              ) : null}
              <TextField value={path} onChange={setPath} className="field">
                <Label>{t('Where to save it')}</Label>
                <div className="bk-location-row">
                  <Input placeholder={t('C:\\Translations\\fr.csv')} />
                  {nativeDialogsAvailable ? (
                    <Button
                      type="button"
                      aria-label={t('Browse for where to save the file')}
                      isDisabled={busy}
                      onPress={async () => {
                        const picked = await browsePath('export-file', code ? `${code}.${format}` : '', format);
                        if (picked) {
                          setPath(picked);
                          setPickedPath(picked);
                        }
                      }}
                    >{t('Browse…')}</Button>
                  ) : null}
                </div>
                <Text slot="description">{t('Type the full path. A file that is already there is not replaced.')}</Text>
              </TextField>
              <div className="actions">
                <Button type="button" onPress={close}>{t('Cancel')}</Button>
                <Button type="submit" className="primary" isDisabled={busy} data-testid="tf-save">{busy ? t('Saving…') : t('Save the file')}</Button>
              </div>
            </form>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
