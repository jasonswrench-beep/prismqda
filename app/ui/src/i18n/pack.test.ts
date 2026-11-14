import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { detectDelimiter, guardCell, parseCsv, toCsv, unguardCell } from './csv';
import { canonicalCode, directionOf, englishName, isLanguageCode, languageEntries, nativeName, pluralCategoriesOf, standardCodeFor, STANDARD_LANGUAGES } from './languages';
import { checkDraft, codeFromFileName, PackError, packToJson, parsePack, parsePackCsv, parsePackJson, sortedForTranslator, translatorCsv, translatorJson, translatorRows, type SourceMessage } from './pack';

const SOURCE: SourceMessage[] = [
  { id: 'Save', files: ['components/SettingsView.tsx'] },
  { id: 'Delete {name}?', blanks: ['name'], files: ['components/CodeDialogs.tsx'] },
  { id: '{count} codes', forms: { one: '{count} code', other: '{count} codes' }, files: ['components/StartScreen.tsx'] },
  { id: 'Press <kbd>Enter</kbd> to go', marks: ['kbd'], files: ['components/Reader.tsx'] },
  { id: 'Open', context: 'menu', files: ['components/Ribbon.tsx'] },
  { id: '+{overflow} more', blanks: ['overflow'], files: ['components/Reader.tsx'] },
];

describe('spreadsheets', () => {
  it('are read with commas, semicolons, or tabs, and quoted cells', () => {
    assert.deepEqual(parseCsv('a,b\r\n1,"x, ""y"""\r\n').rows, [['a', 'b'], ['1', 'x, "y"']]);
    assert.equal(parseCsv('a;b\n1;2').delimiter, ';');
    assert.deepEqual(parseCsv('a\tb\n1\t2').rows, [['a', 'b'], ['1', '2']]);
    assert.deepEqual(parseCsv('﻿sep=;\r\nKey;Translation\r\nA;B\r\n').rows, [['Key', 'Translation'], ['A', 'B']]);
    assert.equal(detectDelimiter('"a,b";c'), ';', 'a comma inside quotes is not a delimiter');
  });
  it('keep a line break inside a quoted cell and skip empty lines', () => {
    assert.deepEqual(parseCsv('a,b\n"line one\nline two",x\n\n').rows, [['a', 'b'], ['line one\nline two', 'x']]);
  });
  it('are written so a spreadsheet does not take text for a formula', () => {
    assert.equal(guardCell('+5 more'), '\t+5 more');
    assert.equal(unguardCell('\t+5 more'), '+5 more');
    assert.equal(guardCell('Save'), 'Save');
    assert.equal(toCsv([['a', 'b,c', 'd"e']]), 'a,"b,c","d""e"\r\n');
    assert.deepEqual(parseCsv(toCsv([['Key', 'T'], [guardCell('=x'), 'y']])).rows.map((r) => r.map(unguardCell)), [['Key', 'T'], ['=x', 'y']]);
  });
});

describe('the standard list of languages', () => {
  it('has names from the browser, in the language itself and in English', () => {
    assert.equal(englishName('fr'), 'French');
    assert.equal(nativeName('fr'), 'Français');
    assert.equal(englishName('pt-BR'), 'Portuguese (Brazil)');
    assert.equal(nativeName('de'), 'Deutsch');
  });
  it('knows which languages read right to left and which forms a count needs', () => {
    assert.equal(directionOf('ar'), 'rtl');
    assert.equal(directionOf('he'), 'rtl');
    assert.equal(directionOf('fr'), 'ltr');
    assert.deepEqual(pluralCategoriesOf('en'), ['one', 'other']);
    assert.deepEqual(pluralCategoriesOf('ar'), ['zero', 'one', 'two', 'few', 'many', 'other']);
    assert.deepEqual(pluralCategoriesOf('ja'), ['other']);
    assert.deepEqual(pluralCategoriesOf('ru'), ['one', 'few', 'many', 'other']);
  });
  it('accepts a language code as people write it and finds the entry it stands for', () => {
    assert.equal(canonicalCode('pt-br'), 'pt-BR');
    assert.equal(canonicalCode('fr_CA'), 'fr-CA');
    assert.equal(canonicalCode('French'), null);
    assert.equal(canonicalCode('x'), null);
    assert.equal(standardCodeFor('fr-BE'), 'fr', 'a region that is not listed falls back to the language');
    assert.equal(standardCodeFor('pt-BR'), 'pt-BR');
    assert.equal(standardCodeFor('tlh'), null);
    assert.ok(isLanguageCode('zh-Hans') && !isLanguageCode('../x'));
  });
  it('lists every language with a name, sorted, none twice', () => {
    const list = languageEntries();
    assert.equal(list.length, STANDARD_LANGUAGES.length);
    assert.equal(new Set(list.map((l) => l.code)).size, list.length);
    assert.ok(list.every((l) => l.english && l.english !== l.code), 'every code has an English name');
    assert.ok(list.every((l) => l.native));
    assert.deepEqual(list.map((l) => l.english), [...list.map((l) => l.english)].sort((a, b) => a.localeCompare(b, 'en')));
  });
});

