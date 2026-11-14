// Moves the interface's words into the language catalog, mechanically, for the cases that have one right answer:
//
//   text between tags          <b>Save {n} codes</b>        ->  <b>{tn(n, { one: 'Save {count} code', other: 'Save {count} codes' })}</b>
//   text with inline marks     Press <kbd>Enter</kbd> to go ->  {rich('Press <kbd>Enter</kbd> to go', { kbd: (c) => <kbd>{c}</kbd> })}
//   attributes with words      aria-label="Close"           ->  aria-label={t('Close')}
//   words handed to a call     announce(`Saved ${name}`)    ->  announce(t('Saved {name}', { name }))
//   words in a table           { label: 'Home' }             ->  { get label() { return t('Home'); } }   (at the top of a file)
//   n.toLocaleString('en-US')                               ->  num(n)
//   plural(n, 'unit')                                       ->  tn(n, { one: '{count} unit', other: '{count} units' })
//
// What it cannot decide it lists and leaves alone, for a person to finish. Run without --write to see the plan.
//
//   node scripts/i18n/convert.mjs [--write] [--files a.tsx,b.ts] [--verbose]

import fs from 'node:fs';
import path from 'node:path';
import { SRC, applyEdits, lineIndex, parseSource, readSource, sourceFiles, walk } from './ast.mjs';
import { looksLikeWords, templateHasProse } from './audit.mjs';

const ATTRS = new Set(['none', 'what', 'detail', 'scopeLabel', 'selectionText', 'aria-label', 'aria-description', 'aria-roledescription', 'aria-valuetext', 'title', 'placeholder', 'alt', 'label', 'description', 'noun', 'summary', 'caption', 'hint', 'emptyText', 'tooltip']);
const SINK_CALLS = new Set(['announce', 'setError', 'setNotice', 'setNote', 'setLocationError', 'say', 'done', 'change', 'push', 'button']);
const PROP_KEYS = new Set(['label', 'title', 'help', 'description', 'caption', 'keywords', 'keys', 'does', 'hint', 'message', 'problem', 'reason', 'note', 'what', 'text', 'button', 'summary']);
const INLINE_TAGS = new Set(['strong', 'em', 'b', 'i', 'code', 'kbd', 'a', 'span', 'mark', 'abbr', 'q', 'small', 'sub', 'sup', 'u', 's', 'cite', 'var', 'samp', 'time', 'bdi']);
const BLOCK_TAGS = new Set(['p', 'div', 'ul', 'ol', 'li', 'dl', 'dt', 'dd', 'table', 'thead', 'tbody', 'tfoot', 'tr', 'td', 'th', 'section', 'article', 'aside', 'header', 'footer', 'nav', 'main', 'form', 'fieldset', 'legend', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'blockquote', 'figure', 'figcaption', 'details', 'summary', 'hr', 'label', 'button', 'select', 'option', 'textarea', 'caption', 'output']);
const KEEP_TAGS = new Set(['pre', 'style', 'script', 'textarea']);

const ENT = { quot: '"', apos: "'", amp: '&', lt: '<', gt: '>', nbsp: ' ', rsquo: '’', lsquo: '‘', rdquo: '”', ldquo: '“', bull: '•', mdash: '—', ndash: '–', hellip: '…', middot: '·', times: '×', rarr: '→', larr: '←' };

export function decodeEntities(s) {
  return s.replace(/&(#x[0-9a-fA-F]+|#\d+|[a-zA-Z]+);/g, (m, e) => {
    if (e[0] === '#') {
      const code = e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
      return Number.isFinite(code) ? String.fromCodePoint(code) : m;
    }
    return ENT[e] ?? m;
  });
}

/** The text of a run of JSX text as it renders: lines are trimmed, joined by a space, and entities are decoded. */
export function cleanJsxText(raw) {
  const lines = raw.split(/\r\n|\n|\r/);
  let lastNonEmpty = 0;
  for (let i = 0; i < lines.length; i++) if (/[^ \t]/.test(lines[i])) lastNonEmpty = i;
  let out = '';
  for (let i = 0; i < lines.length; i++) {
    let line = lines[i].replace(/\t/g, ' ');
    if (i !== 0) line = line.replace(/^ +/, '');
    if (i !== lines.length - 1) line = line.replace(/ +$/, '');
    if (line) {
      if (i !== lastNonEmpty) line += ' ';
      out += line;
    }
  }
  return decodeEntities(out);
}

