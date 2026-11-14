import { useEffect, useMemo, useState } from 'react';
import { Button, Radio, RadioGroup, Tab, TabList, TabPanel, Tabs } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { LEVEL_LABELS } from '../lib/importMapping';
import { num, t, tn } from '../i18n';
import { goTo, refreshEverything } from '../state/actions';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type { CodeMeasure, QualitizeOutcome, QualitizePlan, QualitizeTakeBack, TransformOutcome, TransformSpec } from '../types';
import { Notice } from './common';
import { CheckPicker, codeChoices } from './explore/shared';
import { useCaseVariables } from './explore/Matrix';
import { Tick } from './Tick';
import { JointDisplays } from './JointDisplays';
import { DesignAndIntegration, RecordInLog } from './MixedDesign';
import { useKept } from './jointShared';

/**
 * Mixed Methods (spec 14): moving between the two kinds of data. *Codes into variables* (quantitizing) makes a variable of cases from the
 * coding, which the Statistics page and a rule can then use like any other; *variables into codes* (qualitizing) makes a code for each value
 * of a variable and codes the text of each case with the code of its value, so a query can speak of the groups in the language of the coding.
 */
export function MixedMethodsView() {
  const [tab, setTab] = useState<'move' | 'joint' | 'design'>('move');
  /** The joint display that is shown; the design page can open one of them. */
  const [which, setWhich] = useKept('which', 'ordered');
  return (
    <main id="main-content" tabIndex={-1} className="view-page mixed-page" aria-labelledby="h-mixed">
      <h2 id="h-mixed">{t('Mixed Methods')}</h2>
      <p className="help">
        {t('Move between the two kinds of data: what the coding says becomes a number you can test, and what a variable says becomes a code you can search and compare. Joint displays put both on one page, and the design, the triangulation matrix, and the integration log keep how you mixed them. Everything made here is an ordinary variable or code, and nothing is hidden: the Data editor and the Codebook show it.')}
      </p>
      <Tabs selectedKey={tab} onSelectionChange={(k) => setTab(k as 'move' | 'joint' | 'design')}>
        <TabList aria-label={t('Parts of Mixed Methods')}>
          <Tab id="move" data-testid="mixed-tab-move">{t('Codes and variables')}</Tab>
          <Tab id="joint" data-testid="mixed-tab-joint">{t('Joint displays')}</Tab>
          <Tab id="design" data-testid="mixed-tab-design">{t('Design and integration')}</Tab>
        </TabList>
        <TabPanel id="move" shouldForceMount>
          <CodesToVariables />
          <VariablesToCodes />
        </TabPanel>
        <TabPanel id="joint" shouldForceMount>
          <JointDisplays which={which} onWhich={setWhich} />
        </TabPanel>
        <TabPanel id="design" shouldForceMount>
          <DesignAndIntegration
            onOpenDisplay={(id) => {
              setWhich(id);
              setTab('joint');
            }}
          />
        </TabPanel>
      </Tabs>
    </main>
  );
}

// ---------------------------------------------------------------- codes into variables

