import { useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { openAt } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import type { WorkingSet } from '../../lib/workingSet';
import type { TopicReport } from '../../types';
import { Notice } from '../common';
import { StaleSet } from '../WorkingSet';
import { Tick } from '../Tick';
import { num, t, tn } from '../../i18n';

/** What was read, in one sentence. */
export function topicsSummary(r: TopicReport): string {
  const topics = tn(r.topics.length, { one: '{count} topic', other: '{count} topics' });
  const base = t('{topics} found in {passages} passages, worked out from {words} different words.', { topics, passages: num(r.passages_used), words: num(r.vocabulary) });
  return r.passages_left_out > 0 ? `${base} ${tn(r.passages_left_out, { one: '{count} passage had too few of those words to be placed.', other: '{count} passages had too few of those words to be placed.' })}` : base;
}

/**
 * Topics (decision 202): groups of words that tend to appear in the same passages, with the passages that belong to each most. The number of
 * topics is the person's choice, and the page says plainly that a topic is a way to find passages to read and not yet a finding.
 */
export function TopicsTab() {
  const cases = useApp((s) => s.cases);
  const [caseId, setCaseId] = useState('');
  const [count, setCount] = useState('5');
  const [common, setCommon] = useState(true);
  const [stem, setStem] = useState(false);
  const [result, setResult] = useState<TopicReport | null>(null);
  const [ranSet, setRanSet] = useState<WorkingSet | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setError(null);
    setBusy(true);
    const setThen = appStore.get().workingSet;
    try {
      const r = await call('analytics.topics', { ...(caseId ? { case_id: Number(caseId) } : {}), topics: Number(count), leave_out_common: common, stem });
      setResult(r);
      setRanSet(setThen);
      announce(topicsSummary(r));
    } catch (e) {
      setResult(null);
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="ac-card" aria-labelledby="h-text-topics" data-testid="text-topics">
      <h3 id="h-text-topics" className="bk-h3">{t('Topics')}</h3>
      <p className="help">{t('Groups of words that tend to appear in the same passages, found from the words alone. You choose how many. A topic is a way to find passages to read; it is not a finding, and not yet a theme. Another number of topics gives other groups, so try more than one. Nothing leaves this computer.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      <div className="team-sample">
        <div className="field">
          <label htmlFor="topics-case">{t('Documents')}</label>
          <select id="topics-case" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
            <option value="">{t('Every document')}</option>
            {cases.map((c) => (
              <option key={c.id} value={c.id}>{t('The documents of the case {name}', { name: c.name })}</option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor="topics-count">{t('How many topics')}</label>
          <select id="topics-count" value={count} onChange={(e) => setCount(e.target.value)} data-testid="topics-count">
            {Array.from({ length: 19 }, (_, i) => i + 2).map((n) => (
              <option key={n} value={n}>{n}</option>
            ))}
          </select>
        </div>
      </div>
      <Tick isSelected={common} onChange={setCommon} testId="topics-common">{t('Leave out common English words (the, and, of, and so on)')}</Tick>
      <Tick isSelected={stem} onChange={setStem} testId="topics-stem">{t('Count the forms of an English word together (connect, connected, connecting)')}</Tick>
      <div className="actions">
        <Button className="primary" onPress={() => void run()} isDisabled={busy} data-testid="topics-run">{busy ? t('Looking for topics…') : t('Find the topics')}</Button>
      </div>

      {result ? (
        <div data-testid="topics-result">
          <StaleSet since={ranSet} what="results" />
          <p role="status" data-testid="topics-summary">{topicsSummary(result)}</p>
          <ol className="topics-list" data-testid="topics-list">
            {result.topics.map((topic) => (
              <li key={topic.number} data-testid="topic">
                <h4 className="bk-h4">{t('Topic {number}: {label}', { number: num(topic.number), label: topic.label })}</h4>
                <p>{tn(topic.passages, { one: '{count} passage, {share} of those read.', other: '{count} passages, {share} of those read.' }, { share: num(topic.share, { style: 'percent', maximumFractionDigits: 0 }) })}</p>
                <p data-testid="topic-words">{t('Its words, the strongest first: {words}.', { words: topic.words.map((w) => w.word).join(', ') })}</p>
                <ul className="topic-examples">
                  {topic.examples.map((e) => (
                    <li key={e.unit_id}>
                      <blockquote className="class-hard-text">{e.text}</blockquote>
                      <span className="muted">{e.source_name}</span>{' '}
                      <Button onPress={() => void openAt(e.source_id, e.start, e.end)} aria-label={t('Open {source_name} at this passage', { source_name: e.source_name })}>{t('Open')}</Button>
                    </li>
                  ))}
                </ul>
              </li>
            ))}
          </ol>
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
