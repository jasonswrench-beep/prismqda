// Prints the source line of each string the audit still finds: node scripts/i18n/show.mjs [file ...]
import { auditFile } from './audit.mjs';
import { readSource, sourceFiles } from './ast.mjs';
const only = process.argv.slice(2);
for (const rel of sourceFiles()) {
  if (only.length && !only.some((o) => rel.includes(o))) continue;
  const found = auditFile(rel);
  if (!found.length) continue;
  const lines = readSource(rel).split(/\r?\n/);
  console.log(`\n== ${rel}`);
  const seen = new Set();
  for (const f of found) {
    if (seen.has(f.line)) continue;
    seen.add(f.line);
    console.log(`${f.line}: ${lines[f.line - 1].trim().slice(0, 330)}`);
  }
}
