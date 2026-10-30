import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export function releaseNotes(changelog, version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-beta\.(?:0|[1-9]\d*)|-rc\.[1-9]\d*)?$/.test(version)) throw new Error('Ungültige Release-Version');
  const sections = changelog.split(/^## /m).slice(1);
  const section = sections.find(s => s.split('\n')[0].match(/^\[?([^\]\s]+)\]?/)?.[1] === version);
  const notes = section?.slice(section.indexOf('\n') + 1).trim();
  if (!notes) throw new Error(`CHANGELOG-Eintrag ${version} fehlt oder ist leer`);
  return `${notes}\n`;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [version, output] = process.argv.slice(2);
  writeFileSync(output, releaseNotes(readFileSync('CHANGELOG.md', 'utf8'), version));
}
