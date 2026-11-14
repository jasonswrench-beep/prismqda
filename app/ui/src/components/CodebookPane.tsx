import { useMemo, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { Button, Collection, Input, Label, Menu, MenuItem, MenuSection, MenuTrigger, Popover, SearchField, Separator, Toolbar, Tree, TreeItem, TreeItemContent, type Key } from 'react-aria-components';
import { slotOf } from '../lib/favorites';
import { abbreviations } from '../lib/labels';
import { interpretCodebookKey } from '../lib/shortcuts';
import { buildCodeTree, filterCodeTree, planDrop, planMove, type CodeNode, type MoveKind, type MoveResult } from '../lib/tree';
import { announce } from '../state/announce';
import { applyCode, goTo, moveCode, restoreCode, retireCode, selectCode, toggleActiveCode, toggleFavorite } from '../state/actions';
import { appStore, useApp } from '../state/app';
import { toApiError } from '../bridge';
import type { CodeRow } from '../types';
import { Icon } from './Icon';
import { beginPointerDrag } from './pointerDrag';
import { plural } from './common';
import { t, tn } from '../i18n';

/**
 * The number beside a code: the codings of the code itself and of every code beneath it, so a group shows the total of
 * its branch and a leaf shows its own. Retired and merged codes keep their codings and count too.
 */
export function subtreeUsage(node: CodeNode): number {
  return node.code.usage + node.children.reduce((sum, child) => sum + subtreeUsage(child), 0);
}

export function CodebookPane() {
  const codes = useApp((s) => s.codes);
  const showInactive = useApp((s) => s.showInactive);
  const favorites = useApp((s) => s.favorites);
  const selectedCodeId = useApp((s) => s.selectedCodeId);
  const chosen = useApp((s) => s.activeCodeIds);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [query, setQuery] = useState('');
  const [searching, setSearching] = useState(false);
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set());

  const labels = useMemo(() => abbreviations(codes), [codes]);
  const tree = useMemo(() => buildCodeTree(codes, { showInactive }), [codes, showInactive]);
  const filtered = useMemo(() => filterCodeTree(tree, query, (id) => labels.get(id) ?? ''), [tree, query, labels]);
  const selected = codes.find((c) => c.id === selectedCodeId) ?? null;

  const parents = useMemo(() => {
    const ids = new Set<number>();
    const walk = (nodes: readonly CodeNode[]) => {
      for (const n of nodes) {
        if (n.children.length > 0) ids.add(n.code.id);
        walk(n.children);
      }
    };
    walk(filtered.nodes);
    return ids;
  }, [filtered]);

  const expandedKeys = useMemo(() => {
    const open = new Set<Key>();
    for (const id of parents) if (query.trim() !== '' ? true : !collapsed.has(id)) open.add(id);
    return open;
  }, [parents, collapsed, query]);

  const runMove = async (id: number, result: MoveResult) => {
    if (!result.ok) {
      announce(result.reason, 'assertive');
      return;
    }
    try {
      await moveCode(id, result.plan);
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };

  const reorganize = (kind: MoveKind) => {
    if (!selected) return announce(t('Select a code first.'), 'assertive');
    void runMove(selected.id, planMove(codes, selected.id, kind, showInactive));
  };

  // A code is dragged with the pointer, not with the browser's drag and drop, which the Windows window does not pass to the page
  // (components/pointerDrag.ts, decision 169). The keys and the menu are the way for the keyboard and a screen reader.
  const startDrag = !readOnly
    ? (e: ReactPointerEvent<HTMLElement>, code: CodeRow) =>
        beginPointerDrag(e, {
          label: code.name,
          target: 'data-key',
          // the top quarter of a row is before it, the bottom quarter after it, and the middle on it (a sub-code of it)
          mark: (place, ev) => {
            const r = place.getBoundingClientRect();
            const f = r.height > 0 ? (ev.clientY - r.top) / r.height : 0.5;
            return f < 0.25 ? 'before' : f > 0.75 ? 'after' : 'on';
          },
          onDrop: (key, where) => {
            if (Number(key) === code.id) return;
            void runMove(code.id, planDrop(codes, code.id, Number(key), where === 'before' || where === 'after' ? where : 'on'));
          },
        })
    : undefined;

  const onKeyDownCapture = (e: KeyboardEvent<HTMLDivElement>) => {
    const kind = interpretCodebookKey(e.nativeEvent);
    if (!kind) return;
    const row = (e.target as HTMLElement).closest('[data-key]');
    const key = row?.getAttribute('data-key');
    const id = key ? Number(key) : selectedCodeId;
    if (id === null || id === undefined || Number.isNaN(id)) return;
    e.preventDefault();
    e.stopPropagation();
    void runMove(id, planMove(codes, id, kind, showInactive));
  };

  const renderNode = (node: CodeNode): React.ReactElement => {
    const c = node.code;
    const fav = slotOf(favorites, c.id);
    const status = c.status === 'active' ? '' : c.status === 'retired' ? t('retired') : t('merged into another code');
    const shown = chosen.includes(c.id);
    const name = [c.name, status, tn(c.usage, { one: '{count} coding', other: '{count} codings' }), fav ? t('favorite {fav}', { fav }) : '', shown ? t('its passages are shown') : ''].filter(Boolean).join(', ');
    return (
      <TreeItem key={c.id} id={c.id} textValue={c.name} aria-label={name} className={`code-item status-${c.status}${node.onlyAsParent ? ' only-parent' : ''}`}>
        <TreeItemContent>
          {({ hasChildItems }) => (
            <div className="code-row" onPointerDownCapture={startDrag ? (e) => startDrag(e, c) : undefined}>
              {hasChildItems ? (
                <Button slot="chevron" className="chevron">
                  <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true" focusable="false">
                    <path d="M4 2l4 4-4 4" fill="none" stroke="currentColor" strokeWidth="1.8" />
                  </svg>
                </Button>
              ) : (
                <span className="chevron-space" aria-hidden="true" />
              )}
              <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: c.color }} />
              <span className={`code-name${node.children.length > 0 ? ' group' : ''}`}>{c.name}</span>
              <span className="code-count" aria-hidden="true">
                ({subtreeUsage(node)})
              </span>
              {shown ? <Icon name="code-activated" size={14} className="shown-mark" /> : null}
              {c.kind === 'theme' ? <span className="tag">{t('theme')}</span> : null}
              {c.status !== 'active' ? <span className="tag">{c.status}</span> : null}
              {fav ? (
                <span className="tag fav" aria-hidden="true">{t('key {fav}', { fav })}
                </span>
              ) : null}
            </div>
          )}
        </TreeItemContent>
        <Collection items={node.children}>{renderNode}</Collection>
      </TreeItem>
    );
  };

  const runMenu = (id: string) => {
    if (id === 'version') return appStore.set({ dialog: { kind: 'snapshot' } });
    if (id === 'qdc') return appStore.set({ dialog: { kind: 'import-qdc' } });
    if (id === 'theme') return appStore.set({ dialog: { kind: 'new-theme', codeIds: [] } });
    if (id === 'themes') return goTo('themes');
    if (id === 'show' || id === 'inactive') return;
    if (!selected) return announce(t('Select a code first.'), 'assertive');
    switch (id) {
      case 'apply':
        return void applyCode(selected.id);
      case 'definition':
        return goTo('codebook');
      case 'sub':
        return appStore.set({ dialog: { kind: 'new-code', parentId: selected.id } });
      case 'pin':
        return toggleFavorite(selected.id);
      case 'up':
      case 'down':
      case 'indent':
      case 'outdent':
        return reorganize(id);
      case 'merge':
        return appStore.set({ dialog: { kind: 'merge', codeId: selected.id } });
      case 'split':
        return appStore.set({ dialog: { kind: 'split', codeId: selected.id } });
      case 'retire':
        return void retireCode(selected.id).catch((e) => announce(toApiError(e).message, 'assertive'));
      case 'restore':
        return void restoreCode(selected.id).catch((e) => announce(toApiError(e).message, 'assertive'));
    }
  };

  const canEdit = !readOnly;
  return (
    <section id="codebook-section" tabIndex={-1} className="pane-part codebook bk-card" aria-labelledby="h-codebook">
      <div className="bk-pane-head">
        <Icon name="code-code" size={20} className="bk-pane-icon" />
        <h2 id="h-codebook">{t('Codes')}</h2>
        <span className="visually-hidden">{tn(codes.filter((c) => c.status === 'active').length, { one: '{count} code', other: '{count} codes' })}</span>
        <Toolbar aria-label={t('Codes actions')} className="toolbar-inline">
          <Button className="icon-button" aria-label={t('Add code')} isDisabled={!canEdit} onPress={() => appStore.set({ dialog: { kind: 'new-code', parentId: null } })}>
            <Icon name="ui-plus" size={18} />
          </Button>
          <Button
            className="icon-button"
            aria-label={t('Search codes')}
            aria-pressed={searching || query !== ''}
            onPress={() => {
              if (searching && query === '') setSearching(false);
              else setSearching(true);
            }}
          >
            <Icon name="cmd-search" size={18} />
          </Button>
          <MenuTrigger>
            <Button className="icon-button" aria-label={t('More code actions')}>
              <Icon name="ui-more" size={18} />
            </Button>
            <Popover className="popover" placement="bottom end">
              <Menu className="menu" aria-label={t('More code actions')} onAction={(key) => runMenu(String(key))}>
                <MenuItem id="apply" className="menu-item" isDisabled={!selected}>{t('Apply to selection')}</MenuItem>
                <MenuItem id="definition" className="menu-item" isDisabled={!selected}>{t('Open definition…')}</MenuItem>
                <MenuItem id="sub" className="menu-item" isDisabled={!canEdit || !selected}>{t('Add sub-code')}</MenuItem>
                <MenuItem id="pin" className="menu-item" isDisabled={!selected}>
                  {selected && slotOf(favorites, selected.id) ? t('Unpin favorite') : t('Pin as favorite')}
                </MenuItem>
                <Separator className="menu-separator" />
                <MenuSection
                  selectionMode="multiple"
                  shouldCloseOnSelect
                  selectedKeys={new Set<Key>([...(selected && chosen.includes(selected.id) ? ['show'] : []), ...(showInactive ? ['inactive'] : [])])}
                  onSelectionChange={(keys) => {
                    const now = keys === 'all' ? new Set<Key>(['show', 'inactive']) : keys;
                    if (selected && now.has('show') !== chosen.includes(selected.id)) toggleActiveCode(selected.id);
                    if (now.has('inactive') !== showInactive) appStore.set({ showInactive: now.has('inactive') });
                  }}
                >
                  <MenuItem id="show" className="menu-item" isDisabled={!selected}>{t('Show passages of this code')}</MenuItem>
                  <MenuItem id="inactive" className="menu-item">{t('Show retired and merged codes')}</MenuItem>
                </MenuSection>
                <Separator className="menu-separator" />
                <MenuItem id="up" className="menu-item" isDisabled={!canEdit || !selected}>{t('Move up')}</MenuItem>
                <MenuItem id="down" className="menu-item" isDisabled={!canEdit || !selected}>{t('Move down')}</MenuItem>
                <MenuItem id="indent" className="menu-item" isDisabled={!canEdit || !selected}>{t('Make sub-code')}</MenuItem>
                <MenuItem id="outdent" className="menu-item" isDisabled={!canEdit || !selected}>{t('Move out')}</MenuItem>
                <Separator className="menu-separator" />
                <MenuItem id="merge" className="menu-item" isDisabled={!canEdit || !selected || selected.status !== 'active'}>{t('Merge…')}</MenuItem>
                <MenuItem id="split" className="menu-item" isDisabled={!canEdit || !selected || selected.status !== 'active'}>{t('Split…')}</MenuItem>
                {selected && selected.status === 'retired' ? (
                  <MenuItem id="restore" className="menu-item" isDisabled={!canEdit}>{t('Restore')}</MenuItem>
                ) : (
                  <MenuItem id="retire" className="menu-item" isDisabled={!canEdit || !selected || selected.status !== 'active'}>{t('Retire')}</MenuItem>
                )}
                <Separator className="menu-separator" />
                <MenuItem id="version" className="menu-item" isDisabled={!canEdit}>{t('Save version…')}</MenuItem>
                <MenuItem id="theme" className="menu-item" isDisabled={!canEdit}>{t('Add theme…')}</MenuItem>
                <MenuItem id="themes" className="menu-item">{t('Open the Themes page')}</MenuItem>
                <MenuItem id="qdc" className="menu-item" isDisabled={!canEdit}>{t('Import a codebook…')}</MenuItem>
              </Menu>
            </Popover>
          </MenuTrigger>
        </Toolbar>
      </div>
      <p className="visually-hidden" id="codebook-help">{t('Select text, then double-click a code or press Enter on it to apply it. Press Alt with an arrow key to reorganize.')}</p>

      {searching || query !== '' ? (
        <SearchField value={query} onChange={setQuery} className="field search" aria-label={t('Find a code')} autoFocus>
          <Label className="visually-hidden">{t('Find a code')}</Label>
          <Input placeholder={t('Find a code')} />
          <Button aria-label={t('Clear the search')}>×</Button>
        </SearchField>
      ) : null}

      <div className="tree-wrap" onKeyDownCapture={onKeyDownCapture}>
        {filtered.nodes.length === 0 ? (
          <p className="empty">{codes.length === 0 ? t('No codes yet. Add one with "Add code", or select text and press I to make a code from the words.') : t('No code matches that search.')}</p>
        ) : (
          <Tree
            aria-label={t('Codes')}
            aria-describedby="codebook-help"
            className="code-tree"
            items={filtered.nodes}
            // The rows show which codes are favorites and which have their passages shown; tell the tree to redraw them when those change.
            dependencies={[chosen, favorites]}
            selectionMode="single"
            selectionBehavior="replace"
            selectedKeys={selectedCodeId === null ? [] : [selectedCodeId]}
            onSelectionChange={(keys) => {
              const k = [...keys][0];
              if (k !== undefined) selectCode(Number(k));
            }}
            expandedKeys={expandedKeys}
            onExpandedChange={(keys) => {
              if (query.trim() !== '') return;
              const next = new Set<number>();
              for (const id of parents) if (!keys.has(id)) next.add(id);
              setCollapsed(next);
            }}
            onAction={(key) => void applyCode(Number(key))}
          >
            {renderNode}
          </Tree>
        )}
      </div>
    </section>
  );
}
