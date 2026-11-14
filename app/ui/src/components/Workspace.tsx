import { useEffect, useRef, useState } from 'react';
import { readJson, writeJson } from '../lib/storage';
import { useApp } from '../state/app';
import { CodebookPane } from './CodebookPane';
import { DetailPanel } from './DetailPanel';
import { DocumentsPane } from './DocumentsPane';
import { Reader } from './Reader';
import { RetrievedPane } from './RetrievedPane';
import { Splitter } from './Splitter';
import { t } from '../i18n';

interface Layout {
  /** Width of the left column (Documents over Codes). */
  left: number;
  /** Width of Segment Details. */
  right: number;
  /** Height of Documents, above Codes. */
  sources: number;
  /** Height of Retrieved Segments, under the reader. */
  passages: number;
}

const LAYOUT_KEY = 'prismqda.layout.v2';
/** The standard sizes, in pixels at the standard text size, from the screen specification book's picture (322 and 344 wide). */
const DEFAULT_LAYOUT: Layout = { left: 322, right: 344, sources: 290, passages: 235 };
const MIN_SIDE = 200;
const MIN_CENTER = 360;
const MIN_PART = 140;
/** The results area's smallest height: its toolbar and a few rows. */
const EMPTY_PASSAGES = 120;

function loadLayout(): Layout {
  const raw = readJson<Partial<Layout>>(LAYOUT_KEY, {});
  const ok = (v: unknown, d: number) => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : d);
  return { left: ok(raw.left, DEFAULT_LAYOUT.left), right: ok(raw.right, DEFAULT_LAYOUT.right), sources: ok(raw.sources, DEFAULT_LAYOUT.sources), passages: ok(raw.passages, DEFAULT_LAYOUT.passages) };
}

export function Workspace() {
  const [layout, setLayout] = useState<Layout>(loadLayout);
  const [size, setSize] = useState({ w: 1200, h: 700 });
  const box = useRef<HTMLDivElement>(null);
  const panes = useApp((s) => s.panes);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // The sizes are kept as they are at the standard text size and grow with the text size in Settings, so the panels
  // hold the same amount of text at any size. Dragging a bar at 130 percent keeps its place at every other size.
  const k = useApp((s) => s.settings.textSize) / 100;
  const SPLIT = 8;
  const hasLeft = panes.documents || panes.codes;
  const hasRight = panes.details;
  const minSide = MIN_SIDE * k;
  const minCenter = MIN_CENTER * k;
  const minPart = MIN_PART * k;
  const leftTaken = hasLeft ? layout.left * k + SPLIT : 0;
  const rightTaken = hasRight ? layout.right * k + SPLIT : 0;
  const leftMax = Math.max(minSide, size.w - rightTaken - minCenter - SPLIT);
  const rightMax = Math.max(minSide, size.w - leftTaken - minCenter - SPLIT);
  const left = Math.min(Math.max(layout.left * k, minSide), leftMax);
  const right = Math.min(Math.max(layout.right * k, minSide), rightMax);
  const passagesMax = Math.max(minPart, size.h - 260 * k);
  const passages = Math.min(Math.max(layout.passages * k, EMPTY_PASSAGES * k), passagesMax);
  const sourcesMax = Math.max(minPart, size.h - minPart - SPLIT - 40 * k);
  const sources = Math.min(Math.max(layout.sources * k, minPart), sourcesMax);

  const remember = (patch: Partial<Layout>) => {
    const next = { ...layout, ...patch };
    setLayout(next);
    writeJson(LAYOUT_KEY, next);
  };

  const columns = [hasLeft ? `${left + SPLIT}px` : null, 'minmax(0, 1fr)', hasRight ? `${right + SPLIT}px` : null].filter(Boolean).join(' ');
  const sideRows = panes.documents && panes.codes ? `${sources}px ${SPLIT}px minmax(0, 1fr)` : 'minmax(0, 1fr)';

  // Every dividing line sits inside one of the landmark areas, so no control is outside a landmark.
  return (
    <div className="workspace-wrap bk-workspace" ref={box}>
      <div
        className="workspace"
        style={{
          gridTemplateColumns: columns,
          ['--left' as string]: `${left}px`,
          ['--right' as string]: `${right}px`,
          ['--sources' as string]: `${sources}px`,
          ['--passages' as string]: `${panes.results ? passages : 0}px`,
        }}
      >
        {hasLeft ? (
          <aside id="pane-left" className="pane pane-left" aria-label={t('Documents and codes')}>
            <div className="side-stack" style={{ gridTemplateRows: sideRows }}>
              {panes.documents ? <DocumentsPane /> : null}
              {panes.documents && panes.codes ? (
                <Splitter
                  orientation="horizontal"
                  label={t('Resize Documents')}
                  value={sources}
                  min={minPart}
                  max={sourcesMax}
                  controls="pane-sources"
                  onChange={(v) => setLayout((l) => ({ ...l, sources: v / k }))}
                  onCommit={(v) => remember({ sources: v / k })}
                />
              ) : null}
              {panes.codes ? <CodebookPane /> : null}
            </div>
            <Splitter
              orientation="vertical"
              label={t('Resize the side panel')}
              value={left}
              min={minSide}
              max={leftMax}
              controls="pane-left"
              onChange={(v) => setLayout((l) => ({ ...l, left: v / k }))}
              onCommit={(v) => remember({ left: v / k })}
            />
          </aside>
        ) : null}
        <div className="center-stack" style={{ gridTemplateRows: panes.results ? undefined : 'minmax(0, 1fr)' }}>
          <Reader />
          {panes.results ? (
            <RetrievedPane
              splitter={
                <Splitter
                  orientation="horizontal"
                  label={t('Resize Retrieved Segments')}
                  value={passages}
                  min={EMPTY_PASSAGES * k}
                  max={passagesMax}
                  reverse
                  controls="pane-retrieved"
                  onChange={(v) => setLayout((l) => ({ ...l, passages: v / k }))}
                  onCommit={(v) => remember({ passages: v / k })}
                />
              }
            />
          ) : null}
        </div>
        {hasRight ? (
          <aside id="pane-right" className="pane pane-right" aria-label={t('Inspector')}>
            <Splitter
              orientation="vertical"
              label={t('Resize Segment Details')}
              value={right}
              min={minSide}
              max={rightMax}
              reverse
              controls="pane-details"
              onChange={(v) => setLayout((l) => ({ ...l, right: v / k }))}
              onCommit={(v) => remember({ right: v / k })}
            />
            <div className="right-inner">
              <DetailPanel />
            </div>
          </aside>
        ) : null}
      </div>
    </div>
  );
}
