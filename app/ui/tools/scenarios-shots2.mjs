// Pictures of the parts of PrismQDA built since the first set for the website (themes, the Team page, the working set, the document
// portrait, pictures, PDF pages, recordings, mixed methods, privacy). This is not a test: it drives the interface in the headless browser
// and saves a picture of each page, from the made-up Urban Mobility Study that comes with the program and a few invented additions
// (a bus stop drawn here, a second coder, a silent recording, the sample PDF of the tests, and 36 invented survey respondents).
//
//   PRISMQDA_ONLY=shots2 PRISMQDA_SHOTS_DIR=<folder> PRISMQDA_SHOTS_SIZE=1600x1000 PRISMQDA_SHOTS_SCALE=1 node tools/browser-check.mjs   (after npm run build)
//
// Nothing here is real participant text. The recording is silent and is never played. The pictures are of the interface in a headless
// browser, which draws the same page the window does; they are not pictures of the Windows window frame.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { SITES, COMMENTS, CODES, seeded } from './scenarios-shots.mjs';

const PDF_FIXTURE = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), '../../../crates/prismqda-import/tests/fixtures/pdf/three_pages.pdf');

/** Twenty seconds of silence as an 8-bit mono WAV. */
function silentWav(seconds = 20, rate = 8000) {
  const samples = seconds * rate;
  const buf = Buffer.alloc(44 + samples, 0x80);
  buf.write('RIFF', 0);
  buf.writeUInt32LE(36 + samples, 4);
  buf.write('WAVE', 8);
  buf.write('fmt ', 12);
  buf.writeUInt32LE(16, 16);
  buf.writeUInt16LE(1, 20);
  buf.writeUInt16LE(1, 22);
  buf.writeUInt32LE(rate, 24);
  buf.writeUInt32LE(rate, 28);
  buf.writeUInt16LE(1, 32);
  buf.writeUInt16LE(8, 34);
  buf.write('data', 36);
  buf.writeUInt32LE(samples, 40);
  return buf;
}

/** A bus stop, drawn here, 800 by 500: a shelter with a bench, a sign on a pole, and a bus. */
const SCENE = `<!doctype html><html><body style="margin:0"><svg xmlns="http://www.w3.org/2000/svg" width="800" height="500" viewBox="0 0 800 500">
<defs><linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#bfe0ff"/><stop offset="1" stop-color="#eef7ff"/></linearGradient></defs>
<rect width="800" height="500" fill="url(#sky)"/><circle cx="700" cy="70" r="36" fill="#ffe08a"/>
<rect x="0" y="120" width="130" height="240" fill="#b8a99a"/><rect x="140" y="170" width="110" height="190" fill="#9fb4c7"/><rect x="620" y="110" width="180" height="250" fill="#c9b7a5"/>
<g fill="#e9f1f8"><rect x="20" y="140" width="22" height="30"/><rect x="60" y="140" width="22" height="30"/><rect x="20" y="190" width="22" height="30"/><rect x="60" y="190" width="22" height="30"/><rect x="650" y="140" width="24" height="32"/><rect x="700" y="140" width="24" height="32"/><rect x="750" y="140" width="24" height="32"/><rect x="650" y="200" width="24" height="32"/><rect x="700" y="200" width="24" height="32"/><rect x="750" y="200" width="24" height="32"/></g>
<rect x="0" y="350" width="800" height="30" fill="#d6d3cc"/><rect x="0" y="380" width="800" height="120" fill="#6b7077"/><g fill="#f4f1e8"><rect x="40" y="436" width="90" height="8"/><rect x="220" y="436" width="90" height="8"/><rect x="400" y="436" width="90" height="8"/><rect x="580" y="436" width="90" height="8"/></g>
<rect x="60" y="238" width="200" height="12" fill="#4a5560"/><rect x="66" y="250" width="6" height="100" fill="#4a5560"/><rect x="248" y="250" width="6" height="100" fill="#4a5560"/><rect x="74" y="252" width="172" height="80" fill="#cfe9f4" opacity="0.6"/>
<rect x="90" y="326" width="140" height="10" fill="#8a5a3a"/><rect x="100" y="336" width="6" height="14" fill="#5b3b25"/><rect x="214" y="336" width="6" height="14" fill="#5b3b25"/>
<rect x="302" y="232" width="6" height="118" fill="#4a5560"/><rect x="270" y="210" width="70" height="50" rx="6" fill="#1f5fae"/><text x="305" y="243" font-family="Arial,Helvetica,sans-serif" font-size="22" font-weight="bold" fill="#fff" text-anchor="middle">BUS</text>
<rect x="470" y="270" width="250" height="100" rx="14" fill="#f2b632"/><g fill="#dff1fb"><rect x="486" y="284" width="46" height="34" rx="4"/><rect x="540" y="284" width="46" height="34" rx="4"/><rect x="594" y="284" width="46" height="34" rx="4"/><rect x="648" y="284" width="56" height="34" rx="4"/></g>
<rect x="470" y="334" width="250" height="8" fill="#c8901a"/><circle cx="530" cy="372" r="18" fill="#2f3338"/><circle cx="530" cy="372" r="8" fill="#9aa1a8"/><circle cx="660" cy="372" r="18" fill="#2f3338"/><circle cx="660" cy="372" r="8" fill="#9aa1a8"/>
</svg></body></html>`;

