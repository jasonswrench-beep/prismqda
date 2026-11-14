import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { createElement } from 'react';
import { interpolate, lookup, pickPlural, placeholdersOf, pseudo, resolveLocale, tagsOf, formatTag } from './core';
import { N_, currentLocale, dateTime, list, num, rich, setLocaleData, t, tc, tn } from './index';
import { richNodes } from './rich';

afterEach(() => setLocaleData('en', null));

describe('interpolate', () => {
  it('fills blanks and leaves an unfilled one visible', () => {
    assert.equal(interpolate('Delete {name}?', { name: 'Welcome' }), 'Delete Welcome?');
    assert.equal(interpolate('Delete {name}?', {}), 'Delete {name}?');
    assert.equal(interpolate('Delete {name}?'), 'Delete {name}?');
  });
  it('turns doubled braces into single ones and does not fill them', () => {
    assert.equal(interpolate('A {{name}} is a {kind}', { name: 'x', kind: 'code' }), 'A {name} is a code');
  });
  it('writes numbers plainly and handles a name used twice', () => {
    assert.equal(interpolate('{n} of {n}', { n: 1234 }), '1234 of 1234');
  });
  it('does not look up names on the object prototype', () => {
    assert.equal(interpolate('{toString}', {}), '{toString}');
  });
});

describe('placeholders and tags', () => {
  it('lists blanks and marks in order of first use', () => {
    assert.deepEqual(placeholdersOf('{a} and {b} and {a}'), ['a', 'b']);
    assert.deepEqual(tagsOf('Press <kbd>Enter</kbd> to <b>go</b>, <kbd>now</kbd><br/>'), ['kbd', 'b', 'br']);
    assert.deepEqual(placeholdersOf('{{not}} a blank'), []);
  });
});

describe('t, tc, N_', () => {
  it('shows the English text when there is no translation', () => {
    assert.equal(t('Save'), 'Save');
    assert.equal(t('Saved {n} changes', { n: 3 }), 'Saved 3 changes');
    assert.equal(N_('Home'), 'Home');
  });
  it('uses a translation, and an empty one counts as none', () => {
    setLocaleData('xx', { messages: { Save: 'Guardar', Cancel: '' } });
    assert.equal(currentLocale(), 'xx');
    assert.equal(t('Save'), 'Guardar');
    assert.equal(t('Cancel'), 'Cancel');
    assert.equal(t('Other'), 'Other');
  });
  it('keeps translations of the same English words apart by their note', () => {
    setLocaleData('xx', { messages: { Open: 'Abierto' }, contexts: { command: { Open: 'Abrir' } } });
    assert.equal(t('Open'), 'Abierto');
    assert.equal(tc('command', 'Open'), 'Abrir');
    assert.equal(tc('nothing', 'Open'), 'Open');
  });
  it('fills blanks of a translation', () => {
    setLocaleData('xx', { messages: { 'Delete {name}?': '¿Borrar {name}?' } });
    assert.equal(t('Delete {name}?', { name: 'A' }), '¿Borrar A?');
  });
});

describe('tn', () => {
  const codes = { one: '{count} code', other: '{count} codes' };
  it('chooses the English form by the count and groups the digits', () => {
    assert.equal(tn(0, codes), '0 codes');
    assert.equal(tn(1, codes), '1 code');
    assert.equal(tn(2, codes), '2 codes');
    assert.equal(tn(1234, codes), '1,234 codes');
  });
  it('chooses a translated form by the translated language\'s rules', () => {
    setLocaleData('ru', { messages: { '{count} codes': { one: '{count} код', few: '{count} кода', many: '{count} кодов', other: '{count} кода' } } });
    assert.equal(tn(1, codes), '1 код');
    assert.equal(tn(3, codes), '3 кода');
    assert.equal(tn(5, codes), '5 кодов');
    assert.equal(tn(21, codes), '21 код');
  });
  it('lets a language with one form give it as a plain text', () => {
    setLocaleData('ja', { messages: { '{count} codes': '{count} 件のコード' } });
    assert.equal(tn(1, codes), '1 件のコード');
    assert.equal(tn(40, codes), '40 件のコード');
  });
  it('falls back to the English form when a needed category is missing', () => {
    setLocaleData('ru', { messages: { '{count} codes': { one: '{count} код', other: '{count} кодов' } } });
    assert.equal(tn(3, codes), '3 кодов');
  });
  it('fills other blanks too', () => {
    assert.equal(tn(2, { one: '{count} hit in {where}', other: '{count} hits in {where}' }, { where: 'A' }), '2 hits in A');
  });
  it('picks forms for Arabic\'s six categories', () => {
    const rules = new Intl.PluralRules('ar');
    const forms = { zero: 'z', one: 'o', two: 't', few: 'f', many: 'm', other: 'x' };
    const english = { one: 'one', other: 'other' };
    assert.equal(pickPlural(0, english, forms, rules), 'z');
    assert.equal(pickPlural(1, english, forms, rules), 'o');
    assert.equal(pickPlural(2, english, forms, rules), 't');
    assert.equal(pickPlural(5, english, forms, rules), 'f');
    assert.equal(pickPlural(15, english, forms, rules), 'm');
    assert.equal(pickPlural(100, english, forms, rules), 'x');
  });
});

