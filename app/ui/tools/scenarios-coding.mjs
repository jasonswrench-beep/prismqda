// Creating codes and coding text: by mouse, by keyboard alone, with number keys, and in vivo.

export async function runCoding(ctx, api) {
  const { h, rpc } = ctx;
  const { page, mainTab, S, axe, waitStatus, surface, surfaceText, selectByMouse, caretFromStart, graphemesBefore, codeRow, addCodeThroughDialog, sleep, openDefinition, closeDefinition, openCodingOptions, openCodingsHere } = api;

  /** The scalar offsets a phrase should have in the source, computed independently of the interface. */
  const expectedSpan = (text, phrase) => {
    const i = text.indexOf(phrase);
    const start = Array.from(text.slice(0, i)).length;
    return { start, end: start + Array.from(phrase).length };
  };
  const graphemeCount = (s) => [...new Intl.Segmenter('en', { granularity: 'grapheme' }).segment(s)].length;
  const sourceText = async () => (await rpc('source.get', { id: 1 })).text;
  const lastCoding = async () => {
    const list = await rpc('coding.list', { source_id: 1 });
    return list.reduce((a, b) => (a && a.id > b.id ? a : b), null);
  };
  const selection = () => page.evaluate(() => window.getSelection().toString());

  /** Selects a phrase with Shift+arrow keys only, starting from the top of the text. */
  async function selectByKeyboard(phrase) {
    const text = await surfaceText();
    await caretFromStart(graphemesBefore(text, phrase));
    for (let i = 0; i < graphemeCount(phrase); i++) await page.keyboard.press('Shift+ArrowRight');
    return selection();
  }

  await S('codes can be added with a name and a definition', async () => {
    await addCodeThroughDialog('Welcome', 'Feeling accepted and at ease.');
    await addCodeThroughDialog('Cost');
    await addCodeThroughDialog('Staff', 'People who work at the program.');
    for (const name of ['Welcome', 'Cost', 'Staff']) {
      await codeRow(name).waitFor();
    }
    const codes = await rpc('code.list');
    h.equal(codes.map((c) => c.name).sort(), ['Cost', 'Staff', 'Welcome'], 'the engine has the three codes');
    h.check(new Set(codes.map((c) => c.color)).size === 3, 'each code got its own color');
    await axe('the workspace with a codebook');
  });

  await S('a code with no definition shows a gentle reminder', async () => {
    await openDefinition('Cost');
    const panel = page.getByTestId('code-detail');
    await panel.waitFor();
    h.check(/no definition yet/.test(await panel.textContent()), 'the reminder is shown for Cost');
    await closeDefinition();
    await openDefinition('Welcome');
    await page.locator('#code-name').waitFor();
    h.check(!/no definition yet/.test(await page.getByTestId('code-detail').textContent()), 'no reminder for Welcome');
    await axe('the code definition window');
    await closeDefinition();
  });

  await S('coding with the mouse: select words, then double-click a code', async () => {
    const picked = await selectByMouse('felt welcome');
    h.equal(picked, 'felt welcome', 'the mouse selection');
    await page.waitForFunction(() => /Selected: 'felt welcome'/.test(document.querySelector('[data-testid=selection-line]').textContent));
    await codeRow('Welcome').dblclick();
    const said = await waitStatus(/Coded 'felt welcome' as Welcome/);
    h.check(said.includes("Coded 'felt welcome' as Welcome"), `announced: ${said}`);
    const text = await sourceText();
    const codings = await rpc('coding.list', { source_id: 1 });
    h.equal(codings.length, 1, 'one coding stored');
    h.equal({ start: codings[0].start, end: codings[0].end }, expectedSpan(text, 'felt welcome'), 'offsets');
    // The words stay selected (checked first: opening a list below moves the focus).
    h.equal(await selection(), 'felt welcome', `the selection is kept after coding (focus is on ${await page.evaluate(() => `${document.activeElement?.tagName}.${document.activeElement?.className?.toString().slice(0, 30)}`)})`);
    await page.locator('.stripe[data-coding]').first().waitFor();
    const stripe = page.locator('.stripe[data-coding]').first();
    h.equal(await stripe.getAttribute('title'), 'Welcome', 'the marker names its code in its tooltip, not only by color');
    h.check((await stripe.getAttribute('class')).includes('pattern-'), 'the marker keeps its pattern class');
    h.equal(await page.locator('[data-testid=gutter]').getAttribute('aria-hidden'), 'true', 'stripes are hidden from screen readers');
    // Codings here is a collapsed list; opening it shows the codings of the paragraph in words.
    await openCodingsHere();
    const here = page.getByTestId('codings-here-list');
    await here.waitFor();
    h.check(/Welcome/.test(await here.textContent()) && /felt welcome/.test(await here.textContent()), 'the Codings here list names the code and the words');
    h.check((await page.getByRole('row', { name: /^Welcome, 1 coding/ }).count()) === 1, 'the codebook count went up');
    await page.screenshot({ path: `${ctx.shots}/after-first-coding.png` });
    await axe('the document with a coding');
  });

  await S('favorites: pin codes to the number keys', async () => {
    for (const [name, slot] of [['Welcome', 1], ['Cost', 2], ['Staff', 3]]) {
      await codeRow(name).click();
      await api.codeAction('Pin as favorite');
      const said = await waitStatus(new RegExp(`${name} is now favorite number ${slot}`));
      h.check(said.includes(`favorite number ${slot}`), `announced: ${said}`);
    }
    // The coding row above the text is gone; the favorite's number is told in the code's row, and the bar over selected words has the buttons.
    for (const [name, slot] of [['Welcome', 1], ['Cost', 2], ['Staff', 3]]) {
      h.check(await page.getByRole('row', { name: new RegExp(`^${name},.*favorite ${slot}`) }).count() === 1, `the row of ${name} says it is favorite ${slot}`);
    }
    h.equal(await page.evaluate(() => Object.keys(localStorage).filter((k) => k.startsWith('prismqda.favorites')).length), 1, 'favorites are remembered in local storage');
  });

  await S('keyboard only: Shift+arrows select, a number key applies a favorite', async () => {
    const phrase = 'The staff greeted me';
    const picked = await selectByKeyboard(phrase);
    h.equal(picked, phrase, 'the keyboard selection');
    await page.waitForFunction((p) => document.querySelector('[data-testid=selection-line]').textContent.includes(p), phrase);
    await page.keyboard.press('3');
    const said = await waitStatus(/Coded 'The staff greeted me' as Staff/);
    h.check(said.includes("Coded 'The staff greeted me' as Staff"), `announced: ${said}`);
    const c = await lastCoding();
    h.equal({ start: c.start, end: c.end }, expectedSpan(await sourceText(), phrase), 'offsets');
    h.equal(c.code_id, (await rpc('code.list')).find((x) => x.name === 'Staff').id, 'the code is Staff');
    h.check(await surface().evaluate((el) => document.activeElement === el), 'the focus stays in the text');
    h.equal(await selection(), phrase, 'the selection is still there, so another code can be added to it');
  });

  await S('the caret announces the codes it is in', async () => {
    const text = await surfaceText();
    // A note about the caret is held back for a moment after an action is announced.
    await sleep(2700);
    await caretFromStart(graphemesBefore(text, 'The staff greeted me') + 5);
    await sleep(900);
    h.check(/In Staff/.test(await waitStatus(/In Staff/)), 'entering coded text says which code');
    await caretFromStart(graphemesBefore(text, 'This is the last paragraph') + 3);
    h.check(/Outside any coding/.test(await waitStatus(/Outside any coding/)), 'leaving coded text says so');
  });

  await S('Enter opens the code box, typing finds a code, Enter applies it', async () => {
    const phrase = 'hard part';
    h.equal(await selectByKeyboard(phrase), phrase, 'the keyboard selection');
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => document.activeElement?.getAttribute('role') === 'combobox');
    await page.keyboard.type('cos');
    await page.getByRole('option', { name: 'Cost' }).waitFor();
    // The list of matches is a pop-up that React Aria places at the end of the page. It belongs to
    // the box through aria-controls, so the "all content in landmarks" rule is not applied to it.
    await axe('the code box with its list of matches open', { exclude: ['.popover'] });
    await page.keyboard.press('Enter');
    const said = await waitStatus(/Coded 'hard part' as Cost/);
    h.check(said.includes("Coded 'hard part' as Cost"), `announced: ${said}`);
    await page.waitForFunction(() => document.activeElement?.getAttribute('data-testid') === 'surface');
    h.check(true, 'the focus went back to the text');
    h.equal(await selection(), 'hard part', 'the selection is back too, not a caret at the top of the document');
  });

  await S('Enter on a code in the codebook applies it to the selection the text still remembers', async () => {
    // The selection ('hard part') was made in the text; moving to the codebook must not lose it.
    await codeRow('Staff').click();
    h.check(await page.evaluate(() => !!document.activeElement?.closest('[role=treegrid]')), 'the focus is in the codebook');
    await page.keyboard.press('Enter');
    const said = await waitStatus(/Coded 'hard part' as Staff/);
    h.check(said.includes("Coded 'hard part' as Staff"), `announced: ${said}`);
  });

  await S('offsets are right after an emoji (a character above U+FFFF)', async () => {
    const phrase = '\u{1F600} Cost';
    h.equal(await selectByKeyboard(phrase), phrase, 'the keyboard selection');
    await page.keyboard.press('2');
    const said = await waitStatus(/Coded '.*Cost' as Cost/);
    h.check(said.includes(`Coded '${phrase}' as Cost`), `the engine quoted the same words: ${said}`);
    const text = await sourceText();
    const c = await lastCoding();
    h.equal({ start: c.start, end: c.end }, expectedSpan(text, phrase), 'scalar offsets, not UTF-16');
    h.check(c.end - c.start === 6, 'the span is six characters');
  });

  await S('offsets are right for CJK and right-to-left text selected with the mouse', async () => {
    const text = await sourceText();
    const cjk = '我感到很受欢迎。';
    h.equal(await selectByMouse(cjk), cjk, 'the CJK selection');
    await codeRow('Welcome').dblclick();
    await waitStatus(/Coded '我/);
    let c = await lastCoding();
    h.equal({ start: c.start, end: c.end }, expectedSpan(text, cjk), 'CJK offsets');
    const hebrew = 'שלום עולם';
    h.equal(await selectByMouse(hebrew), hebrew, 'the Hebrew selection');
    await codeRow('Staff').dblclick();
    await waitStatus(/Coded 'ש/);
    c = await lastCoding();
    h.equal({ start: c.start, end: c.end }, expectedSpan(text, hebrew), 'Hebrew offsets');
    await page.screenshot({ path: `${ctx.shots}/rtl-cjk-coded.png` });
  });

  await S('offsets are right with a combining accent that has no single-character form', async () => {
    const phrase = 'q́ apart';
    h.equal(await selectByKeyboard(phrase), phrase, 'the keyboard selection');
    await page.keyboard.press('1');
    await waitStatus(/Coded 'q/);
    const c = await lastCoding();
    h.equal({ start: c.start, end: c.end }, expectedSpan(await sourceText(), phrase), 'offsets');
    h.equal(c.end - c.start, 8, 'two scalars for the letter and its accent');
  });

  await S('in-vivo coding makes a code from the selected words and applies it', async () => {
    const phrase = 'Naïve plans';
    h.equal(await selectByKeyboard(phrase), phrase, 'the keyboard selection');
    await page.keyboard.press('i');
    const said = await waitStatus(/Coded 'Naïve plans' as Naïve plans/);
    h.check(said.includes("Coded 'Naïve plans' as Naïve plans"), `announced: ${said}`);
    const codes = await rpc('code.list');
    h.check(codes.some((c) => c.name === phrase && c.parent_id === null), 'the code exists at the top of the codebook');
    await codeRow(phrase).waitFor();
    const c = await lastCoding();
    h.equal({ start: c.start, end: c.end }, expectedSpan(await sourceText(), phrase), 'offsets');
    // A second in-vivo on the same words reuses the code instead of failing.
    await page.keyboard.press('i');
    await waitStatus(/Coded/);
    h.equal((await rpc('code.list')).filter((x) => x.name === phrase).length, 1, 'no duplicate code');
  });

  await S('repeat last code applies the same code to a new selection', async () => {
    const phrase = 'last paragraph';
    h.equal(await selectByKeyboard(phrase), phrase, 'the keyboard selection');
    await page.keyboard.press('r');
    const said = await waitStatus(/Coded 'last paragraph' as Naïve plans/);
    h.check(said.includes("Coded 'last paragraph' as Naïve plans"), `announced: ${said}`);
  });

  await S('a coding can be opened, changed, annotated, marked as an example, and removed', async () => {
    const welcome = (await rpc('code.list')).find((x) => x.name === 'Welcome');
    const codings = await rpc('coding.list', { source_id: 1 });
    const first = codings.find((c) => c.code_id === welcome.id && Array.from('felt welcome').length === c.end - c.start);
    await page.locator(`.stripe[data-coding="${first.id}"]`).click();
    const detail = page.getByTestId('coding-detail');
    await detail.waitFor();
    h.equal((await page.getByTestId('coding-quote').textContent()).trim(), '“felt welcome”', 'the quote is shown');
    await axe('the coding details');

    // Changes to the coding are behind "Options for this coding".
    await openCodingOptions();
    await detail.getByLabel('Code', { exact: true }).selectOption({ label: 'Cost' });
    await waitStatus(/Changed the coding to Cost/);
    h.equal((await rpc('coding.list', { source_id: 1 })).find((c) => c.id === first.id).code_id, (await rpc('code.list')).find((x) => x.name === 'Cost').id, 'the engine changed the code');
    await detail.getByLabel('Code', { exact: true }).selectOption({ label: 'Welcome' });
    await waitStatus(/Changed the coding to Welcome/);

    await detail.getByRole('textbox', { name: /Note: why did you code it this way/ }).fill('explicit feeling');
    await detail.getByRole('button', { name: 'Save note' }).click();
    await waitStatus(/Saved the note/);
    h.equal((await rpc('coding.list', { source_id: 1 })).find((c) => c.id === first.id).rationale, 'explicit feeling', 'the note was saved');

    await detail.getByText(/Use as an example of Welcome/).click();
    await waitStatus(/Marked this coding as an example/);
    h.check(await detail.getByRole('checkbox', { name: /Use as an example of Welcome/ }).isChecked(), 'the example box is checked');
    h.check((await rpc('events.list', { limit: 500 })).some((e) => e.action === 'anchor.add' && e.target_id === first.id), 'the engine logged the example');

    await detail.getByRole('button', { name: 'Remove this coding…' }).click();
    await detail.getByRole('button', { name: 'Yes, remove this coding' }).click();
    await waitStatus(/Removed the coding 'felt welcome'/);
    h.check(!(await rpc('coding.list', { source_id: 1 })).some((c) => c.id === first.id), 'the coding is gone');
    h.check((await page.getByTestId('coding-detail-empty').count()) === 1, 'the details area goes back to its empty message');
  });

  // Links between passages with a named relation (decision 162): made from Segment Details, listed on the Synthesize page, removed, taken back.
  let linkIds = [];
  await S('two passages are linked with a named relation from Segment Details, and the link shows on both', async () => {
    const plain = (await rpc('coding.list', { source_id: 1 })).filter((c) => !c.region && c.end - c.start >= 8);
    h.check(plain.length >= 2, 'the open document has two coded passages to link');
    const [a, b] = plain;
    const bWords = (await rpc('segments.list', { coding_ids: [b.id] })).segments[0].text;
    await page.locator(`.stripe[data-coding="${a.id}"]`).click();
    const detail = page.getByTestId('coding-detail');
    await detail.waitFor();
    h.check(/not linked to another yet/.test(await page.getByTestId('seg-links').innerText()), 'a passage with no links says so');
    await page.getByTestId('link-add').click();
    await page.getByTestId('link-here').waitFor();
    await page.getByTestId('link-save').click();
    await page.getByRole('alert').filter({ hasText: 'Choose the passage to link to' }).waitFor();
    await page.getByTestId('link-search').fill(bWords.slice(0, 10));
    await page.getByTestId('link-choice').first().click();
    await page.getByTestId('link-save').click();
    await page.getByRole('alert').filter({ hasText: 'in a word or two' }).waitFor();
    await page.getByTestId('link-relation-input').fill('explains');
    await axe('the dialog that links two passages');
    await page.getByTestId('link-save').click();
    await page.getByTestId('passage-link').first().waitFor();
    const there = await page.getByTestId('seg-links').innerText();
    h.check(/Links \(1\)/.test(there) && /This passage explains/.test(there), `the first passage shows the link: ${there.replace(/\s+/g, ' ').slice(0, 160)}`);
    const links = await rpc('link.list', {});
    linkIds = links.map((l) => l.id);
    h.equal([links.length, links[0].relation, links[0].from.coding_id, links[0].to.coding_id], [1, 'explains', a.id, b.id], 'the engine holds one link, from the first passage to the second');
    // the other end sees it the other way round
    await page.locator(`.stripe[data-coding="${b.id}"]`).click();
    await page.waitForFunction(() => /explains this passage/.test(document.querySelector('[data-testid=seg-links]')?.textContent ?? ''), null, { timeout: 5000 });
    h.check(true, 'the second passage shows it as "explains this passage"');
    await axe('Segment Details with a link');
  });

  await S('the Synthesize page lists the link with both passages, and removing it can be taken back with Undo', async () => {
    await mainTab('Synthesize').click();
    await page.getByTestId('links-page').waitFor();
    const listed = await page.getByTestId('links-page').innerText();
    h.check(/Links between passages/.test(listed) && /explains/.test(listed), 'the page lists the link and its relation');
    h.equal(await page.getByTestId('passage-link').count(), 1, 'one link');
    await axe('the Synthesize page with a link');
    await page.getByTestId('link-remove').click();
    await page.getByTestId('links-empty').waitFor();
    h.equal((await rpc('link.list', {})).length, 0, 'removed');
    await rpc('undo');
    await mainTab('Code').click();
    await mainTab('Synthesize').click();
    await page.getByTestId('passage-link').first().waitFor();
    h.equal((await rpc('link.list', {})).map((l) => l.id), linkIds, 'Undo put the link back with its number');
    await mainTab('Code').click();
  });

  void sleep;
}
