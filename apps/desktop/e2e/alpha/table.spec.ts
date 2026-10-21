// Alpha-Kriterium 2 (docs/plan/M2.md 4.3): Haupttabelle zeigt alle URLs; Tabwechsel, Filter-Dropdown mit Zählern, Sortierung (Pfeil,
// Gleichstand F-3), einfache Suche, Zähler „N von M“; Live-Einsortierung ohne Springen der Auswahl. Echte Bridge, echter Kern.
import type { Page } from "@playwright/test";
import { expect, test, type Alpha } from "./fixture";

const TALL = { width: 1280, height: 1500 };

/** Mitte eines Spaltenkopfs in Canvas-Koordinaten, berechnet aus der Spaltenverwaltung der Oberfläche (feste Breiten). */
async function headerX(page: Page, tab: string, key: string): Promise<number> {
  const columns = await page.evaluate(async (tab) => {
    const path = "/src/state/columns.ts";
    const { useColumns } = await import(/* @vite-ignore */ path);
    return (useColumns.getState().byTab[tab] as { key: string; width: number; visible: boolean }[]).filter((c) => c.visible).map((c) => [c.key, c.width] as const);
  }, tab);
  let x = 0;
  for (const [name, width] of columns) {
    if (name === key) return x + width / 2;
    x += width;
  }
  throw new Error(`Spalte ${key} ist im Tab ${tab} nicht sichtbar`);
}

async function sortBy(alpha: Alpha, tab: string, key: string): Promise<void> {
  await alpha.page.locator(".os-data-grid").click({ position: { x: await headerX(alpha.page, tab, key), y: 12 } });
}

async function crawled(alpha: Alpha, tab: string, viewport?: { width: number; height: number }): Promise<void> {
  if (viewport) await alpha.page.setViewportSize(viewport);
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await alpha.openTable(tab);
}

const addresses = (rows: { cells: string[] }[]) => rows.map((r) => r.cells[0]!);

