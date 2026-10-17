import type { ActionId } from "../shortcuts";
import type { Platform } from "../tokens/tokens";

/** Hauptmenüs in der Reihenfolge von UI-002; `app`, `edit` und `window` gibt es nur auf macOS (F-5). */
export type TopMenuId =
  | "app"
  | "file"
  | "edit"
  | "config"
  | "mode"
  | "bulkExport"
  | "reports"
  | "sitemaps"
  | "visualisations"
  | "crawlAnalysis"
  | "view"
  | "mcp"
  | "window"
  | "help";

export const MENU_ORDER: Record<Platform, readonly TopMenuId[]> = {
  mac: ["app", "file", "edit", "config", "mode", "bulkExport", "reports", "sitemaps", "visualisations", "crawlAnalysis", "view", "mcp", "window", "help"],
  win: ["file", "config", "mode", "bulkExport", "reports", "sitemaps", "visualisations", "crawlAnalysis", "view", "mcp", "help"],
};

/** Vom Betriebssystem bereitgestellte Einträge (macOS-Bearbeiten-Menü, App-Menü, Fenster). */
export type PredefinedKind =
  | "undo"
  | "redo"
  | "cut"
  | "copy"
  | "paste"
  | "selectAll"
  | "services"
  | "hide"
  | "hideOthers"
  | "showAll"
  | "minimize"
  | "maximize"
  | "bringAllToFront";

/** Zustand, aus dem sich Aktivierung und Markierung der Einträge ergeben (Selektoren auf den Stores). */
export type MenuContext = Readonly<Record<string, unknown>> & {
  hasCrawl?: boolean;
  crawlRunning?: boolean;
};

export interface MenuItemSpec {
  /** Eindeutig innerhalb des Menüs, z. B. `file.new`. */
  id: string;
  /** Fluent-Schlüssel der Beschriftung (bei `predefined` und Trennern entfällt sie). */
  labelKey?: string;
  /** Aktions-ID, die bei Auswahl läuft; zugleich Schlüssel im Kürzelregister, wenn `shortcut` fehlt. */
  action?: ActionId;
  /** Kürzel aus dem Register, falls abweichend von `action`. */
  shortcut?: ActionId;
  separator?: boolean;
  predefined?: PredefinedKind;
  items?: readonly MenuItemSpec[];
  platforms?: readonly Platform[];
  enabled?: (ctx: MenuContext) => boolean;
  checked?: (ctx: MenuContext) => boolean;
  /** Grund für „deaktiviert“ (Tooltip bzw. Statuszeile, UI-002). */
  disabledReasonKey?: string;
}

/** Beitrag eines Features (`features/<f>/menu.ts`) zu einem Hauptmenü. */
export interface MenuContribution {
  menu: TopMenuId;
  /** Gruppe innerhalb des Menüs; zwischen Gruppen steht ein Trenner. */
  group: string;
  /** Sortierung innerhalb des Menüs (aufsteigend, Gruppen bleiben zusammen). */
  order: number;
  items: readonly MenuItemSpec[];
}

export interface ResolvedItem {
  id: string;
  kind: "item" | "check" | "separator" | "submenu" | "predefined";
  label: string;
  action?: ActionId;
  predefined?: PredefinedKind;
  /** Anzeige des Kürzels in Plattformschreibweise (⌘N bzw. Strg+N). */
  shortcutText?: string;
  /** Natives Accelerator-Format für Tauri (`CmdOrCtrl+N`), nur wenn das Betriebssystem das Kürzel abfangen soll. */
  accelerator?: string;
  enabled: boolean;
  disabledReason?: string;
  checked?: boolean;
  items?: ResolvedItem[];
}

export interface ResolvedMenu {
  id: TopMenuId;
  label: string;
  items: ResolvedItem[];
}

export interface MenuModel {
  platform: Platform;
  lang: string;
  menus: ResolvedMenu[];
  /** Aktionen mit nativem Accelerator (der Dispatcher löst sie nicht zusätzlich aus). */
  nativeAccelerators: ActionId[];
}
