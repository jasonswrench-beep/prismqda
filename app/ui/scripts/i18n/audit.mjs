// Finds the words in the interface's source code that are not yet in the language catalog.
//
// A person-facing sentence is one of three things in the code: text between tags (<b>Save</b>), a string given
// to an attribute or a call (aria-label="Close", announce('Saved')), or a string stored in a table (label: 'Home').
// This tool lists every string that looks like words and is not already the first argument of t(), tc(), tn(), or N_().
//
//   node scripts/i18n/audit.mjs            list what is left, grouped by where the string sits
//   node scripts/i18n/audit.mjs --check    exit 1 when anything is left that is not in scripts/i18n/allow.json
//   node scripts/i18n/audit.mjs --files a,b   only these files (relative to src)
//
// allow.json lists the strings that look like words but are not for the person (a key name compared against, a
// word sent to the engine, the text that is meant for a chatbot). Each entry says why.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { UI_ROOT, lineIndex, parseSource, readSource, sourceFiles, walk } from './ast.mjs';

const ALLOW_FILE = path.join(UI_ROOT, 'scripts', 'i18n', 'allow.json');

/** The calls whose first argument is already in the catalog. */
export const MARKERS = new Set(['t', 'tc', 'tn', 'N_', 'rich']);

/** JSX attributes that never hold words for a person. */
const QUIET_ATTRS = new Set([
  'className', 'id', 'key', 'ref', 'role', 'type', 'name', 'href', 'src', 'htmlFor', 'for', 'style', 'target', 'rel', 'method', 'action',
  'autoComplete', 'inputMode', 'pattern', 'accept', 'lang', 'dir', 'viewBox', 'd', 'fill', 'stroke', 'points', 'xmlns', 'transform', 'width', 'height',
  'x', 'y', 'cx', 'cy', 'r', 'rx', 'ry', 'x1', 'x2', 'y1', 'y2', 'textAnchor', 'dominantBaseline', 'preserveAspectRatio', 'strokeLinecap', 'strokeLinejoin',
  'aria-labelledby', 'aria-describedby', 'aria-controls', 'aria-owns', 'aria-live', 'aria-haspopup', 'aria-orientation', 'aria-sort', 'aria-current',
  'aria-relevant', 'aria-atomic', 'aria-autocomplete', 'aria-invalid', 'aria-checked', 'aria-expanded', 'aria-selected', 'aria-pressed', 'aria-disabled',
  'aria-hidden', 'aria-busy', 'aria-modal', 'aria-multiline', 'aria-multiselectable', 'aria-readonly', 'aria-required', 'aria-keyshortcuts', 'aria-activedescendant',
  'tabIndex', 'slot', 'variant', 'size', 'color', 'kind', 'mode', 'orientation', 'placement', 'shouldFlip', 'selectionMode', 'selectionBehavior',
  'defaultValue', 'value', 'min', 'max', 'step', 'data-testid', 'icon', 'to', 'crossOrigin', 'loading', 'preload', 'controls', 'wrap', 'spellCheck', 'enterKeyHint',
  'validationBehavior', 'translate', 'draggable', 'contentEditable', 'scope', 'headers', 'colSpan', 'rowSpan', 'align', 'valign', 'media', 'sizes', 'srcSet',
  'focusRingClass', 'dataKey', 'tone', 'status', 'stateKey', 'page', 'kindOf', 'weight',
]);

