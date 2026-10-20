import { describe, expect, it, vi } from "vitest";
import { KIND_KEY, onInlinksAnchorRequest, openVisualisation, readWindowParams, requestInlinksForAnchor, waitForKind, windowLabelFor, type KindStore } from "./open";
import { urlMenuEntries, VIZ_MENU } from "./menu";

function memory(): KindStore & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

describe("Visualisierungsfenster öffnen", () => {
  // spec: UI-090
  it("Fensterlabel wie im Kern (viz- + Hex der Kennung), Parameter aus Query bzw. gemeinsamem Speicher", () => {
    expect(windowLabelFor("ab1")).toBe("viz-616231");
    const store = memory();
    store.setItem(KIND_KEY + "s1", "crawlGraph");
    expect(readWindowParams("?snapshotId=s1", store)).toEqual({ snapshotId: "s1", kind: "crawlGraph", rootUrlId: null, label: windowLabelFor("s1") });
    expect(readWindowParams("?snapshotId=s1&kind=directoryTree&root=7&label=viz-x", store)).toEqual({ snapshotId: "s1", kind: "directoryTree", rootUrlId: 7, label: "viz-x" });
    expect(readWindowParams("?snapshotId=s1&kind=bogus", memory()).kind).toBeNull();
  });

  // spec: UI-090
  it("im Browser öffnet die Oberfläche ein benanntes, nicht modales Fenster je Schnappschuss (mehrere gleichzeitig)", async () => {
    const opened: [string, string][] = [];
    const store = memory();
    let n = 0;
    const open = vi.fn(async () => ({ snapshotId: `snap-${++n}`, window: `viz-${n}` }));
    for (const kind of ["crawlTree", "crawlGraph", "directoryTree", "crawlTree"] as const) {
      await openVisualisation(kind, kind === "directoryTree" ? 9 : null, { open, hostOpensWindow: false, openWindow: (u, name) => opened.push([u, name]), store, search: "?bridge=ws%3A%2F%2Fx&foo=1" });
    }
    expect(opened.map(([, name]) => name)).toEqual(["viz-1", "viz-2", "viz-3", "viz-4"]);
    expect(opened[2]![0]).toBe("viz.html?bridge=ws%3A%2F%2Fx&label=viz-3&snapshotId=snap-3&kind=directoryTree&root=9");
    expect(open).toHaveBeenNthCalledWith(3, { kind: "directoryTree", rootUrlId: 9 });
    expect(store.data.get(KIND_KEY + "snap-2")).toBe("crawlGraph");
  });

  // spec: UI-090
  it("in der App öffnet der Kern das Fenster; die Art wartet im gemeinsamen Speicher", async () => {
    const openWindow = vi.fn();
    const store = memory();
    await openVisualisation("crawlGraph", null, { open: async () => ({ snapshotId: "s9", window: "viz-73" }), hostOpensWindow: true, openWindow, store, search: "" });
    expect(openWindow).not.toHaveBeenCalled();
    await expect(waitForKind("s9", 10, store)).resolves.toBe("crawlGraph");
    await expect(waitForKind("unknown", 10, store)).resolves.toBeNull();
  });

  // spec: UI-090
  it("Menü „Visualisierungen“ bietet die drei Strukturansichten, nur mit geöffnetem Crawl; Kontextmenü einer URL zusätzlich die Wortwolke", () => {
    const items = VIZ_MENU[0]!.items;
    expect(VIZ_MENU[0]!.menu).toBe("visualisations");
    expect(items.map((i) => i.action)).toEqual(["viz.open.crawlTree", "viz.open.crawlGraph", "viz.open.directoryTree"]);
    expect(items[0]!.enabled!({ hasCrawl: false })).toBe(false);
    expect(items[0]!.enabled!({ hasCrawl: true })).toBe(true);
    const calls: [string, number][] = [];
    const entries = urlMenuEntries(42, (k) => k, (kind, root) => calls.push([kind, root]));
    entries.at(-1)!.onAction!();
    expect(entries.map((e) => e.label)).toEqual(["viz-kind-crawlTree", "viz-kind-crawlGraph", "viz-kind-directoryTree", "viz-kind-anchorWords"]);
    expect(calls).toEqual([["anchorWords", 42]]);
  });
});

describe("Begriff → Inlinks im Hauptfenster", () => {
  // spec: UI-099
  it("Klick auf einen Begriff sendet URL und Ankertext an das Hauptfenster", async () => {
    const got = new Promise((resolve) => {
      const off = onInlinksAnchorRequest((req) => {
        off();
        resolve(req);
      });
    });
    requestInlinksForAnchor(5, "see our prices");
    await expect(got).resolves.toEqual({ type: "inlinksAnchor", urlId: 5, anchor: "see our prices" });
  });
});
