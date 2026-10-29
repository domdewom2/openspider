import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect } from "@playwright/test";
import { clientPreview, TEST_TOKEN } from "../../src/features/mcp/fixtures/agent-m2-ui";
import type { McpClientKind } from "../../src/ipc/gen/McpClientKind";
import type {} from "./harness";

let home: string;
let writes: number;
const originals: Record<McpClientKind, string> = {
  claudeDesktop: '{"mcpServers":{"foreign":{"command":"foreign-helper"}}}',
  claudeCode: '{"mcpServers":{"foreign":{"command":"foreign-helper"}}}',
  codex: '[mcp_servers.foreign]\ncommand = "foreign-helper"\n',
};
const configPath = (kind: McpClientKind) => join(home, `${kind}.${kind === "codex" ? "toml" : "json"}`);

test.beforeEach(async ({ page }) => {
  home = await mkdtemp(join(tmpdir(), "openspider-u25-"));
  writes = 0;
  for (const kind of Object.keys(originals) as McpClientKind[]) await writeFile(configPath(kind), originals[kind]);
  await page.exposeFunction("mcpFilePreview", async (kind: McpClientKind) => ({ ...clientPreview(kind, await readFile(configPath(kind), "utf8")), path: configPath(kind) }));
  await page.exposeFunction("mcpFileWrite", async (kind: McpClientKind) => {
    // Fixture implementation of the K17 write contract: backup before write, preserve unrelated entries.
    const before = await readFile(configPath(kind), "utf8");
    const preview = clientPreview(kind, before);
    const backupPath = `${configPath(kind)}.backup`;
    await writeFile(backupPath, before);
    await writeFile(configPath(kind), preview.after);
    writes++;
    return { backupPath };
  });
  await page.clock.install({ time: new Date("2026-10-19T12:00:00Z") });
  await page.goto("/e2e/mcp/index.html");
  await page.getByRole("button", { name: "Open MCP", exact: true }).click();
  await expect(page.getByRole("button", { name: "Start server", exact: true })).toBeEnabled();
});
test.afterEach(async () => { await rm(home, { recursive: true, force: true }); });

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
test("all three clients require a reviewed diff and create a backup while retaining foreign entries", async ({ page }) => {
  await page.getByRole("button", { name: "Close", exact: true }).click();
  for (const kind of ["claudeDesktop", "claudeCode", "codex"] as const) {
    await page.evaluate(name => window.mcpFixture.action(`mcp.${name}`), kind);
    await page.getByRole("button", { name: "Preview changes", exact: true }).click();
    await expect(page.getByLabel("Configuration diff")).toContainText("foreign-helper");
    const write = page.getByRole("button", { name: "Write configuration with backup", exact: true });
    await expect(write).toBeDisabled();
    expect(await readFile(configPath(kind), "utf8")).toBe(originals[kind]);
    await page.getByText("I have reviewed the changes and want to update this client configuration.", { exact: true }).click();
    await write.click();
    await expect(page.getByText("Client configuration saved.", { exact: true })).toBeVisible();
    expect(await readFile(`${configPath(kind)}.backup`, "utf8")).toBe(originals[kind]);
    const result = await readFile(configPath(kind), "utf8");
    expect(result).toContain("foreign-helper");
    expect(result).toContain("openspider");
    if (kind !== "codex") expect(JSON.parse(result).mcpServers.foreign.command).toBe("foreign-helper");
    await page.getByRole("button", { name: "Close", exact: true }).click();
  }
  expect(writes).toBe(3);
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
test("multiline JSON and TOML client arguments never expose credentials in the rendered preview", async ({ page }) => {
  await page.getByRole("button", { name: "Close", exact: true }).click();
  for (const kind of ["claudeDesktop", "claudeCode", "codex"] as const) {
    const original = kind === "codex"
      ? `[mcp_servers.foreign]\ncommand = "foreign-helper"\nargs = [\n  "--token",\n  "${TEST_TOKEN}",\n]\n`
      : JSON.stringify({ mcpServers: { foreign: { command: "foreign-helper", args: ["--token", TEST_TOKEN] } } }, null, 2);
    await writeFile(configPath(kind), original);
    await page.evaluate(name => window.mcpFixture.action(`mcp.${name}`), kind);
    await page.getByRole("button", { name: "Preview changes", exact: true }).click();
    await expect(page.getByLabel("Configuration diff")).toContainText("foreign-helper");
    await expect(page.getByLabel("Configuration diff")).toContainText("[redacted]");
    await page.getByText("Resulting configuration", { exact: true }).click();
    expect(await page.locator("body").innerHTML()).not.toContain(TEST_TOKEN);
    await page.getByRole("button", { name: "Close", exact: true }).click();
  }
});
