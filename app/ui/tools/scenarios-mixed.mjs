// Mixed Methods: the coding becomes variables of cases (whether a case has a code, how many excerpts carry it, which of several codes it has
// most), and the values of a variable of cases become codes that code each case's text. The cases, codes and codings are the check's own
// (eight invented respondents; names no earlier step uses), and every number below was worked out by hand.
//
//   The answers come in as one document with a unit for each respondent; each answer reads "I felt welcome here number M1." (30 characters).
//   Mx Welcome:  M1 twice (0-14, 15-19), M2, M5, M7, M8 once      -> counts 2 1 0 0 1 0 1 1
//   Mx Doubt:    M1, M2, M3, M4 once, M7 twice                     -> counts 1 1 1 1 0 0 2 0
//   Most used of the two:  M1 Welcome, M2 both (a tie), M3 Doubt, M4 Doubt, M5 Welcome, M6 None, M7 Doubt, M8 Welcome
//   Mx site:     M1-M4 Mx North, M5-M8 Mx South
//   Mx visits:   12 3 8 1 20 5 7 15

import fs from 'node:fs';
import path from 'node:path';

const IDS = ['M1', 'M2', 'M3', 'M4', 'M5', 'M6', 'M7', 'M8'];
const VISITS = [12, 3, 8, 1, 20, 5, 7, 15];
const WELCOME = [[0, 0, 14], [0, 15, 19], [1, 0, 14], [4, 0, 14], [6, 0, 14], [7, 0, 14]];
const DOUBT = [[0, 20, 26], [1, 20, 26], [2, 0, 6], [3, 0, 6], [6, 0, 6], [6, 7, 14]];

