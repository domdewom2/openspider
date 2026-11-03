// E2E-A M3-V02 gegen die Test-Bridge (synthetische Schnappschüsse wie die Fixtures fx-ui-dirs/fx-ui-words):
// Verzeichnisgraph (UI-094), Links eines Knotens einblenden (UI-097), 3D-Crawl-/Verzeichnisgraph per WebGL (UI-098),
// Body-Text-Wortwolke (UI-100). Je Visualisierung: öffnen, Export (SVG/CSV bzw. PNG), axe ohne critical/serious.
// Mit OS_V02_SHOTS=1 legt der Lauf je Visualisierung einen Screenshot unter docs/plan/M3-reports/v02/ ab.
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { FakeVizCore } from "../../src/features/viz/fixtures/fakeCore";
import { fxUiDirs, fxUiWords, fxUiWordsGraph } from "../../src/features/viz/fixtures/synthetic";
import { canvasBox, layoutDone, nodePoint, openViz, readDownload, validateSvg } from "./support";
import { startVizBridge, type VizBridge } from "./viz-bridge";

let bridge: VizBridge;
const SHOTS = process.env.OS_V02_SHOTS === "1" ? fileURLToPath(new URL("../../../../docs/plan/M3-reports/v02/", import.meta.url)) : null;

async function shot(page: Page, name: string, browserName: string) {
  if (!SHOTS || browserName !== "chromium") return;
  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, `${name}.png`) });
}

