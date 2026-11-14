// Pseudonymize and redact: a list of names is written, suggestions are taken, what the list would do is previewed, the list is applied to
// what leaves the project (checked inside a file that was exported), and then the names are replaced in the project for good, with the key
// kept as a file and the list gone from the project. The names are unusual on purpose (nothing else in the check's project holds them),
// because replacing them for good changes every document that does.

import fs from 'node:fs';
import path from 'node:path';

const TEXT =
  'Zelda Quimby told me the bus was late. I called her on 555-014-2778 and wrote to zelda.quimby@example.test about it.\n\n' +
  'We met in Quimbyville because Zelda lives there. Later Zelda Quimby said Quimbyville was far, and the Quimbyville trip took an hour.';

export async function runPrivacy(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const main = () => page.getByRole('main', { name: 'Pseudonymize and redact' });
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  const state = () => rpc('deid.state');
  /** Waits (the interface asks the engine and answers a moment later) until the project's list or switch is what the step expects. */
  const until = async (ok, what) => {
    for (let i = 0; i < 60; i++) {
      if (ok(await state())) return;
      await page.waitForTimeout(100);
    }
    h.check(false, `waited for ${what}`);
  };
  const goToPage = async () => {
    const back = page.getByRole('button', { name: 'Back to the coding workspace' });
    if ((await back.count()) > 0) await back.first().click();
    await page.getByRole('tab', { name: 'Import', exact: true }).click();
    await page.getByRole('button', { name: 'Pseudonymize and redact', exact: true }).click();
    await main().waitFor();
  };
  let source;
  let late;

  await S('the page is reached from the Import tab and says what it is for, with an empty list', async () => {
    const file = path.join(workDir, 'Interview with Zelda Quimby.txt');
    fs.writeFileSync(file, TEXT);
    late = (await rpc('code.add', { name: 'Lateness' })).id;
    await rpc('import.run', { path: file });
    source = (await rpc('source.list')).find((s) => s.name === 'Interview with Zelda Quimby');
    const start = TEXT.indexOf('the bus was late');
    await rpc('coding.apply', { source_id: source.id, start, end: start + 16, code_id: late });
    await rpc('case.add', { name: 'Zelda Quimby', notes: 'lives in Quimbyville' });
    await rpc('memo.add', { kind: 'analytic', scope: 'standalone', title: 'About Zelda', body: 'Zelda seemed nervous.' });
    await page.reload();
    await goToPage();
    const intro = await text(main());
    h.check(/A pseudonym is a made-up name/.test(intro) && /Read the result before you share anything/.test(intro), 'the page says what the words mean and warns that the list is exact');
    h.equal(await page.getByTestId('privacy-empty').count(), 1, 'the list starts empty');
    h.equal((await state()).rules.length, 0, 'and the project holds no rules');
    h.equal(await page.getByTestId('privacy-apply').isDisabled(), true, 'nothing can be replaced for good with an empty list');
    await axe('the pseudonymize and redact page with an empty list');
  });

  await S('words are added to the list one to a line with one label, and a bad label is refused in words', async () => {
    await page.getByTestId('privacy-terms').fill('Zelda Quimby\nZelda');
    await page.getByTestId('privacy-replacement').fill('Smith, J.');
    await page.getByTestId('privacy-save').click();
    h.check(/comma/.test(await text(main().getByRole('alert').first())), 'a label with a comma is refused, and the reason is a plain sentence');
    h.equal((await state()).rules.length, 0, 'nothing was added');
    await page.getByTestId('privacy-replacement').fill('Participant 3');
    await page.getByTestId('privacy-save').click();
    await page.getByTestId('privacy-table').waitFor();
    const rows = page.getByTestId('privacy-row');
    h.equal(await rows.count(), 1, 'the two lines with the same label are one line of the list');
    const row = await text(rows.first());
    h.check(/Zelda Quimby, Zelda/.test(row) && /Participant 3/.test(row), `the line shows its words and the label: ${row}`);
    h.equal((await state()).rules.map((r) => r.term), ['Zelda Quimby', 'Zelda'], 'the engine holds one rule for each way of writing the name');
    await page.getByTestId('privacy-terms').fill('zelda');
    await page.getByTestId('privacy-replacement').fill('Someone');
    await page.getByTestId('privacy-save').click();
    await page.waitForFunction(() => document.getElementById('privacy-terms').value === '', null, { timeout: 5000 });
    h.equal((await state()).rules.length, 2, 'the same word with the same case rule is not added a second time');
  });

  await S('a line is changed in place', async () => {
    await page.getByRole('button', { name: 'Change Participant 3' }).click();
    h.equal(await page.getByTestId('privacy-terms').inputValue(), 'Zelda Quimby\nZelda', 'the form is filled with the line');
    await page.getByTestId('privacy-replacement').fill('Participant 4');
    await page.getByTestId('privacy-save').click();
    await page.getByRole('button', { name: 'Change Participant 4' }).waitFor();
    h.equal((await state()).rules.map((r) => r.replacement), ['Participant 4', 'Participant 4'], 'the label changed for both words');
    await page.getByRole('button', { name: 'Change Participant 4' }).click();
    await page.getByTestId('privacy-replacement').fill('Participant 3');
    await page.getByTestId('privacy-save').click();
    await page.getByRole('button', { name: 'Change Participant 3' }).waitFor();
  });

  await S('suggestions find the rest, with labels, and the ticked ones are added', async () => {
    await page.getByTestId('privacy-look').click();
    const table = page.getByTestId('privacy-suggestions');
    await table.waitFor();
    const body = await text(table);
    h.check(/zelda\.quimby@example\.test/.test(body) && /555-014-2778/.test(body) && /Quimbyville/.test(body), `the address, the number, and the place are suggested: ${body}`);
    h.check(!/Zelda Quimby/.test(body), 'what is already in the list is not suggested again');
    h.check(/Email address/.test(body) && /Phone number/.test(body), 'each says what it looks like');
    for (const term of ['zelda.quimby@example.test', '555-014-2778', 'Quimbyville']) {
      await table.getByRole('row', { name: new RegExp(term.replace(/[.]/g, '\\.')) }).getByTestId('privacy-suggestion-tick').click();
    }
    await page.getByTestId('privacy-add-ticked').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=privacy-row]').length >= 4, null, { timeout: 5000 });
    const rules = (await state()).rules;
    h.equal(rules.length, 5, 'five words are in the list now');
    const email = rules.find((r) => r.term === 'zelda.quimby@example.test');
    h.equal([email.kind, email.replacement, email.whole_word], ['redact', '[email]', false], 'an address is redacted with a mark, and is not looked for as a whole word');
    const place = rules.find((r) => r.term === 'Quimbyville');
    h.equal([place.kind, place.match_case], ['pseudonym', true], 'a place is given a pseudonym, and its capital letter matters');
  });

  await S('the preview counts what the list finds, shows before and after, and lists the other places', async () => {
    await page.getByTestId('privacy-show-preview').click();
    const body = page.getByTestId('privacy-preview-body');
    await body.waitFor();
    const said = await text(body);
    h.check(/finds 8 things in 1 document/.test(said), `eight things in one document: ${said.slice(0, 120)}`);
    h.check(/Participant 3 told me the bus was late/.test(said) && /\[phone\]/.test(said), 'an example shows the words after the change');
    h.check(/Document names: 1 time/.test(said) && /Memos: 2 times/.test(said) && /Cases: 2 times/.test(said) && /The audit trail/.test(said), 'the other places that would change for good are listed');
    const found = await page.getByTestId('privacy-row').first().innerText();
    h.check(/\b3\b/.test(found), `the list shows how many times the first line was found: ${found}`);
    h.equal((await rpc('source.get', { id: source.id })).text.startsWith('Zelda Quimby'), true, 'nothing was changed by looking');
    await axe('the pseudonymize and redact page with a list, suggestions, and a preview');
    await page.screenshot({ path: `${ctx.shots}/privacy.png`, fullPage: true });
  });

  await S('the switch for exports applies the list to a file that is exported, and the project keeps its names', async () => {
    await page.getByTestId('privacy-exports-switch').click();
    await until((s) => s.exports, 'the switch to be on');
    h.equal((await state()).exports, true, 'the switch is on in the project');
    const csv = path.join(workDir, 'privacy-segments.csv');
    await rpc('export.segments', { format: 'csv', path: csv, overwrite: true });
    const exported = fs.readFileSync(csv, 'utf8');
    h.check(!/Zelda|Quimby/.test(exported), 'the exported file holds none of the names');
    h.check(/Interview with Participant 3/.test(exported), 'and holds what took their place');
    h.check((await rpc('source.get', { id: source.id })).text.startsWith('Zelda Quimby told me'), 'the text of the project is not changed');
    await page.getByRole('tab', { name: 'Report', exact: true }).click();
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Export' });
    await dialog.waitFor();
    h.check(/Your list of names is being applied/.test(await text(dialog)), 'the export window says that the list is being applied');
    await axe('the export window while the list is applied');
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await goToPage();
    await page.getByTestId('privacy-exports-switch').click();
    await until((s) => !s.exports, 'the switch to be off');
    h.equal((await state()).exports, false, 'the switch is off again');
  });

  await S('a copy to share has the names replaced and leaves this project, its list, and its backups as they are', async () => {
    const copy = path.join(workDir, 'privacy-shared.pqda');
    const key = path.join(workDir, 'privacy-key-copy.csv');
    fs.rmSync(copy, { force: true });
    fs.rmSync(key, { force: true });
    // The button is off for a moment after the switch above was turned, while the page finishes that; the check waits for it instead of asking at once.
    await page.waitForFunction(() => document.querySelector('[data-testid=privacy-make-copy]')?.disabled === false, null, { timeout: 8000 }).catch(() => {});
    h.equal(await page.getByTestId('privacy-make-copy').isDisabled(), false, 'the button is there once the list is not empty');
    await page.getByTestId('privacy-key-path').fill(key);
    await page.getByTestId('privacy-copy-path').fill(copy);
    await page.getByTestId('privacy-make-copy').click();
    const outcome = page.getByTestId('privacy-copy-outcome');
    await outcome.waitFor({ timeout: 20000 });
    const said = await text(outcome);
    h.check(/Made the copy privacy-shared\.pqda, with 8 names and numbers replaced in 1 document/.test(said), `the copy is told: ${said.slice(0, 200)}`);
    h.check(/This project was not changed, and its list is still here/.test(said) && /key was saved as privacy-key-copy\.csv/.test(said), 'the project and the key are told');
    h.check(/finds nothing left in the copy/.test(said), 'nothing is left in the copy');
    h.check(fs.existsSync(copy), 'the copy is a file');
    const bytes = fs.readFileSync(copy).toString('latin1');
    h.check(!/Zelda|Quimby|555-014-2778|zelda\.quimby/.test(bytes), 'the copy, read as bytes, holds no name');
    h.check(/Zelda Quimby,Participant 3/.test(fs.readFileSync(key, 'utf8')), 'the key file holds the words and what replaced them');
    h.check((await rpc('source.get', { id: source.id })).text.startsWith('Zelda Quimby told me'), 'the text of this project is not changed');
    h.equal((await state()).rules.length, 5, 'and its list is still there');
    h.check(!fs.existsSync(copy + '-wal') && !fs.existsSync(path.join(workDir, 'privacy-shared.backups')), 'nothing was left beside the copy');
    await axe('the pseudonymize and redact page after a copy was made');
    fs.rmSync(copy, { force: true });
  });

  await S('changing the project for good needs a tick, saves the key first, and leaves no list and no names', async () => {
    const key = path.join(workDir, 'privacy-key.csv');
    await page.getByTestId('privacy-key-path').fill(key);
    h.equal(await page.getByTestId('privacy-apply').isDisabled(), true, 'the button waits for the tick');
    await page.getByTestId('privacy-confirm').click();
    h.equal(await page.getByTestId('privacy-apply').isDisabled(), false, 'and wakes when it is ticked');
    await page.getByTestId('privacy-apply').click();
    const outcome = page.getByTestId('privacy-outcome');
    await outcome.waitFor({ timeout: 20000 });
    const said = await text(outcome);
    h.check(/Replaced 8 names and numbers in 1 document/.test(said), `eight replacements in one document: ${said.slice(0, 160)}`);
    h.check(/backup, with the names as they were/.test(said) && /key was saved as privacy-key\.csv/.test(said), 'the backup and the key are told');
    h.check(/finds nothing left in the project/.test(said), 'nothing is left');
    const keyText = fs.readFileSync(key, 'utf8');
    h.check(/Original,Replacement/.test(keyText) && /Zelda Quimby,Participant 3/.test(keyText), 'the key file holds the words and what replaced them');
    const doc = await rpc('source.get', { id: source.id });
    h.check(doc.text.startsWith('Participant 3 told me the bus was late.') && !/Zelda|Quimby/.test(doc.text), 'the text of the document has no name');
    const renamed = (await rpc('source.list')).find((s) => s.id === source.id);
    h.equal(renamed.name, 'Interview with Participant 3', 'the name of the document has none either');
    h.equal((await state()).rules.length, 0, 'the list is gone from the project');
    const coded = (await rpc('segments.list', {})).segments.filter((s) => s.source_id === source.id);
    h.check(coded.some((s) => s.text === 'the bus was late'), 'the coding stayed on its words');
    h.check(/Names were last replaced for good on/.test(await text(main())), 'the page remembers that it was done');
    await page.getByRole('button', { name: 'Back to the coding workspace' }).first().click();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /Interview with Participant 3/ }).first().click();
    await page.getByText(/Participant 3 told me/).first().waitFor();
    h.check(true, 'the reader shows the new words');
    await goToPage();
    await axe('the pseudonymize and redact page after names were replaced for good');
  });

  await S('the key brings the list back', async () => {
    const key = path.join(workDir, 'privacy-key.csv');
    await page.getByTestId('privacy-key-path').fill(key);
    await page.getByTestId('privacy-load-key').click();
    await page.getByTestId('privacy-table').waitFor();
    h.equal((await state()).rules.length, 5, 'all five are back');
    // Leave the check's project as it was found: no list in it.
    const ids = (await state()).rules.map((r) => r.id);
    await rpc('deid.remove', { ids });
    h.equal((await state()).rules.length, 0, 'and taken out again');
  });

  // The offline lock and the ethics summary (decision 165).
  await S('the offline lock stops a prompt packet and shows at the bottom of the window, and the ethics summary is a report of one section', async () => {
    await goToPage();
    h.equal(await page.getByTestId('privacy-lock').count(), 1, 'the page has the offline lock');
    h.equal(await page.getByTestId('privacy-ethics').count(), 1, 'and the ethics summary');
    await axe('the Privacy page with the offline lock and the ethics summary');
    await page.getByTestId('privacy-lock-switch').click();
    await page.getByText('The offline lock is on.', { exact: true }).waitFor();
    h.equal((await rpc('project.info')).offline_lock, true, 'the project says the lock is on');
    await page.getByText('Offline lock on', { exact: true }).waitFor();
    h.check(true, 'and the bottom of the window says so');
    let message = '';
    try {
      await rpc('ai.packet', {});
    } catch (e) {
      message = String(e?.message ?? e);
    }
    h.check(/offline lock is on/.test(message) && /no prompt packet/.test(message), `a prompt packet is refused in words: ${message.slice(0, 160)}`);

    await page.getByTestId('privacy-ethics-make').click();
    const d = page.getByRole('dialog', { name: 'Export', exact: true });
    await d.waitFor();
    const sections = d.getByTestId('report-sections');
    h.equal(await sections.locator('input:checked').count(), 1, 'the ethics summary opens the report with one section chosen');
    h.equal(await sections.getByTestId('report-section-ethics').locator('input').isChecked(), true, 'the ethics section');
    const file = path.join(workDir, 'ethics-summary.docx');
    await d.getByRole('textbox', { name: 'Where to save it' }).fill(file);
    await d.getByRole('button', { name: 'Save the file', exact: true }).click();
    await d.getByText(/Saved|saved/).first().waitFor();
    h.check(fs.existsSync(file) && fs.statSync(file).size > 3000, 'the Word file is made');
    const docx = fs.readFileSync(file);
    h.check(docx.includes(Buffer.from('PK')), 'and is a Word package');
    await d.getByRole('button', { name: 'Close', exact: true }).click();
    await d.waitFor({ state: 'hidden' });

    // and the lock comes off again
    await page.getByTestId('privacy-lock-switch').click();
    for (let i = 0; i < 50 && (await rpc('project.info')).offline_lock; i++) await page.waitForTimeout(100);
    h.equal((await rpc('project.info')).offline_lock, false, 'the lock can be taken off');
  });
}
