// The data editor's later tools: the part of a date, a random sample, a summary of each group, putting the cases in order, tables by group and
// restructuring (stood on end, laid across, saved as a file), merging variables from a file by a key, and on the Statistics page counting cases
// by a weight and running an analysis for each group. The cases are the check's own (six invented respondents with variable names no earlier
// step uses), and every number below was worked out by hand.
//
//   case:     D1   D2   D3   D4   D5   D6
//   Dt site:  N    N    N    S    S    S
//   Dt score: 10   12   14   20   22   24
//   Dt seen:  2024-03-14, 2024-12-30, 2021-01-03, 2020-12-31, 2022-01-01, 2023-06-30
//   Dt w1:    1    4    7    10   13   16        Dt w2: 2 5 (none) 11 14 17        Dt w3: 3 6 9 12 15 18
//   Dt times: 2    1    1    3    1    0         (how many times each is counted)

import fs from 'node:fs';
import path from 'node:path';

const ROWS = [
  ['D1', 'N', 10, '2024-03-14', 1, 2, 3, 2],
  ['D2', 'N', 12, '2024-12-30', 4, 5, 6, 1],
  ['D3', 'N', 14, '2021-01-03', 7, '', 9, 1],
  ['D4', 'S', 20, '2020-12-31', 10, 11, 12, 3],
  ['D5', 'S', 22, '2022-01-01', 13, 14, 15, 1],
  ['D6', 'S', 24, '2023-06-30', 16, 17, 18, 0],
];

