// The frame: the ribbon tabs and their commands, the pages that open in the work area, the Ctrl+K box, undo, the
// overview, the report page, and Retrieved Segments.

import path from 'node:path';

export async function runFrame(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe, waitStatus, mainTab, sleep, surface } = api;

  const ribbon = () => page.getByRole('toolbar', { name: 'Commands' });
  const ribbonButton = (name) => ribbon().getByRole('button', { name, exact: true });
  const exportDialog = () => page.getByRole('dialog', { name: 'Export', exact: true });
  const ribbonTab = (name) => page.getByRole('tab', { name, exact: true });
  /** Shows one tab of the ribbon (only the commands change). */
  const showTab = async (name) => {
    await ribbonTab(name).click();
    await page.getByRole('region', { name: 'Command ribbon' }).waitFor();
  };

  async function firstWord() {
    const sources = await rpc('source.list');
    for (const s of sources) {
      const text = Array.from((await rpc('source.get', { id: s.id })).text);
      const units = await rpc('unit.list', { source_id: s.id });
      for (const u of units.filter((x) => !x.excluded).slice(0, 20)) {
        const m = /[A-Za-z]{6,}/.exec(text.slice(u.start, u.end).join(''));
        if (m) return { sourceId: s.id, word: m[0] };
      }
    }
    throw new Error('no source with a word to search for');
  }

  /** Whatever a step leaves open must not get in the way of the next one. */
  async function closeDialogs() {
    for (let i = 0; i < 3 && (await page.getByRole('dialog').count()) > 0; i++) {
      await page.keyboard.press('Escape');
      await sleep(80);
    }
  }

  await S('the fourteen ribbon tabs only change which commands are on show', async () => {
    await closeDialogs();
    await mainTab('Coding').click();
    // A small document is open while the screen is checked, so the check does not have to walk 380,000 characters.
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).click();
    await surface().waitFor();
    const tabs = (await page.getByRole('tablist', { name: 'Ribbon tabs' }).getByRole('tab').allTextContents()).map((t) => t.trim());
    h.equal(tabs, ['Home', 'Import', 'Code', 'Memos', 'Explore', 'Mixed Methods', 'Visual Tools', 'Synthesize', 'Text Analytics', 'Statistics', 'Report', 'Team', 'AI', 'View'], 'the tabs are the book\'s, in its order');
    h.equal(await ribbonTab('Home').getAttribute('aria-selected'), 'true', 'Home is the tab that is chosen');
    const names = async () => (await ribbon().getByRole('button').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label') ?? e.textContent.trim()))).filter((n) => !/^More ways/.test(n));
    const home = await names();
    const expected = ['New Project', 'Open Project', 'Save (Ctrl+S)', 'Add Documents', 'New Code', 'New Memo', 'New Case', 'Code Selection', 'Auto Code', 'Code In Vivo', 'Quick Code (Ctrl+D)', 'Code Summary', 'Code Relations', 'Query', 'Visualize', 'Compare Cases', 'Report Builder'];
    h.equal(home, expected, 'the Home ribbon is the book\'s, button for button');
    const groups = await ribbon().getByRole('group').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    h.equal(groups, ['Project', 'Documents', 'Add', 'Coding', 'Explore', 'Analyze & Report'], 'the groups carry the book\'s captions');
    await page.screenshot({ path: path.join(ctx.shots, 'frame-home-ribbon.png') });
    await axe('the Home ribbon');
    // Another tab changes the commands and nothing else: the open document stays.
    const doc = await page.getByTestId('surface').evaluate((el) => el.textContent.length);
    await showTab('Code');
    h.check((await names()).some((n) => /^Undo/.test(n)) && (await names()).some((n) => /^Previous Uncoded/.test(n)), 'the Code tab has Undo and the uncoded jumps');
    h.equal(await page.getByTestId('surface').evaluate((el) => el.textContent.length), doc, 'the document is still open and unchanged');
    h.equal(await page.locator('[role=tab][aria-selected=true]').evaluateAll((els) => els.filter((e) => e.closest('[aria-label="Ribbon tabs"]')).length), 1, 'exactly one tab is chosen');
    // Mixed Methods leads to its page, and to the two pages whose variables it makes and uses.
    await showTab('Mixed Methods');
    const mixed = await names();
    for (const n of ['Mixed Methods page', 'Statistics page', 'Data editor']) h.check(mixed.includes(n), `the Mixed Methods tab has ${n}`);
    h.check(!/not built yet/.test((await page.getByRole('region', { name: 'Command ribbon' }).textContent()) ?? ''), 'and no longer says it is not built');
    await axe('the Mixed Methods ribbon tab');
    await showTab('View');
    const view = await names();
    for (const n of ['Documents', 'Codes', 'Retrieved Segments', 'Segment Details', 'Reset', 'Settings']) h.check(view.includes(n), `the View tab has ${n}`);
    await showTab('Home');
  });

  await S('the pages open in the work area, the ribbon stays, and a bar leads back', async () => {
    await closeDialogs();
    await mainTab('Home').click();
    await page.getByRole('heading', { level: 2 }).first().waitFor();
    h.check(await ribbon().isVisible(), 'the ribbon is there on the overview');
    const back = page.getByRole('button', { name: 'Back to the coding workspace' });
    h.equal(await back.count(), 1, 'the way back is on the page');
    await page.screenshot({ path: path.join(ctx.shots, 'frame-home.png') });
    await axe('the project overview');
    await mainTab('Data').click();
    await page.getByRole('heading', { name: 'Data', level: 2 }).waitFor();
    h.check(await ribbon().isVisible(), 'the ribbon is there on the data table');
    const listed = await rpc('source.list');
    h.equal(await page.getByRole('table', { name: 'Sources in this project' }).locator('tbody tr').count(), listed.length, 'the data table lists every source');
    await page.screenshot({ path: path.join(ctx.shots, 'frame-data.png') });
    await axe('the data table page');
    await mainTab('Synthesize').click();
    await page.getByRole('heading', { name: 'Memos', level: 2 }).waitFor();
    await mainTab('Report').click();
    await page.getByRole('heading', { name: 'Report', level: 2 }).waitFor();
    await page.screenshot({ path: path.join(ctx.shots, 'frame-report.png') });
    await axe('the report page');
    await mainTab('Code').click();
    await page.getByRole('heading', { name: 'Documents', level: 2 }).waitFor();
    h.equal(await back.count(), 0, 'the bar is gone in the coding workspace');
    h.equal(await ribbonTab('Home').getAttribute('aria-selected'), 'true', 'Home is the tab that is chosen again');
  });

  await S('a command that cannot run now stays visible and says why', async () => {
    await closeDialogs();
    await mainTab('Data').click();
    await ribbonTab('Home').click();
    const inVivo = ribbonButton('Code In Vivo');
    h.equal(await inVivo.getAttribute('aria-disabled'), 'true', 'Code In Vivo is marked unavailable away from the coding workspace');
    // Playwright will not click something marked unavailable, but a person can press it: use the keyboard.
    await inVivo.focus();
    await page.keyboard.press('Enter');
    const said = await waitStatus(/not available right now/);
    h.check(/coding workspace/.test(said), `the reason is spoken: ${said}`);
    await mainTab('Coding').click();
    h.check((await inVivo.getAttribute('aria-disabled')) === null, 'Code In Vivo is available again in the coding workspace with a document open');
    // Auto Code needs no open document, so it is available wherever the project is open.
    h.check((await ribbonButton('Auto Code').getAttribute('aria-disabled')) === null, 'Auto Code is available');
  });

  await S('Ctrl+K finds a command by what it does, and runs it', async () => {
    await closeDialogs();
    await page.keyboard.press('Control+k');
    const dialog = page.getByRole('dialog', { name: 'Search or run a command' });
    await dialog.waitFor();
    await page.screenshot({ path: path.join(ctx.shots, 'frame-palette.png') });
    await axe('the command box');
    await page.keyboard.type('export');
    const option = dialog.getByRole('option', { name: /Export the codebook or the coded passages/ });
    await option.waitFor();
    // Several commands mention exporting, so the one wanted is chosen by name.
    await option.click();
    await exportDialog().waitFor();
    h.check(true, 'choosing the command opened the Export dialog');
    await page.keyboard.press('Escape');
    await exportDialog().waitFor({ state: 'detached' });
  });

  await S('Ctrl+K finds words in the text and opens the document there', async () => {
    await closeDialogs();
    const { sourceId, word } = await firstWord();
    await mainTab('Home').click();
    await page.keyboard.press('Control+k');
    const dialog = page.getByRole('dialog', { name: 'Search or run a command' });
    await dialog.waitFor();
    await page.keyboard.type(word);
    await dialog.getByText(/in the text/).waitFor({ timeout: 8000 });
    const hit = dialog.locator('.palette-hit').first();
    await hit.waitFor();
    await page.keyboard.press('Enter');
    await page.getByRole('heading', { level: 2, name: (await rpc('source.list')).find((s) => s.id === sourceId).name }).waitFor();
    h.equal(await page.getByRole('button', { name: 'Back to the coding workspace' }).count(), 0, 'the search result opened the coding workspace');
    await page.waitForFunction(() => /Selected:/.test(document.querySelector('[data-testid=selection-line]')?.textContent ?? ''), word, { timeout: 5000 });
    h.check(true, 'the passage is selected in the text');
  });

  await S('Undo takes back the newest change and says what it was', async () => {
    await closeDialogs();
    const source = (await rpc('source.list'))[0];
    const code = (await rpc('code.list')).find((c) => c.status === 'active');
    const before = (await rpc('coding.list', { source_id: source.id })).length;
    // A project that codes whole units needs the unit the words are in.
    const unitMode = (await rpc('project.info')).coding_mode === 'unit';
    const unit = unitMode ? (await rpc('unit.list', { source_id: source.id })).find((u) => !u.excluded) : null;
    const span = unit ? { start: unit.start, end: Math.min(unit.start + 4, unit.end), unit_id: unit.id } : { start: 0, end: 4 };
    await rpc('coding.apply', { source_id: source.id, code_id: code.id, ...span });
    h.equal((await rpc('coding.list', { source_id: source.id })).length, before + 1, 'a coding was made behind the screen');
    await showTab('Code');
    await ribbonButton('Undo (Ctrl+Z)').click();
    const said = await waitStatus(/Undid:/);
    h.check(/Undid:/.test(said), `the undo is spoken: ${said}`);
    h.equal((await rpc('coding.list', { source_id: source.id })).length, before, 'the coding is gone again');
    await showTab('Home');
  });

  await S('the dashboard is the book\'s: the study, four counts that lead to their lists, the activity chart, and the data by type', async () => {
    await closeDialogs();
    await mainTab('Home').click();
    const sources = await rpc('source.list');
    const info = await rpc('project.info');
    // The heading and the name above the counts.
    await page.getByRole('heading', { level: 2, name: `Project Dashboard – ${info.name}` }).waitFor();
    h.check(await page.getByRole('button', { name: new RegExp(`^About ${info.name}`) }).isVisible(), 'an info button sits beside the name');
    h.check(/^Last saved: (Today|Yesterday|[A-Z][a-z]{2} \d{1,2}, \d{4}), \d{1,2}:\d{2} [AP]M$/.test((await page.getByTestId('dashboard-saved').innerText()).trim()), `the last save is told in words: ${await page.getByTestId('dashboard-saved').innerText()}`);
    // Four counts, each naming what it counts.
    const codes = await rpc('code.list');
    const cases = await rpc('case.list');
    const codings = sources.reduce((n, s) => n + s.codings, 0);
    const card = (name) => page.getByRole('list', { name: 'Counts in this project' }).getByRole('button', { name: new RegExp(`^[\\d,]+\\s*${name}$`) });
    const number = async (name) => Number((await card(name).innerText()).replace(/[^\d]/g, ''));
    h.equal(await number('Documents'), sources.length, 'Documents counts every source');
    h.equal(await page.getByRole('list', { name: 'Counts in this project' }).getByRole('button').count(), 4, 'four cards');
    await page.waitForFunction((n) => /\d/.test([...document.querySelectorAll('.bk-dash-card')].find((e) => /Cases/.test(e.textContent))?.textContent ?? ''), cases.length);
    h.equal(await number('Cases'), cases.length, 'Cases counts the cases');
    h.equal(await number('Codes'), codes.filter((c) => c.status === 'active').length, 'Codes counts the active codes');
    h.equal(await number('Coded Segments'), codings, 'Coded Segments counts every coding');
    // Each says what it counts and where it leads.
    h.check(/Press to see them in the data table/.test(await page.locator('#dash-what-Documents').textContent()), 'a card has a hidden sentence saying what it counts and where it goes');
    // Project Activity: the same numbers as a table, and a sentence.
    const rows = await page.getByTestId('activity-table').locator('tbody tr').evaluateAll((trs) => trs.map((r) => ({ day: r.querySelector('th').textContent, n: Number(r.querySelector('td').textContent) })));
    h.equal(rows.length, 14, 'fourteen days in the table');
    h.equal(rows.at(-1).day, 'Today', 'ending with today');
    const events = await rpc('events.list', { limit: 5000 });
    const from = new Date();
    from.setHours(0, 0, 0, 0);
    from.setDate(from.getDate() - 13);
    const expected = events.filter((e) => new Date(e.ts) >= from).length;
    h.equal(rows.reduce((n, r) => n + r.n, 0), expected, 'the days add up to the changes in the project history over those days');
    h.check(/changes? in the last 14 days|No changes in the last 14 days/.test(await page.getByTestId('activity-summary').innerText()), 'a sentence says the window and the measure');
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=activity-chart] circle').length === 14);
    // Data by Type: shares add up to 100, one row for each kind.
    const legend = await page.getByRole('list', { name: 'Documents by kind' }).getByRole('listitem').evaluateAll((lis) => lis.map((l) => ({ name: l.querySelector('.bk-legend-name').textContent, pct: Number(/(\d+)%/.exec(l.querySelector('.bk-legend-value').textContent)[1]) })));
    h.equal(legend.reduce((n, l) => n + l.pct, 0), 100, 'the percentages add up to 100');
    h.equal(legend.length, new Set(sources.map((s) => s.source_type)).size, 'one row for each kind of document');
    await page.screenshot({ path: path.join(ctx.shots, 'frame-dashboard.png'), fullPage: true });
    await axe('the project dashboard');
    // The study's question is shown, and the info button says where the file is.
    h.check((await page.getByTestId('dashboard-about').innerText()).trim().length > 0, 'the study description line is there');
    await page.getByRole('button', { name: new RegExp(`^About ${info.name}`) }).click();
    h.check((await page.getByRole('dialog', { name: 'About this project' }).innerText()).includes(info.path.split(/[\\/]/).pop()), 'the info says which file this is');
    await page.keyboard.press('Escape');
    // Project Details is a menu.
    await page.getByRole('button', { name: 'Project Details' }).click();
    const items = (await page.getByRole('menu', { name: 'Project Details' }).getByRole('menuitem').allTextContents()).map((t) => t.trim());
    h.equal(items, ['Study details (memo)', 'All activity', 'Save a backup copy now', 'Export the codebook or passages…'], 'Project Details offers what belongs to the project');
    await page.keyboard.press('Escape');
    // The Documents card leads to the data table; Codes leads back to the coding workspace.
    await card('Documents').click();
    await page.getByRole('heading', { name: 'Data', level: 2 }).waitFor();
    h.check(true, 'the Documents card opens the data table');
    await mainTab('Home').click();
    await page.getByRole('heading', { level: 2, name: /^Project Dashboard/ }).waitFor();
    await card('Codes').click();
    await page.getByRole('heading', { name: 'Codes', level: 2 }).waitFor();
    h.check(true, 'the Codes card goes to the codes');
    await page.getByRole('heading', { level: 3, name: 'Next step' }).count();
    await mainTab('Code').click();
  });

  await S('the dashboard still offers the next step, and the activity view is one click away', async () => {
    await mainTab('Home').click();
    await page.getByRole('heading', { name: 'Next step', level: 3 }).waitFor();
    await page.getByRole('navigation', { name: 'Home views' }).getByRole('button', { name: 'Activity' }).click();
    await page.getByRole('heading', { name: 'History', level: 2 }).waitFor();
    await page.getByRole('navigation', { name: 'Home views' }).getByRole('button', { name: 'Dashboard' }).click();
    await page.getByRole('heading', { level: 2, name: /^Project Dashboard/ }).waitFor();
    await mainTab('Code').click();
  });

  await S('the report page opens the export dialog on the kind of file chosen', async () => {
    await closeDialogs();
    await mainTab('Report').click();
    await page.getByRole('button', { name: 'Export coded passages…' }).click();
    const d = exportDialog();
    await d.waitFor();
    h.check(await d.getByRole('radio', { name: /The coded passages/ }).isChecked(), 'the coded passages are chosen');
    await page.keyboard.press('Escape');
    await d.waitFor({ state: 'detached' });
    await page.getByRole('button', { name: 'Export the codebook…' }).click();
    await d.waitFor();
    h.check(await d.getByRole('radio', { name: /The codebook/ }).isChecked(), 'the codebook is chosen');
    await page.keyboard.press('Escape');
    await mainTab('Code').click();
    await sleep(50);
  });

  // ------------------------------------------------------------------------------ Retrieved Segments
  const pane = () => page.getByRole('region', { name: /^Retrieved Segments/ });
  const table = () => pane().getByRole('grid', { name: 'Retrieved Segments' });
  const dataRows = () => table().locator('tbody [role=row], [role=rowgroup]:not(:first-child) [role=row]');
  const chosenCode = async () => (await rpc('code.list')).find((c) => c.status === 'active' && c.parent_id === null && c.usage > 0);
  const summaryText = () => pane().getByTestId('results-summary').innerText();
  async function waitSummary(re, ms = 8000) {
    const t0 = Date.now();
    let text = '';
    while (Date.now() - t0 < ms) {
      text = await summaryText();
      if (re.test(text)) return text;
      await sleep(80);
    }
    return text;
  }
  const countRows = async () => (await table().getByRole('row').count()) - 1; // the header row is one

  await S('Retrieved Segments lists every coding until codes are chosen', async () => {
    await closeDialogs();
    await mainTab('Coding').click();
    const total = (await rpc('segments.list', { limit: 1 })).total;
    const said = await waitSummary(new RegExp(`^${Math.min(total, 500).toLocaleString('en-US')} segments? with any code, in every document`));
    h.check(/with any code, in every document/.test(said), `the table says what it shows: ${said}`);
    const loaded = Math.min(total, 500);
    h.equal(await countRows(), Math.min(loaded, 100), 'the first hundred rows are drawn, one for each coding');
    if (loaded > 100) {
      h.check(/Showing 100 of /.test(await pane().getByTestId('more-rows').innerText()), 'and the rest are told, with how many are shown');
      await pane().getByRole('button', { name: /^Show \d+ more$/ }).click();
      h.equal(await countRows(), Math.min(loaded, 200), 'Show more adds the next hundred');
    }
    h.equal((await pane().getByTestId('results-count').innerText()).replace(/[(),\s]/g, ''), String(Math.min(total, 500)), 'the heading carries the count');
    const cols = (await table().getByRole('columnheader').allTextContents()).map((t) => t.trim()).filter(Boolean);
    h.equal(cols, ['#', 'Source', 'Code', 'Case', 'Excerpt (preview)', 'Notes'], 'the columns are the book\'s');
    await showTab('View');
    h.equal(await ribbonButton('Scope: No filter').getAttribute('aria-disabled'), null, 'the scope chip is there and says there is no filter');
    h.equal(await ribbonButton('Reset').getAttribute('aria-disabled'), 'true', 'Reset has nothing to do yet');
    await page.screenshot({ path: path.join(ctx.shots, 'frame-retrieved.png') });
    await axe('Retrieved Segments with every coding');
  });

  await S('the Codes filter narrows the table to the codes chosen', async () => {
    const code = await chosenCode();
    const total = (await rpc('segments.list', { code_ids: [code.id], limit: 1 })).total;
    await pane().getByRole('button', { name: /^Codes: / }).click();
    await page.getByRole('menuitemcheckbox', { name: code.name, exact: true }).click();
    await page.keyboard.press('Escape');
    const said = await waitSummary(new RegExp(`^${total.toLocaleString('en-US')} segments? coded ${code.name}, in every document`));
    h.check(new RegExp(`^${total.toLocaleString('en-US')} segments? coded ${code.name}, in every document`).test(said), `the table says how many: ${said}`);
    h.equal(await countRows(), Math.min(total, 100), 'a row for each coding of that code (the first hundred)');
    h.check(await pane().getByRole('button', { name: `Codes: ${code.name}` }).isVisible(), 'the filter button names the code');
    await showTab('View');
    await ribbonButton('Scope: 1 code').waitFor();
    await page.screenshot({ path: path.join(ctx.shots, 'frame-passages.png') });
    await axe('Retrieved Segments with a code chosen');
  });

  await S('choosing a row opens its document with those words selected and fills Segment Details', async () => {
    const code = await chosenCode();
    const first = (await rpc('segments.list', { code_ids: [code.id], limit: 1 })).segments[0];
    const source = (await rpc('source.list')).find((s) => s.id === first.source_id);
    // The first row of the table, whichever document it is from (the table is in its own order).
    const firstRow = table().getByRole('row').nth(1);
    const rowSource = ((await firstRow.locator('.bk-col-source').innerText()) ?? '').trim();
    void source;
    await firstRow.locator('.bk-col-source').click();
    await page.getByRole('heading', { level: 2, name: rowSource }).waitFor();
    const opened = (await rpc('source.list')).find((s) => s.name === rowSource);
    if (opened.units > 1000 || opened.characters > 400000) {
      const said = await waitStatus(/Opened/);
      h.check(/Opened/.test(said), `a very large source opens as a grid and says so: ${said}`);
      // Back to a plain document for the steps that follow.
      await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).click();
      await surface().waitFor();
    } else {
      await page.waitForFunction(() => /Selected:/.test(document.querySelector('[data-testid=selection-line]')?.textContent ?? ''), null, { timeout: 5000 });
      h.check(true, 'the passage is selected in the text');
      await page.getByTestId('coding-detail').waitFor();
      h.check(/Selected Segment/.test(await page.getByTestId('coding-detail').textContent()), 'Segment Details describes the coding');
    }
  });

  await S('the table can look only in the open document, and the chip says so', async () => {
    const code = await chosenCode();
    const open = (await rpc('segments.list', { code_ids: [code.id], limit: 1 })).segments[0].source_id;
    const total = (await rpc('segments.list', { code_ids: [code.id], source_ids: [open], limit: 1 })).total;
    await pane().getByRole('button', { name: /^Documents: / }).click();
    await page.getByRole('menuitemradio', { name: 'The open document' }).click();
    const said = await waitSummary(new RegExp(`^${total.toLocaleString('en-US')} segments? coded`));
    h.check(/in /.test(said) && !/every document/.test(said), `only the open document is listed: ${said}`);
    await showTab('View');
    await ribbonButton('Scope: 1 code, open source').waitFor();
    await pane().getByRole('button', { name: /^Documents: / }).click();
    await page.getByRole('menuitemradio', { name: 'All documents' }).click();
    await waitSummary(/every document/);
  });

  await S('searching and sorting the table, and a note when the chosen segment is out of view', async () => {
    const rows = await countRows();
    h.check(rows > 0, 'the table has rows to search');
    const word = (await table().getByRole('row').nth(1).locator('.bk-col-excerpt').innerText()).replace(/[“”]/g, '').split(/\s+/).find((w) => w.length >= 4) ?? 'the';
    await pane().getByRole('searchbox', { name: 'Search segments' }).fill(word);
    await page.waitForFunction((n) => document.querySelectorAll('[aria-label="Retrieved Segments"] [role=row]').length - 1 <= n, rows, { timeout: 5000 });
    h.check((await countRows()) <= rows && (await countRows()) >= 1, `the search keeps the rows with "${word}"`);
    await pane().getByRole('searchbox', { name: 'Search segments' }).fill('');
    await page.waitForFunction((n) => document.querySelectorAll('[aria-label="Retrieved Segments"] [role=row]').length - 1 === n, rows, { timeout: 5000 });
    const header = table().getByRole('columnheader', { name: 'Code' });
    await header.click();
    h.equal(await header.getAttribute('aria-sort'), 'ascending', 'pressing a column heading sorts by it');
    const names = (await table().locator('.bk-col-code .bk-chip').allTextContents()).map((t) => t.trim());
    h.equal(names, [...names].sort((a, b) => a.localeCompare(b)), 'the rows are in order of code name');
    await header.click();
    h.equal(await header.getAttribute('aria-sort'), 'descending', 'pressing it again reverses the order');
    // A filter that hides the chosen segment says so and leaves the reader where it is.
    const others = (await rpc('code.list')).filter((c) => c.status === 'active' && c.usage === 0);
    if (others.length > 0) {
      // Choose a coding in the open document, then filter the table to a code that has no codings at all.
      const shownCode = await chosenCode();
      const openName = ((await page.locator('#h-source').textContent()) ?? '').trim();
      const open = (await rpc('source.list')).find((s) => s.name === openName);
      const mine = (await rpc('coding.list', { source_id: open.id }))[0];
      await page.locator(`.stripe[data-coding="${mine.id}"]`).click();
      await page.getByTestId('coding-detail').waitFor();
      const escape = (t) => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      await pane().getByRole('button', { name: /^Codes: / }).click();
      // The code the table showed is taken off the filter, and the empty one put on.
      await page.getByRole('menuitemcheckbox', { name: new RegExp(`${escape(shownCode.name)}$`) }).click();
      await page.getByRole('menuitemcheckbox', { name: new RegExp(`${escape(others[0].name)}$`) }).click();
      await page.keyboard.press('Escape');
      await pane().getByTestId('results-outside').waitFor();
      h.check(true, 'the table says the chosen segment is outside the rows shown');
    }
  });

  await S('Reset clears every filter, in the ribbon and in the table', async () => {
    await showTab('View');
    await ribbonButton('Reset').click();
    await waitSummary(/with any code, in every document/);
    await ribbonButton('Scope: No filter').waitFor();
    h.equal(await ribbonButton('Reset').getAttribute('aria-disabled'), 'true', 'Reset has nothing left to do');
    await showTab('Home');
  });

  await S('Show passages in the codebook toggles the selected code, and the row says so', async () => {
    const code = await chosenCode();
    await page.getByRole('row', { name: new RegExp(`^${code.name},`) }).click();
    h.check(await api.codeToggle('Show passages of this code'), 'the menu item is checked once pressed');
    await page.getByRole('row', { name: new RegExp(`^${code.name},.*its passages are shown`) }).waitFor();
    await waitSummary(/segments? coded/);
    h.check(!(await api.codeToggle('Show passages of this code')), 'pressing it again stops showing them');
    await waitSummary(/with any code/);
  });
}
