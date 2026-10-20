import AxeBuilder from "@axe-core/playwright";
import { expect, test, backgroundOf } from "../fixtures/app";

// Diese Specs ändern persistierte Einstellungen bzw. setzen den Ausgangszustand voraus.
test.use({ freshState: true });

const LIGHT_BG = "rgb(238, 240, 243)";
const DARK_BG = "rgb(19, 20, 22)";

test.describe("Einstellungsfenster und Erscheinungsbild", () => {
  // spec: UI-120
  test("Umschalten des Themes wirkt ohne Neustart auf alle offenen Fenster", async ({ page, context, app }) => {
    await app.open(page, "main");
    const settings = await context.newPage();
    await app.open(settings, "settings", { "": "" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    expect(await backgroundOf(page)).toBe(LIGHT_BG);

    await settings.getByRole("tab", { name: /Appearance|Oberfläche/ }).click();
    await settings.getByRole("radiogroup", { name: "Theme" }).getByText(/^(Dark|Dunkel)$/).click();

    await expect(settings.locator("html")).toHaveAttribute("data-theme", "dark");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    expect(await backgroundOf(page)).toBe(DARK_BG);
    expect(await backgroundOf(settings)).toBe(DARK_BG);
  });

  // spec: UI-120
  test("System folgt dem Betriebssystem-Theme zur Laufzeit", async ({ page, app }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await app.open(page, "main");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await expect(page.locator("html")).toHaveAttribute("data-theme-setting", "system");
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark", { timeout: 1000 });
  });

  // spec: UI-120
  test("Akzent #FFFF66 löst die Kontrastwarnung aus, Standard-Dichte ist kompakt (22 px)", async ({ page, app }) => {
    await app.open(page, "settings", { "": "" });
    await page.getByRole("tab", { name: /Appearance|Oberfläche/ }).click();
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact");
    expect(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--os-row-height"))).toBe("22px");
    await page.getByRole("textbox", { name: /Custom color|Eigene Farbe/ }).fill("#FFFF66");
    await expect(page.getByTestId("accent-warning")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("data-accent-warn", "on");
  });

  // spec: UI-120
  test("Zoom skaliert ohne Neustart (CSS-Zoom außerhalb von Tauri)", async ({ page, app }) => {
    await app.open(page, "settings", { "": "" });
    await page.getByRole("tab", { name: /Appearance|Oberfläche/ }).click();
    await page.getByRole("button", { name: /Zoom/ }).click();
    await page.getByRole("option", { name: "125 %" }).click();
    await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue("zoom"))).toBe("1.25");
  });

  // spec: UI-016
  test("ein Anker öffnet den Abschnitt und fokussiert das Ziel-Steuerelement", async ({ page, app }) => {
    await app.open(page, "settings", { "": "" });
    await page.evaluate(() => (location.hash = "#appearance.zoom"));
    await expect(page.getByRole("tab", { name: /Appearance|Oberfläche/ })).toHaveAttribute("aria-selected", "true");
    await expect.poll(() => page.evaluate(() => document.activeElement?.closest("[data-setting-anchor]")?.getAttribute("data-setting-anchor"))).toBe("appearance.zoom");
  });

  // spec: UI-126
  test("Hauptfenster und Einstellungen haben keine kritischen oder schwerwiegenden axe-Verstöße", async ({ page, app }) => {
    for (const target of ["main", "settings"] as const) {
      await app.open(page, target, target === "settings" ? { "": "" } : undefined);
      await expect(page.locator("html")).toHaveAttribute("data-theme", /light|dark/);
      const results = await new AxeBuilder({ page }).analyze();
      const serious = results.violations.filter((v) => v.impact === "critical" || v.impact === "serious");
      expect(serious.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`), target).toEqual([]);
    }
  });
});