describe('reading a translation file', () => {
  it('takes JSON, with what it says about itself', () => {
    const d = parsePackJson(JSON.stringify({ locale: 'fr', label: 'Français', credit: 'A. Roy', madeBy: 'person', messages: { Save: 'Enregistrer' } }));
    assert.equal(d.code, 'fr');
    assert.equal(d.credit, 'A. Roy');
    assert.equal(d.madeBy, 'person');
    assert.deepEqual(d.messages, { Save: 'Enregistrer' });
  });
  it('refuses what is not a translation, with a reason a person can follow', () => {
    assert.throws(() => parsePackJson('not json'), PackError);
    assert.throws(() => parsePackJson('{"a":1}'), /no "messages" part/);
    assert.throws(() => parsePackCsv('a,b\n1,2'), /Key and Translation/);
  });
  it('takes a spreadsheet and joins the rows of one count message into its forms', () => {
    const d = parsePackCsv('Key,Context,Form,English,Translation,Keep\r\nSave,,,Save, Enregistrer \r\n{count} codes,,one,{count} code,{count} code\r\n{count} codes,,other,{count} codes,{count} codes\r\nOpen,menu,,Open,Ouvrir\r\n');
    assert.equal(d.messages.Save, 'Enregistrer', 'spaces the spreadsheet added are dropped');
    assert.deepEqual(d.messages['{count} codes'], { one: '{count} code', other: '{count} codes' });
    assert.deepEqual(d.contexts.menu, { Open: 'Ouvrir' });
  });
  it('chooses CSV or JSON by what the file looks like', () => {
    assert.equal(parsePack('{"messages":{}}', 'x.txt').format, 'json');
    assert.equal(parsePack('Key,Translation\nA,B', 'x.csv').format, 'csv');
  });
  it('guesses the language from the file name', () => {
    assert.equal(codeFromFileName('fr.csv'), 'fr');
    assert.equal(codeFromFileName('pt-BR.json'), 'pt-BR');
    assert.equal(codeFromFileName('prismqda_es.csv'), 'es');
    assert.equal(codeFromFileName('translation.csv'), null);
  });
});

describe('checking a translation against the program', () => {
  const draftOf = (messages: Record<string, unknown>, contexts: Record<string, Record<string, unknown>> = {}) => ({ format: 'json' as const, code: 'fr', label: null, credit: null, madeBy: null, messages, contexts, unreadable: 0 });

  it('keeps what is right and counts it', () => {
    const r = checkDraft(draftOf({ Save: 'Enregistrer', 'Delete {name}?': 'Supprimer {name} ?', 'Press <kbd>Enter</kbd> to go': 'Appuyez sur <kbd>Entrée</kbd>' }, { menu: { Open: 'Ouvrir' } }), SOURCE);
    assert.equal(r.done, 4);
    assert.equal(r.total, SOURCE.length);
    assert.deepEqual(r.problems, []);
    assert.equal(r.catalog.messages.Save, 'Enregistrer');
    assert.equal(r.catalog.contexts?.menu?.Open, 'Ouvrir');
  });
  it('leaves out a message whose blank was changed or lost, and says why', () => {
    const r = checkDraft(draftOf({ 'Delete {name}?': 'Supprimer {nom} ?' }), SOURCE);
    assert.equal(r.done, 0);
    assert.match(r.problems[0]?.what ?? '', /uses \{nom\}/);
    const lost = checkDraft(draftOf({ 'Delete {name}?': 'Supprimer ?' }), SOURCE);
    assert.match(lost.problems[0]?.what ?? '', /missing \{name\}/);
  });
  it('leaves out a message whose marks differ, or that has a single brace', () => {
    assert.match(checkDraft(draftOf({ 'Press <kbd>Enter</kbd> to go': 'Appuyez' }), SOURCE).problems[0]?.what ?? '', /marks/);
    assert.match(checkDraft(draftOf({ Save: 'a { b' }), SOURCE).problems[0]?.what ?? '', /single brace/);
  });
  it('takes the forms of a count message and needs "other"', () => {
    const ok = checkDraft(draftOf({ '{count} codes': { one: '{count} code', other: '{count} codes', few: '' } }), SOURCE);
    assert.equal(ok.done, 1);
    assert.deepEqual(ok.catalog.messages['{count} codes'], { one: '{count} code', other: '{count} codes' });
    assert.match(checkDraft(draftOf({ '{count} codes': { one: '{count} code' } }), SOURCE).problems[0]?.what ?? '', /other/);
    assert.match(checkDraft(draftOf({ '{count} codes': { several: 'x', other: 'y' } }), SOURCE).problems[0]?.what ?? '', /several/);
    assert.match(checkDraft(draftOf({ '{count} codes': { one: '{nope}', other: 'y' } }), SOURCE).problems[0]?.what ?? '', /\{nope\}/);
  });
  it('skips empty translations without complaint and counts messages the program does not have', () => {
    const r = checkDraft(draftOf({ Save: '', 'Gone message': 'x' }), SOURCE);
    assert.equal(r.done, 0);
    assert.equal(r.unknown, 1);
    assert.deepEqual(r.problems, []);
  });
  it('refuses a file with far more entries than the program has', () => {
    const many: Record<string, unknown> = {};
    for (let i = 0; i < 20_100; i++) many[`m${i}`] = 'x';
    assert.throws(() => checkDraft(draftOf(many), SOURCE), PackError);
  });
  it('does not take a message from the prototype of an object', () => {
    const r = checkDraft(draftOf({ constructor: 'x', __proto__: 'y', toString: 'z' }), SOURCE);
    assert.equal(r.done, 0);
  });
});

