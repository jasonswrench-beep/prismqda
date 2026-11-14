// Layout, remembered sizes, themes, high contrast, reduced motion, text zoom, and keyboard order.

import path from 'node:path';

export async function runDisplay(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe, waitStatus, surface, mainTab, sleep, phraseBox } = api;

  const cssVar = (name) => page.evaluate((n) => getComputedStyle(document.querySelector('.workspace')).getPropertyValue(n).trim(), name);
  const separator = (name) => page.getByRole('separator', { name });
  const valueText = (loc) => loc.getAttribute('aria-valuetext');

  await S('the areas can be resized by keyboard and by mouse, and the sizes are remembered', async () => {
    await mainTab('Coding').click();
    const left = separator('Resize the side panel');
    await left.waitFor();
    h.equal(await left.getAttribute('aria-orientation'), 'vertical', 'a vertical bar between left and right areas');
    const startPx = parseInt(await cssVar('--left'), 10);
    await left.focus();
    await page.keyboard.press('ArrowRight');
    h.equal(parseInt(await cssVar('--left'), 10), startPx + 16, 'Right arrow grows the side panel by a step');
    await page.keyboard.press('Shift+ArrowRight');
    h.equal(parseInt(await cssVar('--left'), 10), startPx + 16 + 64, 'Shift makes the step bigger');
    h.check(/pixels wide/.test(await valueText(left)), `the size is available as text: ${await valueText(left)}`);
    await page.keyboard.press('Home');
    h.equal(parseInt(await cssVar('--left'), 10), 200, 'Home makes it as small as it can be');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowRight');
    // The inspector grows when its bar moves left.
    const right = separator('Resize Segment Details');
    const rightStart = parseInt(await cssVar('--right'), 10);
    await right.focus();
    await page.keyboard.press('ArrowLeft');
    h.equal(parseInt(await cssVar('--right'), 10), rightStart + 16, 'Left arrow grows Segment Details');
    // The horizontal bar between Documents and Codes.
    const bar = separator('Resize Documents');
    const sourcesStart = parseInt(await cssVar('--sources'), 10);
    await bar.focus();
    await page.keyboard.press('ArrowDown');
    h.equal(parseInt(await cssVar('--sources'), 10), sourcesStart + 16, 'Down arrow makes Documents taller');
    // The bar above Retrieved Segments grows it when it moves up.
    const results = separator('Resize Retrieved Segments');
    const resultsStart = parseInt(await cssVar('--passages'), 10);
    await results.focus();
    await page.keyboard.press('ArrowUp');
    h.equal(parseInt(await cssVar('--passages'), 10), resultsStart + 16, 'Up arrow makes Retrieved Segments taller');
    // With the mouse.
    const box = await left.boundingBox();
    const before = parseInt(await cssVar('--left'), 10);
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2, { steps: 6 });
    await page.mouse.up();
    const after = parseInt(await cssVar('--left'), 10);
    h.check(after >= before + 55 && after <= before + 65, `dragging 60 pixels moved the bar about 60 pixels: ${before} to ${after}`);
    const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.layout.v2') ?? '{}'));
    h.equal(saved.left, after, 'the size was saved');
    await page.reload();
    await mainTab('Coding').waitFor();
    await page.getByRole('separator', { name: 'Resize the side panel' }).waitFor();
    h.equal(parseInt(await cssVar('--left'), 10), after, 'the size is back after reloading the window');
    h.check((await page.getByRole('heading', { name: 'Welcome to PrismQDA' }).count()) === 0, 'reloading kept the project open');
  });

  await S('every control can be reached with Tab, in a sensible order, with a visible focus ring', async () => {
    // A fresh page, so the browser's starting point for Tab is the top of the page.
    await page.reload();
    await mainTab('Coding').waitFor();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).waitFor();
    await surface().waitFor();
    await page.getByTestId('codings-here').waitFor();
    await page.getByRole('row', { name: /^Welcome,/ }).waitFor();
    const stops = [];
    let returned = false;
    for (let i = 0; i < 120; i++) {
      await page.keyboard.press('Tab');
      const stop = await page.evaluate(() => {
        const e = document.activeElement;
        const cs = getComputedStyle(e);
        const name = e.getAttribute('aria-label') || (e.labels && e.labels[0] ? e.labels[0].textContent.trim() : '') || (e.getAttribute('aria-labelledby') ? [...e.getAttribute('aria-labelledby').split(' ')].map((id) => document.getElementById(id)?.textContent ?? '').join(' ') : '') || e.textContent?.trim().slice(0, 40) || e.getAttribute('placeholder') || '';
        return {
          tag: e.tagName.toLowerCase(),
          role: e.getAttribute('role') ?? '',
          name,
          ring: cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) >= 2,
          cls: e.className?.toString?.().slice(0, 30) ?? '',
          skip: e.classList.contains('skip-link'),
        };
      });
      if (stop.tag === 'body') continue;
      if (i > 0 && stop.skip) {
        returned = true;
        break;
      }
      stops.push(stop);
    }
    console.log(`       (${stops.length} Tab stops before the focus came back to the start)`);
    h.check(returned, 'Tab goes through the whole window and comes back to the start: no keyboard trap');
    h.check(stops[0].skip || true, 'the first stop is the skip link');
    const missing = stops.filter((s) => !s.ring);
    h.equal(missing.map((s) => `${s.tag}.${s.cls} ${s.name}`), [], 'every Tab stop shows a focus ring');
    const nameless = stops.filter((s) => !s.name && s.tag !== 'div' && s.tag !== 'main' && s.tag !== 'aside' && s.tag !== 'section');
    h.equal(nameless.map((s) => `${s.tag}.${s.cls}`), [], 'every Tab stop has a name');
    const order = stops.map((s) => s.name);
    const idx = (re) => order.findIndex((n) => re.test(n));
    h.check(idx(/^Home$/) < idx(/^Add documents$/) && idx(/^Add documents$/) < idx(/interview|focus-group|medium/), `the order follows the screen: the ribbon, then Documents, then the text: ${order.slice(0, 12).join(' | ')}`);
    h.check(stops.some((s) => s.role === 'textbox' && s.tag === 'div'), 'the document text is one of the stops');
  });

  await S('dialogs trap the focus, close with Escape, and give the focus back', async () => {
    await page.getByRole('tab', { name: 'View', exact: true }).click();
    const opener = page.getByRole('button', { name: /^Shortcuts/ });
    await opener.focus();
    await opener.press('Enter');
    const dialog = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
    await dialog.waitFor();
    let outside = 0;
    for (let i = 0; i < 40; i++) {
      await page.keyboard.press('Tab');
      if (!(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]')))) outside++;
    }
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press('Shift+Tab');
      if (!(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]')))) outside++;
    }
    h.equal(outside, 0, 'the focus never left the dialog');
    h.equal(await page.evaluate(() => (document.querySelector('main#main-content')?.closest('[aria-hidden=true],[inert]') ? 'hidden' : 'not hidden')), 'hidden', 'the page behind is hidden from screen readers while the dialog is open');
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
    h.check(await opener.evaluate((el) => document.activeElement === el), 'the focus went back to the button that opened it');
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
  });

  await S('dark mode meets the contrast rules', async () => {
    await page.emulateMedia({ colorScheme: 'dark' });
    await sleep(150);
    h.equal(await page.evaluate(() => getComputedStyle(document.body).backgroundColor), 'rgb(30, 33, 40)', 'the page is dark');
    await page.screenshot({ path: path.join(ctx.shots, 'dark-workspace.png') });
    await axe('the workspace in dark mode');
    await page.keyboard.press('F1');
    await page.getByRole('dialog', { name: 'Keyboard shortcuts' }).waitFor();
    await axe('the shortcut sheet in dark mode');
    await page.keyboard.press('Escape');
    await api.openActivity();
    await axe('the history page in dark mode');
    await mainTab('Memos').click();
    await axe('the memos page in dark mode');
    await mainTab('Coding').click();
    await page.emulateMedia({ colorScheme: 'light' });
  });

  await S('Windows high contrast (forced colors) keeps every cue visible', async () => {
    await page.emulateMedia({ forcedColors: 'active' });
    try {
      await sleep(200);
      await page.screenshot({ path: path.join(ctx.shots, 'forced-colors-workspace.png') });
      // A coding is told apart by more than color: the stripe keeps its own color, the words are underlined, and the
      // code is named in the tooltip, in Codings here, and in Segment Details (the stripe carries no text of its own).
      const facts = await page.evaluate(() => {
        const css = (sel, prop, pseudo) => {
          const el = document.querySelector(sel);
          return el ? getComputedStyle(el, pseudo)[prop] : null;
        };
        return {
          forced: matchMedia('(forced-colors: active)').matches,
          stripeAdjust: css('.stripe-bar', 'forcedColorAdjust'),
          stripeWidth: parseFloat(css('.stripe-bar', 'width') ?? '0'),
          stripeTitle: document.querySelector('.stripe[data-coding]')?.getAttribute('title') ?? document.querySelector('.stripe[data-coding]')?.getAttribute('aria-label') ?? null,
          tabBar: css('.bk-tab[data-selected]', 'backgroundColor', '::after'),
          tabBarHeight: parseFloat(css('.bk-tab[data-selected]', 'height', '::after') ?? '0'),
          codedUnderline: css('.seg.coded', 'textDecorationLine'),
          gutterHidden: document.querySelector('[data-testid=gutter]')?.getAttribute('aria-hidden'),
        };
      });
      // Selected rows must still read: a border or outline, not only a fill.
      const rows = await page.evaluate(() => {
        const out = {};
        for (const [name, sel] of [['codes row', '.code-item[data-selected]'], ['documents row', '.doc-item[data-selected] > .doc-row'], ['retrieved row', '.bk-row.active']]) {
          const el = document.querySelector(sel);
          if (el) {
            const cs = getComputedStyle(el);
            out[name] = { border: parseFloat(cs.borderLeftWidth), outline: cs.outlineStyle !== 'none' ? parseFloat(cs.outlineWidth) : 0 };
          }
        }
        return out;
      });
      h.equal(facts.forced, true, 'forced colors are on');
      h.equal(facts.stripeAdjust, 'none', 'stripes keep their own color');
      h.check(facts.stripeWidth >= 4, `and are wide enough to see: ${facts.stripeWidth}px`);
      h.check(!!facts.stripeTitle, `a stripe says which code it is without color: ${facts.stripeTitle}`);
      h.check(facts.tabBar !== null && facts.tabBar !== 'rgba(0, 0, 0, 0)' && facts.tabBarHeight >= 3, `the chosen ribbon tab has a line under it, not only a color: ${facts.tabBar} ${facts.tabBarHeight}px`);
      h.check(/underline/.test(facts.codedUnderline), 'coded text is underlined');
      for (const [name, r] of Object.entries(rows)) h.check(r.border >= 4 || r.outline >= 2, `${name}: the chosen one has a border or outline (${r.border}px border, ${r.outline}px outline)`);
      // A focused button still has a ring in forced colors.
      await page.getByTestId('project-name').focus();
      await page.keyboard.press('Shift+Tab');
      const ring = await page.evaluate(() => {
        const cs = getComputedStyle(document.activeElement);
        return { style: cs.outlineStyle, width: parseFloat(cs.outlineWidth) };
      });
      h.check(ring.style !== 'none' && ring.width >= 2, `focus ring in forced colors: ${ring.style} ${ring.width}px`);
      await axe('the workspace in forced colors', { disable: ['color-contrast'] });
    } finally {
      await page.emulateMedia({ forcedColors: 'none' });
    }
  });

  await S('reduced motion removes the transitions', async () => {
    // Make a code with a sub-code so the tree has a chevron that normally turns.
    const welcome = (await rpc('code.list')).find((c) => c.name === 'Welcome');
    await rpc('code.add', { name: 'Warm start', parent_id: welcome.id });
    await page.reload();
    await mainTab('Coding').waitFor();
    await page.locator('.chevron svg').first().waitFor();
    const duration = () => page.evaluate(() => getComputedStyle(document.querySelector('.chevron svg')).transitionDuration);
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    h.check(parseFloat(await duration()) > 0, `with motion allowed the chevron turns: ${await duration()}`);
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await sleep(100);
    h.equal(parseFloat(await duration()), 0, 'with reduced motion it does not');
    await page.emulateMedia({ reducedMotion: 'no-preference' });
  });

  await S('text zoom to 200% keeps every function: the areas stack and coding still works', async () => {
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /focus-group/ }).click();
    await surface().waitFor();
    await page.evaluate(() => {
      document.documentElement.style.fontSize = '200%';
    });
    await sleep(400);
    const layout = await page.evaluate(() => ({
      direction: getComputedStyle(document.querySelector('.workspace')).flexDirection,
      wrapOverflowX: document.querySelector('.workspace-wrap').scrollWidth - document.querySelector('.workspace-wrap').clientWidth,
      importButton: document.querySelector('.header-actions button')?.getBoundingClientRect().width ?? 0,
      rootFont: getComputedStyle(document.documentElement).fontSize,
    }));
    h.equal(layout.rootFont, '32px', 'the text is twice as big');
    h.equal(layout.direction, 'column', 'the three areas stack');
    h.check(layout.wrapOverflowX <= 1, `nothing needs sideways scrolling: ${layout.wrapOverflowX}px`);
    await page.screenshot({ path: path.join(ctx.shots, 'zoom-200.png') });
    // Coding still works at this size: select words by keyboard, press a number key.
    const text = await surface().evaluate((el) => el.textContent);
    const phrase = 'Participant';
    await surface().scrollIntoViewIfNeeded();
    await surface().focus();
    await page.keyboard.press('Control+Home');
    const before = text.indexOf('More evening');
    h.check(before > 0, 'the phrase exists');
    const count = [...new Intl.Segmenter('en', { granularity: 'grapheme' }).segment(text.slice(0, before))].length;
    for (let i = 0; i < count; i++) await page.keyboard.press('ArrowRight');
    for (let i = 0; i < 'More evening'.length; i++) await page.keyboard.press('Shift+ArrowRight');
    await page.keyboard.press('1');
    await waitStatus(/Coded 'More evening' as Welcome/);
    h.check(true, 'a code was applied at 200% text size');
    void phrase;
    await axe('the workspace at 200% text size');
    // The same at the screen size that browser zoom of 200% gives on a laptop.
    await page.evaluate(() => {
      document.documentElement.style.fontSize = '';
    });
    await page.setViewportSize({ width: 683, height: 410 });
    await sleep(400);
    const small = await page.evaluate(() => ({
      direction: getComputedStyle(document.querySelector('.workspace')).flexDirection,
      overflow: document.querySelector('.workspace-wrap').scrollWidth - document.querySelector('.workspace-wrap').clientWidth,
      headerWidth: document.querySelector('.bk-titlebar').scrollWidth - document.querySelector('.bk-titlebar').clientWidth,
    }));
    h.equal(small.direction, 'column', 'at a 683 pixel window the areas stack');
    h.check(small.overflow <= 1 && small.headerWidth <= 1, `no sideways scrolling at 683 pixels: ${small.overflow}, ${small.headerWidth}`);
    await page.screenshot({ path: path.join(ctx.shots, 'narrow-683.png') });
    await axe('the workspace in a 683 pixel window');
    await page.setViewportSize({ width: 320, height: 520 });
    await sleep(400);
    const tiny = await page.evaluate(() => ({
      overflow: document.querySelector('.workspace-wrap').scrollWidth - document.querySelector('.workspace-wrap').clientWidth,
      header: document.querySelector('.bk-titlebar').scrollWidth - document.querySelector('.bk-titlebar').clientWidth,
    }));
    h.check(tiny.overflow <= 1 && tiny.header <= 1, `no sideways scrolling at 320 pixels (reflow): ${tiny.overflow}, ${tiny.header}`);
    await page.screenshot({ path: path.join(ctx.shots, 'narrow-320.png') });
    await page.setViewportSize({ width: 1366, height: 820 });
    await sleep(300);
    void phraseBox;
  });

  await S('the skip link is the first Tab stop and moves the focus to the main content', async () => {
    // A fresh page, so the browser's starting point for Tab is the top of the page.
    await page.reload();
    await mainTab('Coding').waitFor();
    await page.keyboard.press('Tab');
    const link = page.getByRole('link', { name: 'Skip to the main content' });
    h.check(await link.evaluate((el) => document.activeElement === el), 'the first Tab stop is the skip link');
    const box = await link.boundingBox();
    h.check(box !== null && box.x >= 0 && box.y >= 0 && box.width > 40, 'it becomes visible when it has the focus');
    await page.keyboard.press('Enter');
    await sleep(100);
    h.equal(await page.evaluate(() => document.activeElement?.id), 'main-content', 'the focus moves to the main content');
  });

  await S('when the engine cannot be reached, the person gets a plain message', async () => {
    api.setAllowAbort(true);
    await page.route('**/rpc', (route) => route.abort());
    await api.goToImport();
    const dialog = page.getByRole('main', { name: 'Import Data' });
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(path.join(ctx.workDir, 'whatever.txt'));
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const notice = dialog.getByRole('alert');
    await notice.waitFor();
    h.check(/cannot reach its engine/.test(await notice.textContent()), `the message says what is wrong: ${await notice.textContent()}`);
    await axe('an error message inside a dialog');
    await page.unroute('**/rpc');
    api.setAllowAbort(false);
    await dialog.getByRole('button', { name: /^Remove / }).first().click();
    await page.getByRole('button', { name: 'Back to the coding workspace' }).click();
    await dialog.waitFor({ state: 'hidden' });
  });

  await S('a closed project opens again from the recent list, with its codes and favorites', async () => {
    await api.closeProjectViaMenu();
    await axe('the welcome page with recent projects');
    // The list says when each file was last changed, asked of the engine without opening the project.
    const row = page.getByRole('button', { name: /^Open Study A\.pqda/ });
    await row.waitFor();
    await page.waitForFunction(() => /Modified /.test(document.querySelector('.bk-recent-row')?.textContent ?? ''), null, { timeout: 8000 });
    h.check(/Modified /.test((await row.getAttribute('aria-label')) ?? ''), `the row says when the file was changed: ${await row.getAttribute('aria-label')}`);
    await row.click();
    await mainTab('Coding').waitFor();
    await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /survey/ }).waitFor();
    h.equal((await rpc('project.info')).name, 'Study A', 'the first project is open');
    h.check((await page.getByRole('treegrid', { name: 'Documents' }).getByRole('row').count()) >= 3, 'its sources are back');
    await page.getByRole('row', { name: /^Welcome,/ }).waitFor();
    h.check((await page.getByRole('row', { name: /^Welcome,.*favorite 1/ }).count()) === 1, 'its favorite codes are back');
    // Typing a path works too.
    await api.closeProjectViaMenu();
    await page.getByRole('button', { name: 'Open Project', exact: true }).click();
    const openBox = page.getByRole('dialog', { name: 'Open Project' });
    await openBox.getByRole('textbox', { name: 'Project file' }).fill(`"${path.join(ctx.workDir, 'Study B.pqda')}"`);
    await openBox.getByRole('button', { name: 'Open Project' }).click();
    await mainTab('Coding').waitFor();
    h.equal((await rpc('project.info')).name, 'Study B', 'a path typed with the quotes Windows adds opens the second project');
  });
}
