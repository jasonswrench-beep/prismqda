import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { abbreviations, patternFor } from '../lib/labels';
import { codingsAtCaret, paragraphsOf, scopeAt } from '../lib/scope';
import { documentTarget } from '../lib/targets';
import type { DocCommand } from '../lib/shortcuts';
import { unitIndexForCaret } from '../lib/units';
import { sliceScalar } from '../offsets';
import { announce, announcedRecently } from '../state/announce';
import { applyFavorite, findUncodedUnit, inVivo, repeatLastCode, selectCoding } from '../state/actions';
import { appStore, focusApplyBox, focusPane, setCenterFocus, setJumpProvider, setLineProvider, setSelectionProvider, setSelectionReader, useApp, type SourceData } from '../state/app';
import { TextSurface, type CodeVisual, type SurfaceHandle } from '../surface/TextSurface';
import type { ScalarSelection } from '../surface/domMap';
import { CodingsHere } from './CodingsHere';
import { SelectionLine } from './SelectionLine';
import { FloatingCodeBar } from './FloatingCodeBar';
import { describeCount } from '../lib/targets';
import { t, tn } from '../i18n';

const SELECTION_PREVIEW = 70;

export function DocumentView(props: { data: SourceData }) {
  const { data } = props;
  const codes = useApp((s) => s.codes);
  const mode = useApp((s) => s.project?.coding_mode ?? 'free_span');
  const selectedCodingId = useApp((s) => s.selectedCodingId);
  const announceCaret = useApp((s) => s.settings.announceCaret);
  const lineNumbers = useApp((s) => s.settings.lineNumbers);
  const floatingBar = useApp((s) => s.settings.floatingBar);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const handle = useRef<SurfaceHandle>(null);
  const [caret, setCaret] = useState(0);
  // The start of each unit is marked when the person asks, and always in a project that codes whole units, where the units are the work.
  const showUnits = useApp((s) => s.settings.showUnits || s.project?.coding_mode === 'unit');

  // A different source starts with the caret at its top.
  useEffect(() => {
    setCaret(0);
  }, [data.id]);

  const labels = useMemo(() => abbreviations(codes), [codes]);
  const visuals = useMemo(() => {
    const m = new Map<number, CodeVisual>();
    for (const c of codes) m.set(c.id, { id: c.id, name: c.name, label: labels.get(c.id) ?? '?', color: c.color, pattern: patternFor(c.color) });
    return m;
  }, [codes, labels]);
  const paragraphs = useMemo(() => paragraphsOf(data.text, data.index), [data.text, data.index]);
  const scope = useMemo(() => scopeAt({ paragraphs, units: data.units, mode, caret }), [paragraphs, data.units, mode, caret]);

  // Arriving from a search result or a retrieved passage: select that passage once the text is drawn.
  const arrival = useApp((s) => s.pendingJump);
  useEffect(() => {
    if (!arrival || arrival.sourceId !== data.id) return;
    const timer = window.setTimeout(() => {
      handle.current?.select(arrival.start, arrival.end, { focus: true, scroll: true });
      // The selection puts no coding in Segment Details by itself; the segment that was asked for is the one shown there.
      if (arrival.codingId !== undefined) selectCoding(arrival.codingId);
      appStore.set({ pendingJump: null });
    }, 60);
    return () => window.clearTimeout(timer);
  }, [arrival, data.id]);

  // The recording is playing and the text follows it: scroll to the turn that is playing, without taking the focus from the player.
  const follow = useApp((s) => s.mediaFollow);
  useEffect(() => {
    if (!follow || follow.sourceId !== data.id) return;
    handle.current?.scrollToOffset(follow.offset);
  }, [follow, data.id]);

  // What the toolbar and the code buttons apply to.
  useEffect(() => {
    setSelectionProvider(() =>
      documentTarget({ text: data.text, index: data.index, selection: handle.current?.getSelection() ?? null, mode, units: data.units }),
    );
    setCenterFocus((keep) => {
      // Coming back from a dialog, the words that were selected go back to being selected.
      if (keep && keep.end > keep.start && keep.end <= data.index.scalarLength) handle.current?.select(keep.start, keep.end, { focus: true, scroll: false });
      else handle.current?.focus();
    });
    setSelectionReader(() => {
      const s = handle.current?.getSelection();
      return s ? { start: s.start, end: s.end } : null;
    });
    return () => {
      setSelectionProvider(null);
      setCenterFocus(null);
      setSelectionReader(null);
    };
  }, [data.text, data.index, data.units, mode]);

  // The details panel names the lines a coding runs over; the surface knows where the lines are.
  useEffect(() => {
    setLineProvider((start, end) => handle.current?.lineRange(start, end) ?? null);
    return () => setLineProvider(null);
  }, []);
  const onLinesChange = useCallback(() => appStore.set((s) => ({ linesStamp: s.linesStamp + 1 })), []);

  const onSelection = useCallback(
    (sel: ScalarSelection | null) => {
      if (!sel) return;
      setCaret(sel.focus);
      if (sel.start === sel.end) {
        appStore.set({ selection: { start: sel.start, end: sel.end }, selectionText: '' });
        return;
      }
      const raw = sliceScalar(data.text, data.index, sel.start, sel.end).replace(/\s+/g, ' ').trim();
      const chars = Array.from(raw);
      const shown = chars.length > SELECTION_PREVIEW ? chars.slice(0, SELECTION_PREVIEW).join('') + '...' : raw;
      const n = sel.end - sel.start;
      appStore.set({ selection: { start: sel.start, end: sel.end }, selectionText: t("Selected: '{text}' ({count})", { text: shown, count: tn(n, { one: '{count} character', other: '{count} characters' }) }) });
    },
    [data.text, data.index],
  );

  const here = useMemo(() => codingsAtCaret(data.codings, caret, data.index.scalarLength), [data.codings, caret, data.index]);

  // Say which codes the caret is in, when that changes.
  const lastSaid = useRef<string>('');
  useEffect(() => {
    if (!announceCaret) return;
    const key = here.map((c) => c.code_id).join(',');
    if (key === lastSaid.current) return;
    const timer = window.setTimeout(() => {
      const previous = lastSaid.current;
      lastSaid.current = key;
      if (previous === '' && key === '') return;
      // After an action ("Coded ... as ...") the caret note would only talk over it.
      if (announcedRecently(2500)) return;
      if (here.length === 0) announce(t('Outside any coding.'), 'polite', { background: true });
      else {
        const names = here.map((c) => visuals.get(c.code_id)?.name ?? t('a code'));
        announce(t('In {list}.', { list: names.join(', ') }), 'polite', { background: true });
      }
    }, 450);
    return () => window.clearTimeout(timer);
  }, [here, announceCaret, visuals]);

  const jump = useCallback(
    (direction: 1 | -1) => {
      if (data.units.length === 0) {
        announce(t('This source has no units.'), 'assertive');
        return;
      }
      const from = handle.current?.getSelection()?.focus ?? 0;
      const at = findUncodedUnit(from, direction);
      if (at < 0) {
        announce(direction === 1 ? t('No more units without a coding after this one.') : t('No earlier units without a coding.'));
        return;
      }
      const u = data.units[at];
      if (mode === 'unit') handle.current?.select(u.start, u.start, { focus: true });
      else handle.current?.select(u.start, u.end, { focus: true });
      const words = sliceScalar(data.text, data.index, u.start, Math.min(u.end, u.start + 60)).replace(/\s+/g, ' ').trim();
      announce(u.speaker ? t('Unit {at} of {count}, not coded yet, {speaker}: {words}', { at: at + 1, count: data.units.length, speaker: u.speaker, words }) : t('Unit {at} of {count}, not coded yet: {words}', { at: at + 1, count: data.units.length, words }));
    },
    [data, mode],
  );

  const onCommand = useCallback(
    (cmd: DocCommand) => {
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
          const sel = handle.current?.getSelection();
          const at = codingsAtCaret(appStore.get().sourceData?.codings ?? [], sel?.focus ?? 0, data.index.scalarLength);
          if (at.length === 0) {
            announce(t('There is no coding at the caret.'), 'assertive');
            break;
          }
          selectCoding(at[0].id);
          window.setTimeout(() => focusPane('pane-details'), 0);
          break;
        }
        case 'selectUnit': {
          const sel = handle.current?.getSelection();
          const i = unitIndexForCaret(data.units, sel?.focus ?? 0);
          if (i < 0) {
            announce(t('The caret is not inside a unit.'), 'assertive');
            break;
          }
          const u = data.units[i];
          handle.current?.select(u.start, u.end, { scroll: false });
          announce(t('Selected unit {i} of {count}.', { i: i + 1, count: data.units.length }));
          break;
        }
      }
    },
    [data, jump],
  );

  const onStripeClick = useCallback(
    (id: number) => {
      const c = appStore.get().sourceData?.codings.find((x) => x.id === id);
      selectCoding(id);
      if (c) handle.current?.select(c.start, c.end, { focus: true, scroll: false });
    },
    [],
  );

  const hasUnits = data.units.length > 0;
  // Next and Previous Uncoded are commands on the ribbon; they call this.
  useEffect(() => {
    setJumpProvider(hasUnits ? jump : null);
    return () => setJumpProvider(null);
  }, [hasUnits, jump]);
  return (
    <div className="document-view">
      <p id="doc-description" className="visually-hidden">{t('Read-only text. Move the caret with the arrow keys and hold Shift to select. Press 1 to 9 to apply a favorite code, Enter to find a code by name, I for in-vivo coding, R to repeat the last code, and N for the next unit without a coding.')}</p>
      <TextSurface
        text={data.text}
        index={data.index}
        runs={data.runs}
        units={data.units}
        codings={data.codings}
        codes={visuals}
        showUnits={showUnits && hasUnits}
        lineNumbers={lineNumbers}
        onLinesChange={onLinesChange}
        selectedCodingId={selectedCodingId}
        labelId="h-source"
        descriptionId="doc-description"
        floating={readOnly || !floatingBar ? undefined : <FloatingCodeBar />}
        handleRef={handle}
        onSelection={onSelection}
        onCommand={onCommand}
        onStripeClick={onStripeClick}
      />
      <SelectionLine />
      <CodingsHere
        scopeLabel={scope.label}
        scopeKind={scope.kind}
        codings={scope.kind === 'none' ? [] : data.codings.filter((c) => c.start < scope.end && c.end > scope.start)}
        text={data.text}
        index={data.index}
        visuals={visuals}
        caretCodes={here.map((c) => c.id)}
        onOpen={(c) => {
          selectCoding(c.id);
          handle.current?.select(c.start, c.end, { focus: false, scroll: true });
        }}
      />
    </div>
  );
}
