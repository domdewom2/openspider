import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';

// A protocol client for deterministic packaging tests, not an LLM evaluator.
export class McpClient {
  constructor(binary, args, env) {
    this.child = spawn(binary, args, { env, shell: false, stdio: ['pipe', 'pipe', 'pipe'] });
    this.nextId = 0;
    this.pending = new Map();
    this.stderr = '';
    this.failure = null;
    this.child.stderr.setEncoding('utf8').on('data', (chunk) => { this.stderr += chunk; });
    const fail = (error) => {
      this.failure = error;
      for (const request of this.pending.values()) request.reject(error);
      this.pending.clear();
    };
    this.child.on('error', fail);
    this.child.on('exit', (code) => fail(new Error(`MCP exited ${code}: ${this.stderr}`)));
    this.child.stdin.on('error', fail);
    createInterface({ input: this.child.stdout }).on('line', (line) => {
      try {
        const message = JSON.parse(line);
        if (message.id !== undefined) {
          const pending = this.pending.get(message.id);
          if (!pending) return;
          this.pending.delete(message.id);
          if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
          else pending.resolve(message.result);
        }
      } catch (error) { fail(error); }
    });
  }

  request(method, params = {}) {
    if (this.failure) return Promise.reject(this.failure);
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`MCP timeout: ${method}; ${this.stderr}`)); }, 30_000);
      this.pending.set(id, {
        resolve: (value) => { clearTimeout(timer); resolve(value); },
        reject: (error) => { clearTimeout(timer); reject(error); },
      });
      this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
    });
  }

  async initialize() {
    await this.request('initialize', { protocolVersion: '2024-11-05', capabilities: {}, clientInfo: { name: 'openspider-integrations-test', version: '1.0.0' } });
    this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
  }

  async call(name, args = {}) {
    const result = await this.request('tools/call', { name, arguments: args });
    assert(!result.isError, `${name}: ${JSON.stringify(result)}`);
    const envelope = JSON.parse(result.content[0].text);
    return { ...envelope, content: result.content };
  }

  async close() {
    if (!this.child.pid) {
      this.child.stdin.destroy();
      return;
    }
    if (this.child.exitCode !== null || this.child.signalCode !== null) return;
    const closed = once(this.child, 'exit');
    this.child.stdin.end();
    const timer = setTimeout(() => this.child.kill(), 2000);
    try { await closed; } finally { clearTimeout(timer); }
  }
}
