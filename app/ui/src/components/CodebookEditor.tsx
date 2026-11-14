import { useEffect, useMemo, useState } from 'react';
import { Button, Checkbox, Collection, Input, Label, Menu, MenuItem, MenuTrigger, Popover, Radio, RadioGroup, SearchField, Separator, Tab, TabList, TabPanel, Tabs, Text, TextArea, TextField, Toolbar, Tree, TreeItem, TreeItemContent, type Key } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { describeEvent, formatWhen, type WordingLookups } from '../lib/eventWording';
import { PALETTE, abbreviations, colorName } from '../lib/labels';
import { buildCodeTree, filterCodeTree, pathLabel, planMove, type CodeNode, type MoveKind } from '../lib/tree';
import { editCode, goTo, moveCode, openAt, quoteFromOpenSource, restoreCode, retireCode, selectCode, setAnchor, setCodeColor, setCounterexample } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import type { CodeRow, CodebookVersion, SegmentRow } from '../types';
import { subtreeUsage } from './CodebookPane';
import { Icon } from './Icon';
import { Notice, plural } from './common';
import { compareText, num, rich, t, tn } from '../i18n';

type TabId = 'details' | 'examples' | 'usage' | 'history';

/**
 * The Codebook Editor: the authoritative view of what each code means. The tree of codes is on the left, the chosen code's
 * Code Details, Examples, Usage, and History are on the right, and the toolbar holds the things done to the codebook as a
 * whole (new code, merge, hierarchy, compare versions, export). It shares the chosen code with the Codes pane, so the
 * code you were on is the code you open.
 */
