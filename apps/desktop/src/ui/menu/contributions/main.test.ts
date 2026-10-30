import { describe, it, expect } from "vitest";
import { BASE_MENU } from "../../../app/baseMenu";
import { i18n, makeT } from "../../../i18n";
import { catalogFixture } from "../../../features/help/__fixtures__/fx-ui-basic";
import { buildMenuModel, flattenItems } from "../build";
import { MENU_ORDER } from "../types";
import { findDuplicateBindings, findReservedCollisions, shortcutById, formatShortcut, canonicalCombo, isEditingCombo } from "../../shortcuts";
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
      expect(entries.find((e) => e.id === "export.bulk.Pages With Uncrawlable Internal Outlinks")?.label).toBe("Pages With Uncrawlable Internal Outlinks");
      expect(entries.find((e) => e.id === "export.report.JavaScript Console Log Summary")?.enabled).toBe(true);
      expect(entries.find((e) => e.id === "export.report.Analysis report")?.disabledReason).toBeTruthy();
      expect(entries.find((e) => e.id === "recent.0")?.label).toBe("/fixture/basic.ospider");
      const own = mainMenuContributions(fixture).flatMap((contribution) => flattenSpecs(contribution.items));
      expect(own.filter((entry) => entry.labelKey && !entry.labelKey.startsWith("fixture-") && !i18n.has(lang, entry.labelKey))).toEqual([]);
    });
    // spec: UI-002
    it(`${lang}: fehlende Voraussetzungen deaktivieren Einträge mit Begründung`, () => {
      const all = flattenItems(model(false).menus.flatMap((m) => m.items));
      for (const id of ["file.saveAs", "analysis.start", "export.sitemap.pages", "viz.crawlTree"]) {
        expect(all.find((e) => e.id === id)?.enabled, id).toBe(false);
        expect(all.find((e) => e.id === id)?.disabledReason, id).toBeTruthy();
      }
      expect(all.find((e) => e.id === "mcp.start")?.enabled).toBe(true);
      expect(all.find((e) => e.id === "mcp.stop")?.enabled).toBe(false);
      expect(all.filter((e) => e.label === "Speichern" || e.label === "Save")).toEqual([]);
    });
    // spec: UI-113 [P1]
    it(`${lang}: Menübelegung stammt aus dem Register; native Accelerators aus F04, Bearbeitungskürzel ausgenommen`, () => {
      const m = model(true);
      for (const entry of flattenItems(m.menus.flatMap((menu) => menu.items))) {
        const shortcut = shortcutById(entry.action ?? "");
        if (!shortcut) continue;
        expect(entry.shortcutText).toBe(formatShortcut(shortcut.keys[platform], platform, lang));
        if (isEditingCombo(canonicalCombo(shortcut.keys[platform], platform))) expect(entry.accelerator).toBeUndefined();
        else expect(entry.accelerator).toBeTruthy();
        expect(entry.shortcutInLabel).toBeUndefined();
        expect(entry.showDisabledReason).toBeUndefined();
      }
      expect(m.nativeAccelerators).toContain("crawl.new");
      expect(m.nativeAccelerators).toContain("crawl.open");
      expect(m.nativeAccelerators).not.toContain("table.copy");
      expect(findReservedCollisions()).toEqual([]);
      expect(findDuplicateBindings()).toEqual([]);
    });
  }
});
import type { MenuItemSpec } from "../types";
function flattenSpecs(items: readonly MenuItemSpec[]): MenuItemSpec[] { return items.flatMap((i) => [i, ...flattenSpecs(i.items ?? [])]); }

// spec: UI-002
it("U04 und die U16-Katalogbeiträge ergeben je genau ein Untermenü und einen Sitemap-Eintrag", () => {
  const u16 = [
    { menu: "bulkExport" as const, group: "export-Links", order: 10, items: [{ id: "export-group-bulk-Links", labelKey: "fixture-links", items: [{ id: "export.bulk.Pages With Uncrawlable Internal Outlinks", action: "export.bulk.Pages With Uncrawlable Internal Outlinks", labelKey: "fixture-outlinks" }] }] },
    { menu: "reports" as const, group: "export-JavaScript", order: 10, items: [{ id: "export-group-reports-JavaScript", labelKey: "fixture-js", items: [{ id: "export.report.JavaScript Console Log Summary", action: "export.report.JavaScript Console Log Summary", labelKey: "fixture-console" }] }] },
    { menu: "sitemaps" as const, group: "export-sitemaps", order: 10, items: [{ id: "export.sitemap.pages", action: "export.sitemap.pages", labelKey: "fixture-pages" }] },
  ];
  for (const contributions of [[...mainMenuContributions(fixture), ...u16], [...u16, ...mainMenuContributions(fixture)]]) {
    const model = buildMenuModel(contributions, { platform: "win", lang: "en", t: makeT("en"), ctx: { hasCrawl: true }, native: false });
    for (const menu of model.menus.filter(menu => ["bulkExport", "reports", "sitemaps"].includes(menu.id))) {
      const ids = flattenItems(menu.items).filter(item => item.kind !== "separator").map(item => item.id);
      expect(new Set(ids).size).toBe(ids.length);
      expect(menu.items.filter(item => item.kind !== "separator")).toHaveLength(1);
    }
  }
});

// spec: UI-124
it("uses index metadata for a recent label and retains the path-based action", () => {
  const recent = [{ path: "/fixture/uuid.ospider", label: "http://127.0.0.1:1234/fixture · 2026-10-29" }];
  const model = buildMenuModel(mainMenuContributions({ ...fixture, recent }), { platform: "mac", lang: "de", t: makeT("de"), ctx: {}, native: true });
  const entry = flattenItems(model.menus.flatMap(menu => menu.items)).find(entry => entry.id === "recent.0");
  expect(entry?.label).toBe(recent[0].label);
  expect(entry?.action).toBe("recent.0");
  expect(recent[0].path).toBe("/fixture/uuid.ospider");
});
