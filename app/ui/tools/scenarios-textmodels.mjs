// Text Analytics, the second part (decision 202): the forms of a word counted together, topics, and sentiment; and, in Settings, the page that
// says how an assistant is connected (decision 203). The documents and cases are the check's own, and the words in them are made up, so
// nothing depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';

export async function runTextModels(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const open = async () => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Text Analytics', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Text Analytics', exact: true }).click();
      await page.getByRole('button', { name: 'Text Analytics page', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Text Analytics', exact: true }).waitFor();
    }
  };
  const count = async () => {
    await page.getByTestId('text-count').click();
    await page.getByTestId('text-summary').waitFor();
    await page.waitForFunction(() => !document.querySelector('[data-testid=text-count]')?.textContent?.includes('Counting'), null, { timeout: 15000 });
  };
  const makeCase = async (name, file, text) => {
    fs.writeFileSync(path.join(workDir, file), text, 'utf8');
    const id = (await rpc('import.run', { path: path.join(workDir, file) })).outcome.source_ids[0];
    const kase = await rpc('case.add', { name });
    await rpc('case.assign', { case_id: kase.id, source_id: id });
    return id;
  };

  await S('the forms of an English word can be counted as one, and each form is listed', async () => {
    await makeCase('Stem case', 'text-stem.txt', 'I connect the lines. They connected it. Connecting is hard. Connections matter.');
    await page.reload();
    await open();
    await page.locator('#text-case').selectOption({ label: 'The documents of the case Stem case' });
    await count();
    h.check(!/Forms counted together/.test(await page.getByTestId('text-table').innerText()), 'without the tick nothing is grouped');
    await page.getByTestId('text-stem').click();
    await count();
    const table = page.getByTestId('text-table');
    h.check(/Forms counted together/.test(await table.innerText()), 'the table has a column for the forms');
    const row = table.locator('tbody tr').filter({ hasText: 'connect' }).first();
    const said = (await row.innerText()).replace(/\s+/g, ' ');
    h.check(/^connect 4 /.test(said) && /connect \(1\), connected \(1\), connecting \(1\), connections \(1\)/.test(said), `four forms counted as one word: ${said}`);
    await axe('Text Analytics with the forms of a word counted together');
    await page.getByTestId('text-stem').click();
  });

  await S('topics are found from the words, as many as asked for, with the passages that belong to each, and read again the same', async () => {
    const travel = ['bus', 'train', 'station', 'platform', 'ticket', 'driver', 'route', 'timetable', 'commute', 'journey'];
    const food = ['soup', 'bread', 'kitchen', 'dinner', 'recipe', 'flavour', 'breakfast', 'lunch', 'cooking', 'dessert'];
    const lines = [];
    for (let i = 0; i < 20; i++) {
      lines.push(`The ${[0, 1, 2, 3, 4, 5].map((j) => travel[(i * 3 + j) % 10]).join(' and the ')} was very late today.`);
      lines.push(`The ${[0, 1, 2, 3, 4, 5].map((j) => food[(i * 3 + j) % 10]).join(' and the ')} was very tasty today.`);
    }
    await makeCase('Topic case', 'text-topics.txt', lines.join('\n\n'));
    await page.reload();
    await open();
    await page.getByTestId('text-tab-topics').click();
    await page.getByTestId('text-topics').waitFor();
    await page.locator('#topics-case').selectOption({ label: 'The documents of the case Topic case' });
    await page.getByTestId('topics-count').selectOption('2');
    await page.getByTestId('topics-run').click();
    await page.getByTestId('topics-result').waitFor();
    const summary = await page.getByTestId('topics-summary').innerText();
    h.check(/2 topics found in 40 passages/.test(summary), `what was found: ${summary}`);
    const topics = page.getByTestId('topic');
    h.equal(await topics.count(), 2, 'two topics');
    const kinds = [];
    for (let i = 0; i < 2; i++) {
      const words = await topics.nth(i).getByTestId('topic-words').innerText();
      kinds.push(/late/.test(words) ? 'travel' : /tasty/.test(words) ? 'food' : 'mixed');
    }
    h.equal([...kinds].sort(), ['food', 'travel'], `each topic is one subject: ${kinds}`);
    h.check(/not a finding/.test(await page.getByTestId('topics-result').innerText()), 'the page says a topic is not a finding');
    const first = await page.getByTestId('topics-list').innerText();
    await page.getByTestId('topics-run').click();
    await page.waitForFunction(() => !document.querySelector('[data-testid=topics-run]')?.textContent?.includes('Looking'), null, { timeout: 20000 });
    h.equal(await page.getByTestId('topics-list').innerText(), first, 'the same text and settings give the same topics');
    await axe('Text Analytics with topics');
  });

  await S('sentiment counts the passages by the words in them, turns a denied word round, and takes the person\'s own words', async () => {
    await makeCase('Mood case', 'text-mood.txt', 'The bus was great and the driver was kind.\n\nThe fares are terrible.\n\nIt was not good.\n\nWe went to the shop.');
    await page.reload();
    await open();
    await page.getByTestId('text-tab-sentiment').click();
    await page.getByTestId('text-sentiment').waitFor();
    await page.locator('#sentiment-case').selectOption({ label: 'The documents of the case Mood case' });
    await page.getByTestId('sentiment-run').click();
    await page.getByTestId('sentiment-result').waitFor();
    const total = (await page.getByTestId('sentiment-total').locator('tbody tr').innerText()).replace(/\s+/g, ' ');
    h.check(/1 \(25%\) 2 \(50%\) 0 \(0%\) 1 \(25%\)/.test(total), `one positive, two negative (one of them "not good"), none mixed, one neutral: ${total}`);
    const examples = await page.getByTestId('sentiment-example').allInnerTexts();
    h.check(examples.some((e) => /great, kind/.test(e)) && examples.some((e) => /not good/.test(e)), `the passages that counted are given with their words: ${examples.join(' | ').replace(/\s+/g, ' ')}`);
    h.check(/read a sample of each kind/.test(await page.getByTestId('sentiment-result').innerText()), 'the page says what a list of words cannot do');
    await axe('Text Analytics with sentiment');
    await page.getByTestId('sentiment-positive').fill('shop');
    await page.getByTestId('sentiment-run').click();
    await page.waitForFunction(() => /2 \(50%\) 2 \(50%\) 0 \(0%\) 0 \(0%\)/.test((document.querySelector('[data-testid=sentiment-total] tbody tr')?.innerText ?? '').replace(/\s+/g, ' ')), null, { timeout: 15000 });
    await page.getByTestId('sentiment-negative').fill('shop');
    await page.getByTestId('sentiment-run').click();
    await page.getByRole('alert').filter({ hasText: 'both the positive and the negative' }).waitFor();
  });

  await S('Settings says how an assistant is connected, and writes the settings for the application to read', async () => {
    await page.getByRole('tab', { name: 'View', exact: true }).click();
    await page.getByRole('group', { name: 'Pages' }).getByRole('button', { name: 'Settings' }).click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'AI Assistant' }).click();
    const card = page.getByTestId('assistant-card');
    await card.waitFor();
    h.check(/opens no port and connects to nothing/.test(await card.innerText()), 'the card says PrismQDA opens no port');
    const read = JSON.parse(await page.getByTestId('assistant-settings').inputValue());
    const args = read.mcpServers.prismqda.args;
    h.equal([args[0], args[2], args[3]], ['mcp', '--permission', 'read'], 'the settings start the program with mcp, for reading');
    h.equal(args[1], (await rpc('project.info')).path, 'for the open project');
    const note = await page.getByTestId('assistant-passphrase-note').innerText();
    if (!/PRISMQDA_PASSPHRASE/.test(note) || !/same Windows user/.test(note) || !/computer you control/.test(note)) throw new Error('the passphrase note is missing or incomplete: ' + note);
    await page.getByTestId('assistant-propose').check();
    await page.getByTestId('assistant-actor').fill('Reader Name');
    const propose = JSON.parse(await page.getByTestId('assistant-settings').inputValue());
    h.equal(propose.mcpServers.prismqda.args.slice(2), ['--permission', 'propose', '--actor', 'Reader Name'], 'a session that proposes says whose coding it would become');
    await axe('Settings, AI Assistant, connecting an assistant');
  });
}
