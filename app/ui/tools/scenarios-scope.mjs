// What the Statistics page offers (decision 193): PrismQDA describes, and the tests of difference, association, and prediction are
// PrismStatistics's. The page begins by describing; Settings > Analysis (or a button on the page) turns the tests on. And the case table
// that PrismStatistics opens is made from the Export dialog, where the page's notice sends the person.

import fs from 'node:fs';
import path from 'node:path';
import { unzipEntry } from './scenarios-export.mjs';

/** Turns the tests on in Settings, the way a person would, and goes back to where the person was. Other scenarios that use the test chooser begin with this. */
export async function useTests(api) {
  const { page } = api;
  await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
  await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Analysis', exact: true }).click();
  await page.locator('label').filter({ has: page.getByRole('radio', { name: /Describing and testing/ }) }).click();
}

export async function runScope(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe, sleep } = api;

  const openStatistics = async () => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Statistics', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Statistics', exact: true }).click();
      await page.getByRole('button', { name: 'Statistics page', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Statistics', exact: true }).waitFor();
    }
  };
  const goalCount = () => page.locator('[data-testid^="stats-goal-"]').count();
  const stored = () => page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.settings') ?? 'null'));

  await S('the Statistics page begins by describing, with a notice that says where the tests are done, and the tests can be turned on and off', async () => {
    // A fresh browser has no stored settings, so this is what a person gets at first.
    const before = await stored();
    h.check(before === null || before.statisticsScope === undefined || before.statisticsScope === 'descriptive', 'the tests are not on to begin with');
    await openStatistics();
    const card = page.getByTestId('stats-scope');
    await card.waitFor();
    h.check(/This page describes/.test(await card.innerText()), 'the notice says the page describes');
    h.check(/PrismStatistics/.test(await card.innerText()), 'and names PrismStatistics as the place for the tests');
    h.equal(await goalCount(), 1, 'one thing to do: describe one variable');
    h.check(await page.getByTestId('stats-goal-describe').isVisible(), 'and it is describing');
    h.equal(await page.getByTestId('stats-goal-compare_groups').count(), 0, 'no test of difference is offered');
    h.equal(await page.getByTestId('stats-goal-predict').count(), 0, 'no regression is offered');
    await axe('the Statistics page while it describes');

    // The button on the page turns the tests on, and says so on the page.
    await page.getByTestId('stats-show-tests').click();
    await page.getByTestId('stats-hide-tests').waitFor();
    h.check((await goalCount()) > 8, 'with the tests on, every goal is offered');
    h.check(await page.getByTestId('stats-goal-compare_groups').isVisible(), 'including the comparison of groups');
    h.equal((await stored()).statisticsScope, 'tests', 'the choice is remembered on this computer');
    h.check(/also set to offer the tests/.test(await card.innerText()), 'the notice now says the tests are on');
    await axe('the Statistics page with the tests on');

    // And back, from the page.
    await page.getByTestId('stats-hide-tests').click();
    await page.getByTestId('stats-show-tests').waitFor();
    h.equal(await goalCount(), 1, 'back to describing');
    h.equal((await stored()).statisticsScope, 'descriptive');
  });

  await S('Settings > Analysis has the same choice, and it agrees with the page', async () => {
    await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Analysis', exact: true }).click();
    const group = page.getByTestId('analysis-scope');
    await group.waitFor();
    h.check(await page.getByRole('radio', { name: /Describing the data/ }).isChecked(), 'describing is the one chosen');
    h.check(/PrismStatistics/.test(await group.innerText()), 'and the choice says where the tests are done');
    await axe('Settings, Analysis, with the choice of what the Statistics page offers');
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /Describing and testing/ }) }).click();
    h.equal((await stored()).statisticsScope, 'tests');
    await openStatistics();
    h.check((await goalCount()) > 8, 'the page now offers the tests');
    // Back to describing for the steps that follow.
    await page.getByTestId('stats-hide-tests').click();
    await page.getByTestId('stats-show-tests').waitFor();
  });

  await S('the case table for PrismStatistics is made from the Export dialog the notice opens, and says what it holds', async () => {
    await openStatistics();
    // The whole check has cases by now; the short run of this group starts from a new project, so it makes two.
    if ((await rpc('case.list', {})).length === 0) {
      await rpc('case.add', { name: 'Scope case one' });
      await rpc('case.add', { name: 'Scope case two' });
    }
    const cases = await rpc('case.list', {});
    h.check(Array.isArray(cases) && cases.length > 0, 'the project has cases to send');
    await page.getByTestId('stats-send-table').click();
    const dialog = page.getByRole('dialog', { name: 'Export' });
    await dialog.waitFor();
    h.check(await dialog.getByRole('radio', { name: /The case table for PrismStatistics/ }).isChecked(), 'the dialog opens on the case table');
    h.check(/never a coding/.test(await dialog.getByTestId('casetable-note').innerText()), 'and says a case is never a coding');
    await axe('the Export dialog on the case table');

    const target = path.join(workDir, 'scope-case-table');
    await dialog.getByLabel('Where to save it').fill(target);
    await dialog.getByRole('button', { name: 'Save the file' }).click();
    const report = dialog.getByTestId('casetable-report');
    await report.waitFor({ timeout: 60000 });
    const text = await report.innerText();
    h.check(new RegExp(`The file holds ${cases.length.toLocaleString('en-US')} cases`).test(text), `the report counts the project's ${cases.length} cases: ${text.slice(0, 120)}`);
    h.check(/No excerpt, document text, memo or coder/.test(text), 'and says what is never written');
    const file = `${target}.pstat`;
    h.check(fs.existsSync(file), 'the file is there with its ending');
    const project = JSON.parse(unzipEntry(file, 'project.json').toString('utf8'));
    const part = JSON.parse(unzipEntry(file, 'case-table.json').toString('utf8'));
    h.equal(project.profile, 'PrismQDA case table 1');
    h.equal(project.format, 'PrismStatistics project');
    h.equal(project.dataset.columns[0].values.length, cases.length, 'one row for each case');
    h.equal(part.part, 'PrismQDA case table');
    h.equal(part.project.id.length, 32, 'the part names the project by its id');
    h.check(/^PrismQDA /.test(project.app_version), 'and the file says which program made it');

    // A file that is there is not replaced by a typed path.
    await dialog.getByRole('button', { name: 'Save the file' }).click();
    await page.waitForFunction(() => /already exists/.test(document.querySelector('[role=dialog]')?.textContent ?? ''), null, { timeout: 20000 });
    h.check(true, 'a second save to the same path is refused in words');

    // Leaving the names out leaves them out.
    await dialog.getByTestId('export-casetable-names').click();
    await dialog.getByLabel('Where to save it').fill(path.join(workDir, 'scope-case-table-numbers'));
    await dialog.getByRole('button', { name: 'Save the file' }).click();
    await page.waitForFunction(() => /names of the cases were left out|names were left out/.test(document.querySelector('[data-testid=casetable-report]')?.textContent ?? ''), null, { timeout: 60000 });
    const numbers = JSON.parse(unzipEntry(path.join(workDir, 'scope-case-table-numbers.pstat'), 'project.json').toString('utf8'));
    h.check(numbers.dataset.variables.every((v) => v.name !== 'case_name'), 'no case_name variable');
    h.equal(JSON.parse(unzipEntry(path.join(workDir, 'scope-case-table-numbers.pstat'), 'case-table.json').toString('utf8')).case_name_variable, null);
    await dialog.getByRole('button', { name: 'Close' }).click();
    await sleep(100);
  });
}
