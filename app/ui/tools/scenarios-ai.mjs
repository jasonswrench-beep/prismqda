// The AI Assistant by prompt packet: building a packet from chosen documents and codes, reading its exact text, pasting an answer back,
// what the check refuses, the suggestions waiting in the review queue as AI suggestions (and one undo for a decision), the AI use log
// with its methods sentence, a document and a code kept out, and AI switched off. Nothing here talks to a model: the "answer" is typed.
// The documents and codes are the check's own, so nothing depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';

const RIDERS = 'Fares are too high for me.\n\nThe stop is far from my house.\n\nI love the new buses.\n';
const PRIVATE = 'Private words that must stay on this computer.\n';

export async function runAi(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe, codeRow, waitStatus } = api;

  const sourceNamed = async (name) => (await rpc('source.list')).find((s) => s.name === name);
  const codeNamed = async (name) => (await rpc('code.list')).find((c) => c.name === name);
  const tabs = () => page.getByRole('tablist', { name: 'AI Assistant' });
  const radio = (label) => page.locator('label').filter({ hasText: label });
  const packetText = () => page.getByTestId('ai-packet-text');
  /** The suggestions from an AI that wait in the queue (Auto Code's own suggestions from an earlier step may be waiting too). */
  const aiWaiting = async () => (await rpc('proposal.list', { status: 'pending' })).filter((q) => q.origin === 'ai');

  /** Opens the page from the AI tab of the ribbon. */
  async function open(half) {
    if (!(await page.getByRole('heading', { level: 2, name: 'AI Assistant' }).isVisible())) {
      await page.getByRole('tab', { name: 'AI', exact: true }).click();
      await page.getByRole('button', { name: 'AI Assistant', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'AI Assistant' }).waitFor();
    }
    await tabs().getByRole('tab', { name: half === 'log' ? /^AI use log/ : 'Prompt packet' }).click();
  }

  /** Waits (a few seconds at most) for something the engine or the page must come to say. */
  async function until(test, what) {
    for (let i = 0; i < 50; i++) {
      if (await test()) return;
      await page.waitForTimeout(100);
    }
    h.check(false, `timed out waiting for ${what}`);
  }

  /** Picks from one of the form's pickers: the button that opens it, the name of its list, and the names to leave checked. */
  async function pick(buttonName, dialogName, ...names) {
    await page.getByRole('button', { name: buttonName }).click();
    const dialog = page.getByRole('dialog', { name: dialogName });
    await dialog.waitFor();
    await dialog.getByRole('button', { name: 'None' }).click();
    for (const n of names) await dialog.locator('label').filter({ has: page.getByRole('checkbox', { name: n, exact: true }) }).click();
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
  }

  await S('the AI Assistant opens from the AI tab of the ribbon and says that it sends nothing itself', async () => {
    for (const [file, text] of [['ai-riders.txt', RIDERS], ['ai-private.txt', PRIVATE]]) fs.writeFileSync(path.join(workDir, file), text, 'utf8');
    await rpc('import.run', { path: path.join(workDir, 'ai-riders.txt') });
    await rpc('import.run', { path: path.join(workDir, 'ai-private.txt') });
    if (!(await codeNamed('AI cost'))) await rpc('code.add', { name: 'AI cost', definition: 'Talk about money.', inclusion_criteria: 'prices or fares come up' });
    if (!(await codeNamed('AI access'))) await rpc('code.add', { name: 'AI access' });
    await page.reload();
    await open('packet');
    const names = (await tabs().getByRole('tab').allTextContents()).map((t) => t.trim());
    h.equal(names, ['Prompt packet', 'AI use log'], 'the two halves; nothing has been built yet');
    const main = await page.getByRole('main').innerText();
    h.check(/does not connect to any AI service/.test(main), 'it says that PrismQDA does not connect to an AI service');
    h.check(/Nothing is coded until you accept it/.test(main), 'and that nothing is coded until a suggestion is accepted');
    h.equal(await page.getByTestId('ai-packet').count(), 0, 'no packet is shown before one is built');
    await axe('the AI Assistant before a packet is built');
  });

  await S('a document marked as one that must never be sent can be marked, and the mark is in the project', async () => {
    await page.getByTestId('ai-private').locator('summary').click();
    await page.locator('label').filter({ has: page.getByRole('checkbox', { name: 'Never send ai-private' }) }).click();
    await until(async () => (await sourceNamed('ai-private')).do_not_share, 'the mark to reach the engine');
    await page.getByTestId('ai-private').locator('summary').filter({ hasText: '(1 marked)' }).waitFor();
    h.equal((await sourceNamed('ai-private')).do_not_share, true, 'the engine has the mark');
    h.equal((await sourceNamed('ai-riders')).do_not_share, false, 'and only on that document');
    await axe('the AI Assistant with a document kept back');
  });

  await S('a packet is built from chosen documents and codes, and its exact text is on the screen', async () => {
    await radio('Documents I choose').click();
    await pick(/^Documents to send:/, 'Documents to send', 'ai-riders');
    await pick(/^Codes to ask about:/, 'Codes to ask about', 'AI cost', 'AI access');
    await page.getByTestId('ai-build').click();
    await packetText().waitFor();
    const text = await packetText().inputValue();
    h.check(text.includes('[P1] Fares are too high for me.\n[P2] The stop is far from my house.\n[P3] I love the new buses.'), `the passages are in the packet, one line each: ${text.slice(text.indexOf('PASSAGES'), text.indexOf('PASSAGES') + 120).replace(/\n/g, ' | ')}`);
    h.check(text.includes('- AI cost: Talk about money. Use it when: prices or fares come up.') && text.includes('- AI access\n'), 'and the codes with what each means');
    h.check(!text.includes('Private words'), 'a document marked never to be sent is not in it');
    const facts = (await page.getByTestId('ai-packet-facts').locator('li').allTextContents()).map((t) => t.trim());
    h.check(/^The packet has 3 passages and 2 codes \(AI cost, AI access\), [\d,]+ characters in all\.$/.test(facts[0]), `it says what it holds: ${facts[0]}`);
    h.check(/Nothing has left your computer/.test(await page.getByTestId('ai-nothing-sent').innerText()), 'and that nothing has left the computer');
    h.equal((await rpc('proposal.list')).filter((q) => q.origin === 'ai').length, 0, 'no AI suggestion is in the review queue');
    h.equal((await rpc('coding.list', { source_id: (await sourceNamed('ai-riders')).id })).length, 0, 'and nothing is coded');
    const runs = await rpc('ai.runs');
    h.equal([runs.length, runs[0].answered, runs[0].route], [1, false, 'packet'], 'the run is kept at once, with no answer yet');
    await page.screenshot({ path: `${ctx.shots}/ai-packet.png`, fullPage: true });
    await axe('the AI Assistant with a packet');
  });

  await S('the answer is checked and only what passes goes to the review queue; what is refused is told', async () => {
    const send = page.getByTestId('ai-send');
    h.equal(await send.isDisabled(), true, 'sending waits for a model name and an answer');
    await page.getByLabel('Which model gave this answer').fill('Test Bot 1');
    h.equal(await send.isDisabled(), true, 'a model name alone is not enough');
    const answer = [
      'Sure! Here is my answer [as you asked]:',
      '```json',
      JSON.stringify([
        { passage: 'P1', code: 'AI cost', quote: 'too high', reason: 'The rider talks about fares.' },
        { passage: 'P2', code: 'ai access', quote: '', reason: 'Distance to the stop.' },
        { passage: 'P3', code: 'AI cost', quote: 'invented words that are not there', reason: 'Made up.' },
        { passage: 'P1', code: 'Joy', quote: 'Fares', reason: 'No such code.' },
      ]),
      '```',
    ].join('\n');
    await page.getByLabel(/^The model.s answer$/).fill(answer);
    await send.click();
    await page.getByTestId('ai-outcome').waitFor();
    const facts = (await page.getByTestId('ai-outcome').locator('li').allTextContents()).map((t) => t.trim());
    h.equal(facts[0], 'The answer held 4 suggestions.', 'it says how many suggestions the answer held');
    h.check(/^2 suggestions passed the check and are waiting in the review queue\./.test(facts[1]), `and how many passed: ${facts[1]}`);
    h.equal(facts[2], '2 suggestions were refused by the check.', 'and how many were refused');
    const refused = (await page.getByTestId('ai-refused').locator('li').allTextContents()).map((t) => t.trim());
    h.check(/not in passage P3, word for word/.test(refused[0]) && /Joy/.test(refused[1]), `with the reason for each: ${refused.join(' | ')}`);
    const queue = await aiWaiting();
    h.equal(queue.map((q) => [q.origin, q.text]).sort(), [['ai', 'The stop is far from my house.'], ['ai', 'too high']].sort(), 'two AI suggestions wait, with the quoted words and the whole passage');
    h.equal((await rpc('coding.list', { source_id: (await sourceNamed('ai-riders')).id })).length, 0, 'nothing is coded until a person decides');
    const run = (await rpc('ai.runs'))[0];
    h.equal([run.model, run.answered, run.suggested, run.checker_rejected], ['Test Bot 1', true, 2, 2], 'the run keeps the model and the counts');
    await axe('the AI Assistant after an answer');
  });

  await S('the suggestions are in the review queue as AI suggestions, and one undo takes a decision back', async () => {
    await page.getByTestId('ai-review').click();
    await page.getByTestId('autocode-queue').waitFor();
    await page.getByTestId('ac-row').first().waitFor();
    h.equal(await page.getByTestId('ac-ai-badge').count(), 2, 'both rows say they are AI suggestions');
    const meta = await page.getByTestId('ac-row').first().innerText();
    h.check(/AI suggestion \(Test Bot 1\)/.test(meta), `and say which model: ${meta.replace(/\s+/g, ' ').slice(0, 160)}`);
    const row = page.getByTestId('ac-row').filter({ hasText: 'too high' });
    await row.getByTestId('ac-accept').click();
    await page.getByTestId('ac-result').waitFor();
    const source = await sourceNamed('ai-riders');
    const codings = await rpc('coding.list', { source_id: source.id });
    h.equal(codings.map((c) => c.origin), ['ai_suggestion_accepted'], 'accepting makes a coding that remembers it came from an AI suggestion');
    h.check(codings[0].ai_run_id !== null && codings[0].ai_run_id !== undefined, 'and from which run');
    await page.getByTestId('ac-undo').click();
    await page.waitForFunction(() => !document.querySelector('[data-testid=ac-result]'));
    h.equal((await rpc('coding.list', { source_id: source.id })).length, 0, 'one undo takes the acceptance back');
    h.equal((await aiWaiting()).length, 2, 'and the suggestion waits again');
    await axe('the review queue with AI suggestions');
  });

  await S('the AI use log lists the run, shows the exact packet and answer, and writes the sentence for a methods section', async () => {
    await open('log');
    await page.getByTestId('ai-run').first().waitFor();
    const line = await page.getByTestId('ai-run').first().innerText();
    h.check(/Packet \d+, answered by Test Bot 1/.test(line) && /2 suggestions sent to the review queue/.test(line) && /2 refused by the check/.test(line) && /2 waiting/.test(line), `the run is told in one line: ${line.replace(/\s+/g, ' ')}`);
    await page.getByTestId('ai-run').first().click();
    const detail = page.getByTestId('ai-run-detail');
    await detail.getByLabel('The packet, exactly as it was built').waitFor();
    h.check((await detail.getByLabel('The packet, exactly as it was built').inputValue()).includes('[P1] Fares are too high for me.'), 'the packet is there exactly as it was built');
    h.check((await detail.getByLabel('The answer, exactly as it was pasted').inputValue()).includes('invented words that are not there'), 'and the answer exactly as it was pasted');
    h.check(/SHA-256/.test(await detail.innerText()), 'with a fingerprint of the packet');
    const paragraph = await page.getByLabel('Statement about AI use').inputValue();
    h.check(/only through prompt packets/.test(paragraph) && /Test Bot 1/.test(paragraph) && /the program sent nothing itself/.test(paragraph), `the methods sentence says what was done: ${paragraph.slice(0, 120)}`);
    h.check(!paragraph.includes('—'), 'and has no em dashes');
    await detail.getByTestId('ai-run-queue').click();
    await page.getByTestId('autocode-queue').waitFor();
    h.check(true, 'the run opens its suggestions in the review queue');
    await open('log');
    await axe('the AI use log');
  });

  await S('a second packet can go on from where the first stopped, and a run with no answer can be answered later', async () => {
    await open('packet');
    await page.getByLabel('How many passages').fill('2');
    await page.getByTestId('ai-build').click();
    await page.getByTestId('ai-next').waitFor();
    h.check(/\[P1\] Fares are too high/.test(await packetText().inputValue()), 'the first two passages go in the packet');
    h.check(/1 more passage could go in another packet/.test(await page.getByTestId('ai-packet-facts').innerText()), 'it says one more could go in another');
    await page.getByTestId('ai-next').click();
    await page.waitForFunction(() => /\[P1\] I love the new buses\./.test(document.querySelector('[data-testid=ai-packet-text]')?.value ?? ''));
    h.check(true, 'the next packet starts at the passage after the first two, and its labels start again at P1');
    h.equal((await rpc('ai.runs')).length, 3, 'each packet is a run of its own');
    // Leaving the page and coming back keeps the packet; the log offers to answer a run that has no answer.
    await open('log');
    await page.getByTestId('ai-run').first().click();
    await page.getByTestId('ai-run-continue').click();
    await page.getByTestId('ai-packet-text').waitFor();
    h.check(/I love the new buses/.test(await packetText().inputValue()), 'the log opens the unanswered packet on the Prompt packet tab, ready for its answer');
    await page.getByLabel('Which model gave this answer').fill('Test Bot 2');
    await page.getByLabel(/^The model.s answer$/).fill('I am sorry, I cannot help with that.');
    await page.getByTestId('ai-send').click();
    const alert = await page.getByTestId('ai-answer-card').getByRole('alert').innerText();
    h.check(/no list/.test(alert), `an answer with no list is refused and says why: ${alert.replace(/\s+/g, ' ')}`);
    h.equal((await rpc('ai.runs'))[0].answered, false, 'and the packet still waits for its answer');
    await page.getByLabel(/^The model.s answer$/).fill('[]');
    await page.getByTestId('ai-send').click();
    await page.getByTestId('ai-outcome').waitFor();
    h.equal((await page.getByTestId('ai-outcome').locator('li').allTextContents()).map((t) => t.trim()), ['The answer held no suggestions.'], 'an empty list is a valid answer: the model found nothing');
  });

  await S('AI Suggestible turned off in the Codebook Editor keeps a code out of every packet', async () => {
    await page.getByRole('navigation', { name: 'Page' }).getByRole('button', { name: 'Back to the coding workspace' }).click();
    await page.getByRole('tab', { name: 'Code', exact: true }).click();
    await page.getByRole('button', { name: 'Codebook Editor', exact: true }).click();
    await api.editorHeading().waitFor();
    await codeRow('AI access').click();
    const form = page.getByTestId('code-detail');
    const box = form.getByRole('checkbox', { name: /^AI Suggestible/ });
    h.equal(await box.isChecked(), true, 'AI Suggestible is on to begin with');
    await page.locator('label').filter({ has: page.getByRole('checkbox', { name: /^AI Suggestible/ }) }).click();
    h.check(/never put in a prompt packet/.test(await page.getByTestId('prop-ai-note').innerText()), 'turning it off says what it means');
    await form.getByRole('button', { name: 'Save changes' }).click();
    await waitStatus(/Saved the code AI access/);
    h.equal((await codeNamed('AI access')).ai_suggestible, false, 'the engine has it');
    await axe('the Codebook Editor with AI Suggestible off');
    await api.closeDefinition();
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
    await open('packet');
    await pick(/^Codes to ask about:/, 'Codes to ask about');
    await page.getByTestId('ai-build').click();
    // The packet of the step before is still on the page until the new one replaces it: wait for what only the new one says.
    await page.getByTestId('ai-packet-facts').getByText(/was left out: AI access/).waitFor();
    const text = await packetText().inputValue();
    h.check(text.includes('- AI cost') && !text.includes('AI access'), 'a packet built with no codes chosen asks about AI cost but not AI access');
    h.check(/1 code marked do not share or not for AI is never sent/.test(await page.getByTestId('ai-packet-tab').innerText()), 'and the form says one code is held back');
  });

  await S('switching AI off in Settings stops packets for the project, and an undo of the setting puts it back', async () => {
    await page.getByRole('tab', { name: 'View', exact: true }).click();
    await page.getByRole('group', { name: 'Pages' }).getByRole('button', { name: 'Settings' }).click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'AI Assistant' }).click();
    const section = page.getByTestId('settings-section');
    h.check(/never connects to an AI service/.test(await section.innerText()), 'the Settings section says PrismQDA never connects to an AI service');
    const allow = section.getByRole('checkbox', { name: /^Allow prompt packets/ });
    h.equal(await allow.isChecked(), true, 'prompt packets are allowed to begin with');
    await page.locator('label').filter({ has: page.getByRole('checkbox', { name: /^Allow prompt packets/ }) }).click();
    await until(async () => (await rpc('project.info')).ai_mode === 'off', 'the setting to reach the engine');
    h.equal((await rpc('project.info')).ai_mode, 'off', 'the project records it');
    await axe('Settings, AI Assistant, switched off');
    await page.getByTestId('settings-open-ai').click();
    await page.getByTestId('ai-off').waitFor();
    h.equal(await page.getByTestId('ai-build').isDisabled(), true, 'the page says AI is off and the packet cannot be built');
    h.equal((await rpc('ai.runs')).length >= 3, true, 'the record of earlier packets is kept');
    await page.getByTestId('ai-turn-on').click();
    await page.getByTestId('ai-off').waitFor({ state: 'hidden' });
    await until(async () => (await rpc('project.info')).ai_mode === 'packet', 'the setting to reach the engine');
    h.equal((await rpc('project.info')).ai_mode, 'packet', 'allowing packets again turns it back on');
    h.equal(await page.getByTestId('ai-build').isDisabled(), false, 'and the packet can be built');
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
  });
}
