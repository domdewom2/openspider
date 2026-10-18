import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEffect, useState } from "react";
import { createClient, setClient } from "../../../ipc/client";
import { useSession } from "../../../state/session";
import { SelectionModel } from "../../../state/selection";
import { setLanguage } from "../../../i18n";
import type { DataGridProps } from "../../../grid/DataGrid";
import type { ViewSpec } from "../../../ipc/gen/ViewSpec";
import type { DetailActions } from "./types";
import { DetailFrame } from "./DetailFrame";
import deepPath from "./fixtures/fx-ui-depth.json";
import { detailFixtureCore } from "./fixtures/core";
import { DEFAULT_LINK_OPTIONS, linkSpec } from "../anchors/model";
import { URL_DETAIL_FIELDS } from "../url-details/UrlDetails";
import { headerPreview } from "../headers/Headers";
import { COOKIE_COLUMNS } from "../cookies/Cookies";
import { IMAGE_COLUMNS } from "../images/Images";
import { updateDetailState } from "./persistence";
import { normalizeDetailPreferences } from "./preferences";
import { DETAIL_NAVIGATE_EVENT, setDetailSelection } from "./selection";

// Keep the shared bounded data source; substitute only the canvas when testing the feature in jsdom.
vi.mock("../../../grid/DataGrid", async importOriginal => {
  const original = await importOriginal<typeof import("../../../grid/DataGrid")>();
  return { ...original, DataGrid: function FixtureGrid(p: DataGridProps) {
    const [, refresh] = useState(0);
    useEffect(() => p.source.subscribe(() => refresh(n => n + 1)), [p.source]);
    return <table aria-label={p.ariaLabel} data-total={p.source.total}><thead><tr>{p.columns.filter(c => c.visible).map(c => <th key={c.key}>{c.key}</th>)}</tr></thead><tbody>
      {Array.from({ length: Math.min(p.source.total, 4) }, (_, row) => <tr key={row} data-testid={`detail-row-${row}`}
        onClick={() => { const id = p.source.rowKey(row); if (id !== undefined) p.onSelectionChange(SelectionModel.empty().select(id)); }}
        onDoubleClick={() => { const id = p.source.rowKey(row); if (id !== undefined) p.onActivate(id, "From"); }}>
        {p.columns.filter(c => c.visible).map(c => <td key={c.key} onContextMenu={e => { e.preventDefault(); p.onContextMenu({ column: c.key, row, x: 1, y: 1 }); }}>{String(p.source.getCell(p.source.columns.findIndex(meta => meta.key === c.key), row) ?? "")}</td>)}
      </tr>)}
    </tbody></table>;
  } };
});
let fixture: ReturnType<typeof detailFixtureCore>;
let actions: DetailActions;
const selection = { urlIds: [1, 2, 3], focus: 2, topTab: "Internal", crawlId: "fixture" };
async function settle(ms = 100) { await act(async () => { await vi.advanceTimersByTimeAsync(ms); }); }
async function selectTab(name: string) { fireEvent.click(screen.getByRole("tab", { name, hidden: true })); await settle(); }
const opens = () => fixture.core.calls.filter(c => c.cmd === "view.open");
const lastSpec = () => (opens().at(-1)!.args as { spec: ViewSpec }).spec;
beforeEach(async () => {
  vi.useFakeTimers(); fixture = detailFixtureCore(); setClient(createClient(fixture.core.transport()));
  actions = { navigate: vi.fn(), back: vi.fn(), openBrowser: vi.fn(), export: vi.fn(), reportError: vi.fn() };
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockResolvedValue(undefined) } });
  await setLanguage("en");
});
afterEach(async () => { cleanup(); await settle(0); setClient(null); vi.useRealTimers(); setDetailSelection({ urlIds: [] }); });

