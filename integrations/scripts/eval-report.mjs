import assert from 'node:assert/strict';
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { json } from './validate.mjs';
import { root } from './sync-skill.mjs';

export function report(tasks, results = []) {
  assert(Array.isArray(results));
  const byId = new Map();
  for (const result of results) {
    assert(tasks.some((task) => task.id === result.id), `Unknown task ${result.id}`);
    assert(!byId.has(result.id), `Duplicate result ${result.id}`);
    assert(['pass', 'fail', 'skipped'].includes(result.status), 'Invalid result status');
    assert(typeof result.evidence === 'string' && result.evidence.trim(), 'Evidence/reason required');
    byId.set(result.id, result);
  }
  const cases = tasks.map((task) => byId.get(task.id) ?? { id: task.id, status: 'not_run' });
  const passed = cases.filter((task) => task.status === 'pass').length;
  const evaluated = cases.filter((task) => ['pass', 'fail'].includes(task.status)).length;
  return { total: tasks.length, evaluated, passed, success_rate: evaluated ? passed / evaluated : null, completion_rate: evaluated / tasks.length, cases };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const result = report(json(join(root, 'skills/openspider/evals/tasks.json')), process.env.OPENSPIDER_EVAL_RESULTS ? json(resolve(process.env.OPENSPIDER_EVAL_RESULTS)) : []);
  const output = resolve(root, '../target/integrations/eval-report.json');
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
  const summary = `Skill evals: ${result.passed}/${result.evaluated} evaluated passed; ${result.total} total. Success rate: ${result.success_rate === null ? 'not measured (optional agent evaluation not run)' : `${(result.success_rate * 100).toFixed(1)}%`}. Completion: ${(result.completion_rate * 100).toFixed(1)}%.\n`;
  console.log(summary);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary);
}
