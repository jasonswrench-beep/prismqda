// The browser check: builds the engine bridge, serves the built interface, and drives the real
// screens in a headless Chromium through the whole coding flow, with axe on every screen.
//
// Run it from a shell pinned to the efficiency cores (see README.md), after `npm run build`:
//   $env:PLAYWRIGHT_DIR = '<folder>\playwright-tools\node_modules\playwright'
//   node tools/browser-check.mjs
//
// It starts two processes (the engine bridge and the browser) and stops exactly those two at the end.
// It never opens a visible window and never plays sound. All test text is made up.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import AxeBuilder from '@axe-core/playwright';
import { createHarness } from './lib/harness.mjs';
import { buildBridge, freePort, rpc, startBridge, startStatic, stopChild } from './lib/servers.mjs';
import { writeFiles } from './lib/testdata.mjs';
import { runScenarios } from './scenarios.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const uiRoot = path.resolve(here, '..');
const repoRoot = path.resolve(uiRoot, '..', '..');
const dist = path.join(uiRoot, 'dist');
const shots = path.join(uiRoot, 'test-results');

const playwrightDir = process.env.PLAYWRIGHT_DIR;
if (!playwrightDir) {
  console.error("Set PLAYWRIGHT_DIR to the Playwright folder, for example <folder>\\playwright-tools\\node_modules\\playwright");
  process.exit(2);
}
if (!fs.existsSync(path.join(dist, 'index.html'))) {
  console.error('There is no dist folder. Run `npm run build` first.');
  process.exit(2);
}

const log = (m) => console.log(`-- ${m}`);
const h = createHarness();
const processes = [];
let browser = null;
let staticServer = null;
let exitCode = 0;
const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'prismqda-check-'));

try {
  fs.mkdirSync(shots, { recursive: true });
  const exe = process.env.PRISMQDA_EXE && fs.existsSync(process.env.PRISMQDA_EXE) ? process.env.PRISMQDA_EXE : buildBridge(repoRoot, log);
  const bridgePort = await freePort();
  const bridge = await startBridge(exe, bridgePort);
  processes.push(bridge.child);
  log(`bridge started (pid ${bridge.pid}) on port ${bridgePort}`);
  const web = await startStatic(dist, bridgePort);
  staticServer = web.server;
  const base = `http://127.0.0.1:${web.port}`;
  log(`interface served at ${base}`);

  const { chromium } = await import(pathToFileURL(path.join(playwrightDir, 'index.mjs')).href);
  // Muted, so that nothing a page plays can reach the computer's speakers (the recordings the check makes are digital silence as well).
  browser = await chromium.launch({ headless: true, args: ['--mute-audio'] });
  log(`browser started (${browser.version()})`);

  const files = writeFiles(workDir);
  const ctx = {
    h,
    browser,
    base,
    bridgePort,
    rpc: (cmd, args) => rpc(bridgePort, cmd, args),
    files,
    workDir,
    shots,
    AxeBuilder,
    log,
  };
  await runScenarios(ctx);
} catch (e) {
  console.error('The browser check could not finish:', e);
  exitCode = 1;
} finally {
  try {
    await browser?.close();
  } catch {
    // The browser may already be gone.
  }
  await new Promise((r) => (staticServer ? staticServer.close(r) : r()));
  for (const p of processes) await stopChild(p);
  try {
    fs.rmSync(workDir, { recursive: true, force: true });
  } catch {
    // Leftover temp files are harmless.
  }
}

const summary = h.summary();
if (summary.failed > 0) exitCode = 1;
process.exit(exitCode);
