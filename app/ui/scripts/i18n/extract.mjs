// Collects every message in the interface's source: the first argument of t(), tc(), tn(), N_(), and rich().
//
//   node scripts/i18n/extract.mjs           print how many there are, and any problem
//   node scripts/i18n/extract.mjs --write   write src/i18n/messages.json (the list translators start from)
//   node scripts/i18n/extract.mjs --list    print every message

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { SRC, lineIndex, parseSource, readSource, sourceFiles, walk } from './ast.mjs';
import { ENGINE_FILE } from './engine-extract.mjs';

export const MESSAGES_FILE = path.join(SRC, 'i18n', 'messages.json');
const TOKEN = /\{\{|\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}|<(\/?)([a-z][a-z0-9]*)(\s*\/)?>/g;

function constString(node) {
  if (!node) return null;
  if (node.type === 'Literal' && typeof node.value === 'string') return node.value;
  if (node.type === 'TemplateLiteral' && node.expressions.length === 0) return node.quasis[0].value.cooked ?? node.quasis[0].value.raw;
  if (node.type === 'BinaryExpression' && node.operator === '+') {
    const a = constString(node.left);
    const b = constString(node.right);
    return a !== null && b !== null ? a + b : null;
  }
  return null;
}

export function infoOf(template) {
  const placeholders = [];
  const tags = [];
  for (const m of template.matchAll(TOKEN)) {
    if (m[1] && !placeholders.includes(m[1])) placeholders.push(m[1]);
    if (m[3] && !tags.includes(m[3])) tags.push(m[3]);
  }
  // single braces that are not a blank
  const stray = template.replace(/\{\{|\}\}/g, '').replace(/\{[A-Za-z_][A-Za-z0-9_]*\}/g, '').match(/[{}]/);
  return { placeholders, tags, stray: Boolean(stray) };
}

function calleeName(call) {
  const c = call.callee;
  return c && c.type === 'Identifier' ? c.name : null;
}

function objectKeys(node) {
  if (!node || node.type !== 'ObjectExpression') return null;
  const keys = [];
  for (const p of node.properties) {
    if (p.type !== 'Property' || p.computed) return null;
    keys.push(p.key.type === 'Identifier' ? p.key.name : String(p.key.value));
  }
  return keys;
}

/** Everything the source says: { messages: Map(key -> message), dynamic: [], problems: [] } */
export function extractAll(files = sourceFiles()) {
  const messages = new Map();
  const dynamic = [];
  const problems = [];

  const add = (rel, line, m) => {
    const key = (m.context ? m.context + '\u0004' : '') + m.id;
    let e = messages.get(key);
    if (!e) {
      e = { id: m.id, context: m.context ?? null, plural: false, forms: null, placeholders: [], tags: [], files: [] };
      messages.set(key, e);
    }
    if (m.forms) {
      e.plural = true;
      e.forms = { ...(e.forms ?? {}), ...m.forms };
    }
    for (const p of m.placeholders) if (!e.placeholders.includes(p)) e.placeholders.push(p);
    for (const t of m.tags) if (!e.tags.includes(t)) e.tags.push(t);
    if (!e.files.includes(rel)) e.files.push(rel);
    e.where = e.where ?? `${rel}:${line}`;
  };

  for (const rel of files) {
    const text = readSource(rel);
    const ast = parseSource(text, rel);
    const lineAt = lineIndex(text);
    walk(ast, (node) => {
      if (node.type !== 'CallExpression') return true;
      const name = calleeName(node);
      if (!name || !['t', 'tc', 'tn', 'N_', 'rich'].includes(name)) return true;
      const line = lineAt(node.start);
      const where = `${rel}:${line}`;
      const argc = node.arguments;
      const check = (template, argsNode, extraBlanks = []) => {
        const info = infoOf(template);
        if (info.stray) problems.push({ where, msg: `a single brace in a message: ${JSON.stringify(template)}` });
        const keys = objectKeys(argsNode);
        if (name !== 'rich' && info.tags.length) problems.push({ where, msg: `marks in a message that is not shown with rich(): ${JSON.stringify(template)}` });
        if (keys) {
          for (const p of info.placeholders) if (!keys.includes(p) && !extraBlanks.includes(p)) problems.push({ where, msg: `the blank {${p}} has no value: ${JSON.stringify(template)}` });
          for (const k of keys) if (!info.placeholders.includes(k) && !info.tags.includes(k) && name !== 'tn') problems.push({ where, msg: `the value ${k} is not used by the message: ${JSON.stringify(template)}` });
        } else if (argsNode === undefined) {
          for (const p of info.placeholders) if (!extraBlanks.includes(p)) problems.push({ where, msg: `the blank {${p}} has no value: ${JSON.stringify(template)}` });
        }
        return info;
      };
      if (name === 't' || name === 'N_') {
        const id = constString(argc[0]);
        if (id === null) {
          dynamic.push({ where, name, code: text.slice(node.start, Math.min(node.end, node.start + 80)) });
          return true;
        }
        const info = name === 't' ? check(id, argc[1]) : infoOf(id);
        add(rel, line, { id, ...info });
      } else if (name === 'tc') {
        const ctx = constString(argc[0]);
        const id = constString(argc[1]);
        if (ctx === null || id === null) {
          dynamic.push({ where, name, code: text.slice(node.start, Math.min(node.end, node.start + 80)) });
          return true;
        }
        const info = check(id, argc[2]);
        add(rel, line, { id, context: ctx, ...info });
      } else if (name === 'rich') {
        const id = constString(argc[0]);
        if (id === null) {
          dynamic.push({ where, name, code: text.slice(node.start, Math.min(node.end, node.start + 80)) });
          return true;
        }
        const info = check(id, argc[1]);
        add(rel, line, { id, ...info });
      } else if (name === 'tn') {
        const forms = argc[1];
        if (!forms || forms.type !== 'ObjectExpression') {
          dynamic.push({ where, name, code: text.slice(node.start, Math.min(node.end, node.start + 80)) });
          return true;
        }
        const got = {};
        for (const p of forms.properties) {
          if (p.type !== 'Property' || p.computed) continue;
          const k = p.key.type === 'Identifier' ? p.key.name : String(p.key.value);
          const v = constString(p.value);
          if (v === null) {
            problems.push({ where, msg: `a form of a count message is not plain text (${k})` });
            return true;
          }
          got[k] = v;
        }
        if (typeof got.other !== 'string') {
          problems.push({ where, msg: 'a count message has no "other" form' });
          return true;
        }
        const all = { placeholders: [], tags: [] };
        for (const v of Object.values(got)) {
          const info = infoOf(v);
          for (const p of info.placeholders) if (!all.placeholders.includes(p)) all.placeholders.push(p);
          for (const t of info.tags) if (!all.tags.includes(t)) all.tags.push(t);
          check(v, argc[2], ['count']);
        }
        add(rel, line, { id: got.other, forms: got, placeholders: all.placeholders, tags: all.tags });
      }
      return true;
    });
  }

  // the sentences the engine says (collected from the Rust source into engine-messages.json)
  if (fs.existsSync(ENGINE_FILE)) {
    const engine = JSON.parse(fs.readFileSync(ENGINE_FILE, 'utf8'));
    for (const m of engine.messages) {
      const info = infoOf(m.template);
      const key = m.template;
      let e = messages.get(key);
      if (!e) {
        e = { id: m.template, context: null, plural: false, forms: null, placeholders: [], tags: [], files: [] };
        messages.set(key, e);
      }
      e.engine = true;
      for (const p of info.placeholders) if (!e.placeholders.includes(p)) e.placeholders.push(p);
      for (const f of m.files) if (!e.files.includes(f)) e.files.push(f);
      e.where = e.where ?? m.files[0];
    }
  }
  return { messages, dynamic, problems };
}

/** The shape written to messages.json. */
export function toFile(messages) {
  const list = [...messages.values()]
    .map((e) => {
      const out = { id: e.id };
      if (e.context) out.context = e.context;
      if (e.plural) out.forms = Object.fromEntries(Object.entries(e.forms).sort(([a], [b]) => a.localeCompare(b)));
      if (e.placeholders.length) out.blanks = e.placeholders.filter((p) => !(e.plural && p === 'count'));
      if (e.plural && !out.blanks?.length) delete out.blanks;
      if (e.tags.length) out.marks = e.tags;
      if (e.engine) out.engine = true;
      out.files = [...e.files].sort();
      return out;
    })
    .sort((a, b) => a.id.localeCompare(b.id, 'en') || (a.context ?? '').localeCompare(b.context ?? ''));
  return { version: 1, count: list.length, messages: list };
}

export function serialize(file) {
  return JSON.stringify(file, null, 2) + '\n';
}

function main() {
  const args = process.argv.slice(2);
  const { messages, dynamic, problems } = extractAll();
  if (args.includes('--list')) for (const e of messages.values()) console.log(`${e.where}  ${e.context ? `[${e.context}] ` : ''}${JSON.stringify(e.id)}${e.plural ? '  (count)' : ''}`);
  for (const p of problems) console.log(`PROBLEM ${p.where}: ${p.msg}`);
  if (args.includes('--dynamic')) for (const d of dynamic) console.log(`DYNAMIC ${d.where}: ${d.code}`);
  console.log(`${messages.size} messages, ${dynamic.length} calls with words that are not written out, ${problems.length} problems.`);
  if (args.includes('--write')) {
    fs.writeFileSync(MESSAGES_FILE, serialize(toFile(messages)));
    console.log(`Wrote ${path.relative(process.cwd(), MESSAGES_FILE)}.`);
  }
  if (problems.length > 0) process.exitCode = 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
