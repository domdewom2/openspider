#!/usr/bin/env node
// Erzeugt die Command-Wrapper je Domäne aus src/ipc/commands.json (docs/plan/M2.md 3.8): src/ipc/<domäne>.ts.
//   node scripts/gen-ipc.mjs           schreibt die Dateien
//   node scripts/gen-ipc.mjs --check   prüft, ob die eingecheckten Dateien dem Katalog entsprechen (Vertragstest)
import { existsSync, mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const ipcDir = join(root, "src", "ipc");
const catalog = JSON.parse(readFileSync(join(ipcDir, "commands.json"), "utf8"));

const RESERVED_FILES = new Set(["client", "errors", "events", "commands", "index"]);
const RESERVED_WORDS = new Set(["export", "import", "default", "delete", "new", "class", "function", "var", "let", "const"]);
const BUILTIN = new Set(["void", "string", "number", "boolean", "null", "undefined", "unknown"]);
const SPECIAL_IMPORT = { JsonValue: "./gen/serde_json/JsonValue" };

export function fileNameFor(domain) {
  return RESERVED_FILES.has(domain) ? `${domain}.commands` : domain;
}
export function constNameFor(domain) {
  return RESERVED_WORDS.has(domain) ? `${domain}Api` : domain;
}

function typeNames(expr) {
  return [...expr.matchAll(/[A-Za-z_][A-Za-z0-9_]*/g)].map((m) => m[0]).filter((n) => !BUILTIN.has(n));
}

export function render(domain, cmds) {
  const imports = new Set();
  const needsOp = cmds.some((c) => c.longRunning);
  if (needsOp) imports.add("OpHandle");
  for (const c of cmds) {
    typeNames(c.args).forEach((n) => imports.add(n));
    // Lang laufende Commands liefern sofort ein OpHandle; der Ergebnistyp steht nur im Kommentar.
    if (!c.longRunning) typeNames(c.ret).forEach((n) => imports.add(n));
  }
  const lines = [
    "// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).",
    ...[...imports].sort().map((n) => `import type { ${n} } from "${SPECIAL_IMPORT[n] ?? `./gen/${n}`}";`),
    'import { client } from "./client";',
    "",
    `export const ${constNameFor(domain)} = {`,
  ];
  for (const c of cmds) {
    const short = c.name.slice(domain.length + 1);
    const result = c.longRunning ? "OpHandle" : c.ret;
    const doc = c.longRunning ? `/** \`${c.name}\` – lang laufend: liefert sofort \`OpHandle\`, Ergebnis (${c.ret}) über \`Tick.ops[].result\`. */` : `/** \`${c.name}\` */`;
    lines.push(`  ${doc}`);
    if (c.args === "void") {
      lines.push(`  ${short}: (signal?: AbortSignal) => client.invoke<${result}>("${c.name}", undefined, signal),`);
    } else {
      lines.push(`  ${short}: (args: ${c.args}, signal?: AbortSignal) => client.invoke<${result}>("${c.name}", args, signal),`);
    }
  }
  lines.push("} as const;", "");
  return lines.join("\n");
}

export function generate() {
  const byDomain = new Map();
  for (const c of catalog) {
    const domain = c.name.split(".")[0];
    if (!byDomain.has(domain)) byDomain.set(domain, []);
    byDomain.get(domain).push(c);
  }
  const files = new Map();
  for (const [domain, cmds] of byDomain) files.set(`${fileNameFor(domain)}.ts`, render(domain, cmds));
  return files;
}

const GENERATED_MARK = "// Generiert von scripts/gen-ipc.mjs";

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const check = process.argv.includes("--check");
  const files = generate();
  const problems = [];
  for (const [name, text] of files) {
    const path = join(ipcDir, name);
    // Zeilenenden egal (Windows-Checkout mit autocrlf)
    const current = existsSync(path) ? readFileSync(path, "utf8").replace(/\r\n/g, "\n") : null;
    if (current !== text) {
      if (check) problems.push(`${name}: veraltet oder fehlend`);
      else writeFileSync(path, text);
    }
  }
  // Verwaiste generierte Dateien (Domäne entfallen)
  for (const f of readdirSync(ipcDir)) {
    if (!f.endsWith(".ts") || files.has(f)) continue;
    const text = readFileSync(join(ipcDir, f), "utf8");
    if (text.startsWith(GENERATED_MARK)) {
      if (check) problems.push(`${f}: verwaist`);
      else unlinkSync(join(ipcDir, f));
    }
  }
  if (check && problems.length > 0) {
    console.error("gen:ipc --check fehlgeschlagen (npm run gen:ipc ausführen):\n  " + problems.join("\n  "));
    process.exit(1);
  }
  console.log(check ? `gen:ipc: ${files.size} Wrapper-Dateien aktuell` : `gen:ipc: ${files.size} Wrapper-Dateien geschrieben`);
}
