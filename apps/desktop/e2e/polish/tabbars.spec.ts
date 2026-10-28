// Abnahme M2-U34 (1)–(3): drei Tab-Leisten ohne gekappte Beschriftungen, alle Tabs erreichbar, Konfiguration einheitlich
// (Befunde A02-01, A02-02, A02-03, A02-06). Fenstergrößen nach UI-001, Sprache de/en, Theme hell/dunkel.
import type { Page } from "@playwright/test";
import { expect, test } from "../alpha/fixture";
import { CONFIGURE, RESET, SIZES, contextMenu, crawledWithSelection, measureBar, overflowMenu, showPanes, type Bar } from "./support";

const BARS: Bar[] = ["top", "detail", "sidebar"];
/** Anzahl der Tabs je Leiste: Katalog (oben, ohne Vergleichsmodus), DETAIL_TABS (unten), Seitenleiste ohne Embeddings (rechts). */
async function expectedCounts(page: Page): Promise<Record<Bar, number>> {
  const top = await page.evaluate(async () => {
    const path = "/src/ipc/catalog.ts";
    const { catalog } = await import(/* @vite-ignore */ path);
    return (await catalog.tabs()).length as number;
  });
  return { top, detail: 17, sidebar: 7 };
}

async function checkAllBars(page: Page, label: string, counts: Record<Bar, number>): Promise<void> {
  for (const bar of BARS) {
    const { tabs, hasTrigger } = await measureBar(page, bar);
    expect(hasTrigger, `${label} ${bar}: Überlaufknopf`).toBe(true);
    for (const tab of tabs) {
      // (1) nie vertikal gekappt; horizontal nur mit „…“, Tooltip und zugänglichem Namen = voller Text
      expect(tab.verticalCut, `${label} ${bar} „${tab.text}“ vertikal gekappt`).toBe(false);
      if (tab.horizontalCut) expect(tab.title, `${label} ${bar} „${tab.text}“ gekürzt ohne Tooltip`).toBe(tab.text);
      expect(tab.overlapsTrigger, `${label} ${bar} „${tab.text}“ liegt unter dem Überlaufknopf`).toBe(false);
      expect(tab.outsideList, `${label} ${bar} „${tab.text}“ ragt aus der Leiste`).toBe(false);
    }
    // (2) sichtbare Tabs ∪ Menüeinträge = alle Tabs der Leiste
    const menu = await overflowMenu(page, bar);
    expect(menu.checkboxes, `${label} ${bar}: alle Tabs im Menü`).toHaveLength(counts[bar]);
    const shown = menu.checkboxes.filter((c) => c.checked).map((c) => c.name);
    const reachable = new Set([...tabs.map((t) => t.text), ...menu.items]);
    for (const name of shown) expect(reachable.has(name), `${label} ${bar}: „${name}“ weder sichtbar noch im Überlaufmenü`).toBe(true);
    // (3) Konfiguration über dasselbe Bedienelement
    expect(menu.items, `${label} ${bar}: Überlaufmenü`).toEqual(expect.arrayContaining([expect.stringMatching(CONFIGURE), expect.stringMatching(RESET)]));
  }
}

for (const locale of ["de-DE", "en-US"] as const) {
  for (const colorScheme of ["light", "dark"] as const) {
    test.describe(`${locale} ${colorScheme}`, () => {
      test.use({ locale, colorScheme });
      // spec: UI-004 [P1]
      // spec: UI-030
      // spec: UI-060
      // spec: UI-001
      test("tab labels are never cut, every tab is reachable and the overflow menu configures each bar", async ({ alpha }) => {
        test.setTimeout(120_000);
        const { page } = alpha;
        await crawledWithSelection(alpha);
        const counts = await expectedCounts(page);
        for (const size of SIZES) {
          await page.setViewportSize(size);
          for (const sidebar of ["default", 200] as const) {
            await showPanes(page, sidebar);
            await checkAllBars(page, `${size.width}×${size.height} Seitenleiste ${sidebar}`, counts);
          }
        }
      });
    });
  }
}

test.describe("deviceScaleFactor (Windows-Skalierung)", () => {
  for (const deviceScaleFactor of [1.25, 1.5, 2]) {
    test.describe(`${deviceScaleFactor}`, () => {
      test.use({ locale: "de-DE", colorScheme: "dark", deviceScaleFactor });
      // spec: UI-001
      // spec: UI-004 [P1]
      test(`labels stay whole at scale ${deviceScaleFactor}`, async ({ alpha, browserName }) => {
        test.skip(browserName !== "chromium", "Skalierungsfaktoren prüft die Abnahme mit Chromium (WebView2)");
        test.setTimeout(120_000);
        const { page } = alpha;
        await crawledWithSelection(alpha);
        const counts = await expectedCounts(page);
        for (const size of SIZES) {
          await page.setViewportSize(size);
          await showPanes(page, 200);
          await checkAllBars(page, `${size.width}×${size.height} @${deviceScaleFactor}`, counts);
        }
      });
    });
  }
});

test.describe("configuration controls (A02-06)", () => {
  test.use({ locale: "de-DE" });
  // spec: UI-112
  // spec: UI-004 [P1]
  test("all three bars offer configure and reset in the overflow menu and in the tab context menu; no extra controls", async ({ alpha }) => {
    const { page } = alpha;
    await crawledWithSelection(alpha);
    await page.setViewportSize({ width: 1440, height: 900 });
    await showPanes(page, "default");
    for (const bar of BARS) {
      const menu = await overflowMenu(page, bar);
      expect(menu.items).toEqual(expect.arrayContaining(["Tabs konfigurieren…", "Tabs zurücksetzen"]));
      const context = await contextMenu(page, bar);
      expect(context).toEqual(expect.arrayContaining(["Tab schließen", "Andere Tabs schließen", "Alle Tabs schließen", "Tabs konfigurieren…", "Tabs zurücksetzen"]));
    }
    await expect(page.getByText("Tabs anpassen")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Detail-Tabs konfigurieren" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Tabs zurücksetzen" })).toHaveCount(0);
  });

  // spec: UI-112
  test("„Alle Tabs schließen“ leaves the mandatory tab of each bar; closed tabs come back through the overflow menu", async ({ alpha }) => {
    const { page } = alpha;
    await crawledWithSelection(alpha);
    await page.setViewportSize({ width: 1440, height: 900 });
    await showPanes(page, "default");
    const mandatory: Record<Bar, string> = { top: "Intern", detail: "URL-Details", sidebar: "Übersicht" };
    for (const bar of BARS) {
      const list = page.getByRole("tablist", { name: { top: "Daten-Tabs", detail: "Detail-Tabs", sidebar: "Crawl-Zusammenfassung" }[bar] });
      await list.getByRole("tab").first().click({ button: "right" });
      await page.getByRole("menuitem", { name: "Alle Tabs schließen" }).click();
      await expect(list.getByRole("tab")).toHaveText([mandatory[bar]]);
      const menu = await overflowMenu(page, bar);
      expect(menu.checkboxes.filter((c) => c.checked).map((c) => c.name)).toEqual([mandatory[bar]]);
      const trigger = list.locator("xpath=ancestor::div[contains(@class,'os-tabbar')][1]").locator("[data-tabbar-overflow] button");
      await trigger.click();
      await page.getByRole("menuitem", { name: "Tabs zurücksetzen" }).click();
      await expect(list.getByRole("tab").nth(1)).toBeVisible();
    }
  });
});
