// Hilfen der Visualisierungs-E2E: Fenster direkt öffnen, auf Daten/Layout warten, Downloads lesen, SVG prüfen.
import { expect, type Download, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import type { VizBridge } from "./viz-bridge";

export type Kind = "crawlTree" | "crawlGraph" | "directoryTree" | "anchorWords";

/** Fensterlabel wie im Kern: `viz-` + Hex der UTF-8-Bytes der Kennung. */
export function labelFor(snapshotId: string): string {
  return `viz-${Buffer.from(snapshotId, "utf8").toString("hex")}`;
}

/** Legt im Test-Kern einen Schnappschuss an und öffnet das Fenster wie `openVisualisation` im Browser. */
export async function openViz(page: Page, bridge: VizBridge, kind: Kind, opts: { root?: number; exportDir?: string } = {}): Promise<string> {
  const { snapshotId } = (await bridge.viz.invoke("viz.open", { kind, rootUrlId: opts.root ?? null })) as { snapshotId: string };
  const q = new URLSearchParams({ bridge: bridge.url, label: labelFor(snapshotId), snapshotId, kind });
  if (opts.root !== undefined) q.set("root", String(opts.root));
  if (opts.exportDir) q.set("exportDir", opts.exportDir);
  await page.goto(`/viz.html?${q.toString()}`);
  if (kind === "anchorWords") await page.waitForFunction(() => (window.__osVizWords?.placed.length ?? 0) > 0);
  else await page.waitForFunction(() => (window.__osViz?.nodes ?? 0) > 0);
  return snapshotId;
}

export async function layoutDone(page: Page, timeout = 15_000): Promise<void> {
  await page.waitForFunction(() => window.__osViz?.layout.phase === "done", undefined, { timeout });
}

export async function canvasBox(page: Page): Promise<{ x: number; y: number; width: number; height: number }> {
  const box = await page.locator(".os-viz-canvas").boundingBox();
  if (!box) throw new Error("Zeichenfläche fehlt");
  return box;
}

/** Bildschirmposition eines Knotens relativ zur Seite. */
export async function nodePoint(page: Page, id: number): Promise<{ x: number; y: number }> {
  const box = await canvasBox(page);
  const p = await page.evaluate((nid) => window.__osViz?.screenOf(nid) ?? null, id);
  if (!p) throw new Error(`Knoten ${id} nicht sichtbar`);
  return { x: box.x + p[0], y: box.y + p[1] };
}

export function readDownload(d: Download): Promise<Buffer> {
  return d.path().then((p) => readFileSync(p));
}

/** SVG-1.1-Strukturprüfung im Browser: wohlgeformt, Namensraum, Version, nur bekannte Elemente, Farben als Hex. */
export async function validateSvg(page: Page, svg: string): Promise<{ circles: number; labels: number; hollow: string[]; radius: Record<string, number> }> {
  const result = await page.evaluate((text) => {
    const doc = new DOMParser().parseFromString(text, "image/svg+xml");
    const errors = doc.getElementsByTagName("parsererror").length;
    const root = doc.documentElement;
    const allowed = new Set(["svg", "rect", "g", "path", "circle", "text"]);
    const bad: string[] = [];
    for (const el of Array.from(doc.getElementsByTagName("*"))) {
      if (!allowed.has(el.localName)) bad.push(el.localName);
      for (const a of ["fill", "stroke"]) {
        const v = el.getAttribute(a);
        if (v !== null && v !== "none" && !/^#[0-9a-f]{6}$/.test(v)) bad.push(`${a}=${v}`);
      }
    }
    const radius: Record<string, number> = {};
    for (const c of Array.from(doc.querySelectorAll("circle.node"))) radius[c.id] = Number(c.getAttribute("r"));
    return {
      errors,
      ns: root.namespaceURI,
      version: root.getAttribute("version"),
      bad,
      circles: doc.querySelectorAll("circle.node").length,
      labels: doc.querySelectorAll("text.label").length,
      hollow: Array.from(doc.querySelectorAll('circle.node[fill="none"]')).map((c) => c.id),
      radius,
    };
  }, svg);
  expect(result.errors).toBe(0);
  expect(result.ns).toBe("http://www.w3.org/2000/svg");
  expect(result.version).toBe("1.1");
  expect(result.bad).toEqual([]);
  return result;
}
