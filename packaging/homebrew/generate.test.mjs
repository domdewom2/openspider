import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { generate } from './generate.mjs';

const fixture = JSON.parse(readFileSync(new URL('./fixtures.json', import.meta.url)));
function setup(t) {
  const root = mkdtempSync(join(tmpdir(), 'p01-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const directory = join(root, 'assets');
  mkdirSync(directory);
  for (const [name, bytes] of Object.entries(fixture)) writeFileSync(join(directory, name), bytes);
  return { directory, output: join(root, 'out'), version: '1.0.0', repository: 'example/openspider', packageId: 'OpenSpider.CITest' };
}
const parse = text => JSON.parse(execFileSync('python3', ['-c', 'import json,sys,yaml;print(json.dumps(yaml.safe_load(sys.stdin)))'], { input: text }));

// spec: OUT-166 [M3]
test('Cask links the CLI at the stable app path and removes registrations without implicit zap', t => {
  const result = generate(setup(t));
  const cask = result['openspider.rb'];
  assert.match(cask, /app "OpenSpider.app"/);
  assert.match(cask, /binary "#\{appdir\}\/OpenSpider.app\/Contents\/MacOS\/openspider"/);
  assert.match(cask, /auto_updates true/);
  assert.match(cask, /uninstall launchctl: "de.openspider.task.\*"/);
  assert.match(cask, /delete: "~\/Library\/LaunchAgents\/de.openspider.task.\*.plist"/);
  assert.doesNotMatch(cask.split('uninstall launchctl:')[1].split('zap trash:')[0], /Application Support|Preferences|Caches/);
  assert.match(cask, /uninstall_preflight do/);
  assert.match(cask, /postflight do/);
  assert.match(cask, /zap trash:[\s\S]*org.openspider.desktop/);
  assert.match(cask, new RegExp(createHash('sha256').update(fixture['OpenSpider_1.0.0_x64.dmg']).digest('hex')));
  assert.match(cask, /\/v1.0.0\/OpenSpider_1.0.0_aarch64.dmg/);
  const ruby = spawnSync('ruby', ['-c'], { input: cask, encoding: 'utf8' });
  if (ruby.error?.code === 'ENOENT') t.diagnostic('ruby unavailable; syntax check remains mandatory in macOS CI');
  else assert.equal(ruby.status, 0, ruby.stderr);
});

// spec: OUT-266 [M3]
test('fixture generates MSI machine and NSIS user with quiet switches and exact SHA256', t => {
  const opts = setup(t);
  const result = generate(opts);
  const installer = parse(result['OpenSpider.CITest.installer.yaml']);
  assert.deepEqual(installer.Installers.map(i => [i.InstallerType, i.Scope]), [['msi', 'machine'], ['nullsoft', 'user']]);
  assert.equal(installer.Installers[0].InstallerSwitches.Silent, '/qn /norestart');
  assert.equal(installer.Installers[1].InstallerSwitches.Silent, '/S');
  assert.equal(installer.Installers[0].InstallerSwitches.Custom, 'ALLUSERS=1 ADDTOPATH=1');
  for (const item of installer.Installers) {
    const name = decodeURIComponent(new URL(item.InstallerUrl).pathname.split('/').at(-1));
    assert.equal(item.InstallerSha256, createHash('sha256').update(fixture[name]).digest('hex').toUpperCase());
    assert.doesNotMatch(JSON.stringify(item), /REMOVEUSERDATA/);
  }
  rmSync(join(opts.output, 'openspider.rb'));
  assert.match(execFileSync('python3', ['packaging/winget/validate.py', opts.output], { encoding: 'utf8' }), /ok: winget schema/);
});

test('K3-08 has no default ID; missing/ambiguous assets fail closed before writes', t => {
  const opts = setup(t);
  assert.throws(() => generate({ ...opts, packageId: undefined }), /K3-08/);
  assert.throws(() => generate({ ...opts, version: 'bad"\n' }), /Version/);
  assert.throws(() => generate({ ...opts, baseUrl: 'https://token@example.com' }), /Zugangsdaten/);
  writeFileSync(join(opts.directory, 'OpenSpider_2.0.0_x64-setup.exe'), 'other release');
  assert.throws(() => generate(opts), /genau ein Asset/);
  rmSync(join(opts.directory, 'OpenSpider_2.0.0_x64-setup.exe'));
  rmSync(join(opts.directory, 'OpenSpider_1.0.0_x64_en-US.msi'));
  assert.throws(() => generate(opts), /genau ein Asset/);
});

test('stale output rejected; changed bytes change checksum; local drafts allowed only on loopback', t => {
  const opts = setup(t);
  const before = generate(opts);
  assert.throws(() => generate(opts), /leer/);
  rmSync(opts.output, { recursive: true });
  writeFileSync(join(opts.directory, 'OpenSpider_1.0.0_x64-setup.exe'), 'changed');
  const after = generate({ ...opts, baseUrl: 'http://127.0.0.1:12345' });
  assert.notEqual(before['OpenSpider.CITest.installer.yaml'], after['OpenSpider.CITest.installer.yaml']);
  assert.throws(() => generate({ ...opts, baseUrl: 'http://example.com' }), /Loopback/);
});

test('publication has only release published trigger and defaults to no external writes', () => {
  const workflow = readFileSync('.github/workflows/publish-packages.yml', 'utf8');
  assert.match(workflow, /types: \[published\]/);
  assert.doesNotMatch(workflow, /^  (push|workflow_dispatch|workflow_call|pull_request):/m);
  assert.match(workflow, /RELEASE_PACKAGES_ENABLED/);
  assert.match(workflow, /if \[\[ "\$ENABLED" != true/);
  assert.match(workflow, /exit 0/);
  for (const line of workflow.split('\n').filter(l => l.includes('uses: actions/'))) {
    assert.match(line, /@[a-f0-9]{40} /);
  }
  const publish = spawnSync('bash', ['packaging/winget/publish.sh'], { env: { PATH: process.env.PATH }, encoding: 'utf8' });
  assert.notEqual(publish.status, 0);
  assert.match(publish.stderr, /RELEASE_TAG/);
  const parsed = parse(workflow);
  assert.equal(parsed.jobs.gate.environment, undefined);
  assert.equal(parsed.jobs.publish.needs, 'gate');
  const gate = parsed.jobs.gate.steps[0].run;
  const root = mkdtempSync(join(tmpdir(), 'p01-gate-'));
  try {
    const summary = join(root, 'summary');
    const output = join(root, 'output');
    const skipped = spawnSync('bash', ['-c', gate], { encoding: 'utf8', env: {
      PATH: process.env.PATH, ENABLED: 'true', PACKAGE_ID: '', TAP_REPOSITORY: '', WINGET_FORK: '',
      TAP_TOKEN: '', WINGET_TOKEN: '', GITHUB_OUTPUT: output, GITHUB_STEP_SUMMARY: summary,
    } });
    assert.equal(skipped.status, 0, skipped.stderr);
    assert.match(readFileSync(summary, 'utf8'), /übersprungen/);
    assert.match(skipped.stdout, /notice/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
