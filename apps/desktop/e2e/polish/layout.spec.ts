// Abnahme M2-U34 (4) und Befunde A02-05, A02-17, A02-19, A02-21 im Hauptfenster gegen die echte Bridge.
import { expect, test } from "../alpha/fixture";
import { SIZES, crawledWithSelection, showPanes } from "./support";

test.use({ locale: "de-DE" });

// spec: UI-015
// spec: UI-001
test("exactly one „Exportieren“ outside the detail area and no empty column next to the table (A02-05)", async ({ alpha }) => {
  const { page } = alpha;
  await crawledWithSelection(alpha);
  for (const size of SIZES) {
    await page.setViewportSize(size);
    await showPanes(page, "default");
    const outside = await page.evaluate(() => {
      const detail = document.getElementById("shell-detail");
      const named = [...document.querySelectorAll<HTMLElement>("button, [role=button]")]
        .filter((b) => b.getClientRects().length > 0 && !detail?.contains(b))
        .filter((b) => (b.getAttribute("aria-label") ?? b.textContent ?? "").trim() === "Exportieren");
      return named.length;
    });
    expect(outside, `${size.width}×${size.height}: „Exportieren“ außerhalb des Detailbereichs`).toBe(1);
    const geometry = await page.evaluate(() => {
      const main = document.querySelector<HTMLElement>("main[data-region=table]")!.getBoundingClientRect();
      const table = document.querySelector<HTMLElement>(".u10-table")!.getBoundingClientRect();
      const aside = document.getElementById("shell-sidebar")!.getBoundingClientRect();
      const grip = document.querySelector<HTMLElement>(".os-shell-divider-sidebar")?.getBoundingClientRect().width ?? 0;
      return { width: window.innerWidth, main: main.width, mainLeft: main.left, tableLeft: table.left, table: table.width, sidebar: aside.width, grip };
    });
    // UI-001: Tabellenfläche = Fensterbreite − Seitenleiste − Trenner; die Tabelle beginnt am linken Rand der Fläche.
    expect(Math.abs(geometry.main - (geometry.width - geometry.sidebar - geometry.grip)), JSON.stringify(geometry)).toBeLessThanOrEqual(1);
    expect(Math.abs(geometry.tableLeft - geometry.mainLeft), JSON.stringify(geometry)).toBeLessThanOrEqual(1);
    expect(Math.abs(geometry.table - geometry.main), JSON.stringify(geometry)).toBeLessThanOrEqual(1);
  }
});

// spec: OUT-075 [P1]
test("the multi export shows its selection after a crawl and enables the export once something is selected (A02-17)", async ({ alpha }) => {
  const { page } = alpha;
  await crawledWithSelection(alpha);
  await page.evaluate(async () => {
    const path = "/src/ui/actions.ts";
    const { runAction } = await import(/* @vite-ignore */ path);
    runAction("export.multi", { source: "menu" });
  });
  const dialog = page.getByRole("dialog", { name: "Exportoptionen" });
  for (const name of ["Ausgabe", "Auswahl", "Lokale Dateien", "Looker Studio", "Vorlagen"]) await expect(dialog.getByRole("heading", { name })).toBeVisible();
  const tree = dialog.getByRole("group", { name: "Verfügbare Exporte" });
  // Der Baum ist gefüllt und hat eine eigene, sichtbare Höhe (Befund: Auswahl nicht auffindbar).
  await expect(tree.getByRole("group", { name: "Intern", exact: true })).toBeVisible();
  expect((await tree.boundingBox())!.height).toBeGreaterThanOrEqual(200);
  await expect(dialog.getByText("1 Pflichtangabe fehlt")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Exportieren…" })).toBeDisabled();
  await expect(dialog.getByRole("checkbox", { name: /Looker Studio/ })).toBeDisabled();
  await expect(dialog.getByText(/Kein Google-Drive-Konto verbunden/)).toBeVisible();
  await tree.getByRole("group", { name: "Intern", exact: true }).getByText("Alle", { exact: true }).click();
  await expect(dialog.getByRole("list", { name: "Ausgewählt (1)" })).toContainText("Intern › Alle");
  await expect(dialog.getByRole("button", { name: "Exportieren…" })).toBeEnabled();
});

// spec: UI-068
test("segments page: JSON field below its label at full width and a German file choice (A02-19)", async ({ alpha }) => {
  const { page } = alpha;
  await alpha.open();
  await page.evaluate(async () => {
    const path = "/src/features/config/frame/store.ts";
    const { openConfig } = await import(/* @vite-ignore */ path);
    openConfig({ anchor: "segments" });
  });
  const dialog = page.getByRole("dialog", { name: "Crawl-Konfiguration" });
  const json = dialog.getByRole("textbox", { name: "Segmentliste als JSON" });
  await expect(json).toBeVisible();
  const label = dialog.locator(".os-segments-json > label");
  const [l, j, page_] = await Promise.all([label.boundingBox(), json.boundingBox(), dialog.locator(".os-segments-json").boundingBox()]);
  expect(j!.y).toBeGreaterThanOrEqual(l!.y + l!.height - 1);
  expect(Math.abs(j!.width - page_!.width)).toBeLessThanOrEqual(2);
  await expect(dialog.getByRole("button", { name: "Datei wählen…" })).toBeVisible();
  await expect(dialog.getByText("Keine Datei ausgewählt")).toBeVisible();
  await expect(dialog.locator("input[type=file]")).toBeHidden();
});

// spec: UI-016
test("rendering page explains why JavaScript rendering is locked and what it needs (A02-21)", async ({ alpha }) => {
  const { page } = alpha;
  await crawledWithSelection(alpha);
  await page.evaluate(async () => {
    const path = "/src/features/config/frame/store.ts";
    const { openConfig } = await import(/* @vite-ignore */ path);
    openConfig({ anchor: "rendering.mode" });
  });
  const dialog = page.getByRole("dialog", { name: "Crawl-Konfiguration" });
  await expect(dialog.getByTestId("rendering-locked-how")).toContainText("einen neuen Crawl beginnen");
  await expect(dialog.getByTestId("rendering-chromium-note")).toContainText("JavaScript-Rendering braucht Chromium");
});
