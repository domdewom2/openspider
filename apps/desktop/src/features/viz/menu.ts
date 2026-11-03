// Menü „Visualisierungen“ (UI-090, Wurzel = Start-URL) und Kontextmenü-Einträge einer URL (Wurzel = diese URL).
// Die Einträge des Hauptmenüs stehen in `ui/menu/contributions/main.ts` (Gruppe „visualisations“, Aktionen
// `viz.<art>`); dieses Paket registriert die Aktionen (VizHost). `VIZ_MENU` bleibt für Fenster ohne Hauptmenü leer.
import type { MenuEntry } from "../../ui/primitives";
import type { MenuContribution } from "../../ui/menu/types";
import type { TFn } from "../../i18n";
import { WINDOW_KINDS, type VizWindowKind } from "./open";

export const ACTION_PREFIX = "viz.";

/** Aktions-ID wie im Hauptmenü (`viz.crawlTree`, `viz.directoryGraph3d`, `viz.bodyWords` …). */
export function actionFor(kind: VizWindowKind): string {
  return ACTION_PREFIX + kind;
}

/** Arten mit Menüaktion: alle Fensterarten (Wortwolken öffnen mit der Start-URL). */
export const MENU_KINDS: readonly VizWindowKind[] = WINDOW_KINDS;

export const VIZ_MENU: readonly MenuContribution[] = [];

/** Einträge für das Kontextmenü einer URL (UI-110 → Visualisierungen); der Besitzer des Kontextmenüs bindet sie ein. */
export function urlMenuEntries(urlId: number, t: TFn, open: (kind: VizWindowKind, root: number) => void): MenuEntry[] {
  return WINDOW_KINDS.map((kind, i) => ({
    id: `viz-url-${kind}`,
    label: t(`viz-kind-${kind}`),
    onAction: () => open(kind, urlId),
    separatorBefore: i === 0,
  }));
}
