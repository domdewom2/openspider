import { test, expect, type Page } from "@playwright/test";
import { mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative } from "node:path";
import type { McpClientPreview } from "../../src/ipc/gen/McpClientPreview";
import type { McpStatus } from "../../src/ipc/gen/McpStatus";
import type { ToolInfo } from "../../src/ipc/gen/ToolInfo";
import type { CrawlRef } from "../../src/ipc/gen/CrawlRef";
import type { CallLogRow } from "../../src/ipc/gen/CallLogRow";
import { HttpMcpClient } from "./httpClient";
import { startRealBridge, outBasicSite } from "./realBridge";
import type {} from "./bridgeHarness";

async function connect(page: Page, bridge: Awaited<ReturnType<typeof startRealBridge>>) {
  await page.goto(`/e2e/mcp/bridge.html?bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => !!window.mcpBridge);
  expect(await page.evaluate(() => window.mcpBridge.transport)).toBe("bridge");
  await page.getByRole("button", { name: "Open MCP", exact: true }).click();
}
async function server(page: Page) {
  await page.getByRole("button", { name: "Start server", exact: true }).click();
  await expect(page.getByRole("button", { name: "Stop server", exact: true })).toBeEnabled();
  const status = await page.evaluate(() => window.mcpBridge.invoke<McpStatus>("mcp.status"));
  expect(status.kind).toBe("ready");
  const token = await page.evaluate(() => window.mcpBridge.invoke<{ value: string }>("mcp.token", { reveal: true }));
  if (status.kind === "off") throw new Error("MCP did not start");
  return new HttpMcpClient(status.port, token.value);
}
async function status(page: Page, value: string) {
  // One-second bound is measured at every real HTTP transition, without injecting UI ticks.
  await expect(page.getByRole("button", { name: value, exact: true, includeHidden: true })).toBeAttached({ timeout: 1_000 });
}
async function sandboxFile(path: string, home: string) {
  const rel = relative(await realpath(home), path);
  expect(rel && !rel.startsWith("..") && !isAbsolute(rel), "K17 must route client config paths into the bridge's temporary AppConfig home").toBeTruthy();
  await mkdir(dirname(path), { recursive: true });
  // Check symlinks too before any test writer touches an existing config.
  const realParent = await realpath(dirname(path));
  const parentRel = relative(await realpath(home), realParent);
  expect(!parentRel.startsWith("..") && !isAbsolute(parentRel)).toBeTruthy();
  const existing = await realpath(path).catch((error: NodeJS.ErrnoException) => {
    if (error.code !== "ENOENT") throw error;
    return null;
  });
  if (existing) {
    const fileRel = relative(await realpath(home), existing);
    expect(fileRel && !fileRel.startsWith("..") && !isAbsolute(fileRel)).toBeTruthy();
  }
}

// spec: UI-129
test("real bridge reports service availability without a mock fallback", async ({ page }) => {
  const bridge = await startRealBridge();
  try {
    await connect(page, bridge);
    const result = await page.evaluate(async () => {
      try { return { status: await window.mcpBridge.invoke<McpStatus>("mcp.status") }; }
      catch (error) { return { code: (error as { code: string }).code }; }
    });
    if (result.code === "Unsupported") {
      await expect(page.getByRole("alert")).toContainText("not available in this build");
      await page.reload();
      await page.waitForFunction(() => !!window.mcpBridge);
      await expect(page.getByRole("alert")).toHaveCount(0);
      await page.getByRole("button", { name: "Open MCP", exact: true }).click();
      await expect(page.getByRole("alert")).toContainText("not available in this build");
    }
    else { expect(result.code).toBeUndefined(); expect(result.status?.kind).toBe("off"); }
  } finally { await bridge.stop(); }
});

// spec: AGENT-043
test("real writer backs up all client configs after reviewed diffs and preserves foreign entries", async ({ page }) => {
  test.fixme(true, "wartet auf M2-K17: echte Client-Pfade, Vorschau und atomarer Schreiber; Auflösung M2-N02 nach C9");
  const bridge = await startRealBridge();
  try {
    await connect(page, bridge);
    await page.getByRole("tab", { name: "Clients", exact: true }).click();
    for (const kind of ["claudeDesktop", "claudeCode", "codex"] as const) {
      const initial = await page.evaluate(kind => window.mcpBridge.invoke<McpClientPreview>("mcp.clientConfigPreview", { client: kind }), kind);
      await sandboxFile(initial.path, bridge.home);
      const original = kind === "codex" ? '[mcp_servers.foreign]\ncommand = "foreign-helper"\n' : '{"mcpServers":{"foreign":{"command":"foreign-helper"}}}';
      await writeFile(initial.path, original);
      await page.getByRole("button", { name: /Client/ }).click();
      await page.getByRole("option", { name: { claudeDesktop: "Claude Desktop", claudeCode: "Claude Code", codex: "Codex" }[kind], exact: true }).click();
      await page.getByRole("button", { name: "Preview changes", exact: true }).click();
      await expect(page.getByLabel("Configuration diff")).toContainText("foreign-helper");
      const write = page.getByRole("button", { name: "Write configuration with backup", exact: true });
      await expect(write).toBeDisabled();
      expect(await readFile(initial.path, "utf8")).toBe(original);
      const preview = await page.evaluate(kind => window.mcpBridge.invoke<McpClientPreview>("mcp.clientConfigPreview", { client: kind }), kind);
      await page.getByRole("checkbox", { name: /I have reviewed/ }).check();
      await write.click();
      await expect(page.getByText("Client configuration saved.", { exact: true })).toBeVisible();
      expect(await readFile(initial.path, "utf8")).toBe(preview.after);
      const backupText = await page.locator(".os-mcp-path").filter({ hasText: "Backup:" }).textContent();
      expect(backupText).toBeTruthy();
      const backupPath = backupText!.replace(/^Backup:\s*/, "");
      await sandboxFile(backupPath, bridge.home);
      expect(await readFile(backupPath, "utf8")).toBe(original);
      expect(preview.after).toContain("foreign-helper");
    }
  } finally { await bridge.stop(); }
});

// spec: AGENT-043
test("three genuine HTTP calls against an out-basic-site crawl appear in the UI log", async ({ page }) => {
  test.fixme(true, "wartet auf M2-K17/M2-K04: MCP-Service, Aufrufprotokoll und Crawl-Commands; Auflösung M2-N02 nach C9");
  test.setTimeout(120_000);
  const bridge = await startRealBridge();
  let site: Awaited<ReturnType<typeof outBasicSite>> | undefined;
  let http: HttpMcpClient | undefined;
  try {
    site = await outBasicSite();
    await connect(page, bridge);
    const crawl = await page.evaluate(url => window.mcpBridge.invoke<CrawlRef>("crawl.start", { source: { kind: "spider", url }, scope: "domain" }), site.url);
    await expect.poll(() => page.evaluate(() => window.mcpBridge.crawl()?.phase), { timeout: 90_000 }).toBe("completed");
    http = await server(page); await http.initialize();
    for (const [name, args] of [["list_crawls", {}], ["query_urls", { crawl_id: crawl.crawlId, tab: "Internal" }], ["url_details", { crawl_id: crawl.crawlId, url: site.url }]] as const) {
      const result = await http.rpc("tools/call", { name, arguments: args });
      expect(result?.isError).not.toBe(true); expect(result?.content?.length).toBeGreaterThan(0);
    }
    await page.getByRole("tab", { name: "Call log", exact: true }).click();
    const table = page.getByRole("table", { name: "Call log", exact: true });
    const logs = await page.evaluate(() => window.mcpBridge.invoke<CallLogRow[]>("mcp.callLog", { offset: 0, limit: 25 }));
    for (const tool of ["list_crawls", "query_urls", "url_details"]) {
      const entry = logs.find(row => row.tool === tool)!;
      expect(entry).toBeTruthy(); expect(entry.ok).toBe(true);
      expect(entry.at).toMatch(/^\d{4}-\d\d-\d\dT/); expect(entry.client).toContain("u25-http-client");
      expect(entry.durationMs).toBeGreaterThanOrEqual(0); expect(entry.resultBytes).toBeGreaterThan(0);
      const row = table.getByRole("row").filter({ hasText: tool });
      await expect(row).toContainText(entry.at); await expect(row).toContainText(`${new Intl.NumberFormat("en").format(entry.durationMs)} ms`);
      await expect(row).toContainText(new Intl.NumberFormat("en").format(entry.resultBytes!));
    }
    await expect(table).toContainText(crawl.crawlId);
    expect(JSON.stringify(logs)).not.toContain(http.token); expect(await page.locator("body").innerHTML()).not.toContain(http.token);
  } finally { try { await http?.close(); } finally { await site?.stop(); await bridge.stop(); } }
});

// spec: OUT-285 [P1]
// spec: AGENT-003 [M2]
test("HTTP tools/list, UI schemas and saved Markdown agree before and after disabling a tool", async ({ page }) => {
  test.fixme(true, "wartet auf M2-K17: Freigaben, tools/list und Markdown über den echten Service; Auflösung M2-N02 nach C9");
  const bridge = await startRealBridge();
  let http: HttpMcpClient | undefined;
  try {
    await connect(page, bridge); http = await server(page); await http.initialize();
    await page.getByRole("tab", { name: "Tools", exact: true }).click();
    for (const disabled of [false, true]) {
      if (disabled) await page.getByRole("checkbox", { name: "query_urls", exact: true }).uncheck();
      const listed = (await http.rpc("tools/list"))?.tools; expect(listed).toBeTruthy();
      const ui = await page.evaluate(() => window.mcpBridge.invoke<ToolInfo[]>("mcp.tools"));
      expect(ui.filter(tool => tool.enabled).map(tool => ({ name: tool.name, schema: tool.schema })).sort((a, b) => a.name.localeCompare(b.name)))
        .toEqual(listed!.map(tool => ({ name: tool.name, schema: tool.inputSchema })).sort((a, b) => a.name.localeCompare(b.name)));
      for (const tool of ui) {
        const row = page.getByRole("table").getByRole("row").filter({ has: page.getByRole("checkbox", { name: tool.name, exact: true }) });
        expect(await row.locator("pre").textContent()).toBe(JSON.stringify(tool.schema, null, 2));
      }
      const path = join(bridge.home, `tools-${disabled}.md`);
      await page.evaluate(path => window.mcpBridge.destination(path), path);
      await page.getByRole("button", { name: "Download API description (Markdown)…", exact: true }).click();
      await expect.poll(() => readFile(path, "utf8").catch(() => "")).toContain("## ");
      const markdown = await readFile(path, "utf8");
      expect([...markdown.matchAll(/^## (.+)$/gm)].map(match => match[1]).sort()).toEqual(listed!.map(tool => tool.name).sort());
      for (const tool of listed!) {
        const section = markdown.split(`## ${tool.name}\n`)[1]!.split("\n## ")[0]!;
        const parameters = Object.fromEntries([...section.matchAll(/^\| (\w+) \| `(.*)` \|$/gm)].map(match => [match[1]!, JSON.parse(match[2]!.replaceAll("\\|", "|")) as unknown]));
        expect(parameters).toEqual(tool.inputSchema.properties ?? {});
      }
      if (disabled) {
        expect(listed!.some(tool => tool.name === "query_urls")).toBe(false);
        expect((await http.rpc("tools/call", { name: "query_urls", arguments: {} }))?.isError).toBe(true);
      }
    }
  } finally { try { await http?.close(); } finally { await bridge.stop(); } }
});

