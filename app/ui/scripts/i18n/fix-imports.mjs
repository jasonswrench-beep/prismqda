// Makes sure each file that calls t(), tn(), tc(), rich(), N_(), num(), list(), or dateTime() imports them from the catalog module.
//   node scripts/i18n/fix-imports.mjs [--write]
import fs from 'node:fs';
import path from 'node:path';
import { SRC, parseSource, readSource, sourceFiles, walk } from './ast.mjs';
import { addImport } from './convert.mjs';

const NAMES = ['t', 'tn', 'tc', 'rich', 'N_', 'num', 'list', 'dateTime'];
const write = process.argv.includes('--write');
let fixed = 0;
for (const rel of sourceFiles()) {
  const text = readSource(rel);
  if (!/\b(t|tn|tc|rich|N_|num|list|dateTime)\(/.test(text)) continue;
  const ast = parseSource(text, rel);
  const eol = text.includes('\r\n') ? '\r\n' : '\n';
  const modPath = path.posix.relative(path.posix.dirname(rel), 'i18n');
  const spec = modPath.startsWith('.') ? modPath : './' + modPath;
  const imported = new Set();
  const declared = new Set();
  for (const n of ast.body) {
    if (n.type === 'ImportDeclaration') for (const s of n.specifiers) imported.add(s.local.name);
  }
  const used = new Set();
  walk(ast, (n, parent) => {
    if (n.type === 'CallExpression' && n.callee.type === 'Identifier' && NAMES.includes(n.callee.name)) used.add(n.callee.name);
    return true;
  });
  const missing = [...used].filter((u) => !imported.has(u));
  if (missing.length === 0) continue;
  // a name the file declares itself (not imported) would clash: report it
  const clash = missing.filter((m) => new RegExp(`(const|let|var|function)\\s+${m}\\b`).test(text));
  if (clash.length) {
    console.log(`${rel}: declares ${clash.join(', ')} itself`);
    continue;
  }
  fixed++;
  console.log(`${rel}: adds ${missing.join(', ')}`);
  if (write) fs.writeFileSync(path.join(SRC, rel), addImport(text, missing.sort(), spec, eol));
}
console.log(`${fixed} files`);
