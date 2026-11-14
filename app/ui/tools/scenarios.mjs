// The scenarios the browser check walks through. Each step is named, counts its checks, and takes
// a screenshot into test-results/ when it fails.

import fs from 'node:fs';
import path from 'node:path';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function runScenarios(ctx) {
  const { h, browser, base, files, workDir, shots, rpc } = ctx;
  const context = await browser.newContext({ viewport: { width: 1366, height: 820 }, locale: 'en-US', colorScheme: 'light', reducedMotion: 'no-preference' });
  const page = await context.newPage();
  // A step that cannot find its control should fail in seconds, not after half a minute.
  page.setDefaultTimeout(10000);
  const problems = [];
  page.on('pageerror', (e) => problems.push(`page error: ${e.message}`));
  let allowAbort = false;
  page.on('console', (m) => {
    // A request the check cancels on purpose (to test the engine-unreachable message) logs one of these.
    if (m.type() === 'error' && !(allowAbort && /ERR_FAILED/.test(m.text()))) problems.push(`console error: ${m.text()}`);
  });

  const axeTotals = { screens: 0, violations: 0 };
  const knownDisabled = []; // rules switched off, each with its reason, are listed here and in the README

  /** Runs axe on the page (or one part of it) and records any violation as a failed check. */
  async function axe(label, opts = {}) {
    let builder = new ctx.AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa', 'best-practice']);
    if (opts.include) builder = builder.include(opts.include);
    if (opts.exclude) builder = builder.exclude(opts.exclude);
    const off = [...knownDisabled, ...(opts.disable ?? [])];
    if (off.length) builder = builder.disableRules(off);
    const r = await builder.analyze();
    axeTotals.screens++;
    axeTotals.violations += r.violations.length;
    const related = (n) => [...(n.any ?? []), ...(n.all ?? []), ...(n.none ?? [])].flatMap((c) => (c.relatedNodes ?? []).map((x) => x.target.join(' '))).slice(0, 4);
    const detail = r.violations
      .map((v) => `${v.id} [${v.impact}] ${v.help}: ${v.nodes.slice(0, 4).map((n) => n.target.join(' ') + ` <${(n.html ?? '').slice(0, 120)}>` + (related(n).length ? ` (related: ${related(n).join(', ')})` : '')).join(' | ')}`)
      .join(' ;; ');
    h.check(r.violations.length === 0, `axe on ${label} found ${r.violations.length} problem(s): ${detail}`);
  }

  let shotNumber = 0;
  // PRISMQDA_ONLY=images (or language, team, settings, and the other groups listed below) runs the welcome and new-project steps and then only that scenario, to save time while it is worked on.
  const onlyName = process.env.PRISMQDA_ONLY ?? '';
  const only = onlyName === 'images' || onlyName === 'language' || onlyName === 'team' || onlyName === 'merge' || onlyName === 'themes' || onlyName === 'text' || onlyName === 'stats' || onlyName === 'dataeditor' || onlyName === 'mixed' || onlyName === 'pdf' || onlyName === 'clips' || onlyName === 'class' || onlyName === 'privacy' || onlyName === 'encryption' || onlyName === 'map' || onlyName === 'datatools' || onlyName === 'shots' || onlyName === 'shots2' || onlyName === 'settings';
  let inOnly = false;
  let stepsSeen = 0;
  async function S(name, fn) {
    stepsSeen++;
    if (only && !inOnly && stepsSeen > 4) return;
    const before = h.results.length;
    await h.step(name, fn);
    const entry = h.results[before];
    if (entry && entry.failures.length) {
      shotNumber++;
      const file = path.join(shots, `fail-${String(shotNumber).padStart(2, '0')}.png`);
      await page.screenshot({ path: file }).catch(() => {});
      console.log(`       screenshot: ${file}`);
      // Whether the engine still answers tells a broken screen from an engine that is stuck.
      const t0 = Date.now();
      const alive = await Promise.race([ctx.rpc('project.info').then(() => true, () => false), sleep(8000).then(() => null)]);
      console.log(`       engine ${alive === null ? 'did not answer within 8 s' : alive ? `answered in ${Date.now() - t0} ms` : 'refused the call'}`);
    }
  }

  const status = () => page.getByTestId('status');
  const alertBox = () => page.getByTestId('alert');
  async function waitStatus(re, ms = 8000) {
    const t0 = Date.now();
    let text = '';
    while (Date.now() - t0 < ms) {
      text = (await status().textContent()) ?? '';
      if (re.test(text)) return text;
      await sleep(60);
    }
    return text;
  }
  const surface = () => page.getByTestId('surface');

  /** The text of the document as the browser holds it. */
  const surfaceText = () => surface().evaluate((el) => el.textContent);

  /** Where a phrase is on screen, as points for a mouse drag. */
  async function phraseBox(phrase) {
    return page.evaluate((p) => {
      const root = document.querySelector('[data-testid=surface]');
      const text = root.textContent;
      const i = text.indexOf(p);
      if (i < 0) return null;
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      let pos = 0;
      let node;
      let sNode;
      let sOff = 0;
      let eNode;
      let eOff = 0;
      while ((node = walker.nextNode())) {
        const len = node.length;
        if (!sNode && i < pos + len) {
          sNode = node;
          sOff = i - pos;
        }
        if (i + p.length <= pos + len) {
          eNode = node;
          eOff = i + p.length - pos;
          break;
        }
        pos += len;
      }
      if (!sNode || !eNode) return null;
      sNode.parentElement.scrollIntoView({ block: 'center' });
      const whole = document.createRange();
      whole.setStart(sNode, sOff);
      whole.setEnd(eNode, eOff);
      const rects = [...whole.getClientRects()].filter((r) => r.width > 0);
      const first = rects[0];
      const last = rects[rects.length - 1];
      if (Math.abs(first.top - last.top) > first.height / 2) {
        // The phrase wraps onto a second line, so one straight drag cannot cover it (the browser's own rules for
        // dragging across a line break in right-to-left text are not ours to test). The caller sets the selection
        // through the selection API instead, which still goes through the document's own selection handling.
        // Callers that only click on the phrase get its first line.
        return { x0: first.left + 0.5, y0: first.top + first.height / 2, x1: first.right - 0.5, y1: first.top + first.height / 2, wrapped: true };
      }
      const left = Math.min(...rects.map((r) => r.left));
      const right = Math.max(...rects.map((r) => r.right));
      const mid = first.top + first.height / 2;
      // From the left edge of the phrase to its right edge, so right-to-left text works too.
      return { x0: left + 0.5, y0: mid, x1: right - 0.5, y1: mid };
    }, phrase);
  }

  /** Selects a phrase the way a person would with the mouse. */
  async function selectByMouse(phrase) {
    // Wait until the layout has stopped moving (the margin can widen right after a coding).
    let box = await phraseBox(phrase);
    for (let i = 0; i < 20; i++) {
      await sleep(60);
      const again = await phraseBox(phrase);
      if (again && box && JSON.stringify(again) === JSON.stringify(box)) break;
      box = again;
    }
    if (!box) throw new Error(`the phrase is not on screen: ${phrase}`);
    if (box.wrapped) {
      return page.evaluate((p) => {
        const root = document.querySelector('[data-testid=surface]');
        const i = root.textContent.indexOf(p);
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
        let pos = 0;
        let node;
        let s = null;
        let so = 0;
        let e = null;
        let eo = 0;
        while ((node = walker.nextNode())) {
          const len = node.length;
          if (!s && i < pos + len) {
            s = node;
            so = i - pos;
          }
          if (i + p.length <= pos + len) {
            e = node;
            eo = i + p.length - pos;
            break;
          }
          pos += len;
        }
        root.focus();
        window.getSelection().setBaseAndExtent(s, so, e, eo);
        return window.getSelection().toString();
      }, phrase);
    }
    // A first click puts the caret there; the browser may scroll a half-hidden line into view when
    // the text takes the focus, so settle that before measuring for the real drag.
    await page.mouse.click(box.x0, box.y0);
    await sleep(150);
    box = await phraseBox(phrase);
    for (let i = 0; i < 20; i++) {
      await sleep(60);
      const again = await phraseBox(phrase);
      if (again && box && JSON.stringify(again) === JSON.stringify(box)) break;
      box = again;
    }
    if (process.env.DEBUG_BOX) {
      console.log('DEBUG box', JSON.stringify(box), await page.evaluate(([x, y]) => { const e = document.elementFromPoint(x, y); return e ? e.tagName + '.' + e.className + ' ' + (e.textContent ?? '').slice(0, 20) : 'none'; }, [box.x0, box.y0]));
      await page.evaluate(([a, b, c, d]) => { for (const [x, y, col] of [[a, b, 'red'], [c, d, 'blue']]) { const m = document.createElement('div'); m.className = 'debug-marker'; m.style.cssText = `position:fixed;left:${x - 3}px;top:${y - 3}px;width:6px;height:6px;background:${col};z-index:9999`; document.body.appendChild(m); } }, [box.x0, box.y0, box.x1, box.y1]);
      await page.screenshot({ path: path.join(shots, `debug-box-${phrase.length}.png`) });
      await page.evaluate(() => document.querySelectorAll('.debug-marker').forEach((m) => m.remove()));
    }
    await page.mouse.move(box.x0, box.y0);
    await page.mouse.down();
    if (process.env.DEBUG_DRAG) {
      console.log('DEBUG after down', await page.evaluate(([p]) => {
        const sc = document.querySelector('.surface-scroll');
        const s = getSelection();
        const again = (() => { const root = document.querySelector('[data-testid=surface]'); const t = root.textContent; return t.indexOf(p); })();
        return JSON.stringify({ scrollTop: sc.scrollTop, anchor: s.anchorNode && s.anchorNode.textContent.slice(0, 12), off: s.anchorOffset, again, win: [window.scrollX, window.scrollY, document.querySelector('.center').scrollTop] });
      }, [phrase]));
    }
    await page.mouse.move((box.x0 + box.x1) / 2, (box.y0 + box.y1) / 2, { steps: 4 });
    await page.mouse.move(box.x1, box.y1, { steps: 4 });
    await page.mouse.up();
    return page.evaluate(() => window.getSelection().toString());
  }

  /** Puts the caret `graphemes` characters from the start of the text, using only the keyboard. */
  async function caretFromStart(graphemes) {
    await surface().focus();
    await page.keyboard.press('Control+Home');
    for (let i = 0; i < graphemes; i++) await page.keyboard.press('ArrowRight');
  }

  const graphemesBefore = (text, phrase) => {
    const i = text.indexOf(phrase);
    return [...new Intl.Segmenter('en', { granularity: 'grapheme' }).segment(text.slice(0, i))].length;
  };

  const codeRow = (name) => page.getByRole('row', { name: new RegExp(`^${name},`) });
  // The pages of the program open from the View tab of the ribbon (and Settings from the gear); the coding workspace is Home.
  // The old names still work: Coding and Code are the workspace, Memos is the memos page, History is the overview (then Activity).
  const PAGE_COMMANDS = { Home: 'Project overview', History: 'Project overview', Data: 'Data table', Synthesize: 'Memos page', Memos: 'Memos page', Report: 'Report page' };
  const mainTab = (name) => ({
    async click() {
      if (name === 'Coding' || name === 'Code') {
        const back = page.getByRole('button', { name: 'Back to the coding workspace' });
        if (await back.count()) await back.click();
        await page.getByRole('tab', { name: 'Home', exact: true }).click();
        return;
      }
      if (name === 'Settings') {
        await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
        return;
      }
      await page.getByRole('tab', { name: 'View', exact: true }).click();
      await page.getByRole('button', { name: PAGE_COMMANDS[name] ?? name, exact: true }).click();
    },
    async waitFor(opts) {
      await page.getByRole('tablist', { name: 'Ribbon tabs' }).waitFor(opts);
    },
  });
  /** The less-used codebook actions live in the "More code actions" menu: open it and choose one. */
  const codeAction = async (name) => {
    await page.getByRole('button', { name: 'More code actions' }).click();
    await page.getByRole('menuitem', { name, exact: true }).click();
  };
  /** Import Data is a page: reach it from wherever the check is (the Documents pane has the plus). */
  const goToImport = async () => {
    const heading = page.getByRole('heading', { level: 2, name: 'Import Data' });
    if ((await heading.count()) === 0) {
      await mainTab('Coding').click();
      await page.getByRole('button', { name: 'Add documents', exact: true }).first().click();
    }
    await heading.waitFor();
  };
  /** Closing the project is in the project menu of the title bar; the welcome page comes back. */
  const closeProjectViaMenu = async () => {
    await page.getByTestId('project-name').click();
    await page.getByRole('menuitem', { name: 'Close project' }).click();
    await page.getByRole('heading', { level: 2, name: 'Welcome to PrismQDA' }).waitFor();
  };
  /** A code's definition opens in a window of its own: select the code in Codes, then "Open definition…". */
  const editorHeading = () => page.getByRole('heading', { level: 2, name: 'Codebook Editor' });
  const openDefinition = async (name) => {
    if (name) await codeRow(name).click();
    await codeAction('Open definition…');
    await editorHeading().waitFor();
  };
  /** Back from the Codebook Editor to the coding workspace. */
  const closeDefinition = async () => {
    await page.getByRole('button', { name: 'Back to the coding workspace' }).first().click();
    await editorHeading().waitFor({ state: 'hidden' });
    await page.getByRole('heading', { level: 2, name: 'Documents' }).waitFor();
  };
  /** Segment Details keeps its changes behind "Options for this coding": open them if they are closed. */
  const openCodingOptions = async () => {
    const toggle = page.getByTestId('coding-detail').getByRole('button', { name: 'Options for this coding' });
    if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
  };
  /** Codings here is a collapsed list under the text: open it if it is closed. */
  const openCodingsHere = async () => {
    const box = page.getByTestId('codings-here');
    if (!(await box.evaluate((el) => el.open))) await box.locator('summary').click();
  };
  /** The two check-mark choices in that menu ("Show passages of this code", "Show retired and merged codes"): open it, press one, close it, say whether it is now checked. */
  const codeToggle = async (name) => {
    const menu = page.getByRole('menu', { name: 'More code actions' });
    await page.getByRole('button', { name: 'More code actions' }).click();
    await page.getByRole('menuitemcheckbox', { name, exact: true }).click();
    await menu.waitFor({ state: 'hidden' }); // choosing one closes the menu, so the change is in view
    await page.getByRole('button', { name: 'More code actions' }).click();
    const checked = await page.getByRole('menuitemcheckbox', { name, exact: true }).getAttribute('aria-checked');
    await page.keyboard.press('Escape');
    await menu.waitFor({ state: 'hidden' });
    return checked === 'true';
  };
  /** History is the Activity view under Home. */
  const openActivity = async () => {
    await mainTab('Home').click();
    await page.getByRole('navigation', { name: 'Home views' }).getByRole('button', { name: 'Activity' }).click();
  };

  async function addCodeThroughDialog(name, definition = '', parent = null) {
    await page.getByRole('button', { name: 'Add code', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Add a code' });
    await dialog.getByRole('textbox', { name: 'Name' }).fill(name);
    if (definition) await dialog.getByRole('textbox', { name: /Definition/ }).fill(definition);
    if (parent) await dialog.getByLabel('Place it under').selectOption({ label: parent });
    await dialog.getByRole('button', { name: 'Add code' }).click();
    await dialog.waitFor({ state: 'hidden' });
  }

  // ===========================================================================================
  await S('the welcome page opens in the layout of the book and has no accessibility problems', async () => {
    await page.goto(base);
    await page.getByRole('heading', { level: 2, name: 'Welcome to PrismQDA' }).waitFor();
    h.check(await page.getByRole('heading', { level: 1, name: 'PrismQDA' }).isVisible(), 'the title bar names the program');
    h.check(await page.getByRole('link', { name: 'Skip to the main content' }).count() === 1, 'there is a skip link');
    h.check(await page.getByText('Qualitative, quantitative, and mixed methods analysis in one project.').isVisible(), 'the line under the title is the book\'s');
    for (const n of ['New Project', 'Open Project', 'Sample Projects']) h.equal(await page.getByRole('button', { name: n, exact: true }).count(), 1, `${n} is one of the three ways to begin`);
    const rail = await page.getByRole('navigation', { name: 'Destinations' }).getByRole('button').evaluateAll((els) => els.map((e) => ({ name: e.textContent.trim(), current: e.getAttribute('aria-current'), disabled: e.getAttribute('aria-disabled') })));
    h.equal(rail.map((r) => r.name), ['Home', 'Data', 'Code', 'Explore', 'Text Analytics', 'Statistics', 'Synthesize', 'Report', 'Team', 'Settings'], 'the rail lists the destinations in the book\'s order');
    h.equal(rail[0].current, 'page', 'Home is the current one');
    h.check(rail.filter((r) => r.name !== 'Home' && r.name !== 'Settings').every((r) => r.disabled === 'true'), 'the ones that need a project say so');
    h.equal(rail.find((r) => r.name === 'Settings').disabled, null, 'Settings can be opened before a project exists');
    for (const n of ['Recent Projects', 'Get Started']) h.equal(await page.getByRole('heading', { level: 2, name: n }).count(), 1, `a heading for ${n}`);
    const steps = await page.getByRole('region', { name: 'Get Started' }).getByRole('button').evaluateAll((els) => els.map((e) => e.textContent.trim()));
    h.equal(steps, ['Import your data', 'Create a codebook', 'Code your first document', 'Explore and analyze', 'Build a report'], 'the five steps of Get Started');
    h.check(await page.getByText(/No recent projects yet/).isVisible(), 'with no recent project there is a short message and the three ways to begin stay');
    await axe('the welcome page');
  });

  await S('the three ways to begin, the recent list, and Get Started sit side by side as in the picture', async () => {
    const m = await page.evaluate(() => {
      const box = (el) => el.getBoundingClientRect();
      const find = (name) => [...document.querySelectorAll('.bk-action')].find((e) => e.textContent.trim() === name);
      const actions = ['New Project', 'Open Project', 'Sample Projects'].map((n) => box(find(n)));
      const recent = box(document.querySelector('.bk-recent'));
      const started = box(document.querySelector('.bk-getstarted'));
      const rail = box(document.querySelector('.bk-rail'));
      const mainBox = box(document.querySelector('.bk-launcher-main'));
      const primary = getComputedStyle(find('New Project')).backgroundImage;
      return { tops: actions.map((a) => Math.round(a.top)), lefts: actions.map((a) => a.left), recentLeft: recent.left, startedLeft: started.left, recentTop: Math.round(recent.top), startedTop: Math.round(started.top), railRight: rail.right, mainLeft: mainBox.left, primary };
    });
    h.equal(new Set(m.tops).size, 1, 'the three cards are in one row');
    h.check(m.lefts[0] < m.lefts[1] && m.lefts[1] < m.lefts[2], 'New Project, Open Project, Sample Projects, left to right');
    h.check(m.recentLeft < m.startedLeft && m.recentTop === m.startedTop, 'Recent Projects is left of Get Started, at the same height');
    h.check(m.railRight <= m.mainLeft + 1, 'the rail is on the left of the page');
    h.check(/gradient/.test(m.primary), 'New Project is the one filled, primary card');
  });

  await S('a mistake on New Project is shown at its field, and cancelling leaves nothing behind', async () => {
    await page.getByRole('button', { name: 'New Project', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'New Project' }).waitFor();
    h.check(await page.getByRole('navigation', { name: 'You are here' }).isVisible(), 'the title bar says Welcome / New Project');
    await axe('New Project');
    await page.getByRole('button', { name: 'Create Project' }).click();
    await page.getByText('Give the project a name.').waitFor();
    await page.getByRole('textbox', { name: 'Project name' }).fill('Study: A');
    await page.getByRole('button', { name: 'Create Project' }).click();
    await page.getByText(/cannot contain ":"/).waitFor();
    await page.getByRole('textbox', { name: 'Project name' }).fill('Study A');
    await page.getByRole('button', { name: 'Create Project' }).click();
    await page.getByText('Choose where to save the project.').waitFor();
    // Where the file will go is told before anything is written.
    await page.getByRole('textbox', { name: 'Save location' }).fill(workDir);
    const where = await page.getByTestId('np-where').textContent();
    h.check(/Study A\.pqda/.test(where ?? '') && (where ?? '').includes(workDir), `the file name and folder are shown first: ${where}`);
    await page.getByRole('button', { name: 'Cancel' }).click();
    await page.getByRole('heading', { level: 2, name: 'Welcome to PrismQDA' }).waitFor();
    let created = true;
    try {
      await rpc('project.info');
    } catch {
      created = false;
    }
    h.equal(created, false, 'cancelling made no project');
    h.equal(fs.existsSync(path.join(workDir, 'Study A.pqda')), false, 'and no file');
    // Opening a file that is not there says so, and the welcome page stays.
    await page.getByRole('button', { name: 'Open Project', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Open Project' });
    await dialog.getByRole('textbox', { name: 'Project file' }).fill(path.join(workDir, 'missing.pqda'));
    await dialog.getByRole('button', { name: 'Open Project' }).click();
    const notice = dialog.locator('.notice-error').first();
    await notice.waitFor();
    const msg = await notice.textContent();
    h.check(/does not exist|cannot|No such|not find/i.test(msg ?? ''), `opening a missing file explains the problem: ${msg}`);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await dialog.waitFor({ state: 'hidden' });
  });

  await S('creating a project opens the workspace', async () => {
    await page.getByRole('button', { name: 'New Project', exact: true }).click();
    await page.getByRole('textbox', { name: 'Project name' }).fill('Study A');
    await page.getByRole('textbox', { name: 'Save location' }).fill(workDir);
    await page.getByRole('button', { name: 'Create Project' }).click();
    await mainTab('Coding').waitFor();
    const info = await rpc('project.info');
    h.equal(info.name, 'Study A', 'project name');
    h.equal(info.coding_mode, 'free_span', 'coding mode');
    h.check(info.path.endsWith('Study A.pqda'), `the file is named from the project: ${info.path}`);
    h.check((await waitStatus(/Created the project Study A/)).includes('Created the project Study A'), 'the creation is announced');
    // The four areas are real landmarks with headings, named as in the book.
    for (const name of ['Documents', 'Codes', 'Retrieved Segments', 'Segment Details']) {
      h.check(await page.getByRole('heading', { level: 2, name: new RegExp(`^${name}`) }).isVisible(), `a heading for ${name}`);
    }
    h.check(await page.getByRole('main').count() === 1, 'there is exactly one main landmark');
    h.check(await page.getByRole('complementary', { name: 'Documents and codes' }).count() === 1, 'the side panel is a landmark');
    h.check(await page.getByRole('complementary', { name: 'Inspector' }).count() === 1, 'the inspector is a landmark');
    h.check(await page.getByRole('region', { name: 'Documents', exact: true }).count() === 1, 'Documents is a region landmark');
    h.check(await page.getByRole('region', { name: 'Command ribbon' }).count() === 1, 'the command ribbon is a landmark');
    h.check(await page.getByRole('tablist', { name: 'Ribbon tabs' }).count() === 1, 'the ribbon tabs are a tab list');
    h.check(await page.getByRole('region', { name: 'Codes', exact: true }).count() === 1, 'Codes is a region landmark');
    h.check(await page.getByRole('region', { name: 'Segment Details' }).count() === 1, 'Segment Details is a region landmark');
    h.check(await page.getByRole('region', { name: /^Retrieved Segments/ }).count() === 1, 'Retrieved Segments is a region landmark');
    h.check(await page.getByRole('banner').count() === 1, 'there is a banner');
    h.check(await page.getByRole('button', { name: 'Add documents', exact: true }).first().isVisible(), 'Add documents is in the reader, where the first document goes');
    await axe('the empty workspace');
  });

  await S('importing a text file shows what will come in, then the report', async () => {
    await page.getByRole('button', { name: 'Add Documents', exact: true }).click();
    const dialog = page.getByRole('main', { name: 'Import Data' });
    await dialog.waitFor();
    await axe('the import dialog (first step)');
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(files.interview);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    const text = await dialog.textContent();
    h.check(text.includes('interview'), 'the source name is shown');
    h.check(/Characters/.test(text) && /Units/.test(text), 'characters and units are shown');
    h.check(/No warnings/.test(text), 'no warnings for a clean text file');
    await axe('the import dialog (review step)');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const finished = page.getByTestId('import-finished');
    await finished.waitFor();
    h.check((await finished.textContent()).includes('Imported 1 source'), 'the report says what was imported');
    await axe('the import dialog (finished step)');
    await finished.getByRole('button', { name: 'Done' }).click();
    await finished.waitFor({ state: 'hidden' });
    await page.getByRole('row', { name: /^interview/ }).waitFor();
    await surface().waitFor();
    const t = await surfaceText();
    h.check(t.includes('I felt welcome from the first day.'), 'the text is shown in the document view');
    h.check(t.includes('\u{1F600}') && t.includes('שלום') && t.includes('我感到'), 'emoji, Hebrew, and CJK text are shown');
    await page.screenshot({ path: path.join(shots, 'workspace-document.png') });
  });

  await S('the document is a read-only text box with a caret', async () => {
    const attrs = await surface().evaluate((el) => ({
      role: el.getAttribute('role'),
      multiline: el.getAttribute('aria-multiline'),
      readonly: el.getAttribute('aria-readonly'),
      editable: el.getAttribute('contenteditable'),
      tab: el.tabIndex,
      name: el.getAttribute('aria-labelledby'),
    }));
    h.equal(attrs.role, 'textbox', 'role');
    h.equal(attrs.multiline, 'true', 'aria-multiline');
    h.equal(attrs.readonly, 'true', 'aria-readonly');
    h.equal(attrs.editable, 'true', 'contenteditable');
    h.equal(attrs.tab, 0, 'it is in the tab order');
    const named = page.getByRole('textbox', { name: 'interview' });
    h.check(await named.count() === 1, 'the text box has the source name as its accessible name');
    const before = await surfaceText();
    await surface().focus();
    await page.keyboard.type('hello 123');
    await page.keyboard.press('Backspace');
    await page.keyboard.press('Delete');
    await page.keyboard.press('Enter');
    // Enter in the text opens Code Selection (the way to code from the keyboard); it is not an edit.
    const codeBox = page.getByRole('dialog', { name: 'Apply a code' });
    await codeBox.waitFor();
    // The first Escape closes the list of matches if it is open, the next closes the box.
    await page.keyboard.press('Escape');
    if (!(await codeBox.waitFor({ state: 'hidden', timeout: 700 }).then(() => true, () => false))) await page.keyboard.press('Escape');
    await codeBox.waitFor({ state: 'hidden' });
    await surface().focus();
    await page.keyboard.press('Control+b');
    h.check((await surfaceText()) === before, 'typing, deleting, and Enter do not change the text');
    // Paste, cut, and drop are refused too.
    const refused = await surface().evaluate((el) => {
      const out = {};
      out.paste = !el.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true }));
      out.cut = !el.dispatchEvent(new ClipboardEvent('cut', { bubbles: true, cancelable: true }));
      out.drop = !el.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true }));
      out.beforeinput = !el.dispatchEvent(new InputEvent('beforeinput', { bubbles: true, cancelable: true, inputType: 'insertText', data: 'x' }));
      return out;
    });
    h.equal(refused, { paste: true, cut: true, drop: true, beforeinput: true }, 'paste, cut, drop, and beforeinput are all cancelled');
    // If an input method changes the text before it can be stopped, the text is put back.
    await surface().evaluate((el) => {
      const node = el.querySelector('.seg').firstChild;
      node.data = 'CHANGED BY AN INPUT METHOD';
      el.dispatchEvent(new Event('input', { bubbles: true }));
    });
    h.check((await surfaceText()) === before, 'text changed behind the interface\'s back is restored');
  });

  // The rest of the scenarios live in more files so that this one stays readable.
  const api = { setAllowAbort: (v) => { allowAbort = v; }, page, mainTab, openActivity, codeAction, codeToggle, closeProjectViaMenu, goToImport, editorHeading, openDefinition, closeDefinition, openCodingOptions, openCodingsHere, S, axe, status, alertBox, waitStatus, surface, surfaceText, phraseBox, selectByMouse, caretFromStart, graphemesBefore, codeRow, addCodeThroughDialog, sleep, problems, axeTotals, knownDisabled, context };
  if (only) {
    inOnly = true;
    if (onlyName === 'language') {
      const { runLanguage } = await import('./scenarios-language.mjs');
      await runLanguage(ctx, api);
    } else if (onlyName === 'team') {
      const { runTeam } = await import('./scenarios-team.mjs');
      await runTeam(ctx, api);
    } else if (onlyName === 'text') {
      const { runText } = await import('./scenarios-text.mjs');
      await runText(ctx, api);
    } else if (onlyName === 'stats') {
      const { runStats } = await import('./scenarios-stats.mjs');
      await runStats(ctx, api);
    } else if (onlyName === 'settings') {
      const { runSettings } = await import('./scenarios-settings.mjs');
      await runSettings(ctx, api);
    } else if (onlyName === 'shots2') {
      const { runShots2 } = await import('./scenarios-shots2.mjs');
      await runShots2(ctx, api);
    } else if (onlyName === 'shots') {
      const { runShots } = await import('./scenarios-shots.mjs');
      await runShots(ctx, api);
    } else if (onlyName === 'dataeditor') {
      const { runDataEditor } = await import('./scenarios-dataeditor.mjs');
      await runDataEditor(ctx, api);
    } else if (onlyName === 'mixed') {
      const { runMixed } = await import('./scenarios-mixed.mjs');
      await runMixed(ctx, api);
    } else if (onlyName === 'themes') {
      const { runThemes } = await import('./scenarios-themes.mjs');
      await runThemes(ctx, api);
    } else if (onlyName === 'merge') {
      const { runMerge } = await import('./scenarios-merge.mjs');
      await runMerge(ctx, api);
    } else if (onlyName === 'datatools') {
      const { runDataTools } = await import('./scenarios-datatools.mjs');
      await runDataTools(ctx, api);
    } else if (onlyName === 'pdf') {
      const { runPdf } = await import('./scenarios-pdf.mjs');
      await runPdf(ctx, api);
    } else if (onlyName === 'map') {
      const { runMap } = await import('./scenarios-map.mjs');
      await runMap(ctx, api);
    } else if (onlyName === 'clips') {
      const { runClips } = await import('./scenarios-clips.mjs');
      await runClips(ctx, api);
    } else if (onlyName === 'class') {
      const { runClass } = await import('./scenarios-class.mjs');
      await runClass(ctx, api);
    } else if (onlyName === 'privacy') {
      const { runPrivacy } = await import('./scenarios-privacy.mjs');
      await runPrivacy(ctx, api);
    } else if (onlyName === 'encryption') {
      const { runEncryption } = await import('./scenarios-encryption.mjs');
      await runEncryption(ctx, api);
    } else {
      const { runImages } = await import('./scenarios-image.mjs');
      await runImages(ctx, api);
    }
    console.log(`axe: ${axeTotals.screens} screens and dialogs checked, ${axeTotals.violations} violations`);
    await context.close();
    return;
  }
  const { runCoding } = await import('./scenarios-coding.mjs');
  await runCoding(ctx, api);
  const { runMore } = await import('./scenarios-more.mjs');
  await runMore(ctx, api);
  const { runData } = await import('./scenarios-data.mjs');
  await runData(ctx, api);
  const { runDisplay } = await import('./scenarios-display.mjs');
  await runDisplay(ctx, api);
  const { runExport } = await import('./scenarios-export.mjs');
  await runExport(ctx, api);
  const { runFrame } = await import('./scenarios-frame.mjs');
  await runFrame(ctx, api);
  const { runBar } = await import('./scenarios-bar.mjs');
  await runBar(ctx, api);
  const { runSettings } = await import('./scenarios-settings.mjs');
  await runSettings(ctx, api);
  const { runLook } = await import('./scenarios-look.mjs');
  await runLook(ctx, api);
  const { runEditor } = await import('./scenarios-editor.mjs');
  await runEditor(ctx, api);
  const { runLauncher } = await import('./scenarios-launcher.mjs');
  await runLauncher(ctx, api);

  await S('no script errors appeared in the console', async () => {
    h.equal(problems, [], 'console and page errors');
  });
  console.log(`axe: ${axeTotals.screens} screens and dialogs checked, ${axeTotals.violations} violations`);
  await context.close();
}
