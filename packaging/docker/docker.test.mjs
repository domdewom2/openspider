// Vertragstests für das Docker-Image (OUT-274): Dockerfile, Chromium-Starter, Build-Kontext, Workflow und Spec sowie die
// reinen Hilfsfunktionen von smoke.mjs und oci.mjs gegen Fixtures. Ohne Docker lauffähig: node --test packaging/docker/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { cell, column, mcpRequests, mcpSummary, normalizeText, parseArgs, parseCsv, summaryTable, testsitePort, versionMatches } from './smoke.mjs';
import { checkAttestations, readLayout, sameImage } from './oci.mjs';

const root = resolve(import.meta.dirname, '../..');
const read = path => readFileSync(join(root, path), 'utf8');
const dockerfile = () => read('packaging/docker/Dockerfile');
const workflow = () => read('.github/workflows/docker.yml');

// Dockerfile in logische Anweisungen zerlegen (Fortsetzungszeilen zusammengefasst, Kommentare entfernt).
function instructions(text) {
  const out = [];
  let current = '';
  for (const raw of text.split('\n')) {
    const line = raw.trimEnd();
    if (!current && (line.trim().startsWith('#') || !line.trim())) continue;
    if (line.trim().startsWith('#')) continue;
    current += (current ? ' ' : '') + line.replace(/\\$/, '').trim();
    if (!line.endsWith('\\')) { out.push(current); current = ''; }
  }
  if (current) out.push(current);
  return out.map(text => ({ keyword: text.split(/\s+/)[0].toUpperCase(), text }));
}

// Anweisungen je Stufe; die globalen ARGs vor dem ersten FROM liegen in der Stufe "global".
function stages(text) {
  const result = [{ name: 'global', from: null, steps: [] }];
  for (const step of instructions(text)) {
    if (step.keyword === 'FROM') {
      const match = /^FROM\s+(\S+)(?:\s+AS\s+(\S+))?$/i.exec(step.text);
      assert.ok(match, `FROM-Syntax: ${step.text}`);
      result.push({ name: match[2] ?? `#${result.length}`, from: match[1], steps: [] });
    } else {
      result.at(-1).steps.push(step);
    }
  }
  return result;
}

// Jobs eines Workflows als Textblöcke (zwei Leerzeichen Einrückung unter `jobs:`).
function jobs(text) {
  const body = text.split('\njobs:\n')[1];
  const blocks = {};
  let name = null;
  for (const line of body.split('\n')) {
    const match = /^ {2}([a-z][\w-]*):\s*$/.exec(line);
    if (match) { name = match[1]; blocks[name] = ''; continue; }
    if (name) blocks[name] += `${line}\n`;
  }
  return blocks;
}

// spec: OUT-274
test('Basis-Image ist mit Tag und Digest gepinnt, kein latest, und jede Stufe baut darauf oder auf einer eigenen Stufe auf', () => {
  const all = stages(dockerfile());
  const base = all[0].steps.find(s => s.keyword === 'ARG' && s.text.startsWith('ARG DEBIAN_IMAGE='));
  assert.ok(base, 'globales ARG DEBIAN_IMAGE fehlt');
  const image = base.text.split('=')[1];
  assert.match(image, /^debian:trixie-\d{8}-slim@sha256:[0-9a-f]{64}$/);
  assert.doesNotMatch(dockerfile(), /:latest\b/);
  const names = new Set(all.map(s => s.name));
  for (const stage of all.slice(1)) {
    assert.ok(stage.from === '${DEBIAN_IMAGE}' || names.has(stage.from), `${stage.name}: FROM ${stage.from}`);
  }
  assert.deepEqual(all.slice(1).map(s => s.name), ['fetch', 'archive', 'verify', 'runtime']);
});

