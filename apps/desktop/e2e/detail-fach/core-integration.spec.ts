import { createServer } from "node:http";
import { expect, test } from "@playwright/test";
import { startBridge } from "../support/bridge";

// spec: UI-037
test("a real local crawl feeds the SERP snippet and the source tabs through the bridge", async ({ page }) => {
  test.fixme(true, "wartet auf M2-K09");
  const site = createServer((_, response) => { response.writeHead(200, { "content-type": "text/html" }); response.end("<title>Core fixture</title><meta name=description content=\"Core description\"><h1>Hi</h1>"); });
  await new Promise<void>(resolve => site.listen(0, "127.0.0.1", resolve));
  const address = site.address(); if (!address || typeof address === "string") throw new Error("Expected loopback port");
  const bridge = await startBridge();
  try {
    expect(bridge, "Build os-app-bridge before running core integration").not.toBeNull();
    await page.goto(`/e2e/detail-fach/index.html?bridge=${encodeURIComponent(bridge!.url)}&url=1`);
    await page.waitForFunction(() => "fachInvoke" in window);
    await page.evaluate(async url => { await (window as unknown as { fachInvoke: (c: string, a?: unknown) => Promise<unknown> }).fachInvoke("crawl.start", { source: { kind: "spider", url }, scope: "folder" }); }, `http://127.0.0.1:${address.port}/`);
    await page.locator('[data-tab-id="serp"]').click();
    await expect(page.getByRole("textbox", { name: "Title" })).toHaveValue("Core fixture");
  } finally {
    await bridge?.stop();
    await new Promise<void>((resolve, reject) => site.close(error => error ? reject(error) : resolve()));
  }
});
