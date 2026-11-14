// Merging a copy of the project: the Merge tab of the Team page checks a copy another coder worked in and says what a merge would do
// without changing anything, merges it after saving a backup, says it brings nothing in the second time, and Undo will not reach past it.
// The copy is a second project the check makes and fills itself, so nothing depends on what earlier steps left in this one.

import fs from 'node:fs';
import path from 'node:path';

const NOTES = 'The staff were kind.\n\nOne driver helped me.\n\nNobody greeted me.\n';

export async function runMerge(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe, mainTab } = api;
  const copyPath = path.join(workDir, 'ben-copy.pqda');
  let ourPath = '';
  let sourcesBefore = 0;
  let backup = '';

  const openMerge = async () => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Team', exact: true }).click();
      await page.getByRole('button', { name: 'Team page', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();
    }
    await page.getByTestId('team-tab-merge').click();
    await page.getByTestId('merge').waitFor();
  };

  await S('another coder\'s copy is made, with a coder, a code, a document, codings, and a memo that this project lacks', async () => {
    ourPath = (await rpc('project.info')).path;
    sourcesBefore = (await rpc('source.list')).length;
    // something the two will differ on: a value of a case, and the wording of a code
    const sharedCase = await rpc('case.add', { name: 'Merge shared' });
    await rpc('variable.define', { name: 'Merge shared var', level: 'ratio' });
    await rpc('variable.set_values', { variable: 'Merge shared var', changes: [{ case_id: sharedCase.id, value: '10' }] });
    await rpc('code.add', { name: 'Merge wording', definition: 'The wording here.' });
    await rpc('project.create', { path: copyPath, name: 'Ben copy' });
    const ben = await rpc('coder.add', { name: 'Ben', role: 'human', local_id: 'ben-merge-check' });
    const code = await rpc('code.add', { name: 'Merge kindness', definition: 'People being kind.' });
    fs.writeFileSync(path.join(workDir, 'merge-notes.txt'), NOTES, 'utf8');
    const done = await rpc('import.run', { path: path.join(workDir, 'merge-notes.txt') });
    const source = done.outcome.source_ids[0];
    for (const u of await rpc('unit.list', { source_id: source })) {
      await rpc('coding.apply', { source_id: source, start: u.start, end: u.end, code_id: code.id, coder_id: ben.id, unit_id: u.id });
    }
    await rpc('memo.add', { kind: 'analytic', scope: 'project', title: 'Ben\'s note', body: 'Kindness keeps coming up.' });
    // a case that holds the document, with a variable of its own
    const mergeCase = await rpc('case.add', { name: 'Merge respondent', notes: 'Asked in the second wave' });
    await rpc('case.assign', { case_id: mergeCase.id, source_id: source });
    await rpc('variable.define', { name: 'Merge wave', level: 'ratio' });
    await rpc('variable.set_values', { variable: 'Merge wave', changes: [{ case_id: mergeCase.id, value: '2' }] });
    const theirShared = await rpc('case.add', { name: 'Merge shared' });
    await rpc('variable.define', { name: 'Merge shared var', level: 'ratio' });
    await rpc('variable.set_values', { variable: 'Merge shared var', changes: [{ case_id: theirShared.id, value: '20' }] });
    await rpc('code.add', { name: 'Merge wording', definition: 'The wording in the copy.' });
    h.equal((await rpc('source.list')).length, 1, 'the copy has the one document');
    await rpc('project.close', {});
    await rpc('project.open', { path: ourPath, backup: false });
    await page.reload();
    h.equal((await rpc('source.list')).length, sourcesBefore, 'this project is open again and has what it had');
  });

  await S('the Merge tab checks the copy and says what a merge would do, and changes nothing', async () => {
    await openMerge();
    await page.getByTestId('merge-path').fill(copyPath);
    await page.getByTestId('merge-check').click();
    await page.getByTestId('merge-result').waitFor();
    const sentences = await page.getByTestId('merge-sentences').innerText();
    h.check(/Documents added: merge-notes\./.test(sentences), `the document is named: ${sentences.replace(/\s+/g, ' ').slice(0, 200)}`);
    h.check(/Coders added: .*Ben/.test(sentences), 'the coder is named');
    h.check(/Codes added: Merge kindness\./.test(sentences), 'the code is named');
    h.check(/Codings added: 3\./.test(sentences), 'three codings would come in');
    h.check(/Memos added: 1\./.test(sentences), 'and a memo');
    h.check(/Cases added: Merge respondent\./.test(sentences), 'the case is named');
    h.check(/1 document was put in a case here/.test(sentences), 'with the document it holds');
    h.check(/Variables of cases added: Merge wave\./.test(sentences), 'the variable of cases is named');
    h.check(/1 value of a case was added\./.test(sentences), 'and its value');
    h.equal(await page.getByTestId('merge-value-conflicts').locator('tbody tr').count(), 1, 'the value that differs is listed, with the one here and the one in the copy');
    h.check(/Merge shared/.test(await page.getByTestId('merge-value-conflicts').innerText()) && /10/.test(await page.getByTestId('merge-value-conflicts').innerText()) && /20/.test(await page.getByTestId('merge-value-conflicts').innerText()), 'with both values');
    h.check(/Merge wording/.test(await page.getByTestId('merge-conflicts').innerText()), 'and the wording that differs');
    h.check(/Nothing is taken from the copy/.test(await page.getByTestId('merge-chosen').innerText()), 'nothing is taken from the copy until it is chosen');
    h.check(/What a merge would do/.test(await page.getByTestId('merge-result').innerText()), 'it says it is only a check');
    h.equal((await rpc('source.list')).length, sourcesBefore, 'and nothing was added');
    h.equal((await rpc('coder.list')).some((c) => c.name === 'Ben'), false, 'nor any coder');
    h.equal(await page.getByTestId('merge-now').isEnabled(), true, 'merging is offered');
    await axe('the Merge tab with a preview');
  });

  await S('merging brings the work in with its coder, after a backup, and says so', async () => {
    // the copy's value and the copy's wording are chosen; everything that is not chosen keeps what is here
    await page.getByTestId('merge-take-value-0').click();
    await page.getByTestId('merge-take-def-0').click();
    h.check(/2 differences will take the copy's/.test(await page.getByTestId('merge-chosen').innerText()), 'the page says how many differences will take the copy\'s');
    await axe('the Merge tab with differences to settle');
    await page.getByTestId('merge-now').click();
    await page.getByTestId('merge-backup').waitFor();
    const said = await page.getByTestId('merge-backup').innerText();
    backup = said.replace(/^A backup of this project was saved first: /, '').trim();
    h.check(fs.existsSync(backup), `the backup is a real file: ${backup}`);
    h.check(/What the merge did/.test(await page.getByTestId('merge-result').innerText()), 'it now says what it did');
    const sources = await rpc('source.list');
    h.equal(sources.length, sourcesBefore + 1, 'the document came in');
    const notes = sources.find((s) => s.name === 'merge-notes');
    h.equal([notes.units, notes.codings], [3, 3], 'with its units and its codings');
    const ben = (await rpc('coder.list')).find((c) => c.name === 'Ben');
    h.check(!!ben, 'Ben is a coder here now');
    h.equal((await rpc('coding.list', { source_id: notes.id })).every((c) => c.coder_id === ben.id), true, 'and the codings are his');
    h.check((await rpc('code.list')).some((c) => c.name === 'Merge kindness' && c.definition === 'People being kind.'), 'the code came with its definition');
    h.check((await rpc('memo.list', {})).some((m) => m.title === 'Ben\'s note'), 'and the memo');
    const data = await rpc('data.get');
    const sharedRow = data.cases.findIndex((c) => c.name === 'Merge shared');
    const sharedCol = data.variables.findIndex((v) => v.name === 'Merge shared var');
    h.equal(data.values[sharedRow][sharedCol], '20', 'the value the copy had took the place of the one here, as chosen');
    h.equal((await rpc('code.list')).find((c) => c.name === 'Merge wording').definition, 'The wording in the copy.', 'and so did the wording');
    h.check(/changed to the one in the copy, as you chose/.test(await page.getByTestId('merge-sentences').innerText()), 'the page says so');
    const theirCase = data.cases.find((c) => c.name === 'Merge respondent');
    h.check(!!theirCase, 'the case came in');
    const col = data.variables.findIndex((v) => v.name === 'Merge wave');
    h.check(col >= 0 && data.values[data.cases.indexOf(theirCase)][col] === '2', 'with the variable and its value');
    h.equal((await rpc('case.list', {})).find((c) => c.name === 'Merge respondent').source_ids, [notes.id], 'and the document it holds');
    await axe('the Merge tab after a merge');
  });

  await S('merging the same copy again brings nothing in, and Undo does not reach back past a merge', async () => {
    await page.getByTestId('merge-check').click();
    await page.getByText('There is nothing to merge.').waitFor();
    h.equal(await page.getByTestId('merge-now').isDisabled(), true, 'the button to merge is off');
    h.check(/Codings added: 0\. Already here: 3\./.test(await page.getByTestId('merge-sentences').innerText()), 'the three codings are already here');
    let message = '';
    try {
      await rpc('undo');
    } catch (e) {
      message = String(e?.message ?? e);
    }
    h.check(/too large to undo/.test(message), `Undo says a merge is too large: ${message.slice(0, 160)}`);
    // A copy that is this very project, and a file that is not there, are refused in words.
    await page.getByTestId('merge-path').fill(ourPath);
    await page.getByTestId('merge-check').click();
    await page.getByRole('alert').filter({ hasText: 'this project itself' }).waitFor();
    await page.getByTestId('merge-path').fill(path.join(workDir, 'no-such-copy.pqda'));
    await page.getByTestId('merge-check').click();
    await page.getByRole('alert').filter({ hasText: 'not a file' }).waitFor();
  });

  // Coder packets (decision 161): a small project for another coder, opened as that coder, and merged back.
  const packetPath = path.join(workDir, 'packet-for-pat.pqda');
  let packetCoder = null;

  await S('the Packets tab makes a packet for another coder from the chosen documents, with the instructions, and says what it leaves out', async () => {
    await openMerge();
    await page.getByTestId('team-tab-packets').click();
    await page.getByTestId('packets').waitFor();
    await page.getByTestId('packet-make').click();
    await page.getByRole('alert').filter({ hasText: 'Say who the packet is for' }).waitFor();
    await page.getByTestId('packet-coder').fill('Packet Pat');
    await page.getByTestId('packet-some').click();
    await page.getByTestId('packet-docs').waitFor();
    await page.getByTestId('packet-make').click();
    await page.getByRole('alert').filter({ hasText: 'Choose at least one document' }).waitFor();
    const docs = await rpc('source.list');
    const mine = docs.find((d) => d.name === 'merge-notes');
    await page.getByTestId(`packet-doc-${mine.id}`).click();
    await page.getByTestId('packet-instructions').fill('Code every kind act.');
    await page.getByTestId('packet-path').fill(packetPath);
    await axe('the Packets tab, filled in');
    await page.getByTestId('packet-make').click();
    await page.getByTestId('packet-result').waitFor();
    const said = (await page.getByTestId('packet-sentences').innerText()).replace(/\s+/g, ' ');
    h.check(/1 document: merge-notes/.test(said), `the one document is named: ${said.slice(0, 200)}`);
    h.check(/codes in the codebook|code in the codebook/.test(said), 'the codes are counted');
    h.check(/new to this project/.test(said), 'the coder is new');
    const left = (await page.getByTestId('packet-left-out').innerText()).replace(/\s+/g, ' ');
    h.check(/blind/.test(left) && /recordings/.test(left), `what it leaves out is said: ${left.slice(0, 200)}`);
    h.check(fs.existsSync(packetPath), 'the packet file is there');
    // a second packet to the same file is refused in words
    await page.getByTestId('packet-make').click();
    await page.getByRole('alert').filter({ hasText: 'already exists' }).waitFor();
    await axe('the Packets tab after making a packet');
  });

  await S('opened, the packet says whom it is for, the person using it is that coder, and what they code merges back to the same coder', async () => {
    const ourPath = (await rpc('project.info')).path;
    await rpc('project.close', {});
    await rpc('project.open', { path: packetPath, backup: false });
    await page.reload();
    await page.getByText(/This project is a packet made for Packet Pat/).waitFor();
    const info = await rpc('project.info');
    h.equal(info.packet.coder_name, 'Packet Pat', 'the project says it is a packet');
    const coders = await rpc('coder.list');
    h.equal(coders.map((c) => c.name), ['Packet Pat'], 'its one coder is the person it was made for');
    h.equal(info.me, coders[0].id, 'and the person using it is that coder');
    packetCoder = coders[0];
    const memos = await rpc('memo.list', {});
    h.check(memos.some((m) => m.title === 'Instructions for Packet Pat' && /Code every kind act/.test(m.body)), 'the instructions are a memo');
    h.equal(memos.some((m) => m.title === 'Ben\'s note'), false, 'the study\'s memos are not there');
    const sources = await rpc('source.list');
    h.equal(sources.map((s) => s.name), ['merge-notes'], 'only the chosen document');
    const kind = (await rpc('code.list')).find((c) => c.name === 'Merge kindness');
    const units = await rpc('unit.list', { source_id: sources[0].id });
    await rpc('coding.apply', { source_id: sources[0].id, start: units[0].start, end: units[0].end, code_id: kind.id, unit_id: units[0].id });
    await rpc('project.close', {});
    await rpc('project.open', { path: ourPath, backup: false });
    await page.reload();
    await openMerge();
    await page.getByTestId('merge-path').fill(packetPath);
    await page.getByTestId('merge-check').click();
    await page.getByTestId('merge-result').waitFor();
    const sentences = await page.getByTestId('merge-sentences').innerText();
    h.check(/Coders added: .*Packet Pat/.test(sentences), `the coder would be added: ${sentences.replace(/\s+/g, ' ').slice(0, 220)}`);
    h.check(/Codings added: 1\./.test(sentences), 'one coding would come in');
    await page.getByTestId('merge-now').click();
    await page.getByTestId('merge-backup').waitFor();
    const pat = (await rpc('coder.list')).find((c) => c.name === 'Packet Pat');
    h.check(!!pat && pat.local_id === packetCoder.local_id, 'the coder here has the packet coder\'s id');
  });

  // The working set (decision 163): documents, cases, and codes chosen once, followed by every page that can.
  await S('the working set narrows what the pages ask for, is shown under the ribbon on every page, and shows everything again', async () => {
    if ((await rpc('source.list')).length < 2) {
      fs.writeFileSync(path.join(workDir, 'ws-extra.txt'), 'A second document, so there is something to leave out of the working set.\n', 'utf8');
      await rpc('import.run', { path: path.join(workDir, 'ws-extra.txt') });
      await page.reload();
    }
    const sources = await rpc('source.list');
    h.check(sources.length >= 2, `the project has documents to choose between: ${sources.length}`);
    await mainTab('Code').click();
    await page.getByRole('tab', { name: 'View', exact: true }).click();
    await page.locator('[data-command="working-set"]').click();
    await page.getByTestId('ws-documents').waitFor();
    await axe('the working set dialog');
    await page.getByTestId('ws-documents-all').click();
    await page.getByTestId('ws-documents-untick-shown').click();
    await page.getByTestId('ws-documents-item').first().click();
    // Retrieved Segments is on the screen, so it asks the engine again the moment the set is applied, and only about that document.
    const asked = page.waitForRequest((r) => r.url().endsWith('/rpc') && r.method() === 'POST' && /"cmd":"segments.list"/.test(r.postData() ?? '') && /"source_ids"/.test(r.postData() ?? ''));
    await page.getByTestId('ws-apply').click();
    const body = JSON.parse((await asked).postData());
    h.equal(body.args.source_ids, [sources[0].id], 'Retrieved Segments asked about the one document');
    await page.getByTestId('working-set-bar').waitFor();
    const parts = await page.getByTestId('working-set-parts').innerText();
    h.check(new RegExp(`1 of ${sources.length} documents`).test(parts), `the bar says what is narrowed: ${parts}`);
    h.check(/Data table, the Team page, the Codebook Editor, exports, and reports do not/.test(await page.getByTestId('working-set-bar').innerText()), 'and which pages do not follow it');
    await axe('a page with the working set bar');

    // the bar is on other pages too, and Change opens the dialog again with the choice kept
    await page.getByRole('tab', { name: 'Team', exact: true }).click();
    await page.getByRole('button', { name: 'Team page', exact: true }).click();
    await page.getByTestId('working-set-bar').waitFor();
    await page.getByTestId('working-set-change').click();
    await page.getByTestId('ws-documents').waitFor();
    h.equal(await page.getByTestId('ws-documents-all').getAttribute('data-selected'), null, 'the dialog opens with one document chosen, not every document');
    await page.getByRole('button', { name: 'Cancel', exact: true }).click();

    // everything again
    await page.getByTestId('working-set-reset').click();
    await page.getByTestId('working-set-bar').waitFor({ state: 'detached' });
    h.equal(await page.getByTestId('working-set-bar').count(), 0, 'the bar is gone');
    await mainTab('Code').click();
  });
}
