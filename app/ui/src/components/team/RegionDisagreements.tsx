import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { describeShape } from '../../lib/regions';
import { announce } from '../../state/announce';
import { openAt } from '../../state/actions';
import { useApp } from '../../state/app';
import type { RegionCase } from '../../types';
import { Notice } from '../common';
import { SegmentQuote } from '../SegmentQuote';
import { dateTime, num, t, tn } from '../../i18n';

/** One area or stretch the coders did not all mark: who marked it, who did not, and the form that decides whether it is final. */
function Case(props: { item: RegionCase; blind: boolean; readOnly: boolean; onSaved: (key: number) => void; onError: (m: string) => void }) {
  const { item, blind } = props;
  const pdf = useApp((s) => s.sources.find((x) => x.id === item.source_id)?.pdf ?? null);
  const onPage = pdf !== null && pdf !== undefined;
  const [editing, setEditing] = useState(item.decision === null);
  const [reason, setReason] = useState(item.decision?.reason ?? '');
  const [busy, setBusy] = useState(false);
  const where = onPage ? t('{name}, page {page}', { name: item.source_name, page: num(item.page) }) : item.source_name;
  const total = item.members.length + item.missing.length;

  const decide = async (accept: boolean) => {
    if (reason.trim() === '') return props.onError(t('A decision needs its reason: say why this area belongs in the final layer, or why it does not.'));
    setBusy(true);
    try {
      await call('adjudication.decide_region', { key: item.key, accept, reason });
      announce(accept ? t('Accepted the {code} in {where} into the final layer.', { code: item.code_name, where }) : t('Left the {code} in {where} out of the final layer.', { code: item.code_name, where }));
      setEditing(false);
      props.onSaved(item.key);
    } catch (e) {
      props.onError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <li className="team-case" data-testid="region-disagreement">
      <h4 className="bk-h3">{t('{code} in {where}', { code: item.code_name, where })}</h4>
      <ul className="team-cases" aria-label={t('What each coder marked')}>
        {item.members.map((m) => (
          <li key={m.region_id} data-testid="region-member">
            <SegmentQuote text={describeShape(m, onPage ? 'page' : 'picture')} sourceId={item.source_id} region={m} page={onPage ? item.page : null} />
            {blind ? null : <p className="muted">{t('Marked by {name}', { name: m.coder_name })}</p>}
          </li>
        ))}
      </ul>
      <p className="muted" data-testid="region-who">
        {blind
          ? t('{marked} of {total} coders of this document marked it.', { marked: num(item.members.length), total: num(total) })
          : t('Not marked by {names}.', { names: item.missing.join(', ') })}
      </p>

      {item.decision && !editing ? (
        <div className="team-decision" data-testid="region-decision">
          <p>
            <strong>{item.decision.accepted ? t('Settled: it is in the final layer.') : t('Settled: it is left out of the final layer.')}</strong>{' '}
            {t('By {name}, {when}.', { name: item.decision.adjudicator_name, when: dateTime(new Date(item.decision.created_at), { dateStyle: 'medium', timeStyle: 'short' }) })}
          </p>
          <p>{t('Reason: {reason}', { reason: item.decision.reason })}</p>
          <Button onPress={() => setEditing(true)} isDisabled={props.readOnly} aria-label={t('Change the decision for the {code} in {where}', { code: item.code_name, where })}>{t('Change it')}</Button>
        </div>
      ) : (
        <fieldset className="team-form" data-testid="region-decision-form">
          <legend>{t('Does this belong in the final layer?')}</legend>
          <div className="field">
            <label htmlFor={`region-reason-${item.key}`}>{t('Your reason')}</label>
            <textarea id={`region-reason-${item.key}`} rows={2} value={reason} onChange={(e) => setReason(e.target.value)} data-testid="region-reason" />
          </div>
          <div className="actions">
            <Button className="primary" onPress={() => void decide(true)} isDisabled={busy || props.readOnly} data-testid="region-accept">{t('Accept into the final layer')}</Button>
            <Button onPress={() => void decide(false)} isDisabled={busy || props.readOnly} data-testid="region-leave-out">{t('Leave it out')}</Button>
            {item.decision ? <Button onPress={() => setEditing(false)}>{t('Keep the earlier decision')}</Button> : null}
          </div>
        </fieldset>
      )}
      <div className="actions">
        <Button onPress={() => void openAt(item.source_id, 0, 0, -item.members[0].region_id)} aria-label={t('Open the {code} in {where}', { code: item.code_name, where })}>
          {item.kind === 'clip' ? t('Open the recording') : t('Open the picture')}
        </Button>
      </div>
    </li>
  );
}

/**
 * Areas of pictures and pages and stretches of recordings that some of a document's coders marked and others did not (decision 176).
 * An area has no unit, so what a person decides is whether it belongs in the final layer. Coders' areas of one code that overlap by at
 * least half are the same area. Nothing a coder marked is changed.
 */
export function RegionDisagreements({ blind, show }: { blind: boolean; show: 'open' | 'all' }) {
  const version = useApp((s) => s.dataVersion);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [queue, setQueue] = useState<RegionCase[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  // A case settled just now stays in view, showing what was decided, until the person changes what is shown.
  const [justSettled, setJustSettled] = useState<number[]>([]);

  useEffect(() => {
    let live = true;
    call('adjudication.region_queue', {})
      .then((q) => {
        if (!live) return;
        setQueue(q);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version, reload]);

  useEffect(() => setJustSettled([]), [show]);

  // Nothing to decide, and nothing decided before: the section stays out of the way.
  if (queue === null || queue.length === 0) return error ? <Notice>{error}</Notice> : null;

  const open = queue.filter((d) => d.decision === null).length;
  const shown = queue.filter((d) => show === 'all' || d.decision === null || justSettled.includes(d.key));

  return (
    <section aria-labelledby="h-team-region-disagree" data-testid="region-disagreements">
      <h4 id="h-team-region-disagree" className="bk-h3">{t('Areas and stretches')}</h4>
      <p className="help">{t('Areas of pictures and pages, and stretches of recordings, that some coders marked and others did not. Accept one to put it in the final layer, or leave it out, and say why. Areas of one code that overlap by at least half count as the same area.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      <p role="status" data-testid="region-disagree-count">
        {t('Not settled yet: {open} of {total}.', { open: num(open), total: tn(queue.length, { one: '{count} area or stretch', other: '{count} areas and stretches' }) })}
      </p>
      <ul className="team-cases">
        {shown.map((d) => (
          <Case
            key={`${d.key}-${d.decision?.created_at ?? 0}`}
            item={d}
            blind={blind}
            readOnly={readOnly}
            onSaved={(key) => {
              setJustSettled((ids) => (ids.includes(key) ? ids : [...ids, key]));
              setReload((n) => n + 1);
            }}
            onError={setError}
          />
        ))}
      </ul>
    </section>
  );
}
