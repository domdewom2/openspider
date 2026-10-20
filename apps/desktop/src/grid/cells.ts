import type { ColumnMeta } from "../ipc/gen/ColumnMeta";
import { LOADING, type CellValue } from "../data/ViewDataSource";
import type { TokenMap } from "../ui/tokens/tokens";

const numbers = new Map<string, Intl.NumberFormat>();
const dates = new Map<string, Intl.DateTimeFormat>();
function number(value: number, locale: string, decimals = 0): string {
  const key = `${locale}:${decimals}`;
  if (!numbers.has(key)) numbers.set(key, new Intl.NumberFormat(locale, { minimumFractionDigits: decimals, maximumFractionDigits: decimals }));
  return numbers.get(key)!.format(value);
}
export function formatCell(value: CellValue, column: ColumnMeta, locale: string): string {
  if (value === null) return "";
  if (typeof value === "number") {
    const decimals = column.kind.kind === "float" ? column.kind.decimals : column.kind.kind === "seconds3" ? 3 : 0;
    return number(value, locale, decimals) + (column.key === "Percent" || column.key.includes("%") ? " %" : "");
  }
  if (column.kind.kind === "timestamp") {
    const date = new Date(String(value));
    if (!Number.isNaN(date.valueOf())) {
      if (!dates.has(locale)) dates.set(locale, new Intl.DateTimeFormat(locale, { dateStyle: "short", timeStyle: "short", timeZone: "UTC" }));
      return dates.get(locale)!.format(date);
    }
  }
  return String(value);
}
export function cellTooltip(value: CellValue, column: ColumnMeta, locale: string): string {
  if (value === null) return "";
  if (typeof value === "number" && /bytes/i.test(column.key)) return `${number(value / 1024, locale, 2)} KB / ${number(value / 1048576, locale, 2)} MB`;
  const text = String(value);
  return text.length > 2000 ? `${text.slice(0, 2000)}…` : text;
}
export interface CanvasCell { text: string; color: string; align: CanvasTextAlign; loading: boolean }
export function gridCell(value: CellValue | typeof LOADING, column: ColumnMeta, locale: string, tokens: TokenMap): CanvasCell {
  if (value === LOADING) return { text: "…", color: tokens["text.muted"], align: column.align, loading: true };
  let color = tokens.text;
  if (column.kind.kind === "url") color = tokens.link;
  if (column.key === "Status Code") {
    const status = Number(value);
    color = tokens[status >= 500 ? "status.5xx" : status >= 400 ? "status.4xx" : status >= 300 ? "status.3xx" : status >= 200 ? "status.2xx" : "status.none"];
  }
  return { text: formatCell(value, column, locale), color, align: column.align, loading: false };
}

/** Binary search bounds text measurement work even for very long cell values. */
export function ellipsis(measure: (text: string) => number, text: string, width: number): string {
  if (measure(text) <= width) return text;
  let lo = 0, hi = text.length;
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if (measure(text.slice(0, mid) + "…") <= width) lo = mid;
    else hi = mid - 1;
  }
  // Never split a UTF-16 surrogate pair at the clipping boundary.
  if (lo > 0 && /[\uD800-\uDBFF]/.test(text[lo - 1])) lo--;
  return text.slice(0, lo) + "…";
}
