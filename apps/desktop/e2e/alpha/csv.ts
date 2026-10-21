// Kleiner CSV-Leser (RFC 4180) und Normalisierung für den Vergleich UI = CLI (OUT-001).
import { readFileSync } from "node:fs";

export type Table = string[][];

export function parseCsv(text: string): Table {
  const src = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  const rows: Table = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < src.length; i++) {
    const c = src[i]!;
    if (quoted) {
      if (c === '"') {
        if (src[i + 1] === '"') {
          field += '"';
          i++;
        } else quoted = false;
      } else field += c;
    } else if (c === '"') quoted = true;
    else if (c === ",") {
      row.push(field);
      field = "";
    } else if (c === "\r" || c === "\n") {
      if (c === "\r" && src[i + 1] === "\n") i++;
      row.push(field);
      field = "";
      rows.push(row);
      row = [];
    } else field += c;
  }
  if (field !== "" || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

export function readCsv(path: string): Table {
  return parseCsv(readFileSync(path, "utf8"));
}

export interface Normalize {
  /** Basis-URL der Fixture (`http://127.0.0.1:<port>`), wird zu `{BASE}`. */
  base: string;
  /** Spalten, die von Lauf zu Lauf schwanken (Zeiten, Zeitstempel). */
  volatileColumns?: string[];
  /** Zeilen eines Berichts (Spalte `Item`), die von Lauf zu Lauf schwanken. */
  volatileItems?: string[];
}

/** Ersetzt Basis-URL und Loopback-Namen, maskiert flüchtige Spalten/Zeilen und sortiert die Datenzeilen. */
export function normalizeTable(table: Table, n: Normalize): { header: string[]; rows: string[][] } {
  const [header = [], ...body] = table;
  const volatile = new Set((n.volatileColumns ?? []).map((c) => header.indexOf(c)).filter((i) => i >= 0));
  const itemColumn = header.indexOf("Item");
  const swap = (v: string) => v.split(n.base).join("{BASE}");
  const rows = body
    .filter((r) => !(itemColumn >= 0 && (n.volatileItems ?? []).includes(r[itemColumn] ?? "")))
    .map((r) => r.map((v, i) => (volatile.has(i) ? "·" : swap(v))));
  rows.sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));
  return { header, rows };
}
