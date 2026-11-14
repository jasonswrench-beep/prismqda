// License check for the interface's JavaScript packages.
//
// Rules (specification 2.2):
//  - Everything that ships (the packages reachable from "dependencies") must carry a permissive
//    license: MIT, Apache-2.0, BSD, ISC, 0BSD, Zlib, or Unicode. Nothing else, ever.
//  - Packages used only to build or test (everything else installed) may also be permissive, plus
//    the explicit exceptions listed below, each with its reason. They never ship.
//  - A license expression such as "(MIT OR Apache-2.0)" passes if one alternative is allowed;
//    "A AND B" needs both.
//
// Run: node scripts/check-licenses.mjs   (exit code 1 on a problem)

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const nodeModules = path.join(root, 'node_modules');

const PERMISSIVE = new Set(
  [
    'MIT',
    'MIT-0',
    'Apache-2.0',
    'BSD-2-Clause',
    'BSD-3-Clause',
    '0BSD',
    'ISC',
    'Zlib',
    'Unicode-DFS-2016',
    'Unicode-3.0',
    'BlueOak-1.0.0',
    'CC0-1.0',
    'Python-2.0',
  ].map((s) => s.toLowerCase()),
);

/** Packages that may use a license that is not on the permissive list, only as build or test tools. */
const DEV_ONLY_EXCEPTIONS = {
  '@axe-core/playwright': { license: 'MPL-2.0', why: 'accessibility checks run by the browser check; never imported by the interface' },
  'axe-core': { license: 'MPL-2.0', why: 'the engine behind @axe-core/playwright; used only by the browser check' },
  lightningcss: { license: 'MPL-2.0', why: 'a CSS processor that Vite uses at build time; its code is not part of the built interface' },
  'lightningcss-win32-x64-msvc': { license: 'MPL-2.0', why: 'the Windows binary of lightningcss, used only while building' },
};

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

/** The license text of a package.json, as one string (possibly an SPDX expression). */
function licenseOf(pkg) {
  if (typeof pkg.license === 'string') return pkg.license;
  if (pkg.license && typeof pkg.license.type === 'string') return pkg.license.type;
  if (Array.isArray(pkg.licenses)) return pkg.licenses.map((l) => (typeof l === 'string' ? l : l.type)).join(' OR ');
  return '';
}

/** Evaluates an SPDX-style expression: OR needs one allowed, AND needs all. */
function expressionAllowed(expr, allowed) {
  const tokens = expr.replace(/\(/g, ' ( ').replace(/\)/g, ' ) ').split(/\s+/).filter(Boolean);
  let i = 0;
  const parseOr = () => {
    let ok = parseAnd();
    while (tokens[i] && tokens[i].toUpperCase() === 'OR') {
      i++;
      const next = parseAnd();
      ok = ok || next;
    }
    return ok;
  };
  const parseAnd = () => {
    let ok = parseAtom();
    while (tokens[i] && tokens[i].toUpperCase() === 'AND') {
      i++;
      const next = parseAtom();
      ok = ok && next;
    }
    return ok;
  };
  const parseAtom = () => {
    const t = tokens[i++];
    if (t === '(') {
      const ok = parseOr();
      if (tokens[i] === ')') i++;
      return ok;
    }
    if (!t) return false;
    return allowed(t.replace(/\+$/, '').toLowerCase());
  };
  return parseOr();
}

/** Every installed package: its folder, name, version, and license. */
function scanInstalled() {
  const found = new Map();
  const visit = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (!entry.isDirectory() && !entry.isSymbolicLink()) continue;
      if (entry.name.startsWith('.')) continue;
      const full = path.join(dir, entry.name);
      if (entry.name.startsWith('@')) {
        visit(full);
        continue;
      }
      const manifest = path.join(full, 'package.json');
      if (fs.existsSync(manifest)) {
        const real = fs.realpathSync(full);
        const pkg = readJson(manifest);
        if (pkg.name && !found.has(real)) found.set(real, { dir: real, name: pkg.name, version: pkg.version, license: licenseOf(pkg), pkg });
        const nested = path.join(full, 'node_modules');
        if (fs.existsSync(nested)) visit(nested);
      }
    }
  };
  if (!fs.existsSync(nodeModules)) throw new Error('node_modules is missing. Run: npm install --ignore-scripts --no-audit --no-fund');
  visit(nodeModules);
  return found;
}

