import { test } from 'node:test';
import assert from 'node:assert/strict';
import { browsePath, nativeDialogsAvailable, pickPath, suggestedProjectFile } from './pickFile';

test('in a browser there is no native dialog, so a typed path is used and an empty one gives nothing', async () => {
  assert.equal(nativeDialogsAvailable, false);
  assert.equal(await pickPath('open-project', '  "C:\\Projects\\my study.pqda"  '), 'C:\\Projects\\my study.pqda');
  assert.equal(await pickPath('import-file', ''), null);
  assert.equal(await pickPath('new-project', '   ', 'My study'), null);
  assert.equal(await browsePath('import-file'), null);
});

test('a project name becomes a file name Windows accepts', () => {
  assert.equal(suggestedProjectFile('My study'), 'My study.pqda');
  assert.equal(suggestedProjectFile('  Grief: part 1/2  '), 'Grief- part 1-2.pqda');
  assert.equal(suggestedProjectFile(''), 'My study.pqda');
  assert.equal(suggestedProjectFile('a*b?c'), 'a-b-c.pqda');
});
