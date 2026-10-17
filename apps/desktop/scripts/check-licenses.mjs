#!/usr/bin/env node
// npm-Lizenzprüfung gegen dieselbe Positivliste wie `cargo deny` (deny.toml, [licenses] allow): jedes Paket der
// package-lock.json muss unter einer erlaubten Lizenz stehen (SPDX-Ausdruck: bei OR genügt eine, bei AND müssen alle erlaubt sein).
//   node scripts/check-licenses.mjs [--list]
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

export function readAllowList(denyToml) {
  const m = /^\s*allow\s*=\s*\[([\s\S]*?)\]/m.exec(denyToml.slice(denyToml.indexOf("[licenses]")));
  if (!m) throw new Error("deny.toml: [licenses] allow nicht gefunden");
  return [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
}

/** Ausnahmen mit Begründung (leer lassen, wenn möglich). */
export const EXCEPTIONS = {
  // Beide nur transitiv über jsdom (devDependency, nicht Teil der ausgelieferten App). Die Lizenzen sind freizügig;
  // Aufnahme in die Positivliste von deny.toml ist als Bedarf an den Orchestrator gemeldet (M2-F04).
  "lru-cache": "BlueOak-1.0.0 (freizügig), jsdom-Abhängigkeit, nur Tests",
  "mdn-data": "CC0-1.0 (Datensammlung ohne Rechtevorbehalt), jsdom-Abhängigkeit, nur Tests",
};

function tokenize(expr) {
  return expr.replace(/[()]/g, " ( $& ) ").replace(/\(\s*\(/g, "( (").split(/\s+/).filter(Boolean);
}

/** Wertet einen SPDX-Ausdruck gegen die erlaubten Lizenzen aus. */
export function isAllowed(expr, allowed) {
  const set = new Set(allowed);
  const tokens = tokenize(expr.replace(/\(/g, " ( ").replace(/\)/g, " ) "));
  let i = 0;
  const parseOr = () => {
    let v = parseAnd();
    while (tokens[i] === "OR") {
      i++;
      const r = parseAnd();
      v = v || r;
    }
    return v;
  };
  const parseAnd = () => {
    let v = parseAtom();
    while (tokens[i] === "AND") {
      i++;
      const r = parseAtom();
      v = v && r;
    }
    return v;
  };
  const parseAtom = () => {
    if (tokens[i] === "(") {
      i++;
      const v = parseOr();
      if (tokens[i] === ")") i++;
      return v;
    }
    const id = tokens[i++] ?? "";
    if (id === "WITH") return false;
    let name = id.replace(/\+$/, "");
    if (tokens[i] === "WITH") {
      i++;
      name = `${name} WITH ${tokens[i++]}`;
    }
    return set.has(name) || set.has(id);
  };
  return parseOr();
}

export function checkLockfile(lock, allowed, exceptions = EXCEPTIONS) {
  const problems = [];
  const seen = new Map();
  for (const [path, pkg] of Object.entries(lock.packages ?? {})) {
    if (path === "" || pkg.link) continue;
    const name = path.replace(/^.*node_modules\//, "");
    const license = pkg.license ?? (Array.isArray(pkg.licenses) ? pkg.licenses.map((l) => l.type).join(" OR ") : undefined);
    const key = `${name}@${pkg.version}`;
    if (exceptions[name]) continue;
    if (!license) {
      problems.push(`${key}: keine Lizenzangabe in der package-lock.json`);
      continue;
    }
    seen.set(license, (seen.get(license) ?? 0) + 1);
    if (!isAllowed(String(license), allowed)) problems.push(`${key}: Lizenz "${license}" steht nicht auf der Positivliste`);
  }
  return { problems, seen };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const allowed = readAllowList(readFileSync(join(repo, "deny.toml"), "utf8"));
  const lock = JSON.parse(readFileSync(join(repo, "package-lock.json"), "utf8"));
  const { problems, seen } = checkLockfile(lock, allowed);
  if (process.argv.includes("--list")) console.log([...seen].sort().map(([l, n]) => `${n}\t${l}`).join("\n"));
  if (problems.length > 0) {
    console.error("npm-Lizenzprüfung fehlgeschlagen:\n  " + problems.join("\n  "));
    process.exit(1);
  }
  console.log(`npm-Lizenzprüfung: ${Object.keys(lock.packages ?? {}).length - 1} Pakete, alle unter der Positivliste (${[...seen.keys()].sort().join(", ")})`);
}
