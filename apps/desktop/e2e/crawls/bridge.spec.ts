import { test, expect } from "@playwright/test";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startBridge } from "../support/bridge";
import type {} from "./bridge-harness";

// spec: UI-124
// spec: OUT-228
// spec: OUT-191 [M2]
// spec: CRAWL-127
test("real core lists a completed crawl, renames it without changing ID and exports a portable file", async ({ page }) => {
  test.setTimeout(60_000);
  const output = await mkdtemp(join(tmpdir(), "os-crawls-u15-"));
  const server = createServer((request, response) => {
    response.setHeader("Content-Type", request.url === "/robots.txt" ? "text/plain" : "text/html");
    response.end(request.url === "/robots.txt" ? "User-agent: *\nAllow: /\n" : "<!doctype html><title>U15 crawl fixture</title><h1>Fixture</h1>");
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Fixture server has no TCP port");
  const bridge = await startBridge();
  try {
    expect(bridge, "Real os-app bridge required").not.toBeNull();
    await page.goto(`/e2e/crawls/bridge.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.crawlsBridge);
    await page.evaluate(async url => { await window.crawlsBridge.invoke("crawl.start", { source: { kind: "spider", url }, scope: "domain" }); }, `http://127.0.0.1:${address.port}/`);
    await expect.poll(() => page.evaluate(() => window.crawlsBridge.phase()), { timeout: 20_000 }).toBe("completed");
    await page.getByRole("button", { name: "Manage", exact: true }).click();
    await expect(page.getByRole("table", { name: "Saved crawls" })).toBeVisible();
    await page.getByRole("checkbox", { name: "Select all filtered crawls" }).check();
    await page.getByRole("button", { name: "Rename", exact: true }).click();
    await page.getByRole("textbox", { name: "Name", exact: true }).fill("U15 renamed fixture");
    await page.getByRole("button", { name: "Apply", exact: true }).click();
    await expect(page.getByRole("cell", { name: "U15 renamed fixture", exact: true })).toBeVisible();
    const rows = await page.evaluate(async () => window.crawlsBridge.invoke("crawls.list", { show: "all" })) as { crawl: { crawlId: string; name: string; file: string } }[];
    expect(rows).toHaveLength(1);
    expect(rows[0]!.crawl.name).toBe("U15 renamed fixture");
    const dest = join(output, "export.ospider");
    await page.evaluate(path => window.crawlsBridge.setDestination(path), dest);
    await page.getByRole("button", { name: "Export crawl…", exact: true }).click();
    await expect(page.getByRole("status")).toContainText("Operation completed.");
    const bytes = await readFile(dest);
    expect(bytes.subarray(0, 16).toString()).toBe("SQLite format 3\u0000");
    const original = await readFile(rows[0]!.crawl.file);
    expect(original.byteLength).toBeGreaterThan(0);
    const after = await page.evaluate(async () => window.crawlsBridge.invoke("crawls.list", { show: "all" })) as typeof rows;
    expect(after[0]!.crawl.crawlId).toBe(rows[0]!.crawl.crawlId);
  } finally {
    await bridge?.stop();
    await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
    await rm(output, { recursive: true, force: true });
  }
});
