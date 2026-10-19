// E2E-A gegen die echte Bridge `os-app-bridge` (Kern-Commands `viz.*` aus M2-K16): Crawl einer lokalen Site,
// Crawl-Baum und Verzeichnisbaum im Visualisierungsfenster, CSV-Export in einen Ordner, Hinweis „Daten geändert“.
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { bridgeAvailable, startBridge, type Bridge } from "../support/bridge";
import { labelFor } from "./support";

const PAGES: Record<string, string[]> = {
  "/": ["/docs/a.html", "/docs/b.html", "/blog/post"],
  "/docs/a.html": ["/"],
  "/docs/b.html": ["/blog/post?page=2"],
  "/blog/post": ["/"],
  "/blog/post?page=2": ["/"],
};

function site(): Promise<{ server: Server; origin: string }> {
  const server = createServer((req, res) => {
    const links = PAGES[req.url ?? ""];
    if (!links) {
      res.statusCode = 404;
      res.end("not found");
      return;
    }
    res.setHeader("Content-Type", "text/html; charset=utf-8");
    res.end(`<!doctype html><html><head><title>${req.url}</title></head><body>${links.map((l) => `<a href="${l}">to ${l}</a>`).join(" ")}</body></html>`);
  });
  return new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => {
      const a = server.address() as { port: number };
      resolve({ server, origin: `http://127.0.0.1:${a.port}` });
    }),
  );
}

async function crawl(page: Page, start: string): Promise<void> {
  await page.evaluate(async (url) => {
    const path = "/src/ipc/crawl.ts";
    const { crawl } = (await import(path)) as typeof import("../../src/ipc/crawl");
    await crawl.start({ source: { kind: "spider", url }, scope: "subdomains" });
  }, start);
}

/** Öffnet einen Schnappschuss im Hauptfenster und liefert dessen Knotenzahl (wartet, bis der Crawl alles enthält). */
async function openSnapshot(page: Page, kind: string): Promise<{ snapshotId: string; nodes: number }> {
  return page.evaluate(async (k) => {
    const path = "/src/ipc/viz.ts";
    const { viz } = (await import(path)) as typeof import("../../src/ipc/viz");
    const opened = await viz.open({ kind: k as never });
    const g = await viz.graph({ snapshotId: opened.snapshotId, kind: k as never, budget: 10_000, metric: "crawl_depth", expandDepth: 100 });
    return { snapshotId: opened.snapshotId, nodes: g.nodes.length };
  }, kind);
}

test.describe("Visualisierungen gegen die echte Bridge", () => {
  test.skip(!bridgeAvailable(), "os-app-bridge nicht gebaut (cargo build -p os-app --features e2e-bridge --bin os-app-bridge)");
  let bridge: Bridge | null;
  let web: { server: Server; origin: string };

  test.beforeEach(async () => {
    bridge = await startBridge();
    web = await site();
  });
  test.afterEach(async () => {
    await bridge?.stop();
    await new Promise<void>((r) => web.server.close(() => r()));
  });

  // spec: UI-090
  // spec: UI-091
  test("Crawl-Baum der gecrawlten Site, CSV-Export (nodes.csv/edges.csv) mit genau den dargestellten Knoten, Hinweis „Daten geändert“", async ({ page, context }) => {
    expect(bridge).not.toBeNull();
    const dir = mkdtempSync(join(tmpdir(), "viz-bridge-"));
    try {
      await page.goto(`/?bridge=${encodeURIComponent(bridge!.url)}`);
      await crawl(page, `${web.origin}/`);
      await expect.poll(async () => (await openSnapshot(page, "crawlTree")).nodes, { timeout: 30_000 }).toBe(5);
      const { snapshotId } = await openSnapshot(page, "crawlTree");

      const viz = await context.newPage();
      const q = new URLSearchParams({ bridge: bridge!.url, label: labelFor(snapshotId), snapshotId, kind: "crawlTree", exportDir: dir });
      await viz.goto(`/viz.html?${q.toString()}`);
      await expect(viz.getByTestId("viz-node-count")).toHaveText("5 nodes");
      await viz.getByRole("button", { name: "Export nodes and edges (CSV)" }).click();
      await expect(viz.getByTestId("viz-export-done")).toBeVisible({ timeout: 15_000 });
      const nodes = readFileSync(join(dir, "nodes.csv"), "utf8").trim().split(/\r?\n/);
      const edges = readFileSync(join(dir, "edges.csv"), "utf8").trim().split(/\r?\n/);
      expect(nodes[0]).toBe("Id,Address,Parent Id,Depth,Status Code,Indexability,Segment,Size Metric,Color Value,x,y");
      expect(nodes).toHaveLength(1 + 5);
      expect(edges[0]).toBe("Source Id,Target Id,Edge Type");
      expect(edges).toHaveLength(1 + 4);
      expect(existsSync(join(dir, "nodes.csv"))).toBe(true);

      // Daten derselben Crawl-Datei ändern sich (URL nachträglich hinzugefügt) → Hinweis, Neuladen erst per Klick.
      PAGES["/docs/c.html"] = ["/"];
      await page.evaluate(async (url) => {
        const path = "/src/ipc/crawl.ts";
        const { crawl } = (await import(path)) as typeof import("../../src/ipc/crawl");
        await crawl.addUrls({ urls: [url] });
      }, `${web.origin}/docs/c.html`);
      await expect(viz.getByTestId("viz-data-changed")).toBeVisible({ timeout: 15_000 });
      await expect(viz.getByTestId("viz-node-count")).toHaveText("5 nodes");
    } finally {
      delete PAGES["/docs/c.html"];
      rmSync(dir, { recursive: true, force: true });
    }
  });

  // spec: UI-093
  test("Verzeichnisbaum: Verzeichnisse ohne Indexseite als hohle Pfadknoten", async ({ page, context }) => {
    expect(bridge).not.toBeNull();
    await page.goto(`/?bridge=${encodeURIComponent(bridge!.url)}`);
    await crawl(page, `${web.origin}/`);
    await expect.poll(async () => (await openSnapshot(page, "crawlTree")).nodes, { timeout: 30_000 }).toBe(5);
    const { snapshotId } = await openSnapshot(page, "directoryTree");
    const viz = await context.newPage();
    const q = new URLSearchParams({ bridge: bridge!.url, label: labelFor(snapshotId), snapshotId, kind: "directoryTree" });
    await viz.goto(`/viz.html?${q.toString()}`);
    await viz.waitForFunction(() => (window.__osViz?.nodes ?? 0) > 0);
    const hollow = await viz.evaluate(() => (window.__osViz!.svg().match(/<circle [^>]*fill="none"/g) ?? []).length);
    // Schema-Knoten „http“ plus /docs und /blog (beide ohne eigene Indexseite).
    expect(hollow).toBe(3);
  });
});
