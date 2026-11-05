import { appendFileSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { downloads } from './assets.mjs';

const versionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-beta\.(?:0|[1-9]\d*)|-rc\.[1-9]\d*)?$/;
function checkVersion(version) {
  if (!versionPattern.test(version ?? '')) throw new Error('Ungültige Release-Version');
}

// OUT-251 (Notes vor dem Bau): one checked-in notes document per version is the only source for the
// update dialog (latest.json/latest-beta.json) and the release text (OUT-261). It is fixed before the build.
export function notesPath(version) {
  checkVersion(version);
  return join('docs', 'release', 'notes', `${version}.md`);
}

// Validates a notes document: not empty, no level-1/2 headings (they would split the CHANGELOG
// section and the release text), no draft markers.
export function versionNotes(text, version) {
  checkVersion(version);
  const notes = String(text ?? '').replaceAll('\r\n', '\n').trim();
  if (!notes) throw new Error(`Release-Notes ${version} sind leer: ${notesPath(version)} vor dem Bauen ausfüllen`);
  if (/^#{1,2}\s/m.test(notes)) throw new Error(`Release-Notes ${version}: keine Überschriften der Ebene 1/2 (# oder ##), höchstens ###`);
  if (/\b(?:TODO|STUB)\b|Entwurfshinweis/.test(notes)) throw new Error(`Release-Notes ${version} enthalten Entwurfsmarkierungen (TODO/STUB/Entwurfshinweis)`);
  return `${notes}\n`;
}

// CHANGELOG section of a version (without the heading line); null if absent.
export function changelogSection(changelog, version) {
  checkVersion(version);
  const sections = changelog.replaceAll('\r\n', '\n').split(/^## /m).slice(1);
  const section = sections.find(s => s.split('\n')[0].match(/^\[?([^\]\s]+)\]?/)?.[1] === version);
  return section === undefined ? null : section.slice(section.indexOf('\n') + 1).trim();
}

// CHANGELOG notes of a version; fails if the section is missing or empty.
export function releaseNotes(changelog, version) {
  const notes = changelogSection(changelog, version);
  if (!notes) throw new Error(`CHANGELOG-Eintrag ${version} fehlt oder ist leer`);
  return `${notes}\n`;
}

// Gate before the build (prepare job): notes document present and valid; the CHANGELOG section,
// if `requireChangelog` (release build) must exist and match it word for word.
export function checkNotes({ root = process.cwd(), version, requireChangelog = true }) {
  const path = join(root, notesPath(version));
  if (!existsSync(path)) {
    throw new Error(`Release-Notes für ${version} fehlen: ${notesPath(version)} anlegen und committen, bevor gebaut wird (docs/release/release-rhythmus.md)`);
  }
  const notes = versionNotes(readFileSync(path, 'utf8'), version);
  const section = changelogSection(readFileSync(join(root, 'CHANGELOG.md'), 'utf8'), version);
  if (section === null && !requireChangelog) return notes;
  if (!section) throw new Error(`CHANGELOG-Eintrag ${version} fehlt: node scripts/release/notes.mjs --changelog ${version} --date <JJJJ-MM-TT>`);
  if (`${section}\n` !== notes) {
    throw new Error(`CHANGELOG-Eintrag ${version} weicht von ${notesPath(version)} ab; die Notes-Datei ist die einzige Quelle`);
  }
  return notes;
}

// Inserts the CHANGELOG section for a version from its notes document (directly below [Unreleased]).
// An existing identical section is kept; a differing one is never overwritten.
export function withChangelogSection(changelog, version, notes, date) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date ?? '') || Number.isNaN(Date.parse(date))) throw new Error('Datum erwartet: JJJJ-MM-TT');
  const body = versionNotes(notes, version);
  const existing = changelogSection(changelog, version);
  if (existing !== null) {
    if (`${existing}\n` === body) return changelog;
    throw new Error(`CHANGELOG enthält bereits einen abweichenden Eintrag ${version}; nicht überschrieben`);
  }
  const text = changelog.replaceAll('\r\n', '\n');
  const section = `## [${version}] - ${date}\n\n${body}\n`;
  const unreleased = /^## \[Unreleased\][^\n]*\n(?:(?!^## )[^\n]*\n?)*/m.exec(text);
  if (unreleased) {
    const end = unreleased.index + unreleased[0].length;
    const before = text.slice(0, end).replace(/\n*$/, '\n\n');
    return before + section + text.slice(end).replace(/^\n+/, '');
  }
  const first = text.search(/^## /m);
  return first < 0 ? `${text.replace(/\n*$/, '\n\n')}${section}` : text.slice(0, first) + section + text.slice(first);
}

// Feedback link in the download box: prereleases → beta page, stable → GitHub Discussions.
export function feedbackLink(version, repository) {
  checkVersion(version);
  return version.includes('-')
    ? '[openspider.de/beta](https://openspider.de/beta/)'
    : `[GitHub Discussions](https://github.com/${repository}/discussions)`;
}

// GitHub release body: download box, then the notes. Nothing else (OUT-261; draft hints go to the
// Actions summary only). Never used for the update manifests.
export function releaseBody({ version, repository, notes }) {
  checkVersion(version);
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
    `Anleitung: [Benutzerhandbuch](https://openspider.de/handbuch/) · [User guide](https://openspider.de/en/manual/) · Feedback: ${feedbackLink(version, repository)}`,
    '',
    '## Neu in dieser Version',
    '',
    notes.trim(),
    '',
  ].join('\n');
}

function main() {
  // notes.mjs <version> <update-notes> [--body <release-body> --repository <owner/name>]
  // notes.mjs --check <version>                      (prepare job, before the build)
  // notes.mjs --changelog <version> --date YYYY-MM-DD (writes the CHANGELOG section from the notes document)
  const { values, positionals } = parseArgs({ allowPositionals: true, options: {
    body: { type: 'string' }, repository: { type: 'string' }, check: { type: 'boolean' },
    changelog: { type: 'boolean' }, date: { type: 'string' } } });
  const usage = 'Aufruf: notes.mjs <Version> <Update-Notes> [--body <Datei> --repository <owner/name>] | --check <Version> | --changelog <Version> --date <JJJJ-MM-TT>';
  if (values.check || values.changelog) {
    if (positionals.length !== 1 || (values.check && values.changelog) || values.body || values.repository) throw new Error(usage);
    const [version] = positionals;
    if (values.check) {
      checkNotes({ version });
      console.log(`Release-Notes ${version}: ${notesPath(version)} vorhanden und gleich dem CHANGELOG-Eintrag`);
    } else {
      const notes = readFileSync(notesPath(version), 'utf8');
      writeFileSync('CHANGELOG.md', withChangelogSection(readFileSync('CHANGELOG.md', 'utf8'), version, notes, values.date));
    }
  } else {
    const [version, output] = positionals;
    if (!output || positionals.length !== 2 || values.date) throw new Error(usage);
    if (values.repository && !values.body) throw new Error('--repository nur zusammen mit --body');
    const notes = checkNotes({ version });
    writeFileSync(output, notes);
    if (values.body) writeFileSync(values.body, releaseBody({ version, repository: values.repository, notes }));
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    // Clear message instead of a stack trace; annotated in GitHub Actions and in the run summary.
    console.error(error.message);
    if (process.env.GITHUB_ACTIONS === 'true') console.log(`::error title=Release-Notes::${error.message}`);
    if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\n**Release-Notes:** ${error.message}\n`);
    process.exitCode = 1;
  }
}
