// Coach mode and an assignment's settings (decisions 199 and 200): the Coach tab of the Team page asks questions about the coding of the person
// using the project, and only asks; an instructor's assignment settings go into each student's packet, where the coach stays on, AI prompt
// packets stay closed, and pages are closed; and the answer key is a file of its own. The student's side is made by opening the packet through
// the command layer, so nothing here depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';

const TEXT = 'The fares are too high and the bus is late. I love the bus but the fares are rising.';

export async function runCoach(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  const coachPath = path.join(workDir, 'coach-exercise.pqda');

  const openTeamTab = async (tabId, readyId) => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Team', exact: true }).click();
      await page.getByRole('button', { name: 'Team page', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();
    }
    await page.getByTestId(tabId).click();
    await page.getByTestId(readyId).waitFor();
  };

  await S('the coach asks questions about your own coding, in the words of a question, and changes nothing', async () => {
    await rpc('project.create', { path: coachPath, name: 'Coach exercise' });
    fs.writeFileSync(path.join(workDir, 'coach-riders.txt'), TEXT, 'utf8');
    await rpc('import.run', { path: path.join(workDir, 'coach-riders.txt') });
    await rpc('code.add', { name: 'Cost' });
    const source = (await rpc('source.list'))[0];
    const id = (await rpc('code.list')).find((c) => c.name === 'Cost').id;
    await rpc('coding.apply', { source_id: source.id, start: 4, end: 9, code_id: id });
    const before = await rpc('project.info');
    await page.reload();
    await openTeamTab('team-tab-coach', 'coach-tab');
    await page.getByTestId('coach-check').click();
    await page.getByTestId('coach-result').waitFor();
    h.check(/1 coding of yours in 1 document looked at, using 1 code\./.test(await text(page.getByTestId('coach-summary'))), 'the page says what it looked at');
    const questions = page.getByTestId('coach-question');
    h.equal(await questions.count(), 2, 'two questions: a code with no definition, and words coded in one place and not in another');
    h.check(/“Cost” has no definition, and you used it once\. How would you tell someone else when to use it\?/.test(await text(questions.nth(0))), `the first is a question: ${await text(questions.nth(0))}`);
    const second = await text(questions.nth(1));
    h.check(/You coded “fares” as “Cost”, and the same words stand uncoded in one other place\. Is there a difference in meaning, or should they be coded alike\?/.test(second) && /fares are rising/.test(second), `the second shows the place: ${second}`);
    h.check(/Codes used once: nothing to ask/.test(await text(page.getByTestId('coach-rules'))), 'a kind of question with nothing to ask is said so');
    const after = await rpc('project.info');
    h.equal(after.codings, before.codings, 'the coach changed no coding in the project');
    await axe('the coach with its questions');
  });

  await S('a chatbot prompt can be made for the questions, asks for questions and not answers, and is closed when AI is off', async () => {
    await page.getByTestId('coach-prompt').click();
    const area = page.getByTestId('coach-prompt-text');
    await area.waitFor();
    const value = await area.inputValue();
    h.check(/Do not tell them which code to use/.test(value) && /- Cost: \(no definition yet\)/.test(value) && /"Cost" has no definition and was used 1 time\(s\)\./.test(value), `the prompt asks for questions and holds the findings: ${value.slice(0, 160)}`);
    h.check(/Making it sends nothing/.test(await text(page.getByTestId('coach-ai-card'))), 'the page says making it sends nothing');
    await axe('the coach with a prompt made');
    await rpc('ai.set_mode', { mode: 'off' });
    await page.reload();
    await openTeamTab('team-tab-coach', 'coach-tab');
    h.equal(await page.getByTestId('coach-prompt').isDisabled(), true, 'with AI off the prompt cannot be made');
    h.check(/switched off for this project/.test(await text(page.getByTestId('coach-ai-off'))), 'and the page says why');
    await page.getByTestId('coach-check').click();
    await page.getByTestId('coach-result').waitFor();
    h.equal(await page.getByTestId('coach-question').count(), 2, 'the questions need no AI');
    await rpc('ai.set_mode', { mode: 'packet' });
  });

  await S('an assignment\'s settings go into each packet: the coach stays on, AI is closed, and pages are closed', async () => {
    await page.reload();
    await openTeamTab('team-tab-class', 'class-tab');
    const folder = path.join(workDir, 'coach-packets');
    await page.getByTestId('class-students').fill('Dina');
    await page.getByTestId('class-folder').fill(folder);
    await page.getByTestId('class-assignment-title').fill('Week 3');
    await page.getByTestId('class-assignment-ai').click();
    await page.getByTestId('class-assignment-coach').click();
    h.equal(await page.getByTestId('class-closed-team').locator('input').isDisabled(), true, 'the Team page, which holds the coach, cannot be closed while the coach is kept on');
    await page.getByTestId('class-closed-explore').click();
    await page.getByTestId('class-closed-stats').click();
    await axe('the Class tab with the settings of an assignment');
    await page.getByTestId('class-make').click();
    await page.getByTestId('class-packets-result').waitFor();
    const packetFile = path.join(folder, 'coach-exercise - packet for Dina.pqda');
    h.check(fs.existsSync(packetFile), 'the packet was made');

    // The student opens it.
    await rpc('project.close', {});
    await rpc('project.open', { path: packetFile, backup: false });
    const info = await rpc('project.info');
    const settings = info.packet.assignment;
    h.equal([settings.title, settings.ai_allowed, settings.coach_locked, settings.closed], ['Week 3', false, true, ['explore', 'stats']], 'the settings are in the packet');
    h.equal(info.ai_mode, 'off', 'AI prompt packets are closed in it');
    let refused = '';
    try {
      await rpc('ai.set_mode', { mode: 'packet' });
    } catch (e) {
      refused = String(e.message);
    }
    h.check(/instructor has turned AI prompt packets off/.test(refused), `and cannot be turned on again: ${refused}`);

    await page.reload();
    const bar = page.getByTestId('coach-bar');
    await bar.waitFor();
    await page.getByTestId('coach-bar-text').waitFor();
    h.check(/The coach has \d+ questions? for you|The coach has no question for you at the moment|The coach is on/.test(await text(bar)), `the coach is on while the student codes: ${await text(bar)}`);
    await page.getByTestId('coach-bar-open').click();
    await page.getByTestId('coach-tab').waitFor();
    h.check(/instructor has asked that the coach stay on/.test(await text(page.getByTestId('coach-locked'))), 'the Team page opens on the coach, and says why it is kept on');
    h.check(/instructor has turned this off for this assignment/.test(await text(page.getByTestId('coach-ai-off'))), 'and that AI is off for the assignment');
    await axe('the coach of a student whose instructor keeps it on');

    // A closed page says so, and the open pages are as they were.
    await page.getByRole('tab', { name: 'Explore', exact: true }).click();
    await page.getByRole('button', { name: 'Query', exact: true }).click();
    await page.getByTestId('page-closed').waitFor();
    h.check(/Your instructor has closed this page for this assignment/.test(await text(page.getByTestId('page-closed'))), 'Explore is closed');
    await axe('a page closed by the instructor');

    // The answer key is a file of its own: only the instructor's coding.
    await rpc('project.close', {});
    await rpc('project.open', { path: coachPath, backup: false });
    await page.reload();
    await openTeamTab('team-tab-class', 'class-tab');
    await page.getByTestId('class-folder').fill(folder);
  });

  await S('the instructor\'s coding is saved as an answer key of its own', async () => {
    const instructor = (await rpc('coder.list')).find((c) => c.role === 'human');
    h.check(Boolean(instructor), 'the project has its person who codes');
    await page.getByTestId('class-key').click();
    const made = page.getByTestId('class-key-made');
    await made.waitFor();
    const said = await text(made);
    h.check(/Saved .*answer key\.pqda: 1 coding of yours\./.test(said), `the page says what was saved: ${said}`);
    const keyFile = path.join(workDir, 'coach-packets', 'Coach exercise answer key.pqda');
    h.check(fs.existsSync(keyFile), 'the file is there');
    await page.getByTestId('class-key').click();
    await page.getByRole('alert').filter({ hasText: 'already exists' }).waitFor();
    await axe('the Class tab with the answer key saved');
  });
}
