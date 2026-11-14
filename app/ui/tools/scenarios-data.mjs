// Importing spreadsheets and problem files, threads, unit-mode coding, and large sources.

import fs from 'node:fs';
import path from 'node:path';

export async function runData(ctx, api) {
  const { h, rpc, files, workDir } = ctx;
  const { page, S, axe, waitStatus, surface, mainTab, sleep, codeRow, addCodeThroughDialog, phraseBox, selectByMouse } = api;

  const importDialog = () => page.getByRole('main', { name: 'Import Data' });
  async function openImport(file) {
    await api.goToImport();
    const dialog = importDialog();
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(file);
    return dialog;
  }
  const sourceNames = async () => (await rpc('source.list')).map((s) => s.name);

  await S('importing a spreadsheet: the wizard guesses the columns and lets you change them', async () => {
    const dialog = await openImport(files.survey);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'Tell PrismQDA what each column is' }).waitFor();
    const role = (col) => dialog.getByLabel(`What column ${col} is for`);
    h.equal(await role('pid').inputValue(), 'id', 'pid is guessed as the person ID');
    h.equal(await role('answer').inputValue(), 'text', 'answer is guessed as open-ended text');
    h.equal(await role('group').inputValue(), 'attribute', 'group is guessed as describing the person');
    h.equal(await dialog.getByLabel('Kind of values in column group').inputValue(), 'nominal', 'group is guessed as categories');
    const guessed = (await rpc('import.inspect', { path: files.survey })).table.columns.find((c) => c.name === 'age').guessed_level;
    h.equal(await dialog.getByLabel('Kind of values in column age').inputValue(), guessed, 'age shows the engine\'s own guess for its kind of values');
    await dialog.getByLabel('Kind of values in column group').selectOption({ label: 'Ordered categories (ordinal)' });
    await axe('the spreadsheet import step');
    // Asking for a preview with no text column explains what is missing.
    await role('answer').selectOption({ label: 'Leave out' });
    await dialog.getByRole('button', { name: 'Show what will be imported' }).click();
    await dialog.getByRole('alert').waitFor();
    h.check(/open-ended text/.test(await dialog.getByRole('alert').textContent()), 'a plain message when no text column is chosen');
    h.equal(await dialog.getByRole('button', { name: 'Import', exact: true }).isDisabled(), true, 'and Import is off until it is');
    await role('answer').selectOption({ label: 'Open-ended text' });
    await dialog.getByRole('button', { name: 'Show what will be imported' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    const summary = await dialog.textContent();
    h.check(/6 cases will be made/.test(summary), `the preview counts the cases: ${summary.slice(summary.indexOf('What will come in'), summary.indexOf('What will come in') + 200)}`);
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    await done.getByRole('button', { name: 'Done' }).click();
    await done.waitFor({ state: 'hidden' });
    const cases = await rpc('case.list');
    h.equal(cases.length, 6, 'six cases in the engine');
    h.check((await sourceNames()).some((n) => /survey/.test(n)), 'the survey source is listed');
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /survey/ }).waitFor();
    await page.screenshot({ path: path.join(ctx.shots, 'after-survey-import.png') });
  });

  await S('importing a file again shows a plain duplicate warning and offers to import again', async () => {
    const dialog = await openImport(files.interview);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    const warning = dialog.getByRole('alert');
    await warning.waitFor();
    h.check(/already imported as/.test(await warning.textContent()), `the duplicate is named: ${await warning.textContent()}`);
    await axe('the duplicate warning');
    await dialog.getByRole('button', { name: 'Import again' }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    await done.getByRole('button', { name: 'Done' }).click();
    h.equal((await sourceNames()).filter((n) => n === 'interview').length, 2, 'the file is now in the project twice');
  });

  await S('switching to another source forgets the selection', async () => {
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /interview/ }).first().click();
    await surface().waitFor();
    const picked = await selectByMouse('felt welcome');
    h.equal(picked, 'felt welcome', 'a selection in the first source');
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /survey/ }).click();
    await page.getByTestId('grid').or(surface()).waitFor();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /interview/ }).first().click();
    await surface().waitFor();
    await sleep(300);
    h.equal((await page.getByTestId('selection-line').textContent()).trim(), '', 'nothing is told as selected under the text');
    h.equal(await page.evaluate(() => window.getSelection().toString()), '', 'and nothing is selected in the text');
  });

  await S('problem files give plain messages from the engine, each at its own row of the queue', async () => {
    fs.writeFileSync(path.join(workDir, 'fake.docx'), 'this is not a Word file');
    fs.writeFileSync(path.join(workDir, 'thing.xyz'), 'x');
    const dialog = await openImport(path.join(workDir, 'nope.txt'));
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const queue = dialog.getByTestId('import-queue');
    await queue.getByText(/cannot read/).waitFor();
    h.check(/Failed/.test(await queue.locator('tr', { hasText: 'nope.txt' }).innerText()), 'a missing file is marked Failed, and says it cannot be read');
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(path.join(workDir, 'fake.docx'));
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await queue.locator('tr', { hasText: 'fake.docx' }).getByText(/Word|docx|zip|damaged|not a/i).first().waitFor();
    h.check(true, 'a damaged Word file is explained in its own row');
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(path.join(workDir, 'thing.xyz'));
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await queue.locator('tr', { hasText: 'thing.xyz' }).getByText(/not a kind of file/).waitFor();
    h.check(true, 'an unsupported kind of file is explained in its own row');
    h.equal(await queue.locator('tbody tr').count(), 3, 'all three are in the queue, each with its own outcome');
    await axe('the import queue with three failed files');
    // Nothing can be imported while all of them failed, and the page says why.
    h.equal(await dialog.getByRole('button', { name: 'Import', exact: true }).isDisabled(), true, 'Import is off when nothing is ready');
    h.check(/tried again, or removed/.test(await dialog.innerText()), 'and the page says what can be done');
    for (let n = 0; n < 3; n++) await queue.getByRole('button', { name: /^Remove / }).first().click();
    await dialog.getByText(/Nothing is queued/).waitFor();
    await page.getByRole('button', { name: 'Back to the coding workspace' }).click();
    await dialog.waitFor({ state: 'hidden' });
  });

  await S('a mixed batch imports what it can, shows each outcome, and a retry adds nothing twice', async () => {
    const good = path.join(workDir, 'batch-good.txt');
    const late = path.join(workDir, 'batch-late.txt');
    fs.writeFileSync(good, 'A made-up note for the batch check. It has two sentences.');
    fs.rmSync(late, { force: true });
    const before = (await rpc('source.list')).length;
    const dialog = await openImport(good);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(late);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const queue = dialog.getByTestId('import-queue');
    await queue.locator('tr', { hasText: 'batch-late.txt' }).getByText('Failed').waitFor();
    await queue.locator('tr', { hasText: 'batch-good.txt' }).getByText('Ready').waitFor();
    h.equal((await queue.locator('tbody tr').count()), 2, 'both are queued, one ready and one failed');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    h.check(/1 file imported, 1 failed\./.test(await done.getByTestId('import-outcome').innerText()), `the outcome counts both: ${await done.getByTestId('import-outcome').innerText()}`);
    h.equal((await rpc('source.list')).length, before + 1, 'the readable file went in');
    h.check(/Complete/.test(await queue.locator('tr', { hasText: 'batch-good.txt' }).innerText()), 'its row says Complete');
    await axe('the import outcome with a failure');
    // The missing file turns up; a retry takes only the one that failed.
    fs.writeFileSync(late, 'The late file for the batch check.');
    await done.getByRole('button', { name: 'Retry the files that failed' }).click();
    await queue.locator('tr', { hasText: 'batch-late.txt' }).getByText('Complete').waitFor();
    h.equal((await rpc('source.list')).length, before + 2, 'the retry added the one that failed and nothing else');
    h.equal((await rpc('source.list')).filter((s) => s.name === 'batch-good').length, 1, 'the file that was complete was not imported again');
    await done.getByRole('button', { name: 'Done' }).click();
    await done.waitFor({ state: 'hidden' });
  });

  await S('reference libraries, e-mail, saved web pages, and news feeds are imported, each told by its kind, with their details kept as variables', async () => {
    const ris = path.join(workDir, 'review.ris');
    fs.writeFileSync(ris, 'TY  - JOUR\nAU  - Hernandez, Maria\nAU  - Li, Wei\nTI  - Bus reliability and daily stress\nJO  - Journal of Urban Mobility\nPY  - 2021///\nAB  - Riders describe delays as a source of stress.\nER  - \n\nTY  - BOOK\nAU  - Okafor, Ngozi\nTI  - Cities in motion\nPY  - 2019\nER  - \n');
    const mbox = path.join(workDir, 'letters.mbox');
    fs.writeFileSync(mbox, 'From maria@example.org Tue Nov  3 14:05:00 2026\nFrom: Maria Lopez <maria@example.org>\nSubject: Check one\nDate: Tue, 3 Nov 2026 14:05:00 +0000\n\nThe first made-up letter.\n\nFrom wei@example.org Wed Nov  4 09:00:00 2026\nFrom: wei@example.org\nSubject: Check two\n\nThe second made-up letter.\n');
    const feed = path.join(workDir, 'town-news.rss');
    fs.writeFileSync(feed, '<?xml version="1.0"?><rss version="2.0"><channel><title>Town News</title><item><title>Buses run late</title><pubDate>Tue, 3 Nov 2026 14:05:00 GMT</pubDate><description>Riders waited a long time on Monday.</description></item></channel></rss>');
    const saved = path.join(workDir, 'saved-page.mhtml');
    fs.writeFileSync(saved, 'From: <Saved by a browser>\nSnapshot-Content-Location: https://example.org/page\nSubject: A saved page\nDate: Tue, 3 Nov 2026 14:05:00 +0000\nMIME-Version: 1.0\nContent-Type: text/html; charset=utf-8\n\n<html><body><h1>A saved page</h1><p>Words that were saved with the page.</p></body></html>\n');

    const before = (await rpc('source.list')).length;
    const casesBefore = (await rpc('case.list')).length;
    const dialog = await openImport(ris);
    const queue = dialog.getByTestId('import-queue');
    for (const [i, file] of [ris, mbox, feed, saved].entries()) {
      if (i > 0) await dialog.getByRole('textbox', { name: 'File to import' }).fill(file);
      await dialog.getByRole('button', { name: 'Check this file' }).click();
      await queue.locator('tr', { hasText: path.basename(file) }).getByText('Ready').waitFor();
    }
    const kindOf = async (file) => (await queue.locator('tr', { hasText: path.basename(file) }).innerText()).replace(/\s+/g, ' ');
    h.check(/References \(2\)/.test(await kindOf(ris)), `a reference library says what it holds: ${await kindOf(ris)}`);
    h.check(/E-mail messages \(2\)/.test(await kindOf(mbox)), `a mailbox too: ${await kindOf(mbox)}`);
    h.check(/News item/.test(await kindOf(feed)), `a feed: ${await kindOf(feed)}`);
    h.check(/Saved web page/.test(await kindOf(saved)), `and a saved web page: ${await kindOf(saved)}`);
    // Every kind can be chosen.
    for (const name of [/^Reference Library/, /^Web \/ News/, /^Email/, /^Documents/]) h.equal(await dialog.getByRole('button', { name }).isEnabled(), true, `${name} can be chosen`);
    h.equal(await dialog.getByRole('button', { name: /^Audio \/ Video/ }).isEnabled(), true, 'Audio / Video can be chosen: a recording comes in through its captions');
    h.equal(await dialog.getByRole('button', { name: /^Images/ }).isEnabled(), true, 'Images can be chosen: a picture is kept inside the project');
    h.check(/does not fetch an address/.test(await dialog.getByTestId('kinds-note').innerText()), 'Web / News says it reads saved pages and feeds and fetches nothing');
    await queue.locator('tr', { hasText: 'review.ris' }).getByRole('button', { name: /^Remove / }).waitFor();
    await axe('the Import Data page with reference, e-mail, web, and feed files queued');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    await page.getByTestId('import-finished').waitFor();
    h.equal((await rpc('source.list')).length, before + 6, 'two references, two messages, one news item, and one page went in');
    const names = (await rpc('source.list')).map((s) => s.name);
    h.check(names.includes('Hernandez & Li 2021 \u2013 Bus reliability and daily stress') && names.includes('Buses run late \u2013 2026-11-03') && names.includes('A saved page'), `they are named for what they are: ${names.slice(-6).join(' | ')}`);
    const folders = new Set((await rpc('source.list')).filter((s) => ['References', 'Email', 'News', 'Web pages'].includes(s.folder)).map((s) => s.folder));
    h.equal([...folders].sort(), ['Email', 'News', 'References', 'Web pages'], 'each kind is filed in a folder of its own');
    const cases = await rpc('case.list');
    h.equal(cases.length, casesBefore + 6, 'each has a case, so its details are variables');
    await page.getByTestId('import-finished').getByRole('button', { name: 'Done' }).click();
    await page.getByTestId('import-finished').waitFor({ state: 'hidden' });
    const back = page.getByRole('button', { name: 'Back to the coding workspace' });
    if ((await back.count()) > 0) await back.first().click();
  });

  await S('the destination folder and the case are chosen before the import, and a duplicate under a new name is still told', async () => {
    const note = path.join(workDir, 'placed-note.txt');
    fs.writeFileSync(note, 'A note that is placed in a folder and a case. It has a second sentence.');
    const dialog = await openImport(note);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    h.check(/placed in a folder/.test(await dialog.getByTestId('import-preview-text').innerText()), 'the first words of the file are shown before anything is imported');
    await dialog.getByRole('textbox', { name: 'Destination folder' }).fill('Placed notes');
    await dialog.getByRole('combobox', { name: 'Case', exact: true }).fill('Placed case');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    const source = (await rpc('source.list')).find((s) => s.name === 'placed-note');
    h.equal(source.folder, 'Placed notes', 'the document is in the folder chosen');
    const cases = await rpc('case.list');
    const placed = cases.find((c) => c.name === 'Placed case');
    h.check(!!placed && placed.source_ids.includes(source.id), 'a case was made for the name typed, and the document belongs to it');
    await done.getByRole('button', { name: 'Import another file' }).click();
    // The same words under another name are the same file: it is told, and the choices are skip, import again, or (not yet) link.
    const renamed = path.join(workDir, 'placed-note-renamed.txt');
    fs.copyFileSync(note, renamed);
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(renamed);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const box = dialog.getByTestId('duplicate-box');
    await box.waitFor();
    h.check(/already imported as/.test(await box.innerText()) && /even if its name is different/.test(await box.innerText()), 'the renamed copy is flagged as a duplicate');
    h.equal(await box.getByRole('button', { name: 'Link as a revision' }).count(), 0, 'an identical file is not offered as a revision: there is nothing new in it');
    h.equal(await box.getByRole('button').allTextContents().then((all) => all.map((x) => x.trim())), ['Skip this file', 'Import again'], 'the choices for a duplicate are to skip it or import it again');
    await axe('the duplicate choices');
    await box.getByRole('button', { name: 'Skip this file' }).click();
    await dialog.getByTestId('import-queue').locator('tr', { hasText: 'placed-note-renamed.txt' }).getByText('Skipped').waitFor();
    h.equal((await rpc('source.list')).filter((s) => /^placed-note/.test(s.name)).length, 1, 'skipping imported nothing');
    await page.getByRole('button', { name: 'Back to the coding workspace' }).click();
  });

  await S('a newer version of a document replaces its text after a plain warning, and the codings follow the words that survive', async () => {
    const note = (await rpc('source.list')).find((s) => s.name === 'placed-note');
    const cost = (await rpc('code.list')).find((c) => c.name === 'Cost');
    const first = 'A note that is placed in a folder and a case. It has a second sentence.';
    const start = Array.from(first).length - 'second sentence.'.length;
    await rpc('coding.apply', { source_id: note.id, start, end: start + 'second sentence'.length, code_id: cost.id });
    const sourcesBefore = (await rpc('source.list')).length;
    // The coding was made behind the screen's back, so the page is loaded again to know about it.
    await page.reload();
    await mainTab('Coding').waitFor();

    const newer = path.join(workDir, 'placed-note-v2.txt');
    fs.writeFileSync(newer, 'A note that is placed in a folder and a case. It has a much longer second sentence now.');
    const dialog = await openImport(newer);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const section = dialog.getByTestId('revise-section');
    await section.waitFor();
    h.check(/Undo cannot take this back/.test(await section.innerText()), 'the section says that Undo cannot take it back');
    h.equal(await section.getByTestId('revise-ask').isDisabled(), true, 'it waits for a document to be chosen');
    await axe('the Import Data page offering a newer version of a document');
    await section.getByLabel('The document this file is a newer version of').selectOption(String(note.id));
    await section.getByTestId('revise-ask').click();
    const sure = section.getByTestId('revise-sure');
    await sure.waitFor();
    h.check(/Replace the text of placed-note with placed-note-v2\.txt/.test(await sure.innerText()) && /1 coding will follow the words that survive/.test(await sure.innerText()), 'it asks again, naming the document, the file, and what happens to the coding');
    h.equal((await rpc('source.get', { id: note.id })).text, first, 'nothing has changed yet');
    await axe('the question before a newer version replaces a document');
    await sure.getByTestId('revise-confirm').click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    const said = await done.innerText();
    h.check(/Replaced the text of placed-note with placed-note-v2\.txt/.test(said) && /1 coding moved with its words/.test(said) && /none was removed/.test(said) && /A backup copy of the project was saved first/.test(said), `the finish tells what happened: ${said.replace(/\s+/g, ' ').slice(0, 260)}`);
    h.equal((await rpc('source.list')).length, sourcesBefore, 'no second copy of the document was made');
    const detail = await rpc('source.get', { id: note.id });
    h.check(/much longer second sentence now\.$/.test(detail.text), 'the document has the new text');
    const coding = (await rpc('coding.list', { source_id: note.id }))[0];
    h.equal(Array.from(detail.text).slice(coding.start, coding.end).join(''), 'second sentence', 'and the coding follows its words');
    const events = await rpc('events.list', { limit: 10 });
    h.check(events.some((e) => e.action === 'source.revise'), 'History has the revision on record');
    await axe('the Import Data page after a newer version replaced a document');
    await done.getByRole('button', { name: 'Done' }).click();
  });

  await S('thread view lists replies under their posts, indented', async () => {
    // The engine cannot import threaded posts yet, so the unit list of one source is changed in
    // flight to make each unit reply to the one before it.
    const survey = (await rpc('source.list')).find((s) => /survey/.test(s.name));
    await page.route('**/rpc', async (route) => {
      const body = JSON.parse(route.request().postData() ?? '{}');
      if (body.cmd === 'unit.list' && body.args.source_id === survey.id) {
        const response = await route.fetch();
        const json = await response.json();
        json.result = json.result.map((u, i, all) => ({ ...u, parent_id: i === 0 ? null : all[i - 1].id }));
        await route.fulfill({ response, json });
      } else await route.continue();
    });
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /interview/ }).first().click();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /survey/ }).click();
    const thread = page.getByTestId('thread');
    await thread.waitFor();
    const levels = await thread.locator('[role=row][aria-level]').evaluateAll((rows) => rows.map((r) => r.getAttribute('aria-level')));
    h.equal(levels.slice(0, 4), ['1', '2', '3', '4'], 'each reply is one level deeper');
    h.check(/Reply, level 2/.test(await thread.locator('[role=row][aria-level="2"]').first().textContent()), 'the level is also in the row text for screen readers');
    await axe('the thread view');
    await page.unroute('**/rpc');
    // Back to a plain list for the remaining steps.
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /interview/ }).first().click();
  });

  // ---------------------------------------------------------------------------- unit mode
  await S('a project that codes whole units: a new project, a transcript, speakers shown', async () => {
    await api.closeProjectViaMenu();
    h.check((await page.getByRole('heading', { level: 2, name: 'Recent Projects' }).count()) === 1, 'the project just closed is listed as recent');
    h.check((await page.getByRole('region', { name: 'Recent Projects' }).getByText('Study A').count()) >= 1, 'and it is Study A');
    await page.getByRole('button', { name: 'New Project', exact: true }).click();
    await page.getByRole('textbox', { name: 'Project name' }).fill('Study B');
    await page.getByRole('textbox', { name: 'Save location' }).fill(workDir);
    // How to code is one of the optional study details.
    await page.getByRole('button', { name: 'Optional study details' }).click();
    await page.getByText('Code whole units', { exact: true }).click();
    await page.getByRole('button', { name: 'Create Project' }).click();
    await mainTab('Coding').waitFor();
    h.equal((await rpc('project.info')).coding_mode, 'unit', 'the engine has unit mode');
    await mainTab('Home').click();
    await page.getByRole('button', { name: /^About Study B/ }).click();
    h.check(/Whole units/.test(await page.getByRole('dialog', { name: 'About this project' }).innerText()), 'the dashboard says how this project codes');
    await page.keyboard.press('Escape');
    await mainTab('Coding').click();

    const dialog = await openImport(files.transcript);
    await dialog.getByText('More choices').click();
    await dialog.getByLabel('Kind of material').selectOption({ label: 'Transcript' });
    await dialog.getByRole('textbox', { name: 'Interviewer names in a transcript' }).fill('Interviewer');
    await axe('the import dialog with more choices open');
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    h.check(/speaker turns?/.test(await dialog.textContent()), 'the units are speaker turns');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    await done.getByRole('button', { name: 'Done' }).click();
    await surface().waitFor();
    const units = await rpc('unit.list', { source_id: 1 });
    h.equal(units.length, 6, 'six turns');
    h.equal(units.filter((u) => u.excluded).length, 3, 'the interviewer turns are left out of coding');
    const speakers = await page.locator('.seg[data-speaker]').evaluateAll((els) => els.map((e) => e.getAttribute('data-speaker')));
    h.check(speakers.includes('Interviewer') && speakers.includes('Participant'), `speakers are shown: ${speakers.join(', ')}`);
    await page.screenshot({ path: path.join(ctx.shots, 'unit-mode-transcript.png') });
    await axe('a transcript in unit mode');
  });

  await S('unit mode: a number key codes the whole unit at the caret; N goes to the next unit with no coding', async () => {
    await addCodeThroughDialog('Welcome', 'Feeling accepted.');
    await codeRow('Welcome').click();
    await api.codeAction('Pin as favorite');
    await waitStatus(/favorite number 1/);
    // Put the caret in the first participant turn with a mouse click.
    const box = await phraseBox('felt welcome right away');
    await page.mouse.click(box.x0 + 10, box.y0);
    await page.keyboard.press('1');
    const said = await waitStatus(/Coded 'A friend told me about it/);
    h.check(/I was nervous at first but felt welcome right away/.test(said), `the whole turn was coded: ${said}`);
    const units = await rpc('unit.list', { source_id: 1 });
    const codings = await rpc('coding.list', { source_id: 1 });
    const first = units.find((u) => !u.excluded);
    h.equal(codings.length, 1, 'one coding');
    h.equal({ start: codings[0].start, end: codings[0].end, unit_id: codings[0].unit_id }, { start: first.start, end: first.end, unit_id: first.id }, 'the coding is exactly the first participant turn');
    // Next uncoded unit.
    await page.keyboard.press('n');
    const moved = await waitStatus(/Unit \d of 6, not coded yet/);
    h.check(/Participant/.test(moved), `N goes to the next participant turn, skipping the interviewer: ${moved}`);
    await page.keyboard.press('1');
    await waitStatus(/Coded 'The cost/);
    await page.keyboard.press('Shift+n');
    const back = await waitStatus(/not coded yet|No earlier/);
    h.check(/No earlier/.test(back), `Shift+N looks backward and finds nothing before the first uncoded unit: ${back}`);
    // The progress bar counts units, not characters.
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).waitFor();
    // The row's name carries the progress, in words, for screen readers and for this check.
    const progress = await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).getAttribute('aria-label');
    h.check(/2 of 3 units coded/.test(progress), `the documents list counts units: ${progress}`);
  });

  await S('unit mode: a turn that is left out of coding cannot be coded, and says why', async () => {
    const box = await phraseBox('Thank you for joining me today');
    await page.mouse.click(box.x0 + 10, box.y0);
    await page.keyboard.press('1');
    await page.getByTestId('alert').filter({ hasText: /left out of coding/ }).waitFor();
    h.check(true, 'the refusal is announced');
    const refused = (await rpc('coding.list', { source_id: 1 })).length;
    h.equal(refused, 2, 'nothing was added');
  });

  await S('unit mode: selected words inside a unit are coded as part of that unit', async () => {
    const text = await surface().evaluate((el) => el.textContent);
    const phrase = 'More evening hours';
    const before = text.slice(0, text.indexOf(phrase));
    const count = [...new Intl.Segmenter('en', { granularity: 'grapheme' }).segment(before)].length;
    await surface().focus();
    await page.keyboard.press('Control+Home');
    for (let i = 0; i < count; i++) await page.keyboard.press('ArrowRight');
    for (let i = 0; i < phrase.length; i++) await page.keyboard.press('Shift+ArrowRight');
    await page.keyboard.press('1');
    await waitStatus(/Coded 'More evening hours' as Welcome/);
    const last = (await rpc('coding.list', { source_id: 1 })).at(-1);
    const unit = (await rpc('unit.list', { source_id: 1 })).find((u) => u.id === last.unit_id);
    h.check(unit && last.start >= unit.start && last.end <= unit.end && last.end - last.start === phrase.length, 'the span lies inside its unit');
  });

  await S('headings and emphasis from an imported document are shown', async () => {
    const dialog = await openImport(files.notes);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    await done.getByRole('button', { name: 'Done' }).click();
    await surface().waitFor();
    await page.locator('.seg.h1').first().waitFor();
    h.equal((await page.locator('.seg.h1').first().textContent()).trim(), 'Findings', 'the first heading');
    h.check((await page.locator('.seg.h2').count()) >= 1, 'a second-level heading');
    h.equal((await page.locator('.seg.b').first().textContent()).trim(), 'kind', 'bold text');
    h.equal((await page.locator('.seg.i').first().textContent()).trim(), 'schedule', 'italic text');
    const sizes = await page.evaluate(() => ({
      h1: parseFloat(getComputedStyle(document.querySelector('.seg.h1')).fontSize),
      body: parseFloat(getComputedStyle(document.querySelector('.surface')).fontSize),
      bold: getComputedStyle(document.querySelector('.seg.b')).fontWeight,
      italic: getComputedStyle(document.querySelector('.seg.i')).fontStyle,
    }));
    h.check(sizes.h1 > sizes.body * 1.3, `the heading is bigger than the text: ${sizes.h1}px against ${sizes.body}px`);
    h.equal(sizes.italic, 'italic', 'italic is italic');
    h.check(Number(sizes.bold) >= 600, `bold is bold: ${sizes.bold}`);
    await page.screenshot({ path: path.join(ctx.shots, 'formatting.png') });
  });

  // ---------------------------------------------------------------------------- big sources
  await S('a source with 15,000 units opens in the windowed grid and stays responsive', async () => {
    const dialog = await openImport(files.big);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'Tell PrismQDA what each column is' }).waitFor();
    h.equal(await dialog.getByLabel('What column id is for').inputValue(), 'id', 'the id column is guessed');
    h.equal(await dialog.getByLabel('What column text is for').inputValue(), 'text', 'the text column is guessed');
    await dialog.getByRole('button', { name: 'Show what will be imported' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor({ timeout: 90000 });
    const started = Date.now();
    await done.getByRole('button', { name: 'Done' }).click();
    const grid = page.getByTestId('grid');
    await grid.waitFor({ timeout: 30000 });
    const opened = Date.now() - started;
    console.log(`       (the 15,000-unit grid appeared ${opened} ms after the dialog closed)`);
    h.check(opened < 5000, `the grid appears quickly: ${opened} ms`);
    h.equal(await grid.getAttribute('aria-rowcount'), '15001', 'screen readers are told how many rows there are');
    const rendered = await grid.locator('[role=row]').count();
    h.check(rendered < 80, `only the rows on screen are in the page: ${rendered}`);
    h.check(/15,000 units|15,000 responses/.test((await grid.getAttribute('aria-label')) ?? ''), `the grid is named with its size: ${await grid.getAttribute('aria-label')}`);
    await page.screenshot({ path: path.join(ctx.shots, 'big-grid.png') });
    await axe('the grid of 15,000 units');
  });

  await S('the grid moves by keyboard, codes rows with a number key, and the Document view asks first', async () => {
    const grid = page.getByTestId('grid');
    await grid.focus();
    const t0 = Date.now();
    await page.keyboard.press('End');
    await page.getByRole('row', { name: /Response 15000/ }).waitFor();
    const jump = Date.now() - t0;
    console.log(`       (End key to the last of 15,000 rows: ${jump} ms)`);
    h.check(jump < 3000, `the End key reaches the last row quickly: ${jump} ms`);
    const rendered = await grid.locator('[role=row]').count();
    h.check(rendered < 80, `still only the visible rows are in the page: ${rendered}`);
    h.equal(await grid.getAttribute('aria-activedescendant') !== null, true, 'the active row is announced through aria-activedescendant');
    // Extend the selection upward by three rows and code them all.
    for (let i = 0; i < 3; i++) await page.keyboard.press('Shift+ArrowUp');
    await page.waitForFunction(() => /Selected 4 rows/.test(document.querySelector('[data-testid=selection-line]').textContent));
    const t1 = Date.now();
    await page.keyboard.press('1');
    await waitStatus(/Coded 4 units as Welcome/);
    const coded = Date.now() - t1;
    console.log(`       (coding 4 rows of a 15,000-unit source: ${coded} ms)`);
    h.check(coded < 2000, `coding is quick: ${coded} ms`);
    const big = (await rpc('source.list')).find((s) => s.units === 15000);
    h.equal((await rpc('coding.list', { source_id: big.id })).length, 4, 'four codings in the engine');
    await page.waitForFunction(() => [...document.querySelectorAll('[role=treegrid][aria-label="Documents"] [role=row]')].some((r) => /4 of 15,000 units coded/.test(r.getAttribute('aria-label') ?? '')));
    h.check(true, 'the progress text counts units');
    // The Document view asks before drawing 15,000 units.
    // The way to show a document is in the reader's More menu ("Show this document as").
    await page.getByRole('main').getByRole('button', { name: 'More document actions' }).click();
    await page.getByRole('menuitemradio', { name: 'Document view' }).click();
    await page.getByText(/puts all of its text on the page at once/).waitFor();
    await axe('the large-source warning');
    await page.getByRole('button', { name: 'Use the Grid view' }).click();
    await page.getByTestId('grid').waitFor();
  });

  await S('a long document with many codings shows all its stripes quickly in the Document view', async () => {
    const dialog = await openImport(files.medium);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    await done.getByRole('button', { name: 'Done' }).click();
    await surface().waitFor();
    const medium = (await rpc('source.list')).find((s) => /medium/.test(s.name));
    const welcome = (await rpc('code.list')).find((c) => c.name === 'Welcome');
    const units = await rpc('unit.list', { source_id: medium.id });
    h.equal(units.length, 800, 'eight hundred sentences');
    for (let i = 0; i < units.length; i += 3) {
      await rpc('coding.apply', { source_id: medium.id, start: units[i].start, end: units[i].end, code_id: welcome.id, unit_id: units[i].id });
    }
    // Leave and come back so the interface loads all of it fresh.
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).click();
    const t0 = Date.now();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /medium-document/ }).click();
    await surface().waitFor();
    await page.waitForFunction(() => document.querySelectorAll('.stripe[data-coding]').length >= 267, null, { timeout: 20000 });
    const ms = Date.now() - t0;
    const stripes = await page.locator('.stripe[data-coding]').count();
    console.log(`       (document of ${medium.characters} characters with ${stripes} stripes ready in ${ms} ms)`);
    h.equal(stripes, 267, 'a stripe for every coding');
    h.check(ms < 4000, `the document and its stripes are ready quickly: ${ms} ms`);
    // Scrolling keeps working and the stripes follow the text.
    const first = await page.locator('.stripe[data-coding]').first().boundingBox();
    h.check(first !== null, 'the first stripe has a position');
    await surface().focus();
    await page.keyboard.press('Control+End');
    await sleep(200);
    h.check((await page.evaluate(() => document.querySelector('.surface-scroll').scrollTop)) > 1000, 'the document scrolled to its end');
    await axe('a long coded document');
  });

  await S('the biggest source that opens in the Document view (about 380,000 characters) stays usable', async () => {
    const dialog = await openImport(files.long);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const done = page.getByTestId('import-finished');
    await done.waitFor({ timeout: 60000 });
    const big = (await rpc('source.list')).find((s) => /long-document/.test(s.name));
    h.check(big.characters > 350000 && big.characters <= 400000, `about 380,000 characters: ${big.characters}`);
    h.check(big.units <= 1000, `under 1,000 units, so the Document view is the default: ${big.units}`);
    const welcome = (await rpc('code.list')).find((c) => c.name === 'Welcome');
    const units = await rpc('unit.list', { source_id: big.id });
    let made = 0;
    for (let i = 0; i < units.length; i += 3) {
      await rpc('coding.apply', { source_id: big.id, start: units[i].start, end: units[i].end, code_id: welcome.id, unit_id: units[i].id });
      made++;
    }
    await done.getByRole('button', { name: 'Done' }).click();
    await surface().waitFor();
    // Leave and come back so the interface loads the codings made behind its back.
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).click();
    const t0 = Date.now();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /long-document/ }).click();
    await surface().waitFor();
    await page.waitForFunction((n) => document.querySelectorAll('.stripe[data-coding]').length >= n, made, { timeout: 60000 });
    const ready = Date.now() - t0;
    console.log(`       (${big.characters} characters with ${made} codings: text and stripes ready in ${ready} ms)`);
    h.check(ready < 8000, `the long document and its stripes are ready: ${ready} ms`);
    // Keyboard movement and coding stay quick.
    await surface().focus();
    await page.keyboard.press('Control+Home');
    for (let i = 0; i < 40; i++) await page.keyboard.press('ArrowDown');
    const t1 = Date.now();
    await page.keyboard.press('1');
    await waitStatus(/Coded '/);
    const coded = Date.now() - t1;
    console.log(`       (coding a unit in the long document: ${coded} ms)`);
    h.check(coded < 3000, `coding a unit is quick: ${coded} ms`);
    // Typing into the text is still blocked at this size.
    const length = await surface().evaluate((el) => el.textContent.length);
    await page.keyboard.type('xyz');
    h.equal(await surface().evaluate((el) => el.textContent.length), length, 'the text is unchanged');
  });
}
