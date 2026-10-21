// Alpha-Kriterium 4 (docs/plan/M2.md 4.3) und „UI = CLI“ (OUT-001, OUT-182): Exporte der Ansicht, ein Bulk-Export und ein Report aus den
// Menüs sind nach Normalisierung identisch mit `openspider --export-tabs/--bulk-export/--save-report` – für dieselbe Crawl-Datei
// (Byte für Byte) und für zwei getrennte Crawls derselben Fixture (UI über die Bridge, CLI über `--crawl`).
import { copyFileSync, existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { normalizeTable, readCsv } from "./csv";
import { expect, test, type Alpha } from "./fixture";
import { listFiles, removeDir, runCli, tempDir, toolBinary } from "./support";
import { readXlsx } from "./xlsx";
import { spawnSync } from "node:child_process";

const VOLATILE_COLUMNS = ["Response Time", "Crawl Timestamp"];
const VOLATILE_ITEMS = ["Crawl Start", "Crawl End", "Elapsed Time", "Average Response Time (ms)", "Average URLs/s"];

/** UI-Crawl der Fixture bis zum Ende inklusive Analyse; liefert den Pfad einer eigenständigen Kopie der Crawl-Datei. */
async function crawlAndSnapshot(alpha: Alpha, dir: string): Promise<string> {
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  const files = readdirSync(alpha.bridge.crawlsDir).filter((f) => f.endsWith(".ospider"));
  expect(files).toHaveLength(1);
  const source = join(alpha.bridge.crawlsDir, files[0]!);
  const copy = join(dir, "ui-crawl.ospider");
  // die Bridge hält die Datei im WAL-Modus offen: Datenbank und Journal gemeinsam kopieren
  copyFileSync(source, copy);
  if (existsSync(`${source}-wal`)) copyFileSync(`${source}-wal`, `${copy}-wal`);
  return copy;
}

interface CliExport {
  folder: string;
  stdout: string;
}

async function cliExport(args: string[], dir: string, name: string): Promise<CliExport> {
  const folder = join(dir, name);
  const result = await runCli([...args, "--output-folder", folder], { home: join(dir, `${name}-home`) });
  expect(result.code, `openspider ${args.join(" ")}\n${result.stderr}`).toBe(0);
  return { folder, stdout: result.stdout };
}

const sameBytes = (a: string, b: string) => expect(readFileSync(a).equals(readFileSync(b)), `${a} und ${b} sind byte-identisch`).toBe(true);

/** Öffnet die Exportansicht des Menüs (`export.bulk.*`, `export.report.*`) und speichert in den vorbereiteten Ordner. */
async function exportFromMenu(page: Page, action: string, options: { format?: "xlsx" } = {}): Promise<void> {
  await page.evaluate(async (id) => {
    const path = "/src/ui/actions.ts";
    const { runAction } = await import(/* @vite-ignore */ path);
    runAction(id, { source: "menu" });
  }, action);
  const dialog = page.getByRole("dialog", { name: "Export options" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText(/^Suggested filename: /)).toBeVisible();
  if (options.format === "xlsx") {
    await dialog.getByRole("button", { name: "File format" }).click();
    await page.getByRole("option", { name: "Excel Workbook (.xlsx)" }).click();
    await expect(dialog.getByText(/^Suggested filename: .*\.xlsx$/)).toBeVisible();
  }
  await dialog.getByRole("button", { name: "Save…" }).click();
  await expect(page.getByRole("region", { name: "Export status" })).toContainText("Export complete.");
}

/** Menüeintrag des Hauptfensters im Menümodell (das native Menü wird aus diesem Modell gebaut). */
async function menuItem(page: Page, menu: string, id: string): Promise<{ enabled: boolean; label: string } | undefined> {
  return page.evaluate(
    ({ menu, id }) => {
      type Item = { id: string; label: string; enabled: boolean; items?: Item[] };
      const find = (items: Item[]): Item | undefined => {
        for (const item of items) {
          if (item.id === id) return item;
          const hit = item.items ? find(item.items) : undefined;
          if (hit) return hit;
        }
        return undefined;
      };
      const model = (window as unknown as { __osMenu: { model: { menus: { id: string; items: Item[] }[] } } }).__osMenu.model;
      const hit = find(model.menus.find((m) => m.id === menu)?.items ?? []);
      return hit && { enabled: hit.enabled, label: hit.label };
    },
    { menu, id },
  );
}

// spec: OUT-001 [M2]
// spec: OUT-006
test("a view exported as CSV from the table equals openspider --export-tabs of the same crawl file", async ({ alpha }) => {
  test.setTimeout(120_000);
  const { page } = alpha;
  const dir = tempDir("os-alpha-export-");
  try {
    const crawlFile = await crawlAndSnapshot(alpha, dir);
    await alpha.openTable("Response Codes");
    await expect(alpha.table()).toContainText("Showing: 27 of 27");
    await alpha.settledGrid();

    // UI: Export-Schaltfläche der Tabelle (Pfad im Dialog, CSV) – einmal für „All“, einmal für den Filter „Client Error (4XX)“
    const uiAll = join(dir, "ui-all.csv");
    await alpha.table().getByRole("button", { name: "Export", exact: true }).click();
    await page.getByRole("textbox", { name: "Destination CSV file" }).fill(uiAll);
    await page.getByRole("dialog").getByRole("button", { name: "Export", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Complete" }).first()).toBeVisible();

    await alpha.table().getByRole("button", { name: /Filter/, exact: true }).click();
    await page.getByRole("option", { name: /^Client Error \(4XX\)\s*1$/ }).click();
    await expect(alpha.table()).toContainText("Showing: 1 of 27");
    const uiFiltered = join(dir, "ui-4xx.csv");
    await alpha.table().getByRole("button", { name: "Export", exact: true }).click();
    await page.getByRole("textbox", { name: "Destination CSV file" }).fill(uiFiltered);
    await page.getByRole("dialog").getByRole("button", { name: "Export", exact: true }).click();
    await expect.poll(() => existsSync(uiFiltered) && statSync(uiFiltered).size > 0).toBe(true);

    // CLI: dieselbe Crawl-Datei
    const cli = await cliExport(["--load-crawl", crawlFile, "--export-tabs", "Response Codes:All,Response Codes:Client Error (4xx)"], dir, "cli");
    expect(listFiles(cli.folder)).toEqual(expect.arrayContaining(["response_codes_all.csv", "response_codes_client_error_4xx.csv"]));
    sameBytes(uiAll, join(cli.folder, "response_codes_all.csv"));
    sameBytes(uiFiltered, join(cli.folder, "response_codes_client_error_4xx.csv"));
    // Inhalt: 27 Zeilen plus Kopfzeile, die 404 steht im gefilterten Export
    expect(readCsv(uiAll)).toHaveLength(28);
    const failing = readCsv(uiFiltered);
    expect(failing).toHaveLength(2);
    expect(failing[1]![0]).toBe(`${alpha.site.base}/missing`);
  } finally {
    removeDir(dir);
  }
});

// spec: OUT-001 [M2]
test("the Internal:All view exported with the core command export.view equals openspider --export-tabs, CSV and XLSX", async ({ alpha }) => {
  test.setTimeout(120_000);
  const dir = tempDir("os-alpha-export-");
  try {
    const crawlFile = await crawlAndSnapshot(alpha, dir);
    // Die Oberfläche kann Internal nicht öffnen (A01-01) und die Tabelle bietet kein XLSX (A01-14); den Befehl, den die Dialoge aufrufen,
    // führt dieser Test mit den Standardspalten des Tabs unmittelbar aus.
    const columns = (await alpha.invoke<{ key: string; defaultVisible: boolean }[]>("catalog.columns", { tab: "Internal" })).filter((c) => c.defaultVisible).map((c) => c.key);
    const handle = await alpha.invoke<{ viewId: string; version: number }>("view.open", { spec: { kind: "tab", tab: "Internal", filter: "All", search: null, sort: [] }, columns });
    for (const format of ["csv", "xlsx"] as const) {
      const path = join(dir, `ui-internal.${format}`);
      const op = await alpha.invoke<{ opId: string }>("export.view", { viewId: handle.viewId, version: handle.version, path, format, allColumns: false, columns });
      expect(op.opId).toBeTruthy();
      await expect.poll(() => existsSync(path) && statSync(path).size > 0, { timeout: 20_000 }).toBe(true);
    }
    const cli = await cliExport(["--load-crawl", crawlFile, "--export-tabs", "Internal:All"], dir, "cli-csv");
    const cliX = await cliExport(["--load-crawl", crawlFile, "--export-tabs", "Internal:All", "--export-format", "xlsx"], dir, "cli-xlsx");
    sameBytes(join(dir, "ui-internal.csv"), join(cli.folder, "internal_all.csv"));
    expect(readCsv(join(dir, "ui-internal.csv"))).toHaveLength(26); // 25 interne URLs plus Kopfzeile
    // XLSX: ZIP-Zeitstempel unterscheiden sich, die Zellen nicht; der Blattname weicht ab (eigener Test, Befund A01-15)
    const ui = readXlsx(join(dir, "ui-internal.xlsx"));
    const reference = readXlsx(join(cliX.folder, "internal_all.xlsx"));
    expect(ui.sheets).toHaveLength(1);
    expect(reference.sheets).toHaveLength(1);
    expect(Object.values(ui.data)[0]).toEqual(Object.values(reference.data)[0]);
    expect(Object.values(ui.data)[0]).toHaveLength(26);
  } finally {
    removeDir(dir);
  }
});

// spec: OUT-001 [M2]
test("the XLSX sheet name of a view export equals the one of openspider --export-tabs", async ({ alpha }) => {
  test.fixme(true, "Befund A01-15 (docs/plan/M2-acceptance/alpha/befunde.md): export.view schreibt das Blatt „Export“, die CLI „Internal All“ (OUT-001: UI und CLI liefern dieselbe Datei; Blattname aus „Tab Filter“, OUT-007)");
  const dir = tempDir("os-alpha-export-");
  try {
    const crawlFile = await crawlAndSnapshot(alpha, dir);
    const columns = (await alpha.invoke<{ key: string; defaultVisible: boolean }[]>("catalog.columns", { tab: "Internal" })).filter((c) => c.defaultVisible).map((c) => c.key);
    const handle = await alpha.invoke<{ viewId: string; version: number }>("view.open", { spec: { kind: "tab", tab: "Internal", filter: "All", search: null, sort: [] }, columns });
    const path = join(dir, "ui-internal.xlsx");
    await alpha.invoke("export.view", { viewId: handle.viewId, version: handle.version, path, format: "xlsx", allColumns: false, columns });
    await expect.poll(() => existsSync(path) && statSync(path).size > 0, { timeout: 20_000 }).toBe(true);
    const cli = await cliExport(["--load-crawl", crawlFile, "--export-tabs", "Internal:All", "--export-format", "xlsx"], dir, "cli-xlsx");
    expect(readXlsx(path).sheets).toEqual(readXlsx(join(cli.folder, "internal_all.xlsx")).sheets);
  } finally {
    removeDir(dir);
  }
});

// spec: OUT-001 [M2]
// spec: OUT-006
test("a bulk export and a report from the menus equal the CLI exports of the same crawl file (CSV and XLSX)", async ({ alpha }) => {
  test.setTimeout(150_000);
  const { page } = alpha;
  const dir = tempDir("os-alpha-export-");
  try {
    const crawlFile = await crawlAndSnapshot(alpha, dir);
    await alpha.chooseExportPath(dir);
    const out = join(dir, "folder");

    // Menü: Bulk Export → Links → All Inlinks, Reports → Crawl Overview (beide aus dem Menümodell des Hauptfensters)
    await expect.poll(() => menuItem(page, "bulkExport", "export.bulk.Links:All Inlinks")).toMatchObject({ enabled: true });
    await expect.poll(() => menuItem(page, "reports", "export.report.Crawl Overview")).toMatchObject({ enabled: true });
    await exportFromMenu(page, "export.bulk.Links:All Inlinks");
    await exportFromMenu(page, "export.report.Crawl Overview");
    await exportFromMenu(page, "export.bulk.Links:All Inlinks", { format: "xlsx" });
    await expect(page.getByRole("button", { name: "Open folder" })).toBeVisible();
    expect(listFiles(out)).toEqual(expect.arrayContaining(["crawl_overview.csv", "links_all_inlinks.csv", "links_all_inlinks.xlsx"]));

    const cli = await cliExport(["--load-crawl", crawlFile, "--bulk-export", "Links:All Inlinks", "--save-report", "Crawl Overview"], dir, "cli-csv");
    const cliX = await cliExport(["--load-crawl", crawlFile, "--bulk-export", "Links:All Inlinks", "--export-format", "xlsx"], dir, "cli-xlsx");
    sameBytes(join(out, "links_all_inlinks.csv"), join(cli.folder, "links_all_inlinks.csv"));
    sameBytes(join(out, "crawl_overview.csv"), join(cli.folder, "crawl_overview.csv"));
    // XLSX: gleiche Zellen; der Blattname weicht ab („Export“ statt „Links All Inlinks“, Befund A01-15)
    const ui = readXlsx(join(out, "links_all_inlinks.xlsx"));
    const reference = readXlsx(join(cliX.folder, "links_all_inlinks.xlsx"));
    expect(Object.values(ui.data)).toEqual(Object.values(reference.data));
    // 47 Links auf interne Adressen der Fixture plus Kopfzeile (Zählung der CLI-Datei)
    expect(readCsv(join(out, "links_all_inlinks.csv")).length).toBeGreaterThan(40);

    // OUT-006: Format und Verzeichnis des letzten Exports sind vorbelegt, auch nach einem Neustart der Seite; „Ordner öffnen“ meldet den Pfad
    await page.getByRole("button", { name: "Open folder" }).click();
    await expect.poll(() => page.evaluate(() => (window as unknown as { __revealed?: string[] }).__revealed ?? [])).toHaveLength(1);
    await page.reload();
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await alpha.chooseExportPath(dir);
    await expect(page.getByTestId("crawl-phase")).toHaveText("Ready");
    const prefs = await alpha.invoke<{ export: { directory: string | null; format: string } }>("settings.get");
    expect(prefs.export.format).toBe("xlsx");
    expect(prefs.export.directory).toBe(out);
  } finally {
    removeDir(dir);
  }
});

// spec: OUT-001 [M2]
// spec: OUT-182 [M2]
test("the same fixture crawled through the UI and through openspider --crawl yields identical exports after normalization", async ({ alpha }) => {
  test.setTimeout(180_000);
  const { page } = alpha;
  const dir = tempDir("os-alpha-export-");
  try {
    // UI-Seite: Crawl über die Kopfleiste, Exporte über „Multi export…“ und die Menüs
    await crawlAndSnapshot(alpha, dir);
    await alpha.chooseExportPath(dir);
    await page.evaluate(async () => {
      const path = "/src/ui/actions.ts";
      const { runAction } = await import(/* @vite-ignore */ path);
      runAction("export.multi", { source: "menu" });
    });
    const dialog = page.getByRole("dialog", { name: "Export options" });
    await expect(dialog).toBeVisible();
    for (const [group, item] of [["Internal", "All"], ["Response Codes", "All"]] as const) {
      await dialog.getByRole("group", { name: group, exact: true }).getByText(item, { exact: true }).click();
      await expect(dialog.getByRole("group", { name: group, exact: true }).getByRole("checkbox", { name: item, exact: true })).toBeChecked();
    }
    await dialog.getByRole("textbox", { name: "Search exports" }).fill("Links:All Inlinks");
    await dialog.getByText("Links:All Inlinks", { exact: true }).click();
    await expect(dialog.getByRole("checkbox", { name: "Links:All Inlinks", exact: true })).toBeChecked();
    await dialog.getByRole("textbox", { name: "Search exports" }).fill("Crawl Overview");
    await dialog.getByText("Crawl Overview", { exact: true }).last().click();
    await expect(dialog.getByRole("checkbox", { name: "Crawl Overview", exact: true })).toBeChecked();
    await dialog.getByRole("button", { name: "Save…" }).click();
    await expect(page.getByRole("region", { name: "Export status" })).toContainText("Export complete.");
    const ui = join(dir, "folder");
    expect(listFiles(ui)).toEqual(expect.arrayContaining(["crawl_overview.csv", "internal_all.csv", "links_all_inlinks.csv", "response_codes_all.csv"]));

    // CLI-Seite: eigener Crawl derselben Fixture (gleicher Server, also gleiche Basis-URL), dieselben Exporte
    const cli = await cliExport(["--crawl", alpha.startUrl, "--export-tabs", "Internal:All,Response Codes:All", "--bulk-export", "Links:All Inlinks", "--save-report", "Crawl Overview"], dir, "cli");

    const normalize = { base: alpha.site.base, volatileColumns: VOLATILE_COLUMNS, volatileItems: VOLATILE_ITEMS };
    for (const file of ["internal_all.csv", "response_codes_all.csv", "links_all_inlinks.csv", "crawl_overview.csv"]) {
      const a = normalizeTable(readCsv(join(ui, file)), normalize);
      const b = normalizeTable(readCsv(join(cli.folder, file)), normalize);
      expect(a.header, `${file}: Kopfzeile`).toEqual(b.header);
      expect(a.rows, `${file}: Zeilen nach Normalisierung`).toEqual(b.rows);
    }

    // zusätzlich der Vergleichsweg der Parität: os-parity diff mit einer aus der Kopfzeile erzeugten Zuordnung (Schlüssel Address)
    for (const file of ["internal_all.csv", "response_codes_all.csv"]) {
      const header = readCsv(join(ui, file))[0]!;
      const mapping = join(dir, file.replace(/\.csv$/, ".toml"));
      writeFileSync(
        mapping,
        [`key = ["Address"]`, "", "[columns]", ...header.map((c) => (VOLATILE_COLUMNS.includes(c) ? `"${c}" = { ignore = true, reason = "flüchtig" }` : `"${c.replace(/"/g, '\\"')}" = "${c.replace(/"/g, '\\"')}"`))].join("\n"),
      );
      const out = join(dir, `parity-${file}`);
      const diff = spawnSync(toolBinary("os-parity"), ["diff", "--ref", join(cli.folder, file), "--os", join(ui, file), "--mapping", mapping, "--ref-base", alpha.site.base, "--os-base", alpha.site.base, "--out", out], { encoding: "utf8" });
      expect(diff.status, `os-parity diff ${file}\n${diff.stdout}${diff.stderr}`).toBe(0);
      expect(readFileSync(join(out, "report.md"), "utf8")).toContain("Ergebnis: **bestanden**");
    }
  } finally {
    removeDir(dir);
  }
});

// spec: OUT-006
test("the view export of the table offers CSV and Excel in the export dialog", async ({ alpha }) => {
  test.fixme(true, "Befund A01-14 (docs/plan/M2-acceptance/alpha/befunde.md): die Tabelle ruft den Export-Dialog von U16 nicht auf (registerExportContext wird nirgends gesetzt); sie öffnet einen eigenen CSV-Dialog mit Pfadfeld, XLSX der Ansicht ist über die Oberfläche nicht erreichbar");
  const { page } = alpha;
  const dir = tempDir("os-alpha-export-");
  try {
    await crawlAndSnapshot(alpha, dir);
    await alpha.openTable("Response Codes");
    await alpha.chooseExportPath(dir);
    await page.keyboard.press(process.platform === "darwin" ? "Meta+e" : "Control+e");
    const dialog = page.getByRole("dialog", { name: "Export options" });
    await dialog.getByRole("button", { name: "File format" }).click();
    await page.getByRole("option", { name: "Excel Workbook (.xlsx)" }).click();
    await dialog.getByRole("button", { name: "Save…" }).click();
    await expect(page.getByRole("region", { name: "Export status" })).toContainText("Export complete.");
    expect(listFiles(dir).some((f) => f.endsWith(".xlsx"))).toBe(true);
  } finally {
    removeDir(dir);
  }
});
