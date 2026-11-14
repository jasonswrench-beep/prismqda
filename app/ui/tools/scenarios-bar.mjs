// The floating coding bar: it appears above words selected with the mouse and does what the keys do.

export async function runBar(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe, mainTab, sleep, codeRow, codeAction, addCodeThroughDialog, selectByMouse, phraseBox, surface, surfaceText } = api;

  const bar = () => page.getByTestId('float-bar');
  const favoriteButtons = () => bar().getByRole('button', { name: /favorite \d/ });
  const focusIsInText = () => page.evaluate(() => document.activeElement?.getAttribute('data-testid') === 'surface');
  const selectedText = () => page.evaluate(() => window.getSelection().toString());
  const usageOf = async (name) => (await rpc('code.list')).find((c) => c.name === name)?.usage ?? 0;

  /**
   * A word in the open source to select: a long lowercase word (a speaker's name above a turn is a label, not part
   * of what can be coded), inside text that can be coded, and found only once in what is on screen so the mouse
   * lands on the right place.
   */
  async function aWord(skip = 0) {
    const shown = await surfaceText();
    const name = ((await page.locator('#h-source').textContent()) ?? '').trim();
    const src = (await rpc('source.list')).find((s) => s.name === name);
    let pool = shown;
    if (src) {
      const text = Array.from((await rpc('source.get', { id: src.id })).text);
      const units = (await rpc('unit.list', { source_id: src.id })).filter((u) => !u.excluded);
      if (units.length > 0) pool = units.map((u) => text.slice(u.start, u.end).join('')).join('\n');
    }
    const once = (w) => shown.split(w).length === 2;
    const words = [...new Set([...pool.matchAll(/\b[a-z]{7,}\b/g)].map((m) => m[0]))].filter(once);
    if (words.length === 0) throw new Error('the open source has no word to select');
    return words[Math.min(skip, words.length - 1)];
  }

  async function ensureOnCode() {
    await mainTab('Coding').click();
    await surface().waitFor();
    // Leave nothing selected or open from an earlier step.
    await page.keyboard.press('Escape');
  }

  /** Makes a new code and pins it as a favorite; returns its name and key number. Nothing is coded with it yet. */
  async function newFavorite(name) {
    await addCodeThroughDialog(name);
    await codeRow(name).first().click();
    await codeAction('Pin as favorite');
    await page.waitForFunction((n) => /favorite \d/.test(document.querySelector(`[role=row][aria-label^="${n},"]`)?.getAttribute('aria-label') ?? ''), name, { timeout: 5000 });
    const slot = Number(/favorite (\d)/.exec((await codeRow(name).first().getAttribute('aria-label')) ?? '')?.[1]);
    return { name, slot };
  }

  let a = null;
  let b = null;

  await S('words selected with the mouse show the floating bar above them, with the favorites and In vivo', async () => {
    await ensureOnCode();
    a = await newFavorite('Bar check A');
    b = await newFavorite('Bar check B');
    h.check(a.slot >= 1 && b.slot > a.slot && b.slot <= 6, `two new favorites are on keys ${a.slot} and ${b.slot}`);
    h.equal(await bar().count(), 0, 'no bar while nothing is selected');
    const word = await aWord();
    h.equal(await selectByMouse(word), word, 'the word is selected');
    await bar().waitFor();
    const group = bar().getByRole('group', { name: 'Code the selected words' });
    h.equal(await group.count(), 1, 'the bar is a labeled group');
    const names = await bar().getByRole('button').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    h.check(names.includes(`Code the selection as ${a.name}, favorite ${a.slot}`), `the favorites are on the bar: ${names.join(' | ')}`);
    h.check(names.includes('In vivo: make a code from the selected words'), 'In vivo is on the bar');
    h.check(names.includes('Other code: choose any code to apply'), 'Other code is on the bar');
    // It sits next to the words: above them, or under them when there is no room above.
    const box = await bar().boundingBox();
    const at = await phraseBox(word);
    const below = await bar().evaluate((el) => el.classList.contains('below'));
    if (below) h.check(box.y > at.y0, `the bar is under the selected line: bar top ${Math.round(box.y)}, line ${Math.round(at.y0)}`);
    else h.check(box.y + box.height < at.y0, `the bar is above the selected line: bar bottom ${Math.round(box.y + box.height)}, line ${Math.round(at.y0)}`);
    h.check(box.x >= 0 && box.x + box.width <= page.viewportSize().width, 'the bar is inside the window');
    const tabIndexes = await bar().getByRole('button').evaluateAll((els) => els.map((e) => e.getAttribute('tabindex')));
    h.check(tabIndexes.every((t) => t === '-1'), `the bar adds no Tab stops: ${tabIndexes.join(',')}`);
    await axe('the floating coding bar over selected words');
  });

  await S('pressing a code on the bar codes the selection and leaves the words selected and the focus in the text', async () => {
    const word = await selectedText();
    h.check(word.length > 0 && (await bar().isVisible()), 'the words from the last step are still selected, with the bar showing');
    const usedA = await usageOf(a.name);
    await bar().getByRole('button', { name: `Code the selection as ${a.name}, favorite ${a.slot}` }).click();
    for (let i = 0; i < 40 && (await usageOf(a.name)) === usedA; i++) await sleep(100);
    h.equal(await usageOf(a.name), usedA + 1, `${a.name} was applied once`);
    h.check(await focusIsInText(), 'the focus is still in the text');
    h.equal(await selectedText(), word, 'the words are still selected');
    h.check(await bar().isVisible(), 'the bar stays, so a second code can follow');
    // A second code on the same words, from the keyboard, still works.
    const usedB = await usageOf(b.name);
    await page.keyboard.press(String(b.slot));
    for (let i = 0; i < 40 && (await usageOf(b.name)) === usedB; i++) await sleep(100);
    h.equal(await usageOf(b.name), usedB + 1, 'a number key still codes the same words');
    // Escape puts the bar away and keeps the selection.
    await page.keyboard.press('Escape');
    await bar().waitFor({ state: 'detached' });
    h.equal(await selectedText(), word, 'Escape put the bar away and kept the words selected');
  });

  await S('the bar also offers other codes from the codebook, so it is useful before anything is pinned', async () => {
    await page.keyboard.press('Escape');
    const word = await aWord(4);
    await selectByMouse(word);
    await bar().waitFor();
    const labels = await bar().getByRole('button').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    const codeButtons = labels.filter((l) => /^Code the selection as /.test(l));
    const others = codeButtons.filter((l) => !/, favorite \d$|, the code used last$/.test(l));
    h.check(others.length >= 1, `codes with no key are on the bar too: ${others.join(' | ')}`);
    h.check(codeButtons.length <= 8, `no more than eight codes are on the bar: ${codeButtons.length}`);
    const name = /^Code the selection as (.+)$/.exec(others[0])[1];
    const used = await usageOf(name);
    await bar().getByRole('button', { name: others[0], exact: true }).click();
    for (let i = 0; i < 40 && (await usageOf(name)) === used; i++) await sleep(100);
    h.equal(await usageOf(name), used + 1, `${name} was applied once from the bar`);
    h.check(await focusIsInText(), 'the focus is still in the text');
    await page.keyboard.press('Escape');
  });

  await S('the bar goes away when the words are selected another way, the text loses the focus, or the caret is placed', async () => {
    const word = await aWord(1);
    await selectByMouse(word);
    await bar().waitFor();
    await page.keyboard.press('Shift+ArrowRight');
    await bar().waitFor({ state: 'detached' });
    h.check(true, 'extending the selection from the keyboard puts the bar away');

    await selectByMouse(word);
    await bar().waitFor();
    await codeRow(a.name).first().click();
    await bar().waitFor({ state: 'detached' });
    h.check(true, 'moving to the codebook puts the bar away');

    await selectByMouse(word);
    await bar().waitFor();
    const at = await phraseBox(word);
    await page.mouse.click(at.x0 + 3, at.y0);
    await bar().waitFor({ state: 'detached' });
    h.check(true, 'clicking in the text puts the bar away');

    // Words selected with the keyboard alone never raise the bar: the keys already do everything it does.
    await surface().focus();
    await page.keyboard.press('Control+Home');
    for (let i = 0; i < 5; i++) await page.keyboard.press('Shift+ArrowRight');
    await sleep(250);
    h.equal(await bar().count(), 0, 'a keyboard selection does not show the bar');
  });

  await S('In vivo on the bar makes a code from the words; Other code moves to the apply box', async () => {
    const word = await aWord(2);
    // Whatever the mouse selected is what names the code (in a project that codes whole units the sentence can come with it).
    const picked = ((await selectByMouse(word)) ?? '').replace(/\s+/g, ' ').trim();
    const start = picked.toLowerCase().replace(/[.,;:!?]+$/, '').slice(0, 16);
    await bar().waitFor();
    await bar().getByRole('button', { name: 'In vivo: make a code from the selected words' }).click();
    let made = false;
    for (let i = 0; i < 40 && !made; i++) {
      made = (await rpc('code.list')).some((c) => c.name.toLowerCase().startsWith(start));
      if (!made) await sleep(100);
    }
    h.check(made && start.length > 0, `a code named from the selected words (${picked.slice(0, 40)}) now exists`);
    h.check(await focusIsInText(), 'the focus is still in the text');
    await page.keyboard.press('Escape');

    await selectByMouse(await aWord(3));
    await bar().waitFor();
    await bar().getByRole('button', { name: 'Other code: choose any code to apply' }).click();
    await page.waitForFunction(() => document.activeElement?.getAttribute('role') === 'combobox');
    h.check(true, 'Other code put the focus in the apply box');
    await bar().waitFor({ state: 'detached' });
    h.check(true, 'and the bar was put away, since the apply box is now where the work is');
    // The first Escape closes the list of matches, the next closes the box.
    for (let i = 0; i < 3 && (await page.getByRole('dialog').count()) > 0; i++) {
      await page.keyboard.press('Escape');
      await sleep(150);
    }
  });
}
