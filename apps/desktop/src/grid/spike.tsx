// Dedicated measurement entry, shipped only by spike.vite.ts; no fixture switch in product UI.
import "../ui/tokens/base.css";
import { StrictMode, useCallback, useState } from "react";
import { createRoot } from "react-dom/client";
import { createBridgeClient, createTauriClient, setClient } from "../ipc/client";
import { isTauri } from "../ipc/transport/tauri";
import { view } from "../ipc/view";
import { ViewDataSource, LOADING } from "../data/ViewDataSource";
import { SelectionModel } from "../state/selection";
import { resolveTokens } from "../ui/tokens/tokens";
import { defaultSetZoom } from "../app/appearance";
import { DataGrid, defaultColumns, type GridController } from "./DataGrid";
import { paint, recordFrames } from "./recorder";

const bridge = new URLSearchParams(location.search).get("bridge");
if (isTauri()) setClient(createTauriClient());
else if (bridge) setClient(createBridgeClient(bridge));
else throw new Error("Grid gate requires a real stub-views bridge or native app");
const handle = await view.open({ spec: { kind: "tab", tab: "Internal", filter: "All", sort: [] }, columns: [] });
const source = new ViewDataSource(handle);
let controller: GridController | null = null;
let selection = SelectionModel.empty();
let zoom = 1;
const errors: string[] = [];
const failures: string[] = [];
const capture = (error: unknown) => { errors.push(String(error)); };
window.addEventListener("error", event => capture(event.message));
window.addEventListener("unhandledrejection", event => capture(event.reason));
function SnapshotApp() {
  const [columns, setColumns] = useState(defaultColumns(handle.columns));
  const [selected, setSelected] = useState(selection);
  const connect = useCallback((next: GridController | null) => { controller = next; }, []);
  return <main style={{ height: "100vh", display: "flex", flexDirection: "column", fontFamily: "system-ui" }}>
    <nav style={{ height: 36, flexShrink: 0 }}>
      <button onClick={() => controller?.scrollTo(source.total - 1)}>Last row</button>
      <button onClick={() => void window.gridSpike.record("vertical").then(r => { document.querySelector("output")!.textContent = JSON.stringify(r); })}>Record vertical 10 s</button>
      <button onClick={() => void window.gridSpike.record("horizontal").then(r => { document.querySelector("output")!.textContent = JSON.stringify(r); })}>Record horizontal 10 s</button>
      <select aria-label="Zoom" defaultValue="1" onChange={event => void window.gridSpike.zoom(Number(event.target.value))}>
        {[0.8, 1, 1.25, 1.5, 2].map(n => <option key={n} value={n}>{n * 100}%</option>)}
      </select>
      <output style={{ fontSize: 11 }} />
    </nav>
    <div style={{ flex: 1, minHeight: 0 }}><DataGrid source={source} columns={columns} selection={selected}
      onSelectionChange={next => { selection = next; setSelected(next); }} onColumnsChange={setColumns}
      onSortChange={sort => { void view.update({ viewId: source.viewId, spec: { kind: "tab", tab: "Internal", filter: "All", sort } }).then(h => {
        source.setSorted(sort.length > 0); setColumns(old => old.map(c => ({ ...c, sort: sort.find(s => s.column === c.key)?.dir })));
        return source.onViewChanged(h.version, h.total ?? 0);
      }).catch(capture); }}
      onContextMenu={() => undefined} onActivate={() => undefined} onError={capture}
      confirmLargeCopy={async () => false} onExportSelection={() => failures.push("export-required")}
      rowHeight={26} theme={{ tokens: resolveTokens("light", "#2f855a"), fontFamily: "system-ui", fontSize: 12, zebra: true }}
      ariaLabel="Grid gate fixture" locale="en" controller={connect} /></div>
  </main>;
}
function scrollElement(): HTMLElement {
  const elements = Array.from(document.querySelectorAll<HTMLElement>(".os-data-grid *"));
  const element = elements.find(e => e.scrollHeight > e.clientHeight + 1000 && getComputedStyle(e).overflowY !== "visible");
  if (!element) throw new Error("Grid scroll element missing");
  return element;
}
const api = {
  activate: async () => {
    if (isTauri()) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().show();
      await getCurrentWindow().setFocus();
    }
    return { visibility: document.visibilityState, focus: document.hasFocus() };
  },
  ready: () => controller !== null && source.getCell(0, 0) !== LOADING,
  errors, failures,
  snapshot: () => ({ total: source.total, columns: source.columns.length, visible: controller?.visible(),
    selection: selection.value, focus: selection.focus, cachedRows: source.cachedRows,
    last: source.getCell(0, source.total - 1) === LOADING ? "loading" : source.getCell(0, source.total - 1),
    rowCount: document.querySelector('[role="grid"]')?.getAttribute("aria-rowcount"),
    visibility: document.visibilityState, documentFocus: document.hasFocus(), innerWidth, innerHeight,
    domRows: document.querySelectorAll('[role="row"]').length, dpr: devicePixelRatio, zoom,
    canvas: Array.from(document.querySelectorAll("canvas")).filter(c => c.getBoundingClientRect().width > 0).map(c => {
      const bounds = c.getBoundingClientRect();
      return { width: c.width, height: c.height, cssWidth: bounds.width, cssHeight: bounds.height,
        widthError: c.width - bounds.width * devicePixelRatio, heightError: c.height - bounds.height * devicePixelRatio };
    }), cellWidth: controller ? controller.bounds(1, controller.visible().y)!.width : null,
    bounds: controller?.bounds(1, controller.visible().y), headerBounds: controller?.bounds(1, -1) }),
  bounds: (column: number, row: number) => controller?.bounds(column, row),
  jump: async (row: number, column = 0) => {
    const start = performance.now();
    if (column < 2) scrollElement().scrollLeft = 0;
    controller?.scrollTo(row, column);
    while (source.getCell(column, row) === LOADING && performance.now() - start < 5000) await paint();
    await paint();
    const region = controller?.visible();
    return { requestedRow: row, requestedVisible: !!region && region.y <= row && region.y + region.height > row,
      contentReady: source.getCell(column, row) !== LOADING, contentMs: performance.now() - start, ...api.snapshot() };
  },
  zoom: async (factor: number) => { zoom = factor; const choice = document.querySelector<HTMLSelectElement>('select[aria-label="Zoom"]'); if (choice) choice.value = String(factor); await defaultSetZoom(factor, document.documentElement); await paint(); await paint(); return api.snapshot(); },
  record: async (axis: "vertical" | "horizontal") => {
    const element = scrollElement(); element.scrollTop = 0; element.scrollLeft = 0; await paint();
    const metrics = await recordFrames(elapsed => {
      if (axis === "vertical") element.scrollTop = elapsed * 1.5;
      else element.scrollLeft = Math.min(element.scrollWidth - element.clientWidth, elapsed * (element.scrollWidth - element.clientWidth) / 10_000);
    });
    return { axis, userAgent: navigator.userAgent, native: isTauri(), ...metrics, ...api.snapshot() };
  },
};
declare global { interface Window { gridSpike: typeof api } }
window.gridSpike = api;
createRoot(document.getElementById("root")!).render(new URLSearchParams(location.search).get("strict") === "0" ? <SnapshotApp /> : <StrictMode><SnapshotApp /></StrictMode>);
window.addEventListener("pagehide", () => { void source.dispose(); });
