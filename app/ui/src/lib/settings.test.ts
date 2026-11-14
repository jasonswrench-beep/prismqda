import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DEFAULT_SETTINGS, TEXT_SIZES, applySettings, fontPercent, normalizeSettings, themeLabel, cleanHistoryName } from './settings';

test('nothing stored gives the defaults', () => {
  assert.deepEqual(normalizeSettings(undefined), DEFAULT_SETTINGS);
  assert.deepEqual(normalizeSettings(null), DEFAULT_SETTINGS);
  assert.deepEqual(normalizeSettings('dark'), DEFAULT_SETTINGS);
  assert.deepEqual(normalizeSettings({}), DEFAULT_SETTINGS);
});

test('good values are kept', () => {
  const good = { theme: 'dark', textSize: 130, announceCaret: false, lineNumbers: false, textWidth: 'comfortable', showUnits: false, keepBackups: 5, backupFolder: 'D:\Backups', floatingBar: false, codingMode: 'unit', language: 'pt-BR', historyName: 'Ada Lovelace', confidenceLevel: 0.99, bootstrapResamples: 5000, pairMethod: 'tukey', fixedSeed: 17, subcodes: false };
  assert.deepEqual(normalizeSettings(good), good);
});

test('the language follows the computer unless a code is chosen, and a bad value falls back to following it', () => {
  assert.equal(DEFAULT_SETTINGS.language, 'auto');
  assert.equal(normalizeSettings({ language: 'fr' }).language, 'fr');
  assert.equal(normalizeSettings({ language: 'zh-Hans' }).language, 'zh-Hans');
  assert.equal(normalizeSettings({ language: 'en-XA' }).language, 'en-XA');
  assert.equal(normalizeSettings({ language: 'French' }).language, 'auto');
  assert.equal(normalizeSettings({ language: 7 }).language, 'auto');
  assert.equal(normalizeSettings({ language: '../x' }).language, 'auto');
});

test('the backup folder is kept as typed text and the coding bar defaults on, with bad values falling back', () => {
  assert.equal(DEFAULT_SETTINGS.backupFolder, '', 'beside the project is the standard');
  assert.equal(DEFAULT_SETTINGS.floatingBar, true);
  assert.equal(normalizeSettings({ backupFolder: '  D:\Backups  ' }).backupFolder, 'D:\Backups');
  assert.equal(normalizeSettings({ backupFolder: 5 }).backupFolder, '');
  assert.equal(normalizeSettings({ floatingBar: 'no' }).floatingBar, true);
  assert.equal(normalizeSettings({ floatingBar: false }).floatingBar, false);
});

test('the reading choices default to numbered lines that fill the width, and bad values fall back', () => {
  assert.equal(DEFAULT_SETTINGS.lineNumbers, true);
  assert.equal(DEFAULT_SETTINGS.textWidth, 'fill');
  assert.equal(normalizeSettings({ lineNumbers: 'no' }).lineNumbers, true);
  assert.equal(normalizeSettings({ textWidth: 'wide' }).textWidth, 'fill');
  assert.equal(normalizeSettings({ textWidth: 'comfortable' }).textWidth, 'comfortable');
  assert.equal(normalizeSettings({ lineNumbers: false }).lineNumbers, false);
  assert.equal(DEFAULT_SETTINGS.showUnits, false);
  assert.equal(normalizeSettings({ showUnits: 'yes' }).showUnits, false);
  assert.equal(normalizeSettings({ showUnits: true }).showUnits, true);
});

test('an unknown theme, a wrong type, or a bad number falls back without throwing away the rest', () => {
  assert.deepEqual(normalizeSettings({ theme: 'neon', textSize: 'big', announceCaret: 'yes' }), DEFAULT_SETTINGS);
  assert.equal(normalizeSettings({ theme: 'contrast', textSize: Number.NaN }).theme, 'contrast');
  assert.equal(normalizeSettings({ theme: 'contrast', textSize: Number.NaN }).textSize, 100);
});

test('a size that is not offered snaps to the nearest one that is', () => {
  assert.equal(normalizeSettings({ textSize: 120 }).textSize, 115);
  assert.equal(normalizeSettings({ textSize: 500 }).textSize, 150);
  assert.equal(normalizeSettings({ textSize: 10 }).textSize, 90);
  for (const size of TEXT_SIZES) assert.equal(normalizeSettings({ textSize: size }).textSize, size);
});

test('the root font size follows the text size from the standard 14 pixels', () => {
  assert.equal(fontPercent(100), 87.5);
  assert.equal(fontPercent(130), 113.75);
  assert.equal((fontPercent(130) / 100) * 16, 18.2, '130 percent of 14 pixels');
});

