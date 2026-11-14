// Text Analytics: counting the words of some documents (leaving out common words, short words, or words outside a code), and finding where
// a word or phrase is used with the words round it. The documents, their case, and their code are the check's own, and the words in
// them are made up, so nothing depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';

const FIRST = 'Zorbles are too round. Zorbles are a puzzle, and I do not like the zorbles.';
const SECOND = 'The glimmer was kind. The glimmer was helpful.';

export async function runText(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;
  let firstId = 0;

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
  const rows = () => page.getByTestId('text-table').locator('tbody tr');

  await S('Text Analytics opens from its tab, and counts the words of a case, the commonest first', async () => {
    for (const [file, text] of [['text-a.txt', FIRST], ['text-b.txt', SECOND]]) fs.writeFileSync(path.join(workDir, file), text, 'utf8');
    firstId = (await rpc('import.run', { path: path.join(workDir, 'text-a.txt') })).outcome.source_ids[0];
    const secondId = (await rpc('import.run', { path: path.join(workDir, 'text-b.txt') })).outcome.source_ids[0];
    const kase = await rpc('case.add', { name: 'Text case' });
    for (const id of [firstId, secondId]) await rpc('case.assign', { case_id: kase.id, source_id: id });
    await page.reload();
    await open();
    await axe('Text Analytics');
    await page.locator('#text-case').selectOption({ label: 'The documents of the case Text case' });
    await count();
    h.equal((await page.getByTestId('text-summary').innerText()).trim(), '10 of 23 words were counted, 7 of them different, in 2 documents.', 'what was counted');
    const first = await rows().nth(0).innerText();
    h.check(first.includes('zorbles') && first.includes('3') && first.includes('30'), `the commonest word, its count and share: ${first.replace(/\s+/g, ' ')}`);
    h.check(/glimmer/.test(await rows().nth(1).innerText()), 'then the next');
    h.equal(await rows().count(), 7, 'seven different words are listed');
    await axe('Text Analytics with a count');
  });

  await S('common words and short words can be counted too, and only the words inside a code can be counted', async () => {
    await page.getByTestId('text-common').click();
    await page.getByLabel('Shortest word, in letters').fill('1');
    await count();
    h.equal((await page.getByTestId('text-summary').innerText()).trim(), '23 of 23 words were counted, 16 of them different, in 2 documents.', 'every word is counted');
    const all = await page.getByTestId('text-table').innerText();
    h.check(/\bthe\b/.test(all), 'the common words are listed now');
    await page.getByTestId('text-common').click();
    await page.getByLabel('Shortest word, in letters').fill('3');

    const money = await rpc('code.add', { name: 'Text money' });
    await rpc('coding.apply', { source_id: firstId, start: 0, end: 22, code_id: money.id });
    await page.reload();
    await open();
    await page.locator('#text-case').selectOption({ label: 'The documents of the case Text case' });
    await page.locator('#text-code').selectOption({ label: 'Only the words inside passages coded Text money' });
    await count();
    h.equal((await page.getByTestId('text-summary').innerText()).trim(), '2 of 4 words were counted, 2 of them different, in 2 documents.', 'only the first sentence is counted');
    const text = await page.getByTestId('text-table').innerText();
    h.check(/zorbles/.test(text) && /round/.test(text) && !/puzzle/.test(text), 'and only its words are listed');
  });

  await S('a word chosen from the list is looked up in context, and a word can be opened in its document', async () => {
    await page.getByRole('button', { name: 'See where zorbles is used', exact: true }).click();
    await page.getByTestId('kwic-summary').waitFor();
    h.equal((await page.getByTestId('kwic-summary').innerText()).trim(), 'Used 3 times in 1 document.', 'the word is used three times');
    const kwic = page.getByTestId('kwic-table').locator('tbody tr');
    h.equal(await kwic.count(), 3, 'a line for each place');
    const first = (await kwic.nth(0).innerText()).replace(/\s+/g, ' ');
    h.check(/Zorbles/.test(first) && /are too round/.test(first), `with the words after it: ${first}`);
    const last = (await kwic.nth(2).innerText()).replace(/\s+/g, ' ');
    h.check(/like the/.test(last), `and before it: ${last}`);
    await axe('Keywords in context');
    await kwic.nth(1).getByRole('button', { name: /^Open text-a at this word/ }).click();
    await page.getByRole('heading', { level: 2, name: 'Text Analytics', exact: true }).waitFor({ state: 'hidden' });
    await open();
  });

  await S('a phrase is found as its words in order, and nothing to look for is refused in words', async () => {
    await page.getByTestId('text-tab-context').click();
    await page.getByTestId('text-query').fill('GLIMMER was');
    await page.getByTestId('text-search').click();
    await page.waitForFunction(() => /Used 2 times/.test(document.querySelector('[data-testid=kwic-summary]')?.textContent ?? ''), null, { timeout: 8000 });
    h.check(/glimmer was/i.test((await page.getByTestId('kwic-table').locator('tbody tr').nth(0).innerText()).replace(/\s+/g, ' ')), 'a phrase and capitals do not matter');
    await page.getByTestId('text-query').fill('zorble');
    await page.getByTestId('text-search').click();
    await page.waitForFunction(() => /not used/.test(document.querySelector('[data-testid=kwic-summary]')?.textContent ?? ''), null, { timeout: 8000 });
    h.check(true, 'part of a word is not the word');
    await page.getByTestId('text-query').fill('   ');
    await page.getByTestId('text-search').click();
    await page.getByRole('alert').filter({ hasText: 'Type a word' }).waitFor();
  });

  // The working set (decision 163): a page keeps what it shows until it is run again, and says so when the set has changed since.
  await S('a count made before the working set changed says so, and counting again takes the note away', async () => {
    await page.getByTestId('text-tab-words').click();
    await count();
    h.equal(await page.getByTestId('stale-set').count(), 0, 'a fresh count has no note');
    await page.getByRole('tab', { name: 'View', exact: true }).click();
    await page.locator('[data-command="working-set"]').click();
    await page.getByTestId('ws-documents').waitFor();
    await page.getByTestId('ws-documents-all').click();
    await page.getByTestId('ws-documents-untick-shown').click();
    await page.getByTestId('ws-documents-item').first().click();
    await page.getByTestId('ws-apply').click();
    await page.getByTestId('stale-set').waitFor();
    h.check(/working set has changed/.test(await page.getByTestId('stale-set').innerText()), 'the count says it was made before the working set changed');
    await axe('Text Analytics with a count from before the working set changed');
    await count();
    h.equal(await page.getByTestId('stale-set').count(), 0, 'counting again takes the note away');
    await page.getByTestId('working-set-reset').click();
    await page.getByTestId('stale-set').waitFor();
    await count();
    h.equal(await page.getByTestId('stale-set').count(), 0, 'and so does counting after showing everything again');
  });
}
