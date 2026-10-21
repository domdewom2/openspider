// Minimaler XLSX-Leser (ZIP + Blatt-XML) für den Vergleich der Excel-Exporte; keine neue Abhängigkeit.
import { readFileSync } from "node:fs";
import { inflateRawSync } from "node:zlib";

function entries(buf: Buffer): Map<string, Buffer> {
  const out = new Map<string, Buffer>();
  let eocd = buf.length - 22;
  while (eocd >= 0 && buf.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  if (eocd < 0) throw new Error("Kein ZIP-Archiv");
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  for (let i = 0; i < count; i++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new Error("Zentralverzeichnis defekt");
    const method = buf.readUInt16LE(p + 10);
    const size = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extraLen = buf.readUInt16LE(p + 30);
    const commentLen = buf.readUInt16LE(p + 32);
    const local = buf.readUInt32LE(p + 42);
    const name = buf.toString("utf8", p + 46, p + 46 + nameLen);
    const dataStart = local + 30 + buf.readUInt16LE(local + 26) + buf.readUInt16LE(local + 28);
    const data = buf.subarray(dataStart, dataStart + size);
    out.set(name, method === 0 ? data : inflateRawSync(data));
    p += 46 + nameLen + extraLen + commentLen;
  }
  return out;
}

const unescape = (s: string) => s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(Number(d))).replace(/&amp;/g, "&");

function column(ref: string): number {
  let n = 0;
  for (const ch of ref.replace(/\d+/g, "")) n = n * 26 + ch.charCodeAt(0) - 64;
  return n - 1;
}

export interface Workbook {
  /** Blattnamen in Reihenfolge. */
  sheets: string[];
  /** Zellwerte als Text je Blatt (Zeilen × Spalten). */
  data: Record<string, string[][]>;
}

export function readXlsx(path: string): Workbook {
  const files = entries(readFileSync(path));
  const text = (name: string) => files.get(name)?.toString("utf8") ?? "";
  const shared = [...text("xl/sharedStrings.xml").matchAll(/<si>([\s\S]*?)<\/si>/g)].map((m) => unescape([...m[1]!.matchAll(/<t[^>]*>([\s\S]*?)<\/t>/g)].map((t) => t[1]).join("")));
  const names = [...text("xl/workbook.xml").matchAll(/<sheet [^>]*name="([^"]*)"/g)].map((m) => unescape(m[1]!));
  const data: Record<string, string[][]> = {};
  names.forEach((name, i) => {
    const xml = text(`xl/worksheets/sheet${i + 1}.xml`);
    const rows: string[][] = [];
    for (const r of xml.matchAll(/<row[^>]*>([\s\S]*?)<\/row>/g)) {
      const cells: string[] = [];
      for (const c of r[1]!.matchAll(/<c ([^>]*?)(?:\/>|>([\s\S]*?)<\/c>)/g)) {
        const ref = /r="([A-Z]+\d+)"/.exec(c[1]!)?.[1] ?? "A1";
        const type = /t="([^"]+)"/.exec(c[1]!)?.[1];
        const inner = c[2] ?? "";
        const raw = /<v>([\s\S]*?)<\/v>/.exec(inner)?.[1];
        let value = "";
        if (type === "s") value = shared[Number(raw)] ?? "";
        else if (type === "inlineStr") value = unescape([...inner.matchAll(/<t[^>]*>([\s\S]*?)<\/t>/g)].map((t) => t[1]).join(""));
        else if (raw !== undefined) value = unescape(raw);
        cells[column(ref)] = value;
      }
      rows.push(Array.from(cells, (v) => v ?? ""));
    }
    data[name] = rows;
  });
  return { sheets: names, data };
}
