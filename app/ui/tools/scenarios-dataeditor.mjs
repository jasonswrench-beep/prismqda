// The data editor: the variables of cases and their values in a grid edited with the keyboard, the variable view, and the dialogs that
// make new variables from others (a formula, a recoding, z-scores, ranks, ranges) with the effect shown before anything is saved. The
// cases and variables are the check's own (eight invented respondents with variable names no earlier step uses), and every number
// below was worked out by hand.
//
//   Ed group:  A A A A  B B B B
//   Ed score:  10 12 11 13  15 17 16 18
//   Ed age:    21 25 33 41  29 38 44 52

import fs from 'node:fs';
import path from 'node:path';

const ROWS = [
  ['E1', 'A', 10, 21], ['E2', 'A', 12, 25], ['E3', 'A', 11, 33], ['E4', 'A', 13, 41],
  ['E5', 'B', 15, 29], ['E6', 'B', 17, 38], ['E7', 'B', 16, 44], ['E8', 'B', 18, 52],
];

export async function runDataEditor(ctx, api) {
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
  const cell = async (caseName, variable) => {
    const d = await data();
    const r = d.cases.findIndex((c) => c.name === caseName);
    const c = d.variables.findIndex((v) => v.name === variable);
    return page.locator(`[data-testid=ve-grid] td[data-r="${r}"][data-c="${c}"]`);
  };
  const active = () => page.evaluate(() => {
    const el = document.activeElement;
    return { r: el?.dataset?.r ?? null, c: el?.dataset?.c ?? null, tag: el?.tagName ?? null };
  });
  const tab = (name) => page.getByRole('tab', { name, exact: true });
  const toVariables = async () => {
    await tab('Variable view').click();
    await page.getByTestId('ve-variables').waitFor();
  };
  const toData = async () => {
    await tab('Data view').click();
    await page.getByTestId('ve-grid').waitFor();
  };
  const dialog = (name) => page.getByRole('dialog', { name });
  const preview = async (dlg) => {
    await dlg.locator('[data-testid=tf-preview-summary],[data-testid=tf-preview-error]').first().waitFor({ timeout: 8000 });
  };

  await S('the Data editor opens from the Statistics tab and lists the variables of cases, with the level of each', async () => {
    let csv = 'id,Ed group,Ed score,Ed age,answer\n';
    for (const [id, group, score, age] of ROWS) csv += `${id},${group},${score},${age},I felt welcome here number ${id}.\n`;
    const file = path.join(workDir, 'editor-cases.csv');
    fs.writeFileSync(file, csv, 'utf8');
    const mapping = { id_column: 'id', text_columns: ['answer'], attribute_columns: [
      { name: 'Ed group', level: 'nominal' }, { name: 'Ed score', level: 'ratio' }, { name: 'Ed age', level: 'ratio' }] };
    await rpc('import.run', { path: file, mapping });
    await page.reload();
    await open();
    const grid = page.getByTestId('ve-grid');
    await grid.waitFor();
    const d = await data();
    h.equal(await grid.getAttribute('aria-rowcount'), String(d.cases.length + 1), 'the grid says how many rows it has, the header included');
    h.equal(await grid.getAttribute('aria-colcount'), String(d.variables.length + 1), 'and how many columns, the case name included');
    const heads = await text(grid.locator('thead'));
    h.check(heads.includes('Ed score') && heads.includes('Numbers with a true zero (ratio)') && heads.includes('Ed group') && heads.includes('Categories (nominal)'), `each column says its name and its level: ${heads}`);
    h.check((await text(page.getByTestId('ve-range'))).length > 0, 'it says how many cases there are');
    h.check(await page.getByRole('toolbar', { name: 'Make and change variables' }).isVisible(), 'the tools for making variables are a toolbar');
    await axe('the Data editor, data view');
  });

  await S('the grid is one tab stop with the arrow keys moving inside it, and the active cell is plainly marked', async () => {
    const first = await cell('E1', 'Ed score');
    await first.click();
    h.equal(await page.locator('[data-testid=ve-grid] [tabindex="0"]').count(), 1, 'one cell is in the tab order, the active one');
    const start = await active();
    await page.keyboard.press('ArrowDown');
    const down = await active();
    h.equal(Number(down.r), Number(start.r) + 1, 'Arrow Down moves to the next case');
    await page.keyboard.press('ArrowRight');
    h.equal(Number((await active()).c), Number(start.c) + 1, 'Arrow Right moves to the next variable');
    await page.keyboard.press('Home');
    h.equal(Number((await active()).c), 0, 'Home goes to the first variable of the row');
    await page.keyboard.press('Control+End');
    const d = await data();
    const end = await active();
    h.equal([Number(end.r), Number(end.c)], [d.cases.length - 1, d.variables.length - 1], 'Control+End goes to the last cell');
    await page.keyboard.press('Control+Home');
    h.equal([(await active()).r, (await active()).c], ['0', '0'], 'Control+Home goes to the first');
    const look = await page.evaluate(() => {
      const el = document.activeElement;
      const cs = getComputedStyle(el);
      return { outline: cs.outlineStyle, width: parseFloat(cs.outlineWidth) };
    });
    h.check(look.outline !== 'none' && look.width >= 2, `the focused cell has a visible outline: ${JSON.stringify(look)}`);
    await page.keyboard.press('Tab');
    h.check(await page.evaluate(() => !document.activeElement?.closest('[data-testid=ve-grid]')), 'Tab leaves the grid, so the keyboard is not trapped in it');
    await page.keyboard.press('Shift+Tab');
    h.check(await page.evaluate(() => !!document.activeElement?.closest('[data-testid=ve-grid]')), 'and Shift+Tab comes back into it');
  });

  await S('Enter changes a cell and saves it as one step, Escape puts the old value back, and Delete clears the cell', async () => {
    const c = await cell('E1', 'Ed score');
    await c.click();
    await page.keyboard.press('Enter');
    const input = page.getByTestId('ve-input');
    await input.waitFor();
    h.equal(await input.inputValue(), '10', 'the box holds the old value');
    h.check((await input.getAttribute('aria-label')).includes('Ed score') && (await input.getAttribute('aria-label')).includes('E1'), 'and is named by the variable and the case');
    await axe('the Data editor, a cell being changed');
    await input.fill('99.5');
    await page.keyboard.press('Escape');
    await input.waitFor({ state: 'detached' });
    h.equal(await valueOf('E1', 'Ed score'), '10', 'Escape leaves the value as it was');
    h.equal((await active()).r, (await cell('E1', 'Ed score').then((x) => x.getAttribute('data-r'))), 'and the focus is back on the cell');

    const newest = async () => (await rpc('events.list', { limit: 1 }))[0]?.id ?? 0;
    const lastEvent = await newest();
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').fill('20');
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').waitFor({ state: 'detached' });
    h.equal(await valueOf('E1', 'Ed score'), '20', 'Enter saves the new value');
    const added = (await rpc('events.list', { limit: 10 })).filter((e) => e.id > lastEvent);
    h.check(added.length === 1 && added[0].action === 'variable.apply', `as one step in the history: ${added.map((e) => e.action).join(', ')}`);
    const d = await data();
    h.equal(Number((await active()).r), d.cases.findIndex((x) => x.name === 'E1') + 1, 'and the focus moves down to the next case');

    await (await cell('E1', 'Ed score')).click();
    await page.keyboard.press('Delete');
    await page.waitForTimeout(400);
    h.equal(await valueOf('E1', 'Ed score'), null, 'Delete clears the cell');
    await page.getByTestId('ve-undo').click();
    await page.waitForTimeout(400);
    h.equal(await valueOf('E1', 'Ed score'), '20', 'Undo puts the cleared value back');
    await page.getByTestId('ve-undo').click();
    await page.waitForTimeout(400);
    h.equal(await valueOf('E1', 'Ed score'), '10', 'and another Undo puts the first value back');
    h.equal(await text(await cell('E1', 'Ed score')), '10', 'the grid shows it');
  });

  await S('a letter typed on a number cell starts a change, and a value the variable cannot hold stays in the box with the reason', async () => {
    const c = await cell('E2', 'Ed score');
    await c.click();
    await page.keyboard.press('x');
    const input = page.getByTestId('ve-input');
    await input.waitFor();
    h.equal(await input.inputValue(), 'x', 'typing starts a change with the letter typed');
    await page.keyboard.press('Enter');
    // the notice on the page (the announcement of it is a second alert for a moment, so the page's own is chosen)
    const alert = page.locator('[role=alert].notice').filter({ hasText: 'takes a number' });
    await alert.waitFor();
    h.check((await text(alert)).includes("'x' is not one"), `the reason is said in words: ${await text(alert)}`);
    h.equal(await valueOf('E2', 'Ed score'), '12', 'and nothing was saved');
    h.check(await input.isVisible(), 'the box stays open to be put right');
    await axe('the Data editor, a value refused');
    await page.keyboard.press('Escape');
    await input.waitFor({ state: 'detached' });
  });

  await S('the variable view lists each variable, and one can be changed: value labels show their words and missing values are marked without color', async () => {
    await toVariables();
    const rows = page.getByTestId('ve-variable');
    h.check((await rows.count()) >= 3, 'the variables are listed');
    const row = rows.filter({ hasText: 'Ed score' });
    h.check((await text(row)).includes('Typed in or imported') && (await text(row)).includes('Only empty values'), `a variable says how it was made and what counts as missing: ${await text(row)}`);
    await axe('the Data editor, variable view');
    await row.getByTestId('ve-edit').click();
    const dlg = dialog(/Change the variable Ed score/);
    await dlg.waitFor();
    await axe('the Change a variable window');
    await dlg.getByTestId('var-labels').fill('not a label line');
    await dlg.getByTestId('var-save').click();
    await dlg.getByRole('alert').filter({ hasText: 'Line 1 has no equals sign' }).waitFor();
    h.check(true, 'a label line without an equals sign is refused, naming the line');
    await dlg.getByTestId('var-labels').fill('');
    await dlg.getByTestId('var-missing').fill('1, 2, 3, 4');
    await dlg.getByTestId('var-save').click();
    await dlg.getByRole('alert').filter({ hasText: 'At most 3' }).waitFor();
    h.check(true, 'four missing values are refused');
    await dlg.getByTestId('var-missing').fill('99');
    await dlg.getByTestId('var-save').click();
    await dlg.waitFor({ state: 'detached' });
    const def = (await data()).variables.find((v) => v.name === 'Ed score');
    h.equal(def.missing_codes, ['99'], 'the missing value is saved');

    await rows.filter({ hasText: 'Ed group' }).getByTestId('ve-edit').click();
    const dlg2 = dialog(/Change the variable Ed group/);
    await dlg2.getByTestId('var-labels').fill('A = Alpha\nB = Beta');
    await dlg2.getByTestId('var-save').click();
    await dlg2.waitFor({ state: 'detached' });
    h.equal((await data()).variables.find((v) => v.name === 'Ed group').value_labels, { A: 'Alpha', B: 'Beta' }, 'value labels are saved');

    await toData();
    h.equal(await text(await cell('E1', 'Ed group')), 'Alpha', 'the grid shows the words and keeps the value');
    h.equal(await valueOf('E1', 'Ed group'), 'A', 'the stored value is still A');
    h.equal(await (await cell('E1', 'Ed group')).getAttribute('aria-label'), 'Alpha, value A', 'and a screen reader hears both');
    // A value that counts as missing is italic with a dotted underline, and is said to be missing.
    await (await cell('E3', 'Ed score')).click();
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').fill('99');
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').waitFor({ state: 'detached' });
    const miss = await cell('E3', 'Ed score');
    const style = await miss.evaluate((el) => ({ italic: getComputedStyle(el).fontStyle, line: getComputedStyle(el).textDecorationLine, label: el.getAttribute('aria-label') }));
    h.check(style.italic === 'italic' && style.line.includes('underline'), `a missing value is marked by more than color: ${JSON.stringify(style)}`);
    h.check(style.label.includes('counted as missing'), `and is said to be missing: ${style.label}`);
    await page.getByTestId('ve-undo').click();
    await page.waitForTimeout(400);
    h.equal(await valueOf('E3', 'Ed score'), '11', 'Undo takes the 99 back out');
    await axe('the Data editor, with labels and a missing value');
  });

  await S('a variable can be added and deleted, and Undo brings it back with its values', async () => {
    await page.getByTestId('ve-add').click();
    const dlg = dialog('Add a variable');
    await dlg.waitFor();
    await dlg.getByTestId('var-save').click();
    await dlg.getByRole('alert').filter({ hasText: 'Give the variable a name' }).waitFor();
    await dlg.getByTestId('var-name').fill('Ed score');
    await dlg.getByTestId('var-save').click();
    await dlg.getByRole('alert').filter({ hasText: /already|in use|exists/i }).waitFor();
    h.check(true, 'a name already in use is refused in words');
    await dlg.getByTestId('var-name').fill('Ed note');
    await dlg.getByTestId('var-level').selectOption('text');
    await dlg.getByTestId('var-save').click();
    await dlg.waitFor({ state: 'detached' });
    h.check((await data()).variables.some((v) => v.name === 'Ed note' && v.level === 'text'), 'the variable exists, at the level chosen');
    await (await cell('E1', 'Ed note')).click();
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').fill('said it twice');
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').waitFor({ state: 'detached' });
    h.equal(await valueOf('E1', 'Ed note'), 'said it twice', 'a text value is kept');

    await toVariables();
    await page.getByTestId('ve-variable').filter({ hasText: 'Ed note' }).getByTestId('ve-delete').click();
    const del = page.getByRole('alertdialog', { name: /Delete the variable Ed note/ });
    await del.waitFor();
    await axe('the Delete a variable window');
    await del.getByTestId('var-delete-confirm').click();
    await del.waitFor({ state: 'detached' });
    h.check(!(await data()).variables.some((v) => v.name === 'Ed note'), 'it is deleted');
    await page.getByTestId('ve-undo').click();
    await page.waitForTimeout(400);
    h.equal(await valueOf('E1', 'Ed note'), 'said it twice', 'Undo brings the variable back with its values');
    await toData();
  });

  await S('Compute shows what it would make before it makes it, says what is wrong with a formula, and asks before it replaces a variable', async () => {
    await page.getByTestId('ve-make-compute').click();
    const dlg = dialog(/Make a variable: Compute/);
    await dlg.waitFor();
    await dlg.getByTestId('tf-target').fill('Ed total');
    await dlg.getByRole('group', { name: 'Insert the name of a variable' }).getByRole('button', { name: 'Ed score', exact: true }).click();
    await page.waitForFunction(() => document.activeElement?.id === 'tf-formula');
    await page.keyboard.type(' + ');
    await dlg.getByRole('group', { name: 'Insert the name of a variable' }).getByRole('button', { name: 'Ed age', exact: true }).click();
    const formula = await dlg.getByTestId('tf-formula').inputValue();
    h.equal(formula, '`Ed score` + `Ed age`', `a name with a space is put in backticks: ${formula}`);
    await preview(dlg);
    const summary = await text(dlg.getByTestId('tf-preview-summary'));
    h.check(/8 cases get a value/.test(summary) && /Level: Numbers with equal steps/.test(summary), `the effect is shown first, with the level it would have: ${summary}`);
    h.check(!(await data()).variables.some((v) => v.name === 'Ed total'), 'and nothing is saved by looking');
    await dlg.getByTestId('tf-formula').fill('`Ed score` + nothing');
    await dlg.getByTestId('tf-preview-error').waitFor({ timeout: 8000 });
    h.check((await text(dlg.getByTestId('tf-preview-error'))).includes('nothing'), `a formula that cannot work says why: ${await text(dlg.getByTestId('tf-preview-error'))}`);
    h.equal(await dlg.getByTestId('tf-make').isDisabled(), true, 'and the variable cannot be made');
    await axe('the Make a variable window, a formula with a problem');
    await dlg.getByTestId('tf-formula').fill('`Ed score` + `Ed age`');
    await preview(dlg);
    await dlg.getByTestId('tf-preview-summary').waitFor();
    await axe('the Make a variable window, with its preview');
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    h.equal(await valueOf('E1', 'Ed total'), '31', 'the new variable has 10 + 21 for the first case');
    h.equal(await valueOf('E8', 'Ed total'), '70', 'and 18 + 52 for the last');
    const made = (await data()).variables.find((v) => v.name === 'Ed total');
    h.check(made.level === 'interval' && made.derived?.transform?.kind === 'compute', 'it keeps how it was made');
    h.check((await text(page.getByTestId('ve-grid').locator('thead'))).includes('Ed total'), 'and appears as a column');

    await page.getByTestId('ve-make-compute').click();
    const again = dialog(/Make a variable: Compute/);
    await again.getByTestId('tf-target').fill('ed total');
    await again.getByTestId('tf-formula').fill('1');
    await preview(again);
    h.equal(await again.getByTestId('tf-make').isDisabled(), true, 'a name already in use cannot be made over by accident');
    await again.getByTestId('tf-replace').click();
    h.equal(await again.getByTestId('tf-make').isDisabled(), false, 'unless the person ticks that it replaces the old one');
    await again.getByRole('button', { name: 'Cancel' }).click();
    await again.waitFor({ state: 'detached' });
    h.equal(await valueOf('E1', 'Ed total'), '31', 'cancelling changes nothing');
  });

  await S('Recode, Standardize, Rank, and Cut each make a variable the engine can show, and each is one step to undo', async () => {
    // Recode: A becomes Team Alpha, and a value no rule names stays.
    await page.getByTestId('ve-make-recode').click();
    let dlg = dialog(/Make a variable: Recode/);
    await dlg.waitFor();
    await dlg.getByTestId('tf-target').fill('Ed team');
    await dlg.getByTestId('tf-source').selectOption({ label: 'Ed group' });
    await dlg.getByLabel('Rule 1: these values').fill('A');
    await dlg.getByLabel('Rule 1: become').fill('Team Alpha');
    await preview(dlg);
    await axe('the Make a variable window, a recoding');
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    h.equal([await valueOf('E1', 'Ed team'), await valueOf('E5', 'Ed team')], ['Team Alpha', 'B'], 'a recoding changes the values named and keeps the others');

    // Standardize.
    await page.getByTestId('ve-make-standardize').click();
    dlg = dialog(/Make a variable: Standardize/);
    await dlg.getByTestId('tf-target').fill('Ed score z');
    await dlg.getByTestId('tf-source').selectOption({ label: 'Ed score' });
    await preview(dlg);
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    const z = (await data());
    const zc = z.variables.findIndex((v) => v.name === 'Ed score z');
    const zs = z.values.map((row) => row[zc]).filter((v) => v !== null).map(Number);
    h.equal(zs.length, 8, 'every respondent has a z-score');
    h.check(Math.abs(zs.reduce((a, b) => a + b, 0)) < 1e-9, 'which add to zero');

    // Rank, reached by changing the kind inside an open dialog; the highest score is place 1.
    await page.getByTestId('ve-make-compute').click();
    dlg = dialog(/Make a variable: Compute/);
    await dlg.getByTestId('tf-kind').selectOption('rank');
    dlg = page.getByRole('dialog');
    await dlg.getByRole('heading', { name: 'Make a variable: Rank' }).waitFor();
    await dlg.getByTestId('tf-target').fill('Ed place');
    await dlg.getByTestId('tf-source').selectOption({ label: 'Ed age' });
    await dlg.getByTestId('tf-descending').click();
    await preview(dlg);
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    h.equal([await valueOf('E8', 'Ed place'), await valueOf('E1', 'Ed place')], ['1', '8'], 'ranks run from the highest age, which is place 1, to the lowest');

    // Cut.
    await page.getByTestId('ve-make-cut').click();
    dlg = dialog(/Make a variable: Cut into ranges/);
    await dlg.getByTestId('tf-target').fill('Ed age band');
    await dlg.getByTestId('tf-source').selectOption({ label: 'Ed age' });
    await dlg.getByTestId('tf-cuts').fill('20, 30, 40, 60');
    await preview(dlg);
    h.check((await text(dlg.getByTestId('tf-preview'))).includes('[20, 30]'), 'the ranges are named in the preview');
    await axe('the Make a variable window, a cut');
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });
    const band = (await data()).variables.find((v) => v.name === 'Ed age band');
    h.check(band.level === 'ordinal' && band.value_labels['1'] === '[20, 30]' && band.value_labels['3'] === '(40, 60]', `a cut is ordinal and its ranges are its labels: ${JSON.stringify(band.value_labels)}`);
    h.equal([await valueOf('E1', 'Ed age band'), await valueOf('E4', 'Ed age band'), await valueOf('E8', 'Ed age band')], ['1', '3', '3'], 'ages 21, 41, and 52 fall in ranges 1, 3, and 3');
    h.equal(await text(await cell('E1', 'Ed age band')), '[20, 30]', 'the grid shows the range, not its number');

    // One step each.
    const before = (await data()).variables.length;
    await page.getByTestId('ve-undo').click();
    await page.waitForTimeout(400);
    h.equal((await data()).variables.length, before - 1, 'one Undo takes the whole new variable away');
  });

  await S('Make it again brings a made variable up to date after the variables it came from changed', async () => {
    await (await cell('E1', 'Ed score')).click();
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').fill('20');
    await page.keyboard.press('Enter');
    await page.getByTestId('ve-input').waitFor({ state: 'detached' });
    h.equal(await valueOf('E1', 'Ed total'), '31', 'a made variable does not change behind the person\'s back');
    await toVariables();
    const row = page.getByTestId('ve-variable').filter({ hasText: 'Ed total' });
    h.check((await text(row)).includes('Computed: `Ed score` + `Ed age`'), `it says how it was made: ${await text(row)}`);
    h.equal(await page.getByTestId('ve-variable').filter({ hasText: 'Ed note' }).getByTestId('ve-recompute').count(), 0, 'a typed variable has no Make it again');
    await row.getByTestId('ve-recompute').click();
    await page.waitForTimeout(500);
    h.equal(await valueOf('E1', 'Ed total'), '41', 'it is 20 + 21 now');
    await toData();
  });

  await S('Statistics can be limited by a rule: the rule is checked first, a rule that is not true or false is refused, and the page says which cases it used', async () => {
    await page.getByTestId('ve-to-stats').click();
    await page.getByRole('heading', { level: 2, name: 'Statistics', exact: true }).waitFor();
    await page.getByTestId('stats-goal-describe').check();
    await page.getByTestId('stats-role-x').selectOption({ label: 'Ed score (scale)' });
    const rule = page.getByTestId('stats-filter-rule');
    await rule.fill('`Ed group` == "A"');
    await page.getByTestId('stats-filter-check').click();
    await page.getByTestId('stats-filter-result').filter({ hasText: /picks 4 of \d+ cases/ }).waitFor();
    h.check(true, 'the rule is checked first, with how many cases it picks');
    await rule.fill('`Ed score` * 2');
    await page.getByTestId('stats-filter-check').click();
    await page.getByTestId('stats-filter-result').filter({ hasText: 'true or false' }).waitFor();
    h.check(true, 'a rule that gives numbers is refused in words');
    await rule.fill('`Ed group` == "Z"');
    await page.getByTestId('stats-filter-check').click();
    await page.getByTestId('stats-filter-result').filter({ hasText: 'picks no cases' }).waitFor();
    h.check(true, 'and a rule that picks nobody is said so');
    await rule.fill('`Ed group` == "A"');
    const before = await page.getByTestId('stats-page').count();
    await page.getByTestId('stats-run').click();
    await page.waitForFunction((n) => document.querySelectorAll('[data-testid=stats-page]').length > n, before, { timeout: 10000 });
    const pg = page.getByTestId('stats-page').last();
    h.check((await text(pg.getByTestId('stats-page-filter'))).includes('Only cases where: `Ed group` == "A"'), 'the page says which rule was used');
    const t = await text(pg.getByTestId('stats-table').first());
    h.check(t.includes('Cases with a value 4') && t.includes('M 14.00'), `and the numbers are of those four cases (20, 12, 11, 13): ${t}`);
    await axe('Statistics, a page limited by a rule');
    await page.getByTestId('stats-filter-rule').fill('');
  });

  await S('the data view reflows in a narrow window, and the history words what the editor did', async () => {
    await open();
    const before = page.viewportSize();
    await page.setViewportSize({ width: 360, height: 740 });
    await page.waitForTimeout(400);
    const flow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    h.check(flow <= 1, `the page does not scroll sideways at 360 px wide: ${flow}`);
    await axe('the Data editor in a narrow window');
    await page.setViewportSize(before);
    const events = await rpc('events.list', { limit: 200 });
    h.check(events.some((e) => e.action === 'variable.apply'), 'the editor wrote its steps to the history');
  });

  await S('at 150 percent text the grid keeps its cells reachable and the Make a variable window fits the screen, scrolling inside itself', async () => {
    const gear = page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true });
    await gear.click();
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^150 percent/ }) }).click();
    await open();
    await page.getByTestId('ve-grid').waitFor();
    const flow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    h.check(flow <= 1, `the page does not scroll sideways at 150 percent: ${flow}`);
    await (await cell('E2', 'Ed score')).click();
    await page.keyboard.press('ArrowRight');
    const seen = await page.evaluate(() => { const r = document.activeElement.getBoundingClientRect(); const box = document.querySelector('.ve-scroll').getBoundingClientRect(); return { cell: [r.left, r.right], box: [box.left, box.right], ok: r.width > 0 && r.right <= box.right + 1 && r.left >= box.left - 1 }; });
    h.check(seen.ok, `moving to a cell keeps it inside the scrolling box: ${JSON.stringify(seen)}`);
    await axe('the Data editor at 150 percent text');
    await page.getByTestId('ve-make-compute').click();
    const dlg = dialog(/Make a variable: Compute/);
    await dlg.waitFor();
    const fit = await dlg.evaluate((el) => { const b = el.getBoundingClientRect(); return { bottom: b.bottom, top: b.top, h: window.innerHeight, scrolls: el.scrollHeight > el.clientHeight, overflow: getComputedStyle(el).overflowY }; });
    h.check(fit.top >= -1 && fit.bottom <= fit.h + 1, `the window is inside the screen: ${JSON.stringify(fit)}`);
    h.check(!fit.scrolls || fit.overflow === 'auto' || fit.overflow === 'scroll', 'and if it is taller than the screen it scrolls inside itself');
    await axe('the Make a variable window at 150 percent text');
    await dlg.getByRole('button', { name: 'Cancel' }).click();
    await dlg.waitFor({ state: 'detached' });
    await gear.click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^100 percent/ }) }).click();
    await open();
  });

  await S('a long list of cases is shown a page at a time, with the row numbers of the whole table', async () => {
    let csv = 'id,Ed rows,answer\n';
    for (let i = 1; i <= 120; i++) csv += `Q${String(i).padStart(3, '0')},${i},I felt welcome here number ${i}.\n`;
    const file = path.join(workDir, 'editor-many.csv');
    fs.writeFileSync(file, csv, 'utf8');
    await rpc('import.run', { path: file, mapping: { id_column: 'id', text_columns: ['answer'], attribute_columns: [{ name: 'Ed rows', level: 'ratio' }] } });
    await page.reload();
    await open();
    const total = (await data()).cases.length;
    h.equal(await text(page.getByTestId('ve-range')), `Cases 1 to 100 of ${total}`, 'the first hundred cases are shown, and how many there are');
    h.equal(await page.locator('[data-testid=ve-grid] tbody tr').count(), 100, 'a hundred rows are drawn, not all of them');
    await page.getByTestId('ve-next').click();
    await page.waitForFunction((n) => document.querySelector('[data-testid=ve-range]')?.textContent?.startsWith('Cases 101 to'), total);
    h.equal(await text(page.getByTestId('ve-range')), `Cases 101 to ${total} of ${total}`, 'the next page continues');
    h.equal(await page.locator('[data-testid=ve-grid] tbody tr').first().getAttribute('aria-rowindex'), '102', 'its first row is row 102 of the whole table, the header being row 1');
    h.equal(await page.getByTestId('ve-next').isDisabled(), true, 'and there is nothing after the last page');
    await axe('the Data editor, the second page of cases');
    await page.getByTestId('ve-prev').click();
    await page.waitForFunction(() => document.querySelector('[data-testid=ve-range]')?.textContent?.startsWith('Cases 1 to'));
    h.check(true, 'Earlier cases goes back');
  });
}
