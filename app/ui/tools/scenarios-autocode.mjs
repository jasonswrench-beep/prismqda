// Auto Code: finding passages by words, by speaker, and by heading; previewing without changing anything; the review queue
// (accept, reject, adjust, accept a whole run, one undo for a whole decision); and what each step leaves in the engine. The
// documents are the check's own, so nothing here depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';

const WORDS = 'The cost was high. We stayed anyway.\n\nAccess was easy and the staff were warm. No problem at all.\n\nThe cost of access is a worry. It keeps coming up.\n';
const TALK = 'Interviewer: How do you get to work?\nPriya: I take the train most days.\nINTERVIEWER: And the cost?\npriya: It costs a lot.\nSam: I walk.\n';
const REPORT = '# Background\n\nPeople ride the bus.\n\n# Findings\n\nThe cost is high.\n\n## Access\n\nStops are far apart.\n\n# Next steps\n\nAsk again.\n';

export async function runAutoCode(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe, waitStatus } = api;

  const sourceNamed = async (name) => (await rpc('source.list')).find((s) => s.name === name);
  const codeNamed = async (name) => (await rpc('code.list')).find((c) => c.name === name);
  const textOf = async (sourceId, start, end) => Array.from((await rpc('source.get', { id: sourceId })).text).slice(start, end).join('');
  const queueTab = () => page.getByRole('tablist', { name: 'Auto Code' }).getByRole('tab', { name: /^Review queue/ });
  const setupTab = () => page.getByRole('tablist', { name: 'Auto Code' }).getByRole('tab', { name: 'Set up' });
  const rows = () => page.getByTestId('ac-row');
  const radio = (label) => page.locator('label').filter({ hasText: label });

  /** Opens the page from the ribbon, on a half. */
  async function open(half) {
    if (!(await page.getByRole('heading', { level: 2, name: 'Auto Code' }).isVisible())) {
      await page.getByRole('tab', { name: 'Home', exact: true }).click();
      await page.getByRole('button', { name: 'Auto Code', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Auto Code' }).waitFor();
    }
    await (half === 'queue' ? queueTab() : setupTab()).click();
  }

  /** Picks the documents the form looks in. */
  async function lookIn(...names) {
    await radio('Documents I choose').click();
    await page.getByRole('button', { name: /^Documents to look in:/ }).click();
    const dialog = page.getByRole('dialog', { name: 'Documents to look in' });
    await dialog.waitFor();
    await dialog.getByRole('button', { name: 'None' }).click();
    for (const n of names) await dialog.locator('label').filter({ has: page.getByRole('checkbox', { name: n, exact: true }) }).click();
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
  }

  const facts = async () => (await page.getByTestId('ac-facts').locator('li').allTextContents()).map((t) => t.trim());
  const found = async () => {
    // What was found before is still on the page until the new answer replaces it, so wait for its "earlier version" mark to go.
    const stale = page.getByTestId('ac-stale');
    const wasStale = await stale.isVisible();
    await page.getByTestId('ac-find').click();
    if (wasStale) await stale.waitFor({ state: 'hidden' });
    await page.getByTestId('ac-facts').waitFor();
    return facts();
  };

  await S('Auto Code opens from the ribbon, says no AI is used, and asks for what is missing', async () => {
    for (const [file, text] of [['ac-words.txt', WORDS], ['ac-talk.txt', TALK], ['ac-report.md', REPORT]]) fs.writeFileSync(path.join(workDir, file), text, 'utf8');
    await rpc('import.run', { path: path.join(workDir, 'ac-words.txt') });
    await rpc('import.run', { path: path.join(workDir, 'ac-talk.txt'), source_type: 'transcript', interviewers: ['Interviewer'] });
    await rpc('import.run', { path: path.join(workDir, 'ac-report.md') });
    // The sample has Cost; the check makes the second code it needs, so it does not depend on the sample's wording.
    if (!(await codeNamed('Access'))) await rpc('code.add', { name: 'Access' });
    await page.reload();
    await open('setup');
    const tabs = (await page.getByRole('tablist', { name: 'Auto Code' }).getByRole('tab').allTextContents()).map((t) => t.trim());
    h.equal(tabs, ['Set up', 'Review queue'], 'the two halves; nothing waits yet');
    h.check(/No AI or language model is used/.test(await page.getByRole('main').innerText()), 'it says that no model is involved');
    h.check(/Suggestions that come back from a chatbot wait in this same queue/.test(await page.getByTestId('ac-ai-note').innerText()), 'and that suggestions from a chatbot wait in the same queue, with a way to the AI Assistant');
    await page.getByTestId('ac-find').click();
    const alert = await page.getByRole('main').getByRole('alert').first().innerText();
    h.check(/Type the words to find/.test(alert), `an empty form says what is missing: ${alert}`);
    await axe('Auto Code, set up before anything is typed');
  });

  await S('Find shows what a run would suggest and changes nothing', async () => {
    const cost = await codeNamed('Cost');
    await page.getByLabel('Words to find').fill('cost');
    await lookIn('ac-words');
    await page.getByLabel('Code to suggest', { exact: true }).selectOption(String(cost.id));
    const read = await page.getByTestId('ac-readback').innerText();
    h.check(/Find cost in ac-words, and suggest the code Cost for the whole sentence\./.test(read), `the run is read back in words: ${read.replace(/\s+/g, ' ').slice(0, 160)}`);
    const sentences = await found();
    h.equal(sentences, ['Found 2 places.', '2 suggestions in 1 document can be sent to the review queue.'], 'it says what it found');
    const text = await page.getByTestId('ac-found').innerText();
    h.check(/The cost was high\./.test(text) && /The cost of access is a worry\./.test(text), 'and shows the two sentences');
    h.equal((await rpc('proposal.list')).length, 0, 'nothing is in the queue');
    h.equal((await rpc('coding.list', { source_id: (await sourceNamed('ac-words')).id })).length, 0, 'and nothing is coded');
    await page.screenshot({ path: `${ctx.shots}/autocode-setup.png`, fullPage: true });
    await axe('Auto Code, set up with what a run finds');
  });

  await S('changing the form marks what was found as an earlier version and stops it being sent', async () => {
    await page.getByLabel('Words to find').fill('access');
    await page.getByTestId('ac-stale').waitFor();
    h.equal(await page.getByTestId('ac-send').isDisabled(), true, 'an earlier version cannot be sent');
    await page.getByLabel('Words to find').fill('cost');
    await page.getByTestId('ac-stale').waitFor({ state: 'hidden' });
    h.equal(await page.getByTestId('ac-send').isDisabled(), false, 'putting it back makes it current again');
  });

  await S('sending makes a run in the review queue, and the words are shown in their place', async () => {
    await page.getByTestId('ac-send').click();
    await page.getByTestId('autocode-queue').waitFor();
    h.equal(await queueTab().getAttribute('aria-selected'), 'true', 'the page moves to the review queue');
    h.check(/^Review queue \(2\)$/.test((await queueTab().innerText()).trim()), 'and the tab says two wait');
    await page.getByRole('heading', { level: 3, name: 'Words: cost' }).waitFor();
    await rows().first().waitFor();
    h.equal(await rows().count(), 2, 'two suggestions');
    const first = await rows().first().innerText();
    h.check(/The cost was high\./.test(first) && /We stayed anyway\./.test(first), `the first shows the sentence and the words after it: ${first.replace(/\s+/g, ' ').slice(0, 140)}`);
    h.check(/Cost/.test(first) && /ac-words/.test(first) && /autocoded from search: cost/.test(first), 'with its code, its document, and why it was suggested');
    const pending = await rpc('proposal.list', { status: 'pending' });
    h.equal(pending.length, 2, 'the engine has two waiting');
    h.equal((await rpc('coding.list', { source_id: (await sourceNamed('ac-words')).id })).length, 0, 'and still nothing is coded');
    await page.screenshot({ path: `${ctx.shots}/autocode-queue.png`, fullPage: true });
    await axe('Auto Code, the review queue');
  });

  await S('accepting one by keyboard codes it and moves to the next, and Undo takes it back', async () => {
    const words = await sourceNamed('ac-words');
    const accept = rows().first().getByTestId('ac-accept');
    await accept.focus();
    await page.keyboard.press('Enter');
    await page.getByTestId('ac-result').waitFor();
    h.check(/Accepted 1 suggestion; it is now coded\./.test(await page.getByTestId('ac-result').innerText()), 'the result is told');
    await page.waitForFunction(() => document.querySelectorAll('[data-ac-row]').length === 1);
    const codings = await rpc('coding.list', { source_id: words.id });
    h.equal(codings.length, 1, 'one coding is made');
    h.equal(await textOf(words.id, codings[0].start, codings[0].end), 'The cost was high.', 'of the first sentence');
    h.check((await page.evaluate(() => document.activeElement?.closest('[data-ac-row]') !== null)), 'the focus is on the next suggestion, not lost');
    h.check(/^Review queue \(1\)$/.test((await queueTab().innerText()).trim()), 'the tab count follows');
    await page.getByTestId('ac-undo').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-ac-row]').length === 2);
    h.equal((await rpc('coding.list', { source_id: words.id })).length, 0, 'the coding is gone');
    h.equal((await rpc('proposal.list', { status: 'pending' })).length, 2, 'and both wait again');
  });

  await S('rejecting one keeps it listed as rejected, and the filter and the counts say so', async () => {
    await rows().first().getByTestId('ac-reject').click();
    await page.getByTestId('ac-result').waitFor();
    h.check(/Rejected 1 suggestion\./.test(await page.getByTestId('ac-result').innerText()), 'the result is told');
    await page.waitForFunction(() => document.querySelectorAll('[data-ac-row]').length === 1);
    await radio('Rejected (1)').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-ac-row]').length === 1 && /Rejected/.test(document.querySelector('[data-ac-row]').textContent));
    h.check(true, 'the Rejected filter lists it, marked as rejected');
    await radio('Waiting (1)').click();
    await page.getByTestId('ac-undo').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-ac-row]').length === 2);
    h.equal((await rpc('proposal.list', { status: 'rejected' })).length, 0, 'Undo puts it back in the queue');
  });

  await S('a suggestion can be adjusted by whole words and given another code before it is accepted', async () => {
    const words = await sourceNamed('ac-words');
    const access = await codeNamed('Access');
    h.check(access !== undefined, 'the sample has a code Access to give');
    const row = rows().nth(1);
    h.check(/The cost of access is a worry\./.test(await row.innerText()), 'the second suggestion is the sentence about the cost of access');
    await row.getByTestId('ac-adjust').click();
    const editor = page.getByTestId('ac-adjust-editor');
    await editor.waitFor();
    await editor.getByRole('button', { name: 'Start one word later' }).click();
    await editor.getByRole('button', { name: 'End one word earlier' }).click();
    h.equal((await editor.getByTestId('ac-adjust-passage').locator('mark').innerText()).trim(), 'cost of access is a', 'the words follow the buttons');
    await editor.getByLabel('Code', { exact: true }).selectOption(String(access.id));
    await axe('Auto Code, adjusting a suggestion');
    await editor.getByTestId('ac-adjust-save').click();
    await editor.waitFor({ state: 'hidden' });
    const changed = (await rpc('proposal.list', { status: 'pending' })).find((p) => p.text === 'cost of access is a');
    h.check(changed !== undefined && changed.code_id === access.id, 'the engine has the new words and the new code');
    h.check(/Access/.test(await rows().nth(1).innerText()), 'and the row shows the new code');
    await rows().nth(1).getByTestId('ac-accept').click();
    await page.getByTestId('ac-result').waitFor();
    const codings = await rpc('coding.list', { source_id: words.id });
    h.equal(codings.length, 1, 'accepting makes one coding');
    h.equal([codings[0].code_id, await textOf(words.id, codings[0].start, codings[0].end)], [access.id, 'cost of access is a'], 'with the adjusted words and code');
  });

  await S('running the same search again says what is already waiting or coded, and sends only what is new', async () => {
    await open('setup');
    await page.getByLabel('Words to find').fill('cost');
    let sentences = await found();
    h.check(sentences.includes('Found 2 places.') && sentences.some((t) => /^1 is already waiting in the review queue/.test(t)) && sentences.some((t) => /^1 suggestion in 1 document/.test(t)), `a waiting suggestion is left out: ${sentences.join(' | ')}`);
    const access = await codeNamed('Access');
    await page.getByLabel('Words to find').fill('access');
    await page.getByLabel('Each suggestion covers').selectOption('hit');
    await page.getByLabel('Code to suggest', { exact: true }).selectOption(String(access.id));
    sentences = await found();
    h.check(sentences.some((t) => /^1 is already coded this way/.test(t)), `the word inside the accepted coding is left out: ${sentences.join(' | ')}`);
    await page.getByLabel('Words to find').fill('nonsenseword');
    sentences = await found();
    h.equal(sentences, ['Nothing was found.', 'There is nothing new to send.'], 'a search that finds nothing says so');
    h.equal(await page.getByTestId('ac-send').isDisabled(), true, 'and there is nothing to send');
  });

  await S('a whole run is accepted in one step and taken back with one undo, and so is rejecting it', async () => {
    const words = await sourceNamed('ac-words');
    const cost = await codeNamed('Cost');
    await page.getByLabel('Words to find').fill('was OR were OR is');
    await page.getByLabel('Code to suggest', { exact: true }).selectOption(String(cost.id));
    const sentences = await found();
    h.check(sentences.includes('Found 4 places.'), `four places: ${sentences.join(' | ')}`);
    await page.getByTestId('ac-send').click();
    await page.getByTestId('autocode-queue').waitFor();
    await page.getByRole('heading', { level: 3, name: /^Words: was OR were OR is/ }).waitFor();
    const before = (await rpc('coding.list', { source_id: words.id })).length;
    await page.getByTestId('ac-accept-all').click();
    await page.getByTestId('ac-result').waitFor();
    h.check(/Accepted 4 suggestions; they are now coded\./.test(await page.getByTestId('ac-result').innerText()), 'the result counts them');
    h.equal((await rpc('coding.list', { source_id: words.id })).length, before + 4, 'four codings are made');
    await page.getByTestId('ac-undo').click();
    const said = await waitStatus(/Took back 4 accepted suggestions/);
    h.check(/Took back 4 accepted suggestions/.test(said), `one undo takes back all four: ${said}`);
    h.equal((await rpc('coding.list', { source_id: words.id })).length, before, 'the codings are gone');
    h.equal((await rpc('proposal.list', { status: 'pending' })).length, 5, 'and the four wait again with the one still waiting');
    await page.getByTestId('ac-reject-all').click();
    await page.getByTestId('ac-result').waitFor();
    h.check(/Rejected 4 suggestions\./.test(await page.getByTestId('ac-result').innerText()), 'rejecting the whole run is one step too');
    await page.getByTestId('ac-undo').click();
    await waitStatus(/Put 4 rejected suggestions back in the queue/);
    h.equal((await rpc('proposal.list', { status: 'pending' })).length, 5, 'and one undo puts the four back');
  });

  await S('speakers are listed once each, a code of their own name is ready, and their turns become suggestions', async () => {
    await open('setup');
    await radio('Speakers').filter({ hasText: /Every turn/ }).click();
    await lookIn('ac-talk');
    const table = page.getByTestId('ac-speakers');
    await table.waitFor();
    const cells = await table.locator('tbody tr').evaluateAll((rs) => rs.map((r) => [...r.querySelectorAll('th,td')].map((c) => c.textContent.replace(/\s+/g, ' ').trim())));
    h.check(/^Interviewer/.test(cells[0][1]) && /Also written: INTERVIEWER/.test(cells[0][1]) && /0 \(2 left out of coding\)/.test(cells[0][2]), `the interviewer's spellings are one speaker and the turns left out are counted: ${cells[0].join(' | ')}`);
    h.check(/^Priya/.test(cells[1][1]) && /Also written: priya/.test(cells[1][1]) && cells[1][2] === '2', `Priya and priya are one speaker with two turns: ${cells[1].join(' | ')}`);
    h.equal(cells[2][2], '1', 'Sam has one turn');
    h.equal(await page.getByRole('checkbox', { name: 'Use the turns of Interviewer' }).isDisabled(), true, 'a speaker with no turns to code cannot be chosen');
    await axe('Auto Code, speakers');
    for (const n of ['Priya', 'Sam']) await page.locator('label').filter({ has: page.getByRole('checkbox', { name: `Use the turns of ${n}` }) }).click();
    const sentences = await found();
    h.equal(sentences, ['Found 3 turns.', '3 suggestions in 1 document can be sent to the review queue.'], 'three turns');
    await page.getByTestId('ac-send').click();
    await page.getByTestId('autocode-queue').waitFor();
    await page.getByRole('heading', { level: 3, name: 'Speakers: Priya, Sam' }).waitFor();
    const priya = await codeNamed('Priya');
    h.check(priya !== undefined && (await codeNamed('Sam')) !== undefined, 'a code for each speaker was made when the run was sent');
    h.check(/^#/.test(priya.color) && priya.color !== '#808080', `with a color from the palette: ${priya.color}`);
    await page.getByTestId('ac-accept-all').click();
    await page.getByTestId('ac-result').waitFor();
    const talk = await sourceNamed('ac-talk');
    const codings = await rpc('coding.list', { source_id: talk.id });
    h.equal(codings.length, 3, 'accepting codes the three turns');
    h.equal(await textOf(talk.id, codings[0].start, codings[0].end), 'I take the train most days.', "Priya's first turn, without her name");
  });

  await S('headings give the sections under them, and documents without headings say so', async () => {
    await open('setup');
    await radio('Headings').filter({ hasText: /section under each heading/ }).click();
    await lookIn('ac-words');
    await page.getByText(/have no headings, so there is nothing to use/).waitFor();
    h.check(true, 'a document without headings says so');
    await lookIn('ac-report');
    const level1 = page.locator('label').filter({ has: page.getByRole('checkbox', { name: /^Level 1/ }) });
    await level1.waitFor();
    h.check(/Level 1 \(3 headings in 1 document\)/.test(await level1.innerText()) && /Level 2 \(1 heading in 1 document\)/.test(await page.getByRole('group', { name: 'Heading levels' }).innerText()), 'the levels the documents have are listed with their counts');
    await level1.click();
    await page.getByLabel('Only headings that contain (optional)').fill('findings');
    await page.getByLabel('Code to suggest', { exact: true }).selectOption('new');
    await page.getByLabel('Name of the new code for Code to suggest').fill('Findings section');
    await axe('Auto Code, headings');
    const sentences = await found();
    h.equal(sentences, ['Found 1 section.', '1 suggestion in 1 document can be sent to the review queue.'], 'one section');
    const example = await page.getByRole('list', { name: 'Examples of what it finds' }).innerText();
    h.check(/Findings/.test(example) && /Stops are far apart\./.test(example), 'it runs through the level-2 heading inside it');
    await page.getByTestId('ac-send').click();
    await page.getByTestId('autocode-queue').waitFor();
    await page.getByTestId('ac-accept-all').click();
    await page.getByTestId('ac-result').waitFor();
    const report = await sourceNamed('ac-report');
    const codings = await rpc('coding.list', { source_id: report.id });
    h.equal(codings.length, 1, 'one coding');
    const coded = await textOf(report.id, codings[0].start, codings[0].end);
    h.check(coded.startsWith('Findings') && coded.endsWith('Stops are far apart.'), `the whole section is coded: ${JSON.stringify(coded)}`);
  });

  await S('the History page tells a bulk decision as one line', async () => {
    const events = await rpc('events.list', { limit: 500 });
    const accepts = events.filter((e) => e.action === 'proposal.accept');
    h.check(accepts.length >= 6, `the engine logs each accepted suggestion (${accepts.length})`);
    // The page folds them: look at the Project overview's recent activity.
    await page.getByRole('button', { name: 'Back to the coding workspace' }).first().click();
    await page.keyboard.press('Control+k');
    const dialog = page.getByRole('dialog', { name: 'Search or run a command' });
    await dialog.waitFor();
    await page.keyboard.type('overview');
    await dialog.getByRole('option', { name: /Go to the project overview/ }).click();
    await page.getByRole('heading', { level: 2 }).first().waitFor();
    const text = await page.getByRole('main').innerText();
    h.check(/Accepted \d+ suggested codings/.test(text) || /Accepted a suggested coding/.test(text), `the overview tells acceptances in one line each: ${text.match(/Accepted[^\n]{0,40}/)?.[0]}`);
    h.check(!/Coded .* as Priya/.test(text), 'and does not list the codings the acceptance made one by one');
    h.check(/suggestions? from Auto Code or an AI (is|are) waiting for your decision/.test(text), 'and, with suggestions still waiting, offers the review as the next step');
    await page.getByRole('button', { name: 'Review the suggestions' }).click();
    await page.getByTestId('autocode-queue').waitFor();
    h.check(true, 'which opens the review queue');
  });
}
