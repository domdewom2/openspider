import { expect, test } from "../fixtures/app";

// Diese Specs ändern persistierte Einstellungen bzw. setzen den Ausgangszustand voraus.
test.use({ freshState: true });

test.describe("Sprachwechsel ohne Neustart", () => {
  // spec: UI-121
  test("Deutsch/Englisch wechselt in allen Fenstern und baut das Menü neu auf", async ({ page, context, app }) => {
    await app.open(page, "main");
    await expect(page.getByRole("navigation", { name: "Data views" })).toBeVisible();
    const before = await page.evaluate(() => window.__osMenu?.rebuilds ?? 0);

    const settings = await context.newPage();
    await app.open(settings, "settings", { "": "" });
    await settings.getByRole("button", { name: /Language/ }).click();
    await settings.getByRole("option", { name: "Deutsch" }).click();

    await expect(page.getByRole("navigation", { name: "Datenansichten" })).toBeVisible();
    await expect(settings.getByRole("tablist", { name: "Einstellungsbereiche" })).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await expect(settings.locator("html")).toHaveAttribute("lang", "de");

    // Menü: neu aufgebaut, Titel deutsch (Datei, Ansicht, Hilfe)
    await expect.poll(() => page.evaluate(() => window.__osMenu?.model.lang)).toBe("de");
    expect(await page.evaluate(() => window.__osMenu?.rebuilds ?? 0)).toBeGreaterThan(before);
    const titles = await page.evaluate(() => window.__osMenu?.model.menus.map((m) => m.label));
    expect(titles).toEqual(expect.arrayContaining(["Datei", "Ansicht", "Hilfe"]));

    // zurück auf Englisch
    await settings.getByRole("button", { name: /Sprache/ }).click();
    await settings.getByRole("option", { name: "English" }).click();
    await expect(page.getByRole("navigation", { name: "Data views" })).toBeVisible();
    await expect.poll(() => page.evaluate(() => window.__osMenu?.model.menus.map((m) => m.label))).toEqual(expect.arrayContaining(["File", "View", "Help"]));
  });

  // spec: UI-121
  test("die gewählte Sprache bleibt nach einem Neuladen erhalten", async ({ page, app }) => {
    await app.open(page, "settings", { "": "" });
    await page.getByRole("button", { name: /Language/ }).click();
    await page.getByRole("option", { name: "Deutsch" }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await page.reload();
    await expect(page.getByRole("tablist", { name: "Einstellungsbereiche" })).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
  });

  // spec: OUT-271
  test("Systemsprache de-AT ergibt Deutsch, Polnisch ergibt Englisch", async ({ browser, app }) => {
    for (const [locale, expected] of [["de-AT", "de"], ["pl-PL", "en"]] as const) {
      const context = await browser.newContext({ locale });
      const page = await context.newPage();
      await app.open(page, "main");
      await expect(page.locator("html")).toHaveAttribute("lang", expected);
      await context.close();
    }
  });
});
