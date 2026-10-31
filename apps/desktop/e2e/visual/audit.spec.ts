import { test, expect } from "@playwright/test";
import { audit } from "./support";
// spec: UI-001
// spec: UI-121
test("Prüfer erkennt Kappen innerhalb einer Box, Überlappung und fehlenden Ellipsen-Tooltip", async ({ page }) => {
  await page.setContent(`<style>button{position:absolute;width:80px;height:24px;overflow:hidden;font:12px/20px system-ui;padding:0;border:0} .os-tab-label{display:block;white-space:nowrap;width:40px;overflow:hidden;text-overflow:ellipsis}</style>
    <button style="left:10px;top:10px" aria-label="Long fixture label" title="Long fixture label"><span class="os-tab-label">Long fixture label</span></button>
    <button style="left:50px;top:10px">Overlap</button>
    <button style="left:10px;top:50px;height:5px">Vertical clipping</button>`);
  const result = await audit(page);
  expect(result.collisions).toHaveLength(1);
  expect(result.text.some(hit => hit.includes("Vertical clipping"))).toBe(true);
  expect(result.text.some(hit => hit.includes("SPAN.os-tab-label"))).toBe(false);
  await page.locator("button").first().evaluate(e => e.removeAttribute("title"));
  expect((await audit(page)).text.some(hit => hit.includes("SPAN.os-tab-label"))).toBe(true);
});