/** Calls whose string arguments are never words for a person. */
const QUIET_CALLS = new Set([
  'querySelector', 'querySelectorAll', 'getElementById', 'getAttribute', 'setAttribute', 'removeAttribute', 'hasAttribute', 'closest', 'matches',
  'addEventListener', 'removeEventListener', 'dispatchEvent', 'getItem', 'setItem', 'removeItem', 'rpc', 'invoke', 'import', 'require', 'createElement',
  'createElementNS', 'matchMedia', 'getComputedStyle', 'getPropertyValue', 'setProperty', 'removeProperty', 'RegExp', 'Symbol', 'parseInt', 'parseFloat',
  'startsWith', 'endsWith', 'indexOf', 'lastIndexOf', 'split', 'join', 'replace', 'replaceAll', 'padStart', 'padEnd', 'repeat', 'normalize', 'localeCompare',
  'has', 'get', 'set', 'delete', 'add', 'test', 'match', 'exec', 'log', 'info', 'warn', 'error', 'debug', 'assert', 'listen', 'emit', 'on', 'off', 'once',
  'getElementsByClassName', 'getElementsByTagName', 'toggle', 'contains', 'remove', 'call', 'bind', 'resolve', 'basename', 'dirname', 'extname',
  'createStore', 'useRef', 'createContext', 'forwardRef', 'cx', 'clsx', 'classNames', 'cn', 'sort', 'getDay', 'focus', 'scrollIntoView', 'hasOwnProperty',
  'defineProperty', 'getOwnPropertyDescriptor', 'fromEntries', 'entries', 'keys', 'values', 'stringify', 'parse', 'pick', 'omit', 'format', 'toString',
  'toLocaleString', 'toLocaleDateString', 'toLocaleTimeString', 'DateTimeFormat', 'NumberFormat', 'PluralRules', 'ListFormat', 'Collator', 'readFileSync',
  'Boolean', 'Number', 'String', 'Object', 'Array', 'JSON', 'Math', 'Date', 'Promise', 'Set', 'Map', 'WeakMap', 'URL', 'URLSearchParams', 'Blob', 'File',
  'useCallback', 'useMemo', 'useEffect', 'useState', 'useReducer', 'useId', 'createSlice', 'post', 'fetch', 'open', 'close', 'send',
]);

function calleeName(call) {
  const c = call.callee;
  if (!c) return null;
  if (c.type === 'Identifier') return c.name;
  if (c.type === 'MemberExpression' && !c.computed && c.property.type === 'Identifier') return c.property.name;
  return null;
}

