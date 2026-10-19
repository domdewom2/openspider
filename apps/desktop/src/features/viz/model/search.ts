// URL-Suche im Visualisierungsfenster (UI-090): Teilstring auf Address, Groß/Klein egal, „i von n“, Enter/Shift+Enter.
import type { VizTree } from "./tree";

/** Treffer in Breitensuche-Reihenfolge über alle geladenen Knoten; Pfadknoten ohne URL werden über ihren Pfad gefunden. */
export function findMatches(tree: VizTree, query: string): number[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: number[] = [];
  for (const id of tree.order) {
    const n = tree.get(id);
    if (!n) continue;
    const text = (n.address ?? n.label).toLowerCase();
    if (text.includes(q)) out.push(id);
  }
  return out;
}

export interface SearchState {
  query: string;
  matches: number[];
  /** Index des aktuellen Treffers, -1 ohne Treffer. */
  index: number;
}

export const EMPTY_SEARCH: SearchState = { query: "", matches: [], index: -1 };

export function search(tree: VizTree, query: string): SearchState {
  const matches = findMatches(tree, query);
  return { query, matches, index: matches.length ? 0 : -1 };
}

/** Nächster (+1) bzw. vorheriger (-1) Treffer, zyklisch. */
export function step(s: SearchState, dir: 1 | -1): SearchState {
  if (!s.matches.length) return s;
  const n = s.matches.length;
  return { ...s, index: (s.index + dir + n) % n };
}

export function current(s: SearchState): number | null {
  return s.index >= 0 ? (s.matches[s.index] ?? null) : null;
}
