import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Providers } from "../../../app/Providers";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { SegmentRow } from "../../../ipc/gen/SegmentRow";
import type { SegmentsViewArgs } from "../../../ipc/gen/SegmentsViewArgs";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fixture from "../../../../e2e/sidebar-fach/fach-dto.json";
import { SidebarFrame } from "../frame/SidebarFrame";
import { useSidebarNavigation } from "../frame/state";
import { useSegmentsDialog } from "../../segments-dialog/store";
import { classValue, columnsOf, coreView, orderRows, segmented } from "./model";

const rowsOf = fixture.segmentRows as unknown as Record<string, SegmentRow[]>;
let core: MockCore;
let variant: keyof typeof rowsOf = "indexability";

beforeEach(async () => {
  variant = "indexability";
  core = new MockCore();
  core.handlers.set("sidebar.overview", () => ({ summary: [], tabs: [] }));
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  core.handlers.set("sidebar.segments", args => {
    const view = (args as SegmentsViewArgs).view;
    return variant === "empty" ? rowsOf.empty : rowsOf[view] ?? rowsOf.indexability;
  });
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  useSegmentsDialog.setState({ open: false });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });

async function open() {
  render(<Providers><SidebarFrame /></Providers>);
  fireEvent.click(await screen.findByRole("tab", { name: "Segments" }));
}

// spec: UI-067
it("derives % Segmented from all internal URLs, keeps “no segment” last and maps views to the core aggregate", () => {
  const rows = orderRows([...rowsOf.indexability!].reverse());
  expect(rows.at(-1)?.name).toBeNull();
  const share = segmented(rows);
  expect(share.total).toBe(40);
  expect(share.pct(rows.find(r => r.name === "Blog")!)).toBe(25);
  expect(rows.reduce((s, r) => s + share.pct(r), 0)).toBeCloseTo(100, 0);
  expect(columnsOf("overview").map(c => c.header)).toEqual(["Indexable", "Non-Indexable", "Issues", "Warnings", "Opportunities", "High", "Medium", "Low"]);
  expect(columnsOf("issuePriority").map(c => c.header)).toEqual(["High", "Medium", "Low"]);
  expect(columnsOf("crawlDepth").map(c => c.header)).toEqual(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10+"]);
  expect(classValue(rowsOf.crawlDepth![0]!, "10+")).toBe(2);
  expect(classValue(rowsOf.indexability![0]!, "Nonexistent")).toBeNull();
  expect(["overview", "issuePriority", "indexability"].map(v => coreView(v as never))).toEqual(["indexability", "indexability", "indexability"]);
  expect(coreView("crawlDepth")).toBe("crawlDepth");
});

// spec: UI-067
it("shows the default view with the specified columns and a last row “No segment”", async () => {
  await open();
  const table = await screen.findByRole("table", { name: "Key figures per segment" });
  expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Segment", "URLs", "% Segmented", "Indexable", "Non-Indexable", "Issues", "Warnings", "Opportunities", "High", "Medium", "Low"]);
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows.map(r => within(r).getAllByRole("rowheader")[0]?.textContent)).toEqual(["Blog", "Thin pages", "Rest", "No segment"]);
  expect(rows[0]).toHaveTextContent("25.0 %");
  expect(rows[0]).toHaveTextContent("10");
});

// spec: UI-067
it("sets the global segment filter from a click, also for “No segment”, and can clear it again", async () => {
  await open();
  fireEvent.click(await screen.findByRole("button", { name: "Filter by segment: Blog" }));
  expect(useSidebarNavigation.getState().scope.segment).toEqual({ kind: "segment", name: "Blog" });
  fireEvent.click(screen.getByRole("button", { name: "Filter by segment: No segment" }));
  expect(useSidebarNavigation.getState().scope.segment).toEqual({ kind: "none" });
  fireEvent.click(screen.getByRole("button", { name: "Clear segment filter" }));
  expect(useSidebarNavigation.getState().scope.segment).toBeUndefined();
});

// spec: UI-067
it("switches the view: crawl depth 0…10+, response codes and issues by priority", async () => {
  await open();
  const table = await screen.findByRole("table", { name: "Key figures per segment" });
  fireEvent.change(screen.getByLabelText("View"), { target: { value: "crawlDepth" } });
  await waitFor(() => expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toContain("10+"));
  expect(core.calls.some(c => c.cmd === "sidebar.segments" && (c.args as SegmentsViewArgs).view === "crawlDepth")).toBe(true);
  const blog = within(table).getByRole("row", { name: /Blog/ });
  expect(within(blog).getAllByRole("cell").map(c => c.textContent)).toEqual(["10", "33.3 %", "0", "4", "3", "1", "–", "–", "–", "–", "–", "–", "2"]);
  fireEvent.change(screen.getByLabelText("View"), { target: { value: "responseCodes" } });
  await waitFor(() => expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toContain("4xx"));
  fireEvent.change(screen.getByLabelText("View"), { target: { value: "issuePriority" } });
  await waitFor(() => expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Segment", "URLs", "% Segmented", "High", "Medium", "Low"]));
});

// spec: UI-067
it("opens the segment dialog from the button and offers “Create segments” in the empty state", async () => {
  variant = "empty";
  await open();
  expect(await screen.findByText(/No segments are configured yet/)).toBeVisible();
  expect(screen.queryByRole("table", { name: "Key figures per segment" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Create segments" }));
  expect(useSegmentsDialog.getState().open).toBe(true);
  useSegmentsDialog.setState({ open: false });
  fireEvent.click(screen.getByRole("button", { name: "Configure segments" }));
  expect(useSegmentsDialog.getState().open).toBe(true);
});
