import assert from 'node:assert/strict';
import { test } from 'node:test';
import { freeName, joinPath, projectFileName, splitPath, targetPath, validateProjectName } from './projectPath';

test('a usable name passes, and the ways a name can fail are told in words', () => {
  assert.equal(validateProjectName('Urban Mobility Study'), null);
  assert.equal(validateProjectName('Study: wave 2'), 'The name cannot contain ":". Windows does not allow \\ / : * ? " < > | in a file name.');
  assert.match(validateProjectName('   ') ?? '', /Give the project a name/);
  assert.match(validateProjectName('a/b') ?? '', /cannot contain "\/"/);
  assert.match(validateProjectName('say "hi"') ?? '', /quotation mark/);
  assert.match(validateProjectName('CON') ?? '', /keeps for itself/);
  assert.match(validateProjectName('com3.txt') ?? '', /keeps for itself/);
  assert.match(validateProjectName('x'.repeat(101)) ?? '', /100 characters/);
  assert.match(validateProjectName('Study.') ?? '', /end with a period/);
  assert.equal(validateProjectName('Etude \u00e9t\u00e9 \u4e2d\u6587'), null, 'letters of any language are fine');
});

test('the file name comes from the project name, with the ending added once', () => {
  assert.equal(projectFileName('Urban Mobility Study'), 'Urban Mobility Study.pqda');
  assert.equal(projectFileName('a:b*c'), 'a-b-c.pqda');
  assert.equal(projectFileName('   '), 'My study.pqda');
  assert.equal(projectFileName('Study. '), 'Study.pqda');
});

test('folder and file are joined with the slash the folder already uses', () => {
  assert.equal(joinPath('C:\\Users\\me\\Documents\\PrismQDA Projects', 'a.pqda'), 'C:\\Users\\me\\Documents\\PrismQDA Projects\\a.pqda');
  assert.equal(joinPath('C:\\Projects\\', 'a.pqda'), 'C:\\Projects\\a.pqda');
  assert.equal(joinPath('/home/me/projects', 'a.pqda'), '/home/me/projects/a.pqda');
  assert.equal(joinPath('  ', 'a.pqda'), 'a.pqda');
});

test('a save location is a folder, unless it already names the project file', () => {
  assert.equal(targetPath('C:\\Projects', 'My Study'), 'C:\\Projects\\My Study.pqda');
  assert.equal(targetPath('C:\\Projects\\my-study.pqda', 'Anything'), 'C:\\Projects\\my-study.pqda');
  assert.equal(targetPath('"C:\\Projects\\x.pqda"', 'Anything'), 'C:\\Projects\\x.pqda', 'quotes from Copy as path are dropped');
  assert.equal(targetPath('', 'My Study'), 'My Study.pqda');
});

test('a path is split into its folder and its file name', () => {
  assert.deepEqual(splitPath('C:\\Projects\\a.pqda'), { folder: 'C:\\Projects', file: 'a.pqda' });
  assert.deepEqual(splitPath('/x/y.pqda'), { folder: '/x', file: 'y.pqda' });
  assert.deepEqual(splitPath('y.pqda'), { folder: '', file: 'y.pqda' });
});

test('a taken name gets the next free number', () => {
  assert.equal(freeName('Study', ['other']), 'Study');
  assert.equal(freeName('Study', ['study']), 'Study 2');
  assert.equal(freeName('Study', ['Study', 'Study 2', 'study 3']), 'Study 4');
});
