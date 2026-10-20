import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { checkReferences, checkSchema, frontmatter, json, validate } from '../scripts/validate.mjs';
import { root, syncSkill } from '../scripts/sync-skill.mjs';
import { report } from '../scripts/eval-report.mjs';
import { run } from '../scripts/process.mjs';

// spec: AGENT-041
test('skill format, references, bundled files, evals and actual registry agree', () => {
  const result = validate();
  assert(result.tasks >= 10);
  assert(result.tools.list_issues === 'read');
});

// spec: AGENT-041
test('format validation rejects invalid YAML, names and missing descriptions', () => {
  for (const text of [
    'name: openspider',
    '---\nname: Bad_Name\ndescription: Audit\n---\nBody',
    '---\nname: openspider\n---\nBody',
    '---\nname: openspider\nname: repeated\ndescription: Audit\n---\nBody',
  ]) assert.throws(() => frontmatter(text, true));
});

// spec: AGENT-041
test('broken references and stale Windows-compatible bundle copies fail validation', (t) => {
  const temp = mkdtempSync(join(tmpdir(), 'openspider-contract-'));
  t.after(() => rmSync(temp, { recursive: true, force: true }));
  const skill = join(temp, 'skills/openspider');
  cpSync(join(root, 'skills/openspider'), skill, { recursive: true });
  syncSkill(false, temp);
  const file = join(skill, 'SKILL.md');
  writeFileSync(file, readFileSync(file, 'utf8') + '\n[Broken](references/not-there.md)\n');
  assert.throws(() => checkReferences(skill));
  assert.throws(() => syncSkill(true, temp), /Stale bundled skill/);
});

// spec: AGENT-041, AGENT-042
test('a nonexistent tool in skill instructions fails even when the bundle is in sync', (t) => {
  const temp = mkdtempSync(join(tmpdir(), 'openspider-tool-ref-'));
  t.after(() => rmSync(temp, { recursive: true, force: true }));
  for (const dir of ['skills', 'claude-plugin', '.claude-plugin']) {
    cpSync(join(root, dir), join(temp, dir), { recursive: true });
  }
  const file = join(temp, 'skills/openspider/SKILL.md');
  writeFileSync(file, readFileSync(file, 'utf8').replace('`list_crawls`', '`invented_tool`'));
  syncSkill(false, temp);
  assert.throws(() => validate(temp), /Unknown tool\/field reference invented_tool/);
});

// spec: AGENT-042
test('plugin manifests reject wrong types, invalid sources and permission overrides', () => {
  const plugin = json(join(root, 'claude-plugin/.claude-plugin/plugin.json'));
  assert.throws(() => checkSchema('plugin', { ...plugin, version: 1 }));
  const market = json(join(root, '.claude-plugin/marketplace.json'));
  market.plugins[0].source = '../outside';
  assert.throws(() => checkSchema('marketplace', market));
  assert.throws(() => checkSchema('mcp', { mcpServers: { openspider: { command: 'node', args: ['--allow', 'cost'] } } }));
});

// spec: AGENT-042
test('binary override preserves arguments, stdout, stderr and nonzero exit status without a shell', async (t) => {
  const temp = mkdtempSync(join(tmpdir(), 'openspider launch space '));
  t.after(() => rmSync(temp, { recursive: true, force: true }));
  // Node is a real executable; its mcp argument loads this disposable test program.
  writeFileSync(join(temp, 'mcp'), 'console.log(JSON.stringify({protocol:"test",argument:process.argv[1]})); console.error("diagnostic"); process.exitCode=7;');
  const result = await run(process.execPath, [join(root, 'claude-plugin/scripts/launch.mjs')], {
    cwd: temp,
    env: { ...process.env, OPENSPIDER_BINARY: process.execPath },
  });
  assert.equal(result.code, 7);
  assert.equal(JSON.parse(result.stdout).protocol, 'test');
  assert(JSON.parse(result.stdout).argument.endsWith('mcp'));
  assert.equal(result.stderr.trim(), 'diagnostic');
});

// spec: AGENT-041
test('optional eval report separates success from completion and does not invent results', () => {
  const tasks = json(join(root, 'skills/openspider/evals/tasks.json'));
  assert.equal(report(tasks).success_rate, null);
  const result = report(tasks, [
    { id: tasks[0].id, status: 'pass', evidence: 'fixture transcript A' },
    { id: tasks[1].id, status: 'fail', evidence: 'fixture transcript B: missing source URL' },
    { id: tasks[2].id, status: 'skipped', evidence: 'no agent runner configured' },
  ]);
  assert.equal(result.success_rate, 0.5);
  assert.equal(result.completion_rate, 2 / tasks.length);
  assert.throws(() => report(tasks, [{ id: tasks[0].id, status: 'pass', evidence: '' }]));
  assert.throws(() => report(tasks, [{ id: 'unknown', status: 'pass', evidence: 'x' }]));
});
