// Translations that a person brings in: a file made elsewhere is read, checked, labelled, and added; the window is shown in it; a
// copy can be saved, a spreadsheet for a translator can be saved, and a translation can be removed. The translations here are
// invented (a handful of words in Spanish and German), only enough to show the window changes and the checks tell what they should.

import fs from 'node:fs';
import path from 'node:path';

export async function runTranslations(ctx, api) {
  const { h, workDir } = ctx;
  const { page, S, axe } = api;

  const messagesFile = path.resolve(import.meta.dirname, '..', 'src', 'i18n', 'messages.json');
  const program = JSON.parse(fs.readFileSync(messagesFile, 'utf8'));

  const settingsTab = () => page.getByRole('banner').getByRole('button', { name: /^(Settings|Ajustes)$/ });
  const dialog = (name) => page.getByRole('dialog', { name });
  const stored = () => page.evaluate(() => Object.keys(localStorage).filter((k) => k.startsWith('prismqda.language')));

  async function openSettings() {
    await settingsTab().click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'General', exact: true }).click();
    await page.getByRole('heading', { level: 4, name: /^(Language|Idioma)/ }).waitFor();
  }

  async function addFile(file, { code = null, label = null, credit = null, madeBy = null, existing = null, use = true } = {}) {
    await page.getByTestId('add-translation').click();
    const d = dialog('Add a translation');
    await d.waitFor();
    await d.getByLabel('Translation file').fill(file);
    await d.getByRole('button', { name: 'Read this file' }).click();
    await d.getByTestId('translation-report').waitFor();
    if (code) await d.getByTestId('al-language').selectOption(code);
    if (label !== null) await d.getByLabel('What to call it').fill(label);
    if (credit !== null) await d.getByLabel('Translated by').fill(credit);
    if (madeBy) await d.locator('label').filter({ has: page.getByRole('radio', { name: madeBy }) }).click();
    if (existing !== null) await d.getByTestId('al-replace').selectOption(existing);
    if (!use) await d.locator('label').filter({ hasText: 'Show the window in it now' }).click();
    return d;
  }

  const es = path.join(workDir, 'es.json');
  fs.writeFileSync(
    es,
    JSON.stringify({
      locale: 'es',
      credit: 'La clase de prueba',
      madeBy: 'person',
      messages: {
        Settings: 'Ajustes',
        Language: 'Idioma',
        'Interface language': 'Idioma de la ventana',
        'Translations on this computer': 'Traducciones en este equipo',
        'Add a translation…': 'Añadir una traducción…',
        '{count} backups': 'Copias: {count}',
        'Removed the translation {label}.': 'Se quitó {nombre}.',
        'This message is not in the program': 'x',
      },
    }),
  );

  await S('Settings lists no translations to begin with, and says what a translation is', async () => {
    await openSettings();
    h.equal(await page.getByLabel('Interface language').isDisabled(), true, 'only English, so nothing to choose');
    h.check(await page.getByTestId('no-translations').isVisible(), 'and the page says none have been added');
    h.check(/does not translate/.test(await page.getByTestId('no-translations').innerText()), 'and that PrismQDA does not translate');
    await axe('Settings, Language, before any translation');
  });

  await S('a translation file is read and checked before anything is kept, and what is wrong in it is told', async () => {
    const d = await addFile(es);
    const report = await d.getByTestId('translation-report').innerText();
    const total = program.messages.length;
    h.check(new RegExp(`6 of ${total.toLocaleString('en-US')} messages are translated`).test(report), `how much is translated: ${report.split('\n')[0]}`);
    h.check(/1 entry was left out/.test(report) && /uses \{nombre\}/.test(report), 'an entry whose blank was changed is left out, with the reason');
    h.check(/1 entry is for a message this version of PrismQDA does not have/.test(report), 'and an entry for a message that does not exist is told');
    h.equal(await d.getByTestId('al-language').inputValue(), 'es', 'the language the file says is chosen');
    h.equal(await d.getByLabel('What to call it').inputValue(), 'Español', 'and it is called by its own name until the person says otherwise');
    h.equal(await d.getByLabel('Translated by').inputValue(), 'La clase de prueba', 'the credit the file carries is there');
    const options = await d.getByTestId('al-language').locator('option').allTextContents();
    h.check(options.length >= 70, `the standard list has ${options.length} languages`);
    h.check(options.some((o) => /^Français \(French\)$/.test(o)) && options.some((o) => /^Deutsch \(German\)$/.test(o)) && options.some((o) => /\(Arabic\)$/.test(o)), 'each is named in its own language and in English');
    await axe('the Add a translation window, after a file was read');
    await d.getByRole('button', { name: 'Cancel' }).click();
    await d.waitFor({ state: 'hidden' });
    h.equal(await stored(), [], 'cancelling keeps nothing');
  });

  await S('a translation is labelled, added, and the window is shown in it, and stays so after a reload', async () => {
    const d = await addFile(es, { label: 'Español (prueba)', credit: 'La clase de prueba, 2026', madeBy: 'A machine translated it' });
    await d.getByTestId('al-add').click();
    await d.waitFor({ state: 'hidden' });
    await page.getByRole('heading', { level: 2, name: 'Ajustes' }).waitFor();
    h.equal(await page.evaluate(() => document.documentElement.lang), 'es', 'the page says it is in Spanish');
    h.equal(await page.getByLabel('Idioma de la ventana').inputValue(), 'es', 'the language choice shows the new translation');
    const row = page.getByTestId('translation-es');
    const text = await row.innerText();
    h.check(/Español \(prueba\)/.test(text) && /6 of/.test(text) && /machine translation, not checked/.test(text) && /La clase de prueba, 2026/.test(text), `the list says what it is: ${text.replace(/\s+/g, ' ')}`);
    h.check((await stored()).includes('prismqda.language.es'), 'it is kept on this computer');
    await page.reload();
    await settingsTab().waitFor();
    h.equal(await page.getByRole('banner').getByRole('button', { name: 'Ajustes', exact: true }).count(), 1, 'after a reload the window is still in Spanish');
    h.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('prismqda.settings')).language), 'es', 'the choice is remembered');
  });

  await S('the choice can go back to English or to following the computer, and a word with no translation shows in English', async () => {
    await openSettings();
    const select = page.getByLabel('Idioma de la ventana');
    const names = await select.locator('option').allTextContents();
    h.check(names.some((n) => n === 'Español (prueba)') && names.some((n) => n === 'English'), `the list: ${names.join(' | ')}`);
    h.check(names.some((n) => /^Same as Windows$/.test(n)) || names.length >= 3, 'with the choice to follow the computer');
    await select.selectOption('en');
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    h.equal(await page.evaluate(() => document.documentElement.lang), 'en', 'English again');
    await page.getByLabel('Interface language').selectOption('es');
    await page.getByRole('heading', { level: 2, name: 'Ajustes' }).waitFor();
    // 'Home' has no translation in the file, so it is English on the screen
    h.check((await page.getByRole('main').innerText()).includes('Backup'), 'a word the file does not translate is in English');
    await page.getByLabel('Idioma de la ventana').selectOption('en');
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
  });

  await S('a second translation of the same language replaces the first or is kept beside it, as the person chooses', async () => {
    let d = await addFile(es, { label: 'Español (prueba)', credit: 'La clase de prueba, 2026', madeBy: 'A machine translated it', use: false });
    h.equal(await d.getByTestId('al-replace').inputValue(), 'es', 'a translation of the language is already here, and replacing it is the standard choice');
    h.check((await d.getByTestId('al-replace').locator('option').allTextContents()).includes('Keep both'), 'keeping both is a choice');
    await d.getByTestId('al-add').click();
    await d.waitFor({ state: 'hidden' });
    h.equal(await page.getByTestId('translations-list').locator('li').count(), 1, 'replacing leaves one');
    d = await addFile(es, { label: 'Español (otra)', existing: 'keep', use: false });
    await d.getByTestId('al-add').click();
    await d.waitFor({ state: 'hidden' });
    h.equal(await page.getByTestId('translations-list').locator('li').count(), 2, 'keeping both leaves two');
    h.check(await page.getByTestId('translation-es~2').isVisible(), 'the second has its own name');
    await axe('Settings, Language, with two translations');
  });

  await S('a spreadsheet saved by Excel, with semicolons and the old Windows letters, is read, and the window warns about the letters', async () => {
    const csv = path.join(workDir, 'de-excel.csv');
    fs.writeFileSync(csv, Buffer.from('Key;Context;Form;English;Translation\r\nSettings;;;Settings;Einstellungen\r\nLanguage;;;Language;Sprache\r\nInterface language;;;Interface language;Oberfläche\r\n', 'latin1'));
    const d = await addFile(csv, { code: 'de', label: 'Deutsch (Tabelle)', madeBy: 'A person reviewed a machine translation' });
    const report = await d.getByTestId('translation-report').innerText();
    h.check(/3 of/.test(report), `three messages are translated: ${report.split('\n')[0]}`);
    h.check(/not saved as UTF-8/.test(report), 'the page warns that accents may be wrong');
    await d.getByTestId('al-add').click();
    await d.waitFor({ state: 'hidden' });
    await page.getByRole('heading', { level: 2, name: 'Einstellungen' }).waitFor();
    h.check((await page.getByLabel('Oberfläche').count()) === 1, 'the letter with an accent came out right');
    await page.getByLabel('Oberfläche').selectOption('en');
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
  });

  await S('a file that is not a translation is refused with a reason', async () => {
    const bad = path.join(workDir, 'notes.csv');
    fs.writeFileSync(bad, 'Name,Age\r\nAda,36\r\n');
    await page.getByTestId('add-translation').click();
    const d = dialog('Add a translation');
    await d.getByLabel('Translation file').fill(bad);
    await d.getByRole('button', { name: 'Read this file' }).click();
    await d.getByText(/no columns called Key and Translation/).waitFor();
    h.equal(await d.getByTestId('translation-report').count(), 0, 'nothing is offered to add');
    await d.getByLabel('Translation file').fill(path.join(workDir, 'missing.csv'));
    await d.getByRole('button', { name: 'Read this file' }).click();
    await d.getByText(/cannot read/).waitFor();
    h.check(true, 'a file that is not there is told');
    await d.getByLabel('Translation file').fill('es.json');
    await d.getByRole('button', { name: 'Read this file' }).click();
    await d.getByRole('alert').getByText(/full path/).waitFor();
    h.check(true, 'a path that is not complete says what to type');
    await d.getByRole('button', { name: 'Cancel' }).click();
    await d.waitFor({ state: 'hidden' });
  });

  await S('a file for a translator lists the words of the window, the first screens first, and does not replace a file that is there', async () => {
    await page.getByTestId('translator-file').click();
    const d = dialog('Save a file for a translator');
    await d.waitFor();
    await d.getByLabel('Where to save it').fill(path.join(workDir, 'it-blank.csv'));
    await d.getByTestId('tf-save').click();
    await d.getByText('Choose the language the file is for.').waitFor();
    await d.getByTestId('tf-language').selectOption('it');
    await axe('the Save a file for a translator window');
    await d.getByTestId('tf-save').click();
    await d.getByTestId('translator-file-saved').waitFor();
    const text = fs.readFileSync(path.join(workDir, 'it-blank.csv'), 'utf8');
    h.check(text.startsWith('﻿sep=,\r\nKey,Context,Form,English,Translation,Keep these as they are\r\n'), 'a spreadsheet that opens in Excel with the right letters and the right delimiter');
    const lines = text.split('\r\n');
    h.check(lines.length > 2800, `a line for each message: ${lines.length}`);
    const first = program.messages.filter((m) => m.files.includes('components/StartScreen.tsx')).map((m) => m.id);
    h.check(first.some((id) => lines[2].replace(/""/g, '"').includes(id.slice(0, 15))), `the start screen comes first: ${lines[2].slice(0, 80)}`);
    await d.getByRole('button', { name: 'Done' }).click();
    await d.waitFor({ state: 'hidden' });
    // again to the same place
    await page.getByTestId('translator-file').click();
    await d.waitFor();
    await d.getByTestId('tf-language').selectOption('it');
    await d.getByLabel('Where to save it').fill(path.join(workDir, 'it-blank.csv'));
    await d.getByTestId('tf-save').click();
    await d.getByText(/already exists/).waitFor();
    h.equal(fs.readFileSync(path.join(workDir, 'it-blank.csv'), 'utf8'), text, 'the file that was there is as it was');
    await d.getByRole('button', { name: 'Cancel' }).click();
    await d.waitFor({ state: 'hidden' });
  });

  await S('a copy of a translation is saved as a file that can be added again', async () => {
    await page.getByRole('button', { name: 'Save a copy of Español (prueba)' }).click();
    const d = dialog('Save a copy of Español (prueba)');
    await d.waitFor();
    await d.locator('label').filter({ has: page.getByRole('radio', { name: 'A JSON file' }) }).click();
    await d.getByLabel('Where to save it').fill(path.join(workDir, 'es-copy.json'));
    await d.getByTestId('tf-save').click();
    await d.getByTestId('translator-file-saved').waitFor();
    const copy = JSON.parse(fs.readFileSync(path.join(workDir, 'es-copy.json'), 'utf8'));
    h.equal([copy.locale, copy.label, copy.madeBy, copy.messages.Settings], ['es', 'Español (prueba)', 'machine', 'Ajustes'], 'the copy carries the label, how it was made, and the words');
    await d.getByRole('button', { name: 'Done' }).click();
    await d.waitFor({ state: 'hidden' });
  });

  await S('a translation is removed after asking, and the window goes back to English when it was the one in use', async () => {
    const select = page.getByLabel('Interface language');
    await select.selectOption('es~2');
    await page.getByRole('heading', { level: 2, name: 'Ajustes' }).waitFor();
    await page.getByRole('button', { name: 'Remove Español (otra)' }).click();
    await page.getByText(/Remove Español \(otra\) from this computer\?/).waitFor();
    await page.getByRole('button', { name: 'Keep it' }).click();
    h.check(await page.getByTestId('translation-es~2').isVisible(), 'saying keep it keeps it');
    await page.getByRole('button', { name: 'Remove Español (otra)' }).click();
    await page.getByTestId('remove-yes-es~2').click();
    await page.getByRole('heading', { level: 2, name: /^(Settings|Ajustes)$/ }).waitFor();
    h.equal(await page.getByTestId('translation-es~2').count(), 0, 'the translation is gone from the list');
    h.check(!(await stored()).includes('prismqda.language.es~2'), 'and from this computer');
    h.equal(await page.evaluate(() => document.documentElement.lang), 'en', 'the window follows the computer again, which is English');
    for (let i = 0; i < 2; i++) {
      await page.getByTestId('translations-list').getByRole('button', { name: /^Remove / }).first().click();
      await page.getByRole('button', { name: 'Yes, remove it' }).click();
    }
    await page.getByTestId('no-translations').waitFor();
    h.equal(await stored(), ['prismqda.languages'], 'nothing but the empty index is left');
    h.equal(await page.getByLabel('Interface language').isDisabled(), true, 'only English again');
  });
}
