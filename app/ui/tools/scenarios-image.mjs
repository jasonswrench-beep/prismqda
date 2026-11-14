// Pictures: importing one, drawing areas with the pointer and with the keys, coding them like words, the list of coded areas, zoom and
// turn, moving an area, the description, and what the lists and Explore show for an area. The picture is a small PNG the check makes itself.

import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

function crc32(buf) {
  let c;
  let crc = 0xffffffff;
  for (let n = 0; n < buf.length; n++) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

/**
 * A JPEG that is only a header, with the facts a camera writes into a photograph: made by Nikon, taken at `moment` (`YYYY:MM:DD HH:MM:SS`),
 * and where it was (51 deg 30 min north, 0 deg 7 min 39 sec west). It cannot be drawn, but the program reads only the facts.
 */
export function makePhotograph(moment) {
  const u16 = (v) => {
    const b = Buffer.alloc(2);
    b.writeUInt16BE(v);
    return b;
  };
  const u32 = (v) => {
    const b = Buffer.alloc(4);
    b.writeUInt32BE(v);
    return b;
  };
  const entry = (tag, kind, count, value) => Buffer.concat([u16(tag), u16(kind), u32(count), value]);
  const side = (letter) => Buffer.from([letter.charCodeAt(0), 0, 0, 0]);
  const tiff = Buffer.concat([
    Buffer.from([0x4d, 0x4d, 0, 0x2a]), u32(8),
    u16(3), entry(0x010f, 2, 6, u32(50)), entry(0x0132, 2, 20, u32(56)), entry(0x8825, 4, 1, u32(76)), u32(0),
    Buffer.from('Nikon\0', 'latin1'), Buffer.from(`${moment}\0`, 'latin1'),
    u16(4), entry(0x0001, 2, 2, side('N')), entry(0x0002, 5, 3, u32(130)), entry(0x0003, 2, 2, side('W')), entry(0x0004, 5, 3, u32(154)), u32(0),
    ...[51, 1, 30, 1, 0, 1, 0, 1, 7, 1, 39, 1].map(u32),
  ]);
  const app1 = Buffer.concat([Buffer.from('Exif\0\0', 'latin1'), tiff]);
  const length = Buffer.alloc(2);
  length.writeUInt16BE(app1.length + 2);
  return Buffer.concat([Buffer.from([0xff, 0xd8, 0xff, 0xe1]), length, app1, Buffer.from([0xff, 0xc0, 0, 17, 8, 0, 100, 0, 200, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1])]);
}

/** A PNG of four coloured quarters, so a thumbnail of one corner is plainly that corner. */
export function makePng(w, h) {
  const rows = [];
  for (let y = 0; y < h; y++) {
    const row = Buffer.alloc(1 + w * 3);
    for (let x = 0; x < w; x++) {
      const left = x < w / 2;
      const top = y < h / 2;
      const [r, g, b] = top ? (left ? [230, 90, 70] : [70, 150, 230]) : left ? [90, 190, 110] : [240, 200, 70];
      row[1 + x * 3] = r;
      row[2 + x * 3] = g;
      row[3 + x * 3] = b;
    }
    rows.push(row);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(Buffer.concat(rows))), chunk('IEND', Buffer.alloc(0))]);
}

