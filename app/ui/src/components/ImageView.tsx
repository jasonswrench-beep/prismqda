import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { Button } from 'react-aria-components';
import { imageUrl, pageWidthFor, pdfPageUrl } from '../bridge';
import { abbreviations } from '../lib/labels';
import { brief, clampPage, pagePixels, pagesWithAreas, pageWords } from '../lib/pdfPages';
import {
  boundsOf,
  boxFrom,
  cornerOf,
  describeShape,
  dragCorner,
  fitShape,
  fitZoom,
  isCodable,
  moveShape,
  nudgePoint,
  outlineFromStroke,
  pointFromScreen,
  polygonPoints,
  resizeShape,
  SHAPE_LABELS,
  shownSize,
  startBox,
  stepZoom,
  translateShape,
  turnPoint,
  turnShape,
  ZOOM_MAX,
  ZOOM_MIN,
  type Corner,
  type Point,
} from '../lib/regions';
import { applyFavorite, removeCoding, repeatLastCode, reshapeArea, saveImageDescription, selectCoding, setPdfPage } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, focusApplyBox, setCenterFocus, setSelectionProvider, setSelectionReader, useApp, type SourceData } from '../state/app';
import type { AreaShape, CodingRow, PdfRow, SourceImage } from '../types';
import { FloatingCodeBar } from './FloatingCodeBar';
import { Icon } from './Icon';
import { Notice } from './common';
import { RegionThumb } from './RegionThumb';
import { SelectionLine } from './SelectionLine';
import { t, tn } from '../i18n';

type Tool = 'select' | 'rectangle' | 'ellipse' | 'polygon';

const TOOLS: { id: Tool; label: string; key: string; icon?: 'med-region-rect' | 'med-region-polygon'; help: string }[] = [
  { id: 'select', get label() { return t('Select'); }, key: 'V', get help() { return t('Choose a coded area, and move it by dragging'); } },
  { id: 'rectangle', get label() { return t('Rectangle'); }, key: 'B', icon: 'med-region-rect', get help() { return t('Drag a rectangle over part of the picture'); } },
  { id: 'ellipse', get label() { return t('Ellipse'); }, key: 'O', get help() { return t('Drag an ellipse over part of the picture'); } },
  { id: 'polygon', get label() { return t('Outline'); }, key: 'P', icon: 'med-region-polygon', get help() { return t('Click the corners of an outline, or draw it freehand'); } },
];

/** What the pointer is doing on the picture right now. */
type Drag =
  | { kind: 'draw'; shape: 'rectangle' | 'ellipse'; a: Point; b: Point }
  | { kind: 'stroke'; points: Point[]; length: number; last: [number, number] }
  | { kind: 'move'; from: Point; origin: AreaShape; ids: number[] | 'draft'; now: AreaShape; moved: boolean }
  | { kind: 'resize'; corner: Corner; origin: AreaShape; ids: number[] | 'draft'; now: AreaShape };

/** The coded areas that are exactly the same shape, drawn once with all their codes. */
interface Group {
  key: string;
  shape: AreaShape;
  codings: CodingRow[];
}

const POINTER_SLOP = 4;
const CLOSE_DISTANCE = 12;

function shapeSentence(shape: AreaShape): string {
  return `${SHAPE_LABELS[shape.shape].toLowerCase()}`;
}

/**
 * The reader for a picture, or for the pages of a PDF. The picture is drawn at a zoom that fits the window (or at the one the person
 * chose), turned as asked, with its coded areas drawn over it. An area is drawn with the pointer or with the keys, and the codes are
 * applied to it exactly as they are applied to words. Everything the pointer does has a key: a keyboard box or outline, arrows to move
 * it, Alt and the arrows to resize it, and a list of the coded areas below the picture that works as a plain list of buttons.
 *
 * With `pdf` given the surface is one page of the PDF at a time, drawn by the engine, and an area is always a rectangle on that page.
 * Every other part (zoom, keys, the floating bar, the list of areas, applying codes) is the same.
 */