export async function runDataTools(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const open = async () => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Data editor', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Statistics', exact: true }).click();
      await page.getByRole('button', { name: 'Data editor', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Data editor', exact: true }).waitFor();
    }
  };
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  const data = () => rpc('data.get');
  const valueOf = async (caseName, variable) => {
    const d = await data();
    const r = d.cases.findIndex((c) => c.name === caseName);
    const c = d.variables.findIndex((v) => v.name === variable);
    return d.values[r][c];
  };
  const dialog = (name) => page.getByRole('dialog', { name });
  /** Ticks a box: its input is hidden, so the label around it is pressed. */
  const tick = (scope, name) => scope.locator('label').filter({ has: page.getByRole('checkbox', { name, exact: true }) }).click();
  const preview = async (dlg) => {
    await dlg.locator('[data-testid=tf-preview-summary],[data-testid=tf-preview-error]').first().waitFor({ timeout: 8000 });
  };

  await S('the cases for the data tools are imported, and the Data editor offers the new tools beside the first five', async () => {
    let csv = 'id,Dt site,Dt score,Dt seen,Dt w1,Dt w2,Dt w3,Dt times,answer\n';
    for (const [id, site, score, seen, w1, w2, w3, times] of ROWS) csv += `${id},${site},${score},${seen},${w1},${w2},${w3},${times},I felt welcome here number ${id}.\n`;
    const file = path.join(workDir, 'datatools-cases.csv');
    fs.writeFileSync(file, csv, 'utf8');
    const mapping = {
      id_column: 'id',
      text_columns: ['answer'],
      attribute_columns: [
        { name: 'Dt site', level: 'nominal' }, { name: 'Dt score', level: 'ratio' }, { name: 'Dt seen', level: 'date' },
        { name: 'Dt w1', level: 'ratio' }, { name: 'Dt w2', level: 'ratio' }, { name: 'Dt w3', level: 'ratio' }, { name: 'Dt times', level: 'ratio' },
      ],
    };
    await rpc('import.run', { path: file, mapping });
    await page.reload();
    await open();
    for (const id of ['date_part', 'sample', 'aggregate']) await page.getByTestId(`ve-make-${id}`).waitFor();
    for (const id of ['ve-sort', 've-merge', 've-tables']) await page.getByTestId(id).waitFor();
    h.equal(await page.getByTestId('ve-make-date_part').innerText(), 'Part of a date…', 'the new kinds are named in the toolbar');
    await axe('the Data editor with the new tools');
  });

  await S('a part of a date is made as a variable with its months named, and a variable that is not dates is refused', async () => {
    await page.getByTestId('ve-make-date_part').click();
    const dlg = dialog(/Make a variable: Part of a date/);
    await dlg.waitFor();
    h.equal((await dlg.getByTestId('tf-source').locator('option').allInnerTexts()).filter((o) => o.startsWith('Dt')).join(','), 'Dt seen', 'only a variable of dates is offered');
    await dlg.getByTestId('tf-target').fill('Dt month');
    await dlg.getByTestId('tf-source').selectOption({ label: 'Dt seen' });
    await dlg.getByTestId('tf-part').selectOption('month');
    await preview(dlg);
    h.check(/6 cases get a value/.test(await text(dlg.getByTestId('tf-preview-summary'))), `the six respondents would get a value: ${await text(dlg.getByTestId('tf-preview-summary'))}`);
    await axe('the Make a variable window, a part of a date');
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    h.equal([await valueOf('D1', 'Dt month'), await valueOf('D2', 'Dt month'), await valueOf('D4', 'Dt month')], ['3', '12', '12'], 'March, December, December');
    const def = (await data()).variables.find((v) => v.name === 'Dt month');
    h.check(def.level === 'ordinal' && def.value_labels['12'] === 'December', `a month is ordinal and carries its name: ${JSON.stringify(def.value_labels)}`);
    // The week crosses the new year: 2020-12-31 is in week 53 and 2021-01-03 too.
    await rpc('variable.transform', { target: 'Dt week', kind: 'date_part', source: 'Dt seen', part: 'week' });
    h.equal([await valueOf('D3', 'Dt week'), await valueOf('D4', 'Dt week'), await valueOf('D2', 'Dt week')], ['53', '53', '1'], 'the ISO week runs across the new year');
  });

  await S('a random sample drawn within the groups is the same each time, and the dialog says what the seed is for', async () => {
    await page.getByTestId('ve-make-sample').click();
    const dlg = dialog(/Make a variable: Random sample/);
    await dlg.waitFor();
    await dlg.getByTestId('tf-target').fill('Dt pick');
    await dlg.getByTestId('tf-sample-by').selectOption('percent');
    await dlg.getByTestId('tf-sample-value').fill('50');
    await dlg.getByTestId('tf-within').selectOption({ label: 'Dt site' });
    await dlg.getByTestId('tf-seed').fill('11');
    await preview(dlg);
    h.check(/6 cases get a value/.test(await text(dlg.getByTestId('tf-preview-summary'))), 'the six respondents can be drawn, and the other cases of the project cannot (they have no site)');
    h.check((await text(dlg)).includes('same seed over the same cases always draws the same sample'), 'the dialog says what the seed is for');
    await axe('the Make a variable window, a random sample');
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    const pick = async () => ['D1', 'D2', 'D3', 'D4', 'D5', 'D6'].map(async (c) => valueOf(c, 'Dt pick'));
    const first = await Promise.all(await pick());
    h.equal([first.slice(0, 3).filter((v) => v === '1').length, first.slice(3).filter((v) => v === '1').length], [2, 2], 'half of each group is in the sample: two of three in each, rounded');
    await rpc('variable.recompute', { name: 'Dt pick' });
    h.equal(await Promise.all(await pick()), first, 'making it again draws the same cases');
  });

  await S('a group summary is written on every case of the group, and a count works on any variable', async () => {
    await page.getByTestId('ve-make-aggregate').click();
    const dlg = dialog(/Make a variable: Group summary/);
    await dlg.waitFor();
    await dlg.getByTestId('tf-target').fill('Dt site mean');
    await dlg.getByTestId('tf-source').selectOption({ label: 'Dt score' });
    await tick(dlg.getByTestId('tf-group-by'), 'Dt site');
    await preview(dlg);
    await axe('the Make a variable window, a group summary');
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    h.equal([await valueOf('D1', 'Dt site mean'), await valueOf('D3', 'Dt site mean'), await valueOf('D4', 'Dt site mean')], ['12', '12', '22'], 'each case has the mean of its site');
    await page.getByTestId('ve-make-aggregate').click();
    const again = dialog(/Make a variable: Group summary/);
    await again.getByTestId('tf-target').fill('Dt site seen');
    await again.getByTestId('tf-function').selectOption('count');
    await again.getByTestId('tf-source').selectOption({ label: 'Dt seen' });
    await tick(again.getByTestId('tf-group-by'), 'Dt site');
    await preview(again);
    await again.getByTestId('tf-make').click();
    await again.waitFor({ state: 'detached' });
    h.equal(await valueOf('D5', 'Dt site seen'), '3', 'how many of the site have a date: a count works on dates, which a mean would refuse');
    await page.getByRole('tab', { name: 'Variable view' }).click();
    h.check((await text(page.getByTestId('ve-variables'))).includes('mean of Dt score for each group of Dt site'), 'the variable view says how the summary was made');
    await page.getByRole('tab', { name: 'Data view' }).click();
  });

  await S('the cases are put in order by a variable without changing the project, and shown in their own order again on request', async () => {
    const newest = async () => JSON.stringify((await rpc('events.list', { limit: 1 }))[0] ?? null);
    const before = await newest();
    await page.getByTestId('ve-sort').click();
    const dlg = dialog('Sort the cases');
    await dlg.waitFor();
    await dlg.getByTestId('sort-var-0').selectOption({ label: 'Dt score' });
    await dlg.getByTestId('sort-dir-0').selectOption('down');
    await axe('the Sort the cases window');
    await dlg.getByTestId('sort-apply').click();
    await dlg.waitFor({ state: 'detached' });
    await page.getByTestId('ve-sorted').waitFor();
    h.check((await text(page.getByTestId('ve-sorted'))).includes('sorted by Dt score (high to low)'), 'the page says how the cases are shown');
    await page.waitForFunction(() => /D6/.test(document.querySelector('[data-testid=ve-grid] tbody tr')?.textContent ?? ''), null, { timeout: 8000 });
    h.check(true, 'the case with the highest score is the first row');
    h.equal(await newest(), before, 'sorting wrote nothing to the history');
    await page.getByTestId('ve-unsort').click();
    await page.waitForFunction(() => !/D6/.test(document.querySelector('[data-testid=ve-grid] tbody tr')?.textContent ?? ''), null, { timeout: 8000 });
    h.equal(await page.getByTestId('ve-sorted').count(), 0, 'the cases are in their own order again');
  });

  await S('a table with a row for each group is shown and saved, and the cases stood on end and laid across again are the same values', async () => {
    await page.getByTestId('ve-tables').click();
    const dlg = dialog('Tables by group, and restructuring');
    await dlg.waitFor();
    // A row for each group.
    await tick(dlg.getByTestId('tables-by'), 'Dt site');
    await dlg.getByTestId('tables-sum-var-0').selectOption({ label: 'Dt score' });
    await dlg.getByTestId('tables-show').click();
    await dlg.getByTestId('tables-result').waitFor();
    const groups = await text(dlg.getByTestId('tables-table'));
    h.check(/Dt site Cases mean of Dt score N 3 12 S 3 22/.test(groups), `the table has a row for each site with its mean: ${groups}`);
    await axe('the Tables and restructuring window, a table by group');
    const groupsFile = path.join(workDir, 'dt-groups.csv');
    await dlg.getByTestId('tables-save-path').fill(groupsFile);
    await dlg.getByTestId('tables-save').click();
    await dlg.getByTestId('tables-saved').waitFor();
    h.check(fs.readFileSync(groupsFile, 'utf8').includes('N,3,12'), 'the table is saved as a file other programs read');

    // The cases stood on end.
    await dlg.getByTestId('tables-op').selectOption('stand_on_end');
    await tick(dlg.getByTestId('tables-keep'), 'Case');
    for (const w of ['Dt w1', 'Dt w2', 'Dt w3']) await tick(dlg.getByTestId('tables-measures'), w);
    await dlg.getByTestId('tables-index-name').fill('Wave');
    await dlg.getByTestId('tables-value-name').fill('Score');
    await dlg.getByTestId('tables-drop-missing').click();
    await dlg.getByTestId('tables-show').click();
    await dlg.getByTestId('tables-result').waitFor();
    h.check(/17 rows/.test(await text(dlg.getByTestId('tables-summary'))), `six cases and three waves, less the one with no value: ${await text(dlg.getByTestId('tables-summary'))}`);
    const longFile = path.join(workDir, 'dt-long.csv');
    await dlg.getByTestId('tables-save-path').fill(longFile);
    await dlg.getByTestId('tables-save').click();
    await dlg.getByTestId('tables-saved').waitFor();
    const longText = fs.readFileSync(longFile, 'utf8');
    h.check(longText.startsWith('﻿Case,Wave,Score') && longText.includes('D3,Dt w3,9') && !longText.includes('D3,Dt w2'), 'the long file has a row for each wave, and none for the missing one');

    // And laid across again, from that file.
    await dlg.getByTestId('tables-op').selectOption('lay_across');
    await dlg.getByTestId('tables-from-file').click();
    await dlg.getByTestId('tables-file').fill(longFile);
    await dlg.getByTestId('tables-file-read').click();
    await dlg.getByTestId('tables-file-read-summary').waitFor();
    await tick(dlg.getByTestId('tables-id'), 'Case');
    await dlg.getByTestId('tables-index').selectOption('Wave');
    await tick(dlg.getByTestId('tables-values'), 'Score');
    await dlg.getByTestId('tables-show').click();
    await dlg.getByTestId('tables-result').waitFor();
    const wide = await text(dlg.getByTestId('tables-table'));
    h.check(/Case Dt w1 Dt w2 Dt w3/.test(wide) && /D3 7 9/.test(wide) && /D6 16 17 18/.test(wide), `laid across, a row for each case and a column for each wave: ${wide}`);
    await dlg.getByTestId('tables-close').click();
    await dlg.waitFor({ state: 'detached' });
  });

  await S('variables are merged in from a file by the name of the case, and the result says what matched and what did not', async () => {
    const extra = path.join(workDir, 'dt-extra.csv');
    fs.writeFileSync(extra, 'who,Dt age,Dt mood\nD1,31,up\nd2,40,down\nD4,52,up\nD9,99,up\n', 'utf8');
    await page.getByTestId('ve-merge').click();
    const dlg = dialog('Merge variables from a file');
    await dlg.waitFor();
    await dlg.getByTestId('merge-file').fill(extra);
    await dlg.getByTestId('merge-file-read').click();
    await dlg.getByTestId('merge-columns').waitFor();
    await dlg.getByTestId('merge-key').selectOption('who');
    await dlg.getByTestId('merge-include-Dt age').click();
    await dlg.getByTestId('merge-include-Dt mood').click();
    await axe('the Merge variables from a file window');
    await dlg.getByTestId('merge-go').click();
    await dlg.getByTestId('merge-result').waitFor();
    h.check(/3 cases had a row in the file/.test(await text(dlg.getByTestId('merge-matched'))), `three respondents matched, without regard to capitals: ${await text(dlg.getByTestId('merge-matched'))}`);
    h.check(/1 row of the file matches no case.*D9/.test(await text(dlg.getByTestId('merge-unmatched-rows'))), 'the row that matched no case is named');
    h.equal([await valueOf('D1', 'Dt age'), await valueOf('D2', 'Dt age'), await valueOf('D3', 'Dt age'), await valueOf('D4', 'Dt mood')], ['31', '40', null, 'up'], 'the values are in, and a case the file lacks has none');
    await dlg.getByTestId('merge-close').click();
    await dlg.waitFor({ state: 'detached' });
  });

  await S('Statistics can count each case by a weight, and runs an analysis for each group with a page for each', async () => {
    await page.getByRole('tab', { name: 'Statistics', exact: true }).click();
    await page.getByRole('button', { name: 'Statistics page', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Statistics', exact: true }).waitFor();
    await page.getByTestId('stats-goal-describe').check();
    await page.getByTestId('stats-role-x').selectOption({ label: 'Dt score (scale)' });
    h.check((await page.getByTestId('stats-weight').locator('option').allInnerTexts()).includes('Dt times'), 'a variable of numbers is offered as the weight');
    h.check((await page.getByTestId('stats-split').locator('option').allInnerTexts()).includes('Dt site'), 'a variable of categories is offered to split by');
    // Weighted by Dt times: D1 twice, D2 once, D3 once, D4 three times, D5 once, D6 not at all: 8 scores with the mean 16.
    await page.getByTestId('stats-weight').selectOption('Dt times');
    const before = await page.getByTestId('stats-page').count();
    await page.getByTestId('stats-run').click();
    await page.waitForFunction((n) => document.querySelectorAll('[data-testid=stats-page]').length > n, before, { timeout: 10000 });
    const weighted = page.getByTestId('stats-page').last();
    const line = await text(weighted.getByTestId('stats-page-weight'));
    // In the whole check the project also holds other cases, which have no weight, so the number of cases is not six there.
    h.check(/counted as many times as Dt times says: \d+ cases? count as 8\./.test(line) && /(1 case has|\d+ cases have) no weight or a weight of 0/.test(line), `the page says it was weighted: ${line}`);
    const table = await text(weighted.getByTestId('stats-table').first());
    h.check(/Cases with a value 8/.test(table) && /M 16\.00/.test(table), `eight scores count, with the mean 16: ${table}`);
    await axe('Statistics, a weighted analysis');

    // Split by site: a page for each of the two sites, kept in sections named for the groups.
    await page.getByTestId('stats-weight').selectOption('');
    await page.getByTestId('stats-split').selectOption('Dt site');
    const pages = await page.getByTestId('stats-page').count();
    await page.getByTestId('stats-run').click();
    await page.waitForFunction((n) => document.querySelectorAll('[data-testid=stats-page]').length >= n + 2, pages, { timeout: 15000 }).catch(() => {});
    const notebook = await rpc('stats.notebook', {});
    const split = notebook.filter((p) => p.kind === 'analysis' && /`Dt site` == /.test(p.request?.options?.filter ?? ''));
    h.equal(split.map((p) => p.section).sort(), ['Dt site: N', 'Dt site: S'], 'each group has its own section');
    h.equal(split.map((p) => p.result.n_used).sort(), [3, 3], 'and each page used the three cases of its group');
    await page.getByTestId('stats-split').selectOption('');
  });
}
