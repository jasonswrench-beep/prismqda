// Checks the language catalog.
//
//   node scripts/i18n/check.mjs
//
//   - every message in the source is written out in full (a call that looks a message up by a variable is listed in allow.json);
//   - src/i18n/messages.json and src/i18n/engine-messages.json are what the source says;
//   - no string that looks like words is outside the catalog (scripts/i18n/audit.mjs);
//   - each language file in src/i18n/locales is well made: its blanks and marks match the English, its count forms are the ones
//     its language needs, and it names no message that is gone.
// A missing translation is not an error; it is shown in English. The checks print how much of each language is done.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { SRC, UI_ROOT, sourceFiles } from './ast.mjs';
import { auditFile, loadAllow } from './audit.mjs';
import { ENGINE_FILE, extractEngine, serializeEngine } from './engine-extract.mjs';
import { MESSAGES_FILE, extractAll, infoOf, serialize, toFile } from './extract.mjs';

const LOCALES_DIR = path.join(SRC, 'i18n', 'locales');
const MANIFEST = path.join(SRC, 'i18n', 'manifest.json');
const CATEGORIES = new Set(['zero', 'one', 'two', 'few', 'many', 'other']);

/** Checks one language file against the source messages. Returns { errors, warnings, total, done }. */
export function checkLocale(file, source) {
  const errors = [];
  const warnings = [];
  let data;
  try {
    data = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (e) {
    return { errors: [`${path.basename(file)}: not valid JSON (${e.message})`], warnings, total: 0, done: 0 };
  }
  const code = path.basename(file, '.json');
  const name = path.basename(file);
  if (data.locale !== code) errors.push(`${name}: "locale" says ${JSON.stringify(data.locale)}, but the file is for ${code}`);
  if (typeof data.name !== 'string' || !data.name.trim()) errors.push(`${name}: "name" (the language's name, in that language) is missing`);
  if (data.dir !== undefined && data.dir !== 'ltr' && data.dir !== 'rtl') errors.push(`${name}: "dir" is ltr or rtl`);
  let needed;
  try {
    needed = new Intl.PluralRules(code).resolvedOptions().pluralCategories;
  } catch {
    errors.push(`${name}: ${code} is not a language code the browser knows`);
    needed = ['one', 'other'];
  }

  const byKey = new Map();
  for (const m of source.messages) byKey.set((m.context ? m.context + '\u0004' : '') + m.id, m);

  const sameSet = (a, b) => a.length === b.length && a.every((x) => b.includes(x));
  let done = 0;
  const visit = (id, context, value) => {
    const key = (context ? context + '\u0004' : '') + id;
    const src = byKey.get(key);
    const label = context ? `[${context}] ${id}` : id;
    if (!src) {
      warnings.push(`${name}: "${label.slice(0, 70)}" is not a message of the program any more`);
      return;
    }
    const forms = typeof value === 'string' ? { other: value } : value;
    if (typeof value !== 'string' && (value === null || typeof value !== 'object' || Array.isArray(value))) {
      errors.push(`${name}: "${label.slice(0, 70)}" must be text, or an object of forms`);
      return;
    }
    const entries = Object.entries(forms);
    if (entries.every(([, v]) => v === '' || v === undefined)) return; // not translated yet
    for (const [k, v] of entries) {
      if (!CATEGORIES.has(k)) {
        errors.push(`${name}: "${label.slice(0, 70)}" has a form called ${k}; the forms are zero, one, two, few, many, other`);
        continue;
      }
      if (typeof v !== 'string') {
        errors.push(`${name}: "${label.slice(0, 70)}" form ${k} is not text`);
        continue;
      }
      if (v === '') continue;
      const info = infoOf(v);
      if (info.stray) errors.push(`${name}: "${label.slice(0, 70)}" form ${k} has a single brace; { and } are written {{ and }}`);
      const allowed = [...src.blanks ?? [], ...(src.forms ? ['count'] : [])];
      for (const p of info.placeholders) if (!allowed.includes(p)) errors.push(`${name}: "${label.slice(0, 70)}" form ${k} uses {${p}}, which the English does not have`);
      if (!src.forms) {
        // a message without a count must use every blank of the English
        for (const p of src.blanks ?? []) if (!info.placeholders.includes(p)) errors.push(`${name}: "${label.slice(0, 70)}" is missing {${p}}`);
      }
      if (!sameSet(info.tags, src.marks ?? [])) errors.push(`${name}: "${label.slice(0, 70)}" form ${k} has the marks ${JSON.stringify(info.tags)}, but the English has ${JSON.stringify(src.marks ?? [])}`);
    }
    if (src.forms) {
      if (!forms.other) errors.push(`${name}: "${label.slice(0, 70)}" needs the form "other"`);
      else {
        const lacking = needed.filter((c) => !forms[c]);
        if (lacking.length && typeof value !== 'string') warnings.push(`${name}: "${label.slice(0, 70)}" lacks the forms ${lacking.join(', ')} that ${code} needs; "other" is used for them`);
      }
    }
    done++;
  };
  for (const [id, value] of Object.entries(data.messages ?? {})) visit(id, null, value);
  for (const [ctx, table] of Object.entries(data.contexts ?? {})) for (const [id, value] of Object.entries(table)) visit(id, ctx, value);
  return { errors, warnings, total: source.messages.length, done };
}

export function runChecks() {
  const errors = [];
  const warnings = [];
  const lines = [];
  const allow = loadAllow();

  const { messages, dynamic, problems } = extractAll();
  for (const p of problems) errors.push(`${p.where}: ${p.msg}`);

  const okDynamic = new Set((allow.dynamic ?? []).map((d) => `${d.file}\u0001${d.code}`));
  for (const d of dynamic) {
    const file = d.where.replace(/:\d+$/, '');
    if (!okDynamic.has(`${file}\u0001${d.code}`)) errors.push(`${d.where}: ${d.code} looks a message up by a variable; write the message out, or mark the variable's value with N_() and list this call in scripts/i18n/allow.json`);
  }

  const engineWant = serializeEngine(extractEngine());
  // A file checked out on Windows with its line endings changed (git's autocrlf) is the same file, so the endings are not compared.
  const engineHave = fs.existsSync(ENGINE_FILE) ? fs.readFileSync(ENGINE_FILE, 'utf8').replace(/\r\n/g, '\n') : '';
  if (engineWant !== engineHave) errors.push('src/i18n/engine-messages.json is not what the Rust source says. Run: npm run i18n:engine');

  const want = serialize(toFile(messages));
  const have = fs.existsSync(MESSAGES_FILE) ? fs.readFileSync(MESSAGES_FILE, 'utf8').replace(/\r\n/g, '\n') : '';
  if (want !== have) errors.push('src/i18n/messages.json is not what the source says. Run: npm run i18n:extract');

  let left = 0;
  for (const rel of sourceFiles()) {
    const allowed = new Set((allow.entries ?? []).filter((e) => e.file === rel).map((e) => e.text));
    for (const f of auditFile(rel)) {
      if (allowed.has(f.text)) continue;
      left++;
      if (left <= 25) errors.push(`${rel}:${f.line}: ${JSON.stringify(f.text).slice(0, 100)} is words outside the catalog; write t('...') around it (or list it in scripts/i18n/allow.json if it is not words for the person)`);
    }
  }
  if (left > 25) errors.push(`... and ${left - 25} more strings outside the catalog (node scripts/i18n/audit.mjs lists them)`);

  // calls that sort or write by the language of the computer instead of the language of the window
  for (const rel of sourceFiles()) {
    const text = fs.readFileSync(path.join(SRC, rel), 'utf8');
    const lines = text.split(/\r?\n/);
    lines.forEach((line, i) => {
      if (/\.(localeCompare|toLocaleString|toLocaleDateString|toLocaleTimeString)\(/.test(line)) errors.push(`${rel}:${i + 1}: sorts or writes by the language of the computer; use compareText, num, or dateTime from the i18n module`);
    });
  }

  // the languages
  const manifest = JSON.parse(fs.readFileSync(MANIFEST, 'utf8'));
  const source = toFile(messages);
  const files = fs.existsSync(LOCALES_DIR) ? fs.readdirSync(LOCALES_DIR).filter((f) => f.endsWith('.json')).sort() : [];
  for (const l of manifest.locales) {
    if (l.code !== 'en' && !files.includes(`${l.code}.json`)) errors.push(`manifest.json lists ${l.code}, but src/i18n/locales/${l.code}.json does not exist`);
  }
  for (const f of files) {
    const code = path.basename(f, '.json');
    if (!manifest.locales.some((l) => l.code === code)) errors.push(`src/i18n/locales/${f} is not listed in src/i18n/manifest.json`);
    const r = checkLocale(path.join(LOCALES_DIR, f), source);
    errors.push(...r.errors);
    warnings.push(...r.warnings);
    lines.push(`${code}: ${r.done} of ${r.total} messages translated (${r.total ? Math.round((100 * r.done) / r.total) : 0}%)`);
  }

  lines.unshift(`${messages.size} messages (${[...messages.values()].filter((m) => m.engine).length} from the engine), ${files.length} language file(s)`);
  return { errors, warnings, lines };
}

function main() {
  const { errors, warnings, lines } = runChecks();
  for (const l of lines) console.log(l);
  for (const w of warnings.slice(0, 30)) console.log(`warning: ${w}`);
  if (warnings.length > 30) console.log(`warning: ... and ${warnings.length - 30} more`);
  for (const e of errors) console.log(`ERROR: ${e}`);
  if (errors.length) {
    console.log(`${errors.length} error(s).`);
    process.exit(1);
  }
  console.log('The language catalog is in order.');
}

void UI_ROOT;
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
