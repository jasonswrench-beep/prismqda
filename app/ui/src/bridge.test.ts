import { test, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ApiError, call, inTauri, setTransportForTests, toApiError } from './bridge';

const g = globalThis as unknown as Record<string, unknown>;

afterEach(() => {
  setTransportForTests(null);
  delete g.window;
  delete g.fetch;
});

function mockFetch(handler: (url: string, init: RequestInit) => { status?: number; body: unknown } | 'network-error' | 'not-json') {
  g.fetch = async (url: string, init: RequestInit) => {
    const r = handler(url, init);
    if (r === 'network-error') throw new TypeError('Failed to fetch');
    if (r === 'not-json') return { status: 502, json: async () => { throw new Error('bad json'); } };
    return { status: r.status ?? 200, json: async () => r.body };
  };
}

test('in a browser the command is posted to /rpc and the result is returned', async () => {
  let seen: { url: string; body: unknown } | null = null;
  mockFetch((url, init) => {
    seen = { url, body: JSON.parse(String(init.body)) };
    return { body: { ok: true, result: { name: 'PrismQDA', version: '0.1.0' } } };
  });
  const r = await call('ping');
  assert.deepEqual(r, { name: 'PrismQDA', version: '0.1.0' });
  assert.deepEqual(seen, { url: '/rpc', body: { cmd: 'ping', args: {} } });
});

test('arguments are passed through unchanged', async () => {
  let body: unknown = null;
  mockFetch((_u, init) => {
    body = JSON.parse(String(init.body));
    return { body: { ok: true, result: { id: 1, quote: 'felt welcome' } } };
  });
  await call('coding.apply', { source_id: 1, start: 2, end: 14, code_id: 3 });
  assert.deepEqual(body, { cmd: 'coding.apply', args: { source_id: 1, start: 2, end: 14, code_id: 3 } });
});

test('an error from the engine becomes an ApiError with its code and message', async () => {
  mockFetch(() => ({ body: { ok: false, error: { code: 'invalid', message: 'the span 2..999 does not fit source 1' } } }));
  await assert.rejects(call('coding.apply', { source_id: 1, start: 2, end: 999, code_id: 3 }), (e: unknown) => {
    assert.ok(e instanceof ApiError);
    assert.equal(e.code, 'invalid');
    assert.match(e.message, /does not fit/);
    return true;
  });
});

test('a missing engine and an unreadable answer give plain messages', async () => {
  mockFetch(() => 'network-error');
  await assert.rejects(call('ping'), (e: unknown) => e instanceof ApiError && e.code === 'network' && /cannot reach/.test(e.message));
  mockFetch(() => 'not-json');
  await assert.rejects(call('ping'), (e: unknown) => e instanceof ApiError && e.code === 'network' && /status 502/.test(e.message));
});

test('inside Tauri the command goes through invoke("call", {cmd, args})', async () => {
  const calls: unknown[][] = [];
  g.window = {
    __TAURI_INTERNALS__: {
      invoke: async (name: string, args: unknown) => {
        calls.push([name, args]);
        return { name: 'PrismQDA', version: 'tauri' };
      },
    },
  };
  assert.equal(inTauri(), true);
  const r = await call('ping');
  assert.equal(r.version, 'tauri');
  assert.deepEqual(calls[0], ['call', { cmd: 'ping', args: {} }]);
});

test('inside Tauri a rejected command becomes an ApiError too', async () => {
  g.window = {
    __TAURI_INTERNALS__: {
      invoke: async () => {
        throw { code: 'no_project', message: 'no project is open; create or open one first' };
      },
    },
  };
  await assert.rejects(call('source.list'), (e: unknown) => e instanceof ApiError && e.code === 'no_project');
  g.window = { __TAURI_INTERNALS__: { invoke: async () => { throw 'plain text failure'; } } };
  await assert.rejects(call('source.list'), (e: unknown) => e instanceof ApiError && e.code === 'internal' && e.message === 'plain text failure');
});

test('browser mode never needs the Tauri package', () => {
  g.window = {};
  assert.equal(inTauri(), false);
});

test('toApiError copes with anything', () => {
  assert.equal(toApiError(new Error('boom')).message, 'boom');
  assert.equal(toApiError(null).code, 'internal');
  assert.equal(toApiError({ code: 'x', message: 'y' }).code, 'x');
});