/** True for text that a person could read as words: letters, and either several words or a capital first letter. */
export function looksLikeWords(s) {
  const text = s.replace(/\s+/g, ' ').trim();
  if (!/\p{L}/u.test(text)) return false;
  // A short lower-case phrase of plain words ("a code", "no one yet") is a piece of a sentence.
  if (text.includes(' ') && /^[a-z]+(?:[ ,.'’]+[a-z]+)+[.,]?$/.test(text) && /[a-z]{2,}/.test(text)) return true;
  // Key combinations are not words.
  if (/^((Ctrl|Alt|Shift|Meta|Cmd)\+)+[A-Za-z0-9]+$/.test(text)) return false;
  // Identifiers, class names, urls, selectors, mime types, file names, keys, units, css values.
  if (/^[a-z0-9_\-.:/#@[\]=*>+~,()'"^$|%!?<&;{} ]+$/.test(text) && !/ [a-z]+ [a-z]+/.test(text)) {
    if (!/[A-Z]/.test(text) || !text.includes(' ')) return false;
  }
  if (/^https?:\/\//.test(text)) return false;
  if (/^[A-Za-z0-9_.-]+\.(png|jpg|jpeg|gif|webp|bmp|svg|pdf|docx?|xlsx?|csv|json|txt|md|html?|rtf|odt|pqda|qdc|zip|wav|mp3|mp4|vtt|srt)$/i.test(text)) return false;
  if (/^[a-z]+(_[a-z0-9]+)+$/.test(text)) return false;
  if (/^[a-z][a-zA-Z0-9]*$/.test(text)) return false; // camelCase or lower-case single word: a key
  if (/^[A-Z][A-Z0-9_]+$/.test(text) && text.length < 5) return false; // PDF, CSV, XLSX stay (they are names), but ids like 'A' do not
  const words = text.split(' ').filter((w) => /\p{L}/u.test(w));
  if (words.length >= 2) return true;
  return /^\p{Lu}/u.test(text) && text.length >= 2;
}

/** A template with blanks is a sentence when some word of two letters or more stands on its own (not joined by - _ / or .). */
export const templateHasProse = (stat) => /(^|[\s("'“])\p{L}{2,}([\s.,;:!?)"'”]|$)/u.test(stat.join(''));

function textOfLiteral(node) {
  if (node.type === 'Literal' && typeof node.value === 'string') return node.value;
  if (node.type === 'TemplateLiteral') return node.quasis.map((q) => q.value.cooked ?? q.value.raw).join('${}');
  return null;
}

function keyName(prop) {
  if (!prop.key) return null;
  if (prop.key.type === 'Identifier') return prop.key.name;
  if (prop.key.type === 'Literal') return String(prop.key.value);
  return null;
}

/** How a node is used: the name of what consumes the string, so strings can be grouped and the harmless ones set aside. */
function consumer(node, parentOf) {
  let cur = node;
  let p = parentOf.get(cur);
  while (p) {
    switch (p.type) {
      case 'ConditionalExpression':
        if (p.test === cur) return 'test';
        break;
      case 'LogicalExpression':
      case 'ParenthesizedExpression':
      case 'TSAsExpression':
      case 'TSSatisfiesExpression':
      case 'TSNonNullExpression':
      case 'ArrayExpression':
      case 'SequenceExpression':
        break;
      case 'TemplateLiteral':
        break;
      case 'JSXExpressionContainer': {
        const pp = parentOf.get(p);
        if (pp && pp.type === 'JSXAttribute') return `attr:${pp.name.name ?? pp.name.property?.name}`;
        return 'child';
      }
      case 'JSXAttribute':
        return `attr:${p.name.name ?? p.name.property?.name}`;
      case 'CallExpression':
      case 'NewExpression':
        if (p.callee === cur) return 'callee';
        return `call:${p.type === 'NewExpression' ? 'new ' : ''}${calleeName(p) ?? '?'}`;
      case 'Property':
        if (p.key === cur) return 'key';
        return `prop:${keyName(p) ?? '?'}`;
      case 'ReturnStatement':
        return 'return';
      case 'ArrowFunctionExpression':
        return 'arrow-return';
      case 'VariableDeclarator':
        return `var:${p.id.name ?? '?'}`;
      case 'AssignmentExpression':
        return `assign:${p.left.property?.name ?? p.left.name ?? '?'}`;
      case 'BinaryExpression':
        return `binary:${p.operator}`;
      case 'SwitchCase':
        return 'case';
      case 'ImportDeclaration':
      case 'ExportAllDeclaration':
      case 'ExportNamedDeclaration':
        return 'import';
      case 'TSLiteralType':
      case 'TSTypeAnnotation':
      case 'TSUnionType':
      case 'TSTypeReference':
      case 'TSPropertySignature':
        return 'type';
      case 'ThrowStatement':
        return 'throw';
      case 'ExpressionStatement':
        return 'directive';
      case 'AssignmentPattern':
        return 'default';
      case 'TemplateElement':
        break;
      default:
        return `other:${p.type}`;
    }
    cur = p;
    p = parentOf.get(cur);
  }
  return 'none';
}

export function loadAllow() {
  try {
    return JSON.parse(fs.readFileSync(ALLOW_FILE, 'utf8'));
  } catch {
    return { entries: [] };
  }
}

/** What the audit finds in one file: [{ line, kind, how, text }] */
export function auditFile(rel) {
  const text = readSource(rel);
  const ast = parseSource(text, rel);
  const lineAt = lineIndex(text);
  const parentOf = new Map();
  const found = [];
  const seen = new Set();
  walk(ast, (node, parent) => {
    if (parent) parentOf.set(node, parent);
    return true;
  });

  const inMarker = (node) => {
    let p = parentOf.get(node);
    let cur = node;
    while (p) {
      if ((p.type === 'CallExpression') && p.arguments[0] === cur && MARKERS.has(calleeName(p))) return true;
      if (p.type === 'CallExpression' && MARKERS.has(calleeName(p)) && calleeName(p) === 'tc' && p.arguments[1] === cur) return true;
      if (p.type === 'CallExpression' && calleeName(p) === 'tn' && p.arguments[1] === cur) return true;
      if (p.type === 'Property' && p.key !== cur) {
        // an object passed as the plural forms of tn
        const pp = parentOf.get(p);
        const ppp = pp && parentOf.get(pp);
        if (ppp && ppp.type === 'CallExpression' && calleeName(ppp) === 'tn') return true;
      }
      cur = p;
      p = parentOf.get(cur);
    }
    return false;
  };

  // A top-level table whose values are all plain strings ({ name: 'name', color: 'color' }) holds names for the person even
  // when they are single lower-case words.
  const tableValues = new Set();
  walk(ast, (node) => {
    if (node.type === 'ObjectExpression' && parentOf.get(node)?.type === 'VariableDeclarator' && node.properties.length >= 2) {
      let top = true;
      let p = parentOf.get(node);
      while (p) {
        if (p.type === 'FunctionDeclaration' || p.type === 'FunctionExpression' || p.type === 'ArrowFunctionExpression' || p.type === 'ClassBody') top = false;
        p = parentOf.get(p);
      }
      const props = node.properties.filter((q) => q.type === 'Property' && q.kind === 'init' && q.value.type === 'Literal' && typeof q.value.value === 'string');
      if (top && props.length >= 2 && props.every((q) => /^[A-Za-z][A-Za-z ,'’-]*$/.test(q.value.value))) for (const q of props) tableValues.add(q.value);
    }
    return true;
  });

  walk(ast, (node) => {
    // TypeScript types hold words only as names of kinds; skip them whole.
    if (node.type.startsWith('TS') && node.type !== 'TSAsExpression' && node.type !== 'TSNonNullExpression' && node.type !== 'TSSatisfiesExpression') return false;
    if (node.type === 'ImportDeclaration' || node.type === 'ExportAllDeclaration') return false;
    if (node.type === 'JSXText') {
      const cleaned = node.value.replace(/\s+/g, ' ').trim();
      if (cleaned && /\p{L}/u.test(cleaned)) found.push({ line: lineAt(node.start), kind: 'jsx-text', how: 'child', text: cleaned });
      return true;
    }
    if (node.type === 'Literal' && typeof node.value === 'string' || node.type === 'TemplateLiteral') {
      if (seen.has(node)) return true;
      const s = textOfLiteral(node);
      if (s === null) return true;
      if (node.type === 'TemplateLiteral' && node.expressions.length > 0) {
        const stat = node.quasis.map((q) => q.value.cooked ?? q.value.raw);
        if (stat.some((x) => x.includes('://')) || !templateHasProse(stat)) return true;
      } else if (!looksLikeWords(s) && !tableValues.has(node)) return true;
      if (inMarker(node)) return true;
      const how = consumer(node, parentOf);
      if (how === 'import' || how === 'type' || how === 'key' || how === 'directive' || how === 'test') return true;
      // A template literal's inner quasis are not visited as literals, so only the whole template is reported.
      const p = parentOf.get(node);
      if (p && p.type === 'JSXAttribute' && QUIET_ATTRS.has(p.name.name)) return true;
      if (how.startsWith('attr:') && QUIET_ATTRS.has(how.slice(5))) return true;
      if (how.startsWith('call:') && QUIET_CALLS.has(how.slice(5).replace(/^new /, ''))) return true;
      if (how.startsWith('binary:') && /^binary:(===|!==|==|!=)$/.test(how)) {
        found.push({ line: lineAt(node.start), kind: 'compare', how, text: s });
        return true;
      }
      if (how === 'case') return true;
      found.push({ line: lineAt(node.start), kind: node.type === 'TemplateLiteral' ? 'template' : 'string', how, text: s });
    }
    return true;
  });
  return found;
}

function entryKey(rel, f) {
  return `${rel}\u0001${f.text}`;
}

function main() {
  const args = process.argv.slice(2);
  const check = args.includes('--check');
  const filesArg = args.indexOf('--files');
  const only = filesArg >= 0 ? new Set(args[filesArg + 1].split(',')) : null;
  const allow = loadAllow();
  const allowed = new Set((allow.entries ?? []).map((e) => `${e.file}\u0001${e.text}`));
  const byHow = new Map();
  const perFile = new Map();
  let total = 0;
  for (const rel of sourceFiles()) {
    if (only && !only.has(rel)) continue;
    const found = auditFile(rel).filter((f) => !allowed.has(entryKey(rel, f)));
    if (found.length === 0) continue;
    perFile.set(rel, found);
    for (const f of found) {
      total++;
      const k = `${f.kind} ${f.how}`;
      byHow.set(k, (byHow.get(k) ?? 0) + 1);
    }
  }
  if (args.includes('--summary')) {
    console.log(`${total} strings left in ${perFile.size} files`);
    for (const [k, n] of [...byHow.entries()].sort((a, b) => b[1] - a[1])) console.log(String(n).padStart(5), k);
    return;
  }
  if (args.includes('--counts')) {
    console.log(`${total} strings left in ${perFile.size} files`);
    for (const [rel, found] of [...perFile.entries()].sort((a, b) => b[1].length - a[1].length)) console.log(String(found.length).padStart(5), rel);
    return;
  }
  for (const [rel, found] of perFile) {
    console.log(`\n${rel} (${found.length})`);
    for (const f of found) console.log(`  ${String(f.line).padStart(5)}  ${f.kind.padEnd(8)} ${f.how.padEnd(18)} ${JSON.stringify(f.text).slice(0, 150)}`);
  }
  console.log(`\n${total} strings left in ${perFile.size} files`);
  if (check && total > 0) process.exit(1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
