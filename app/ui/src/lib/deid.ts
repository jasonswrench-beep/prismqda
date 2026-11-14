// Pseudonymizing and redacting: the small decisions the Privacy page makes, kept apart from the screen so they can be tested.

import { t } from '../i18n';
import type { DeidRule, DeidSuggestionKind } from '../types';

/** The words to find as typed: one on each line (a semicolon also ends one), with spaces tidied, and empty lines and repeats left out. */
export function parseTerms(text: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of text.split(/[\r\n;]+/)) {
    const term = raw.split(/\s+/).filter(Boolean).join(' ');
    if (term === '') continue;
    const key = term.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(term);
  }
  return out;
}

/** Why what was typed cannot take the place of words, or null when it can. The engine checks the same things. */
export function replacementProblem(text: string): string | null {
  const r = text.trim();
  if (r === '') return t('Say what takes the place of the words, for example Participant 3 or [city].');
  if (/[,"\t\r\n]/.test(r)) return t('What takes the place of the words cannot contain a comma, a double quote, a tab, or a line break, because it could break the columns of a spreadsheet.');
  if ([...r].length > 200) return t('What takes the place of the words can be at most 200 characters.');
  return null;
}

/** The next label of a kind: after "Participant 3" and "Participant 7" comes "Participant 8". */
export function nextLabel(rules: Pick<DeidRule, 'replacement'>[], prefix: string): string {
  let highest = 0;
  for (const r of rules) {
    if (!r.replacement.startsWith(prefix)) continue;
    const n = Number(r.replacement.slice(prefix.length).trim());
    if (Number.isInteger(n) && n > highest) highest = n;
  }
  return `${prefix}${highest + 1}`;
}

/** The rules that put the same thing in place of their words, with the same settings, shown as one line. */
export interface DeidGroup {
  key: string;
  replacement: string;
  kind: DeidRule['kind'];
  match_case: boolean;
  whole_word: boolean;
  rules: DeidRule[];
}

export function groupRules(rules: DeidRule[]): DeidGroup[] {
  const groups: DeidGroup[] = [];
  for (const r of rules) {
    const key = JSON.stringify([r.replacement, r.kind, r.match_case, r.whole_word]);
    let g = groups.find((x) => x.key === key);
    if (!g) {
      g = { key, replacement: r.replacement, kind: r.kind, match_case: r.match_case, whole_word: r.whole_word, rules: [] };
      groups.push(g);
    }
    g.rules.push(r);
  }
  return groups;
}

export interface GroupEdit {
  terms: string[];
  replacement: string;
  kind: DeidRule['kind'];
  match_case: boolean;
  whole_word: boolean;
}

/** What to ask the engine for when a line of the list is changed: take out the words that are gone, add the new ones, and change the rest. */
export interface GroupEditPlan {
  remove: number[];
  add: string[];
  update: { id: number; replacement: string; kind: DeidRule['kind']; match_case: boolean; whole_word: boolean }[];
}

export function planGroupEdit(group: DeidGroup, edit: GroupEdit): GroupEditPlan {
  const wanted = new Map(edit.terms.map((term) => [term.toLowerCase(), term]));
  const remove: number[] = [];
  const update: GroupEditPlan['update'] = [];
  const kept = new Set<string>();
  const changed = group.replacement !== edit.replacement || group.kind !== edit.kind || group.match_case !== edit.match_case || group.whole_word !== edit.whole_word;
  for (const r of group.rules) {
    const key = r.term.toLowerCase();
    if (!wanted.has(key)) {
      remove.push(r.id);
    } else {
      kept.add(key);
      if (changed) update.push({ id: r.id, replacement: edit.replacement, kind: edit.kind, match_case: edit.match_case, whole_word: edit.whole_word });
    }
  }
  return { remove, add: edit.terms.filter((term) => !kept.has(term.toLowerCase())), update };
}

/** What a suggestion is, in a few words. */
export function suggestionKindLabel(kind: DeidSuggestionKind): string {
  switch (kind) {
    case 'speaker':
      return t('Speaker label');
    case 'case':
      return t('Case name');
    case 'email':
      return t('Email address');
    case 'web':
      return t('Web address');
    case 'phone':
      return t('Phone number');
    case 'number':
      return t('Identity number');
    default:
      return t('Name');
  }
}

/** Addresses and numbers are redacted (blanked out); names are given pseudonyms. */
export function defaultRuleKind(kind: DeidSuggestionKind): DeidRule['kind'] {
  return kind === 'email' || kind === 'web' || kind === 'phone' || kind === 'number' ? 'redact' : 'pseudonym';
}

export function ruleKindLabel(kind: DeidRule['kind']): string {
  return kind === 'redact' ? t('Redact') : t('Pseudonym');
}

/** Where a message about the key file is kept short: just the file's name. */
export function fileNameOf(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}
