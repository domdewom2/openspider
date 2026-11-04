import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { downloads } from './assets.mjs';

// CHANGELOG notes only: these go into latest.json/latest-beta.json and the in-app update dialog (OUT-251).
export function releaseNotes(changelog, version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-beta\.(?:0|[1-9]\d*)|-rc\.[1-9]\d*)?$/.test(version)) throw new Error('Ungültige Release-Version');
  const sections = changelog.split(/^## /m).slice(1);
  const section = sections.find(s => s.split('\n')[0].match(/^\[?([^\]\s]+)\]?/)?.[1] === version);
  const notes = section?.slice(section.indexOf('\n') + 1).trim();
  if (!notes) throw new Error(`CHANGELOG-Eintrag ${version} fehlt oder ist leer`);
  return `${notes}\n`;
}

// GitHub release body: download box first, then the CHANGELOG notes (OUT-261, Nutzerentscheidung 04.11.2026).
// Never used for the update manifests.
export function releaseBody({ version, repository, notes }) {
  releaseNotes(`## ${version}\nvalidation`, version);
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository ?? '')) throw new Error('Ungültiges GitHub-Repository');
  if (!notes?.trim()) throw new Error('Release-Notes fehlen');
  const { macos, windows } = downloads(version);
  const link = file => `[${file}](https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(file)})`;
  return [
    '## Download',
    '',
    '| System | Datei / File |',
    '|---|---|',
    `| **macOS 13+** (Apple Silicon und Intel) | ${link(macos.file)} |`,
    `| **Windows 10/11** (x64, auch Windows on ARM) | ${link(windows.file)} |`,
    '',
    'Mehr brauchst du nicht. Die übrigen Dateien unten sind für automatische Updates, die Kommandozeile (CLI), Firmen-Installationen (MSI) und Prüfsummen (`SHA256SUMS`).',
    '*That\'s all you need. The other files below are for automatic updates, the command line (CLI), managed installs (MSI) and checksums.*',
    '',
    'Anleitung: [Benutzerhandbuch](https://openspider.de/handbuch/) · [User guide](https://openspider.de/en/manual/) · Feedback: [openspider.de/beta](https://openspider.de/beta/)',
    '',
    '## Neu in dieser Version',
    '',
    notes.trim(),
    '',
  ].join('\n');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  // notes.mjs <version> <update-notes> [--body <release-body> --repository <owner/name>]
  const { values, positionals } = parseArgs({ allowPositionals: true, options: {
    body: { type: 'string' }, repository: { type: 'string' } } });
  const [version, output] = positionals;
  if (!output || positionals.length !== 2) throw new Error('Aufruf: notes.mjs <Version> <Update-Notes> [--body <Datei> --repository <owner/name>]');
  if (values.repository && !values.body) throw new Error('--repository nur zusammen mit --body');
  const notes = releaseNotes(readFileSync('CHANGELOG.md', 'utf8'), version);
  writeFileSync(output, notes);
  if (values.body) writeFileSync(values.body, releaseBody({ version, repository: values.repository, notes }));
}
