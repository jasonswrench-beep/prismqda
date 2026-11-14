// Tests of the language tools themselves: node --test scripts/i18n/

import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { looksLikeWords, templateHasProse } from './audit.mjs';
import { checkLocale } from './check.mjs';
import { cleanJsxText, convertFile, decodeEntities, quote } from './convert.mjs';
import { readString, splitArgs, toTemplate } from './engine-extract.mjs';
import { infoOf } from './extract.mjs';
import { fill } from './template.mjs';

const convert = (text, rel = 'components/X.tsx', returns = false) => {
  const r = convertFile(rel, { text, returns });
  return r.changed ? r.newText : text;
};

test('JSX text is cleaned the way React reads it', () => {
  assert.equal(cleanJsxText('\n    Hello,\n    world\n  '), 'Hello, world');
  assert.equal(cleanJsxText(' a  b '), ' a  b ');
  assert.equal(cleanJsxText('\n  '), '');
  assert.equal(cleanJsxText(' '), ' ');
  assert.equal(cleanJsxText('Qualitative &bull; Quantitative'), 'Qualitative \u2022 Quantitative');
});

test('entities and quotes', () => {
  assert.equal(decodeEntities('&quot;a&quot; &amp; &#65;&#x42;'), '"a" & AB');
  assert.equal(quote("it's"), `"it's"`);
  assert.equal(quote('a\\b'), "'a\\\\b'");
  assert.equal(quote('it\'s "x"'), `'it\\'s "x"'`);
});

test('words are told from identifiers', () => {
  for (const w of ['Save', 'Save the project', 'a code', 'no one yet', 'Home']) assert.ok(looksLikeWords(w), w);
  for (const w of ['bk-button', 'src-text', 'free_span', 'https://a.b', 'image/png', '#ff0000', 'Ctrl+S', 'x', 'row-1']) assert.ok(!looksLikeWords(w), w);
  assert.ok(templateHasProse(['', ' of ', '']));
  assert.ok(!templateHasProse(['', 'px']));
  assert.ok(!templateHasProse(['row-', '']));
});

test('text between tags becomes a message', () => {
  const out = convert('export const A = () => (<p className="x">Save the project</p>);\n');
  assert.match(out, /<p className="x">\{t\('Save the project'\)\}<\/p>/);
  assert.match(out, /import \{ t \} from '\.\.\/i18n';/);
});

test('blanks are named and plural conditions become counts', () => {
  const out = convert("export const A = ({ n, name }: { n: number; name: string }) => (<p>{n} {n === 1 ? 'code' : 'codes'} in {name}</p>);\n");
  assert.match(out, /tn\(n, \{ one: '\{count\} code in \{name\}', other: '\{count\} codes in \{name\}' \}, \{ name \}\)/);
});

test('marked words become a rich message', () => {
  const out = convert('export const A = () => (<p>Press <kbd className="k">Enter</kbd> to go</p>);\n');
  assert.match(out, /rich\('Press <kbd>Enter<\/kbd> to go', \{ kbd: \(inside\) => <kbd className="k">\{inside\}<\/kbd> \}\)/);
});

test('attributes with words are wrapped and others are not', () => {
  const out = convert('export const A = () => (<button aria-label="Close the window" className="a b" id="close">x</button>);\n');
  assert.match(out, /aria-label=\{t\('Close the window'\)\}/);
  assert.match(out, /className="a b"/);
});

test('a table at the top of a file is read when it is used', () => {
  const out = convert("export const L = [{ id: 'a', label: 'Home page' }];\n");
  assert.match(out, /get label\(\) \{ return t\('Home page'\); \}/);
});

test('inside a function a value is just wrapped', () => {
  const out = convert("export function f() { return { label: 'Home page' }; }\n");
  assert.match(out, /label: t\('Home page'\)/);
});

test('numbers and counts', () => {
  const out = convert("export const f = (n: number) => `${n.toLocaleString('en-US')} items left`;\n", 'lib/x.ts', true);
  assert.match(out, /t\('\{n\} items left', \{ n: num\(n\) \}\)/);
});

test('data sent to the engine is left alone', () => {
  const src = "export const f = () => rpc('memo.add', { title: 'New memo' });\n";
  assert.equal(convert(src, 'lib/x.ts'), src);
});

