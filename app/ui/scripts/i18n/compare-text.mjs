// One-time rewrite: a.localeCompare(b) and a.localeCompare(b, undefined, options) sort by the language of the computer. The
// order of names should follow the language of the window, so they become compareText(a, b) and compareText(a, b, options),
// which asks the browser for the order of the language in use (see src/i18n/index.ts).
//   node scripts/i18n/compare-text.mjs [--write]

import fs from 'node:fs';
import path from 'node:path';
import { SRC, applyEdits, parseSource, readSource, sourceFiles, walk } from './ast.mjs';
import { addImport } from './convert.mjs';

const write = process.argv.includes('--write');
let total = 0;
for (const rel of sourceFiles()) {
  const text = readSource(rel);
  if (!text.includes('localeCompare')) continue;
  const ast = parseSource(text, rel);
  const edits = [];
  walk(ast, (n) => {
    if (n.type === 'CallExpression' && n.callee.type === 'MemberExpression' && !n.callee.computed && n.callee.property.name === 'localeCompare') {
      const src = (x) => text.slice(x.start, x.end);
      const args = [src(n.callee.object), src(n.arguments[0])];
      const options = n.arguments[2];
      if (options) args.push(src(options));
      edits.push({ start: n.start, end: n.end, text: `compareText(${args.join(', ')})` });
    }
    return true;
  });
  if (edits.length === 0) continue;
  total += edits.length;
  console.log(`${rel}: ${edits.length}`);
  if (write) {
    const eol = text.includes('\r\n') ? '\r\n' : '\n';
    const modPath = path.posix.relative(path.posix.dirname(rel), 'i18n');
    const spec = modPath.startsWith('.') ? modPath : './' + modPath;
    fs.writeFileSync(path.join(SRC, rel), addImport(applyEdits(text, edits), ['compareText'], spec, eol));
  }
}
console.log(`${write ? 'Changed' : 'Would change'} ${total} places.`);
