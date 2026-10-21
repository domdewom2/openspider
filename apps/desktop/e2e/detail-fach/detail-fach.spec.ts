import { deflateSync } from "node:zlib";
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// Prüfaufbau: e2e/detail-fach/scene.tsx (Detailrahmen + Mock-Kern, bis die Kern-Commands von M2-K09/D20/D21 gemergt sind).
const SCENE = "/e2e/detail-fach/index.html";
async function open(page: Page, query: Record<string, string>) {
  await page.goto(`${SCENE}?${new URLSearchParams(query)}`);
  await expect(page.getByRole("region", { name: "Details" })).toBeVisible();
}
/** Radio und Kontrollkästchen sind visuell versteckte Eingaben: Der Klick geht auf das umgebende Label (WebKit ignoriert Klicks auf die versteckte Eingabe). */
async function pick(page: Page, role: "radio" | "checkbox", name: string) {
  await page.getByRole(role, { name }).locator("xpath=ancestor::label[1]").click();
}
async function tab(page: Page, id: string, name: string) {
  const target = page.locator(`[data-tab-id="${id}"]`);
  if (await target.isVisible()) await target.click();
  else { await page.getByRole("button", { name: "More detail tabs" }).click(); await page.getByRole("menuitem", { name, exact: true }).click(); }
  await expect(page.getByRole("tabpanel", { name })).toBeVisible();
}
const calls = (page: Page, cmd: string) => page.evaluate(cmd => (window as unknown as { fach: { core: { calls: { cmd: string; args: unknown }[] } } }).fach.core.calls.filter(c => c.cmd === cmd).map(c => c.args), cmd);
async function noSeriousViolations(page: Page, scope: string) {
  const results = await new AxeBuilder({ page }).include(scope).analyze();
  expect(results.violations.filter(v => v.impact === "critical" || v.impact === "serious").map(v => `${v.id}: ${v.nodes.map(n => n.target.join(" ")).join(", ")}`)).toEqual([]);
}
/** Minimales PNG (Vollfläche), damit die Kacheln echte Bilder sind. */
function png(width: number, height: number, rgb: [number, number, number]): Buffer {
  const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = (buf: Buffer) => { let c = 0xffffffff; for (const b of buf) c = crcTable[(c ^ b) & 255]! ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (type: string, data: Buffer) => { const body = Buffer.concat([Buffer.from(type), data]); const out = Buffer.alloc(body.length + 8); out.writeUInt32BE(data.length, 0); body.copy(out, 4); out.writeUInt32BE(crc(body), body.length + 4); return out; };
  const header = Buffer.alloc(13); header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 2;
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array.from({ length: width }, () => rgb).flat())]);
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", header), chunk("IDAT", deflateSync(Buffer.concat(Array.from({ length: height }, () => row)))), chunk("IEND", Buffer.alloc(0))]);
}
const grid = (page: Page, name: string) => page.getByRole("grid", { name, exact: true });

// spec: UI-037
// spec: ONPAGE-194
test("SERP: device limits, bold terms, prefix, edit and reset with core measurements", async ({ page }) => {
  await open(page, { url: "3" });
  await tab(page, "serp", "SERP Snippet");
  await expect(page.locator(".os-serp-title")).toHaveAttribute("data-truncated", "true");
  await expect(page.locator(".os-serp-title")).toContainText(" …");
  await expect(page.getByRole("row", { name: /^Description/ })).toContainText("985");
  await pick(page, "radio", "Mobile");
  await expect(page.getByRole("row", { name: /^Description/ })).toContainText("680");
  await page.getByRole("textbox", { name: "Search terms to highlight" }).fill("aaa");
  await expect(page.locator(".os-serp-description b").first()).toHaveText("aaa");
  await pick(page, "radio", "Date");
  await expect(page.locator(".os-serp-prefix")).toHaveText("Oct 13, 2026 — ");
  const title = page.getByRole("textbox", { name: "Title" });
  await title.fill("Short");
  await expect(page.getByRole("row", { name: /^Title/ })).toContainText("5");
  await expect(page.locator(".os-serp-title")).toHaveAttribute("data-truncated", "false");
  await expect.poll(async () => (await calls(page, "detail.serpSaveEdit")).length).toBe(1);
  await expect(page.getByText("Edited")).toBeVisible();
  await noSeriousViolations(page, '[role="tabpanel"]');
  await page.getByRole("button", { name: "Reset", exact: true }).click();
  await expect(title).toHaveValue("W".repeat(30));
  expect(await calls(page, "detail.serpReset")).toEqual([{ urlId: 3 }]);
});

