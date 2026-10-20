import type { FC } from "react";

export type FieldKind = "auto" | "bool" | "int" | "number" | "enum" | "radio" | "text" | "lines" | "storeCrawl";

/**
 * Ein Feld einer Konfigurationsseite. Der Pfad ist der TOML-Pfad der Konfiguration (`limits.max_urls`) und zugleich der
 * Sprungziel-Anker (`data-setting-anchor`). Typ, Grenzen und Aufzählungswerte liest der Rahmen aus dem Schema des Kerns.
 */
export interface FieldDef {
  path: string;
  /** Darstellung; `auto` leitet sie aus dem Schema ab. */
  kind?: FieldKind;
  /** Fluent-Schlüssel der Beschriftung; Standard `config-f-<pfad>` (Punkte und Unterstriche werden zu `-`). */
  labelKey?: string;
  /** Fluent-Schlüssel eines Hinweistextes. */
  hintKey?: string;
  /** Beschriftung der Auswahl „nicht gesetzt“ bei optionalen Zahlen und Texten (Standard: unbegrenzt). */
  nullLabelKey?: string;
  /** Einheit hinter dem Eingabefeld. */
  unitKey?: string;
  /** Abschnittsüberschrift über dem Feld (Fluent-Schlüssel). */
  sectionKey?: string;
  /** Nur für Suche und Sprungziele: Die Seite zeichnet das Steuerelement selbst (Tabellen, Editoren). */
  hidden?: boolean;
  /** Feld nur aktiv, wenn die Bedingung erfüllt ist (Pfad → Wert). */
  enabledWhen?: { path: string; equals: unknown };
}

/** Eine Seite der Seitennavigation. Seitenmodule liegen unter `features/config/pages/<id>/index.tsx` und exportieren `page` oder `pages`. */
export interface ConfigPage {
  id: string;
  /** Gruppe in der Navigation (`spider`, `custom`); ohne Gruppe steht die Seite auf der obersten Ebene. */
  group?: string;
  order: number;
  titleKey: string;
  /** Sprungziele (Feldpfade oder Präfixe), die diese Seite öffnen. */
  anchors: readonly string[];
  /** Felder für die Suche und für die Seitenansicht. */
  fields: readonly FieldDef[];
  /** Zusätzliche Suchbegriffe (Fluent-Schlüssel). */
  searchKeys?: readonly string[];
  Component: FC;
}

/** Bekannte Seiten laut M2-ui-concept 1.5; fehlt das Modul (M2-U13B liefert es nach), zeigt der Rahmen einen Hinweis. */
export interface KnownPage {
  id: string;
  group?: string;
  order: number;
}

export const GROUP_ORDER: Record<string, number> = { spider: 10, custom: 100 };

export const KNOWN_PAGES: readonly KnownPage[] = [
  { id: "spider-crawl", group: "spider", order: 10 },
  { id: "spider-extraction", group: "spider", order: 20 },
  { id: "spider-limits", group: "spider", order: 30 },
  { id: "spider-rendering", group: "spider", order: 40 },
  { id: "spider-advanced", group: "spider", order: 50 },
  { id: "spider-presets", group: "spider", order: 60 },
  { id: "robots", order: 70 },
  { id: "rewrite", order: 80 },
  { id: "include-exclude", order: 90 },
  { id: "speed", order: 95 },
  { id: "user-agent", order: 96 },
  { id: "headers", order: 97 },
  { id: "custom-search", group: "custom", order: 100 },
  { id: "custom-extraction", group: "custom", order: 110 },
  { id: "custom-js", group: "custom", order: 120 },
  { id: "link-positions", group: "custom", order: 130 },
  { id: "api", order: 140 },
  { id: "auth", order: 150 },
  { id: "segments", order: 160 },
  { id: "analysis", order: 170 },
  { id: "change-detection", order: 180 },
  { id: "profiles", order: 200 },
];

export const pageTitleKey = (id: string): string => `config-page-${id}`;
export const groupTitleKey = (id: string): string => `config-group-${id}`;
