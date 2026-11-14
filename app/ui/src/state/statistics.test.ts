import assert from 'node:assert/strict';
import test from 'node:test';
import { DEFAULT_SETTINGS } from '../lib/settings';
import { analysisDefaults } from './statistics';

test('the Statistics page begins with the analysis choices of the settings', () => {
  const usual = analysisDefaults(DEFAULT_SETTINGS);
  assert.equal(usual.level, 0.95);
  assert.equal(usual.resamples, 2000);
  assert.equal(usual.posthoc, '', 'Holm is an adjustment of the p-values, not one of the three tests for pairs');
  assert.equal(usual.adjustment, 'holm');
  assert.match(usual.seed ?? '', /^\d+$/, 'a new seed each time, shown as a whole number');

  const chosen = analysisDefaults({ ...DEFAULT_SETTINGS, confidenceLevel: 0.99, bootstrapResamples: 5000, pairMethod: 'games_howell', fixedSeed: 17 });
  assert.deepEqual([chosen.level, chosen.resamples, chosen.posthoc, chosen.adjustment, chosen.seed], [0.99, 5000, 'games_howell', 'holm', '17'], 'a test for pairs leaves Holm for the tests that have none');

  const adjusted = analysisDefaults({ ...DEFAULT_SETTINGS, pairMethod: 'benjamini_hochberg', fixedSeed: 0 });
  assert.deepEqual([adjusted.posthoc, adjusted.adjustment, adjusted.seed], ['', 'benjamini_hochberg', '0'], 'a seed of 0 is a seed');
});
