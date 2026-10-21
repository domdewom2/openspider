import { test, expect } from "@playwright/test";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startBridge } from "../support/bridge";
import type { ViewHandle } from "../../src/ipc/gen/ViewHandle";
import type {} from "./harness";

// spec: OUT-020 [M2]
test("real bridge exports the filtered view into the predetermined save path", async ({ page }) => {
  const output = await mkdtemp(join(tmpdir(), "os-export-u16-"));
  const server = createServer((request, response) => {
    response.setHeader("Content-Type", request.url === "/robots.txt" ? "text/plain" : "text/html");
    response.end(request.url === "/robots.txt" ? "User-agent: *\nAllow: /\n" : request.url === "/" ? '<title>Start</title><a href="/blog/missing">Missing</a>' : "<h1>Missing title</h1>");
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Fixture server has no TCP port");
  const bridge = await startBridge();
  try {
    expect(bridge, "real os-app bridge required; no mock fallback").not.toBeNull();
    await page.goto(`/e2e/export/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.exportFixture);
    const start = `http://127.0.0.1:${address.port}/`;
    await page.evaluate(async url => { await window.exportFixture.invoke("crawl.start", { source: { kind: "spider", url }, scope: "domain" }); }, start);
    await expect.poll(() => page.evaluate(() => window.exportFixture.phase())).toBe("completed");
    const handle = await page.evaluate(async () => window.exportFixture.invoke("view.open", { spec: { kind: "tab", tab: "Page Titles", filter: "Missing", sort: [] }, columns: ["Address", "Title 1"] })) as ViewHandle;
    const path = join(output, "titles.csv");
    await page.evaluate(({ handle, path }) => {
      window.exportFixture.setDestination(path);
      window.exportFixture.open({ kind: "view", tab: "Page Titles", filter: "Missing", viewId: handle.viewId, version: handle.version, columns: ["Address", "Title 1"] });
    }, { handle, path });
    await page.getByRole("button", { name: "Save…" }).click();
    await expect(page.getByText("Export complete.")).toBeVisible();
    const csv = await readFile(path, "utf8");
    expect(csv).toContain(`${start}blog/missing`);
    expect(csv).not.toContain(",Start");
  } finally {
    await bridge?.stop();
    await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
    await rm(output, { recursive: true, force: true });
  }
});
