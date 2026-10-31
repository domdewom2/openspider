import { test, expect } from "@playwright/test";
import { audit, prepare } from "../visual/support";
import { test as bridgeTest, selectFirstRow, scan, tabTo, visibleFocus } from "../a11y/fixture";
import { test as alphaTest } from "../alpha/fixture";

// spec: UI-126
test("ComboBox keeps input focus on listbox padding and selects by mouse", async ({ page }) => {
  await page.goto("/e2e/n12/fixture.html");
  const input = page.getByRole("combobox", { name: "Choices" });
  await input.press("ArrowDown");
  const list = page.getByRole("listbox");
  await expect(list).toBeVisible();
  await list.evaluate(el => { (el as HTMLElement).style.padding = "16px"; });
  const box = (await list.boundingBox())!;
  await page.mouse.click(box.x + 2, box.y + 2);
  await expect(input).toBeFocused();
  await expect(list).toBeVisible();
  await page.getByRole("option", { name: "Two" }).click();
  await expect(page.getByTestId("choice")).toHaveText("two");
  await expect(input).toBeFocused();
  await expect(list).toHaveCount(0);
});

// spec: UI-126
test("information survives removal of colour: issue names and diff signs", async ({ page }) => {
  await page.goto("/e2e/n12/fixture.html");
  await page.addStyleTag({ content: "* { color: black !important; background-color: white !important; border-color: black !important; box-shadow: none !important; }" });
  const badges = page.locator(".os-sidebar-badge");
  await expect(badges).toHaveCount(3);
  for (const [index, type] of ["Issue", "Warning", "Opportunity"].entries()) {
    await expect(badges.nth(index)).toContainText(type);
    await expect(badges.nth(index)).toContainText("Medium");
  }
  await expect(page.locator(".os-src-marker-added").last()).toHaveText("+");
  await expect(page.locator(".os-src-marker-removed")).toHaveText("−");
  await expect(page.locator(".os-src-marker-added").last()).toBeVisible();
  await expect(page.locator(".os-src-marker-removed")).toBeVisible();
});

// spec: UI-126
// spec: UI-001
test("all three tab bars link their selected tab to the displayed panel after keyboard changes", async ({ page }) => {
  await prepare(page, "en", "light");
  const boxes = await page.locator(".os-tab-label:visible").evaluateAll(labels => labels.map(el => {
    const style = getComputedStyle(el);
    return { line: parseFloat(style.lineHeight), height: el.getBoundingClientRect().height };
  }));
  expect(boxes.length).toBeGreaterThan(3);
  for (const box of boxes) {
    expect(Number.isInteger(box.line)).toBe(true);
    expect(box.height).toBe(box.line);
  }
  for (const list of await page.getByRole("tablist").all()) {
    for (const key of ["Home", "ArrowRight", "End", "Home"]) {
      const selected = list.getByRole("tab", { selected: true });
      await selected.press(key);
      const current = list.getByRole("tab", { selected: true });
      const panelId = await current.getAttribute("aria-controls");
      expect(panelId).toBeTruthy();
      const panel = page.locator(`[id="${panelId}"]`);
      await expect(panel).toBeVisible();
      await expect(panel).toHaveAttribute("role", "tabpanel");
      await expect(panel).toHaveAttribute("aria-labelledby", (await current.getAttribute("id"))!);
    }
  }
});

// spec: UI-001
test("text audit detects clipping ancestors and tolerates less than one device pixel", async ({ page }) => {
  await page.setContent(`<style>button { display:block; width:180px; height:24px; white-space:nowrap; margin:20px; } .clip { width:80px; overflow:hidden; } .clip button { margin:0; } </style>
    <div class="clip"><button>Ancestor clipping fixture</button></div>
    <button><span style="display:block;width:30px;overflow:hidden;white-space:nowrap">Nested clipping fixture</span></button>
    <button id="fraction">Subpixel</button><input value="Native input"><select><option>Native select</option></select>`);
  expect((await audit(page)).text.some(hit => hit.includes("Ancestor clipping fixture"))).toBe(true);
  expect((await audit(page)).text.some(hit => hit.includes("Nested clipping fixture"))).toBe(true);
  await page.locator("#fraction").evaluate(el => {
    const range = document.createRange(); range.selectNodeContents(el);
    const width = range.getBoundingClientRect().width;
    Object.assign((el as HTMLElement).style, { padding: "0", border: "0", width: `${width - 0.25 / devicePixelRatio}px`, overflow: "hidden" });
  });
  expect((await audit(page)).text.some(hit => hit.includes("Subpixel"))).toBe(false);
  await page.locator("#fraction").evaluate(el => { (el as HTMLElement).style.width = "4px"; });
  expect((await audit(page)).text.some(hit => hit.includes("Subpixel"))).toBe(true);
  expect((await audit(page)).text.some(hit => /Native (input|select)/.test(hit))).toBe(false);
});

// Use the real loopback crawl to verify the canvas grid's accessible status text.
// spec: UI-126
bridgeTest("HTTP status codes remain textual with all colours removed", async ({ page, audit: core }) => {
  await core.crawl();
  await selectFirstRow(page);
  await page.addStyleTag({ content: "* { color: black !important; background-color: white !important; box-shadow: none !important; }" });
  await expect(page.locator("main .os-data-grid").getByRole("gridcell", { name: "200", exact: true }).first()).toBeAttached();
});

for (const theme of ["light", "dark"] as const) {
  // spec: UI-126
  bridgeTest(`sidebar issues and segment actions retain contrast in ${theme} and Forced Colors`, async ({ page, audit: core }) => {
    await page.emulateMedia({ colorScheme: theme });
    await core.crawl();
    const list = page.getByRole("tablist", { name: "Crawl summary", exact: true });
    const selected = list.getByRole("tab", { selected: true });
    await selected.press("Home");
    await selected.press("ArrowRight");
    const badge = page.locator(".os-sidebar-priority-medium").first();
    await expect(badge).toBeVisible();
    const ratio = await badge.evaluate(el => {
      const luminance = (rgb: string) => rgb.match(/[\d.]+/g)!.slice(0, 3).map(Number).map(v => v / 255)
        .map(v => v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)
        .reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i]!, 0);
      const fg = luminance(getComputedStyle(el).color);
      const bg = luminance(getComputedStyle(el.closest(".os-sidebar")!).backgroundColor);
      return (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05);
    });
    expect(ratio).toBeGreaterThan(5);
    await bridgeTest.info().attach("medium-priority-contrast", { body: JSON.stringify({ theme, ratio }), contentType: "application/json" });
    await selected.press("ArrowRight");
    await selected.press("ArrowRight");
    const configure = page.getByRole("button", { name: "Configure segments", exact: true });
    await expect(configure).toBeVisible();
    await expect(configure).toHaveClass(/os-btn-secondary/);
    await page.emulateMedia({ colorScheme: theme, forcedColors: "active" });
    await tabTo(page, configure);
    await visibleFocus(configure);
    await scan(page, `n12-forced-colors-segments-${theme}`, "#shell-sidebar");
  });
}

// spec: UI-126
// spec: UI-001
alphaTest("the shared table fixture scopes grid and filter actions to the linked tab panel", async ({ alpha }) => {
  await alpha.open();
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  const panel = alpha.table();
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("grid")).toBeVisible();
  await panel.getByRole("button", { name: /^HTML(?: \([\d,.\s]+\))?$/ }).click();
  await expect(panel.getByRole("grid")).toBeVisible();
});
