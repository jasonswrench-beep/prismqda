import { useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { openAt } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import type { WorkingSet } from '../../lib/workingSet';
import type { SentimentCounts, SentimentReport } from '../../types';
import { Notice } from '../common';
import { StaleSet } from '../WorkingSet';
import { Tick } from '../Tick';
import { num, t, tn } from '../../i18n';

/** The words a person typed, one to a line or separated by commas, as a list. */
export function wordList(text: string): string[] {
  return text.split(/[\s,;]+/).map((w) => w.trim()).filter((w) => w !== '');
}

/** A kind's share of the passages, as the language writes a percentage. */
function share(count: number, of: number): string {
  return num(of > 0 ? count / of : 0, { style: 'percent', maximumFractionDigits: 0 });
}

function CountsRow({ name, counts, header }: { name: string; counts: SentimentCounts; header?: boolean }) {
  const total = counts.positive + counts.negative + counts.mixed + counts.neutral;
  return (
    <tr data-testid="sentiment-row">
      {header ? <th scope="row">{name}</th> : <td>{name}</td>}
      <td>{num(counts.positive)} ({share(counts.positive, total)})</td>
      <td>{num(counts.negative)} ({share(counts.negative, total)})</td>
      <td>{num(counts.mixed)} ({share(counts.mixed, total)})</td>
      <td>{num(counts.neutral)} ({share(counts.neutral, total)})</td>
    </tr>
  );
}

function CountsTable({ caption, first, rows, testId }: { caption: string; first: string; rows: { name: string; counts: SentimentCounts }[]; testId: string }) {
  return (
    <div className="table-wrap" tabIndex={-1}>
      <table className="data" data-testid={testId}>
        <caption>{caption}</caption>
        <thead>
          <tr>
            <th scope="col">{first}</th>
            <th scope="col">{t('Positive')}</th>
            <th scope="col">{t('Negative')}</th>
            <th scope="col">{t('Mixed')}</th>
            <th scope="col">{t('Neutral')}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <CountsRow key={r.name} name={r.name} counts={r.counts} header />
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * Sentiment (decision 202): how many passages use positive words, negative words, both, or neither, from a short list of English words that the
 * person can add to and take from. It says plainly what a list of words cannot do, and gives the passages so they can be read.
 */
export function SentimentTab() {
  const cases = useApp((s) => s.cases);
  const [caseId, setCaseId] = useState('');
  const [positive, setPositive] = useState('');
  const [negative, setNegative] = useState('');
  const [ownOnly, setOwnOnly] = useState(false);
  const [result, setResult] = useState<SentimentReport | null>(null);
  const [ranSet, setRanSet] = useState<WorkingSet | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setError(null);
    setBusy(true);
    const setThen = appStore.get().workingSet;
    try {
      const r = await call('analytics.sentiment', { ...(caseId ? { case_id: Number(caseId) } : {}), positive_words: wordList(positive), negative_words: wordList(negative), own_words_only: ownOnly, examples: 5 });
      setResult(r);
      setRanSet(setThen);
      announce(t('{passages} passages read: {positive} positive, {negative} negative, {mixed} mixed, {neutral} neutral.', { passages: num(r.passages), positive: num(r.counts.positive), negative: num(r.counts.negative), mixed: num(r.counts.mixed), neutral: num(r.counts.neutral) }));
    } catch (e) {
      setResult(null);
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const examples = (title: string, id: string, list: SentimentReport['most_positive']) =>
    list.length === 0 ? null : (
      <>
        <h4 className="bk-h4" id={id}>{title}</h4>
        <ol className="class-hardest" aria-labelledby={id}>
          {list.map((p) => (
            <li key={p.unit_id} data-testid="sentiment-example">
              <blockquote className="class-hard-text">{p.text}</blockquote>
              <p>{t('Words that counted: {words}.', { words: [...p.positive_words, ...p.negative_words].join(', ') })}</p>
              <span className="muted">{p.source_name}</span>{' '}
              <Button onPress={() => void openAt(p.source_id, p.start, p.end)} aria-label={t('Open {source_name} at this passage', { source_name: p.source_name })}>{t('Open')}</Button>
            </li>
          ))}
        </ol>
      </>
    );

  return (
    <section className="ac-card" aria-labelledby="h-text-sentiment" data-testid="text-sentiment">
      <h3 id="h-text-sentiment" className="bk-h3">{t('Sentiment')}</h3>
      <p className="help">{t('How many passages use positive words, negative words, both, or neither, by a list of English words. A word that follows not, never, no or the like within three words is counted the other way. A list of words cannot tell irony, or a word used in its own way in your field, so use the counts to find passages to read, and read a sample of each kind before relying on them. Nothing leaves this computer.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      <div className="field">
        <label htmlFor="sentiment-case">{t('Documents')}</label>
        <select id="sentiment-case" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
          <option value="">{t('Every document')}</option>
          {cases.map((c) => (
            <option key={c.id} value={c.id}>{t('The documents of the case {name}', { name: c.name })}</option>
          ))}
        </select>
      </div>
      <div className="team-sample">
        <div className="field">
          <label htmlFor="sentiment-positive">{t('More positive words (separate them with commas or spaces)')}</label>
          <textarea id="sentiment-positive" rows={3} value={positive} onChange={(e) => setPositive(e.target.value)} data-testid="sentiment-positive" />
        </div>
        <div className="field">
          <label htmlFor="sentiment-negative">{t('More negative words (separate them with commas or spaces)')}</label>
          <textarea id="sentiment-negative" rows={3} value={negative} onChange={(e) => setNegative(e.target.value)} data-testid="sentiment-negative" />
        </div>
      </div>
      <p className="help">{t('A word you give goes on its list and comes off the other, so you can move a word the built-in list puts on the wrong side for your material.')}</p>
      <Tick isSelected={ownOnly} onChange={setOwnOnly} testId="sentiment-own">{t('Use only my words, not the built-in list')}</Tick>
      <div className="actions">
        <Button className="primary" onPress={() => void run()} isDisabled={busy} data-testid="sentiment-run">{busy ? t('Reading…') : t('Read the sentiment')}</Button>
      </div>

      {result ? (
        <div data-testid="sentiment-result">
          <StaleSet since={ranSet} what="results" />
          <p role="status" data-testid="sentiment-summary">{tn(result.passages, { one: '{count} passage read, with {positive} positive words and {negative} negative words on the lists.', other: '{count} passages read, with {positive} positive words and {negative} negative words on the lists.' }, { positive: num(result.positive_list), negative: num(result.negative_list) })}</p>
          <CountsTable caption={t('All the passages')} first={t('Passages')} rows={[{ name: t('All the passages'), counts: result.counts }]} testId="sentiment-total" />
          {result.by_document.length > 1 ? <CountsTable caption={t('By document')} first={t('Document')} rows={result.by_document} testId="sentiment-documents" /> : null}
          {result.by_code.length > 0 ? <CountsTable caption={t('By code: the passages given each code')} first={t('Code')} rows={result.by_code} testId="sentiment-codes" /> : null}
          {examples(t('The most positive passages'), 'sentiment-pos-h', result.most_positive)}
          {examples(t('The most negative passages'), 'sentiment-neg-h', result.most_negative)}
          <ul className="class-notes">
            {result.notes.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
