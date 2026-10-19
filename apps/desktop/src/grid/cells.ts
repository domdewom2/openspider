import { GridCellKind, type GridCell } from "@glideapps/glide-data-grid";
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
export function gridCell(value: CellValue | typeof LOADING, column: ColumnMeta, locale: string, tokens: TokenMap): GridCell {
  if (value === LOADING) return { kind: GridCellKind.Loading, allowOverlay: false };
  const text = formatCell(value, column, locale);
  if (column.kind.kind === "bool" && value !== null) return { kind: GridCellKind.Boolean, data: Boolean(value), readonly: true, allowOverlay: false };
  let color = tokens.text;
  if (column.kind.kind === "url") color = tokens.link;
  if (column.key === "Status Code") {
    const status = Number(value);
    color = tokens[status >= 500 ? "status.5xx" : status >= 400 ? "status.4xx" : status >= 300 ? "status.3xx" : status >= 200 ? "status.2xx" : "status.none"];
  }
  return { kind: GridCellKind.Text, data: text, displayData: text, allowOverlay: false, readonly: true,
    contentAlign: column.align, themeOverride: { textDark: color } };
}
