// The engine (written in Rust) says a few things in English that the window shows as they come: why a request was refused, a note
// under a table, a warning after an import. This tool collects those sentences from the Rust source as templates with named blanks
// ("there is already a code named '{name}' at the new place") into src/i18n/engine-messages.json. The window looks a sentence
// from the engine up in that list (see src/i18n/engine.ts), so a translator can translate it like any other message.
//
//   node scripts/i18n/engine-extract.mjs            print what was found
//   node scripts/i18n/engine-extract.mjs --write    write src/i18n/engine-messages.json
//   node scripts/i18n/engine-extract.mjs --check    exit 1 when the file is not what the source says

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { SRC, UI_ROOT } from './ast.mjs';

export const ENGINE_FILE = path.join(SRC, 'i18n', 'engine-messages.json');
const CRATES = path.resolve(UI_ROOT, '..', '..', 'crates');

function rustFiles() {
  const out = [];
  const visit = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) {
        if (e.name === 'tests' || e.name === 'target' || e.name === 'benches') continue;
        visit(full);
      } else if (e.name.endsWith('.rs')) out.push(full);
    }
  };
  for (const c of fs.readdirSync(CRATES)) {
    const src = path.join(CRATES, c, 'src');
    if (fs.existsSync(src)) visit(src);
  }
  return out.sort();
}

/** Reads a Rust string literal starting at text[i] === '"'; returns { value, end } with escapes resolved. */
export function readString(text, i) {
  let out = '';
  let j = i + 1;
  while (j < text.length) {
    const c = text[j];
    if (c === '"') return { value: out, end: j + 1 };
    if (c === '\\') {
      const n = text[j + 1];
      if (n === '\n' || n === '\r') {
        // a backslash at the end of a line joins the next line, without its leading spaces
        j += 2;
        while (j < text.length && /[ \t\r\n]/.test(text[j])) j++;
        continue;
      }
      const map = { n: '\n', t: '\t', r: '\r', '\\': '\\', '"': '"', "'": "'", '0': '\0' };
      if (n === 'u') {
        const m = text.slice(j).match(/^\\u\{([0-9a-fA-F]+)\}/);
        if (m) {
          out += String.fromCodePoint(parseInt(m[1], 16));
          j += m[0].length;
          continue;
        }
      }
      out += map[n] ?? n;
      j += 2;
      continue;
    }
    out += c;
    j++;
  }
  return null;
}

/** The text between a call's parentheses starting after text[i] === '('; returns { body, end }. */
function readParens(text, i) {
  let depth = 0;
  let j = i;
  while (j < text.length) {
    const c = text[j];
    if (c === '"') {
      const s = readString(text, j);
      if (!s) return null;
      j = s.end;
      continue;
    }
    if (c === "'" && /^'(\\.|[^\\'])'/.test(text.slice(j, j + 4))) {
      j += text.slice(j).match(/^'(\\.|[^\\'])'/)[0].length;
      continue;
    }
    if (c === '(' || c === '[' || c === '{') depth++;
    if (c === ')' || c === ']' || c === '}') {
      depth--;
      if (depth === 0) return { body: text.slice(i + 1, j), end: j + 1 };
    }
    j++;
  }
  return null;
}

/** Splits at top-level commas. */
export function splitArgs(body) {
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let j = 0; j < body.length; j++) {
    const c = body[j];
    if (c === '"') {
      const s = readString(body, j);
      if (!s) break;
      j = s.end - 1;
      continue;
    }
    if (c === '(' || c === '[' || c === '{') depth++;
    else if (c === ')' || c === ']' || c === '}') depth--;
    else if (c === ',' && depth === 0) {
      parts.push(body.slice(start, j).trim());
      start = j + 1;
    }
  }
  const last = body.slice(start).trim();
  if (last) parts.push(last);
  return parts;
}

