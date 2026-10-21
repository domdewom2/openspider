// Alpha-Kriterium 3 (docs/plan/M2.md 4.3): Die Auswahl lädt URL Details, Inlinks, Outlinks, HTTP Headers, Resources und Image Details;
// die Seitenleiste zeigt Overview und Issues mit Erklärung (Text = MCP-Text); ein Klick navigiert in Tab + Filter.
import { readdirSync } from "node:fs";
import type { Page } from "@playwright/test";
import { mcpCall, type McpIssue } from "./mcp";
import { expect, test, type Alpha } from "./fixture";

const CLIENT_ERROR = "Response Codes: Internal Client Error (4XX)";

/** Veröffentlicht die Auswahl über die Übergabeschnittstelle des Detailfensters (siehe Befund A01-02, die Haupttabelle tut es nicht). */
async function selectInDetails(page: Page, address: string): Promise<number> {
  return page.evaluate(async (address) => {
    const viewPath = "/src/ipc/view.ts";
    const selectionPath = "/src/features/detail/frame/selection.ts";
    const { view } = await import(/* @vite-ignore */ viewPath);
    const { setDetailSelection } = await import(/* @vite-ignore */ selectionPath);
    // Die Ansicht kann zwischen open und rows eine neue Version bekommen (Analyse läuft nach): dann erneut versuchen.
    let rows: { cells: unknown[][]; rowKeys: number[] } | undefined;
    for (let attempt = 0; attempt < 15 && !rows; attempt++) {
      if (attempt > 0) await new Promise((r) => setTimeout(r, 300));
      const handle = await view.open({ spec: { kind: "tab", tab: "Response Codes", filter: "All", search: null, sort: [] }, columns: ["Address"] });
      try {
        rows = await view.rows({ viewId: handle.viewId, version: handle.version, offset: 0, limit: 100, columns: ["Address"] });
      } catch (error) {
        if (!String((error as Error).message).includes("stale")) throw error;
      } finally {
        await view.close({ viewId: handle.viewId });
      }
    }
    if (!rows) throw new Error("view.rows blieb veraltet");
    const index = rows.cells.findIndex((c: unknown[]) => c[0] === address);
    if (index < 0) throw new Error(`${address} nicht in der Ansicht`);
    const id = rows.rowKeys[index] as number;
    setDetailSelection({ urlIds: [id], focus: id, topTab: "Response Codes" });
    return id;
  }, address);
}

async function crawled(alpha: Alpha, sections?: Record<string, Record<string, unknown>>): Promise<void> {
  await alpha.open();
  if (sections) await alpha.useProfile("alpha-detail", sections);
  await alpha.startFromHeader();
  await alpha.waitCompleted();
}

const detail = (page: Page) => page.getByRole("region", { name: "Details" });
const panel = (page: Page, name: string) => page.getByRole("tabpanel", { name, exact: true });
const openDetailTab = async (page: Page, name: string) => {
  await detail(page).getByRole("tab", { name, exact: true }).click();
  return panel(page, name);
};

// spec: UI-030
test("without a selection every detail tab asks for a URL in the upper table", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  for (const name of ["URL Details", "Inlinks", "Outlinks", "Image Details", "Resources", "HTTP Headers"]) {
    await expect(await openDetailTab(page, name)).toContainText("Select a row in the table above to see its details.");
  }
});

// spec: UI-030
test("selecting a row in the main table loads its URL Details", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await alpha.openTable("Response Codes");
  await alpha.settledGrid();
  await page.locator(".os-data-grid").click({ position: { x: 160, y: 26 + 11 } });
  await expect(await openDetailTab(page, "URL Details")).toContainText(alpha.startUrl);
});

