import { test, expect } from "@playwright/test";
import { clientPreview, TEST_TOKEN } from "../../src/features/mcp/fixtures/agent-m2-ui";
import type {} from "./harness";

test.beforeEach(async ({ page }) => {
  await page.clock.install({ time: new Date("2026-10-19T12:00:00Z") });
  await page.goto("/e2e/mcp/index.html");
  await page.getByRole("button", { name: "Open MCP", exact: true }).click();
  await expect(page.getByRole("button", { name: "Start server", exact: true })).toBeEnabled();
});

// spec: UI-129
// spec: OUT-285 [P1]
test("menu and status work without a crawl; session changes appear immediately", async ({ page }) => {
  await page.getByRole("button", { name: "Start server", exact: true }).click();
  await expect(page.getByRole("button", { name: "MCP: ready (Port 51234)" })).toBeAttached();
  await page.getByRole("button", { name: "Close", exact: true }).click();
  for (const clients of [1, 2, 1, 0]) {
    await page.evaluate(count => window.mcpFixture.setStatus(count ? { kind: "connected", port: 51234, clients: count } : { kind: "ready", port: 51234 }), clients);
    await expect(page.getByRole("button", { name: clients ? `MCP: connected (${clients} ${clients === 1 ? "client" : "clients"})` : "MCP: ready (Port 51234)" })).toBeVisible();
  }
  await page.getByRole("button", { name: "MCP: ready (Port 51234)" }).click();
  await page.getByRole("menuitem", { name: "Stop server", exact: true }).click();
  await expect(page.getByRole("button", { name: "MCP: off" })).toBeVisible();
  const menu = await page.evaluate(() => window.__osMenu?.model);
  expect(menu?.menus.find(m => m.id === "mcp")?.items.some(i => i.id === "mcp.codex")).toBe(true);
});

// spec: AGENT-002
test("copy keeps the token out of the DOM; reveal, hide, regenerate and Escape clear it", async ({ page }) => {
  await page.getByRole("tab", { name: "Access", exact: true }).click();
  await page.getByRole("button", { name: "Copy token", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.mcpFixture.clipboard.at(-1))).toBe(TEST_TOKEN);
  expect(await page.locator("body").innerHTML()).not.toContain(TEST_TOKEN);
  await page.getByRole("button", { name: "Show token", exact: true }).click();
  await expect(page.getByText(TEST_TOKEN, { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Regenerate token", exact: true }).click();
  await expect(page.getByText(TEST_TOKEN, { exact: true })).toHaveCount(0);
  expect(await page.locator("body").innerHTML()).not.toContain(`${TEST_TOKEN}-rotated`);
  await page.getByRole("button", { name: "Show token", exact: true }).click();
  await expect(page.getByText(`${TEST_TOKEN}-rotated`, { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await page.locator("body").innerHTML()).not.toContain(TEST_TOKEN);
  await expect(page.getByRole("button", { name: "Open MCP", exact: true })).toBeFocused();
});

// spec: AGENT-043
// spec: AGENT-003 [M2]
test("each menu entry opens its own dialog and connects in one click with the crawl permission", async ({ page }) => {
  await page.getByRole("button", { name: "Close", exact: true }).click();
  for (const [kind, program] of [["claudeDesktop", "Claude Desktop"], ["claudeCode", "Claude Code"], ["codex", "Codex"]] as const) {
    await page.evaluate(name => window.mcpFixture.action(`mcp.${name}`), kind);
    const dialog = page.getByRole("dialog", { name: `Connect ${program}` });
    await expect(dialog.getByText("Not connected yet.", { exact: true })).toBeVisible();
    await expect(dialog.getByRole("checkbox", { name: "Agent may start crawls" })).toBeChecked();
    await dialog.getByRole("button", { name: "Connect", exact: true }).click();
    await expect(dialog.getByText(`Connected. Start a new session in ${program} so that OpenSpider appears there.`, { exact: true })).toBeVisible();
    await expect(dialog.getByText(`Backup: ${kind}.backup`, { exact: true })).toBeVisible();
    await expect(dialog.getByText("Already connected.", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
  }
  expect(await page.evaluate(() => window.mcpFixture.connected())).toEqual(["claudeDesktop", "claudeCode", "codex"]);
  const connects = await page.evaluate(() => window.mcpFixture.calls().filter(call => call.cmd === "mcp.clientConnect").map(call => call.args));
  expect(connects).toEqual([
    { client: "claudeDesktop", allowCrawl: true }, { client: "claudeCode", allowCrawl: true }, { client: "codex", allowCrawl: true },
  ]);
});

// spec: AGENT-043
test("three fixture calls show time, client, parameters, duration and result size without the token", async ({ page }) => {
  await page.evaluate(() => window.mcpFixture.recordCalls());
  await page.getByRole("tab", { name: "Call log", exact: true }).click();
  const table = page.getByRole("table", { name: "Call log", exact: true });
  await expect(table.getByRole("row")).toHaveCount(4);
  await expect(table).toContainText("out-basic-site");
  await expect(table).toContainText("fixture-client");
  await expect(table).toContainText("2026-10-19T12:00:00Z");
  await expect(table).toContainText("12 ms");
  await expect(table).toContainText("2,048 B");
  expect(await page.locator("body").innerHTML()).not.toContain(TEST_TOKEN);
});

// spec: AGENT-003 [M2]
// spec: OUT-285 [P1]
test("classes and individual tools use the service contract and canceling a Markdown save has no effect", async ({ page }) => {
  await page.getByRole("tab", { name: "Tools", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "Read", exact: true })).toBeChecked();
  await expect(page.getByRole("checkbox", { name: "crawl_start", exact: true })).toBeDisabled();
  await page.getByText("Crawl", { exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "crawl_start", exact: true })).toBeEnabled();
  await page.getByRole("checkbox", { name: "query_urls", exact: true }).focus();
  await page.getByRole("checkbox", { name: "query_urls", exact: true }).press("Space");
  await expect(page.getByRole("checkbox", { name: "query_urls", exact: true })).not.toBeChecked();
  await page.evaluate(() => window.mcpFixture.setDestination(null));
  await page.getByRole("button", { name: "Download API description (Markdown)…", exact: true }).click();
  expect(await page.evaluate(() => window.mcpFixture.calls().filter(call => call.cmd === "mcp.saveToolsMarkdown"))).toEqual([]);
});

// spec: AGENT-043
test("the optional preview shows only changed lines and never credentials", async ({ page }) => {
  const value = clientPreview("codex", "notConnected", {
    connectDiff: `@@ -2 +2,2 @@\n-token = "${TEST_TOKEN}"\n+token = "${TEST_TOKEN}"\n+[mcp_servers.openspider]\n`,
  });
  await page.evaluate(preview => window.mcpFixture.setClientPreview(preview), value);
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.evaluate(() => window.mcpFixture.action("mcp.codex"));
  const dialog = page.getByRole("dialog", { name: "Connect Codex" });
  await expect(dialog.getByLabel("Configuration diff")).toBeHidden();
  await dialog.getByText("Show changes", { exact: true }).click();
  await expect(dialog.getByLabel("Configuration diff")).toContainText("+[mcp_servers.openspider]");
  expect(await page.locator("body").innerHTML()).not.toContain(TEST_TOKEN);
  await dialog.getByRole("button", { name: "Copy entry", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.mcpFixture.clipboard.at(-1))).toBe(value.entry);
});
