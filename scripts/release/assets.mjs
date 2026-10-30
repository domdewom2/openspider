// Collect only distributable files; verification logs remain in the Actions artifacts.
import { readdirSync, lstatSync, mkdirSync, copyFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const [source, destination, version] = process.argv.slice(2);
if (!/^\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?$/.test(version)) throw new Error('Ungültige Version');
mkdirSync(destination, { recursive: true });
function collect(directory) {
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    const stat = lstatSync(path);
    if (stat.isDirectory()) { collect(path); continue; }
    if (!stat.isFile()) throw new Error(`Kein reguläres Asset: ${path}`);
    if (!/\.(?:dmg|exe|msi|tar\.gz|zip)(?:\.sig)?$/.test(name)) continue;
    if (!/^[A-Za-z0-9][A-Za-z0-9._+-]*$/.test(name)) throw new Error(`Ungültiger Assetname: ${name}`);
    // Preserve names and bytes referenced by updater signatures.
    const output = name;
    if (existsSync(join(destination, output))) throw new Error(`Asset-Kollision: ${output}`);
    copyFileSync(path, join(destination, output));
  }
}
collect(source);
