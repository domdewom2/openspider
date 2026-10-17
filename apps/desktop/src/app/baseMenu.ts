// Grundgerüst der Menüleiste (UI-002, F-5): macOS-App-Menü, „Bearbeiten“ direkt nach „Datei“, „Fenster“ vor „Hilfe“;
// Windows ohne Bearbeiten-Menü, Einstellungen und Beenden unter „Datei“. Die Fachmenüs (Konfiguration, Modus, Exporte …)
// hängen die Features über `features/<f>/menu.ts` ein.
import type { MenuContribution } from "../ui/menu/types";

export const BASE_MENU: readonly MenuContribution[] = [
  {
    menu: "app",
    group: "about",
    order: 10,
    items: [{ id: "app.about", labelKey: "menu-item-about", action: "app.about", platforms: ["mac"] }],
  },
  {
    menu: "app",
    group: "settings",
    order: 20,
    items: [{ id: "app.settings", labelKey: "menu-item-settings", action: "app.settings", platforms: ["mac"] }],
  },
  {
    menu: "app",
    group: "system",
    order: 30,
    items: [
      { id: "app.services", labelKey: "menu-item-services", predefined: "services" },
      { id: "app.hide", labelKey: "menu-item-hide", predefined: "hide" },
      { id: "app.hideOthers", labelKey: "menu-item-hide-others", predefined: "hideOthers" },
      { id: "app.showAll", labelKey: "menu-item-show-all", predefined: "showAll" },
    ],
  },
  {
    menu: "app",
    group: "quit",
    order: 40,
    items: [{ id: "app.quit", labelKey: "menu-item-quit", action: "app.quit", platforms: ["mac"] }],
  },
  {
    menu: "file",
    group: "crawl",
    order: 10,
    items: [
      { id: "file.new", labelKey: "shortcut-crawl-new", action: "crawl.new" },
      { id: "file.open", labelKey: "shortcut-crawl-open", action: "crawl.open" },
      { id: "file.saveAs", labelKey: "shortcut-crawl-save-as", action: "crawl.saveAs", enabled: (c) => c.hasCrawl === true, disabledReasonKey: "menu-reason-no-crawl" },
    ],
  },
  {
    menu: "file",
    group: "app",
    order: 90,
    items: [
      { id: "file.settings", labelKey: "menu-item-settings", action: "app.settings", platforms: ["win"] },
      { id: "file.quit", labelKey: "menu-item-quit", action: "app.quit", platforms: ["win"] },
    ],
  },
  {
    // Nur macOS: ohne diese Systemeinträge bedient die WebView Cmd+C/V/X/Z/A in Textfeldern nicht (F-5).
    menu: "edit",
    group: "edit",
    order: 10,
    items: [
      { id: "edit.undo", labelKey: "menu-item-undo", predefined: "undo" },
      { id: "edit.redo", labelKey: "menu-item-redo", predefined: "redo" },
      { id: "edit.sep1", separator: true },
      { id: "edit.cut", labelKey: "menu-item-cut", predefined: "cut" },
      { id: "edit.copy", labelKey: "menu-item-copy", predefined: "copy" },
      { id: "edit.paste", labelKey: "menu-item-paste", predefined: "paste" },
      { id: "edit.selectAll", labelKey: "menu-item-select-all", predefined: "selectAll" },
    ],
  },
  {
    menu: "view",
    group: "panes",
    order: 10,
    items: [
      { id: "view.detail", labelKey: "shortcut-view-toggle-detail", action: "view.toggleDetail" },
      { id: "view.sidebar", labelKey: "shortcut-view-toggle-sidebar", action: "view.toggleSidebar" },
    ],
  },
  {
    menu: "view",
    group: "zoom",
    order: 20,
    items: [
      { id: "view.zoomIn", labelKey: "shortcut-zoom-in", action: "zoom.in" },
      { id: "view.zoomOut", labelKey: "shortcut-zoom-out", action: "zoom.out" },
      { id: "view.zoomReset", labelKey: "shortcut-zoom-reset", action: "zoom.reset" },
    ],
  },
  {
    menu: "window",
    group: "window",
    order: 10,
    items: [
      { id: "window.minimize", labelKey: "menu-item-minimize", predefined: "minimize" },
      { id: "window.zoom", labelKey: "menu-item-zoom-window", predefined: "maximize" },
      { id: "window.front", labelKey: "menu-item-bring-to-front", predefined: "bringAllToFront" },
    ],
  },
  {
    menu: "help",
    group: "help",
    order: 10,
    items: [
      { id: "help.shortcuts", labelKey: "menu-item-shortcuts", action: "help.shortcuts" },
      { id: "help.about", labelKey: "menu-item-about", action: "app.about", platforms: ["win"] },
    ],
  },
];
