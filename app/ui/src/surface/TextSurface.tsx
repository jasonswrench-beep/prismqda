// The reading and coding surface for text.
//
// The text sits in an element that has a real caret but cannot be changed: contenteditable with
// every input blocked, marked role="textbox" aria-readonly. The browser then gives keyboard users
// a caret and Shift+arrow selection for free, and screen readers treat it as a read-only text box.
// Stripes in the margin show the codings; the "Codings here" panel is their text equivalent.
// See README.md for why this was chosen over the other approaches.

import { useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode, type Ref } from 'react';
import type { OffsetIndex } from '../offsets';
import { edgeLines, placeBar, sameSelection, type Box, type Placement } from '../lib/floatbar';
import { highlightBackground } from '../lib/labels';
import { assignLanes } from '../lib/lanes';
import { groupLines, lineLabel, lineRangeFor, linesBetween, type Line } from '../lib/lines';
import { findSpeakerLabels, labelColumn } from '../lib/speakers';
import { buildSegments } from '../lib/segments';
import { interpretDocumentKey, type DocCommand } from '../lib/shortcuts';
import type { CodingRow, FormatRun, UnitRow } from '../types';
import { buildSurfaceDom, readSelection, rangeFor, scalarToPoint, segmentAt, writeSelection, type ScalarSelection, type SurfaceMap } from './domMap';
import { t } from '../i18n';

export interface CodeVisual {
  id: number;
  name: string;
  /** Short text label, such as "WEL". */
  label: string;
  color: string;
  pattern: number;
}

export interface SurfaceHandle {
  focus(): void;
  /** The live selection when the text has the focus, otherwise the last one it had. */
  getSelection(): ScalarSelection | null;
  select(start: number, end: number, opts?: { focus?: boolean; scroll?: boolean }): void;
  scrollToOffset(offset: number): void;
  element(): HTMLElement | null;
  /** The numbers of the first and last lines (as drawn on screen) that the text from `start` to `end` runs over. */
  lineRange(start: number, end: number): { first: number; last: number } | null;
}

interface Stripe {
  id: number;
  lane: number;
  top: number;
  height: number;
  code: CodeVisual;
}

const MAX_LANES = 5;
/** The margin always has room for this many stripes side by side, so the text does not jump when the first coding is made. */
const MIN_LANES = 1;
const LANE_REM = 0.9;

interface Props {
  text: string;
  index: OffsetIndex;
  runs: FormatRun[];
  units: UnitRow[];
  codings: CodingRow[];
  codes: ReadonlyMap<number, CodeVisual>;
  showUnits: boolean;
  /** Number the lines of text in a column at the left. */
  lineNumbers: boolean;
  /** Called after the lines are measured, so other parts of the screen that name lines can draw again. */
  onLinesChange?: () => void;
  selectedCodingId: number | null;
  labelId: string;
  descriptionId: string;
  /** Names the text for the checks that look for it; the reader's own is "surface". */
  testId?: string;
  handleRef: Ref<SurfaceHandle>;
  /** Shown above words selected with the mouse (the floating coding bar). Leave it out to show nothing. */
  floating?: ReactNode;
  onSelection: (sel: ScalarSelection | null) => void;
  onCommand: (cmd: DocCommand) => void;
  onStripeClick: (codingId: number) => void;
}

const UNKNOWN: CodeVisual = { id: -1, get name() { return t('Unknown code'); }, label: '?', color: '#808080', pattern: 0 };

