import { test, expect } from "@playwright/test";
import { startBridge } from "../support/bridge";

// Real core (os-app-bridge, K02): the text syntax is parsed by `search.parse` and regexes are checked by
// `search.validateRegex` in the fancy-regex dialect; the UI only renders the returned rule tree.
// spec: UI-011 [P1]
test("real bridge: the core parses the text syntax into the rule tree and validates regexes with back references", async ({ page }) => {
  const frames: string[] = [];
  page.on("websocket", socket => {
    socket.on("framesent", event => frames.push(`send ${event.payload}`));
    socket.on("framereceived", event => frames.push(`receive ${event.payload}`));
    socket.on("socketerror", error => frames.push(`error ${error}`));
  });
  const bridge = await startBridge();
  expect(bridge).not.toBeNull();
  try {
    await page.goto(`/?bridge=${encodeURIComponent(bridge!.url)}`);
    // The debug bridge needs a moment on a cold CI runner; wait until the table has settled before parsing.
    await expect(page.getByText(/^Showing:/)).toBeVisible({ timeout: 20_000 });
    // The tab catalog may arrive after "Showing:" (slow Windows runner); it resets the search box, so wait for it first.
    await expect(page.getByRole("tab", { name: "Internal", exact: true }).first()).toBeVisible({ timeout: 20_000 });
    await page.getByRole("button", { name: "Advanced search", exact: true }).click();
    const panel = page.getByRole("region", { name: "Advanced search" });
    const syntax = page.getByLabel("Search query (text syntax)", { exact: true });
    await syntax.fill("Address ~");
    await syntax.press("Enter");
    await expect(page.getByText(/^Query syntax error at character \d+$/)).toBeVisible({ timeout: 15_000 });
    await syntax.fill('Address ~ "/blog/" AND "Word Count" < 200');
    await syntax.press("Enter");
    await expect(panel.getByLabel("Query", { exact: true })).toHaveValue('Address ~ "/blog/" AND "Word Count" < "200"', { timeout: 15_000 });
    await expect(panel.getByLabel("Operator", { exact: true })).toHaveCount(2);
    await expect(panel.getByLabel("Operator", { exact: true }).nth(1)).toHaveValue("lessThan");
    await expect(panel.getByLabel("Value", { exact: true }).nth(0)).toHaveValue("/blog/");
    await panel.getByLabel("Operator", { exact: true }).nth(0).selectOption("regex");
    await panel.getByLabel("Value", { exact: true }).nth(0).fill("(unclosed");
    await panel.getByRole("button", { name: "Search", exact: true }).click();
    await expect(panel.getByRole("alert")).toContainText("Invalid regular expression", { timeout: 15_000 });
    await panel.getByLabel("Value", { exact: true }).nth(0).fill("/([^/]+)/\\1/");
    await panel.getByRole("button", { name: "Search", exact: true }).click();
    await expect(panel.getByLabel("Query", { exact: true })).toHaveValue('Address REGEX "/([^/]+)/\\\\1/" AND "Word Count" < "200"', { timeout: 15_000 });
    await expect(panel.getByRole("alert")).toHaveCount(0);
  } finally {
    await test.info().attach("bridge-diagnostics", { body: `stderr:\n${bridge?.stderr?.() ?? ""}\nwebsocket:\n${frames.join("\n")}`, contentType: "text/plain" });
    await bridge?.stop(); }
});
