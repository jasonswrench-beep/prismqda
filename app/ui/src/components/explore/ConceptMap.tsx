// The Concept Map: codes, cases, and notes on a page, joined by links with words on them. The drawing can be worked with the mouse
// (drag a node) or the keyboard (arrow keys move the chosen node), and everything the drawing shows is also written out as lists and
// forms, so nothing depends on seeing it.

import { useEffect, useMemo, useRef, useState } from 'react';
import { Button, Checkbox, Input, Label, TextField } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { edgePoint, KIND_WORDS, linkSentence, mapInWords, midpoint, moveBy, NODE_H, NODE_W, shorten, viewBoxFor } from '../../lib/conceptMap';
import { bumpData, goTo, selectCode, setExploreMap, undoLast } from '../../state/actions';
import { announce } from '../../state/announce';
import { appStore, useApp } from '../../state/app';
import type { ConceptMap, MapLayout, MapLink, MapNode, MapRow } from '../../types';
import { Notice, plural } from '../common';
import { CheckPicker, codeChoices } from './shared';
import { t, tn } from '../../i18n';

type Form = { mode: 'new' | 'rename'; name: string };

function Box() {
  return (
    <div className="box" aria-hidden="true">
      <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
        <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
      </svg>
    </div>
  );
}

/** A place for the next node: to the right of the rightmost, or the middle of an empty page. */
function freePlace(nodes: readonly MapNode[]): { x: number; y: number } {
  if (nodes.length === 0) return { x: 0, y: 0 };
  const right = nodes.reduce((a, n) => (n.x > a.x ? n : a), nodes[0]);
  return { x: right.x + NODE_W + 50, y: right.y };
}

