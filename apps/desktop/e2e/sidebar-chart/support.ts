// UI-102 E2E helpers: chart area locators, SVG 1.1 structure check, PNG size from the IHDR chunk, box overlap.
import { expect, type Download, type Locator, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

export const area = (page: Page): Locator => page.getByRole("region", { name: "Chart of the selected entry" });
export const chartItem = (page: Page, key: string): Locator => area(page).locator(`[data-ovc-key="${key}"]`);
export const segment = (page: Page, key: string): Locator => area(page).locator(`[data-ovc-segment="${key}"]`);
/** The name button of a tab block in the Overview tree (the arrow is „▾ <name>“). */
export const treeTab = (page: Page, name: string): Locator => page.locator(".os-sidebar-overview").getByRole("button", { name, exact: true });

export async function readDownload(d: Download): Promise<Buffer> {
  return readFileSync(await d.path());
}

/** Width and height of a PNG (IHDR). */
export function pngSize(png: Buffer): { width: number; height: number } {
  expect(png.subarray(1, 4).toString("ascii")).toBe("PNG");
  return { width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
}

/** SVG 1.1 structure check in the browser (like the visualisation E2E): well formed, namespace, version, known elements, hex colours. */
export async function validateSvg(page: Page, svg: string): Promise<{ texts: string[]; segments: { fill: string }[]; patterns: Record<string, string> }> {
  const result = await page.evaluate((text) => {
    const doc = new DOMParser().parseFromString(text, "image/svg+xml");
    const root = doc.documentElement;
    const allowed = new Set(["svg", "defs", "pattern", "rect", "g", "path", "text"]);
    const bad: string[] = [];
    for (const el of Array.from(doc.getElementsByTagName("*"))) {
      if (!allowed.has(el.localName)) bad.push(el.localName);
      for (const a of ["fill", "stroke"]) {
        const v = el.getAttribute(a);
        if (v !== null && v !== "none" && !/^#[0-9a-f]{6}$/.test(v) && !/^url\(#hatch-\d+\)$/.test(v)) bad.push(`${a}=${v}`);
      }
    }
    const patterns: Record<string, string> = {};
    for (const p of Array.from(doc.getElementsByTagName("pattern"))) patterns[p.id] = p.querySelector("rect")?.getAttribute("fill") ?? "";
    return {
      errors: doc.getElementsByTagName("parsererror").length,
      ns: root.namespaceURI,
      version: root.getAttribute("version"),
      bad,
      texts: Array.from(doc.getElementsByTagName("text")).map(t => t.textContent ?? ""),
      segments: Array.from(doc.querySelectorAll("path.segment")).map(p => ({ fill: p.getAttribute("fill") ?? "" })),
      patterns,
    };
  }, svg);
  expect(result.errors).toBe(0);
  expect(result.ns).toBe("http://www.w3.org/2000/svg");
  expect(result.version).toBe("1.1");
  expect(result.bad).toEqual([]);
  return result;
}

export function overlaps(a: { x: number; y: number; width: number; height: number }, b: { x: number; y: number; width: number; height: number }): boolean {
  return a.x < b.x + b.width - 0.5 && b.x < a.x + a.width - 0.5 && a.y < b.y + b.height - 0.5 && b.y < a.y + a.height - 0.5;
}

/** Periodic background polls of other regions (API status, sign-in status, export catalogue, events): not caused by a selection. */
export const BACKGROUND = /^(api\.status|auth\.loginStatus|export\.catalog|events\.|diag\.|crawl\.status|op\.)/;

/**
 * Writes every pending ui-state change now (shell layout of UI-001/UI-125 with its 1-s debounce, batched ui-state
 * writes): a startup save then cannot fall into a measuring window, and a save caused by the measured action would be
 * written (and counted) before the window closes.
 */
export async function settleUiState(page: import("@playwright/test").Page): Promise<void> {
  await page.evaluate(async () => {
    const { useLayout } = await import(/* @vite-ignore */ String("/src/state/layout.ts"));
    const { flushUiState } = await import(/* @vite-ignore */ String("/src/state/uistate.ts"));
    await useLayout.getState().flush().catch(() => undefined);
    await flushUiState().catch(() => undefined);
  });
}
