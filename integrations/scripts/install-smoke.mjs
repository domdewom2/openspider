import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { root } from './sync-skill.mjs';
import { json } from './validate.mjs';
import { run } from './process.mjs';

export async function installSmoke(temp) {
  assert(process.env.OPENSPIDER_CLAUDE_BINARY, 'Set OPENSPIDER_CLAUDE_BINARY to the Claude Code executable; installation and slash execution are required');
  const binary = resolve(process.env.OPENSPIDER_CLAUDE_BINARY);
  const config = join(temp, 'claude-config');
  const env = { ...process.env, CLAUDE_CONFIG_DIR: config, CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: '1' };
  for (const args of [
    ['plugin', 'validate', root],
    ['plugin', 'validate', join(root, 'claude-plugin')],
    ['plugin', 'marketplace', 'add', root],
    ['plugin', 'install', 'openspider@openspider-marketplace', '--scope', 'user'],
  ]) {
    const result = await run(binary, args, { env, cwd: temp });
    assert.equal(result.code, 0, `claude ${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
  }
  const installed = json(join(config, 'plugins/installed_plugins.json'));
  const entry = installed.plugins['openspider@openspider-marketplace'];
  assert(entry?.length, 'Plugin missing from Claude installation registry');
  return { plugin: entry[0].installPath, binary, env };
}