// spec: OUT-285 [P1]
// spec: AGENT-002
test("real HTTP sessions update status within a second, reject unauthorized requests and expire after ten minutes", async ({ page }) => {
  test.fixme(true, "wartet auf M2-K17: HTTP-Sitzungszähler und Ablauf an echte Statusereignisse anbinden; Auflösung M2-N02 nach C9");
  test.setTimeout(660_000);
  const bridge = await startRealBridge();
  let first: HttpMcpClient | undefined; let second: HttpMcpClient | undefined;
  try {
    await connect(page, bridge); await status(page, "MCP: off");
    first = await server(page); await status(page, `MCP: ready (Port ${first.port})`);
    const unauthorized = await first.request("POST", { jsonrpc: "2.0", id: 1, method: "initialize" }, { Authorization: "Bearer invalid" });
    expect(unauthorized.status).toBe(401);
    const forbidden = await first.request("POST", {}, { Origin: "https://foreign.example" }); expect(forbidden.status).toBe(403);
    const badHost = await first.request("POST", {}, { Host: `foreign.example:${first.port}` }); expect(badHost.status).toBe(403);
    await status(page, `MCP: ready (Port ${first.port})`);
    await first.initialize(); await status(page, "MCP: connected (1 client)");
    second = new HttpMcpClient(first.port, first.token, "u25-second-client");
    await second.initialize(); await status(page, "MCP: connected (2 clients)");
    await first.close(); await status(page, "MCP: connected (1 client)");
    // Keep the second client's SSE connection open, then abort it as a real client disconnect.
    const disconnect = new AbortController();
    const stream = await fetch(`http://127.0.0.1:${second.port}/mcp`, { headers: { Accept: "text/event-stream", Authorization: `Bearer ${second.token}`, "Mcp-Session-Id": second.session!, "MCP-Protocol-Version": "2025-06-18" }, signal: disconnect.signal });
    expect(stream.status).toBe(200); disconnect.abort(); await stream.body?.cancel().catch(() => {});
    second.session = null;
    await status(page, `MCP: ready (Port ${first.port})`);
    await second.initialize(); await status(page, "MCP: connected (1 client)");
    // Browser fake time cannot advance the Rust server. Exercise the actual ten-minute timeout.
    const expires = Date.now() + 600_000;
    while (Date.now() < expires) await page.waitForTimeout(Math.min(30_000, expires - Date.now()));
    await status(page, `MCP: ready (Port ${first.port})`);
    const expired = await second.request("POST", { jsonrpc: "2.0", id: 99, method: "tools/list", params: {} });
    expect(expired.status).toBe(404); second.session = null;
    await page.getByRole("button", { name: "Stop server", exact: true }).click(); await status(page, "MCP: off");
  } finally { try { await first?.close(); await second?.close(); } finally { await bridge.stop(); } }
});

