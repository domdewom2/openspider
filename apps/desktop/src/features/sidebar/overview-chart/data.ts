// UI-102 data exports: CSV after the export engine's CSV rules (OUT-002: RFC 4180, UTF-8 BOM, CRLF, numbers with a
// decimal point, empty cell for a missing value) and „Daten kopieren“ tab separated (UI-060).
import type { TFn } from "../../../i18n";
import { tsv } from "../frame/PanelActions";
import type { ChartModel } from "./model";

export const CSV_COLUMNS = ["Entry", "Category", "Filter", "URLs", "% of Total", "Base"] as const;

export type Cell = string | number | null;

/** One row per category, without bundling; residual categories with an empty `Filter`, no counter → empty `URLs`. */
export function dataRows(model: ChartModel, opts: { uiHeaders?: boolean; t?: TFn } = {}): Cell[][] {
  const head: Cell[] = opts.uiHeaders && opts.t ? CSV_COLUMNS.map((c, i) => opts.t!(`ovc-col-${i}`, undefined, { fallback: c })) : [...CSV_COLUMNS];
  if (model.kind !== "chart" && model.kind !== "noCounts") return [head];
  const categories = model.kind === "chart" ? model.table : model.categories;
  const base = model.header.base;
  return [head, ...categories.map(c => [
    model.header.name,
    c.canonical,
    c.role === "filter" || c.role === "summary" ? c.filter : null,
    c.count,
    c.pct == null ? null : round1(c.pct),
    base,
  ])];
}

const round1 = (n: number) => Math.round(n * 10) / 10;

function csvCell(cell: Cell): string {
  if (cell == null) return "";
  if (typeof cell === "number") return Number.isInteger(cell) ? String(cell) : cell.toFixed(1);
  return /[",\r\n]/.test(cell) ? `"${cell.replace(/"/g, '""')}"` : cell;
}

/** CSV text with BOM and CRLF (defaults of OUT-002). */
export function toCsv(rows: readonly Cell[][]): string {
  return "﻿" + rows.map(r => r.map(csvCell).join(",")).join("\r\n") + "\r\n";
}

/** Tab separated text for the clipboard (same rows). */
export function toTsv(rows: readonly Cell[][]): string {
  return tsv(rows.map(r => r.map(c => c == null ? "" : typeof c === "number" && !Number.isInteger(c) ? c.toFixed(1) : c)));
}

const slug = (s: string) => s.toLowerCase().normalize("NFKD").replace(/[̀-ͯ]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "entry";

/** `overview-<eintrag>[-<segment>]-<datum>.<ext>` (UI-102). */
export function fileName(entryName: string, segment: string | null, ext: string, now: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  return `overview-${slug(entryName)}${segment ? `-${slug(segment)}` : ""}-${date}.${ext}`;
}