describe('the files a translator starts from', () => {
  it('put the screens a person meets first', () => {
    const order = sortedForTranslator(SOURCE).map((m) => m.id);
    assert.equal(order[0], '{count} codes', 'the start screen comes first');
    assert.ok(order.indexOf('Open') < order.indexOf('Save'), 'the ribbon before Settings');
  });
  it('have a row for each form a language needs, and a row for each plain message', () => {
    const fr = translatorRows(SOURCE, 'de');
    assert.equal(fr.filter((r) => r[0] === '{count} codes').length, 2, 'German needs one and other');
    assert.equal(translatorRows(SOURCE, 'fr').filter((r) => r[0] === '{count} codes').length, 3, 'French also has a form for millions');
    const ru = translatorRows(SOURCE, 'ru');
    assert.equal(ru.filter((r) => r[0] === '{count} codes').map((r) => r[2]).join(), 'one,few,many,other');
    const ja = translatorRows(SOURCE, 'ja');
    assert.equal(ja.filter((r) => r[0] === '{count} codes').length, 1);
    const plus = fr.find((r) => r[3].includes('overflow'));
    assert.ok(plus?.[0].startsWith('\t'), 'text a spreadsheet would read as a formula is guarded');
    assert.equal(fr.find((r) => r[0] === 'Delete {name}?')?.[5], '{name}', 'the blanks to keep are listed');
    assert.equal(fr.find((r) => r[0] === 'Press <kbd>Enter</kbd> to go')?.[5], '<kbd>...</kbd>');
  });
  it('can hold only what is not translated yet, and bring back what is', () => {
    const existing = { messages: { Save: 'Enregistrer' } };
    const missing = translatorRows(SOURCE, 'fr', existing, true);
    assert.ok(!missing.some((r) => r[0] === 'Save'));
    assert.ok(missing.some((r) => r[0] === 'Delete {name}?'));
    const all = translatorRows(SOURCE, 'fr', existing, false);
    assert.equal(all.find((r) => r[0] === 'Save')?.[4], 'Enregistrer');
  });
  it('make a spreadsheet that comes back in whole through the reader', () => {
    const { text, rows } = translatorCsv(SOURCE, 'de');
    assert.ok(text.startsWith('﻿sep=,\r\n'));
    assert.equal(rows, SOURCE.length + 1, 'the count message has two rows');
    // fill in the Translation column the way a translator would
    const parsed = parseCsv(text).rows;
    const head = parsed[0] as string[];
    const t = head.indexOf('Translation');
    const filled = parsed.map((r, i) => (i === 0 ? r : r.map((c, j) => (j === t ? `T:${unguardCell(r[3] as string)}` : c))));
    const back = parsePackCsv(toCsv(filled));
    const checked = checkDraft(back, SOURCE);
    assert.equal(checked.done, SOURCE.length);
    assert.equal(checked.problems.length, 0, JSON.stringify(checked.problems));
  });
  it('make a JSON file that comes back in whole too', () => {
    const json = translatorJson(SOURCE, 'ru', { label: 'Русский', credit: 'N', madeBy: 'person' });
    const draft = parsePackJson(json);
    assert.equal(draft.code, 'ru');
    assert.deepEqual(Object.keys((draft.messages['{count} codes'] as Record<string, string>)), ['one', 'few', 'many', 'other']);
    assert.equal(checkDraft(draft, SOURCE).done, 0, 'nothing is translated yet');
  });
  it('write a translation as a file that reads back the same', () => {
    const catalog = { messages: { Save: 'Enregistrer', '{count} codes': { one: '{count} code', other: '{count} codes' } }, contexts: { menu: { Open: 'Ouvrir' } } };
    const info = { id: 'fr', code: 'fr', label: 'Français', credit: 'A. Roy', madeBy: 'person' as const, added: '2026-11-07', dir: 'ltr' as const, done: 3, total: 6 };
    const draft = parsePackJson(packToJson(info, catalog));
    const checked = checkDraft(draft, SOURCE);
    assert.deepEqual(checked.catalog, catalog);
    assert.equal(draft.label, 'Français');
    assert.equal(draft.credit, 'A. Roy');
  });
});
