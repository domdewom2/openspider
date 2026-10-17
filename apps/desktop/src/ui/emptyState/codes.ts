// Leer-, Hinweis- und Voraussetzungszustände (UI-016): abschließende Codes, Rangfolge und Auswahl.
import type { EmptyAction } from "../../ipc/gen/EmptyAction";
import type { EmptyCode } from "../../ipc/gen/EmptyCode";
import type { EmptyState } from "../../ipc/gen/EmptyState";

export type { EmptyAction, EmptyCode, EmptyState };

/** Rang laut UI-016: bei mehreren Ursachen gilt der kleinste Rang. */
export const EMPTY_RANK: Readonly<Record<EmptyCode, number>> = {
  NO_SELECTION: 1,
  MODE_UNSUPPORTED: 2,
  NEEDS_JS_RENDERING: 3,
  FEATURE_DISABLED: 4,
  HTML_NOT_STORED: 5,
  RENDERED_HTML_NOT_STORED: 5,
  API_NOT_CONNECTED: 6,
  NEEDS_CRAWL_ANALYSIS: 7,
  NO_DATA_YET: 8,
  NO_MATCH: 9,
  NOT_APPLICABLE: 10,
};

export const EMPTY_CODES = Object.keys(EMPTY_RANK) as EmptyCode[];

/** Codes mit Argument in Klammern: `FEATURE_DISABLED(cookies)`. */
export const CODES_WITH_ARG: ReadonlySet<EmptyCode> = new Set<EmptyCode>(["FEATURE_DISABLED", "API_NOT_CONNECTED", "NEEDS_CRAWL_ANALYSIS"]);

/** Wert des Attributs `data-empty-state` (für E2E). */
export function emptyStateAttr(s: Pick<EmptyState, "code" | "arg">): string {
  return CODES_WITH_ARG.has(s.code) && s.arg ? `${s.code}(${s.arg})` : s.code;
}

/** Fluent-Schlüssel des Hinweistextes: `NEEDS_JS_RENDERING` → `empty-needs-js-rendering`. */
export function emptyMessageKey(code: EmptyCode): string {
  return "empty-" + code.toLowerCase().replace(/_/g, "-");
}

/** Wählt unter mehreren erfüllten Ursachen die mit dem kleinsten Rang (stabil bei Gleichstand). */
export function pickEmptyState(candidates: readonly EmptyState[]): EmptyState | null {
  let best: EmptyState | null = null;
  for (const c of candidates) if (best === null || EMPTY_RANK[c.code] < EMPTY_RANK[best.code]) best = c;
  return best;
}
