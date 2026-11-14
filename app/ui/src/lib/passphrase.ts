// Passphrases for encrypted projects (decisions 137 and 157): the small checks the screens make before they ask the engine.
// The engine checks the same things again; these only say so earlier, in words.

import { t } from '../i18n';

/** The fewest characters a new passphrase can have. A longer one is much safer. */
export const MIN_PASSPHRASE_CHARS = 8;

const characters = (s: string): number => [...s].length;

/** Why two typed passphrases cannot be used to encrypt a project, or null when they can. */
export function passphraseProblem(pass: string, again: string): string | null {
  if (pass.trim() === '') return t('Type a passphrase.');
  if (/^x'/i.test(pass)) return t("A passphrase cannot begin with x' (it would be read as a raw key). Begin it another way.");
  if (characters(pass) < MIN_PASSPHRASE_CHARS) return t('A passphrase needs at least {min} characters.', { min: MIN_PASSPHRASE_CHARS });
  if (pass !== again) return t('The two passphrases are not the same.');
  return null;
}

/** A line under the field while a passphrase is typed. It counts characters and nothing else: it cannot tell a good passphrase from a bad one. */
export function passphraseHint(pass: string): string {
  const n = characters(pass);
  if (n === 0) return '';
  if (n < MIN_PASSPHRASE_CHARS) return t('Too short: at least {min} characters.', { min: MIN_PASSPHRASE_CHARS });
  if (n < 12) return t('Short. Twelve or more characters, or four unrelated words, is much safer.');
  return t('A good length.');
}

/** The words that open a passphrase dialog: the project's file name without its folder or its ending. */
export function projectLabel(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  return name.replace(/\.pqda$/i, '');
}
