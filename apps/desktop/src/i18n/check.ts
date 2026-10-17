// Statische Prüfung der Übersetzungen (UI-121, CI): Vollständigkeit de/en je Feature, gleiche Platzhalter,
// eindeutige Schlüssel, auflösbare Verweise. Weitere Sprachen dürfen unvollständig sein (Rückfall auf Englisch) und
// werden mit Prozentangabe gemeldet.
import { parse, type Message, type Term } from "@fluent/syntax";
import type { FtlFile } from "./core";

export interface CatalogReport {
  problems: string[];
  /** Vollständigkeit in Prozent je Sprache gegenüber Englisch. */
  completeness: Record<string, number>;
  /** Anzahl der übersetzbaren Schlüssel (Englisch). */
  keys: number;
}

interface Entry {
  key: string;
  vars: Set<string>;
  refs: { kind: "message" | "term"; id: string }[];
}

type Node = { type?: string; [k: string]: unknown };

function walk(node: unknown, visit: (n: Node) => void): void {
  if (Array.isArray(node)) {
    for (const x of node) walk(x, visit);
    return;
  }
  if (typeof node !== "object" || node === null) return;
  const n = node as Node;
  if (typeof n.type === "string") visit(n);
  for (const [k, v] of Object.entries(n)) if (k !== "type" && typeof v === "object") walk(v, visit);
}

function collect(prefix: string, pattern: unknown, into: Entry[]): void {
  const vars = new Set<string>();
  const refs: Entry["refs"] = [];
  walk(pattern, (n) => {
    if (n.type === "VariableReference") vars.add(((n.id as Node).name as string) ?? "");
    if (n.type === "MessageReference") refs.push({ kind: "message", id: (n.id as Node).name as string });
    if (n.type === "TermReference") refs.push({ kind: "term", id: (n.id as Node).name as string });
  });
  into.push({ key: prefix, vars, refs });
}

export interface ParsedFile {
  file: FtlFile;
  entries: Entry[];
  terms: Set<string>;
  junk: string[];
}

export function parseFile(file: FtlFile): ParsedFile {
  const res = parse(file.source, { withSpans: false });
  const entries: Entry[] = [];
  const terms = new Set<string>();
  const junk: string[] = [];
  for (const item of res.body) {
    if (item.type === "Junk") {
      junk.push(item.annotations.map((a) => `${a.code}: ${a.message}`).join("; ") || "unlesbarer Eintrag");
    } else if (item.type === "Message") {
      const m = item as Message;
      if (m.value) collect(m.id.name, m.value, entries);
      for (const a of m.attributes) collect(`${m.id.name}.${a.id.name}`, a.value, entries);
    } else if (item.type === "Term") {
      terms.add((item as Term).id.name);
    }
  }
  return { file, entries, terms, junk };
}

export function checkCatalog(files: readonly FtlFile[], required: readonly string[] = ["de", "en"]): CatalogReport {
  const problems: string[] = [];
  const parsed = files.map(parseFile);
  const langs = [...new Set(files.map((f) => f.lang))];

  // Syntax und doppelte Schlüssel
  for (const p of parsed) for (const j of p.junk) problems.push(`${p.file.path}: ungültiger Eintrag (${j})`);
  const seen = new Map<string, string>();
  for (const p of parsed) {
    for (const e of p.entries) {
      const id = `${p.file.lang}|${e.key}`;
      const prev = seen.get(id);
      if (prev && prev !== p.file.path) problems.push(`Schlüssel ${e.key} (${p.file.lang}) doppelt: ${prev} und ${p.file.path}`);
      seen.set(id, p.file.path);
    }
  }

  // Vollständigkeit und Platzhalter je Feature und Pflichtsprache
  const features = [...new Set(files.map((f) => f.feature))];
  for (const feature of features) {
    const here = parsed.filter((p) => p.file.feature === feature);
    const present = required.filter((l) => here.some((p) => p.file.lang === l));
    if (present.length > 0 && present.length < required.length) {
      const missing = required.filter((l) => !present.includes(l));
      problems.push(`${feature}: Sprachdatei fehlt (${missing.join(", ")})`);
      continue;
    }
    const keyMaps = new Map<string, Map<string, Entry>>();
    for (const lang of required) {
      const m = new Map<string, Entry>();
      for (const p of here.filter((x) => x.file.lang === lang)) for (const e of p.entries) m.set(e.key, e);
      keyMaps.set(lang, m);
    }
    const all = new Set<string>();
    for (const m of keyMaps.values()) for (const k of m.keys()) all.add(k);
    for (const key of [...all].sort()) {
      const variants = required.map((l) => [l, keyMaps.get(l)?.get(key)] as const);
      for (const [lang, entry] of variants) if (!entry) problems.push(`${feature}: Schlüssel ${key} fehlt in ${lang}`);
      const have = variants.filter((v): v is readonly [string, Entry] => v[1] !== undefined);
      const [ref, ...rest] = have;
      if (ref)
        for (const [lang, entry] of rest) {
          const a = [...ref[1].vars].sort().join(",");
          const b = [...entry.vars].sort().join(",");
          if (a !== b) problems.push(`${feature}: Platzhalter von ${key} weichen ab (${ref[0]}: {${a}} / ${lang}: {${b}})`);
        }
    }
  }

  // Verweise auflösbar (je Sprache über alle Features)
  for (const lang of langs) {
    const ids = new Set(parsed.filter((p) => p.file.lang === lang).flatMap((p) => p.entries.map((e) => e.key.split(".")[0] as string)));
    const terms = new Set(parsed.filter((p) => p.file.lang === lang).flatMap((p) => [...p.terms]));
    for (const p of parsed.filter((x) => x.file.lang === lang))
      for (const e of p.entries)
        for (const r of e.refs) {
          if (r.kind === "message" ? !ids.has(r.id) : !terms.has(r.id)) problems.push(`${p.file.path}: ${e.key} verweist auf unbekanntes ${r.kind === "term" ? "-" : ""}${r.id}`);
        }
  }

  // Vollständigkeit weiterer Sprachen
  const enKeys = new Set(parsed.filter((p) => p.file.lang === "en").flatMap((p) => p.entries.map((e) => e.key)));
  const completeness: Record<string, number> = {};
  for (const lang of langs) {
    const have = new Set(parsed.filter((p) => p.file.lang === lang).flatMap((p) => p.entries.map((e) => e.key)));
    let n = 0;
    for (const k of enKeys) if (have.has(k)) n++;
    completeness[lang] = enKeys.size === 0 ? 100 : Math.round((n / enKeys.size) * 100);
  }
  return { problems, completeness, keys: enKeys.size };
}