// spec: UI-004
// spec: UI-007
test("the Internal tab lists all 25 internal URLs with the default columns after a crawl", async ({ alpha }) => {
  test.fixme(true, "Befund A01-01 (docs/plan/M2-acceptance/alpha/befunde.md): view.open scheitert im Kern für Spalten mit benannten Parametern („Wrong number of parameters passed to query“); die Tabelle zeigt „Store“ statt der 25 Zeilen");
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  await expect(alpha.page.getByRole("tab", { name: "Internal", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(alpha.table()).toContainText("Showing: 25 of 25");
  const rows = (await alpha.settledGrid()).rows;
  expect(rows.length).toBeGreaterThan(0);
  expect(addresses(rows)[0]).toBe(alpha.startUrl);
});

// spec: UI-004
test("tab switching shows each tab's table and keeps the last filter chosen per tab", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha, "External");
  await expect(alpha.table()).toContainText("Showing: 2 of 2");
  const external = addresses((await alpha.settledGrid()).rows);
  expect(external).toHaveLength(2);
  expect(external.every((a) => a.startsWith("http://localhost:"))).toBe(true);

  await alpha.openTable("Response Codes");
  await expect(alpha.table()).toContainText("Showing: 27 of 27");
  // Filter-Dropdown mit Zählern je Eintrag
  await alpha.table().getByRole("button", { name: /Filter/, exact: true }).click();
  const option = (label: RegExp) => page.getByRole("option", { name: label });
  await expect(option(/^Success \(2XX\)\s*25$/)).toBeVisible();
  await expect(option(/^Redirection \(3XX\)\s*1$/)).toBeVisible();
  await expect(option(/^Client Error \(4XX\)\s*1$/)).toBeVisible();
  await expect(option(/^Server Error \(5XX\)\s*0$/)).toBeVisible();
  await option(/^Client Error \(4XX\)\s*1$/).click();
  await expect(alpha.table()).toContainText("Showing: 1 of 27");
  const failing = (await alpha.settledGrid()).rows;
  expect(failing).toHaveLength(1);
  expect(failing[0]!.cells[0]).toBe(`${alpha.site.base}/missing`);
  expect(failing[0]!.cells[2]).toBe("404");

  // Tabwechsel und zurück: der zuletzt gewählte Filter bleibt erhalten
  await alpha.openTable("External");
  await expect(alpha.table()).toContainText("Showing: 2 of 2");
  await alpha.openTable("Response Codes");
  await expect(alpha.table()).toContainText("Showing: 1 of 27");
  await expect(alpha.table().getByRole("combobox", { name: "Filter" })).toHaveValue(/Client Error \(4XX\)/);
});

// Alpha 2, Sortierung (UI-010, Gleichstand F-3): gehört zu einem fremden Paket und trägt deshalb kein Spec-Tag.
test("sorting cycles ascending, descending and off, and ties keep the F-3 order (Address ascending)", async ({ alpha }) => {
  await crawled(alpha, "Page Titles", TALL);
  await expect(alpha.table()).toContainText("Showing: 22 of 22");
  const original = (await alpha.settledGrid()).rows;
  expect(original).toHaveLength(22);

  const byte = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
  const length = (r: { cells: string[] }) => Number(r.cells[3]);
  const headerText = async () => (await alpha.gridWindow()).headers.join("|");

  const lengths = original.map(length);
  const sortedAsc = [...lengths].sort((a, b) => a - b);
  const sortedDesc = [...lengths].sort((a, b) => b - a);
  const tiesInAddressOrder = (rows: { cells: string[] }[]) => {
    for (const len of new Set(rows.map(length))) {
      const group = rows.filter((r) => length(r) === len).map((r) => r.cells[0]!);
      expect(group, `Gleichstand bei Länge ${len}`).toEqual([...group].sort(byte));
    }
  };

  // 1. Klick: aufsteigend, numerisch (nicht lexikografisch), Gleichstand nach Address (Bytevergleich)
  await sortBy(alpha, "Page Titles", "Title 1 Length");
  await expect.poll(headerText).toContain("Title 1 Length ↑");
  await expect.poll(async () => (await alpha.settledGrid()).rows.map(length)).toEqual(sortedAsc);
  const asc = (await alpha.settledGrid()).rows;
  expect(new Set(lengths).size).toBeLessThan(lengths.length); // es gibt Gleichstände
  tiesInAddressOrder(asc);
  expect(asc.slice(0, 2).map((r) => r.cells[2])).toEqual(["Shared title", "Shared title"]);

  // 2. Klick: absteigend; der Gleichstand bleibt nach Address aufsteigend
  await sortBy(alpha, "Page Titles", "Title 1 Length");
  await expect.poll(headerText).toContain("Title 1 Length ↓");
  await expect.poll(async () => (await alpha.settledGrid()).rows.map(length)).toEqual(sortedDesc);
  tiesInAddressOrder((await alpha.settledGrid()).rows);

  // 3. Klick: Sortierung aufgehoben, Ursprungsreihenfolge (Reihenfolge des Auffindens)
  await sortBy(alpha, "Page Titles", "Title 1 Length");
  await expect.poll(headerText).not.toMatch(/[↑↓]/);
  await expect.poll(async () => addresses((await alpha.settledGrid()).rows)).toEqual(addresses(original));
});

// Alpha 2, einfache Suche und Zähler „N von M“ (UI-011, UI-006): fremde IDs, kein Spec-Tag.
test("simple search matches the Address on Enter, shows N of M and clears with Escape", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha, "Response Codes");
  await expect(alpha.table()).toContainText("Showing: 27 of 27");
  const search = page.getByLabel("Search Address", { exact: true });
  await search.fill("PAGE-1");
  // ohne Enter ändert sich nichts
  await expect(alpha.table()).toContainText("Showing: 27 of 27");
  await search.press("Enter");
  // page-1.html und page-10 … page-19: Teilstring, ohne Groß-/Kleinschreibung
  await expect(alpha.table()).toContainText(/Showing: 11 of 27.*Search active/);
  const hits = addresses((await alpha.settledGrid()).rows);
  expect(hits).toHaveLength(11);
  expect(hits.every((a) => /page-1/.test(a))).toBe(true);
  await search.fill("no-such-address-xyz");
  await search.press("Enter");
  await expect(alpha.table()).toContainText(/Showing: 0 of 27.*Search active/);
  await search.press("Escape");
  await expect(alpha.table()).toContainText("Showing: 27 of 27");
  await expect(alpha.table()).not.toContainText("Search active");
});

type Snapshot = { address: string; relative: number; absolute: number };

