// Explore: the Query Builder, the Code Matrix, Code Relations, Code Summary, and Visualizations, on the Urban Mobility sample, whose
// ten codings are known: Alex has Daily Travel, Reliability (rated), Cost, and Sustainability; Priya has Reliability and Desired
// Improvements; Jordan has Mode Choice and Safety; the focus group has Accessibility and Cost. The cases are Urban Residents (Alex and
// Priya) and Commuters (Jordan and the focus group). One coder made them all, and no two codings overlap.

export async function runExplore(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe } = api;

  const goTool = async (button) => {
    await page.getByRole('tab', { name: 'Explore', exact: true }).click();
    await page.getByRole('button', { name: button, exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Explore' }).waitFor();
  };
  const tab = (name) => page.getByRole('tablist', { name: 'Explore tools' }).getByRole('tab', { name });
  /** A table as rows of cell texts. */
  const cells = (locator) => locator.locator('tr').evaluateAll((rows) => rows.map((r) => [...r.querySelectorAll('th,td')].map((c) => c.textContent.replace(/\s+/g, ' ').trim())));

  /** Opens a picker, clears it, and ticks the named things in that order. */
  async function choose(buttonName, names) {
    await page.getByRole('button', { name: new RegExp(`^${buttonName}:`) }).click();
    const dialog = page.getByRole('dialog', { name: buttonName });
    await dialog.waitFor();
    await dialog.getByRole('button', { name: 'None' }).click();
    for (const n of names) await dialog.locator('label').filter({ has: page.getByRole('checkbox', { name: n, exact: true }) }).click();
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
  }
  const byName = async (name) => (await rpc('code.list')).find((c) => c.name === name);

  await S('Explore opens from the ribbon with its tools', async () => {
    await goTool('Query');
    const tabs = (await page.getByRole('tablist', { name: 'Explore tools' }).getByRole('tab').allTextContents()).map((t) => t.trim());
    h.equal(tabs, ['Query Builder', 'Code Matrix', 'Crosstabs', 'Code Relations', 'Code Summary', 'Visualizations', 'Concept Map', 'Document Portrait'], 'the book\'s tabs that are built');
    h.check(!/not built yet/.test(await page.getByRole('main').innerText()), 'every tool of the page is built, so nothing on it says it is not');
    await page.getByTestId('query-tool').waitFor();
    h.equal(await page.getByTestId('run-query').isDisabled(), true, 'a query with no conditions cannot be run');
    h.check(/no conditions yet/i.test(await page.getByTestId('query-readback').innerText()), 'and says why, in its readback');
    await axe('Explore, the Query Builder before anything is added');
  });

  await S('a query is built, read back in words, run, and its results open at their source and stay when the page is left', async () => {
    const reliability = await byName('Reliability');
    const cost = await byName('Cost');
    await page.getByTestId('add-condition').click();
    h.equal(await page.getByTestId('run-query').isDisabled(), true, 'a condition with no code chosen cannot be run');
    h.check(/no code chosen/.test(await page.getByTestId('query-readback').innerText()), 'and the readback says what is missing');
    await choose('Condition 1, codes', ['Reliability', 'Cost']);
    const read = await page.getByTestId('query-readback').innerText();
    h.check(/the code is Reliability or Cost/.test(read), `the readback is in words: ${read.replace(/\s+/g, ' ').slice(0, 220)}`);
    h.check(/Look through 10 codings in 7 documents/.test(read), 'and says what is looked through');
    await axe('Explore, the Query Builder with a condition');
    await page.getByTestId('run-query').click();
    const count = page.getByTestId('query-count');
    await count.waitFor();
    h.check(/^4 excerpts from 4 matching codings, looking through 10 codings in 4 documents/.test((await count.innerText()).trim()), `the count says what it counts: ${await count.innerText()}`);
    const items = page.getByRole('list', { name: 'Query results' }).locator('li');
    h.equal(await items.count(), 4, 'four excerpts are listed');
    const text = await page.getByTestId('query-results').innerText();
    h.check(/reliability can be a problem/.test(text) && /signal failure/.test(text) && /monthly pass/.test(text) && /For me it is the cost/.test(text), 'they are the four passages coded Reliability or Cost');
    const engine = await rpc('explore.query', { rule: { type: 'group', match: 'any', items: [{ type: 'code', code_ids: [reliability.id, cost.id] }] }, result_unit: 'excerpts' });
    h.equal(engine.total, 4, 'the engine agrees');
    await page.screenshot({ path: `${ctx.shots}/explore-query.png`, fullPage: true });
    await axe('Explore, the Query Builder with results');

    // A changed query marks the old results as an earlier version, and putting it back takes the mark away.
    await page.getByTestId('add-condition').click();
    await page.getByTestId('stale').waitFor();
    h.check(true, 'adding a condition marks the results as from an earlier version');
    await page.getByRole('button', { name: 'Remove condition 2' }).click();
    await page.getByTestId('stale').waitFor({ state: 'hidden' });
    h.check(true, 'taking it away again makes them current');

    // A result opens at its source.
    await items.first().getByRole('button', { name: /^Open .* at these words$/ }).click();
    await page.getByRole('heading', { level: 2, name: 'Documents' }).waitFor();
    h.check(true, 'Open at source goes to the document');
    await goTool('Query');
    await page.getByTestId('query-count').waitFor();
    h.equal(await page.getByRole('list', { name: 'Query results' }).locator('li').count(), 4, 'the query and its results are still there when the page is visited again');
  });

  await S('a query can ask for words, a case variable, and the same words also coded with another code, and can be turned round', async () => {
    // Text contains "bus": five stretches mention a bus or buses.
    await page.getByRole('button', { name: 'Start again' }).click();
    await page.getByTestId('add-condition').click();
    await page.getByLabel('Condition 1: ingredient').selectOption('text');
    await page.getByRole('textbox', { name: 'Condition 1: the words' }).fill('bus');
    h.check(/the words contain “bus”/.test(await page.getByTestId('query-readback').innerText()), 'the readback follows what is typed');
    await page.getByTestId('run-query').click();
    await page.getByTestId('query-count').filter({ hasText: /^5 excerpts/ }).waitFor();
    h.check(true, 'five stretches contain the word');
    // Turned round: the words do not contain "bus". The other five.
    await page.getByLabel('Condition 1: operator').selectOption('not');
    h.check(/do not contain/.test(await page.getByTestId('query-readback').innerText()), 'turning it round is in the readback');
    await page.getByTestId('stale').waitFor();
    await page.getByTestId('run-query').click();
    await page.getByTestId('stale').waitFor({ state: 'hidden' });
    await page.getByTestId('query-count').filter({ hasText: /^5 excerpts/ }).waitFor();
    h.check(!/\bbus/i.test(await page.getByRole('list', { name: 'Query results' }).innerText()), 'none of the five mentions a bus');
    // Also coded: Reliability where the same words are also coded Cost: none overlap in this sample.
    await page.getByRole('button', { name: 'Start again' }).click();
    await page.getByTestId('add-condition').click();
    await page.getByLabel('Condition 1: ingredient').selectOption('also');
    await choose('Condition 1, codes', ['Cost']);
    await page.getByTestId('match-top').selectOption('all');
    await page.getByRole('button', { name: 'Add Condition', exact: true }).first().click();
    await choose('Condition 2, codes', ['Reliability']);
    h.check(/the same words are also coded Cost/.test(await page.getByTestId('query-readback').innerText()) && /all of these are true/.test(await page.getByTestId('query-readback').innerText()), 'also coded, with all of the conditions, reads back in words');
    await page.getByTestId('run-query').click();
    await page.getByTestId('query-count').filter({ hasText: /^0 excerpts/ }).waitFor();
    h.check(/Nothing in the project matches/.test(await page.getByTestId('query-results').innerText()), 'no match is said as no match, after looking at every coding');
    await axe('Explore, a query with no matches');
  });

  await S('Compare Cases opens the code matrix by cases, with every number opening its evidence and the totals counted once', async () => {
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
    await page.getByRole('button', { name: 'Compare Cases' }).click();
    await page.getByTestId('matrix-tool').waitFor();
    h.equal(await page.getByLabel('Rows', { exact: true }).inputValue(), 'cases', 'Compare Cases sets the rows to cases');
    await choose('Codes for the columns', ['Reliability', 'Cost']);
    const table = page.getByTestId('matrix-table');
    await table.waitFor();
    h.check(/Assignments of each code, by case/.test(await table.locator('caption').innerText()), 'the table says what it shows');
    const rows = await cells(table);
    h.equal(rows.map((r) => r.join('|')), ['Case|Reliability|Cost|Total (distinct)', 'Commuters|0|1|1', 'Urban Residents|2|1|3', 'All cases (distinct)|2|2|4'], 'assignments by case, hand-checked');
    h.check(/Looked at 10 codings in 4 documents/.test(await page.getByTestId('matrix-universe').innerText()), 'the universe is told');
    // Another metric: cases with evidence. Urban Residents is one case, so its cells are 1.
    await page.getByLabel('Metric').selectOption('cases');
    await table.locator('caption').filter({ hasText: /Cases with evidence/ }).waitFor();
    const byCases = await cells(table);
    h.equal(byCases.map((r) => r.join('|')), ['Case|Reliability|Cost|Total (distinct)', 'Commuters|0|1|1', 'Urban Residents|1|1|1', 'All cases (distinct)|1|2|2'], 'cases with evidence: the totals count each case once');
    h.check(/cells add up to 3 and the distinct total is 2/.test(await page.getByTestId('matrix-notes').innerText()), 'and the page says why the totals are smaller than the cells added up');
    // Excerpts: distinct stretches.
    await page.getByLabel('Metric').selectOption('assignments');
    await page.getByRole('button', { name: /^Urban Residents, Reliability: 2 assignments/ }).click();
    const evidence = page.getByTestId('matrix-evidence');
    await evidence.waitFor();
    await evidence.getByTestId('evidence-count').filter({ hasText: /^2 codings/ }).waitFor();
    const shown = await evidence.innerText();
    h.check(/reliability can be a problem/.test(shown) && /signal failure/.test(shown), 'a cell opens the passages behind it');
    await axe('Explore, the code matrix with evidence');
    await page.screenshot({ path: `${ctx.shots}/explore-matrix.png`, fullPage: true });
    await evidence.getByRole('button', { name: 'Close the evidence' }).click();
    await evidence.waitFor({ state: 'hidden' });
  });

  await S('a parent code counts its sub-codes unless told not to, documents can be the rows, and a cell with nothing behind it is a plain zero', async () => {
    await choose('Codes for the columns', ['Barriers & Challenges']);
    const table = page.getByTestId('matrix-table');
    await page.getByRole('button', { name: /^Barriers & Challenges, all rows: 6 assignments/ }).waitFor();
    h.equal((await cells(table)).at(-1), ['All cases (distinct)', '6', '6'], 'Barriers & Challenges counts the codings of Cost, Reliability, Accessibility and Safety under it');
    await page.locator('label').filter({ has: page.getByRole('checkbox', { name: 'A code also counts its sub-codes' }) }).click();
    await page.waitForFunction(() => [...document.querySelectorAll('[data-testid=matrix-table] tfoot td')].every((td) => td.textContent.trim() === '0'));
    h.equal((await cells(table)).at(-1), ['All cases (distinct)', '0', '0'], 'with sub-codes left out it has no codings of its own');
    h.check(/only the codings of its own code/.test(await page.getByTestId('matrix-notes').innerText()), 'and the page says which rule is in force');
    await page.locator('label').filter({ has: page.getByRole('checkbox', { name: 'A code also counts its sub-codes' }) }).click();
    await page.getByLabel('Rows', { exact: true }).selectOption('documents');
    await page.getByRole('rowheader', { name: 'SR-001 – Survey comments' }).waitFor();
    const docRows = await cells(table);
    h.equal(docRows.length, 1 + 7 + 1, 'a row for each of the seven documents, between the header and the totals');
    h.equal(docRows.find((r) => r[0].startsWith('SR-001')), ['SR-001 – Survey comments', '0', '0'], 'a document with no coding has zeros, which are not buttons');
    h.equal(await page.getByRole('rowheader', { name: 'SR-001 – Survey comments' }).locator('xpath=..').getByRole('button').count(), 0, 'there is nothing to open behind a zero');
    await page.getByLabel('Rows', { exact: true }).selectOption('cases');
  });

  await S('Code Relations counts pairs that overlap, or documents that have both codes, and opens the evidence', async () => {
    await tab('Code Relations').click();
    const table = page.getByTestId('relations-table');
    await table.waitFor();
    const rowOf = async (name) => (await cells(table)).find((r) => r[0] === name);
    const grid = await cells(table);
    const head = grid[0].slice(1);
    const at = (row, col) => row[1 + head.indexOf(col)];
    h.equal(at(await rowOf('Reliability'), 'Reliability'), '2', 'on the diagonal a code is counted on its own');
    h.equal(at(await rowOf('Reliability'), 'Cost'), '0', 'no codings in the sample overlap, so no pair is counted');
    h.check(/pairs of codings, one of each code, whose words or picture areas overlap and that were made by the same coder/.test(await page.getByTestId('relations-notes').innerText()), 'the counting rule is told');
    await page.locator('label').filter({ has: page.getByRole('radio', { name: 'In the same document' }) }).click();
    await page.waitForFunction(() => /documents that have a coding of both/.test(document.querySelector('[data-testid=relations-notes]')?.textContent ?? ''));
    const docs = await cells(table);
    const docHead = docs[0].slice(1);
    const docRow = docs.find((r) => r[0] === 'Reliability');
    h.equal(docRow[1 + docHead.indexOf('Cost')], '1', 'Reliability and Cost are both in Alex\'s interview');
    await page.getByRole('button', { name: /^Reliability with Cost: 1 document/ }).click();
    const evidence = page.getByTestId('relations-evidence');
    await evidence.getByTestId('evidence-count').filter({ hasText: /^2 codings/ }).waitFor();
    h.check(/reliability can be a problem/.test(await evidence.innerText()) && /monthly pass/.test(await evidence.innerText()), 'the evidence is the Reliability and the Cost coding in that document');
    await axe('Explore, Code Relations');
  });

  await S('Code Summary lists every code with sub-codes counted in, and a code shows where it was used and by whom', async () => {
    await tab('Code Summary').click();
    const table = page.getByTestId('summary-table');
    await table.waitFor();
    const rows = await cells(table);
    h.equal(rows.length - 1, 15, 'a row for each of the fifteen codes');
    h.equal(rows.find((r) => r[0] === 'Reliability'), ['Reliability', '2', '2', '2', '1', '1'], 'Reliability: two codings, two excerpts, two documents, one case, one coder');
    h.equal(rows.find((r) => r[0] === 'Barriers & Challenges'), ['Barriers & Challenges', '6', '6', '4', '2', '1'], 'a parent counts its sub-codes\' codings, each once, and the documents and cases they are in');
    await page.getByRole('button', { name: /^Reliability[:,]/ }).click();
    const profile = page.getByTestId('profile');
    await profile.waitFor();
    h.check(/2 codings \(2 distinct excerpts\) in 2 documents and 1 case, by 1 coder\./.test(await page.getByTestId('profile-counts').innerText()), `the profile counts: ${await page.getByTestId('profile-counts').innerText()}`);
    const text = await profile.innerText();
    h.check(/INT-001/.test(text) && /INT-002/.test(text) && /Urban Residents/.test(text), 'it says which documents and which case');
    h.check(/Show every passage in Retrieved Segments/.test(text) && /Open in the Codebook Editor/.test(text), 'and where to go next');
    await axe('Explore, Code Summary');
  });

  await S('Visualizations draws the same numbers as bars with the names and numbers written beside them, and says what the chart shows', async () => {
    await tab('Visualizations').click();
    const chart = page.getByTestId('chart-by-code');
    await chart.waitFor();
    const bars = await chart.locator('li').allInnerTexts();
    h.equal(bars.map((t) => t.replace(/\s+/g, ' ').trim()), ['Barriers & Challenges 6 assignments'], 'one bar for each column chosen, with the code and its number in words');
    h.check(/The longest bar is Barriers & Challenges, with 6 assignments/.test(await chart.innerText()), 'and a sentence says what the chart shows');
    await choose('Codes for the columns', ['Reliability', 'Cost', 'Accessibility']);
    await page.getByTestId('chart-by-code').locator('li').nth(2).waitFor();
    const three = (await page.getByTestId('chart-by-code').locator('li').allInnerTexts()).map((t) => t.replace(/\s+/g, ' ').trim());
    h.equal(three, ['Reliability 2 assignments', 'Cost 2 assignments', 'Accessibility 1 assignment'], 'the bars follow the setup shared with Code Matrix');
    const widths = await page.getByTestId('chart-by-code').locator('.ex-bar').evaluateAll((els) => els.map((e) => Math.round(parseFloat(e.style.width))));
    h.check(widths[0] === widths[1] && widths[2] < widths[0] && widths[2] > 0, `a bar is as long as its number: ${widths.join(', ')}`);
    h.check((await page.locator('.ex-chart-grid figure').count()) >= 2, 'a chart for each case, drawn on one scale so they can be compared');
    await page.screenshot({ path: `${ctx.shots}/explore-charts.png`, fullPage: true });
    await axe('Explore, Visualizations');
    await page.getByRole('button', { name: 'See the numbers as a table' }).click();
    await page.getByTestId('matrix-table').waitFor();
    h.check(true, 'the chart leads back to the table of the same numbers');
  });

  await S('Document Portrait draws each document with coded words as a strip of marks at their places, and the codeline gives each code a row of its own', async () => {
    await tab('Document Portrait').click();
    await page.getByTestId('portrait-tool').waitFor();
    await page.getByTestId('pt-portrait').first().waitFor();
    const words = (await rpc('segments.list', { limit: 5000 })).segments.filter((s) => s.kind === 'text');
    h.equal(await page.getByTestId('pt-mark').count(), words.length, 'one mark for each coded passage of words');
    h.check((await page.getByTestId('pt-portrait').count()) >= 1, 'a portrait for each document that has coded words');
    const label = await page.getByTestId('pt-mark').first().getAttribute('aria-label');
    h.check(/: “.+”, from \d+ to \d+ percent of /.test(label), `a mark says its code, its words, and where it is: ${label}`);
    h.check(/\d+ percent of the document is coded/.test(await page.getByTestId('pt-portrait').first().innerText()), 'and each portrait says how much of the document is coded');
    h.check((await page.getByTestId('pt-table').count()) >= 1, 'the same numbers are in a table');
    await axe('Explore, Document Portrait');

    // a mark opens the passage in its document
    await page.getByTestId('pt-mark').first().click();
    await page.getByRole('heading', { level: 2, name: 'Documents' }).waitFor();
    await page.getByTestId('coding-detail').waitFor();
    h.check(true, 'pressing a mark opens the passage in its document, with its details');

    // the codeline of one document: one row for each code used in it
    await goTool('Query');
    await tab('Document Portrait').click();
    await page.getByTestId('pt-view').selectOption('codeline');
    await page.getByTestId('pt-codeline').waitFor();
    const doc = Number(await page.getByTestId('pt-doc').inputValue());
    const inDoc = new Set(words.filter((s) => s.source_id === doc).map((s) => s.code_id));
    h.equal(await page.getByTestId('pt-codeline-row').count(), inDoc.size, 'a row for each code used in the document');
    await axe('Explore, the codeline');
  });

  await S('the Explore tools work from the keyboard: the tabs move with the arrow keys and every control has a name', async () => {
    await tab('Query Builder').focus();
    await page.keyboard.press('ArrowRight');
    h.equal(await tab('Code Matrix').getAttribute('aria-selected'), 'true', 'ArrowRight chooses the next tool');
    await page.keyboard.press('ArrowLeft');
    h.equal(await tab('Query Builder').getAttribute('aria-selected'), 'true', 'and ArrowLeft the one before');
    const unnamed = await page.getByTestId('query-tool').locator('select, input, button').evaluateAll((els) => els.filter((e) => !(e.getAttribute('aria-label') || e.labels?.length || e.textContent.trim() || e.getAttribute('aria-labelledby'))).length);
    h.equal(unnamed, 0, 'every control in the Query Builder has a name');
    await page.getByRole('button', { name: 'Back to the coding workspace' }).first().click();
    await page.getByRole('heading', { level: 2, name: 'Documents' }).waitFor();
  });
}
