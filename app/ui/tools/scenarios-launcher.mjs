// The welcome page after a project has been used: a sample project copy, the recent list, and a project that moved.

import fs from 'node:fs';
import path from 'node:path';

import { runAutoCode } from './scenarios-autocode.mjs';
import { runExplore } from './scenarios-explore.mjs';
import { runExplore2 } from './scenarios-explore2.mjs';
import { runMap } from './scenarios-map.mjs';
import { runAi } from './scenarios-ai.mjs';
import { runSplit } from './scenarios-split.mjs';
import { runMedia } from './scenarios-media.mjs';
import { runImages } from './scenarios-image.mjs';
import { runPdf } from './scenarios-pdf.mjs';
import { runClips } from './scenarios-clips.mjs';
import { runPrivacy } from './scenarios-privacy.mjs';
import { runEncryption } from './scenarios-encryption.mjs';
import { runDataTools } from './scenarios-datatools.mjs';
import { runTeam } from './scenarios-team.mjs';
import { runMerge } from './scenarios-merge.mjs';
import { runClass } from './scenarios-class.mjs';
import { runThemes } from './scenarios-themes.mjs';
import { runText } from './scenarios-text.mjs';
import { runStats } from './scenarios-stats.mjs';
import { runScope } from './scenarios-scope.mjs';
import { runEvaluation } from './scenarios-evaluation.mjs';
import { runDataEditor } from './scenarios-dataeditor.mjs';
import { runMixed } from './scenarios-mixed.mjs';
import { runLanguage } from './scenarios-language.mjs';