/**
 * Sortiert die Tabelle während eines laufenden (gedrosselten) Crawls nach Address, markiert eine Zeile mitten im Bild und beobachtet
 * sie bis zum Ende des Crawls. Gewertet werden nur Momentaufnahmen, die zweimal im Abstand von 150 ms gleich sind: Während der
 * Live-Aktualisierung zeigt die Tabelle kurz Platzhalter („…“) und die Auswahl ist für Augenblicke nicht markiert (Befund A01-07).
 */
async function selectWhileSortedRowsArrive(alpha: Alpha): Promise<{ origin: Snapshot; samples: Snapshot[]; last: Snapshot }> {
  const { page } = alpha;
  await alpha.open();
  await alpha.slowCrawl(2);
  await alpha.selectTab("Response Codes");
  await alpha.startFromHeader();
  // Sobald die Tabelle gefüllt ist, nach Address aufsteigend sortieren und eine Zeile markieren. Gesetzt wird die Sortierung über den
  // Zustand der Tabellen-Tabs: der Kopfklick bzw. Enter blieben während eines laufenden Crawls oft wirkungslos (Befund A01-09).
  await expect.poll(() => alpha.counter().catch(() => ""), { timeout: 20_000 }).toMatch(/Showing: (1\d|2\d) of/);
  await page.evaluate(async () => {
    const path = "/src/state/tabs.ts";
    const { useTabs } = await import(/* @vite-ignore */ path);
    useTabs.getState().query({ sort: [{ column: "Address", dir: "asc" }] });
  });
  await expect.poll(async () => (await alpha.gridWindow()).headers.join("|")).toContain("Address ↑");
  await alpha.settledGrid();
  await page.locator(".os-data-grid").click({ position: { x: 160, y: 26 + 22 * 4 + 11 } });
  await expect(alpha.table()).toContainText("1 selected");

  const read = async (): Promise<Snapshot | null> => {
    const w = await alpha.gridWindow();
    if (w.rows.some((r) => r.cells.some((c) => c === "…"))) return null;
    const first = Math.min(...w.rows.map((r) => r.index));
    const sel = w.rows.find((r) => r.selected);
    return sel ? { address: sel.cells[0]!, relative: sel.index - first, absolute: sel.index } : null;
  };
  const stable = async (): Promise<Snapshot | null> => {
    const a = await read();
    await page.waitForTimeout(150);
    const b = await read();
    return a && b && JSON.stringify(a) === JSON.stringify(b) ? a : null;
  };
  let origin: Snapshot | null = null;
  for (let i = 0; i < 40 && !origin; i++) origin = await stable();
  expect(origin, "markierte Zeile ist nach dem Klick sichtbar").not.toBeNull();
  const samples: Snapshot[] = [];
  for (let i = 0; i < 80; i++) {
    const state = await stable();
    if (state) samples.push(state);
    if (/Completed/.test(await page.getByTestId("crawl-phase").innerText())) break;
    await page.waitForTimeout(200);
  }
  await alpha.waitCompleted();
  // Nach dem Ende wird die markierte Zeile wiedergefunden. Unter Last bleibt die Tabelle gelegentlich leer oder zeigt einen Fehler (Befund A01-16);
  // dann wird über einen anderen Tab erneut gewechselt (Annotation `counts-stuck`). Der Zähler „N von M“ wird in anderen Tests geprüft.
  let last: Snapshot | null = null;
  for (let attempt = 0; attempt < 3 && !last; attempt++) {
    for (let i = 0; i < 30 && !last; i++) last = await stable();
    if (!last) {
      test.info().annotations.push({ type: "counts-stuck", description: `Response Codes: markierte Zeile nach Crawl-Ende nicht sichtbar (${await alpha.counter().catch(() => "?")})` });
      await alpha.selectTab("External");
      await alpha.selectTab("Response Codes");
    }
  }
  expect(last, "markierte Zeile nach Abschluss sichtbar").not.toBeNull();
  return { origin: origin!, samples, last: last! };
}

// spec: UI-018
test("the selected URL stays selected while rows are sorted in above it during the crawl", async ({ alpha }) => {
  test.setTimeout(90_000);
  const { origin, samples, last } = await selectWhileSortedRowsArrive(alpha);
  expect(samples.length, "genügend auswertbare Momentaufnahmen").toBeGreaterThanOrEqual(6);
  for (const s of samples) expect(s.address).toBe(origin.address);
  expect(last.address).toBe(origin.address);
  // nachträglich gefundene URLs liegen oberhalb der Auswahl: ihre Zeilennummer wächst
  expect(last.absolute, "neue URLs wurden oberhalb der Auswahl einsortiert").toBeGreaterThan(origin.absolute);
});

