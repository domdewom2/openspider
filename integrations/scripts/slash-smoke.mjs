import assert from 'node:assert/strict';
import { once } from 'node:events';
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { join, resolve } from 'node:path';
import { run } from './process.mjs';
import { root } from './sync-skill.mjs';

// A scripted Messages API replaces only the model. Claude Code must expand the
// installed command, discover the plugin MCP server and execute every real tool.
// This tests packaging and dispatch, not an LLM's ability to reason about issues.
// spec: AGENT-042
export async function slashSmoke(installed, temp, launchEnv, fixture) {
  const issue = fixture.issues.summary.issues.find((item) => item.id === 'response-codes-internal-client-error-4xx');
  assert(issue, '404 issue missing from baseline');
  const calls = [];
  const requests = [];
  let failure;
  let finalText;
  const command = readFileSync(join(installed.plugin, 'commands/openspider-issues.md'), 'utf8')
    .split('---')[2].trim().replaceAll('$ARGUMENTS', `${fixture.id} en`);
  // Local marketplaces can load in place rather than from installPath's cache.
  // Both roots belong to the installation; verify Claude's reported source below.
  const pluginRoots = [installed.plugin, join(root, 'claude-plugin')];
  let runtimeRoot;
  const plan = [
    ['Read', { file_path: join(installed.plugin, 'skills/openspider/SKILL.md') }],
    ['list_crawls', {}],
    ['crawl_overview', { crawl_id: fixture.id }],
    ['list_issues', { crawl_id: fixture.id, lang: 'en' }],
    ['query_urls', { crawl_id: fixture.id, tab: issue.query.tab, filter: issue.query.filter }],
    ['url_details', { crawl_id: fixture.id, url: `${fixture.origin}/missing.html`, sections: ['overview', 'inlinks', 'issues'] }],
  ];
  const results = [];
  const server = createServer(async (req, res) => {
    try {
      let raw = '';
      for await (const chunk of req) raw += chunk;
      const body = JSON.parse(raw);
      if (req.url.startsWith('/v1/messages/count_tokens')) {
        res.setHeader('Content-Type', 'application/json');
        res.end(JSON.stringify({ input_tokens: 100 }));
        return;
      }
      assert(req.url.startsWith('/v1/messages'), `Unexpected API route ${req.url}`);
      const prompt = body.messages.flatMap((m) => typeof m.content === 'string' ? [m.content] : m.content.filter((c) => c.type === 'text').map((c) => c.text)).join('\n');
      requests.push({ prompt, tools: (body.tools ?? []).map((tool) => tool.name) });
      // No --plugin-dir or copied prompt is passed: only a real installed slash
      // expansion can put the command body and substituted arguments here.
      const normalize = (text) => text.replaceAll('\r\n', '\n').replaceAll('\\', '/');
      runtimeRoot ??= pluginRoots.find((path) => normalize(prompt).includes(normalize(command.replaceAll('${CLAUDE_PLUGIN_ROOT}', path))));
      assert(runtimeRoot && normalize(prompt).includes(normalize(command.replaceAll('${CLAUDE_PLUGIN_ROOT}', runtimeRoot))), 'Installed slash command was not expanded with its plugin root and crawl selection');
      plan[0][1].file_path = join(runtimeRoot, 'skills/openspider/SKILL.md');
      if (calls.length) {
        const blocks = body.messages.flatMap((message) => Array.isArray(message.content) ? message.content : []);
        const result = blocks.find((c) => c.type === 'tool_result' && c.tool_use_id === `fixture_${calls.length}`);
        assert(result && !result.is_error, `Tool failed: ${JSON.stringify(result)}`);
        results.push(result);
      }

      let content;
      if (calls.length < plan.length) {
        const [name, input] = plan[calls.length];
        const tool = body.tools.find((t) => name === 'Read' ? t.name === name : t.name.endsWith(`__${name}`));
        assert(tool, `Installed plugin did not expose ${name}`);
        calls.push({ name: tool.name, input });
        content = [{ type: 'tool_use', id: `fixture_${calls.length}`, name: tool.name, input }];
      } else {
        const issueContent = results[3].content;
        const texts = typeof issueContent === 'string' ? [issueContent] : issueContent.filter((c) => c.type === 'text').map((c) => c.text);
        const envelopes = texts.flatMap((text) => { try { return [JSON.parse(text)]; } catch { return []; } });
        const received = envelopes.find((value) => value.summary?.issues);
        assert.deepEqual(received?.summary.issues, fixture.issues.summary.issues, 'Claude Code did not receive real issue data');
        assert(JSON.stringify(results[1]).includes(fixture.id), 'list_crawls did not discover the fixture crawl');
        assert(JSON.stringify(results[2]).toLowerCase().includes('completed'), 'crawl_overview did not report completion');
        assert(JSON.stringify(results[4]).includes('/missing.html'), 'Missing real query evidence');
        assert(JSON.stringify(results[5]).includes('/missing.html'), 'Missing real URL details');
        finalText = `${issue.priority}: ${issue.name}; affected URLs: ${issue.urls}.\nWhat: ${issue.what}\nWhy: ${issue.why}\nFix: ${issue.fix}\nExample: ${fixture.origin}/missing.html\nVerification: recrawl and check Client Error (4xx).`;
        content = [{ type: 'text', text: finalText }];
      }
      const message = { id: `msg_fixture_${requests.length}`, type: 'message', role: 'assistant', model: body.model, content, stop_reason: content[0].type === 'tool_use' ? 'tool_use' : 'end_turn', stop_sequence: null, usage: { input_tokens: 100, output_tokens: 100 } };
      if (body.stream) {
        res.setHeader('Content-Type', 'text/event-stream');
        const event = (type, data) => res.write(`event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n`);
        event('message_start', { message: { ...message, content: [], stop_reason: null } });
        content.forEach((block, index) => {
          event('content_block_start', { index, content_block: block.type === 'tool_use' ? { ...block, input: {} } : { type: 'text', text: '' } });
          event('content_block_delta', { index, delta: block.type === 'tool_use' ? { type: 'input_json_delta', partial_json: JSON.stringify(block.input) } : { type: 'text_delta', text: block.text } });
          event('content_block_stop', { index });
        });
        event('message_delta', { delta: { stop_reason: message.stop_reason, stop_sequence: null }, usage: message.usage });
        event('message_stop', {});
        res.end();
      } else {
        res.setHeader('Content-Type', 'application/json');
        res.end(JSON.stringify(message));
      }
    } catch (error) {
      failure ??= error;
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ type: 'error', error: { type: 'invalid_request_error', message: error.message } }));
    }
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  // Deliberately do not inherit provider credentials, OAuth tokens, proxy or
  // alternate-provider settings. The sole model endpoint is ephemeral loopback.
  const env = {};
  for (const key of Object.keys(launchEnv)) {
    if (/^(path|home|userprofile|systemroot|comspec|pathext|temp|tmp|tmpdir|appdata|localappdata|openspider_home|openspider_secret_store|openspider_binary)$/i.test(key)) env[key] = launchEnv[key];
  }
  Object.assign(env, {
    CLAUDE_CONFIG_DIR: installed.env.CLAUDE_CONFIG_DIR,
    ANTHROPIC_BASE_URL: `http://127.0.0.1:${server.address().port}`,
    ANTHROPIC_API_KEY: 'local-fixture-only',
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: '1',
    DISABLE_AUTOUPDATER: '1',
    ENABLE_TOOL_SEARCH: 'false',
  });
  try {
    const result = await run(installed.binary, [
      '-p', `/openspider:openspider-issues ${fixture.id} en`,
      '--output-format', 'stream-json', '--verbose', '--no-session-persistence',
      '--model', 'sonnet', '--max-turns', '10', '--tools', 'Read',
      '--allowedTools', 'Read', 'mcp__plugin_openspider_openspider',
      '--permission-mode', 'dontAsk',
    ], { cwd: temp, env }).catch((error) => ({ code: null, stdout: '', stderr: error.stack }));
    const artifact = resolve(root, '../target/integrations/slash-smoke.json');
    mkdirSync(resolve(artifact, '..'), { recursive: true });
    writeFileSync(artifact, JSON.stringify({ platform: process.platform, command: '/openspider:openspider-issues', calls, requests, stdout: result.stdout, stderr: result.stderr }, null, 2) + '\n');
    assert.ifError(failure);
    assert.equal(result.code, 0, `${result.stdout}\n${result.stderr}`);
    assert.equal(calls.length, plan.length, `Incomplete slash execution: ${result.stdout}\n${result.stderr}`);
    const messages = result.stdout.trim().split('\n').map((line) => JSON.parse(line));
    const init = messages.find((m) => m.type === 'system' && m.subtype === 'init');
    assert(init?.plugins.some((plugin) => plugin.name === 'openspider' && plugin.source === 'openspider@openspider-marketplace' && resolve(plugin.path) === resolve(runtimeRoot)), 'Command did not come from the marketplace-installed plugin');
    const completed = messages.find((m) => m.type === 'result');
    assert(completed && !completed.is_error, 'Claude Code did not complete successfully');
    assert.equal(completed.result, finalText, 'Issue output did not survive the Claude Code loop');
    if (process.env.GITHUB_STEP_SUMMARY) {
      appendFileSync(process.env.GITHUB_STEP_SUMMARY, `AGENT-042 (${process.platform}): installed /openspider:openspider-issues passed; skill read, five real MCP tool calls and fixture issue output verified. Model: scripted loopback Messages API.\n`);
    }
    return { status: 'passed', model: 'scripted loopback Messages API', tools: calls.map((call) => call.name), artifact: 'target/integrations/slash-smoke.json' };
  } finally {
    server.closeAllConnections();
    await new Promise((done) => server.close(done));
  }
}
