import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { Button, Dialog, Heading, Modal, ModalOverlay } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { LEVEL_LABELS, LEVELS } from '../lib/importMapping';
import { shownCell } from '../lib/tableCells';
import { groupFunctions } from '../lib/variables';
import { browseManyFiles, browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { bumpData } from '../state/actions';
import { announce } from '../state/announce';
import type { CaseData, DataTableResult, DataTableSpec, FileColumn, GroupFunctionId, InspectedFile, Level, MergeResult, SortKeySpec } from '../types';
import { Notice } from './common';
import { Tick } from './Tick';
import { num, t, tn } from '../i18n';

const CASE_COLUMN = 'Case';

// ---------------------------------------------------------------- putting the cases in order

/** Chooses the order the cases are shown in: up to three variables, each going up or down. Nothing in the project is changed. */
export function SortDialog(props: { data: CaseData; keys: SortKeySpec[]; onApply: (keys: SortKeySpec[]) => void; onClose: () => void }) {
  const [rows, setRows] = useState<{ variable: string; descending: boolean }[]>(
    props.keys.length > 0 ? props.keys.map((k) => ({ variable: k.variable, descending: !!k.descending })) : [{ variable: '', descending: false }],
  );
  const names = [CASE_COLUMN, ...props.data.variables.map((v) => v.name)];
  const label = (name: string) => (name === CASE_COLUMN ? t('The name of the case') : name);
  const set = (i: number, patch: Partial<{ variable: string; descending: boolean }>) => setRows(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const chosen = rows.filter((r) => r.variable !== '');

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && props.onClose()} className="overlay">
      <Modal className="modal">
        <Dialog className="dialog" aria-labelledby="sort-title">
          <Heading slot="title" id="sort-title" className="dialog-title">{t('Sort the cases')}</Heading>
          <p className="help">{t('This only changes the order the data view shows the cases in. Nothing in the project is moved, and a case with no value in the variable comes last.')}</p>
          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              props.onApply(chosen.map((r) => ({ variable: r.variable, descending: r.descending })));
              props.onClose();
            }}
            noValidate
          >
            {rows.map((r, i) => (
              <div className="ve-rule" key={i} data-testid="sort-key">
                <div className="field">
                  <label htmlFor={`sort-var-${i}`}>{i === 0 ? t('Sort by') : t('Then by')}</label>
                  <select id={`sort-var-${i}`} value={r.variable} onChange={(e) => set(i, { variable: e.target.value })} data-testid={`sort-var-${i}`}>
                    <option value="">{t('Choose a variable')}</option>
                    {names.map((n) => (
                      <option key={n} value={n}>
                        {label(n)}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label htmlFor={`sort-dir-${i}`}>{t('Order {n}', { n: i + 1 })}</label>
                  <select id={`sort-dir-${i}`} value={r.descending ? 'down' : 'up'} onChange={(e) => set(i, { descending: e.target.value === 'down' })} data-testid={`sort-dir-${i}`}>
                    <option value="up">{t('Low to high, A to Z, earliest first')}</option>
                    <option value="down">{t('High to low, Z to A, latest first')}</option>
                  </select>
                </div>
              </div>
            ))}
            {rows.length < 3 ? (
              <div className="actions">
                <Button onPress={() => setRows([...rows, { variable: '', descending: false }])} data-testid="sort-add">{t('Add another variable to break ties')}</Button>
              </div>
            ) : null}
            <div className="actions">
              <Button type="submit" className="primary" isDisabled={chosen.length === 0} data-testid="sort-apply">{t('Sort')}</Button>
              <Button
                onPress={() => {
                  props.onApply([]);
                  props.onClose();
                }}
                isDisabled={props.keys.length === 0}
                data-testid="sort-clear"
              >
                {t('Show the cases in their own order')}
              </Button>
              <Button type="button" onPress={props.onClose}>{t('Cancel')}</Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

// ---------------------------------------------------------------- a file's columns

/** A path to a file with a Browse button in the window, and a button that reads it. */
function FileChooser(props: { id: string; label: string; path: string; setPath: (v: string) => void; onRead: () => void; busy: boolean; read: InspectedFile | null }) {
  const choose = async () => {
    const files = await browseManyFiles('documents');
    if (files[0]) props.setPath(files[0]);
  };
  return (
    <div className="field">
      <label htmlFor={props.id}>{props.label}</label>
      <input id={props.id} type="text" value={props.path} onChange={(e) => props.setPath(e.target.value)} autoComplete="off" spellCheck={false} placeholder={t('C:\\Data\\wave2.xlsx')} data-testid={props.id} />
      <div className="actions">
        <Button onPress={props.onRead} isDisabled={props.busy || cleanTypedPath(props.path) === ''} data-testid={`${props.id}-read`}>{t('Read the file')}</Button>
        {nativeDialogsAvailable ? <Button onPress={() => void choose()} isDisabled={props.busy}>{t('Choose a file…')}</Button> : null}
      </div>
      {props.read ? (
        <p className="help" role="status" data-testid={`${props.id}-read-summary`}>
          {t('{name}: {columns} and {rows}.', {
            name: props.read.file_name,
            columns: tn(props.read.columns.length, { one: '{count} column', other: '{count} columns' }),
            rows: tn(props.read.rows, { one: '{count} row', other: '{count} rows' }),
          })}
        </p>
      ) : null}
    </div>
  );
}

function useInspect() {
  const [path, setPath] = useState('');
  const [read, setRead] = useState<InspectedFile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inspect = async () => {
    setBusy(true);
    setError(null);
    try {
      setRead(await call('data.inspect_file', { path: cleanTypedPath(path) }));
    } catch (e) {
      setRead(null);
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  return { path, setPath, read, error, busy, inspect, reset: () => setRead(null) };
}

// ---------------------------------------------------------------- tables: by group, stood on end, laid across

type TableOp = DataTableSpec['op'];

interface SummaryRow {
  source: string;
  fn: GroupFunctionId;
}

function CheckList(props: { legend: string; names: string[]; chosen: string[]; setChosen: (v: string[]) => void; testId: string; help?: string }) {
  return (
    <fieldset className="stats-fieldset" data-testid={props.testId}>
      <legend>{props.legend}</legend>
      {props.names.map((n) => (
        <Tick key={n} isSelected={props.chosen.includes(n)} onChange={(on) => props.setChosen(on ? [...props.chosen, n] : props.chosen.filter((x) => x !== n))}>
          {n}
        </Tick>
      ))}
      {props.help ? <p className="help">{props.help}</p> : null}
    </fieldset>
  );
}

/**
 * Tables that leave the project as a file: a row for each group of cases with summaries, the cases (or a file's rows) stood on end so each
 * measure of each row is a row, and observations laid across so each who is a row. Rows of Statistics are cases, so these are not kept in
 * the project; they are shown, and saved as CSV or Excel for another program.
 */
export function TablesDialog(props: { data: CaseData; onClose: () => void }) {
  const { data } = props;
  const [op, setOp] = useState<TableOp>('groups');
  const [fromFile, setFromFile] = useState(false);
  const file = useInspect();
  const [by, setBy] = useState<string[]>([]);
  const [summaries, setSummaries] = useState<SummaryRow[]>([{ source: '', fn: 'mean' }]);
  const [keep, setKeep] = useState<string[]>([]);
  const [measures, setMeasures] = useState<string[]>([]);
  const [indexName, setIndexName] = useState('Measure');
  const [valueName, setValueName] = useState('Value');
  const [dropMissing, setDropMissing] = useState(false);
  const [idCols, setIdCols] = useState<string[]>([]);
  const [indexCol, setIndexCol] = useState('');
  const [valueCols, setValueCols] = useState<string[]>([]);
  const [table, setTable] = useState<DataTableResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [savePath, setSavePath] = useState('');
  const [saveFormat, setSaveFormat] = useState<'csv' | 'xlsx'>('csv');
  const [saved, setSaved] = useState<string | null>(null);

  const restructure = op !== 'groups';
  const columns = useMemo(() => (fromFile ? (file.read?.columns.map((c: FileColumn) => c.name) ?? []) : [CASE_COLUMN, ...data.variables.map((v) => v.name)]), [fromFile, file.read, data]);
  const variableNames = data.variables.filter((v) => v.level !== 'text').map((v) => v.name);

  // Anything chosen is forgotten when the choices change, so a table is never shown beside choices that did not make it.
  useEffect(() => {
    setTable(null);
    setSaved(null);
  }, [op, fromFile, by, summaries, keep, measures, indexName, valueName, dropMissing, idCols, indexCol, valueCols, file.read]);

  const from = fromFile ? ({ kind: 'file', path: cleanTypedPath(file.path) } as const) : ({ kind: 'cases' } as const);
  const spec = useMemo<{ spec: DataTableSpec | null; problem: string | null }>(() => {
    if (op === 'groups') {
      if (by.length === 0) return { spec: null, problem: t('Choose at least one variable that makes the groups.') };
      const chosen = summaries.filter((s) => s.source !== '');
      return { spec: { op, by, summaries: chosen.map((s) => ({ source: s.source, function: s.fn })) }, problem: null };
    }
    if (fromFile && !file.read) return { spec: null, problem: t('Choose the file and read it first.') };
    if (op === 'stand_on_end') {
      if (measures.length < 2) return { spec: null, problem: t('Choose at least two columns to stand on end.') };
      return { spec: { op, from, keep, measures, index_name: indexName.trim(), value_name: valueName.trim(), drop_missing: dropMissing }, problem: null };
    }
    if (idCols.length === 0 || indexCol === '' || valueCols.length === 0) return { spec: null, problem: t('Choose who each row is about, which observation it is, and the values to lay across.') };
    return { spec: { op, from, id: idCols, index: indexCol, values: valueCols }, problem: null };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [op, by, summaries, fromFile, file.read, file.path, measures, keep, indexName, valueName, dropMissing, idCols, indexCol, valueCols]);

  const show = async () => {
    if (!spec.spec) return;
    setBusy(true);
    setError(null);
    try {
      const made = await call('data.table', { spec: spec.spec });
      setTable(made);
      announce(tn(made.total_rows, { one: 'The table has {count} row.', other: 'The table has {count} rows.' }));
    } catch (e) {
      setTable(null);
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    if (!spec.spec) return;
    const typed = cleanTypedPath(savePath);
    if (typed === '') {
      setError(t('Type the full path of the file to write, for example C:\\Data\\groups.csv'));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const done = await call('data.table_save', { spec: spec.spec, path: typed, format: saveFormat });
      setSaved(done.path);
      announce(t('Saved the table to {path}.', { path: done.path }));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const browseSave = async () => {
    const chosen = await browsePath('export-file', `table.${saveFormat}`, saveFormat);
    if (chosen) setSavePath(chosen);
  };

  const setSummary = (i: number, patch: Partial<SummaryRow>) => setSummaries(summaries.map((s, j) => (j === i ? { ...s, ...patch } : s)));
  const fns = groupFunctions();
  // The summaries of a group table come after the variables that make the groups and the number of cases.
  const roundedFrom = op === 'groups' ? by.length + 1 : null;

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && props.onClose()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog ve-transform" aria-labelledby="tables-title">
          <Heading slot="title" id="tables-title" className="dialog-title">{t('Tables by group, and restructuring')}</Heading>
          <p className="help">{t('A row of Statistics is always a case, so these tables are not kept in the project. They are shown here and saved as a file for another program.')}</p>
          {error ? <Notice>{error}</Notice> : null}
          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              void show();
            }}
            noValidate
          >
            <div className="field">
              <label htmlFor="tables-op">{t('Make')}</label>
              <select id="tables-op" value={op} onChange={(e) => setOp(e.target.value as TableOp)} data-testid="tables-op">
                <option value="groups">{t('A table with a row for each group of cases')}</option>
                <option value="stand_on_end">{t('Wide to long: stand the columns on end, so each measure is a row')}</option>
                <option value="lay_across">{t('Long to wide: lay the observations across, so each who is a row')}</option>
              </select>
            </div>

            {restructure ? (
              <>
                <Tick isSelected={fromFile} onChange={setFromFile} testId="tables-from-file">
                  {t('Use the rows of a file instead of the cases')}
                </Tick>
                {fromFile ? <FileChooser id="tables-file" label={t('Full path of the file')} path={file.path} setPath={file.setPath} onRead={() => void file.inspect()} busy={file.busy} read={file.read} /> : null}
                {file.error ? <Notice>{file.error}</Notice> : null}
              </>
            ) : null}

            {op === 'groups' ? (
              <>
                <CheckList legend={t('The groups are the cases that share a value of')} names={variableNames} chosen={by} setChosen={setBy} testId="tables-by" />
                <fieldset className="stats-fieldset" data-testid="tables-summaries">
                  <legend>{t('What to work out for each group')}</legend>
                  {summaries.map((s, i) => (
                    <div className="ve-rule" key={i}>
                      <div className="field">
                        <label htmlFor={`tables-sum-fn-${i}`}>{t('Summary {n}', { n: i + 1 })}</label>
                        <select id={`tables-sum-fn-${i}`} value={s.fn} onChange={(e) => setSummary(i, { fn: e.target.value as GroupFunctionId })} data-testid={`tables-sum-fn-${i}`}>
                          {fns.map((f) => (
                            <option key={f.id} value={f.id}>
                              {f.label}
                            </option>
                          ))}
                        </select>
                      </div>
                      <div className="field">
                        <label htmlFor={`tables-sum-var-${i}`}>{t('of the variable {n}', { n: i + 1 })}</label>
                        <select id={`tables-sum-var-${i}`} value={s.source} onChange={(e) => setSummary(i, { source: e.target.value })} data-testid={`tables-sum-var-${i}`}>
                          <option value="">{t('Choose a variable')}</option>
                          {data.variables.map((v) => (
                            <option key={v.id} value={v.name}>
                              {v.name}
                            </option>
                          ))}
                        </select>
                      </div>
                      {summaries.length > 1 ? <Button onPress={() => setSummaries(summaries.filter((_, j) => j !== i))} aria-label={t('Remove summary {n}', { n: i + 1 })}>{t('Remove')}</Button> : null}
                    </div>
                  ))}
                  <div className="actions">
                    <Button onPress={() => setSummaries([...summaries, { source: '', fn: 'mean' }])} data-testid="tables-add-summary">{t('Add a summary')}</Button>
                  </div>
                  <p className="help">{t('Each table also says how many cases are in each group. A case with no value in a variable that makes the groups is in no group, and the table says how many.')}</p>
                </fieldset>
              </>
            ) : null}

            {op === 'stand_on_end' ? (
              <>
                <CheckList legend={t('Columns kept on every row (who the row is about)')} names={columns} chosen={keep} setChosen={setKeep} testId="tables-keep" />
                <CheckList legend={t('Columns to stand on end (the same thing measured more than once)')} names={columns.filter((c) => !keep.includes(c))} chosen={measures} setChosen={setMeasures} testId="tables-measures" />
                <div className="field">
                  <label htmlFor="tables-index-name">{t('Name of the new column that says which measure a row is')}</label>
                  <input id="tables-index-name" type="text" value={indexName} onChange={(e) => setIndexName(e.target.value)} autoComplete="off" data-testid="tables-index-name" />
                </div>
                <div className="field">
                  <label htmlFor="tables-value-name">{t('Name of the new column of values')}</label>
                  <input id="tables-value-name" type="text" value={valueName} onChange={(e) => setValueName(e.target.value)} autoComplete="off" data-testid="tables-value-name" />
                </div>
                <Tick isSelected={dropMissing} onChange={setDropMissing} testId="tables-drop-missing">
                  {t('Leave out a row that has no value')}
                </Tick>
              </>
            ) : null}

            {op === 'lay_across' ? (
              <>
                <CheckList legend={t('Columns that say who a row is about (rows that share them become one row)')} names={columns.filter((c) => c !== indexCol && !valueCols.includes(c))} chosen={idCols} setChosen={setIdCols} testId="tables-id" />
                <div className="field">
                  <label htmlFor="tables-index">{t('Column that says which observation a row is (a time, a wave, a condition)')}</label>
                  <select id="tables-index" value={indexCol} onChange={(e) => setIndexCol(e.target.value)} data-testid="tables-index">
                    <option value="">{t('Choose a column')}</option>
                    {columns.filter((c) => !idCols.includes(c) && !valueCols.includes(c)).map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                </div>
                <CheckList legend={t('Columns of values to lay across')} names={columns.filter((c) => !idCols.includes(c) && c !== indexCol)} chosen={valueCols} setChosen={setValueCols} testId="tables-values" help={t('Two rows for the same who and the same observation cannot be laid across; the table names the first such pair so you can correct it.')} />
              </>
            ) : null}

            <div className="actions">
              <Button type="submit" className="primary" isDisabled={!spec.spec || busy} data-testid="tables-show">{t('Show the table')}</Button>
              <Button type="button" onPress={props.onClose} data-testid="tables-close">{t('Close')}</Button>
            </div>
            {!spec.spec && spec.problem ? <p className="muted" data-testid="tables-why">{spec.problem}</p> : null}
          </form>

          {table ? (
            <section aria-labelledby="tables-result-h" data-testid="tables-result" className="ve-preview">
              <h3 id="tables-result-h" className="stats-h4">{t('The table')}</h3>
              <p role="status" data-testid="tables-summary">
                {table.total_rows > table.shown_rows
                  ? tn(table.total_rows, { one: '{count} row (the first {shown} are shown here; the file has them all).', other: '{count} rows (the first {shown} are shown here; the file has them all).' }, { shown: num(table.shown_rows) })
                  : tn(table.total_rows, { one: '{count} row.', other: '{count} rows.' })}
                {table.left_out ? ` ${tn(table.left_out, { one: '{count} case is in no group because it has no value in a variable that makes the groups.', other: '{count} cases are in no group because they have no value in a variable that makes the groups.' })}` : ''}
                {roundedFrom !== null && table.rows.some((r) => r.some((cell, j) => j >= roundedFrom && shownCell(cell) !== cell)) ? ` ${t('The summaries are rounded to two decimals here; the file has them in full.')}` : ''}
              </p>
              <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The table, as far as it is shown')}>
                <table className="data" data-testid="tables-table">
                  <caption className="visually-hidden">{t('The table made from your choices')}</caption>
                  <thead>
                    <tr>
                      {table.columns.map((c, i) => (
                        <th key={i} scope="col">{c}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {table.rows.map((r, i) => (
                      <tr key={i}>
                        {r.map((cell, j) => (
                          <td key={j}>{(roundedFrom !== null && j >= roundedFrom ? shownCell(cell) : cell) ?? ''}</td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div className="field">
                <label htmlFor="tables-save-path">{t('Full path of the file to save')}</label>
                <input id="tables-save-path" type="text" value={savePath} onChange={(e) => setSavePath(e.target.value)} autoComplete="off" spellCheck={false} placeholder={t('C:\\Data\\groups.csv')} data-testid="tables-save-path" />
                <div className="actions">
                  <select aria-label={t('Kind of file')} value={saveFormat} onChange={(e) => setSaveFormat(e.target.value as 'csv' | 'xlsx')} data-testid="tables-save-format">
                    <option value="csv">{t('CSV (a text file every program reads)')}</option>
                    <option value="xlsx">{t('Excel workbook')}</option>
                  </select>
                  <Button onPress={() => void save()} isDisabled={busy} data-testid="tables-save">{t('Save the table')}</Button>
                  {nativeDialogsAvailable ? <Button onPress={() => void browseSave()} isDisabled={busy}>{t('Choose where…')}</Button> : null}
                </div>
                {saved ? <p role="status" data-testid="tables-saved">{t('Saved to {path}.', { path: saved })}</p> : null}
              </div>
            </section>
          ) : null}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

// ---------------------------------------------------------------- merging variables from a file

interface MergeRow {
  include: boolean;
  name: string;
  level: Level;
}

/** Brings columns of a file in as variables of the cases, matched to the cases by a key. */
export function MergeDialog(props: { data: CaseData; onClose: () => void }) {
  const { data } = props;
  const file = useInspect();
  const [key, setKey] = useState('');
  const [matchOn, setMatchOn] = useState(CASE_COLUMN);
  const [rows, setRows] = useState<Record<string, MergeRow>>({});
  const [overwrite, setOverwrite] = useState(false);
  const [result, setResult] = useState<MergeResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Reading a file sets each column up as a variable of the level its values suggest.
  useEffect(() => {
    if (!file.read) return;
    const next: Record<string, MergeRow> = {};
    for (const c of file.read.columns) next[c.name] = { include: false, name: c.name, level: c.level };
    setRows(next);
    setKey(file.read.columns[0]?.name ?? '');
    setResult(null);
  }, [file.read]);

  const columns = file.read?.columns ?? [];
  const chosen = columns.filter((c) => c.name !== key && rows[c.name]?.include);
  const existing = new Set(data.variables.map((v) => v.name.toLowerCase()));
  const clash = chosen.filter((c) => existing.has((rows[c.name]?.name ?? c.name).trim().toLowerCase()));
  const canMerge = file.read !== null && key !== '' && chosen.length > 0 && (clash.length === 0 || overwrite) && !busy && result === null;
  const setRow = (name: string, patch: Partial<MergeRow>) => setRows({ ...rows, [name]: { ...rows[name], ...patch } });

  const merge = async () => {
    setBusy(true);
    setError(null);
    try {
      const done = await call('data.merge', {
        path: cleanTypedPath(file.path),
        key_column: key,
        match_on: matchOn === CASE_COLUMN ? null : matchOn,
        columns: chosen.map((c) => ({ name: c.name, as: (rows[c.name].name ?? c.name).trim(), level: rows[c.name].level })),
        overwrite,
      });
      setResult(done);
      bumpData();
      announce(t('Merged {count} variables from the file into {cases} cases.', { count: done.variables_made.length + done.variables_filled.length, cases: done.cases_matched }));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && props.onClose()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog ve-transform" aria-labelledby="merge-title">
          <Heading slot="title" id="merge-title" className="dialog-title">{t('Merge variables from a file')}</Heading>
          <p className="help">{t('Brings columns of a spreadsheet, a text file of values, or an SPSS file in as variables of your cases, matching each row of the file to a case by a key, such as the name of the case or an identifier. A case the file has no row for gets no value. A row that matches no case brings nothing in, and the result says so.')}</p>
          {error ? <Notice>{error}</Notice> : null}
          {file.error ? <Notice>{file.error}</Notice> : null}
          <FileChooser id="merge-file" label={t('Full path of the file')} path={file.path} setPath={file.setPath} onRead={() => void file.inspect()} busy={file.busy} read={file.read} />

          {file.read ? (
            <form
              onSubmit={(e: FormEvent) => {
                e.preventDefault();
                if (canMerge) void merge();
              }}
              noValidate
            >
              <div className="field">
                <label htmlFor="merge-key">{t('The column of the file that holds the key')}</label>
                <select id="merge-key" value={key} onChange={(e) => setKey(e.target.value)} data-testid="merge-key">
                  {columns.map((c) => (
                    <option key={c.name} value={c.name}>
                      {c.name}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label htmlFor="merge-match">{t('The key is matched with')}</label>
                <select id="merge-match" value={matchOn} onChange={(e) => setMatchOn(e.target.value)} data-testid="merge-match">
                  <option value={CASE_COLUMN}>{t('The name of the case')}</option>
                  {data.variables.filter((v) => v.level !== 'text').map((v) => (
                    <option key={v.id} value={v.name}>
                      {t('The values of {name}', { name: v.name })}
                    </option>
                  ))}
                </select>
                <p className="help">{t('Capitals and spaces at the ends do not matter. When two rows have the same key the first is used.')}</p>
              </div>
              <fieldset className="stats-fieldset" data-testid="merge-columns">
                <legend>{t('Columns to bring in')}</legend>
                <div className="table-wrap">
                  <table className="data">
                    <caption className="visually-hidden">{t('The columns of the file, and the variable each would become')}</caption>
                    <thead>
                      <tr>
                        <th scope="col">{t('Bring in')}</th>
                        <th scope="col">{t('Variable name')}</th>
                        <th scope="col">{t('Level')}</th>
                        <th scope="col">{t('A few of its values')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {columns.filter((c) => c.name !== key).map((c) => (
                        <tr key={c.name}>
                          <th scope="row">
                            <Tick isSelected={!!rows[c.name]?.include} onChange={(on) => setRow(c.name, { include: on })} testId={`merge-include-${c.name}`}>
                              {c.name}
                            </Tick>
                          </th>
                          <td>
                            <input aria-label={t('Variable name for {column}', { column: c.name })} type="text" value={rows[c.name]?.name ?? c.name} onChange={(e) => setRow(c.name, { name: e.target.value })} autoComplete="off" />
                          </td>
                          <td>
                            <select aria-label={t('Level for {column}', { column: c.name })} value={rows[c.name]?.level ?? c.level} onChange={(e) => setRow(c.name, { level: e.target.value as Level })}>
                              {LEVELS.map((l) => (
                                <option key={l} value={l}>
                                  {LEVEL_LABELS[l]}
                                </option>
                              ))}
                            </select>
                          </td>
                          <td>{c.examples.join(', ')}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </fieldset>
              {clash.length > 0 ? (
                <Tick isSelected={overwrite} onChange={setOverwrite} testId="merge-overwrite">
                  {t('Fill in the variables that exist already ({names}) for the cases the file has; the other cases keep what they have', { names: clash.map((c) => rows[c.name]?.name ?? c.name).join(', ') })}
                </Tick>
              ) : null}
              <div className="actions">
                <Button type="submit" className="primary" isDisabled={!canMerge} data-testid="merge-go">{t('Merge')}</Button>
                <Button type="button" onPress={props.onClose} data-testid="merge-close">{t('Close')}</Button>
              </div>
            </form>
          ) : (
            <div className="actions">
              <Button onPress={props.onClose}>{t('Close')}</Button>
            </div>
          )}

          {result ? (
            <section aria-labelledby="merge-result-h" data-testid="merge-result" className="ve-preview">
              <h3 id="merge-result-h" className="stats-h4">{t('What the merge did')}</h3>
              <ul>
                <li data-testid="merge-matched">
                  {tn(result.cases_matched, { one: '{count} case had a row in the file and now has the values.', other: '{count} cases had a row in the file and now have the values.' })}
                </li>
                {result.cases_unmatched > 0 ? (
                  <li data-testid="merge-unmatched-cases">
                    {tn(result.cases_unmatched, { one: '{count} case has no row in the file and has no value: {names}.', other: '{count} cases have no row in the file and have no value, such as {names}.' }, { names: result.unmatched_case_names.join(', ') })}
                  </li>
                ) : null}
                {result.rows_unmatched > 0 ? (
                  <li data-testid="merge-unmatched-rows">
                    {tn(result.rows_unmatched, { one: '{count} row of the file matches no case and brought nothing in: {keys}.', other: '{count} rows of the file match no case and brought nothing in, such as {keys}.' }, { keys: result.unmatched_keys.join(', ') })}
                  </li>
                ) : null}
                {result.rows_without_key > 0 ? <li>{tn(result.rows_without_key, { one: '{count} row of the file has no key and was left out.', other: '{count} rows of the file have no key and were left out.' })}</li> : null}
                {result.rows_repeated > 0 ? <li>{tn(result.rows_repeated, { one: '{count} row repeated a key an earlier row had, and was not used.', other: '{count} rows repeated a key an earlier row had, and were not used.' })}</li> : null}
                {result.variables_made.length > 0 ? <li data-testid="merge-made">{t('Made: {names}.', { names: result.variables_made.join(', ') })}</li> : null}
                {result.variables_filled.length > 0 ? <li>{t('Filled in: {names}.', { names: result.variables_filled.join(', ') })}</li> : null}
              </ul>
              <p className="help">{t('Each variable is a step in the history, so Undo the last change takes them back one at a time.')}</p>
            </section>
          ) : null}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