// spec: UI-030
test("URL Details, Inlinks and Outlinks of the selected URL come from the core", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await selectInDetails(page, alpha.startUrl);

  const urlDetails = await openDetailTab(page, "URL Details");
  await expect(urlDetails).toContainText(alpha.startUrl);
  await expect(urlDetails.getByRole("row", { name: /^Status Code\s*200/ })).toBeVisible();
  await expect(urlDetails.getByRole("row", { name: /^Inlinks\s*20/ })).toBeVisible();
  await expect(urlDetails.getByRole("row", { name: /^Outlinks\s*23/ })).toBeVisible();

  // Die Startseite wird von 20 Seiten verlinkt und verlinkt 23 interne und 2 externe Adressen
  const inlinks = await openDetailTab(page, "Inlinks");
  await expect(inlinks.getByRole("grid")).toHaveAttribute("aria-rowcount", "21");
  await expect(inlinks).toContainText(`${alpha.site.base}/page-1.html`);
  const outlinks = await openDetailTab(page, "Outlinks");
  await expect(outlinks.getByRole("grid")).toHaveAttribute("aria-rowcount", "26");
  await expect(outlinks).toContainText(`${alpha.site.base}/page-1.html`);
});

// spec: UI-030
test("HTTP Headers show request and response headers when header extraction is on", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha, { extraction: { http_headers: true } });
  await selectInDetails(page, alpha.startUrl);
  const headers = await openDetailTab(page, "HTTP Headers");
  await expect(headers).toContainText("Request headers");
  await expect(headers).toContainText("Response headers");
  await expect(headers).toContainText(/content-type\s*text\/html; charset=utf-8/);
  await expect(headers).toContainText("HTTP/1.1 200");
});

// spec: UI-030
test("HTTP Headers without header extraction name the setting that is missing", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await selectInDetails(page, alpha.startUrl);
  await expect(await openDetailTab(page, "HTTP Headers")).toContainText(/HTTP header/i);
});

// spec: UI-030
test("Image Details list the images of the selected page", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await selectInDetails(page, alpha.startUrl);
  const images = await openDetailTab(page, "Image Details");
  await expect(images).toContainText(`${alpha.site.base}/pixel.svg`);
});

// spec: UI-030
test("Resources list the resources of the selected page", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await selectInDetails(page, alpha.startUrl);
  const resources = await openDetailTab(page, "Resources");
  await expect(resources).toContainText(`${alpha.site.base}/pixel.svg`);
});

// Alpha 3, Overview der Seitenleiste (UI-061): Summe und Gruppen stimmen mit der Fixture-Zusammenfassung überein (expected.json von fx-ui-basic).
test("the sidebar Overview shows the crawl summary and the Internal groups with counts", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  const overview = page.getByRole("tabpanel", { name: "Overview" });
  await expect(overview).toBeVisible();
  const summary = overview.getByRole("table", { name: "Summary" });
  const value = async (name: RegExp) => Number((await summary.getByRole("row", { name }).getByRole("cell").first().innerText()).replace(/\D/g, ""));
  expect(await value(/^Total URLs Encountered/)).toBe(29);
  expect(await value(/^Total URLs Crawled/)).toBe(29);
  expect(await value(/^Total Internal URLs/)).toBe(25);
  expect(await value(/^Total External URLs/)).toBe(2);
  const internal = overview.getByRole("table", { name: "Internal", exact: true });
  await expect(internal.getByRole("row", { name: /^All\s*25\b/ })).toBeVisible();
  await expect(internal.getByRole("row", { name: /^HTML\s*23\b/ })).toBeVisible();
  await expect(internal.getByRole("row", { name: /^Images\s*1\b/ })).toBeVisible();
  const codes = overview.getByRole("table", { name: "Response Codes", exact: true });
  await expect(codes.getByRole("row", { name: /^Client Error \(4XX\)\s*1\b/ })).toBeVisible();
});

function crawlIdOf(alpha: Alpha): string {
  const files = readdirSync(alpha.bridge.crawlsDir).filter((f) => f.endsWith(".ospider"));
  expect(files).toHaveLength(1);
  return files[0]!.replace(/\.ospider$/, "");
}

