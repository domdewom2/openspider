// Collect only distributable files; verification logs remain in the Actions artifacts.
// OUT-261 (Nutzerentscheidung 04.11.2026): macOS wird nur als Universal-DMG veröffentlicht;
// zusätzlich entstehen versionslose, bytegleiche Kopien für feste latest/download-Links.
import { readdirSync, lstatSync, mkdirSync, copyFileSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const versionPattern = /^\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?$/;
const namePattern = /^[A-Za-z0-9][A-Za-z0-9._+-]*$/;

function checkVersion(version) {
  if (!versionPattern.test(version ?? '')) throw new Error('Ungültige Version');
}

// Die beiden Dateien für Endnutzer: versionierter Name dieses Releases und versionslose Kopie.
export function downloads(version) {
  checkVersion(version);
  return {
    macos: { file: `OpenSpider_${version}_universal.dmg`, alias: 'OpenSpider-macOS.dmg' },
    windows: { file: `OpenSpider_${version}_x64-setup.exe`, alias: 'OpenSpider-Windows-Setup.exe' },
  };
}

// Gebaut und geprüft, aber nicht veröffentlicht: DMGs und CLI-Archive je macOS-Architektur sowie
// das Universal-Updater-Archiv (das Update-Manifest nutzt nur aarch64/x86_64). Unbekannte macOS-
// Dateinamen sind ein Fehler statt still veröffentlicht oder verworfen zu werden.
export function isPublished(name, version) {
  checkVersion(version);
  if (name.endsWith('.dmg')) {
    if (!new Set(['aarch64', 'x64', 'universal'].map(a => `OpenSpider_${version}_${a}.dmg`)).has(name)) {
      throw new Error(`Unerwartetes DMG: ${name}`);
    }
    return name === downloads(version).macos.file;
  }
  if (/^openspider_.*_darwin-[^/]*\.tar\.gz$/.test(name)) {
    if (!new Set(['aarch64', 'x86_64', 'universal'].map(a => `openspider_${version}_darwin-${a}.tar.gz`)).has(name)) {
      throw new Error(`Unerwartetes macOS-CLI-Archiv: ${name}`);
    }
    return name === `openspider_${version}_darwin-universal.tar.gz`;
  }
  const updater = /^OpenSpider_([A-Za-z0-9_]+)\.app\.tar\.gz(?:\.sig)?$/.exec(name);
  if (updater) {
    if (!['aarch64', 'x86_64', 'universal'].includes(updater[1])) throw new Error(`Unerwartetes Updater-Archiv: ${name}`);
    return updater[1] !== 'universal';
  }
  return true;
}

export function collectAssets(source, destination, version) {
  checkVersion(version);
  mkdirSync(destination, { recursive: true });
  const skipped = [];
  const aliases = [];
  function collect(directory) {
    for (const name of readdirSync(directory)) {
      const path = join(directory, name);
      const stat = lstatSync(path);
      if (stat.isDirectory()) { collect(path); continue; }
      if (!stat.isFile()) throw new Error(`Kein reguläres Asset: ${path}`);
      if (!/\.(?:dmg|exe|msi|tar\.gz|zip)(?:\.sig)?$/.test(name)) continue;
      if (!namePattern.test(name)) throw new Error(`Ungültiger Assetname: ${name}`);
      if (Object.values(downloads(version)).some(d => d.alias === name)) throw new Error(`Asset-Kollision: ${name}`);
      if (!isPublished(name, version)) { skipped.push(name); continue; }
      // Preserve names and bytes referenced by updater signatures.
      const output = name;
      if (existsSync(join(destination, output))) throw new Error(`Asset-Kollision: ${output}`);
      copyFileSync(path, join(destination, output));
    }
  }
  collect(source);
  for (const { file, alias } of Object.values(downloads(version))) {
    // Without Apple/Windows build output (e.g. a dry run without secrets) there is nothing to alias;
    // the draft job refuses to create a release without both aliases.
    if (!existsSync(join(destination, file))) continue;
    copyFileSync(join(destination, file), join(destination, alias));
    aliases.push(alias);
  }
  return { skipped: skipped.sort(), aliases };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [source, destination, version] = process.argv.slice(2);
  const { skipped, aliases } = collectAssets(source, destination, version);
  for (const name of skipped) console.log(`nicht veröffentlicht (OUT-261): ${name}`);
  for (const name of aliases) console.log(`versionslose Kopie: ${name}`);
}