// spec: UI-018
test("the selected row keeps its screen position while rows are sorted in above it (anchor on the focus row)", async ({ alpha }) => {
  test.fixme(true, "Befund A01-10 (docs/plan/M2-acceptance/alpha/befunde.md): die markierte Zeile rutscht beim Einsortieren neuer Zeilen um 1–2 Positionen nach unten (UI-018, Anker auf Fokuszeile)");
  test.setTimeout(90_000);
  const { origin, samples, last } = await selectWhileSortedRowsArrive(alpha);
  for (const s of [...samples, last]) expect(s.relative).toBe(origin.relative);
});

// spec: UI-004 [P1]
test("tabs can be reordered and hidden, persist through a restart and reset; Internal cannot be hidden", async ({ alpha }) => {
  const { page } = alpha;
  await alpha.open();
  await expect(page.getByRole("tab", { name: "Internal", exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Images", exact: true }).dragTo(page.getByRole("tab", { name: "Internal", exact: true }));
  const dataTabs = page.getByRole("tablist", { name: "Data tabs" }).getByRole("tab");
  await expect(dataTabs.first()).toHaveText("Images");
  // Per Tastatur geöffnet: ein Tab überdeckt die Schaltfläche für Mausklicks (Befund A01-08)
  const configure = page.getByRole("button", { name: "Configure tabs", exact: true });
  await configure.focus();
  await configure.press("Enter");
  await expect(page.getByRole("menuitemcheckbox", { name: "Internal", exact: true })).toBeDisabled();
  await page.getByRole("menuitemcheckbox", { name: "Hreflang", exact: true }).click();
  await expect(page.getByRole("menuitemcheckbox", { name: "Hreflang", exact: true })).toHaveAttribute("aria-checked", "false");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("tab", { name: "Hreflang", exact: true })).toHaveCount(0);
  // gespeichert wird über die Bridge (ui-state.json); erst danach neu laden
  await expect
    .poll(async () => JSON.stringify(((await alpha.invoke<{ state: { topTabs?: unknown } }>("uistate.load")).state ?? {}).topTabs ?? null))
    .toContain("Hreflang");
  await page.reload();
  await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
  await expect(dataTabs.first()).toHaveText("Images");
  await expect(page.getByRole("tab", { name: "Hreflang", exact: true })).toHaveCount(0);
  // Zurücksetzen stellt Standardreihenfolge und -sichtbarkeit wieder her
  await configure.focus();
  await configure.press("Enter");
  await page.getByRole("menuitem", { name: "Reset tabs", exact: true }).click();
  await expect(dataTabs.first()).toHaveText("Internal");
  await expect(page.getByRole("tab", { name: "Hreflang", exact: true })).toBeVisible();
});

// spec: UI-007
test("empty values stay empty cells, status codes and numbers appear as text and Address is pinned left", async ({ alpha }) => {
  await crawled(alpha, "Page Titles", TALL);
  await expect(alpha.table()).toContainText("Showing: 22 of 22");
  const window = await alpha.settledGrid();
  expect(window.headers[0]).toBe("Address");
  for (const row of window.rows) for (const cell of row.cells) expect(["null", "undefined", "NaN", "0 undefined"]).not.toContain(cell);
  const home = window.rows.find((r) => r.cells[0] === alpha.startUrl)!;
  expect(home.cells[window.headers.indexOf("Title 2")]).toBe(""); // keine zweite Titelangabe: leere Zelle
  expect(home.cells[window.headers.indexOf("Title 1 Length")]).toBe("14");
  await alpha.openTable("Response Codes");
  const status = (await alpha.settledGrid()).rows.map((r) => r.cells[2]);
  expect(status).toContain("200");
  expect(status).toContain("301");
  expect(status).toContain("404");
  // Address bleibt beim horizontalen Scrollen stehen (angeheftet): die erste Spalte ist in der Spaltenverwaltung als `pinned` markiert
  const pinned = await alpha.page.evaluate(async () => {
    const path = "/src/state/columns.ts";
    const { useColumns } = await import(/* @vite-ignore */ path);
    return (useColumns.getState().byTab["Response Codes"] as { key: string; pinned: boolean }[]).filter((c) => c.pinned).map((c) => c.key);
  });
  expect(pinned).toContain("Address");
});