// spec: OUT-285 [P1]
test("autostart survives a real app restart and a busy port produces a visible failure", async ({ page }) => {
  test.fixme(true, "wartet auf M2-K17: persistierter Auto-Start und Startfehler-Notice; Auflösung M2-N02 nach C9");
  const home = await mkdtemp(join(tmpdir(), "u25-restart-"));
  let bridge: Awaited<ReturnType<typeof startRealBridge>> | undefined;
  const blocker = createServer();
  try {
    bridge = await startRealBridge({ home }); await connect(page, bridge);
    const http = await server(page);
    await page.getByRole("checkbox", { name: "Start automatically when the app opens", exact: true }).check();
    await expect.poll(() => page.evaluate(() => window.mcpBridge.invoke<{ mcp: { autostart: boolean } }>("settings.get").then(settings => settings.mcp.autostart))).toBe(true);
    await bridge.stop(); bridge = await startRealBridge({ home });
    await connect(page, bridge);
    const visibleElapsed = await page.evaluate(() => performance.now() - window.mcpBridge.appearedAt);
    const started = performance.now();
    await expect.poll(async () => {
      try { const response = await fetch(`http://127.0.0.1:${http.port}/mcp`); await response.body?.cancel(); return response.status; }
      catch { return 0; }
    }, { timeout: 2_000 }).toBe(401);
    expect(visibleElapsed + performance.now() - started).toBeLessThanOrEqual(2_000);
    await status(page, `MCP: ready (Port ${http.port})`);
    await bridge.stop(); bridge = undefined;
    await new Promise<void>((resolve, reject) => { blocker.once("error", reject); blocker.listen(http.port, "127.0.0.1", resolve); });
    bridge = await startRealBridge({ home }); await connect(page, bridge);
    await status(page, "MCP: off");
    await expect(page.getByRole("alert")).toContainText("MCP operation failed");
  } finally {
    await bridge?.stop();
    if (blocker.listening) await new Promise<void>((resolve, reject) => blocker.close(error => error ? reject(error) : resolve()));
    await rm(home, { force: true, recursive: true });
  }
});
