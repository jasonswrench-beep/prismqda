// Makes the project the recording check opens: a project with one recording that stands for its document (eleven minutes of mostly silence
// with one loud second at ten minutes, larger than the 4 MiB a window is sent at once) and one coded stretch on that loud second.
//
//   node make_recording_project.mjs <folder>
//
// It starts the development bridge (target\debug\prismqda.exe, built with `cargo build -p prismqda-cli --features dev-server`), makes the
// project through it, and stops exactly that one process. Everything is made up. Nothing here plays a sound.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const EXE = path.resolve(here, '..', '..', 'target', 'debug', 'prismqda.exe');
if (!process.argv[2]) {
  console.error('usage: node make_recording_project.mjs <folder>');
  process.exit(2);
}
const out = path.resolve(process.argv[2]);
fs.mkdirSync(out, { recursive: true });

/** 8-bit mono WAV: silence, with a loud second starting at `at` seconds. */
function toneWav(seconds, rate, at) {
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
  for (let i = at * rate; i < (at + 1) * rate; i++) buf[44 + i] = (i >> 1) % 2 === 0 ? 0xe0 : 0x20;
  return buf;
}

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
  const wav = path.join(out, 'real-big.wav');
  fs.writeFileSync(wav, toneWav(650, 8000, 600));
  const project = path.join(out, 'recording.pqda');
  fs.rmSync(project, { force: true });
  await rpc('project.create', { path: project, name: 'Recording check', coding_mode: 'free_span' });
  const done = await rpc('import.run', { path: wav });
  const code = await rpc('code.add', { name: 'Loud' });
  await rpc('region.add', { source_id: done.outcome.source_ids[0], shape: 'span', start_ms: 600000, end_ms: 601000, code_id: code.id });
  await rpc('project.close');
  console.log(`made ${project} (recording ${fs.statSync(wav).size} bytes)`);
} finally {
  child.kill();
}