describe('the test language', () => {
  it('bends letters, keeps blanks and marks, frames the text, and pads it', () => {
    const p = pseudo('Delete {name} <b>now</b>');
    assert.ok(p.startsWith('⟦') && p.endsWith('⟧'));
    assert.ok(p.includes('{name}'));
    assert.ok(p.includes('<b>') && p.includes('</b>'));
    assert.ok(!/[A-Za-z]{3}/.test(p.replace('{name}', '').replace(/<\/?b>/g, '')), p);
    assert.ok(p.includes('~'));
  });
  it('is used for every message, with the blanks filled', () => {
    setLocaleData('en-XA', null);
    assert.ok(t('Save').startsWith('⟦'));
    assert.ok(t('Delete {name}?', { name: 'A' }).includes('A'));
    assert.ok(tn(2, { one: '{count} code', other: '{count} codes' }).startsWith('⟦'));
    assert.ok(tn(2, { one: '{count} code', other: '{count} codes' }).includes('2'));
  });
  it('maps every letter to a different, non-ASCII one', () => {
    const lower = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ';
    const bent = pseudo(lower).slice(1, 53);
    assert.equal(Array.from(bent).length, 52);
    for (const ch of bent) assert.ok(ch.charCodeAt(0) > 127, ch);
    assert.equal(new Set(Array.from(bent)).size, 52);
  });
});

describe('rich', () => {
  it('places marked words and blanks', () => {
    const html = renderToStaticMarkup(
      createElement('p', null, richNodes('Press <kbd>Enter</kbd> to code {name}.', { kbd: (inside) => createElement('kbd', null, inside), name: 'Welcome' })),
    );
    assert.equal(html, '<p>Press <kbd>Enter</kbd> to code Welcome.</p>');
  });
  it('lets a translation reorder the marked words', () => {
    setLocaleData('xx', { messages: { 'Press <kbd>Enter</kbd> to go': 'Para ir, pulse <kbd>Enter</kbd>' } });
    const html = renderToStaticMarkup(createElement('p', null, rich('Press <kbd>Enter</kbd> to go', { kbd: (inside) => createElement('kbd', null, inside) })));
    assert.equal(html, '<p>Para ir, pulse <kbd>Enter</kbd></p>');
  });
  it('nests marks, takes element blanks, and closes marks left open', () => {
    const html = renderToStaticMarkup(
      createElement('p', null, richNodes('<b>a <i>b</i></b> {x} <b>open', { b: (c) => createElement('b', null, c), i: (c) => createElement('i', null, c), x: createElement('span', null, 'X') })),
    );
    assert.equal(html, '<p><b>a <i>b</i></b> <span>X</span> <b>open</b></p>');
  });
  it('shows a stray closing mark as written', () => {
    const html = renderToStaticMarkup(createElement('p', null, richNodes('a </b> b', {})));
    assert.equal(html, '<p>a &lt;/b&gt; b</p>');
  });
});

describe('formatting', () => {
  it('writes numbers as US English at first', () => {
    assert.equal(num(1234567), '1,234,567');
    assert.equal(num(0.5, { style: 'percent' }), '50%');
  });
  it('writes numbers and lists as the chosen language does', () => {
    setLocaleData('de', null);
    assert.equal(num(1234567), '1.234.567');
    assert.equal(list(['a', 'b', 'c']), 'a, b und c');
  });
  it('writes lists in English with the serial comma', () => {
    assert.equal(list(['a', 'b', 'c']), 'a, b, and c');
    assert.equal(list(['a', 'b']), 'a and b');
    assert.equal(list(['a', 'b'], 'disjunction'), 'a or b');
  });
  it('writes a date the way the language does', () => {
    const d = new Date(Date.UTC(2026, 10, 7, 12, 0, 0));
    assert.equal(dateTime(d, { timeZone: 'UTC', year: 'numeric', month: 'long', day: 'numeric' }), 'November 7, 2026');
  });
  it('asks for US English formats for the English of this program', () => {
    assert.equal(formatTag('en'), 'en-US');
    assert.equal(formatTag('en-XA'), 'en-US');
    assert.equal(formatTag('fr'), 'fr');
  });
});

describe('resolveLocale', () => {
  const have = ['en', 'fr', 'pt-BR'];
  it('keeps a choice that exists and falls back to English for one that does not', () => {
    assert.equal(resolveLocale('fr', have, ['de']), 'fr');
    assert.equal(resolveLocale('xx', have, ['fr']), 'en');
  });
  it('follows the computer\'s languages in order when the choice is automatic', () => {
    assert.equal(resolveLocale('auto', have, ['de-DE', 'fr-CA', 'en-US']), 'fr');
    assert.equal(resolveLocale('auto', have, ['pt-BR']), 'pt-BR');
    assert.equal(resolveLocale('auto', have, ['pt-PT']), 'pt-BR');
    assert.equal(resolveLocale('auto', have, ['de']), 'en');
    assert.equal(resolveLocale('auto', have, []), 'en');
  });
});

describe('lookup', () => {
  it('does not find a message among the things every object has', () => {
    assert.equal(lookup({ messages: {} }, 'constructor'), undefined);
    assert.equal(lookup({ messages: {} }, 'toString'), undefined);
    assert.equal(lookup({ messages: {}, contexts: {} }, 'x', 'constructor'), undefined);
    assert.equal(lookup({ messages: JSON.parse('{"__proto__":"x"}') }, '__proto__'), 'x');
  });
  it('ignores empty translations', () => {
    assert.equal(lookup({ messages: { a: '' } }, 'a'), undefined);
    assert.equal(lookup({ messages: { a: 'b' } }, 'a'), 'b');
    assert.equal(lookup({ messages: {}, contexts: { c: { a: 'b' } } }, 'a', 'c'), 'b');
  });
});
