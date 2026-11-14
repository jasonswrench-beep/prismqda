// Shared helpers for the language tools: read a source file, parse it (TypeScript with JSX), and walk the tree
// with each node's parent and the property it hangs from. The parser is the one that ships with the build tool
// (rolldown), so no extra package is needed.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseAst } from 'rolldown/parseAst';

export const UI_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
export const SRC = path.join(UI_ROOT, 'src');

/** Every source file of the interface that is not a test, as paths relative to src with forward slashes. */
// A file named *.code.ts holds code in another language that the program writes for the person to run (the R of a recorded script):
// none of its strings is words for the person, so it is not audited. The words round that code are written in files that are audited.
export function sourceFiles() {
  const out = [];
  const visit = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === 'node_modules' || entry.name === 'i18n') continue;
        visit(full);
      } else if (/\.(ts|tsx)$/.test(entry.name) && !/\.test\.(ts|tsx)$/.test(entry.name) && !/\.d\.ts$/.test(entry.name) && !/\.generated\.ts$/.test(entry.name) && !/\.code\.ts$/.test(entry.name)) {
        out.push(path.relative(SRC, full).split(path.sep).join('/'));
      }
    }
  };
  visit(SRC);
  return out.sort();
}

export function readSource(rel) {
  // Files are read as they are on disk (some end lines with CRLF); offsets in the tree match this text.
  return fs.readFileSync(path.join(SRC, rel), 'utf8');
}

export function parseSource(text, rel) {
  return parseAst(text, { lang: rel.endsWith('.tsx') ? 'tsx' : 'ts' });
}

/**
 * Calls visit(node, parent, key) for every node, parents first. Returning false from visit skips the node's
 * children.
 */
export function walk(root, visit) {
  const go = (node, parent, key) => {
    if (!node || typeof node.type !== 'string') return;
    if (visit(node, parent, key) === false) return;
    for (const k of Object.keys(node)) {
      if (k === 'type' || k === 'start' || k === 'end') continue;
      const v = node[k];
      if (Array.isArray(v)) {
        for (const child of v) go(child, node, k);
      } else if (v && typeof v === 'object') {
        go(v, node, k);
      }
    }
  };
  go(root, null, null);
}

/** 1-based line number of an offset. */
export function lineOf(text, offset) {
  let line = 1;
  for (let i = 0; i < offset && i < text.length; i++) if (text.charCodeAt(i) === 10) line++;
  return line;
}

export function lineIndex(text) {
  const starts = [0];
  for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 10) starts.push(i + 1);
  return (offset) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= offset) lo = mid;
      else hi = mid - 1;
    }
    return lo + 1;
  };
}

/** Applies edits ({start, end, text}) that do not overlap to a text. */
export function applyEdits(text, edits) {
  const sorted = [...edits].sort((a, b) => b.start - a.start || b.end - a.end);
  let out = text;
  let lastStart = Infinity;
  for (const e of sorted) {
    if (e.end > lastStart) throw new Error(`overlapping edits at ${e.start}-${e.end}`);
    out = out.slice(0, e.start) + e.text + out.slice(e.end);
    lastStart = e.start;
  }
  return out;
}
