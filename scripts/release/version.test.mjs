import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, cpSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('version source propagates to all manifests and detects drift', () => {
  const root = mkdtempSync(join(tmpdir(), 'openspider-version-'));
  try {
    mkdirSync(join(root, 'scripts/release'), { recursive: true });
    mkdirSync(join(root, 'apps/desktop/src-tauri'), { recursive: true });
    for (const file of ['set-version.mjs', 'check-version.mjs']) {
      cpSync(fileURLToPath(new URL(file, import.meta.url)), join(root, 'scripts/release', file));
    }
    writeFileSync(join(root, 'Cargo.toml'), '[workspace.package]\nversion = "0.1.0"\n[workspace.dependencies]\nsemver = "1"\n');
    for (const file of ['package.json', 'apps/desktop/package.json', 'apps/desktop/src-tauri/tauri.conf.json']) {
      writeFileSync(join(root, file), JSON.stringify({ version: '0.1.0' }));
    }
    writeFileSync(join(root, 'package-lock.json'), JSON.stringify({ version: '0.1.0', packages: { '': { version: '0.1.0' }, 'apps/desktop': { version: '0.1.0' } } }));
    const run = (script, ...args) => spawnSync(process.execPath, [join(root, 'scripts/release', script), ...args], { cwd: tmpdir(), encoding: 'utf8' });
    for (const version of ['1.0.0-beta.1', '1.0.0-rc.2', '1.0.0']) {
      assert.equal(run('set-version.mjs', version).status, 0);
      assert.match(run('check-version.mjs').stdout, new RegExp(version.replaceAll('.', '\\.')));
      const cargo = readFileSync(join(root, 'Cargo.toml'), 'utf8');
      assert.ok(cargo.includes(`version = "${version}"`));
      assert.ok(cargo.includes('semver = "1"'));
    }
    for (const version of ['01.0.0', '1.0.0-beta.01', '1.0.0;echo unsafe']) {
      assert.notEqual(run('set-version.mjs', version).status, 0);
    }
    writeFileSync(join(root, 'apps/desktop/package.json'), '{"version":"9.0.0"}');
    assert.notEqual(run('check-version.mjs').status, 0);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
