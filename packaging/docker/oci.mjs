#!/usr/bin/env node
// OCI-Export des Images prüfen (OUT-274): SBOM und Provenance aus den BuildKit-Attestierungen lesen und ablegen, und
// belegen, dass der Export dieselben Schichten und dieselbe Konfiguration hat wie das per Smoke-Test geprüfte Image.
//
//   docker image inspect openspider:smoke > inspect.json
//   node packaging/docker/oci.mjs --archive openspider-image.oci.tar --inspect inspect.json --out attestations
//
// Schreibt <out>/sbom.spdx.json, <out>/provenance.json und <out>/oci.md; Exit 1, wenn SBOM oder Provenance fehlen oder
// das SBOM die Pakete aus REQUIRED_PACKAGES nicht nennt. Mit $GITHUB_OUTPUT zusätzlich `identical=true|false`
// (gleiche Schicht-IDs und Konfiguration wie das geprüfte Image); nur dann darf der Workflow veröffentlichen.
// Nur Node-Standardbibliothek; das Archiv entpackt das System-`tar`.

import { spawnSync } from 'node:child_process';
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const REQUIRED_PACKAGES = ['chromium', 'tini', 'ca-certificates'];
const SPDX = 'https://spdx.dev/Document';
const ATTESTATION = 'attestation-manifest';
const INDEX_TYPES = new Set(['application/vnd.oci.image.index.v1+json', 'application/vnd.docker.distribution.manifest.list.v2+json']);

function blobPath(dir, digest) {
  const match = /^sha256:([0-9a-f]{64})$/.exec(digest ?? '');
  if (!match) throw new Error(`Ungültiger Digest: ${digest}`);
  return join(dir, 'blobs', 'sha256', match[1]);
}

function readJson(dir, digest) {
  return JSON.parse(readFileSync(blobPath(dir, digest), 'utf8'));
}

/** Liest ein entpacktes OCI-Layout: Image-Manifest, Konfiguration, Schicht-IDs und Attestierungen (in-toto). */
export function readLayout(dir) {
  const top = JSON.parse(readFileSync(join(dir, 'index.json'), 'utf8'));
  if (top.manifests?.length !== 1) throw new Error(`index.json: genau ein Eintrag erwartet, gefunden ${top.manifests?.length ?? 0}`);
  let entries = top.manifests;
  let indexDigest = null;
  if (INDEX_TYPES.has(entries[0].mediaType)) {
    indexDigest = entries[0].digest;
    entries = readJson(dir, indexDigest).manifests ?? [];
  }
  const isAttestation = m => m.annotations?.['vnd.docker.reference.type'] === ATTESTATION;
  const images = entries.filter(m => !isAttestation(m));
  if (images.length !== 1) throw new Error(`genau ein Image-Manifest erwartet, gefunden ${images.length}`);
  const manifestDigest = images[0].digest;
  const manifest = readJson(dir, manifestDigest);
  const config = readJson(dir, manifest.config.digest);
  const attestations = [];
  for (const entry of entries.filter(m => isAttestation(m) && m.annotations['vnd.docker.reference.digest'] === manifestDigest)) {
    for (const layer of readJson(dir, entry.digest).layers ?? []) {
      const statement = readJson(dir, layer.digest);
      attestations.push({ predicateType: layer.annotations?.['in-toto.io/predicate-type'] ?? statement.predicateType, statement });
    }
  }
  return { indexDigest, manifestDigest, configDigest: manifest.config.digest, config, diffIds: config.rootfs?.diff_ids ?? [], attestations };
}

/** SBOM (SPDX) und Provenance (SLSA) finden und das SBOM auf Pflichtpakete prüfen. */
export function checkAttestations(layout, required = REQUIRED_PACKAGES) {
  const sbom = layout.attestations.find(a => a.predicateType === SPDX);
  const provenance = layout.attestations.find(a => /^https:\/\/slsa\.dev\/provenance\//.test(a.predicateType));
  if (!sbom) throw new Error('SBOM-Attestierung (SPDX) fehlt');
  if (!provenance) throw new Error('Provenance-Attestierung (SLSA) fehlt');
  const names = new Set((sbom.statement.predicate?.packages ?? []).map(p => p.name));
  const missing = required.filter(name => !names.has(name));
  if (missing.length) throw new Error(`SBOM nennt nicht: ${missing.join(', ')}`);
  return { sbom: sbom.statement, provenance: provenance.statement, packages: names.size, provenanceType: provenance.predicateType };
}

/** Gleiches Image wie das geprüfte? Schicht-IDs (unkomprimiert) und Benutzer, Einstiegspunkt, Umgebung, Labels. */
export function sameImage(layout, inspect) {
  const image = Array.isArray(inspect) ? inspect[0] : inspect;
  const differences = [];
  const same = (a, b) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
  if (!same(image.RootFS?.Layers, layout.diffIds)) differences.push('Schichten');
  const fields = [['User', 'User'], ['Entrypoint', 'Entrypoint'], ['Cmd', 'Cmd'], ['Env', 'Env'], ['WorkingDir', 'WorkingDir'], ['Labels', 'Labels']];
  for (const [docker, oci] of fields) if (!same(image.Config?.[docker], layout.config.config?.[oci])) differences.push(docker);
  return { identical: differences.length === 0, differences };
}

function parseArgs(argv) {
  const options = { archive: '', inspect: '', out: '' };
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i].replace(/^--/, '');
    if (!(key in options) || argv[i + 1] === undefined) throw new Error(`Unbekannte oder unvollständige Option: ${argv[i]}`);
    options[key] = argv[++i];
  }
  for (const key of Object.keys(options)) if (!options[key]) throw new Error(`--${key} fehlt`);
  return options;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const dir = mkdtempSync(join(process.env.RUNNER_TEMP || tmpdir(), 'openspider-oci-'));
  try {
    const untar = spawnSync('tar', ['-xf', resolve(options.archive), '-C', dir], { encoding: 'utf8' });
    if (untar.status !== 0) throw new Error(`tar: ${untar.stderr || untar.error?.message}`);
    const layout = readLayout(dir);
    const checked = checkAttestations(layout);
    const comparison = sameImage(layout, JSON.parse(readFileSync(options.inspect, 'utf8')));
    mkdirSync(options.out, { recursive: true });
    writeFileSync(join(options.out, 'sbom.spdx.json'), `${JSON.stringify(checked.sbom, null, 2)}\n`);
    writeFileSync(join(options.out, 'provenance.json'), `${JSON.stringify(checked.provenance, null, 2)}\n`);
    const report = [
      '### OCI-Export und Attestierungen', '',
      `- Image-Manifest: \`${layout.manifestDigest}\`${layout.indexDigest ? `, Index: \`${layout.indexDigest}\`` : ''}`,
      `- SBOM (SPDX): ${checked.packages} Pakete, darunter ${REQUIRED_PACKAGES.join(', ')}`,
      `- Provenance: ${checked.provenanceType}`,
      `- Gleich dem geprüften Image: ${comparison.identical ? 'ja' : `nein (${comparison.differences.join(', ')})`}`,
      '',
    ].join('\n');
    writeFileSync(join(options.out, 'oci.md'), report);
    console.log(report);
    if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${report}\n`);
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `identical=${comparison.identical}\nmanifest=${layout.manifestDigest}\n`);
    if (!comparison.identical) console.log(`::warning::OCI-Export weicht vom geprüften Image ab (${comparison.differences.join(', ')}); Veröffentlichung gesperrt.`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