describe("detail frame and URL properties", () => {
  // spec: UI-030
  it("does not request data without a selection and preserves the standard tab order", async () => {
    render(<DetailFrame actions={actions} />); await settle();
    expect(screen.getByText("Select a row in the table above to see its details.")).toBeInTheDocument();
    expect(fixture.core.calls.filter(c => c.cmd.startsWith("detail.") || c.cmd.startsWith("view.")).length).toBe(0);
    const list = screen.getByRole("tablist"); expect(within(list).getAllByRole("tab", { hidden: true }).slice(0, 5).map(e => e.textContent)).toEqual(["URL Details", "Inlinks", "Outlinks", "Image Details", "Resources"]);
  });
  // spec: UI-145
  it("debounces fast changes for 100 ms and only requests the visible tab", async () => {
    const { rerender } = render(<DetailFrame selection={selection} actions={actions} />);
    await settle(50); rerender(<DetailFrame selection={{ ...selection, focus: 3 }} actions={actions} />);
    await settle(99); expect(fixture.core.calls.filter(c => c.cmd === "detail.urlDetails")).toHaveLength(0);
    await settle(1); expect(fixture.core.calls.filter(c => c.cmd === "detail.urlDetails").map(c => c.args)).toEqual([{ urlId: 3 }]); expect(opens()).toHaveLength(0);
  });
  // spec: UI-031
  it("shows every canonical property in order including empty fields and navigates redirects", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle();
    const table = screen.getByRole("table");
    expect(within(table).getAllByRole("rowheader").map(e => e.textContent)).toEqual(URL_DETAIL_FIELDS);
    expect(within(table).getByRole("rowheader", { name: "Content Type" }).parentElement?.textContent?.trim()).toBe("Content Type");
    fireEvent.click(screen.getByRole("button", { name: "https://site.test/final" })); expect(actions.navigate).toHaveBeenCalledWith({ address: "https://site.test/final", tab: "auto" });
  });
  // spec: UI-081
  it("displays the Link Score provided by the core", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle();
    expect(screen.getByRole("rowheader", { name: "Link Score" }).parentElement).toHaveTextContent("73");
  });
  // spec: UI-031
  it("copies a property value and its name through the property context menu", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle();
    const row = screen.getByRole("rowheader", { name: "Link Score" }).parentElement!;
    fireEvent.contextMenu(within(row).getByText("73")); await settle(0);
    fireEvent.click(screen.getByRole("menuitem", { name: "Copy name and value" })); await settle(0);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Link Score\t73");
  });
  // spec: UI-145
  it("rejects a late response from the previous selection", async () => {
    let complete!: (value: unknown) => void;
    fixture.core.handlers.set("detail.urlDetails", () => new Promise(resolve => { complete = resolve; }));
    const { rerender } = render(<DetailFrame selection={selection} actions={actions} />); await settle();
    rerender(<DetailFrame selection={{ ...selection, focus: 99 }} actions={actions} />);
    fixture.core.handlers.set("detail.urlDetails", () => [{ name: "Address", value: "current" }]);
    await act(async () => complete([{ name: "Address", value: "obsolete" }])); await settle();
    expect(screen.getByText("current")).toBeInTheDocument(); expect(screen.queryByText("obsolete")).toBeNull();
  });
  // spec: ONPAGE-192 [M2]
  it("uses the shared URL selection from any top tab and shows a non-applicable empty state", async () => {
    setDetailSelection({ ...selection, topTab: "Response Codes" });
    render(<DetailFrame actions={actions} />); await settle(); await selectTab("Inlinks");
    expect(lastSpec()).toMatchObject({ kind: "inlinks", urlIds: [1, 2, 3] });
    await act(async () => setDetailSelection({ ...selection, topTab: "Page Titles" })); await settle();
    await act(async () => setDetailSelection({ ...selection, related: false })); await settle();
    expect(document.querySelector('[data-empty-state="NO_SELECTION"]')).not.toBeNull();
  });
  // spec: UI-030
  it("persists reordered tabs after one second and preserves unrelated state", async () => {
    fixture.core.uiState = { version: 1, state: { unrelated: { x: 7 } } };
    render(<DetailFrame selection={selection} actions={actions} />); await settle();
    fireEvent.keyDown(screen.getByRole("tablist").parentElement!, { key: "ArrowRight", altKey: true, shiftKey: true }); await settle(1000);
    expect(fixture.core.uiState.state).toMatchObject({ unrelated: { x: 7 } });
    expect((fixture.core.uiState.state as { detail: { order: string[] } }).detail.order.slice(0, 2)).toEqual(["inlinks", "urlDetails"]);
    const normalized = normalizeDetailPreferences({ order: ["outlinks", "outlinks", "unknown"], hidden: ["urlDetails", "cookies"] });
    expect(normalized.order.slice(0, 4)).toEqual(["outlinks", "urlDetails", "inlinks", "images"]); expect(normalized.hidden).toEqual(["cookies"]);
  });
});

