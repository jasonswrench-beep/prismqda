import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import { Button, Input, Label, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { type Choices, LEVELS, LEVEL_LABELS, ROLE_LABELS, choicesFromSuggestion, mappingFromChoices, setLevel, setRole, validateChoices, type ColumnRole } from '../lib/importMapping';
import { STATUS_LABELS, alreadyQueued, fileNameOf, importBlocker, outcomeLine, tally, toImport, toRetry, type DuplicateAction, type ItemStatus, type QueueItem } from '../lib/importQueue';
import { browseManyFiles, listenForFileDrops, nativeDialogsAvailable, cleanTypedPath, type ImportKindId } from '../pickFile';
import { announce } from '../state/announce';
import { goTo, refreshCases, refreshSources, reviseDocument, selectSource } from '../state/actions';
import { appStore, useApp } from '../state/app';
import type { ImportSummary, InspectResult, RevisionOutcome, SourceType, UnitKind } from '../types';
import { Icon, type IconName } from './Icon';
import { Tick } from './Tick';
import { Notice, SOURCE_TYPE_LABELS, UNIT_KIND_LABELS, plural } from './common';
import { num, rich, t, tn } from '../i18n';
import { engineText } from '../i18n/engine';

const ROLES: ColumnRole[] = ['unused', 'id', 'unit_id', 'text', 'attribute', 'unit_attribute'];
const SOURCE_TYPES: SourceType[] = ['document', 'transcript', 'survey', 'social', 'pdf', 'other'];
/** What the Type column says for a file, from the kind of file it was read as and how many sources it holds. */
const FORMAT_LABELS: Record<string, (count: number) => string> = {
  ris: (n) => tn(n, { one: 'Reference', other: 'References' }),
  bibtex: (n) => tn(n, { one: 'Reference', other: 'References' }),
  'csl-json': (n) => tn(n, { one: 'Reference', other: 'References' }),
  'prism-family-exchange': (n) => tn(n, { one: 'Library source', other: 'Library sources' }),
  eml: (n) => tn(n, { one: 'E-mail message', other: 'E-mail messages' }),
  mbox: (n) => tn(n, { one: 'E-mail message', other: 'E-mail messages' }),
  mhtml: (n) => tn(n, { one: 'Saved web page', other: 'Saved web pages' }),
  feed: (n) => tn(n, { one: 'News item', other: 'News items' }),
  image: (n) => tn(n, { one: 'Picture', other: 'Pictures' }),
};
function typeLabel(first: { source_type: string; format?: string | null }, count: number): string {
  const kinds = first.format ? FORMAT_LABELS[first.format] : undefined;
  if (kinds) return count === 1 ? kinds(1) : t('{kind} ({count})', { kind: kinds(count), count });
  return SOURCE_TYPE_LABELS[first.source_type] ?? first.source_type;
}

const UNIT_KINDS: UnitKind[] = ['sentence', 'paragraph', 'speaker_turn', 'survey_response', 'post', 'comment'];

/** The six ways in that the book shows. The ones whose importers do not exist yet are shown, and say so. */
const KINDS: { id: ImportKindId | null; label: string; hint: string; icon: IconName; built: boolean }[] = [
  { id: 'documents', get label() { return t('Documents'); }, get hint() { return t('PDF, DOCX, TXT, and spreadsheets'); }, icon: 'src-text', built: true },
  { id: 'media', get label() { return t('Audio / Video'); }, get hint() { return t('MP3, MP4, WAV; captions optional'); }, icon: 'src-video', built: true },
  { id: 'image', get label() { return t('Images'); }, get hint() { return t('PNG, JPG, GIF, WebP, BMP'); }, icon: 'src-image', built: true },
  { id: 'reference', get label() { return t('Reference Library'); }, get hint() { return t('RIS, BibTeX, CSL JSON'); }, icon: 'src-reference', built: true },
  { id: 'web', get label() { return t('Web / News'); }, get hint() { return t('Saved pages and feeds'); }, icon: 'src-web', built: true },
  { id: 'email', get label() { return t('Email'); }, get hint() { return t('EML, MBOX'); }, icon: 'src-email', built: true },
];

/** What is chosen for the next files that are added, and for each file once it is in the queue. */
interface Options {
  folder: string;
  caseName: string;
  sourceType: SourceType | '';
  unitKind: UnitKind | '';
  interviewers: string;
  /** Pictures: which of the camera's facts to keep as variables of a case made for the picture. */
  pictureWhen: boolean;
  pictureCamera: boolean;
  picturePlace: boolean;
}

interface Item extends QueueItem {
  options: Options;
  inspect: InspectResult | null;
  summary: ImportSummary | null;
  choices: Choices;
  result: { names: string[]; sourceIds: number[]; units: number; report: ImportSummary; revision?: RevisionOutcome & { backup: string } } | null;
}

const EMPTY: Options = { folder: '', caseName: '', sourceType: '', unitKind: '', interviewers: '', pictureWhen: false, pictureCamera: false, picturePlace: false };

const engineOptions = (o: Options) => ({
  ...(o.folder.trim() ? { folder: o.folder.trim() } : {}),
  ...(o.sourceType ? { source_type: o.sourceType } : {}),
  ...(o.unitKind ? { unit_kind: o.unitKind } : {}),
  ...(o.interviewers.trim() ? { interviewers: o.interviewers.split(',').map((s) => s.trim()).filter(Boolean) } : {}),
  ...(o.pictureWhen || o.pictureCamera || o.picturePlace ? { picture_variables: { when: o.pictureWhen, camera: o.pictureCamera, place: o.picturePlace } } : {}),
});

/** A spreadsheet whose columns still need a decision counts as needing one; otherwise it is ready. */
function shownStatus(i: Item): ItemStatus {
  if (i.status === 'ready' && i.inspect?.kind === 'table' && validateChoices(i.choices)) return 'needs-choice';
  return i.status;
}

/**
 * Import Data: add files (drop them, choose them, or type a path), see what each one is and what will come in, say where
 * it goes, and import. Each file in the queue is its own item with its own outcome, so one that cannot be read does not
 * stop the others, and trying it again never imports what is already in.
 */
export function ImportPage() {
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const cases = useApp((s) => s.cases);
  const [items, setItems] = useState<Item[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [path, setPath] = useState('');
  const [defaults, setDefaults] = useState<Options>(EMPTY);
  const [notice, setNotice] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const [busy, setBusy] = useState(false);
  const nextId = useRef(1);
  const itemsRef = useRef<Item[]>([]);
  itemsRef.current = items;
  const pathBox = useRef<HTMLInputElement>(null);

  const patch = (id: number, change: Partial<Item>) => setItems((all) => all.map((i) => (i.id === id ? { ...i, ...change } : i)));

  /** Reads a file to see what is in it, and says what it found (the screen may not have caught up yet, so the caller gets it too). */
  const inspectItem = async (id: number, file: string, options: Options): Promise<Partial<Item>> => {
    let found: Partial<Item>;
    try {
      const r = await call('import.inspect', { path: file, ...engineOptions(options) });
      const table = r.kind === 'table';
      found = {
        inspect: r,
        summary: table ? null : r.summary,
        choices: table ? choicesFromSuggestion(r.table, r.suggested_mapping) : {},
        status: r.duplicate_of !== null ? 'duplicate' : 'ready',
        error: undefined,
      };
    } catch (e) {
      found = { status: 'failed', error: toApiError(e).message };
    }
    patch(id, found);
    return found;
  };

  /** Puts files in the queue and reads each one. */
  const add = (paths: string[]) => {
    const fresh: Item[] = [];
    const known = [...itemsRef.current];
    for (const raw of paths) {
      const file = cleanTypedPath(raw);
      if (file === '' || alreadyQueued(known, file) || fresh.some((f) => f.path === file)) continue;
      fresh.push({ id: nextId.current++, path: file, status: 'checking', duplicate: 'skip', options: { ...defaults }, inspect: null, summary: null, choices: {}, result: null });
    }
    if (fresh.length === 0) {
      if (paths.length > 0) setNotice(t('That file is already in the queue.'));
      return;
    }
    setNotice(null);
    setItems((all) => [...all, ...fresh]);
    // The preview shows the file that was just added.
    setSelected(fresh[0].id);
    announce(t('{files} added to the import queue.', { files: tn(fresh.length, { one: '{count} file', other: '{count} files' }) }));
    for (const f of fresh) void inspectItem(f.id, f.path, f.options);
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (cleanTypedPath(path) === '') {
      setNotice(t('Type the path of the file to import, for example C:\\Data\\interview-1.docx'));
      return;
    }
    add([path]);
    setPath('');
  };

  // Files dropped on the window (the desktop window says where they are).
  useEffect(() => {
    let off: (() => void) | undefined;
    let live = true;
    void listenForFileDrops((paths) => add(paths), setOver).then((fn) => {
      if (live) off = fn;
      else fn();
    });
    return () => {
      live = false;
      off?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [defaults]);

  const chooseFiles = async (kind: ImportKindId = 'documents') => {
    if (!nativeDialogsAvailable) {
      pathBox.current?.focus();
      return;
    }
    const files = await browseManyFiles(kind);
    if (files.length > 0) add(files);
  };

  /** Gives each new source its case, making the case first if it is new. */
  const assignCase = async (name: string, sourceIds: number[]) => {
    const wanted = name.trim();
    if (wanted === '' || sourceIds.length === 0) return;
    let c = appStore.get().cases.find((x) => x.name.toLowerCase() === wanted.toLowerCase());
    if (!c) c = await call('case.add', { name: wanted });
    for (const id of sourceIds) await call('case.assign', { case_id: c.id, source_id: id });
    await refreshCases();
  };

  const runItem = async (item: Item, again = false) => {
    patch(item.id, { status: 'importing', error: undefined });
    try {
      const chosen = item.path;
      const r = await call('import.run', {
        path: chosen,
        ...engineOptions(item.options),
        ...(item.inspect?.kind === 'table' ? { mapping: mappingFromChoices(item.inspect.table, item.choices) } : {}),
        ...(again || item.inspect?.duplicate_of != null ? { force: true } : {}),
      });
      await refreshSources();
      const made = appStore.get().sources.filter((s) => r.outcome.source_ids.includes(s.id));
      await assignCase(item.options.caseName, r.outcome.source_ids);
      patch(item.id, { status: 'complete', result: { names: made.map((s) => s.name), sourceIds: r.outcome.source_ids, units: r.outcome.units_created, report: r.summary } });
      return true;
    } catch (e) {
      const err = toApiError(e);
      patch(item.id, { status: err.code === 'duplicate' ? 'duplicate' : 'failed', error: err.message });
      return false;
    }
  };

  const importAll = async () => {
    const todo = toImport(itemsRef.current.map((i) => ({ ...i, status: shownStatus(i) })) as Item[]).map((q) => itemsRef.current.find((i) => i.id === q.id)!);
    if (todo.length === 0) return;
    setBusy(true);
    for (const item of todo) await runItem(item);
    setBusy(false);
    const counts = tally(itemsRef.current);
    announce(t('Import finished. {outcome}', { outcome: outcomeLine(counts) }));
  };

  const retryFailed = async () => {
    setBusy(true);
    for (const q of toRetry(itemsRef.current)) {
      const item = itemsRef.current.find((i) => i.id === q.id)!;
      if (!item.inspect) {
        patch(item.id, { status: 'checking', error: undefined });
        const found = await inspectItem(item.id, item.path, item.options);
        if (found.status === 'ready') await runItem({ ...item, ...found } as Item);
      } else await runItem(item);
    }
    setBusy(false);
    announce(t('Tried again. {outcome}', { outcome: outcomeLine(tally(itemsRef.current)) }));
  };

  const setDuplicate = (id: number, action: DuplicateAction) => {
    const item = itemsRef.current.find((i) => i.id === id);
    if (!item) return;
    if (action === 'skip') {
      patch(id, { duplicate: 'skip', status: 'skipped' });
      announce(t('{file} will be skipped.', { file: fileNameOf(item.path) }));
    } else {
      patch(id, { duplicate: 'again' });
      void (async () => {
        setBusy(true);
        await runItem({ ...item, duplicate: 'again' }, true);
        setBusy(false);
      })();
    }
  };

  /** Takes this file as the new version of a document that is already in the project. */
  const reviseItem = async (item: Item, sourceId: number) => {
    setBusy(true);
    patch(item.id, { status: 'importing', error: undefined });
    const r = await reviseDocument(sourceId, { path: item.path, ...engineOptions(item.options) });
    setBusy(false);
    if ('problem' in r) {
      patch(item.id, { status: 'ready', error: r.problem });
      announce(r.problem, 'assertive');
      return;
    }
    const name = appStore.get().sources.find((s) => s.id === sourceId)?.name ?? t('document {id}', { id: sourceId });
    patch(item.id, {
      status: 'complete',
      error: undefined,
      result: { names: [name], sourceIds: [sourceId], units: r.result.outcome.units_added, report: r.result.summary, revision: { ...r.result.outcome, backup: r.result.backup } },
    });
    announce(t('{name} now has the text of {file}. {revision}', { name, file: fileNameOf(item.path), revision: revisionLine(r.result.outcome) }));
  };

  const remove = (id: number) => {
    setItems((all) => all.filter((i) => i.id !== id));
    setSelected((cur) => (cur === id ? null : cur));
  };

  const shown = items.map((i) => ({ ...i, status: shownStatus(i) }));
  const current = shown.find((i) => i.id === selected) ?? shown[0] ?? null;
  const blocker = readOnly ? t('This project is read only.') : importBlocker(shown);
  const counts = tally(items);
  const finished = items.length > 0 && counts.waiting === 0 && !busy;
  const created = items.flatMap((i) => i.result?.sourceIds ?? []);

  const done = () => {
    goTo('code');
    const last = created[created.length - 1];
    if (last !== undefined) void selectSource(last);
  };

  return (
    <main id="main-content" tabIndex={-1} className="view-page bk-import-page" aria-labelledby="h-import">
      <h2 id="h-import">{t('Import Data')}</h2>
      <p className="help">{t('PrismQDA can read Word, OpenDocument, RTF, PDF with text, HTML, Markdown, plain text, caption files, and spreadsheets (CSV, TSV, XLSX). Each file is read first, so you see what will come in before anything is added to the project.')}</p>
      {notice ? <Notice>{notice}</Notice> : null}

      <section aria-labelledby="h-add" className="bk-import-add">
        <h3 id="h-add" className="visually-hidden">{t('Add files')}</h3>
        <div className={`bk-dropzone${over ? ' over' : ''}`} data-testid="dropzone">
          <Icon name="cmd-import" size={44} className="bk-drop-icon" />
          <p className="bk-drop-title">{nativeDialogsAvailable ? t('Drag and drop files here') : t('Add files by typing their path')}</p>
          <p className="muted">{nativeDialogsAvailable ? t('or click to browse') : t('A browser cannot tell PrismQDA where a dropped file is. The desktop window can.')}</p>
          <Button className="bk-link" onPress={() => void chooseFiles()}>
            {nativeDialogsAvailable ? t('Choose files…') : t('Go to the path box')}
          </Button>
        </div>

        <form onSubmit={onSubmit} noValidate className="bk-import-form">
          <TextField value={path} onChange={setPath} className="field">
            <Label>{t('File to import')}</Label>
            <Input ref={pathBox} placeholder={t('C:\\Data\\interview-1.docx')} />
            <Text slot="description">{t('Type the full path of the file, then press Check this file.')}</Text>
          </TextField>
          <details className="advanced">
            <summary>{t('More choices')}</summary>
            <TextField value={defaults.folder} onChange={(v) => setDefaults({ ...defaults, folder: v })} className="field">
              <Label>{t('Put it in a folder called')}</Label>
              <Input />
              <Text slot="description">{t('Folders group documents in the list. Leave empty for none.')}</Text>
            </TextField>
            <TextField value={defaults.caseName} onChange={(v) => setDefaults({ ...defaults, caseName: v })} className="field">
              <Label>{t('Add it to the case')}</Label>
              <Input list="import-cases" />
              <Text slot="description">{t('A case is a person, site, or group. Type a new name to make one.')}</Text>
              <datalist id="import-cases">
                {cases.map((c) => (
                  <option key={c.id} value={c.name} />
                ))}
              </datalist>
            </TextField>
            <div className="field">
              <label htmlFor="imp-type">{t('Kind of material')}</label>
              <select id="imp-type" value={defaults.sourceType} onChange={(e) => setDefaults({ ...defaults, sourceType: e.target.value as SourceType | '' })}>
                <option value="">{t('Work it out from the file')}</option>
                {SOURCE_TYPES.map((k) => (
                  <option key={k} value={k}>
                    {SOURCE_TYPE_LABELS[k]}
                  </option>
                ))}
              </select>
              <p className="help">{t('Choose Transcript to split a document into speaker turns.')}</p>
            </div>
            <div className="field">
              <label htmlFor="imp-unit">{t('Split into units by')}</label>
              <select id="imp-unit" value={defaults.unitKind} onChange={(e) => setDefaults({ ...defaults, unitKind: e.target.value as UnitKind | '' })}>
                <option value="">{t('The usual choice for this kind of material')}</option>
                {UNIT_KINDS.map((k) => (
                  <option key={k} value={k}>
                    {UNIT_KIND_LABELS[k]}
                  </option>
                ))}
              </select>
            </div>
            <TextField value={defaults.interviewers} onChange={(v) => setDefaults({ ...defaults, interviewers: v })} className="field">
              <Label>{t('Interviewer names in a transcript')}</Label>
              <Input />
              <Text slot="description">{t('Separate names with commas. Their turns are left out of coding.')}</Text>
            </TextField>
            <p className="muted">{t('These choices apply to the files you add from now on.')}</p>
          </details>
          <div className="actions">
            <Button type="submit" className="primary" isDisabled={readOnly}>{t('Check this file')}</Button>
            <Button type="button" isDisabled={readOnly} onPress={() => appStore.set({ dialog: { kind: 'import-folder' } })}>{t('Import a whole folder instead')}</Button>
          </div>
        </form>
      </section>

      <section aria-labelledby="h-kinds" className="bk-import-kinds">
        <h3 id="h-kinds" className="visually-hidden">{t('Kinds of material')}</h3>
        <ul>
          {KINDS.map((k) => (
            <li key={k.label}>
              <Button className="bk-kind" isDisabled={!k.built} onPress={() => void chooseFiles(k.id ?? 'documents')} aria-describedby={`kind-${k.label.replace(/\W/g, '')}`}>
                <Icon name={k.icon} size={32} className="bk-kind-icon" />
                <span className="bk-kind-name">{k.label}</span>
                <span className="bk-kind-hint">({k.hint})</span>
              </Button>
              <span id={`kind-${k.label.replace(/\W/g, '')}`} className="visually-hidden">
                {k.built ? t('Choose {label} files to import.', { label: k.label.toLowerCase() }) : t('Not built yet.')}
              </span>
            </li>
          ))}
        </ul>
        <p className="muted bk-kinds-note" data-testid="kinds-note">{t('Web / News reads pages that a browser saved whole (MHTML), HTML files, and news feeds saved as RSS or Atom files. It does not fetch an address, because PrismQDA makes no network requests of its own: save the page or the feed first. Audio and video come in through the captions beside them: PrismQDA does not turn speech into text. Images are not built yet.')}</p>
      </section>

      <section aria-labelledby="h-queue" className="bk-import-queue">
        <h3 id="h-queue">{t('Import Queue ({count})', { count: items.length })}</h3>
        {items.length === 0 ? (
          <p className="muted">{t('Nothing is queued. Add a file above and it appears here with what it is and where it stands.')}</p>
        ) : (
          <div className="table-wrap" tabIndex={-1}>
            <table className="data bk-queue" data-testid="import-queue">
              <caption className="visually-hidden">{t('Files waiting to be imported')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('File')}</th>
                  <th scope="col">{t('Type')}</th>
                  <th scope="col">{t('Status')}</th>
                  <th scope="col">
                    <span className="visually-hidden">{t('Actions')}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {shown.map((i) => {
                  const first = i.summary?.sources[0] ?? null;
                  const type = i.inspect?.kind === 'table' ? t('Spreadsheet') : first ? typeLabel(first, i.summary?.sources.length ?? 1) : '';
                  return (
                    <tr key={i.id} className={i.id === current?.id ? 'current' : undefined}>
                      <th scope="row">
                        <Button className="bk-link bk-queue-file" onPress={() => setSelected(i.id)} aria-pressed={i.id === current?.id} aria-label={i.id === current?.id ? t('{file}, shown below', { file: fileNameOf(i.path) }) : t('{file}: show it below', { file: fileNameOf(i.path) })}>
                          <Icon name="src-text" size={16} />
                          {fileNameOf(i.path)}
                        </Button>
                      </th>
                      <td>{type || <span className="muted">…</span>}</td>
                      <td>
                        <span className={`bk-qstatus s-${i.status}`} data-status={i.status}>
                          {STATUS_LABELS[i.status]}
                        </span>
                        {i.error ? <span className="bk-qstatus-why"> {i.error}</span> : null}
                      </td>
                      <td className="bk-queue-actions">
                        {i.status === 'failed' ? (
                          <Button className="bk-link" onPress={() => void retryOne(i.id)} isDisabled={busy}>{t('Try again')}</Button>
                        ) : null}
                        {i.status !== 'complete' && i.status !== 'importing' ? (
                          <Button className="bk-link" onPress={() => remove(i.id)} aria-label={t('Remove {file} from the queue', { file: fileNameOf(i.path) })}>{t('Remove')}</Button>
                        ) : null}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {current && current.status !== 'checking' && current.status !== 'failed' && current.status !== 'complete' ? (
        <Preview item={current} patch={(change) => patch(current.id, change)} onDuplicate={(a) => setDuplicate(current.id, a)} cases={cases.map((c) => c.name)} onRevise={(id) => void reviseItem(current, id)} busy={busy} />
      ) : null}

      {current && current.status === 'failed' ? (
        <Notice>
          {t('{file} could not be imported. {error}', { file: fileNameOf(current.path), error: current.error ?? '' })}
        </Notice>
      ) : null}

      <div className="actions bk-import-go">
        <Button className="primary" onPress={() => void importAll()} isDisabled={blocker !== null || busy}>
          {busy ? t('Importing…') : t('Import')}
        </Button>
        {blocker && items.length > 0 ? <span className="muted bk-why">{blocker}</span> : null}
      </div>

      {finished ? (
        <section aria-labelledby="h-done" className="bk-import-done" data-testid="import-finished">
          <h3 id="h-done">{t('Import finished')}</h3>
          <p role="status" data-testid="import-outcome">
            {outcomeLine(counts)}
          </p>
          <ul className="bk-outcomes">
            {items
              .filter((i) => i.result)
              .map((i) => (
                <li key={i.id}>
                  {i.result!.revision ? (
                    <>{rich('Replaced the text of <strong>{value}</strong> with {file}. {revision} A backup copy of the project was saved first ({file2}).', { value: i.result!.names[0], file: fileNameOf(i.path), revision: revisionLine(i.result!.revision), file2: fileNameOf(i.result!.revision.backup), strong: (inside) => <strong>{inside}</strong> })}</>
                  ) : (
                    <>{rich('Imported {sources} with {units} from {file}: <strong>{list}</strong>.', { sources: tn(i.result!.names.length, { one: '{count} source', other: '{count} sources' }), units: tn(i.result!.units, { one: '{count} unit', other: '{count} units' }), file: fileNameOf(i.path), list: i.result!.names.join(', '), strong: (inside) => <strong>{inside}</strong> })}</>
                  )}
                </li>
              ))}
          </ul>
          {items.map((i) => (i.result ? <Report key={i.id} summary={i.result.report} /> : null))}
          <div className="actions">
            <Button className="primary" onPress={done}>{t('Done')}</Button>
            {counts.failed > 0 ? (
              <Button onPress={() => void retryFailed()} isDisabled={busy}>{t('Retry the files that failed')}</Button>
            ) : null}
            <Button
              onPress={() => {
                setItems((all) => all.filter((i) => i.status !== 'complete' && i.status !== 'skipped'));
                setSelected(null);
                pathBox.current?.focus();
              }}
            >{t('Import another file')}</Button>
          </div>
        </section>
      ) : null}
    </main>
  );

  async function retryOne(id: number) {
    const item = itemsRef.current.find((i) => i.id === id);
    if (!item) return;
    setBusy(true);
    if (!item.inspect) {
      patch(id, { status: 'checking', error: undefined });
      const found = await inspectItem(id, item.path, item.options);
      if (found.status === 'ready') await runItem({ ...item, ...found } as Item);
    } else await runItem(item);
    setBusy(false);
  }
}

// ---------------------------------------------------------------- the preview of the chosen file

function Preview({ item, patch, onDuplicate, cases, onRevise, busy }: { item: Item & { status: ItemStatus }; patch: (change: Partial<Item>) => void; onDuplicate: (a: DuplicateAction) => void; cases: string[]; onRevise: (sourceId: number) => void; busy: boolean }) {
  const inspect = item.inspect;
  const duplicate = inspect?.duplicate_of ?? null;
  const duplicateName = useMemo(() => {
    if (duplicate === null) return '';
    return appStore.get().sources.find((s) => s.id === duplicate)?.name ?? t('source {duplicate}', { duplicate });
  }, [duplicate]);
  const [asking, setAsking] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const setOpt = (change: Partial<Options>) => patch({ options: { ...item.options, ...change } });
  if (!inspect) return null;

  const previewTable = async () => {
    if (inspect.kind !== 'table') return;
    const missing = validateChoices(item.choices);
    setProblem(missing);
    if (missing) return;
    setAsking(true);
    try {
      const r = await call('import.inspect', { path: item.path, ...engineOptions(item.options), mapping: mappingFromChoices(inspect.table, item.choices) });
      if (r.kind === 'table') patch({ summary: r.summary });
    } catch (e) {
      patch({ error: toApiError(e).message });
    } finally {
      setAsking(false);
    }
  };

  return (
    <section aria-labelledby="h-preview" className="bk-import-preview">
      <h3 id="h-preview">{t('Preview: {file}', { file: fileNameOf(item.path) })}</h3>
      <p className="path-line">
        {rich('<strong>File:</strong> <span>{path}</span>', { path: item.path, strong: (inside) => <strong>{inside}</strong>, span: (inside) => <span className="mono">{inside}</span> })}
      </p>

      {duplicate !== null ? (
        <div className="bk-duplicate" data-testid="duplicate-box">
          <Notice kind="warning">{rich('This exact file was already imported as <strong>{duplicateName}</strong>. A file with the same content counts as a duplicate even if its name is different. Importing it again makes a second copy, with its own codings.', { duplicateName, strong: (inside) => <strong>{inside}</strong> })}</Notice>
          <div className="actions">
            <Button onPress={() => onDuplicate('skip')} isDisabled={item.status === 'importing'}>{t('Skip this file')}</Button>
            <Button onPress={() => onDuplicate('again')} isDisabled={item.status === 'importing'}>{t('Import again')}</Button>
          </div>
        </div>
      ) : null}

      {inspect.kind === 'table' ? (
        <section aria-labelledby="imp-columns">
          <h4 id="imp-columns">{t('Tell PrismQDA what each column is')}</h4>
          <p className="help">{t('This spreadsheet has {rows}. Pick the column that identifies each person, the columns with the words people wrote, and the columns that describe the people. PrismQDA made a first guess.', { rows: tn(inspect.table.row_count, { one: '{count} row', other: '{count} rows' }) })}</p>
          {problem ? <Notice>{problem}</Notice> : validateChoices(item.choices) ? <Notice kind="info">{validateChoices(item.choices)}</Notice> : null}
          {inspect.warnings.length > 0 ? (
            <ul className="warning-list">
              {inspect.warnings.map((w, i) => (
                <li key={i}>{engineText(w.message)}</li>
              ))}
            </ul>
          ) : null}
          <div className="table-wrap" tabIndex={-1}>
            <table className="data">
              <caption>{t('Columns in the spreadsheet')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Column')}</th>
                  <th scope="col">{t('What it is for')}</th>
                  <th scope="col">{t('Kind of values')}</th>
                  <th scope="col">{t('Examples')}</th>
                </tr>
              </thead>
              <tbody>
                {inspect.table.columns.map((col) => {
                  const c = item.choices[col.name];
                  const needsLevel = c?.role === 'attribute' || c?.role === 'unit_attribute';
                  return (
                    <tr key={col.name}>
                      <th scope="row">{col.name}</th>
                      <td>
                        <select aria-label={t('What column {name} is for', { name: col.name })} value={c?.role ?? 'unused'} onChange={(e) => patch({ choices: setRole(item.choices, col.name, e.target.value as ColumnRole) })}>
                          {ROLES.map((r) => (
                            <option key={r} value={r}>
                              {ROLE_LABELS[r]}
                            </option>
                          ))}
                        </select>
                      </td>
                      <td>
                        {needsLevel ? (
                          <select aria-label={t('Kind of values in column {name}', { name: col.name })} value={c.level} onChange={(e) => patch({ choices: setLevel(item.choices, col.name, e.target.value as (typeof LEVELS)[number]) })}>
                            {LEVELS.map((l) => (
                              <option key={l} value={l}>
                                {LEVEL_LABELS[l]}
                              </option>
                            ))}
                          </select>
                        ) : (
                          <span className="muted">{col.looks_like === 'number' ? 'numbers' : col.looks_like}</span>
                        )}
                      </td>
                      <td className="samples">{col.samples.slice(0, 2).join(' | ') || <span className="muted">{t('empty')}</span>}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <div className="actions">
            <Button onPress={() => void previewTable()} isDisabled={asking}>{t('Show what will be imported')}</Button>
          </div>
        </section>
      ) : null}

      {inspect.kind === 'batch' && inspect.media && inspect.media.captions === null ? (
        <p className="bk-recording" data-testid="recording-alone-note">
          {inspect.media.kind === 'video'
            ? rich('<strong>Recording:</strong> {file_name} (video). There are no captions beside it, and PrismQDA does not turn speech into text, so it comes in as a document of its own, with a description for you to write. You code it by stretches of time: mark where a stretch starts and ends, and give it a code. The recording stays where it is; PrismQDA does not copy it into the project. To code the words as well, put a WebVTT (.vtt) or SubRip (.srt) caption file with the same name beside it and import it again.', { file_name: inspect.media.file_name, strong: (inside) => <strong>{inside}</strong> })
            : rich('<strong>Recording:</strong> {file_name} (audio). There are no captions beside it, and PrismQDA does not turn speech into text, so it comes in as a document of its own, with a description for you to write. You code it by stretches of time: mark where a stretch starts and ends, and give it a code. The recording stays where it is; PrismQDA does not copy it into the project. To code the words as well, put a WebVTT (.vtt) or SubRip (.srt) caption file with the same name beside it and import it again.', { file_name: inspect.media.file_name, strong: (inside) => <strong>{inside}</strong> })}</p>
      ) : null}

      {inspect.kind === 'batch' && inspect.media && inspect.media.captions !== null ? (
        <p className="bk-recording" data-testid="recording-note">
          {inspect.media.kind === 'video'
            ? rich('<strong>Recording:</strong> {file_name} (video). The words below are read from {captions}, and the recording is linked to the document so the text can follow it. The recording stays where it is; PrismQDA does not copy it into the project.', { file_name: inspect.media.file_name, captions: inspect.media.captions, strong: (inside) => <strong>{inside}</strong> })
            : rich('<strong>Recording:</strong> {file_name} (audio). The words below are read from {captions}, and the recording is linked to the document so the text can follow it. The recording stays where it is; PrismQDA does not copy it into the project.', { file_name: inspect.media.file_name, captions: inspect.media.captions, strong: (inside) => <strong>{inside}</strong> })}</p>
      ) : null}

      {inspect.kind === 'batch' && item.summary?.sources[0]?.format === 'image' ? (
        <p className="bk-recording" data-testid="picture-note">
          {rich('<strong>Picture:</strong> the file is kept inside the project exactly as it is, so the project can be copied or sent without it. Once it is in, write a description of it (the words a screen reader speaks), then draw rectangles, ellipses, or outlines over parts of it and give them codes. PrismQDA keeps PNG, JPEG, GIF, WebP, and BMP; save a TIFF or HEIC picture as a PNG or JPEG first.', { strong: (inside) => <strong>{inside}</strong> })}</p>
      ) : null}

      {inspect.kind === 'batch' && item.summary?.sources[0]?.format === 'image' && inspect.picture && (inspect.picture.taken || inspect.picture.camera || inspect.picture.place) ? (
        <fieldset className="bk-picture-facts" data-testid="picture-facts">
          <legend>{t('What the camera wrote into this picture')}</legend>
          <p className="help">{t('Any of it can be kept as variables of a case named for this picture, so pictures can be compared by when or with what they were taken. Nothing is kept unless you tick it.')}</p>
          {inspect.picture.taken ? <Tick isSelected={item.options.pictureWhen} onChange={(on) => setOpt({ pictureWhen: on })} testId="picture-when">{t('When it was taken: {when}', { when: inspect.picture.taken })}</Tick> : null}
          {inspect.picture.camera ? <Tick isSelected={item.options.pictureCamera} onChange={(on) => setOpt({ pictureCamera: on })} testId="picture-camera">{t('What it was taken with: {camera}', { camera: inspect.picture.camera })}</Tick> : null}
          {inspect.picture.place ? <Tick isSelected={item.options.picturePlace} onChange={(on) => setOpt({ picturePlace: on })} testId="picture-place">{t('Where it was taken, as latitude and longitude. A precise place can show where a person lives, so keep it only if your study needs it and your ethics approval allows it.')}</Tick> : null}
        </fieldset>
      ) : null}

      {item.summary ? <SummaryView summary={item.summary} /> : inspect.kind === 'table' ? <p className="help">{t('Use "Show what will be imported" to preview the result before importing.')}</p> : null}

      {inspect.kind === 'batch' && !(inspect.media && inspect.media.captions === null) && (item.summary?.sources.length ?? 1) === 1 && item.summary?.sources[0]?.format !== 'image' ? <ReviseSection fileName={fileNameOf(item.path)} onRevise={onRevise} disabled={busy || item.status === 'importing'} /> : null}

      <div className="bk-import-where">
        <TextField value={item.options.folder} onChange={(v) => setOpt({ folder: v })} className="field">
          <Label>{t('Destination folder')}</Label>
          <Input />
          <Text slot="description">{t('The folder in Documents that the new document goes into. Empty means none.')}</Text>
        </TextField>
        <TextField value={item.options.caseName} onChange={(v) => setOpt({ caseName: v })} className="field">
          <Label>{t('Case')}</Label>
          <Input list="preview-cases" />
          <Text slot="description">{t('Assign the new document to a case. Type a new name to make one.')}</Text>
          <datalist id="preview-cases">
            {cases.map((c) => (
              <option key={c} value={c} />
            ))}
          </datalist>
        </TextField>
      </div>
    </section>
  );
}

function SummaryView({ summary }: { summary: ImportSummary }) {
  const first = summary.sources[0];
  return (
    <section aria-labelledby="imp-summary">
      <h4 id="imp-summary">{t('What will come in')}</h4>
      <div className="table-wrap" tabIndex={-1}>
        <table className="data">
          <caption>{t('Documents that will be made')}</caption>
          <thead>
            <tr>
              <th scope="col">{t('Name')}</th>
              <th scope="col">{t('Kind')}</th>
              <th scope="col">{t('Characters')}</th>
              <th scope="col">{t('Units')}</th>
              <th scope="col">{t('Formatting kept')}</th>
            </tr>
          </thead>
          <tbody>
            {summary.sources.map((s, i) => (
              <tr key={i}>
                <th scope="row">{s.name}</th>
                <td>{SOURCE_TYPE_LABELS[s.source_type] ?? s.source_type}</td>
                <td>{num(s.characters)}</td>
                <td>
                  {tn(s.units, { one: '{count} ({value})', other: '{count} ({value}s)' }, { value: UNIT_KIND_LABELS[s.default_unit_kind] ?? s.default_unit_kind })}</td>
                <td>{num(s.format_runs)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {summary.cases > 0 ? <p>{t('{cases} will be made, with {attributes}.', { cases: tn(summary.cases, { one: '{count} case', other: '{count} cases' }), attributes: tn(summary.case_attributes, { one: '{count} attribute', other: '{count} attributes' }) })}</p> : null}
      {first?.preview ? (
        <>
          <h5 id="imp-preview-h" className="bk-h3">{t('The first words of {name}', { name: first.name })}
          </h5>
          <pre className="bk-preview" aria-labelledby="imp-preview-h" tabIndex={0} data-testid="import-preview-text">
            {first.preview}
            {first.characters > first.preview.length ? '…' : ''}
          </pre>
        </>
      ) : null}
      <Warnings summary={summary} />
    </section>
  );
}

function Warnings({ summary }: { summary: ImportSummary }) {
  const all = summary.sources.flatMap((s) => s.warnings.map((w) => ({ source: s.name, ...w })));
  if (all.length === 0) return <p className="help">{t('No warnings.')}</p>;
  return (
    <div>
      <h4>{t('Warnings ({count})', { count: all.length })}</h4>
      <ul className="warning-list">
        {all.map((w, i) => (
          <li key={i}>
            {engineText(w.message)}
            {w.location ? <span className="muted"> ({w.location})</span> : null}
            {summary.sources.length > 1 ? <span className="muted"> {t('in {source}', { source: w.source })}</span> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Report({ summary }: { summary: ImportSummary }) {
  return (
    <section aria-label={t('Import report for {name}', { name: summary.sources[0]?.name ?? t('the file') })}>
      <Warnings summary={summary} />
    </section>
  );
}

// ---------------------------------------------------------------- a newer version of a document

/** What a revision did to the codings, in a sentence. */
function revisionLine(o: RevisionOutcome): string {
  const parts = [tn(o.codings_moved, { one: '{count} coding moved with its words', other: '{count} codings moved with their words' })];
  parts.push(o.codings_removed === 0 ? t('none was removed') : tn(o.codings_removed, { one: '{codings} was removed because its words are not in the new text (History lists it)', other: '{codings} were removed because their words are not in the new text (History lists them)' }, { codings: tn(o.codings_removed, { one: '{count} coding', other: '{count} codings' }) }));
  if (o.units_added > 0) parts.push(t('{new_units} added', { new_units: tn(o.units_added, { one: '{count} new unit', other: '{count} new units' }) }));
  if (o.proposals_rejected > 0) parts.push(t('{waiting_suggestions} turned down', { waiting_suggestions: tn(o.proposals_rejected, { one: '{count} waiting suggestion', other: '{count} waiting suggestions' }) }));
  return `${parts.join('; ')}.`;
}

/**
 * Offers to take the file as the new version of a document already in the project, instead of adding a second copy with none of
 * the coding. It says what will happen before it does it, because Undo cannot take it back; a backup copy is the way back.
 */
function ReviseSection(props: { fileName: string; onRevise: (sourceId: number) => void; disabled: boolean }) {
  const sources = useApp((s) => s.sources);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [chosen, setChosen] = useState('');
  const [sure, setSure] = useState(false);
  const target = sources.find((s) => String(s.id) === chosen) ?? null;
  return (
    <section className="bk-revise" aria-labelledby="h-revise" data-testid="revise-section">
      <h4 id="h-revise">{t('A newer version of a document you already have?')}</h4>
      <p className="help">{t('Use this file as the new version of that document, instead of adding a second copy with none of its coding. The document keeps its codings, memos, case, and attributes. Its text becomes this file’s text, and each coding follows the words that survive. A coding whose words are not in the new text is removed, and History lists it. A backup copy of the project is saved first, because Undo cannot take this back.')}</p>
      <div className="field">
        <label htmlFor="rev-doc">{t('The document this file is a newer version of')}</label>
        <select
          id="rev-doc"
          value={chosen}
          onChange={(e) => {
            setChosen(e.target.value);
            setSure(false);
          }}
        >
          <option value="">{t('Choose a document…')}</option>
          {sources.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
              {s.codings > 0 ? ` (${tn(s.codings, { one: '{count} coding', other: '{count} codings' })})` : ''}
            </option>
          ))}
        </select>
      </div>
      {sure && target ? (
        <div className="bk-revise-sure" data-testid="revise-sure">
          <Notice kind="warning">{target.codings > 0
            ? rich('Replace the text of <strong>{name}</strong> with {fileName}? Its {codings} will follow the words that survive. This cannot be undone with Undo; the backup copy is the way back.', { name: target.name, fileName: props.fileName, codings: tn(target.codings, { one: '{count} coding', other: '{count} codings' }), strong: (inside) => <strong>{inside}</strong> })
            : rich('Replace the text of <strong>{name}</strong> with {fileName}? This cannot be undone with Undo; the backup copy is the way back.', { name: target.name, fileName: props.fileName, strong: (inside) => <strong>{inside}</strong> })}</Notice>
          <div className="actions">
            <Button className="primary" onPress={() => props.onRevise(target.id)} isDisabled={props.disabled} data-testid="revise-confirm">{t('Replace the text')}</Button>
            <Button onPress={() => setSure(false)}>{t('Cancel')}</Button>
          </div>
        </div>
      ) : (
        <div className="actions">
          <Button onPress={() => setSure(true)} isDisabled={target === null || props.disabled || readOnly} data-testid="revise-ask">{t('Use this file as the new version')}</Button>
          {readOnly ? <span className="muted">{t('This project is read only.')}</span> : null}
        </div>
      )}
    </section>
  );
}