async function expectAccessible(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  const serious = results.violations.filter((v) => v.impact === "critical" || v.impact === "serious");
  expect(serious.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
}

/** Ob die Umgebung WebGL 2 bietet; wenn ja, muss die 3D-Ansicht es auch nutzen (kein stilles Überspringen). */
async function webgl(page: Page): Promise<boolean> {
  await page.waitForFunction(() => window.__osViz3d !== undefined);
  const available = await page.evaluate(() => !!document.createElement("canvas").getContext("webgl2"));
  if (available) await expect.poll(() => page.evaluate(() => window.__osViz3d!.webgl)).toBe(true);
  return available;
}

async function layout3dDone(page: Page) {
  await page.waitForFunction(() => window.__osViz3d?.layout.phase === "done", undefined, { timeout: 30_000 });
}

/** Bildschirmpunkt eines Knotens in der 3D-Szene (Seitenkoordinaten). */
async function point3d(page: Page, id: number) {
  const box = await canvasBox(page);
  const p = await page.evaluate((n) => window.__osViz3d!.screenOf(n), id);
  if (!p) throw new Error(`Knoten ${id} nicht sichtbar`);
  return { x: box.x + p[0], y: box.y + p[1] };
}

test.beforeEach(async () => {
  bridge = await startVizBridge(new FakeVizCore(fxUiWordsGraph, fxUiDirs, fxUiWords));
});

test.afterEach(async () => {
  await bridge.stop();
});

test.describe("Kraftgerichteter Verzeichnisgraph", () => {
  // spec: UI-094
  // spec: UI-090 [M3]
  test("öffnet mit den Knoten und Kanten des Verzeichnisbaums, SVG valide mit hohlen Pfadknoten, CSV-Export, axe", async ({ page, browserName }) => {
    const dir = mkdtempSync(join(tmpdir(), "os-viz-v02-"));
    try {
      const snapshotId = await openViz(page, bridge, "directoryGraph", { exportDir: dir });
      await layoutDone(page);
      const graphArgs = bridge.viz.calls.find((c) => c.cmd === "viz.graph")!.args as Record<string, unknown>;
      const tree = (await bridge.viz.invoke("viz.graph", { ...graphArgs, snapshotId, kind: "directoryTree" })) as { nodes: unknown[]; edges: unknown[] };
      expect(await page.evaluate(() => [window.__osViz!.nodes, window.__osViz!.edges])).toEqual([tree.nodes.length, tree.edges.length]);
      await expect(page.getByTestId("viz-node-count")).toHaveText("8 nodes");
      const [svgDownload] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "SVG", exact: true }).click()]);
      expect(svgDownload.suggestedFilename()).toBe("directoryGraph.svg");
      const svg = await validateSvg(page, (await readDownload(svgDownload)).toString("utf8"));
      expect(svg.hollow.sort()).toEqual(["nm1", "nm2", "nm3"]);
      // Wurzel liegt nach Konvergenz in der Mitte (fixiert, wie UI-092)
      const box = await canvasBox(page);
      const root = await page.evaluate(() => window.__osViz!.screenOf(-1));
      expect(root![0]).toBeCloseTo(box.width / 2, 0);
      await page.getByRole("button", { name: "Export nodes and edges (CSV)" }).click();
      await expect(page.getByTestId("viz-export-done")).toContainText(dir);
      expect(bridge.viz.lastExport!.nodes).toHaveLength(8);
      await expectAccessible(page);
      await shot(page, "verzeichnisgraph", browserName);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

test.describe("Links eines Knotens einblenden (2D)", () => {
  // spec: UI-097
  // spec: UI-090 [M3]
  test("„Inlinks zeigen“ blendet die 23 Unique Inlinks ein, SVG/CSV enthalten sie, „Zurücksetzen“ entfernt sie", async ({ page, browserName }) => {
    const dir = mkdtempSync(join(tmpdir(), "os-viz-v02-"));
    try {
      await openViz(page, bridge, "crawlGraph", { exportDir: dir });
      await layoutDone(page);
      const p = await nodePoint(page, 2);
      await page.mouse.click(p.x, p.y, { button: "right" });
      await expect(page.getByRole("menuitem", { name: "Show inlinks to children" })).toBeVisible();
      await expectAccessible(page);
      await page.getByRole("menuitem", { name: "Show inlinks", exact: true }).click();
      await expect.poll(() => page.evaluate(() => window.__osViz!.links?.edges)).toBe(23);
      await expect(page.getByTestId("viz-links-state")).toHaveText("Show inlinks: 23 links");
      await shot(page, "links-einblenden-2d", browserName);
      const [svgDownload] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "SVG", exact: true }).click()]);
      const text = (await readDownload(svgDownload)).toString("utf8");
      await validateSvg(page, text);
      expect(text).toContain('class="edges-inlink"');
      expect(text).toContain('class="node dim"');
      await page.getByRole("button", { name: "Export nodes and edges (CSV)" }).click();
      await expect(page.getByTestId("viz-export-done")).toContainText(dir);
      expect(bridge.viz.lastExport!.edges.filter((e) => e.edgeType === "inlink")).toHaveLength(23);
      // Optionen: ohne nofollow 18
      await page.getByText("Include nofollow links").click();
      await expect.poll(() => page.evaluate(() => window.__osViz!.links?.edges)).toBe(18);
      await expectAccessible(page);
      const q = await nodePoint(page, 2);
      await page.mouse.click(q.x, q.y, { button: "right" });
      await page.getByRole("menuitem", { name: "Reset links" }).click();
      await expect.poll(() => page.evaluate(() => window.__osViz!.links)).toBeNull();
      expect(await page.evaluate(() => window.__osViz!.svg())).not.toContain("edges-inlink");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

test.describe("3D-Varianten (WebGL)", () => {
  // spec: UI-098
  // spec: UI-097
  // spec: UI-090 [M3]
  test("3D-Crawl-Graph: Drehen, Zoomen (0,05× … 8×), Rechtsziehen verschiebt, Rechtsklick öffnet das Knotenmenü, Links ein/aus, Suche fliegt zum Knoten, PNG", async ({ page, browserName }) => {
    await openViz(page, bridge, "crawlGraph3d");
    test.skip(!(await webgl(page)), "WebGL 2 in dieser Umgebung nicht verfügbar (Hinweis-Test unten)");
    await layout3dDone(page);
    await expect(page.getByTestId("viz-node-count")).toHaveText("26 nodes");
    const box = await canvasBox(page);
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    const cam = () => page.evaluate(() => window.__osViz3d!.camera);
    // Linksziehen dreht
    const c0 = await cam();
    await page.mouse.move(cx - 150, cy + 100);
    await page.mouse.down();
    await page.mouse.move(cx - 90, cy + 80, { steps: 4 });
    await page.mouse.up();
    const c1 = await cam();
    expect(c1.theta).not.toBe(c0.theta);
    expect(c1.target).toEqual(c0.target);
    // Mausrad zoomt, hart begrenzt
    for (let i = 0; i < 40; i++) await page.mouse.wheel(0, -2000);
    await expect.poll(async () => (await cam()).k).toBe(8);
    for (let i = 0; i < 60; i++) await page.mouse.wheel(0, 2000);
    await expect.poll(async () => (await cam()).k).toBe(0.05);
    await page.evaluate(() => window.__osViz3d!.setZoom(1));
    // Rechtsziehen verschiebt (Ziel bewegt sich), ohne ein Menü zu öffnen
    const c2 = await cam();
    await page.mouse.move(cx - 150, cy + 100);
    await page.mouse.down({ button: "right" });
    await page.mouse.move(cx - 90, cy + 60, { steps: 4 });
    await page.mouse.up({ button: "right" });
    const c3 = await cam();
    expect(c3.target).not.toEqual(c2.target);
    expect(c3.theta).toBe(c2.theta);
    await expect(page.getByRole("menu")).toHaveCount(0);
    // Suche fliegt zum Knoten: Ziel = Knoten in der Bildmitte
    await page.getByRole("searchbox", { name: "Search URL" }).fill("target.html");
    await expect(page.getByTestId("viz-search-count")).toHaveText("1 of 1");
    await expect
      .poll(async () => {
        const at = await page.evaluate(() => window.__osViz3d!.screenOf(2));
        return at ? Math.hypot(at[0] - box.width / 2, at[1] - box.height / 2) : Infinity;
      })
      .toBeLessThan(2);
    // Tooltip nach 300 ms mit den Zeilen aus UI-090
    const h = await point3d(page, 2);
    await page.mouse.move(h.x, h.y);
    const tip = page.getByRole("tooltip");
    await expect(tip).toBeVisible();
    expect(await tip.locator("dt").allTextContents()).toEqual(["Address", "Status Code", "Indexability", "Crawl Depth", "Unique Inlinks", "Link Score", "Crawl Depth"]);
    await expect(tip.locator("dd").first()).toHaveText("http://127.0.0.1:8000/target.html");
    // Rechtsklick ohne Ziehen auf den Knoten öffnet dessen Kontextmenü
    const p = await point3d(page, 2);
    await page.mouse.click(p.x, p.y, { button: "right" });
    await expect(page.getByRole("menuitem", { name: "Show in table" })).toBeEnabled();
    await expectAccessible(page);
    await page.getByRole("menuitem", { name: "Show inlinks", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.__osViz3d!.links?.edges)).toBe(23);
    await expect(page.getByTestId("viz-links-state")).toHaveText("Show inlinks: 23 links");
    await shot(page, "3d-crawl-graph-links", browserName);
    // PNG des aktuellen Ausschnitts (1×), Bild nicht leer
    const [png] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "PNG 1×" }).click()]);
    expect(png.suggestedFilename()).toBe("crawlGraph3d@1x.png");
    const bytes = await readDownload(png);
    expect(bytes.subarray(1, 4).toString("ascii")).toBe("PNG");
    expect(bytes.readUInt32BE(16)).toBe(Math.round(box.width));
    const colours = await page.evaluate(async (b64) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = document.createElement("canvas");
      c.width = img.width;
      c.height = img.height;
      const ctx = c.getContext("2d")!;
      ctx.drawImage(img, 0, 0);
      const d = ctx.getImageData(0, 0, c.width, c.height).data;
      const set = new Set<number>();
      for (let i = 0; i < d.length; i += 4 * 97) set.add((d[i]! << 16) | (d[i + 1]! << 8) | d[i + 2]!);
      return set.size;
    }, bytes.toString("base64"));
    expect(colours).toBeGreaterThan(2);
    // Zurücksetzen über das Knotenmenü
    const q = await point3d(page, 2);
    await page.mouse.click(q.x, q.y, { button: "right" });
    await page.getByRole("menuitem", { name: "Reset links" }).click();
    await expect.poll(() => page.evaluate(() => window.__osViz3d!.links)).toBeNull();
  });

  // spec: UI-098
  // spec: UI-094
  test("3D-Verzeichnisgraph öffnet mit der Verzeichnisstruktur, Knotengrenze 100 000, PNG 2×, axe", async ({ page, browserName }) => {
    await openViz(page, bridge, "directoryGraph3d");
    test.skip(!(await webgl(page)), "WebGL 2 in dieser Umgebung nicht verfügbar");
    await layout3dDone(page);
    await expect(page.getByTestId("viz-node-count")).toHaveText("8 nodes");
    expect(bridge.viz.calls.find((c) => c.cmd === "viz.graph")!.args).toMatchObject({ kind: "directoryGraph3d", budget: 100_000 });
    await expect(page.getByLabel("Node limit")).toHaveValue("100000");
    const box = await canvasBox(page);
    const [png] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "PNG 2×" }).click()]);
    expect((await readDownload(png)).readUInt32BE(16)).toBe(Math.round(box.width * 2));
    await expectAccessible(page);
    await shot(page, "3d-verzeichnisgraph", browserName);
  });

  // spec: UI-098
  test("Kontextverlust: Hinweis statt Leerfläche, nach Wiederherstellung wieder vollständig gezeichnet (WEBGL_lose_context)", async ({ page }) => {
    await openViz(page, bridge, "crawlGraph3d");
    test.skip(!(await webgl(page)), "WebGL 2 in dieser Umgebung nicht verfügbar");
    await layout3dDone(page);
    const ok = await page.evaluate(() => {
      const c = document.querySelector("canvas.os-viz-canvas-3d") as HTMLCanvasElement;
      const ext = c.getContext("webgl2")?.getExtension("WEBGL_lose_context");
      (window as unknown as { __lose?: WEBGL_lose_context | null }).__lose = ext ?? null;
      ext?.loseContext();
      return !!ext;
    });
    expect(ok).toBe(true);
    await expect(page.getByTestId("viz-context-lost")).toBeVisible();
    await expect.poll(() => page.evaluate(() => window.__osViz3d!.webgl)).toBe(false);
    await page.evaluate(() => (window as unknown as { __lose: WEBGL_lose_context }).__lose.restoreContext());
    await expect(page.getByTestId("viz-context-lost")).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => window.__osViz3d!.webgl)).toBe(true);
    // Szene vollständig zurück: Knoten an ihren Positionen sichtbar, Export zeigt mehr als den Hintergrund.
    const p = await point3d(page, 1);
    expect(p.x).toBeGreaterThan(0);
    const [png] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "PNG 1×" }).click()]);
    const bytes = await readDownload(png);
    const colours = await page.evaluate(async (b64) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = document.createElement("canvas");
      c.width = img.width;
      c.height = img.height;
      const ctx = c.getContext("2d")!;
      ctx.drawImage(img, 0, 0);
      const d = ctx.getImageData(0, 0, c.width, c.height).data;
      // Farbige Bildpunkte (Knoten); der Hintergrund ist grau.
      let saturated = 0;
      for (let i = 0; i < d.length; i += 4) if (Math.max(d[i]!, d[i + 1]!, d[i + 2]!) - Math.min(d[i]!, d[i + 1]!, d[i + 2]!) > 60) saturated++;
      return saturated;
    }, bytes.toString("base64"));
    expect(colours).toBeGreaterThan(50);
  });

  // spec: UI-098
  test("ohne WebGL-Unterstützung erscheint ein Hinweis statt eines leeren Fensters", async ({ page }) => {
    await page.addInitScript(() => {
      const original = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement, id: string, ...rest: unknown[]) {
        if (id === "webgl2" || id === "webgl" || id === "experimental-webgl") return null;
        return (original as (...a: unknown[]) => unknown).call(this, id, ...rest);
      } as typeof original;
    });
    await openViz(page, bridge, "crawlGraph3d");
    await expect(page.getByTestId("viz-no-webgl")).toContainText("needs WebGL 2");
    await expect(page.locator("canvas.os-viz-canvas-3d")).toHaveCount(0);
    await expectAccessible(page);
  });
});

