import { test, expect } from "@playwright/test";

// spec: UI-121 [P1]
test("es/fr/it vollständig, wählbar und per Systemsprache erkannt", async ({ page }) => {
  test.fixme(true, "M3: vollständige es/fr/it-Kataloge nach K2-79 zurückgestellt; M2 prüft das Community-Kriterium");
  await page.goto("/?harness=settings&lang=en");
  await expect(page.locator("[data-harness-ready]")).toBeVisible();
  const languages = await page.evaluate(async () => {
    const path = "/src/i18n/index.ts";
    const { i18n } = await import(/* @vite-ignore */ path);
    return ["es", "fr", "it"].map((lang) => ({ lang, present: i18n.languages().includes(lang), percent: i18n.completeness(lang), resolved: i18n.resolve("system", [`${lang}-${lang.toUpperCase()}`]) }));
  });
  expect(languages).toEqual(["es", "fr", "it"].map((lang) => ({ lang, present: true, percent: 100, resolved: lang })));
});

// spec: UI-121 [P1]
test("Community-Ressource ohne Codeänderung: Auswahl, Prozent und englischer Fallback", async ({ page }) => {
  await page.goto("/?harness=settings&lang=en");
  await expect(page.locator("[data-harness-ready]")).toBeVisible();
  const result = await page.evaluate(async () => {
    const corePath = "/src/i18n/core.ts", optionsPath = "/src/i18n/languages.ts";
    const { createI18n } = await import(/* @vite-ignore */ corePath);
    const { languageOptions } = await import(/* @vite-ignore */ optionsPath);
    const fixture = [{ path: "en.ftl", lang: "en", feature: "u32", source: "hello = Hello\nbye = Goodbye" }, { path: "pl.ftl", lang: "pl", feature: "u32", source: "hello = Cześć" }];
    const catalog = createI18n(fixture);
    return { options: languageOptions(catalog), system: catalog.resolve("system", ["pl-PL"]), fallback: catalog.translate("pl", "bye") };
  });
  expect(result).toEqual({ options: [{ value: "en", label: "English" }, { value: "pl", label: "Polski (50%)" }], system: "pl", fallback: "Goodbye" });
});

// spec: UI-121 [P1]
test("Community-Datei erscheint in der echten Sprachauswahl mit Prozent und wirkt ohne Neustart", async ({ page }) => {
  await page.goto("/?harness=community&lang=en");
  await expect(page.locator("[data-harness-ready]")).toHaveAttribute("data-harness-ready", "community");
  await page.getByRole("button", { name: /Language$/ }).click();
  const polish = page.getByRole("option", { name: /^Polski \(\d+%\)$/ });
  await expect(polish).toBeVisible();
  await polish.click();
  await expect(page.locator("html")).toHaveAttribute("lang", "pl");
  await expect(page.getByRole("button", { name: /Język$/ })).toBeVisible();
  // Missing community labels use the English catalog in the same window.
  await expect(page.getByRole("switch", { name: /notification/i })).toBeVisible();
});
