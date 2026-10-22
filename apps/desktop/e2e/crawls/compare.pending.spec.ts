import { test, expect } from "@playwright/test";
import { createServer } from "node:http";
import { startBridge } from "../support/bridge";
import type { CrawlListing } from "../../src/ipc/gen/CrawlListing";
import type { CompareOpenArgs } from "../../src/ipc/gen/CompareOpenArgs";
import type { CompareSession } from "../../src/ipc/gen/CompareSession";
import type {} from "./bridge-harness";

// spec: UI-124
// spec: OUT-228
test("two selected non-running crawls enable comparison and open a real core comparison session", async ({ page }) => {
  test.setTimeout(60_000);
  const server = createServer((request, response) => {
    response.setHeader("Content-Type", request.url === "/robots.txt" ? "text/plain" : "text/html");
    response.end(request.url === "/robots.txt" ? "User-agent: *\nAllow: /\n" : "<!doctype html><title>U15 comparison fixture</title><h1>Fixture</h1>");
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  let bridge: Awaited<ReturnType<typeof startBridge>> = null;
  try {
    const address = server.address();
    if (!address || typeof address === "string") throw new Error("Fixture server has no TCP port");
    bridge = await startBridge();
    expect(bridge, "Real os-app bridge required").not.toBeNull();
    // Observe the UI's actual wire request and the core response; no command interception/mocks.
    const calls: { id: number; args: CompareOpenArgs }[] = [];
    const responses = new Map<number, { ok?: CompareSession; err?: unknown }>();
    page.on("websocket", socket => {
      if (socket.url() !== bridge!.url) return;
      socket.on("framesent", frame => {
        const message = JSON.parse(String(frame.payload)) as { id: number; cmd: string; args: CompareOpenArgs };
        if (message.cmd === "compare.open") calls.push({ id: message.id, args: message.args });
      });
      socket.on("framereceived", frame => {
        const message = JSON.parse(String(frame.payload)) as { id?: number; ok?: CompareSession; err?: unknown };
        if (message.id !== undefined) responses.set(message.id, message);
      });
    });
    await page.goto(`/e2e/crawls/bridge.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.crawlsBridge);
    for (let index = 0; index < 2; index++) {
      await page.evaluate(async url => {
        await window.crawlsBridge.invoke("crawl.start", { source: { kind: "spider", url }, scope: "domain" });
      }, `http://127.0.0.1:${address.port}/?crawl=${index}`);
      await expect.poll(async () => {
        const rows = await page.evaluate(() => window.crawlsBridge.invoke("crawls.list", { show: "all" })) as CrawlListing[];
        return rows.length === index + 1 && rows.every(row => row.crawl.phase === "completed");
      }, { timeout: 20_000 }).toBe(true);
      // Durable metadata can finish before the engine/automatic analysis releases the session.
      // Detach only after that work settles; clear preserves the saved crawl file.
      await expect.poll(() => page.evaluate(async () => {
        try {
          await window.crawlsBridge.invoke("crawl.clear");
          return "cleared";
        } catch (error) {
          if ((error as { code?: string }).code === "Busy") return "busy";
          throw error;
        }
      }), { timeout: 20_000 }).toBe("cleared");
    }
    const rows = await page.evaluate(() => window.crawlsBridge.invoke("crawls.list", { show: "all" })) as CrawlListing[];
    expect(rows).toHaveLength(2);
    expect(rows.every(row => row.crawl.phase !== "running")).toBe(true);
    await page.getByRole("button", { name: "Manage", exact: true }).click();
    await expect(page.getByRole("checkbox")).toHaveCount(3);
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
    const action = page.getByRole("button", { name: "Choose for comparison", exact: true });
    await expect(action).toBeDisabled();
    const selections = page.getByRole("checkbox", { name: /^Select: / });
    await selections.nth(0).check();
    await expect(action).toBeDisabled();
    await selections.nth(1).check();
    await expect(action).toBeEnabled();
    await action.click();
    await expect.poll(() => calls.length).toBe(1);
    const call = calls[0]!;
    expect(new Set([call.args.current, call.args.previous])).toEqual(new Set(rows.map(row => row.crawl.crawlId)));
    await expect.poll(() => responses.has(call.id)).toBe(true);
    const response = responses.get(call.id)!;
    expect(response.err).toBeUndefined();
    expect(response.ok?.current.crawlId).toBe(call.args.current);
    expect(response.ok?.previous.crawlId).toBe(call.args.previous);
  } finally {
    await bridge?.stop();
    await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  }
});
