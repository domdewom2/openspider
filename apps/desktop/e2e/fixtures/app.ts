// Fixtures für E2E-A: Transport (Bridge oder Mock), Seiten öffnen, Fensterlabel.
import { test as base, expect, type Page } from "@playwright/test";
import { startBridge, type Bridge } from "../support/bridge";

export type TransportMode = "bridge" | "mock";

/** `OS_E2E_TRANSPORT`: bridge (Pflicht), mock (ohne Kern) oder auto (Bridge, wenn sie startet, sonst Mock). */
const REQUESTED = (process.env.OS_E2E_TRANSPORT ?? "auto") as "bridge" | "mock" | "auto";

export interface AppFixture {
  mode: TransportMode;
  /** Öffnet ein Fenster der App: `main`, `settings` oder die Harness-Szene. */
  open(page: Page, target: "main" | "settings" | { harness: string; params?: Record<string, string> }, params?: Record<string, string>): Promise<void>;
}

interface Worker {
  bridge: Bridge | null;
  mode: TransportMode;
}

export const test = base.extend<{ app: AppFixture }, { workerBridge: Worker }>({
  workerBridge: [
    async ({}, use) => {
      let bridge: Bridge | null = null;
      let mode: TransportMode = "mock";
      if (REQUESTED !== "mock") {
        bridge = await startBridge();
        if (bridge) mode = "bridge";
        else if (REQUESTED === "bridge") throw new Error("OS_E2E_TRANSPORT=bridge, aber os-app-bridge startet nicht (cargo build -p os-app --features e2e-bridge --bin os-app-bridge)");
      }
      console.log(`[e2e] Transport: ${mode}${mode === "mock" ? " (os-app-bridge nicht verfügbar)" : ""}`);
      await use({ bridge, mode });
      await bridge?.stop();
    },
    { scope: "worker", auto: true },
  ],

  app: async ({ workerBridge }, use, testInfo) => {
    const { bridge, mode } = workerBridge;
    testInfo.annotations.push({ type: "transport", description: mode });
    const query = (extra: Record<string, string> = {}) => {
      const p = new URLSearchParams(extra);
      if (bridge) p.set("bridge", bridge.url);
      else p.set("transport", "mock");
      return p.toString();
    };
    await use({
      mode,
      async open(page, target, params) {
        if (target === "main") await page.goto(`/?${query(params)}`);
        else if (target === "settings") await page.goto(`/settings.html?${query(params)}`);
        else await page.goto(`/?${query({ harness: target.harness, ...target.params })}`);
      },
    });
  },
});

export { expect };

/** Hintergrundfarbe des Dokuments (für „Umschalten wirkt auf alle Fenster“). */
export async function backgroundOf(page: Page): Promise<string> {
  return page.evaluate(() => getComputedStyle(document.body).backgroundColor);
}
