// Holds PrismQDA's `.pstat` reader to the shared core's conformance files.
//
//   node tools/pstat-check/conformance.mjs --core <checkout of openstage-core> [--work <folder>]
//
// The core makes 33 files (13 sound, 20 that each break one rule of PSTAT-1) and says for each whether a reader must open or refuse it.
// This script has the core make them, runs `crates/prismqda-import/tests/pstat_conformance.rs` over them, and compares: a sound file must
// be opened, and a file that breaks a rule must be refused. Exit code 0 when PrismQDA does what the specification says with all 33.

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '..', '..');

function arg(name) {
  const at = process.argv.indexOf(`--${name}`);
  return at >= 0 ? process.argv[at + 1] : undefined;
}
const core = arg('core');
if (!core) {
  console.error('usage: node conformance.mjs --core <checkout of openstage-core> [--work <folder>]');
  process.exit(2);
}
const work = arg('work') ? path.resolve(arg('work')) : fs.mkdtempSync(path.join(os.tmpdir(), 'pstat-conformance-'));
fs.mkdirSync(work, { recursive: true });

// 1. The core makes the files and says what each must do (the package's own reader is checked there too).
const made = spawnSync('node', [path.join(core, 'specs', 'pstat', 'conformance', 'check_reader.mjs'), '--work', work], { encoding: 'utf8' });
const expected = [];
for (const line of made.stdout.split('\n')) {
  const m = /^(?:ok  |WRONG)\s+(.+?): must (open|refuse \((\w+)\))/.exec(line);
  if (m) expected.push({ name: m[1], must: m[2] === 'open' ? 'open' : 'refuse', rule: m[3] ?? '' });
}
if (expected.length === 0) {
  console.error('the core made no files:\n' + made.stdout + made.stderr);
  process.exit(2);
}

// 2. PrismQDA's reader goes over them.
const run = spawnSync('cargo', ['test', '-q', '-p', 'prismqda-import', '--test', 'pstat_conformance'], { cwd: repo, encoding: 'utf8', env: { ...process.env, PRISMQDA_PSTAT_FILES: work } });
if (run.status !== 0) {
  console.error(run.stdout + run.stderr);
  process.exit(2);
}
const answers = fs.readFileSync(path.join(work, 'results.txt'), 'utf8').split('\n').filter(Boolean);

// 3. Compare.
let wrong = 0;
expected.forEach((e, i) => {
  const number = String(i + 1).padStart(2, '0');
  const got = answers.find((a) => a.startsWith(`${number} `)) ?? `${number} (no answer)`;
  const opened = got.startsWith(`${number} open`);
  const ok = e.must === 'open' ? opened : !opened && !got.includes('(no answer)');
  if (!ok) wrong++;
  console.log(`${ok ? 'ok   ' : 'WRONG'}  ${e.name}: must ${e.must === 'open' ? 'open' : `refuse (${e.rule})`}; PrismQDA: ${got.slice(number.length + 1)}`);
});
console.log(`\n${expected.length - wrong} of ${expected.length} as the specification says.`);
process.exit(wrong ? 1 : 0);
