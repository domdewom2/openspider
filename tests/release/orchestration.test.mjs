import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, readdirSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { manifests } from '../../scripts/release/manifest.mjs';
import { checksums } from '../../scripts/release/checksums.mjs';
import { releaseNotes } from '../../scripts/release/notes.mjs';

const root = resolve(import.meta.dirname, '../..');
const fixtures = ['OpenSpider_aarch64.app.tar.gz', 'OpenSpider_x86_64.app.tar.gz',
  'OpenSpider_1.0.0-beta.0_x64-setup.exe', 'OpenSpider_1.0.0-beta.0_x64_en-US.msi'];
// Public test-only, format-valid signature envelope; no claim of cryptographic verification.
const signature = Buffer.from('untrusted comment: fixture\nRWRmaXh0dXJl\ntrusted comment: fixture\nZml4dHVyZQ==\n').toString('base64');
function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'openspider-release-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  for (const name of fixtures) {
    writeFileSync(join(directory, name), `fixture:${name}`);
    writeFileSync(join(directory, `${name}.sig`), signature);
  }
  return { directory, version: '1.0.0-beta.0', repository: 'example/openspider', notes: 'Fixture notes', date: '2026-10-30T00:00:00Z' };
}

// spec: OUT-261 [M3]
test('Manifest liefert pro Architektur und Windows-Installer genau das passende signierte Asset', t => {
  const options = fixture(t);
  manifests(options);
  const manifest = JSON.parse(readFileSync(join(options.directory, 'latest-beta.json')));
  assert.deepEqual(Object.keys(manifest.platforms), ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64-nsis', 'windows-x86_64-msi']);
  Object.values(manifest.platforms).forEach((entry, i) => {
    assert.equal(entry.signature, signature);
    assert.equal(entry.url, `https://github.com/example/openspider/releases/download/v1.0.0-beta.0/${fixtures[i]}`);
  });
  assert.equal(readFileSync(join(options.directory, 'latest.json'), 'utf8'), readFileSync(join(options.directory, 'latest-beta.json'), 'utf8'));
});

test('Stabiler Release aktualisiert auch den Beta-Endpunkt', t => {
  const options = fixture(t);
  manifests({ ...options, version: '1.0.0' });
  for (const file of ['latest.json', 'latest-beta.json']) {
    assert.equal(JSON.parse(readFileSync(join(options.directory, file))).version, '1.0.0');
  }
});

test('Ohne Secrets grün und unsigniert, ohne leere oder veraltete Manifeste', t => {
  const options = fixture(t);
  manifests(options);
  rmSync(join(options.directory, `${fixtures[0]}.sig`));
  assert.throws(() => manifests(options), /fehlen/);
  assert.deepEqual(manifests({ ...options, allowUnsigned: true }), { status: 'unsigniert', missing: ['darwin-aarch64'], files: [] });
  assert.ok(!readdirSync(options.directory).some(n => n.startsWith('latest')));
});

test('Mehrdeutige Installer und ungültige Signaturen werden abgelehnt', t => {
  const options = fixture(t);
  writeFileSync(join(options.directory, 'Other_x64-setup.exe'), 'duplicate');
  assert.throws(() => manifests(options), /Mehrdeutig/);
  rmSync(join(options.directory, 'Other_x64-setup.exe'));
  writeFileSync(join(options.directory, `${fixtures[0]}.sig`), 'invalid');
  assert.throws(() => manifests({ ...options, allowUnsigned: true }), /Signatur/);
});

test('SHA256SUMS erfasst jedes Asset einschließlich Manifesten und Signaturen reproduzierbar', async t => {
  const options = fixture(t);
  manifests(options);
  const result = await checksums(options.directory);
  assert.equal(await checksums(options.directory), result);
  const lines = result.trim().split('\n');
  assert.equal(lines.length, readdirSync(options.directory).length - 1);
  for (const line of lines) {
    const [hash, name] = line.split('  ');
    assert.equal(hash, createHash('sha256').update(readFileSync(join(options.directory, name))).digest('hex'));
  }
});

test('Release-Notes wählen ausschließlich die angeforderte Changelog-Sektion', () => {
  assert.equal(releaseNotes('# Log\n## [Unreleased]\nfuture\n## [1.0.0-beta.0] - today\nBeta\n## [0.1.0]\nold', '1.0.0-beta.0'), 'Beta\n');
  assert.throws(() => releaseNotes('## [Unreleased]\nfuture', '1.0.0'), /fehlt/);
});

// spec: OUT-273 [M3]
test('Linux-CLI-Archiv aus linux-core bleibt unverändert im gesammelten Release und in SHA256SUMS', async t => {
  const options = fixture(t);
  const incoming = join(options.directory, 'incoming');
  const output = join(options.directory, 'out');
  mkdirSync(incoming);
  const archive = 'openspider_1.0.0-beta.0_linux-x86_64.tar.gz';
  // Real tar fixture; the workflow separately executes the built/extracted Linux binary.
  writeFileSync(join(incoming, 'openspider'), '#!/bin/sh\necho openspider 1.0.0-beta.0\n', { mode: 0o755 });
  assert.equal(spawnSync('tar', ['-czf', join(incoming, archive), '-C', incoming, 'openspider']).status, 0);
  writeFileSync(join(incoming, 'verify-app.log'), 'not distributable');
  const result = spawnSync(process.execPath, ['scripts/release/assets.mjs', incoming, output, options.version], { cwd: root, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(readdirSync(output), [archive]);
  assert.deepEqual(readFileSync(join(output, archive)), readFileSync(join(incoming, archive)));
  assert.match(await checksums(output), new RegExp(archive.replaceAll('.', '\\.')));
  const extracted = join(options.directory, 'extracted');
  mkdirSync(extracted);
  assert.equal(spawnSync('tar', ['-xzf', join(output, archive), '-C', extracted]).status, 0);
  assert.equal(spawnSync(join(extracted, 'openspider'), ['--version'], { encoding: 'utf8' }).stdout.trim(), 'openspider 1.0.0-beta.0');
});

test('Asset-Kollisionen werden abgelehnt statt Signaturen oder Installer zu überschreiben', t => {
  const options = fixture(t);
  const nested = join(options.directory, 'nested');
  mkdirSync(nested);
  writeFileSync(join(nested, fixtures[0]), 'collision');
  const output = join(options.directory, 'out');
  // Output must not be nested inside input in production; use a separate sibling here.
  const result = spawnSync(process.execPath, ['scripts/release/assets.mjs', options.directory, `${output}-external`, options.version], { cwd: root, encoding: 'utf8' });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Kollision/);
});

// spec: OUT-255 [M3]
test('D01-Hook meldet fehlende Archivierung, führt vorhandenes Skript aus und propagiert Fehler', t => {
  const options = fixture(t);
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');
  const body = workflow.split('      - name: Archiv-Kompatibilitäts-Fixture (D01)')[1]
    .split('        run: |\n')[1].split('      - uses:')[0]
    .split('\n').map(line => line.startsWith('          ') ? line.slice(10) : line).join('\n');
  mkdirSync(join(options.directory, 'release-reports'));
  const summary = join(options.directory, 'summary.txt');
  const run = () => spawnSync('bash', ['-c', body], { cwd: options.directory,
    env: { ...process.env, VERSION: options.version, GITHUB_STEP_SUMMARY: summary }, encoding: 'utf8' });
  assert.equal(run().status, 0);
  assert.match(readFileSync(summary, 'utf8'), /STUB\/TODO\(D01\).*keine Archiv-Kompatibilität nachgewiesen/);
  assert.match(readFileSync(join(options.directory, 'release-reports/release-notes.md'), 'utf8'), /STUB\/TODO\(D01\)/);
  mkdirSync(join(options.directory, 'scripts/release'), { recursive: true });
  const hook = join(options.directory, 'scripts/release/archive-compat-fixture.mjs');
  writeFileSync(hook, `import {writeFileSync} from 'node:fs'; writeFileSync('hook-args.json', JSON.stringify(process.argv.slice(2)));`);
  assert.equal(run().status, 0);
  assert.deepEqual(JSON.parse(readFileSync(join(options.directory, 'hook-args.json'))),
    ['--version', '1.0.0-beta.0', '--out', 'release-reports/compat']);
  writeFileSync(hook, 'process.exit(7);');
  assert.equal(run().status, 7);
});

test('Release-Erstellung bleibt Draft, normale Beta, kein Latest und Trockenlauf standardmäßig an', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');
  assert.match(workflow, /--verify-tag --draft --prerelease=false --latest=false/);
  assert.match(workflow, /if: github.event_name == 'push' \|\| !inputs.dry_run/);
  assert.match(workflow, /dry_run:[\s\S]*?default: true/);
  for (const line of workflow.split('\n').filter(line => line.includes('uses:') && !line.includes('uses: ./'))) {
    assert.match(line, /@[0-9a-f]{40} # v\d/);
  }
});

test('Trockenlauf v1.0.0-beta.0: Skript-CLIs erzeugen beide Manifeste, Notes und vollständig prüfbare Assets', async t => {
  const options = fixture(t);
  const notes = join(options.directory, 'notes.md');
  function run(script, ...args) {
    const result = spawnSync(process.execPath, [`scripts/release/${script}.mjs`, ...args], { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result;
  }
  run('notes', options.version, notes);
  const expected = readFileSync(notes, 'utf8');
  rmSync(notes);
  const outsideNotes = join(options.directory, '..', `${options.directory.split('/').at(-1)}-notes.md`);
  t.after(() => rmSync(outsideNotes, { force: true }));
  writeFileSync(outsideNotes, expected);
  run('manifest', options.directory, options.version, options.repository, outsideNotes, options.date);
  run('checksums', options.directory);
  assert.equal(JSON.parse(readFileSync(join(options.directory, 'latest-beta.json'))).notes, expected);
  assert.equal(JSON.parse(readFileSync(join(options.directory, 'latest.json'))).version, '1.0.0-beta.0');
  assert.match(readFileSync(join(options.directory, 'updater-status.txt'), 'utf8'), /signierte Updater-Artefakte/);
  const verification = spawnSync('sha256sum', ['-c', 'SHA256SUMS'], { cwd: options.directory, encoding: 'utf8' });
  assert.equal(verification.status, 0, verification.stderr);
  assert.equal(verification.stdout.trim().split('\n').length, 11);
});