test('Rust format strings become templates with named blanks', () => {
  assert.equal(toTemplate("there is already a code named '{}'", ['code.name']), "there is already a code named '{name}'");
  assert.equal(toTemplate('{} of {}', ['a.len()', 'total']), '{count} of {total}');
  assert.equal(toTemplate('{x} and {x:?}', []), '{x} and {x}');
  assert.equal(toTemplate('a {{brace}}', []), 'a {{brace}}');
  assert.deepEqual(splitArgs('"a, b", f(x, y), [1, 2]'), ['"a, b"', 'f(x, y)', '[1, 2]']);
  assert.deepEqual(readString('"a\\"b\\\\c\\n" rest', 0), { value: 'a"b\\c\n', end: 11 });
});

test('blanks and marks of a message are listed', () => {
  assert.deepEqual(infoOf('Delete {name} <b>now</b> {{x}}'), { placeholders: ['name'], tags: ['b'], stray: false });
  assert.equal(infoOf('a { b').stray, true);
});

test('a language file is checked against the program', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'prismqda-lang-'));
  const source = {
    messages: [
      { id: 'Save', files: ['x'] },
      { id: 'Delete {name}', blanks: ['name'], files: ['x'] },
      { id: '{count} codes', forms: { one: '{count} code', other: '{count} codes' }, files: ['x'] },
      { id: 'Press <kbd>Enter</kbd>', marks: ['kbd'], files: ['x'] },
    ],
  };
  const write = (code, data) => {
    const f = path.join(dir, `${code}.json`);
    fs.writeFileSync(f, JSON.stringify(data));
    return f;
  };
  const good = checkLocale(
    write('fr', {
      locale: 'fr',
      name: 'Français',
      messages: { Save: 'Enregistrer', 'Delete {name}': 'Supprimer {name}', '{count} codes': { one: '{count} code', other: '{count} codes' }, 'Press <kbd>Enter</kbd>': 'Appuyez sur <kbd>Entrée</kbd>' },
    }),
    source,
  );
  assert.deepEqual(good.errors, []);
  assert.equal(good.done, 4);
  const bad = checkLocale(
    write('de', { locale: 'de', name: 'Deutsch', messages: { 'Delete {name}': 'Löschen {nom}', 'Press <kbd>Enter</kbd>': 'Drücken', Gone: 'x', '{count} codes': { few: 'x', other: 'y {zz}' } } }),
    source,
  );
  assert.ok(bad.errors.some((e) => /uses \{nom\}/.test(e)), bad.errors.join('\n'));
  assert.ok(bad.errors.some((e) => /missing \{name\}/.test(e)));
  assert.ok(bad.errors.some((e) => /marks/.test(e)));
  assert.ok(bad.errors.some((e) => /uses \{zz\}/.test(e)));
  assert.ok(bad.warnings.some((w) => /Gone/.test(w)));
  const empty = checkLocale(write('es', { locale: 'es', name: 'Español', messages: { Save: '' } }), source);
  assert.deepEqual(empty.errors, []);
  assert.equal(empty.done, 0);
  const wrong = checkLocale(write('it', { locale: 'pt', name: 'Italiano', messages: {} }), source);
  assert.ok(wrong.errors.some((e) => /says "pt"/.test(e)));
});

test('a new language starts with every message empty, and an update adds only the new ones', () => {
  const source = { messages: [{ id: 'Save', files: [] }, { id: '{count} codes', forms: {}, files: [] }, { id: 'Open', context: 'menu', files: [] }] };
  const { file, added } = fill({ locale: 'fr', name: 'F', messages: {} }, source, ['one', 'many', 'other']);
  assert.equal(added, 3);
  assert.deepEqual(file.messages['{count} codes'], { one: '', many: '', other: '' });
  assert.equal(file.contexts.menu.Open, '');
  const again = fill({ ...file, messages: { ...file.messages, Save: 'Enregistrer' } }, { messages: [...source.messages, { id: 'New', files: [] }] }, ['one', 'other']);
  assert.equal(again.added, 1);
  assert.equal(again.file.messages.Save, 'Enregistrer');
});
