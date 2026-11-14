import assert from 'node:assert/strict';
import { test } from 'node:test';
import { alreadyQueued, fileNameOf, importBlocker, outcomeLine, tally, toImport, toRetry, type ItemStatus, type QueueItem } from './importQueue';

let n = 0;
const item = (status: ItemStatus, extra: Partial<QueueItem> = {}): QueueItem => ({ id: ++n, path: `C:\\data\\file-${n}.txt`, status, duplicate: 'skip', ...extra });

test('a file name is the last part of the path, whichever slash is used', () => {
  assert.equal(fileNameOf('C:\\Data\\Interviews\\INT-001.docx'), 'INT-001.docx');
  assert.equal(fileNameOf('/home/me/notes.md'), 'notes.md');
  assert.equal(fileNameOf('C:\\Data\\Folder\\'), 'Folder');
});

test('Import takes the ready files, and a duplicate only when the person chose to keep it', () => {
  const ready = item('ready');
  const keep = item('duplicate', { duplicate: 'again' });
  const items = [ready, item('duplicate'), keep, item('needs-choice'), item('complete'), item('failed'), item('skipped')];
  assert.deepEqual(toImport(items).map((i) => i.id), [ready.id, keep.id]);
});

test('when Import cannot run it says why in the order a person would meet the reasons', () => {
  assert.equal(importBlocker([]), 'Add a file first.');
  assert.equal(importBlocker([item('checking'), item('ready')]), 'Wait for the files to be checked.');
  assert.equal(importBlocker([item('ready')]), null);
  assert.match(importBlocker([item('needs-choice')]) ?? '', /column/);
  assert.match(importBlocker([item('duplicate')]) ?? '', /Skip the duplicate/);
  assert.match(importBlocker([item('failed')]) ?? '', /tried again/);
  assert.equal(importBlocker([item('complete'), item('skipped')]), 'Everything in the queue is done.');
});

test('the outcome is counted and told in one sentence', () => {
  const t = tally([item('complete'), item('complete'), item('failed'), item('skipped'), item('ready')]);
  assert.deepEqual(t, { complete: 2, failed: 1, skipped: 1, waiting: 1 });
  assert.equal(outcomeLine(t), '2 files imported, 1 failed, 1 skipped, 1 waiting.');
  assert.equal(outcomeLine(tally([item('complete')])), '1 file imported.');
  assert.equal(outcomeLine(tally([])), 'Nothing in the queue.');
});

test('a retry takes only what failed, so what is complete is never imported twice', () => {
  const failed = item('failed');
  const items = [item('complete'), failed, item('skipped'), item('ready')];
  assert.deepEqual(toRetry(items).map((i) => i.id), [failed.id]);
});

test('the same path is not queued twice, whatever its capitals, unless the earlier one was skipped', () => {
  const a = item('ready', { path: 'C:\\Data\\A.txt' });
  assert.equal(alreadyQueued([a], 'c:\\data\\a.TXT'), true);
  assert.equal(alreadyQueued([a], 'C:\\Data\\B.txt'), false);
  assert.equal(alreadyQueued([item('skipped', { path: 'C:\\Data\\A.txt' })], 'C:\\Data\\A.txt'), false);
});
