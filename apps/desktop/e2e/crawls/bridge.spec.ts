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
test("real core manages portable crawls, reports comparison availability and rejects duplicate project names", async ({ page }) => {
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
    await expect(page.getByRole("checkbox")).toHaveCount(2);
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
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
    // Exercise an actual core rejection, rather than a successful comparison mock.
    await page.getByRole("button", { name: "Duplicate", exact: true }).click();
    await expect.poll(async () => (await page.evaluate(() => window.crawlsBridge.invoke("crawls.list", { show: "all" })) as typeof rows).length).toBe(2);
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("checkbox")).toHaveCount(3);
    await page.getByRole("checkbox", { name: "Select all filtered crawls" }).check();
    const comparison = page.getByRole("button", { name: "Choose for comparison", exact: true });
    await expect(comparison).toBeDisabled();
    await expect(comparison.locator("..")).toHaveAttribute("title", "Crawl comparison is not available yet.");
    await page.getByRole("checkbox", { name: "Select all filtered crawls" }).focus();
    await page.keyboard.press("Shift+F10");
    const comparisonItem = page.getByRole("menuitem", { name: "Choose for comparison", exact: true });
    await expect(comparisonItem).toHaveAttribute("aria-disabled", "true");
    await page.keyboard.press("Escape");
    await expect(page.getByText("Crawl comparison is not available yet.", { exact: true })).toBeVisible();
    const ids = (await page.evaluate(() => window.crawlsBridge.invoke("crawls.list", { show: "all" })) as typeof rows).map(r => r.crawl.crawlId);
    // The core (M2-C01) implements compare.open now; the dialog action stays disabled until the N02 activation.
    expect(await page.evaluate(async ids => {
      try { await window.crawlsBridge.invoke("compare.open", { current: ids[0], previous: ids[1], mapping: [], elements: [] }); return "opened"; }
      catch (error) { return (error as { code: string }).code; }
    }, ids)).toBe("opened");

    // The same real bridge supplies the duplicate-name error to the active project form.
    await page.evaluate(() => window.crawlsBridge.invoke("projects.create", { name: "Existing project" }));
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const projectForm = page.getByRole("dialog", { name: "Create project", exact: true });
    await projectForm.getByRole("textbox", { name: "Name", exact: true }).fill("Existing project");
    await projectForm.getByRole("button", { name: "Apply", exact: true }).click();
    await expect(projectForm.getByRole("alert")).toBeVisible();
    await expect(projectForm.getByRole("textbox", { name: "Name", exact: true })).toHaveValue("Existing project");
  } finally {
    await bridge?.stop();
    await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
    await rm(output, { recursive: true, force: true });
  }
});