export function CodebookEditor() {
  const codes = useApp((s) => s.codes);
  const selectedId = useApp((s) => s.selectedCodeId);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const anchors = useApp((s) => s.anchors);
  const [tab, setTab] = useState<TabId>('details');
  const [query, setQuery] = useState('');
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set());
  const [exampleCount, setExampleCount] = useState<number | null>(null);

  const labels = useMemo(() => abbreviations(codes), [codes]);
  const tree = useMemo(() => buildCodeTree(codes, { showInactive: true }), [codes]);
  const filtered = useMemo(() => filterCodeTree(tree, query, (id) => labels.get(id) ?? ''), [tree, query, labels]);
  const selected = codes.find((c) => c.id === selectedId) ?? null;
  const activeCount = codes.filter((c) => c.status === 'active').length;

  // The editor always has a code to show when there is one.
  useEffect(() => {
    if (selected === null && codes.length > 0) selectCode((codes.find((c) => c.status === 'active') ?? codes[0]).id, { openDetails: false });
  }, [selected, codes]);

  // How many examples the chosen code has, for the tab's name.
  useEffect(() => {
    if (!selected) {
      setExampleCount(null);
      return;
    }
    let live = true;
    call('segments.list', { code_ids: [selected.id], limit: 5000 })
      .then((r) => live && setExampleCount(r.segments.filter((s) => s.code_id === selected.id && anchors.has(s.coding_id)).length))
      .catch(() => live && setExampleCount(null));
    return () => {
      live = false;
    };
  }, [selected?.id, anchors]);

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
    for (const id of parents) if (query.trim() !== '' || !collapsed.has(id)) open.add(id);
    return open;
  }, [parents, collapsed, query]);

  const reorganize = async (kind: MoveKind) => {
    if (!selected) return;
    const result = planMove(codes, selected.id, kind, true);
    if (!result.ok) {
      announce(result.reason, 'assertive');
      return;
    }
    try {
      await moveCode(selected.id, result.plan);
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };

  const noEdit = readOnly || !selected;
  const canStructure = !readOnly && !!selected && selected.status === 'active';

  const renderNode = (node: CodeNode): React.ReactElement => {
    const c = node.code;
    const status = c.status === 'active' ? '' : c.status === 'retired' ? t('retired') : t('merged into another code');
    return (
      <TreeItem key={c.id} id={c.id} textValue={c.name} aria-label={[c.name, status, tn(subtreeUsage(node), { one: '{count} coding', other: '{count} codings' })].filter(Boolean).join(', ')} className={`code-item status-${c.status}`}>
        <TreeItemContent>
          {({ hasChildItems }) => (
            <div className="code-row">
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
              {c.kind === 'theme' ? <span className="tag">{t('theme')}</span> : null}
              {c.status !== 'active' ? <span className="tag">{c.status}</span> : null}
            </div>
          )}
        </TreeItemContent>
        <Collection items={node.children}>{renderNode}</Collection>
      </TreeItem>
    );
  };

  return (
    <main id="main-content" tabIndex={-1} className="view-page bk-editor-page" aria-labelledby="h-editor">
      <h2 id="h-editor">{t('Codebook Editor')}</h2>
      <Toolbar aria-label={t('Codebook actions')} className="bk-editor-toolbar">
        <Button className="bk-tool" isDisabled={readOnly} onPress={() => appStore.set({ dialog: { kind: 'new-code', parentId: null } })}>
          <Icon name="cmd-new-code" size={20} />
          <span>{t('New Code')}</span>
        </Button>
        <Button
          className="bk-tool"
          isDisabled={noEdit}
          onPress={() => {
            setTab('details');
            window.setTimeout(() => document.getElementById('code-name')?.focus(), 60);
          }}
        >
          <Icon name="ui-edit" size={20} />
          <span>{t('Edit')}</span>
        </Button>
        <Button className="bk-tool" isDisabled={!canStructure} onPress={() => selected && appStore.set({ dialog: { kind: 'merge', codeId: selected.id } })}>
          <Icon name="code-merge" size={20} />
          <span>{t('Merge')}</span>
        </Button>
        <MenuTrigger>
          <Button className="bk-tool" isDisabled={!canStructure} aria-label={t('Hierarchy')}>
            <Icon name="code-parent" size={20} />
            <span>{t('Hierarchy')}</span>
            <Icon name="ui-chevron-down" size={14} />
          </Button>
          <Popover className="popover" placement="bottom start">
            <Menu
              className="menu"
              aria-label={t('Hierarchy')}
              onAction={(key) => {
                if (key === 'sub') appStore.set({ dialog: { kind: 'new-code', parentId: selected?.id ?? null } });
                else if (key === 'split') appStore.set({ dialog: { kind: 'split', codeId: selected!.id } });
                else void reorganize(String(key) as MoveKind);
              }}
            >
              <MenuItem id="sub" className="menu-item">{t('Add sub-code')}</MenuItem>
              <MenuItem id="up" className="menu-item">{t('Move up')}</MenuItem>
              <MenuItem id="down" className="menu-item">{t('Move down')}</MenuItem>
              <MenuItem id="indent" className="menu-item">{t('Make sub-code')}</MenuItem>
              <MenuItem id="outdent" className="menu-item">{t('Move out')}</MenuItem>
              <Separator className="menu-separator" />
              <MenuItem id="split" className="menu-item">{t('Split…')}</MenuItem>
            </Menu>
          </Popover>
        </MenuTrigger>
        <Button className="bk-tool" onPress={() => appStore.set({ dialog: { kind: 'codebook-compare' } })}>
          <Icon name="mix-case-compare" size={20} />
          <span>{t('Compare')}</span>
        </Button>
        <Button className="bk-tool" onPress={() => appStore.set({ dialog: { kind: 'export', what: 'codebook' } })}>
          <Icon name="cmd-export" size={20} />
          <span>{t('Export')}</span>
        </Button>
        <Button className="bk-tool" isDisabled={readOnly} onPress={() => appStore.set({ dialog: { kind: 'snapshot' } })}>
          <Icon name="cmd-snapshot" size={20} />
          <span>{t('Save version')}</span>
        </Button>
      </Toolbar>

      <div className="bk-editor">
        <section className="bk-editor-tree bk-card" aria-labelledby="h-editor-tree">
          <div className="bk-pane-head">
            <Icon name="code-code" size={20} className="bk-pane-icon" />
            <h3 id="h-editor-tree" className="bk-h3">{t('Codebook ({activeCount})', { activeCount })}</h3>
          </div>
          <SearchField value={query} onChange={setQuery} className="field search" aria-label={t('Find a code')}>
            <Label className="visually-hidden">{t('Find a code')}</Label>
            <Input placeholder={t('Find a code')} />
            <Button aria-label={t('Clear the search')}>×</Button>
          </SearchField>
          <p className="muted bk-tree-note" id="editor-tree-note">{t('Counts are codings, and a code with sub-codes counts theirs too.')}</p>
          <div className="tree-wrap">
            {filtered.nodes.length === 0 ? (
              <p className="empty">{codes.length === 0 ? t('No codes yet. New Code makes the first one.') : t('No code matches that search.')}</p>
            ) : (
              <Tree
                aria-label={t('Codebook')}
                aria-describedby="editor-tree-note"
                className="code-tree"
                items={filtered.nodes}
                selectionMode="single"
                selectionBehavior="replace"
                disallowEmptySelection
                selectedKeys={selected ? [selected.id] : []}
                onSelectionChange={(keys) => {
                  const k = [...keys][0];
                  if (k !== undefined) selectCode(Number(k), { openDetails: false });
                }}
                expandedKeys={expandedKeys}
                onExpandedChange={(keys) => {
                  if (query.trim() !== '') return;
                  const next = new Set<number>();
                  for (const id of parents) if (!keys.has(id)) next.add(id);
                  setCollapsed(next);
                }}
              >
                {renderNode}
              </Tree>
            )}
          </div>
        </section>

        <section className="bk-editor-detail bk-card" aria-label={t('The chosen code')}>
          {selected ? (
            <Tabs selectedKey={tab} onSelectionChange={(k) => setTab(k as TabId)} className="bk-editor-tabs">
              <TabList aria-label={t('{name}: details', { name: selected.name })} className="bk-editor-tablist">
                <Tab id="details" className="bk-tab">{t('Code Details')}</Tab>
                <Tab id="examples" className="bk-tab">{exampleCount === null ? t('Examples') : t('Examples ({count})', { count: exampleCount })}
                </Tab>
                <Tab id="usage" className="bk-tab">{t('Usage')}</Tab>
                <Tab id="history" className="bk-tab">{t('History')}</Tab>
              </TabList>
              <TabPanel id="details" className="bk-editor-panel">
                <DetailsTab key={`${selected.id}-${selected.version}-${selected.kind}`} code={selected} />
              </TabPanel>
              <TabPanel id="examples" className="bk-editor-panel">
                <ExamplesTab code={selected} />
              </TabPanel>
              <TabPanel id="usage" className="bk-editor-panel">
                <UsageTab code={selected} />
              </TabPanel>
              <TabPanel id="history" className="bk-editor-panel">
                <HistoryTab code={selected} />
              </TabPanel>
            </Tabs>
          ) : (
            <p className="help bk-editor-empty">{codes.length === 0 ? t('There are no codes yet. New Code makes the first one, or select words in a document and press I.') : t('Choose a code in the tree to see what it means.')}</p>
          )}
        </section>
      </div>
      <p className="bk-editor-foot">
        <Button className="bk-link" onPress={() => goTo('code')}>{t('Back to the coding workspace')}</Button>
      </p>
    </main>
  );
}