/** Finds the installed folder for a dependency name, looking outward from the folder that needs it. */
function resolveFrom(fromDir, name) {
  let dir = fromDir;
  for (;;) {
    const candidate = path.join(dir, 'node_modules', name);
    if (fs.existsSync(path.join(candidate, 'package.json'))) return fs.realpathSync(candidate);
    const parent = path.dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

/** The set of package folders reachable from the given dependency names. */
function closure(names, installed) {
  const seen = new Set();
  const queue = names.map((n) => resolveFrom(root, n)).filter(Boolean);
  while (queue.length) {
    const dir = queue.pop();
    if (seen.has(dir)) continue;
    seen.add(dir);
    const info = installed.get(dir);
    const pkg = info ? info.pkg : readJson(path.join(dir, 'package.json'));
    const deps = { ...(pkg.dependencies ?? {}), ...(pkg.optionalDependencies ?? {}) };
    for (const dep of Object.keys(deps)) {
      const next = resolveFrom(dir, dep);
      if (next) queue.push(next);
    }
  }
  return seen;
}

const rootPkg = readJson(path.join(root, 'package.json'));
const installed = scanInstalled();
const shipped = closure(Object.keys(rootPkg.dependencies ?? {}), installed);
const problems = [];
const shippedLicenses = new Map();
const devLicenses = new Map();
const usedExceptions = new Set();

for (const info of installed.values()) {
  const isShipped = shipped.has(info.dir);
  const label = `${info.name}@${info.version}`;
  const expr = info.license;
  const bucket = isShipped ? shippedLicenses : devLicenses;
  bucket.set(expr || '(none)', (bucket.get(expr || '(none)') ?? 0) + 1);
  if (!expr) {
    problems.push(`${label} (${isShipped ? 'shipped' : 'build/test only'}) declares no license`);
    continue;
  }
  if (expressionAllowed(expr, (id) => PERMISSIVE.has(id))) continue;
  const exception = DEV_ONLY_EXCEPTIONS[info.name];
  if (!isShipped && exception && expressionAllowed(expr, (id) => PERMISSIVE.has(id) || id === exception.license.toLowerCase())) {
    usedExceptions.add(info.name);
    continue;
  }
  problems.push(
    isShipped
      ? `${label} ships and has the license "${expr}", which is not permissive`
      : `${label} (build/test only) has the license "${expr}", which is not permissive and not on the exception list`,
  );
}

// The exception packages must never be reachable from what ships, or imported by the interface.
for (const name of Object.keys(DEV_ONLY_EXCEPTIONS)) {
  for (const dir of shipped) {
    if (installed.get(dir)?.name === name) problems.push(`${name} is reachable from "dependencies", so it would ship`);
  }
  if (rootPkg.dependencies && name in rootPkg.dependencies) problems.push(`${name} is listed under "dependencies"; it may only be a devDependency`);
}
const walkSource = (dir) => {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walkSource(full);
    else if (/\.(tsx?|css|html)$/.test(entry.name) && /axe-core|@axe-core/.test(fs.readFileSync(full, 'utf8'))) {
      problems.push(`${path.relative(root, full)} mentions axe; the interface must not use it`);
    }
  }
};
walkSource(path.join(root, 'src'));
const dist = path.join(root, 'dist');
if (fs.existsSync(dist)) {
  const scanDist = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) scanDist(full);
      // (The built CSS mentions "lightningcss" in the names of two custom properties it adds; that is not code.)
      else if (/\.(js|html)$/.test(entry.name) && /axe-core|axe\.min|@axe-core/.test(fs.readFileSync(full, 'utf8'))) {
        problems.push(`the built file ${path.relative(root, full)} contains axe code`);
      }
    }
  };
  scanDist(dist);
}

const show = (title, map) => {
  console.log(title);
  for (const [lic, n] of [...map.entries()].sort((a, b) => b[1] - a[1])) console.log(`  ${String(n).padStart(4)}  ${lic}`);
};
console.log(`Checked ${installed.size} installed packages: ${shipped.size} are reachable from "dependencies" (they ship), ${installed.size - shipped.size} are only for building and testing.`);
show('Licenses of the packages that ship:', shippedLicenses);
show('Licenses of the build and test packages:', devLicenses);
if (usedExceptions.size) {
  console.log('Build and test packages allowed by an explicit exception:');
  for (const name of usedExceptions) console.log(`  ${name}: ${DEV_ONLY_EXCEPTIONS[name].license}, ${DEV_ONLY_EXCEPTIONS[name].why}`);
}
if (problems.length) {
  console.error('\nLicense problems:');
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log('\nOK: nothing that ships has a license outside the permissive list.');