// spec: OUT-274
test('Laufzeit-Image läuft als fester Nicht-root-Benutzer 10001 mit eigenem Konto', () => {
  const runtime = stages(dockerfile()).at(-1);
  const users = runtime.steps.filter(s => s.keyword === 'USER');
  assert.equal(users.length, 1);
  assert.equal(users[0].text, 'USER 10001:10001');
  assert.equal(runtime.steps.at(-3).keyword, 'USER', 'nach USER nur noch ENTRYPOINT und CMD');
  const setup = runtime.steps.find(s => s.keyword === 'RUN' && s.text.includes('useradd'));
  assert.match(setup.text, /groupadd --system --gid 10001 openspider/);
  assert.match(setup.text, /useradd --system --uid 10001 --gid 10001 /);
  assert.match(setup.text, /install -d -o 10001 -g 10001 -m 0755 \/data \/config/);
  for (const stage of stages(dockerfile()).slice(1)) {
    assert.ok(!stage.steps.some(s => s.keyword === 'USER' && /\b(root|0)(:|$)/.test(s.text.split(/\s+/)[1])), `${stage.name}: USER root`);
  }
});

// spec: OUT-274
test('SHA-256 ohne Standardwert und vor dem Entpacken geprüft; Inhalt und Version des Archivs werden verglichen', () => {
  const all = stages(dockerfile());
  const verify = all.find(s => s.name === 'verify');
  assert.ok(verify.steps.some(s => s.text === 'ARG OPENSPIDER_SHA256'), 'ARG OPENSPIDER_SHA256 ohne Standardwert');
  assert.ok(!instructions(dockerfile()).some(s => s.keyword === 'ARG' && s.text.includes('OPENSPIDER_SHA256=')), 'kein Standardwert für die Prüfsumme');
  const run = verify.steps.find(s => s.keyword === 'RUN').text;
  const check = run.indexOf('sha256sum --check --strict');
  assert.ok(check > 0, 'sha256sum --check fehlt');
  assert.ok(run.indexOf('[0-9a-f]{64}') < check, 'Format der Prüfsumme wird zuerst geprüft');
  assert.ok(check < run.indexOf('tar -tzf') && check < run.indexOf('tar -xzf'), 'Prüfsumme vor jedem Lesen des Archivs');
  assert.match(run, /"LICENSE NOTICE THIRD_PARTY_NOTICES\.txt openspider "/);
  assert.match(run, /openspider \$\{OPENSPIDER_VERSION\} \("\*", linux-x86_64\)/);
  // Der Download erlaubt nur HTTPS, auch bei Weiterleitungen.
  const download = all.find(s => s.name === 'archive').steps.find(s => s.keyword === 'RUN').text;
  assert.match(download, /--proto '=https' --proto-redir '=https'/);
  assert.match(download, /releases\/download\/v\$\{OPENSPIDER_VERSION\}\/\$\{name\}/);
  assert.match(download, /name="openspider_\$\{OPENSPIDER_VERSION\}_linux-x86_64\.tar\.gz"/);
});

// spec: OUT-274
test('Labels, Einstiegspunkt, Arbeitsordner, Volume und Umgebung des Laufzeit-Images', () => {
  const runtime = stages(dockerfile()).at(-1);
  const label = runtime.steps.find(s => s.keyword === 'LABEL').text;
  for (const [key, value] of [
    ['org.opencontainers.image.source', 'https://github.com/wojcikdominik/openspider'],
    ['org.opencontainers.image.licenses', 'Apache-2.0'],
    ['org.opencontainers.image.version', '${OPENSPIDER_VERSION}'],
    ['org.opencontainers.image.revision', '${OPENSPIDER_REVISION}'],
    ['org.opencontainers.image.title', 'OpenSpider'],
  ]) assert.ok(label.includes(`${key}="${value}"`), key);
  assert.match(label, /org\.opencontainers\.image\.description="[^"]*No web UI[^"]*"/);
  const steps = runtime.steps.map(s => s.text);
  assert.ok(steps.includes('ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/openspider"]'));
  assert.ok(steps.includes('CMD ["--help"]'));
  assert.ok(steps.includes('WORKDIR /data'));
  assert.ok(steps.includes('VOLUME ["/config"]'));
  const env = runtime.steps.find(s => s.keyword === 'ENV').text;
  for (const pair of ['OPENSPIDER_HOME=/config', 'OPENSPIDER_CHROMIUM=/usr/local/bin/openspider-chromium', 'OPENSPIDER_SECRET_STORE=memory', 'OPENSPIDER_DISABLE_UPDATE_CHECK=1']) {
    assert.ok(env.includes(pair), pair);
  }
  const install = runtime.steps.find(s => s.keyword === 'RUN' && s.text.includes('apt-get install')).text;
  for (const pkg of ['ca-certificates', 'chromium', 'fonts-liberation', 'fonts-noto-color-emoji', 'tini']) assert.match(install, new RegExp(`\\s${pkg}\\s`));
  assert.match(install, /--no-install-recommends/);
  assert.match(install, /rm -rf \/var\/lib\/apt\/lists\/\*/);
  assert.ok(steps.includes('COPY --from=verify /out/LICENSE /out/NOTICE /out/THIRD_PARTY_NOTICES.txt /usr/share/doc/openspider/'));
});

