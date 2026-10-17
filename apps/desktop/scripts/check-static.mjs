#!/usr/bin/env node
// `npm run check:static`: statische Prüfungen aus M2-ui-concept 7.1 (docs/plan/M2.md 2a C4), Build schlägt bei Verstoß fehl.
//   1. Wrapper ↔ commands.json aktuell (gen:ipc --check)
//   2. npm-Lizenzprüfung gegen die Positivliste von deny.toml
//   3. vitest src/ui/static: i18n-Vollständigkeit, Kürzelregister, Voraussetzungsregister, Quelltext-Regeln, Glide-Pin
//   4. Kontrast-Unit-Test der Tokens (UI-120)
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const vitestBin = join(dirname(createRequire(import.meta.url).resolve("vitest/package.json")), "vitest.mjs");
const steps = [
  ["gen:ipc --check", [join(root, "scripts", "gen-ipc.mjs"), "--check"]],
  ["npm-Lizenzen", [join(root, "scripts", "check-licenses.mjs")]],
  ["vitest static + Tokens", [vitestBin, "run", "src/ui/static", "src/ui/tokens"]],
];

let failed = false;
for (const [name, args] of steps) {
  process.stdout.write(`\n== check:static / ${name}\n`);
  const r = spawnSync(process.execPath, args, { cwd: root, stdio: "inherit" });
  if (r.status !== 0) {
    failed = true;
    console.error(`check:static: Schritt "${name}" fehlgeschlagen`);
  }
}
process.exit(failed ? 1 : 0);
