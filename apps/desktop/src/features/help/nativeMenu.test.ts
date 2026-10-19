import { expect, it, vi } from "vitest";
import { buildMenuModel } from "../../ui/menu/build";
import { applyNativeMenu } from "../../ui/menu/tauriMenu";
import { mainMenuContributions } from "../../ui/menu/contributions/main";
import { makeT } from "../../i18n";
import { catalogFixture } from "./__fixtures__/fx-ui-basic";

const nativeItems = vi.hoisted(() => [] as Record<string, unknown>[]);
vi.mock("@tauri-apps/api/menu", () => {
  const factory = { new: vi.fn(async (options: Record<string, unknown>) => {
    nativeItems.push(options);
    return { close: async () => {}, setAsAppMenu: async () => {}, setAsWindowMenu: async () => {} };
  }) };
  return { Menu: factory, MenuItem: factory, CheckMenuItem: factory, PredefinedMenuItem: factory, Submenu: factory };
});

// spec: UI-002, UI-113 [P1]
it.each(["mac", "win"] as const)("%s: native Menüs haben Accelerator-Spalten und kurze Beschriftungen", async platform => {
  nativeItems.length = 0;
  const model = buildMenuModel(mainMenuContributions({ recent: [], mode: "spider", mcpRunning: false, mcpAutostart: false, catalog: catalogFixture }), {
    platform, lang: "en", t: makeT("en"), native: true, ctx: { hasCrawl: true },
  });
  await applyNativeMenu(model);
  expect(nativeItems.find(item => item.id === "file.new")).toMatchObject({ text: "New crawl", accelerator: "CmdOrCtrl+N" });
  expect(nativeItems.find(item => item.id === "mcp.stop")).toMatchObject({ text: "Stop server", enabled: false });
  expect(nativeItems.find(item => item.id === "help.updates")).toMatchObject({ text: "Check for updates", enabled: false });
});
