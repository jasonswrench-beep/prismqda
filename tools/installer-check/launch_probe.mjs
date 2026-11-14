// Looks at the running PrismQDA window over WebView2's debugging port (port 9335) and says whether it shows what a double click should have caused.
// No operating-system mouse or keyboard is used: it only reads the page.
//
//   node launch_probe.mjs open "Alpha One"        the project is open (waits up to 40 s)
//   node launch_probe.mjs already "Beta Two"      that project is still open and the bar says it is already open
//   node launch_probe.mjs passphrase              the passphrase window is showing (then it is closed with Escape)
//   node launch_probe.mjs failed "Beta Two"       the bar says a file could not be opened and that project is still open
//
// Exit code 0 when what was asked for was seen, 1 when not.
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_DIR ?? 'playwright');
const [, , phase, wanted] = process.argv;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${process.env.PROBE_PORT ?? '9335'}`);
const context = browser.contexts()[0];
const page = context.pages().find((p) => !p.url().startsWith('devtools')) ?? context.pages()[0];

const openName = () =>
  page
    .evaluate(async () => {
      try {
        return (await window.__TAURI_INTERNALS__.invoke('call', { cmd: 'project.info', args: {} })).name;
      } catch {
        return null;
      }
    })
    .catch(() => null);
const status = () => page.evaluate(() => document.querySelector('[data-testid="status"]')?.textContent ?? '').catch(() => '');
const alertText = () => page.evaluate(() => document.querySelector('[data-testid="alert"]')?.textContent ?? '').catch(() => '');
const until = async (fn, ms) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) return null;
    await sleep(250);
  }
};

let ok = false;
let said = '';
if (phase === 'open') {
  ok = !!(await until(async () => (await openName()) === wanted, 40000));
  said = `the open project is "${await openName()}"`;
} else if (phase === 'already') {
  const seen = await until(async () => /already open/.test(await status()), 15000);
  ok = !!seen && (await openName()) === wanted;
  said = `the bar says "${await status()}", the open project is "${await openName()}"`;
} else if (phase === 'passphrase') {
  const shown = await until(() => page.locator('[data-testid="passphrase-dialog"]').isVisible().catch(() => false), 15000);
  ok = !!shown;
  said = `the passphrase window is ${shown ? 'showing' : 'not showing'}; the open project is "${await openName()}"`;
  if (shown) {
    await page.keyboard.press('Escape');
    await sleep(500);
  }
} else if (phase === 'failed') {
  const seen = await until(async () => /could not be opened/.test(await alertText()), 15000);
  ok = !!seen && (await openName()) === wanted;
  said = `the bar says "${await alertText()}", the open project is "${await openName()}"`;
} else {
  console.error('unknown phase');
  process.exit(2);
}
console.log(`${ok ? 'ok  ' : 'FAIL'}  ${phase}: ${said}`);
await browser.close().catch(() => undefined);
process.exit(ok ? 0 : 1);
