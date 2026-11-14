// Process helpers for the browser check: build and start the engine bridge, and serve the built
// interface with /rpc forwarded to the bridge. Every process started here is tracked by its exact
// PID and stopped by that PID only.

import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import path from 'node:path';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
};

export function freePort() {
  return new Promise((resolve, reject) => {
    const s = net.createServer();
    s.unref();
    s.on('error', reject);
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
}

export function exeName(base) {
  return process.platform === 'win32' ? `${base}.exe` : base;
}

/** Builds the command-line program with the development bridge. */
export function buildBridge(repoRoot, log) {
  log('building the engine bridge: cargo build -p prismqda-cli --features dev-server');
  const r = spawnSync('cargo', ['build', '-p', 'prismqda-cli', '--features', 'dev-server'], {
    cwd: repoRoot,
    stdio: ['ignore', 'inherit', 'inherit'],
    shell: false,
  });
  if (r.status !== 0) throw new Error(`cargo build failed (exit ${r.status})`);
  const exe = path.join(repoRoot, 'target', 'debug', exeName('prismqda'));
  if (!fs.existsSync(exe)) throw new Error(`the built program is missing: ${exe}`);
  return exe;
}

/** Starts `prismqda serve`, waits until it listens, and returns the child and its port. */
export async function startBridge(exe, port, actor = 'Ada Lovelace') {
  const child = spawn(exe, ['--actor', actor, 'serve', '--port', String(port)], {
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let out = '';
  child.stdout.on('data', (d) => (out += d));
  child.stderr.on('data', (d) => (out += d));
  const started = Date.now();
  for (;;) {
    if (out.includes('listening')) break;
    if (child.exitCode !== null) throw new Error(`the bridge exited early:\n${out}`);
    if (Date.now() - started > 20_000) throw new Error(`the bridge did not start:\n${out}`);
    await new Promise((r) => setTimeout(r, 50));
  }
  return { child, pid: child.pid, port };
}

/** Calls the bridge directly, for setting up test data without the interface. */
export async function rpc(port, cmd, args = {}) {
  const res = await fetch(`http://127.0.0.1:${port}/rpc`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ cmd, args }),
  });
  const body = await res.json();
  if (!body.ok) throw new Error(`${cmd}: ${body.error.code}: ${body.error.message}`);
  return body.result;
}

/** Serves `dist` and forwards POST /rpc to the bridge. */
/**
 * The content security policy the desktop window enforces (from app/src-tauri/tauri.conf.json), so the browser
 * check fails on anything the window would refuse. Only `connect-src` differs: here the page talks to its own
 * server instead of Tauri's IPC.
 */
function windowPolicy(dist) {
  try {
    const conf = JSON.parse(fs.readFileSync(path.join(dist, '..', '..', 'src-tauri', 'tauri.conf.json'), 'utf8'));
    const csp = conf?.app?.security?.csp;
    if (typeof csp !== 'string') return null;
    return csp.replace(/connect-src [^;]*/, "connect-src 'self'");
  } catch {
    return null;
  }
}

export function startStatic(dist, bridgePort) {
  const policy = windowPolicy(dist);
  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? '/', 'http://localhost');
    if (req.method === 'POST' && url.pathname === '/rpc') {
      const chunks = [];
      req.on('data', (c) => chunks.push(c));
      req.on('end', () => {
        const body = Buffer.concat(chunks);
        // A call that takes long is named in the log, so a stuck engine can be traced to what it was asked.
        let what = '?';
        try {
          const sent = JSON.parse(body.toString('utf8'));
          what = `${sent.cmd} ${JSON.stringify(sent.args ?? {}).slice(0, 100)}`;
        } catch {
          // Not JSON: leave the name as it is.
        }
        const started = Date.now();
        const warn = setTimeout(() => console.log(`       (slow engine call still running after 6 s: ${what})`), 6000);
        res.on('close', () => {
          clearTimeout(warn);
          const took = Date.now() - started;
          if (took > 1500) console.log(`       (slow engine call: ${what} took ${took} ms)`);
        });
        const proxy = http.request(
          { host: '127.0.0.1', port: bridgePort, path: '/rpc', method: 'POST', agent: false, headers: { 'Content-Type': 'application/json', 'Content-Length': body.length, Connection: 'close' } },
          (back) => {
            res.writeHead(back.statusCode ?? 502, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
            back.pipe(res);
          },
        );
        // If the page goes away (a reload) before its answer has been sent on, the answer is thrown away. Left unread, a large
        // one fills the connection and leaves the one-at-a-time engine bridge waiting to write it, for good. This is
        // registered now, not when the answer arrives, because the page may be gone before it does.
        res.on('close', () => {
          if (!res.writableFinished) proxy.destroy();
        });
        proxy.on('error', (e) => {
          if (res.headersSent || res.destroyed) {
            res.destroy();
            return;
          }
          res.writeHead(502, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ ok: false, error: { code: 'network', message: String(e.message) } }));
        });
        proxy.end(body);
      });
      return;
    }
    // A player asking for a recording: passed to the bridge with its Range header, and the answer (and its headers) passed back.
    if (req.method === 'GET' && (url.pathname.startsWith('/media/') || url.pathname.startsWith('/image/') || url.pathname.startsWith('/pdf/'))) {
      const headers = {};
      if (req.headers.range) headers.Range = req.headers.range;
      const proxy = http.request({ host: '127.0.0.1', port: bridgePort, path: url.pathname, method: 'GET', agent: false, headers: { ...headers, Connection: 'close' } }, (back) => {
        const pass = {};
        for (const name of ['content-type', 'content-range', 'content-length', 'accept-ranges', 'cache-control']) if (back.headers[name]) pass[name] = back.headers[name];
        res.writeHead(back.statusCode ?? 502, pass);
        back.pipe(res);
      });
      res.on('close', () => {
        if (!res.writableFinished) proxy.destroy();
      });
      proxy.on('error', () => {
        if (!res.headersSent) res.writeHead(502);
        res.end();
      });
      proxy.end();
      return;
    }
    if (req.method !== 'GET' && req.method !== 'HEAD') {
      res.writeHead(405);
      res.end();
      return;
    }
    let rel = decodeURIComponent(url.pathname);
    if (rel.endsWith('/')) rel += 'index.html';
    let file = path.normalize(path.join(dist, rel));
    if (!file.startsWith(dist)) {
      res.writeHead(403);
      res.end();
      return;
    }
    if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) file = path.join(dist, 'index.html');
    res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] ?? 'application/octet-stream', 'Cache-Control': 'no-store', ...(policy ? { 'Content-Security-Policy': policy } : {}) });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => resolve({ server, port: server.address().port }));
  });
}

/** Stops a child process we started, by its handle (so only that exact PID). */
export async function stopChild(child) {
  if (!child || child.exitCode !== null) return;
  await new Promise((resolve) => {
    child.once('exit', resolve);
    child.kill();
    setTimeout(resolve, 5000);
  });
}
