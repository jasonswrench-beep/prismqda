// Codebook management, memos, history, and the shortcut sheet.

export async function runMore(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe, waitStatus, surface, codeRow, mainTab, addCodeThroughDialog, sleep, codeAction, codeToggle, openDefinition, closeDefinition } = api;

  const codes = () => rpc('code.list');
  const byName = async (name) => (await codes()).find((c) => c.name === name);
  const rowNames = () => page.getByRole('treegrid', { name: 'Codes' }).getByRole('row').evaluateAll((rows) => rows.map((r) => r.getAttribute('aria-label').split(',')[0]));
  const button = (name) => page.getByRole('button', { name, exact: true });

  await S('a code can be edited: a color changes the moment it is chosen, a definition is saved with the button', async () => {
    await openDefinition('Cost');
    const form = page.getByTestId('code-detail');
    await form.waitFor();
    const definition = form.getByRole('textbox', { name: 'Description' });
    await definition.fill('Money worries and what things cost.');
    // Choosing a color is the whole action: there is no Save to forget, and words typed in the other boxes stay.
    await form.getByText('Pink', { exact: true }).click();
    await waitStatus(/Cost is now Pink/);
    h.equal((await byName('Cost')).color.toUpperCase(), '#CC79A7', 'the color was saved at once');
    h.equal(await definition.inputValue(), 'Money worries and what things cost.', 'words typed in the other boxes were kept');
    h.equal((await byName('Cost')).definition, '', 'and the definition itself still waits for Save changes');
    await form.getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code Cost/);
    h.equal((await byName('Cost')).definition, 'Money worries and what things cost.', 'definition saved');
    h.check(!/no definition yet/.test(await page.getByTestId('code-detail').textContent()), 'the reminder is gone');
    // Every color the codebook offers can be picked, including the ones past the first eight.
    const names = await form.getByRole('radio').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label') ?? e.closest('label')?.textContent?.trim()));
    h.check(names.length >= 16, `at least sixteen colors to choose from: ${names.length}`);
    await form.getByText('Teal', { exact: true }).click();
    await waitStatus(/Cost is now Teal/);
    h.equal((await byName('Cost')).color.toUpperCase(), '#1B998B', 'a later color works too');
    await form.getByText('Blue', { exact: true }).click();
    await waitStatus(/Cost is now Blue/);
    h.equal((await byName('Cost')).color.toUpperCase(), '#0072B2', 'and back to Blue');
    await axe('the code definition window');
    await closeDefinition();
  });

  await S('a sub-code can be added under a code', async () => {
    await codeRow('Welcome').click();
    await codeAction('Add sub-code');
    const dialog = page.getByRole('dialog', { name: 'Add a code' });
    await dialog.waitFor();
    h.check((await dialog.getByLabel('Place it under').inputValue()) === String((await byName('Welcome')).id), 'the parent is already chosen');
    await axe('the add code dialog');
    await dialog.getByRole('textbox', { name: 'Name' }).fill('Warm greeting');
    await dialog.getByRole('button', { name: 'Add code' }).click();
    await dialog.waitFor({ state: 'hidden' });
    await codeRow('Warm greeting').waitFor();
    h.equal((await byName('Warm greeting')).parent_id, (await byName('Welcome')).id, 'parent in the engine');
    h.equal(await codeRow('Warm greeting').getAttribute('aria-level'), '2', 'it is shown one level down');
  });

  await S('reorganizing with buttons: move down, move up with the keyboard, indent, outdent', async () => {
    h.equal((await rowNames()).slice(0, 2), ['Cost', 'Naïve plans'], 'the starting order');
    await codeRow('Cost').click();
    await codeAction('Move down');
    await waitStatus(/Moved Cost down one place/);
    h.equal((await rowNames()).slice(0, 2), ['Naïve plans', 'Cost'], 'Cost moved down');
    // The same with the keyboard: focus is on the row, Alt+Up moves it back.
    await codeRow('Cost').focus();
    await page.keyboard.press('Alt+ArrowUp');
    await waitStatus(/Moved Cost up one place/);
    h.equal((await rowNames()).slice(0, 2), ['Cost', 'Naïve plans'], 'Cost moved back up with Alt+Up');
    h.check(await page.evaluate(() => document.activeElement?.closest('[role=row]')?.getAttribute('aria-label')?.startsWith('Cost,')), 'the focus stayed on the moved code');
    // Indent: Staff goes under the code above it.
    await codeRow('Staff').click();
    await codeAction('Make sub-code');
    await waitStatus(/Moved Staff under/);
    const staff = await byName('Staff');
    h.check(staff.parent_id !== null, `Staff is now a sub-code (parent ${staff.parent_id})`);
    await codeAction('Move out');
    await waitStatus(/Moved Staff to the top level/);
    h.equal((await byName('Staff')).parent_id, null, 'Staff is back at the top level');
    await page.keyboard.press('Escape');
  });

  // The browser's own drag and drop does not reach the page in the Windows window, so a code is dragged with pointer events
  // (components/pointerDrag.ts, decision 169); the check drives a real mouse, as a hand does.
  await S('a code is dragged onto another with the mouse to make it a sub-code, before another to put it there, and the drag can be cancelled', async () => {
    const welcome = await byName('Welcome');
    const grab = async (name, target, where, release = true) => {
      const a = await codeRow(name).locator('.code-name').boundingBox();
      await page.mouse.move(a.x + 4, a.y + a.height / 2);
      await page.mouse.down();
      await page.mouse.move(a.x + 30, a.y + a.height / 2 + 5, { steps: 3 });
      const r = await codeRow(target).boundingBox();
      const y = where === 'on' ? r.y + r.height / 2 : where === 'before' ? r.y + 2 : r.y + r.height - 2;
      await page.mouse.move(r.x + r.width / 3, y, { steps: 6 });
      if (release) await page.mouse.up();
    };
    // Escape stops a drag, and nothing moves
    await grab('Staff', 'Welcome', 'on', false);
    h.equal(await codeRow('Welcome').getAttribute('data-drop-over'), 'on', 'the row under the pointer says it would take the code as a sub-code');
    h.equal(await page.locator('.drag-ghost').count(), 1, 'and the code\'s name follows the pointer');
    await page.keyboard.press('Escape');
    await page.mouse.up();
    await sleep(200);
    h.equal((await byName('Staff')).parent_id, null, 'Escape cancels: Staff did not move');
    h.equal(await page.locator('.drag-ghost').count(), 0, 'and nothing follows the pointer');
    // on: a sub-code
    await grab('Staff', 'Welcome', 'on');
    await sleep(400);
    h.equal((await byName('Staff')).parent_id, welcome.id, 'dragged under Welcome');
    // before: out of it, to the place before Welcome
    await grab('Staff', 'Welcome', 'before', false);
    h.equal(await codeRow('Welcome').getAttribute('data-drop-over'), 'before', 'the top of a row says it would put the code before it');
    await page.mouse.up();
    await sleep(400);
    h.equal((await byName('Staff')).parent_id, null, 'dropped before Welcome, Staff is a code of its own level again');
    // Put it back for the steps that follow.
    await codeRow('Staff').click();
    if ((await byName('Staff')).parent_id !== null) {
      await codeAction('Move out');
      await waitStatus(/Moved Staff/);
    }
  });

  await S('codes can be merged: codings and anchors move, the old code is kept as merged', async () => {
    const text = (await rpc('source.get', { id: 1 })).text;
    await addCodeThroughDialog('Price');
    const price = await byName('Price');
    const start = Array.from(text.slice(0, text.indexOf('Everything here'))).length;
    await rpc('coding.apply', { source_id: 1, start, end: start + 10, code_id: price.id });
    await page.reload();
    await mainTab('Coding').waitFor();
    await codeRow('Price').waitFor();
    const before = (await byName('Cost')).usage;
    await codeRow('Price').click();
    await codeAction('Merge…');
    const dialog = page.getByRole('dialog', { name: /Merge Price/ });
    await dialog.waitFor();
    await axe('the merge dialog');
    await dialog.getByLabel('Merge into').selectOption({ label: 'Cost' });
    await dialog.getByRole('button', { name: 'Merge', exact: true }).click();
    const said = await waitStatus(/Merged Price into Cost/);
    h.check(/1 coding moved/.test(said), `announced: ${said}`);
    const after = await codes();
    h.equal(after.find((c) => c.name === 'Price').status, 'merged', 'Price is merged');
    h.equal(after.find((c) => c.name === 'Cost').usage, before + 1, 'Cost took the coding');
    h.check((await codeRow('Price').count()) === 0, 'merged codes are hidden by default');
  });

  await S('a code can be split: chosen codings move to a new code', async () => {
    await codeRow('Cost').click();
    await codeAction('Split…');
    const dialog = page.getByRole('dialog', { name: /Split Cost/ });
    await dialog.waitFor();
    await axe('the split dialog');
    // Asking to split with nothing chosen explains what is missing.
    await dialog.getByRole('textbox', { name: 'Name of the new code' }).fill('Cost: household');
    await dialog.getByRole('button', { name: 'Split', exact: true }).click();
    await dialog.getByRole('alert').waitFor();
    h.check(/at least one coding/.test(await dialog.getByRole('alert').textContent()), 'a plain message when nothing is chosen');
    await dialog.getByText(/hard part/).first().click();
    await dialog.getByRole('button', { name: 'Split', exact: true }).click();
    await dialog.waitFor({ state: 'hidden' });
    const split = await byName('Cost: household');
    h.equal(split.usage, 1, 'the new code has the chosen coding');
    h.equal(split.parent_id, (await byName('Cost')).parent_id, 'it sits beside the original');
  });

  await S('a code can be retired and restored; retired codes cannot be applied', async () => {
    await codeRow('Warm greeting').click();
    await codeAction('Retire');
    await waitStatus(/Retired Warm greeting/);
    h.check((await codeRow('Warm greeting').count()) === 0, 'retired codes are hidden by default');
    h.check(await codeToggle('Show retired and merged codes'), 'the menu item for retired and merged codes is checked once pressed');
    await codeRow('Warm greeting').waitFor();
    h.check((await codeRow('Warm greeting').textContent()).includes('retired'), 'the row says it is retired');
    await codeRow('Warm greeting').click();
    await codeAction('Apply to selection');
    await page.getByTestId('alert').filter({ hasText: /retired/ }).waitFor();
    h.check(true, 'applying a retired code is refused with a message');
    await axe('the codebook with retired and merged codes shown');
    await codeAction('Restore');
    await waitStatus(/Restored Warm greeting/);
    h.equal((await byName('Warm greeting')).status, 'active', 'restored in the engine');
    h.check(!(await codeToggle('Show retired and merged codes')), 'turning it off hides them again');
  });

  await S('the codebook can be searched and a version can be saved', async () => {
    // The search box opens from the magnifier in the Codes header.
    await page.getByRole('button', { name: 'Search codes' }).click();
    const box = page.getByRole('searchbox', { name: 'Find a code' });
    await box.fill('wel');
    await page.waitForFunction(() => document.querySelectorAll('[role=treegrid][aria-label="Codes"] [role=row]').length <= 3);
    const names = await rowNames();
    h.check(names.includes('Welcome') && !names.includes('Staff'), `only matching codes and their parents remain: ${names.join(', ')}`);
    await box.fill('');
    await page.waitForFunction(() => document.querySelectorAll('[role=treegrid][aria-label="Codes"] [role=row]').length >= 5);
    // With nothing typed, the magnifier closes the box again.
    await page.getByRole('button', { name: 'Search codes' }).click();
    await box.waitFor({ state: 'hidden' });
    await codeAction('Save version…');
    const dialog = page.getByRole('dialog', { name: /Save a version/ });
    await axe('the save version dialog');
    await dialog.getByRole('textbox', { name: 'Note' }).fill('after pilot');
    await dialog.getByRole('button', { name: 'Save version' }).click();
    await waitStatus(/Saved codebook version 1/);
    h.equal((await rpc('codebook.versions'))[0].note, 'after pilot', 'the version is in the engine');
  });

  // ------------------------------------------------------------------------------ memos
  await S('memos: add for the project, the source, a code, and a coding; edit; delete', async () => {
    await mainTab('Memos').click();
    await page.getByRole('heading', { name: 'Memos', level: 2 }).waitFor();
    const area = page.getByTestId('memos-panel');
    await area.waitFor();
    // Default is the most specific thing selected (here: a coding), so choose each in turn.
    const aboutSelect = page.getByLabel('Memos about');
    /** Chooses the option whose text matches (a string for an exact match, or a pattern). */
    const about = {
      async selectOption({ label }) {
        const texts = await aboutSelect.locator('option').allTextContents();
        const found = typeof label === 'string' ? label : texts.find((t) => label.test(t));
        await aboutSelect.selectOption({ label: found });
      },
      locator: (s) => aboutSelect.locator(s),
    };
    const addMemo = async (kind, title, body) => {
      await area.getByLabel('Kind of memo').selectOption({ label: kind });
      await area.getByRole('textbox', { name: 'Title' }).first().fill(title);
      await area.getByRole('textbox', { name: 'Memo', exact: true }).first().fill(body);
      await area.getByRole('button', { name: 'Add memo' }).click();
      await waitStatus(new RegExp(`Added the memo ${title}`));
    };
    await about.selectOption({ label: 'The whole project' });
    await addMemo('Method decision', 'Coding approach', 'We code selected words first.');
    await about.selectOption({ label: /The open source/ });
    await addMemo('Reflexive journal', 'My stance', 'I notice I expected cost to matter.');
    const optionTexts = await about.locator('option').allTextContents();
    h.check(optionTexts.some((t) => /The selected code/.test(t)), 'a code is selected, so its memos are offered');
    await about.selectOption({ label: /The selected code/ });
    await addMemo('Analytic', 'On this code', 'It overlaps with another code.');
    const memos = await rpc('memo.list');
    h.equal(memos.length, 3, 'three memos in the engine');
    h.equal(memos.map((m) => m.scope).sort(), ['code', 'project', 'source'], 'each memo has its scope');
    h.equal(memos.map((m) => m.kind).sort(), ['analytic', 'method_decision', 'reflexive'], 'each memo has its kind');
    await about.selectOption({ label: 'Every memo in the project' });
    await page.getByText('On this code').first().waitFor();
    await area.getByText(/3 memos in this project/).waitFor();
    h.check((await area.getByText(/3 memos in this project/).count()) === 1, 'the list shows all three');
    await axe('the memos page');
    // Edit one.
    const item = area.locator('li.memo-item[data-memo-title="My stance"]');
    await item.getByRole('textbox', { name: 'Title' }).fill('My stance, revised');
    await item.getByRole('button', { name: 'Save memo' }).click();
    await waitStatus(/Saved the memo/);
    h.check((await rpc('memo.list')).some((m) => m.title === 'My stance, revised'), 'the edit was saved');
    // Delete one, with a second step to confirm.
    const doomed = area.locator('li.memo-item[data-memo-title="Coding approach"]');
    await doomed.getByRole('button', { name: 'Delete…' }).click();
    await doomed.getByRole('button', { name: 'Yes, delete this memo' }).click();
    await waitStatus(/Deleted the memo/);
    h.equal((await rpc('memo.list')).length, 2, 'one memo left');
    await mainTab('Coding').click();
    // A memo about a coding: Segment Details has a Memo section, and its plus opens the memos page for that coding.
    await page.locator('.stripe[data-coding]').first().click();
    await page.getByRole('button', { name: 'Write a memo about this coding' }).click();
    await page.getByRole('heading', { name: 'Memos', level: 2 }).waitFor();
    const panel = page.getByTestId('memos-panel');
    await axe('the memos page, with a coding chosen');
    await panel.getByLabel('Kind of memo').selectOption({ label: 'To-do' });
    await panel.getByRole('textbox', { name: 'Title' }).first().fill('Check this quote');
    await panel.getByRole('button', { name: 'Add memo' }).click();
    await waitStatus(/Added the memo Check this quote/);
    h.check((await rpc('memo.list')).some((m) => m.scope === 'coding' && m.kind === 'todo'), 'a coding memo with the to-do kind');
    await mainTab('Coding').click();
    // Back in Segment Details, the memo about this coding is listed.
    await page.locator('.stripe[data-coding]').first().click();
    await page.getByRole('button', { name: 'Open the memos about this coding' }).waitFor();
    h.check(/Check this quote/.test(await page.getByTestId('coding-detail').textContent()), 'Segment Details lists the memo about the coding');
  });

  // ------------------------------------------------------------------------------ history
  await S('history: plain-language timeline, filter, and an added note', async () => {
    await api.openActivity();
    await page.getByRole('heading', { name: 'History', level: 2 }).waitFor();
    const timeline = page.getByTestId('timeline');
    await timeline.locator('li').first().waitFor();
    // Quotes arrive once the sources' text is loaded.
    await page.waitForFunction(() => /Coded 'The staff greeted me' as Staff/.test(document.querySelector('[data-testid=timeline]').textContent));
    const text = await timeline.textContent();
    h.check(text.includes("Coded 'The staff greeted me' as Staff"), 'a coding is described in plain words');
    h.check(text.includes("Coded 'felt welcome' as Welcome") || text.includes("Removed the coding 'felt welcome' (Welcome)"), 'the first coding and its removal are both there');
    h.check(text.includes('Added the code Staff'), 'adding a code is described');
    h.check(/Merged the code Price into Cost/.test(text), 'a merge is described');
    h.check(/Split the code Cost/.test(text), 'a split is described');
    h.check(/Saved codebook version 1: after pilot/.test(text), 'a codebook version is described');
    h.check(/Imported the source 'interview'/.test(text), 'an import is described');
    h.check(!/coding\.add|code\.merge|\.rationale/.test(text), 'no raw action names leak into the timeline');
    h.check(/Method decision/.test(text), 'a method-decision memo is marked as a milestone');
    // Newest first.
    const first = await timeline.locator('li').first().textContent();
    h.check(/memo|Added|Deleted/.test(first), `the newest entry comes first: ${first.slice(0, 80)}`);
    await axe('the history page');
    // Filter.
    await page.getByLabel('Show', { exact: true }).selectOption({ label: 'Memos' });
    const memoOnly = await timeline.locator('li').allTextContents();
    h.check(memoOnly.length > 0 && memoOnly.every((t) => /memo/i.test(t)), 'the filter keeps only memo entries');
    await page.getByLabel('Show', { exact: true }).selectOption({ label: 'Everything' });
    // A note.
    await page.getByRole('textbox', { name: 'Add a note to the history' }).fill('Moved to unit coding after the pilot.');
    await page.getByRole('button', { name: 'Add note' }).click();
    await waitStatus(/Added the note to the history/);
    await page.waitForFunction(() => /Note: Moved to unit coding after the pilot\./.test(document.querySelector('[data-testid=timeline]').textContent));
    h.check((await rpc('events.list', { limit: 5 })).some((e) => e.action === 'annotation'), 'the engine logged the annotation');
    await mainTab('Coding').click();
  });

  // ------------------------------------------------------------------------------ help
  await S('the shortcut sheet opens with F1, can be printed, and closes with Escape', async () => {
    await surface().focus();
    await page.keyboard.press('F1');
    const dialog = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
    await dialog.waitFor();
    const text = await dialog.textContent();
    for (const needle of ['1 to 9', 'In-vivo', 'Repeat the last code', 'next or previous unit', 'F6', 'Alt+Up', 'Shift+Arrow']) {
      h.check(text.includes(needle), `the sheet mentions ${needle}`);
    }
    h.check((await dialog.getByRole('button', { name: 'Print this list' }).count()) === 1, 'a print button');
    h.check((await dialog.locator('table').count()) >= 4, 'the sheet is made of real tables');
    await axe('the shortcut sheet');
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
    h.check(await surface().evaluate((el) => document.activeElement === el), 'focus returns to the text');
    // The print style hides the app and keeps the sheet.
    await page.keyboard.press('F1');
    await dialog.waitFor();
    await page.emulateMedia({ media: 'print' });
    const printed = await page.evaluate(() => ({
      header: getComputedStyle(document.querySelector('.app-tabs')).display,
      dialog: getComputedStyle(document.querySelector('.help-dialog')).display,
      button: getComputedStyle(document.querySelector('.no-print')).display,
    }));
    h.equal(printed.header, 'none', 'the app is hidden when printing');
    h.check(printed.dialog !== 'none', 'the sheet is kept when printing');
    h.equal(printed.button, 'none', 'the buttons are not printed');
    await page.emulateMedia({ media: 'screen' });
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
  });

  await S('F6 moves between the main areas and announces where the focus went', async () => {
    await surface().focus();
    const order = [];
    for (let i = 0; i < 5; i++) {
      await page.keyboard.press('F6');
      await sleep(120);
      order.push(await page.evaluate(() => document.activeElement?.id));
    }
    h.equal(order, ['pane-retrieved', 'pane-details', 'pane-sources', 'codebook-section', 'main-content'], 'F6 cycles through the areas');
    await page.keyboard.press('Shift+F6');
    await sleep(120);
    h.equal(await page.evaluate(() => document.activeElement?.id), 'codebook-section', 'Shift+F6 goes back');
    h.check(/Now in/.test(await waitStatus(/Now in/)), 'the move is announced');
  });
}
