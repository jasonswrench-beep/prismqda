import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { engineText } from './engine';
import { setLocaleData } from './index';

afterEach(() => setLocaleData('en', null));

describe('sentences from the engine', () => {
  it('are shown as they come in English', () => {
    assert.equal(engineText("there is already a code named 'Joy' at the new place"), "there is already a code named 'Joy' at the new place");
    assert.equal(engineText(''), '');
  });

  it('are translated by their shape, with the blanks carried over', () => {
    setLocaleData('xx', { messages: { "there is already a code named '{name}' at the new place": "ya hay un código '{name}' en el lugar nuevo", 'a case needs a name': 'un caso necesita un nombre' } });
    assert.equal(engineText("there is already a code named 'Joy' at the new place"), "ya hay un código 'Joy' en el lugar nuevo");
    assert.equal(engineText('a case needs a name'), 'un caso necesita un nombre');
  });

  it('keep a name with punctuation in it whole', () => {
    setLocaleData('xx', { messages: { "there is already a code named '{name}' at the new place": "ya hay '{name}'" } });
    assert.equal(engineText("there is already a code named 'It's 5 o'clock, (really)' at the new place"), "ya hay 'It's 5 o'clock, (really)'");
  });

  it('are left as they came when the catalog has no translation or the sentence is not known', () => {
    setLocaleData('xx', { messages: {} });
    assert.equal(engineText('a case needs a name'), 'a case needs a name');
    assert.equal(engineText('something the engine never says'), 'something the engine never says');
  });

  it('go through the test language like every other message', () => {
    setLocaleData('en-XA', null);
    assert.ok(engineText('a case needs a name').startsWith('⟦'));
    assert.ok(engineText("there is already a code named 'Joy' at the new place").includes('Joy'));
  });

  it('prefer the template with more fixed words when two could fit', () => {
    setLocaleData('xx', { messages: { '{what} {id} does not exist': 'general', 'code {id} does not exist': 'specific' } });
    assert.equal(engineText('code 7 does not exist'), 'specific');
    assert.equal(engineText('memo 7 does not exist'), 'general');
  });
});
