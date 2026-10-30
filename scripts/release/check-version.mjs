import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

export function readVersion(root) {
  const cargo = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
  const section = cargo.match(/^\[workspace\.package\]\s*\n([\s\S]*?)(?=^\[|(?![\s\S]))/m)?.[1];
  const version = section?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) throw new Error('workspace.package.version fehlt');
  return version;
}
export function checkVersion(root) {
  const expected = readVersion(root);
  for (const file of ['package.json', 'apps/desktop/package.json', 'apps/desktop/src-tauri/tauri.conf.json', 'package-lock.json']) {
    const doc = JSON.parse(readFileSync(resolve(root, file), 'utf8'));
    if (doc.version !== expected) throw new Error(`${file}: ${doc.version} statt ${expected}`);
    if (doc.packages && [doc.packages[''].version, doc.packages['apps/desktop'].version].some(v => v !== expected)) {
      throw new Error(`${file}: Workspace-Lockversion abweichend`);
    }
  }
  return expected;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(`Versionen gleich: ${checkVersion(resolve(dirname(fileURLToPath(import.meta.url)), '../..'))}`);
}