function nameFor(expr) {
  let e = expr.trim().replace(/^&+|^\*+/, '').replace(/^mut\s+/, '');
  e = e.replace(/\.(to_string|display|to_owned|clone|as_str|to_lowercase|to_uppercase|trim|unwrap_or_default)\(\)/g, '');
  if (/\.len\(\)$/.test(e)) return 'count';
  if (/^\(?\s*\w+\s*\+\s*1\s*\)?$/.test(e)) return e.match(/\w+/)[0];
  const m = e.match(/([A-Za-z_][A-Za-z0-9_]*)\s*(?:\([^()]*\))?\s*$/);
  return m ? m[1].replace(/^(self|spec|args|a|item)$/, 'value') : 'value';
}

/** A Rust format string and its arguments become a template with named blanks. */
export function toTemplate(fmt, args) {
  const names = [];
  const used = new Set();
  const unique = (base) => {
    let n = base;
    for (let k = 2; used.has(n); k++) n = base + k;
    used.add(n);
    return n;
  };
  let position = 0;
  const out = fmt.replace(/\{\{|\}\}|\{([^{}]*)\}/g, (m, inner) => {
    if (m === '{{') return '{{';
    if (m === '}}') return '}}';
    const spec = inner.split(':')[0].trim();
    if (spec === '' || /^\d+$/.test(spec)) {
      const index = spec === '' ? position++ : Number(spec);
      const expr = args[index];
      const name = expr ? nameFor(expr) : 'value';
      names[index] = names[index] ?? unique(name);
      return `{${names[index]}}`;
    }
    const named = spec;
    if (!used.has(named)) used.add(named);
    return `{${named}}`;
  });
  return out;
}

