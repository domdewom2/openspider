import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createClient, getClient, setClient } from "../../ipc/client";
import { connectWindowEvents } from "../../ipc/events";
import { runAction } from "../../ui/actions";
import { buildMenuModel } from "../../ui/menu/build";
import { allMenuContributions } from "../../ui/menu/registry";
import { FakeVizCore } from "./fixtures/fakeCore";
import { genUiLarge } from "./fixtures/synthetic";
import { windowLabelFor } from "./open";
import { VizHost } from "./VizHost";
import { VizWindow } from "./VizWindow";

let core: FakeVizCore;
let offEvents: () => void = () => undefined;

async function openWindow(kind: "crawlTree" | "crawlGraph" | "directoryTree" | "anchorWords", rootUrlId: number | null = null) {
  const { snapshotId } = (await core.invoke("viz.open", { kind, rootUrlId })) as { snapshotId: string };
  const label = windowLabelFor(snapshotId);
  setClient(createClient(core.transport(label)));
  offEvents = connectWindowEvents(getClient(), label);
  render(<VizWindow params={{ snapshotId, kind, rootUrlId, label }} exportDir="/tmp/viz-out" />);
  return snapshotId;
}

const graphCalls = () => core.calls.filter((c) => c.cmd === "viz.graph").map((c) => c.args as Record<string, unknown>);

beforeEach(() => {
  core = new FakeVizCore();
});

afterEach(() => {
  offEvents();
  cleanup();
  setClient(null);
  vi.restoreAllMocks();
});

