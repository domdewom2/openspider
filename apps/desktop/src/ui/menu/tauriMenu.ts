// Anwenden des Menümodells auf die native Menüleiste (Tauri `@tauri-apps/api/menu`).
import { Menu, MenuItem, CheckMenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
import { runAction } from "../actions";
import type { MenuModel, PredefinedKind, ResolvedItem } from "./types";

type Predefined = NonNullable<Parameters<typeof PredefinedMenuItem.new>[0]>["item"];

const PREDEFINED: Record<PredefinedKind, Predefined> = {
  undo: "Undo",
  redo: "Redo",
  cut: "Cut",
  copy: "Copy",
  paste: "Paste",
  selectAll: "SelectAll",
  services: "Services",
  hide: "Hide",
  hideOthers: "HideOthers",
  showAll: "ShowAll",
  minimize: "Minimize",
  maximize: "Maximize",
  bringAllToFront: "BringAllToFront",
};

/** Additive Feature-Optionen: vorhandene Beiträge behalten ihre bisherige Beschriftung. */
function nativeItemText(item: ResolvedItem): string {
  return item.label
    + (item.shortcutInLabel && item.shortcutText ? ` (${item.shortcutText})` : "")
    + (item.showDisabledReason && !item.enabled && item.disabledReason ? ` — ${item.disabledReason}` : "");
}

async function toNative(item: ResolvedItem): Promise<MenuItem | CheckMenuItem | PredefinedMenuItem | Submenu> {
  switch (item.kind) {
    case "separator":
      return PredefinedMenuItem.new({ item: "Separator" });
    case "predefined":
      return PredefinedMenuItem.new({ item: PREDEFINED[item.predefined as PredefinedKind], ...(item.label ? { text: item.label } : {}) });
    case "submenu":
      return Submenu.new({ text: item.label, enabled: item.enabled, items: await Promise.all((item.items ?? []).map(toNative)) });
    case "check":
      return CheckMenuItem.new({
        id: item.id,
        text: nativeItemText(item),
        enabled: item.enabled,
        checked: item.checked ?? false,
        ...(item.accelerator ? { accelerator: item.accelerator } : {}),
        action: () => void runAction(item.action ?? item.id, { source: "menu" }),
      });
    default: {
      const quit = item.action === "app.quit";
      return MenuItem.new({
        id: item.id,
        text: nativeItemText(item),
        enabled: item.enabled,
        ...((item.accelerator || quit) ? { accelerator: item.accelerator ?? "CmdOrCtrl+Q" } : {}),
        // Quit is handled by the shell, also when a WebView is busy.
        ...(!quit ? { action: () => void runAction(item.action ?? item.id, { source: "menu" }) } : {}),
      });
    }
  }
}

let current: Menu | null = null;

/** Baut die native Menüleiste neu auf (Sprachwechsel ohne Neustart, UI-121) und ersetzt die alte. */
export async function applyNativeMenu(model: MenuModel): Promise<void> {
  const top = await Promise.all(
    model.menus.map(async (m) => Submenu.new({ text: m.label, items: await Promise.all(m.items.map(toNative)) })),
  );
  const menu = await Menu.new({ items: top });
  const previous = current;
  current = menu;
  if (model.platform === "mac") await menu.setAsAppMenu();
  else await menu.setAsWindowMenu();
  if (previous) await previous.close().catch(() => undefined);
}
