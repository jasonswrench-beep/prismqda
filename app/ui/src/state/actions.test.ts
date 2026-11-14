import { test } from 'node:test';
import assert from 'node:assert/strict';
import { defaultViewFor, inVivoName, withProjectExtension } from './actions';

test('an in-vivo code name is made from the selected words, tidied', () => {
  assert.equal(inVivoName('felt welcome'), 'felt welcome');
  assert.equal(inVivoName('  felt \n welcome  '), 'felt welcome');
  assert.equal(inVivoName('"felt welcome."'), 'felt welcome');
  assert.equal(inVivoName('(well, maybe)'), 'well, maybe');
  assert.equal(inVivoName('...'), '');
});

test('a long selection gives a short name that stops at a word', () => {
  const name = inVivoName('word '.repeat(40));
  assert.ok(Array.from(name).length <= 60, name);
  assert.ok(!name.endsWith(' '));
  const cjk = inVivoName('\u6211'.repeat(100));
  assert.equal(Array.from(cjk).length, 60, 'text without spaces is cut at 60 characters, never in the middle of one');
  const emoji = inVivoName('\u{1F600}'.repeat(100));
  assert.equal(Array.from(emoji).length, 60);
});

test('the project file ending is added only when it is missing', () => {
  assert.equal(withProjectExtension('C:\\Projects\\study'), 'C:\\Projects\\study.pqda');
  assert.equal(withProjectExtension('/home/me/study'), '/home/me/study.pqda');
  assert.equal(withProjectExtension('study.pqda'), 'study.pqda');
  assert.equal(withProjectExtension('C:\\a.b\\study'), 'C:\\a.b\\study.pqda', 'a dot in a folder name is not an ending');
  assert.equal(withProjectExtension('study.db'), 'study.db', 'another ending is left alone');
});

test('large sources open in the grid, replies in the thread, the rest as a document', () => {
  assert.equal(defaultViewFor({ units: 10, characters: 3000 }, false), 'document');
  assert.equal(defaultViewFor({ units: 1000, characters: 3000 }, false), 'document');
  assert.equal(defaultViewFor({ units: 1001, characters: 3000 }, false), 'grid');
  assert.equal(defaultViewFor({ units: 10, characters: 400_000 }, false), 'document');
  assert.equal(defaultViewFor({ units: 10, characters: 400_001 }, false), 'grid');
  assert.equal(defaultViewFor({ units: 15_000, characters: 2_600_000 }, true), 'thread');
});
