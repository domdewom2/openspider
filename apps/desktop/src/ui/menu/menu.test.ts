import { afterEach, describe, expect, it } from "vitest";
import { BASE_MENU } from "../../app/baseMenu";
import { makeT, setLanguage } from "../../i18n";
import { buildMenuModel, flattenItems, menuLabelKey, toNativeAccelerator } from "./build";
import { startMenuController } from "./controller";
import { allMenuContributions, registerMenuContributions } from "./registry";
import { MENU_ORDER, type MenuContribution, type MenuModel } from "./types";

const build = (platform: "mac" | "win", lang = "en", ctx = {}, native = false, extra: readonly MenuContribution[] = []) =>
  buildMenuModel([...BASE_MENU, ...extra], { platform, lang, t: makeT(lang), ctx, native });

afterEach(async () => {
  await setLanguage("en");
});

describe("Menüaufbau (UI-002, F-5)", () => {
  // spec: UI-002
  it("Eintrag mit override ersetzt die Definition gleicher ID und bleibt an der ersten Stelle", () => {
    const modes: MenuContribution = { menu: "mode", group: "modes", order: 10, items: [
      { id: "mode.spider", label: "Spider", action: "mode.spider" },
      { id: "mode.apis", label: "Generic", action: "mode.apis" },
      { id: "mode.compare", label: "Compare", action: "mode.compare" },
    ] };
    const feature = (override: boolean): MenuContribution => ({ menu: "mode", group: "variants", order: 40, items: [
      { id: "mode.apis", override, label: "APIs", action: "mode.apis", checked: () => true },
    ] });
    const ids = (c: MenuContribution) => build("win", "en", {}, false, [modes, c]).menus.find((m) => m.id === "mode")?.items;
    expect(ids(feature(true))?.map((i) => i.id)).toEqual(["mode.spider", "mode.apis", "mode.compare"]);
    expect(ids(feature(true))?.[1]).toMatchObject({ label: "APIs", kind: "check", checked: true });
    // ohne override gilt weiter: die später einsortierte Definition steht an ihrer eigenen Stelle
    expect(ids(feature(false))?.map((i) => i.id)).toEqual(["mode.spider", "mode.compare", "sep.mode.variants", "mode.apis"]);
  });

  // spec: UI-002
  it("macOS: App-Menü ganz links, „Bearbeiten“ direkt nach „Datei“, „Fenster“ vor „Hilfe“", () => {
    const ids = build("mac").menus.map((m) => m.id);
    expect(ids).toEqual(["app", "file", "edit", "view", "window", "help"]);
    // Reihenfolge entspricht der Gesamtreihenfolge laut Spec, nur ohne leere Fachmenüs
    const order = MENU_ORDER.mac;
    expect(ids.map((i) => order.indexOf(i))).toEqual([...ids.map((i) => order.indexOf(i))].sort((a, b) => a - b));
    expect(order.indexOf("edit")).toBe(order.indexOf("file") + 1);
    expect(order.indexOf("window")).toBe(order.indexOf("help") - 1);
  });

  // spec: UI-002
  it("Windows: kein Bearbeiten- und kein Fenster-Menü, Einstellungen und Beenden unter „Datei“", () => {
    const m = build("win");
    expect(m.menus.map((x) => x.id)).toEqual(["file", "view", "help"]);
    const file = flattenItems(m.menus[0]?.items ?? []).map((i) => i.id);
    expect(file).toContain("file.settings");
    expect(file).toContain("file.quit");
    expect(MENU_ORDER.win).not.toContain("edit");
    expect(MENU_ORDER.win).not.toContain("window");
  });

  // spec: UI-002
  it("macOS-Bearbeiten-Menü besteht aus vordefinierten Systemeinträgen mit App-Beschriftung", () => {
    const edit = build("mac", "de").menus.find((m) => m.id === "edit");
    const predefined = edit?.items.filter((i) => i.kind === "predefined").map((i) => i.predefined);
    expect(predefined).toEqual(["undo", "redo", "cut", "copy", "paste", "selectAll"]);
    expect(edit?.items.find((i) => i.predefined === "copy")?.label).toBe("Kopieren");
  });

  // spec: UI-002
  it("Windows registriert keine Accelerators für Strg+C/V/X/A/Z; andere Kürzel sind nativ, wenn das Menü nativ ist", () => {
    const extra: MenuContribution[] = [
      {
        menu: "file",
        group: "x",
        order: 50,
        items: [
          { id: "x.copy", labelKey: "shortcut-table-copy", action: "table.copy" },
          { id: "x.all", labelKey: "shortcut-table-select-all", action: "table.selectAll" },
          { id: "x.remove", labelKey: "shortcut-table-remove", action: "table.remove" },
          { id: "x.back", labelKey: "shortcut-nav-back", action: "nav.back" },
        ],
      },
    ];
    const m = build("win", "en", {}, true, extra);
    const items = flattenItems(m.menus.flatMap((x) => x.items));
    const by = (id: string) => items.find((i) => i.id === id);
    expect(by("x.copy")?.accelerator).toBeUndefined();
    expect(by("x.all")?.accelerator).toBeUndefined();
    expect(by("x.remove")?.accelerator).toBeUndefined();
    expect(by("x.back")?.accelerator).toBeUndefined();
    expect(by("x.copy")?.shortcutText).toBe("Ctrl+C"); // Beschriftung bleibt
    expect(by("file.new")?.accelerator).toBe("CmdOrCtrl+N");
    expect(m.nativeAccelerators).toContain("crawl.new");
    expect(m.nativeAccelerators).not.toContain("table.copy");
    // ohne natives Menü (Browser/E2E-A): nur Beschriftung
    const web = build("win", "en", {}, false);
    expect(flattenItems(web.menus.flatMap((x) => x.items)).some((i) => i.accelerator)).toBe(false);
    expect(web.nativeAccelerators).toEqual([]);
  });

  // spec: UI-002
  it("jeder Eintrag mit Kürzel zeigt es plattformgerecht an (⌘ auf macOS, Strg/Ctrl auf Windows)", () => {
    const mac = flattenItems(build("mac").menus.flatMap((m) => m.items));
    expect(mac.find((i) => i.id === "file.new")?.shortcutText).toBe("⌘N");
    expect(mac.find((i) => i.id === "app.settings")?.shortcutText).toBe("⌘,");
    const winDe = flattenItems(build("win", "de").menus.flatMap((m) => m.items));
    expect(winDe.find((i) => i.id === "file.new")?.shortcutText).toBe("Strg+N");
    expect(winDe.find((i) => i.id === "file.settings")?.shortcutText).toBe("Strg+,");
    const winEn = flattenItems(build("win", "en").menus.flatMap((m) => m.items));
    expect(winEn.find((i) => i.id === "view.sidebar")?.shortcutText).toBe("Ctrl+Shift+J");
  });

  // spec: UI-002
  it("Einträge ohne Voraussetzung sind deaktiviert und nennen den Grund", () => {
    const without = flattenItems(build("win", "en", { hasCrawl: false }).menus.flatMap((m) => m.items)).find((i) => i.id === "file.saveAs");
    expect(without?.enabled).toBe(false);
    expect(without?.disabledReason).toBe("Open or start a crawl first.");
    const withCrawl = flattenItems(build("win", "en", { hasCrawl: true }).menus.flatMap((m) => m.items)).find((i) => i.id === "file.saveAs");
    expect(withCrawl?.enabled).toBe(true);
    expect(withCrawl?.disabledReason).toBeUndefined();
  });

  // spec: UI-002
  it("Menütexte sind vollständig übersetzt; fehlende Texte fallen auf Englisch zurück, nie auf einen Schlüssel", () => {
    for (const lang of ["de", "en"]) {
      const m = build("mac", lang);
      const labels = flattenItems(m.menus.flatMap((x) => [{ label: x.label } as never, ...x.items])).map((i) => i.label as string);
      for (const l of labels) {
        expect(l).not.toMatch(/^(menu|shortcut)-/);
      }
    }
    expect(build("mac", "fr").menus[0]?.label).toBe("OpenSpider");
    expect(makeT("de")(menuLabelKey("bulkExport"))).toBe("Bulk-Export");
  });

  it("gleiche Eintrags-ID in mehreren Beiträgen: der spätere ersetzt den früheren; Gruppen sind durch Trenner getrennt", () => {
    const m = build("win", "en", {}, false, [{ menu: "file", group: "crawl", order: 11, items: [{ id: "file.new", labelKey: "shortcut-crawl-open", action: "crawl.open" }] }]);
    const file = m.menus[0]?.items ?? [];
    expect(file.filter((i) => i.id === "file.new")).toHaveLength(1);
    expect(file.find((i) => i.id === "file.new")?.label).toBe("Open crawl");
    expect(file.some((i) => i.kind === "separator")).toBe(true);
    expect(file.at(-1)?.kind).not.toBe("separator");
  });

  it("toNativeAccelerator übersetzt Mod und Sondertasten", () => {
    expect(toNativeAccelerator("Mod+Shift+A", "win")).toBe("CmdOrCtrl+Shift+A");
    expect(toNativeAccelerator("Cmd+,", "mac")).toBe("CmdOrCtrl+,");
    expect(toNativeAccelerator("Shift+F10", "win")).toBe("Shift+F10");
    expect(toNativeAccelerator("Alt+Left", "win")).toBe("Alt+ArrowLeft");
  });
});

