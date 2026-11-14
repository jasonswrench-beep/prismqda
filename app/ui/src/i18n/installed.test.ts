import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, it } from 'node:test';
import { addInstalled, freeId, installedCatalog, installedInfo, removeInstalled } from './installed';

/** A stand-in for the browser's storage, with a limit on how much it holds. */
class FakeStorage {
  private data = new Map<string, string>();
  constructor(private limit = Infinity) {}
  getItem(k: string) {
    return this.data.get(k) ?? null;
  }
  setItem(k: string, v: string) {
    let used = 0;
    for (const [key, value] of this.data) if (key !== k) used += value.length;
    if (used + v.length > this.limit) throw new Error('QuotaExceededError');
    this.data.set(k, v);
  }
  removeItem(k: string) {
    this.data.delete(k);
  }
  keys() {
    return [...this.data.keys()];
  }
}

const g = globalThis as unknown as { localStorage?: unknown };
let fake: FakeStorage;
const entry = (code: string, label = code) => ({ code, label, credit: 'A. Roy', madeBy: 'person' as const, done: 2, total: 10 });

beforeEach(() => {
  fake = new FakeStorage();
  g.localStorage = fake;
});
afterEach(() => {
  delete g.localStorage;
});

describe('translations kept on this computer', () => {
  it('are added with a name of their own and read back', () => {
    const r = addInstalled(entry('fr', 'Français'), { messages: { Save: 'Enregistrer' }, contexts: { menu: { Open: 'Ouvrir' } } });
    assert.ok(r.ok);
    assert.equal(r.ok && r.info.id, 'fr');
    assert.equal(installedInfo().length, 1);
    assert.deepEqual(installedCatalog('fr'), { messages: { Save: 'Enregistrer' }, contexts: { menu: { Open: 'Ouvrir' } } });
    assert.equal(installedInfo()[0]?.dir, 'ltr');
    assert.equal(installedInfo()[0]?.label, 'Français');
  });
  it('know a language that reads right to left', () => {
    addInstalled(entry('ar'), { messages: {} });
    assert.equal(installedInfo()[0]?.dir, 'rtl');
  });
  it('can be kept beside each other or replace one another', () => {
    addInstalled(entry('fr', 'one'), { messages: { A: '1' } });
    const second = addInstalled(entry('fr', 'two'), { messages: { A: '2' } });
    assert.equal(second.ok && second.info.id, 'fr~2', 'a second translation of a language gets its own name');
    assert.equal(installedInfo().length, 2);
    const third = addInstalled(entry('fr', 'three'), { messages: { A: '3' } }, 'fr');
    assert.equal(third.ok && third.info.id, 'fr', 'replacing keeps the name');
    assert.equal(installedInfo().length, 2);
    assert.equal(installedCatalog('fr')?.messages.A, '3');
    assert.equal(installedInfo().find((p) => p.id === 'fr')?.label, 'three');
  });
  it('are removed from the list and from storage', () => {
    addInstalled(entry('fr'), { messages: {} });
    removeInstalled('fr');
    assert.deepEqual(installedInfo(), []);
    assert.equal(installedCatalog('fr'), null);
    assert.deepEqual(fake.keys().filter((k) => k.startsWith('prismqda.language.')), []);
  });
  it('are not added when there is no room, and nothing is left half done', () => {
    fake = new FakeStorage(500);
    g.localStorage = fake;
    const r = addInstalled(entry('fr'), { messages: { Save: 'x'.repeat(2000) } });
    assert.equal(r.ok, false);
    assert.equal(!r.ok && /not enough room/.test(r.reason), true);
    assert.deepEqual(installedInfo(), []);
    assert.deepEqual(fake.keys(), [], 'no part of it is kept');
  });
  it('leave out records that are damaged, and give nothing when storage is missing', () => {
    fake.setItem('prismqda.languages', JSON.stringify([{ id: '../x', code: 'fr' }, { id: 'ok', code: 'de', label: 'D' }, 'nonsense', { id: 'bad', code: 'not a code' }]));
    assert.deepEqual(installedInfo().map((p) => p.id), ['ok']);
    fake.setItem('prismqda.language.ok', '{not json');
    assert.equal(installedCatalog('ok'), null);
    delete g.localStorage;
    assert.deepEqual(installedInfo(), []);
    const r = addInstalled(entry('fr'), { messages: {} });
    assert.equal(r.ok, false);
  });
  it('choose the next free name', () => {
    assert.equal(freeId('fr', []), 'fr');
    assert.equal(freeId('fr', ['fr']), 'fr~2');
    assert.equal(freeId('fr', ['fr', 'fr~2']), 'fr~3');
  });
});
