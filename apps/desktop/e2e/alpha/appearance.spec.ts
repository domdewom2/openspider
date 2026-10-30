// Alpha-Kriterium 5 (docs/plan/M2.md 4.3): Sprachwechsel de ↔ en und Theme-Wechsel ohne Neustart; Fenster 1 024 × 640 ohne Überlappung.
import type { Page } from "@playwright/test";
import { expect, test } from "./fixture";

const LIGHT_BG = "rgb(238, 240, 243)";
const DARK_BG = "rgb(19, 20, 22)";

// spec: UI-013
// spec: UI-007
test("switching the language between English and German changes the open main window without a restart", async ({ alpha }) => {
  // Crawl bis zum Abschluss plus zwei Sprachwechsel überschreiten auf windows-latest die Standardfrist von 30 s.
  test.setTimeout(90_000);
  const { page } = alpha;
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await alpha.openTable("Response Codes");
  await expect(alpha.table()).toContainText("Showing: 27 of 27");
  // Zahlenformat nach UI-Sprache (UI-007): Anteil der HTML-Seiten in der Overview der Seitenleiste
  const share = async () => page.getByRole("tabpanel", { name: /^(Overview|Übersicht)$/ }).getByRole("table", { name: /^(Internal|Intern)$/, exact: true }).getByRole("row", { name: /^HTML/ }).getByRole("cell").last().innerText();
  expect(await share()).toBe("92.0 %"); // en: Dezimalpunkt

  const settings = await alpha.openSettings();
  await settings.getByRole("button", { name: /Language/ }).click();
  await settings.getByRole("option", { name: "Deutsch" }).click();

  // Hauptfenster: Kopf-, Status- und Tabellenbeschriftungen deutsch, Daten unverändert
  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await expect(page.getByRole("navigation", { name: "Datenansichten" })).toBeVisible();
  await expect(page.getByTestId("crawl-phase")).toHaveText("Fertig");
  await expect(page.getByTestId("crawl-status")).toContainText("Modus: Spider");
  await expect(page.getByTestId("crawl-status")).toContainText("Erledigt 27 von 27 (100 %)");
  await expect(page.getByLabel("Start-URL")).toBeVisible();
  await expect(page.getByRole("button", { name: "Löschen", exact: true })).toBeEnabled();
  await expect(alpha.table()).toContainText("Angezeigt: 27 von 27");
  expect(await share()).toBe("92,0 %"); // de: Dezimalkomma
  await expect.poll(() => page.evaluate(() => (window as unknown as { __osMenu?: { model: { menus: { label: string }[] } } }).__osMenu?.model.menus.map((m) => m.label))).toEqual(expect.arrayContaining(["Datei", "Ansicht", "Hilfe"]));

  // zurück auf Englisch, ebenfalls ohne Neustart
  await settings.getByRole("button", { name: /Sprache/ }).click();
  await settings.getByRole("option", { name: "English" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.getByTestId("crawl-phase")).toHaveText("Completed");
  await expect(alpha.table()).toContainText("Showing: 27 of 27");
  expect(await share()).toBe("92.0 %");
});

test("switching the theme changes the main window without a restart and keeps the crawl data", async ({ alpha }) => {
  const { page } = alpha;
  await page.emulateMedia({ colorScheme: "light" });
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await alpha.openTable("Response Codes");
  await alpha.settledGrid();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(LIGHT_BG);

  const settings = await alpha.openSettings();
  await settings.getByRole("tab", { name: "Appearance" }).click();
  await settings.getByRole("radiogroup", { name: "Theme" }).getByText("Dark", { exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(DARK_BG);
  // Tabelle und Statusleiste bleiben bedienbar und zeigen dieselben Daten
  await expect(alpha.table()).toContainText("Showing: 27 of 27");
  await expect(page.getByTestId("crawl-phase")).toHaveText("Completed");

  await settings.getByRole("radiogroup", { name: "Theme" }).getByText("Light", { exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(LIGHT_BG);
});

interface LayoutReport {
  interactive: number;
  outside: string[];
  overlaps: string[];
  horizontalScroll: boolean;
}

/**
 * Layout-Test aus UI-001: Alle sichtbaren interaktiven Elemente liegen im Fenster und schneiden sich nicht. „Sichtbar“ heißt: nach
 * Beschneidung durch Vorfahren mit Überlauf (z. B. die scrollbare Tab-Leiste), nicht verborgen und nicht leer.
 */
async function layoutReport(page: Page): Promise<LayoutReport> {
  return page.evaluate(() => {
    const selector = 'button, a[href], input, select, textarea, summary, [role=tab], [role=button], [role=combobox], [role=checkbox], [role=menuitem], [role=separator][tabindex], [tabindex]:not([tabindex="-1"])';
    const visibleRect = (e: HTMLElement) => {
      const r = e.getBoundingClientRect();
      let x0 = r.left;
      let y0 = r.top;
      let x1 = r.right;
      let y1 = r.bottom;
      for (let p = e.parentElement; p; p = p.parentElement) {
        const cs = getComputedStyle(p);
        if (cs.overflowX !== "visible" || cs.overflowY !== "visible") {
          const c = p.getBoundingClientRect();
          x0 = Math.max(x0, c.left);
          y0 = Math.max(y0, c.top);
          x1 = Math.min(x1, c.right);
          y1 = Math.min(y1, c.bottom);
        }
      }
      return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    };
    const elements = [...document.querySelectorAll<HTMLElement>(selector)].filter((e) => {
      const cs = getComputedStyle(e);
      const r = visibleRect(e);
      return r.w > 0 && r.h > 0 && cs.visibility !== "hidden" && cs.display !== "none" && !(e instanceof HTMLInputElement && e.type === "hidden");
    });
    const name = (e: HTMLElement) => `${e.tagName.toLowerCase()}${e.getAttribute("role") ? `[${e.getAttribute("role")}]` : ""} „${(e.getAttribute("aria-label") ?? e.textContent ?? "").trim().slice(0, 30)}“`;
    const outside: string[] = [];
    const overlaps: string[] = [];
    for (const e of elements) {
      const b = visibleRect(e);
      if (b.x < -0.5 || b.y < -0.5 || b.x + b.w > innerWidth + 0.5 || b.y + b.h > innerHeight + 0.5) outside.push(name(e));
    }
    for (let i = 0; i < elements.length; i++) {
      for (let j = i + 1; j < elements.length; j++) {
        const a = elements[i]!;
        const c = elements[j]!;
        if (a.contains(c) || c.contains(a)) continue;
        const p = visibleRect(a);
        const q = visibleRect(c);
        const w = Math.min(p.x + p.w, q.x + q.w) - Math.max(p.x, q.x);
        const h = Math.min(p.y + p.h, q.y + q.h) - Math.max(p.y, q.y);
        if (w > 1 && h > 1) overlaps.push(`${name(a)} × ${name(c)} (${w.toFixed(0)}×${h.toFixed(0)} px)`);
      }
    }
    return { interactive: elements.length, outside, overlaps, horizontalScroll: document.documentElement.scrollWidth > innerWidth };
  });
}

// spec: UI-001
test("at 1024 x 640 the sidebar starts collapsed and every interactive element lies inside the window", async ({ alpha }) => {
  const { page } = alpha;
  await page.setViewportSize({ width: 1024, height: 640 });
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await alpha.openTable("Response Codes");
  await alpha.settledGrid();
  // Fensterbreite beim ersten Start < 1280 px: Seitenleiste eingeklappt
  await expect(page.getByRole("complementary", { name: "Sidebar" })).toBeHidden();
  const report = await layoutReport(page);
  expect(report.interactive).toBeGreaterThan(30);
  expect(report.outside).toEqual([]);
  expect(report.horizontalScroll).toBe(false);
  // das Hauptfenster füllt genau das Fenster, Kopf- und Statusleiste sind sichtbar
  await expect(page.getByRole("banner")).toBeVisible();
  await expect(page.getByRole("contentinfo")).toBeVisible();
});

// spec: UI-001
test("at 1024 x 640 no interactive elements overlap", async ({ alpha }) => {
  const { page } = alpha;
  await page.setViewportSize({ width: 1024, height: 640 });
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await alpha.openTable("Response Codes");
  await alpha.settledGrid();
  expect((await layoutReport(page)).overlaps).toEqual([]);
});

// spec: UI-001
test("with the sidebar open at 1280 x 800 the main regions do not overlap and nothing leaves the window", async ({ alpha }) => {
  const { page } = alpha;
  await page.setViewportSize({ width: 1280, height: 800 });
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await alpha.openTable("Response Codes");
  await alpha.settledGrid();
  const regions = await page.evaluate(() => {
    const box = (selector: string) => {
      const r = document.querySelector(selector)!.getBoundingClientRect();
      return { x: r.x, y: r.y, w: r.width, h: r.height };
    };
    return { header: box(".os-app-header"), tabs: box(".os-app-tabs"), main: box("main"), sidebar: box(".os-app-sidebar"), detail: box("#shell-detail"), status: box(".os-app-status") };
  });
  const bottom = (r: { y: number; h: number }) => r.y + r.h;
  expect(regions.tabs.y).toBeGreaterThanOrEqual(bottom(regions.header) - 1);
  expect(regions.main.y).toBeGreaterThanOrEqual(bottom(regions.tabs) - 1);
  expect(regions.detail.y).toBeGreaterThanOrEqual(bottom(regions.main) - 1);
  expect(regions.status.y).toBeGreaterThanOrEqual(bottom(regions.detail) - 1);
  expect(regions.sidebar.x).toBeGreaterThanOrEqual(regions.main.x + regions.main.w - 1);
  expect(regions.sidebar.x + regions.sidebar.w).toBeLessThanOrEqual(1281);
  expect(bottom(regions.status)).toBeLessThanOrEqual(801);
});
