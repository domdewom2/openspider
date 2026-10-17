import type { FC } from "react";

/**
 * Ein Abschnitt des Einstellungsfensters als Modul `features/settings/sections/<id>/index.tsx`, das `section` exportiert
 * (M2-F04: Allgemein und Oberfläche; die übrigen Abschnitte liefern M2-U14/U27 als Module nach – die Rahmen-Registry
 * findet sie per Glob, ohne dass der Rahmen geändert wird).
 */
export interface SettingsSection {
  id: string;
  order: number;
  /** Fluent-Schlüssel der Überschrift. */
  titleKey: string;
  Component: FC;
}

/** Abschnitte laut M2-ui-concept 1.5 in Anzeigereihenfolge; fehlt das Modul, zeigt der Rahmen einen Platzhalter. */
export const KNOWN_SECTIONS: readonly { id: string; order: number }[] = [
  { id: "general", order: 10 },
  { id: "appearance", order: 20 },
  { id: "storage", order: 30 },
  { id: "crawls", order: 40 },
  { id: "exports", order: 50 },
  { id: "network", order: 60 },
  { id: "accounts", order: 70 },
  { id: "external-tools", order: 80 },
  { id: "mcp", order: 90 },
];

export const sectionTitleKey = (id: string): string => `settings-section-${id}`;
