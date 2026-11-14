import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { SourceRow } from '../types';
import { ancestorKeys, buildDocTree, filterDocTree, folderKeys, folderParts } from './docTree';

const src = (id: number, name: string, folder = ''): SourceRow => ({
  id,
  name,
  source_type: 'document',
  folder,
  characters: 10,
  default_unit_kind: 'sentence',
  text_revision: 1,
  units: 1,
  codings: 0,
  do_not_share: false,
  created_at: '2026-11-06T00:00:00Z',
});

const sources = [
  src(1, 'INT-002 – Priya', 'Interviews'),
  src(2, 'INT-001 – Alex', 'Interviews'),
  src(3, 'FG-001 – Commuters', 'Focus Groups'),
  src(4, 'Notes'),
  src(5, 'INT-010 – Sam', 'Interviews/Spring'),
];

test('folder names are paths split on either slash', () => {
  assert.deepEqual(folderParts('Interviews/Spring'), ['Interviews', 'Spring']);
  assert.deepEqual(folderParts('A\\B'), ['A', 'B']);
  assert.deepEqual(folderParts('  '), []);
  assert.deepEqual(folderParts(''), []);
});

test('the project is the root, folders come before documents, and counts are of everything beneath', () => {
  const root = buildDocTree('Urban Mobility Study', sources);
  assert.equal(root.label, 'Urban Mobility Study');
  assert.equal(root.count, 5);
  assert.deepEqual(root.children.map((c) => [c.kind, c.label, c.count]), [
    ['folder', 'Focus Groups', 1],
    ['folder', 'Interviews', 3],
    ['document', 'Notes', 1],
  ]);
  const interviews = root.children[1];
  assert.deepEqual(interviews.children.map((c) => c.label), ['Spring', 'INT-001 – Alex', 'INT-002 – Priya'], 'subfolder first, then documents in name order');
});

test('numbers in names sort as numbers', () => {
  const root = buildDocTree('P', [src(1, 'Doc 10'), src(2, 'Doc 9'), src(3, 'Doc 2')]);
  assert.deepEqual(root.children.map((c) => c.label), ['Doc 2', 'Doc 9', 'Doc 10']);
});

test('a document keeps its source and a key built from its id', () => {
  const root = buildDocTree('P', sources);
  const alex = root.children[1].children[1];
  assert.equal(alex.key, 'd2');
  assert.equal(alex.source?.id, 2);
});

test('searching keeps matches with the folders that hold them, and counts the matches', () => {
  const root = buildDocTree('P', sources);
  const found = filterDocTree(root, 'int sam');
  assert.equal(found.count, 1);
  assert.deepEqual(found.children.map((c) => c.label), ['Interviews']);
  assert.deepEqual(found.children[0].children.map((c) => c.label), ['Spring']);
  assert.equal(filterDocTree(root, '  ').count, 5, 'an empty search changes nothing');
  assert.equal(filterDocTree(root, 'zebra').count, 0);
  assert.deepEqual(filterDocTree(root, 'zebra').children, []);
});

test('every folder key and the way down to a document can be listed', () => {
  const root = buildDocTree('P', sources);
  assert.deepEqual(folderKeys(root), ['root', 'f:Focus Groups', 'f:Interviews', 'f:Interviews/Spring']);
  assert.deepEqual(ancestorKeys(root, 'd5'), ['root', 'f:Interviews', 'f:Interviews/Spring']);
  assert.deepEqual(ancestorKeys(root, 'd4'), ['root']);
  assert.deepEqual(ancestorKeys(root, 'd99'), []);
});
