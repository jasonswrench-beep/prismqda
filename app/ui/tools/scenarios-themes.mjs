// Developing themes (decision 167): the Themes page lists each theme with its codes and quotations; a theme is added, codes are moved under it
// (by the list beside each code, and ticked ones are made into a theme together), a theme's name, definition, and stage are changed, and a
// thematic map is made of the themes. The check's project is a fresh one made here, so nothing depends on what earlier steps left in it.

import fs from 'node:fs';
import path from 'node:path';

const NOTES = 'The staff gave me a warm welcome.\n\nThe wait for the bus was long.\n\nIt rained and nobody had an umbrella.\n\nA stranger said hello on the platform.\n';

export async function runThemes(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;
  let codes = {};
  let first;

  const goThemes = async () => {
    const back = page.getByRole('button', { name: 'Back to the coding workspace' });
    if ((await back.count()) > 0) await back.first().click();
    await page.getByRole('tab', { name: 'View', exact: true }).click();
    await page.locator('[data-command="go-themes"]').click();
    await page.getByTestId('themes-view').waitFor();
  };

  await S('the check makes four codes and codes four passages with them', async () => {
    fs.writeFileSync(path.join(workDir, 'themes-notes.txt'), NOTES, 'utf8');
    const done = await rpc('import.run', { path: path.join(workDir, 'themes-notes.txt') });
    first = done.outcome.source_ids[0];
    for (const name of ['Warm welcome', 'Long wait', 'Weather', 'Greeting']) codes[name] = (await rpc('code.add', { name })).id;
    const units = await rpc('unit.list', { source_id: first });
    const give = ['Warm welcome', 'Long wait', 'Weather', 'Greeting'];
    for (let i = 0; i < 4; i++) await rpc('coding.apply', { source_id: first, start: units[i].start, end: units[i].end, code_id: codes[give[i]], unit_id: units[i].id });
    await page.reload();
    h.equal((await rpc('theme.overview', {})).themes.length, 0, 'there is no theme yet');
  });

  await S('the Themes page says there is no theme yet and lists the codes still to place', async () => {
    await goThemes();
    await page.getByTestId('themes-empty').waitFor();
    const left = await page.getByTestId('themes-unassigned').innerText();
    for (const n of ['Warm welcome', 'Long wait', 'Weather', 'Greeting']) h.check(left.includes(n), `${n} is among the codes under no theme`);
    h.equal(await page.getByTestId('theme-map').isDisabled(), true, 'a thematic map needs a theme first');
    await axe('the Themes page with no theme');
  });

  await S('a theme is added from the page, and a code is moved under it with the list beside the code', async () => {
    await page.getByTestId('theme-add').click();
    await page.getByTestId('newtheme-save').click();
    await page.getByRole('alert').filter({ hasText: 'A theme needs a name' }).waitFor();
    await page.getByTestId('newtheme-name').fill('Being welcomed');
    await page.getByTestId('newtheme-definition').fill('People are greeted and made to feel at home.');
    await axe('the Add a theme dialog');
    await page.getByTestId('newtheme-save').click();
    await page.getByTestId('theme-card').first().waitFor();
    const o = await rpc('theme.overview', {});
    h.equal([o.themes.length, o.themes[0].name, o.themes[0].stage], [1, 'Being welcomed', 'candidate'], 'one candidate theme with the name it was given');
    h.equal(o.themes[0].definition, 'People are greeted and made to feel at home.', 'and its definition');
    h.check(/No code is under this theme yet/.test(await page.getByTestId('theme-no-codes').innerText()), 'it holds no code yet, and says so');
    // move "Warm welcome" under it with the list beside the code
    const row = page.getByTestId('themes-unassigned').getByTestId('theme-code').filter({ hasText: 'Warm welcome' });
    await row.getByTestId('theme-move').selectOption({ label: 'Being welcomed' });
    await page.getByTestId('theme-card').getByTestId('theme-code').filter({ hasText: 'Warm welcome' }).waitFor();
    const after = await rpc('theme.overview', {});
    h.equal(after.themes[0].codes.map((c) => c.name), ['Warm welcome'], 'the code is under the theme');
    const left1 = after.unassigned.map((c) => c.name);
    h.check(!left1.includes('Warm welcome') && ['Greeting', 'Long wait', 'Weather'].every((n) => left1.includes(n)), 'and no longer among the codes to place, while the others still are');
    h.check(/1 passage/.test(await page.getByTestId('theme-counts').first().innerText()), 'the theme counts its passage');
    h.check((await page.getByTestId('theme-quote').count()) >= 1, 'and quotes it');
    await axe('the Themes page with a theme');
  });

  await S('ticked codes are made into a theme together, and the name, definition, and stage of a theme are changed', async () => {
    const un = page.getByTestId('themes-unassigned');
    for (const n of ['Long wait', 'Weather']) await un.getByTestId('theme-code').filter({ hasText: n }).getByTestId('theme-code-tick').click();
    await page.getByTestId('theme-from-ticked').click();
    const listed = await page.getByTestId('newtheme-codes').innerText();
    h.check(/Long wait/.test(listed) && /Weather/.test(listed), 'the dialog lists the ticked codes');
    await page.getByTestId('newtheme-name').fill('Waiting in the weather');
    await page.getByTestId('newtheme-save').click();
    await page.getByTestId('theme-card').nth(1).waitFor();
    const o = await rpc('theme.overview', {});
    h.equal(o.themes.map((t) => t.name).sort(), ['Being welcomed', 'Waiting in the weather'], 'two themes');
    h.equal(o.themes.find((t) => t.name === 'Waiting in the weather').codes.map((c) => c.name).sort(), ['Long wait', 'Weather'], 'the second holds the two ticked codes');
    const left2 = o.unassigned.map((c) => c.name);
    h.check(left2.includes('Greeting') && !left2.includes('Long wait') && !left2.includes('Weather'), 'Greeting is still to place and the two ticked codes are not');

    // stage, name, and definition
    const card = page.getByTestId('theme-card').filter({ has: page.getByRole('heading', { name: 'Waiting in the weather' }) });
    await card.getByTestId('theme-stage').selectOption('reviewed');
    const waiting = async () => (await rpc('theme.overview', {})).themes.find((t) => /^Waiting/.test(t.name));
    for (let i = 0; i < 50 && (await waiting()).stage !== 'reviewed'; i++) await page.waitForTimeout(100);
    h.equal((await waiting()).stage, 'reviewed', 'the stage was changed');
    await card.getByTestId('theme-name').fill('Waiting and weather');
    await card.getByTestId('theme-definition').fill('The wait is long and the weather makes it worse.');
    await card.getByTestId('theme-save').click();
    for (let i = 0; i < 50 && (await waiting()).name !== 'Waiting and weather'; i++) await page.waitForTimeout(100);
    const saved = await waiting();
    h.equal([saved.name, saved.definition], ['Waiting and weather', 'The wait is long and the weather makes it worse.'], 'the name and the definition were saved');
    h.equal(await page.getByTestId('themes-stages').innerText(), 'Candidate: 1 · Reviewed: 1 · Defined: 0', 'the page counts the themes by stage');
    await axe('the Themes page with two themes');
  });

  // The browser's own drag and drop does not reach the page in the Windows window (Tauri takes over the drop for files), so a code is dragged
  // with pointer events (components/pointerDrag.ts); the check drives the real mouse, which is what a person's hand does.
  await S('a code is dragged onto a theme card with the mouse and back to the codes under no theme, and a click or a press on a control drags nothing', async () => {
    // A tall window, so that the code and the card are on the screen together however many themes and codes the whole check has made, and
    // the mouse is let go whatever happens, so that a failure here does not leave a drag going into the steps after it.
    const window0 = page.viewportSize();
    await page.setViewportSize({ width: window0.width, height: 3200 });
    try {
    const un = page.getByTestId('themes-unassigned');
    const card = page.getByTestId('theme-card').filter({ has: page.getByRole('heading', { name: 'Being welcomed' }) });
    const greeting = (scope) => scope.getByTestId('theme-code').filter({ hasText: 'Greeting' });
    const underWelcome = async () => (await rpc('theme.overview', {})).themes.find((t) => t.name === 'Being welcomed').codes.map((c) => c.name).sort();
    h.equal(await underWelcome(), ['Warm welcome'], 'to begin with, one code is under the theme');

    const drag = async (row, target, release = true) => {
      await row.scrollIntoViewIfNeeded();
      // press on the code's name (a row can have the list that moves it on a second line, and that is a control)
      const a = await row.locator('.th-code-name, [data-testid="theme-code-tick"]').first().boundingBox();
      await page.mouse.move(a.x + Math.min(14, a.width / 2), a.y + a.height / 2);
      await page.mouse.down();
      await page.mouse.move(a.x + 40, a.y + a.height / 2 + 6, { steps: 3 });
      // a point of the target that is on the screen (a tall card may be scrolled partly out of the window, under the ribbon)
      const point = await target.evaluate((el, height) => {
        const r = el.getBoundingClientRect();
        const x = r.left + r.width / 2;
        for (let y = Math.min(r.bottom - 8, height - 8); y > Math.max(r.top, 0); y -= 8) {
          const hit = document.elementFromPoint(x, y);
          if (hit && el.contains(hit)) return { x, y };
        }
        return null;
      }, page.viewportSize().height);
      if (!point) throw new Error('no part of the drop target is on the screen');
      await page.mouse.move(point.x, point.y, { steps: 8 });
      if (release) await page.mouse.up();
    };

    // onto the card: the card shows where it would go, something follows the pointer, and letting go moves the code
    await drag(greeting(un), card, false);
    h.equal(await card.getAttribute('data-drop-over'), 'true', 'the card shows where the code would go');
    h.equal(await page.locator('.drag-ghost').count(), 1, 'and the code\'s name follows the pointer');
    await page.mouse.up();
    await card.getByTestId('theme-code').filter({ hasText: 'Greeting' }).waitFor();
    h.equal(await underWelcome(), ['Greeting', 'Warm welcome'], 'the code is under the theme');
    h.equal(await page.locator('.drag-ghost').count(), 0, 'nothing follows the pointer any more');
    h.equal(await card.getAttribute('data-drop-over'), null, 'and the card is no longer marked');

    // back onto the codes under no theme
    await drag(greeting(card), un);
    await un.getByTestId('theme-code').filter({ hasText: 'Greeting' }).waitFor();
    h.equal(await underWelcome(), ['Warm welcome'], 'dragging it back takes it from the theme');

    // Escape stops a drag; letting go anywhere else moves nothing
    await drag(greeting(un), card, false);
    await page.keyboard.press('Escape');
    await page.mouse.up();
    h.equal(await underWelcome(), ['Warm welcome'], 'Escape cancels the drag');
    h.equal(await page.locator('.drag-ghost').count(), 0, 'and takes the name from the pointer');
    const a = await greeting(un).boundingBox();
    await page.mouse.move(a.x + 14, a.y + a.height / 2);
    await page.mouse.down();
    await page.mouse.move(a.x + 40, a.y + a.height / 2 + 6, { steps: 3 });
    await page.mouse.move(2, 2, { steps: 4 });
    await page.mouse.up();
    h.equal(await underWelcome(), ['Warm welcome'], 'a drop on nothing moves nothing');

    // a drag that starts on the words of a tick does not tick it, and a click on the words still does
    const box = greeting(un).locator('input[type=checkbox]');
    h.equal(await box.isChecked(), false, 'the tick was not ticked by the drags above');
    await greeting(un).getByTestId('theme-code-tick').click();
    h.equal(await box.isChecked(), true, 'a click on the words of a tick still ticks it');
    await greeting(un).getByTestId('theme-code-tick').click();
    h.equal(await box.isChecked(), false, 'and clicks it off again');

    // a press on a control (the button that shows the passages) is the control's, not a drag's
    const passages = await greeting(un).getByRole('button', { name: /^Show the passages coded/ }).boundingBox();
    await page.mouse.move(passages.x + 4, passages.y + passages.height / 2);
    await page.mouse.down();
    // away from the button, so that letting go is not a click on it
    await page.mouse.move(passages.x + 4, passages.y - 45, { steps: 4 });
    h.equal(await page.locator('.drag-ghost').count(), 0, 'a press on a button beside a code starts no drag');
    await page.mouse.up();
    await axe('the Themes page after dragging');
    } finally {
      await page.mouse.up().catch(() => undefined);
      await page.keyboard.press('Escape').catch(() => undefined);
      await page.setViewportSize(window0);
    }
  });

  await S('a thematic map is made of the themes and opens in the Concept Map tool', async () => {
    await page.getByTestId('theme-map').click();
    await page.getByRole('heading', { level: 2, name: 'Explore' }).waitFor();
    const maps = await rpc('map.list', {});
    h.check(maps.some((m) => m.name === 'Thematic map'), `the map is made: ${maps.map((m) => m.name).join(', ')}`);
    const map = await rpc('map.get', { id: maps.find((m) => m.name === 'Thematic map').id });
    h.equal(map.nodes.length, 5, 'two themes and the three codes under them');
    await axe('Explore, the thematic map');
  });

  await S('an AI is asked for candidate themes: a packet with no passage, an answer pasted back, checked, and accepted or rejected one by one', async () => {
    await goThemes();
    await page.getByTestId('theme-ai-build').click();
    await page.getByTestId('theme-ai-packet').waitFor();
    const prompt = await page.getByTestId('theme-ai-prompt').inputValue();
    h.check(/Group the codes into candidate themes/.test(prompt) && /- Greeting/.test(prompt) && /- Warm welcome/.test(prompt), 'the packet asks for themes and lists the codes');
    h.check(!/umbrella/.test(prompt), 'and holds no passage of a document');
    await page.getByTestId('theme-ai-check').isDisabled().then((d) => h.equal(d, true, 'the answer cannot be checked before there is one and a model is named'));
    await page.getByTestId('theme-ai-model').fill('Test chatbot 1');
    await page.getByTestId('theme-ai-answer').fill(JSON.stringify([{ theme: 'Hello and welcome', definition: 'Being greeted.', codes: ['Greeting', 'warm welcome', 'Invented code'] }, { theme: 'Alone', codes: ['Greeting'] }]));
    await axe('the Themes page with a packet to take to a chatbot');
    // Leaving the page for the coding workspace and coming back keeps the packet, the model's name, and the answer being pasted.
    const typed = await page.getByTestId('theme-ai-answer').inputValue();
    await page.getByRole('button', { name: 'Back to the coding workspace' }).first().click();
    await page.getByTestId('themes-view').waitFor({ state: 'detached' });
    await goThemes();
    await page.getByTestId('theme-ai-packet').waitFor();
    h.equal(await page.getByTestId('theme-ai-answer').inputValue(), typed, 'the answer being pasted is still there after leaving the page and coming back');
    h.equal(await page.getByTestId('theme-ai-model').inputValue(), 'Test chatbot 1', 'and the name of the model');
    h.equal(await page.getByTestId('theme-ai-prompt').inputValue(), prompt, 'and the packet');
    await page.getByTestId('theme-ai-check').click();
    await page.getByTestId('theme-ai-run').waitFor();
    h.equal(await page.getByTestId('theme-suggestion').count(), 1, 'one suggestion passed the check');
    h.check(/Invented code/.test(await page.getByTestId('suggestion-problems').innerText()), 'and says which code it dropped');
    h.check(/fewer than two codes/.test(await page.getByTestId('theme-ai-rejected').innerText()), 'the checker says why the other was refused');
    h.equal((await rpc('theme.overview', {})).themes.length, 2, 'nothing is made yet');
    await axe('the Themes page with a suggestion waiting');
    await page.getByTestId('suggestion-name').fill('Hello and welcome');
    await page.getByTestId('suggestion-accept').click();
    await page.getByTestId('suggestion-status').waitFor();
    const o = await rpc('theme.overview', {});
    h.equal(o.themes.map((t) => t.name).sort(), ['Being welcomed', 'Hello and welcome', 'Waiting and weather'], 'accepting made the theme');
    h.equal(o.themes.find((t) => t.name === 'Hello and welcome').codes.map((c) => c.name).sort(), ['Greeting', 'Warm welcome'], 'with the codes under it');
    h.check(!o.unassigned.some((c) => c.name === 'Greeting' || c.name === 'Warm welcome'), 'and they are no longer among the codes to place');
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=theme-card]').length === 3, null, { timeout: 8000 }).catch(() => undefined);
    h.equal(await page.getByTestId('theme-card').count(), 3, 'and the page shows three themes');
    const runs = await rpc('ai.runs', {});
    const themeRun = runs.find((r) => r.task === 'suggest_themes');
    h.equal([themeRun.accepted, themeRun.pending], [1, 0], 'the AI log records the run and the decision');
  });
}
