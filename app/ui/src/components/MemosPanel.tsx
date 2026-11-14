import { useCallback, useEffect, useState } from 'react';
import { Button, Input, Label, TextArea, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { formatWhen } from '../lib/eventWording';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import type { MemoKind, MemoRow, MemoScope } from '../types';
import { Notice, plural } from './common';
import { t, tn } from '../i18n';

export const MEMO_KINDS: { id: MemoKind; label: string }[] = [
  { id: 'analytic', get label() { return t('Analytic'); } },
  { id: 'reflexive', get label() { return t('Reflexive journal'); } },
  { id: 'method_decision', get label() { return t('Method decision'); } },
  { id: 'todo', get label() { return t('To-do'); } },
];

export function memoKindLabel(kind: string): string {
  return MEMO_KINDS.find((k) => k.id === kind)?.label ?? kind;
}

interface PanelProps {
  /** `null` lists every memo in the project. */
  scope: MemoScope | null;
  targetId: number | null;
  /** Plain words for what the memos are about, such as "the code Welcome". */
  about: string;
  idPrefix: string;
}

/** The memos for one thing (or all of them), with a form to add one and editing in place. */
/** The words with a name in them when there is a name, else the words alone. */
const namedOr = (withName: string, without: string, name?: string): string => (name ? withName : without);

export function MemosPanel(props: PanelProps) {
  const { scope, targetId } = props;
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const codes = useApp((s) => s.codes);
  const sources = useApp((s) => s.sources);
  const [memos, setMemos] = useState<MemoRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [kind, setKind] = useState<MemoKind>('analytic');
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');

  const load = useCallback(async () => {
    try {
      const args = scope === null ? {} : { scope, ...(targetId !== null ? { target_id: targetId } : {}) };
      setMemos(await call('memo.list', args));
      setError(null);
    } catch (e) {
      setError(toApiError(e).message);
    }
  }, [scope, targetId]);

  useEffect(() => {
    setMemos(null);
    void load();
  }, [load]);

  const add = async () => {
    if (scope === null) return;
    if (!title.trim() && !body.trim()) return setError(t('Write a title or some text for the memo first.'));
    try {
      await call('memo.add', { kind, scope, target_id: scope === 'project' ? null : targetId, title: title.trim(), body });
      setTitle('');
      setBody('');
      announce(title.trim() ? t('Added the memo {title}.', { title: title.trim() }) : t('Added the memo.'));
      await load();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  const aboutWhat = (m: MemoRow): string => {
    switch (m.scope) {
      case 'project':
        return t('the project');
      case 'source':
        return namedOr(t('the source {name}', { name: sources.find((s) => s.id === m.target_id)?.name ?? '' }), t('the source'), sources.find((s) => s.id === m.target_id)?.name);
      case 'code':
        return namedOr(t('the code {name}', { name: codes.find((c) => c.id === m.target_id)?.name ?? '' }), t('the code'), codes.find((c) => c.id === m.target_id)?.name);
      case 'coding':
        return (m.target_id ?? 0) < 0 ? t('an area of a picture or a stretch of a recording') : t('a coding');
      default:
        return m.scope;
    }
  };

  const canAdd = scope !== null && !readOnly && (scope === 'project' || targetId !== null);
  return (
    <div className="memos" data-testid="memos-panel">
      {error ? <Notice>{error}</Notice> : null}
      {canAdd ? (
        <form
          className="memo-form"
          onSubmit={(e) => {
            e.preventDefault();
            void add();
          }}
        >
          <h3>{t('Add a memo about {about}', { about: props.about })}</h3>
          <div className="field">
            <label htmlFor={`${props.idPrefix}-kind`}>{t('Kind of memo')}</label>
            <select id={`${props.idPrefix}-kind`} value={kind} onChange={(e) => setKind(e.target.value as MemoKind)}>
              {MEMO_KINDS.map((k) => (
                <option key={k.id} value={k.id}>
                  {k.label}
                </option>
              ))}
            </select>
          </div>
          <TextField value={title} onChange={setTitle} className="field">
            <Label>{t('Title')}</Label>
            <Input />
          </TextField>
          <TextField value={body} onChange={setBody} className="field">
            <Label>{t('Memo')}</Label>
            <TextArea rows={3} />
          </TextField>
          <Button type="submit" className="primary">{t('Add memo')}</Button>
        </form>
      ) : scope !== null ? (
        <p className="muted">{t('Select something first to add a memo about it.')}</p>
      ) : null}

      <h3 className="memo-count">{memos === null ? t('Loading memos…') : scope === null ? t('{memos} in this project', { memos: tn(memos.length, { one: '{count} memo', other: '{count} memos' }) }) : t('{memos} about {about}', { memos: tn(memos.length, { one: '{count} memo', other: '{count} memos' }), about: props.about })}</h3>
      {memos && memos.length === 0 ? <p className="muted">{t('No memos yet.')}</p> : null}
      <ul className="memo-list">
        {(memos ?? []).map((m) => (
          <MemoItem key={`${m.id}-${m.updated_at}`} memo={m} readOnly={readOnly} showAbout={scope === null ? aboutWhat(m) : null} onChanged={load} idPrefix={props.idPrefix} />
        ))}
      </ul>
    </div>
  );
}

function MemoItem(props: { memo: MemoRow; readOnly: boolean; showAbout: string | null; onChanged: () => Promise<void>; idPrefix: string }) {
  const { memo } = props;
  const [title, setTitle] = useState(memo.title);
  const [body, setBody] = useState(memo.body);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const dirty = title !== memo.title || body !== memo.body;

  const save = async () => {
    try {
      await call('memo.update', { id: memo.id, title, body });
      announce(t('Saved the memo.'));
      await props.onChanged();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };
  const remove = async () => {
    try {
      await call('memo.delete', { id: memo.id });
      announce(t('Deleted the memo.'));
      await props.onChanged();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  return (
    <li className="memo-item" data-memo-title={memo.title}>
      <div className="memo-meta">
        <span className="tag">{memoKindLabel(memo.kind)}</span>
        {props.showAbout ? <span className="muted">{t('about {showAbout}', { showAbout: props.showAbout })}</span> : null}
        <span className="muted">{t('Updated {when}', { when: formatWhen(memo.updated_at) })}</span>
      </div>
      {error ? <Notice>{error}</Notice> : null}
      <TextField value={title} onChange={setTitle} className="field" isReadOnly={props.readOnly}>
        <Label>{t('Title')}</Label>
        <Input />
      </TextField>
      <TextField value={body} onChange={setBody} className="field" isReadOnly={props.readOnly}>
        <Label>{t('Memo')}</Label>
        <TextArea rows={3} />
      </TextField>
      {props.readOnly ? null : (
        <div className="actions">
          <Button onPress={() => void save()} isDisabled={!dirty}>{t('Save memo')}</Button>
          {confirming ? (
            <>
              <Button className="danger" onPress={() => void remove()}>{t('Yes, delete this memo')}</Button>
              <Button onPress={() => setConfirming(false)}>{t('Keep it')}</Button>
            </>
          ) : (
            <Button onPress={() => setConfirming(true)}>{t('Delete…')}</Button>
          )}
        </div>
      )}
    </li>
  );
}

/** Which thing the Memos area is about: the coding, code, or source that is selected, or the project. */
export type MemoAbout = 'coding' | 'code' | 'source' | 'project' | 'all';

export function MemosArea(props: { allowAll: boolean; idPrefix: string }) {
  const selectedCodingId = useApp((s) => s.selectedCodingId);
  const selectedCodeId = useApp((s) => s.selectedCodeId);
  const currentSourceId = useApp((s) => s.currentSourceId);
  const codes = useApp((s) => s.codes);
  const sources = useApp((s) => s.sources);
  const [choice, setChoice] = useState<MemoAbout | null>(null);

  const options: { id: MemoAbout; label: string; available: boolean }[] = [
    { id: 'coding', label: t('The selected coding'), available: selectedCodingId !== null },
    { id: 'code', label: selectedCodeId !== null ? t('The selected code: {name}', { name: codes.find((c) => c.id === selectedCodeId)?.name ?? '' }) : t('The selected code'), available: selectedCodeId !== null },
    { id: 'source', label: currentSourceId !== null ? t('The open source: {name}', { name: sources.find((s) => s.id === currentSourceId)?.name ?? '' }) : t('The open source'), available: currentSourceId !== null },
    { id: 'project', label: t('The whole project'), available: true },
  ];
  if (props.allowAll) options.push({ id: 'all', label: t('Every memo in the project'), available: true });

  const first = options.find((o) => o.available)!.id;
  const effective = choice && options.find((o) => o.id === choice && o.available) ? choice : first;

  let scope: MemoScope | null = null;
  let target: number | null = null;
  let about = t('the project');
  if (effective === 'coding') {
    scope = 'coding';
    target = selectedCodingId;
    about = t('this coding');
  } else if (effective === 'code') {
    scope = 'code';
    target = selectedCodeId;
    about = namedOr(t('the code {name}', { name: codes.find((c) => c.id === selectedCodeId)?.name ?? '' }), t('the code'), codes.find((c) => c.id === selectedCodeId)?.name);
  } else if (effective === 'source') {
    scope = 'source';
    target = currentSourceId;
    about = namedOr(t('the source {name}', { name: sources.find((s) => s.id === currentSourceId)?.name ?? '' }), t('the source'), sources.find((s) => s.id === currentSourceId)?.name);
  } else if (effective === 'project') {
    scope = 'project';
  }

  return (
    <div>
      <div className="field">
        <label htmlFor={`${props.idPrefix}-about`}>{t('Memos about')}</label>
        <select id={`${props.idPrefix}-about`} value={effective} onChange={(e) => setChoice(e.target.value as MemoAbout)}>
          {options.map((o) => (
            <option key={o.id} value={o.id} disabled={!o.available}>
              {o.label}
            </option>
          ))}
        </select>
      </div>
      <MemosPanel scope={scope} targetId={target} about={about} idPrefix={props.idPrefix} />
    </div>
  );
}

export function openMemos(): void {
  appStore.set({ workspace: 'synthesize' });
}
