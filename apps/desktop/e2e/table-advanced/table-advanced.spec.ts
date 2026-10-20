import { test, expect, type Page } from "@playwright/test";
const fixture = "/src/features/table-advanced/fixtures/u20.html";
test.beforeEach(async ({ page }) => {
  await page.clock.install({ time: new Date("2026-10-18T10:00:00Z") });
  await page.goto(fixture);
  await expect(page.getByText("Showing: 5 of 5", { exact: true })).toBeVisible();
});
const lastSearch = (page: Page) => page.evaluate(() => {
  const calls = window.__osMock!.calls.filter(c => c.cmd === "view.open");
  return (calls.at(-1)?.args as { spec: { search?: unknown } }).spec.search ?? null;
});
// The full App also renders the sidebar tabs; the top tab bar is the "Data views" region.
const topTabs = (page: Page) => page.getByRole("navigation", { name: "Data views", exact: true }).getByRole("tab");
const panel = (page: Page) => page.getByRole("region", { name: "Advanced search" });

// spec: UI-011
// spec: UI-011 [P1]
test("plain Address search, case switch, regex with back reference and the rule „Word Count < 50 OR Status Code = 404“", async ({ page }) => {
  const field = page.getByLabel("Search Address", { exact: true });
  await field.fill("blog"); await field.press("Enter");
  await expect(page.getByText(/Showing: 3 of 5.*Search active/)).toBeVisible();
  await page.getByRole("button", { name: "Advanced search", exact: true }).click();
  await expect(panel(page).getByRole("checkbox", { name: "Case sensitive" })).not.toBeChecked();
  await panel(page).getByLabel("Value", { exact: true }).fill("Blog");
  await panel(page).getByText("Case sensitive", { exact: true }).click();
  await panel(page).getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByText(/Showing: 1 of 5/)).toBeVisible();
  await panel(page).getByLabel("Operator", { exact: true }).selectOption("regex");
  await panel(page).getByLabel("Value", { exact: true }).fill("^.*/blog/");
  await panel(page).getByRole("button", { name: "Search", exact: true }).click();
  await expect.poll(() => lastSearch(page)).toMatchObject({ caseSensitive: true, tree: { children: [{ op: "regex", value: "^.*/blog/" }] } });
  await expect(page.getByText(/Showing: 1 of 5/)).toBeVisible();
  await panel(page).getByLabel("Value", { exact: true }).fill("/([^/]+)/\\1/");
  await panel(page).getByRole("button", { name: "Search", exact: true }).click();
  await expect.poll(() => lastSearch(page)).toMatchObject({ tree: { children: [{ op: "regex", value: "/([^/]+)/\\1/" }] } });
  await expect(page.getByText(/Showing: 1 of 5/)).toBeVisible();
  const syntax = page.getByLabel("Search query (text syntax)", { exact: true });
  await syntax.fill('"Word Count" < 50 OR "Status Code" = 404'); await syntax.press("Enter");
  await expect(page.getByText(/Showing: 3 of 5.*Search active/)).toBeVisible();
  await expect.poll(() => lastSearch(page)).toEqual({ kind: "rules", caseSensitive: true, tree: { kind: "group", op: "or", children: [
    { kind: "cond", column: "Word Count", op: "lessThan", value: "50" }, { kind: "cond", column: "Status Code", op: "equals", value: "404" }] } });
  await expect(panel(page).getByLabel("Column", { exact: true })).toHaveCount(2);
  await page.getByLabel("Recent searches", { exact: true }).selectOption({ index: 1 });
  await expect(panel(page).getByLabel("Query", { exact: true })).toHaveValue('"Word Count" < "50" OR "Status Code" = "404"');
});

// spec: UI-011 [P1]
test("invalid regex and syntax errors colour the field and keep the current view", async ({ page }) => {
  await page.getByRole("button", { name: "Advanced search", exact: true }).click();
  await panel(page).getByLabel("Operator", { exact: true }).selectOption("regex");
  const value = panel(page).getByLabel("Value", { exact: true });
  await value.fill("(unclosed");
  const opens = await page.evaluate(() => window.__osMock!.calls.filter(c => c.cmd === "view.open").length);
  await panel(page).getByRole("button", { name: "Search", exact: true }).click();
  await expect(panel(page).getByRole("alert")).toContainText("Invalid regular expression");
  await expect(value).toHaveAttribute("aria-invalid", "true");
  const syntax = page.getByLabel("Search query (text syntax)", { exact: true });
  await syntax.fill("Address ~"); await syntax.press("Enter");
  await expect(page.getByText("Query syntax error at character 9", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__osMock!.calls.filter(c => c.cmd === "view.open").length)).toBe(opens);
  await expect(page.getByText("Showing: 5 of 5", { exact: true })).toBeVisible();
});

