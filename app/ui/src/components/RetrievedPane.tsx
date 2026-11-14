import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { Button, Cell, Checkbox, Column, Menu, MenuItem, MenuTrigger, Popover, Row, SearchField, Input, Label, Separator, Table, TableBody, TableHeader, ToggleButton, type Key, type Selection, type SortDescriptor } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { highlightColor } from '../lib/labels';
import { describeShape } from '../lib/regions';
import { describePassages, passagesRequest, previewText, pruneActive, searchRows, sortRows, type ResultRow, type SortKey } from '../lib/retrieval';
import { pathLabel } from '../lib/tree';
import { openAt, resetScope, selectCoding, setActiveCodes, setRetrievalCase, setRetrievalMode, setRetrievalQuery, setRetrievalView } from '../state/actions';
import { appStore, useApp } from '../state/app';
import type { AreaShape, SegmentRow } from '../types';
import { ClipPlaylist, type PlaylistClip } from './ClipPlaylist';
import { Icon } from './Icon';
import { RegionThumb } from './RegionThumb';
import { Notice } from './common';
import { compareText, num, rich, t, tn } from '../i18n';

interface Result {
  key: string;
  segments: SegmentRow[];
  total: number;
}

/** How many rows are drawn at a time. The rest are one press away: drawing hundreds of rows on every coding made coding feel slow. */
const PAGE = 100;

const COLUMN_KEYS: Record<string, SortKey> = { source: 'source', code: 'code', case: 'case', excerpt: 'excerpt', notes: 'notes' };

/**
 * Retrieved Segments: every coding in scope as a table (the document, the code, the case, a preview of the words, and
 * the note), kept current as you code. Pressing a row jumps to its words and fills Segment Details; the check boxes
 * mark rows for a bulk action without changing which row Segment Details describes.
 *
 * `splitter` is the dividing line on this pane's top edge; it lives inside the landmark so no control is left outside one.
 */