// spec: OUT-274
test('Kein Geheimnis im Image: keine Schlüssel, Token oder Passwörter in ENV, ARG oder Labels', () => {
  for (const step of instructions(dockerfile()).filter(s => ['ENV', 'ARG', 'LABEL'].includes(s.keyword))) {
    const names = [...step.text.matchAll(/([A-Za-z_][A-Za-z0-9_.]*)=/g)].map(m => m[1]);
    for (const name of names) {
      if (name === 'OPENSPIDER_SECRET_STORE') continue;
      assert.doesNotMatch(name, /KEY|TOKEN|PASSWORD|PASSPHRASE|SECRET|CREDENTIAL/i, `${step.keyword} ${name}`);
    }
  }
  assert.match(stages(dockerfile()).at(-1).steps.find(s => s.keyword === 'ENV').text, /OPENSPIDER_SECRET_STORE=memory/);
});

// spec: OUT-274
test('Build-Kontext enthält nur den Chromium-Starter; jede COPY aus dem Kontext ist erlaubt', () => {
  const ignore = read('packaging/docker/.dockerignore').split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'));
  assert.deepEqual(ignore, ['*', '!openspider-chromium']);
  const fromContext = instructions(dockerfile()).filter(s => ['COPY', 'ADD'].includes(s.keyword) && !/--from=/.test(s.text));
  assert.deepEqual(fromContext.map(s => s.text), ['COPY --chmod=0755 openspider-chromium /usr/local/bin/openspider-chromium']);
  assert.ok(!instructions(dockerfile()).some(s => s.keyword === 'ADD'), 'kein ADD');
});

// spec: OUT-274
test('Chromium-Starter: ohne Sandbox nur ohne OPENSPIDER_CHROMIUM_SANDBOX=1, /dev/shm umgangen, exec auf Debian-Chromium', () => {
  const script = read('packaging/docker/openspider-chromium');
  assert.ok(script.startsWith('#!/bin/sh\n'));
  assert.match(script, /^set -eu$/m);
  assert.match(script, /if \[ "\$\{OPENSPIDER_CHROMIUM_SANDBOX:-0\}" != "1" \]; then\n {2}set -- --no-sandbox "\$@"\nfi/);
  assert.match(script, /^exec \/usr\/bin\/chromium --disable-dev-shm-usage "\$@"$/m);
  assert.equal(script.match(/^exec /gm).length, 1);
});

