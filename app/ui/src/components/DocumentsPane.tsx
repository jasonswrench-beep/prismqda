import { useEffect, useMemo, useState } from 'react';
import { Button, Collection, Input, Label, Menu, MenuItem, MenuTrigger, Popover, SearchField, Toolbar, Tree, TreeItem, TreeItemContent, type Key } from 'react-aria-components';
import { ancestorKeys, buildDocTree, filterDocTree, folderKeys, type DocNode } from '../lib/docTree';
import { sourceIcon } from '../lib/clips';
import { describeCoverage } from '../lib/coverage';
import { goTo, selectSource } from '../state/actions';
import { appStore, useApp } from '../state/app';
import { Icon } from './Icon';
import { plural } from './common';
import { t, tn } from '../i18n';

/**
 * Documents: the project as a tree of folders and documents, each with a count. Choosing a document opens it in the
 * reader (its own tab); choosing a folder opens or closes it and opens nothing. The document open in the reader is the
 * one that is selected here, including when a result or a search opened it.
 */
export function DocumentsPane() {
  const sources = useApp((s) => s.sources);
  const current = useApp((s) => s.currentSourceId);
  const coverage = useApp((s) => s.coverage);
  const projectName = useApp((s) => s.project?.name ?? t('Project'));
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [query, setQuery] = useState('');
  const [searching, setSearching] = useState(false);
  const [closed, setClosed] = useState<ReadonlySet<string>>(new Set());

  const full = useMemo(() => buildDocTree(projectName, sources), [projectName, sources]);
  const shown = useMemo(() => filterDocTree(full, query), [full, query]);
  const searchingNow = query.trim() !== '';

  // The folders that hold the open document are opened, so it can be seen in the tree whichever way it was opened.
  useEffect(() => {
    if (current === null) return;
    const need = ancestorKeys(full, `d${current}`);
    setClosed((prev) => {
      if (!need.some((k) => prev.has(k))) return prev;
      const next = new Set(prev);
      for (const k of need) next.delete(k);
      return next;
    });
  }, [current, full]);

  const expanded = useMemo(() => new Set<Key>(folderKeys(shown).filter((k) => searchingNow || !closed.has(k))), [shown, closed, searchingNow]);

  const toggle = (key: string) =>
    setClosed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  const rows = (node: DocNode): React.ReactElement => {
    const isDoc = node.kind === 'document';
    const cov = node.source ? (coverage[node.source.id] ?? null) : null;
    const label = isDoc ? `${node.label}. ${describeCoverage(cov, node.source!.codings)}` : `${node.label}, ${tn(node.count, { one: '{count} document', other: '{count} documents' })}`;
    return (
      <TreeItem key={node.key} id={node.key} textValue={node.label} aria-label={label} className={`doc-item kind-${node.kind}`}>
        <TreeItemContent>
          {({ hasChildItems }) => (
            <div className="doc-row">
              {hasChildItems ? (
                <Button slot="chevron" className="chevron">
                  <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true" focusable="false">
                    <path d="M4 2l4 4-4 4" fill="none" stroke="currentColor" strokeWidth="1.8" />
                  </svg>
                </Button>
              ) : (
                <span className="chevron-space" aria-hidden="true" />
              )}
              <Icon name={isDoc ? sourceIcon(node.source) : 'src-folder'} size={18} className="doc-icon" />
              <span className="doc-name">{node.label}</span>
              {isDoc ? null : <span className="doc-count" aria-hidden="true">({node.count})</span>}
            </div>
          )}
        </TreeItemContent>
        <Collection items={node.children}>{rows}</Collection>
      </TreeItem>
    );
  };

  const nothing = sources.length === 0;
  return (
    <section id="pane-sources" className="pane-part sources bk-card" aria-labelledby="h-sources" tabIndex={-1}>
      <div className="bk-pane-head">
        <Icon name="src-folder" size={20} className="bk-pane-icon" />
        <h2 id="h-sources">{t('Documents')}</h2>
        <Toolbar aria-label={t('Documents actions')} className="toolbar-inline">
          <Button className="icon-button" aria-label={t('Add documents')} isDisabled={readOnly} onPress={() => goTo('import')}>
            <Icon name="ui-plus" size={18} />
          </Button>
          <Button
            className="icon-button"
            aria-label={t('Search documents')}
            aria-pressed={searching || query !== ''}
            onPress={() => {
              if (searching && query === '') setSearching(false);
              else setSearching(true);
            }}
          >
            <Icon name="cmd-search" size={18} />
          </Button>
          <MenuTrigger>
            <Button className="icon-button" aria-label={t('More document actions')}>
              <Icon name="ui-more" size={18} />
            </Button>
            <Popover className="popover" placement="bottom end">
              <Menu
                className="menu"
                aria-label={t('More document actions')}
                onAction={(key) => {
                  if (key === 'import') goTo('import');
                  else if (key === 'folder') appStore.set({ dialog: { kind: 'import-folder' } });
                  else if (key === 'collapse') setClosed(new Set(folderKeys(full)));
                  else if (key === 'expand') setClosed(new Set());
                }}
              >
                <MenuItem id="import" className="menu-item" isDisabled={readOnly}>{t('Add a file…')}</MenuItem>
                <MenuItem id="folder" className="menu-item" isDisabled={readOnly}>{t('Add a whole folder…')}</MenuItem>
                <MenuItem id="collapse" className="menu-item">{t('Close every folder')}</MenuItem>
                <MenuItem id="expand" className="menu-item">{t('Open every folder')}</MenuItem>
              </Menu>
            </Popover>
          </MenuTrigger>
        </Toolbar>
      </div>
      {searching || query !== '' ? (
        <SearchField value={query} onChange={setQuery} className="field search" aria-label={t('Find a document')} autoFocus>
          <Label className="visually-hidden">{t('Find a document')}</Label>
          <Input placeholder={t('Find a document')} />
          <Button aria-label={t('Clear the search')}>×</Button>
        </SearchField>
      ) : null}
      <div className="tree-wrap">
        {nothing ? (
          <div className="empty">
            <p>{t('No documents yet. Add an interview, a document, or a spreadsheet of survey answers to begin.')}</p>
          </div>
        ) : shown.count === 0 ? (
          <p className="empty">{t('No document matches that search.')}</p>
        ) : (
          <Tree
            aria-label={t('Documents')}
            className="doc-tree"
            items={[shown]}
            dependencies={[coverage, current]}
            selectionMode="single"
            selectionBehavior="replace"
            disabledKeys={folderKeys(shown)}
            disabledBehavior="selection"
            selectedKeys={current === null ? [] : [`d${current}`]}
            onSelectionChange={(keys) => {
              const k = [...keys][0];
              if (typeof k === 'string' && k.startsWith('d')) void selectSource(Number(k.slice(1)));
            }}
            onAction={(key) => {
              if (typeof key === 'string' && (key === 'root' || key.startsWith('f:'))) toggle(key);
              else if (typeof key === 'string' && key.startsWith('d')) void selectSource(Number(key.slice(1)));
            }}
            expandedKeys={expanded}
            onExpandedChange={(keys) => {
              if (searchingNow) return;
              const next = new Set<string>();
              for (const k of folderKeys(shown)) if (!keys.has(k)) next.add(k);
              setClosed(next);
            }}
          >
            {rows}
          </Tree>
        )}
      </div>
    </section>
  );
}
