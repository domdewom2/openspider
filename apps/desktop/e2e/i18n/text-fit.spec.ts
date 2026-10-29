import { expect, test } from "@playwright/test";
import { textFits } from "./text-fit";

// spec: UI-121
test("detects nested and plain text clipping and requires the full ellipsis name", async ({ page }) => {
  await page.setContent('<main><div style="width:40px;overflow:hidden;white-space:nowrap"><label><span>Nested clipped caption</span></label></div></main>');
  await expect(textFits(page.locator("main"), "nested")).rejects.toThrow("Nested clipped caption");
  await page.setContent('<main><div style="width:40px;overflow:hidden;white-space:nowrap">Ordinary clipped text outside label selectors</div></main>');
  await expect(textFits(page.locator("main"), "ordinary text node")).rejects.toThrow("Ordinary clipped text outside label selectors");
  await page.setContent('<main><p style="width:40px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis" title="Incomplete">A complete accessible caption</p></main>');
  await expect(textFits(page.locator("main"), "incomplete tooltip")).rejects.toThrow("A complete accessible caption");
  await page.locator("p").evaluate(el => { el.setAttribute("title", el.textContent!); });
  await textFits(page.locator("main"), "full tooltip");
  await page.locator("p").evaluate(el => { el.removeAttribute("title"); el.setAttribute("aria-label", el.textContent!); });
  await textFits(page.locator("main"), "full accessible name");
  await page.locator("p").evaluate(el => {
    el.removeAttribute("aria-label");
    el.setAttribute("aria-describedby", "full-tooltip");
    const tip = document.createElement("span");
    tip.id = "full-tooltip";
    tip.setAttribute("role", "tooltip");
    tip.textContent = el.textContent;
    el.after(tip);
  });
  await textFits(page.locator("main"), "full linked tooltip");
  await page.setContent('<main><div style="width:40px;overflow:hidden;white-space:nowrap"><span style="display:contents">Clipped display contents caption</span></div></main>');
  await expect(textFits(page.locator("main"), "display contents")).rejects.toThrow("Clipped display contents caption");
  await page.setContent('<main><div style="width:100px"><span>A caption that wraps onto several lines without clipping.</span></div></main>');
  await textFits(page.locator("main"), "wrapping");
});
