// A tiny test harness: named steps, checks that record instead of throwing, and a summary.

export function createHarness() {
  const results = [];
  let current = null;
  let checks = 0;

  async function step(name, fn) {
    const entry = { name, failures: [], checks: 0, started: Date.now() };
    current = entry;
    try {
      await fn();
    } catch (e) {
      entry.failures.push(`threw: ${e && e.stack ? e.stack.split('\n').slice(0, 6).join('\n') : e}`);
    }
    current = null;
    entry.ms = Date.now() - entry.started;
    results.push(entry);
    const mark = entry.failures.length === 0 ? 'ok  ' : 'FAIL';
    console.log(`${mark} ${String(results.length).padStart(3)} ${name} (${entry.checks} checks, ${entry.ms} ms)`);
    for (const f of entry.failures) console.log(`       ${f}`);
  }

  function check(condition, message) {
    checks++;
    if (current) current.checks++;
    if (!condition) {
      if (current) current.failures.push(message);
      else console.log(`FAIL (outside a step) ${message}`);
    }
    return Boolean(condition);
  }

  function equal(actual, expected, message) {
    const ok = JSON.stringify(actual) === JSON.stringify(expected);
    return check(ok, `${message}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }

  function summary() {
    const failed = results.filter((r) => r.failures.length > 0);
    console.log('');
    console.log(`${results.length - failed.length} of ${results.length} steps passed, ${checks} checks run.`);
    if (failed.length) {
      console.log('Failed steps:');
      for (const f of failed) console.log(`  - ${f.name}`);
    }
    return { steps: results.length, failed: failed.length, checks };
  }

  return { step, check, equal, summary, results };
}
