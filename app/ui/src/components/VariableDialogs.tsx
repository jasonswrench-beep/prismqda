import { useEffect, useMemo, useRef, useState } from 'react';
import { Button, Dialog, Heading, Modal, ModalOverlay } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { LEVEL_LABELS, LEVELS } from '../lib/importMapping';
import {
  RESULT_LEVELS,
  codesToText,
  dateParts,
  formulaName,
  groupFunctions,
  kindLabel,
  labelsToText,
  parseCodes,
  parseCuts,
  parseLabels,
  parseValueList,
  specProblem,
  transformKinds,
} from '../lib/variables';
import { bumpData } from '../state/actions';
import { announce } from '../state/announce';
import type { CaseData, DatePartId, GroupFunctionId, Level, RecodeOtherwise, TransformKindId, TransformOutcome, TransformSpec, VariableDef } from '../types';
import { Notice } from './common';
import { Tick } from './Tick';
import { t, tn } from '../i18n';

// ---------------------------------------------------------------- one variable: add it or change what it says

/** Adds a variable of cases, or changes the name, level, labels, and missing values of one. Its values are typed in the data view. */
export function VariableDialog({ existing, onClose }: { existing: VariableDef | null; onClose: () => void }) {
  const [name, setName] = useState(existing?.name ?? '');
  const [level, setLevel] = useState<Level>(existing?.level ?? 'nominal');
  const [labels, setLabels] = useState(labelsToText(existing?.value_labels ?? {}));
  const [codes, setCodes] = useState(codesToText(existing?.missing_codes ?? []));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setError(null);
    const trimmed = name.trim();
    if (trimmed === '') return setError(t('Give the variable a name.'));
    const l = parseLabels(labels);
    if (l.error) return setError(l.error);
    const c = parseCodes(codes);
    if (c.error) return setError(c.error);
    setBusy(true);
    try {
      await call('variable.define', { name: trimmed, level, value_labels: l.labels, missing_codes: c.codes, ...(existing ? { existing: existing.name } : {}) });
      bumpData();
      announce(existing ? t('Saved the variable {name}.', { name: trimmed }) : t('Added the variable {name}. Its values are empty until you type them in the data view.', { name: trimmed }));
      onClose();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && onClose()} className="overlay">
      <Modal className="modal">
        <Dialog className="dialog" aria-labelledby="var-dialog-title">
          <Heading slot="title" id="var-dialog-title" className="dialog-title">
            {existing ? t('Change the variable {name}', { name: existing.name }) : t('Add a variable')}
          </Heading>
          {error ? <Notice>{error}</Notice> : null}
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
            noValidate
          >
            <div className="field">
              <label htmlFor="var-name">{t('Name')}</label>
              <input id="var-name" type="text" value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" autoFocus data-testid="var-name" />
            </div>
            <div className="field">
              <label htmlFor="var-level">{t('Level of measurement')}</label>
              <select id="var-level" value={level} onChange={(e) => setLevel(e.target.value as Level)} data-testid="var-level">
                {LEVELS.map((l) => (
                  <option key={l} value={l}>
                    {LEVEL_LABELS[l]}
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="var-labels">{t('Value labels')}</label>
              <textarea id="var-labels" rows={4} value={labels} onChange={(e) => setLabels(e.target.value)} aria-describedby="var-labels-help" data-testid="var-labels" />
              <p id="var-labels-help" className="help">{t('One to a line, as a value, an equals sign, then the words: 1 = Agree. The data view shows the words and keeps the value.')}</p>
            </div>
            <div className="field">
              <label htmlFor="var-missing">{t('Values that mean missing')}</label>
              <input id="var-missing" type="text" value={codes} onChange={(e) => setCodes(e.target.value)} autoComplete="off" aria-describedby="var-missing-help" data-testid="var-missing" />
              <p id="var-missing-help" className="help">{t('Up to three, separated by commas, such as 99, -1. Statistics leaves these out, as it leaves out an empty value.')}</p>
            </div>
            <div className="actions">
              <Button type="submit" className="primary" isDisabled={busy} data-testid="var-save">
                {existing ? t('Save') : t('Add variable')}
              </Button>
              <Button type="button" onPress={onClose}>{t('Cancel')}</Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

// ---------------------------------------------------------------- deleting one

export function DeleteVariableDialog({ variable, onClose }: { variable: VariableDef; onClose: () => void }) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const remove = async () => {
    setBusy(true);
    try {
      await call('variable.delete', { name: variable.name });
      bumpData();
      announce(t('Deleted the variable {name}. Undo brings it back with its values.', { name: variable.name }));
      onClose();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };
  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && onClose()} className="overlay">
      <Modal className="modal">
        <Dialog role="alertdialog" className="dialog" aria-labelledby="var-delete-title">
          <Heading slot="title" id="var-delete-title" className="dialog-title">{t('Delete the variable {name}?', { name: variable.name })}</Heading>
          {error ? <Notice>{error}</Notice> : null}
          <p>{t('Every value of it is removed. Undo brings the variable back with its values.')}</p>
          <div className="actions">
            <Button className="primary" onPress={() => void remove()} isDisabled={busy} autoFocus data-testid="var-delete-confirm">{t('Delete it')}</Button>
            <Button onPress={onClose}>{t('Keep it')}</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

// ---------------------------------------------------------------- making a variable from others

interface RuleRow {
  from: string;
  to: string;
}

const NUMBER_LEVELS: Level[] = ['interval', 'ratio', 'ordinal'];

/** The first few different values of a variable, so a recoding can be written without looking them up. */
function distinctValues(data: CaseData, name: string, max = 14): string[] {
  const at = data.variables.findIndex((v) => v.name === name);
  if (at < 0) return [];
  const seen: string[] = [];
  for (const row of data.values) {
    const v = row[at];
    if (v !== null && v !== '' && !seen.includes(v)) {
      seen.push(v);
      if (seen.length > max) break;
    }
  }
  return seen;
}

export function TransformDialog({ kind: startKind, data, onClose }: { kind: TransformKindId; data: CaseData; onClose: () => void }) {
  const [kind, setKind] = useState<TransformKindId>(startKind);
  const [target, setTarget] = useState('');
  const [replace, setReplace] = useState(false);
  const [source, setSource] = useState('');
  const [formula, setFormula] = useState('');
  const [rules, setRules] = useState<RuleRow[]>([{ from: '', to: '' }]);
  const [otherwise, setOtherwise] = useState<RecodeOtherwise>('keep');
  const [descending, setDescending] = useState(false);
  const [cutsText, setCutsText] = useState('');
  const [right, setRight] = useState(true);
  const [levelChoice, setLevelChoice] = useState<Level | ''>('');
  const [part, setPart] = useState<DatePartId>('year');
  const [sampleBy, setSampleBy] = useState<'count' | 'percent'>('count');
  const [sampleValue, setSampleValue] = useState('');
  const [seed, setSeed] = useState('1');
  const [within, setWithin] = useState('');
  const [aggFunction, setAggFunction] = useState<GroupFunctionId>('mean');
  const [groupBy, setGroupBy] = useState<string[]>([]);
  const [outcome, setOutcome] = useState<TransformOutcome | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const formulaRef = useRef<HTMLTextAreaElement>(null);

  const kinds = transformKinds();
  const info = kinds.find((k) => k.id === kind) ?? kinds[0];
  const clash = data.variables.some((v) => v.name.toLowerCase() === target.trim().toLowerCase());
  const countsOnly = groupFunctions().find((f) => f.id === aggFunction)?.counts ?? false;
  const sources = useMemo(
    () =>
      data.variables.filter((v) => {
        if (kind === 'recode') return true;
        if (kind === 'date_part') return v.level === 'date';
        if (kind === 'aggregate' && countsOnly) return true;
        return NUMBER_LEVELS.includes(v.level);
      }),
    [data, kind, countsOnly],
  );

  // What the engine is asked, or why it cannot be asked yet.
  const built = useMemo<{ spec: TransformSpec | null; problem: string | null }>(() => {
    const base = { target: target.trim(), level: levelChoice === '' ? null : levelChoice };
    let spec: TransformSpec;
    switch (kind) {
      case 'compute':
        spec = { ...base, kind, formula };
        break;
      case 'recode':
        spec = { ...base, kind, source, rules: rules.map((r) => ({ from: parseValueList(r.from), to: r.to.trim() })).filter((r) => r.from.length > 0), otherwise };
        break;
      case 'standardize':
        spec = { ...base, kind, source };
        break;
      case 'rank':
        spec = { ...base, kind, source, descending };
        break;
      case 'cut': {
        const c = parseCuts(cutsText);
        if (c.error) return { spec: null, problem: cutsText.trim() === '' ? specProblem({ ...base, kind, source, cuts: [] }) : c.error };
        spec = { ...base, kind, source, cuts: c.cuts, right };
        break;
      }
      case 'date_part':
        spec = { ...base, kind, source, part };
        break;
      case 'sample': {
        const value = Number(sampleValue);
        const seedNumber = Number(seed);
        if (sampleValue.trim() === '' || !Number.isFinite(value)) return { spec: null, problem: sampleBy === 'count' ? t('Write how many cases the sample holds.') : t('Write the percent of the cases the sample holds.') };
        if (seed.trim() === '' || !Number.isInteger(seedNumber) || seedNumber < 0) return { spec: null, problem: t('The seed is a whole number from 0 up, such as 1.') };
        spec = { ...base, kind, size: { by: sampleBy, value }, seed: seedNumber, within: within === '' ? null : within };
        break;
      }
      case 'aggregate':
        spec = { ...base, kind, source, by: groupBy, function: aggFunction };
        break;
    }
    const problem = specProblem(spec);
    return { spec: problem ? null : spec, problem };
  }, [kind, target, source, formula, rules, otherwise, descending, cutsText, right, levelChoice, part, sampleBy, sampleValue, seed, within, aggFunction, groupBy]);

  // The effect of the choices so far, worked out by the engine and not saved, a moment after the typing stops.
  const key = built.spec ? JSON.stringify(built.spec) : '';
  useEffect(() => {
    if (!built.spec) {
      setOutcome(null);
      setPreviewError(null);
      return;
    }
    let live = true;
    const spec = { ...built.spec, replace: true };
    const timer = setTimeout(() => {
      call('variable.preview', spec)
        .then((o) => {
          if (!live) return;
          setOutcome(o);
          setPreviewError(null);
        })
        .catch((e) => {
          if (!live) return;
          setOutcome(null);
          setPreviewError(toApiError(e).message);
        });
    }, 300);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // The key stands for the spec.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const insertName = (name: string) => {
    const el = formulaRef.current;
    const text = formulaName(name);
    if (!el) return setFormula((f) => f + text);
    const from = el.selectionStart ?? formula.length;
    const to = el.selectionEnd ?? formula.length;
    const next = formula.slice(0, from) + text + formula.slice(to);
    setFormula(next);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(from + text.length, from + text.length);
    });
  };

  const canMake = built.spec !== null && previewError === null && outcome !== null && (!clash || replace) && !busy;
  const why = built.problem ?? (clash && !replace ? t('A variable named {name} exists. Tick the box to replace it, or give the new variable another name.', { name: target.trim() }) : previewError);

  const make = async () => {
    if (!built.spec) return;
    setError(null);
    setBusy(true);
    try {
      const done = await call('variable.transform', { ...built.spec, replace });
      bumpData();
      announce(
        tn(done.cases_with_value, { one: '{kind}: made {name}. One case has a value.', other: '{kind}: made {name}. {count} cases have a value.' }, { kind: kindLabel(kind), name: done.variable }) +
          (done.cases_missing > 0 ? ' ' + tn(done.cases_missing, { one: 'One case is missing.', other: '{count} cases are missing.' }) : ''),
      );
      onClose();
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  const setRule = (i: number, patch: Partial<RuleRow>) => setRules(rules.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const known = source ? distinctValues(data, source) : [];
  const shownValue = (v: string | null) => (v === null ? t('missing') : (outcome?.value_labels[v] ?? v));

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && onClose()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog ve-transform" aria-labelledby="tf-title">
          <Heading slot="title" id="tf-title" className="dialog-title">{t('Make a variable: {kind}', { kind: info.label })}</Heading>
          <p className="help">{info.description}</p>
          {error ? <Notice>{error}</Notice> : null}
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (canMake) void make();
            }}
            noValidate
          >
            <div className="field">
              <label htmlFor="tf-kind">{t('What to do')}</label>
              <select
                id="tf-kind"
                value={kind}
                onChange={(e) => {
                  setKind(e.target.value as TransformKindId);
                  setSource('');
                }}
                data-testid="tf-kind"
              >
                {kinds.map((k) => (
                  <option key={k.id} value={k.id}>
                    {k.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="tf-target">{t('Name of the new variable')}</label>
              <input id="tf-target" type="text" value={target} onChange={(e) => setTarget(e.target.value)} autoComplete="off" autoFocus data-testid="tf-target" />
            </div>
            {clash && target.trim() !== '' ? (
              <Tick isSelected={replace} onChange={setReplace} testId="tf-replace">
                {t('Replace the variable named {name} (Undo puts it back)', { name: target.trim() })}
              </Tick>
            ) : null}

            {info.needsSource ? (
              <div className="field">
                <label htmlFor="tf-source">{kind === 'recode' ? t('Variable to recode') : kind === 'date_part' ? t('Variable of dates') : kind === 'aggregate' ? t('Variable to summarize') : t('Variable to use')}</label>
                <select id="tf-source" value={source} onChange={(e) => setSource(e.target.value)} data-testid="tf-source">
                  <option value="">{t('Choose a variable')}</option>
                  {sources.map((v) => (
                    <option key={v.id} value={v.name}>
                      {v.name}
                    </option>
                  ))}
                </select>
              </div>
            ) : null}

            {kind === 'compute' ? (
              <div className="field">
                <label htmlFor="tf-formula">{t('Formula')}</label>
                <textarea id="tf-formula" ref={formulaRef} rows={3} value={formula} onChange={(e) => setFormula(e.target.value)} spellCheck={false} aria-describedby="tf-formula-help" data-testid="tf-formula" />
                <p id="tf-formula-help" className="help">{t('Write it as R does: score + age, ifelse(age >= 30, 1, 0), round(score / 3, 1). A name with a space goes in backticks.')}</p>
                {data.variables.length > 0 ? (
                  <div className="ve-names" role="group" aria-label={t('Insert the name of a variable')}>
                    {data.variables.map((v) => (
                      <Button key={v.id} className="ve-chip" onPress={() => insertName(v.name)}>
                        {v.name}
                      </Button>
                    ))}
                  </div>
                ) : null}
                <details className="ve-reference">
                  <summary>{t('What a formula can say')}</summary>
                  <ul>
                    <li>{t('Arithmetic: + - * / ^ and %% (remainder) and %/% (whole division).')}</li>
                    <li>{t('Comparing: == != < <= > >=. Joining: & (and), | (or), ! (not). Being one of a list: age %in% c(18, 19, 20).')}</li>
                    <li>{t('Functions: abs, sqrt, exp, log (log(x, base) too), sin, cos, tan, floor, ceiling, trunc, round(x, digits), pmin, pmax, ifelse(test, yes, no), is.na, as.numeric.')}</li>
                    <li>{t('Across the variables of one case: rowmean, rowsum, rowmin, rowmax, and nmiss (how many are missing).')}</li>
                    <li>{t('NA is a missing value. Anything worked out from a missing value is missing, as in R.')}</li>
                  </ul>
                </details>
              </div>
            ) : null}

            {kind === 'recode' ? (
              <fieldset className="stats-fieldset" data-testid="tf-rules">
                <legend>{t('Rules')}</legend>
                {known.length > 0 ? <p className="help">{t('Values in {source}: {values}', { source, values: known.slice(0, 12).join(', ') + (known.length > 12 ? '…' : '') })}</p> : null}
                {rules.map((r, i) => (
                  <div className="ve-rule" key={i} data-testid="tf-rule">
                    <div className="field">
                      <label htmlFor={`tf-from-${i}`}>{t('Rule {n}: these values', { n: i + 1 })}</label>
                      <input id={`tf-from-${i}`} type="text" value={r.from} onChange={(e) => setRule(i, { from: e.target.value })} autoComplete="off" />
                    </div>
                    <div className="field">
                      <label htmlFor={`tf-to-${i}`}>{t('Rule {n}: become', { n: i + 1 })}</label>
                      <input id={`tf-to-${i}`} type="text" value={r.to} onChange={(e) => setRule(i, { to: e.target.value })} autoComplete="off" />
                    </div>
                    {rules.length > 1 ? (
                      <Button onPress={() => setRules(rules.filter((_, j) => j !== i))} aria-label={t('Remove rule {n}', { n: i + 1 })}>{t('Remove')}</Button>
                    ) : null}
                  </div>
                ))}
                <p className="help">{t('Write several values separated by commas. Leave the new value empty to make those values missing.')}</p>
                <div className="actions">
                  <Button onPress={() => setRules([...rules, { from: '', to: '' }])} data-testid="tf-add-rule">{t('Add a rule')}</Button>
                </div>
                <div className="field">
                  <label htmlFor="tf-otherwise">{t('A value no rule names')}</label>
                  <select id="tf-otherwise" value={otherwise} onChange={(e) => setOtherwise(e.target.value as RecodeOtherwise)} data-testid="tf-otherwise">
                    <option value="keep">{t('Stays as it is')}</option>
                    <option value="missing">{t('Becomes missing')}</option>
                  </select>
                </div>
              </fieldset>
            ) : null}

            {kind === 'date_part' ? (
              <div className="field">
                <label htmlFor="tf-part">{t('Part of the date')}</label>
                <select id="tf-part" value={part} onChange={(e) => setPart(e.target.value as DatePartId)} data-testid="tf-part">
                  {dateParts().map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.label}
                    </option>
                  ))}
                </select>
                <p className="help">{t('The variable must have the level date, and its values are written year-month-day, like 2024-03-14, with a time after it if you want the hour. A value written another way has no parts.')}</p>
              </div>
            ) : null}

            {kind === 'sample' ? (
              <>
                <div className="field">
                  <label htmlFor="tf-sample-by">{t('The sample holds')}</label>
                  <select id="tf-sample-by" value={sampleBy} onChange={(e) => setSampleBy(e.target.value as 'count' | 'percent')} data-testid="tf-sample-by">
                    <option value="count">{t('this many cases')}</option>
                    <option value="percent">{t('this percent of the cases')}</option>
                  </select>
                </div>
                <div className="field">
                  <label htmlFor="tf-sample-value">{sampleBy === 'count' ? t('Number of cases') : t('Percent of the cases')}</label>
                  <input id="tf-sample-value" type="text" inputMode="decimal" value={sampleValue} onChange={(e) => setSampleValue(e.target.value)} autoComplete="off" data-testid="tf-sample-value" />
                </div>
                <div className="field">
                  <label htmlFor="tf-within">{t('Draw within the groups of')}</label>
                  <select id="tf-within" value={within} onChange={(e) => setWithin(e.target.value)} data-testid="tf-within">
                    <option value="">{t('No groups: draw from all the cases')}</option>
                    {data.variables.filter((v) => v.level !== 'text').map((v) => (
                      <option key={v.id} value={v.name}>
                        {v.name}
                      </option>
                    ))}
                  </select>
                  <p className="help">{t('Drawn within groups, a percent takes its share of every group, so a small group is not left out by chance. A case with no value in the variable cannot be drawn.')}</p>
                </div>
                <div className="field">
                  <label htmlFor="tf-seed">{t('Seed')}</label>
                  <input id="tf-seed" type="text" inputMode="numeric" value={seed} onChange={(e) => setSeed(e.target.value)} autoComplete="off" aria-describedby="tf-seed-help" data-testid="tf-seed" />
                  <p id="tf-seed-help" className="help">{t('The same seed over the same cases always draws the same sample. Write the seed in your methods so the sample can be drawn again; a different seed draws a different sample.')}</p>
                </div>
              </>
            ) : null}

            {kind === 'aggregate' ? (
              <>
                <div className="field">
                  <label htmlFor="tf-function">{t('What to work out for each group')}</label>
                  <select id="tf-function" value={aggFunction} onChange={(e) => { setAggFunction(e.target.value as GroupFunctionId); setSource(''); }} data-testid="tf-function">
                    {groupFunctions().map((f) => (
                      <option key={f.id} value={f.id}>
                        {f.label}
                      </option>
                    ))}
                  </select>
                </div>
                <fieldset className="stats-fieldset" data-testid="tf-group-by">
                  <legend>{t('The groups are the cases that share a value of')}</legend>
                  {data.variables.filter((v) => v.level !== 'text').map((v) => (
                    <Tick key={v.id} isSelected={groupBy.includes(v.name)} onChange={(on) => setGroupBy(on ? [...groupBy, v.name] : groupBy.filter((n) => n !== v.name))}>
                      {v.name}
                    </Tick>
                  ))}
                  <p className="help">{t('A case with no value in one of these variables is in no group and gets no value. With more than one variable, a group is the cases that share all of them.')}</p>
                </fieldset>
              </>
            ) : null}

            {kind === 'rank' ? (
              <Tick isSelected={descending} onChange={setDescending} testId="tf-descending">
                {t('Rank the highest value first (place 1 for the highest)')}
              </Tick>
            ) : null}

            {kind === 'cut' ? (
              <>
                <div className="field">
                  <label htmlFor="tf-cuts">{t('Cut points')}</label>
                  <input id="tf-cuts" type="text" value={cutsText} onChange={(e) => setCutsText(e.target.value)} autoComplete="off" aria-describedby="tf-cuts-help" data-testid="tf-cuts" />
                  <p id="tf-cuts-help" className="help">{t('Numbers where one range ends and the next begins, including the lowest and highest you want, such as 18, 30, 50, 100. A range written (30, 50] starts after 30 and includes 50.')}</p>
                </div>
                <Tick isSelected={right} onChange={setRight} testId="tf-right">
                  {t('A value on a cut point goes in the range below it')}
                </Tick>
              </>
            ) : null}

            <div className="field">
              <label htmlFor="tf-level">{t('Level of the new variable')}</label>
              <select id="tf-level" value={levelChoice} onChange={(e) => setLevelChoice(e.target.value as Level | '')} data-testid="tf-level">
                <option value="">{t('Let PrismQDA choose')}</option>
                {RESULT_LEVELS.map((l) => (
                  <option key={l} value={l}>
                    {LEVEL_LABELS[l]}
                  </option>
                ))}
              </select>
            </div>

            <section className="ve-preview" aria-labelledby="tf-preview-h" data-testid="tf-preview">
              <h3 id="tf-preview-h" className="stats-h4">{t('What it would make')}</h3>
              <div aria-live="polite" aria-atomic="true">
                {outcome && !previewError ? (
                  <>
                    <p data-testid="tf-preview-summary">
                      {tn(outcome.cases_with_value, { one: '{count} case gets a value', other: '{count} cases get a value' })}
                      {outcome.cases_missing > 0 ? `, ${tn(outcome.cases_missing, { one: '{count} case is missing', other: '{count} cases are missing' })}` : ''}
                      {'. '}
                      {t('Level: {level}.', { level: LEVEL_LABELS[outcome.level] })}
                    </p>
                    {outcome.preview.length > 0 ? (
                      <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The first cases and the values they would get')}>
                        <table className="data">
                          <caption className="visually-hidden">{t('The first cases and the values they would get')}</caption>
                          <thead>
                            <tr>
                              <th scope="col">{t('Case')}</th>
                              <th scope="col">{t('New value')}</th>
                            </tr>
                          </thead>
                          <tbody>
                            {outcome.preview.map((p, i) => (
                              <tr key={i}>
                                <th scope="row">{p.case_name}</th>
                                <td>{shownValue(p.value)}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    ) : null}
                  </>
                ) : previewError ? (
                  <p className="ve-problem" data-testid="tf-preview-error">{previewError}</p>
                ) : (
                  <p className="muted">{built.problem ?? ''}</p>
                )}
              </div>
            </section>

            <div className="actions">
              <Button type="submit" className="primary" isDisabled={!canMake} data-testid="tf-make">{t('Make the variable')}</Button>
              <Button type="button" onPress={onClose}>{t('Cancel')}</Button>
            </div>
            {!canMake && why ? <p className="muted" data-testid="tf-why">{why}</p> : null}
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
