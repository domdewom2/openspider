#!/usr/bin/env node
// Build/CI tool only; never bundled or executed by the application.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, realpathSync, writeFileSync, rmSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { mkdtempSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { readAllowList } from '../../../apps/desktop/scripts/check-licenses.mjs';

export const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
export const destination = join(root, 'apps/desktop/src-tauri/resources/third-party/THIRD_PARTY_NOTICES.txt');
const run = (exe, args) => execFileSync(exe, args, { cwd: root, encoding: 'utf8', shell: process.platform === 'win32' && exe === 'npm.cmd', maxBuffer: 128 * 1024 * 1024 });

// Parse the entire expression, including parentheses and WITH exceptions. Fail closed.
export function allowedExpression(expression, allowed) {
  const tokens = String(expression ?? '').match(/\(|\)|[^\s()]+/g) ?? [];
  let i = 0;
  function atom() {
    if (tokens[i] === '(') {
      i++; const result = or();
      if (tokens[i++] !== ')') throw Error('Unclosed SPDX expression');
      return result;
    }
    let id = tokens[i++];
    if (!id || ['AND', 'OR', 'WITH', ')'].includes(id)) throw Error('Invalid SPDX expression');
    if (tokens[i] === 'WITH') {
      i++; const exception = tokens[i++];
      if (!exception || ['AND', 'OR', 'WITH', '(', ')'].includes(exception)) throw Error('Invalid SPDX exception');
      id += ` WITH ${exception}`;
    }
    return allowed.includes(id);
  }
  function and() { let result = atom(); while (tokens[i] === 'AND') { i++; const right = atom(); result = result && right; } return result; }
  function or() { let result = and(); while (tokens[i] === 'OR') { i++; const right = and(); result = result || right; } return result; }
  try { const result = or(); return i === tokens.length && result; } catch { return false; }
}

export function validate(entry, allowed) {
  if (!entry.name || !entry.version || !allowedExpression(entry.license, allowed)) throw Error(`${entry.name}@${entry.version}: forbidden/missing license: ${entry.license}`);
  if (!entry.text?.trim()) throw Error(`${entry.name}@${entry.version}: missing license text`);
  return entry;
}

export function rustEntries(data, allowed, supplemental = {}, canonical = {}) {
  return data.crates.filter(({ package: pkg }) => pkg.source != null).map(({ package: pkg, license }) => {
    const key = `${pkg.name}@${pkg.version}`;
    const texts = data.licenses.filter((item) => item.used_by.some((user) => user.crate.id === pkg.id));
    const local = pkg.manifest_path ? licenseFiles(dirname(pkg.manifest_path)) : '';
    const reviewed = canonical[key];
    if (!texts.length || texts.some((item) => !item.text?.trim()) ||
        (!local && !supplemental[key] && !(reviewed && reviewed.license === pkg.license && pkg.authors?.length))) {
      throw Error(`${key}: missing sourced license text (requires a version-specific review)`);
    }
    for (const text of texts) if (!allowedExpression(text.id, allowed)) throw Error(`${pkg.name}: forbidden selected license ${text.id}`);
    const attribution = `Authors (Cargo.toml): ${(pkg.authors ?? []).join(', ')}\nRepository: ${pkg.repository ?? ''}`;
    const canonicalNote = !local && !supplemental[key] && reviewed ? `Canonical license text — ${reviewed.reason}\n` : '';
    const textParts = [local, supplemental[key], ...texts.map((item) => `${item.id}\n${item.text.trim()}`)].filter(Boolean);
    return validate({ ecosystem: 'Rust', name: pkg.name, version: pkg.version, license, attribution: canonicalNote + attribution,
      textParts, text: textParts.join('\n\n') }, allowed);
  });
}

export function licenseFiles(directory) {
  const texts = [];
  function visit(dir, depth) {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en'))) {
      const path = join(dir, entry.name);
      if (entry.isFile() && /^(licen[cs]e|copying|copyright|notice)([._-]|$)/i.test(entry.name)) {
        const text = readFileSync(path, 'utf8').trim();
        if (!text) throw Error(`${path}: empty license text`);
        texts.push(`${relative(directory, path).split('\\').join('/')}\n${text}`);
      } else if (entry.isDirectory() && depth === 0 && /^(licenses?|legal)$/i.test(entry.name)) visit(path, depth + 1);
    }
  }
  visit(directory, 0);
  return texts.join('\n\n');
}

export function npmEntries(paths, allowed, supplemental = {}) {
  const lock = JSON.parse(readFileSync(join(root, 'package-lock.json'), 'utf8'));
  const seen = new Set();
  return paths.filter(Boolean).map((path) => realpathSync(path)).filter((path) => {
    if (path === root || path === join(root, 'apps/desktop') || seen.has(path)) return false;
    seen.add(path); return true;
  }).map((path) => {
    const pkg = JSON.parse(readFileSync(join(path, 'package.json'), 'utf8'));
    const locked = lock.packages[relative(root, path).split('\\').join('/')];
    if (!locked || locked.version !== pkg.version || locked.license !== pkg.license) throw Error(`${pkg.name}: installed package differs from lockfile`);
    return validate({ ecosystem: 'npm', name: pkg.name, version: pkg.version, license: pkg.license, text: licenseFiles(path) || supplemental[`${pkg.name}@${pkg.version}`] }, allowed);
  });
}