/** A JS string literal for a message. */
export function quote(s) {
  const escaped = s.replace(/\\/g, '\\\\').replace(/\r/g, '\\r').replace(/\n/g, '\\n');
  if (escaped.includes("'") && !escaped.includes('"')) return `"${escaped}"`;
  return `'${escaped.replace(/'/g, "\\'")}'`;
}

function messageEscape(s) {
  return s.replace(/\{/g, '{{').replace(/\}/g, '}}');
}

const hasLetters = (s) => /\p{L}/u.test(s);
const hasWords = (s) => /\p{L}{2,}/u.test(s);

// ---------------------------------------------------------------------------------------------- per file

export function convertFile(rel, opts = {}) {
  const text = opts.text ?? readSource(rel);
  const ast = parseSource(text, rel);
  const eol = text.includes('\r\n') ? '\r\n' : '\n';
  const lineAt = lineIndex(text);
  const parentOf = new Map();
  walk(ast, (node, parent) => {
    if (parent) parentOf.set(node, parent);
    return true;
  });

  // A name the catalog needs (t, tn, num, rich) that the file declares at its top for something else cannot be added as it is.
  // (A name reused inside a function is left to the type check, which reports a call that goes to the wrong thing.)
  {
    const taken = new Set();
    for (const n of ast.body) {
      const decl = n.type === 'ExportNamedDeclaration' && n.declaration ? n.declaration : n;
      if ((decl.type === 'FunctionDeclaration' || decl.type === 'ClassDeclaration') && decl.id && ['t', 'tn', 'num', 'rich'].includes(decl.id.name)) taken.add(decl.id.name);
      if (decl.type === 'VariableDeclaration') for (const d of decl.declarations) if (d.id.type === 'Identifier' && ['t', 'tn', 'num', 'rich'].includes(d.id.name)) taken.add(d.id.name);
    }
    if (taken.size > 0) {
      const names = [...taken].join(', ');
      return { rel, changed: false, notes: [{ line: 1, kind: 'shadow', msg: `the file declares the name ${names} for something else` }], text, newText: text, edits: 0, list: [] };
    }
  }
  const notes = []; // things left for a person
  const note = (node, kind, msg) => notes.push({ line: lineAt(node.start), kind, msg });
  const edits = []; // phase 1
  const used = new Set(); // names to import from the catalog module: t, tn, rich, num

  const src = (node) => text.slice(node.start, node.end);
  const isFn = (n) => n.type === 'FunctionDeclaration' || n.type === 'FunctionExpression' || n.type === 'ArrowFunctionExpression';
  const atTopLevel = (node) => {
    let p = parentOf.get(node);
    while (p) {
      if (isFn(p) || p.type === 'ClassBody') return false;
      p = parentOf.get(p);
    }
    return true;
  };
  /** An object that is sent on to the engine or to storage holds data, not words for the screen. */
  const handedToEngine = (node) => {
    let p = parentOf.get(node);
    while (p) {
      if (p.type === 'CallExpression') {
        const c = p.callee;
        const name = c.type === 'Identifier' ? c.name : c.type === 'MemberExpression' && c.property.type === 'Identifier' ? c.property.name : '';
        if (['rpc', 'invoke', 'stringify', 'setItem', 'send', 'post', 'fetch'].includes(name)) return true;
      }
      p = parentOf.get(p);
    }
    return false;
  };
  const containsJsx = (node) => {
    let found = false;
    walk(node, (n) => {
      if (n.type === 'JSXElement' || n.type === 'JSXFragment') found = true;
      return !found;
    });
    return found;
  };

  /** The source of a node with the phase 1 edits inside it applied. */
  const srcWith = (node, extra = []) => {
    const inner = [...edits, ...extra].filter((e) => e.start >= node.start && e.end <= node.end);
    // outermost edits win
    const kept = inner.filter((e) => !inner.some((o) => o !== e && o.start <= e.start && o.end >= e.end && (o.start < e.start || o.end > e.end)));
    return applyEdits(src(node), kept.map((e) => ({ start: e.start - node.start, end: e.end - node.start, text: e.text })));
  };

  // ------------------------------------------------------------------------------------------ naming blanks

  const CALL_NAMES = { fileNameOf: 'file', formatWhen: 'when', formatClock: 'time', describeCount: 'count', describeShape: 'shape', shapeSentence: 'shape', linkSentence: 'link', outcomeLine: 'outcome', revisionLine: 'revision', metricNoun: 'noun', metricLabel: 'metric', percentText: 'percent', quoted: 'quote', pathLabel: 'path' };
  const THROUGH = new Set(['toLocaleString', 'toLowerCase', 'toUpperCase', 'trim', 'trimEnd', 'trimStart', 'slice', 'toFixed', 'toString', 'replace', 'replaceAll', 'normalize', 'at']);
  function baseName(e) {
    if (e.type === 'Identifier') return e.name;
    if (e.type === 'ChainExpression') return baseName(e.expression);
    if (e.type === 'MemberExpression' && !e.computed && e.property.type === 'Identifier') return e.property.name === 'length' ? 'count' : e.property.name;
    if (e.type === 'CallExpression' && e.callee.type === 'Identifier') {
      const f = e.callee.name;
      if (f === 'num' && e.arguments[0]) return baseName(e.arguments[0]);
      if (f === 'plural' && e.arguments[1]?.type === 'Literal') return String(e.arguments[1].value).replace(/\W+/g, '_') + 's';
      if (CALL_NAMES[f]) return CALL_NAMES[f];
      return f.replace(/(Of|Text|Label|Name|Line)$/, '') || f;
    }
    if (e.type === 'CallExpression' && e.callee.type === 'MemberExpression' && e.callee.property.type === 'Identifier') {
      const m = e.callee.property.name;
      if (THROUGH.has(m)) return baseName(e.callee.object);
      if (m === 'join') return 'list';
      if (e.callee.object.type === 'Identifier' && e.callee.object.name === 'Math' && e.arguments[0]) return baseName(e.arguments[0]);
      return m;
    }
    if (e.type === 'BinaryExpression') return baseName(e.left) === 'value' ? baseName(e.right) : baseName(e.left);
    if (e.type === 'LogicalExpression') return baseName(e.left);
    if (e.type === 'ConditionalExpression') return 'text';
    return 'value';
  }

  function makeNamer() {
    const bySource = new Map();
    const names = new Set();
    return (e) => {
      const key = srcWith(e);
      if (bySource.has(key)) return bySource.get(key);
      let base = baseName(e).replace(/[^A-Za-z0-9_]/g, '_');
      if (!/^[A-Za-z_]/.test(base)) base = 'value';
      let name = base;
      for (let n = 2; names.has(name); n++) name = base + n;
      names.add(name);
      bySource.set(key, name);
      return name;
    };
  }

  const argsText = (args) => {
    if (args.length === 0) return '';
    return `{ ${args.map(([name, code]) => (name === code ? name : `${name}: ${code}`)).join(', ')} }`;
  };

  // ------------------------------------------------------------------------------------------ plural conditionals

  /** `n === 1 ? 'a' : 'b'` (or !==): { count: source of n, one, other } else null. */
  function pluralOf(e) {
    if (e.type !== 'ConditionalExpression') return null;
    const t = e.test;
    if (t.type !== 'BinaryExpression' || (t.operator !== '===' && t.operator !== '!==')) return null;
    const one = t.right.type === 'Literal' && t.right.value === 1 ? t.left : t.left.type === 'Literal' && t.left.value === 1 ? t.right : null;
    if (!one) return null;
    const lit = (n) => (n.type === 'Literal' && typeof n.value === 'string' ? n.value : null);
    const a = lit(e.consequent);
    const b = lit(e.alternate);
    if (a === null || b === null) return null;
    return { count: one, countSrc: src(one), one: t.operator === '===' ? a : b, other: t.operator === '===' ? b : a };
  }

  const isCountOf = (e, countSrc) => {
    if (src(e) === countSrc) return true;
    if (e.type === 'CallExpression' && e.callee.type === 'Identifier' && e.callee.name === 'num' && e.arguments[0] && src(e.arguments[0]) === countSrc) return true;
    // an already converted n.toLocaleString('en-US')
    if (e.type === 'CallExpression' && e.callee.type === 'MemberExpression' && e.callee.property.name === 'toLocaleString' && src(e.callee.object) === countSrc) return true;
    return false;
  };

  // ------------------------------------------------------------------------------------------ building messages

  /**
   * parts: [{ text }] | [{ expr }] | [{ open: 'tag' }] | [{ close: 'tag' }] | [{ self: 'tag' }]
   * Returns the code of a call (t, tn, or rich) or null when nothing here is a message.
   */
  function buildCall(parts, tagArgs = [], opts2 = {}) {
    const namer = makeNamer();
    // a plural?
    let plural = null;
    for (const p of parts) {
      if (p.expr) {
        const pl = pluralOf(p.expr);
        if (pl) {
          if (plural && plural.countSrc !== pl.countSrc) return { error: 'more than one count in one sentence' };
          plural = pl;
        }
      }
    }
    const args = [];
    const argSeen = new Set();
    const addArg = (name, code) => {
      if (!argSeen.has(name)) {
        argSeen.add(name);
        args.push([name, code]);
      }
    };
    const render = (which) => {
      let s = '';
      for (const p of parts) {
        if (p.text !== undefined) s += messageEscape(p.text);
        else if (p.open) s += `<${p.open}>`;
        else if (p.close) s += `</${p.close}>`;
        else if (p.self) s += `<${p.self}/>`;
        else if (p.expr) {
          const pl = plural ? pluralOf(p.expr) : null;
          if (pl) s += messageEscape(which === 'one' ? pl.one : pl.other);
          else if (plural && isCountOf(p.expr, plural.countSrc)) s += '{count}';
          else if (p.expr.type === 'Literal' && typeof p.expr.value === 'string') s += messageEscape(p.expr.value);
          else {
            const name = namer(p.expr);
            addArg(name, srcWith(p.expr));
            s += `{${name}}`;
          }
        }
      }
      return s;
    };
    const rich = parts.some((p) => p.open || p.self);
    if (plural) {
      const one = render('one');
      const other = render('other');
      if (rich) return { error: 'a count and marked words in one sentence' };
      used.add('tn');
      const countSrc = srcWith(plural.count);
      const rest = argsText(args);
      return { code: `tn(${countSrc}, { one: ${quote(one)}, other: ${quote(other)} }${rest ? ', ' + rest : ''})`, message: other };
    }
    const msg = render('other');
    if (!hasLetters(msg.replace(/\{[A-Za-z_][A-Za-z0-9_]*\}/g, '')) && !opts2.force) return null;
    if (rich) {
      used.add('rich');
      const all = [...args, ...tagArgs];
      return { code: `rich(${quote(msg)}${all.length ? ', ' + argsText(all) : ''})`, message: msg, rich: true };
    }
    used.add('t');
    return { code: `t(${quote(msg)}${args.length ? ', ' + argsText(args) : ''})`, message: msg };
  }

  // ------------------------------------------------------------------------------------------ values in expressions

  /** Edits that turn the words in an expression into catalog calls. */
  function wrapValue(e, out) {
    switch (e.type) {
      case 'Literal':
        if (typeof e.value === 'string' && looksLikeWords(e.value)) {
          used.add('t');
          out.push({ start: e.start, end: e.end, text: `t(${quote(e.value)})` });
        }
        return;
      case 'TemplateLiteral': {
        const stat = e.quasis.map((q) => q.value.cooked ?? q.value.raw);
        if (stat.some((x) => x.includes('://'))) return;
        if (e.expressions.length === 0 ? !looksLikeWords(stat[0]) : !templateHasProse(stat)) return;
        const parts = [];
        e.quasis.forEach((q, i) => {
          const s = q.value.cooked ?? q.value.raw;
          if (s) parts.push({ text: s });
          if (e.expressions[i]) parts.push({ expr: e.expressions[i] });
        });
        const call = buildCall(parts, [], { force: true });
        if (!call) return;
        if (call.error) {
          note(e, 'template', call.error);
          return;
        }
        out.push({ start: e.start, end: e.end, text: call.code });
        return;
      }
      case 'ConditionalExpression':
        wrapValue(e.consequent, out);
        wrapValue(e.alternate, out);
        return;
      case 'LogicalExpression':
        if (e.operator === '&&') wrapValue(e.right, out);
        else {
          wrapValue(e.left, out);
          wrapValue(e.right, out);
        }
        return;
      default:
    }
  }

  const addEdits = (list) => {
    for (const e of list) edits.push(e);
  };

  // ------------------------------------------------------------------------------------------ phase 1: single values

  // phase 0: rewrites that stand alone, made first so a sentence that holds them can carry them along
  walk(ast, (node) => {
    // toLocaleString('en-US')
    if (node.type === 'CallExpression' && node.callee.type === 'MemberExpression' && !node.callee.computed && node.callee.property.name === 'toLocaleString' && node.arguments.length === 1 && node.arguments[0].type === 'Literal' && node.arguments[0].value === 'en-US') {
      used.add('num');
      edits.push({ start: node.start, end: node.end, text: `num(${srcWith(node.callee.object)})` });
      return true;
    }
    // plural(n, 'one'[, 'many'])
    const helper = node.type === 'CallExpression' && node.callee.type === 'Identifier' && (node.callee.name === 'plural' || node.callee.name === 'describeCount' || (node.callee.name === 'count' && (rel === 'lib/ai.ts' || rel === 'lib/autocode.ts')));
    if (helper && node.arguments.length >= 2) {
      const [n, one, many] = node.arguments;
      if (one.type === 'Literal' && typeof one.value === 'string' && (!many || (many.type === 'Literal' && typeof many.value === 'string'))) {
        used.add('tn');
        const manyText = many ? many.value : one.value + 's';
        edits.push({ start: node.start, end: node.end, text: `tn(${srcWith(n)}, { one: ${quote('{count} ' + messageEscape(one.value))}, other: ${quote('{count} ' + messageEscape(manyText))} })` });
      } else {
        note(node, 'plural', 'plural() with a noun that is not a plain word');
      }
      return true;
    }
    return true;
  });

  // phase 1: the words
  walk(ast, (node) => {
    // attributes
    if (node.type === 'JSXAttribute' && node.name.type === 'JSXIdentifier' && ATTRS.has(node.name.name) && node.value) {
      const v = node.value;
      if (v.type === 'Literal' && typeof v.value === 'string') {
        const s = decodeEntities(v.value);
        if (looksLikeWords(s)) {
          used.add('t');
          edits.push({ start: v.start, end: v.end, text: `{t(${quote(s)})}` });
        }
      } else if (v.type === 'JSXExpressionContainer' && v.expression.type !== 'JSXEmptyExpression') {
        const out = [];
        wrapValue(v.expression, out);
        addEdits(out);
      }
      return true;
    }
    // children expressions that are not part of a sentence get their words wrapped (a run in phase 2 embeds these)
    if (node.type === 'JSXExpressionContainer') {
      const p = parentOf.get(node);
      if (p && (p.type === 'JSXElement' || p.type === 'JSXFragment') && node.expression.type !== 'JSXEmptyExpression') {
        const out = [];
        wrapValue(node.expression, out);
        addEdits(out);
      }
      return true;
    }
    // words handed to calls that show or say them
    if (node.type === 'CallExpression' || node.type === 'NewExpression') {
      const c = node.callee;
      const name = c.type === 'Identifier' ? c.name : c.type === 'MemberExpression' && !c.computed && c.property.type === 'Identifier' ? c.property.name : null;
      if (name && SINK_CALLS.has(name)) {
        const out = [];
        for (const a of node.arguments) wrapValue(a, out);
        addEdits(out);
      } else if (node.type === 'NewExpression' && name === 'ApiError' && node.arguments[1]) {
        const out = [];
        wrapValue(node.arguments[1], out);
        addEdits(out);
      }
      return true;
    }
    // words in a table
    if (node.type === 'Property' && node.kind === 'init' && !node.computed && !node.shorthand && !node.method) {
      const key = node.key.type === 'Identifier' ? node.key.name : node.key.type === 'Literal' ? String(node.key.value) : null;
      const parent = parentOf.get(node);
      const isPaletteName = key === 'name' && parent && parent.type === 'ObjectExpression' && parent.properties.some((q) => q.key && q.key.name === 'hex');
      if (key && (PROP_KEYS.has(key) || isPaletteName) && parent && parent.type === 'ObjectExpression' && !handedToEngine(parent)) {
        const out = [];
        wrapValue(node.value, out);
        if (out.length === 0) return true;
        if (atTopLevel(node)) {
          // evaluated when read, not when the file loads, so the language in use then is the one that counts
          const valueSrc = srcWith(node.value, out);
          const keySrc = node.key.type === 'Identifier' ? node.key.name : quote(String(node.key.value));
          edits.push({ start: node.start, end: node.end, text: `get ${node.key.type === 'Literal' ? '[' + keySrc + ']' : keySrc}() { return ${valueSrc}; }` });
        } else {
          addEdits(out);
        }
      }
      return true;
    }
    // a table of names keyed by an id: { coding: 'Coding', codebook: 'Codebook' }
    if (node.type === 'ObjectExpression' && atTopLevel(node) && parentOf.get(node)?.type === 'VariableDeclarator' && node.properties.length >= 2) {
      const plain = (q) => q.type === 'Property' && q.kind === 'init' && !q.computed && !q.method && !q.shorthand && q.value.type === 'Literal' && typeof q.value.value === 'string' && /^[A-Za-z][A-Za-z ,'’&/()]*$/.test(q.value.value) && q.key.name !== q.value.value;
      const converted = (q) => q.type === 'Property' && q.kind === 'get';
      const ok = node.properties.every((q) => plain(q) || converted(q)) && node.properties.filter(plain).length >= 1;
      if (ok) {
        for (const q of node.properties.filter(plain)) {
          const key = q.key.type === 'Identifier' ? q.key.name : quote(String(q.key.value));
          const bracket = q.key.type === 'Literal';
          used.add('t');
          edits.push({ start: q.start, end: q.end, text: `get ${bracket ? '[' + key + ']' : key}() { return t(${quote(q.value.value)}); }` });
        }
      }
      return true;
    }
    // words returned by a function
    if (node.type === 'ReturnStatement' && node.argument && opts.returns) {
      const out = [];
      wrapValue(node.argument, out);
      addEdits(out);
      return true;
    }
    if (node.type === 'ArrowFunctionExpression' && node.expression && opts.returns && node.body.type !== 'JSXElement') {
      const out = [];
      wrapValue(node.body, out);
      addEdits(out);
      return true;
    }
    return true;
  });

  // ------------------------------------------------------------------------------------------ phase 2: sentences between tags

  const consumed = new Set();
  const runEdits = [];

  const tagName = (el) => (el.openingElement.name.type === 'JSXIdentifier' ? el.openingElement.name.name : null);

  /** Does an element hold words (text or a string)? */
  const holdsWords = (el) => {
    let found = false;
    walk(el, (n) => {
      if (n.type === 'JSXText' && hasLetters(cleanJsxText(n.raw))) found = true;
      if (n.type === 'JSXAttribute') return false;
      return !found;
    });
    return found;
  };

  /** Can an element be written inside a sentence as a mark? Only plain inline elements whose insides are words. */
  const inlineSimple = (el) => {
    const tag = tagName(el);
    if (!tag || !INLINE_TAGS.has(tag)) return false;
    if (el.openingElement.attributes.some((a) => a.type === 'JSXSpreadAttribute')) return false;
    return el.children.every((c) => {
      if (c.type === 'JSXText') return true;
      if (c.type === 'JSXExpressionContainer') return c.expression.type !== 'JSXEmptyExpression' && !containsJsx(c);
      if (c.type === 'JSXElement') return inlineSimple(c);
      return false;
    });
  };

  function classify(child) {
    if (child.type === 'JSXText') {
      const cleaned = cleanJsxText(child.raw);
      return cleaned === '' ? { kind: 'ws' } : { kind: 'text', text: cleaned };
    }
    if (child.type === 'JSXExpressionContainer') {
      const e = child.expression;
      if (e.type === 'JSXEmptyExpression') return { kind: 'ws' };
      if (containsJsx(e)) return { kind: 'block' };
      if (e.type === 'Literal' && typeof e.value === 'string') return { kind: 'text', text: e.value };
      return { kind: 'expr', expr: e };
    }
    if (child.type === 'JSXElement') {
      const tag = tagName(child);
      if (child.openingElement.selfClosing) return { kind: 'deco', el: child };
      if (tag && inlineSimple(child)) return { kind: 'inline', el: child };
      if (tag && BLOCK_TAGS.has(tag)) return { kind: 'block' };
      if (!holdsWords(child)) return { kind: 'deco', el: child };
      return { kind: 'comp', el: child };
    }
    return { kind: 'block' };
  }

  let tagCounter;
  function partsFor(items, tagArgs) {
    const parts = [];
    for (const it of items) {
      if (it.kind === 'text') parts.push({ text: it.text });
      else if (it.kind === 'expr') parts.push({ expr: it.expr });
      else if (it.kind === 'ws') continue;
      else if (it.kind === 'inline') {
        const el = it.el;
        const tag = tagName(el);
        const seen = tagCounter.get(tag) ?? 0;
        tagCounter.set(tag, seen + 1);
        const name = seen === 0 ? tag : tag + (seen + 1);
        const opening = srcWith(el.openingElement);
        const closing = `</${tag}>`;
        tagArgs.push([name, `(inside) => ${opening}{inside}${closing}`]);
        parts.push({ open: name });
        const inner = el.children.map(classify);
        parts.push(...partsFor(inner, tagArgs));
        parts.push({ close: name });
        consumed.add(el);
      } else if (it.kind === 'deco') {
        const seen = tagCounter.get('x') ?? 0;
        tagCounter.set('x', seen + 1);
        const name = 'x' + (seen + 1);
        tagArgs.push([name, srcWith(it.el)]);
        parts.push({ self: name });
        consumed.add(it.el);
      }
    }
    return parts;
  }

  walk(ast, (node) => {
    if (consumed.has(node)) return false;
    if (node.type !== 'JSXElement' && node.type !== 'JSXFragment') return true;
    const tag = node.type === 'JSXElement' ? tagName(node) : null;
    if (tag && KEEP_TAGS.has(tag)) return true;
    const kids = node.children;
    const items = kids.map(classify);
    items.forEach((it, i) => {
      it.child = kids[i];
    });
    // split into segments at block-level pieces
    let seg = [];
    const flush = () => {
      if (seg.length) handleSegment(node, seg);
      seg = [];
    };
    for (let i = 0; i < items.length; i++) {
      const it = items[i];
      if (it.kind === 'block') flush();
      else if (it.kind === 'comp') {
        // an element with words in it that is not a mark: a separator, unless it sits inside a sentence
        const before = nearestText(items, i, -1);
        const after = nearestText(items, i, 1);
        if (before && after) {
          seg.push(it);
        } else flush();
      } else seg.push(it);
    }
    flush();
    return true;
  });

  function nearestText(items, i, dir) {
    // the item next to i (skipping nothing) is text or an expression that is not set off by a line break
    const j = i + dir;
    const it = items[j];
    if (!it) return false;
    const raw = it.child.type === 'JSXText' ? it.child.raw : null;
    if (it.kind === 'text' && raw !== null) {
      if (dir < 0) return !/\n[ \t]*$/.test(raw) && hasLetters(it.text);
      return !/^[ \t]*\r?\n/.test(raw) && hasLetters(it.text);
    }
    if (it.kind === 'expr' || it.kind === 'inline') return true;
    return false;
  }

  function handleSegment(parent, seg) {
    // trim decoration and whitespace from both ends of the segment
    let a = 0;
    let b = seg.length - 1;
    const edge = (it) => it.kind === 'ws' || it.kind === 'deco';
    while (a <= b && edge(seg[a])) a++;
    while (b >= a && edge(seg[b])) b--;
    if (a > b) return;
    const items = seg.slice(a, b + 1);
    // one element that holds all the words: it is its own sentence, found when the walk reaches it
    const meaningful = items.filter((it) => !(it.kind === 'ws' || (it.kind === 'text' && /^\s*$/.test(it.text))));
    if (meaningful.length === 1 && meaningful[0].kind === 'inline') return;
    if (items.some((it) => it.kind === 'comp')) {
      note(items.find((it) => it.kind === 'comp').child, 'mixed', 'words around a component that holds words');
      return;
    }
    if (!items.some((it) => it.kind === 'text' || it.kind === 'inline')) {
      // only expressions: nothing to translate, unless one is a count sentence (handled by the plural pieces)
      if (!items.some((it) => it.kind === 'expr' && pluralOf(it.expr))) return;
    }
    const tagArgs = [];
    tagCounter = new Map();
    // leading and trailing spaces of the segment stay outside the message
    const first = items[0];
    const last = items[items.length - 1];
    let start = first.child.start;
    let end = last.child.end;
    const adjusted = items.map((it) => ({ ...it }));
    if (first.kind === 'text' && first.child.type === 'JSXText') {
      const lead = first.child.raw.match(/^[ \t]*/)[0].length;
      const cleanedLead = first.text.match(/^ */)[0].length;
      if (cleanedLead > 0 && lead > 0) {
        start += lead;
        adjusted[0].text = first.text.slice(cleanedLead);
      } else if (cleanedLead > 0) {
        adjusted[0].text = first.text.slice(cleanedLead);
      }
    }
    if (last.kind === 'text' && last.child.type === 'JSXText') {
      const trail = last.child.raw.match(/[ \t]*$/)[0].length;
      const cleanedTrail = last.text.match(/ *$/)[0].length;
      const idx = adjusted.length - 1;
      if (cleanedTrail > 0) {
        if (trail > 0) end -= trail;
        adjusted[idx].text = adjusted[idx].text.slice(0, adjusted[idx].text.length - cleanedTrail);
      }
    }
    const parts = partsFor(adjusted.filter((it) => !(it.kind === 'text' && it.text === '')), tagArgs);
    if (!parts.some((p) => p.text !== undefined && hasWords(p.text)) && !parts.some((p) => p.expr && pluralOf(p.expr))) {
      // no word of two letters: a symbol, a unit, a single letter
      if (!parts.some((p) => p.text !== undefined && looksLikeWords(p.text))) {
        for (const it of items) if (it.kind === 'inline') consumed.delete(it.el);
        return;
      }
    }
    const call = buildCall(parts, tagArgs);
    if (!call) return;
    if (call.error) {
      note(first.child, 'mixed', call.error);
      for (const it of items) if (it.kind === 'inline') consumed.delete(it.el);
      return;
    }
    for (const it of items) {
      if (it.kind === 'expr') consumed.add(it.child);
      if (it.kind === 'text') consumed.add(it.child);
    }
    runEdits.push({ start, end, text: `{${call.code}}` });
  }

  // outermost edits win; edits inside a replaced run were embedded in its code
  const all = [...runEdits, ...edits];
  const kept = all.filter((e) => !all.some((o) => o !== e && o.start <= e.start && o.end >= e.end && (o.start < e.start || o.end > e.end)));
  // two edits on exactly the same range: keep the first
  const seen = new Set();
  const final = kept.filter((e) => {
    const k = `${e.start}:${e.end}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });

  if (final.length === 0) return { rel, changed: false, notes, text, newText: text, edits: 0, list: [] };
  let newText = applyEdits(text, final);

  // the import
  const need = [...used].sort();
  const modPath = path.posix.relative(path.posix.dirname(rel), 'i18n');
  const spec = modPath.startsWith('.') ? modPath : './' + modPath;
  newText = addImport(newText, need, spec, eol);
  const list = final.map((e) => ({ line: lineAt(e.start), from: text.slice(e.start, e.end).replace(/\s+/g, ' '), to: e.text }));
  return { rel, changed: true, notes, text, newText, edits: final.length, list };
}

/** Adds names to an import from the catalog module, or a new import line after the last import. */
export function addImport(text, names, spec, eol) {
  if (names.length === 0) return text;
  const existing = new RegExp(`import \\{([^}]*)\\} from '${spec.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}';`);
  const m = text.match(existing);
  if (m) {
    const have = m[1].split(',').map((s) => s.trim()).filter(Boolean);
    const merged = [...new Set([...have, ...names])].sort((a, b) => a.localeCompare(b));
    return text.replace(existing, `import { ${merged.join(', ')} } from '${spec}';`);
  }
  const line = `import { ${names.join(', ')} } from '${spec}';`;
  const ast = parseSource(text, 'x.tsx');
  let lastEnd = 0;
  for (const node of ast.body) {
    if (node.type === 'ImportDeclaration') lastEnd = node.end;
  }
  if (lastEnd === 0) {
    // after the comments at the top of the file, on a line of its own
    let at = 0;
    const lines = text.split(/(?<=\n)/);
    let inBlock = false;
    for (const l of lines) {
      const trimmed = l.trim();
      if (inBlock) {
        at += l.length;
        if (trimmed.includes('*/')) inBlock = false;
      } else if (trimmed.startsWith('//')) at += l.length;
      else if (trimmed.startsWith('/*')) {
        at += l.length;
        if (!trimmed.includes('*/')) inBlock = true;
      } else if (trimmed === '') {
        at += l.length;
      } else break;
    }
    const before = text.slice(0, at);
    return before + (before === '' || before.endsWith('\n') ? '' : eol) + line + eol + (before.trim() === '' ? '' : eol).slice(0, 0) + text.slice(at);
  }
  return text.slice(0, lastEnd) + eol + line + text.slice(lastEnd);
}

// ---------------------------------------------------------------------------------------------- main

function main() {
  const args = process.argv.slice(2);
  const write = args.includes('--write');
  const verbose = args.includes('--verbose');
  const returns = args.includes('--returns');
  const fi = args.indexOf('--files');
  const only = fi >= 0 ? new Set(args[fi + 1].split(',')) : null;
  let changed = 0;
  let total = 0;
  const allNotes = [];
  for (const rel of sourceFiles()) {
    if (only && !only.has(rel)) continue;
    let r;
    try {
      r = convertFile(rel, { returns });
    } catch (e) {
      console.error(`${rel}: ${e.stack || e}`);
      continue;
    }
    for (const n of r.notes) allNotes.push({ rel, ...n });
    if (!r.changed) continue;
    changed++;
    total += r.edits;
    if (write) fs.writeFileSync(path.join(SRC, rel), r.newText);
    if (verbose) console.log(`${rel}: ${r.edits} changes`);
    if (args.includes('--list')) for (const e of r.list) console.log(`${rel}:${e.line}
   - ${e.from.slice(0, 160)}
   + ${e.to.slice(0, 220)}`);
  }
  console.log(`${write ? 'Changed' : 'Would change'} ${total} places in ${changed} files.`);
  const byKind = new Map();
  for (const n of allNotes) byKind.set(n.kind, (byKind.get(n.kind) ?? 0) + 1);
  console.log('Left for a person:', [...byKind.entries()].map(([k, v]) => `${k} ${v}`).join(', ') || 'nothing');
  if (verbose || args.includes('--notes')) for (const n of allNotes) console.log(`  ${n.rel}:${n.line}  ${n.kind}  ${n.msg}`);
}

import { pathToFileURL } from 'node:url';
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
