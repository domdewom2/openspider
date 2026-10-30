import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { checkVersion, readVersion } from './check-version.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const version = process.argv[2];
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-beta\.(?:0|[1-9]\d*)|-rc\.[1-9]\d*)?$/.test(version ?? '')) {
  throw new Error('Version erwartet: x.y.z, x.y.z-beta.N oder x.y.z-rc.N');
}
const cargo = resolve(root, 'Cargo.toml');
const text = readFileSync(cargo, 'utf8');
const previous = readVersion(root);
// Only local workspace records change. Registry versions, checksums and dependency resolution stay intact.
const lockPath = resolve(root, 'Cargo.lock');
const lock = readFileSync(lockPath, 'utf8').replace(/\[\[package\]\]\n[\s\S]*?(?=\n\[\[package\]\]|$(?![\s\S]))/g, block => {
  if (/^source\s*=/m.test(block)) return block;
  if (!block.includes(`version = "${previous}"`)) throw new Error('Lokaler Cargo.lock-Eintrag hat abweichende Version');
  return block.replace(/^version = "[^"]+"/m, `version = "${version}"`);
});
const files = ['package.json', 'apps/desktop/package.json', 'apps/desktop/src-tauri/tauri.conf.json', 'package-lock.json'];
const documents = files.map(path => [resolve(root, path), JSON.parse(readFileSync(resolve(root, path), 'utf8'))]);
writeFileSync(cargo, text.replace(/(\[workspace\.package\][\s\S]*?^version\s*=\s*")[^"]+(".*$)/m, (_, before, after) => `${before}${version}${after}`));
writeFileSync(lockPath, lock);
// Replace only the version in workspace.package, leaving dependency constraints intact.
// npm's workspace and root lock records must match their manifests for npm ci.
for (const [path, doc] of documents) {
  doc.version = version;
  if (doc.packages) {
    doc.packages[''].version = version;
    doc.packages['apps/desktop'].version = version;
  }
  writeFileSync(path, JSON.stringify(doc, null, 2) + '\n');
}
checkVersion(root);