describe("Menü-Controller (Sprachwechsel ohne Neustart)", () => {
  // spec: UI-121
  it("baut das Menü bei Sprachwechsel neu auf, ohne Neustart, und wartet auf das Anwenden", async () => {
    const off = registerMenuContributions("test/base", BASE_MENU);
    const applied: MenuModel[] = [];
    const ctl = startMenuController({
      platform: "mac",
      getContext: () => ({}),
      apply: async (m) => {
        await Promise.resolve();
        applied.push(m);
      },
      native: false,
    });
    await ctl.rebuild();
    expect(applied.at(-1)?.menus.find((m) => m.id === "file")?.label).toBe("File");
    const before = ctl.rebuilds();
    await setLanguage("de");
    expect(ctl.rebuilds()).toBeGreaterThan(before);
    expect(applied.at(-1)?.lang).toBe("de");
    expect(applied.at(-1)?.menus.find((m) => m.id === "file")?.label).toBe("Datei");
    expect(applied.at(-1)?.menus.find((m) => m.id === "edit")?.label).toBe("Bearbeiten");
    ctl.stop();
    off();
    expect(allMenuContributions().some((c) => c.menu === "app")).toBe(false);
  });

  it("baut bei geändertem Zustand und neuen Beiträgen neu auf", async () => {
    const applied: MenuModel[] = [];
    let hasCrawl = false;
    let notify = () => undefined as void;
    const ctl = startMenuController({
      platform: "win",
      getContext: () => ({ hasCrawl }),
      subscribeContext: (cb) => {
        notify = cb;
        return () => undefined;
      },
      apply: (m) => void applied.push(m),
      native: false,
    });
    const off = registerMenuContributions("test/base", BASE_MENU);
    await new Promise((r) => setTimeout(r, 0));
    expect(flattenItems(applied.at(-1)?.menus.flatMap((m) => m.items) ?? []).find((i) => i.id === "file.saveAs")?.enabled).toBe(false);
    hasCrawl = true;
    notify();
    await new Promise((r) => setTimeout(r, 0));
    expect(flattenItems(applied.at(-1)?.menus.flatMap((m) => m.items) ?? []).find((i) => i.id === "file.saveAs")?.enabled).toBe(true);
    ctl.stop();
    off();
  });
});
