import { describe, expect, it, vi } from "vitest";
import { isDirectoryKind, KIND_KEY, layoutOf, onInlinksAnchorRequest, openVisualisation, PENDING_MAX_AGE_MS, readWindowParams, rememberPendingKind, requestInlinksForAnchor, takePendingKind, waitForKind, windowLabelFor, WINDOW_KINDS, type KindStore } from "./open";
import { actionFor, MENU_KINDS, urlMenuEntries, VIZ_MENU } from "./menu";

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
  // spec: UI-094
  // spec: UI-098
  // spec: UI-100
  it("Menüaktionen heißen wie im Hauptmenü (viz.<art>) für alle acht Arten; Kontextmenü einer URL bietet alle Arten", () => {
    expect(VIZ_MENU).toEqual([]);
    expect(MENU_KINDS.map(actionFor)).toEqual([
      "viz.crawlTree", "viz.crawlGraph", "viz.directoryTree", "viz.directoryGraph", "viz.crawlGraph3d", "viz.directoryGraph3d", "viz.anchorWords", "viz.bodyWords",
    ]);
    const calls: [string, number][] = [];
    const entries = urlMenuEntries(42, (k) => k, (kind, root) => calls.push([kind, root]));
    entries.at(-1)!.onAction!();
    expect(entries.map((e) => e.label)).toEqual(WINDOW_KINDS.map((k) => `viz-kind-${k}`));
    expect(calls).toEqual([["bodyWords", 42]]);
    expect(layoutOf("directoryGraph")).toBe("force");
    expect(layoutOf("directoryGraph3d")).toBe("force3d");
    expect(layoutOf("bodyWords")).toBe("words");
    expect(isDirectoryKind("directoryGraph3d") && !isDirectoryKind("crawlGraph3d")).toBe(true);
  });

  // spec: UI-090 [M3]
  it("Kontextmenü der Tabelle: die Art reist als kurzlebiger Hinweis zum Fenster, das der Kern ohne Art öffnet", async () => {
    const store = memory();
    rememberPendingKind("directoryGraph3d", store, 1_000);
    expect(takePendingKind(store, 1_000 + PENDING_MAX_AGE_MS + 1)).toBeNull();
    rememberPendingKind("bodyWords", store);
    await expect(waitForKind("s-pending", 10, store)).resolves.toBe("bodyWords");
    // genau einmal: das nächste Fenster wählt selbst
    await expect(waitForKind("s-other", 10, store)).resolves.toBeNull();
    // die Art bleibt für dieses Fenster gespeichert (Neuladen)
    await expect(waitForKind("s-pending", 10, store)).resolves.toBe("bodyWords");
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
