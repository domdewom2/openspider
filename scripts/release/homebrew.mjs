// B3-BREW: Casks für den eigenen Homebrew-Tap (`<owner>/homebrew-openspider`) aus Version und SHA256SUMS eines
// veröffentlichten Releases. Kein Download der DMG: Dateiname aus assets.mjs (OUT-261, Universal-DMG), Prüfsumme aus
// SHA256SUMS, Cask-Text aus packaging/homebrew/generate.mjs (gleicher Text wie die P01-Veröffentlichung).
//
//   node scripts/release/homebrew.mjs generate --version 1.0.0-beta.3 --sums SHA256SUMS --repository owner/repo --output dir
//   node scripts/release/homebrew.mjs update-tap --generated dir --tap tap-checkout
//
// Kanäle: Vorabversionen (beta/rc) erzeugen nur `Casks/openspider@beta.rb`; stabile Versionen zusätzlich
// `Casks/openspider.rb`. Ein Tap wird nie auf eine ältere Version zurückgesetzt und eine vorhandene Version nie mit
// anderem Inhalt überschrieben (veröffentlichte Assets werden nicht still ersetzt).
import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { downloads } from './assets.mjs';
import { caskSource, caskChannels } from '../../packaging/homebrew/generate.mjs';

export const tapName = 'homebrew-openspider';
const sumsLine = /^([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._+-]*)$/;

// Liest SHA256SUMS im Format von checksums.mjs (`<sha256>  <name>`); alles andere ist ein Fehler.
export function parseSums(text) {
  const sums = new Map();
  for (const line of text.split('\n')) {
    if (!line) continue;
    const match = sumsLine.exec(line);
    if (!match) throw new Error(`Ungültige Zeile in SHA256SUMS: ${JSON.stringify(line.slice(0, 120))}`);
    if (sums.has(match[2])) throw new Error(`Doppelter Eintrag in SHA256SUMS: ${match[2]}`);
    sums.set(match[2], match[1]);
  }
  if (!sums.size) throw new Error('SHA256SUMS ist leer');
  return sums;
}

function parseVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:-(beta|rc)\.(\d+))?$/.exec(version ?? '');
  if (!match) throw new Error(`Ungültige Version: ${version}`);
  return [...match.slice(1, 4).map(Number), { beta: 0, rc: 1 }[match[4]] ?? 2, Number(match[5] ?? 0)];
}

// < 0, 0, > 0 wie bei sort(); beta < rc < stabil bei gleicher Hauptversion.
export function compareVersions(a, b) {
  const [x, y] = [parseVersion(a), parseVersion(b)];
  const index = x.findIndex((value, i) => value !== y[i]);
  return index < 0 ? 0 : x[index] - y[index];
}

export const isStable = version => parseVersion(version)[3] === 2;

// Pfade relativ zum Tap-Wurzelverzeichnis → Cask-Text.
export function casks({ version, sums, repository }) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository ?? '')) throw new Error('Ungültiges GitHub-Repository');
  const { file } = downloads(version).macos;
  const sha256 = sums.get(file);
  if (!sha256) throw new Error(`SHA256SUMS enthält ${file} nicht (Universal-DMG, OUT-261)`);
  // Gleiche URL-Form wie die Update-Manifeste (manifest.mjs).
  const url = `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(file)}`;
  const channels = isStable(version) ? ['stable', 'beta'] : ['beta'];
  return Object.fromEntries(channels.map(channel =>
    [`Casks/${caskChannels[channel].token}.rb`, caskSource({ channel, version, sha256, url })]));
}

export function writeCasks(output, files) {
  mkdirSync(output, { recursive: true });
  if (readdirSync(output).length) throw new Error('Ausgabeordner muss leer sein');
  mkdirSync(join(output, 'Casks'));
  for (const [path, content] of Object.entries(files)) writeFileSync(join(output, path), content);
  return Object.keys(files);
}

const caskVersion = text => /^ {2}version "([^"]+)"$/m.exec(text)?.[1];

// Übernimmt erzeugte Casks in einen Tap-Checkout. Ergebnis je Datei: neu, aktualisiert, unverändert, übersprungen.
// `openspider@beta` folgt der neuesten Version: eine stabile Version hinter einer neueren Beta lässt ihn stehen.
export function updateTap({ generated, tap }) {
  const names = existsSync(join(generated, 'Casks')) ? readdirSync(join(generated, 'Casks')).sort() : [];
  const known = Object.values(caskChannels).map(channel => `${channel.token}.rb`);
  if (!names.length || names.some(name => !known.includes(name))) {
    throw new Error(`Erwartet nur ${known.join(', ')} in ${join(generated, 'Casks')}: ${names.join(', ') || 'keine'}`);
  }
  const plan = names.map(name => {
    const path = `Casks/${name}`;
    const content = readFileSync(join(generated, path), 'utf8');
    const version = caskVersion(content);
    parseVersion(version);
    if (!existsSync(join(tap, path))) return { path, version, action: 'neu', content };
    const existing = readFileSync(join(tap, path), 'utf8');
    const current = caskVersion(existing);
    const order = compareVersions(version, current);
    if (order === 0) {
      if (existing !== content) {
        throw new Error(`${path}: Version ${version} steht schon mit anderem Inhalt im Tap; wird nicht still ersetzt`);
      }
      return { path, version, action: 'unverändert' };
    }
    if (order < 0) {
      if (name === `${caskChannels.beta.token}.rb` && isStable(version)) {
        return { path, version, action: `übersprungen (Tap hat neuere ${current})` };
      }
      throw new Error(`${path}: ${version} ist älter als ${current} im Tap; kein Zurücksetzen`);
    }
    return { path, version, action: `aktualisiert (vorher ${current})`, content };
  });
  // Erst schreiben, wenn alle Dateien geprüft sind.
  mkdirSync(join(tap, 'Casks'), { recursive: true });
  for (const { path, content } of plan) if (content) writeFileSync(join(tap, path), content);
  return plan.map(({ path, version, action }) => ({ path, version, action }));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { values, positionals } = parseArgs({ allowPositionals: true, options: Object.fromEntries(
    ['version', 'sums', 'repository', 'output', 'generated', 'tap'].map(key => [key, { type: 'string' }])) });
  const [command] = positionals;
  if (command === 'generate' && positionals.length === 1) {
    const files = casks({ version: values.version, sums: parseSums(readFileSync(values.sums, 'utf8')), repository: values.repository });
    for (const path of writeCasks(values.output, files)) console.log(`erzeugt: ${path}`);
  } else if (command === 'update-tap' && positionals.length === 1) {
    for (const { path, version, action } of updateTap({ generated: values.generated, tap: values.tap })) {
      console.log(`${path} ${version}: ${action}`);
    }
  } else {
    console.error('Aufruf: homebrew.mjs generate --version V --sums SHA256SUMS --repository owner/repo --output DIR\n' +
      '       homebrew.mjs update-tap --generated DIR --tap DIR');
    process.exit(2);
  }
}
