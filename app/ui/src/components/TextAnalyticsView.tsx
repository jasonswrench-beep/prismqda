import { useEffect, useMemo, useState } from 'react';
import { Button, Input, Label, Tab, TabList, TabPanel, Tabs, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { barWidth, contextSummary, frequencySummary, shareOf } from '../lib/textAnalytics';
import { pathLabel } from '../lib/tree';
import { openAt } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import type { WorkingSet } from '../lib/workingSet';
import type { WordContexts, WordFrequencies } from '../types';
import { Notice } from './common';
import { StaleSet } from './WorkingSet';
import { Tick } from './Tick';
import { compareText, num, t } from '../i18n';

/**
 * Text Analytics, the first part: which words the documents use, and where a word is used. A word is a run of letters or digits, written
 * in lower case; nothing is guessed and no dictionary is used, so the common words that can be left out are English only. A picture's
 * description is not counted.
 */
export function TextAnalyticsView() {
  const [tab, setTab] = useState<'words' | 'context'>('words');
  const [lookup, setLookup] = useState<{ word: string; n: number } | null>(null);

  return (
    <main id="main-content" tabIndex={-1} className="view-page text-page" aria-labelledby="h-text">
      <h2 id="h-text">{t('Text Analytics')}</h2>
      <p className="help">{t('Which words your documents use, and where a word is used. A word is a run of letters or digits, written in lower case. Nothing is guessed, and the words that can be left out as common are English ones; turn that off for another language. A picture\'s description is not counted.')}</p>
      <Tabs selectedKey={tab} onSelectionChange={(k) => setTab(k as 'words' | 'context')} className="bk-editor-tabs">
        <TabList aria-label={t('Text Analytics')} className="bk-editor-tablist">
          <Tab id="words" className="bk-tab" data-testid="text-tab-words">{t('Word frequencies')}</Tab>
          <Tab id="context" className="bk-tab" data-testid="text-tab-context">{t('Keywords in context')}</Tab>
        </TabList>
        <TabPanel id="words" className="bk-editor-panel">
          <WordsTab
            onSee={(word) => {
              setLookup({ word, n: (lookup?.n ?? 0) + 1 });
              setTab('context');
            }}
          />
        </TabPanel>
        <TabPanel id="context" className="bk-editor-panel">
          <ContextTab lookup={lookup} />
        </TabPanel>
      </Tabs>
    </main>
  );
}

function WordsTab({ onSee }: { onSee: (word: string) => void }) {
  const cases = useApp((s) => s.cases);
  const codes = useApp((s) => s.codes);
  const [caseId, setCaseId] = useState('');
  const [codeId, setCodeId] = useState('');
  const [common, setCommon] = useState(true);
  const [minLength, setMinLength] = useState('3');
  const [limit, setLimit] = useState('100');
  const [result, setResult] = useState<WordFrequencies | null>(null);
  const [ranSet, setRanSet] = useState<WorkingSet | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const activeCodes = useMemo(() => codes.filter((c) => c.status === 'active').sort((a, b) => compareText(pathLabel(codes, a.id), pathLabel(codes, b.id))), [codes]);

  const count = async () => {
    setError(null);
    setBusy(true);
    const setThen = appStore.get().workingSet;
    try {
      const r = await call('analytics.words', {
        ...(caseId ? { case_id: Number(caseId) } : {}),
        ...(codeId ? { code_ids: [Number(codeId)] } : {}),
        leave_out_common: common,
        min_length: Math.max(1, Math.floor(Number(minLength)) || 1),
        limit: Math.max(1, Math.floor(Number(limit)) || 100),
      });
      setResult(r);
      setRanSet(setThen);
      announce(frequencySummary(r));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const max = result?.words[0]?.count ?? 0;
  return (
    <section className="ac-card" aria-labelledby="h-text-words" data-testid="text-words">
      <h3 id="h-text-words" className="bk-h3">{t('Word frequencies')}</h3>
      {error ? <Notice>{error}</Notice> : null}
      <div className="field">
        <label htmlFor="text-case">{t('Documents')}</label>
        <select id="text-case" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
          <option value="">{t('Every document')}</option>
          {cases.map((c) => (
            <option key={c.id} value={c.id}>{t('The documents of the case {name}', { name: c.name })}</option>
          ))}
        </select>
      </div>
      <div className="field">
        <label htmlFor="text-code">{t('Words')}</label>
        <select id="text-code" value={codeId} onChange={(e) => setCodeId(e.target.value)}>
          <option value="">{t('Every word')}</option>
          {activeCodes.map((c) => (
            <option key={c.id} value={c.id}>{t('Only the words inside passages coded {name}', { name: pathLabel(codes, c.id) })}</option>
          ))}
        </select>
      </div>
      <Tick isSelected={common} onChange={setCommon} testId="text-common">{t('Leave out common English words (the, and, of, and so on)')}</Tick>
      <div className="team-sample">
        <TextField value={minLength} onChange={setMinLength} className="field" type="number">
          <Label>{t('Shortest word, in letters')}</Label>
          <Input min={1} />
        </TextField>
        <TextField value={limit} onChange={setLimit} className="field" type="number">
          <Label>{t('How many words to list')}</Label>
          <Input min={1} />
        </TextField>
      </div>
      <div className="actions">
        <Button className="primary" onPress={() => void count()} isDisabled={busy} data-testid="text-count">{busy ? t('Counting…') : t('Count the words')}</Button>
      </div>

      {result ? (
        <div data-testid="text-result">
          <StaleSet since={ranSet} what="results" />
          <p role="status" data-testid="text-summary">{frequencySummary(result)}</p>
          {result.words.length > 0 ? (
            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="text-table">
                <caption>{t('The commonest words')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Word')}</th>
                    <th scope="col">{t('Times used')}</th>
                    <th scope="col">{t('Share')}</th>
                    <th scope="col">{t('In documents')}</th>
                    <th scope="col"><span className="visually-hidden">{t('Bar')}</span></th>
                    <th scope="col"><span className="visually-hidden">{t('Where it is used')}</span></th>
                  </tr>
                </thead>
                <tbody>
                  {result.words.map((w) => (
                    <tr key={w.word}>
                      <th scope="row">{w.word}</th>
                      <td>{num(w.count)}</td>
                      <td>{shareOf(w.count, result.counted_words)}</td>
                      <td>{num(w.documents)}</td>
                      <td aria-hidden="true">
                        <span className="text-bar" style={{ width: `${barWidth(w.count, max)}%` }} />
                      </td>
                      <td>
                        <Button onPress={() => onSee(w.word)} aria-label={t('See where {word} is used', { word: w.word })}>{t('See where')}</Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

function ContextTab({ lookup }: { lookup: { word: string; n: number } | null }) {
  const cases = useApp((s) => s.cases);
  const [query, setQuery] = useState('');
  const [caseId, setCaseId] = useState('');
  const [around, setAround] = useState('6');
  const [result, setResult] = useState<WordContexts | null>(null);
  const [ranSet, setRanSet] = useState<WorkingSet | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const search = async (wanted?: string) => {
    const q = (wanted ?? query).trim();
    setError(null);
    if (q === '') return setError(t('Type a word, or the words of a phrase, to look for.'));
    setBusy(true);
    const setThen = appStore.get().workingSet;
    try {
      const r = await call('analytics.kwic', { query: q, ...(caseId ? { case_id: Number(caseId) } : {}), words_around: Math.max(0, Math.floor(Number(around)) || 0) });
      setResult(r);
      setRanSet(setThen);
      announce(contextSummary(r));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  // A word chosen on the frequencies is looked up here as soon as this tab is shown.
  useEffect(() => {
    if (lookup) {
      setQuery(lookup.word);
      void search(lookup.word);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lookup?.n]);

  return (
    <section className="ac-card" aria-labelledby="h-text-context" data-testid="text-context">
      <h3 id="h-text-context" className="bk-h3">{t('Keywords in context')}</h3>
      {error ? <Notice>{error}</Notice> : null}
      <TextField value={query} onChange={setQuery} className="field">
        <Label>{t('A word, or the words of a phrase')}</Label>
        <Input data-testid="text-query" />
      </TextField>
      <div className="team-sample">
        <div className="field">
          <label htmlFor="kwic-case">{t('Documents')}</label>
          <select id="kwic-case" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
            <option value="">{t('Every document')}</option>
            {cases.map((c) => (
              <option key={c.id} value={c.id}>{t('The documents of the case {name}', { name: c.name })}</option>
            ))}
          </select>
        </div>
        <TextField value={around} onChange={setAround} className="field" type="number">
          <Label>{t('Words to show on each side')}</Label>
          <Input min={0} />
        </TextField>
      </div>
      <div className="actions">
        <Button className="primary" onPress={() => void search()} isDisabled={busy} data-testid="text-search">{busy ? t('Looking…') : t('Find it')}</Button>
      </div>
      {result ? (
        <div data-testid="kwic-result">
          <StaleSet since={ranSet} what="results" />
          <p role="status" data-testid="kwic-summary">{contextSummary(result)}</p>
          {result.items.length > 0 ? (
            <div className="table-wrap" tabIndex={-1}>
              <table className="data text-kwic" data-testid="kwic-table">
                <caption>{t('Where the word is used')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Document')}</th>
                    <th scope="col">{t('Before')}</th>
                    <th scope="col">{t('The word')}</th>
                    <th scope="col">{t('After')}</th>
                    <th scope="col"><span className="visually-hidden">{t('Open')}</span></th>
                  </tr>
                </thead>
                <tbody>
                  {result.items.map((c) => (
                    <tr key={`${c.source_id}-${c.start}`}>
                      <th scope="row">{c.source_name}</th>
                      <td className="text-before">{c.left}</td>
                      <td><strong>{c.hit}</strong></td>
                      <td>{c.right}</td>
                      <td>
                        <Button onPress={() => void openAt(c.source_id, c.start, c.end)} aria-label={t('Open {source_name} at this word', { source_name: c.source_name })}>{t('Open')}</Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