export async function runImages(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const importDialog = () => page.getByRole('main', { name: 'Import Data' });
  const stage = () => page.getByTestId('image-stage');
  const canvas = () => page.getByTestId('image-canvas');
  const tool = (id) => page.getByTestId(`image-tool-${id}`);
  const documentRow = (name) => page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: new RegExp(name) }).first();
  let source;
  let joy;
  let fear;

  /** A point of the picture (fractions) to a place on the screen, with the picture unturned. */
  const at = async (fx, fy) => {
    const box = await canvas().boundingBox();
    return { x: box.x + fx * box.width, y: box.y + fy * box.height };
  };
  const drag = async (from, to, steps = 8) => {
    const a = await at(...from);
    const b = await at(...to);
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move(b.x, b.y, { steps });
    await page.mouse.up();
  };
  const regions = async () => rpc('region.list', { source_id: source.id });
  const near = (a, b, eps = 0.02) => Math.abs(a - b) <= eps;

  await S('a picture is imported from Import Data, and the page says it is kept inside the project', async () => {
    const png = path.join(workDir, 'street scene.png');
    fs.writeFileSync(png, makePng(320, 200));
    joy = (await rpc('code.add', { name: 'Picture joy' })).id;
    fear = (await rpc('code.add', { name: 'Picture fear' })).id;
    await api.goToImport();
    const dialog = importDialog();
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(png);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const note = dialog.getByTestId('picture-note');
    await note.waitFor();
    h.check(/kept inside the project/.test(await note.innerText()), 'the page says the picture is kept inside the project');
    h.check(/Picture/.test(await dialog.getByTestId('import-queue').innerText()), 'what comes in is a picture');
    h.equal(await dialog.getByRole('button', { name: /Replace the text of an existing document/ }).count(), 0, 'a picture cannot be offered as the new version of a document');
    await axe('the Import Data page with a picture');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    await page.getByTestId('import-finished').waitFor();
    source = (await rpc('source.list')).find((s) => s.name === 'street scene');
    h.equal([source.image.width, source.image.height, source.image.mime], [320, 200, 'image/png'], 'the picture is a document that knows its size');
    await page.getByTestId('import-finished').getByRole('button', { name: 'Done' }).click();
    // The codes were made behind the interface's back, so it is loaded again to know them.
    await page.reload();
    await documentRow('street scene').click();
  });

  await S('the reader shows the picture fitted to the window, with tools, and says what it is', async () => {
    await page.getByTestId('image-view').waitFor();
    await page.waitForFunction(() => {
      const i = document.querySelector('[data-testid=image-picture]');
      return !!i && i.complete && i.naturalWidth > 0;
    }, null, { timeout: 8000 });
    h.equal(await page.evaluate(() => document.querySelector('[data-testid=image-picture]').naturalWidth), 320, 'the picture was loaded from the project');
    h.check(/Picture/.test(await page.getByTestId('source-facts').innerText()) && /320 by 200 pixels/.test(await page.getByTestId('source-facts').innerText()), 'the header says it is a picture and how large');
    h.check(/Pictures are coded by area/.test(await page.getByTestId('image-no-areas').innerText()), 'the list below says no area is coded yet and how to start');
    h.equal(await tool('select').getAttribute('aria-pressed'), 'true', 'the Select tool is on to begin with');
    h.check((await page.getByTestId('image-zoom').innerText()).endsWith('%'), 'the zoom is shown');
    await page.screenshot({ path: `${ctx.shots}/image-reader.png`, fullPage: true });
    await axe('the reader with a picture');
  });

  await S('dragging a rectangle makes an area that waits for a code, and the bar of codes appears under it', async () => {
    await tool('rectangle').click();
    await drag([0.1, 0.1], [0.45, 0.4]);
    await page.getByTestId('region-draft').waitFor();
    const selection = await page.locator('[data-testid=selection-line]').innerText();
    h.check(/Selected: Rectangle, 10% to 45% across and 10% to 40% down/.test(selection), `the status line tells the area: ${selection}`);
    await page.getByTestId('image-float').waitFor();
    h.equal((await regions()).length, 0, 'nothing is stored until a code is applied');
    await page.screenshot({ path: `${ctx.shots}/image-draft.png`, fullPage: true });
    await axe('a picture with an area drawn and waiting for a code');
  });

  await S('a code chosen from the bar of codes is given to the area, and it is listed, drawn, and shown in Segment Details', async () => {
    await page.getByTestId('image-float').getByRole('button', { name: /Other code/ }).click();
    const chooser = page.getByRole('dialog', { name: 'Apply a code' });
    await chooser.waitFor();
    await chooser.getByRole('combobox').fill('Picture joy');
    await page.keyboard.press('Enter');
    await chooser.waitFor({ state: 'detached' });
    await page.waitForFunction(() => document.querySelectorAll('[data-testid^=region-]:not([data-testid=region-draft]):not([data-testid=region-drawing]):not([data-testid=region-thumb])').length === 1, null, { timeout: 5000 });
    const rows = await regions();
    h.equal([rows.length, rows[0].code_id, rows[0].shape], [1, joy, 'rectangle'], 'one area is stored, with the code');
    h.check(near(rows[0].x, 0.1) && near(rows[0].y, 0.1) && near(rows[0].w, 0.35) && near(rows[0].h, 0.3), `and its shape is what was dragged: ${JSON.stringify(rows[0])}`);
    h.equal(await page.getByTestId('region-draft').count(), 0, 'the waiting area is now a coded one');
    h.check(/Picture joy/.test(await page.getByTestId('coded-areas').innerText()), 'the list of coded areas names the code');
    const details = await page.getByTestId('coding-detail').innerText();
    h.check(/Rectangle, 10% to 45% across/.test(details) && /Applied Codes \(1\)/.test(details), 'Segment Details describes the area');
    h.equal(await page.getByTestId('segment-quote-area').count() >= 1, true, 'and shows a small picture of it');
    await page.locator('[data-testid=area-excerpt]').first().waitFor();
    h.check((await page.locator('[data-testid=area-excerpt]').count()) >= 1, 'Retrieved Segments has a row for the area, with a small picture');
    h.equal((await rpc('source.list')).find((s) => s.id === source.id).codings, 1, 'the document counts it');
    await axe('a picture with a coded area');
  });

  await S('the keys draw an ellipse, move it, change its size, and code it from the code box', async () => {
    await stage().focus();
    await page.keyboard.press('o');
    h.equal(await tool('ellipse').getAttribute('aria-pressed'), 'true', 'O chooses the Ellipse tool');
    await page.keyboard.press(' ');
    await page.getByTestId('region-draft').waitFor();
    let d = await page.evaluate(() => document.querySelector('[data-testid=selection-line]').textContent);
    h.check(/Selected: Ellipse, 40% to 60% across and 40% to 60% down/.test(d), `Space puts an ellipse in the middle: ${d}`);
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Alt+ArrowRight');
    d = await page.evaluate(() => document.querySelector('[data-testid=selection-line]').textContent);
    h.check(/Ellipse, 41% to 62% across and 45% to 65% down/.test(d), `the arrow keys moved it and Alt with an arrow changed its size: ${d}`);
    await page.keyboard.press('Enter');
    const box = page.getByRole('dialog', { name: 'Apply a code' });
    await box.waitFor();
    h.check(/Ellipse, 41% to 62%/.test(await box.getByTestId('picker-selection').innerText()), 'the code box says what it will be applied to');
    await box.getByRole('combobox').fill('Picture fear');
    await page.keyboard.press('Enter');
    await box.waitFor({ state: 'detached' });
    const rows = await regions();
    h.equal([rows.length, rows.filter((r) => r.shape === 'ellipse').length, rows.find((r) => r.shape === 'ellipse').code_id], [2, 1, fear], 'the ellipse is stored with its code');
    h.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'image-stage', 'the focus is back on the picture, so coding goes on from the keys');
  });

  await S('a second code can be given to the same area, and an area is not given a code twice', async () => {
    await page.keyboard.press('Enter');
    const box = page.getByRole('dialog', { name: 'Apply a code' });
    await box.waitFor();
    await box.getByRole('combobox').fill('Picture joy');
    await page.keyboard.press('Enter');
    await box.waitFor({ state: 'detached' });
    const ellipse = (await regions()).filter((r) => r.shape === 'ellipse');
    h.equal(ellipse.length, 2, 'the same ellipse now has two codes');
    h.equal(JSON.stringify([ellipse[0].x, ellipse[0].y, ellipse[0].w, ellipse[0].h]), JSON.stringify([ellipse[1].x, ellipse[1].y, ellipse[1].w, ellipse[1].h]), 'with exactly the same shape');
    const rowsInList = await page.getByTestId('coded-areas').getByRole('listitem').count();
    h.equal(rowsInList, 2, 'the list shows the two areas once each, however many codes they have');
    await page.keyboard.press('Enter');
    await box.waitFor();
    await box.getByRole('combobox').fill('Picture joy');
    await page.keyboard.press('Enter');
    await box.waitFor({ state: 'detached' });
    h.equal((await regions()).filter((r) => r.shape === 'ellipse').length, 2, 'giving it the same code again changes nothing');
  });

  await S('an outline is built from corners with the keys and finished with Enter', async () => {
    await page.keyboard.press('Escape');
    await page.keyboard.press('p');
    await page.getByTestId('crosshair').waitFor();
    for (const key of ['ArrowLeft', 'ArrowLeft', 'ArrowUp']) await page.keyboard.press(`Shift+${key}`);
    await page.keyboard.press(' ');
    await page.keyboard.press('Shift+ArrowRight');
    await page.keyboard.press('Shift+ArrowRight');
    await page.keyboard.press('Shift+ArrowRight');
    await page.keyboard.press('Shift+ArrowRight');
    await page.keyboard.press(' ');
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press(' ');
    h.equal(await page.getByTestId('outline-progress').count(), 1, 'the corners placed so far are drawn');
    await page.keyboard.press('Enter');
    await page.getByTestId('region-draft').waitFor();
    const d = await page.evaluate(() => document.querySelector('[data-testid=selection-line]').textContent);
    h.check(/Outline of 3 points/.test(d), `Enter finishes the outline: ${d}`);
    await page.keyboard.press('Delete');
    h.equal(await page.getByTestId('region-draft').count(), 0, 'Delete throws away an area that has no code yet');
    h.equal((await regions()).length, 3, 'and nothing was stored');
  });

  await S('an outline drawn with the pointer is coded, and turns with the picture', async () => {
    await tool('polygon').click();
    const corners = [[0.55, 0.55], [0.9, 0.6], [0.8, 0.95]];
    for (const c of corners) {
      const p = await at(...c);
      await page.mouse.click(p.x, p.y);
    }
    await page.getByRole('button', { name: 'Finish outline' }).click();
    await page.getByTestId('region-draft').waitFor();
    await page.getByTestId('image-float').getByRole('button', { name: /the code used last/ }).click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=coded-areas] li').length === 3, null, { timeout: 5000 });
    const rows = await regions();
    const poly = rows.find((r) => r.shape === 'polygon');
    h.equal([rows.length, poly.points.length, poly.code_id], [4, 3, joy], 'the outline is stored with its three corners and the code the bar offered as the one used last');

    // Turned a quarter turn, the area stays on the same part of the picture.
    const before = await page.evaluate((id) => {
      const g = document.querySelector(`[data-testid=region-${-id}] .img-shape`);
      const c = document.querySelector('[data-testid=image-canvas]').getBoundingClientRect();
      const b = g.getBoundingClientRect();
      return { x: (b.x - c.x) / c.width, y: (b.y - c.y) / c.height, w: b.width / c.width, h: b.height / c.height };
    }, poly.id);
    await page.getByTestId('image-turn').click();
    h.equal(await canvas().getAttribute('data-turns'), '1', 'the picture is turned');
    const after = await page.evaluate((id) => {
      const g = document.querySelector(`[data-testid=region-${-id}] .img-shape`);
      const c = document.querySelector('[data-testid=image-canvas]').getBoundingClientRect();
      const b = g.getBoundingClientRect();
      return { x: (b.x - c.x) / c.width, y: (b.y - c.y) / c.height, w: b.width / c.width, h: b.height / c.height };
    }, poly.id);
    // A clockwise quarter turn sends the box (x, y, w, h) to (1 - y - h, x, h, w).
    h.check(near(after.x, 1 - before.y - before.h) && near(after.y, before.x) && near(after.w, before.h) && near(after.h, before.w), `the area turned with the picture: ${JSON.stringify(before)} then ${JSON.stringify(after)}`);
    await page.screenshot({ path: `${ctx.shots}/image-turned.png`, fullPage: true });
    await axe('a picture turned a quarter turn');
    await page.getByTestId('image-turn').click();
    await page.getByTestId('image-turn').click();
    await page.getByTestId('image-turn').click();
    h.equal(await canvas().getAttribute('data-turns'), '0', 'four turns are no turn');
  });

  await S('zoom steps, fit, and one to one change the size the picture is drawn at', async () => {
    const zoomOf = async () => Number(await canvas().getAttribute('data-zoom'));
    const fit = await zoomOf();
    await page.getByTestId('image-zoom-in').click();
    const bigger = await zoomOf();
    h.check(bigger > fit, `zoom in makes it larger: ${fit} then ${bigger}`);
    await page.getByTestId('image-actual').click();
    h.check(near(await zoomOf(), 1, 0.001), 'one to one draws a picture pixel as a screen pixel');
    h.check((await page.getByTestId('image-zoom').innerText()) === '100%', 'and says 100%');
    await page.getByTestId('image-zoom-out').click();
    h.check((await zoomOf()) < 1, 'zoom out makes it smaller');
    await page.getByTestId('image-fit').click();
    h.check(near(await zoomOf(), fit, 0.001), 'fit goes back to the first size');
  });

  await S('a coded area is chosen from the list, moved with the keys, and the change is one change that undo takes back', async () => {
    const before = (await regions()).find((r) => r.shape === 'rectangle');
    await page.getByTestId(`area-button-${-before.id}`).click();
    await page.waitForFunction((id) => document.querySelector(`[data-testid=region-${id}]`)?.classList.contains('chosen'), -before.id, { timeout: 5000 });
    h.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'image-stage', 'choosing from the list puts the focus on the picture');
    for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(900);
    const moved = (await regions()).find((r) => r.id === before.id);
    h.check(near(moved.x, before.x + 0.03, 0.001) && near(moved.y, before.y, 0.001), `three presses moved the area three steps: ${before.x} to ${moved.x}`);
    const history = await rpc('coding.history', { id: -before.id });
    h.equal(history.map((x) => x.action), ['region.add', 'region.reshape'], 'three presses are one change in its history');
    await rpc('undo');
    await page.reload();
    await documentRow('street scene').click();
    await page.getByTestId('image-view').waitFor();
    const restored = (await regions()).find((r) => r.id === before.id);
    h.check(near(restored.x, before.x, 0.001), 'undo put the area back where it was');
  });

  await S('the square bracket keys move through the coded areas and say each one', async () => {
    await stage().focus();
    await page.keyboard.press(']');
    const first = await page.evaluate(() => document.querySelector('[data-testid=coding-facts]') !== null);
    h.check(first, 'the first area is chosen and Segment Details shows it');
    const chosen = await page.locator('.img-area.chosen').count();
    h.equal(chosen, 1, 'one area is drawn as chosen');
    await page.keyboard.press(']');
    await page.keyboard.press(']');
    await page.keyboard.press(']');
    await page.keyboard.press(']');
    h.equal(await page.locator('.img-area.chosen').count(), 1, 'going past the last comes round to the first');
    await page.keyboard.press('Escape');
    h.equal(await page.locator('.img-area.chosen').count(), 0, 'Escape lets go of the chosen area');
  });

  await S('removing a coded area with the Delete key takes only that coding away, and undo brings it back', async () => {
    await page.keyboard.press(']');
    const target = await page.evaluate(() => Number(document.querySelector('.img-area.chosen').getAttribute('data-testid').replace('region-', '')));
    const total = (await regions()).length;
    await page.keyboard.press('Delete');
    await page.waitForFunction((n) => document.querySelectorAll('[data-testid=coded-areas] li').length <= n, 3, { timeout: 5000 }).catch(() => {});
    await page.waitForTimeout(400);
    h.equal((await regions()).length, total - 1, 'one coding was removed');
    h.check(!(await regions()).some((r) => -r.id === target), 'and it was the chosen one');
    await rpc('undo');
    h.equal((await regions()).length, total, 'undo puts it back');
    await page.reload();
    await documentRow('street scene').click();
    await page.getByTestId('image-view').waitFor();
  });

  await S('the description of the picture is edited, saved, and spoken as the picture\'s name', async () => {
    await page.getByTestId('image-describe').click();
    await page.getByTestId('image-description-box').fill('A street at dusk with a child laughing near a parked bicycle.');
    await axe('editing the description of a picture');
    await page.getByRole('button', { name: 'Save description' }).click();
    await page.getByTestId('image-description').waitFor();
    h.equal(await page.getByTestId('image-description').innerText(), 'A street at dusk with a child laughing near a parked bicycle.', 'the description shows');
    h.check((await rpc('source.get', { id: source.id })).text.startsWith('A street at dusk'), 'and is stored');
    h.check(/A street at dusk/.test((await stage().getAttribute('aria-label')) ?? ''), 'the picture is named by it for a screen reader');
    h.equal((await regions()).length > 0, true, 'the coded areas are still there');
  });

  await S('Retrieved Segments shows the areas as a gallery under their codes, a tile opens its area, and the list comes back', async () => {
    const stored = await regions();
    const toggle = page.getByRole('button', { name: 'Show the areas of pictures and PDF pages as a gallery' });
    h.equal(await toggle.getAttribute('aria-pressed'), 'false', 'the list is what shows to begin with');
    await toggle.click();
    await page.getByTestId('results-gallery').waitFor();
    h.equal(await toggle.getAttribute('aria-pressed'), 'true', 'the gallery button says it is on');
    const heads = (await page.locator('.bk-gallery-head').allInnerTexts()).map((x) => x.replace(/\s+/g, ' ').trim());
    h.check(heads.length >= 2 && heads.some((x) => /Picture joy/.test(x)) && heads.some((x) => /Picture fear/.test(x)), `a heading for each code that has areas: ${heads.join(' | ')}`);
    h.equal(await page.locator('.bk-tile').count(), stored.length, 'a tile for every coded area, and no tile for anything else');
    const names = await page.locator('.bk-tile').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    h.check(names.every((n) => /^Open street scene at this area, coded Picture /.test(n ?? '')), `each tile says where it opens: ${names[0]}`);
    h.check((await page.locator('.bk-tile [data-testid=region-thumb]').count()) === stored.length, 'and shows the part of the picture the area covers');
    await axe('Retrieved Segments as a gallery');
    await page.locator('.bk-tile').first().click();
    await page.waitForFunction(() => document.querySelector('[data-testid=coding-detail]')?.textContent?.includes('Applied Codes'), null, { timeout: 5000 });
    h.check(/Applied Codes/.test(await page.getByTestId('coding-detail').innerText()), 'a tile opens its area in Segment Details');
    await page.getByRole('button', { name: 'Show the areas of pictures and PDF pages as a gallery' }).click();
    await page.getByRole('grid', { name: 'Retrieved Segments' }).waitFor();
    h.equal(await page.getByRole('button', { name: 'Show the areas of pictures and PDF pages as a gallery' }).getAttribute('aria-pressed'), 'false', 'turning it off brings the list back');
  });

  await S('a memo can be written about an area, and Segment Details counts it and names what it is about', async () => {
    const areas = await regions();
    const target = areas[0];
    await rpc('memo.add', { kind: 'analytic', scope: 'coding', target_id: -target.id, title: 'Why this corner', body: 'The child is laughing here.' });
    // The list also holds the words coded earlier, so only the rows that carry a small picture are chosen, one after the other.
    const areaRows = page.getByRole('grid', { name: 'Retrieved Segments' }).getByRole('row').filter({ has: page.locator('[data-testid=area-excerpt]') });
    const memoCounts = [];
    for (let i = 0; i < (await areaRows.count()); i++) {
      await areaRows.nth(i).click();
      await page.waitForTimeout(300);
      memoCounts.push(/Memo \(1\)/.test(await page.getByTestId('coding-detail').innerText()));
    }
    h.equal(memoCounts.length, areas.length, 'every coded area has a row');
    h.check(memoCounts.some(Boolean) && memoCounts.filter(Boolean).length === 1, `exactly one area shows its memo in Segment Details: ${memoCounts.join(', ')}`);
    const memos = await rpc('memo.list', { scope: 'coding', target_id: -target.id });
    h.equal([memos.length, memos[0].title], [1, 'Why this corner'], 'and the memo is kept against that area');
    h.equal((await rpc('memo.list', { scope: 'coding', target_id: target.id })).length, 0, 'not against the coding of words that has the same number');
  });

  await S('Explore counts areas with words, and a query returns them with a small picture', async () => {
    const m = await rpc('explore.matrix', { rows: { kind: 'documents' }, code_ids: [joy, fear], include_descendants: true });
    const row = m.rows.find((r) => r.label === 'street scene');
    h.check(row && row.total.assignments >= 4, `the matrix counts the areas: ${JSON.stringify(row && row.total)}`);
    h.check(row.total.excerpts < row.total.assignments, 'two codes on one shape are one excerpt');
    const q = await rpc('explore.query', { rule: { type: 'code', code_ids: [joy] }, result_unit: 'excerpts' });
    h.check(q.items.every((i) => i.kind === 'image' && i.region), 'a query returns the areas with their shape');
  });

  await S('the facts a camera wrote into a photograph are offered as variables, and only what is ticked is kept', async () => {
    const jpg = path.join(workDir, 'night walk.jpg');
    fs.writeFileSync(jpg, makePhotograph('2023:12:25 08:30:00'));
    const casesBefore = (await rpc('case.list')).length;
    await api.goToImport();
    const dialog = importDialog();
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(jpg);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const facts = dialog.getByTestId('picture-facts');
    await facts.waitFor();
    const text = await facts.innerText();
    h.check(/2023-12-25 08:30:00/.test(text) && /Nikon/.test(text) && /latitude and longitude/.test(text), `the page tells what the photograph says about itself: ${text.slice(0, 160)}`);
    h.check(!/51\.5|0\.12/.test(text), 'and does not print the numbers of the place');
    h.equal(await facts.locator('input:checked').count(), 0, 'nothing is ticked to begin with');
    await axe('the Import Data page with a photograph');
    await dialog.getByTestId('picture-when').click();
    h.equal(await facts.locator('input:checked').count(), 1, 'the date is ticked');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    await page.getByTestId('import-finished').waitFor();
    const cases = await rpc('case.list');
    const walk = cases.find((c) => c.name === 'night walk');
    h.check(cases.length === casesBefore + 1 && walk && walk.sources === 1, 'a case named for the picture holds it');
    const variables = await rpc('case.variables');
    const names = variables.map((v) => v.name);
    h.check(['Date taken', 'Month taken', 'Year taken'].every((n) => names.includes(n)), `the date became three variables: ${names.join(', ')}`);
    h.check(!names.includes('Camera') && !names.includes('Latitude') && !names.includes('Longitude'), 'the camera and the place were not ticked, so they were not kept');
    h.equal(JSON.stringify(variables.find((v) => v.name === 'Month taken').values), JSON.stringify([['2023-12', 1]]), 'and the month is the one the photograph says');
    await page.getByTestId('import-finished').getByRole('button', { name: 'Done' }).click();
  });
}
