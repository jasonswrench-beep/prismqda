// A class exercise (decision 183): the Class tab of the Team page makes a blind packet for every student from a pasted list, brings the
// students' files back from one folder in one step (a file that is not a packet is told and left), and reviews the class against the
// instructor's own coding: a table of students, the codes the class found hardest, and the passages it coded least like the instructor,
// with the students shown as numbers when asked, and the table saved as a CSV file. The students' work is made through the command
// layer by opening each packet as the student, so nothing here depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';

const PARAGRAPHS = ['Fares are too high for me.', 'The stop is far from my house.', 'I love the new buses.'];

export async function runClass(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();

  const ourPath = (await rpc('project.info')).path;
  const examPath = path.join(workDir, 'class-exercise.pqda');
  const folder = path.join(workDir, 'class-packets');
  const packetFor = (name) => path.join(folder, `class-exercise - packet for ${name}.pqda`);

  const openClass = async () => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Team', exact: true }).click();
      await page.getByRole('button', { name: 'Team page', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();
    }
    await page.getByTestId('team-tab-class').click();
    await page.getByTestId('class-tab').waitFor();
  };

  /** Codes paragraph `which` of the only document of the open project with `code`, as the person using the project. */
  const codeParagraph = async (which, code) => {
    const source = (await rpc('source.list'))[0];
    const units = await rpc('unit.list', { source_id: source.id });
    const id = (await rpc('code.list')).find((c) => c.name === code).id;
    await rpc('coding.apply', { source_id: source.id, start: units[which].start, end: units[which].end, code_id: id, unit_id: units[which].id });
  };

  await S('an exercise is coded by the instructor, and the Class tab explains what to do', async () => {
    await rpc('project.create', { path: examPath, name: 'Class exercise' });
    fs.writeFileSync(path.join(workDir, 'class-riders.txt'), PARAGRAPHS.join('\n\n'), 'utf8');
    await rpc('import.run', { path: path.join(workDir, 'class-riders.txt') });
    await rpc('code.add', { name: 'Cost' });
    await rpc('code.add', { name: 'Access' });
    await codeParagraph(0, 'Cost');
    await codeParagraph(1, 'Access');
    await codeParagraph(2, 'Cost');
    await page.reload();
    await openClass();
    const said = await text(page.getByTestId('class-tab'));
    h.check(/Code the documents of the exercise yourself first/.test(said) && /1\. Make a packet for every student/.test(said) && /2\. Bring the students’ work in/.test(said) && /3\. Review the class against your coding/.test(said), `the tab says the three steps: ${said.slice(0, 200)}`);
    h.equal(await page.getByTestId('class-folder').getAttribute('placeholder'), `${workDir}${path.sep}class-exercise class packets`, 'the folder suggested is beside the project and named for it');
    h.check(/There is no other coder yet/.test(await text(page.getByTestId('class-student-ticks'))), 'no student is listed before any work is brought in');
    await axe('the Class tab before anything is made');
  });

  await S('a packet is made for every student in a pasted list, a name that came twice once, and none holds the instructor\'s coding', async () => {
    await page.getByTestId('class-make').click();
    await page.getByRole('alert').filter({ hasText: 'Type the names of the students' }).waitFor();
    await page.getByTestId('class-students').fill('Annika\n  Bohdan  \n\nannika\nCyrus\n');
    h.check(/3 students\. A name that came twice is used once: annika\./.test(await text(page.getByTestId('class-packets-card'))), 'the list is counted and the repeated name said');
    await page.getByTestId('class-folder').fill(folder);
    await page.getByTestId('class-instructions').fill('Code each paragraph with Cost or Access.');
    await page.getByTestId('class-make').click();
    await page.getByTestId('class-packets-result').waitFor();
    const rows = page.getByTestId('class-packet-row');
    h.equal(await rows.count(), 3, 'a row for each student');
    h.check((await text(rows.first())).includes('Made.'), 'each was made');
    h.equal(fs.readdirSync(folder).filter((f) => f.endsWith('.pqda')).sort(), ['class-exercise - packet for Annika.pqda', 'class-exercise - packet for Bohdan.pqda', 'class-exercise - packet for Cyrus.pqda'], 'a file for each, named for the project and the student');
    h.check(/3 packets were made in .*Send each student their own file/.test(await text(page.getByTestId('class-packets-result'))), 'the page says what to do next');
    // Again: nothing is replaced, and each says why.
    await page.getByTestId('class-make').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=class-packet-row]').length === 3 && /already exists/.test(document.querySelector('[data-testid=class-packet-table]')?.textContent ?? ''), null, { timeout: 8000 });
    h.equal(fs.readdirSync(folder).filter((f) => f.endsWith('.pqda')).length, 3, 'making them again changed nothing');
    await axe('the Class tab with the packets made');
  });

  await S('the students code in their own packets, and one call brings all their work in, leaving a file that is not a packet', async () => {
    for (const [name, work] of [['Annika', [[0, 'Cost'], [1, 'Access'], [2, 'Cost']]], ['Bohdan', [[0, 'Cost'], [1, 'Cost']]]]) {
      await rpc('project.close', {});
      await rpc('project.open', { path: packetFor(name), backup: false });
      for (const [which, code] of work) await codeParagraph(which, code);
    }
    await rpc('project.close', {});
    await rpc('project.open', { path: examPath, backup: false });
    fs.writeFileSync(path.join(folder, 'Junk.pqda'), 'this is not a project');
    fs.writeFileSync(path.join(folder, 'notes.txt'), 'not a project either');
    await page.reload();
    await openClass();
    await page.getByTestId('class-folder').fill(folder);
    await page.getByTestId('class-collect').click();
    await page.getByTestId('class-collect-result').waitFor({ timeout: 20000 });
    const status = async (file) => page.getByTestId('class-collect-row').filter({ hasText: file }).first().getAttribute('data-status');
    h.equal([await status('Annika'), await status('Bohdan'), await status('Cyrus'), await status('Junk')], ['merged', 'merged', 'merged', 'failed'], 'the three packets were brought in; the file that is not a project was told');
    h.equal(await page.getByTestId('class-collect-row').count(), 4, 'only .pqda files are listed');
    const said = await text(page.getByTestId('class-collect-result'));
    h.check(/5 codings were brought in from 3 packets\. A backup was saved first/.test(said), `the page says how much came in: ${said.slice(0, 200)}`);
    h.check(/It could not be read/.test(await text(page.getByTestId('class-collect-row').filter({ hasText: 'Junk' }))), 'and why the junk file was left');
    const coders = (await rpc('coder.list')).map((c) => c.name).sort();
    h.check(['Annika', 'Bohdan', 'Cyrus'].every((n) => coders.includes(n)), `the students are coders now: ${coders.join(', ')}`);
    h.equal((await rpc('source.list')).length, 1, 'no document came in with a packet');
    await axe('the Class tab after the students\' work was brought in');
    // Again: nothing is new.
    await page.getByTestId('class-collect').click();
    await page.waitForFunction(() => /already here/.test(document.querySelector('[data-testid=class-collect-table]')?.textContent ?? ''), null, { timeout: 20000 });
    h.equal(await status('Annika'), 'nothing_new', 'the second time nothing is new');
  });

  await S('the class is reviewed against the instructor: a table of students, the hardest codes, and the passages the class coded least like the instructor', async () => {
    const ticks = await text(page.getByTestId('class-student-ticks'));
    h.check(/Annika/.test(ticks) && /Bohdan/.test(ticks) && /Cyrus/.test(ticks), `the students are listed to choose from: ${ticks}`);
    await page.getByTestId('class-review').click();
    await page.getByTestId('class-review-result').waitFor();
    const rows = page.getByTestId('class-student-row');
    h.equal(await rows.count(), 3, 'a row for each student');
    h.equal(await Promise.all([0, 1, 2].map((i) => rows.nth(i).locator('th').innerText())), ['Annika', 'Bohdan', 'Cyrus'], 'in the order of their names');
    const cells = async (i) => (await rows.nth(i).locator('td').allInnerTexts()).map((c) => c.trim());
    // Passages coded, same, some of the same, other codes only, left uncoded, coded where the instructor did not, passages the same, codes found
    h.equal(await cells(0), ['3', '3', '0', '0', '0', '0', '100%', '100%'], 'Annika coded all three as the instructor did');
    h.equal(await cells(1), ['2', '1', '0', '1', '1', '0', '33%', '33%'], 'Bohdan matched one, differed on one, and left one uncoded');
    h.equal(await cells(2), ['0', '0', '0', '0', '3', '0', '0%', '0%'], 'Cyrus coded nothing');
    h.check(/1 student coded nothing in these documents/.test(await text(page.getByTestId('class-notes'))), 'the page says a student coded nothing');
    const codes = page.getByTestId('class-code-row');
    h.equal(await codes.count(), 2, 'a row for each code');
    h.equal(await codes.first().locator('th').innerText(), 'Access', 'the code the class found hardest is first');
    const hard = page.getByTestId('class-hard-passage');
    h.equal(await hard.count(), 3, 'every passage someone missed is listed');
    const first = await text(hard.first());
    h.check(/1 of 3 students gave the instructor’s codes/.test(first) && /The stop is far from my house\./.test(first) && /The instructor gave: Access\./.test(first) && /Students also gave: Cost \(1\)\./.test(first) && /Students left out: Access \(2\)\./.test(first), `the hardest passage says what was given and left out: ${first}`);
    await page.getByTestId('class-review-card').scrollIntoViewIfNeeded();
    await page.screenshot({ path: `${ctx.shots}/class-review.png`, fullPage: true });
    await axe('the Class tab with the review of a class');
  });

  await S('the students can be shown as numbers, and the table is saved as a CSV file that is never replaced', async () => {
    await page.getByTestId('class-anonymous').click();
    h.equal(await page.getByTestId('class-review-result').count(), 0, 'a review that named the students is not left on the page when the labels change');
    await page.getByTestId('class-review').click();
    await page.waitForFunction(() => /Student 1/.test(document.querySelector('[data-testid=class-students-table]')?.textContent ?? ''), null, { timeout: 8000 });
    const table = await text(page.getByTestId('class-students-table'));
    h.check(/Student 1/.test(table) && /Student 2/.test(table) && /Student 3/.test(table) && !/Annika|Bohdan|Cyrus/.test(table), `no name is in the table: ${table.slice(0, 160)}`);
    const body = await text(page.getByTestId('class-review-result'));
    h.check(!/Annika|Bohdan|Cyrus/.test(body), 'and no name is anywhere in the review');
    await page.getByTestId('class-anonymous').click();
    await page.getByTestId('class-review').click();
    await page.getByTestId('class-save-path').waitFor();
    // The file.
    const csv = path.join(workDir, 'class-review.csv');
    await page.getByTestId('class-save-path').fill(csv);
    await page.getByTestId('class-save').click();
    await page.getByTestId('class-saved').waitFor();
    const written = fs.readFileSync(csv, 'utf8');
    h.check(/Student,Passages coded,Same codes as the instructor/.test(written) && /Annika,3,3,0,0,0,0,3,0,0,100\.0,100\.0,0\.0/.test(written) && /Cyrus,0,0,0,0,3,0,0,3,0,0\.0,0\.0,/.test(written), `the file has a row for each student: ${written.slice(0, 200)}`);
    await page.getByTestId('class-save').click();
    await page.getByRole('alert').filter({ hasText: 'already exists' }).waitFor();
    h.equal(fs.readFileSync(csv, 'utf8'), written, 'a second save was refused and the file is as it was');
  });

  await S('the instructor\'s own project is open again afterwards', async () => {
    await rpc('project.close', {});
    await rpc('project.open', { path: ourPath, backup: false });
    await page.reload();
    h.equal((await rpc('project.info')).path, ourPath, 'the project the check began with is open');
  });
}