// spec: OUT-274
test('Workflow: alle fremden Actions per Commit-SHA gepinnt, oberste Rechte nur Lesen', () => {
  const text = workflow();
  const uses = [...text.matchAll(/^\s*(?:-\s+)?uses:\s*(\S+)(.*)$/gm)].map(m => [m[1], m[2]]);
  assert.ok(uses.length >= 6);
  for (const [ref, comment] of uses) {
    assert.match(ref, /^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/, ref);
    assert.match(comment, /# v\d/, `${ref}: Versionskommentar`);
  }
  assert.match(text, /\npermissions:\n {2}contents: read\n\n/);
});

// spec: OUT-274
test('Workflow: Push nach ghcr.io nur im Job publish, nur per workflow_dispatch mit publish=true aus einem Release und mit Environment-Freigabe', () => {
  const text = workflow();
  const blocks = jobs(text);
  assert.deepEqual(Object.keys(blocks), ['contract', 'image', 'publish']);
  // Eingang publish: Boolean, Standard aus.
  assert.match(text, /\n {6}publish:\n {8}description: [^\n]+\n {8}type: boolean\n {8}default: false\n/);
  const publish = blocks.publish;
  const condition = /\n {4}if: >-\n([\s\S]*?)\n {4}runs-on:/.exec(publish)[1].replace(/\s+/g, ' ').trim();
  assert.equal(condition, "github.event_name == 'workflow_dispatch' && inputs.publish && inputs.source == 'release' && github.ref == 'refs/heads/main' && needs.image.outputs.identical == 'true'");
  assert.match(publish, /\n {4}needs: image\n/);
  assert.match(publish, /\n {4}environment: ghcr\n/);
  assert.match(publish, /required_reviewers/, 'Environment ohne Pflichtprüfer wird abgelehnt');
  assert.match(publish, /existiert bereits; nicht überschrieben/);
  // Schreibrechte für Pakete und Registry-Zugriffe gibt es nur in publish.
  for (const [name, block] of Object.entries(blocks)) {
    if (name === 'publish') continue;
    assert.doesNotMatch(block, /packages: write|skopeo|docker\/login-action|ghcr\.io/, name);
    assert.doesNotMatch(block, /push: true/, name);
  }
  assert.match(publish, /packages: write/);
  // Bauen ohne Push; Smoke-Test als eigenes Skript; SBOM/Provenance am geprüften Build.
  const image = blocks.image;
  assert.equal((image.match(/push: false/g) ?? []).length, 2);
  assert.match(image, /node packaging\/docker\/smoke\.mjs --image openspider:smoke/);
  assert.match(image, /provenance: mode=max\n {10}sbom: true\n {10}outputs: type=oci,/);
  assert.match(image, /node packaging\/docker\/oci\.mjs /);
  // Kein automatischer Push bei Tags: Tag-Läufe kommen nur über workflow_run in den Job image.
  assert.doesNotMatch(text, /\n {2}push:\n(?: {4}.*\n)* {4}tags:/);
  assert.doesNotMatch(text, /\n {2}release:\n/);
});

// spec: OUT-274
test('Workflow hängt am Release-Lauf: Name, Artefakt release-assets mit Linux-Archiv und SHA256SUMS', () => {
  const release = read('.github/workflows/release.yml');
  const name = /^name: (.+)$/m.exec(release)[1];
  assert.match(workflow(), new RegExp(`workflow_run:\\n {4}workflows: \\["${name}"\\]\\n {4}types: \\[completed\\]`));
  assert.match(release, /tar -czf "release-artifacts\/openspider_\$\{VERSION\}_linux-x86_64\.tar\.gz"/);
  assert.match(release, /node scripts\/release\/checksums\.mjs release-assets/);
  assert.match(release, /name: release-assets\n {10}path: release-assets\//);
  assert.match(workflow(), /name: release-assets\n {10}path: \$\{\{ runner\.temp \}\}\/release-assets/);
});

// spec: OUT-274
test('Spec: OUT-274 steht im Abschnitt 13 mit Akzeptanzkriterien, Inhaltsübersicht nennt den Bereich', () => {
  const spec = read('docs/spec/06-output-ops.md');
  const section13 = spec.split('\n## 13 Plattformanforderungen\n')[1].split('\n## 14 ')[0];
  const entry = section13.split('\n### OUT-274 ')[1];
  assert.ok(entry, 'OUT-274 fehlt in Abschnitt 13');
  for (const phrase of ['nicht root', 'SHA-256-Prüfsumme des Archivs wird vor dem Entpacken geprüft', 'Kein Push in eine Registry ohne ausdrückliche Freigabe', 'Smoke-Test', 'keine Weboberfläche']) {
    assert.ok(entry.toLowerCase().includes(phrase.toLowerCase()), phrase);
  }
  assert.match(spec, /\| OUT-260 – OUT-274 \|[^\n]*Docker-Image \|/);
  assert.equal(spec.match(/^### OUT-274 /gm).length, 1);
});

// spec: OUT-274
test('Smoke-Hilfen: Argumente, Port von os-testsite, Versionszeile, Tabelle', () => {
  assert.deepEqual(parseArgs(['--image', 'openspider:smoke', '--version', '1.0.0-beta.3', '--testsite', 't']),
    { image: 'openspider:smoke', version: '1.0.0-beta.3', testsite: 't', report: '' });
  assert.throws(() => parseArgs(['--image', 'x', '--version', 'latest', '--testsite', 't']), /Ungültige Version/);
  assert.throws(() => parseArgs(['--image', 'x', '--testsite', 't']), /--version fehlt/);
  assert.equal(testsitePort('out-basic-site -> http://127.0.0.1:41234\n'), 41234);
  assert.equal(testsitePort('noch nichts'), null);
  assert.ok(versionMatches('openspider 1.0.0-beta.3 (f90bb133, linux-x86_64)\n', '1.0.0-beta.3'));
  assert.ok(!versionMatches('openspider 1.0.0-beta.30 (f90bb133, linux-x86_64)', '1.0.0-beta.3'));
  assert.ok(!versionMatches('openspider 1.0.0-beta.3 (f90bb133, macos-aarch64)', '1.0.0-beta.3'));
  const table = summaryTable([{ name: 'a|b', ok: true, detail: 'x\ny' }, { name: 'c', ok: false, detail: '' }], 'T');
  assert.match(table, /\| a\\\|b \| ok \| x y \|/);
  assert.match(table, /\| c \| FEHLER \|/);
});

// spec: OUT-274
test('Smoke-Hilfen lesen die Golden-CSVs der Fixtures (out-basic-site, out-b3-diff)', () => {
  const basic = parseCsv(read('tests/golden/out-basic-site/internal_all.csv'));
  assert.ok(basic.length > 2);
  assert.ok(basic.every(row => row.length === basic[0].length), 'gleiche Feldzahl je Zeile');
  assert.ok(basic[0].includes('rel="next" 1'), 'maskierte Anführungszeichen');
  assert.ok(column(basic, 'Address').some(a => a.endsWith('/about.html')));
  const changes = parseCsv(read('tests/golden/out-b3-diff/crawl_changes.csv'));
  assert.equal(changes[0][0], 'Change');
  assert.equal(cell([['Address', 'Title 1'], ['http://site.test/', 'Rendered']], 'http://site.test/', 'Title 1'), 'Rendered');
  assert.throws(() => cell(changes, 'http://nope.test/', 'Change'), /Spalte fehlt|Adresse fehlt/);
  assert.equal(normalizeText('﻿a\r\nb'), 'a\nb');
  assert.deepEqual(parseCsv('a,"b\nc"\n1,2'), [['a', 'b\nc'], ['1', '2']]);
});

// spec: OUT-274
test('MCP-Auswertung: Servername und Werkzeuge aus initialize und tools/list', () => {
  const requests = mcpRequests([{ jsonrpc: '2.0', id: 3, method: 'tools/call' }]);
  assert.deepEqual(requests.map(r => r.method), ['initialize', 'notifications/initialized', 'tools/list', 'tools/call']);
  assert.equal(requests[1].id, undefined, 'Benachrichtigung ohne id');
  const summary = mcpSummary([
    { jsonrpc: '2.0', id: 2, result: { tools: [{ name: 'list_crawls' }, { name: 'query_urls' }] } },
    { jsonrpc: '2.0', id: 1, result: { serverInfo: { name: 'openspider' } } },
  ]);
  assert.deepEqual(summary, { server: 'openspider', tools: ['list_crawls', 'query_urls'] });
  assert.throws(() => mcpSummary([{ id: 1, error: {} }, { id: 2, result: { tools: [] } }]), /initialize ohne Ergebnis/);
});

// OCI-Layout-Fixture wie aus `docker buildx build --output type=oci` mit SBOM und Provenance.
function ociFixture(t, { sbomPackages = ['chromium', 'tini', 'ca-certificates', 'bash'], provenance = true } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'openspider-oci-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  mkdirSync(join(dir, 'blobs', 'sha256'), { recursive: true });
  const put = value => {
    const body = JSON.stringify(value);
    const hex = createHash('sha256').update(body).digest('hex');
    writeFileSync(join(dir, 'blobs', 'sha256', hex), body);
    return { digest: `sha256:${hex}`, size: body.length };
  };
  const config = put({ config: { User: '10001:10001', Entrypoint: ['/usr/bin/tini', '--', '/usr/local/bin/openspider'], Cmd: ['--help'], Env: ['OPENSPIDER_HOME=/config'], WorkingDir: '/data', Labels: { 'org.opencontainers.image.licenses': 'Apache-2.0' } }, rootfs: { type: 'layers', diff_ids: ['sha256:' + 'a'.repeat(64), 'sha256:' + 'b'.repeat(64)] } });
  const manifest = put({ schemaVersion: 2, mediaType: 'application/vnd.oci.image.manifest.v1+json', config: { mediaType: 'application/vnd.oci.image.config.v1+json', ...config }, layers: [] });
  const layers = [{ mediaType: 'application/vnd.in-toto+json', annotations: { 'in-toto.io/predicate-type': 'https://spdx.dev/Document' }, ...put({ predicateType: 'https://spdx.dev/Document', predicate: { packages: sbomPackages.map(name => ({ name })) } }) }];
  if (provenance) layers.push({ mediaType: 'application/vnd.in-toto+json', annotations: { 'in-toto.io/predicate-type': 'https://slsa.dev/provenance/v0.2' }, ...put({ predicateType: 'https://slsa.dev/provenance/v0.2', predicate: {} }) });
  const attestation = put({ schemaVersion: 2, mediaType: 'application/vnd.oci.image.manifest.v1+json', config: { mediaType: 'application/vnd.oci.image.config.v1+json', ...put({}) }, layers });
  const index = put({ schemaVersion: 2, mediaType: 'application/vnd.oci.image.index.v1+json', manifests: [
    { mediaType: 'application/vnd.oci.image.manifest.v1+json', ...manifest, platform: { architecture: 'amd64', os: 'linux' } },
    { mediaType: 'application/vnd.oci.image.manifest.v1+json', ...attestation, platform: { architecture: 'unknown', os: 'unknown' }, annotations: { 'vnd.docker.reference.digest': manifest.digest, 'vnd.docker.reference.type': 'attestation-manifest' } },
  ] });
  writeFileSync(join(dir, 'index.json'), JSON.stringify({ schemaVersion: 2, manifests: [{ mediaType: 'application/vnd.oci.image.index.v1+json', ...index }] }));
  writeFileSync(join(dir, 'oci-layout'), '{"imageLayoutVersion":"1.0.0"}');
  const inspect = [{ Config: { User: '10001:10001', Entrypoint: ['/usr/bin/tini', '--', '/usr/local/bin/openspider'], Cmd: ['--help'], Env: ['OPENSPIDER_HOME=/config'], WorkingDir: '/data', Labels: { 'org.opencontainers.image.licenses': 'Apache-2.0' } }, RootFS: { Type: 'layers', Layers: ['sha256:' + 'a'.repeat(64), 'sha256:' + 'b'.repeat(64)] } }];
  return { dir, inspect, manifest };
}

// spec: OUT-274
test('OCI-Export: SBOM und Provenance gefunden, Pflichtpakete geprüft, Gleichheit mit dem geprüften Image', t => {
  const { dir, inspect, manifest } = ociFixture(t);
  const layout = readLayout(dir);
  assert.equal(layout.manifestDigest, manifest.digest);
  const checked = checkAttestations(layout);
  assert.equal(checked.packages, 4);
  assert.equal(checked.provenanceType, 'https://slsa.dev/provenance/v0.2');
  assert.deepEqual(sameImage(layout, inspect), { identical: true, differences: [] });
  const other = structuredClone(inspect);
  other[0].RootFS.Layers[1] = 'sha256:' + 'c'.repeat(64);
  other[0].Config.User = '0';
  assert.deepEqual(sameImage(layout, other), { identical: false, differences: ['Schichten', 'User'] });
});

// spec: OUT-274
test('OCI-Export ohne Provenance oder ohne Chromium im SBOM wird abgelehnt', t => {
  assert.throws(() => checkAttestations(readLayout(ociFixture(t, { provenance: false }).dir)), /Provenance-Attestierung/);
  assert.throws(() => checkAttestations(readLayout(ociFixture(t, { sbomPackages: ['tini', 'ca-certificates'] }).dir)), /SBOM nennt nicht: chromium/);
});