export async function runLauncher(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe, mainTab } = api;

  const copyName = 'Mobility copy';
  const copyPath = path.join(workDir, `${copyName}.pqda`);
  const movedPath = path.join(workDir, 'Mobility moved.pqda');

  await S('Sample Projects makes an editable copy of the Urban Mobility Study', async () => {
    await api.closeProjectViaMenu();
    await page.getByRole('button', { name: 'Sample Projects', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Sample Projects' });
    await dialog.waitFor();
    await axe('the Sample Projects window');
    const text = (await dialog.textContent()) ?? '';
    h.check(/Urban Mobility Study/.test(text), 'the study is listed');
    h.check(/7 documents, 15 codes, 10 codings/.test(text), `what it holds is told before it is copied: ${text.match(/\d+ documents?, \d+ codes?, \d+ codings?/)?.[0]}`);
    // An empty or unusable name is refused at the field, and nothing is made.
    await dialog.getByRole('textbox', { name: 'Name of your copy' }).fill('');
    await dialog.getByRole('textbox', { name: 'Save the copy in' }).fill(workDir);
    await dialog.getByRole('button', { name: 'Make an editable copy' }).click();
    await dialog.getByText('Give the project a name.').waitFor();
    await dialog.getByRole('textbox', { name: 'Name of your copy' }).fill(copyName);
    const where = await dialog.getByText(/Will be saved as/).textContent();
    h.check(where.includes(`${copyName}.pqda`) && where.includes(workDir), `the final file name and folder are shown first: ${where}`);
    await dialog.getByRole('button', { name: 'Make an editable copy' }).click();
    await mainTab('Coding').waitFor();
    const info = await rpc('project.info');
    h.equal(info.name, 'Urban Mobility Study', 'the copy is open, named as the study is (the name chosen is the file name)');
    h.equal(info.sample_id, 'urban-mobility', 'the engine knows which sample it came from');
    h.check(fs.existsSync(copyPath), 'the copy is a file in the folder chosen');
    const tag = page.getByTestId('project-name');
    h.check(/Sample/.test(await tag.textContent()), 'the title bar tags it as a sample');
    h.equal((await rpc('source.list')).length, 7, 'seven documents');
    h.equal((await rpc('code.list')).length, 15, 'fifteen codes');
    h.equal((await rpc('segments.list', { limit: 1 })).total, 10, 'ten codings');
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row').nth(1).waitFor();
    await axe('the Urban Mobility sample');
  });

  await S('the sample opens with its codings listed, and its rated coding shows a rating, a memo, and a history', async () => {
    await page.waitForFunction(() => /\(10\)/.test(document.querySelector('[data-testid=results-count]')?.textContent ?? ''), null, { timeout: 8000 });
    h.check(true, 'Retrieved Segments lists all ten codings');
    // The coding the sample rates 4 of 5 (the book's reference case).
    let found = null;
    for (const s of await rpc('source.list')) {
      const c = (await rpc('coding.list', { source_id: s.id })).find((x) => x.rating === 4);
      if (c) found = { source: s, coding: c };
    }
    h.check(found !== null, 'one coding carries a rating of 4');
    const seg = (await rpc('segments.list', { limit: 50 })).segments.find((x) => x.coding_id === found.coding.id);
    const snippet = seg.text.slice(0, 24);
    await page.locator('[aria-label="Retrieved Segments"] [role=row]', { hasText: snippet }).first().locator('.bk-col-source').click();
    const detail = page.getByTestId('coding-detail');
    await detail.waitFor();
    h.equal((await page.getByTestId('rating-value').textContent()).trim(), '(4 / 5)', 'Segment Details shows the rating as stars and as a number');
    h.equal((await detail.getByTestId('rating-label').innerText()).trim(), 'Intensity', 'the code says what its rating measures');
    h.check(/1 = Mentioned in passing\s+·\s+5 = Central to the account/.test(await detail.getByTestId('rating-scale').innerText()), 'and what the two ends of the scale mean');
    h.check(/Last modified/.test(await page.getByTestId('provenance').textContent()), 'Provenance says when it was last changed');
    h.check(/Service reliability and daily stress/.test(await detail.textContent()), 'the memo about this coding is listed');
    h.check(/Coded by/.test(await detail.textContent()), 'and who coded it');
    await page.screenshot({ path: path.join(ctx.shots, 'sample-segment-details.png') });
    await axe('Segment Details with a rated coding');
    // The rating can be changed and cleared; not rated is its own state.
    await detail.getByRole('radio', { name: '5 stars out of 5' }).click();
    await page.waitForFunction(() => /\(5 \/ 5\)/.test(document.querySelector('[data-testid=rating-value]')?.textContent ?? ''), null, { timeout: 5000 });
    await detail.getByRole('button', { name: 'Clear the rating' }).click();
    await page.waitForFunction(() => /Not rated/.test(document.querySelector('[data-testid=rating-value]')?.textContent ?? ''), null, { timeout: 5000 });
    h.equal((await rpc('coding.list', { source_id: found.source.id })).find((x) => x.id === found.coding.id).rating, null, 'the engine has it as not rated, not zero');
    await detail.getByRole('radio', { name: '4 stars out of 5' }).click();
    await page.waitForFunction(() => /\(4 \/ 5\)/.test(document.querySelector('[data-testid=rating-value]')?.textContent ?? ''), null, { timeout: 5000 });
  });

  await S('the sample shows its rated code and its counterexample in the Codebook Editor, and a counterexample can be taken away and chosen again', async () => {
    const reliability = (await rpc('code.list')).find((c) => c.name === 'Reliability');
    await mainTab('Coding').click();
    await page.getByRole('tab', { name: 'Code', exact: true }).click();
    await page.getByRole('button', { name: 'Codebook Editor', exact: true }).click();
    await api.editorHeading().waitFor();
    await api.codeRow('Reliability').click();
    const form = page.getByTestId('code-detail');
    h.equal(await form.getByRole('checkbox', { name: /^Weighted \/ Rated/ }).isChecked(), true, 'Reliability is a rated code');
    h.equal(await form.getByRole('textbox', { name: 'What the rating measures' }).inputValue(), 'Intensity', 'with its scale described');
    await page.getByRole('tablist', { name: /: details$/ }).getByRole('tab', { name: /^Examples/ }).click();
    const panel = page.getByTestId('examples-tab');
    await panel.getByTestId('counter-heading').waitFor();
    h.check(/Counterexamples \(1\)/.test(await panel.getByTestId('counter-heading').innerText()), 'the Examples tab counts one counterexample');
    const listed = await panel.getByTestId('counter-list').innerText();
    h.check(/monthly pass/.test(listed) && /coded Cost/.test(listed), `it shows the words and the code they are coded with: ${listed.split('\n')[0]}`);
    await axe('the Examples tab with a counterexample');
    await panel.getByRole('button', { name: /^Stop using this passage as a counterexample/ }).click();
    await panel.getByText('No passage is marked as a counterexample of this code yet.').waitFor();
    h.equal((await rpc('code.counterexamples', { id: reliability.id })).length, 0, 'taking it away is in the engine');
    await panel.getByRole('button', { name: 'Add a counterexample' }).click();
    const dialog = page.getByRole('dialog', { name: /Add a counterexample of Reliability/ });
    await dialog.waitFor();
    await dialog.getByRole('searchbox', { name: 'Find a passage' }).fill('monthly pass');
    await axe('the counterexample picker');
    h.equal(await dialog.getByTestId('counter-picker').getByRole('listitem').count(), 1, 'the search narrows the passages to the one asked for');
    await dialog.getByRole('button', { name: /^Use this passage as a counterexample: Cost is also a factor/ }).click();
    await dialog.waitFor({ state: 'hidden' });
    await panel.getByTestId('counter-list').waitFor();
    h.equal((await rpc('code.counterexamples', { id: reliability.id })).map((s) => s.code_name), ['Cost'], 'choosing it again makes it a counterexample once more');
    await api.closeDefinition();
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
  });

  await S('a case can be made by hand and given a document, and Retrieved Segments can show one case', async () => {
    const cases = await rpc('case.list');
    h.equal(cases.length, 2, 'the sample has two cases');
    const first = cases[0];
    await page.getByRole('button', { name: /^Cases: / }).click();
    await page.getByRole('menuitemradio', { name: first.name, exact: true }).click();
    const members = new Set(first.source_ids);
    const wanted = (await rpc('segments.list', { limit: 50 })).segments.filter((s) => members.has(s.source_id)).length;
    await page.waitForFunction((n) => document.querySelectorAll('[aria-label="Retrieved Segments"] [role=row]').length - 1 === n, wanted, { timeout: 5000 });
    h.check(true, `the table shows the ${wanted} codings of the case ${first.name}`);
    await page.getByRole('button', { name: /^Cases: / }).click();
    await page.getByRole('menuitemradio', { name: 'All cases', exact: true }).click();
  });

  // The Explore tools, on the sample's ten codings.
  await runExplore(ctx, api);

  // Auto Code, on documents of its own.
  await runAutoCode(ctx, api);

  // Explore again: crosstabs, percentages, dates and ranges in a query, and saved, frozen, and compared queries.
  await runExplore2(ctx, api);

  // The Concept Map.
  await runMap(ctx, api);

  // The AI Assistant, by prompt packet.
  await runAi(ctx, api);

  // Team: agreement between coders.
  await runTeam(ctx, api);

  // Split view: a second document beside the one being read.
  await runSplit(ctx, api);

  // Recordings: a player above the transcript, which follows it.
  await runMedia(ctx, api);

  // Pictures: areas drawn with the pointer and the keys, coded like words.
  await runImages(ctx, api);

  // PDF pages: a PDF kept with its file, its pages drawn, areas of them coded.
  await runPdf(ctx, api);

  // Coding a stretch of a recording: a recording without captions as a document of its own, marked, coded, moved, and listed.
  await runClips(ctx, api);

  // Pseudonymize and redact: a list of names, applied to what is exported and then to the project for good.
  await runPrivacy(ctx, api);

  // Encrypted projects: a passphrase put on the project, asked for when it is opened, changed, and taken off again.
  await runEncryption(ctx, api);
  await runLanguage(ctx, api);

  // Merging a copy of the project, which another coder worked in.
  await runMerge(ctx, api);

  // A class exercise: a packet for every student, their work brought back in one step, and the class reviewed against the instructor.
  await runClass(ctx, api);

  // Developing themes: the Themes page, a theme made and filled, its stage, and a thematic map.
  await runThemes(ctx, api);

  // Text Analytics: word frequencies and keywords in context.
  await runText(ctx, api);

  // What the Statistics page offers: it describes to begin with, the tests can be turned on, and the case table for PrismStatistics is made from Export.
  await runScope(ctx, api);

  // Statistics: the variables of cases, the test chooser, and the output notebook.
  await runStats(ctx, api);

  // The data editor: variables of cases, their values, and the transformations.
  await runDataEditor(ctx, api);

  // The data editor's later tools: date parts, samples, group summaries, sorting, tables, merging, weights, and splitting.
  await runDataTools(ctx, api);

  // Mixed Methods: codes into variables of cases, and the values of a variable into codes.
  await runMixed(ctx, api);

  // What comes in from the other programs of the family: an evaluation kept in PrismStatistics, and the Reference Manager's exchange file.
  await runEvaluation(ctx, api);

  await S('a project that was moved is told so, and Locate finds it again', async () => {
    await api.closeProjectViaMenu();
    // The file moves while the program is closed to it: the list must not pretend it is still there.
    fs.renameSync(copyPath, movedPath);
    await page.reload();
    const row = page.getByRole('button', { name: new RegExp(`^Open ${copyName}\\.pqda`) });
    await row.waitFor();
    await page.waitForFunction(() => /not found/i.test(document.querySelector('.bk-recent-row')?.textContent ?? ''), null, { timeout: 8000 });
    h.check(true, 'the row says the file is not at its saved place');
    await axe('the welcome page with a project that moved');
    await row.click();
    await page.getByText(/was not found at/).waitFor();
    h.check(true, 'opening it explains what is wrong and points to the menu');
    await page.getByRole('button', { name: 'More options for Urban Mobility Study' }).click();
    await page.getByRole('menuitem', { name: 'Locate…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Find Urban Mobility Study' });
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'Project file' }).fill(movedPath);
    await dialog.getByRole('button', { name: 'Find and open' }).click();
    await mainTab('Coding').waitFor();
    h.equal((await rpc('project.info')).name, 'Urban Mobility Study', 'the same project opened from its new place');
    h.equal((await rpc('project.info')).sample_id, 'urban-mobility', 'and it is still known as a sample copy');
    await api.closeProjectViaMenu();
    const list = await page.getByRole('region', { name: 'Recent Projects' }).textContent();
    h.check(/Mobility moved\.pqda/.test(list ?? ''), 'the list now shows the new place');
    h.check(!/Mobility copy\.pqda/.test(list ?? ''), 'and no longer shows the old one');
    // Remove from the list leaves the file alone.
    await page.getByRole('button', { name: new RegExp('^More options for ') }).last().click();
    await page.getByRole('menuitem', { name: 'Remove from recent list' }).click();
    h.check(fs.existsSync(movedPath), 'removing a project from the list does not delete the file');
  });
}
