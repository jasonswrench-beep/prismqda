// Split view: a second document, read-only, beside the one being read. It is a toggle in the reader's header; the second document can be
// chosen, swapped with the first, and left behind; it cannot be coded from; and it says so when it cannot be shown.

const escapeRegExp = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

export async function runSplit(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe } = api;

  const mainName = () => page.locator('#h-source').innerText();
  const sideName = () => page.locator('#h-split').innerText();
  const documents = () => page.getByRole('treegrid', { name: 'Documents' });
  const toggle = () => page.getByTestId('split-toggle');

  async function backToCoding() {
    const back = page.getByRole('navigation', { name: 'Page' }).getByRole('button', { name: 'Back to the coding workspace' });
    if ((await back.count()) > 0) await back.first().click();
    await page.getByTestId('split-toggle').or(page.locator('#h-source')).first().waitFor();
  }

  async function openDocument(name) {
    await documents().getByRole('row', { name: new RegExp(escapeRegExp(name)) }).first().click();
    await page.waitForFunction((n) => document.querySelector('#h-source')?.textContent === n, name);
  }

  await S('Split view is a button in the reader that turns on a second document beside the one being read', async () => {
    await backToCoding();
    const sources = await rpc('source.list');
    const coded = [...sources].sort((a, b) => b.codings - a.codings);
    const first = coded[0];
    await openDocument(first.name);
    h.equal(await toggle().getAttribute('aria-pressed'), 'false', 'split view is off to begin with');
    h.equal(await page.getByTestId('split-pane').count(), 0, 'and there is no second pane');
    await toggle().click();
    await page.getByTestId('split-pane').waitFor();
    h.equal(await toggle().getAttribute('aria-pressed'), 'true', 'the button says it is on');
    const beside = await sideName();
    h.check(beside !== first.name && sources.some((s) => s.name === beside), `the second pane shows another document of the project: ${beside}`);
    await page.getByTestId('split-surface').waitFor();
    h.equal([await page.getByTestId('surface').count(), await page.getByTestId('split-surface').count()], [1, 1], 'two texts are on the screen, one in each pane');
    const other = sources.find((s) => s.name === beside);
    const words = (await rpc('source.get', { id: other.id })).text.split(/\s+/).filter(Boolean).slice(0, 3).join(' ');
    h.check((await page.getByTestId('split-surface').innerText()).replace(/\s+/g, ' ').includes(words), `the second document's own words are in it: ${words}`);
    h.check(/character/.test(await page.getByTestId('split-facts').innerText()), 'its header says what it is');
    await page.screenshot({ path: `${ctx.shots}/split-view.png`, fullPage: true });
    await axe('the reader with split view on');
  });

  await S('the second document can be chosen, shows its coding, and cannot be coded from', async () => {
    const sources = await rpc('source.list');
    const coded = [...sources].sort((a, b) => b.codings - a.codings);
    const main = await mainName();
    const choice = coded.find((s) => s.name !== main && s.codings > 0);
    await page.getByLabel('Document shown beside').selectOption(String(choice.id));
    await page.waitForFunction((n) => document.querySelector('#h-split')?.textContent === n, choice.name);
    h.equal(await sideName(), choice.name, 'choosing a document puts it beside');
    await page.getByTestId('split-surface').waitFor();
    await page.locator('[data-testid=split-pane] .stripe').first().waitFor();
    h.check((await page.locator('[data-testid=split-pane] .stripe').count()) >= 1, 'its coded words are marked in the margin, as in the reader');
    const before = (await rpc('coding.list', { source_id: choice.id })).length;
    const text = await page.getByTestId('split-surface').innerText();
    await page.getByTestId('split-surface').click();
    await page.keyboard.type('x1');
    await page.keyboard.press('Enter');
    h.equal(await page.getByTestId('split-surface').innerText(), text, 'typing into it changes nothing');
    h.equal((await rpc('coding.list', { source_id: choice.id })).length, before, 'and a favorite code key makes no coding in it');
    h.equal((await rpc('source.get', { id: choice.id })).text.length > 0, true, 'its text is as it was');
  });

  await S('sides can be swapped, the second document stays while the first is changed, and a clash is told', async () => {
    const left = await mainName();
    const right = await sideName();
    await page.getByTestId('split-swap').click();
    await page.waitForFunction((n) => document.querySelector('#h-source')?.textContent === n, right);
    h.equal([await mainName(), await sideName()], [right, left], 'Swap sides reads the other document and puts the first beside it');
    // Choosing the document that is beside, in the list of documents, leaves it on both sides: the pane says so instead of drawing it twice.
    await openDocument(left);
    h.equal(await sideName(), left, 'the pane still shows what was put there');
    await page.getByTestId('split-pane').getByRole('alert').or(page.getByTestId('split-pane').getByRole('status')).first().waitFor();
    h.check(/being read on the left/.test(await page.getByTestId('split-pane').innerText()), 'a document that is on both sides is not drawn twice: the pane says to choose another');
    h.equal(await page.getByTestId('split-surface').count(), 0, 'and has no second text');
    await axe('split view with the same document on both sides');
    const sources = await rpc('source.list');
    const next = sources.find((s) => s.name !== left);
    await page.getByLabel('Document shown beside').selectOption(String(next.id));
    await page.getByTestId('split-surface').waitFor();
    const third = sources.find((s) => s.name !== left && s.name !== next.name);
    await openDocument(third.name);
    h.equal([await mainName(), await sideName(), await page.getByTestId('split-pane').count()], [third.name, next.name, 1], 'changing the first document keeps the second beside it');
  });

  await S('split view is turned off with the button or the pane, and leaves the reader as it was', async () => {
    await page.getByTestId('split-close').click();
    await page.getByTestId('split-pane').waitFor({ state: 'hidden' });
    h.equal(await toggle().getAttribute('aria-pressed'), 'false', 'the button says it is off');
    h.equal([await page.getByTestId('surface').count(), await page.getByTestId('split-surface').count()], [1, 0], 'one text is on the screen again');
    await toggle().click();
    await page.getByTestId('split-pane').waitFor();
    await toggle().click();
    await page.getByTestId('split-pane').waitFor({ state: 'hidden' });
    h.check(true, 'the button turns it off too');
    await axe('the reader after split view is turned off');
  });
}
