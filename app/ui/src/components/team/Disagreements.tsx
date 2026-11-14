import { useEffect, useMemo, useState } from 'react';
import { Button, Radio, RadioGroup } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { canMeasure } from '../../lib/agreement';
import { announce } from '../../state/announce';
import { openAt } from '../../state/actions';
import { useApp } from '../../state/app';
import type { AdjudicationItem } from '../../types';
import { Notice } from '../common';
import { Tick } from '../Tick';
import { RegionDisagreements } from './RegionDisagreements';
import { dateTime, num, t, tn } from '../../i18n';

/** The codes a decision for one unit can carry, ticked or not, with the order they are listed in. */
function codeChoices(item: AdjudicationItem, extra: number[], names: Map<number, string>): { id: number; name: string }[] {
  const ids: number[] = [];
  for (const c of item.by_coder) for (const k of c.codes) if (!ids.includes(k.id)) ids.push(k.id);
  for (const k of item.decision?.codes ?? []) if (!ids.includes(k.id)) ids.push(k.id);
  for (const id of extra) if (!ids.includes(id)) ids.push(id);
  return ids.map((id) => ({ id, name: names.get(id) ?? String(id) }));
}

/** One unit: its words, what each coder gave it, and the form that settles it. */
function Case(props: { item: AdjudicationItem; names: Map<number, string>; active: { id: number; name: string }[]; onSaved: (unitId: number) => void; onError: (m: string) => void }) {
  const { item } = props;
  const [editing, setEditing] = useState(item.decision === null);
  const [ticked, setTicked] = useState<number[]>(() => item.decision?.codes.map((k) => k.id) ?? []);
  const [extra, setExtra] = useState<number[]>([]);
  const [reason, setReason] = useState(item.decision?.reason ?? '');
  const [busy, setBusy] = useState(false);
  const choices = codeChoices(item, extra, props.names);
  const others = props.active.filter((c) => !choices.some((x) => x.id === c.id));
  const where = `${item.source_name}`;

  const save = async () => {
    if (reason.trim() === '') return props.onError(t('A decision needs its reason: say why these codes are the right ones.'));
    setBusy(true);
    try {
      await call('adjudication.decide', { unit_id: item.unit_id, code_ids: ticked, reason });
      announce(t('Saved the decision for the unit in {where}.', { where }));
      setEditing(false);
      props.onSaved(item.unit_id);
    } catch (e) {
      props.onError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <li className="team-case" data-testid="disagreement">
      <h4 className="bk-h3">{where}</h4>
      <blockquote className="bk-quote" data-testid="disagreement-text">{`“${item.text}”`}</blockquote>
      <div className="table-wrap" tabIndex={-1}>
        <table className="data">
          <caption className="visually-hidden">{t('What each coder gave this unit')}</caption>
          <thead>
            <tr>
              <th scope="col">{t('Coder')}</th>
              <th scope="col">{t('Codes given')}</th>
            </tr>
          </thead>
          <tbody>
            {item.by_coder.map((c) => (
              <tr key={c.label}>
                <th scope="row">{c.role === 'model' ? t('{name} (AI model)', { name: c.label }) : c.label}</th>
                <td>{c.codes.map((k) => k.name).join(', ')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {item.decision && !editing ? (
        <div className="team-decision" data-testid="decision">
          <p>
            <strong>{item.decision.codes.length === 0 ? t('Settled: no code applies.') : t('Settled: {codes}.', { codes: item.decision.codes.map((k) => k.name).join(', ') })}</strong>{' '}
            {t('By {name}, {when}.', { name: item.decision.adjudicator_name, when: dateTime(new Date(item.decision.created_at), { dateStyle: 'medium', timeStyle: 'short' }) })}
          </p>
          <p>{t('Reason: {reason}', { reason: item.decision.reason })}</p>
          <Button onPress={() => setEditing(true)} aria-label={t('Change the decision for the unit in {where}', { where })}>{t('Change it')}</Button>
        </div>
      ) : (
        <fieldset className="team-form" data-testid="decision-form">
          <legend>{t('What this unit should carry')}</legend>
          {choices.map((c) => (
            <Tick key={c.id} isSelected={ticked.includes(c.id)} onChange={(on) => setTicked(on ? [...ticked, c.id] : ticked.filter((x) => x !== c.id))} testId={`decide-code-${c.id}`}>{c.name}</Tick>
          ))}
          {others.length > 0 ? (
            <div className="field">
              <label htmlFor={`other-${item.unit_id}`}>{t('Another code')}</label>
              <select
                id={`other-${item.unit_id}`}
                value=""
                onChange={(e) => {
                  const id = Number(e.target.value);
                  if (id) {
                    setExtra([...extra, id]);
                    setTicked([...ticked, id]);
                  }
                }}
              >
                <option value="">{t('Choose a code to add')}</option>
                {others.map((c) => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
              </select>
            </div>
          ) : null}
          <p className="help">{t('Tick none of them to say that no code applies.')}</p>
          <div className="field">
            <label htmlFor={`reason-${item.unit_id}`}>{t('Why these codes')}</label>
            <textarea id={`reason-${item.unit_id}`} rows={2} value={reason} onChange={(e) => setReason(e.target.value)} data-testid="decision-reason" />
          </div>
          <div className="actions">
            <Button className="primary" onPress={() => void save()} isDisabled={busy} data-testid="decision-save">{t('Save the decision')}</Button>
            {item.decision ? <Button onPress={() => setEditing(false)}>{t('Keep the earlier decision')}</Button> : null}
          </div>
        </fieldset>
      )}
      <div className="actions">
        <Button onPress={() => void openAt(item.source_id, item.start, item.end)} aria-label={t('Open the unit in {where} in the document', { where })}>{t('Open in the document')}</Button>
      </div>
    </li>
  );
}

/**
 * Disagreements: every unit that two or more of the chosen coders coded differently. A person settles each with the codes it should
 * carry and a reason. The decisions are kept apart from every coder's own coding, which is never changed.
 */
export function DisagreementsTab({ picked }: { picked: number[] }) {
  const codes = useApp((s) => s.codes);
  const version = useApp((s) => s.dataVersion);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [blind, setBlind] = useState(false);
  const [show, setShow] = useState<'open' | 'all'>('open');
  const [queue, setQueue] = useState<AdjudicationItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  // A unit settled just now stays in view, showing what was decided, until the person changes what is shown.
  const [justSettled, setJustSettled] = useState<number[]>([]);

  const names = useMemo(() => new Map(codes.map((c) => [c.id, c.name])), [codes]);
  const active = useMemo(() => codes.filter((c) => c.status === 'active').map((c) => ({ id: c.id, name: c.name })), [codes]);
  const ready = canMeasure(picked);
  const key = picked.join(',');

  useEffect(() => {
    if (!ready) {
      setQueue(null);
      return;
    }
    let live = true;
    call('adjudication.queue', { coder_ids: picked, blind })
      .then((q) => {
        if (!live) return;
        setQueue(q);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // `picked` is read through its key, so a new array with the same coders does not ask again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, blind, version, reload, ready]);

  const open = (queue ?? []).filter((d) => d.decision === null).length;
  const shown = (queue ?? []).filter((d) => show === 'all' || d.decision === null || justSettled.includes(d.unit_id));

  return (
    <section className="ac-card" aria-labelledby="h-team-disagree" data-testid="disagreements">
      <h3 id="h-team-disagree" className="bk-h3">{t('Disagreements')}</h3>
      <p className="help">{t('Every unit that two or more of the chosen coders coded differently. Settle each with the codes it should carry and your reason. What you decide is kept apart from every coder\'s own coding, which is never changed.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      {readOnly ? <Notice kind="warning">{t('This project is read-only, so nothing can be decided.')}</Notice> : null}
      <Tick isSelected={blind} onChange={setBlind} testId="team-blind">{t('Hide who coded what (the coders are called Coder A, Coder B, and so on)')}</Tick>
      <RadioGroup value={show} onChange={(v) => { setShow(v as 'open' | 'all'); setJustSettled([]); }} className="field ac-radios ac-inline" aria-label={t('Which units to show')}>
        <Radio value="open" className="react-aria-Radio"><span>{t('Not settled yet')}</span></Radio>
        <Radio value="all" className="react-aria-Radio"><span>{t('All of them')}</span></Radio>
      </RadioGroup>
      {!ready ? (
        <p className="muted" data-testid="disagree-need">{t('Choose at least two coders above to see where they differ.')}</p>
      ) : queue === null ? (
        <p className="muted">{t('Looking for disagreements…')}</p>
      ) : (
        <>
          <p role="status" data-testid="disagree-count">
            {queue.length === 0
              ? t('These coders did not code any unit differently.')
              : t('{open} of {total} are not settled yet.', { open: num(open), total: tn(queue.length, { one: '{count} unit', other: '{count} units' }) })}
          </p>
          <ul className="team-cases">
            {shown.map((d) => (
              <Case
                key={`${d.unit_id}-${blind ? 'b' : 'n'}-${d.decision?.id ?? 0}`}
                item={d}
                names={names}
                active={active}
                onSaved={(unitId) => {
                  setJustSettled((ids) => (ids.includes(unitId) ? ids : [...ids, unitId]));
                  setReload((n) => n + 1);
                }}
                onError={setError}
              />
            ))}
          </ul>
        </>
      )}
      <RegionDisagreements blind={blind} show={show} />
    </section>
  );
}
