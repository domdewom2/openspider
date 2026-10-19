import { test, expect } from "@playwright/test";
import { startBridge } from "../support/bridge";

// Real core (os-app-bridge, K02): the text syntax is parsed by `search.parse` and regexes are checked by
// `search.validateRegex` in the fancy-regex dialect; the UI only renders the returned rule tree.
// spec: UI-011 [P1]
test("real bridge: the core parses the text syntax into the rule tree and validates regexes with back references", async ({ page }) => {
  const bridge = await startBridge();
  expect(bridge).not.toBeNull();
  try {
    await page.goto(`/?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.getByRole("button", { name: "Advanced search", exact: true }).click();
    const panel = page.getByRole("region", { name: "Advanced search" });
    const syntax = page.getByLabel("Search query (text syntax)", { exact: true });
    await syntax.fill("Address ~");
    await syntax.press("Enter");
    await expect(page.getByText(/^Query syntax error at character \d+$/)).toBeVisible();
    await syntax.fill('Address ~ "/blog/" AND "Word Count" < 200');
    await syntax.press("Enter");
    await expect(panel.getByLabel("Query", { exact: true })).toHaveValue('Address ~ "/blog/" AND "Word Count" < "200"');
    await expect(panel.getByLabel("Operator", { exact: true })).toHaveCount(2);
    await expect(panel.getByLabel("Operator", { exact: true }).nth(1)).toHaveValue("lessThan");
    await expect(panel.getByLabel("Value", { exact: true }).nth(0)).toHaveValue("/blog/");
    await panel.getByLabel("Operator", { exact: true }).nth(0).selectOption("regex");
    await panel.getByLabel("Value", { exact: true }).nth(0).fill("(unclosed");
    await panel.getByRole("button", { name: "Search", exact: true }).click();
    await expect(panel.getByRole("alert")).toContainText("Invalid regular expression");
    await panel.getByLabel("Value", { exact: true }).nth(0).fill("/([^/]+)/\\1/");
    await panel.getByRole("button", { name: "Search", exact: true }).click();
    await expect(panel.getByLabel("Query", { exact: true })).toHaveValue('Address REGEX "/([^/]+)/\\\\1/" AND "Word Count" < "200"');
    await expect(panel.getByRole("alert")).toHaveCount(0);
  } finally { await bridge?.stop(); }
});
