// Alpha-Kriterium 7 (docs/plan/M2.md 4.3): keine Konsolenfehler und axe ohne „critical“ auf dem Hauptfenster. Die Fixture `alpha` bricht
// jeden Test dieser Suite ab, sobald das Hauptfenster oder ein zweites Fenster `console.error` oder eine ungefangene Ausnahme meldet;
// dieser Test geht zusätzlich einen langen Weg durch die Oberfläche und prüft axe in drei Zuständen.
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { expect, test } from "./fixture";

async function critical(page: Page, label: string): Promise<string[]> {
  const result = await new AxeBuilder({ page }).analyze();
  const serious = result.violations.filter((v) => v.impact === "serious" || v.impact === "moderate").map((v) => `${v.impact} ${v.id} (${v.nodes.length})`);
  test.info().annotations.push({ type: "axe", description: `${label}: ${serious.join(", ") || "keine weiteren Verstöße"}` });
  return result.violations.filter((v) => v.impact === "critical").map((v) => `${v.id}: ${v.help} (${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join("; ")})`);
}

// spec: UI-013
test("a tour through the main window produces no console errors and axe finds nothing critical", async ({ alpha }) => {
  test.setTimeout(120_000);
  const { page } = alpha;
  const failed: string[] = [];
  page.on("requestfailed", (r) => failed.push(`${r.method()} ${r.url()}`));
  await alpha.open();
  expect(await critical(page, "leeres Hauptfenster")).toEqual([]);

  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await alpha.openTable("Response Codes");
  expect(await critical(page, "nach dem Crawl, Tabelle und Seitenleiste")).toEqual([]);

  // Tabelle: Filter, Suche, Tabwechsel
  await alpha.table().getByRole("button", { name: /Filter/, exact: true }).click();
  await page.getByRole("option", { name: /^Success \(2XX\)\s*25$/ }).click();
  await expect(alpha.table()).toContainText("Showing: 25 of 27");
  const search = page.getByRole("searchbox", { name: "Search Address", exact: true });
  await search.fill("page-2");
  await search.press("Enter");
  await expect(alpha.table()).toContainText(/Search active/);
  await search.press("Escape");
  for (const tab of ["External", "Page Titles", "Images"]) await alpha.selectTab(tab);

  // Seitenleiste und Detailfenster: jeder Reiter einmal
  // M2-U34 (K2-72): Reiter, die nicht in die Breite passen, stehen im Überlaufmenü der Leiste (UI-004, UI-030, UI-060).
  for (const name of ["Issues", "Site Structure", "Segments", "Response Times", "API", "Spelling & Grammar", "Overview"]) {
    const tab = page.getByRole("tablist", { name: "Crawl summary" }).getByRole("tab", { name, exact: true });
    if (await tab.count()) { await tab.focus(); await tab.press("Enter"); }
    else { await page.getByRole("button", { name: "More sidebar tabs" }).press("Enter"); await page.getByRole("menuitem", { name, exact: true }).press("Enter"); }
    await expect(tab).toHaveAttribute("aria-selected", "true");
  }
  for (const name of ["Inlinks", "Outlinks", "Image Details", "Resources", "SERP Snippet", "HTTP Headers", "Cookies", "URL Details"]) {
    const tab = page.getByRole("region", { name: "Details" }).getByRole("tab", { name, exact: true });
    if (await tab.count()) await tab.click();
    else { await page.getByRole("button", { name: "More detail tabs" }).click(); await page.getByRole("menuitem", { name, exact: true }).click(); }
  }

  // Dialoge: Spalten, Crawl-Verwaltung, Tastaturkürzel, Export (abgebrochen)
  await alpha.table().getByRole("button", { name: "Configure columns…", exact: true }).click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  for (const action of ["crawls.manage", "help.shortcuts", "export.multi"]) {
    await page.evaluate(async (id) => {
      const path = "/src/ui/actions.ts";
      const { runAction } = await import(/* @vite-ignore */ path);
      runAction(id, { source: "menu" });
    }, action);
    await expect(page.getByRole("dialog")).toBeVisible();
    if (action === "crawls.manage") expect(await critical(page, "Crawl-Verwaltung")).toEqual([]);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  }

  // Einstellungsfenster und Sprachwechsel (beide Fenster werden auf Konsolenfehler beobachtet)
  const settings = await alpha.openSettings();
  expect(await critical(settings, "Einstellungsfenster")).toEqual([]);
  await settings.getByRole("button", { name: /Language/ }).click();
  await settings.getByRole("option", { name: "Deutsch" }).click();
  await expect(page.getByTestId("crawl-phase")).toHaveText("Fertig");
  expect(await critical(page, "Hauptfenster deutsch")).toEqual([]);
  expect(failed, "fehlgeschlagene Netzwerkanfragen").toEqual([]);
});