describe("Visualisierungsfenster", () => {
  // spec: UI-091
  it("zeigt den Crawl-Baum mit Knotenzahl und Legende (indexierbar, nicht indexierbar, gekappt)", async () => {
    await openWindow("crawlTree");
    expect(await screen.findByTestId("viz-node-count")).toHaveTextContent("25 nodes");
    const legend = screen.getByTestId("viz-legend");
    expect(legend).toHaveTextContent("Indexable");
    expect(legend).toHaveTextContent("Non-indexable");
    expect(legend).toHaveTextContent("Subpages hidden");
    expect(graphCalls()[0]).toMatchObject({ kind: "crawlTree", budget: 10_000, expandDepth: 3, metric: "crawl_depth", includeNonHtml: false, includeNonIndexable: true });
    expect(screen.getByRole("img", { name: /Crawl tree with 25 nodes/ })).toBeInTheDocument();
  });

  // spec: UI-090
  it("Suche: Zähler „i von n“, Enter und Shift+Enter wechseln den Treffer", async () => {
    await openWindow("crawlTree");
    await screen.findByText("25 nodes");
    const box = screen.getByRole("searchbox", { name: "Search URL" });
    fireEvent.change(box, { target: { value: "PAGE-2" } });
    expect(screen.getByTestId("viz-search-count")).toHaveTextContent("1 of 6");
    fireEvent.keyDown(box, { key: "Enter" });
    expect(screen.getByTestId("viz-search-count")).toHaveTextContent("2 of 6");
    fireEvent.keyDown(box, { key: "Enter", shiftKey: true });
    expect(screen.getByTestId("viz-search-count")).toHaveTextContent("1 of 6");
    fireEvent.change(box, { target: { value: "nothing-here" } });
    expect(screen.getByTestId("viz-search-count")).toHaveTextContent("No matches");
  });

  // spec: UI-090
  it("„Daten geändert – Aktualisieren“: Hinweis nach Meldung des Kerns, Neuladen erst per Klick mit gleicher Wurzel", async () => {
    const id = await openWindow("crawlTree", 1);
    await screen.findByText("25 nodes");
    expect(screen.queryByTestId("viz-data-changed")).toBeNull();
    const before = graphCalls().length;
    act(() => core.notifyChanged(id));
    const hint = await screen.findByTestId("viz-data-changed");
    expect(hint).toHaveTextContent("Data changed");
    expect(graphCalls()).toHaveLength(before);
    fireEvent.click(within(hint).getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(screen.queryByTestId("viz-data-changed")).toBeNull());
    await waitFor(() => expect(graphCalls().length).toBe(before + 1));
    expect(core.calls.find((c) => c.cmd === "viz.open" && (c.args as { refreshSnapshotId?: string }).refreshSnapshotId === id)).toBeTruthy();
    expect(graphCalls().at(-1)).toMatchObject({ root: 1 });
  });

  // spec: UI-095
  it("Metriken ohne Daten sind ausgegraut; Größenskalierung zeigt Min-/Max-Radius und Legende", async () => {
    await openWindow("crawlGraph");
    await screen.findByText("25 nodes");
    const select = screen.getByLabelText("Scaling metric") as HTMLSelectElement;
    const option = (name: string) => Array.from(select.options).find((o) => o.text === name)!;
    expect(option("GA Sessions").disabled).toBe(true);
    expect(option("Word Count").disabled).toBe(false);
    fireEvent.change(select, { target: { value: "word_count" } });
    await waitFor(() => expect(graphCalls().at(-1)).toMatchObject({ metric: "word_count", expandDepth: 1_000_000 }));
    fireEvent.click(screen.getByRole("switch", { name: "Scale node size by metric" }));
    expect(screen.getByLabelText("Min. radius (px)")).toBeInTheDocument();
    expect(screen.getByTestId("viz-legend")).toHaveTextContent("Size: Word Count");
    fireEvent.change(screen.getByLabelText("Colour by"), { target: { value: "segment" } });
    expect(screen.getByTestId("viz-legend")).toHaveTextContent("No segment");
  });

  // spec: UI-096
  it("Budget einstellbar bis 50 000 mit Warnung über 10 000; Kontextmenü „Als Wurzel setzen“ mit Pfadleiste und Rückkehr", async () => {
    core = new FakeVizCore(() => genUiLarge([1, 40, 2000, 4000]));
    await openWindow("crawlTree");
    await screen.findByText("6,041 nodes");
    const budget = screen.getByLabelText("Maximum nodes");
    fireEvent.change(budget, { target: { value: "60000" } });
    fireEvent.blur(budget);
    await waitFor(() => expect(graphCalls().at(-1)).toMatchObject({ budget: 50_000 }));
    expect(screen.getByText(/More than 10,000 nodes/)).toBeInTheDocument();
    fireEvent.change(budget, { target: { value: "1000" } });
    fireEvent.blur(budget);
    await screen.findByText("1,000 nodes");
    expect(screen.getByText(/Node budget reached/)).toBeInTheDocument();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search URL" }), { target: { value: "/l1/p2.html" } });
    const canvas = screen.getByRole("img");
    fireEvent.keyDown(canvas, { key: "F10", shiftKey: true });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Focus" }));
    await waitFor(() => expect(graphCalls().at(-1)).toMatchObject({ root: 2 }));
    const trail = screen.getByRole("navigation", { name: "Root path" });
    await waitFor(() => expect(within(trail).getAllByRole("listitem")).toHaveLength(2));
    expect(within(trail).getByText("http://127.0.0.1:8000/l1/p2.html")).toHaveAttribute("aria-current", "page");
    fireEvent.click(within(trail).getByRole("button"));
    await waitFor(() => expect(graphCalls().at(-1)).toMatchObject({ root: null }));
    await waitFor(() => expect(within(trail).getAllByRole("listitem")).toHaveLength(1));
  });

  // spec: UI-096
  it("„Teilbaum öffnen“ lädt die direkten Kinder eines gekappten Knotens nach", async () => {
    core = new FakeVizCore(() => genUiLarge([1, 30, 3000]));
    await openWindow("crawlTree");
    await screen.findByText("3,031 nodes");
    const budget = screen.getByLabelText("Maximum nodes");
    fireEvent.change(budget, { target: { value: "20" } });
    fireEvent.blur(budget);
    await screen.findByText("20 nodes");
    fireEvent.change(screen.getByRole("searchbox", { name: "Search URL" }), { target: { value: "/l1/p2.html" } });
    fireEvent.keyDown(screen.getByRole("img"), { key: "F10", shiftKey: true });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Explore" }));
    await waitFor(() => expect(graphCalls().at(-1)).toMatchObject({ exploreNode: 2 }));
    await screen.findByText("120 nodes");
  });

  // spec: UI-096
  it("über Budget + 1 000 erscheint statt des Nachladens die Rückfrage, den Knoten als Wurzel zu setzen", async () => {
    core = new FakeVizCore(() => genUiLarge([1, 3000]));
    await openWindow("crawlTree");
    await screen.findByText("3,001 nodes");
    const budget = screen.getByLabelText("Maximum nodes");
    fireEvent.change(budget, { target: { value: "10" } });
    fireEvent.blur(budget);
    await screen.findByText("10 nodes");
    fireEvent.change(screen.getByRole("searchbox", { name: "Search URL" }), { target: { value: "8000/" } });
    fireEvent.keyDown(screen.getByRole("img"), { key: "F10", shiftKey: true });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Explore" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("2,991 subpages");
    expect(graphCalls().some((c) => c.exploreNode === 1)).toBe(false);
    fireEvent.click(within(dialog).getByRole("button", { name: "Focus" }));
    await waitFor(() => expect(graphCalls().at(-1)).toMatchObject({ root: 1 }));
  });

  // spec: UI-090
  it("Export: SVG als Datei, CSV über viz.exportCsv mit genau den dargestellten Knoten", async () => {
    const blobs: Blob[] = [];
    URL.createObjectURL = vi.fn((b: Blob) => {
      blobs.push(b);
      return "blob:viz";
    }) as typeof URL.createObjectURL;
    URL.revokeObjectURL = vi.fn();
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    await openWindow("crawlTree");
    await screen.findByText("25 nodes");
    fireEvent.click(screen.getByRole("button", { name: "SVG" }));
    expect(blobs).toHaveLength(1);
    expect(blobs[0]!.type).toBe("image/svg+xml");
    expect(await blobs[0]!.text()).toMatch(/^<\?xml version="1.0" encoding="UTF-8"\?>\n<svg xmlns="http:\/\/www.w3.org\/2000\/svg" version="1.1"/);
    expect(screen.getByLabelText("Target folder")).toHaveValue("/tmp/viz-out");
    fireEvent.click(screen.getByRole("button", { name: "Export nodes and edges (CSV)" }));
    expect(await screen.findByTestId("viz-export-done")).toHaveTextContent("/tmp/viz-out");
    const call = core.calls.find((c) => c.cmd === "viz.exportCsv")!.args as { nodes: unknown[]; edges: unknown[]; layout: string };
    expect(call.nodes).toHaveLength(25);
    expect(call.edges).toHaveLength(24);
    expect(call.layout).toBe("crawlTree");
  });

  // spec: UI-093
  it("Verzeichnisbaum: Legende nennt hohle Pfadknoten", async () => {
    await openWindow("directoryTree");
    await screen.findByText("8 nodes");
    expect(screen.getByTestId("viz-legend")).toHaveTextContent("Directory without crawled URL");
    expect(graphCalls()[0]).toMatchObject({ kind: "directoryTree" });
  });
});

