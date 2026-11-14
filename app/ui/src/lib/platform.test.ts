import assert from 'node:assert/strict';
import { test } from 'node:test';
import { isApplePlatform, shortcutLabel } from './platform';

test('shortcuts are named the way the computer calls its keys', () => {
  assert.equal(shortcutLabel('k', false), 'Ctrl+K');
  assert.equal(shortcutLabel('s', true), '⌘S');
  assert.equal(shortcutLabel('d', false), 'Ctrl+D');
});

test('a Mac is recognised from its platform or its user agent', () => {
  assert.equal(isApplePlatform({ platform: 'MacIntel' }), true);
  assert.equal(isApplePlatform({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' }), true);
  assert.equal(isApplePlatform({ platform: 'Win32', userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' }), false);
  assert.equal(isApplePlatform({}), false);
});
