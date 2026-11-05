#!/usr/bin/env node
// Smoke-Test des Docker-Images (OUT-274). Läuft im Workflow .github/workflows/docker.yml und lokal auf Linux:
//
//   cargo build --locked -p os-testsite
//   node packaging/docker/smoke.mjs --image openspider:smoke --version 1.0.0-beta.3 --testsite target/debug/os-testsite
//
// Prüft: Metadaten (nicht root, Labels, Einstiegspunkt), `--version`, Standard-CMD `--help`, Crawl mit CSV-Export gegen
// os-testsite (als Image-Benutzer 10001), Vergleich mit dem vorherigen Crawl samt Report „Crawl Changes“ (mit
// `--user` des Aufrufers und gemountetem /config), JavaScript-Rendering mit dem Chromium des Images, Fehler statt Hänger
// bei fehlendem Geheimnis, MCP über STDIO (`initialize` → `tools/list`, Standard nur lesend, `--allow read,crawl`,
// `list_crawls` liest das Volume) und die Lizenzhinweise im Image.
// Alle Prüfungen laufen durch; am Ende eine Tabelle (stdout, $GITHUB_STEP_SUMMARY, --report) und Exit 1 bei Fehlern.
// Der Crawl braucht `docker run --network host` (os-testsite lauscht auf 127.0.0.1), also Linux bzw. Docker mit Host-Netz.
// Nur Node-Standardbibliothek.

import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../..', import.meta.url)));
export const IMAGE_USER = '10001:10001';
const MINUTE = 60_000;

// ---- reine Hilfsfunktionen (Vertragstest packaging/docker/docker.test.mjs) ------------------------------------------

export function parseArgs(argv) {
  const options = { image: '', version: '', testsite: '', report: '' };
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i].replace(/^--/, '');
    if (!(key in options) || argv[i + 1] === undefined) throw new Error(`Unbekannte oder unvollständige Option: ${argv[i]}`);
    options[key] = argv[++i];
  }
  for (const key of ['image', 'version', 'testsite']) if (!options[key]) throw new Error(`--${key} fehlt`);
  if (!/^\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?$/.test(options.version)) throw new Error(`Ungültige Version: ${options.version}`);
  return options;
}

/** Port aus der Startzeile von os-testsite (`<szenario> -> http://127.0.0.1:<port>`). */
export function testsitePort(text) {
  const match = /-> http:\/\/127\.0\.0\.1:(\d+)/.exec(text);
  return match ? Number(match[1]) : null;
}

/** `openspider --version` nach OUT-181: `openspider <semver> (<git-hash>, linux-x86_64)`. */
export function versionMatches(output, version) {
  const line = output.trim();
  return line.startsWith(`openspider ${version} (`) && /, linux-x86_64\)$/.test(line) && !line.includes('\n');
}

/** Text ohne BOM und mit LF (Normalisierung wie die CLI-Tests). */
export function normalizeText(text) {
  return text.replace(/^﻿/, '').replace(/\r\n/g, '\n');
}

/** CSV nach RFC 4180 (Anführungszeichen, Kommas und Zeilenumbrüche in Feldern); Ergebnis: Zeilen als Arrays. */
export function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let quoted = false;
  const input = normalizeText(text);
  for (let i = 0; i < input.length; i += 1) {
    const c = input[i];
    if (quoted) {
      if (c === '"' && input[i + 1] === '"') { field += '"'; i += 1; }
      else if (c === '"') quoted = false;
      else field += c;
    } else if (c === '"') quoted = true;
    else if (c === ',') { row.push(field); field = ''; }
    else if (c === '\n') { row.push(field); rows.push(row); row = []; field = ''; }
    else field += c;
  }
  if (quoted) throw new Error('CSV endet in einem Feld mit Anführungszeichen');
  if (field !== '' || row.length) { row.push(field); rows.push(row); }
  return rows;
}

