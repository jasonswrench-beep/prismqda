import { useEffect, useState } from 'react';
import { Button, Input, Label, Radio, RadioGroup, Tab, TabList, TabPanel, Tabs, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { COEFFICIENTS, canMeasure, codeMeasure, coderLabel, disagreementShare, fixed, headline, intervalNote, intervalText, termLabel } from '../lib/agreement';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type { AgreementReport, AgreementRunRow } from '../types';
import { ClassTab } from './team/Class';
import { DisagreementsTab } from './team/Disagreements';
import { MergeTab } from './team/Merge';
import { PacketsTab } from './team/Packets';
import { Notice } from './common';
import { Tick } from './Tick';
import { dateTime, num, t } from '../i18n';

type Mode = 'single_label' | 'per_code';
type Term = '' | 'intercoder_reliability' | 'concordance';

/**
 * Team: how well the coders agree. The researcher chooses two or more coders and how to compare them, and the engine measures
 * agreement on the units the coders both coded, with intervals, a table of where they differ, and a written report that says
 * what was done in plain words. Each run is kept in the project; a run that is not archived keeps the documents it used from
 * being edited, so the figures always describe the text they were worked out on.
 */
export function TeamView() {
  const coders = useApp((s) => s.coders);
  const version = useApp((s) => s.dataVersion);
  const [tab, setTab] = useState<'agreement' | 'disagreements' | 'merge' | 'packets' | 'class'>('agreement');
  const [chosen, setChosen] = useState<number[] | null>(null);
  const [mode, setMode] = useState<Mode>('single_label');
  const [units, setUnits] = useState<'all' | 'sample'>('all');
  const [size, setSize] = useState('30');
  const [seed, setSeed] = useState('1');
  const [term, setTerm] = useState<Term>('');
  const [areas, setAreas] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<AgreementReport | null>(null);
  const [runs, setRuns] = useState<AgreementRunRow[] | null>(null);
  const [archiving, setArchiving] = useState<number | null>(null);

  // Until a choice is made, every person who codes is chosen; a model is chosen only on purpose.
  const picked = chosen ?? coders.filter((c) => c.role === 'human').map((c) => c.id);
  const toggle = (id: number, on: boolean) => setChosen(on ? [...picked, id] : picked.filter((x) => x !== id));

  useEffect(() => {
    let live = true;
    call('reliability.list', {})
      .then((r) => live && setRuns(r))
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version]);

  const measure = async () => {
    setError(null);
    if (!canMeasure(picked)) return setError(t('Choose at least two coders to compare.'));
    setBusy(true);
    try {
      let unitIds: number[] | undefined;
      if (units === 'sample') {
        const wanted = Math.floor(Number(size));
        if (!Number.isFinite(wanted) || wanted < 1) return setError(t('Say how many units to draw: a whole number, one or more.'));
        const drawn = await call('sample.draw', { size: wanted, seed: Number.isFinite(Number(seed)) ? Math.floor(Number(seed)) : 1 });
        unitIds = drawn.unit_ids;
      }
      const r = await call('reliability.run', {
        coder_ids: picked,
        mode,
        ...(unitIds ? { unit_ids: unitIds } : {}),
        ...(term ? { reporting_term: term } : {}),
        areas,
      });
      setReport(r);
      announce(headline(r));
      setRuns(await call('reliability.list', {}));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const open = async (id: number) => {
    setError(null);
    try {
      const rec = await call('reliability.get', { id });
      setReport({ ...rec.report, run_id: id });
      announce(t('Opened run {id}.', { id: num(id) }));
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  const archive = async (id: number) => {
    setArchiving(null);
    try {
      await call('reliability.archive', { id });
      setRuns(await call('reliability.list', {}));
      announce(t('Archived run {id}. The documents it used can be edited again.', { id: num(id) }));
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  const copy = async () => {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(report.report_text);
      announce(t('Copied the report.'));
    } catch {
      announce(t('The report could not be copied. Select it and copy it yourself.'));
    }
  };

  return (
    <main id="main-content" tabIndex={-1} className="view-page team-page" aria-labelledby="h-team">
      <h2 id="h-team">{t('Team')}</h2>
      <p className="help">{t('Agreement between coders: how often the coders you choose gave the same code to the same unit, worked out with coefficients that allow for chance. Agreement with an AI model is called concordance, because a model is not an independent coder.')}</p>
      {error ? <Notice>{error}</Notice> : null}

      <section className="ac-card" aria-labelledby="h-team-coders">
        <h3 id="h-team-coders" className="bk-h3">{t('Coders')}</h3>
        <fieldset className="field team-coders">
          <legend>{t('Coders to compare')}</legend>
          {coders.length === 0 ? <p className="muted">{t('There are no coders yet.')}</p> : null}
          {coders.map((c) => (
            <Tick key={c.id} isSelected={picked.includes(c.id)} onChange={(on) => toggle(c.id, on)} testId={`team-coder-${c.id}`}>{coderLabel(c)}</Tick>
          ))}
        </fieldset>
      </section>

      <Tabs selectedKey={tab} onSelectionChange={(k) => setTab(k as 'agreement' | 'disagreements' | 'merge' | 'packets' | 'class')} className="bk-editor-tabs">
        <TabList aria-label={t('Team')} className="bk-editor-tablist">
          <Tab id="agreement" className="bk-tab" data-testid="team-tab-agreement">{t('Agreement')}</Tab>
          <Tab id="disagreements" className="bk-tab" data-testid="team-tab-disagreements">{t('Disagreements')}</Tab>
          <Tab id="merge" className="bk-tab" data-testid="team-tab-merge">{t('Merge')}</Tab>
          <Tab id="packets" className="bk-tab" data-testid="team-tab-packets">{t('Packets')}</Tab>
          <Tab id="class" className="bk-tab" data-testid="team-tab-class">{t('Class')}</Tab>
        </TabList>
        <TabPanel id="agreement" className="bk-editor-panel">
      <section className="ac-card" aria-labelledby="h-team-who">
        <h3 id="h-team-who" className="bk-h3">{t('Measure agreement')}</h3>

        <RadioGroup value={mode} onChange={(v) => setMode(v as Mode)} className="field ac-radios" aria-label={t('How to compare')}>
          <Radio value="single_label" className="react-aria-Radio ac-radio">
            <span className="ac-radio-label">{t('One code for each unit')}</span>
            <span className="ac-radio-help">{t('Each coder gave each unit one code, as in content analysis. The coefficients are worked out across all the codes.')}</span>
          </Radio>
          <Radio value="per_code" className="react-aria-Radio ac-radio">
            <span className="ac-radio-label">{t('Each code on its own')}</span>
            <span className="ac-radio-help">{t('For every code: did the coder apply it to the unit? A unit can have several codes. The coefficients are worked out for each code.')}</span>
          </Radio>
        </RadioGroup>

        <RadioGroup value={units} onChange={(v) => setUnits(v as 'all' | 'sample')} className="field ac-radios" aria-label={t('Which units')}>
          <Radio value="all" className="react-aria-Radio ac-radio">
            <span className="ac-radio-label">{t('Every unit that at least two of these coders coded')}</span>
          </Radio>
          <Radio value="sample" className="react-aria-Radio ac-radio">
            <span className="ac-radio-label">{t('A random sample of units')}</span>
            <span className="ac-radio-help">{t('The same number and seed draw the same sample again, so the sample can be described in a report.')}</span>
          </Radio>
        </RadioGroup>
        {units === 'sample' ? (
          <div className="team-sample">
            <TextField value={size} onChange={setSize} className="field" type="number">
              <Label>{t('How many units')}</Label>
              <Input min={1} />
            </TextField>
            <TextField value={seed} onChange={setSeed} className="field" type="number">
              <Label>{t('Seed')}</Label>
              <Input />
            </TextField>
          </div>
        ) : null}

        <div className="field">
          <label htmlFor="team-term">{t('Call it')}</label>
          <select id="team-term" value={term} onChange={(e) => setTerm(e.target.value as Term)}>
            <option value="">{t('The usual word for these coders')}</option>
            <option value="intercoder_reliability">{t('Intercoder reliability')}</option>
            <option value="concordance">{t('Concordance')}</option>
          </select>
        </div>
        <Tick isSelected={areas} onChange={setAreas} testId="team-areas">{t('Also compare the areas coders drew on pictures')}</Tick>

        <div className="actions">
          <Button className="primary" onPress={() => void measure()} isDisabled={busy} data-testid="team-measure">{busy ? t('Measuring…') : t('Measure agreement')}</Button>
        </div>
      </section>

      {report ? (
        <section className="ac-card" aria-labelledby="h-team-result" data-testid="agreement-result">
          <h3 id="h-team-result" className="bk-h3">{report.run_id ? t('Result of run {id}', { id: num(report.run_id) }) : t('Result')}</h3>
          <p role="status" className="team-headline" data-testid="agreement-headline">{headline(report)}</p>
          <p className="help">{t('Called {term}. Coders: {coders}.', { term: termLabel(report.reporting_term), coders: report.coders.map(coderLabel).join(', ') })}</p>

          {report.overall ? (
            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="agreement-overall">
                <caption>{t('Agreement across all codes')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Coefficient')}</th>
                    <th scope="col">{t('Value')}</th>
                    <th scope="col">{t('Interval')}</th>
                  </tr>
                </thead>
                <tbody>
                  {COEFFICIENTS.filter((c) => c.key === 'krippendorff_alpha' || report.overall![c.key] !== null).map((c) => (
                    <tr key={c.key}>
                      <th scope="row">{c.label}</th>
                      <td>{fixed(report.overall![c.key])}</td>
                      <td>{intervalText(report.intervals[c.key])}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="help">{intervalNote(report)}</p>
            </div>
          ) : null}

          {report.by_code.length > 0 ? (
            <div className="table-wrap" tabIndex={-1}>
              <table className="data" data-testid="agreement-by-code">
                <caption>{t('Agreement for each code, the least agreed first')}</caption>
                <thead>
                  <tr>
                    <th scope="col">{t('Code')}</th>
                    <th scope="col">{t("Krippendorff's alpha")}</th>
                    <th scope="col">{t('Percent agreement')}</th>
                    <th scope="col">{t('Prevalence index')}</th>
                    <th scope="col">{t('Bias index')}</th>
                    <th scope="col">{t('Against the target')}</th>
                  </tr>
                </thead>
                <tbody>
                  {report.by_code.map((c) => (
                    <tr key={c.code_id}>
                      <th scope="row">{c.name}</th>
                      <td>{fixed(codeMeasure(c))}</td>
                      <td>{fixed(c.detail.agreement.percent_agreement)}</td>
                      <td>{fixed(c.detail.prevalence_index)}</td>
                      <td>{fixed(c.detail.bias_index)}</td>
                      <td>{c.below_target ? t('Below the target') : t('Meets the target')}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}

          {report.confusion && report.coders.length === 2 ? (
            <div className="table-wrap" tabIndex={-1}>
              <table className="data team-confusion" data-testid="agreement-confusion">
                <caption>{t('How the codes of {first} (down) and {second} (across) line up', { first: report.coders[0].name, second: report.coders[1].name })}</caption>
                <thead>
                  <tr>
                    <td>
                      <span className="visually-hidden">{t('Code of {first}', { first: report.coders[0].name })}</span>
                    </td>
                    {report.confusion_codes.map((c) => (
                      <th key={c} scope="col">{c}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {report.confusion.map((row, i) => (
                    <tr key={report.confusion_codes[i]}>
                      <th scope="row">{report.confusion_codes[i]}</th>
                      {row.map((n, j) => (
                        <td key={report.confusion_codes[j]} className={i === j ? 'team-agree' : undefined}>{num(n)}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="help" data-testid="agreement-disagreements">{t('The coders differed on {share}', { share: disagreementShare(report) })}</p>
            </div>
          ) : null}

          {report.notes.length > 0 ? (
            <div data-testid="agreement-notes">
              <h4 className="bk-h3">{t('Notes')}</h4>
              <ul>
                {report.notes.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            </div>
          ) : null}

          <div className="field">
            <label htmlFor="team-report-text">{t('The written report')}</label>
            <textarea id="team-report-text" readOnly rows={14} value={report.report_text} className="mono team-report-text" data-testid="agreement-report-text" />
          </div>
          <div className="actions">
            <Button onPress={() => void copy()} data-testid="agreement-copy">{t('Copy the report')}</Button>
          </div>
        </section>
      ) : null}

      <section className="ac-card" aria-labelledby="h-team-runs">
        <h3 id="h-team-runs" className="bk-h3">{runs && runs.length > 0 ? t('Runs kept in this project ({count})', { count: num(runs.length) }) : t('Runs kept in this project')}</h3>
        {runs === null ? <p className="muted">{t('Looking for runs…')}</p> : runs.length === 0 ? <p className="muted" data-testid="agreement-no-runs">{t('No run has been kept yet.')}</p> : (
          <div className="table-wrap" tabIndex={-1}>
            <table className="data" data-testid="agreement-runs">
              <caption>{t('Runs kept in this project')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Run')}</th>
                  <th scope="col">{t('Made')}</th>
                  <th scope="col">{t('Called')}</th>
                  <th scope="col">{t('Units')}</th>
                  <th scope="col">{t('State')}</th>
                  <th scope="col">{t('Actions')}</th>
                </tr>
              </thead>
              <tbody>
                {runs.map((r) => (
                  <tr key={r.id}>
                    <th scope="row">{num(r.id)}</th>
                    <td>{dateTime(new Date(r.created_at), { dateStyle: 'medium', timeStyle: 'short' })}</td>
                    <td>{termLabel(r.reporting_term)}</td>
                    <td>{num(r.units)}</td>
                    <td>{r.archived ? t('Archived') : t('Keeps its documents from being edited')}</td>
                    <td>
                      <Button onPress={() => void open(r.id)} aria-label={t('Open run {id}', { id: num(r.id) })}>{t('Open')}</Button>{' '}
                      {r.archived ? null : archiving === r.id ? (
                        <>
                          <Button onPress={() => void archive(r.id)} aria-label={t('Archive run {id} now', { id: num(r.id) })}>{t('Archive it')}</Button>{' '}
                          <Button onPress={() => setArchiving(null)}>{t('Keep it')}</Button>
                        </>
                      ) : (
                        <Button onPress={() => setArchiving(r.id)} aria-label={t('Archive run {id}', { id: num(r.id) })}>{t('Archive')}</Button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="help">{t('A run that is not archived keeps the documents it used from being edited, so the figures always describe the text they were worked out on. Archiving it lets those documents change again; the run stays in the project as a record.')}</p>
          </div>
        )}
      </section>
        </TabPanel>
        <TabPanel id="disagreements" className="bk-editor-panel">
          <DisagreementsTab picked={picked} />
        </TabPanel>
        <TabPanel id="merge" className="bk-editor-panel">
          <MergeTab />
        </TabPanel>
        <TabPanel id="packets" className="bk-editor-panel">
          <PacketsTab />
        </TabPanel>
        <TabPanel id="class" className="bk-editor-panel">
          <ClassTab />
        </TabPanel>
      </Tabs>
    </main>
  );
}
