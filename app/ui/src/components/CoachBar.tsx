import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { call } from '../bridge';
import { goTo } from '../state/actions';
import { useApp } from '../state/app';
import { t, tn } from '../i18n';

/**
 * The coach, kept on (decision 200): while a student codes in an assignment whose instructor asked for it, a line above the work says how many
 * questions the coach has, and one button opens them. It looks again, a moment after the coding changes, and it only reads.
 */
export function CoachBar() {
  const version = useApp((s) => s.dataVersion);
  const [questions, setQuestions] = useState<number | null>(null);

  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => {
      call('coach.check', {})
        .then((r) => live && setQuestions(r.findings.length))
        .catch(() => live && setQuestions(null));
    }, 500);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [version]);

  return (
    <section className="coach-bar" aria-label={t('Coach')} data-testid="coach-bar">
      <p role="status" data-testid="coach-bar-text">
        {questions === null
          ? t('The coach is on: your instructor asked for it in this assignment.')
          : questions === 0
            ? t('The coach has no question for you at the moment.')
            : tn(questions, { one: 'The coach has {count} question for you.', other: 'The coach has {count} questions for you.' })}
      </p>
      <Button onPress={() => goTo('team')} data-testid="coach-bar-open">{t('Open the coach')}</Button>
    </section>
  );
}