test.describe("Body-Text-Wortwolke", () => {
  // spec: UI-100
  test("Wortwolke mit Tabelle Term/Count (Standardoptionen), SVG-Export, axe", async ({ page, browserName }) => {
    await openViz(page, bridge, "bodyWords", { root: 2 });
    await expect(page.getByTestId("viz-terms").locator("tbody tr")).toHaveCount(17);
    await expect(page.getByTestId("viz-terms").locator("tbody tr").first()).toHaveText("pricing6");
    expect(bridge.viz.calls.find((c) => c.cmd === "viz.words")!.args).toMatchObject({ kind: "body", urlId: 2, removeStopwords: true, minLength: 3, maxTerms: 100 });
    const [svg] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "SVG", exact: true }).click()]);
    expect(svg.suggestedFilename()).toBe("body-words.svg");
    const text = (await readDownload(svg)).toString("utf8");
    const parsed = await page.evaluate((s) => {
      const doc = new DOMParser().parseFromString(s, "image/svg+xml");
      return { errors: doc.getElementsByTagName("parsererror").length, words: doc.querySelectorAll("text").length };
    }, text);
    expect(parsed.errors).toBe(0);
    expect(parsed.words).toBeGreaterThan(10);
    await expectAccessible(page);
    await shot(page, "body-wortwolke", browserName);
  });

  // spec: UI-100
  test("ohne gespeichertes HTML: Hinweis HTML_NOT_STORED nach UI-016", async ({ page }) => {
    bridge.viz.bodyWords = false;
    await openViz(page, bridge, "bodyWords", { root: 2 });
    await expect(page.locator('[data-empty-state="HTML_NOT_STORED"]')).toBeVisible();
    await expectAccessible(page);
  });
});
