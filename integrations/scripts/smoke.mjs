import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import Ajv from 'ajv';
import { root } from './sync-skill.mjs';
import { registry } from './validate.mjs';
import { run } from './process.mjs';
import { McpClient } from './mcp-client.mjs';
import { installSmoke } from './install-smoke.mjs';

const suffix = process.platform === 'win32' ? '.exe' : '';
const metadata = await run('cargo', ['metadata', '--no-deps', '--locked', '--format-version', '1'], { cwd: resolve(root, '..') });
assert.equal(metadata.code, 0, metadata.stderr);
const target = JSON.parse(metadata.stdout).target_directory;
const binary = resolve(process.env.OPENSPIDER_TEST_BINARY || join(target, `debug/openspider${suffix}`));
const testsite = resolve(process.env.OPENSPIDER_TESTSITE_BINARY || join(target, `debug/os-testsite${suffix}`));
const temp = mkdtempSync(join(tmpdir(), 'openspider integration '));
const env = { ...process.env, OPENSPIDER_HOME: join(temp, 'home'), OPENSPIDER_SECRET_STORE: 'memory' };
const pathKey = Object.keys(env).find((key) => key.toLowerCase() === 'path') ?? 'PATH';
let site;
let client;
try {
  const installed = await installSmoke(temp);
  const plugin = installed ?? join(root, 'claude-plugin');
  const launcher = join(plugin, 'scripts/launch.mjs');
  const command = readFileSync(join(plugin, 'commands/openspider-issues.md'), 'utf8');
  assert(command.includes('`list_issues`') && command.includes('`list_crawls`'));

  // spec: AGENT-042 — missing binary: useful stderr, no non-protocol stdout.
  const missing = await run(process.execPath, [launcher], {
    env: { ...env, [pathKey]: join(temp, 'empty-path'), OPENSPIDER_BINARY: '' },
  });
  assert.equal(missing.code, 1);
  assert.equal(missing.stdout, '');
  assert.match(missing.stderr, /cargo install --path crates\/os-cli --locked/);
  assert.match(missing.stderr, /PATH/);

  // spec: AGENT-042 — real out-basic-site, ephemeral loopback, no user state.
  site = spawn(testsite, [join(root, '../tests/fixtures/out-basic-site'), '0'], { env, stdio: ['ignore', 'pipe', 'pipe'] });
  let siteError = '';
  site.stderr.setEncoding('utf8').on('data', (chunk) => { siteError += chunk; });
  const origin = await new Promise((resolveOrigin, reject) => {
    const timer = setTimeout(() => reject(new Error(`Fixture startup timeout: ${siteError}`)), 15_000);
    site.once('error', (error) => { clearTimeout(timer); reject(error); });
    site.once('exit', (code) => { clearTimeout(timer); reject(new Error(`Fixture exited ${code}: ${siteError}`)); });
    createInterface({ input: site.stdout }).on('line', (line) => {
      const match = line.match(/-> (http:\/\/\S+)/);
      if (match) { clearTimeout(timer); resolveOrigin(match[1]); }
    });
  });
  const crawl = await run(binary, ['--crawl', `${origin}/`, '--headless', '--set', 'threads=1', '--set', 'robots.retries=0'], { env, cwd: temp });
  assert.equal(crawl.code, 0, `${crawl.stdout}\n${crawl.stderr}`);
  assert(readdirSync(join(env.OPENSPIDER_HOME, 'crawls')).some((file) => file.endsWith('.ospider')));

  // Launch through PATH, as installed; the executable override has its own launcher test.
  const launchEnv = { ...env, [pathKey]: dirname(binary) + delimiter + (env[pathKey] ?? ''), OPENSPIDER_BINARY: '' };
  client = new McpClient(process.execPath, [launcher], launchEnv);
  await client.initialize();
  const tools = (await client.request('tools/list')).tools;
  const expected = Object.entries(registry()).filter(([, cls]) => cls === 'read').map(([name]) => name).sort();
  assert.deepEqual(tools.map((tool) => tool.name).sort(), expected);
  // Collect read schemas; validate all examples after discovering the full register below.
  const schemas = new Map(tools.map((tool) => [tool.name, tool.inputSchema]));
  const discovered = await client.call('list_crawls');
  const id = discovered.summary.crawls[0]?.crawl_id;
  assert(id, 'Fixture crawl missing from list_crawls');
  const overview = await client.call('crawl_overview', { crawl_id: id });
  assert.equal(overview.summary.state.toLowerCase(), 'completed');
  const issues = await client.call('list_issues', { crawl_id: id, lang: 'en' });
  assert(issues.total > 0, 'Basic fixture must produce real issues');
  const rows = await client.call('query_urls', { crawl_id: id, tab: 'Response Codes', filter: 'Client Error (4xx)' });
  assert(rows.total >= 1);
  assert(JSON.stringify(rows.content).includes('/missing.html'), 'Missing expected 404 evidence');
  await client.close();

  // Full register is exposed solely in this isolated test process, not the plugin config.
  client = new McpClient(binary, ['mcp', '--allow', 'read,crawl,write,cost'], env);
  await client.initialize();
  for (const tool of (await client.request('tools/list')).tools) schemas.set(tool.name, tool.inputSchema);
  assert.deepEqual([...schemas.keys()].sort(), Object.keys(registry()).sort());
  const examples = readFileSync(join(root, 'skills/openspider/references/tools.md'), 'utf8').replaceAll('\r\n', '\n').match(/```json\n([\s\S]*?)\n```/)[1];
  for (const line of examples.split('\n')) {
    const example = JSON.parse(line);
    const check = new Ajv({ strict: false }).compile(schemas.get(example.tool));
    assert(check(example.arguments), `${example.tool}: ${JSON.stringify(check.errors)}`);
  }
  console.log(JSON.stringify({ marketplace_install: installed ? 'passed' : 'not run (set OPENSPIDER_CLAUDE_BINARY)', fixture: 'out-basic-site', missing_binary: 'passed', mcp_issues_workflow: 'passed', live_schema_examples: 'passed', agent_slash_execution: 'optional manual evaluation' }, null, 2));
} finally {
  if (client) await client.close();
  if (site?.pid && site.exitCode === null && site.signalCode === null) {
    const closed = once(site, 'exit');
    site.kill();
    await closed;
  }
  rmSync(temp, { recursive: true, force: true });
}
