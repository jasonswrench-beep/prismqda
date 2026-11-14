// Settings: the gear tab, the colors, the text size, the caret announcements, and putting things back.

import fs from 'node:fs';
import path from 'node:path';

export async function runSettings(ctx, api) {
  const { h } = ctx;
  const { page, S, axe, mainTab, sleep, surface } = api;

  const html = (fn, arg) => page.evaluate(fn, arg);
  const cssVar = (n) => page.evaluate((x) => getComputedStyle(document.querySelector('.workspace')).getPropertyValue(x).trim(), n);
  const theme = () => html(() => document.documentElement.dataset.theme ?? null);
  const rootSize = () => html(() => parseFloat(getComputedStyle(document.documentElement).fontSize));
  const stored = () => html(() => JSON.parse(localStorage.getItem('prismqda.settings') ?? 'null'));
  const background = () => html(() => getComputedStyle(document.body).backgroundColor);
  // Settings is the gear in the title bar.
  const tab = () => page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true });

  /** The radio inputs are visually hidden; a person clicks the words next to them. */
  const choose = (name) => page.locator('label').filter({ has: page.getByRole('radio', { name }) }).click();

  /** Settings opens on General; the other sections are chosen from the list on the left. */
  async function openSettings(name = 'General') {
    await tab().click();
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name, exact: true }).click();
  }

  await S('Settings is the gear in the title bar, opens from there and from Ctrl+K, and has no accessibility problems', async () => {
    await mainTab('Home').click();
    h.check(await tab().isVisible(), 'the gear is in the title bar');
    const box = await tab().boundingBox();
    const search = await page.getByTestId('search-trigger').boundingBox();
    h.check(box.x > search.x + search.width - 1, 'and it is to the right of the search box');
    await openSettings();
    h.equal(await tab().getAttribute('aria-current'), 'page', 'the gear is marked as the current page');
    h.equal(await page.getByRole('main').count(), 1, 'one main landmark');
    await axe('the Settings page');
    await mainTab('Home').click();
    await page.keyboard.press('Control+k');
    await page.getByRole('dialog').waitFor();
    await page.keyboard.type('appearance');
    await page.getByRole('option', { name: /Settings/ }).first().waitFor();
    await page.keyboard.press('Enter');
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    h.check(true, 'Ctrl+K finds Settings by the word appearance');
  });

  await S('the colors can be chosen, are remembered, reach every screen, and have no accessibility problems', async () => {
    await openSettings();
    h.equal(await theme(), null, 'the standard is to follow Windows, so there is no theme attribute');
    for (const [label, id] of [['Dark', 'dark'], ['High Contrast', 'contrast']]) {
      await choose(new RegExp(`^${label}`));
      h.equal(await theme(), id, `${label}: the page carries the theme`);
      h.equal((await stored()).theme, id, `${label}: it is remembered on this computer`);
      const bg = await background();
      const dark = (bg.match(/\d+/g) ?? []).slice(0, 3).map(Number).every((n) => n < 80);
      h.check(dark, `${label}: the background is dark (${bg})`);
      await axe(`the Settings page in ${label}`);
      await mainTab('Home').click();
      await axe(`Home in ${label}`);
      await mainTab('Coding').click();
      await surface().waitFor();
      await axe(`the Code screen in ${label}`);
      // Not Data: on the project with the 15,000-unit source it takes the debug engine many seconds, and the
      // reload below would queue behind it. Report is quick and has the cards and buttons Data has.
      await mainTab('Report').click();
      await axe(`Report in ${label}`);
      await openSettings();
    }
    // Still there after the window is reloaded, and the project stays open.
    await page.reload();
    await tab().waitFor();
    h.equal(await theme(), 'contrast', 'the theme is back after reloading the window');
    await openSettings();
    await choose(/^Light/);
    h.equal(await theme(), 'light', 'Light is a choice of its own');
    const light = (await background()).match(/\d+/g).slice(0, 3).map(Number).every((n) => n > 200);
    h.check(light, 'Light: the background is light');
    await choose(/^Follow Windows/);
    h.equal(await theme(), null, 'Follow Windows removes the attribute again');
  });

  await S('the text size changes everything in the window, is remembered, and can be put back', async () => {
    await openSettings('Interface');
    const standard = await rootSize();
    h.equal(standard, 14, 'the standard text size is 14 pixels');
    const sampleBefore = (await page.getByTestId('text-sample').boundingBox()).height;
    await choose(/^130 percent/);
    const big = await rootSize();
    h.check(Math.abs(big - 18.2) < 0.1, `130 percent is about 18.2 pixels: ${big}`);
    h.equal((await stored()).textSize, 130, 'it is remembered');
    const sampleAfter = (await page.getByTestId('text-sample').boundingBox()).height;
    h.check(sampleAfter > sampleBefore * 1.2, `the sample grew: ${Math.round(sampleBefore)} to ${Math.round(sampleAfter)}`);
    const ribbonBefore = 44;
    const ribbon = (await page.getByRole('toolbar', { name: 'Commands' }).boundingBox()).height;
    h.check(ribbon > ribbonBefore, `the ribbon grew with the text: ${Math.round(ribbon)}`);
    // The page does not scroll sideways at the larger size.
    const overflow = await html(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    h.check(overflow <= 1, `no sideways scrolling at 130 percent: ${overflow}`);
    await axe('the Settings page at 130 percent');
    // The panels in Code grow with the text, so they hold the same amount of it.
    await page.getByRole('button', { name: 'Reset panel sizes' }).click();
    await mainTab('Coding').click();
    await page.getByRole('separator', { name: 'Resize the side panel' }).waitFor();
    const left130 = parseInt(await cssVar('--left'), 10);
    h.check(Math.abs(left130 - 419) <= 1, `the side panel grew with the text, 322 x 1.3: ${left130}`);
    const passages130 = parseInt(await cssVar('--passages'), 10);
    // Retrieved Segments is never smaller than its toolbar and a few rows: 120 x 1.3.
    h.check(passages130 >= 155, `and Retrieved Segments kept its smallest height, 120 x 1.3: ${passages130}`);
    await openSettings();
    await page.reload();
    await tab().waitFor();
    h.check(Math.abs((await rootSize()) - 18.2) < 0.1, 'the size is back after reloading the window');
    await openSettings('Interface');
    await page.getByRole('button', { name: 'Reset all settings' }).click();
    h.equal(await rootSize(), 14, 'resetting puts the standard size back');
    h.equal(await theme(), null, 'and the standard colors');
    h.equal(await stored(), null, 'and forgets what was stored');
  });

  await S('the caret announcements can be turned off and on', async () => {
    const box = () => page.getByRole('checkbox', { name: /Say which codes the caret is in/ });
    const toggle = () => page.locator('label').filter({ has: box() }).click();
    /** A point inside the first coded words, or the first uncoded words. */
    const pointIn = (coded) =>
      page.evaluate((wantCoded) => {
        const els = [...document.querySelectorAll('[data-testid=surface] .seg')].filter((e) => e.classList.contains('coded') === wantCoded && e.getBoundingClientRect().width > 30);
        const el = els[0];
        if (!el) return null;
        el.scrollIntoView({ block: 'center' });
        const r = el.getBoundingClientRect();
        return { x: r.left + 12, y: r.top + r.height / 2 };
      }, coded);
    const moveCaretInto = async () => {
      // The caret note keeps quiet for a few seconds after something else has been announced (the tab change, the
      // setting), so it does not talk over it; wait that out so the check is a fair one.
      await sleep(2800);
      const outside = await pointIn(false);
      const inside = await pointIn(true);
      h.check(!!outside && !!inside, 'the text has coded and uncoded words to move between');
      await page.mouse.click(outside.x, outside.y);
      await sleep(700);
      const again = await pointIn(true);
      await page.mouse.click(again.x, again.y);
    };
    const watch = () =>
      page.getByTestId('status').evaluate((el) => {
        el.dataset.seen = '';
        new MutationObserver(() => (el.dataset.seen += `${el.textContent}|`)).observe(el, { childList: true, characterData: true, subtree: true });
      });
    const seen = () => page.getByTestId('status').evaluate((el) => el.dataset.seen ?? '');

    await openSettings('Accessibility');
    h.check(await box().isChecked(), 'on by default');
    await toggle();
    h.check(!(await box().isChecked()), 'turned off');
    h.equal((await stored()).announceCaret, false, 'remembered');
    await mainTab('Coding').click();
    await surface().waitFor();
    await watch();
    await moveCaretInto();
    await sleep(1200);
    h.check(!/(^|\|)In /.test(await seen()), `with it off, moving into coded words says nothing about codes: ${(await seen()).slice(0, 120)}`);

    await openSettings('Accessibility');
    await toggle();
    h.check(await box().isChecked(), 'turned on again');
    await mainTab('Coding').click();
    await surface().waitFor();
    await watch();
    await moveCaretInto();
    let said = '';
    for (let i = 0; i < 30 && !/(^|\|)In /.test(said); i++) {
      await sleep(100);
      said = await seen();
    }
    h.check(/(^|\|)In /.test(said), `with it on, the caret says which codes it is in: ${said.slice(0, 120)}`);
  });

  await S('Settings lists the book\'s eleven sections, and General has its location, autosave, backup, language, and theme', async () => {
    await openSettings();
    const names = (await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button').allTextContents()).map((t) => t.trim());
    h.equal(names, ['General', 'Interface', 'Project Defaults', 'Coding', 'Analysis', 'AI Assistant', 'Team & Sharing', 'Privacy & Security', 'Accessibility', 'Updates', 'About'], 'the sections are the book\'s, in its order');
    h.equal(await page.getByRole('navigation', { name: 'Settings sections' }).locator('[aria-current="page"]').innerText(), 'General', 'General is the one shown');
    for (const heading of ['Default Project Location', 'Autosave', 'Backup', 'Language', 'Theme']) {
      h.equal(await page.getByRole('heading', { level: 4, name: new RegExp(`^${heading}`) }).count(), 1, `General has ${heading}`);
    }
    // Autosave tells the truth: there is no timer, every change is written as it is made.
    h.check(/saved to the project file as you make it/.test(await page.getByTestId('autosave-state').innerText()), 'Autosave says each change is written as it is made');
    // The theme is four cards, and the chosen one is marked.
    const themes = await page.getByRole('radiogroup', { name: 'Theme' }).getByRole('radio').evaluateAll((els) => els.map((e) => ({ name: e.closest('label')?.querySelector('.bk-theme-name')?.textContent, on: e.checked })));
    h.equal(themes.map((t) => t.name), ['Follow Windows', 'Light', 'Dark', 'High Contrast'], 'the theme cards');
    h.equal(themes.filter((t) => t.on).map((t) => t.name), ['Follow Windows'], 'one is chosen');
    // Scope is told: these belong to this computer, not to a project.
    h.check((await page.getByTestId('settings-section').locator('.bk-scope').count()) >= 4, 'each group that applies to this computer says so');
    await page.screenshot({ path: `${ctx.shots}/settings-general.png`, fullPage: true });
    await axe('Settings, General');
  });

  await S('the default project location is kept only if the folder exists, and the last good one stays when it does not', async () => {
    await openSettings();
    const box = page.getByRole('textbox', { name: 'Folder new projects are saved in' });
    const stored = () => page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.projectFolder') ?? '""'));
    await box.fill(ctx.workDir);
    await box.press('Enter');
    await page.getByTestId('location-note').filter({ hasText: /Saved|already the default/ }).waitFor();
    h.equal(await stored(), ctx.workDir, 'a folder that exists is kept');
    await box.fill(`${ctx.workDir}\\no-such-folder`);
    await box.press('Enter');
    const error = page.getByTestId('location-error');
    await error.waitFor();
    h.check(/was not found/.test(await error.innerText()) && (await error.innerText()).includes(ctx.workDir), `the error says why and names the folder still in use: ${await error.innerText()}`);
    h.equal(await stored(), ctx.workDir, 'and the earlier folder is still the default');
    await axe('Settings with a location that was not found');
    // A file is not a folder.
    await box.fill(ctx.files.interview);
    await box.press('Enter');
    await page.getByTestId('location-error').filter({ hasText: /is a file, not a folder/ }).waitFor();
    h.equal(await stored(), ctx.workDir, 'a file is refused too');
    // New Project then offers the folder.
    await box.fill(ctx.workDir);
    await box.press('Enter');
    await page.getByTestId('location-note').filter({ hasText: /Saved|already the default/ }).waitFor();
    h.equal(await stored(), ctx.workDir, 'the last good folder is the default again');
  });

  await S('the number of backups can be chosen, and what new projects do is a project default', async () => {
    await openSettings();
    const keep = page.getByLabel('Recent backups kept for each project');
    h.equal(await keep.inputValue(), '10', 'ten are kept unless told otherwise');
    await keep.selectOption('5');
    h.equal((await page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.settings'))?.keepBackups)), 5, 'the choice is remembered on this computer');
    h.equal(await page.getByLabel('Backup location').inputValue(), 'project', 'backups are kept beside the project unless a folder is chosen');
    h.equal(await page.getByLabel('Interface language').isDisabled(), true, 'one language so far, shown');
    await keep.selectOption('10');
    await openSettings('Project Defaults');
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^Code whole units/ }) }).click();
    h.equal((await page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.settings'))?.codingMode)), 'unit', 'new projects will code whole units unless the form says otherwise');
    await axe('Settings, Project Defaults');
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^Select any words/ }) }).click();
  });

  await S('the name in the project history can be chosen, is kept on this computer, and is recorded on the next change; an empty name goes back to the one the program made', async () => {
    const rpc = (cmd, args) =>
      page.evaluate(async ([c, a]) => (await fetch('/rpc', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ cmd: c, args: a }) })).json(), [cmd, args]);
    const newestActor = async () => {
      const r = await rpc('events.list', { limit: 1 });
      return r.result[r.result.length - 1].actor;
    };
    await openSettings();
    const name = page.getByTestId('history-name');
    h.equal(await name.inputValue(), '', 'empty unless one was chosen: the program made a name for this computer');
    h.equal(await page.getByText('It is never your Windows sign-in name.', { exact: false }).count() > 0, true, 'the page says the Windows name is never used');
    await name.fill('Dr. Test Person');
    await name.press('Enter');
    h.equal((await stored()).historyName, 'Dr. Test Person', 'remembered on this computer');
    await rpc('events.annotate', { note: 'a change made under the chosen name' });
    h.equal(await newestActor(), 'Dr. Test Person', 'the next change is recorded under it');
    await axe('Settings, the name in the project history');
    await name.fill('   ');
    await name.press('Enter');
    h.equal((await stored()).historyName, '', 'only spaces is no name');
    await rpc('events.annotate', { note: 'a change made under the name the program made' });
    h.equal(await newestActor(), 'Ada Lovelace', 'and the name the program started with is back');
  });

  await S('the Updates section says plainly that updates cannot be checked in a browser window, and has no accessibility problems', async () => {
    await openSettings('Updates');
    await page.getByTestId('updates-outside').waitFor();
    h.check((await page.getByTestId('updates-outside').innerText()).includes('installed program'), 'it says updates are checked in the installed program');
    h.equal(await page.getByTestId('updates-check').count(), 0, 'and offers no button where nothing can be checked');
    await axe('Settings, Updates in a browser window');
  });

  await S('Check for updates asks only when pressed, shows the notes, asks again before installing, and says what went wrong in words', async () => {
    // The window's own `update.*` commands belong to the desktop program; here the page is given the answers it would get from it.
    const answers = { state: null, check: null, install: null };
    const asked = [];
    // The matcher and the handler are kept, because taking the interception off needs the very same two functions.
    const isRpc = (url) => url.pathname === '/rpc';
    const handler = async (route) => {
      const body = JSON.parse(route.request().postData() ?? '{}');
      if (typeof body.cmd === 'string' && body.cmd.startsWith('update.')) {
        asked.push(body.cmd);
        const answer = answers[body.cmd.slice('update.'.length)];
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(answer) });
      }
      return route.continue();
    };
    await page.route(isRpc, handler);
    const ok = (result) => ({ ok: true, result });
    const no = (code, message) => ({ ok: false, error: { code, message } });
    const setUp = { version: '0.1.0-beta.1', set_up: true, host: 'github.com', portable: false, blocked: null };
    const open = async () => {
      await openSettings('General');
      await openSettings('Updates');
      await page.getByTestId('updates-version').waitFor();
      // The line shows an ellipsis until the engine has answered; wait for the answer, so a busy computer cannot make a check read it too early.
      await page
        .waitForFunction(() => {
          const e = document.querySelector('[data-testid="updates-version"]');
          return e !== null && !e.textContent.includes('\u2026');
        }, null, { timeout: 5000 })
        .catch(() => {});
    };

    // Set up: the page says who is asked and what is sent, and nothing has been asked yet.
    answers.state = ok(setUp);
    await open();
    h.check((await page.getByTestId('updates-version').innerText()).includes('0.1.0-beta.1'), 'the version of this program is shown');
    h.check((await page.getByText('github.com', { exact: false }).count()) > 0, 'who would be asked is named');
    h.check((await page.getByText('Nothing from any project is sent.', { exact: false }).count()) > 0, 'and it says nothing from a project is sent');
    h.equal(asked.filter((c) => c !== 'update.state').length, 0, 'nothing was asked of the feed by opening the page');

    // A newer version: its number and notes, then a second step before anything is downloaded.
    answers.check = ok({ available: true, version: '0.2.0', notes: 'Fixes for the test.\n\n\n\nMore fixes.', date: '2026-11-13' });
    await page.getByTestId('updates-check').click();
    await page.getByTestId('updates-found').waitFor();
    h.check((await page.getByTestId('updates-found').innerText()).includes('Version 0.2.0 is available.'), 'the new version is named');
    h.check((await page.getByTestId('updates-found').innerText()).includes('Fixes for the test.'), 'with its notes');
    h.equal(asked.filter((c) => c === 'update.install').length, 0, 'nothing is downloaded yet');
    await page.getByTestId('updates-install').click();
    await page.getByTestId('updates-confirm').waitFor();
    h.check((await page.getByTestId('updates-confirm').innerText()).includes('check its signature'), 'it says the signature is checked');
    await axe('Settings, Updates, a newer version');
    await page.getByRole('button', { name: 'Not now', exact: true }).click();
    await page.getByTestId('updates-install').waitFor();
    h.equal(asked.filter((c) => c === 'update.install').length, 0, 'saying not now installs nothing');

    // Going ahead, and the signature not matching: refused in words, with nothing changed.
    answers.install = no('update_failed', 'the update file does not match its signature');
    await page.getByTestId('updates-install').click();
    await page.getByTestId('updates-go').click();
    await page.getByTestId('updates-failed').waitFor();
    const failed = await page.getByTestId('updates-failed').innerText();
    h.check(failed.includes('The update was not installed') && failed.includes('Nothing was changed.') && failed.includes('does not match its signature'), `the failure is said in words: ${failed}`);
    await axe('Settings, Updates, a failed update');

    // The newest version already.
    await open();
    answers.check = ok({ available: false });
    await page.getByTestId('updates-check').click();
    await page.getByTestId('updates-uptodate').waitFor();

    // The feed cannot be reached.
    await open();
    answers.check = no('update_failed', 'error sending request');
    await page.getByTestId('updates-check').click();
    await page.getByTestId('updates-failed').waitFor();
    h.check((await page.getByTestId('updates-failed').innerText()).includes('could not check for updates'), 'a check that fails says so');

    // The offline lock: no button, and the reason.
    answers.state = ok({ ...setUp, blocked: 'offline_lock' });
    await open();
    h.equal(await page.getByTestId('updates-check').isDisabled(), true, 'the button is off while the offline lock is on');
    h.check((await page.getByTestId('updates-blocked').innerText()).includes('offline lock'), 'and says why');
    h.equal(asked.filter((c) => c === 'update.check').length, 3, 'no check was made while it was locked');

    // Not set up in this build: the button is off and the releases page is named.
    answers.state = ok({ ...setUp, set_up: false, host: '', blocked: 'not_set_up' });
    await open();
    h.equal(await page.getByTestId('updates-check').isDisabled(), true);
    h.check((await page.getByTestId('updates-blocked').innerText()).includes('github.com/jasonswrench-beep/prismqda/releases'), 'the page where new versions are published is named');
    await axe('Settings, Updates, not set up');

    // A portable copy is told to download the new zip.
    answers.state = ok({ ...setUp, portable: true });
    await open();
    answers.check = ok({ available: true, version: '0.2.0', notes: '' });
    await page.getByTestId('updates-check').click();
    await page.getByTestId('updates-found').waitFor();
    h.equal(await page.getByTestId('updates-install').count(), 0, 'a portable copy has no install button');
    h.check((await page.getByTestId('updates-found').innerText()).includes('portable copy'), 'it says why');

    await page.unroute(isRpc, handler);
    await openSettings('General');
  });

  await S('the backup location can be a folder the person chooses; one that cannot be used is refused, with the last good choice kept', async () => {
    await openSettings();
    const where = page.getByLabel('Backup location');
    await where.selectOption('folder');
    const box = page.getByRole('textbox', { name: 'Folder for backups' });
    const kept = () => page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.settings'))?.backupFolder ?? '');
    h.equal(await kept(), '', 'choosing "A folder I choose" changes nothing until a folder is given');
    await box.fill(path.join(ctx.workDir, 'no-such-backup-folder'));
    await box.press('Enter');
    const error = page.getByTestId('backup-error');
    await error.waitFor();
    const said = await error.innerText();
    h.check(/was not found/.test(said) && /Backups stay beside each project/.test(said), `the error says why, and that backups stay where they were: ${said}`);
    h.equal(await kept(), '', 'the setting is unchanged');
    await axe('Settings with a backup folder that was not found');
    await box.fill(ctx.files.interview);
    await box.press('Enter');
    await page.getByTestId('backup-error').filter({ hasText: /is a file, not a folder/ }).waitFor();
    h.equal(await kept(), '', 'a file is refused too');
    // A folder that exists and accepts files is kept, and the next backup goes there.
    const good = path.join(ctx.workDir, 'my-backups');
    fs.mkdirSync(good, { recursive: true });
    await box.fill(good);
    await box.press('Enter');
    await page.getByTestId('backup-note').filter({ hasText: /Saved/ }).waitFor();
    h.equal(await kept(), good, 'a folder that can be used is kept');
    const made = await ctx.rpc('project.backup');
    h.check(made.path.startsWith(good) && /\.backups[\\/]/.test(made.path) && fs.existsSync(made.path), `the backup went to the chosen folder, in a folder of its own for the project: ${made.path}`);
    h.equal(made.note, null, 'with nothing to report');
    await axe('Settings with a backup folder chosen');
    // Back to beside the project.
    await where.selectOption('project');
    await page.getByTestId('backup-note').filter({ hasText: /beside each project/ }).waitFor();
    h.equal(await kept(), '', 'back to beside the project');
    const beside = await ctx.rpc('project.backup');
    h.check(!beside.path.startsWith(good), `and the next backup is not in the chosen folder: ${beside.path}`);
  });

  await S('Coding tells the rules of the open project and holds the coding bar switch, and Team & Sharing lists the coders', async () => {
    await openSettings('Coding');
    const bar = page.getByRole('checkbox', { name: /Show the coding bar/ });
    h.equal(await bar.isChecked(), true, 'the coding bar is on to begin with');
    const rules = await page.getByTestId('coding-rules').innerText();
    h.check(/How it codes/.test(rules) && /Mutually exclusive groups/.test(rules) && /Rated codes/.test(rules), `the project's rules are told: ${rules.replace(/\s+/g, ' ')}`);
    await page.locator('label').filter({ has: bar }).click();
    h.equal((await page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.settings'))?.floatingBar)), false, 'switching it off is remembered on this computer');
    await axe('Settings, Coding');
    await page.locator('label').filter({ has: bar }).click();
    await openSettings('Team & Sharing');
    const team = await page.getByTestId('settings-section').innerText();
    h.check(/\(you\)/.test(team) || /Nobody has coded in this project yet/.test(team), `the coders are listed, with one marked as you, or the project is said to have none: ${team.split('\n').slice(0, 4).join(' | ')}`);
    h.check(/settle their differences on the Team page/.test(team) && !/not built yet/.test(team), 'and sharing points to the Team page, and no longer says it is not built');
    await axe('Settings, Team and Sharing');
    await page.getByTestId('settings-open-team').click();
    await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();
    h.check(true, 'the button opens the Team page');
    await openSettings();
  });

  await S('a section that is not built says so, and Privacy, Updates, and About say what is true', async () => {
    for (const name of ['AI Assistant']) {
      await openSettings(name);
      h.check(!/Analysis settings are not built/.test(await page.getByTestId('settings-section').innerText()), `${name} opens`);
    }
    await openSettings('Privacy & Security');
    h.check(/connects to nothing on its own/.test(await page.getByTestId('settings-section').innerText()), 'Privacy says PrismQDA connects to nothing on its own');
    h.check(/This project is not encrypted/.test(await page.getByTestId('settings-section').innerText()), 'and that the project is not encrypted, with the way to encrypt it');
    await axe('Settings, Privacy and Security');
    await openSettings('Updates');
    h.check(/never looks for updates by itself|can only be checked in the installed program/.test(await page.getByTestId('settings-section').innerText()), 'Updates says it never looks for updates by itself (or, in a browser window, that it cannot check)');
    await openSettings('About');
    await page.waitForFunction(() => /Version \d/.test(document.querySelector('[data-testid=about-version]')?.textContent ?? ''), null, { timeout: 5000 });
    h.check(/Apache License 2\.0/.test(await page.getByTestId('settings-section').innerText()), 'About names the license');
    await axe('Settings, About');
    await openSettings();
  });

  await S('Analysis holds where the analysis pages begin: the confidence level, how pairs are compared, the bootstrap, and sub-codes; a change is remembered and can be put back', async () => {
    const tick = (name) => page.locator('label').filter({ has: page.getByRole('checkbox', { name }) }).click();
    await openSettings('Analysis');
    const text = await page.getByTestId('settings-section').innerText();
    h.check(/Confidence level of intervals/.test(text) && /Comparing pairs of groups/.test(text) && /Bootstrap/.test(text) && /sub-codes/.test(text), 'the four groups are there');
    h.check(!/not built yet/.test(text), 'and it no longer says it is not built');
    await axe('Settings, Analysis');
    try {
      await choose(/^99%/);
      await choose(/^Tukey/);
      await choose('5,000 resamples');
      await tick(/Begin every bootstrap with the same seed/);
      await page.getByTestId('analysis-seed').fill('17');
      await tick(/A code also stands for its sub-codes/);
      const kept = await stored();
      h.check(kept.confidenceLevel === 0.99 && kept.pairMethod === 'tukey' && kept.bootstrapResamples === 5000 && kept.fixedSeed === 17 && kept.subcodes === false, `the choices are remembered on this computer: ${JSON.stringify(kept)}`);
    } finally {
      // Put the choices back, so that the pages that follow begin as they always did.
      await openSettings('Analysis');
      await choose(/^95%/);
      await choose(/^Holm/);
      await choose('2,000 resamples');
      if ((await stored()).fixedSeed !== null) await tick(/Begin every bootstrap with the same seed/);
      if ((await stored()).subcodes === false) await tick(/A code also stands for its sub-codes/);
    }
    const back = await stored();
    h.check(back.confidenceLevel === 0.95 && back.pairMethod === 'holm' && back.bootstrapResamples === 2000 && back.fixedSeed === null && back.subcodes === true, 'and put back');
    await openSettings();
  });

  await S('the panels in Code can be put back to their standard sizes', async () => {
    await mainTab('Coding').click();
    const left = page.getByRole('separator', { name: 'Resize the side panel' });
    await left.waitFor();
    await left.focus();
    for (let i = 0; i < 4; i++) await page.keyboard.press('ArrowRight');
    const moved = parseInt(await cssVar('--left'), 10);
    await openSettings();
    await page.getByRole('button', { name: 'Reset panel sizes' }).click();
    await mainTab('Coding').click();
    await page.getByRole('separator', { name: 'Resize the side panel' }).waitFor();
    const back = parseInt(await cssVar('--left'), 10);
    h.equal(back, 322, `the side panel is back to its standard width (it was ${moved})`);
  });
}
