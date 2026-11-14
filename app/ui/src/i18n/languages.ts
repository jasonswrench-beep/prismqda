// The standard list of languages a translation can be for. A translation file says which language it is by one of these codes, so
// the list of installed languages, the language of the page, the way numbers are written, and the plural forms are all known from
// the code. The names come from the browser (Intl.DisplayNames), so no list of translated language names is kept here.

/**
 * About seventy languages and regional variants, by BCP 47 code. A variant is listed where speakers commonly want their own
 * (Brazilian Portuguese, Latin American Spanish, the two Chinese scripts). English is built in, and is not in the list as a
 * language to translate into, except British English, for those who want another spelling.
 */
export const STANDARD_LANGUAGES: readonly string[] = [
  'af', 'am', 'ar', 'bg', 'bn', 'ca', 'cs', 'cy', 'da', 'de', 'el', 'en-GB', 'es', 'es-419', 'et', 'eu', 'fa', 'fi', 'fil', 'fr',
  'fr-CA', 'ga', 'gl', 'gu', 'ha', 'he', 'hi', 'hr', 'hu', 'hy', 'id', 'is', 'it', 'ja', 'ka', 'kk', 'km', 'kn', 'ko', 'lt',
  'lv', 'mi', 'ml', 'mn', 'mr', 'ms', 'my', 'nb', 'ne', 'nl', 'pa', 'pl', 'ps', 'pt-BR', 'pt-PT', 'ro', 'ru', 'si', 'sk', 'sl',
  'sq', 'sr', 'sv', 'sw', 'ta', 'te', 'th', 'tr', 'uk', 'ur', 'uz', 'vi', 'xh', 'yo', 'zh-Hans', 'zh-Hant', 'zu',
];

const RIGHT_TO_LEFT = new Set(['ar', 'he', 'fa', 'ur', 'ps', 'sd', 'ug', 'yi', 'dv', 'ckb']);

/** Whether the text of a language runs from right to left. */
export function directionOf(code: string): 'ltr' | 'rtl' {
  try {
    const loc = new Intl.Locale(code) as Intl.Locale & { getTextInfo?: () => { direction: string }; textInfo?: { direction: string } };
    const info = typeof loc.getTextInfo === 'function' ? loc.getTextInfo() : loc.textInfo;
    if (info && (info.direction === 'rtl' || info.direction === 'ltr')) return info.direction;
  } catch {
    // fall through to the list
  }
  return RIGHT_TO_LEFT.has(code.split('-')[0]?.toLowerCase() ?? '') ? 'rtl' : 'ltr';
}

/** The plural forms a language needs, in the order zero, one, two, few, many, other. */
export function pluralCategoriesOf(code: string): Intl.LDMLPluralRule[] {
  const order: Intl.LDMLPluralRule[] = ['zero', 'one', 'two', 'few', 'many', 'other'];
  try {
    const have = new Intl.PluralRules(code).resolvedOptions().pluralCategories;
    return order.filter((c) => have.includes(c));
  } catch {
    return ['one', 'other'];
  }
}

function displayName(code: string, inLocale: string): string {
  try {
    const name = new Intl.DisplayNames([inLocale], { type: 'language', languageDisplay: 'standard' }).of(code);
    return name && name !== code ? name : code;
  } catch {
    return code;
  }
}

/** The name of a language in that language ("français"), with a capital first letter where the language has capitals. */
export function nativeName(code: string): string {
  const name = displayName(code, code);
  return name.charAt(0).toLocaleUpperCase(code) + name.slice(1);
}

/** The name of a language in English ("French (Canada)"). */
export function englishName(code: string): string {
  return displayName(code, 'en');
}

export interface LanguageEntry {
  code: string;
  native: string;
  english: string;
  dir: 'ltr' | 'rtl';
}

/** The list, with names, sorted by the English name. */
export function languageEntries(): LanguageEntry[] {
  return STANDARD_LANGUAGES.map((code) => ({ code, native: nativeName(code), english: englishName(code), dir: directionOf(code) })).sort((a, b) => a.english.localeCompare(b.english, 'en'));
}

/** True for a code written like a language tag (fr, pt-BR, zh-Hans) that the browser accepts. */
export function isLanguageCode(code: string): boolean {
  if (!/^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(code)) return false;
  try {
    return Intl.getCanonicalLocales(code).length === 1;
  } catch {
    return false;
  }
}

/** The code in its standard form (pt-br becomes pt-BR), or null if it is not a language code. */
export function canonicalCode(code: string): string | null {
  const trimmed = code.trim().replace('_', '-');
  if (!isLanguageCode(trimmed)) return null;
  try {
    return Intl.getCanonicalLocales(trimmed)[0] ?? null;
  } catch {
    return null;
  }
}

/** The entry of the standard list that a code stands for: the code itself, or else the same language without its region. */
export function standardCodeFor(code: string): string | null {
  const c = canonicalCode(code);
  if (!c) return null;
  if (STANDARD_LANGUAGES.includes(c)) return c;
  const base = c.split('-')[0] as string;
  return STANDARD_LANGUAGES.includes(base) ? base : null;
}
