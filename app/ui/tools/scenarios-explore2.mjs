// Explore, second part: Crosstabs, percentages and month rows in the Code Matrix, date and range conditions in the Query Builder, and
// saved, frozen, and compared queries. The respondents are the check's own (six, with a group, a site, an age, and a visit date), so
// nothing depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';

const SURVEY = [
  'id,group,site,age,visit,answer',
  'R1,A,North,23,2024-02-10,I felt welcome here and the staff were kind.',
  'R2,A,South,31,2024-03-05,The cost was high for me and my family.',
  'R3,A,North,37,2024-03-20,Staff were warm and the place was easy to reach.',
  'R4,B,North,45,2024-04-11,It is far away and the bus is slow.',
  'R5,B,South,52,2024-05-02,I would come back because they listen.',
  'R6,,South,n/a,March 3rd,Nothing special to say about it.',
  '',
].join('\n');

export async function runExplore2(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const tab = (name) => page.getByRole('tablist', { name: 'Explore tools' }).getByRole('tab', { name });
  const cells = (locator) => locator.locator('tr').evaluateAll((rows) => rows.map((r) => [...r.querySelectorAll('th,td')].map((c) => c.textContent.replace(/\s+/g, ' ').trim())));
  const codeNamed = async (name) => (await rpc('code.list')).find((c) => c.name === name);
  const textOf = async (sourceId, start, end) => Array.from((await rpc('source.get', { id: sourceId })).text).slice(start, end).join('');

  async function openExplore(toolTab) {
    if (!(await page.getByRole('heading', { level: 2, name: 'Explore' }).isVisible())) {
      await page.getByRole('tab', { name: 'Explore', exact: true }).click();
      await page.getByRole('button', { name: 'Query', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Explore' }).waitFor();
    }
    await tab(toolTab).click();
  }

  let survey = null;
  /** Codes the first words that match a phrase in the survey, returning the coding. */
  async function code(phrase, codeId) {
    const text = (await rpc('source.get', { id: survey.id })).text;
    const start = Array.from(text.slice(0, text.indexOf(phrase))).length;
    return rpc('coding.apply', { source_id: survey.id, start, end: start + Array.from(phrase).length, code_id: codeId });
  }

  await S('Crosstabs counts cases by two case variables, with every case counted once and a place for the cases with no value', async () => {
    const file = path.join(workDir, 'x2-survey.csv');
    fs.writeFileSync(file, SURVEY, 'utf8');
    const mapping = {
      id_column: 'id',
      text_columns: ['answer'],
      attribute_columns: [
        { name: 'group', level: 'nominal' },
        { name: 'site', level: 'nominal' },
        { name: 'age', level: 'ratio' },
        { name: 'visit', level: 'date' },
      ],
    };
    await rpc('import.run', { path: file, mapping });
    survey = (await rpc('source.list')).find((s) => s.name.includes('x2-survey'));
    h.check(survey !== undefined, 'the survey is a document of the project');
    await page.reload();
    await openExplore('Crosstabs');
    await axe('Explore, Crosstabs before anything is chosen');
    const total = (await rpc('case.list')).length;
    const others = total - 6;
    await page.getByLabel('Rows', { exact: true }).selectOption('group');
    await page.getByLabel('Columns', { exact: true }).selectOption('site');
    const table = page.getByTestId('crosstab-table');
    await table.waitFor();
    const grid = await cells(table);
    h.equal(grid[0], ['group', 'North', 'South', '(no value)', 'Total'], 'the columns, with the cases that have no site last');
    h.equal(grid.slice(1, 4), [['A', '2', '1', '0', '3'], ['B', '1', '1', '0', '2'], ['(no value)', '0', '1', String(others), String(1 + others)]], 'the rows, with the respondent who gave no group and the cases of the sample that have none');
    h.equal(grid[4], ['Total', '3', '3', String(others), String(total)], 'the totals add up to every case');
    h.check(new RegExp(`Counting ${total} cases`).test(await page.getByTestId('crosstab-universe').innerText()), 'the page says how many cases it counts');
    h.check(/does not test whether the pattern is more than chance/.test(await page.getByTestId('crosstab-notes').innerText()), 'and that it does not claim a test');
    await page.screenshot({ path: `${ctx.shots}/explore-crosstab.png`, fullPage: true });
    await axe('Explore, Crosstabs');
  });

  await S('a crosstab shows percentages of the row, the column, or everything, says which, and opens the cases in a cell', async () => {
    await page.getByTestId('crosstab-percent').selectOption("row");
    const table = page.getByTestId('crosstab-table');
    await page.getByRole('button', { name: /^A, North: 2 cases, 66\.7% of 3/ }).waitFor();
    const grid = await cells(table);
    h.equal(grid[1], ['A', '2 (66.7%)', '1 (33.3%)', '0 (0%)', '3'], 'each cell of the row is a part of the row');
    h.check(/cases that have that value of group/.test(await page.getByTestId('crosstab-percent-note').innerText()), 'the sentence says what the whole is');
    await page.getByTestId('crosstab-percent').selectOption('column');
    await page.getByRole('button', { name: /^A, North: 2 cases, 66\.7% of 3/ }).waitFor();
    h.check(/that value of site/.test(await page.getByTestId('crosstab-percent-note').innerText()), 'and changes with the choice');
    await page.getByTestId('crosstab-percent').selectOption('none');
    await page.getByRole('button', { name: /^A, North: 2 cases\. Show these cases/ }).click();
    const list = page.getByTestId('crosstab-cases');
    await list.waitFor();
    h.equal((await list.locator('li').allInnerTexts()).map((t) => t.split('\n')[0].trim().split(' ')[0]), ['R1', 'R3'], 'the cell opens to the two respondents');
    h.equal(await list.getByRole('button', { name: /^Open the words of/ }).count(), 2, 'each with a way to its words');
    await axe('Explore, Crosstabs with the cases of a cell');
  });

  await S('numbers can be grouped into ranges, and a value that is not a number stays as it is', async () => {
    await page.getByLabel('Rows', { exact: true }).selectOption('age');
    await page.getByLabel('Group the row numbers into ranges this wide').fill('10');
    const table = page.getByTestId('crosstab-table');
    await page.getByRole('rowheader', { name: '20 to under 30' }).waitFor();
    const labels = (await cells(table)).slice(1, -1).map((r) => r[0]);
    h.equal(labels, ['20 to under 30', '30 to under 40', '40 to under 50', '50 to under 60', 'n/a', '(no value)'], 'ranges in number order, the word after the numbers, the cases with no value last');
    await page.getByLabel('Group the row numbers into ranges this wide').fill('');
    await page.getByRole('rowheader', { name: '23' }).waitFor();
    h.check(true, 'without a width every age is a row of its own');
    h.equal((await rpc('explore.crosstab', { row_variable: 'age', column_variable: 'site', row_width: 10 })).rows.values.length, 6, 'the engine agrees');
  });

  await S('the Code Matrix can have a row for each month, shows percentages that say what they are of, and puts back a choice that no longer makes sense', async () => {
    await tab('Code Matrix').click();
    await page.getByLabel('Rows', { exact: true }).selectOption('months');
    const table = page.getByTestId('matrix-table');
    await table.waitFor();
    const head = (await cells(table))[0];
    h.equal(head[0], 'Month coded', 'the rows are months');
    const months = (await cells(table)).slice(1, -1);
    h.check(months.length >= 1 && /^\d{4}-\d{2}$/.test(months[0][0]), `a row for each month with coding: ${months.map((r) => r[0]).join(', ')}`);
    const options = await page.getByTestId('matrix-percent').locator('option').allTextContents();
    h.equal(options, ['Numbers only', "Percent of the row's assignments", "Percent of the column's assignments", 'Percent of all the assignments shown'], 'with months as rows there is no group to take N of');
    await page.getByTestId('matrix-percent').selectOption('column');
    const first = months[0][0];
    await page.waitForFunction((m) => [...document.querySelectorAll('[data-testid=matrix-table] tbody tr')].some((r) => r.querySelector('th')?.textContent === m && /\(\d+(\.\d)?%\)/.test(r.textContent)), first);
    h.check(/column's total/.test(await page.getByTestId('matrix-percent-note').innerText()), 'the sentence says each is a part of its column');
    await page.getByLabel('Rows', { exact: true }).selectOption('var:group');
    await page.getByLabel('Metric', { exact: true }).selectOption('cases');
    const choices = await page.getByTestId('matrix-percent').locator('option').allTextContents();
    h.check(choices.includes('Percent of the cases in each group'), 'cases counted in groups of cases can be shown as a part of N');
    await page.getByTestId('matrix-percent').selectOption('group');
    h.check(/N: the number of cases that have that value of group/.test(await page.getByTestId('matrix-percent-note').innerText()), 'and the sentence says what N is');
    await axe('Explore, the Code Matrix with percentages');
    await page.getByLabel('Metric', { exact: true }).selectOption('assignments');
    h.equal(await page.getByTestId('matrix-percent').inputValue(), 'none', 'a percentage that needs N is put back to numbers when the metric changes');
    await page.getByLabel('Rows', { exact: true }).selectOption('cases');
  });

  await S('a query can ask for the day a coding was made and for a range of a case variable, and says what a limit leaves out', async () => {
    const cost = await codeNamed('Cost');
    await code('welcome', cost.id);
    await code('cost', cost.id);
    await tab('Query Builder').click();
    await page.getByRole('button', { name: 'Start again' }).click();
    await page.getByTestId('add-condition').click();
    await page.getByLabel('Condition 1: ingredient').selectOption('coded');
    await page.getByRole('textbox', { name: 'Condition 1: first day' }).fill('3/14/2024');
    h.equal(await page.getByTestId('run-query').isDisabled(), true, 'a day written month first is not run');
    h.check(/year-month-day/.test(await page.getByTestId('query-readback').innerText()), 'and the readback says how to write it');
    await page.getByRole('textbox', { name: 'Condition 1: first day' }).fill('2000-01-01');
    await page.getByRole('textbox', { name: 'Condition 1: last day' }).fill('2999-12-31');
    h.check(/the coding was made from 2000-01-01 to 2999-12-31/.test(await page.getByTestId('query-readback').innerText()), 'the readback tells it in words');
    await page.getByTestId('run-query').click();
    await page.getByTestId('query-count').waitFor();
    const everything = await rpc('explore.query', { rule: { type: 'group', match: 'any', items: [{ type: 'coded_between', from: '2000-01-01', to: '2999-12-31' }] }, result_unit: 'excerpts' });
    h.check(new RegExp(`^${everything.total} excerpts`).test((await page.getByTestId('query-count').innerText()).trim()), `every coding was made in that span: ${everything.total}`);

    await page.getByLabel('Condition 1: ingredient').selectOption('between');
    await page.getByLabel('Condition 1: variable').selectOption('visit');
    h.check(/5 cases of 6 with a value read as a date/.test(await page.getByTestId('between-help').innerText()), 'the page says how many visit dates it can read, so what a limit leaves out is known');
    await page.getByRole('textbox', { name: 'Condition 1: lowest value or first day' }).fill('2024-02-01');
    await page.getByRole('textbox', { name: 'Condition 1: highest value or last day' }).fill('2024-03-31');
    h.check(/the case's visit is from 2024-02-01 to 2024-03-31/.test(await page.getByTestId('query-readback').innerText()), 'the range is read back');
    await page.getByTestId('run-query').click();
    await page.waitForFunction(() => /^2 excerpts/.test(document.querySelector('[data-testid=query-count]')?.textContent?.trim() ?? ''));
    h.check(true, 'the two respondents visited in February and March, whose words were coded, are found');
    await page.getByLabel('Condition 1: variable').selectOption('age');
    await page.getByRole('textbox', { name: 'Condition 1: lowest value or first day' }).fill('30');
    await page.getByRole('textbox', { name: 'Condition 1: highest value or last day' }).fill('40');
    await page.getByTestId('run-query').click();
    await page.waitForFunction(() => /^1 excerpt/.test(document.querySelector('[data-testid=query-count]')?.textContent?.trim() ?? ''));
    h.check(true, 'an age from 30 to 40 finds one coded respondent');
    await page.getByRole('textbox', { name: 'Condition 1: lowest value or first day' }).fill('2024-01-01');
    h.equal(await page.getByTestId('run-query').isDisabled(), true, 'limits that mix a date and a number are not run');
    await axe('Explore, the Query Builder with a range of a case variable');
    await page.getByLabel('Condition 1: variable').selectOption('visit');
    await page.getByRole('textbox', { name: 'Condition 1: lowest value or first day' }).fill('2024-02-01');
    await page.getByRole('textbox', { name: 'Condition 1: highest value or last day' }).fill('2024-03-31');
  });

  await S('a query is saved under a name, frozen, compared with what the same question finds now, and opened again as it was frozen', async () => {
    await page.getByTestId('save-query').click();
    await page.getByTestId('query-name').fill('Spring visits');
    await page.getByTestId('query-name-save').click();
    await page.getByTestId('saved-note').waitFor();
    h.check(/Saved the query Spring visits\./.test(await page.getByTestId('saved-note').innerText()), 'the save is told');
    h.check(/The builder matches it\./.test(await page.getByTestId('saved-banner').innerText()), 'the builder says it is the saved query');
    const row = page.getByTestId('saved-row').filter({ hasText: 'Spring visits' });
    h.check(/Follows the coding/.test(await row.innerText()), 'a saved query follows the coding until it is frozen');
    h.equal((await rpc('query.list')).map((q) => q.name), ['Spring visits'], 'the engine keeps it');

    await page.getByRole('button', { name: 'Freeze Spring visits', exact: true }).click();
    await row.getByText(/Frozen: 2 codings/).waitFor();
    h.check(/Froze Spring visits: it found 2 codings/.test(await page.getByTestId('saved-note').innerText()), 'freezing says how many codings it kept');

    // The coding grows: a third respondent in range is coded.
    const cost = await codeNamed('Cost');
    await code('Staff were warm', cost.id);
    await page.getByRole('button', { name: /^Compare Spring visits with what the same question finds now/ }).click();
    const compare = page.getByTestId('saved-compare');
    await compare.waitFor();
    const said = await compare.innerText();
    h.check(/finds 3 codings now/.test(said) && /1 coding is found now and not then/.test(said) && /2 codings are found both times/.test(said), `the comparison is in plain words: ${said.replace(/\s+/g, ' ').slice(0, 260)}`);
    await axe('Explore, a frozen saved query compared with now');

    await page.getByRole('button', { name: 'Open the saved query Spring visits' }).click();
    await page.waitForFunction(() => /^2 excerpts/.test(document.querySelector('[data-testid=query-count]')?.textContent?.trim() ?? ''));
    h.check(/frozen/.test(await page.getByTestId('saved-banner').innerText()), 'opened again, it says it is frozen');
    h.check(true, 'and shows the two codings it found, not the third');
    await page.getByTestId('run-live').click();
    await page.waitForFunction(() => /^3 excerpts/.test(document.querySelector('[data-testid=query-count]')?.textContent?.trim() ?? ''));
    h.check(true, 'asking the question now finds the third, and the frozen answer stays');
  });

  await S('a frozen query cannot be changed, an unfrozen one can be updated, renamed, deleted, and the deletion taken back', async () => {
    await page.getByTestId('add-condition').click();
    h.equal(await page.getByTestId('update-query').isDisabled(), true, 'a frozen query is not updated with changes');
    h.check(/changes that are not saved in it/.test(await page.getByTestId('saved-banner').innerText()), 'the banner says the builder has changes');
    h.equal(await page.getByTestId('save-query').isDisabled(), true, 'a half-made condition cannot be saved either, and says why');
    await page.getByRole('button', { name: 'Remove condition 2' }).click();

    await page.getByRole('button', { name: 'Unfreeze Spring visits' }).click();
    const row = page.getByTestId('saved-row').filter({ hasText: 'Spring visits' });
    await row.getByText('Follows the coding').waitFor();
    // A change to the draft can now be put in the saved query.
    await page.getByLabel('Condition 1: variable').selectOption('age');
    await page.getByRole('textbox', { name: 'Condition 1: lowest value or first day' }).fill('20');
    await page.getByRole('textbox', { name: 'Condition 1: highest value or last day' }).fill('60');
    h.equal(await page.getByTestId('update-query').isDisabled(), false, 'an unfrozen query can be updated');
    await page.getByTestId('update-query').click();
    await page.getByText(/The builder matches it\./).waitFor();
    const saved = (await rpc('query.list'))[0];
    const got = await rpc('query.get', { id: saved.id });
    h.equal(got.spec.rule.items[0].name, 'age', 'the engine has the changed question');

    await page.getByRole('button', { name: 'Rename Spring visits', exact: true }).click();
    await page.getByTestId('query-name').fill('Everyone aged 20 to 60');
    await page.getByTestId('query-name-save').click();
    await page.getByTestId('saved-row').filter({ hasText: 'Everyone aged 20 to 60' }).waitFor();
    h.equal((await rpc('query.list')).map((q) => q.name), ['Everyone aged 20 to 60'], 'renamed');

    await page.getByRole('button', { name: 'Delete Everyone aged 20 to 60' }).click();
    await page.getByTestId('saved-none').waitFor();
    h.equal((await rpc('query.list')).length, 0, 'deleted');
    await page.getByTestId('saved-undo').click();
    await page.getByTestId('saved-row').filter({ hasText: 'Everyone aged 20 to 60' }).waitFor();
    h.equal((await rpc('query.list')).map((q) => q.name), ['Everyone aged 20 to 60'], 'one undo brings it back');
    await page.getByRole('button', { name: 'Start again' }).click();
    await axe('Explore, the Query Builder with a saved query');
  });
}
