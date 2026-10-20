// Suche im Konfigurationsdialog: Seitentitel, Feldbeschriftungen und Feldpfade (z. B. `speed.threads`).
import type { TFn } from "../../../i18n";
import { fieldLabelKey } from "./fields";
import type { ResolvedPage } from "./registry";

export interface SearchHit {
  pageId: string;
  /** Feldpfad; `null` = die Seite selbst. */
  path: string | null;
  pageTitle: string;
  label: string;
}

const norm = (s: string): string => s.toLowerCase().normalize("NFKD").replace(/[̀-ͯ]/g, "");

/** Treffer für die Eingabe (alle Wörter müssen vorkommen); leere Eingabe liefert keine Treffer. */
export function searchPages(pages: readonly ResolvedPage[], query: string, t: TFn): SearchHit[] {
  const words = norm(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  const all = (hay: string) => words.every((w) => hay.includes(w));
  const hits: SearchHit[] = [];
  for (const p of pages) {
    const pageTitle = t(p.titleKey);
    const extra = (p.searchKeys ?? []).map((k) => t(k)).join(" ");
    if (all(norm(`${pageTitle} ${extra}`))) hits.push({ pageId: p.id, path: null, pageTitle, label: pageTitle });
    for (const f of p.fields) {
      const label = t(f.labelKey ?? fieldLabelKey(f.path));
      if (all(norm(`${label} ${f.path} ${pageTitle}`))) hits.push({ pageId: p.id, path: f.path, pageTitle, label });
    }
  }
  return hits;
}