// spec: ONPAGE-194
test("SERP: a page without meta description gets a hint and German date format", async ({ page }) => {
  await open(page, { url: "4", lang: "de" });
  await tab(page, "serp", "SERP Snippet");
  await expect(page.getByText("Diese Seite hat keine Meta Description.")).toBeVisible();
  await pick(page, "radio", "Datum");
  await expect(page.locator(".os-serp-prefix")).toHaveText("13.10.2026 — ");
  await page.getByRole("textbox", { name: "Beschreibung" }).fill("Text");
  await expect(page.locator(".os-serp-description")).toContainText("13.10.2026 — Text");
});

// spec: ONPAGE-192 [M2]
test("Page Titles row: the SERP lower tab shows Title 1 and Meta Description 1 of the selected page", async ({ page }) => {
  await open(page, { url: "1", top: "Page Titles" });
  await tab(page, "serp", "SERP Snippet");
  await expect(page.getByRole("textbox", { name: "Title" })).toHaveValue("Start");
  await expect(page.getByRole("textbox", { name: "Description" })).toHaveValue("A short description of the start page.");
});

// spec: UI-039
test("Rendered page: tiled screenshot from the tile path, blocked resources by default and download of the whole image", async ({ page }) => {
  await page.route("**/e2e/detail-fach/shot/*", route => route.fulfill({ contentType: "image/png", body: png(64, 48, [30, 120, 200]) }));
  await open(page, { url: "1", shots: "1" });
  await tab(page, "rendered", "Rendered Page");
  // Kacheln werden erst nahe am sichtbaren Bereich geladen (8 192 px hohe Screenshots, UI-039): drei Platzhalter, Bilder folgen beim Scrollen.
  await expect(page.locator(".os-rendered-tile")).toHaveCount(3);
  const tiles = page.locator(".os-rendered-tile img");
  await expect.poll(() => tiles.evaluateAll(images => (images as HTMLImageElement[]).some(img => img.complete && img.naturalWidth === 64))).toBe(true);
  await page.locator(".os-rendered-shot").evaluate(node => { node.scrollTop = node.scrollHeight; });
  await expect(tiles).toHaveCount(3);
  await expect.poll(() => tiles.evaluateAll(images => (images as HTMLImageElement[]).every(img => img.complete && img.naturalWidth === 64))).toBe(true);
  await expect(grid(page, "Resources loaded while rendering").locator("canvas").first()).toBeVisible();
  expect((await calls(page, "view.open")).at(-1)).toMatchObject({ spec: { detail: "blockedResources" } });
  await pick(page, "radio", "100 %");
  await expect(page.locator(".os-rendered-tile").first()).toHaveCSS("width", "1024px");
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Save screenshot as…" }).click();
  expect((await download).suggestedFilename()).toBe("site.test.png");
  await noSeriousViolations(page, '[role="tabpanel"]');
});

// spec: UI-039
test("Rendered page: without JavaScript rendering the tab explains the prerequisite", async ({ page }) => {
  await open(page, { url: "1", cfg: JSON.stringify({ rendering: { mode: "text" } }) });
  await tab(page, "rendered", "Rendered Page");
  await expect(page.locator('[data-empty-state="NEEDS_JS_RENDERING"]')).toBeVisible();
  await expect(page.getByRole("button", { name: "Open setting" })).toBeVisible();
});

