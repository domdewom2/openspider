import { test, expect } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";

// spec: UI-120
// spec: UI-121
test("a delayed settings read cannot undo a live theme and language change", async ({ page }) => {
  const site = await startSite(); const bridge = await startAlphaBridge();
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    const result = await page.evaluate(async () => {
      const { settings } = await import(/* @vite-ignore */ String("/src/ipc/settings.ts"));
      const { usePrefs } = await import(/* @vite-ignore */ String("/src/state/prefs.ts"));
      const original = settings.get;
      let first = true; let captured!: () => void;
      const ready = new Promise<void>(resolve => { captured = resolve; });
      settings.get = async (signal?: AbortSignal) => {
        const dto = await original(signal);
        if (first) { first = false; captured(); await new Promise(r => setTimeout(r, 50)); }
        return dto;
      };
      try {
        const loading = usePrefs.getState().load();
        await ready;
        await usePrefs.getState().patch({ appearance: { theme: "dark" }, general: { language: "de" } });
        await loading;
        return { language: usePrefs.getState().language, theme: usePrefs.getState().appearance.theme };
      } finally { settings.get = original; }
    });
    expect(result).toEqual({ language: "de", theme: "dark" });
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  } finally { await bridge.stop(); await site.stop(); }
});

// spec: UI-120
// spec: UI-121
test("theme and language reach both open windows within 200 ms", async ({ page, context }) => {
  const bridge = await startAlphaBridge(); const settingsPage = await context.newPage();
  try {
    const query = new URLSearchParams({ bridge: bridge.url });
    await page.goto(`/?${query}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await settingsPage.goto(`/settings.html?${query}`);
    await expect(settingsPage.locator("[data-settings-section]").first()).toBeVisible();
    for (const [language, theme] of [["de", "dark"], ["en", "light"]] as const) {
      for (const window of [page, settingsPage]) await window.evaluate(({ language, theme }) => {
        const root = document.documentElement;
        const sample = { at: 0 };
        const observer = new MutationObserver(() => {
          if (root.lang === language && root.dataset.theme === theme) { sample.at = performance.timeOrigin + performance.now(); observer.disconnect(); }
        });
        observer.observe(root, { attributes: true });
        Object.assign(globalThis, { __n03PreferenceSample: sample });
      }, { language, theme });
      const started = await settingsPage.evaluate(async ({ language, theme }) => {
        const { usePrefs } = await import(/* @vite-ignore */ String("/src/state/prefs.ts"));
        const started = performance.timeOrigin + performance.now();
        await usePrefs.getState().patch({ general: { language }, appearance: { theme } });
        return started;
      }, { language, theme });
      for (const window of [page, settingsPage]) {
        await expect(window.locator("html")).toHaveAttribute("lang", language);
        await expect(window.locator("html")).toHaveAttribute("data-theme", theme);
        const changedAt = await window.evaluate(() => (globalThis as unknown as { __n03PreferenceSample: { at: number } }).__n03PreferenceSample.at);
        expect(changedAt - started).toBeGreaterThanOrEqual(0);
        expect(changedAt - started).toBeLessThanOrEqual(200);
      }
      await expect(page.getByRole("combobox", { name: language === "de" ? "Bereich" : "Scope", exact: true })).toBeVisible();
    }
  } finally { await settingsPage.close(); await bridge.stop(); }
});
