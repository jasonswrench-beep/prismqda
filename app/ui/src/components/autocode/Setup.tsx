// Auto Code, set up: how to find passages, where to look, which code to suggest, what a run finds, and sending it to the
// review queue. Nothing is coded here; every suggestion waits for a decision in the review queue.

import { useEffect, useMemo, useState } from 'react';
import { Button, Checkbox, Radio, RadioGroup } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import {
  buildArgs, METHOD_LABELS, previewSentences, readback, setupKey, TARGET_LABELS, type AutoSetup, type SpeakerSetup,
} from '../../lib/autocode';
import { goAutoCode, openAt, sendAutoCode, setAutoSetup } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import type { AutocodePreview, AutoMethod, AutoScope, AutoTarget, HeadingLevel, SpeakerGroup } from '../../types';
import { Notice, plural } from '../common';
import { CheckPicker, useNames } from '../explore/shared';
import { CodeSelect } from './CodeSelect';
import { Passage } from './Passage';
import { num, t, tn } from '../../i18n';

const METHODS: { id: AutoMethod; label: string; help: string }[] = [
  { id: 'words', get label() { return t('Words'); }, get help() { return t('Passages that contain words, a phrase, or a pattern you type.'); } },
  { id: 'speakers', get label() { return t('Speakers'); }, get help() { return t('Every turn of the speakers you choose in a transcript.'); } },
  { id: 'headings', get label() { return t('Headings'); }, get help() { return t('The section under each heading of the levels you choose.'); } },
];

const TARGETS: AutoTarget[] = ['hit', 'words', 'sentence', 'paragraph', 'unit'];

/** The scope of the form as the engine takes it, or null while the form does not name one yet. */
function scopeOf(s: AutoSetup): AutoScope | null {
  if (s.scope === 'documents') return s.sourceIds.length === 0 ? null : { source_ids: s.sourceIds };
  if (s.scope === 'case') return s.caseId === null ? null : { case_id: s.caseId };
  return {};
}

