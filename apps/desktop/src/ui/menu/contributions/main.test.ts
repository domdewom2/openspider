import { describe, it, expect } from "vitest";
import { BASE_MENU } from "../../../app/baseMenu";
import { i18n, makeT } from "../../../i18n";
import { catalogFixture } from "../../../features/help/__fixtures__/fx-ui-basic";
import { buildMenuModel, flattenItems } from "../build";
import { MENU_ORDER } from "../types";
import { findDuplicateBindings, findReservedCollisions, shortcutById, formatShortcut } from "../../shortcuts";
import { mainMenuContributions, type MainMenuState } from "./main";
const fixture: MainMenuState = { recent: [{ path: "/fixture/basic.ospider" }], mode: "list", mcpRunning: false, mcpAutostart: false, catalog: catalogFixture };

describe.each(["mac", "win"] as const)("UI-002 %s", (platform) => {
  for (const lang of ["de", "en"] as const) {
    const model = (hasCrawl: boolean) => buildMenuModel([...BASE_MENU, ...mainMenuContributions(fixture)], { platform, lang, t: makeT(lang), ctx: { hasCrawl }, native: true });
    // spec: UI-002
    it(`${lang}: vollständige Menüfolge, Modi und Katalog-Untermenüs`, () => {
      const m = model(true);
      expect(m.menus.map((menu) => menu.id)).toEqual(MENU_ORDER[platform]);
      expect(m.menus.map((menu) => `${menu.label}\n${flattenItems(menu.items).map((entry) =>
        `  ${entry.enabled ? "+" : "-"} ${entry.label}${entry.checked === undefined ? "" : entry.checked ? " [x]" : " [ ]"}${entry.shortcutText ? ` | ${entry.shortcutText}` : ""}${entry.disabledReason ? ` | ${entry.disabledReason}` : ""}`.trimEnd(),
      ).join("\n")}`).join("\n\n")).toMatchSnapshot();
      expect(m.menus.find((menu) => menu.id === "mode")?.items.map((entry) => [entry.action, entry.checked])).toEqual(["spider", "list", "serp", "apis", "compare"].map((name) => [`mode.${name}`, name === "list"]));
      const entries = flattenItems(m.menus.flatMap((menu) => menu.items));
      expect(entries.find((e) => e.id === "catalog.bulkExport.Pages With Uncrawlable Internal Outlinks")?.label).toBe("Pages With Uncrawlable Internal Outlinks");
      expect(entries.find((e) => e.id === "catalog.reports.JavaScript Console Log Summary")?.enabled).toBe(true);
      expect(entries.find((e) => e.id === "catalog.reports.Analysis report")?.disabledReason).toBeTruthy();
      expect(entries.find((e) => e.id === "recent.0")?.label).toBe("/fixture/basic.ospider");
      const own = mainMenuContributions(fixture).flatMap((contribution) => flattenSpecs(contribution.items));
      expect(own.filter((entry) => entry.labelKey && !entry.labelKey.startsWith("fixture-") && !i18n.has(lang, entry.labelKey))).toEqual([]);
    });
    // spec: UI-002
    it(`${lang}: fehlende Voraussetzungen deaktivieren Einträge mit Begründung`, () => {
      const all = flattenItems(model(false).menus.flatMap((m) => m.items));
      for (const id of ["file.saveAs", "analysis.start", "catalog.sitemaps.pages", "viz.crawlTree"]) {
        expect(all.find((e) => e.id === id)?.enabled, id).toBe(false);
        expect(all.find((e) => e.id === id)?.disabledReason, id).toBeTruthy();
      }
      expect(all.find((e) => e.id === "mcp.start")?.enabled).toBe(true);
      expect(all.find((e) => e.id === "mcp.stop")?.enabled).toBe(false);
      expect(all.filter((e) => e.label === "Speichern" || e.label === "Save")).toEqual([]);
    });
    // spec: UI-113 [P1]
    it(`${lang}: Menübelegung stammt aus dem Register; kein nativer Accelerator umgeht Fokusregeln`, () => {
      const m = model(true);
      for (const entry of flattenItems(m.menus.flatMap((menu) => menu.items))) {
        const shortcut = shortcutById(entry.action ?? "");
        if (!shortcut) continue;
        expect(entry.shortcutText).toBe(formatShortcut(shortcut.keys[platform], platform, lang));
        expect(entry.accelerator).toBeUndefined();
      }
      expect(m.nativeAccelerators).toEqual([]);
      expect(findReservedCollisions()).toEqual([]);
      expect(findDuplicateBindings()).toEqual([]);
    });
  }
});
import type { MenuItemSpec } from "../types";
function flattenSpecs(items: readonly MenuItemSpec[]): MenuItemSpec[] { return items.flatMap((i) => [i, ...flattenSpecs(i.items ?? [])]); }
