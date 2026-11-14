// The Codebook Editor: the toolbar, the tree, and the four tabs of one code (Code Details, Examples, Usage, History), plus Compare.

import fs from 'node:fs';
import path from 'node:path';

export async function runEditor(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe, mainTab, codeRow, waitStatus } = api;

  const byName = async (name) => (await rpc('code.list')).find((c) => c.name === name);
  const tree = () => page.getByRole('treegrid', { name: 'Codebook' });
  const form = () => page.getByTestId('code-detail');
  const tab = (name) => page.getByRole('tablist', { name: /: details$/ }).getByRole('tab', { name });

  async function openEditor(codeName) {
    await mainTab('Coding').click();
    await page.getByRole('tab', { name: 'Code', exact: true }).click();
    await page.getByRole('button', { name: 'Codebook Editor', exact: true }).click();
    await api.editorHeading().waitFor();
    if (codeName) await codeRow(codeName).click();
  }
  async function leaveEditor() {
    await api.closeDefinition();
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
  }

  await S('the Codebook Editor opens from the Code ribbon with the book\'s toolbar, tree, and four tabs', async () => {
    await openEditor('Look check A');
    const tools = (await page.getByRole('toolbar', { name: 'Codebook actions' }).getByRole('button').allTextContents()).map((t) => t.trim());
    h.equal(tools, ['New Code', 'Edit', 'Merge', 'Hierarchy', 'Compare', 'Export', 'Save version'], 'the toolbar: New Code, Edit, Merge, Hierarchy, Compare, Export (and Save version, which Compare needs)');
    const active = (await rpc('code.list')).filter((c) => c.status === 'active').length;
    await page.getByRole('heading', { level: 3, name: `Codebook (${active})` }).waitFor();
    h.check(true, `the tree is headed with the number of codes: ${active}`);
    h.check(/count their codings|counts are codings/i.test(await page.locator('#editor-tree-note').innerText()), 'and says what the counts count');
    const tabs = (await page.getByRole('tablist', { name: /: details$/ }).getByRole('tab').allTextContents()).map((t) => t.trim());
    h.equal(tabs.map((t) => t.replace(/ \(\d+\)$/, '')), ['Code Details', 'Examples', 'Usage', 'History'], 'the four tabs');
    h.equal(await form().getByRole('textbox', { name: 'Title' }).inputValue(), 'Look check A', 'choosing a code loads its title');
    for (const label of ['Description', 'Inclusion Criteria', 'Exclusion Criteria']) h.equal(await form().getByRole('textbox', { name: label }).count(), 1, `${label} is a box`);
    // Code Properties: all are real, AI Suggestible included.
    const props = await form().getByRole('group', { name: 'Code Properties' }).getByRole('checkbox').evaluateAll((els) => els.map((e) => ({ name: e.closest('label')?.textContent?.trim(), disabled: e.disabled, on: e.checked })));
    h.equal(props.map((p) => p.name?.replace(/\s*not built yet$/, '')), ['Active', 'Theme that groups other codes', 'Weighted / Rated', 'AI Suggestible'], 'the properties that are switches');
    h.equal(props.filter((p) => p.disabled).length, 0, 'every property is a working switch');
    h.equal(props[3].on, true, 'AI Suggestible is on to begin with: a code may be suggested unless it is turned off');
    h.equal(props[0].on, true, 'Active is on');
    h.equal(await form().getByRole('combobox', { name: /^Mutually Exclusive Group/ }).count(), 1, 'the group is a box with the existing group names to choose from');
    await page.screenshot({ path: `${ctx.shots}/codebook-editor.png`, fullPage: true });
    await axe('the Codebook Editor');
  });

  await S('title, description, and both criteria are saved together, and a renamed code keeps its codings', async () => {
    const before = await byName('Look check A');
    await form().getByRole('textbox', { name: 'Description' }).fill('The first code the look check uses.');
    await form().getByRole('textbox', { name: 'Inclusion Criteria' }).fill('When the check word is the one selected.');
    await form().getByRole('textbox', { name: 'Exclusion Criteria' }).fill('Never for the other check words.');
    await form().getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Look check A/);
    const saved = await byName('Look check A');
    h.equal([saved.definition, saved.inclusion_criteria, saved.exclusion_criteria], ['The first code the look check uses.', 'When the check word is the one selected.', 'Never for the other check words.'], 'the engine has all three');
    h.check(!/no definition yet/.test(await form().textContent()), 'the reminder is gone');
    // A new name: the code is the same code, so its codings are still its own.
    await form().getByRole('textbox', { name: 'Title' }).fill('Look check Alpha');
    await form().getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Look check Alpha/);
    const renamed = (await rpc('code.list')).find((c) => c.id === before.id);
    h.equal([renamed.name, renamed.usage], ['Look check Alpha', before.usage], 'the name changed and the codings stayed');
    await codeRow('Look check Alpha').waitFor();
    await form().getByRole('textbox', { name: 'Title' }).fill('Look check A');
    await form().getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Look check A\./);
  });

  await S('Active turns a code off and on, and says what that means', async () => {
    const box = () => page.getByRole('checkbox', { name: /^Active/ });
    await page.locator('label').filter({ has: box() }).click();
    await waitStatus(/Retired Look check A/);
    h.equal((await byName('Look check A')).status, 'retired', 'turning Active off retires the code');
    h.check(/retired/.test(await form().textContent()), 'the page says it is retired and keeps its codings');
    await page.locator('label').filter({ has: box() }).click();
    await waitStatus(/Restored Look check A/);
    h.equal((await byName('Look check A')).status, 'active', 'and on restores it');
  });

  await S('the Hierarchy menu and Merge open from the toolbar, and Cancel changes nothing', async () => {
    await page.getByRole('button', { name: 'Hierarchy' }).click();
    const items = (await page.getByRole('menu', { name: 'Hierarchy' }).getByRole('menuitem').allTextContents()).map((t) => t.trim());
    h.equal(items, ['Add sub-code', 'Move up', 'Move down', 'Make sub-code', 'Move out', 'Split…'], 'the hierarchy actions');
    await page.keyboard.press('Escape');
    const before = (await rpc('code.list')).length;
    await page.getByRole('button', { name: 'Merge' }).click();
    const dialog = page.getByRole('dialog', { name: /Merge Look check A/ });
    await dialog.waitFor();
    await axe('the merge dialog opened from the editor');
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await dialog.waitFor({ state: 'hidden' });
    const after = await rpc('code.list');
    h.check(after.length === before && after.every((c) => c.status !== 'merged' || c.name !== 'Look check A'), 'a cancelled merge changed nothing');
  });

  await S('Examples lists a coding marked as an example, opens it at its source, and can take it off', async () => {
    const code = await byName('Look check A');
    const segs = (await rpc('segments.list', { code_ids: [code.id], limit: 5 })).segments.filter((s) => s.code_id === code.id);
    h.check(segs.length > 0, 'the code has a coding to use as an example');
    await rpc('coding.anchor', { id: segs[0].coding_id, anchor: true });
    // Examples are read when the project opens, so open it again.
    await page.reload();
    await openEditor('Look check A');
    await tab(/^Examples \(1\)$/).waitFor();
    await tab(/^Examples/).click();
    const panel = page.getByTestId('examples-tab');
    await panel.locator('blockquote').first().waitFor();
    h.check((await panel.textContent()).includes(segs[0].text.trim().slice(0, 12)), 'the example shows its words');
    h.check((await panel.textContent()).includes(segs[0].source_name), 'and its source');
    await axe('the Examples tab');
    await panel.getByRole('button', { name: 'Open at source' }).click();
    await page.getByRole('heading', { level: 2, name: 'Documents' }).waitFor();
    h.check(true, 'Open at source goes to the coding workspace');
    await openEditor('Look check A');
    await tab(/^Examples/).click();
    await page.getByTestId('examples-tab').getByRole('button', { name: /^Stop using this passage as an example/ }).click();
    await tab(/^Examples \(0\)$/).waitFor();
    h.check(/No coding is marked as an example/.test(await page.getByTestId('examples-tab').textContent()), 'taking it off leaves none, and says how to add one');
  });

  await S('Usage says where a code is used, document by document, with what the count covers', async () => {
    const welcome = await byName('Welcome');
    await codeRow('Welcome').click();
    await tab('Usage').click();
    const panel = page.getByTestId('usage-tab');
    await page.getByTestId('usage-summary').waitFor();
    const total = (await rpc('segments.list', { code_ids: [welcome.id], limit: 1 })).total;
    h.check(new RegExp(`^${total.toLocaleString('en-US')} codings? in \\d+ documents?`).test((await page.getByTestId('usage-summary').innerText()).trim()), `the summary names the count and the documents: ${await page.getByTestId('usage-summary').innerText()}`);
    h.check(/includes the codings of any sub-codes/.test(await panel.innerText()), 'and what the count includes');
    const rows = await panel.locator('tbody tr').evaluateAll((trs) => trs.map((r) => Number(r.querySelector('td').textContent.replace(/,/g, ''))));
    h.equal(rows.reduce((a, b) => a + b, 0), total, 'the documents add up to the total');
    await axe('the Usage tab');
    await panel.getByRole('button', { name: /^Open .* at its first coding of Welcome$/ }).first().click();
    await page.getByRole('heading', { level: 2, name: 'Documents' }).waitFor();
    h.check(true, 'a row opens that document');
    await openEditor('Look check A');
  });

  await S('History shows the changes to a code and the version of the codebook that was in effect', async () => {
    await rpc('codebook.snapshot', { note: 'editor check' });
    await page.getByRole('button', { name: 'Back to the coding workspace' }).first().click();
    await openEditor('Look check A');
    await form().getByRole('textbox', { name: 'Description' }).fill('A description written after the version was saved.');
    await form().getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Look check A\./);
    await tab('History').click();
    const events = page.getByTestId('code-events');
    await events.waitFor();
    const text = await events.innerText();
    h.check(text.trim().length > 0 && !/code\.\w+|\.update/.test(text), `the changes are in words: ${text.split('\n').slice(0, 2).join(' | ')}`);
    const versions = page.getByTestId('code-versions');
    await versions.waitFor();
    const vtext = await versions.innerText();
    h.check(/Version \d+: editor check/.test(vtext), 'the version that was saved is listed with its note');
    h.check(/The first code the look check uses\./.test(vtext), 'with the description that was in effect then, not today\'s');
    h.check(!/A description written after/.test(vtext), 'which is not the one written afterwards');
    await axe('the History tab');
  });

  await S('Compare shows what changed between a saved version and the codebook today, and Cancel leaves everything as it was', async () => {
    await page.getByRole('button', { name: 'Compare' }).click();
    const dialog = page.getByRole('dialog', { name: 'Compare codebook versions' });
    await dialog.waitFor();
    await dialog.getByTestId('compare-summary').waitFor();
    h.equal((await dialog.getByLabel('Later').inputValue()), 'today', 'the later side is the codebook today');
    const summary = await dialog.getByTestId('compare-summary').innerText();
    h.check(/1 code changed/.test(summary), `one code changed: ${summary}`);
    const changed = await dialog.getByRole('region', { name: 'Changed' }).innerText().catch(async () => await dialog.getByTestId('compare-result').innerText());
    h.check(/Look check A/.test(changed) && /Description/.test(changed) && /The first code the look check uses\./.test(changed) && /A description written after/.test(changed), 'it names the code, the field, and both descriptions');
    await axe('the Compare dialog');
    // The same version against itself has nothing to show.
    await dialog.getByLabel('Later').selectOption({ index: 1 });
    await page.waitForFunction(() => /No differences/.test(document.querySelector('[data-testid=compare-summary]')?.textContent ?? ''));
    h.check(true, 'a version against itself has no differences');
    await dialog.getByRole('button', { name: 'Close' }).click();
    await dialog.waitFor({ state: 'hidden' });
    const code = await byName('Look check A');
    h.equal(code.definition, 'A description written after the version was saved.', 'comparing changed nothing');
  });

  await S('the editor works without a pointer: the tree moves with the arrow keys and the tabs with theirs', async () => {
    await tab('Code Details').click();
    await codeRow('Look check A').focus();
    const first = await form().getByRole('textbox', { name: 'Title' }).inputValue();
    await page.keyboard.press('ArrowDown');
    await page.waitForFunction((was) => document.querySelector('#code-name')?.value !== was, first);
    h.check(true, 'ArrowDown in the tree chose another code and loaded it');
    h.check(await page.evaluate(() => !!document.activeElement?.closest('[role=treegrid]')), 'the focus stayed in the tree');
    // Tab leaves the tree for the tabs, and the arrow keys move between them.
    let onTab = false;
    for (let i = 0; i < 8 && !onTab; i++) {
      await page.keyboard.press('Tab');
      onTab = await page.evaluate(() => document.activeElement?.getAttribute('role') === 'tab');
    }
    h.check(onTab, 'Tab reaches the tabs');
    await page.keyboard.press('ArrowRight');
    h.equal(await tab(/^Examples/).getAttribute('aria-selected'), 'true', 'ArrowRight chooses the next tab');
    h.check(await page.evaluate(() => getComputedStyle(document.activeElement).outlineStyle !== 'none'), 'and the focus is visible');
    await leaveEditor();
    h.check((await page.getByRole('row', { name: /^Look check A|^Look check B/ }).count()) >= 1, 'the way back returns to the coding workspace and its Codes');
  });

  // ---- the code properties built in alpha 12: groups, rated codes, and counterexamples -------------------

  const surfaceBox = () => page.getByTestId('surface');
  const toneRow = () => page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /tone-check/ }).first();
  /** A small document of its own, so these steps do not depend on what the earlier steps left in the project. */
  async function toneSource() {
    let found = (await rpc('source.list')).find((s) => s.name === 'tone-check');
    if (!found) {
      const file = path.join(ctx.workDir, 'tone-check.txt');
      fs.writeFileSync(file, 'I felt welcome from the first day. The staff greeted me by name.\n', 'utf8');
      await rpc('import.run', { path: file });
      found = (await rpc('source.list')).find((s) => s.name === 'tone-check');
    }
    return found;
  }
  /** The words of its first sentence, as a span (and its unit, when the project codes whole units). */
  async function toneSpan(source) {
    if ((await rpc('project.info')).coding_mode === 'unit') {
      const unit = (await rpc('unit.list', { source_id: source.id })).find((u) => !u.excluded);
      return { start: unit.start, end: unit.end, unit_id: unit.id };
    }
    const text = (await rpc('source.get', { id: source.id })).text;
    const phrase = 'I felt welcome from the first day.';
    const start = Array.from(text.slice(0, text.indexOf(phrase))).length;
    return { start, end: start + Array.from(phrase).length };
  }
  const replaceDialog = () => page.getByRole('dialog', { name: 'Replace the other code?' });

  await S('a code can be put in a mutually exclusive group, joining by name whatever the capitals, and the editor names the other members', async () => {
    await rpc('code.add', { name: 'Tone warm' });
    await rpc('code.add', { name: 'Tone cold' });
    await page.reload();
    await openEditor('Tone warm');
    const group = () => form().getByRole('combobox', { name: /^Mutually Exclusive Group/ });
    h.equal(await group().inputValue(), '', 'a new code is in no group');
    await group().fill('Tone check');
    await form().getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Tone warm/);
    h.equal((await byName('Tone warm')).exclusive_group, 'Tone check', 'the engine has the group');
    await page.getByTestId('group-mates').waitFor();
    h.check(/only code in the group Tone check/.test(await page.getByTestId('group-mates').innerText()), 'alone in a group, it says so');
    await codeRow('Tone cold').click();
    await group().fill('tone check');
    await form().getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Tone cold/);
    h.equal((await byName('Tone cold')).exclusive_group, 'Tone check', 'a group is joined by name whatever the capitals, in the spelling it already has');
    h.equal(await group().inputValue(), 'Tone check', 'and the box shows that spelling');
    await page.waitForFunction(() => /In the group Tone check with Tone warm/.test(document.querySelector('[data-testid=group-mates]')?.textContent ?? ''));
    h.check(true, 'the editor names the other member of the group');
    h.check(await group().evaluate((el) => [...el.list.options].some((o) => o.value === 'Tone check')), 'existing group names are offered as suggestions');
    h.check(await form().getByRole('button', { name: 'Save changes' }).isDisabled(), 'nothing is waiting to be saved');
    await axe('the Codebook Editor with a mutually exclusive group');
  });

  await S('choosing a second code of the group on the same words asks first, Cancel changes nothing, and Replace swaps them', async () => {
    const warm = await byName('Tone warm');
    const cold = await byName('Tone cold');
    const source = await toneSource();
    const span = await toneSpan(source);
    await rpc('coding.apply', { source_id: source.id, ...span, code_id: warm.id });
    await leaveEditor();
    await page.reload();
    await mainTab('Coding').waitFor();
    await toneRow().click();
    await surfaceBox().waitFor();
    await api.selectByMouse('felt welcome');
    await codeRow('Tone cold').dblclick();
    await replaceDialog().waitFor();
    const said = await replaceDialog().getByTestId('replace-message').innerText();
    h.check(/'Tone cold' is in the group 'Tone check', which allows one code at a time, and these words already have 'Tone warm'\./.test(said), `the dialog says why, in the engine's words: ${said}`);
    await axe('the Replace dialog');
    await replaceDialog().getByRole('button', { name: 'Cancel' }).click();
    await replaceDialog().waitFor({ state: 'hidden' });
    let codings = (await rpc('coding.list', { source_id: source.id })).filter((c) => c.code_id === warm.id || c.code_id === cold.id);
    h.equal(codings.map((c) => c.code_id), [warm.id], 'Cancel left the first coding alone and made no second one');
    await api.selectByMouse('felt welcome');
    await codeRow('Tone cold').dblclick();
    await replaceDialog().waitFor();
    await replaceDialog().getByRole('button', { name: 'Replace' }).click();
    await replaceDialog().waitFor({ state: 'hidden' });
    await waitStatus(/as Tone cold/);
    codings = (await rpc('coding.list', { source_id: source.id })).filter((c) => c.code_id === warm.id || c.code_id === cold.id);
    h.equal(codings.map((c) => c.code_id), [cold.id], 'Replace took the first away and applied the second');
    const log = (await rpc('events.list', { limit: 10 })).map((x) => x.action);
    h.check(log.includes('coding.remove') && log.filter((a) => a === 'coding.add').length >= 1, `both steps are on the activity log: ${log.slice(-4).join(', ')}`);
    // The same code on the same words is not a clash, and words of another code are free of the group.
    await rpc('code.add', { name: 'Tone outside' });
    const outside = await byName('Tone outside');
    await rpc('coding.apply', { source_id: source.id, ...span, code_id: outside.id });
    h.equal((await rpc('coding.list', { source_id: source.id })).filter((c) => c.code_id === outside.id).length, 1, 'a code outside the group can share the words');
  });

  await S('Weighted / Rated is turned on with a described scale, Segment Details follows it, and turning it off says what it would clear', async () => {
    await page.reload();
    await mainTab('Coding').waitFor();
    await openEditor('Tone cold');
    await page.locator('label').filter({ has: page.getByRole('checkbox', { name: /^Weighted \/ Rated/ }) }).click();
    await form().getByRole('textbox', { name: 'What the rating measures' }).fill('Warmth');
    await form().getByRole('textbox', { name: 'A 1 means' }).fill('Cool');
    await form().getByRole('textbox', { name: 'A 5 means' }).fill('Glowing');
    await axe('the Codebook Editor with a rating scale');
    await form().getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Tone cold/);
    const cold = await byName('Tone cold');
    h.equal([cold.rated, cold.rating_label, cold.rating_low, cold.rating_high], [true, 'Warmth', 'Cool', 'Glowing'], 'the engine has the scale');
    // Open the coding in Segment Details by clicking its stripe in the document, and rate it.
    const source = await toneSource();
    const toneCoding = async () => (await rpc('coding.list', { source_id: source.id })).find((c) => c.code_id === cold.id);
    await leaveEditor();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row').nth(1).waitFor();
    await toneRow().click();
    await surfaceBox().waitFor();
    await page.locator(`.stripe[data-coding="${(await toneCoding()).id}"]`).click();
    const detail = page.getByTestId('coding-detail');
    await detail.waitFor();
    h.equal((await detail.getByTestId('rating-label').innerText()).trim(), 'Warmth', 'Segment Details names what the rating measures');
    h.check(/1 = Cool\s+·\s+5 = Glowing/.test(await detail.getByTestId('rating-scale').innerText()), 'and what the two ends mean');
    await detail.getByRole('radio', { name: '4 stars out of 5' }).click();
    await page.waitForFunction(() => /\(4 \/ 5\)/.test(document.querySelector('[data-testid=rating-value]')?.textContent ?? ''));
    h.equal((await toneCoding()).rating, 4, 'the engine has the rating');
    await axe('Segment Details with a described rating');
    // Switching rating off while a coding carries a rating is refused, with a way to clear them together.
    await openEditor('Tone cold');
    await page.locator('label').filter({ has: page.getByRole('checkbox', { name: /^Weighted \/ Rated/ }) }).click();
    await form().getByRole('button', { name: 'Save changes' }).click();
    await form().getByTestId('clear-ratings-offer').waitFor();
    h.check(/1 coding of this code carries a rating/.test(await form().textContent()), 'the refusal says how many codings carry a rating');
    h.equal((await byName('Tone cold')).rated, true, 'nothing changed yet');
    await form().getByRole('button', { name: 'Clear the ratings and turn rating off' }).click();
    await waitStatus(/Saved the code Tone cold/);
    h.equal([(await byName('Tone cold')).rated, (await toneCoding()).rating], [false, null], 'rating is off and the rating is cleared, not zero');
    await leaveEditor();
    await toneRow().click();
    await surfaceBox().waitFor();
    await page.locator(`.stripe[data-coding="${(await toneCoding()).id}"]`).click();
    await page.getByTestId('rating-off').waitFor();
    h.check(/not a rated code/.test(await page.getByTestId('rating-off').innerText()), 'a coding of a code that is not rated says so and does not show stars');
    h.equal(await page.getByTestId('rating').count(), 0, 'with no stars to set');
  });
}