// ---------------------------------------------------------------- Code Details

function DetailsTab({ code }: { code: CodeRow }) {
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [name, setName] = useState(code.name);
  const [definition, setDefinition] = useState(code.definition);
  const [inclusion, setInclusion] = useState(code.inclusion_criteria);
  const [exclusion, setExclusion] = useState(code.exclusion_criteria);
  const [theme, setTheme] = useState(code.kind === 'theme');
  const [group, setGroup] = useState(code.exclusive_group);
  const [rated, setRated] = useState(code.rated);
  const [aiOk, setAiOk] = useState(code.ai_suggestible !== false);
  const [ratingLabel, setRatingLabel] = useState(code.rating_label);
  const [ratingLow, setRatingLow] = useState(code.rating_low);
  const [ratingHigh, setRatingHigh] = useState(code.rating_high);
  const [error, setError] = useState<string | null>(null);
  // Set when switching rating off was refused because codings carry ratings: the person can choose to clear them with it.
  const [clearOffer, setClearOffer] = useState(false);
  const codes = useApp((s) => s.codes);
  const dataVersion = useApp((s) => s.dataVersion);
  // After a save (or an undo) the boxes show what the engine now holds, for example the spelling a group already had.
  useEffect(() => {
    setGroup(code.exclusive_group);
    setRated(code.rated);
    setAiOk(code.ai_suggestible !== false);
    setRatingLabel(code.rating_label);
    setRatingLow(code.rating_low);
    setRatingHigh(code.rating_high);
  }, [code.exclusive_group, code.rated, code.ai_suggestible, code.rating_label, code.rating_low, code.rating_high]);
  const merged = code.status === 'merged';
  const locked = readOnly || merged;
  const dirty =
    name !== code.name ||
    definition !== code.definition ||
    inclusion !== code.inclusion_criteria ||
    exclusion !== code.exclusion_criteria ||
    theme !== (code.kind === 'theme') ||
    group.trim() !== code.exclusive_group ||
    rated !== code.rated ||
    aiOk !== (code.ai_suggestible !== false) ||
    ratingLabel.trim() !== code.rating_label ||
    ratingLow.trim() !== code.rating_low ||
    ratingHigh.trim() !== code.rating_high;
  const inPalette = PALETTE.some((p) => p.hex.toLowerCase() === code.color.toLowerCase());

  // The groups that exist, to offer as suggestions, and the other codes of this code's group.
  const groupNames = useMemo(() => [...new Set(codes.map((c) => c.exclusive_group).filter((g) => g !== ''))].sort((a, b) => compareText(a, b)), [codes]);
  const groupMates = useMemo(
    () => (code.exclusive_group === '' ? [] : codes.filter((c) => c.id !== code.id && c.status === 'active' && c.exclusive_group.toLowerCase() === code.exclusive_group.toLowerCase())),
    [codes, code.id, code.exclusive_group],
  );
  // Passages that were coded before the group existed can already hold two of its codes; they are told, not changed.
  const [breaking, setBreaking] = useState(0);
  useEffect(() => {
    if (code.exclusive_group === '') {
      setBreaking(0);
      return;
    }
    let live = true;
    call('code.group_check', { group: code.exclusive_group })
      .then((r) => live && setBreaking(r.violations))
      .catch(() => live && setBreaking(0));
    return () => {
      live = false;
    };
  }, [code.exclusive_group, code.usage, dataVersion]);

  const save = async (clearRatings = false) => {
    setError(null);
    setClearOffer(false);
    if (!name.trim()) return setError(t('A code needs a title.'));
    try {
      await editCode(code.id, {
        ...(name !== code.name ? { name: name.trim() } : {}),
        ...(definition !== code.definition ? { definition } : {}),
        ...(inclusion !== code.inclusion_criteria ? { inclusion_criteria: inclusion } : {}),
        ...(exclusion !== code.exclusion_criteria ? { exclusion_criteria: exclusion } : {}),
        ...(theme !== (code.kind === 'theme') ? { theme } : {}),
        ...(group.trim() !== code.exclusive_group ? { exclusive_group: group } : {}),
        ...(rated !== code.rated ? { rated } : {}),
        ...(aiOk !== (code.ai_suggestible !== false) ? { ai_suggestible: aiOk } : {}),
        ...(ratingLabel.trim() !== code.rating_label ? { rating_label: ratingLabel } : {}),
        ...(ratingLow.trim() !== code.rating_low ? { rating_low: ratingLow } : {}),
        ...(ratingHigh.trim() !== code.rating_high ? { rating_high: ratingHigh } : {}),
        ...(clearRatings ? { clear_ratings: true } : {}),
      });
    } catch (e) {
      const message = toApiError(e).message;
      setError(message);
      if (code.rated && !rated && /carr(y|ies) a rating/.test(message)) setClearOffer(true);
    }
  };

  const setActive = async (on: boolean) => {
    try {
      if (on) await restoreCode(code.id);
      else await retireCode(code.id);
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
  };

  return (
    <form
      className="code-detail bk-editor-form"
      data-testid="code-detail"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      {code.status !== 'active' ? (
        <Notice kind="warning">{code.status === 'retired' ? t('This code is retired: it keeps its codings but cannot be applied to new text.') : t('This code is merged into another code and cannot be changed.')}</Notice>
      ) : null}
      {code.status === 'active' && code.definition.trim() === '' ? (
        <Notice kind="info">{t('This code has no definition yet. A sentence or two about what it means helps you apply it the same way each time, and helps anyone else who codes.')}</Notice>
      ) : null}
      {error ? <Notice>{error}</Notice> : null}
      {clearOffer ? (
        <div className="actions" data-testid="clear-ratings-offer">
          <Button onPress={() => void save(true)}>{t('Clear the ratings and turn rating off')}</Button>
        </div>
      ) : null}

      <TextField value={name} onChange={setName} className="field" isReadOnly={locked}>
        <Label>{t('Title')}</Label>
        <Input id="code-name" />
      </TextField>
      <TextField value={definition} onChange={setDefinition} className="field" isReadOnly={locked}>
        <Label>{t('Description')}</Label>
        <TextArea rows={3} />
      </TextField>
      <TextField value={inclusion} onChange={setInclusion} className="field" isReadOnly={locked}>
        <Label>{t('Inclusion Criteria')}</Label>
        <TextArea rows={3} />
      </TextField>
      <TextField value={exclusion} onChange={setExclusion} className="field" isReadOnly={locked}>
        <Label>{t('Exclusion Criteria')}</Label>
        <TextArea rows={3} />
      </TextField>

      <RadioGroup value={inPalette ? code.color : ''} onChange={(c) => void setCodeColor(code.id, c)} className="field color-field" isDisabled={locked}>
        <Label>{rich('Color <span>(now {color}; it changes as soon as you choose)</span>', { color: colorName(code.color), span: (inside) => <span className="muted">{inside}</span> })}
        </Label>
        <div className="color-options">
          {PALETTE.map((p) => (
            <Radio key={p.hex} value={p.hex} className="react-aria-Radio color-radio">
              <span className="swatch plain" style={{ ['--swatch-color' as string]: p.hex }} aria-hidden="true" />
              <span>{p.name}</span>
            </Radio>
          ))}
        </div>
      </RadioGroup>

      <fieldset className="bk-properties">
        <legend>{t('Code Properties')}</legend>
        <div className="bk-props-grid">
          <Checkbox isSelected={code.status === 'active'} onChange={(v) => void setActive(v)} isDisabled={locked} className="check">
            <Box />{t('Active')}</Checkbox>
          <Checkbox isSelected={theme} onChange={setTheme} isDisabled={locked} className="check">
            <Box />{t('Theme that groups other codes')}</Checkbox>
          <Checkbox isSelected={rated} onChange={setRated} isDisabled={locked} className="check">
            <Box />{t('Weighted / Rated')}</Checkbox>
          <Checkbox isSelected={aiOk} onChange={setAiOk} isDisabled={locked} className="check" data-testid="prop-ai">
            <Box />{t('AI Suggestible')}</Checkbox>
        </div>

        <TextField value={group} onChange={setGroup} className="field bk-prop-field" isReadOnly={locked} data-testid="prop-group">
          <Label>{t('Mutually Exclusive Group')}</Label>
          <Input list="bk-group-names" placeholder={t('Not in a group')} />
          <Text slot="description" className="muted">{t('Codes with the same group name cannot be given by one coder to the same words: choosing a second one asks whether to replace the first. Leave it empty for no group.')}</Text>
        </TextField>
        {!aiOk ? (
          <p className="muted" data-testid="prop-ai-note">{t('This code is never put in a prompt packet for an AI, so no AI suggestion can use it.')}</p>
        ) : null}
        <datalist id="bk-group-names">
          {groupNames.map((g) => (
            <option key={g} value={g} />
          ))}
        </datalist>
        {code.exclusive_group !== '' ? (
          <p className="muted" data-testid="group-mates">
            {groupMates.length === 0
              ? t('{name} is the only code in the group {exclusive_group} so far.', { name: code.name, exclusive_group: code.exclusive_group })
              : t('In the group {exclusive_group} with {list}.', { exclusive_group: code.exclusive_group, list: groupMates.map((c) => c.name).join(', ') })}
            {breaking > 0 ? ' ' + tn(breaking, { one: '{passages} already has two codes of this group. They stay as they are; the rule applies to new coding.', other: '{passages} already have two codes of this group. They stay as they are; the rule applies to new coding.' }, { passages: tn(breaking, { one: '{count} passage', other: '{count} passages' }) }) : ''}
          </p>
        ) : null}

        {rated ? (
          <div className="bk-rating-setup" data-testid="prop-rating">
            <p className="muted">{t('A rated code lets each coding be rated from 1 to 5 stars in Segment Details. Describe the scale so everyone who codes reads it the same way.')}</p>
            <TextField value={ratingLabel} onChange={setRatingLabel} className="field" isReadOnly={locked}>
              <Label>{t('What the rating measures')}</Label>
              <Input placeholder={t('For example, Intensity')} />
            </TextField>
            <div className="bk-scale-ends-fields">
              <TextField value={ratingLow} onChange={setRatingLow} className="field" isReadOnly={locked}>
                <Label>{t('A 1 means')}</Label>
                <Input placeholder={t('For example, Mentioned in passing')} />
              </TextField>
              <TextField value={ratingHigh} onChange={setRatingHigh} className="field" isReadOnly={locked}>
                <Label>{t('A 5 means')}</Label>
                <Input placeholder={t('For example, Central to the account')} />
              </TextField>
            </div>
          </div>
        ) : null}
        <p className="muted">{t('Active means the code can be applied to new text; turning it off retires it, and its codings stay. AI suggestions come with the AI part of the program.')}</p>
      </fieldset>

      <p className="muted">{t('Used in {codings}. Version {version}.', { codings: tn(code.usage, { one: '{count} coding', other: '{count} codings' }), version: code.version })}</p>
      {locked ? null : (
        <div className="actions">
          <Button type="submit" className="primary" isDisabled={!dirty}>{t('Save changes')}</Button>
        </div>
      )}
    </form>
  );
}

function Box() {
  return (
    <div className="box" aria-hidden="true">
      <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
        <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
      </svg>
    </div>
  );
}

// ---------------------------------------------------------------- Examples

function useSegments(code: CodeRow) {
  const version = useApp((s) => s.dataVersion);
  const codes = useApp((s) => s.codes);
  const [rows, setRows] = useState<SegmentRow[] | null>(null);
  const [total, setTotal] = useState(0);
  useEffect(() => {
    let live = true;
    setRows(null);
    call('segments.list', { code_ids: [code.id], limit: 5000 })
      .then((r) => {
        if (!live) return;
        setRows(r.segments);
        setTotal(r.total);
      })
      .catch(() => live && setRows([]));
    return () => {
      live = false;
    };
    // The count on the code changes when a coding is made or removed, which is when the list is read again.
  }, [code.id, code.usage, version, codes.length]);
  return { rows, total };
}

function ExamplesTab({ code }: { code: CodeRow }) {
  const anchors = useApp((s) => s.anchors);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const dataVersion = useApp((s) => s.dataVersion);
  const { rows } = useSegments(code);
  const examples = (rows ?? []).filter((s) => s.code_id === code.id && anchors.has(s.coding_id));
  const [counter, setCounter] = useState<SegmentRow[] | null>(null);
  useEffect(() => {
    let live = true;
    call('code.counterexamples', { id: code.id })
      .then((r) => live && setCounter(r))
      .catch(() => live && setCounter([]));
    return () => {
      live = false;
    };
  }, [code.id, dataVersion]);
  return (
    <div className="bk-editor-list" data-testid="examples-tab">
      <h4 className="bk-h3">{t('Supporting examples ({count})', { count: examples.length })}</h4>
      {rows === null ? (
        <p className="muted" role="status">{t('Loading…')}</p>
      ) : examples.length === 0 ? (
        <p className="muted">{t('No coding is marked as an example of this code yet. Open a coded passage, choose Options for this coding in Segment Details, and tick "Use as an example of {name}".', { name: code.name })}</p>
      ) : (
        <ul className="bk-example-list">
          {examples.map((s) => (
            <li key={s.coding_id}>
              <blockquote className="bk-quote" style={{ ['--seg-color' as string]: code.color }}>
                {`“${s.text.trim()}”`}
              </blockquote>
              <p className="muted">
                <Icon name="src-transcript" size={14} /> {s.source_name}
              </p>
              <div className="actions">
                <Button onPress={() => void openAt(s.source_id, s.start, s.end, s.coding_id)}>{t('Open at source')}</Button>
                {readOnly ? null : (
                  <Button onPress={() => void setAnchor(s.coding_id, false)} aria-label={t('Stop using this passage as an example of {name}', { name: code.name })}>{t('Remove as example')}</Button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
      <h4 className="bk-h3" data-testid="counter-heading">{t('Counterexamples ({count})', { count: counter?.length ?? 0 })}</h4>
      <p className="muted">{t('Passages that look as if they belong to {name} but do not. Each is coded with another code.', { name: code.name })}</p>
      {counter === null ? (
        <p className="muted" role="status">{t('Loading…')}</p>
      ) : counter.length === 0 ? (
        <p className="muted">{t('No passage is marked as a counterexample of this code yet.')}</p>
      ) : (
        <ul className="bk-example-list" data-testid="counter-list">
          {counter.map((s) => (
            <li key={s.coding_id}>
              <blockquote className="bk-quote" style={{ ['--seg-color' as string]: code.color }}>
                {`“${s.text.trim()}”`}
              </blockquote>
              <p className="muted">
                <Icon name="src-transcript" size={14} /> {t('{source_name} · coded {code_name}', { source_name: s.source_name, code_name: s.code_name })}
              </p>
              <div className="actions">
                <Button onPress={() => void openAt(s.source_id, s.start, s.end, s.coding_id)}>{t('Open at source')}</Button>
                {readOnly ? null : (
                  <Button onPress={() => void attemptCounter(code.id, s.coding_id, false)} aria-label={t('Stop using this passage as a counterexample of {name}', { name: code.name })}>{t('Remove as counterexample')}</Button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
      {readOnly ? null : (
        <div className="actions">
          <Button onPress={() => appStore.set({ dialog: { kind: 'counterexample', codeId: code.id } })}>{t('Add a counterexample')}</Button>
        </div>
      )}
    </div>
  );
}

async function attemptCounter(codeId: number, codingId: number, on: boolean): Promise<void> {
  try {
    await setCounterexample(codeId, codingId, on);
  } catch (e) {
    announce(toApiError(e).message, 'assertive');
  }
}

// ---------------------------------------------------------------- Usage

function UsageTab({ code }: { code: CodeRow }) {
  const { rows, total } = useSegments(code);
  const bySource = useMemo(() => {
    const m = new Map<number, { id: number; name: string; n: number; first: SegmentRow }>();
    for (const s of rows ?? []) {
      const cur = m.get(s.source_id);
      if (cur) cur.n++;
      else m.set(s.source_id, { id: s.source_id, name: s.source_name, n: 1, first: s });
    }
    return [...m.values()].sort((a, b) => b.n - a.n || compareText(a.name, b.name));
  }, [rows]);
  return (
    <div className="bk-editor-list" data-testid="usage-tab">
      <h4 className="bk-h3">{t('Where {name} is used', { name: code.name })}</h4>
      {rows === null ? (
        <p className="muted" role="status">{t('Loading…')}</p>
      ) : bySource.length === 0 ? (
        <p className="muted">{t('This code has not been applied yet.')}</p>
      ) : (
        <>
          <p className="muted" data-testid="usage-summary">
            {t('{codings} in {documents}. The count includes the codings of any sub-codes, and a coding is one code on one stretch of text.', { codings: tn(total, { one: '{count} coding', other: '{count} codings' }), documents: tn(bySource.length, { one: '{count} document', other: '{count} documents' }) })}</p>
          <table className="bk-usage-table">
            <caption className="visually-hidden">{t('Codings of {name} by document', { name: code.name })}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Document')}</th>
                <th scope="col">{t('Codings')}</th>
                <th scope="col">
                  <span className="visually-hidden">{t('Open')}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {bySource.map((s) => (
                <tr key={s.id}>
                  <th scope="row">{s.name}</th>
                  <td>{num(s.n)}</td>
                  <td>
                    <Button className="bk-link" onPress={() => void openAt(s.id, s.first.start, s.first.end, s.first.coding_id)} aria-label={t('Open {name} at its first coding of {name2}', { name: s.name, name2: code.name })}>{t('Open')}</Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- History

interface InEffect {
  version: CodebookVersion;
  /** The code as that version had it; null when it was not in the codebook yet. */
  then: { name: string; definition: string } | null;
}

function HistoryTab({ code }: { code: CodeRow }) {
  const codes = useApp((s) => s.codes);
  const sources = useApp((s) => s.sources);
  const dataVersion = useApp((s) => s.dataVersion);
  const [events, setEvents] = useState<{ ts: string; text: string }[] | null>(null);
  const [versions, setVersions] = useState<InEffect[] | null>(null);

  const lookups = useMemo<WordingLookups>(
    () => ({
      codeName: (id) => (id === null || id === undefined ? null : (codes.find((c) => c.id === id)?.name ?? null)),
      sourceName: (id) => (id === null || id === undefined ? null : (sources.find((s) => s.id === id)?.name ?? null)),
      quote: (sourceId, start, end) => quoteFromOpenSource(sourceId, start, end),
    }),
    [codes, sources],
  );

  useEffect(() => {
    let live = true;
    call('events.list', { limit: 5000 })
      .then((all) => {
        if (!live) return;
        const mine = all.filter((e) => e.target_type === 'code' && e.target_id === code.id);
        setEvents(mine.map((e) => ({ ts: e.ts, text: describeEvent(e, lookups).text })).reverse());
      })
      .catch(() => live && setEvents([]));
    return () => {
      live = false;
    };
  }, [code.id, code.version, dataVersion, lookups]);

  useEffect(() => {
    let live = true;
    (async () => {
      try {
        const list = await call('codebook.versions');
        const out: InEffect[] = [];
        for (const v of list) {
          const snap = await call('codebook.version', { version: v.version });
          const was = snap.codes.find((c) => c.id === code.id);
          out.push({ version: v, then: was ? { name: was.name, definition: was.definition } : null });
        }
        if (live) setVersions(out);
      } catch {
        if (live) setVersions([]);
      }
    })();
    return () => {
      live = false;
    };
  }, [code.id, code.version]);

  return (
    <div className="bk-editor-list" data-testid="history-tab">
      <h4 className="bk-h3">{t('Changes to {path}', { path: pathLabel(codes, code.id) })}</h4>
      {events === null ? (
        <p className="muted" role="status">{t('Loading…')}</p>
      ) : events.length === 0 ? (
        <p className="muted">{t('No change to this code is on record.')}</p>
      ) : (
        <ol className="recent-list" data-testid="code-events">
          {events.map((e, i) => (
            <li key={i}>
              <time dateTime={e.ts} className="event-time">
                {formatWhen(e.ts)}
              </time>
              <span>{e.text}</span>
            </li>
          ))}
        </ol>
      )}

      <h4 className="bk-h3">{t('Versions of the codebook')}</h4>
      {versions === null ? (
        <p className="muted" role="status">{t('Loading…')}</p>
      ) : versions.length === 0 ? (
        <p className="muted">{t('No version of the codebook has been saved. Save version freezes the codebook as it is now, so a later change can be compared with it.')}</p>
      ) : (
        <ul className="bk-version-list" data-testid="code-versions">
          {versions.map(({ version, then }) => (
            <li key={version.version}>
              {version.note ? rich('<strong>Version {version}: {note}</strong> <span>({when})</span>', { version: version.version, note: version.note, when: formatWhen(version.created_at), strong: (inside) => <strong>{inside}</strong>, span: (inside) => <span className="muted">{inside}</span> }) : rich('<strong>Version {version}</strong> <span>({when})</span>', { version: version.version, when: formatWhen(version.created_at), strong: (inside) => <strong>{inside}</strong>, span: (inside) => <span className="muted">{inside}</span> })}
              <p>{then ? (then.definition.trim() ? then.definition : t('No description yet in this version.')) : t('This code was not in the codebook yet.')}</p>
              {then && then.name !== code.name ? <p className="muted">{t('It was called {name}.', { name: then.name })}</p> : null}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