export function ImageView(props: { data: SourceData; image?: SourceImage | null; pdf?: PdfRow | null }) {
  const { data } = props;
  const pdf = props.pdf ?? null;
  const pageShown = useApp((s) => s.pdfPage[data.id] ?? 1);
  const page = pdf ? clampPage(pageShown, pdf.page_count) : 1;
  // The surface drawn on: a picture at its own size, or a page at the size a PDF reader shows it at 100 percent.
  const pageSize = pdf ? (pdf.pages[page - 1] ?? pdf.pages[0]) : null;
  const surface = useMemo(() => {
    if (pdf && pageSize) return { ...pagePixels(pageSize), stamp: pdf.size_bytes };
    const image = props.image!;
    return { width: image.width, height: image.height, stamp: image.size_bytes };
  }, [pdf, pageSize, props.image]);
  const codes = useApp((s) => s.codes);
  const draft = useApp((s) => s.imageDraft);
  const selectedId = useApp((s) => s.selectedCodingId);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const floatingBar = useApp((s) => s.settings.floatingBar);
  const [tool, setTool] = useState<Tool>('select');
  const tools = pdf ? TOOLS.filter((x) => x.id === 'select' || x.id === 'rectangle') : TOOLS;
  // `fit` fits a picture whole; a PDF page is long, so for it `fit` fits the width and the page scrolls, and `page` fits the whole page.
  const [zoomMode, setZoomMode] = useState<'fit' | 'page' | number>('fit');
  const [turns, setTurns] = useState(0);
  const [frame, setFrame] = useState({ w: 0, h: 0 });
  const [drag, setDrag] = useState<Drag | null>(null);
  const [vertices, setVertices] = useState<Point[]>([]);
  const [hover, setHover] = useState<Point | null>(null);
  // The crosshair the keys move when an outline is built with the keyboard.
  const [crosshair, setCrosshair] = useState<Point | null>(null);
  const [failed, setFailed] = useState(false);
  const [editing, setEditing] = useState(false);
  const [words, setWords] = useState(data.text);
  // Shapes changed with the keys show at once; the engine is told when the keys stop, so one run of presses is one change.
  const [live, setLive] = useState<Record<number, AreaShape>>({});
  const stage = useRef<HTMLDivElement>(null);
  const canvasEl = useRef<HTMLDivElement>(null);
  const barEl = useRef<HTMLDivElement>(null);
  const [barLeft, setBarLeft] = useState<number | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  const commit = useRef<number | null>(null);
  const liveRef = useRef(live);
  liveRef.current = live;

  useEffect(() => setWords(data.text), [data.text]);

  const labels = useMemo(() => abbreviations(codes), [codes]);
  const codeOf = useCallback((id: number) => codes.find((c) => c.id === id), [codes]);

  // ---- where the picture sits
  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const measure = () => setFrame({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const watch = new ResizeObserver(measure);
    watch.observe(el);
    return () => watch.disconnect();
  }, []);
  const fit = fitZoom({ frameW: frame.w - 2, frameH: frame.h - 2, imageW: surface.width, imageH: surface.height, turns });
  const fitWidth = pdf && frame.w > 0 ? Math.min(1.5, Math.max(ZOOM_MIN, (frame.w - 4) / surface.width)) : fit;
  const zoom = zoomMode === 'page' ? fit : zoomMode === 'fit' ? fitWidth : zoomMode;
  const shown = shownSize({ imageW: surface.width, imageH: surface.height, turns, zoom });

  const setZoom = useCallback(
    (z: 'fit' | 'page' | number) => {
      setZoomMode(z);
      const percent = Math.round((z === 'page' ? fit : z === 'fit' ? fitWidth : z) * 100);
      announce(
        z === 'page' ? t('The whole page fits the window, {percent}%.', { percent }) : z === 'fit' ? (pdf ? t('Fitted to the width of the window, {percent}%.', { percent }) : t('Fitted to the window, {percent}%.', { percent })) : t('Zoom {percent}%.', { percent }),
        'polite',
        { background: true },
      );
    },
    [fit, fitWidth, pdf],
  );
  const zoomBy = useCallback(
    (direction: 1 | -1) => {
      const next = stepZoom(zoom, direction);
      setZoomMode(Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, next)));
      announce(t('Zoom {next}%.', { next: Math.round(next * 100) }), 'polite', { background: true });
    },
    [zoom],
  );
  const turn = useCallback((direction: 1 | -1) => {
    // A page is read upright; only a picture is turned.
    if (pdf) return;
    setTurns((t) => (t + direction + 4) % 4);
    announce(direction === 1 ? t('Turned the picture a quarter turn clockwise. The coded areas turn with it.') : t('Turned the picture a quarter turn the other way.'), 'polite', { background: true });
  }, [pdf]);

  // Holding Ctrl and turning the wheel zooms. A listener of its own, because the wheel must be stopped from zooming the whole window.
  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      zoomBy(e.deltaY < 0 ? 1 : -1);
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, [zoomBy]);

  // ---- the coded areas
  const groups = useMemo<Group[]>(() => {
    const by = new Map<string, Group>();
    for (const c of data.codings) {
      if (!c.region || c.region.shape === 'span') continue;
      // On a PDF an area belongs to one page; the pages are shown one at a time.
      if (pdf && (c.page ?? 1) !== page) continue;
      const shape = live[c.id] ?? c.region;
      const key = JSON.stringify(fitShape(c.region));
      const g = by.get(key);
      if (g) g.codings.push(c);
      else by.set(key, { key, shape, codings: [c] });
    }
    const list = [...by.values()].map((g) => ({ ...g, shape: live[g.codings[0].id] ?? g.shape }));
    // The largest is drawn first and the smallest last, so a small area inside a large one is on top and can be reached.
    const area = (s: AreaShape) => {
      const [x0, y0, x1, y1] = boundsOf(s);
      return (x1 - x0) * (y1 - y0);
    };
    return list.sort((a, b) => area(b.shape) - area(a.shape));
  }, [data.codings, live, pdf, page]);
  const ordered = useMemo(() => [...groups].sort((a, b) => boundsOf(a.shape)[1] - boundsOf(b.shape)[1] || boundsOf(a.shape)[0] - boundsOf(b.shape)[0]), [groups]);
  const selectedGroup = selectedId !== null && selectedId < 0 ? (groups.find((g) => g.codings.some((c) => c.id === selectedId)) ?? null) : null;
  // The shape the keys and the code buttons work on: the one just drawn, else the one chosen.
  const active: AreaShape | null = draft ?? selectedGroup?.shape ?? null;

  // What Segment Details and the code box say is selected.
  useEffect(() => {
    if (!active) {
      appStore.set({ selectionText: '' });
      return;
    }
    const words = describeShape(active, pdf ? 'page' : 'picture');
    appStore.set({ selectionText: draft ? t('Selected: {shape} (not coded yet)', { shape: words }) : t('Selected: {shape}', { shape: words }) });
  }, [active, draft, pdf]);

  const hint = pdf
    ? t('PDF pages are coded by area. Draw a rectangle over a figure, a table, or any part of the page, then choose a code.')
    : t('Pictures are coded by area. Draw a rectangle, ellipse, or outline over part of the picture, then choose a code.');
  useEffect(() => {
    setSelectionProvider(() => ({ ok: false, message: hint }));
    setSelectionReader(() => null);
    setCenterFocus(() => stage.current?.focus());
    return () => {
      setSelectionProvider(null);
      setSelectionReader(null);
      setCenterFocus(null);
    };
  }, [hint]);

  // A coded area chosen from a list that is on another page of the PDF takes the view to that page.
  useEffect(() => {
    if (!pdf || selectedId === null || selectedId >= 0) return;
    const chosen = data.codings.find((c) => c.id === selectedId);
    const there = chosen?.page ?? 1;
    if (chosen?.region && there !== page) setPdfPage(data.id, there);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  // ---- the pages of a PDF
  const [typedPage, setTypedPage] = useState<string | null>(null);
  const goToPage = useCallback(
    (to: number, say = true) => {
      if (!pdf) return;
      const next = clampPage(to, pdf.page_count);
      if (next === page) {
        setTypedPage(null);
        return;
      }
      void flush();
      appStore.set({ imageDraft: null });
      setPdfPage(data.id, next);
      setTypedPage(null);
      if (say) announce(t('Page {page} of {count}.', { page: next, count: pdf.page_count }), 'polite', { background: true });
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [pdf, page, data.id],
  );
  const wordsOnPage = useMemo(() => (pdf ? pageWords(data.runs, data.text, data.index, page) : ''), [pdf, data.runs, data.text, data.index, page]);
  const pagesWith = useMemo(() => (pdf ? pagesWithAreas(data.codings) : []), [pdf, data.codings]);

  // ---- positions
  const toShown = useCallback((p: Point): [number, number] => {
    const [u, v] = turnPoint(p, turns);
    return [u * shown.w, v * shown.h];
  }, [turns, shown.w, shown.h]);

  const pointOf = useCallback(
    (e: { clientX: number; clientY: number }): Point => {
      const box = svg.current?.getBoundingClientRect();
      if (!box) return [0, 0];
      return pointFromScreen(e.clientX - box.left, e.clientY - box.top, shown, turns);
    },
    [shown, turns],
  );
  /** The pointer as a fraction of the picture as shown (before the turn is undone). */
  const shownFraction = useCallback(
    (e: { clientX: number; clientY: number }): Point => {
      const box = svg.current?.getBoundingClientRect();
      if (!box || shown.w <= 0 || shown.h <= 0) return [0, 0];
      return [Math.min(1, Math.max(0, (e.clientX - box.left) / shown.w)), Math.min(1, Math.max(0, (e.clientY - box.top) / shown.h))];
    },
    [shown],
  );

  // ---- changing a shape
  const flush = useCallback(async () => {
    if (commit.current !== null) {
      window.clearTimeout(commit.current);
      commit.current = null;
    }
    const pending = liveRef.current;
    const ids = Object.keys(pending).map(Number);
    if (ids.length === 0) return;
    for (const id of ids) await reshapeArea(id, pending[id]);
    setLive({});
  }, []);

  // A different area chosen, or leaving the picture, finishes the change in hand.
  useEffect(() => {
    return () => {
      void flush();
    };
  }, [flush, data.id]);

  const changeActive = useCallback(
    (next: AreaShape) => {
      const st = appStore.get();
      if (st.imageDraft) {
        appStore.set({ imageDraft: next });
        return;
      }
      if (!selectedGroup) return;
      const ids = selectedGroup.codings.map((c) => c.id);
      setLive((l) => ({ ...l, ...Object.fromEntries(ids.map((id) => [id, next])) }));
      if (commit.current !== null) window.clearTimeout(commit.current);
      commit.current = window.setTimeout(() => void flush(), 500);
    },
    [selectedGroup, flush],
  );

  const setDraft = useCallback((shape: AreaShape | null) => {
    appStore.set({ imageDraft: shape, ...(shape ? { selectedCodingId: null } : {}) });
  }, []);

  const choose = useCallback(
    (id: number | null) => {
      void flush();
      appStore.set({ imageDraft: null });
      selectCoding(id);
    },
    [flush],
  );

  const describeGroup = useCallback(
    (g: Group, index: number, total: number): string => {
      const names = g.codings.map((c) => codeOf(c.code_id)?.name ?? t('a code'));
      return t('Area {index} of {total}: {shape}. Coded {list}.', { index: index + 1, total, shape: describeShape(g.shape, pdf ? 'page' : 'picture'), list: names.join(', ') });
    },
    [codeOf, pdf],
  );

  const finishOutline = useCallback(() => {
    // A corner placed twice in the same place (a double click) counts once.
    const pts = vertices.filter((p, i) => i === 0 || Math.hypot(p[0] - vertices[i - 1][0], p[1] - vertices[i - 1][1]) > 0.002);
    const shape = fitShape({ shape: 'polygon', points: pts });
    if (pts.length < 3 || !isCodable(shape)) {
      announce(t('An outline needs at least three corners that enclose some of the picture. Add more corners, or press Escape to start again.'), 'assertive');
      return;
    }
    setVertices([]);
    setCrosshair(null);
    setDraft(shape);
    announce(t('Drew an outline of {count} corners. Choose a code to apply to it.', { count: pts.length }));
  }, [vertices, setDraft]);

  // ---- the pointer
  const onPointerDown = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (e.button !== 0 || readOnly) return;
    stage.current?.focus({ preventScroll: true });
    const p = pointOf(e);
    if (tool === 'rectangle' || tool === 'ellipse') {
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      setDrag({ kind: 'draw', shape: tool, a: p, b: p });
    } else if (tool === 'polygon') {
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      setCrosshair(null);
      setDrag({ kind: 'stroke', points: [p], length: 0, last: [e.clientX, e.clientY] });
    } else {
      // Pressing the empty picture lets go of whatever was chosen.
      choose(null);
    }
  };

  const beginMove = (e: ReactPointerEvent<SVGElement>, ids: number[] | 'draft', shape: AreaShape, selectId: number | null) => {
    if (tool !== 'select' || e.button !== 0) return;
    e.stopPropagation();
    stage.current?.focus({ preventScroll: true });
    if (selectId !== null) {
      void flush();
      appStore.set({ imageDraft: null });
      selectCoding(selectId);
    }
    if (readOnly) return;
    (svg.current as Element).setPointerCapture(e.pointerId);
    setDrag({ kind: 'move', from: shownFraction(e), origin: shape, ids, now: shape, moved: false });
  };

  const beginResize = (e: ReactPointerEvent<SVGElement>, corner: Corner, ids: number[] | 'draft', shape: AreaShape) => {
    if (e.button !== 0 || readOnly) return;
    e.stopPropagation();
    (svg.current as Element).setPointerCapture(e.pointerId);
    setDrag({ kind: 'resize', corner, origin: shape, ids, now: shape });
  };

  const onPointerMove = (e: ReactPointerEvent<SVGSVGElement>) => {
    const p = pointOf(e);
    if (tool === 'polygon' && vertices.length > 0) setHover(p);
    if (!drag) return;
    if (drag.kind === 'draw') setDrag({ ...drag, b: p });
    else if (drag.kind === 'stroke') {
      const moved = Math.hypot(e.clientX - drag.last[0], e.clientY - drag.last[1]);
      if (moved < 2) return;
      setDrag({ ...drag, points: [...drag.points, p], length: drag.length + moved, last: [e.clientX, e.clientY] });
    } else if (drag.kind === 'move') {
      const f = shownFraction(e);
      const shownShape = turnShape(drag.origin, turns);
      const moved = translateShape(shownShape, f[0] - drag.from[0], f[1] - drag.from[1]);
      const back = turnShape(moved, 4 - turns);
      setDrag({ ...drag, now: back, moved: drag.moved || Math.hypot(f[0] - drag.from[0], f[1] - drag.from[1]) * Math.max(shown.w, shown.h) > POINTER_SLOP });
    } else if (drag.kind === 'resize') {
      const f = shownFraction(e);
      const next = dragCorner(turnShape(drag.origin, turns), drag.corner, f);
      setDrag({ ...drag, now: turnShape(next, 4 - turns) });
    }
  };

  const onPointerUp = (e: ReactPointerEvent<SVGSVGElement>) => {
    const d = drag;
    setDrag(null);
    if (!d) return;
    if (d.kind === 'draw') {
      const shape = fitShape(boxFrom(d.shape, d.a, pointOf(e)));
      if (isCodable(shape) && Math.hypot((shape.shape !== 'polygon' ? shape.w : 0) * shown.w, (shape.shape !== 'polygon' ? shape.h : 0) * shown.h) > POINTER_SLOP * 2) {
        setDraft(shape);
        announce(t('Drew a {shape}: {shape2}. Choose a code to apply to it.', { shape: shapeSentence(shape), shape2: describeShape(shape) }));
      } else {
        announce(pdf ? t('That is too small to code. Drag over a larger part of the page.') : t('That is too small to code. Drag over a larger part of the picture.'), 'assertive');
      }
    } else if (d.kind === 'stroke') {
      if (d.length > 10) {
        const shape = outlineFromStroke([...d.points, pointOf(e)]);
        if (shape) {
          setVertices([]);
          setDraft(shape);
          announce(t('Drew a freehand outline: {shape}. Choose a code to apply to it.', { shape: describeShape(shape) }));
        } else announce(t('That line does not enclose any of the picture. Draw all the way round the part you mean.'), 'assertive');
      } else {
        const p = pointOf(e);
        // A click near the first corner closes the outline.
        if (vertices.length >= 3) {
          const [fx, fy] = toShown(vertices[0]);
          const [cx, cy] = toShown(p);
          if (Math.hypot(fx - cx, fy - cy) <= CLOSE_DISTANCE) {
            finishOutline();
            return;
          }
        }
        setVertices((v) => [...v, p]);
        announce(t('Corner {count} placed.', { count: vertices.length + 1 }), 'polite', { background: true });
      }
    } else if (d.kind === 'move' || d.kind === 'resize') {
      const changed = JSON.stringify(fitShape(d.now)) !== JSON.stringify(fitShape(d.origin));
      if (!changed) return;
      const next = fitShape(d.now);
      if (!isCodable(next)) {
        announce(t('That would make the area too small to code.'), 'assertive');
        return;
      }
      const ids = d.ids;
      if (ids === 'draft') appStore.set({ imageDraft: next });
      else {
        setLive((l) => ({ ...l, ...Object.fromEntries(ids.map((id) => [id, next])) }));
        void (async () => {
          for (const id of ids) await reshapeArea(id, next);
          setLive({});
        })();
      }
    }
  };

  const onDoubleClick = () => {
    if (tool === 'polygon' && vertices.length >= 3) finishOutline();
  };

  // ---- the keys
  const nothingToChange = () => announce(t('No area is chosen. Draw one first, or choose a coded area from the list below the picture.'), 'assertive');

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.ctrlKey || e.metaKey) return;
    const key = e.key;
    const big = e.shiftKey;
    const arrow = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[key as 'ArrowLeft'] as [number, number] | undefined;
    const handled = () => {
      e.preventDefault();
      e.stopPropagation();
    };

    if (arrow) {
      handled();
      if (readOnly && active) return;
      // Building an outline with the keys: the arrows move the crosshair.
      if (tool === 'polygon' && !active) {
        const from = crosshair ?? [0.5, 0.5];
        const next = nudgePoint(from, arrow[0], arrow[1], big, turns);
        setCrosshair(next);
        setHover(next);
        return;
      }
      if (!active) {
        // Nothing to move: the arrows scroll the picture, as they would in any window.
        stage.current?.scrollBy({ left: arrow[0] * 60, top: arrow[1] * 60 });
        return;
      }
      changeActive(e.altKey ? resizeShape(active, arrow[0], arrow[1], big, turns) : moveShape(active, arrow[0], arrow[1], big, turns));
      return;
    }

    // On a PDF Page Up and Page Down turn the page; the square brackets move through the areas.
    if (pdf && (key === 'PageUp' || key === 'PageDown')) {
      handled();
      goToPage(page + (key === 'PageDown' ? 1 : -1));
      return;
    }

    switch (key) {
      case 'v':
      case 'V':
        handled();
        setTool('select');
        announce(t('Select tool. Press the left and right square bracket keys to move through the coded areas.'));
        return;
      case 'b':
      case 'B':
        handled();
        setTool('rectangle');
        announce(t('Rectangle tool. Press Space for a rectangle in the middle, then the arrow keys to move it, or drag with the pointer.'));
        return;
      case 'o':
      case 'O':
        handled();
        if (pdf) {
          announce(t('An area of a PDF page is a rectangle. Press B for the rectangle tool.'), 'assertive');
          return;
        }
        setTool('ellipse');
        announce(t('Ellipse tool. Press Space for an ellipse in the middle, then the arrow keys to move it, or drag with the pointer.'));
        return;
      case 'p':
      case 'P':
        handled();
        if (pdf) {
          announce(t('An area of a PDF page is a rectangle. Press B for the rectangle tool.'), 'assertive');
          return;
        }
        setTool('polygon');
        setCrosshair([0.5, 0.5]);
        announce(t('Outline tool. The arrow keys move a crosshair, Space puts down a corner, and Enter finishes the outline. You can also click the corners, or draw freehand.'));
        return;
      case ' ':
        handled();
        if (readOnly) return;
        if (tool === 'rectangle' || tool === 'ellipse') {
          const shape = startBox(tool, crosshair ?? [0.5, 0.5]);
          setDraft(shape);
          announce(
            pdf
              ? t('A rectangle in the middle of the page. The arrow keys move it, Alt and the arrow keys change its size, and Shift makes the steps larger. Then choose a code.')
              : t('A {shape} in the middle of the picture. The arrow keys move it, Alt and the arrow keys change its size, and Shift makes the steps larger. Then choose a code.', { shape: shapeSentence(shape) }),
          );
        } else if (tool === 'polygon') {
          const p = crosshair ?? [0.5, 0.5];
          setCrosshair(p);
          setVertices((v) => [...v, p]);
          announce(t('Corner {count} placed at {value}% across and {value2}% down.', { count: vertices.length + 1, value: Math.round(p[0] * 100), value2: Math.round(p[1] * 100) }));
        } else announce(t('Choose the Rectangle, Ellipse, or Outline tool to draw an area, or press the left and right square bracket keys to move through the coded areas.'));
        return;
      case 'Enter':
        handled();
        if (tool === 'polygon' && vertices.length > 0) finishOutline();
        else if (!active) nothingToChange();
        else focusApplyBox();
        return;
      case 'Escape':
        if (vertices.length > 0) {
          handled();
          setVertices([]);
          setCrosshair(tool === 'polygon' ? [0.5, 0.5] : null);
          announce(t('Cancelled the outline.'));
        } else if (draft) {
          handled();
          setDraft(null);
          announce(t('Discarded the area that was not coded.'));
        } else if (selectedGroup) {
          handled();
          choose(null);
          announce(t('Nothing is chosen now.'));
        }
        return;
      case 'Delete':
      case 'Backspace':
        handled();
        if (readOnly) return;
        if (draft) {
          setDraft(null);
          announce(t('Discarded the area that was not coded.'));
        } else if (selectedId !== null && selectedId < 0) void removeCoding(selectedId);
        else nothingToChange();
        return;
      case '[':
      case ']':
      case 'PageUp':
      case 'PageDown': {
        handled();
        if (ordered.length === 0) {
          announce(pdf ? t('No area of this page is coded yet.') : t('No area of this picture is coded yet.'), 'assertive');
          return;
        }
        const forward = key === ']' || key === 'PageDown';
        const here = selectedGroup ? ordered.findIndex((g) => g.key === selectedGroup.key) : -1;
        const index = here < 0 ? (forward ? 0 : ordered.length - 1) : (here + (forward ? 1 : -1) + ordered.length) % ordered.length;
        const g = ordered[index];
        choose(g.codings[0].id);
        announce(describeGroup(g, index, ordered.length));
        return;
      }
      case 'Home':
      case 'End':
        if (pdf) {
          handled();
          goToPage(key === 'Home' ? 1 : pdf.page_count);
          return;
        }
        return;
      case '+':
      case '=':
        handled();
        zoomBy(1);
        return;
      case '-':
      case '_':
        handled();
        zoomBy(-1);
        return;
      case '0':
        handled();
        setZoom('fit');
        return;
      case 't':
      case 'T':
        handled();
        turn(e.shiftKey ? -1 : 1);
        return;
      case 'r':
      case 'R':
        handled();
        if (!active) nothingToChange();
        else void repeatLastCode();
        return;
      case 'i':
      case 'I':
        handled();
        announce(t('In vivo coding makes a code from selected words, so it is not available for a picture. Press Enter to choose a code.'), 'assertive');
        return;
      default:
        if (/^[1-9]$/.test(key)) {
          handled();
          if (!active) nothingToChange();
          else void applyFavorite(Number(key));
        }
    }
  };

  // ---- drawing
  const toScreenShape = (s: AreaShape) => turnShape(s, turns);

  const shapeElement = (s: AreaShape, props: Record<string, unknown>) => {
    const t = toScreenShape(s);
    if (t.shape === 'rectangle') return <rect x={t.x * shown.w} y={t.y * shown.h} width={t.w * shown.w} height={t.h * shown.h} {...props} />;
    if (t.shape === 'ellipse') return <ellipse cx={(t.x + t.w / 2) * shown.w} cy={(t.y + t.h / 2) * shown.h} rx={(t.w / 2) * shown.w} ry={(t.h / 2) * shown.h} {...props} />;
    return <polygon points={polygonPoints(t.points, shown.w, shown.h)} {...props} />;
  };

  const labelAt = (s: AreaShape): [number, number] => {
    const [x0, y0] = boundsOf(toScreenShape(s));
    return [x0 * shown.w + 4, y0 * shown.h + 14];
  };

  // What the pointer is drawing, as a shape.
  const drawing: AreaShape | null = useMemo(() => {
    if (!drag) return null;
    if (drag.kind === 'draw') return boxFrom(drag.shape, drag.a, drag.b);
    if (drag.kind === 'stroke' && drag.length > 10) return { shape: 'polygon', points: drag.points };
    if (drag.kind === 'move' || drag.kind === 'resize') return drag.now;
    return null;
  }, [drag]);

  const activeToShow: AreaShape | null = drag && (drag.kind === 'move' || drag.kind === 'resize') ? drag.now : active;
  const handles = activeToShow && activeToShow.shape !== 'polygon' && !readOnly ? (['nw', 'ne', 'se', 'sw'] as Corner[]) : [];
  const activeIds: number[] | 'draft' | null = draft ? 'draft' : selectedGroup ? selectedGroup.codings.map((c) => c.id) : null;

  // The bar of codes floats under the area being worked on.
  const bar = useMemo(() => {
    if (!active || !floatingBar || readOnly || drag) return null;
    const [x0, , , y1] = boundsOf(toScreenShape(active));
    const left = Math.max(4, Math.min(x0 * shown.w, Math.max(4, shown.w - 280)));
    const below = y1 * shown.h + 8;
    const top = below + 44 > shown.h && shown.h > 120 ? Math.max(4, boundsOf(toScreenShape(active))[1] * shown.h - 48) : below;
    return { left, top };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, floatingBar, readOnly, drag, shown.w, shown.h, turns]);

  // The bar is kept inside the part of the window the picture is shown in, whatever the picture's own width.
  useLayoutEffect(() => {
    const b = barEl.current;
    const s = stage.current;
    const c = canvasEl.current;
    if (!bar || !b || !s || !c) {
      setBarLeft(null);
      return;
    }
    const cBox = c.getBoundingClientRect();
    const sBox = s.getBoundingClientRect();
    const wanted = cBox.left + bar.left;
    const clamped = Math.max(sBox.left + 4, Math.min(wanted, sBox.right - b.offsetWidth - 4));
    setBarLeft(Math.round(clamped - cBox.left));
  }, [bar, frame.w, shown.w]);

  const polyline = vertices.length > 0 ? [...vertices, ...(hover && !drag ? [hover] : [])] : [];
  const stamp = surface.stamp;
  const zoomPercent = Math.round(zoom * 100);

  return (
    <div className="img-view" data-testid="image-view">
      <div className="img-toolbar" role="toolbar" aria-label={pdf ? t('PDF page tools') : t('Picture tools')} data-testid="image-toolbar">
        {pdf ? (
          <div className="img-group" role="group" aria-label={t('Page')} data-testid="pdf-pages">
            <Button className="icon-button flip" aria-label={t('Previous page')} aria-keyshortcuts="PageUp" isDisabled={page <= 1} onPress={() => goToPage(page - 1)} data-testid="pdf-prev">
              <Icon name="ui-chevron-right" size={18} />
            </Button>
            <label className="img-page-field">
              <span>{t('Page')}</span>
              <input
                type="text"
                inputMode="numeric"
                className="img-page-input"
                value={typedPage ?? String(page)}
                size={4}
                aria-label={t('Page number, from 1 to {count}', { count: pdf.page_count })}
                data-testid="pdf-page-input"
                onFocus={(e) => e.currentTarget.select()}
                onChange={(e) => setTypedPage(e.target.value)}
                onBlur={() => {
                  const n = Number(typedPage);
                  if (typedPage !== null && typedPage.trim() !== '' && Number.isFinite(n)) goToPage(n);
                  else setTypedPage(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    const n = Number(typedPage);
                    if (typedPage !== null && typedPage.trim() !== '' && Number.isFinite(n)) goToPage(n);
                    else setTypedPage(null);
                    window.setTimeout(() => stage.current?.focus(), 0);
                  } else if (e.key === 'Escape') {
                    setTypedPage(null);
                  }
                  e.stopPropagation();
                }}
              />
              <span data-testid="pdf-page-of">{t('of {count}', { count: pdf.page_count })}</span>
            </label>
            <Button className="icon-button" aria-label={t('Next page')} aria-keyshortcuts="PageDown" isDisabled={page >= pdf.page_count} onPress={() => goToPage(page + 1)} data-testid="pdf-next">
              <Icon name="ui-chevron-right" size={18} />
            </Button>
          </div>
        ) : null}
        <div className="img-group" role="group" aria-label={t('Tool')}>
          {tools.map((item) => (
            <Button
              key={item.id}
              className={`img-tool${tool === item.id ? ' pressed' : ''}`}
              aria-pressed={tool === item.id}
              aria-keyshortcuts={item.key}
              data-key={item.key}
              isDisabled={readOnly && item.id !== 'select'}
              data-testid={`image-tool-${item.id}`}
              onPress={() => {
                setTool(item.id);
                // The crosshair is for the keys; a person using the pointer does not need one.
                setCrosshair(null);
                if (item.id !== 'polygon') setVertices([]);
                announce(t('{label} tool. {help}.', { label: item.label, help: item.help }), 'polite', { background: true });
                window.setTimeout(() => stage.current?.focus(), 0);
              }}
            >
              {item.icon ? <Icon name={item.icon} size={18} /> : <span className="img-glyph" aria-hidden="true">{item.id === 'select' ? '↖' : '◯'}</span>}
              <span>{item.label}</span>
              <kbd aria-hidden="true">{item.key}</kbd>
            </Button>
          ))}
        </div>
        <div className="img-group" role="group" aria-label={t('View')}>
          <Button className="icon-button" aria-label={t('Zoom out')} aria-keyshortcuts="-" onPress={() => zoomBy(-1)} data-testid="image-zoom-out">
            <Icon name="ui-zoom-out" size={20} />
          </Button>
          <span className="img-zoom" data-testid="image-zoom" role="status" aria-label={t('Zoom {zoomPercent} percent', { zoomPercent })}>
            {zoomPercent}%
          </span>
          <Button className="icon-button" aria-label={t('Zoom in')} aria-keyshortcuts="+" onPress={() => zoomBy(1)} data-testid="image-zoom-in">
            <Icon name="ui-zoom-in" size={20} />
          </Button>
          <Button className="img-tool" onPress={() => setZoom('fit')} aria-keyshortcuts="0" data-testid="image-fit">
            <Icon name="ui-fit" size={18} />
            <span>{pdf ? t('Fit width') : t('Fit')}</span>
          </Button>
          {pdf ? (
            <Button className="img-tool" onPress={() => setZoom('page')} data-testid="image-whole-page">
              <span>{t('Whole page')}</span>
            </Button>
          ) : null}
          <Button className="img-tool" onPress={() => setZoom(1)} data-testid="image-actual">
            <span>100%</span>
          </Button>
          {pdf ? null : (
            <Button className="img-tool" aria-keyshortcuts="T" onPress={() => turn(1)} data-testid="image-turn">
              <Icon name="ui-refresh" size={18} />
              <span>{t('Turn')}</span>
            </Button>
          )}
        </div>
        {vertices.length > 0 ? (
          <div className="img-group" role="group" aria-label={t('Outline in progress')}>
            <span className="muted" role="status">
              {tn(vertices.length, { one: '{count} corner', other: '{count} corners' })}
            </span>
            <Button className="img-tool primary" isDisabled={vertices.length < 3} onPress={finishOutline} data-testid="image-finish">{t('Finish outline')}</Button>
            <Button className="img-tool" onPress={() => { setVertices([]); announce(t('Cancelled the outline.')); }}>{t('Cancel')}</Button>
          </div>
        ) : null}
      </div>

      <p id="img-keys" className="visually-hidden">
        {pdf
          ? t('The page of a PDF. Page Up and Page Down turn the page, and Home and End go to the first and last page. B chooses the rectangle tool. Space starts a rectangle. The arrow keys move the chosen area, Alt with the arrow keys changes its size, and Shift makes the steps larger. Enter chooses a code. The left and right square bracket keys move through the coded areas of this page, and Delete removes the chosen coding. Plus, minus, and 0 zoom in, zoom out, and fit the page.')
          : t('The picture. B, O, and P choose the rectangle, ellipse, and outline tools. Space starts a shape or puts down an outline corner. The arrow keys move the chosen area, Alt with the arrow keys changes its size, and Shift makes the steps larger. Enter chooses a code. The left and right square bracket keys move through the coded areas, and Delete removes the chosen coding. Plus, minus, 0, and T zoom, fit, and turn the picture.')}
      </p>
      <div
        ref={stage}
        className="img-stage"
        tabIndex={0}
        role="application"
        aria-label={pdf ? t('Page {page} of {count} of {name}. {text}', { page, count: pdf.page_count, name: data.name, text: wordsOnPage === '' ? t('No words were read from this page.') : brief(wordsOnPage, 400) }) : t('Picture {name}. {text}', { name: data.name, text: data.text })}
        aria-describedby="img-keys"
        data-testid="image-stage"
        onKeyDown={onKeyDown}
        onBlur={() => void flush()}
      >
        {failed ? (
          <Notice kind="warning">{pdf ? t('The window could not draw this page. The PDF is kept in the project, and the coded areas are listed below.') : t('The window could not show this picture. Its file is kept in the project, and the coded areas are listed below.')}</Notice>
        ) : (
          <div ref={canvasEl} className="img-canvas" style={{ width: shown.w, height: shown.h }} data-testid="image-canvas" data-turns={turns} data-zoom={zoom.toFixed(3)}>
            <img
              className="img-pic"
              key={pdf ? `${data.id}-${page}` : data.id}
              src={pdf ? pdfPageUrl(data.id, page, pageWidthFor(shown.w, window.devicePixelRatio || 1), stamp) : imageUrl(data.id, stamp)}
              alt=""
              draggable={false}
              onError={() => setFailed(true)}
              onLoad={() => setFailed(false)}
              data-testid="image-picture"
              data-page={pdf ? page : undefined}
              style={{
                width: surface.width * zoom,
                height: surface.height * zoom,
                left: (shown.w - surface.width * zoom) / 2,
                top: (shown.h - surface.height * zoom) / 2,
                transform: `rotate(${turns * 90}deg)`,
              }}
            />
            <svg
              ref={svg}
              className={`img-overlay tool-${tool}`}
              width={shown.w}
              height={shown.h}
              aria-hidden="true"
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={() => setDrag(null)}
              onDoubleClick={onDoubleClick}
            >
              {groups.map((g) => {
                const first = g.codings[0];
                const color = codeOf(first.code_id)?.color ?? '#808080';
                const chosen = selectedGroup?.key === g.key;
                const shape = drag && (drag.kind === 'move' || drag.kind === 'resize') && drag.ids !== 'draft' && drag.ids.includes(first.id) ? drag.now : g.shape;
                const [lx, ly] = labelAt(shape);
                const text = g.codings.map((c) => labels.get(c.code_id) ?? '?').join(' ');
                return (
                  <g key={g.key} className={`img-area${chosen ? ' chosen' : ''}`} data-testid={`region-${first.id}`} data-codes={g.codings.map((c) => c.code_id).join(',')} style={{ ['--area-color' as string]: color }}>
                    {shapeElement(shape, { className: 'img-halo' })}
                    {shapeElement(shape, {
                      className: 'img-shape',
                      onPointerDown: (e: ReactPointerEvent<SVGElement>) => beginMove(e, g.codings.map((c) => c.id), g.shape, first.id),
                    })}
                    <text x={lx} y={ly} className="img-tag">
                      {text}
                    </text>
                  </g>
                );
              })}
              {draft && !(drag && (drag.kind === 'move' || drag.kind === 'resize')) ? (
                <g className="img-draft" data-testid="region-draft">
                  {shapeElement(draft, { className: 'img-halo' })}
                  {shapeElement(draft, { className: 'img-shape', onPointerDown: (e: ReactPointerEvent<SVGElement>) => beginMove(e, 'draft', draft, null) })}
                  <text x={labelAt(draft)[0]} y={labelAt(draft)[1]} className="img-tag">{t('new')}</text>
                </g>
              ) : null}
              {drawing ? (
                <g className="img-draft" data-testid="region-drawing">
                  {shapeElement(drawing, { className: 'img-halo' })}
                  {shapeElement(drawing, { className: 'img-shape' })}
                </g>
              ) : null}
              {polyline.length > 0 ? (
                <g className="img-draft" data-testid="outline-progress">
                  <polyline points={polygonPoints(polyline.map((p) => turnPoint(p, turns)), shown.w, shown.h)} className="img-halo" fill="none" />
                  <polyline points={polygonPoints(polyline.map((p) => turnPoint(p, turns)), shown.w, shown.h)} className="img-shape" fill="none" />
                  {vertices.map((p, i) => {
                    const [cx, cy] = toShown(p);
                    return <circle key={i} cx={cx} cy={cy} r={i === 0 ? 6 : 4} className="img-corner" />;
                  })}
                </g>
              ) : null}
              {tool === 'polygon' && crosshair && !drag ? (() => {
                const [cx, cy] = toShown(crosshair);
                return (
                  <g className="img-cross" data-testid="crosshair" pointerEvents="none">
                    <line x1={cx - 10} y1={cy} x2={cx + 10} y2={cy} />
                    <line x1={cx} y1={cy - 10} x2={cx} y2={cy + 10} />
                  </g>
                );
              })() : null}
              {activeToShow
                ? handles.map((c) => {
                    // The corner of the box as it appears is where the handle goes.
                    const [sx, sy] = cornerOf(turnShape(activeToShow, turns), c).map((v, i) => v * (i === 0 ? shown.w : shown.h));
                    return (
                      <rect
                        key={c}
                        x={sx - 5}
                        y={sy - 5}
                        width={10}
                        height={10}
                        className={`img-handle corner-${c}`}
                        data-testid={`handle-${c}`}
                        onPointerDown={(e) => activeIds && beginResize(e, c, activeIds, drag && (drag.kind === 'move' || drag.kind === 'resize') ? drag.origin : activeToShow)}
                      />
                    );
                  })
                : null}
            </svg>
            {bar ? (
              <div ref={barEl} className="img-float" style={{ left: barLeft ?? bar.left, top: bar.top }} data-testid="image-float">
                <FloatingCodeBar />
              </div>
            ) : null}
          </div>
        )}
      </div>

      <SelectionLine />
      <section className="img-below" aria-labelledby="img-areas-h">
        <h3 id="img-areas-h" className="bk-h3">{pdf ? t('Coded areas of this page ({count})', { count: groups.reduce((n, g) => n + g.codings.length, 0) }) : t('Coded areas ({count})', { count: groups.reduce((n, g) => n + g.codings.length, 0) })}</h3>
        {ordered.length === 0 ? (
          <p className="muted" data-testid="image-no-areas">{pdf ? t('No area of this page is coded yet. {hint}', { hint }) : t('No area of this picture is coded yet. {hint}', { hint })}
          </p>
        ) : (
          <ul className="img-areas" data-testid="coded-areas" aria-label={pdf ? t('Coded areas of this page, top to bottom') : t('Coded areas of this picture, top to bottom')}>
            {ordered.map((g, i) => (
              <li key={g.key}>
                <button
                  type="button"
                  className={`img-area-button${selectedGroup?.key === g.key ? ' current' : ''}`}
                  aria-pressed={selectedGroup?.key === g.key}
                  data-testid={`area-button-${g.codings[0].id}`}
                  onClick={() => {
                    choose(g.codings[0].id);
                    announce(describeGroup(g, i, ordered.length), 'polite', { background: true });
                    window.setTimeout(() => stage.current?.focus(), 0);
                  }}
                >
                  <RegionThumb sourceId={data.id} image={pdf ? null : props.image} page={pdf ? page : null} shape={g.shape} size={48} />
                  <span className="img-area-text">
                    <span className="img-area-codes">
                      {g.codings.map((c) => (
                        <span key={c.id} className="bk-chip">
                          <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: codeOf(c.code_id)?.color ?? '#808080' }} />
                          {codeOf(c.code_id)?.name ?? t('A code')}
                        </span>
                      ))}
                    </span>
                    <span className="muted">{describeShape(g.shape, pdf ? 'page' : 'picture')}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {pdf ? (
          <div className="img-describe" data-testid="pdf-page-info">
            {pagesWith.length > 0 ? (
              <>
                <h3 className="bk-h3" id="pdf-pages-with-h">{t('Pages with coded areas')}</h3>
                <ul className="pdf-pages-with" aria-labelledby="pdf-pages-with-h" data-testid="pdf-pages-with">
                  {pagesWith.map((p) => (
                    <li key={p.page}>
                      <button
                        type="button"
                        className={`img-area-button${p.page === page ? ' current' : ''}`}
                        aria-current={p.page === page ? 'page' : undefined}
                        data-testid={`pdf-goto-${p.page}`}
                        onClick={() => {
                          goToPage(p.page);
                          window.setTimeout(() => stage.current?.focus(), 0);
                        }}
                      >
                        {t('Page {page}', { page: p.page })} <span className="muted">{tn(p.areas, { one: '({count} area)', other: '({count} areas)' })}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            ) : null}
            <details className="pdf-words" data-testid="pdf-words">
              <summary>{t('The words read from page {page}', { page })}</summary>
              {wordsOnPage === '' ? <p className="muted">{t('No words were read from this page. It may be a picture of a page, or blank.')}</p> : <p className="pdf-words-text" data-testid="pdf-words-text">{wordsOnPage}</p>}
            </details>
          </div>
        ) : (
        <div className="img-describe">
          <h3 className="bk-h3">{t('Description')}</h3>
          {editing ? (
            <div className="field">
              <label htmlFor="img-description">{t('What the picture shows, for anyone who cannot see it')}</label>
              <textarea id="img-description" rows={3} value={words} onChange={(e) => setWords(e.target.value)} data-testid="image-description-box" />
              <div className="actions">
                <Button
                  className="primary"
                  onPress={() => {
                    void saveImageDescription(data.id, words).then((ok) => ok && setEditing(false));
                  }}
                >{t('Save description')}</Button>
                <Button
                  onPress={() => {
                    setWords(data.text);
                    setEditing(false);
                  }}
                >{t('Cancel')}</Button>
              </div>
            </div>
          ) : (
            <>
              <p data-testid="image-description">{data.text}</p>
              {readOnly ? null : (
                <Button onPress={() => setEditing(true)} data-testid="image-describe">{t('Edit the description')}</Button>
              )}
            </>
          )}
        </div>
        )}
      </section>
    </div>
  );
}
