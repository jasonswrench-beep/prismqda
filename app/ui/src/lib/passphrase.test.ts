import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MIN_PASSPHRASE_CHARS, passphraseHint, passphraseProblem, projectLabel } from './passphrase';

test('a passphrase is checked in the order a person would fix things', () => {
  assert.match(passphraseProblem('', '') ?? '', /Type a passphrase/);
  assert.match(passphraseProblem('   ', '   ') ?? '', /Type a passphrase/);
  assert.match(passphraseProblem("x'0011223344", "x'0011223344") ?? '', /raw key/);
  assert.match(passphraseProblem('short', 'short') ?? '', /at least 8 characters/);
  assert.match(passphraseProblem('long enough one', 'long enough two') ?? '', /not the same/);
  assert.equal(passphraseProblem('long enough one', 'long enough one'), null);
});

test('characters are counted, not bytes', () => {
  const eight = '\u{1F600}'.repeat(MIN_PASSPHRASE_CHARS);
  assert.equal(passphraseProblem(eight, eight), null);
  assert.match(passphraseProblem('\u{1F600}'.repeat(MIN_PASSPHRASE_CHARS - 1), '\u{1F600}'.repeat(MIN_PASSPHRASE_CHARS - 1)) ?? '', /at least 8/);
});

test('the hint counts length and says nothing more', () => {
  assert.equal(passphraseHint(''), '');
  assert.match(passphraseHint('abc'), /Too short/);
  assert.match(passphraseHint('abcdefghij'), /Twelve or more/);
  assert.match(passphraseHint('abcdefghijkl'), /good length/);
});

test('a project is named by its file without the folder or the ending', () => {
  assert.equal(projectLabel('C:\\Studies\\Urban Mobility.pqda'), 'Urban Mobility');
  assert.equal(projectLabel('/home/a/study.PQDA'), 'study');
  assert.equal(projectLabel('plain'), 'plain');
});
