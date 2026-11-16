// Checks the case tables PrismQDA writes with the shared core's own checker, `checkPstat` from `@openstage/pstat`.
//
//   node tools/pstat-check/check.mjs --files <folder of .pstat files> --pstat <folder or file of @openstage/pstat> [--prism <prism.exe>]
//
// With --prism, PrismStatistics's own command-line program is also asked to open each file (`open_project`), the way the core's
// conformance check does, so the reader that matters is tried too.
//
// The files come from the export test (it writes them when PRISMQDA_PSTAT_OUT is set):
//
//   PRISMQDA_PSTAT_OUT=<folder> cargo test -p prismqda-export --test pstat the_files_can_be_written_out_for_the_cores_checker
//
// The package is the one on the core's release page (`npm install` of its tarball) or a checkout of `openstage-core/packages/pstat`.
// Exit code 0 when every file is sound and the package reads it as PrismStatistics would; 1 otherwise.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

function arg(name) {
  const at = process.argv.indexOf(`--${name}`);
  return at >= 0 ? process.argv[at + 1] : undefined;
}

const filesDir = arg('files');
const pkg = arg('pstat');
const prism = arg('prism');
if (!filesDir || !pkg) {
  console.error('usage: node check.mjs --files <folder of .pstat files> --pstat <folder or index.js of @openstage/pstat>');
  process.exit(2);
}
const entry = fs.statSync(pkg).isDirectory() ? path.join(pkg, fs.existsSync(path.join(pkg, 'src', 'index.js')) ? 'src/index.js' : 'index.js') : pkg;
const { checkPstat, readPstat, readPart, attributeNames } = await import(pathToFileURL(path.resolve(entry)).href);

let failed = 0;
const files = fs.readdirSync(filesDir).filter((f) => f.endsWith('.pstat')).sort();
if (files.length === 0) {
  console.error(`no .pstat file in ${filesDir}`);
  process.exit(2);
}
for (const file of files) {
  const bytes = new Uint8Array(fs.readFileSync(path.join(filesDir, file)));
  const problems = checkPstat(bytes);
  const project = readPstat(bytes);
  const part = readPart(bytes);
  const errors = [...problems.map((p) => `${p.rule} ${p.path}: ${p.message}`)];
  // The names must be what the core's rule makes from the attributes' names.
  const labels = part.attributes.map((a) => a.name);
  const made = attributeNames(labels, part.case_name_variable === null ? ['case_id'] : ['case_id', 'case_name']);
  const names = project.dataset.variables.filter((v) => part.attributes.some((a) => a.variable === v.id)).map((v) => v.name);
  if (JSON.stringify(made) !== JSON.stringify(names)) errors.push(`the names made by the core are ${JSON.stringify(made)}, the file has ${JSON.stringify(names)}`);
  if (prism) {
    const script = path.join(filesDir, file.replace(/\.pstat$/, '.pscript'));
    fs.writeFileSync(script, `open_project ${JSON.stringify({ path: path.join(filesDir, file) })}\n`);
    try {
      execFileSync(prism, ['run', script], { stdio: ['ignore', 'ignore', 'pipe'] });
    } catch (e) {
      errors.push(`PrismStatistics refused the file: ${String(e.stderr ?? e.message).trim().split('\n').pop()}`);
    }
  }
  console.log(`${errors.length === 0 ? 'ok  ' : 'FAIL'} ${file}: ${project.dataset.variables.length} variables, ${project.dataset.columns[0].values.length} cases, profile ${project.profile}`);
  for (const e of errors) console.log(`       ${e}`);
  if (errors.length) failed++;
}
process.exit(failed ? 1 : 0);
