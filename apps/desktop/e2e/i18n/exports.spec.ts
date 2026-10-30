import { createServer } from "node:http";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { startBridge } from "../support/bridge";
import type { ViewHandle } from "../../src/ipc/gen/ViewHandle";
import type {} from "../export/harness";

// spec: UI-121
// spec: CRAWL-137
// spec: OUT-271
test("real catalog names are translated and de/en export identical canonical CSV headers, decimal points and filenames", async ({ page }) => {
  test.setTimeout(120_000);
  const output = await mkdtemp(join(tmpdir(), "os-u33-"));
  const server = createServer((request, response) => {
    response.setHeader("Content-Type", request.url === "/robots.txt" ? "text/plain" : "text/html");
    response.end(request.url === "/robots.txt" ? "User-agent: *\nAllow: /\n" : "<html><head><title>Language fixture</title></head><body><p>This is a small local fixture with enough text to calculate a fractional text ratio.</p></body></html>");
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("No fixture address");
  const bridge = await startBridge();
  try {
    expect(bridge, "real bridge required").not.toBeNull();
    await page.clock.setFixedTime(new Date("2026-10-29T10:00:00Z"));
    await page.goto(`/e2e/export/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.exportFixture);
    await page.evaluate(url => window.exportFixture.invoke("crawl.start", { source: { kind: "spider", url }, scope: "domain" }), `http://127.0.0.1:${address.port}/`);
    await expect.poll(() => page.evaluate(() => window.exportFixture.phase())).toBe("completed");
    const missing = await page.evaluate(async () => {
      const path = "/src/i18n/index.ts";
      const { i18n } = await import(/* @vite-ignore */ path);
      const catalog = await window.exportFixture.invoke("export.catalog") as { bulk: { labelKey: string; items: { labelKey: string; disabledReason?: string | null }[] }[]; reports: { labelKey: string; items: { labelKey: string; disabledReason?: string | null }[] }[] };
      const names = [...catalog.bulk, ...catalog.reports].flatMap(group => [group.labelKey, ...group.items.map(item => item.labelKey)]);
      const reasons = [...catalog.bulk, ...catalog.reports].flatMap(group => group.items.flatMap(item => item.disabledReason ? [item.disabledReason] : []));
      return [...names, ...reasons].filter(name => !i18n.has("de", name) || !i18n.has("en", name));
    });
    expect(missing).toEqual([]);
    const handle = await page.evaluate(() => window.exportFixture.invoke("view.open", { spec: { kind: "tab", tab: "Internal", filter: "All", sort: [] }, columns: ["Address", "Text Ratio"] })) as ViewHandle;
    const files: string[] = [];
    const suggestions: string[] = [];
    for (const lang of ["de", "en"]) {
      const path = join(output, `${lang}.csv`);
      await page.evaluate(async ({ lang, path, handle }) => {
        const module = "/src/i18n/index.ts";
        await (await import(/* @vite-ignore */ module)).setLanguage(lang);
        window.exportFixture.setDestination(path);
        window.exportFixture.open({ kind: "view", tab: "Internal", filter: "All", viewId: handle.viewId, version: handle.version, columns: ["Address", "Text Ratio"] });
      }, { lang, path, handle });
      await page.getByRole("button", { name: lang === "de" ? "Speichern…" : "Save…", exact: true }).click();
      await expect(page.getByRole("dialog")).toHaveCount(0);
      await expect(page.getByRole("status")).toContainText(lang === "de" ? "Export abgeschlossen." : "Export complete.");
      files.push(await readFile(path, "utf8"));
      suggestions.push(await page.evaluate(() => window.exportFixture.destinations.at(-1)!.defaultName));
    }
    expect(files[0]).toBe(files[1]);
    expect(files[0]).toContain("Address,Text Ratio");
    expect(files[0]).toMatch(/,\d+\.\d+/);
    expect(suggestions[0]).toBe(suggestions[1]);
  } finally {
    await bridge?.stop();
    await new Promise<void>(resolve => server.close(() => resolve()));
    await rm(output, { recursive: true, force: true });
  }
});
