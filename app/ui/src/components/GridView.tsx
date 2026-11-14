import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';
import { abbreviations, patternFor } from '../lib/labels';
import { interpretDocumentKey, type DocCommand } from '../lib/shortcuts';
import { unitsTarget, describeCount } from '../lib/targets';
import { threadRows, type ThreadRow } from '../lib/thread';
import { unitIndexAt } from '../lib/units';
import { sliceScalar } from '../offsets';
import { announce } from '../state/announce';
import { applyFavorite, findUncodedUnit, inVivo, repeatLastCode, selectCoding } from '../state/actions';
import { appStore, focusApplyBox, focusPane, setCenterFocus, setJumpProvider, setSelectionProvider, useApp, type SourceData } from '../state/app';
import type { CodeVisual } from '../surface/TextSurface';
import type { CodingRow, UnitRow } from '../types';
import { CodingsHere } from './CodingsHere';
import { SelectionLine } from './SelectionLine';
import { num, t, tn } from '../i18n';

const ROW_REM = 3.6;
const OVERSCAN = 6;

interface Row {
  unit: UnitRow;
  depth: number;
  /** Position in the text order, from 0. */
  index: number;
}

/** Codings that belong to each unit: by unit id when they have one, else by where they sit. */
function codingsByUnit(units: readonly UnitRow[], codings: readonly CodingRow[]): Map<number, CodingRow[]> {
  const map = new Map<number, CodingRow[]>();
  const add = (id: number, c: CodingRow) => {
    const list = map.get(id);
    if (list) list.push(c);
    else map.set(id, [c]);
  };
  for (const c of codings) {
    if (c.unit_id !== null) {
      add(c.unit_id, c);
      continue;
    }
    for (let i = Math.max(unitIndexAt(units, c.start), 0); i < units.length && units[i].start < c.end; i++) {
      if (units[i].end > c.start) add(units[i].id, c);
    }
  }
  return map;
}

/**
 * One row per unit, drawn only for the rows on screen, so a source with 15,000 units stays quick.
 * With `thread` set, replies are listed under the post they answer, indented.
 */
