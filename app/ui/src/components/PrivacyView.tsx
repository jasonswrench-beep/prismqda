import { useCallback, useEffect, useMemo, useState, type FormEvent } from 'react';
import { Button, Input, Label, Radio, RadioGroup, Text, TextArea, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { defaultRuleKind, fileNameOf, groupRules, nextLabel, parseTerms, planGroupEdit, replacementProblem, ruleKindLabel, suggestionKindLabel, type DeidGroup } from '../lib/deid';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { afterNamesReplaced, goTo } from '../state/actions';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type { DeidCopyOutcome, DeidOutcome, DeidPlace, DeidPreview, DeidState, DeidSuggestion } from '../types';
import { Notice, plural } from './common';
import { OfflineAndEthics } from './OfflineEthics';
import { Tick } from './Tick';
import { num, t, tn } from '../i18n';

/** What the form at the top of the list holds: new words to find, or a change to one line of the list. */
interface Form {
  terms: string;
  replacement: string;
  kind: 'pseudonym' | 'redact';
  matchCase: boolean;
  wholeWord: boolean;
  editing: DeidGroup | null;
}

const EMPTY_FORM: Form = { terms: '', replacement: '', kind: 'pseudonym', matchCase: false, wholeWord: true, editing: null };

/**
 * Pseudonymize and redact (decisions 138 and 156). The researcher writes a list of words to find and what takes their place, and
 * chooses how to use it: for everything that leaves the project, or for the project's own text, for good.
 */
export function PrivacyView() {
  const sources = useApp((s) => s.sources);
  const projectName = useApp((s) => s.project?.name ?? '');
  const [state, setState] = useState<DeidState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState<Form>(EMPTY_FORM);
  const [formProblem, setFormProblem] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<DeidSuggestion[] | null>(null);
  const [ticked, setTicked] = useState<Set<string>>(new Set());
  const [labels, setLabels] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<DeidPreview | null>(null);
  const [keyPath, setKeyPath] = useState('');
  const [keyMessage, setKeyMessage] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [outcome, setOutcome] = useState<DeidOutcome | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [copyPath, setCopyPath] = useState('');
  const [copyOutcome, setCopyOutcome] = useState<DeidCopyOutcome | null>(null);

  const load = useCallback(async () => {
    try {
      setState(await call('deid.state', {}));
      setError(null);
    } catch (e) {
      setError(toApiError(e).message);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  const rules = state?.rules ?? [];
  const groups = useMemo(() => groupRules(rules), [rules]);
  const counts = useMemo(() => new Map((preview?.rules ?? []).map((r) => [r.id, r])), [preview]);

  /** Runs a step with the page marked busy; shows why it failed. */
  const run = async (what: string, fn: () => Promise<void>) => {
    setBusy(what);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(null);
    }
  };

  const changed = async (message: string) => {
    setPreview(null);
    setOutcome(null);
    await load();
    announce(message);
  };

  // ---- the list

  const startEdit = (g: DeidGroup) => {
    setForm({ terms: g.rules.map((r) => r.term).join('\n'), replacement: g.replacement, kind: g.kind, matchCase: g.match_case, wholeWord: g.whole_word, editing: g });
    setFormProblem(null);
    document.getElementById('privacy-terms')?.focus();
  };

  const saveForm = (e: FormEvent) => {
    e.preventDefault();
    const terms = parseTerms(form.terms);
    if (terms.length === 0) return setFormProblem(t('Write at least one word or phrase to find.'));
    const problem = replacementProblem(form.replacement);
    if (problem) return setFormProblem(problem);
    setFormProblem(null);
    void run('form', async () => {
      const replacement = form.replacement.trim();
      const common = { replacement, kind: form.kind, match_case: form.matchCase, whole_word: form.wholeWord };
      if (form.editing) {
        const plan = planGroupEdit(form.editing, { terms, ...common, kind: form.kind, match_case: form.matchCase, whole_word: form.wholeWord });
        if (plan.remove.length > 0) await call('deid.remove', { ids: plan.remove });
        for (const u of plan.update) await call('deid.update', u);
        if (plan.add.length > 0) await call('deid.add', { terms: plan.add, ...common });
        setForm(EMPTY_FORM);
        return changed(t('Saved the change to the list.'));
      }
      const r = await call('deid.add', { terms, ...common });
      setForm(EMPTY_FORM);
      const skipped = r.already_there.length;
      await changed(
        skipped > 0
          ? t('{added} added to the list. {skipped} already there.', { added: tn(r.added.length, { one: '{count} thing to find', other: '{count} things to find' }), skipped: num(skipped) })
          : t('{added} added to the list.', { added: tn(r.added.length, { one: '{count} thing to find', other: '{count} things to find' }) }),
      );
    });
  };

  const removeGroup = (g: DeidGroup) =>
    run('remove', async () => {
      await call('deid.remove', { ids: g.rules.map((r) => r.id) });
      if (form.editing?.key === g.key) setForm(EMPTY_FORM);
      await changed(t('Took {replacement} and its words out of the list.', { replacement: g.replacement }));
    });

  // ---- suggestions

  const look = () =>
    run('suggest', async () => {
      const r = await call('deid.suggest', {});
      setSuggestions(r.suggestions);
      setTicked(new Set());
      setLabels(Object.fromEntries(r.suggestions.map((s) => [s.term, s.replacement])));
      announce(r.suggestions.length === 0 ? t('Nothing found to suggest.') : tn(r.suggestions.length, { one: '{count} suggestion.', other: '{count} suggestions.' }));
    });

  const addTicked = (asOne: boolean) => {
    const chosen = (suggestions ?? []).filter((s) => ticked.has(s.term));
    if (chosen.length === 0) return setError(t('Tick the suggestions to add first.'));
    for (const s of chosen) {
      const problem = replacementProblem(labels[s.term] ?? s.replacement);
      if (problem) return setError(`${s.term}: ${problem}`);
    }
    void run('add-suggestions', async () => {
      const shared = asOne ? (labels[chosen[0].term] ?? chosen[0].replacement).trim() : null;
      // Terms that get the same label, kind, and case rule are added together.
      const batches = new Map<string, { terms: string[]; replacement: string; kind: 'pseudonym' | 'redact'; match_case: boolean }>();
      for (const s of chosen) {
        const replacement = shared ?? (labels[s.term] ?? s.replacement).trim();
        const kind = defaultRuleKind(s.kind);
        const key = JSON.stringify([replacement, kind, s.match_case]);
        const b = batches.get(key) ?? { terms: [], replacement, kind, match_case: s.match_case };
        b.terms.push(s.term);
        batches.set(key, b);
      }
      let added = 0;
      for (const b of batches.values()) {
        const r = await call('deid.add', { ...b, whole_word: !['email', 'web', 'phone', 'number'].includes(chosen.find((c) => b.terms.includes(c.term))?.kind ?? 'name') });
        added += r.added.length;
      }
      setSuggestions((suggestions ?? []).filter((s) => !ticked.has(s.term)));
      setTicked(new Set());
      await changed(tn(added, { one: '{count} thing added to the list.', other: '{count} things added to the list.' }));
    });
  };

  // ---- what the list would do

  const showPreview = () =>
    run('preview', async () => {
      const r = await call('deid.preview', {});
      setPreview(r);
      announce(tn(r.total, { one: 'The list finds {count} thing.', other: 'The list finds {count} things.' }));
    });

  // ---- the key

  const needPath = async (purpose: 'export-file' | 'key-file', suggestion: string): Promise<{ path: string; dialog: boolean } | null> => {
    const typed = cleanTypedPath(keyPath);
    if (typed !== '') return { path: typed, dialog: false };
    const picked = await browsePath(purpose, suggestion, 'csv');
    if (!picked) {
      setError(t('Type the full path of the key file, for example C:\\Private\\key.csv'));
      return null;
    }
    setKeyPath(picked);
    return { path: picked, dialog: true };
  };

  const saveKey = () =>
    run('key', async () => {
      const where = await needPath('export-file', 'key.csv');
      if (!where) return;
      const r = await call('deid.key', { path: where.path, overwrite: where.dialog });
      setKeyPath(r.path);
      setKeyMessage(t('Saved the key ({rows}) to {path}. Keep it apart from the project.', { rows: plural(r.rows, 'row'), path: r.path }));
      announce(t('Saved the key.'));
    });

  const loadKey = () =>
    run('key', async () => {
      const where = await needPath('key-file', 'key.csv');
      if (!where) return;
      const r = await call('deid.load_key', { path: where.path });
      setKeyMessage(r.added > 0 ? t('Added {count} to the list from the key file.', { count: tn(r.added, { one: '{count} thing to find', other: '{count} things to find' }) }) : t('The key file added nothing: everything in it was already in the list.'));
      await changed(t('Loaded the key file.'));
    });

  // ---- using the list

  const setExports = (on: boolean) =>
    run('exports', async () => {
      setState(await call('deid.set_exports', { on }));
      announce(on ? t('The list is now applied to everything you export.') : t('The list is no longer applied to what you export.'));
    });

  const replaceForGood = () =>
    run('apply', async () => {
      const typed = cleanTypedPath(keyPath);
      const r = await call('deid.apply', { ...(typed !== '' ? { key_path: typed } : {}) });
      setOutcome(r);
      setConfirm(false);
      setPreview(null);
      setSuggestions(null);
      await afterNamesReplaced();
      await load();
      announce(t('Replaced {count} for good. The backup was made first.', { count: tn(r.replacements, { one: '{count} name or number', other: '{count} names and numbers' }) }));
    });

  const makeCopy = () =>
    run('copy', async () => {
      let where = cleanTypedPath(copyPath);
      if (where === '') {
        const picked = await browsePath('project-copy', projectName);
        if (!picked) {
          setError(t('Type the full path of the copy, for example C:\\Shared\\study-shared.pqda'));
          return;
        }
        where = picked;
        setCopyPath(picked);
      }
      const typedKey = cleanTypedPath(keyPath);
      const r = await call('deid.copy', { path: where, ...(typedKey !== '' ? { key_path: typedKey } : {}) });
      setCopyOutcome(r);
      await load();
      announce(t('Made a copy with {count} replaced. This project was not changed.', { count: tn(r.outcome.replacements, { one: '{count} name or number', other: '{count} names and numbers' }) }));
    });

  const listSize = rules.length;
  const withNames = (state?.last_applied?.replacements ?? 0) > 0;

  return (
    <main id="main-content" tabIndex={-1} className="view-page privacy-page" aria-labelledby="h-privacy">
      <h2 id="h-privacy">{t('Pseudonymize and redact')}</h2>
      <p className="help">{t('Write down the names, places, and numbers that could point to a person, and what should stand in their place. Then choose how to use the list: for what you export, or for the project itself.')}</p>
      <p className="help">{t('A pseudonym is a made-up name or label, such as Participant 3. A redaction puts a mark, such as [phone], where the words were. Either way the words are really taken out: nothing is hidden under a black box.')}</p>
      <p className="help">{t('The list finds exactly the words you write. It does not understand sentences, so it can miss a nickname, a misspelling, or a detail such as the only dentist in town. Read the result before you share anything.')}</p>

      {error ? <Notice>{error}</Notice> : null}
      {withNames && state?.last_applied ? (
        <Notice kind="info">
          {tn(state.last_applied.replacements, {
            one: 'Names were last replaced for good on {date}: {count} replacement in {documents}.',
            other: 'Names were last replaced for good on {date}: {count} replacements in {documents}.',
          }, { date: state.last_applied.at.slice(0, 10), documents: plural(state.last_applied.documents, 'document') })}
        </Notice>
      ) : null}

      <section className="card" aria-labelledby="h-priv-list" data-testid="privacy-list">
        <h3 id="h-priv-list">{t('The list')}</h3>
        {groups.length === 0 ? (
          <p className="muted" data-testid="privacy-empty">{t('The list is empty. Add words below, or look for suggestions.')}</p>
        ) : (
          <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The list of words to find')}>
            <table className="data" data-testid="privacy-table">
              <caption className="visually-hidden">{t('Words to find, and what takes their place')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Words to find')}</th>
                  <th scope="col">{t('Takes their place')}</th>
                  <th scope="col">{t('Kind')}</th>
                  <th scope="col">{t('Found')}</th>
                  <th scope="col">{t('Change')}</th>
                </tr>
              </thead>
              <tbody>
                {groups.map((g) => {
                  const found = preview ? g.rules.reduce((n, r) => n + (counts.get(r.id)?.hits ?? 0), 0) : null;
                  return (
                    <tr key={g.key} data-testid="privacy-row">
                      <td>
                        {g.rules.map((r) => r.term).join(', ')}
                        <span className="muted">{g.match_case ? ` ${t('(capital letters matter)')}` : ''}{g.whole_word ? '' : ` ${t('(also inside other words)')}`}</span>
                      </td>
                      <th scope="row">{g.replacement}</th>
                      <td>{ruleKindLabel(g.kind)}</td>
                      <td>{found === null ? '' : num(found)}</td>
                      <td className="privacy-row-actions">
                        <Button onPress={() => startEdit(g)} aria-label={t('Change {replacement}', { replacement: g.replacement })}>{t('Change')}</Button>
                        <Button onPress={() => void removeGroup(g)} aria-label={t('Remove {replacement}', { replacement: g.replacement })} isDisabled={busy !== null}>{t('Remove')}</Button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        <form onSubmit={saveForm} noValidate className="privacy-form" aria-labelledby="h-priv-form">
          <h4 id="h-priv-form">{form.editing ? t('Change a line of the list') : t('Add to the list')}</h4>
          {formProblem ? <Notice>{formProblem}</Notice> : null}
          <TextField value={form.terms} onChange={(v) => setForm({ ...form, terms: v })} className="field">
            <Label>{t('Words to find, one on each line')}</Label>
            <TextArea rows={3} id="privacy-terms" data-testid="privacy-terms" />
            <Text slot="description">{t('Write each way the name appears, for example the full name on one line and the first name on the next.')}</Text>
          </TextField>
          <TextField value={form.replacement} onChange={(v) => setForm({ ...form, replacement: v })} className="field">
            <Label>{t('What takes their place')}</Label>
            <Input placeholder={nextLabel(rules, t('Participant '))} data-testid="privacy-replacement" />
            <Text slot="description">{t('The same label for every line above, for example Participant 3 or [city].')}</Text>
          </TextField>
          <RadioGroup value={form.kind} onChange={(v) => setForm({ ...form, kind: v as Form['kind'] })} className="field" orientation="horizontal">
            <Label>{t('Kind')}</Label>
            <Radio value="pseudonym">{t('Pseudonym (a name for a name)')}</Radio>
            <Radio value="redact">{t('Redact (a mark where the words were)')}</Radio>
          </RadioGroup>
          <div className="field">
            <Tick isSelected={form.matchCase} onChange={(on) => setForm({ ...form, matchCase: on })} testId="privacy-match-case">{t('Capital letters matter (Will is not will)')}</Tick>
            <Tick isSelected={form.wholeWord} onChange={(on) => setForm({ ...form, wholeWord: on })} testId="privacy-whole-word">{t('Whole words only (Ann is not found inside Anne)')}</Tick>
          </div>
          <div className="actions">
            <Button type="submit" className="primary" isDisabled={busy !== null} data-testid="privacy-save">{form.editing ? t('Save the change') : t('Add to the list')}</Button>
            {form.editing ? <Button type="button" onPress={() => { setForm(EMPTY_FORM); setFormProblem(null); }}>{t('Cancel the change')}</Button> : null}
          </div>
        </form>

        <div className="privacy-key">
          <TextField value={keyPath} onChange={setKeyPath} className="field">
            <Label>{t('Key file')}</Label>
            <Input placeholder="C:\Private\key.csv" data-testid="privacy-key-path" />
            <Text slot="description">{nativeDialogsAvailable ? t('The key is the list as a file: each word and what replaced it. It is plain text, not encrypted, so keep it somewhere private. Type the full path, or leave this empty to choose a place.') : t('The key is the list as a file: each word and what replaced it. It is plain text, not encrypted, so keep it somewhere private. Type the full path.')}</Text>
          </TextField>
          <div className="actions">
            <Button onPress={() => void saveKey()} isDisabled={busy !== null || listSize === 0} data-testid="privacy-save-key">{t('Save the list as a key file')}</Button>
            <Button onPress={() => void loadKey()} isDisabled={busy !== null} data-testid="privacy-load-key">{t('Add the words in a key file')}</Button>
          </div>
          {keyMessage ? <Notice kind="info">{keyMessage}</Notice> : null}
        </div>
      </section>

      <section className="card" aria-labelledby="h-priv-suggest" data-testid="privacy-suggest">
        <h3 id="h-priv-suggest">{t('Suggestions')}</h3>
        <p>{t('PrismQDA can look through your documents for words that may name someone: speaker labels, case names, email and web addresses, phone numbers, and words that begin with a capital letter in the middle of a sentence. These are guesses. Take out what is not a name, and add what it missed.')}</p>
        <div className="actions">
          <Button onPress={() => void look()} isDisabled={busy !== null || sources.length === 0} data-testid="privacy-look">{busy === 'suggest' ? t('Looking…') : t('Look for names and identifiers')}</Button>
        </div>
        {suggestions !== null ? (
          suggestions.length === 0 ? (
            <p className="muted" data-testid="privacy-no-suggestions">{t('Nothing found to suggest.')}</p>
          ) : (
            <>
              <div className="actions">
                <Button onPress={() => setTicked(new Set(suggestions.map((s) => s.term)))}>{t('Tick all')}</Button>
                <Button onPress={() => setTicked(new Set())}>{t('Tick none')}</Button>
              </div>
              <div className="table-wrap" tabIndex={0} role="region" aria-label={t('Suggested words to find')}>
                <table className="data" data-testid="privacy-suggestions">
                  <caption className="visually-hidden">{t('Suggested words to find')}</caption>
                  <thead>
                    <tr>
                      <th scope="col">{t('Add')}</th>
                      <th scope="col">{t('Words')}</th>
                      <th scope="col">{t('What it looks like')}</th>
                      <th scope="col">{t('Found')}</th>
                      <th scope="col">{t('Takes their place')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {suggestions.map((s) => (
                      <tr key={s.term}>
                        <td>
                          <Tick
                            isSelected={ticked.has(s.term)}
                            onChange={(on) => {
                              const next = new Set(ticked);
                              if (on) next.add(s.term);
                              else next.delete(s.term);
                              setTicked(next);
                            }}
                            testId="privacy-suggestion-tick"
                          >
                            <span className="visually-hidden">{t('Add {term} to the list', { term: s.term })}</span>
                          </Tick>
                        </td>
                        <th scope="row">{s.term}</th>
                        <td>{suggestionKindLabel(s.kind)}</td>
                        <td>{s.count === 0 ? t('only as a label') : tn(s.count, { one: '{count} time in {documents}', other: '{count} times in {documents}' }, { documents: plural(s.documents, 'document') })}</td>
                        <td>
                          <input
                            type="text"
                            value={labels[s.term] ?? s.replacement}
                            onChange={(e) => setLabels({ ...labels, [s.term]: e.target.value })}
                            aria-label={t('What takes the place of {term}', { term: s.term })}
                            data-testid="privacy-suggestion-label"
                          />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div className="actions">
                <Button className="primary" onPress={() => addTicked(false)} isDisabled={busy !== null} data-testid="privacy-add-ticked">{t('Add the ticked words to the list')}</Button>
                <Button onPress={() => addTicked(true)} isDisabled={busy !== null} data-testid="privacy-add-as-one">{t('Add the ticked words as one person')}</Button>
              </div>
              <p className="muted">{t('"As one person" gives every ticked word the label in the first ticked row, for a full name and the first name found alone.')}</p>
            </>
          )
        ) : null}
      </section>

      <section className="card" aria-labelledby="h-priv-preview" data-testid="privacy-preview">
        <h3 id="h-priv-preview">{t('What the list would do')}</h3>
        <p>{t('See what the list finds before you use it. Nothing is changed by looking.')}</p>
        <div className="actions">
          <Button onPress={() => void showPreview()} isDisabled={busy !== null || listSize === 0} data-testid="privacy-show-preview">{busy === 'preview' ? t('Looking…') : t('Show what the list would do')}</Button>
        </div>
        {preview ? <PreviewBody preview={preview} /> : null}
      </section>

      <section className="card" aria-labelledby="h-priv-exports" data-testid="privacy-exports">
        <h3 id="h-priv-exports">{t('For what you export')}</h3>
        <p>{t('While this is on, every Word, Excel, and CSV file, the report, and every packet you take to a chatbot has the list applied on its way out. The project itself is not changed, so nothing is lost and you can change the list at any time.')}</p>
        <Tick isSelected={state?.exports ?? false} onChange={(on) => void setExports(on)} testId="privacy-exports-switch">
          {t('Replace these names in everything I export')}
        </Tick>
        <p className="muted">{t('Not covered: pictures and PDF pages (so coded areas of them are described in words and the picture is left out), charts saved as pictures, text you copy with a Copy button (except the packet for a chatbot), and a copy of the whole project file. To share a project without the names, make a copy to share (below).')}</p>
        {state?.exports ? <Notice kind="info">{t('The list is being applied to what you export.')}</Notice> : null}
      </section>

      <section className="card" aria-labelledby="h-priv-copy" data-testid="privacy-copy">
        <h3 id="h-priv-copy">{t('Make a copy to share')}</h3>
        <p>{t('This makes a new project file with the names replaced for good, in the places that copy text as well as in the documents, and leaves this project, its list, and its backups exactly as they are. It is the way to share a project without the names.')}</p>
        <ul className="privacy-notes">
          <li>{t('The copy does not hold the list, and no backup is made beside it. Save the key (above) first if you will need to know who is who.')}</li>
          <li>{t('Pictures and PDFs cannot be changed, so they are left out of the copy, with the areas coded on them. Their text stays, with the names replaced.')}</li>
          <li>{t('Links to recordings are left out of the copy (the recordings themselves are not copied).')}</li>
          <li>{t('If this project is encrypted, so is the copy, with the same passphrase.')}</li>
        </ul>
        <TextField value={copyPath} onChange={setCopyPath} className="field">
          <Label>{t('Where to save the copy')}</Label>
          <Input placeholder={t('C:\\Shared\\study-shared.pqda')} data-testid="privacy-copy-path" />
          <Text slot="description">{nativeDialogsAvailable ? t('Type the full path, or leave this empty to choose a place. The file must not exist yet.') : t('Type the full path. The file must not exist yet.')}</Text>
        </TextField>
        <div className="actions">
          <Button className="primary" onPress={() => void makeCopy()} isDisabled={listSize === 0 || busy !== null} data-testid="privacy-make-copy">
            {busy === 'copy' ? t('Making the copy…') : t('Make a copy with the names replaced')}
          </Button>
        </div>
        {listSize === 0 ? <p className="muted">{t('Add the names to find to the list first.')}</p> : null}
        {copyOutcome ? <CopyOutcomeBody made={copyOutcome} /> : null}
      </section>

      <section className="card" aria-labelledby="h-priv-good" data-testid="privacy-good">
        <h3 id="h-priv-good">{t('Change the project for good')}</h3>
        <p>{t('This replaces the words in the text of your documents and in the places that copy text: document names, speaker labels, memos, cases and their values, the codebook, saved searches, the AI log, and the audit trail. Codings stay on the same words.')}</p>
        <ul className="privacy-notes">
          <li>{t('A backup is made first. It keeps the names, and so do the earlier backups of this project: keep them somewhere private, or delete them when you no longer need them.')}</li>
          <li>{t('The list holds the real names, so it is taken out of the project afterwards. Save it as a key file (above) if you may need to know who is who, and keep that file apart from the project.')}</li>
          <li>{t('PDFs and pictures cannot be changed, so they are left as they are and listed afterwards.')}</li>
          <li>{t('Undo cannot take this back. Only the backup can.')}</li>
        </ul>
        {cleanTypedPath(keyPath) === '' ? <Notice kind="warning">{t('No key file is chosen in the Key file box above, so after this only the backup can tell you who is who.')}</Notice> : null}
        <Tick isSelected={confirm} onChange={setConfirm} testId="privacy-confirm">
          {t('I understand that this changes the project for good.')}
        </Tick>
        <div className="actions">
          <Button className="primary" onPress={() => void replaceForGood()} isDisabled={!confirm || listSize === 0 || busy !== null} data-testid="privacy-apply">
            {busy === 'apply' ? t('Replacing…') : t('Replace the names for good')}
          </Button>
        </div>
        {outcome ? <OutcomeBody outcome={outcome} /> : null}
      </section>

      <OfflineAndEthics />

      <p className="bk-editor-foot">
        <Button className="bk-link" onPress={() => goTo('code')}>{t('Back to the coding workspace')}</Button>
      </p>
    </main>
  );
}

function Places({ places, label }: { places: DeidPlace[]; label: string }) {
  if (places.length === 0) return null;
  return (
    <ul aria-label={label} className="privacy-places">
      {places.map((p) => (
        <li key={p.place}>
          {tn(p.count, { one: '{place}: {count} time', other: '{place}: {count} times' }, { place: p.place })}
        </li>
      ))}
    </ul>
  );
}

function PreviewBody({ preview }: { preview: DeidPreview }) {
  return (
    <div data-testid="privacy-preview-body">
      <p>
        {preview.total === 0
          ? t('The list finds nothing in the text of your documents.')
          : tn(preview.total, { one: 'The list finds {count} thing in {documents}.', other: 'The list finds {count} things in {documents}.' }, { documents: plural(preview.sources.length, 'document') })}
      </p>
      {preview.sources.length > 0 ? (
        <div className="table-wrap" tabIndex={0} role="region" aria-label={t('Documents the list finds words in')}>
          <table className="data">
            <caption className="visually-hidden">{t('Documents the list finds words in')}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Document')}</th>
                <th scope="col">{t('Found')}</th>
              </tr>
            </thead>
            <tbody>
              {preview.sources.map((s) => (
                <tr key={s.id}>
                  <th scope="row">{s.name}</th>
                  <td>{num(s.hits)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {preview.samples.length > 0 ? (
        <>
          <h4>{t('Examples, before and after')}</h4>
          <ul className="privacy-samples" aria-label={t('Examples, before and after')}>
            {preview.samples.slice(0, 12).map((s, i) => (
              <li key={i}>
                <span className="muted">{s.source_name}</span>
                <span className="privacy-before"><span className="visually-hidden">{t('Before: ')}</span>{s.before}</span>
                <span className="privacy-after"><span className="visually-hidden">{t('After: ')}</span>{s.after}</span>
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {preview.skipped.length > 0 ? (
        <>
          <h4>{t('Left as they are')}</h4>
          <ul aria-label={t('Documents that cannot be changed')}>
            {preview.skipped.map((s) => (
              <li key={s.id}>
                <strong>{s.name}</strong> {s.why}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      <Places places={preview.elsewhere.filter((p) => p.covered)} label={t('Other places that would be changed for good')} />
      {preview.elsewhere.some((p) => p.covered) ? <p className="muted">{t('These other places would be changed if you change the project for good. They are not changed by the switch for exports, except where an export shows them.')}</p> : null}
      <Places places={preview.elsewhere.filter((p) => !p.covered)} label={t('Places the program cannot change')} />
      {preview.elsewhere.some((p) => !p.covered) ? <p className="muted">{t('The places above hold the words but are not changed by this page. Look at them yourself before you share the project.')}</p> : null}
    </div>
  );
}

function CopyOutcomeBody({ made }: { made: DeidCopyOutcome }) {
  const { outcome } = made;
  return (
    <div data-testid="privacy-copy-outcome" className="privacy-outcome">
      <Notice kind="info">
        {tn(outcome.replacements, { one: 'Made the copy {file}, with {count} name or number replaced in {documents}.', other: 'Made the copy {file}, with {count} names and numbers replaced in {documents}.' }, { file: fileNameOf(made.path), documents: plural(outcome.documents_changed, 'document') })}
      </Notice>
      <ul>
        <li>{t('This project was not changed, and its list is still here.')}</li>
        <li>{made.key_path ? t('The key was saved as {file} before the copy was made. Keep it apart from the copy.', { file: fileNameOf(made.key_path) }) : t('No key file was saved. The copy does not hold the list, so keep it here if you need to know who is who.')}</li>
        <li>{tn(outcome.codings_moved, { one: '{count} coding moved with its words.', other: '{count} codings moved with their words.' })}</li>
        {made.recordings_unlinked > 0 ? <li>{tn(made.recordings_unlinked, { one: '{count} link to a recording was left out of the copy.', other: '{count} links to recordings were left out of the copy.' })}</li> : null}
        {made.runs_archived > 0 ? <li>{tn(made.runs_archived, { one: '{count} agreement run in progress was archived in the copy, so its documents could be changed.', other: '{count} agreement runs in progress were archived in the copy, so their documents could be changed.' })}</li> : null}
      </ul>
      {made.taken_out.length > 0 ? (
        <>
          <h4>{t('Left out of the copy')}</h4>
          <ul aria-label={t('Pictures and PDFs left out of the copy')}>
            {made.taken_out.map((s) => (
              <li key={s.id}>
                <strong>{s.name}</strong>{' '}
                {s.kind === 'picture' ? t('A picture, left out.') : t('A PDF, left out.')}{' '}
                {s.areas_removed > 0 ? tn(s.areas_removed, { one: '{count} coded area went with it.', other: '{count} coded areas went with it.' }) : null}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      <Places places={outcome.changed} label={t('Where names were replaced in the copy')} />
      {outcome.remaining.length > 0 ? (
        <>
          <Notice kind="warning">{t('The words are still in these places in the copy. Deal with them before you share it:')}</Notice>
          <Places places={outcome.remaining} label={t('Where the words are still found in the copy')} />
        </>
      ) : (
        <p>{t('The list finds nothing left in the copy.')}</p>
      )}
    </div>
  );
}

function OutcomeBody({ outcome }: { outcome: DeidOutcome }) {
  return (
    <div data-testid="privacy-outcome" className="privacy-outcome">
      <Notice kind="info">
        {tn(outcome.replacements, { one: 'Replaced {count} name or number in {documents}.', other: 'Replaced {count} names and numbers in {documents}.' }, { documents: plural(outcome.documents_changed, 'document') })}
      </Notice>
      <ul>
        <li>{t('The backup, with the names as they were, is {path}. Keep it private.', { path: outcome.backup })}</li>
        {outcome.backup_note ? <li>{outcome.backup_note}</li> : null}
        {outcome.earlier_backups > 0 ? <li>{tn(outcome.earlier_backups, { one: '{count} earlier backup in the same folder was made before the names were replaced, so it holds the names too. Delete it when you no longer need it.', other: '{count} earlier backups in the same folder were made before the names were replaced, so they hold the names too. Delete them when you no longer need them.' })}</li> : null}
        <li>{outcome.key_path ? t('The key was saved as {file} before anything changed. Keep it apart from the project.', { file: fileNameOf(outcome.key_path) }) : t('No key file was saved, so only the backup can tell you who is who.')}</li>
        <li>{tn(outcome.codings_moved, { one: '{count} coding moved with its words.', other: '{count} codings moved with their words.' })}</li>
        {outcome.proposals_rejected > 0 ? <li>{tn(outcome.proposals_rejected, { one: '{count} waiting suggestion was turned down, because it pointed into the old text.', other: '{count} waiting suggestions were turned down, because they pointed into the old text.' })}</li> : null}
        <li>{outcome.compacted ? t('The file was rewritten, so the old text is not left in unused parts of it.') : t('The file could not be rewritten to clear unused parts of it.')}</li>
      </ul>
      <Places places={outcome.changed} label={t('Where names were replaced')} />
      {outcome.skipped.length > 0 ? (
        <>
          <h4>{t('Left as they were')}</h4>
          <ul aria-label={t('Documents that were not changed')}>
            {outcome.skipped.map((s) => (
              <li key={s.id}>
                <strong>{s.name}</strong> {s.why}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {outcome.remaining.length > 0 ? (
        <>
          <Notice kind="warning">{t('The words are still in these places. Deal with them before you share the project:')}</Notice>
          <Places places={outcome.remaining} label={t('Where the words are still found')} />
        </>
      ) : (
        <p>{t('The list finds nothing left in the project.')}</p>
      )}
    </div>
  );
}
