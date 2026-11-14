import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { Button, Tab, TabList, TabPanel, Tabs } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { LEVEL_LABELS } from '../lib/importMapping';
import { ROWS_PER_PAGE, cellShown, cellSpoken, derivedLine, transformKinds } from '../lib/variables';
import { bumpData, goTo, undoLast } from '../state/actions';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type { CaseData, SortKeySpec, TransformKindId, VariableDef } from '../types';
import { Notice } from './common';
import { MergeDialog, SortDialog, TablesDialog } from './DataTools';
import { DeleteVariableDialog, TransformDialog, VariableDialog } from './VariableDialogs';
import { num, t, tn } from '../i18n';

type Dialog =
  | { kind: 'variable'; existing: VariableDef | null }
  | { kind: 'transform'; transform: TransformKindId }
  | { kind: 'delete'; variable: VariableDef }
  | { kind: 'sort' }
  | { kind: 'tables' }
  | { kind: 'merge' };

/**
 * The data editor: the variables of cases and their values. The data view is a grid with a row for each case and a column for each
 * variable, edited in place; the variable view lists the variables and what each one says (its level, value labels, and the values
 * that mean missing). New variables are made from others by formulas and recodings, and every change is one step that Undo takes back.
 */
export function VariablesView() {
  const path = useApp((s) => s.project?.path ?? null);
  const version = useApp((s) => s.dataVersion);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [data, setData] = useState<CaseData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<'data' | 'variables'>('data');
  const [dialog, setDialog] = useState<Dialog | null>(null);
  // The order the data view shows the cases in, when the person has sorted them: the keys, and the cases' numbers in that order. Nothing in
  // the project is changed by a sort.
  const [sortKeys, setSortKeys] = useState<SortKeySpec[]>([]);
  const [order, setOrder] = useState<number[] | null>(null);

  useEffect(() => {
    if (sortKeys.length === 0) {
      setOrder(null);
      return;
    }
    let live = true;
    call('data.sort', { keys: sortKeys })
      .then((r) => live && setOrder(r.case_ids))
      .catch((e) => {
        if (!live) return;
        setError(toApiError(e).message);
        setSortKeys([]);
      });
    return () => {
      live = false;
    };
  }, [sortKeys, version, path]);

  useEffect(() => {
    let live = true;
    call('data.get', {})
      .then((d) => {
        if (!live) return;
        setData(d);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [path, version]);

  /** A value just saved shows at once, before the whole table is read again. */
  const patch = useCallback((row: number, col: number, value: string | null) => {
    setData((d) => (d ? { ...d, values: d.values.map((vs, i) => (i === row ? vs.map((x, j) => (j === col ? value : x)) : vs)) } : d));
  }, []);

  // The data in the order shown, and where each shown row is in the data as the engine has it.
  const shown = useMemo(() => {
    if (!data || !order) return { data, rowOf: null as number[] | null };
    const at = new Map(data.cases.map((c, i) => [c.id, i]));
    const rowOf = order.map((id) => at.get(id)).filter((i): i is number => i !== undefined);
    if (rowOf.length !== data.cases.length) return { data, rowOf: null as number[] | null };
    return { data: { ...data, cases: rowOf.map((i) => data.cases[i]), values: rowOf.map((i) => data.values[i]) }, rowOf };
  }, [data, order]);
  const showPatch = useCallback((row: number, col: number, value: string | null) => patch(shown.rowOf ? shown.rowOf[row] : row, col, value), [patch, shown.rowOf]);
  const sortedWords = sortKeys
    .map((k) => `${k.variable === 'Case' ? t('the name of the case') : k.variable} ${k.descending ? t('(high to low)') : t('(low to high)')}`)
    .join(', ');

  const noCases = data !== null && data.cases.length === 0;
  const kinds = transformKinds();

  return (
    <main id="main-content" tabIndex={-1} className="view-page ve-page" aria-labelledby="h-ve">
      <h2 id="h-ve">{t('Data editor')}</h2>
      <p className="help">
        {t('The variables of your cases and their values, one row for each case. Type values in, give them labels, say which values mean missing, and make new variables from others with a formula, a recoding, z-scores, ranks, or ranges. Statistics reads these variables.')}
      </p>
      {error ? <Notice>{error}</Notice> : null}

      <div className="actions ve-toolbar" role="toolbar" aria-label={t('Make and change variables')}>
        <Button className="primary" isDisabled={readOnly} onPress={() => setDialog({ kind: 'variable', existing: null })} data-testid="ve-add">
          {t('Add a variable')}
        </Button>
        {kinds.map((k) => (
          <Button key={k.id} isDisabled={readOnly || noCases || data === null} onPress={() => setDialog({ kind: 'transform', transform: k.id })} data-testid={`ve-make-${k.id}`}>
            {k.label}…
          </Button>
        ))}
        <Button isDisabled={noCases || data === null} onPress={() => setDialog({ kind: 'sort' })} data-testid="ve-sort">{t('Sort cases…')}</Button>
        <Button isDisabled={readOnly || noCases || data === null} onPress={() => setDialog({ kind: 'merge' })} data-testid="ve-merge">{t('Merge from a file…')}</Button>
        <Button isDisabled={noCases || data === null} onPress={() => setDialog({ kind: 'tables' })} data-testid="ve-tables">{t('Tables and restructuring…')}</Button>
        <Button isDisabled={readOnly} onPress={() => void undoLast()} data-testid="ve-undo">{t('Undo the last change')}</Button>
        <Button onPress={() => goTo('stats')} data-testid="ve-to-stats">{t('Go to Statistics')}</Button>
      </div>

      {noCases ? (
        <Notice kind="info">{t('This project has no cases yet. A case is the person, site, or family you compare. Importing a spreadsheet with a column for it creates them, and so does New case on the Data page.')}</Notice>
      ) : null}

      {sortKeys.length > 0 ? (
        <p className="muted" role="status" data-testid="ve-sorted">
          {t('The cases are shown sorted by {keys}. Nothing in the project was moved.', { keys: sortedWords })}{' '}
          <Button className="link-button" onPress={() => setSortKeys([])} data-testid="ve-unsort">{t('Show them in their own order')}</Button>
        </p>
      ) : null}

      <Tabs selectedKey={tab} onSelectionChange={(k) => setTab(k as 'data' | 'variables')} className="bk-editor-tabs">
        <TabList aria-label={t('Views of the data')}>
          <Tab id="data">{t('Data view')}</Tab>
          <Tab id="variables">{t('Variable view')}</Tab>
        </TabList>
        <TabPanel id="data">{shown.data ? <DataGrid data={shown.data} readOnly={readOnly} patch={showPatch} /> : <p className="muted" role="status">{t('Loading…')}</p>}</TabPanel>
        <TabPanel id="variables">
          {data ? (
            <VariableTable
              data={data}
              readOnly={readOnly}
              onEdit={(v) => setDialog({ kind: 'variable', existing: v })}
              onDelete={(v) => setDialog({ kind: 'delete', variable: v })}
            />
          ) : (
            <p className="muted" role="status">{t('Loading…')}</p>
          )}
        </TabPanel>
      </Tabs>

      {dialog?.kind === 'variable' ? <VariableDialog existing={dialog.existing} onClose={() => setDialog(null)} /> : null}
      {dialog?.kind === 'delete' ? <DeleteVariableDialog variable={dialog.variable} onClose={() => setDialog(null)} /> : null}
      {dialog?.kind === 'transform' && data ? <TransformDialog kind={dialog.transform} data={data} onClose={() => setDialog(null)} /> : null}
      {dialog?.kind === 'sort' && data ? <SortDialog data={data} keys={sortKeys} onApply={setSortKeys} onClose={() => setDialog(null)} /> : null}
      {dialog?.kind === 'tables' && data ? <TablesDialog data={data} onClose={() => setDialog(null)} /> : null}
      {dialog?.kind === 'merge' && data ? <MergeDialog data={data} onClose={() => setDialog(null)} /> : null}
    </main>
  );
}

// ---------------------------------------------------------------- the data view

interface Editing {
  r: number;
  c: number;
  draft: string;
}

function DataGrid({ data, readOnly, patch }: { data: CaseData; readOnly: boolean; patch: (row: number, col: number, value: string | null) => void }) {
  const [pageIndex, setPageIndex] = useState(0);
  const pages = Math.max(1, Math.ceil(data.cases.length / ROWS_PER_PAGE));
  const page = Math.min(pageIndex, pages - 1);
  const first = page * ROWS_PER_PAGE;
  const last = Math.min(data.cases.length, first + ROWS_PER_PAGE);
  const rowCount = last - first;
  const colCount = data.variables.length;
  const [active, setActive] = useState({ r: 0, c: 0 });
  const [editing, setEditing] = useState<Editing | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const table = useRef<HTMLTableElement>(null);
  const wantFocus = useRef(false);
  const committing = useRef(false);
  // What is being edited as of the last drawing, so a blur that comes from the box being taken away is not mistaken for leaving it.
  const editingNow = useRef<Editing | null>(null);
  editingNow.current = editing;

  // A cell takes the focus after a move or after an edit closes, once it has been drawn.
  useEffect(() => {
    if (!wantFocus.current || editing) return;
    wantFocus.current = false;
    table.current?.querySelector<HTMLElement>(`[data-r="${active.r}"][data-c="${active.c}"]`)?.focus();
  });

  if (colCount === 0) {
    return <p className="muted" data-testid="ve-no-variables">{t('There are no variables of cases yet. Add one, or import a spreadsheet with columns for the cases.')}</p>;
  }

  const clampRow = (r: number) => Math.max(0, Math.min(rowCount - 1, r));
  const clampCol = (c: number) => Math.max(0, Math.min(colCount - 1, c));
  const moveTo = (r: number, c: number) => {
    wantFocus.current = true;
    setActive({ r: clampRow(r), c: clampCol(c) });
  };

  const startEdit = (r: number, c: number, draft?: string) => {
    if (readOnly) return announce(t('This project is read only, so values cannot be changed.'), 'assertive');
    setProblem(null);
    setActive({ r, c });
    setEditing({ r, c, draft: draft ?? data.values[first + r][c] ?? '' });
  };

  const close = (move: 'down' | 'right' | 'stay', e: Editing) => {
    setEditing(null);
    wantFocus.current = true;
    if (move === 'down') setActive({ r: clampRow(e.r + 1), c: e.c });
    else if (move === 'right') setActive({ r: e.r, c: clampCol(e.c + 1) });
    else setActive({ r: e.r, c: e.c });
  };

  /** Saves one cell as one step (so one Undo takes it back). A value the variable cannot hold stays in the box with the reason. */
  const commit = async (e: Editing, move: 'down' | 'right' | 'stay', toValue?: string | null) => {
    if (committing.current) return;
    const row = first + e.r;
    const variable = data.variables[e.c];
    const prev = data.values[row][e.c] ?? null;
    const next = toValue !== undefined ? toValue : e.draft.trim() === '' ? null : e.draft.trim();
    if (prev === next) return close(move, e);
    committing.current = true;
    try {
      await call('variable.set_values', { variable: variable.name, changes: [{ case_id: data.cases[row].id, value: next }] });
      setProblem(null);
      patch(row, e.c, next);
      bumpData();
      announce(next === null ? t('Cleared {variable} for {case}.', { variable: variable.name, case: data.cases[row].name }) : t('Set {variable} for {case} to {value}.', { variable: variable.name, case: data.cases[row].name, value: next }));
      close(move, e);
    } catch (err) {
      const message = toApiError(err).message;
      setProblem(message);
      announce(message, 'assertive');
    } finally {
      committing.current = false;
    }
  };

  const onKeyDown = (ev: KeyboardEvent<HTMLTableElement>) => {
    if (editing) return;
    const cell = (ev.target as HTMLElement).closest<HTMLElement>('[data-r]');
    if (!cell || ev.target !== cell) return;
    const r = Number(cell.dataset.r);
    const c = Number(cell.dataset.c);
    const ctrl = ev.ctrlKey || ev.metaKey;
    const go = (nr: number, nc: number) => {
      ev.preventDefault();
      moveTo(nr, nc);
    };
    switch (ev.key) {
      case 'ArrowDown':
        return go(r + 1, c);
      case 'ArrowUp':
        return go(r - 1, c);
      case 'ArrowLeft':
        return go(r, c - 1);
      case 'ArrowRight':
        return go(r, c + 1);
      case 'Home':
        return ctrl ? go(0, 0) : go(r, 0);
      case 'End':
        return ctrl ? go(rowCount - 1, colCount - 1) : go(r, colCount - 1);
      case 'PageDown':
        return go(r + 10, c);
      case 'PageUp':
        return go(r - 10, c);
      case 'Enter':
      case 'F2':
        ev.preventDefault();
        return startEdit(r, c);
      case 'Delete':
      case 'Backspace':
        ev.preventDefault();
        if (readOnly) return announce(t('This project is read only, so values cannot be changed.'), 'assertive');
        return void commit({ r, c, draft: '' }, 'stay', null);
      default:
        if (ev.key.length === 1 && !ctrl && !ev.altKey) {
          ev.preventDefault();
          startEdit(r, c, ev.key);
        }
    }
  };

  const rows = Array.from({ length: rowCount }, (_, r) => r);
  return (
    <div className="ve-data" data-testid="ve-data">
      {problem ? <Notice>{problem}</Notice> : null}
      <p id="ve-grid-help" className="help">
        {t('Arrow keys move between cells. Press Enter or F2 to change a cell, or just start typing. Enter saves and moves down, Tab saves and moves right, Escape puts the old value back, and Delete clears the cell. Values in italics are counted as missing.')}
      </p>
      <div className="table-wrap ve-scroll" tabIndex={-1}>
        <table
          ref={table}
          className="data ve-grid"
          role="grid"
          aria-label={t('Values of the variables, one row for each case')}
          aria-describedby="ve-grid-help"
          aria-rowcount={data.cases.length + 1}
          aria-colcount={colCount + 1}
          onKeyDown={onKeyDown}
          data-testid="ve-grid"
        >
          <thead>
            <tr role="row" aria-rowindex={1}>
              <th role="columnheader" scope="col" aria-colindex={1}>{t('Case')}</th>
              {data.variables.map((v, c) => (
                <th key={v.id} role="columnheader" scope="col" aria-colindex={c + 2} className={numeric(v) ? 'num' : undefined}>
                  {v.name}
                  <span className="ve-level">{LEVEL_LABELS[v.level]}</span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const row = first + r;
              return (
                <tr key={data.cases[row].id} role="row" aria-rowindex={row + 2}>
                  <th role="rowheader" scope="row" aria-colindex={1}>{data.cases[row].name}</th>
                  {data.variables.map((v, c) => {
                    const raw = data.values[row][c] ?? null;
                    const shown = cellShown(v, raw);
                    const isActive = active.r === r && active.c === c;
                    const isEditing = editing !== null && editing.r === r && editing.c === c;
                    return (
                      <td
                        key={v.id}
                        role="gridcell"
                        aria-colindex={c + 2}
                        aria-readonly={readOnly || undefined}
                        aria-label={isEditing ? undefined : cellSpoken(v, raw)}
                        tabIndex={isActive && !isEditing ? 0 : -1}
                        data-r={r}
                        data-c={c}
                        className={[numeric(v) ? 'num' : '', shown.missing ? 've-missing' : '', isActive ? 've-active' : ''].filter(Boolean).join(' ')}
                        onFocus={(e) => {
                          if (e.target === e.currentTarget && !isActive) setActive({ r, c });
                        }}
                        onClick={() => setActive({ r, c })}
                        onDoubleClick={() => startEdit(r, c)}
                      >
                        {isEditing && editing ? (
                          <input
                            className="ve-input"
                            type="text"
                            value={editing.draft}
                            aria-label={t('{variable} for {case}', { variable: v.name, case: data.cases[row].name })}
                            aria-invalid={problem !== null || undefined}
                            ref={(el) => {
                              if (el && !el.dataset.ready) {
                                el.dataset.ready = '1';
                                el.focus();
                                el.setSelectionRange(el.value.length, el.value.length);
                              }
                            }}
                            onChange={(e) => setEditing({ ...editing, draft: e.target.value })}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                e.preventDefault();
                                void commit(editing, 'down');
                              } else if (e.key === 'Tab' && !e.shiftKey) {
                                e.preventDefault();
                                void commit(editing, 'right');
                              } else if (e.key === 'Escape') {
                                e.preventDefault();
                                e.stopPropagation();
                                setProblem(null);
                                close('stay', editing);
                              }
                            }}
                            onBlur={() => {
                              const now = editingNow.current;
                              if (now && !committing.current) void commit(now, 'stay');
                            }}
                            data-testid="ve-input"
                          />
                        ) : (
                          shown.text
                        )}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {pages > 1 ? (
        <nav className="ve-pager" aria-label={t('Pages of cases')}>
          <Button isDisabled={page === 0} onPress={() => { setPageIndex(page - 1); setActive({ r: 0, c: active.c }); }} data-testid="ve-prev">{t('Earlier cases')}</Button>
          <span role="status" data-testid="ve-range">{t('Cases {from} to {to} of {total}', { from: num(first + 1), to: num(last), total: num(data.cases.length) })}</span>
          <Button isDisabled={page >= pages - 1} onPress={() => { setPageIndex(page + 1); setActive({ r: 0, c: active.c }); }} data-testid="ve-next">{t('Later cases')}</Button>
        </nav>
      ) : (
        <p className="muted" data-testid="ve-range">{tn(data.cases.length, { one: '{count} case', other: '{count} cases' })}</p>
      )}
    </div>
  );
}

function numeric(v: VariableDef): boolean {
  return v.level === 'interval' || v.level === 'ratio';
}

// ---------------------------------------------------------------- the variable view

function VariableTable({ data, readOnly, onEdit, onDelete }: { data: CaseData; readOnly: boolean; onEdit: (v: VariableDef) => void; onDelete: (v: VariableDef) => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const codes = useApp((s) => s.codes);
  const codeNameOf = (id: number) => codes.find((c) => c.id === id)?.name ?? t('code {id}', { id });

  const recompute = async (v: VariableDef) => {
    setBusy(v.name);
    setProblem(null);
    try {
      const done = await call('variable.recompute', { name: v.name });
      bumpData();
      announce(tn(done.cases_with_value, { one: 'Made {name} again from the variables as they are now. One case has a value.', other: 'Made {name} again from the variables as they are now. {count} cases have a value.' }, { name: v.name }));
    } catch (e) {
      const message = toApiError(e).message;
      setProblem(message);
      announce(message, 'assertive');
    } finally {
      setBusy(null);
    }
  };

  if (data.variables.length === 0) {
    return <p className="muted" data-testid="ve-no-variables">{t('There are no variables of cases yet. Add one, or import a spreadsheet with columns for the cases.')}</p>;
  }
  return (
    <div className="ve-variables" data-testid="ve-variables">
      {problem ? <Notice>{problem}</Notice> : null}
      <p className="help">{t('A variable made from others keeps how it was made. If the variables it came from change, Make it again brings it up to date.')}</p>
      <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The variables of cases')}>
        <table className="data">
          <caption className="visually-hidden">{t('The variables of cases and what each one says')}</caption>
          <thead>
            <tr>
              <th scope="col">{t('Name')}</th>
              <th scope="col">{t('Level')}</th>
              <th scope="col">{t('Value labels')}</th>
              <th scope="col">{t('Counted as missing')}</th>
              <th scope="col">{t('How it was made')}</th>
              <th scope="col">{t('Actions')}</th>
            </tr>
          </thead>
          <tbody>
            {data.variables.map((v) => {
              const labels = Object.entries(v.value_labels);
              const made = derivedLine(v, codeNameOf);
              return (
                <tr key={v.id} data-testid="ve-variable">
                  <th scope="row">{v.name}</th>
                  <td>{LEVEL_LABELS[v.level]}</td>
                  <td>
                    {labels.length === 0
                      ? t('None')
                      : labels
                          .slice(0, 4)
                          .map(([k, l]) => `${k} = ${l}`)
                          .join('; ') + (labels.length > 4 ? t(' and {n} more', { n: num(labels.length - 4) }) : '')}
                  </td>
                  <td>{v.missing_codes.length === 0 ? t('Only empty values') : v.missing_codes.join(', ')}</td>
                  <td>{made ?? t('Typed in or imported')}</td>
                  <td>
                    <div className="ve-actions">
                      <Button isDisabled={readOnly} onPress={() => onEdit(v)} aria-label={t('Change {name}', { name: v.name })} data-testid="ve-edit">{t('Change')}</Button>
                      {made ? (
                        <Button isDisabled={readOnly || busy !== null} onPress={() => void recompute(v)} aria-label={t('Make {name} again', { name: v.name })} data-testid="ve-recompute">{t('Make it again')}</Button>
                      ) : null}
                      <Button isDisabled={readOnly} onPress={() => onDelete(v)} aria-label={t('Delete {name}', { name: v.name })} data-testid="ve-delete">{t('Delete')}</Button>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
