// Menü-Controller: baut das Modell aus den Beiträgen, wendet es nativ an und baut bei Sprachwechsel, geändertem Zustand
// oder neuen Beiträgen neu auf (UI-121: Sprachwechsel ohne Neustart inkl. Menü).
import { getT, onLanguageChanged, useLanguageStore } from "../../i18n";
import { isTauri } from "../../ipc/transport/tauri";
import { buildMenuModel } from "./build";
import { allMenuContributions, onMenuContributionsChanged } from "./registry";
import type { MenuContext, MenuModel } from "./types";
import type { Platform } from "../tokens/tokens";

export interface MenuController {
  /** Baut sofort neu auf und wartet auf das Anwenden. */
  rebuild(): Promise<MenuModel>;
  model(): MenuModel | null;
  /** Anzahl der Neuaufbauten (E2E: „Menü wurde neu aufgebaut“). */
  rebuilds(): number;
  stop(): void;
}

declare global {
  interface Window {
    /** Letztes Menümodell (Tests/E2E-A; in der Tauri-App liegt das Menü nativ vor). */
    __osMenu?: { model: MenuModel; rebuilds: number };
  }
}

export interface MenuControllerOptions {
  platform: Platform;
  getContext: () => MenuContext;
  /** Hängt den Zustand an (z. B. Session-Store); liefert die Abmeldung. */
  subscribeContext?: (cb: () => void) => () => void;
  /** Überschreibt das Anwenden (Tests); Standard: native Menüleiste in Tauri, sonst `window.__osMenu`. */
  apply?: (model: MenuModel) => Promise<void> | void;
  native?: boolean;
}

export function startMenuController(opts: MenuControllerOptions): MenuController {
  const native = opts.native ?? isTauri();
  let last: MenuModel | null = null;
  let count = 0;
  let queued = false;
  let chain: Promise<unknown> = Promise.resolve();

  async function applyModel(model: MenuModel): Promise<void> {
    if (opts.apply) return void (await opts.apply(model));
    if (native) {
      const { applyNativeMenu } = await import("./tauriMenu");
      await applyNativeMenu(model);
    }
    if (typeof window !== "undefined") window.__osMenu = { model, rebuilds: count };
  }

  function rebuild(): Promise<MenuModel> {
    const run = async () => {
      const lang = useLanguageStore.getState().lang;
      const model = buildMenuModel(allMenuContributions(), { platform: opts.platform, lang, t: getT(), ctx: opts.getContext(), native });
      count++;
      last = model;
      await applyModel(model);
      return model;
    };
    const p = chain.then(run, run);
    chain = p.catch(() => undefined);
    return p;
  }

  const schedule = () => {
    if (queued) return;
    queued = true;
    queueMicrotask(() => {
      queued = false;
      void rebuild();
    });
  };

  const offs = [onLanguageChanged(async () => void (await rebuild())), onMenuContributionsChanged(schedule)];
  if (opts.subscribeContext) offs.push(opts.subscribeContext(schedule));
  void rebuild();

  return {
    rebuild,
    model: () => last,
    rebuilds: () => count,
    stop: () => offs.forEach((o) => o()),
  };
}
