import { createServer } from "node:http";
import { test, expect } from "@playwright/test";
import { startBridge } from "../support/bridge";

// spec: ONPAGE-192 [M2]
test("a real local crawl connects a selected URL to properties and outlinks through the bridge", async ({ page }) => {
  const site = createServer((request, response) => {
    response.writeHead(request.url === "/missing" ? 404 : 200, { "content-type": "text/html" });
    response.end(request.url === "/missing" ? "<h1>Missing</h1>" : '<title>Detail fixture</title><a href="/missing">Broken destination</a>');
  });
  await new Promise<void>(resolve => site.listen(0, "127.0.0.1", resolve));
  const address = site.address(); if (!address || typeof address === "string") throw new Error("Expected loopback port");
  const url = `http://127.0.0.1:${address.port}/`;
  const bridge = await startBridge();
  try {
    expect(bridge, "Build os-app-bridge before running core integration").not.toBeNull();
    await page.goto(`/e2e/detail/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => "detailInvoke" in window);
    await page.evaluate(async url => {
      const invoke = (window as unknown as { detailInvoke: (cmd: string, args?: unknown) => Promise<unknown> }).detailInvoke;
      await invoke("crawl.start", { source: { kind: "spider", url }, scope: "folder" });
    }, url);
    await expect.poll(() => page.evaluate(async () => {
      const invoke = (window as unknown as { detailInvoke: (cmd: string, args?: unknown) => Promise<unknown> }).detailInvoke;
      const fields = await invoke("detail.urlDetails", { urlId: 1 }) as { name: string; value: unknown }[];
      return fields.find(field => field.name === "Status Code")?.value;
    })).toBe(200);
    await page.getByRole("button", { name: "Select one URL" }).click();
    await expect(page.getByRole("rowheader", { name: "Address", exact: true }).locator("..")).toContainText(url);
    await page.getByRole("tab", { name: "Outlinks", exact: true }).click();
    await expect(page.getByLabel("Outlinks", { exact: true }).locator("canvas").first()).toBeVisible();
    await page.getByRole("searchbox").fill("Broken destination");
    await expect(page.getByLabel("Outlinks", { exact: true }).locator("canvas").first()).toBeVisible();
  } finally {
    await bridge?.stop();
    await new Promise<void>((resolve, reject) => site.close(error => error ? reject(error) : resolve()));
  }
});
