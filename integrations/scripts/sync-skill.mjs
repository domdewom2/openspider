// Ship real files rather than symlinks so marketplace caches work on Windows too.
import { cpSync, readdirSync, readFileSync, mkdirSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? files(join(dir, entry.name)) : [join(dir, entry.name)]);
}
export function syncSkill(check = false, base = root) {
  const source = join(base, 'skills/openspider');
  const target = join(base, 'claude-plugin/skills/openspider');
  const shipped = files(source).filter((file) => relative(source, file).split(sep)[0] !== 'evals');
  if (check) {
    const actual = files(target).map((file) => relative(target, file)).sort();
    const expected = shipped.map((file) => relative(source, file)).sort();
    if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('Bundled skill file list differs; run npm run sync');
  }
  for (const file of shipped) {
    const destination = join(target, relative(source, file));
    if (check) {
      if (!readFileSync(destination).equals(readFileSync(file))) throw new Error(`Stale bundled skill: ${destination}`);
    } else {
      mkdirSync(dirname(destination), { recursive: true });
      cpSync(file, destination);
    }
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  syncSkill(process.argv.includes('--check'));
}
