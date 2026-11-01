import { render } from "@testing-library/react";
import { expect, it } from "vitest";
import { GraphCanvas } from "./GraphCanvas";
import { fxUiBasic, selectGraph } from "./fixtures/synthetic";
import { VizTree } from "./model/tree";
import { buildScene } from "./model/scene";
import { DEFAULT_STYLE } from "./model/render";

// spec: UI-144
it("releases a fixture canvas backing store when its window unmounts", () => {
  const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10000, expandDepth: 3, metric: "crawl_depth" }));
  const scene = buildScene(tree, new Map(), { layout: "force", colorMode: "indexability", palette: "standard", sizeByMetric: false, minRadius: 3, maxRadius: 30, r0: 14, f: 0.8 }, "path");
  const view = render(<GraphCanvas scene={scene} version={0} style={DEFAULT_STYLE} labelsAt={() => false} highlights={new Set()} current={null} draggable={false} label="fixture" initialTransform={() => ({ k: 1, x: 0, y: 0 })} />);
  const canvas = view.container.querySelector("canvas")!;
  canvas.width = 1280; canvas.height = 800;
  view.unmount();
  expect(canvas.width).toBe(0); expect(canvas.height).toBe(0);
});

// spec: UI-144
it("closing a fixture graph during CSV export drops the tick listener and debug scene", async () => {
  const { act, fireEvent, screen, waitFor } = await import("@testing-library/react");
  const { vi } = await import("vitest");
  const { FakeVizCore } = await import("./fixtures/fakeCore");
  const { createClient, setClient } = await import("../../ipc/client");
  const { eventBus } = await import("../../ipc/events");
  const { GraphView } = await import("./GraphView");
  const core = new FakeVizCore();
  const { snapshotId } = await core.invoke("viz.open", { kind: "crawlTree" }) as { snapshotId: string };
  const invoke = core.invoke.bind(core);
  vi.spyOn(core, "invoke").mockImplementation(async (cmd, args, window) => cmd === "viz.exportCsv"
    ? { opId: "pending-export", labelKey: "op-viz-export", cancellable: true }
    : invoke(cmd, args, window));
  setClient(createClient(core.transport()));
  const before = eventBus.listenerCount("tick");
  const view = render(<GraphView kind="crawlTree" snapshotId={snapshotId} generation={0} initialRoot={null} exportDir="fixture-output" />);
  try {
    await screen.findByText("25 nodes");
    fireEvent.click(screen.getByRole("button", { name: "Export nodes and edges (CSV)" }));
    await waitFor(() => expect(eventBus.listenerCount("tick")).toBe(before + 1));
    expect(window.__osViz?.nodes).toBe(25);
    await act(async () => view.unmount());
    expect(eventBus.listenerCount("tick")).toBe(before);
    expect(window.__osViz).toBeUndefined();
  } finally { view.unmount(); setClient(null); vi.restoreAllMocks(); }
});