describe("Ankertext-Wortwolke", () => {
  // spec: UI-099
  it("zeigt Wolke und Tabelle Term/Count; Klick auf einen Begriff zeigt die gefilterten Inlinks im Hauptfenster", async () => {
    await openWindow("anchorWords", 5);
    const table = await screen.findByTestId("viz-terms");
    expect(within(table).getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["Term", "Count"]);
    expect(within(table).getAllByRole("row")).toHaveLength(7);
    expect(core.calls.find((c) => c.cmd === "viz.words")!.args).toMatchObject({ kind: "anchors", urlId: 5, mode: "phrase", removeStopwords: true, maxTerms: 100, includeNofollow: true });
    const received = new Promise((resolve) => {
      const ch = new BroadcastChannel("os-viz");
      ch.onmessage = (e) => {
        ch.close();
        resolve(e.data);
      };
    });
    fireEvent.click(within(table).getByRole("button", { name: "see our prices" }));
    await expect(received).resolves.toEqual({ type: "inlinksAnchor", urlId: 5, anchor: "see our prices" });
    expect(core.calls.find((c) => c.cmd === "viz.focusInMain")!.args).toEqual({ urlId: 5 });
    await waitFor(() => expect(screen.getAllByRole("button", { name: /links\. Show these inlinks/ }).length).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("radio", { name: "Single word" }));
    await waitFor(() => expect(core.calls.filter((c) => c.cmd === "viz.words").at(-1)!.args).toMatchObject({ mode: "word" }));
  });
});

describe("Einbindung im Hauptfenster", () => {
  // spec: UI-090
  it("Menü „Visualisierungen“ öffnet je Aufruf ein eigenes Fenster (mehrere gleichzeitig)", async () => {
    setClient(createClient(core.transport("main")));
    const opened: string[] = [];
    vi.spyOn(window, "open").mockImplementation(((url: string, name: string) => {
      opened.push(`${name} ${url}`);
      return null;
    }) as typeof window.open);
    render(<VizHost />);
    const model = buildMenuModel(allMenuContributions(), { platform: "mac", lang: "en", ctx: { hasCrawl: true }, t: (k: string) => k, native: false });
    const viz = model.menus.find((m) => m.id === "visualisations");
    expect(viz?.items.map((i) => i.id)).toEqual(["viz.open.crawlTree", "viz.open.crawlGraph", "viz.open.directoryTree"]);
    for (const kind of ["crawlTree", "crawlGraph", "directoryTree", "crawlTree"]) runAction(`viz.open.${kind}`);
    await waitFor(() => expect(opened).toHaveLength(4));
    expect(new Set(opened.map((o) => o.split(" ")[0])).size).toBe(4);
    expect(opened[1]).toContain("kind=crawlGraph");
  });
});
