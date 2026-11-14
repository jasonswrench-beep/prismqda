// Gives better names to the blanks of messages that were named after the function that made their value
// ({toLocaleString}, {asNumber}, {str}): the name should say what goes there, since a translator reads it.
//   node scripts/i18n/polish-names.mjs [--write]

import fs from 'node:fs';
import path from 'node:path';
import { SRC, applyEdits, parseSource, readSource, sourceFiles, walk } from './ast.mjs';

const WEAK = /^(toLocaleString|asNumber|str|tn\d*|plural\d*|slice|trim|trimmed|v|n|i|x|round\d*|value\d*|fileNameOf\d*|metricNoun\d*|describeShape\d*|shapeSentence|describeWhen|joinList|joinOr|join|words|spoken|formatClock|formatWhen|percentText|friendlyTime|codeWords|quoted|durationOf|pathLabel|toLowerCase|replace|map)$/;

function nameOf(e, text) {
  switch (e.type) {
    case 'Identifier':
      return e.name;
    case 'ChainExpression':
      return nameOf(e.expression, text);
    case 'MemberExpression':
      if (!e.computed && e.property.type === 'Identifier') return e.property.name === 'length' ? 'count' : e.property.name;
      return null;
    case 'CallExpression': {
      const c = e.callee;
      const f = c.type === 'Identifier' ? c.name : c.type === 'MemberExpression' && c.property.type === 'Identifier' ? c.property.name : null;
      if (f === 'num' || f === 'String' || f === 'asNumber' || f === 'str' || f === 'toLocaleString' || f === 'toFixed' || f === 'trim' || f === 'toLowerCase' || f === 'slice') {
        if (c.type === 'MemberExpression' && f !== 'num') return nameOf(c.object, text);
        return e.arguments[0] ? nameOf(e.arguments[0], text) : null;
      }
      if (f === 'tn') {
        const forms = e.arguments[1];
        const other = forms?.properties?.find((p) => (p.key.name ?? p.key.value) === 'other')?.value;
        const s = other?.type === 'Literal' && typeof other.value === 'string' ? other.value : null;
        if (s) {
          const word = s.replace(/\{[^}]*\}/g, ' ').match(/[A-Za-z]+(?=[^A-Za-z]*$)/);
          if (word) return word[0].toLowerCase();
        }
        return null;
      }
      if (f === 'fileNameOf') return 'file';
      if (f === 'formatWhen' || f === 'describeWhen' || f === 'friendlyTime') return 'when';
      if (f === 'formatClock') return 'time';
      if (f === 'join' || f === 'joinList' || f === 'joinOr') return 'list';
      if (f === 'percentText') return 'percent';
      if (f === 'metricNoun') return 'noun';
      if (f === 'describeShape' || f === 'shapeSentence') return 'shape';
      if (f === 'quoted') return 'quote';
      return null;
    }
    case 'BinaryExpression':
      return nameOf(e.left, text) ?? nameOf(e.right, text);
    default:
      return null;
  }
}

function polishFile(rel, write) {
  const text = readSource(rel);
  if (!/\b(t|tn|tc|rich)\(/.test(text)) return 0;
  const ast = parseSource(text, rel);
  const edits = [];
  walk(ast, (node) => {
    if (node.type !== 'CallExpression' || node.callee.type !== 'Identifier') return true;
    const fn = node.callee.name;
    if (!['t', 'tn', 'tc', 'rich'].includes(fn)) return true;
    const msgNodes = [];
    let argsNode;
    if (fn === 't' || fn === 'rich') {
      msgNodes.push(node.arguments[0]);
      argsNode = node.arguments[1];
    } else if (fn === 'tc') {
      msgNodes.push(node.arguments[1]);
      argsNode = node.arguments[2];
    } else {
      const forms = node.arguments[1];
      if (forms && forms.type === 'ObjectExpression') for (const p of forms.properties) msgNodes.push(p.value);
      argsNode = node.arguments[2];
    }
    if (!argsNode || argsNode.type !== 'ObjectExpression') return true;
    const lits = msgNodes.filter((m) => m && m.type === 'Literal' && typeof m.value === 'string');
    if (lits.length !== msgNodes.length) return true;
    const keys = argsNode.properties.map((p) => (p.type === 'Property' && !p.computed ? (p.key.type === 'Identifier' ? p.key.name : String(p.key.value)) : null));
    if (keys.some((k) => k === null)) return true;
    const taken = new Set(keys);
    const renames = new Map();
    argsNode.properties.forEach((p, i) => {
      const k = keys[i];
      if (!WEAK.test(k)) return;
      let better = nameOf(p.value, text);
      if (!better || better === k || WEAK.test(better) || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(better)) return;
      let name = better;
      for (let n = 2; taken.has(name) && name !== k; n++) name = better + n;
      if (name === k) return;
      taken.delete(k);
      taken.add(name);
      renames.set(k, { name, prop: p });
    });
    if (renames.size === 0) return true;
    for (const [oldName, { name, prop }] of renames) {
      // the key (a shorthand property needs the value kept)
      if (prop.shorthand) edits.push({ start: prop.start, end: prop.end, text: `${name}: ${text.slice(prop.value.start, prop.value.end)}` });
      else edits.push({ start: prop.key.start, end: prop.key.end, text: name });
      for (const lit of lits) {
        const raw = text.slice(lit.start, lit.end);
        const token = `{${oldName}}`;
        let at = raw.indexOf(token);
        while (at >= 0) {
          edits.push({ start: lit.start + at, end: lit.start + at + token.length, text: `{${name}}` });
          at = raw.indexOf(token, at + token.length);
        }
      }
    }
    return true;
  });
  if (edits.length === 0) return 0;
  // two calls can share a literal (never), but the same range twice would throw
  const seen = new Set();
  const unique = edits.filter((e) => {
    const k = `${e.start}:${e.end}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  if (write) fs.writeFileSync(path.join(SRC, rel), applyEdits(text, unique));
  return unique.length;
}

const write = process.argv.includes('--write');
let total = 0;
for (const rel of sourceFiles()) {
  const n = polishFile(rel, write);
  if (n) console.log(`${rel}: ${n}`);
  total += n;
}
console.log(`${write ? 'Changed' : 'Would change'} ${total} places.`);
