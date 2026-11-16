// Pictures of the program for the website. This is not a test: it drives the interface in the headless browser and saves a picture
// of each page. Part A uses the built-in made-up Urban Mobility Study; part B makes a second project of 36 invented survey
// respondents, coded by two coders, for the statistics, the data editor, the word counts, and agreement.
//
//   PRISMQDA_ONLY=shots PRISMQDA_SHOTS_DIR=<folder> node tools/browser-check.mjs   (after npm run build)
//
// Nothing here is real participant text. The pictures are of the interface in a headless browser, which draws the same page the
// window does; they are not pictures of the Windows window frame.

import fs from 'node:fs';
import path from 'node:path';

// A small seeded generator, so the invented respondents are the same every time.
export function seeded(seed) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

export const SITES = ['Northside', 'Riverside', 'Hillcrest'];
// [comment, the code a careful coder gives it]
export const COMMENTS = [
  ['The bus comes every ten minutes now, and that changed how I plan my day.', 'Reliability'],
  ['I stopped taking the train because the fare went up and nobody told us why.', 'Cost'],
  ['Getting to the stop is the hard part. There is no crossing on my street.', 'Access'],
  ['The drivers are kind. One of them held the door for my mother and her walker.', 'Service'],
  ['It is cheaper than parking, but it takes twice as long to get downtown.', 'Cost'],
  ['I felt unsafe waiting after dark. The shelter light has been out for months.', 'Safety'],
  ['The app told me the bus was on time, and then it never came.', 'Reliability'],
  ['I like reading on the way to work. It is the only quiet hour I get.', 'Service'],
  ['Transfers are the problem. Missing one means waiting thirty minutes in the cold.', 'Reliability'],
  ['My son rides alone now, and I worry less because the route is simple.', 'Safety'],
  ['The new line to the hospital made a real difference for our family.', 'Access'],
  ['Costs add up when you ride every day. A monthly pass would help.', 'Cost'],
];
export const CODES = ['Cost', 'Reliability', 'Access', 'Safety', 'Service'];