// spec: UI-040
// spec: TECH-114
test("Console log: level filter, long message handling and export entry points", async ({ page }) => {
  await open(page, { url: "1" });
  await tab(page, "consoleLog", "Chrome Console Log");
  const table = grid(page, "Chrome Console Log");
  await expect(table.locator("canvas").first()).toBeVisible();
  await expect.poll(async () => (await calls(page, "view.rows")).length).toBeGreaterThan(0);
  await page.getByRole("button", { name: /Level/ }).click(); await page.getByRole("option", { name: "Error" }).click();
  await expect.poll(async () => JSON.stringify((await calls(page, "view.open")).at(-1))).toContain('"level":"error"');
  await table.focus(); await table.press("Home");
  await expect(page.getByRole("region", { name: "Message" })).toContainText("Uncaught Error: fixture-error");
  await expect(page.getByRole("button", { name: "Export all pages with JavaScript issues" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Console message summary of all pages" })).toBeEnabled();
  await page.getByRole("button", { name: "Export all pages with JavaScript issues" }).click();
  expect(await page.evaluate(() => (window as unknown as { exportDialog: { getState: () => { request: unknown } } }).exportDialog.getState().request)).toEqual({ kind: "bulk", name: "JavaScript:Pages With JavaScript Issues" });
  await noSeriousViolations(page, '[role="tabpanel"]');
});

// spec: UI-041
test("View source: two code views, line diff with margin marks, F7 navigation and search", async ({ page }) => {
  await open(page, { url: "1" });
  await tab(page, "source", "View Source");
  const left = page.locator('[data-side="left"] .cm-content'); const right = page.locator('[data-side="right"] .cm-content');
  await expect(left).toContainText("<title>Start</title>"); await expect(right).toContainText("Start - rendered");
  await pick(page, "checkbox", "Show differences");
  await expect(page.getByTestId("diff-summary")).toHaveText("+2 / −1 lines");
  await expect(page.locator('[data-side="right"] .os-src-line-added')).toHaveCount(2);
  await expect(page.locator('[data-side="left"] .os-src-marker-removed:visible')).toHaveText("−");
  await page.keyboard.press("F7");
  await expect(page.getByTestId("change-position")).toHaveText("Difference 1 of 2");
  await page.keyboard.press("F7"); await expect(page.getByTestId("change-position")).toHaveText("Difference 2 of 2");
  await page.keyboard.press("Shift+F7"); await expect(page.getByTestId("change-position")).toHaveText("Difference 1 of 2");
  const box = page.getByRole("searchbox", { name: "Search in the active side" });
  await box.fill("title"); await expect(page.getByTestId("match-count")).toHaveText("Match 1 of 2");
  await box.press("Enter"); await expect(page.getByTestId("match-count")).toHaveText("Match 2 of 2");
  await noSeriousViolations(page, '[role="tabpanel"]');
});

// spec: UI-041
test("View source: visible text compares word by word and a large document stays virtualized", async ({ page }) => {
  await open(page, { url: "1" });
  await tab(page, "source", "View Source");
  await pick(page, "radio", "Visible text");
  await pick(page, "checkbox", "Show differences");
  await expect(page.getByTestId("diff-summary")).toHaveText("+4 / −0 words");
  await expect(page.locator('[data-side="right"] .os-src-word-added')).toHaveCount(4);
  await page.goto(`${SCENE}?url=1&large=20000`);
  await tab(page, "source", "View Source");
  await expect(page.locator('[data-side="left"]')).toContainText("20,000 lines", { timeout: 20_000 });
  await expect.poll(async () => (await calls(page, "detail.sourceLines")).length, { timeout: 20_000 }).toBe(10);
  expect(await page.locator('[data-side="left"] .cm-line').count()).toBeLessThan(400);
});

// spec: UI-044
// spec: ONPAGE-145
test("Duplicate details: exact and near lists, word diff with margin marks and prerequisite states", async ({ page }) => {
  await open(page, { url: "20" });
  await tab(page, "duplicates", "Duplicate Details");
  await pick(page, "radio", "Near");
  await expect(page.locator('[data-empty-state="NEEDS_CRAWL_ANALYSIS(near_duplicates)"]')).toBeVisible();
  await expect(page.getByRole("radio", { name: "Semantic" })).toBeDisabled();
  await page.goto(`${SCENE}?url=10`);
  await tab(page, "duplicates", "Duplicate Details");
  const list = page.getByRole("grid", { name: "Duplicate partners" });
  await expect(list.getByRole("row")).toHaveCount(3);
  await expect(list.getByRole("row").nth(1)).toContainText("https://site.test/a?ref=x");
  await list.getByRole("row").nth(2).click();
  await expect(page.locator(".os-dup-text")).toHaveCount(2);
  await expect(page.getByText("0 words only on this page, 0 words only on the partner page.")).toBeVisible();
  await noSeriousViolations(page, '[role="tabpanel"]');
});

// spec: UI-045
// spec: TECH-189
test("Structured data details: tree with symbols, parse error node and finding filter", async ({ page }) => {
  await open(page, { url: "1" });
  await tab(page, "structuredData", "Structured Data Details");
  const tree = page.getByRole("treegrid", { name: "Structured data items" });
  await expect(tree.getByRole("row").first()).toContainText("JSON-LD");
  await expect(tree).toContainText("Parse Error (3:7)");
  await expect(tree.getByRole("row", { name: /^Product/ }).getByRole("img", { name: "Error" })).toBeVisible();
  const findings = page.getByRole("table", { name: "Validation messages" });
  await expect(findings.getByRole("row")).toHaveCount(4);
  await tree.getByRole("row", { name: /^brand/ }).click();
  await expect(findings.getByRole("row")).toHaveCount(2);
  await tree.getByRole("row", { name: /Parse Error/ }).click();
  await expect(page.getByText("line 3, column 7")).toBeVisible();
  await page.getByRole("button", { name: "Show all findings" }).click();
  await findings.getByRole("row").filter({ hasText: "priceCurrency" }).click();
  await expect(tree.getByRole("row", { selected: true })).toContainText("Offer");
  await noSeriousViolations(page, '[role="tabpanel"]');
});

// spec: UI-047
// spec: ONPAGE-157
test("Spelling and grammar details: default columns, excerpt with all occurrences marked, context actions", async ({ page }) => {
  await open(page, { url: "1" });
  await tab(page, "spelling", "Spelling & Grammar Details");
  const table = grid(page, "Spelling & Grammar Details");
  await expect(table.locator("canvas").first()).toBeVisible();
  await expect.poll(async () => JSON.stringify((await calls(page, "view.open")).at(-1))).toContain('"columns":["Error","Type","Detail","Suggestion","Page Section","Rule ID"]');
  await table.focus(); await table.press("Home"); await table.press("ArrowDown");
  await expect.poll(async () => JSON.stringify(await calls(page, "detail.spellingContext"))).toContain('"findingId":2');
  await expect(page.locator("mark")).toHaveCount(2);
  await expect(page.locator("mark").first()).toHaveAttribute("data-first", "true");
  await table.press("Shift+F10");
  const menu = page.getByRole("menu", { name: "Detail actions" });
  await expect(menu.getByRole("menuitem").filter({ hasNotText: /^(✓ )?(Error|Type|Detail|Suggestion|Page Section|Rule ID)$/ })).toHaveText(["Copy cell", "Copy row", "Ignore (all)", "Ignore rule", "Add to dictionary", "Export details"]);
  await expect(menu.getByRole("menuitem", { name: "Ignore rule" })).toHaveAttribute("aria-disabled", "true");
  await noSeriousViolations(page, '[role="tabpanel"]');
});

// spec: UI-047
test("Spelling and grammar details: the prerequisite state names the setting", async ({ page }) => {
  await open(page, { url: "1", cfg: JSON.stringify({ spelling: { enabled: false } }) });
  await tab(page, "spelling", "Spelling & Grammar Details");
  await expect(page.locator('[data-empty-state="FEATURE_DISABLED(spelling.enabled)"]')).toBeVisible();
});

// spec: UI-048
// spec: TECH-259
test("Accessibility details: result switch, impact order, affected elements and the jump to the rendered source", async ({ page }) => {
  await open(page, { url: "1" });
  await tab(page, "a11y", "Accessibility Details");
  await expect(page.getByRole("radio", { name: "Violation" })).toBeChecked();
  const rules = grid(page, "Rules"); await expect(rules.locator("canvas").first()).toBeVisible();
  expect((await calls(page, "view.open")).at(-1)).toMatchObject({ spec: { detail: "a11yRules", params: { result: "Violation" }, sort: [{ column: "Impact", dir: "asc" }, { column: "Nodes", dir: "desc" }, { column: "Issue", dir: "asc" }] } });
  await rules.focus(); await rules.press("Home");
  const nodes = grid(page, "Affected elements"); await expect(nodes.locator("canvas").first()).toBeVisible();
  expect((await calls(page, "view.open")).at(-1)).toMatchObject({ spec: { detail: "a11yNodes", params: { rule: "image-alt", result: "Violation" } } });
  await pick(page, "radio", "Passed");
  await rules.focus(); await rules.press("Home");
  await expect(page.getByText("No elements with findings")).toBeVisible();
  await pick(page, "radio", "Violation");
  await rules.focus(); await rules.press("Home"); await rules.press("ArrowDown");
  await expect.poll(async () => JSON.stringify((await calls(page, "view.open")).at(-1))).toContain('"rule":"heading-order"');
  await nodes.focus(); await nodes.press("Home"); await nodes.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Show in rendered HTML" }).click();
  await expect(page.getByRole("tab", { name: "View Source" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByText("Marked h1 in the rendered HTML.")).toBeVisible();
});

// spec: UI-048
test("Accessibility details: rank-ordered prerequisite states", async ({ page }) => {
  await open(page, { url: "1", cfg: JSON.stringify({ extraction: { accessibility: false } }) });
  await tab(page, "a11y", "Accessibility Details");
  await expect(page.locator('[data-empty-state="FEATURE_DISABLED(accessibility)"]')).toBeVisible();
});

// spec: UI-126
test("the technical tabs have no serious axe violations in the dark theme", async ({ page }) => {
  await open(page, { url: "1", dark: "1" });
  for (const [id, name] of [["serp", "SERP Snippet"], ["source", "View Source"], ["structuredData", "Structured Data Details"]] as const) {
    await tab(page, id, name);
    await expect(page.getByRole("tabpanel", { name })).toBeVisible();
    await noSeriousViolations(page, '[role="tabpanel"]');
  }
});
