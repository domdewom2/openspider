import { shortcut, test, expect, tabTo, visibleFocus, selectFirstRow, BARS } from "./fixture";

test.use({ locale: "en-US" });
test.setTimeout(120_000);

for (const theme of ["light", "dark"] as const) {
  // spec: UI-126
  test(`three tablists: roving focus, every tab, overflow actions and return (${theme})`, async ({ page, audit }) => {
    await page.emulateMedia({ colorScheme: theme });
    await audit.crawl();
    await selectFirstRow(page);
    for (const name of BARS) {
      const list = page.getByRole("tablist", { name, exact: true });
      const selected = list.getByRole("tab", { selected: true });
      await expect(list.locator('[role=tab][tabindex="0"]')).toHaveCount(1);
      await tabTo(page, selected);
      await visibleFocus(selected);
      const ids = await list.locator("[role=tab]").evaluateAll(tabs => tabs.map(t => t.getAttribute("data-tab-id")));
      await page.keyboard.press("Home");
      for (const id of ids) {
        const tab = list.locator(`[data-tab-id="${id}"]`);
        await expect(tab).toBeFocused();
        await expect(tab).toHaveAttribute("aria-selected", "true");
        await expect(tab).toBeVisible();
        await visibleFocus(tab);
        await page.keyboard.press("ArrowRight");
      }
      await page.keyboard.press("End");
      await expect(list.getByRole("tab", { selected: true })).toHaveAttribute("data-tab-id", ids.at(-1)!);
      await page.keyboard.press("Home");
      if (ids.length > 1) {
        await page.keyboard.press("Alt+Shift+ArrowRight");
        await expect(list.locator("[data-tab-id]").first()).toHaveAttribute("data-tab-id", ids[1]!);
      }
      const trigger = list.locator("..").getByRole("button");
      await tabTo(page, trigger);
      await visibleFocus(trigger);
      await page.keyboard.press("Enter");
      const menu = page.getByRole("menu");
      await expect(menu).toBeVisible();
      await page.keyboard.press("End");
      await expect(menu.getByRole("menuitem", { name: "Reset tabs", exact: true })).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(menu).toHaveCount(0);
      await expect(trigger).toBeFocused();
      await expect(list.locator("[data-tab-id]").first()).toHaveAttribute("data-tab-id", ids[0]!);
      await page.keyboard.press("Enter");
      await page.keyboard.press("End");
      await page.keyboard.press("ArrowUp");
      await expect(menu.getByRole("menuitem", { name: "Configure tabs…", exact: true })).toBeFocused();
      await page.keyboard.press("Enter");
      const configuration = page.getByRole("dialog", { name: "Configure tabs", exact: true });
      await expect(configuration).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(configuration).toHaveCount(0);
      await expect(trigger).toBeFocused();
      await page.keyboard.press("Shift+Tab");
      await expect(list.getByRole("tab", { selected: true })).toBeFocused();
      await page.keyboard.press("Shift+Tab");
      await expect(list.getByRole("tab", { selected: true })).not.toBeFocused();
    }
  });
}

// spec: UI-126
test("grid total/index, keyboard selection, sorting, context action and filter search", async ({ page, audit }) => {
  await audit.crawl();
  const grid = page.locator("main .os-data-grid");
  await expect(grid).toHaveAttribute("aria-rowcount", "4");
  await selectFirstRow(page);
  await visibleFocus(grid);
  await grid.press("End");
  await expect(grid.getByRole("row", { selected: true })).toHaveAttribute("aria-rowindex", "4");
  await grid.press("Home");
  await grid.press("ArrowUp");
  await grid.press("Enter");
  await expect(grid.getByRole("columnheader").first()).toHaveAttribute("aria-sort", "ascending");
  await grid.press("ArrowDown");
  await grid.press("Shift+F10");
  await expect(page.getByRole("menu")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(grid).toBeFocused();
  const search = page.getByRole("searchbox", { name: "Search Address", exact: true });
  await tabTo(page, search);
  await search.pressSequentially("no-such-page");
  await search.press("Enter");
  await expect(grid).toHaveAttribute("aria-rowcount", "1");
  await search.press("Escape");
  await expect(grid).toHaveAttribute("aria-rowcount", "4");
  const filter = page.locator(".u10-toolbar .os-combobox-button");
  await tabTo(page, filter, true);
  await page.keyboard.press("Enter");
  await expect(page.getByRole("listbox")).toBeVisible();
  await page.keyboard.press("End");
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  await expect(filter).toBeFocused();
});

// spec: UI-126
test("configuration and export trap modal focus, Escape restores the trigger", async ({ page, audit }) => {
  await audit.crawl();
  for (const [key, title] of [["k", "Crawl configuration"], ["e", "Export options"]]) {
    const origin = page.locator("main .os-data-grid");
    await tabTo(page, origin);
    await shortcut(page, key!);
    const dialog = page.getByRole("dialog", { name: title, exact: true });
    await expect(dialog).toBeVisible();
    const count = await dialog.locator('button, input, textarea, select, [tabindex="0"]').count();
    for (const key of ["Tab", "Shift+Tab"]) {
      for (let i = 0; i < count + 2; i++) {
        await page.keyboard.press(key);
        expect(await dialog.evaluate(el => el.contains(document.activeElement))).toBe(true);
      }
    }
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(origin).toBeFocused();
  }
});

// spec: UI-126
test("complete main-window Tab inventory includes detail, sidebar and every overview action", async ({ page, audit }) => {
  const { keyboardInventory } = await import("./fixture");
  await audit.crawl();
  await selectFirstRow(page);
  await keyboardInventory(page);
});

// spec: UI-126
test("all settings sections expose their enabled controls in the sequential keyboard order", async ({ audit, context }) => {
  const { keyboardInventory } = await import("./fixture");
  const page = await context.newPage();
  await page.goto(`/settings.html?bridge=${encodeURIComponent(audit.bridge)}`);
  const tabs = page.getByRole("tab");
  await expect(tabs.first()).toBeVisible();
  const count = await tabs.count();
  await tabTo(page, page.getByRole("tab", { selected: true }));
  await page.keyboard.press("Home");
  for (let i = 0; i < count; i++) {
    await expect(tabs.nth(i)).toHaveAttribute("aria-selected", "true");
    await keyboardInventory(page);
    await tabTo(page, tabs.nth(i));
    if (i + 1 < count) await page.keyboard.press("ArrowDown");
  }
});
