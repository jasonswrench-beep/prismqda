// The reading surface and the details: coded words look highlighted, lines are numbered, the text fills the middle,
// and the details panel says what a coding is, where it is, and where it came from.

import fs from 'node:fs';
import path from 'node:path';

export async function runLook(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe, mainTab, sleep, codeRow, codeAction, addCodeThroughDialog, selectByMouse, surface, surfaceText, waitStatus } = api;

  const bar = () => page.getByTestId('float-bar');
  const usageOf = async (name) => (await rpc('code.list')).find((c) => c.name === name)?.usage ?? 0;
  const codeNamed = async (name) => (await rpc('code.list')).find((c) => c.name === name);
  const rgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  const tint = (hex) => `rgba(${rgb(hex).join(', ')}, 0.32)`;

  /** A long lowercase word in text that can be coded, found only once on screen so the mouse lands on it. */
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
    // A word that is not already inside coded text, so the first code on it is the only one.
    const free = await page.evaluate(
      (ws) =>
        ws.filter((w) => {
          const el = [...document.querySelectorAll('[data-testid=surface] .seg')].find((e) => e.textContent.includes(w));
          return !!el && !el.classList.contains('coded');
        }),
      words,
    );
    const list = free.length > 0 ? free : words;
    return list[Math.min(skip, list.length - 1)];
  }

  /** The background of the first piece of text that contains the word, and how many codings cover it. */
  const wordLook = (word) =>
    page.evaluate((w) => {
      const els = [...document.querySelectorAll('[data-testid=surface] .seg')];
      const el = els.find((e) => e.textContent.includes(w)) ?? els.find((e) => w.includes(e.textContent) && e.textContent.length > 2);
      if (!el) return null;
      const cs = getComputedStyle(el);
      return { background: el.style.background, line: cs.textDecorationLine, style: cs.textDecorationStyle, n: el.dataset.n ?? '0', coded: el.classList.contains('coded') };
    }, word);

  /** The same, once the text has been drawn with the number of codings expected: the engine has a change a moment before the text shows it. */
  async function wordLookWhen(w, n) {
    let look = await wordLook(w);
    for (let i = 0; i < 30 && (!look || look.n !== n); i++) {
      await sleep(100);
      look = await wordLook(w);
    }
    return look;
  }

  async function ensureOnCode() {
    await mainTab('Coding').click();
    await surface().waitFor();
    await page.keyboard.press('Escape');
  }

  async function newFavorite(name) {
    await addCodeThroughDialog(name);
    await codeRow(name).first().click();
    await codeAction('Pin as favorite');
    await page.waitForFunction((n) => /favorite \d/.test(document.querySelector(`[role=row][aria-label^="${n},"]`)?.getAttribute('aria-label') ?? ''), name, { timeout: 5000 });
    return Number(/favorite (\d)/.exec((await codeRow(name).first().getAttribute('aria-label')) ?? '')?.[1]);
  }

  let word = null;
  let slotA = 0;
  let slotB = 0;

  await S('coded words are highlighted in their code color, with no dotted lines', async () => {
    await ensureOnCode();
    // A source of its own with nothing coded yet, so the words are not already covered by codes from earlier steps.
    const file = path.join(workDir, 'look-check.txt');
    const lines = ['Look check source.', '', 'The quick fox answered carefully while the morning traveled somewhere wonderful.', 'Another sentence follows here, with several different lowercase words included.', ''];
    fs.writeFileSync(file, lines.join('\n'));
    await api.goToImport();
    const dialog = page.getByRole('main', { name: 'Import Data' });
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(file);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    const finished = page.getByTestId('import-finished');
    await finished.waitFor();
    await finished.getByRole('button', { name: 'Done' }).click();
    await finished.waitFor({ state: 'hidden' });
    await page.locator('#h-source').filter({ hasText: 'look-check' }).waitFor();
    await surface().waitFor();
    slotA = await newFavorite('Look check A');
    slotB = await newFavorite('Look check B');
    word = await aWord(0);
    await selectByMouse(word);
    await bar().waitFor();
    const used = await usageOf('Look check A');
    await page.keyboard.press(String(slotA));
    for (let i = 0; i < 40 && (await usageOf('Look check A')) === used; i++) await sleep(100);
    h.equal(await usageOf('Look check A'), used + 1, 'the word was coded');
    await page.keyboard.press('Escape');
    const a = await codeNamed('Look check A');
    const look = await wordLookWhen(word, '1');
    h.check(look && look.coded, 'the word is drawn as coded text');
    h.equal(look.background, tint(a.color), `the word carries its code color at highlighter strength (${a.color})`);
    h.check(look.style !== 'dotted' && look.style !== 'dashed', `there is no dotted or dashed line under it: ${look.line} ${look.style}`);
    // Coded words that are not the chosen coding have no line at all.
    const lined = await page.evaluate(() => [...document.querySelectorAll('[data-testid=surface] .seg.coded:not(.chosen)')].map((e) => getComputedStyle(e).textDecorationLine).filter((l) => l !== 'none'));
    h.equal(lined, [], 'and no coded words have an underline unless they are the chosen coding');
    // Every coded piece of text on screen carries one of the codes' colors, one color or bands of several.
    const colors = new Set((await rpc('code.list')).map((c) => tint(c.color)));
    const stray = await page.evaluate((known) => {
      return [...document.querySelectorAll('[data-testid=surface] .seg.coded')]
        .map((e) => e.style.background)
        .filter((b) => !b || (!known.includes(b) && !(b.startsWith('linear-gradient') && b.includes('rgba('))));
    }, [...colors]);
    h.equal(stray, [], 'every coded piece of text is highlighted in a code color');
    await axe('the document with highlighted words');
  });

  await S('choosing a color for a code highlights its words in that color at once', async () => {
    await api.openDefinition('Look check A');
    const form = page.getByTestId('code-detail');
    await form.waitFor();
    await form.getByText('Hot pink', { exact: true }).click();
    await waitStatus(/Look check A is now Hot pink/);
    h.equal((await codeNamed('Look check A')).color.toUpperCase(), '#F0388F', 'the color was saved');
    // Back in the coding workspace, the words and the codebook show it.
    await api.closeDefinition();
    await surface().waitFor();
    await page.waitForFunction((bg) => [...document.querySelectorAll('[data-testid=surface] .seg.coded')].some((e) => e.style.background === bg), tint('#F0388F'), { timeout: 5000 });
    h.check(true, 'the words are drawn in the new color');
    const dot = await codeRow('Look check A').first().locator('.code-dot').evaluate((e) => getComputedStyle(e).backgroundColor);
    h.equal(dot, 'rgb(240, 56, 143)', 'and the dot in the codebook shows it too');
  });

  await S('words with two codes show a band of each color', async () => {
    await selectByMouse(word);
    await bar().waitFor();
    const used = await usageOf('Look check B');
    await page.keyboard.press(String(slotB));
    for (let i = 0; i < 40 && (await usageOf('Look check B')) === used; i++) await sleep(100);
    await page.keyboard.press('Escape');
    const look = await wordLookWhen(word, '2');
    const a = await codeNamed('Look check A');
    const b = await codeNamed('Look check B');
    h.equal(look.n, '2', 'two codings cover the word');
    h.check(look.background.startsWith('linear-gradient') && look.background.includes(tint(a.color)) && look.background.includes(tint(b.color)), `bands of both colors: ${look.background}`);
  });

  await S('the lines of text are numbered, one number for each line as drawn', async () => {
    await ensureOnCode();
    const lines = page.getByTestId('line-numbers');
    await lines.waitFor();
    // From the top, so the first number drawn is the first line.
    await page.evaluate(() => {
      document.querySelector('.surface-scroll').scrollTop = 0;
    });
    await sleep(200);
    h.equal(await lines.getAttribute('aria-hidden'), 'true', 'the numbers are decoration for the eye and are hidden from a screen reader');
    const m = await page.evaluate(() => {
      const nums = [...document.querySelectorAll('[data-testid=line-numbers] .line-no')].map((e) => ({ n: Number(e.dataset.line), label: e.textContent, mid: e.getBoundingClientRect().top + e.getBoundingClientRect().height / 2 }));
      const surface = document.querySelector('[data-testid=surface]');
      const rects = [...surface.querySelectorAll('.seg')].flatMap((e) => [...e.getClientRects()]).filter((r) => r.width > 0.5 && r.height > 0.5);
      // Lines of the text itself, found independently: rectangles that overlap vertically are one line.
      const rows = [];
      for (const r of rects.sort((a, b) => a.top - b.top)) {
        const last = rows[rows.length - 1];
        const mid = (r.top + r.bottom) / 2;
        if (last && mid >= last.top && mid <= last.bottom) {
          last.top = Math.min(last.top, r.top);
          last.bottom = Math.max(last.bottom, r.bottom);
        } else rows.push({ top: r.top, bottom: r.bottom });
      }
      const sel = getComputedStyle(document.querySelector('[data-testid=line-numbers]')).userSelect;
      return { nums, rows: rows.length, onALine: nums.every((x) => rows.some((r) => x.mid >= r.top - 1 && x.mid <= r.bottom + 1)), userSelect: sel, declared: Number(document.querySelector('[data-testid=line-numbers]').dataset.lines), surfaceStart: surface.textContent.slice(0, 3) };
    });
    h.check(m.nums.length >= 3, `there are numbers: ${m.nums.length}`);
    h.equal(m.nums[0].n, 1, 'the first line is number 1');
    h.check(m.nums.every((x, i) => x.n === m.nums[0].n + i), 'the numbers count up one at a time with no gaps');
    h.check(m.nums.every((x) => x.label === String(x.n).padStart(3, '0')), 'each is shown with three digits');
    h.check(m.onALine, 'each number sits level with a line of text');
    h.equal(m.declared, m.rows, `as many numbers as lines of text drawn: ${m.declared} against ${m.rows}`);
    h.equal(m.userSelect, 'none', 'the numbers cannot be selected, so they are never copied or coded with the words');
    h.check(!/^\d{3}/.test(m.surfaceStart), 'and they are not part of the text itself');
  });

  await S('the numbers follow the window: a narrower window wraps more lines, and every one is still numbered', async () => {
    const lines = page.getByTestId('line-numbers');
    const before = Number(await lines.getAttribute('data-lines'));
    const size = page.viewportSize();
    await page.setViewportSize({ width: 1000, height: size.height });
    for (let i = 0; i < 30 && Number(await lines.getAttribute('data-lines')) === before; i++) await sleep(100);
    const after = Number(await lines.getAttribute('data-lines'));
    h.check(after > before, `more lines at the narrower width: ${before} then ${after}`);
    const nums = await lines.locator('.line-no').evaluateAll((els) => els.map((e) => Number(e.dataset.line)));
    h.check(nums.every((n, i) => n === nums[0] + i), 'still counted without gaps');
    await page.setViewportSize(size);
    for (let i = 0; i < 30 && Number(await lines.getAttribute('data-lines')) !== before; i++) await sleep(100);
    h.equal(Number(await lines.getAttribute('data-lines')), before, 'and back to the first count when the window is wide again');
  });

  await S('the text fills the middle of the window, and Settings can limit it to a comfortable line length', async () => {
    const reach = () =>
      page.evaluate(() => {
        const scroller = document.querySelector('.surface-scroll').getBoundingClientRect();
        const surface = document.querySelector('[data-testid=surface]').getBoundingClientRect();
        const gutter = document.querySelector('[data-testid=gutter]').getBoundingClientRect();
        return { scrollerWidth: scroller.width, surfaceWidth: surface.width, gutterLeft: gutter.left, gutterWidth: gutter.width, surfaceRight: surface.right, scrollerRight: scroller.right, root: parseFloat(getComputedStyle(document.documentElement).fontSize) };
      });
    // A wide window, so that "fill" and "comfortable" are not the same thing.
    const size = page.viewportSize();
    await page.setViewportSize({ width: 1900, height: size.height });
    await sleep(400);
    const wide = await reach();
    h.check(wide.surfaceWidth > wide.scrollerWidth * 0.6, `the text is most of the width of its box: ${Math.round(wide.surfaceWidth)} of ${Math.round(wide.scrollerWidth)}`);
    h.check(wide.gutterLeft - wide.surfaceRight < 40, `and it runs up to the margin stripes on the right: ${Math.round(wide.gutterLeft - wide.surfaceRight)} pixels between`);
    h.check(wide.scrollerRight - wide.gutterLeft <= wide.gutterWidth + 0.6 * wide.root + 24, 'which sit at the right edge, next to the details');

    await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await axe('Settings with the reading choices');
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^Comfortable line length/ }) }).click();
    h.equal(await page.evaluate(() => document.documentElement.dataset.textWidth), 'comfortable', 'the choice is on the page');
    await mainTab('Coding').click();
    await surface().waitFor();
    const narrow = await reach();
    h.check(narrow.surfaceWidth <= 46 * narrow.root + 1, `the text stops at about 46 rem: ${Math.round(narrow.surfaceWidth)} pixels`);
    h.check(narrow.surfaceWidth < wide.surfaceWidth, 'and is narrower than when it fills the window');
    await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^Fill the window/ }) }).click();
    h.equal(await page.evaluate(() => document.documentElement.dataset.textWidth ?? null), null, 'Fill the window is the standard again');
    await mainTab('Coding').click();
    await surface().waitFor();
    await page.setViewportSize(size);
    await sleep(300);
  });

  await S('line numbers can be turned off in Settings, and come back', async () => {
    const box = () => page.getByRole('checkbox', { name: 'Number the lines of text' });
    const toggle = () => page.locator('label').filter({ has: box() }).click();
    const padding = () => page.evaluate(() => parseFloat(getComputedStyle(document.querySelector('.surface-body')).paddingLeft));
    const withNumbers = await padding();
    await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    h.check(await box().isChecked(), 'on by default');
    await toggle();
    h.equal((await page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.settings'))?.lineNumbers)), false, 'remembered');
    await mainTab('Coding').click();
    await surface().waitFor();
    h.equal(await page.getByTestId('line-numbers').count(), 0, 'no numbers with it off');
    h.check((await padding()) < withNumbers, 'and their room is given back to the text');
    await page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await toggle();
    await mainTab('Coding').click();
    await page.getByTestId('line-numbers').waitFor();
    h.check(true, 'back on');
  });

  await S('the details of a coding: the selected words, where they are, the codes on them, memos, and provenance', async () => {
    await ensureOnCode();
    // A stripe of the word that two codes cover.
    const a = await codeNamed('Look check A');
    const stripe = page.locator(`.stripe[data-coding]`).filter({ has: page.locator('.stripe-label') }).first();
    await stripe.waitFor();
    const codingId = await page.evaluate((name) => [...document.querySelectorAll('.stripe[data-coding]')].find((s) => s.title === name)?.dataset.coding, a.name);
    await page.locator(`.stripe[data-coding="${codingId}"]`).click();
    const detail = page.getByTestId('coding-detail');
    await detail.waitFor();
    h.equal(((await page.getByTestId('coding-quote').textContent()) ?? '').trim(), `“${word}”`, 'the selected segment shows the words');
    const facts = await page.getByTestId('coding-facts').textContent();
    h.check(/Source/.test(facts) && /Where/.test(facts), `it names the source and the place: ${facts}`);
    const lines = (await page.getByTestId('coding-lines').textContent()) ?? '';
    h.check(/^\d{3,}( – \d{3,})? \(\d+ lines?\)$/.test(lines.trim()), `and the lines, as numbered in the text: ${lines}`);
    const shown = Number(/^(\d+)/.exec(lines.trim())[1]);
    // The line the words are on is the line with that number: the number beside the text is the same one.
    const numberedLine = await page.evaluate((n) => {
      const el = document.querySelector(`[data-testid=line-numbers] .line-no[data-line="${n}"]`);
      return el ? Math.round(el.getBoundingClientRect().top) : null;
    }, shown);
    h.check(numberedLine !== null, `line ${shown} is one of the numbers beside the text`);
    h.equal(await detail.getByRole('heading', { level: 3, name: /^Applied Codes \(2\)$/ }).count(), 1, 'two codes are applied to these words');
    const chips = await page.getByTestId('applied-codes').getByRole('button').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    h.check(chips.some((c) => /^Look check A/.test(c)) && chips.some((c) => /^Look check B/.test(c)), `a chip for each: ${chips.join(' | ')}`);
    h.equal(await detail.getByRole('heading', { level: 3, name: /^Memo/ }).count(), 1, 'there is a memo section');
    const prov = await page.getByTestId('provenance').textContent();
    h.check(/Coded by/.test(prov) && /Coded on/.test(prov), `provenance says who and when: ${prov}`);
    h.check(/20\d\d/.test(prov), 'with a date');
    await axe('the details of a coding');
  });

  await S('a code chip in the details can be removed from the words, and the other stays', async () => {
    const b = await codeNamed('Look check B');
    const usedBefore = await usageOf('Look check B');
    await page.getByTestId('applied-codes').getByRole('button', { name: /^Remove the code Look check B from these words/ }).click();
    for (let i = 0; i < 40 && (await usageOf('Look check B')) === usedBefore; i++) await sleep(100);
    h.equal(await usageOf('Look check B'), usedBefore - 1, `${b.name} was taken off the words`);
    const look = await wordLookWhen(word, '1');
    h.equal(look.n, '1', 'one coding is left on them');
    h.check(look.background.startsWith('rgba('), 'drawn in a single color again');
  });

  await S('the top of the document moves to the next and the previous source and says where you are in the list', async () => {
    await ensureOnCode();
    const heading = page.locator('#h-source');
    const position = () => page.getByTestId('source-position').textContent().then((t) => (t ?? '').trim());
    const [start, total] = /^(\d+) of (\d+)$/.exec(await position()).slice(1).map(Number);
    h.check(total >= 2, `there are sources to move between: ${total}`);
    const name = ((await heading.textContent()) ?? '').trim();
    const next = page.getByRole('button', { name: 'Next document' });
    const previous = page.getByRole('button', { name: 'Previous document' });
    h.equal(await previous.isDisabled(), start === 1, 'Previous is off at the first source only');
    h.equal(await next.isDisabled(), start === total, 'and Next at the last only');
    if (start < total) {
      await next.click();
      await page.waitForFunction((n) => document.querySelector('#h-source')?.textContent?.trim() !== n, name, { timeout: 10000 });
      h.equal(await position(), `${start + 1} of ${total}`, 'Next moved one place along');
      await previous.click();
      await page.waitForFunction((n) => document.querySelector('#h-source')?.textContent?.trim() === n, name, { timeout: 10000 });
      h.equal(await position(), `${start} of ${total}`, 'and Previous came back to the same source');
    } else {
      await previous.click();
      await page.waitForFunction((n) => document.querySelector('#h-source')?.textContent?.trim() !== n, name, { timeout: 10000 });
      await next.click();
      await page.waitForFunction((n) => document.querySelector('#h-source')?.textContent?.trim() === n, name, { timeout: 10000 });
    }
    await axe('the document header with the source buttons');
  });
}
