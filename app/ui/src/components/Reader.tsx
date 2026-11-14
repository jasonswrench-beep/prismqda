import { useMemo, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { Button, Menu, MenuItem, MenuSection, MenuTrigger, Popover, Separator } from 'react-aria-components';
import { hasReplies } from '../lib/thread';
import { sourceIcon } from '../lib/clips';
import { folderParts } from '../lib/docTree';
import { closeTab, defaultViewFor, goTo, selectSource, setView, toggleSplit } from '../state/actions';
import { appStore, isBigSource, useApp, type ViewKind } from '../state/app';
import { DocumentView } from './DocumentView';
import { GridView } from './GridView';
import { Icon } from './Icon';
import { Notice, SOURCE_TYPE_LABELS, plural } from './common';
import { ImageView } from './ImageView';
import { AttachPdf } from './AttachPdf';
import { MediaPlayer } from './MediaPlayer';
import { RecordingView } from './RecordingView';
import { SplitPane } from './SplitPane';
import type { SourceRow } from '../types';
import { compareText, num, t, tn } from '../i18n';
import { engineText } from '../i18n/engine';

const VIEW_LABELS: Record<ViewKind, string> = { get document() { return t('Document view'); }, get grid() { return t('Grid view'); }, get thread() { return t('Thread view'); }, get pages() { return t('Page view'); } };

/** The documents that share a folder with this one, in the order the Documents tree lists them: the set "1 of 3" counts in. */
export function siblingsOf(sources: readonly SourceRow[], current: SourceRow): SourceRow[] {
  const key = folderParts(current.folder).join('/');
  return sources
    .filter((s) => folderParts(s.folder).join('/') === key)
    .sort((a, b) => compareText(a.name, b.name, { numeric: true, sensitivity: 'base' }));
}

/**
 * The reader: a strip of tabs for the open documents, the document's header (its name, what it is, and the case it
 * belongs to), and the text with its coding. Closing a tab shows its neighbor; the plus opens another document.
 */
export function Reader() {
  const sources = useApp((s) => s.sources);
  const currentId = useApp((s) => s.currentSourceId);
  const tabs = useApp((s) => s.openTabs);
  const data = useApp((s) => s.sourceData);
  const loading = useApp((s) => s.loadingSourceId);
  const error = useApp((s) => s.sourceError);
  const choice = useApp((s) => s.viewChoice);
  const allowBig = useApp((s) => s.allowBigDocument);
  const cases = useApp((s) => s.cases);
  const splitId = useApp((s) => s.splitSourceId);
  const source = sources.find((s) => s.id === currentId) ?? null;
  const picture = source?.image ?? null;
  // A recording imported without captions is a document of its own: its text is a description, and it is coded by stretches of time.
  const standsAlone = !picture && !!source?.recording?.stands_alone;
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const tabEls = useRef<Record<number, HTMLButtonElement | null>>({});

  const openTabs = useMemo(() => tabs.map((id) => sources.find((s) => s.id === id)).filter((s): s is SourceRow => !!s), [tabs, sources]);
  const notOpen = useMemo(() => sources.filter((s) => !tabs.includes(s.id)), [sources, tabs]);

  const onTabKey = (e: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const go = (i: number) => {
      const target = openTabs[(i + openTabs.length) % openTabs.length];
      if (target) {
        e.preventDefault();
        tabEls.current[target.id]?.focus();
        void selectSource(target.id);
      }
    };
    if (e.key === 'ArrowRight') go(index + 1);
    else if (e.key === 'ArrowLeft') go(index - 1);
    else if (e.key === 'Home') go(0);
    else if (e.key === 'End') go(openTabs.length - 1);
    else if (e.key === 'Delete' && openTabs[index]) {
      e.preventDefault();
      closeTab(openTabs[index].id);
    }
  };

  const tabStrip = (
    <div className="bk-tabstrip">
      <p id="doc-tab-hint" className="visually-hidden">{t('Press Delete to close this document.')}</p>
      <div role="tablist" aria-label={t('Open documents')} className="bk-doc-tabs">
        {openTabs.map((t, i) => {
          const active = t.id === currentId;
          return (
            <div key={t.id} className={`bk-doc-tab${active ? ' current' : ''}`}>
              <button
                type="button"
                role="tab"
                id={`doc-tab-${t.id}`}
                aria-selected={active}
                aria-controls="main-content"
                aria-keyshortcuts="Delete"
                aria-describedby="doc-tab-hint"
                tabIndex={active ? 0 : -1}
                ref={(el) => {
                  tabEls.current[t.id] = el;
                }}
                className="bk-doc-tab-name"
                onClick={() => void selectSource(t.id)}
                onKeyDown={(e) => onTabKey(e, i)}
              >
                <Icon name={sourceIcon(t)} size={18} />
                <span className="bk-doc-tab-label">{t.name}</span>
                {/* A tab list may hold only tabs, so the close mark is for the pointer; the keyboard route is Delete on the tab. */}
                <span
                  className="bk-doc-tab-close"
                  aria-hidden="true"
                  data-testid={`close-tab-${t.id}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    closeTab(t.id);
                  }}
                >
                  <Icon name="ui-close" size={14} />
                </span>
              </button>
            </div>
          );
        })}
      </div>
      <MenuTrigger>
        <Button className="bk-tab-plus" aria-label={t('Open another document in a new tab')} isDisabled={notOpen.length === 0}>
          <Icon name="ui-plus" size={18} />
        </Button>
        <Popover className="popover" placement="bottom start">
          <Menu className="menu" aria-label={t('Documents that are not open')} onAction={(key) => void selectSource(Number(key))}>
            {notOpen.map((s) => (
              <MenuItem key={s.id} id={s.id} textValue={s.name} className="menu-item">
                {s.name}
              </MenuItem>
            ))}
          </Menu>
        </Popover>
      </MenuTrigger>
    </div>
  );

  if (!source) {
    return (
      <main id="main-content" tabIndex={-1} className="center bk-card bk-reader empty-center" aria-labelledby="h-source">
        {tabStrip}
        <div className="bk-reader-empty">
          <h2 id="h-source">{t('Reader')}</h2>
          <p>
            {sources.length === 0
              ? t('Add a document to begin. Interviews, documents, and spreadsheets of survey answers all work.')
              : t('Choose a document in Documents, or open one in a new tab with the plus above.')}
          </p>
          {sources.length === 0 ? (
            <Button className="primary" onPress={() => goTo('import')}>{t('Add documents')}</Button>
          ) : null}
        </div>
      </main>
    );
  }

  const ready = data && data.id === source.id;
  const view: ViewKind = choice[source.id] ?? defaultViewFor(source, ready ? hasReplies(data!.units) : false);
  // A PDF that keeps its file can be shown as its pages; the words view is then the words alone, without the coded areas of the pages.
  const showPages = view === 'pages' && !!source.pdf && !!ready && !!data!.pdf;
  // The coded areas of a PDF's pages and the coded stretches of a recording are not words, so the views of words are not given them.
  const textData = ready && (data!.pdf || source.recording) ? { ...data!, codings: data!.codings.filter((c) => !c.region) } : data;
  const textView: ViewKind = view === 'pages' ? 'document' : view;
  const big = !picture && !standsAlone && isBigSource(source);
  const blocked = textView === 'document' && !showPages && big && !allowBig[source.id];
  const set = siblingsOf(sources, source);
  const position = set.findIndex((s) => s.id === source.id);
  const caseNames = cases.filter((c) => c.source_ids?.includes(source.id)).map((c) => c.name);
  const facts = [
    ...(picture
      ? [t('Picture'), t('{width} by {height} pixels', { width: num(picture.width), height: num(picture.height) }), tn(source.codings, { one: '{count} coded area', other: '{count} coded areas' })]
      : standsAlone
        ? [source.recording!.kind === 'video' ? t('Video recording') : t('Audio recording'), tn(source.codings, { one: '{count} coded stretch', other: '{count} coded stretches' })]
        : [
          SOURCE_TYPE_LABELS[source.source_type] ?? source.source_type,
          ...(source.pdf ? [tn(source.pdf.page_count, { one: '{count} page', other: '{count} pages' })] : []),
          tn(source.characters, { one: '{count} character', other: '{count} characters' }),
          source.default_unit_kind === 'speaker_turn' ? tn(source.units, { one: '{count} turn', other: '{count} turns' }) : tn(source.units, { one: '{count} unit', other: '{count} units' }),
          tn(source.codings, { one: '{count} coding', other: '{count} codings' }),
        ]),
    ...(caseNames.length > 0 ? [t('Case: {names}', { names: caseNames.join(', ') })] : []),
  ];

  return (
    <main id="main-content" tabIndex={-1} className="center bk-card bk-reader" aria-labelledby="h-source" aria-busy={loading === source.id}>
      {tabStrip}
      <header className="bk-reader-head">
        <Icon name={sourceIcon(source)} size={30} className="bk-doc-icon" />
        <div className="bk-reader-title">
          <h2 id="h-source">{source.name}</h2>
          <p className="bk-meta" data-testid="source-facts">
            {facts.map((f, i) => (
              <span key={i}>{f}</span>
            ))}
          </p>
        </div>
        <div className="bk-reader-tools">
          <div className="source-nav" role="group" aria-label={t('Move between documents')}>
            <Button className="icon-button flip" aria-label={t('Previous document')} isDisabled={position <= 0} onPress={() => void selectSource(set[position - 1].id)}>
              <Icon name="ui-chevron-right" size={16} />
            </Button>
            <span data-testid="source-position">
              {t('{position} of {count}', { position: position + 1, count: set.length })}
            </span>
            <Button className="icon-button" aria-label={t('Next document')} isDisabled={position < 0 || position >= set.length - 1} onPress={() => void selectSource(set[position + 1].id)}>
              <Icon name="ui-chevron-right" size={16} />
            </Button>
          </div>
          {source.pdf ? (
            <div className="source-nav pdf-view-switch" role="group" aria-label={t('Show this PDF as')}>
              <Button className={`img-tool${showPages ? '' : ' pressed'}`} aria-pressed={!showPages} onPress={() => setView(source.id, 'document')} data-testid="pdf-view-words">{t('Words')}</Button>
              <Button className={`img-tool${showPages ? ' pressed' : ''}`} aria-pressed={showPages} onPress={() => setView(source.id, 'pages')} data-testid="pdf-view-pages">{t('Pages')}</Button>
            </div>
          ) : null}
          <Button className={`icon-button${splitId !== null ? ' pressed' : ''}`} aria-label={t('Split view')} aria-pressed={splitId !== null} onPress={toggleSplit} data-testid="split-toggle">
            <Icon name="ui-split-view" size={20} />
          </Button>
          <MenuTrigger>
            <Button className="icon-button" aria-label={t('More document actions')}>
              <Icon name="ui-more" size={20} />
            </Button>
            <Popover className="popover" placement="bottom end">
              <Menu className="menu" aria-label={t('Reader options')} onAction={(key) => {
                if (key === 'close') closeTab(source.id);
                else if (typeof key === 'string' && key.startsWith('view-')) setView(source.id, key.slice(5) as ViewKind);
              }}>
                {picture || standsAlone ? null : (
                  <MenuSection aria-label={t('Show this document as')} selectionMode="single" disallowEmptySelection selectedKeys={new Set([`view-${view}`])}>
                    {((source.pdf ? ['document', 'grid', 'thread', 'pages'] : ['document', 'grid', 'thread']) as ViewKind[]).map((v) => (
                      <MenuItem key={v} id={`view-${v}`} className="menu-item">
                        {VIEW_LABELS[v]}
                      </MenuItem>
                    ))}
                  </MenuSection>
                )}
                {picture || standsAlone ? null : <Separator className="menu-separator" />}
                <MenuItem id="close" className="menu-item">{t('Close this tab')}</MenuItem>
              </Menu>
            </Popover>
          </MenuTrigger>
        </div>
      </header>

      {ready && data!.report.length > 0 ? (
        <details className="import-report">
          <summary>{t('Import report: {notes}', { notes: tn(data!.report.length, { one: '{count} note', other: '{count} notes' }) })}</summary>
          <ul>
            {data!.report.map((w, i) => (
              <li key={i}>
                {engineText(w.message)}
                {w.location ? <span className="muted"> ({w.location})</span> : null}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
      {big ? <p className="muted source-facts">{t('This is a large source, so the Grid view is shown first.')}</p> : null}
      {view === 'thread' && ready && !hasReplies(data!.units) ? <p className="muted source-facts">{t('No unit replies to another, so the thread is a plain list.')}</p> : null}
      {ready && source.source_type === 'pdf' && !source.pdf && !readOnly ? <AttachPdf sourceId={source.id} /> : null}

      {ready && !standsAlone ? <MediaPlayer data={data!} isTranscript={source.source_type === 'transcript' || (source.source_type === 'other' && !picture)} /> : null}
      <SplitFrame on={splitId !== null} pane={<SplitPane currentId={source.id} />}>
      {error ? (
        <div>
          <Notice>{error}</Notice>
          <Button onPress={() => { appStore.set({ currentSourceId: null, sourceData: null }); void selectSource(source.id); }}>{t('Try again')}</Button>
        </div>
      ) : !ready ? (
        <p role="status">{t('Loading {name}…', { name: source.name })}</p>
      ) : blocked ? (
        <div className="big-warning">
          <Notice kind="warning">{t('This source has {units} and {characters}. The Document view puts all of its text on the page at once, which can be slow. The Grid view stays quick.', { units: tn(source.units, { one: '{count} unit', other: '{count} units' }), characters: tn(source.characters, { one: '{count} character', other: '{count} characters' }) })}</Notice>
          <div className="actions">
            <Button onPress={() => setView(source.id, 'grid')}>{t('Use the Grid view')}</Button>
            <Button onPress={() => appStore.set((s) => ({ allowBigDocument: { ...s.allowBigDocument, [source.id]: true } }))}>{t('Show the document anyway')}</Button>
          </div>
        </div>
      ) : picture ? (
        <ImageView key={source.id} data={data!} image={picture} />
      ) : standsAlone ? (
        <RecordingView key={source.id} data={data!} />
      ) : showPages ? (
        <ImageView key={source.id} data={data!} pdf={data!.pdf} />
      ) : textView === 'document' ? (
        <DocumentView data={textData!} />
      ) : (
        <GridView data={textData!} thread={textView === 'thread'} />
      )}
      </SplitFrame>
    </main>
  );
}

/** With split view on, the document being read and the second document stand side by side; off, the document fills the reader as before. */
function SplitFrame(props: { on: boolean; pane: ReactNode; children: ReactNode }) {
  if (!props.on) return <>{props.children}</>;
  return (
    <div className="bk-split">
      <div className="bk-split-main">{props.children}</div>
      {props.pane}
    </div>
  );
}