export async function runMixed(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const open = async () => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Mixed Methods', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Mixed Methods', exact: true }).click();
      await page.getByRole('button', { name: 'Mixed Methods page', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Mixed Methods', exact: true }).waitFor();
    }
  };
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  const column = async (variable) => {
    const d = await rpc('data.get');
    const c = d.variables.findIndex((v) => v.name === variable);
    if (c < 0) return null;
    return IDS.map((id) => d.values[d.cases.findIndex((k) => k.name === id)][c]);
  };
  const choose = (name) => page.locator('label').filter({ has: page.getByRole('radio', { name }) }).click();
  const quant = () => page.getByTestId('mixed-quantitize');
  const qual = () => page.getByTestId('mixed-qualitize');
  const codeId = async (name) => (await rpc('code.list')).find((c) => c.name === name)?.id;
  /** The unit of each of the eight respondents (the document they are in, and where their answer is), in the order of their numbers. */
  let units = [];
  /** Codes words inside the answer of respondent `i`, counting from the start of the answer. */
  const codeIn = (i, start, end, code) => rpc('coding.apply', { source_id: units[i].source_id, start: units[i].start + start, end: units[i].start + end, code_id: code });

  await S('Mixed Methods opens from its tab, says what it does, and has no accessibility problems', async () => {
    let csv = 'id,Mx site,Mx visits,answer\n';
    IDS.forEach((id, i) => {
      csv += `${id},${i < 4 ? 'Mx North' : 'Mx South'},${VISITS[i]},I felt welcome here number ${id}.\n`;
    });
    const file = path.join(workDir, 'mixed-cases.csv');
    fs.writeFileSync(file, csv, 'utf8');
    const mapping = { id_column: 'id', text_columns: ['answer'], attribute_columns: [{ name: 'Mx site', level: 'nominal' }, { name: 'Mx visits', level: 'ratio' }] };
    const imported = await rpc('import.run', { path: file, mapping });
    h.equal(imported.outcome.case_ids.length, 8, 'eight cases came in, one for each respondent');
    units = (await rpc('unit.list', { source_id: imported.outcome.source_ids[0] })).filter((u) => !u.excluded).sort((a, b) => a.start - b.start);
    h.equal(units.length, 8, 'and eight answers');
    h.equal([units[0].end - units[0].start, units[7].end - units[7].start], [30, 30], 'each of them 30 characters long');
    const welcome = await rpc('code.add', { name: 'Mx Welcome' });
    const doubt = await rpc('code.add', { name: 'Mx Doubt' });
    for (const [i, start, end] of WELCOME) await codeIn(i, start, end, welcome.id);
    for (const [i, start, end] of DOUBT) await codeIn(i, start, end, doubt.id);
    await page.reload();
    await open();
    h.check((await text(page.getByRole('main'))).includes('what the coding says becomes a number you can test'), 'the page says what it is for');
    h.equal(await page.getByRole('main').count(), 1, 'one main landmark');
    h.check(await quant().isVisible() && (await qual().isVisible()), 'and offers the two directions, codes into variables and variables into codes');
    h.equal(await page.getByTestId('mixed-make').isDisabled(), true, 'nothing can be made before a code is chosen');
    h.check((await text(page.getByTestId('mixed-why'))).includes('Choose the code'), 'and it says what is missing');
    await axe('Mixed Methods, before anything is chosen');
  });

  await S('whether a case has a code: the effect is shown before anything is saved, and then the variable is an ordinary variable of cases', async () => {
    await page.locator('#mixed-code').selectOption({ label: 'Mx Welcome' });
    h.equal(await page.locator('#mixed-name').inputValue(), 'Has Mx Welcome', 'the new variable is named after the code, and the name can be changed');
    await page.getByTestId('mixed-preview').click();
    const shown = page.getByTestId('mixed-result');
    await shown.waitFor();
    h.check(/Has Mx Welcome would be made\. Its level would be Categories \(nominal\)\. \d+ cases would have a value\./.test(await text(shown)), `the preview says what it would give: ${await text(shown)}`);
    const rows = await shown.locator('tbody tr').count();
    h.check(rows > 0, 'with the first cases and the value each would get');
    h.equal(await column('Has Mx Welcome'), null, 'and nothing was saved yet');
    await page.getByTestId('mixed-make').click();
    await page.getByText(/^Made the variable Has Mx Welcome\./).waitFor();
    h.equal(await column('Has Mx Welcome'), ['1', '1', '0', '0', '1', '0', '1', '1'], 'the cases that carry the code are 1, the others 0');
    const d = await rpc('data.get');
    const col = d.variables.find((v) => v.name === 'Has Mx Welcome');
    h.equal([col.level, col.value_labels['0'], col.value_labels['1']], ['nominal', 'No', 'Yes'], 'it is a variable of categories, labelled No and Yes');
    const sentence = await text(page.getByTestId('mixed-result'));
    const total = d.cases.length;
    const withValue = d.values.filter((r) => r[d.variables.indexOf(col)] !== null).length;
    h.check(sentence.includes(`${withValue} cases have a value.`) || (withValue === 1 && sentence.includes('1 case has a value.')), `and says how many cases have a value (${withValue} of ${total}): ${sentence}`);
    await axe('Mixed Methods, a variable made from a code');
  });

  await S('how many excerpts carry a code counts distinct excerpts, and a name that is taken is not taken over', async () => {
    await choose(/How many excerpts carry a code/);
    await page.locator('#mixed-code').selectOption({ label: 'Mx Welcome' });
    h.equal(await page.locator('#mixed-name').inputValue(), 'Mx Welcome excerpts', 'the name follows the measure');
    await page.getByTestId('mixed-make').click();
    await page.getByText(/^Made the variable Mx Welcome excerpts\./).waitFor();
    h.equal(await column('Mx Welcome excerpts'), ['2', '1', '0', '0', '1', '0', '1', '1'], 'the number of excerpts of each case');
    const col = (await rpc('data.get')).variables.find((v) => v.name === 'Mx Welcome excerpts');
    h.equal(col.level, 'ratio', 'a count is a number with a true zero');
    await page.getByTestId('mixed-make').click();
    await page.getByText(/already a variable/).first().waitFor();
    h.check(true, 'making the same variable again is refused, in words');
  });

  await S('which of several codes a case has most names the code, both when they tie, and None when there is none', async () => {
    await choose(/Which of several codes a case has most/);
    h.equal(await page.getByTestId('mixed-make').isDisabled(), true, 'it cannot be made before the codes are chosen');
    await page.getByTestId('mixed-codes').click();
    const pop = page.getByRole('dialog', { name: 'Codes to compare' });
    await pop.waitFor();
    await pop.getByText('Mx Welcome', { exact: true }).click();
    await pop.getByText('Mx Doubt', { exact: true }).click();
    await page.keyboard.press('Escape');
    await pop.waitFor({ state: 'detached' });
    h.equal(await page.locator('#mixed-name').inputValue(), 'Most used of Mx Welcome, Mx Doubt', 'the name says which codes');
    await page.getByTestId('mixed-make').click();
    await page.getByText(/^Made the variable Most used of Mx Welcome, Mx Doubt\./).waitFor();
    h.equal(
      await column('Most used of Mx Welcome, Mx Doubt'),
      ['Mx Welcome', 'Mx Welcome and Mx Doubt', 'Mx Doubt', 'Mx Doubt', 'Mx Welcome', 'None', 'Mx Doubt', 'Mx Welcome'],
      'each case has the code most of its excerpts carry; M2 has one of each, M6 has none',
    );
    await axe('Mixed Methods, a variable of the code a case has most');
    await page.getByTestId('mixed-record-quant').click();
    await page.getByText('Recorded in the integration log', { exact: true }).waitFor();
    const recorded = (await rpc('mixed.integration')).find((e) => e.what.startsWith('Made the variable Most used of Mx Welcome, Mx Doubt from the coding of Mx Welcome, Mx Doubt.'));
    h.check(recorded && recorded.stage === 'analysis' && recorded.how === 'transforming', `a step can be recorded in the integration log with one press: ${JSON.stringify(recorded)}`);
  });

  await S('a variable made from codes says so in the Data editor, and follows the coding when it is made again', async () => {
    await page.getByTestId('mixed-to-editor').click();
    await page.getByRole('heading', { level: 2, name: 'Data editor', exact: true }).waitFor();
    await page.getByRole('tab', { name: 'Variable view', exact: true }).click();
    const table = page.getByTestId('ve-variables');
    await table.waitFor();
    const row = table.locator('tr').filter({ hasText: 'Has Mx Welcome' });
    h.check((await text(row)).includes('From the coding: whether a case has Mx Welcome'), `how it was made is said with the code's name: ${await text(row)}`);
    const welcome = await codeId('Mx Welcome');
    await codeIn(2, 15, 19, welcome);
    h.equal((await column('Has Mx Welcome'))[2], '0', 'a variable keeps what it was when it was made');
    await row.getByTestId('ve-recompute').click();
    const t0 = Date.now();
    while ((await column('Has Mx Welcome'))[2] !== '1' && Date.now() - t0 < 8000) await api.sleep(150);
    h.equal((await column('Has Mx Welcome'))[2], '1', 'and follows the coding when it is made again');
  });

  await S('the values of a variable become codes: the plan is shown, values can be left out, and the text of each case is coded', async () => {
    await open();
    await page.locator('#mixed-variable').selectOption({ label: 'Mx site' });
    const plan = page.getByTestId('mixed-plan');
    await plan.waitFor();
    h.equal(await page.locator('#mixed-parent').inputValue(), 'Mx site', 'the parent code is named after the variable');
    let said = await text(plan);
    h.check(said.includes('This would make 2 codes under Mx site.'), `the plan says what codes it makes: ${said}`);
    h.check(said.includes('Mx North: 4 cases') && said.includes('Mx South: 4 cases'), 'and how many cases have each value');
    await page.getByTestId('mixed-value-Mx South').click();
    await page.getByText('This would make 1 code under Mx site.').waitFor();
    said = await text(plan);
    h.check(!said.includes('Mx South'), 'a value that is left out is not in the plan');
    await page.getByTestId('mixed-value-Mx South').click();
    await page.getByText('This would make 2 codes under Mx site.').waitFor();
    await page.locator('#mixed-parent').fill('Mx site group');
    await page.getByText('This would make 2 codes under Mx site group.').waitFor();
    h.equal((await rpc('code.list')).some((c) => c.name === 'Mx site group'), false, 'and nothing is made before the button is pressed');
    await axe('Mixed Methods, the plan for the values of a variable');
    await page.getByTestId('mixed-qualitize-make').click();
    const done = page.getByTestId('mixed-qualitized');
    await done.waitFor();
    h.check((await text(done)).startsWith('Made 2 codes.'), `it says what it made: ${await text(done)}`);
    const codes = await rpc('code.list');
    const parent = codes.find((c) => c.name === 'Mx site group');
    const north = codes.find((c) => c.name === 'Mx North');
    const south = codes.find((c) => c.name === 'Mx South');
    h.check(parent && north && south, 'the parent code and a code for each value exist');
    h.equal([north.parent_id, south.parent_id], [parent.id, parent.id], 'under the parent');
    const coded = await rpc('coding.list', { source_id: units[0].source_id });
    const spanOf = (i) => [units[i].start, units[i].end];
    h.check(coded.some((c) => c.code_id === north.id && c.start === spanOf(0)[0] && c.end === spanOf(0)[1]), 'M1 is in the north, and the whole of its answer carries that code');
    h.check(!coded.some((c) => c.code_id === south.id && c.start === spanOf(0)[0]), 'and not the other one');
    h.check(coded.some((c) => c.code_id === south.id && c.start === spanOf(7)[0] && c.end === spanOf(7)[1]), 'M8 is in the south');
    h.equal([coded.filter((c) => c.code_id === north.id).length, coded.filter((c) => c.code_id === south.id).length], [4, 4], 'four answers in each');
    h.equal(await page.getByTestId('mixed-qualitize-make').isDisabled(), true, 'the same codes are not made twice by pressing again');

    // A coding made by hand with one of the new codes is not what the step made, so taking it back leaves it.
    const byHand = await rpc('coding.apply', { source_id: units[0].source_id, start: units[0].start, end: units[0].start + 4, code_id: north.id });
    await page.getByTestId('mixed-qualitize-undo').click();
    const back = page.getByTestId('mixed-took-back');
    await back.waitFor();
    h.check((await text(back)).startsWith('Took back 8 codings. Retired 3 codes.'), `taking it back says what it removed: ${await text(back)}`);
    const after = await rpc('code.list');
    h.equal(['Mx site group', 'Mx North', 'Mx South'].map((n) => after.find((c) => c.name === n).status), ['retired', 'retired', 'retired'], 'the codes stay in the Codebook, retired');
    const left = (await rpc('coding.list', { source_id: units[0].source_id })).filter((c) => c.code_id === north.id || c.code_id === south.id);
    h.equal(left.map((c) => c.id), [byHand.id], 'and every coding the step made is gone, but the one made by hand');
    await axe('Mixed Methods, after taking it back');

    // Then it can be made again, since the retired codes do not hold the names.
    await page.getByTestId('mixed-qualitize-make').click();
    await page.getByTestId('mixed-qualitized').waitFor();
    const again = await rpc('code.list');
    h.equal(['Mx site group', 'Mx North', 'Mx South'].map((n) => again.filter((c) => c.name === n && c.status === 'active').length), [1, 1, 1], 'made again, each code is there once more');
    await page.getByTestId('mixed-to-codebook').click();
    await page.getByRole('heading', { level: 2, name: 'Codebook Editor', exact: true }).waitFor();
    h.check(true, 'and the Codebook is one click away');
  });

  const jointTab = async (which = 'ordered') => {
    await open();
    await page.getByTestId('mixed-tab-joint').click();
    await page.getByTestId('joint-displays').waitFor();
    await page.locator('#jd-which').selectOption(which);
  };
  const pickIn = async (button, dialogName, labels) => {
    await page.getByTestId(button).click();
    const pop = page.getByRole('dialog', { name: dialogName });
    await pop.waitFor();
    for (const l of labels) await pop.getByText(l, { exact: true }).click();
    await page.keyboard.press('Escape');
    await pop.waitFor({ state: 'detached' });
  };
  const rowOf = (name) => page.getByTestId('jd-ordered-row').filter({ has: page.locator('th', { hasText: new RegExp(`^${name}$`) }) });

  await S('the joint displays tab says what is in Explore and what is built here, and has no accessibility problems', async () => {
    await jointTab();
    h.check((await text(page.getByTestId('jd-elsewhere'))).includes('crosstab and the segment matrix are already built'), 'it says where the crosstab and the segment matrix are');
    const offered = await page.locator('#jd-which option').allTextContents();
    h.equal(offered, ['Case-ordered display', 'Typology table', 'Side-by-side display', 'Deviation table', 'Similarity analysis', 'Code by variable bubble plot'], 'the six displays are offered, one at a time');
    h.check(await page.getByTestId('jd-ordered').isVisible(), 'and the case-ordered display is the one shown first');
    h.equal(await page.getByTestId('jd-typology').isVisible(), false, 'the others are not shown beside it');
    h.equal(await page.getByTestId('mixed-quantitize').isVisible(), false, 'nor is the other tab');
    h.check((await text(page.getByTestId('jd-ordered-why'))).includes('Choose the variable'), 'a display that is not set up says what is missing');
    await axe('Mixed Methods, the joint displays tab');
    await page.locator('#jd-which').selectOption('typology');
    h.check((await text(page.getByTestId('jd-typology-why'))).includes('Choose the codes that make the groups'), 'and so does the typology table');
    await axe('Mixed Methods, the typology table before anything is chosen');
    await page.getByTestId('jd-to-crosstab').click();
    await page.getByRole('heading', { level: 2, name: 'Explore', exact: true }).waitFor();
    h.check(true, 'Open Crosstabs leads to Explore');
  });

  await S('the case-ordered display puts the cases in the order of a number and shows the words coded beside each', async () => {
    await jointTab();
    await page.locator('#jd-sort').selectOption({ label: 'Mx visits' });
    await pickIn('jd-codes', 'Codes whose words are shown', ['Mx Welcome']);
    const summary = page.getByTestId('jd-ordered-summary');
    await summary.waitFor();
    // Welcome is on M1 (twice), M2, M3 (since the step that made a variable again), M5, M7, and M8; visits 12, 3, 8, 1, 20, 5, 7, 15.
    const names = async () => page.getByTestId('jd-ordered-row').evaluateAll((rows) => rows.map((r) => r.querySelector('th').textContent.trim()));
    h.equal(await names(), ['M2', 'M7', 'M3', 'M1', 'M8', 'M5'], 'lowest number first: 3, 7, 8, 12, 15, 20');
    h.check(/6 of \d+ cases, ordered by Mx visits in the order of its numbers\./.test(await text(summary)), `it says how many cases are shown, by what, and how: ${await text(summary)}`);
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /Highest first/ }) }).click();
    await page.waitForFunction(() => document.querySelector('[data-testid=jd-ordered-row] th')?.textContent?.trim() === 'M5');
    h.equal(await names(), ['M5', 'M8', 'M1', 'M3', 'M7', 'M2'], 'highest first reverses it');
    const m1 = rowOf('M1');
    const cell = await text(m1);
    h.check(cell.includes('12') && cell.includes('2 excerpts') && cell.includes('I felt welcome') && cell.includes('here'), `M1 has its number and its two excerpts, quoted: ${cell}`);
    await axe('Mixed Methods, a case-ordered display');
  });

  await S('a quotation can be limited, the rest are counted and opened as evidence, and other variables can be shown', async () => {
    await page.locator('#jd-quotes').selectOption('1');
    await page.getByTestId('jd-more').waitFor();
    h.equal(await page.getByTestId('jd-more').count(), 1, 'only M1 has more than one excerpt, so only it offers the rest');
    h.check((await text(rowOf('M1'))).includes('Show all, 1 more'), 'and says how many more there are');
    await page.getByTestId('jd-more').click();
    const evidence = page.getByTestId('jd-evidence');
    await evidence.waitFor();
    await page.getByTestId('evidence-count').waitFor();
    h.check((await text(evidence)).includes('2 codings'), `the evidence behind the count is opened: ${(await text(evidence)).slice(0, 120)}`);
    await pickIn('jd-also', 'Other variables to show', ['Mx site']);
    await page.getByRole('columnheader', { name: 'Mx site', exact: true }).waitFor();
    h.check((await text(rowOf('M1'))).includes('Mx North') && (await text(rowOf('M5'))).includes('Mx South'), 'the other variable is shown beside the one it is ordered by');
    await page.getByTestId('jd-only-coded').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=jd-ordered-row]').length > 6);
    const d = await rpc('data.get');
    h.equal(await page.getByTestId('jd-ordered-row').count(), d.cases.length, 'without the tick, every case is a row');
    h.check((await text(rowOf('M4'))).includes('None'), 'a case with no coding of the code says None');
    await page.getByTestId('jd-only-coded').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=jd-ordered-row]').length === 6);
    h.check(true, 'and with it back, only the six');
  });

  await S('a quotation opens its document at the words', async () => {
    await rowOf('M5').getByRole('button', { name: /Open .* at these words, from M5/ }).first().click();
    await page.getByTestId('surface').waitFor();
    h.check(await page.getByTestId('surface').isVisible(), 'the document is open in the reader');
    await open();
    await page.getByTestId('mixed-tab-joint').click();
    await page.getByTestId('jd-ordered-summary').waitFor();
    h.check(/6 of \d+ cases/.test(await text(page.getByTestId('jd-ordered-summary'))), 'and the display is as it was left when we come back');
  });

  await S('the typology table describes the cases that have each code, with the mean and spread of a number and the counts of categories', async () => {
    await jointTab('typology');
    await pickIn('jd-group-codes', 'Codes that make the groups', ['Mx Welcome', 'Mx Doubt']);
    await pickIn('jd-group-vars', 'Variables that describe the groups', ['Mx visits', 'Mx site']);
    const table = page.getByTestId('jd-typology-result');
    await table.getByRole('columnheader', { name: 'Mx site', exact: true }).waitFor();
    const row = (label) => page.getByTestId('jd-group-row').filter({ has: page.locator('th', { hasText: label }) });
    const welcome = await text(row('Cases that have Mx Welcome'));
    // Welcome: M1, M2, M3, M5, M7, M8 with 12, 3, 8, 20, 7, 15 visits: n 6, mean 65/6, sd sqrt(186.8333/5); three of them North and three South.
    h.check(welcome.includes('6 cases') && /\b6\b.*10\.83.*6\.11.*\b6\b.*3 \(50%\).*3 \(50%\)/.test(welcome), `the group that has Welcome: ${welcome}`);
    const doubt = await text(row('Cases that have Mx Doubt'));
    // Doubt: M1, M2, M3, M4, M7 with 12, 3, 8, 1, 7: n 5, mean 6.2, sd sqrt(74.8/4); four North and one South.
    h.check(doubt.includes('5 cases') && /\b5\b.*6\.20.*4\.32.*\b5\b.*4 \(80%\).*1 \(20%\)/.test(doubt), `the group that has Doubt: ${doubt}`);
    const all = await text(row('All cases'));
    // Everyone: visits 12, 3, 8, 1, 20, 5, 7, 15 (n 8, mean 71/8, sd sqrt(286.875/7), which is 6.40); no other case has a value.
    h.check(/\b8\b.*8\.88.*6\.40/.test(all), `all the cases: ${all}`);
    const none = await text(row('none of these codes'));
    // Only M6 holds text and has neither code; it has 5 visits and no spread to speak of.
    h.check(/\b1\b.*5\.00.*—/.test(none), `the cases with neither code: ${none}`);
    h.check((await text(page.getByTestId('jd-typology-note'))).includes('hold text'), 'it says that only a case with text can have a code');
    await axe('Mixed Methods, a typology table');
  });

  await S('the cases of a group can be listed, and the displays fit a narrow window', async () => {
    await page.getByTestId('jd-group-row').filter({ has: page.locator('th', { hasText: 'Cases that have Mx Doubt' }) }).getByTestId('jd-show-cases').click();
    const list = page.getByTestId('jd-cases');
    await list.waitFor();
    h.equal((await list.locator('li').allTextContents()).map((s) => s.trim()).sort(), ['M1', 'M2', 'M3', 'M4', 'M7'], 'the five cases that have Doubt');
    await page.getByTestId('jd-cases').getByRole('button', { name: 'Close the list' }).click();
    await list.waitFor({ state: 'detached' });
    const before = page.viewportSize();
    await page.setViewportSize({ width: 360, height: 740 });
    const over = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
    h.check(over <= 1, `nothing is wider than the window (${over} px over)`);
    await page.setViewportSize(before);
  });

  const group = async (heading) => page.getByTestId('jd-dev-group').filter({ has: page.getByRole('heading', { name: new RegExp(`^${heading}`) }) });

  await S('the side-by-side display puts the words of each topic beside the numbers of the cases that have it and of the others', async () => {
    await jointTab('side');
    h.check((await text(page.getByTestId('jd-side-why'))).includes('Choose the codes that are the topics'), 'a display that is not set up says what is missing');
    await axe('Mixed Methods, the side-by-side display before anything is chosen');
    await pickIn('jd-side-codes', 'Codes that are the topics', ['Mx Welcome', 'Mx Doubt']);
    await pickIn('jd-side-vars', 'Variables that stand for the numbers', ['Mx visits', 'Mx site']);
    await page.getByTestId('jd-side-result').getByRole('columnheader', { name: 'Mx site', exact: true }).waitFor();
    const row = (name) => page.getByTestId('jd-side-row').filter({ has: page.locator('th', { hasText: name }) });
    // Welcome is on M1 (twice), M2, M3, M5, M7, and M8, with 12, 3, 8, 20, 7, 15 visits: n 6, mean 65/6, sd 6.11. The others are M4 and M6,
    // with 1 and 5 visits: n 2, mean 3, sd sqrt(8). Seven excerpts in all. These were checked in R with mean() and sd().
    const welcome = await text(row('Mx Welcome'));
    h.check(welcome.includes('6 cases') && welcome.includes('7 excerpts') && welcome.includes('I felt welcome') && welcome.includes('Show all, 5 more'), `the topic's words: ${welcome}`);
    h.check(/n = 6, mean 10\.83, SD 6\.11/.test(welcome) && /n = 2, mean 3\.00, SD 2\.83/.test(welcome) && /Difference of the means: \+7\.83/.test(welcome), `and the numbers of the cases that have it and of the others: ${welcome}`);
    h.check(/Mx North: 3 \(50%\)/.test(welcome) && /Mx South: 1 \(50%\)/.test(welcome), 'with the counts of the categories');
    // Doubt is on M1, M2, M3, M4, and M7 (twice), with 12, 3, 8, 1, 7 visits; the others are M5, M6, and M8 with 20, 5, 15.
    const doubt = await text(row('Mx Doubt'));
    h.check(doubt.includes('5 cases') && doubt.includes('6 excerpts') && /n = 5, mean 6\.20, SD 4\.32/.test(doubt) && /n = 3, mean 13\.33, SD 7\.64/.test(doubt) && /Difference of the means: [-\u2212]7\.13/.test(doubt), `the second topic: ${doubt}`);
    h.check((await row('Mx Welcome').getByRole('button', { name: /Open .* at these words, from M1/ }).count()) >= 1, 'a quotation opens its document at the words');
    await row('Mx Welcome').getByTestId('jd-side-more').click();
    const evidence = page.getByTestId('jd-side-evidence');
    await evidence.waitFor();
    await page.getByTestId('evidence-count').waitFor();
    h.check((await text(evidence)).includes('7 codings'), `Show all opens the seven excerpts behind the count: ${(await text(evidence)).slice(0, 100)}`);
    await axe('Mixed Methods, a side-by-side display');
  });

  await S('the deviation table says which codes are most and least typical of each group, with the residual R gives', async () => {
    await jointTab('deviation');
    h.check((await text(page.getByTestId('jd-dev-why'))).includes('Choose the variable whose values make the groups'), 'a display that is not set up says what is missing');
    await page.locator('#jd-dev-variable').selectOption({ label: 'Mx site' });
    await pickIn('jd-dev-codes', 'Codes to look at', ['Mx Welcome', 'Mx Doubt']);
    const summary = page.getByTestId('jd-dev-summary');
    await summary.waitFor();
    await page.waitForFunction(() => document.querySelector('[data-testid=jd-dev-group]')?.textContent?.includes('Mx Doubt'));
    h.check((await text(summary)).includes('Every share is a share of the 8 cases that hold text and have a value of Mx site.'), `it says what every share is a share of: ${await text(summary)}`);
    h.equal(await page.getByTestId('jd-dev-group').count(), 2, 'there is a group for each value');
    // Doubt is on all four of the North and on one of the four of the South; R: chisq.test(matrix(c(4, 0, 1, 3), 2), correct = FALSE)$stdres[1, 1]
    // is 2.190890230021 (and minus that for the South). Welcome is on three of each, so it is typical of neither.
    const north = await text(await group('Mx North'));
    h.check(north.includes('Mx Doubt') && north.includes('4 of 4 (100%)') && north.includes('1 of 4 (25%)') && north.includes('+75 points') && north.includes('+2.19'), `the North has Doubt more often: ${north}`);
    h.check(!north.includes('Mx Welcome') && north.includes('No code is less common here than in the other cases.'), 'and no code less often, and Welcome is in neither list');
    const south = await text(await group('Mx South'));
    h.check(south.includes('Mx Doubt') && south.includes('\u221275 points') && south.includes('\u22122.19') && south.includes('No code is more common here than in the other cases.'), `the South has it less often: ${south}`);
    await (await group('Mx North')).getByTestId('jd-dev-cases').click();
    h.equal((await (await group('Mx North')).locator('.jd-case-list li').allTextContents()).map((s) => s.trim()).sort(), ['M1', 'M2', 'M3', 'M4'], 'the cases behind a figure can be listed');
    await axe('Mixed Methods, a deviation table');
  });

  await S('similarity analysis lists the pairs of cases that are most alike and how, and gives the table of every pair', async () => {
    await jointTab('similarity');
    h.check((await text(page.getByTestId('jd-sim-why'))).includes('Choose at least one code or one variable'), 'a display that is not set up says what is missing');
    await pickIn('jd-sim-codes', 'Codes to compare on', ['Mx Welcome', 'Mx Doubt']);
    await pickIn('jd-sim-vars', 'Variables to compare on', ['Mx visits']);
    const summary = page.getByTestId('jd-sim-summary');
    await summary.waitFor();
    await page.waitForFunction(() => document.querySelector('[data-testid=jd-sim-summary]')?.textContent?.includes('compared on 3 features'));
    // On its own the project has the eight respondents; in the whole check it also holds the sample project's cases, which have none of
    // these codes and no visits, so they are alike on nothing and the pairs among the eight are the same ones either way.
    h.check(/^\d+ cases that hold text are compared on 3 features\./.test(await text(summary)) && (await text(summary)).includes('pairs have something to be compared on.'), `it says what is compared: ${await text(summary)}`);
    const caseCount = Number((await text(summary)).match(/^(\d+) cases/)[1]);
    const pairs = page.getByTestId('jd-sim-pair');
    h.equal(await pairs.count(), 15, 'fifteen pairs are listed, as asked by default');
    // Gower, as the R in the next line works it out for the visits (range 19): (1 + 1 + (1 - 1/19)) / 3 = 0.982456 for M3 and M7;
    // (1 + 1 + (1 - 4/19)) / 3 = 0.929825 for M1 and M3 and for M2 and M7, which tie and so come in the order of the cases.
    const first = await text(pairs.nth(0));
    h.check(first.includes('M3 and M7') && first.includes('0.98') && first.includes('3 features'), `the most alike: ${first}`);
    h.check((await text(pairs.nth(1))).includes('M1 and M3') && (await text(pairs.nth(1))).includes('0.93') && (await text(pairs.nth(2))).includes('M2 and M7'), 'and the two that tie come in the order of the cases');
    await pairs.nth(0).getByTestId('jd-sim-how').click();
    const how = await text(pairs.nth(0));
    h.check(how.includes('Mx Welcome') && how.includes('Has it') && /Mx visits.*8.*7.*0\.95/.test(how), `Show how gives the parts of the pair: ${how}`);
    await page.locator('#jd-sim-pairs').selectOption('5');
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=jd-sim-pair]').length === 5);
    h.check((await text(page.getByTestId('jd-similarity'))).includes('both lack a code are not compared on it'), 'it says that two cases that both lack a code are not compared on it');
    await page.getByTestId('jd-sim-shared').click();
    await page.waitForFunction(() => !document.querySelector('[data-testid=jd-similarity]')?.textContent?.includes('both lack a code are not compared on it'));
    h.check(true, 'and the note goes when shared absence is asked for');
    await page.getByTestId('jd-sim-shared').click();
    if (caseCount > 60) {
      h.equal(await page.getByTestId('jd-sim-matrix').count(), 0, 'with more than 60 cases there is no table of every pair');
      h.check((await text(page.getByTestId('jd-similarity'))).includes('There are more than 60 cases, so only the most alike pairs are listed'), 'and it says why');
    } else {
      await page.getByTestId('jd-sim-matrix').locator('summary').click();
      const m3 = page.getByTestId('jd-sim-matrix').locator('tbody tr').filter({ has: page.locator('th', { hasText: /^M3$/ }) });
      h.equal((await m3.locator('td').allTextContents()).map((s) => s.trim())[6], '0.98', 'the table of every pair has M3 and M7 at 0.98');
    }
    await axe('Mixed Methods, a similarity analysis');
  });

  await S('the bubble plot shows a code across two variables, with the numbers beside it, and can be saved as a picture', async () => {
    await jointTab('bubble');
    h.check((await text(page.getByTestId('jd-bubble-why'))).includes('Choose the code'), 'a display that is not set up says what is missing');
    await page.locator('#jd-bubble-code').selectOption({ label: 'Mx Doubt' });
    await page.locator('#jd-bubble-x').selectOption({ label: 'Mx site' });
    await page.locator('#jd-bubble-y').selectOption({ label: 'Has Mx Welcome' });
    await page.getByTestId('jd-bubble-chart').waitFor();
    h.check((await text(page.getByTestId('jd-bubble-summary'))).startsWith('8 cases that hold text and have a value of both variables are in the plot.'), `it says what is in the plot: ${await text(page.getByTestId('jd-bubble-summary'))}`);
    const marks = page.locator('.jd-bubble-mark');
    h.equal(await marks.count(), 4, 'a bubble for each of the four combinations');
    // Doubt is on M1, M2, M3 (all North, with Welcome), M4 (North, without), and M7 twice (South, with); M6 is South without Welcome.
    const labels = await marks.evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    h.check(labels.includes('Mx North, Yes: 3 excerpts, from 3 of 3 cases') && labels.includes('Mx South, Yes: 2 excerpts, from 1 of 3 cases') && labels.includes('Mx North, No: 1 excerpt, from 1 of 1 case'), `each bubble says its numbers: ${labels.join(' | ')}`);
    h.equal(await page.locator('.jd-bubble-none').count(), 1, 'a combination with cases and no excerpts is a dotted ring, not a bubble');
    h.check((await text(page.getByTestId('jd-bubble-chart'))).includes('No coding of this code has a weight'), 'and it says that nothing has a weight to colour by');
    const table = await text(page.getByTestId('jd-bubble-table'));
    h.check(table.includes('3 excerpts 3 of 3 cases') && table.includes('2 excerpts 1 of 3 cases'), `the same numbers are in the table: ${table}`);
    await page.getByTestId('jd-bubble-show').first().click();
    await page.getByTestId('jd-bubble-evidence').waitFor();
    await page.getByTestId('evidence-count').waitFor();
    h.check(/\d+ codings?/.test(await text(page.getByTestId('jd-bubble-evidence'))), 'a cell opens the excerpts behind it');
    await axe('Mixed Methods, a bubble plot');

    // Save it as a picture, both ways.
    const target = path.join(workDir, 'bubble-picture');
    await page.getByTestId('jd-bubble-save').click();
    const dlg = page.getByTestId('stat-export-dialog');
    await dlg.waitFor();
    await dlg.getByTestId('stat-export-path').fill(target);
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*bubble-picture\.png/).waitFor();
    const png = fs.readFileSync(target + '.png');
    h.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a', 'a PNG file');
    await dlg.getByTestId('stat-export-svg').click();
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*bubble-picture\.svg/).waitFor();
    const svg = fs.readFileSync(target + '.svg', 'utf8');
    h.check(svg.includes('<svg') && svg.includes('Mx Doubt: Mx site by Has Mx Welcome') && svg.includes('<circle'), 'and an SVG drawing with the bubbles in it and the name of the chart');
    await dlg.getByTestId('stat-export-close').click();
    await dlg.waitFor({ state: 'detached' });
  });

  await S('a rated code colours the bubbles by the mean weight of its codings', async () => {
    const warmth = await rpc('code.add', { name: 'Mx Warmth', rated: true, rating_label: 'Warmth', rating_low: 'Cool', rating_high: 'Glowing' });
    for (const [i, rating] of [[0, 2], [1, 4], [4, 5]]) {
      const coding = await codeIn(i, 0, 14, warmth.id);
      await rpc('coding.rating', { id: coding.id, rating });
    }
    await page.reload();
    await jointTab('bubble');
    await page.locator('#jd-bubble-code').selectOption({ label: 'Mx Warmth' });
    await page.locator('#jd-bubble-x').selectOption({ label: 'Mx site' });
    await page.locator('#jd-bubble-y').selectOption({ label: 'Has Mx Welcome' });
    await page.getByTestId('jd-bubble-chart').waitFor();
    const labels = await page.locator('.jd-bubble-mark').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    // M1 and M2 (North, with Welcome) are rated 2 and 4, M5 (South, with Welcome) is rated 5; M4 and M6 have no Warmth.
    h.check(labels.includes('Mx North, Yes: 2 excerpts, from 2 of 3 cases, mean weight 3.00') && labels.includes('Mx South, Yes: 1 excerpt, from 1 of 3 cases, mean weight 5.00'), `the mean weight is in each bubble: ${labels.join(' | ')}`);
    h.equal(await page.locator('.jd-bubble-mark.jd-seq-2').count() + (await page.locator('.jd-bubble-mark.jd-seq-4').count()), 2, 'the bubbles are in the step of their mean weight (3 and 5)');
    h.check((await text(page.getByTestId('jd-bubble-chart'))).includes('Mean weight:'), 'and the chart has a key to the colours');
    h.check((await text(page.getByTestId('jd-bubble-table'))).includes('mean weight 3.00, 2 rated'), 'the table has the mean weight and how many are rated');
    await axe('Mixed Methods, a bubble plot coloured by weight');
  });

  const designTab = async (part = 'design') => {
    await open();
    await page.getByTestId('mixed-tab-design').click();
    await page.getByTestId('mixed-design-page').waitFor();
    await page.locator('#md-part').selectOption(part);
  };
  const logRows = async () => page.getByTestId('md-log-row').evaluateAll((rows) => rows.map((r) => r.innerText.replace(/\s+/g, ' ').trim()));

  await S('the design page offers four designs, shows what a template would make before it makes it, and sets it up once', async () => {
    await designTab();
    h.check((await text(page.getByTestId('md-none'))).includes('No design has been chosen yet.'), 'it says that no design has been chosen');
    h.equal(await page.getByTestId('md-kinds').getByRole('radio').count(), 4, 'there are four designs');
    await axe('Mixed Methods, the design page before a design is chosen');
    await page.getByTestId('md-kind-explanatory_sequential').click();
    await page.waitForFunction(() => document.querySelector('[data-testid=md-plan]')?.textContent?.includes('Quantitative result group'));
    const plan = await text(page.getByTestId('md-plan'));
    h.check(plan.includes('A variable of cases named Quantitative result group') && plan.includes('A theme code named Explaining the quantitative result'), `the plan says what would be made: ${plan.slice(0, 200)}`);
    h.equal((await rpc('data.get')).variables.some((v) => v.name === 'Quantitative result group'), false, 'and nothing is made until the button is pressed');
    h.equal(await page.locator('#md-priority').inputValue(), 'quantitative', "the design's usual priority is offered");
    await page.locator('#md-purpose').fill('To explain why some respondents visit often.');
    await page.getByTestId('md-apply').click();
    const outcome = page.getByTestId('md-outcome');
    await outcome.waitFor();
    h.check((await text(outcome)).includes('Chose the explanatory sequential design.') && (await text(outcome)).includes('Made the variable Quantitative result group, the theme code Explaining the quantitative result.'), `it says what it made: ${await text(outcome)}`);
    const design = await rpc('mixed.design');
    h.equal([design.kind, design.priority, design.purpose], ['explanatory_sequential', 'quantitative', 'To explain why some respondents visit often.'], 'the design is kept in the project');
    const variable = (await rpc('data.get')).variables.find((v) => v.name === 'Quantitative result group');
    h.equal(variable && variable.level, 'nominal', 'the variable is an ordinary variable of categories');
    const theme = (await rpc('code.list')).find((c) => c.name === 'Explaining the quantitative result');
    h.equal([theme && theme.kind, theme && theme.status], ['theme', 'active'], 'and the code is an ordinary theme in the Codebook');
    const entry = (await rpc('mixed.integration')).find((e) => e.what === 'Chose the explanatory sequential design.');
    h.check(entry && entry.stage === 'design' && entry.how === 'connecting' && entry.why === 'To explain why some respondents visit often.', `the first line of the integration log is written: ${JSON.stringify(entry)}`);
    h.check((await text(page.getByTestId('md-current'))).includes('The study follows the explanatory sequential design. The numbers have priority.'), 'and the page says which design it is');
    await page.getByTestId('md-apply').click();
    await page.waitForFunction(() => document.querySelector('[data-testid=md-outcome]')?.textContent?.includes('Nothing new was needed.'));
    const sameEntries = (await rpc('mixed.integration')).filter((e) => e.what === 'Chose the explanatory sequential design.').length;
    h.equal(sameEntries, 1, 'pressing again makes nothing twice and writes no second line');
    await axe('Mixed Methods, a design set up');
  });

  await S('a design leads to the joint displays that suit it, and the design can be forgotten without losing what was made', async () => {
    await page.getByTestId('md-open-deviation').click();
    await page.getByTestId('jd-deviation').waitFor();
    h.equal(await page.locator('#jd-which').inputValue(), 'deviation', 'Open it goes to the display on the joint displays tab');
    await designTab();
    await page.getByTestId('md-open-ordered').click();
    await page.getByTestId('jd-ordered').waitFor();
    h.equal(await page.locator('#jd-which').inputValue(), 'ordered', 'and to another one the next time');
    await designTab();
    await page.getByTestId('md-forget').click();
    await page.getByTestId('md-none').waitFor();
    h.equal(await rpc('mixed.design'), null, 'forgetting the design clears the choice');
    h.check((await rpc('data.get')).variables.some((v) => v.name === 'Quantitative result group') && (await rpc('code.list')).some((c) => c.name === 'Explaining the quantitative result'), 'but the variable and the code stay');
    await page.getByTestId('md-kind-convergent').click();
    await page.waitForFunction(() => document.querySelector('[data-testid=md-plan]')?.textContent?.includes('This design needs no variable or code of its own'));
    h.check(true, 'a design that needs nothing says so');
    await page.getByTestId('md-apply').click();
    await page.getByTestId('md-outcome').waitFor();
    h.equal((await rpc('mixed.design')).kind, 'convergent', 'and a design can be chosen again');
  });

  await S('the triangulation matrix keeps your words and mark beside the words and numbers behind a code and a variable', async () => {
    await designTab('triangulation');
    h.check((await text(page.getByTestId('md-tri-empty'))).includes('There are no topics yet.'), 'it starts empty');
    await page.getByTestId('md-tri-add').click();
    await page.getByTestId('md-tri-save').click();
    await page.getByText(/a topic needs a name/).waitFor();
    h.check(true, 'a topic with no name is refused in words');
    await page.locator('#md-tri-topic').fill('Welcome');
    await page.locator('#md-tri-code').selectOption({ label: 'Mx Welcome' });
    await page.locator('#md-tri-variable').selectOption({ label: 'Mx visits' });
    await page.locator('#md-tri-words').fill('People felt welcome.');
    await page.locator('#md-tri-numbers').fill('Visits varied.');
    await page.getByTestId('md-tri-mark-convergence').click();
    await page.locator('#md-tri-note').fill('Both say people came.');
    await axe('Mixed Methods, a triangulation topic being written');
    await page.getByTestId('md-tri-save').click();
    const row = page.getByTestId('md-tri-row').first();
    await row.waitFor();
    const said = await text(row);
    h.check(said.includes('Welcome') && said.includes('People felt welcome.') && said.includes('Visits varied.') && said.includes('Convergence') && said.includes('Both say people came.'), `the row has what was written: ${said}`);
    // Welcome is on 7 excerpts of 6 cases; the visits of those six are 12, 3, 8, 20, 7, 15 and of M4 and M6 are 1 and 5 (worked out above, in R).
    h.check(said.includes('Mx Welcome: 7 excerpts from 6 cases') && /n = 6, mean 10\.83, SD 6\.11/.test(said) && /n = 2, mean 3\.00, SD 2\.83/.test(said) && /Difference of the means: \+7\.83/.test(said), `and the words and numbers behind its code and variable: ${said}`);
    await page.getByTestId('md-tri-add').click();
    await page.locator('#md-tri-topic').fill('Doubt');
    await page.locator('#md-tri-variable').selectOption({ label: 'Mx visits' });
    await page.getByTestId('md-tri-save').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=md-tri-row]').length === 2);
    const second = await text(page.getByTestId('md-tri-row').nth(1));
    h.check(second.includes('All the cases that hold text') && /n = 8, mean 8\.88, SD 6\.40/.test(second) && second.includes('Not marked'), `a topic with a variable and no code describes every case with text: ${second}`);
    await page.getByTestId('md-tri-row').nth(1).getByTestId('md-tri-edit').click();
    await page.getByTestId('md-tri-mark-discordance').click();
    await page.getByTestId('md-tri-save').click();
    await page.waitForFunction(() => document.querySelector('[data-testid=md-tri-summary]')?.textContent?.includes('1 discordance'));
    h.check((await text(page.getByTestId('md-tri-summary'))).includes('2 topics. 1 marked convergence, 0 complementarity, 1 discordance, and 0 not marked.'), `the summary counts the marks: ${await text(page.getByTestId('md-tri-summary'))}`);
    await page.getByRole('button', { name: 'Move Doubt up', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('[data-testid=md-tri-row] th')?.textContent?.trim() === 'Doubt');
    h.equal((await rpc('mixed.triangulation')).map((r) => r.topic), ['Doubt', 'Welcome'], 'a topic can be moved up');
    await axe('Mixed Methods, the triangulation matrix');
    await page.getByRole('button', { name: 'Remove the topic Doubt', exact: true }).click();
    await page.getByTestId('md-tri-remove-yes').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=md-tri-row]').length === 1);
    h.equal((await rpc('mixed.triangulation')).map((r) => r.topic), ['Welcome'], 'and a topic can be removed, after being asked');
    // A quotation opens its document at the words, and the rest are counted.
    h.check((await page.getByTestId('md-tri-row').first().getByRole('button', { name: /Open .* at these words/ }).count()) >= 1, 'a quotation opens its document at the words');
    await page.getByTestId('md-tri-more').click();
    await page.getByTestId('md-tri-evidence').waitFor();
    await page.getByTestId('evidence-count').waitFor();
    h.check((await text(page.getByTestId('md-tri-evidence'))).includes('7 codings'), 'and Show all opens the seven excerpts behind the count');
  });

  await S('the integration log keeps dated entries in your words, refuses a date that is not one, and takes a step recorded from the page', async () => {
    await designTab('log');
    await page.getByTestId('md-log-row').first().waitFor();
    const before = await logRows();
    h.check(before.some((r) => r.includes('Chose the convergent design.')) && before.some((r) => r.includes('Made the variable Most used of Mx Welcome, Mx Doubt from the coding of Mx Welcome, Mx Doubt.')), `the log has the design line and the step that was recorded: ${before.join(' | ')}`);
    await page.getByTestId('md-log-add').click();
    await page.locator('#md-log-date').fill('next week');
    await page.locator('#md-log-stage').selectOption({ label: 'Interpretation' });
    await page.locator('#md-log-how').selectOption({ label: 'Connecting: one strand leads to the other' });
    await page.locator('#md-log-what').fill('Used the interviews to explain the visits.');
    await page.locator('#md-log-why').fill('The visits alone did not say why.');
    await page.getByTestId('md-log-save').click();
    await page.getByText(/is not a date/).waitFor();
    h.check(true, 'a date that is not a date is refused, in words');
    await page.locator('#md-log-date').fill('2026-03-02');
    await axe('Mixed Methods, an integration log entry being written');
    await page.getByTestId('md-log-save').click();
    await page.waitForFunction(() => [...document.querySelectorAll('[data-testid=md-log-row]')].some((r) => r.textContent.includes('Used the interviews to explain the visits.')));
    const mine = (await logRows()).find((r) => r.includes('Used the interviews'));
    h.check(mine.startsWith('2026-03-02 Interpretation Connecting Used the interviews to explain the visits. The visits alone did not say why.'), `the entry is in the log: ${mine}`);
    h.check((await rpc('mixed.integration'))[0].on_date <= '2026-03-02', 'and the log is in the order of the dates, the oldest first');
    await page.getByRole('button', { name: 'Edit the entry of 2026-03-02', exact: true }).click();
    await page.locator('#md-log-why').fill('The visits alone did not say why. Changed my mind about the order.');
    await page.getByTestId('md-log-save').click();
    await page.waitForFunction(() => [...document.querySelectorAll('[data-testid=md-log-row]')].some((r) => r.textContent.includes('Changed my mind')));
    h.check(true, 'an entry can be edited');
    await axe('Mixed Methods, the integration log');
    await page.getByRole('button', { name: 'Remove the entry of 2026-03-02', exact: true }).click();
    await page.getByTestId('md-log-remove-yes').click();
    await page.waitForFunction(() => ![...document.querySelectorAll('[data-testid=md-log-row]')].some((r) => r.textContent.includes('Used the interviews')));
    h.check(true, 'and removed, after being asked');
  });

  await S('the report can carry the design, the log, and the matrix, and says so when asked for them', async () => {
    const report = await rpc('export.report', { path: path.join(workDir, 'mixed-report.docx'), sections: ['mixed_methods'] }).catch((e) => ({ error: String(e && e.message) }));
    h.check(report && !report.error, `the report with the mixed methods section is made: ${JSON.stringify(report)}`);
    h.check(fs.existsSync(path.join(workDir, 'mixed-report.docx')) && fs.statSync(path.join(workDir, 'mixed-report.docx')).size > 2000, 'and is a file with something in it');
  });

  await S('Mixed Methods fits a narrow window without a sideways scroll', async () => {
    await open();
    const before = page.viewportSize();
    await page.setViewportSize({ width: 360, height: 740 });
    const over = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
    h.check(over <= 1, `nothing is wider than the window (${over} px over)`);
    for (const which of ['side', 'deviation', 'similarity', 'bubble']) {
      await page.getByTestId('mixed-tab-joint').click();
      await page.locator('#jd-which').selectOption(which);
      const wide = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
      h.check(wide <= 1, `the ${which} display is not wider than the window (${wide} px over)`);
    }
    for (const part of ['design', 'triangulation', 'log']) {
      await page.getByTestId('mixed-tab-design').click();
      await page.locator('#md-part').selectOption(part);
      const wide = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
      h.check(wide <= 1, `the ${part} part is not wider than the window (${wide} px over)`);
    }
    await page.setViewportSize(before);
  });
}
