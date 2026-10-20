// Menü „Visualisierungen“ (UI-090, Wurzel = Start-URL) und Kontextmenü-Einträge einer URL (Wurzel = diese URL).
import type { MenuEntry } from "../../ui/primitives";
import type { MenuContribution } from "../../ui/menu/types";
import type { TFn } from "../../i18n";
import { GRAPH_KINDS, type VizWindowKind } from "./open";

export const ACTION_PREFIX = "viz.open.";

export function actionFor(kind: VizWindowKind): string {
  return ACTION_PREFIX + kind;
}

export const VIZ_MENU: readonly MenuContribution[] = [
  {
    menu: "visualisations",
    group: "viz-structure",
    order: 10,
    items: GRAPH_KINDS.map((kind) => ({
      id: actionFor(kind),
      action: actionFor(kind),
      labelKey: `viz-kind-${kind}`,
      enabled: (c) => c.hasCrawl === true,
      disabledReasonKey: "menu-reason-no-crawl",
    })),
  },
];

/** Einträge für das Kontextmenü einer URL (UI-110 → Visualisierungen); der Besitzer des Kontextmenüs bindet sie ein. */
export function urlMenuEntries(urlId: number, t: TFn, open: (kind: VizWindowKind, root: number) => void): MenuEntry[] {
  return [...GRAPH_KINDS, "anchorWords" as const].map((kind, i) => ({
    id: `viz-url-${kind}`,
    label: t(`viz-kind-${kind}`),
    onAction: () => open(kind, urlId),
    separatorBefore: i === 0,
  }));
}
