import { createHash } from 'node:crypto';
import { createReadStream, readdirSync, lstatSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export async function checksums(directory) {
  const lines = [];
  for (const name of readdirSync(directory).sort()) {
    if (name === 'SHA256SUMS') continue;
    if (!/^[A-Za-z0-9][A-Za-z0-9._+-]*$/.test(name) || !lstatSync(join(directory, name)).isFile()) {
      throw new Error(`Ungültiges Release-Asset: ${name}`);
    }
    const hash = createHash('sha256');
    for await (const chunk of createReadStream(join(directory, name))) hash.update(chunk);
    lines.push(`${hash.digest('hex')}  ${name}\n`);
  }
  if (!lines.length) throw new Error('Keine Release-Assets');
  const result = lines.join('');
  writeFileSync(join(directory, 'SHA256SUMS'), result);
  return result;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await checksums(process.argv[2]);