const CALLS = [
  /(?:Error|ExportError|ImportError|ApiError)::(?:Invalid|ReadOnly|TooLarge|Io|Unsupported|Refused|NotFound|AlreadyExists)\s*\(/g,
  /ApiError::new\s*\(/g,
  /ImportWarning::new\s*\(/g,
  /"reason"\s*:\s*(?=")/g,
  /\.(?:warn|warning|note|push_warning)\s*\(/g,
];

export function extractEngine() {
  const found = new Map();
  const add = (template, file, line) => {
    const literal = template.replace(/\{[A-Za-z_][A-Za-z0-9_]*\}/g, '').replace(/\{\{|\}\}/g, '');
    if (!/[A-Za-z]{3,}/.test(literal)) return; // nothing but blanks
    if (!found.has(template)) found.set(template, { template, files: [] });
    const e = found.get(template);
    const loc = `${path.relative(CRATES, file).split(path.sep).join('/')}:${line}`;
    if (!e.files.includes(loc)) e.files.push(loc);
  };
  for (const file of rustFiles()) {
    let text = fs.readFileSync(file, 'utf8');
    const cut = text.search(/#\[cfg\(test\)\]/);
    if (cut >= 0) text = text.slice(0, cut);
    const lineOf = (offset) => text.slice(0, offset).split('\n').length;

    // #[error("...")]
    for (const m of text.matchAll(/#\[error\(\s*"/g)) {
      const start = m.index + m[0].length - 1;
      const s = readString(text, start);
      if (!s) continue;
      // arguments after the string, up to the closing )]
      const rest = text.slice(s.end);
      const close = rest.match(/^\s*(,[\s\S]*?)?\)\]/);
      const args = close && close[1] ? splitArgs(close[1].replace(/^,/, '')) : [];
      // thiserror writes {0} for a field and {} never; a field is a blank named by its place
      const named = s.value.replace(/\{(\d+)(?::[^}]*)?\}/g, (_, n) => `{${args[Number(n)] ? nameFor(args[Number(n)]) : 'value'}}`);
      add(toTemplate(named, args), file, lineOf(m.index));
    }

    for (const re of CALLS) {
      re.lastIndex = 0;
      for (const m of text.matchAll(re)) {
        let args;
        if (m[0].startsWith('"reason"')) {
          // "reason": "..." or "reason": format!(...) inside json!({ ... })
          const rest = text.slice(m.index + m[0].length);
          const lit = readString(rest, 0);
          const f = rest.match(/^format!\s*\(/);
          args = [lit ? rest.slice(0, lit.end) : f ? rest.slice(0, f[0].length - 1 + (readParens(rest, f[0].length - 1)?.end ?? 0) - (f[0].length - 1)) : ''];
        } else {
          const open = m.index + m[0].length - 1;
          const p = readParens(text, open);
          if (!p) continue;
          args = splitArgs(p.body);
        }
        // the message is the second argument for ApiError::new(code, message) and ImportWarning::new(code, message), else the first
        const msgArg = /ApiError::new|ImportWarning::new/.test(m[0]) ? args[1] : args[0];
        if (!msgArg) continue;
        let fmt = null;
        let fmtArgs = [];
        const lit = msgArg.match(/^"/) ? readString(msgArg, 0) : null;
        if (lit && /^\s*(\.to_string\(\)|\.into\(\)|\.to_owned\(\))?\s*$/.test(msgArg.slice(lit.end))) {
          fmt = lit.value;
        } else {
          const f = msgArg.match(/^format!\s*\(/);
          if (f) {
            const inner = readParens(msgArg, f[0].length - 1);
            if (inner) {
              const fa = splitArgs(inner.body);
              const first = fa[0]?.match(/^"/) ? readString(fa[0], 0) : null;
              if (first && first.end === fa[0].length) {
                fmt = first.value;
                fmtArgs = fa.slice(1);
              }
            }
          }
        }
        if (fmt === null) continue;
        add(toTemplate(fmt, fmtArgs), file, lineOf(m.index));
      }
    }
  }
  // what an undo says it did: the arms of a table in undo.rs ("coding.add" => "Removed the coding you added")
  for (const file of rustFiles()) {
    if (!/undo\.rs$/.test(file)) continue;
    let text = fs.readFileSync(file, 'utf8');
    const cut = text.search(/#\[cfg\(test\)\]/);
    if (cut >= 0) text = text.slice(0, cut);
    const lineOf = (offset) => text.slice(0, offset).split('\n').length;
    for (const m of text.matchAll(/"[a-z_]+(?:\.[a-z_]+){1,2}"\s*=>\s*(?=")/g)) {
      const s = readString(text, m.index + m[0].length);
      if (s && /\s/.test(s.value)) add(toTemplate(s.value, []), file, lineOf(m.index));
    }
  }
  // the notes the explore code writes
  for (const file of rustFiles()) {
    if (!/explore\.rs$/.test(file)) continue;
    let text = fs.readFileSync(file, 'utf8');
    const cut = text.search(/#\[cfg\(test\)\]/);
    if (cut >= 0) text = text.slice(0, cut);
    const lineOf = (offset) => text.slice(0, offset).split('\n').length;
    for (const m of text.matchAll(/"((?:[^"\\]|\\.)*(?:counts?|Each row|A column|A cell|A code|A document)[^"]*)"\s*\.to_string\(\)/g)) {
      const s = readString(text, m.index);
      if (s && s.value.length > 25) add(toTemplate(s.value, []), file, lineOf(m.index));
    }
    for (const m of text.matchAll(/format!\(\s*"((?:Each row|A cell|A column|A code)[^"]*)"\s*,?([^)]*)\)/g)) {
      const args = splitArgs(m[2]);
      add(toTemplate(m[1], args), file, lineOf(m.index));
    }
  }
  return [...found.values()].sort((a, b) => a.template.localeCompare(b.template, 'en'));
}

export function serializeEngine(list) {
  return JSON.stringify({ version: 1, count: list.length, messages: list }, null, 2) + '\n';
}

function main() {
  const args = process.argv.slice(2);
  const list = extractEngine();
  if (args.includes('--list')) for (const e of list) console.log(`${e.files[0]}  ${JSON.stringify(e.template)}`);
  console.log(`${list.length} sentences from the engine.`);
  const text = serializeEngine(list);
  if (args.includes('--write')) {
    fs.writeFileSync(ENGINE_FILE, text);
    console.log(`Wrote ${path.relative(process.cwd(), ENGINE_FILE)}.`);
  }
  if (args.includes('--check')) {
    const have = fs.existsSync(ENGINE_FILE) ? fs.readFileSync(ENGINE_FILE, 'utf8') : '';
    if (have !== text) {
      console.log('src/i18n/engine-messages.json is not what the Rust source says. Run: npm run i18n:engine');
      process.exitCode = 1;
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