export function RetrievedPane({ splitter }: { splitter: ReactNode }) {
  const codes = useApp((s) => s.codes);
  const chosen = useApp((s) => s.activeCodeIds);
  const mode = useApp((s) => s.retrievalMode);
  const view = useApp((s) => s.retrievalView);
  const caseId = useApp((s) => s.retrievalCase);
  const query = useApp((s) => s.retrievalQuery);
  const openId = useApp((s) => s.currentSourceId);
  const sources = useApp((s) => s.sources);
  const cases = useApp((s) => s.cases);
  const activeCoding = useApp((s) => s.selectedCodingId);
  const version = useApp((s) => s.dataVersion);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [sort, setSort] = useState<SortDescriptor | undefined>(undefined);
  const [checked, setChecked] = useState<Selection>(new Set());
  const [retry, setRetry] = useState(0);
  const [drawn, setDrawn] = useState(PAGE);

  const ids = useMemo(() => pruneActive(chosen, codes), [chosen, codes]);
  const request = useMemo(() => passagesRequest(ids, mode, openId), [ids, mode, openId]);
  const requestKey = JSON.stringify(request);
  const activeCodes = useMemo(() => codes.filter((c) => c.status === 'active').sort((a, b) => compareText(pathLabel(codes, a.id), pathLabel(codes, b.id))), [codes]);
  const caseName = caseId === null ? null : (cases.find((c) => c.id === caseId)?.name ?? null);

  // Codes that were chosen and then retired or deleted fall out of the chosen list.
  useEffect(() => {
    if (ids.length !== chosen.length) setActiveCodes(ids);
  }, [ids, chosen.length]);

  // Ask the engine a moment after anything that could change the answer: the filters, the open document, or the coding
  // itself. The answer to an earlier question is dropped if a newer one has been asked since.
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    const timer = window.setTimeout(() => {
      void call('segments.list', request)
        .then((r) => {
          if (cancelled) return;
          setResult({ key: requestKey, segments: r.segments, total: r.total });
          setError(null);
          setLoading(false);
        })
        .catch((e) => {
          if (cancelled) return;
          setError(toApiError(e).message);
          setLoading(false);
        });
    }, 150);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // `codes` and `version` are here on purpose: a new coding changes them, and the table must follow.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestKey, codes, version, retry]);

  const caseOfSource = useMemo(() => {
    const m = new Map<number, string[]>();
    for (const c of cases) for (const sid of c.source_ids ?? []) m.set(sid, [...(m.get(sid) ?? []), c.name]);
    return m;
  }, [cases]);

  const imageOf = useMemo(() => new Map(sources.filter((s) => s.image).map((s) => [s.id, s.image!])), [sources]);
  const pdfIds = useMemo(() => new Set(sources.filter((s) => s.pdf).map((s) => s.id)), [sources]);
  const recordingOf = useMemo(() => new Map(sources.filter((s) => s.recording).map((s) => [s.id, s.recording!])), [sources]);

  const allRows = useMemo<(ResultRow & { color: string })[]>(() => {
    const byId = new Map(codes.map((c) => [c.id, c]));
    return (result?.segments ?? []).map((s) => ({
      coding_id: s.coding_id,
      source_id: s.source_id,
      source_name: s.source_name,
      code_id: s.code_id,
      code_name: s.code_name,
      text: s.text,
      notes: s.rationale ?? '',
      case_name: (caseOfSource.get(s.source_id) ?? []).join(', '),
      start: s.start,
      end: s.end,
      region: s.region,
      page: s.page,
      color: byId.get(s.code_id)?.color ?? '#808080',
    }));
  }, [result, codes, caseOfSource]);

  const rows = useMemo(() => {
    let r = allRows;
    if (caseId !== null) {
      const members = new Set(cases.find((c) => c.id === caseId)?.source_ids ?? []);
      r = r.filter((x) => members.has(x.source_id));
    }
    r = searchRows(r, query);
    const key = sort?.column ? (COLUMN_KEYS[String(sort.column)] ?? null) : null;
    return sortRows(r, key, sort?.direction ?? 'ascending');
  }, [allRows, caseId, cases, query, sort]);

  // A different question starts again from the first rows.
  useEffect(() => setDrawn(PAGE), [requestKey, caseId, query, sort]);
  const visible = rows.slice(0, drawn);
  // The gallery shows the areas of pictures among the rows, grouped by code in the order the rows come.
  const areas = useMemo(() => rows.filter((r) => r.region && r.region.shape !== 'span' && (imageOf.has(r.source_id) || pdfIds.has(r.source_id))), [rows, imageOf, pdfIds]);
  // The coded stretches of recordings among the rows, in the order the rows are in, for playing one after another.
  const [clipsOpen, setClipsOpen] = useState(false);
  const clips = useMemo<PlaylistClip[]>(
    () =>
      rows.flatMap((r) => {
        const rec = recordingOf.get(r.source_id);
        if (!rec || !r.region || r.region.shape !== 'span') return [];
        return [{ codingId: r.coding_id, sourceId: r.source_id, sourceName: r.source_name, codeName: r.code_name, startMs: r.region.start_ms, endMs: r.region.end_ms, kind: rec.kind }];
      }),
    [rows, recordingOf],
  );
  const galleryGroups = useMemo(() => {
    const groups: { code_id: number; code_name: string; color: string; items: typeof areas }[] = [];
    for (const r of areas.slice(0, drawn)) {
      let g = groups.find((x) => x.code_id === r.code_id);
      if (!g) {
        g = { code_id: r.code_id, code_name: r.code_name, color: r.color, items: [] };
        groups.push(g);
      }
      g.items.push(r);
    }
    return groups;
  }, [areas, drawn]);

  const filtered = ids.length > 0 || mode === 'open' || caseId !== null || query.trim() !== '';
  const openName = sources.find((s) => s.id === openId)?.name ?? null;
  const names = ids.map((id) => codes.find((c) => c.id === id)?.name ?? '').filter(Boolean);
  const stale = !result || result.key !== requestKey;
  // The engine lists at most MAX rows. When it cut the list and nothing was narrowed here, say how many there really are.
  const cut = !!result && result.total > result.segments.length && caseId === null && query.trim() === '';
  const summary = result && !stale ? describePassages({ total: cut ? result.total : rows.length, shown: rows.length, codeNames: names, mode, sourceName: openName }) : '';
  // The segment shown in Segment Details can be hidden by a filter; the table says so and leaves the reader where it is.
  const outside = activeCoding !== null && !stale && !error && !rows.some((r) => r.coding_id === activeCoding);

  const onChosenCodes = (keys: Selection) => setActiveCodes(keys === 'all' ? activeCodes.map((c) => c.id) : [...keys].map((k: Key) => Number(k)));

  const codesLabel = ids.length === 0 ? t('All Codes') : ids.length === 1 ? (codes.find((c) => c.id === ids[0])?.name ?? tn(1, { one: '{count} code', other: '{count} codes' })) : tn(ids.length, { one: '{count} code', other: '{count} codes' });
  const sourcesLabel = mode === 'open' ? t('The open document') : t('All Sources');
  const casesLabel = caseName ?? t('All Cases');

  return (
    <section className="bk-results-wrap" aria-labelledby="h-retrieved">
      {splitter}
      <div id="pane-retrieved" className="pane-part retrieved bk-card" tabIndex={-1}>
        <div className="bk-results-head">
          <Icon name="cmd-retrieve" size={20} className="bk-pane-icon" />
          <h2 id="h-retrieved">{rich('Retrieved Segments <span>({count})</span>', { count: result && !stale ? rows.length : '…', span: (inside) => <span data-testid="results-count">{inside}</span> })}
          </h2>
          <div className="bk-filters" role="group" aria-label={t('Filters for Retrieved Segments')}>
            <MenuTrigger>
              <Button className="bk-filter" aria-label={t('Codes: {codesLabel}', { codesLabel })}>
                <Icon name="ui-filter" size={16} />
                <span>{codesLabel}</span>
                <Icon name="ui-chevron-down" size={14} />
              </Button>
              <Popover className="popover" placement="bottom start">
                <Menu
                  className="menu choose-codes"
                  aria-label={t('Codes to show')}
                  selectionMode="multiple"
                  selectedKeys={new Set<Key>(ids)}
                  onSelectionChange={onChosenCodes}
                  renderEmptyState={() => <div className="menu-empty">{t('There are no codes yet.')}</div>}
                >
                  {activeCodes.map((c) => (
                    <MenuItem key={c.id} id={c.id} textValue={pathLabel(codes, c.id)} className="menu-item">
                      {pathLabel(codes, c.id)}
                    </MenuItem>
                  ))}
                </Menu>
              </Popover>
            </MenuTrigger>
            <MenuTrigger>
              <Button className="bk-filter" aria-label={t('Documents: {sourcesLabel}', { sourcesLabel })}>
                <Icon name="src-transcript" size={16} />
                <span>{sourcesLabel}</span>
                <Icon name="ui-chevron-down" size={14} />
              </Button>
              <Popover className="popover" placement="bottom start">
                <Menu className="menu" aria-label={t('Documents to show')} selectionMode="single" disallowEmptySelection selectedKeys={new Set<Key>([mode])} onSelectionChange={(keys) => {
                  const k = keys === 'all' ? 'all' : [...keys][0];
                  if (k === 'all' || k === 'open') setRetrievalMode(k);
                }}>
                  <MenuItem id="all" className="menu-item">{t('All documents')}</MenuItem>
                  <MenuItem id="open" className="menu-item">{t('The open document')}</MenuItem>
                </Menu>
              </Popover>
            </MenuTrigger>
            <MenuTrigger>
              <Button className="bk-filter" aria-label={t('Cases: {casesLabel}', { casesLabel })}>
                <Icon name="src-case" size={16} />
                <span>{casesLabel}</span>
                <Icon name="ui-chevron-down" size={14} />
              </Button>
              <Popover className="popover" placement="bottom start">
                <Menu
                  className="menu"
                  aria-label={t('Cases to show')}
                  selectionMode="single"
                  disallowEmptySelection
                  selectedKeys={new Set<Key>([caseId === null ? 'all' : String(caseId)])}
                  onSelectionChange={(keys) => {
                    const k = keys === 'all' ? 'all' : [...keys][0];
                    setRetrievalCase(k === undefined || k === 'all' ? null : Number(k));
                  }}
                  renderEmptyState={() => <div className="menu-empty">{t('There are no cases yet.')}</div>}
                >
                  <MenuItem id="all" className="menu-item">{t('All cases')}</MenuItem>
                  {cases.map((c) => (
                    <MenuItem key={c.id} id={String(c.id)} className="menu-item">
                      {c.name}
                    </MenuItem>
                  ))}
                </Menu>
              </Popover>
            </MenuTrigger>
            <ToggleButton className="bk-filter" isSelected={view === 'gallery'} onChange={(on) => setRetrievalView(on ? 'gallery' : 'list')} aria-label={t('Show the areas of pictures and PDF pages as a gallery')}>
              <Icon name="src-image" size={16} />
              <span>{t('Gallery')}</span>
            </ToggleButton>
            {clips.length > 0 || clipsOpen ? (
              <ToggleButton className="bk-filter" isSelected={clipsOpen} onChange={setClipsOpen} aria-label={t('Play the coded stretches of recordings one after another')} data-testid="clips-toggle">
                <Icon name="src-audio" size={16} />
                <span>{t('Clips')}</span>
              </ToggleButton>
            ) : null}
            <SearchField value={query} onChange={setRetrievalQuery} className="bk-segment-search" aria-label={t('Search segments')}>
              <Label className="visually-hidden">{t('Search segments')}</Label>
              <Icon name="cmd-search" size={16} />
              <Input placeholder={t('Search segments…')} />
              <Button aria-label={t('Clear the search')}>×</Button>
            </SearchField>
            <MenuTrigger>
            <Button className="icon-button" aria-label={t('More segment actions')}>
              <Icon name="ui-more" size={20} />
            </Button>
            <Popover className="popover" placement="bottom end">
              <Menu
                className="menu"
                aria-label={t('More segment actions')}
                onAction={(key) => {
                  if (key === 'clear') resetScope();
                  else if (key === 'export') appStore.set({ dialog: { kind: 'export', what: 'segments' } });
                  else if (key === 'uncheck') setChecked(new Set());
                }}
              >
                <MenuItem id="clear" className="menu-item" isDisabled={!filtered}>{t('Clear every filter')}</MenuItem>
                <MenuItem id="uncheck" className="menu-item" isDisabled={checked !== 'all' && checked.size === 0}>{t('Clear the checked rows')}</MenuItem>
                <Separator className="menu-separator" />
                <MenuItem id="export" className="menu-item">{t('Export segments…')}</MenuItem>
              </Menu>
            </Popover>
            </MenuTrigger>
          </div>
        </div>

        <p className="visually-hidden" role="status" data-testid="results-summary">
          {loading && !result ? t('Loading segments.') : summary}
        </p>
        {outside ? (
          <p className="bk-outside" role="status" data-testid="results-outside">{t('The selected segment is outside the rows shown here. Clear a filter, or choose another row; the reader has not moved.')}<Button className="bk-link" onPress={() => resetScope()}>{t('Clear filters')}</Button>
          </p>
        ) : null}

        {clipsOpen ? <ClipPlaylist clips={clips} /> : null}

        {error ? (
          <div className="bk-results-state">
            <Notice>{error}</Notice>
            <Button onPress={() => setRetry((n) => n + 1)}>{t('Try again')}</Button>
          </div>
        ) : !result || stale ? (
          <p className="bk-results-state muted" role="status">{t('Loading segments…')}</p>
        ) : rows.length === 0 ? (
          <div className="bk-results-state" data-testid="results-empty">
            <p>{mode === 'open' && openId === null ? t('No document is open.') : filtered ? t('No segment matches these filters.') : t('Nothing is coded yet. Select words in the reader and apply a code, and the segments appear here.')}</p>
            {filtered ? <Button onPress={() => resetScope()}>{t('Clear filters')}</Button> : null}
          </div>
        ) : view === 'gallery' ? (
          <div className="bk-table-scroll bk-gallery" data-testid="results-gallery">
            {areas.length === 0 ? (
              <p className="bk-results-state" data-testid="gallery-empty">{t('No area of a picture or a PDF page matches these filters. The gallery shows areas of pictures and PDF pages only; the list shows every segment.')}</p>
            ) : (
              <>
                <p className="visually-hidden" role="status">{tn(areas.length, { one: '{count} area of a picture or a PDF page.', other: '{count} areas of pictures and PDF pages.' })}</p>
                {galleryGroups.map((g) => (
                  <section key={g.code_id} className="bk-gallery-group" aria-labelledby={`gallery-${g.code_id}`}>
                    <h3 id={`gallery-${g.code_id}`} className="bk-gallery-head">
                      <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: g.color }} />
                      <span>{g.code_name}</span>
                      <span className="muted">{`(${num(areas.filter((a) => a.code_id === g.code_id).length)})`}</span>
                    </h3>
                    <ul className="bk-gallery-list">
                      {g.items.map((r) => (
                        <li key={r.coding_id}>
                          <Button
                            className={`bk-tile${r.coding_id === activeCoding ? ' active' : ''}`}
                            aria-label={t('Open {source_name} at this area, coded {code_name}', { source_name: r.source_name, code_name: r.code_name })}
                            onPress={() => {
                              selectCoding(r.coding_id);
                              void openAt(r.source_id, r.start, r.end, r.coding_id);
                            }}
                          >
                            <RegionThumb sourceId={r.source_id} image={imageOf.get(r.source_id) ?? null} page={r.page} shape={r.region as AreaShape} size={144} />
                            <span className="bk-tile-name bk-ellipsis">{r.source_name}</span>
                            {r.notes ? <span className="bk-tile-note bk-ellipsis muted">{r.notes}</span> : null}
                          </Button>
                        </li>
                      ))}
                    </ul>
                  </section>
                ))}
                {areas.length > drawn ? (
                  <div className="bk-more-rows" data-testid="more-rows">
                    <span role="status">{t('Showing {count} of {total} segments.', { count: num(Math.min(drawn, areas.length)), total: num(areas.length) })}</span>
                    <Button onPress={() => setDrawn((n) => n + PAGE)}>{t('Show {PAGE} more', { PAGE: Math.min(PAGE, areas.length - drawn) })}</Button>
                  </div>
                ) : null}
                {rows.length > areas.length ? (
                  <p className="muted bk-gallery-note" data-testid="gallery-words-note">{tn(rows.length - areas.length, { one: '{count} other segment is words, not a picture. It is in the list.', other: '{count} other segments are words, not pictures. They are in the list.' })}</p>
                ) : null}
              </>
            )}
          </div>
        ) : (
          <div className="bk-table-scroll">
            <Table
              aria-label={t('Retrieved Segments')}
              className="bk-table"
              selectionMode="multiple"
              selectionBehavior="toggle"
              selectedKeys={checked}
              onSelectionChange={setChecked}
              sortDescriptor={sort}
              onSortChange={setSort}
              onRowAction={(key) => {
                const r = rows.find((x) => x.coding_id === Number(key));
                if (!r) return;
                selectCoding(r.coding_id);
                void openAt(r.source_id, r.start, r.end, r.coding_id);
              }}
            >
              <TableHeader>
                <Column id="check" className="bk-col-check">
                  <Checkbox slot="selection" aria-label={t('Check every row shown')} />
                </Column>
                <Column id="num" className="bk-col-num">
                  #
                </Column>
                <Column id="source" isRowHeader allowsSorting className="bk-col-source">{t('Source')}</Column>
                <Column id="code" allowsSorting className="bk-col-code">{t('Code')}</Column>
                <Column id="case" allowsSorting className="bk-col-case">{t('Case')}</Column>
                <Column id="excerpt" allowsSorting className="bk-col-excerpt">{t('Excerpt (preview)')}</Column>
                <Column id="notes" allowsSorting className="bk-col-notes">{t('Notes')}</Column>
              </TableHeader>
              <TableBody items={visible}>
                {(r) => (
                  <Row id={r.coding_id} className={`bk-row${r.coding_id === activeCoding ? ' active' : ''}`} textValue={`${r.source_name}, ${r.code_name}`}>
                    <Cell className="bk-col-check">
                      <Checkbox slot="selection" aria-label={t('Check row for {source_name}, {code_name}', { source_name: r.source_name, code_name: r.code_name })} />
                    </Cell>
                    <Cell className="bk-col-num">{visible.indexOf(r) + 1}</Cell>
                    <Cell className="bk-col-source">
                      <span className="bk-cell">
                        <Icon name={r.region?.shape === 'span' ? (recordingOf.get(r.source_id)?.kind === 'video' ? 'src-video' : 'src-audio') : r.region ? 'src-image' : 'src-transcript'} size={16} />
                        <span>{r.source_name}</span>
                      </span>
                    </Cell>
                    <Cell className="bk-col-code">
                      <span className="bk-chip" style={{ background: highlightColor(r.color, 0.22) }}>
                        <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: r.color }} />
                        <span>{r.code_name}</span>
                      </span>
                    </Cell>
                    <Cell className="bk-col-case">
                      {r.case_name ? (
                        <span className="bk-cell">
                          <Icon name="src-case" size={16} />
                          <span>{r.case_name}</span>
                        </span>
                      ) : (
                        <span className="muted" aria-label={t('No case')}>
                          –
                        </span>
                      )}
                    </Cell>
                    <Cell className="bk-col-excerpt">
                      {r.region?.shape === 'span' ? (
                        <span className="bk-cell" data-testid="clip-excerpt">
                          <span className="bk-ellipsis">{describeShape(r.region)}</span>
                        </span>
                      ) : r.region && (imageOf.get(r.source_id) || pdfIds.has(r.source_id)) ? (
                        <span className="bk-cell" data-testid="area-excerpt">
                          <RegionThumb sourceId={r.source_id} image={imageOf.get(r.source_id) ?? null} page={r.page} shape={r.region} size={32} />
                          <span className="bk-ellipsis">{previewText(r.text)}</span>
                        </span>
                      ) : (
                        <span className="bk-ellipsis">{`“${previewText(r.text)}”`}</span>
                      )}
                    </Cell>
                    <Cell className="bk-col-notes">
                      <span className="bk-ellipsis">{r.notes}</span>
                    </Cell>
                  </Row>
                )}
              </TableBody>
            </Table>
            {rows.length > visible.length ? (
              <div className="bk-more-rows" data-testid="more-rows">
                <span role="status">{t('Showing {count} of {total} segments.', { count: num(visible.length), total: num(rows.length) })}</span>
                <Button onPress={() => setDrawn((n) => n + PAGE)}>{t('Show {PAGE} more', { PAGE: Math.min(PAGE, rows.length - visible.length) })}</Button>
              </div>
            ) : null}
          </div>
        )}
      </div>
    </section>
  );
}