/** Werte einer Spalte (ohne Kopfzeile). */
export function column(rows, name) {
  const index = rows[0]?.indexOf(name) ?? -1;
  if (index < 0) throw new Error(`Spalte fehlt: ${name}`);
  return rows.slice(1).map(row => row[index]);
}

/** Wert einer Spalte in der Zeile mit der Adresse `address`. */
export function cell(rows, address, name) {
  const addresses = column(rows, 'Address');
  const values = column(rows, name);
  const i = addresses.indexOf(address);
  if (i < 0) throw new Error(`Adresse fehlt im Export: ${address}`);
  return values[i];
}

/** MCP-Antworten auswerten: Servername und Werkzeugnamen aus `initialize` (id 1) und `tools/list` (id 2). */
export function mcpSummary(responses) {
  const byId = new Map(responses.map(r => [r.id, r]));
  const init = byId.get(1);
  const list = byId.get(2);
  if (!init?.result) throw new Error(`initialize ohne Ergebnis: ${JSON.stringify(init)}`);
  if (!Array.isArray(list?.result?.tools)) throw new Error(`tools/list ohne Werkzeugliste: ${JSON.stringify(list)}`);
  return { server: init.result.serverInfo?.name, tools: list.result.tools.map(t => t.name) };
}

export function mcpRequests(extra = []) {
  return [
    { jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-11-25', capabilities: {}, clientInfo: { name: 'docker-smoke', version: '0' } } },
    { jsonrpc: '2.0', method: 'notifications/initialized' },
    { jsonrpc: '2.0', id: 2, method: 'tools/list' },
    ...extra,
  ];
}

/** Markdown-Tabelle der Ergebnisse. */
export function summaryTable(results, title) {
  const escape = s => String(s).replace(/\|/g, '\\|').replace(/\n/g, ' ');
  const lines = [`### ${title}`, '', '| Prüfung | Ergebnis | Details |', '|---|---|---|'];
  for (const r of results) lines.push(`| ${escape(r.name)} | ${r.ok ? 'ok' : 'FEHLER'} | ${escape(r.detail ?? '')} |`);
  return `${lines.join('\n')}\n`;
}

// ---- Prozesse --------------------------------------------------------------------------------------------------

function run(command, args, { timeout = 5 * MINUTE, input } = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', timeout, input, maxBuffer: 64 * 1024 * 1024 });
  if (result.error) throw new Error(`${command} ${args.join(' ')}: ${result.error.message}`);
  return { code: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}

function tail(text, lines = 15) {
  return text.trim().split('\n').slice(-lines).join('\n');
}

function expectExit(result, code, what) {
  if (result.code !== code) throw new Error(`${what}: Exit ${result.code} statt ${code}\n${tail(result.stdout)}\n${tail(result.stderr)}`);
}

async function startTestsite(binary, scenario) {
  const child = spawn(binary, [scenario], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const exited = new Promise(r => child.on('exit', r));
  let output = '';
  const port = await new Promise((resolvePort, reject) => {
    const timer = setTimeout(() => reject(new Error(`os-testsite ${scenario} meldet keinen Port: ${output}`)), 30_000);
    const onData = chunk => {
      output += chunk;
      const found = testsitePort(output);
      if (found) { clearTimeout(timer); resolvePort(found); }
    };
    child.stdout.on('data', onData);
    child.stderr.on('data', chunk => { output += chunk; });
    child.on('exit', code => { clearTimeout(timer); reject(new Error(`os-testsite ${scenario} beendet (${code}): ${output}`)); });
  });
  return {
    port,
    async stop() {
      if (child.exitCode !== null || child.signalCode !== null) return;
      child.kill('SIGINT');
      const timer = setTimeout(() => child.kill('SIGKILL'), 5_000);
      await exited;
      clearTimeout(timer);
    },
  };
}

/** Startet `docker run -i … mcp …`, sendet die Anfragen und wartet auf `expected` Antworten (mit `id`). */
async function mcpSession(dockerArgs, requests, expected, timeout = 2 * MINUTE) {
  const child = spawn('docker', dockerArgs, { stdio: ['pipe', 'pipe', 'pipe'] });
  const exited = new Promise(r => child.on('exit', r));
  let stderr = '';
  child.stderr.on('data', chunk => { stderr += chunk; });
  const responses = [];
  const nonJson = [];
  const done = new Promise((resolveDone, reject) => {
    const timer = setTimeout(() => { child.kill('SIGKILL'); reject(new Error(`MCP: keine ${expected} Antworten in ${timeout / 1000} s; stderr: ${tail(stderr)}`)); }, timeout);
    createInterface({ input: child.stdout }).on('line', line => {
      try {
        const message = JSON.parse(line);
        if (message.id !== undefined) responses.push(message);
      } catch {
        nonJson.push(line);
      }
      if (responses.length >= expected) { clearTimeout(timer); resolveDone(); }
    });
    child.on('exit', code => {
      if (responses.length < expected) { clearTimeout(timer); reject(new Error(`MCP-Prozess endete (${code}) nach ${responses.length} Antworten; stderr: ${tail(stderr)}`)); }
    });
  });
  for (const request of requests) child.stdin.write(`${JSON.stringify(request)}\n`);
  await done;
  child.stdin.end();
  const timer = setTimeout(() => child.kill('SIGKILL'), 30_000);
  const code = await exited;
  clearTimeout(timer);
  if (nonJson.length) throw new Error(`stdout enthält Zeilen außerhalb des Protokolls: ${nonJson.slice(0, 3).join(' | ')}`);
  return { responses, code, stderr };
}

// ---- Prüfungen -------------------------------------------------------------------------------------------------

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const { image, version } = options;
  const testsite = resolve(options.testsite);
  if (!existsSync(testsite)) throw new Error(`os-testsite fehlt: ${testsite} (cargo build --locked -p os-testsite)`);
  const work = mkdtempSync(join(process.env.RUNNER_TEMP || tmpdir(), 'openspider-docker-smoke-'));
  const uid = typeof process.getuid === 'function' ? `${process.getuid()}:${process.getgid()}` : null;
  const results = [];
  const check = async (name, fn) => {
    const started = Date.now();
    try {
      const detail = await fn();
      results.push({ name, ok: true, detail: `${detail ?? ''} (${Math.round((Date.now() - started) / 1000)} s)`.trim() });
      console.log(`ok      ${name}`);
    } catch (error) {
      results.push({ name, ok: false, detail: error.message });
      console.log(`FEHLER  ${name}\n${error.message}`);
    }
  };
  // Ordner für den Image-Benutzer 10001 beschreibbar machen (Bind-Mount ohne --user).
  const openDir = name => { const dir = join(work, name); mkdirSync(dir, { recursive: true }); chmodSync(dir, 0o777); return dir; };
  const ownDir = name => { const dir = join(work, name); mkdirSync(dir, { recursive: true }); return dir; };
  const asCaller = () => {
    if (!uid) throw new Error('docker run --user braucht eine POSIX-Benutzerkennung (Linux)');
    return ['--user', uid];
  };

  await check('Metadaten: Benutzer, Labels, Einstiegspunkt, Umgebung', () => {
    const inspect = run('docker', ['image', 'inspect', image]);
    expectExit(inspect, 0, 'docker image inspect');
    const config = JSON.parse(inspect.stdout)[0].Config;
    const problems = [];
    if (config.User !== IMAGE_USER) problems.push(`User ${config.User}`);
    const labels = config.Labels ?? {};
    const want = {
      'org.opencontainers.image.licenses': 'Apache-2.0',
      'org.opencontainers.image.version': version,
      'org.opencontainers.image.source': 'https://github.com/wojcikdominik/openspider',
      'org.opencontainers.image.title': 'OpenSpider',
    };
    for (const [key, value] of Object.entries(want)) if (labels[key] !== value) problems.push(`${key}=${labels[key]}`);
    if (!labels['org.opencontainers.image.description']) problems.push('Beschreibung fehlt');
    if (JSON.stringify(config.Entrypoint) !== JSON.stringify(['/usr/bin/tini', '--', '/usr/local/bin/openspider'])) problems.push(`Entrypoint ${JSON.stringify(config.Entrypoint)}`);
    if (JSON.stringify(config.Cmd) !== JSON.stringify(['--help'])) problems.push(`Cmd ${JSON.stringify(config.Cmd)}`);
    if (config.WorkingDir !== '/data') problems.push(`WorkingDir ${config.WorkingDir}`);
    if (!Object.hasOwn(config.Volumes ?? {}, '/config')) problems.push('Volume /config fehlt');
    for (const env of ['OPENSPIDER_HOME=/config', 'OPENSPIDER_CHROMIUM=/usr/local/bin/openspider-chromium', 'OPENSPIDER_SECRET_STORE=memory']) {
      if (!config.Env.includes(env)) problems.push(`Env ${env} fehlt`);
    }
    if (problems.length) throw new Error(problems.join('; '));
    return `User ${config.User}, Revision ${labels['org.opencontainers.image.revision'] || '-'}`;
  });

  await check('Prozess läuft nicht als root', () => {
    const id = run('docker', ['run', '--rm', '--entrypoint', 'id', image]);
    expectExit(id, 0, 'id');
    if (!/^uid=10001\(openspider\) gid=10001\(openspider\)/.test(id.stdout.trim())) throw new Error(id.stdout.trim());
    return id.stdout.trim();
  });

  await check('openspider --version', () => {
    const result = run('docker', ['run', '--rm', image, '--version']);
    expectExit(result, 0, '--version');
    if (!versionMatches(result.stdout, version)) throw new Error(`Ausgabe: ${result.stdout.trim()}`);
    return result.stdout.trim();
  });

  await check('Standard-CMD zeigt die Hilfe', () => {
    const result = run('docker', ['run', '--rm', image]);
    expectExit(result, 0, 'docker run ohne Argumente');
    for (const flag of ['--crawl', '--export-tabs', '--output-folder']) if (!result.stdout.includes(flag)) throw new Error(`Hilfe ohne ${flag}`);
    return 'Hilfe auf stdout';
  });

  await check('Chromium im Image', () => {
    const result = run('docker', ['run', '--rm', '--entrypoint', '/usr/local/bin/openspider-chromium', image, '--version']);
    expectExit(result, 0, 'Chromium --version');
    if (!/Chromium \d+\./.test(result.stdout)) throw new Error(result.stdout.trim());
    return result.stdout.trim();
  });

  await check('Lizenzhinweise im Image', () => {
    // Release-Archiv genau; Debian-Pakete über ihre copyright-Dateien (Chromium ggf. im Ordner von chromium-common).
    const script = 'set -e; for f in /usr/share/doc/openspider/LICENSE /usr/share/doc/openspider/NOTICE '
      + '/usr/share/doc/openspider/THIRD_PARTY_NOTICES.txt /usr/share/doc/tini/copyright /usr/share/doc/ca-certificates/copyright; '
      + 'do test -s "$f"; echo "$f"; done; ls /usr/share/doc/chromium*/copyright';
    const result = run('docker', ['run', '--rm', '--entrypoint', '/bin/sh', image, '-c', script]);
    expectExit(result, 0, 'Lizenzdateien');
    return `${result.stdout.trim().split('\n').length} Dateien vorhanden`;
  });

  // Crawl mit Export als Image-Benutzer (Bind-Mount, Ordner für alle beschreibbar).
  await check('Crawl mit CSV-Export (out-basic-site, Benutzer 10001)', async () => {
    const site = await startTestsite(testsite, 'out-basic-site');
    try {
      const out = openDir('basic');
      const result = run('docker', ['run', '--rm', '--network', 'host', '-v', `${out}:/data`, image,
        '--headless', '--crawl', 'http://site.test/', '--resolve', `*.test=127.0.0.1:${site.port}`,
        '--export-tabs', 'Internal:All', '--save-report', 'Crawl Overview', '--save-crawl', '--output-folder', '/data'], { timeout: 10 * MINUTE });
      expectExit(result, 0, 'Crawl');
      const rows = parseCsv(readFileSync(join(out, 'internal_all.csv'), 'utf8'));
      const golden = parseCsv(readFileSync(join(ROOT, 'tests/golden/out-basic-site/internal_all.csv'), 'utf8'));
      if (rows.length !== golden.length) throw new Error(`internal_all.csv: ${rows.length - 1} Zeilen, Golden ${golden.length - 1}`);
      if (!column(rows, 'Address').includes('http://site.test/about.html')) throw new Error('about.html fehlt im Export');
      for (const file of ['crawl_overview.csv', 'crawl.ospider']) if (!existsSync(join(out, file))) throw new Error(`${file} fehlt`);
      return `${rows.length - 1} URLs wie Golden, crawl_overview.csv, crawl.ospider`;
    } finally {
      await site.stop();
    }
  });

  // Vergleich mit dem vorherigen Crawl: zwei Läufe mit derselben /config, Benutzer des Aufrufers.
  const config = ownDir('config');
  await check('Vergleich mit dem vorherigen Crawl, Report „Crawl Changes“ (--user, /config gemountet)', async () => {
    const compare = async (scenario, folder) => {
      const site = await startTestsite(testsite, scenario);
      try {
        const out = ownDir(folder);
        const result = run('docker', ['run', '--rm', ...asCaller(), '--network', 'host', '-v', `${out}:/data`, '-v', `${config}:/config`, image,
          '--crawl', 'http://www.site.test/', '--resolve', `*.test=127.0.0.1:${site.port}`, '--set', 'robots.retries=0',
          '--project-name', 'docker-smoke', '--compare-with-previous', '--save-report', 'Crawl Changes', '--output-folder', '/data'], { timeout: 10 * MINUTE });
        expectExit(result, 0, `Crawl ${scenario}`);
        return readFileSync(join(out, 'crawl_changes.csv'), 'utf8');
      } finally {
        await site.stop();
      }
    };
    const first = parseCsv(await compare('out-b3-diff/v1', 'compare-1'));
    if (first.length !== 1) throw new Error(`erster Lauf: ${first.length - 1} Zeilen statt nur der Kopfzeile`);
    const second = normalizeText(await compare('out-b3-diff/v2', 'compare-2'));
    const golden = normalizeText(readFileSync(join(ROOT, 'tests/golden/out-b3-diff/crawl_changes.csv'), 'utf8'));
    if (second !== golden) throw new Error(`crawl_changes.csv weicht vom Golden tests/golden/out-b3-diff/crawl_changes.csv ab:\n${tail(second, 5)}`);
    return `${parseCsv(second).length - 1} Änderungen wie Golden`;
  });

  await check('JavaScript-Rendering mit Chromium (crawl-m2-render)', async () => {
    const site = await startTestsite(testsite, 'crawl-m2-render');
    try {
      const out = openDir('render');
      const result = run('docker', ['run', '--rm', '--network', 'host', '-v', `${out}:/data`, image,
        '--crawl', 'http://site.test/', '--resolve', `*.test=127.0.0.1:${site.port}`, '--set', 'robots.retries=0',
        '--set', 'rendering.mode=javascript', '--set', 'limits.max_depth=1', '--max-urls', '20',
        '--export-tabs', 'Internal:All', '--output-folder', '/data'], { timeout: 10 * MINUTE });
      expectExit(result, 0, 'Crawl mit Rendering');
      const rows = parseCsv(readFileSync(join(out, 'internal_all.csv'), 'utf8'));
      const title = cell(rows, 'http://site.test/', 'Title 1');
      if (title !== 'Rendered') throw new Error(`Title 1 der Startseite: '${title}' statt 'Rendered'`);
      if (!column(rows, 'Address').includes('http://site.test/js-only')) throw new Error('nur per JavaScript verlinkte Seite fehlt');
      return 'Titel nach JavaScript, JS-Link gefunden';
    } finally {
      await site.stop();
    }
  });

  await check('Fehlendes Geheimnis: klarer Fehler statt Hänger', () => {
    const out = ownDir('secret');
    writeFileSync(join(out, 'secret.osconfig'), 'schema_version = 1\n\n[[auth.basic]]\nprefix = "http://127.0.0.1:1/"\nusername = "tester"\nsecret_ref = "docker-smoke-missing"\n');
    const started = Date.now();
    const result = run('docker', ['run', '--rm', ...asCaller(), '-v', `${out}:/data`, '-v', `${config}:/config`, image,
      '--crawl', 'http://127.0.0.1:1/', '--config', '/data/secret.osconfig'], { timeout: MINUTE });
    if (result.code === 0) throw new Error('Exit 0 trotz fehlendem Geheimnis');
    if (!(result.stderr.includes('nicht verfügbar') && result.stderr.includes('docker-smoke-missing'))) throw new Error(tail(result.stderr));
    return `Exit ${result.code} nach ${Math.round((Date.now() - started) / 1000)} s`;
  });

  const mcpBase = ['run', '-i', '--rm', ...(uid ? ['--user', uid] : []), '-v', `${config}:/config`, image, 'mcp'];
  await check('MCP über STDIO: initialize, tools/list, Standard nur lesend, list_crawls aus /config', async () => {
    const call = { jsonrpc: '2.0', id: 3, method: 'tools/call', params: { name: 'list_crawls', arguments: {} } };
    const session = await mcpSession(mcpBase, mcpRequests([call]), 3);
    const { server, tools } = mcpSummary(session.responses);
    if (server !== 'openspider') throw new Error(`serverInfo.name: ${server}`);
    if (!tools.includes('list_crawls')) throw new Error(`list_crawls fehlt: ${tools.join(', ')}`);
    if (tools.includes('start_crawl')) throw new Error('start_crawl ohne Freigabe sichtbar (AGENT-003: Standard nur lesen)');
    const listed = session.responses.find(r => r.id === 3);
    if (!listed?.result || listed.result.isError) throw new Error(`list_crawls: ${JSON.stringify(listed)}`);
    if (!JSON.stringify(listed.result).includes('docker-smoke')) throw new Error('list_crawls kennt die Crawls aus /config nicht');
    if (session.code !== 0) throw new Error(`Exit ${session.code} nach geschlossenem stdin`);
    return `${tools.length} Werkzeuge, nur lesend`;
  });

  await check('MCP mit --allow read,crawl zeigt start_crawl', async () => {
    const session = await mcpSession([...mcpBase, '--allow', 'read,crawl'], mcpRequests(), 2);
    const { tools } = mcpSummary(session.responses);
    if (!tools.includes('start_crawl')) throw new Error(`start_crawl fehlt: ${tools.join(', ')}`);
    return `${tools.length} Werkzeuge`;
  });

  await check('Image-Größe', () => {
    const result = run('docker', ['image', 'inspect', '--format', '{{.Size}}', image]);
    expectExit(result, 0, 'Größe');
    return `${Math.round(Number(result.stdout.trim()) / 1e6)} MB unkomprimiert`;
  });

  const table = summaryTable(results, `Docker-Smoke ${image} (${version})`);
  console.log(`\n${table}`);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${table}\n`);
  if (options.report) {
    mkdirSync(options.report, { recursive: true });
    writeFileSync(join(options.report, 'smoke.md'), table);
    writeFileSync(join(options.report, 'smoke.json'), `${JSON.stringify(results, null, 2)}\n`);
  }
  try {
    rmSync(work, { recursive: true, force: true, maxRetries: 2 });
  } catch (error) {
    console.log(`Hinweis: Arbeitsordner ${work} nicht vollständig entfernt (${error.code ?? error.message}); Dateien des Image-Benutzers 10001.`);
  }
  const failed = results.filter(r => !r.ok).length;
  if (failed) {
    console.error(`${failed} von ${results.length} Prüfungen fehlgeschlagen`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
