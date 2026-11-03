// M3-V02: Verzeichnisgraph (UI-094), Links eines Knotens (UI-097), 3D-Fenster ohne WebGL (UI-098), Body-Wortwolke (UI-100)
// gegen den Test-Kern (FakeVizCore) wie in VizWindow.test.tsx.
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createClient, getClient, setClient } from "../../ipc/client";
import { connectWindowEvents } from "../../ipc/events";
import { FakeVizCore } from "./fixtures/fakeCore";
import { fxUiDirs, fxUiWords, fxUiWordsGraph } from "./fixtures/synthetic";
import { windowLabelFor, type VizWindowKind } from "./open";
import { VizWindow } from "./VizWindow";

let core: FakeVizCore;
let offEvents: () => void = () => undefined;

async function openWindow(kind: VizWindowKind, rootUrlId: number | null = null) {
  const { snapshotId } = (await core.invoke("viz.open", { kind, rootUrlId })) as { snapshotId: string };
  const label = windowLabelFor(snapshotId);
  setClient(createClient(core.transport(label)));
  offEvents = connectWindowEvents(getClient(), label);
  render(<VizWindow params={{ snapshotId, kind, rootUrlId, label }} exportDir="<export-dir>" />);
  return snapshotId;
}

const calls = (cmd: string) => core.calls.filter((c) => c.cmd === cmd).map((c) => c.args as Record<string, unknown>);

afterEach(() => {
  offEvents();
  cleanup();
  setClient(null);
  vi.restoreAllMocks();
});

describe("Kraftgerichteter Verzeichnisgraph", () => {
  // spec: UI-094
  it("lädt die Verzeichnisstruktur als Graph mit allen Ebenen, hohlen Pfadknoten in der Legende und den Graph-Optionen", async () => {
    core = new FakeVizCore(fxUiWordsGraph, fxUiDirs);
    await openWindow("directoryGraph");
    expect(await screen.findByTestId("viz-node-count")).toHaveTextContent("8 nodes");
    expect(calls("viz.graph")[0]).toMatchObject({ kind: "directoryGraph", expandDepth: 1_000_000, budget: 10_000 });
    expect(screen.getByTestId("viz-legend")).toHaveTextContent("Directory without crawled URL");
    // Optionen aus UI-092 und UI-095
    for (const label of ["Base radius (px)", "Decrease per level", "Repulsion", "Spring length (px)", "Show labels from zoom", "Random seed", "Colour by", "Scaling metric"]) expect(screen.getByLabelText(label)).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Scale node size by metric" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Pause simulation" })).toBeInTheDocument();
    expect(await screen.findByRole("img", { name: /Force-directed directory graph with 8 nodes/ })).toBeInTheDocument();
  });
});

describe("Links eines Knotens einblenden", () => {
  // spec: UI-097
  it("Kontextmenü „Inlinks zeigen“ blendet genau die Unique Inlinks ein, graut die übrigen aus; Zurücksetzen entfernt sie", async () => {
    core = new FakeVizCore(fxUiWordsGraph, fxUiDirs, fxUiWords);
    await openWindow("crawlGraph");
    await screen.findByText("26 nodes");
    await waitFor(() => expect(window.__osViz?.screenOf(2)).toBeTruthy());
    const [x, y] = window.__osViz!.screenOf(2)!;
    fireEvent.contextMenu(screen.getByRole("img", { name: /crawl graph/i }), { clientX: x, clientY: y });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Show inlinks" }));
    await waitFor(() => expect(window.__osViz?.links?.edges).toBe(23));
    expect(calls("viz.graph").at(-1)).toMatchObject({ linksNode: 2, linksMode: "inlinks", linksIncludeNofollow: true, linksExcludeNavFooter: false });
    expect(window.__osViz!.links!.total).toBe(23);
    expect(screen.getByTestId("viz-links-state")).toHaveTextContent("Show inlinks: 23 links");
    // Verbundene Knoten hervorgehoben (Zielseite + 22 Quellen), ausgegraut bleiben /p22, /deep-1, /deep-2.
    expect(window.__osViz!.links!.connected.sort((a, b) => a - b)).toEqual(Array.from({ length: 23 }, (_, i) => i + 1));
    expect(window.__osViz!.svg()).toMatch(/class="edges-inlink"/);
    expect(screen.getByTestId("viz-legend")).toHaveTextContent("Inlink");
    // Optionen wirken sofort
    fireEvent.click(screen.getByRole("checkbox", { name: "Include nofollow links" }));
    await waitFor(() => expect(window.__osViz?.links?.edges).toBe(18));
    fireEvent.click(screen.getByRole("checkbox", { name: "Exclude navigation and footer links" }));
    await waitFor(() => expect(window.__osViz?.links?.edges).toBe(17));
    // CSV enthält die Zusatzkanten mit Art
    expect(window.__osViz!.csv()!.edges.filter((e) => e.edgeType === "inlink")).toHaveLength(17);
    // Struktur unverändert: kein Neustart mit neuem Baum (Knotenzahl gleich)
    expect(screen.getByTestId("viz-node-count")).toHaveTextContent("26 nodes");
    fireEvent.click(screen.getByRole("button", { name: "Reset links" }));
    await waitFor(() => expect(window.__osViz?.links).toBeNull());
    expect(calls("viz.graph").at(-1)!.linksNode).toBeUndefined();
    expect(window.__osViz!.svg()).not.toMatch(/edges-inlink|node dim/);
  });

  // spec: UI-097
  it("Outlinks und Outlinks von Kindern: Kanten vom Knoten bzw. von seinen Kindern", async () => {
    core = new FakeVizCore(fxUiWordsGraph, fxUiDirs, fxUiWords);
    await openWindow("crawlGraph");
    await screen.findByText("26 nodes");
    await waitFor(() => expect(window.__osViz?.screenOf(2)).toBeTruthy());
    const canvas = screen.getByRole("img", { name: /crawl graph/i });
    for (const [label, edges] of [["Show outlinks", 7], ["Show outlinks from children", 2], ["Show inlinks to children", 3]] as const) {
      const [x, y] = window.__osViz!.screenOf(2)!;
      fireEvent.contextMenu(canvas, { clientX: x, clientY: y });
      fireEvent.click(await screen.findByRole("menuitem", { name: label }));
      await waitFor(() => expect(window.__osViz?.links?.edges).toBe(edges));
    }
  });
});

