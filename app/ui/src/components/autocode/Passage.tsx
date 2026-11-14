// A suggested passage in its place: the words just before, the suggested words picked out, and the words just after.

/** The characters the engine sends on each side; a side that is this long has more text beyond it. */import { rich, t } from '../../i18n';

const CONTEXT = 70;

export function Passage(props: { before: string; text: string; after: string; truncated?: boolean; className?: string; testId?: string }) {
  const before = Array.from(props.before).length >= CONTEXT ? `…${props.before}` : props.before;
  const after = Array.from(props.after).length >= CONTEXT ? `${props.after}…` : props.after;
  return (
    <p className={`ac-passage${props.className ? ` ${props.className}` : ''}`} data-testid={props.testId}>
      {rich('<span>{before}</span><span2>[suggested words: </span2><mark>{text}</mark>', { before, text: props.text, span: (inside) => <span className="ac-context">{inside}</span>, span2: (inside) => <span className="visually-hidden">{inside}</span>, mark: (inside) => <mark className="ac-words">{inside}</mark> })}
      {props.truncated ? <span className="ac-more">{t('… (this passage is longer than what is shown)')}</span> : null}
      <span className="visually-hidden">]</span>
      <span className="ac-context">{after}</span>
    </p>
  );
}