export function GridView(props: { data: SourceData; thread: boolean }) {
  const { data, thread } = props;
  const codes = useApp((s) => s.codes);
  const selectedCodingId = useApp((s) => s.selectedCodingId);
  const scroller = useRef<HTMLDivElement>(null);
  const [rowPx, setRowPx] = useState(60);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState(500);
  const [active, setActive] = useState(0);
  const [selected, setSelected] = useState<ReadonlySet<number>>(new Set());
  const anchor = useRef(0);

  const rows = useMemo<Row[]>(() => {
    if (thread) return threadRows(data.units).map((r: ThreadRow) => ({ unit: r.unit, depth: r.depth, index: r.index }));
    return data.units.map((unit, index) => ({ unit, depth: 0, index }));
  }, [data.units, thread]);
  const showSpeaker = useMemo(() => data.units.some((u) => u.speaker || u.excluded), [data.units]);
  const labels = useMemo(() => abbreviations(codes), [codes]);
  const visuals = useMemo(() => {
    const m = new Map<number, CodeVisual>();
    for (const c of codes) m.set(c.id, { id: c.id, name: c.name, label: labels.get(c.id) ?? '?', color: c.color, pattern: patternFor(c.color) });
    return m;
  }, [codes, labels]);
  const byUnit = useMemo(() => codingsByUnit(data.units, data.codings), [data.units, data.codings]);

  // Row height follows the text size, so zooming the text keeps rows readable.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const measure = () => {
      const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      setRowPx(Math.round(rem * ROW_REM));
      setViewport(el.clientHeight);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const total = rows.length;
  const first = Math.max(0, Math.floor(scrollTop / rowPx) - OVERSCAN);
  const last = Math.min(total - 1, Math.ceil((scrollTop + viewport) / rowPx) + OVERSCAN);
  const pageRows = Math.max(1, Math.floor(viewport / rowPx) - 1);

  const selectedUnits = useCallback(() => rows.filter((r) => selected.has(r.unit.id)).map((r) => r.unit), [rows, selected]);

  // What the toolbar and the code buttons apply to.
  useEffect(() => {
    setSelectionProvider(() => {
      const chosen = selectedUnits();
      if (chosen.length === 0 && rows[active]) return unitsTarget([rows[active].unit]);
      return unitsTarget(chosen);
    });
    setCenterFocus(() => scroller.current?.focus({ preventScroll: true }));
    return () => {
      setSelectionProvider(null);
      setCenterFocus(null);
    };
  }, [selectedUnits, rows, active]);

  // Keep the toolbar's "selected" line and the details area up to date.
  useEffect(() => {
    const chosen = selectedUnits();
    let text = '';
    if (chosen.length === 1) {
      const u = chosen[0];
      text = t("Selected row {row} of {total}: '{text}'", { row: rows.findIndex((r) => r.unit.id === u.id) + 1, total: num(total), text: sliceScalar(data.text, data.index, u.start, Math.min(u.end, u.start + 60)).replace(/\s+/g, ' ').trim() });
    } else if (chosen.length > 1) text = t('Selected {count} rows.', { count: num(chosen.length) });
    appStore.set({ selection: null, selectionText: text });
  }, [selectedUnits, rows, total, data.text, data.index]);

  // Start on the first row, and follow the active row with the scroll position.
  useEffect(() => {
    if (total > 0 && selected.size === 0) setSelected(new Set([rows[Math.min(active, total - 1)].unit.id]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows]);

  const ensureVisible = useCallback(
    (i: number) => {
      const el = scroller.current;
      if (!el) return;
      // The column header is 2rem tall and stays at the top of the scroll area.
      const headerPx = 2 * (parseFloat(getComputedStyle(document.documentElement).fontSize) || 16);
      const top = i * rowPx;
      if (top < el.scrollTop) el.scrollTop = top;
      else if (top + rowPx > el.scrollTop + el.clientHeight - headerPx) el.scrollTop = top + rowPx - el.clientHeight + headerPx;
    },
    [rowPx],
  );

  const moveTo = useCallback(
    (i: number, extend: boolean) => {
      const next = Math.min(Math.max(i, 0), total - 1);
      if (next < 0) return;
      setActive(next);
      if (extend) {
        const [a, b] = [Math.min(anchor.current, next), Math.max(anchor.current, next)];
        setSelected(new Set(rows.slice(a, b + 1).map((r) => r.unit.id)));
      } else {
        anchor.current = next;
        setSelected(new Set([rows[next].unit.id]));
      }
      ensureVisible(next);
    },
    [rows, total, ensureVisible],
  );

  const jump = useCallback(
    (direction: 1 | -1) => {
      const from = rows[active]?.unit.start ?? 0;
      const at = findUncodedUnit(from, direction);
      if (at < 0) {
        announce(direction === 1 ? t('No more units without a coding after this one.') : t('No earlier units without a coding.'));
        return;
      }
      const unit = data.units[at];
      const rowIndex = rows.findIndex((r) => r.unit.id === unit.id);
      moveTo(rowIndex, false);
      scroller.current?.focus({ preventScroll: true });
      announce(unit.speaker ? t('Row {rowIndex}, not coded yet, {speaker}.', { rowIndex: rowIndex + 1, speaker: unit.speaker }) : t('Row {rowIndex}, not coded yet.', { rowIndex: rowIndex + 1 }));
    },
    [rows, active, data.units, moveTo],
  );

  const onCommand = (cmd: DocCommand) => {
    switch (cmd.type) {
      case 'favorite':
        void applyFavorite(cmd.slot);
        break;
      case 'pick':
        focusApplyBox();
        break;
      case 'invivo':
        void inVivo();
        break;
      case 'repeat':
        void repeatLastCode();
        break;
      case 'next':
        jump(1);
        break;
      case 'previous':
        jump(-1);
        break;
      case 'examine': {
        const list = byUnit.get(rows[active]?.unit.id ?? -1) ?? [];
        if (list.length === 0) {
          announce(t('There is no coding in this row.'), 'assertive');
          break;
        }
        selectCoding(list[0].id);
        window.setTimeout(() => focusPane('pane-details'), 0);
        break;
      }
      case 'selectUnit':
        break;
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const extend = e.shiftKey;
    const plain = !e.ctrlKey && !e.altKey && !e.metaKey;
    if (plain) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          return moveTo(active + 1, extend);
        case 'ArrowUp':
          e.preventDefault();
          return moveTo(active - 1, extend);
        case 'PageDown':
          e.preventDefault();
          return moveTo(active + pageRows, extend);
        case 'PageUp':
          e.preventDefault();
          return moveTo(active - pageRows, extend);
        case 'Home':
          e.preventDefault();
          return moveTo(0, extend);
        case 'End':
          e.preventDefault();
          return moveTo(total - 1, extend);
        case ' ':
          e.preventDefault();
          return moveTo(active, false);
        default:
          break;
      }
    }
    const cmd = interpretDocumentKey(e.nativeEvent);
    if (cmd) {
      e.preventDefault();
      onCommand(cmd);
    }
  };

  const onRowClick = (e: MouseEvent, i: number) => {
    scroller.current?.focus({ preventScroll: true });
    if (e.shiftKey) return moveTo(i, true);
    if (e.ctrlKey || e.metaKey) {
      const id = rows[i].unit.id;
      setActive(i);
      anchor.current = i;
      setSelected((s) => {
        const n = new Set(s);
        if (n.has(id)) n.delete(id);
        else n.add(id);
        return n;
      });
      return;
    }
    moveTo(i, false);
  };

  const activeRow = rows[active];
  const hereCodings = activeRow ? (byUnit.get(activeRow.unit.id) ?? []) : [];
  const visible: Row[] = total === 0 ? [] : rows.slice(first, last + 1);
  const unitKind = data.units[0]?.kind;

  return (
    <div className="grid-view">
      <SelectionLine />
      <p id="grid-description" className="visually-hidden">{t('A list with one row per unit. Use the up and down arrow keys to move between rows, hold Shift to select several, and press 1 to 9 to apply a favorite code to the selected rows.')}</p>
      {total === 0 ? (
        <p className="empty">{t('This source has no units to list.')}</p>
      ) : (
        <div
          ref={scroller}
          className={`grid-scroll${showSpeaker ? '' : ' no-speaker'}`}
          role={thread ? 'treegrid' : 'grid'}
          aria-label={[data.name, unitKind === 'speaker_turn' ? tn(total, { one: '{count} turn', other: '{count} turns' }) : unitKind === 'survey_response' ? tn(total, { one: '{count} response', other: '{count} responses' }) : tn(total, { one: '{count} unit', other: '{count} units' }), thread ? t('replies indented') : ''].filter(Boolean).join(', ')}
          aria-describedby="grid-description"
          aria-rowcount={total + 1}
          aria-multiselectable="true"
          aria-activedescendant={activeRow ? `grid-row-${activeRow.unit.id}` : undefined}
          tabIndex={0}
          data-testid={thread ? 'thread' : 'grid'}
          onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
          onKeyDown={onKeyDown}
        >
          <div className="grid-head" role="row" aria-rowindex={1}>
            <div role="columnheader" className="c-num">
              #
            </div>
            {showSpeaker ? (
              <div role="columnheader" className="c-speaker">{t('Speaker')}</div>
            ) : null}
            <div role="columnheader" className="c-text">{t('Text')}</div>
            <div role="columnheader" className="c-codes">{t('Codes')}</div>
          </div>
          <div className="grid-body" style={{ height: total * rowPx }}>
            {visible.map((r, k) => {
              const i = first + k;
              const list = byUnit.get(r.unit.id) ?? [];
              const text = sliceScalar(data.text, data.index, r.unit.start, r.unit.end);
              const isSel = selected.has(r.unit.id);
              return (
                <div
                  key={r.unit.id}
                  id={`grid-row-${r.unit.id}`}
                  role="row"
                  aria-rowindex={i + 2}
                  aria-selected={isSel}
                  aria-level={thread ? r.depth + 1 : undefined}
                  className={`grid-row${isSel ? ' selected' : ''}${i === active ? ' active' : ''}${r.unit.excluded ? ' excluded' : ''}`}
                  style={{ top: i * rowPx, height: rowPx }}
                  onClick={(e) => onRowClick(e, i)}
                >
                  <div role="gridcell" className="c-num">
                    {num(r.index + 1)}
                  </div>
                  {showSpeaker ? (
                    <div role="gridcell" className="c-speaker">
                      {r.unit.speaker ?? ''}
                      {r.unit.excluded ? <span className="tag">{t('left out')}</span> : null}
                    </div>
                  ) : null}
                  <div role="gridcell" className="c-text" style={{ paddingInlineStart: `${r.depth * 1.25}rem` }}>
                    {r.depth > 0 ? <span className="visually-hidden">{t('Reply, level {depth}.', { depth: r.depth + 1 })} </span> : null}
                    <span className="clamp">{text}</span>
                  </div>
                  <div role="gridcell" className="c-codes">
                    {list.length === 0 ? (
                      <span className="muted">{t('none')}</span>
                    ) : (
                      list.slice(0, 4).map((c) => {
                        const v = visuals.get(c.code_id);
                        return (
                          <span key={c.id} className={`chip${c.id === selectedCodingId ? ' chosen' : ''}`} style={{ ['--chip-color' as string]: v?.color ?? '#808080' }}>
                            <span className={`chip-swatch pattern-${v?.pattern ?? 0}`} aria-hidden="true" />
                            <span className="chip-text">{v?.name ?? 'code'}</span>
                          </span>
                        );
                      })
                    )}
                    {list.length > 4 ? <span className="muted">+{list.length - 4}</span> : null}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
      <CodingsHere
        scopeLabel={activeRow ? t('Row {active} of {total}', { active: active + 1, total: num(total) }) : ''}
        scopeKind={activeRow ? 'unit' : 'none'}
        codings={hereCodings}
        text={data.text}
        index={data.index}
        visuals={visuals}
        caretCodes={[]}
        onOpen={(c) => selectCoding(c.id)}
      />
      <p className="visually-hidden" aria-live="off">
        {t('{count} selected', { count: tn(selected.size, { one: '{count} row', other: '{count} rows' }) })}</p>
    </div>
  );
}