export async function runShots2(ctx, api) {
  const { rpc, workDir, base, browser } = ctx;
  const dir = process.env.PRISMQDA_SHOTS_DIR;
  if (!dir) throw new Error('Set PRISMQDA_SHOTS_DIR to the folder the pictures go in.');
  fs.mkdirSync(dir, { recursive: true });

  const [W, H] = (process.env.PRISMQDA_SHOTS_SIZE ?? '1600x1000').split('x').map(Number);
  const SCALE = Number(process.env.PRISMQDA_SHOTS_SCALE ?? 1);
  const context = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: SCALE, locale: 'en-US', colorScheme: 'light', reducedMotion: 'reduce' });
  const page = await context.newPage();
  page.setDefaultTimeout(15000);
  const made = [];
  const skipped = [];
  // Before each picture: nothing on the page may name the person using the computer or a folder on it (the check's own folders are in the
  // user's temporary folder). The visible text and the values of the fields are read, and a hit is written in the log at the end.
  const leaks = [];
  const guard = async (name) => {
    const found = await page.evaluate(() => {
      const bits = [document.body.innerText];
      for (const el of document.querySelectorAll('input, textarea, select')) bits.push(el.value ?? '');
      const all = bits.join('\n');
      const m = all.match(/.{0,40}(Jason|AppData|[A-Za-z]:\\Users\\(?!you\\)|prismqda-check|@aol|@gmail).{0,40}/gi);
      return m ? m.slice(0, 3) : [];
    });
    if (found.length) {
      leaks.push(`${name}: ${found.join(' | ')}`);
      console.log(`LEAK in ${name}: ${found.join(' | ')}`);
    }
  };

  const view = async (width, height) => {
    await page.setViewportSize({ width, height });
    await page.waitForTimeout(400);
  };
  const shot = async (name, opts = {}) => {
    await page.waitForTimeout(opts.wait ?? 500);
    await guard(name);
    await (opts.locator ?? page).screenshot({ path: path.join(dir, `${name}.png`) });
    made.push(name);
  };
  /** A page that scrolls inside itself is drawn in full: the window is made as tall as the element, then put back. */
  const tall = async (name, locator, width = 1280, opts = {}) => {
    await view(width, 3200);
    await page.waitForTimeout(600);
    const box = await locator.boundingBox();
    await guard(name);
    if (opts.cropToContent && box) {
      const bottom = await locator.evaluate((el) => Math.max(...[...el.children].map((k) => k.getBoundingClientRect().bottom)));
      await page.screenshot({ path: path.join(dir, `${name}.png`), clip: { x: box.x, y: box.y, width: box.width, height: Math.min(box.height, bottom - box.y + 24) } });
      made.push(name);
    } else {
      await view(width, Math.min(3200, Math.ceil((box?.y ?? 0) + (box?.height ?? 800) + 80)));
      await shot(name, { locator, wait: 600 });
    }
    await view(W, H);
  };
  const attempt = async (name, fn) => {
    try {
      await fn();
    } catch (e) {
      skipped.push(`${name}: ${String(e.message ?? e).split('\n')[0]}`);
      console.log(`SKIPPED ${name}: ${String(e.message ?? e).split('\n')[0]}`);
      await view(W, H).catch(() => {});
      await page.keyboard.press('Escape').catch(() => {});
    }
  };
  const act = (fn, ...args) => page.evaluate(([f, a]) => window.__prismqda.actions[f](...a), [fn, args]);
  const go = async (workspace) => {
    await act('goTo', workspace);
    await page.waitForTimeout(500);
  };
  const reload = async () => {
    await page.goto(`${base}/?lang=en`);
    await page.waitForFunction(() => !!window.__prismqda && document.querySelector('#root')?.children.length > 0);
    await page.waitForTimeout(700);
  };
  const ribbon = async (tab, command) => {
    const back = page.getByRole('button', { name: 'Back to the coding workspace' });
    if ((await back.count()) > 0) await back.first().click();
    await page.getByRole('tab', { name: tab, exact: true }).click();
    await page.locator(`[data-command="${command}"]`).click();
    await page.waitForTimeout(600);
  };
  const teamTab = async (id) => {
    await go('team');
    await page.getByTestId(`team-tab-${id}`).click();
    await page.waitForTimeout(500);
  };

  // ================================================================ the made-up Urban Mobility Study, with a second coder
  const urbanPath = path.join(workDir, 'Urban Mobility Study.pqda');
  await rpc('project.close').catch(() => {});
  await reload();
  await rpc('sample.create', { id: 'urban-mobility', path: urbanPath });
  await rpc('project.open', { path: urbanPath, backup: false }).catch(() => {});
  const me = (await rpc('coder.list')).find((c) => c.role === 'human');
  const sam = await rpc('coder.add', { name: 'Sam Okafor', role: 'human' });
  const codeNamed = async (n) => (await rpc('code.list')).find((c) => c.name === n);
  const sources = await rpc('source.list');
  const richest = [...sources].sort((a, b) => b.codings - a.codings)[0];

  // two coders code the units of one interview, and differ on some, so the Disagreements page has units to settle
  await attempt('second coder', async () => {
    const pool = (await rpc('source.list')).filter((x) => !x.recording && !x.image && !x.pdf && /INT-00[23]/.test(x.name));
    const target = pool[0] ?? richest;
    const units = (await rpc('unit.list', { source_id: target.id })).filter((u) => !u.excluded && u.end - u.start > 60).slice(0, 6);
    const names = ['Reliability', 'Cost', 'Accessibility', 'Safety', 'Reliability', 'Cost'];
    const other = ['Reliability', 'Accessibility', 'Accessibility', 'Cost', 'Reliability', 'Cost'];
    for (let i = 0; i < units.length; i++) {
      const a = await codeNamed(names[i]);
      const b = await codeNamed(other[i]);
      await rpc('coding.apply', { source_id: target.id, start: units[i].start, end: units[i].end, code_id: a.id, coder_id: me.id, unit_id: units[i].id });
      await rpc('coding.apply', { source_id: target.id, start: units[i].start, end: units[i].end, code_id: b.id, coder_id: sam.id, unit_id: units[i].id });
    }
  });

  // ---- themes
  await attempt('themes', async () => {
    const find = async (n) => (await codeNamed(n))?.id;
    const makeTheme = async (name, definition, members) => {
      const t = await rpc('code.add', { name, theme: true, definition });
      for (const m of members) {
        const id = await find(m);
        if (id) await rpc('code.move', { id, parent_id: t.id });
      }
      return t;
    };
    await makeTheme('Getting around costs time and money', 'What people give up, in money and in waiting, to get where they are going.', ['Cost', 'Reliability']);
    await makeTheme('Feeling able and safe to travel', 'Whether the person can reach the service and feels safe using it.', ['Accessibility', 'Safety']);
    await makeTheme('Wanting a better system', 'What people ask for, and what they expect to do differently.', ['Desired Improvements', 'Future Use']);
    await reload();
    await view(W, H);
    await ribbon('View', 'go-themes');
    await page.getByTestId('themes-view').waitFor();
    await shot('themes-page', { wait: 900 });
    await page.getByTestId('theme-map').click();
    await page.waitForTimeout(1500);
    await tall('thematic-map', page.locator('#main-content'), 1280, { cropToContent: true });
  });

  // ---- working set
  await attempt('working set', async () => {
    await reload();
    await view(W, H);
    await ribbon('View', 'working-set');
    const dlg = page.getByTestId('ws-layer');
    await dlg.waitFor();
    await page.waitForFunction(() => !/^Looking/.test(document.querySelector('[data-testid=ws-final-summary]')?.textContent ?? 'Looking'), null, { timeout: 8000 });
    await shot('working-set-dialog', { wait: 600 });
    await page.getByTestId('ws-final-layer').click();
    await page.getByTestId('ws-apply').click();
    await page.getByTestId('working-set-bar').waitFor();
    await go('code');
    await act('openAt', richest.id, 0, 0);
    await shot('working-set-bar', { wait: 900 });
    await page.getByTestId('working-set-reset').click();
  });

  // ---- document portrait and codeline
  await attempt('portrait', async () => {
    await ribbon('Visual Tools', 'document-portrait');
    await page.waitForTimeout(900);
    await shot('document-portrait', { wait: 900 });
    await page.locator('#pt-view').selectOption('codeline');
    await page.waitForTimeout(600);
    // a document with four codes on it, so the codeline has something to say
    await page.locator('select').nth(1).selectOption({ label: 'INT-001 – Alex' }).catch(() => undefined);
    await page.waitForTimeout(900);
    await shot('document-codeline', { wait: 900 });
  });

  // ---- a picture, drawn here, coded by two coders
  let pictureId = null;
  await attempt('picture and areas', async () => {
    const scene = await context.newPage();
    await scene.setViewportSize({ width: 800, height: 500 });
    await scene.setContent(SCENE);
    const png = path.join(workDir, 'Bus stop on Elm Street.png');
    await scene.screenshot({ path: png });
    await scene.close();
    const done = await rpc('import.run', { path: png });
    pictureId = done.outcome.source_ids[0];
    const safety = (await codeNamed('Safety')).id;
    const reliability = (await codeNamed('Reliability')).id;
    const access = (await codeNamed('Accessibility')).id;
    const box = (x, y, w, h) => ({ shape: 'rectangle', x, y, w, h });
    const add = (who, code, shape, rationale = '') => rpc('region.add', { source_id: pictureId, ...shape, code_id: code, coder_id: who, rationale });
    await add(me.id, safety, box(0.075, 0.46, 0.25, 0.24), 'A shelter with a roof and a glass panel.');
    await add(sam.id, safety, box(0.08, 0.47, 0.24, 0.23));
    await add(me.id, reliability, box(0.5875, 0.54, 0.3125, 0.2), 'The bus is at the stop.');
    await add(sam.id, reliability, box(0.59, 0.545, 0.3, 0.2));
    await add(me.id, access, box(0.3375, 0.42, 0.0875, 0.1), 'The sign is how a rider finds the stop.');
    await add(sam.id, access, box(0.1125, 0.65, 0.175, 0.07), 'A place to sit while waiting.');
    await reload();
    await view(W, H);
    await go('code');
    await act('openAt', pictureId, 0, 0);
    await page.getByTestId('image-view').waitFor();
    await ribbon('View', 'pane-results');
    await go('code');
    await page.getByTestId('image-view').waitFor();
    await page.waitForTimeout(1500);
    await shot('picture-areas', { wait: 900 });
    await ribbon('View', 'pane-results');
    await go('code');
  });

  // ---- Team: disagreements (words and areas), packets
  await attempt('team disagreements', async () => {
    await reload();
    await view(W, H);
    await teamTab('disagreements');
    await page.getByTestId('region-disagreements').waitFor({ timeout: 15000 });
    // one area is settled, the other left open, so both states are on the page
    const first = page.getByTestId('region-disagreement').first();
    await first.getByTestId('region-reason').fill('The sign is how a rider finds the stop, so it belongs in the final layer.');
    await first.getByTestId('region-accept').click();
    await first.getByTestId('region-decision').waitFor();
    await page.waitForTimeout(800);
    await tall('team-disagreements', page.locator('#main-content'), 1280, { cropToContent: true });
    await tall('team-disagreements-areas', page.getByTestId('region-disagreements'), 1000);
  });
  await attempt('team packets', async () => {
    await view(W, H);
    await teamTab('packets');
    await page.getByTestId('packets').waitFor();
    await page.getByTestId('packet-coder').fill('Sam Okafor');
    await page.getByTestId('packet-instructions').fill('Code each interview with the codebook. Where a passage is unclear, write a memo and leave it uncoded. Send the file back when you have finished.');
    // the folder the packet would go in is not shown: the box shows a neutral place, and nothing has the focus
    await page.getByTestId('packet-path').evaluate((el) => { el.value = 'C:\\Projects\\Urban Mobility Study - packet for Sam Okafor.pqda'; });
    await page.getByRole('heading', { name: 'Make a packet for another coder' }).click();
    await tall('team-packets', page.locator('#main-content'), 1280, { cropToContent: true });
  });

  // ---- a PDF page with a coded rectangle
  await attempt('pdf page', async () => {
    const file = path.join(workDir, 'Garden notes.pdf');
    fs.copyFileSync(PDF_FIXTURE, file);
    const done = await rpc('import.run', { path: file });
    const id = done.outcome.source_ids[0];
    const joy = (await codeNamed('Convenience'))?.id ?? (await rpc('code.add', { name: 'Page joy' })).id;
    await rpc('region.add', { source_id: id, page: 1, shape: 'rectangle', x: 0.1, y: 0.12, w: 0.8, h: 0.1, code_id: joy, coder_id: me.id, rationale: 'The title of the notes.' });
    await reload();
    await view(W, 1100);
    await go('code');
    await act('openAt', id, 0, 0);
    await page.getByTestId('pdf-view-pages').click();
    await page.getByTestId('image-view').waitFor();
    await page.waitForFunction(() => {
      const i = document.querySelector('[data-testid=image-picture]');
      return !!i && i.complete && i.naturalWidth > 0;
    }, null, { timeout: 10000 });
    await page.waitForTimeout(900);
    await shot('pdf-coded-rectangle', { wait: 900 });
    await view(W, H);
  });

  // ---- a recording with coded stretches
  await attempt('recording', async () => {
    const wav = path.join(workDir, 'Walking interview.wav');
    fs.writeFileSync(wav, silentWav(20));
    const done = await rpc('import.run', { path: wav });
    const id = done.outcome.source_ids[0];
    const reliability = (await codeNamed('Reliability')).id;
    const safety = (await codeNamed('Safety')).id;
    await rpc('region.add', { source_id: id, shape: 'span', start_ms: 2000, end_ms: 6500, code_id: reliability, coder_id: me.id, rationale: 'Talks about missing a connection.' });
    await rpc('region.add', { source_id: id, shape: 'span', start_ms: 9000, end_ms: 14500, code_id: safety, coder_id: me.id });
    await rpc('region.add', { source_id: id, shape: 'span', start_ms: 15500, end_ms: 19000, code_id: reliability, coder_id: me.id });
    await reload();
    await view(W, H);
    await go('code');
    await act('openAt', id, 0, 0);
    await page.getByTestId('media-element').waitFor({ timeout: 10000 });
    await page.waitForFunction(() => {
      const a = document.querySelector('[data-testid=media-element]');
      return !!a && a.readyState >= 1 && a.duration > 10;
    }, null, { timeout: 10000 });
    await shot('recording-stretches', { wait: 1000 });
  });

  // ---- the gallery of areas under the codes
  await attempt('gallery', async () => {
    await view(W, H);
    await go('code');
    await act('openAt', pictureId, 0, 0);
    await page.getByLabel(/as a gallery/).first().click();
    await view(W, 1500);
    await page.waitForTimeout(1500);
    await shot('picture-gallery', { locator: page.getByRole('region', { name: /^Retrieved Segments/ }), wait: 900 });
    await view(W, H);
  });

  // ---- privacy: the names to replace, the offline lock, the ethics summary
  await attempt('privacy', async () => {
    await rpc('deid.add', { terms: ['Alex'], replacement: 'Participant A', kind: 'pseudonym' }).catch(() => undefined);
    await rpc('deid.add', { terms: ['Priya'], replacement: 'Participant B', kind: 'pseudonym' }).catch(() => undefined);
    await rpc('deid.add', { terms: ['Jordan'], replacement: 'Participant C', kind: 'pseudonym' }).catch(() => undefined);
    await rpc('privacy.set_offline_lock', { on: true });
    await reload();
    await view(W, H);
    await go('privacy');
    await page.waitForTimeout(900);
    await shot('privacy-page', { wait: 900 });
    await tall('privacy-offline-ethics', page.locator('#main-content'), 1280, { cropToContent: true });
    await rpc('privacy.set_offline_lock', { on: false });
  });

  // ---- merge: another coder's copy, with a value that differs
  await attempt('merge preview', async () => {
    const ourPath = (await rpc('project.info')).path;
    const copyPath = path.join(workDir, 'Sam copy.pqda');
    const mine = await rpc('case.add', { name: 'Alex' });
    await rpc('variable.define', { name: 'Rides per week', level: 'ratio' });
    await rpc('variable.set_values', { variable: 'Rides per week', changes: [{ case_id: mine.id, value: '5' }] });
    await rpc('variable.define', { name: 'Age', level: 'ratio' });
    await rpc('variable.set_values', { variable: 'Age', changes: [{ case_id: mine.id, value: '34' }] });
    await rpc('project.create', { path: copyPath, name: 'Sam copy' });
    await rpc('coder.add', { name: 'Sam Okafor', role: 'human', local_id: 'sam-copy' });
    await rpc('code.add', { name: 'Convenience of the stop', definition: 'How close and easy the stop is.' });
    const theirs = await rpc('case.add', { name: 'Alex' });
    await rpc('variable.define', { name: 'Rides per week', level: 'ratio' });
    await rpc('variable.set_values', { variable: 'Rides per week', changes: [{ case_id: theirs.id, value: '6' }] });
    await rpc('variable.define', { name: 'Age', level: 'ratio' });
    await rpc('variable.set_values', { variable: 'Age', changes: [{ case_id: theirs.id, value: '35' }] });
    const other = await rpc('case.add', { name: 'Priya' });
    await rpc('variable.set_values', { variable: 'Rides per week', changes: [{ case_id: other.id, value: '9' }] });
    await rpc('project.close', {});
    await rpc('project.open', { path: ourPath, backup: false });
    await reload();
    await view(W, H);
    await teamTab('merge');
    await page.getByTestId('merge-path').fill(copyPath);
    await page.getByTestId('merge-check').click();
    await page.getByTestId('merge-value-conflicts').waitFor({ timeout: 15000 });
    // the first difference is settled in favour of the copy, the second is left as it is
    await page.getByTestId('merge-value-conflicts').getByText("Use the copy's", { exact: true }).first().click();
    await page.waitForTimeout(500);
    // the folder of the copy is not shown: the box shows a neutral place instead
    await page.getByTestId('merge-path').evaluate((el) => { el.value = 'C:\\Projects\\Sam copy.pqda'; });
    await tall('team-merge-preview', page.locator('#main-content'), 1280, { cropToContent: true });
  });

  // ================================================================ mixed methods: 36 invented respondents, coded by two coders
  await attempt('mixed methods', async () => {
    const rnd = seeded(20261107);
    const truth = [];
    let csv = 'id,Age,Site,Rides per week,Satisfaction,answer\n';
    for (let i = 1; i <= 36; i++) {
      const site = SITES[Math.floor(rnd() * 3)];
      const age = 19 + Math.floor(rnd() * 52);
      const rides = 1 + Math.floor(rnd() * 12);
      const baseScore = site === 'Northside' ? 6.2 : site === 'Riverside' ? 5.0 : 4.1;
      const satisfaction = Math.max(1, Math.min(10, Math.round(baseScore + rides * 0.18 + (rnd() - 0.5) * 3)));
      const [comment, code] = COMMENTS[Math.floor(rnd() * COMMENTS.length)];
      truth.push(code);
      csv += `Resident ${String(i).padStart(2, '0')},${age},${site},${rides},${satisfaction},"${comment}"\n`;
    }
    const surveyPath = path.join(workDir, 'Neighborhood Transit Survey.pqda');
    await rpc('project.close').catch(() => {});
    await rpc('project.create', { path: surveyPath, name: 'Neighborhood Transit Survey' });
    fs.writeFileSync(path.join(workDir, 'survey-respondents.csv'), csv, 'utf8');
    await rpc('import.run', {
      path: path.join(workDir, 'survey-respondents.csv'),
      mapping: { id_column: 'id', text_columns: ['answer'], attribute_columns: [{ name: 'Age', level: 'ratio' }, { name: 'Site', level: 'nominal' }, { name: 'Rides per week', level: 'ratio' }, { name: 'Satisfaction', level: 'ratio' }] },
    });
    const codeIds = {};
    for (const c of CODES) codeIds[c] = (await rpc('code.add', { name: c })).id;
    const owner = (await rpc('coder.list')).find((c) => c.role === 'human');
    const docs = await rpc('source.list');
    const doc = docs.find((d) => d.units >= truth.length) ?? docs[0];
    const units = (await rpc('unit.list', { source_id: doc.id })).filter((u) => !u.excluded);
    for (let i = 0; i < Math.min(units.length, truth.length); i++) {
      await rpc('coding.apply', { source_id: doc.id, start: units[i].start, end: units[i].end, code_id: codeIds[truth[i]], coder_id: owner.id, unit_id: units[i].id });
    }
    await reload();
    await view(W, H);
    await go('mixed');
    await page.getByTestId('mixed-tab-joint').click();
    await page.getByTestId('joint-displays').waitFor();
    await page.locator('#jd-which').selectOption('ordered');
    await page.locator('#jd-sort').selectOption({ label: 'Satisfaction' });
    await page.getByTestId('jd-codes').click();
    const pop = page.getByRole('dialog', { name: 'Codes whose words are shown' });
    await pop.waitFor();
    await pop.getByText('Reliability', { exact: true }).click();
    await pop.getByText('Cost', { exact: true }).click();
    await page.keyboard.press('Escape');
    await page.getByTestId('jd-ordered-summary').waitFor();
    await page.waitForTimeout(900);
    await tall('mixed-methods-joint-display', page.locator('#main-content'), 1280, { cropToContent: true });
  });

  console.log(`\n${made.length} pictures saved in ${dir}:\n  ${made.join('\n  ')}`);
  if (skipped.length) console.log(`\n${skipped.length} skipped:\n  ${skipped.join('\n  ')}`);
  console.log(leaks.length ? `\n${leaks.length} PICTURE(S) NAME A PERSON OR A FOLDER:\n  ${leaks.join('\n  ')}` : '\nNo picture names a person or a folder (checked in the text and the fields of every page before it was drawn).');
  await context.close();
}
