import assert from 'node:assert/strict';
import { test } from 'node:test';
import { defaultRuleKind, fileNameOf, groupRules, nextLabel, parseTerms, planGroupEdit, replacementProblem } from './deid';
import type { DeidRule } from '../types';

const rule = (id: number, term: string, replacement: string, extra: Partial<DeidRule> = {}): DeidRule => ({
  id,
  term,
  replacement,
  kind: 'pseudonym',
  match_case: false,
  whole_word: true,
  note: '',
  created_at: '',
  ...extra,
});

test('the words to find are taken one to a line, tidied, and without repeats', () => {
  assert.deepEqual(parseTerms('Sarah Jones\n  Sarah  \n\nsarah jones\nBoston; Maple   Station'), ['Sarah Jones', 'Sarah', 'Boston', 'Maple Station']);
  assert.deepEqual(parseTerms('  \n '), []);
});

test('what takes their place cannot be empty or break a spreadsheet', () => {
  assert.match(replacementProblem('  ') ?? '', /Say what takes/);
  assert.match(replacementProblem('Smith, J.') ?? '', /comma/);
  assert.match(replacementProblem('say "hi"') ?? '', /comma/);
  assert.equal(replacementProblem('Participant 3'), null);
  assert.equal(replacementProblem('[city]'), null);
});

test('a label is numbered after the highest number already used', () => {
  const rules = [rule(1, 'a', 'Participant 3'), rule(2, 'b', 'Participant 7'), rule(3, 'c', 'Person 2'), rule(4, 'd', '[email]')];
  assert.equal(nextLabel(rules, 'Participant '), 'Participant 8');
  assert.equal(nextLabel(rules, 'Person '), 'Person 3');
  assert.equal(nextLabel([], 'Person '), 'Person 1');
});

test('rules that give the same label with the same settings are shown as one line', () => {
  const rules = [rule(1, 'Sarah Jones', 'P1'), rule(2, 'Sarah', 'P1'), rule(3, 'Boston', 'City A', { match_case: true }), rule(4, 'Austin', 'City A', { match_case: true }), rule(5, 'boston', 'City A')];
  assert.deepEqual(groupRules(rules).map((g) => g.rules.map((r) => r.id)), [[1, 2], [3, 4], [5]]);
});

test('a change to a line is planned as words taken out, words added, and settings changed', () => {
  const g = groupRules([rule(1, 'Sarah Jones', 'P1'), rule(2, 'Sarah', 'P1')])[0];
  const same = planGroupEdit(g, { terms: ['sarah jones', 'Sarah'], replacement: 'P1', kind: 'pseudonym', match_case: false, whole_word: true });
  assert.deepEqual(same, { remove: [], add: [], update: [] });
  const plan = planGroupEdit(g, { terms: ['Sarah', 'Ms Jones'], replacement: 'P2', kind: 'redact', match_case: true, whole_word: true });
  assert.deepEqual(plan.remove, [1]);
  assert.deepEqual(plan.add, ['Ms Jones']);
  assert.deepEqual(plan.update, [{ id: 2, replacement: 'P2', kind: 'redact', match_case: true, whole_word: true }]);
});

test('addresses and numbers are redacted and names are given pseudonyms', () => {
  assert.equal(defaultRuleKind('email'), 'redact');
  assert.equal(defaultRuleKind('phone'), 'redact');
  assert.equal(defaultRuleKind('name'), 'pseudonym');
  assert.equal(defaultRuleKind('speaker'), 'pseudonym');
});

test('a file is named by its last part', () => {
  assert.equal(fileNameOf('C:\\Keys\\key.csv'), 'key.csv');
  assert.equal(fileNameOf('/home/a/key.csv'), 'key.csv');
  assert.equal(fileNameOf('key.csv'), 'key.csv');
});
