import { spawn } from 'node:child_process';

// Avoid shell interpolation on both macOS and Windows, including paths with spaces.
export function run(binary, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { shell: false, ...options, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    const timer = setTimeout(() => { child.kill(); reject(new Error(`Timed out: ${binary}`)); }, 120_000);
    child.stdout.setEncoding('utf8').on('data', (chunk) => { stdout += chunk; });
    child.stderr.setEncoding('utf8').on('data', (chunk) => { stderr += chunk; });
    child.on('error', (error) => { clearTimeout(timer); reject(error); });
    child.on('close', (code) => { clearTimeout(timer); resolve({ code, stdout, stderr }); });
  });
}