export function renderNotices(entries) {
  const pool = new Map();
  const inventory = [...entries].sort((a, b) => `${a.ecosystem}/${a.name}/${a.version}`.localeCompare(`${b.ecosystem}/${b.name}/${b.version}`, 'en'))
    .map((entry) => {
      const references = (entry.textParts ?? [entry.text]).map((text) => {
        const normalized = text.trim();
        if (!pool.has(normalized)) pool.set(normalized, `L${String(pool.size + 1).padStart(4, '0')}`);
        return pool.get(normalized);
      });
      return `${entry.ecosystem}: ${entry.name}\nVersion: ${entry.version}\nLicense: ${entry.license}\n${entry.attribution ? entry.attribution + '\n' : ''}License texts: ${[...new Set(references)].join(', ')}\n`;
    }).join('\n');
  return 'OpenSpider — THIRD PARTY NOTICES\nGenerated from locked Rust and frontend production dependencies.\nChromium, Lighthouse and Node.js are not bundled.\nFull license texts follow the inventory; text IDs refer to the complete text below.\n\n'
    + inventory + '\n' + [...pool].map(([text, id]) => `${'='.repeat(80)}\nLicense text ${id}\n\n${text}\n`).join('\n');
}

export function verifiedSourceText(buffer, source) {
  if (createHash('sha256').update(buffer).digest('hex') !== source.sha256) throw Error(`License source checksum mismatch: ${source.url}`);
  return `Source: ${source.url}\n${buffer.toString('utf8')}`;
}

export async function supplementalTexts(file = 'npm-sources.json') {
  const sources = JSON.parse(readFileSync(join(root, 'scripts/release/licenses', file), 'utf8'));
  const cache = new Map();
  async function fetchText(source) {
    if (!cache.has(source.url)) cache.set(source.url, (async () => {
      const response = await fetch(source.url, { signal: AbortSignal.timeout(30_000) });
      if (!response.ok) throw Error(`License source: HTTP ${response.status} ${source.url}`);
      return Buffer.from(await response.arrayBuffer());
    })());
    return verifiedSourceText(await cache.get(source.url), source);
  }
  return Object.fromEntries(await Promise.all(Object.entries(sources).map(async ([key, source]) =>
    [key, (await Promise.all((Array.isArray(source) ? source : [source]).map(fetchText))).join('\n\n')])));
}

export async function generate() {
  // Invalidate an earlier result before attempting a new build.
  writeFileSync(destination, 'STUB(M3-L01): generated notices unavailable; run the license generator.\n');
  const policy = readFileSync(join(root, 'deny.toml'), 'utf8');
  const allowed = readAllowList(policy); // Also the npm positivity list; no dev-only exceptions here.
  const targets = /^targets\s*=\s*(\[[\s\S]*?\])/m.exec(policy)?.[1];
  if (!targets) throw Error('deny.toml: missing graph targets');
  const temporary = mkdtempSync(join(tmpdir(), 'openspider-licenses-'));
  try {
    const config = join(temporary, 'about.toml');
    writeFileSync(config, `accepted = ${JSON.stringify(allowed)}\ntargets = ${targets}\nignore-dev-dependencies = true\n`);
    const rustSources = await supplementalTexts('rust-sources.json');
    const canonical = JSON.parse(readFileSync(join(root, 'scripts/release/licenses/rust-canonical.json'), 'utf8'));
    // cargo-about refuses captured stdout under PowerShell (Windows runner): always write to a file.
    const about = (name, args) => {
      const output = join(temporary, `${name}.json`);
      run('cargo', ['about', 'generate', ...args, '--fail', '--format', 'json', '--config', config, '--output-file', output]);
      return JSON.parse(readFileSync(output, 'utf8'));
    };
    const desktop = rustEntries(about('desktop', ['--locked', '--workspace']), allowed, rustSources, canonical);
    // Linux ships the CLI only: do not pull in GTK/WebKit build dependencies for a Linux desktop.
    const cli = rustEntries(about('cli', ['--locked', '--manifest-path', 'crates/os-cli/Cargo.toml', '--target', 'x86_64-unknown-linux-gnu']), allowed, rustSources, canonical);
    const rust = [...new Map([...desktop, ...cli].map((entry) => [`${entry.name}@${entry.version}`, entry])).values()];
    // npm's own resolver includes transitive dependencies, optional packages and peers,
    // and excludes other workspaces and frontend devDependencies.
    const supplemental = await supplementalTexts();
    const npm = npmEntries(run(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['ls', '--workspace', 'apps/desktop', '--omit=dev', '--all', '--parseable']).trim().split(/\r?\n/), allowed, supplemental);
    const axe = join(root, 'crates/os-render/resources/axe');
    const bundled = validate({ ecosystem: 'vendored', name: 'axe-core', version: '4.11.0', license: 'MPL-2.0',
      text: readFileSync(join(axe, 'LICENSE'), 'utf8') + '\n' + readFileSync(join(axe, 'THIRD_PARTY_NOTICES'), 'utf8') }, allowed);
    writeFileSync(destination, renderNotices([...rust, ...npm, bundled]));
    // NOTICE remains a separate section in the About dialog and is bundled verbatim too.
    writeFileSync(join(dirname(destination), 'NOTICE'), readFileSync(join(root, 'NOTICE')));
    console.log(`License notices: ${rust.length} Rust, ${npm.length} npm, 1 vendored component`);
  } finally { rmSync(temporary, { recursive: true, force: true }); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await generate();