export function AutoCodeSetup() {
  const setup = useApp((s) => s.autoCodeSetup);
  const sources = useApp((s) => s.sources);
  const cases = useApp((s) => s.cases);
  const codes = useApp((s) => s.codes);
  const codingMode = useApp((s) => s.project?.coding_mode);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const version = useApp((s) => s.dataVersion);
  const baseNames = useNames();
  const names = useMemo(() => ({ ...baseNames, speaker: (key: string) => setup.speakers[key]?.label ?? key }), [baseNames, setup.speakers]);
  const [found, setFound] = useState<{ key: string; result: AutocodePreview } | null>(null);
  const [busy, setBusy] = useState<'finding' | 'sending' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [groups, setGroups] = useState<SpeakerGroup[] | null>(null);
  const [levels, setLevels] = useState<HeadingLevel[] | null>(null);

  const unitMode = codingMode === 'unit';
  const built = useMemo(() => buildArgs(setup), [setup]);
  const key = 'args' in built ? setupKey(built.args) : null;
  const stale = found !== null && found.key !== key;
  const words = 'args' in built ? readback(built.args, names) : null;
  const scope = scopeOf(setup);
  const scopeKey = JSON.stringify(scope);

  const update = (patch: Partial<AutoSetup>) => setAutoSetup(patch);

  // Speakers and heading levels depend on where the form looks, so they are read again when that changes.
  useEffect(() => {
    if (setup.method !== 'speakers' || scope === null) return;
    let live = true;
    call('autocode.speakers', scope)
      .then((list) => {
        if (!live) return;
        setGroups(list);
        // A speaker seen for the first time starts unchosen, with a code of their own name ready.
        // Speakers who are not in these documents are dropped, so a choice made for other documents is not sent.
        const current = appStore.get().autoCodeSetup;
        const next: Record<string, SpeakerSetup> = {};
        for (const g of list) {
          if (current.speakers[g.key]) {
            next[g.key] = { ...current.speakers[g.key], label: g.label };
            continue;
          }
          const same = codes.find((c) => c.status === 'active' && c.name.toLowerCase() === g.label.toLowerCase());
          next[g.key] = same ? { label: g.label, include: false, mode: 'existing', codeId: same.id, newCode: '' } : { label: g.label, include: false, mode: 'new', codeId: null, newCode: g.label };
        }
        setAutoSetup({ speakers: next });
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // The speakers are listed again when the place to look or the project changes; the codes only seed the first guess.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setup.method, scopeKey, version]);

  useEffect(() => {
    if (setup.method !== 'headings' || scope === null) return;
    let live = true;
    call('autocode.headings', scope)
      .then((r) => live && setLevels(r.levels))
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setup.method, scopeKey, version]);

  const find = async () => {
    setError(null);
    if (!('args' in built)) {
      setError(built.problem);
      announce(built.problem, 'assertive');
      return;
    }
    setBusy('finding');
    try {
      const result = await call('autocode.preview', built.args);
      setFound({ key: setupKey(built.args), result });
      announce(previewSentences(result).join(' '));
    } catch (e) {
      const m = toApiError(e).message;
      setError(m);
      announce(m, 'assertive');
    } finally {
      setBusy(null);
    }
  };

  const send = async () => {
    if (!('args' in built) || found === null || stale) return;
    setBusy('sending');
    setError(null);
    const r = await sendAutoCode(built.args);
    setBusy(null);
    if (!r) return;
    const made = r.codes_created.length > 0 ? ' ' + tn(r.codes_created.length, { one: 'New code: {names}.', other: 'New codes: {names}.' }, { names: r.codes_created.map((c) => c.name).join(', ') }) : '';
    announce(t('Sent {suggestions} to the review queue.{made}', { suggestions: tn(r.created, { one: '{count} suggestion', other: '{count} suggestions' }), made }));
    setFound(null);
    goAutoCode('queue', r.batch);
  };

  const result = found?.result ?? null;
  const cantSend = readOnly || busy !== null || result === null || stale || result.suggestions === 0;
  const sourceChoices = sources.map((s) => ({ id: s.id, label: s.name }));

  return (
    <div className="ac-setup" data-testid="autocode-setup">
      <p className="help">{t('Auto Code finds passages by a rule you can read and suggests a code for each one. Nothing is coded until you accept it in the review queue, where you can also correct it. No AI or language model is used.')}</p>

      <section className="ac-card" aria-labelledby="h-ac-how">
        <h3 id="h-ac-how" className="bk-h3">{t('How to find passages')}</h3>
        <RadioGroup value={setup.method} onChange={(v) => update({ method: v as AutoMethod })} className="field ac-radios" aria-label={t('How to find passages')}>
          {METHODS.map((m) => (
            <Radio key={m.id} value={m.id} className="react-aria-Radio ac-radio">
              <span className="ac-radio-label">{m.label}</span>
              <span className="ac-radio-help">{m.help}</span>
            </Radio>
          ))}
        </RadioGroup>
      </section>

      <section className="ac-card" aria-labelledby="h-ac-where">
        <h3 id="h-ac-where" className="bk-h3">{t('Where to look')}</h3>
        <RadioGroup value={setup.scope} onChange={(v) => update({ scope: v as AutoSetup['scope'] })} className="field ac-radios ac-inline" aria-label={t('Where to look')}>
          <Radio value="all" className="react-aria-Radio">
            <span>{t('Every document')}</span>
          </Radio>
          <Radio value="documents" className="react-aria-Radio">
            <span>{t('Documents I choose')}</span>
          </Radio>
          <Radio value="case" className="react-aria-Radio">
            <span>{t('The documents of one case')}</span>
          </Radio>
        </RadioGroup>
        {setup.scope === 'documents' ? (
          <div className="field">
            <CheckPicker noun="documents" label={t('Documents to look in')} choices={sourceChoices} value={setup.sourceIds} onChange={(ids) => update({ sourceIds: ids })} none={t('Choose documents')} testId="ac-documents" />
          </div>
        ) : null}
        {setup.scope === 'case' ? (
          <div className="field">
            <label htmlFor="ac-case">{t('Case')}</label>
            <select id="ac-case" value={setup.caseId === null ? '' : String(setup.caseId)} onChange={(e) => update({ caseId: e.target.value === '' ? null : Number(e.target.value) })}>
              <option value="">{t('Choose a case…')}</option>
              {cases.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </div>
        ) : null}
      </section>

      <section className="ac-card" aria-labelledby="h-ac-rule">
        <h3 id="h-ac-rule" className="bk-h3">
          {setup.method === 'words' ? t('The words') : setup.method === 'speakers' ? t('The speakers') : t('The headings')}
        </h3>

        {setup.method === 'words' ? (
          <>
            <div className="field">
              <label htmlFor="ac-query">{t('Words to find')}</label>
              <input id="ac-query" type="text" value={setup.query} onChange={(e) => update({ query: e.target.value })} aria-describedby="ac-query-help" autoComplete="off" spellCheck={false} />
              <span id="ac-query-help" className="help">{t('A word (cost), a phrase in quotes ("first day"), a pattern (welcom* or wom?n), AND, OR, NOT in capitals, cost NEAR/5 access, or /a regular expression/.')}</span>
            </div>
            <div className="field">
              <label htmlFor="ac-target">{t('Each suggestion covers')}</label>
              <select id="ac-target" value={setup.target} onChange={(e) => update({ target: e.target.value as AutoTarget })}>
                {TARGETS.filter((t) => !unitMode || t === 'hit' || t === 'unit').map((t) => (
                  <option key={t} value={t}>
                    {TARGET_LABELS[t]}
                  </option>
                ))}
              </select>
              {unitMode ? <span className="help">{t('This project codes whole units, so a suggestion covers the matched words or the whole unit.')}</span> : null}
            </div>
            {setup.target === 'words' ? (
              <div className="field">
                <label htmlFor="ac-around">{t('Words either side')}</label>
                <input id="ac-around" type="number" min={1} max={50} value={Number.isFinite(setup.around) ? setup.around : ''} onChange={(e) => update({ around: e.target.value === '' ? NaN : Number(e.target.value) })} />
                <span className="help">{t('From 1 to 50. The suggestion stops at the start or end of its paragraph or unit.')}</span>
              </div>
            ) : null}
            <CodeSelect
              id="ac-code"
              label={t('Code to suggest')}
              value={{ mode: setup.codeMode, codeId: setup.codeId, newCode: setup.newCode }}
              onChange={(p) => update({ codeMode: p.mode, codeId: p.codeId, newCode: p.newCode })}
            />
          </>
        ) : null}

        {setup.method === 'speakers' ? (
          <SpeakersTable groups={groups} scopeReady={scope !== null} speakers={setup.speakers} onChange={(speakers) => update({ speakers })} />
        ) : null}

        {setup.method === 'headings' ? (
          <>
            {unitMode ? (
              <Notice kind="warning">{t('This project codes whole units, and a section under a heading is a stretch of words, not a unit. Headings are not available here.')}</Notice>
            ) : levels === null ? (
              <p className="muted" role="status">{t('Looking for headings…')}</p>
            ) : levels.length === 0 ? (
              <p className="help">{t('The documents you chose have no headings, so there is nothing to use. Headings come from Word, Markdown, and web page files when they are imported.')}</p>
            ) : (
              <fieldset className="field ac-levels">
                <legend>{t('Heading levels')}</legend>
                {levels.map((l) => (
                  <Checkbox
                    key={l.level}
                    isSelected={setup.levels.includes(l.level)}
                    onChange={(on) => update({ levels: on ? [...setup.levels, l.level] : setup.levels.filter((x) => x !== l.level) })}
                    className="check"
                  >
                    <Box />{t('Level {level} ({headings} in {documents})', { level: l.level, headings: tn(l.count, { one: '{count} heading', other: '{count} headings' }), documents: tn(l.sources, { one: '{count} document', other: '{count} documents' }) })}</Checkbox>
                ))}
              </fieldset>
            )}
            <div className="field">
              <label htmlFor="ac-heading-query">{t('Only headings that contain (optional)')}</label>
              <input id="ac-heading-query" type="text" value={setup.headingQuery} onChange={(e) => update({ headingQuery: e.target.value })} autoComplete="off" spellCheck={false} />
              <span className="help">{t('The same words and patterns as above, for example methods OR design.')}</span>
            </div>
            <Checkbox isSelected={setup.includeHeading} onChange={(on) => update({ includeHeading: on })} className="check">
              <Box />{t('A suggestion includes the heading itself')}</Checkbox>
            <CodeSelect
              id="ac-code-headings"
              label={t('Code to suggest')}
              value={{ mode: setup.codeMode, codeId: setup.codeId, newCode: setup.newCode }}
              onChange={(p) => update({ codeMode: p.mode, codeId: p.codeId, newCode: p.newCode })}
            />
            <p className="help">{t('A section runs from its heading to the next heading of the same or a higher level. Sections that overlap are joined.')}</p>
          </>
        ) : null}
      </section>

      <section className="ac-card" aria-labelledby="h-ac-words" data-testid="ac-readback">
        <h3 id="h-ac-words" className="bk-h3">{t('This run in words')}</h3>
        {words ? <p>{words}</p> : <p className="muted">{'problem' in built ? built.problem : ''}</p>}
        <div className="actions">
          <Button className="primary" onPress={() => void find()} isDisabled={busy !== null} data-testid="ac-find">
            {busy === 'finding' ? t('Finding…') : t('Find')}
          </Button>
        </div>
        {error ? <Notice>{error}</Notice> : null}
      </section>

      <section className="ac-card" aria-labelledby="h-ac-found" data-testid="ac-found">
        <h3 id="h-ac-found" className="bk-h3">{t('What it finds')}</h3>
        {result === null ? (
          <p className="muted">{t('Nothing has been looked for yet. Press Find to see what this run would suggest, before anything is sent.')}</p>
        ) : (
          <>
            {stale ? (
              <Notice kind="warning">
                <span data-testid="ac-stale">{t('This is what an earlier version of the form found. Press Find again to see what the form above would suggest.')}</span>
              </Notice>
            ) : null}
            <ul className={`ac-facts${stale ? ' stale' : ''}`} data-testid="ac-facts">
              {previewSentences(result).map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
            {result.codes.length > 0 ? (
              <p className="muted">
                {result.codes.map((c) => (c.new ? t('{name} (a new code): {suggestions}', { name: c.name, suggestions: tn(c.suggestions, { one: '{count} suggestion', other: '{count} suggestions' }) }) : t('{name}: {suggestions}', { name: c.name, suggestions: tn(c.suggestions, { one: '{count} suggestion', other: '{count} suggestions' }) }))).join('; ')}.
              </p>
            ) : null}
            {result.examples.length > 0 ? (
              <>
                <p className="ac-examples-title">{result.suggestions > result.examples.length ? t('The first {count} of {suggestions}:', { count: result.examples.length, suggestions: num(result.suggestions) }) : t('The suggestions:')}</p>
                <ul className={`ac-examples${stale ? ' stale' : ''}`} aria-label={t('Examples of what it finds')}>
                  {result.examples.map((x) => (
                    <li key={`${x.source_id}-${x.start}-${x.end}`}>
                      <Passage before={x.before} text={x.text} after={x.after} truncated={x.truncated} />
                      <p className="muted">
                        {t('{source_name} · would be coded {code_name}', { source_name: x.source_name, code_name: x.code_name })}
                      </p>
                      <Button onPress={() => void openAt(x.source_id, x.start, x.end)} aria-label={t('Open {source_name} at these words', { source_name: x.source_name })}>{t('Open at source')}</Button>
                    </li>
                  ))}
                </ul>
              </>
            ) : null}
            <div className="actions">
              <Button className="primary" onPress={() => void send()} isDisabled={cantSend} data-testid="ac-send">
                {busy === 'sending' ? t('Sending…') : result.suggestions > 0 ? t('Send {suggestions} to the review queue', { suggestions: tn(result.suggestions, { one: '{count} suggestion', other: '{count} suggestions' }) }) : t('Send to the review queue')}
              </Button>
            </div>
            {readOnly ? <p className="muted">{t('This project is read only, so nothing can be sent.')}</p> : null}
          </>
        )}
      </section>
    </div>
  );
}

function Box() {
  return (
    <div className="box" aria-hidden="true">
      <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
        <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
      </svg>
    </div>
  );
}

function SpeakersTable(props: { groups: SpeakerGroup[] | null; scopeReady: boolean; speakers: Record<string, SpeakerSetup>; onChange: (next: Record<string, SpeakerSetup>) => void }) {
  if (!props.scopeReady) return <p className="help">{t('Choose the documents or the case first.')}</p>;
  if (props.groups === null) {
    return (
      <p className="muted" role="status">{t('Looking for speakers…')}</p>
    );
  }
  if (props.groups.length === 0) {
    return <p className="help">{t('These documents have no speaker labels. Speakers come from transcripts: import a file as a transcript, or choose another way to find passages.')}</p>;
  }
  const set = (key: string, patch: Partial<SpeakerSetup>) => props.onChange({ ...props.speakers, [key]: { ...props.speakers[key], ...patch } });
  return (
    <div className="ac-table-wrap">
      <table className="ex-matrix ac-speakers" data-testid="ac-speakers">
        <caption>{t('Each speaker in these documents. Spellings that differ only in capitals, punctuation, or spacing are one speaker.')}</caption>
        <thead>
          <tr>
            <th scope="col">{t('Use')}</th>
            <th scope="col">{t('Speaker')}</th>
            <th scope="col">{t('Turns')}</th>
            <th scope="col">{t('Code to suggest')}</th>
          </tr>
        </thead>
        <tbody>
          {props.groups.map((g) => {
            const v = props.speakers[g.key];
            if (!v) return null;
            const others = g.variants.filter((x) => x !== g.label);
            return (
              <tr key={g.key}>
                <td>
                  <Checkbox isSelected={v.include} onChange={(on) => set(g.key, { include: on })} className="check" aria-label={t('Use the turns of {label}', { label: g.label })} isDisabled={g.turns === 0}>
                    <Box />
                  </Checkbox>
                </td>
                <th scope="row">
                  {g.label}
                  {others.length > 0 ? <span className="ac-variants"> {t('Also written: {list}', { list: others.join(', ') })}</span> : null}
                </th>
                <td className="ex-num">
                  {num(g.turns)}
                  {g.left_out > 0 ? <span className="ac-variants"> {t('({left_out} left out of coding)', { left_out: num(g.left_out) })}</span> : null}
                </td>
                <td>
                  <CodeSelect
                    id={`ac-sp-${g.key.replace(/[^a-z0-9]+/gi, '-')}`}
                    label={t('Code for {label}', { label: g.label })}
                    quietLabel
                    value={{ mode: v.mode, codeId: v.codeId, newCode: v.newCode }}
                    onChange={(p) => set(g.key, { mode: p.mode, codeId: p.codeId, newCode: p.newCode })}
                  />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
