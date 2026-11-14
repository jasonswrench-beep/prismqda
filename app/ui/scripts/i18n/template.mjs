// Starts a language, or brings a language file up to date with the program.
//
//   node scripts/i18n/template.mjs fr "Français"            makes src/i18n/locales/fr.json with every message and an empty translation
//   node scripts/i18n/template.mjs ar "العربية" --rtl         the same for a language that is written from right to left
//   node scripts/i18n/template.mjs fr --update              adds the messages the program has gained since; nothing is removed
//
// An empty translation shows the English. Fill in only what has been translated by a person.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { SRC } from './ast.mjs';
import { extractAll, toFile } from './extract.mjs';

const LOCALES_DIR = path.join(SRC, 'i18n', 'locales');
const MANIFEST = path.join(SRC, 'i18n', 'manifest.json');

export function emptyEntry(msg, categories) {
  if (!msg.forms) return '';
  return Object.fromEntries(categories.map((c) => [c, '']));
}

/** The file's messages with the missing ones added, keeping what is there; sorted by the English text. */
export function fill(existing, source, categories) {
  const messages = { ...(existing.messages ?? {}) };
  const contexts = JSON.parse(JSON.stringify(existing.contexts ?? {}));
  let added = 0;
  for (const m of source.messages) {
    if (m.context) {
      contexts[m.context] ??= {};
      if (!(m.id in contexts[m.context])) {
        contexts[m.context][m.id] = emptyEntry(m, categories);
        added++;
      }
    } else if (!(m.id in messages)) {
      messages[m.id] = emptyEntry(m, categories);
      added++;
    }
  }
  const sortKeys = (o) => Object.fromEntries(Object.entries(o).sort(([a], [b]) => a.localeCompare(b, 'en')));
  const out = { ...existing, messages: sortKeys(messages) };
  if (Object.keys(contexts).length) out.contexts = Object.fromEntries(Object.entries(contexts).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => [k, sortKeys(v)]));
  return { file: out, added };
}

function main() {
  const args = process.argv.slice(2).filter((a) => !a.startsWith('--'));
  const update = process.argv.includes('--update');
  const rtl = process.argv.includes('--rtl');
  const [code, name] = args;
  if (!code || (!update && !name)) {
    console.log('usage: node scripts/i18n/template.mjs <code> "<name in that language>" [--rtl]\n       node scripts/i18n/template.mjs <code> --update');
    process.exit(2);
  }
  if (!/^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(code) || code === 'en') {
    console.log(`${code} is not a language code to start (use a code such as fr, pt-BR, or zh-Hans; English needs no file).`);
    process.exit(2);
  }
  let categories;
  try {
    categories = new Intl.PluralRules(code).resolvedOptions().pluralCategories;
  } catch {
    console.log(`The browser does not know a language called ${code}.`);
    process.exit(2);
  }
  const { messages } = extractAll();
  const source = toFile(messages);
  const file = path.join(LOCALES_DIR, `${code}.json`);
  fs.mkdirSync(LOCALES_DIR, { recursive: true });
  let existing;
  if (fs.existsSync(file)) {
    if (!update) {
      console.log(`${path.relative(process.cwd(), file)} exists; use --update to add the new messages.`);
      process.exit(2);
    }
    existing = JSON.parse(fs.readFileSync(file, 'utf8'));
  } else {
    if (update) {
      console.log(`${code}.json does not exist.`);
      process.exit(2);
    }
    existing = { locale: code, name, dir: rtl ? 'rtl' : 'ltr', messages: {} };
  }
  const { file: out, added } = fill(existing, source, categories);
  fs.writeFileSync(file, JSON.stringify(out, null, 2) + '\n');
  if (!update) {
    const manifest = JSON.parse(fs.readFileSync(MANIFEST, 'utf8'));
    if (!manifest.locales.some((l) => l.code === code)) manifest.locales.push({ code, name, dir: rtl ? 'rtl' : 'ltr' });
    fs.writeFileSync(MANIFEST, JSON.stringify(manifest, null, 2) + '\n');
  }
  console.log(`${update ? 'Added' : 'Wrote'} ${added} message(s) ${update ? 'to' : 'in'} ${path.relative(process.cwd(), file)}. Plural forms for ${code}: ${categories.join(', ')}.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
