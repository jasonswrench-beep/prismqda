import { useEffect, useMemo, useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import {
  assumptionLine,
  canBootstrap,
  categoryOptions,
  goalsInScope,
  isCategorical,
  chosenCasesLine,
  levelText,
  measureWord,
  methodsFor,
  methodsInScope,
  predictorAccepts,
  resultReading,
  resultSentence,
  resultTables,
  resultTitle,
  usable,
  usedLine,
  valueLabel,
  variableLabel,
  type Goal,
  type GoalInfo,
} from '../lib/statistics';
import { goTo, updateSettings } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import { analyses, loadNotebook, newSeed, removeFromNotebook, resetStats, runIntoNotebook, statsStore, useStats, type NotebookEntry } from '../state/statistics';
import type { StatAdjustment, StatOptions, StatPostHoc, StatProcedure, StatRequest, StatResult, StatVariable, StatVariables } from '../types';
import { Notice } from './common';
import { MoreOptions } from './StatOptionsMore';
import { StatExportDialog } from './StatExportDialog';
import { copyTable } from '../lib/statExport';
import { pageScript } from '../lib/rScript';
import { StatChartFigure } from './StatCharts';
import { Tick } from './Tick';
import { NoteCard, NotebookBar, PageTools, inSection } from './StatNotebookParts';
import { compareText, dateTime, num, t, tn } from '../i18n';

/**
 * Statistics: tests on the variables of cases. Rows are cases and only cases, so a person is never counted once for each of their
 * codings. On the left are the variables, in the middle the output notebook (one page for each analysis, with APA tables and a
 * results sentence), and on the right the chooser, which asks what is wanted and offers only the tests that are valid for the
 * variables chosen.
 */
export function StatisticsView() {
  const scope = useApp((s) => s.settings.statisticsScope);
  const project = useApp((s) => s.project);
  const cases = useApp((s) => s.cases);
  const codes = useApp((s) => s.codes);
  const path = project?.path ?? null;
  const version = useApp((s) => s.dataVersion);
  const [data, setData] = useState<StatVariables | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Another project starts another notebook.
  useEffect(() => {
    if (statsStore.get().projectPath !== path) {
      resetStats(path);
      loadNotebook(path).catch((e) => setError(toApiError(e).message));
    }
  }, [path]);

  useEffect(() => {
    let live = true;
    setError(null);
    call('stats.variables', {})
      .then((v) => {
        if (!live) return;
        setData(v);
        // The pages of the notebook read the variables (for the R code), so a list that has not changed is not set again: setting it would draw every page twice.
        if (JSON.stringify(statsStore.get().variables) !== JSON.stringify(v.variables)) statsStore.set({ variables: v.variables });
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [path, cases.length, codes.length, version]);

  return (
    <main id="main-content" tabIndex={-1} className="view-page stats-page" aria-labelledby="h-stats">
      <h2 id="h-stats">{t('Statistics')}</h2>
      <p className="help">
        {scope === 'tests'
          ? t('Tests on the variables of your cases. Each row is one case, a person or a site or a family, and never a coding: many codings come from one person, and a test would count that person many times. What a case said about a theme enters as one value, whether its documents carry the code, or on how many excerpts.')
          : t('Counts, shares, averages, spread, and charts for the variables of your cases. Each row is one case, a person or a site or a family, and never a coding: many codings come from one person. What a case said about a theme enters as one value, whether its documents carry the code, or on how many excerpts.')}
      </p>
      <ScopeCard />
      <div className="actions">
        <Button onPress={() => goTo('variables')} data-testid="stats-to-editor">{t('Open the data editor')}</Button>
      </div>
      {error ? <Notice>{error}</Notice> : null}
      {data && data.cases === 0 ? (
        <Notice>{t('This project has no cases yet. Statistics works on cases and the variables they carry; add cases when you import a spreadsheet, or on the Data page.')}</Notice>
      ) : null}
      <div className="stats-layout">
        <VariableList data={data} />
        <Notebook />
        <Chooser data={data} />
      </div>
    </main>
  );
}

// ---------------------------------------------------------------- describing here, testing in PrismStatistics

/**
 * Says what this page is for and where the rest is done (decision 193). PrismQDA describes what is in a study; the tests of difference,
 * association, and prediction are PrismStatistics's, which opens the case table this page's variables make. A person who wants the tests
 * here can ask for them, and the page offers them as before.
 */
function ScopeCard() {
  const scope = useApp((s) => s.settings.statisticsScope);
  return (
    <section className="ac-card stats-scope" aria-labelledby="h-stats-scope" data-testid="stats-scope">
      <h3 id="h-stats-scope" className="bk-h3">{t('Describing here, testing in PrismStatistics')}</h3>
      <p>
        {scope === 'descriptive'
          ? t('This page describes: counts and shares for categories, and the mean, spread, and shape of numbers, with charts. The tests of difference, association, and prediction are done in PrismStatistics. Make the case table, open it there, and the variables you see here, and a variable for every code, are ready to test.')
          : t('This page is also set to offer the tests of difference, association, and prediction. PrismStatistics does them with more to check and report, and opens the case table made from this project: the variables you see here, and a variable for every code.')}
      </p>
      <div className="actions">
        <Button onPress={() => appStore.set({ dialog: { kind: 'export', what: 'casetable' } })} data-testid="stats-send-table">{t('Make the case table for PrismStatistics')}</Button>
        {scope === 'descriptive' ? (
          <Button onPress={() => updateSettings({ statisticsScope: 'tests' })} data-testid="stats-show-tests">{t('Show the tests here too')}</Button>
        ) : (
          <Button onPress={() => updateSettings({ statisticsScope: 'descriptive' })} data-testid="stats-hide-tests">{t('Only describe here')}</Button>
        )}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------- the variables

function VariableList({ data }: { data: StatVariables | null }) {
  const [find, setFind] = useState('');
  const shown = useMemo(() => {
    const words = find.toLowerCase().split(/\s+/).filter(Boolean);
    return (data?.variables ?? []).filter((v) => {
      const hay = variableLabel(v).toLowerCase();
      return words.every((w) => hay.includes(w));
    });
  }, [data, find]);
  const attributes = shown.filter((v) => v.kind === 'attribute');
  const fromCodes = shown.filter((v) => v.kind !== 'attribute');
  return (
    <section className="ac-card stats-vars" aria-labelledby="h-stats-vars" data-testid="stats-variables">
      <h3 id="h-stats-vars" className="bk-h3">{t('Variables')}</h3>
      {data ? (
        <p className="muted" data-testid="stats-case-count">
          {tn(data.cases, { one: '{count} case', other: '{count} cases' })}
          {data.cases_with_documents < data.cases ? `. ${t('{n} have no document, so they have no code values.', { n: num(data.cases - data.cases_with_documents) })}` : ''}
        </p>
      ) : null}
      <div className="field">
        <label htmlFor="stats-find">{t('Find a variable')}</label>
        <input id="stats-find" type="search" value={find} onChange={(e) => setFind(e.target.value)} autoComplete="off" />
      </div>
      <VariableGroup title={t('Variables of cases')} variables={attributes} testId="stats-attributes" />
      <VariableGroup title={t('From the codes')} variables={fromCodes} testId="stats-codes" />
      {data && shown.length === 0 ? <p className="muted">{t('No variable matches.')}</p> : null}
    </section>
  );
}

function VariableGroup({ title, variables, testId }: { title: string; variables: StatVariable[]; testId: string }) {
  if (variables.length === 0) return null;
  return (
    <div data-testid={testId}>
      <h4 className="stats-h4">{title}</h4>
      <ul className="stats-var-list">
        {variables.map((v) => (
          <li key={v.key} data-testid="stats-var">
            <span className="stats-var-name">{variableLabel(v)}</span>
            <span className="stats-var-meta">
              {measureWord(v.measure)}, {tn(v.valid, { one: '{count} value', other: '{count} values' })}
              {v.missing > 0 ? `, ${t('{n} missing', { n: num(v.missing) })}` : ''}
              {v.not_a_number > 0 ? `, ${t('{n} not a number', { n: num(v.not_a_number) })}` : ''}
              {!usable(v) ? `. ${t('Not analyzed here.')}` : ''}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

// ---------------------------------------------------------------- the chooser

const LEVELS = [0.9, 0.95, 0.99];
const RESAMPLES = [1000, 2000, 5000, 10000];
/** What the pairs of groups after an analysis of variance can be compared by: three tests made for it, and the adjusted t tests. */
const PAIR_METHODS: { id: StatPostHoc | StatAdjustment; label: () => string; anovaOnly: boolean }[] = [
  { id: 'tukey', label: () => t("Tukey's honestly significant difference (groups of similar spread)"), anovaOnly: true },
  { id: 'games_howell', label: () => t("Games-Howell (groups of different spread or size)"), anovaOnly: true },
  { id: 'scheffe', label: () => t("Scheffé (the most cautious: holds for every comparison of groups)"), anovaOnly: true },
  { id: 'holm', label: () => t('Holm (never weaker than Bonferroni, and as safe)'), anovaOnly: false },
  { id: 'bonferroni', label: () => t('Bonferroni (each p times the number of pairs)'), anovaOnly: false },
  { id: 'benjamini_hochberg', label: () => t('Benjamini-Hochberg (controls the share of false findings)'), anovaOnly: false },
  { id: 'none', label: () => t('No adjustment'), anovaOnly: false },
];
const MAX_PREDICTORS = 20;

function Chooser({ data }: { data: StatVariables | null }) {
  const s = useStats((x) => x);
  const scope = useApp((x) => x.settings.statisticsScope);
  const info = goalsInScope(scope);
  const goal = info.find((g) => g.id === s.goal) ?? info[0];
  const byKey = useMemo(() => new Map((data?.variables ?? []).map((v) => [v.key, v])), [data]);
  const picked = goal.roles.map((r) => byKey.get(s.picks[r.id] ?? ''));
  const predicting = goal.id === 'predict' || goal.id === 'predict_binary';
  const predictors = predicting ? s.predictors.map((p) => ({ ...p, variable: byKey.get(p.key) })) : [];
  const chosenPredictors = predictors.filter((p): p is typeof p & { variable: StatVariable } => p.variable !== undefined);
  const listing = goal.list !== undefined;
  const chosenList = (listing ? s.list : []).map((key) => byKey.get(key)).filter((v): v is StatVariable => v !== undefined);
  const groupVar = goal.id === 'compare_groups' ? picked[1] : undefined;
  const allGroups = groupVar?.categories.map((c) => c.value) ?? [];
  const ticked = s.groups === null ? allGroups : s.groups.filter((g) => allGroups.includes(g));
  const methods = methodsInScope(scope, methodsFor(goal.id, picked, goal.id === 'compare_groups' ? ticked.length : listing ? chosenList.length : chosenPredictors.length));
  const method: StatProcedure | null = methods.find((m) => m.procedure === s.method)?.procedure ?? methods[0]?.procedure ?? null;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ruleCheck, setRuleCheck] = useState<{ text: string; bad: boolean } | null>(null);

  const complete = picked.every((v) => v !== undefined) && (!predicting || chosenPredictors.length > 0) && (!listing || chosenList.length >= (goal.list?.min ?? 1));
  const why = !complete
    ? t('Choose the variables first.')
    : method === null
      ? goal.id === 'compare_groups' && ticked.length < 2
        ? t('Tick at least two groups to compare.')
        : t('No test fits these variables.')
      : null;

  const set = (patch: Partial<typeof s>) => statsStore.set(patch);
  const chooseGoal = (id: Goal) => set({ goal: id, picks: {}, groups: null, method: null, predictors: [{ key: '', step: 1 }], list: [''] });
  const setPredictor = (i: number, patch: Partial<{ key: string; step: number }>) => set({ predictors: s.predictors.map((p, j) => (j === i ? { ...p, ...patch } : p)) });

  const run = async () => {
    if (!method || !complete) return;
    setError(null);
    setBusy(true);
    const options: StatOptions = { level: s.level };
    if (method === 'one_sample_t' || (method === 'wilcoxon' && goal.id === 'against_value')) options.mu = Number(s.mu) || 0;
    if (method === 'independent_t') options.equal_variances = s.equalVariances;
    if (method === 'one_way_anova' || method === 'kruskal_wallis') options.adjustment = s.adjustment;
    if (method === 'one_way_anova' && s.posthoc !== '') options.posthoc = s.posthoc;
    const first = picked[0];
    if (method === 'sign' && goal.id === 'against_value') options.mu = Number(s.mu) || 0;
    if (method === 'ks_normal') {
      options.mu = Number(s.mu) || 0;
      if (s.sd.trim() !== '') options.sd = Number(s.sd);
    }
    if (method === 'runs') {
      options.cut = s.cut;
      if (s.cut === 'value') options.mu = Number(s.mu) || 0;
    }
    if (method === 'binomial' && first) {
      options.proportion = Number(s.proportion);
      options.success = s.success !== '' ? s.success : first.categories[0]?.value;
    }
    if (method === 'curve_fit' && s.curves.length > 0 && s.curves.length < 8) options.curves = s.curves;
    if (method === 'logistic_regression' && first && s.success !== '' && first.categories.some((c) => c.value === s.success)) options.event = s.success;
    if (method === 'goodness_of_fit' && first && !s.equalShares) options.expected = Object.fromEntries(first.categories.map((c) => [c.value, Number(s.shares[c.value]) || 0]));
    if (method === 'moses') {
      if (s.control !== '') options.control = s.control;
      if (s.trim.trim() !== '') options.trim = Math.max(0, Math.floor(Number(s.trim)));
    }
    if (goal.id === 'compare_groups' && groupVar && ticked.length < allGroups.length) options.groups = ticked;
    const variables = predicting
      ? [(picked[0] as StatVariable).key, ...chosenPredictors.map((p) => p.variable.key)]
      : listing
        ? [...picked.map((v) => (v as StatVariable).key), ...chosenList.map((v) => v.key)]
        : picked.map((v) => (v as StatVariable).key);
    if (predicting) {
      if (s.useSteps) options.steps = chosenPredictors.map((p) => p.step);
      const references: Record<string, string> = {};
      for (const p of chosenPredictors) if (isCategorical(p.variable) && s.references[p.variable.key]) references[p.variable.key] = s.references[p.variable.key];
      if (Object.keys(references).length > 0) options.references = references;
    }
    if (s.filter.trim() !== '') options.filter = s.filter.trim();
    if (s.weight !== '') options.weight = s.weight;
    if (s.bootstrap) {
      const whole = s.bootstrapWhole || !canBootstrap(method, picked) || s.cluster !== '';
      options.bootstrap = { resamples: s.resamples, seed: Math.max(0, Math.floor(Number(s.seed)) || 1), ...(whole ? { analysis: true } : {}), ...(s.cluster !== '' ? { cluster: s.cluster } : {}) };
    }
    const request: StatRequest = { procedure: method, variables, options };
    try {
      if (s.split !== '') {
        // Split by groups: the same analysis on each group by itself, a page for each, kept together in a section named for the group.
        const plan = await call('stats.split_groups', { variable: s.split });
        const problems: string[] = [];
        let made = 0;
        for (const g of plan.groups) {
          const rule = options.filter ? `(${options.filter}) & (${g.rule})` : g.rule;
          try {
            await runIntoNotebook({ ...request, options: { ...options, filter: rule } }, `${s.split}: ${g.label}`);
            made++;
          } catch (e) {
            problems.push(`${g.label}: ${toApiError(e).message}`);
          }
        }
        announce(tn(made, { one: 'Ran the analysis on {count} group, a page for it.', other: 'Ran the analysis on {count} groups, a page for each.' }));
        if (problems.length > 0) setError(t('{count} groups could not be analyzed: {problems}', { count: problems.length, problems: problems.join(' ') }));
      } else {
        const page = await runIntoNotebook(request);
        announce(resultSentence(page.result));
      }
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const taken = new Set([...Object.values(s.picks), ...s.predictors.map((p) => p.key), ...s.list]);
  return (
    <section className="ac-card stats-chooser" aria-labelledby="h-stats-choose" data-testid="stats-chooser">
      <h3 id="h-stats-choose" className="bk-h3">{t('Test chooser')}</h3>
      {error ? <Notice>{error}</Notice> : null}
      <fieldset className="stats-fieldset">
        <legend>{t('What do you want to do?')}</legend>
        {info.map((g) => (
          <label key={g.id} className="stats-radio">
            <input type="radio" name="stats-goal" checked={g.id === goal.id} onChange={() => chooseGoal(g.id)} data-testid={`stats-goal-${g.id}`} />
            <span>
              <strong>{g.label}</strong>
              <span className="muted"> {g.description}</span>
            </span>
          </label>
        ))}
      </fieldset>

      {goal.roles.map((role, i) => (
        <div className="field" key={`${goal.id}-${role.id}`}>
          <label htmlFor={`stats-role-${role.id}`}>{role.label}</label>
          <select
            id={`stats-role-${role.id}`}
            data-testid={`stats-role-${role.id}`}
            value={s.picks[role.id] ?? ''}
            onChange={(e) => set({ picks: { ...s.picks, [role.id]: e.target.value }, groups: null, method: null })}
          >
            <option value="">{t('Choose a variable')}</option>
            {(data?.variables ?? [])
              .filter((v) => role.accepts(v) && !goal.roles.some((other, j) => j !== i && s.picks[other.id] === v.key) && !(predicting && s.predictors.some((p) => p.key === v.key)) && !(listing && s.list.includes(v.key)))
              .sort((a, b) => compareText(variableLabel(a), variableLabel(b)))
              .map((v) => (
                <option key={v.key} value={v.key}>
                  {variableLabel(v)} ({measureWord(v.measure)})
                </option>
              ))}
          </select>
        </div>
      ))}

      {goal.list ? (
        <fieldset className="stats-fieldset" data-testid="stats-list">
          <legend>{goal.list.label}</legend>
          {s.list.map((key, i) => (
            <div className="stats-predictor" key={i} data-testid="stats-list-item">
              <div className="field">
                <label htmlFor={`stats-list-${i}`}>{goal.list?.itemLabel(i + 1)}</label>
                <select id={`stats-list-${i}`} data-testid={`stats-list-${i}`} value={key} onChange={(e) => set({ list: s.list.map((k, j) => (j === i ? e.target.value : k)), method: null })}>
                  <option value="">{t('Choose a variable')}</option>
                  {(data?.variables ?? [])
                    .filter((x) => goal.list?.accepts(x) && (x.key === key || !taken.has(x.key)))
                    .sort((a, b) => compareText(variableLabel(a), variableLabel(b)))
                    .map((x) => (
                      <option key={x.key} value={x.key}>
                        {variableLabel(x)} ({measureWord(x.measure)})
                      </option>
                    ))}
                </select>
              </div>
              {s.list.length > 1 ? (
                <Button onPress={() => set({ list: s.list.filter((_, j) => j !== i), method: null })} aria-label={t('Remove {name}', { name: goal.list?.itemLabel(i + 1) ?? '' })}>
                  {t('Remove')}
                </Button>
              ) : null}
            </div>
          ))}
          <div className="actions">
            <Button onPress={() => set({ list: [...s.list, ''] })} isDisabled={s.list.length >= MAX_PREDICTORS} data-testid="stats-list-add">
              {goal.list.add}
            </Button>
          </div>
        </fieldset>
      ) : null}

      {predicting ? (
        <fieldset className="stats-fieldset" data-testid="stats-predictors">
          <legend>{t('Predictors')}</legend>
          {s.predictors.map((p, i) => {
            const v = byKey.get(p.key);
            return (
              <div className="stats-predictor" key={i} data-testid="stats-predictor">
                <div className="field">
                  <label htmlFor={`stats-predictor-${i}`}>{t('Predictor {n}', { n: num(i + 1) })}</label>
                  <select id={`stats-predictor-${i}`} data-testid={`stats-predictor-${i}`} value={p.key} onChange={(e) => setPredictor(i, { key: e.target.value })}>
                    <option value="">{t('Choose a variable')}</option>
                    {(data?.variables ?? [])
                      .filter((x) => predictorAccepts(x) && (x.key === p.key || !taken.has(x.key)))
                      .sort((a, b) => compareText(variableLabel(a), variableLabel(b)))
                      .map((x) => (
                        <option key={x.key} value={x.key}>
                          {variableLabel(x)} ({measureWord(x.measure)})
                        </option>
                      ))}
                  </select>
                </div>
                {v && isCategorical(v) ? (
                  <div className="field">
                    <label htmlFor={`stats-ref-${i}`}>{t('Compare the others with')}</label>
                    <select id={`stats-ref-${i}`} data-testid={`stats-ref-${i}`} value={s.references[v.key] ?? v.categories[0]?.value ?? ''} onChange={(e) => set({ references: { ...s.references, [v.key]: e.target.value } })}>
                      {v.categories.map((c) => (
                        <option key={c.value} value={c.value}>
                          {valueLabel(v.kind, c.value, c.label)}
                        </option>
                      ))}
                    </select>
                  </div>
                ) : null}
                {s.useSteps ? (
                  <div className="field">
                    <label htmlFor={`stats-step-${i}`}>{t('Enter in step')}</label>
                    <select id={`stats-step-${i}`} data-testid={`stats-step-${i}`} value={p.step} onChange={(e) => setPredictor(i, { step: Number(e.target.value) })}>
                      {Array.from({ length: Math.max(1, s.predictors.length) }, (_, k) => k + 1).map((k) => (
                        <option key={k} value={k}>
                          {num(k)}
                        </option>
                      ))}
                    </select>
                  </div>
                ) : null}
                {s.predictors.length > 1 ? (
                  <Button onPress={() => set({ predictors: s.predictors.filter((_, j) => j !== i) })} aria-label={t('Remove predictor {n}', { n: num(i + 1) })}>
                    {t('Remove')}
                  </Button>
                ) : null}
              </div>
            );
          })}
          <div className="actions">
            <Button onPress={() => set({ predictors: [...s.predictors, { key: '', step: 1 }] })} isDisabled={s.predictors.length >= MAX_PREDICTORS} data-testid="stats-add-predictor">
              {t('Add a predictor')}
            </Button>
          </div>
          <Tick isSelected={s.useSteps} onChange={(on) => set({ useSteps: on })} testId="stats-use-steps">
            {t('Enter the predictors in steps, to see what each step adds')}
          </Tick>
        </fieldset>
      ) : null}

      {groupVar && groupVar.categories.length > 2 ? (
        <fieldset className="stats-fieldset" data-testid="stats-groups">
          <legend>{t('Groups to compare')}</legend>
          {categoryOptions(groupVar).map((o) => (
            <Tick
              key={o.value}
              isSelected={ticked.includes(o.value)}
              onChange={(on) => set({ groups: on ? [...ticked, o.value] : ticked.filter((g) => g !== o.value), method: null })}
              testId={`stats-group-${o.value}`}
            >
              {o.label}
            </Tick>
          ))}
        </fieldset>
      ) : null}

      {complete ? (
        <fieldset className="stats-fieldset" data-testid="stats-methods">
          <legend>{t('Test')}</legend>
          {methods.length === 0 ? (
            <p className="muted">{why}</p>
          ) : (
            methods.map((m) => (
              <label key={m.procedure} className="stats-radio">
                <input type="radio" name="stats-method" checked={m.procedure === method} onChange={() => set({ method: m.procedure })} data-testid={`stats-method-${m.procedure}`} />
                <span>
                  <strong>{m.label}</strong>
                  <span className="muted"> {m.description}</span>
                </span>
              </label>
            ))
          )}
        </fieldset>
      ) : null}

      {method ? <Options goal={goal} method={method} canBoot={canBootstrap(method, picked)} variable={picked[0]} groupChoices={groupVar ? categoryOptions(groupVar).filter((o) => ticked.includes(o.value)) : []} /> : null}

      <fieldset className="stats-fieldset" data-testid="stats-filter">
        <legend>{t('Cases to use')}</legend>
        <div className="field">
          <label htmlFor="stats-filter-rule">{t('Only cases where')}</label>
          <input
            id="stats-filter-rule"
            type="text"
            value={s.filter}
            onChange={(e) => {
              set({ filter: e.target.value });
              setRuleCheck(null);
            }}
            autoComplete="off"
            spellCheck={false}
            aria-describedby="stats-filter-help"
            data-testid="stats-filter-rule"
          />
          <p id="stats-filter-help" className="help">{t('A rule written as R does, such as age >= 30 & group == "B". Leave it empty to use every case.')}</p>
        </div>
        <div className="actions">
          <Button isDisabled={s.filter.trim() === ''} onPress={() => void checkRule(s.filter, setRuleCheck)} data-testid="stats-filter-check">{t('Check the rule')}</Button>
        </div>
        <p role="status" className={ruleCheck?.bad ? 've-problem' : 'muted'} data-testid="stats-filter-result">{ruleCheck?.text ?? ''}</p>
        <div className="field">
          <label htmlFor="stats-weight">{t('Count each case as many times as')}</label>
          <select id="stats-weight" value={s.weight} onChange={(e) => set({ weight: e.target.value })} aria-describedby="stats-weight-help" data-testid="stats-weight">
            <option value="">{t('No weights: count every case once')}</option>
            {(data?.variables ?? []).filter((v) => v.kind === 'attribute' && v.measure === 'scale').map((v) => (
              <option key={v.key} value={v.name}>
                {v.name}
              </option>
            ))}
          </select>
          <p id="stats-weight-help" className="help">{t('A weight is how many times a case is counted: a whole number, 0 or more (a case with 0 or none is left out). The numbers are what you would get if each case were written out that many times, so a bootstrap cannot be added.')}</p>
        </div>
        <div className="field">
          <label htmlFor="stats-split">{t('Run it for each group of')}</label>
          <select id="stats-split" value={s.split} onChange={(e) => set({ split: e.target.value })} aria-describedby="stats-split-help" data-testid="stats-split">
            <option value="">{t('No split: run it once on all the cases')}</option>
            {(data?.variables ?? []).filter((v) => v.kind === 'attribute' && (v.measure === 'nominal' || v.measure === 'ordinal') && v.valid > 0 && v.distinct <= 30).map((v) => (
              <option key={v.key} value={v.name}>
                {v.name}
              </option>
            ))}
          </select>
          <p id="stats-split-help" className="help">{t('The analysis is run on each group by itself, and each group gets a page of its own in a section named for the group. A group the test cannot use is named after the others have run.')}</p>
        </div>
      </fieldset>

      <div className="actions">
        <Button className="primary" onPress={() => void run()} isDisabled={busy || why !== null} data-testid="stats-run">
          {busy ? t('Running…') : t('Run the test')}
        </Button>
      </div>
      {why ? <p className="muted" data-testid="stats-why">{why}</p> : null}
    </section>
  );
}

/** Asks the engine how many cases a rule picks, or why the rule cannot be used. */
async function checkRule(rule: string, show: (r: { text: string; bad: boolean }) => void): Promise<void> {
  try {
    const r = await call('variable.rule', { formula: rule.trim() });
    const text = r.matched === 0 ? t('The rule picks no cases.') : t('The rule picks {matched} of {total} cases.', { matched: num(r.matched), total: num(r.total) });
    show({ text, bad: r.matched === 0 });
    announce(text);
  } catch (e) {
    const message = toApiError(e).message;
    show({ text: message, bad: true });
    announce(message, 'assertive');
  }
}

/** The procedures with no interval of their own, for which a confidence level would mean nothing. */
const NO_LEVEL: StatProcedure[] = ['mann_whitney', 'kruskal_wallis', 'wilcoxon', 'shapiro_wilk', 'ks_normal', 'runs', 'goodness_of_fit', 'friedman', 'ks_two', 'median_test', 'moses', 'mcnemar', 'ordinal_association'];

function Options({ goal, method, canBoot, variable, groupChoices }: { goal: GoalInfo; method: StatProcedure; canBoot: boolean; variable?: StatVariable; groupChoices: { value: string; label: string }[] }) {
  const s = useStats((x) => x);
  const set = (patch: Partial<typeof s>) => statsStore.set(patch);
  const needsValue = method === 'one_sample_t' || ((method === 'wilcoxon' || method === 'sign') && goal.id === 'against_value');
  const hasInterval = !NO_LEVEL.includes(method);
  const posthoc = method === 'one_way_anova' || method === 'kruskal_wallis';
  const more = method === 'ks_normal' || method === 'runs' || method === 'binomial' || method === 'goodness_of_fit' || method === 'moses' || method === 'logistic_regression' || method === 'curve_fit';
  const clusters = s.variables.filter((v) => (v.measure === 'nominal' || v.measure === 'ordinal') && v.valid > 0 && v.distinct >= 5).sort((a, b) => compareText(variableLabel(a), variableLabel(b)));
  return (
    <div className="stats-options" data-testid="stats-options">
      <MoreOptions method={method} variable={variable} groupChoices={groupChoices} />
      {needsValue ? (
        <div className="field">
          <label htmlFor="stats-mu">{t('Compare with the value')}</label>
          <input id="stats-mu" type="number" step="any" value={s.mu} onChange={(e) => set({ mu: e.target.value })} data-testid="stats-mu" />
        </div>
      ) : null}
      {hasInterval || s.bootstrap ? (
        <div className="field">
          <label htmlFor="stats-level">{t('Confidence level of intervals')}</label>
          <select id="stats-level" value={s.level} onChange={(e) => set({ level: Number(e.target.value) })} data-testid="stats-level">
            {LEVELS.map((l) => (
              <option key={l} value={l}>
                {levelText(l)}
              </option>
            ))}
          </select>
        </div>
      ) : null}
      {method === 'independent_t' ? (
        <Tick isSelected={s.equalVariances} onChange={(on) => set({ equalVariances: on })} testId="stats-equal-variances">
          {t("Assume equal variances (Student's t test). Welch's test, which does not, is the default.")}
        </Tick>
      ) : null}
      {posthoc ? (
        <div className="field">
          <label htmlFor="stats-adjust">{method === 'one_way_anova' ? t('Compare pairs of groups by') : t('Adjust the p-values of pairs of groups by')}</label>
          <select
            id="stats-adjust"
            value={method === 'one_way_anova' && s.posthoc !== '' ? s.posthoc : s.adjustment}
            onChange={(e) => {
              const v = e.target.value;
              if (v === 'tukey' || v === 'games_howell' || v === 'scheffe') set({ posthoc: v });
              else set({ adjustment: v as StatAdjustment, posthoc: '' });
            }}
            data-testid="stats-adjust"
          >
            {PAIR_METHODS.filter((a) => method === 'one_way_anova' || !a.anovaOnly).map((a) => (
              <option key={a.id} value={a.id}>
                {a.label()}
              </option>
            ))}
          </select>
        </div>
      ) : null}
      {(
        <fieldset className="stats-fieldset" data-testid="stats-bootstrap">
          <legend>{t('Bootstrap')}</legend>
          <Tick isSelected={s.bootstrap} onChange={(on) => set({ bootstrap: on })} testId="stats-bootstrap-on">
            {t('Add bootstrap intervals: draw the cases again and again, with replacement, and see how much the result moves. They rest on fewer assumptions than the usual intervals.')}
          </Tick>
          {s.bootstrap ? (
            <>
              {canBoot ? (
                <fieldset className="stats-fieldset" data-testid="stats-bootstrap-what">
                  <legend>{t('What to bootstrap')}</legend>
                  <label className="stats-radio">
                    <input type="radio" name="stats-bootstrap-what" checked={!s.bootstrapWhole && s.cluster === ''} onChange={() => set({ bootstrapWhole: false })} disabled={s.cluster !== ''} data-testid="stats-bootstrap-statistic" />
                    <span>{t('The main statistic of this test (with the bias-corrected interval where it applies)')}</span>
                  </label>
                  <label className="stats-radio">
                    <input type="radio" name="stats-bootstrap-what" checked={s.bootstrapWhole || s.cluster !== ''} onChange={() => set({ bootstrapWhole: true })} data-testid="stats-bootstrap-whole" />
                    <span>{t('The whole analysis: run the test again on each resample, and give an interval for each of its numbers')}</span>
                  </label>
                </fieldset>
              ) : (
                <p className="help">{t('This test has no single statistic of its own to bootstrap, so the whole analysis is run again on each resample, and each of its numbers gets an interval.')}</p>
              )}
              <div className="field">
                <label htmlFor="stats-cluster">{t('Resample')}</label>
                <select id="stats-cluster" value={s.cluster} onChange={(e) => set({ cluster: e.target.value })} aria-describedby="stats-cluster-help" data-testid="stats-cluster">
                  <option value="">{t('Single cases, one at a time')}</option>
                  {clusters.map((v) => (
                    <option key={v.key} value={v.key}>
                      {t('Whole groups of cases that share a value of {name} ({count} groups)', { name: variableLabel(v), count: num(v.distinct) })}
                    </option>
                  ))}
                </select>
                <p id="stats-cluster-help" className="help">{t('Resampling whole groups keeps the cases that belong together (comments in one thread, people in one family) together, which gives wider and more honest intervals when cases in a group resemble one another. It runs the whole analysis.')}</p>
              </div>
              <div className="field">
                <label htmlFor="stats-resamples">{t('Resamples')}</label>
                <select id="stats-resamples" value={s.resamples} onChange={(e) => set({ resamples: Number(e.target.value) })} data-testid="stats-resamples">
                  {RESAMPLES.map((n) => (
                    <option key={n} value={n}>
                      {num(n)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label htmlFor="stats-seed">{t('Seed (the same seed gives the same intervals)')}</label>
                <input id="stats-seed" type="number" min="0" step="1" value={s.seed} onChange={(e) => set({ seed: e.target.value })} data-testid="stats-seed" />
              </div>
              <div className="actions">
                <Button onPress={() => set({ seed: String(newSeed()) })} data-testid="stats-new-seed">
                  {t('A new seed')}
                </Button>
              </div>
            </>
          ) : null}
        </fieldset>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- the notebook

function Notebook() {
  const notebook = useStats((s) => s.notebook);
  const shown = useStats((s) => s.sectionShown);
  const justAdded = useStats((s) => s.justAdded);
  const [exporting, setExporting] = useState(false);
  // Only the analyses are numbered, so a page of notes does not move the numbers the exports and the script give.
  const numbers = new Map(analyses(notebook).map((e, i) => [e.id, i + 1]));
  const visible = notebook.filter((e) => inSection(e, shown));
  return (
    <section className="stats-notebook" aria-labelledby="h-stats-notebook" data-testid="stats-notebook">
      <h3 id="h-stats-notebook" className="bk-h3">{t('Output notebook')}</h3>
      <p className="muted">{t('The notebook is kept in the project: close it and open it again, and the pages are here.')}</p>
      <NotebookBar items={notebook} onExport={() => setExporting(true)} />
      {exporting ? <StatExportDialog target={{ kind: 'notebook' }} onClose={() => setExporting(false)} /> : null}
      {notebook.length === 0 ? (
        <p className="muted" data-testid="stats-empty">
          {t('Nothing has been run yet. Choose what you want to do on the right; each analysis you run is added here as a page with its tables, a results sentence, and the conditions it depends on.')}
        </p>
      ) : (
        <ol className="stats-pages">
          {visible.map((e) =>
            e.kind === 'note' ? <NoteCard key={e.id} item={e} /> : <Page key={e.id} entry={e} number={numbers.get(e.id) ?? 0} fresh={e.id === justAdded} />,
          )}
        </ol>
      )}
    </section>
  );
}

function Page({ entry, number, fresh }: { entry: NotebookEntry; number: number; fresh: boolean }) {
  const r: StatResult = entry.result;
  const cases = useApp((s) => s.cases);
  const names = useMemo(() => new Map(cases.map((c) => [c.id, c.name])), [cases]);
  const head = useRef<HTMLHeadingElement>(null);
  // The page just added takes the focus, so a person who ran a test lands on its result.
  useEffect(() => {
    if (fresh) head.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entry.id]);
  const tables = resultTables(r);
  const chosenCases = chosenCasesLine(entry.request.options);
  const variables = useStats((s) => s.variables);
  const code = useMemo(() => {
    try {
      return pageScript(entry, number, new Map(variables.map((v) => [v.key, v])));
    } catch {
      return '';
    }
  }, [entry, number, variables]);
  return (
    <li id={`stats-page-${entry.id}`} className="ac-card stats-page-card" data-testid="stats-page" data-procedure={r.procedure}>
      <h4 ref={head} tabIndex={-1} className="stats-page-title" data-testid="stats-title">
        {t('Page {n}', { n: num(number) })}: {resultTitle(r)}
      </h4>
      <p className="muted">
        {usedLine(r)} {dateTime(new Date(entry.ranAt), { dateStyle: 'medium', timeStyle: 'short' })}
      </p>
      {entry.request.options?.filter ? (
        <p className="muted" data-testid="stats-page-filter">{t('Only cases where: {rule}', { rule: entry.request.options.filter })}</p>
      ) : null}
      {chosenCases ? (
        <p className="muted" data-testid="stats-page-cases">{chosenCases}</p>
      ) : null}
      {r.weight ? (
        <p className="muted" data-testid="stats-page-weight">
          {t('Each case is counted as many times as {variable} says: {cases} count as {counted}.', {
            variable: r.weight.variable,
            cases: tn(r.weight.cases, { one: '{count} case', other: '{count} cases' }),
            counted: num(r.weight.counted),
          })}
          {r.weight.left_out > 0 ? ` ${tn(r.weight.left_out, { one: '{count} case has no weight or a weight of 0 and is left out.', other: '{count} cases have no weight or a weight of 0 and are left out.' })}` : ''}
        </p>
      ) : null}
      {tables.map((tb, i) => (
        <div className="table-wrap" tabIndex={0} role="region" aria-label={t('{title}, page {n}', { title: tb.title, n: num(number) })} key={i}>
          <table className="data apa" data-testid="stats-table">
            <caption>{tb.title}</caption>
            <thead>
              <tr>
                {tb.head.map((h, j) => (
                  <th key={j} scope="col">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {tb.rows.map((row, ri) => (
                <tr key={ri}>
                  {row.map((cell, ci) => (ci === 0 ? <th key={ci} scope="row">{cell}</th> : <td key={ci}>{cell}</td>))}
                </tr>
              ))}
            </tbody>
          </table>
          {tb.note ? <p className="muted stats-note">{tb.note}</p> : null}
          <Button
            className="stats-copy"
            aria-label={t('Copy the table "{title}"', { title: tb.title })}
            data-testid="stats-copy-table"
            onPress={async () => announce((await copyTable(tb)) ? t('Copied the table "{title}".', { title: tb.title }) : t('The table could not be copied. Select it and copy it yourself.'))}
          >
            {t('Copy table')}
          </Button>
        </div>
      ))}
      <p className="stats-sentence" data-testid="stats-sentence">{resultSentence(r)}</p>
      {resultReading(r) ? <p className="stats-reading" data-testid="stats-reading">{resultReading(r)}</p> : null}
      {r.charts.length > 0 ? (
        <div data-testid="stats-charts">
          <h5 className="stats-h5">{t('Charts')}</h5>
          {r.charts.map((chart, i) => (
            <StatChartFigure key={i} chart={chart} result={r} id={`stats-chart-${entry.id}-${i}`} page={number} caseName={(id) => names.get(id) ?? String(id)} />
          ))}
        </div>
      ) : null}
      {r.assumptions.length > 0 ? (
        <div data-testid="stats-assumptions">
          <h5 className="stats-h5">{t('What the test depends on')}</h5>
          <ul className="stats-assumptions">
            {r.assumptions.map((a, i) => {
              const line = assumptionLine(a);
              return (
                <li key={i} className={`stats-status-${line.status}`} data-status={line.status}>
                  <span className="stats-badge">{line.badge}</span> {line.text}
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}
      {code ? (
        <details className="stats-script" data-testid="stats-script">
          <summary>{t('The R code for this page')}</summary>
          <p className="muted">{t('The same analysis in R, which prints the numbers above. It reads the variables from a table called data and uses the helper functions of the whole script. Export the notebook as an R script to get them, with the data.')}</p>
          <pre className="stats-code" tabIndex={0} role="region" aria-label={t('The R code for page {n}', { n: num(number) })}>
            <code>{code}</code>
          </pre>
          <Button
            aria-label={t('Copy the R code for page {n}', { n: num(number) })}
            data-testid="stats-copy-code"
            onPress={async () => {
              try {
                await navigator.clipboard.writeText(code);
                announce(t('Copied the R code for page {n}.', { n: num(number) }));
              } catch {
                announce(t('The code could not be copied. Select it and copy it yourself.'));
              }
            }}
          >
            {t('Copy the code')}
          </Button>
        </details>
      ) : null}
      {r.groups.length > 0 ? (
        <details className="stats-cases">
          <summary>{t('The cases behind each group')}</summary>
          <ul>
            {r.groups.map((g) => (
              <li key={g.value}>
                <strong>{g.label}</strong>: {g.case_ids.map((id) => names.get(id) ?? String(id)).join(', ')}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
      <PageTools entry={entry} number={number} />
      <div className="actions">
        <Button onPress={() => void removeFromNotebook(entry.id)} aria-label={t('Remove page {n} from the notebook', { n: num(number) })}>{t('Remove')}</Button>
      </div>
    </li>
  );
}