function CodesToVariables() {
  const codes = useApp((s) => s.codes);
  const subcodes = useApp((s) => s.settings.subcodes);
  const choices = useMemo(() => codeChoices(codes), [codes]);
  const [measure, setMeasure] = useState<CodeMeasure>('present');
  const [codeId, setCodeId] = useState<number | null>(null);
  const [codeIds, setCodeIds] = useState<number[]>([]);
  const [sub, setSub] = useState(subcodes);
  const [name, setName] = useState('');
  const [touched, setTouched] = useState(false);
  const [preview, setPreview] = useState<TransformOutcome | null>(null);
  const [made, setMade] = useState<TransformOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const chosen = measure === 'dominant' ? codeIds : codeId === null ? [] : [codeId];
  const nameOf = (id: number) => choices.find((c) => c.id === id)?.label ?? '';
  const suggested = useMemo(() => {
    if (chosen.length === 0) return '';
    if (measure === 'present') return t('Has {code}', { code: nameOf(chosen[0]) });
    if (measure === 'count') return t('{code} excerpts', { code: nameOf(chosen[0]) });
    return chosen.length <= 3 ? t('Most used of {codes}', { codes: chosen.map(nameOf).join(', ') }) : t('Most used of {n} codes', { n: num(chosen.length) });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [measure, codeId, codeIds, choices]);
  useEffect(() => {
    if (!touched) setName(suggested);
  }, [suggested, touched]);
  // What was shown is no longer what would be made once anything it depends on changes.
  useEffect(() => {
    setPreview(null);
    setMade(null);
  }, [measure, codeId, codeIds, sub, name]);

  const spec: TransformSpec = { kind: 'from_codes', target: name.trim(), codes: chosen, measure, include_descendants: sub };
  const problem =
    chosen.length === 0 ? (measure === 'dominant' ? t('Choose the codes to compare.') : t('Choose the code.')) : measure === 'dominant' && chosen.length < 2 ? t('Choose at least two codes to compare.') : name.trim() === '' ? t('Give the new variable a name.') : null;

  const run = async (make: boolean) => {
    setError(null);
    setBusy(true);
    try {
      if (make) {
        const outcome = await call('variable.transform', spec);
        setMade(outcome);
        await refreshEverything();
        announce(t('Made the variable {name}.', { name: spec.target }));
      } else setPreview(await call('variable.preview', spec));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const shown = made ?? preview;
  return (
    <section className="ac-card mixed-card" aria-labelledby="h-mixed-quant" data-testid="mixed-quantitize">
      <h3 id="h-mixed-quant" className="bk-h3">{t('Codes into variables')}</h3>
      <p className="muted">{t('Make a variable of cases from the coding. A case that holds no document has no value, since there is nothing to code.')}</p>
      <RadioGroup value={measure} onChange={(v) => setMeasure(v as CodeMeasure)} className="field ac-radios" aria-label={t('What the variable says')} data-testid="mixed-measure">
        <Radio value="present">
          <span className="radio-title">{t('Whether a case has a code')}</span>
          <span className="radio-help">{t('Yes or No: some excerpt of its documents carries the code.')}</span>
        </Radio>
        <Radio value="count">
          <span className="radio-title">{t('How many excerpts carry a code')}</span>
          <span className="radio-help">{t('A number for each case; the same words coded by two coders count once.')}</span>
        </Radio>
        <Radio value="dominant">
          <span className="radio-title">{t('Which of several codes a case has most')}</span>
          <span className="radio-help">{t('A category: the code carried by the most excerpts; codes that tie are named together, and a case with none is None.')}</span>
        </Radio>
      </RadioGroup>
      {measure === 'dominant' ? (
        <div className="field">
          <span className="ex-field-label" id="mixed-codes-label">{t('Codes to compare')}</span>
          <CheckPicker noun={t('codes')} label={t('Codes to compare')} choices={choices} value={codeIds} onChange={setCodeIds} testId="mixed-codes" />
        </div>
      ) : (
        <div className="field">
          <label htmlFor="mixed-code">{t('The code')}</label>
          <select id="mixed-code" value={codeId ?? ''} onChange={(e) => setCodeId(e.target.value === '' ? null : Number(e.target.value))} data-testid="mixed-code">
            <option value="">{t('Choose a code')}</option>
            {choices.map((c) => (
              <option key={c.id} value={c.id}>
                {'  '.repeat(c.depth ?? 0)}
                {c.label}
              </option>
            ))}
          </select>
        </div>
      )}
      <Tick isSelected={sub} onChange={setSub} testId="mixed-sub">{t('A code also stands for its sub-codes')}</Tick>
      <div className="field">
        <label htmlFor="mixed-name">{t('Name of the new variable')}</label>
        <input
          id="mixed-name"
          type="text"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            setTouched(true);
          }}
          autoComplete="off"
          data-testid="mixed-name"
        />
      </div>
      {error ? <Notice>{error}</Notice> : null}
      {problem ? <p className="muted" data-testid="mixed-why">{problem}</p> : null}
      <div className="actions">
        <Button isDisabled={problem !== null || busy} onPress={() => void run(false)} data-testid="mixed-preview">{t('Show what it would give')}</Button>
        <Button isDisabled={problem !== null || busy} onPress={() => void run(true)} data-testid="mixed-make">{t('Make the variable')}</Button>
      </div>
      {shown ? (
        <div data-testid="mixed-result" role="status">
          <p>
            {made
              ? t('Made the variable {name}. Its level is {level}.', { name: made.variable, level: LEVEL_LABELS[made.level] })
              : t('{name} would be made. Its level would be {level}.', { name: shown.variable, level: LEVEL_LABELS[shown.level] })}{' '}
            {made
              ? tn(made.cases_with_value, { one: '{count} case has a value.', other: '{count} cases have a value.' })
              : tn(shown.cases_with_value, { one: '{count} case would have a value.', other: '{count} cases would have a value.' })}
            {shown.cases_missing > 0 ? ` ${tn(shown.cases_missing, { one: '{count} case holds no document, so it has no value.', other: '{count} cases hold no document, so they have no value.' })}` : ''}
          </p>
          {shown.preview.length > 0 ? (
            <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The first values of {name}', { name: shown.variable })}>
              <table className="data apa">
                <caption>{t('The first {n} cases', { n: num(shown.preview.length) })}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Case')}</th>
                    <th scope="col">{t('Value')}</th>
                  </tr>
                </thead>
                <tbody>
                  {shown.preview.map((row, i) => (
                    <tr key={i}>
                      <th scope="row">{row.case_name}</th>
                      <td>{row.value === null ? '—' : (shown.value_labels[row.value] ?? row.value)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
          {made ? (
            <p className="muted">
              {t('It is a variable of cases like any other: the Statistics page and a rule can use it. It keeps how it was made, so *Make it again* in the Data editor follows the coding when the coding changes.')}{' '}
              <Button className="link-button" onPress={() => goTo('variables')} data-testid="mixed-to-editor">{t('Open the Data editor')}</Button>{' '}
              <RecordInLog what={t('Made the variable {name} from the coding of {codes}.', { name: made.variable, codes: chosen.map(nameOf).join(', ') })} testId="mixed-record-quant" />
            </p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

// ---------------------------------------------------------------- variables into codes

function VariablesToCodes() {
  const variables = useCaseVariables().filter((v) => v.level !== 'text' && v.values.length >= 1);
  const [variable, setVariable] = useState('');
  const [parent, setParent] = useState('');
  const [parentTouched, setParentTouched] = useState(false);
  const [ticked, setTicked] = useState<string[] | null>(null);
  const [plan, setPlan] = useState<QualitizePlan | null>(null);
  const [made, setMade] = useState<QualitizeOutcome | null>(null);
  const [tookBack, setTookBack] = useState<QualitizeTakeBack | null>(null);
  /** Counts what was made or taken back, so the plan is worked out again after it. */
  const [round, setRound] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!parentTouched) setParent(variable);
  }, [variable, parentTouched]);
  useEffect(() => {
    setTicked(null);
    setMade(null);
    setTookBack(null);
  }, [variable]);

  const wanted = ticked === null ? undefined : ticked;
  useEffect(() => {
    if (variable === '') {
      setPlan(null);
      return;
    }
    let live = true;
    setError(null);
    call('mixed.qualitize.preview', { variable, parent: parent.trim() === '' ? undefined : parent.trim(), values: wanted })
      .then((p) => live && setPlan(p))
      .catch((e) => {
        if (live) {
          setPlan(null);
          setError(toApiError(e).message);
        }
      });
    return () => {
      live = false;
    };
  }, [variable, parent, ticked, round]); // eslint-disable-line react-hooks/exhaustive-deps

  // The values the variable has, to tick from: the plan lists the ticked ones, so the choice comes from the variable itself.
  const all = variables.find((v) => v.name === variable)?.values ?? [];
  const current = ticked ?? all.map(([v]) => v);

  const make = async () => {
    setError(null);
    setBusy(true);
    try {
      const outcome = await call('mixed.qualitize', { variable, parent: parent.trim() === '' ? undefined : parent.trim(), values: wanted });
      setMade(outcome);
      setTookBack(null);
      await refreshEverything();
      announce(`${tn(outcome.code_ids.length, { one: 'Made {count} code.', other: 'Made {count} codes.' })} ${tn(outcome.codings, { one: 'It coded {count} place.', other: 'It coded {count} places.' })}`);
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const takeBack = async () => {
    if (!made) return;
    setError(null);
    setBusy(true);
    try {
      const back = await call('mixed.qualitize.take_back', { parent_id: made.parent_id, code_ids: made.code_ids, coding_ids: made.coding_ids });
      setTookBack(back);
      setMade(null);
      setRound((r) => r + 1);
      await refreshEverything();
      announce(`${tn(back.codings_removed, { one: 'Took back {count} coding.', other: 'Took back {count} codings.' })} ${tn(back.codes_retired, { one: 'Retired {count} code.', other: 'Retired {count} codes.' })}`);
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="ac-card mixed-card" aria-labelledby="h-mixed-qual" data-testid="mixed-qualitize">
      <h3 id="h-mixed-qual" className="bk-h3">{t('Variables into codes')}</h3>
      <p className="muted">{t('Make a code for each value of a variable of cases, under one parent code, and code the text of each case with the code of its value. A query or a matrix can then say "the cases in this group" in the language of the coding.')}</p>
      <div className="field">
        <label htmlFor="mixed-variable">{t('The variable')}</label>
        <select id="mixed-variable" value={variable} onChange={(e) => setVariable(e.target.value)} data-testid="mixed-variable">
          <option value="">{t('Choose a variable')}</option>
          {variables.map((v) => (
            <option key={v.name} value={v.name}>
              {v.name}
            </option>
          ))}
        </select>
      </div>
      {variable !== '' ? (
        <>
          <div className="field">
            <label htmlFor="mixed-parent">{t('Name of the parent code')}</label>
            <input
              id="mixed-parent"
              type="text"
              value={parent}
              onChange={(e) => {
                setParent(e.target.value);
                setParentTouched(true);
              }}
              autoComplete="off"
              data-testid="mixed-parent"
            />
          </div>
          <fieldset className="stats-fieldset" data-testid="mixed-values">
            <legend>{t('Values to make into codes')}</legend>
            {all.map(([value, count]) => (
              <Tick
                key={value}
                isSelected={current.includes(value)}
                onChange={(on) => setTicked(on ? [...current, value] : current.filter((x) => x !== value))}
                testId={`mixed-value-${value}`}
              >
                {value} <span className="muted">({tn(count, { one: '{count} case', other: '{count} cases' })})</span>
              </Tick>
            ))}
          </fieldset>
        </>
      ) : null}
      {error ? <Notice>{error}</Notice> : null}
      {plan ? (
        <div data-testid="mixed-plan" role="status">
          <p>
            {tn(plan.values.length, { one: 'This would make {count} code under {parent}.', other: 'This would make {count} codes under {parent}.' }, { parent: plan.parent })}{' '}
            {tn(plan.codings, { one: 'It would code {count} place.', other: 'It would code {count} places.' })}{' '}
            {plan.unit_mode ? t('A place is one unit of a document, since this project codes whole units.') : t('A place is a whole document, or the part of it that belongs to the case.')}
          </p>
          <ul>
            {plan.values.map((v) => (
              <li key={v.value}>
                {v.label}: {tn(v.cases, { one: '{count} case', other: '{count} cases' })}, {tn(v.places, { one: '{count} coding', other: '{count} codings' })}
              </li>
            ))}
          </ul>
          {plan.cases_without_text > 0 ? <p className="muted">{tn(plan.cases_without_text, { one: '{count} case with one of these values holds no document, so nothing of it can be coded.', other: '{count} cases with one of these values hold no document, so nothing of them can be coded.' })}</p> : null}
        </div>
      ) : null}
      <div className="actions">
        <Button isDisabled={variable === '' || plan === null || busy || made !== null} onPress={() => void make()} data-testid="mixed-qualitize-make">{t('Make the codes and code the text')}</Button>
      </div>
      {made ? (
        <p role="status" data-testid="mixed-qualitized">
          {tn(made.code_ids.length, { one: 'Made {count} code.', other: 'Made {count} codes.' })} {tn(made.codings, { one: 'It coded {count} place.', other: 'It coded {count} places.' })}{' '}
          {t('Nothing is hidden: the codes are in the Codebook, and each coding is in the history. If this is not what you wanted, you can take it back.')}{' '}
          <Button className="link-button" onPress={() => goTo('codebook')} data-testid="mixed-to-codebook">{t('Open the Codebook')}</Button>{' '}
          <RecordInLog what={t('Made the codes for the values of {variable} under {parent} and coded the text of each case with the code of its value.', { variable, parent: parent.trim() === '' ? variable : parent.trim() })} testId="mixed-record-qual" />
        </p>
      ) : null}
      {made ? (
        <div className="actions">
          <Button isDisabled={busy} onPress={() => void takeBack()} data-testid="mixed-qualitize-undo">{t('Take back what was just made')}</Button>
        </div>
      ) : null}
      {tookBack ? (
        <p role="status" data-testid="mixed-took-back">
          {tn(tookBack.codings_removed, { one: 'Took back {count} coding.', other: 'Took back {count} codings.' })} {tn(tookBack.codes_retired, { one: 'Retired {count} code.', other: 'Retired {count} codes.' })}{' '}
          {t('The retired codes stay in the Codebook, and each removal is in the history. A coding of those codes that you made by hand was left alone.')}
        </p>
      ) : null}
    </section>
  );
}
