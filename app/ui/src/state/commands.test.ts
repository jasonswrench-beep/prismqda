import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ICON_DEFS } from '../assets/icons.generated';
import { initialState, type AppState } from './app';
import { COMMANDS, NOT_BUILT_TABS, RIBBON, RIBBON_TABS, commandById, commandMatches, shortcutFor } from './commands';

const state = (patch: Partial<AppState>): AppState => ({ ...initialState, ...patch });

test('every command has a unique id, a label, a sentence, and an icon that exists', () => {
  const ids = COMMANDS.map((c) => c.id);
  assert.equal(new Set(ids).size, ids.length, 'ids are unique');
  for (const c of COMMANDS) {
    assert.ok(c.label.trim() !== '', `${c.id} has a label`);
    assert.ok(c.title.trim().length > c.label.length || c.title !== c.label, `${c.id} has a fuller title`);
    assert.ok(c.icon in ICON_DEFS, `${c.id} uses an icon that exists: ${c.icon}`);
  }
});

test('the tabs are the fourteen of the screen specification book, in its order', () => {
  assert.deepEqual(
    RIBBON_TABS.map((t) => t.label),
    ['Home', 'Import', 'Code', 'Memos', 'Explore', 'Mixed Methods', 'Visual Tools', 'Synthesize', 'Text Analytics', 'Statistics', 'Report', 'Team', 'AI', 'View'],
  );
});

test('the Home ribbon is the Home ribbon of the book, group by group and button by button', () => {
  const names = (ids: string[]) => ids.map((id) => commandById(id).label);
  const flat = (g: (typeof RIBBON)['home'][number]) => g.items.map((i) => (typeof i === 'string' ? i : i.id));
  const home = RIBBON.home;
  assert.deepEqual(home.map((g) => g.caption), ['Project', '', 'Add', 'Coding', 'Explore', 'Analyze & Report']);
  assert.deepEqual(names(flat(home[0])), ['New Project', 'Open Project', 'Save']);
  assert.deepEqual(names(flat(home[1])), ['Add Documents']);
  assert.deepEqual(names(flat(home[2])), ['New Code', 'New Memo', 'New Case']);
  assert.deepEqual(names(flat(home[3])), ['Code Selection', 'Auto Code', 'Code In Vivo', 'Quick Code']);
  assert.deepEqual(names(flat(home[4])), ['Code Summary', 'Code Relations', 'Query']);
  assert.deepEqual(names(flat(home[5])), ['Visualize', 'Compare Cases', 'Report Builder']);
});

test('every ribbon button is a command that exists, and a tab with no commands says which part is not built', () => {
  for (const tab of RIBBON_TABS) {
    for (const g of RIBBON[tab.id]) {
      for (const item of g.items) {
        const ids = typeof item === 'string' ? [item] : [item.id, ...item.menu];
        for (const id of ids) assert.ok(COMMANDS.some((c) => c.id === id), `${tab.id}: ${id} is a command`);
      }
    }
    if (RIBBON[tab.id].length === 0) assert.ok(NOT_BUILT_TABS[tab.id], `${tab.id} explains why it is empty`);
  }
});

test('the Explore tools and Auto Code can be run, and Auto Code says why it cannot in a read-only project', () => {
  for (const id of ['code-summary', 'code-relations', 'query', 'visualize', 'compare-cases', 'auto-code', 'review-queue']) {
    assert.equal(commandById(id).unavailable(state({})), null, `${id} can be run`);
  }
  for (const id of ['auto-code', 'review-queue']) {
    assert.match(commandById(id).unavailable(state({ project: { read_only: true } as never })) ?? '', /read only/, `${id} is shut in a read-only project`);
  }
});

test('shortcuts are shown with the key of this computer', () => {
  assert.equal(shortcutFor(commandById('save'), false), 'Ctrl+S');
  assert.equal(shortcutFor(commandById('save'), true), '⌘S');
  assert.equal(shortcutFor(commandById('repeat-code'), true), '⌘D');
  assert.equal(shortcutFor(commandById('in-vivo'), true), 'I', 'a plain letter key stays a letter');
  assert.equal(shortcutFor(commandById('new-code'), false), undefined);
});

test('a command that cannot run says why, in words, and one that can returns null', () => {
  const inVivo = commandById('in-vivo');
  const onHome = inVivo.unavailable(state({ workspace: 'home', currentSourceId: 1 }));
  assert.match(onHome ?? '', /Go back to the coding workspace|coding workspace/);
  const noSource = inVivo.unavailable(state({ workspace: 'code', currentSourceId: null }));
  assert.match(noSource ?? '', /Open a document/);
  assert.equal(inVivo.unavailable(state({ workspace: 'code', currentSourceId: 3 })), null);
});

test('read-only projects switch off the commands that change things, and nothing else', () => {
  const readOnly = state({ project: { read_only: true } as AppState['project'], workspace: 'code', currentSourceId: 1, lastCodeId: 2 });
  for (const id of ['undo', 'backup', 'import', 'import-folder', 'new-code', 'new-case', 'in-vivo', 'repeat-code']) {
    assert.match(commandById(id).unavailable(readOnly) ?? '', /read only/, `${id} is off in a read-only project`);
  }
  for (const id of ['search', 'export', 'memo', 'shortcuts', 'go-home']) {
    assert.equal(commandById(id).unavailable(readOnly), null, `${id} still works in a read-only project`);
  }
});

test('Quick Code needs a code to repeat', () => {
  const repeat = commandById('repeat-code');
  assert.match(repeat.unavailable(state({ workspace: 'code', currentSourceId: 1, lastCodeId: null })) ?? '', /Apply a code first/);
  assert.equal(repeat.unavailable(state({ workspace: 'code', currentSourceId: 1, lastCodeId: 4 })), null);
});

test('the command box matches every typed word against the name, the sentence, and the keywords', () => {
  const backup = commandById('backup');
  assert.ok(commandMatches(backup, 'backup'));
  assert.ok(commandMatches(backup, 'save'), 'a keyword finds it');
  assert.ok(commandMatches(backup, 'dated copy'), 'words from its sentence find it');
  assert.ok(commandMatches(backup, '  SAVE   copy '), 'case and extra spaces do not matter');
  assert.ok(!commandMatches(backup, 'save zebra'), 'every typed word has to match');
  assert.ok(commandMatches(backup, ''), 'nothing typed shows everything');
  assert.ok(commandMatches(commandById('export'), 'export'));
  assert.ok(commandMatches(commandById('go-report'), 'export'), 'Go to Report is also found by export');
});

test('Reset only works when something is chosen, and the scope chip words follow the choice', () => {
  const reset = commandById('reset-scope');
  assert.match(reset.unavailable(state({ activeCodeIds: [], retrievalMode: 'all' })) ?? '', /Nothing is filtered/);
  assert.equal(reset.unavailable(state({ activeCodeIds: [], retrievalMode: 'all', retrievalQuery: 'bus' })), null, 'a search is a filter too');
  assert.equal(reset.unavailable(state({ activeCodeIds: [], retrievalMode: 'all', retrievalCase: 2 })), null, 'so is a case');
  assert.equal(reset.unavailable(state({ activeCodeIds: [2], retrievalMode: 'all' })), null);
  assert.equal(reset.unavailable(state({ activeCodeIds: [], retrievalMode: 'open' })), null, 'looking only in the open source counts as a choice');
  const chip = commandById('scope');
  assert.equal(chip.dynamicLabel?.(state({ activeCodeIds: [] })), 'No filter');
  assert.equal(chip.dynamicLabel?.(state({ activeCodeIds: [1, 2] })), '2 codes');
});
