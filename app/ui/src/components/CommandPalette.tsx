import { useEffect, useMemo, useState } from 'react';
import { Autocomplete, Dialog, Header, Heading, Input, ListBox, ListBoxItem, ListBoxSection, Modal, ModalOverlay, SearchField, type Key } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { openAt } from '../state/actions';
import { appStore, useApp } from '../state/app';
import { COMMANDS, GROUP_LABELS, commandMatches, runCommand } from '../state/commands';
import type { SearchHit } from '../types';
import { Icon } from './Icon';
import { plural } from './common';
import { t, tn } from '../i18n';

const MAX_HITS = 20;

/**
 * Ctrl+K: one box that finds a command by its name and finds words in the text of every source. A person who
 * does not know where something lives types what they want and gets either the command or the passage.
 */
export function CommandPalette({ mode }: { mode?: 'commands' | 'search' }) {
  const state = useApp((s) => s);
  const [q, setQ] = useState('');
  const [result, setResult] = useState<{ query: string; hits: SearchHit[]; total: number; truncated: boolean } | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  // Close only if this box is still the open dialog: a command it just ran may have opened another one.
  const close = () => appStore.set((s) => (s.dialog?.kind === 'palette' ? { dialog: null } : {}));
  const searchOnly = mode === 'search';
  const typed = q.trim();

  const commands = useMemo(() => (searchOnly ? [] : COMMANDS.filter((c) => commandMatches(c, typed))), [searchOnly, typed]);

  // Search the text a moment after typing stops. A phrase the engine cannot read is explained, not ignored.
  useEffect(() => {
    if (typed.length < 2) {
      setResult(null);
      setProblem(null);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(async () => {
      try {
        const r = await call('search.run', { query: typed, limit: MAX_HITS });
        if (cancelled) return;
        setResult({ query: typed, hits: r.hits, total: r.total, truncated: r.truncated });
        setProblem(null);
      } catch (e) {
        if (cancelled) return;
        setResult(null);
        setProblem(toApiError(e).message);
      }
    }, 220);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [typed]);

  const hits = result && result.query === typed ? result.hits : [];

  const onAction = (key: Key) => {
    const id = String(key);
    if (id.startsWith('cmd-')) {
      const c = COMMANDS.find((x) => `cmd-${x.id}` === id);
      close();
      if (c) queueMicrotask(() => void runCommand(c));
    } else if (id.startsWith('hit-')) {
      const h = hits[Number(id.slice(4))];
      close();
      if (h) void openAt(h.source_id, h.spans[0]?.[0] ?? h.scope_start, h.spans[0]?.[1] ?? h.scope_start);
    }
  };

  let summary = '';
  if (typed.length >= 2) {
    if (problem) summary = problem;
    else if (result && result.query === typed) summary = result.truncated
        ? t('{places} in the text (the first {max} are listed).', { places: tn(result.total, { one: '{count} place', other: '{count} places' }), max: MAX_HITS })
        : t('{places} in the text.', { places: tn(result.total, { one: '{count} place', other: '{count} places' }) });
  }

  return (
    <ModalOverlay isOpen isDismissable onOpenChange={(o) => !o && close()} className="overlay palette-overlay">
      <Modal className="modal palette-modal">
        <Dialog className="dialog palette" aria-label={searchOnly ? t('Search the text') : t('Search or run a command')}>
          <Heading slot="title" className="visually-hidden">
            {searchOnly ? t('Search the text') : t('Search or run a command')}
          </Heading>
          <Autocomplete inputValue={q} onInputChange={setQ} filter={() => true}>
            <SearchField aria-label={searchOnly ? t('Search the text of every source') : t('Search the text, or type a command')} autoFocus className="palette-search">
              <Icon name="cmd-search" size={18} />
              <Input placeholder={searchOnly ? t('Search the text of every source') : t('Search the text, or type a command')} />
            </SearchField>
            <ListBox
              aria-label={t('Results')}
              className="palette-list"
              selectionMode="none"
              onAction={onAction}
              renderEmptyState={() => <div className="menu-empty">{typed === '' ? t('Type a command, or words to find in the text.') : t('Nothing matches. Try other words.')}</div>}
            >
              {hits.length > 0 ? (
                <ListBoxSection className="palette-section">
                  <Header className="palette-header">{t('In the text')}</Header>
                  {hits.map((h, i) => (
                    <ListBoxItem key={`hit-${i}`} id={`hit-${i}`} textValue={`${h.source_name}: ${h.snippet}`} className="palette-item palette-hit">
                      <span className="palette-hit-source">{h.source_name}</span>
                      <span className="palette-hit-snippet">{h.snippet}</span>
                    </ListBoxItem>
                  ))}
                </ListBoxSection>
              ) : null}
              {commands.length > 0 ? (
                <ListBoxSection className="palette-section">
                  <Header className="palette-header">{t('Commands')}</Header>
                  {commands.map((c) => {
                    const reason = c.unavailable(state);
                    return (
                      <ListBoxItem key={c.id} id={`cmd-${c.id}`} textValue={`${c.title}${reason ? `. ${reason}` : ''}`} className={`palette-item${reason ? ' unavailable' : ''}`}>
                        <Icon name={c.icon} size={18} />
                        <span className="palette-title">{c.title}</span>
                        <span className="palette-group">{GROUP_LABELS[c.group]}</span>
                        {c.shortcut ? <kbd>{c.shortcut}</kbd> : null}
                        {reason ? <span className="palette-reason">{reason}</span> : null}
                      </ListBoxItem>
                    );
                  })}
                </ListBoxSection>
              ) : null}
            </ListBox>
          </Autocomplete>
          <p className="palette-summary" role="status">
            {summary}
          </p>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
