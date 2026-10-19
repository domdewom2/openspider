// Launch without a shell: executable paths may contain spaces or shell metacharacters.
import { spawn } from 'node:child_process';
import { constants } from 'node:os';

const binary = process.env.OPENSPIDER_BINARY || 'openspider';
const child = spawn(binary, ['mcp'], { stdio: 'inherit', shell: false });
child.on('error', (error) => {
  console.error(`OpenSpider could not start (${error.code}). Install the OpenSpider CLI and put openspider${process.platform === 'win32' ? '.exe' : ''} on PATH, or set OPENSPIDER_BINARY to its absolute executable path.`);
  console.error('Source installation: in an OpenSpider repository checkout, run: cargo install --path crates/os-cli --locked');
  console.error('Verify with: openspider --version and openspider mcp --print-config claude-code. Restart Claude Code after changing PATH. See the plugin README.md for macOS and Windows setup.');
  process.exitCode = 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => child.kill(signal));
}
child.on('exit', (code, signal) => {
  process.exitCode = code ?? 128 + (constants.signals[signal] ?? 1);
});