export async function runShots(ctx, api) {
  const { rpc, workDir, base, browser } = ctx;
  const dir = process.env.PRISMQDA_SHOTS_DIR;
  if (!dir) throw new Error('Set PRISMQDA_SHOTS_DIR to the folder the pictures go in.');
  fs.mkdirSync(dir, { recursive: true });

  // The size of a full window and the pixel scale can be set for a set of pictures: PRISMQDA_SHOTS_SIZE=1600x1000 and PRISMQDA_SHOTS_SCALE=1.
  const [W, H] = (process.env.PRISMQDA_SHOTS_SIZE ?? '1440x900').split('x').map(Number);
  const SCALE = Number(process.env.PRISMQDA_SHOTS_SCALE ?? 2);
  const context = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: SCALE, locale: 'en-US', colorScheme: 'light', reducedMotion: 'reduce' });
  // The pictures of the Statistics page show a test being run, so this browser begins with the tests turned on (the page describes unless a person asks for them).
  await context.addInitScript(() => {
    try {
      if (!localStorage.getItem('prismqda.settings')) localStorage.setItem('prismqda.settings', JSON.stringify({ statisticsScope: 'tests' }));
    } catch {
      // A browser without storage shows the page as it begins.
    }
  });
  const page = await context.newPage();
  page.setDefaultTimeout(15000);
  const made = [];
  const skipped = [];
  // Before each picture: nothing on the page may name the person using the computer or a folder on it (the check's own folders are in the
  // user's temporary folder). The visible text and the values of the fields are read, and a hit is written in the log at the end.
  const leaks = [];
  const guard = async (name) => {
    const found = await page.evaluate(() => {
      const bits = [document.body.innerText];
      for (const el of document.querySelectorAll('input, textarea, select')) bits.push(el.value ?? '');
      const all = bits.join('\n');
      const m = all.match(/.{0,40}(Jason|AppData|[A-Za-z]:\\Users\\(?!you\\)|prismqda-check|@aol|@gmail).{0,40}/gi);
      return m ? m.slice(0, 3) : [];
    });
    if (found.length) {
      leaks.push(`${name}: ${found.join(' | ')}`);
      console.log(`LEAK in ${name}: ${found.join(' | ')}`);
    }
  };

  const view = async (width, height) => {
    await page.setViewportSize({ width, height });
    await page.waitForTimeout(400);
  };
  const shot = async (name, opts = {}) => {
    await page.waitForTimeout(opts.wait ?? 500);
    await guard(name);
    const file = path.join(dir, `${name}.png`);
    await (opts.locator ?? page).screenshot({ path: file });
    made.push(name);
  };
  /** Draws a tall element in full: the window is made as tall as the element for the picture, since the page scrolls inside itself. */
  const tall = async (name, locator, width = 1280, opts = {}) => {
    await view(width, 3200);
    await page.waitForTimeout(500);
    const box = await locator.boundingBox();
    await guard(name);
    if (opts.cropToContent && box) {
      // A page fills the window, so the picture ends where its last block ends.
      const bottom = await locator.evaluate((el) => Math.max(...[...el.children].map((k) => k.getBoundingClientRect().bottom)));
      const file = path.join(dir, `${name}.png`);
      await page.screenshot({ path: file, clip: { x: box.x, y: box.y, width: box.width, height: Math.min(box.height, bottom - box.y + 24) } });
      made.push(name);
    } else {
      await view(width, Math.min(3200, Math.ceil((box?.y ?? 0) + (box?.height ?? 800) + 80)));
      await shot(name, { locator, wait: 600 });
    }
    await view(W, H);
  };
  const attempt = async (name, fn) => {
    try {
      await fn();
    } catch (e) {
      skipped.push(`${name}: ${String(e.message ?? e).split('\n')[0]}`);
      console.log(`SKIPPED ${name}: ${String(e.message ?? e).split('\n')[0]}`);
      await view(W, H).catch(() => {});
      await page.keyboard.press('Escape').catch(() => {});
    }
  };
  const act = (fn, ...args) => page.evaluate(([f, a]) => window.__prismqda.actions[f](...a), [fn, args]);
  const go = async (workspace) => {
    await act('goTo', workspace);
    await page.waitForTimeout(500);
  };
  const reload = async () => {
    await page.goto(`${base}/?lang=en`);
    await page.waitForFunction(() => !!window.__prismqda && document.querySelector('#root')?.children.length > 0);
    await page.waitForTimeout(700);
  };

  // ================================================================ part A: the made-up Urban Mobility Study
  await attempt('sample-projects', async () => {
    await rpc('project.close').catch(() => {});
    await reload();
    await page.getByRole('button', { name: 'Sample Projects', exact: true }).click();
    const dlg = page.getByRole('dialog', { name: 'Sample Projects' });
    await dlg.waitFor();
    await shot('sample-projects');
    await dlg.getByRole('textbox', { name: 'Name of your copy' }).fill('Urban Mobility Study');
    await dlg.getByRole('textbox', { name: 'Save the copy in' }).fill(workDir);
    await dlg.getByRole('button', { name: 'Make an editable copy' }).click();
    await page.getByRole('treegrid', { name: 'Documents' }).waitFor();
  });
  if (!fs.existsSync(path.join(workDir, 'Urban Mobility Study.pqda'))) {
    await rpc('sample.create', { id: 'urban-mobility', path: path.join(workDir, 'Urban Mobility Study.pqda') });
  }

  // A few codes get the definitions and rules a real codebook would have.
  const codes = await rpc('code.list');
  const named = (n) => codes.find((c) => c.name === n);
  const describe = async (name, definition, inclusion_criteria, exclusion_criteria) => {
    const c = named(name);
    if (c) await rpc('code.update', { id: c.id, definition, inclusion_criteria, exclusion_criteria });
  };
  await describe('Mobility Experience', 'How the participant describes getting around the city from day to day.', 'Routes, timing, the choice of one way of travelling over another.', 'Opinions about city policy: use Perceptions & Attitudes.');
  await describe('Reliability', 'Whether the service comes when and how the person expects it to.', 'Delays, cancellations, missed connections, a changed timetable.', 'Complaints about price alone: use Cost.');
  await describe('Cost', 'What getting around costs the person, and how that shapes what they choose.', 'Fares, passes, parking, and comparisons of what each way of travelling costs.', 'Time lost: that is not a cost in money.');
  await reload();

  const sources = await rpc('source.list');
  const richest = [...sources].sort((a, b) => b.codings - a.codings)[0];
  await attempt('coding-workspace', async () => {
    await act('openAt', richest.id, 0, 0);
    await page.waitForTimeout(900);
    await shot('coding-workspace');
  });
  await attempt('coding-details', async () => {
    const codings = await rpc('coding.list', { source_id: richest.id });
    if (codings.length > 0) await act('selectCoding', codings[0].id, { openDetails: true });
    await page.waitForTimeout(700);
    await shot('coding-details');
  });

  await view(W, H);
  await attempt('codebook-editor', async () => {
    await go('codebook');
    await page.getByText('Reliability', { exact: true }).first().click();
    await page.waitForTimeout(500);
    await shot('codebook-editor');
  });
  // Import Data and Memos are left out: in a browser the first asks for a typed path where the window has a drop zone, and the second is empty.
  for (const [name, ws] of [['auto-code', 'autocode'], ['ai-assistant', 'ai'], ['project-overview', 'home']]) {
    await attempt(name, async () => {
      await go(ws);
      await shot(name, { wait: 800 });
    });
  }
  await attempt('report', async () => {
    await go('report');
    await shot('report');
    await page.getByRole('button', { name: /^Build a report/ }).click();
    const dlg = page.getByRole('dialog');
    await dlg.waitFor();
    await view(1280, 1300);
    // The last field asks for a typed path in a browser (the window has a Save dialog), so the picture stops above it.
    await page.waitForTimeout(500);
    const dbox = await dlg.boundingBox();
    const where = await dlg.getByText('Where to save it', { exact: true }).boundingBox();
    if (dbox && where) {
      await guard('report-sections');
      await page.screenshot({ path: path.join(dir, 'report-sections.png'), clip: { x: dbox.x, y: dbox.y, width: dbox.width, height: where.y - dbox.y - 10 } });
      made.push('report-sections');
    }
    await page.keyboard.press('Escape');
    await view(W, H);
  });
  await attempt('settings', async () => {
    await go('settings');
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await shot('settings-accessibility');
  });

  // The same working page in other themes and sizes.
  await view(W, H);
  await attempt('themes', async () => {
    await act('openAt', richest.id, 0, 0);
    await go('code');
    await page.waitForTimeout(700);
    await act('updateSettings', { theme: 'dark' });
    await shot('coding-workspace-dark', { wait: 800 });
    await act('updateSettings', { theme: 'contrast' });
    await shot('coding-workspace-high-contrast', { wait: 800 });
    await act('updateSettings', { theme: 'light', textSize: 150 });
    await shot('coding-workspace-large-text', { wait: 900 });
    await act('updateSettings', { theme: 'system', textSize: 100 });
  });

  // ================================================================ part B: 36 invented respondents, coded by two coders
  const surveyPath = path.join(workDir, 'Neighborhood Transit Survey.pqda');
  await rpc('project.close').catch(() => {});
  await rpc('project.create', { path: surveyPath, name: 'Neighborhood Transit Survey' });
  const rnd = seeded(20261107);
  const truth = [];
  let csv = 'id,Age,Site,Rides per week,Satisfaction,answer\n';
  for (let i = 1; i <= 36; i++) {
    const site = SITES[Math.floor(rnd() * 3)];
    const age = 19 + Math.floor(rnd() * 52);
    const rides = 1 + Math.floor(rnd() * 12);
    const baseScore = site === 'Northside' ? 6.2 : site === 'Riverside' ? 5.0 : 4.1;
    const satisfaction = Math.max(1, Math.min(10, Math.round(baseScore + rides * 0.18 + (rnd() - 0.5) * 3)));
    const [comment, code] = COMMENTS[Math.floor(rnd() * COMMENTS.length)];
    truth.push(code);
    csv += `Resident ${String(i).padStart(2, '0')},${age},${site},${rides},${satisfaction},"${comment}"\n`;
  }
  fs.writeFileSync(path.join(workDir, 'survey-respondents.csv'), csv, 'utf8');
  await rpc('import.run', {
    path: path.join(workDir, 'survey-respondents.csv'),
    mapping: {
      id_column: 'id',
      text_columns: ['answer'],
      attribute_columns: [
        { name: 'Age', level: 'ratio' },
        { name: 'Site', level: 'nominal' },
        { name: 'Rides per week', level: 'ratio' },
        { name: 'Satisfaction', level: 'ratio' },
      ],
    },
  });
  const codeIds = {};
  for (const c of CODES) codeIds[c] = (await rpc('code.add', { name: c })).id;
  await attempt('coding-the-survey', async () => {
    const me = (await rpc('coder.list')).find((c) => c.role === 'human');
    const other = await rpc('coder.add', { name: 'Sam Okafor', role: 'human' });
    // The spreadsheet became one document with a unit for each respondent's answer, in the order of the rows.
    const docs = await rpc('source.list');
    const doc = docs.find((d) => d.units >= truth.length) ?? docs[0];
    const units = (await rpc('unit.list', { source_id: doc.id })).filter((u) => !u.excluded);
    for (let i = 0; i < Math.min(units.length, truth.length); i++) {
      const u = units[i];
      const second = rnd() < 0.2 ? CODES[Math.floor(rnd() * CODES.length)] : truth[i];
      await rpc('coding.apply', { source_id: doc.id, start: u.start, end: u.end, code_id: codeIds[truth[i]], coder_id: me.id, unit_id: u.id });
      await rpc('coding.apply', { source_id: doc.id, start: u.start, end: u.end, code_id: codeIds[second], coder_id: other.id, unit_id: u.id });
    }
  });
  await reload();

  // ---- the data editor
  await view(W, H);
  await attempt('data-editor', async () => {
    await go('variables');
    await page.getByTestId('ve-grid').waitFor();
    await shot('data-editor-before');

    await view(W, 1400);
    await page.getByTestId('ve-make-cut').click();
    const dlg = page.getByRole('dialog', { name: /Make a variable: Cut into ranges/ });
    await dlg.waitFor();
    await dlg.getByTestId('tf-target').fill('Age band');
    await dlg.getByTestId('tf-source').selectOption({ label: 'Age' });
    await dlg.getByTestId('tf-cuts').fill('18, 30, 50, 80');
    await dlg.getByTestId('tf-preview-summary').waitFor({ timeout: 8000 });
    await shot('data-editor-make-variable', { locator: dlg });
    await dlg.getByTestId('tf-make').click();
    await dlg.waitFor({ state: 'detached' });

    await page.getByTestId('ve-make-compute').click();
    const dlg2 = page.getByRole('dialog', { name: /Make a variable: Compute/ });
    await dlg2.getByTestId('tf-target').fill('Rides per year');
    await dlg2.getByTestId('tf-formula').fill('`Rides per week` * 52');
    await dlg2.getByTestId('tf-preview-summary').waitFor({ timeout: 8000 });
    await shot('data-editor-compute', { locator: dlg2 });
    await dlg2.getByTestId('tf-make').click();
    await dlg2.waitFor({ state: 'detached' });
    await view(W, H);
    await page.waitForTimeout(500);
    await shot('data-editor');

    await page.getByRole('tab', { name: 'Variable view', exact: true }).click();
    await page.getByTestId('ve-variables').waitFor();
    await shot('data-editor-variables');
    await page.getByRole('tab', { name: 'Data view', exact: true }).click();
  });

  // ---- statistics: compare satisfaction across sites, relate rides and satisfaction
  await view(W, H);
  await attempt('statistics', async () => {
    await go('stats');
    await page.getByTestId('stats-variables').waitFor();
    await page.getByTestId('stats-goal-compare_groups').check();
    await page.getByTestId('stats-role-outcome').selectOption({ label: 'Satisfaction (scale)' });
    await page.getByTestId('stats-role-group').selectOption({ label: 'Site (nominal)' });
    await page.getByTestId('stats-methods').waitFor();
    await shot('statistics-chooser');
    await page.getByTestId('stats-run').click();
    await page.getByTestId('stats-page').first().waitFor();
    await page.waitForTimeout(800);
    await shot('statistics-notebook');
    await tall('statistics-result', page.getByTestId('stats-page').last(), 820);

    await page.getByTestId('stats-goal-correlate').check();
    await page.getByTestId('stats-role-x').selectOption({ label: 'Rides per week (scale)' });
    await page.getByTestId('stats-role-y').selectOption({ label: 'Satisfaction (scale)' });
    await page.getByTestId('stats-run').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=stats-page]').length >= 2);
    await page.waitForTimeout(800);
    await tall('statistics-correlation', page.getByTestId('stats-page').last(), 820);
  });
  await attempt('statistics-dark', async () => {
    await act('updateSettings', { theme: 'dark' });
    await view(W, H);
    await page.evaluate(() => document.querySelector('#main-content')?.scrollTo(0, 0));
    await shot('statistics-notebook-dark', { wait: 900 });
    await act('updateSettings', { theme: 'system' });
  });

  // ---- text analytics, explore, team
  await view(W, H);
  await attempt('text-analytics', async () => {
    await go('text');
    await page.getByRole('spinbutton', { name: 'How many words to list' }).fill('12');
    await page.getByRole('button', { name: 'Count the words', exact: true }).click();
    await page.waitForTimeout(800);
    await tall('text-analytics', page.locator('#main-content'), 1280, { cropToContent: true });
  });
  await attempt('explore-code-matrix', async () => {
    await go('explore');
    await page.getByRole('tab', { name: 'Code Matrix', exact: true }).click();
    await page.waitForTimeout(900);
    await shot('explore-code-matrix');
  });
  await attempt('team-agreement', async () => {
    await go('team');
    await page.getByTestId('team-measure').waitFor();
    await page.getByTestId('team-measure').click();
    await page.getByTestId('agreement-headline').waitFor({ timeout: 20000 });
    await page.waitForFunction(() => !document.querySelector('[data-testid=team-measure]')?.textContent?.includes('Measuring'), null, { timeout: 20000 });
    await tall('team-agreement', page.locator('#main-content'), 1280, { cropToContent: true });
  });

  // ---- the welcome page last, so the list of recent projects has something in it
  await attempt('welcome', async () => {
    await view(W, H);
    await rpc('project.close').catch(() => {});
    await reload();
    await page.getByRole('button', { name: 'Sample Projects', exact: true }).waitFor();
    await shot('welcome');
  });

  console.log(`\n${made.length} pictures saved in ${dir}:\n  ${made.join('\n  ')}`);
  if (skipped.length) console.log(`\n${skipped.length} skipped:\n  ${skipped.join('\n  ')}`);
  console.log(leaks.length ? `\n${leaks.length} PICTURE(S) NAME A PERSON OR A FOLDER:\n  ${leaks.join('\n  ')}` : '\nNo picture names a person or a folder (checked in the text and the fields of every page before it was drawn).');
  await context.close();
}