describe("paged tables", () => {
  // spec: UI-032
  it("loads the union of selected URLs and keeps repeated inlinks as separate rows", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Inlinks");
    expect(lastSpec()).toMatchObject({ kind: "inlinks", urlIds: [1, 2, 3] });
    const table = screen.getByRole("table", { name: "Inlinks" }); expect(within(table).getAllByText("https://site.test/source")).toHaveLength(2);
    expect(within(table).queryByRole("columnheader", { name: "Link Crawlability" })).toBeNull();
    fireEvent.doubleClick(screen.getByTestId("detail-row-0")); await settle(0);
    expect(actions.navigate).toHaveBeenCalledWith({ address: "https://site.test/source", tab: "auto" });
  });
  // spec: UI-033
  it("shows external and uncrawlable destinations with status, Target and a crawlability filter", async () => {
    render(<DetailFrame selection={selection} actions={actions} storeUncrawlable />); await settle(); await selectTab("Outlinks");
    expect(screen.getByRole("columnheader", { name: "Target" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Link Crawlability" })).toBeInTheDocument();
    expect(screen.getAllByText("Uncrawlable")).toHaveLength(2);
    fireEvent.doubleClick(screen.getByTestId("detail-row-0")); await settle(0);
    expect(actions.navigate).toHaveBeenCalledWith({ address: "https://outside.test/a?x=2", tab: "auto" });
    fireEvent.keyDown(screen.getByRole("region", { name: "Details" }), { key: "ArrowLeft", altKey: true }); expect(actions.back).toHaveBeenCalledOnce();
  });
  // spec: ONPAGE-036
  it("displays the link's Target value", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Outlinks"); expect(screen.getByText("_blank")).toBeInTheDocument();
  });
  // spec: CRAWL-103
  it("displays Original URL alongside the rewritten destination", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Outlinks");
    expect(screen.getByText("https://outside.test/a?sid=1&x=2")).toBeInTheDocument(); expect(screen.getByText("https://outside.test/a?x=2")).toBeInTheDocument();
  });
  // spec: INTEG-084
  it("includes Link Path, custom Link Position and sends the position filter to the core", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Outlinks");
    expect(screen.getByRole("columnheader", { name: "Link Path" })).toBeInTheDocument(); expect(screen.getAllByText("Editorial").length).toBeGreaterThan(0);
    const spec = linkSpec("inlinks", [1], { ...DEFAULT_LINK_OPTIONS, position: "Editorial", excludeNofollow: true }, "", []);
    expect(spec).toMatchObject({ filter: { position: "Editorial", excludeNofollow: true } });
  });
  // spec: UI-034
  it("provides the Links/Anchors switch", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Inlinks");
    expect(screen.getByRole("button", { name: /View/ })).toBeInTheDocument();
  });
  // spec: TECH-146
  it("requests anchor grouping and deterministic sorting without folding case", () => {
    expect(linkSpec("outlinks", [1, 2], { ...DEFAULT_LINK_OPTIONS, mode: "anchors", excludeSelf: true }, "", [])).toMatchObject({
      kind: "outlinks", urlIds: [1, 2], filter: { mode: "anchors", excludeSelf: true, excludeNofollow: false }, sort: [{ column: "Links", dir: "desc" }, { column: "Anchor Text", dir: "asc" }],
    });
    expect(fixture.fixture.anchors.map(row => row["Anchor Text"])).toEqual(["Produkte", "produkte"]);
  });
  // spec: UI-145
  it("loads a bounded first window for one million links and closes the view on tab change", async () => {
    fixture.options.total = 1_000_000; render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Inlinks");
    const reads = fixture.core.calls.filter(c => c.cmd === "view.rows").map(c => c.args as { offset: number; limit: number });
    expect(reads.length).toBeGreaterThan(0); expect(reads.every(read => read.limit <= 200 && read.offset < 1000)).toBe(true);
    await selectTab("URL Details"); expect(fixture.views.size).toBe(0); expect(fixture.core.calls.some(c => c.cmd === "view.close")).toBe(true);
  });
  // spec: TECH-089
  it("shows annotation confirmation statuses and Address for a multi-selection in Hreflang", async () => {
    render(<DetailFrame selection={{ ...selection, topTab: "Hreflang" }} actions={actions} />); await settle();
    expect(lastSpec()).toMatchObject({ kind: "detail", detail: "hreflang", urlIds: [1, 2, 3] });
    expect(screen.getByRole("columnheader", { name: "Address" })).toBeInTheDocument(); expect(screen.getByText("Non-200")).toBeInTheDocument(); expect(screen.getByText("Confirmed")).toBeInTheDocument();
  });
  // spec: UI-036
  it("shows blocked rendering resources and the responsible block reason", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Resources");
    expect(lastSpec()).toMatchObject({ kind: "resources", urlIds: [1, 2, 3] }); expect(screen.getByText("robots.txt")).toBeInTheDocument(); expect(screen.getByText("Rendered HTML")).toBeInTheDocument();
  });
  // spec: UI-043
  it("shows focus cookies with default hidden Path/Max-Age and HTTP/JavaScript attribution", async () => {
    render(<DetailFrame selection={selection} actions={actions} rendering={false} />); await settle(); await selectTab("Cookies");
    expect(lastSpec()).toMatchObject({ kind: "detail", detail: "cookies", urlIds: [2] });
    expect(screen.queryByRole("columnheader", { name: "Max-Age" })).toBeNull(); expect(screen.queryByRole("columnheader", { name: "Path" })).toBeNull();
    expect(screen.getByText("JavaScript cookies require JavaScript rendering.")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "First-Party" })).toBeInTheDocument(); expect(screen.getByRole("columnheader", { name: "SameSite" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Export details" }));
    expect(actions.export).toHaveBeenCalledWith(expect.objectContaining({ detail: "cookies" }), { kind: "ids", ids: [2] });
    expect(COOKIE_COLUMNS).toContain("Max-Age");
  });
  // spec: UI-111
  it("uses full From/To values and the stored XPath in context actions", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Outlinks");
    fireEvent.contextMenu(screen.getByText("https://outside.test/a?x=2")); await settle(0);
    fireEvent.click(screen.getByRole("menuitem", { name: "Show link element in source" })); await settle(0);
    expect(actions.navigate).toHaveBeenCalledWith({ address: "https://site.test/redirect", tab: "auto", source: true, linkPath: "/html[1]/body[1]/main[1]/a[1]" });
  });
  // spec: UI-050
  it("shows a crawl path with redirects and the direct-upload hint", async () => {
    render(<DetailFrame selection={selection} actions={actions} listMode />); await settle();
    fireEvent.click(screen.getByRole("button", { name: "Crawl Path" })); await settle();
    expect(screen.getByText("HTTP Redirect")).toBeInTheDocument(); expect(screen.getByText(/Directly uploaded/)).toBeInTheDocument();
  });
  // spec: OUT-023
  it("exports a crawl-path report for the selected URLs in selection order", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle();
    fireEvent.click(screen.getByRole("button", { name: "Crawl Path" })); await settle(); fireEvent.click(screen.getByRole("button", { name: "Export details" }));
    expect(actions.export).toHaveBeenCalledWith(expect.objectContaining({ kind: "detail", detail: "crawlPath", urlIds: [1, 2, 3] }), { kind: "ids", ids: [1, 2, 3] });
  });
});

