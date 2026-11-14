// PDF pages: a PDF is imported with its file kept, its pages are drawn when asked for, a rectangle on a page is coded like words are, the
// pages with areas are listed, and the lists and Segment Details say which page an area is on. The PDF is the three-page file the
// importer's own tests use (the second page is blank), copied into the working folder.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';

const FIXTURE = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), '../../../crates/prismqda-import/tests/fixtures/pdf/three_pages.pdf');

export async function runPdf(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const stage = () => page.getByTestId('image-stage');
  const canvas = () => page.getByTestId('image-canvas');
  const documentRow = (name) => page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: new RegExp(name) }).first();
  let source;
  let joy;
  let fear;

  const at = async (fx, fy) => {
    const box = await canvas().boundingBox();
    return { x: box.x + fx * box.width, y: box.y + fy * box.height };
  };
  const drag = async (from, to) => {
    const a = await at(...from);
    const b = await at(...to);
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move(b.x, b.y, { steps: 8 });
    await page.mouse.up();
  };
  const regions = () => rpc('region.list', { source_id: source.id });
  const pageLoaded = (n) =>
    page.waitForFunction(
      (want) => {
        const i = document.querySelector('[data-testid=image-picture]');
        return !!i && i.complete && i.naturalWidth > 0 && i.getAttribute('data-page') === String(want);
      },
      n,
      { timeout: 10000 },
    );
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  /** Codes the rectangle just drawn with the named code, through the bar of codes under it. */
  const codeDrawn = async (name) => {
    await page.getByTestId('image-float').getByRole('button', { name: /Other code/ }).click();
    const chooser = page.getByRole('dialog', { name: 'Apply a code' });
    await chooser.waitFor();
    await chooser.getByRole('combobox').fill(name);
    await page.keyboard.press('Enter');
    await chooser.waitFor({ state: 'detached' });
  };

  await S('a PDF is imported with its file kept, and the document says it has pages', async () => {
    // A page is tall, so this scenario works in a taller window and puts the window back at its end.
    await page.setViewportSize({ width: 1366, height: 1100 });
    const file = path.join(workDir, 'garden notes.pdf');
    fs.copyFileSync(FIXTURE, file);
    joy = (await rpc('code.add', { name: 'Page joy' })).id;
    fear = (await rpc('code.add', { name: 'Page fear' })).id;
    const done = await rpc('import.run', { path: file });
    source = (await rpc('source.list')).find((s) => s.name === 'garden notes');
    h.equal(done.outcome.source_ids, [source.id], 'one document came in');
    h.equal([source.source_type, source.pdf.page_count, source.pdf.file_name, source.image], ['pdf', 3, 'garden notes.pdf', null], 'it is a PDF document that keeps its file and has three pages');
    const kept = await rpc('pdf.get', { source_id: source.id });
    h.equal(kept.pages.length, 3, 'the size of every page is kept');
    h.check(kept.pages[0].width > 100 && kept.pages[0].height > kept.pages[0].width, 'a page is taller than it is wide');
    // The codes were made behind the interface's back, so it is loaded again to know them.
    await page.reload();
    await documentRow('garden notes').click();
    await page.getByTestId('pdf-view-pages').waitFor();
    const facts = await text(page.getByTestId('source-facts'));
    h.check(/3 pages/.test(facts), `the header says how many pages: ${facts}`);
    h.equal(await page.getByTestId('pdf-view-words').getAttribute('aria-pressed'), 'true', 'the words are shown first, as they always were');
    // The words load after the header shows, and in a project with many documents that takes a moment longer.
    await page.getByText('Field Notes on a Neighbourhood Garden').first().waitFor({ timeout: 8000 }).catch(() => {});
    h.check((await page.getByText('Field Notes on a Neighbourhood Garden').count()) >= 1, 'the words of the PDF are there');
  });

  await S('the pages view shows page 1 drawn by the engine, with page and tool controls and no accessibility problems', async () => {
    await page.getByTestId('pdf-view-pages').click();
    await page.getByTestId('image-view').waitFor();
    await pageLoaded(1);
    const natural = await page.evaluate(() => document.querySelector('[data-testid=image-picture]').naturalWidth);
    h.check(natural >= 480, `a page was drawn at a real width: ${natural}`);
    h.equal(await page.getByTestId('pdf-view-pages').getAttribute('aria-pressed'), 'true', 'the Pages button is pressed');
    // A page is first fitted to the width of the window and scrolls; the whole page can be fitted too.
    const widthZoom = Number(await canvas().getAttribute('data-zoom'));
    await page.getByTestId('image-whole-page').click();
    const wholeZoom = Number(await canvas().getAttribute('data-zoom'));
    h.check(wholeZoom > 0 && wholeZoom <= widthZoom + 1e-9, `fitting the whole page is no larger than fitting its width: ${wholeZoom} against ${widthZoom}`);
    h.equal(await page.getByTestId('pdf-page-input').inputValue(), '1', 'the page box says 1');
    h.equal(await text(page.getByTestId('pdf-page-of')), 'of 3', 'and how many there are');
    h.equal(await page.getByTestId('pdf-prev').isDisabled(), true, 'there is no page before the first');
    h.equal([await page.getByTestId('image-tool-rectangle').count(), await page.getByTestId('image-tool-ellipse').count(), await page.getByTestId('image-tool-polygon').count(), await page.getByTestId('image-turn').count()], [1, 0, 0, 0], 'an area of a page is a rectangle, and a page is not turned');
    h.check(/PDF pages are coded by area/.test(await text(page.getByTestId('image-no-areas'))), 'the list below says how to begin');
    const label = await stage().getAttribute('aria-label');
    h.check(/Page 1 of 3 of garden notes\./.test(label) && /Field Notes on a Neighbourhood Garden/.test(label), `a screen reader is told the page and its words: ${label}`);
    await page.screenshot({ path: `${ctx.shots}/pdf-page.png`, fullPage: true });
    await axe('the reader with a PDF page');
  });

  await S('a rectangle drawn on page 1 is coded, stored on page 1, and told with its page', async () => {
    await page.getByTestId('image-tool-rectangle').click();
    await drag([0.1, 0.08], [0.6, 0.16]);
    await page.getByTestId('region-draft').waitFor();
    const selection = await text(page.locator('[data-testid=selection-line]'));
    h.check(/Selected: Rectangle, 10% to 60% across and 8% to 16% down, \d+% of the page/.test(selection), `the status line tells the area on the page: ${selection}`);
    await codeDrawn('Page joy');
    await page.waitForFunction(() => document.querySelectorAll('[data-testid^=region-]:not([data-testid=region-draft]):not([data-testid=region-drawing]):not([data-testid=region-thumb])').length === 1, null, { timeout: 5000 });
    const rows = await regions();
    h.equal([rows.length, rows[0].page, rows[0].code_id, rows[0].shape], [1, 1, joy, 'rectangle'], 'one area is stored, on page 1');
    h.check(/^Page 1, Rectangle/.test(rows[0].text) && /of the page$/.test(rows[0].text), `the engine tells it with its page: ${rows[0].text}`);
    const details = await text(page.getByTestId('coding-detail'));
    h.check(/Page 1, Rectangle, 10% to 60% across/.test(details) || /Rectangle, 10% to 60% across and 8% to 16% down, \d+% of the page/.test(details), `Segment Details says where: ${details}`);
    h.check((await page.getByTestId('segment-quote-area').count()) >= 1, 'and shows a small picture of the area');
    h.equal((await rpc('source.list')).find((s) => s.id === source.id).codings, 1, 'the document counts the area');
    await axe('a PDF page with a coded area');
  });

  await S('the next page has no area yet, the words of a blank page say so, and Page Down turns the page', async () => {
    await page.getByTestId('pdf-next').click();
    await pageLoaded(2);
    h.equal(await page.getByTestId('pdf-page-input').inputValue(), '2', 'the page box says 2');
    h.equal(await page.getByTestId('region-draft').count(), 0, 'no area is waiting');
    h.check(/No area of this page is coded yet/.test(await text(page.getByTestId('image-no-areas'))), 'the list says nothing is coded on this page');
    const words = await page.getByTestId('pdf-words').textContent();
    h.check(/No words were read from this page/.test(words), `the words box says the page has none: ${words}`);
    await stage().focus();
    await page.keyboard.press('PageDown');
    await pageLoaded(3);
    h.equal(await page.getByTestId('pdf-page-input').inputValue(), '3', 'Page Down went to page 3');
    h.equal(await page.getByTestId('pdf-next').isDisabled(), true, 'there is no page after the last');
    h.check(/Second Season/.test(await page.getByTestId('pdf-words-text').textContent()), 'the words of page 3 are the words read from it');
  });

  await S('an area drawn on page 3 is stored on page 3, and the pages with areas are listed', async () => {
    await page.getByTestId('image-tool-rectangle').click();
    await drag([0.15, 0.1], [0.7, 0.3]);
    await page.getByTestId('region-draft').waitFor();
    await codeDrawn('Page fear');
    await page.waitForFunction(() => document.querySelectorAll('[data-testid^=region-]:not([data-testid=region-draft]):not([data-testid=region-drawing]):not([data-testid=region-thumb])').length === 1, null, { timeout: 5000 });
    const rows = await regions();
    h.equal(rows.map((r) => [r.page, r.code_id]), [[1, joy], [3, fear]], 'the two areas are on their own pages, in page order');
    const list = page.getByTestId('pdf-pages-with');
    await list.waitFor();
    h.equal((await list.getByRole('listitem').allInnerTexts()).map((s) => s.replace(/\s+/g, ' ').trim()), ['Page 1 (1 area)', 'Page 3 (1 area)'], 'the pages that have areas are listed with how many');
    await page.getByTestId('pdf-goto-1').click();
    await pageLoaded(1);
    h.equal(await page.getByTestId('coded-areas').getByRole('listitem').count(), 1, 'page 1 shows only its own area');
    h.check(/Page joy/.test(await text(page.getByTestId('coded-areas'))), 'and it is the one coded Page joy');
    await axe('a PDF with areas on two pages');
  });

  await S('an area chosen from Retrieved Segments on another page takes the view to that page', async () => {
    await page.getByTestId('image-picture').waitFor();
    // Only the rows of this PDF's areas (a project may hold the areas of pictures too).
    const row = page.getByTestId('area-excerpt').filter({ hasText: /^Page \d+,/ });
    await row.first().waitFor();
    h.equal(await row.count(), 2, 'Retrieved Segments has a row, with a small picture, for each area');
    // The row of the area on page 3 tells its page in words.
    const third = row.filter({ hasText: /^Page 3,/ });
    h.equal(await third.count(), 1, 'one row is for page 3');
    const secondThumb = third.getByTestId('region-thumb').locator('img');
    await secondThumb.waitFor();
    await page.waitForFunction(() => [...document.querySelectorAll('[data-testid=area-excerpt] img')].every((i) => i.complete && i.naturalWidth > 0), null, { timeout: 10000 });
    h.check((await secondThumb.getAttribute('src')).includes('/pdf/'), 'the small picture of an area is cut from the page the engine drew');
    await third.click();
    await pageLoaded(3);
    h.equal(await page.getByTestId('pdf-page-input').inputValue(), '3', 'the view went to page 3');
    h.check(/Page 3, Rectangle|Rectangle, 15% to 70% across/.test(await text(page.getByTestId('coding-detail'))), 'and Segment Details has the area');
  });

  await S('the words view is the words alone, and a typed page number and the Home and End keys move through the pages', async () => {
    await page.getByTestId('pdf-view-words').click();
    await page.getByText('Field Notes on a Neighbourhood Garden').first().waitFor();
    h.equal(await page.getByTestId('image-view').count(), 0, 'the pages are not drawn in the words view');
    await page.getByTestId('pdf-view-pages').click();
    await pageLoaded(3);
    h.equal(await page.getByTestId('pdf-page-input').inputValue(), '3', 'coming back to the pages is at the page that was left');
    await page.getByTestId('pdf-page-input').fill('1');
    await page.keyboard.press('Enter');
    await pageLoaded(1);
    await page.getByTestId('pdf-page-input').fill('99');
    await page.keyboard.press('Enter');
    await pageLoaded(3);
    h.equal(await page.getByTestId('pdf-page-input').inputValue(), '3', 'a page number past the end is the last page');
    await stage().focus();
    await page.keyboard.press('Home');
    await pageLoaded(1);
    await page.keyboard.press('End');
    await pageLoaded(3);
    await page.setViewportSize({ width: 1366, height: 820 });
  });
}
