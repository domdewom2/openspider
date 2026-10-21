// Reine Funktionen des SERP-Tabs (UI-037, ONPAGE-194). Pixelwerte kommen ausschließlich aus dem Kern (`detail.serpMeasure`);
// das Frontend misst nie selbst (UI-038).
import type { SerpMeasureRow } from "../../../ipc/gen/SerpMeasureRow";

export const SERP_DEVICES = ["desktop", "mobile", "tablet"] as const;
export type SerpPrefixKind = "none" | "date" | "custom";
export const SERP_ELLIPSIS = " …";
export const DATE_SEPARATOR = " — ";

/** Codepoints statt UTF-16-Einheiten: Längen und Schnittpositionen zählen Zeichen (ONPAGE-003, A-2). */
export const codepoints = (text: string): string[] => Array.from(text);

/** Angezeigter Text: der Kern liefert die Schnittposition (`cutAt`, in Zeichen) am letzten ganzen Wort. */
export function displayText(full: string, row: Pick<SerpMeasureRow, "truncated" | "cutAt"> | undefined): string {
  if (!row?.truncated || row.cutAt === null || row.cutAt === undefined) return full;
  return codepoints(full).slice(0, row.cutAt).join("").trimEnd() + SERP_ELLIPSIS;
}
/** Kommagetrennte Suchbegriffe, ohne Leerstellen und Dubletten. */
export function parseTerms(input: string): string[] {
  return [...new Set(input.split(",").map(term => term.trim()).filter(term => term.length > 0))];
}
export interface Segment { text: string; bold: boolean }
const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
/** Zerlegt `text` in normale und fett darzustellende Abschnitte (Groß-/Kleinschreibung egal). */
export function boldSegments(text: string, terms: readonly string[]): Segment[] {
  if (terms.length === 0 || text.length === 0) return text ? [{ text, bold: false }] : [];
  const pattern = new RegExp(`(${[...terms].sort((a, b) => b.length - a.length).map(escapeRegExp).join("|")})`, "giu");
  // `split` mit Fanggruppe liefert abwechselnd Text und Treffer; leere Teile erst nach der Zuordnung entfernen.
  return text.split(pattern).map((part, index) => ({ text: part, bold: index % 2 === 1 })).filter(part => part.text.length > 0);
}
/** Datumspräfix: „13.10.2026 — “ (de) bzw. „Oct 13, 2026 — “ (en); zählt zur Pixelbreite der Beschreibung. */
export function datePrefix(isoDate: string, lang: string): string {
  const date = new Date(`${isoDate.slice(0, 10)}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return "";
  const text = lang.startsWith("de")
    ? new Intl.DateTimeFormat("de-DE", { day: "2-digit", month: "2-digit", year: "numeric", timeZone: "UTC" }).format(date)
    : new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", year: "numeric", timeZone: "UTC" }).format(date);
  return text + DATE_SEPARATOR;
}
export function prefixText(kind: SerpPrefixKind, isoDate: string, custom: string, lang: string): string {
  return kind === "date" ? datePrefix(isoDate, lang) : kind === "custom" ? custom : "";
}
/** Datum (ISO) eines Crawl-Zeitstempels; ohne gültigen Wert der übergebene Ersatz. */
export function crawlDate(timestamp: unknown, fallback: Date): string {
  const date = typeof timestamp === "string" ? new Date(timestamp) : undefined;
  return (date && !Number.isNaN(date.getTime()) ? date : fallback).toISOString().slice(0, 10);
}
export function sameText(a: string, b: string): boolean { return a === b; }