describe("3D-Varianten", () => {
  // spec: UI-098
  it("ohne WebGL 2 erscheint ein Hinweis statt einer leeren Fläche; Knotengrenze 100 000, Export nur PNG", async () => {
    core = new FakeVizCore(fxUiWordsGraph, fxUiDirs);
    // Die 3D-Ansicht fragt nur WebGL-Kontexte an; ohne WebGL liefert die Umgebung `null`.
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    await openWindow("crawlGraph3d");
    expect(await screen.findByTestId("viz-no-webgl")).toHaveTextContent("needs WebGL 2");
    expect(calls("viz.graph")[0]).toMatchObject({ kind: "crawlGraph3d", budget: 100_000 });
    expect(screen.getByLabelText("Node limit")).toHaveValue(100000);
    expect(screen.queryByRole("button", { name: "SVG" })).toBeNull();
    expect(screen.queryByRole("button", { name: /CSV/ })).toBeNull();
    expect(screen.getByRole("button", { name: "PNG 1×" })).toBeDisabled();
    // Budget über 100 000 wird begrenzt
    const limit = screen.getByLabelText("Node limit");
    fireEvent.change(limit, { target: { value: "250000" } });
    fireEvent.blur(limit);
    await waitFor(() => expect(calls("viz.graph").at(-1)).toMatchObject({ budget: 100_000 }));
  });
});

describe("Body-Text-Wortwolke", () => {
  // spec: UI-100
  it("Standardoptionen Stoppwörter an, Mindestlänge 3, max. 100; Tabelle Term/Count; Optionen gehen an den Kern", async () => {
    core = new FakeVizCore(fxUiWordsGraph, fxUiDirs);
    await openWindow("bodyWords", 2);
    await waitFor(() => expect(screen.getByTestId("viz-terms").querySelectorAll("tbody tr")).toHaveLength(17));
    expect(calls("viz.words")[0]).toMatchObject({ kind: "body", urlId: 2, removeStopwords: true, minLength: 3, maxTerms: 100 });
    expect(screen.getByRole("checkbox", { name: "Remove stop words" })).toBeChecked();
    expect(screen.queryByRole("radio", { name: "Single word" })).toBeNull();
    expect(screen.queryByRole("checkbox", { name: "Include nofollow links" })).toBeNull();
    const min = screen.getByLabelText("Minimum word length");
    fireEvent.change(min, { target: { value: "5" } });
    fireEvent.blur(min);
    await waitFor(() => expect(calls("viz.words").at(-1)).toMatchObject({ minLength: 5 }));
    await waitFor(() => expect(screen.getByTestId("viz-terms").querySelectorAll("tbody tr").length).toBeLessThan(17));
    expect(screen.getByTestId("viz-terms")).toHaveTextContent("pricing");
  });

  // spec: UI-100
  it("ohne gespeichertes HTML erscheint der Hinweis HTML_NOT_STORED (UI-016) statt einer Fehlermeldung", async () => {
    core = new FakeVizCore(fxUiWordsGraph, fxUiDirs);
    core.bodyWords = false;
    await openWindow("bodyWords", 2);
    await waitFor(() => expect(document.querySelector('[data-empty-state="HTML_NOT_STORED"]')).not.toBeNull());
    expect(document.querySelector('[data-empty-state="NO_DATA_YET"]')).toBeNull();
    expect(screen.queryByText(/HTML is not stored for this URL/)).toBeNull();
  });

  // spec: UI-100
  // spec: UI-090 [M3]
  it("ohne URL (Menü, Wurzel = Start-URL) bestimmt das Fenster die Wurzel des Schnappschusses", async () => {
    core = new FakeVizCore(fxUiWordsGraph, fxUiDirs);
    await openWindow("bodyWords", null);
    await waitFor(() => expect(calls("viz.words")[0]).toMatchObject({ kind: "body", urlId: 1 }));
    expect(calls("viz.graph")[0]).toMatchObject({ kind: "crawlTree", budget: 1, expandDepth: 0 });
    await act(async () => undefined);
  });
});