export function TextSurface(props: Props) {
  const { text, index, runs, units, codings, codes, showUnits, selectedCodingId } = props;
  const rootRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<SurfaceMap | null>(null);
  const lastSelection = useRef<ScalarSelection | null>(null);
  const chosenEls = useRef<HTMLElement[]>([]);
  const [stripes, setStripes] = useState<Stripe[]>([]);
  const [laneCount, setLaneCount] = useState(MIN_LANES);
  const [overflow, setOverflow] = useState(0);
  const selectedRef = useRef<number | null>(selectedCodingId);
  selectedRef.current = selectedCodingId;
  const onSelectionRef = useRef(props.onSelection);
  onSelectionRef.current = props.onSelection;
  const onCommandRef = useRef(props.onCommand);
  onCommandRef.current = props.onCommand;

  // The floating bar: shown for words the mouse selected, until the selection changes some other way, the text
  // loses the focus, or Escape is pressed.
  const [bar, setBar] = useState<{ start: number; end: number } | null>(null);
  const [place, setPlace] = useState<Placement | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const barSel = useRef(bar);
  barSel.current = bar;
  const pressing = useRef(false);
  const hasFloating = props.floating !== undefined && props.floating !== null;
  const hasFloatingRef = useRef(hasFloating);
  hasFloatingRef.current = hasFloating;

  // Speaker labels ("Alex: ") are drawn in a column of their own, with the words hanging beside them.
  const labels = useMemo(() => findSpeakerLabels(text, index), [text, index]);
  const speakerColumn = useMemo(() => labelColumn(labels), [labels]);
  const segments = useMemo(
    () => buildSegments({ length: index.scalarLength, codings, runs, units, labels }),
    [index, codings, runs, units, labels],
  );

  // The color each coding is highlighted in: its code's color, so coded words look marked with a highlighter.
  const colorByCoding = useMemo(() => {
    const m = new Map<number, string>();
    for (const c of codings) m.set(c.id, (codes.get(c.code_id) ?? UNKNOWN).color);
    return m;
  }, [codings, codes]);
  // Only a change in the colors themselves redraws the text; a refreshed list of codes with the same colors does not.
  const colorSig = useMemo(() => Array.from(colorByCoding, ([id, hex]) => `${id}${hex}`).join('|'), [colorByCoding]);
  const highlightRef = useRef<(ids: readonly number[]) => string>(() => '');
  highlightRef.current = (ids) => {
    const hexes: string[] = [];
    for (const id of ids) {
      const hex = colorByCoding.get(id);
      if (hex && !hexes.includes(hex)) hexes.push(hex);
    }
    return highlightBackground(hexes);
  };
  const highlight = useCallback((ids: readonly number[]) => highlightRef.current(ids), []);

  // ---- the line numbers ----
  const [lines, setLines] = useState<Line[]>([]);
  const linesRef = useRef<Line[]>([]);
  const [view, setView] = useState({ from: -800, to: 2400 });
  const onLinesChangeRef = useRef(props.onLinesChange);
  onLinesChangeRef.current = props.onLinesChange;

  /** Reads where each line of text sits. Lines are the ones drawn on screen, so this runs again whenever the layout can have changed. */
  const measureLines = useCallback(() => {
    const root = rootRef.current;
    const body = bodyRef.current;
    if (!root || !body) return;
    let next: Line[] = [];
    // Measured even with the numbers hidden: the details name the lines a coding runs over.
    if (root.firstChild) {
      const bodyTop = body.getBoundingClientRect().top;
      const range = document.createRange();
      range.selectNodeContents(root);
      next = groupLines(Array.from(range.getClientRects(), (r) => ({ top: r.top - bodyTop, bottom: r.bottom - bodyTop, left: r.left, right: r.right })));
    }
    const prev = linesRef.current;
    const same = prev.length === next.length && next.every((l, i) => Math.abs(l.top - prev[i].top) < 0.5 && Math.abs(l.bottom - prev[i].bottom) < 0.5);
    if (same) return;
    linesRef.current = next;
    setLines(next);
    onLinesChangeRef.current?.();
  }, []);

  const updateView = useCallback(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    // Quantized, so scrolling a little does not draw the numbers again; the window is a screen or so either side.
    const from = Math.floor((scroller.scrollTop - 800) / 400) * 400;
    const to = from + scroller.clientHeight + 1600 + 400;
    setView((v) => (v.from === from && v.to === to ? v : { from, to }));
  }, []);

  // ---- measuring the stripes ----
  const measure = useCallback(() => {
    const map = mapRef.current;
    const body = bodyRef.current;
    if (!map || !body) return;
    measureLines();
    const bodyTop = body.getBoundingClientRect().top;
    const boxes: { id: number; top: number; bottom: number }[] = [];
    for (const c of codings) {
      const range = rangeFor(map, c.start, c.end);
      if (!range) continue;
      const rects = range.getClientRects();
      if (rects.length === 0) continue;
      let top = Infinity;
      let bottom = -Infinity;
      for (let i = 0; i < rects.length; i++) {
        if (rects[i].height === 0) continue;
        top = Math.min(top, rects[i].top);
        bottom = Math.max(bottom, rects[i].bottom);
      }
      if (top === Infinity) continue;
      boxes.push({ id: c.id, top: top - bodyTop, bottom: bottom - bodyTop });
    }
    const { lanes, count, overflow: over } = assignLanes(boxes, MAX_LANES);
    const next: Stripe[] = boxes.map((b) => {
      const coding = codings.find((c) => c.id === b.id);
      return {
        id: b.id,
        lane: lanes.get(b.id) ?? 0,
        top: b.top,
        height: Math.max(b.bottom - b.top, 4),
        code: (coding && codes.get(coding.code_id)) || UNKNOWN,
      };
    });
    setStripes(next);
    // The margin only ever grows while a source is open. If it could also shrink, a narrower text
    // column would change where lines wrap and so the stripes, and the width would flip back and forth.
    setLaneCount((prev) => Math.max(prev, count, MIN_LANES));
    setOverflow(over.length);
  }, [codings, codes, measureLines]);

  // A different source starts with the usual margin and no remembered selection.
  useLayoutEffect(() => {
    setLaneCount(MIN_LANES);
    lastSelection.current = null;
    setBar(null);
  }, [index]);

  // ---- building the text ----
  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const hadFocus = root.contains(document.activeElement);
    const previous = lastSelection.current;
    const active = document.activeElement as HTMLElement | null;
    const map = buildSurfaceDom(root, {
      text,
      index,
      segments,
      units,
      showUnits,
      selectedCodingId: null,
      selectedRange: null,
      highlight,
    });
    mapRef.current = map;
    chosenEls.current = [];
    markChosen();
    if (previous && previous.end <= index.scalarLength) {
      writeSelection(map, previous.anchor, previous.focus);
      // Setting a selection must not pull the focus into the text if the person is elsewhere.
      if (!hadFocus && active && active !== document.activeElement && document.contains(active)) active.focus({ preventScroll: true });
    }
    measure();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [segments, text, index, showUnits, units, colorSig]);

  useLayoutEffect(() => {
    measure();
  }, [measure]);

  // The first look at the lines, and the first window of numbers to draw.
  useLayoutEffect(() => {
    measureLines();
    updateView();
  }, [measureLines, updateView]);

  useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    let frame = 0;
    const onScroll = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(updateView);
    };
    scroller.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      scroller.removeEventListener('scroll', onScroll);
    };
  }, [updateView]);

  // A change in the width of the margin moves the text sideways and can change where lines wrap,
  // so measure again once the margin has its new width.
  useLayoutEffect(() => {
    measure();
  }, [laneCount, measure]);

  // ---- showing which coding is chosen ----
  function markChosen() {
    const map = mapRef.current;
    for (const el of chosenEls.current) el.classList.remove('chosen');
    chosenEls.current = [];
    const id = selectedRef.current;
    const coding = id === null ? null : codings.find((c) => c.id === id);
    if (!map || !coding) return;
    for (let i = segmentAt(map, coding.start); i < map.els.length && map.startScalar[i] < coding.end; i++) {
      if (map.startScalar[i + 1] > coding.start) {
        map.els[i].classList.add('chosen');
        chosenEls.current.push(map.els[i]);
      }
    }
  }
  useEffect(markChosen, [selectedCodingId, codings]);

  // ---- keeping the layout right when the size changes ----
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    let frame = 0;
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        measure();
        placeRef.current();
      });
    };
    const observer = new ResizeObserver(schedule);
    observer.observe(body);
    // The text can get narrower or wider with the body staying the same size (line numbers shown or hidden, a wider margin).
    if (rootRef.current) observer.observe(rootRef.current);
    void document.fonts?.ready.then(schedule);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [measure]);

  // ---- following the selection ----
  /** Reads the live selection into `lastSelection` if it lies in the text; returns it. */
  const syncSelection = useCallback((): ScalarSelection | null => {
    const root = rootRef.current;
    const map = mapRef.current;
    if (!root || !map) return null;
    const sel = window.getSelection();
    if (!sel || !sel.anchorNode || !sel.focusNode) return null;
    const inside = (n: Node) => n === root || root.contains(n);
    if (!inside(sel.anchorNode) || !inside(sel.focusNode)) return null;
    const read = readSelection(map, root);
    if (read) lastSelection.current = read;
    return read;
  }, []);

  useEffect(() => {
    let frame = 0;
    const onChange = () => {
      const read = syncSelection();
      if (!read) return;
      // Words selected another way (the keyboard, a jump from a search result) are not the ones the bar was shown for.
      if (barSel.current && !pressing.current && !sameSelection(barSel.current, read)) setBar(null);
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => onSelectionRef.current(read));
    };
    document.addEventListener('selectionchange', onChange);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener('selectionchange', onChange);
    };
  }, [syncSelection]);

  // The bar appears when the mouse is let go with words selected, not while the selection is still being dragged.
  useEffect(() => {
    let frame = 0;
    const onUp = () => {
      if (!pressing.current) return;
      pressing.current = false;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const read = syncSelection();
        setBar(hasFloatingRef.current && read && read.start < read.end ? { start: read.start, end: read.end } : null);
      });
    };
    document.addEventListener('pointerup', onUp);
    document.addEventListener('pointercancel', onUp);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener('pointerup', onUp);
      document.removeEventListener('pointercancel', onUp);
    };
  }, [syncSelection]);

  // ---- placing the bar ----
  const placeNow = useCallback(() => {
    const body = bodyRef.current;
    const scroller = scrollRef.current;
    const map = mapRef.current;
    const el = barRef.current;
    const sel = barSel.current;
    if (!body || !scroller || !map || !el || !sel) return;
    const range = rangeFor(map, sel.start, sel.end);
    if (!range) return;
    const rects: Box[] = Array.from(range.getClientRects(), (r) => ({ top: r.top, left: r.left, right: r.right, bottom: r.bottom }));
    const edges = edgeLines(rects);
    if (!edges) return;
    const b = body.getBoundingClientRect();
    const s = scroller.getBoundingClientRect();
    const rel = (r: Box): Box => ({ top: r.top - b.top, bottom: r.bottom - b.top, left: r.left - b.left, right: r.right - b.left });
    const next = placeBar(rel(edges.first), rel(edges.last), { width: el.offsetWidth, height: el.offsetHeight }, { top: s.top - b.top, bottom: s.bottom - b.top, width: b.width });
    setPlace((prev) => (prev && prev.top === next.top && prev.left === next.left && prev.below === next.below ? prev : next));
  }, []);
  const placeRef = useRef(placeNow);
  placeRef.current = placeNow;

  // After the text is drawn again (a code was applied, the margin grew), put the bar back where the words are.
  useLayoutEffect(() => {
    if (!bar) {
      setPlace(null);
      return;
    }
    placeNow();
  }, [bar, segments, laneCount, placeNow, props.floating]);

  useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller || !bar) return;
    let frame = 0;
    const onScroll = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => placeRef.current());
    };
    scroller.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      scroller.removeEventListener('scroll', onScroll);
    };
  }, [bar]);

  // ---- blocking every change to the text ----
  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const stop = (e: Event) => {
      e.preventDefault();
    };
    const restore = () => {
      // An input method can change the text before it can be stopped; put it back.
      const keep = lastSelection.current;
      const map = buildSurfaceDom(root, { text, index, segments, units, showUnits, selectedCodingId: null, selectedRange: null, highlight });
      mapRef.current = map;
      chosenEls.current = [];
      markChosen();
      if (keep) writeSelection(map, keep.anchor, keep.focus);
    };
    const events = ['beforeinput', 'paste', 'cut', 'drop', 'dragstart'] as const;
    for (const ev of events) root.addEventListener(ev, stop);
    root.addEventListener('input', restore);
    return () => {
      for (const ev of events) root.removeEventListener(ev, stop);
      root.removeEventListener('input', restore);
    };
  }, [text, index, segments, units, showUnits, highlight]);

  // ---- what the parent can ask of the surface ----
  const scrollOffsetIntoView = useCallback((offset: number) => {
    const map = mapRef.current;
    const scroller = scrollRef.current;
    if (!map || !scroller) return;
    const p = scalarToPoint(map, offset, 'start');
    if (!p) return;
    const range = document.createRange();
    range.setStart(p.node, p.offset);
    range.collapse(true);
    let rect = range.getClientRects()[0] ?? range.getBoundingClientRect();
    if (!rect || (rect.width === 0 && rect.height === 0 && rect.top === 0)) {
      rect = map.els[segmentAt(map, offset)].getBoundingClientRect();
    }
    const box = scroller.getBoundingClientRect();
    const margin = 40;
    if (rect.top < box.top + margin) scroller.scrollTop += rect.top - box.top - margin;
    else if (rect.bottom > box.bottom - margin) scroller.scrollTop += rect.bottom - box.bottom + margin;
  }, []);

  useImperativeHandle(
    props.handleRef,
    (): SurfaceHandle => ({
      focus() {
        const root = rootRef.current;
        const map = mapRef.current;
        if (!root) return;
        // Focusing an editable element puts its caret at the very start; give the person back
        // the caret or selection they had before they went to another box.
        const keep = lastSelection.current;
        root.focus({ preventScroll: true });
        if (keep && map && keep.end <= map.index.scalarLength) writeSelection(map, keep.anchor, keep.focus);
      },
      getSelection() {
        const root = rootRef.current;
        const map = mapRef.current;
        if (root && map && root.contains(document.activeElement)) return readSelection(map, root) ?? lastSelection.current;
        return lastSelection.current;
      },
      select(start, end, opts) {
        const root = rootRef.current;
        const map = mapRef.current;
        if (!root || !map) return;
        if (opts?.focus) root.focus({ preventScroll: true });
        writeSelection(map, start, end);
        lastSelection.current = readSelection(map, root) ?? { start, end, anchor: start, focus: end };
        if (opts?.scroll !== false) scrollOffsetIntoView(end === start ? start : start);
        onSelectionRef.current(lastSelection.current);
      },
      scrollToOffset: scrollOffsetIntoView,
      element: () => rootRef.current,
      lineRange(start, end) {
        const map = mapRef.current;
        const body = bodyRef.current;
        if (!map || !body || linesRef.current.length === 0) return null;
        const range = rangeFor(map, start, end);
        if (!range) return null;
        const bodyTop = body.getBoundingClientRect().top;
        let top = Infinity;
        let bottom = -Infinity;
        for (const r of Array.from(range.getClientRects())) {
          if (r.height === 0) continue;
          top = Math.min(top, r.top - bodyTop);
          bottom = Math.max(bottom, r.bottom - bodyTop);
        }
        // A span's top is the top of its text; ask for the lines its middle reaches, so a span that ends at the start of a line is not counted on it.
        if (top === Infinity) return null;
        return lineRangeFor(linesRef.current, top + 1, bottom - 1);
      },
    }),
    [scrollOffsetIntoView],
  );

  const shownLines = useMemo(() => linesBetween(lines, view.from, view.to), [lines, view]);
  const gutterRem = laneCount * LANE_REM;
  const bodyStyle = { '--gutter': `${gutterRem}rem` } as CSSProperties;

  return (
    <div className="surface-scroll" ref={scrollRef}>
      <div className={`surface-body${props.lineNumbers ? ' numbered' : ''}`} ref={bodyRef} style={bodyStyle}>
        {props.lineNumbers ? (
          <div className="line-numbers" aria-hidden="true" data-testid="line-numbers" data-lines={lines.length}>
            {shownLines.map((l) => (
              <span key={l.n} className="line-no" data-line={l.n} style={{ top: l.top, height: l.bottom - l.top }}>
                {lineLabel(l.n)}
              </span>
            ))}
          </div>
        ) : null}
        <div className="gutter" aria-hidden="true" data-testid="gutter">
          {stripes.map((s) => (
            <div
              key={s.id}
              className={`stripe pattern-${s.code.pattern}${s.id === selectedCodingId ? ' chosen' : ''}`}
              data-coding={s.id}
              data-lane={s.lane}
              title={s.code.name}
              style={{ top: s.top, height: s.height, left: `${s.lane * LANE_REM}rem`, ['--stripe-color' as string]: s.code.color } as CSSProperties}
              onClick={() => props.onStripeClick(s.id)}
            >
              <span className="stripe-bar" />
              <span className="stripe-label">{s.code.label}</span>
            </div>
          ))}
          {overflow > 0 ? (
            <div className="stripe-more" style={{ top: 0 }}>{t('+{overflow} more', { overflow })}</div>
          ) : null}
        </div>
        <div
          ref={rootRef}
          className="surface"
          role="textbox"
          aria-multiline="true"
          aria-readonly="true"
          aria-labelledby={props.labelId}
          aria-describedby={props.descriptionId}
          contentEditable
          suppressContentEditableWarning
          spellCheck={false}
          tabIndex={0}
          translate="no"
          autoCapitalize="off"
          autoCorrect="off"
          inputMode="none"
          data-testid={props.testId ?? 'surface'}
          style={{ ['--speaker-col' as string]: speakerColumn > 0 ? `${(speakerColumn * 0.58).toFixed(2)}em` : '0px' } as CSSProperties}
          onKeyDown={(e) => {
            // The browser reports selection changes a moment late; read it now so a command
            // pressed straight after selecting acts on what the person sees.
            syncSelection();
            if (e.key === 'Escape' && barSel.current) setBar(null);
            const cmd = interpretDocumentKey(e.nativeEvent);
            if (cmd) {
              e.preventDefault();
              onCommandRef.current(cmd);
            }
          }}
          onPointerDown={() => {
            pressing.current = true;
            setBar(null);
          }}
          onBlur={() => {
            syncSelection();
            setBar(null);
          }}
          onPointerUp={syncSelection}
        />
        {bar && hasFloating ? (
          <div
            ref={barRef}
            className={`float-bar${place?.below ? ' below' : ''}`}
            style={place ? { top: place.top, left: place.left } : { top: 0, left: 0, visibility: 'hidden' }}
            // Pressing a button here must not move the focus out of the text, or the selection would be lost.
            onMouseDown={(e) => e.preventDefault()}
            data-testid="float-bar"
          >
            {props.floating}
          </div>
        ) : null}
      </div>
    </div>
  );
}
