// Looks at the running PrismQDA window over WebView2's debugging port (9336) and tries the recording tools in it: the waveform of a recording
// larger than 4 MiB (the window reads it through its own address, `http://media.localhost`, under the content security policy), and a coded
// stretch saved as a WAV file. It reads the page and clicks buttons of the page only (no operating-system mouse or keyboard), and it presses
// nothing that plays a sound: the waveform and the saved clip are decoded, never played.
//
//   node recording_probe.mjs <folder to save the clip in>
//
// Exit code 0 when everything was seen, 1 when not.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_DIR ?? 'playwright');
const saveDir = path.resolve(process.argv[2] ?? '');
fs.mkdirSync(saveDir, { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (ok, what) => {
  if (!ok) failed++;
  console.log(`${ok ? 'ok  ' : 'FAIL'}  ${what}`);
};

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${process.env.PROBE_PORT ?? '9336'}`);
const context = browser.contexts()[0];
const page = context.pages().find((p) => !p.url().startsWith('devtools')) ?? context.pages()[0];
const problems = [];
page.on('console', (m) => {
  if (m.type() === 'error') problems.push(m.text());
});
page.on('pageerror', (e) => problems.push(String(e)));

try {
  const until = async (fn, ms) => {
    const end = Date.now() + ms;
    for (;;) {
      const v = await fn().catch(() => null);
      if (v) return v;
      if (Date.now() > end) return null;
      await sleep(300);
    }
  };
  const open = await until(() => page.evaluate(async () => (await window.__TAURI_INTERNALS__.invoke('call', { cmd: 'project.info', args: {} })).name), 40000);
  check(open === 'Recording check', `the project is open: ${open}`);

  // The recording's document, opened from the Documents list.
  const row = page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: /real-big/ }).first();
  await row.waitFor({ timeout: 20000 });
  await row.click();
  await page.getByTestId('media-player').waitFor({ timeout: 20000 });
  check(true, 'the document of the recording is open, with its player');

  // The waveform: asked for, drawn from the whole recording.
  await page.getByTestId('waveform-show').click();
  const drawn = await page.getByTestId('waveform').waitFor({ timeout: 60000 }).then(() => true, () => false);
  check(drawn, 'the waveform is drawn in the real window');
  if (drawn) {
    const total = Number(await page.getByTestId('waveform').getAttribute('aria-valuemax'));
    check(Math.abs(total - 650000) < 1000, `it spans the whole recording (${total} ms of 650000)`);
    const note = (await page.getByTestId('waveform-note').innerText()).replace(/\s+/g, ' ');
    check(/The loudest part is at (9:59|10:00)\./.test(note), `it finds the loud second: ${note}`);
  } else {
    const shown = await page.getByTestId('waveform-box').innerText().catch(() => '');
    console.log(`      the box says: ${shown.replace(/\s+/g, ' ')}`);
  }

  // The coded stretch saved as its own sound file.
  await page.getByTestId('clip-save-path').fill(saveDir + path.sep);
  await page.getByTestId('clip-row').first().getByTestId('clip-save').click();
  const saved = await page.getByTestId('clip-saved').waitFor({ timeout: 90000 }).then(() => true, () => false);
  check(saved, 'a stretch is saved as a file in the real window');
  const file = path.join(saveDir, 'real-big 10m00s-10m01s Loud.wav');
  check(fs.existsSync(file), `the file is there: ${fs.readdirSync(saveDir).join(', ')}`);
  if (fs.existsSync(file)) {
    const b = fs.readFileSync(file);
    const frames = b.readUInt32LE(40) / (b.readUInt16LE(22) * (b.readUInt16LE(34) / 8));
    const seconds = frames / b.readUInt32LE(24);
    let peak = 0;
    for (let at = 44; at + 1 < b.length; at += 2) peak = Math.max(peak, Math.abs(b.readInt16LE(at)));
    check(b.toString('latin1', 0, 4) === 'RIFF' && Math.abs(seconds - 1) < 0.02 && peak / 32768 > 0.5, `it is one loud second of WAV (${seconds.toFixed(3)} s, peak ${(peak / 32768).toFixed(2)}, ${b.readUInt32LE(24)} Hz)`);
  }
  const csp = problems.filter((p) => /Content Security Policy|Refused to|CORS|blocked/i.test(p));
  check(csp.length === 0, `no content security policy or cross-origin error in the console${csp.length ? ': ' + csp[0].slice(0, 200) : ''}`);
} catch (e) {
  failed++;
  console.log(`FAIL  the probe stopped: ${e.message.split('\n')[0]}`);
}
await browser.close().catch(() => undefined);
process.exit(failed ? 1 : 0);
