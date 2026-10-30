import { shortcut, test, expect, scan, selectFirstRow, tabTo, BARS } from "./fixture";

test.use({ locale: "en-US" });
test.setTimeout(240_000);

for (const theme of ["light", "dark"] as const) {
  // spec: UI-126
  test(`axe main, populated grid, filter, detail, sidebar and three overflow menus (${theme})`, async ({ page, audit }) => {
    await page.emulateMedia({ colorScheme: theme });
    await audit.crawl();
    await selectFirstRow(page);
    await scan(page, `main-${theme}`);
    // Visit every detail/sidebar panel, including overflowed tabs, through the tablist keyboard contract.
    for (const name of BARS.slice(1)) {
      const list = page.getByRole("tablist", { name, exact: true });
      const ids = await list.locator("[data-tab-id]").evaluateAll(tabs => tabs.map(t => t.getAttribute("data-tab-id")));
      await tabTo(page, list.getByRole("tab", { selected: true }));
      await page.keyboard.press("Home");
      for (const id of ids) {
        await expect(list.getByRole("tab", { selected: true })).toHaveAttribute("data-tab-id", id!);
        await scan(page, `${name}-${id}-${theme}`, name === "Detail tabs" ? "#shell-detail [role=tabpanel]" : "#shell-sidebar [role=tabpanel]");
        await tabTo(page, list.getByRole("tab", { selected: true }));
        await page.keyboard.press("ArrowRight");
      }
      await page.keyboard.press("Home");
    }
    const filter = page.locator(".u10-toolbar .os-combobox-button");
    await filter.press("Enter");
    await expect(page.getByRole("listbox")).toBeVisible();
    await scan(page, `filter-${theme}`);
    await page.keyboard.press("Escape");
    for (const name of BARS) {
      const root = page.getByRole("tablist", { name, exact: true }).locator("..");
      await root.getByRole("button").press("Enter");
      await expect(page.getByRole("menu")).toBeVisible();
      await scan(page, `${name}-overflow-${theme}`);
      await page.keyboard.press("Escape");
    }
    const view = theme === "light" ? "Crawl summary-issues-light" : "Crawl summary-segments-dark";
    if (test.info().errors.every(error => error.message?.startsWith(`Error: ${view}\n`))) {
      test.fail(true, `${theme === "light" ? "U30-F03" : "U30-F04"}: known contrast failure until N12`);
    }
  });

  // spec: UI-126
  test(`axe configuration, export and settings sections (${theme})`, async ({ page, context, audit }) => {
    await page.emulateMedia({ colorScheme: theme });
    await shortcut(page, "k");
    await expect(page.getByRole("dialog", { name: "Crawl configuration" })).toBeVisible();
    await scan(page, `configuration-${theme}`);
    await page.keyboard.press("Escape");
    await audit.crawl();
    await tabTo(page, page.locator("main .os-data-grid"));
    await shortcut(page, "e");
    await expect(page.getByRole("dialog", { name: "Export options" })).toBeVisible();
    await scan(page, `export-${theme}`);
    await page.keyboard.press("Escape");
    const settings = await context.newPage();
    await settings.emulateMedia({ colorScheme: theme });
    await settings.goto(`/settings.html?bridge=${encodeURIComponent(audit.bridge)}`);
    const tabs = settings.getByRole("tab");
    await expect(tabs.first()).toBeVisible();
    const count = await tabs.count();
    await tabTo(settings, settings.getByRole("tab", { selected: true }));
    await settings.keyboard.press("Home");
    for (let i = 0; i < count; i++) {
      await expect(tabs.nth(i)).toHaveAttribute("aria-selected", "true");
      await scan(settings, `settings-${i}-${theme}`);
      if (i + 1 < count) await settings.keyboard.press("ArrowDown");
    }
  });
}

// spec: UI-126
test("axe populated comparison, setup dialog and comparison detail", async ({ page, audit }) => {
  await audit.crawl();
  await audit.invoke("crawl.clear");
  await expect(page.getByTestId("crawl-phase")).toHaveText("Ready");
  // Seed the second crawl through the fixture API; Enter on the populated URL history selects a suggestion first.
  await audit.invoke("crawl.start", { source: { kind: "spider", url: `${audit.url}?version=2` }, scope: "domain" });
  await expect(page.getByTestId("crawl-phase")).toHaveText("Completed", { timeout: 30_000 });
  await expect(page.locator("main .os-data-grid")).toHaveAttribute("aria-rowcount", "4");
  const compare = page.getByRole("button", { name: "Compare crawls", exact: true });
  await tabTo(page, compare);
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Compare crawls", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Compare", exact: true })).toBeEnabled();
  await scan(page, "comparison-setup");
  await dialog.getByRole("button", { name: "Compare", exact: true }).press("Enter");
  const grid = page.getByRole("grid", { name: "Comparison table", exact: true });
  await expect(grid).toBeVisible({ timeout: 30_000 });
  await grid.press("Home");
  await grid.press("ArrowDown");
  await scan(page, "comparison");
});