export function ConceptMapTool() {
  const { mapId, selected } = useApp((s) => s.exploreMap);
  const version = useApp((s) => s.dataVersion);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const codes = useApp((s) => s.codes);
  const [maps, setMaps] = useState<MapRow[] | null>(null);
  const [map, setMap] = useState<ConceptMap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<{ text: string; canUndo: boolean } | null>(null);
  const [form, setForm] = useState<Form | null>(null);
  const [drag, setDrag] = useState<{ id: number; x: number; y: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const svgRef = useRef<SVGSVGElement>(null);
  const moved = useRef(false);

  useEffect(() => {
    let live = true;
    call('map.list')
      .then((r) => {
        if (!live) return;
        setMaps(r);
        if (r.length > 0 && !r.some((m) => m.id === appStore.get().exploreMap.mapId)) setExploreMap({ mapId: r[0].id, selected: null });
        if (r.length === 0) setExploreMap({ mapId: null, selected: null });
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version]);

  useEffect(() => {
    if (mapId === null) {
      setMap(null);
      return;
    }
    let live = true;
    call('map.get', { id: mapId })
      .then((m) => {
        if (!live) return;
        setMap(m);
        setError(null);
        if (selected !== null && !m.nodes.some((n) => n.id === selected)) setExploreMap({ selected: null });
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // The map is read again when its number or anything in the project changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mapId, version]);

  const fail = (e: unknown) => {
    const m = toApiError(e).message;
    setError(m);
    announce(m, 'assertive');
  };

  /** Runs a change, then reads the map again; says why when it could not be made. */
  const change = async <T,>(fn: () => Promise<T>, said?: string | ((r: T) => string), canUndo = true): Promise<T | undefined> => {
    setError(null);
    setBusy(true);
    try {
      const r = await fn();
      bumpData();
      const text = typeof said === 'function' ? said(r) : said;
      if (text) {
        setNote({ text, canUndo });
        announce(text);
      }
      return r;
    } catch (e) {
      fail(e);
      return undefined;
    } finally {
      setBusy(false);
    }
  };

  const nodes = map?.nodes ?? [];
  const links = map?.links ?? [];
  const chosen = nodes.find((n) => n.id === selected) ?? null;
  const at = (n: MapNode) => (drag && drag.id === n.id ? { x: drag.x, y: drag.y } : { x: n.x, y: n.y });
  const centers = nodes.map(at);
  const box = viewBoxFor(centers);
  const codeColor = (n: MapNode) => (n.kind === 'code' ? codes.find((c) => c.id === n.ref_id)?.color : undefined);
  const words = useMemo(() => mapInWords(nodes, links), [nodes, links]);
  const labelOf = (id: number) => nodes.find((n) => n.id === id)?.label ?? '?';

  // ---- the mouse: drag a node
  const toMap = (clientX: number, clientY: number): { x: number; y: number } => {
    const svg = svgRef.current;
    const m = svg?.getScreenCTM();
    if (!svg || !m) return { x: 0, y: 0 };
    const p = new DOMPoint(clientX, clientY).matrixTransform(m.inverse());
    return { x: p.x, y: p.y };
  };
  const startDrag = (e: React.PointerEvent, n: MapNode) => {
    if (readOnly || e.button !== 0) return;
    const start = toMap(e.clientX, e.clientY);
    const offset = { x: n.x - start.x, y: n.y - start.y };
    moved.current = false;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const p = toMap(ev.clientX, ev.clientY);
      const next = { id: n.id, x: p.x + offset.x, y: p.y + offset.y };
      if (Math.abs(next.x - n.x) + Math.abs(next.y - n.y) > 4) moved.current = true;
      setDrag(next);
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      const p = toMap(ev.clientX, ev.clientY);
      const end = { x: Math.round(p.x + offset.x), y: Math.round(p.y + offset.y) };
      setDrag(null);
      if (moved.current) void change(() => call('map.node_move', { id: n.id, x: end.x, y: end.y }), t('Moved {label}.', { label: n.label }));
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };

  // ---- the keyboard: a chosen node moves with the arrow keys
  const keyOnNode = (e: React.KeyboardEvent, n: MapNode) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      setExploreMap({ selected: n.id });
      announce(t('{label} chosen.', { label: n.label }));
      return;
    }
    if (readOnly) return;
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      void removeNode(n);
      return;
    }
    const to = moveBy({ x: n.x, y: n.y }, e.key, e.shiftKey);
    if (to) {
      e.preventDefault();
      if (!busy) void change(() => call('map.node_move', { id: n.id, x: to.x, y: to.y }));
    }
  };

  const removeNode = async (n: MapNode) => {
    await change(() => call('map.node_delete', { id: n.id }), t('Took {label} off the map, with {links}.', { label: n.label, links: tn(links.filter((l) => l.from_node === n.id || l.to_node === n.id).length, { one: '{count} link', other: '{count} links' }) }));
    setExploreMap({ selected: null });
  };

  return (
    <div data-testid="map-tool">
      <section className="ex-explore-controls" aria-label={t('Maps')} data-testid="map-picker">
        <div className="field">
          <label htmlFor="cm-map">{t('Map')}</label>
          <select id="cm-map" value={mapId === null ? '' : String(mapId)} onChange={(e) => setExploreMap({ mapId: e.target.value === '' ? null : Number(e.target.value), selected: null })} disabled={!maps || maps.length === 0}>
            {!maps || maps.length === 0 ? <option value="">{t('No maps yet')}</option> : null}
            {(maps ?? []).map((m) => (
              <option key={m.id} value={m.id}>
                {m.name} ({tn(m.nodes, { one: '{count} node', other: '{count} nodes' })}, {tn(m.links, { one: '{count} link', other: '{count} links' })})
              </option>
            ))}
          </select>
        </div>
        <div className="actions">
          <Button onPress={() => setForm({ mode: 'new', name: '' })} isDisabled={readOnly} data-testid="new-map">{t('New map')}</Button>
          <Button onPress={() => map && setForm({ mode: 'rename', name: map.name })} isDisabled={readOnly || !map}>{t('Rename')}</Button>
          <Button
            onPress={() =>
              map &&
              void change(async () => {
                await call('map.delete', { id: map.id });
                setExploreMap({ mapId: null, selected: null });
              }, t('Deleted the map {name}.', { name: map.name }))
            }
            isDisabled={readOnly || !map || busy}
            data-testid="delete-map"
          >{t('Delete')}</Button>
        </div>
      </section>

      {form ? (
        <form
          className="ex-saved-form"
          aria-label={form.mode === 'new' ? t('Make a map') : t('Rename the map')}
          onSubmit={(e) => {
            e.preventDefault();
            if (form.mode === 'new') {
              void change(async () => {
                const m = await call('map.create', { name: form.name });
                setExploreMap({ mapId: m.id, selected: null });
                setForm(null);
              }, t('Made the map {name}.', { name: form.name.trim() }));
            } else if (map) {
              void change(async () => {
                await call('map.update', { id: map.id, name: form.name });
                setForm(null);
              }, t('Renamed the map to {name}.', { name: form.name.trim() }));
            }
          }}
        >
          <TextField value={form.name} onChange={(v) => setForm({ ...form, name: v })} className="field" autoFocus>
            <Label>{t('Name of the map')}</Label>
            <Input data-testid="map-name" />
          </TextField>
          <div className="actions">
            <Button type="submit" className="primary" isDisabled={form.name.trim() === ''} data-testid="map-name-save">
              {form.mode === 'new' ? t('Make the map') : t('Rename')}
            </Button>
            <Button onPress={() => setForm(null)}>{t('Cancel')}</Button>
          </div>
        </form>
      ) : null}

      {error ? <Notice>{error}</Notice> : null}
      {note ? (
        <div className="ac-result" role="status" data-testid="map-note">
          <p>{note.text}</p>
          <div className="actions">
            {note.canUndo ? (
              <Button
                onPress={() => {
                  setNote(null);
                  void undoLast();
                }}
                isDisabled={readOnly}
                data-testid="map-undo"
              >{t('Undo')}</Button>
            ) : null}
            <Button onPress={() => setNote(null)}>{t('Dismiss')}</Button>
          </div>
        </div>
      ) : null}

      {!map ? (
        <p className="help" data-testid="map-none">
          {maps === null ? t('Loading the maps…') : t('A concept map puts codes, cases, and notes on a page and joins them with links that have words on them. Make a map to begin.')}
        </p>
      ) : (
        <>
          <AddToMap map={map} nodes={nodes} readOnly={readOnly} busy={busy} change={change} />

          <div className="cm-canvas" data-testid="map-canvas">
            <svg
              ref={svgRef}
              viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`}
              role="group"
              aria-label={t('Concept map {name}: {nodes} and {links}. The same map is written out below the drawing.', { name: map.name, nodes: tn(nodes.length, { one: '{count} node', other: '{count} nodes' }), links: tn(links.length, { one: '{count} link', other: '{count} links' }) })}
              className="cm-svg"
            >
              <defs>
                <marker id="cm-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">
                  <path d="M 0 0 L 10 5 L 0 10 z" className="cm-arrowhead" />
                </marker>
              </defs>
              {links.map((l) => {
                const a = nodes.find((n) => n.id === l.from_node);
                const b = nodes.find((n) => n.id === l.to_node);
                if (!a || !b) return null;
                const pa = at(a);
                const pb = at(b);
                const from = edgePoint(pa, pb);
                const to = edgePoint(pb, pa);
                const mid = midpoint(from, to);
                return (
                  <g key={l.id} className="cm-link" aria-hidden="true">
                    <line x1={from.x} y1={from.y} x2={to.x} y2={to.y} markerEnd={l.directed ? 'url(#cm-arrow)' : undefined} />
                    {l.label ? (
                      <text x={mid.x} y={mid.y - 6} textAnchor="middle">
                        {shorten(l.label, 28)}
                      </text>
                    ) : null}
                  </g>
                );
              })}
              {nodes.map((n) => {
                const p = at(n);
                const isSelected = n.id === selected;
                return (
                  <g
                    key={n.id}
                    transform={`translate(${p.x - NODE_W / 2} ${p.y - NODE_H / 2})`}
                    className={`cm-node ${n.kind}${isSelected ? ' selected' : ''}${n.missing ? ' missing' : ''}`}
                    tabIndex={0}
                    role="button"
                    aria-pressed={isSelected}
                    aria-label={isSelected ? t('{label}, a {value}, chosen. Press Enter to choose it, the arrow keys to move it, Delete to take it off the map.', { label: n.label, value: KIND_WORDS[n.kind] ?? n.kind }) : t('{label}, a {value}. Press Enter to choose it, the arrow keys to move it, Delete to take it off the map.', { label: n.label, value: KIND_WORDS[n.kind] ?? n.kind })}
                    data-testid="map-node"
                    data-node={n.id}
                    onClick={() => {
                      if (!moved.current) setExploreMap({ selected: n.id });
                    }}
                    onKeyDown={(e) => keyOnNode(e, n)}
                    onPointerDown={(e) => startDrag(e, n)}
                  >
                    <rect width={NODE_W} height={NODE_H} rx={10} />
                    {codeColor(n) ? <rect className="cm-swatch" width={9} height={NODE_H} rx={4} style={{ fill: codeColor(n) }} /> : null}
                    <text className="cm-kind" x={NODE_W / 2} y={17} textAnchor="middle">
                      {KIND_WORDS[n.kind] ?? n.kind}
                    </text>
                    <text className="cm-label" x={NODE_W / 2} y={35} textAnchor="middle">
                      {shorten(n.label)}
                    </text>
                    <title>{n.label}</title>
                  </g>
                );
              })}
            </svg>
          </div>

          <div className="cm-below">
            <section className="cm-side" aria-labelledby="h-cm-words" data-testid="map-words">
              <h3 id="h-cm-words" className="bk-h3">{t('The map in words')}</h3>
              {nodes.length === 0 ? (
                <p className="help">{t('There is nothing on this map yet. Add a code, a case, or a note above.')}</p>
              ) : (
                <>
                  <ul aria-label={t('Nodes')} className="cm-list">
                    {nodes.map((n) => (
                      <li key={n.id}>
                        <Button className={`bk-link${n.id === selected ? ' current' : ''}`} aria-pressed={n.id === selected} onPress={() => setExploreMap({ selected: n.id })} data-testid="map-node-button">
                          {n.label} ({KIND_WORDS[n.kind]})
                        </Button>
                      </li>
                    ))}
                  </ul>
                  {words.sentences.length > 0 ? (
                    <ul aria-label={t('Links')} className="cm-list">
                      {words.sentences.map((t, i) => (
                        <li key={i}>{t}</li>
                      ))}
                    </ul>
                  ) : (
                    <p className="muted">{t('No links yet.')}</p>
                  )}
                  {words.alone.length > 0 && words.sentences.length > 0 ? <p className="muted">{t('Not joined to anything: {list}.', { list: words.alone.join(', ') })}</p> : null}
                </>
              )}
            </section>

            <section className="cm-side" aria-labelledby="h-cm-node" data-testid="map-node-panel">
              <h3 id="h-cm-node" className="bk-h3">
                {chosen ? chosen.label : t('The chosen node')}
              </h3>
              {chosen ? (
                <NodePanel map={map} node={chosen} nodes={nodes} links={links} labelOf={labelOf} readOnly={readOnly} busy={busy} change={change} remove={() => void removeNode(chosen)} />
              ) : (
                <p className="help">{t('Choose a node in the drawing, or in the list, to change it and to join it to another.')}</p>
              )}
            </section>
          </div>
        </>
      )}
    </div>
  );
}

type ChangeFn = <T>(fn: () => Promise<T>, said?: string | ((r: T) => string), canUndo?: boolean) => Promise<T | undefined>;

/** The forms that put things on the map: a code, several codes, a case, a note, and a layout. */
function AddToMap(props: { map: ConceptMap; nodes: MapNode[]; readOnly: boolean; busy: boolean; change: ChangeFn }) {
  const { map, nodes } = props;
  const codes = useApp((s) => s.codes);
  const cases = useApp((s) => s.cases);
  const choices = useMemo(() => codeChoices(codes), [codes]);
  const [codeId, setCodeId] = useState('');
  const [caseId, setCaseId] = useState('');
  const [noteText, setNoteText] = useState('');
  const [many, setMany] = useState<number[]>([]);
  const [hierarchy, setHierarchy] = useState(true);
  const [together, setTogether] = useState(false);
  const [minTogether, setMinTogether] = useState('2');
  const [layout, setLayout] = useState<MapLayout>('layers');
  const dis = props.readOnly || props.busy;
  const onMap = (kind: string, id: number) => nodes.some((n) => n.kind === kind && n.ref_id === id);
  const place = freePlace(nodes);
  const minNumber = Number(minTogether);

  return (
    <section className="ex-explore-controls cm-add" aria-label={t('Put things on the map')} data-testid="map-add">
      <div className="field">
        <label htmlFor="cm-code">{t('A code')}</label>
        <select id="cm-code" value={codeId} onChange={(e) => setCodeId(e.target.value)}>
          <option value="">{t('Choose a code')}</option>
          {choices.map((c) => (
            <option key={c.id} value={c.id} disabled={onMap('code', c.id)}>
              {'  '.repeat(c.depth ?? 0)}
              {c.label}
              {onMap('code', c.id) ? ' ' + t('(on the map)') : ''}
            </option>
          ))}
        </select>
        <Button
          onPress={() => void props.change(() => call('map.node_add', { map_id: map.id, kind: 'code', ref_id: Number(codeId), x: place.x, y: place.y }), t('Put the code on the map.')).then(() => setCodeId(''))}
          isDisabled={dis || codeId === ''}
          data-testid="add-code-node"
        >{t('Add the code')}</Button>
      </div>
      <div className="field">
        <label htmlFor="cm-case">{t('A case')}</label>
        <select id="cm-case" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
          <option value="">{t('Choose a case')}</option>
          {cases.map((c) => (
            <option key={c.id} value={c.id} disabled={onMap('case', c.id)}>
              {c.name}
              {onMap('case', c.id) ? ' ' + t('(on the map)') : ''}
            </option>
          ))}
        </select>
        <Button onPress={() => void props.change(() => call('map.node_add', { map_id: map.id, kind: 'case', ref_id: Number(caseId), x: place.x, y: place.y }), t('Put the case on the map.')).then(() => setCaseId(''))} isDisabled={dis || caseId === ''}>{t('Add the case')}</Button>
      </div>
      <div className="field">
        <label htmlFor="cm-note">{t('A note')}</label>
        <input id="cm-note" type="text" value={noteText} onChange={(e) => setNoteText(e.target.value)} placeholder={t('Words of your own')} />
        <Button onPress={() => void props.change(() => call('map.node_add', { map_id: map.id, kind: 'note', label: noteText, x: place.x, y: place.y }), t('Put the note on the map.')).then(() => setNoteText(''))} isDisabled={dis || noteText.trim() === ''} data-testid="add-note-node">{t('Add the note')}</Button>
      </div>
      <div className="field cm-many">
        <span className="ex-field-label">{t('Several codes at once')}</span>
        <CheckPicker noun="codes" label={t('Codes to put on the map')} choices={choices} value={many} onChange={setMany} none={t('Choose codes')} testId="map-codes" />
        <Checkbox isSelected={hierarchy} onChange={setHierarchy} className="check ex-explore-check">
          <Box />{t('Join a code to its sub-codes')}</Checkbox>
        <Checkbox isSelected={together} onChange={setTogether} className="check ex-explore-check">
          <Box />{t('Join codes that occur together at least')}</Checkbox>
        <input aria-label={t('How many times codes must occur together')} type="number" min={1} value={minTogether} onChange={(e) => setMinTogether(e.target.value)} className="cm-number" disabled={!together} />
        <Button
          onPress={() =>
            void props
              .change(
                () => call('map.add_codes', { map_id: map.id, code_ids: many, hierarchy, ...(together && Number.isInteger(minNumber) && minNumber >= 1 ? { min_together: minNumber } : {}) }),
                (r) =>
                  t('Put {codes} on the map with {links}.', { codes: tn(r.nodes_added, { one: '{count} code', other: '{count} codes' }), links: tn(r.links_added, { one: '{count} link', other: '{count} links' }) }) + (r.already_there > 0 ? ' ' + tn(r.already_there, { one: '{codes} was already there.', other: '{codes} were already there.' }, { codes: tn(r.already_there, { one: '{count} code', other: '{count} codes' }) }) : ''),
              )
              .then((r) => r && setMany([]))
          }
          isDisabled={dis || many.length === 0}
          data-testid="add-codes-together"
        >{t('Add these codes')}</Button>
      </div>
      <div className="field">
        <label htmlFor="cm-layout">{t('Arrange the nodes')}</label>
        <select id="cm-layout" value={layout} onChange={(e) => setLayout(e.target.value as MapLayout)}>
          <option value="layers">{t('In layers (arrows point down)')}</option>
          <option value="circle">{t('In a circle')}</option>
          <option value="grid">{t('In a grid')}</option>
        </select>
        <Button onPress={() => void props.change(() => call('map.layout', { map_id: map.id, how: layout }), t('Arranged the nodes.'))} isDisabled={dis || nodes.length === 0} data-testid="arrange">{t('Arrange')}</Button>
      </div>
    </section>
  );
}

/** The chosen node: its words, what it is joined to, and a form to join it to another. */
function NodePanel(props: {
  map: ConceptMap;
  node: MapNode;
  nodes: MapNode[];
  links: MapLink[];
  labelOf: (id: number) => string;
  readOnly: boolean;
  busy: boolean;
  change: ChangeFn;
  remove: () => void;
}) {
  const { map, node, links } = props;
  const [words, setWords] = useState(node.label);
  const [target, setTarget] = useState('');
  const [relation, setRelation] = useState('');
  const [directed, setDirected] = useState(true);
  const [editing, setEditing] = useState<{ id: number; label: string; directed: boolean } | null>(null);
  const dis = props.readOnly || props.busy;
  const mine = links.filter((l) => l.from_node === node.id || l.to_node === node.id);

  useEffect(() => {
    setWords(node.label);
    setTarget('');
    setRelation('');
    setEditing(null);
  }, [node.id, node.label]);

  return (
    <div>
      <p className="muted">
        {node.kind === 'note' ? t('A note: words of your own.') : node.missing ? t('This {kind} no longer exists.', { kind: node.kind }) : t('Stands for the {value} of this name; if the name changes, it changes here.', { value: KIND_WORDS[node.kind] })}
      </p>
      {node.kind === 'note' ? (
        <div className="field">
          <label htmlFor="cm-words">{t('The words of the note')}</label>
          <input id="cm-words" type="text" value={words} onChange={(e) => setWords(e.target.value)} />
          <Button onPress={() => void props.change(() => call('map.node_note', { id: node.id, label: words }), t('Changed the note.'))} isDisabled={dis || words.trim() === '' || words === node.label} data-testid="save-note">{t('Save the words')}</Button>
        </div>
      ) : null}
      {node.kind === 'code' && node.ref_id !== null && !node.missing ? (
        <p>
          <Button
            className="bk-link"
            onPress={() => {
              selectCode(node.ref_id!, { openDetails: false });
              goTo('codebook');
            }}
          >{t('Open this code in the Codebook Editor')}</Button>
        </p>
      ) : null}
      <p>
        <Button onPress={props.remove} isDisabled={dis} data-testid="remove-node">{t('Take this node off the map')}</Button>
      </p>

      <h4 className="bk-h3">{t('Its links')}</h4>
      {mine.length === 0 ? (
        <p className="muted">{t('Not joined to anything yet.')}</p>
      ) : (
        <ul className="cm-list" aria-label={t('Links of this node')}>
          {mine.map((l) => (
            <li key={l.id} data-testid="map-link-row">
              {editing?.id === l.id ? (
                <span className="cm-edit">
                  <input aria-label={t('Words of the link: {link}', { link: linkSentence({ from: props.labelOf(l.from_node), to: props.labelOf(l.to_node), label: l.label, directed: l.directed }) })} type="text" value={editing.label} onChange={(e) => setEditing({ ...editing, label: e.target.value })} />
                  <Checkbox isSelected={editing.directed} onChange={(v) => setEditing({ ...editing, directed: v })} className="check">
                    <Box />{t('Has a direction')}</Checkbox>
                  <Button
                    onPress={() => void props.change(() => call('map.link_update', { id: l.id, label: editing.label, directed: editing.directed }), t('Changed the link.')).then(() => setEditing(null))}
                    isDisabled={dis}
                    data-testid="save-link"
                  >{t('Save')}</Button>
                  <Button onPress={() => setEditing(null)}>{t('Cancel')}</Button>
                </span>
              ) : (
                <>
                  <span>{linkSentence({ from: props.labelOf(l.from_node), to: props.labelOf(l.to_node), label: l.label, directed: l.directed })}</span>{' '}
                  <Button onPress={() => setEditing({ id: l.id, label: l.label, directed: l.directed })} isDisabled={dis} aria-label={t('Change the link: {link}', { link: linkSentence({ from: props.labelOf(l.from_node), to: props.labelOf(l.to_node), label: l.label, directed: l.directed }) })}>{t('Change')}</Button>{' '}
                  <Button
                    onPress={() => void props.change(() => call('map.link_delete', { id: l.id }), t('Removed the link.'))}
                    isDisabled={dis}
                    aria-label={t('Remove the link: {link}', { link: linkSentence({ from: props.labelOf(l.from_node), to: props.labelOf(l.to_node), label: l.label, directed: l.directed }) })}
                  >{t('Remove')}</Button>
                </>
              )}
            </li>
          ))}
        </ul>
      )}

      <h4 className="bk-h3">{t('Join it to another node')}</h4>
      <div className="field">
        <label htmlFor="cm-target">{t('Join {label} to', { label: node.label })}</label>
        <select id="cm-target" value={target} onChange={(e) => setTarget(e.target.value)}>
          <option value="">{t('Choose a node')}</option>
          {props.nodes
            .filter((n) => n.id !== node.id)
            .map((n) => (
              <option key={n.id} value={n.id}>
                {n.label} ({KIND_WORDS[n.kind]})
              </option>
            ))}
        </select>
      </div>
      <div className="field">
        <label htmlFor="cm-relation">{t('The words on the link')}</label>
        <input id="cm-relation" type="text" value={relation} onChange={(e) => setRelation(e.target.value)} placeholder={t('for example leads to')} />
      </div>
      <Checkbox isSelected={directed} onChange={setDirected} className="check">
        <Box />{t('The link has a direction, from {label} to the other node', { label: node.label })}</Checkbox>
      <p>
        <Button
          onPress={() => void props.change(() => call('map.link_add', { map_id: map.id, from: node.id, to: Number(target), label: relation, directed }), t('Joined the two nodes.')).then(() => setRelation(''))}
          isDisabled={dis || target === ''}
          data-testid="add-link"
        >{t('Add the link')}</Button>
      </p>
    </div>
  );
}
