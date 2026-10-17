import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "../fixtures/app";

const THEMES = ["light", "dark"] as const;

test.describe("Harness und Design-System", () => {
  // spec: UI-016
  test("alle elf Leerzustände sind mit data-empty-state auffindbar (de und en)", async ({ page, app }) => {
    for (const lang of ["en", "de"]) {
      await app.open(page, { harness: "empty", params: { lang } });
      await expect(page.locator("[data-harness-ready]")).toBeVisible();
      const codes = await page.locator("[data-empty-state]").evaluateAll((els) => els.map((e) => e.getAttribute("data-empty-state")));
      expect(codes).toEqual([
        "NO_SELECTION",
        "MODE_UNSUPPORTED",
        "NEEDS_JS_RENDERING",
        "FEATURE_DISABLED(cookies)",
        "HTML_NOT_STORED",
        "RENDERED_HTML_NOT_STORED",
        "API_NOT_CONNECTED(gsc)",
        "NEEDS_CRAWL_ANALYSIS(canonicals)",
        "NO_DATA_YET",
        "NO_MATCH",
        "NOT_APPLICABLE",
      ]);
      const text = await page.locator("[data-harness-ready]").innerText();
      expect(text).not.toMatch(/empty-[a-z]/);
    }
  });

  for (const theme of THEMES) {
    // spec: UI-120
    test(`Primitive und Leerzustände ohne kritische axe-Verstöße im Theme ${theme}`, async ({ page, app }) => {
      for (const scene of ["primitives", "empty", "settings"]) {
        await app.open(page, { harness: scene, params: { theme } });
        await expect(page.locator("[data-harness-ready]")).toBeVisible();
        await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
        const results = await new AxeBuilder({ page }).analyze();
        const bad = results.violations.filter((v) => v.impact === "critical" || v.impact === "serious");
        expect(bad.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`), `${scene}/${theme}`).toEqual([]);
      }
    });
  }

  // spec: UI-113
  test("Kürzelanzeige je Plattform: macOS-Symbole, Windows deutsch Strg+…", async ({ page, app }) => {
    await app.open(page, { harness: "shortcuts", params: { platform: "mac", lang: "en" } });
    await expect(page.getByRole("row", { name: /Pause crawl/ })).toContainText("⌘P");
    await app.open(page, { harness: "shortcuts", params: { platform: "win", lang: "de" } });
    await expect(page.getByRole("row", { name: /Crawl pausieren/ })).toContainText("Strg+P");
    await expect(page.locator("tr")).toHaveCount(await page.locator("table").getAttribute("data-count").then(Number));
  });

  // spec: UI-113
  test("Tastaturkürzel lösen Aktionsereignisse aus", async ({ page, app }) => {
    await app.open(page, "main");
    await page.evaluate(() => {
      (window as unknown as { __actions: string[] }).__actions = [];
      document.addEventListener("os:action", (e) => (window as unknown as { __actions: string[] }).__actions.push((e as CustomEvent).detail.id));
    });
    // Mod richtet sich nach der Plattform-Erkennung des Browsers (macOS: Cmd, sonst Strg)
    const isMac = (await page.locator("html").getAttribute("data-platform")) === "mac";
    const key = isMac ? "Meta" : "Control";
    await page.keyboard.press(`${key}+P`);
    await page.keyboard.press(`${key}+Shift+A`);
    expect(await page.evaluate(() => (window as unknown as { __actions: string[] }).__actions)).toEqual(["crawl.pause", "analysis.start"]);
  });

  // spec: UI-113
  test("Mod+, öffnet das Einstellungsfenster als eigenes Fenster (nicht modal)", async ({ page, app }) => {
    await app.open(page, "main");
    const isMac = (await page.locator("html").getAttribute("data-platform")) === "mac";
    const [popup] = await Promise.all([page.waitForEvent("popup"), page.keyboard.press(`${isMac ? "Meta" : "Control"}+,`)]);
    await popup.waitForLoadState();
    await expect(popup.getByRole("tablist", { name: /Settings sections|Einstellungsbereiche/ })).toBeVisible();
    await expect(page.getByRole("main")).toBeVisible(); // Hauptfenster bleibt bedienbar
  });
});
