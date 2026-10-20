import { expect, test, type Page } from "@playwright/test";

async function tab(page: Page, id: string, name: string) {
  const target = page.locator(`[data-tab-id="${id}"]`);
  if (await target.isVisible()) await target.click();
  else { await page.getByRole("button", { name: "More detail tabs" }).click(); await page.getByRole("menuitem", { name, exact: true }).click(); }
}
const calls = (page: Page, cmd: string) => page.evaluate(cmd => (window as unknown as { detailFixture: { core: { calls: { cmd: string; args: unknown }[] } } }).detailFixture.core.calls.filter(c => c.cmd === cmd).map(c => c.args), cmd);

// spec: UI-030
test("only the visible detail tab loads data and reacts to multi-selection", async ({ page }) => {
  await page.goto("/e2e/detail/index.html");
  await expect(page.locator('[data-empty-state="NO_SELECTION"]')).toBeVisible();
  expect(await calls(page, "detail.urlDetails")).toEqual([]);
  await page.getByRole("button", { name: "Select three URLs" }).click();
  await expect(page.getByRole("rowheader", { name: "Link Score" })).toBeVisible();
  expect(await calls(page, "detail.urlDetails")).toEqual([{ urlId: 2 }]); expect(await calls(page, "view.open")).toEqual([]);
  await tab(page, "inlinks", "Inlinks"); await expect(page.getByLabel("Inlinks", { exact: true }).locator("canvas").first()).toBeVisible();
  await expect.poll(() => calls(page, "view.rows")).not.toEqual([]);
  expect(await calls(page, "view.open")).toEqual([expect.objectContaining({ spec: expect.objectContaining({ kind: "inlinks", urlIds: [1, 2, 3] }) })]);
  await page.getByRole("button", { name: "Clear selection" }).click(); await expect(page.locator('[data-empty-state="NO_SELECTION"]')).toBeVisible();
  await expect.poll(() => calls(page, "view.close")).not.toEqual([]);
});

// spec: TECH-146
test("Anchors uses core aggregation, default exclusions and exact drill-down", async ({ page }) => {
  await page.goto("/e2e/detail/index.html"); await page.getByRole("button", { name: "Select one URL" }).click(); await tab(page, "inlinks", "Inlinks");
  await page.getByRole("button", { name: /View/ }).click(); await page.getByRole("option", { name: "Anchors", exact: true }).click();
  await expect.poll(async () => JSON.stringify(await calls(page, "view.open"))).toContain('"mode":"anchors"');
  const request = (await calls(page, "view.open")).at(-1) as { spec: { filter: unknown; sort: unknown } };
  expect(request.spec.filter).toMatchObject({ mode: "anchors", excludeSelf: true, excludeNofollow: false });
  expect(request.spec.sort).toEqual([{ column: "Links", dir: "desc" }, { column: "Anchor Text", dir: "asc" }]);
  await expect.poll(() => calls(page, "view.rows")).not.toEqual([]);
  const canvas = page.getByLabel("Inlinks", { exact: true }).locator("canvas").first();
  await expect(canvas).toBeVisible();
  // Keyboard focus lives on the grid element (the canvas is decorative in the shared grid).
  const grid = page.getByRole("grid", { name: "Inlinks", exact: true });
  await grid.focus();
  // Selecting the first row already activates the Anchors drill-down.
  await grid.press("Home");
  await expect.poll(async () => JSON.stringify(await calls(page, "view.open"))).toContain('"anchor":"Produkte"');
});

// spec: INTEG-084
test("custom link position filters are sent independently of search", async ({ page }) => {
  await page.goto("/e2e/detail/index.html"); await page.getByRole("button", { name: "Select one URL" }).click(); await tab(page, "outlinks", "Outlinks");
  await page.getByRole("button", { name: /Link Position/ }).click(); await page.getByRole("option", { name: "Editorial" }).click();
  await page.getByRole("searchbox").fill("outside");
  await expect.poll(async () => JSON.stringify(await calls(page, "view.open"))).toContain('"search":"outside"');
  const request = (await calls(page, "view.open")).at(-1) as { spec: unknown }; expect(request.spec).toMatchObject({ search: "outside", filter: { position: "Editorial" } });
});

// spec: UI-035
test("image preview requires consent before loading an unstored image", async ({ page }) => {
  const remote: string[] = []; page.on("request", request => { if (request.url().includes("site.test")) remote.push(request.url()); });
  await page.goto("/e2e/detail/index.html?stored=false"); await page.getByRole("button", { name: "Select image" }).click(); await tab(page, "images", "Image Details");
  await expect(page.getByRole("button", { name: "Load image from server…" })).toBeVisible(); expect(remote).toEqual([]);
  await expect(page.getByRole("img")).toHaveCount(0);
  await page.getByRole("button", { name: "Load image from server…" }).click(); await expect(page.getByRole("img")).toHaveAttribute("src", "ospider://img/fixture/7");
});

// spec: UI-042
test("headers preserve repeated fields and support case-insensitive search", async ({ page }) => {
  await page.goto("/e2e/detail/index.html"); await page.getByRole("button", { name: "Select three URLs" }).click(); await tab(page, "headers", "HTTP Headers");
  await expect(page.getByText("set-cookie", { exact: true })).toHaveCount(3);
  await page.getByRole("searchbox").fill("X-ROBOTS"); await expect(page.getByText("noindex", { exact: true })).toBeVisible(); await expect(page.getByText("set-cookie", { exact: true })).toHaveCount(0);
});

// spec: UI-145
test("one million link rows use bounded windows and real canvas virtualization", async ({ page }) => {
  await page.goto("/e2e/detail/index.html?large=1"); await page.getByRole("button", { name: "Select one URL" }).click(); await tab(page, "inlinks", "Inlinks");
  await expect(page.getByLabel("Inlinks", { exact: true }).locator("canvas").first()).toBeVisible();
  await expect.poll(() => calls(page, "view.rows")).not.toEqual([]);
  const reads = await calls(page, "view.rows") as { offset: number; limit: number }[];
  expect(reads.every(read => read.limit <= 200 && read.offset < 2000)).toBe(true);
  expect(reads.length).toBeLessThan(12);
});

// spec: OUT-023
test("crawl path export uses the selected URLs and the complete core export", async ({ page }) => {
  await page.goto("/e2e/detail/index.html"); await page.getByRole("button", { name: "Select three URLs" }).click();
  await page.getByRole("button", { name: "Crawl Path", exact: true }).click();
  await page.getByRole("button", { name: "Export details", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Export details", exact: true }); await dialog.getByRole("textbox", { name: "Destination file" }).fill("crawl-path.csv");
  await dialog.getByRole("button", { name: "Export details" }).click();
  await expect.poll(() => calls(page, "export.detail")).toEqual([expect.objectContaining({ spec: expect.objectContaining({ kind: "detail", detail: "crawlPath", urlIds: [1, 2, 3] }), selection: { kind: "ids", ids: [1, 2, 3] }, path: "crawl-path.csv", format: "csv" })]);
});
