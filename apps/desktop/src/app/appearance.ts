// Erscheinungsbild und Sprache laufend auf das Fenster anwenden (UI-120, UI-121): Theme hell/dunkel/System ohne Neustart,
// Akzent, Dichte, Zebra, Zoom über den WebView-Zoom, Sprachwechsel ohne Neustart.
import { setLanguage } from "../i18n";
import { events } from "../ipc/events.commands";
import { isTauri } from "../ipc/transport/tauri";
import { usePrefs } from "../state/prefs";
import { currentPlatform } from "../ui/platform";
import { applyAppearance, systemPrefersDark, type AppearanceState } from "../ui/tokens/theme";
import type { Platform } from "../ui/tokens/tokens";

export interface AppearanceSyncOptions {
  root?: HTMLElement;
  platform?: Platform;
  /** Zoom auf das Fenster anwenden (Standard: Tauri-WebView-Zoom, sonst CSS-Zoom). */
  setZoom?: (factor: number, root: HTMLElement) => Promise<void> | void;
  /** Dieses Fenster meldet das Live-Intervall an den Kern (nur das Hauptfenster). */
  reportLiveInterval?: boolean;
}

/** WebView-Zoom (`Webview.setZoom`, 2.8); außerhalb von Tauri CSS `zoom` auf dem Wurzelelement. */
export async function defaultSetZoom(factor: number, root: HTMLElement): Promise<void> {
  if (isTauri()) {
    try {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      await getCurrentWebview().setZoom(factor);
      root.style.removeProperty("zoom");
      return;
    } catch {
      /* ohne Berechtigung: Rückfall auf CSS-Zoom */
    }
  }
  root.style.setProperty("zoom", String(factor));
}

export interface AppearanceHandle {
  stop(): void;
  /** Wendet den aktuellen Zustand sofort an (Tests). */
  apply(): void;
}

export function startAppearanceSync(opts: AppearanceSyncOptions = {}): AppearanceHandle {
  const root = opts.root ?? document.documentElement;
  const platform = opts.platform ?? currentPlatform();
  const setZoom = opts.setZoom ?? defaultSetZoom;
  const mql = typeof window !== "undefined" && typeof window.matchMedia === "function" ? window.matchMedia("(prefers-color-scheme: dark)") : null;
  let lastZoom: number | null = null;
  let lastLang: string | null = null;
  let lastInterval: number | null = null;

  const apply = () => {
    const s = usePrefs.getState();
    const a: AppearanceState = s.appearance;
    applyAppearance(root, a, { platform, systemDark: mql ? mql.matches : systemPrefersDark() });
    if (lastZoom !== a.zoomPct) {
      lastZoom = a.zoomPct;
      void setZoom(a.zoomPct / 100, root);
    }
    if (lastLang !== s.language) {
      lastLang = s.language;
      void setLanguage(s.language);
    }
    if (opts.reportLiveInterval && s.loaded && lastInterval !== s.liveIntervalMs) {
      lastInterval = s.liveIntervalMs;
      void events.setInterval({ ms: s.liveIntervalMs }).catch(() => undefined);
    }
  };

  apply();
  const unsub = usePrefs.subscribe(apply);
  // „System“ folgt einem Wechsel des Betriebssystem-Themes zur Laufzeit sofort (≤ 1 s)
  mql?.addEventListener("change", apply);
  return {
    apply,
    stop() {
      unsub();
      mql?.removeEventListener("change", apply);
    },
  };
}