// Alpha 3, Issues mit Erklärung (UI-062, AGENT-013): der Text der Seitenleiste ist der Text der MCP-Antwort `list_issues`.
test("the Issues explanation equals the MCP text in English and German", async ({ alpha }) => {
  test.setTimeout(90_000);
  const { page } = alpha;
  await crawled(alpha);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  const issues = page.getByRole("tabpanel", { name: "Issues" });
  const row = issues.getByRole("button", { name: CLIENT_ERROR, exact: true });
  await expect(row).toBeVisible();
  await expect(issues.getByRole("row", { name: new RegExp(`^${CLIENT_ERROR.replace(/[()]/g, "\\$&")}.*\\b1\\b`) })).toBeVisible();
  await row.click();

  const explanation = page.getByRole("region", { name: /^(Issue explanation|Befund-Erklärung)$/ });
  const texts = async () => ({
    what: await explanation.locator("dt").nth(0).locator("xpath=following-sibling::dd[1]").innerText(),
    why: await explanation.locator("dt").nth(1).locator("xpath=following-sibling::dd[1]").innerText(),
    fix: await explanation.locator("dt").nth(2).locator("xpath=following-sibling::dd[1]").innerText(),
  });
  const mcp = async (lang: "en" | "de") => {
    const reply = await mcpCall(alpha.bridge.home, "list_issues", { crawl_id: crawlIdOf(alpha), lang, limit: 500 });
    const issue = (reply.summary?.issues ?? reply.issues ?? []).find((i: McpIssue) => `${i.tab}: ${i.filter}`.toLowerCase() === CLIENT_ERROR.toLowerCase());
    expect(issue, `list_issues liefert ${CLIENT_ERROR}`).toBeDefined();
    return { what: issue.what as string, why: issue.why as string, fix: issue.fix as string };
  };
  await expect(explanation).toContainText("What does this mean?");
  expect(await texts()).toEqual(await mcp("en"));

  // Sprachwechsel ohne Neustart: die Erklärung wird deutsch und entspricht dem deutschen MCP-Text
  await page.evaluate(async () => {
    const path = "/src/state/prefs.ts";
    const { usePrefs } = await import(/* @vite-ignore */ path);
    await usePrefs.getState().patch({ general: { language: "de" } });
  });
  await expect(page.getByRole("region", { name: "Befund-Erklärung" })).toContainText("Was bedeutet das?");
  const german = await mcp("de");
  expect(await texts()).toEqual(german);
  expect(german.what).not.toEqual((await mcp("en")).what);
});

// spec: UI-018
test("a click on an issue publishes tab and filter as the table navigation target", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  await page.getByRole("tabpanel", { name: "Issues" }).getByRole("button", { name: CLIENT_ERROR, exact: true }).click();
  const target = await page.evaluate(async () => {
    const path = "/src/features/sidebar/frame/state.ts";
    const { useSidebarNavigation } = await import(/* @vite-ignore */ path);
    return useSidebarNavigation.getState().target?.spec;
  });
  expect(target).toMatchObject({ kind: "tab", tab: "Response Codes", filter: "Internal Client Error (4XX)" });
});

test("a click on an issue or an Overview row navigates the table to that tab and filter", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  await page.getByRole("tabpanel", { name: "Issues" }).getByRole("button", { name: CLIENT_ERROR, exact: true }).click();
  await expect(page.getByRole("tab", { name: "Response Codes", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(alpha.table()).toContainText("Showing: 1 of 27");
  expect((await alpha.settledGrid()).rows[0]!.cells[0]).toBe(`${alpha.site.base}/missing`);
});

// Alpha 3, Issue-Namen (UI-062, UI-064): Schwellwerte in den Namen sind aufgelöst, keine Platzhalter der Filterdefinition.
test("Issue names show the configured thresholds instead of raw placeholders", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  const issues = page.getByRole("tabpanel", { name: "Issues" });
  await expect(issues.getByRole("button", { name: CLIENT_ERROR, exact: true })).toBeVisible();
  expect(await issues.innerText()).not.toMatch(/\{[a-z_.]+\}/);
});

// Alpha 3, Seitenleiste (UI-060): jeder Reiter lässt sich mit der Maus erreichen.
test("every sidebar tab can be selected with the mouse", async ({ alpha }) => {
  const { page } = alpha;
  await crawled(alpha);
  const tabs = page.getByRole("tablist", { name: "Crawl summary" }).getByRole("tab");
  const names = await tabs.allInnerTexts();
  expect(names).toContain("Spelling & Grammar");
  for (const name of names) {
    await page.getByRole("tablist", { name: "Crawl summary" }).getByRole("tab", { name, exact: true }).click({ timeout: 5000 });
    await expect(page.getByRole("tablist", { name: "Crawl summary" }).getByRole("tab", { name, exact: true })).toHaveAttribute("aria-selected", "true");
  }
});