// spec: UI-009
test("configures 60+ columns with two lists and applies template „Basis“ to External with a missing-column notice", async ({ page }) => {
  await page.getByRole("button", { name: "Configure columns…", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Configure columns" });
  const available = dialog.getByRole("listbox", { name: "Available columns" });
  const displayed = dialog.getByRole("listbox", { name: "Displayed columns" });
  expect(await available.getByRole("option").count() + await displayed.getByRole("option").count()).toBeGreaterThanOrEqual(60);
  await dialog.getByLabel("Find columns", { exact: true }).fill("metric 0");
  await displayed.getByRole("option", { name: "Metric 03" }).click();
  await displayed.getByRole("option", { name: "Metric 04" }).click({ modifiers: ["Shift"] });
  await dialog.getByRole("button", { name: "Remove", exact: true }).click();
  await dialog.getByLabel("Find columns", { exact: true }).fill("");
  await expect(displayed.getByRole("option")).toHaveText(["Address", "Status Code", "Word Count", "Title 1", "Segment", "Metric 01", "Metric 02"]);
  await available.getByRole("option", { name: "Metric 10" }).click();
  await dialog.getByRole("button", { name: "Add", exact: true }).click();
  await expect(displayed.getByRole("option")).toHaveCount(8);
  await dialog.getByLabel("Template name", { exact: true }).fill("Basis");
  await dialog.getByRole("button", { name: "Save as template", exact: true }).click();
  await dialog.getByLabel("Apply to tab", { exact: true }).selectOption("External");
  await dialog.getByRole("button", { name: "Apply template to tab", exact: true }).click();
  await expect(dialog.getByRole("status")).toContainText("Applied to External; 6 columns are not available there and were skipped");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("button", { name: "Configure columns…", exact: true }).click();
  await expect(page.getByRole("listbox", { name: "Displayed columns" }).getByRole("option")).toHaveCount(9);
});

// spec: UI-112
// spec: UI-004 [P1]
test("tab context menu closes all but Internal; the dropdown shows closed tabs again; the dialog survives a restart", async ({ page }) => {
  await page.getByRole("tab", { name: "Images", exact: true }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Close all tabs", exact: true }).click();
  await expect(topTabs(page)).toHaveText(["Internal"]);
  await page.getByRole("tab", { name: "Internal", exact: true }).focus();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menuitem", { name: "Close tab", exact: true })).toBeDisabled();
  await expect(page.getByRole("menuitem", { name: "Configure tabs…", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu", { name: "Tab actions" })).toHaveCount(0);
  await page.getByRole("button", { name: "Configure tabs", exact: true }).click();
  await page.getByRole("menuitemcheckbox", { name: "AMP", exact: true }).click();
  await page.mouse.click(640, 790);
  await expect(topTabs(page)).toHaveText(["Internal", "AMP"]);
  await page.getByRole("tab", { name: "AMP", exact: true }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Configure tabs…", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Configure tabs" });
  await expect(dialog.getByRole("checkbox", { name: "Internal" })).toBeDisabled();
  await dialog.getByRole("checkbox", { name: "External" }).check();
  await dialog.getByRole("button", { name: "Move AMP up", exact: true }).click();
  await dialog.getByRole("button", { name: "Move AMP up", exact: true }).click();
  await dialog.getByRole("button", { name: "OK", exact: true }).click();
  await expect(topTabs(page)).toHaveText(["Internal", "AMP", "External"]);
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem("u20.fixture.uiState") ?? "{}").state?.topTabs?.order?.slice(0, 3))).toEqual(["Internal", "AMP", "External"]);
  await page.reload();
  await expect(topTabs(page)).toHaveText(["Internal", "AMP", "External"]);
});

// spec: UI-021
test("resets the columns of all tables after confirmation and keeps the template and tab order across a restart", async ({ page }) => {
  await page.getByRole("button", { name: "Configure columns…", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Configure columns" });
  await dialog.getByRole("listbox", { name: "Displayed columns" }).getByRole("option", { name: "Word Count" }).click();
  await dialog.getByRole("button", { name: "Remove", exact: true }).click();
  await dialog.getByLabel("Template name", { exact: true }).fill("Basis");
  await dialog.getByRole("button", { name: "Save as template", exact: true }).click();
  await dialog.getByRole("button", { name: "OK", exact: true }).click();
  await page.getByRole("tab", { name: "Images", exact: true }).dragTo(page.getByRole("tab", { name: "Internal", exact: true }));
  await expect.poll(() => page.evaluate(() => localStorage.getItem("u20.fixture.uiState") ?? "")).toContain("Basis");
  // Header context menu of the grid (canvas header row), as a user reaches the action.
  await page.locator(".os-data-grid").click({ position: { x: 225, y: 12 }, button: "right" });
  await page.getByRole("menuitem", { name: "Reset columns of all tables", exact: true }).click();
  const confirm = page.getByRole("dialog", { name: "Reset columns of all tables" });
  await expect(confirm).toBeVisible();
  const reset = confirm.getByRole("button", { name: "Reset", exact: true });
  await reset.focus();
  await page.keyboard.press("Enter");
  await expect(confirm).toBeHidden();
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem("u20.fixture.uiState") ?? "{}").state?.tableColumns)).toBeUndefined();
  await page.reload();
  await expect(topTabs(page).first()).toHaveText("Images");
  await page.getByRole("button", { name: "Configure columns…", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Configure columns" }).getByRole("listbox", { name: "Displayed columns" }).getByRole("option", { name: "Word Count" })).toBeVisible();
  await expect(page.getByLabel("Template", { exact: true }).getByRole("option", { name: "Basis" })).toHaveCount(1);
});

// spec: UI-020
test("segment filter narrows table and counts and shows a badge", async ({ page }) => {
  await page.getByLabel("Segment", { exact: true }).selectOption({ label: "Blog" });
  await expect(page.getByText(/Showing: 1 of 1/)).toBeVisible();
  await expect(page.getByRole("status", { name: "Segment filter: Blog" })).toBeVisible();
  await page.getByLabel("Segment", { exact: true }).selectOption({ label: "No segment" });
  await expect(page.getByText(/Showing: 2 of 2/)).toBeVisible();
  await page.getByLabel("Segment", { exact: true }).selectOption({ label: "All segments" });
  await expect(page.getByText("Showing: 5 of 5", { exact: true })).toBeVisible();
  await expect(page.getByRole("status", { name: /Segment filter/ })).toHaveCount(0);
});

// spec: UI-110 [P1]
test("row context menu enables single-URL entries for one row and disables them for three", async ({ page }) => {
  const canvas = page.locator(".os-data-grid");
  await canvas.click({ position: { x: 225, y: 43 } });
  await expect(page.getByText(/1 selected/)).toBeVisible();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menuitem", { name: "Crawl tree", exact: true })).toBeEnabled();
  await expect(page.getByRole("menuitem", { name: "Filter by this URL's segment", exact: true })).toBeEnabled();
  await expect(page.getByRole("menuitem", { name: "Show crawl path", exact: true })).toBeDisabled();
  // The long menu may reach past the viewport; scrolling would dismiss the popover, so the item is pressed in place.
  await page.getByRole("menuitem", { name: "Crawl tree", exact: true }).dispatchEvent("click");
  await expect.poll(() => page.evaluate(() => window.__osMock!.calls.find(c => c.cmd === "viz.open")?.args)).toEqual({ kind: "crawlTree", rootUrlId: 1 });
  await canvas.click({ position: { x: 225, y: 43 } });
  await page.keyboard.press("Shift+ArrowDown"); await page.keyboard.press("Shift+ArrowDown");
  await expect(page.getByText(/3 selected/)).toBeVisible();
  await page.keyboard.press("Shift+F10");
  const tree = page.getByRole("menuitem", { name: "Crawl tree", exact: true });
  await expect(tree).toBeDisabled();
  await expect(page.getByRole("menuitem", { name: "Show crawl path", exact: true })).toBeDisabled();
  await expect(page.getByRole("menuitem", { name: "Copy URLs", exact: true })).toBeEnabled();
});