describe("view lifecycle and preferences", () => {
  // spec: UI-030
  it("restores saved column order and widths for the matching detail table", async () => {
    fixture.core.uiState = { version: 1, state: { detailColumns: { "inlinks:links": [
      { key: "To", width: 250, pinned: false, visible: true },
      { key: "From", width: 180, pinned: false, visible: true },
    ] } } };
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Inlinks");
    expect(within(screen.getByRole("table", { name: "Inlinks" })).getAllByRole("columnheader").slice(0, 2).map(e => e.textContent)).toEqual(["To", "From"]);
  });
  // spec: UI-050
  it("shows all eight discovery steps for a URL at depth seven", async () => {
    fixture.fixture.crawlPath = deepPath;
    render(<DetailFrame selection={selection} actions={actions} />); await settle();
    fireEvent.click(screen.getByRole("button", { name: "Crawl Path" })); await settle();
    expect(screen.getByRole("table", { name: "Crawl Path" })).toHaveAttribute("data-total", "8");
    expect(fixture.fixture.crawlPath.at(-1)?.Address).toBe("https://site.test/");
  });
  // spec: UI-030
  it("merges simultaneous tab and column writes without losing unrelated UI state", async () => {
    fixture.core.uiState = { version: 1, state: { other: 42 } };
    await Promise.all([
      updateDetailState(state => ({ ...state, detail: { hidden: ["cookies"] } })),
      updateDetailState(state => ({ ...state, detailColumns: { inlinks: [] } })),
    ]);
    expect(fixture.core.uiState.state).toEqual({ other: 42, detail: { hidden: ["cookies"] }, detailColumns: { inlinks: [] } });
  });
  // spec: UI-145
  it("renders a building view when its row count arrives in a live tick", async () => {
    const open = fixture.core.handlers.get("view.open")!;
    fixture.core.handlers.set("view.open", async (args, ctx) => ({ ...await open(args, ctx) as object, total: null, state: "building" }));
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Inlinks");
    expect(screen.queryByRole("table", { name: "Inlinks" })).toBeNull();
    const viewId = [...fixture.views.keys()][0];
    await act(async () => useSession.setState({ views: { [viewId]: { viewId, version: 2, total: 2 } } }));
    await settle(0);
    expect(screen.getByRole("table", { name: "Inlinks" })).toBeInTheDocument();
    expect(screen.getByText("Mehr")).toBeInTheDocument();
  });
  // spec: UI-145
  it("closes a late view handle even if the visible tab changed while it opened", async () => {
    const open = fixture.core.handlers.get("view.open")!;
    let complete!: () => void;
    fixture.core.handlers.set("view.open", (args, ctx) => new Promise(resolve => { complete = () => resolve(open(args, ctx)); }));
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Inlinks");
    await selectTab("URL Details");
    await act(async () => complete()); await settle(0);
    expect(fixture.views.size).toBe(0); expect(fixture.core.calls.filter(c => c.cmd === "view.close")).toHaveLength(1);
  });
  // spec: TECH-089
  it("shows the server's hreflang truncation count above the annotations", async () => {
    fixture.core.handlers.set("detail.urlDetails", () => [{ name: "Hreflang Truncated", value: 10 }]);
    render(<DetailFrame selection={{ ...selection, urlIds: [1], topTab: "Hreflang" }} actions={actions} />); await settle();
    expect(screen.getByText("10 additional annotations were not evaluated.")).toBeInTheDocument();
  });
  // spec: UI-030
  it("persists each detail table's column visibility without changing export columns", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("Cookies");
    fireEvent.click(screen.getByText("Columns", { selector: "summary" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Cookie Value" })); await settle(1000);
    expect(screen.queryByRole("columnheader", { name: "Cookie Value" })).toBeNull();
    expect(fixture.core.uiState.state).toMatchObject({ detailColumns: { cookies: expect.arrayContaining([expect.objectContaining({ key: "Cookie Value", visible: false })]) } });
    await selectTab("URL Details"); await selectTab("Cookies");
    expect(screen.queryByRole("columnheader", { name: "Cookie Value" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Export details" }));
    expect(actions.export).toHaveBeenCalledWith(expect.objectContaining({ detail: "cookies" }), { kind: "ids", ids: [2] });
  });
});

describe("headers and images", () => {
  // spec: UI-042
  it("shows headers of the focus redirect preserving repeated names and searches without case sensitivity", async () => {
    render(<DetailFrame selection={selection} actions={actions} />); await settle(); await selectTab("HTTP Headers");
    expect(screen.getByText("HTTP/2 301")).toBeInTheDocument(); expect(screen.getAllByText("set-cookie")).toHaveLength(3);
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "X-ROBOTS" } });
    expect(screen.getByText("noindex")).toBeInTheDocument(); expect(screen.queryByText("set-cookie")).toBeNull();
    expect(headerPreview("ä".repeat(5000))).toHaveLength(4097);
  });
  // spec: UI-035
  it("does not load an unstored image until explicit consent, using an img element for safe SVG display", async () => {
    fixture.options.stored = false; render(<DetailFrame selection={{ ...selection, topTab: "Images" }} actions={actions} />); await settle(); await selectTab("Image Details");
    expect(screen.queryByRole("img")).toBeNull(); fireEvent.click(screen.getByRole("button", { name: "Load image from server…" }));
    expect(screen.getByRole("img")).toHaveAttribute("src", "ospider://img/fixture/7"); expect(screen.queryByRole("object")).toBeNull();
  });
  // spec: ONPAGE-172
  it("shows image instances in parity order with empty non-rendered dimensions and hidden extensions", async () => {
    render(<DetailFrame selection={selection} actions={actions} rendering={false} />); await settle(); await selectTab("Image Details");
    const table = screen.getByRole("table", { name: "Image Details" });
    expect(within(table).getAllByRole("columnheader").map(e => e.textContent)).toEqual(IMAGE_COLUMNS.slice(0, -2));
    expect(screen.getByText("400x300")).toBeInTheDocument(); expect(screen.getByText("HTML")).toBeInTheDocument();
    expect(fixture.core.calls.some(c => c.cmd === "detail.imagePreview")).toBe(false);
  });
  // spec: UI-111
  it("publishes an address navigation request for the top table to resolve Internal/External", async () => {
    const listener = vi.fn(); document.addEventListener(DETAIL_NAVIGATE_EVENT, listener);
    render(<DetailFrame selection={selection} />); await settle(); fireEvent.click(screen.getByRole("button", { name: "https://site.test/final" }));
    expect(listener.mock.calls[0][0].detail).toEqual({ address: "https://site.test/final", tab: "auto" }); document.removeEventListener(DETAIL_NAVIGATE_EVENT, listener);
  });
});
