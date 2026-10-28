// Display names of canonical core names (tabs, filters, columns; UI-121, Konvention 05-ui „Spalten- und Filterschlüssel“): the core
// keeps the English names, the UI translates them through i18n. Keys are Fluent identifiers derived from the name (M2-U34, A02-04).
import type { TFn } from "./core";

export type CanonKind = "tab" | "filter" | "column";

/** `Client Error (4xx)` → `filter-client-error-4xx`; never contains characters Fluent would reject or read as an attribute. */
export function canonKey(kind: CanonKind, name: string): string {
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return `${kind}-${slug || "unnamed"}`;
}

export function tabLabel(t: TFn, id: string): string {
  return t(canonKey("tab", id), undefined, { fallback: id });
}

export function columnLabel(t: TFn, key: string): string {
  return t(canonKey("column", key), undefined, { fallback: key });
}

/**
 * A filter's label template carries at most one threshold placeholder (`Over {title.max_chars} Characters`); the filter id holds
 * the resolved value (`Over 60 Characters`). The catalogs use `{ $n }` for it, so the threshold the core resolved is shown as is.
 */
export function thresholdOf(template: string, id: string): string | undefined {
  if (!/\{[^}]+\}/.test(template)) return undefined;
  const parts = template.split(/\{[^}]+\}/).map(part => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  return new RegExp(`^${parts.join("(.+?)")}$`).exec(id)?.[1];
}

export function filterLabel(t: TFn, filter: { id: string; labelTemplate?: string }): string {
  const template = filter.labelTemplate ?? filter.id;
  const n = thresholdOf(template, filter.id);
  return t(canonKey("filter", template), n === undefined ? undefined : { n }, { fallback: filter.id });
}
