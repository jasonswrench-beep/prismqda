import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MAX_RECENT, describeWhen, fileNameOf, forgetRecent, migrateOldRecent, normalizeRecent, rememberRecent, type RecentEntry } from './recent';

const e = (path: string, openedAt: number, uid: string | null = null, name = 'P'): RecentEntry => ({ path, name, uid, openedAt });

test('file names come from either kind of slash', () => {
  assert.equal(fileNameOf('C:\\Projects\\Study A.pqda'), 'Study A.pqda');
  assert.equal(fileNameOf('/home/me/study.pqda'), 'study.pqda');
  assert.equal(fileNameOf('study.pqda'), 'study.pqda');
});

test('stored entries are cleaned: bad ones go, repeats merge, newest first, and the list is capped', () => {
  const raw = [
    { path: 'C:\\a.pqda', name: 'A', uid: 'u1', openedAt: 10 },
    { path: 'c:/A.PQDA', name: 'A again', uid: null, openedAt: 20 },
    { path: 42 },
    null,
    { path: '', name: 'x' },
    { path: 'C:\\b.pqda', openedAt: 30 },
    { path: 'C:\\c.pqda', name: 'C', uid: 'u1', openedAt: 5 },
  ];
  const list = normalizeRecent(raw);
  assert.deepEqual(list.map((x) => x.path), ['C:\\b.pqda', 'C:\\a.pqda'], 'the second a (same path) and the c (same identity as a) are repeats');
  assert.equal(list[0].name, 'b', 'a missing name is made from the file name');
  assert.equal(normalizeRecent('nope').length, 0);
  const many = Array.from({ length: 20 }, (_, i) => ({ path: `C:\\p${i}.pqda`, openedAt: i }));
  assert.equal(normalizeRecent(many).length, MAX_RECENT);
});

test('the old list of paths becomes entries in the same order', () => {
  const list = migrateOldRecent(['C:\\one.pqda', 7, 'C:\\two.pqda'], 100_000);
  assert.deepEqual(list.map((x) => x.path), ['C:\\one.pqda', 'C:\\two.pqda']);
  assert.ok(list[0].openedAt > list[1].openedAt);
  assert.deepEqual(migrateOldRecent(undefined, 1), []);
});

test('opening a project again, by any route, leaves one entry at the top', () => {
  let list = [e('C:\\a.pqda', 1, 'u1'), e('C:\\b.pqda', 2, 'u2')];
  list = rememberRecent(list, e('c:/A.pqda', 9, 'u1', 'Renamed'));
  assert.deepEqual(list.map((x) => [x.path, x.name]), [['c:/A.pqda', 'Renamed'], ['C:\\b.pqda', 'P']]);
  list = rememberRecent(list, e('D:\\moved\\b.pqda', 10, 'u2'));
  assert.deepEqual(list.map((x) => x.path), ['D:\\moved\\b.pqda', 'c:/A.pqda'], 'a moved project is the same project, found by identity');
});

test('removing an entry changes only the list', () => {
  const list = [e('C:\\a.pqda', 1), e('C:\\b.pqda', 2)];
  assert.deepEqual(forgetRecent(list, 'c:/a.PQDA').map((x) => x.path), ['C:\\b.pqda']);
  assert.equal(forgetRecent(list, 'C:\\none.pqda').length, 2);
});

test('times are told as today, yesterday, or a date', () => {
  const now = new Date(2026, 10, 6, 15, 0).getTime();
  assert.match(describeWhen(new Date(2026, 10, 6, 14, 14).getTime(), now, 'en-US'), /^today, 2:14\s?PM$/);
  assert.match(describeWhen(new Date(2026, 10, 5, 18, 37).getTime(), now, 'en-US'), /^yesterday, 6:37\s?PM$/);
  assert.equal(describeWhen(new Date(2026, 0, 20, 9, 0).getTime(), now, 'en-US'), 'Jan 20, 2026');
  assert.equal(describeWhen(Number.NaN, now), '');
});
