// Makes the files the double-click check opens: three real projects (one of them encrypted) and a text file with a .pqda ending.
//
//   node make_projects.mjs <folder>
//
// It starts the development bridge (target\debug\prismqda.exe, built with `cargo build -p prismqda-cli --features dev-server`),
// makes the projects through it, and stops exactly that one process. Everything is made up.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const EXE = path.resolve(here, '..', '..', 'target', 'debug', 'prismqda.exe');
const out = path.resolve(process.argv[2] ?? '');
if (!process.argv[2]) {
  console.error('usage: node make_projects.mjs <folder>');
  process.exit(2);
}
fs.mkdirSync(out, { recursive: true });

const port = await new Promise((resolve) => {
  const s = net.createServer();
  s.listen(0, '127.0.0.1', () => {
    const p = s.address().port;
    s.close(() => resolve(p));
  });
});
const child = spawn(EXE, ['--actor', 'Tester', 'serve', '--port', String(port)], { stdio: 'ignore' });
try {
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/health`);
      if (r.ok) break;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  const rpc = async (cmd, args = {}) => {
    const r = await fetch(`http://127.0.0.1:${port}/rpc`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ cmd, args }) });
    const body = await r.json();
    if (!body.ok) throw new Error(`${cmd}: ${JSON.stringify(body.error)}`);
    return body.result;
  };
  const make = async (file, name, extra = {}) => {
    const p = path.join(out, file);
    fs.rmSync(p, { force: true });
    await rpc('project.create', { path: p, name, coding_mode: 'free_span', ...extra });
    await rpc('project.close');
    console.log(`made ${p}`);
  };
  await make('alpha.pqda', 'Alpha One');
  await make('beta.pqda', 'Beta Two');
  await make('secret.pqda', 'Secret Three', { passphrase: 'correct horse battery' });
  fs.writeFileSync(path.join(out, 'broken.pqda'), 'This is only a text file with a project ending.\n');
  console.log(`made ${path.join(out, 'broken.pqda')} (not a project)`);
} finally {
  child.kill();
}