test('applying the settings sets the theme attribute and the size, and the standard values clear them', () => {
  const root = { dataset: {} as Record<string, string | undefined>, style: { fontSize: '' } };
  applySettings({ ...DEFAULT_SETTINGS, theme: 'dark', textSize: 130, textWidth: 'comfortable' }, root);
  assert.equal(root.dataset.theme, 'dark');
  assert.equal(root.dataset.textWidth, 'comfortable');
  assert.equal(root.style.fontSize, '113.75%');
  applySettings(DEFAULT_SETTINGS, root);
  assert.equal('theme' in root.dataset, false, 'following Windows leaves no theme attribute');
  assert.equal('textWidth' in root.dataset, false, 'filling the width is the standard and leaves no attribute');
  assert.equal(root.style.fontSize, '', 'the standard size leaves the style sheet in charge');
});

test('themes have plain names', () => {
  assert.equal(themeLabel('system'), 'Follow Windows');
  assert.equal(themeLabel('contrast'), 'High Contrast');
});

test('the number of backups snaps to one that is offered, and the way new projects code falls back to any words', () => {
  assert.equal(DEFAULT_SETTINGS.keepBackups, 10, 'the engine keeps ten unless told otherwise');
  assert.equal(normalizeSettings({ keepBackups: 4 }).keepBackups, 3);
  assert.equal(normalizeSettings({ keepBackups: 9 }).keepBackups, 10);
  assert.equal(normalizeSettings({ keepBackups: 500 }).keepBackups, 20);
  assert.equal(normalizeSettings({ keepBackups: 'many' }).keepBackups, 10);
  assert.equal(normalizeSettings({ codingMode: 'unit' }).codingMode, 'unit');
  assert.equal(normalizeSettings({ codingMode: 'both' }).codingMode, 'free_span');
});

test('the analysis choices begin as the usual ones, and a bad value falls back or snaps to one that is offered', () => {
  assert.equal(DEFAULT_SETTINGS.confidenceLevel, 0.95);
  assert.equal(DEFAULT_SETTINGS.bootstrapResamples, 2000);
  assert.equal(DEFAULT_SETTINGS.pairMethod, 'holm');
  assert.equal(DEFAULT_SETTINGS.fixedSeed, null, 'a new seed each time unless one is chosen');
  assert.equal(DEFAULT_SETTINGS.subcodes, true);
  assert.equal(normalizeSettings({ confidenceLevel: 0.97 }).confidenceLevel, 0.95, 'the nearest level that is offered');
  assert.equal(normalizeSettings({ confidenceLevel: 0.5 }).confidenceLevel, 0.9);
  assert.equal(normalizeSettings({ bootstrapResamples: 3900 }).bootstrapResamples, 5000);
  assert.equal(normalizeSettings({ pairMethod: 'nonsense' }).pairMethod, 'holm');
  assert.equal(normalizeSettings({ pairMethod: 'games_howell' }).pairMethod, 'games_howell');
  assert.equal(normalizeSettings({ fixedSeed: 42 }).fixedSeed, 42);
  assert.equal(normalizeSettings({ fixedSeed: -1 }).fixedSeed, null, 'a seed is a whole number from 0');
  assert.equal(normalizeSettings({ fixedSeed: 1.5 }).fixedSeed, null);
  assert.equal(normalizeSettings({ fixedSeed: '7' }).fixedSeed, null);
  assert.equal(normalizeSettings({ subcodes: 'no' }).subcodes, true);
  assert.equal(normalizeSettings({ subcodes: false }).subcodes, false);
});

test('the name for the project history is kept on one line and short, and is empty (the name the program made) unless the person chose one', () => {
  assert.equal(DEFAULT_SETTINGS.historyName, '', 'the engine uses the name the program made for this computer');
  assert.equal(normalizeSettings({ historyName: 'Ada Lovelace' }).historyName, 'Ada Lovelace');
  assert.equal(normalizeSettings({ historyName: '  Ada \n \t Lovelace  ' }).historyName, 'Ada Lovelace', 'spaces and line breaks become single spaces');
  assert.equal(normalizeSettings({ historyName: 'x'.repeat(100) }).historyName.length, 60);
  assert.equal(normalizeSettings({ historyName: 42 }).historyName, '');
  assert.equal(normalizeSettings({}).historyName, '');
  assert.equal(cleanHistoryName('Ada \u0000 Lovelace'), 'Ada Lovelace', 'a control character is a space');
  assert.equal(Array.from(cleanHistoryName('\u{1F600}'.repeat(70))).length, 60, 'sixty characters, not sixty code units');
});
